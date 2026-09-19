# CCR Format Specification

> **YAMA KAYDI — 2026-09-18b:** başlık notuna CCR canonical-codec-only kuralı (protobuf re-encode ve generic compressor yasak; opak bytes olarak taşınır).


> **YAMA KAYDI — 2026-09-18 [AMENDMENT-6]:** kanun bandı → transport-tdd §0 referansı · §35.3 early_data istisnası (Crypto opak işler) · §35.6 BatchPayload sahiplik notu (birimi Transport üretir; kodda YOK, interim 11B çerçeve; canonical S3 [OPEN]) · §35.6 sıkıştırma pointer'ı → app-rpc §3 · §35.7 HubHandle ≠ SessionHandle · statü matrisi S3 diliyle. P0 kripto yamaları DEĞİŞMEDİ (yerleşimden bağımsız).


> **Katman grafiği:** transport-layer-tdd §0 INV-NEIGHBOR-1 (App → Transport → Crypto → UDP; Crypto'nun tek üst komşusu Transport). Bu belge E2E byte otoritesidir; yerleşimden bağımsızdır. **CCR nesneleri (cert, attestation, TBS) YALNIZ bu belgenin canonical codec'iyle serileştirilir**; protobuf/generic serializer ile yeniden kodlanmaz, generic compressor'dan geçmez, başka mesajda opak `bytes` olarak taşınır (transport-tdd §0 INV-PLAINTEXT-NOT-SEMANTICS).

> **YAMA KAYDI — 2026-09-07b:** §35.12 parser-reject ve §30.x negative-corpus satırlarındaki "RESPONSE sonrası epoch_0 → reject" ifadeleri grace'e tabi hale getirildi (P0-9; erken-reject testi yazılmaz) · statü matrisine entegre protector HOLD gerekçesi (S2-noise.2 + CRY-1 + noise-wire D1-ENTRY) ve KS-GRACE-1 notu eklendi.


> **YAMA KAYDI — 2026-09-07 (CRYPTO-P0-FREEZE):** §35.5 epoch-0 reject wording'i confirmation-safe makineye referansla düzeltildi (P0-9/§12) · §35.13'e zorunlu test seti (§19) · doküman sonuna CRYPTO-P0-FREEZE statü matrisi (§15). Wire format / key schedule / primitive DEĞİŞMEDİ.


> **YAMA KAYDI — 2026-09-02 (S2 blocker kapatma, B1+B2):**
> - **YENİ §35.2.1** — HKDF instantiation exact (HMAC-BLAKE2s-256 pini; kanonik `KDF(label,secret,context,L)`
>   sarmalayıcısı = Extract(salt=label, IKM=secret) + Expand(PRK, info=context, L); D1–D7 türetme tablosu;
>   INV-KDF-1/2/3, INV-INIT-KEY-1, INV-RESP-KEY-1; V-KDF-1..7 vektörleri).
>   **D5 (epoch-1 traffic keys) daha önce hiçbir yerde tanımlı DEĞİLDİ — eklendi.**
> - **YENİ §35.3.1** — SESSION_INIT AAD + nonce exact (`KARAR-2 = A`, `KARAR-3 = Evet` uygulandı).
> - **YENİ §35.5.1** — SESSION_INIT_RESPONSE AAD + nonce exact.
> - **§35.9'a not** — SSD1 ile EXTENDED AAD arasındaki asimetrinin kasıtlı olduğu; `epoch` = u16_be(dh_epoch).
> - Uygulanan karar: **KARAR-1 = A** (INIT/RESPONSE ayrı domain-separated anahtar `k_init`/`k_resp`, nonce96(0)).
>   Gerekçe §35.2.1 D6/D7 notunda. **Geri alınabilir:** B seçeneğine dönüş = D6/D7 satırlarını kaldır +
>   §4 EPOCH-PN-INIT'e "epoch-0 sayacı 1'den başlar" kuralı ekle.
> - Mevcut hiçbir normatif satır silinmedi/değiştirilmedi; yalnız ekleme yapıldı.
>
> **YAMA KAYDI — 2026-09-07 (Hasan raporu / B4-EXT mekanik kalemler):**
> - **§30.4b** SIGNED_CONTROL `signed_control_context` kısa TBS'i → §35.10 otoriter TBS'ine çekildi (A-4/B-1).
>   ⚠ Bu bölümün SIGNED_CONTROL vektörleri yeniden üretilmelidir.
> - **§35.6** seq16 satırına AEAD nonce mapping eklendi: `nonce96 = 0x00×4 || LE64(full_counter)` (C-1).
> - **§35.5** reject kurallarına RESPONSE flags simetrisi (C-4) ve ephemeral-tazelik kuralı (C-3) eklendi.
> - **§35.8** reconstruction'a beraberlik kuralı (C-9); **§30.4b** boş early_data / init_counter=0 geçerliliği (C-10).


**Doküman:** `ccr-format-spec.md`
**Versiyon:** v1.0
**Durum:** Implementation-ready binary format spec — freeze-ready (DiscoveryEndpointRecord transport-key reconciliation sonrası)
**Normatif kaynak:** `identity-and-trust-model.md` v1.5 (semantik otorite)
**İlgili dokümanlar:** `discovery-architecture.md`, `rendezvous-dht-architecture.md`, `transport-layer-tdd.md`, `revocation-model.md`, `ra-ca-deployment-tdd.md`

---

## 1. Purpose / Amaç

Bu doküman Kama'nın kimlik/PKI nesnelerinin **kesin canonical binary formatını** tanımlar. Hedef: bir Java RA/CA implementasyonu, bir Rust parser/validator ve bir C++/Rust client/hub implementasyonunun **birebir aynı byte'ları üretmesi ve aynı imzaları doğrulaması**.

Bu doküman şunları tanımlar:
- CCR sertifikalarının binary layout'u
- Canonical CCR TBS encoding (imza girdisi)
- ~~SignedNoisePrekey, PrekeyBundle~~ (DEPRECATED, statik X25519), DeviceCertBundle binary encoding, Session Model V3 (Section 35)
- DiscoveryRecord encoding sınırı
- Kesin hash/signature girdileri
- Parser rejection kuralları
- Canonicalization test vector'leri
- Java/Rust/C++ interop gereksinimleri

`identity-and-trust-model.md` v1.5 **trust/PKI/revocation/endpoint-validation semantiği** için otoriterdir; bu doküman onun tanımladığı kimlik/PKI yapılarının byte-level temsilini verir (çeliştiğinde kimlik/PKI/trust → v1.5). **İSTİSNA (P0-3):** Session Model V3 (Section 35) **wire/session canonical byte ve hot-path message semantiği** için BU doküman otoriterdir (identity-model bu konuda ccr §35'e reconcile edilir).

> **v1.5 ile bilinçli byte-level fark (NOT):** v1.5'te canonical TBS yapıları `magic` alanı içermez. Bu doküman, tüm canonical nesnelere (CCR/DiscoveryRecord/Session Model V3) format-disambiguation ve domain-separation amaçlı 4-byte `magic` ekler ve bunu TBS'e dahil eder. Bu, v1.5'in **semantik** kararlarını değiştirmez (aynı alanlar, aynı trust path, aynı imzalayan key); yalnızca imza girdisinin byte temsilini netleştirir. Tüm implementasyonlar bu dokümandaki TBS tanımını kullanır. NOT: SignedNoisePrekey/canonical_prekey_tbs/SRPK statik X25519 kararıyla KALDIRILDI (Section 18 DEPRECATED).
>
> **Magic-in-TBS kuralı — tüm canonical imzalı nesneler için (NORMATİF):** Bu format spec'te tanımlı tüm canonical imzalı nesneler, domain separation için 4-byte magic değerlerini imzalı TBS'e **dahil eder**:
> ```
> canonical_ccr_tbs               magic "SRCR" içerir
> canonical_discovery_record_tbs  (dr_tbs)  magic "SRDR" içerir
> canonical_async_message_tbs     magic "SRAM" içerir  [DEPRECATED — Section 35-DEPRECATED]
> ── Session Model V3 (Section 35, CAD-54) ──
> SESSION_INIT                    magic "SSI1" içerir (AAD'de)
> SESSION_INIT_RESPONSE           magic "SSR1" içerir (AAD'de)
> SESSION_DATA                    magic "SSD1" içerir (AAD'de; wire'da YOK)
> SIGNED_CONTROL                  magic "SSC1" içerir (signed_control_context'te)
> [DEPRECATED: ASYNC_INIT/ACCEPT/DATA "SAI1"/"SAA1"/"SAD1"/"SAC1" — Section 35-DEPRECATED-V2, MUST REJECT]
> ── Routing (routing-rendezvous-architecture.md) ──
> RoutingRecord                   magic "SRRR" içerir
> ANNOUNCE                        magic "SRAN" içerir
> AuthorizedHubSetRecord          magic "SRAH" içerir
> DiscoveryEndpointRecord        magic "SRDE" içerir
> [canonical_prekey_tbs/"SRPK" KALDIRILDI — statik X25519, Section 18 DEPRECATED]
> canonical_discovery_record_tbs  (dr_tbs)  magic "SRDR" içerir
> ```
> Bu byte-level canonicalization kuralıdır ve identity-and-trust-model.md v1.5'in semantik trust modelini değiştirmez. **Tüm gelecek TDD'ler bu kuralı tutarlı şekilde referans almalı; implementer'lar DiscoveryRecord TBS'inden (veya herhangi bir TBS'ten) magic'i çıkarmamalı** — aksi halde imza mismatch oluşur.

---

## 2. Scope Boundaries

### 2.1 Bu Doküman Sahibi (owns)

- CCR sertifikalarının binary canonical formatı (Root/Intermediate/Identity/Device/Hub/RA/CT)
- İmza için canonical TBS encoding (magic-prefixed domain separation)
- CompactDeviceCert (~152B) byte layout
- DeviceCertBundle + Session Model V3 (SESSION_INIT/SESSION_INIT_RESPONSE/SESSION_DATA/SIGNED_CONTROL; Section 35)
- DiscoveryRecord + HubListRecord + OrgEntitlementRecord byte layout (Section 21/21.5/21.6)
- Kesin hash/signature girdileri
- Parser rejection kuralları
- Canonicalization test vector'leri
- Interop gereksinimleri
- [SignedNoisePrekey/PrekeyBundle DEPRECATED / MUST NOT IMPLEMENT, Section 18-19]

### 2.2 Bu Doküman Sahibi DEĞİL

| Konu | Sahip doküman |
|------|---------------|
| VOPRF protokol mekaniği | `discovery-architecture.md` |
| Custom ARQ/FEC/Noise transport mekaniği | `transport-layer-tdd.md` |
| CRLite filter binary format | `revocation-model.md` |
| RA deployment topolojisi | `ra-ca-deployment-tdd.md` |
| DHT storage/routing mekaniği | `rendezvous-dht-architecture.md` |
| Business authorization policy | Ayrı policy dokümanı |

### 2.3 Transport Boundary Notu (Confirmed, v1.5)

```
RA Server → Intermediate CA Server: izole mTLS (opsiyonel gRPC over mTLS)
Kama custom adaptive transport RA→CA'da KULLANILMAZ. (KCP çıkarıldı; hiçbir aktif CCR nesnesi KCP'ye bağlı değil.)
Kama custom transport edge/public yollar için:
  Client→RA, Hub→RA, Client→Hub, Hub→Hub, Client↔Client (hub relay).
```

Bu doküman canonical byte'ları tanımlar; bu byte'lar gRPC/mTLS ile sarmalanabilir (RA→CA) veya Kama adaptive transport / Noise-based transport envelope ile taşınabilir (edge), ama **sarmalayıcı ne olursa olsun içerideki imzalı byte'lar bu dokümandaki canonical formattır.**

---

## 3. Design Constraints

| Kısıt | Karar |
|-------|-------|
| Byte order | Unsigned fixed-width **big-endian** (network byte order) |
| Varint | **Yasak** (LEB128/bit-level variable-width; canonical imzalı yapılarda). Fixed-width length-prefix u16_be/u32_be varint DEĞİLDİR — Section 4.4 |
| Platform-endian | **Yasak** |
| Canonical signing input | Kama-native binary; JSON/protobuf/ASN.1-DER **değil** |
| Field order | Sabit, deterministik |
| Optional reordering | Yasak |
| Duplicate fields | Yasak |
| Unknown fields (signed TBS içinde) | Yasak (sıkı tanımlı extension block hariç) |
| Hash | SHA-256 |
| Signature | Ed25519 |
| String | Protokol sabitleri ASCII |

`Confirmed:` CCR Kama-native'dir, X.509 TBS projection **değildir**. Wire doğrulama canonical CCR TBS byte'ları üzerinde yapılır.

---

## 4. Canonical Encoding Rules

```
All integers are unsigned fixed-width big-endian.
All field order is fixed.
No varint.
No platform-endian.
No implicit defaults in signed TBS.
No duplicate fields.
No unknown fields inside signed TBS.
No unordered maps.
No JSON as canonical signing input.
No protobuf encoding as canonical signing input.
No ASN.1 DER as canonical signing input.
CCR is Kama-native, not X.509 TBS projection.
```

### 4.1 String Encoding

- Protokol sabitleri ve `usage_context`: **ASCII only** (her byte 0x00–0x7F).
- Display alanları (gerekirse): UTF-8, yalnızca açıkça işaretliyse.
- İmzalı alanlarda Unicode normalizasyonu yok (açıkça tanımlanmadıkça).
- Domain separator'lar: tam ASCII byte string.

### 4.2 Boolean Encoding

```
false = 0x00
true  = 0x01
(başka değer → parser reject)
```

### 4.3 Array Encoding

```
u16_be count || item[0] || item[1] || ...
```
Her array alanı için sıkı `max count` tanımlanır (Section 6).

### 4.4 Variable-Length Field Encoding

> **Terminoloji netleştirmesi (NORMATİF — interop kritik):** Section 3'teki "Varint: Yasak" kuralı **LEB128/bit-level variable-width integer encoding'i** (Protobuf-tarzı, 7-bit grup + continuation bit) yasaklar. **Fixed-width length-prefix (`u16_be` / `u32_be`) bir varint DEĞİLDİR ve izinlidir.** `varbytes16`/`varbytes32` length alanı her zaman sabit-genişlikli big-endian'dır (2 veya 4 byte), continuation-bit içermez. Java ve Rust implementasyonları length-prefix'i her zaman sabit u16_be/u32_be olarak okur/yazar — asla variable-width değil. Bu ayrım imza determinizmi için zorunludur (yanlış yorumlanırsa byte-mismatch → imza fail).

```
varbytes16 = u16_be length || bytes        # length: SABİT 2 byte big-endian (varint DEĞİL)
varbytes32 = u32_be length || bytes        # length: SABİT 4 byte big-endian (varint DEĞİL)
```
Her variable-length alan tanımlar: max length, empty izinli mi, TBS'e dahil mi, canonical encoding, parser reject kuralı.

---

## 5. Primitive Types

| Tip | Boyut | Açıklama |
|-----|-------|----------|
| `u8` | 1 byte | unsigned |
| `u16_be` | 2 byte | unsigned big-endian |
| `u32_be` | 4 byte | unsigned big-endian |
| `u64_be` | 8 byte | unsigned big-endian |
| `bytes4` | 4 byte | sabit (magic) |
| `bytes16` | 16 byte | sabit (device_id, routing_key) |
| `bytes32` | 32 byte | sabit (hash, SPKI, Ed25519/X25519 pub) |
| `bytes64` | 64 byte | sabit (Ed25519 signature) |
| `bytes152` | 152 byte | sabit (CompactDeviceCertV1; Section 13.1) |
| `varbytes16` | 2+N byte | `u16_be len \|\| bytes`, max 65535 |
| `varbytes32` | 4+N byte | `u32_be len \|\| bytes`, büyük payload |
| `bool_u8` | 1 byte | 0x00 / 0x01 |
| `unix_time_u64` | 8 byte | `u64_be` saniye (Unix epoch) |
| `ascii_fixed[N]` | N byte | sabit-uzunluk ASCII (padding yok; tam eşleşme) |

### 5.1 X25519 Public Key Handling (NORMATİF — tüm X25519 kullanımları)

```
- all-zero 32-byte public key → parse/validation'da reject.
- Kütüphane known low-order public key'leri tespit edebiliyorsa → reject.
- HER X25519 DH operasyonu shared_secret != all_zero KONTROL ETMELİ.
- shared_secret all_zero ise → mesaj/oturum reddedilir.
```
Bu özellikle Session Model V3 DH'leri için kritik (CompactDeviceCert.device_x25519_pub, sender_ephemeral_x25519, bob_ephemeral, Hub Noise key). All-zero/low-order nokta contributory-behavior saldırılarını ve sıfır-anahtar DH'i engeller.

---

## 6. Common Constants

### 6.1 Magic Values (bytes4, ASCII)

| Nesne | Magic | Hex |
|-------|-------|-----|
| CCR envelope | `"SRCR"` | `53 52 43 52` |
| DiscoveryRecord | `"SRDR"` | `53 52 44 52` |
| HubListRecord | `"SRHL"` | `53 52 48 4C` |
| **OrgEntitlementRecord** | `"SROE"` | `53 52 4F 45` |
| DeviceCertBundle | `"SRDB"` | `53 52 44 42` |
| **SESSION_INIT** (Section 35) | `"SSI1"` | `53 53 49 31` |
| **SESSION_INIT_RESPONSE** (Section 35.5) | `"SSR1"` | `53 53 52 31` |
| **SESSION_DATA** (Section 35.6, AAD-only) | `"SSD1"` | `53 53 44 31` |
| **SIGNED_CONTROL** (Section 35.10) | `"SSC1"` | `53 53 43 31` |
| ~~ASYNC_INIT/ACCEPT/DATA/CONTROL~~ (DEPRECATED V2) | ~~`SAI1/SAA1/SAD1/SAC1`~~ | Section 35-DEPRECATED-V2, MUST REJECT |
| **RoutingRecord** (routing-rendezvous) | `"SRRR"` | `53 52 52 52` |
| **ANNOUNCE** (routing-rendezvous) | `"SRAN"` | `53 52 41 4E` |
| **AuthorizedHubSetRecord** (routing-rendezvous) | `"SRAH"` | `53 52 41 48` |
| **DiscoveryEndpointRecord** (Section 21.7) | `"SRDE"` | `53 52 44 45` |
| **BootstrapBundle** (Section 21.8, KAT5) | `"SRBB"` | `53 52 42 42` |
| ~~AsyncMessage~~ (DEPRECATED) | ~~`"SRAM"`~~ | KALDIRILDI (Session Model V3, Section 35-DEPRECATED) |
| ~~SignedNoisePrekey~~ (DEPRECATED) | ~~`"SRPK"`~~ | KALDIRILDI (statik X25519, Section 18) |
| ~~PrekeyBundle~~ (DEPRECATED) | ~~`"SRPB"`~~ | KALDIRILDI (statik X25519, Section 19) |

> **Domain separation (NORMATİF):** Her imzalı yapının TBS'i kendi magic'iyle başlar (SRCR/SRDR/SRHL/SROE + Session Model V3: SSI1/SSR1/SSD1/SSC1 + routing: SRRR/SRAN/SRAH; SRAM + ASYNC_* SA?1 DEPRECATED) → bir yapının imzası başka yapı olarak yorumlanamaz (cross-protocol signature confusion önlenir). DeviceCertBundle unsigned container (magic SRDB envelope ama imza yok — içindeki CCR'lar kendi imzalarını taşır).

### 6.2 cert_type (u8)

```
0x01 Root CA
0x02 Intermediate CA
0x03 Identity CA / User Identity
0x04 Device
0x05 Hub Identity
0x06 Hub Noise
0x07 RA Identity
0x08 RA Noise
0x09 CT Log Signing
0x0A License Authority
0x0B Admin Server
```

### 6.3 public_key_alg / sig_alg (u8)

```
0x01 Ed25519
0x02 X25519
```
(X25519 yalnızca `public_key_alg` olarak; X25519 `sig_alg` olamaz — imza algoritması her zaman Ed25519.)

### 6.4 key_usage bitmask (u16_be)

```
bit 0 (0x0001) digitalSignature
bit 1 (0x0002) keyCertSign
bit 2 (0x0004) cRLSign
bit 3 (0x0008) keyAgreement
bit 4 (0x0010) nonRepudiation
bits 5-15      reserved (0 olmalı; aksi → reject)
```

### 6.5 EKU constants (u16_be, eku_list içinde)

```
0x0001 id-kama-intermediate
0x0002 id-kama-identity
0x0003 id-kama-device
0x0004 id-kama-hub
0x0005 id-kama-ra
0x0006 id-kama-ct-log
0x0007 id-kama-license-authority
0x0008 id-kama-admin-server
(Root CA: EKU yok — eku_count=0)
```

### 6.6 Extension IDs (u16_be)

```
0x0001 nameConstraints
0x0002 subjectAltName (SAN)
0x0003 deviceId
0x0004 ctLogMetadata
0x0005 kamaNamespace
0x0100-0x7FFF reserved (standard)
0x8000-0xFFFF reserved (operator/experimental)
```

### 6.7 Max Sizes (parser limitleri)

| Alan | Max |
|------|-----|
| eku_count | 8 |
| extension_count | 16 |
| extension value | 4096 byte |
| name (varbytes16) | 256 byte |
| nameConstraints entry count | 8 |
| SAN entry count | 8 |
| ra_attestation (varbytes16) | 512 byte |
| DISCOVERY_ENDPOINT_COUNT (Section 21.7) | 8 |
| DISCOVERY_ENDPOINT_LEN (Section 21.7) | 512 byte |
| CCR total | 8192 byte |
| DeviceCertBundle total | 16384 byte |
| usage_context | sabit 25 byte ("SR-client-noise-static-v1") |

### 6.8 Validity Bounds (cert_type başına)

| cert_type / nesne | max validity |
|-----------|--------------|
| Root CA | 20 yıl (offline, pinned) |
| RoftCore License Authority | 1-3 yıl (önerilen 2 yıl; Root-signed, online HSM sign-only; cert_type 0x0A) |
| Admin Server | 1-3 yıl (Alpha önerisi 1 yıl; per-org Intermediate-signed; cert_type 0x0B) |
| Intermediate CA | **5 yıl** (namespace omurgası; lisans mekanizması DEĞİL — identity v1.5 CAD-52) |
| Identity CA / User Identity | **1 yıl** (RA license gate + Admin approval ile yenilenir — identity v1.5 E-11) |
| Device CCR (CompactDeviceCert) | **90 gün**; statik pk_x25519 içerir (prekey kaldırıldı); rotation YOK |
| Hub Identity / RA Identity | 3 yıl (Intermediate altyapısı, lisanstan bağımsız) |
| Hub Noise / RA Noise | 1 yıl |
| OrgEntitlementRecord | lisans dönemi (genelde 1 yıl); `DeviceCert/IdentityCert/HubListRecord.not_after <= license_not_after` |

> **Lisans clamp (identity v1.5 §7.14, E-11):** Identity/Device/HubList validity, OrgEntitlementRecord.license_not_after'ı aşamaz. Intermediate (5 yıl) bu clamp'e tabi DEĞİL (namespace omurgası, lisanstan bağımsız).

---

## 7. CCR Object Model (Envelope)

```
CCR {
  magic             : bytes4      # "SRCR"
  version           : u8          # 1
  cert_type         : u8          # Section 6.2
  tbs_len           : u32_be      # canonical_ccr_tbs byte uzunluğu
  tbs               : CCR_TBS     # tam tbs_len byte (Section 8)
  sig_alg           : u8          # 0x01 Ed25519
  signature         : bytes64     # Ed25519_Sign(issuer_sk, canonical_ccr_tbs)
}
```

**Kurallar:**
- `signature = Ed25519_Sign(issuer_private_key, canonical_ccr_tbs_bytes)`.
- İmzalanan byte'lar **tam olarak** `tbs` alanının canonical byte'larıdır (envelope'un `magic/version/cert_type/tbs_len/sig_alg/signature` alanları imzaya dahil **değildir**).
- `tbs_len`, gerçek TBS uzunluğuna **eşit olmalı**.
- Parser: trailing byte reddeder.
- Parser: length mismatch reddeder.
- Parser: desteklenmeyen version/cert_type/sig_alg reddeder.

> **İmza kapsamı netleştirmesi:** Envelope `magic`, `version`, `cert_type`, `sig_alg` alanları imzanın dışındadır AMA canonical_ccr_tbs **kendi içinde** `version` ve `cert_type` alanlarını tekrar taşır (Section 8). Böylece bu alanlar imza tarafından korunur; envelope kopyaları yalnızca framing içindir ve TBS'teki değerlerle eşleşmek zorundadır (uyuşmazlık → reject).

---

## 8. CCR TBS Encoding (canonical_ccr_tbs)

```
CCR_TBS {
  magic               : bytes4      # "SRCR" (TBS'e dahil — domain separation)
  version             : u8          # 1
  cert_type           : u8
  serial              : bytes16     # Section 27
  issuer_key_id       : bytes32     # SHA-256(issuer_spki)
  subject_key_id      : bytes32     # SHA-256(subject_spki)
  issuer_name         : Name        # varbytes16 (Section 24.1)
  subject_name        : Name        # varbytes16
  not_before          : u64_be      # unix seconds
  not_after           : u64_be      # unix seconds
  public_key_alg      : u8          # 0x01 Ed25519 / 0x02 X25519
  public_key          : bytes32     # subject public key
  key_usage           : u16_be      # bitmask (Section 6.4)
  eku_count           : u8          # max 8
  eku_list            : u16_be[eku_count]  # ascending, duplicate yok
  basic_constraints   : BasicConstraints   # Section 8.2
  extension_count     : u16_be      # max 16
  extensions          : Extension[extension_count]   # ext_id ascending (Section 28)
}
```

`canonical_ccr_tbs` = yukarıdaki alanların **tam sırada, fixed-width big-endian** concatenation'ı. `signature` alanı TBS'te **yoktur**.

### 8.1 SPKI Encoding ve key_id

```
spki_encoding(public_key_alg, public_key) =
    public_key_alg (u8) || public_key (bytes32)        # toplam 33 byte

subject_key_id = SHA-256(spki_encoding(subject_alg, subject_pub))
issuer_key_id  = SHA-256(spki_encoding(issuer_alg, issuer_pub))
```

> **identity_hash ilişkisi (kritik):** Identity CA / User Identity CCR için `public_key_alg = 0x01 (Ed25519)`, `public_key = user_identity_ed25519_pub`. v1.5 gereği `identity_hash = SHA-256(user_identity_ed25519_pub)` — yani **ham 32-byte public key üzerinden**, spki_encoding (33 byte) üzerinden DEĞİL. Bu iki hash farklıdır ve karıştırılmamalıdır:
> ```
> identity_hash  = SHA-256(public_key)                          # 32 byte ham key
> subject_key_id = SHA-256(public_key_alg || public_key)        # 33 byte spki
> ```
> `identity_hash` routing identity türetimidir (v1.5 CAD-16); `subject_key_id` CCR-içi chain referansıdır. Implementasyon ikisini ayrı hesaplar.

### 8.2 BasicConstraints

```
BasicConstraints {
  is_ca            : bool_u8       # CA:TRUE=0x01, CA:FALSE=0x00
  path_len_present : bool_u8
  path_len         : u8            # path_len_present=0x00 ise 0x00 olmalı
}
```
Kurallar: `is_ca=0x00` ise `path_len_present` 0x00 olmalı ve cert keyCertSign taşıyamaz (Section 29). Toplam 3 byte (sabit).

### 8.3 CompactDeviceCertV1 device_cert_hash (hot-path device cert)

```
# Aktif wire device cert = CompactDeviceCertV1 (Section 13), Generic CCR envelope DEĞİL
canonical_compact_device_cert =
  device_ed25519_pub || device_x25519_pub || issuer_key_id || not_after || issuer_signature   # 152B

device_cert_hash = SHA-256(canonical_compact_device_cert)
```

> **Önemli:** `device_cert_hash`, CompactDeviceCertV1'in tam 152B'si (issuer_signature dahil) üzerinden hesaplanır. Bu, Session Model V3'nin (Section 35, SESSION_INIT) `sender_cert_ref` referansının spesifik imzalı Device Cert'e bağlanmasını sağlar (reference-first cache) ve revocation_id olarak kullanılır (Section 13.6). İmzalayan Identity/User Cert'tir (sk_user_identity), Intermediate DEĞİL.

### 8.4 Root CA Self-Signature ve Pinning

Root CA CCR:
- `cert_type = 0x01`, self-signed: `issuer_key_id == subject_key_id`, `issuer_name == subject_name`.
- `signature = Ed25519_Sign(root_sk, canonical_ccr_tbs)` (self).
- Runtime client Root CA'yı **ağdan trust source olarak çekmez**; yalnızca pinned `ROOT_CA_SPKI_HASH` ile doğrular:
  ```
  ROOT_CA_SPKI_HASH = SHA-256(spki_encoding(0x01, root_public_key))
  ```
- Chain doğrulama (v1.5 Section 15.2): bir CCR'ın issuer'ı ROOT ise, önce `SHA-256(spki_encoding(issuer_alg, issuer_spki)) == ROOT_CA_SPKI_HASH` kontrol edilir (pin), sonra imza o doğrulanmış Root public key ile verify edilir.

> **Pin hash girdisi netleştirmesi:** `ROOT_CA_SPKI_HASH`, **spki_encoding** (33 byte: alg||key) üzerinden hesaplanır, ham key üzerinden değil. Bu, `issuer_key_id`/`subject_key_id` ile aynı hash fonksiyonudur (tutarlılık).

---

## 9. Certificate Types — Genel Kurallar

Her cert_type için: required fields, forbidden fields, allowed algorithms, key usage, EKU, basic constraints, pathLen, validity bounds, issuer, validation kuralları aşağıda tanımlıdır. Tüm tipler aynı `CCR` envelope (Section 7) ve `CCR_TBS` (Section 8) yapısını kullanır; farklılıklar alan değerlerindedir.

| cert_type | is_ca | path_len | key_usage | EKU | pub_alg | issuer |
|-----------|-------|----------|-----------|-----|---------|--------|
| 0x01 Root CA | TRUE | yok | keyCertSign+cRLSign | — | Ed25519 | self |
| 0x02 Intermediate | TRUE | 1 | keyCertSign+cRLSign+digitalSignature | intermediate | Ed25519 | Root |
| 0x03 Identity CA | TRUE | 0 | keyCertSign+cRLSign+digitalSignature | identity | Ed25519 | Intermediate |
| 0x04 Device | FALSE | — | digitalSignature+nonRepudiation | device | Ed25519 | Identity CA |
| 0x05 Hub Identity | TRUE | 0 | keyCertSign+cRLSign+digitalSignature | hub | Ed25519 | Intermediate |
| 0x06 Hub Noise | FALSE | — | keyAgreement | — | X25519 | Hub Identity |
| 0x07 RA Identity | TRUE | 0 | keyCertSign+cRLSign+digitalSignature | ra | Ed25519 | Intermediate |
| 0x08 RA Noise | FALSE | — | keyAgreement | — | X25519 | RA Identity |
| 0x09 CT Log Signing | FALSE | — | digitalSignature | ct-log | Ed25519 | Root veya Intermediate |
| 0x0A License Authority | FALSE | — | digitalSignature | license-authority | Ed25519 | Root |
| 0x0B Admin Server | FALSE | — | digitalSignature | admin-server | Ed25519 | Intermediate (per-org) |

---

## 10. Root CA CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x01 |
| public_key_alg | 0x01 (Ed25519 only) |
| is_ca | TRUE |
| path_len_present | FALSE (sınırsız) |
| key_usage | keyCertSign(0x0002) + cRLSign(0x0004) = 0x0006 |
| eku_count | 0 |
| issuer | self (issuer_key_id == subject_key_id) |
| validity | max 20 yıl |
| signature | self: Ed25519_Sign(root_sk, canonical_ccr_tbs) |

**Validation:**
- `issuer_key_id == subject_key_id` ve `issuer_name == subject_name` (self-signed).
- `SHA-256(spki_encoding(0x01, public_key)) == ROOT_CA_SPKI_HASH` (pin).
- Ağdan trust source olarak **çekilmez**; yalnızca pin doğrulaması.
- X25519 public key → reject (Ed25519 only).

> **Root imzalama yetkisi (NORMATİF):** Root CA self-signed trust anchor'dur ve şunları imzalar: Intermediate CA cert, **License Authority cert (cert_type 0x0A)**, emergency/root-level trust update. Root **OrgEntitlementRecord'u DOĞRUDAN İMZALAMAZ** (offline kalır; OrgEntitlement License Authority tarafından imzalanır, §17.5/§21.6).

---

## 11. Intermediate CA CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x02 |
| public_key_alg | 0x01 (Ed25519) |
| is_ca | TRUE, path_len_present=TRUE, path_len=1 |
| key_usage | 0x0006 + digitalSignature(0x0001) = 0x0007 |
| eku_list | [0x0001 id-kama-intermediate] |
| issuer | Root CA |
| validity | max 5 yıl |

**Validation:** issuer chain → Root (pin). path_len=1. Identity CA, Hub Identity, RA Identity, CT Log Signing, Admin Server CCR'larını + HubListRecord + AuthorizedHubSetRecord + DiscoveryEndpointRecord + RevocationSnapshot/Delta checkpoint'lerini imzalamaya yetkili (CRLite/checkpoint imza semantiği `revocation-model.md`). **Intermediate CA OrgEntitlementRecord'u İMZALAMAZ** (lisans otoritesi değil; OrgEntitlement License Authority-imzalı, §17.5/§21.6).

> **NAMESPACE OMURGASI (identity v1.5 CAD-52):** Intermediate CA 5 yıl, kurum başına ayrı key, RoftCore'da. **Business-license enforcement mekanizması DEĞİLDİR** — kurumun kriptografik namespace omurgasıdır (Identity CA, Hub/RA/Admin cert, HubListRecord, CT/audit, CRLite issuer scope ona bağlı). Yalnızca planned rollover veya compromise response ile değişir. Lisans Intermediate expiry ile DEĞİL, OrgEntitlementRecord + RA license gate + Identity Cert 1 yıl ile enforce edilir (identity v1.5 E-11). OrgEntitlementRecord ise **RoftCore License Authority** (cert_type 0x0A, Intermediate değil, Root değil) tarafından imzalanır (Section 21.6).

---

## 12. Identity CA / User Identity CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x03 |
| public_key_alg | 0x01 (Ed25519) |
| public_key | `user_identity_ed25519_pub` |
| is_ca | TRUE, path_len=0 |
| key_usage | 0x0007 |
| eku_list | [0x0002 id-kama-identity] |
| extensions | nameConstraints (0x0001) zorunlu: URI `kama://<dest_hash>/*` |
| issuer | Intermediate CA (RA aracılığıyla) |
| validity | **max 1 yıl** (RA license gate + Admin approval ile yenilenir; `not_after <= OrgEntitlementRecord.license_not_after` — identity v1.5 E-11) |

**Türetim (v1.5 CAD-16):**
```
identity_hash = SHA-256(public_key)              # ham 32-byte ed25519 pub
dest_hash     = SHA-256(identity_hash)
routing_key   = dest_hash[0:16]
```

**Validation:** nameConstraints URI'sindeki `<dest_hash>` (hex/base16, Section 24.2), `SHA-256(SHA-256(public_key))` ile **eşleşmeli** — burada `public_key = user_identity_ed25519_pub` (bu Identity/User Cert'in KENDİ key'i; yani `dest_hash = SHA-256(identity_hash)`, `identity_hash = SHA-256(user_identity_ed25519_pub)`). path_len=0 → yalnızca end-entity (Device) imzalayabilir. Device CCR'ı (CompactDeviceCertV1) **lokal** imzalar (`sk_user_identity` = bu Identity/User Cert'in private key'i, CA online değil). Bu Identity/User Cert key'i CompactDeviceCert'lerin routing identity'sini (issuer_identity_hash/issuer_dest_hash) belirler — Section 13.3/13.5.

> **Hex canonical case (NORMATİF — interop kritik):** URI içindeki tüm hex-encoded değerler (`<dest_hash>`, `<device_id>`) **lowercase** (`0-9a-f`) olmalıdır. Uppercase veya mixed-case hex → reject. Bu, bytewise-unsigned ascending sıralamanın (Section 24.2) Java/Rust arasında deterministik olmasını ve nameConstraints eşleşmesinin canonical olmasını garanti eder. Üretici lowercase üretir; doğrulayıcı lowercase olmayan hex'i reject eder (re-canonicalize ETMEZ — ham byte eşleşmesi).

---

## 13. Device CCR / CompactDeviceCertV1

> **İKİ TEMSİL (NORMATİF ayrım):** (a) **CompactDeviceCertV1** = ağda dolaşan tek client device cert (hot-path, sabit 152B, aşağıda). (b) **Generic Device CCR** (cert_type=0x04) = yalnızca audit/mirror/non-hot-path temsil; **aktif wire sender device cert olarak hostile wire'da KULLANILMAZ** (aktif = CompactDeviceCertV1, Session Model V3). Aktif wire device cert = CompactDeviceCertV1. Implementer tek aktif path kullanır: CompactDeviceCertV1.

### 13.1 CompactDeviceCertV1 — AĞDA DOLAŞAN TEK CERT (sabit 152B)

```
CompactDeviceCertV1 {
  device_ed25519_pub : bytes32     # device SIGNING key (SIGNED_CONTROL_MESSAGE / delivery receipts / audit signatures — normal hot-path İMZASIZ, CAD-55)
  device_x25519_pub  : bytes32     # statik DH (Session Model V3 recipient static key — SESSION_INIT ss_s/ss_e, Section 35)
  issuer_key_id      : bytes16     # Identity/User Cert subject_key_id[0:16] (cache lookup hint)
  not_after          : uint64_be   # geçerlilik sonu (revocation penceresi; max +90 gün)
  issuer_signature   : bytes64     # Ed25519_Sign(sk_user_identity, compact_device_cert_tbs)
}
TOPLAM = 32 + 32 + 16 + 8 + 64 = 152 bytes (EXACT)
```

> **İMZALAYAN = Identity CA / User Identity Cert (KRİTİK):** `issuer_signature` Identity/User Cert private key'i `sk_user_identity` ile üretilir — **Intermediate DEĞİL**. Intermediate yalnızca Identity/User Cert'i imzalar. Device Cert trust path: **CompactDeviceCert → Identity/User Cert → Intermediate → Root**. `intermediate_sig` terimi CompactDeviceCert'te KULLANILMAZ (yalnızca gerçekten Intermediate-imzalı HubListRecord'da geçer).

### 13.2 compact_device_cert_tbs (imzalanan) + canonical object

```
compact_device_cert_tbs =
  domain_sep("SR_CDC_V1") ||           # domain separation (16B sabit etiket)
  device_ed25519_pub ||
  device_x25519_pub ||
  issuer_key_id ||
  not_after

issuer_signature = Ed25519_Sign(sk_user_identity, compact_device_cert_tbs)

canonical_compact_device_cert =        # ağda dolaşan tam 152B
  device_ed25519_pub || device_x25519_pub || issuer_key_id || not_after || issuer_signature

device_cert_hash = SHA-256(canonical_compact_device_cert)    # = revocation_id (Section 13.6)
```

> **Boyut kararı (NORMATİF):** CompactDeviceCertV1 hot-path'te magic/version alanı TAŞIMAZ (152B korunur); tip/versiyon protokol context'inden gelir. Gelecekte explicit versiyon gerekirse **CompactDeviceCertV2** tanımlanır ve boyut artışı kabul edilir. **Alpha hot-path CompactDeviceCertV1 = tam 152B sabit nesne: magic yok, version yok, cert_type yok, serial yok, extension yok, SAN URI yok, device_id yok.**

### 13.3 Kimlik türetimi — DEVICE key'den DEĞİL (KRİTİK)

```
# YANLIŞ (KULLANMA): identity_hash = SHA-256(device_ed25519_pub)
# DOĞRU: kimlik yalnızca issuer Identity/User Cert key'inden
issuer_identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub)
issuer_dest_hash     = SHA-256(issuer_identity_hash)
routing_key          = issuer_dest_hash[0:16]
```

CompactDeviceCert içindeki `device_ed25519_pub`, **user identity key DEĞİLDİR** — yalnızca device signing key'dir. `identity_hash`/`dest_hash` yalnızca Identity/User Cert'teki `user_identity_ed25519_pub`'tan türetilir. CompactDeviceCert kendisi dest_hash TAŞIMAZ ve dest_hash'i `device_ed25519_pub`'tan TÜRETMEZ.

### 13.4 issuer_key_id semantiği

```
CompactDeviceCertV1 issuer_key_id:
  bytes16 = Identity/User Cert subject_key_id[0:16]
  Yalnızca compact cache lookup hint.
  Doğrulama, çözülen Identity/User Cert'in issuer_signature'ı gerçekten doğruladığını TEYİT ETMELİ.
  Lookup için collision-resistant yeterli; tek başına trust otoritesi DEĞİL (imza final authority).
```
(Generic CCR `subject_key_id`/`issuer_key_id` = bytes32 = SHA-256(spki); Section 8.1. CompactDeviceCert 16B kullanır — Section 13.1.)

### 13.5 Namespace izolasyonu (SAN olmadan, doğru model)

```
identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub)
dest_hash     = SHA-256(identity_hash)

IdentityCert nameConstraints / namespace commitment dest_hash ile EŞLEŞMELİ.
CompactDeviceCert o IdentityCert altındadır ÇÜNKÜ issuer_signature
  user_identity_ed25519_pub ile doğrulanır.

CompactDeviceCert kendisi dest_hash taşımaz.
CompactDeviceCert dest_hash'i device_ed25519_pub'tan türetmez.
DiscoveryRecordV2.dest_hash, CompactDeviceCert'i imzalayan Identity/User Cert key'inden
  türetilen dest_hash'e EŞİT olmalı (Section 21.2).
```

### 13.6 Revocation identifier (hash-based, serial YOK)

```
CompactDeviceCertV1'in serial alanı YOKTUR.
Device revocation:
  revocation_id = device_cert_hash = SHA-256(canonical_compact_device_cert)

CRLite/CRLSet device cert'leri için device_cert_hash revocation entry DESTEKLEMELİ.
Generic CCR cert'leri serial-based revocation kullanabilir.
CompactDeviceCert hash-based revocation kullanır.

Future option: CompactDeviceCertV2 serial bytes16 ekleyip 168B'ye büyüyebilir. Alpha BUNU YAPMAZ.
```

### 13.7 Statik X25519 (rotation YOK)

`device_x25519_pub` statik Session Model V3 recipient key'idir (SESSION_INIT: `ss_e=DH(sender_ephemeral, recipient_static)`, `ss_s=DH(sender_static, recipient_static)`; Section 35.2). **Normal Device Cert renewal'da (90 gün) `device_x25519_pub` AYNI kalır** — rotation YOK; forward secrecy her mesajdaki taze `sender_ephemeral` ile sağlanır. Yeni `device_x25519_pub` YALNIZCA explicit compromise/recovery path'inde üretilir (eski cert revoke + yeni CompactDeviceCert + DiscoveryRecordV2 update; identity v1.5 §15.13). Alpha'da yalnızca Session Model V3 için kullanılır (sync/voice Beta+ → ayrı static X25519 key). KAYIP: oturum-başlangıcı FS zayıflar (static sızarsa bekleyen async mesajlar) — kabul (cihaz ele geçirme kapsam dışı). `device_x25519_pub` `compact_device_cert_tbs`'e dahildir (imza onu da korur).

### 13.8 Validation — verify_compact_device_cert

```
verify_compact_device_cert(compact_cert, identity_cert, org_entitlement, revocation_context) -> validated | FAIL:
  # revocation_context ∈ { FreshRevocationState, KnownContactBoundedStaleCache, NoFreshState }
  # (semantik policy: revocation-model.md §7.5 + identity §15.3; CCR byte layout DEĞİŞMEZ)
  1-3. Parse tam 152B; malformed length / trailing byte → reject
  4.   device_ed25519_pub geçersiz Ed25519 key (kütüphane destekliyorsa) → reject
  5.   device_x25519_pub all-zero → reject
  6.   low-order X25519 (kütüphane destekliyorsa) → reject; en azından DH all-zero shared secret → reject
  7.   issuer_key_id ile Identity/User Cert çöz; issuer_key_id == identity_cert.subject_key_id[0:16] teyit
  8.   Identity/User Cert chain doğrula: Identity/User Cert → Intermediate → Root (pinned)
  9.   Ed25519_Verify(identity_cert.user_identity_ed25519_pub, compact_device_cert_tbs, issuer_signature)
       # INTERMEDIATE key DEĞİL — Identity/User Cert key (KRİTİK)
  10.  now <= not_after   # bounded-stale dahil HİÇBİR context expiry'yi bypass etmez
  11.  not_after <= identity_cert.not_after (parent containment)
  12.  OrgEntitlementRecord context'te ise: not_after <= org_entitlement.license_not_after
  13.  device_cert_hash = SHA-256(canonical_compact_device_cert)
  14.  Revocation (revocation_context'e göre):
       FreshRevocationState:
         normal CRLite/CRL check; revocation_id = device_cert_hash; revoked → reject (fail-closed)
       KnownContactBoundedStaleCache (YALNIZCA şu koşulların HEPSİ):
         - device_cert_hash == cached current_device_cert_hash (TAM eşleşme)
         - cached validation FRESH revocation state altında yapılmıştı
         - cached snapshot bu cert'i revoked İŞARETLEMEMİŞ (işaretlediyse → reject)
         - adım 7-9 (chain + imza) yine GEÇTİ (bypass yok)
         - adım 10 (expiry) yine GEÇTİ (bypass yok)
         → sonuç stale_revocation=true / risk flag İÇERMEK ZORUNDA
       NoFreshState:
         yeni/unknown trust kararı → reject (fail-closed)
  15.  return validated {
         device_ed25519_pub,                                    # SIGNED_CONTROL/receipts/audit imza (hot-path imzasız)
         device_x25519_pub,                                     # Session Model V3 recipient static (ss_s/ss_e)
         device_cert_hash,
         issuer_identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub),  # routing identity
         issuer_dest_hash     = SHA-256(issuer_identity_hash),
         stale_revocation     = (revocation_context == KnownContactBoundedStaleCache)  # risk flag
       }
```

> **NORMATİF (yetki sınırı):** ccr-format-spec **byte-level authority** olarak kalır. Revocation stale
> policy (known-contact tanımı, bounded-stale matrix, audit event) **semantik validation policy**'dir ve
> revocation-model.md §7.5 + identity-and-trust-model.md §15.3'e aittir. Bu değişiklik HİÇBİR CCR byte
> layout'unu değiştirmez.

> **NORMATİF (issuer_key_id — lookup hint only, KAT1-P2-03):** `issuer_key_id[0:16]` YALNIZCA cache
> lookup hint'idir. Trust authority olarak KULLANILAMAZ. Trust şunları gerektirir: Identity/User Cert'i
> resolve etmek, chain'i pinned Root'a doğrulamak ve CompactDeviceCert `issuer_signature`'ını **resolved**
> `user_identity_ed25519_pub` ile doğrulamak (adım 7→8→9). 16-byte truncation collision'ı yalnızca yanlış
> cert'e resolve → adım 9 imza FAIL → reject üretir (availability etkisi; security bypass DEĞİL).

> **NORMATİF (operator isolation — katman ayrımı, KAT1-P2-02):** `verify_compact_device_cert` yalnız
> **kriptografik chain + signature + revocation** doğrulaması yapar; **operator namespace authorization
> kararı DEĞİLDİR.** Cross-operator isolation discovery, HubList, AuthorizedHubSet, operator_id scope ve
> routing policy katmanında enforce edilir. Root'a chain eden bir cert cryptographically valid olabilir,
> fakat local operator scope içinde authorized olduğu anlamına gelmez. Routing/discovery dışındaki
> yüzeylerde (örn. audit, harici entegrasyon) caller, operator context ile AYRICA authorization yapmalıdır.

> **NORMATİF:** `issuer_identity_hash`/`issuer_dest_hash` `device_ed25519_pub`'tan TÜRETİLMEZ — yalnızca Identity/User Cert'in `user_identity_ed25519_pub`'undan.

**Forbidden:**
- `intermediate_sig` CompactDeviceCert'te → reject (yanlış model; imza Identity/User Cert'ten).
- `identity_hash`/`dest_hash` device key'den türetim → yasak (Section 17 MUST NOT IMPLEMENT).
- SignedNoisePrekey → KALDIRILDI (Section 18 DEPRECATED).
- is_ca=TRUE / keyCertSign → reject (device end-entity).
- Generic Device CCR aktif wire sender device cert olarak KULLANILMAZ (MUST NOT). Aktif wire device cert = CompactDeviceCertV1 (Session Model V3'ün referansladığı). → yasak (Section 17).


---

## 14. Hub Identity CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x05 |
| public_key_alg | 0x01 (Ed25519) |
| is_ca | TRUE, path_len=0 |
| key_usage | 0x0007 |
| eku_list | [0x0004 id-kama-hub] |
| extensions | nameConstraints (0x0001): DNS + IP; SAN (0x0002): DNS + IP (zorunlu) |
| issuer | Intermediate CA (RA aracılığıyla) |
| validity | **max 3 yıl** (Intermediate altyapısı, **lisanstan bağımsız**; lisans enforcement hub cert expiry ile YAPILMAZ) |

> **Hub Identity lifetime (NORMATİF, madde 9):** Hub Identity Cert 3 yıl — lisans yetkisi anlamına GELMEZ. Lisans enforcement hub cert expiry ile değil, HubListRecord validity ile yapılır: lisans bittiğinde RA yeni HubListRecord vermez, `HubListRecord.not_after <= OrgEntitlementRecord.license_not_after` olduğu için hub bootstrap/routing kademeli durur; ama Hub Identity Cert (3 yıl) geçerli kalabilir. Aynı mantık RA Identity (3 yıl) için de geçerli.

**Validation:** Yalnızca **Hub Noise CCR + ANNOUNCE payload + RoutingRecordV2** imzalar (v1.5 CAD-22 güncel; blocker-9). Başka imzalama rolü → reject. SAN DNS/IP zorunlu; ANNOUNCE adresleriyle eşleşmeli (routing-rendezvous-architecture.md). RoutingRecordV2 hub-imzalı routing kaydıdır (dest_hash→hub_id); Hub Identity key onu da imzalar (ayrı Hub Routing Signing cert tanımlanMAZ — tek Hub Identity key üç rolü kapsar: Noise CCR + ANNOUNCE + RoutingRecordV2).

---

## 15. Hub Noise CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x06 |
| public_key_alg | 0x02 (X25519) |
| is_ca | FALSE |
| key_usage | keyAgreement(0x0008) |
| eku_count | 0 |
| issuer | Hub Identity CCR |
| validity | max 1 yıl |

**Validation:** X25519 public key. Yalnızca Hub Noise key agreement (transport). Client tarafında statik X25519 Device CCR'da taşınır (Section 13). keyCertSign/Ed25519 → reject.

---

## 16. RA Identity / RA Noise CCR

### 16.1 RA Identity CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x07 |
| public_key_alg | 0x01 (Ed25519) |
| is_ca | TRUE, path_len=0 (RA Noise CCR imzaladığı için) |
| key_usage | 0x0007 |
| eku_list | [0x0005 id-kama-ra] |
| extensions | nameConstraints + SAN: DNS/IP |
| issuer | Intermediate CA |
| validity | max 3 yıl |

### 16.2 RA Noise CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x08 |
| public_key_alg | 0x02 (X25519) |
| is_ca | FALSE |
| key_usage | keyAgreement(0x0008) |
| issuer | RA Identity CCR |
| validity | max 1 yıl |

**Confirmed (v1.5 CA/RA izolasyon):**
- RA, Intermediate CA ile **izole mTLS** (opsiyonel gRPC over mTLS) üzerinden konuşur.
- RA→CA'da Kama custom adaptive transport **kullanılmaz** (izole mTLS/gRPC).
- Client/Hub yalnızca RA ile konuşur; Intermediate CA veya Root CA ile **asla doğrudan** konuşmaz.
- Client/Hub Intermediate CA'nın IP/konumunu bilmez; Root CA'yı yalnızca pinned `ROOT_CA_SPKI_HASH` olarak bilir.
- RA Noise CCR client→RA Noise NK handshake'inde `rs` olarak kullanılır (bootstrap bundle'dan pinned).

---

## 17. CT Log Signing CCR

| Alan | Değer |
|------|-------|
| cert_type | 0x09 |
| public_key_alg | 0x01 (Ed25519) |
| is_ca | FALSE |
| key_usage | digitalSignature(0x0001) |
| eku_list | [0x0006 id-kama-ct-log] |
| issuer | Root CA veya Intermediate CA |
| validity | Root/Intermediate'ten kısa |

**Validation:** Yalnızca CT checkpoint/STH imzalar. **Connection authorization DEĞİL** (v1.5 CAD-23). CT async audit only; CT unavailable → fail-open (identity validation engine'de). Bu CCR connection kararlarında kullanılmaz.

---

## 17.5 License Authority CCR

RoftCore'un online/HSM-backed lisans imza anahtarı. Root CA offline kalır; OrgEntitlementRecord'u Root **doğrudan imzalamaz**, Intermediate **imzalamaz** (Intermediate kurum namespace omurgası). License Authority **yalnızca OrgEntitlementRecord** imzalar (pki-ca-ra-hsm-tdd.md §4 ile birebir).

| Alan | Değer |
|------|-------|
| cert_type | 0x0A |
| public_key_alg | 0x01 (Ed25519) |
| is_ca | FALSE |
| path_len_present | FALSE |
| key_usage | digitalSignature(0x0001) only |
| eku_list | [0x0007 id-kama-license-authority] |
| issuer | Root CA |
| validity | 1-3 yıl, önerilen 2 yıl |

**Validation:**

```
- cert_type == 0x0A
- public_key_alg == Ed25519
- is_ca == FALSE
- path_len_present == FALSE
- key_usage TAM OLARAK digitalSignature (0x0001); keyCertSign/cRLSign/keyAgreement/nonRepudiation varsa → reject
- eku_list, id-kama-license-authority (0x0007) içermeli; license-authority dışı signing capability vermez
- issuer chain → Root CA, ROOT_CA_SPKI_HASH pinli (recursive verify_chain)
- now validity aralığında
- revoked değil
```

**Allowed signing:**

```
License Authority → OrgEntitlementRecord ONLY.
```

**Forbidden signing (hepsi kritik authorization ihlali):**

```
License Authority → Root / Intermediate / Identity-User Cert
License Authority → CompactDeviceCert
License Authority → Hub Identity / Hub Noise / RA Identity / RA Noise / Admin Server
License Authority → HubListRecord
License Authority → AuthorizedHubSetRecord
License Authority → DiscoveryEndpointRecord
License Authority → DiscoveryRecord
License Authority → RoutingRecord / ANNOUNCE
License Authority → RevocationRecord / RevocationPush
License Authority → RevocationSnapshot / RevocationDelta
License Authority → CT checkpoint / STH
```

> **NORMATİF:** Bir verifier, License Authority public key'iyle OrgEntitlementRecord DIŞINDA herhangi bir object/cert imzasını doğrulamayı kabul ederse, bu **kritik authorization bug**'dır. License Authority CA değildir (is_ca=FALSE, keyCertSign yok); cert üretemez.

---

## 17.6 Admin Server CCR

Kurumun Admin Server'ının makine kimliği. Admin↔RA mTLS peer authentication için kullanılır (admin-control-plane-tdd.md). Per-org Intermediate tarafından imzalanır. Admin Server **hiçbir canonical Kama record/cert imzalamaz** — yalnız mTLS peer kimliği (+ opsiyonel kendi control-request envelope imzası).

| Alan | Değer |
|------|-------|
| cert_type | 0x0B |
| public_key_alg | 0x01 (Ed25519) |
| is_ca | FALSE |
| path_len_present | FALSE |
| key_usage | digitalSignature(0x0001) only |
| eku_list | [0x0008 id-kama-admin-server] |
| issuer | per-org Intermediate CA |
| validity | 1-3 yıl (Alpha önerisi 1 yıl) |
| extensions | operator_id / admin_server_id / SAN |

**Validation:**

```
- cert_type == 0x0B
- public_key_alg == Ed25519
- is_ca == FALSE
- path_len_present == FALSE
- key_usage TAM OLARAK digitalSignature (0x0001); keyCertSign/cRLSign/keyAgreement varsa → reject
- eku_list, id-kama-admin-server (0x0008) içermeli
- issuer chain → per-org Intermediate → Root, ROOT_CA_SPKI_HASH pinli (recursive verify_chain)
- operator_id, request/operator scope ile eşleşir
- subject_key_id/admin_server_id RA'da o operator_id için register edilmiş
- now validity aralığında
- revoked değil (fresh state, fail-closed)
```

**Allowed use:**

```
Admin Server Cert → Admin↔RA mTLS peer authentication (+ opsiyonel AdminControlRequest envelope imzası).
```

**Forbidden signing (hepsi kritik authorization ihlali):**

```
Admin Server → Identity/User Cert / CompactDeviceCert
Admin Server → HubListRecord
Admin Server → AuthorizedHubSetRecord
Admin Server → DiscoveryEndpointRecord
Admin Server → RevocationRecord / RevocationPush
Admin Server → RevocationSnapshot / RevocationDelta
Admin Server → OrgEntitlementRecord
```

> **NORMATİF:** Admin Server CA değildir (is_ca=FALSE, keyCertSign yok). Admin yalnız draft hazırlar; HubList/AHS/DiscoveryEndpoint canonical imzaları RA gate SONRASI Intermediate tarafından, revocation imzaları RA Identity tarafından üretilir (admin-control-plane-tdd.md §9). Bir verifier Admin Server public key'iyle yukarıdaki object'lerden herhangi birinin imzasını kabul ederse, bu kritik authorization bug'dır.

---

## 18. SignedNoisePrekey Encoding — ⚠️ DEPRECATED / MUST NOT IMPLEMENT (Haziran 2026 — statik X25519 ile KALDIRILDI)

> **MUST NOT IMPLEMENT (Device CCR trust-model yasakları — bu pass, Haziran 2026):**
> ```
> - identity_hash'i device pk_ed25519'dan türetmek
> - dest_hash'i device pk_ed25519'dan türetmek
> - CompactDeviceCert'te intermediate_sig alanı/ismi
> - CompactDeviceCert'i doğrudan Intermediate ile imzalamak
> - CompactDeviceCert issuer_key_id'nin Intermediate'i işaret etmesi
> - Generic Device CCR'ı aktif wire sender device cert olarak kullanmak (aktif = CompactDeviceCertV1, Session Model V3)
> - CompactDeviceCert için device_cert_hash desteklemeyen yalnızca-serial-tabanlı revocation
> - Client↔Client async = Session Model V3 N-türevi authenticated one-way (interaktif Noise IK/KK DEĞİL; Section 35)
> - SignedNoisePrekey / PrekeyBundle aktif davranışı (aşağıda)
> - normal Device Cert renewal'da X25519 rotation (statik; yalnızca compromise/recovery'de yeni key)
> ```
> Doğru model: `identity_hash = SHA-256(user_identity_ed25519_pub)`; CompactDeviceCert `issuer_signature = Sign(sk_user_identity, ...)`; trust path CompactDeviceCert → Identity/User Cert → Intermediate → Root (Section 13).

> **DEPRECATED — KULLANILMIYOR (KRİTİK MİMARİ KARAR, Haziran 2026):** SignedNoisePrekey nesnesi **tamamen kaldırılmıştır.** Statik X25519 kararıyla (Section 13) X25519 transport anahtarı artık Device CCR'da statik olarak taşınır (`pk_device_x25519`). SignedNoisePrekey'in tek somut işi değişken X25519 prekey dağıtmaktı; statik X25519 ile bu iş bitti.
>
> **Neden kaldırıldı:** Değişken prekey = her mesajda hub'dan taze prekey çekme = hub bağımlılığı = hostile ağda bağlantı kurulamama. Statik X25519 = tek-temassız asenkron mesaj. Asenkron mesaj artık Session Model V3 authenticated one-way kullanır (Section 35): `ss_e = DH(sender_ephemeral, recipient_static)` + `ss_s = DH(sender_static, recipient_static)` → init_root — prekey nesnesine ihtiyaç YOK. `"SRPK"` magic, `canonical_prekey_tbs`, `sequence`-tabanlı prekey rotation state machine — hepsi kaldırıldı.
>
> **Yerine ne geldi:** Device CCR statik `pk_device_x25519` (Section 13) + DiscoveryRecord `current_device_x25519` (Section 21) + Session Model V3 (Section 35). Forward secrecy artık her mesajdaki taze `sender_ephemeral_x25519` ile sağlanır (prekey sequence rotation'a gerek yok).
>
> Aşağıdaki eski encoding **yalnızca tarihçe/referans içindir** — implementasyon BUNU KULLANMAMALI. Yeni implementasyonlar prekey nesnesi üretmez/doğrulamaz/saklamaz.

<details>
<summary>Eski SignedNoisePrekey encoding (DEPRECATED — implementasyon kullanmaz)</summary>

```
SignedNoisePrekey {
  magic                : bytes4      # "SRPK"
  version              : u8          # 1
  usage_context        : ascii_fixed[25]   # "SR-client-noise-static-v1"
  identity_hash        : bytes32
  device_id            : bytes16
  device_cert_hash     : bytes32     # (DEPRECATED struct; aktif: SHA-256(canonical_compact_device_cert))
  x25519_pub           : bytes32
  sequence             : u64_be      # monotonic per (identity_hash, device_id)
  not_before           : u64_be
  not_after            : u64_be
  signature            : bytes64      # Ed25519 over canonical_prekey_tbs
}
```
Eski toplam: 230 byte; eski TBS: 166 byte (magic-dahil "SRPK"). Bu yapı artık üretilmez.
canonical_prekey_tbs, sequence rotation, containment kuralları — hepsi DEPRECATED.

</details>



---

## 19. PrekeyBundle Encoding — ⚠️ DEPRECATED / MUST NOT IMPLEMENT (Haziran 2026 — prekey kaldırıldı)

> **DEPRECATED — KULLANILMIYOR:** PrekeyBundle, SignedNoisePrekey'i (Section 18, DEPRECATED) paketleyen container'dı. Prekey nesnesi statik X25519 kararıyla kaldırıldığı için PrekeyBundle de **gereksizdir.** Alice artık Bob'un statik X25519'unu DiscoveryRecord'dan (`current_device_x25519`, Section 21) alır — ayrı bir prekey bundle fetch'ine gerek yok. `"SRPB"` magic ve aşağıdaki yapı artık üretilmez/doğrulanmaz. Yeni implementasyonlar PrekeyBundle kullanmaz.

<details>
<summary>Eski PrekeyBundle encoding (DEPRECATED — implementasyon kullanmaz)</summary>

```
PrekeyBundle {
  magic                  : bytes4      # "SRPB"
  version                : u8          # 1
  identity_hash          : bytes32
  dest_hash              : bytes32
  routing_key            : bytes16
  device_id              : bytes16
  device_cert_hash       : bytes32
  signed_prekey_len      : u16_be      # SABİT u16_be; her zaman 230
  signed_noise_prekey    : SignedNoisePrekey   # tam signed_prekey_len byte
  record_sequence        : u64_be      # publication record sequence
  record_valid_until     : u64_be
}
```

### 19.1 İmza Modeli — Unsigned Container

`Confirmed:` PrekeyBundle bir **imzasız container**'dır. Authoritative imza, gömülü `SignedNoisePrekey.signature`'dır (sk_device). PrekeyBundle'ın kendi ayrı imzası **yoktur**.

**Gerekçe (v1.5 uyumlu):** SignedNoisePrekey zaten identity_hash, device_id, device_cert_hash, x25519_pub, sequence, lifetime'ı kriptografik olarak bağlar. PrekeyBundle bu alanları yalnızca routing/lookup kolaylığı için **tekrarlar** (dest_hash, routing_key dahil). Tekrarlanan alanlar gömülü prekey ile **çapraz-doğrulanır** (aşağıda); uyuşmazlık → reject. Ayrı imza eklemek gereksiz karmaşıklık ve ek attack surface olurdu. Bu, v1.5'in "SignedNoisePrekey self-authenticating" modeliyle tutarlıdır.

### 19.2 Validation

```
- magic == "SRPB", version == 1
- signed_prekey_len == 230 (sabit; mismatch → reject)
- signed_noise_prekey parse + format-valid (Section 18.2)
- identity_hash == signed_noise_prekey.identity_hash
- device_id == signed_noise_prekey.device_id
- device_cert_hash == signed_noise_prekey.device_cert_hash
- dest_hash == SHA-256(identity_hash)
- routing_key == dest_hash[0:16]
- record_valid_until <= signed_noise_prekey.not_after
- trailing byte yok
```

`record_sequence` (publication) ≠ `signed_noise_prekey.sequence` (key rotation) — iki ayrı kavram (v1.5). Hub PrekeyBundle'ı `(identity_hash, device_id)` ile saklar; daha yüksek `signed_noise_prekey.sequence` ile gelen güncelleme öncekini geçersiz kılar (hub forge edemez, yalnızca saklar/sunar — v1.5 CAD-30).

</details>

---

## 20. DeviceCertBundle Encoding

> **NORMATİF:** Aktif Alpha wire device cert = **CompactDeviceCertV1** (152B sabit, Section 13). DeviceCertBundle bu compact cert'i + issuer Identity/User Cert chain'ini paketler. **Generic Device CCR alanı KULLANILMAZ** (audit-only). "device_cert" = CompactDeviceCertV1.

```
DeviceCertBundle {
  magic                  : bytes4      # "SRDB"
  version                : u8          # 1
  compact_device_cert_v1 : bytes152    # tam 152B CompactDeviceCertV1 (Section 13.1; sabit, len-prefix YOK)
  identity_ccr_len       : u32_be
  identity_ccr           : bytes        # tam identity_ccr_len byte (Identity/User Cert = issuer)
  intermediate_ccr_len   : u32_be      # 0 = cached (boş)
  intermediate_ccr       : bytes        # tam intermediate_ccr_len byte (opsiyonel)
  device_cert_hash       : bytes32      # = SHA-256(canonical_compact_device_cert)
}
```

### 20.1 İmza Modeli — Unsigned Container

DeviceCertBundle **imzasızdır**; içerdiği CompactDeviceCertV1 (issuer_signature) ve Identity/Intermediate CCR'lar kendi imzalarını taşır. Bundle yalnızca chain materyalini paketler.

### 20.2 intermediate_ccr Opsiyonelliği

```
intermediate_ccr_len == 0  → intermediate CCR cache'te mevcut varsayılır
                              (parser boş byte dizisi olarak temsil eder)
intermediate_ccr_len > 0   → tam o uzunlukta intermediate CCR mevcut
```
Parser `intermediate_ccr_len == 0` durumunu "cached" flag'i olarak temsil eder; validation engine intermediate CCR'ı lokal cache'ten çözer. Cache'te de yoksa → chain doğrulanamaz → reject (v1.5: cache miss'te fetch).

### 20.3 Validation

```
- magic == "SRDB", version == 1
- compact_device_cert_v1 tam 152B (Section 13.1; malformed → reject)
- identity_ccr_len / intermediate_ccr_len gerçek byte uzunluğuna eşit (mismatch → reject)
- device_cert_hash == SHA-256(canonical_compact_device_cert)   # 152B; serial YOK, hash-based
- compact_device_cert_v1.issuer_signature → identity_ccr (user_identity_ed25519_pub ile doğrulanır; Section 13.8)
- identity_ccr chain → Intermediate CA → Root (pin)
- compact_device_cert_v1.issuer_key_id == identity_ccr.subject_key_id[0:16]
- trailing byte yok
```

> **Generic Device CCR (cert_type=0x04):** audit/mirror/non-hot-path temsil. DeviceCertBundle aktif wire path'inde KULLANILMAZ (aktif = CompactDeviceCertV1) (Section 13, Section 17). Aktif wire cert CompactDeviceCertV1'dir.

---

## 21. DiscoveryRecord Encoding Boundary

> VOPRF mekaniği `discovery-architecture.md`'ye aittir. Bu doküman yalnızca DiscoveryRecord'un canonical byte layout'unu ve user_signature TBS'ini tanımlar (interop için).

```
DiscoveryRecord {
  magic                     : bytes4      # "SRDR"
  version                   : u8          # 1
  identity_hash             : bytes32
  dest_hash                 : bytes32
  routing_key               : bytes16
  user_identity_ed25519_pub : bytes32
  current_device_id         : bytes16
  current_device_cert_hash  : bytes32
  current_device_x25519     : bytes32      # STATİK X25519 (CompactDeviceCert.device_x25519_pub)
  record_sequence           : u64_be
  valid_until               : u64_be
  ra_attestation            : varbytes16    # max 512 byte; opaque (RA/discovery docs)
  user_signature            : bytes64       # Ed25519_Sign(sk_user_identity, dr_tbs)
}
```

> **STATİK X25519 REFERANSI (KRİTİK MİMARİ KARAR — Haziran 2026):** DiscoveryRecord artık `current_prekey_bundle` (değişken SignedNoisePrekey) TAŞIMAZ. Bunun yerine `current_device_x25519` (CompactDeviceCert.device_x25519_pub statik) taşır. Gerekçe: statik X25519 ile prekey rotation YOK → değişken prekey bundle gereksiz. Discovery'de Alice tek seferde (iyi internette) Bob'un statik X25519'unu alır + cache'ler → 90 gün boyunca tekrar çekmez → hostile ağda asenkron Session Model V3 (SESSION_INIT) mesajı tek-temassız gönderir. `current_device_x25519`, `current_device_cert_hash` ile referans verilen CompactDeviceCert'in `device_x25519_pub`'una EŞİT olmalı (Alice doğrular). Eski `current_prekey_bundle` modeli İPTAL.

> **DiscoveryRecordV2 KONUM TAŞIMAZ (KRİTİK MİMARİ KARAR — Haziran 2026, CAD-53):** DiscoveryRecordV2 yalnızca **kimlik** taşır (dest_hash + cert + statik X25519); **hub konum bilgisi (home_hub_id) TAŞIMAZ.** Gerekçe: client routing topolojisini (Bob'un hangi hub'ta olduğunu) bilmemeli — (a) metadata minimizasyonu (client peer konumunu öğrenmez → social-graph/konum sızıntısı yok), (b) konum dinamiktir (failover ile değişir) ve statik imzalı record'da bayatlar. Routing tamamen hub-mesh + DHT'de: client yalnızca `dest_hash`'i kendi primary hub'ına verir, "hangi hub" sorusunu hub-mesh DHT RoutingRecord ile çözer (routing-rendezvous-architecture.md). DiscoveryRecordV2 = kimlik (client okur), RoutingRecord = konum (hub okur, client GÖRMEZ). İkisi ayrık. Eski `home_hub_id` alanı KALDIRILDI.

### 21.1 canonical_discovery_record_tbs

```
dr_tbs =
  magic || version || identity_hash || dest_hash || routing_key ||
  user_identity_ed25519_pub || current_device_id || current_device_cert_hash ||
  current_device_x25519 || record_sequence || valid_until ||
  ra_attestation (varbytes16: len || bytes)

user_signature = Ed25519_Sign(sk_user_identity, dr_tbs)
```
`user_signature` TBS'e **dahil değildir**.

> **Magic-in-TBS (NORMATİF, Section 1):** `dr_tbs` ilk alan olarak `magic` ("SRDR") içerir — `canonical_ccr_tbs` ("SRCR") ve Session Model V3 AAD domain separator'ları ("SSI1"/"SSR1"/"SSD1"/"SSC1") ile aynı domain-separation disiplini. Implementer DiscoveryRecord imzasını üretirken/doğrularken magic'i TBS'ten **çıkarmamalı**; aksi halde imza mismatch oluşur. Bu byte-level kuraldır; v1.5 semantiğini değiştirmez.

### 21.2 Validation (format-level) — verify_discovery_record_v2

```
1.  magic == "SRDR", version == 1
2.  identity_hash == SHA-256(user_identity_ed25519_pub)
3.  dest_hash     == SHA-256(identity_hash)
4.  routing_key   == dest_hash[0:16]
5.  Ed25519_Verify(user_identity_ed25519_pub, dr_tbs, user_signature)
6.  ra_attestation, discovery/RA policy'ye göre doğrulanır
7.  gerekirse DeviceCertBundle'ı current_device_cert_hash ile çöz
8.  CompactDeviceCertV1'i, user_identity_ed25519_pub ile eşleşen Identity/User Cert kullanarak doğrula
      (verify_compact_device_cert, Section 13.8)
9.  current_device_cert_hash == SHA-256(canonical_compact_device_cert)
10. compact_device_cert.device_x25519_pub == current_device_x25519
11. CompactDeviceCert issuer identity == DiscoveryRecordV2.user_identity_ed25519_pub
      (yani CompactDeviceCert'i imzalayan Identity/User Cert'in key'i == bu record'un user key'i)
12. current_device_x25519 geçerli X25519 public key (all-zero → reject)
13. record_sequence rollback koruması (highest_seen; eski reddedilir)
14. now <= valid_until
15. yeni trust kararı öncesi CRLite freshness (fail-closed)
16. plaintext phone YOK, plain phone hash YOK (yapısal); trailing byte yok
```

> **DiscoveryRecordV2 binding (NORMATİF):** Record, user identity'yi current device'a şöyle bağlar: (a) `user_signature` `current_device_cert_hash` + `current_device_x25519` üzerinde (Identity/User key ile), (b) CompactDeviceCert `issuer_signature` `sk_user_identity` ile (yani CompactDeviceCert o user'ın Identity cert'i altında), (c) `current_device_x25519 == CompactDeviceCert.device_x25519_pub`. Üçü birden: routing identity (user) ↔ device cert ↔ transport key kriptografik olarak bağlı. **`DiscoveryRecordV2.dest_hash`, CompactDeviceCert'i imzalayan Identity/User Cert key'inden türetilen dest_hash'e eşit olmalı** (device key'den DEĞİL).

> **device_id rolü (NORMATİF, identity v1.5 Section 15.14):** `current_device_id` bir **lookup etiketidir**; `current_device_cert_hash` kriptografik binding referansıdır. Client, `current_device_id`'yi `current_device_cert_hash` ve ilişkili validated CompactDeviceCert kontrol edilmeden KABUL ETMEMELİDİR. Discovery lookup `(identity_hash, current_device_id)` kullanabilir, AMA trust validation şunları kullanmalı: `current_device_cert_hash`, CompactDeviceCert chain validation, `current_device_x25519 == CompactDeviceCert.device_x25519_pub` eşleşmesi, issuer identity == user key, CRLite revocation. Çıplak `current_device_id` trust kararı vermez.

---

## 21.5 HubListRecord (Kurum Hub Bootstrap Listesi)

> **Mimari bağlam (Haziran 2026):** Client'lar bir kuruma ait hub'ların adreslerini RA'dan alır (app bundle'a gömülü DEĞİL — her federated kurumun hub'ları farklıdır, gömülemez). HubListRecord, kurumun Intermediate CA'sı tarafından imzalanır ve hub'ların bağlantı bilgilerini IP SAN içinde taşır. App bundle'da yalnızca RoftCore Root CA pinlenir (ROOT_CA_SPKI_HASH, sabit); client HubListRecord'u Intermediate imzasından Root'a kadar zincirle doğrular. Onboarding'de RA, client'a hem kendi sertifika zincirini hem o kuruma ait HubListRecord'u verir.

### 21.5.1 Wire Format

```
HubListRecord {
  magic               : bytes4      # "SRHL"
  version             : u8          # 1
  operator_id         : bytes16     # hangi kurum (Intermediate namespace)
  issuer_key_id       : bytes16     # imzalayan Intermediate CA (cache lookup)
  hub_count           : u16_be      # liste uzunluğu
  hubs                : HubEntryV3[hub_count]   # KAT5-P1-02: cert-bound (raw-key HubEntry DEPRECATED)
  sequence            : u64_be      # monoton (liste güncelleme; LWW)
  not_after           : u64_be      # liste geçerlilik sonu
  intermediate_sig    : bytes64     # Ed25519_Sign(sk_intermediate, canonical_hublist_tbs)
}

# --- HubEntryV3 (CERT-BOUND canonical model — KAT5-DECISION-3) ------------------
# Client hub Noise static key'ini RAW alandan DEĞİL, Hub Noise Cert'ten çıkarır.
# Hub Noise Cert, Hub Identity Cert tarafından imzalıdır (Intermediate → Root pin).
# Böylece client-facing bootstrap ile hub-mesh ANNOUNCE AYNI cert-bound identity modelini kullanır
# (iki ayrı authoritative raw-key temsili YOK → raw-key divergence eliminated).

HubEntryV3 {
  hub_id              : bytes16                     # hub kimliği (RoutingRecord ile eşleşir)
  address_count       : u8                          # 1..N
  addresses           : HubAddress[address_count]   # AddrEntry ≡ HubAddress (aşağıda)
  hub_identity_cert   : varbytes16                  # CCR envelope, cert_type=0x05 Hub Identity (Ed25519, EKU id-kama-hub)
  hub_noise_cert      : varbytes16                  # CCR envelope, cert_type=0x06 Hub Noise (X25519 keyAgreement, issuer=Hub Identity)
  hub_priority        : u8                          # 0=primary tercih, artan=fallback (çok-hub self-healing)
}
# Türetilen değerler (raw alan olarak TAŞINMAZ):
#   hub_ed25519_pub      = parse(hub_identity_cert).public_key
#   hub_noise_x25519_pub = parse(hub_noise_cert).public_key

HubAddress {
  ip_version          : u8          # 0x04 = IPv4, 0x06 = IPv6
  ip_bytes            : bytes4 (ip_version=0x04) | bytes16 (ip_version=0x06)   # versiyona göre sabit
  port                : u16_be
}
# HubAddress sabit-yapılı: IPv4 → 1+4+2 = 7B; IPv6 → 1+16+2 = 19B. len-prefix YOK (ip_version uzunluğu belirler).
# varbytes8 primitive'i KULLANILMAZ (tanımsızdı); HubAddress binary formatı yerini aldı.

# --- DEPRECATED (MUST NOT IMPLEMENT — KAT5) ------------------------------------
# Eski raw-key HubEntry Alpha canonical modeli DEĞİLDİR. Raw hub_noise_x25519_pub'ı
# tek başına authoritative kaynak olarak taşımak, client-facing key ile mesh Hub Noise
# Cert arasında iki ayrı authoritative temsil yaratıp raw-key divergence riski doğurur.
#   HubEntry { hub_id, hub_address, hub_ed25519_pub, hub_noise_x25519_pub, hub_priority }
#     → SUPERSEDED by HubEntryV3. MUST NOT IMPLEMENT.
```

### 21.5.2 canonical_hublist_tbs

```
canonical_hublist_tbs =
  "SRHL" (magic, 4B) || version || operator_id || issuer_key_id ||
  hub_count || (her HubEntryV3:
      hub_id ||
      address_count || (her HubAddress: ip_version || ip_bytes || port) ||
      varbytes16(hub_identity_cert) ||
      varbytes16(hub_noise_cert) ||
      hub_priority) ||
  sequence || not_after
# NOT: dış envelope alan seti/sırası (operator_id || issuer_key_id || hub_count || … || sequence || not_after)
# KORUNDU — yalnız per-hub serialization HubEntryV3'e taşındı (LWW sequence + license clamp not_after semantiği bozulmaz).

intermediate_sig = Ed25519_Sign(sk_intermediate, canonical_hublist_tbs)
```
`intermediate_sig` TBS'e **dahil değildir**. Magic-in-TBS disiplini (Section 1) geçerli.

### 21.5.3 Validation

```
- magic == "SRHL", version == 1
- issuer_key_id ile Intermediate CA cache'ten bulunur
- Ed25519_Verify(intermediate_pub, canonical_hublist_tbs, intermediate_sig)
- Intermediate chain → Root (pinned ROOT_CA_SPKI_HASH) — client Root'a pinli
- operator_id, Intermediate'in namespace'i ile tutarlı (cross-operator hub listesi reject)
- now <= not_after (expired liste reject)
- her HubEntryV3 için validate_hub_entry(entry, ROOT_CA_SPKI_HASH, now) geçmeli (§21.5.4; cert-bound)
- her HubAddress geçerli (ip_version ∈ {0x04,0x06}; ip_bytes uzunluğu versiyona uygun; malformed → tüm liste reject)
- hub_count == gerçek HubEntryV3 sayısı (mismatch → reject)
- LWW: daha yüksek sequence ile gelen liste öncekini geçersiz kılar
- trailing byte yok
```

### 21.5.4 validate_hub_entry (HubEntryV3 — CERT-BOUND, KAT5)

```
validate_hub_entry(entry, ROOT_CA_SPKI_HASH, now):
  1.  parse exact; trailing byte YOK.
  2.  hub_identity_cert.cert_type == 0x05 (HUB_IDENTITY_CERT).
  3.  hub_identity_cert.public_key_alg == Ed25519.
  4.  hub_identity_cert EKU id-kama-hub içerir.
  5.  verify_chain(hub_identity_cert, ROOT_CA_SPKI_HASH): Hub Identity → Intermediate → Root pin.
  6.  hub_noise_cert.cert_type == 0x06 (HUB_NOISE_CERT).
  7.  hub_noise_cert.public_key_alg == X25519.
  8.  hub_noise_cert.key_usage == keyAgreement.
  9.  hub_noise_cert.issuer_key_id == hub_identity_cert.subject_key_id.
  10. Ed25519_Verify(hub_identity_cert.public_key, hub_noise_cert.tbs, hub_noise_cert.signature).
  11. hub_noise_cert.public_key all-zero / low-order point DEĞİL → reject.
  12. entry.addresses, hub_identity_cert SAN (DNS/IP) policy ile eşleşir (san_matches_addresses).
  13. now, hub_identity_cert VE hub_noise_cert validity içinde.
  14. (HubListRecord signature Intermediate ile §21.5.3'te doğrulanır — dış katman.)
  → hub_ed25519_pub      = hub_identity_cert.public_key
  → hub_noise_x25519_pub = hub_noise_cert.public_key   # client Noise IK/KK responder static
```

> **Güvenlik:** HubListRecord Intermediate-imzalı olduğu için kötü hub veya MITM sahte hub listesi enjekte edemez (imza tutmaz). Client Root'a pinli → Intermediate→Root zinciri doğrulanır → liste güvenilir. IP SAN'lar Intermediate tarafından onaylanmış (kurum kendi hub'larını imzalar). `hub_ed25519_pub` ile client ANNOUNCE imzalarını ve Hub Noise Cert binding'ini doğrular (Section 22 / v1.5). `hub_priority` çok-hub self-healing'de hangi hub'ın primary olduğunu gösterir (failover sırası).

> **`hub_noise_x25519_pub` — Client↔Hub Noise bootstrap (NORMATİF, KAT4-P1-01):** Client, hub'ın Noise static X25519 public key'ini **handshake başlamadan önce** bilmek zorundadır: (1) Noise **IK** pattern'inde responder (hub) static key initiator (client) tarafından önceden bilinir ("K"=known); (2) ClientHint prologue `ECDH(client_ephemeral, hub_noise_x25519_pub)` ile AEAD şifrelenir (prologue handshake'ten önce). Bu nedenle hub Noise static key **in-band IK'da sunulmaz** — control-plane'den, Intermediate-imzalı HubListRecord (veya onboarding bootstrap bundle) içinde gelir. Client bu key'i doğrulanmış (Intermediate→Root pinned) kaynaktan almış olur → MITM hub_noise_x25519_pub enjekte edemez. `KK` 0-RTT de bu cached key'i kullanır.

> **`hub_noise_x25519_pub` KAYNAĞI — CERT-BOUND (NORMATİF, KAT5-P1-02, KAT4-P1-01'i günceller):** Client, `hub_noise_x25519_pub`'ı artık **raw alandan DEĞİL**, `HubEntryV3.hub_noise_cert.public_key`'den çıkarır (§21.5.4). Hub Noise Cert, Hub Identity Cert tarafından imzalıdır (`issuer_key_id == hub_identity_cert.subject_key_id`, X25519 keyAgreement) ve Hub Identity → Intermediate → Root pin zinciriyle doğrulanır. Böylece client-facing Noise key ile hub-mesh ANNOUNCE'taki Hub Noise Cert **tek nesnedir** — iki ayrı authoritative raw-key temsili yoktur, raw-key divergence imkânsızdır. KAT4-P1-01'in "key handshake öncesi bilinmeli (IK K=known + ClientHint prologue)" gerekçesi geçerliliğini korur; yalnızca key'in KAYNAĞI cert-bound olur. Compromised/coerced Admin Server bile client-facing Noise key'i ikame edemez (sahte Hub Noise Cert için hub'ın Identity private key'i gerekir).

> **Hub Noise key rotation (NORMATİF):** Hub Noise static key rotate edilirse HubListRecord `sequence`+1 ile yeniden yayımlanır (Admin Server hazırlar → Intermediate imzalar). Rotation sırasında **eski ve yeni noise key için overlap/grace period** gerekir (in-flight KK handshake'leri kırılmasın). Eski key tamamen kaldırıldıysa: client eski key ile KK dener → decrypt fail → yeni HubListRecord fetch eder (iyi ağ) VEYA `hub_priority`/AHS ile başka authorized hub'a failover eder (CAD-46). Client hostile ağda güncel HubListRecord çekemezse, cached HubEntry'lerden en yüksek `sequence`'lı liste kullanılır (LWW).

> **Lisans clamp (identity v1.5 §7.14):** `HubListRecord.not_after <= OrgEntitlementRecord.license_not_after` (Section 21.6). RA, lisans bitmişse HubListRecord yenilemez (hub bootstrap bilgisi lisans süresini aşamaz).

---

## 21.6 OrgEntitlementRecord (Lisans Kaydı)

> **Mimari bağlam (Haziran 2026, identity v1.5 §7.14 / CAD-51; signer netleştirme):** Business model'i kriptografiye gömen lisans kaydı. **RoftCore License Authority (cert_type 0x0A, EKU `id-kama-license-authority`) tarafından imzalanır** — Root CA değil (Root offline kalır), Intermediate CA değil (Intermediate kurum namespace omurgası; lisans global business authority'dir). License Authority cert'i Root tarafından ceremony ile imzalanır; online HSM-backed sign-only; **YALNIZCA OrgEntitlementRecord imzalama yetkisi vardır** (Identity/Device/Hub/RA/AHS/DiscoveryEndpoint imzalayamaz — pki-ca-ra-hsm-tdd.md §4). RA, sertifika basmadan önce bu kaydı doğrular (license gate, identity v1.5 §15.11). **Intermediate CA ömrünü bozmaz** (Intermediate 5 yıl namespace omurgası; lisans Intermediate expiry ile DEĞİL, bu kayıt + Identity Cert 1 yıl yenileme ile enforce edilir — identity v1.5 E-11). Bu kayıt ağda client'a dolaşmaz; RA↔RoftCore ve RA iç doğrulamasında kullanılır (RA gate). Yine de canonical byte formatı interop için burada tanımlıdır (RA implementasyonu Java/Rust/C++ tutarlı doğrulamalı).

### 21.6.1 Wire Format

```
OrgEntitlementRecord {
  magic               : bytes4      # "SROE"
  version             : u8          # 1
  org_id              : bytes16     # kurum kimliği
  intermediate_key_id : bytes16     # hangi Intermediate namespace (bu lisans hangi kuruma)
  seat_limit          : u32_be      # maksimum aktif kullanıcı (RA enforce eder)
  license_not_before  : u64_be      # lisans başlangıcı (unix saniye)
  license_not_after   : u64_be      # lisans sonu (genelde +1 yıl)
  allowed_features    : varbytes16   # feature bitset (opaque; max 256B; RA/business docs)
  record_sequence     : u64_be      # monoton (lisans güncelleme/yenileme; LWW)
  issuer_key_id       : bytes16      # License Authority subject_key_id[0:16] — hangi LA key imzaladı (key rotation determinism)
  signature           : bytes64     # Ed25519_Sign(sk_license_authority, canonical_entitlement_tbs)
}
```

### 21.6.2 canonical_entitlement_tbs

```
canonical_entitlement_tbs =
  "SROE" (magic, 4B) || version || org_id || intermediate_key_id ||
  seat_limit (u32_be) || license_not_before (u64_be) || license_not_after (u64_be) ||
  allowed_features (varbytes16: len || bytes) || record_sequence (u64_be) || issuer_key_id (16B)

signature = Ed25519_Sign(sk_license_authority, canonical_entitlement_tbs)
```
`signature` TBS'e **dahil değildir** (ama `issuer_key_id` TBS'e DAHİLDİR → hangi License Authority key'inin imzaladığı signed context'e bind edilir; key rotation / çoklu License Authority durumunda belirsizlik elenir). Magic-in-TBS disiplini (Section 1) geçerli. İmzalayan = **RoftCore License Authority** (cert_type 0x0A, EKU `id-kama-license-authority`; Root-signed, online HSM sign-only). `issuer_key_id` = License Authority CCR `subject_key_id[0:16]`. Bu key Root'tan ve Intermediate'ten bağımsızdır; key_id RA'da pinli. Lisans otoritesi RoftCore'da, Identity/Device/Hub namespace'inden ayrıdır.

### 21.6.3 Validation (RA tarafı, identity v1.5 §15.11 apply_license_gate)

```
verify_org_entitlement_record(oer, ROOT_CA_SPKI_HASH):
  1. magic == "SROE", version == 1, exact parse, trailing byte yok.
  2. issuer_key_id ile License Authority CCR çöz (RA'da pinli/cache).
  3. License Authority CCR doğrula (Section 17.5):
       cert_type == 0x0A
       EKU id-kama-license-authority (0x0007)
       key_usage == digitalSignature only (keyCertSign/cRLSign/keyAgreement varsa reject)
       is_ca == FALSE
       chain → Root, ROOT_CA_SPKI_HASH pinli (recursive verify_chain)
       revoked değil, validity içinde
  4. Ed25519_Verify(license_authority.public_key, canonical_entitlement_tbs, signature)
  5. intermediate_key_id, sertifika isteğinin org'unun Intermediate'i ile eşleşir
  6. license_not_before <= now <= license_not_after (lisans aktif mi)
  7. seat_limit: RA aktif cert sayısını sayar; >= seat_limit ise yeni Identity Cert REDDEDİLİR
  8. record_sequence: LWW (daha yüksek sequence öncekini geçersiz kılar; lisans yenileme)
  9. allowed_features parser limitleri + canonical encoding doğrulanır

FORBIDDEN (yapılırsa kritik authorization bug):
  - Root public key ile OrgEntitlementRecord imzasını doğrudan kabul etme.
  - Intermediate public key ile OrgEntitlementRecord imzasını kabul etme.
  - RA Identity public key ile OrgEntitlementRecord imzasını kabul etme.
  - License Authority cert EKU'suzsa veya keyCertSign taşıyorsa kabul etme.

Issuance lifetime clamp (RA, cert basarken uygular):
  IdentityCert.not_after  <= license_not_after
  DeviceCert.not_after    <= min(identity_cert.not_after, license_not_after)
  HubListRecord.not_after  <= license_not_after
```

> **Lisans yaşam döngüsü (identity v1.5 §7.14):** Lisans yenilenince RoftCore yeni OrgEntitlementRecord imzalar (yeni `license_not_after`, `record_sequence++`). RA yeni Identity Cert'leri yeni `license_not_after`'a kadar verir. Lisans yenilenmezse: mevcut Identity Cert'ler 1 yıl içinde expire → Device Cert'ler parent'ı aşamaz → routing kademeli durur. **Intermediate (5 yıl) etkilenmez** — yeni lisans alınınca aynı Intermediate namespace altında devam (CA rollover gerekmez). Lisans enforcement Intermediate expiry ile DEĞİL (identity v1.5 CAD-52).

---

## 21.7 DiscoveryEndpointRecord (Discovery Server Endpoint + OPRF Public Key Dağıtımı)

> **Mimari bağlam (Haziran 2026):** Discovery Server adresleri ve VOPRF public key'i (`pk_oprf`) client'a **imzalı** olarak dağıtılır — serbest/imzasız config değeri DEĞİL. Mekanizma HubListRecord (§21.5) ile **birebir aynı disiplindedir**: kaydı **kurumun Admin Server'ı hazırlar, RoftCore Intermediate CA otomatik imzalar** (server-to-server). Client Root'a pinli olduğu için `DiscoveryEndpointRecord`'u Intermediate imzasından Root'a kadar zincirle doğrular. `pk_oprf` imzaya bind edilir → kötü hub/MITM sahte OPRF public key (veya sahte endpoint) enjekte edemez (VOPRF verifiable mode proof doğrulaması bu `pk_oprf`'a dayanır). VOPRF mekaniği `discovery-architecture.md`'ye aittir; bu doküman yalnızca canonical byte layout'u ve TBS'i tanımlar (interop için).

### 21.7.0 Suite Constants

```
VOPRF suite ids:
  0x0001 = ristretto255-SHA512 VOPRF mode (verifiable)

Discovery transport suite ids:
  0x0001 = Noise_NK_25519_ChaChaPoly_BLAKE2s
  Kurallar: Alpha yalnız 0x0001 kabul eder; bilinmeyen transport_suite_id → reject;
            plaintext discovery transport'a downgrade → reject.
```

### 21.7.1 Wire Format

```
DiscoveryEndpointRecord {
  magic               : bytes4      # "SRDE"
  version             : u8          # 1
  operator_id         : bytes16     # org/operator scope (Intermediate namespace)
  issuer_key_id       : bytes16     # imzalayan Intermediate CA (cache lookup; HubListRecord disiplini)
  endpoint_count      : u8          # 1..8 (Section 6.7 max DISCOVERY_ENDPOINT_COUNT = 8)
  endpoints           : DiscoveryEndpointEntry[endpoint_count]
  pk_oprf                        : bytes32     # VOPRF public key (ristretto255-SHA512); imzaya bind
  voprf_suite_id                 : u16_be      # 0x0001 = ristretto255-SHA512 VOPRF mode
  discovery_transport_x25519_pub : bytes32     # Discovery Server static E2E transport key; imzaya bind
  transport_suite_id             : u16_be      # 0x0001 = Noise_NK_25519_ChaChaPoly_BLAKE2s
  sequence_number     : u64_be      # monoton (endpoint/key rollback anti-stale; LWW)
  not_before          : u64_be      # unix saniye
  not_after           : u64_be      # unix saniye; not_after <= OrgEntitlementRecord.license_not_after
  signature           : bytes64     # Ed25519_Sign(sk_intermediate, canonical_discovery_endpoint_record_tbs)
}

DiscoveryEndpointEntry {
  transport_type      : u8          # 0x01=https, 0x02=kama-hub-proxy
  endpoint_len        : u16_be      # 1..512 (Section 6.7 max DISCOVERY_ENDPOINT_LEN = 512)
  endpoint            : bytes[endpoint_len]   # ASCII/UTF-8 canonical endpoint string (URL veya hub-proxy hedefi)
  priority            : u8          # 0=primary tercih, artan=fallback
}
```

### 21.7.2 canonical_discovery_endpoint_record_tbs

```
canonical_discovery_endpoint_record_tbs =
  "SRDE" (magic, 4B) || version || operator_id || issuer_key_id ||
  endpoint_count ||
  (her DiscoveryEndpointEntry: transport_type || endpoint_len || endpoint || priority) ||
  pk_oprf || voprf_suite_id ||
  discovery_transport_x25519_pub || transport_suite_id ||
  sequence_number || not_before || not_after

signature = Ed25519_Sign(sk_intermediate, canonical_discovery_endpoint_record_tbs)
```
`signature` TBS'e **dahil değildir**. Magic-in-TBS disiplini (Section 1) geçerli — `"SRDE"` ilk alandır.

> **NORMATİF:** Hem `pk_oprf` hem `discovery_transport_x25519_pub` Intermediate tarafından imzalanır. Kötü hub/MITM ne VOPRF verification key'ini ne de Discovery Server E2E transport key'ini ikame edemez. İki ayrı suite alanı: `voprf_suite_id` = VOPRF algoritması; `transport_suite_id` = Client↔Discovery Server E2E channel algoritması.

### 21.7.3 Validation — verify_discovery_endpoint_record

```
- magic == "SRDE", version == 1
- endpoint_count ∈ [1, 8]  (0 veya >8 → reject); endpoints sayısı == endpoint_count (mismatch → reject)
- her endpoint: endpoint_len ∈ [1, 512] (0 veya >512 → reject); transport_type ∈ {0x01, 0x02} (bilinmeyen → reject)
- issuer_key_id ile Intermediate CA cache'ten bulunur
- Ed25519_Verify(intermediate_pub, canonical_discovery_endpoint_record_tbs, signature)
- Intermediate chain → Root (pinned ROOT_CA_SPKI_HASH) — client Root'a pinli
- operator_id, Intermediate'in namespace'i ile tutarlı (cross-operator endpoint reject)
- not_before <= now <= not_after  (henüz geçerli değil / expired → reject)
- not_after <= OrgEntitlementRecord.license_not_after  (lisans clamp; aşıyorsa reject)
- sequence_number: LWW rollback koruması (cache'teki highest_seen'den düşük → reject; stale endpoint/key)
- VOPRF suite: voprf_suite_id == 0x0001; pk_oprf geçerli ristretto255 public key encoding; client VOPRF proof doğrulamasında bu pk_oprf'u kullanır
- Discovery transport suite: transport_suite_id == 0x0001; discovery_transport_x25519_pub geçerli X25519 public key;
    all-zero public key → reject; low-order key → reject (library destekliyorsa); Noise NK sırasında all-zero shared secret → reject
- Client davranışı: PUBLISH / VOPRF_BLIND_BATCH / LOOKUP_BATCH'ten ÖNCE client discovery_transport_x25519_pub ile E2E discovery channel kurMALI; kurulamıyorsa abort
- trailing byte yok (record sonrası fazladan byte → reject)
- duplicate/unsupported field → reject (sabit-sıralı canonical; bilinmeyen alan yok)
```

> **Güvenlik:** `pk_oprf` imzaya bind olduğu için sahte VOPRF public key enjekte edilemez (verifiable mode proof bu key'e dayanır → yanlış key proof'u client'ta VerifyProof ile yakalanır). `sequence_number` eski endpoint/OPRF key rotation'da rollback'i engeller (saldırgan eski `pk_oprf`'lu kaydı replay edemez). `voprf_suite_id`/`transport_suite_id` sabitlemesi suite-downgrade'i engeller; `discovery_transport_x25519_pub` da imzaya bind olduğundan kötü hub Discovery Server E2E transport key'ini ikame edemez. Client, Discovery Server'a **herhangi bir sorgu yapmadan önce** bu kaydı doğrular (`discovery-architecture.md` bootstrap/refresh akışı).

> **Lisans clamp (identity v1.5 §7.14):** `DiscoveryEndpointRecord.not_after <= OrgEntitlementRecord.license_not_after`. Lisans bitmişse RA/Admin yeni DiscoveryEndpointRecord vermez → discovery endpoint bilgisi lisans süresini aşamaz (HubListRecord ile aynı disiplin).

### 21.7.4 Client ↔ Discovery Server E2E Channel (NORMATİF)

DiscoveryEndpointRecord, Discovery Server'ın static transport public key'ini imzaya bind eder. Client, **güvenilmez hub proxy üzerinden** bir E2E discovery channel kurmak için `discovery_transport_x25519_pub`'ı kullanır.

```
Korunan RPC'ler: PUBLISH, VOPRF_BLIND_BATCH, LOOKUP_BATCH
Hub bu channel'ı TERMINATE EDEMEZ. Hub-visible plaintext discovery RPC YASAK.

Hub görebilir:   paket timing, paket boyutu, client'ın bir Discovery Server endpoint'iyle temas ettiği
Hub GÖREMEZ:     blinded VOPRF points, lookup_key değerleri, LOOKUP response entries,
                 DiscoveryRecord/CompactDeviceCert response içeriği, PUBLISH payload içeriği
```

> **Hub gizliliği VOPRF'e değil, E2E discovery channel'a bağlıdır:** VOPRF blinding yalnızca raw telefon numaralarını blind eval sırasında gizler; lookup_key'lerin ve response'un hub'dan gizlenmesi E2E channel ile sağlanır.

### 21.7.5 Version Kararı (Alpha)

```
DiscoveryEndpointRecord version = 1 KALIR.
v1 wire layout freeze öncesi güncellendi; deployed compatibility YOK (pre-freeze layout).
Implementer "eski v1 mi yeni v1 mi" kararsızlığı yaşamaz: tek geçerli v1 = bu layout
(pk_oprf + voprf_suite_id + discovery_transport_x25519_pub + transport_suite_id).
```

### 21.7.6 Test Corpus

```
VALID:
  DiscoveryEndpointRecord v1: pk_oprf + voprf_suite_id=0x0001 + discovery_transport_x25519_pub +
    transport_suite_id=0x0001 + geçerli Intermediate signature + geçerli sequence_number + geçerli not_before/not_after.

INVALID (reject):
  - discovery_transport_x25519_pub eksik
  - all-zero discovery_transport_x25519_pub
  - low-order discovery_transport_x25519_pub (tespit edilebiliyorsa)
  - bilinmeyen transport_suite_id
  - voprf_suite_id != 0x0001
  - transport key signed TBS'e dahil değil → signature mismatch
  - endpoint_count == 0 / > 8
  - endpoint_len == 0 / > 512
  - stale sequence_number
  - not_after > license_not_after
```

### 21.7.7 Acceptance Criteria — DiscoveryEndpointRecord

```
AC-SRDE-1: DiscoveryEndpointRecord hem pk_oprf hem discovery_transport_x25519_pub'ı imzalar.
AC-SRDE-2: Client pk_oprf'u yalnız geçerli Intermediate-imzalı DiscoveryEndpointRecord'dan alır.
AC-SRDE-3: Client discovery_transport_x25519_pub'u yalnız geçerli Intermediate-imzalı DiscoveryEndpointRecord'dan alır.
AC-SRDE-4: Client PUBLISH/VOPRF_BLIND_BATCH/LOOKUP_BATCH'ten ÖNCE E2E discovery channel kurMALI.
AC-SRDE-5: Hub-visible plaintext discovery RPC YASAK.
AC-SRDE-6: Bilinmeyen voprf_suite_id veya transport_suite_id reddedilir.
AC-SRDE-7: All-zero / geçersiz X25519 transport public key reddedilir.
AC-SRDE-8: DiscoveryEndpointRecord sequence_number endpoint/key rollback'i engeller.
```

---

## 21.8 BootstrapBundle (Cert-Bound Onboarding Kökü — KAT5)

> **Mimari bağlam (KAT5-DECISION-4/5):** Onboarding'de client'a dağıtılan tüm client-facing transport static key'ler (hub Noise, RA Noise) **raw endpoint metadata'sından değil, cert-bound CCR zincirinden** türetilir. BootstrapBundle; Root anchor (app-bundle pin ile uyumlu), HubListRecord (§21.5, cert-bound HubEntryV3) ve RA sunucularını (RAServerEntry, cert-bound) tek imzalı zarfta taşır. Alpha canonical bootstrap güvenlik modeli **full cert-bound**'dur; compression / cert-ref / cached-chain optimizasyonu Beta.

### 21.8.1 Wire Format

```
BootstrapBundle {
  magic                : bytes4      # "SRBB"
  version              : u8          # 1
  operator_id          : bytes16     # kurum namespace (Intermediate)
  root_ca_cert         : varbytes16  # Root anchor mirror (app-bundle ROOT_CA_SPKI_HASH pin ile eşleşmeli)
  hublist_record       : varbytes32  # gömülü HubListRecord (SRHL, §21.5; kendi Intermediate imzasını taşır)
  ra_server_count      : u8          # 1..N
  ra_servers           : RAServerEntry[ra_server_count]
  issued_at            : u64_be
  not_after            : u64_be      # <= OrgEntitlementRecord.license_not_after (clamp)
  signature            : bytes64     # Ed25519_Sign(sk_intermediate, canonical_bootstrap_bundle_tbs)
}

RAServerEntry {
  ra_server_id         : bytes16
  address_count        : u8
  addresses            : HubAddress[address_count]   # AddrEntry ≡ HubAddress (§21.5.1)
  ra_identity_cert     : varbytes16  # CCR envelope, cert_type=0x07 RA Identity (Ed25519, EKU id-kama-ra)
  ra_noise_cert        : varbytes16  # CCR envelope, cert_type=0x08 RA Noise (X25519 keyAgreement, issuer=RA Identity)
}
# Türetilen (raw alan TAŞINMAZ):
#   ra_ed25519_pub      = parse(ra_identity_cert).public_key
#   ra_noise_x25519_pub = parse(ra_noise_cert).public_key    # client→RA Noise NK/IK responder static (rs)
```

### 21.8.2 canonical_bootstrap_bundle_tbs

```
canonical_bootstrap_bundle_tbs =
  "SRBB" || version || operator_id ||
  varbytes16(root_ca_cert) ||
  varbytes32(hublist_record) ||
  ra_server_count || (her RAServerEntry:
      ra_server_id ||
      address_count || (her HubAddress: ip_version || ip_bytes || port) ||
      varbytes16(ra_identity_cert) ||
      varbytes16(ra_noise_cert)) ||
  issued_at || not_after

signature = Ed25519_Sign(sk_intermediate, canonical_bootstrap_bundle_tbs)
```
`signature` TBS'e dahil değildir. Magic-in-TBS disiplini (Section 1) geçerli. İmzalayan = kurumun Intermediate CA'sı (HubListRecord ile aynı disiplin, §21.5.2).

### 21.8.3 validate_ra_server_entry (CERT-BOUND, KAT5)

```
validate_ra_server_entry(entry, ROOT_CA_SPKI_HASH, now):
  1.  ra_identity_cert.cert_type == 0x07 (RA_IDENTITY_CERT).
  2.  ra_identity_cert.public_key_alg == Ed25519.
  3.  ra_identity_cert EKU id-kama-ra içerir.
  4.  verify_chain(ra_identity_cert, ROOT_CA_SPKI_HASH): RA Identity → Intermediate → Root pin.
  5.  ra_noise_cert.cert_type == 0x08 (RA_NOISE_CERT).
  6.  ra_noise_cert.public_key_alg == X25519.
  7.  ra_noise_cert.key_usage == keyAgreement.
  8.  ra_noise_cert.issuer_key_id == ra_identity_cert.subject_key_id.
  9.  Ed25519_Verify(ra_identity_cert.public_key, ra_noise_cert.tbs, ra_noise_cert.signature).
  10. ra_noise_cert.public_key all-zero / low-order point DEĞİL → reject.
  11. entry.addresses SAN/policy ile uyumlu.
  → ra_noise_x25519_pub = ra_noise_cert.public_key   # client→RA Noise rs (§16.2 ile tutarlı)
```

### 21.8.4 BootstrapBundle Validation

```
validate_bootstrap_bundle(bundle, ROOT_CA_SPKI_HASH, now):
  - magic == "SRBB", version == 1, parse exact, trailing byte YOK.
  - root_ca_cert SHA-256(spki) == app-bundle ROOT_CA_SPKI_HASH (pin eşleşmezse → reject).
  - hublist_record: §21.5.3 doğrula (+ her HubEntryV3 → §21.5.4).
  - her RAServerEntry: validate_ra_server_entry (§21.8.3).
  - Ed25519_Verify(intermediate_pub, canonical_bootstrap_bundle_tbs, signature) + Intermediate → Root pin.
  - now <= not_after; not_after <= OrgEntitlementRecord.license_not_after (clamp).
```

> **Not (bootstrap-direct vs bundle):** İlk onboarding'de client RA'ya well-known endpoint üzerinden DOĞRUDAN bağlanır (henüz bundle yok, pki §14); bu ilk temas da pinned Root → RA Identity → RA Noise Cert ile kriptografik doğrulanır (RA Noise Cert `rs`, §16.2). BootstrapBundle, onboarding SONRASI RA temaslarını (Client→Hub→RA proxy) ve failover'ı cert-authenticated yapar; RA Noise / hub Noise key'leri asla doğrulanmamış endpoint metadata'sından alınmaz.

### 21.8.5 Acceptance Criteria — BootstrapBundle / RAServerEntry

```
AC-CCR-RAENTRY-1: RAServerEntry, RA Identity Cert ve RA Noise Cert taşır.
AC-CCR-RAENTRY-2: Client RA Noise key'ini yalnız RA Noise Cert.public_key'den çıkarır (endpoint metadata'dan DEĞİL).
AC-CCR-SRBB-1:    BootstrapBundle Intermediate-imzalı + Root'a zincir; root_ca_cert pin ROOT_CA_SPKI_HASH ile eşleşmeli.
AC-CCR-SRBB-2:    Gömülü HubListRecord ve her RAServerEntry cert-bound doğrulanır; raw-key kabul YOK.
```

---



> Bu bölüm doğrulama **sırasını** tanımlar. Format-spec sahipliğindeki adımlar `[FMT]`, identity validation engine sahipliğindekiler `[ENG]` ile işaretlidir.

### 22.1 Device Cert Chain

```
1. [FMT] CCR envelope parse (magic/version/cert_type/tbs_len/sig_alg)
2. [FMT] canonical_ccr_tbs yeniden inşa (tbs alanı = imzalanan byte'lar)
3. [ENG] Ed25519_Verify(issuer_public_key, canonical_ccr_tbs, signature)
4. [ENG] EKU / key_usage / basic_constraints kontrol
5. [ENG] nameConstraints kontrol (subject SAN, issuer constraints içinde)
6. [ENG] validity (not_before <= now <= not_after)
7. [ENG] chain → pinned ROOT_CA_SPKI_HASH (bounded-iterative, MAX_CHAIN_DEPTH=5, cycle detection; v1.5 Section 15.2)
8. [ENG] CRLite (revocation-model.md — bu doküman dışı)
```

> **Chain-walking DoS sertleştirmesi:** Chain doğrulama bounded ve cycle-safe'tir (v1.5 Section 15.2 ile birebir): `MAX_CHAIN_DEPTH = 5` (normal chain en fazla 4 sertifika: Device/HubNoise/RANoise → ... → Root; bir güvenli margin). Tekrarlanan `subject_key_id` → cycle → reject. Bu, malformed/döngüsel issuer zincirinin stack exhaustion / DoS yaratmasını engeller. Parser ayrıca length-bounded'dır (Section 7, 29).

> **CompactDeviceCertV1 chain (NORMATİF):** Yukarıdaki generic CCR chain adımları Root/Intermediate/Identity/Hub/RA/CT cert'leri içindir (envelope'lu CCR). **CompactDeviceCertV1 ayrı doğrulanır** (Section 13.8 verify_compact_device_cert): 152B sabit, SAN yok (namespace issuer Identity nameConstraints ile), serial yok (revocation device_cert_hash ile), issuer_signature Identity/User Cert key ile (`Ed25519_Verify(identity_cert.user_identity_ed25519_pub, compact_device_cert_tbs, issuer_signature)`). CompactDeviceCert'in issuer'ı (Identity/User Cert) bu generic chain ile Root'a doğrulanır; CompactDeviceCert kendisi generic envelope kullanmaz.

### 22.2 SignedNoisePrekey ⚠️ DEPRECATED / MUST NOT IMPLEMENT (prekey kaldırıldı — Section 18)

> **DEPRECATED:** SignedNoisePrekey doğrulaması artık yok (prekey nesnesi statik X25519 ile kaldırıldı). Yerine **Session Model V3** (Section 35) geçer: normal hot-path İMZASIZ (authenticated one-way key schedule, ss_s possession). `sender_signature` yalnızca SIGNED_CONTROL_MESSAGE'da (Section 35.9): `Ed25519_Verify(pk_device_ed25519, signed_control_tbs, sig)`. `device_x25519_pub` CompactDeviceCert'ten doğrulanır (issuer_signature → Identity/User Cert → Intermediate → Root). Statik X25519 ayrı bir imzalı prekey gerektirmez — Device CCR imzası onu zaten korur.

### 22.3 İmza Girdisi Kesin Kuralı

Her imzalı nesne için:
- `signature` kendi TBS'ine **dahil değildir**.
- TBS byte dizisi deterministiktir (fixed-width big-endian, sabit alan sırası).
- Doğrulayan public key ve onun trust path'i tanımlıdır:

| Nesne | İmzalayan key | Trust path |
|-------|---------------|------------|
| Root CA CCR | root_sk (self) | pinned ROOT_CA_SPKI_HASH |
| Intermediate CCR | root_sk | Root CA |
| Identity CA CCR | intermediate_sk | Intermediate → Root |
| Device CCR | sk_user_identity | Identity CA → Intermediate → Root |
| Hub Identity CCR | intermediate_sk | Intermediate → Root |
| Hub Noise CCR | hub_identity_sk | Hub Identity → Intermediate → Root |
| RA Identity CCR | intermediate_sk | Intermediate → Root |
| RA Noise CCR | ra_identity_sk | RA Identity → Intermediate → Root |
| CT Log Signing CCR | root_sk veya intermediate_sk | Root/Intermediate → Root |
| License Authority CCR (Section 17.5) | root_sk | Root → pinned ROOT_CA_SPKI_HASH |
| Admin Server CCR (Section 17.6) | intermediate_sk | per-org Intermediate → Root (mTLS peer kimliği; record imzalamaz) |
| OrgEntitlementRecord (Section 21.6) | sk_license_authority | License Authority → Root (Intermediate/Root DEĞİL) |
| HubListRecord (Section 21.5) | sk_intermediate | Intermediate → Root |
| AuthorizedHubSetRecord (SRAH) | sk_intermediate | Intermediate → Root (RA gate) |
| RevocationRecord / RevocationPush | sk_ra_identity | RA Identity → Intermediate → Root |
| RevocationSnapshot / RevocationDelta | sk_intermediate | Intermediate → Root |
| SIGNED_CONTROL_MESSAGE (Section 35.9) | sk_device | Device CCR → Identity CA → Intermediate → Root |
| Session Model V3 hot-path (Section 35) | İMZASIZ (ss_s possession) | AEAD-auth key schedule |
| DiscoveryRecord | sk_user_identity | user_identity_ed25519_pub (== Identity CA public key) |
| DiscoveryEndpointRecord (Section 21.7) | sk_intermediate | Intermediate → Root (Admin Server hazırlar, Intermediate imzalar) |

> **Revocation signer ayrımı (NORMATİF):** RA Identity → RevocationRecord/RevocationPush; Intermediate → RevocationSnapshot/Delta checkpoint; **License Authority → revocation object İMZALAMAZ** (yalnız OrgEntitlementRecord).

---

## 23. Hashing Rules

Tüm hash'ler SHA-256; girdi byte'ları kesin tanımlı:

```
ROOT_CA_SPKI_HASH = SHA-256(spki_encoding(0x01, root_public_key))
                  = SHA-256(0x01 || root_public_key)             # 33 byte girdi

subject_key_id    = SHA-256(spki_encoding(subject_alg, subject_pub))
issuer_key_id     = SHA-256(spki_encoding(issuer_alg, issuer_pub))   # generic CCR (32B)

identity_hash     = SHA-256(user_identity_ed25519_pub)           # 32 byte ham key (user identity)
dest_hash         = SHA-256(identity_hash)                        # 32 byte girdi
routing_key       = dest_hash[0:16]                              # truncation, hash değil

device_cert_hash  = SHA-256(canonical_compact_device_cert)       # tam 152B CompactDeviceCertV1
  canonical_compact_device_cert =
    device_ed25519_pub || device_x25519_pub || issuer_key_id || not_after || issuer_signature
```

> **CompactDeviceCertV1 hash (NORMATİF):** `device_cert_hash`, tam 152-byte CompactDeviceCertV1 nesnesi üzerinden hesaplanır. **Generic CCR envelope üzerinden DEĞİL**, **Generic Device CCR üzerinden DEĞİL**, **yalnızca-TBS nesnesi üzerinden DEĞİL**. `issuer_signature`'ı İÇERİR (152B'nin son 64B'si). = revocation_id (Section 13.6).

> **Generic CCR vs CompactDeviceCertV1 hash ayrımı (NORMATİF):**
> ```
> Generic CCR object hash (Root/Intermediate/Identity/Hub/RA/CT/audit):
>   SHA-256(canonical_ccr_envelope)   # ayrı tanımlı (envelope: magic||version||cert_type||tbs_len||tbs||sig_alg||signature)
> CompactDeviceCertV1 hash (aktif device cert):
>   device_cert_hash = SHA-256(canonical_compact_device_cert)   # tam 152 byte; envelope DEĞİL
> ```

> **Kritik ayrım (tekrar):** `identity_hash` ham public key (32 byte) üzerinden; `subject_key_id`/`ROOT_CA_SPKI_HASH` spki_encoding (33 byte: alg||key) üzerinden. Asla karıştırılmaz. `identity_hash` yalnızca `user_identity_ed25519_pub`'tan (device key'den DEĞİL). Belirsiz `hash(cert)` ifadesi **yasak** — her zaman kesin girdi (`SHA-256(canonical_compact_device_cert)` device için, `SHA-256(canonical_ccr_envelope)` generic CCR için).

---

## 24. Name Constraints Encoding

### 24.1 Name

```
Name = varbytes16    # u16_be len || UTF-8 bytes, max 256
```
İmzalı TBS'te Name byte'ları ham karşılaştırılır (Unicode normalizasyonu yok). Boş Name yalnızca açıkça izinli tiplerde (Root self-name hariç değil — Root için name zorunlu).

### 24.2 NameConstraints Extension (ext_id 0x0001)

```
NameConstraints {
  entry_count    : u16_be      # max 8
  entries        : NameConstraintEntry[entry_count]
}
NameConstraintEntry {
  type           : u8          # 0x01 URI, 0x02 DNS, 0x03 IP
  value          : varbytes16  # max 256
}
```

**URI dest_hash encoding:** `kama://<dest_hash_hex>/*` — `<dest_hash_hex>` = dest_hash'in **lowercase base16** (64 ASCII hex karakter). Validation: URI'den çıkarılan dest_hash, `SHA-256(SHA-256(subject_public_key))` ile eşleşmeli (Identity CA için).

**DNS/IP:** ASCII; IP IPv4 (4 byte) veya IPv6 (16 byte) için kanonik string formu. entries ascending `type` sonra `value` byte sırasıyla; duplicate → reject.

> **Sıralama netleştirmesi (NORMATİF — interop kritik):** "ascending byte sırası" = **bytewise unsigned lexicographic** karşılaştırma (her byte 0x00-0xFF unsigned). Java `byte` signed olduğu için implementasyon `Byte.toUnsignedInt` veya `& 0xFF` kullanmalı; Rust `[u8]::cmp` zaten unsigned. **Case-folding YOKTUR** — ham byte karşılaştırılır. Bu yüzden URI/hex değerlerinin canonical case'i sabit olmalı (aşağıya bakınız), aksi halde `aaaa` vs `AAaa` farklı sıralanır ve Java-üretilen cert Rust'ta (veya tersi) "ascending değil" diye reject edilir (interop-DoS).

---

## 25. EKU / Key Usage Encoding

```
key_usage  : u16_be bitmask (Section 6.4)
eku_count  : u8 (max 8)
eku_list   : u16_be[eku_count]    # ascending, duplicate yok (Section 6.5)
```

**Validation:**
- key_usage reserved bitler (5–15) set ise → reject.
- CA:FALSE cert keyCertSign(0x0002) taşırsa → reject.
- eku_list ascending değilse veya duplicate → reject.
- cert_type ile EKU/key_usage uyumsuzsa (Section 9 tablosu) → reject.

---

## 26. Validity Time Encoding

```
not_before : unix_time_u64    # u64_be, Unix epoch saniye, UTC
not_after  : unix_time_u64
```

**Validation:**
- `not_after > not_before` (eşit veya küçük → reject).
- `(not_after - not_before) <= cert_type max validity` (Section 6.8) → aşılırsa reject.
- 0 değeri → reject (her iki alan da).
- Clock skew toleransı validation engine'de (bu doküman ham byte'ı tanımlar).

---

## 27. Serial Number Encoding

```
serial : bytes16    # 128-bit keyspace (2^128), CSPRNG ile üretilir
```

**Validation:**
- 16 byte sabit.
- All-zero serial → reject (üretici CSPRNG kullanmalı).
- CRLite serial eşleşmesi bu 16 byte üzerinden (revocation-model.md).

**Collision profili (16-byte identifier disiplini):** 16 byte = **128-bit keyspace (2^128)**. Rastgele any-collision birthday bound ≈ **2^64** iş (çok sayıda serial üretilirse %50 çakışma); belirli bir hedefe **targeted** collision/preimage ≈ **2^128** iş. CSPRNG üretimi any-collision olasılığını pratikte ihmal edilebilir kılar. Aynı disiplin `routing_key` (16 byte) için de geçerlidir (identity-and-trust-model v1.5 Section 5.1).

---

## 28. Extension Encoding

```
Extension {
  ext_id      : u16_be      # Section 6.6
  critical    : bool_u8
  value_len   : u16_be      # max 4096
  value       : bytes       # tam value_len byte
}
```

**Kurallar:**
- Extensions `ext_id` **ascending** sıralı (TBS içinde).
- Duplicate `ext_id` → reject.
- Bilinmeyen **critical** extension → reject.
- Bilinmeyen **non-critical** extension → ignore (ama TBS byte'larında korunur — imza bütünlüğü için byte'lar aynen kalır).
- `value_len > 4096` → reject.
- `extension_count > 16` → reject.

**Built-in extension value formatları:**

| ext_id | İsim | value formatı |
|--------|------|---------------|
| 0x0001 | nameConstraints | Section 24.2 |
| 0x0002 | subjectAltName | SAN entries (NameConstraintEntry ile aynı tip yapısı) |
| 0x0003 | deviceId | bytes16 (tam 16 byte) |
| 0x0004 | ctLogMetadata | opaque (CT docs) |
| 0x0005 | kamaNamespace | opaque (reserved) |

---

## 29. Parser Rejection Rules

Parser aşağıdaki durumlarda **reddeder** (panic değil — temiz error):

```
- desteklenmeyen magic / version / cert_type / sig_alg / public_key_alg
- tbs_len mismatch (gerçek TBS uzunluğuyla uyuşmuyor)
- trailing bytes (nesne sonrası artık byte)
- duplicate extension (aynı ext_id)
- unknown critical extension
- unknown cert_type
- geçersiz enum değeri (bool 0x00/0x01 dışı, vb.)
- non-canonical integer (yanlış genişlik)
- varint tespit edilirse (yapısal olarak beklenmeyen byte deseni)
- little-endian encode edilmiş integer (cross-check ile yakalanan)
- ASCII gereken yerde non-ASCII byte (usage_context, EKU sabitleri)
- not_after <= not_before
- cert_type için geçersiz path_len
- CA:FALSE cert keyCertSign taşıyor
- X25519 key Ed25519 gereken yerde (veya tersi)
- DiscoveryRecord current_device_x25519 != referans CompactDeviceCert device_x25519_pub (binding fail)
- DiscoveryRecord dest_hash/routing_key mismatch (türetim hatası)
- SESSION_INIT session_context_v1 profile/suite tutarsız (downgrade; Section 35.7)
- SESSION_INIT recipient_dest_hash != my_dest_hash (recipient confusion)
- OrgEntitlementRecord signature RoftCore key ile doğrulanamıyor
- HubListRecord not_after > OrgEntitlementRecord.license_not_after (clamp ihlali)
- DeviceCertBundle hash mismatch
- certificate chain depth MAX_CHAIN_DEPTH (=5) aşıyor
- issuer chain cycle içeriyor / tekrarlanan subject_key_id
- signature alanı TBS'e dahil edilmiş (yapısal hata)
- field count mismatch (eku_count/extension_count gerçekle uyuşmuyor)
- extension_count > 16, eku_count > 8
- herhangi max size aşımı (Section 6.7)
- key_usage reserved bit set
- eku_list ascending değil veya duplicate
- all-zero serial
- usage_context uzunluğu != 25 veya içerik mismatch
- URI içinde uppercase/mixed-case hex (dest_hash/device_id lowercase olmalı; PARSER-01)
- nameConstraints entries bytewise-unsigned ascending değil veya duplicate (PARSER-01)
- [DEPRECATED — prekey kaldırıldı] ~~SignedNoisePrekey device_id/device_cert_hash binding~~ → artık AsyncMessage sender_cert_ref ile device_cert_hash binding (Section 35.5b)
- DiscoveryRecord current_device_id mevcut ama current_device_cert_hash eksik (v1.5 15.14)
- CompactDeviceCert length != 152B
- CompactDeviceCert trailing byte var
- CompactDeviceCert issuer_key_id non-Identity cert'e çözülüyor (Identity/User Cert değil)
- CompactDeviceCert issuer_signature, çözülen Identity/User Cert public key altında doğrulanamıyor
- CompactDeviceCert içinde intermediate_sig alanı/ismi (yanlış model; imza Identity/User Cert'ten)
- identity_hash/dest_hash validation path'inin herhangi yerinde device key'den türetilmiş
- DiscoveryRecord current_device_x25519 != CompactDeviceCert.device_x25519_pub
- DiscoveryRecord identity key (user_identity_ed25519_pub) != CompactDeviceCert issuer Identity key
- AsyncMessage sender cert ref çözülemiyor VE inline cert sağlanmamış (hostile cache-miss → fail-closed)
- X25519 public key all-zero (CompactDeviceCert.device_x25519_pub, sender_ephemeral, Hub Noise)
- X25519 DH shared_secret all-zero
- Generic Device CCR'ın aktif wire sender device cert olarak kullanılması (aktif = CompactDeviceCertV1)
```

Parser her zaman **bounded** çalışır: önce length okunur, max'a karşı kontrol edilir, sonra o kadar byte okunur (OOB read imkansız).

---

## 30. Canonicalization Test Vectors

> Gerçek kriptografik imzalar hesaplanmadıysa `TEST_VECTOR_PLACEHOLDER` ile işaretlenir; ama tüm byte alan sırası kesindir. Hex byte'lar big-endian.

### 30.1 SignedNoisePrekey TBS ⚠️ DEPRECATED / MUST NOT IMPLEMENT (prekey kaldırıldı)

> **DEPRECATED:** SignedNoisePrekey test vector'ü artık geçerli değil (prekey nesnesi kaldırıldı, Section 18). Statik X25519 Device CCR'ın `pk_device_x25519` alanında taşınır (Section 13 CompactDeviceCert). Session Model V3 test vektörleri için Section 30.4b (SESSION_INIT/DATA, magic **SSI1/SSD1**)   <!-- [2026-09-07] eski metin "SAI1/SAD1" (deprecated V2 magic) yazıyordu → vektör üreticisini yanlış yönlendiriyordu --> + Section 35.2 key schedule.

### 30.2 Root CA CCR_TBS (örnek alan sırası)

```
magic            : 53 52 43 52                              # "SRCR"
version          : 01
cert_type        : 01                                       # Root CA
serial           : 11 ... 11 (16 byte)
issuer_key_id    : EE ... EE (32 byte, == subject_key_id, self-signed)
subject_key_id   : EE ... EE (32 byte)
issuer_name      : 00 04 52 6F 6F 74                        # len=4 "Root"
subject_name     : 00 04 52 6F 6F 74                        # len=4 "Root"
not_before       : 00 00 00 00 68 00 00 00
not_after        : 00 00 00 00 7E 00 00 00                  # +20 yıl placeholder
public_key_alg   : 01                                       # Ed25519
public_key       : FF ... FF (32 byte root pub)
key_usage        : 00 06                                    # keyCertSign+cRLSign
eku_count        : 00
basic_constraints: 01 00 00                                 # CA:TRUE, pathLen yok
extension_count  : 00 00
---- TBS sonu ----
(envelope: sig_alg=01, signature=Ed25519_Sign(root_sk, TBS))
```

### 30.3 Identity CA CCR_TBS (nameConstraints ile)

```
... (ortak alanlar) ...
cert_type        : 03
public_key_alg   : 01
public_key       : <user_identity_ed25519_pub 32B>
key_usage        : 00 07
eku_count        : 01
eku_list         : 00 02                                    # id-kama-identity
basic_constraints: 01 01 00                                 # CA:TRUE, pathLen=0
extension_count  : 00 01
ext[0]:
  ext_id         : 00 01                                    # nameConstraints
  critical       : 01
  value_len      : 00 4A                                    # 74 byte
  value          : 00 01 00 01 00 46 73 61 66 65 72 6F 6F 6D ...
                   # entry_count=1, type=URI, len=70, "kama://<64hex>/*"
```

### 30.4 CompactDeviceCertV1 Golden Vector (152B) — AĞDA DOLAŞAN TEK CERT (Section 13)

> Bu, eski SAN URI + deviceId extension'lı Device CCR formatının YERİNE geçer. CompactDeviceCert minimal: SAN yok, deviceId cert'te yok, dest_hash türetilir.

```
CompactDeviceCertV1 (152B, byte sırası):
  device_ed25519_pub : <32B>   # device SIGNING key; routing identity ÜRETMEZ
  device_x25519_pub  : <32B>   # statik DH (Session Model V3 recipient static)
  issuer_key_id      : <16B>   # Identity/User Cert subject_key_id[0:16] (cache lookup hint)
  not_after          : 00 00 00 00 6A 1B 2C 00   # u64_be unix saniye (max +90 gün)
  issuer_signature   : <64B>   # Ed25519_Sign(sk_user_identity, compact_device_cert_tbs)  ← Identity/User Cert imzalar
  ────────────────────────
  TOPLAM             : 152B

compact_device_cert_tbs (imzalanan) =
  domain_sep("SR_CDC_V1") || device_ed25519_pub || device_x25519_pub || issuer_key_id || not_after
device_cert_hash = SHA-256(canonical_compact_device_cert)   # tüm 152B; = revocation_id

Doğrulama (Section 13.8):
  issuer Identity/User Cert'i issuer_key_id ile çöz (subject_key_id[0:16] eşleşmeli)
  Ed25519_Verify(identity_cert.user_identity_ed25519_pub, compact_device_cert_tbs, issuer_signature)
    ← INTERMEDIATE key DEĞİL; Identity/User Cert key (KRİTİK)
  chain: Identity/User Cert → Intermediate → Root (pinned)
  routing identity (DEVICE key'den DEĞİL):
    issuer_identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub)
    issuer_dest_hash     = SHA-256(issuer_identity_hash)
  not_after <= identity_cert.not_after; OrgEntitlement context'te ise <= license_not_after
  device_x25519_pub all-zero → reject; X25519 DH all-zero shared secret → reject
  CRLite revocation_id = device_cert_hash (serial DEĞİL)
```

### 30.4b Session Model V3 Test Vektörleri (Section 35)

```
SESSION_INIT_REF (121B app-envelope; magic "SSI1" AAD'de):
  flags              : 00                                   # bits0..1 cert_mode=00 CERT_REF_32;
                                                            # bits2..3 init_variant=00 SR_SM3_AUTHENTICATED_ONE_WAY_DEFAULT;
                                                            # bits4..7 reserved=0000
  recipient_dest_hash: <32B>                                # hub routing (cleartext) + context
  sender_ephemeral   : <32B>                                # e (public)
  sender_cert_ref    : <32B>                                # SHA-256(canonical_compact_device_cert)
  init_counter       : 00 00 00 00 00 00 00 01              # u64_be (epoch 0)
  # [C-10] GEÇERLİLİK: early_data uzunluğu N=0 GEÇERLİDİR (yalnız session kurma amaçlı INIT);
  #        init_counter = 0 GEÇERLİ bir başlangıç değeridir. Her ikisi için de ayrı reject kuralı YOKTUR.
  ciphertext(early)  : <bytes + AEAD tag 16B>
  # sender_signature YOK; init_variant generic Noise pattern selector DEĞİL (Section 35.2 otoriter)

init_root = HKDF("SR-SESSION-INIT-V3", ss_e || ss_s, session_context_v1)
  ss_e = DH(sender_ephemeral_sk, recipient_static_x25519_pub)
  ss_s = DH(sender_static_x25519_sk, recipient_static_x25519_pub)   # sender auth (possession)
session_id_internal = Trunc128(HKDF("SR-SESSION-ID-V3", init_root, session_context_v1))   # wire'da YOK

SESSION_DATA (18B app-envelope; magic "SSD1" AAD'de, wire'da YOK):
  seq16              : 00 01                                # u16_be (resync hint; full counter receiver state)
  ciphertext         : <bytes>
  aead_tag           : <16B>
  # length IMPLICIT (UDP datagram); link_id YOK; cert-ref YOK; imza YOK
  AAD = "SSD1" || version || cipher_suite_id || direction || epoch || seq16 || session_id_internal(16B)
  # transport L2 (wire, AAD'de DEĞİL): hub_handle <4B> (hub routing/demux)

SIGNED_CONTROL (SESSION_DATA + 64B Ed25519; magic "SSC1"):
  seq16 + ciphertext + aead_tag(16B) + sender_ed25519_signature(64B)
  signed_control_tbs = §35.10 OTORİTER (bu bölüm yalnız vektör taşır, tanım TAŞIMAZ):
    "SSC1" || version || sender_device_cert_hash || session_context_hash || direction || epoch
    || reconstructed_full_counter || command_type || application_command_id || SHA-256(ciphertext)
  # [A-4/B-1 düzeltmesi] Buradaki eski kısa biçim (command_type||epoch||seq16||session_context_hash||command_id)
  # counter'ı ve ciphertext'i BAĞLAMIYORDU → imzalı-komut replay'i + ciphertext substitution mümkündü.
  # ⚠ Bu bölümün SIGNED_CONTROL vektörleri §35.10 TBS'i ile YENİDEN ÜRETİLMELİDİR.
```

### 30.4c OrgEntitlementRecord canonical_entitlement_tbs (Section 21.6)

```
canonical_entitlement_tbs (imzalanan):
  magic              : 53 52 4F 45                          # "SROE"
  version            : 01
  org_id             : <16B>
  intermediate_key_id: <16B>
  seat_limit         : 00 00 00 64                          # u32_be = 100
  license_not_before : 00 00 00 00 67 00 00 00              # u64_be
  license_not_after  : 00 00 00 00 68 76 5D 00              # u64_be (+~1 yıl)
  allowed_features   : <len u16_be> <bytes>                 # opaque bitset
  record_sequence    : 00 00 00 00 00 00 00 01              # u64_be = 1
  issuer_key_id      : <16B>                                # License Authority subject_key_id[0:16] (TBS'e dahil)
signature = Ed25519_Sign(sk_license_authority, canonical_entitlement_tbs)   # License Authority key (0x0A), Root/Intermediate DEĞİL

Clamp doğrulama (RA):
  IdentityCert.not_after  <= license_not_after
  DeviceCert.not_after    <= min(identity_cert.not_after, license_not_after)
  HubListRecord.not_after  <= license_not_after
```

### 30.5 PrekeyBundle (örnek) ⚠️ DEPRECATED / MUST NOT IMPLEMENT

> **Deprecated golden vector YASAK (NORMATİF):** Aşağıdaki prekey örneği **aktif golden vector olarak KULLANILMAZ**. Yalnızca historical negative corpus olarak tutulur — parser bu nesneyi aktif wire'da görürse REDDETMELİ (Section 30.7 DEPRECATED-REJECT-1/2). Aktif device cert golden vector = CompactDeviceCertV1 (Section 30.4).

```
magic            : 53 52 50 42                              # "SRPB" (DEPRECATED — aktif wire'da reject)
version          : 01
identity_hash    : <32B>
dest_hash        : <32B == SHA-256(identity_hash)>
routing_key      : <16B == dest_hash[0:16]>
device_id        : <16B>
device_cert_hash : <32B>
signed_prekey_len: 00 E6                                    # 230
# [DEPRECATED: signed_noise_prekey kaldırıldı — DeviceCertBundle artık sadece Device CCR + chain]
record_sequence  : 00 00 00 00 00 00 00 03
record_valid_until: 00 00 00 00 68 76 5D 00                 # <= prekey.not_after
```

### 30.6 DeviceCertBundle (örnek)

```
magic                  : 53 52 44 42                        # "SRDB"
version                : 01
compact_device_cert_v1 : <152B>   # tam CompactDeviceCertV1 (Section 13.1; len-prefix YOK, sabit)
identity_ccr_len       : 00 00 01 40   (örnek)
identity_ccr           : <bytes>       # Identity/User Cert (issuer)
intermediate_ccr_len   : 00 00 00 00                        # cached
intermediate_ccr       : (boş)
device_cert_hash       : <32B == SHA-256(canonical_compact_device_cert)>   # 152B; serial YOK
```

### 30.7 Geçersiz Örnekler (reject edilmeli)

```
# Genel encoding
INVALID-1: unsupported magic/version → reject
INVALID-2: non-canonical length prefix (varint denemesi / yanlış genişlik) → reject
INVALID-3: trailing bytes → reject

# CompactDeviceCertV1 (aktif device cert)
INVALID-4: CompactDeviceCertV1 length != 152B → reject
INVALID-5: device_cert_hash != SHA-256(canonical_compact_device_cert) → reject
INVALID-6: CompactDeviceCertV1 issuer_key_id, Identity/User Cert subject_key_id[0:16]'ya çözülmüyor → reject
INVALID-7: CompactDeviceCertV1 issuer_signature, çözülen Identity/User Cert public key altında doğrulanamıyor → reject
INVALID-8: device_x25519_pub all-zero → reject
INVALID-9: X25519 DH shared_secret all-zero → reject

# DiscoveryRecordV2 binding
INVALID-10: DiscoveryRecordV2.current_device_x25519 != CompactDeviceCertV1.device_x25519_pub → reject
INVALID-11: DiscoveryRecordV2.current_device_cert_hash != SHA-256(canonical_compact_device_cert) → reject
INVALID-12: identity_hash, user_identity_ed25519_pub yerine device_ed25519_pub'tan türetilmiş → reject

# AsyncMessage
INVALID-13: SESSION_INIT sender_cert_ref cache-miss + inline CompactDeviceCertV1/bundle YOK → fail-closed/defer
INVALID-14: SESSION_INIT recipient_dest_hash != local dest_hash → reject
INVALID-15: SIGNED_CONTROL sender_signature, CompactDeviceCertV1.device_ed25519_pub altında doğrulanamıyor → reject (normal SESSION_DATA imzasız — bkz Section 35)
INVALID-16: replay counter sliding window dışında veya duplicate → reject
INVALID-17: CRLite device_cert_hash içeriyor (revoke) → reject

# Generic CCR (Root/Intermediate/Identity/Hub/RA/CT)
INVALID-18: reserved key_usage bit (key_usage = 00 20) → reject
INVALID-19: CA:FALSE + keyCertSign → reject
INVALID-20: chain cycle (tekrarlanan subject_key_id) → reject
INVALID-21: chain depth > MAX_CHAIN_DEPTH (5) → reject (DoS guard)
INVALID-22: nameConstraints uppercase/mixed-case hex (lowercase olmalı, PARSER-01) → reject

# Deprecated negative corpus (parser aktif wire'da reddetmeli)
DEPRECATED-REJECT-1: SignedNoisePrekey ("SRPK") aktif wire'da görünüyor → reject
DEPRECATED-REJECT-2: PrekeyBundle ("SRPB") aktif wire'da görünüyor → reject
```

> **Test corpus disiplini (NORMATİF):** Yukarıdaki INVALID-1..22 aktif parser/validator test suite'idir. DEPRECATED-REJECT-1/2 yalnızca **historical negative corpus**'tur (parser eski prekey nesnelerini aktif wire'da reddetmeli). **Deprecated prekey örnekleri aktif golden vector olarak KULLANILMAZ** (Section 30.5, 31).

---

## 31. Fuzzing Requirements

Rust parser libFuzzer hedefleri (zorunlu):

```
- ccr_envelope_parser
- ccr_tbs_parser
- compact_device_cert_parser (152B fixed; malformed length → reject)
- session_init_parser / session_init_response_parser / session_data_parser / signed_control_parser
- compact_device_cert_parser / device_cert_bundle_parser
- discovery_record_parser / hublist_record_parser / org_entitlement_record_parser
- deprecated_async_v2_reject_parser (SAI1/SAA1/SAD1/SAC1 → reject-only corpus)
- deprecated_async_message_reject_parser (SRAM → reject-only corpus)
  # NORMATİF: deprecated parser'lar yalnızca reject-only corpus üretir; hiçbir production parser
  #          SRAM/SAI1/SAA1/SAD1/SAC1 için ACCEPT döndürmez.
- discovery_record_parser (current_device_x25519 binding)
- hublist_record_parser (IP SAN malformed)
- org_entitlement_parser (RoftCore sig + clamp)
- [DEPRECATED] ~~signed_noise_prekey_parser / prekey_bundle_parser~~ (prekey kaldırıldı — MUST NOT IMPLEMENT)
- device_cert_bundle_parser
- discovery_record_boundary_parser
```

Her fuzz hedefi şunları assert eder:
```
- panic yok
- OOB read/write yok
- integer overflow yok
- malformed length reject
- trailing bytes reject
- valid x için: canonical_reserialize(parse(x)) == x   (round-trip)
- invalid x asla verify olmaz
```

**Differential testler:**
```
- Java encoder → Rust decoder (byte-identik)
- Rust encoder → Java decoder
- C++ transport carrier byte'ları aynen korur (reinterpret yok)
```

---

## 32. Interop Requirements: Java / Rust / C++

```
- Java RA/CA, Rust validator ile birebir aynı canonical CCR byte'larını üretir.
- Java RA/CA, RA→CA internal iletişiminde gRPC over izole mTLS kullanabilir.
- RA→CA Kama custom adaptive transport KULLANMAZ (izole mTLS/gRPC).
- Rust validator Java-üretimi CCR'ı doğrular.
- C++ hub/client CCR/Session Model V3'yi OPAQUE byte olarak taşır; yalnızca Rust validation
  layer çağrılırken yorumlanır.
- JSON/protobuf signature TBS olarak KULLANILMAZ.
- protobuf/gRPC opaque canonical byte'ları SARABILIR (RA→CA veya transport),
  ama içerideki byte'lar bu dokümandaki canonical formattır.
- Base64 yalnızca log/debug; asla canonical input.
```

### 32.1 Katman Sorumluluğu

```
Java (RA/CA):     canonical CCR/OrgEntitlementRecord ÜRETİR + imzalar (BouncyCastle Ed25519)
Rust (validator): canonical parse + VERIFY (memory-safe, libFuzzer hardened)
C++ (hub/client): canonical byte'ları TAŞIR (adaptive transport/Noise), Rust'a delege eder
```

---

## 33. Security Considerations

- **Canonical encoding** imza belirsizliğini önler: tek byte temsili → tek imza.
- **Fixed-size big-endian** parser belirsizliğini azaltır.
- **No varint** hostile-parser attack surface'i azaltır (varint parsing RCE yüzeyi).
- **Signed TBS, signature alanını dışlar** (self-reference imkansız).
- **Domain separation:** her TBS magic (bytes4) + usage_context ile başlar → cross-protocol imza yeniden kullanımı imkansız.
- **DiscoveryRecordV2 tek başına global fresh DEĞİL** (identity v1.5 CAD-42): hub suppress/rollback edebilir; record_sequence cache + multi-source ile mitige (identity engine). Prekey kavramı KALDIRILDI.
- **Device Cert revocation statik X25519 dahil her şeyi geçersiz kılar** (identity v1.5 CAD-40, 3-katman): transport key cert içinde, ayrı revoke yok.
- **CT connection authorize etmez** (CAD-23): CT Log Signing CCR connection kararında kullanılmaz.
- **CRLite enforcement** identity validation engine'de (bu format tek başına revocation yapmaz).
- **Chain-walking DoS direnci**: chain doğrulama bounded (MAX_CHAIN_DEPTH=5) ve cycle-safe (tekrarlanan subject_key_id reject); malformed/döngüsel issuer zinciri stack exhaustion yaratamaz (Section 22.1).
- **Root CA online network path'i yok**: yalnızca pinned ROOT_CA_SPKI_HASH; bu format Root'u ağdan trust source olarak çekmez.
- **RA/CA izolasyonu korunur**: bu format RA→CA boundary'sini değiştirmez; canonical byte'lar izole mTLS/gRPC ile taşınır, custom transport değil.
- **RA→CA boring/auditable**: Tier-0 signing path standart mTLS/gRPC; sürpriz custom transport yok.

---

## 34. Acceptance Criteria

| # | Kriter |
|---|--------|
| AC-01 | Java implementasyonu Root/Intermediate/Identity/Device CCR'ı **tam** encode edebilir |
| AC-02 | Rust implementasyonu tüm CCR tiplerini parse + validate edebilir |
| AC-03 | C++ transport canonical byte'ları reinterpret etmeden taşır |
| AC-04 | Ed25519 signature input belirsiz değil (canonical_ccr_tbs / dr_tbs / signed_control_tbs; Session Model V3 session_context_v1) |
| AC-05 | Session Model V3 key schedule belirsiz değil (SESSION_INIT/RESPONSE/DATA, session_context_v1 binding, Section 35.2) |
| AC-06 | DeviceCertBundle/DiscoveryRecord/OrgEntitlementRecord hash'leri ve TBS'leri reproducible (Java/Rust/C++ identik) |
| AC-07 | Hiçbir alan platform-specific endian davranışı gerektirmez |
| AC-08 | İmzalı yapılarda varint yok |
| AC-09 | Parser rejection kuralları tam (Section 29) |
| AC-10 | Identity v1.5 validation algoritmaları bu formatı **yeni tasarım kararı olmadan** tüketebilir |
| AC-11 | RA→CA boundary net: izole mTLS/gRPC, Kama custom adaptive transport değil |
| AC-12 | identity_hash (ham key) vs subject_key_id (spki) ayrımı net, karıştırılamaz |
| AC-13 | CompactDeviceCertV1 tam 152B encode/decode (device_ed25519_pub+device_x25519_pub+issuer_key_id+not_after+issuer_signature); imzalayan Identity/User Cert (Intermediate DEĞİL); SAN/deviceId/serial YOK; dest_hash issuer Identity key'inden türetilir (device key'den DEĞİL) |
| AC-14 | Session Model V3 AAD (SESSION_DATA exact byte layout "SSD1", session_context_v1 binding) Section 35.6/35.9 ile tutarlı encode edilir |
| AC-15 | cipher_suite_id hem msg_tbs hem AEAD AAD'de; downgrade reddi test edilir |
| AC-16 | DiscoveryRecord current_device_x25519 == CompactDeviceCert device_x25519_pub binding doğrulanır (Section 21.2) |
| AC-17 | HubListRecord Intermediate-imzalı + Root'a zincir + operator_id tutarlılık; her **HubEntryV3 CERT-BOUND** (Hub Identity Cert + Hub Noise Cert; raw hub_noise_x25519_pub authoritative DEĞİL — §21.5.4, KAT5-P1-02) (Section 21.5.3) |
| AC-18 | OrgEntitlementRecord **License Authority-imzalı** (cert_type 0x0A; Root/Intermediate DEĞİL) + lifetime clamp (Identity/Device/HubList <= license_not_after) (Section 21.6.3) |
| AC-19 | Magic domain separation: SRCR/SRDR/SRHL/SROE + SSI1/SSR1/SSD1/SSC1 + SRRR/SRAN/SRAH çapraz-yorumlanamaz (SRAM + ASYNC_* DEPRECATED) |
| AC-20 | Intermediate 5 yıl / Identity 1 yıl / Device 90 gün validity bounds enforce (Section 6.8) |
| AC-CCR-HUBENTRY-1 | HubEntryV3, Hub Identity Cert + Hub Noise Cert taşır (Section 21.5.1) |
| AC-CCR-HUBENTRY-2 | Client hub_noise_x25519_pub değerini Hub Noise Cert.public_key'den çıkarır (Section 21.5.4) |
| AC-CCR-HUBENTRY-3 | Hub Noise Cert, Hub Identity Cert-imzalı + X25519 keyAgreement olmalı; aksi → reject (Section 21.5.4) |
| AC-CCR-HUBENTRY-4 | HubEntryV3 addresses, Hub Identity Cert SAN policy ile eşleşmeli (Section 21.5.4) |
| AC-CCR-RAENTRY-1 | RAServerEntry, RA Identity Cert + RA Noise Cert taşır (Section 21.8.1) |
| AC-CCR-RAENTRY-2 | Client RA Noise key'ini RA Noise Cert.public_key'den çıkarır (Section 21.8.3) |
| AC-A1 | Eski AsyncMessage/SRAM aktif protokol değildir; yalnızca Section 35-DEPRECATED appendix'te kalır |
| AC-A2 | Session Model V3 key schedule tek canonical async modeldir (Section 35.2) |
| AC-A3 | SESSION_INIT/RESPONSE/DATA byte layout deterministiktir (Section 35.3-35.6) |
| ~~AC-A4~~ (DEPRECATED V2) | ~~SESSION_DATA 39B~~ → Session Model V3: SESSION_DATA 18B app-envelope (seq16 2B + tag16 16B), link_id YOK, length implicit. Bkz **AC-SM3-1..12 / AC-SI-1..2 (Section 35.13)** |
| AC-A5 | Sender authentication normal hot-path'te AEAD tag + ss_s possession ile sağlanır (Section 35.2) |
| AC-A6 | Ed25519 sender_signature yalnızca SIGNED_CONTROL_MESSAGE için kullanılır (Section 35.9) |
| AC-A7 | recipient_discovery_record_hash key schedule'dan çıkarılmıştır; session_context_v1 recipient_static_x25519_hash kullanır (Section 35.2) |
| AC-A8 | message_type session_context'ten çıkarılmış, message-specific AAD magic'ine taşınmıştır (Section 35.2) |
| AC-A9 | "full-cert" terimi kullanılmaz; 272B path inline-compact-cert (cert_mode=1), chain için cert_mode=2 bundle (Section 35.3/35.11) |
| AC-A10 | policy_sequence key schedule'dan çıkarılmıştır; Alpha profile_id+cipher_suite_id sabit, downgrade yüzeyi yok (Section 35.7) |
| AC-A11 | deprecated SRAM/Noise N implementation path aktif parser/validator'da kabul edilmez (negative corpus, Section 30) |
| AC-LA-1 | CCR parser cert_type 0x0A License Authority'yi tanır (Section 17.5) |
| AC-LA-2 | License Authority CCR validation: is_ca=FALSE, key_usage=digitalSignature only, EKU=id-kama-license-authority, issuer=Root enforce edilir |
| AC-LA-3 | OrgEntitlementRecord signature yalnız License Authority public key'iyle doğrulanır (issuer_key_id → LA CCR çözümü) |
| AC-LA-4 | Root veya Intermediate ile imzalı OrgEntitlementRecord KABUL EDİLMEZ |
| AC-LA-5 | License Authority key'i OrgEntitlementRecord DIŞI object/cert imzasında kullanılırsa verifier reject eder (kritik authorization bug) |
| AC-LA-6 | License Authority cert Root'a, ROOT_CA_SPKI_HASH pinine kadar doğrulanır |
| AC-AS-1 | CCR parser cert_type 0x0B Admin Server'ı tanır (Section 17.6) |
| AC-AS-2 | Admin Server CCR validation: is_ca=FALSE, key_usage=digitalSignature only, EKU=id-kama-admin-server, issuer=per-org Intermediate enforce edilir |
| AC-AS-3 | Admin Server Cert yalnız Admin↔RA mTLS peer authentication için; canonical record/cert imzalayamaz |
| AC-AS-4 | Admin Server public key'iyle HubList/AHS/DiscoveryEndpoint/Revocation/OrgEntitlement imzası kabul edilirse verifier reject eder (kritik authorization bug) |

---

## 34.1 Final Self-Audit (Device CCR Trust-Model Hardening)

| Criterion | Status | Evidence section |
|-----------|--------|------------------|
| identity_hash yalnızca user_identity_ed25519_pub'tan | ✅ | §13.3, §13.5, §21.2 |
| dest_hash yalnızca identity_hash'ten | ✅ | §13.3, §13.5 |
| device pk_ed25519 yalnızca device signing key | ✅ | §13.1, §13.3 |
| CompactDeviceCert issuer = Identity/User Cert | ✅ | §13.1, §13.8 |
| CompactDeviceCert signature alanı = issuer_signature | ✅ | §13.1, §13.2 |
| CompactDeviceCert'te intermediate_sig YOK | ✅ | §13.1, §17, §29 |
| CompactDeviceCert tam boyut = 152B | ✅ | §13.1, §13.2 |
| CompactDeviceCert magic/version/serial/device_id/SAN/dest_hash YOK | ✅ | §13.2 |
| CompactDeviceCert revocation_id = device_cert_hash | ✅ | §13.6 |
| DiscoveryRecordV2 identity → device_cert_hash → device_x25519 bağlar | ✅ | §21.2 |
| Session Model V3 sender auth (ss_s possession, hot-path imzasız) | ✅ | §35.2 |
| Session Model V3 sender identity issuer Identity/User Cert'ten | ✅ | §35.12 |
| Client↔Client async: authenticated one-way (SESSION_INIT, IK/prekey YOK) | ✅ | §35 |
| SignedNoisePrekey aktif davranış YOK | ✅ | §18 DEPRECATED |
| PrekeyBundle aktif davranış YOK | ✅ | §19 DEPRECATED |
| Rutin X25519 rotation YOK | ✅ | §13.7 |
| Hostile cache-miss policy tanımlı | ✅ | §35.5 |
| X25519 all-zero DH reddi tanımlı | ✅ | §5.1, §13.8, §35.5b |
| Hub Identity lifetime tutarlı (3 yıl) | ✅ | §14 |
| device_cert_hash = SHA-256(canonical_compact_device_cert) (152B) | ✅ | §8.3, §13.6, §23 |
| Aktif device cert hashing'de canonical_device_ccr YOK | ✅ | §23 |
| Generic CCR vs CompactDeviceCert hash ayrımı net | ✅ | §23 |
| Generic Device CCR audit/mirror/non-hot-path only | ✅ | §13, §20.3 |
| DeviceCertBundle compact_device_cert_v1 kullanır | ✅ | §20, §20.3 |
| DiscoveryEndpointRecord hem pk_oprf hem discovery_transport_x25519_pub'ı imzalar | ✅ | §21.7 |
| Client↔Discovery Server E2E transport key Intermediate-imzalı | ✅ | §21.7 |
| Hub-visible plaintext discovery RPC byte-format düzeyinde yasak | ✅ | §21.7 |
| voprf_suite_id ve transport_suite_id ayrı alanlar | ✅ | §21.7 |
| Aktif test suite'te prekey golden vector YOK | ✅ | §30.5, §30.7, §31 |
| Aktif INVALID corpus CompactDeviceCertV1-tabanlı | ✅ | §30.7 (INVALID-1..22) |

---

## 35. Session Model V3 (SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL)

> **Mimari faz değişimi (Haziran 2026, CAD-60 — TLS 1.3 dersi):** Eski **Async Session V2** (ASYNC_INIT/ACCEPT/DATA, hot-path ≈39B app-envelope, link_id 16B + seq 4B + len 2B, magic SAI1/SAA1/SAD1/SAC1) **DEPRECATED** (Section 35-DEPRECATED-V2, MUST NOT IMPLEMENT / MUST REJECT) — 39B universal hot-path TLS/TCP/IPv4 established baseline'a karşı byte avantajı sağlamıyordu. Yeni aktif model **Session Model V3**: SESSION_INIT (tek seferlik authenticated bootstrap) → SESSION_INIT_RESPONSE → SESSION_DATA (18B E2E app-envelope hot-path) → SIGNED_CONTROL (nadir non-repudiation); magic SSI1/SSR1/SSD1/SSC1.
>
> **OTORİTE BÖLÜŞÜMÜ (P0-3, NORMATİF):** `identity-and-trust-model.md` **trust lifecycle, PKI, revocation, endpoint-validation semantiği** için otoriterdir. **Bu doküman (ccr §35)** Session Model V3 **wire/session canonical byte'ları ve hot-path message semantiği** için byte-level/wire-level otoriterdir. İkisi çelişirse: kimlik/PKI/trust → identity; wire/byte/session → ccr §35.
> **CROSS-DOC — [2026-09-07 KAPANDI]:** identity-and-trust-model.md'deki eski ASYNC_INIT/ACCEPT/DATA ≈39B modeli Session Model V3 ile reconcile EDİLDİ (identity YAMA KAYDI 2026-09-07 / E-13: per-message HKDF, `SR-DATA-*` label şeması, `session_root` adı, ham-seq replay ve kısa SIGNED_CONTROL TBS kaldırıldı). Aşağıdaki metin tarihsel gerekçe olarak korunur: eski ASYNC model; eski model deprecated appendix'e taşınmalı.

### 35.0 TLS 1.3 Dersi (NORMATİF ilke)

**Handshake sonrası bağ byte'larda değil, anahtardadır.** TLS established record her mesajda `connection_id`, `cert_hash`, `handshake_hash`, public key, CertificateVerify, imza veya certificate chain TAŞIMAZ — bağ, handshake transcript'ten türeyen application traffic key'dedir (16B AEAD tag). Kama aynı modeli benimser.

```
Kama established SESSION_DATA TAŞIMAZ:
  - per-message cert-ref          - per-message length
  - per-message link_id           - outer transport AEAD tag
  - per-message Ed25519 imza      - FEC header (block/shard framing dışında)
  - per-message dest_hash / routing-info

Established message binding = session_key + direction + epoch + reconstructed_counter + AEAD tag.
```

### 35.1 Mesaj Taksonomisi

```
SESSION_INIT           : konuşma başına 1 kez. zero-RTT authenticated bootstrap (Noise-inspired one-way, §35.2) +
                         early_data (ilk payload). recipient identity binding + sender auth (static
                         X25519 possession) + session_context + init_root + hub mailbox/session +
                         hub_handle assignment. UÇ DOĞRULAMA ZORUNLU (Section 35.4).
SESSION_INIT_RESPONSE  : responder'ın handshake tamamlaması (Bob ephemeral, FS upgrade, epoch).
                         Hot-path DEĞİL; Bob offline ise sonra üretilir (Section 35.5).
SESSION_DATA           : established hot-path. TEK form. 18B E2E app-envelope (Section 35.6).
SIGNED_CONTROL         : SESSION_DATA + Ed25519(64B) + signed_control_context. Nadir non-repudiation
                         (komut/makbuz/legally-significant). Hot-path DEĞİL (Section 35.10).
```

### 35.2 Authenticated Key Schedule (session_context_v1 — KORUNDU, CAD-55/60)

Saf Noise N (sender anonim → ayrı imza) YERİNE authenticated one-way: sender authentication AEAD key derivation'a gömülü, ayrı per-message imza YOK.

> **NORMATİF (Noise-inspired, generic Noise pattern DEĞİL):** Kama Session Model V3, **N-türevi authenticated one-way DH schedule** kullanır (`ss_e` FS + `ss_s` sender auth). Generic bir Noise pattern transcript'i DEĞİLDİR ve **interaktif KK/IK DEĞİLDİR** (tek-yön, tek mesaj, Bob offline olabilir). cert-ref ≈ Noise K-benzeri, cert-inline ≈ Noise X-benzeri — ama standart Noise library transcript'i değil; bu bölümdeki canonical key schedule **otoriterdir**. (Senkron PTT/voice için interaktif IK/KK = Beta, ayrı konu. Beta'da formal Noise pattern mapping yapılabilir.)
>
> **İMZA/DEST_HASH (Section 13 + 35.4 ile tutarlı):** `sender_cert_ref = SHA-256(canonical_compact_device_cert)`; CompactDeviceCert'i **Identity/User Cert (sk_user_identity)** imzalar. `recipient_dest_hash = SHA-256(SHA-256(identity_cert.user_identity_ed25519_pub))` — Identity key'den, device key'den DEĞİL.

```
# SESSION_INIT root (Alice → Bob):
ss_e      = DH(sender_ephemeral_sk, recipient_static_x25519_pub)    # forward secrecy
ss_s      = DH(sender_static_x25519_sk, recipient_static_x25519_pub) # sender AUTH (possession proof)
init_root = HKDF("SR-SESSION-INIT-V3", ss_e || ss_s, session_context_v1)

# session_context_v1 (session-wide HKDF binding, 100B — wire'da TAŞINMAZ):
session_context_v1 =
  protocol_version              (u8,      1B)   ||   # 0x01
  profile_id                    (u8,      1B)   ||   # 0x01 = SESSION_PROFILE_V3_DEFAULT (Alpha sabit)
  cipher_suite_id               (u16_be,  2B)   ||   # 0x0001 (Alpha sabit)
  sender_cert_ref               (bytes32, 32B)  ||   # SHA-256(canonical_compact_device_cert) (ref/inline AYNI)
  recipient_dest_hash           (bytes32, 32B)  ||   # tam dest_hash (retarget koruması)
  recipient_static_x25519_hash  (bytes32, 32B)       # SHA-256(recipient_static_x25519_pub) (cert/x25519 swap pin)
# = 1+1+2+32+32+32 = 100B

# Session keys (Noise split benzeri):
k_send / k_recv  = HKDF("SR-SESSION-TRAFFIC-V3", init_root, direction)   # yön-ayrık transport keys
# epoch 0 başlar; SESSION_INIT_RESPONSE FS upgrade ile epoch ilerletebilir (Section 35.5).

# session_id (INTERNAL, wire'da TAŞINMAZ — AAD binding için):
session_id_internal = Trunc128(HKDF("SR-SESSION-ID-V3", init_root, session_context_v1))   # 16B
```

> **link_id KALDIRILDI (CAD-60):** Eski 16B `link_id` artık YOK. Routing rolü transport L2 `hub_handle`'a (Section 35.7), kriptografik link-binding rolü `session_id_internal` + per-session key'e geçti. Per-session key zaten cross-session ayrımı sağlar (A oturumunun key'iyle B oturumu decrypt edilemez). Korelasyon-direnci `hub_handle`'ın hub-assigned opaque (dest_hash'ten türemez) olmasıyla korunur.

### 35.2.1 HKDF Instantiation (NORMATİF — exact) [B1, 2026-09-02]

> Bu alt-bölüm §35.2'deki `HKDF(label, secret, context)` yazımının **tek** kanonik anlamını verir.
> Primitive seçimi burada YAPILMAZ — pki §13 crypto primitives registry (cipher_suite_id 0x0001) otoriterdir.
> Boşluk sınıfı: DR-4 dersi ("semantik tanımlı ama serialization tanımsız") — noise-wire §13.5 checklist.

```
PRIMITIVE (pki §13, cipher_suite_id 0x0001):
  KDF     = HKDF-BLAKE2s (RFC 5869 yapısı, hash = BLAKE2s-256)
  HashLen = 32B

KRİTİK — HMAC vs KEYED-HASH: RFC 5869 HKDF, HMAC-Hash üzerine tanımlıdır.
  Buradaki HMAC = HMAC-BLAKE2s-256 (RFC 2104; BLAKE2s block size 64B, output 32B).
  BLAKE2s'in NATIVE KEYED MODE'u (BLAKE2s-with-key) KULLANILMAZ — iki FARKLI fonksiyondur
  ve sessiz interop kırılmasının en yaygın kaynağıdır.
  Referans implementasyon: RustCrypto `hkdf::Hkdf<blake2::Blake2s256>` (HMAC'ı içeride kurar).

İKİ PRIMITIVE (RFC 5869 §2.2 / §2.3):
  Extract(salt, IKM)   = HMAC-BLAKE2s-256(key = salt, msg = IKM)        → PRK (32B)
  Expand(PRK, info, L) = RFC 5869 §2.3 (T(i) zinciri, sayaç 0x01'den)   → OKM (L byte)

KANONİK SARMALAYICI — bu belgedeki HER `HKDF(label, secret, context)` yazımı ŞU demektir:

  KDF(label, secret, context, L):
      PRK = Extract(salt = ASCII(label), IKM = secret)
      OKM = Expand(PRK, info = context, L = L)
      return OKM

  · label   : ASCII byte'ları; NUL-terminator YOK; padding YOK; uzunluk tam
  · secret  : konkatenasyon SIRASI aşağıdaki tabloda sabit
  · context : byte dizisi; boş olabilir (o hâlde info = boş dizi)
  · L       : tabloda sabit; L ≤ 255 × 32
  · TEK-KURAL: her türetme Extract+Expand yapar. Expand-only kısayolu, salt=zeros varyantı,
    label'ı info'ya taşıma YASAK.
  · SALT = LABEL SEÇİMİNİN GEREKÇESİ [CRY-4]: RFC 5869 §3.1 salt'ın public ve keyfi olmasına
    izin verir; salt'ın birincil işlevi domain separation'dır. Her türetmeye kendi public
    label'ını salt olarak vermek, her birine AYRIK bir Extract domain'i verir ve tek kod yolu
    korunur. Bu, Noise'un (salt = chaining key) ve TLS 1.3'ün (HKDF-Expand-Label) kalıbından
    bilinçli bir SAPMAdır; denetçi için burada kayıtlıdır. Doğrulama: HKDF implementasyonunun
    kendisi önce RFC 5869'un SHA-256 vektörleriyle sınanır, sonra BLAKE2s vektörleri üretilir. Gerekçe: init_root ve epoch_1_root ham DH materyali
    içerir → Extract kriptografik olarak ZORUNLU; kalan üçünde gereksiz ama zararsızdır.
    Tek kod yolu = implementer hatası ve interop ayrışma yüzeyi minimum.
```

**TÜRETME TABLOSU (NORMATİF — sıra, uzunluk ve byte temsilleri sabit):**

```
D1  init_root                                                            L = 32
    label   = "SR-SESSION-INIT-V3"
    secret  = ss_e (32B) || ss_s (32B)                                   # 64B
    context = session_context_v1 (100B, §35.2)

D2  epoch-0 traffic keys                                                 L = 32 (her yön)
    label   = "SR-SESSION-TRAFFIC-V3"
    secret  = init_root (32B)
    context = direction (1B)   # 0x01 = A→B (initiator→responder)
                               # 0x02 = B→A          (§35.9 direction alanıyla AYNI kodlama)
    k_A2B_e0 = KDF(label, init_root, 0x01, 32)
    k_B2A_e0 = KDF(label, init_root, 0x02, 32)
    Rol eşlemesi: initiator k_send = k_A2B_e0, k_recv = k_B2A_e0; responder tersi.

D3  session_id_internal                                                  L = 16
    label   = "SR-SESSION-ID-V3"
    secret  = init_root (32B)
    context = session_context_v1 (100B)
    NOT: L=16 ile "L=32 üret + ilk 16B'yi al" BYTE-ÖZDEŞ sonuç verir (RFC 5869 T(1) prefix
         özelliği). §35.2'deki `Trunc128(...)` yazımı TAM OLARAK bunu ifade eder; kanonik
         implementasyon yazımı L=16'dır.

D4  epoch_1_root                                                         L = 32
    label   = "SR-SESSION-EPOCH-V3"
    secret  = init_root (32B) || ss_e2 (64B)                             # 96B
              ss_e2 = DH(bob_eph_sk, sender_eph_pub) || DH(bob_eph_sk, sender_static_pub)
    context = u16_be(1) = 0x00 0x01                                      # 2B

D5  epoch-1 traffic keys                                                 L = 32 (her yön)
    [YENİ — §35.5 epoch_1_root'u veriyordu ama traffic key türetimini TANIMLAMIYORDU]
    label   = "SR-SESSION-TRAFFIC-V3"     # D2 ile AYNI label; PRK farklı ⇒ key farklı
    secret  = epoch_1_root (32B)
    context = direction (1B, D2 ile aynı kodlama)
    k_A2B_e1 = KDF(label, epoch_1_root, 0x01, 32)
    k_B2A_e1 = KDF(label, epoch_1_root, 0x02, 32)

D6  SESSION_INIT seal key                                                L = 32
    [KARAR-1 = A uygulandı: ayrı domain-separated anahtar]
    label   = "SR-SESSION-INIT-DATA-V3"
    secret  = init_root (32B)
    context = 0x01                                                       # A→B
    k_init  = KDF(label, init_root, 0x01, 32)

D7  SESSION_INIT_RESPONSE seal key                                       L = 32
    [KARAR-1 = A uygulandı]
    label   = "SR-SESSION-RESP-DATA-V3"
    secret  = epoch_1_root (32B)
    context = 0x02                                                       # B→A
    k_resp  = KDF(label, epoch_1_root, 0x02, 32)
```

> **KARAR-1 gerekçesi (D6/D7 neden ayrı anahtar):** SESSION_INIT `k_A2B_e0` ile ve nonce96(0)
> ile mühürlenseydi, epoch-0 SESSION_DATA sayacı da 0'dan başladığı için (noise-wire §4
> EPOCH-PN-INIT) **aynı anahtar altında nonce çakışması** doğardı. Alternatif "0 rezerve
> edilmiştir" konvansiyonu yerine ayrı anahtar seçildi: nonce ayrımı **yapısal** olur,
> §4'ün sayaç-başlangıç kuralı dokunulmadan kalır, denetimde konvansiyona değil
> domain-separation'a dayanılır. Maliyet: session başına 2 ekstra HKDF çağrısı.

**INVARIANT'lar (NORMATİF):**

| ID | Statement |
|----|-----------|
| INV-KDF-1 | Tüm türetmeler tek kanonik sarmalayıcıdan geçer; başka HKDF çağrım biçimi (Expand-only, salt=zeros, label'ı info'ya taşıma) YASAK. |
| INV-KDF-2 | `label` byte'ları protocol_version V1 için IMMUTABLE; değişirse yeni protocol_version. |
| INV-KDF-3 | HMAC-BLAKE2s-256 ≠ BLAKE2s-keyed; test bunu AYRI negatif vektörle assert eder. |
| INV-INIT-KEY-1 | Her `init_root` TAM OLARAK BİR SESSION_INIT mühürler. Payload değişirse yeni `init_counter` + YENİ ephemeral + yeni session attempt (noise-wire §0.7-B / MADDE-5). Retry = cached byte-for-byte datagram; yeniden seal YASAK. |
| INV-RESP-KEY-1 | Her `epoch_1_root` TAM OLARAK BİR SESSION_INIT_RESPONSE mühürler (fresh responder ephemeral ⇒ fresh `epoch_1_root`). RESPONSE retry'ı da cached byte-for-byte. |

**Zorunlu test vektörleri:**

```
V-KDF-1 : sabit ss_e/ss_s/session_context_v1 → init_root exact 32B
V-KDF-2 : k_A2B_e0, k_B2A_e0 exact + k_A2B_e0 ≠ k_B2A_e0   (INV-NONCE-DIR-1 birincil testi)
V-KDF-3 : session_id_internal exact 16B; L=16 ≡ Trunc128(L=32) byte-özdeşlik assert'i
V-KDF-4 : epoch_1_root exact; k_*_e1 ≠ k_*_e0               (INV-NONCE-EPOCH-1 birincil testi)
V-KDF-5 : k_init ≠ k_A2B_e0 ve k_resp ≠ k_B2A_e1            (D6/D7 domain-separation)
V-KDF-6 : NEGATİF — HMAC-BLAKE2s-256 vs BLAKE2s-keyed FARKLI çıktı verir
V-KDF-7 : üç-kaynak eşitliği (spec byte'ları + RustCrypto + bağımsız referans; §13.5)
```

### 35.3 SESSION_INIT Wire Format + Cert Policy

> **[AMENDMENT-6] `early_data` istisnası (NORMATİF):** Application plaintext'i bir kez normal data-path dışında Crypto'ya girer — `connect(peer, early_unit?)` → Transport → SessionControl `Initiate{peer, early_unit}`. Crypto `early_unit`'i **OPAK byte dizisi** olarak işler; uygulama semantiğini (TEXT/FILE/…) HİÇBİR ZAMAN yorumlamaz. Application yine Crypto'yu görmez (INV-NEIGHBOR-1).

**flags alanı (NORMATİF — canonical bit allocation):**
```
SESSION_INIT.flags : u8
  bits 0..1 = cert_mode
    00 = CERT_REF_32
    01 = INLINE_COMPACT_DEVICE_CERT_V1_152
    10 = DEVICE_CERT_BUNDLE
    11 = RESERVED → reject
  bits 2..3 = init_variant
    00 = SR_SM3_AUTHENTICATED_ONE_WAY_DEFAULT
    01..11 = RESERVED → reject
  bits 4..7 = reserved; MUST be 0; non-zero → reject
```
> **NORMATİF:** `init_variant` generic bir Noise pattern selector DEĞİLDİR ("KK/IK pattern bit'i" YOK — implementer'ı yanlış Noise library mapping'ine iter). Session Model V3 key schedule (Section 35.2) canonical ve otoriterdir.

**SESSION_INIT canonical wire layout (cert_mode'a göre ayrık — NORMATİF):**
```
SESSION_INIT_REF (cert_mode=00):
  flags                : u8
  recipient_dest_hash  : bytes32
  sender_ephemeral     : bytes32
  sender_cert_ref      : bytes32
  init_counter         : u64_be
  ciphertext           : bytes[N]
  aead_tag             : bytes16

SESSION_INIT_INLINE_COMPACT (cert_mode=01):
  flags                : u8
  recipient_dest_hash  : bytes32
  sender_ephemeral     : bytes32
  compact_device_cert  : bytes152
  init_counter         : u64_be
  ciphertext           : bytes[N]
  aead_tag             : bytes16

SESSION_INIT_BUNDLE (cert_mode=10):
  flags                : u8
  recipient_dest_hash  : bytes32
  sender_ephemeral     : bytes32
  device_cert_bundle   : DeviceCertBundle (Section 6.7, hard max)
  init_counter         : u64_be
  ciphertext           : bytes[N]
  aead_tag             : bytes16
# hub_handle SESSION_INIT'te TAŞINMAZ — hub INIT'i işleyince session yaratır + hub_handle ATAR (L2 control).
```

**Parser reject kuralları (NORMATİF):**
```
cert_mode=00 ama sender_cert_ref length != 32 → reject
cert_mode=01 ama compact_device_cert length != 152 → reject
cert_mode=10 ama DeviceCertBundle malformed / max aşımı → reject
cert_mode=11 → reject
init_variant != 00 → reject
reserved flags (bits 4..7) non-zero → reject
```

**Cert policy (reference-first, NORMATİF):**
```
- reference-first: sender_cert cache-hit ise full cert GÖNDERİLMEZ (ref 32B).
- cache miss → inline CompactDeviceCertV1 (152B) gönderilebilir.
- DeviceCertBundle hard max byte budget zorunlu (Section 6.7); değişken bırakılmaz.
- bundle lowband/hot-path DEĞİL; MTU/lowband budget aşılırsa inline EDİLMEZ → transport bulk/multi-datagram
  path'e düşer veya defer edilir (cert_ref/cache/fetch).
```

**SESSION_INIT byte tablosu (NORMATİF, exact):**
```
SESSION_INIT_REF (cert_mode=00):
  1 + 32 + 32 + 32 + 8 + tag 16 = 121B app-envelope + early_data
  UDP/IPv4 = 149B + early_data    |    UDP/IPv6 = 169B + early_data
SESSION_INIT_INLINE_COMPACT (cert_mode=01):
  1 + 32 + 32 + 152 + 8 + tag 16 = 241B app-envelope + early_data
  UDP/IPv4 = 269B + early_data    |    UDP/IPv6 = 289B + early_data
SESSION_INIT_BUNDLE (cert_mode=10):
  hard max = DeviceCertBundle budget (Section 6.7); NOT hot-path; fragmentation/MTU policy
  (1200B default MTU aşımı → multi-datagram bulk path, transport-layer-tdd.md Pipeline C).
```

> **Noise-core notu (dürüstlük):** Çıplak Noise ilk mesajı ~48-96B; SESSION_INIT bunun ÜSTÜNE recipient_dest_hash (32, hub routing) + sender_cert_ref/cert (32/152, identity) taşır → REF ~121B / INLINE ~241B app-envelope. Bu fark **routing + identity bootstrap maliyetidir** (konuşma başına 1 kez amortize); çıplak Noise sayısı olarak satılmaz.

> **Future optimization (NON-NORMATIVE, Alpha canonical DEĞİL):** `recipient_dest_hash32 → recipient_routing_key16` + `sender_cert_ref32 → sender_cert_hint16` ileride SESSION_INIT_REF'i 121B → ~89B app-envelope'a indirebilir. Alpha canonical değil; collision/fail-closed/cache semantiği + cross-doc review gerektirir.

### 35.3.1 SESSION_INIT AAD + Nonce (NORMATİF) [B2, 2026-09-02]

Wire'da magic TAŞINMAZ; AAD içindedir (§35.9 ile aynı domain-separation disiplini).

```
SESSION_INIT_AAD =
  "SSI1"                (4B)          ||   # 53 53 49 31 — AAD-only, wire'da YOK
  protocol_version      (u8,     1B)  ||
  cipher_suite_id       (u16_be, 2B)  ||
  flags                 (u8,     1B)  ||   # wire'daki byte, AYNEN
  recipient_dest_hash   (32B)         ||   # wire'daki byte, AYNEN
  sender_ephemeral      (32B)         ||   # wire'daki byte, AYNEN
  cert_field            (değişken)    ||   # cert_mode'a göre wire'da NE VARSA aynen:
                                           #   00 → sender_cert_ref     (32B)
                                           #   01 → compact_device_cert (152B)
                                           #   10 → device_cert_bundle  (alınan byte'lar)
  init_counter          (u64_be, 8B)  ||
  session_id_internal   (16B)              # INTERNAL (wire'da YOK); §35.2.1 D3

# [KARAR-2 = A, KARAR-3 = Evet uygulandı]
```

> **İLKE (KARAR-2 gerekçesi):** AAD, ciphertext'ten ÖNCEKİ **tüm** cleartext wire byte'larını
> birebir kapsar (+ magic/version/suite öneki, + `session_id_internal` soneki).
> Zorunluluk sebebi: `flags` ve `init_counter` **`session_context_v1`'e GİRMEZ** → anahtar bağı
> onları korumaz. AAD kapsamazsa transit'te değiştirilebilirler (cert_mode karışıklığı,
> replay-identity oynatma). Minimal AAD teknik olarak yeterli olabilirdi ama doğrulaması
> "hangi alan dolaylı olarak bağlı" muhakemesi gerektirirdi; tam kapsamda böyle bir muhakeme
> yoktur. AAD wire'da taşınmadığı için değişken uzunluğun byte maliyeti SIFIRDIR.

```
NONCE:
  nonce = nonce96(0)          # noise-wire §4.5 mapping: 4B zero-prefix || LE64(0)
  KEY   = k_init              # §35.2.1 D6
  Güvenlik dayanağı: INV-INIT-KEY-1 — her init_root tam olarak bir SESSION_INIT mühürler;
  retry cached byte-for-byte olduğu için aynı (key, nonce) ile İKİNCİ BİR plaintext üretilmez.
```

**Zorunlu test vektörleri:**
```
V-SSI-1 : exact AAD byte dizisi — üç cert_mode için AYRI vektör
V-SSI-2 : exact nonce + ciphertext + tag
V-SSI-3 : NEGATİF — flags'te 1 bit değişimi → AEAD fail
V-SSI-4 : NEGATİF — init_counter değişimi → AEAD fail
V-SSI-5 : NEGATİF — cert_mode ile uzunluk uyumsuzluğu → parser reject (AEAD'ye ULAŞMAZ)
```

### 35.4 SESSION_INIT Uç Doğrulama (ZORUNLU — NORMATİF, hub-untrusted)

> Bu bölüm E2E güvenliğin can damarıdır. Hub GÜVENİLMEZDİR; hub'ın admission kontrolü (Root CA join gate) E2E authentication YERİNE GEÇMEZ.

> **İMZA ZİNCİRİ (Section 13 ile birebir — KRİTİK):** CompactDeviceCertV1 `issuer_signature = Ed25519_Sign(sk_user_identity, compact_device_cert_tbs)` — yani **Identity/User Cert** imzalar, **Intermediate DEĞİL**. Trust path: `CompactDeviceCert → Identity/User Cert → Intermediate → pinned Root`. `intermediate_sig` terimi CompactDeviceCert bağlamında KULLANILMAZ. Device cert doğrulama public key'i = `identity_cert.user_identity_ed25519_pub`.
>
> **dest_hash TÜRETİMİ (KRİTİK — Identity key'den, device key'den DEĞİL):** `identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub)`, `dest_hash = SHA-256(identity_hash)`. Device cert'teki `device_ed25519_pub` routing identity ÜRETMEZ (bir kullanıcının çok cihazı olabilir; hepsi aynı user identity → aynı dest_hash). `device_x25519_pub`, Identity/User Cert altında imzalı olduğu için o user identity'ye bağlıdır.

```
Alice ve Bob (her iki uç) ZORUNLU doğrular:
  1. CompactDeviceCert.issuer_signature, identity_cert.user_identity_ed25519_pub ALTINDA doğrulanır
     (sk_user_identity imzaladı; Intermediate DEĞİL). Sonra identity_cert chain → Intermediate →
     PINNED Root CA (app bundle SPKI; ASLA ağdan). verify_chain_to_root (§15.2).
  2. compact_device_cert_tbs TEK imzalı TBS'te device_ed25519_pub + device_x25519_pub'u BİRLİKTE bağlar
     (Identity/User Cert imzası). pk_x25519 swap → issuer_signature geçersiz.
  3. recipient_dest_hash = SHA-256(SHA-256(identity_cert.user_identity_ed25519_pub)) == hedeflenen
     recipient dest_hash (discovery'den). DiscoveryRecord.dest_hash ↔ CompactDeviceCert issuer identity EŞLEŞİR.
  4. DiscoveryRecord.current_device_x25519 == CompactDeviceCert.device_x25519_pub (binding, CAD-42).
  5. Noise-inspired static-DH possession (ss_s) → peer device_x25519_pub'un private key'ini elinde tutuyor.
  6. SADECE bundan sonra session aktive. SESSION_INIT recipient static X25519'u bu doğrulanmış
     CompactDeviceCert'ten alır.

SALDIRI ANALİZİ (neden hub MITM yapamaz):
  - Hub cert'i saldırganınkiyle değiştirir → saldırganın user identity'si → farklı dest_hash → adım 3 PATLAR.
  - Hub device_x25519_pub'u swap eder → Identity/User Cert issuer_signature geçersiz (sk_user_identity
    olmadan forge edilemez) → adım 1/2 PATLAR.
  - Hub başka kullanıcının geçerli cert'ini koyar → dest_hash hedefle eşleşmez → adım 3 PATLAR.
  → Hub'ın kötülüğü DoS/metadata'ya iner; içerik dinlemeye ASLA. Endpoint doğrulamasını ATLAYAN
    dürüst uç olmadıkça (bu yüzden uç-doğrulama PROTOKOL-ZORUNLU, opsiyonel değil) saldırı çalışmaz.
```

### 35.5 SESSION_INIT_RESPONSE (responder handshake / FS upgrade — canonical)

> Responder state transition KAYBOLMAZ; SESSION_INIT_RESPONSE canonical olarak tanımlıdır.

```
SESSION_INIT_RESPONSE (E2E, Bob → Alice; magic "SSR1" AAD'de):
  flags                 : u8        # bit0 = FS upgrade mode
  responder_ephemeral   : bytes32   # bob_ephemeral X25519 (FS upgrade)
  response_counter      : u64_be 8B # epoch 1 başlangıç
  ciphertext            : bytes[N]  # Bob'un ilk cevabı (early response data)
  aead_tag              : bytes16
# = flags 1 + responder_ephemeral 32 + counter 8 + tag 16 = 57B app-envelope + response_data

ss_e2        = DH(bob_ephemeral_sk, sender_ephemeral_pub) || DH(bob_ephemeral_sk, sender_static_pub)
epoch_1_root = HKDF("SR-SESSION-EPOCH-V3", init_root || ss_e2, u16_be(1))
```

> **hub_handle ATAMA KARARI (NORMATİF):** `hub_handle` **E2E SESSION_INIT_RESPONSE'ta TAŞINMAZ.** hub_handle hub↔client (per-hop) bir L2 routing değeridir; hub, client'ın SESSION_INIT'ini işleyip session/mailbox state yaratınca **L2 control acknowledgment** ile client'a atar (transport-layer-tdd.md L2). Her hop kendi hub_handle'ını bağımsız atar (Alice↔HubA ayrı, Bob↔HubB ayrı). E2E handshake (INIT/RESPONSE) ile hub_handle atama AYRI düzlemlerdir (E2E vs L2).

**State machine (NORMATİF):**
```
INIT_SENT               : Alice SESSION_INIT gönderdi
INIT_STORED_OFFLINE     : Bob offline → hub mailbox'ta bekliyor
INIT_DELIVERED_TO_RESPONDER : Bob online, INIT'i aldı
INIT_RESPONSE_PENDING   : Bob doğruladı, RESPONSE üretiyor
INIT_RESPONSE_VERIFIED  : Alice RESPONSE'u doğruladı (epoch 1 FS aktif)
DATA_EPOCH_ACTIVE       : SESSION_DATA akışı (epoch 1)
```

**Reject kuralları:**
```
- RESPONSE gelmeden epoch_1 SESSION_DATA → DEFER/REJECT
- **epoch_0 SESSION_DATA acceptance, RESPONSE gönderimi/alımıyla DOĞRUDAN KAPANMAZ [P0-9/§12, 2026-09-07].**
  `dh_epoch` 0→1 transition ve old-epoch retirement `kama-noise-wire §4 confirmation-safe state machine`'ine
  tabidir (OTORİTE oradadır): previous-epoch receive state YALNIZ implicit confirmation (ilk GEÇERLİ,
  AEAD-doğrulanmış epoch-1 paketi) + bounded grace tamamlandıktan sonra DISCARD edilir. **Discard sonrası
  epoch_0 → REJECT.** State direction-local tutulur; RESPONSE kaybolursa epoch-0 usable kalır.
  (Önceki metin "RESPONSE sonrası epoch_0 → REJECT" idi; blanket okuma confirmation-safe makineyle
  çelişiyordu. Wire format ve key schedule DEĞİŞMEDİ.)
- responder_ephemeral all-zero → REJECT
- DH all-zero shared secret → REJECT (Section 6.x low-order/all-zero kontrolü)
- session_context mismatch → AEAD fail/REJECT
- **[C-4] RESPONSE flags (INIT ile SİMETRİK):** `flags.bit0 = 0` zorunlu; `bits1..7` reserved →
  nonzero ise REJECT (fail-closed). `response_counter` monoton olmalı; eşit/geri giden → REJECT.
- **[C-3] Ephemeral tazeliği (NORMATİF):** Bir `sender_ephemeral`, FARKLI bir `init_counter` ile
  İKİNCİ KEZ KULLANILAMAZ. Payload değişirse → yeni init_counter + YENİ ephemeral + yeni session attempt.
  Retransmit yalnız cached byte-for-byte datagram'dır (§35.2.1 INV-INIT-KEY-1; noise-wire §0.7-B MADDE-5).
  Aynı kural responder tarafında `responder_ephemeral` / `response_counter` için geçerlidir (INV-RESP-KEY-1).
```

### 35.5.1 SESSION_INIT_RESPONSE AAD + Nonce (NORMATİF) [B2, 2026-09-02]

```
SESSION_INIT_RESPONSE_AAD =
  "SSR1"                (4B)          ||   # 53 53 52 31 — AAD-only, wire'da YOK
  protocol_version      (u8,     1B)  ||
  cipher_suite_id       (u16_be, 2B)  ||
  flags                 (u8,     1B)  ||   # wire'daki byte, AYNEN
  responder_ephemeral   (32B)         ||   # wire'daki byte, AYNEN
  response_counter      (u64_be, 8B)  ||
  session_id_internal   (16B)              # §35.2.1 D3 (init_root'tan; epoch-1'de DEĞİŞMEZ)

NONCE:
  nonce = nonce96(0)
  KEY   = k_resp             # §35.2.1 D7
  Güvenlik dayanağı: INV-RESP-KEY-1 — her epoch_1_root tam olarak bir RESPONSE mühürler.
```

**Zorunlu test vektörleri:**
```
V-SSR-1 : exact AAD + nonce + ciphertext + tag
V-SSR-2 : NEGATİF — responder_ephemeral all-zero → reject (§35.5 kuralı; AEAD'den ÖNCE)
V-SSR-3 : NEGATİF — response_counter değişimi → AEAD fail
```

### 35.6 SESSION_DATA Canonical Bytes (HOT-PATH — EN KRİTİK)

```
SESSION_DATA app-envelope canonical (E2E — hub okuyamaz):
  seq16        : u16_be     # düşük-bit / resync hint (Section 35.8 — TEK BAŞINA nonce DEĞİL)
                            # AEAD nonce96 = 0x00×4 || LE64(reconstructed_full_counter)
                            #   (noise-wire §4.5 mapping; DR-4 CLOSED) [C-1]
                            # direction ve epoch nonce'ta DEĞİL — ANAHTARDA (§35.2.1 D2/D5) ve AAD'de (§35.9);
                            # cross-direction / cross-epoch replay savunması anahtar ayrımındadır.

# ─── HOT-PATH BYTE BÜTÇESİ — MUST NOT [2026-09-16, Abkarada byte kuralı] ───────────────
# Established hot-path E2E paketine yeni metadata eklemek "son çare" DEĞİL, **DEFAULT YASAK**.
# Aşağıdakiler bu zarfa EKLENEMEZ:
#   session_id · resp_hint · key_phase · transport_id · ack_id · fec metadata · rpc method_id
#   · explicit length · ikinci/outer AEAD tag · imza
# Bir alan gerçekten zorunluysa sorulacak soru "kaç byte ekleyelim" DEĞİLDİR; sırasıyla:
#   (1) mevcut bir alanı ÇIKARABİLİR miyiz?
#   (2) stateful çözebilir miyiz (taşıyıcı/sunucu state'i, 0 wire byte)?
#   (3) hub'a güvenerek çözebilir miyiz (trust takası, 0 wire byte)?
# Gerekçe: 3.125 B/s tabanında her 2 byte ≈ 0.64 s hava zamanıdır. 22B zarf, TLS/TCP/IPv4'ün
# ~62B'sine karşı ürünün tek gerçek kaldıracıdır; büyürse ürünün varlık sebebi kalmaz.
  ciphertext   : bytes[N]
  aead_tag     : bytes16

Fixed app-envelope overhead = seq16 2B + aead_tag 16B = 18B + payload   (length IMPLICIT — UDP datagram sınırı)

Transport L2 (CCR app-envelope'a DAHİL DEĞİL, aynı datagramda):
  hub_handle : 4B           # cleartext, hub routing/demux (Section 35.7)

LAYER ACCOUNTING:
  app-envelope only      : 18B + payload
  UDP payload            : hub_handle 4 + seq16 2 + ct + tag 16 = 22B + payload
  / UDP / IPv4           : 22 + 28 = 50B + payload
  / UDP / IPv6           : 22 + 48 = 70B + payload
```

**TLS established kıyas (layer-matched, NORMATİF dürüstlük):**
```
TLS record ~22B + TCP/IPv4 40B           = 62B + payload
Kama SESSION_DATA / UDP / IPv4        = 50B + payload
Δ ≈ 12B daha iyi/paket.

Kama TLS'i byte'ta EZMİYOR; established data path'te ~12B avantaj. ASIL avantaj YAPISAL:
zero-RTT/session amortization, offline/store-and-forward, no per-message signature, no per-message
cert, no re-handshake, no stream dependency.
```

### 35.7 hub_handle Semantiği + Allocation (NORMATİF)

> **[KARAR-SLOTU KS-HANDLE-1 — 2026-09-19] `hub_handle` yeniden kullanım koşulu:** Bir handle invalidate edildikten (§35.16 rebind, teardown, `hub_handle_epoch` rollover) sonra **hiçbir peer'a** yeniden atanmadan önce (a) o handle'a bağlı transport outstanding/ARQ state'i ölmüş olmalı VE (b) `HANDLE_TOMBSTONE` dolmuş olmalı. **Türetim kuralı (NORMATİF — magic number değil):** `HANDLE_TOMBSTONE ≥ max(hop-ARQ maksimum RTO×retry ufku, MAX_REORDER_DISTANCE'ın süre karşılığı, mailbox teslim ufku)` — yani o handle'a **geç gelebilecek son meşru datagramın** ufku; namespace 2³² olduğu için tombstone'un bellek maliyeti düşüktür, kısa tutmanın gerekçesi yoktur. Değer bu üç girdiden hub sahibi tarafından hesaplanır `[KALİBRASYON]`. Aksi hâlde eski peer'ın uçuştaki datagramı yeni peer'ın leg'ine düşer (AEAD fail eder ama transport state'e girer — availability). Bu satır olmadan §35.7'nin PRF türetimi "bir handle = bir leg" garantisini yalnız *aktif* tabloda verir.

> **[AMENDMENT-6] `hub_handle` (HubHandle — wire/routing kimliği) ≠ `SessionHandle` (Crypto'nun bastığı yerel capability; wire'da YOK; transport-tdd §0 INV-NEIGHBOR-1). İki tip BİRLEŞTİRİLEMEZ; eşleme Crypto'nun içindedir (kod: legacy.rs `LegacySessionProtector::new(hub_handle, …)`, session.rs h_ext → Session).**

```
- 32-bit opaque hub-local routing handle. authentication DEĞİL. E2E boundary DEĞİL.
- 4B default (Alpha lowband). High-risk deployment 8B handle profili MAY; Alpha default 4B.
- hub_handle_epoch İÇİNDE collision-free allocate EDİLMELİ.
- PRF-derived OLMALI (predictable plain counter DEĞİL); collision retry için allocation_attempt:
    hub_handle = Trunc32(PRF(hub_secret, internal_session_id || hub_handle_epoch || allocation_attempt))
- allocation_attempt 0'dan başlar; hub active table'da collision kontrol eder; collision → attempt++ retry.
- attempt > MAX_HANDLE_ALLOC_ATTEMPTS (=8) → hub_handle_epoch rollover VEYA 8B handle profili.
- handle wrap veya yüksek collision pressure → hub_handle_epoch rollover ZORUNLU.
- unknown handle → cheap-drop / rate-limit (LINK_ROUTE_MISS, Section 35.16); kriptografik accept DEĞİL.
- hub_handle değişse E2E key DEĞİŞMEZ (binding key'de). Malicious hub handle değiştirerek geçerli
  SESSION_DATA ÜRETEMEZ (inner AEAD key yok → endpoint doğrulamaz).
- yalnızca hub routing/demux için.
```

### 35.8 seq16 + Full-Counter Reconstruction + Replay (NORMATİF — sayısal)

> `seq16` güvenlik counter'ı DEĞİLDİR — wire'da taşınan düşük-bit/resync hint'tir. Gerçek AEAD nonce/counter **full session counter**'dır (receiver state'inden reconstruct).

```
reconstruct_counter(expected_counter, seq16):
  base = expected_counter & ~0xffff
  candidates = [ base | seq16, (base - 0x10000) | seq16, (base + 0x10000) | seq16 ]
  candidate = argmin |c - expected_counter|  over candidates
  # [C-9] BERABERLİK KURALI (NORMATİF — noise-wire §4 32-bit PN kuralıyla AYNI):
  #   iki aday eşit uzaklıktaysa → geriye doğru olan (düşük) aday seçilir.
  #   (Simetrik formülasyon: backward tie → düşük, forward tie → yüksek.)
  #   İki farklı tie kuralı = iki implementasyonun AYNI pakette ayrışması → interop kırılması.

  ACCEPT only if:
    candidate >= lowest_acceptable_counter
    candidate <= expected_counter + MAX_FUTURE_GAP
    candidate not already seen in replay bitmap
    |candidate - expected_counter| <= MAX_REORDER_DISTANCE
    epoch matches
    AEAD verifies
  REJECT if: duplicate | too old | too far future | ambiguous wrap | AEAD fail

NUMERIC CONSTANTS (Alpha default; saha-kalibre):
  REPLAY_WINDOW_SIZE     = 1024          MAX_FUTURE_GAP         = 4096
  MAX_REORDER_DISTANCE   = 8192          MAX_MESSAGES_PER_EPOCH = 2^32
  REKEY_THRESHOLD        = 2^31  (epoch rollover bundan ÖNCE; seq16 ambiguity oluşmadan)

INVARIANTS:
  MAX_REORDER_DISTANCE < 2^15 ... DÜZELTME: MAX_REORDER_DISTANCE ve MAX_FUTURE_GAP < 2^15 OLMALI
  (seq16 16-bit penceresi → reorder/future pencereleri 2^15'i AŞAMAZ, aksi halde wrap ambiguity).
  → Alpha: MAX_REORDER_DISTANCE = 8192 (<2^15 ✓), MAX_FUTURE_GAP = 4096 (<2^15 ✓).
  seq16 TEK BAŞINA nonce DEĞİL. AEAD nonce/counter = reconstructed full counter.
  replay identity = (session_id_internal, direction, epoch, full_counter).
  replay window AEAD success'ten ÖNCE GÜNCELLENMEZ.
```

### 35.9 SESSION_DATA AAD (NORMATİF)

Wire'da magic TAŞINMAZ; magic/domain-separator AAD içindedir. Parser tipi transport carrier/frame class ile bilir.

```
SESSION_DATA_AAD =
  "SSD1"                  (4B)        ||   # SESSION_DATA domain separator (AAD-only, wire'da YOK)
  protocol_version        (u8, 1B)    ||
  cipher_suite_id         (u16_be, 2B)||
  direction               (u8, 1B)    ||   # 0x01 A→B / 0x02 B→A (cross-direction replay engeli)
  epoch                   (u16_be, 2B)||
  seq16                   (u16_be, 2B)||
  session_id_internal     (16B)            # INTERNAL (wire'da YOK) — cross-session ayrımı
# hub_handle E2E AAD'ye GİRMEZ (hub-local remap serbest kalsın). link_id KALDIRILDI (binding key/context'te).
```

> **AAD asimetrisi KASITLIDIR [B2 notu, 2026-09-02]:** SSD1 AAD'si `direction` / `epoch` /
> `session_id_internal`'ı AÇIKÇA taşır; EXTENDED zarf AAD'si (noise-wire §2.8) taşımaz — orada
> bunlar ANAHTARDA bağlıdır (INV-NONCE-DIR-1 / INV-NONCE-EPOCH-1: yön ve epoch başına ayrı key).
> İki farklı yaklaşım, ikisi de yeterli; çelişki DEĞİLDİR.
> `epoch` alanı = `u16_be(dh_epoch)`, dh_epoch ∈ {0, 1}.


### 35.10 SIGNED_CONTROL (non-repudiation, nadir)

```
SIGNED_CONTROL = SESSION_DATA app-envelope (seq16 + ciphertext + tag16)
               + sender_ed25519_signature (64B)

signed_control_tbs (imza KAPSAMI — gerçek komutu/ciphertext'i de bağlar) =
  "SSC1"                       (4B)     ||
  protocol_version             (u8)     ||
  sender_device_cert_hash      (bytes32)||   # = SHA-256(canonical_compact_device_cert) (hangi cihaz)
  session_context_hash         (bytes32)||   # = SHA-256(session_context_v1)
  direction                    (u8)     ||
  epoch                        (u16_be) ||
  reconstructed_full_counter   (u64_be) ||   # seq16 değil, tam counter (replay/binding)
  command_type                 (u16_be) ||
  application_command_id       (bytes16)||
  SHA-256(ciphertext)          (bytes32)     # gerçek mesajı bağlar (plaintext yerine ciphertext hash)

signature = Ed25519_Sign(device_ed25519_sk, signed_control_tbs)

Kurallar:
  - Normal SESSION_DATA İMZASIZDIR. Non-repudiation YALNIZCA SIGNED_CONTROL'de.
  - sender device public key = validated CompactDeviceCert.device_ed25519_pub (Section 35.4 doğrulanmış).
  - AEAD tag HÂLÂ zorunlu (confidentiality/integrity); imza non-repudiation EKLER.
  - imza yalnızca context'i DEĞİL gerçek komutu/ciphertext'i de bağlar (SHA-256(ciphertext)).
  - device_cert_hash imza context'ine DAHİL (hangi cihazın imzaladığı bağlı).
  - rare / command / receipt / legally-significant action için. Hot-path DEĞİL.
```

### 35.11 Substrate Pozisyonu (TLS / QUIC / Kama adaptive transport — §35 notu)

```
TLS:  Mature connection-oriented reliable byte-stream. RA/onboarding/admin/API için kullanılabilir.
      SESSION_DATA hostile async path'in YERİNE GEÇMEZ. TLS full AUTHENTICATED handshake
      (Cert+CertVerify+Finished) ile kıyaslanır, bare CH/SH ile DEĞİL.
QUIC: FORKLENMEZ. Normal/iyi ağlarda (hub↔hub / RA / onboarding) kullanılabilir.
      QUIC = ZORUNLU benchmark referansı; bir rejimde QUIC daha iyiyse o rejimde QUIC seçilir.
      YERİNE GEÇMEZ (normal ağda).
Kama adaptive transport:
      KCP çıkarıldı. Kama transport = TEK adaptif transport engine.
      Engine ARQ ve FEC mekanizmalarını İÇERİR. Recovery stratejisi fuzzy controller ile seçilir:
        ARQ / FEC / hybrid / none.
      Seçim ölçülen bandwidth, packet loss, jitter, queue delay, delivered-rate confidence ve
      path health'e bağlıdır. Transport application-level rollere (hot-path/bulk/file/chat/control/
      SESSION_DATA) göre DALLANMAZ.
      CCR §35 yalnızca canonical Session Model V3 byte'larını tanımlar; transport policy
      transport-layer-tdd.md'nindir (byte-level authority transport'u yeniden tasarlamaz).
```

> **TLS handshake kıyası (P1-8, NORMATİF dürüstlük — tek sayı KULLANMA):** TLS full authenticated handshake boyutu **deployment-bağımlıdır**: tipik X.509 cert chain ile çoğu zaman KB-ölçeğinde; PSK / raw public key / certificate compression / cached cert ile çok daha küçük olabilir. SESSION_INIT_REF/INLINE **seçilen TLS profiline** karşı kıyaslanır. Bare ClientHello+ServerHello **non-equivalent lower bound**'dur (full endpoint authentication içermez). Established veri yolu kıyası: SESSION_DATA/UDP/IPv4 ~50B vs established TLS/TCP/IPv4 ~62B → ~12B/paket. "TLS'i byte'ta ezdik" DENMEZ; asıl üstünlük: one-time bootstrap amortization, offline/store-and-forward, no per-message cert/signature, no stream dependency.

### 35.12 Parser Reject Kuralları (Session Model V3)

```
- SESSION_DATA link_id taşıyorsa → reject;  length taşıyorsa → reject;  cert_ref taşıyorsa → reject.
- SESSION_DATA imza taşıyıp SIGNED_CONTROL değilse → reject.
- SESSION_DATA unknown hub_handle → LINK_ROUTE_MISS / cheap-drop (kriptografik accept DEĞİL).
- seq16 duplicate / outside replay window / ambiguous wrap → reject (wrap → rekey gerekir).
- AEAD fail → reject, replay window GÜNCELLENMEZ.
- all-zero X25519 DH → reject (Section 6.x low-order/all-zero kontrolü).
- cert chain invalid / dest_hash mismatch / static_x25519 cert'e bağlı değil → reject.
- RESPONSE öncesi epoch_1 SESSION_DATA → defer/reject; RESPONSE sonrası epoch_0 → **grace'e tabi** — yalnız implicit confirmation + bounded grace sonrası (epoch-0 receive state discard edildikten sonra) reject [P0-9; otorite kama-noise-wire §4].
- deprecated ASYNC_* (SAI1/SAA1/SAD1/SAC1) aktif wire → reject; eski SRAM/Noise-N/per-message Ed25519 → reject.
```

### 35.13 Acceptance Criteria (Session Model V3 — AC-A4 ÖLDÜRÜLDÜ)

> Eski **AC-A4 ("ASYNC_DATA hot-path 39B app-envelope")** DEPRECATED (Section 34, MUST NOT IMPLEMENT). Yerine:

```
AC-SM3-1 : SESSION_DATA app-envelope fixed overhead = seq16(2B) + tag16(16B) = 18B + payload.
AC-SM3-2 : SESSION_DATA UDP payload overhead = hub_handle4 + seq16 + tag16 = 22B + payload.
AC-SM3-3 : SESSION_DATA/UDP/IPv4 = 50B + payload, layer-matched accounting'te established
           TLS/TCP/IPv4 baseline (62B) ALTINDA.
AC-SM3-4 : SESSION_DATA link_id / cert_ref / dest_hash / routing_info / explicit length / signature /
           outer AEAD tag / FEC header / transport magic TAŞIMAZ.
AC-SM3-5 : Binding kriptografik: session_context + session_key + direction + epoch + reconstructed
           full counter.
AC-SM3-6 : hub_handle yalnızca routing/demux, authentication DEĞİL.
AC-SM3-7 : seq16 full-counter reconstruction algoritması zorunlu ve test edilir.
AC-SM3-8 : ACK trafiği byte-budget testlerine dahil; ACK-only trafik SESSION_DATA byte avantajını SİLMEZ.
AC-SM3-9 : CompactDeviceCert issuer_signature, Identity/User Cert public key altında doğrulanır —
           Intermediate DEĞİL.
AC-SM3-10: dest_hash, Identity/User Cert public key'den türetilir (SHA-256(SHA-256(user_identity_ed25519_pub))) —
           device_ed25519_pub'tan ASLA.
AC-SM3-11: SESSION_INIT_RESPONSE canonical byte'ları ve state transition tanımlı (Section 35.5).
AC-SM3-12: Deprecated Async V2 formları (SAI1/SAA1/SAD1/SAC1, ASYNC_*) aktif wire parser'da REJECT.

--- CRYPTO-P0-FREEZE ZORUNLU TEST SETİ [2026-09-07] ---

KDF:
  V-KDF-1  exact init_root (D1)
  V-KDF-2  direction-separated epoch-0 keys; k_A2B_e0 != k_B2A_e0 (D2)
  V-KDF-3  exact session_id_internal; L=16 ≡ Trunc128(L=32) (D3)
  V-KDF-4  exact epoch_1_root + epoch separation; k_*_e1 != k_*_e0 (D4/D5)
  V-KDF-5  k_init != k_A2B_e0 ve k_resp != k_B2A_e1 (D6/D7 domain separation)
  V-KDF-6  NEGATIF: HMAC-BLAKE2s-256 != native keyed BLAKE2s
  V-KDF-7  bağımsız implementasyon eşitliği (üçüncü kaynak; libsodium HKDF-BLAKE2s SAĞLAMAZ)

SSI1:
  exact AAD (üç cert_mode) · exact nonce · exact ciphertext · exact tag
  flags bit flip -> AEAD fail · init_counter bit flip -> AEAD fail
  "SSI1" byte'ları wire'da BEKLENMEZ (parser SSI1 aramaz)

SSR1:
  exact AAD · exact nonce · exact ciphertext · exact tag
  responder_ephemeral all-zero -> reject · response_counter mutation -> AEAD fail
  "SSR1" byte'ları wire'da BEKLENMEZ · wire session_id lookup YOK

REPLAY (security-critical regression — P0-6):
  forged paket: counter görülmemiş, AEAD FAIL
    ASSERT replay bitmap DEĞİŞMEDİ ve highest/expected DEĞİŞMEDİ
  sonra AYNI full_counter ile GEÇERLİ paket
    ASSERT kabul edildi
  + duplicate valid reject · too-old reject · future-gap reject · seq16 wrap
  + half-span / tie kuralı · yanlış direction fail · yanlış epoch fail
  + AEAD failure ASLA replay commit etmez · underflow/wrap aritmetiği (P0-7)

EPOCH TRANSITION (P0-9):
  RESPONSE normal teslim : Alice epoch1'e geçer · Bob ilk epoch1'e kadar epoch0 kabul eder ·
                           ilk GEÇERLİ epoch1 -> implicit confirmation -> grace başlar ·
                           grace sonrası epoch0 reject
  RESPONSE kayıp         : Alice epoch0'da kalır · Bob epoch0 accept'i korur · session ÖLMEZ
  Reordering             : grace İÇİNDE geç epoch0 kabul · grace SONRASI geç epoch0 reject
  Forgery (SECURITY)     : AEAD-geçersiz epoch1 paketi implicit confirmation OLUŞTURAMAZ,
                           grace BAŞLATAMAZ, state transition YAPAMAZ
AC-SI-1  : SESSION_INIT uç doğrulama zorunlu (cert chain→pinned Root + dest_hash + device_x25519 binding).
AC-SI-2  : SESSION_INIT reference-first; cache-hit'te full cert gönderilmez; bundle hard-max + MTU policy.
AC-SM3-13: Alpha LINK_ROUTE_MISS recovery = SESSION_INIT_REF rebind; tanımsız SESSION_RESUME frame YOK.
AC-SM3-14: SESSION_INIT.flags canonical bit allocation; cert_mode ∈ {00,01,10}; 11/reserved/init_variant≠00 → reject.
```

### 35.14 Test Corpus

```
VALID golden vectors: SESSION_INIT_REF | SESSION_INIT_INLINE_COMPACT | SESSION_INIT_RESPONSE |
  SESSION_DATA (minimal payload) | SESSION_DATA (batched payload) | SIGNED_CONTROL.

INVALID (negative) corpus:
  SESSION_DATA carries link_id → reject
  SESSION_DATA carries explicit length → reject
  SESSION_DATA carries cert_ref → reject
  SESSION_DATA carries signature but not SIGNED_CONTROL → reject
  SESSION_DATA carries FEC/outer-AEAD header → reject (established thin path)
  unknown hub_handle → cheap-drop / LINK_ROUTE_MISS (NOT crypto accept)
  seq16 duplicate / outside replay window / too far future / ambiguous wrap → reject (wrap→rekey)
  AEAD fail → reject, replay window NOT updated
  CompactDeviceCert verified with Intermediate key → FAIL (must verify under Identity/User Cert)
  CompactDeviceCert issuer_signature not valid under Identity/User Cert → reject
  dest_hash derived from device_ed25519_pub → reject (must be from user_identity_ed25519_pub)
  DiscoveryRecord.dest_hash mismatch with Identity/User Cert → reject
  current_device_x25519 mismatch with CompactDeviceCert.device_x25519_pub → reject
  all-zero X25519 public key / DH all-zero shared secret → reject
  responder_ephemeral all-zero → reject
  RESPONSE öncesi epoch_1 SESSION_DATA → defer/reject; RESPONSE sonrası epoch_0 → GRACE'E TABİ
  (yalnız implicit confirmation + bounded grace sonrası reject; erken-reject testi YAZILMAZ — P0-9)
  deprecated SAI1/SAA1/SAD1/SAC1 active wire → reject; old ASYNC_*/SRAM/Noise-N/per-message Ed25519 → reject
  SESSION_INIT flags cert_mode=11 → reject
  SESSION_INIT flags reserved bits (4..7) non-zero → reject; init_variant != 00 → reject
  SESSION_INIT cert_mode=00 ama sender_cert_ref != 32B → reject
  SESSION_INIT cert_mode=01 ama compact cert != 152B → reject
  SESSION_INIT cert_mode=10 malformed DeviceCertBundle → reject
  SESSION_DATA batch subframe LEB128/protobuf varint kullanıyor → reject
  unknown hub_handle → LINK_ROUTE_MISS / cheap-drop (crypto accept DEĞİL)
  LINK_ROUTE_MISS sonrası tanımsız SESSION_RESUME frame → Alpha'da reject
  hub_handle collision → allocation_attempt ile retry (aynı input aynı output olmamalı)
```
> **Batching ↔ implicit-length (P1-6 / madde-5, NORMATİF — VARINT YOK):** batch tek AEAD birimi; alt-mesaj framing ciphertext İÇİNDE (deterministic, wire-overhead değil). **LEB128/protobuf-style varint Session Model V3'ün HİÇBİR yerinde (encrypted subframe dahil) KULLANILMAZ.** Canonical iç framing:
```
# [AMENDMENT-6 sahiplik notu, 2026-09-18] Bu yapı, seam'in seal()'e ALDIĞI plaintext birimin çerçevesidir; birimi
# TRANSPORT üretir (batching transport'un). **CANONICAL = BU YAPI** (KM-SEAM-TC-001 KARAR-1 = (a), 2026-09-18);
# kodda henüz yok (interim client-core wire.rs `[type][seq u16][unit_id u64]` 11 B çerçeve emekliye ayrılır, transport v1.x).
# `payload` içi → application-message-protocol.md (Application'ın).
BatchPayload {
  submsg_count : u8
  submsg[i]    : BatchSubMessage[submsg_count]
}
BatchSubMessage {
  len_mode : u8
    0x00..0xEF = inline length 0..239
    0xF0       = extended u16_be length follows
    0xF1..0xFF = reserved → reject
  if len_mode == 0xF0: length : u16_be
  payload : bytes[length]
}
```
> Tek seq16, tek tag (batch tek AEAD birimi). Düşük bant için bu kompakt form **TEK canonical encoding'dir** (KM-SEAM-TC-001 KARAR-1, 2026-09-19). ~~Alternatif basit form (`submsg_count u8` + her biri `u16_be len + payload`) da geçerli~~ — **SUPERSEDED**: bir canonical spec'te iki geçerli encoding olamaz; alternatif form REJECT edilir (len_mode 0xF1..0xFF gibi). Varint YOK, deterministic parser VAR.

> **Sıkıştırma (P1-7; sahiplik app-rpc §3 [AMENDMENT-4], wire sözleşmesi application-message-protocol.md §5; transport-tdd §8 normatif değil — özet):** Sıkıştırma policy-controlled + thresholded + **context-isolated** (per-message/per-batch izole bağlam). Saldırgan-kontrollü plaintext ile gizli sır AYNI sıkıştırma bağlamında KARIŞTIRILMAZ. Eşik altı küçük mesaj (expansion riski) sıkıştırılMAZ. Ses/video çift-sıkıştırma YOK. Uzunluk-leak metadata olarak belgelenir (CRIME E2E'de geçersiz ama side-channel tümüyle yok sayılmaz).

### 35.15 Hub Session State DoS Koruması (NORMATİF, CAD-57)

```
- Hub session/mailbox state SESSION_INIT ile yaratılır → unauthenticated INIT flood DoS riski.
- Mitigasyon: INIT'in hub'a maliyeti DÜŞÜK tutulur (session state ancak ilk geçerli SESSION_DATA
  veya RESPONSE sonrası tam allocate); yarım-açık session TTL + cap; per-source rate-limit;
  hub_handle ancak doğrulanmış akışta kalıcılaşır.
- Hub INIT içeriğini açamaz (E2E) ama recipient_dest_hash routing + admission (Root CA join gate) görür.
```

> **[KARAR-SLOTU KS-DOS-1 — 2026-09-07, değer donmadan implementasyon başlamaz]** Yukarıdaki
> "TTL + cap + per-source rate-limit" isimli ama DEĞERSİZ. Üç sabit birlikte donar:
> ```
> HALF_OPEN_TTL              = [KARAR]
> HALF_OPEN_CAP              = [KARAR]   # eşzamanlı yarım-açık session
> PER_SOURCE_INIT_RATE       = [KARAR]
> PENDING_RESPONSE_TRIAL_CAP = [KARAR]   # Alice tarafı: RESPONSE demux'ta denenecek
>                                        # pending session üst sınırı (§35.5 demux notu)
> ```
> **Boyutlandırma kuralı (NORMATİF):** `HALF_OPEN_CAP`, saldırgan modeline değil **dürüst retry
> profiline** göre taban alınır — %90 kayıpta meşru istemci INIT'i cached byte-for-byte defalarca
> gönderir; cap küçük seçilirse MEŞRU retry'lar meşru kayıtları düşürür. Saldırgan sınırlaması
> `PER_SOURCE_INIT_RATE` ile yapılır.
> **`PENDING_RESPONSE_TRIAL_CAP` neden burada:** demux yazılmadan varsayılan davranış "pending
> kümesinde dene-çöz" olur; cap yoksa her sahte RESPONSE N×(2 X25519) maliyeti doğurur
> (amplifikasyon). Cap bu ailenin parçasıdır.
> **SEMANTİK (NORMATİF, 2026-09-19 — değerden bağımsız donar; kalan yalnız kalibrasyon):**
> - `HALF_OPEN_TTL`: hub'ın yarım-açık session kaydını tuttuğu **monotonic** süre; dolunca kayıt düşer, sonraki
>   INIT yeni kayıt açar (replay kümesi ayrı, §35.8 — TTL replay korumasını GEVŞETMEZ).
> - `HALF_OPEN_CAP`: **per-source** sınırdır, global değil (global cap bir saldırganın tüm hub'ı kilitlemesine izin
>   verir). Dolunca **en eski yarım-açık kayıt düşürülür** (drop-oldest), yeni INIT reddedilmez — dürüst retry'lar
>   kendi eski kayıtlarını yeniler, saldırgan ise yalnız kendi kotasını tüketir. Ek **global soft-cap** yalnız
>   bellek koruması içindir ve per-source'tan büyük olmalıdır.
> - `PER_SOURCE_INIT_RATE` — **"source" tanımı:** bind ÖNCESİ (CLIENT_BIND yok) = UDP 4-tuple; NAT arkası çok
>   istemci aynı tuple'ı paylaşabilir (adil olmayan ama güvenli — kota paylaşılır, bypass edilemez). Bind SONRASI =
>   doğrulanmış client identity (dest_hash); spoof edilemez. IP spoofing ile kota bypass'ı Plane 0 Noise handshake
>   tamamlanmadan INIT kabul edilmediği için mümkün değildir (INIT yalnız Plane 1 üzerinden). Aşımda INIT sessiz
>   düşer (drop, reject cevabı YOK — amplifikasyon vermez).
> - `PENDING_RESPONSE_TRIAL_CAP`: **per-peer** (Alice'in Bob'a açık pending sayısı) VE sahte-RESPONSE başına deneme
>   üst sınırı; deneme sırası en-yeni-önce (KM-SEAM-TC-001 K7); cap aşımında `Failed` (opak). Değeri
>   `HALF_OPEN_CAP` ile aynı ailede kalibre edilir (aynı dürüst-retry profili).
> Bu dört semantik donmuştur; `[KARAR]` işaretleri yalnız **sayısal kalibrasyon** ister.

### 35.16 LINK_ROUTE_MISS + Rebind (NORMATİF, CAD-57)

```
Alpha AYRI bir lightweight SESSION_RESUME wire formatı TANIMLAMAZ.
LINK_ROUTE_MISS / unknown hub_handle recovery = SESSION_INIT_REF rebind.

On LINK_ROUTE_MISS (hub restart / TTL expiry / unknown hub_handle):
  - sender unknown hub_handle'ı KULLANMAYI BIRAKIR.
  - sender SESSION_INIT_REF gönderir (ucuz, cache-hit cert ref) → session'ı mevcut hub üzerinden rebind eder.
  - receiver SESSION_INIT'i normal doğrular (Section 35.4).
  - eski hub_handle invalidate edilir.
  - lightweight resume Alpha wire object DEĞİL (Beta+ / future extension).
  - client keepalive YOK (state hub'da; miss → SESSION_INIT_REF rebind, sürekli ping değil).
```
> **NOT:** Lightweight resume Beta'da istenirse exact wire + AAD + key schedule + replay rules ZORUNLU yazılır; aksi halde isim olarak bile bırakılmaz (parser belirsizliği).

### 35.17 Invariants (Session Model V3 — NORMATİF)

| ID | Statement |
|----|-----------|
| INV-FLAGS-1 | SESSION_INIT.flags canonical bit allocation; reserved bitler 0 OLMALI; cert_mode=11/init_variant≠00/reserved≠0 → reject. |
| INV-NO-NOISE-PATTERN-FLAG | SESSION_INIT.flags generic Noise KK/IK pattern SEÇMEZ; Section 35.2 Kama SM3 key schedule otoriter. |
| INV-MAGIC-1 | Aktif Session Model V3 yalnızca SSI1/SSR1/SSD1/SSC1 kullanır. SAI1/SAA1/SAD1/SAC1 deprecated reject-only. |
| INV-BATCH-1 | Session Model V3 LEB128/protobuf varint KULLANMAZ (encrypted batch subframe dahil). |
| INV-RESUME-1 | Alpha ayrı lightweight SESSION_RESUME wire object TANIMLAMAZ; LINK_ROUTE_MISS recovery = SESSION_INIT_REF rebind. |
| INV-HANDLE-1 | hub_handle opaque, collision-checked, PRF-derived (allocation_attempt'li); authentication = AEAD/session key, handle DEĞİL. |
| INV-BIND-1 | Established binding anahtarda (session_context + key + direction + epoch + reconstructed counter); wire'da görünür bağ yok. |
| INV-CERT-1 | CompactDeviceCert issuer_signature Identity/User Cert key altında doğrulanır (Intermediate DEĞİL); dest_hash Identity key'den. |

### 35.18 Final Target Statement + Status (NORMATİF)

```
Kama async hot-path "KCP over UDP" DEĞİL, "QUIC clone" DEĞİL, "TLS reimplementation" DEĞİL.

SESSION_INIT     : one-time authenticated bootstrap; cert/reference-first; static X25519 possession;
                   endpoint-verified; hub-untrusted.
SESSION_INIT_RESPONSE : canonical FS upgrade (Section 35.5).
SESSION_DATA     : 18B E2E app-envelope + 4B hub_handle carrier = 22B UDP payload overhead;
                   no link_id, no cert, no signature, no length, no FEC, no outer AEAD.
SIGNED_CONTROL   : rare non-repudiation extension.

BYTE CLAIM (dürüst): "TLS'i byte'ta ezdik" DENMEZ. established SESSION_DATA/UDP/IPv4 ~50B+payload vs
  TLS/TCP/IPv4 ~62B+payload, ~12B daha iyi. ASIL avantaj: one-time bootstrap amortization,
  offline/store-and-forward, no per-message cert/signature, no mandatory stream connection semantics.

STATUS:
  Core architecture                : GO
  CompactDeviceCert trust model    : GO
  Session Model V3 byte accounting  : GO
  SESSION_DATA hot-path             : GO
  Parser/canonicalization blockers  : resolved
  Implementation freeze             : GO, pending generated test vectors + cross-doc identity-model reconciliation

CROSS-DOC: identity-and-trust-model.md bazı bölümlerde hâlâ historical Async Session V2 / 39B
  terminolojisi taşıyor → full cross-doc freeze ÖNCESİ Session Model V3'e reconcile EDİLMELİ.
```

## 36. Open TODOs

**İzinli (architecture-bloke etmez):**

| ID | Konu |
|----|------|
| TODO-01 | Gerçek RoftCore PEN/OID numaraları (Beta; CCR native olduğu için Alpha bağımlı değil) |
| TODO-02 | Gerçek kriptografik key'lerle production test vector'leri (placeholder → real) |
| TODO-03 | CT Log signing key lifetime (revocation-model.md'ye delege) |

**İzinli DEĞİL (bu dokümanda çözüldü):** byte order, integer encoding, TBS field order, signature input, hash input, AsyncMessage field layout (Section 35), Device CCR statik x25519 policy (CompactDeviceCert 152B, Section 13), DeviceCertBundle required fields, parser rejection behavior, AsyncMessage AAD/cipher_suite_id (Section 35.3), DiscoveryRecord binding (Section 21), HubListRecord (Section 21.5), OrgEntitlementRecord (Section 21.6), Java/Rust/C++ canonical interop, RA→CA transport boundary — **hepsi tanımlı.** [SignedNoisePrekey/PrekeyBundle DEPRECATED / MUST NOT IMPLEMENT — statik X25519]

---

**Doküman sonu — CCR Format Specification v1.0**

Bu doküman `identity-and-trust-model.md` v1.5'in (Haziran 2026 ERRATA, statik X25519 canonical) tanımladığı tüm kimlik/PKI nesnelerinin kesin canonical binary formatını verir: CCR envelope + cert tipleri (Root 20y / Intermediate 5y namespace omurgası / Identity 1y / Device 90g), canonical_ccr_tbs, **CompactDeviceCert** (~152B, statik pk_x25519, SAN/deviceId yok, dest_hash türetilir, Section 13), DeviceCertBundle (unsigned container), **DiscoveryRecord** (current_device_x25519 binding, KONUM taşımaz/home_hub_id KALDIRILDI, magic SRDR, Section 21), **HubListRecord** (Intermediate-imzalı IP SAN bootstrap, magic SRHL, Section 21.5), **OrgEntitlementRecord** (RoftCore-imzalı lisans kaydı + lifetime clamp, magic SROE, Section 21.6), **Session Model V3** (SESSION_INIT/SESSION_INIT_RESPONSE/SESSION_DATA/SIGNED_CONTROL, TLS-dersi key-binding, 18B SESSION_DATA hot-path, magic SSI1/SSR1/SSD1/SSC1, Section 35; eski Async V2 SA?1 + tek-yapı SRAM/Noise N DEPRECATED Section 35-DEPRECATED/V2). Lisans enforcement Intermediate expiry ile DEĞİL — OrgEntitlementRecord + RA license gate + Identity 1 yıl + clamp (identity v1.5 E-11). [SignedNoisePrekey/PrekeyBundle statik X25519 kararıyla DEPRECATED / MUST NOT IMPLEMENT, Section 18-19.] Tüm integer big-endian fixed-width; imzalı yapılarda varint yok; canonical input Kama-native (JSON/protobuf/DER değil). Magic domain separation (SRCR/SRDR/SRHL/SROE + Session V3: SSI1/SSR1/SSD1/SSC1 + routing: SRRR/SRAN/SRAH; SRAM + Async V2 SA?1 DEPRECATED) cross-protocol signature confusion'ı önler. Hash girdileri kesin (identity_hash ham key, subject_key_id/ROOT_CA_SPKI_HASH spki). Parser rejection kuralları tam; libFuzzer hedefleri + Java/Rust/C++ differential interop tanımlı. RA→CA boundary korunur (izole mTLS/gRPC, custom transport değil). v1.5 ile tek bilinçli fark: TBS'e domain-separation magic eklenmesi (semantik değişmez, açıkça not edildi). Mimari hiçbir karar bu dokümanda yeniden tasarlanmadı — identity-and-trust-model.md semantik otorite, bu doküman byte-level projeksiyonu.

> **STATÜ [2026-09-19] — CRYPTO-P0-FREEZE (07 Eyl) bloğu bu bloğun SUPERSEDE eder:**
> ```
> Core crypto architecture            : GO   (kanonik: App → Transport → Crypto tel kenarında → UDP; transport-tdd §0 INV-NEIGHBOR-1)
> CCR canonical codec                 : GO
> D1–D7 key schedule (§35.2.1)        : GO
> SSI1 / SSR1 / SSD1 crypto           : GO
> SESSION_DATA hot-path (seam çıktısı): GO
> Replay / nonce model                : GO   (P0-6/P0-7; PN sayacı CRYPTO'da — KM-SEAM-TC-001 K2)
> dh_epoch 0→1 transition             : GO   (kama-noise-wire §4)
> RESPONSE demux (KS-DEMUX-1)         : GO   (KM-SEAM-TC-001 K7: candidate seçimi CRYPTO içinde, cap'li; Transport dispatch
>                                              anahtarı = DELIVER/MailboxItem envelope_type, app-rpc AMENDMENT-7)
> Transport↔Crypto seam (S3) tasarımı : GO   (KM-SEAM-TC-001 K1–K9; ProtectionContext(Session|HubChannel); KARAR-1 = §35.6 BatchPayload)
> S3 SEAM-FROZEN                      : HOLD — KS-DOS-1 sabitleri (§35.15) + KS-GRACE-1 (N_g/T_g) + §35.13 vektörler + harness + stub
> Application Message Protocol v1     : HOLD — alan/limit kararları (application-message-protocol.md §8)
> Production interoperability freeze  : HOLD — vektörler + interop/adversarial testler + noise-wire D1-ENTRY (N1–N10 + POC)
> ```
> Açık product/calibration kalemleri: KS-DOS-1 (§35.15) · KS-GRACE-1 · KS-REV-1 (revocation-model, K10 ile) · KS-RA-1 (pki).
> ~~S2-noise.2~~ ve ~~CRY-1~~ kapandı (S3 K1/K2). ~~KS-ACK-1~~ söküldü (app-rpc MAILBOX_PRUNE).

---

## 35-DEPRECATED. Eski AsyncMessage (Tek-Yapı, İmzalı) — MUST NOT IMPLEMENT

> **⚠️ DEPRECATED (CAD-54, Haziran 2026):** Aşağıdaki tek-yapı imzalı AsyncMessage modeli (her mesajda full dest_hash + ephemeral + cert + 64B Ed25519 imza ≈200B metadata) **stateless-per-message** olduğu için düşük-bant bütçesini öldürüyordu. Session Model V3 (Section 35: SESSION_INIT/RESPONSE/DATA/SIGNED_CONTROL + amortize link + hub state) ile DEĞİŞTİRİLDİ. **Aktif wire'da KULLANILMAZ.** Parser bu yapıyı aktif path'te görmemeli. Yalnızca tarihsel referans.

<details>
<summary>Eski AsyncMessage tek-yapı modeli (tarihsel, MUST NOT IMPLEMENT)</summary>

**[DEPRECATED] Eski AsyncMessage (Asenkron Store-and-Forward Mesaj)**

> **Mimari bağlam (Haziran 2026):** Kama Alpha mesajlaşması ASENKRON store-and-forward'dur (zero-trust + hostile-optimized). Alice mesajı hub'a bırakır (tek yönlü, round-trip YOK), Bob istediği zaman çeker. Handshake YOK — Alice, Bob'un statik `pk_device_x25519`'unu discovery'de aldığı için Noise N one-way ile sıfır-RTT şifreli mesaj gönderir. Zero-RTT async delivery, ilk mesaj öncesi interaktif session setup (TLS handshake gibi) gereksinimini ortadan kaldırır — hostile ağda interaktif 2-RTT handshake tamamlanamayacağı için kritik.

### D1.1 Wire Format

```
AsyncMessage {
  ── dış katman (hub görür — routing + transport) ──
  recipient_dest_hash    : bytes32      # alıcı (hub teslim için; metadata hub'a güven, kabul)
  ── kriptografik gövde (TBS) ──
  msg_tbs {
    cipher_suite_id         : u16_be     # AEAD/DH/imza süit kimliği (downgrade koruması; AAD'ye dahil)
    sender_ephemeral_x25519 : bytes32    # Alice'in bu mesaj için TAZE ephemeral (forward secrecy)
    message_counter         : u64_be     # monoton (replay + ratchet senkronu, çift görev)
    timestamp               : u64_be     # tazelik (replay penceresi)
    ciphertext              : varbytes32  # Noise N AEAD: k=DH(sender_eph, recipient_static_x25519)
  }
  ── kimlik (reference-first) ──
  sender_cert_mode       : u8           # 0x00 = cert_hash ref (32B), 0x01 = full cert (152B)
  sender_cert            : bytes32 (mode 0x00) | CompactDeviceCert ~152B (mode 0x01)
  ── imza ──
  sender_signature       : bytes64       # Ed25519_Sign(sk_device_ed25519, msg_tbs || sender_cert_field)
}
```

> **cipher_suite_id (NORMATİF):** Crypto-agility için süit kimliği. Alpha tek süit: `0x0001` = `Noise_N_25519_ChaChaPoly_BLAKE2s` (Ed25519 imza + X25519 DH + ChaCha20-Poly1305 AEAD). cipher_suite_id hem `msg_tbs`'e (imza korur) hem AEAD AAD'ye (downgrade önler) dahildir. Beta+ ML-KEM hibrit süitleri buraya eklenir (`0x0002+`). Bilinmeyen süit → reject.

### D1.2 canonical_async_message_tbs

```
msg_tbs (imzalanan) =
  "SRAM" (magic, 4B, domain separation) ||
  recipient_dest_hash ||
  cipher_suite_id (u16_be) ||
  sender_ephemeral_x25519 || message_counter || timestamp ||
  ciphertext (varbytes32: len || bytes) ||
  sender_cert_mode || sender_cert (mode'a göre 32B veya tam CompactDeviceCert byte'ları)

sender_signature = Ed25519_Sign(sk_device_ed25519, msg_tbs)
```
`sender_signature` TBS'e **dahil değildir**. `recipient_dest_hash` ve `cipher_suite_id` imzaya DAHİLDİR (hub yeniden hedefleyemez/downgrade yapamaz).

> **Magic-in-TBS (NORMATİF, Section 1):** `msg_tbs` magic="SRAM" ile başlar — `canonical_ccr_tbs`("SRCR"), `dr_tbs`("SRDR"), `canonical_entitlement_tbs`("SROE") ile aynı domain-separation disiplini.

### D1.3 AEAD Associated Data (AAD) — Noise N (NORMATİF, identity v1.5 §10.6)

Noise N ciphertext üretilirken AEAD'nin associated data'sı **en az** şunları bağlar:

```
AAD =
  protocol_magic        ("SRAM", 4B)
  version               (u8)
  cipher_suite_id       (u16_be)
  recipient_dest_hash   (32B)
  sender_device_cert_hash (32B)   # mode 0x00'da ref; mode 0x01'de SHA-256(gönderilen cert)
  message_counter       (u64_be)
  timestamp             (u64_be)
```

> **Binding sonucu:** hub mesajı başka alıcıya yönlendiremez (recipient_dest_hash AAD'de → AEAD decrypt başarısız), downgrade yapamaz (cipher_suite_id AAD'de), replay/reorder tespit edilir (counter+timestamp AAD'de + §35.5), sender cert değiştiremez (sender_device_cert_hash AAD'de + Ed25519 imza). AAD alanları `msg_tbs` ile tutarlı olmalı (cipher_suite_id, recipient_dest_hash, counter, timestamp her ikisinde de aynı); doğrulayıcı tutarsızlık → reject.

### D1.4 Güvenlik Özellikleri (CIA + ekstra)

```
AUTHENTICITY:   sender_signature = sk_device_ed25519 imzası. Saldırgan Bob'un public cert'ini
                hash'leyip sahte mesaj koyamaz → sk_device YOK → imza tutmaz → Bob reddeder.
INTEGRITY:      msg_tbs imzalı → hub değiştiremez. recipient imzada → hub yeniden hedefleyemez.
CONFIDENTIALITY: ciphertext = Noise N (k=DH(sender_eph, recipient_static_x25519)).
                Sadece Bob (recipient_static private) + Alice (ephemeral) açar.
                SINIRI: Bob'un static x25519 sızarsa bekleyen async mesajlar açılır (kabul,
                cihaz ele geçirme kapsam dışı; ephemeral×static tek DH, offline alıcı).
AVAILABILITY:   hub mesajı SİLEBİLİR/geciktirebilir (routing residual, kabul; Tor bad-guard sınıfı).
                N-mesaj-ACK penceresi (identity v1.5) sessiz kaybı yakalar.
NON-REPUDIATION: Ed25519 imza her mesajda — gönderen inkar edemez.
```
> **Ek HMAC kullanılmaz.** Mesaj bütünlüğü ve orijinalliği şunlarla sağlanır: (1) AEAD authentication tag (ciphertext + associated data üzerinde), (2) Ed25519 sender signature (canonical msg_tbs üzerinde), (3) strict domain separation (protocol_magic "SRAM" / version / cipher_suite_id), (4) recipient binding (recipient_dest_hash hem imzalı metadata'da hem AEAD AAD'de). Üç ayrı koruma katmanı → ek simetrik HMAC gereksiz.

### D1.5 Reference-First Cert + Hostile Cache-Miss Policy (NORMATİF)

```
AsyncMessage sender cert reference policy:

İlk temas / bilinmeyen sender (mode 0x01):
  AsyncMessage, validation için yeterli inline CompactDeviceCertV1 (152B) VEYA DeviceCertBundle
  İÇERMELİ — alıcı sender device_cert_hash'i zaten cache'te tutmuyorsa.

Alıcı cache'i sıcaksa (mode 0x00):
  AsyncMessage yalnızca sender_device_cert_hash (32B) kullanabilir.

Hostile / degraded network cache-miss (mode 0x00 ama alıcıda yok):
  Alıcı FAIL-CLOSED olmalı VEYA DeviceCertBundle gelene dek validation'ı ertelemeli.
  Alıcı çıplak device_id'ye veya sender iddialarına GÜVENMEMELİ.

Mesaj öncesi ZORUNLU canlı fetch YOK:
  Async path, bir mesajı işlemeden önce RA/hub/prekey-store ile iletişim GEREKTİRMEMELİ.
  (Cache-miss'te hub'dan cert almak OPSİYONEL kolaylıktır — iyi internet varsa; zorunlu değil.
   Hub erişilemezse mesaj ertelenir/fail-closed, ama protokol hub'a bağımlı DEĞİL.)

CERT ROTATION (90 gün): yeni cert → yeni device_cert_hash → alıcı cache-miss → ilk mesajda mode 0x01.
```
> **Low-bandwidth nüansı:** İlk-temas maliyeti (152B) KABUL edilir; tekrarlı mesajlar 32B `device_cert_hash` referansı kullanır. `device_cert_hash` = kriptografik BINDING (SHA-256 collision-resistant), etiket değil.

### D1.5b AsyncMessage Sender Validation (CompactDeviceCert düzeltmesine bağlı)

```
sender cert ref: inline CompactDeviceCertV1 (mode 0x01) VEYA device_cert_hash (mode 0x00)

Validation:
  1. validated CompactDeviceCertV1'i çöz (inline veya cache; cache-miss → fail-closed/defer)
  2. issuer Identity/User Cert chain doğrula (Identity/User → Intermediate → Root)
  3. sender identity'yi issuer Identity/User Cert public key'inden TÜRET:
       sender_identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub)
       sender_dest_hash     = SHA-256(sender_identity_hash)
       (device_ed25519_pub'tan DEĞİL)
  4. sender_signature'ı CompactDeviceCert.device_ed25519_pub ile doğrula
  5. recipient_dest_hash == local dest_hash (recipient confusion önleme)
  6. AEAD AAD, recipient_dest_hash + sender_device_cert_hash + cipher_suite_id içermeli (§35.3)
  7. replay check key = (sender_identity_hash, sender_device_cert_hash)
       çıplak device_id replay/auth key OLAMAZ
  8. X25519 DH: shared_secret all-zero → reddet
```
> **replay_state_key = (sender_identity_hash, sender_device_cert_hash)** — identity v1.5 §15.6 ile birebir. Sender authentication device `pk_ed25519` imzasıyla; sender identity issuer Identity/User Cert'ten. İkisi ayrı: device key imzalar, user key kimliği belirler.

### D1.6 Replay Koruması (alıcı tarafı, identity v1.5 ile uyumlu)

```
Bob tutar (per sender identity_hash):
  - highest_counter_seen
  - sliding_window bitmap (512-1024, hostile sırasız teslimat için)
  - timestamp penceresi (max_age 7 gün, S&F TTL ile uyumlu)

Gelen mesaj:
  counter > highest               → kabul, window kaydır, bitmap işaretle
  counter window içinde + bit YOK → kabul (sırasız ama geçerli), bit işaretle
  counter window içinde + bit VAR → REPLAY → reddet
  counter window'dan eski         → reddet (çok geç)
  timestamp > max_age             → reddet (bounded, DoS yok)
İlk mesaj (highest yokken): ATOMİK counter check — counter=1'i işle+kaydet,
  sonraki counter=1 (replay) RAM'de bile reddet (race condition önlenir).
```

> Tam asenkron oturum lifecycle, ratchet senkronu, N-mesaj-ACK penceresi → `identity-and-trust-model.md` v1.5 ve `discovery-architecture.md`.

---

</details>

---

## Appendix D2 — Deprecated Async Session V2 / Historical Only

> **THIS APPENDIX IS HISTORICAL.** Active parsers MUST reject these forms. No implementation may instantiate these structs except negative tests. All SAI1/SAA1/SAD1/SAC1 forms are obsolete. The current active model is **Session Model V3: SSI1 / SSR1 / SSD1 / SSC1** (Section 35).

```
Historical Async Session V2 attempted:
  ASYNC_INIT → ASYNC_ACCEPT → ASYNC_DATA
  39B app-envelope hot-path
  link_id16 + seq4 + len2

It was replaced by Session Model V3:
  SESSION_INIT → SESSION_INIT_RESPONSE → SESSION_DATA
  18B app-envelope hot-path
  hub_handle (L2) + seq16 + tag16
```

> **KATMAN UYARISI (NORMATİF):** Bu bölümdeki tüm byte rakamları **application-layer async envelope**'dır. Transport framing, KCP/PDP header, UDP/IP header, retransmission, opsiyonel FEC ve link-layer overhead'i HARİÇTİR (`transport-layer-tdd.md`). `ASYNC_DATA ≈ 39B` full wire maliyeti DEĞİLDİR — yalnızca app-layer envelope + AEAD tag.

### D2.1 Üç Mesaj Tipi — Genel Bakış

```
ASYNC_INIT   (magic "SAI1"): self-contained bootstrap. Bob online olmak zorunda değil.
             Alice, Bob'un static X25519'unu discovery cache'ten bilir → handshake YOK.
             full recipient_dest_hash + sender_ephemeral + sender cert (ref|inline-compact|bundle) taşır.
             link_id + init_root burada kurulur. Pahalı ama konuşma başına 1 kez.

ASYNC_ACCEPT (magic "SAA1"): Bob INIT'i mailbox'tan alınca session'ı AKTİVE eder.
             Real-time olmak zorunda değil (store-and-forward olabilir).
             Alpha default = STRONGER: Bob ephemeral ZORUNLU (forward secrecy upgrade).
             link_id (INIT'ten) + bob_ephemeral taşır. Konuşma başına 1 kez.

ASYNC_DATA   (magic "SAD1"): established session hot-path. ASIL ÜRÜN HOT-PATH'İ.
             SADECE link_id + seq + flags + len + ciphertext + AEAD tag.
             recipient_dest_hash YOK, cert YOK, ephemeral YOK, timestamp YOK, imza YOK.
             ≈39B app-envelope. Her mesaj.

SIGNED_CONTROL_MESSAGE (magic "SAC1"): non-repudiation gereken kritik mesaj (Section 35.9).
             ASYNC_DATA + sender_signature(64B) + full sender context. NADİR (her chat değil).
```

### D2.2 Authenticated One-Way Key Schedule (CAD-55)

Saf Noise N (sender anonim → ayrı 64B imza) YERİNE Kama **authenticated one-way** key schedule. Sender authentication AEAD key derivation'a gömülür; ayrı per-message imza YOK.

```
# ASYNC_INIT root (Alice → Bob):
ss_e      = DH(sender_ephemeral_sk, recipient_static_x25519_pub)   # forward secrecy
ss_s      = DH(sender_static_x25519_sk, recipient_static_x25519_pub) # sender AUTH (possession)
init_root = HKDF("SR-ASYNC-INIT-V2", ss_e || ss_s, session_context_v1)

# session_context_v1 (session-wide HKDF binding) — EXACT BYTE LAYOUT (blocker-3 DÜZELTME, E-12):
# DÜZELTME GEREKÇESİ (3 sorun çözüldü):
#   (1) message_type ÇIKARILDI — session root context INIT/ACCEPT/DATA arasında AYNI olmalı.
#       message_type sadece message-specific AAD'de kalır (Section 35.2 AAD layouts).
#   (2) recipient_discovery_record_hash ÇIKARILDI — Bob her zaman aynı dr_tbs'i reconstruct edemez
#       (Alice eski-ama-geçerli DiscoveryRecord'dan göndermiş olabilir → false decrypt fail).
#       Yerine recipient_static_x25519_hash: hem Alice (discovery cache) hem Bob (kendi static key)
#       deterministik bilir → mismatch kaynaklı false-negative ELENİR; cert/x25519 swap binding korunur.
#   (3) policy_sequence ÇIKARILDI — Alpha'da profile_id + cipher_suite_id SABİT, negotiation YOK,
#       downgrade yüzeyi YOK (Section 35.7). İki tarafın aynı policy_sequence'i kesin bildiği garanti
#       değildi (wire'da taşınmıyordu) → decrypt fail riski. Beta policy binding için Section 35.7 B-opsiyonu.
session_context_v1 =
  protocol_version              (u8,      1B)   ||   # 0x01 (Alpha)
  profile_id                    (u8,      1B)   ||   # 0x01 = ASYNC_PROFILE_V2_DEFAULT (Alpha sabit)
  cipher_suite_id               (u16_be,  2B)   ||   # 0x0001 (aşağıda tam tanım; Alpha sabit)
  sender_cert_ref               (bytes32, 32B)  ||   # = SHA-256(canonical_compact_device_cert) (ref/inline AYNI)
  recipient_dest_hash           (bytes32, 32B)  ||   # tam dest_hash
  recipient_static_x25519_hash  (bytes32, 32B)       # = SHA-256(recipient_static_x25519_pub)
# Toplam session_context_v1 = 1+1+2+32+32+32 = 100B (HKDF info girdisi; wire'da taşınmaz)
# recipient_static_x25519_hash = SHA-256(recipient_static_x25519_pub):
#   Alice bu değeri discovery/cache'ten bilir (DiscoveryRecordV2.current_device_x25519);
#   Bob kendi static key'inden deterministik üretir. Cert/x25519 swap'ı pinler (downgrade/retarget koruması)
#   AMA DiscoveryRecord versiyonuna bağlı OLMADAN → false decrypt fail oluşmaz.

# link_id (CAD-56) — dest_hash'ten TÜREMEZ (correlation önleme):
link_id   = Trunc128(HKDF("SR-LINK-ID-V1", init_root, session_context_v1))   # 16B

# ── EXACT AEAD AAD LAYOUTS (blocker-4) ──
# NORMATİF: message_type session_context'te DEĞİL, her mesaj tipinin kendi AAD magic'inde (domain separation).
# ASYNC_INIT_AAD:
ASYNC_INIT_AAD =
  "SAI1"                        (4B)   ||
  protocol_version              (u8,  1B)   ||
  flags                         (u8,  1B)   ||
  cipher_suite_id               (u16_be, 2B) ||
  link_id                       (16B)  ||
  recipient_dest_hash           (32B)  ||
  sender_cert_ref               (32B)  ||
  sender_ephemeral_x25519       (32B)  ||   # INIT public ephemeral → AAD binding (açık + test edilebilir)
  message_counter               (u64_be, 8B) ||
  timestamp                     (u64_be, 8B)
# = 4+1+1+2+16+32+32+32+8+8 = 144B

# ASYNC_ACCEPT_AAD:
ASYNC_ACCEPT_AAD =
  "SAA1"                        (4B)   ||
  protocol_version              (u8,  1B)   ||
  flags                         (u8,  1B)   ||
  cipher_suite_id               (u16_be, 2B) ||
  link_id                       (16B)  ||
  bob_ephemeral_x25519          (32B)  ||
  message_counter               (u64_be, 8B)
# = 4+1+1+2+16+32+8 = 64B

# ASYNC_DATA_AAD (SAD1 yalnızca AAD domain separator, wire'da YOK):
ASYNC_DATA_AAD =
  "SAD1"                        (4B)   ||
  flags                         (u8,  1B)   ||   # epoch+direction+message_type bitleri
  link_id                       (16B)  ||
  seq                           (u32_be, 4B)
# = 4+1+16+4 = 25B

# ── cipher_suite_id 0x0001 TAM TANIM (blocker-5) ──
# cipher_suite_id 0x0001 = SR_ASYNC_V2_25519_CHACHAPOLY_BLAKE2S:
#   DH:        X25519 (Curve25519, RFC 7748); all-zero/low-order output → reject
#   KDF:       HKDF-BLAKE2s (RFC 5869 yapısı, hash=BLAKE2s-256)
#              salt   = sabit "SR-ASYNC-V2-SALT" (16B ASCII) — tüm async KDF çağrıları için
#              info   = ilgili label ("SR-ASYNC-INIT-V2" / "SR-ASYNC-SESSION-V2" /
#                       "SR-DATA-A2B-E0" / "SR-DATA-A2B-E1" / "SR-DATA-B2A-E1" /
#                       "SR-LINK-ID-V1") || session_context_v1 (uygulanan yerde)
#              output = 32B (key) veya 16B (link_id Trunc128)
#   AEAD:      ChaCha20-Poly1305 (RFC 8439); key 32B, nonce 12B, tag 16B
#              nonce  = per_message_key türetiminde HKDF'ten 12B (key 32B || nonce 12B = 44B output)
#              AAD    = yukarıdaki exact layout (mesaj tipine göre)
#   Bilinmeyen cipher_suite_id → reject (Section 35.7 downgrade).

# ASYNC_ACCEPT (Bob → Alice, Alpha default STRONGER — CAD-55 blocker-3 düzeltmesi):
# Yeterli FS upgrade için ephemeral×ephemeral ZORUNLU (tek ephemeral×static YETERSİZ):
ss_ee2    = DH(alice_init_ephemeral_sk, bob_ephemeral_pub)   # ephemeral×ephemeral (ASIL FS upgrade)
ss_se2    = DH(alice_static_x25519_sk, bob_ephemeral_pub)    # static×ephemeral (ZORUNLU — Alpha tek profil; blocker-6)
session_root = HKDF("SR-ASYNC-SESSION-V2", init_root || ss_ee2 || ss_se2, session_context_v1)
# Alice, init_ephemeral_sk'i ACCEPT gelene dek SAKLAR, session_root türetince SİLER (FS).
# Alpha default profilinde ss_ee2 VE ss_se2 İKİSİ DE ZORUNLU (mandatory); biri eksikse fail-closed.
# Bob tarafı simetrik: ss_ee2 = DH(bob_ephemeral_sk, alice_init_ephemeral_pub),
#                      ss_se2 = DH(bob_ephemeral_sk, alice_static_x25519_pub).

# ASYNC_DATA epoch'lu chain (CAD-54 blocker-4: offline burst — Bob ACCEPT'ten ÖNCE Alice devam edebilmeli):
# epoch 0 = PRE-ACCEPT, A→B tek yön, init_root'tan (Bob henüz cevap vermedi):
k_send_A2B_e0 = HKDF("SR-DATA-A2B-E0", init_root, "")
# epoch 1 = POST-ACCEPT, çift yön, session_root'tan (Bob ACCEPT gönderdi):
k_send_A2B_e1 = HKDF("SR-DATA-A2B-E1", session_root, "")
k_send_B2A_e1 = HKDF("SR-DATA-B2A-E1", session_root, "")
# her ASYNC_DATA: per-message key = HKDF(k_send_dir_epoch, seq) → AEAD nonce/key (Section 35.6)
# ASYNC_DATA flags EPOCH + DIRECTION bitleri taşır (Section 35.5).
# Alice, ACCEPT gelmeden epoch-0 chain ile mesaj göndermeye devam eder (amortization korunur);
# ACCEPT gelince epoch-1'e geçer (FS upgrade). Bob epoch-0 mesajları init_root ile açar.
```

> **AEAD tag = İKİ kanıt (NORMATİF):** ASYNC_INIT'te AEAD tag hem (1) integrity hem (2) **sender static possession** kanıtlar: `ss_s = DH(sender_static, recipient_static)` üretmek için sender_static_x25519_sk gerekir (hub'da yok). Bob, k'yi türetip tag'i doğrularsa gönderenin o static key'e sahip olduğunu kanıtlamış olur. Bu Noise "X" pattern'inin one-way halidir. Hub forge edemez (ss_s yok); hub retarget edemez (recipient_dest_hash session_context_v1'te → farklı k → tag fail).

### D2.3 ASYNC_INIT Wire Format

```
ASYNC_INIT {
  ── dış (hub görür) ──
  magic                  : bytes4      # "SAI1"
  version                : u8          # 1
  flags                  : u8          # bit0-1: cert_mode (0=ref, 1=inline-compact, 2=bundle); bit2-7: reserved
  recipient_dest_hash    : bytes32     # tam dest_hash (control-plane; hub routing + Hub B mailbox bind)
  link_id                : bytes16     # init_root'tan türetilmiş (Section 35.2); session route anahtarı
  ── kriptografik gövde ──
  sender_ephemeral_x25519: bytes32     # taze ephemeral (forward secrecy)
  message_counter        : u64_be      # monoton (replay)
  timestamp              : u64_be      # tazelik (replay penceresi)
  ciphertext_len         : u16_be      # ciphertext uzunluğu (≤ MAX_INIT_PAYLOAD)
  ciphertext             : bytes        # AEAD(init_root-derived k, AAD, plaintext); AEAD tag DAHİL
  ── kimlik (reference-first) ──
  sender_cert            : device_cert_hash 32B          (cert_mode=0: sender_cert_ref)
                         | CompactDeviceCertV1 152B      (cert_mode=1: inline_compact_device_cert)
                         | DeviceCertBundle / chain mat. (cert_mode=2: sender_device_cert_bundle, variable)
  # sender_signature YOK (CAD-55; auth AEAD tag'de)
}
```

App-envelope (ciphertext payload hariç, AEAD tag dahil): **ref-mode 152B**, **inline-compact-cert 272B**, **bundle-mode variable** (cache-miss/chain-carrying path). Konuşma başına 1 kez (veya route-miss'te).

> **Cert mode terminoloji (NORMATİF, E-12 düzeltmesi):** `cert_mode=1` (272B) **inline CompactDeviceCertV1**'dir, **full chain DEĞİLDİR**. 272B içindeki `sender_cert` yalnızca CompactDeviceCertV1 (152B) taşır; Bob bunu doğrulamak için sender Identity/User Cert'ine de ihtiyaç duyar. Identity/User Cert cache'te yoksa 152B inline cert tek başına YETMEZ → `cert_mode=2` (DeviceCertBundle / chain material) veya defer (Section 35.11). "full-cert" terimi KULLANILMAZ (yanıltıcı).

### D2.4 ASYNC_ACCEPT Wire Format (Alpha default STRONGER)

```
ASYNC_ACCEPT {
  magic                  : bytes4      # "SAA1"
  version                : u8          # 1
  flags                  : u8          # reserved (Alpha tek profil)
  link_id                : bytes16     # INIT'ten (session route)
  bob_ephemeral_x25519   : bytes32     # ZORUNLU (Alpha default STRONGER; FS upgrade)
  message_counter        : u64_be      # monoton
  ciphertext_len         : u16_be
  ciphertext             : bytes        # AEAD(session_root-derived k, AAD, plaintext); tag DAHİL
}
```

App-envelope: **80B** (4 magic + 1 ver + 1 flags + 16 link_id + 32 bob_eph + 8 counter + 2 len + 16 tag). Konuşma başına 1 kez. 

> **Alpha tek profil — ultra-lowband/aggressive YOKTUR (CAD-59, blocker-9):** Downgrade yüzeyi yaratmamak için Alpha default TEK profil: ASYNC_PROFILE_V2_DEFAULT (STRONGER ACCEPT, Bob ephemeral zorunlu). Ultra-lowband ACCEPT (Bob ephemeral yok), aggressive ASYNC_DATA (link_id 8B / seq u24) Alpha CAD'i DEĞİLDİR — yalnızca Beta+/future signed-policy profile olarak ayrılmıştır (bu belgede normatif Alpha kuralı olarak yer ALMAZ).

### D2.5 ASYNC_DATA Wire Format (established hot-path — ASIL HEDEF)

```
ASYNC_DATA {
  # magic "SAD1" WIRE'DA TAŞINMAZ (CAD-54 blocker-2) — yalnızca AAD domain separator.
  # Tip ayrımı flags.message_type'ta; hub link_id ile demux eder, wire magic'e gerek yok.
  flags                  : u8          # bit0-1: epoch (0=pre-accept,1=post-accept); bit2: direction; bit3-5: message_type; bit6-7: reserved
  link_id                : bytes16     # session route (dest_hash YOK — CAD-56)
  seq                    : u32_be      # per-(link,epoch,dir) monoton (replay + chain key index)
  ciphertext_len         : u16_be
  ciphertext             : bytes        # AEAD(per-message k = HKDF(k_send_dir_epoch, seq), AAD, plaintext); tag DAHİL
}
```

App-envelope (ciphertext payload hariç, AEAD tag dahil): **39B** (1 flags + 16 link_id + 4 seq + 2 len + 16 tag). SAD1 wire'da YOK → 43B değil 39B. **Her mesaj.** İçinde: recipient_dest_hash YOK, sender_cert YOK, sender_ephemeral YOK, timestamp YOK, sender_signature YOK — hepsi session state'te (init_root + session_root).

> **link_id güvenliği (NORMATİF):** Hub link_id'yi cleartext GÖRÜR (routing demux — dest_hash'in yaptığı işi yapar, aynı metadata sınırı). AMA link_id ile mesaj ENJEKTE EDEMEZ: ASYNC_DATA ciphertext = AEAD(k_session-derived), saldırgan k_session'ı bilmez (init_root + ss_be gerektirir) → AEAD üretemez. Çalınan link_id ile sahte paket → Bob AEAD fail → DROP (availability denemesi, içerik enjeksiyonu DEĞİL). link_id `HKDF` ile 128-bit, tahmin edilemez; dest_hash'ten TÜREMEZ (correlation önleme).

### D2.6 ASYNC_DATA Per-Message Key + Replay

```
# Epoch+yön bazlı chain (Section 35.2):
per_message_key(epoch, dir, seq) = HKDF(k_send_dir_epoch, u32_be(seq))
  # epoch ∈ {0 pre-accept, 1 post-accept}; dir ∈ {A2B, B2A} (epoch-0 yalnızca A2B)
  → AEAD key + nonce türetilir; her (epoch,dir,seq) benzersiz key (nonce reuse YOK)

# AEAD AAD (ASYNC_DATA) — magic AAD-only (wire'da değil):
AAD = "SAD1" || flags || link_id || seq    # flags epoch+direction içerir → epoch/dir/link/seq binding
# epoch+direction AAD'de → epoch-0/epoch-1 ve A2B/B2A mesajları birbirine karıştırılamaz (cross-epoch replay korur)

# Replay (alıcı session state):
  highest_seq(link_id, dir) + sliding window bitmap (genişlik 512-1024)
  seq <= highest - window  → DROP (çok eski)
  seq zaten görüldü (bitmap) → DROP (replay)
  yeni seq → kabul, bitmap güncelle
```

### D2.7 Downgrade Protection (NORMATİF, CAD-55/56)

```
Protocol profile selection DETERMİNİSTİK: signed Kama policy + signed recipient
capabilities + local implementation rules'tan. KULLANICI/HUB/PEER kararı DEĞİL,
müzakere EDİLEMEZ.

HKDF session_context_v1 (Section 35.2) şunları bağlar:
  protocol_version, profile_id, cipher_suite_id,
  sender_cert_ref, recipient_dest_hash, recipient_static_x25519_hash.
AEAD AAD ayrıca message_type (magic), link_id, message_counter/seq, ephemeral'i bağlar.
(message_type session_context'te DEĞİL — message-specific AAD'de; policy_sequence ve
 recipient_discovery_record_hash key schedule'dan ÇIKARILDI — Section 35.2 blocker-3 düzeltmesi.)

Profile mismatch → FAIL CLOSED.
ASYNC_ACCEPT, ASYNC_INIT profilini DÜŞÜREMEZ (aynı profile_id/cipher_suite_id session_context'te).
Bilinmeyen profile_id/cipher_suite_id → reject.
Alpha: profile_id + cipher_suite_id SABİT → negotiation yok → downgrade yüzeyi yok.
(Beta policy binding gerekirse: policy_hash:bytes32 session_context'e eklenir ANCAK iki tarafın
 aynı policy_hash'i kesin bildiği — wire'da taşınan veya local deterministic — garanti edilmeli.)
```

> **cert ref vs inline-compact downgrade DEĞİLDİR (NORMATİF):** `cert_mode` (32B ref / 152B inline-compact / bundle) bir güvenlik profili değil, **cert availability** farkıdır. Bob sender cert.i cache.te biliyorsa 32B ref; bilmiyorsa 152B inline CompactDeviceCert (veya chain eksikse bundle). Hepsi aynı güvenlik; downgrade yüzeyi değil.

### D2.8 Implicit Magic / Parser Notu

```
ASYNC_DATA hot-path olduğu için magic "SAD1" WIRE'DA TAŞINMAZ (39B hedefi; blocker-2) —
yalnızca AAD domain separator olarak kullanılır. Tip ayrımı flags.message_type'ta; hub
link_id ile demux eder. cipher_suite_id de ASYNC_DATA wire'da TAŞINMAZ — session_root'ta
sabitlenmiştir (INIT/ACCEPT'te belirlendi, AAD'de bağlı). Bu, Alpha tek-suite kararıyla
tutarlı (Section 35.7); suite session ömrü boyunca değişmez.
```

### D2.9 SIGNED_CONTROL_MESSAGE (non-repudiation, CAD-55)

```
Normal chat/file = ASYNC_DATA (imzasız, deniable). Non-repudiation GEREKEN durumlar
→ ayrı tip (her pakette DEĞİL):

SIGNED_CONTROL_MESSAGE {
  magic                  : bytes4      # "SAC1"
  version                : u8
  flags                  : u8
  link_id                : bytes16
  seq                    : u32_be
  ciphertext_len         : u16_be
  ciphertext             : bytes        # AEAD; tag DAHİL
  sender_signature       : bytes64      # Ed25519_Sign(sk_device_ed25519, signed_control_tbs)
}
signed_control_tbs = magic || version || flags || link_id || seq || ciphertext_len || ciphertext

App-envelope: 107B (4 magic SAC1 wire + 1 ver + 1 flags + 16 link_id + 4 seq + 2 len + 16 tag + 64 sig). Kullanım: explicit signed command (kritik komut), delivery receipt. PAHALI (+64B) ama NADİR.
Audit ihtiyacı: bu + RA/Admin enrollment audit + delivery receipt audit (her chat değil).
```

### D2.10 Güvenlik Özellikleri (CIA + ekstra)

```
AUTHENTICITY:    AEAD tag = ss_s possession (Section 35.2). Hub Bob'un public cert'ini
                 hash'leyip sahte mesaj koyamaz → sender_static_sk YOK → ss_s türetilemez → tag fail.
INTEGRITY:       AEAD + transcript binding → hub değiştiremez/retarget edemez.
CONFIDENTIALITY: ciphertext = AEAD(DH-chain k). Sadece Bob (static private) + Alice açar.
                 FS: ss_e (INIT) + ss_be (ACCEPT) ephemeral'lerden. SINIRI: static x25519
                 sızarsa bekleyen INIT açılabilir (V1 ile aynı, cihaz ele geçirme kapsam dışı).
AVAILABILITY:    hub SİLEBİLİR/geciktirebilir (suppression residual; Tor bad-guard sınıfı).
                 N-mesaj-ACK penceresi (identity v1.5) + çok-hub failover telafi eder.
NON-REPUDIATION: BİLİNÇLİ FEDA (deniability tercih). Bob göndereni BİLİR ama üçüncü tarafa
                 kanıtlayamaz. Gerekirse SIGNED_CONTROL_MESSAGE (Section 35.9).
DENIABILITY:     KAZANILDI — k AEAD'yi Bob da üretebildiği için mesajı "forge" edebilirdi →
                 üçüncü tarafa Alice'i bağlamaz (Signal/OTR modeli).
```

> **Ek HMAC kullanılmaz.** Bütünlük+orijinallik: (1) AEAD tag (ciphertext + AAD), (2) ss_s possession auth (key schedule), (3) domain separation (magic + session_context_v1), (4) recipient/link binding (AAD). Üç+ katman → ek simetrik HMAC gereksiz.

### D2.11 Reference-First Cert + Hostile Cache-Miss (ASYNC_INIT, NORMATİF)

```
ASYNC_INIT sender cert policy (yalnızca INIT — DATA cert taşımaz):

İlk temas / bilinmeyen sender (cert_mode=1: inline-compact):
  ASYNC_INIT inline CompactDeviceCertV1 (152B) içermeli (Bob sender cert'i cache'te yoksa).
  NOT: inline CompactDeviceCert TEK BAŞINA Bob'un sender Identity/User Cert'ini doğrulamasına
  yetmeyebilir — Identity/User Cert cache'te yoksa cert_mode=2 (bundle) gerekir.

Bob cache'i sıcaksa (cert_mode=0: ref):
  ASYNC_INIT yalnızca sender_cert_ref (32B device_cert_hash) kullanır.

Chain eksik / ilk-ilk temas (cert_mode=2: bundle):
  ASYNC_INIT DeviceCertBundle / chain-carrying material taşır (CompactDeviceCert + Identity/User CCR
  + opsiyonel Intermediate CCR cache). Bob hem device cert'i hem chain'i tek mesajdan doğrular.

Hostile cache-miss (cert_mode=0 ama Bob'da yok):
  Bob FAIL-CLOSED veya DeviceCertBundle (cert_mode=2) gelene dek erteler. Çıplak device_id'ye GÜVENMEZ.

ZORUNLU canlı fetch YOK: INIT işlemek RA/hub/prekey-store iletişimi GEREKTİRMEZ
  (cache-miss'te hub'dan cert opsiyonel kolaylık; zorunlu değil; hub'a bağımlı DEĞİL).

CERT ROTATION (90 gün): yeni cert → yeni device_cert_hash → Bob cache-miss → INIT cert_mode=1 veya 2.
```

### D2.12 ASYNC_INIT Sender Validation

```
sender cert: inline CompactDeviceCertV1 (cert_mode=1) | device_cert_hash (cert_mode=0) | DeviceCertBundle (cert_mode=2)

Validation:
  1. validated CompactDeviceCertV1'i çöz (inline/bundle veya cache; cache-miss → fail-closed/defer)
  2. issuer Identity/User Cert chain (Identity/User → Intermediate → Root, recursive); CRLite fresh/fail-closed
  3. sender identity issuer Identity/User Cert key'inden TÜRE:
       sender_identity_hash = SHA-256(identity_cert.user_identity_ed25519_pub)
       sender_dest_hash     = SHA-256(sender_identity_hash)   (device key'den DEĞİL)
  4. sender_static_x25519    = CompactDeviceCert.device_x25519_pub
     recipient_static_x25519 = local device_x25519_pub
     recipient_static_x25519_hash = SHA-256(recipient_static_x25519)   (session_context_v1 alanı)
  5. session_context_v1 reconstruct et (Section 35.2: ver||profile||suite||cert_ref||dest_hash||x25519_hash)
  6. ss_e = DH(sender_ephemeral, recipient_static);  ss_s = DH(sender_static, recipient_static)
  7. init_root türet; ciphertext AEAD tag doğru → ss_s possession KANIT (sender auth)
  8. recipient_dest_hash == local dest_hash (recipient confusion önleme)
  9. link_id expected kontrol:
       link_id == Trunc128(HKDF("SR-LINK-ID-V1", init_root, session_context_v1))   (mismatch → reject)
 10. session_context_v1 tutarlı (profile_id/cipher_suite_id; Alpha sabit, mismatch → reject)
 11. replay key = (sender_identity_hash, link_id, message_counter)
 12. pending/inbound mailbox AsyncSessionState oluştur (epoch=PRE_ACCEPT)
```

### D2.13 Parser Reject Kuralları (Async Session V2)

```
ASYNC-REJECT-1:  outer frame_type ∉ {INIT, ACCEPT, DATA, SIGNED_CONTROL} → reject
  # ASYNC_INIT/ACCEPT/SIGNED_CONTROL: magic (SAI1/SAA1/SAC1) WIRE'DA taşınır → magic eşleşmezse reject.
  # ASYNC_DATA: SAD1 WIRE'DA TAŞINMAZ (AAD-only). Tip outer frame_type (transport packet_class/flags)
  #   ile seçilir; SAD1 yalnızca AAD'de domain separator. ASYNC_DATA parse'ında wire magic ARANMAZ.
ASYNC-REJECT-2:  version != 1 → reject
ASYNC-REJECT-3:  ASYNC_INIT length != beklenen (cert_mode'a göre) → reject
ASYNC-REJECT-4:  ASYNC_ACCEPT bob_ephemeral all-zero → reject (Alpha STRONGER zorunlu)
ASYNC-REJECT-5:  ASYNC_DATA seq <= highest-window veya replay (bitmap, per epoch+dir) → reject
ASYNC-REJECT-5b: ASYNC_DATA epoch-1 mesajı ama session henüz ACCEPT almadı (epoch-1 chain yok) → reject/defer
ASYNC-REJECT-5c: ASYNC_DATA epoch-0 mesajı ACCEPT SONRASI (epoch kapandı, replay) → reject
ASYNC-REJECT-6:  AEAD tag doğrulanamadı (k türetilemedi / tampered) → reject
ASYNC-REJECT-7:  recipient_dest_hash != local dest_hash (INIT) → reject
ASYNC-REJECT-8:  session_context_v1 profile/suite mismatch → reject (downgrade)
ASYNC-REJECT-9:  link_id bilinmeyen (DATA, session state yok) → LINK_ROUTE_MISS sinyali (reject değil; routing belgesi)
ASYNC-REJECT-10: sender_ephemeral / X25519 DH all-zero (low-order) → reject
ASYNC-REJECT-11: SIGNED_CONTROL_MESSAGE sender_signature Ed25519 fail → reject
ASYNC-REJECT-12: trailing byte → reject
```

### D2.14 Byte Tablosu (app-layer envelope; transport HARİÇ)

```
┌──────────────────────────────┬──────────┬──────────────────────┐
│ Tip                           │ App-env  │ Sıklık               │
├───────────────────────────────┼──────────┼──────────────────────┤
│ ASYNC_INIT ref-mode           │  152B    │ konuşma başına 1 kez  │
│ ASYNC_INIT inline-compact-cert│  272B    │ ilk-ilk temas, 1 kez  │
│ ASYNC_INIT bundle-mode        │ variable │ cache-miss/chain path │
│ ASYNC_ACCEPT stronger         │   80B    │ konuşma başına 1 kez  │
│ ASYNC_DATA (SAD1 wire'da YOK) │   39B    │ HER MESAJ (hot-path)  │
│ SIGNED_CONTROL_MESSAGE        │  107B    │ NADİR (kritik komut)  │
└───────────────────────────────┴──────────┴──────────────────────┘

NOT (NORMATİF): Bunlar APPLICATION-LAYER envelope'dır (AEAD tag dahil, ciphertext payload
hariç). Transport framing (session_id 8B), KCP/PDP header, UDP/IP, retransmission, FEC,
link-layer HARİÇTİR (transport-layer-tdd.md). Full wire = app-envelope + transport overhead.
ASYNC_DATA 39B full wire DEĞİLDİR.
```

### D2.15 RNS Kıyası (katman-eşleşmeli, NORMATİF dürüstlük)

```
RNS established link GERÇEK overhead: ~19B header + HMAC-SHA256 32B = ~51B (ct hariç).
  ("19-35B" rakamı çıplak routing header; HMAC/encryption/framing dahil DEĞİL.)
RNS link setup: ~297B (3 paket, 1 kez — public-key maliyeti bir kez ödenir).

Kama ASYNC_DATA app-envelope (≈39B, AEAD tag dahil) RNS established encrypted packet
overhead ile AYNI SINIFTA, potansiyel olarak DAHA KÜÇÜK olabilir. ANCAK kesin wire kıyası
İKİ TARAFTA da transport/PDP/KCP/UDP/FEC/link-layer maliyetlerini dahil etmeyi GEREKTİRİR.
"Kama RNS'i kesin geçti" hükmü verilemez — katman-eşleşmeli ölçüm şart.

Kama'nın RNS'e göre mimari avantajı: always-on hub'lar route/mailbox state tutar →
client keep-alive ödemez (RNS'te link state uçlarda, keepalive şart). Bu byte değil,
state-yönetimi avantajıdır.
```

> Tam asenkron oturum lifecycle, hub session route cache, LINK_ROUTE_MISS, ASYNC_RESUME → `routing-rendezvous-architecture.md`. N-mesaj-ACK penceresi, ratchet senkronu → `identity-and-trust-model.md` v1.5.

### D2.16 Hub Session State DoS Koruması (NORMATİF, CAD-57 blocker-7)

Hub, ASYNC_INIT'in **içeriğini doğrulayamaz** (E2E şifreli; ss_s possession'ı yalnızca Bob doğrular). Bu nedenle hub, INIT'ten gelen session state'i sınırsız tutamaz — aksi halde saldırgan sahte INIT'lerle hub belleğini doldurur (state-exhaustion DoS). Hub şu sınırları uygular:

```
İKİ AYRI STATE — karıştırılmamalı (blocker-7; offline async ile çakışmamalı):

(1) MAILBOX BINDING (Bob'un hub'ı, Hub B) — uzun ömürlü, INIT ulaşır ulaşmaz:
  - inbound_mailbox[link_id] = Bob mailbox/dest_hash. TTL = conversation TTL (7 gün).
  - Bob OFFLINE olsa bile bu binding KALIR → Alice epoch-0 DATA burst'ü Bob'un mailbox'ına
    birikir (amortization KORUNUR; Bob offline iken Alice INIT tekrarlamak ZORUNDA değil).
  - Bu binding "pending" DEĞİL — INIT geçer geçmez kurulur, ciphertext mailbox'a yazılır.
  - State exhaustion koruması: per-recipient mailbox kotası (Bob başına aktif link cap) +
    mailbox boyut limiti (Bob çekmezse en eski mesaj TTL ile düşer) — pending TTL DEĞİL.

(2) OUTBOUND ROUTE CACHE (Alice'in hub'ı, Hub A) — kısa ömürlü, doğrulanmamış:
  - outbound_session_route[link_id] = Hub_B (routing hint). TTL kısa (örn. 60 sn ila dakikalar).
  - Bu cache kaybolursa Alice LINK_ROUTE_MISS alır → ASYNC_RESUME (route'u yeniden çözer, Section 35.17).
  - Kısa TTL burada SORUN DEĞİL: route hint kaybı ucuz ASYNC_RESUME ile telafi; Bob'un mailbox'ı (1) etkilenmez.
  - State exhaustion koruması: per-client route cache kotası + LRU eviction.

VERIFIED transition (Hub A): Bob ASYNC_ACCEPT döndürünce route cache "verified" işaretlenir,
  TTL uzar (conversation TTL). Ama epoch-0 burst için VERIFIED BEKLENMEZ — outbound route hint
  (kısa TTL) + Bob mailbox binding (uzun TTL) epoch-0'ı zaten taşır.

Kritik ayrım: Bob'un OFFLINE olması epoch-0 amortization'ı BOZMAZ — çünkü mailbox binding (Hub B,
  uzun TTL) INIT'te kurulur ve Bob offline iken DATA'ları biriktirir. Kısa TTL yalnızca Alice'in
  hub'ındaki route HINT'inde (kaybı ucuz resume); mailbox'ta DEĞİL.
```

### D2.17 LINK_ROUTE_MISS + ASYNC_RESUME (NORMATİF, CAD-57 blocker-8)

```
Hub session route state kaybolursa (TTL expire / hub restart / eviction):
  Hub → client: LINK_ROUTE_MISS { link_id }
  → AUTHENTICATED transport içinde gelmeli (Section transport; client↔hub Noise session).
    Aksi halde saldırgan sahte LINK_ROUTE_MISS ile client'ı pahalı INIT'e zorlar (DoS).

Client fallback SIRASI (rate-limited):
  1. ÖNCE ASYNC_RESUME { link_id, route_resume_token } dene (UCUZ — hub-issued opaque token).
     # ÖNEMLİ (blocker-8): Hub E2E session secret'ı BİLMEZ → resume "session secret proof"
     # ile doğrulanAMAZ. Bunun yerine İKİ seçenekten biri:
     #   (a) HUB-ISSUED OPAQUE ROUTE TOKEN: Hub A, VERIFIED route kurulduğunda client'a opak
     #       bir route_resume_token verir (hub'ın kendi MAC'i ile imzalı, içeriği client'a opak).
     #       Resume'da client bu token'ı geri sunar → Hub A kendi MAC'ini doğrular → route'u
     #       cache'ten/DHT'den geri kurar. Token E2E secret İÇERMEZ; yalnızca routing material.
     #   (b) DHT RE-LOOKUP: token yoksa/expired → hub, recipient_dest_hash'i (INIT'ten biliyorsa)
     #       veya client'ın yeniden sağladığı routing material ile FIND_VALUE(routing_key) → Hub_B.
     # İki yol da E2E secret GEREKTİRMEZ — yalnızca hub-tarafı routing.
  2. ASYNC_RESUME başarısız (token expired + DHT lookup miss / Bob tarafı da state kaybetti)
     → yeni ASYNC_INIT (PAHALI). E2E session secret'ı yalnızca client'lar bilir; INIT yeni init_root kurar.
  3. Client, pahalı INIT fallback'ini RATE-LIMIT eder (örn. dakikada N) — sahte route-miss
     seli pahalı INIT amplification'ı tetikleyemez.

Düşük bantta kabul: pahalı INIT yalnızca çift-taraflı state kaybında (nadir); çoğu miss
  hub-issued route token veya DHT re-lookup ile ucuz çözülür. Amortization korunur.
```

> ASYNC_RESUME / LINK_ROUTE_MISS wire formatı ve hub route cache mekaniği `routing-rendezvous-architecture.md`'de (bu bölüm yalnızca app-layer güvenlik gereksinimini normatif kılar).
