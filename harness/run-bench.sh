#!/usr/bin/env bash
# harness/run-bench.sh — quinn baseline orkestratörü: 7 profil × {cubic,bbr} × N repeat × çoklu deadline.
# §4.4.9 reproducibility: results/ + metadata (image digest, netem script hash, seed, quinn, cc, date).
# Faz D'de SEN koşarsın (privileged değil — container NET_ADMIN cap; docker erişimi yeter).
set -euo pipefail
cd "$(dirname "$0")/.."
source harness/netem-profiles.sh

REPEATS="${REPEATS:-5}"                       # A: varyans için ≥5 tekrar
PROFILES="${PROFILES:-p1_loss50 p2_loss80 p3_loss95 p4_jitter p5_revpath p6_blackout p7_bbr}"
CCS="${CCS:-cubic bbr}"                        # §4.4.1: Cubic default + Bbr experimental
MSGS="${MSGS:-100}"; PAYLOAD="${PAYLOAD:-256}"; MAX_DEADLINE_S="${MAX_DEADLINE_S:-120}"
DEADLINES="${DEADLINES:-10,30,120}"           # B: completion-vs-deadline eğrisi
CLIENT=kama-bench-client; SERVER=kama-bench-server
BIN=/app/target/release/bench

mkdir -p results
# reproducibility metadata (§4.4.9)
printf '{"image_digest":"%s","netem_script_sha256":"%s","date":"%s","quinn":"0.11.11","repeats":%s,"deadlines_s":"%s"}\n' \
  "$(docker inspect --format '{{.Image}}' "$SERVER" 2>/dev/null || echo unknown)" \
  "$(sha256sum harness/netem-profiles.sh | cut -d' ' -f1)" \
  "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$REPEATS" "$DEADLINES" > results/run-metadata.json

# clap --server SocketAddr (IP) ister; server container IP'sini çöz.
SERVER_IP=$(docker exec "$CLIENT" getent hosts "$SERVER" | awk '{print $1}')

for profile in $PROFILES; do
  for cc in $CCS; do
    for r in $(seq 0 $((REPEATS - 1))); do
      docker exec -d "$SERVER" "$BIN" serve --bind "[::]:4433" --cc "$cc"
      sleep 1
      toggler=""
      if [ "$profile" = "p6_blackout" ]; then
        # ⑥ intermittent blackout: client koşarken 2s kesinti / 8s normal döngü (EW jamming).
        ( while true; do
            docker exec "$CLIENT" tc qdisc replace dev eth0 root netem loss 100% 2>/dev/null || true
            docker exec "$SERVER" tc qdisc replace dev eth0 root netem loss 100% 2>/dev/null || true
            sleep 2; clear_netem "$CLIENT" "$SERVER"; sleep 8
          done ) & toggler=$!
      else
        apply_profile "$profile" "$CLIENT" "$SERVER"
      fi

      out="results/${profile}_${cc}_r${r}.jsonl"
      docker exec "$CLIENT" "$BIN" client \
        --server "${SERVER_IP}:4433" --msgs "$MSGS" --payload "$PAYLOAD" \
        --max-deadline-s "$MAX_DEADLINE_S" --cc "$cc" --seed "$r" --repeat "$r" \
        --profile "$profile" --out "/app/$out" || true

      [ -n "$toggler" ] && kill "$toggler" 2>/dev/null || true
      clear_netem "$CLIENT" "$SERVER"
      docker exec "$SERVER" pkill -f 'bench serve' 2>/dev/null || true
      echo "✓ $profile / $cc / repeat $r → $out"
    done
    # (profil×cc) özeti: A varyans + B eğri
    runs=""; for r in $(seq 0 $((REPEATS - 1))); do runs="$runs /app/results/${profile}_${cc}_r${r}.jsonl"; done
    docker exec "$CLIENT" "$BIN" metrics --runs $runs --deadlines-s "$DEADLINES" \
      | tee "results/${profile}_${cc}_summary.txt"
  done
done
echo "TÜM koşular bitti → results/ (özet: *_summary.txt)"
