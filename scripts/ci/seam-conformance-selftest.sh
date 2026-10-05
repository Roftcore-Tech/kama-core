#!/usr/bin/env bash
# seam conformance kapisinin OZ-TESTI.
#
#   scripts/ci/seam-conformance-selftest.sh
#
# Muhafiz yazmak yetmez. Bu repoda olculmus asil hata sinifi "muhafiz var ama hicbir sey olcmuyor"
# (BUL-02/BUL-03: 0 test kosan kapi; ci-verdict'te `skipped` yesil sayiliyordu). Bu betik her
# muhafizi GERCEKTEN kirmizi yapip gosterir, sonra agaci eski haline dondurur (trap ile HER durumda).
#
#   A. Kirmizi test       — basarisiz bir suit testi → kapi 1
#   B. Vakum              — test hedefi var ama icinde test yok → kapi 1 (3 DEGIL: bosluk sanilmaz)
#   C. Kapsam yalani      — kod §6 satiri iddia ediyor, beyan etmiyor → kapi 1
#   D. Ratchet tutarsizligi — beyan tabani gerceklikten yuksek → kapi 1
#   E. Bosluk kapanisi    — gercek hedef kaydedilince "bosluk beyani" testi → kapi 1
#   F. API drift          — sozlesme kaynagi spec'ten ayrisinca (metot silinince) drift kapisi → 1
#                           (S1 ve S3 icin ayri ayri; kaynak = crates/kama-contracts)
#   G. Hedef parametresi  — gecersiz/eksik hedef → 2 · mevcut olmayan hedef → 3 · mock → 0
#   H. Kategori sizintisi — --target mock, real-adapter ikilisini KOSMAZ
#   I. Kanit enflasyonu   — tek hedef varken satir 'implemented' beyan edilirse → 1
#   J. Sessiz kapsam kaybi— kosan bir satir CONTRACT_ROWS'tan silinirse → 1
#
# Cikis kodu: 0 = on muhafiz da calisiyor · 1 = en az biri vakumda.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 2

GATE="scripts/ci/seam-conformance.sh"
DRIFT="scripts/ci/seam-api-drift.py"
SEAM="crates/kama-integration/tests/common/seam"
TESTS="crates/kama-integration/tests"
CONTRACTS="crates/kama-contracts/src"

MUTABLE=(
  "$SEAM/target.rs"
  "$SEAM/contract.rs"
  "$TESTS/seam_conformance_core/main.rs"
  "$TESTS/seam_mock_fixture/main.rs"
  "$TESTS/seam_real_adapter/main.rs"
  "scripts/ci/seam-coverage.json"
)

BACKUP="$(mktemp -d -t seam-selftest-bak-XXXXXX)"
TMPD="$(mktemp -d -t seam-selftest-tmp-XXXXXX)"
for f in "${MUTABLE[@]}"; do
  mkdir -p "$BACKUP/$(dirname "$f")"
  cp "$f" "$BACKUP/$f"
done
restore() { for f in "${MUTABLE[@]}"; do cp "$BACKUP/$f" "$f"; done; }
cleanup() { restore; rm -rf "$BACKUP" "$TMPD"; }
trap cleanup EXIT

