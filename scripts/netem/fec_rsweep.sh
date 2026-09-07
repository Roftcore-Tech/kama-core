#!/usr/bin/env bash
# [R-SWEEP] Repair-quantum sweep: L1 / L2 / yapı-gate seviyeleri; reorder YOK.
# Soru: "FEC açıldığında kaç repair gerçekten ekonomik?" → teslim(≤2×RTT) vs wire Pareto.
#
# KESİRLİ TASARIM (owner onaylı gerekçeler):
#  * gate ekseni yalnız 400 msg × ≥15% kayıpta canlı (onaylı oran ≥100k'a ancak orada ulaşır)
#  * 0% kayıp: risk=0 → tüm politikalar özdeş → yalnız {off, mevcut}
#  * L1 ekseni reorder'sız neredeyse inert (bedava, dahil)
# Politika kodu: p<L1>-<L2>-<G> (‰; K=8'de 125=r1 250=r2 375=r3 500=r4). Mevcut = p125-250-500.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: fec_rsweep.sh OUTDIR [REPS]}"; REPS="${2:-5}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=8000
CUR="125-250-500"
FOUR="125-250-500 250-250-500 125-375-500 250-375-500"
TWELVE=""; for l1 in 125 250; do for l2 in 250 375; do for g in 250 375 500; do TWELVE="$TWELVE $l1-$l2-$g"; done; done; done
arms_for() { # $1=msgs $2=loss → kol listesi
  local M=$1 L=$2
  if [ "$L" = "0" ]; then echo "off $CUR"
  elif [ "$M" = "400" ] && [ "$L" -ge 15 ]; then echo "off $TWELVE"
  else echo "off $FOUR"; fi
}
run() { # $1=dir $2=arm $3=loss $4=msgs
  local d="$1" arm="$2" L="$3" M="$4"; mkdir -p "$d"; P=$((P+1))
  if [ "$L" = "0" ]; then kapi3_netem "delay 100ms rate 1mbit" "delay 100ms rate 1mbit"
  else kapi3_netem "delay 100ms loss ${L}% rate 1mbit" "delay 100ms loss 2% rate 1mbit"; fi
  local c="--transport kama-product --fec-profile 3 --payload 256 --fec-mode adaptive --loss-confirm-k 0"
  if [ "$arm" != "off" ]; then
    IFS=- read -r l1 l2 g <<< "$arm"
    c="$c --fec-risk --spec-l1-permille $l1 --spec-l2-permille $l2 --gate-permille $g"
  fi
  printf 'arm=%s loss=%s msgs=%s\n' "$arm" "$L" "$M" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 200 "$BENCH" client --server "10.10.1.1:$P" $c --msgs "$M" --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
}
TOTAL=0; for M in 96 400; do for L in 0 5 10 15 20 25; do for a in $(arms_for $M $L); do TOTAL=$((TOTAL+1)); done; done; done
echo "== R-SWEEP reps=$REPS  hücre×kol=$TOTAL  toplam koşu=$((TOTAL*REPS))  RTT=200ms 1mbit reorder=0 =="
START=$(date +%s); DONE=0
for rep in $(seq 1 "$REPS"); do
  for M in 96 400; do for L in 0 5 10 15 20 25; do for a in $(arms_for $M $L); do
    run "$OUT/f${M}_l${L}_${a}/rep$rep" "$a" "$L" "$M"; DONE=$((DONE+1))
  done; done; done
  echo "  rep $rep/$REPS bitti  ($DONE/$((TOTAL*REPS)), t=$(( $(date +%s) - START ))s)"
done
echo "BITTI → $OUT ($(( $(date +%s) - START ))s)"
