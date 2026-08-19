# Kama — PKI / CA / RA / HSM TDD

**Durum:** Implementation-ready / freeze-ready Alpha.

**Merge gerekçesi:** Bu belge üç eski kapsamı tek otorite altında birleştirir — `ra-ca-deployment` + `key-management-hsm` + `crypto-primitives`. Doc-sprawl yerine cert üreten her şeyin tek canonical evi.

**Belge otorite sınırı (NORMATİF):**

```
ccr-format-spec.md     : byte-level object format otoritesi (cert/record wire layout).
pki-ca-ra-hsm-tdd.md   : crypto primitive / key policy / CA-RA-HSM otoritesi (bu belge).
transport-layer-tdd.md : transport profile otoritesi.
identity-and-trust-model.md : semantik trust otoritesi.
```

Çelişki halinde: byte-level için ccr-spec, crypto primitive/key policy için bu belge, trust semantiği için identity esastır.

---

## 1. Scope / Non-Goals

Kapsam:

```
- PKI topolojisi (kim nerede)
- Cert hiyerarşisi + validity
- İmza otorite matrisi (kim neyi imzalar)
- Key ceremony (Root offline 3-of-5, Intermediate per-org overlap rollover, License Authority, RA/Admin/Hub/Discovery enrollment)
- RA 3-gate (license gate + admin allowlist gate + device approval / SIM-swap mitigation gate)
- Intermediate auto-sign (Model A otomasyon)
- İmzalanan canonical object'ler
- Lisans enforcement 5 katman (kriptografik zincir)
- Key lifecycle & rotation
- HSM (YubiHSM2 Alpha, PKCS#11)
- DİRAK Beta/open-gap
- Crypto primitives registry (cipher_suite_id 0x0001)
- CSR / enrollment wire akışı + onboarding bundle
- Audit / CT plane (transparency ≠ authorization)
```

Non-goals:

```
- CCR byte-level wire layout (ccr-format-spec.md otoritesi)
- Revocation object formatları (revocation-model.md)
- Admin panel UI / RA-Admin app detayı (admin-control-plane-tdd.md)
- Transport hot-path (transport-layer-tdd.md)
- Root compromise tam disaster-recovery runbook (Ops dokümanı)
```

---

## 2. PKI Topolojisi (kim nerede)

```
RoftCore tarafında (güvenlik-kritik, profesyonel koruma):
  - Root CA                    (offline, ceremony-only HSM)
  - RoftCore License Authority (online HSM, OrgEntitlementRecord imzalar)
  - Intermediate CA            (HER KURUM İÇİN AYRI key; online signing HSM, RA gate arkasında)
  - RA (Registration Authority)(kayıt + loglama, 3 gate)

Kurum tarafında (egemenlik kurumda, zero-trust):
  - Admin Server   (yönetim düzlemi; Intermediate-imzalı cert)
  - Hub'lar        (güvenilmez; istedikleri kadar)
  - Discovery Server (kurum OPRF key, HSM-backed)

Gerekçe (cross-contamination önleme):
  Her kurum AYRI Intermediate key → yalnız o kurumun aktörlerini imzalar.
  RoftCore tek Intermediate işletseydi düşman ona bağlanıp imza alır, başka kurumda da çalışırdı.
  Güvenlik-kritik her şey (Root/License/Intermediate/RA) RoftCore'da; kurum key kaybetse RoftCore'u ilgilendirmez.
```

---

## 3. Cert Hiyerarşisi + Validity

```
Root CA                       20 yıl   (offline, pinned: ROOT_CA_SPKI_HASH)
RoftCore License Authority    1-3 yıl  (önerilen 2 yıl; online HSM, sign-only)
Intermediate CA (per-org)     5 yıl    (kurumun kriptografik namespace omurgası)
Identity CA / User Cert       1 yıl    (routing identity; CompactDeviceCert imzalar)
CompactDeviceCertV1           90 gün   (ağda dolaşan TEK cert; static X25519 içerir)

OrgEntitlementRecord/License  lisans dönemi (genelde 1 yıl)
HubListRecord                 not_after <= license_not_after
Hub Identity Cert             3 yıl
Hub Noise Cert                1 yıl
RA Identity Cert              kurumsal (Intermediate-imzalı)
Admin Server Cert             kurumsal (Intermediate-imzalı)
```

> Validity değerleri identity-and-trust-model.md ile birebir aynıdır. Intermediate = 5 yıl: business-license enforcement mekanizması DEĞİL, kurumun namespace omurgasıdır (her yıl değiştirmek operasyonel kâbus). Lisans enforcement §9'daki 5 katmanla yapılır.

