# DR-4 — AEAD nonce byte-kurgusu aktif ccr-§35'te TANIMSIZ

- **Kaynak:** D1 Sprint-1 (kama-noise-wire seal/open) + ccr-§35 spec-extraction.
- **Tür:** spec-boşluğu (aktif-normatifte-eksik; byte-exact-tanım yalnız DEPRECATED bölümde). KURAL-2: spec-değiştirilmedi.
- **Statü:** AÇIK. Sprint-1 görev-promptunun varsayılanıyla İLERLEDİ (aşağıda); Abkarada-kararı wire-golden'ı sabitler.

## Gözlem (extraction-kanıtı)

Aktif Session Model V3 (§35), AEAD-nonce'un u64-counter → 12B-nonce byte-eşlemesini TANIMLAMIYOR:
- `ccr:2442` — "Gerçek AEAD nonce/counter **full session counter**'dır (receiver state'inden reconstruct)."
- `ccr:2468` — "seq16 TEK BAŞINA nonce DEĞİL. AEAD nonce/counter = reconstructed full counter."
- Byte-exact (endianness / 12B-pad / salt-XOR) tanımı YALNIZ **DEPRECATED** Appendix D2'de (`ccr:2879` "Historical
  Only", `ccr:2722` "MUST NOT IMPLEMENT"): `ccr:3000` — "nonce 12B ... HKDF'ten 12B (per_message_key türetimi)".

pki-§13 (`pki:455`): "AEAD ChaCha20-Poly1305 (RFC 8439) — key 32B / nonce 12B / tag 16B" — nonce **boyu** 12B sabit,
**kurgusu** değil. cipher_suite `0x0001 = Noise_N_25519_ChaChaPoly_BLAKE2s` (`ccr:2755`) → ChaCha20-Poly1305 primitifi
registry-teyitli (cipher-seçimi DR-GEREKTİRMEZ; yalnız nonce-kurgusu).

## Sprint-1'de uygulanan varsayılan (görev-prompt §2.3)

`nonce = [4B-zero || full_pn u64_be]` (big-endian; dh_epoch nonce'a GİRMEZ — epoch AYRI cipher-state/key'de, ccr-emsal
`epoch_1_root = HKDF(..., u16_be(1))` `ccr:2368`). Bu wire-golden'a PINLENDI (`golden_wire.rs`). Nonce-reuse yapısal-
guard: (session, direction, dh_epoch) başına monoton-full-PN + non-monotonic-seal-reddi.

## Karar-talebi (Abkarada)

1. Nonce byte-kurgusunu SABİTLE: (a) `[4B-zero ‖ full_pn u64_be]` (Sprint-1 varsayılanı, basit, D2-HKDF-türevi-değil)
   VEYA (b) D2-emsali per-message-HKDF (32B-key+12B-nonce = 44B türet) — ama D2 "MUST NOT IMPLEMENT", yeniden-canlandırma-kararı
   gerekir VEYA (c) libsodium-uyumlu bir kurgu (DR-5 ile bağlı).
2. dh_epoch nonce'a mı key'e mi girer teyidi (Sprint-1: key'e; nonce epoch-agnostik + ayrı-cipher-state).

## Etki

- Karar (a)=Sprint-1 ile aynı → yalnız spec-netleştirme; wire-golden değişmez.
- Karar (b)/(c) → `protector.rs::nonce_for` + `golden_wire.rs` PIN güncellenir (izole; tek-fonksiyon).
