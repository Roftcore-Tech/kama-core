#!/usr/bin/env bash
# KM-SEAM-AT-001 App↔Transport seam conformance runner — hedef secicili.
#
#   scripts/ci/seam-conformance.sh --target mock|real|both
#
# AMAC: ayni sozlesme suiti hem sahte hem gercek module karsi kosabilsin; hangisinin kostugu
# CIKTIDAN okunsun; kosmayan hedef YESIL SAYILMASIN.
#
# UC KATEGORI (dizinler: crates/kama-integration/tests/):
#   seam_conformance_core  — sozlesmenin gozlenebilir davranisi; hedef-agnostik, her hedefte kosar
#   seam_mock_fixture      — sahte modul consumer'i yaniltmiyor mu; YALNIZ mock
#   seam_real_adapter      — implementasyona ozgu (socket reset vb.); YALNIZ real
#
# CIKIS KODLARI
#   0  yesil
#   1  kirmizi (test kirmizi · vakum · #[ignore]'lu test · beyan ↔ gerceklik ayrismasi · drift ·
#      iddia edilen satirin testi listede yok · statik satirin kaniti (layer_check.py) kirmizi)
#   2  ortam/kullanim hatasi (eksik/gecersiz --target, python3 yok, dosya yok, layer_check.py'nin
#      kendi ortam hatasi). Ortam hatasi, o ana kadar birikmis bir kirmiziyi ORTER: kosum yarida
#      kesilir ve 2 doner (cikti birikmis kirmiziyi ayrica soyler).
#   3  BEYAN EDILMIS BOSLUK — istenen hedef bu derlemede yok ve yoklugu kimlikli olarak beyanli.
#      Yesil DEGILDIR; "unuttuk" ile "henuz yok"u ayirir. ~~(bugun: --target real → GAP-SEAM-REAL-1)~~
#      [2026-10-08 L-T1: gercek hedef kayitli; kayitli hedefin ABSENT beyani KIRMIZIDIR (1)]
#
# TASARIM KARARI — "atlandi" diye bir sonuc YOK. Rust'ta alisilmis kalip, ortam degiskeni yoksa
# testi `#[ignore]` etmektir; sonuc yesil kosuda gorunmez bir bosluktur. Bu repoda bu hata sinifi
# olculdu (BUL-02/BUL-03 "0 test kosan kapi", ci-verdict'te `skipped` yesil sayiliyordu). Burada
# istenen hedef yoksa kapi AYRI BIR KOD dondurur ve CI o kodu ACIKCA bekler; boylece "hedef
# parametresi calisiyor" iddiasi her kosuda OLCULUR.
set -uo pipefail

ROOT="${SEAM_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT" || exit 2

PKG="kama-integration"
COVERAGE="scripts/ci/seam-coverage.json"
DRIFT="scripts/ci/seam-api-drift.py"
LAYER="scripts/ci/layer_check.py"
TARGETS_RS="crates/kama-integration/tests/common/seam/target.rs"

usage() {
  sed -n '2,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

fail=0
gap=""
step() { printf '\n── %s ──\n' "$1"; }
bad() { printf '::error title=SEAM-CONFORMANCE::%s\n' "$1"; printf 'KIRMIZI: %s\n' "$1"; fail=1; }

# ── argumanlar ───────────────────────────────────────────────────────────────────────────────
TARGET=""
while (($#)); do
  case "$1" in
    --target) TARGET="${2:-}"; shift 2 || true ;;
    --target=*) TARGET="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'kullanim hatasi: bilinmeyen arguman %s\n\n' "$1"; usage; exit 2 ;;
  esac
done
case "$TARGET" in
  mock|real|both) ;;
  "") printf 'kullanim hatasi: --target zorunlu\n\n'; usage; exit 2 ;;
  *) printf 'kullanim hatasi: gecersiz hedef %s (mock|real|both)\n\n' "$TARGET"; usage; exit 2 ;;
