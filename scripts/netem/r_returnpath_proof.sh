#!/usr/bin/env bash
# [FAZ-1] Dönüş-yolu ayrım kanıtı: ileri bozulma ≠ dönüş bozulması, ÜRÜN runtime durumunda.
set -u
REPO=/home/ryuzaki/Desktop/kama-core; BENCH=$REPO/target/release/bench
OUT="${1:?kullanım: r_returnpath_proof.sh OUTDIR}"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }
if [ -z "${INNER:-}" ]; then mkdir -p "$OUT"; INNER=1 exec unshare -rnm bash "$0" "$OUT"; fi
source "$REPO/scripts/netem/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
P=9300
run() { # $1=etiket $2=fwd $3=rev
  local tag="$1" fwd="$2" rev="$3"; local d="$OUT/$tag"; mkdir -p "$d"; P=$((P+1))
  kapi3_netem "$fwd" "$rev"
  local c="--transport kama-product --payload 256"
  printf 'tag=%s fwd=[%s] rev=[%s]\n' "$tag" "$fwd" "$rev" > "$d/argv.txt"
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$P" $c 2>"$d/s.err" >/dev/null &
  local sp=$!; sleep 1
  timeout 150 "$BENCH" client --server "10.10.1.1:$P" $c --msgs 300 --submit-interval-ms 20 \
    --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
  printf '%-4s ' "$tag"; grep -o "return_path=\[[^]]*\]" "$d/c.err" | tail -1
}
run R0 "delay 100ms rate 1mbit"           "delay 100ms rate 1mbit"
run R1 "delay 100ms loss 20% rate 1mbit"  "delay 100ms rate 1mbit"
run R2 "delay 100ms rate 1mbit"           "delay 100ms loss 40% rate 1mbit"
run R3 "delay 100ms rate 1mbit"           "delay 100ms rate 64kbit"
run R4 "delay 20ms rate 1mbit"            "delay 200ms rate 1mbit"
run R5 "delay 100ms rate 1mbit"           "delay 100ms loss 95% rate 1mbit"
echo "== BİTTİ"
