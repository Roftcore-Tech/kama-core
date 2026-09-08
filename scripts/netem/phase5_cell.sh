#!/usr/bin/env bash
# [FAZ-5] TEK HÜCRE KOŞUCUSU — bir T-hücresini kendi izole 3-netns'inde koşar.
#
# Ön-kayıt: reports/transport-v1/phase5/EXPECTATION.md (donmuş; bu betik ondan SAPAMAZ).
# Bozulma dizeleri EXPECTATION §3'ten VERBATIM gelir; burada yeni model TANIMLANMAZ.
#
# Kullanım: phase5_cell.sh CELL OUTDIR REPS DEADLINE_S [ARMS...]
#   CELL      — EXPECTATION §3 hücre kimliği (T0, T1a, ... T7, T3b)
#   REPS      — kol başına tekrar sayısı
#   DEADLINE_S— ön-kayıtlı deadline (kıyas hücrelerinde 20)
#   ARMS      — verilmezse: kama-product quinn tcp
#
# Çıktı (tekrar başına): <out>/<cell>/<arm>/rep<N>/{out.jsonl, manifest.json, c.err, s.err, rc}
# Hiçbir koşu sessizce yeniden denenmez; rc dosyası her koşunun çıkış kodunu taşır.
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
BENCH="$REPO/target/release/bench"
CELL="${1:?kullanım: phase5_cell.sh CELL OUTDIR REPS DEADLINE_S [ARMS...]}"
OUT="${2:?OUTDIR gerekli}"; REPS="${3:?REPS gerekli}"; DEADLINE="${4:?DEADLINE_S gerekli}"
shift 4
ARMS=("$@"); [ ${#ARMS[@]} -eq 0 ] && ARMS=(kama-product quinn tcp)
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }

# ── HÜCRE KATALOĞU — EXPECTATION.md §3 ile BİREBİR (kaynak dosya:satır yorumda) ────────────
case "$CELL" in
  T0)  FWD="delay 10ms loss 1% rate 10mbit";   REV="$FWD";                       MSGS=100; PAY=256;  IVAL=50   ;; # h6_rtt20  kapi_manifest.py:87
  T1a) FWD="loss 50% rate 100kbit";            REV="$FWD";                       MSGS=100; PAY=256;  IVAL=50   ;; # p1_loss50 kapi_manifest.py:41
  T1b) FWD="loss 80% rate 200kbit";            REV="$FWD";                       MSGS=100; PAY=256;  IVAL=50   ;; # p2_loss80 kapi_manifest.py:43
  T1c) FWD="delay 200ms loss 50%";             REV="delay 200ms loss 10%";       MSGS=100; PAY=256;  IVAL=50   ;; # iid50     product_vs_quinn.sh:88
  T2a) FWD="loss gemodel 50% 25% 90% 10% rate 250kbit"; REV="rate 250kbit";      MSGS=100; PAY=256;  IVAL=50   ;; # h3b       kapi_manifest.py:71
  T2b) FWD="delay 200ms loss 50% 50%";         REV="delay 200ms loss 10%";       MSGS=100; PAY=256;  IVAL=50   ;; # burst50   product_vs_quinn.sh:89
  T3a) FWD="delay 300ms 200ms distribution normal loss 25% rate 8kbit"; REV="$FWD"; MSGS=30; PAY=64; IVAL=50   ;; # h2_cell   kapi_manifest.py:65
  T3b) FWD="rate 0.5kbit";                     REV="$FWD";                       MSGS=20;  PAY=32;  IVAL=1000 ;; # p9_l0     kapi_manifest.py:104
  T4)  FWD="delay 100ms loss 1% rate 10mbit";  REV="$FWD";                       MSGS=100; PAY=256;  IVAL=50   ;; # h6_rtt200 kapi_manifest.py:89
  T5)  FWD="delay 100ms rate 1mbit";           REV="delay 100ms rate 64kbit";    MSGS=100; PAY=256;  IVAL=50   ;; # R3        r_returnpath_proof.sh:27
  T6)  FWD="loss 5% rate 100kbit";             REV="loss 80% rate 100kbit";      MSGS=100; PAY=256;  IVAL=50   ;; # p5_revpath kapi_manifest.py:51
  T7)  FWD="delay 100ms 20ms loss 20% reorder 30% 50% rate 1mbit"; REV="delay 100ms loss 2% rate 1mbit"; MSGS=100; PAY=256; IVAL=50 ;; # C4 fec_persist_family.sh:26
  *) echo "bilinmeyen hücre: $CELL"; exit 2 ;;