> **NORMATİF (operator isolation — KAT1-P2-02):** Intermediate per-operator namespace ayrımı **chain organizasyonuna** yardımcı olur, ancak **runtime cross-operator authorization** RA/Admin/HubList/AuthorizedHubSet/discovery/routing policy katmanında enforce edilir. **Cert chain geçerliliği tek başına cross-operator iletişim için bir authorization kararı DEĞİLDİR** — Root'a chain eden geçerli bir cert, local operator scope içinde authorized olduğu anlamına gelmez (ccr §13.8 operator-isolation notu + routing-rendezvous-architecture.md ile tutarlı).

---

## 4. İmza Otorite Matrisi (CANONICAL — kim neyi imzalar)

```
Root CA (offline, 20y):
  → Intermediate CA cert (per-org, ceremony)
  → RoftCore License Authority cert (ceremony)
  → emergency / root-level trust update

RoftCore License Authority (online HSM, 1-3y):
  → OrgEntitlementRecord   (YALNIZCA bu)

Intermediate CA (per-org, 5y):
  → Identity CA / User Cert
  → Hub Identity Cert
  → RA Identity Cert
  → Admin Server Cert
  → HubListRecord
  → AuthorizedHubSetRecord
  → DiscoveryEndpointRecord
  → RevocationSnapshot / RevocationDelta checkpoint

Identity / User Cert (1y):
  → CompactDeviceCertV1   (issuer_signature = Ed25519_Sign(sk_user_identity, tbs))

RA Identity:
  → RevocationRecord
  → RevocationPush
  → operational RA attestations (ra_attestation, discovery binding)

Hub Identity:
  → Hub Noise Cert
  → RoutingRecord
  → ANNOUNCE
```

> **KRİTİK NÜANSLAR:**
> - **CompactDeviceCert'i Identity/User key imzalar**, Intermediate DEĞİL (identity-model'de kilitli; ağda tek cert, lokal-imza, offline çalışma).
> - **OrgEntitlementRecord'u License Authority imzalar** — Root DEĞİL (Root offline kalır), Intermediate DEĞİL (Intermediate kurum namespace omurgası; lisans global business authority'dir).
> - License Authority **YALNIZCA** OrgEntitlementRecord imzalar; Identity/Device/Hub/AHS/DiscoveryEndpoint imzalayamaz (§13 CCR registry; ccr-format-spec.md cert_type 0x0A + EKU id-kama-license-authority ile enforce edilir).

---

## 5. Key Ceremony & Enrollment

### 5.1 Root CA — offline, 3-of-5

```
- Root key offline ceremony HSM'de üretilir; ASLA production online signer DEĞİLDİR.
- M-of-N = 3-of-5 (5 keyholder share, 3 ile yetki).
  Gerekçe: tek/iki kişi root'u hareket ettiremez; iki keyholder kaybında sistem kilitlenmez.
- Ceremony: air-gapped, witnessed, append-only ceremony log.
- Root yalnız şunlar için ceremony'ye çıkar: Intermediate cert, License Authority cert, emergency trust update.
- ROOT_CA_SPKI_HASH app bundle'da pinlidir; root pub bundle'dan yüklenir ve hash ile doğrulanır.
```

### 5.2 Intermediate CA — per-org, overlap rollover

```
- Her kurum için AYRI Intermediate key (online signing HSM, RA gate arkasında, server-to-server mTLS).
- 5 yıl validity.

OVERLAP ROLLOVER (NORMATİF — planned rollover / 5y dolumu):
  Eski Intermediate:
    yalnız önceden-issue edilmiş object'leri expiry'sine kadar valide eder.
  Yeni Intermediate:
    yeni Identity/User Cert, HubListRecord, AHS, DiscoveryEndpointRecord imzalar.
  Overlap:
    client/hub her ikisini de KABUL eder (ikisi de pinned Root'a zincirler + revoke değil).
  → HubListRecord, Identity cert cache, DiscoveryEndpointRecord, AHS validation KIRILMAZ.

COMPROMISE durumunda:
  overlap YOK → emergency revocation + root-signed recovery ceremony + operator rollover.
```

### 5.3 RoftCore License Authority

```
- Root-signed cert (ceremony).
- online / HSM-backed sign-only key.
- EKU = id-kama-license-authority.
- private key non-exportable.
- validity 1-3 yıl (önerilen 2 yıl).
- YALNIZCA OrgEntitlementRecord imzalar.
```

### 5.4 RA / Admin / Hub / Discovery enrollment

```
RA Identity Cert      : Intermediate-imzalı; online HSM-backed sign-only.
Admin Server Cert     : Intermediate-imzalı; kurum yönetim makine identity (RA mTLS'te tanır); cert_type 0x0B.
Hub Identity Cert     : Intermediate-imzalı; EKU id-kama-hub; SAN=DNS+IP (ANNOUNCE binding).
Hub Noise Cert        : Hub Identity-imzalı; X25519 keyAgreement.
Discovery Endpoint    : DiscoveryEndpointRecord Intermediate-imzalı (pk_oprf + transport key).
```

