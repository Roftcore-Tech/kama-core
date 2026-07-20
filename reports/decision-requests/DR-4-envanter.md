# DR-4 — LEGACY AEAD/NONCE ENVANTERİ [karar-paketi BÖLÜM-4.6 formatı]

> Otorite: `specs/DR-4-DR-5-karar-paketi-ve-spec-patchleri.md` (EN-YÜKSEK). Bu envanter, karar-paketi
> KOŞUL-A/B/C kuralını (BÖLÜM-1) besler. Kanıt yöntemi: exhaustive workspace-taraması (12 crate + tests/
> benches/examples/specs/reports) + doğrudan-spec-okuma; **yokluk grep-ile-KANITLANDI, varsayılmadı.**

## 1. Gerçek-legacy-AEAD-implementation var mı?

**HAYIR.** Workspace'te `kama-noise-wire` DIŞINDA çalışan hiçbir AEAD operasyonu yok.
- `crates/crypto/src/lib.rs` = **3-satır doc-comment iskeleti** ("API-first iskelet; trait imzaları sonraki iş");
  `[dependencies]` BOŞ. Gerçek kripto-kod YOK.
- Diğer tüm crate'ler (ccr-codec, hub-server, client-core, ra-ca, discovery, admin-server) = 3-satır iskelet.
- `crates/bench/src/kama.rs:38` `const TAG_STUB: usize = 16;` + `:59` `b.resize(..+TAG_STUB, 0) // AEAD-tag SİMÜLASYONU`
  = **16×0x00 zero-fill** (key/cipher/nonce YOK) — gerçek-AEAD DEĞİL.
- Yokluk-kanıtı: `grep -rniE 'ring::aead|libsodium|sodiumoxide|crypto_aead|dryoc|orion' --include=*.rs crates/`
  → yalnız 1 hit: `crates/crypto/src/lib.rs:1` (doc-comment). Çalışan-kod SIFIR.

## 2. Crate/kod-yolu (tek gerçek AEAD = yeni kod)

`crates/kama-noise-wire/src/protector.rs` — import `:10-11`, cipher-kurulum `:116/:120/:139/:140`,
gerçek `.encrypt()` `:236`, gerçek `.decrypt()` `:301`. Bu **inceleme-altındaki-yeni-kod** (Sprint-1), legacy DEĞİL.

## 3. Nonce-mapping exact-bytes (mevcut kod)

Tek counter→12B-nonce üretici, yeni-crate'te: `protector.rs:161-165`
```
fn nonce_for(full_pn: u64) -> Nonce {
    let mut n = [0u8; 12];
    n[4..12].copy_from_slice(&full_pn.to_be_bytes());   // BIG-ENDIAN (provizyonel Sprint-1-varsayılanı)
```
Mapping = `[4B-zero ‖ BE64(full_pn)]`. **Bu, D1-görev-promptunun geçici-varsayılanıdır** (karar-paketi-öncesi),
herhangi bir legacy-artefakta DAYANMAZ. `kama-noise-wire` DIŞINDA hiçbir AEAD-nonce byte-mapping YOK (diğer
`to_be_bytes` hitleri seq/block_id/length wire-framing'i, nonce DEĞİL).

## 4. Appendix-D2 exact-metni (satır-alıntı)

ccr-format-spec.md **DEPRECATED Appendix-D2** (`ccr:2879` "Historical Only", `ccr:2722` "MUST NOT IMPLEMENT"),
cipher_suite 0x0001 tanımı `ccr:3001`:
```
#   AEAD:  ChaCha20-Poly1305 (RFC 8439); key 32B, nonce 12B, tag 16B
#          nonce = per_message_key türetiminde HKDF'ten 12B (key 32B || nonce 12B = 44B output)
```
**KRİTİK:** D2-nonce'u **HKDF-türevli-per-message-nonce**'tur — counter→nonce **byte-mapping DEĞİL**. Yani D2, LE64
veya BE64 counter-mapping'i için **hiçbir referans üretmez** (bambaşka nonce-modeli). Aktif §35.8 (`ccr:2442/2468`)
"full session counter"-nonce der ama **byte-layout tanımlamaz**. Sonuç: counter→nonce96 mapping'i için ne LE ne BE
sabitleyen hiçbir normatif-metin/vektör mevcut.

## 5. Eski-ciphertext/vector'lar

**Yok (legacy/bağımsız).**
- `crates/adaptive-transport/tests/tv_fec.rs`: banner `:3-6` "RETIRED/DEPRECATED"; tag = placeholder `:14` `0x11×16`,
  opaque-byte olarak test edilir — AEAD-hesaplı DEĞİL.
- `crates/kama-noise-wire/tests/golden_wire.rs:13` (`GOLDEN_DATAGRAM`): gerçek RFC-8439-ciphertext AMA (a) yeni-crate
  içinde, (b) mevcut-BE64-impl'in **kendisi tarafından üretilmiş** → **döngüsel** (yalnız "kod-şu-an-BE-yapıyor"u
  kanıtlar; bağımsız/legacy-artefakt DEĞİL).
