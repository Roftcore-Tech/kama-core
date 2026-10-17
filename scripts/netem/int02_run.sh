#!/usr/bin/env bash
# INT-02 netem sürücüsü (plan K4G-05; CORE-CLOSE kritik yolu).
#
# Her (tohum, kol) için AYRI bir ağ ad alanı (`unshare -rn`): lo up → yeni provisioning
# (`kama-provision init`, geçici dizin; root.json yalnız otorite dizininde kalır, D9) → netem
# `lo`'ya BİR kez (lo'da her paket her yönde netem'den BİR kez geçer — T5 ölçümü) → ÖLÇÜM ARACI
# DOĞRULAMASI (ping + qdisc sayaçları, koşudan ÖNCE) → `int02_node` örneği → qdisc sayaçları (koşudan
# SONRA) → tek JSON dosyası `<out>/seed-<S>-<kol>.json` (örneğin satırı + sürücü/netem kayıtları).
#
# PROFİL (sahibin metni birebir): `delay 200ms 200ms loss 30%` — netem sözdiziminde 200 ms gecikme
# ± 200 ms jitter, %30 kayıp; `seed <S>` netem'in rastgele üretecini sabitler. El sıkışma zarfları
# süreç-içi röleden netem DIŞINDA geçer (DUR-K4G-11 → B; beyanlı boşluk).
#
# KESİNTİ KOLU (`ek-kesinti`, NON-GATING) eşgüdümü: örnek `<work>/sync/KESINTI_BASLA` dosyasını yaratır;
# sürücü bunu görünce `tc qdisc change … netem loss 100%` uygular, `KESINTI_UYGULANDI` yaratır, 60 s
# bekler, profili (aynı tohumla) geri kurar ve `KESINTI_BITTI` yaratır. Anlar sürücü kaydına yazılır.
#
# PENCERE (`--window`, CEVAP PAKETİ 4): son gönderimden (ek-kesinti kolunda kesinti bitiminden) sonra
# tamamlanma üst sınırı — bütün kollar için TEK parametre (örneğin `--window-s`'i).
#
# ZAMAN ÇİZELGESİ (`--timeline-every-ms`, CEVAP PAKETİ 6): örneğin zaman çizelgesi aralığı (ms; motor
# alanları + ACK çerçevesi / durum değişikliği günlüğü `m_log`). Tavanı örnek parametrelerden türetir.
#
# Bu betik derleme YAPMAZ; ikilileri önceden derleyin (bkz. --help).
set -euo pipefail

SELF="$(readlink -f "$0")"
REPO="$(cd "$(dirname "$SELF")/../.." && pwd)"

PROFILE_NAME="d200j200l30"
NETEM_ARGS="delay 200ms 200ms loss 30%"
OUTAGE_ARGS="loss 100%"
OUTAGE_S=60
PORT_ALICE=40001
PORT_BOB=40002
PING_COUNT=20
PING_INTERVAL=0.2
PING_WAIT=2
# Kol başına süre bütçesi (s; `timeout`): örneğin kendi üst sınırlarının (connect 60 + gönderim +
# pencere 120 + sakinleşme 10) üstünde pay. `--window` / `--settle-s` / `--outage-wait-s` verilirse
# bütçeye eklenir.
BUDGET_DEFAULT=900
BUDGET_OUTAGE=1200

