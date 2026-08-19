#!/usr/bin/env bash
# [KAPI-1 W5+W7] kapi3 harness SELFTEST — enstrüman doğrulaması (measurement-tool-first).
# Result-bearing koşu DEĞİLDİR; harness'in ölçmeye uygunluğunu pinler:
#   P1 baseline RTT < 20ms (topoloji + forward çalışıyor)
#   P2 netem delay 100ms×2yön → RTT ∈ [180,320]ms (netem ORTA-ns'te SADAKATLE uygulanıyor)
#   P3 fwd loss 100% → teslim 0 AMA ARP pinli kalıyor; kaldırınca teslim geri geliyor
#   P4 üç-düzlem capture: injected(A) == gönderilen N; admitted(MID)≈N/2 (loss 50%);
#      received(B) == admitted (kayıp YALNIZ netem'de; capture kendisi kayıpsız)
#   P5 trace uygulayıcı: pencere içinde kayıp %100, pencere sonrası düzeliyor
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"

if [ "${1:-}" != "--inner" ]; then
  exec unshare -rnm bash "$0" --inner
fi

source "$DIR/kapi3_harness.sh"
PASS=0; FAIL=0
ok()  { echo "PASS: $1"; PASS=$((PASS+1)); }
bad() { echo "FAIL: $1"; FAIL=$((FAIL+1)); }

TMP=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown; rm -rf "$TMP"' EXIT

kapi3_setup
echo "offload: $KAPI3_OFFLOAD"

# Echo server B'de.
nsenter -t "$CHLB" -n python3 "$DIR/udp_echo.py" server 10.10.1.1 9100 > "$TMP/echo.ready" 2> "$TMP/echo.err" < /dev/null &
for _ in $(seq 1 40); do grep -q READY "$TMP/echo.ready" 2>/dev/null && break; sleep 0.05; done

# P1 — baseline
R=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9100 5 1.0)
rx=$(echo "$R" | jq .received); avg=$(echo "$R" | jq '.rtt_ms_avg // 9999')
if [ "$rx" = 5 ] && python3 -c "exit(0 if $avg < 20 else 1)"; then ok "P1 baseline rx=5 rtt=${avg}ms"; else bad "P1 $R"; fi

# P2 — delay sadakati
kapi3_netem "delay 100ms" "delay 100ms"
R=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9100 5 2.0)
avg=$(echo "$R" | jq '.rtt_ms_avg // 0')
if python3 -c "exit(0 if 180 <= $avg <= 320 else 1)"; then ok "P2 delay-fidelity rtt=${avg}ms (beklenen ~200)"; else bad "P2 $R"; fi

# P3 — %100 fwd kayıpta ARP pinli; kaldırınca düzelir
kapi3_netem "loss 100%" "none"
R=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9100 4 0.5)
rx=$(echo "$R" | jq .received)
arp=$(ip neigh show dev va0 | grep -c PERMANENT)
if [ "$rx" = 0 ] && [ "$arp" -ge 1 ]; then ok "P3a loss100 rx=0, ARP PERMANENT"; else bad "P3a rx=$rx arp=$arp"; fi
kapi3_netem "none" "none"
R=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9100 4 1.0)
rx=$(echo "$R" | jq .received)
if [ "$rx" -ge 3 ]; then ok "P3b restore rx=$rx/4"; else bad "P3b $R"; fi

# P4 — üç-düzlem capture tutarlılığı (tek-yön blast, loss 50%)
kapi3_netem "loss 50%" "none"
kapi3_capture_start "$TMP/cap"
kapi3_a python3 - <<'PYEOF'
import socket, time
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
for i in range(200):
    s.sendto(i.to_bytes(4, "big") + b"\x5a" * 96, ("10.10.1.1", 9200))
    time.sleep(0.004)
PYEOF
sleep 0.5
kapi3_capture_stop
inj=$(jq -s '[.[] | select(.dir=="out" and .dport==9200)] | length' "$TMP/cap/A_va0.jsonl")
adm=$(jq -s '[.[] | select(.dir=="out" and .dport==9200)] | length' "$TMP/cap/MID_mb0.jsonl")
rcv=$(jq -s '[.[] | select(.dir=="in"  and .dport==9200)] | length' "$TMP/cap/B_vb0.jsonl")
if [ "$inj" = 200 ]; then ok "P4a injected==200 (pre-impairment YAPISAL)"; else bad "P4a injected=$inj"; fi
if [ "$adm" -ge 60 ] && [ "$adm" -le 140 ]; then ok "P4b admitted=$adm (~100, loss50)"; else bad "P4b admitted=$adm"; fi
if [ "$rcv" = "$adm" ]; then ok "P4c received==admitted ($rcv)"; else bad "P4c rcv=$rcv adm=$adm"; fi
kapi3_realized "$TMP/cap" selftest
[ -s "$TMP/cap/realized_selftest.json" ] && ok "P4d realized-log yazıldı" || bad "P4d realized-log yok"
kapi3_netem "none" "none"

# P5 — trace uygulayıcı (0s: %100 kayıp; 2s: temiz)
cat > "$TMP/plan.txt" <<'PLAN'
0 loss 100%|none
2 none|none
PLAN
kapi3_trace "$TMP/plan.txt"
sleep 0.6
R1=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9100 3 0.3)
sleep 2.0
R2=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9100 3 1.0)
kapi3_trace_stop
rx1=$(echo "$R1" | jq .received); rx2=$(echo "$R2" | jq .received)
if [ "$rx1" = 0 ] && [ "$rx2" -ge 2 ]; then ok "P5 trace pencere: içinde rx=0, sonra rx=$rx2"; else bad "P5 rx1=$rx1 rx2=$rx2"; fi

echo "----"
echo "SELFTEST: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" = 0 ]
