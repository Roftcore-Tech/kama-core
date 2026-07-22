#!/usr/bin/env bash
# Commit-AH — CONTENDED-rootless-netem (R1+R2-kapatma): büyük-w_units + dar-horizon + saturating-profil
# → starvation-biter; rx-outcome-ledger (decode/dl_op) toplanır. Decisive-profiller: blackout30 + lowbw30k
# + offered_c. reps=8 (PRE-DECLARED). Arm: leftover/p15/f15(+f10-blackout).
set -euo pipefail
BIN=${RUNNER_BIN:?}; OUT=${NETEM_OUT:?}; mkdir -p "$OUT"
export BIN OUT
unshare -r -n -m bash <<'INNER'
set -euo pipefail
ip link add v0 type veth peer name v1
ip addr add 10.77.0.1/24 dev v0; ip addr add 10.77.0.2/24 dev v1
ip link set v0 up; ip link set v1 up; ip link set lo up
declare -A FWD=( [c_lowbw30k]="delay 50ms rate 30kbit" [c_offered_c]="delay 50ms rate 240kbit loss 5%" [c_blackout30]="delay 50ms loss 8%" )
declare -A EXT=( [c_lowbw30k]=90000 [c_offered_c]=60000 [c_blackout30]=90000 )
declare -A WU=( [c_lowbw30k]=200 [c_offered_c]=400 [c_blackout30]=300 )
declare -A GAP=( [c_lowbw30k]=250 [c_offered_c]=40 [c_blackout30]=40 )
for prof in c_offered_c c_lowbw30k c_blackout30; do
  arms=(leftover p15 f15); [[ $prof == c_black* ]] && arms+=(f10)
  for rep in $(seq 1 8); do
    order=("${arms[@]}"); (( rep % 2 == 0 )) && mapfile -t order < <(printf '%s\n' "${arms[@]}" | tac)
    for policy in "${order[@]}"; do
      rid="$prof-$policy-$rep"; ext=${EXT[$prof]}; wu=${WU[$prof]}
      tc qdisc replace dev v0 root netem ${FWD[$prof]}
      tc qdisc replace dev v1 root netem delay 1ms
      pol=$policy; extra=""; [[ $policy == f10 ]] && { pol=f15; extra="--entitlement-share-permille 100 --hardcap-share-permille 200"; }
      $BIN --role rx --bind 10.77.0.2:47002 --peer 10.77.0.1:47001 --run-id "$rid" --output-dir "$OUT" --operational-horizon 15000 --extended-horizon $ext & RX=$!
      if [[ $prof == c_black* ]]; then ( sleep 5; tc qdisc replace dev v1 root netem loss 100%; sleep 30; tc qdisc replace dev v1 root netem delay 1ms ) & BK=$!; fi
      timeout $((ext/1000+15)) $BIN --role tx --bind 10.77.0.1:47001 --peer 10.77.0.2:47002 --policy $pol $extra \
        --run-id "$rid" --seed $((100+rep)) --output-dir "$OUT" --operational-horizon 15000 --extended-horizon $ext --w-units $wu \
        || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
      [[ -n ${BK:-} ]] && { wait $BK 2>/dev/null||true; unset BK; }
      kill $RX 2>/dev/null||true; wait $RX 2>/dev/null||true
      echo "$rid done" >> "$OUT/RUNLOG"
    done
  done
  echo "PROFIL-TAMAM $prof" >> "$OUT/RUNLOG"
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
