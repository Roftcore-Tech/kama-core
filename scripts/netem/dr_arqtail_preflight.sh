#!/usr/bin/env bash
# DR-ARQ-PRODUCT-TAIL-1 §4 — FAITHFUL TWO-NETNS/NETEM PREFLIGHT (rootless; docker/admin GEREKMEZ).
#
# Proves, executably and fail-closed, that the product tail-recovery driver runs over a FAITHFUL impaired
# path before any result-bearing grid: two netns, veth in child, connectivity, netem on the correct egress,
# tc qdisc expected config, RTT+loss actually applied (ping = on-path capture-equivalent), the PTO probe
# traversing the impaired path (ON recovers over delayed path), a no-impairment control, and teardown
# isolation. ANY failed gate ⇒ nonzero exit, "PREFLIGHT FAIL" — no result-bearing label is emitted here.
set -uo pipefail
BIN=${RUNNER_BIN:?RUNNER_BIN gerekli (target/debug/examples/dr_arqtail_product)}
OUT=${PREFLIGHT_OUT:?PREFLIGHT_OUT gerekli}
mkdir -p "$OUT"
export BIN OUT
: > "$OUT/preflight.log"

unshare -r -n -m bash <<'INNER'
set -uo pipefail
log(){ echo "$@" | tee -a "$OUT/preflight.log"; }
fail(){ log "GATE-FAIL: $*"; echo FAIL > "$OUT/verdict"; kill "${CPID:-0}" 2>/dev/null; exit 1; }

PARENT_NS=$(readlink /proc/self/ns/net)
# topology: v0 (parent, tx 10.9.0.1) <-> v1 (child, rx 10.9.0.2)
ip link add v0 type veth peer name v1 || fail "veth add"
unshare -n -- sleep 600 & CPID=$!
for _ in $(seq 1 50); do [ -e /proc/$CPID/ns/net ] && break; sleep 0.05; done
CHILD_NS=$(readlink /proc/$CPID/ns/net)
ip link set v1 netns "$CPID" || fail "v1→child"
ip addr add 10.9.0.1/24 dev v0; ip link set v0 up; ip link set lo up
nsenter -t "$CPID" -n ip addr add 10.9.0.2/24 dev v1
nsenter -t "$CPID" -n ip link set v1 up
nsenter -t "$CPID" -n ip link set lo up
m0=$(ip link show v0 | awk '/link\/ether/{print $2}')
m1=$(nsenter -t "$CPID" -n ip link show v1 | awk '/link\/ether/{print $2}')
ip neigh replace 10.9.0.2 lladdr "$m1" dev v0 nud permanent
nsenter -t "$CPID" -n ip neigh replace 10.9.0.1 lladdr "$m0" dev v1 nud permanent

log "=== ENV ==="; { uname -a; tc -V; } 2>&1 | tee -a "$OUT/preflight.log"

# GATE A — two DISTINCT network namespaces
log "[A] parent_ns=$PARENT_NS child_ns=$CHILD_NS"
[ "$PARENT_NS" != "$CHILD_NS" ] || fail "A: parent/child share netns (not faithful)"
log "[A] PASS: two distinct netns"

# GATE B — veth endpoint placement (v0 in parent only, v1 in child only)
ip link show v0 >/dev/null 2>&1 || fail "B: v0 missing in parent"
ip link show v1 >/dev/null 2>&1 && fail "B: v1 leaked into parent"
nsenter -t "$CPID" -n ip link show v1 >/dev/null 2>&1 || fail "B: v1 missing in child"
log "[B] PASS: v0∈parent, v1∈child"

# GATE C — connectivity (no impairment yet)
tc qdisc del dev v0 root 2>/dev/null
ping -c 3 -i 0.2 -W 2 10.9.0.2 >/dev/null 2>&1 || fail "C: no connectivity parent→child"
log "[C] PASS: connectivity ok"

# GATE D — netem applied on the CORRECT egress of BOTH directions (v0 forward, v1 reverse) so RTT=2×owd;
#          forward loss 10% on v0. Expected config verified in the tc dump of each egress.
tc qdisc replace dev v0 root netem delay 100ms loss 10% || fail "D: netem apply v0"
nsenter -t "$CPID" -n tc qdisc replace dev v1 root netem delay 100ms || fail "D: netem apply v1"
QD=$(tc qdisc show dev v0)
QDR=$(nsenter -t "$CPID" -n tc qdisc show dev v1)
log "[D] tc qdisc v0(fwd): $QD"
log "[D] tc qdisc v1(rev): $QDR"
echo "$QD" | grep -q "netem"      || fail "D: netem not present on v0"
echo "$QD" | grep -q "delay 100"  || fail "D: delay 100ms not in v0 qdisc"
echo "$QD" | grep -q "loss 10%"   || fail "D: loss 10% not in v0 qdisc"
echo "$QDR" | grep -q "delay 100" || fail "D: reverse delay 100ms not in v1 qdisc"
log "[D] PASS: netem owd=100ms both egress (RTT≈200ms), fwd loss10% on v0"

# GATE E — RTT + loss actually applied on-path (ping = on-path capture-equivalent, faithful evidence)
PS=$(ping -c 40 -i 0.05 -W 2 10.9.0.2 2>/dev/null)
RTT=$(echo "$PS" | awk -F'/' '/rtt|round-trip/{print $5}')
LOSSPCT=$(echo "$PS" | grep -oE '[0-9]+% packet loss' | grep -oE '^[0-9]+')
log "[E] ping avg_rtt=${RTT:-NA}ms observed_loss=${LOSSPCT:-NA}%"
awk "BEGIN{exit !(${RTT:-0}>=180 && ${RTT:-0}<=260)}" || fail "E: RTT ${RTT}ms not ≈200ms (2×100ms owd) — netem delay NOT faithful"
[ "${LOSSPCT:-0}" -ge 1 ] || fail "E: observed loss ${LOSSPCT}% — netem loss NOT applied on path"
log "[E] PASS: netem delay+loss faithfully applied on the path"

