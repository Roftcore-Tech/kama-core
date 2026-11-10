#!/usr/bin/env bash
# Üç gece cargo yuvasını (cargo-gece.sh) tutar → ölçüm süresince ajan derlemesi bekler; -o: kilit fd'si çocuklara geçmez.
OUT=<ölçüm-kökü>/yol3/tesis/2026-11-10-a6
echo "başlangıç $(date -Is); yuvalar bekleniyor" >> "$OUT/KOSU-KAYDI.txt"
exec flock -o "$HOME/.cache/kama-cargo-slot1.lock" flock -o "$HOME/.cache/kama-cargo-slot2.lock" flock -o "$HOME/.cache/kama-cargo-slot3.lock" bash "$OUT/kos-ic.sh"
