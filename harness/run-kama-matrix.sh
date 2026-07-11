#!/usr/bin/env bash
# harness/run-kama-matrix.sh — Kama killer-chart matrisi: 7 profil × N repeat (cubic-only; Kama tek
# adaptif controller → cubic=bbr). quinn baseline (docs/results/quinn-baseline-v1.md) ile karşılaştırma;
# netem profilleri AYNI (§4.4.9 reproducible). sent_ns-fixed binary (app-offer frame, review-hardened).
set -euo pipefail
cd "$(dirname "$0")/.."
source harness/netem-profiles.sh

REPEATS="${REPEATS:-5}"
PROFILES="${PROFILES:-p1_loss50 p2_loss80 p3_loss95 p4_jitter p5_revpath p6_blackout p7_bbr}"
MSGS="${MSGS:-100}"; PAYLOAD="${PAYLOAD:-256}"; MAX_DEADLINE_S="${MAX_DEADLINE_S:-120}"
CLIENT=kama-bench-client; SERVER=kama-bench-server
BIN=/app/target/release/bench

mkdir -p results
SERVER_IP=$(docker exec "$CLIENT" getent hosts "$SERVER" | awk '{print $1}')

for profile in $PROFILES; do
  for r in $(seq 0 $((REPEATS - 1))); do
    docker exec -d "$SERVER" "$BIN" serve --transport kama --bind "[::]:4433"
    sleep 1
    toggler=""
    if [ "$profile" = "p6_blackout" ]; then
      ( while true; do
          docker exec "$CLIENT" tc qdisc replace dev eth0 root netem loss 100% 2>/dev/null || true
          docker exec "$SERVER" tc qdisc replace dev eth0 root netem loss 100% 2>/dev/null || true
          sleep 2; clear_netem "$CLIENT" "$SERVER"; sleep 8
        done ) & toggler=$!
    else
      apply_profile "$profile" "$CLIENT" "$SERVER"
    fi

    docker exec "$CLIENT" "$BIN" client --transport kama \
      --server "${SERVER_IP}:4433" --msgs "$MSGS" --payload "$PAYLOAD" \
      --max-deadline-s "$MAX_DEADLINE_S" --seed "$r" --repeat "$r" \
      --profile "$profile" --out "/app/results/kama_${profile}_r${r}.jsonl" || true

    [ -n "$toggler" ] && kill "$toggler" 2>/dev/null || true
    clear_netem "$CLIENT" "$SERVER"
    docker exec "$SERVER" pkill -f 'bench serve' 2>/dev/null || true
    echo "✓ kama / $profile / r$r"
  done
done
echo "KAMA MATRIX DONE"
