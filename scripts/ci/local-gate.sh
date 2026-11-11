#!/usr/bin/env bash
# Kama yerel kapı (EK-1 D-17; depoda docs/decisions/KARAR-KAYDI-2026-11.md §2). Hosted CI fatura ya da
# altyapı yüzünden başlamazsa merge'e yeten kapı budur (CLAUDE.md §7). `.github/workflows/ci.yml` ve
# `spec-word-coverage.yml`'deki PR adımlarını AYNI komutlarla (ci.yml'deki ::error ileti sarmalayıcıları
# hariç), ci.yml'deki sırayla, tek cargo süreciyle koşar. İlk adım kapının kendi öz-testidir.
#
# Kullanım:
#   scripts/ci/local-gate.sh --ref <REF> [--out DİZİN] [--base REF]     (ÖNERİLEN)
#       adayı ayrı, sabit bir worktree'de sınar (git worktree add --detach). Başka ajanların aynı ağaca
#       yazması koşuyu etkileyemez. Derleme önbelleği paylaşılır: CARGO_TARGET_DIR =
#       KAMA_GATE_TARGET (varsayılan ~/.cache/kama-local-gate/target). Worktree koşu sonunda silinir.
#   scripts/ci/local-gate.sh [--out DİZİN] [--base REF]
#       içinde bulunulan ağacı sınar; ağaç TEMİZ olmalıdır (izlenmeyen dosya dahil), yoksa reddeder.
#   scripts/ci/local-gate.sh --list
#       adımları yazdırır, koşmaz.
#     --out   çıktı dizini (varsayılan: ~/.cache/kama-local-gate/<kısa-sha>-<zaman>)
#     --base  karşılaştırma tabanı (varsayılan: git merge-base HEAD origin/main); conformance teşhisi
#             (CONF_DIFF_BASE) ve silme kapısı (--base) bunu kullanır
#
# Sonuç ve çıkış kodu (özetin son satırı "SONUÇ: …"):
#   0 YEŞİL       bütün adımlar geçti, CI tanımı kaymamış, ağaç koşu boyunca değişmedi
#   1 KIRMIZI     en az bir adım kırmızı ya da CI tanımı bu betiğin yansıttığı sürümden kaymış (CI-DRIFT)
#   2 REDDEDİLDİ  koşu başlamadı: kirli ağaç, kilit açılamadı/alınamadı, geçersiz argüman ya da worktree hatası
#   3 GEÇERSİZ    koşu sırasında HEAD ya da çalışma ağacı değişti; sonuç hiçbir commit'i temsil etmez
#
# Kurallar:
#   * Toolchain rust-toolchain.toml'dan gelir (1.96.0). CARGO_BUILD_JOBS=2. PYTHONDONTWRITEBYTECODE=1
#     (yalnız python önbellek dosyalarının ağaçta iz bırakmaması için; CI'da da etkisizdir).
#   * Tek cargo süreci: betik D-34 kilidini (scripts/ci/cargo-lock.sh ile aynı dosya) alır ve sonuna dek
#     tutar. Kilit dosyası açılamaz ya da flock başarısız olursa koşu başlamaz (çıkış 2).
#   * Kırmızıda DURMAZ; bütün adımları koşar.
#   * Hosted CI'a kalan adımlar (D-17, adıyla) burada KOŞMAZ ve özette öyle yazılır:
#     miri (ci.yml miri), ASan (asan), MSan (msan), cargo audit (security), aead-differential.
#   * Bu betik ci.yml'in belirli bir sürümünü yansıtır (CI_SHA256, SWC_SHA256). Sürüm kaymışsa sonuç
#     KIRMIZI'dır; bu dosya ci.yml'i değiştiren PR'da güncellenir.
#   * KAMA_GATE_SELFTEST=1 ve KAMA_GATE_STEPS yalnız öz-test içindir (scripts/ci/local-gate-selftest.sh);
#     o kipte özet "ÖZ-TEST ADIM SETİ" satırını taşır ve gerçek kapı sonucu sayılmaz.
#
# PR'a yazılacak satır (D-17): "CI-KİLİT İSTİSNASI ile merge: yerel kapı <SONUÇ> @<sha>,
# özet: <out>/summary.txt" + summary.txt içeriği.
set -u

