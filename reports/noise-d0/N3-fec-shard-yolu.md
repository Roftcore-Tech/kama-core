# N3 — FEC-SHARD-YOLU UYUM-RAPORU [RAPOR-ONLY, KOD-YAZILMADI]

> **Kapsam:** noise-wire-D0-A N3 (kama-noise-wire-tdd §12). **Karar-DURUMU:** N3 KESİN — FEC-shard =
> AYRI-EXTENDED-secure-wire-class (§2.5 h_ext + §2.6 26B-outer); yeniden-seçilMEZ. Bu rapor mevcut
> `fec.rs` FEC_SHARD(0x20) yolunu bu **donmuş** karara karşı ENVANTERLER (uyum-doğrulaması), tasarımı
> yeniden-açmaz.
> **Otorite:** `specs/kama-noise-wire-tdd-v2.0.md` §2.5/§2.6/§2.56/§2.8 · `specs/fec-core-tdd-v3.0.md`
> §0.8 (INV-FEC-1 · FEC-SHARD-TAŞIMA=AYRI-EXTENDED-class, PATCH-2).
> **SONUÇ (özet):** mevcut `fec.rs` FEC_SHARD(0x20) framing yolu §2.6-EXTENDED layout'a **UYMUYOR**
> (yapısal-uyumsuz, byte-layout-farklı, Cryptex-ihlali). D1'de yeniden-yazılacak. Ayrıntı §3-4-5.

---

## (1) SPEC NE DİYOR (§2.5 · §2.6 · §2.56 · fec-core §0.8)

### §2.5 — PN-namespace + WIDTH-DEMUX [YOL-1 seçildi; YOL-2 reddedildi]
FEC-shard'lar SESSION_DATA-envelope'una (h_legacy) GİRMEZ; **ayrı `h_ext` parser'ı** seçer. Hub, decrypt-
ÖNCESİ hangi parser'ı çalıştıracağını **HANDLE-uzayıyla** belirler (frame_type byte'ıyla DEĞİL):
```
h_legacy : [h_legacy:4][seq16:2][ciphertext:N][tag:16]           (mühürlü SESSION_DATA — DEĞİŞMEZ)
h_ext    : [h_ext:4][wire_format:1][wire_flags:1][pn32:4][ciphertext:N][tag:16]   (yeni FEC/control)
           wire_format = EXTENDED_V1 (0x01) · wire_flags.bit0 = dh_epoch · diğer-bitler=0→REJECT
```
- Hub `h_ext`'i HANDLE_ASSIGN (§2.55) ile dağıtır; parser-layout'unu **decrypt-öncesi handle'dan** seçer.
- `wire_format + wire_flags + pn32` **AAD'ye dahildir** (§2.8: `"KAMA-WIRE-EXT-V1" ‖ h_ext ‖ wf ‖ flags ‖ pn`).
- FEC source/repair/control semantiği **inner-ciphertext'te**; outer SIZDIRMAZ (tüm-extended tek-görünüm → Cryptex).

### §2.6 — FINAL-BYTE-LAYOUT [KİLİTLİ]
**EXTENDED-OUTER (fixed 26B + encrypted_inner):**
```
h_ext u32_be(4) | wire_format u8=0x01(1) | wire_flags u8 bit0=dh_epoch(1) | pn_trunc u32_be(4) | ciphertext var | tag 16B
→ sabit-outer = 4+1+1+4+16 = 26B ; FEC-semantiği bu 26B'nin HİÇBİRİNDE yok (yalnız routing/PN/epoch/tag).
```
**FEC-SHARD ENCRYPTED-INNER (§2.56 UNIFIED-HEADER + FEC variant_body) — ciphertext'in İÇİNDE:**
```
inner_version u8=0x01 · inner_kind u8 (0x01 FEC_SOURCE | 0x02 FEC_REPAIR) · flags u8 (reserved-nonzero→reject) ·
profile_id u8 · block_id u32_be · esi u16_be · k_actual u8 · symbol_bytes[profile.E]
→ inner-header = 1+1+1+1+4+2+1 = 11B, sonra TAM-E symbol.  symbol_len ALANI YOK (MADDE-6: her shard TAM profile.E).
```
`actual_payload_len` wire-header'da DEĞİL — RS-kodlanan CANONICAL-SOURCE-SYMBOL-PREFIX'inde
(`message_id·chunk_offset·actual_payload_len·source_flags`), kayıpta metadata da recover edilir.