usage() {
  cat <<EOF
kullanım: scripts/netem/int02_run.sh [seçenekler]

  --seeds "S1 S2 …"     tohumlar (varsayılan: "1 2 3 4 5")
  --arms  "K1 K2 …"     kollar (varsayılan: "gating1 gating2 gating6 negatif")
                        NON-GATING: ek-dosya-ptt ek-kesinti
  --out DİZİN           çıktı (varsayılan: <repo>/results/int02/<YYYY-MM-DD>; results/ gitignore'da)
  --bin DİZİN           derleme çıktısı (varsayılan: <repo>/target/release)
  --example YOL         int02_node ikilisi (varsayılan: <bin>/examples/int02_node)
  --provision YOL       kama-provision ikilisi (varsayılan: <bin>/kama-provision)
  --count N             yön başına TEXT (örneğe iletilir; varsayılan örnekte 100)
  --interval-ms MS      TEXT aralığı (örneğe iletilir; varsayılan örnekte 100)
  --window S            pencere: son gönderimden (ek-kesinti kolunda kesinti bitiminden) sonra
                        tamamlanma üst sınırı (s), bütün kollar için tek parametre (örneğin
                        --window-s'i; verilmezse örneğin varsayılanı 120).
                        Not: negatif kol (kurtarma kapalı) tamamlanmaz, pencerenin tamamını bekler;
                        kısa pencereyle ayrı koşun (ör. --arms negatif --window 30).
  --outage-wait-s S     YALNIZ ek-kesinti: kesinti uygulandıktan sonra KESINTI_BITTI işaretinin
                        beklenme üst sınırı — pencere DEĞİLDİR (verilmezse örneğin varsayılanı 180)
  --settle-s S          tamamlanmadan sonra Transport sakinleşmesi üst sınırı (örneğe iletilir;
                        verilmezse örneğin varsayılanı 10)
  --timeline-every-ms MS
                        zaman çizelgesi aralığı (örneğe iletilir; verilmezse örneğin varsayılanı
                        5000; 0 = kapalı, kapalıyken m_log da tutulmaz)
  --keep-work           geçici çalışma dizinlerini silme
  --dry-run             yalnız planı ve komutları yaz, hiçbir şey koşma
  --check               önkoşulları denetle (ikililer, komutlar, sch_netem, unprivileged userns)
  --help

derleme (orkestratör):
  cargo build --release -p kama-integration --example int02_node
  cargo build --release -p kama-provision
tek tohum : scripts/netem/int02_run.sh --seeds 1 --arms gating1
5 tohum   : scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms "gating1 gating2 gating6"
negatif   : scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms negatif --window 30
yeniden koşu (CEVAP PAKETİ 4):
  scripts/netem/int02_run.sh --seeds "1 2 3 4 5 6 7 8 9 10" --arms gating2 --window 600
  scripts/netem/int02_run.sh --seeds "1 2 3 4 5" --arms "ek-kesinti ek-dosya-ptt" --window 600
M1/M2 teşhis ölçümü (CEVAP PAKETİ 6; beklenti kaydı docs/reports/INT-02-M1M2-BEKLENTI.md):
  scripts/netem/int02_run.sh --seeds "3 9" --arms gating2 --window 600 --timeline-every-ms 1000 --out <out>
  scripts/netem/int02_run.sh --seeds "1 3" --arms ek-kesinti --window 600 --timeline-every-ms 1000 --out <out>
analiz    : python3 scripts/netem/int02_analyze.py <out>
EOF
}

die() { echo "int02_run: $*" >&2; exit 2; }

# ── İç kip: tek (tohum, kol), ağ ad alanının İÇİNDE ─────────────────────────────────────────
inner() {
  local seed="$1" arm="$2" work="$3" outfile="$4"
  local example="$INT02_EXAMPLE" provision="$INT02_PROVISION"
  local t_start; t_start="$(date +%s.%N)"
  ip link set lo up

  # Provisioning: her (tohum, kol) için YENİ kimlikler. root.json otorite dizininde kalır (D9).
  "$provision" init --out "$work/authority" --devices alice,bob \
    --direct "alice=127.0.0.1:$PORT_ALICE,bob=127.0.0.1:$PORT_BOB" >"$work/provision.log" 2>&1
  mkdir -p "$work/alice" "$work/bob" "$work/sync"
  cp "$work/authority/device-alice.json" "$work/alice/"
  cp "$work/authority/device-bob.json" "$work/bob/"
  if [ -e "$work/alice/root.json" ] || [ -e "$work/bob/root.json" ]; then
    echo "int02_run: D9 ihlali: root.json cihaz dizininde" >&2
    exit 3
  fi

  # netem: lo'ya BİR kez (her paket her yönde bir kez görür).
  # shellcheck disable=SC2086
  tc qdisc add dev lo root netem $NETEM_ARGS seed "$seed"
  tc -s qdisc show dev lo >"$work/qdisc_initial.txt"

  # Ölçüm aracı doğrulaması (koşudan ÖNCE): netem ateşliyor mu — ping RTT ve kaybı.
  ping -n -c "$PING_COUNT" -i "$PING_INTERVAL" -W "$PING_WAIT" 127.0.0.1 >"$work/ping.txt" 2>&1 || true
  tc -s qdisc show dev lo >"$work/qdisc_before.txt"

  local args=(
    --arm "$arm" --seed "$seed" --profile "$PROFILE_NAME"
    --alice-config "$work/alice/device-alice.json" --bob-config "$work/bob/device-bob.json"
    --alice-data "$work/alice/data" --bob-data "$work/bob/data" --scratch-dir "$work"
  )
  if [ -n "${INT02_COUNT:-}" ]; then args+=(--count "$INT02_COUNT"); fi
  if [ -n "${INT02_INTERVAL_MS:-}" ]; then args+=(--interval-ms "$INT02_INTERVAL_MS"); fi
  if [ -n "${INT02_SETTLE_S:-}" ]; then args+=(--settle-s "$INT02_SETTLE_S"); fi
  # PENCERE: bütün kollara aynı parametre.
  if [ -n "${INT02_WINDOW_S:-}" ]; then args+=(--window-s "$INT02_WINDOW_S"); fi
  if [ -n "${INT02_TIMELINE_EVERY_MS:-}" ]; then args+=(--timeline-every-ms "$INT02_TIMELINE_EVERY_MS"); fi
  local budget="$BUDGET_DEFAULT"
  if [ "$arm" = "ek-kesinti" ]; then
    args+=(--sync-dir "$work/sync")
    budget="$BUDGET_OUTAGE"
    if [ -n "${INT02_OUTAGE_WAIT_S:-}" ]; then args+=(--outage-wait-s "$INT02_OUTAGE_WAIT_S"); fi
  fi
  # Uzatılmış üst sınırlar verildiyse süre bütçesi de o kadar uzar (örneğin kendi sınırları geçerli kalsın).
  budget=$(( budget + ${INT02_WINDOW_S:-0} + ${INT02_SETTLE_S:-0} + ${INT02_OUTAGE_WAIT_S:-0} ))

  local t_run; t_run="$(date +%s.%N)"
  local t_apply="" t_restore="" exit_code=0
  timeout "$budget" "$example" "${args[@]}" >"$work/run.json" 2>"$work/run.err" &
  local pid=$!
  if [ "$arm" = "ek-kesinti" ]; then
    while [ ! -e "$work/sync/KESINTI_BASLA" ] && kill -0 "$pid" 2>/dev/null; do
      sleep 0.05
    done
    if [ -e "$work/sync/KESINTI_BASLA" ]; then
      # shellcheck disable=SC2086
      tc qdisc change dev lo root netem $OUTAGE_ARGS
      t_apply="$(date +%s.%N)"
      : >"$work/sync/KESINTI_UYGULANDI"
      sleep "$OUTAGE_S"
      # shellcheck disable=SC2086
      tc qdisc change dev lo root netem $NETEM_ARGS seed "$seed"
      t_restore="$(date +%s.%N)"
      : >"$work/sync/KESINTI_BITTI"
    fi
  fi
  wait "$pid" || exit_code=$?
  local t_end; t_end="$(date +%s.%N)"
  tc -s qdisc show dev lo >"$work/qdisc_after.txt"

  INT02_SEED="$seed" INT02_ARM="$arm" INT02_WORK="$work" INT02_OUTFILE="$outfile" \
  INT02_EXIT="$exit_code" INT02_BUDGET="$budget" INT02_T_START="$t_start" INT02_T_RUN="$t_run" \
  INT02_T_APPLY="$t_apply" INT02_T_RESTORE="$t_restore" INT02_T_END="$t_end" \
  INT02_NETEM_ARGS="$NETEM_ARGS" INT02_OUTAGE_ARGS="$OUTAGE_ARGS" INT02_OUTAGE_S="$OUTAGE_S" \
  INT02_PROFILE="$PROFILE_NAME" INT02_PING_COUNT="$PING_COUNT" \
  python3 - <<'PY'
"""Örneğin JSON satırını sürücü ve netem doğrulama kayıtlarıyla birleştirir (ham değerler; hüküm
int02_analyze.py'de)."""
import json
import os
import re

env = os.environ
work = env["INT02_WORK"]


def read(name):
    try:
        with open(os.path.join(work, name), encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return ""


def qdisc(name):
    text = read(name)
    m = re.search(r"Sent (\d+) bytes (\d+) pkt \(dropped (\d+)", text)
    if not m:
        return {"raw": text.strip(), "sent_bytes": None, "sent_pkts": None, "dropped": None}
    return {"raw": text.strip(), "sent_bytes": int(m.group(1)), "sent_pkts": int(m.group(2)),
            "dropped": int(m.group(3))}


def ping():
    text = read("ping.txt")
    out = {"method": "ping", "sent": None, "received": None, "rtt_min_ms": None,
           "rtt_avg_ms": None, "rtt_max_ms": None, "rtt_mdev_ms": None, "raw_tail": text.strip()[-400:]}
    m = re.search(r"(\d+) packets transmitted, (\d+) (?:packets )?received", text)
    if m:
        out["sent"], out["received"] = int(m.group(1)), int(m.group(2))
    m = re.search(r"= ([\d.]+)/([\d.]+)/([\d.]+)/([\d.]+) ms", text)
    if m:
        out["rtt_min_ms"], out["rtt_avg_ms"], out["rtt_max_ms"], out["rtt_mdev_ms"] = (
            float(m.group(i)) for i in range(1, 5))
    return out


def ts(key):
    value = env.get(key, "")
    return float(value) if value else None


before, after = qdisc("qdisc_before.txt"), qdisc("qdisc_after.txt")
run_delta = None
if before["sent_pkts"] is not None and after["sent_pkts"] is not None:
    sent = after["sent_pkts"] - before["sent_pkts"]
    dropped = after["dropped"] - before["dropped"]
    total = sent + dropped
    run_delta = {"sent_pkts": sent, "dropped": dropped,
                 "loss_pct": (100.0 * dropped / total) if total else None}
t_run = ts("INT02_T_RUN")
driver = {
    "profile": env["INT02_PROFILE"],
    "netem_args": env["INT02_NETEM_ARGS"] + " seed " + env["INT02_SEED"],
    "netem_device": "lo",
    "example_exit": int(env["INT02_EXIT"]),
    "budget_s": int(env["INT02_BUDGET"]),
    "timed_out": int(env["INT02_EXIT"]) == 124,
    "t_start_unix": ts("INT02_T_START"),
    "t_run_unix": t_run,
    "t_end_unix": ts("INT02_T_END"),
    "outage": None,
    "stderr_tail": read("run.err").strip()[-2000:],
}
if env["INT02_ARM"] == "ek-kesinti":
    t_apply, t_restore = ts("INT02_T_APPLY"), ts("INT02_T_RESTORE")
    driver["outage"] = {
        "args": env["INT02_OUTAGE_ARGS"], "planned_s": int(env["INT02_OUTAGE_S"]),
        "applied_unix": t_apply, "restored_unix": t_restore,
        "measured_s": (t_restore - t_apply) if (t_apply and t_restore) else None,
    }
netem_check = {"ping": ping(), "qdisc_initial": qdisc("qdisc_initial.txt"),
               "qdisc_before_run": before, "qdisc_after_run": after, "run_delta": run_delta}

line = read("run.json").strip().splitlines()
record = None
if line:
    try:
        record = json.loads(line[-1])
    except json.JSONDecodeError as e:
        record = None
        driver["parse_error"] = str(e)
if not isinstance(record, dict):
    record = {"schema": "int02-node-v3", "arm": env["INT02_ARM"], "seed": int(env["INT02_SEED"]),
              "profile": env["INT02_PROFILE"], "result": "error",
              "error": "sürücü: örnek JSON satırı üretmedi (çıkış %s)" % env["INT02_EXIT"]}
record["driver"] = driver
record["netem_check"] = netem_check
with open(env["INT02_OUTFILE"], "w", encoding="utf-8") as f:
    json.dump(record, f, ensure_ascii=False, sort_keys=False)
    f.write("\n")
PY
  if [ -f "$work/run.err" ]; then cp "$work/run.err" "${outfile%.json}.err"; fi
}

if [ "${1:-}" = "__inner" ]; then
  shift
  inner "$@"
  exit 0
fi

# ── Dış kip ──────────────────────────────────────────────────────────────────────────────────
SEEDS="1 2 3 4 5"
ARMS="gating1 gating2 gating6 negatif"
OUT=""
BIN="$REPO/target/release"
EXAMPLE=""
PROVISION=""
COUNT=""
INTERVAL_MS=""
WINDOW_S=""
OUTAGE_WAIT_S=""
SETTLE_S=""
TIMELINE_EVERY_MS=""
KEEP=0
DRY=0
CHECK=0
while [ $# -gt 0 ]; do
  case "$1" in
    --seeds) SEEDS="${2:?--seeds değer ister}"; shift 2 ;;
    --arms) ARMS="${2:?--arms değer ister}"; shift 2 ;;
    --out) OUT="${2:?--out değer ister}"; shift 2 ;;
    --bin) BIN="${2:?--bin değer ister}"; shift 2 ;;
    --example) EXAMPLE="${2:?--example değer ister}"; shift 2 ;;
    --provision) PROVISION="${2:?--provision değer ister}"; shift 2 ;;
    --count) COUNT="${2:?--count değer ister}"; shift 2 ;;
    --interval-ms) INTERVAL_MS="${2:?--interval-ms değer ister}"; shift 2 ;;
    --window) WINDOW_S="${2:?--window değer ister}"; shift 2 ;;
    --outage-wait-s) OUTAGE_WAIT_S="${2:?--outage-wait-s değer ister}"; shift 2 ;;
    --settle-s) SETTLE_S="${2:?--settle-s değer ister}"; shift 2 ;;
    --timeline-every-ms) TIMELINE_EVERY_MS="${2:?--timeline-every-ms değer ister}"; shift 2 ;;
    --keep-work) KEEP=1; shift ;;
    --dry-run) DRY=1; shift ;;
    --check) CHECK=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "bilinmeyen argüman: $1 (bkz. --help)" ;;
  esac
