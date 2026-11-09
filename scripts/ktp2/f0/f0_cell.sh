#!/usr/bin/env bash
# [KTP2-F0] Ortak hücre koşucusu — iki süreç, 3-netns faithful netem (kapi3_harness.sh), N kol
# aynı netns'te ve her tekrarda kol sırası döndürülerek.
#
# Türetildiği yer: ~/kama-k4g-work/tools/netem7_cell.sh (K4G-05b netem-7; 2 oturumda smoke + 560
# koşu) ← scripts/netem/phase5_cell.sh deseni. Yeni bozulma modeli YOK: netem dizeleri hücre
# dosyasından (cells/<şerit>/*.cell) aynen `tc qdisc replace … netem <dize>`'ye gider.
#
# Kullanım: f0_cell.sh HÜCRE_DOSYASI ÇIKTI_KÖKÜ
#   HÜCRE_DOSYASI (bash; source edilir — yalnız depodaki hücre dosyaları):
#     CELL=<ad>  FWD="<netem>"  REV="<netem>"  (ya da FWD=trace REV=trace TRACE="<plan>")
#     REPS=<N>  ARMS="<kol1> <kol2> …"  BUDGET_S=<koşu başına üst sınır s>  CAPTURE=0|1
#     ARM_<kol>_SRV='<sunucu komutu>'  ARM_<kol>_CLI='<istemci komutu>'
#     (isteğe bağlı) SETTLE_S=<sunucu açılış beklemesi, varsayılan 1>
#   Yer tutucular (komut dizelerinde): @SRV@ = sunucu ip:port · @SRV_IP@ · @PORT@ · @CLI_IP@ ·
#     @D@ = koşu dizini · @REP@ = tekrar numarası · @BIN_<AD>@ = ortam değişkeni BIN_<AD>
#     (ör. @BIN_TABAN@ → $BIN_TABAN). Sunucu ns-B'de (10.10.1.1), istemci ns-A'da (10.10.0.1).
#   Ortam: HDIR = kapi3_harness.sh dizini (varsayılan: bu deponun scripts/netem'i).
#
# Çıktı: <kök>/<CELL>/{cell.env, realized_fwd.txt, realized_rev.txt, DONE}
#        <kök>/<CELL>/<kol>/rep<N>/{rc, t_start, t_end, c.err, s.err, cmd.txt, realized_pre.json,
#                                    realized_post.json, cap/ (CAPTURE=1), + kolun kendi çıktıları}
# Kurallar: sessiz yeniden koşu YOK (rc dosyası her koşuda yazılır, düşen koşu düşük kalır);
# kol sırası tekrar başına döner (konum yanlılığı); netem hücre boyunca sabit (trace hariç).
set -u
SPEC="${1:?kullanım: f0_cell.sh HÜCRE_DOSYASI ÇIKTI_KÖKÜ}"; ROOT="${2:?}"
SPEC="$(readlink -f "$SPEC")"; ROOT="$(readlink -f "$ROOT")"
SELF="$(readlink -f "$0")"
HDIR="${HDIR:-$(cd "$(dirname "$SELF")/../../netem" && pwd)}"

# shellcheck disable=SC1090
source "$SPEC"
: "${CELL:?CELL yok}" "${FWD:?FWD yok}" "${REV:?REV yok}" "${REPS:?REPS yok}" "${ARMS:?ARMS yok}" "${BUDGET_S:?BUDGET_S yok}"
TRACE="${TRACE:-}"; CAPTURE="${CAPTURE:-0}"; SETTLE_S="${SETTLE_S:-1}"
OUT="$ROOT/$CELL"

if [ -z "${F0_INNER:-}" ]; then
  mkdir -p "$OUT"
  [ -e "$OUT/DONE" ] && { echo "ATLA $CELL (DONE var)"; exit 0; }
  F0_INNER=1 exec unshare -rnm bash "$SELF" "$SPEC" "$ROOT"
fi
# shellcheck disable=SC1091
source "$HDIR/kapi3_harness.sh"
trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT
kapi3_setup

if [ -z "$TRACE" ]; then
  kapi3_netem "$FWD" "$REV"
  for dev_dir in "mb0:ileri" "ma0:dönüş"; do
    dev="${dev_dir%%:*}"; label="${dev_dir##*:}"
    nsenter -t "$MID" -n tc qdisc show dev "$dev" | grep -q netem || {
      echo "BARRIER-FAIL: $label yön netem kurulmamış ($dev)" | tee "$OUT/BARRIER-FAIL.txt"; exit 4; }
  done
  nsenter -t "$MID" -n tc qdisc show dev mb0 > "$OUT/realized_fwd.txt"
  nsenter -t "$MID" -n tc qdisc show dev ma0 > "$OUT/realized_rev.txt"