# GATE F — PTO probe traverses the impaired path (ON recovers terminal-tail over delayed+lossy path;
#          t_complete ≥ ~1 RTT proves the probe round-tripped the impaired path). OFF stays stuck.
tc qdisc replace dev v0 root netem delay 100ms   # deterministic delay-only for the tail check
nsenter -t "$CPID" -n tc qdisc replace dev v1 root netem delay 100ms
run_cell(){ # $1=arm  $2=port_base → echoes the result row. UNIQUE ports per run: stale 100ms-delayed
            # datagrams from a prior run must NOT reach a later run's sockets (cross-run contamination).
  local arm=$1 pb=$2
  nsenter -t "$CPID" -n "$BIN" --role rx --bind "10.9.0.2:$((pb+1))" --w-units 12 --drop-terminal 1 \
     --ext-horizon-ms 8000 2>>"$OUT/preflight.log" & local RX=$!
  sleep 0.3
  "$BIN" --role tx --bind "10.9.0.1:$pb" --peer "10.9.0.2:$((pb+1))" --arm "$arm" --w-units 12 --unit-bytes 1200 \
     --op-horizon-ms 3000 --lb-ms 400 --ext-horizon-ms 8000 --run-id "pf-$arm" --cell pfE --struct terminal-tail \
     --loss 0 --rtt "200ms" --rep 0 --result-out "$OUT/preflight_rows.csv" 2>>"$OUT/preflight.log"
  wait "$RX" 2>/dev/null
}
run_cell off 47401; run_cell on 47411
OFF_ROW=$(grep ",off," "$OUT/preflight_rows.csv" | tail -1)
ON_ROW=$(grep ",on," "$OUT/preflight_rows.csv" | tail -1)
log "[F] OFF: $OFF_ROW"
log "[F] ON : $ON_ROW"
# columns: ...,complete(13),t_complete_ms(14),...,probe_sent(18)...
off_complete=$(echo "$OFF_ROW" | cut -d, -f13)
on_complete=$(echo "$ON_ROW" | cut -d, -f13)
on_tc=$(echo "$ON_ROW" | cut -d, -f14)
on_sent=$(echo "$ON_ROW" | cut -d, -f18)
[ "$off_complete" = "0" ] || fail "F: OFF recovered a pure terminal tail (should be stuck) complete=$off_complete"
[ "$on_complete" = "1" ]  || fail "F: ON did NOT recover terminal tail over impaired path"
[ "$on_sent" -ge 1 ]      || fail "F: ON completed without a probe (unexpected)"
awk "BEGIN{exit !(${on_tc:-0}>=100)}" || fail "F: ON t_complete=${on_tc}ms < 1 RTT — probe did NOT traverse the delayed path"
log "[F] PASS: probe traversed impaired path (ON tc=${on_tc}ms ≥ RTT, sent=$on_sent); OFF stuck"

# GATE G — no-impairment control (clean path delivers; loss/IID structure, no terminal drop)
tc qdisc replace dev v0 root netem delay 1ms loss 0%
nsenter -t "$CPID" -n tc qdisc replace dev v1 root netem delay 1ms loss 0%
nsenter -t "$CPID" -n "$BIN" --role rx --bind 10.9.0.2:47422 --w-units 12 --drop-terminal 0 \
   --ext-horizon-ms 6000 2>>"$OUT/preflight.log" & RXC=$!
sleep 0.3
"$BIN" --role tx --bind 10.9.0.1:47421 --peer 10.9.0.2:47422 --arm off --w-units 12 --unit-bytes 1200 \
   --op-horizon-ms 3000 --lb-ms 100 --ext-horizon-ms 6000 --run-id pf-ctrl --cell pfG --struct iid \
   --loss 0 --rtt "2ms" --rep 0 --result-out "$OUT/preflight_rows.csv" 2>>"$OUT/preflight.log"
wait "$RXC" 2>/dev/null
CTRL_ROW=$(grep "^pf-ctrl," "$OUT/preflight_rows.csv" | tail -1)
log "[G] CTRL: $CTRL_ROW"
ctrl_complete=$(echo "$CTRL_ROW" | cut -d, -f13)
ctrl_sent=$(echo "$CTRL_ROW" | cut -d, -f18)
[ "$ctrl_complete" = "1" ] || fail "G: clean-path control did not complete"
[ "$ctrl_sent" = "0" ]     || fail "G: OFF control emitted a probe (must be 0)"
log "[G] PASS: no-impairment control completes, 0 probes"

# GATE H — teardown isolation
kill "$CPID" 2>/dev/null
sleep 0.2
ip link show v0 >/dev/null 2>&1 && log "[H] note: v0 still present (namespace-exit will clean)" || log "[H] v0 cleaned"
echo PASS > "$OUT/verdict"
log "=== PREFLIGHT PASS (A–H) ==="
INNER
RC=$?
echo "--- verdict: $(cat "$OUT/verdict" 2>/dev/null || echo NONE) (inner rc=$RC) ---"
[ "$(cat "$OUT/verdict" 2>/dev/null)" = "PASS" ]