### 5.5 Admin Server Cert — ceremony / CSR / lifecycle

```
Issuance:
  1. Kurum Admin Server keypair'i kendi HSM/keystore'unda üretir (RoftCore üretmez).
  2. Private key kurum tarafında kalır; mümkünse non-exportable.
  3. Admin Server CSR, RoftCore ceremony/onboarding sırasında sunulur (operator_id + admin_server_id).
  4. RA operator_id + ceremony onayını doğrular.
  5. Per-org Intermediate Admin Server Cert imzalar (cert_type 0x0B, EKU id-kama-admin-server,
     is_ca=FALSE, key_usage digitalSignature only, validity 1-3 yıl / Alpha önerisi 1 yıl; ccr §17.6).
  6. RA subject_key_id/admin_server_id'yi o operator_id için register eder (Admin↔RA mTLS peer allowlist).

Lifecycle:
  - Rotation: ADMIN_ROTATE_ADMIN_SERVER_CERT → yeni CSR → Intermediate yeni cert → RA registry overlap güncelleme
    (admin-control-plane-tdd.md §8.11).
  - Revocation (normal path): ADMIN_REVOKE_ADMIN_SERVER → RA RevocationRecord (subject_type=ADMIN_SERVER_CERT 0x0B,
    subject = Admin Server cert_hash) + RA registry'den düşür; fresh revocation state ile fail-closed.
  - Revocation (break-glass path): trusted Admin Server compromised/unavailable ise:
      kurum → RoftCore out-of-band emergency contact;
      RoftCore-authorized AdminServerCertRevocationDirective (SRAD) / OCSP-benzeri infra status;
      RA registered_admin_server entry'yi disable eder; revocation-model.md §9.5 otoritedir.
      Root OFFLINE kalır; Alpha default SRAD signer = per-org Intermediate CA (Beta: Root-signed Emergency Revocation Authority).
      Compromised Admin Server kendini revoke etmesi için güvenilmez.

NORMATİF:
  - Admin Server Cert YALNIZCA Admin↔RA mTLS peer authentication (+ opsiyonel kendi control-request envelope imzası).
  - Admin Server hiçbir canonical Kama record/cert imzalamaz (HubList/AHS/DiscoveryEndpoint/Revocation/OrgEntitlement);
    draft hazırlar, RA gate sonrası Intermediate imzalar (admin-control-plane-tdd.md §9).
  - Admin Server Intermediate'e DOĞRUDAN erişmez; RA tek runtime gateway'dir.
```

### 5.6 Hub Enrollment — Pubkey-Bound CSR PoP Model (KAT5-DECISION-1/2)

> Hub enrollment **IP-only OLAMAZ**. IP bir authentication factor DEĞİLDİR — yalnız SAN policy, rate-limit, coarse pre-filter ve ops visibility için kullanılır. Hub kimliği = **Ed25519 SPKI fingerprint allowlist + CSR proof-of-possession** (Admin Server enrollment §5.5 ile aynı disiplin: ağ-katmanı kimliği değil kriptografik possession).

```
1. Hub, lokal Hub Identity Ed25519 keypair üretir.
   Private key mümkünse HSM/TPM/non-exportable keystore'da tutulur.

2. Admin, hub provisioning sırasında hub_identity_spki_hash'i Admin Server'a girer
   (IP'yi girdiği anın aynısı — TEK ek alan, seromoni YOK):
     hub_identity_spki_hash = SHA-256(spki_encoding(Ed25519, hub_identity_ed25519_pub))

3. Admin Server → RA (mTLS): HubEnrollmentAllowlistEntry {
     operator_id, hub_id, hub_identity_spki_hash,
     expected SAN IP/DNS listesi, hub_role/priority, enrollment_window }.
   RA bunu operator_id başına cache'ler (admin-control-plane-tdd.md §8.2b).

4. Hub → RA: { operator_id, hub_id, Hub Identity CSR, CSR proof-of-possession,
               requested SAN IP/DNS, optional attestation }.

5. RA doğrular (HEPSİ):
     - operator_id scope
     - hub_id allowlisted mı
     - SHA-256(spki(CSR.public_key)) == allowlisted hub_identity_spki_hash
     - CSR proof-of-possession geçerli (hub, sk_hub_identity possession kanıtlar)
     - requested SAN IP/DNS, allowlist SAN policy ile uyumlu
     - enrollment_window geçerli
     - license / org policy uygun

6. CSR pubkey fingerprint allowlist'te DEĞİLSE → Hub Identity Cert issuance YOK.

7. Source IP TEK BAŞINA hiçbir zaman issuance sebebi olamaz
   (yalnız coarse pre-filter / rate-limit / anomaly detection / expected-SAN policy).

8. Geçerse Intermediate Hub Identity Cert imzalar (cert_type 0x05, EKU id-kama-hub, SAN=DNS+IP).

9. Hub Noise keypair üretir; Hub Noise Cert Hub Identity Cert altında imzalanır
   (cert_type 0x06, X25519, keyAgreement, issuer = Hub Identity Cert).

10. RA/Admin pipeline yeni HubListRecord (cert-bound HubEntryV3, ccr §21.5) /
    BootstrapBundle (ccr §21.8) üretir. Hub cert + liste ile ANNOUNCE yapar
    (routing-rendezvous-architecture.md §3; self-healing DHT devralır).
```

