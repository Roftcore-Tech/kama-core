#!/usr/bin/env bash
# DR-CS-1 F4 W-2 — EXACT-ONCE ACK-DROP faithful (self-heal via cumulative-SACK; RANDOM-LOSS YASAK).
# Çift-yön AÇIK (yalnız delay). rx --drop-first-sack ile İLK non-boş-SACK'i exact-once-suppress eder;
# sonraki 50ms-cadence-SACK re-ack'ler → PTO-öncesi bootstrap (spurious-retransmit ölçülür, iddia-edilmez).
# RTT{100,250,1000} × size{65,300,1200} × 3-rep. Provenance + acceptance-RTT-pin. Tek-orkestratör (concurrency-yok).
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
BIN_CS=${BIN_CS:?}; OUT=${WIRE_OUT:?}; mkdir -p "$OUT"
export LIB BIN_CS OUT
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"; faithful_setup; trap faithful_teardown EXIT
# PROVENANCE + acceptance (RTT100-pin).
{
  echo "git_commit=$(git -C /home/ryuzaki/Desktop/kama-core rev-parse --short HEAD)"
  echo "dirty=$(git -C /home/ryuzaki/Desktop/kama-core status --porcelain | wc -l)"
  echo "kernel=$(uname -r)"; echo "tc=$(tc -V 2>&1)"
  echo "netns=parent:$(readlink /proc/self/ns/net) child:$(readlink /proc/$CHILD/ns/net)"
  faithful_netem "delay 50ms" "delay 50ms"
  ping -c1 -W2 10.9.0.2 >/dev/null 2>&1
  m=$(ping -c3 -W3 -i0.3 10.9.0.2 2>&1 | grep min/avg | grep -oE "= [0-9.]+/" | grep -oE "[0-9.]+" | head -1)
  echo "acceptance_rtt100_min_ms=${m%.*}"
  echo "scenario=W2-exact-once-ack-drop; loss_model=EXACT-ONCE-first-SACK-suppress (RANDOM-LOSS-YOK)"
} > "$OUT/W2-PROVENANCE.txt"

# rx STDOUT-yönlendirmeli (komut-ikame-blok-bug'ı önle; bkz run_dr_cs1_wire.sh runrx notu).
runrx() { faithful_rx "$BIN_CS" --role rx --bind 10.9.0.2:47002 --peer 10.9.0.1:47001 "$@" >/dev/null 2>&1 & echo $!; }

for rtt in 100 250 1000; do
  half=$((rtt/2))
  for sz in 65 300 1200; do
    for rep in 1 2 3; do
      id="w2-${rtt}-${sz}-r${rep}"
      faithful_netem "delay ${half}ms" "delay ${half}ms"
      RX=$(runrx --run-id "$id" --output-dir "$OUT" --drop-first-sack \
        --operational-horizon 4000 --extended-horizon 40000); sleep 0.3
      timeout 45 "$BIN_CS" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
        --run-id "$id" --output-dir "$OUT" --operational-horizon 4000 --extended-horizon 30000 \
        --w-units 8 --unit-bytes "$sz" --tx-settle-ms $((rtt*6)) >/dev/null 2>&1
      wait $RX 2>/dev/null; echo "$id done" >> "$OUT/W2-RUNLOG"
    done
  done
done
echo "W2-ALL-DONE" >> "$OUT/W2-RUNLOG"
INNER
