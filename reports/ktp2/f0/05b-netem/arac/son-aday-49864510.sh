#!/usr/bin/env bash
# Son aday 49864510: (1) tam yerel kapı (donmuş wt/05b-son) → (2) aday bench → (3) netem-7 (aynı ön-kayıt) → analiz
set -u
SHA=49864510
WT=/home/ryuzaki/kama-k4g-work/wt/05b-son
G=/home/ryuzaki/kama-k4g-work/k4g05b/gate-$SHA
N7=/home/ryuzaki/kama-k4g-work/k4g05b/netem7/run-$SHA
mkdir -p $G $N7
git -C $WT checkout -q -f --detach $SHA
[ "$(git -C $WT rev-parse --short=8 HEAD)" = "$SHA" ] && [ "$(git -C $WT status --porcelain | wc -l)" = 0 ] || { echo "worktree $SHA değil ya da kirli" > $G/HATA; exit 2; }
cd $WT
{ rustc --version; cargo --version; rustup show active-toolchain; git rev-parse HEAD; git status --porcelain | wc -l; } > $G/toolchain.txt 2>&1
CARGO_TARGET_DIR=/home/ryuzaki/kama-k4g-work/targets/05b-son /home/ryuzaki/kama-k4g-work/tools/k4g-gate.sh $WT $G --vectors --integration -- \
  "cargo metadata --locked --format-version 1 >/dev/null && cargo check --workspace --all-targets --all-features --locked" \
  "systemd-analyze verify deploy/kama-hub.service"
{ echo "Cargo.lock değişmedi mi: $(git -C $WT diff --quiet -- Cargo.lock && echo EVET || echo HAYIR)"; echo "ağaç temiz mi: $(git -C $WT status --porcelain | wc -l) değişiklik"; echo "HEAD sonra: $(git -C $WT rev-parse HEAD)"; } >> $G/summary.txt
# (2) aday bench
( cd $WT && env -u CARGO_TARGET_DIR CARGO_BUILD_JOBS=2 cargo build --release --locked -p bench --bin bench ) > $N7/build-aday.log 2>&1; echo "build rc=$?" >> $N7/build-aday.log
[ -x $WT/target/release/bench ] || { echo "aday bench yok" > $N7/HATA; exit 3; }
# (3) netem-7 — ön-kayıt ONKAYIT.md 6d667853… (aynı kural, yeni aday)
B0=/home/ryuzaki/kama-k4g-work/wt/netem-base/target/release/bench
B1=$WT/target/release/bench
H=/home/ryuzaki/kama-k4g-work/wt/netem-base/scripts/netem
sha256sum $B0 $B1 > $N7/ikili-sha.txt
sha256sum /home/ryuzaki/kama-k4g-work/k4g05b/netem7/ONKAYIT.md /home/ryuzaki/kama-k4g-work/tools/netem7_cell.sh /home/ryuzaki/kama-k4g-work/tools/netem7_analyze.py > $N7/onkayit-hash-kosu-ani.txt
date -u +"başlangıç %FT%TZ" > $N7/driver.log
running=0
for p in p6_blackout p1_loss50 p2_loss80 p3_loss95 p4_jitter p5_revpath p7_bbr; do
  while [ "$running" -ge 3 ]; do wait -n 2>/dev/null || true; running=$((running-1)); done
  ( timeout 9000 bash /home/ryuzaki/kama-k4g-work/tools/netem7_cell.sh $p $N7 20 $H $B0 $B1 > $N7/$p.log 2>&1; rc=$?; [ $rc -ne 0 ] && echo "SHARD-FAIL $p rc=$rc" >> $N7/driver.log ) &
  running=$((running+1))
done
wait
date -u +"bitiş %FT%TZ" >> $N7/driver.log
python3 -I /home/ryuzaki/kama-k4g-work/tools/netem7_analyze.py $N7 > $N7/ANALIZ.txt 2>&1
python3 -I /home/ryuzaki/kama-k4g-work/tools/netem7_ci.py $N7 $N7/../kosular-$SHA.csv > $N7/../betimsel-ci-$SHA.md 2>&1
echo BİTTİ >> $N7/driver.log
