#!/usr/bin/env bash
# TUR-A1 W-3-65 (B1) — restore=61 · --no-settle-exit · OBSERVATION_HORIZON=120s · RX_TEST_PATIENCE=130s · 3 rep.
# Enstrüman-zinciri: provenance + per-cell RTT-realization + overlap-gate + inter-run-drain (BUG-H3).
# Beklenen (R2 pre-reg §3): finalized=1 · terminal=TimedOut · recovery_observation=RecoveredByForwardRecoveryControl
# · PostTimeoutRecovery YOK · app_dup=0 · no-busy-loop · no-second-timeout.
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
BIN_CS=${BIN_CS:?}; OUT=${WIRE_OUT:?}; mkdir -p "$OUT"
export LIB BIN_CS OUT
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"; faithful_setup; trap faithful_teardown EXIT
# PROVENANCE + acceptance (RTT100-pin).
{
  echo "git_commit=$(git -C /home/ryuzaki/Desktop/kama-core rev-parse --short HEAD)"
  echo "dirty=$(git -C /home/ryuzaki/Desktop/kama-core status --porcelain | wc -l)"
  echo "kernel=$(uname -r)"; echo "tc=$(tc -V 2>&1)"
  echo "netns=parent:$(readlink /proc/self/ns/net) child:$(readlink /proc/$CHILD/ns/net)"
  faithful_netem "delay 50ms" "delay 50ms"
  ping -c1 -W2 10.9.0.2 >/dev/null 2>&1
  m=$(ping -c5 -W3 -i0.3 10.9.0.2 2>&1 | grep min/avg | grep -oE "= [0-9.]+/[0-9.]+" | grep -oE "[0-9.]+" | sed -n '1p')
  echo "acceptance_rtt100_min_ms=${m%.*}"
  echo "scenario=W-3-65-B1; restore=61s; no-settle-exit; OBSERVATION_HORIZON=120s; RX_TEST_PATIENCE=130s"
} > "$OUT/W365-PROVENANCE.txt"
runrx() { faithful_rx "$BIN_CS" --role rx --bind 10.9.0.2:47002 --peer 10.9.0.1:47001 "$@" >/dev/null 2>&1 & echo $!; }

for rep in 1 2 3; do
  id="w365-r${rep}"
  faithful_netem "delay 50ms" "delay 50ms"
  # PER-CELL RTT-REALIZATION (forward+reverse; p50/p95).
  { ping -c8 -W3 -i0.2 10.9.0.2 2>&1 | grep -oE "time=[0-9.]+" | grep -oE "[0-9.]+" | sort -n | \
      awk '{a[NR]=$1} END{p50=a[int(NR*0.5)+1]; p95=a[int(NR*0.95)+1]; printf "rtt_p50=%s rtt_p95=%s\n", p50, p95}'; } > "$OUT/$id-rtt.txt" 2>/dev/null || echo "rtt_measure_failed" > "$OUT/$id-rtt.txt"
  RX=$(runrx --run-id "$id" --output-dir "$OUT" --deficit 1 --deficit-from-block 0 \
    --operational-horizon 2000 --extended-horizon 130000); sleep 0.3
  ( sleep 0.5; faithful_netem "delay 50ms loss 100%" "delay 50ms loss 100%"
    sleep 61; faithful_netem "delay 50ms" "delay 50ms" ) & BK=$!
  timeout 128 "$BIN_CS" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
    --run-id "$id" --output-dir "$OUT" --operational-horizon 2000 --extended-horizon 120000 \
    --w-units 8 --unit-bytes 65 --no-settle-exit >/dev/null 2>&1
  wait $BK 2>/dev/null; wait $RX 2>/dev/null
  faithful_netem "delay 1ms loss 100%" "delay 1ms loss 100%"; sleep 1.2
  echo "$id done" >> "$OUT/W365-RUNLOG"
done
echo "W365-ALL-DONE" >> "$OUT/W365-RUNLOG"
INNER
