#!/usr/bin/env bash
# conformance kapısının ÖZ-TESTİ (T-00.3).
#
#   scripts/ci/conformance-selftest.sh
#
# Kapının üç muhafızının GERÇEKTEN kırmızı olduğunu kanıtlar. Muhafız yazmak yetmiyor:
# bu oturumda tekrar tekrar görüldü ki asıl hata sınıfı "muhafız var ama hiçbir şey ölçmüyor".
#
#   A. Ratchet   — conf_* testi eklenip taban güncellenmezse KIRMIZI
#   B. Koşum     — taban doğruyken test GERÇEKTEN koşar (0 değil)
#   C. Vakum     — conf_* dosyası var ama içinde test yoksa KIRMIZI
#   D. Başarı    — BAŞARISIZ bir conf_* testi KIRMIZI  (kapının ASIL amacı)
#   E. Kayıp     — conf_* silinip taban bayat kalırsa KIRMIZI (sessiz kapsam kaybı yok)
#   F. Teşhis    — CONF_DIFF_BASE geçersizken kapı YİNE koşar (teşhis atlanır, kapı atlanmaz)
#   G. Veri      — conf_*.rs hedefi yokken conf_* veri dosyası varsa KIRMIZI
#
# Geçici bir conformance testi yazar ve HER DURUMDA temizler (trap).
# Çıkış kodu: 0 = üç muhafız da çalışıyor · 1 = en az biri vakumda.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 2

PROBE="crates/kama-vectors/tests/conf_selftest_probe.rs"
DATA_PROBE="crates/kama-vectors/tests/conf_selftest_probe.json"
COPY="$(mktemp -t kama-conf-copy-XXXXXX.sh)"
cleanup() { rm -f "$PROBE" "$DATA_PROBE" "$COPY"; }
trap cleanup EXIT

pass=0; fail=0
ok()  { printf 'PASS  %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf 'FAIL  %s\n      %s\n' "$1" "$2"; fail=$((fail + 1)); }

# gate <beklenen-taban> -> cikis kodu + cikti
gate() {
  sed "s/^CONFORMANCE_TARGETS_EXPECTED=.*/CONFORMANCE_TARGETS_EXPECTED=$1/" \
    scripts/ci/conformance.sh > "$COPY"
  CONF_ROOT="$ROOT" bash "$COPY" 2>&1
}

printf '# gercek taban degeri: %s\n\n' \
  "$(sed -n 's/^CONFORMANCE_TARGETS_EXPECTED=//p' scripts/ci/conformance.sh)"

cat > "$PROBE" <<'RS'
// conformance-selftest.sh tarafından yazılır ve silinir. Repoda kalıcı DEĞİLDİR.
#[test]
fn probe() {
    assert_eq!(kama_vectors::hex("00").unwrap(), vec![0u8]);
}
RS

# ── A. Ratchet ──
out="$(gate 0)"; rc=$?
if [[ $rc -eq 0 ]]; then
  bad "A ratchet" "conf_* testi eklendi, taban 0 kaldı ama kapı YEŞİL döndü"
elif ! grep -q "tabandan farklı" <<<"$out"; then
  bad "A ratchet" "kırmızı ama gerekçe ratchet değil: $(tail -3 <<<"$out")"
else
  ok "A ratchet: yeni conf_* testi tabanı güncellemeden geçemiyor"
fi

# ── B. Koşum ──
out="$(gate 1)"; rc=$?
if [[ $rc -ne 0 ]]; then
  bad "B koşum" "taban doğruyken kapı kırmızı: $(tail -5 <<<"$out")"
elif ! grep -Eq "test result: ok\. [1-9]" <<<"$out"; then
  bad "B koşum" "yeşil ama testin koştuğuna dair kanıt yok"
else
  ok "B koşum: hedef keşfediliyor ve gerçekten koşuyor"
fi

# ── C. Vakum ──
printf '// test YOK — vakum muhafızı fixture'\''ı\n' > "$PROBE"
out="$(gate 1)"; rc=$?
if [[ $rc -eq 0 ]]; then
  bad "C vakum" "conf_* dosyası var, içinde test yok ama kapı YEŞİL döndü"
elif ! grep -q "0 test koştu" <<<"$out"; then
  bad "C vakum" "kırmızı ama gerekçe vakum değil: $(tail -3 <<<"$out")"
else
  ok "C vakum: testsiz conf_* dosyası geçemiyor"
fi

# ── D. Başarısız test ── kapının ASIL amacı; T-00.3'te ilk yazıldığında SINANMAMIŞTI.
cat > "$PROBE" <<'RS'
#[test]
fn deliberately_failing() {
    assert_eq!(kama_vectors::hex("00").unwrap(), vec![0xffu8]);
}
RS
out="$(gate 1)"; rc=$?
if [[ $rc -eq 0 ]]; then
  bad "D başarı" "BAŞARISIZ conformance testi varken kapı YEŞİL döndü"
else
  ok "D başarı: başarısız conformance testi kapıyı kırmızı yapıyor"
fi

# ── E. Kapsam kaybı ── test silindi, taban bayat.
rm -f "$PROBE"
out="$(gate 1)"; rc=$?
if [[ $rc -eq 0 ]]; then
  bad "E kayıp" "conf_* silindi, taban 1 kaldı ama kapı YEŞİL döndü"
elif ! grep -q "tabandan farklı" <<<"$out"; then
  bad "E kayıp" "kırmızı ama gerekçe ratchet değil"
else
  ok "E kayıp: silinen conf_* sessizce geçmiyor"
fi

# ── F. Teşhis çökerse kapı çökmemeli ──
out="$(CONF_ROOT="$ROOT" CONF_DIFF_BASE=boyle-bir-ref-yok bash scripts/ci/conformance.sh 2>&1)"; rc=$?
if [[ $rc -ne 0 ]]; then
  bad "F teşhis" "geçersiz CONF_DIFF_BASE kapıyı düşürdü"
elif ! grep -q "teşhis atlandı" <<<"$out"; then
  bad "F teşhis" "geçersiz taban sessizce yutuldu"
else
  ok "F teşhis: diff tabanı yoksa teşhis atlanır, KAPI atlanmaz"
fi

# ── G. Okuyucusuz veri ──
printf '{"not": "bir cargo hedefi degil"}\n' > "$DATA_PROBE"
out="$(gate 0)"; rc=$?
if [[ $rc -eq 0 ]]; then
  bad "G veri" "conf_*.rs hedefi yokken conf_* veri dosyası sessizce yok sayıldı"
else
  ok "G veri: okuyucusu olmayan conf_* veri dosyası kapıyı kırmızı yapıyor"
fi
rm -f "$DATA_PROBE"

printf '\nÖZ-TEST (conformance): %d geçti, %d başarısız\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