**Saldırı kapanışı (KAT5-P1-01):** BGP hijack, on-path active substitution veya stale cloud IP reuse saldırganı, allowlisted hub'ın **private key'ine sahip olmadığından** geçerli CSR PoP üretemez → hangi IP'den gelirse gelsin Hub Identity Cert ALAMAZ. IP-only DV-CA mis-issuance sınıfı kapanır. Network MITM yalnız DoS/gecikme yapabilir; rogue hub BASAMAZ (availability düşebilir, authenticity korunur).

**FORBIDDEN (yasak ifadeler):**
```
- "allowlisted IP'den geldiyse Hub Cert issue edilir"
- "source IP hub authentication factor'dır"
- "IP ownership is sufficient for hub enrollment"
- "Hub CSR pubkey allowlist gerekmez"
```

---

## 6. RA 3-Gate

```
Gate-1 — license gate (CRYPTOGRAPHIC):
  RA, CSR'yi Intermediate'e GÖNDERMEDEN önce OrgEntitlementRecord (license_not_after + seat_limit +
  renewal policy) kontrol eder.
  10'luk paket, 11. kayıt → RA Intermediate'e HİÇ göndermez → 0x0602 SEAT_LIMIT.
  Mali kontrol kriptografiye gömülü: lisans = sertifika basma hakkı.

Gate-2 — admin allowlist gate:
  Admin Server RA'ya "bu kullanıcı/org üyesi enrollment'a authorized" bildirmiş mi (allowlist)?
  Bildirmemişse → reject; telefon/IP/konum'u Admin Server'a push + ŞÜPHELİ işaretle.

Gate-3 — device approval / SIM-swap mitigation gate:
  Yeni cihaz enrollment veya riskli device rotation için admin/device approval gerekir.
  SIM-swap mitigasyonu: SMS doğrulama + Admin cihaz onayı ("bu cihaz gerçekten X mi").

NORMATİF: Intermediate auto-sign YALNIZCA bu 3 gate geçtikten sonra çalışır. RA gate geçmeden
Intermediate imza ÜRETMEZ.
```

---

## 7. Intermediate Auto-Sign (Model A Otomasyon)

```
Akış (sürtünme yok, güvenlik RoftCore'da):
  1. Admin Server → RA (mTLS): authorized user/device + CSR.
  2. RA: Gate-1 (license/seat) + Gate-2 (admin allowlist) + Gate-3 (device approval / SIM-swap) kontrol.
  3. Uygunsa: Intermediate CA OTOMATİK imzalar (server-to-server, düşük RTT, iyi internet varsayımı).
  4. RA → kuruma: imzalı cert / HubListRecord / AHS / DiscoveryEndpointRecord.
  5. RA → Admin Server PUSH: telefon + dest_hash + identity_hash + kayıt zamanı + konum (kurum loglar, denetlenebilirlik).

NORMATİF:
  - Client RA'ya DOĞRUDAN bağlanmaz (post-onboarding) → Client → Hub → RA (proxy, gerçek adres gizli).
  - HubListRecord/AHS/DiscoveryEndpointRecord: Admin Server hazırlar → Intermediate otomatik imzalar.
  - Admin Server kendi cert'iyle (Intermediate-imzalı) RA mTLS'te tanınır; rastgele sunucu değil, PKI aktörü.
```

---

## 8. İmzalanan Canonical Object'ler (referans)

```
Object                    İmzalayan              ccr-spec ref
CompactDeviceCertV1       Identity/User Cert     §13
Identity/User Cert        Intermediate CA        §6
HubListRecord (SRHL)      Intermediate CA        §21.5
AuthorizedHubSetRecord    Intermediate CA (RA gate)  SRAH
DiscoveryEndpointRecord   Intermediate CA        §21.7
OrgEntitlementRecord(SROE)RoftCore License Authority  §21.6  ← signer DEĞİŞTİ (License Authority)
RoutingRecord (SRRR)      Hub Identity           routing-rendezvous §...
RevocationRecord/Push     RA Identity            revocation-model §3
RevocationSnapshot/Delta  Intermediate CA        revocation-model §3
```

