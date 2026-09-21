#!/usr/bin/env bash
# Kama test-vektör doğrulayıcısı — öz-test (T-00.1 / T-00.2).
#
#   scripts/vectors/selftest.sh
#
# Vaka listesi BURADA DEĞİL: scripts/vectors/testdata/EXPECTATIONS.json'dan okunur.
# Aynı dosyayı Rust yükleyicisinin parite testi de okur (crates/kama-vectors/tests/parity.rs),
# böylece iki araç tek tabloya bakar ve tablo iki yerde ayrı bakım istemez.
#
# Ret vakalarında yalnız "kırmızı mı" değil, BEKLENEN SV- koduyla mı kırmızı olduğu denetlenir —
# yanlış nedenle düşen bir fixture yeşil sayılmaz.
#
# Çıkış kodu: 0 = hepsi beklendiği gibi · 1 = en az bir beklenti tutmadı.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TABLE="$ROOT/scripts/vectors/testdata/EXPECTATIONS.json"
cd "$ROOT" || exit 2

pass=0
fail=0
ok()  { printf 'PASS  %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf 'FAIL  %s\n      %s\n' "$1" "$2"; fail=$((fail + 1)); }

# EXPECTATIONS.json -> "<kind><TAB><code><TAB><label><TAB><yol> <yol>..."
rows="$(python3 - "$TABLE" <<'PY'
import json, sys
table = json.load(open(sys.argv[1], encoding="utf-8"))
for entry in table["accept"]:
    print("accept", "-", entry["label"], " ".join(entry["paths"]), sep="\t")
for entry in table["reject"]:
    extra = ["--max-file-bytes", str(entry["max_file_bytes"])] if "max_file_bytes" in entry else []
    print("reject", entry["code"], entry["label"], " ".join(entry["paths"] + extra), sep="\t")
# python_only: Rust karşılığı olmayan, CLI bayrağına bağlı denetimler.
for entry in table.get("python_only", []):
    kind = "reject" if entry.get("code") else "accept"
    args = " ".join(entry["paths"] + entry.get("args", []))
    print(kind, entry.get("code") or "-", "[python-only] " + entry["label"], args, sep="\t")
PY
)" || { echo "EXPECTATIONS.json okunamadı" >&2; exit 2; }

while IFS=$'\t' read -r kind code label paths; do
  [[ -z "$kind" ]] && continue
  read -r -a argv <<<"$paths"
  out="$(python3 scripts/vectors/validate.py "${argv[@]}" 2>&1)"; rc=$?

  if [[ "$kind" == "accept" ]]; then
    if [[ $rc -eq 0 ]]; then ok "kabul: $label"; else bad "kabul: $label" "çıkış $rc — $out"; fi
  else
    if [[ $rc -eq 0 ]]; then
      bad "ret: $label" "doğrulayıcı KABUL etti (çıkış 0) — sessiz geçiş"
    elif ! grep -q "\[$code\]" <<<"$out"; then
      bad "ret: $label" "reddedildi ama $code kodu yok — $out"
    else
      ok "ret: $label ($code)"
    fi
  fi
done <<<"$rows"

# ── SV-009: yalnız Python tarafı ──
# Rust yükleyici SCHEMA.json'u OKUMAZ (kuralları koda gömülüdür), bu yüzden "şemada
# tanınmayan anahtar sözcük" kavramının Rust karşılığı yoktur ve paylaşılan tabloya
# giremez. Fail-closed davranışı burada ayrıca kanıtlanır: şemaya değerlendiricinin
# bilmediği bir anahtar sözcük enjekte edilir; doğrulayıcı ATLAMAMALI, SV-009 ile durmalı.
mutated="$(mktemp -t kama-schema-XXXXXX.json)"
trap 'rm -f "$mutated"' EXIT
python3 - "$ROOT/vectors/SCHEMA.json" "$mutated" <<'PY'
import json, sys
schema = json.load(open(sys.argv[1], encoding="utf-8"))
schema["properties"]["suite"]["format"] = "hostname"   # alt kümede YOK
json.dump(schema, open(sys.argv[2], "w", encoding="utf-8"), indent=2)
PY
out="$(python3 scripts/vectors/validate.py vectors/ --schema "$mutated" 2>&1)"; rc=$?
if [[ $rc -eq 0 ]]; then
  bad "ret: sema alt-kumesinde olmayan anahtar sozcuk" "doğrulayıcı KABUL etti — kural sessizce zorlanmadan geçti"
elif ! grep -q '\[SV-009\]' <<<"$out"; then
  bad "ret: sema alt-kumesinde olmayan anahtar sozcuk" "reddedildi ama SV-009 yok — $out"
else
  ok "ret: sema alt-kumesinde olmayan anahtar sozcuk (SV-009, fail-closed)"
fi