### §2.56 — INNER_KIND-REGISTRY
Decrypt-sonrası tek-birleşik alan: `inner_version(0x01) · inner_kind · variant_body`. FEC codepoint'leri:
`0x01 FEC_SOURCE`, `0x02 FEC_REPAIR` (control'lar 0x10..0x18). Bilinmeyen inner_kind → FAIL-CLOSED reject.

### fec-core §0.8 — TRANSPORT-NON-REGRESSION + INV-FEC-1
- FEC-SHARD-TAŞIMA [PATCH-2]: FEC source/repair shard'ları **SESSION_DATA-DEĞİLDİR**, **22B-envelope'u SARMAZ**;
  **AYRI-EXTENDED-class** (§2.5 h_ext). ▼KOD-ZEMİNİ-1: mevcut fec.rs FEC_SHARD(0x20) yolu bu karara UYUYOR-mu.
- INV-FEC-1 (code-then-encrypt-authenticate-shards): FEC plaintext-block'ta encode → per-shard AEAD → wire'da
  YALNIZ ciphertext. Alıcı: AEAD-verify SONRA decode. Metadata (block_id/esi/source-repair/profil) AEAD-
  ciphertext'te (§2 Cryptex; AAD-DEĞİL).

---

## (2) MEVCUT KOD NE YAPIYOR (dosya:satır)

### `crates/adaptive-transport/src/fec.rs` — FEC_SHARD(0x20) framing codec (TAM okundu)
**Wire-frame layout** (`fec.rs:47-57` encode / `fec.rs:67-99` decode):
```
offset  alan                        genişlik  kaynak
0       frame_type = 0x20           1B        fec.rs:17 (FRAME_TYPE_FEC_SHARD), :49 push
1       block_id  (u32_be)          4B        fec.rs:30 alan, :51 encode
5       shard_index (u8)            1B        fec.rs:32 alan, :52 encode
6       k  (u8)                     1B        fec.rs:34 alan, :53 encode
7       r  (u8)                     1B        fec.rs:36 alan, :54 encode
8       shard_len (u16_be)          2B        fec.rs:38 alan, :54 encode
10      opaque_secured = ct(shard_len) ‖ tag(16)   değişken   fec.rs:41 alan, :55 encode; :84-85 decode blob_len=shard_len+16
```
- Cleartext framing header = **10B** (`fec.rs:48` `10 + opaque_secured.len()`); + `SHARD_TAG_LEN=16B` (`fec.rs:24`)
  tag opaque_secured içinde → toplam-overhead = 10 + 16 = **26B** (ct hariç).
- `opaque_secured` (= ct ‖ aead_tag) transport için TEK-OPAK-BLOB: parse/verify EDİLMEZ (`fec.rs:39-41, :82-85`).
  Kripto bu crate'te YOK (`fec.rs:5-7`; INV-ENGINE-SOC/INV-FEC-1 kastı).
- Bounds/validation: `k+r>255 → InvalidShardCount` (`fec.rs:79-81`), truncated/trailing typed-error (`fec.rs:84-90`).
- **METADATA-LEAK-01 (dosya kendi kabul ediyor, `fec.rs:9-11`):** framing metadata PLAINTEXT; k/r sızıntısı
  → DPI/EW okuyabilir. "Alpha KABUL, Beta+ minimize." (§2.6-Cryptex bu sızıntıyı D1'de kapatır.)

**Golden-vektör (`tests/tv_fec.rs:25`)** — byte-for-byte teyit:
`20 00 00 00 01 02 08 04 00 03  AA BB CC  11×16` (block_id=1, shard_index=2, k=8, r=4, shard_len=3, 29B).

### `crates/adaptive-transport/src/sender.rs` — TransmitKind::FecParity
`sender.rs:126-135` `enum TransmitKind { Data, Retransmit, Duplicate, FecParity }`. `FecParity` (`:134`)
= "POLİTİKA; transport üretmez (güvenlik katmanı encode eder). **Emit EDİLMEZ.**" (`sender.rs:133`).

### `crates/bench/src/kama.rs` — FecParity emit-durumu
`kama.rs:7-8`: "FEC parity ÜRETİLMEZ (`TransmitKind::FecParity` emit edilmez) → wire'da yalnız DATA + ACK →
INV-FEC-1 ihlali fiziksel olarak imkânsız." `kama.rs:32`: "FEC_SHARD 0x20 wire'da hiç görünmez."
`kama.rs:133`: `TransmitKind::FecParity => {}` (emit edilmez, §10 politika). → **Bench'te FEC yolu ÖLÜ.**

### `crates/adaptive-transport/src/receiver.rs` — RxKind::FecShard (İÇ-TUTARSIZLIK BULGUSU)
`receiver.rs:39-48` `RxKind::FecShard { block_id: u64, shard_index: u16, k: u16, r: u16 }`.
→ `fec.rs` (block_id **u32**, shard_index/k/r **u8**) ile genişlikler UYUŞMUYOR; ikisi de §2.6 (esi **u16**,
k_actual **u8**, block_id **u32**, r wire'da YOK) ile uyuşmuyor. Mevcut FEC tip-yüzeyi kendi-içinde tutarsız.

---

## (3) UYUMLU / UYUMSUZ — §2.6-EXTENDED karşısında byte-byte

**GENEL HÜKÜM: UYUMSUZ (yapısal).** Sayısal-tesadüf dışında (26B ≙ 26B) hiçbir alan/sıra/katman uymuyor.

| # | Boyut | Mevcut fec.rs (0x20) | §2.6-EXTENDED spec | Uyum |
|---|---|---|---|---|
| A | Demux | cleartext `frame_type=0x20` byte (`fec.rs:17,49`) | `h_ext` 4B routing-handle, hub decrypt-öncesi handle'dan seçer (§2.5, HANDLE_ASSIGN §2.55) | **UYUMSUZ** — frame_type byte YOK spec'te; h_ext yok kodda |
| B | Metadata konumu | block_id/shard_index/k/r/shard_len **PLAINTEXT** outer'da (`fec.rs:51-54`) | tümü **encrypted_inner** (ciphertext) içinde; outer SIZDIRMAZ (§2.6, Cryptex §2) | **UYUMSUZ** — METADATA-LEAK-01 |
| C | Outer alanları | h_ext/wire_format/wire_flags/pn_trunc **YOK** | 26B-outer = h_ext4+wf1+flags1+pn4+tag16 (§2.6) | **UYUMSUZ** — 4 outer-alan eksik |
| D | source/repair ayrımı | YOK (yalnız k/r sayaçları) | `inner_kind` 0x01 FEC_SOURCE / 0x02 FEC_REPAIR (§2.56) | **UYUMSUZ** — inner_kind yok |
| E | inner_version | YOK | `inner_version u8=0x01` (§2.56) | **UYUMSUZ** — yok |
| F | profile_id / flags | YOK | inner'da `flags u8 · profile_id u8` (§2.6) | **UYUMSUZ** — yok |
| G | ESI | `shard_index u8` (`fec.rs:32`) | `esi u16_be` inner'da (§2.6) | **UYUMSUZ** — ad+genişlik (u8 vs u16) |
| H | K taşıma | `k u8` (outer, plaintext) | `k_actual u8` (inner, encrypted; §2.6, PATCH-4) | **KISMEN** — u8 aynı ama YANLIŞ-KATMAN (plaintext) |
| I | R taşıma | `r u8` (per-shard, plaintext) | R wire'da YOK — profile/code-space'ten türer (§4 R-üçlü; §2.6 inner'da r-alanı yok) | **UYUMSUZ** — spec'te per-shard r-alanı yok |
| J | symbol_len | `shard_len u16` VAR (`fec.rs:38`) | **symbol_len ALANI YOK** — her shard TAM-E; kısa-source wire-encoding YASAK (§2.6 MADDE-6) | **UYUMSUZ** — fazladan alan |
| K | actual_payload_len | wire-header'da yok (short-source shard_len ile ima) | RS-kodlu source-symbol-PREFIX'inde (§2.6 canonical-prefix) | **UYUMSUZ** — konum farkı |
| L | block_id | u32_be (`fec.rs:30`) | u32_be (§2.6) | **genişlik-UYUMLU** ama YANLIŞ-KATMAN (plaintext vs inner) |
| M | tag | 16B, opaque_secured içinde (`fec.rs:24`) | 16B, outer-sonu (§2.6) | boyut-aynı; konumsal-fark (blob-içi vs outer-fixed) |

**INV-FEC-1 / SoC hizası (kısmen-doğru-niyet, yanlış-yerleşim):**
- **DOĞRU tarafı:** `opaque_secured` (ct‖tag) tek-opak-blob olarak taşınır, transport parse/verify ETMEZ
  (`fec.rs:39-41,82-85`); crate kripto-içermez (`fec.rs:5-7`). Bu "framing-plaintext, ct‖tag-opak" + verify-
  before-decode niyeti INV-ENGINE-SOC ile uyumlu.
- **YANLIŞ tarafı:** INV-FEC-1'in per-shard-AEAD'i FEC-semantik-metadata'yı **ciphertext içinde** authenticate
  etmeli (§7 per-shard-AEAD; §2 Cryptex). Mevcut fec.rs'te block_id/shard_index/k/r **AEAD dışında plaintext**
  → ne gizli ne authenticated (yalnız payload+tag opaque_secured'da). Cryptex-korumaları (§2: hangi-paket-source/
  repair, block_id, ESI, K/R-geometrisi gizli) SAĞLANMIYOR. § 2.8-AAD (`h_ext‖wf‖flags‖pn`) mevcut layout'ta
  üretilemez (bu alanların hiçbiri yok).

**Hub h_ext-demux opaque mı?** **HAYIR.** Mevcut yolda hub, on-path gözlemcinin de gördüğü cleartext
`0x20` + block_id/k/r'yi okur; §2.5'in "handle ile decrypt-öncesi demux + outer SIZDIRMAZ" gereği sağlanmıyor.
Hub-authenticated HANDLE_ASSIGN (§2.55) / h_ext-tahsisi kod-tabanında YOK.

**Gerçek outer-overhead == 26B mi?** Sayısal olarak **EVET-tesadüfen** (10B-header + 16B-tag = 26B),
ama **kompozisyon UYUMSUZ:** §2.6'da 26B = h_ext4+wf1+flags1+pn4+tag16 ve FEC-semantiği içermez; mevcut 26B =
10B-FEC-metadata-plaintext + 16B-tag. Aynı sayı, farklı byte'lar. "26B teyidi" yanıltıcı sayılmalı.

---

## (4) UYUMSUZ → KOD-DÜZELTME PLANI (spec DEĞİŞMEZ; D1 işi)

`fec.rs` FEC_SHARD(0x20) framing'i, mevcut haliyle **kaldırılıp §2.6-EXTENDED'e taşınır.** Adımlar:

1. **Discriminator: 0x20-frame_type → h_ext (§2.5).** `FRAME_TYPE_FEC_SHARD=0x20` kalkar; demux h_ext-handle'a
   döner. h_ext tahsisi HANDLE_ASSIGN (§2.55, hub-authenticated, hub↔client kanalı) ile — bu D1'de ayrı iş
   (schema-crate + noise-wire), fec.rs'in sorumluluğu değil.
2. **Outer 26B (noise-wire crate, fec-core-DIŞI):** `h_ext u32 · wire_format u8=0x01 · wire_flags u8(bit0=dh_epoch)
   · pn_trunc u32 · ct · tag16`. Bu KamaNoiseWireProtector.seal()'in işi (fec.rs değil); §2.8-AAD =
   `"KAMA-WIRE-EXT-V1"‖h_ext‖wf‖flags‖pn`. `sealed_len(EXTENDED)=26+encrypted_inner` (§11, §0.5 exact-sözleşme).
3. **Inner header (§2.56 + §2.6), ct İÇİNDE:** `inner_version=0x01 · inner_kind(0x01/0x02) · flags · profile_id
   · block_id u32 · esi u16 · k_actual u8 · symbol_bytes[E]`. Mevcut alan-eşlemesi:
   `block_id`→korunur (u32, inner'a taşınır) · `shard_index(u8)`→`esi(u16)` · `k(u8)`→`k_actual(u8)` (inner) ·
   `r(u8)`→**KALDIRILIR** (R profile/code-space'ten; per-shard-wire'da yok) · `shard_len(u16)`→**KALDIRILIR**
   (symbol TAM-E; kısa-source YASAK, MADDE-6) · source/repair→`inner_kind` eklenir · `profile_id`/`flags`/
   `inner_version` yeni-eklenir.
4. **actual_payload_len → canonical-source-symbol-PREFIX (§2.6):** `message_id u32·chunk_offset u32·
   actual_payload_len u16·source_flags u8 ‖ app_payload ‖ zero_pad[E−11−len]`; RS bunu kodlar (metadata-recovery,
   fec-core §6.5). shard_len wire-alanı bu prefix'e taşınmış olur.
5. **Katman-sınırı:** framing-plaintext-metadata-YOK; tüm FEC-semantiği ct içinde (Cryptex). fec-codec yalnız
   opaque-symbol üretir/tüketir; outer-seal + inner-encrypt noise-wire crate'inde (§1 gönderim-sırası:
   App→FEC→UnsealedTransportShard→KamaNoise-Seal→UDP). fec.rs'in mevcut "framing-codec" rolü, unsealed-inner-
   serialization'a evrilir; opaque_secured-blob mantığı kalkar (artık noise-wire seal eder).
6. **Tip-tutarlılık:** `receiver.rs:39-48` RxKind::FecShard genişlikleri (block_id u64, shard_index/k/r u16)
   §2.6'ya çekilir (esi u16, k_actual u8, block_id u32; r kalkar) — veya OpenedTransportShard/LogicalShardHeader
   (§12 schema) ile değiştirilir.
7. **outer-overhead-teyidi:** düzeltme sonrası `sealed_len==26+inner` unit-test'le assert (K-WIRE-SIZE-1);
   mevcut "26B tesadüf"ü yerine yapısal-26B.

**DOKUNULMAZLIK (fec-core §0.8-3):** bu düzeltme SESSION_DATA-22B-envelope'una (h_legacy), bw.rs/sender.rs
send_rate-loss/ZAMAN-MODELİ'ne DOKUNMAZ — FEC-shard AYRI-class, mühürlü-envelope yanında. TransmitKind::FecParity
emit-YOK durumu (§10 politika) D1-FEC-emisyonu açılana dek korunur.

---

## (5) D1'DE YAZILACAK EKSİKLER

- **E1 — EXTENDED-outer seal/open (noise-wire crate):** 26B-outer + §2.8-AAD + `sealed_len()` exact-sözleşme
  (K-WIRE-SIZE-1, K-WIRE-AAD-1). fec.rs'te YOK; KamaNoiseWireProtector henüz yok.
- **E2 — §2.56 UNIFIED-INNER-HEADER parser:** inner_version/inner_kind (0x01/0x02 FEC + 0x10..0x18 control);
  bilinmeyen→fail-closed. Mevcut kodda inner-header kavramı YOK.
- **E3 — §2.6 FEC-inner + canonical-source-symbol-PREFIX:** profile_id/esi-u16/k_actual/TAM-E symbol +
  RS-kodlu prefix (message_id/chunk_offset/actual_payload_len/source_flags). fec.rs'te YOK.
- **E4 — h_ext tahsis/demux (§2.55 HANDLE_ASSIGN):** hub-authenticated handle atama + decrypt-öncesi parser
  seçimi + opaque-outer. Kod-tabanında hiç YOK.
- **E5 — PN/replay/dh_epoch (§4):** pn32-truncated-reconstruction, per-(session,direction,dh_epoch) replay-window,
  epoch-0→1 geçiş. FEC-shard'ın full-PN/nonce'u (§3 dört-kimlik) — bu N4/N5/N8'in konusu, N3-frame'i buna bağlı.
- **E6 — RS-motor (fec-core):** fec.rs "RS-motor YOK" (fec-core §Zemin); encode/reconstruct + golden-vector
  + known-zero-injection (§6) fec-codec crate'inde yazılacak. opaque_secured→gerçek-symbol pipeline.
- **E7 — SealedPacketMeta / OpenedTransportShard (§0.5, §12):** PN→logical eşleme, sealed-type private-constructor.
  Mevcut FecShardFrame bunların yerini tutmaz.
- **E8 — Tip-tutarlılık temizliği:** receiver.rs RxKind::FecShard genişlik uyumsuzluğu (u64/u16 vs u32/u8)
  şema-crate tiplerine göre düzeltilir. sender.rs TransmitKind::FecParity gerçek-emisyona bağlanır (§8 controller).
- **E9 — golden-vector taşıma:** tv_fec.rs mevcut 29B-golden'ı (0x20…) §2.6-inner golden'a re-baseline
  (K-WIRE-WIRE-1/K-FEC-WIRE-1: reconstructed-metadata dahil, iki-impl aynı-byte).
