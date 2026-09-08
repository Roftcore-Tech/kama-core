#!/usr/bin/env bash
# [DENETİM] v1 AKTİVASYON KANITI: ürün profili (bayraksız = kama-v1-full) gerçek netem'de
# config→öneri→arbiter→encoder→TEL zincirini fiilen tamamlıyor mu? Performans ölçmez; ATEŞLEME kanıtlar.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: v1_activation_proof.sh OUTDIR}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=9100
run() { # $1=etiket $2=fwd $3=rev $4..=ek argümanlar
  local tag="$1" fwd="$2" rev="$3"; shift 3
  local d="$OUT/$tag"; mkdir -p "$d"; P=$((P+1))
  kapi3_netem "$fwd" "$rev"
  local c="--transport kama-product --payload 256 $*"
  printf 'tag=%s fwd=[%s] rev=[%s] args=[%s]\n' "$tag" "$fwd" "$rev" "$*" > "$d/argv.txt"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  # shellcheck disable=SC2086
  timeout 120 "$BENCH" client --server "10.10.1.1:$P" $c --msgs 200 --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
  echo "== $tag"; grep -h "KP-MANIFEST role=client" "$d/c.err" | head -3
  grep -h "KP-ACTIVATED" "$d/c.err" | head -1
}
run clean   "delay 100ms rate 1mbit"            "delay 100ms rate 1mbit"
run loss20  "delay 100ms loss 20% rate 1mbit"   "delay 100ms loss 2% rate 1mbit"
run burst50 "delay 100ms loss 50% 50% rate 1mbit" "delay 100ms loss 10% rate 1mbit"
echo "== BİTTİ"
