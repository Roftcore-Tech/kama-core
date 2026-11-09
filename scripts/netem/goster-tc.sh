#!/usr/bin/env bash
# [YOL3 · Ö] Gösterim `tc` betiği (ISEMRI-YOL3-Ö §8.5, KK-Ö-17/18; sahip 2026-11-09).
# `scripts/netem/prova.sh`'den (#142) BAĞIMSIZ. YALNIZ ÇIKIŞ, porta filtreli, delay/loss/rate
# parametrik; IFB yok, ingress yok, jitter seçeneği yok. Her uç yalnız kendi çıkışını bozar →
# her yön bir kez (K7). Yeni bozulma modeli YOK: `netem delay <D>ms loss <p>% rate <r>kbit`.
#
#   sudo goster-tc.sh up     --dev <if> --port <udp> --match sport|dport|both \
#                            --delay-ms <D> --loss-pct <p> --rate-kbit <r> [--per-dst <ip> ...]
#   sudo goster-tc.sh down   --dev <if>
#        goster-tc.sh status --dev <if>            # tc -s qdisc/filter + sahiplik kaydı
#        goster-tc.sh taban  --peer <ip> [-c 50]   # ping RTT medyanı (ms), D = round((300 − M)/2)
#
# Kurulan yapı (up): kök `prio bands B priomap 1 2 2 2 1 2 0 0 1 1 1 1 1 1 1 1` (handle 1:);
# B = 4 (tek sınıf) ya da 3 + N (`--per-dst` N hedef, en çok 8). Sınıf 1:(4+i) altında
# `netem` handle (4+i)0: — tek sınıfta 1:4 / 40:. u32 filtreleri: `ip protocol 17` ve
# sport/dport = --port (`both` → iki filtre), `--per-dst` varsa ayrıca `ip dst <ip>/32`.
# Eşleşmeyen trafik (SSH, köprü portları, DNS) priomap bantlarından bozulmadan geçer.
# Yalnız IPv4 (u32 `ip` eşleşmesi).
#
# Kurallar (her biri goster-tc-selftest.sh'de bir vaka):
#   1 yalnız çıkış · 2 port filtresi · 3 parametreler zorunlu, varsayılan sayı yok, jitter yok ·
#   4 yabancı kök korunur: kök qdisc varsayılan değilse (handle 0: değil) hiçbir şeye dokunmadan
#     çıkış 3; kör `tc qdisc del … root` yok · 5 sahiplik kaydı arayüze bağlı
#     ($KAMA_GOSTER_TC_RUN/<dev>.state); down yalnız kayıt varsa VE kök kayıttakiyle aynıysa siler,
#     değilse dokunmadan çıkış 3 · 6 işlem kilidi (flock $KAMA_GOSTER_TC_LOCK; alınamazsa çıkış 2) ·
#   7 aynı arayüzde ikinci up durur (çıkış 3; önce down).
# Çıkış kodları: 0 tamam · 1 kullanım/parametre · 2 kilit · 3 reddedildi (dokunulmadı) · 4 tc hatası
# (up yarıda kaldıysa yalnız kendi kurduğu kök geri alınır).
# Ortam (öz-test için; varsayılanlar gerçek kullanım): KAMA_GOSTER_TC_RUN=/run/kama-goster-tc,
# KAMA_GOSTER_TC_LOCK=/run/kama-goster-tc.lock.
# Ayrıcalıklı komut: up/down'u İNSAN koşar (sudo); ajan koşmaz. status/taban ayrıcalık istemez.
set -u
export LC_ALL=C

RUN_DIR="${KAMA_GOSTER_TC_RUN:-/run/kama-goster-tc}"
LOCK="${KAMA_GOSTER_TC_LOCK:-/run/kama-goster-tc.lock}"
PRIOMAP="1 2 2 2 1 2 0 0 1 1 1 1 1 1 1 1"
MAX_DST=8
TC="${KAMA_GOSTER_TC_TC:-tc}"

