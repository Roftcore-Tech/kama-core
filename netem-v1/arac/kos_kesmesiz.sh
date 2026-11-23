#!/usr/bin/env bash
# kullanım: kos.sh <tohum>   (tek kilit altında bir senaryo v1 taban koşusu)
t="$1"; out=<scratch>/olcum/v1/kesmesiz-$t; mkdir -p "$out"
{ echo "başlangıç $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
  flock "$HOME/.cache/kama-cargo-run.lock" env V1_SEED="$t" HDIR=<scratch>/olcum/v1/arac/hdir BIN_F0A_ADAY=<scratch>/olcum/v1/arac/f0_flows.73715edb bash <wt>/olcum-ent/scripts/ktp2/f0/f0_cell.sh <scratch>/olcum/v1/arac/v1_kesmesiz.cell "$out"
  echo "rc=$?"
  echo "bitiş $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
} > "$out/kosu.log" 2>&1
