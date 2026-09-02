#!/usr/bin/env bash
# Commit-FB+FD — offered≈C graceful-rerun + FIXED-R-ORACLE + FUZZY-DATASET (rootless-userns; FA-marker).
# LeftoverOnly-sabit; oracle-arm'ları r0/r1/r2/r4 (--fixed-r; --policy leftover). --emit-fuzzy her-run.
set -euo pipefail
BIN=${RUNNER_BIN:?}; OUT=${NETEM_OUT:?}; mkdir -p "$OUT"; export BIN OUT
unshare -r -n -m bash <<'INNER'
set -euo pipefail
ip link add v0 type veth peer name v1
ip addr add 10.77.0.1/24 dev v0; ip addr add 10.77.0.2/24 dev v1
ip link set v0 up; ip link set v1 up; ip link set lo up
# Profiller (FusionInputs-çeşitliliği + offered≈C-closure): nominal/loss-med/loss-high/ge-burst/
# blackout30/lowbw/offered_c. Her-profilde 4-oracle-arm (r0/r1/r2/r4, LeftoverOnly) × 8-rep.
declare -A FWD=(
  [nominal]="delay 50ms loss 5%"
  [loss_med]="delay 50ms loss 15%"
  [loss_high]="delay 50ms loss 30%"
  [ge_burst]="delay 50ms loss gemodel 12% 55%"
  [blackout30]="delay 50ms loss 8%"
  [lowbw30k]="delay 50ms rate 30kbit loss 5%"
  [offered_c]="delay 50ms rate 240kbit loss 5%"
)
declare -A EXT=([nominal]=25000 [loss_med]=25000 [loss_high]=30000 [ge_burst]=30000 [blackout30]=75000 [lowbw30k]=90000 [offered_c]=30000)
declare -A WU=([lowbw30k]=200 [offered_c]=400)
for prof in nominal loss_med loss_high ge_burst blackout30 lowbw30k offered_c; do
  ext=${EXT[$prof]}; wu=${WU[$prof]:-300}
  for rep in $(seq 1 8); do
    rs=(0 1 2 4); (( rep % 2 == 0 )) && rs=(4 2 1 0)
    for r in "${rs[@]}"; do
      rid="$prof-r$r-$rep"
      tc qdisc replace dev v0 root netem ${FWD[$prof]}; tc qdisc replace dev v1 root netem delay 1ms
      $BIN --role rx --bind 10.77.0.2:47002 --peer 10.77.0.1:47001 --run-id "$rid" --output-dir "$OUT" --operational-horizon 15000 --extended-horizon $ext & RX=$!
      if [[ $prof == blackout* ]]; then ( sleep 5; tc qdisc replace dev v1 root netem loss 100%; sleep 30; tc qdisc replace dev v1 root netem delay 1ms ) & BK=$!; fi
      timeout $((ext/1000+20)) $BIN --role tx --bind 10.77.0.1:47001 --peer 10.77.0.2:47002 --policy leftover --fixed-r $r --emit-fuzzy \
        --run-id "$rid" --seed $((100+rep)) --output-dir "$OUT" --operational-horizon 15000 --extended-horizon $ext --w-units $wu \
        || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
      [[ -n ${BK:-} ]] && { wait $BK 2>/dev/null||true; unset BK; }
      wait $RX 2>/dev/null || echo "RXFAIL $rid" >> "$OUT/RUNLOG"
      m="$OUT/$rid-rxdone.marker"
      if [[ ! -f $m ]]; then echo "INCOMPLETE $rid" >> "$OUT/RUNLOG"; elif grep -q "clean_shutdown=false" "$m"; then echo "UNCLEAN $rid" >> "$OUT/RUNLOG"; else echo "$rid clean" >> "$OUT/RUNLOG"; fi
    done
  done
  echo "PROFIL-TAMAM $prof" >> "$OUT/RUNLOG"
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
