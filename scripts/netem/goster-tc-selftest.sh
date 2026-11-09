#!/usr/bin/env bash
# [YOL3 · Ö] goster-tc.sh öz-testi (ISEMRI-YOL3-Ö §8.5) — ROOTLESS: `unshare -rn` + iki ağ ad alanı
# + veth; kök/sudo gerekmez, makinenin arayüzlerine dokunmaz. Ölçüm değildir.
#
#   scripts/netem/goster-tc-selftest.sh          # çıkış 0 = bütün vakalar geçti
#
# Vakalar: (a) temiz arayüzde up → status → down gidiş-dönüşü, kök varsayılana döner, ingress/IFB
# yok · (b) yabancı kökte (prio 1:, netem) up dokunmadan durur · (c) kayıtsız down dokunmadan durur
# · (d) kayıtla uyuşmayan kökte down durur · (e) filtre: yalnız hedef port netem sınıfına düşer
# (dport ve sport), başka port (köprü 47201) düşmez; --per-dst hedef başına sınıf · (f) ikinci up
# durur · (g) kilit alınamazsa çıkış 2 · (p) parametreler zorunlu, jitter seçeneği yok · (t) taban
# medyanı ve D.
set -u
export LC_ALL=C
SELF="$(readlink -f "$0")"
GT="$(dirname "$SELF")/goster-tc.sh"

if [ -z "${GT_SELFTEST_INNER:-}" ]; then
  exec unshare -rn env GT_SELFTEST_INNER=1 bash "$SELF" "$@"
fi

