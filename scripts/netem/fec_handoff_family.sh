#!/usr/bin/env bash
# [HANDOFF AİLESİ] Spekülatif → kanonik devir teslimde "cliff" var mı?
# İnvaryant: kanonik sinyalin gelmesi FEC kurtarmasını KÖTÜLEŞTİREMEZ.
# TEK EKSEN: --fec-risk; akış uzunluğu {96,400,1000} × kayıp {5,10,20}; reorder YOK; 50 ms zaman serisi.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: fec_handoff_family.sh OUTDIR [REPS]}"; REPS="${2:-3}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=7700
run() {
  local d="$1" risk="$2" L="$3" M="$4"
  mkdir -p "$d"; P=$((P+1))
  kapi3_netem "delay 100ms loss ${L}% rate 1mbit" "delay 100ms loss 2% rate 1mbit"
  local c="--transport kama-product --fec-profile 3 --payload 256 --fec-mode adaptive --loss-confirm-k 0"
  [ "$risk" = "1" ] && c="$c --fec-risk"
  printf 'risk=%s loss=%s msgs=%s\n' "$risk" "$L" "$M" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 200 "$BENCH" client --server "10.10.1.1:$P" $c --msgs "$M" --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" --ts-out "$d/ts.csv" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
}
echo "== HANDOFF AİLESİ reps=$REPS  msgs={${MSGS_LIST:-96 400 1000}} × kayıp={${LOSS_LIST:-5 10 20}} × kollar={${ARMS_LIST:-0 1}}  RTT=200ms 1mbit =="
START=$(date +%s)
for rep in $(seq 1 "$REPS"); do
  for M in ${MSGS_LIST:-96 400 1000}; do for L in ${LOSS_LIST:-5 10 20}; do for R in ${ARMS_LIST:-0 1}; do
    run "$OUT/m${M}_l${L}_r${R}/rep$rep" "$R" "$L" "$M"
  done; done; done
  echo "  rep $rep/$REPS bitti (t=$(( $(date +%s) - START ))s)"
done
echo "BITTI → $OUT ($(( $(date +%s) - START ))s)"
