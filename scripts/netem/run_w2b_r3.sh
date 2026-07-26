#!/usr/bin/env bash
# TUR-A1 W-2B-R3 — FULL ACK-STARVATION PROGRESS/COMPLETION CHARACTERIZATION (Abkarada A+C).
# 54 koşu: RTT{100,250,1000}×size{65,300,1200}×feature{ON=bin_cs,OFF=bin_off}×3rep. Size-scaled horizon
# (30/85/45s; 1200B runtime-cap → NO_COMPLETION_BY_45S). rx VE tx birebir --w-units/--unit-bytes.
# Enstrüman-zinciri: provenance + per-cell RTT-realization + overlap-gate + inter-run-drain (BUG-H3).
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
BIN_CS=${BIN_CS:?}; BIN_OFF=${BIN_OFF:?}; OUT=${WIRE_OUT:?}; mkdir -p "$OUT"
export LIB BIN_CS BIN_OFF OUT
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"; faithful_setup; trap faithful_teardown EXIT
declare -A HZ=( [65]=30000 [300]=85000 [1200]=45000 )
{
  echo "git_commit=$(git -C /home/ryuzaki/Desktop/kama-core rev-parse --short HEAD)"
  echo "dirty=$(git -C /home/ryuzaki/Desktop/kama-core status --porcelain | wc -l)"
  echo "kernel=$(uname -r)"; echo "tc=$(tc -V 2>&1)"
  echo "netns=parent:$(readlink /proc/self/ns/net) child:$(readlink /proc/$CHILD/ns/net)"
  faithful_netem "delay 50ms" "delay 50ms"; ping -c1 -W2 10.9.0.2 >/dev/null 2>&1
  m=$(ping -c5 -W3 -i0.3 10.9.0.2 2>&1 | grep min/avg | grep -oE "= [0-9.]+/[0-9.]+" | grep -oE "[0-9.]+" | sed -n '1p')
  echo "acceptance_rtt100_min_ms=${m%.*}"
  echo "scenario=W-2B-R3; A+C-bounds; size-scaled-horizon 65:30s/300:85s/1200:45s(NO_COMPLETION_BY_45S)"
} > "$OUT/R3-PROVENANCE.txt"
rxrun() { local bin=$1; shift; faithful_rx "$bin" --role rx --bind 10.9.0.2:47002 --peer 10.9.0.1:47001 "$@" >/dev/null 2>&1 & echo $!; }

for rtt in 100 250 1000; do
  half=$((rtt/2))
  for sz in 65 300 1200; do
    H=${HZ[$sz]}; rxext=$((H+5000)); txto=$((H/1000 + 12))
    for feat in on off; do
      [ "$feat" = on ] && BIN="$BIN_CS" || BIN="$BIN_OFF"
      for rep in 1 2 3; do
        id="r3-${rtt}-${sz}-${feat}-r${rep}"
        faithful_netem "delay ${half}ms" "delay ${half}ms"
        # per-cell RTT-realization
        { ping -c8 -W3 -i0.2 10.9.0.2 2>&1 | grep -oE "time=[0-9.]+" | grep -oE "[0-9.]+" | sort -n | \
            awk -v cfg="$rtt" '{a[NR]=$1} END{p50=a[int(NR*0.5)+1]; p95=a[int(NR*0.95)+1];
              tol_p50=(p50<=cfg+((cfg*0.05>5)?cfg*0.05:5) && p50>=cfg-((cfg*0.05>5)?cfg*0.05:5))?"PASS":"FAIL";
              tol_p95=(p95<=cfg+((cfg*0.10>20)?cfg*0.10:20))?"PASS":"FAIL";
              printf "configured_rtt=%s rtt_p50=%s rtt_p95=%s rtt_tol_p50=%s rtt_tol_p95=%s\n", cfg, p50, p95, tol_p50, tol_p95}'; } \
          > "$OUT/$id-rtt.txt" 2>/dev/null || echo "rtt_measure_failed" > "$OUT/$id-rtt.txt"
        RX=$(rxrun "$BIN" --run-id "$id" --output-dir "$OUT" --w-units 8 --unit-bytes "$sz" \
          --suppress-data-acks --operational-horizon 4000 --extended-horizon "$rxext"); sleep 0.3
        timeout "$txto" "$BIN" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
          --run-id "$id" --output-dir "$OUT" --operational-horizon 4000 --extended-horizon "$H" \
          --w-units 8 --unit-bytes "$sz" --no-settle-exit --suppress-data-acks >/dev/null 2>&1
        wait $RX 2>/dev/null
        faithful_netem "delay 1ms loss 100%" "delay 1ms loss 100%"; sleep 1.2
        echo "$id done" >> "$OUT/R3-RUNLOG"
      done
    done
  done
done
echo "R3-ALL-DONE" >> "$OUT/R3-RUNLOG"
INNER
