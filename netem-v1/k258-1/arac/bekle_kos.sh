#!/usr/bin/env bash
# Konakta cargo/rustc kalmayınca VE 1 dk yük ortalaması < 4,0 olunca (art arda 3 yoklama, 1 s arayla; kilit boş) yinelemeyi başlatır. En çok 25 dk bekler.
K="$(cd "$(dirname "$0")/.." && pwd)"; ok=0
for _ in $(seq 1 1500); do
  if [ "$(pgrep -c -x 'cargo|rustc')" = 0 ] && flock -n "$HOME/.cache/kama-cargo-run.lock" true && awk '{exit !($1 < 4.0)}' /proc/loadavg; then ok=$((ok+1)); else ok=0; fi
  [ "$ok" -ge 3 ] && break
  sleep 1
done
[ "$ok" -ge 3 ] || { echo "BEKLEME DOLDU: cargo/rustc bitmedi"; exit 3; }
echo "boşta görüldü $(date -u +%FT%TZ) load=$(cat /proc/loadavg)"
bash "$K/arac/kos_yineleme.sh" 1
echo "yineleme bitti rc_dosyası=$(cat "$K/1-yineleme/v1_taban_t1/aday/rep1/rc" 2>/dev/null)"; cat "$K/1-yineleme/kosu.log"
