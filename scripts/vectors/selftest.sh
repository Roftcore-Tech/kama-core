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
    print("reject", entry["code"], entry["label"], " ".join(entry["paths"]), sep="\t")
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

echo
printf 'ÖZ-TEST (python): %d geçti, %d başarısız\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