CI_SHA256=6565f5a53a74e5638d2c58b9486ceae457241a8369ee7200d04f9d3252628e0f      # .github/workflows/ci.yml
SWC_SHA256=2907c16c67373aa7e95ecb0e072b9b746bc346b1ba462d56aa63a1c35664ab5d     # .github/workflows/spec-word-coverage.yml
HOSTED_ONLY="miri · asan · msan · cargo audit · aead-differential"

OUT=""; BASE=""; LIST=0; REF=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out)  [ $# -ge 2 ] || { echo "--out değer ister" >&2; exit 2; }; OUT=$2; shift 2 ;;
    --base) [ $# -ge 2 ] || { echo "--base değer ister" >&2; exit 2; }; BASE=$2; shift 2 ;;
    --ref)  [ $# -ge 2 ] || { echo "--ref değer ister" >&2; exit 2; }; REF=$2; shift 2 ;;
    --list) LIST=1; shift ;;
    *) echo "bilinmeyen argüman: $1" >&2; exit 2 ;;
  esac
done

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "git ağacı değil" >&2; exit 2; }
cd "$ROOT" || exit 2

# ── Adımlar: ad | komut. Komut metinleri ci.yml'deki `run:` satırlarıyla aynıdır. ──────────────────
# ci.yml'de `/tmp/pp.txt` gibi sabit yollar $OUT altına alınmıştır; başka fark yoktur.
STEPS=()
add() { STEPS+=("$1|$2"); }
# kapının kendi kapısı (ci.yml'de yok; hata yollarının gerçekten kırmızı/ret/geçersiz verdiğini sınar)
add gate_self        'scripts/ci/local-gate-selftest.sh'
# static (ci.yml: static)
add layer_check      'python3 scripts/ci/layer_check.py --selftest && python3 scripts/ci/layer_check.py'
add systemd_unit     'systemd-analyze verify deploy/kama-hub.service'
add no_inline        'python3 scripts/ci/no_inline_expected.py --selftest && python3 scripts/ci/no_inline_expected.py --min-files 1'
add fmt              'cargo fmt --all -- --check'
add clippy_all       'cargo clippy --all-targets --all-features -- -D warnings'
add clippy_nodef     'cargo clippy --all-targets --no-default-features -- -D warnings'
# test-fast (ci.yml: test-fast)
add sealed_type      'set -o pipefail; out=$(cargo test -p kama-noise-wire --locked --test trybuild -- --list) && echo "$out" && echo "$out" | grep -q "sealed_type_cannot_be_forged" && echo "$out" | grep -q "hub_handle_cannot_stand_in_for_session_handle"'
add test_fast        'cargo test --workspace --all-features --exclude kama-integration'
add product_profile  'set -o pipefail; cargo test -p client-core --all-features --test product_profile_v1 2>&1 | tee "$OUT/pp.txt" && grep -Eq "test result: ok\. [1-9][0-9]* passed" "$OUT/pp.txt" && cargo test -p adaptive-transport --all-features --test fec_loss_risk'
# test-runtime (ci.yml: test-runtime) — adım env'i (K4G05B_RUNTIME_TESTS) komutun başında
add test_runtime     'K4G05B_RUNTIME_TESTS=11; set -o pipefail; cargo test --workspace --all-features --exclude kama-integration --test k4g05b_runtime_ownership -- --ignored 2>&1 | tee "$OUT/rt.txt" && grep -Eq "test result: ok\. ${K4G05B_RUNTIME_TESTS} passed; 0 failed" "$OUT/rt.txt"'
# conformance (ci.yml: conformance)
add conformance_self 'scripts/ci/conformance-selftest.sh'
add conformance      'CONF_DIFF_BASE="$BASE" scripts/ci/conformance.sh'
# security (ci.yml: security) — cargo audit hosted'a kalır (D-17)
add deny             'cargo deny check'
# vectors (ci.yml: vectors) — jsonschema kurulu olmalı (ci.yml: pip install jsonschema); yoksa adım kırmızı
add vectors_self     'scripts/vectors/selftest.sh'
add vectors_valid    'python3 scripts/vectors/validate.py vectors/ --canonical --min-files 2 --require-cross-check'
add spec_coverage    'python3 scripts/vectors/spec_coverage.py'
add ref_crypto       'python3 ref/crypto/selftest.py'
add ref_vectors      'python3 ref/crypto/gen_vectors.py --check'
# seam-conformance (ci.yml: seam-conformance)
add seam_self        'scripts/ci/seam-conformance-selftest.sh'
add seam_mock        'scripts/ci/seam-conformance.sh --target mock'
add seam_real        'scripts/ci/seam-conformance.sh --target real'
add noise_mock       'scripts/ci/noise-conformance.sh --target mock'
add noise_identity   'scripts/ci/noise-conformance.sh --target identity'
add noise_vectors    'scripts/ci/noise-conformance.sh --target vectors'
add noise_real       'scripts/ci/noise-conformance.sh --target real'
# spec-word-coverage.yml (silme kapısı; CI'da yalnız kapsamdaki belge değişince koşar, burada her zaman)
add word_cov_self    'python3 scripts/ci/word_coverage.py --selftest'
add word_cov         'python3 scripts/ci/word_coverage.py --base "$BASE"'

