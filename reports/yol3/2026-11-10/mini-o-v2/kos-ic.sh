#!/usr/bin/env bash
# mini-Ö v2 (ONKAYIT-V2.md; sahip kararı 2026-11-10 madde 3, 4, 5): yol2/goster, iki ayrı süreç, 7 dosya / 90 iş.
# Orkestratör (D-18, D-34 gündüz). Ortak cargo kilidi koşu boyunca tutulur; ajan ve cargo durur. Ayrık süreç.
set -u
R=$HOME/kama-wt/yol2-goster
OUT=$HOME/kama-k4g-work/yol3/mini-o/2026-11-10-v2
LOG=$OUT/KOSU-KAYDI.txt
fin() { echo "$1" >> "$LOG"; touch "$OUT/BITTI"; exit "${2:-1}"; }
cd "$R" || fin "worktree yok" 2
[ -z "$(git status --porcelain)" ] || fin "REDDEDİLDİ: yol2-goster kirli" 2
export BIN_GOSTER=$HOME/kama-wt/target-goster/release/examples/f0_flows BIN_GOSTER_BENCH=$HOME/kama-wt/target-goster/release/bench
{ echo "ağaç: yol2/goster $(git rev-parse HEAD)"; echo "commit'ler 3beab598..HEAD:"; git log --oneline 3beab598..HEAD;
  sha256sum "$BIN_GOSTER" "$BIN_GOSTER_BENCH" scripts/yol3/mini-o/SIRA-V2.txt scripts/yol3/mini-o/ONKAYIT-V2.md;
  echo "KK-ORK-01: FEC kapalı. Gösterim seti K1A6s (A, --goster on, goster.rs) / K1A6 (b, bench); taban K0; W 5, T 2,0 s. f0_grid -j 11."
  echo "diğer cargo/rustc: $(pgrep -c -x cargo || true)/$(pgrep -c -x rustc || true)"; } >> "$LOG"
echo "ölçüm başlangıç $(date -Is)" >> "$LOG"
flock "$HOME/.cache/kama-cargo.lock" systemd-inhibit --what=sleep:idle --why="YOL3 mini-O v2" \
  scripts/ktp2/f0/f0_grid.sh -j 11 "$OUT" $(cat scripts/yol3/mini-o/SIRA-V2.txt) >> "$OUT/grid.log" 2>&1
echo "grid rc=$? bitiş $(date -Is)" >> "$LOG"
python3 -I scripts/yol3/mini-o/oku.py --surum v2 --kok "$OUT" --onkayit scripts/yol3/mini-o/ONKAYIT-V2.md > "$OUT/oku.log" 2>&1
echo "oku rc=$?" >> "$LOG"
sha256sum "$OUT"/okuma/* >> "$LOG" 2>/dev/null
touch "$OUT/BITTI"
