# N4 — GERÇEK-WIRE-OVERHEAD UYUM-RAPORU [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; kod YAZILMADI). noise-wire-D0-A N4 maddesi (§12).
> **Kapsam (§12-N4):** "gerçek-wire-overhead: outer_hdr + inner_hdr + AEAD_tag-byte
> (fec-core-PMTU-bütçesi §11-ortak-için)." Mevcut kodun paket-sınıfı başına ÜRETTİĞİ gerçek
> byte-overhead'ini envanterler ve §2.6 sabitlerine (22B / 26B / 11B) karşı ölçer.
> **Otorite:** `specs/kama-noise-wire-tdd-v2.0.md` §2.5/§2.6/§2.56/§2.8 + `specs/fec-core-tdd-v3.0.md` §11 ·
> LOCKED byte-facts: `crates/kama-transport-schema/src/lib.rs:37,41,45`.
> **HÜKÜM (özet):** §2.6-sealed sınıfların (EXTENDED-FEC / legacy-SESSION_DATA) **gerçek seal-eden kodu YOK**
> (crypto 3-satır iskelet, noise-wire crate yok). Mevcut byte üreten iki yol — `fec.rs` FEC_SHARD(0x20)
> ve `bench/kama.rs` DATA(0x00) — **UYUMSUZ**: `fec.rs` §2.6-EXTENDED'e byte-kompozisyonu uymuyor (26B
> tesadüfen eşit ama 11B inner-header YOK + plaintext-metadata) · bench DATA 27B (spec-legacy 22B'nin +5B
> üstünde, `unit_id`-8B wire'da yok, tag=zero-fill-simülasyon). Gerçek §2.6-FEC symbol-dışı-overhead =
> **26 (outer) + 11 (inner) = 37B**; mevcut kod bunu 26B'de sıkıştırıp 11B inner'ı ATLIYOR. Ayrıntı §3-4-5.

---

## (1) SPEC NE DİYOR (§2.5 · §2.6 · §2.56 · fec-core §11)

### §2.6 — FINAL-BYTE-LAYOUT [KİLİTLİ; tek-otorite: bu-tablo + schema-crate]
```
EXTENDED-OUTER (fixed 26B + encrypted_inner):
  h_ext u32_be(4) | wire_format u8=0x01(1) | wire_flags u8 bit0=dh_epoch(1) | pn_trunc u32_be(4) | ciphertext var | tag 16B
  → outer-overhead = 4+1+1+4+16 = 26B  (FEC-semantiği bu 26B'nin HİÇBİRİNDE yok — yalnız routing/PN/epoch/tag)

FEC-SHARD ENCRYPTED-INNER (§2.56 UNIFIED-HEADER + FEC variant_body — ciphertext'in İÇİNDE):
  inner_version u8=0x01(1) · inner_kind u8(1) · flags u8(1) · profile_id u8(1) · block_id u32_be(4) ·
  esi u16_be(2) · k_actual u8(1) · symbol_bytes[profile.E]
  → inner-header = 1+1+1+1+4+2+1 = 11B, sonra TAM-E symbol.  symbol_len ALANI YOK (MADDE-6: her shard TAM-E)

CANONICAL-SOURCE-SYMBOL-PREFIX (RS-kodlanır; symbol'ün İÇİNDE, boyut E'ye dahil):
  message_id u32_be(4) · chunk_offset u32_be(4) · actual_payload_len u16_be(2) · source_flags u8(1) = 11B
  · application_payload[actual_payload_len] · canonical_zero_pad[ E − 11 − len ]
```
- **LEGACY-SESSION_DATA (mühürlü, DEĞİŞMEZ — §0.9/§2.5):** `[h_legacy:4][seq16:2][ciphertext:N][tag:16]`
  → outer-overhead = hub_handle 4 + seq 2 + tag 16 = **22B** (schema `LEGACY_SESSION_DATA_OVERHEAD`).
- **CRYPTEX (§2/§2.6):** FEC-semantic-metadata (block_id/esi/k/source-repair/profil) **ciphertext-içinde**;
  outer yalnız-routing/PN/epoch + tag SIZDIRIR. AAD (§2.8) = `"KAMA-WIRE-EXT-V1" ‖ h_ext ‖ wire_format ‖
  wire_flags ‖ pn_trunc` (tag AAD'nin parçası DEĞİL).

### fec-core §11 — WIRE-BYTE/PMTU [İKİ-BELGE-ORTAK, PATCH-16]
```
sealed_len (EXTENDED)            = 26 + encrypted_inner_len        # h_ext4+wf1+flags1+pn4+tag16 (§2.6)
sealed_len (legacy-SESSION_DATA) = 22 + plaintext_payload_len      # AEAD-expansion = yalnız-tag varsayımı;
                                                                   # farklıysa EXACT-formül N7-raporunda güncellenir
WIRE-ADMISSION: sealed_len(packet) ≤ max_udp_payload_bytes
```
NORMATİF (§0.5-P0-3, K-WIRE-SIZE-1): `seal(p).len() == sealed_len(p)` — üst-sınır DEĞİL, **KESİN**.

### LOCKED byte-facts (schema-crate — zaten inşa edildi)
`kama-transport-schema/src/lib.rs`: `EXTENDED_OUTER_OVERHEAD = 26` (:41) · `LEGACY_SESSION_DATA_OVERHEAD = 22`
(:45) · `SOURCE_PREFIX_LEN = 11` (:37). Golden-assert: `tests/golden_v0.rs:185-189`. **symbol_bytes == profile.E;
symbol_len wire-alanı YOK** (`lib.rs:67`, `SchemaError::SymbolLenMismatch` :86-87).

---

## (2) MEVCUT KOD NE YAPIYOR (dosya:satır)

### 2.a §2.6-sealed katman (gerçek seal/AEAD/tag) — **YOK**
- `crates/crypto/src/lib.rs:1-3` — **3 satır** doc-comment; hiç seal/open/AEAD/tag/nonce kodu yok.
- `crates/ccr-codec/src/lib.rs:1-3` — **3 satır** iskelet; SESSION_DATA/EXTENDED wire-codec burada değil.
- **`kama-noise-wire` crate YOK** (`crates/` listesi: crypto/ccr-codec/adaptive-transport/… — noise-wire yok).
  → `sealed_len()` / `KamaNoiseWireSizer` / `SealedPacket` / gerçek 16B-AEAD-tag **hiçbir yerde tanımlı değil**
  (`grep sealed_len` yalnız schema doc-comment `lib.rs:40,43` + golden test yorumu; fonksiyon YOK).

### 2.b EXTENDED/FEC yolu — `crates/adaptive-transport/src/fec.rs` FEC_SHARD(0x20) (TAM okundu)
Serileştirilen gerçek wire-frame (`fec.rs:47-57` encode / `:67-99` decode):
```
offset  alan                          genişlik  kaynak
0       frame_type = 0x20             1B        fec.rs:17,49
1       block_id  u32_be             4B        fec.rs:30,50
5       shard_index u8               1B        fec.rs:32,51
6       k  u8                        1B        fec.rs:34,52
7       r  u8                        1B        fec.rs:36,53
8       shard_len u16_be             2B        fec.rs:38,54
10      opaque_secured = ct(shard_len) ‖ tag(16)   değişken   fec.rs:41,55; decode blob=shard_len+16 (:84-85)
```
- Cleartext framing-header = **10B** (`fec.rs:48` `10 + opaque_secured.len()`); + `SHARD_TAG_LEN=16` (`:24`,
  opaque_secured içinde) → **symbol-dışı overhead = 10 + 16 = 26B** (ct hariç).
- `opaque_secured` = ct ‖ tag TEK-OPAK-BLOB; transport parse/verify ETMEZ (`fec.rs:39-41,82-85`). Kripto yok (`:5-7`).
- **§2.6-inner-header (11B) HİÇ YOK:** inner_version/inner_kind/flags/profile_id/k_actual alanları yok
  (`fec.rs`'te inner-header kavramı yok). Yerine block_id/shard_index/k/r/shard_len **plaintext-outer'da**
  (METADATA-LEAK-01, `fec.rs:9-11`). `shard_len u16` (`:38`) §2.6-MADDE-6'yı ihlal eder (symbol TAM-E; symbol_len-YOK).
- Golden (`tests/tv_fec.rs`): `20 00000001 02 08 04 0003 AA BB CC 11×16` → 29B (10B-hdr + 3B-ct + 16B-tag).

### 2.c LEGACY/DATA yolu — `crates/bench/src/kama.rs` DATA(0x00) (bench-local sim)
- **Gerçek §2.6-legacy-SESSION_DATA codec YOK** (transport crate'inde de yok; `grep hub_handle` yalnız schema
  sabiti + doc). Mevcut tek DATA-framing = bench harness sim:
- `kama.rs:33-35`: `FRAME_DATA=0x00` · `HDR_DATA = frame_type(1) + seq u16(2) + unit_id u64(8) = 11B`.
- `kama.rs:53-61` `encode_data`: 11B-header + payload + `TAG_STUB=16` (`b.resize(...,0)` **zero-fill** — gerçek
  AEAD değil, `kama.rs:36-38,59` "AEAD-tag SİMÜLASYONU"). → **symbol-dışı overhead = 11 + 16 = 27B**.
- `kama.rs:4-6` modül-yorumu: "payload düz `0x5A`, seq/unit_id **cleartext**; anahtar/AEAD/Noise **YOK**."

### 2.d ACK/control yolu — `crates/adaptive-transport/src/ack.rs` (bitişik; N6/N8 asıl konusu)
- V2 (0x14): `frame_type 1 + ack_epoch 1 + largest_seq u16 2 + ack_delay u16 2 + block0_len u16 2 +
  range_count u8 1 + ranges[]{gap u16, len u16}(4×n) + seen_count u32 4 + arrival_rate u16 2` = **15B sabit + 4n**
  (`ack.rs:3-8`). **CLEARTEXT, AEAD-tag YOK**, u16-seq-uzayı (§2.57 ACK'in u64-PN-range + EXTENDED-AEAD'i DEĞİL).
  → N4-PMTU-bütçesinde bu sınıf **0B tag** taşır; §2.6-sealed-class değil. (Uyumsuzluğun tam envanteri N6.)

### 2.e Bench'te FEC yolu ölü
- `kama.rs:7-8,32,133`: `TransmitKind::FecParity` emit EDİLMEZ → wire'da yalnız DATA + ACK; FEC_SHARD(0x20)
  wire'da hiç görünmez. `fec.rs` codec'i unit-test dışında **çalışmıyor** (ölü yol).

---

## (3) HÜKÜM: UYUMLU / UYUMSUZ / YOK — paket-sınıfı başına

| Paket-sınıfı | Spec (§2.6) symbol-dışı-overhead | Kod gerçek-overhead | Hüküm |
|---|---|---|---|
| **EXTENDED-FEC (source/repair)** | outer 26B + inner 11B = **37B** (+ E-symbol; source'ta 11B prefix E-içinde) | `fec.rs`: 10B-plaintext-hdr + 16B-tag = **26B**; inner-11B YOK | **UYUMSUZ** — 11B inner-header eksik, plaintext-metadata (N3) |
| **Legacy SESSION_DATA** | hub_handle 4 + seq 2 + tag 16 = **22B** | gerçek codec **YOK**; bench sim: 11B-hdr (frame1+seq2+unit_id8) + 16B-tag-sim = **27B** | **YOK (gerçek) / UYUMSUZ (bench-sim)** — +5B, `unit_id`-8B wire'da yok, tag zero-fill |
| **AEAD-tag (16B)** | her §2.6-sealed-paket +16B gerçek Noise-tag | `fec.rs:24` SHARD_TAG_LEN=16 (wire-size-placeholder) · `kama.rs:38` TAG_STUB=16 (zero-fill sim); gerçek AEAD YOK | **YOK** — gerçek tag D1 (crypto iskelet) |
| **Control (§2.57 sealed)** | EXTENDED 26B + inner (6B ortak + body) | `ack.rs` cleartext 15B+4n, tag YOK, u16-seq | **UYUMSUZ** — sealed-değil (N6) |

**NET HÜKÜM: UYUMSUZ (üreten-iki-yol) / YOK (gerçek-sealed-katman).**

1. **Gerçek §2.6-seal-eden kod YOK.** noise-wire crate yok, crypto 3-satır iskelet; `sealed_len()`/gerçek
   16B-tag hiçbir yerde. Byte üreten her şey (fec.rs framing, bench DATA/ACK) ya ölü-codec ya security-stub.
2. **`fec.rs` 26B "teyidi" YANILTICI (N3 ile tam-hizalı).** Sayısal 26B == `EXTENDED_OUTER_OVERHEAD` ama
   **kompozisyon UYUMSUZ**: §2.6-26B = h_ext4+wf1+flags1+pn4+tag16 (FEC-semantiği İÇERMEZ); mevcut 26B =
   10B-FEC-metadata-**plaintext** + 16B-tag. Aynı sayı, **farklı byte'lar**. Dahası mevcut yol §2.6'nın **11B
   encrypted-inner-header'ını tümüyle ATLIYOR** → gerçek §2.6-FEC symbol-dışı-overhead **37B**, mevcut kod **26B**
   üretir (11B eksik + `shard_len u16` fazlalık, §2.6-MADDE-6-ihlali).
3. **Legacy-yol: gerçek codec YOK; bench-sim 22B DEĞİL.** Bench DATA symbol-dışı-overhead = **27B** (spec-legacy
   22B'nin +5B üstünde): `unit_id u64`(8B) §2.6-legacy'de (hub_handle4+seq2+tag16) YOK — ölçüm-join-alanı;
   `frame_type`(1B) da §2.6-legacy'de yok; tag zero-fill-simülasyon. Gerçek mühürlü-22B-zarf L4-Noise-katmanında
   (§0.9-DEĞİŞMEZ), bu crate'lerde değil.

---

## (4) KOD-DÜZELTME-PLANI (D1) — spec DEĞİŞMEZ (KURAL-2: kod düzeltilir)

- **E1 — EXTENDED-outer seal (noise-wire crate, YENİ):** `[h_ext u32 · wire_format u8=0x01 · wire_flags u8
  (bit0=dh_epoch) · pn_trunc u32 · ct · tag16]` = 26B; §2.8-AAD üret; `sealed_len(EXTENDED)=26+encrypted_inner`
  KESİN-assert (K-WIRE-SIZE-1). fec.rs'in `opaque_secured`-blob mantığı KALKAR (seal noise-wire'da).
- **E2 — §2.56 UNIFIED-INNER-HEADER + §2.6 FEC-inner (11B):** `inner_version=0x01 · inner_kind(0x01/0x02) ·
  flags · profile_id · block_id u32 · esi u16 · k_actual u8` ct-İÇİNDE. Mevcut fec.rs alan-eşlemesi:
  `block_id`→inner(u32-korunur) · `shard_index u8`→`esi u16` · `k u8`→`k_actual u8`(inner) · `r u8`→**KALDIR**
  (R profile/code-space'ten; §2.6-inner'da r-alanı yok) · `shard_len u16`→**KALDIR** (symbol TAM-E; MADDE-6).
- **E3 — source-symbol-prefix (11B, E-içinde):** `message_id u32 · chunk_offset u32 · actual_payload_len u16 ·
  source_flags u8 ‖ payload ‖ zero_pad[E−11−len]` — schema `prefix::SourcePrefix::serialize_symbol` ZATEN VAR
  (`kama-transport-schema/src/prefix.rs`); noise-wire buna bağlanır. `shard_len` wire-alanı bu prefix'e taşınmış olur.
- **E4 — frame_type(0x20)→h_ext-demux (§2.5/§2.55):** cleartext-discriminator KALKAR; hub decrypt-öncesi
  HANDLE_ASSIGN-handle'dan parser seçer (opaque-outer). fec.rs'in 0x20-byte'ı silinir.
- **E5 — gerçek 16B-AEAD-tag:** SHARD_TAG_LEN/TAG_STUB placeholder → per-shard-AEAD (§7) gerçek tag;
  outer-tag konumsal-trailer (26B'nin son 16B'si). crypto/noise-wire katmanı yazılır.
- **E6 — legacy-SESSION_DATA-codec (mühürlü, dokunma-kısıtlı):** 22B-zarf L4-Noise'da (§0.9). Bench DATA-sim
  ölçüm-harness'ı olarak kalabilir AMA "22B-legacy" iddiası taşımaz; `unit_id`-8B ölçüm-artefaktı olarak etiketli.

---

## (5) D1-EKSİKLERİ

- **[D1-1] Gerçek seal/open + `sealed_len()` (KRİTİK):** noise-wire crate + `KamaNoiseWireSizer` yok; §0.5-P0-3
  `seal(p).len()==sealed_len(p)` KESİN-sözleşme hiçbir yerde. (N7 sözleşme-taslağını taşır.)
- **[D1-2] §2.6-11B-inner-header:** inner_version/inner_kind/flags/profile_id/esi-u16/k_actual — fec.rs'te YOK;
  gerçek §2.6-FEC symbol-dışı-overhead 37B (26+11) burada doğar. schema `LogicalShardHeader` (`lib.rs:55-61`)
  logical-formu var ama wire-serializer noise-wire'da yazılacak.
- **[D1-3] Gerçek 16B-AEAD-tag:** crypto 3-satır; şu an fec.rs=wire-size-placeholder, bench=zero-fill-sim.
  Gerçek AEAD-expansion (tag-only mu, yoksa nonce-on-wire/length-expansion mı) crypto-katmanı gelince EXACT-teyit.
- **[D1-4] `shard_len u16` kaldırımı + symbol==E kilidi:** fec.rs `shard_len` (`:38`) §2.6-MADDE-6-ihlali;
  schema `SymbolLenMismatch` (`lib.rs:86`) enforce eder ama fec.rs wire-yolu bunu kullanmıyor.
- **[D1-5] Cleartext-metadata kapatma (Cryptex):** block_id/shard_index/k/r plaintext-outer'da (METADATA-LEAK-01)
  → ct-içine taşınır; §2.8-AAD (`h_ext‖wf‖flags‖pn`) mevcut layout'ta üretilemez (alanların hiçbiri yok).
- **[D1-6] Tip-tutarlılık:** `receiver.rs` `RxKind::FecShard { block_id u64, shard_index/k/r u16 }` (N3-bulgusu)
  §2.6-genişlikleriyle (block_id u32, esi u16, k_actual u8; r YOK) uyuşmuyor → schema-tiplerine çekilir.

**Cross-ref → N7:** bu rapordaki symbol-dışı-overhead sayıları (EXTENDED 26 + inner 11 = 37; legacy 22; tag 16;
source-prefix 11) N7-PMTU-bütçe-tablosunun girdi-sabitleridir. N7 bunları E=64-katalog + IPv4/IPv6/tunnel-MTU
ile birleştirip `sealed_len` per-class exact-byte + max-E + sub-kbps tablosunu üretir.
