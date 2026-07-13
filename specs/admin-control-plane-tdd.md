# SafeRoom — Admin Control Plane TDD

**Durum:** Implementation-ready / freeze-ready Alpha.

**Amaç:** Kurum Admin Server ↔ RoftCore RA arasındaki trust, transport, method, audit ve revocation kontrol sözleşmesini tanımlar.

**Belge otorite sınırı (NORMATİF):**

```
admin-control-plane-tdd.md   : Admin Server identity, Admin↔RA method contract, admin RBAC/audit,
                               allowlist sync, admin-triggered record preparation, suspend/revoke lifecycle (bu belge).
pki-ca-ra-hsm-tdd.md         : PKI topology, key ceremony, Admin Server Cert issuance, HSM/key policy.
ccr-format-spec.md           : Admin Server Cert byte layout / cert_type / EKU + canonical record formatları.
revocation-model.md          : RevocationRecord/Snapshot/Delta/Push object formatları + freshness.
application-protocol-and-rpc-spec.md : Client↔Hub↔RA proxy (client'lar için; Admin↔RA DEĞİL).
```

Bu belge `identity-and-trust-model.md`, `pki-ca-ra-hsm-tdd.md`, `ccr-format-spec.md`, `revocation-model.md` ve `application-protocol-and-rpc-spec.md` ile %100 uyumludur.

---

## 1. Scope / Non-Goals

Kapsam:

```
- Admin Server identity ve trust modeli
- Admin Server Cert ceremony / CSR / validation gereksinimleri (referans)
- Admin↔RA transport kontratı (mTLS/gRPC)
- AdminControlRequest ortak envelope
- Admin↔RA method seti
- Allowlist sync modeli (Gate-2 kaynağı)
- HubList / AHS / DiscoveryEndpoint draft hazırlama
- RA 3-gate enforcement (license + admin allowlist + device approval/SIM-swap)
- Intermediate auto-sign Model A
- Admin-triggered suspend / deactivate / device revoke / emergency revoke
- RA → Admin audit push
- License seat query/status
- Admin panel grants / prohibitions
- RBAC + emergency roller
- SSO / LDAP / SCIM Alpha/Beta sınırı
- Threat model, self-audit, AC, forbidden wording
```

Non-goals:

```
- Admin UI tasarımı
- İnsan kimlik sağlayıcı iç mekaniği
- Client↔Hub app protocol (application-protocol-and-rpc-spec.md)
- CCR byte-level layout (ccr-format-spec.md)
- Revocation object byte formatı (revocation-model.md)
- HSM düşük-seviye vendor komutları
- Root disaster recovery runbook
```

---

## 2. Çekirdek İlke — "Admin olunmaz; Admin Server yetkilendirilir" (NORMATİF)

```
Bir sunucu kendini Admin Server İLAN EDEMEZ.

Admin Server yetkisi sertifika-tabanlıdır:
  - Kurum Admin Server'ı kendi keypair'ini kurum-kontrollü HSM/keystore'da üretir.
  - Kurum, RoftCore ceremony/onboarding sırasında CSR sunar.
  - Per-org Intermediate CA bir Admin Server Cert imzalar.
  - RA, o Admin Server Cert / subject_key_id'yi ilgili operator_id için register eder.
  - RA admin-plane method'larını YALNIZCA bu doğrulanmış Admin Server Cert üzerinden (mTLS) kabul eder.

Hiçbir software flag, DNS adı, IP adresi, bearer token, config dosyası, ortam değişkeni veya
veritabanı satırı, Intermediate-imzalı Admin Server Cert olmadan Admin Server yetkisi YARATAMAZ.
```

Makine kimliği ile insan admini ayrımı:

```
Admin Server Identity:
  makine kimliği.
  Intermediate-imzalı Admin Server Cert.
  RA mTLS peer kimliği.

Admin Actor Identity:
  kurum içindeki insan admin / güvenlik görevlisi / operatör.
  request payload/audit'te admin_actor_id olarak temsil edilir.
  kurum RBAC'ı tarafından yönetilir.

RA önce Admin Server makine kimliğini authenticate eder.
Sonra request'in admin_actor_id / RBAC / method scope'unu kontrol eder.
```

---

## 3. Cross-Doc Gereksinim — CCR / PKI Patch

Bu belge CCR ve PKI'da şu patch'leri gerektirir (belge sonunda cross-doc patch listesi tekrarlanır):