esac

if [ -z "${P5_INNER:-}" ]; then
  mkdir -p "$OUT/$CELL"
  P5_INNER=1 exec unshare -rnm bash "$0" "$CELL" "$OUT" "$REPS" "$DEADLINE" "${ARMS[@]}"
fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
kapi3_netem "$FWD" "$REV"

# [BARRIER] Bozulma GERÇEKTEN kurulu mu? Kurulmadıysa koşu ölçüm DEĞİLDİR (infra arızası).
for dev_dir in "mb0:ileri" "ma0:dönüş"; do
  dev="${dev_dir%%:*}"; label="${dev_dir##*:}"
  nsenter -t "$MID" -n tc qdisc show dev "$dev" | grep -q netem || {
    echo "BARRIER-FAIL: $label yön netem kurulmamış ($dev)" | tee "$OUT/$CELL/BARRIER-FAIL.txt"; exit 4; }
done
nsenter -t "$MID" -n tc qdisc show dev mb0 > "$OUT/$CELL/realized_fwd.txt"
nsenter -t "$MID" -n tc qdisc show dev ma0 > "$OUT/$CELL/realized_rev.txt"
cat > "$OUT/$CELL/cell.json" <<EOF
{"cell":"$CELL","fwd":"$FWD","rev":"$REV","msgs":$MSGS,"payload":$PAY,
 "submit_interval_ms":$IVAL,"deadline_s":$DEADLINE,"reps":$REPS,
 "arms":"$(echo "${ARMS[@]}")","commit":"$(git -C "$REPO" rev-parse HEAD)"}
EOF

PORT=$(( 20000 + (RANDOM % 20000) ))
SRV_IP=10.10.1.1
NARMS=${#ARMS[@]}

run_one() { # $1=arm $2=rep
  local arm="$1" rep="$2"
  local d="$OUT/$CELL/$arm/rep$rep"; mkdir -p "$d"; PORT=$((PORT+1))
  local srv_extra=""
  [ "$arm" = "kama-product" ] && srv_extra="--payload $PAY"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "$SRV_IP:$PORT" --transport "$arm" $srv_extra \
    2>"$d/s.err" >/dev/null &
  local SRV=$!
  sleep 1
  local budget=$(( MSGS * IVAL / 1000 + DEADLINE + 45 ))
  timeout "$budget" "$BENCH" client --server "$SRV_IP:$PORT" --transport "$arm" \
    --msgs "$MSGS" --payload "$PAY" --submit-interval-ms "$IVAL" --max-deadline-s "$DEADLINE" \
    --seed "$rep" --repeat "$rep" \
    --fwd-impair "$FWD" --rev-impair "$REV" \
    --manifest "$d/manifest.json" --out "$d/out.jsonl" \
    2>"$d/c.err" >/dev/null
  echo "$?" > "$d/rc"
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
}

echo "== HÜCRE $CELL  fwd='$FWD'  rev='$REV'  geo=${MSGS}x${PAY}B@${IVAL}ms  D=${DEADLINE}s  reps=$REPS =="
for rep in $(seq 1 "$REPS"); do
  # [KONUM YANLILIĞI] Kol sırası her tekrarda DÖNDÜRÜLÜR: hiçbir kol sistematik olarak
  # "ısınmış makine" ya da "taze netem kuyruğu" pozisyonunda kalmaz.
  for k in $(seq 0 $((NARMS-1))); do
    idx=$(( (k + rep) % NARMS ))
    run_one "${ARMS[$idx]}" "$rep"
  done
  printf '.'
done
echo " bitti"
echo "OK $CELL" > "$OUT/$CELL/DONE"
