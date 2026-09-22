# Application Message Protocol (Plane 2 plaintext) — İSKELET v0.1

**Belge:** `application-message-protocol.md` · **Statü:** v1 PROPOSED — A1–A4 için somut öneri yazıldı (2026-09-19); **Hayri (A3/A4) + Abkarada (A1/A2) imzasıyla DESIGN GO**
**Sahip:** Hayri (Application) · **Gözden geçiren:** Abkarada · **Tarih:** 2026-09-18
**Otorite ilişkisi:** ccr §35.6 (plaintext birim kabı `BatchSubMessage.payload` — bu belge o payload'ın **içini** tanımlar) · ccr §3 (Kama-native canonical codec disiplini) · transport-tdd §0 INV-NEIGHBOR-1 + INV-PLAINTEXT-NOT-SEMANTICS · app-rpc §3 [AMENDMENT-4] (sıkıştırma sahipliği)
**Bu belge NE DEĞİLDİR:** Plane 1 hub-RPC (app-rpc'nin) · transport birim çerçevesi (ccr §35.6 BatchPayload, Transport üretir) · oturum/anahtar (Crypto) · teslim güvenilirliği (Transport)

---

## 1. Yerleşim

```
Application nesnesi (TEXT / FILE_* / PTT_* / RECEIPT / SIGNALING_BLOB)
    ↓ AppMessageCodec (bu belge)
PreparedUnit { bytes: AppMessageRecord, compression_policy }
    ↓ transport.send(session, PreparedUnit)             ← Application'ın TEK alt komşusu
Transport: batch (ccr §35.6 BatchPayload) / segment / FEC → UnsealedTransportUnit
Crypto  : seal → SealedWireUnit (ccr §35.6 22 B / noise-wire 26 B)
```
Bir `AppMessageRecord` = tam olarak bir `BatchSubMessage.payload`; record `max_plaintext_unit_bytes`'ı aşamaz (S1 K3 — aşarsa `UnitTooLarge`). Transport kaç record'u bir birime koyacağını **kendisi** seçer (uzunluk yapısal bilgidir, semantik değil) ve **record içini bölmez**.

## 2. Kayıt başlığı (NORMATİF — sabit genişlik, varint YOK, deterministik)

```
AppMessageRecord {
  msg_version   : u8        # 0x01 (v1); bilinmeyen → DROP (reject değil: ileri uyum)
  msg_type      : u8        # §3 registry; 0x00 ve 0xF0..0xFF reserved → REJECT
  flags         : u8        # bit0 = compressed (§5) · bit1..7 reserved-zero → nonzero REJECT
  body_len      : u16_be    # body byte sayısı (sıkıştırılmışsa sıkıştırılmış uzunluk)
  body          : bytes[body_len]
}
```
Neden Kama-native ve protobuf değil: **deterministik byte bütçesi ve parser karmaşıklığı** — güvenlik gerekçesi değil. ccr §3 ve §35.6 ile aynı disiplin (deterministik, varint yok, tek canonicalization); protobuf yalnız operatör backend'inde (ADR-07). **A1 [PROPOSED → onay: Abkarada]:** body içi alanlar için de protobuf **YASAK**; v1 body codec = **fixed-width sayısal alanlar (big-endian)** + **her değişken-uzunluklu alan, kendi exact layout'ında belirtilen fixed-width uzunluk alanıyla ÖNCE gelir** (alan bazında `u8` veya `u16_be`; implicit/enclosing-length YOK; toplam ≤ `body_len` — aşarsa DROP). Opsiyonel alan yok (v1'de her alan zorunlu; opsiyonellik yeni `msg_type` ile).

```
APP_RECORD_HEADER_BYTES = 5                                   # msg_version 1 + msg_type 1 + flags 1 + body_len 2
MAX_BODY_BYTES          = min(65535, max_plaintext_unit_bytes − 5)
MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES
``` Gerekçe: düşük bantta her byte sayılır, varint/tag ek yükü ve parser yüzeyi istenmez; tek canonicalization (ccr §3 disiplini).

## 3. Tip registry (v1)

| Kod | Tip | Zorunlu alanlar (spec'ten türetilen) | Alan listesi |
|---|---|---|---|
| 0x01 | `TEXT` | `msg_id u64_be · sent_at_ms u64_be (gönderen saati, bilgi amaçlı; güvenlik kararı DEĞİL) · text_len u16_be · text bytes (UTF-8, doğrulanır; geçersiz → DROP)` | **A3 PROPOSED** → Hayri |
| 0x02 | `FILE_BEGIN` | `transfer_id u64_be · file_size u64_be · whole_file_hash [32] (BLAKE2s-256, ccr primitive) · chunk_count u32_be · name_len u8 · name bytes[name_len] (UTF-8 ≤255)` — mime v1'de YOK (uzantıdan); `name_len` u8 = A1'e uygun alan-bazlı fixed-width uzunluk | **A3 PROPOSED** → Hayri |
| 0x03 | `FILE_CHUNK` | `transfer_id u64_be · offset u64_be · payload_len u16_be · payload bytes[payload_len]` (18 B sabit önek) — **per-chunk MAC/hash YOK** (her record zaten E2E-mühürlü) | **A3 PROPOSED** → Hayri |
| 0x04 | `FILE_END` | `transfer_id u64_be` | — |
| 0x05 | `FILE_ABORT` | `transfer_id u64_be · reason u8 (0x01 user_cancel · 0x02 hash_mismatch · 0x03 storage_full · 0x04 timeout · diğer → reserved, bilinmeyen → DROP)` | **A3 PROPOSED** → Hayri |
| 0x06 | `PTT_OPEN` | `stream_id u32_be · codec u8 (0x01 opus · 0x02 lyra) · bundle_ms u8` — speaker = session'ın peer'ı (1:1, ayrı alan gerekmez); codec/bundle_ms talk-spurt boyunca SABİT (KM-SEAM-AM kararı) | **A3 PROPOSED** → Hayri + Yaaz |
| 0x07 | `PTT_CLOSE` | `stream_id u32_be` | — |
| 0x08 | `RECEIPT` | `ref_msg_id u64_be · kind u8 (0x01 delivered = uygulama record'u işledi · 0x02 read)` — **transport ACK'ten TÜREMEZ**; uygulama-seviyesi canlılık sinyali | **A4 PROPOSED:** M3a'da `delivered` VAR (demo değeri + tek canlılık sinyali), `read` M3b → Hayri/ürün |
| 0x09 | `SIGNALING_BLOB` | `blob_len u16_be · blob bytes[blob_len]` (opak; signaling-seam-v0: core okumaz, parse etmez) | media ekibi |
| 0x0A | `MEDIA_BUNDLE` | `stream_id u32_be · codec u8 · bundle_ms u8 · payload_len u16_be · payload bytes[payload_len]` (payload içi çerçeve yerleşimi media'nın — bu belge yalnız kabı verir) — **asla sıkıştırılmaz** (§5) | media ekibi (bu belge yalnız kabı verir; alan içi KM-SEAM-AM) — A3 dışı |

**`msg_id` semantiği [A3 PROPOSED → Hayri]:** gönderen tarafından **monoton artan u64** (session-scope'ta; yeniden başlatmada devam garantisi yok → yeni session yeni sayaç); aynı session'da duplicate üretilmez; **alıcı teslim sırasını `msg_id` ile ZORLAMAZ** (sıra Transport'un Reliable politikasıyla gelir; `msg_id` yalnız UI sıralaması, duplicate tespiti ve `RECEIPT.ref_msg_id` referansı içindir). `sent_at_ms` güvenlik/sıra kararı için kullanılmaz.

Dosya için Application'ın SAHİP OLMADIKLARI (INV-NEIGHBOR-1 / LAW-5): chunk retry · chunk ACK · kayıp tespiti · pencereli retransmit · FEC · per-chunk bütünlük. Chunk boyutu `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18` (A2; off-by-header yok).

## 4. Boyut politikası
`5 + body_len ≤ TransportCapabilities.max_plaintext_unit_bytes` ⇒ `body_len ≤ MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − 5)` — Application record'u **limit içinde üretir** (S1 K3/K5): `AppMessageRecord` bir bütün olarak `send()`'e verilir; **Transport record içini bölmez**, yalnız kendi taşıma birimini yönetir (birden çok record'u bir birime batch'ler, birimi segment/FEC'ler). Büyük içerik Application tarafından **record düzeyinde** parçalanır (FILE_CHUNK). Ürün limiti `MAX_BODY_BYTES`'tan küçük seçilebilir `[KALİBRASYON — A2]`. ~~Eski: "record transport bütçesini bilmez; Transport böler"~~ — S1 ile çelişiyordu, S1 kazandı (2026-09-19).

## 5. Sıkıştırma wire sözleşmesi (NORMATİF — app-rpc §3 AMENDMENT-4'ün exact hâli)

```
flags.bit0 = 1  ⇒  body = { compression_id u8 | uncompressed_len u32_be | payload bytes[body_len − 5] }
compression_id : 0x01 = zstd (v1'de TEK algoritma); 0x00 yasak (bit0=0 kullanılır); diğer → REJECT
uncompressed_len ≤ MAX_DECOMPRESSED_MESSAGE      (= MAX_BODY_BYTES, A2 PROPOSED)
sözlük        : YOK (v1); paylaşımlı/ön-eğitimli sözlük YASAK (context izolasyonu)
context       : her record için AYRI, taze compressor context; record'lar arası state YOK
```
**Alıcı kuralları (fail-closed, bounded):**
1. `uncompressed_len > MAX_DECOMPRESSED_MESSAGE` → record DROP (decompress'e başlanmaz — bomb koruması).
2. Decompress çıktısı `uncompressed_len`'den **fazla** ya da **eksik** → DROP (bounded output; stream truncated/inflated).
3. Geçersiz zstd frame → DROP. Retry YOK, sinyal YOK (LAW-5).
4. `MAX_EXPANSION_RATIO` (= 64, A2 PROPOSED) — `uncompressed_len / body_len` bu oranı aşarsa DROP (ek bomb sınırı).
**Gönderici kuralları:**
5. `body_len_uncompressed < COMPRESS_MIN_BYTES` (= 64, A2 PROPOSED) → sıkıştırma YOK (küçük mesaj; kazanç yok, uzunluk sinyali).
6. Tip-türevi: `MEDIA_BUNDLE`, `SIGNALING_BLOB`(zaten kodlanmış olabilir), `FILE_CHUNK` (içerik türü bilinmiyorsa) → varsayılan **sıkıştırma YOK**; `TEXT` → ON. Bu karar **yalnız burada** alınır — Transport tip bilmez (INV-PLAINTEXT-NOT-SEMANTICS).
7. Sıkıştırılmış çıktı `body_len_uncompressed`'dan büyükse → sıkıştırmadan gönder (bit0=0).
**Side-channel ifadesi (kontrollü):** HTTP/TLS-tarzı cross-request oracle yüzeyi yoktur; attacker-controlled plaintext ile secret-bearing plaintext aynı context'te sıkıştırılmaz (kural: her record izole); ciphertext uzunluk sızıntısı **residual metadata** olarak kabul edilir; kova-padding `[KARAR — Beta knob]`.
**CCR / sertifika / anahtar** bu yoldan **geçmez**: canonical codec (ccr §3) ile serileştirilir, opak `bytes` olarak taşınır, sıkıştırılmaz (transport-tdd §0 INV-PLAINTEXT-NOT-SEMANTICS).

## 6. Sürümleme
`msg_version` bilinmeyen → DROP · `msg_type` reserved → REJECT · bilinmeyen-ama-reserved-olmayan tip → DROP (ileri uyum) · `flags` reserved nonzero → REJECT. Tip ekleme = minor; başlık değişimi = `msg_version` artar.

## 7. Vektörler (SEAM/contract kapısı)
Her tip için exact record byte'ları (1 pozitif) + negatif corpus: reserved type, reserved flag, body_len uyumsuz, compressed: `uncompressed_len` aşımı, truncated stream, expansion aşımı, invalid frame. `[KARAR — Hayri üretir, Hasan koşar]`

## 8. Açık kararlar
| # | Karar | Sahip |
|---|---|---|
| A1 | body-içi protobuf yasağı — PROPOSED: YASAK, fixed-width + len-prefixed (§2) | Abkarada |
| A2 | **PROPOSED (türetimli, onay: Abkarada) — byte hesabı düzeltildi 2026-09-19:** `APP_RECORD_HEADER_BYTES = 5` · `MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − 5)` · `MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES` (bounded output) · `MAX_EXPANSION_RATIO = 64` (bomb koruması) · `COMPRESS_MIN_BYTES = 64` (3 B/s'de ≈ 20 s; altında zstd çerçeve ek yükü kazancı yer) · `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18` (transfer_id 8 + offset 8 + payload_len 2) — ~~eski `−12` ve `min(65535, max_plaintext_unit_bytes)` off-by-header hatasıydı~~ | Abkarada + Hayri |
| A3 | TEXT/FILE_BEGIN/FILE_ABORT/PTT_OPEN alan listeleri — PROPOSED (§3 tablosu) | Hayri (PTT: + Yaaz) |
| A4 | RECEIPT — PROPOSED: `delivered` M3a, `read` M3b | Hayri / ürün |
| A5 | Kova-padding | Beta (ertelendi; v1 freeze'i bloklamaz) |
