#!/usr/bin/env bash
# [PRODUCT-VS-QUINN] Ürün runtime'ı (kama-product kolu) vs quinn — GERÇEK RTT'li netem hücresi.
#
# NEDEN: bugüne dek her quinn kıyası `--transport kama` ile koştu; o kol kripto-suz bir
# security-stub olduğundan FEC parity YAPISAL OLARAK emit edilemez (INV-FEC-1). Yani quinn hep
# Kama'nın FEC'siz yarısıyla dövüştü. Bu betik ilk kez ürünün kendisini karşısına koyar.
#
# NE DEĞİL: ön-kayıtlı kampanya DEĞİL. Küçük n, tek geçiş, teşhis tier'ı. Hüküm cümlesi
# kurulmaz; sayılar ham raporlanır. Ön-kayıtlı doğrulama ayrı iştir.
#
# Kullanım: product_vs_quinn.sh OUTDIR [REPS] [MSGS] [PAYLOAD] [DEADLINE_S] [INTERVAL_MS]
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
OUT="${1:?kullanım: product_vs_quinn.sh OUTDIR [REPS] [MSGS] [PAYLOAD] [DEADLINE_S] [INTERVAL_MS]}"
REPS="${2:-4}"; MSGS="${3:-100}"; PAYLOAD="${4:-256}"; DEADLINE="${5:-30}"; INTERVAL="${6:-50}"
BENCH="$REPO/target/release/bench"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }

if [ -z "${PVQ_INNER:-}" ]; then
  mkdir -p "$OUT"
  PVQ_INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS" "$MSGS" "$PAYLOAD" "$DEADLINE" "$INTERVAL"
fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup

PORT=4900
SRV_IP=10.10.1.1

# Bir tekrar koştur. $1=çıktı dizini $2=transport $3..=ekstra client argümanları
# Sunucu ve istemci AYNI ek argümanları alır (FEC iki uçta da aynı kurulmalı).
run_rep() {
  local d="$1"; shift
  local tr="$1"; shift
  local extra="$*"
  mkdir -p "$d"; PORT=$((PORT+1))
  # Sunucu: rx-namespace'inde. kama-product sunucusu payload tavanını da bilmek zorunda.
  local srv_extra="$extra"
  if [ "$tr" = "kama-product" ]; then srv_extra="$extra --payload $PAYLOAD"; fi
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "$SRV_IP:$PORT" --transport "$tr" $srv_extra \
    2>"$d/s.err" >/dev/null &
  local SRV=$!
  sleep 1
  # shellcheck disable=SC2086
  timeout $((DEADLINE + 40)) "$BENCH" client --server "$SRV_IP:$PORT" --transport "$tr" \
    --msgs "$MSGS" --payload "$PAYLOAD" --submit-interval-ms "$INTERVAL" \
    --max-deadline-s "$DEADLINE" --out "$d/out.jsonl" $extra \
    2>"$d/c.err" >/dev/null || true
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
}

# Bir kolu REPS kez koştur ve teslim sayısını özetle.
run_arm() {
  local cell="$1"; local label="$2"; shift 2
  for r in $(seq 1 "$REPS"); do
    run_rep "$OUT/$cell/$label/rep$r" "$@"
  done
  echo "  $label: koştu ($REPS rep)"
}

# Hücreler GERÇEKTEN DÜŞMAN olmalı, yoksa hiçbir kol ayrışmaz.
#
# Kalibrasyon dersi (bu koşudan önce ölçüldü): `loss 30% delay 25ms deadline 20s` hücresinde ÜÇ
# Kama kolu da 80/80 verdi — 20 s içinde ~400 RTT turu var, ARQ her şeyi kurtarıyor. Ayrım ancak
# deadline'ın izin verdiği RTT turu sayısı kayıp-kurtarma için YETMEDİĞİNDE doğar. Bu yüzden:
# yüksek gecikme (tur pahalı) + yüksek kayıp (çok tur gerek) + dar deadline (az tur var).
#
#   iid50   — bağımsız %50 kayıp, 200 ms: kodlamanın kazanması BEKLENEN rejim.
#   burst50 — netem korelasyonlu kayıp (%50, %50 korelasyon): yapı-gate'in kapandığı rejim.
run_cell() {
  local cell="$1"; local fwd="$2"; local rev="$3"
  echo "=== HÜCRE $cell : fwd='$fwd' rev='$rev' ==="
  kapi3_netem "$fwd" "$rev"
  run_arm "$cell" "quinn"                  quinn
  run_arm "$cell" "kp-sevk"                kama-product
  run_arm "$cell" "kp-static-r4"           kama-product --fec-profile 3 --fec-r 4
  # [IW] Kuruluş kaldıracı: ilk uçuşta kaç ORİJİNAL var. Sevk davranışında 1 paket; 2400 B
  # bütçe ~8 paket (her biri Karn-uygun → bağımsız RTT-örneği şansı).
  run_arm "$cell" "kp-iw2400"              kama-product --initial-window-bytes 2400
  # [DR-CS-3] Kuruluşu bariyersiz RTT örneğiyle çözen kol (CsProbe, Karn'ı hiç görmez).
  run_arm "$cell" "kp-bootstrap"           kama-product --bootstrap-rtt
  # İkisi birlikte + FEC: tam ürün.
  run_arm "$cell" "kp-iw-bs"               kama-product --initial-window-bytes 2400 --bootstrap-rtt
  run_arm "$cell" "kp-iw-bs-static"        kama-product --initial-window-bytes 2400 --bootstrap-rtt --fec-profile 3 --fec-r 4
}

echo "== PRODUCT-VS-QUINN  reps=$REPS msgs=$MSGS payload=${PAYLOAD}B deadline=${DEADLINE}s interval=${INTERVAL}ms =="
run_cell iid50   "delay 200ms loss 50%"     "delay 200ms loss 10%"
run_cell burst50 "delay 200ms loss 50% 50%" "delay 200ms loss 10%"

echo "== HAM SAYILAR (teslim = out.jsonl'de delivered_ns!=null olan event) =="
for f in "$OUT"/*/*/rep*/out.jsonl; do
  [ -f "$f" ] || continue
  d=$(grep -c '"delivered_ns":[0-9]' "$f" 2>/dev/null || echo 0)
  echo "$f delivered=$d/$MSGS"
done
echo "BITTI → $OUT"
