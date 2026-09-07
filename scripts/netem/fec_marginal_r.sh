#!/usr/bin/env bash
# [FEC-MARGINAL-R-V1] Ön-kayıtlı koşucu. Sözleşme: reports/fec-marginal-r-v1/PRE-REGISTRATION.md
#
# Tasarım kuralları koda GÖMÜLÜDÜR, yoruma bırakılmaz:
#  · kollar TEK eksende ayrışır (`--fec-r`); mod/profil/yük/zamanlama özdeş
#  · rep sırası ARALIKLI → herhangi bir kesilme DENGELİ tasarım bırakır
#  · HER rep bir kayıt üretir (rep_status), timeout sessizce yutulmaz
#  · deadline burada SÜPÜRÜLMEZ; ham gecikmeler out.jsonl'de, eğri post-hoc
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
OUT="${1:?kullanım: fec_marginal_r.sh OUTDIR [REPS]}"; REPS="${2:-20}"
BENCH="$REPO/target/release/bench"
[ -x "$BENCH" ] || { echo "önce: cargo build --release -p bench --bin bench"; exit 3; }

# ── Ön-kayıtlı parametreler (§4, §3) ──────────────────────────────────────────────────────────
MSGS=96; PAYLOAD=256; INTERVAL_MS=20; D_RUN_GRACE_S=20; PROFILE=3
OWD_MS=100; FWD_LOSS=20; REV_LOSS=2; RATE=1mbit
ARMS="${ARMS_OVERRIDE:-0 2 4}"   # --fec-r; mod hepsinde static (v2: "0 2")
CELLS="iid20 cor20"

if [ -z "${FMR_INNER:-}" ]; then
  mkdir -p "$OUT"
  FMR_INNER=1 exec unshare -rnm bash "$0" "$OUT" "$REPS"
fi
source "$DIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup
PORT=5500; SRV_IP=10.10.1.1

cell_netem() {
  case "$1" in
    iid20) kapi3_netem "delay ${OWD_MS}ms loss ${FWD_LOSS}% rate ${RATE}" \
                       "delay ${OWD_MS}ms loss ${REV_LOSS}% rate ${RATE}" ;;
    cor20) kapi3_netem "delay ${OWD_MS}ms loss ${FWD_LOSS}% 25% rate ${RATE}" \
                       "delay ${OWD_MS}ms loss ${REV_LOSS}% rate ${RATE}" ;;
  esac
}

# $1=dizin $2=r
run_rep() {
  local d="$1" r="$2"
  mkdir -p "$d"; PORT=$((PORT+1))
  local common="--transport kama-product --fec-mode static --fec-profile $PROFILE --fec-r $r"
  # Argümanları kaydet: kol farklılaşması sonradan DENETLENEBİLİR olsun.
  printf 'r=%s port=%s common=%s\n' "$r" "$PORT" "$common" > "$d/argv.txt"
  echo "STARTED" > "$d/rep_status"
  # shellcheck disable=SC2086
  nsenter -t "$CHLB" -n "$BENCH" serve --bind "$SRV_IP:$PORT" $common --payload $PAYLOAD \
    2>"$d/s.err" >/dev/null &
  local SRV=$!; sleep 1
  # shellcheck disable=SC2086
  timeout $((D_RUN_GRACE_S + 40)) "$BENCH" client --server "$SRV_IP:$PORT" $common \
    --msgs $MSGS --payload $PAYLOAD --submit-interval-ms $INTERVAL_MS \
    --max-deadline-s $D_RUN_GRACE_S --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
  local rc=$?
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
  # HER rep bir durum kaydı üretir — timeout'lu rep sessizce KAYBOLMAZ (out.jsonl atomik yazılır,
  # yarıda ölen client hiç dosya bırakmaz; `|| true` ile yutulsaydı kol-bağımlı sessiz kayıp olurdu).
  if [ "$rc" -eq 0 ] && [ -s "$d/out.jsonl" ]; then echo "COMPLETED" > "$d/rep_status"
  elif [ "$rc" -eq 124 ]; then echo "TIMEOUT" > "$d/rep_status"
  else echo "FAILED_rc=$rc" > "$d/rep_status"; fi
}

echo "== FEC-MARGINAL-R-V1  reps=$REPS msgs=$MSGS payload=${PAYLOAD}B interval=${INTERVAL_MS}ms =="
echo "== hücreler: $CELLS   kollar(r): $ARMS   RTT=$((2*OWD_MS))ms rate=$RATE =="
START=$(date +%s)
# ARALIKLI sıra: rep indeksi dış döngü → kesilme dengeli tasarım bırakır (§9).
for rep in $(seq 1 "$REPS"); do
  for cell in $CELLS; do
    cell_netem "$cell"
    for r in $ARMS; do
      run_rep "$OUT/$cell/r$r/rep$rep" "$r"
    done
  done
  echo "  rep $rep/$REPS bitti (t=$(( $(date +%s) - START ))s)"
done
echo "BITTI → $OUT  (toplam $(( $(date +%s) - START ))s)"
