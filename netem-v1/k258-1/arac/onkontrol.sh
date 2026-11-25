#!/usr/bin/env bash
# Ön kontrol (netem yok, loopback iki süreç, bitmap ACK, aynalı yön hızı profili, canlı ayna açık)
# + düğümün başlangıç kaydının (transport-start.txt) koşu SIRASINDA kopyası (düzenek koşu sonunda
# düğüm dizinini siler). Örnek: f0k-kapi/kucuk-sha.sh; fark: ikili yolu, motor ortamı.
set -u
R=$1; BIN=$2
COMMON=(--dir "$R" --alice-addr 127.0.0.1:47631 --bob-addr 127.0.0.1:47632 --scenario dosyali --profile kucuk-loopback --rep 1
  --file-bytes 120000 --file-at-ms 3000 --warmup-texts 2 --warmup-every-ms 500 --texts 3 --text-at-ms 4000 --text-every-ms 2500
  --sigs 1 --sig-at-ms 5000 --ptt-at-ms none --karsi off --window-s 120 --receipt-grace-s 30 --connect-timeout-s 20 --probe-ms 200 --live-diag on)
export KAMA_ACK_FORMAT=bitmap KAMA_TRANSPORT_ENGINE=cc-port KAMA_PILOT_RECORD=1
sha256sum "$BIN" > "$R/ikili.sha256"
( for me in alice bob; do
    ( for _ in $(seq 1 1200); do
        [ -s "$R/node-$me/transport-start.txt" ] && { cp "$R/node-$me/transport-start.txt" "$R/start-$me.txt"; break; }
        sleep 0.05
      done ) &
  done; wait ) &
WP=$!
KAMA_LINK_RATES=62,12500 "$BIN" --role bob "${COMMON[@]}" > "$R/bob.stdout" 2> "$R/bob.stderr" &
BP=$!
sleep 1
KAMA_LINK_RATES=12500,62 "$BIN" --role alice "${COMMON[@]}" > "$R/alice.stdout" 2> "$R/alice.stderr"
echo "alice exit=$?" > "$R/exit"
wait $BP; echo "bob exit=$?" >> "$R/exit"
kill $WP 2>/dev/null; wait $WP 2>/dev/null
