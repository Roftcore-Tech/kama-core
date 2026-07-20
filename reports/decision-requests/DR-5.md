# DR-5 — AEAD implementation-crate: RustCrypto `chacha20poly1305` vs ADR-03 libsodium-primary

- **Kaynak:** D1 Sprint-1 (kama-noise-wire).
- **Tür:** ADR-03 ↔ ADR-01 gerilimi (primitif registry-uyumlu; implementation-crate seçimi). KURAL-2: spec/ADR-değiştirilmedi.
- **Statü:** AÇIK. Sprint-1 RustCrypto ile İLERLEDİ (wire-deterministik, seal/open-arkasında-swappable). Abkarada-kararı D1-EXIT-öncesi.

## Gerilim

- **ADR-03:** "Kripto: libsodium (primary) + dalek (cross-check) + custom DER. Primitif YALNIZ pki §13 registry'den."
  → AEAD-implementation'ın **libsodium-primary** olması beklenir.
- **ADR-01:** "unsafe yüzeyi minimum (yalnız libsodium + PKCS#11 + mobil binding)." → libsodium = FFI + unsafe yüzeyi.
- **Sprint-1 seçimi:** RustCrypto `chacha20poly1305 = "0.10"` (saf-Rust; `#![forbid(unsafe_code)]` crate'te ÇALIŞIYOR;
  FFI-unsafe-yüzeyi-EKLEMEZ). Primitif = ChaCha20-Poly1305 (RFC 8439) = pki-§13-registry-teyitli.

## Neden bu-noktada-bloklamaz

Wire-byte'lar RFC-8439-DETERMİNİSTİK (crate-bağımsız) — `golden_wire.rs` byte-PIN'i bunu kanıtlar. AEAD-implementation
`KamaNoiseWireProtector::seal/open` API'sinin ARKASINDA izole → libsodium'a geçiş tek-modül-değişikliği (wire-golden
değişmez; yalnız implementation-audit-tabanı değişir).

## Karar-talebi (Abkarada)

1. AEAD-implementation: (a) RustCrypto `chacha20poly1305` KAL (saf-Rust, forbid-unsafe, audit: RustCrypto-review) —
   ADR-03 "libsodium-primary" bu-crate-için gevşetilir (gerekçe: ADR-01 unsafe-minimizasyonu + wire-katmanı-saf-Rust);
   VEYA (b) libsodium'a-geç (ADR-03-harfiyen; FFI-unsafe-yüzeyi-kabul, cross-crate wire-golden-eşitlik-testi eklenir).
2. Karar (b) ise: cross-check dalek/RustCrypto ile golden-byte-eşitlik-testi ZORUNLU (iki-impl-aynı-wire, K-WIRE-1-emsali).

## Etki

- (a) → yalnız ADR-03-not-güncellemesi (Abkarada/chat; ben-ADR-düzenlemem).
- (b) → `protector.rs` AEAD-çağrıları libsodium-binding'e; `Cargo.toml` deps; wire-golden değişmez (RFC-deterministik).
