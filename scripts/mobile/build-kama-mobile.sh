#!/usr/bin/env bash
# K4G-15 — kama-mobile Android paketi (MOBILE-HANDOFF; plan K4G-15 NE 3, D9, D10, D11).
#
#   ANDROID_NDK_HOME=<ndk kökü> scripts/mobile/build-kama-mobile.sh [çıktı dizini]
#
# Adımlar:
#   (a) arm64-v8a `libkama_mobile.so`: cargo-ndk, API 28 (PoC-KAPSAM-001 §2: minSdk 28), 16 KB
#       sayfa hizalaması (RUSTFLAGS link-arg; NDK r28+ zaten varsayılan yapar, açıkça istenir).
#   (b) Kotlin bağlayıcıları: kama-mobile'ın `uniffi-bindgen` ikilisi (clap'siz, kütüphane kipi),
#       (a)'nın `.so`'sundan — paketlenen ikili ile bağlayıcı aynı meta-veriden üretilir.
#   (c) örnek cihaz dosyası: `kama-provision init` geçici dizinde alice+bob üretir; pakete YALNIZ
#       `device-alice.json` girer. `root.json` geçici dizinde kalır ve çıkışta silinir (D9).
#   (d) zip: `kama-mobile-<kısa sha>.zip` = .so + .kt + KM-SEAM-AK-001.md +
#       PROVISIONING-FORMAT-v0.md + device-alice.json + README-HASAN.md.
#   (e) denetimler: zip'te `root.json` yok ve kökün gizli anahtarı hiçbir dosyada geçmiyor;
#       `.so`'nun her LOAD segmenti 0x4000 hizalı (`readelf -lW`); zip sha256 basılır.
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
# Örnek cihaz dosyasının direct-mode adresleri (PROVISIONING-FORMAT-v0 §3.2 örneğiyle aynı): telefonda
# yalnız süreç-içi loopback içindir; `start()` için cihazın kendi adresi gerekir (NoDirectAddress).
SAMPLE_DIRECT="alice=127.0.0.1:47001,bob=127.0.0.1:47002"

REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$REPO"

: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME dışarıdan verilmeli (NDK kökü)}"
[ -d "$ANDROID_NDK_HOME" ] || die "ANDROID_NDK_HOME dizin değil: $ANDROID_NDK_HOME"
for tool in cargo readelf zip unzip sha256sum python3; do
  command -v "$tool" >/dev/null || die "$tool bulunamadı"
done
cargo ndk --version >/dev/null 2>&1 || die "cargo-ndk kurulu değil"

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
STAGE="$OUT/$PKG_NAME"
rm -rf "$STAGE" "$ZIP"
mkdir -p "$STAGE"

# Geçici provisioning otoritesi (root.json burada kalır) ve sahne çıkışta silinir: sahnedeki örnek
# cihaz dosyası gizli anahtar taşır, kopyası yalnız zip'te kalır.
PROV="$(mktemp -d)"
cleanup() {
  rm -rf "$PROV" "$STAGE"
}
trap cleanup EXIT

echo "== (a) $ABI $SO_NAME (API $ANDROID_API, sayfa $PAGE_ALIGN)"
RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384" \
  cargo ndk -t "$ABI" -P "$ANDROID_API" -o "$STAGE/jniLibs" build --release --locked -p kama-mobile
SO="$STAGE/jniLibs/$ABI/$SO_NAME"
[ -f "$SO" ] || die "$SO üretilmedi"

echo "== (b) Kotlin bağlayıcıları"
cargo run --locked -q -p kama-mobile --features bindgen --bin uniffi-bindgen -- "$SO" "$STAGE/kotlin"
[ -f "$STAGE/kotlin/$KOTLIN_FILE" ] || die "$KOTLIN_FILE üretilmedi"

echo "== (c) örnek cihaz dosyası (kama-provision init; yalnız device-alice.json)"
cargo run --locked -q -p kama-provision -- init --out "$PROV/authority" --devices alice,bob \
  --direct "$SAMPLE_DIRECT"
cp "$PROV/authority/device-alice.json" "$STAGE/device-alice.json"

echo "== (d) zip"
cp docs/seams/KM-SEAM-AK-001.md docs/provisioning/PROVISIONING-FORMAT-v0.md \
  crates/kama-mobile/README-HASAN.md "$STAGE/"
(cd "$OUT" && zip -q -X -r "$ZIP" "$PKG_NAME")

echo "== (e) denetimler"
# Zip'in kendisi açılıp taranır (boru + `grep -q` erken çıkışı pipefail altında eşleşmeyi
# yutabilirdi). Açılan kopya geçici dizindedir ve çıkışta silinir.
CHECK="$PROV/check"
unzip -q "$ZIP" -d "$CHECK"
[ -z "$(find "$CHECK" -name root.json -print -quit)" ] || die "zip root.json içeriyor (D9)"
ROOT_SECRET="$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["ed25519"]["priv"])' \
  "$PROV/authority/root.json")"
[ "${#ROOT_SECRET}" -eq 64 ] || die "root.json gizli anahtarı okunamadı"
status=0
grep -r -a -F -q -e "$ROOT_SECRET" "$CHECK" || status=$?
case "$status" in
  0) die "kökün gizli anahtarı pakette bir dosyada geçiyor (D9)" ;;
  1) ;;
  *) die "gizli anahtar taraması başarısız (grep çıkışı $status)" ;;
esac
ALIGNS="$(readelf -lW "$SO" | awk '$1 == "LOAD" { print $NF }')"
[ -n "$ALIGNS" ] || die "$SO_NAME LOAD segmenti yok"
while read -r align; do
  [ "$align" = "$PAGE_ALIGN" ] || die "$SO_NAME LOAD hizalaması $align (beklenen $PAGE_ALIGN)"
done <<<"$ALIGNS"
echo "LOAD hizalamaları: $(echo "$ALIGNS" | tr '\n' ' ')"
echo "zip içeriği:"
unzip -Z1 "$ZIP" | sed 's/^/  /'
echo "== $ZIP"
sha256sum "$ZIP"
