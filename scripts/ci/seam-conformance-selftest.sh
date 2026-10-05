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
#   F. API drift          — sozlesme kaynagi spec'ten ayrisinca drift kapisi → 1 (S1 ve S3 icin ayri ayri;
#                           kaynak = crates/kama-contracts). F1–F5: AD duzeyi (metot/tip var mi).
#                           F6–F11: BICIM duzeyi — adlar ayni kalirken variant kumesi, variant'in tasidigi
#                           deger sayisi, struct alan adi ya da metot imzasi degisirse → 1 (nedeniyle)
#   G. Hedef parametresi  — gecersiz/eksik hedef → 2 · mevcut olmayan hedef → 3 · mock → 0
#   H. Kategori sizintisi — --target mock, real-adapter ikilisini KOSMAZ
#   I. Kanit enflasyonu   — tek hedef varken satir 'implemented' beyan edilirse → 1
#   J. Sessiz kapsam kaybi— kosan bir satir CONTRACT_ROWS'tan silinirse → 1
#   K. Hayalet satir      — satir CONTRACT_ROWS'ta ve beyanda duruyor ama #[test] sarmalayicisi yok → 1
#   L. Statik kanit       — statik satirlarin kaniti (layer_check.py) kirmiziyken → 1
#   M. Statik enflasyon   — davranis satiri 'static' beyan edilirse (kosmadan kanitli gorunur) → drift 1;
#                           `kind` de birlikte cevrilse ve uydurma kural adi yazilsa bile (tur spec'ten okunur)
#   N. Susturulmus satir  — satirin sarmalayicisina #[ignore] konursa (listelenir ama kosmaz) → 1
#
# K/L/M/N'de kapinin BEKLENEN nedenle kirmizi oldugu da olculur (cikti o nedeni adiyla tasimali);
# baska bir nedenle kirmizi olan kapi, sinanan muhafizin calistigini gostermez.
#
# Cikis kodu: 0 = butun muhafizlar calisiyor · 1 = en az biri vakumda.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT" || exit 2

GATE="scripts/ci/seam-conformance.sh"
DRIFT="scripts/ci/seam-api-drift.py"
SEAM="crates/kama-integration/tests/common/seam"
TESTS="crates/kama-integration/tests"
CONTRACTS="crates/kama-contracts/src"
# L icin: Application agacina konan IZLENMEYEN probe dosyasi (izlenen hicbir dosyaya dokunulmaz).
LAYER_PROBE="crates/kama-app/tests/selftest_layer_probe.rs"

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
cleanup() { restore; rm -f "$LAYER_PROBE"; rm -rf "$BACKUP" "$TMPD"; }
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