esac
command -v python3 >/dev/null || { echo "ORTAM HATASI: python3 yok"; exit 2; }
[[ -f "$COVERAGE" ]] || { echo "ORTAM HATASI: $COVERAGE yok"; exit 2; }

printf 'SEAM-CONFORMANCE · hedef=%s · kok=%s\n' "$TARGET" "$ROOT"

# run_suite <test-hedefi> <log-degiskeni-dosyasi> ; 0 = yesil ve EN AZ BIR test kostu
run_suite() {
  local t="$1" log="$2"
  # Varsayilan feature kumesi bilerek: bu uc hedef feature-agnostiktir, --all-features yalnizca
  # ilgisiz agir panelleri derler. (main.yml zaten tam workspace'i --all-features kosar.)
  if ! cargo test -p "$PKG" --test "$t" -- --nocapture >"$log" 2>&1; then
    bad "$PKG::$t kirmizi"
    tail -40 "$log"
    return 1
  fi
  if ! grep -Eq "test result: ok\. [1-9][0-9]* passed" "$log"; then
    bad "$PKG::$t 0 test kostu — vakum (dosya var, test yok)"
    tail -20 "$log"
    return 1
  fi
  # #[ignore]'lu test LISTELENIR ama KOSMAZ; "listede var" kaniti onu yesil saydirmasin.
  if ! grep -Eq "test result: ok\. [0-9]+ passed; 0 failed; 0 ignored" "$log"; then
    bad "$PKG::$t icinde #[ignore]'lu (kosmayan) test var — susturulmus test yesil sayilmaz"
    grep -E "test result:| \.\.\. ignored" "$log" | head -10
    return 1
  fi
  grep -E "test result: ok" "$log" | head -1 | sed 's/^/  /'
  return 0
}

# ── 0. API drift kapisi (spec ↔ sozlesme kaynagi ↔ beyan) ────────────────────────────────────
# [B1 2026-10-05] Sozlesmenin TEK derlenen hali crates/kama-contracts'tadir; test-yerel ayna ve
# kama-app'teki kopya kalkti. Kapi iki seam'i de o kaynaga karsi denetler.
step "API drift (KM-SEAM-AT-001 §1/§3/§6 ↔ kama-contracts::s1 ↔ beyan)"
if [[ -f "$DRIFT" ]]; then
  python3 "$DRIFT" --seam s1 || bad "S1: spec ↔ sozlesme kaynagi ↔ beyan ayrismasi (bkz. yukaridaki bulgular)"
  step "API drift (KM-SEAM-TC-001 §1.1/§1.2/§6 ↔ kama-contracts::s3 ↔ beyan)"
  python3 "$DRIFT" --seam s3 || bad "S3: spec ↔ sozlesme kaynagi ↔ beyan ayrismasi (bkz. yukaridaki bulgular)"
else
  bad "$DRIFT yok — sozlesme kaynagi denetimsiz kaldi"
fi

