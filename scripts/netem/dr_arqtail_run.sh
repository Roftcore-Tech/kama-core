#!/usr/bin/env bash
# DR-ARQ-PRODUCT-TAIL-1 §5/§6 — GRID EXECUTOR (rootless two-netns/netem; serial, resource-safe).
#
# Consumes a frozen manifest (dr_arqtail_manifest.py) and runs each result-bearing row over the FAITHFUL
# two-netns path: netem_fwd on v0 (parent egress → child), netem_rev on v1 (child egress → parent),
# fail-closed netem verification, unique ports per run (no stale-packet contamination), dynamic mid-run
# mutations for adversarial slices. Appends the driver's §7 result row per run to $RESULT.
#
# Env: RUNNER_BIN, MANIFEST, RESULT_OUT, [LIMIT=N] (first N manifest rows), [SUBSET=grep-pattern].
set -uo pipefail
BIN=${RUNNER_BIN:?}; MANIFEST=${MANIFEST:?}; RESULT=${RESULT_OUT:?}
LIMIT=${LIMIT:-0}; SUBSET=${SUBSET:-}; SHARD=${SHARD:-0}; NSHARD=${NSHARD:-1}
mkdir -p "$(dirname "$RESULT")"; : > "$RESULT.log"
export BIN MANIFEST RESULT LIMIT SUBSET SHARD NSHARD
# stagger parallel workers so simultaneous veth creation doesn't race (SHARD·0.4s offset)
sleep "$(awk "BEGIN{print ${SHARD:-0}*0.4}")" 2>/dev/null || true
unshare -r -n -m bash <<'INNER'
set -uo pipefail
CPID=""
setup_ns() { # verified, idempotent — returns 0 only when the faithful path is fully usable
  [ -n "$CPID" ] && kill "$CPID" 2>/dev/null; ip link del v0 2>/dev/null
  ip link add v0 type veth peer name v1 2>/dev/null || return 1
  unshare -n -- sleep 100000 & CPID=$!
  for _ in $(seq 1 100); do [ -e /proc/$CPID/ns/net ] && break; sleep 0.05; done
  [ -e /proc/$CPID/ns/net ] || return 1
  ip link set v1 netns "$CPID" 2>/dev/null || return 1
  ip addr add 10.9.0.1/24 dev v0 2>/dev/null; ip link set v0 up 2>/dev/null; ip link set lo up 2>/dev/null
  nsenter -t "$CPID" -n ip addr add 10.9.0.2/24 dev v1 2>/dev/null || return 1
  nsenter -t "$CPID" -n ip link set v1 up 2>/dev/null; nsenter -t "$CPID" -n ip link set lo up 2>/dev/null
  local m0 m1; m0=$(ip link show v0 2>/dev/null|awk '/link\/ether/{print $2}')
  m1=$(nsenter -t "$CPID" -n ip link show v1 2>/dev/null|awk '/link\/ether/{print $2}')
  [ -n "$m0" ] && [ -n "$m1" ] || return 1
  ip neigh replace 10.9.0.2 lladdr "$m1" dev v0 nud permanent 2>/dev/null
  nsenter -t "$CPID" -n ip neigh replace 10.9.0.1 lladdr "$m0" dev v1 nud permanent 2>/dev/null
  tc qdisc replace dev v0 root netem delay 5ms 2>/dev/null || return 1
  nsenter -t "$CPID" -n tc qdisc replace dev v1 root netem delay 5ms 2>/dev/null || return 1
  ping -c 1 -W 2 10.9.0.2 >/dev/null 2>&1 || return 1
  return 0
}
ok=0
for attempt in $(seq 1 10); do setup_ns && { ok=1; break; }; echo "setup-retry $attempt" >>"$RESULT.log"; sleep 0.5; done
[ "$ok" = 1 ] || { echo "SETUP-FAILED" >>"$RESULT.log"; exit 1; }
{ uname -a; tc -V; } >> "$RESULT.log" 2>&1

