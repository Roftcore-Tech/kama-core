# Application Message Protocol (Plane 2 plaintext) — İSKELET v0.1

**Belge:** `application-message-protocol.md` · **Statü:** v1 PROPOSED — A1–A4 için somut öneri yazıldı (2026-09-19); **Hayri (A3/A4) + Abkarada (A1/A2) imzasıyla DESIGN GO**
**Sahip:** Hayri (Application) · **Gözden geçiren:** Abkarada · **Tarih:** 2026-09-18
**Otorite ilişkisi:** ccr §35.6 (plaintext birim kabı `BatchSubMessage.payload` — bu belge o payload'ın **içini** tanımlar) · ccr §3 (Kama-native canonical codec disiplini) · transport-tdd §0 INV-NEIGHBOR-1 + INV-PLAINTEXT-NOT-SEMANTICS · app-rpc §3 [AMENDMENT-4] (sıkıştırma sahipliği)
> **YAMA KAYDI 2026-10-25 — K4G-05d —** §3'e gönderen penceresi ve RECEIPT yükümlülüğü bloğu eklendi (DUR-05d-6, sahip kararı 2026-10-25). Gönderen penceresi kuralı (`MSG_DEDUP_WINDOW`; kalıcı kopyası henüz silinemeyen kayıt dahil) normatif oldu. Alıcının `RECEIPT(delivered)` yükümlülüğü eş ve `msg_id` başınadır, oturumdan bağımsızdır ve pencere altında kapanır. Uygulama `crates/kama-app/src/messaging.rs`. Tel formatı, alan listeleri ve bayt düzeni DEĞİŞMEDİ; üstü çizilen ya da silinen metin yok.

> **YAMA KAYDI 2026-10-12 — K4G-14a —** §3'e 0x09 `SIGNALING_BLOB` kuralları eklendi (0x09 satırına not + tablonun altında bir blok): telde `msg_id` yok → alıcıda dedup YOK, yinelenen blob medya motorunun idempotentliğine kalır (plan K4G-14 NE (1); signaling-seam-v0 §3), ve oturum değişiminde gönderen blob'u yeni oturuma bir kez yeniden sunar (S1 K7) — tutma kuralı (ilk kabulden 60 s, eş başına 256, taşmada en eski düşer) sahip kararıdır (DUR-K4G-14a CEVAP, Abkarada 2026-10-12; plan Ek 2026-10-12 (5)); uygulama `crates/kama-app/src/signaling.rs`; tel formatı, alan listeleri ve bayt düzeni DEĞİŞMEDİ, üstü çizilen ya da silinen metin yok.

> **YAMA KAYDI 2026-10-12 — K4G-09 —** §3 tip tablosuna 0x0B `FILE_RECEIPT` satırı (gövde sabit 9 B: `transfer_id u64_be` + `status u8` ∈ {0x00 ok · 0x01 aborted_by_receiver · 0x02 hash_mismatch · 0x03 timeout · 0x04 rejected}), 0x05 `FILE_ABORT` satırına `0x05 superseded` nedeni ve "yalnız GÖNDEREN kullanır" yön cümlesi (`0x02`/`0x03` kayıtlı kalır, artık üretilmez), §3'e dosya alındısı / `FILE_IDLE_TTL = 60 s` / `FILE_RECEIPT_TTL = 60 s` / oturum değişimi kuralları, §4'e `FILE_RECEIPT` gövde uzunluğu (A2) ve §7'ye vektör sayıları eklendi (`amp-record-v1.json` 17, `amp-record-reject-v1.json` 23; `decode.unknown-msg-type` 0x0B → 0x0C, kimlik aynı) — plan D6 (DUR #108), uygulama `crates/kama-app/src/codec/` + `crates/kama-app/src/file_transfer.rs`; mevcut tiplerin alan listeleri ve bayt düzeni DEĞİŞMEDİ, üstü çizilen ya da silinen metin yok.

> **YAMA KAYDI 2026-10-12 — K4G-08 —** §3'e PTT alıcı kuralları PTT-R0…PTT-R5 plan K4G-08 §2'den birebir eklendi (plan D5, D7 ek; PoC-KAPSAM-001 §2 PTT satırı ve EK-2 madde 1: `PTT_OPEN`, `MEDIA_BUNDLE`, `PTT_CLOSE` Reliable) — uygulama `crates/kama-app/src/ptt.rs` (`PTT_STREAM_IDLE_TTL`, `PTT_MAX_ACTIVE_STREAMS`, `PttStats::ptt_orphan_bundles`); §3'te bu kurallarla doğrudan çelişen cümle yoktu, üstü çizilen metin yok; tel formatı, alan listeleri ve bayt düzeni DEĞİŞMEDİ.

> **YAMA KAYDI 2026-10-12 — K4G-07 —** §3 `msg_id` semantiğinde "yeniden başlatmada devam garantisi yok" üstü çizildi ve yerine gönderenin `msg_id`'yi kalıcı tuttuğu cümle ile D4 beyanlı sınırı (PROVISIONING-FORMAT-v0 §1; PoC-KAPSAM-001 §5 EK-1) eklendi — plan D4 (v1.1), uygulama `crates/kama-app/src/messaging.rs` (`AppStore::reserve_next`, kalıcı outbox); alıcı kuralı, tel formatı, alan listeleri ve bayt düzeni DEĞİŞMEDİ, üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-08 [S2 `msg_id` eş başına + S7 sabitleri — Abkarada kararı]:** §3 `msg_id` semantiği: gönderen sayacı ve alıcı dedup'ı session-scope'tan **eş başına (`PeerRef`)** alındı ve supersession'dan (`Ready(h')` → `Closed(h, Superseded)`) sağ çıkar; S1 K7 / identity §16 yeniden gönderimi ESKİ `msg_id` ile yapılır; dedup penceresinde görülmüş `msg_id`'li TEXT kullanıcıya yeniden teslim EDİLMEZ, `RECEIPT(delivered)` YENİDEN gönderilir (RECEIPT satırına da işlendi). K1'in "oturum yenilenir, u32 yeter" gerekçesi üstü çizildi: sayaç artık oturumla sıfırlanmaz; `u32` tükenirse gönderici tipli hata verir, sarma YOK. S7: `MSG_DEDUP_WINDOW = 4096`, `MAX_OUTBOX_UNACKED = 256`. Uygulama: `crates/kama-app/src/messaging.rs` (B2.2). Tel formatı, alan listeleri ve byte düzeni DEĞİŞMEDİ. Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-06b [L-F07 artıklar-2 F07.4/F07.5 — Abkarada kararı]:** F07.4 (Abkarada'nın yazılı yaması, iş emri 2026-10-06 §0): §7 negatif corpus'a sıkıştırılmış MEDIA_BUNDLE ret vakası `amp-record-reject-v1.decode.media-bundle-compressed-expansion-ratio-exceeded` `ref/amp/encode.py` ile eklendi — 0x0A taşıyıcı, flags.bit0=1, `uncompressed_len/body_len` > `MAX_EXPANSION_RATIO`; hüküm `drop` / `expansion-ratio-exceeded` (mevcut expansion vakasıyla aynı; yeni reason token'ı yok). §3 "asla sıkıştırılmaz" ve §5 kural 6 gönderici kuralıdır; alıcı §5 alıcı kurallarını tipe bakmadan uygular. amp-record-reject-v1.json artık 15 vaka; amp-record-v1.json (11 vaka) baytları DEĞİŞMEDİ. F07.5: §8 A3 ve A4 "imza bekliyor (Hayri)" üstü çizildi — imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi; iş emri 2026-10-06 §0). Alan listeleri ve byte düzeni DEĞİŞMEDİ. Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-06 [F0.5 — Abkarada kararı]:** §7 vektörleri: 0x09 SIGNALING_BLOB ve 0x0A MEDIA_BUNDLE için pozitif ve ret vakaları `ref/amp/encode.py` ile üretildi (vectors/amp-record-v1.json 11 vaka, amp-record-reject-v1.json 14 vaka). §8 A3/A4: "imza bekliyor (Hayri)" işareti — imza atılmadı. Alan listeleri ve byte düzeni DEĞİŞMEDİ.

> **YAMA KAYDI — 2026-10-04 [K1 — Abkarada kararı]:** §8 **A1 ve A2 KAPANDI** (önerildiği gibi). §3 A3 alan listesinde byte düzeltmesi: `TEXT.msg_id` u64 → **u32**; `sent_at_ms` WIRE'DAN ÇIKTI (alıcı kendi zamanını damgalar); `RECEIPT.ref_msg_id` u64 → **u32**. `msg_id` semantiği değişmedi (oturum-scope monoton, duplicate yok). Gerekçe: TEXT başlığı kayıt başlığıyla 23 B'tı ("ok" mesajı telde 75 B, yarısı AMP); düzeltmeyle 11 B. A3/A4 imzası Hayri'de. Üstü çizili metin silinmedi.

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
| 0x01 | `TEXT` | ~~`msg_id u64_be · sent_at_ms u64_be (gönderen saati, bilgi amaçlı; güvenlik kararı DEĞİL) · text_len u16_be · text bytes (UTF-8, doğrulanır; geçersiz → DROP)`~~ **[K1 2026-10-04]** `msg_id u32_be · text_len u16_be · text bytes[text_len] (UTF-8, doğrulanır; geçersiz → DROP)` — gövde öneki 6 B (kayıt başlığıyla 11 B). `sent_at_ms` telde YOK: alıcı kendi zamanını damgalar | **A3 PROPOSED** → Hayri |
| 0x02 | `FILE_BEGIN` | `transfer_id u64_be · file_size u64_be · whole_file_hash [32] (BLAKE2s-256, ccr primitive) · chunk_count u32_be · name_len u8 · name bytes[name_len] (UTF-8 ≤255)` — mime v1'de YOK (uzantıdan); `name_len` u8 = A1'e uygun alan-bazlı fixed-width uzunluk | **A3 PROPOSED** → Hayri |
| 0x03 | `FILE_CHUNK` | `transfer_id u64_be · offset u64_be · payload_len u16_be · payload bytes[payload_len]` (18 B sabit önek) — **per-chunk MAC/hash YOK** (her record zaten E2E-mühürlü) | **A3 PROPOSED** → Hayri |
| 0x04 | `FILE_END` | `transfer_id u64_be` | — |
| 0x05 | `FILE_ABORT` | `transfer_id u64_be · reason u8 (0x01 user_cancel · 0x02 hash_mismatch · 0x03 storage_full · 0x04 timeout ·` **[K4G-09 2026-10-12]** `0x05 superseded ·` `diğer → reserved, bilinmeyen → DROP)` — **[K4G-09 2026-10-12] YALNIZ GÖNDEREN kullanır** (düzen değişmedi, yön alanı yok): alıcı eşten gelen `FILE_ABORT`'u, nedeni ne olursa olsun, yalnız o eşin GÖNDERDİĞİ aktarıma uygular; alıcının kararları 0x0B `FILE_RECEIPT` ile bildirilir. `superseded` = gönderen yarım aktarımı oturum değişiminde kapattı. `0x02 hash_mismatch` ve `0x03 storage_full` alıcı nedenleriydi: kayıtlı kalır (tel ve vektör uyumu) ama artık ÜRETİLMEZ — alıcı `FILE_RECEIPT(hash_mismatch)` / `FILE_RECEIPT(rejected)` gönderir | **A3 PROPOSED** → Hayri |
| 0x06 | `PTT_OPEN` | `stream_id u32_be · codec u8 (0x01 opus · 0x02 lyra) · bundle_ms u8` — speaker = session'ın peer'ı (1:1, ayrı alan gerekmez); codec/bundle_ms talk-spurt boyunca SABİT (KM-SEAM-AM kararı) | **A3 PROPOSED** → Hayri + Yaaz |
| 0x07 | `PTT_CLOSE` | `stream_id u32_be` | — |
| 0x08 | `RECEIPT` | ~~`ref_msg_id u64_be`~~ **[K1 2026-10-04]** `ref_msg_id u32_be` `· kind u8 (0x01 delivered = uygulama record'u işledi · 0x02 read)` — **transport ACK'ten TÜREMEZ**; uygulama-seviyesi canlılık sinyali **[S2 2026-10-08]** tekrar eden TEXT'e (dedup penceresinde görülmüş `msg_id`) de `delivered` YENİDEN gönderilir | **A4 PROPOSED:** M3a'da `delivered` VAR (demo değeri + tek canlılık sinyali), `read` M3b → Hayri/ürün |
| 0x09 | `SIGNALING_BLOB` | `blob_len u16_be · blob bytes[blob_len]` (opak; signaling-seam-v0: core okumaz, parse etmez) — **[K4G-14a 2026-10-12]** `msg_id` TAŞIMAZ → alıcıda dedup YOK; tekrar ve oturum değişimi kuralları tablonun altında | media ekibi |
| 0x0A | `MEDIA_BUNDLE` | `stream_id u32_be · codec u8 · bundle_ms u8 · payload_len u16_be · payload bytes[payload_len]` (payload içi çerçeve yerleşimi media'nın — bu belge yalnız kabı verir) — **asla sıkıştırılmaz** (§5) | media ekibi (bu belge yalnız kabı verir; alan içi KM-SEAM-AM) — A3 dışı |
| 0x0B | `FILE_RECEIPT` | **[K4G-09 2026-10-12]** `transfer_id u64_be` (FILE_BEGIN'deki `transfer_id` ile AYNI genişlik ve kodlama) `· status u8 (0x00 ok · 0x01 aborted_by_receiver · 0x02 hash_mismatch · 0x03 timeout · 0x04 rejected · diğer → reserved, bilinmeyen → DROP)` — gövde sabit 9 B (§4); YALNIZ ALICI gönderir, gönderen onu o eşe GİDEN aktarımına uygular; kurallar tablonun altında | **plan D6** (Abkarada, K4G-09) |

**`msg_id` semantiği [A3 PROPOSED → Hayri]:** gönderen tarafından **monoton artan ~~u64~~ u32** [K1 2026-10-04: ~~oturum `SESSION_MAX_AGE = 7 gün`de yenilendiği için u32 yeter; sayaç sarmadan oturum yenilenir~~ **[S2 2026-10-08]** sayaç supersession'da (yenileme dahil) SIFIRLANMAZ; `u32` tükenirse gönderici tipli hata verir, sarma YOK] (~~session-scope'ta~~ **eş başına (`PeerRef`) [S2 2026-10-08]**; ~~yeniden başlatmada devam garantisi yok~~ **[K4G-07 2026-10-12]** ~~→ yeni session yeni sayaç~~ **[S2 2026-10-08]** → sayaç ve alıcı dedup'ı supersession'dan (`Ready(h')` → `Closed(h, Superseded)`) sağ çıkar); aynı ~~session'da~~ **eşe [S2 2026-10-08]** duplicate üretilmez; **alıcı teslim sırasını `msg_id` ile ZORLAMAZ** (sıra Transport'un Reliable politikasıyla gelir; `msg_id` yalnız UI sıralaması, duplicate tespiti ve `RECEIPT.ref_msg_id` referansı içindir). ~~`sent_at_ms` güvenlik/sıra kararı için kullanılmaz.~~ [K1 2026-10-04: `sent_at_ms` telden çıkarıldı.] **[K4G-07 2026-10-12]** Gönderen `msg_id`'yi kalıcı tutar. **Beyanlı sınır (D4):** kalıcı durum tamamen kaybolup aynı kimlik yeniden kullanılırsa güvenli devam garanti edilmez; yeniden provisioning zorunludur (PROVISIONING-FORMAT-v0 §1).

**[S2 2026-10-08 — Abkarada kararı] Supersession, yeniden gönderim, tekrar:** S1 K7 ve identity §16 (peer-restart supersession, adım 3) gereği Application, `RECEIPT(delivered)`'ı gelmemiş TEXT'leri yeni oturumda **ESKİ `msg_id` ile** (aynı kayıt) yeniden gönderir; bu aynı-oturum retry'ı değildir (S1 K8). Alıcı dedup'ı eş başınadır: `MSG_DEDUP_WINDOW = 4096` `msg_id`'lik pencerede görülmüş bir `msg_id` taşıyan TEXT kullanıcıya YENİDEN teslim edilmez, ama `RECEIPT(delivered)` YENİDEN gönderilir (ilk RECEIPT eski oturumla kaybolmuş olabilir). Gönderen tarafta RECEIPT bekleyen TEXT tavanı `MAX_OUTBOX_UNACKED = 256` (S7); tavan doluyken yeni TEXT kabul edilmez (tipli geri basınç).

**[2026-10-25 — K4G-05d] Gönderen penceresi ve RECEIPT yükümlülüğü:** Gönderen, RECEIPT'i gelmemiş en eski `msg_id` (RECEIPT'i gelmiş ama kalıcı kopyası henüz silinemeyen dahil) ile atayacağı `msg_id` arası `MSG_DEDUP_WINDOW`'a ulaşınca yeni `msg_id` ayırmaz (tipli geri basınç). Alıcının `RECEIPT(delivered)` yükümlülüğü eş ve `msg_id` başınadır, oturumdan bağımsızdır: Transport'un kabul etmediği RECEIPT eşin güncel ya da sıradaki oturumuna sunulur; kabulden sonrası Transport'undur. Bu aynı-oturum retry'ı değildir (S1 K8): reddedilen birim tele çıkmamıştır. Görülen en büyük `msg_id`'nin `MSG_DEDUP_WINDOW` kadar altında kalan yükümlülük, bu kurala uyan gönderende kapanmıştır ve tutulmaz. Kurala uymayan gönderenin (D4 dahil) pencere altı kalan RECEIPT'i gelmeyebilir (beyanlı sınır).

**[K4G-08 2026-10-12 — plan D5, D7 ek; PoC-KAPSAM-001 §2 PTT satırı, EK-2 madde 1] PTT taşıma ve alıcı kuralları:**

- PTT-R0: `PTT_OPEN`, `MEDIA_BUNDLE`, `PTT_CLOSE` Reliable taşınır; alıcı `MEDIA_BUNDLE`'ları sırayla oynatır, eksik parça gelene kadar bekler, geç gelen parçayı **düşürmez**.
- PTT-R1: Bilinen `stream_id` için tekrar gelen `PTT_OPEN` idempotenttir; yeni akış yaratmaz, hata değildir.
- PTT-R2: Bilinmeyen `stream_id` için gelen `PTT_CLOSE` sessizce yok sayılır.
- PTT-R3: Bilinmeyen `stream_id` için gelen `MEDIA_BUNDLE` düşürülür; sayaç `ptt_orphan_bundles` artar.
- PTT-R4: Bir akışa `PTT_STREAM_IDLE_TTL = 120 s` boyunca `MEDIA_BUNDLE` gelmezse alıcı akışı yerel olarak kapatır (CLOSE üretmez).
- PTT-R5: Aynı eşten aynı anda en çok `PTT_MAX_ACTIVE_STREAMS = 2` akış; fazlası düşürülür.

**[K4G-09 2026-10-12 — plan D6 (DUR #108); PoC-KAPSAM-001 §2 fotoğraf satırı] Dosya alındısı, zamanlayıcılar ve `FILE_ABORT` yönü:**

- **Yön.** `FILE_ABORT` yalnız GÖNDEREN tarafından kullanılır; düzeni değişmez, yön alanı eklenmez. Alıcı eşten gelen `FILE_ABORT`'u, nedeni ne olursa olsun, yalnız o eşin GÖNDERDİĞİ aktarıma uygular. Alıcı `FILE_ABORT` GÖNDERMEZ; her kararını `FILE_RECEIPT` ile bildirir.
- **`FILE_RECEIPT`** yalnız ALICI tarafından, aktarım başına en çok bir kez gönderilir; `transfer_id` gönderenin `FILE_BEGIN`'deki kimliğidir ve gönderen onu yalnız o eşe GİDEN aktarımına uygular. `status`: `ok` = `FILE_END` görüldü, bütün parçalar geldi ve `whole_file_hash` tuttu · `hash_mismatch` = bütün parçalar geldi, özet tutmadı · `aborted_by_receiver` = alıcının kullanıcısı kabul edilmiş aktarımı iptal etti · `timeout` = `FILE_IDLE_TTL` boyunca parça gelmedi · `rejected` = alıcı `FILE_BEGIN`'i kabul etmedi: yuva yok, `file_size > MAX_REASSEMBLY_BYTES` (S7 2026-10-08: 64 MiB, BL B2.3) ya da aktarımın `FILE_ABORT`'u `FILE_BEGIN`'inden önce geldi (kabul edilmeden iptal edilmiş aktarım sessiz düşmez). S8 gereği tutarsız `FILE_BEGIN` sessiz DROP kalır (alındı yok). Teslim edildi yalnız `ok`'tur; alındı yeniden gönderim tetiklemez (kaldığı yerden devam YOK).
- **Alıcı zamanlayıcısı:** `FILE_IDLE_TTL = 60 s` — kabul edilmiş bir gelen aktarıma bu süre boyunca yeni `FILE_CHUNK` gelmezse alıcı yuvayı boşaltır ve `FILE_RECEIPT(timeout)` gönderir. Süre `FILE_BEGIN` kabul edilince başlar ve kabul edilen her yeni parçada sıfırlanır; tekrar ya da atılan parça ve `FILE_END` sıfırlamaz.
- **Gönderen zamanlayıcısı:** `FILE_RECEIPT_TTL = 60 s` — `FILE_END` Transport'a kabul ettirildikten sonra bu süre içinde `FILE_RECEIPT` gelmezse aktarım gönderende başarısızdır (tele bir şey gitmez); sonra gelen alındı yok sayılır.
- **Oturum değişimi:** son kaydı (`FILE_END`) henüz kabul ettirilmemiş (yarım) bir aktarımın oturumu değişirse gönderen kalan kayıtları göndermez, aktarımı yeni oturumdan `FILE_ABORT(superseded)` ile kapatır. Yeniden gönderim yeni `transfer_id` ile tam aktarımdır (UI "yeniden gönder"). Kullanıcı iptali bekliyorsa (`FILE_ABORT(user_cancel)` henüz kabul ettirilmedi) o kayıt yeni oturumdan gider ve nedeni `user_cancel` kalır. `FILE_END`'i kabul ettirilmiş aktarım alındısını beklemeye devam eder.

Dosya için Application'ın SAHİP OLMADIKLARI (INV-NEIGHBOR-1 / LAW-5): chunk retry · chunk ACK · kayıp tespiti · pencereli retransmit · FEC · per-chunk bütünlük. Chunk boyutu `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18` (A2; off-by-header yok).

**[K4G-14a 2026-10-12 — plan K4G-14 NE (1); PoC-KAPSAM-001 §2 "Sesli / görüntülü arama", K7; signaling-seam-v0 §3] `SIGNALING_BLOB` tekrarı ve oturum değişimi:**

- **Dedup YOK.** 0x09 telde `msg_id` (ya da başka bir kimlik) TAŞIMAZ: alıcı tekrarı tespit edemez ve her geçerli `SIGNALING_BLOB`'u — aynı baytlarla yeniden gelen dahil — teslim eder; `msg_id` dedup penceresi (`MSG_DEDUP_WINDOW`) bu tipe UYGULANMAZ. Yinelenen blob'u medya motoru idempotent işler (signaling-seam-v0 §3: at-least-once, duplicate mümkün; sıra garanti edilmez).
- **Oturum değişimi — yeniden sunum (S1 K7).** 0x09'un alındısı yoktur: gönderen teslimi öğrenemez ve Transport `h`'de teslim edilmemiş birimi `h'`ye TAŞIMAZ. Gönderen her blob'u en son KABUL edildiği oturum tutamacıyla tutar; eşin güncel oturumu başka bir tutamaçsa blob oraya AYNI baytlarla, `Reliable`, BİR KEZ yeniden sunulur — geçici retlerde kabul edilene dek sonraki denetimde; kabul edilen blob o tutamaca bir daha verilmez. Bu aynı-oturum retry'ı değildir (S1 K8). Eski oturumda teslim edilmiş blob da yeniden sunulur ve alıcıda ikinci kez teslim edilir (yukarıdaki madde). **Tutma kuralı — sahip kararı (DUR-K4G-14a CEVAP, 2026-10-12):** gönderen blob'u ilk kabulden itibaren `SIGNALING_RESEND_TTL = 60 s` yeniden sunum için tutar (yeniden sunum süreyi uzatmaz) ve eş başına en çok `SIGNALING_RESEND_PER_PEER = 256` blob tutar; tavan dolunca KENDİ en eskisini bırakır, yeni gönderimi reddetmez. Değerler sahip kararıyla `FILE_RECEIPT_TTL` ve `MAX_OUTBOX_UNACKED` ile EŞİT SEÇİLDİ, onlara BAĞLI DEĞİLDİR (onlar değişirse bu kural kendiliğinden değişmez). 120 s reddedildi: sinyalleşme zamana duyarlıdır, 60 s'den eski bir teklifin yeniden sunumu bayat çağrı üretir. Süresi dolmuş ya da bırakılmış blob yeniden sunulmaz; kaybını medya motorunun kendi uygulama-ACK'i karşılar (signaling-seam-v0 §3). Plan D8 gereği kayıt bellektedir (`AppStore` yalnız sayaç / dedup / outbox taşır): süreç yeniden başlarsa tutulan blob'lar yeniden sunulmaz (PoC-KAPSAM-001 K2).

## 4. Boyut politikası
`5 + body_len ≤ TransportCapabilities.max_plaintext_unit_bytes` ⇒ `body_len ≤ MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − 5)` — Application record'u **limit içinde üretir** (S1 K3/K5): `AppMessageRecord` bir bütün olarak `send()`'e verilir; **Transport record içini bölmez**, yalnız kendi taşıma birimini yönetir (birden çok record'u bir birime batch'ler, birimi segment/FEC'ler). Büyük içerik Application tarafından **record düzeyinde** parçalanır (FILE_CHUNK). Ürün limiti `MAX_BODY_BYTES`'tan küçük seçilebilir `[KALİBRASYON — A2]`. ~~Eski: "record transport bütçesini bilmez; Transport böler"~~ — S1 ile çelişiyordu, S1 kazandı (2026-09-19).

**[K4G-09 2026-10-12] A2 — `FILE_RECEIPT` gövdesi:** `FILE_RECEIPT_BODY_BYTES = 9` (`transfer_id u64_be` 8 + `status u8` 1), sabit; değişken-uzunluklu alan yok. `body_len < 9` → DROP (A1: sabit alanlar gövdeye sığmıyor); `body_len > 9` → DROP (artık bayt; tek kanonik biçim). Her geçerli `max_plaintext_unit_bytes`'a sığar (`MAX_BODY_BYTES ≥ 18`, A2).

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

**[K4G-09 2026-10-12]** `vectors/amp-record-v1.json` 17 vaka (önce 11; + 0x0B `FILE_RECEIPT`'in her `status`'u için 1 + `FILE_ABORT` reason `0x05 superseded`), `vectors/amp-record-reject-v1.json` 23 vaka (önce 20; + `FILE_RECEIPT` bilinmeyen status, kısa gövde, uzun gövde). `decode.unknown-msg-type` vakası 0x0B'den 0x0C'ye taşındı (0x0B artık kayıtlı; kimlik aynı). Üretici `ref/amp/encode.py`; Rust codec tüketir (`crates/kama-app/tests/amp_vectors.rs`).

## 8. Açık kararlar
| # | Karar | Sahip |
|---|---|---|
| ~~A1~~ | ~~body-içi protobuf yasağı — PROPOSED: YASAK, fixed-width + len-prefixed (§2)~~ **KAPANDI 2026-10-04 (Abkarada):** gövdede protobuf YASAK; sabit-genişlik big-endian sayısal alanlar; her değişken-uzunluklu alan kendi exact layout'ında belirtilen sabit-genişlik uzunluk alanıyla önce gelir; opsiyonel alan yok | — |
| ~~A2~~ | **KAPANDI 2026-10-04 (Abkarada) — önerildiği gibi:** `MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − 5)` · `MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES` · `MAX_EXPANSION_RATIO = 64` · `COMPRESS_MIN_BYTES = 64` · `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18`. Önceki satır: ~~**PROPOSED (türetimli, onay: Abkarada) — byte hesabı düzeltildi 2026-09-19:** `APP_RECORD_HEADER_BYTES = 5` · `MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − 5)` · `MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES` (bounded output) · `MAX_EXPANSION_RATIO = 64` (bomb koruması) · `COMPRESS_MIN_BYTES = 64` (3 B/s'de ≈ 20 s; altında zstd çerçeve ek yükü kazancı yer) · `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18` (transfer_id 8 + offset 8 + payload_len 2) — ~~eski `−12` ve `min(65535, max_plaintext_unit_bytes)` off-by-header hatasıydı~~~~ | — |
| A3 | TEXT/FILE_BEGIN/FILE_ABORT/PTT_OPEN alan listeleri — PROPOSED (§3 tablosu) | Hayri (PTT: + Yaaz) — **[2026-10-04] Abkarada tarafı kapalı** (byte düzeltmesi §3'e işlendi: `msg_id`/`ref_msg_id` u32, `sent_at_ms` yok); imza Hayri'de **[2026-10-06] ~~imza bekliyor (Hayri)~~** **imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi)** |
| A4 | RECEIPT — PROPOSED: `delivered` M3a, `read` M3b | Hayri / ürün **[2026-10-06] ~~imza bekliyor (Hayri)~~** **imza: Abkarada, 2026-10-06 (Hayri'nin kalemleri bu sprint Abkarada'ya devredildi)** |
| A5 | Kova-padding | Beta (ertelendi; v1 freeze'i bloklamaz) |