SELFTEST_MODE=0
if [ "${KAMA_GATE_SELFTEST:-0}" = 1 ] && [ -n "${KAMA_GATE_STEPS:-}" ]; then
  SELFTEST_MODE=1; STEPS=()
  while IFS= read -r line; do [ -n "$line" ] && STEPS+=("$line"); done <<< "$KAMA_GATE_STEPS"
fi

if [ "$LIST" = 1 ]; then
  i=0; for s in "${STEPS[@]}"; do i=$((i+1)); printf '%02d %-17s %s\n' "$i" "${s%%|*}" "${s#*|}"; done
  echo "hosted'a kalan (koşmaz): $HOSTED_ONLY"
  exit 0
fi

# ── Aday ağacı: --ref ile ayrı worktree; yoksa içinde bulunulan ağaç ────────────────────────────────
WT=""
cleanup() { if [ -n "$WT" ]; then git -C "$ROOT" worktree remove --force "$WT" >/dev/null 2>&1 || true; fi; }
trap cleanup EXIT
if [ -n "$REF" ]; then
  SHA=$(git rev-parse --verify -q "${REF}^{commit}") || { echo "REDDEDİLDİ: --ref çözülemedi: $REF" >&2; exit 2; }
  WT_ROOT=${KAMA_GATE_WT_ROOT:-$HOME/.cache/kama-local-gate/wt}
  mkdir -p "$WT_ROOT" || exit 2
  WT="$WT_ROOT/${SHA:0:12}-$$"
  git worktree add -q --detach "$WT" "$SHA" >/dev/null 2>&1 || { WT=""; echo "REDDEDİLDİ: worktree kurulamadı" >&2; exit 2; }
  cd "$WT" || exit 2
  export CARGO_TARGET_DIR=${KAMA_GATE_TARGET:-$HOME/.cache/kama-local-gate/target}
fi

[ -n "$BASE" ] || BASE=$(git merge-base HEAD origin/main 2>/dev/null) || { echo "REDDEDİLDİ: taban bulunamadı (origin/main?); --base verin" >&2; exit 2; }
HEAD_SHA=$(git rev-parse HEAD) || exit 2
[ -n "$OUT" ] || OUT="$HOME/.cache/kama-local-gate/${HEAD_SHA:0:8}-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUT" || exit 2
export OUT BASE
export CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never CARGO_INCREMENTAL=0 RUST_BACKTRACE=1 PYTHONDONTWRITEBYTECODE=1
SUM="$OUT/summary.txt"; : > "$SUM"

reject() { echo "SONUÇ: REDDEDİLDİ — $1" | tee -a "$SUM" >&2; exit 2; }

