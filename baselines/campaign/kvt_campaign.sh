#!/usr/bin/env bash
# KAMA ↔ TCP kampanyası — TEK KOMUT koşucu. Donmuş manifestleri (R1 sonra R2) 4 işçiyle koşar.
# Her işçi KENDİ rootless 3-netns topolojisinde, kendi hücre-bloklarını SERİ koşar (blok bölünmez).
# Devam-edilebilir: yeniden çağrılırsa üretilmiş satırlar atlanır (üstüne yazılmaz).
# Kullanım: kvt_campaign.sh MANIFEST_DIR OUT_DIR      (OUT_DIR kalıcı diskte olmalı; /tmp tmpfs olabilir)
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"; MAN="${1:?MANIFEST_DIR}"; OUT="${2:?OUT_DIR}"; WORKERS="${KVT_WORKERS:-4}"
mkdir -p "$OUT"
for REP in R1 R2; do
  echo "$(date -Is) REPLICA $REP start" >> "$OUT/campaign.log"
  pids=()
  for w in $(seq 0 $((WORKERS-1))); do
    KVT_WORKER=$w bash "$HERE/kvt_run.sh" "$MAN/manifest-$REP.csv" "$OUT/$REP/w$w" >> "$OUT/$REP-w$w.stdout" 2>&1 &
    pids+=($!); sleep 1
  done
  for p in "${pids[@]}"; do wait "$p"; done
  echo "$(date -Is) REPLICA $REP done" >> "$OUT/campaign.log"
done
echo "$(date -Is) CAMPAIGN-DONE" >> "$OUT/campaign.log"