```
CCR (ccr-format-spec.md):
  cert_type:  0x0B Admin Server
  EKU:        0x0008 id-saferoom-admin-server

  Admin Server CCR:
    cert_type       = 0x0B
    public_key_alg  = Ed25519
    is_ca           = FALSE
    key_usage       = digitalSignature only
    eku_list        = [0x0008 id-saferoom-admin-server]
    issuer          = per-org Intermediate CA
    validity        = 1-3 yıl (Alpha önerisi 1 yıl)
    extensions      = operator_id / admin_server_id / SAN

  Allowed use:
    Admin Server Cert YALNIZCA Admin↔RA mTLS peer authentication
    (+ opsiyonel admin-control request envelope imzası) için kullanılır.

  Forbidden (hepsi authorization ihlali):
    Admin Server Cert → Identity/User Cert / CompactDeviceCert / HubListRecord /
    AuthorizedHubSetRecord / DiscoveryEndpointRecord / RevocationRecord/Push /
    RevocationSnapshot/Delta / OrgEntitlementRecord  imzalayamaz.

PKI (pki-ca-ra-hsm-tdd.md):
  Admin Server Cert ceremony / CSR / lifecycle bölümü.
```

PKI ceremony referansı:

```
Admin Server Cert issuance:
  1. Kurum Admin Server keypair'i lokal üretir.
  2. Private key kurum tarafında kalır, mümkünse non-exportable.
  3. Admin Server CSR, RoftCore ceremony/onboarding sırasında sunulur.
  4. RA operator_id + ceremony onayını doğrular.
  5. Per-org Intermediate Admin Server Cert imzalar.
  6. RA subject_key_id/admin_server_id'yi o operator_id için register eder.
```

---

## 4. Aktör Topolojisi

```
Kurum tarafı:
  Admin Server:
    - Admin Server private key'i tutar.
    - RA'ya doğru mTLS client.
    - Gate-2 allowlist + admin policy kaynağı.
    - HubList/AHS/DiscoveryEndpoint için imzasız taslak hazırlar.

RoftCore tarafı:
  RA:
    - tek Admin↔RA giriş noktası.
    - Admin Server Cert'i doğrular.
    - RA 3-gate uygular (license + admin allowlist + device approval/SIM-swap).
    - geçerli imza işlerini Intermediate'e iletir.
    - operasyonel revocation record'larını RA Identity ile imzalar.
    - audit/revocation callback yayınlar.

  Intermediate CA:
    - RoftCore-iç, RA arkasında.
    - Admin Server ASLA doğrudan bağlanmaz.
    - RA gate sonrası Intermediate-authorized record/cert imzalar.

  Root CA:
    - offline ceremony-only.
    - Admin↔RA runtime'ının parçası değil.

  License Authority:
    - OrgEntitlementRecord imzalar.
    - admin method imzalamanın parçası değil.
```

Kritik topoloji kuralı:

```
Admin Server → RA → Intermediate

Admin Server, Intermediate CA'ya DOĞRUDAN BAĞLANAMAZ.
Intermediate CA, doğrudan Admin Server imza talebini REDDEDER.
RA, Intermediate auto-sign'a tek runtime gateway'dir.
```

---

## 5. Admin↔RA Transport

```
Transport:
  - server-to-server mTLS/gRPC.
  - SafeRoom custom Noise/KCP transport KULLANILMAZ (hot-path değil; "TLS kötü değil farklı problem").
  - Admin↔RA stabil datacenter/control-plane ağı varsayar.
  - RA↔Intermediate de RoftCore-iç izole mTLS/gRPC.

mTLS:
  - RA, RA Identity / RA server cert chain sunar.
  - Admin Server, Admin Server Cert sunar.
  - Her iki chain ROOT_CA_SPKI_HASH'e doğrulanır.
  - RA ek olarak cert_type == Admin Server + EKU id-saferoom-admin-server enforce eder.
```

RA kabul kuralı:

```
RA admin method'larını YALNIZCA şu koşullarda kabul eder:
  - mTLS peer cert geçerli Admin Server Cert.
  - cert_type == 0x0B Admin Server.
  - EKU id-saferoom-admin-server mevcut.
  - issuer chain = Admin Server Cert → per-org Intermediate → Root pin.
  - operator_id, request/operator scope ile eşleşir.
  - Admin Server cert subject_key_id, o operator_id için register edilmiş.
  - cert expired/revoked değil.
  - revocation state fresh (fail-closed).
```

---

## 6. Ortak AdminControlRequest Envelope

```
AdminControlRequest {
  magic             : bytes4      # "SRAC"
  version           : u8          # 1
  method_id         : u16_be
  request_id        : bytes16     # idempotency anahtarı
  operator_id       : bytes16
  admin_server_id   : bytes16
  admin_actor_id    : bytes32     # insan/admin/servis actor kimliği
  rbac_role         : u16_be
  sequence_number   : u64_be
  issued_at         : u64_be
  payload_len       : u32_be
  payload           : bytes[payload_len]
}

AdminControlResponse {
  magic             : bytes4      # "SRAR"
  version           : u8          # 1
  request_id        : bytes16
  status            : u16_be      # §16
  sequence_number   : u64_be
  payload_len       : u32_be
  payload           : bytes[payload_len]
}
```

Normatif kurallar:

```
- mTLS, Admin Server makine kimliğini authenticate eder.
- request_id idempotency anahtarıdır; aynı request_id orijinal işlendiyse aynı sonucu döner.
- sequence_number (operator_id, admin_server_id) başına monotondur; düşük sequence rollback → reject.
- issued_at clock-skew penceresi içinde olmalı.
- payload method-spesifiktir.
- RA her kabul edilen/reddedilen request'i append-only audit log'a yazar.
- Alpha: detached Admin request imzası OPSIYONEL.
- Beta: opsiyonel admin_request_signature non-repudiation için eklenebilir.
```

> **Açık uyarı:** İleride eklenirse, AdminControlRequest imzası bir canonical PKI/route/discovery/revocation object imzası DEĞİLDİR. Admin Server yine HubListRecord/AHS/DiscoveryEndpoint/RevocationRecord imzalamaz; yalnızca kendi control request envelope'unu imzalar.

---

## 7. Method ID Registry

```
Admin Server → RA (u16_be):
  0x0001 ADMIN_ALLOWLIST_SYNC
  0x0002 ADMIN_PREPARE_HUBLIST
  0x0003 ADMIN_PREPARE_AHS
  0x0004 ADMIN_PREPARE_DISCOVERY
  0x0005 ADMIN_SUSPEND_USER
  0x0006 ADMIN_DEACTIVATE_USER
  0x0007 ADMIN_REVOKE_DEVICE
  0x0008 ADMIN_EMERGENCY_REVOKE
  0x0009 ADMIN_SEAT_QUERY
  0x000A ADMIN_HEALTH
  0x000B ADMIN_ROTATE_ADMIN_SERVER_CERT
  0x000C ADMIN_REVOKE_ADMIN_SERVER
  0x000D ADMIN_ALLOWLIST_HUB          (KAT5: pubkey-bound hub enrollment allowlist)

RA → Admin Server callback/push (bit15=1):
  0x8001 RA_AUDIT_PUSH
  0x8002 RA_REVOCATION_ACK
  0x8003 RA_SEAT_STATUS
  0x8004 RA_HEALTH_ALERT
```

---

## 8. Method Şemaları + Normatif Davranış

### 8.1 ADMIN_ALLOWLIST_SYNC (0x0001)

```
ADMIN_ALLOWLIST_SYNC payload {
  allowlist_sequence : u64_be
  delta_count        : u32_be
  deltas             : AllowlistDelta[delta_count]
}

AllowlistDelta {
  action             : u8        # 0x01 add, 0x02 update, 0x03 remove
  user_ref_hash      : bytes32   # privacy-preserving kurumsal kullanıcı referansı
  phone_commitment   : bytes32   # kullanılıyorsa; canonical path'te plaintext telefon gerekmez
  device_policy      : u16_be
  expires_at         : u64_be
}
```

Davranış:

```
- Admin Server allowlist delta'sını RA'ya push eder.
- RA allowlist'i operator_id başına cache'ler.
- Gate-2 cache'lenmiş allowlist'i kullanır; RA her enrollment için senkron Admin çağrısı YAPMAZ.
- düşük allowlist_sequence → reject.
- stale allowlist: Alpha default = freshness politikayı aşarsa yeni enrollment FAIL-CLOSED.
- allowlist sync cert issuance DEĞİLDİR; yalnız Gate-2'yi besler.
```

### 8.2 ADMIN_PREPARE_HUBLIST (0x0002)

```
ADMIN_PREPARE_HUBLIST payload {
  draft_sequence     : u64_be
  hub_count          : u16_be
  hub_drafts         : HubEntryDraft[hub_count]
  not_after          : u64_be
}
```

Davranış:

```
Admin imzasız HubListDraft sunar.
RA doğrular: Admin Server scope, operator_id eşleşme, license_not_after clamp, hub ownership/SAN policy, seat/license/policy.
Sonra Intermediate HubListRecord imzalar.
Admin Server HubListRecord İMZALAMAZ.
```

### 8.2b ADMIN_ALLOWLIST_HUB (0x000D) — Hub Enrollment Allowlist (KAT5-DECISION-2)

> Admin, hub enrollment allowlist'ini **IP merkezli değil KEY merkezli** besler. Admin Server hub'ın Ed25519 public key fingerprint'ini RA'ya bildirir; RA, Hub Identity CSR geldiğinde CSR pubkey fingerprint bu allowlist ile eşleşmeden imzalama YAPMAZ (pki-ca-ra-hsm-tdd.md §5.6). Admin-provided IP/SAN authentication DEĞİL, policy constraint'tir.

```
ADMIN_ALLOWLIST_HUB payload {
  operator_id            : bytes16
  hub_id                 : bytes16
  hub_identity_spki_hash : bytes32     # SHA-256(spki(Ed25519 hub_identity_pub)) — kriptografik kimlik
  expected_san_count     : u8
  expected_sans          : SanEntry[expected_san_count]   # DNS/IP policy constraint (auth DEĞİL)
  hub_role               : u8
  hub_priority           : u8
  enrollment_not_before  : u64_be
  enrollment_not_after   : u64_be
  allowlist_sequence     : u64_be      # monoton; düşük → reject
}
```

