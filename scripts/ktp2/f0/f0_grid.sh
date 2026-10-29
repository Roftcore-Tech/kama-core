#!/usr/bin/env bash
# [KTP2-F0] Hücre kuyruğu: en çok J hücre aynı anda (her hücre kendi `unshare -rnm` ad alanında).
# Kullanım: f0_grid.sh -j J ÇIKTI_KÖKÜ HÜCRE1.cell [HÜCRE2.cell …]
#   Bir girdi `KÖK::HÜCRE.cell` biçiminde verilirse o hücre KÖK altına yazılır (şerit başına kök;
#   ortak öncelik sıralı tek kuyruk için). grid.log her zaman ÇIKTI_KÖKÜ'ndedir.
# DONE'u olan hücre atlanır (yeniden koşu değildir; yarım kalmış hücre dizini elle incelenir).
# Kayıt: <kök>/grid.log (başlangıç/bitiş/rc, UTC).
# Ortak yuva (isteğe bağlı): F0_SLOTS_DIR + F0_SLOTS_MAX verilirse birden çok kuyruk (şerit) aynı
# makinede toplam en çok F0_SLOTS_MAX hücreyle koşar (mkdir kilidi; yuva hücre bitince bırakılır).
# Her kuyruk kendi sırasını korur; kuyruklar arası sıra garanti edilmez.
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
  root="$ROOT"
  case "$spec" in *::*) root="${spec%%::*}"; spec="${spec#*::}"; mkdir -p "$root"; root="$(readlink -f "$root")";; esac
  (
    name="$(basename "$spec" .cell)"
    slot=""
    if [ -n "${F0_SLOTS_DIR:-}" ]; then
      mkdir -p "$F0_SLOTS_DIR"
      while [ -z "$slot" ]; do
        for i in $(seq 1 "${F0_SLOTS_MAX:?F0_SLOTS_MAX yok}"); do
          if mkdir "$F0_SLOTS_DIR/slot.$i" 2>/dev/null; then
            slot="$F0_SLOTS_DIR/slot.$i"; echo "$root $name" > "$slot/owner"; break
          fi
        done
        [ -z "$slot" ] && sleep 5
      done
      trap 'rm -f "$slot/owner"; rmdir "$slot" 2>/dev/null' EXIT
    fi
    echo "BAŞLA $name $(date -u +%FT%TZ)${slot:+ yuva=$(basename "$slot")} kök=$root" >> "$LOG"
    bash "$SELFDIR/f0_cell.sh" "$spec" "$root" > "$root/$name.log" 2>&1
    echo "BİTTİ $name rc=$? $(date -u +%FT%TZ)" >> "$LOG"
  ) &
  running=$((running+1))
done
wait
echo "GRID bitiş $(date -u +%FT%TZ)" >> "$LOG"