pass=0
fail=0
ok()  { printf 'PASS  %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf 'FAIL  %s\n      %s\n' "$1" "$2"; fail=$((fail + 1)); }

# expect <beklenen-kod> <muhafiz-adi> -- <komut...>
expect() {
  local want="$1" name="$2"; shift 3
  local out code
  out="$("$@" 2>&1)"; code=$?
  if [[ "$code" == "$want" ]]; then
    ok "$name (cikis $code)"
  else
    bad "$name" "beklenen cikis $want, gelen $code"
    printf '%s\n' "$out" | tail -12 | sed 's/^/      | /'
  fi
  LAST_OUT="$out"
}

printf '── G. hedef parametresi ──\n'
expect 2 "G1 eksik --target → 2" -- bash "$GATE"
expect 2 "G2 gecersiz hedef → 2" -- bash "$GATE" --target bogus
expect 3 "G3 mevcut olmayan hedef → 3 (beyan edilmis bosluk)" -- bash "$GATE" --target real
expect 0 "G4 mock hedefi → 0" -- bash "$GATE" --target mock

printf '\n── H. kategori sizintisi ──\n'
if grep -q 'seam_real_adapter' <<<"$LAST_OUT"; then
  bad "H1 --target mock real-adapter'i kosmamali" "mock kosumunda seam_real_adapter gorundu"
else
  ok "H1 --target mock real-adapter'i kosmuyor"
fi

printf '\n── A. kirmizi test ──\n'
cat >> "$TESTS/seam_mock_fixture/main.rs" <<'PROBE'

#[test]
fn selftest_probe_must_fail() {
    assert!(false, "oz-test probe'u — kapinin kirmizi oldugunu kanitlar");
}
PROBE
expect 1 "A1 basarisiz suit testi → 1" -- bash "$GATE" --target mock
restore

printf '\n── B. vakum ──\n'
cat > "$TESTS/seam_real_adapter/main.rs" <<'VACUUM'
//! oz-test: testsiz hedef (vakum probe'u).
#[path = "../common/seam/harness.rs"]
mod harness;
#[allow(dead_code)]
fn unused() -> bool { harness::target::REGISTERED_TARGETS.is_empty() }
VACUUM
expect 1 "B1 testsiz hedef → 1 (bosluk/3 ile karistirilmaz)" -- bash "$GATE" --target real
restore

printf '\n── C. kapsam yalani ──\n'
# Kod, beyanda OLMAYAN bir satir kimligi iddia eder. (Capa regex'tir: CONTRACT_ROWS'a satir
# eklendikce bu oz-test kirilmasin; kimlik uydurmadir ki hicbir zaman gercek bir satirla cakismasin.)
python3 - "$SEAM/contract.rs" <<'PY'
import re, sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
m = re.search(r'pub const CONTRACT_ROWS: &\[&str\] = &\[(.*?)\];', s, re.S)
assert m, "CONTRACT_ROWS capasi bulunamadi"
inner = m.group(1).strip().rstrip(",")
new = f'pub const CONTRACT_ROWS: &[&str] = &[{inner}, "H-SELFTEST-PROBE"];'
p.write_text(s[:m.start()] + new + s[m.end():], encoding="utf-8")
PY
expect 1 "C1 kod beyansiz §6 satiri iddia ediyor → 1" -- bash "$GATE" --target mock
restore

printf '\n── J. sessiz kapsam kaybi ──\n'
# Beyan 'mock_only' diyor ama govde silinmis: kimse fark etmeden kapsam duser.
python3 - "$SEAM/contract.rs" <<'PY'
import re, sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
m = re.search(r'pub const CONTRACT_ROWS: &\[&str\] = &\[(.*?)\];', s, re.S)
assert m and m.group(1).strip(), "CONTRACT_ROWS capasi bulunamadi ya da zaten bos"
p.write_text(s[:m.start()] + 'pub const CONTRACT_ROWS: &[&str] = &[];' + s[m.end():], encoding="utf-8")
PY
expect 1 "J1 kosan satir beyandan dusurulunce → 1" -- bash "$GATE" --target mock
restore

printf '\n── I. kanit enflasyonu ──\n'
# Govde yalniz sahtede kostu; beyan 'iki hedefte kanitlandi' diyor.
python3 - scripts/ci/seam-coverage.json <<'PY'
import json, sys, pathlib
p = pathlib.Path(sys.argv[1]); d = json.loads(p.read_text(encoding="utf-8"))
for r in d["rows"]:
    if r["id"] == "H-ROUNDTRIP":
        r["status"] = "implemented"
# Tabanlar mutasyon SONRASI gerceklige esitlenir: kapi yalniz "tek hedefken implemented" yuzunden
# kirmizi olmali, ratchet tutarsizligindan degil.
d["implemented_baseline"] = sum(r["status"] == "implemented" for r in d["rows"])
d["mock_only_baseline"] = sum(r["status"] == "mock_only" for r in d["rows"])
p.write_text(json.dumps(d, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY
expect 1 "I1 tek hedefken 'implemented' beyani → 1" -- bash "$GATE" --target mock
restore

printf '\n── D. ratchet tutarsizligi ──\n'
python3 - scripts/ci/seam-coverage.json <<'PY'
import json, sys, pathlib
p = pathlib.Path(sys.argv[1]); d = json.loads(p.read_text(encoding="utf-8"))
d["implemented_baseline"] = 1          # gerceklik 0 uygulanmis satir
p.write_text(json.dumps(d, ensure_ascii=False, indent=2), encoding="utf-8")
PY
expect 1 "D1 beyan tabani gerceklikten yuksek → 1" -- bash "$GATE" --target mock
restore

printf '\n── E. bosluk kapanisi ──\n'
python3 - "$SEAM/target.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
old = 'pub const REGISTERED_TARGETS: &[(&str, TargetKind)] = &[(MockTarget::ID, TargetKind::Mock)];'
new = ('pub const REGISTERED_TARGETS: &[(&str, TargetKind)] =\n'
       '    &[(MockTarget::ID, TargetKind::Mock), ("real", TargetKind::Real)];')
assert old in s, "REGISTERED_TARGETS capasi bulunamadi"
p.write_text(s.replace(old, new), encoding="utf-8")
PY
expect 1 "E1 gercek hedef kaydedilince bosluk beyani → 1" -- bash "$GATE" --target real
restore

printf '\n── F. API drift ──\n'
# Sozlesme kaynaginin KOPYASI bozulur (agac disi); kapi o kopyaya karsi kosturulur.
cp "$CONTRACTS/s1.rs" "$TMPD/s1-drifted.rs"
python3 - "$TMPD/s1-drifted.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
assert "fn recv(&mut self) -> Option<Delivered>;" in s
p.write_text(s.replace("fn recv(&mut self) -> Option<Delivered>;", ""), encoding="utf-8")
PY
expect 1 "F1 S1 kaynagindan metot silinince drift → 1" -- \
  python3 "$DRIFT" --seam s1 --mirror "$TMPD/s1-drifted.rs" --mirror "$CONTRACTS/handles.rs" --quiet
expect 0 "F2 S1 bozulmamis kaynak → 0 (F1'in kirmizisi mutasyondan)" -- \
  python3 "$DRIFT" --seam s1 --mirror "$CONTRACTS/s1.rs" --mirror "$CONTRACTS/handles.rs" --quiet

cp "$CONTRACTS/s3.rs" "$TMPD/s3-drifted.rs"
python3 - "$TMPD/s3-drifted.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
old = "    fn open(&mut self, wire: &[u8]) -> Option<Opened>;"
assert old in s
p.write_text(s.replace(old, ""), encoding="utf-8")
PY
expect 1 "F3 S3 kaynagindan metot silinince drift → 1" -- \
  python3 "$DRIFT" --seam s3 --mirror "$TMPD/s3-drifted.rs" --mirror "$CONTRACTS/handles.rs" --quiet
expect 0 "F4 S3 bozulmamis kaynak → 0" -- \
  python3 "$DRIFT" --seam s3 --mirror "$CONTRACTS/s3.rs" --mirror "$CONTRACTS/handles.rs" --quiet

# Ortak tutamac dosyasi verilmezse §1'de bildirilen SessionHandle/PeerRef kaynakta bulunamaz → 1
# (kapinin "tek dosyaya bakip yesil donme" tuzagina dusmedigini gosterir).
expect 1 "F5 S1 handles.rs'siz → 1 (ortak tipler eksik)" -- \
  python3 "$DRIFT" --seam s1 --mirror "$CONTRACTS/s1.rs" --quiet

printf '\n'
printf 'oz-test: %d gecti, %d kaldi\n' "$pass" "$fail"
if ((fail)); then
  printf '::error title=SEAM-SELFTEST::seam conformance kapisinin %d muhafizi vakumda\n' "$fail"
  exit 1
fi
printf 'SEAM-SELFTEST: on muhafiz da kirmizi olabiliyor\n'
exit 0
