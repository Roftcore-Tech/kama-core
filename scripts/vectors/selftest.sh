#!/usr/bin/env bash
# Kama test-vektör doğrulayıcısı — öz-test (T-00.1).
#
#   scripts/vectors/selftest.sh
#
# Üç şeyi kanıtlar:
#   1. vectors/ ağacı (örnek pozitif + örnek negatif dosya dâhil) KABUL ediliyor.
#   2. testdata/accept/* KABUL ediliyor.
#   3. testdata/reject/* dosyalarının HER BİRİ ayrı ayrı REDDEDİLİYOR — schema_version: 2
#      dâhil. Beklenen doğrulayıcı hata kodu dosya adına göre ayrıca denetlenir, yani
#      "yanlış nedenle kırmızı" bir fixture yeşil sayılmaz.
#   4. id tekilliği DOSYALAR ARASI: tek başına geçen iki dosya birlikte verildiğinde kırmızı.
#
# Çıkış kodu: 0 = hepsi beklendiği gibi · 1 = en az bir beklenti tutmadı.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VALIDATE=(python3 "$ROOT/scripts/vectors/validate_vectors.py")
DATA="$ROOT/scripts/vectors/testdata"

pass=0
fail=0

ok()   { printf 'PASS  %s\n' "$1"; pass=$((pass + 1)); }
bad()  { printf 'FAIL  %s\n     %s\n' "$1" "$2"; fail=$((fail + 1)); }

# expect_accept <etiket> <yol...>
expect_accept() {
  local label="$1"; shift
  local out rc
  out="$("${VALIDATE[@]}" "$@" 2>&1)"; rc=$?
  if [[ $rc -eq 0 ]]; then ok "kabul: $label"; else bad "kabul: $label" "çıkış $rc — $out"; fi
}

# expect_reject <beklenen-kod> <etiket> <yol...>
expect_reject() {
  local code="$1" label="$2"; shift 2
  local out rc
  out="$("${VALIDATE[@]}" "$@" 2>&1)"; rc=$?
  if [[ $rc -eq 0 ]]; then
    bad "ret: $label" "doğrulayıcı KABUL etti (çıkış 0) — sessiz geçiş"
  elif ! grep -q "\[$code\]" <<<"$out"; then
    bad "ret: $label" "reddedildi ama $code kodu yok — $out"
  else
    ok "ret: $label ($code)"
  fi
}

echo "== 1. gerçek vectors/ ağacı =="
expect_accept "vectors/ (örnekler dâhil)" "$ROOT/vectors/"

echo "== 2. kabul fixture'ları =="
expect_accept "testdata/accept/" "$DATA/accept/"

echo "== 3. ret fixture'ları =="
expect_reject SV-004 "schema_version: 2 (GÖREV DONE KRİTERİ)" "$DATA/reject/schema-version-2.json"
expect_reject SV-003 "schema_version yok"                     "$DATA/reject/schema-version-missing.json"
expect_reject SV-003 "schema_version dizge"                   "$DATA/reject/schema-version-string.json"
expect_reject SV-002 "yinelenen JSON anahtarı"                "$DATA/reject/duplicate-json-key.json"
expect_reject SV-006 "hex büyük harf"                         "$DATA/reject/hex-uppercase.json"
expect_reject SV-006 "hex 0x öneki"                           "$DATA/reject/hex-0x-prefix.json"
expect_reject SV-006 "hex tek uzunluk"                        "$DATA/reject/hex-odd-length.json"
expect_reject SV-005 "input + input_hex birlikte"             "$DATA/reject/both-input-forms.json"
expect_reject SV-005 "input yok"                              "$DATA/reject/no-input.json"
expect_reject SV-005 "expected + expected_hex birlikte"       "$DATA/reject/both-expected-forms.json"
expect_reject SV-005 "result üçlü dışında"                    "$DATA/reject/bad-result-value.json"
expect_reject SV-005 "reason yok"                             "$DATA/reject/expected-missing-reason.json"
expect_reject SV-005 "bilinmeyen üst-düzey alan"              "$DATA/reject/unknown-top-level-field.json"
expect_reject SV-005 "bilinmeyen vaka alanı"                  "$DATA/reject/unknown-case-field.json"
expect_reject SV-005 "cases boş"                              "$DATA/reject/empty-cases.json"
expect_reject SV-005 "suite büyük harf"                       "$DATA/reject/uppercase-suite.json"

echo "== 4. id tekilliği dosyalar arası =="
expect_accept "dup-id-a tek başına" "$DATA/reject/dup-id-a.json"
expect_accept "dup-id-b tek başına" "$DATA/reject/dup-id-b.json"
expect_reject SV-007 "dup-id-a + dup-id-b birlikte" \
  "$DATA/reject/dup-id-a.json" "$DATA/reject/dup-id-b.json"

echo
printf 'ÖZ-TEST: %d geçti, %d başarısız\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
