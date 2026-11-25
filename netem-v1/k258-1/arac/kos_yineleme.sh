#!/usr/bin/env bash
# kullanım: kos_yineleme.sh <tohum>   (kos.sh ile aynı; tek fark çıktı kökü: <tohum>-yineleme)
t="$1"; O=<scratch>/olcum; out=$O/k258/$t-yineleme; mkdir -p "$out"
{ echo "başlangıç $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
  flock "$HOME/.cache/kama-cargo-run.lock" env V1_SEED="$t" HDIR=$O/v1/arac/hdir BIN_F0A_ADAY=<wt>/f0k-ikili/f0_flows-911f2c6dd1cc1bfcebb30e655f2e3c13760ff41a bash <wt>/olcum-ent/scripts/ktp2/f0/f0_cell.sh $O/v1/arac/v1_taban.cell "$out"
  echo "rc=$?"
  echo "bitiş $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
} > "$out/kosu.log" 2>&1
