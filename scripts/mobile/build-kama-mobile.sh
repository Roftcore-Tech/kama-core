#!/usr/bin/env bash
# K4G-15 — kama-mobile Android paketi (MOBILE-HANDOFF; plan K4G-15 NE 3, D9, D10, D11).
#
#   ANDROID_NDK_HOME=<ndk kökü> scripts/mobile/build-kama-mobile.sh [çıktı dizini]
#
# Adımlar:
#   (a) arm64-v8a `libkama_mobile.so`: cargo-ndk (>= 4: `-o` kopyasını strip'lemez), API 28
#       (PoC-KAPSAM-001 §2: minSdk 28), 16 KB sayfa hizalaması (RUSTFLAGS link-arg; NDK r28+ zaten
#       varsayılan yapar, açıkça istenir). Çıktıyı değiştirebilecek `CARGO_NDK_*` ortam değişkenleri
#       (ör. `CARGO_NDK_LINK_LIBCXX_SHARED`: `libc++_shared.so` ekler) ve `RUSTFLAGS`'ı ezen
#       `CARGO_ENCODED_RUSTFLAGS` derlemeden önce kayda yazılır ve kaldırılır.
#   (b) Kotlin bağlayıcıları: kama-mobile'ın `uniffi-bindgen` ikilisi (clap'siz, kütüphane kipi;
#       yapılandırma `crates/kama-mobile/uniffi-bindgen.toml`), (a)'nın `.so`'sundan — paketlenen
#       ikili ile bağlayıcı aynı meta-veriden üretilir. Önkoşul: `.so`'da sembol tablosu (`.symtab`)
#       var; UniFFI meta-veriyi oradan okur (strip'lenmiş kütüphanede meta-veri yoktur).
#   (c) örnek cihaz dosyaları: `kama-provision init` geçici dizinde alice+bob üretir; pakete
#       `device-alice.json` ve `device-bob.json` girer (DUR-K4G-15-B CEVAP: yalnız süreç-içi
#       loopback testi için; gerçek kurulumda her telefon yalnız kendi dosyasını alır). `root.json`
#       geçici dizinde kalır ve çıkışta silinir (D9).
#   (d) zip `kama-mobile-<kısa sha>.zip` = .so + .kt + KM-SEAM-AK-001.md + PROVISIONING-FORMAT-v0.md
#       + README-HASAN.md + device-alice.json + device-bob.json. Zip önce GEÇİCİ adla yazılır.
#   (e) denetimler: zip içeriği tam olarak bu yedi dosya (izin listesi); `root.json` yok ve kökün
#       gizli anahtarı (hex ya da ham bayt) hiçbir dosyada geçmiyor — anahtar komut satırına girmez
#       (`python3 -I` dosyadan okur); `.so`'nun her LOAD segmenti 0x4000 hizalı (`readelf -lW`);
#       `.kt`'de kayıtlar değiştirilemez (`val`) ve `EngineEvent.toString()` Rust `Debug`'ına bağlı.
#       Bütün denetimler geçince zip kanonik adına taşınır ve sha256'sı basılır; herhangi biri
#       düşerse geçici zip silinir, kanonik adla hiçbir dosya kalmaz.
#
# Çıktı dizini varsayılanı <repo>/target/kama-mobile-dist (git'te yok sayılır). Paket adı commit
# SHA'sını taşıdığı için izlenen dosyalarda commit edilmemiş değişiklik varsa betik durur
# (KAMA_MOBILE_ALLOW_DIRTY=1 ile `-dirty` ekiyle devam eder). Cargo.lock değiştirilmez (--locked).
# Makine kuralı: derleme işi sayısı dışarıdan (CARGO_BUILD_JOBS=2).
set -euo pipefail

die() {
  echo "build-kama-mobile: HATA: $*" >&2
  exit 1
}

# ── Sabitler ──
ABI=arm64-v8a
ANDROID_API=28
PAGE_ALIGN=0x4000
SO_NAME=libkama_mobile.so
KOTLIN_FILE=uniffi/kama_mobile/kama_mobile.kt
DEVICES=(alice bob)
# Örnek cihaz dosyalarının direct-mode adresleri (PROVISIONING-FORMAT-v0 §3.2 örneğiyle aynı): yalnız
# süreç-içi loopback içindir; `start()` için cihazın kendi adresi gerekir (NoDirectAddress).
SAMPLE_DIRECT="alice=127.0.0.1:47001,bob=127.0.0.1:47002"
# `EngineEvent`'in Rust `Debug`'ını çağıran FFI fonksiyonu (`#[uniffi::export(Debug)]`): Kotlin
# `toString()` bunu çağırır — metin, ses yükü ve blob yerine uzunlukları yazar.
EVENT_DEBUG_FN=uniffi_kama_mobile_fn_method_engineevent_uniffi_trait_debug

REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$REPO"

: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME dışarıdan verilmeli (NDK kökü)}"
[ -d "$ANDROID_NDK_HOME" ] || die "ANDROID_NDK_HOME dizin değil: $ANDROID_NDK_HOME"
for tool in cargo readelf zip unzip sha256sum python3; do
  command -v "$tool" >/dev/null || die "$tool bulunamadı"
done
NDK_TOOL="$(cargo ndk --version 2>/dev/null)" || die "cargo-ndk kurulu değil"
[[ "$NDK_TOOL" =~ ([0-9]+)\.[0-9]+\.[0-9]+ ]] || die "cargo-ndk sürümü okunamadı: $NDK_TOOL"
[ "${BASH_REMATCH[1]}" -ge 4 ] \
  || die "cargo-ndk >= 4 gerekir (bulunan ${BASH_REMATCH[0]}): 3.x -o çıktısını strip'ler, UniFFI meta-verisi kaybolur"
echo "cargo-ndk ${BASH_REMATCH[0]}"

SHORT_SHA="$(git rev-parse --short HEAD)"
if ! git diff --quiet HEAD --; then
  [ "${KAMA_MOBILE_ALLOW_DIRTY:-0}" = 1 ] || die "izlenen dosyalarda commit edilmemiş değişiklik var; paket $SHORT_SHA'yı temsil etmez (KAMA_MOBILE_ALLOW_DIRTY=1 ile -dirty)"
  SHORT_SHA="$SHORT_SHA-dirty"
fi

PKG_NAME="kama-mobile-$SHORT_SHA"
OUT="${1:-$REPO/target/kama-mobile-dist}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
ZIP="$OUT/$PKG_NAME.zip"
# Geçici zip kanonik adla aynı dizinde: son taşıma atomik yeniden adlandırmadır.
ZIP_PART="$OUT/.$PKG_NAME.zip.partial"
STAGE="$OUT/$PKG_NAME"
rm -rf "$STAGE" "$ZIP" "$ZIP_PART"
mkdir -p "$STAGE"

# Geçici provisioning otoritesi (root.json burada kalır), sahne ve geçici zip çıkışta silinir:
# sahnedeki örnek cihaz dosyaları gizli anahtar taşır, kopyaları yalnız denetimden geçmiş zip'te kalır.
PROV="$(mktemp -d)"
cleanup() {
  rm -rf "$PROV" "$STAGE" "$ZIP_PART"
}
trap cleanup EXIT

echo "== (a) $ABI $SO_NAME (API $ANDROID_API, sayfa $PAGE_ALIGN)"
OVERRIDES="$(compgen -e | grep -E '^(CARGO_NDK_|CARGO_ENCODED_RUSTFLAGS$)' || true)"
if [ -n "$OVERRIDES" ]; then
  echo "uyarı: çıktıyı değiştirebilecek ortam değişkenleri derlemeden kaldırıldı: $(tr '\n' ' ' <<<"$OVERRIDES")"
  while read -r name; do
    unset "$name"
  done <<<"$OVERRIDES"
fi
RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384" \
  cargo ndk -t "$ABI" -P "$ANDROID_API" -o "$STAGE/jniLibs" build --release --locked -p kama-mobile
SO="$STAGE/jniLibs/$ABI/$SO_NAME"
[ -f "$SO" ] || die "$SO üretilmedi"
SECTIONS="$(readelf -SW "$SO")"
grep -q '[[:space:]]\.symtab[[:space:]]' <<<"$SECTIONS" \
  || die "$SO_NAME'de .symtab yok (strip'lenmiş): UniFFI meta-verisi okunamaz"

echo "== (b) Kotlin bağlayıcıları"
cargo run --locked -q -p kama-mobile --features bindgen --bin uniffi-bindgen -- "$SO" "$STAGE/kotlin"
KT="$STAGE/kotlin/$KOTLIN_FILE"
[ -f "$KT" ] || die "$KOTLIN_FILE üretilmedi"

echo "== (c) örnek cihaz dosyaları (kama-provision init; ${DEVICES[*]})"
cargo run --locked -q -p kama-provision -- init --out "$PROV/authority" --devices alice,bob \
  --direct "$SAMPLE_DIRECT"
for device in "${DEVICES[@]}"; do
  cp "$PROV/authority/device-$device.json" "$STAGE/device-$device.json"
done

