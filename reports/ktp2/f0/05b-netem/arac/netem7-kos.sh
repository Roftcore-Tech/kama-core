#!/usr/bin/env bash
# netem-7 ön-kayıtlı koşu: aday bench @a8c4e8c6 (donmuş worktree) ↔ taban 1b1425d2; N=20/kol; ≤3 shard
set -u
SHA=a8c4e8c6
WT=/home/ryuzaki/kama-k4g-work/wt/05b-son
N7=/home/ryuzaki/kama-k4g-work/k4g05b/netem7/run-$SHA
mkdir -p $N7
[ "$(git -C $WT rev-parse --short=8 HEAD)" = "$SHA" ] && [ "$(git -C $WT status --porcelain | wc -l)" = 0 ] || { echo "worktree $SHA değil ya da kirli" > $N7/HATA; exit 2; }
( cd $WT && env -u CARGO_TARGET_DIR CARGO_BUILD_JOBS=2 cargo build --release --locked -p bench --bin bench ) > $N7/build-aday.log 2>&1; echo "build rc=$?" >> $N7/build-aday.log
[ -x $WT/target/release/bench ] || { echo "aday bench yok" > $N7/HATA; exit 3; }
B0=/home/ryuzaki/kama-k4g-work/wt/netem-base/target/release/bench
B1=$WT/target/release/bench
sha256sum $B0 $B1 > $N7/ikili-sha.txt
H=/home/ryuzaki/kama-k4g-work/wt/netem-base/scripts/netem
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
echo BİTTİ >> $N7/driver.log