# ── Şema öz-denetimi: bozuk bir ŞEMA veri hatası değil ARAÇ hatasıdır (rc=2) ──
# Doğrulayıcı SCHEMA.json'a güvenir. Şemadaki bir anahtar sözcüğün DEĞERİ yanlış tipteyse
# eskiden doğrulama ortasında Python istisnasıyla düşüyordu: teşhis yerine yığın izi, üstelik
# veri hatası (rc=1) sınıfında. Aşağıdaki üç sınama şemanın kendi kapısını kanıtlar.
schema_probe() { # <etiket> <beklenen-metin> <python-mutasyonu>
  local label="$1" needle="$2" mutation="$3"
  local mutated; mutated="$(mktemp -t kama-schema-XXXXXX.json)"
  python3 - "$ROOT/vectors/SCHEMA.json" "$mutated" "$mutation" <<'PY'
import json, sys
schema = json.load(open(sys.argv[1], encoding="utf-8"))
exec(sys.argv[3], {"schema": schema})
json.dump(schema, open(sys.argv[2], "w", encoding="utf-8"), indent=2)
PY
  local out rc
  out="$(python3 scripts/vectors/validate.py vectors/ --schema "$mutated" 2>&1)"; rc=$?
  rm -f "$mutated"
  if [[ $rc -ne 2 ]]; then
    bad "sema oz-denetimi: $label" "cikis $rc — ARAC hatasi (2) olmaliydi: $out"
  elif ! grep -qF "$needle" <<<"$out"; then
    bad "sema oz-denetimi: $label" "rc=2 ama beklenen teshis yok ($needle): $out"
  else
    ok "sema oz-denetimi: $label"
  fi
}
schema_probe "anahtar sozcuk degeri yanlis tipte" "int bekleniyordu" \
  "schema['properties']['suite']['maxLength'] = '64'"
schema_probe "\$id ile schema_version ayrismis" "/v2/SCHEMA.json" \
  "schema['\$id'] = 'https://kama.invalid/vectors/SCHEMA.json'"
schema_probe "combinator tuzagi (allOf icinde properties)" "combinator kolu" \
  "schema['\$defs']['case']['allOf'].append({'properties': {'tier': {'type': 'string'}}})"

# ── Çapraz kontrol vakumu: kütüphane yoksa SESSİZCE atlanmamalı ──
stub="$(mktemp -d -t kama-stub-XXXXXX)"
echo "raise ImportError('gizlendi')" > "$stub/jsonschema.py"
if PYTHONPATH="$stub" python3 scripts/vectors/validate.py vectors/ --require-cross-check >/dev/null 2>&1; then
  bad "capraz kontrol vakumu" "jsonschema YOKKEN --require-cross-check yesil dondu"
else
  ok "capraz kontrol: kutuphane yoksa hata (sessiz atlama yok)"
fi
rm -rf "$stub"
if python3 scripts/vectors/validate.py vectors/ --require-cross-check >/dev/null 2>&1; then
  ok "capraz kontrol: bagimsiz oracle ile temiz"
else
  bad "capraz kontrol" "jsonschema ile bulgu cikti"
fi

# ── GitHub Actions anotasyonu: bulgular PR diff'inde dosyaya iliştirilmeli ──
ann="$(python3 scripts/vectors/validate.py \
  scripts/vectors/testdata/reject/hex-uppercase.json --github 2>/dev/null)"
if grep -q '^::error file=scripts/vectors/testdata/reject/hex-uppercase.json,title=SV-006::' <<<"$ann"; then
  ok "CI anotasyonu: dosya + SV- kodu tasiniyor"
else
  bad "CI anotasyonu" "beklenen ::error satiri yok: $ann"
fi

# ── Spec izlenebilirlik ratchet'i: kapsam DÜŞERSE kırmızı olmalı ──
# Kapının kendisi de vakuma düşebilir: taban dosyası bozulur ya da karşılaştırma yanlış
# yazılırsa "kapsam düştü" durumu hiç görülmez. Sahte bir tabanla (kapsanmayan bir vakayı
# kapsanmış sayan) kapının GERÇEKTEN kırmızı olduğu kanıtlanır.
if python3 scripts/vectors/spec_coverage.py >/dev/null 2>&1; then
  ok "spec kapsam ratchet'i: gerçek taban yeşil"
else
  bad "spec kapsam ratchet'i: gerçek taban" "kapsam düşmüş görünüyor"
fi

fake="$(mktemp -t kama-coverage-XXXXXX.json)"
printf '{"total":24,"covered":["INVALID-1","INVALID-2"],"missing":[]}\n' > "$fake"
if python3 scripts/vectors/spec_coverage.py --baseline "$fake" >/dev/null 2>&1; then
  bad "spec kapsam ratchet'i: sahte taban" "kapsam DÜŞÜKKEN kapı yeşil kaldı — vakum"
else
  ok "spec kapsam ratchet'i: kapsam düşünce kırmızı"
fi
rm -f "$fake"

echo
printf 'ÖZ-TEST (python): %d geçti, %d başarısız\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
