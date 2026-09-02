#!/usr/bin/env bash
# Commit-FR/FS — WALL-CLOCK exact-deficit pins + utility matrix (FAITHFUL two-netns harness; GERÇEK RTT).
#
# ÇERÇEVE: Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ/substrat (§9-retransmit + §11-forward-recovery)
# BÜTÜN KOLLARDA AKTİFTİR. Ölçülen: ARQ-substratı üzerine eklenen proaktif-FEC-parity'nin (r) marjinal
# WALL-CLOCK faydası. r0=ARQ+0..r4=ARQ+4. FEC ARQ'yu değiştirmez; residual ARQ tamamlar (hibrit).
#
# HARNESS: faithful_harness.sh (unshare -rnm + child-netns; cross-ns netem SADIK-gecikme; acceptance-pin-
# kanıtlı 6/6). RTT profilleri GERÇEK (rootless-tek-ns netem-yalan-gecikmesi DEĞİL).
#
# İKİ-MOD: iso (--drop-repair; saf-FEC-decodability; delay-bağımsız-anchor) + hyb (substrat-aktif;
# FEC-vs-substrat wall-clock-yarışı — ASIL yeni-bilgi). Zengin-ledger: first_repair(T_fec parity-arrival),
# arq_repair_ms(T_arq substrat-onarım-arrival), decode_via(0=substrat,1=FEC), rel_ms(completion-latency).
# Deadline OFFLINE-frozen (Phase-C probe → analiz); op_horizon burada yalnız-referans (geniş).
#
# WARMUP: deficit_from_block=DFB (cold-start-deadlock önle; Phase-A bulgusu: ilk-paket-kaybı PreConverge-
# pinli, geometrik-kopya ~61s@1200B). Deficit yalnız warm-steady bloklara.
set -uo pipefail
LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/faithful_harness.sh"
BIN=${RUNNER_BIN:?RUNNER_BIN}; OUT=${WALLCLOCK_OUT:?WALLCLOCK_OUT}; mkdir -p "$OUT"
W=${W_UNITS:-240}; DFB=${DEFICIT_FROM_BLOCK:-15}; REPS=${REPS:-2}
PROFILES=${RTT_PROFILES:-"rtt250:125 rtt1000:500 rtt2500:1250"}  # label:one-way-ms
export LIB BIN OUT W DFB REPS PROFILES
unshare -rnm bash <<'INNER'
set -uo pipefail
source "$LIB"; faithful_setup; trap faithful_teardown EXIT
{ echo "=== ENV ==="; uname -a; tc -V; echo "git: $(git -C /home/ryuzaki/Desktop/kama-core rev-parse --short HEAD)"; } > "$OUT/ENV.txt"

PORTSEQ=0
# Globaller: LABEL (profil), OW (one-way-ms) — profil-döngüsünde set edilir.
run() { # $1=pin $2=deficit $3=r $4=mode $5=rep
  local dr=""; [ "$4" = iso ] && dr="--drop-repair"
  local rid="$LABEL-$1-d$2-r$3-$4-$5"
  # PER-RUN PORT: netem-buffered stale-paketler (bir-önceki-run) yeni-run'a-karışmasın.
  PORTSEQ=$(( (PORTSEQ + 1) % 9000 ))
  local tp=$(( 40000 + PORTSEQ * 2 )); local rp=$(( tp + 1 ))
  local settle=$(( OW * 6 )); [ $settle -lt 4000 ] && settle=4000
  faithful_rx "$BIN" --role rx --bind 10.9.0.2:$rp --peer 10.9.0.1:$tp --run-id "$rid" --output-dir "$OUT" \
    --deficit "$2" --deficit-from-block "$DFB" $dr --operational-horizon 3000 --extended-horizon 120000 2>/dev/null &
  local RX=$!; sleep 0.3
  timeout 115 "$BIN" --role tx --bind 10.9.0.1:$tp --peer 10.9.0.2:$rp --policy leftover --fixed-r "$3" \
    --run-id "$rid" --seed $((100+$5)) --output-dir "$OUT" --operational-horizon 3000 --extended-horizon 120000 \
    --w-units "$W" --tx-settle-ms "$settle" 2>/dev/null || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
  wait $RX 2>/dev/null || echo "RXFAIL $rid" >> "$OUT/RUNLOG"
  echo "$rid done" >> "$OUT/RUNLOG"
}

for prof in $PROFILES; do
  LABEL=${prof%%:*}; OW=${prof##*:}
  faithful_netem "delay ${OW}ms" "delay ${OW}ms"
  # RTT acceptance-mini-pin (her profil): ölçülen-RTT logla.
  m=$(ping -c 1 -W 3 10.9.0.2 >/dev/null 2>&1; ping -c 3 -W 5 -i 0.3 10.9.0.2 2>&1 | grep "min/avg" | grep -oE "= [0-9.]+/" | grep -oE "[0-9.]+" | head -1)
  echo "$LABEL one-way=${OW}ms measured-RTT-min=${m}ms (configured $((OW*2)))" >> "$OUT/RTT_PINS.txt"
  for rep in $(seq 1 "$REPS"); do
    # HİBRİT (asıl yeni-bilgi: wall-clock FEC-vs-substrat yarışı) — dengeli-sıra
    run pin1 1 0 hyb $rep; run pin1 1 1 hyb $rep
    run pin2 2 1 hyb $rep; run pin2 2 2 hyb $rep
    run pin3 4 2 hyb $rep; run pin3 4 4 hyb $rep
    # İZOLASYON-anchor (delay-bağımsız kombinatoryal; d1 yeterli-anchor)
    run pin1 1 0 iso $rep; run pin1 1 1 iso $rep
  done
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
