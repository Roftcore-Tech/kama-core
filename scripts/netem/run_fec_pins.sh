#!/usr/bin/env bash
# Commit-FK — MARJİNAL-FEC-UTILITY MEKANİK-PİNLERİ (exact-source-deficit; DELAY-BAĞIMSIZ kombinatoryal).
#
# ÇERÇEVE (ZORUNLU): Bu deney ARQ ile FEC'i KARŞILAŞTIRMAZ. ARQ/substrat (§9-retransmit + §11-forward-
# recovery) BÜTÜN KOLLARDA AKTİFTİR. Ölçülen: ARQ-substratı üzerine eklenen proaktif-FEC-parity'nin
# (r) MARJİNAL decode-faydası. r0=ARQ+0, r1=ARQ+1, r2=ARQ+2, r4=ARQ+4. FEC ARQ'yu değiştirmez;
# FEC'in kapatamadığı residual DAİMA ARQ ile tamamlanır (hibrit).
#
# HARNESS-KISITI (KANITLANMIŞ): rootless-userns (unshare -r) tek-namespace-veth netem GECİKMEYİ SADAKATLE
# UYGULAMAZ (ping RTT≈0.03ms; qdisc-yapılandırılsa-da veth-aynı-ns eşleşen-uçları hızlı-yolda-teslim).
# Cross-namespace netem /var/run/netns-mount-izni-yok → root'suz-imkansız. ⇒ WALL-CLOCK RTT>deadline
# ayrışması BU HARNESS'TA KURULAMAZ. Bu yüzden mekanik-ayrışma DELAY-BAĞIMSIZ KOMBİNATORYAL özellik
# üzerinden kurulur: MDS-decodability — (K-deficit)+r ≥ K ⟺ r ≥ deficit. Wall-clock-latency-avantajı
# (FEC=0-round-trip proaktif; substrat-onarımı≥1-round-trip reaktif/ack-conf-tetikli) YAPISAL-COROLLARY
# olarak belirtilir, ölçülMEZ (ayrı sensör/ölçüm-DR gerektirir).
#
# İKİ-MOD:
#   FEC-IZOLASYON (--drop-repair): silinen-esi'nin substrat-onarımı (0x01 ARQ, 0x03 §11-dup) da düşer →
#     blok YALNIZ first-flight (kalan-source + parity) ile tamamlanabilir → SAF-FEC-decodability.
#       r>=deficit → COMPLETE (decode_via=1 FEC);  r<deficit → PENDING (FEC-imkansız).
#   HİBRİT (default): substrat-onarımı geçer → r<deficit'te ARQ/§11 residual'ı tamamlar (decode_via=0,
#     repair_kind=1 §9-ARQ / 3 §11-dup). "veri-ulaşsın" hibrit-taban gösterimi.
#
# Warmup: --deficit-from-block N (ilk-N-blok temiz). İlk-PAKET-kaybı cold-start (PreConverge, PTO-donuk,
# 1-birim-floor) DEADLOCK yapar (FEC'ten-bağımsız) → deficit yalnız warm-steady bloklara uygulanır
# (bu, FM cold-start-vs-steady gözlemlenebilirlik bulgusudur).
set -euo pipefail
BIN=${RUNNER_BIN:?RUNNER_BIN}; OUT=${NETEM_OUT:?NETEM_OUT}; mkdir -p "$OUT"
W=${W_UNITS:-400}; DFB=${DEFICIT_FROM_BLOCK:-20}; REPS=${REPS:-3}
export BIN OUT W DFB REPS
# Not: netem YALNIZ yön-doğrulama (packet-birebir-değil; delay-uygulanmıyor). Kombinatoryal-katman birincil.
unshare -r -n -m bash <<'INNER'
set -euo pipefail
ip link add v0 type veth peer name v1
ip addr add 10.77.0.1/24 dev v0; ip addr add 10.77.0.2/24 dev v1
ip link set v0 up; ip link set v1 up; ip link set lo up
uname -a > "$OUT/ENV.txt"; tc -V >> "$OUT/ENV.txt" 2>&1 || true
# Harness-kısıtı kanıtı ledger'a: RTT ölç (delay-uygulanMAdığını belgele).
ping -c 2 -W 2 10.77.0.2 2>&1 | tail -2 >> "$OUT/ENV.txt" || true
run() { # $1=pin $2=deficit $3=r $4=mode(iso|hyb) $5=rep
  local dr=""; [ "$4" = iso ] && dr="--drop-repair"
  local rid="$1-d$2-r$3-$4-$5"
  $BIN --role rx --bind 10.77.0.2:47002 --peer 10.77.0.1:47001 --run-id "$rid" --output-dir "$OUT" \
    --deficit "$2" --deficit-from-block "$DFB" $dr \
    --operational-horizon 2000 --extended-horizon 40000 2>/dev/null & local RX=$!
  timeout 45 $BIN --role tx --bind 10.77.0.1:47001 --peer 10.77.0.2:47002 --policy leftover --fixed-r "$3" \
    --run-id "$rid" --seed $((100+$5)) --output-dir "$OUT" \
    --operational-horizon 2000 --extended-horizon 40000 --w-units "$W" \
    || echo "TXFAIL $rid" >> "$OUT/RUNLOG"
  wait $RX 2>/dev/null || echo "RXFAIL $rid" >> "$OUT/RUNLOG"
  echo "$rid done" >> "$OUT/RUNLOG"
}
for rep in $(seq 1 "$REPS"); do
  for mode in iso hyb; do
    run pin1 1 0 $mode $rep; run pin1 1 1 $mode $rep   # deficit=1: r0-vs-r1
    run pin2 2 1 $mode $rep; run pin2 2 2 $mode $rep   # deficit=2: r1-vs-r2
    run pin3 4 2 $mode $rep; run pin3 4 4 $mode $rep   # deficit=4: r2-vs-r4
    run pin4 5 4 $mode $rep                            # deficit=5: r4 (residual-1 → hibrit-ARQ)
  done
done
echo "ALL-DONE" >> "$OUT/RUNLOG"
INNER