Davranış:
```
- Admin Server hub pubkey fingerprint'ini RA'ya bildirir (HubEnrollmentAllowlistEntry).
- RA, Hub Identity CSR pubkey SPKI hash == hub_identity_spki_hash eşleşmeden cert issuance YAPMAZ.
- Admin-provided IP/SAN yalnız policy constraint (coarse pre-filter, expected-SAN); authentication factor DEĞİL.
- Source IP tek başına hub identity olarak KABUL EDİLMEZ.
- allowlist_sequence düşük → reject (rollback koruması).
```

**FORBIDDEN:**
```
- "Admin sadece IP allowlist yazar, RA o IP'den gelen CSR'ı imzalar"
- "source IP hub identity kabul edilir"
```

### 8.3 ADMIN_PREPARE_AHS (0x0003)

Alpha AHS granülaritesi (NORMATİF):

```
Alpha AHS granularity = org-wide AHS.
  Admin, org namespace'indeki tüm authorized hub'lar için AHS hazırlar.
  per-dest_hash / per-user / per-group AHS = Beta.

Tradeoff:
  org-wide AHS, authorized-olmayan hub route poisoning'i bloke eder.
  Aynı org içindeki authorized-ama-kötü hub'ın route suppress/delay etmesini TAM engellemez (R-5 ile aynı).
  E2E içerik gizliliği korunur.
```

```
ADMIN_PREPARE_AHS payload {
  ahs_sequence       : u64_be
  hub_count          : u16_be
  authorized_hubs    : HubAuthDraft[hub_count]
  not_after          : u64_be
}
```

Davranış:

```
Admin imzasız AHSDraft sunar.
RA hub ownership/operator scope doğrular.
Intermediate AuthorizedHubSetRecord (SRAH) imzalar.
Admin AHS İMZALAMAZ.
```

### 8.4 ADMIN_PREPARE_DISCOVERY (0x0004)

```
ADMIN_PREPARE_DISCOVERY payload {
  discovery_sequence              : u64_be
  endpoint_count                  : u8
  endpoints                       : DiscoveryEndpointDraft[endpoint_count]
  pk_oprf                         : bytes32
  voprf_suite_id                  : u16_be
  discovery_transport_x25519_pub  : bytes32
  transport_suite_id              : u16_be
  not_before                      : u64_be
  not_after                       : u64_be
}
```

Davranış:

```
Admin imzasız DiscoveryEndpointDraft sunar.
RA doğrular: operator_id scope, endpoint ownership, pk_oprf policy,
  voprf_suite_id desteklenen, discovery_transport_x25519_pub non-zero/geçerli X25519, transport_suite_id desteklenen.
Intermediate DiscoveryEndpointRecord imzalar.
Admin DiscoveryEndpointRecord İMZALAMAZ.
```

> Bu alanlar (pk_oprf + voprf_suite_id + discovery_transport_x25519_pub + transport_suite_id) ZORUNLU dahildir; eski "yalnız pk_oprf+endpoint" modeli eksiktir (ccr §21.7 ile hizalı: VOPRF key + E2E transport key ayrı imzalanır).

### 8.5 ADMIN_SUSPEND_USER (0x0005)

```
ADMIN_SUSPEND_USER payload {
  identity_hash      : bytes32
  reason_code        : u16_be
  expires_at         : u64_be      # 0 = cleared olana dek
}
```

Davranış:

```
RA Admin RBAC + operator scope doğrular.
RA RevocationRecord oluşturur:
  subject_type = IDENTITY_HASH
  action       = SUSPEND
  severity     = soft_suspend
  signer       = RA Identity
Sonra revocation service snapshot/delta checkpoint oluşturur (Intermediate).
Layer-1 push hub'lara gönderilir.
```

### 8.6 ADMIN_DEACTIVATE_USER (0x0006)

```
Hard user disable.
RA identity hard revoke/deactivation record oluşturur (action=REVOKE, severity=hard_revoke).
Identity altındaki mevcut DeviceCert'ler yeni trust kararları için kullanılamaz hale gelir.
```

### 8.7 ADMIN_REVOKE_DEVICE (0x0007)

```
ADMIN_REVOKE_DEVICE payload {
  device_cert_hash   : bytes32
  reason_code        : u16_be
}
```

Davranış:

```
RA RevocationRecord oluşturur:
  subject_type  = DEVICE_CERT_HASH
  action        = REVOKE
  severity      = hard_revoke
  primary match = device_cert_hash

CompactDeviceCert serial ASLA kullanılmaz (revocation-model §5).
```

### 8.8 ADMIN_EMERGENCY_REVOKE (0x0008)

```
ADMIN_EMERGENCY_REVOKE payload {
  subject_type       : u8
  subject_id_len     : u16_be
  subject_id         : bytes[subject_id_len]
  reason_code        : u16_be
}
```

