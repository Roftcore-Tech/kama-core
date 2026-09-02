#!/usr/bin/env bash
# [S2-DIAG] 3-açık-kalem teşhisi: per-rep TAM artefakt (c.err/s.err/out.jsonl) saklanır.
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
OUT="$1"
P1="$REPO/target/release/examples/p1_endpoint"; BENCH="$REPO/target/release/bench"
if [ -z "${S2_INNER:-}" ]; then S2_INNER=1 exec unshare -rnm bash "$0" "$OUT"; fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
mkdir -p "$OUT"; PORT=4800

kama_rep() { # $1 dir $2 fecargs $3 msgs $4 int $5 D
  mkdir -p "$1"; PORT=$((PORT+1))
  nsenter -t "$CHLB" -n "$P1" server --bind "10.10.1.1:$PORT" --expect "$3" --deadline-s "$5" --linger-s 2 $2 2>"$1/s.err" >/dev/null &
  local SRV=$!; sleep 1
  "$P1" client --server "10.10.1.1:$PORT" --units "$3" --interval-ms "$4" --deadline-s "$5" $2 2>"$1/c.err" >/dev/null || true
  wait "$SRV" 2>/dev/null || true
}
quinn_rep() { # $1 dir $2 msgs $3 int $4 D
  mkdir -p "$1"; PORT=$((PORT+1))
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$PORT" --transport quinn 2>"$1/s.err" >/dev/null &
  local SRV=$!; sleep 1
  timeout $(( $4 + 25 )) "$BENCH" client --server "10.10.1.1:$PORT" --transport quinn --msgs "$2" --payload 48 \
    --submit-interval-ms "$3" --max-deadline-s "$4" --out "$1/out.jsonl" 2>"$1/c.err" >/dev/null || true
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
}

echo "== K1: p2fec delivered-metrik (n=3 × 2 kol) =="
kapi3_netem "delay 0ms loss 80% rate 200kbit" "delay 0ms loss 80% rate 200kbit"
for r in 1 2 3; do kama_rep "$OUT/p2fec/kama-off/rep$r" "" 80 1000 120; done
for r in 1 2 3; do kama_rep "$OUT/p2fec/kama-fec/rep$r" "--fec-r 4" 80 1000 120; done

echo "== K2: rt60d300 kama 0/40 teşhisi (n=6) =="
kapi3_netem "delay 300ms loss 60% rate 5mbit" "delay 300ms loss 60% rate 5mbit"
for r in 1 2 3 4 5 6; do kama_rep "$OUT/rt60d300/kama-fec/rep$r" "--fec-r 4" 40 100 10; done

echo "== K3: qbl60d150 quinn bimodallik (n=6) =="
kapi3_netem "delay 150ms loss 60% rate 200kbit" "delay 150ms loss 60% rate 200kbit"
for r in 1 2 3 4 5 6; do quinn_rep "$OUT/qbl60d150/quinn/rep$r" 40 250 45; done

echo "== ÖZET =="
for f in "$OUT"/p2fec/*/rep*/s.err; do echo "$f: $(grep -oE 'delivered=[0-9]+/[0-9]+' "$f" | head -1)"; done
for f in "$OUT"/rt60d300/*/rep*/c.err; do echo "$f: $(grep -oE 'acked=[0-9]+/[0-9]+' "$f" | head -1)"; done
echo "BITTI → $OUT"
