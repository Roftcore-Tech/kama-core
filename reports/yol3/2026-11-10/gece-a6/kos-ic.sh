#!/usr/bin/env bash
# YOL3 gece ölçümü (sahip kararı v4 EK-A §9.3, §10.3, §10.4, §2.1): 7 kol × 3 hücre + q_limit + R3 (K0/K1/K1A6), N = 10.
# Orkestratör (D-18, D-34). kos.sh üç gece yuvasını tutarak çağırır (ölçüm sessizliği). Ayrık süreç.
set -u
R=<worktree-kökü>/yol2-goster-a6
Q=<worktree-kökü>/yol3-quinn-kol
OUT=<ölçüm-kökü>/yol3/tesis/2026-11-10-a6
LOG=$OUT/KOSU-KAYDI.txt
fin() { echo "$1" >> "$LOG"; touch "$OUT/BITTI"; exit "${2:-1}"; }
echo "yuvalar alındı (3/3) $(date -Is)" >> "$LOG"
cd "$R" || fin "worktree yok" 2
[ -z "$(git status --porcelain)" ] || fin "REDDEDİLDİ: yol2-goster-a6 kirli" 2
[ -z "$(git -C "$Q" status --porcelain)" ] || fin "REDDEDİLDİ: quinn-kol kirli" 2
echo "ağaç: yol2/goster-a6 $(git rev-parse HEAD) (= yol2/goster-a 56313c73 + Q2 A-6 e9992229 + ENT-A6 düzenek)" >> "$LOG"
echo "quinn ağacı: yol3/quinn-kol $(git -C "$Q" rev-parse HEAD)" >> "$LOG"
echo "diğer cargo/rustc süreçleri: $(pgrep -c -x cargo || true)/$(pgrep -c -x rustc || true)" >> "$LOG"
export CARGO_TARGET_DIR=<worktree-kökü>/target-goster-a6 CARGO_BUILD_JOBS=10
cargo +1.96.0 build --release --locked -p kama-integration --example f0_flows >> "$OUT/derleme.log" 2>&1 || fin "derleme f0_flows rc=$?"
cargo +1.96.0 build --release --locked -p bench --bin bench >> "$OUT/derleme.log" 2>&1 || fin "derleme bench rc=$?"
[ "$(git -C "$Q" rev-parse HEAD)" = d93aef104b01f7363935e859de0f89c389f909ab ] || fin "quinn-kol beklenen d93aef10 değil" 2
export BIN_TESIS=$CARGO_TARGET_DIR/release/examples/f0_flows BIN_TESIS_BENCH=$CARGO_TARGET_DIR/release/bench BIN_QK=<worktree-kökü>/target-quinn-kol/release/bench
sha256sum "$BIN_TESIS" "$BIN_TESIS_BENCH" "$BIN_QK" scripts/yol3/tesis/SIRA-A6.txt >> "$LOG"
echo "BIN_QK önceki quinn koşusunun ikilisi (beklenen 26a702dc…; d93aef10'dan yeniden derlenmedi)" >> "$LOG"
echo "KK-ORK-01: FEC kapalı. s-kolları --goster on (W = 5, T = 2,0 s). q_limit kolları netem limit (hücre başlığında hesap). Paralellik f0_grid -j 11 (önceki Kama koşularıyla aynı; önceki quinn koşusu -j 3 idi)." >> "$LOG"
echo "ölçüm başlangıç $(date -Is)" >> "$LOG"
flock "$HOME/.cache/kama-cargo.lock" systemd-inhibit --what=sleep:idle --why="YOL3 gece a6" \
  scripts/ktp2/f0/f0_grid.sh -j 11 "$OUT" $(cat scripts/yol3/tesis/SIRA-A6.txt) >> "$OUT/grid.log" 2>&1
echo "grid rc=$? bitiş $(date -Is)" >> "$LOG"
python3 -I scripts/yol3/tesis/oku_a6.py --kok "$OUT" > "$OUT/oku.log" 2>&1
echo "oku rc=$?" >> "$LOG"
sha256sum "$OUT"/okuma-a6/* >> "$LOG" 2>/dev/null
touch "$OUT/BITTI"
