#!/usr/bin/env bash
# mini-Ö (sahip kararı v4 EK-A §6; ISEMRI-YOL3-O KK-Ö-11, KK-Ö-EKA-1): yol2/goster, iki ayrı süreç (f0_flows alice/bob, iki netns), N = 10.
# Orkestratör (D-18, D-34). kos.sh üç gece yuvasını tutarak çağırır. Ayrık süreç.
set -u
R=<worktree-kökü>/yol2-goster
OUT=<ölçüm-kökü>/yol3/mini-o/2026-11-10
LOG=$OUT/KOSU-KAYDI.txt
fin() { echo "$1" >> "$LOG"; touch "$OUT/BITTI"; exit "${2:-1}"; }
echo "yuvalar alındı (3/3) $(date -Is)" >> "$LOG"
cd "$R" || fin "worktree yok" 2
[ -z "$(git status --porcelain)" ] || fin "REDDEDİLDİ: yol2-goster kirli" 2
echo "ağaç: yol2/goster $(git rev-parse HEAD) (= 9e157992 + T1 9273e5b2 + Ö c45d81e4)" >> "$LOG"
echo "diğer cargo/rustc süreçleri: $(pgrep -c -x cargo || true)/$(pgrep -c -x rustc || true)" >> "$LOG"
export CARGO_TARGET_DIR=<worktree-kökü>/target-goster CARGO_BUILD_JOBS=10
cargo +1.96.0 build --release --locked -p kama-integration --example f0_flows >> "$OUT/derleme.log" 2>&1 || fin "derleme f0_flows rc=$?"
cargo +1.96.0 build --release --locked -p bench --bin bench >> "$OUT/derleme.log" 2>&1 || fin "derleme bench rc=$?"
export BIN_GOSTER=$CARGO_TARGET_DIR/release/examples/f0_flows BIN_GOSTER_BENCH=$CARGO_TARGET_DIR/release/bench
sha256sum "$BIN_GOSTER" "$BIN_GOSTER_BENCH" scripts/yol3/mini-o/SIRA.txt scripts/yol3/mini-o/ONKAYIT.md >> "$LOG"
echo "KK-ORK-01: FEC kapalı. Kol K1 (gösterim seti, EK-A §2); şeritler + tampon --goster on (W = 5, T = 2,0 s). Paralellik f0_grid -j 11." >> "$LOG"
echo "ölçüm başlangıç $(date -Is)" >> "$LOG"
flock "$HOME/.cache/kama-cargo.lock" systemd-inhibit --what=sleep:idle --why="YOL3 mini-O" \
  scripts/ktp2/f0/f0_grid.sh -j 11 "$OUT" $(cat scripts/yol3/mini-o/SIRA.txt) >> "$OUT/grid.log" 2>&1
echo "grid rc=$? bitiş $(date -Is)" >> "$LOG"
python3 -I scripts/yol3/mini-o/oku.py --kok "$OUT" --onkayit scripts/yol3/mini-o/ONKAYIT.md > "$OUT/oku.log" 2>&1
echo "oku rc=$?" >> "$LOG"
sha256sum "$OUT"/okuma/* >> "$LOG" 2>/dev/null
touch "$OUT/BITTI"
