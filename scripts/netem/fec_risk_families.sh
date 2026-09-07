#!/usr/bin/env bash
# [FEC-RISK DENEY AİLELERİ] Dört odaklı aile; her birinde TEK EKSEN değişir.
#
# Ortak sabitler: kama-product · profil 0x03 · 256 B yük · 400 mesaj · 20 ms sunum aralığı ·
# adaptif FEC (Controller + yapı-gate) · loss_confirm_k = 0 (SEVK ufku — kol karıştırılmaz).
# Tek değişen: --fec-risk (OFF/ON) ve ailenin kendi ekseni.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: fec_risk_families.sh OUTDIR [REPS]}"; REPS="${2:-2}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=7100

# $1=çıktı-dizini $2=risk(0/1) $3=fwd-netem $4=rev-netem
run() {
  local d="$1" risk="$2" fwd="$3" rev="$4"
  mkdir -p "$d"; P=$((P+1))
  kapi3_netem "$fwd" "$rev"
  local c="--transport kama-product --fec-profile 3 --payload 256 --fec-mode adaptive --loss-confirm-k 0"
  [ "$risk" = "1" ] && c="$c --fec-risk"
  printf 'risk=%s fwd=[%s] rev=[%s]\n' "$risk" "$fwd" "$rev" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 200 "$BENCH" client --server "10.10.1.1:$P" $c --msgs 400 --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
}

START=$(date +%s)
echo "== AİLE 1: kayıp, reorder YOK (FEC tepki gecikmesi) =="
for rep in $(seq 1 "$REPS"); do
  for L in 0 2 5 10 20; do
    if [ "$L" = "0" ]; then F="delay 100ms rate 1mbit"; else F="delay 100ms loss ${L}% rate 1mbit"; fi
    for R in 0 1; do run "$OUT/f1/l${L}_r${R}/rep$rep" "$R" "$F" "delay 100ms loss 2% rate 1mbit"; done
  done
  echo "  f1 rep $rep bitti (t=$(( $(date +%s) - START ))s)"
done

echo "== AİLE 2: reorder, kayıp YOK (yanlış-pozitif parity bedeli) =="
for rep in $(seq 1 "$REPS"); do
  for RO in 5 15 30; do
    F="delay 100ms 20ms reorder ${RO}% 50% rate 1mbit"
    for R in 0 1; do run "$OUT/f2/ro${RO}_r${R}/rep$rep" "$R" "$F" "delay 100ms rate 1mbit"; done
  done
  echo "  f2 rep $rep bitti (t=$(( $(date +%s) - START ))s)"
done

echo "== AİLE 3: kayıp + reorder (gerçekçi zor hâl) =="
for rep in $(seq 1 "$REPS"); do
  for CELL in "5 15" "20 15" "20 30"; do
    set -- $CELL; L=$1; RO=$2
    F="delay 100ms 20ms loss ${L}% reorder ${RO}% 50% rate 1mbit"
    for R in 0 1; do run "$OUT/f3/l${L}ro${RO}_r${R}/rep$rep" "$R" "$F" "delay 100ms loss 2% rate 1mbit"; done
  done
  echo "  f3 rep $rep bitti (t=$(( $(date +%s) - START ))s)"
done

echo "== AİLE 4: bant çökmesi, kayıp YOK (bant ≠ kayıp invaryantı) =="
for BW in 1mbit 10kbit 1kbit 2kbit 300bit; do
  run "$OUT/f4/rev${BW}" 1 "delay 100ms rate 1mbit" "delay 100ms rate ${BW}"
done
echo "BITTI → $OUT ($(( $(date +%s) - START ))s)"