fi
{
  echo "spec=$SPEC"
  echo "spec_sha256=$(sha256sum "$SPEC" | cut -d' ' -f1)"
  echo "runner_sha256=$(sha256sum "$SELF" | cut -d' ' -f1)"
  echo "harness_sha256=$(sha256sum "$HDIR/kapi3_harness.sh" | cut -d' ' -f1)"
  echo "kernel=$(uname -r)"
  env | grep -E '^BIN_[A-Z0-9_]+=' | sort | while IFS='=' read -r k v; do
    echo "$k=$v"; echo "${k}_sha256=$(sha256sum "$v" 2>/dev/null | cut -d' ' -f1)"
  done
} > "$OUT/cell.env"

SRV_IP=10.10.1.1
CLI_IP=10.10.0.1
PORT=$(( 20000 + (RANDOM % 20000) ))

subst() { # $1=şablon $2=koşu dizini $3=rep → yer tutucuları doldur
  local s="$1" d="$2" rep="$3" k v
  s="${s//@SRV@/$SRV_IP:$PORT}"; s="${s//@SRV_IP@/$SRV_IP}"; s="${s//@PORT@/$PORT}"
  s="${s//@CLI_IP@/$CLI_IP}"; s="${s//@D@/$d}"; s="${s//@REP@/$rep}"
  while IFS='=' read -r k v; do s="${s//@$k@/$v}"; done < <(env | grep -E '^BIN_[A-Z0-9_]+=')
  printf '%s' "$s"
}

run_one() { # $1=kol $2=rep
  local arm="$1" rep="$2"
  local d="$OUT/$arm/rep$rep"; mkdir -p "$d"; PORT=$((PORT+1))
  local srv_t cli_t srv_cmd cli_cmd
  srv_t="ARM_${arm}_SRV"; cli_t="ARM_${arm}_CLI"
  srv_cmd="$(subst "${!srv_t}" "$d" "$rep")"; cli_cmd="$(subst "${!cli_t}" "$d" "$rep")"
  printf 'SRV: %s\nCLI: %s\n' "$srv_cmd" "$cli_cmd" > "$d/cmd.txt"
  # [İSTEK-O-D1] Her koşu temiz kuyrukla başlar: önceki koşunun netem kuyruğunda/gecikme hattında
  # kalan paketleri düşürülür (qdisc silinip yeniden kurulur), sayaçlar sıfırlanır. Aksi hâlde
  # düşük hızda bir kolun artığı sonraki kolun önüne geçer (r25'te yüzlerce saniye).
  kapi3_netem none none
  [ -z "$TRACE" ] && kapi3_netem "$FWD" "$REV"
  kapi3_realized "$d" pre
  [ "$CAPTURE" = 1 ] && kapi3_capture_start "$d/cap"
  ( cd "$d" && exec nsenter -t "$CHLB" -n bash -c "exec $srv_cmd" ) 2>"$d/s.err" >"$d/s.out" &
  local SRV=$!
  sleep "$SETTLE_S"
  if [ -n "$TRACE" ]; then
    printf '%s\n' "$TRACE" > "$d/trace.plan"
    kapi3_trace "$d/trace.plan"
  fi
  date -u +%s.%N > "$d/t_start"
  ( cd "$d" && exec timeout "$BUDGET_S" bash -c "exec $cli_cmd" ) 2>"$d/c.err" >"$d/c.out"
  echo "$?" > "$d/rc"
  date -u +%s.%N > "$d/t_end"
  [ -n "$TRACE" ] && kapi3_trace_stop
  sleep 0.5
  kill "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true
  [ "$CAPTURE" = 1 ] && kapi3_capture_stop
  kapi3_realized "$d" post
}

read -r -a ARR <<< "$ARMS"
NA=${#ARR[@]}
echo "== HÜCRE $CELL  fwd='$FWD'  rev='$REV'  kollar=[$ARMS]  reps=$REPS  bütçe=${BUDGET_S}s =="
for rep in $(seq 1 "$REPS"); do
  for k in $(seq 0 $((NA - 1))); do
    idx=$(( (k + rep) % NA ))
    run_one "${ARR[$idx]}" "$rep"
  done
  printf '.'
done
echo " bitti"
echo "OK $CELL $(date -u +%FT%TZ)" > "$OUT/DONE"
