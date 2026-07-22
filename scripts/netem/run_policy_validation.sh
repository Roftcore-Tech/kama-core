#!/usr/bin/env bash
# Z4 — DAR-NETEM-DOĞRULAMASI (Leftover-vs-F15; shortlist=S1). ROOT-GEREKTİRİR.
# BLOCKER-NOTU: non-interaktif-CI/sandbox'ta sudo-parolasız-yetki-YOK → gerçek-koşu-bu-ortamda-BLOKE.
# Güvenlik-sınırları: yalnız-veth-namespace-çifti (host-trafiğine-dokunmaz); cleanup-trap'li.
set -euo pipefail
NS1=kama_tx; NS2=kama_rx; V1=kveth0; V2=kveth1
cleanup(){ sudo ip netns del $NS1 2>/dev/null||true; sudo ip netns del $NS2 2>/dev/null||true; }
trap cleanup EXIT
sudo ip netns add $NS1; sudo ip netns add $NS2
sudo ip link add $V1 type veth peer name $V2
sudo ip link set $V1 netns $NS1; sudo ip link set $V2 netns $NS2
sudo ip netns exec $NS1 ip addr add 10.99.0.1/24 dev $V1; sudo ip netns exec $NS1 ip link set $V1 up
sudo ip netns exec $NS2 ip addr add 10.99.0.2/24 dev $V2; sudo ip netns exec $NS2 ip link set $V2 up
# 6-profil (talimat-§21): her-biri-için tc-netem + 10-tekrar (gap80-benzeri: 30).
declare -A PROF=(
  [p1_rtt1s_burst]="delay 500ms loss gemodel 25% 40%"
  [p2_rtt2500_ge]="delay 1250ms loss gemodel 10% 60%"
  [p3_rev_blackout]="delay 50ms"          # blackout: rx-yönü-ifb'de-periyodik-%100-loss (aşağıda-döngü)
  [p4_offered_eq_c]="delay 50ms rate 480kbit"
  [p5_lowbw_30k]="delay 50ms rate 30kbit"
  [p6_gap80_under]="delay 50ms rate 480kbit loss 5%"
)
OUT=${NETEM_OUT:?NETEM_OUT-dizini}
for prof in "${!PROF[@]}"; do
  reps=10; [[ $prof == p6_* ]] && reps=30
  for rep in $(seq 1 $reps); do
    for policy in leftover f15; do
      sudo ip netns exec $NS1 tc qdisc replace dev $V1 root netem ${PROF[$prof]}
      # BEKLENEN-KOŞUCU: bench-crate-ikilisi (UDP-soket-üstünde gerçek-Sender; policy-arg'lı).
      # cargo run --release -p bench --features adaptive-transport/experimental-fec-budget -- \
      #   --role tx --peer 10.99.0.2 --policy $policy --seed $((100+rep)) --out $OUT/$prof-$policy-$rep.csv
      echo "PLACEHOLDER: bench-koşucusu-bağlanacak ($prof/$policy/rep-$rep)" >> "$OUT/RUNLOG"
    done
  done
done
# Beklenen-çıktı: $OUT/<prof>-<policy>-<rep>.csv (blok-ledger-şeması pos-block-ledger-ile-aynı) +
# ham-paket-trace (tcpdump -i $V1 -w). Sonrası: scripts/pos_analyze.py + sim↔netem-yön-tablosu.
