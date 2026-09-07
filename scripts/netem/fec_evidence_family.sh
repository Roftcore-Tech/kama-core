#!/usr/bin/env bash
# [EVIDENCE AİLESİ] Kanıt-kütlesi (evidence-mass) taraması — YALNIZ spekülatif L2 yetkisi.
# Soru: "kütle küçükken (kısa akış başı) L2 yetkisini kısmak yanlış-L2/tel maliyetini düşürür mü,
#        gerçek kayıpta 2×RTT teslimi düşürmeden ve L1'i geciktirmeden?"
# Persistence P2 (learned Q95×1.25) sabit; politikalar A=250-250-250 · B=250-375-375 (yeniden kalibre YOK).
# Modlar: M0 sevk (kütle okunmaz) · M1 floor (--em-mode floor --em-min N) · M2 scaled (--em-mode scaled --em-full N).
# Hücreler: E1-L{0,2,5,10,15,20,25} kayıp-only RTT200 (rsweep tabanı) · E2-C1b reorder 30% (ömür 0.5 srtt) ·
#           E2-C3p asimetrik reorder (ömür 0.9 srtt, va0 aşaması) · E3-C4 kayıp 20% + reorder 30%.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: fec_evidence_family.sh OUTDIR [REPS]}"; REPS="${2:-5}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=8700
declare -A FWD REV RTT SND
for L in 0 2 5 10 15 20 25; do
  if [ "$L" = 0 ]; then FWD[E1-L0]="delay 100ms rate 1mbit"; REV[E1-L0]="delay 100ms rate 1mbit"
  else FWD[E1-L$L]="delay 100ms loss ${L}% rate 1mbit"; REV[E1-L$L]="delay 100ms loss 2% rate 1mbit"; fi
  RTT[E1-L$L]=200
done
FWD[E2-C1b]="delay 100ms 20ms reorder 30% 50% rate 1mbit"; REV[E2-C1b]="delay 100ms rate 1mbit"; RTT[E2-C1b]=200
FWD[E2-C3p]="rate 1mbit"; REV[E2-C3p]="delay 5ms rate 1mbit"; RTT[E2-C3p]=45; SND[E2-C3p]="delay 40ms 10ms reorder 30% 50%"
FWD[E3-C4]="delay 100ms 20ms loss 20% reorder 30% 50% rate 1mbit"; REV[E3-C4]="delay 100ms loss 2% rate 1mbit"; RTT[E3-C4]=200
POL_A="250-250-250"; POL_B="250-375-375"
P2="--persist-mode learned --persist-alpha 500 --persist-rho 1250 --persist-q 950"
EM_MIN="${EM_MIN:-32}"; EM_FULL="${EM_FULL:-64}"
mode_args() { case "$1" in
  M0) echo "";;
  M1) echo "--em-mode floor --em-min $EM_MIN";;
  M2) echo "--em-mode scaled --em-full $EM_FULL";; esac; }
ARMS="${ARMS_LIST:-off A-M0 B-M0}"
run() { # $1=dir $2=cell $3=arm $4=msgs
  local d="$1" cell="$2" arm="$3" M="$4"; mkdir -p "$d"; P=$((P+1))
  kapi3_netem "${FWD[$cell]}" "${REV[$cell]}"
  if [ -n "${SND[$cell]:-}" ]; then tc qdisc replace dev va0 root netem ${SND[$cell]}; else tc qdisc del dev va0 root 2>/dev/null || true; fi
  local c="--transport kama-product --fec-profile 3 --payload 256 --fec-mode adaptive --loss-confirm-k 0"
  local extra=""
  if [ "$arm" != "off" ]; then
    local pol="${arm%-*}" mode="${arm#*-}"; local trip; [ "$pol" = "A" ] && trip=$POL_A || trip=$POL_B
    IFS=- read -r l1 l2 g <<< "$trip"
    c="$c --fec-risk --spec-l1-permille $l1 --spec-l2-permille $l2 --gate-permille $g $P2 $(mode_args $mode)"
    extra="--ts-out $d/ts.csv"
  fi
  printf 'cell=%s arm=%s msgs=%s rtt=%s em_min=%s em_full=%s fwd=[%s] snd=[%s]\n' "$cell" "$arm" "$M" "${RTT[$cell]}" "$EM_MIN" "$EM_FULL" "${FWD[$cell]}" "${SND[$cell]:-}" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 200 "$BENCH" client --server "10.10.1.1:$P" $c --msgs "$M" --submit-interval-ms 20 \
    --max-deadline-s 20 $extra --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
}
CELLS="${CELLS_LIST:-E1-L0 E1-L2 E1-L5 E1-L10 E1-L15 E1-L20 E1-L25 E2-C1b E2-C3p E3-C4}"; TOTAL=0
for cell in $CELLS; do for M in ${MSGS_LIST:-96}; do for a in $ARMS; do TOTAL=$((TOTAL+1)); done; done; done
echo "== EVIDENCE AİLESİ reps=$REPS hücre×akış×kol=$TOTAL toplam=$((TOTAL*REPS)) em_min=$EM_MIN em_full=$EM_FULL =="
START=$(date +%s); DONE=0
for rep in $(seq 1 "$REPS"); do
  for cell in $CELLS; do for M in ${MSGS_LIST:-96}; do for a in $ARMS; do
    run "$OUT/${cell}_m${M}_${a}/rep$rep" "$cell" "$a" "$M"; DONE=$((DONE+1))
    [ $((DONE % 10)) = 0 ] && echo "  $DONE/$((TOTAL*REPS)) $(( $(date +%s) - START ))s"
  done; done; done
done
echo "== BİTTİ $(( $(date +%s) - START ))s =="