> Tüm imza doğrulamaları recursive verify_chain (RFC 5280 §6) ile ROOT_CA_SPKI_HASH'e kadar; Root pub bundle'dan, ağdan gelmez.

---

## 9. Lisans Enforcement — 5 Katman (kriptografik zincir)

Intermediate expiry lisans enforcement DEĞİLDİR. Enforcement 5 katmanda:

```
1. RA license gate:
   lisans yoksa Identity Cert vermez, Device enrollment onaylamaz, HubListRecord yenilemez, Admin taleplerini reddeder.

2. OrgEntitlementRecord (License Authority-imzalı):
   org_id + intermediate_key_id + seat_limit + license_not_before/after + allowed_features + record_sequence + signature.

3. Identity/User Cert yıllık yenileme:
   RA license gate + Admin approval üzerinden.

4. HubListRecord validity:
   not_after <= license_not_after.

5. CRLite / revocation:
   erken termination (revocation-model.md).
```

Kriptografik zincir (lisans bitince ne olur):

```
IdentityCert.not_after  <= OrgEntitlementRecord.license_not_after
DeviceCert.not_after    <= min(IdentityCert.not_after, org_license_not_after)
HubListRecord.not_after <= license_not_after

→ Lisans bitince: Identity Cert yenilenmez (1y) → Device parent'ını aşamaz → routing durur.
  AMA Intermediate (5y, namespace omurgası) bozulmaz.
  Bu yazılımsal kill-switch DEĞİL; PKI'nin doğal sonucu (yenilenmeme = kriptografik termination).
  Yıllık B2B lisans yenileme kriptografiye gömülü.
```

---

## 10. Key Lifecycle & Rotation

```
Key                         Ömür    Rotation tetikleyici            Rollover
Root CA                     20y     compromise / planned (nadir)    ceremony, client trust update
License Authority           1-3y    expiry / compromise             Root re-sign, overlap
Intermediate CA (per-org)   5y      expiry / compromise             overlap rollover (§5.2)
Identity/User Cert          1y      yıllık yenileme / key rotation  RA gate + admin approval
CompactDeviceCert           90g     otomatik yenileme (iyi internet) Identity yeniden imzalar; static X25519 SABİT kalır
Hub Identity                3y      expiry / compromise             Intermediate re-issue
Hub Noise                   1y      expiry / rotation               Hub Identity re-issue; IKfallback ile dağıtım
RA Identity / Admin Cert    kurumsal expiry / compromise            Intermediate re-issue
OPRF key (discovery)        yıllık  rotation                        30g grace, re-register
```

Normatif notlar:

```
- CompactDeviceCert 90g'de yenilenir ama içindeki static X25519 DEĞİŞMEZ (async cache bayatlamaz; identity-model #19).
- Private key'ler ASLA exportable değildir (HSM server tarafı; StrongBox/TEE client tarafı).
- Key rotation = yeni cert + eski cert overlap (mümkün olan yerde) + revocation (gerekirse).
```

---

## 11. HSM (YubiHSM2 Alpha, PKCS#11)

### 11.1 Key yerleşimi ve erişim politikası (DÜZELTİLDİ — Root online değil)

```
Root CA key:
  offline ceremony HSM / offline-only.
  production online signer DEĞİLDİR.
  3-of-5 M-of-N erişim.

Intermediate CA key (per-org):
  online signing HSM, tightly scoped.
  server-to-server mTLS, RA gate arkasında.
  export YASAK.

RA Identity key:
  online HSM-backed, sign-only.

RoftCore License Authority key:
  online HSM-backed, sign-only.
  YALNIZCA OrgEntitlementRecord imza yetkisi.

(Kurum tarafı: Admin Server / Hub / Discovery kendi HSM/keystore'unu işletir; RoftCore'u ilgilendirmez.)
```

### 11.2 PKCS#11 abstraction

```
- Tüm imza işlemleri PKCS#11 üzerinden (crypto-agility için temiz abstraction).
- Key labels: root_ca / license_authority / int_ca_<org> / ra_identity_<org>.
- Access policy: Root offline-only + M-of-N; Intermediate/RA/License online sign-only, export yasak.
- Backup/recovery: wrapped key backup (HSM-to-HSM), M-of-N restore.
- Ceremony + sign log: append-only, audit.
```

---

## 12. DİRAK HSM — Beta / Open-Gap

```
- TÜBİTAK BİLGEM DİRAK HSM savunma audit için tercih edilen yol.
- Ed25519 / PKCS#11 API desteği TEYİTSİZ → Beta / open-gap.
- Ed25519/API desteği DOĞRULANMADAN "DİRAK Alpha destekliyor" İDDİA EDİLMEZ.
- PKCS#11 abstraction (§11.2) DİRAK swap-in'i kolaylaştırır (kod değişmeden HSM değişir).
- Alpha kesin yol: YubiHSM2 / doğrulanmış Ed25519-capable path.
```

