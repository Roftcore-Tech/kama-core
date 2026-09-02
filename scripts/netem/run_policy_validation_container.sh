#!/usr/bin/env bash
# Commit-AF — DAR-NETEM: REPO-HARNESS-CONTAINER'LARI (harness/docker-compose.yml; cap_add=NET_ADMIN;
# admin/sudo-GEREKMEZ — docker-daemon-erişimi-yeter). Kullanıcı-talimatı: "önceki-testlerin-container'ları".
# Topoloji: client=tx / server=rx (benchnet-bridge); forward-netem=client-egress(eth0),
# reverse/blackout=server-egress(eth0). Arm'lar: leftover/p15/f15 (+f10 blackout'ta); dengeli-sıra.
set -euo pipefail
OUT=${NETEM_OUT:?NETEM_OUT}
PROJ=${NETEM_PROJ:-kamanet}
PROFILES=${NETEM_PROFILES:-p1_rtt1s_burst p2_rtt2500_ge p3_blackout10 p4_blackout30 p5_offered_c p6_lowbw30k p7_gap80_under}
CS=${PROJ}-client; SV=${PROJ}-server
DC="docker compose -p $PROJ -f harness/docker-compose.yml"
mkdir -p "$OUT"
# Yetenek-tespiti (non-interaktif; typed-blocker):
docker info >/dev/null 2>"$OUT/.derr" || { echo "missing_capability=docker-daemon
failed_command=docker info
stderr=$(cat "$OUT/.derr")" > "$OUT/NETEM-BLOCKER.txt"; exit 4; }
cleanup(){ $DC down -t 1 >/dev/null 2>&1 || true; }
trap cleanup EXIT
# Proje-adlı-container-isimleri (compose-container_name-sabit → proje-çakışmasını-önle: override):
export COMPOSE_IGNORE_ORPHANS=1
$DC up -d --no-build 2>/dev/null || $DC up -d
# compose-container_name sabit olduğundan gerçek-adlar:
CS=kama-bench-client; SV=kama-bench-server
dex(){ docker exec "$1" bash -c "$2"; }
# Runner-binary container-içinde-hazır-mı; değilse-derle (bench-build-cache'i-üstüne hızlı):
dex $CS "test -x /app/target/release/examples/udp_budget_runner" 2>/dev/null || \
  dex $CS "cd /app && cargo build --release -q -p adaptive-transport --features experimental-fec-budget --example udp_budget_runner"
dex $SV "test -x /app/target/release/examples/udp_budget_runner" 2>/dev/null || true # aynı-volume-yoksa-server-kendi-derler
dex $SV "test -x /app/target/release/examples/udp_budget_runner" 2>/dev/null || \
  dex $SV "cd /app && cargo build --release -q -p adaptive-transport --features experimental-fec-budget --example udp_budget_runner"
SIP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $SV)
CIP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $CS)
uname -a > "$OUT/ENV.txt"; docker version --format '{{.Server.Version}}' >> "$OUT/ENV.txt"; git rev-parse HEAD >> "$OUT/ENV.txt"
dex $CS "tc -V" >> "$OUT/ENV.txt"
BIN=/app/target/release/examples/udp_budget_runner
declare -A FWD=(
  [p1_rtt1s_burst]="delay 500ms loss gemodel 25% 40%"
  [p2_rtt2500_ge]="delay 1250ms loss gemodel 10% 60%"
  [p3_blackout10]="delay 50ms loss 10%"
  [p4_blackout30]="delay 50ms loss 10%"
  [p5_offered_c]="delay 50ms rate 480kbit"
  [p6_lowbw30k]="delay 50ms rate 30kbit"
  [p7_gap80_under]="delay 50ms rate 480kbit loss 5%"
)
declare -A EXTH=([p1_rtt1s_burst]=30000 [p2_rtt2500_ge]=35000 [p3_blackout10]=45000 [p4_blackout30]=75000 [p5_offered_c]=20000 [p6_lowbw30k]=45000 [p7_gap80_under]=20000)
declare -A WU=([p6_lowbw30k]=48)
for prof in $PROFILES; do
  reps=10; [[ $prof == p7_* ]] && reps=30
  arms=(leftover p15 f15); [[ $prof == p3_* || $prof == p4_* ]] && arms+=(f10)
  for rep in $(seq 1 $reps); do
    order=("${arms[@]}"); (( rep % 2 == 0 )) && mapfile -t order < <(printf '%s\n' "${arms[@]}" | tac)
    for policy in "${order[@]}"; do
      rid="$prof-$policy-$rep"
      dex $CS "tc qdisc replace dev eth0 root netem ${FWD[$prof]}"
      dex $SV "tc qdisc replace dev eth0 root netem delay 1ms"
      echo "$rid $(date -u +%s) fwd=[${FWD[$prof]}]" >> "$OUT/RUNLOG"
      ext=${EXTH[$prof]}; wu=${WU[$prof]:-96}
      pol=$policy; extra=""; [[ $policy == f10 ]] && { pol=f15; extra="--entitlement-share-permille 100 --hardcap-share-permille 200"; }
      docker exec -d $SV bash -c "$BIN --role rx --bind 0.0.0.0:47002 --peer $CIP:47001 --run-id $rid --output-dir /app/results --extended-horizon $ext"
      # Blackout: server-egress-periyodik-%100 (reverse-SACK-karanlığı).
      if [[ $prof == p3_* ]]; then
        ( dex $SV "sleep 4; tc qdisc replace dev eth0 root netem loss 100%; sleep 10; tc qdisc replace dev eth0 root netem delay 1ms; sleep 8; tc qdisc replace dev eth0 root netem loss 100%; sleep 10; tc qdisc replace dev eth0 root netem delay 1ms" ) & BK=$!
      elif [[ $prof == p4_* ]]; then
        ( dex $SV "sleep 4; tc qdisc replace dev eth0 root netem loss 100%; sleep 30; tc qdisc replace dev eth0 root netem delay 1ms" ) & BK=$!
      fi
      docker exec $CS bash -c "timeout $((ext/1000+15)) $BIN --role tx --bind 0.0.0.0:47001 --peer $SIP:47002 --policy $pol $extra --run-id $rid --seed $((100+rep)) --output-dir /app/results --operational-horizon 12000 --extended-horizon $ext --w-units $wu" \
        || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
      [[ -n ${BK:-} ]] && { wait $BK 2>/dev/null || true; unset BK; }
      dex $SV "pkill -f 'udp_budget_runner --role rx'" 2>/dev/null || true
      dex $CS "test -s /app/results/$rid-summary.csv" || echo "MISSING-OUTPUT $rid" >> "$OUT/RUNLOG"
    done
  done
  echo "PROFIL-TAMAM: $prof" >> "$OUT/RUNLOG"
done
docker cp $CS:/app/results "$OUT/container-results" 2>/dev/null || true
echo "NETEM-CONTAINER-TAMAM: $OUT"
