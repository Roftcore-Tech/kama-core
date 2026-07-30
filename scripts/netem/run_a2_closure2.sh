#!/usr/bin/env bash
# CLOSURE-2 — A2 cold-start receiver resource-boundedness under 7 hostile netem profiles (rootless).
# Freeze: reports/decision-requests/A2-CLOSURE-2-pre-registration.md v2 (sha256=d7b9aea0).
# ROOTLESS-USERNS (admin/docker GEREKMEZ). FAITHFUL-NETEM: veth-peer → CHILD-PID netns (tek-ns-veth
# netem'i SESSİZCE-YOK-SAYAR — faithful-netem-harness dersi; ping-doğrulandı delay 200ms→200.27ms).
# netem = v0 (parent egress → child) faithfully applied. Frozen netem-PARAMLARI değişmez; yalnız plumbing.
set -euo pipefail
BIN=${RUNNER_BIN:?RUNNER_BIN}
OUT=${NETEM_OUT:?NETEM_OUT}
mkdir -p "$OUT"
echo "label,run_id,baseline_rss,peak_rss,max_retained,retained_last,retained_prev1s,post_clean_rss,post_clean_retained,rejects,peak_sessions,peak_tombstones,cs_echo,delivered,n_samples" > "$OUT/summary.csv"
export BIN OUT
unshare -r -n -m bash <<'INNER'
set -euo pipefail
# veth pair; peer v1 → child netns (FAITHFUL netem crossing).
ip link add v0 type veth peer name v1
unshare -n -- sleep 600 & CPID=$!
sleep 0.5
ip link set v1 netns "$CPID"
ip addr add 10.99.0.1/24 dev v0; ip link set v0 up; ip link set lo up
nsenter -t "$CPID" -n ip addr add 10.99.0.2/24 dev v1
nsenter -t "$CPID" -n ip link set v1 up
nsenter -t "$CPID" -n ip link set lo up
{ uname -a; tc -V; echo "date=$(date -u +%FT%TZ)";
  echo "faithful-check(delay 200ms):"; } > "$OUT/ENV.txt" 2>&1 || true
tc qdisc replace dev v0 root netem delay 200ms
ping -c 2 -i 0.3 -W 2 10.99.0.2 2>&1 | tail -2 >> "$OUT/ENV.txt" || true
tc qdisc del dev v0 root 2>/dev/null || true

declare -A NETEM=(
  [p1_lowbw]="rate 30kbit"
  [p2_highrtt]="delay 500ms"
  [p3_loss]="loss 30%"
  [p4_jitter]="delay 200ms 100ms distribution normal"
  [p5_burstloss]="loss gemodel 20% 60%"
  [p6_intermittency]="delay 50ms"
  [p7_combined]="delay 300ms 80ms distribution normal rate 64kbit loss gemodel 15% 50%"
)
PROFILES="p1_lowbw p2_highrtt p3_loss p4_jitter p5_burstloss p6_intermittency p7_combined"
REPS=3; DUR_TX=6000; DUR_RX=6800

for prof in $PROFILES; do
  for rep in $(seq 1 $REPS); do
    rid="$prof-r$rep"
    tc qdisc replace dev v0 root netem ${NETEM[$prof]}
    # rx (product-path ReceiverHost + RSS sampling) in CHILD netns
    nsenter -t "$CPID" -n "$BIN" --role rx --bind 10.99.0.2:47202 --duration-ms $DUR_RX --sessions 8 \
       --label "$prof" --run-id "$rid" --out "$OUT/summary.csv" 2>>"$OUT/RUNLOG" &
    RX=$!
    sleep 0.3
    BK=""
    if [[ $prof == p6_* ]]; then
      ( for _ in 1 2; do
          sleep 1; tc qdisc replace dev v0 root netem loss 100%; sleep 2
          tc qdisc replace dev v0 root netem delay 50ms; sleep 3
        done ) & BK=$!
    fi
    # tx seal-flood in PARENT netns (egress v0 → netem → child)
    timeout $((DUR_TX/1000 + 8)) "$BIN" --role tx --bind 10.99.0.1:47201 --peer 10.99.0.2:47202 \
       --duration-ms $DUR_TX --sessions 8 2>>"$OUT/RUNLOG" || echo "TXFAIL $rid" >>"$OUT/RUNLOG"
    [[ -n $BK ]] && { wait "$BK" 2>/dev/null || true; }
    wait "$RX" 2>/dev/null || echo "RXFAIL $rid" >>"$OUT/RUNLOG"
    echo "$rid done" >>"$OUT/RUNLOG"
  done
  echo "PROFIL-TAMAM $prof" >>"$OUT/RUNLOG"
done
kill "$CPID" 2>/dev/null || true
echo "ALL-DONE" >>"$OUT/RUNLOG"
INNER
echo "=== ENV faithful-check ==="; cat "$OUT/ENV.txt"
echo "=== summary.csv ==="; cat "$OUT/summary.csv"
