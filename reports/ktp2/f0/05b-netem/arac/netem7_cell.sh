#!/usr/bin/env bash
# [K4G-05b NETEM-7] Tek profil koşucusu — ADR-18 7 hostile profil (p1..p7), ürün kolu
# (`--transport kama-product`), İKİ ikili (taban / aday) aynı netns'te dönüşümlü.
#
# Türetildiği yer: scripts/netem/phase5_cell.sh (çalıştırma deseni, barrier, rc dosyası,
# kol sırası döndürme) + scripts/netem/kapi3_harness.sh (3-netns, kapi3_trace).
# Profil dizeleri scripts/netem/kapi_manifest.py:40-55'ten VERBATIM; p6 pencere planı :110.
# Yeni bozulma modeli TANIMLANMAZ. Ön-kayıt: ~/kama-k4g-work/k4g05b/netem7/ONKAYIT.md.
#
# Kullanım: netem7_cell.sh PROFILE OUTDIR REPS HARNESS_DIR BIN_TABAN BIN_ADAY
#   HARNESS_DIR — kapi3_harness.sh'nin bulunduğu dizin (taban worktree'nin scripts/netem'i)
# Çıktı: <out>/<profile>/{taban,aday}/rep<N>/{out.jsonl, manifest.json, c.err, s.err, rc}
set -u
PROFILE="${1:?kullanım: netem7_cell.sh PROFILE OUTDIR REPS HARNESS_DIR BIN_TABAN BIN_ADAY}"
OUT="${2:?}"; REPS="${3:?}"; HDIR="${4:?}"; BIN_TABAN="${5:?}"; BIN_ADAY="${6:?}"
for b in "$BIN_TABAN" "$BIN_ADAY"; do [ -x "$b" ] || { echo "ikili yok: $b"; exit 3; }; done

# ── PROFİL KATALOĞU — kapi_manifest.py ile BİREBİR ─────────────────────────────────────────
# Geometri: p6 dışında phase5 sözleşme geometrisi (100×256 B @50 ms, D=20 s; EXPECTATION §3).
# p6: kesinti penceresi 30–60 s; 50 ms aralıkla sunum 5 s'de biter ve pencere hiç örtüşmez →
# kapi CLASS_PLAN "I" paced aralığı (1000 ms) ile sunum 0–99 s, pencereyle örtüşür.
TRACE=""
case "$PROFILE" in
  p1_loss50)   FWD="loss 50% rate 100kbit"; REV="$FWD";                        IVAL=50   ;; # :40
  p2_loss80)   FWD="loss 80% rate 200kbit"; REV="$FWD";                        IVAL=50   ;; # :42
  p3_loss95)   FWD="loss 95% rate 100kbit"; REV="$FWD";                        IVAL=50   ;; # :44
  p4_jitter)   FWD="delay 200ms 100ms distribution normal rate 100kbit"; REV="$FWD"; IVAL=50 ;; # :46
  p5_revpath)  FWD="loss 5% rate 100kbit";  REV="loss 80% rate 100kbit";       IVAL=50   ;; # :49
  p6_blackout) FWD="trace"; REV="trace"; TRACE="0 none|none
30 loss 100%|loss 100%
60 none|none";                                                                  IVAL=1000 ;; # :52,:110
  p7_bbr)      FWD="loss 2% rate 50kbit";   REV="$FWD";                        IVAL=50   ;; # :54
  *) echo "bilinmeyen profil: $PROFILE"; exit 2 ;;
esac
MSGS=100; PAY=256; DEADLINE=20

if [ -z "${N7_INNER:-}" ]; then
  mkdir -p "$OUT/$PROFILE"
  N7_INNER=1 exec unshare -rnm bash "$0" "$PROFILE" "$OUT" "$REPS" "$HDIR" "$BIN_TABAN" "$BIN_ADAY"
fi
# shellcheck disable=SC1091
source "$HDIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup

# p6 dışında bozulma hücre boyunca sabit; p6'da her tekrar kendi pencere planını baştan oynatır
# ("trace" literalinin tc'ye sızması YASAK — kapi_calrun.sh deseni). Plan VERBATIM: "none" =
# qdisc yok (kapi_manifest.py'deki cap_fwd yalnız bütçe türetimi içindir; tc'ye girmez).
if [ -z "$TRACE" ]; then
  kapi3_netem "$FWD" "$REV"
  for dev_dir in "mb0:ileri" "ma0:dönüş"; do
    dev="${dev_dir%%:*}"; label="${dev_dir##*:}"
    nsenter -t "$MID" -n tc qdisc show dev "$dev" | grep -q netem || {
      echo "BARRIER-FAIL: $label yön netem kurulmamış ($dev)" | tee "$OUT/$PROFILE/BARRIER-FAIL.txt"; exit 4; }
  done
  nsenter -t "$MID" -n tc qdisc show dev mb0 > "$OUT/$PROFILE/realized_fwd.txt"
  nsenter -t "$MID" -n tc qdisc show dev ma0 > "$OUT/$PROFILE/realized_rev.txt"
fi
cat > "$OUT/$PROFILE/cell.json" <<EOF
{"profile":"$PROFILE","fwd":"$FWD","rev":"$REV","msgs":$MSGS,"payload":$PAY,
 "submit_interval_ms":$IVAL,"deadline_s":$DEADLINE,"reps":$REPS,
 "bin_taban":"$BIN_TABAN","bin_aday":"$BIN_ADAY",
 "sha_taban":"$(git -C "$(dirname "$BIN_TABAN")/../.." rev-parse HEAD)",
 "sha_aday":"$(git -C "$(dirname "$BIN_ADAY")/../.." rev-parse HEAD)"}
EOF

PORT=$(( 20000 + (RANDOM % 20000) ))
SRV_IP=10.10.1.1

run_one() { # $1=kol(taban|aday) $2=rep
  local arm="$1" rep="$2" bin
  [ "$arm" = "taban" ] && bin="$BIN_TABAN" || bin="$BIN_ADAY"
  local d="$OUT/$PROFILE/$arm/rep$rep"; mkdir -p "$d"; PORT=$((PORT+1))
  local fwd_meta="$FWD" rev_meta="$REV"
  if [ -n "$TRACE" ]; then
    kapi3_netem none none
    fwd_meta="trace:p6_blackout"; rev_meta="trace:p6_blackout"
  fi
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$bin" serve --bind "$SRV_IP:$PORT" --transport kama-product --payload "$PAY" \
    2>"$d/s.err" >/dev/null &
  local SRV=$!
  sleep 1
  if [ -n "$TRACE" ]; then
    # Pencere planı istemcinin başlangıcına göre: t0 = istemci başlatılmadan hemen önce.
    printf '%s\n' "$TRACE" > "$d/trace.plan"
    kapi3_trace "$d/trace.plan"
  fi
  local budget=$(( MSGS * IVAL / 1000 + DEADLINE + 45 ))
  timeout "$budget" "$bin" client --server "$SRV_IP:$PORT" --transport kama-product \
    --msgs "$MSGS" --payload "$PAY" --submit-interval-ms "$IVAL" --max-deadline-s "$DEADLINE" \
    --seed "$rep" --repeat "$rep" \
    --fwd-impair "$fwd_meta" --rev-impair "$rev_meta" \
    --manifest "$d/manifest.json" --out "$d/out.jsonl" \
    2>"$d/c.err" >/dev/null
  echo "$?" > "$d/rc"
  [ -n "$TRACE" ] && kapi3_trace_stop
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
}

echo "== PROFİL $PROFILE  fwd='$FWD'  rev='$REV'  geo=${MSGS}x${PAY}B@${IVAL}ms  D=${DEADLINE}s  reps=$REPS =="
ARMS=(taban aday)
for rep in $(seq 1 "$REPS"); do
  # Kol sırası her tekrarda döndürülür (phase5_cell.sh konum yanlılığı kuralı).
  for k in 0 1; do
    idx=$(( (k + rep) % 2 ))
    run_one "${ARMS[$idx]}" "$rep"
  done
  printf '.'
done
echo " bitti"
echo "OK $PROFILE" > "$OUT/$PROFILE/DONE"