die() { local rc="$1"; shift; echo "goster-tc: $*" >&2; exit "$rc"; }
usage() { sed -n '7,11p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 1; }

is_uint() { [[ "$1" =~ ^[0-9]+$ ]]; }
is_ipv4() {
  [[ "$1" =~ ^([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})$ ]] || return 1
  local o; for o in "${BASH_REMATCH[@]:1}"; do [ "$o" -le 255 ] || return 1; done
}

root_line() { "$TC" qdisc show dev "$DEV" 2>/dev/null | awk '$0 ~ / root( |$)/ {print; exit}'; }
root_handle() { root_line | awk '{print $3}'; }

state_file() { printf '%s/%s.state' "$RUN_DIR" "$DEV"; }

need_dev() {
  [ -n "${DEV:-}" ] || die 1 "--dev zorunlu"
  [[ "$DEV" =~ ^[A-Za-z0-9_.:@-]{1,15}$ ]] || die 1 "geçersiz arayüz adı: $DEV"
  ip link show dev "$DEV" >/dev/null 2>&1 || die 1 "arayüz yok: $DEV"
}

take_lock() {
  mkdir -p "$(dirname "$LOCK")" 2>/dev/null
  exec 9>>"$LOCK" || die 2 "kilit dosyası açılamadı: $LOCK"
  flock -n 9 || die 2 "kilit alınamadı ($LOCK): başka bir up/down sürüyor"
}

# Kayıttaki beklenen qdisc'lerin tc çıktısıyla aynı olup olmadığı (kök prio 1: + bands + netem'ler).
state_matches() {
  local sf="$1" want_bands want_children line kind handle parent bands
  want_bands=$(sed -n 's/^bands=//p' "$sf")
  want_children=$(sed -n 's/^netem=//p' "$sf")
  [ -n "$want_bands" ] && [ -n "$want_children" ] || return 1
  line=$(root_line)
  kind=$(awk '{print $2}' <<< "$line"); handle=$(awk '{print $3}' <<< "$line")
  bands=$(sed -n 's/.* bands \([0-9]*\).*/\1/p' <<< "$line")
  [ "$kind" = prio ] && [ "$handle" = "1:" ] && [ "$bands" = "$want_bands" ] || return 1
  local pair cls nh
  for pair in $want_children; do
    cls="${pair%%=*}"; nh="${pair#*=}"
    "$TC" qdisc show dev "$DEV" | awk -v h="$nh" -v p="$cls" \
      '$2=="netem" && $3==h && $4=="parent" && $5==p {f=1} END {exit !f}' || return 1
  done
  # Kayıtta olmayan başka bir alt qdisc (netem) varsa kök artık bizim kurduğumuz değildir.
  local n_have n_want
  n_have=$("$TC" qdisc show dev "$DEV" | awk '$2=="netem"' | wc -l)
  n_want=$(wc -w <<< "$want_children")
  [ "$n_have" -eq "$n_want" ]
}

cmd_up() {
  local port="" match="" delay="" loss="" rate="" dsts=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --dev) DEV="${2:-}"; shift 2 ;;
      --port) port="${2:-}"; shift 2 ;;
      --match) match="${2:-}"; shift 2 ;;
      --delay-ms) delay="${2:-}"; shift 2 ;;
      --loss-pct) loss="${2:-}"; shift 2 ;;
      --rate-kbit) rate="${2:-}"; shift 2 ;;
      --per-dst) dsts+=("${2:-}"); shift 2 ;;
      *) die 1 "up: bilinmeyen argüman: $1 (jitter seçeneği yoktur)" ;;
    esac
  done
  need_dev
  [ -n "$port" ] && [ -n "$match" ] && [ -n "$delay" ] && [ -n "$loss" ] && [ -n "$rate" ] \
    || die 1 "up: --port --match --delay-ms --loss-pct --rate-kbit zorunlu (varsayılan sayı yok)"
  is_uint "$port" && [ "$port" -ge 1 ] && [ "$port" -le 65535 ] || die 1 "--port 1..65535: $port"
  case "$match" in sport|dport|both) ;; *) die 1 "--match sport|dport|both: $match" ;; esac
  is_uint "$delay" && [ "$delay" -le 10000 ] || die 1 "--delay-ms 0..10000 tam sayı: $delay"
  [[ "$loss" =~ ^[0-9]{1,3}(\.[0-9]{1,3})?$ ]] && awk -v l="$loss" 'BEGIN{exit !(l<=100)}' \
    || die 1 "--loss-pct 0..100: $loss"
  is_uint "$rate" && [ "$rate" -ge 1 ] || die 1 "--rate-kbit ≥ 1 tam sayı: $rate"
  [ "${#dsts[@]}" -le "$MAX_DST" ] || die 1 "--per-dst en çok $MAX_DST"
  local d seen=" "
  for d in "${dsts[@]}"; do
    is_ipv4 "$d" || die 1 "--per-dst IPv4 değil: $d"
    case "$seen" in *" $d "*) die 1 "--per-dst iki kez: $d" ;; esac; seen="$seen$d "
  done

  take_lock
  local sf; sf=$(state_file)
  [ -e "$sf" ] && die 3 "$DEV: sahiplik kaydı var ($sf) — önce down (ikinci up reddedildi)"
  local rh; rh=$(root_handle)
  [ "$rh" = "0:" ] || die 3 "$DEV: kök qdisc varsayılan değil ($(root_line)) — dokunulmadı"

  local n=${#dsts[@]} bands=4
  [ "$n" -gt 0 ] && bands=$((3 + n))
  local netem="delay ${delay}ms loss ${loss}% rate ${rate}kbit"
  local children="" i cls nh
  fail() { # yalnız kendi kurduğumuz kök geri alınır
    local msg="$1"
    if [ "$(root_handle)" = "1:" ] && root_line | grep -q '^qdisc prio 1: root'; then
      "$TC" qdisc del dev "$DEV" root 2>/dev/null
    fi
    die 4 "$DEV: tc hatası ($msg) — kurulum geri alındı"
  }
  "$TC" qdisc add dev "$DEV" root handle 1: prio bands "$bands" priomap $PRIOMAP \
    || die 4 "$DEV: kök prio kurulamadı — dokunulmadı"
  local slots=1; [ "$n" -gt 0 ] && slots=$n
  for i in $(seq 0 $((slots - 1))); do
    cls="1:$((4 + i))"; nh="$(printf '%x' $((4 + i)))0:"
    # shellcheck disable=SC2086
    "$TC" qdisc add dev "$DEV" parent "$cls" handle "$nh" netem $netem || fail "netem $cls"
    local dst_m=()
    [ "$n" -gt 0 ] && dst_m=(match ip dst "${dsts[$i]}/32")
    local m
    for m in sport dport; do
      [ "$match" = both ] || [ "$match" = "$m" ] || continue
      "$TC" filter add dev "$DEV" parent 1: protocol ip prio 1 u32 \
        match ip protocol 17 0xff match ip "$m" "$port" 0xffff "${dst_m[@]}" flowid "$cls" \
        || fail "filtre $m $cls"
    done
    children="$children $cls=$nh"
  done
  mkdir -p "$RUN_DIR" || fail "kayıt dizini $RUN_DIR"
  {
    echo "dev=$DEV"
    echo "root=1:"
    echo "bands=$bands"
    echo "netem=${children# }"
    echo "params=port=$port match=$match delay_ms=$delay loss_pct=$loss rate_kbit=$rate"
    echo "per_dst=${dsts[*]:-}"
    echo "netem_dizgisi=$netem"
    echo "zaman=$(date -u +%FT%TZ)"
  } > "$sf.tmp" && mv -f "$sf.tmp" "$sf" || fail "kayıt yazılamadı $sf"
  echo "goster-tc: $DEV up · $netem · port $port ($match) · sınıf(lar) ${children# } · kayıt $sf"
}

