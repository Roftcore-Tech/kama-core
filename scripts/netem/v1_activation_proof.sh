#!/usr/bin/env bash
# [DENETİM + FAZ-4 §14/§M5] v1 AKTİVASYON KANITI.
#
# Ne kanıtlar: bayraksız (= kanonik `KamaV1Profile`) ürün yolu gerçek netem altında
# config→öneri→arbiter→encoder→TEL zincirini fiilen tamamlıyor mu, VE koşu yapılandırma +
# aktivasyon manifestlerini üretiyor mu.
#
# Ne KANITLAMAZ: performans. Hiçbir hız/completion iddiası bu betikten çıkarılamaz.
# İleri ve dönüş yolu bozulmaları AYRI verilir ve manifeste AYRI kaydedilir (§10/§M5).
set -u
REPO="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
BENCH="$REPO/target/release/bench"
OUT="${1:?kullanım: v1_activation_proof.sh OUTDIR}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=9100
MANIFESTS=()
run() { # $1=etiket $2=fwd $3=rev $4..=ek argümanlar
  local tag="$1" fwd="$2" rev="$3"; shift 3
  local d="$OUT/$tag"; mkdir -p "$d"; P=$((P+1))
  kapi3_netem "$fwd" "$rev"
  local c="--transport kama-product --payload 256 $*"
  printf 'tag=%s fwd=[%s] rev=[%s] args=[%s]\n' "$tag" "$fwd" "$rev" "$*" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # [§10/§M5] İleri ve dönüş bozulmaları AYRI bayraklarla manifeste geçer — tek "network=bad"
  # alanı YOK. [§14] Koşu yapılandırma + aktivasyon manifestini JSON olarak yazar.
  # shellcheck disable=SC2086
  timeout 120 "$BENCH" client --server "10.10.1.1:$P" $c --msgs 200 --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" \
    --manifest "$d/manifest.json" --fwd-impair "$fwd" --rev-impair "$rev" \
    2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
  echo "== $tag"; grep -h "KP-MANIFEST role=client" "$d/c.err" | head -3
  grep -h "KP-ACTIVATED\|KP-ACTIVATION" "$d/c.err" | head -1
  [ -f "$d/manifest.json" ] && MANIFESTS+=("$d/manifest.json")
}
run clean   "delay 100ms rate 1mbit"              "delay 100ms rate 1mbit"
run loss20  "delay 100ms loss 20% rate 1mbit"     "delay 100ms loss 2% rate 1mbit"
run burst50 "delay 100ms loss 50% 50% rate 1mbit" "delay 100ms loss 10% rate 1mbit"

# [§16/§17] Kıyas-adaleti denetimi. Bu üç koşu FARKLI ağlarda koştu → `BLOCKING` beklenir;
# amaç aracın farkı GÖRDÜĞÜNÜ kanıtlamak (sessiz karşılaştırma imkânsız).
if [ "${#MANIFESTS[@]}" -ge 2 ]; then
  echo "== FAIRNESS (farklı ağlar → BLOCKING beklenir)"
  "$BENCH" fairness --manifests "${MANIFESTS[@]}" || true
fi
echo "== BİTTİ"