done
EXAMPLE="${EXAMPLE:-$BIN/examples/int02_node}"
PROVISION="${PROVISION:-$BIN/kama-provision}"
OUT="${OUT:-$REPO/results/int02/$(date +%F)}"

for arm in $ARMS; do
  case "$arm" in
    gating1|gating2|gating6|negatif|ek-dosya-ptt|ek-kesinti) ;;
    *) die "bilinmeyen kol: $arm" ;;
  esac
done
for seed in $SEEDS; do
  case "$seed" in
    ''|*[!0-9]*) die "tohum negatif olmayan tamsayı olmalı: $seed" ;;
  esac
done
for value in "$WINDOW_S" "$OUTAGE_WAIT_S" "$SETTLE_S" "$TIMELINE_EVERY_MS"; do
  case "$value" in
    *[!0-9]*) die "süre seçenekleri negatif olmayan tamsayı (s; --timeline-every-ms için ms) olmalı: $value" ;;
  esac
done

check() {
  local ok=1
  for tool in unshare ip tc ping python3 timeout readlink mktemp; do
    if command -v "$tool" >/dev/null 2>&1; then echo "  ok   komut: $tool"; else echo "  YOK  komut: $tool"; ok=0; fi
  done
  for bin in "$EXAMPLE" "$PROVISION"; do
    if [ -x "$bin" ]; then echo "  ok   ikili: $bin"; else echo "  YOK  ikili: $bin (bkz. --help: derleme)"; ok=0; fi
  done
  if grep -qw '^sch_netem' /proc/modules 2>/dev/null || [ -d /sys/module/sch_netem ]; then
    echo "  ok   çekirdek modülü: sch_netem"
  else
    echo "  YOK  çekirdek modülü: sch_netem (T5: sudo modprobe sch_netem)"; ok=0
  fi
  if [ "$CHECK" = 1 ]; then
    if unshare -rn true 2>/dev/null; then echo "  ok   unprivileged userns + netns (unshare -rn)"; else echo "  YOK  unshare -rn"; ok=0; fi
  fi
  [ "$ok" = 1 ]
}

