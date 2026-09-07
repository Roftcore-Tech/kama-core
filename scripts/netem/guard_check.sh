#!/usr/bin/env bash
# [KURULUŞ-GUARD doğrulaması] burst-fusion kolunu guard ÖNCESİ bankadaki sayılarla kıyaslanabilir
# biçimde tekrar koşar: AYNI hücre, AYNI config (reports/product-vs-quinn-v2 ile birebir).
# Guard hipotezi: PreConverge'de parity açmak kuruluşu kırıyordu → guard sonrası çöküş düşmeli.
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
OUT="${1:?kullanım: guard_check.sh OUTDIR}"; REPS="${2:-6}"
MSGS=100; PAYLOAD=256; DEADLINE=10; INTERVAL=50
BENCH="$REPO/target/release/bench"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${GC_INNER:-}" ]; then mkdir -p "$OUT"; GC_INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"; fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
PORT=5300; SRV_IP=10.10.1.1
run_rep() {
  local d="$1"; shift; local extra="$*"
  mkdir -p "$d"; PORT=$((PORT+1))
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "$SRV_IP:$PORT" --transport kama-product $extra --payload $PAYLOAD 2>"$d/s.err" >/dev/null &
  local SRV=$!; sleep 1
  # shellcheck disable=SC2086
  timeout $((DEADLINE+40)) "$BENCH" client --server "$SRV_IP:$PORT" --transport kama-product \
    --msgs $MSGS --payload $PAYLOAD --submit-interval-ms $INTERVAL --max-deadline-s $DEADLINE \
    --out "$d/out.jsonl" $extra 2>"$d/c.err" >/dev/null || true
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
}
for cell in iid50 burst50; do
  case $cell in
    iid50)   kapi3_netem "delay 200ms loss 50%"     "delay 200ms loss 10%" ;;
    burst50) kapi3_netem "delay 200ms loss 50% 50%" "delay 200ms loss 10%" ;;
  esac
  echo "=== $cell ==="
  # TEK DEĞİŞKEN izolasyonu: İKİ kol da cold-start düzeltmeleriyle koşar (IW + bootstrap),
  # tek fark burst-fusion politikasıdır. Aksi hâlde politika etkisi, kuruluş kırılganlığıyla
  # karışır — ilk denememdeki confound tam buydu.
  COMMON="--initial-window-bytes 2400 --bootstrap-rtt --fec-profile 3 --fec-r 0"
  # shellcheck disable=SC2086
  for r in $(seq 1 "$REPS"); do run_rep "$OUT/$cell/adaptive/rep$r" $COMMON; done
  # shellcheck disable=SC2086
  for r in $(seq 1 "$REPS"); do run_rep "$OUT/$cell/burstfus/rep$r" $COMMON --fec-burst-fusion; done
  echo "  adaptive + burstfus: $REPS rep koştu"
done
echo "BITTI → $OUT"
