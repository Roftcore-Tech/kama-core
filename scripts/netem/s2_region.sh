#!/usr/bin/env bash
# [S2-REGION] Üstünlük-bölgesi haritası (SCREENING; white-paper girdisi — Abkarada onayı 2026-08-27):
# aynı iki-netns netem + aynı mesaj-planında {kama-FEC-ON, kama-OFF, quinn} completion.
# Kama = ürün-runtime (p1_endpoint, acked/units). Quinn = bench (out.jsonl delivered sayımı).
# DÜRÜSTLÜK: farklı binary'ler, aynı link+plan → SCREENING (ön-kayıtlı-eş değil; formal banka kapi-eval-v1).
# Kullanım: s2_region.sh <cells.txt> <out_dir> [REPS=2]
#   cells.txt satırı: <cell_id> <netem_fwd...>|<netem_rev...>|<msgs> <interval_ms> <deadline_s> <arms csv>
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
CELLS="$1"; OUT="$2"; REPS="${3:-2}"
P1="$REPO/target/release/examples/p1_endpoint"; BENCH="$REPO/target/release/bench"
[ -x "$P1" ] && [ -x "$BENCH" ] || { echo "release binaryleri kur"; exit 3; }
if [ -z "${S2_INNER:-}" ]; then S2_INNER=1 exec unshare -rnm bash "$0" "$CELLS" "$OUT" "$REPS"; fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
mkdir -p "$OUT"; PORT=4700
run_kama() { # $1 fecargs  $2 msgs $3 int $4 D  → echo acked
  PORT=$((PORT+1))
  nsenter -t "$CHLB" -n "$P1" server --bind "10.10.1.1:$PORT" --expect "$2" --deadline-s "$4" --linger-s 2 $1 2>/tmp/s.err >/dev/null &
  local SRV=$!; sleep 1
  "$P1" client --server "10.10.1.1:$PORT" --units "$2" --interval-ms "$3" --deadline-s "$4" $1 2>/tmp/c.err >/dev/null || true
  wait "$SRV" 2>/dev/null || true
  grep -oE 'acked=[0-9]+' /tmp/c.err | grep -oE '[0-9]+' | head -1
}
run_quinn() { # $2 msgs $3 int $4 D → echo delivered
  PORT=$((PORT+1))
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$PORT" --transport quinn 2>/dev/null >/dev/null &
  local SRV=$!; sleep 1
  "$BENCH" client --server "10.10.1.1:$PORT" --transport quinn --msgs "$2" --payload 48 \
    --max-deadline-s "$4" --submit-interval-ms "$3" --out /tmp/q.jsonl 2>/dev/null >/dev/null || true
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
  python3 -c "
import json,sys
n=0
try:
  for l in open('/tmp/q.jsonl'):
    try: e=json.loads(l).get('event')
    except: continue
    if e and e.get('delivered_ns') is not None: n+=1
except FileNotFoundError: pass
print(n)"
}
while IFS= read -r LINE; do
  [ -z "$LINE" ] || [ "${LINE:0:1}" = "#" ] && continue
  CELL=$(echo "$LINE" | awk '{print $1}')
  REST=${LINE#"$CELL" }
  NF=$(echo "$REST" | cut -d'|' -f1); NR=$(echo "$REST" | cut -d'|' -f2); TAIL=$(echo "$REST" | cut -d'|' -f3)
  MSGS=$(echo "$TAIL" | awk '{print $1}'); INT=$(echo "$TAIL" | awk '{print $2}'); D=$(echo "$TAIL" | awk '{print $3}'); ARMS=$(echo "$TAIL" | awk '{print $4}')
  kapi3_netem "$NF" "$NR"
  echo "=== CELL $CELL  fwd[$NF] rev[$NR] msgs=$MSGS int=${INT}ms D=${D}s ==="
  for ARM in ${ARMS//,/ }; do
    T=0
    for R in $(seq 1 "$REPS"); do
      case "$ARM" in
        kama-off)    V=$(run_kama "" "$MSGS" "$INT" "$D");;
        kama-fec)    V=$(run_kama "--fec-r 4" "$MSGS" "$INT" "$D");;
        kama-fec-ad) V=$(run_kama "--fec-adaptive" "$MSGS" "$INT" "$D");;
        quinn)       V=$(run_quinn _ "$MSGS" "$INT" "$D");;
      esac
      V=${V:-0}; T=$((T+V))
      printf '  %-11s rep%d: %d/%d\n' "$ARM" "$R" "$V" "$MSGS"
      echo "$CELL $ARM $R $V $MSGS" >> "$OUT/rows.txt"
    done
    printf 'CELL %-14s %-11s mean=%d/%d\n' "$CELL" "$ARM" $((T/REPS)) "$MSGS"
  done
done < "$CELLS"
echo "BITTI → $OUT/rows.txt"
