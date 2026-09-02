#!/usr/bin/env bash
# Commit-AG — ROOTLESS-USERNS netem (admin/docker-daemon GEREKMEZ; unshare -r → CAP_NET_ADMIN-in-userns).
# Tüm-matris TEK-netns-içinde: veth v0(tx)/v1(rx) + per-run-netem(v0=forward); blackout=v1-egress-periyodik.
# PRE-DECLARED-DARALTMA (koşudan-ÖNCE; wall-time-kısıtı): reps=5 (p7=10) — deterministik-katman-birincil-
# kanıt; netem=YÖN-doğrulama (packet-birebir-değil). Arm: leftover/p15/f15 (+f10 blackout). Dengeli-sıra.
set -euo pipefail
BIN=${RUNNER_BIN:?RUNNER_BIN}; OUT=${NETEM_OUT:?NETEM_OUT}; mkdir -p "$OUT"
PROFILES=${NETEM_PROFILES:-p1_rtt1s_burst p2_rtt2500_ge p3_blackout10 p4_blackout30 p7_gap80_under}
export BIN OUT PROFILES
unshare -r -n -m bash <<'INNER'
set -euo pipefail
ip link add v0 type veth peer name v1
ip addr add 10.77.0.1/24 dev v0; ip addr add 10.77.0.2/24 dev v1
ip link set v0 up; ip link set v1 up; ip link set lo up
uname -a > "$OUT/ENV.txt"; tc -V >> "$OUT/ENV.txt" 2>&1 || true
declare -A FWD=(
  [p1_rtt1s_burst]="delay 500ms loss gemodel 25% 40%"
  [p2_rtt2500_ge]="delay 1250ms loss gemodel 10% 60%"
  [p3_blackout10]="delay 50ms loss 10%"
  [p4_blackout30]="delay 50ms loss 10%"
  [p7_gap80_under]="delay 50ms rate 480kbit loss 5%"
)
declare -A EXT=([p1_rtt1s_burst]=30000 [p2_rtt2500_ge]=35000 [p3_blackout10]=45000 [p4_blackout30]=75000 [p7_gap80_under]=20000)
for prof in $PROFILES; do
  reps=5; [[ $prof == p7_* ]] && reps=10
  arms=(leftover p15 f15); [[ $prof == p3_* || $prof == p4_* ]] && arms+=(f10)
  for rep in $(seq 1 $reps); do
    order=("${arms[@]}"); (( rep % 2 == 0 )) && mapfile -t order < <(printf '%s\n' "${arms[@]}" | tac)
    for policy in "${order[@]}"; do
      rid="$prof-$policy-$rep"; ext=${EXT[$prof]}
      tc qdisc replace dev v0 root netem ${FWD[$prof]}
      tc qdisc replace dev v1 root netem delay 1ms
      pol=$policy; extra=""; [[ $policy == f10 ]] && { pol=f15; extra="--entitlement-share-permille 100 --hardcap-share-permille 200"; }
      $BIN --role rx --bind 10.77.0.2:47002 --peer 10.77.0.1:47001 --run-id "$rid" --output-dir "$OUT" --extended-horizon $ext &
      RX=$!
      if [[ $prof == p3_* ]]; then
        ( sleep 4; tc qdisc replace dev v1 root netem loss 100%; sleep 10; tc qdisc replace dev v1 root netem delay 1ms; sleep 8; tc qdisc replace dev v1 root netem loss 100%; sleep 10; tc qdisc replace dev v1 root netem delay 1ms ) & BK=$!
      elif [[ $prof == p4_* ]]; then
        ( sleep 4; tc qdisc replace dev v1 root netem loss 100%; sleep 30; tc qdisc replace dev v1 root netem delay 1ms ) & BK=$!
      fi
      timeout $((ext/1000+15)) $BIN --role tx --bind 10.77.0.1:47001 --peer 10.77.0.2:47002 --policy $pol $extra \
        --run-id "$rid" --seed $((100+rep)) --output-dir "$OUT" --operational-horizon 12000 --extended-horizon $ext --w-units 96 \
        || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
      [[ -n ${BK:-} ]] && { wait $BK 2>/dev/null || true; unset BK; }
      kill $RX 2>/dev/null || true; wait $RX 2>/dev/null || true
      [[ -s "$OUT/$rid-summary.csv" ]] || echo "MISSING $rid" >> "$OUT/RUNLOG"
      echo "$rid done" >> "$OUT/RUNLOG"
    done
  done
  echo "PROFIL-TAMAM $prof" >> "$OUT/RUNLOG"
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
