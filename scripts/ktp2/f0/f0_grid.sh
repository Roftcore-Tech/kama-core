#!/usr/bin/env bash
# [KTP2-F0] Hücre kuyruğu: en çok J hücre aynı anda (her hücre kendi `unshare -rnm` ad alanında).
# Kullanım: f0_grid.sh -j J ÇIKTI_KÖKÜ HÜCRE1.cell [HÜCRE2.cell …]
# DONE'u olan hücre atlanır (yeniden koşu değildir; yarım kalmış hücre dizini elle incelenir).
# Kayıt: <kök>/grid.log (başlangıç/bitiş/rc, UTC).
set -u
J=3
[ "${1:-}" = "-j" ] && { J="$2"; shift 2; }
ROOT="${1:?kullanım: f0_grid.sh -j J ÇIKTI_KÖKÜ HÜCRE…}"; shift
mkdir -p "$ROOT"; ROOT="$(readlink -f "$ROOT")"
SELFDIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
LOG="$ROOT/grid.log"
echo "GRID başlangıç $(date -u +%FT%TZ) j=$J hücre=$#" >> "$LOG"
running=0
for spec in "$@"; do
  while [ "$running" -ge "$J" ]; do wait -n 2>/dev/null || true; running=$((running-1)); done
  (
    name="$(basename "$spec" .cell)"
    echo "BAŞLA $name $(date -u +%FT%TZ)" >> "$LOG"
    bash "$SELFDIR/f0_cell.sh" "$spec" "$ROOT" > "$ROOT/$name.log" 2>&1
    echo "BİTTİ $name rc=$? $(date -u +%FT%TZ)" >> "$LOG"
  ) &
  running=$((running+1))
done
wait
echo "GRID bitiş $(date -u +%FT%TZ)" >> "$LOG"
