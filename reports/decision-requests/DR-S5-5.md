# DR-S5-5 — param_hash BLAKE2s-256 ÜRETİM-BACKEND: RustCrypto `blake2` [ONAYLANDI]

- **Sprint:** 5d (PARAM-bootstrap ön-koşulu). **Statü:** ONAYLANDI (Abkarada, AskUserQuestion 2026-07-21).
- **Bağlam:** 5d-1 PARAM-bootstrap'ın "PARAM-hash-mismatch → fail-closed" davranışı gerçek `param_hash` hesabı ister.
  noise-wire-tdd §8: `param_hash = BLAKE2s-256(DS ‖ fields)`, DS=ASCII("KAMA-PARAM-V1"). Şu-an ÜRETİM-hesabı YOK
  (schema kripto-free; gerçek hesap yalnız `kama-transport-schema/tests/golden_v0.rs` dev-dep `blake2` ile).

## 0. KARAR

**param_hash BLAKE2s-256 üretim-backend'i = RustCrypto `blake2` crate (v0.10).** Üretim-home = `kama-noise-wire`
(kripto-katmanı; zaten `chacha20poly1305` taşır). Yeni pür-fonksiyon `bind_transport_parameters(fields) → [u8;32]`.

## 1. NEDEN DR (sessiz-ekleme YASAK)

- **Registry-durumu:** BLAKE2s-256 pki §13 registry'de VAR (cipher_suite 0x0001 = Ed25519/X25519/ChaCha20-Poly1305/
  **HKDF-BLAKE2s**/SHA-256; pki-ca-ra-hsm-tdd.md:456-457,603). Yani primitif meşru.
- **Backend-çelişkisi:** ADR-03 backend-atamasında BLAKE2s → **libsodium** ("libsodium: ...ChaCha20-Poly1305, BLAKE2s...";
  tech-stack-adr.md:66). DR-5 yalnız AEAD-backend'ini RustCrypto'ya çevirdi, "Ed25519/X25519/**BLAKE2s**/ristretto
  kararları DEĞİŞMEDİ" (adr:78) → BLAKE2s hâlâ ADR-03-libsodium.
- **Latent-uyumsuzluk:** libsodium `crypto_generichash` = **BLAKE2b ≠ BLAKE2s**. libsodium BLAKE2s-256'yı doğrudan
  expose ETMEZ → ADR-03'ün "libsodium: BLAKE2s" satırı üretimde uygulanamaz. Bu, DR-4/DR-5 gibi bir **backend-kararı**
  gerektirir; "Kripto primitifi registry dışından ekleme" (registry-İÇİ, backend-seçimi) + "Spec-edit YASAK, DR yaz" gereği.

## 2. GEREKÇE (RustCrypto blake2)

1. **DR-5 emsali:** production-crypto-yönü RustCrypto (chacha20poly1305 v0.10.1 üretim; libsodium yalnız test-oracle).
   Aynı ekosistem + saf-Rust-toolchain-bütünlüğü (ADR-03-gerekçesi: küçük-FFI-yüzeyi, kolay-cross-compile).
2. **Golden-emsali:** `golden_v0.rs::golden_param_blake2s_hash` zaten `blake2::Blake2s256` ile canonical byte-dizisinin
   32B-digest'ini deterministik-assert ediyor → üretim aynı-crate'i kullanınca golden-vektör drift-gate'i kesintisiz.
3. **BLAKE2s-özgün:** RustCrypto `blake2` gerçek BLAKE2s-256 sağlar (libsodium-BLAKE2b-uyumsuzluğu YOK).

## 3. ZORUNLU-UYGULAMA-KISITLARI (DR-5-kalıbı)

- Üretim-tip AÇIKÇA `blake2::Blake2s256` (BLAKE2b/BLAKE2s-uzunluk-varyantı-karıştırılmaz — 256-bit çıktı assert).
- exact-version Cargo.lock'tan; checksum lockfile-metadata'dan (uydurma-YOK). Cargo.lock DAİMA commit.
- Home = `kama-noise-wire` [dependencies] `blake2 = "0.10"` (schema kripto-free KALIR; adaptive-transport crypto-agnostik KALIR).
- Runtime-backend-switching YOK; public CryptoProvider-abstraction YOK (Kama kapalı-tek-profil).
- **Golden-eşitlik gate:** üretim `bind_transport_parameters` çıktısı `golden_param_blake2s_hash`-vektörüyle BİT-ÖZDEŞ
  (drift-gate). Canonical-input schema-otoritesinden (`ParamFields::canonical_hash_input`), noise-wire yalnız hash'ler.

## 4. ADR-03 AMENDMENT (bu-DR ile; tech-stack-adr.md'ye eklenir)

"### ADR-03 AMENDMENT-2 — Hash Backend (BLAKE2s-256 / param_hash) [DR-S5-5, 2026-07]" — param_hash BLAKE2s-256
üretim-backend'i RustCrypto `blake2`; libsodium-generichash-BLAKE2b-uyumsuzluğu nedeniyle libsodium-BLAKE2s-yolu
uygulanmaz. Diğer BLAKE2s-kullanımları (gelecek Noise HKDF-BLAKE2s) aynı-backend'e yönelir (tutarlılık).

## 5. KAPSAM-SINIRI

Bu-DR yalnız **param_hash BLAKE2s-256 üretim-backend'i**. Noise-handshake HKDF-BLAKE2s (crypto-skeleton, §0.9) ayrı-
iş; aynı-backend-tercihi (RustCrypto blake2) doğal-uzantı ama bu-DR onu implemente-ETMEZ. AEAD-backend (DR-5) değişmez.
