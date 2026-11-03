#!/usr/bin/env bash
# Y6 — prova için kötü ağ profilleri, tek komut (SPRINT_BACKLOG Y6; INT-03 / DEMO-01 provası).
#
#   sudo scripts/netem/prova.sh kotu     [--dev <arayüz>] [--port <udp>]
#   sudo scripts/netem/prova.sh kesinti  [--dev <arayüz>] [--port <udp>] [--sure <s>]
#   sudo scripts/netem/prova.sh iyi      [--dev <arayüz>]
#        scripts/netem/prova.sh durum    [--dev <arayüz>]
#
# Profiller (sayılar SPRINT_BACKLOG'dan; yeni sayı yok):
#   iyi      bozma yok — bu betiğin kurduğu her şeyi kaldırır (arayüz kendi hâline döner).
#   kotu     netem `delay 200ms loss 30%` (A6/INT-03 metni).
#   kesinti  `loss 100%`, varsayılan 60 s; süre dolunca (ya da Ctrl-C) ÖNCEKİ profil aynen geri gelir
#            (kotu açıktıysa kotu, değilse iyi). Başlangıç/bitiş anı UTC olarak yazılır (INT-03 (4) ölçümü).
#
# Neyi bozar — tek kural: UDP `--port`'a (varsayılan 47000, hub portu) GİDEN ya da oradan GELEN her paket,
# `--dev` arayüzünü her geçişinde profilden BİR kez geçer. Başka trafik (SSH, adb, DNS…) dokunulmaz.
#   - Gerçek arayüz (Wi-Fi/Ethernet): çıkış yönü kök `prio` qdisc'inin 4. bandındaki netem'den, giriş yönü
#     ingress → ifb (`kama-ifb0`) üstündeki netem'den geçer. Laptop hub da olsa (Telefon1 ↔ Laptop ↔
#     Telefon2), istemci de olsa (INT-03 düğüm A, hub VPS'te) aynı komut doğru yönleri bozar.
#   - `--dev lo` (her şey tek makinede, INT-02H): lo'da her paket bir kez geçer. Uç → hub ve hub → uç ayrı
#     iki geçiştir: telefondan telefona yolda gecikme iki kez (≈ 400 ms) ve kayıp iki kez (1 − 0,7² ≈ %51)
#     uygulanır. Gerçek arayüzde de laptop hub ise iki telefonun her biri bir kez bozulur — aynı sonuç.
# netem kuyruğu varsayılan 1000 paket: 200 ms gecikmede ~5000 paket/s üstünde netem kendisi ek kayıp verir.
#
# Root (ya da CAP_NET_ADMIN) ister; kökte varsayılan olmayan bir qdisc varsa dokunmadan durur (çıkış 3).
# Öz-test: scripts/netem/prova_selftest.sh (root gerekmez, iki ağ ad alanında ölçer).
set -Eeuo pipefail

readonly IFB=kama-ifb0
readonly BAND=1:4           # prio'nun 4. bandı: yalnız filtrenin seçtiği paketler (priomap 0–2 kullanır)
readonly NETEM_HANDLE=40:
readonly KOTU_ARGS="delay 200ms loss 30%"
readonly KESINTI_ARGS="loss 100%"

usage() {
  awk 'NR > 1 && /^set -/ {exit} NR > 1 {sub(/^# ?/, ""); print}' "$0"
  exit 2
}

die() { echo "prova: $*" >&2; exit "${2:-1}"; }

cmd=${1:-}
[ -n "$cmd" ] || usage
shift
dev="" port=47000 sure=60
while [ $# -gt 0 ]; do
  case $1 in
    --dev) dev=${2:?--dev değer ister}; shift 2 ;;
    --port) port=${2:?--port değer ister}; shift 2 ;;
    --sure) sure=${2:?--sure değer ister}; shift 2 ;;
    -h|--help) usage ;;
    *) die "bilinmeyen argüman: $1" 2 ;;
  esac