---

## 13. Crypto Primitives Registry (cipher_suite_id 0x0001)

```
Signature : Ed25519 (RFC 8032) — tüm cert/record imzaları
KEX / DH  : X25519 (RFC 7748) — Noise + async; all-zero/low-order output → reject
AEAD      : ChaCha20-Poly1305 (RFC 8439) — key 32B / nonce 12B / tag 16B
KDF       : HKDF-BLAKE2s (RFC 5869 yapısı, hash = BLAKE2s-256)
Hash      : SHA-256 (dest_hash / identity_hash / cert_hash / device_cert_hash); BLAKE2s (KDF)
VOPRF     : ristretto255-SHA512 (RFC 9497 VOPRF mode 0x01, suite 0x0001) — discovery
```

Domain separation (özet):

```
dest_hash       = SHA-256(SHA-256(user_identity_ed25519_pub))   # CAD-16
identity_hash   = SHA-256(user_identity_ed25519_pub)
device_cert_hash= SHA-256(canonical_compact_device_cert)        # 152B
Discovery input = "SafeRoom-Discovery-V1" || org_id || normalized_phone || identity_namespace
TBS magic'ler   = "SRCR"/"SRDR"/"SRAM"/"SRHL"/"SROE"/"SRAH"/"SRRR"/"SRRV"/"SRSN"/"SRDL"/"SRPU" (domain separation)
```

PQC ve crypto-agility:

```
- ML-KEM Noise hibridizasyonu = Beta birincil quantum yatırımı.
- PQC imzalar ERTELENDİ (wire maliyeti leaf/device cert için kabul edilemez).
- Crypto-agility ŞİMDİ kurulur: versioned cipher_suite_id + temiz PKCS#11 abstraction.
- KRİTİK-02: identity_hash'e alg-prefix, ikinci 32-byte algoritma (PQC) eklenmeden ÖNCE eklenecek
  (şimdi değil; routing kırılır, Alpha tek-algoritma Ed25519 güvenli).
```

---

## 14. CSR / Enrollment Wire Akışı + Onboarding Bundle

```
İlk onboarding (bootstrap-direct):
  1. App bundle: ROOT_CA_SPKI_HASH + ROFTCORE_ONBOARDING_ENDPOINT (well-known).
  2. Client → RA (DOĞRUDAN; hub listesi yok): org_id + CSR (Identity + Device pub).
  3. RA: Gate-1 (license/seat) + Gate-2 (admin allowlist) + Gate-3 (device approval / SIM-swap).
  4. Intermediate: Identity Cert imzalar; Identity: CompactDeviceCert imzalar.
  5. RA → ilk bundle:
       Identity/User Cert + CompactDeviceCert
       + HubListRecord (Intermediate-imzalı IP SAN)
       + DiscoveryEndpointRecord (Intermediate-imzalı)
       + OrgEntitlementRecord (License Authority-imzalı, cache için)
  6. Client cert'leri ROOT_CA_SPKI_HASH'e kadar doğrular (recursive verify_chain).

Post-onboarding:
  Client → Hub → RA (proxy, REGISTER_OR_RA_PROXY; application-protocol-and-rpc-spec.md §6.6).
```

---

## 15. Audit / CT Plane

```
- CT / audit log: append-only, transparency ≠ authorization.
- CT log AUTHORIZATION DEĞİLDİR; yalnız audit/transparency (identity-model).
- RA'da telefon → dest_hash + identity_hash kaydı (denetlenebilirlik; kurum loglar).
- İçerik ALINMAZ (E2E). Hub'lar kim-kimle metadata DEFAULT tutmaz (minimize).
- Intermediate her imza için X.509 DER (CA audit DB) + CCR (wire) aynı TBS+sig ile üretir.
- Revocation/audit log SİLME yasak (append-only, denetim bütünlüğü).
```

---

## 16. Threat Model

