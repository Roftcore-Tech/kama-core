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
#   1  kırmızı (test kırmızı · vakum: 0 test koştu · kapsama beyanı gerçeklikle ayrışıyor)
#   2  ortam/kullanım hatası
#   3  BEYAN EDİLMİŞ BOŞLUK — istenen hedef bu derlemede yok (bugün: --target real → GAP-S3-REAL-A5).
#      Yeşil DEĞİLDİR; "atlandı" diye bir sonuç yoktur.
#
# HEDEF → NEREDE KOŞAR [B1 2026-10-05]
#   mock | machine | real : crates/kama-noise-wire/tests/noise_conformance_core (özellik: test-util;
#                           MockCrypto o özelliğin altında, sahte gerçeğinin sahibi crate'te yaşar)
#   identity              : crates/client-core/tests/noise_identity_target.rs (IdentitySeam client-core'un;
#                           kama-noise-wire client-core'u göremez — layer_check C3)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 2

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
  mock|real|machine) CARGO_ARGS=(-p kama-noise-wire --locked --features test-util --test noise_conformance_core) ;;
  identity)          CARGO_ARGS=(-p client-core --locked --test noise_identity_target) ;;
  *) echo "kullanım hatası: --target mock|real|machine|identity zorunlu"; exit 2 ;;
esac

# Hedefin testleri `<hedef>_` önekli ad taşır; kayıtlı değilse listede hiç test çıkmaz.
names=$(cargo test "${CARGO_ARGS[@]}" -- --list "${TARGET}_" 2>/dev/null | grep ': test$' || true)
listed=$(printf '%s' "$names" | grep -c ': test$' || true)
if [[ "$listed" -eq 0 ]]; then
  if [[ "$TARGET" == "real" ]]; then
    echo "BEYAN EDİLMİŞ BOŞLUK: hedef=real kayıtlı değil ($GAP — gerçek kripto A5'te)"
    exit 3
  fi
  echo "::error title=NOISE-CONFORMANCE::hedef=$TARGET için 0 test listelendi (vakum)"
  exit 1
fi

# Kapsama beyanı ↔ gerçeklik (yalnız mock koşusunda; beyan tek yerde ölçülür):
#   · 'green' + test_filter  → o filtreye uyan en az bir test LİSTELENMELİ
#   · proof = trybuild       → adı verilen test default-feature koşumunda KOŞMALI ve geçmeli
#                              (trybuild kapısı `test-util` KAPALIYKEN derlenir)
#   · proof = layer_check    → scripts/ci/layer_check.py yeşil olmalı
if [[ "$TARGET" == "mock" ]]; then
  plan=$(NAMES="$names" python3 - "$COVERAGE" <<'PY'
import json, os, sys
rows = json.load(open(sys.argv[1], encoding="utf-8"))["rows"]
names = os.environ["NAMES"]
bad, trybuild, static = [], [], 0
for r in rows:
    proof = r.get("proof")
    if proof == "trybuild":
        if not r.get("proof_test"):
            bad.append(f"{r['id']}: proof=trybuild ama proof_test yok")
        else:
            trybuild.append(r["proof_test"])
    elif proof == "layer_check":
        static += 1
    elif r["mock"] == "green":
        if not r["test_filter"] or r["test_filter"] not in names:
            bad.append(f"{r['id']}: 'green' ama '{r['test_filter']}' filtresine uyan test yok")
if bad:
    print("BAD " + " | ".join(bad))
    sys.exit(0)
green = sum(r["mock"] == "green" for r in rows)
print(f"OK green={green} static={static} pending={len(rows) - green - static} trybuild={','.join(trybuild)}")
PY
)
  if [[ "$plan" != OK* ]]; then
    echo "::error title=NOISE-CONFORMANCE::kapsama beyanı gerçeklikle ayrışıyor: ${plan#BAD }"
    exit 1
  fi
  echo "kapsama beyanı: ${plan#OK }"
  tb=$(sed -n 's/.*trybuild=\([^ ]*\).*/\1/p' <<<"$plan")
  if [[ -n "$tb" ]]; then
    tout=$(cargo test -p kama-noise-wire --locked --test trybuild 2>&1); trc=$?
    for t in ${tb//,/ }; do
      grep -Eq "^test ${t} \.\.\. ok$" <<<"$tout" \
        || { echo "::error title=NOISE-CONFORMANCE::trybuild kanıtı koşmadı/geçmedi: $t"; echo "$tout" | tail -15; exit 1; }
      echo "trybuild kanıtı: $t ... ok"
    done
    [[ $trc -eq 0 ]] || { echo "::error title=NOISE-CONFORMANCE::trybuild kırmızı"; echo "$tout" | tail -15; exit 1; }
  fi
  if grep -q '"proof": "layer_check"' "$COVERAGE"; then
    python3 scripts/ci/layer_check.py >/dev/null \
      || { echo "::error title=NOISE-CONFORMANCE::statik satırın kanıtı (layer_check.py) kırmızı"; exit 1; }
    echo "statik kanıt: layer_check.py yeşil"
  fi
fi

out=$(cargo test "${CARGO_ARGS[@]}" -- "${TARGET}_" --nocapture 2>&1)
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
