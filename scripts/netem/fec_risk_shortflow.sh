#!/usr/bin/env bash
# [FEC-RISK KISA-AKIŞ AİLESİ] Tahmincinin VAROLUŞ GEREKÇESİNİ sınar:
# "akış, kanonik onay ufkundan (≈3.15 s) ÖNCE bitiyorsa FEC hiç kullanılabilir bilgi görmüyor."
#
# TEK EKSEN: --fec-risk (OFF/ON). Diğer her şey önceki ailelerle AYNI:
# kama-product · profil 0x03 · 256 B · 20 ms sunum aralığı · adaptif FEC · loss_confirm_k = 0.
# TEK FARK: 96 mesaj (kısa akış), önceki ailelerde 400'dü.
#
# Rep sırası ARALIKLI (rep dış döngü): koşu kesilirse tasarım DENGELİ kalır.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: fec_risk_shortflow.sh OUTDIR [REPS] [MSGS] [FAMILY]}"; REPS="${2:-5}"; MSGS_ARG="${3:-96}"; FAMILY="${4:-loss}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS" "$MSGS_ARG" "$FAMILY"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=7400
MSGS=$MSGS_ARG

run() {
  local d="$1" risk="$2" L="$3"
  mkdir -p "$d"; P=$((P+1))
  if [ "$FAMILY" = "reorder" ]; then
    # Saf reorder, kayıp YOK: L = reorder yüzdesi ("delay 100ms 20ms reorder L% 50%").
    if [ "$L" = "0" ]; then kapi3_netem "delay 100ms rate 1mbit" "delay 100ms rate 1mbit"
    else kapi3_netem "delay 100ms 20ms reorder ${L}% 50% rate 1mbit" "delay 100ms rate 1mbit"; fi
  elif [ "$L" = "0" ]; then kapi3_netem "delay 100ms rate 1mbit" "delay 100ms rate 1mbit"
  else kapi3_netem "delay 100ms loss ${L}% rate 1mbit" "delay 100ms loss 2% rate 1mbit"; fi
  local c="--transport kama-product --fec-profile 3 --payload 256 --fec-mode adaptive --loss-confirm-k 0"
  [ "$risk" = "1" ] && c="$c --fec-risk"
  printf 'risk=%s loss=%s msgs=%s\n' "$risk" "$L" "$MSGS" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 120 "$BENCH" client --server "10.10.1.1:$P" $c --msgs $MSGS --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
}

if [ "$FAMILY" = "reorder" ]; then LEVELS="0 5 15 30"; else LEVELS="0 2 5 10 15 20 25"; fi
echo "== AİLE=$FAMILY  msgs=$MSGS reps=$REPS  RTT=200ms rate=1mbit  seviyeler=[$LEVELS] =="
START=$(date +%s)
for rep in $(seq 1 "$REPS"); do
  for L in $LEVELS; do
    for R in 0 1; do run "$OUT/l${L}_r${R}/rep$rep" "$R" "$L"; done
  done
  echo "  rep $rep/$REPS bitti (t=$(( $(date +%s) - START ))s)"
done
echo "BITTI → $OUT ($(( $(date +%s) - START ))s)"
