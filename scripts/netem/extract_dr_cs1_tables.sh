#!/usr/bin/env bash
# DR-CS-1 F4 wire-cell tablo-çıkarıcı. Marker/cstx dosyalarından DI-2B/W-3/W-1 satırlarını üretir.
# Kullanım: extract_dr_cs1_tables.sh <DATA_DIR>
set -uo pipefail
D="${1:?DATA_DIR gerekli}"
g() { grep -oE "^$2=.*" "$1" 2>/dev/null | head -1 | cut -d= -f2- ; }   # g FILE KEY

echo "### PROVENANCE"
cat "$D/PROVENANCE.txt" 2>/dev/null
echo

echo "### DI-2B (reverse-blackout; sender-terminal + rx probe-reach)"
echo "size | probe_emit | terminal | rx_events | cs_probe_rx | cs_echo_tx | cs_echo_bytes"
for sz in 65 300 1200; do
  c="$D/di2b-$sz-cstx.txt"; r="$D/di2b-$sz-rxdone.marker"
  echo "$sz | $(g "$c" cs_probe_emitted) | $(g "$c" cs_terminals) | $(g "$r" receiver_events) | $(g "$r" cs_probe_rx) | $(g "$r" cs_echo_tx) | $(g "$r" cs_echo_tx_bytes)"
done
echo

echo "### W-3 (blackout-heal; restore 20/61s)"
echo "size | restore | terminal | rx_events | finalized | pending | cs_probe_rx"
for sz in 65 300 1200; do for rst in 20 61; do
  c="$D/w3-$sz-$rst-cstx.txt"; r="$D/w3-$sz-$rst-rxdone.marker"
  echo "$sz | $rst | $(g "$c" cs_terminals) | $(g "$r" receiver_events) | $(g "$r" finalized_blocks) | $(g "$r" pending_blocks) | $(g "$r" cs_probe_rx)"
done; done
echo

echo "### W-1 (OFF vs ON; block-0 completion vs T_W1_BOUND=30s)"
echo "size | mode | rx_events | finalized | pending | terminal | block0_complete_ms"
for sz in 65 300 1200; do
  for mode in on off; do
    r="$D/w1$mode-$sz-rxdone.marker"; c="$D/w1$mode-$sz-cstx.txt"; led="$D/w1$mode-$sz-rxledger.csv"
    # block-0 complete_ms: rxledger'da block==0 satırının complete_ms sütunu (varsa)
    b0=$(awk -F, 'NR>1 && $2==0 {print $4; exit}' "$led" 2>/dev/null)
    echo "$sz | $mode | $(g "$r" receiver_events) | $(g "$r" finalized_blocks) | $(g "$r" pending_blocks) | $(g "$c" cs_terminals) | ${b0:-NA}"
  done
done
echo

echo "### ACCEPTANCE GATE (her koşu receiver_events>0 olmalı; =0 → INVALID/non-overlap)"
inv=0
for m in "$D"/*-rxdone.marker; do
  [ -f "$m" ] || continue
  ev=$(g "$m" receiver_events)
  if [ "${ev:-0}" = "0" ]; then echo "INVALID(receiver_events=0): $(basename "$m")"; inv=$((inv+1)); fi
done
[ "$inv" = "0" ] && echo "TÜM-KOŞULAR receiver_events>0 (overlap OK)" || echo "TOPLAM-INVALID=$inv"