- `find crates specs results reports -name '*.bin'/-'*.hex'/-'*.vec'` → hiçbiri. Ciphertext-fixture YOK.

## 6. VECTOR-CLASSIFICATION

**`NO_VECTOR`** — counter→nonce96 byte-mapping'ini sabitleyen hiçbir legacy/bağımsız-vektör YOK. (D2 = farklı
HKDF-nonce-modeli; aktif-§35.8 = byte-layout-tanımsız; golden_wire = self-generated-döngüsel.)

## 7. COMPATIBILITY-DEBT (karar-paketi BÖLÜM-1 esas-kuralı)

**`NO`.** Borç-tanımının dört-şartı da SAĞLANMIYOR (yani borç YOK):
- persisted/released-ciphertext-açılmak-zorunda → **YOK** (redb/disk/DB ciphertext yok; `redb`/`persist`/`session_state`
  yalnız hub-server doc-comment, deps-boş).
- eski-peer-interop-korunmalı → **YOK** (sahada peer yok; tüm session/handshake katmanı Sprint-4'e ertelenmiş,
  `protector.rs:92`/`lib.rs:9`).
- başka-implementation-yayımlanmış-vektöre-bağlı → **YOK** (`grep '^name = "(libsodium-sys|sodiumoxide|dryoc|orion)"'
  Cargo.lock` → sıfır; dış-impl yok).
- yayımlanmış-korunması-gereken-wire-contract → **YOK** (D2 "MUST NOT IMPLEMENT/Historical"; tek-başına-borç-yaratmaz —
  karar-paketi BÖLÜM-1 `:19-22/:29`).

## 8. RECOMMENDATION

**`RECOMMEND_LE64`** → karar-paketi **KOŞUL-A(2) KESİN sağlanır** (gerçek-compatibility-borcu-YOK). LE64 =
Noise-Framework-ChaChaPoly-kanonik-mapping (`nonce[4..12]=full_pn.to_le_bytes()`). D2'nin farklı-model-içindeki
byte-sırası, gerçek-borç-olmadığı-için YALNIZ-TARİHSEL-NOTTUR — LE'yi-engellemez (karar-paketi `:29`).
→ **OTOMATİK-KİLİT uygulanır (GÖREV-2): VARYANT-LE §4.5 + gate CLOSED + override-kaldır + Sprint-1-golden-re-baseline.**

## 9. CHANGED-FILES (GÖREV-2 otomatik-kilit kapsamı)

- `specs/kama-noise-wire-tdd-v2.0.md`: §4.5 VARYANT-LE eklendi (§5-öncesi) · gate `AEAD nonce96 mapping → CLOSED` ·
  GEÇİCİ-STATÜ-OVERRIDE bloğu kaldırıldı.
- `specs/fec-core-tdd-v3.0.md`: gate `AEAD nonce96 mapping → CLOSED` · GEÇİCİ-STATÜ-OVERRIDE bloğu kaldırıldı.
- `crates/kama-noise-wire/src/protector.rs`: `to_be_bytes` → `to_le_bytes` (LE64-kilit).
- `crates/kama-noise-wire/tests/golden_wire.rs`: `GOLDEN_DATAGRAM` LE64 ile re-baseline.
- (DR-5) `specs/tech-stack-adr.md`: ADR-03 AEAD-backend amendment.

## 10. TESTS (K-WIRE-NONCE96-1 dört-katman)

- KATMAN-1 RFC-8439-vektörü (`tests/rfc8439_vectors.rs`) — primitive-doğruluğu, harici-oracle.
- KATMAN-2 nonce-mapping-vektörleri (`tests/nonce96_vectors.rs`) — full_pn={0,1,2³²,2⁶³} → exact LE64-nonce96-bytes.
- KATMAN-3 RustCrypto==libsodium-diff (`tests/aead_differential.rs`, feature-gate `libsodium-oracle`, ayrı-CI-job,
  production-graph-dışı, HARD-FAIL).
- KATMAN-4 full-wire-golden (`tests/golden_wire.rs`) — reconstructed-full_pn + nonce96 + AAD + ct + tag + framing.

## 11. RISKS

- **Düşük.** LE64-geçişi yalnız `nonce_for` (tek-fonksiyon) + `GOLDEN_DATAGRAM` (self-golden) etkiler; roundtrip-testleri
  (seal→open aynı-mapping) mapping-agnostik → LE'de de geçer. Legacy-interop-riski YOK (borç-yok).
- KATMAN-3 libsodium-native-build ortam-bağımlı → feature-gate'li (yerel `cargo test` bozulmaz; CI-job libsodium'lu).
- Self-golden tek-oracle-DEĞİL: KATMAN-1 (RFC-8439) harici-standart-referansı bunu karşılar.

CHECKPOINT: Applied Abkarada DR-4 conditional lock (KOŞUL-A / LE64). Sprint-4 status: GO (DR-4-CLOSED sonrası, D1-promptu-§5 kapsamı) — bu turda BAŞLATILMADI (atomik-merge koordinasyonu için Abkarada'ya bırakıldı). Sprint-5 remains HOLD.
