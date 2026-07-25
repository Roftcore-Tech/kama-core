#!/usr/bin/env bash
# DR-CS-1 F4-CLOSURE wire-hücreleri (faithful two-netns; static-ARP; acceptance-6/6-önkoşul).
# DI-2B (reverse-blackout) + W-3 (blackout-heal 3-boyut) + W-1-rerun (OFF-vs-ON 3-boyut). Provenance-emit.
# BIN_CS = cold-start-runner; BIN_OFF = flag-OFF-baseline.
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
BIN_CS=${BIN_CS:?}; BIN_OFF=${BIN_OFF:?}; OUT=${WIRE_OUT:?}; mkdir -p "$OUT"
export LIB BIN_CS BIN_OFF OUT
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"; faithful_setup; trap faithful_teardown EXIT
# PROVENANCE + acceptance-6/6-önkoşul (mini: RTT100-pin; tam-6/6 ayrı-acceptance-script'te).
{
  echo "git_commit=$(git -C /home/ryuzaki/Desktop/kama-core rev-parse --short HEAD)"
  echo "dirty=$(git -C /home/ryuzaki/Desktop/kama-core status --porcelain | wc -l)"
  echo "kernel=$(uname -r)"; echo "tc=$(tc -V 2>&1)"
  echo "netns=parent:$(readlink /proc/self/ns/net) child:$(readlink /proc/$CHILD/ns/net)"
  faithful_netem "delay 50ms" "delay 50ms"
  ping -c1 -W2 10.9.0.2 >/dev/null 2>&1
  m=$(ping -c3 -W3 -i0.3 10.9.0.2 2>&1 | grep min/avg | grep -oE "= [0-9.]+/" | grep -oE "[0-9.]+" | head -1)
  echo "acceptance_rtt100_min_ms=${m%.*}"
} > "$OUT/PROVENANCE.txt"

# KRİTİK: rx arka-plan-işinin STDOUT'u da yönlendirilmeli. Aksi hâlde `RX=$(runrx ...)` komut-ikamesi
# arka-plan-rx'in stdout-fd'sini (ikame-borusu) tutar → ikame rx-ÇIKANA-KADAR BLOKLAR (66s) → tx ancak
# rx-öldükten-SONRA başlar → rx/tx HİÇ örtüşmez → receiver_events=0 (tüm-wire-hücreleri-geçersiz-olurdu).
runrx() { faithful_rx "$BIN_CS" --role rx --bind 10.9.0.2:47002 --peer 10.9.0.1:47001 "$@" >/dev/null 2>&1 & echo $!; }

# ── DI-2B: reverse-blackout (forward-open) → probe-reach + echo-lost + TimedOut ──
for sz in 65 300 1200; do
  faithful_netem "delay 50ms" "delay 50ms loss 100%"
  RX=$(runrx --run-id di2b-$sz --output-dir "$OUT" --operational-horizon 2000 --extended-horizon 66000); sleep 0.3
  timeout 75 "$BIN_CS" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
    --run-id di2b-$sz --output-dir "$OUT" --operational-horizon 2000 --extended-horizon 63000 --w-units 8 \
    --unit-bytes $sz --tx-settle-ms 3000 2>/dev/null
  wait $RX 2>/dev/null; echo "di2b-$sz done" >> "$OUT/RUNLOG"
done

# ── W-3: çift-yön-blackout t0..restore → heal. restore=20s (giveup-öncesi) + restore=61s (giveup-sonrası) ──
for sz in 65 300 1200; do
  for restore in 20 61; do
    faithful_netem "delay 50ms" "delay 50ms"
    RX=$(runrx --run-id w3-$sz-$restore --output-dir "$OUT" --deficit 1 --deficit-from-block 0 \
      --operational-horizon 2000 --extended-horizon 120000); sleep 0.3
    # blackout: t0..restore çift-yön %100 loss → restore'da temizle.
    ( sleep 0.5; faithful_netem "delay 50ms loss 100%" "delay 50ms loss 100%"
      sleep "$restore"; faithful_netem "delay 50ms" "delay 50ms" ) & BK=$!
    timeout 118 "$BIN_CS" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
      --run-id w3-$sz-$restore --output-dir "$OUT" --operational-horizon 2000 --extended-horizon 110000 \
      --w-units 8 --unit-bytes $sz --tx-settle-ms 4000 2>/dev/null
    wait $BK 2>/dev/null; wait $RX 2>/dev/null; echo "w3-$sz-$restore done" >> "$OUT/RUNLOG"
  done
done

# ── W-1-rerun: cold-start first-source-loss (RTT100) OFF-vs-ON × 65/300/1200B ──
for sz in 65 300 1200; do
  # ON
  faithful_netem "delay 50ms" "delay 50ms"
  RX=$(runrx --run-id w1on-$sz --output-dir "$OUT" --deficit 1 --deficit-from-block 0 \
    --operational-horizon 2000 --extended-horizon 120000); sleep 0.3
  timeout 100 "$BIN_CS" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
    --run-id w1on-$sz --output-dir "$OUT" --operational-horizon 2000 --extended-horizon 90000 --w-units 16 \
    --unit-bytes $sz --tx-settle-ms 8000 2>/dev/null
  wait $RX 2>/dev/null; echo "w1on-$sz done" >> "$OUT/RUNLOG"
  # OFF (baseline; flag-OFF binary; ext 90s → block-0 tamamlanmaz beklenir)
  faithful_netem "delay 50ms" "delay 50ms"
  faithful_rx "$BIN_OFF" --role rx --bind 10.9.0.2:47002 --peer 10.9.0.1:47001 --run-id w1off-$sz \
    --output-dir "$OUT" --deficit 1 --deficit-from-block 0 --operational-horizon 2000 --extended-horizon 92000 2>/dev/null & RX=$!
  sleep 0.3
  timeout 100 "$BIN_OFF" --role tx --bind 10.9.0.1:47001 --peer 10.9.0.2:47002 --policy leftover --fixed-r 0 \
    --run-id w1off-$sz --output-dir "$OUT" --operational-horizon 2000 --extended-horizon 90000 --w-units 16 \
    --unit-bytes $sz --tx-settle-ms 8000 2>/dev/null
  wait $RX 2>/dev/null; echo "w1off-$sz done" >> "$OUT/RUNLOG"
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
