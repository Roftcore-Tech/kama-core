#!/usr/bin/env bash
# Y6 öz-testi: scripts/netem/prova.sh profilleri gerçekten uyguluyor mu — ÖLÇEREK (root gerekmez).
#
#   scripts/netem/prova_selftest.sh            # iki kip: lo + gerçek arayüz (veth, iki ağ ad alanı)
#
# Yetkisiz kullanıcı ad alanında (`unshare -rn`) koşar; makinenin ağına dokunmaz. Root ile çağrılırsa
# `unshare -n` kullanır. Her kipte UDP sonda (400 paket, 10 ms aralık) + yankı sunucusu:
#   iyi      → kayıp 0, RTT < 20 ms
#   kotu     → hub portunda gidiş-dönüş başarı 0,49 ± 0,10 (0,7²), medyan RTT 400 ± 40 ms (iki geçiş × 200);
#              yön başına netem sayacı kayıp 0,30 ± 0,08 (yalnız gerçek arayüz kipi: çıkış + ifb);
#              port+1'deki trafik bozulmaz (kayıp 0, RTT < 20 ms); lo kipinde IPv6 (::1) aynı beklenti
#   kesinti  → kotu açıkken 3 s: süre içindeki sondalar 0/N; süre dolunca kotu aynen geri (tc dökümü)
#              iyi açıkken 2 s: süre dolunca hiçbir yapı kalmaz
#   iyi      → kök prio, ingress, ifb kalmaz; kayıp 0
#   koruma   → kökte yabancı qdisc varken `kotu` çıkış 3 ve qdisc değişmez
# Çıkış: 0 = hepsi geçti, 1 = en az bir kontrol düştü, 2 = ortam (unshare/tc/python3/modül) yok.
set -uo pipefail

SELF=$(readlink -f "$0")
DIR=$(dirname "$SELF")
PROVA="$DIR/prova.sh"
PORT=47000
CTRL=$((PORT + 1))

if [ "${KAMA_PROVA_INNER:-}" != 1 ]; then
  for t in unshare ip tc python3 nsenter; do
    command -v "$t" >/dev/null 2>&1 || { echo "ORTAM: $t yok" >&2; exit 2; }
  done
  if [ "$(id -u)" = 0 ]; then flags=(-n); else flags=(-r -n); fi
  KAMA_PROVA_INNER=1 exec unshare "${flags[@]}" -- bash "$SELF" "$@"
fi

WORK=$(mktemp -d)
FAIL=0
PIDS=()
cleanup() {
  for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null; done
  rm -rf "$WORK"
}
trap cleanup EXIT

ok() { echo "  GECTI  $*"; }
bad() { echo "  DUSTU  $*"; FAIL=1; }

# ── yardımcı Python: yankı sunucusu ve sonda ────────────────────────────────────────────────────────
cat >"$WORK/echo.py" <<'PY'
import socket, sys
host, port = sys.argv[1], int(sys.argv[2])
s = socket.socket(socket.AF_INET6 if ":" in host else socket.AF_INET, socket.SOCK_DGRAM)
s.bind((host, port))
while True:
    data, peer = s.recvfrom(2048)
    s.sendto(data, peer)
PY
cat >"$WORK/probe.py" <<'PY'
# sonda <host> <port> <adet> <aralık_s> <kuyruk_s> → "sent= recv= ok= med_ms= p10_ms= p90_ms="
import socket, struct, sys, time, select
host, port, n, gap, tail = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]), float(sys.argv[5])
s = socket.socket(socket.AF_INET6 if ":" in host else socket.AF_INET, socket.SOCK_DGRAM)
s.setblocking(False)
t0 = time.monotonic()
i, got, rtts = 0, set(), []
end = None
while True:
    now = time.monotonic()
    if i < n and now - t0 >= i * gap:
        s.sendto(struct.pack("!Id", i, now) + b"\0" * 152, (host, port))
        i += 1
        continue
    if i >= n and end is None:
        end = now + tail
    if end is not None and now >= end:
        break
    wait = (t0 + i * gap - now) if i < n else (end - now)
    r, _, _ = select.select([s], [], [], max(0.0, wait))
    while r:
        try:
            data, _ = s.recvfrom(2048)
        except BlockingIOError:
            break
        seq, ts = struct.unpack("!Id", data[:12])
        if seq not in got:
            got.add(seq)
            rtts.append((time.monotonic() - ts) * 1000)
