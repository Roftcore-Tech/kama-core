#!/usr/bin/env bash
# DR-FEC-FS-1 FS-HARNESS-A — BINARY LOOPBACK / CLOSED-LOOP SMOKE.
# "Gerçek çalıştırılabilir yol"dan (unit-test DEĞİL) doğrulama: A0/A1/F0 clean-loopback + recovery.
# Bu bir GÖSTERİM/smoke'tur — result-bearing DEĞİL (FS-1 v1.2-freeze SONRASI ayrı-tur).
#
# Kanıtladıkları:
#   1. Executable-path bütünlüğü: fs_runner --arm {a0,a1,f0} her arm için delivered=8/8 (clean).
#   2. F0 GERÇEK reconstruct: D2/R2 (drop 2,5) → reconstruct_byte_equal=true, retransmits=0, cause=fec.
#   3. A1 blind-dup HONESTY + egress-fairness: A1-MISS(drop=4) ≡ A0(drop=4) byte-özdeş
#      (retransmits=1, aynı t_arq_arrival, aynı completion) — dup {2,3}'e harcandı, {4}'e FAYDA-YOK.
#   4. A1-HIT(drop=2 ∈ target) → cause=duplicate, retransmits=0, UsableBeforeArq.
#   5. estimator_pure=true (F0 RECOVERED RTT/BtlBw'ye dokunmaz — fake-ACK-YOK).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "=== BUILD fs_runner (experimental-fs-harness) ==="
cargo build -p kama-integration --features experimental-fs-harness --example fs_runner 2>&1 | tail -2
BIN="$ROOT/target/debug/examples/fs_runner"
[ -x "$BIN" ] || { echo "FAIL: binary yok: $BIN"; exit 2; }
echo "binary=$BIN"
echo "git=$(git rev-parse --short HEAD 2>/dev/null || echo NA)"

pass=0; fail=0
field() { echo "$1" | grep -oE "^$2=[^ ]*" | head -1 | cut -d= -f2; }
check() { # $1=label $2=cond(0=ok) $3=detail
  if [ "$2" -eq 0 ]; then echo "PASS  $1  ($3)"; pass=$((pass+1));
  else echo "FAIL  $1  ($3)"; fail=$((fail+1)); fi
}

echo; echo "=== [1] CLEAN LOOPBACK (deficit yok → delivered=8/8, INVALID=false) ==="
for arm in a0 a1 f0; do
  out=$("$BIN" --arm $arm --r 2)
  d=$(echo "$out" | grep -oE "delivered=[0-9]+/8" | head -1)
  inv=$(echo "$out" | grep -oE "INVALID=[a-z]+" | head -1)
  [ "$d" = "delivered=8/8" ] && [ "$inv" = "INVALID=false" ]
  check "clean-loopback:$arm" $? "$d $inv"
done

echo; echo "=== [2] F0 GERÇEK reconstruct (D2/R2 drop 2,5) — FEC blok'u tamamlar (byte-equal + estimator-pure) ==="
# NOT: doğru (büyük) sealed-boyutta parity post-DATA GEÇ varır → ARQ SPURIOUS retransmit sızdırabilir
# AMA FEC yine ERKEN tamamlar (t_repair_usable < t_arq_arrival → cause=fec). Bu GERÇEKÇİ FEC-utility bulgusu.
out=$("$BIN" --arm f0 --r 2 --deficit 2,5)
be=$(echo "$out" | grep -oE "reconstruct_byte_equal=[a-z]+" | head -1)
fd=$(echo "$out" | grep -oE "fec_decoded_sources=[0-9]+" | head -1)
ep=$(echo "$out" | grep -oE "estimator_pure=[a-z]+" | head -1)
cc=$(echo "$out" | grep -oE "completion_cause=[a-z_]+" | head -1)
sp=$(echo "$out" | grep -oE "spurious_retransmits=[0-9]+" | head -1)
[ "$be" = "reconstruct_byte_equal=true" ] && [ "$fd" = "fec_decoded_sources=2" ] && [ "$ep" = "estimator_pure=true" ] && [ "$cc" = "completion_cause=fec" ]
check "f0-real-reconstruct (FEC completes)" $? "$be $fd $ep $cc ($sp — GERÇEKÇİ spurious-ARQ)"

echo; echo "=== [3] A1 blind-dup HIT (drop=2 ∈ target) → cause=duplicate, retransmits=0 ==="
out=$("$BIN" --arm a1 --r 2 --deficit 2)
hit=$(echo "$out" | grep -oE "a1_dup_hits=[0-9]+" | head -1)
rt=$(echo "$out" | grep -oE "retransmits=[0-9]+" | head -1)
cc=$(echo "$out" | grep -oE "completion_cause=[a-z_]+" | head -1)
pc=$(echo "$out" | grep -oE "parity_delay_class=[A-Za-z]+" | head -1)
[ "$hit" = "a1_dup_hits=1" ] && [ "$rt" = "retransmits=0" ] && [ "$cc" = "completion_cause=duplicate" ] && [ "$pc" = "parity_delay_class=UsableBeforeArq" ]
check "a1-dup-hit" $? "$hit $rt $cc $pc"

echo; echo "=== [4] A1-MISS(drop=4) ≡ A0(drop=4) — blind-dup honesty (byte-özdeş ARQ) ==="
a1=$("$BIN" --arm a1 --r 2 --deficit 4)
a0=$("$BIN" --arm a0 --deficit 4)
a1c=$(echo "$a1" | grep -oE "completion_ns=Some\([0-9]+\)" | head -1)
a0c=$(echo "$a0" | grep -oE "completion_ns=Some\([0-9]+\)" | head -1)
a1r=$(echo "$a1" | grep -oE "retransmits=[0-9]+" | head -1)
a0r=$(echo "$a0" | grep -oE "retransmits=[0-9]+" | head -1)
a1miss=$(echo "$a1" | grep -oE "a1_dup_misses=[0-9]+" | head -1)
[ "$a1c" = "$a0c" ] && [ "$a1r" = "retransmits=1" ] && [ "$a0r" = "retransmits=1" ] && [ "$a1miss" = "a1_dup_misses=1" ]
check "a1-miss≡a0 (honesty)" $? "a1{$a1c $a1r $a1miss} a0{$a0c $a0r}"

echo; echo "=== [5] TAIL-LOSS confound probe (drop=7, son-unit) — INFO (gate DEĞİL) ==="
out=$("$BIN" --arm a1 --r 2 --deficit 7)
d=$(echo "$out" | grep -oE "delivered=[0-9]+/8" | head -1)
inv=$(echo "$out" | grep -oE "INVALID=[a-z]+" | head -1)
echo "INFO  tail-loss(drop=7): $d $inv  → FS-HARNESS-B OPEN ITEM (tail-position ARQ-detect confound)"

echo; echo "=== SONUÇ: pass=$pass fail=$fail ==="
[ "$fail" -eq 0 ] && { echo "SMOKE GREEN"; exit 0; } || { echo "SMOKE RED"; exit 1; }
