#!/usr/bin/env bash
# mini-Ö v2 ön koşusu (sayılmaz): bayrak seti, T1 sayaçları, A-6 kaydı, b manifest doğrulaması. F0_REPS=1.
set -u
R=$HOME/kama-wt/yol2-goster
ON=$HOME/kama-k4g-work/yol3/mini-o/2026-11-10-v2-onkosu
mkdir -p "$ON"; LOG=$ON/KOSU-KAYDI.txt
cd "$R" || exit 2
[ -z "$(git status --porcelain)" ] || { echo "kirli ağaç" >> "$LOG"; exit 2; }
export BIN_GOSTER=$HOME/kama-wt/target-goster/release/examples/f0_flows BIN_GOSTER_BENCH=$HOME/kama-wt/target-goster/release/bench
{ echo "ön koşu $(date -Is) ağaç $(git rev-parse HEAD)"; sha256sum "$BIN_GOSTER" "$BIN_GOSTER_BENCH"; } >> "$LOG"
F0_REPS=1 flock "$HOME/.cache/kama-cargo.lock" systemd-inhibit --what=sleep:idle --why="YOL3 mini-O v2 on kosu" \
  scripts/ktp2/f0/f0_grid.sh -j 3 "$ON" scripts/yol3/mini-o/cells-v2/a_r12k_d150_l20_foto150k__K1A6s.cell \
  scripts/yol3/mini-o/cells-v2/b_r25_d300_l20_p65__K1A6.cell scripts/yol3/mini-o/cells-v2/b_r25_d300_l20_p65__K0.cell >> "$ON/grid.log" 2>&1
echo "grid rc=$? $(date -Is)" >> "$LOG"
python3 -I scripts/yol3/mini-o/oku.py --surum v2 --kok "$ON" --onkayit scripts/yol3/mini-o/ONKAYIT-V2.md > "$ON/oku.log" 2>&1
echo "oku rc=$?" >> "$LOG"