# ── mock hedefi ──────────────────────────────────────────────────────────────────────────────
run_mock() {
  local core_log fixture_log rows
  core_log="$(mktemp -t seam-core-XXXXXX.log)"
  fixture_log="$(mktemp -t seam-fixture-XXXXXX.log)"

  step "conformance-core @ mock"
  run_suite seam_conformance_core "$core_log"

  step "mock-fixture @ mock"
  run_suite seam_mock_fixture "$fixture_log"

  # Sozlesme satiri beyani ↔ gerceklik (ADR-03 doktrini: birincil + capraz merci)
  step "§6 kapsam: kod ↔ beyan ↔ listelenen testler capraz kontrolu"
  rows="$(grep -o 'SEAM-CONTRACT-ROWS: target=[^ ]* rows=.*' "$core_log" | head -1)"
  # "Iddia edilen satir gercekten bir test mi" sorusu kosum logundan DEGIL --list'ten okunur:
  # --nocapture ciktisi satir ortasinda ic ice gecebilir (olculdu), --list gecmez. Listede olmak
  # kosmus olmak DEGILDIR; kosumun tamaminin gectigi ve hicbir testin #[ignore] ile susturulmadigi
  # yukarida run_suite'te olculdu (0 failed; 0 ignored).
  listed="$(cargo test -p "$PKG" --test seam_conformance_core -- --list 2>/dev/null | grep ': test$' || true)"

  # Statik satirlar (kind=static) kosum degil kapi-seviyesi denetimdir; kanitlari burada KOSTURULUR.
  static_proof="none"
  if grep -q '"proof": "layer_check"' "$COVERAGE"; then
    if [[ ! -f "$LAYER" ]]; then
      bad "$LAYER yok — statik satirlarin kaniti kosturulamadi"
      static_proof="red"
    elif layer_out="$(python3 "$LAYER" --root "$ROOT" 2>&1)"; then
      static_proof="ok"
      printf '  statik kanit: %s\n' "$(tail -1 <<<"$layer_out")"
    else
      layer_rc=$?
      printf '%s\n' "$layer_out" | tail -12
      if [[ "$layer_rc" -eq 2 ]]; then
        # layer_check'in KENDI ortam hatasi (cargo metadata calismadi vb.) bir katman ihlali degildir.
        echo "ORTAM HATASI: statik satirlarin kaniti kosturulamadi (layer_check.py cikis 2)"
        ((fail)) && echo "(ayrica: bu ana kadar en az bir KIRMIZI kaydedilmisti — yukariya bakin)"
        rm -f "$core_log" "$fixture_log"
        exit 2
      fi
      bad "statik satirlarin kaniti (layer_check.py) kirmizi"
      static_proof="red"
    fi
  fi

  if [[ -z "$rows" ]]; then
    bad "conformance-core kapsam sentinel'ini basmadi (SEAM-CONTRACT-ROWS) — kod tarafi sessiz"
  else
    printf '  %s\n' "$rows"
    ROWS_LINE="$rows" LISTED="$listed" STATIC_PROOF="$static_proof" python3 - "$COVERAGE" <<'PY' || fail=1
import json, os, sys
line = os.environ["ROWS_LINE"]
code_rows = [r for r in line.split("rows=", 1)[1].strip().split(",") if r]
listed = {l.rsplit(": test", 1)[0].strip() for l in os.environ["LISTED"].splitlines() if l.strip()}
static_proof = os.environ["STATIC_PROOF"]
cov = json.load(open(sys.argv[1], encoding="utf-8"))
# Kod tarafi "hangi govdeler kosuyor" der; beyan "o govdenin kanit degeri ne" der.
# Kosan bir govde ya implemented (iki hedefte) ya mock_only (yalniz sahtede) olabilir.
declared = [r["id"] for r in cov["rows"] if r.get("status") in ("implemented", "mock_only")]
proven = [r["id"] for r in cov["rows"] if r.get("status") == "implemented"]
static = [r["id"] for r in cov["rows"] if r.get("status") == "static"]
absent = [r["id"] for r in cov["rows"] if r.get("status") == "not_implemented"]
floor = cov.get("implemented_baseline", 0) + cov.get("mock_only_baseline", 0)
ok = True
def red(title, msg):
    global ok
    print(f"::error title=SEAM-CONFORMANCE::{msg}")
    print(f"KIRMIZI: {title}")
    ok = False
if sorted(code_rows) != sorted(declared):
    red("§6 kapsam beyani gerceklikle ayrisiyor",
        f"kapsam ayrismasi — kodda {sorted(code_rows)}, beyanda {sorted(declared)}. "
        f"Ikisi AYNI commit'te guncellenir.")
if len(code_rows) < floor:
    red("kapsam ratchet'i dustu",
        f"ratchet dustu — kodda {len(code_rows)} satir, taban {floor}. Kapsam kaybi bilincli mi?")
# Iddia edilen her satirin MOCK hedefinde listelenen bir testi olmali (ad = kimligin kucuk harfli,
# '-' yerine '_' konmus hali). CONTRACT_ROWS elle tutulan bir listedir; sarmalayici silinirse
# liste yalan soyler ve bunu baska hicbir sey yakalamaz.
ghost = [r for r in code_rows if f"mock_contract::{r.lower().replace('-', '_')}" not in listed]
if ghost:
    red("iddia edilen §6 satirinin testi yok",
        f"CONTRACT_ROWS'ta olup mock hedefinde listelenen testi OLMAYAN satir(lar): {ghost} "
        f"(beklenen ad: mock_contract::<kimlik kucuk harf, - yerine _>)")
if static and static_proof != "ok":
    red("statik satirlar kanitsiz",
        f"statik beyanli satir(lar) {static} icin kanit kapisi yesil degil (durum: {static_proof})")
if ok:
    total = len(cov["rows"])
    running = len(code_rows) + len(static)
    print(f"  §6 kapsami: {len(proven)}/{total} KANITLANMIS (iki hedefte) · "
          f"{len(code_rows) - len(proven)}/{total} yalniz sahtede · "
          f"{len(static)}/{total} statik (layer_check) · taban {floor}")
    print(f"  §6: {running}/{total} satir kosuyor ({len(code_rows)} davranis @ mock + {len(static)} statik kapi)"
          + (f" · uygulanmamis: {absent}" if absent else ""))
sys.exit(0 if ok else 1)
PY
  fi
  rm -f "$core_log" "$fixture_log"
}

