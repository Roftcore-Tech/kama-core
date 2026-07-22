#!/usr/bin/env bash
# Commit-AC — DAR-NETEM-DOĞRULAMASI (sertleştirilmiş). Arm'lar: leftover + p15 + f15 (+f10 yalnız-
# blackout-sensitivity). 7-profil; dengeli-koşu-sırası (rep-paritesine-göre-arm-sırası-döndürülür).
# Root'suz-çekirdek: runner-root-istemez; root-yalnız-netns/tc-katmanı. İnteraktif-sudo-parolası-SORULMAZ.
set -euo pipefail
OUT=${NETEM_OUT:?NETEM_OUT}
BIN="cargo run --release -q -p adaptive-transport --features experimental-fec-budget --example udp_budget_runner --"
NS1=kama_tx; NS2=kama_rx; V1=kveth0; V2=kveth1

# ── YETENEK-TESPİTİ (non-interaktif; typed/exact-blocker) ──
blocker(){ # $1=missing $2=cmd $3=status $4=stderr
  cat > "$OUT/NETEM-BLOCKER.txt" <<EOB
missing_capability=$1
failed_command=$2
exit_status=$3
stderr=$4
runner_readiness=READY (loopback-smoke-yeşil; bkz. udp_runner_smoke.rs)
remaining_manual_action=self-hosted-nightly'de (root/CAP_NET_ADMIN) bu-script'i NETEM_OUT ile koş
EOB
  echo "NETEM-BLOCKER: $1 (rapor: $OUT/NETEM-BLOCKER.txt)"; exit 4
}
mkdir -p "$OUT"
if [[ $EUID -ne 0 ]]; then
  if sudo -n true 2>"$OUT/.sudoerr"; then SUDO="sudo -n"; else
    blocker "root/CAP_NET_ADMIN" "sudo -n true" "$?" "$(cat "$OUT/.sudoerr")"
  fi
else SUDO=""; fi
for c in ip tc; do command -v $c >/dev/null || blocker "$c-binary" "command -v $c" "1" "not-found"; done
$SUDO ip netns add kama_captest 2>"$OUT/.nserr" || blocker "netns-create" "ip netns add" "$?" "$(cat "$OUT/.nserr")"
$SUDO ip netns del kama_captest

cleanup(){ $SUDO ip netns del $NS1 2>/dev/null||true; $SUDO ip netns del $NS2 2>/dev/null||true; }
trap cleanup EXIT
cleanup
$SUDO ip netns add $NS1; $SUDO ip netns add $NS2
$SUDO ip link add $V1 type veth peer name $V2
$SUDO ip link set $V1 netns $NS1; $SUDO ip link set $V2 netns $NS2
$SUDO ip netns exec $NS1 ip addr add 10.99.0.1/24 dev $V1; $SUDO ip netns exec $NS1 ip link set $V1 up; $SUDO ip netns exec $NS1 ip link set lo up
$SUDO ip netns exec $NS2 ip addr add 10.99.0.2/24 dev $V2; $SUDO ip netns exec $NS2 ip link set $V2 up; $SUDO ip netns exec $NS2 ip link set lo up
uname -a > "$OUT/ENV.txt"; tc -V >> "$OUT/ENV.txt" 2>&1 || true; git rev-parse HEAD >> "$OUT/ENV.txt"

declare -A PROF=(
  [p1_rtt1s_burst]="delay 500ms loss gemodel 25% 40%"
  [p2_rtt2500_ge]="delay 1250ms loss gemodel 10% 60%"
  [p3_blackout10]="delay 50ms"
  [p4_blackout30]="delay 50ms"
  [p5_offered_c]="delay 50ms rate 480kbit"
  [p6_lowbw30k]="delay 50ms rate 30kbit"
  [p7_gap80_under]="delay 50ms rate 480kbit loss 5%"
)
ARMS=(leftover p15 f15)
for prof in p1_rtt1s_burst p2_rtt2500_ge p3_blackout10 p4_blackout30 p5_offered_c p6_lowbw30k p7_gap80_under; do
  reps=10; [[ $prof == p7_* ]] && reps=30
  arms=("${ARMS[@]}"); [[ $prof == p3_* || $prof == p4_* ]] && arms+=(f10)
  for rep in $(seq 1 $reps); do
    order=("${arms[@]}"); (( rep % 2 == 0 )) && order=($(printf '%s\n' "${arms[@]}" | tac)) # dengeli-sıra
    for policy in "${order[@]}"; do
      $SUDO ip netns exec $NS1 tc qdisc replace dev $V1 root netem ${PROF[$prof]}
      echo "$prof $rep $policy $(date -u +%s) ${PROF[$prof]}" >> "$OUT/RUNLOG"
      extra=""; [[ $policy == f10 ]] && extra="--entitlement-share-permille 100 --hardcap-share-permille 200"; pol=$policy; [[ $policy == f10 ]] && pol=f15
      $SUDO ip netns exec $NS2 timeout 90 $BIN --role rx --bind 10.99.0.2:47002 --peer 10.99.0.1:47001 --run-id "$prof-$policy-$rep" --output-dir "$OUT" &
      RXPID=$!
      # Blackout-profilleri: rx-yönü-periyodik-%100-loss (10s/30s-pencere) — tc-change-döngüsü.
      if [[ $prof == p3_* || $prof == p4_* ]]; then
        dark=10; [[ $prof == p4_* ]] && dark=30
        ( for k in 1 2 3; do
            $SUDO ip netns exec $NS2 tc qdisc replace dev $V2 root netem loss 100%; sleep $dark
            $SUDO ip netns exec $NS2 tc qdisc replace dev $V2 root netem delay 1ms; sleep $dark
          done ) & BKPID=$!
      fi
      $SUDO ip netns exec $NS1 timeout 90 $BIN --role tx --bind 10.99.0.1:47001 --peer 10.99.0.2:47002 \
        --policy "$pol" $extra --run-id "$prof-$policy-$rep" --seed $((100+rep)) --output-dir "$OUT" \
        --operational-horizon 20000 --extended-horizon 80000 --w-units 200 || echo "TXFAIL $prof-$policy-$rep" >> "$OUT/RUNLOG"
      wait $RXPID 2>/dev/null || true; [[ -n ${BKPID:-} ]] && { kill $BKPID 2>/dev/null||true; unset BKPID; }
      # Çıktı-doğrulama:
      [[ -s "$OUT/$prof-$policy-$rep-summary.csv" ]] || echo "MISSING-OUTPUT $prof-$policy-$rep" >> "$OUT/RUNLOG"
    done
  done
done
echo "NETEM-TAMAM: $OUT"