echo "== (d) zip (geçici ad)"
cp docs/seams/KM-SEAM-AK-001.md docs/provisioning/PROVISIONING-FORMAT-v0.md \
  crates/kama-mobile/README-HASAN.md "$STAGE/"
(cd "$OUT" && zip -q -X -D -r "$ZIP_PART" "$PKG_NAME")

echo "== (e) denetimler"
# İzin listesi: zip'te TAM OLARAK bu dosyalar (fazlası da eksiği de kırmızı).
EXPECTED="$(printf '%s\n' \
  "$PKG_NAME/jniLibs/$ABI/$SO_NAME" \
  "$PKG_NAME/kotlin/$KOTLIN_FILE" \
  "$PKG_NAME/KM-SEAM-AK-001.md" \
  "$PKG_NAME/PROVISIONING-FORMAT-v0.md" \
  "$PKG_NAME/README-HASAN.md" \
  "$PKG_NAME/device-alice.json" \
  "$PKG_NAME/device-bob.json" | LC_ALL=C sort)"
LISTING="$(unzip -Z1 "$ZIP_PART")"
ACTUAL="$( (grep -v '/$' <<<"$LISTING" || true) | LC_ALL=C sort)"
[ "$ACTUAL" = "$EXPECTED" ] \
  || die "zip içeriği izin listesine uymuyor"$'\n'"$(diff <(echo "$EXPECTED") <(echo "$ACTUAL") || true)"
# Zip'in kendisi açılıp taranır. Açılan kopya geçici dizindedir ve çıkışta silinir.
CHECK="$PROV/check"
unzip -q "$ZIP_PART" -d "$CHECK"
[ -z "$(find "$CHECK" -name root.json -print -quit)" ] || die "zip root.json içeriyor (D9)"
# Kökün gizli anahtarı yalnız Python sürecinin belleğindedir: argv'ye, ortama ya da dosyaya yazılmaz.
python3 -I - "$PROV/authority/root.json" "$CHECK" <<'PY' || die "kök gizli anahtarı taraması kırmızı (D9)"
import json
import os
import sys

root_file, tree = sys.argv[1], sys.argv[2]
with open(root_file, encoding="utf-8") as f:
    secret_hex = json.load(f)["ed25519"]["priv"]
if len(secret_hex) != 64:
    sys.exit("root.json gizli anahtarı okunamadı")
needles = (secret_hex.lower().encode(), secret_hex.upper().encode(), bytes.fromhex(secret_hex))
for dirpath, _, names in os.walk(tree):
    for name in names:
        path = os.path.join(dirpath, name)
        with open(path, "rb") as f:
            data = f.read()
        if any(needle in data for needle in needles):
            sys.exit(f"kökün gizli anahtarı pakette: {os.path.relpath(path, tree)}")
PY
ALIGNS="$(readelf -lW "$SO" | awk '$1 == "LOAD" { print $NF }')"
[ -n "$ALIGNS" ] || die "$SO_NAME LOAD segmenti yok"
while read -r align; do
  [ "$align" = "$PAGE_ALIGN" ] || die "$SO_NAME LOAD hizalaması $align (beklenen $PAGE_ALIGN)"
done <<<"$ALIGNS"
echo "LOAD hizalamaları: $(tr '\n' ' ' <<<"$ALIGNS")"
# Kotlin: kayıt alanları `val` (uniffi-bindgen.toml), `EngineEvent.toString()` Rust Debug'ına bağlı.
python3 -I - "$KT" "$EVENT_DEBUG_FN" <<'PY' || die "Kotlin bağlayıcı denetimi kırmızı"
import re
import sys

kt_file, debug_fn = sys.argv[1], sys.argv[2]
with open(kt_file, encoding="utf-8") as f:
    kt = f.read()
records = ("StreamHandle", "FileHandle", "NodeMetrics", "PeerConnectState")
for record in records:
    m = re.search(rf"data class {record} \((.*?)\n\)", kt, re.S)
    if m is None:
        sys.exit(f"kayıt bulunamadı: {record}")
    fields = m.group(1)
    if re.search(r"^\s*var ", fields, re.M) or not re.search(r"^\s*val ", fields, re.M):
        sys.exit(f"kayıt değiştirilebilir üretildi: {record}")
if debug_fn not in kt:
    sys.exit(f"EngineEvent.toString() Rust Debug'ına bağlı değil ({debug_fn} yok)")
print(f"kayıtlar val alanlı: {', '.join(records)}; EngineEvent.toString() → {debug_fn}")
PY
mv -f "$ZIP_PART" "$ZIP"
echo "zip içeriği:"
unzip -Z1 "$ZIP" | sed 's/^/  /'
echo "== $ZIP"
sha256sum "$ZIP"
