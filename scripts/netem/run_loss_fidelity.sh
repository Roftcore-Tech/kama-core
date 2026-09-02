#!/usr/bin/env bash
# Commit-FT — LOSS-SIGNAL FIDELITY (faithful harness; GERÇEK forward-loss). Prior-turn "loss_windowed_ppm=0
# heryerde" BULGUSU rootless-TEK-ns-netem'de-ölçüldü (loss-UYGULANMADI, delay-gibi). Bu deney GERÇEK
# forward-loss uygular (faithful two-netns) → sensör-nonzero-okuyor-mu? Kod-analizi (loss.rs) diyor-ki:
# loss_windowed_ppm = PRE-RECOVERY erasure-metre (confirmed-DATA-loss / sent-window; recovery-SUBTRACT-etmez,
# retransmit-fresh-seq). ⇒ gerçek-loss'ta NONZERO-beklenir. Bu, prior "sensör-kör" hükmünü SINAR.
#
# ÇERÇEVE: ARQ-vs-FEC değil. ARQ bütün-kollarda-aktif; bu faz SENSÖR-fidelity ölçer (utility değil).
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
BIN=${RUNNER_BIN:?}; OUT=${LOSS_OUT:?}; mkdir -p "$OUT"
export LIB BIN OUT
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"; faithful_setup; trap faithful_teardown EXIT
PORTSEQ=0
run() { # $1=loss-label $2=fwd-loss-spec $3=rep
  PORTSEQ=$((PORTSEQ+1)); local tp=$((45000+PORTSEQ*2)); local rp=$((tp+1))
  local rid="$1-$3"
  # forward=v0 (tx→rx) GERÇEK-loss + delay-50ms; reverse=temiz (ACK-yolu; forward-loss izole).
  faithful_netem "delay 50ms $2" "delay 50ms"
  faithful_rx "$BIN" --role rx --bind 10.9.0.2:$rp --peer 10.9.0.1:$tp --run-id "$rid" --output-dir "$OUT" \
    --operational-horizon 3000 --extended-horizon 90000 2>/dev/null & local RX=$!
  sleep 0.3
  # --emit-fuzzy: loss_windowed_ppm dahil canonical-fuzzy-event-dataset. w_units büyük (warm-steady;
  # W_loss≈8×ack_epoch penceresi-dolsun). tx-settle geniş (ARQ-round-trip'ler).
  timeout 85 "$BIN" --role tx --bind 10.9.0.1:$tp --peer 10.9.0.2:$rp --policy leftover --fixed-r 0 --emit-fuzzy \
    --run-id "$rid" --seed $((100+$3)) --output-dir "$OUT" --operational-horizon 3000 --extended-horizon 90000 \
    --w-units 400 --tx-settle-ms 6000 2>/dev/null || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
  wait $RX 2>/dev/null || true
  echo "$rid done" >> "$OUT/RUNLOG"
}
for rep in 1 2; do
  run loss0  ""          $rep
  run loss10 "loss 10%"  $rep
  run loss20 "loss 20%"  $rep
  run loss30 "loss 30%"  $rep
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