cmd_down() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dev) DEV="${2:-}"; shift 2 ;;
      *) die 1 "down: bilinmeyen argüman: $1" ;;
    esac
  done
  need_dev
  take_lock
  local sf; sf=$(state_file)
  [ -e "$sf" ] || die 3 "$DEV: sahiplik kaydı yok ($sf) — dokunulmadı"
  state_matches "$sf" || die 3 "$DEV: kök kayıttakiyle aynı değil ($(root_line)) — dokunulmadı; kök varsayılansa kayıt bayattır: rm $sf"
  "$TC" qdisc del dev "$DEV" root || die 4 "$DEV: kök silinemedi"
  rm -f "$sf"
  echo "goster-tc: $DEV down · kök: $(root_line)"
}

cmd_status() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dev) DEV="${2:-}"; shift 2 ;;
      *) die 1 "status: bilinmeyen argüman: $1" ;;
    esac
  done
  need_dev
  local sf; sf=$(state_file)
  echo "== qdisc ($DEV)"; "$TC" -s qdisc show dev "$DEV"
  echo "== filter ($DEV)"; "$TC" -s filter show dev "$DEV" parent 1: 2>/dev/null || true
  if [ -e "$sf" ]; then
    echo "== kayıt $sf"; cat "$sf"
    if state_matches "$sf"; then echo "== kök kayıtla aynı"; else echo "== UYARI: kök kayıtla aynı değil"; fi
  else
    echo "== kayıt yok ($sf)"
  fi
}