```
| ID       | Tehdit                                                  | Statü                | Mitigasyon |
|----------|---------------------------------------------------------|----------------------|------------|
| R-PKI-1  | Root online compromise                                  | Closed               | Root offline ceremony-only HSM; production online signer değil. |
| R-PKI-2  | Tek Intermediate cross-contamination                    | Closed               | per-org ayrı Intermediate key + RA allowlist gate. |
| R-PKI-3  | License Authority Identity/Device imzalar (yetki aşımı) | Closed               | EKU id-kama-license-authority; CCR registry yalnız OrgEntitlement (§13/ccr patch). |
| R-PKI-4  | Intermediate rollover sırasında trust kırılır           | Closed               | overlap rollover; iki Intermediate de pinned Root'a zincirler. |
| R-PKI-5  | Lisans bitince Intermediate de bozulur (kâbus)          | Closed               | enforcement Identity/Device/HubList katmanında; Intermediate stabil. |
| R-PKI-6  | RA seat limit bypass (sınırsız cert)                    | Closed               | Gate-1 cryptographic; CSR Intermediate'e hiç gitmez. |
| R-PKI-7  | Kötü admin sessiz cihaz ekler                           | Closed               | sk_device possession şart (CLIENT_BIND, Noise IK). |
| R-PKI-8  | Private key export                                      | Closed               | HSM export yasak; StrongBox/TEE client. |
| R-PKI-9  | DİRAK Ed25519 desteklemiyor, prod patlar                | Mitigated            | DİRAK Beta/open-gap; Alpha YubiHSM2; PKCS#11 swap-in. |
| R-PKI-10 | Root key ceremony tek kişi kontrolü                     | Closed               | 3-of-5 M-of-N. |
| R-PKI-11 | CT log authorization sanılır                            | Closed               | transparency ≠ authorization; trust kararı CT'ye dayanmaz. |
| R-PKI-12 | Hub enrollment IP-only mis-issuance (BGP hijack/on-path/stale cloud IP) | Closed | KAT5: pubkey-fingerprint allowlist + CSR PoP; IP authentication factor DEĞİL (§5.6). |
```

---

## 17. Self-Audit

```
| Kontrol                                                              | Statü | Bölüm |
|----------------------------------------------------------------------|-------|-------|
| Root offline ceremony-only (online signer değil)                    | ✅    | §5.1,§11.1 |
| Root ceremony 3-of-5 M-of-N                                          | ✅    | §5.1  |
| Intermediate per-org (cross-contamination kapalı)                   | ✅    | §2,§5.2 |
| Intermediate overlap rollover                                       | ✅    | §5.2  |
| License Authority ayrı signer (Root-signed, online HSM)             | ✅    | §4,§5.3 |
| License Authority YALNIZCA OrgEntitlementRecord imzalar             | ✅    | §4,§13 |
| OrgEntitlement Root/Intermediate imzalamaz                          | ✅    | §4    |
| CompactDeviceCert'i Identity/User key imzalar (Intermediate değil)  | ✅    | §4    |
| RA 3-gate (license/seat + admin allowlist + device approval/SIM-swap) | ✅    | §6    |
| Intermediate auto-sign Model A (server-to-server mTLS)              | ✅    | §7    |
| Lisans enforcement 5 katman + kriptografik zincir                   | ✅    | §9    |
| Intermediate 5y enforcement mekanizması DEĞİL (namespace omurgası)  | ✅    | §3,§9 |
| static X25519 90g rotation'da SABİT                                 | ✅    | §10   |
| Private key non-exportable (HSM/StrongBox)                          | ✅    | §10,§11 |
| YubiHSM2 Alpha; DİRAK Beta/open-gap                                  | ✅    | §11,§12 |
| Crypto primitives cipher_suite_id 0x0001 registry                  | ✅    | §13   |
| PQC = ML-KEM Noise hibrit Beta; PQC imza ertelendi                  | ✅    | §13   |
| CT transparency ≠ authorization                                     | ✅    | §15   |
```

---

## 18. Acceptance Criteria

```
AC-PKI-1:  Root CA key offline ceremony-only; production online signer olarak kullanılmaz.

AC-PKI-2:  Root ceremony M-of-N = 3-of-5.

AC-PKI-3:  Her kurum için ayrı Intermediate key; cross-operator imza imkânsız.

AC-PKI-4:  Intermediate rollover overlap'tir; grace boyunca iki Intermediate de pinned Root'a zincirler.

AC-PKI-5:  OrgEntitlementRecord yalnız RoftCore License Authority tarafından imzalanır;
           Root ve Intermediate OrgEntitlementRecord imzalamaz.

AC-PKI-6:  License Authority cert EKU = id-kama-license-authority; yalnız OrgEntitlementRecord imza yetkisi
           (Identity/Device/Hub/AHS/DiscoveryEndpoint imzalayamaz).

AC-PKI-7:  CompactDeviceCert issuer_signature = Ed25519_Sign(sk_user_identity); Intermediate imzalamaz.

AC-PKI-8:  RA Gate-1 (seat) cryptographic: limit aşımında CSR Intermediate'e gönderilmez (0x0602 SEAT_LIMIT).

AC-PKI-9:  Lisans bitince Identity yenilenmez → Device parent'ı aşamaz → routing durur; Intermediate bozulmaz.

AC-PKI-10: Tüm private key'ler HSM/StrongBox'ta non-exportable.

AC-PKI-11: Alpha HSM YubiHSM2 (Ed25519-capable); DİRAK Beta/open-gap (Ed25519/API teyidi sonrası).

AC-PKI-HUB-ENROLL-1: RA, Hub Identity Cert issuance ÖNCESİ CSR proof-of-possession doğrular.

AC-PKI-HUB-ENROLL-2: RA, CSR public key fingerprint'in HubEnrollmentAllowlistEntry.hub_identity_spki_hash ile eşleştiğini doğrular.

AC-PKI-HUB-ENROLL-3: Source IP tek başına Hub Identity Cert issuance için yeterli DEĞİLDİR.

AC-PKI-HUB-ENROLL-4: BGP hijack / on-path active substitution / stale cloud IP reuse saldırganı,
                     allowlisted hub private key'e sahip değilse Hub Identity Cert ALAMAZ.

AC-PKI-HUB-ENROLL-5: IP/SAN yalnız policy/operational guardrail; kriptografik kimlik Ed25519 SPKI fingerprint + CSR PoP'tur.

AC-PKI-12: cipher_suite_id 0x0001 = Ed25519 / X25519 / ChaCha20-Poly1305 / HKDF-BLAKE2s / SHA-256.

AC-PKI-13: Tüm imza doğrulamaları recursive verify_chain ile ROOT_CA_SPKI_HASH'e; Root pub bundle'dan.
```

