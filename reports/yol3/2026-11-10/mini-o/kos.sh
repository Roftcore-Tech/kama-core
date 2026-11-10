#!/usr/bin/env bash
# Gece a6 ızgarası bitene dek bekler, sonra üç gece cargo yuvasını tutarak mini-Ö'yü koşar.
OUT=<ölçüm-kökü>/yol3/mini-o/2026-11-10
A6=<ölçüm-kökü>/yol3/tesis/2026-11-10-a6
echo "başlangıç $(date -Is); a6 ızgarası bekleniyor" >> "$OUT/KOSU-KAYDI.txt"
until [ -f "$A6/BITTI" ]; do sleep 20; done
echo "a6 BITTI görüldü $(date -Is); yuvalar bekleniyor" >> "$OUT/KOSU-KAYDI.txt"
exec flock -o "$HOME/.cache/kama-cargo-slot1.lock" flock -o "$HOME/.cache/kama-cargo-slot2.lock" flock -o "$HOME/.cache/kama-cargo-slot3.lock" bash "$OUT/kos-ic.sh"
