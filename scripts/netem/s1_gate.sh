#!/usr/bin/env bash
# [S1-GATE] Tez de-risk (kısa, bounded): iki-netns + netem (gerçek RTT → loss-detector doğru
# çalışır, loopback patolojisi yok) altında FEC-OFF (ARQ-only) vs FEC-ON completion.
# Tez: yüksek-kayıpta coding, ARQ-alone'un deadline içinde tamamlayamadığını tamamlar (RTT-bağlı
# retransmit turlarından az). GATE-kriteri: FEC-ON completion ≥ FEC-OFF (zarar yok) + tercihen >.
# UZUN kampanya DEĞİL; ekran-teşhisi. Kullanım: s1_gate.sh [UNITS] [LOSS] [DELAY] [REPS] [DEADLINE]
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
UNITS="${1:-80}"; LOSS="${2:-30%}"; DELAY="${3:-25ms}"; REPS="${4:-3}"; DEADLINE="${5:-20}"
BIN="$REPO/target/release/examples/p1_endpoint"
[ -x "$BIN" ] || { echo "önce: cargo build --release -p client-core --example p1_endpoint"; exit 3; }

if [ -z "${S1_INNER:-}" ]; then
  S1_INNER=1 exec unshare -rnm bash "$0" "$UNITS" "$LOSS" "$DELAY" "$REPS" "$DEADLINE"
fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
# Forward (DATA) ağır kayıp; reverse (ACK) hafif → karşılaştırma DATA-kurtarımı üstünde.
kapi3_netem "delay $DELAY loss $LOSS" "delay $DELAY loss 5%"

PORT=4600
run_arm() {  # $1=etiket  $2..=fec args
  local label="$1"; shift; local fargs="$*"
  local sd=0 ac=0
  for rep in $(seq 1 "$REPS"); do
    PORT=$((PORT+1))
    nsenter -t "$CHLB" -n "$BIN" server --bind "10.10.1.1:$PORT" --expect "$UNITS" \
      --deadline-s "$DEADLINE" --linger-s 3 $fargs 2>s.err >/dev/null &
    local SRV=$!; sleep 1
    "$BIN" client --server "10.10.1.1:$PORT" --units "$UNITS" --interval-ms "${INTERVAL:-20}" \
      --deadline-s "$DEADLINE" $fargs 2>c.err >/dev/null || true
    wait "$SRV" 2>/dev/null || true
    local d a
    d=$(grep -oE 'delivered=[0-9]+' s.err | grep -oE '[0-9]+' | head -1); d=${d:-0}
    a=$(grep -oE 'acked=[0-9]+'      c.err | grep -oE '[0-9]+' | head -1); a=${a:-0}
    sd=$((sd+d)); ac=$((ac+a))
    printf '  %-14s rep%d: delivered=%d/%d acked=%d/%d\n' "$label" "$rep" "$d" "$UNITS" "$a" "$UNITS"
  done
  local tot=$((UNITS*REPS))
  printf 'ARM %-14s mean_delivered=%d/%d  mean_acked=%d/%d\n' \
    "$label" $((sd/REPS)) "$UNITS" $((ac/REPS)) "$UNITS"
  echo "$label $sd $ac $tot" >> "$RESULT"
}

RESULT="$(mktemp)"
echo "=== S1-GATE  units=$UNITS loss=$LOSS delay=$DELAY reps=$REPS deadline=${DEADLINE}s ==="
run_arm "FEC-OFF"
run_arm "FEC-static-r4" --fec-r 4
run_arm "FEC-adaptive"  --fec-adaptive
echo "--- ÖZET (toplam delivered/acked over $((UNITS*REPS))) ---"
awk '{printf "%-14s delivered=%d/%d (%.1f%%)  acked=%d/%d (%.1f%%)\n",$1,$2,$4,100*$2/$4,$3,$4,100*$3/$4}' "$RESULT"
OFF_D=$(awk '$1=="FEC-OFF"{print $2}' "$RESULT")
SR_D=$(awk '$1=="FEC-static-r4"{print $2}' "$RESULT")
AD_D=$(awk '$1=="FEC-adaptive"{print $2}' "$RESULT")
echo "--- HÜKÜM ---"
if [ "${SR_D:-0}" -ge "${OFF_D:-0}" ]; then echo "static-r: NO-HARM (Δ=$((SR_D-OFF_D)) delivered)"; else echo "static-r: HARM (Δ=$((SR_D-OFF_D)))"; fi
if [ "${AD_D:-0}" -gt "${OFF_D:-0}" ]; then echo "adaptive: gate netem'de ATEŞLEDİ (Δ=$((AD_D-OFF_D)) > 0 → loopback-artefaktı doğrulandı)";
elif [ "${AD_D:-0}" -eq "${OFF_D:-0}" ]; then echo "adaptive: OFF ile eş (gate ateşlememiş olabilir; static-r kanıtı bağımsız)";
else echo "adaptive: OFF'un altında (incele)"; fi
rm -f "$RESULT"
