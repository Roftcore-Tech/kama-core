# SafeRoom — Discovery Architecture (Contact Discovery, VOPRF, Discovery Server)

> **Statü:** Implementation-ready (Alpha). Bu belge `identity-and-trust-model.md` v1.5 (semantik otorite) ve `ccr-format-spec.md` (byte-level projeksiyon) ile %100 tutarlıdır. DiscoveryRecord byte layout'u ccr-spec §21'de canonical'dır; bu belge VOPRF mekaniğini, Discovery Server'ı, bootstrap'ı, publish/fetch akışlarını ve cache modelini tanımlar.
> **Bağımlılık:** identity-and-trust-model.md (CAD-16, CAD-53), ccr-format-spec.md (§21 DiscoveryRecord, §13 CompactDeviceCert, §21.5 HubListRecord, §35 Session Model V3), routing-rendezvous-architecture.md (RoutingRecord, hub-mesh DHT).

---

## 0. Magic Registry / Cipher Suite

Discovery yeni bir **canonical imzalı TBS** eklemez — `DiscoveryRecord` (magic `"SRDR"`) zaten ccr-spec §21'de tanımlıdır. Bu belgedeki VOPRF/PUBLISH/LOOKUP mesajları **transport-level RPC payload'larıdır** (kendi başlarına imzalı canonical nesne değildir; içlerindeki DiscoveryRecord kendi `user_signature`'ını taşır). Domain separation iki **string sabiti** ile sağlanır:

```
VOPRF private_input prefix   : "SafeRoom-Discovery-V1"          (label, blind input domain separation)
discovery_lookup_key prefix  : "SafeRoom-Discovery-Lookup-V1"   (label, lookup-key türetimi)
```

**Cipher suite (Alpha — KESİN):**

```
VOPRF suite        : ristretto255-SHA512  (RFC 9497)
VOPRF mode         : VOPRF (0x01) — Verifiable; Discovery Server doğru key kullandığını proof ile kanıtlar
suite_id (dahili)  : 0x0001
Group              : ristretto255 (prime-order; Curve25519 ailesi — Ed25519/X25519 ile tutarlı)
Hash               : SHA-512
```

> **NORMATİF (suite tutarlılığı):** SafeRoom'un tüm kripto hattı Curve25519 ailesindedir (Ed25519 imza, X25519 DH). VOPRF de aynı aileden `ristretto255` seçilir (prime-order grup → cofactor sorunu yok; libsodium/dalek desteği mevcut). **P-256 (NIST/FIPS/CNSA) Alpha'da implement EDİLMEZ** — gelecekteki "FIPS compatibility profile" olarak Section 12'de rezerve edilir. İkinci kripto ailesi sokmak Alpha'da gereksiz validasyon/serialization/test yükü demektir.

---

## 1. Scope ve Non-Goals

### 1.1 Bu belge neyi çözer

Discovery, **"bu telefon numarası kime ait, kimliği (dest_hash) ne, statik X25519'u ne"** sorusunu cevaplar (contact discovery). Çıktı: Alice'in rehberindeki SafeRoom kullanıcılarının `DiscoveryRecord`'ları (dest_hash + cert + statik X25519), hostile ağa girmeden önce toplu çekilip cache'lenir.

### 1.2 Bu belge neyi ÇÖZMEZ (Non-Goals)

- **"Bu dest_hash şu an hangi hub'ta" → routing-rendezvous-architecture.md (DHT).** Discovery KONUM vermez (CAD-53). Discovery kimlik, DHT konum.
- **Mesaj/çağrı taşıma → routing-rendezvous (hub-to-hub RPC).** Discovery sadece kimi bulacağını söyler.
- **Senkron rendezvous (sesli/görüntülü çağrı kurulumu) → routing-rendezvous §8** (Alpha'da VAR; relay-only signaling). Discovery'nin işi değil.
- **Cross-organization discovery → Beta** (Alpha'da operatör-izole; Section 12).
- **Cert zinciri/PKI semantiği → identity v1.5.** Discovery cert'i taşır ama trust kararını identity modeli verir.

### 1.3 Discovery vs Routing (KRİTİK AYRIM — CAD-53)

| Soru | Cevaplayan | Mekanizma | Okuyucu |
|------|-----------|-----------|---------|
| "Bu telefon kimin? dest_hash'i ne? static X25519'u ne?" | **Discovery Server** (kurum) | VOPRF + DiscoveryRecord | Client |
| "Bu dest_hash şu an hangi hub'ta?" | **DHT** (hub mesh) | FIND_VALUE → RoutingRecord | Hub (client GÖRMEZ) |
| "Mesajı/çağrıyı o hub'a nasıl taşırım?" | **Hub-to-hub** | RPC (async taşıma / sync signaling relay) | Hub |

> **NORMATİF:** DiscoveryRecord **statik kimlik** taşır (dest_hash + cert + static X25519); **hub konum bilgisi (home_hub_id) TAŞIMAZ** (CAD-53). Alice mesaj atarken yalnızca `dest_hash`'i **kendi** primary hub'ına verir; "hangi hub" sorusunu hub-mesh DHT RoutingRecord ile çözer. Böylece client peer routing topolojisini asla öğrenmez (social-graph/konum sızıntısı engeli).

---

## 2. Mimari Konum — Discovery Server Kurumda

```
   ┌──── RoftCore-controlled ────┐        ┌──────────── KURUM (customer/org infra) ────────────────┐
   │                             │        │                                                        │
   │   ┌──────────┐              │ tenant-scoped, audited, mTLS-protected push channel             │
   │   │    RA    │──────────────────────────────────┐                                             │
   │   │ (kayıt + │              │        │           ▼                                             │
   │   │ lisans + │              │        │  ┌──────────────────┐                                   │
   │   │ allowlist│              │        │  │ Discovery Server │   ┌────────┐   blind   ┌────────┐│
   │   │ + device │              │        │  │  - org OPRF key  │◀──│  Hub   │◀───────────│ Client ││
   │   │ approval)│              │        │  │    (HSM-backed)  │──▶│ (proxy)│───────────▶│ (Alice)││
   │   └──────────┘              │        │  │  - lookup_key DB │   └────────┘ blinded eval└────────┘│
   │   (Root CA, Intermediate    │        │  │  - rate-limit    │   (OPRF key GÖRMEZ, telefon GÖRMEZ)│
   │    CA, RA = RoftCore)        │        │  └──────────────────┘                                   │
   └─────────────────────────────┘        │  (Admin Server + Hub'lar + Discovery Server = kurum)    │
                                          └────────────────────────────────────────────────────────┘
   Client → Hub → Discovery Server : blind VOPRF + lookup (proxy; hub içeriği açamaz)
   RA → Discovery Server          : kayıt anında telefon→record push (tenant-scoped, audited, mTLS)
```

> **NORMATİF (deployment boundary — KESİN):** **RA RoftCore-controlled'dır** (Root CA + Intermediate CA + RA hepsi RoftCore tarafında). **Discovery Server customer/org infrastructure'dadır** (Admin Server + Hub'lar + Discovery Server kurum tarafında). RA → Discovery Server akışı **tenant-scoped, audited, mTLS-protected push channel**'dır (basit "kurum içi mTLS" değil — RA kurumda değildir; push channel RoftCore-RA'dan kurumun Discovery Server'ına tenant sınırında geçer). **Discovery Server RA değildir, sertifika basmaz, identity issuer gibi davranmaz; RA Discovery Server değildir.**

**Üç ayrı entity (separation of concerns — Abkarada ilkesi):**

- **RA** (RoftCore-controlled): yalnızca kayıt + lisans + allowlist + device approval gate. Telefon↔identity statik eşlemeyi kayıt anında bilir (#24). Root CA + Intermediate CA + RA hepsi RoftCore tarafındadır. Discovery Server'a record push eder (**tenant-scoped, audited, mTLS-protected push channel** — RA kurumda değil, push tenant sınırını geçer).
- **Discovery Server** (customer/org infrastructure): VOPRF eval (org-scoped OPRF key, HSM-backed) + `lookup_key → DiscoveryRecord` indeks + rate-limit. **Ayrı server, RA değildir** (sertifika basmaz, identity issuer değildir; RA'ya yüklenmez → blast-radius daralır; RA compromise org OPRF key'ini vermez).
- **Hub** (untrusted, public-facing): yalnızca **proxy** (Client ↔ Discovery Server taşıma) + DHT routing. **OPRF key tutmaz, telefon görmez** (blinding), DiscoveryRecord üretmez.

> **NORMATİF (VOPRF neden hub'da DEĞİL — #25 paradoksu):** Telefon numarası düşük-entropi (~10^8–10^9). Hub güvenilmezdir; hub'a OPRF key verilirse hub offline brute-force ile tüm rehberi kırar. Hub'a key dağıtmak paradoks: aynı key → tek kötü hub sızdırır tüm uzay; farklı key → Alice farklı hub'a sorunca farklı `lookup_key` → tutarsız. Ayrıca dağıtık discovery availability'yi N hub'ın iyi çalışmasına bağlar. **Tek güvenilir nokta (kurum Discovery Server, rate-limit, audit) N güvenilmez noktadan güvenlidir.** VOPRF kurum Discovery Server'da çalışır; hub yalnızca taşır.

> **NORMATİF (VOPRF neden RoftCore'da DEĞİL):** Kurumun telefon uzayı kayıtta zaten RoftCore RA'ya çıkar, AMA discovery'de kurum sınırını aşmaz. Discovery Server kurumun kendi altyapısındadır, kurumun kendi OPRF key'ini (HSM-backed) kullanır. Compromise olsa yalnızca **o kurumun phone↔identity eşlemesi** (RA/Admin zaten tutar — kayıtlı kullanıcı uzayı) risk altındadır; diğer kurumlar etkilenmez (kurum-izole OPRF key). NOT: Bu, kayıtlı kullanıcı listesini kapsar — Alicein rehber sorgu seti ve eşleşen contact-graphı ek metadatadır (D-2/D-5, §5.6); geçmiş lookup_key logları varsa k compromise sonrası telefonlara map edilebilir.

---

## 3. Bootstrap (İlk Onboarding — Tavuk-Yumurta Çözümü)

Yeni indirilen client'ta hub listesi yoktur → hub proxy yapamaz (tavuk-yumurta). Çözüm: app bundle'da yalnızca **iki sabit** gömülüdür.

```
APP BUNDLE (sabit, herkes için aynı):
  ROOT_CA_SPKI_HASH            : bytes32   # RoftCore Root CA SPKI pin (ASLA ağdan gelmez)
  ROFTCORE_ONBOARDING_ENDPOINT : well-known URL/anycast (sabit; QR/davet DEĞİL)
```

> **NORMATİF (bootstrap seçimi):** Well-known endpoint seçildi (QR/davet REDDEDİLDİ) çünkü consumer-facing açılımı (WhatsApp/Signal gibi viral yayılma) engellemez. QR/davet modeli yayılmayı kısıtlar.

**Onboarding akışı (Bob, iyi internet):**

```
1. Bob → RA (ROFTCORE_ONBOARDING_ENDPOINT, DOĞRUDAN — hub listesi henüz yok):
     org_id iddiası (kurum kodu, QR DEĞİL) + SMS doğrulama + cihaz attestation
2. RA: allowlist + lisans gate (#24 Kapı-1 seat limit, Kapı-2 admin approval).
     allowlist'te yoksa REDDET (org_id iddiası bypass DEĞİL).
3. RA → Intermediate CSR → Bob'a İLK BUNDLE döner:
     - User Identity Cert + CompactDeviceCert (chain)
     - HubListRecord (Intermediate-imzalı hub IP SAN listesi, ccr §21.5)
     - DiscoveryEndpointRecord (endpoint + pk_oprf; Intermediate-imzalı, ccr §21.7; Section 6.1)
4. Bob artık hub'ları bilir → HubListRecord'dan hub seç.
5. Bundan SONRA tüm Discovery/RA erişimi HUB PROXY ile (Client→Hub→Discovery/RA;
     doğrudan değil, gerçek adres gizli — RA-proxy mantığı #24 ile aynı).
```

> **NORMATİF (well-known endpoint dayanıklılığı):** Well-known endpoint yalnızca **YENİ KAYIT** için kritiktir (saldırı/sansür hedefi). Kayıtlı client'lar (HubListRecord elinde) bu endpoint'e DÖNMEZ — hub proxy ile çalışır. Endpoint down/blocked olsa mevcut kullanıcıları etkilemez, yalnızca yeni onboarding gecikir (kabul; onboarding zaten iyi-internet anı). Resilience (anycast/multi-IP) Beta+ operasyonel not.

---

## 4. VOPRF Protokolü (RFC 9497, ristretto255-SHA512, VOPRF mode)

### 4.1 Amaç

Basit `SHA-256(phone)` rainbow table'a açıktır (~10^9 numara, SHA hızlı). VOPRF, Discovery Server'ın gizli OPRF key'i (`k`, client'ın asla görmediği) ile `tag = PRF(k, phone)` üretir → **org OPRF key `k`'ya sahip olmayan saldırgan için** offline brute-force engellenir (DB/lookup_key sızıntısı tek başına telefona geri döndürülemez); online sorgu rate-limit'e tabidir. (k/HSM compromise olursa o org'un uzayı enumerate edilebilir — D-5.) **Verifiable (VOPRF) mode:** Discovery Server doğru `k`'yı kullandığını proof ile kanıtlar (client `pk_oprf` ile doğrular).

### 4.2 Domain-Separated Input

```
private_input = "SafeRoom-Discovery-V1" || org_id(16) || normalized_phone || identity_namespace(16)

  normalized_phone  : E.164 ASCII (ör. "+905321234567"), leading/trailing whitespace YOK
  org_id            : kurum namespace (cross-operator izolasyon)
  identity_namespace: org'un identity namespace kökü (Intermediate nameConstraints ile bağlı; 16B)
```

> `org_id` ve `identity_namespace` input'a dahildir → aynı telefon farklı kurumda farklı VOPRF output üretir (cross-operator izolasyon kriptografik). Alpha'da operatörler arası eşleşme zaten yok (Section 12).

### 4.3 VOPRF Akışı (RFC 9497 §3.3, VOPRF mode)

```
HAZIRLIK (Discovery Server, kurum):
  k          : OPRF secret key (HSM-backed, kurum-izole)            # ristretto255 scalar
  pk_oprf    : k · G  (VOPRF public key; DiscoveryEndpointRecord'da yayımlanır, ccr §21.7 / Section 6.1)

CLIENT (Alice) — BLIND:
  for each phone_i:
    r_i        = random scalar (nonzero)
    H_i        = HashToGroup(private_input_i)                       # RFC 9497 hash-to-ristretto255
    blinded_i  = r_i · H_i                                          # 32B (compressed ristretto255 point)

DISCOVERY SERVER — EVALUATE:
  evaluated_i  = k · blinded_i                                      # 32B
  proof        = GenerateProof(k, pk_oprf, blinded[], evaluated[])  # DLEQ proof (2 scalars = 64B, batched)

CLIENT — UNBLIND + FINALIZE + VERIFY:
  VerifyProof(pk_oprf, blinded[], evaluated[], proof)               # doğru key kontrolü → fail ise ABORT
  N_i          = (1/r_i) · evaluated_i = k · H_i                    # unblind
  voprf_output_i = Finalize(private_input_i, N_i)                   # SHA-512(...) → 64B (RFC 9497 Finalize)
  discovery_lookup_key_i = SHA-256("SafeRoom-Discovery-Lookup-V1" || org_id || voprf_output_i)   # 32B (Model B)
```

> **NORMATİF (Model B — dest_hash ÜRETİLMEZ):** VOPRF çıktısı `discovery_lookup_key`'tir (indeks anahtarı), **`dest_hash` DEĞİL**. Canonical `dest_hash = SHA-256(SHA-256(user_identity_ed25519_pub))` (CAD-16) kimlik anahtarından türetilir ve DiscoveryRecord içinde gelir. VOPRF yalnızca "telefon → hangi record'a bakayım" eşlemesi yapar; kimlik anahtarına dokunmaz. Böylece identity-model'in dest_hash tanımı bozulmaz.

### 4.4 Byte Boyutları

| Eleman | Boyut | Not |
|--------|-------|-----|
| `blinded_i` / `evaluated_i` | 32B | compressed ristretto255 point |
| VOPRF `proof` (batched) | 64B | DLEQ (2 scalar); tüm batch için TEK proof |
| `voprf_output_i` | 64B | RFC 9497 Finalize (SHA-512) |
| `discovery_lookup_key_i` | 32B | SHA-256 (Model B türetimi) |

---

## 5. Discovery Server

### 5.1 Sorumluluklar

```
- VOPRF eval (org OPRF key k, HSM-backed) + batched DLEQ proof üret.
- DiscoveryRecord store: lookup_key → DiscoveryRecord (+ tam CompactDeviceCert, Section 7.3).
- PUBLISH doğrula (user_signature, ra_attestation) + indeksle (Section 6).
- LOOKUP_BATCH cevapla (eşleşen record'lar + cert'ler).
- Rate-limit / enumeration koruması (Section 5.2).
- OPRF key rotation (Section 12, Beta operasyonel).
```

### 5.2 Rate-Limit / Enumeration Koruması (ZORUNLU)

> **NORMATİF:** VOPRF offline brute-force'u engeller (key client'ta yok), AMA **kayıtlı içeriden saldırgan online enumeration deneyebilir** (rehbere sürekli numara ekleyip sorgulayarak). Bu yüzden Discovery Server **per-user/per-org quota + anomali tespiti uygular** (Signal modeli). Quota aşımı → throttle/deny + audit log. Rate-limit olmadan VOPRF tek başına enumeration'ı durdurmaz.

```
- Per-user batch quota (ör. N sorgu/gün), per-org global quota.
- Sabit batch boyutu (rehber boyutu gizlenir): her LOOKUP_BATCH her zaman B (ör. 256) lookup_key taşır,
  eksikler random padding ile doldurulur → pattern analizi yapılamaz.
- Anomali tespiti: ani yüksek-hacim sorgu → flag + admin alert.
```

### 5.3 OPRF Key — HSM-Backed, Org-İzole

```
- k her kurumda BAĞIMSIZ (org-izole). Global key KULLANILMAZ.
- Compromise → yalnızca o kurumun (zaten bildiği) telefon uzayı; diğer kurumlar etkilenmez.
- k HSM'de; Discovery Server process'i imzalama/eval için HSM'e çağrı yapar, k'yı RAM'e açmaz.
- pk_oprf = k·G; DiscoveryEndpointRecord'da yayımlanır (Intermediate-imzalı, ccr §21.7 / Section 6.1) →
  client VOPRF proof'unu doğrular (verifiable mode).
```

---

### 5.4 Client ↔ Discovery Server Transport Security over Hub Proxy (NORMATİF)

Hub güvenilmez bir proxy'dir. Bu yüzden TÜM Client↔Discovery Server RPC payload'ları client ile Discovery Server arasında bir **E2E channel** ile korunMALIdır. **Hub bu channel'ı TERMINATE EDEMEZ.**

```
Korunan RPC'ler: PUBLISH, VOPRF_BLIND_BATCH, LOOKUP_BATCH

E2E discovery channel şunları hub'dan korur:
  - lookup_key değerleri
  - LOOKUP response entries
  - DiscoveryRecord/CompactDeviceCert response metadata
  - publish payload içeriği

Hub yalnızca görür:
  - client↔hub transport metadata
  - paket boyutu/timing
  - hedef = Discovery Server proxy endpoint

Alpha mekanizması:
  Noise_NK_25519_ChaChaPoly_BLAKE2s
  client ephemeral → Discovery Server static X25519 (DiscoveryEndpointRecord.discovery_transport_x25519_pub, ccr §21.7)
```

> **NORMATİF (red kuralı):** E2E Discovery channel kurulmamışsa client blinded points / lookup_key / publish payload / lookup request GÖNDERMEZ. **Hub-visible plaintext discovery RPC YASAKTIR.** Hub gizliliği VOPRF'e DEĞİL, bu E2E channel'a bağlıdır (VOPRF yalnız blind eval'de raw telefonu gizler).

### 5.5 LOOKUP Response Padding ve Match-Count Leakage (NORMATİF)

Sabit batch request tarafında iyidir; ama response tarafı da match count sızdırabilir.

```
LOOKUP_BATCH request: sabit B lookup_key, rastgele lookup_key'lerle padding.

LOOKUP_BATCH response (Alpha kararı):
  - Request sabit B.
  - Response E2E şifreli (hub okuyamaz).
  - Response variable-size matches-only OLABİLİR.
  - Discovery Server eşleşmeleri bilebilir (kabul edilen Alpha metadata).
  - Hub eşleşmeleri okuyamaz (E2E channel).
  - Network, transport padding profili aktif değilse yaklaşık response boyutunu çıkarabilir.

Beta/security profili: sabit-boyut B şifreli response slot → match-count'u hub/network'ten daha iyi gizler (daha yüksek bant).
```

### 5.6 Discovery Query Log Hardening (NORMATİF)

Discovery Server lookup_key loglarsa, k compromise sonrası geçmiş sorgular telefonlara bağlanabilir.

```
Discovery Server raw lookup_key sorgu listelerini operasyonel gereklilikten uzun TUTMAMALIDIR:
  - raw blinded point default LOGLAMA
  - raw lookup_key değeri default LOGLAMA
  - quota için tam sorgu seti yerine aggregate counter logla
  - abuse soruşturması sorgu logu gerektiriyorsa: at-rest encrypt + tenant-scope + strict retention + access-audited
  - quota counter'ı sorgu içeriğinden AYIR
  - lookup_key loglarını hub'lara ASLA verme
```

> **Threat notu:** lookup_key sorgu logları tutulur ve org OPRF key/HSM sonradan compromise olursa, geçmiş sorgu logları telefon numaralarına map edilebilir. Mitigasyon: minimal retention + encrypted/audited log.

## 6. DiscoveryRecord Publish (Kayıt Anında)

### 6.1 Discovery Endpoint Dağıtımı — DiscoveryEndpointRecord (ccr §21.7)

Discovery Server endpoint'leri ve VOPRF public key'i (`pk_oprf`) client'a **`DiscoveryEndpointRecord`** ile dağıtılır — serbest/imzasız config değeri DEĞİL. **Byte-level canonical layout `ccr-format-spec.md` §21.7'de tanımlıdır** (magic `"SRDE"`, Intermediate-imzalı). İmzalama **HubListRecord (ccr §21.5) mekanizmasının doğal genişlemesidir**: Admin Server hazırlar → RoftCore Intermediate CA otomatik imzalar (server-to-server). Yeni imza protokolü icat edilmez. `pk_oprf` imzaya bind edilir → sahte OPRF public key veya sahte endpoint enjekte edilemez.

```
DiscoveryEndpointRecord (ccr §21.7 — özet; canonical byte layout orada):
  magic="SRDE", version, operator_id, issuer_key_id,
  endpoint_count, endpoints[ {transport_type, endpoint_len, endpoint, priority} ],
  pk_oprf(32), voprf_suite_id(0x0001=ristretto255-SHA512),
  discovery_transport_x25519_pub(32), transport_suite_id(0x0001=Noise_NK_25519_ChaChaPoly_BLAKE2s),
  sequence_number, not_before, not_after (<= license_not_after),
  signature = Ed25519_Sign(sk_intermediate, canonical_discovery_endpoint_record_tbs)
```

> **NORMATİF (client validation — VOPRF'ten ÖNCE):** Client, Discovery Server'a **herhangi bir sorgu yapmadan önce** `DiscoveryEndpointRecord`'u doğrular (ccr §21.7.3 `verify_discovery_endpoint_record`):
> 1. **İmza zinciri:** Intermediate `signature` doğrula → Intermediate chain → **pinned Root** (`ROOT_CA_SPKI_HASH`). Geçmezse ABORT.
> 2. **VOPRF suite:** `voprf_suite_id == 0x0001` (ristretto255-SHA512); başka değer → ABORT (suite-downgrade).
> 2b. **Transport suite:** `transport_suite_id == 0x0001` (Noise_NK); `discovery_transport_x25519_pub` geçerli X25519 (all-zero/low-order → ABORT).
> 3. **pk_oprf:** VOPRF public key'i **bu record'dan** al → verifiable mode proof buna dayanır. **discovery_transport_x25519_pub** da bu record'dan alınır → §5.4 E2E discovery channel için kullanılır.
> 4. **Freshness:** `not_before <= now <= not_after` + `not_after <= OrgEntitlementRecord.license_not_after` + `sequence_number` rollback koruması (cache highest_seen'den düşük → ABORT; eski endpoint/OPRF key replay engeli).
> 5. **Operator scope:** `operator_id` Intermediate namespace'i ile tutarlı (cross-operator endpoint reddi).
> 6. **E2E channel:** PUBLISH/VOPRF_BLIND_BATCH/LOOKUP_BATCH'ten ÖNCE `discovery_transport_x25519_pub` ile E2E discovery channel kur (§5.4); kurulamıyorsa ABORT.
> 7. **Ancak SONRA:** VOPRF blind/lookup (Section 4, 7).
>
> Endpoint/OPRF key rotation `sequence_number` artışıyla yapılır; client daima en yüksek geçerli sequence'i kullanır (stale endpoint/key kabul etmez).

### 6.2 Publish Akışı (Seçenek B — Bob kendi imzalar)

> **NORMATİF (neden Seçenek B):** DiscoveryRecord `user_signature = Ed25519_Sign(sk_user_identity, dr_tbs)` gerektirir (ccr §21). RA Bob'un private key'ine sahip DEĞİLDİR → RA record'u **üretemez**, yalnızca iletebilir. Bu yüzden record'u **Bob kendi `sk_user_identity`'siyle imzalar** (kriptografik binding korunur, §21.2).

```
ONBOARDING PUBLISH (Bob, iyi internet — Section 3'ün devamı):
  1-4. (Bootstrap: cert + HubListRecord + Discovery endpoint alındı, Section 3)
  5. Bob LOKAL: DiscoveryRecord oluşturur (ccr §21 — home_hub_id YOK, CAD-53):
       { magic="SRDR", version, identity_hash, dest_hash, routing_key,
         user_identity_ed25519_pub, current_device_id, current_device_cert_hash,
         current_device_x25519, record_sequence, valid_until, ra_attestation }
       user_signature = Ed25519_Sign(sk_user_identity, dr_tbs)        # Bob'un key'i, RA değil
  6. Bob → Hub (proxy) → Discovery Server:
       PUBLISH { DiscoveryRecord, CompactDeviceCert (152B, inline) }   # cert de gönderilir (Section 7.3)
  7. Discovery Server: doğrula
       - verify_discovery_record (ccr §21.2: user_signature, binding, sequence rollback)
       - ra_attestation doğrula + binding kontrolü (aşağıdaki NORMATİF kurallar)
       - CompactDeviceCert ↔ current_device_cert_hash eşleşmesi
```

> **NORMATİF (ra_attestation binding):** `ra_attestation` EN AZINDAN şunları bind etmelidir: `org_id`, `identity_hash`, `dest_hash`, `user_identity_ed25519_pub`, `current_device_cert_hash`, indeksleme için kullanılan `phone` (veya phone commitment), `issued_at`/`not_after`, RA issuer identity. Discovery Server PUBLISH'i REDDEDER eğer: `ra_attestation.identity_hash != DiscoveryRecord.identity_hash`, veya `ra_attestation.dest_hash != DiscoveryRecord.dest_hash`, veya `ra_attestation.current_device_cert_hash != DiscoveryRecord.current_device_cert_hash`, veya `ra_attestation.org_id != DiscoveryEndpointRecord.operator_id`, veya attestation expired / RA-imzalı değil. Bu, başka birinin record'unu farklı telefon lookup_key altına publish etmeyi engeller.

### 6.3 İndeksleme — `lookup_key`'i Discovery Server Hesaplar (KESİN, Karar 1)

> **NORMATİF (Karar 1 — Discovery Server hesaplar):** Discovery Server, Bob'un record'unu `discovery_lookup_key` ile indekslemelidir ki Alice telefondan bulabilsin. `lookup_key = SHA-256("SafeRoom-Discovery-Lookup-V1" || org_id || VOPRF(k, private_input_bob))`. Bu hesaplama için Bob'un telefonunun VOPRF'i gerekir. **Discovery Server hesaplar** — Bob'un telefonunu **RA push'tan** bilir (RoftCore-controlled RA → org Discovery Server, tenant-scoped audited mTLS; kayıt anında), kendi org OPRF key'iyle direkt VOPRF eval eder, `lookup_key` türetir, indeksler. Bu, **"kayıtta veri kuruma çıkar, discovery'de çıkmaz"** ilkesiyle uyumludur: kayıt anında Discovery Server Bob'un telefonunu indeksleme için kullanır. **Daraltma (NORMATİF):** Kurum/RA/Admin kayıtlı kullanıcıları ve phone→identity eşlemesini bilebilir; AMA Alice'in rehber sorgu seti ve eşleşen contact-graph'ı **ek metadata**'dır. Alpha, org Discovery Server'ın LOOKUP hit'lerinden eşleşen kayıtlı contact'ları çıkarabileceğini kabul eder; bu **hassas metadata** olarak ele alınır ve minimal loglanır (§5.6). ("Kurum zaten biliyor" yalnız kayıtlı kullanıcı listesini kapsar — Alice'in rehberindeki kimlerin eşleştiğini her zaman DEĞİL.) Alice discovery yaparken raw telefon sorgusu **blind**'dır.

```
İNDEKSLEME (Discovery Server, kayıt anında):
  phone_bob          = RA push'tan (RoftCore RA → org Discovery Server; tenant-scoped, audited, mTLS)
  private_input_bob  = "SafeRoom-Discovery-V1" || org_id || normalize(phone_bob) || identity_namespace
  voprf_output_bob   = Finalize(private_input_bob, k · HashToGroup(private_input_bob))   # direkt eval
  lookup_key_bob     = SHA-256("SafeRoom-Discovery-Lookup-V1" || org_id || voprf_output_bob)
  DB[lookup_key_bob] = { DiscoveryRecord_bob, CompactDeviceCert_bob }
```

> **Determinizm garantisi:** VOPRF deterministiktir → Discovery Server'ın direkt hesapladığı `voprf_output_bob`, Alice'in blind→eval→unblind ile elde ettiği değerle **AYNIDIR** (RFC 9497 OPRF correctness). Yani Alice telefonu blind sorgulasa bile aynı `lookup_key_bob`'a ulaşır → Bob'u bulur. Discovery Server Alice'in hangi numarayı sorduğunu cleartext GÖRMEZ (blinding).

---

## 7. DiscoveryRecord Fetch (Alice — Toplu Çekim)

### 7.1 Lazy Discovery REDDİ

> **NORMATİF (#23/9):** Lazy discovery REDDEDİLMİŞTİR. Alice rehberindeki TÜM kişileri **tek seferde** (iyi internette, discovery anında) çeker + cache'ler. Gerekçe: Alice discovery'den hemen sonra hostile ortama (0.025 kbps, %80 kayıp) geçebilir; o anda lazy fetch imkansız. Cache'te her şey hazır olmalı.

### 7.2 Toplu Fetch Akışı

```
DISCOVERY (Alice, iyi internet, TOPLU):
  1. Alice rehberindeki her telefon için blind:
       private_input_i = "SafeRoom-Discovery-V1" || org_id || normalize(phone_i) || identity_namespace
       blinded_i       = r_i · HashToGroup(private_input_i)
  2. Alice → Hub (proxy) → Discovery Server: VOPRF_BLIND_BATCH { blinded_1..blinded_B }   # B sabit (256), padding
  3. Discovery Server: evaluated_i = k · blinded_i + batched DLEQ proof
       (Discovery Server hangi telefonları GÖRMEZ — blinded)
  4. Alice: VerifyProof(pk_oprf, ...) → unblind → voprf_output_i → discovery_lookup_key_i
  5. Alice → Hub (proxy) → Discovery Server: LOOKUP_BATCH { lookup_key_1..lookup_key_B }   # B sabit, padding
  6. Discovery Server: her lookup_key için DB lookup → eşleşenler:
       { DiscoveryRecord_i, CompactDeviceCert_i (152B) }                                   # Karar 2: tam cert
  7. Alice: her eşleşme için:
       - verify_discovery_record (ccr §21.2 binding + sequence rollback)
       - verify_device_cert_chain (CompactDeviceCert → Intermediate(cache) → Root(pin))
       - CompactDeviceCert.device_x25519_pub == DiscoveryRecord.current_device_x25519 (binding)
       → cache (PeerTrustCache, identity §16)
  8. Alice hostile ortama gider → cache'te dest_hash + static X25519 + tam cert var →
       tek-temassız SESSION_INIT gönderebilir (ilk temasta cert_mode=01 inline, ccr §35) → bir daha discovery YAPMAZ
```

### 7.3 LOOKUP Yanıtı — Tam Cert Döner (KESİN, Karar 2)

> **NORMATİF (Karar 2 — tam CompactDeviceCert):** Toplu LOOKUP yanıtı her eşleşme için `DiscoveryRecord` (cert_hash referansı taşır) **+ tam `CompactDeviceCert` (152B)** döner. Gerekçe: Alice ilk `SESSION_INIT`'te Bob'un cert'ini **inline** göndermek zorunda olabilir (cert_mode=01, reference-first cache, ccr §35). Hostile ağda ikinci bir cert-fetch yapamaz. Discovery anında her şeyi (record + cert) toplu alır → cache'inde tam cert hazır. ("Rehberdeki herkesin tüm cert bilgileri aynı anda" ilkesi.)

```
LOOKUP_BATCH response entry:
  { discovery_record : DiscoveryRecord (ccr §21, ~250B),
    device_cert      : CompactDeviceCert (152B, ccr §13) }
  # Identity/User Cert chain: good-network discovery/prefetch sonrası cache'te OLMALIDIR (§8.0).
  #   Cache-miss varsa REF doğrulaması fail-closed/defer olur; bundle yalnız non-hot-path fallback'tir.
  #   Bundle için normatif byte tahmini VERİLMEZ (hostile hot-path budget'a dahil değil).
  # current_device_cert_hash, device_cert'in SHA-256'sına eşit olmalı (Alice doğrular).
```

---

## 8. Cache Modeli

### 8.0 Discovery-Anı Agresif Revocation Prefetch + KnownContactTrustCache (NORMATİF — KAT1-P1-01 Option C)

> **Doktrin:** Lazy discovery reddedilir (§23/9). Sistem "iyi-ağ discovery + cache" doktrinini kullanır.
> **Revocation prefetch artık bu doktrinin parçasıdır** — böylece hostile/blackout'ta known-contact'lar
> için bounded-stale revocation (revocation-model.md §7.5) uygulanabilir.

**İyi ağda contact discovery sırasında (ZORUNLU sıra):**

```
1. Contact'ı VOPRF/discovery ile resolve et.
2. DiscoveryRecordV2 fetch et.
3. Gerekirse DeviceCertBundle fetch et (Identity/User Cert chain).
   NORMATİF (KAT2-P2-03): prefetch TAM CHAIN'i cache'ler — CompactDeviceCert (152B) + issuer Identity/User Cert
   + Intermediate Cert. Amaç: hostile'da GERÇEK ilk-temasta bile client SESSION_INIT_REF (149B) kullanabilsin.
   Chain cache'te olduğundan cert_mode=1 inline path'e veya cert_mode=2 bundle fallback'e düşmemesi HEDEFLENİR.
   SESSION_INIT_BUNDLE hostile lowband hot-path budget'a DAHİL DEĞİLDİR; byte size burada tahmin EDİLMEZ.
   Bundle yalnız cache-miss/correctness fallback'tir ve path/transport policy izin verirse kullanılır;
   aksi halde defer edilir. (Root zaten app bundle'da pinli; chain'in geri kalanı prefetch'te gelir.)
4. FRESH revocation snapshot fetch et / doğrula (aynı iyi-ağ penceresinde).
5. DiscoveryRecordV2 + DeviceCertBundle + CompactDeviceCert'i FRESH revocation state altında validate et.
6. KnownContactTrustCache entry sakla:
     identity_hash
     dest_hash
     current_device_cert_hash
     current_device_x25519_hash
     discovery_record_hash
     cached_cert_chain               # CompactDeviceCert(152B) + Identity/User Cert + Intermediate Cert
                                     #   (KAT2-P2-03: hostile ilk-temasta SESSION_INIT_REF için tam chain hazır)
     revocation_snapshot_sequence / revocation_snapshot_hash
     validated_at
     valid_until / expiry constraints
     operator_id (varsa)
```

**Kurallar (NORMATİF):**

```
- Lazy discovery YİNE reddedilir; prefetch iyi-ağ discovery anında yapılır.
- Cached contact'ın current_device_cert_hash'i DEĞİŞİRSE → contact yeni trust kararından önce
  FRESH revocation gerektirir (bounded-stale cache rotated cert'i yetkilendiremez; INV-REV-NO-SILENT-NEW-DEVICE).
- KnownContactTrustCache entry = revocation-model.md §7.5 "known contact" tanımının kaynağıdır.
- Bu cache yalnızca FRESH revocation state geçerliyken yazılır; stale state altında yazılmaz.
```

> **NORMATİF (field doctrine — yeni contact hostile'da, KAT3-P2-02):** Yeni-contact discovery **iyi-ağ anını gerektirir** (VOPRF good-network + Discovery Server; hub OPRF eval yapamaz, hostile linkte anlamlı proxy edemez). Sürekli-hostile ortamda **önceden keşfedilmemiş yeni contact'a ulaşım GARANTİ EDİLMEZ** (mümkün değil). **Field doctrine:** görev/saha öncesi contact set good-network discovery ile prefetch edilir. Known contact'lar cache + KnownContactTrustCache ile hostile'da çalışır. Bu bir zayıflık değil, operasyon doktrinidir (onboarding/discovery zaten iyi-ağ anına konmuştur).

> **NORMATİF (cert renewal vs yeni cihaz — KAT3-P2-03):**
> ```
> Device Cert renewal (90g): statik pk_x25519 DEĞİŞMEZ (CAD-36 rotation-yok) → Alice'in cache'lediği x25519
>   geçerli kalır → hostile'da bilinen contact ÇALIŞMAYA DEVAM EDER. (device_cert_hash değişir → yeni cert'in
>   revocation freshness'i INV-REV-NO-SILENT-NEW-DEVICE / §7.5 bounded-stale ile ele alınır.)
> Yeni cihaz: yeni device key + YENİ x25519 demektir → hostile cache bunu yetkilendiremez / kripto tutmaz →
>   good-network RE-DISCOVERY gerekir (KAT3-P2-02 ile aynı sınıf bounded limitation).
> ```
> Yani: **cert renewal hostile'da patlatmaz; yeni cihaz re-discovery ister.**

```
PeerTrustCache (identity §16) — discovery sonrası dolu:
  phone_i → { dest_hash, routing_key, user_identity_ed25519_pub,
              current_device_cert_hash, device_pk_x25519 (static),
              CompactDeviceCert (152B, tam), highest_discovery_sequence }
  TTL: Device Cert 90 gün; valid_until'a kadar geçerli.
  Yenileme: yalnızca (a) 90 günde cert expiry yaklaşınca, (b) ilk temasta sequence rollback/mismatch,
            (c) yeni contact eklenince (incremental fetch — sadece yeni numaralar, B'ye padlenir).
```

> **NORMATİF:** Cache, hostile ağda discovery'siz çalışmayı sağlar. Discovery **bir kez** (iyi internet) yapılır; sonraki 90 gün boyunca tekrar gerekmez (Device Cert ömrü). DiscoveryRecord `record_sequence` rollback koruması cache'te tutulur (eski sequence reddedilir).

---

## 9. RPC Envanteri (Client → Hub → Discovery Server)

> Hub yalnızca **proxy**'dir (transport). Payload confidentiality **§5.4 Client↔Discovery Server E2E channel'a bağlıdır** (VOPRF blinding tek başına yetmez — lookup_key ve response E2E channel ile şifrelenir); hub bu channel'ı terminate edemez/içeriği açamaz. Hub gerçek client adresini gizler.

```
Client → Hub → Discovery Server:
  PUBLISH            { DiscoveryRecord, CompactDeviceCert }       → { ack | error }     # kayıt anında (Bob)
  VOPRF_BLIND_BATCH  { blinded_1..blinded_B }                     → { evaluated_1..B, proof }
  LOOKUP_BATCH       { lookup_key_1..lookup_key_B }               → { entries[] (record+cert), padding }

RA push (RoftCore RA → org Discovery Server; tenant-scoped, audited, mTLS — Client değil):
  RA → Discovery Server: REGISTER_PUSH { identity_hash, phone (indeksleme için), ra_attestation }

KALDIRILAN (eski hub-discovery modeli — DEPRECATED, Section 11):
  DISCOVERY_BLIND / DISCOVERY_MATCH / DISCOVERY_FWD_BLIND / DISCOVERY_FWD_MATCH   → VOPRF artık Discovery Server'da
```

---

## 10. Tehdit Modeli

| # | Tehdit | Korunma? | Mekanizma | Residual |
|---|--------|----------|-----------|----------|
| D-1 | Hub Alice'in sorguladığı contact'ları öğrenir | ✓ (E2E discovery channel ile) | VOPRF blinding yalnız raw telefonu blind eval'de gizler; lookup_key'ler ve LOOKUP response da hub'dan şifrelenmeli. Client↔Discovery Server E2E Noise channel (hub proxy üstünde) lookup_key + dönen DiscoveryRecord/CompactDeviceCert entries'i hub'dan gizler (§5.4). **E2E channel yoksa D-1 KORUNMAZ.** | Hub timing/boyut + Alice'in Discovery Server ile temas ettiğini görür |
| D-2 | Discovery Server Alice'in raw telefon sorgularını öğrenir | Kısmi korunma | VOPRF blinding eval sırasında raw telefon numaralarını gizler. AMA LOOKUP_BATCH lookup_key değerlerini ve eşleşen kayıtlı contact'ları Discovery Server'a açar. Full server-private contact discovery PIR/PSI/oblivious response padding gerektirir (Beta+). | Discovery Server authenticated kullanıcılar için eşleşen contact-graph'ı öğrenebilir |
| D-3 | OPRF key (k) olmadan DB leak sonrası offline brute-force | ✓ | lookup_key'ler org OPRF secret k ile keyed; yalnız DB/lookup_key sızıntısı SHA(phone) rainbow table kuramaz | Yok (k olmadan) |
| D-4 | Online enumeration (kayıtlı içeriden) | ✓ | Per-user/org quota + anomali tespiti + sabit batch (Section 5.2) | Quota dahilinde sınırlı sorgu (Signal modeli) |
| D-5 | OPRF key (k) / HSM eval compromise | ⚠ | Saldırgan o org'un telefon uzayını enumerate edip lookup_key'leri map edebilir. Diğer org'lar izole (her org farklı OPRF key). Mitigasyon: HSM-backed key, eval rate-limit, audit log, key rotation, process memory'de raw k yok | Org-scoped enumeration; geçmiş lookup_key logları varsa telefonlara map (§5.6 minimal retention) |
| D-6 | Hub sahte DiscoveryRecord enjekte eder | ✓ | DiscoveryRecord user_signature (sk_user_identity); hub forge edemez | Yok |
| D-7 | Discovery Server sahte record döner | ✓ | user_signature + cert chain (Root pin); Alice doğrular → sahte record reddedilir | Suppress edebilir (D-9) |
| D-8 | VOPRF proof sahteleme (yanlış key) | ✓ | Verifiable mode: VerifyProof(pk_oprf) → yanlış key proof'u geçmez | Yok |
| D-9 | Discovery Server suppression (record gizler) | ⚠ Kabul | Availability residual; Alice contact'ı bulamaz (mesaj atamaz) — kurum güvenilir entity | Kurum kendi kullanıcısını gizlerse kendi zararı |
| D-10 | Onboarding telefon sızıntısı | ⚠ Kabul | SMS doğrulama + RA push telefonu kuruma verir (kaçınılmaz); kurum zaten biliyor (#24) | Belgelenmiş kabul (kurum içi) |
| D-11 | Cross-operator discovery (Alpha) | ✓ | org_id + identity_namespace VOPRF input'ta → farklı kurum farklı output; operatör-izole | Cross-operator Beta (Section 12) |
| D-12 | Well-known endpoint sansür/down | ⚠ Kabul | Yalnızca yeni onboarding gecikir; kayıtlı kullanıcılar hub-proxy ile çalışır (etkilenmez) | Onboarding gecikmesi (Beta: anycast) |
| D-13 | Stale DiscoveryRecord (suppress eski) | ✓ | record_sequence rollback koruması (cache highest_seen) | İlk-temas stale residual |

---

## 11. Deprecated — Eski Hub-Discovery Modeli (MUST NOT IMPLEMENT)

> **Bu bölüm tarihsel referanstır. İmplemente EDİLMEZ.** Eski SRS §6.3 modeli — **hub-local OPRF key** + **cross-hub `DISCOVERY_FWD_BLIND`/`DISCOVERY_FWD_MATCH`** (Alice'in hub'ı blinded query'yi tüm hub'lara forward eder, her hub kendi key'iyle evaluate) — Haziran 2026 kararıyla KALDIRILMIŞTIR (#25).

```
# MUST NOT IMPLEMENT:
- Hub-local OPRF key (her hub kendi k'sı)            → org Discovery Server tek OPRF key (kurum-izole)
- Cross-hub DISCOVERY_FWD_BLIND/MATCH               → VOPRF Discovery Server'da; cross-hub forward YOK
- Hub VOPRF evaluate                                 → hub yalnızca proxy (OPRF key tutmaz)
- DISCOVERY_BLIND(0x02)/DISCOVERY_MATCH(0x03) hub RPC → VOPRF_BLIND_BATCH/LOOKUP_BATCH (Discovery Server)
```

**Neden kaldırıldı:** (1) Hub güvenilmez → hub'da OPRF key paradoks (#25, Section 2). (2) Cross-hub forward → availability tüm hub'ların iyi çalışmasına bağlı (biri kötü/çökük → discovery eksik). (3) Separation: VOPRF/contact-discovery hub'dan çıkınca hub gerçekten yalnızca routing/proxy olur.

---

## 12. Açık Sorular / İleri Faz

```
- Cross-organization discovery (Beta): Model B opt-in cross-cert; iki kurum karşılıklı izin verirse
  operatörler arası contact discovery (RFC 5280 §3.3 cross-certification ile uyumlu).
- OPRF key rotation (Beta operasyonel): yıllık k rotation + grace period; eski/yeni lookup_key
  re-indexing (kayıtlı record'ların lookup_key'i yeni k ile yeniden hesaplanır — kurum içi batch job).
- P-256 / FIPS compatibility profile: CNSA/FIPS şartı çıkan müşteriler için ikinci VOPRF suite
  (NIST P-256). Alpha'da implement edilmez; suite_id rezerve.
- Discovery endpoint resilience: anycast / multi-IP (Beta operasyonel not).
- Sabit batch boyutu B (256?) — performans/gizlilik dengesi ölçülüp netleşecek (TDD).
```

---

## 13. Self-Audit

| Kontrol | Statü | Bölüm |
|---------|-------|-------|
| VOPRF ristretto255-SHA512, RFC 9497, suite 0x0001 (P-256 Beta) | ✅ | 0, 4 |
| Discovery Server kurumda (RoftCore/hub'da değil), org OPRF key HSM-backed | ✅ | 2, 5 |
| Model B: VOPRF lookup_key üretir, dest_hash DEĞİL (CAD-16 korunur) | ✅ | 4.3 |
| DiscoveryRecord home_hub_id TAŞIMAZ (CAD-53); routing = DHT ayrı | ✅ | 1.3 |
| Bootstrap well-known endpoint + ROOT_CA_SPKI_HASH; sonra hub-proxy | ✅ | 3 |
| Hub = proxy (OPRF key yok, telefon görmez, blinding) | ✅ | 2, 9 |
| Publish = Seçenek B (Bob sk_user_identity imzalar, RA üretemez) | ✅ | 6.2 |
| lookup_key Discovery Server hesaplar (RA-known phone, kayıtta) — Karar 1 | ✅ | 6.3 |
| Toplu LOOKUP tam CompactDeviceCert döner — Karar 2 | ✅ | 7.3 |
| Lazy discovery reddi (toplu fetch + cache) | ✅ | 7.1, 8 |
| Rate-limit / enumeration koruması ZORUNLU (Signal modeli) | ✅ | 5.2 |
| Discovery endpoint + pk_oprf = DiscoveryEndpointRecord (ccr §21.7, Intermediate-imzalı, magic SRDE) | ✅ | 6.1, ccr §21.7 |
| pk_oprf imzaya bind (serbest config değil); suite_id + sequence_number + validity client kontrolü | ✅ | 6.1, ccr §21.7.3 |
| Deployment boundary: RA RoftCore-controlled, Discovery Server customer/org infra (ayrık) | ✅ | 2 |
| RA → Discovery Server = tenant-scoped, audited, mTLS push (RA kurumda değil, Discovery Server cert basmaz) | ✅ | 2, 6.3 |
| Eski hub-local key + cross-hub FWD DEPRECATED | ✅ | 11 |
| Cross-operator izolasyon (org_id + identity_namespace VOPRF input'ta) | ✅ | 4.2, D-11 |
| identity v1.5 + ccr §21 + routing-rendezvous ile tutarlı | ✅ | 0, 1.3 |
| Discovery Server raw phone VOPRF eval'de görmez AMA eşleşen lookup hit'leri öğrenebilir | ✅ belgeli | D-2, 5.5 |
| Full server-private contact discovery Alpha'da iddia EDİLMEZ | ✅ | VERDICT, 5.5 |
| Client↔Discovery Server E2E channel (hub proxy üstünde) ZORUNLU | ✅ | 5.4 |
| Hub lookup_key/response göremez — VOPRF değil E2E transport sayesinde | ✅ | 5.4 |
| DiscoveryEndpointRecord pk_oprf + discovery transport key bind eder | ✅ | 6.1 / ccr §21.7 |
| lookup_key logları minimize/retention-limited | ✅ | 5.6 |
| ra_attestation phone/indexing identity'yi DiscoveryRecord alanlarına bind eder | ✅ | 6.2 |

**VERDICT:** Alpha implementation-ready (Discovery transport + privacy-claim reconciliation sonrası). DiscoveryRecord byte layout ccr §21 canonical; bu belge VOPRF/Discovery Server/bootstrap/publish/fetch/cache katmanını tanımlar. Trust sınırı: **Discovery = kimlik (client okur), DHT = konum (hub okur), hub = proxy.**

```
Discovery SAĞLAR:
  - hub-private contact discovery (E2E discovery channel, §5.4)
  - OPRF key olmadan offline DB-leak direnci
  - org-izole VOPRF
  - imzalı DiscoveryRecord authenticity
  - cache-first hostile-network operasyonu

Discovery İDDİA ETMEZ:
  - full server-private contact discovery
  - PIR
  - eşleşen kayıtlı contact'ları org Discovery Server'dan gizleme
```

## 14. Acceptance Criteria — Discovery Freeze Gates

```
AC-DISC-1:  VOPRF çıktısı yalnız discovery_lookup_key türetir; dest_hash ASLA VOPRF'ten türemez.
AC-DISC-2:  Client herhangi discovery RPC'den önce DiscoveryEndpointRecord doğrular; pk_oprf ve
            discovery_transport_x25519_pub yalnız imzalı DiscoveryEndpointRecord'dan alınır.
AC-DISC-3:  Client↔Discovery Server E2E transport (hub proxy üstünde) ZORUNLU; hub-visible plaintext discovery RPC YASAK.
AC-DISC-4:  Hub lookup_key/LOOKUP response entries okuyamaz — VOPRF blinding değil E2E transport sayesinde.
AC-DISC-5:  Discovery Server VOPRF eval'de raw telefon görmez AMA LOOKUP_BATCH hit'lerinden eşleşen
            kayıtlı contact'ları öğrenebilir (belgeli kabul Alpha metadata).
AC-DISC-6:  Belge Alpha'da full server-private contact discovery veya PIR İDDİA ETMEZ.
AC-DISC-7:  OPRF key olmadan DB leak SHA(phone) rainbow table reversal vermez; k/HSM compromise org-scoped.
AC-DISC-8:  Discovery query logları default raw lookup_key listesi tutmaz; istisnai retention encrypted/audited/tenant-scoped/time-limited.
AC-DISC-9:  PUBLISH, identity_hash/dest_hash/user_identity key/current_device_cert_hash/org_id/phone(commitment)'a
            bind RA attestation gerektirir.
AC-DISC-10: LOOKUP response DiscoveryRecord + CompactDeviceCert döner; client şunları doğrular:
            DiscoveryRecord user_signature; CompactDeviceCert hash == current_device_cert_hash;
            current_device_x25519 == CompactDeviceCert.device_x25519_pub; chain → pinned Root; CRLite freshness (identity model).
```
