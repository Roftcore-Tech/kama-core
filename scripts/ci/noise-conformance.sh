#!/usr/bin/env bash
# KM-SEAM-TC-001 Transport↔Crypto (S3) conformance-core koşucusu — hedef seçicili.
#
#   scripts/ci/noise-conformance.sh --target mock|real|machine|identity
#
# identity = PoC IdentitySeam (no-op mühür): H-CTX'in bağlam-kaydı ve no-op yarısı.
# Kapsama beyanı: scripts/ci/noise-seam-coverage.json (mock koşusunda çapraz denetlenir).
#
# machine = MEVCUT epoch_machine.rs + test tutkalı (A3.2 epoch matrisi). mock/real ayrımının dışındadır:
# MockCrypto epoch geçişini modellemez. Beyanlı kırmızı hücreler EPOCH-RED-CELL satırlarıyla basılır.
#
# Süit: crates/kama-noise-wire/tests/noise_conformance_core/ (satır gövdeleri hedef-agnostik).
#
# ÇIKIŞ KODLARI (seam-conformance.sh ile aynı sözlük)
#   0  yeşil
#   1  kırmızı (test kırmızı · vakum: 0 test koştu)
#   2  ortam/kullanım hatası
#   3  BEYAN EDİLMİŞ BOŞLUK — istenen hedef bu derlemede yok (bugün: --target real → GAP-S3-REAL-A5).
#      Yeşil DEĞİLDİR; "atlandı" diye bir sonuç yoktur.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 2

PKG="kama-noise-wire"
BIN="noise_conformance_core"
GAP="GAP-S3-REAL-A5"
COVERAGE="scripts/ci/noise-seam-coverage.json"

TARGET=""
while (($#)); do
  case "$1" in
    --target) TARGET="${2:-}"; shift 2 || true ;;
    --target=*) TARGET="${1#*=}"; shift ;;
    *) echo "kullanım hatası: bilinmeyen argüman $1"; exit 2 ;;
  esac
done
case "$TARGET" in
  mock|real|machine|identity) ;;
  *) echo "kullanım hatası: --target mock|real|machine|identity zorunlu"; exit 2 ;;
esac

# Hedefin süitleri `<hedef>_` önekli modüllerdir; kayıtlı değilse listede hiç test çıkmaz.
listed=$(cargo test -p "$PKG" --locked --test "$BIN" -- --list "${TARGET}_" 2>/dev/null \
  | grep -c ': test$' || true)
if [[ "$listed" -eq 0 ]]; then
  if [[ "$TARGET" == "real" ]]; then
    echo "BEYAN EDİLMİŞ BOŞLUK: hedef=real kayıtlı değil ($GAP — gerçek kripto A5'te)"
    exit 3
  fi
  echo "::error title=NOISE-CONFORMANCE::hedef=$TARGET için 0 test listelendi (vakum)"
  exit 1
fi

# Kapsama beyanı ↔ gerçeklik: 'mock: green' denen her satırın en az bir testi LİSTELENMELİ.
if [[ "$TARGET" == "mock" ]]; then
  names=$(cargo test -p "$PKG" --locked --test "$BIN" -- --list "mock_" 2>/dev/null | grep ': test$')
  NAMES="$names" python3 - "$COVERAGE" <<'PY' || exit 1
import json, os, sys
rows = json.load(open(sys.argv[1], encoding="utf-8"))["rows"]
names = os.environ["NAMES"]
bad = [r["id"] for r in rows if r["mock"] == "green" and (not r["test_filter"] or r["test_filter"] not in names)]
if bad:
    print(f"::error title=NOISE-CONFORMANCE::kapsama beyanı 'green' ama test yok: {bad}")
    sys.exit(1)
print(f"kapsama beyanı tutarlı: {sum(r['mock'] == 'green' for r in rows)} satır green, "
      f"{sum(r['mock'] != 'green' for r in rows)} satır beklemede")
PY
fi

out=$(cargo test -p "$PKG" --locked --test "$BIN" -- "${TARGET}_" --nocapture 2>&1)
rc=$?
echo "$out" | grep -E '^test |^test result|EPOCH-RED-CELL'
if [[ $rc -ne 0 ]]; then
  echo "::error title=NOISE-CONFORMANCE::hedef=$TARGET kırmızı"
  exit 1
fi
echo "$out" | grep -Eq "test result: ok\. ${listed} passed" \
  || { echo "::error title=NOISE-CONFORMANCE::listelenen ($listed) ≠ koşan test sayısı"; exit 1; }
# --nocapture çıktısı test satırlarıyla iç içe geçebilir: satır başına değil, geçiş sayısına bak.
red=$(echo "$out" | grep -o 'EPOCH-RED-CELL' | wc -l)
echo "NOISE-CONFORMANCE: YEŞİL (hedef=$TARGET, $listed test, beyanlı kırmızı hücre: $red)"
