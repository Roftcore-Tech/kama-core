#!/usr/bin/env bash
# harness/netem-profiles.sh — 7 hostile profil (transport-layer-tdd.md §4.4.2).
# Per-container egress tc netem (container NET_ADMIN cap ile; host sudo GEREKMEZ).
# Simetrik profiller iki egress'e aynı (çift-yön kanal kaybı); ⑤ ASİMETRİK
# (server egress = ACK/reverse yolu → ack_confidence çöker → EMERGENCY_FORWARD_REDUNDANCY).
set -euo pipefail

# apply_profile <profil> <client_container> <server_container>
apply_profile() {
  local profile="$1" client="$2" server="$3"
  local c=(docker exec "$client" tc qdisc replace dev eth0 root netem)
  local s=(docker exec "$server" tc qdisc replace dev eth0 root netem)
  case "$profile" in
    p1_loss50)  "${c[@]}" loss 50% rate 100kbit; "${s[@]}" loss 50% rate 100kbit ;;
    p2_loss80)  "${c[@]}" loss 80% rate 200kbit; "${s[@]}" loss 80% rate 200kbit ;;
    p3_loss95)  "${c[@]}" loss 95% rate 100kbit; "${s[@]}" loss 95% rate 100kbit ;;
    p4_jitter)  "${c[@]}" delay 200ms 100ms distribution normal rate 100kbit
                "${s[@]}" delay 200ms 100ms distribution normal rate 100kbit ;;
    # ⑤ reverse-path bozuk (ASİMETRİK): forward hafif (%5), ACK yolu bozuk (%80).
    p5_revpath) "${c[@]}" loss 5%  rate 100kbit
                "${s[@]}" loss 80% rate 100kbit ;;
    # ⑦ P_bbr: QUIC'in kazanabileceği bölge — §4.4.5 dürüstlük.
    p7_bbr)     "${c[@]}" loss 2%  rate 50kbit;  "${s[@]}" loss 2%  rate 50kbit ;;
    # ⑥ p6_blackout DİNAMİKtir → run-bench.sh döngüde apply/clear yapar (statik değil).
    *) echo "bilinmeyen profil: $profile" >&2; return 1 ;;
  esac
}

# clear_netem <client_container> <server_container>
clear_netem() {
  docker exec "$1" tc qdisc del dev eth0 root 2>/dev/null || true
  docker exec "$2" tc qdisc del dev eth0 root 2>/dev/null || true
}
