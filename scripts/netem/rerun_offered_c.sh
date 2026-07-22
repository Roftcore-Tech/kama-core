#!/usr/bin/env bash
# Commit-FB — offered≈C GRACEFUL-rerun (FA-marker'lı; SIGTERM-YOK). Rootless-userns netem.
# Orchestrator marker-doğrulaması: done-marker yoksa→INCOMPLETE; clean_shutdown=false→UNCLEAN.
set -euo pipefail
BIN=${RUNNER_BIN:?}; OUT=${NETEM_OUT:?}; mkdir -p "$OUT"; export BIN OUT
unshare -r -n -m bash <<'INNER'
set -euo pipefail
ip link add v0 type veth peer name v1
ip addr add 10.77.0.1/24 dev v0; ip addr add 10.77.0.2/24 dev v1
ip link set v0 up; ip link set v1 up; ip link set lo up
# offered≈C: rate 240kbit + gap40 (offered=30kB/s=240kbit=rate → tam-doyum) + %5-loss.
FWD="delay 50ms rate 240kbit loss 5%"; EXT=30000; WU=400
for rep in $(seq 1 10); do
  arms=(leftover p15 f15 f10)
  order=("${arms[@]}"); (( rep % 2 == 0 )) && mapfile -t order < <(printf '%s\n' "${arms[@]}" | tac)
  for policy in "${order[@]}"; do
    rid="offered_c-$policy-$rep"
    tc qdisc replace dev v0 root netem $FWD; tc qdisc replace dev v1 root netem delay 1ms
    pol=$policy; extra=""; [[ $policy == f10 ]] && { pol=f15; extra="--entitlement-share-permille 100 --hardcap-share-permille 200"; }
    # rx-arka-plan; SIGTERM-YOK → FINISH-marker'la temiz-çıkar (kendi-kendine-join).
    $BIN --role rx --bind 10.77.0.2:47002 --peer 10.77.0.1:47001 --run-id "$rid" --output-dir "$OUT" --operational-horizon 15000 --extended-horizon $EXT & RX=$!
    timeout $((EXT/1000+20)) $BIN --role tx --bind 10.77.0.1:47001 --peer 10.77.0.2:47002 --policy $pol $extra \
      --run-id "$rid" --seed $((100+rep)) --output-dir "$OUT" --operational-horizon 15000 --extended-horizon $EXT --w-units $WU \
      || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
    wait $RX 2>/dev/null || echo "RXFAIL $rid" >> "$OUT/RUNLOG"  # SIGTERM-YOK; graceful-join
    # Marker-doğrulama (orchestrator-kuralı):
    m="$OUT/$rid-rxdone.marker"
    if [[ ! -f $m ]]; then echo "INCOMPLETE $rid" >> "$OUT/RUNLOG"
    elif grep -q "clean_shutdown=false" "$m"; then echo "UNCLEAN $rid" >> "$OUT/RUNLOG"
    else echo "$rid clean" >> "$OUT/RUNLOG"; fi
  done
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