cmd_taban() {
  local peer="" count=50
  while [ $# -gt 0 ]; do
    case "$1" in
      --peer) peer="${2:-}"; shift 2 ;;
      -c) count="${2:-}"; shift 2 ;;
      *) die 1 "taban: bilinmeyen argüman: $1" ;;
    esac
  done
  [ -n "$peer" ] || die 1 "taban: --peer zorunlu"
  is_uint "$count" && [ "$count" -ge 1 ] && [ "$count" -le 1000 ] || die 1 "-c 1..1000: $count"
  local out; out=$(ping -n -c "$count" "$peer" 2>&1) || true
  printf '%s\n' "$out" | taban_hesap "$peer"
}

# stdin: ping çıktısı → "taban_rtt_medyan_ms=<M> D_ms=<D> n=<k>" (medyan: k çiftse ortadaki ikinin
# ortalaması). D = round((300 − M)/2), M ≥ 300 → D = 0 + uyarı (v4 III.6).
taban_hesap() {
  local peer="$1"
  grep -o 'time=[0-9.]*' | cut -d= -f2 | sort -n | awk -v peer="$peer" '
    { v[NR] = $1 }
    END {
      if (NR == 0) { print "goster-tc: taban: " peer " yanıt yok" > "/dev/stderr"; exit 5 }
      m = (NR % 2) ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2
      d = (300 - m) / 2
      if (d < 0) {
        d = 0
        print "goster-tc: UYARI taban RTT " m " ms ≥ 300 ms: profil \"ölçülen taban + %20 kayıp\" (v4 III.6)" > "/dev/stderr"
      }
      printf "taban_rtt_medyan_ms=%.3f D_ms=%d n=%d peer=%s\n", m, int(d + 0.5), NR, peer
    }'
}

sub="${1:-}"; [ $# -gt 0 ] && shift
case "$sub" in
  up) cmd_up "$@" ;;
  down) cmd_down "$@" ;;
  status) cmd_status "$@" ;;
  taban) cmd_taban "$@" ;;
  _taban_hesap) taban_hesap "${1:-test}" ;;   # öz-test girişi (stdin = ping çıktısı)
  *) usage ;;
esac