rows=$(tail -n +2 "$MANIFEST"); [ -n "$SUBSET" ] && rows=$(echo "$rows" | grep -E "$SUBSET")
[ "$LIMIT" -gt 0 ] && rows=$(echo "$rows" | head -n "$LIMIT")
# shard: this worker takes rows where (line-number % NSHARD == SHARD). Each worker has its OWN netns → no conflict.
[ "$NSHARD" -gt 1 ] && rows=$(echo "$rows" | awk -v s="$SHARD" -v n="$NSHARD" 'NR % n == s')
total=$(echo "$rows" | grep -c .)
echo "EXECUTOR total=$total (LIMIT=$LIMIT SUBSET='$SUBSET')" >> "$RESULT.log"
port=40000; n=0
while IFS=, read -r run_id group struct loss rtt arm rep owd op lb ext w ub fwd rev dt dyn; do
  [ -z "$run_id" ] && continue
  n=$((n+1)); port=$((port+3)); [ $port -gt 60000 ] && port=40000
  tp=$port; rp=$((port+1))
  applied=0
  for na in 1 2 3 4; do
    if tc qdisc replace dev v0 root netem $fwd 2>>"$RESULT.log" \
       && nsenter -t "$CPID" -n tc qdisc replace dev v1 root netem $rev 2>>"$RESULT.log" \
       && tc qdisc show dev v0 | grep -q netem; then applied=1; break; fi
    echo "NETEM-RETRY $run_id attempt=$na ($fwd)" >>"$RESULT.log"
    setup_ns >/dev/null 2>&1 || true   # recover a transiently-broken namespace
    sleep 0.3
  done
  [ "$applied" = 1 ] || { echo "NETEM-SKIP $run_id (after retries)" >>"$RESULT.log"; continue; }
  # rx must outlive warmup(≤ max(ext/2,2000)) + measured(ext); it exits early on the tx FIN.
  rx_ext=$(( ext*2 + 4000 ))
  nsenter -t "$CPID" -n "$BIN" --role rx --bind 10.9.0.2:$rp --w-units $w --unit-bytes $ub \
     --drop-terminal $dt --ext-horizon-ms $rx_ext 2>>"$RESULT.log" & RX=$!
  sleep 0.2
  BK=""
  case "$dyn" in
    pause)    ( sleep 0.6; tc qdisc change dev v0 root netem delay ${owd}ms rate 1mbit loss 100% 2>/dev/null; \
                sleep 0.8;  tc qdisc change dev v0 root netem $fwd 2>/dev/null ) & BK=$! ;;
    stepup)   ( sleep 1.0; tc qdisc change dev v0 root netem delay 500ms rate 1mbit 2>/dev/null; \
                nsenter -t "$CPID" -n tc qdisc change dev v1 root netem delay 500ms rate 1mbit 2>/dev/null ) & BK=$! ;;
    stepdown) ( sleep 1.0; tc qdisc change dev v0 root netem delay 25ms rate 1mbit 2>/dev/null; \
                nsenter -t "$CPID" -n tc qdisc change dev v1 root netem delay 25ms rate 1mbit 2>/dev/null ) & BK=$! ;;
  esac
  "$BIN" --role tx --bind 10.9.0.1:$tp --peer 10.9.0.2:$rp --arm "$arm" --w-units $w --unit-bytes $ub \
     --op-horizon-ms $op --lb-ms $lb --ext-horizon-ms $ext --run-id "$run_id" --cell "$struct/$loss/$rtt" \
     --struct "$struct" --loss "$loss" --rtt "$rtt" --rep "$rep" --result-out "$RESULT" 2>>"$RESULT.log"
  [ -n "$BK" ] && { wait "$BK" 2>/dev/null; }
  wait "$RX" 2>/dev/null
  [ $((n % 25)) -eq 0 ] && echo "PROGRESS $n/$total ($run_id)" >> "$RESULT.log"
done <<< "$rows"
kill "$CPID" 2>/dev/null
echo "GRID-DONE rows_run=$n" >> "$RESULT.log"
INNER
echo "executor rc=$? ; result rows: $(($(wc -l < "$RESULT" 2>/dev/null || echo 1)-1)) ; tail log:"
tail -3 "$RESULT.log"