T="$(mktemp -d "${TMPDIR:-/tmp}/goster-tc-selftest.XXXXXX")"
export KAMA_GOSTER_TC_RUN="$T/run" KAMA_GOSTER_TC_LOCK="$T/lock"
PEER=""
cleanup() { [ -n "$PEER" ] && kill "$PEER" 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT

ip link set lo up
unshare -n sleep 100000 & PEER=$!
for _ in $(seq 1 100); do [ -e "/proc/$PEER/ns/net" ] && break; sleep 0.02; done
ip link add va type veth peer name vb
ip link set vb netns "$PEER"
ip addr add 10.77.0.1/24 dev va
ip link set va up
nsenter -t "$PEER" -n ip link set lo up
nsenter -t "$PEER" -n ip addr add 10.77.0.2/24 dev vb
nsenter -t "$PEER" -n ip link set vb up
MAC_VB=$(nsenter -t "$PEER" -n ip link show vb | awk '/link\/ether/{print $2}')
ip neigh replace 10.77.0.2 lladdr "$MAC_VB" dev va nud permanent
ip neigh replace 10.77.0.3 lladdr 02:00:00:00:00:03 dev va nud permanent   # yalnız çıkış sayımı

PASS=0; FAIL=0
ok() { PASS=$((PASS + 1)); echo "  geçti: $1"; }
bad() { FAIL=$((FAIL + 1)); echo "  KALDI: $1"; }
check() { local what="$1"; shift; if "$@"; then ok "$what"; else bad "$what"; fi; }
rc_is() { [ "$1" = "$2" ]; }

root() { tc qdisc show dev va | awk '$0 ~ / root( |$)/ {print; exit}'; }
gt() { "$GT" "$@" >"$T/out" 2>"$T/err"; echo $?; }
UP=(up --dev va --port 47100 --match both --delay-ms 150 --loss-pct 20 --rate-kbit 100)
send() { # $1 hedef ip  $2 hedef port  $3 kaynak port (0 = rastgele)  $4 adet
  python3 -I -c '
import socket, sys
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
if int(sys.argv[3]):
    s.bind(("10.77.0.1", int(sys.argv[3])))
for _ in range(int(sys.argv[4])):
    s.sendto(b"k" * 32, (sys.argv[1], int(sys.argv[2])))
' "$@"
}
pkts() { # $1 netem handle → gönderilen + düşen paket
  tc -s qdisc show dev va | awk -v h="$1" '
    $1=="qdisc" { on = ($2=="netem" && $3==h); next }
    on && $1=="Sent" { gsub(/[(,]/, " "); for (i=1;i<=NF;i++) { if ($i=="pkt") s=$(i-1); if ($i=="dropped") d=$(i+1) } ; print s+d; exit }'
}
settle() { sleep 0.4; }

ROOT0="$(root)"
echo "== başlangıç kökü: $ROOT0"

echo "(a) gidiş-dönüş"
r=$(gt "${UP[@]}"); check "up rc 0" rc_is "$r" 0
check "kök prio 1: bands 4" grep -q '^qdisc prio 1: root .*bands 4 priomap 1 2 2 2 1 2 0 0 1 1 1 1 1 1 1 1' <<< "$(root)"
check "netem 40: parent 1:4 delay 150ms loss 20% rate 100Kbit" \
  grep -Eq '^qdisc netem 40: parent 1:4 .*delay 150ms loss 20% rate 100Kbit' <<< "$(tc qdisc show dev va)"
check "ingress qdisc yok" test -z "$(tc qdisc show dev va ingress)"
check "IFB arayüzü yok" test -z "$(ip -o link show type ifb 2>/dev/null)"
check "kayıt yazıldı" test -s "$KAMA_GOSTER_TC_RUN/va.state"
r=$(gt status --dev va); check "status rc 0" rc_is "$r" 0
check "status kökü kayıtla aynı der" grep -q 'kök kayıtla aynı' "$T/out"
r=$(gt down --dev va); check "down rc 0" rc_is "$r" 0
check "kök varsayılana döndü" rc_is "$(root)" "$ROOT0"
check "kayıt silindi" test ! -e "$KAMA_GOSTER_TC_RUN/va.state"

echo "(b) yabancı kök korunur"
tc qdisc add dev va root handle 1: prio
F="$(tc qdisc show dev va)"
r=$(gt "${UP[@]}"); check "yabancı prio 1: → up rc 3" rc_is "$r" 3
check "yabancı prio 1: değişmedi" rc_is "$(tc qdisc show dev va)" "$F"
check "kayıt yazılmadı" test ! -e "$KAMA_GOSTER_TC_RUN/va.state"
tc qdisc del dev va root
tc qdisc add dev va root netem delay 7ms
F="$(tc qdisc show dev va)"
r=$(gt "${UP[@]}"); check "yabancı netem kökü → up rc 3" rc_is "$r" 3
check "yabancı netem değişmedi" rc_is "$(tc qdisc show dev va)" "$F"

echo "(c) kayıtsız down"
r=$(gt down --dev va); check "yabancı kök + kayıt yok → down rc 3" rc_is "$r" 3
check "yabancı netem yine değişmedi" rc_is "$(tc qdisc show dev va)" "$F"
tc qdisc del dev va root
r=$(gt down --dev va); check "varsayılan kök + kayıt yok → down rc 3" rc_is "$r" 3
check "kök varsayılan kaldı" rc_is "$(root)" "$ROOT0"

echo "(d) kayıtla uyuşmayan kök"
r=$(gt "${UP[@]}"); check "up rc 0" rc_is "$r" 0
tc qdisc del dev va root
tc qdisc add dev va root handle 1: prio bands 3
F="$(tc qdisc show dev va)"
r=$(gt down --dev va); check "kök değişmiş → down rc 3" rc_is "$r" 3
check "başkasının kökü değişmedi" rc_is "$(tc qdisc show dev va)" "$F"
tc qdisc del dev va root
r=$(gt down --dev va); check "kök varsayılan + bayat kayıt → down rc 3 (dokunmaz)" rc_is "$r" 3
check "bayat kayıt yolu iletide" grep -q "rm $KAMA_GOSTER_TC_RUN/va.state" "$T/err"
rm -f "$KAMA_GOSTER_TC_RUN/va.state"

echo "(e) filtre yalnız hedef portu sınıflar"
r=$(gt up --dev va --port 47100 --match both --delay-ms 1 --loss-pct 0 --rate-kbit 10000)
check "up rc 0" rc_is "$r" 0
send 10.77.0.2 47100 0 20; settle
check "dport 47100: netem 40: = 20 paket" rc_is "$(pkts 40:)" 20
send 10.77.0.2 47201 0 20; settle
check "köprü portu 47201: netem 40: artmadı" rc_is "$(pkts 40:)" 20
send 10.77.0.2 9999 47100 15; settle
check "sport 47100: netem 40: = 35 paket" rc_is "$(pkts 40:)" 35
r=$(gt down --dev va); check "down rc 0" rc_is "$r" 0
r=$(gt up --dev va --port 47100 --match dport --delay-ms 1 --loss-pct 0 --rate-kbit 10000)
check "up --match dport rc 0" rc_is "$r" 0
send 10.77.0.2 9999 47100 10; settle
check "match dport: sport 47100 sınıflanmadı" rc_is "$(pkts 40:)" 0
send 10.77.0.2 47100 0 5; settle
check "match dport: dport 47100 sınıflandı" rc_is "$(pkts 40:)" 5
r=$(gt down --dev va); check "down rc 0" rc_is "$r" 0
r=$(gt up --dev va --port 47000 --match sport --delay-ms 1 --loss-pct 0 --rate-kbit 10000 \
  --per-dst 10.77.0.2 --per-dst 10.77.0.3)
check "up --per-dst ×2 rc 0" rc_is "$r" 0
check "kök bands 5" grep -q '^qdisc prio 1: root .*bands 5 ' <<< "$(root)"
send 10.77.0.2 9999 47000 7; send 10.77.0.3 9999 47000 4; send 10.77.0.2 9999 47001 9; settle
check "hedef .2 → 40: = 7" rc_is "$(pkts 40:)" 7
check "hedef .3 → 50: = 4" rc_is "$(pkts 50:)" 4
r=$(gt down --dev va); check "down rc 0" rc_is "$r" 0
check "kök varsayılana döndü" rc_is "$(root)" "$ROOT0"

echo "(f) ikinci up"
r=$(gt "${UP[@]}"); check "up rc 0" rc_is "$r" 0
F="$(tc qdisc show dev va)"; S="$(cat "$KAMA_GOSTER_TC_RUN/va.state")"
r=$(gt "${UP[@]}"); check "ikinci up rc 3" rc_is "$r" 3
check "kök değişmedi" rc_is "$(tc qdisc show dev va)" "$F"
check "kayıt değişmedi" rc_is "$(cat "$KAMA_GOSTER_TC_RUN/va.state")" "$S"

echo "(g) kilit"
flock "$KAMA_GOSTER_TC_LOCK" -c "touch '$T/held'; sleep 3" &
LK=$!
for _ in $(seq 1 100); do [ -e "$T/held" ] && break; sleep 0.02; done
r=$(gt down --dev va); check "kilit tutuluyken down rc 2" rc_is "$r" 2
check "kök değişmedi" rc_is "$(tc qdisc show dev va)" "$F"
wait "$LK"
r=$(gt down --dev va); check "kilit bırakılınca down rc 0" rc_is "$r" 0

echo "(p) parametreler"
r=$(gt up --dev va --port 47100 --match both --delay-ms 150 --rate-kbit 100)
check "--loss-pct yok → rc 1" rc_is "$r" 1
r=$(gt "${UP[@]}" --jitter-ms 10); check "--jitter-ms → rc 1" rc_is "$r" 1
r=$(gt up --dev va --port 47100 --match iki --delay-ms 150 --loss-pct 20 --rate-kbit 100)
check "--match geçersiz → rc 1" rc_is "$r" 1
r=$(gt up --dev yok0 --port 47100 --match both --delay-ms 150 --loss-pct 20 --rate-kbit 100)
check "olmayan arayüz → rc 1" rc_is "$r" 1
check "parametre hatalarında kök varsayılan" rc_is "$(root)" "$ROOT0"

echo "(t) taban"
th() { printf '%s\n' "$@" | sed 's/^/64 bytes from x: icmp_seq=1 ttl=64 time=/; s/$/ ms/' | "$GT" _taban_hesap x 2>"$T/err"; }
check "tek sayı: medyan 20 → D 140" grep -q 'taban_rtt_medyan_ms=20.000 D_ms=140 n=3' <<< "$(th 30 10 20)"
check "çift sayı: medyan 25 → D 138" grep -q 'taban_rtt_medyan_ms=25.000 D_ms=138 n=4' <<< "$(th 40 10 30 20)"
check "≥ 300 ms → D 0 + uyarı" grep -q 'D_ms=0 ' <<< "$(th 310 320 330)"
check "uyarı iletisi" grep -q 'UYARI taban RTT' "$T/err"
r=$(printf 'no reply\n' | "$GT" _taban_hesap x >/dev/null 2>&1; echo $?); check "yanıtsız → rc 5" rc_is "$r" 5

echo "== $PASS geçti, $FAIL kaldı"
[ "$FAIL" -eq 0 ]