rtts.sort()
q = lambda p: round(rtts[int((len(rtts) - 1) * p)], 1) if rtts else -1
print(f"sent={n} recv={len(got)} ok={len(got)/n:.3f} med_ms={q(0.5)} p10_ms={q(0.1)} p90_ms={q(0.9)}")
PY

field() { # <satır> <ad>
  local v
  for kv in $1; do v=${kv#"$2="}; [ "$v" != "$kv" ] && { echo "$v"; return; }; done
}
fcmp() { python3 -c "import sys; a,op,b=float(sys.argv[1]),sys.argv[2],float(sys.argv[3]); sys.exit(0 if {'<':a<b,'>':a>b,'<=':a<=b,'>=':a>=b}[op] else 1)" "$1" "$2" "$3"; }

# probe <etiket> <çalıştırıcı-öneki> <host> <port> [adet] [kuyruk]
probe() {
  local label=$1 runner=$2 host=$3 port=$4 n=${5:-400} tail=${6:-1.2}
  # shellcheck disable=SC2086
  $runner python3 "$WORK/probe.py" "$host" "$port" "$n" 0.01 "$tail" >"$WORK/p.out" 2>&1
  local line
  line=$(sed -n '1p' "$WORK/p.out")
  echo "    [$label] $line" >&2
  echo "$line"
}

expect_clean() { # <etiket> <satır>
  local okv med
  okv=$(field "$2" ok); med=$(field "$2" med_ms)
  if [ "$okv" = "1.000" ] && fcmp "$med" '<' 20; then ok "$1: kayıp 0, medyan ${med} ms"
  else bad "$1: beklenen kayıp 0 ve < 20 ms; ölçülen ok=$okv medyan=$med"; fi
}

expect_bad() { # <etiket> <satır>
  local okv med
  okv=$(field "$2" ok); med=$(field "$2" med_ms)
  if fcmp "$okv" '>=' 0.39 && fcmp "$okv" '<=' 0.59 && fcmp "$med" '>=' 360 && fcmp "$med" '<=' 440; then
    ok "$1: gidiş-dönüş başarı $okv (beklenen 0,49 ± 0,10), medyan ${med} ms (400 ± 40)"
  else bad "$1: beklenen başarı 0,39–0,59 ve medyan 360–440 ms; ölçülen ok=$okv medyan=$med"; fi
}

# netem qdisc sayacından geçiş başına kayıp: dropped / (sent + dropped)
netem_loss() { # <tc -s çıktısı dosyası> <tutamak> <parent-alanı>
  awk -v h="$2" -v par="$3" '
    $1 == "qdisc" && $2 == "netem" { cur = ($3 == h && ($4 == par || $5 == par)) }
    cur && $1 == "Sent" { pk = $4; gsub(/[(,]/, "", $7); dr = $7; if (!done) { printf "%.3f %d %d\n", (pk + dr ? dr / (pk + dr) : 0), pk, dr; done = 1 } }
  ' "$1"
}

structure_gone() { # <arayüz> <çalıştırıcı>
  local d=$1 runner=$2 root ing ifb
  # shellcheck disable=SC2086
  root=$($runner tc qdisc show dev "$d" root | awk 'NR == 1 {print $2, $3}')
  # shellcheck disable=SC2086
  ing=$($runner tc qdisc show dev "$d" ingress)
  # shellcheck disable=SC2086
  ifb=$($runner ip -o link show dev kama-ifb0 2>/dev/null)
  if [ "$root" != "prio 1:" ] && [ -z "$ing" ] && [ -z "$ifb" ]; then ok "iyi: $d üzerinde kök prio / ingress / ifb kalmadı (kök: ${root:-yok})"
  else bad "iyi: artık yapı var — kök='$root' ingress='$ing' ifb='$ifb'"; fi
}

kesinti_check() { # <arayüz> <sonda çalıştırıcısı> <hedef> <önceki-profil-adı> <süre>
  local d=$1 runner=$2 host=$3 prev=$4 sure=$5
  bash "$PROVA" kesinti --dev "$d" --sure "$sure" >"$WORK/k.out" 2>&1 &
  local kp=$!
  local i
  for i in $(seq 1 100); do grep -q KESINTI_BASLADI "$WORK/k.out" 2>/dev/null && break; sleep 0.05; done
  local line
  line=$(probe "kesinti ($prev'dan, ${sure} s)" "$runner" "$host" "$PORT" 100 0.3)
  wait "$kp"; local rc=$?
  if [ "$(field "$line" recv)" = 0 ]; then ok "kesinti: süre içindeki sondalar 0/100"
  else bad "kesinti: süre içinde yanıt geldi: $line"; fi
  local endline
  endline=$(grep KESINTI_BITTI "$WORK/k.out" || true)
  if [ "$rc" = 0 ] && [ -n "$endline" ]; then ok "kesinti: çıkış 0, $(sed -n '1p' <<<"$endline" | cut -c1-90)"
  else bad "kesinti: çıkış $rc, çıktı: $(tr '\n' ' ' <"$WORK/k.out")"; fi
}

# ── kip koşucu ─────────────────────────────────────────────────────────────────────────────────────
run_mode() { # <ad> <arayüz> <çalıştırıcı (sonda tarafı)> <hub adresi> [ipv6 hub adresi]
  local name=$1 d=$2 runner=$3 hub=$4 hub6=${5:-}
  echo "== kip: $name (arayüz $d, hub $hub:$PORT)"
  local line

  line=$(probe "iyi" "$runner" "$hub" "$PORT"); expect_clean "iyi, port $PORT" "$line"

  if ! bash "$PROVA" kotu --dev "$d" >"$WORK/kotu.out" 2>&1; then
    bad "kotu kurulamadı: $(tr '\n' ' ' <"$WORK/kotu.out")"; return
  fi
  local st
  st=$(bash "$PROVA" durum --dev "$d" | sed -n '1p')
  if grep -q "netem 'delay 200ms loss 30%'" <<<"$st"; then ok "durum: $st"; else bad "durum beklenmedik: $st"; fi

  line=$(probe "kotu" "$runner" "$hub" "$PORT"); expect_bad "kotu, port $PORT" "$line"
  if [ -n "$hub6" ]; then
    line=$(probe "kotu ipv6" "$runner" "$hub6" "$PORT"); expect_bad "kotu, IPv6 [$hub6]:$PORT" "$line"
  fi
  line=$(probe "kontrol" "$runner" "$hub" "$CTRL"); expect_clean "kotu açıkken port $CTRL (dokunulmamalı)" "$line"

  if [ "$d" != lo ]; then
    tc -s qdisc show dev "$d" >"$WORK/eg.txt"; tc -s qdisc show dev kama-ifb0 >"$WORK/in.txt"
    local eg in
    eg=$(netem_loss "$WORK/eg.txt" 40: 1:4); in=$(netem_loss "$WORK/in.txt" 40: root)
    for pair in "çıkış (hub → uç):$eg" "giriş/ifb (uç → hub):$in"; do
      local lab=${pair%%:*} v=${pair#*:} r
      r=${v%% *}
      if [ -n "$r" ] && fcmp "$r" '>=' 0.22 && fcmp "$r" '<=' 0.38; then ok "yön başına kayıp, $lab: $r (geçen/düşen $(cut -d' ' -f2,3 <<<"$v" | tr ' ' /))"
      else bad "yön başına kayıp, $lab: beklenen 0,30 ± 0,08; ölçülen '${v}'"; fi
    done
  fi

  kesinti_check "$d" "$runner" "$hub" kotu 3
  st=$(bash "$PROVA" durum --dev "$d" | sed -n '1p')
  if grep -q "netem 'delay 200ms loss 30%'" <<<"$st"; then ok "kesinti sonrası kotu aynen geri: $st"
  else bad "kesinti sonrası profil yanlış: $st"; fi

  bash "$PROVA" iyi --dev "$d" >/dev/null 2>&1 || bad "iyi çıkışı sıfır değil"
  structure_gone "$d" ""
  line=$(probe "iyi sonrası" "$runner" "$hub" "$PORT"); expect_clean "iyi sonrası, port $PORT" "$line"

  kesinti_check "$d" "$runner" "$hub" iyi 2
  structure_gone "$d" ""

  # koruma: kökte yabancı qdisc
  tc qdisc add dev "$d" root handle 9: netem delay 1ms
  local before after rc
  before=$(tc qdisc show dev "$d")
  bash "$PROVA" kotu --dev "$d" >"$WORK/g.out" 2>&1; rc=$?
  after=$(tc qdisc show dev "$d")
  if [ "$rc" = 3 ] && [ "$before" = "$after" ]; then ok "koruma: yabancı kök qdisc → çıkış 3, dokunulmadı"
  else bad "koruma: çıkış $rc; önce '$before' sonra '$after'"; fi
  tc qdisc del dev "$d" root
}

# ── ortam ──────────────────────────────────────────────────────────────────────────────────────────
ip link set lo up
if ! tc qdisc add dev lo root handle 1: prio 2>/dev/null; then echo "ORTAM: tc qdisc (sch_prio) kurulamıyor" >&2; exit 2; fi
tc qdisc del dev lo root
echo "ortam: $(uname -r) · $(tc -V 2>&1 | sed -n '1p')"

# Kullanım hataları: argümansız, bilinmeyen argüman, varsayılan rota yokken --dev'siz → çıkış 2.
echo "== kullanım"
for args in "" "kotu --bilinmeyen" "kotu --port 70000" "durum"; do
  # shellcheck disable=SC2086
  bash "$PROVA" $args >/dev/null 2>&1; rc=$?
  if [ "$rc" = 2 ]; then ok "'prova.sh $args' → çıkış 2"; else bad "'prova.sh $args' → çıkış $rc (beklenen 2)"; fi
done

# Kip 1 — lo (her şey tek makinede, INT-02H)
python3 "$WORK/echo.py" 127.0.0.1 "$PORT" & PIDS+=($!)
python3 "$WORK/echo.py" 127.0.0.1 "$CTRL" & PIDS+=($!)
python3 "$WORK/echo.py" ::1 "$PORT" & PIDS+=($!)
sleep 0.3
run_mode "lo" lo "" 127.0.0.1 ::1

# Kip 2 — gerçek arayüz: bu ad alanı = laptop (hub, v0 10.77.0.1); çocuk ad alanı = telefon (v1 10.77.0.2)
ip link add v0 type veth peer name v1
unshare -n -- sleep 900 & CPID=$!; PIDS+=("$CPID")
for _ in $(seq 1 50); do [ -e "/proc/$CPID/ns/net" ] && break; sleep 0.05; done
ip link set v1 netns "$CPID"
ip addr add 10.77.0.1/24 dev v0; ip link set v0 up
PHONE="nsenter -t $CPID -n"
$PHONE ip addr add 10.77.0.2/24 dev v1
$PHONE ip link set v1 up; $PHONE ip link set lo up
python3 "$WORK/echo.py" 10.77.0.1 "$PORT" & PIDS+=($!)
python3 "$WORK/echo.py" 10.77.0.1 "$CTRL" & PIDS+=($!)
sleep 0.5
run_mode "gerçek arayüz (veth + ifb)" v0 "$PHONE" 10.77.0.1

echo
if [ "$FAIL" = 0 ]; then echo "PROVA-SELFTEST: GECTI"; else echo "PROVA-SELFTEST: DUSTU"; fi
exit "$FAIL"
