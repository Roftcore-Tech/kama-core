#!/usr/bin/env bash
# [KAPI-1 W5+W7] ÜÇ-NAMESPACE faithful netem harness (rootless; KAPI-1 §4.1 topolojisi).
#
#   ns-A (parent) ── va0 ↔ ma0 ── ns-MID ── mb0 ↔ vb0 ── ns-B
#        10.10.0.1      10.10.0.2   (netem)   10.10.1.2      10.10.1.1
#
# NEDEN ÜÇ NS: Linux packet-tap'leri egress'te qdisc-SONRASI ateşler → iki-ns'te endpoint-egress
# pcap'i netem-sonrası (post-impairment) olurdu. netem'i ORTA ns'e taşımak endpoint-egress
# capture'ını YAPISAL pre-impairment yapar (r5 H "capture qdisc-ÖNCESİ" şartının rootless yolu).
#
# Düzlemler (kapi3_capture_start): A/va0 out = injected_fwd · MID/mb0 out = bottleneck_admitted_fwd ·
# B/vb0 in = receiver_observed_fwd (reverse simetrik: B/vb0 out · MID/ma0 out · A/va0 in).
# Capture: pkt_capture.py (AF_PACKET, bağımlılıksız) + tcpdump VARSA yanına pcap (dış-denetim).
# OTORİTER enjeksiyon sayacı BudgetGovernor'dadır (bench RunMeta.budget); bu katman çapraz-teyit.
#
# pin_neigh: statik ARP dört arayüzde de (r5 J ön-koşulu; ARP-katmanı drop'u ayrıca doğrulanır).
# Offload: ethtool VARSA gro/gso/tso/lro kapatılır; yoksa uyarı loglanır (küçük-UDP + GSO-wrapper
# kapalıyken etki düşük; evaluation-preflight strict modda ethtool+tcpdump ZORUNLU).
#
# Kullanım: `unshare -rnm` İÇİNDE source edilir:
#   kapi3_setup            → topoloji + pin + offload (MID/B pid'leri: $MID, $CHLB)
#   kapi3_netem FWD REV    → netem YALNIZ orta-ns egress'lerinde
#   kapi3_trace FILE       → [W7] zaman-pencere planı uygulayıcı (arka plan; kapi3_trace_stop)
#   kapi3_capture_start DIR / kapi3_capture_stop
#   kapi3_realized DIR TAG → tc -s -j + neigh durumu (realized-channel log şeması, r5 J)
#   kapi3_a CMD… / kapi3_mid CMD… / kapi3_b CMD…   (ns'te koştur; A = mevcut kabuk)
#   kapi3_teardown

KAPI3_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KAPI3_A_IP=10.10.0.1
KAPI3_MIDA_IP=10.10.0.2
KAPI3_MIDB_IP=10.10.1.2
KAPI3_B_IP=10.10.1.1

