#!/usr/bin/env bash
# [PERSIST AİLESİ] Persistence (reorder horizon) kalibrasyonu — A/B adaylarıyla birlikte.
# Soru: "reorder yüzünden L2 ne zaman yanlış açılıyor ve bunu cold-start'ı geciktirmeden nasıl engelleriz?"
# Modlar: P0 fixed α=500 (sevk) · P1 learned Q90 ρ1.25 α=500 · P2 learned Q95 ρ1.25 α=500 · P3 fixed α=1000 (kör referans)
# Politikalar: A=250-250-250 · B=250-375-375.  Hücreler: C0 temiz · C1a/b reorder(OWD-bağlı ömür) ·
# C2 jitter-reorder RTT200 · C3 jitter-reorder RTT50 (ömür > 0.5 srtt) · C4 kayıp+reorder · C5 kayıp (kontrol).
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: fec_persist_family.sh OUTDIR [REPS]}"; REPS="${2:-3}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=8500
declare -A FWD REV RTT
FWD[C0]="delay 100ms rate 1mbit";                                REV[C0]="delay 100ms rate 1mbit";          RTT[C0]=200
FWD[C1a]="delay 100ms 20ms reorder 15% 50% rate 1mbit";          REV[C1a]="delay 100ms rate 1mbit";         RTT[C1a]=200
FWD[C1b]="delay 100ms 20ms reorder 30% 50% rate 1mbit";          REV[C1b]="delay 100ms rate 1mbit";         RTT[C1b]=200
# C2/C3 (jitter-only) VAKUM ÇIKTI: netem `rate` gönderim zamanlarını monoton kılar, jitter reorder üretmez.
# İki-aşamalı: reorder aşaması GÖNDERİCİ tarafında (va0, rate'siz) → ömür = aşamanın base delay'i;
# gecikme+rate MID'de. Ömür ≤ OWD_fwd ≤ 0.5×RTT (simetrik); ömür > 0.5×srtt yalnız ASİMETRİK yolda.
declare -A SND
FWD[C2p]="delay 80ms rate 1mbit";   REV[C2p]="delay 100ms rate 1mbit"; RTT[C2p]=200; SND[C2p]="delay 20ms 15ms reorder 30% 50%"   # ömür ~20 ms = 0.1 srtt
FWD[C3p]="rate 1mbit";              REV[C3p]="delay 5ms rate 1mbit";   RTT[C3p]=45;  SND[C3p]="delay 40ms 10ms reorder 30% 50%"   # asimetrik: ömür ~40 ms = 0.9 srtt
FWD[C4]="delay 100ms 20ms loss 20% reorder 30% 50% rate 1mbit";  REV[C4]="delay 100ms loss 2% rate 1mbit";  RTT[C4]=200
FWD[C5]="delay 100ms loss 20% rate 1mbit";                       REV[C5]="delay 100ms loss 2% rate 1mbit";  RTT[C5]=200
POL_A="250-250-250"; POL_B="250-375-375"
mode_args() { case "$1" in
  P0) echo "--persist-mode fixed --persist-alpha 500";;
  P1) echo "--persist-mode learned --persist-alpha 500 --persist-rho 1250 --persist-q 900";;
  P2) echo "--persist-mode learned --persist-alpha 500 --persist-rho 1250 --persist-q 950";;
  P3) echo "--persist-mode fixed --persist-alpha 1000";; esac; }
arms_for() { if [ "$1" = "C0" ]; then echo "off A-P0"; else echo "off A-P0 A-P1 A-P2 A-P3 B-P0 B-P1 B-P2 B-P3"; fi; }
run() { # $1=dir $2=cell $3=arm $4=msgs
  local d="$1" cell="$2" arm="$3" M="$4"; mkdir -p "$d"; P=$((P+1))
  kapi3_netem "${FWD[$cell]}" "${REV[$cell]}"
  # gönderici-tarafı reorder aşaması (varsa); yoksa temizle
  if [ -n "${SND[$cell]:-}" ]; then tc qdisc replace dev va0 root netem ${SND[$cell]}; else tc qdisc del dev va0 root 2>/dev/null || true; fi
  local c="--transport kama-product --fec-profile 3 --payload 256 --fec-mode adaptive --loss-confirm-k 0"
  if [ "$arm" != "off" ]; then
    local pol="${arm%-*}" mode="${arm#*-}"; local trip; [ "$pol" = "A" ] && trip=$POL_A || trip=$POL_B
    IFS=- read -r l1 l2 g <<< "$trip"
    c="$c --fec-risk --spec-l1-permille $l1 --spec-l2-permille $l2 --gate-permille $g $(mode_args $mode)"
  fi
  printf 'cell=%s arm=%s msgs=%s rtt=%s fwd=[%s] snd=[%s]\n' "$cell" "$arm" "$M" "${RTT[$cell]}" "${FWD[$cell]}" "${SND[$cell]:-}" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 200 "$BENCH" client --server "10.10.1.1:$P" $c --msgs "$M" --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
}
CELLS="${CELLS_LIST:-C0 C1a C1b C2p C3p C4 C5}"; TOTAL=0
for cell in $CELLS; do for M in ${MSGS_LIST:-96 400}; do for a in $(arms_for $cell); do TOTAL=$((TOTAL+1)); done; done; done
echo "== PERSIST AİLESİ reps=$REPS hücre×akış×kol=$TOTAL toplam=$((TOTAL*REPS)) =="
START=$(date +%s); DONE=0
for rep in $(seq 1 "$REPS"); do
  for cell in $CELLS; do for M in ${MSGS_LIST:-96 400}; do for a in $(arms_for $cell); do
    run "$OUT/${cell}_m${M}_${a}/rep$rep" "$cell" "$a" "$M"; DONE=$((DONE+1))
  done; done; done
  echo "  rep $rep/$REPS bitti ($DONE/$((TOTAL*REPS)), t=$(( $(date +%s) - START ))s)"
done
echo "BITTI → $OUT ($(( $(date +%s) - START ))s)"