if [ "$DRY" = 1 ]; then
  echo "KURU KOŞU — hiçbir şey koşulmaz"
  echo "çıktı      : $OUT"
  echo "örnek      : $EXAMPLE"
  echo "provision  : $PROVISION"
  echo "profil     : $PROFILE_NAME = netem $NETEM_ARGS seed <S> (dev lo, bir kez)"
  echo "tohumlar   : $SEEDS"
  echo "kollar     : $ARMS"
  echo "pencere    : ${WINDOW_S:-örnek varsayılanı (120)} s — son gönderimden (ek-kesinti: kesinti bitiminden) sonra tamamlanma üst sınırı, bütün kollar"
  echo "diğer      : outage-wait=${OUTAGE_WAIT_S:-örnek varsayılanı (180)} (yalnız KESINTI_BITTI beklemesi) settle=${SETTLE_S:-örnek varsayılanı (10)}"
  echo "çizelge    : ${TIMELINE_EVERY_MS:-örnek varsayılanı (5000)} ms (0 = kapalı; açıkken m_log da tutulur)"
  for seed in $SEEDS; do
    for arm in $ARMS; do
      echo "  [s$seed $arm] unshare -rn: ip link set lo up; kama-provision init --out <work>/authority" \
           "--devices alice,bob --direct alice=127.0.0.1:$PORT_ALICE,bob=127.0.0.1:$PORT_BOB;" \
           "tc qdisc add dev lo root netem $NETEM_ARGS seed $seed; ping -c $PING_COUNT;" \
           "int02_node --arm $arm --seed $seed …;" \
           "$( [ "$arm" = ek-kesinti ] && echo "KESINTI_BASLA → tc qdisc change … $OUTAGE_ARGS ($OUTAGE_S s) → geri;" )" \
           "→ $OUT/seed-$seed-$arm.json"
    done
  done
  echo "önkoşullar (statik):"
  check || echo "  (eksikler koşudan önce giderilmeli)"
  exit 0