done
case $port in ''|*[!0-9]*) die "--port sayı olmalı: $port" 2 ;; esac
[ "$port" -ge 1 ] && [ "$port" -le 65535 ] || die "--port 1–65535: $port" 2
case $sure in ''|*[!0-9]*) die "--sure saniye (tam sayı) olmalı: $sure" 2 ;; esac

if [ -z "$dev" ]; then
  # Varsayılan rotanın arayüzü (telefonlar aynı Wi-Fi'deyse doğru arayüz budur).
  route=$(ip -o route get 192.0.2.1 2>/dev/null || true)
  dev=$(printf '%s\n' "$route" | awk '{for (i = 1; i < NF; i++) if ($i == "dev" && !d) {print $(i + 1); d = 1}}')
  [ -n "$dev" ] || die "varsayılan rota yok; arayüzü --dev ile verin (tek makinede: --dev lo)" 2
  echo "prova: arayüz $dev (varsayılan rota; başka arayüz için --dev)"
fi
ip link show dev "$dev" >/dev/null 2>&1 || die "arayüz yok: $dev" 2

utc() { date -u +%Y-%m-%dT%H:%M:%S.%3NZ; }

# Kök qdisc'in türü ve tutamacı: "prio 1:" gibi.
root_qdisc() {
  tc qdisc show dev "$1" root | awk 'NR == 1 {print $2, $3}'
}

# Bizim kurduğumuz düzen mi? (kök prio 1: + 4. bantta netem 40:) — çıktı önce değişkene alınır:
# `tc | grep -q` pipefail altında SIGPIPE yarışıdır.
ours() {
  local q
  q=$(tc qdisc show dev "$dev")
  [ "$(root_qdisc "$dev")" = "prio 1:" ] && grep -q "^qdisc netem $NETEM_HANDLE parent $BAND " <<<"$q"
}

# Yarım kalmış kurulum dahil, kökte bizim tutamacımızla bir prio var mı?
ours_root() { [ "$(root_qdisc "$dev")" = "prio 1:" ]; }

# Giriş yönünde bizim ifb'ye yönlendiren bir filtre var mı? (tc: "Egress Redirect to device …")
ours_ingress() {
  local f
  f=$(tc filter show dev "$dev" ingress 2>/dev/null || true)
  grep -q "to device $IFB" <<<"$f"
}

# Kökte başkasının bir düzeni varsa dokunma.
guard() {
  ours && return 0
  local kind
  kind=$(root_qdisc "$dev" | awk '{print $1}')
  case $kind in
    ''|noqueue|mq|fq_codel|pfifo_fast|fq|cake|pfifo) ;;
    *) die "$dev kökünde '$kind' qdisc'i var (bu betiğin değil); dokunulmadı" 3 ;;
  esac
  local i
  i=$(tc qdisc show dev "$dev" ingress)
  if [ -n "$i" ] && ! ours_ingress; then
    die "$dev üzerinde zaten bir ingress qdisc'i var (bu betiğin değil); dokunulmadı" 3
  fi
}

# Etkin netem argümanları ("delay 200ms loss 30%"), yoksa boş. `limit N`'den sonrası alınır.
active_args() {
  ours || return 0
  tc qdisc show dev "$dev" | awk -v h="$NETEM_HANDLE" -v b="$BAND" '
    $1 == "qdisc" && $2 == "netem" && $3 == h && $5 == b {
      out = ""; start = 0
      for (i = 6; i <= NF; i++) {
        if (start) out = out (out == "" ? "" : " ") $i
        if ($i == "limit") { i++; start = 1 }
      }
      if (!seen) { print out; seen = 1 }
    }'
}

# Bu betiğin kurduğu her şeyi kaldırır; yarım kalmış kurulumu da (hata vermez).
clear_all() {
  if ours_root; then tc qdisc del dev "$dev" root || true; fi
  if [ "$dev" != lo ]; then
    if ours_ingress; then tc qdisc del dev "$dev" ingress || true; fi
    if ip link show dev "$IFB" >/dev/null 2>&1; then ip link del dev "$IFB" || true; fi
  fi
}