# expect_reason <muhafiz-adi> <son ciktida aranacak sabit dizge>
expect_reason() {
  if grep -qF -- "$2" <<<"$LAST_OUT"; then
    ok "$1"
  else
    bad "$1" "kapi kirmizi ama beklenen nedenle degil — ciktida yok: $2"
    printf '%s\n' "$LAST_OUT" | tail -12 | sed 's/^/      | /'
  fi
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

printf '\n── K. hayalet satir ──\n'
# Satir CONTRACT_ROWS'ta ve beyanda duruyor ama #[test] sarmalayicisi silinmis: "kosuyor" denen
# satir kosmuyor ve bunu kod ↔ beyan karsilastirmasi GOREMEZ (ikisi de ayni seyi soyluyor).
python3 - "$SEAM/contract.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
old = ('            #[test]\n'
       '            fn h_caps() {\n'
       '                contract::rows::h_caps::<Target>();\n'
       '            }\n')
assert s.count(old) == 1, "h_caps sarmalayici capasi bulunamadi"
p.write_text(s.replace(old, ""), encoding="utf-8")
PY
expect 1 "K1 iddia edilen satirin sarmalayicisi silinince → 1" -- bash "$GATE" --target mock
expect_reason "K2 neden: iddia edilen satirin testi yok (H-CAPS)" "listelenen testi OLMAYAN satir(lar): ['H-CAPS']"
restore

printf '\n── L. statik kanit ──\n'
# Statik satirlarin (H-NO-CRYPTO-EDGE, H-NO-PLANE1) kaniti layer_check.py'dir. Application agacina
# yasak bir Plane 1 sembolu tasiyan dosya konur; kapi kirmizi olmali.
printf 'fn method_id() {}\n' > "$LAYER_PROBE"
expect 1 "L1 statik satirin kaniti (layer_check) kirmiziyken → 1" -- bash "$GATE" --target mock
# Neden, layer_check'in probe dosyasi icin bastigi BULGUNUN kendisidir (genel "kirmizi" cumlesi degil):
# layer_check baska bir nedenle dusseydi bu satir gecmezdi.
expect_reason "L2 neden: layer_check probe dosyasindaki yasak sembolu buldu" "selftest_layer_probe.rs: yasak Plane 1 sembolü"
rm -f "$LAYER_PROBE"

printf '\n── M. statik durum enflasyonu ──\n'
# Davranis satiri 'static' beyan edilirse kosmadan "kanitli" gorunurdu. Beyanin KOPYASI bozulur
# (agac disi); tabanlar mutasyon sonrasi gerceklige esitlenir ki kirmizi yalniz bu kuraldan gelsin.
python3 - scripts/ci/seam-coverage.json "$TMPD/coverage-static-inflated.json" <<'PY'
import json, sys, pathlib
d = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
hit = [r for r in d["rows"] if r["id"] == "H-CAPS"]
assert len(hit) == 1 and hit[0]["kind"] == "behavioral", "H-CAPS davranis satiri capasi bulunamadi"
hit[0].update(status="static", proof="layer_check", proof_rules=["L1"])
for key, status in (("implemented_baseline", "implemented"), ("mock_only_baseline", "mock_only"),
                    ("static_baseline", "static")):
    d[key] = sum(r["status"] == status for r in d["rows"])
pathlib.Path(sys.argv[2]).write_text(json.dumps(d, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY
expect 1 "M1 davranis satiri 'static' beyan edilince drift → 1" -- \
  python3 "$DRIFT" --seam s1 --coverage "$TMPD/coverage-static-inflated.json" --quiet
expect_reason "M2 neden: statik durum yalniz statik satirlar icin" "'H-CAPS' davranis satiri 'static' beyan edilmis"

# Ayni yalan, `kind` de birlikte cevrilerek ve uydurma bir kural adiyla: beyan kendi icinde tutarli
# gorunur. Tur beyandan degil spec §6 metninden okundugu icin yine kirmizi olmali.
python3 - scripts/ci/seam-coverage.json "$TMPD/coverage-kind-flipped.json" <<'PY'
import json, sys, pathlib
d = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
hit = [r for r in d["rows"] if r["id"] == "H-CAPS"]
assert len(hit) == 1 and hit[0]["kind"] == "behavioral", "H-CAPS davranis satiri capasi bulunamadi"
hit[0].update(status="static", kind="static", proof="layer_check", proof_rules=["BOGUS-RULE"])
for key, status in (("implemented_baseline", "implemented"), ("mock_only_baseline", "mock_only"),
                    ("static_baseline", "static")):
    d[key] = sum(r["status"] == status for r in d["rows"])
pathlib.Path(sys.argv[2]).write_text(json.dumps(d, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY
expect 1 "M3 kind + durum birlikte 'static' yapilinca drift → 1" -- \
  python3 "$DRIFT" --seam s1 --coverage "$TMPD/coverage-kind-flipped.json" --quiet
expect_reason "M4 neden: tur spec §6 metninden okunur" "ama spec §6 metnine gore satir behavioral"
expect_reason "M5 neden: uydurma kural adi reddedilir" "layer_check.py'de olmayan kural: ['BOGUS-RULE']"

printf '\n── N. susturulmus satir ──\n'
# Sarmalayici duruyor, CONTRACT_ROWS ve beyan duruyor, test LISTELENIYOR — ama #[ignore] ile kosmuyor.
python3 - "$SEAM/contract.rs" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); s = p.read_text(encoding="utf-8")
old = ('            #[test]\n'
       '            fn h_caps() {\n')
assert s.count(old) == 1, "h_caps sarmalayici capasi bulunamadi"
p.write_text(s.replace(old, '            #[test]\n            #[ignore]\n            fn h_caps() {\n'), encoding="utf-8")
PY
expect 1 "N1 satir #[ignore] ile susturulunca → 1" -- bash "$GATE" --target mock
expect_reason "N2 neden: susturulmus test yesil sayilmaz" "#[ignore]'lu (kosmayan) test var"
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

# BICIM (H): adlar AYNI kalir, bicim degisir. Ad-duzeyi karsilastirma bunlarin hicbirini gormezdi.
# drift_shape <ad> <seam> <dosya> <eski> <yeni> <ciktida beklenen dizge>
drift_shape() {
  local name="$1" seam="$2" file="$3" old="$4" new="$5" reason="$6" other
  OLD="$old" NEW="$new" python3 - "$CONTRACTS/$file" "$TMPD/shape-$file" <<'PY'
import os, sys, pathlib
src = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
old, new = os.environ["OLD"], os.environ["NEW"]
assert src.count(old) == 1, f"bicim mutasyonu capasi {src.count(old)} kez bulundu: {old!r}"
pathlib.Path(sys.argv[2]).write_text(src.replace(old, new), encoding="utf-8")
PY
  local mirrors=()
  for other in "${seam}.rs" handles.rs; do
    if [[ "$other" == "$file" ]]; then mirrors+=(--mirror "$TMPD/shape-$file"); else mirrors+=(--mirror "$CONTRACTS/$other"); fi
  done
  expect 1 "$name → 1" -- python3 "$DRIFT" --seam "$seam" "${mirrors[@]}" --quiet
  expect_reason "$name: neden" "$reason"
}
drift_shape "F6 enum'a variant eklenince (CloseReason + Renewed)" s1 handles.rs \
  $'pub enum CloseReason {\n' $'pub enum CloseReason {\n    Renewed,\n' \
  "enum CloseReason bicimi ayrisiyor: yalniz spec'te [], yalniz kaynakta ['Renewed']"
drift_shape "F7 variant'in tasidigi deger sayisi degisince (Ready'den PeerRef dusunce)" s1 s1.rs \
  '    Ready(SessionHandle, PeerRef),' '    Ready(SessionHandle),' \
  "enum SessionEvent bicimi ayrisiyor"
drift_shape "F8 metot imzasi degisince (cancel)" s1 s1.rs \
  '    fn cancel(&mut self, ticket: ConnectTicket);' '    fn cancel(&mut self, ticket: u64) -> bool;' \
  "KamaTransport::cancel imzasi ayrisiyor"
drift_shape "F9 struct alani yeniden adlandirilinca (TransportCapabilities)" s1 s1.rs \
  '    pub max_plaintext_unit_bytes: u32,' '    pub max_unit_bytes: u32,' \
  "struct TransportCapabilities alanlari ayrisiyor"
drift_shape "F10 S3 enum'una variant eklenince (SessionEvent + Extra)" s3 s3.rs \
  $'pub enum SessionEvent {\n' $'pub enum SessionEvent {\n    Extra,\n' \
  "enum SessionEvent bicimi ayrisiyor: yalniz spec'te [], yalniz kaynakta ['Extra']"
drift_shape "F11 S3 struct-variant alani yeniden adlandirilinca (Initiate.early_unit)" s3 s3.rs \
  '        early_unit: Option<Vec<u8>>,' '        first_message: Option<Vec<u8>>,' \
  "enum SessionCommand bicimi ayrisiyor"
# [2026-10-06] Inceleme bulgulari. Ayni mutasyonlar duzeltmeden onceki betige karsi olculdu:
# F12/F13/F14/F16 YESIL donuyordu (sessiz atlama); F15 kirmiziydi ama yanlis nedenle ("yalniz spec'te
# ['close']" — jenerik metot hic gorulmuyordu); F17 zaten yakalaniyordu (burada regresyon korumasi).
drift_shape "F12 metot varsayilan govde alip imzasi degisince (cancel)" s1 s1.rs \
  '    fn cancel(&mut self, ticket: ConnectTicket);' '    fn cancel(&mut self, ticket: u64) -> bool { false }' \
  "KamaTransport::cancel imzasi ayrisiyor"
drift_shape "F13 alanli struct tuple struct'a donusunce (Delivered)" s1 s1.rs \
  $'pub struct Delivered {\n    pub session: SessionHandle,\n    pub unit: UnitBytes,\n}' \
  'pub struct Delivered(pub SessionHandle, pub UnitBytes);' \
  "Delivered turu ayrisiyor: spec \`struct{}\`, kaynak \`struct()\`"
drift_shape "F14 enum struct'a donusunce (ConnectPhase)" s1 s1.rs \
  'pub enum ConnectPhase {' $'pub struct ConnectPhase(u8);\npub enum ConnectPhaseEski {' \
  "ConnectPhase turu ayrisiyor: spec \`enum\`, kaynak \`struct()\`"
drift_shape "F15 metot jenerik olunca (close)" s1 s1.rs \
  '    fn close(&mut self, session: SessionHandle);' '    fn close<T: Into<SessionHandle>>(&mut self, session: T);' \
  "KamaTransport::close imzasi ayrisiyor"
drift_shape "F16 belge yorumundaki /* sonraki metodu gizleyemez (cancel)" s1 s1.rs \
  '    fn cancel(&mut self, ticket: ConnectTicket);' \
  $'    /// not: /* kapanmamis\n    fn cancel(&mut self, ticket: u64);\n    // */' \
  "KamaTransport::cancel imzasi ayrisiyor"
drift_shape "F17 S3 metodu where kosulu alinca (poll_event)" s3 s3.rs \
  '    fn poll_event(&mut self) -> Option<SessionEvent>;' '    fn poll_event(&mut self) -> Option<SessionEvent> where Self: Sized;' \
  "SessionControl::poll_event imzasi ayrisiyor"

printf '\n'
printf 'oz-test: %d gecti, %d kaldi\n' "$pass" "$fail"
if ((fail)); then
  printf '::error title=SEAM-SELFTEST::seam conformance kapisinin %d muhafizi vakumda\n' "$fail"
  exit 1
fi
printf 'SEAM-SELFTEST: butun muhafizlar kirmizi olabiliyor\n'
exit 0
