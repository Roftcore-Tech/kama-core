#!/usr/bin/env bash
# kullanım: kos2.sh <hücre dosyası> <çıktı kökü> <tohum>
# Tek kilit ALINDIKTAN sonra konak yükü (1 dk) < 2,5 olana dek bekler (en çok 6 dk), sonra koşar.
cell="$1"; out="$2"; t="$3"; mkdir -p "$out"
exec 9>"$HOME/.cache/kama-cargo-run.lock"; flock 9
{ echo "kilit alındı $(date -u +%FT%T.%NZ) load=$(cut -d' ' -f1-3 /proc/loadavg)"
  for i in $(seq 1 180); do l=$(cut -d' ' -f1 /proc/loadavg); awk -v l="$l" 'BEGIN{exit !(l<2.5)}' && break; python3 -I -c 'import time; time.sleep(2)'; done
  echo "başlangıç $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
  env V1_SEED="$t" HDIR=<scratch>/olcum/v1/arac/hdir BIN_F0A_ADAY=<scratch>/olcum/v1/arac/f0_flows.73715edb bash <wt>/olcum-ent/scripts/ktp2/f0/f0_cell.sh "$cell" "$out" 9>&-
  echo "rc=$?"
  echo "bitiş $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
} > "$out/kosu.log" 2>&1