Davranış:

```
Gerekli RBAC: emergency-revoke rolü VEYA M-of-N admin onay politikası.

RA doğrular: Admin Server cert, admin_actor_id RBAC, operator scope, subject scope.

RA RevocationRecord oluşturur:
  severity = emergency_revoke
  action   = REVOKE veya SUSPEND (subject/policy'ye göre)
  signer   = RA Identity

Sonra:
  Revocation Service snapshot/delta günceller (Intermediate checkpoint).
  RevocationPush hub mesh'e gönderilir.
  in-memory deny-cache ANINDA aktif → routing durur.
  RA_REVOCATION_ACK Admin'e döner (revocation_id + snapshot_seq).
```

### 8.9 ADMIN_SEAT_QUERY (0x0009)

```
ADMIN_SEAT_QUERY response {
  seat_limit         : u32_be
  used_seats         : u32_be
  pending_invites    : u32_be
  license_not_after  : u64_be
  entitlement_seq    : u64_be
}
```

```
RA seat otoritesidir. Admin seat kullanımını GÖRÜR, AŞAMAZ.
Gate-1 kontrolü CSR Intermediate'e ulaşmadan ÖNCE olur.
```

### 8.10 ADMIN_HEALTH (0x000A)

```
ADMIN_HEALTH response {
  ra_status               : u8
  intermediate_status     : u8
  revocation_snapshot_seq : u64_be
  revocation_valid_until  : u64_be
  entitlement_seq         : u64_be
}
```

### 8.11 ADMIN_ROTATE_ADMIN_SERVER_CERT (0x000B) / ADMIN_REVOKE_ADMIN_SERVER (0x000C)

```
ADMIN_ROTATE_ADMIN_SERVER_CERT:
  yeni Admin Server CSR → RA ceremony onayı → Intermediate yeni Admin Server Cert imzalar → RA registry günceller (overlap).

ADMIN_REVOKE_ADMIN_SERVER:  (NORMAL PATH — güvenilir Admin Server hâlâ varken)
  Admin Server cert compromise/decommission → RA RevocationRecord (subject_type=ADMIN_SERVER_CERT 0x0B,
    subject=Admin Server cert_hash) + RA registry'den düşürür.
  (Admin Server cert revocation fresh state ile fail-closed; revocation-model.md §9.5.1.)
```

> **NORMATİF — ADMIN_REVOKE_ADMIN_SERVER tek revoke path DEĞİLDİR.**
> Eğer güvenilir Admin Server **compromised/unavailable** ise (kurum admin-plane kontrolünü kaybettiyse):
> ```
> - kurum RoftCore Teknoloji A.Ş.'ye out-of-band emergency contact ile ulaşır;
> - revocation-model.md §9.5.2 break-glass path uygulanır;
> - RoftCore-authorized AdminServerCertRevocationDirective (SRAD) RA'ya gider (compromised Admin imzalamaz);
> - RA registered_admin_server entry'yi disable eder;
> - RA future Admin↔RA mTLS'i ve tüm AdminControlRequest'leri fail-closed reddeder;
> - opsiyonel OCSP-benzeri infra status (§9.5.4) bayat cache'te ek doğrulama sağlar.
> ```
> Compromised Admin Server kendini revoke etmesi için güvenilmez; bu yüzden break-glass otoritedir.

---

## 9. Record Hazırlama Sınırı — Draft vs Signed Object (NORMATİF)

```
Admin Server taslak (draft) hazırlar.
RA taslakları doğrular.
Intermediate canonical record'ları imzalar.

Admin-submitted draft'lar şunlar olmadan TRUSTED değildir:
  - Admin Server Cert doğrulandı.
  - Admin RBAC/scope doğrulandı.
  - RA Gate-1/Gate-2/policy kontrolleri geçti.
  - Intermediate canonical record'u imzaladı.

Admin Server ASLA imzalamaz:
  HubListRecord / AuthorizedHubSetRecord / DiscoveryEndpointRecord /
  Identity/User Cert / CompactDeviceCert / RevocationRecord / RevocationSnapshot/Delta.
```

---

## 10. RA 3-Gate Entegrasyonu

```
Gate-1 — license seat:
  RA OrgEntitlementRecord doğrular.
  RA seat_limit'i CSR Intermediate'e gitmeden ÖNCE kontrol eder.
  Limit aşılırsa: 0x0301 SEAT_LIMIT reject; CSR Intermediate'e HİÇ ulaşmaz.

Gate-2 — admin allowlist:
  RA, ADMIN_ALLOWLIST_SYNC'ten gelen cache'lenmiş allowlist'e bakar.
  Authorized değilse: reject.
  RA şüpheli kayıt denemesini Admin'e push edebilir.

Gate-3 — device approval / SIM-swap mitigation:
  RA, enrollment/device request için device approval state'ini doğrular.
  Yeni cihaz, SIM-swap şüphesi, cihaz değişimi veya yüksek riskli enrollment durumunda
  admin/device approval olmadan CompactDeviceCert issuance yapılmaz.
  Gate-3 enforcement pki-ca-ra-hsm-tdd.md §6 enrollment/device-approval akışında tanımlıdır.

Hiçbir gate TEK BAŞINA cert basamaz:
  License seat tek başına cert basamaz.
  Admin allowlist tek başına cert basamaz.
  Device approval tek başına cert basamaz.
ÜÇ gate de geçmeden CSR Intermediate'e ulaşmaz.
```