fi

if [ "$CHECK" = 1 ]; then
  echo "önkoşullar:"
  if check; then echo "TAMAM"; exit 0; else echo "EKSİK"; exit 1; fi
fi

check >/dev/null || { CHECK=1; echo "önkoşullar eksik:"; check || true; exit 1; }
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
echo "INT-02: çıktı $OUT · tohumlar [$SEEDS] · kollar [$ARMS] · profil $PROFILE_NAME"
for seed in $SEEDS; do
  for arm in $ARMS; do
    outfile="$OUT/seed-$seed-$arm.json"
    work="$(mktemp -d "${TMPDIR:-/tmp}/int02-s$seed-$arm-XXXXXX")"
    echo "→ s$seed $arm (çalışma: $work)"
    rc=0
    INT02_EXAMPLE="$EXAMPLE" INT02_PROVISION="$PROVISION" INT02_COUNT="$COUNT" \
    INT02_INTERVAL_MS="$INTERVAL_MS" INT02_WINDOW_S="$WINDOW_S" \
    INT02_OUTAGE_WAIT_S="$OUTAGE_WAIT_S" INT02_SETTLE_S="$SETTLE_S" \
    INT02_TIMELINE_EVERY_MS="$TIMELINE_EVERY_MS" \
      unshare -rn bash "$SELF" __inner "$seed" "$arm" "$work" "$outfile" || rc=$?
    if [ "$rc" != 0 ]; then
      echo "  ! iç kip çıkışı $rc (kayıt: $outfile varsa ona bakın; çalışma dizini korunuyor: $work)"
      continue
    fi
    if [ "$KEEP" = 0 ]; then rm -rf "$work"; fi
    python3 - "$outfile" <<'PY' || true
import json
import sys

r = json.load(open(sys.argv[1], encoding="utf-8"))
line = ["  sonuç:", str(r.get("result")), "| pencere:", str((r.get("window") or {}).get("ended_by"))]
if r.get("arm") == "ek-kesinti":
    o = r.get("outage") or {}
    line += ["| metin tam:", str(o.get("texts_complete")), "| same_session:", str(o.get("same_session"))]
if r.get("arm") == "ek-dosya-ptt":
    f = ((r.get("file") or {}).get("outcome") or {})
    line += ["| dosya:", str(f.get("status")), str(f.get("reason") or "")]
line += ["| hata:", str(r.get("error"))]
print(" ".join(line))
PY
  done
done
echo "BİTTİ → $OUT"
echo "analiz: python3 $REPO/scripts/netem/int02_analyze.py $OUT"