kapi3_setup() { # yalnız unshare -rnm İÇİNDE
  ip link set lo up
  # İki child-netns: PID-referanslı (mount gerektirmez; faithful_harness deseninin genişletilmişi).
  unshare -n sleep 100000 & MID=$!
  unshare -n sleep 100000 & CHLB=$!
  for _ in $(seq 1 50); do [ -e "/proc/$MID/ns/net" ] && [ -e "/proc/$CHLB/ns/net" ] && break; sleep 0.05; done

  ip link add va0 type veth peer name ma0
  ip link add vb0 type veth peer name mb0
  ip link set ma0 netns "$MID"
  ip link set mb0 netns "$MID"
  ip link set vb0 netns "$CHLB"

  ip addr add "$KAPI3_A_IP/24" dev va0; ip link set va0 up
  nsenter -t "$MID" -n ip link set lo up
  nsenter -t "$MID" -n ip addr add "$KAPI3_MIDA_IP/24" dev ma0
  nsenter -t "$MID" -n ip addr add "$KAPI3_MIDB_IP/24" dev mb0
  nsenter -t "$MID" -n ip link set ma0 up
  nsenter -t "$MID" -n ip link set mb0 up
  nsenter -t "$CHLB" -n ip link set lo up
  nsenter -t "$CHLB" -n ip addr add "$KAPI3_B_IP/24" dev vb0
  nsenter -t "$CHLB" -n ip link set vb0 up

  # Orta-ns forward + rp_filter kapalı (asimetrik profillerde ters-yol filtresi paket yemesin).
  nsenter -t "$MID" -n sysctl -qw net.ipv4.ip_forward=1
  nsenter -t "$MID" -n sysctl -qw net.ipv4.conf.all.rp_filter=0
  nsenter -t "$MID" -n sysctl -qw net.ipv4.conf.ma0.rp_filter=0
  nsenter -t "$MID" -n sysctl -qw net.ipv4.conf.mb0.rp_filter=0

  ip route add 10.10.1.0/24 via "$KAPI3_MIDA_IP" dev va0
  nsenter -t "$CHLB" -n ip route add 10.10.0.0/24 via "$KAPI3_MIDB_IP" dev vb0

  # pin_neigh: DÖRT arayüzde statik ARP (r5 J; sub-kbps'te ARP netem kuyruğunda sessiz ölür).
  local m_va0 m_ma0 m_mb0 m_vb0
  m_va0=$(ip link show va0 | awk '/link\/ether/{print $2}')
  m_ma0=$(nsenter -t "$MID" -n ip link show ma0 | awk '/link\/ether/{print $2}')
  m_mb0=$(nsenter -t "$MID" -n ip link show mb0 | awk '/link\/ether/{print $2}')
  m_vb0=$(nsenter -t "$CHLB" -n ip link show vb0 | awk '/link\/ether/{print $2}')
  ip neigh replace "$KAPI3_MIDA_IP" lladdr "$m_ma0" dev va0 nud permanent
  nsenter -t "$MID" -n ip neigh replace "$KAPI3_A_IP" lladdr "$m_va0" dev ma0 nud permanent
  nsenter -t "$MID" -n ip neigh replace "$KAPI3_B_IP" lladdr "$m_vb0" dev mb0 nud permanent
  nsenter -t "$CHLB" -n ip neigh replace "$KAPI3_MIDB_IP" lladdr "$m_mb0" dev vb0 nud permanent

  # Offload (r5 H): ethtool varsa kapat; yoksa kaydet (evaluation-preflight strict'te ZORUNLU).
  if command -v ethtool >/dev/null 2>&1; then
    ethtool -K va0 gro off gso off tso off lro off >/dev/null 2>&1 || true
    nsenter -t "$MID" -n ethtool -K ma0 gro off gso off tso off lro off >/dev/null 2>&1 || true
    nsenter -t "$MID" -n ethtool -K mb0 gro off gso off tso off lro off >/dev/null 2>&1 || true
    nsenter -t "$CHLB" -n ethtool -K vb0 gro off gso off tso off lro off >/dev/null 2>&1 || true
    KAPI3_OFFLOAD="ethtool-off"
  else
    KAPI3_OFFLOAD="ethtool-missing"
  fi
  export MID CHLB KAPI3_OFFLOAD
}

kapi3_a()   { "$@"; }
kapi3_mid() { nsenter -t "$MID" -n "$@"; }
kapi3_b()   { nsenter -t "$CHLB" -n "$@"; }

# netem YALNIZ orta-ns'te: forward = mb0-egress (B'ye doğru), reverse = ma0-egress (A'ya doğru).
kapi3_netem() { # $1=fwd-spec $2=rev-spec ("none" = kaldır)
  if [ "$1" = "none" ]; then nsenter -t "$MID" -n tc qdisc del dev mb0 root 2>/dev/null || true
  else nsenter -t "$MID" -n tc qdisc replace dev mb0 root netem $1; fi
  if [ "$2" = "none" ]; then nsenter -t "$MID" -n tc qdisc del dev ma0 root 2>/dev/null || true
  else nsenter -t "$MID" -n tc qdisc replace dev ma0 root netem $2; fi
}

# [W7] Zaman-pencere trace uygulayıcı. Dosya satırları (commit'li plan; AYNI plan iki transporta):
#   <offset_s> <fwd-spec…>|<rev-spec…>
# Uygulayıcı monotonic t0'dan ofsetlere uyur (sleep-to-offset; pencere granülü ≥1s, sapma ~ms).
kapi3_trace() { # $1=plan-dosyası (log: $plan.log — boru-mirası hijyeni)
  local plan="$1"
  (
    t0=$(date +%s.%N)
    while IFS= read -r line; do
      case "$line" in ''|'#'*) continue;; esac
      off="${line%% *}"; rest="${line#* }"
      fwd="${rest%%|*}"; rev="${rest#*|}"
      now=$(date +%s.%N)
      sl=$(python3 -c "print(max(0.0, $t0 + $off - $now))")
      sleep "$sl"
      kapi3_netem "$fwd" "$rev"
      echo "TRACE t=$off fwd='$fwd' rev='$rev'"
    done < "$plan"
  ) > "$plan.log" 2>&1 < /dev/null & KAPI3_TRACE_PID=$!
  export KAPI3_TRACE_PID
}
kapi3_trace_stop() { [ -n "${KAPI3_TRACE_PID:-}" ] && kill "$KAPI3_TRACE_PID" 2>/dev/null || true; }