---

## 11. RA → Admin Callbacks

```
RA_AUDIT_PUSH {
  event_id           : bytes16
  event_type         : u16_be
  phone_or_commitment: bytes32      # veya encrypted field
  dest_hash          : bytes32
  identity_hash      : bytes32
  timestamp          : u64_be
  coarse_location    : optional/coarse
}

RA_REVOCATION_ACK {
  request_id         : bytes16
  revocation_id      : bytes32
  snapshot_seq       : u64_be
  push_sequence      : u64_be
  status             : u16_be
}

RA_SEAT_STATUS {
  seat_limit         : u32_be
  used_seats         : u32_be
  warning_level      : u8
}
```

> **Privacy notu:** RA_AUDIT_PUSH kurumsal audit + approval görünürlüğü içindir. Mesaj plaintext'i İÇERMEZ (E2E). Telefon/IP/konum verisini policy'ye göre minimize eder (denetlenebilirlik düzlemi; içerik değil "kim kayıtlı").

---

## 12. Admin Panel — Grants

```
Admin panel yapabilir:
  - kullanıcı davet (invite)
  - cihaz/kullanıcı enrollment onayı
  - kullanıcı suspend
  - kullanıcı deactivate
  - device_cert_hash ile cihaz revoke
  - emergency revoke (RBAC izin veriyorsa)
  - hub list draft yönetimi
  - org-wide AHS draft yönetimi
  - discovery endpoint draft yönetimi
  - license seat kullanımı görüntüleme
  - RA / revocation freshness durumu görüntüleme
  - append-only audit log görüntüleme
  - rol/grup/departman yönetimi (RBAC)
  - Alpha'da manuel provisioning
  - Beta'da SSO/LDAP/SCIM entegrasyon yönetimi
```

---

## 13. Admin Panel — Prohibitions (yapısal/kriptografik)

```
Admin panel YAPAMAZ:
  - mesaj plaintext okuma
  - E2E payload decrypt
  - kullanıcı/cihaz private key export
  - sk_device possession / enrollment path olmadan sessiz cihaz ekleme
  - RA Gate-1 license seat bypass
  - Intermediate CA'ya doğrudan erişim
  - Root/Intermediate trust policy değiştirme
  - crypto suite downgrade
  - revocation/audit log silme veya yeniden yazma
  - kendini Admin Server yetkisi yaratma
  - canonical SafeRoom record'larını doğrudan imzalama
  - Identity/User Cert doğrudan issue
  - CompactDeviceCert doğrudan issue
```

---

## 14. SSO / LDAP / SCIM — Alpha vs Beta

```
Alpha:
  - Admin login / basic auth
  - manuel kullanıcı/cihaz provisioning
  - allowlist sync
  - seat query
  - suspend/revoke
  - audit

Beta:
  - SCIM otomatik kullanıcı provisioning
  - LDAP/AD grup sync
  - SSO/SAML/OIDC kurumsal auth
  - otomatik joiner/mover/leaver workflow

NORMATİF:
  SSO/LDAP/SCIM RA Gate-1/Gate-2'yi ASLA bypass etmez.
  Yalnız Admin Server policy/allowlist/RBAC'ı besler.
```

---

## 15. RBAC

```
- Admin actor'ler kurum RBAC'ı ile yönetilir (admin_actor_id + rbac_role).
- Yüksek-etkili yetkiler ayrı roller gerektirir:
    emergency-revoke = ayrı emergency rolü (opsiyonel M-of-N admin onayı).
    Admin Server cert rotate/revoke = yüksek-ayrıcalık rolü.
- RA, request envelope'taki rbac_role'ü method scope'una karşı doğrular;
  yetersizse 0x0104 RBAC_DENIED.
- RBAC kurum tarafında uygulanır; RA ek olarak method-seviye scope enforce eder (defense-in-depth).
```

---

## 16. Error Codes

```
0x0000 OK
0x0001 ACCEPTED
0x0101 ADMIN_CERT_INVALID
0x0102 ADMIN_CERT_REVOKED
0x0103 ADMIN_CERT_NOT_REGISTERED
0x0104 RBAC_DENIED
0x0201 SEQUENCE_ROLLBACK
0x0202 REPLAY_REQUEST_ID
0x0301 SEAT_LIMIT
0x0302 LICENSE_EXPIRED
0x0401 DRAFT_INVALID
0x0402 UNSUPPORTED_SUITE
0x0501 REVOCATION_STALE
0x0502 SUBJECT_REVOKED
0x0F01 RA_UNAVAILABLE
0x0F02 INTERMEDIATE_UNAVAILABLE
```