# u32 eşlemeleri: IPv4 ve IPv6, UDP, kaynak ya da hedef port.
add_filters() { # <dev> <parent> <eylem...>
  local d=$1 parent=$2; shift 2
  local p=10
  for side in sport dport; do
    tc filter add dev "$d" parent "$parent" protocol ip prio "$p" u32 \
      match ip protocol 17 0xff match ip "$side" "$port" 0xffff "$@"
    tc filter add dev "$d" parent "$parent" protocol ipv6 prio "$((p + 1))" u32 \
      match ip6 protocol 17 0xff match ip6 "$side" "$port" 0xffff "$@"
    p=$((p + 2))
  done
}

install() { # <netem argümanları>
  local args=$1
  clear_all
  # Bir adım düşerse (modül yok, yetki yok…) kurulanı geri al, yarım düzen bırakma.
  trap 'trap - ERR; clear_all; die "kurulum yarıda kaldı ($dev); geri alındı"' ERR
  tc qdisc add dev "$dev" root handle 1: prio bands 4 priomap 1 2 2 2 1 2 0 0 1 1 1 1 1 1 1 1
  # shellcheck disable=SC2086 # argümanlar bilinçli olarak sözcüklere bölünür
  tc qdisc add dev "$dev" parent "$BAND" handle "$NETEM_HANDLE" netem $args
  add_filters "$dev" 1: flowid "$BAND"
  if [ "$dev" != lo ]; then
    ip link add dev "$IFB" type ifb
    ip link set dev "$IFB" up
    # shellcheck disable=SC2086
    tc qdisc add dev "$IFB" root handle "$NETEM_HANDLE" netem $args
    tc qdisc add dev "$dev" handle ffff: ingress
    add_filters "$dev" ffff: action mirred egress redirect dev "$IFB"
  fi
  trap - ERR
}

retune() { # <netem argümanları> — yapı yerinde, yalnız netem değişir
  # shellcheck disable=SC2086
  tc qdisc change dev "$dev" parent "$BAND" handle "$NETEM_HANDLE" netem $1
  if [ "$dev" != lo ]; then
    # shellcheck disable=SC2086
    tc qdisc change dev "$IFB" root handle "$NETEM_HANDLE" netem $1
  fi
}

show() {
  local args
  args=$(active_args)
  if [ -z "$args" ]; then
    echo "profil: iyi ($dev üzerinde bu betiğin kurduğu bir şey yok)"
  else
    echo "profil: netem '$args' · arayüz $dev · UDP port $port (giden + gelen)"
  fi
  tc -s qdisc show dev "$dev"
  if [ "$dev" != lo ] && ip link show dev "$IFB" >/dev/null 2>&1; then
    tc -s qdisc show dev "$IFB"
  fi
}

case $cmd in
  durum)
    show
    ;;
  iyi)
    clear_all
    echo "prova: $(utc) iyi — $dev üzerinde bozma yok"
    ;;
  kotu)
    guard
    args=$KOTU_ARGS
    install "$args"
    echo "prova: $(utc) kotu — netem '$args' · $dev · UDP $port (giden + gelen)"
    ;;
  kesinti)
    guard
    before=$(active_args)
    if [ -n "$before" ]; then retune "$KESINTI_ARGS"; else install "$KESINTI_ARGS"; fi
    restore() {
      trap - INT TERM
      if [ -n "$before" ]; then retune "$before"; else clear_all; fi
      echo "KESINTI_BITTI $(utc) sure_s=$((SECONDS - started)) geri=${before:-iyi}"
    }
    trap 'restore; exit 130' INT TERM
    started=$SECONDS
    echo "KESINTI_BASLADI $(utc) sure_s=$sure · $dev · UDP $port (giden + gelen) · Ctrl-C erken bitirir"
    # Arka planda uyu + wait: TERM/INT tuzağı uyku bitmeden hemen çalışır.
    sleep "$sure" &
    wait $!
    restore
    ;;
  *)
    usage
    ;;
esac