# Üç-düzlem capture: A/va0 · MID/ma0 · MID/mb0 · B/vb0 (yön etiketiyle 6 düzlem türer).
kapi3_capture_start() { # $1=çıktı-dizini
  local d="$1"; mkdir -p "$d"
  # ÖNEMLİ fd-hijyeni: arka-plan süreçleri çağıranın stdout/stderr borusunu MİRAS ALMAMALI —
  # aksi hâlde script bitse bile boru EOF görmez, dıştaki tail/grep sonsuza dek bekler (yaşandı).
  # DİKKAT: fonksiyon-sarmalıyı arka-plana ALMA (alt-kabuk fork → $! ≠ python; TERM ulaşmaz).
  python3 "$KAPI3_DIR/pkt_capture.py" va0 "$d/A_va0.jsonl"  > "$d/A_va0.ready"  2> "$d/A_va0.err"  < /dev/null & KAPI3_CAP1=$!
  nsenter -t "$MID" -n python3 "$KAPI3_DIR/pkt_capture.py" ma0 "$d/MID_ma0.jsonl" > "$d/MID_ma0.ready" 2> "$d/MID_ma0.err" < /dev/null & KAPI3_CAP2=$!
  nsenter -t "$MID" -n python3 "$KAPI3_DIR/pkt_capture.py" mb0 "$d/MID_mb0.jsonl" > "$d/MID_mb0.ready" 2> "$d/MID_mb0.err" < /dev/null & KAPI3_CAP3=$!
  nsenter -t "$CHLB" -n python3 "$KAPI3_DIR/pkt_capture.py" vb0 "$d/B_vb0.jsonl"  > "$d/B_vb0.ready"  2> "$d/B_vb0.err"  < /dev/null & KAPI3_CAP4=$!
  # READY bekle (yarışsız başlangıç).
  for f in A_va0 MID_ma0 MID_mb0 B_vb0; do
    for _ in $(seq 1 40); do grep -q READY "$d/$f.ready" 2>/dev/null && break; sleep 0.05; done
  done
  # tcpdump varsa pcap YANINA (dış-denetim katmanı).
  if command -v tcpdump >/dev/null 2>&1; then
    tcpdump -i va0 -w "$d/A_va0.pcap"  -U udp >/dev/null 2>&1 </dev/null & KAPI3_CAP5=$!
    nsenter -t "$CHLB" -n tcpdump -i vb0 -w "$d/B_vb0.pcap"  -U udp >/dev/null 2>&1 </dev/null & KAPI3_CAP6=$!
  fi
  export KAPI3_CAP1 KAPI3_CAP2 KAPI3_CAP3 KAPI3_CAP4
}
kapi3_capture_stop() {
  for p in "${KAPI3_CAP1:-}" "${KAPI3_CAP2:-}" "${KAPI3_CAP3:-}" "${KAPI3_CAP4:-}" "${KAPI3_CAP5:-}" "${KAPI3_CAP6:-}"; do
    [ -n "$p" ] && kill "$p" 2>/dev/null || true
  done
  sleep 0.3
}

# Realized-channel log (r5 J şeması): configured tc durumu + kuyruğun drop/sent sayaçları + ARP
# durumu + offload kaydı. Analiz katmanı configured-vs-realized kıyasını pcap'ten tamamlar.
kapi3_realized() { # $1=dizin $2=etiket
  local d="$1" tag="$2"
  {
    echo "{\"tag\":\"$tag\",\"t_ns\":$(date +%s%N),\"offload\":\"$KAPI3_OFFLOAD\","
    echo "\"mid_qdisc\":$(nsenter -t "$MID" -n tc -s -j qdisc show 2>/dev/null || echo '[]'),"
    echo "\"neigh_a\":\"$(ip neigh show dev va0 | tr '\n' ';')\","
    echo "\"neigh_b\":\"$(nsenter -t "$CHLB" -n ip neigh show dev vb0 | tr '\n' ';')\"}"
  } >> "$d/realized_$tag.json"
}

kapi3_teardown() {
  kapi3_trace_stop
  kapi3_capture_stop
  [ -n "${MID:-}" ] && kill "$MID" 2>/dev/null || true
  [ -n "${CHLB:-}" ] && kill "$CHLB" 2>/dev/null || true
  ip link del va0 2>/dev/null || true
  ip link del vb0 2>/dev/null || true
}