---

## 17. Threat Model

```
| ID       | Tehdit                                      | Statü            | Mitigasyon |
|----------|---------------------------------------------|------------------|------------|
| R-ADM-1  | Rastgele sunucu admin iddia eder            | Closed           | Admin Server Cert cert_type 0x0B + EKU + RA registry + mTLS. |
| R-ADM-2  | Admin doğrudan Intermediate'e erişir        | Closed           | Intermediate yalnız RA-originated iç mTLS imza işi kabul eder. |
| R-ADM-3  | Admin license seat bypass eder              | Closed           | RA Gate-1 Intermediate'ten önce. |
| R-ADM-4  | Admin kötücül HubList/AHS hazırlar          | Mitigated        | RA operator scope + hub ownership + license clamp doğrular. |
| R-ADM-5  | Admin record'ları doğrudan imzalar          | Closed           | Admin yalnız draft; canonical imza Intermediate/RA. |
| R-ADM-6  | Admin sessiz cihaz ekler                    | Closed           | enrollment + sk_device possession + CLIENT_BIND. |
| R-ADM-7  | Stale allowlist erişim verir                | Closed/Policy    | allowlist sequence + freshness fail-closed. |
| R-ADM-8  | Emergency revoke suistimali                 | Mitigated        | RBAC emergency rolü, opsiyonel M-of-N, audit, RA doğrulama. |
| R-ADM-9  | Admin request replay                        | Closed           | request_id idempotency + sequence_number + issued_at penceresi. |
| R-ADM-10 | Admin içerik okur                           | Closed           | E2E; admin-plane plaintext görmez. |
| R-ADM-11 | Admin Server cert compromise                | Closed/Mitigated | Normal path: ADMIN_REVOKE_ADMIN_SERVER. Compromised/unavailable path: RoftCore break-glass SRAD directive + ADMIN_SERVER_CERT revocation + RA registry disable + fail-closed freshness (revocation-model.md §9.5). |
```

---

## 18. Self-Audit

```
| Kontrol                                                          | Statü | Bölüm |
|------------------------------------------------------------------|-------|-------|
| Admin Server identity certificate-based (self-declare değil)     | ✅    | §2    |
| Admin Server Cert cert_type 0x0B / EKU id-saferoom-admin-server  | ✅    | §3    |
| RA admin methodları yalnız validated Admin Server Cert'ten       | ✅    | §5    |
| Admin actor identity makine kimliğinden ayrı                     | ✅    | §2    |
| Admin→RA→Intermediate topoloji enforce                           | ✅    | §4    |
| Admin doğrudan Intermediate erişimi yasak                        | ✅    | §4    |
| Admin yalnız draft; canonical record imzalamaz                   | ✅    | §9    |
| RA Gate-1 license seat Intermediate'ten önce                     | ✅    | §10   |
| RA Gate-2 allowlist cache + sequence                             | ✅    | §8.1,§10 |
| RA Gate-3 device approval / SIM-swap mitigation enforce edilir   | ✅    | §10, pki §6 |
| ADMIN_PREPARE_DISCOVERY transport_x25519_pub + suite id'ler      | ✅    | §8.4  |
| Alpha AHS org-wide tradeoff belgelendi                           | ✅    | §8.3  |
| Emergency revoke RA-signed + Layer-1 PUSH                        | ✅    | §8.8  |
| Device revoke device_cert_hash (serial değil)                    | ✅    | §8.7  |
| SSO/LDAP/SCIM Beta; Alpha manuel provisioning                    | ✅    | §14   |
| Request envelope replay/idempotency tanımlı                      | ✅    | §6    |
| Admin Server cert rotate/revoke (0x000B/0x000C)                  | ✅    | §8.11 |
```

---

## 19. Acceptance Criteria