# ── real hedefi ──────────────────────────────────────────────────────────────────────────────
run_real() {
  local log real_core_log
  log="$(mktemp -t seam-real-XXXXXX.log)"

  step "real-adapter @ real"
  if ! run_suite seam_real_adapter "$log"; then
    rm -f "$log"
    return
  fi

  local sentinel
  sentinel="$(grep -o 'SEAM-TARGET-REAL: [A-Z]* gap=[A-Za-z0-9-]*' "$log" | head -1)"
  if [[ "$sentinel" == *"ABSENT"* ]]; then
    # [L-T1] Kayitli bir gercek hedef "yok" beyan edilemez: bosluk beyani kaydin GERISINDE kalmis.
    if python3 - "$TARGETS_RS" <<'PY'
import re, sys
s = open(sys.argv[1], encoding="utf-8").read()
m = re.search(r"REGISTERED_TARGETS[^;]*;", s, re.S)
sys.exit(0 if (m and "TargetKind::Real" in m.group(0)) else 1)
PY
    then
      bad "gercek hedef REGISTERED_TARGETS'ta kayitli ama real-adapter 'ABSENT' diyor — bosluk beyani kayitla celisiyor"
      rm -f "$log"
      return
    fi
    gap="${sentinel##*gap=}"
    printf '  %s\n' "$sentinel"
    rm -f "$log"
    return
  fi

  # Gercek hedef kayitli: conformance-core'un real kolu KOSMAK ZORUNDA.
  step "conformance-core @ real"
  real_core_log="$(mktemp -t seam-real-core-XXXXXX.log)"
  if ! cargo test -p "$PKG" --test seam_conformance_core -- --nocapture real >"$real_core_log" 2>&1; then
    bad "conformance-core real kolunda kirmizi"
    tail -40 "$real_core_log"
  elif ! grep -Eq "test result: ok\. [1-9][0-9]* passed" "$real_core_log"; then
    bad "gercek hedef kayitli ama conformance-core'da 'real' adinda 0 test kostu — \
suit gercek hedefte ornekleendi mi? (seam_mechanism_suite!/seam_contract_suite! real satirlari)"
  elif ! grep -Eq "test result: ok\. [0-9]+ passed; 0 failed; 0 ignored" "$real_core_log"; then
    bad "conformance-core real kolunda #[ignore]'lu (kosmayan) test var — susturulmus test yesil sayilmaz"
  else
    grep -E "test result: ok" "$real_core_log" | head -1 | sed 's/^/  /'
    real_rows_cross_check "$real_core_log"
  fi
  rm -f "$log" "$real_core_log"
}

