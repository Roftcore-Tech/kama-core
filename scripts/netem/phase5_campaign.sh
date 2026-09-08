#!/usr/bin/env bash
# [FAZ-5] KAMPANYA SÜRÜCÜSÜ — hücre-shard'larını sınırlı paralellikle koşar.
#
# Her shard AYRI `unshare -rnm` netns'idir (phase5_cell.sh kendi içinde açar) → tam izolasyon.
# Shard başına: kendi timeout'u, kendi artefaktı, kendi DONE/BARRIER-FAIL damgası.
# Sessiz yeniden deneme YOK: düşen shard düşük kalır ve raporlanır.
#
# Kullanım: phase5_campaign.sh OUTDIR REPS DEADLINE_S MAXPAR CELL [CELL...]
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:?kullanım: phase5_campaign.sh OUTDIR REPS DEADLINE_S MAXPAR CELL...}"
REPS="${2:?}"; DEADLINE="${3:?}"; MAXPAR="${4:?}"; shift 4
CELLS=("$@")
mkdir -p "$OUT"
echo "== FAZ-5 KAMPANYA reps=$REPS D=${DEADLINE}s maxpar=$MAXPAR hücreler=${CELLS[*]} ==" | tee "$OUT/driver.log"
date -u +"başlangıç: %Y-%m-%dT%H:%M:%SZ" | tee -a "$OUT/driver.log"

# Shard başına azami süre: koşu-bütçesi × kol × tekrar + pay
per_run=$(( DEADLINE + 60 ))
shard_budget=$(( per_run * 3 * REPS + 300 ))

running=0
for cell in "${CELLS[@]}"; do
  while [ "$running" -ge "$MAXPAR" ]; do wait -n 2>/dev/null || true; running=$((running-1)); done
  (
    timeout "$shard_budget" bash "$DIR/phase5_cell.sh" "$cell" "$OUT" "$REPS" "$DEADLINE" \
      > "$OUT/${cell}.log" 2>&1
    rc=$?
    [ $rc -ne 0 ] && echo "SHARD-FAIL $cell rc=$rc" >> "$OUT/driver.log"
  ) &
  running=$((running+1))
done
wait
date -u +"bitiş: %Y-%m-%dT%H:%M:%SZ" | tee -a "$OUT/driver.log"
echo "== TAMAMLANAN SHARD'LAR ==" | tee -a "$OUT/driver.log"
for cell in "${CELLS[@]}"; do
  if [ -f "$OUT/$cell/DONE" ]; then echo "  DONE $cell" | tee -a "$OUT/driver.log"
  elif [ -f "$OUT/$cell/BARRIER-FAIL.txt" ]; then echo "  BARRIER-FAIL $cell (INFRA)" | tee -a "$OUT/driver.log"
  else echo "  EKSİK $cell (INFRA/TIMEOUT)" | tee -a "$OUT/driver.log"; fi
done
