#!/usr/bin/env bash
# kullanım: kos.sh <tohum>   (tek kilit altında bir senaryo v1 koşusu; v1/arac/kos.sh ile aynı, fark: çıktı kökü ve ikili)
t="$1"; out=<scratch>/olcum/kancali/$t; mkdir -p "$out"
{ echo "başlangıç $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
  flock "$HOME/.cache/kama-cargo-run.lock" env V1_SEED="$t" HDIR=<scratch>/olcum/v1/arac/hdir BIN_F0A_ADAY=<scratch>/olcum/kancali/arac/f0_flows.e1012cf2 bash <wt>/olcum-ent/scripts/ktp2/f0/f0_cell.sh <scratch>/olcum/v1/arac/v1_taban.cell "$out"
  echo "rc=$?"
  echo "bitiş $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
} > "$out/kosu.log" 2>&1