```
AC-ADM-1:  Geçerli Intermediate-imzalı Admin Server Cert (cert_type 0x0B, EKU id-saferoom-admin-server, mTLS)
           sunmayan hiçbir sunucu Admin Server olarak işlem görmez.

AC-ADM-2:  Admin Server Cert subject_key_id/admin_server_id RA'da o operator_id için register olmalı.

AC-ADM-3:  Admin Server Intermediate CA'ya doğrudan erişemez; Intermediate yalnız RA'dan imza işi kabul eder.

AC-ADM-4:  Admin-submitted HubList/AHS/DiscoveryEndpoint girdileri imzasız draft'tır, trusted record değil.

AC-ADM-5:  HubList/AHS/DiscoveryEndpoint canonical imzaları yalnız RA doğrulaması SONRASI Intermediate tarafından üretilir.

AC-ADM-6:  RevocationRecord/RevocationPush yalnız RA Identity ile imzalanır; Admin Server asla imzalamaz.

AC-ADM-7:  Emergency revoke → RevocationRecord + Snapshot/Delta update + RevocationPush + RA_REVOCATION_ACK.

AC-ADM-8:  RA Gate-1 license seat kontrolü CSR Intermediate'e ulaşmadan önce olur.

AC-ADM-9:  RA Gate-2 Admin Server'dan gelen sequenced/fresh allowlist kullanır.

AC-ADM-10: Admin panel E2E plaintext okuyamaz / private key export edemez.

AC-ADM-11: Device revoke primary handle = device_cert_hash.

AC-ADM-12: ADMIN_PREPARE_DISCOVERY pk_oprf + voprf_suite_id + discovery_transport_x25519_pub + transport_suite_id içerir.

AC-ADM-13: Alpha manuel provisioning kullanır; SCIM/LDAP/SSO otomatik provisioning Beta'dır.

AC-ADM-14: AdminControlRequest replay (request_id idempotency + sequence rollback) reddedilir.

AC-ADM-15: RA Gate-3 device approval / SIM-swap mitigation, device enrollment veya high-risk device change sırasında CSR Intermediate'e ulaşmadan ÖNCE enforce edilir (üç gate de geçmeden issuance yok).

AC-ADM-HUB-ENROLL-1: Admin hub enrollment allowlist'i hub_identity_spki_hash taşır (ADMIN_ALLOWLIST_HUB, §8.2b).

AC-ADM-HUB-ENROLL-2: RA, Hub Identity CSR public key fingerprint allowlist ile eşleşmeden cert issuance YAPMAZ.

AC-ADM-HUB-ENROLL-3: ADMIN_ALLOWLIST_HUB IP/SAN alanları yalnız policy constraint'tir; authentication factor DEĞİL.
```

---

## 20. Forbidden Wording / Cleanup

```
YASAK (active):
  - "Admin Server kendini admin register eder"
  - "Herhangi bir sunucu config ile admin olabilir"
  - "Admin Server HubListRecord imzalar"
  - "Admin Server AHS imzalar"
  - "Admin Server DiscoveryEndpointRecord imzalar"
  - "Admin Server RevocationRecord imzalar"
  - "Admin Server doğrudan Intermediate'e bağlanır"
  - "Admin approval license seat'i bypass eder"
  - "License seat admin approval'ı bypass eder"
  - "Device revoke by device serial"
  - "DiscoveryEndpointRecord yalnız pk_oprf+endpoint içerir"
  - "SSO/LDAP/SCIM RA gate'lerini bypass eder"
  - "Admin sadece IP allowlist yazar, RA o IP'den gelen CSR'ı imzalar"
  - "source IP hub identity kabul edilir"
  - "Hub Identity CSR public key fingerprint allowlist gerekmez"

İZİNLİ:
  - Admin Server Cert ile yetkilendirme
  - Admin draft → RA gate → Intermediate imza
  - emergency revoke Admin authorize, RA imza
  - device_cert_hash revocation
  - org-wide AHS (Alpha)
```

---

## 21. Final Verdict

```
Final Status:
  Admin Control Plane TDD Alpha için freeze-ready.

Frozen (kilitli):
  - Admin Server Cert-tabanlı identity (self-declare yasak)
  - cert_type 0x0B / EKU id-saferoom-admin-server cross-doc requirement
  - Admin↔RA mTLS/gRPC
  - Admin→RA→Intermediate topoloji
  - AdminControlRequest/Response envelope (SRAC/SRAR)
  - RA 3-gate entegrasyonu
  - draft → RA validation → Intermediate signing sınırı
  - emergency revoke → RA-signed RevocationRecord → Layer-1 PUSH
  - Alpha org-wide AHS
  - Alpha manuel provisioning / Beta SCIM-LDAP-SSO

Deferred (ertelenen):
  - detached Admin request signature (Beta non-repudiation)
  - otomatik SCIM/LDAP/SSO provisioning
  - per-user/per-dest AHS granülaritesi
  - M-of-N admin human approval workflow detayı

Cross-doc patches required:
  ccr-format-spec.md:
    - cert_type 0x0B Admin Server
    - EKU 0x0008 id-saferoom-admin-server
    - Admin Server CCR validation section (§17.6)
  pki-ca-ra-hsm-tdd.md:
    - Admin Server Cert ceremony / CSR / lifecycle bölümü
```

Bu belge yazıldıktan sonra `identity-and-trust-model.md`, `pki-ca-ra-hsm-tdd.md`, `ccr-format-spec.md`, `revocation-model.md` ve `application-protocol-and-rpc-spec.md` ile çelişmez. Temel invariant: Admin olunmaz, Admin Server Cert ile yetkilendirilir; Admin yalnız draft hazırlar, canonical imza Intermediate/RA'dadır; emergency revoke Admin authorize eder ama RA imzalar.
