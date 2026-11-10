#!/usr/bin/env bash
# mini-Ö v2 r25 ablasyonu (ONKAYIT-V2 §6; v2 okuması R3-r25 "hayır" → TETİK). 8 dosya, 160 iş. Orkestratör (D-18, D-34 gündüz).
set -u
R=$HOME/kama-wt/yol2-goster
OUT=$HOME/kama-k4g-work/yol3/mini-o/2026-11-10-abl-r25
LOG=$OUT/KOSU-KAYDI.txt
fin() { echo "$1" >> "$LOG"; touch "$OUT/BITTI"; exit "${2:-1}"; }
cd "$R" || fin "worktree yok" 2
[ -z "$(git status --porcelain)" ] || fin "REDDEDİLDİ: yol2-goster kirli" 2
export BIN_GOSTER_BENCH=$HOME/kama-wt/target-goster/release/bench
{ echo "tetik: mini-Ö v2 okuması (OKUMA.md sha256 bcaa17e9a0de8e17043bebcbf9105afcdc49801ee07e4c1a60875cb758fdc471) R3-r25 hayır (oran 1,116)";
  echo "ağaç: yol2/goster $(git rev-parse HEAD)"; sha256sum "$BIN_GOSTER_BENCH" scripts/yol3/mini-o/SIRA-ABL-r25.txt scripts/yol3/mini-o/ONKAYIT-V2.md;
  echo "diğer cargo/rustc: $(pgrep -c -x cargo || true)/$(pgrep -c -x rustc || true)"; echo "ölçüm başlangıç $(date -Is)"; } >> "$LOG"
flock "$HOME/.cache/kama-cargo.lock" systemd-inhibit --what=sleep:idle --why="YOL3 mini-O abl r25" \
  scripts/ktp2/f0/f0_grid.sh -j 11 "$OUT" $(cat scripts/yol3/mini-o/SIRA-ABL-r25.txt) >> "$OUT/grid.log" 2>&1
echo "grid rc=$? bitiş $(date -Is)" >> "$LOG"
python3 -I scripts/yol3/mini-o/oku.py --surum abl --hucre r25 --kok "$OUT" --onkayit scripts/yol3/mini-o/ONKAYIT-V2.md > "$OUT/oku.log" 2>&1
echo "oku rc=$?" >> "$LOG"
sha256sum "$OUT"/okuma*/* >> "$LOG" 2>/dev/null
touch "$OUT/BITTI"
