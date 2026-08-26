#!/usr/bin/env bash
# [P1 kapı-2] Süreç-düzeyi e2e: iki-netns + netem (kayıp+gecikme) altında p1_endpoint client/server.
# Faithful-netem kuralı: netem ORTA ns'te (kapi3 topolojisi; tek-ns veth netem'i sessizce yutar).
# Kriter: client N birimin TAMAMININ ACK'ini, server TAMAMININ teslimini görür (exit 0+0).
# Kullanım: p1_e2e_gate.sh [UNITS=100] [LOSS=5%] [DELAY=20ms] [DEADLINE_S=90]
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
UNITS="${1:-100}"; LOSS="${2:-5%}"; DELAY="${3:-20ms}"; DEADLINE="${4:-90}"
BIN="$REPO/target/release/examples/p1_endpoint"
[ -x "$BIN" ] || { echo "önce: cargo build --release -p client-core --example p1_endpoint"; exit 3; }

if [ "${1:-}" = "--inner" ]; then shift; fi
if [ -z "${P1_INNER:-}" ]; then
  P1_INNER=1 exec unshare -rnm bash "$0" "$UNITS" "$LOSS" "$DELAY" "$DEADLINE"
fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
kapi3_netem "delay $DELAY loss $LOSS" "delay $DELAY loss $LOSS"

nsenter -t "$CHLB" -n "$BIN" server --bind 10.10.1.1:4600 --expect "$UNITS" --deadline-s "$DEADLINE" 2>server.err & SRV=$!
sleep 1
"$BIN" client --server 10.10.1.1:4600 --units "$UNITS" --interval-ms 50 --deadline-s "$DEADLINE" 2>client.err; CRC=$?
wait "$SRV"; SRC=$?
cat client.err server.err
if [ "$CRC" -eq 0 ] && [ "$SRC" -eq 0 ]; then echo "P1-GATE: GREEN (units=$UNITS loss=$LOSS delay=$DELAY)"; exit 0
else echo "P1-GATE: RED (client=$CRC server=$SRC)"; exit 1; fi
