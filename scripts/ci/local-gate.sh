#!/usr/bin/env bash
# Kama yerel kapı (EK-1 D-17). Hosted CI fatura ya da altyapı yüzünden başlamazsa merge'e yeten kapı
# budur (CLAUDE.md §7). `.github/workflows/ci.yml` ve `spec-word-coverage.yml`'deki PR adımlarını
# AYNI komutlarla (ci.yml'deki ::error ileti sarmalayıcıları hariç), ci.yml'deki sırayla, tek cargo süreciyle koşar.
#
# Kullanım:
#   scripts/ci/local-gate.sh [--out DİZİN] [--base REF]
#     --out   çıktı dizini (varsayılan: ~/.cache/kama-local-gate/<kısa-sha>-<zaman>)
#     --base  karşılaştırma tabanı (varsayılan: git merge-base HEAD origin/main);
#             conformance teşhisi (CONF_DIFF_BASE) ve silme kapısı (--base) bunu kullanır
#   scripts/ci/local-gate.sh --list    adımları yazdırır, koşmaz
#
# Kurallar:
#   * Toolchain rust-toolchain.toml'dan gelir (1.96.0). CARGO_BUILD_JOBS=2.
#   * Tek cargo süreci: betik başta D-34 kilidini alır (scripts/ci/cargo-lock.sh ile aynı kilit dosyası)
#     ve sonuna dek tutar. Kilit alınamazsa bekler.
#   * Kırmızıda DURMAZ; bütün adımları koşar, sonra çıkış kodu = kırmızı adım sayısı (0 = yeşil).
#   * Hosted CI'a kalan adımlar (D-17, adıyla) burada KOŞMAZ ve özette öyle yazılır:
#     miri (ci.yml miri), ASan (asan), MSan (msan), cargo audit (security), aead-differential.
#   * Bu betik ci.yml'in belirli bir sürümünü yansıtır (aşağıdaki CI_SHA256). ci.yml değişmişse betik
#     yine koşar ama özetin ilk satırı "CI-DRIFT" der; o durumda bu dosya aynı PR'da güncellenir.
#
# PR'a yazılacak satır (D-17): "CI-KİLİT İSTİSNASI ile merge: yerel kapı <yeşil/kırmızı> @<sha>,
# özet: <out>/summary.txt" + summary.txt içeriği.
set -u

CI_SHA256=4764f867d74479188488baab7ed4038e05d0d0695914b8296fa48567b9a9f919      # .github/workflows/ci.yml
SWC_SHA256=2907c16c67373aa7e95ecb0e072b9b746bc346b1ba462d56aa63a1c35664ab5d     # .github/workflows/spec-word-coverage.yml
HOSTED_ONLY="miri · asan · msan · cargo audit · aead-differential"

OUT=""; BASE=""; LIST=0
while [ $# -gt 0 ]; do
  case "$1" in
    --out)  OUT=$2; shift 2 ;;
    --base) BASE=$2; shift 2 ;;
    --list) LIST=1; shift ;;
    *) echo "bilinmeyen argüman: $1" >&2; exit 2 ;;
  esac
done

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "git ağacı değil" >&2; exit 2; }
cd "$ROOT" || exit 2

# ── Adımlar: ad | komut. Komut metinleri ci.yml'deki `run:` satırlarıyla aynıdır. ──────────────────
# ci.yml'de `/tmp/pp.txt` ve `miri.log` gibi sabit yollar $OUT altına alınmıştır; başka fark yoktur.
STEPS=()
add() { STEPS+=("$1|$2"); }
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

if [ "$LIST" = 1 ]; then
  i=0; for s in "${STEPS[@]}"; do i=$((i+1)); printf '%02d %-17s %s\n' "$i" "${s%%|*}" "${s#*|}"; done
  echo "hosted'a kalan (koşmaz): $HOSTED_ONLY"
  exit 0
fi

[ -n "$BASE" ] || BASE=$(git merge-base HEAD origin/main 2>/dev/null) || { echo "taban bulunamadı (origin/main?); --base verin" >&2; exit 2; }
HEAD_SHA=$(git rev-parse HEAD)
[ -n "$OUT" ] || OUT="$HOME/.cache/kama-local-gate/${HEAD_SHA:0:8}-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUT" || exit 2
export OUT BASE
export CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never CARGO_INCREMENTAL=0 RUST_BACKTRACE=1

LOCK=${KAMA_CARGO_LOCK:-$HOME/.cache/kama-cargo.lock}
mkdir -p "$(dirname "$LOCK")"
exec 9>"$LOCK"
echo "D-34 kilidi bekleniyor: $LOCK" >&2
flock 9

SUM="$OUT/summary.txt"; : > "$SUM"
drift=""
[ "$(sha256sum .github/workflows/ci.yml | cut -d' ' -f1)" = "$CI_SHA256" ] || drift="ci.yml"
[ "$(sha256sum .github/workflows/spec-word-coverage.yml | cut -d' ' -f1)" = "$SWC_SHA256" ] || drift="${drift:+$drift, }spec-word-coverage.yml"
if [ -n "$drift" ]; then
  echo "CI-DRIFT: $drift bu betiğin yansıttığı sürümden farklı — local-gate.sh güncellenmeli" >> "$SUM"
fi
{
  echo "HEAD $HEAD_SHA $(git log -1 --format=%s)"
  echo "BASE $BASE"
  echo "toolchain $(rustc --version 2>/dev/null) · CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS"
  if [ -n "$(git status --porcelain --untracked-files=no)" ]; then echo "UYARI: izlenen dosyalarda commit'lenmemiş değişiklik var"; fi
} >> "$SUM"

n=0; red=0
for s in "${STEPS[@]}"; do
  n=$((n+1)); name=${s%%|*}; cmd=${s#*|}
  log=$(printf '%s/%02d-%s.log' "$OUT" "$n" "$name")
  t0=$SECONDS
  bash -c "$cmd" > "$log" 2>&1; rc=$?
  [ "$rc" = 0 ] || red=$((red+1))
  printf '%02d %-17s exit=%-3s %5ss\n' "$n" "$name" "$rc" "$((SECONDS-t0))" >> "$SUM"
done
echo "hosted'a kalan (koşmadı): $HOSTED_ONLY" >> "$SUM"
if [ "$red" = 0 ]; then echo "SONUÇ: YEŞİL ($n/$n)" >> "$SUM"; else echo "SONUÇ: KIRMIZI ($red/$n adım)" >> "$SUM"; fi
cat "$SUM"
echo "günlükler: $OUT" >&2
exit "$red"
