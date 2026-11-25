#!/usr/bin/env bash
# kullanım: kos.sh <tohum> [ek]   (tek kilit altında bir senaryo v1 koşusu; k258/arac/kos.sh ile aynı; farklar: çıktı kökü, ikili,
# koşucu ağacı (f0_cell.sh sha256 aynı: 14cffdad…) ve K2 penceresi için konak gözlemcisi: 0,5 s'de bir cargo/rustc süreç sayısı
# → <çıktı>/k2-ornek.txt (koşuya dokunmaz; yalnız pgrep). Çıktı: k258b/<tohum><ek>)
t="$1"; ek="${2:-}"; O=<scratch>/olcum; out=$O/k258b/$t$ek; mkdir -p "$out"
( while :; do n=$(pgrep -c -x 'cargo|rustc'); echo "$(date -u +%s.%N) $n $( [ "$n" != 0 ] && pgrep -x -l 'cargo|rustc' | tr '\n' ',' )"; sleep 0.5; done ) > "$out/k2-ornek.txt" 2>/dev/null &
GP=$!
{ echo "başlangıç $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
  flock "$HOME/.cache/kama-cargo-run.lock" env V1_SEED="$t" HDIR=$O/v1/arac/hdir BIN_F0A_ADAY=<wt>/f0k-ikili/f0_flows-b0659064c31dd0074372536c7bfb1f6935506032 bash <wt>/olcum-233b/scripts/ktp2/f0/f0_cell.sh $O/v1/arac/v1_taban.cell "$out"
  echo "rc=$?"
  echo "bitiş $(date -u +%FT%T.%NZ) load=$(cat /proc/loadavg) cargo_rustc=$(pgrep -c -x 'cargo|rustc') coredump_son=$(coredumpctl list --no-pager -n1 2>/dev/null | tail -1 | cut -c1-40)"
} > "$out/kosu.log" 2>&1
kill $GP 2>/dev/null; wait $GP 2>/dev/null