# ── D-34 kilidi: açılamaz ya da alınamazsa koşu yok ───────────────────────────────────────────────────
LOCK=${KAMA_CARGO_LOCK:-$HOME/.cache/kama-cargo.lock}
mkdir -p "$(dirname "$LOCK")" 2>/dev/null
exec 9>"$LOCK" || reject "kilit dosyası açılamadı: $LOCK"
echo "D-34 kilidi bekleniyor: $LOCK" >&2
flock 9 || reject "kilit alınamadı (flock): $LOCK"

# ── Başlangıç durumu: ağaç temiz olmalı (izlenmeyen dosya dahil; yoksayılanlar hariç) ───────────────
START_STATUS=$(git status --porcelain --untracked-files=normal) || reject "git status okunamadı"
[ -z "$START_STATUS" ] || reject "çalışma ağacı kirli (sonuç HEAD'i temsil etmezdi; --ref kullanın): $(printf '%s' "$START_STATUS" | head -3 | tr '\n' ' ')"

{
  [ "$SELFTEST_MODE" = 1 ] && echo "ÖZ-TEST ADIM SETİ (KAMA_GATE_STEPS) — gerçek kapı sonucu değildir"
  echo "HEAD $HEAD_SHA $(git log -1 --format=%s)"
  echo "BASE $BASE"
  if [ -n "$WT" ]; then echo "ağaç: ayrı worktree ($WT) · CARGO_TARGET_DIR=$CARGO_TARGET_DIR"; else echo "ağaç: $(pwd) (temiz)"; fi
  echo "toolchain $(rustc --version 2>/dev/null || echo yok) · CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS"
} >> "$SUM"

red=0
drift=""
[ "$(sha256sum .github/workflows/ci.yml 2>/dev/null | cut -d' ' -f1)" = "$CI_SHA256" ] || drift="ci.yml"
[ "$(sha256sum .github/workflows/spec-word-coverage.yml 2>/dev/null | cut -d' ' -f1)" = "$SWC_SHA256" ] || drift="${drift:+$drift, }spec-word-coverage.yml"
if [ -n "$drift" ]; then
  red=$((red+1))
  echo "CI-DRIFT (KIRMIZI): $drift bu betiğin yansıttığı sürümden farklı — local-gate.sh güncellenmeden kapı geçemez" >> "$SUM"
fi

n=0
for s in "${STEPS[@]}"; do
  n=$((n+1)); name=${s%%|*}; cmd=${s#*|}
  log=$(printf '%s/%02d-%s.log' "$OUT" "$n" "$name")
  t0=$SECONDS
  bash -c "$cmd" > "$log" 2>&1; rc=$?
  [ "$rc" = 0 ] || red=$((red+1))
  printf '%02d %-17s exit=%-3s %5ss\n' "$n" "$name" "$rc" "$((SECONDS-t0))" >> "$SUM"
done
echo "hosted'a kalan (koşmadı): $HOSTED_ONLY" >> "$SUM"

# ── Bitiş doğrulaması: HEAD ve çalışma ağacı başlangıçtakiyle aynı mı ───────────────────────────────
END_HEAD=$(git rev-parse HEAD 2>/dev/null || echo "?")
END_STATUS=$(git status --porcelain --untracked-files=normal 2>/dev/null || echo "?")
if [ "$END_HEAD" != "$HEAD_SHA" ] || [ -n "$END_STATUS" ]; then
  {
    echo "SONUÇ: GEÇERSİZ — koşu sırasında ağaç değişti; sonuç hiçbir commit'i temsil etmez"
    [ "$END_HEAD" = "$HEAD_SHA" ] || echo "  HEAD: $HEAD_SHA → $END_HEAD"
    [ -z "$END_STATUS" ] || printf '  çalışma ağacı: %s\n' "$(printf '%s' "$END_STATUS" | head -5 | tr '\n' ' ')"
  } >> "$SUM"
  cat "$SUM"; echo "günlükler: $OUT" >&2
  exit 3
fi

if [ "$red" = 0 ]; then echo "SONUÇ: YEŞİL ($n/$n)" >> "$SUM"; rcode=0
else echo "SONUÇ: KIRMIZI ($red kırmızı: adım ve/veya CI-DRIFT)" >> "$SUM"; rcode=1; fi
cat "$SUM"
echo "günlükler: $OUT" >&2
exit "$rcode"
