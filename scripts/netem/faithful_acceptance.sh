#!/usr/bin/env bash
# Commit-FQ — FAITHFUL HARNESS ACCEPTANCE PINS. Wall-clock utility yorumundan ÖNCE koşulur.
# Gate: configured-RTT ölçülen-RTT ile ±%15-veya-±20ms (hangisi-büyük) içinde eşleşmeli; forward/reverse
# yön-izolasyonu; blackout+recovery; cleanup. Rootless (unshare -rnm + child-netns). Root/docker-daemon-YOK.
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
OUT=${ACCEPT_OUT:?ACCEPT_OUT}; mkdir -p "$OUT"
export LIB OUT
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"
faithful_setup
trap faithful_teardown EXIT
{
  echo "=== ENV MANIFEST ==="; uname -a; ip -Version 2>&1 | head -1; tc -V 2>&1
  echo "namespaces: parent=$(readlink /proc/self/ns/net) child=$(readlink /proc/$CHILD/ns/net)"
  echo "git: $(git -C /home/ryuzaki/Desktop/kama-core rev-parse --short HEAD 2>/dev/null || echo NA)"
} > "$OUT/ENV.txt"

ping_min() { # $1=ip → min-RTT-ms (int); warmup-first
  ping -c 1 -W 3 "$1" >/dev/null 2>&1 || true   # warmup (ARP/cache)
  local out min
  out=$(ping -c 4 -W 5 -i 0.25 "$1" 2>&1 | grep "min/avg")
  min=$(echo "$out" | grep -oE "= [0-9.]+/" | grep -oE "[0-9.]+" | head -1)
  echo "${min%.*}"
}

pass=0; fail=0
check_rtt() { # $1=label $2=one-way-ms $3=expected-rtt-ms
  faithful_netem "delay ${2}ms" "delay ${2}ms"
  local min tol lo hi; min=$(ping_min 10.9.0.2)
  tol=$(( $3 * 15 / 100 )); [ $tol -lt 20 ] && tol=20
  lo=$(( $3 - tol )); hi=$(( $3 + tol ))
  if [ -n "$min" ] && [ "$min" -ge "$lo" ] 2>/dev/null && [ "$min" -le "$hi" ] 2>/dev/null; then
    echo "PASS $1: configured-RTT=${3}ms measured-min=${min}ms (tol±${tol})"; pass=$((pass+1))
  else
    echo "FAIL $1: configured-RTT=${3}ms measured-min=${min:-NONE}ms (want ${lo}-${hi})"; fail=$((fail+1))
  fi
}
check_iso() { # $1=label $2=fwd-spec $3=rev-spec $4=exp
  faithful_netem "$2" "$3"; local min; min=$(ping_min 10.9.0.2)
  if [ -n "$min" ] && [ "$min" -ge $(( $4 - 30 )) ] 2>/dev/null && [ "$min" -le $(( $4 + 60 )) ] 2>/dev/null; then
    echo "PASS $1: ($2 | $3) → RTT-min=${min}ms (≈$4)"; pass=$((pass+1))
  else
    echo "FAIL $1: ($2 | $3) → RTT-min=${min:-NONE}ms (want≈$4)"; fail=$((fail+1))
  fi
}
replies() { ping -c "$1" -W 2 -i 0.3 10.9.0.2 2>&1 | grep -c " bytes from "; }

{
  echo "=== RTT ACCEPTANCE PINS (symmetric) ==="
  check_rtt rtt100  50  100
  check_rtt rtt1s  500 1000
  check_rtt rtt2500 1250 2500
  echo "=== DIRECTION ISOLATION ==="
  check_iso fwd-only "delay 500ms" "delay 1ms" 500
  check_iso rev-only "delay 1ms" "delay 500ms" 500
  echo "=== BLACKOUT + RECOVERY (reverse 100% loss → restore) ==="
  faithful_netem "delay 50ms" "delay 50ms"
  b1=$(replies 2); nsenter -t "$CHILD" -n tc qdisc replace dev v1 root netem loss 100%
  b2=$(replies 2); nsenter -t "$CHILD" -n tc qdisc replace dev v1 root netem delay 50ms; sleep 0.3
  b3=$(replies 3)
  echo "pre=$b1 during-blackout=$b2 post-recovery=$b3"
  if [ "$b1" -ge 1 ] && [ "$b2" -eq 0 ] && [ "$b3" -ge 1 ]; then echo "PASS blackout-recovery"; pass=$((pass+1)); else echo "FAIL blackout-recovery"; fail=$((fail+1)); fi
  echo "=== SONUÇ: PASS=$pass FAIL=$fail ==="
} 2>&1 | tee "$OUT/ACCEPTANCE.txt"
INNER
echo "=== host-side cleanup verification ==="
ip link show 2>/dev/null | grep -E "v0@|v1@" && echo "LEAK: veth host-ta" || echo "clean: host-ta veth-yok"