---

## 19. Forbidden Wording / Cleanup

```
YASAK (active):
  - "Root online HSM'de tutulur / online signer"
  - "Root OrgEntitlementRecord imzalar"
  - "Intermediate OrgEntitlementRecord imzalar"
  - "Intermediate CompactDeviceCert imzalar"
  - "License Authority Identity/Device/Hub imzalar"
  - "Intermediate expiry lisans enforcement mekanizmasıdır"
  - "DİRAK Alpha destekliyor"
  - "static X25519 her 90 günde değişir"
  - "CT log authorization sağlar"
  - "source IP hub identity kabul edilir"
  - "allowlisted IP'den gelen Hub CSR imzalanır"
  - "IP ownership Hub Identity Cert issuance için yeterlidir"
  - "Hub CSR proof-of-possession gerekmez"
  - "Hub Identity CSR public key fingerprint allowlist kontrolü olmadan cert issue edilir"

İZİNLİ:
  - Root offline ceremony-only (3-of-5)
  - License Authority → OrgEntitlementRecord only
  - Intermediate → Identity/Hub/RA/Admin + records
  - Identity/User Cert → CompactDeviceCert
  - overlap rollover
  - YubiHSM2 Alpha / DİRAK Beta
  - lisans enforcement 5 katman (Identity/Device/HubList chain)
  - Hub enrollment = hub Ed25519 SPKI fingerprint allowlist + CSR PoP
  - IP/SAN = policy / operational guardrail / rate-limit / anomaly signal
  - source IP authentication factor değildir
```

---

## 20. Final Verdict

```
Final Status:
  PKI / CA / RA / HSM TDD Alpha için freeze-ready.

Frozen (kilitli):
  - PKI topolojisi (Root/License/Intermediate/RA RoftCore; Admin/Hub/Discovery kurum)
  - validity: Root 20y / License 1-3y / Intermediate 5y / Identity 1y / Device 90g
  - imza otorite matrisi (License Authority → OrgEntitlement; Identity → Device; Intermediate → records)
  - Root offline ceremony 3-of-5; Intermediate overlap rollover
  - RA 3-gate + Intermediate auto-sign (Model A)
  - lisans enforcement 5 katman + kriptografik zincir
  - YubiHSM2 Alpha (PKCS#11, non-exportable); DİRAK Beta/open-gap
  - crypto primitives registry (cipher_suite_id 0x0001)
  - crypto-agility (versioned suite + PKCS#11); PQC ML-KEM Beta

Deferred (ertelenen):
  - DİRAK HSM profili (Ed25519/API teyidi)
  - PQC imza (wire maliyeti); ML-KEM Noise hibrit Beta
  - Root disaster-recovery runbook tam detayı (Ops)
  - License Authority dış denetim/compliance profili (Beta)

Closed cross-doc consistency:
  ccr-format-spec.md License Authority registry / EKU / signing rule ile uyumludur.
  License Authority yalnız OrgEntitlementRecord imzalar.
  Root, Intermediate, RA, Admin Server ve Hub bu kayıtları imzalamaz.
```

Bu belge yazıldıktan sonra `identity-and-trust-model.md`, `ccr-format-spec.md`, `revocation-model.md`, `routing-rendezvous-architecture.md`, `discovery-architecture.md` ve `application-protocol-and-rpc-spec.md` ile çelişmez. License Authority signer rolü ccr-format-spec.md registry'sine yansıtılmıştır (cross-doc consistency kapandı).