# [L-T1 2026-10-08] §6 kapsam @ real: kosan satirlar ↔ beyan ↔ beyanli bosluklar.
# Bir satir real hedefte YALNIZ kimlikli beyanla (SeamTarget::ROW_GAPS → SEAM-ROW-GAP sentinel'i)
# atlanabilir ve beyanda `real_gap` ile AYNI kimligi tasimalidir; boyle bir satir 'implemented'
# sayilamaz. Bosluksuz kosan her satir 'implemented' beyan edilmeli ve real_contract:: altinda
# listelenmeli. Ayrisma kirmizidir.
real_rows_cross_check() {
  local core_log="$1" listed
  step "§6 kapsam @ real: beyan ↔ kosum ↔ beyanli bosluk capraz kontrolu"
  listed="$(cargo test -p "$PKG" --test seam_conformance_core -- --list 2>/dev/null | grep ': test$' || true)"
  REAL_LOG="$core_log" LISTED="$listed" python3 - "$COVERAGE" <<'PY' || fail=1
import json, os, re, sys
log = open(os.environ["REAL_LOG"], encoding="utf-8", errors="replace").read()
listed = {l.rsplit(": test", 1)[0].strip() for l in os.environ["LISTED"].splitlines() if l.strip()}
cov = json.load(open(sys.argv[1], encoding="utf-8"))
rows = {r["id"]: r for r in cov["rows"]}
ok = True
def red(title, msg):
    global ok
    print(f"::error title=SEAM-CONFORMANCE::{msg}")
    print(f"KIRMIZI: {title}")
    ok = False
m = re.search(r"SEAM-CONTRACT-ROWS: target=real rows=([A-Z0-9,-]*)", log)
gaps = dict(re.findall(r"SEAM-ROW-GAP: target=real row=([A-Z0-9-]+) gap=([A-Za-z0-9-]+)", log))
if not m:
    red("real kapsam sentinel'i yok", "conformance-core @ real SEAM-CONTRACT-ROWS basmadi — kod tarafi sessiz")
code_rows = [r for r in (m.group(1).split(",") if m else []) if r]
for rid in code_rows:
    r = rows.get(rid, {})
    declared, seen = r.get("real_gap"), gaps.get(rid)
    if declared or seen:
        if declared != seen:
            red("beyanli bosluk ↔ sentinel ayrisiyor",
                f"{rid}: beyanda real_gap={declared!r}, kosumda bosluk={seen!r}")
        if r.get("status") != "mock_only":
            red("bosluk beyanli satir kanitlanmis sayiliyor",
                f"{rid}: real hedefte KOSMADI (bosluk {seen or declared}) ama durum {r.get('status')!r}")
    else:
        if r.get("status") != "implemented":
            red("beyan gercekligin gerisinde",
                f"{rid}: iki hedefte de kostu ama durum {r.get('status')!r} — 'implemented' olmali")
        name = f"real_contract::{rid.lower().replace('-', '_')}"
        if name not in listed:
            red("real hedefte satirin testi yok", f"{rid}: {name} listelenmiyor")
unknown = sorted(set(gaps) - set(code_rows))
if unknown:
    red("bilinmeyen satir icin bosluk", f"CONTRACT_ROWS'ta olmayan satir(lar) icin bosluk beyani: {unknown}")
if ok:
    ran = len(code_rows) - len(gaps)
    print(f"  §6 @ real: {ran}/{len(code_rows)} satir kostu · beyanli bosluk: "
          + (", ".join(f"{k}={v}" for k, v in sorted(gaps.items())) or "yok"))
sys.exit(0 if ok else 1)
PY
}

case "$TARGET" in
  mock) run_mock ;;
  real) run_real ;;
  both) run_mock; run_real ;;
esac

printf '\n'
if ((fail)); then
  printf 'SEAM-CONFORMANCE: KIRMIZI\n'
  exit 1
fi
if [[ -n "$gap" ]]; then
  printf 'SEAM-CONFORMANCE: BEYAN EDILMIS BOSLUK (%s) — hedef bu derlemede yok, yesil DEGIL\n' "$gap"
  exit 3
fi
printf 'SEAM-CONFORMANCE: YESIL (hedef=%s)\n' "$TARGET"
exit 0
