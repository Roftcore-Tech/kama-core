# Kama — Revocation Model

**Durum:** Implementation-ready / freeze-ready Alpha.

**Alpha kararı:** Tam CRLite / filter-cascade implementasyonu Alpha'da zorunlu **değildir**. Alpha revocation engine = **signed snapshot + signed delta + push propagation + fail-closed freshness**. CRLite / filter-cascade compact encoding bir Beta optimizasyonudur ve aynı semantik korunarak swap-in edilebilir.

> **"CRLite" terminolojisi (NORMATİF):**
> Bu belgede "CRLite" bir **validation interface / freshness contract** anlamında kullanılır.
> Alpha wire/storage implementasyonu **signed revocation snapshot + signed delta**'dır.
> Filter-cascade / Bloom / CRLite-style compact probabilistic encoding Beta'da swap-in edilebilir.
> Semantik değişmez: revoke edilen subject **reject** edilir; fresh revocation state yoksa **unknown/never-validated** yeni trust kararı **fail-closed** olur (known contact §7.5 bounded-stale'e tabi; infra/control-plane fail-closed).

Bu belge `identity-and-trust-model.md` (semantik otorite), `ccr-format-spec.md` (byte-level otorite), `routing-rendezvous-architecture.md`, `discovery-architecture.md` ve `transport-layer-tdd.md` ile %100 uyumludur. Çelişki halinde semantik için identity, byte-level için ccr-spec esas alınır.

---

## 1. Üç Katmanlı Mimari Karar (NORMATİF)

Alpha revocation = **Signed Snapshot + Signed Delta + Push**. Üç katman birbirini tamamlar; hiçbiri tek başına yeterli değildir.

```
LAYER 1 — PUSH (anlık kesinti):
  RA / Revocation Service imzalı revocation event yayınlar.
  Hub'lar RevocationPush alır ve ANINDA enforce eder (in-memory deny-cache).
  Hedef cihaza/identity'ye routing/teslim anında durur (availability zaten hub kontrolünde).

LAYER 2 — KISA ÖMÜR (backstop):
  Device Cert       = 90 gün
  Identity/User Cert = 1 yıl
  Hub Noise Cert    = 1 yıl
  Hub Identity Cert = 3 yıl
  Expiry primary emergency revocation DEĞİL, yalnızca backstop'tur.
  Revoke edilen Device Cert en fazla kendi expiry'sine kadar yaşar.

LAYER 3 — CLIENT DOUBLE-CHECK (hub güvenilmez backstop):
  Client ve hub signed revocation state (snapshot + delta) tutar.
  Fresh revocation state yoksa: unknown / never-validated trust kararı FAIL-CLOSED olur.
  Daha önce fresh revocation state altında doğrulanmış known-contact kararları, §7.5 Known Contact
  Bounded-Stale Policy koşulları sağlanıyorsa ALLOW_WITH_STALE_REVOCATION_FLAG ile devam edebilir.
  Infra/control-plane trust kararları fail-closed kalır.
  Client hub'ın dürüstlüğüne tek başına güvenmez; kendi snapshot/delta'sını doğrular.
```

Birbirine bağımlılık:

```
PUSH tek başına güvenlik otoritesi DEĞİLDİR  → snapshot/delta ile reconcile edilir.
Snapshot/delta tek başına low-latency DEĞİLDİR → PUSH hızlı kesinti sağlar.
Kısa ömür tek başına yeterli DEĞİLDİR        → emergency için PUSH gerekir.
Üçü birlikte Alpha revocation modelidir.
```

---

## 2. Scope / Non-Goals

Bu belge şunları kapsar:

```
- Revocation subject types
- RevocationRecord / RevocationSnapshot / RevocationDelta / RevocationPush canonical format
- RevocationFetchRequest / response
- Matching semantics (CompactDeviceCert hash-based; serial yasağı)
- Fail-closed freshness policy
- Hub enforcement + Client double-check
- RA / Intermediate / Root trust rolü
- Layer-1 PUSH + snapshot/delta + kısa ömür entegrasyonu
- Anti-rollback sequence mantığı
- CompactDeviceCert / DiscoveryRecord / RoutingRecord / AHS entegrasyonu
```

Non-goals (kapsam dışı):

```
- CT Log encoding
- CRLite filter-cascade binary compression
- Root compromise ceremony tam detayı (Ops dokümanı)
- Legal / business policy
- UI / admin workflow detayı (admin-control-plane-tdd.md)
- P2P media protokolü
```

---

## 3. Trust Rolü ve İmzalama Otoritesi (NORMATİF)

İki katmanlı imza otoritesi (role separation):

```
RevocationRecord                      → RA Identity key ile imzalanır (operasyonel revocation event).
RevocationSnapshot / RevocationDelta  → Intermediate CA key ile imzalanır (checkpoint + anti-rollback otoritesi).
RevocationPush envelope               → RA Identity key ile imzalanır (low-latency taşıma).
INTERMEDIATE_CA_EMERGENCY_MARKER      → yalnızca Root-signed out-of-band event referansıdır (online push değil).
```

Doğrulama kuralı (kabul koşulu):

```
Bir RevocationRecord GEÇERLİ kabul edilir EĞER:
  (a) record üzerinde geçerli RA Identity imzası varsa (bağımsız doğrulanabilir, PUSH'ta relay edilebilir),
  VEYA
  (b) record, geçerli Intermediate-signed Snapshot/Delta altında yer alıyorsa
      (Alpha: inline records listesinde; Beta: records_merkle_root inclusion proof).

RA Identity key ve Intermediate CA key zinciri her zaman `ROOT_CA_SPKI_HASH` ile pinlenen Root CA'ya kadar
recursive verify_chain (RFC 5280 §6) ile doğrulanır (identity-and-trust-model.md). Root pub app bundle'dan yüklenir
ve `ROOT_CA_SPKI_HASH` ile doğrulanır; Root ağdan gelmez.
```

Gerekçe: her record RA tarafından bağımsız authentike edilir (PUSH'ta tek başına taşınabilir); aggregate state ise Intermediate checkpoint ile authentike + rollback-korumalı olur. Böylece RA operasyonel hızı verir, Intermediate denetim/anti-rollback omurgasını korur.

---

## 4. Revocation Subject Types

```
RevocationSubjectType (u8):

0x01  DEVICE_CERT_HASH
0x02  IDENTITY_HASH
0x03  USER_IDENTITY_CERT
0x04  HUB_IDENTITY_CERT
0x05  HUB_NOISE_CERT
0x06  RA_IDENTITY_CERT
0x07  RA_NOISE_CERT
0x08  AUTHORIZED_HUBSET_SCOPE
0x09  DISCOVERY_RECORD_SCOPE
0x0A  INTERMEDIATE_CA_EMERGENCY_MARKER
0x0B  ADMIN_SERVER_CERT
```

Normatif açıklamalar:

```
DEVICE_CERT_HASH (0x01):
  CompactDeviceCertV1 için PRIMARY matching handle.
  CompactDeviceCert serial TAŞIMAZ. Matching ASLA device_cert_serial ile yapılmaz.
  subject_id = device_cert_hash = SHA-256(canonical_compact_device_cert) (32B).

IDENTITY_HASH (0x02):
  Kullanıcı suspend / deactivate / revoke.
  subject_id = identity_hash = SHA-256(user_identity_ed25519_pub) (32B).
  Etki: o identity altındaki tüm yeni session/trust kararları bloke; dest_hash = SHA-256(identity_hash) türetilir.

USER_IDENTITY_CERT (0x03):
  Generic CCR temsili (Identity/User Cert) cert_serial veya canonical cert_hash ile revoke.
  Identity/User Cert invalid olursa altındaki CompactDeviceCert chain de geçersizdir.

HUB_IDENTITY_CERT (0x04):
  ANNOUNCE, RoutingRecord imza, Hub Noise issuer ve hub-mesh trust kararlarında reject.
  (C kararı: Hub cert revocation Layer-1 kapsamındadır — kısa ömür beklemek yetmez.)

HUB_NOISE_CERT (0x05):
  Hub-mesh Noise transport key revoke.
  ANNOUNCE / IKfallback / hub-to-hub channel setup sırasında reject; cached pk_noise purge.

RA_IDENTITY_CERT (0x06) / RA_NOISE_CERT (0x07):
  RA proxy / onboarding / admin-to-RA / discovery endpoint validation için.

AUTHORIZED_HUBSET_SCOPE (0x08):
  operator_id + dest_hash scope'unda AHS invalidation.
  DHT STORE için AHS yoksa RoutingRecord kabul edilmez; expired RoutingRecord fallback dahil
  route authorization kapanır (routing-rendezvous-architecture.md AHS-mandatory).

DISCOVERY_RECORD_SCOPE (0x09):
  DiscoveryRecord sequence/scope invalidation. Genelde device/identity revoke ile birlikte kullanılır.

INTERMEDIATE_CA_EMERGENCY_MARKER (0x0A):
  Normal online revocation path DEĞİLDİR.
  Compromise ceremony / root-signed emergency event / client trust update gerektirir.
  Bu belge yalnızca marker + fail-closed davranışı tanımlar; tam ceremony Beta/Ops dokümanına devredilir.
```

subject_id encoding tablosu:

```
subject_type                       subject_id encoding              uzunluk
DEVICE_CERT_HASH        (0x01)      device_cert_hash                 32B
IDENTITY_HASH           (0x02)      identity_hash                    32B
USER_IDENTITY_CERT      (0x03)      cert_hash (primary)              32B (serial = opsiyonel scope_hint)
HUB_IDENTITY_CERT       (0x04)      cert_hash (primary)              32B (serial = opsiyonel scope_hint)
HUB_NOISE_CERT          (0x05)      cert_hash (primary)              32B (serial = opsiyonel scope_hint)
RA_IDENTITY_CERT        (0x06)      cert_hash (primary)              32B (serial = opsiyonel scope_hint)
RA_NOISE_CERT           (0x07)      cert_hash (primary)              32B (serial = opsiyonel scope_hint)
AUTHORIZED_HUBSET_SCOPE (0x08)      dest_hash                        32B (operator_id ayrı alanda)
DISCOVERY_RECORD_SCOPE  (0x09)      identity_hash | dest_hash        32B
INTERMEDIATE_CA_EMERGENCY_MARKER (0x0A)  intermediate_key_id         16B
ADMIN_SERVER_CERT       (0x0B)      cert_hash (primary)              32B (scope_hint = opsiyonel serial/admin_server_id/operator_id)
```

> **ADMIN_SERVER_CERT (0x0B) normatif:** Admin Server Cert bir CompactDeviceCert DEĞİLDİR; **Generic CCR revocation** sınıfıdır. Primary revoke handle = `cert_hash = SHA-256(canonical_admin_server_ccr_tbs)` (ccr §17.6). `serial` yalnız audit/scope_hint olabilir, primary matching değil; `device_cert_hash` ve CompactDeviceCert semantiği KULLANILMAZ. (admin-control-plane-tdd.md ADMIN_REVOKE_ADMIN_SERVER + break-glass; pki §5.5.)

Generic CCR tipleri (Identity/Hub/RA) için **canonical primary handle = `cert_hash = SHA-256(canonical_ccr_tbs)`** (magic dahil). `serial` yalnızca opsiyonel `scope_hint` / audit referansıdır; matching primary'si değildir. CompactDeviceCert için bu seçim yoktur — daima device_cert_hash. (Freeze sadeleştirmesi: tek primary, parser/validator belirsizliği yok.)

---

## 5. Matching Semantics (EN KRİTİK BÖLÜM)

Eski X.509 serial kalıntıları CompactDeviceCert için **yasaktır**.

```
CompactDeviceCertV1 matching:
  primary        = device_cert_hash = SHA-256(canonical_compact_device_cert)
  scope_hint     = issuer_key_id (CRLite bucket / org scope)
  consistency    = identity_hash / current_device_id / dest_hash / DiscoveryRecord binding
  FORBIDDEN      = device_cert_serial

Generic CCR matching (Root / Intermediate / Identity / Hub / RA / CT / Admin Server):
  primary        = cert_hash = SHA-256(canonical_ccr_tbs)   (magic dahil; canonical primary)
  scope_hint     = serial (opsiyonel; audit/scope referansı, matching primary'si DEĞİL)

Admin Server Cert matching:
  subject_type   = ADMIN_SERVER_CERT (0x0B)
  primary        = cert_hash = SHA-256(canonical_admin_server_ccr_tbs)
  scope_hint     = opsiyonel serial / admin_server_id / operator_id
  effect         = Admin↔RA mTLS peer authorization fail-closed;
                   RA registered_admin_server entry disabled/removed;
                   o cert'ten gelen tüm gelecek admin method'ları reject.
  NOT            = device revocation DEĞİL; device_cert_hash KULLANMAZ; CompactDeviceCert semantiği KULLANMAZ.

Identity suspend matching:
  primary        = identity_hash
  derived        = dest_hash = SHA-256(identity_hash)
  effect         = new sessions + DiscoveryRecord trust + DeviceCert validation + RoutingRecord delivery fail-closed.

AHS matching:
  scope          = operator_id || dest_hash
  sequence_floor = scoped_sequence (opsiyonel; bu seq altındaki AHS/RoutingRecord reddedilir)
  effect         = RoutingRecord DHT STORE reject; cached expired fallback blocked.

DiscoveryRecord matching:
  scope          = identity_hash || current_device_cert_hash VEYA dest_hash
  effect         = cached PeerTrustCache entry invalidated.
```

> **MUTLAK KURAL:** CompactDeviceCertV1 serial içermez. CompactDeviceCertV1'i serial ile eşleştirmeye çalışan herhangi bir `RevocationRecord` veya `DeviceRevocationRequest` **geçersizdir ve REDDEDİLMELİDİR.**

`device_cert_hash`'in tanımı `ccr-format-spec.md` ile birebir aynıdır: `canonical_compact_device_cert` = 152B (pk_ed25519(32) || pk_x25519(32) || issuer_key_id(16) || not_after(8) || issuer_sig(64)). Hash = SHA-256 of bu 152B.

---

## 6. Canonical Object Formats

Tüm integer big-endian fixed-width. İmzalı yapılarda varint yoktur (ccr-spec primitive kuralları). `bytes[N]` sabit, `bytes[len]` len-prefixed.

### 6.1 RevocationRecord — magic `"SRRV"`

```
RevocationRecord {
  magic               : bytes4      # "SRRV"
  version             : u8          # 1
  subject_type        : u8          # §4
  operator_id         : bytes16
  subject_id_len      : u16_be      # ≤ 64
  subject_id          : bytes[subject_id_len]
  scope_hint_len      : u16_be      # ≤ 64
  scope_hint          : bytes[scope_hint_len]
  reason_code         : u16_be      # §6.1.1
  severity            : u8          # §6.1.2
  revocation_action   : u8          # §6.1.4 (REVOKE / SUSPEND / CLEAR)
  issued_at           : u64_be
  not_before          : u64_be
  not_after           : u64_be      # §6.1.3 (GC semantiği)
  sequence_number     : u64_be      # (operator_id, subject_type, subject_id) scope'unda monoton
  issuer_key_id       : bytes16     # RA Identity key_id
  signature           : bytes64     # Ed25519_Sign(sk_ra_identity, canonical_revocation_record_tbs)
}
```

Canonical TBS:

```
canonical_revocation_record_tbs =
  "SRRV" || version || subject_type || operator_id ||
  subject_id_len || subject_id ||
  scope_hint_len || scope_hint ||
  reason_code || severity ||
  revocation_action ||
  issued_at || not_before || not_after ||
  sequence_number || issuer_key_id

signature = Ed25519_Sign(sk_ra_identity, canonical_revocation_record_tbs)
```

#### 6.1.1 Reason codes (u16_be)

```
0x0001 device_lost
0x0002 device_compromised
0x0003 user_suspend
0x0004 user_deactivated
0x0005 license_expired
0x0006 admin_revoked
0x0007 hub_compromised
0x0008 key_rotation
0x0009 policy_violation
0x000A admin_server_compromised
0x000B admin_server_decommissioned
0x000C admin_server_suspended
0x000D break_glass_admin_revoke
0x00FF emergency
```

#### 6.1.2 Severity (u8)

```
0x01 soft_suspend     # geçici; un-suspend ile geri alınabilir (örn. user_suspend)
0x02 hard_revoke      # kalıcı; cert ömrü boyunca geçerli
0x03 emergency_revoke # hard_revoke + en yüksek propagasyon önceliği (PUSH zorunlu)
```

#### 6.1.3 not_after / GC semantiği (CANONICAL KARAR)

```
not_after, revocation record'unun ne zaman GC edilebileceğini belirler:

  hard_revoke / emergency_revoke (intrinsic expiry'li subject — Device/Identity/Hub cert):
    not_after = revoke edilen subject cert'in KENDİ not_after'ı.
    Gerekçe: Layer-2 gereği revoke edilen cert en fazla expiry'sine kadar yaşar; o tarihten sonra
    cert zaten geçersizdir, record GC edilebilir. (Snapshot boyutu sınırlı kalır.)

  soft_suspend (intrinsic expiry'siz / süresiz subject — identity suspend "lifted olana dek"):
    not_after = 0  → "superseded olana dek geçerli".
    Un-suspend, daha yüksek sequence_number'lı yeni bir record (severity güncellemesi) ile yapılır.

  AUTHORIZED_HUBSET_SCOPE / DISCOVERY_RECORD_SCOPE:
    not_after = ilgili AHS/DiscoveryRecord validity bound'u; yoksa 0 (superseded olana dek).

Doğrulayıcı not_after = 0'ı "indefinite, only superseded by higher-sequence record or snapshot rebuild" olarak yorumlar.
```

#### 6.1.4 revocation_action (u8) — REVOKE / SUSPEND / CLEAR (CANONICAL)

`severity` olayın ağırlığını, `revocation_action` ise olayın yönünü (negatif event mi, geri alma mı) belirler. Bu ayrım olmadan "un-suspend / clear" temsil edilemezdi.

```
revocation_action (u8):
  0x01 REVOKE    # kalıcı geçersizleştirme (hard_revoke/emergency ile birlikte)
  0x02 SUSPEND   # geçici askıya alma (soft_suspend ile birlikte)
  0x03 CLEAR     # önceki SUSPEND'i geri al (un-suspend)

Kurallar (NORMATİF):
  - hard_revoke / emergency_revoke (REVOKE) AYNI cert hash için CLEAR EDİLEMEZ (kalıcı).
  - soft identity SUSPEND, DAHA YÜKSEK sequence_number'lı bir CLEAR record ile geri alınabilir.
  - CLEAR (IDENTITY_HASH), yeni trust kararlarını ANCAK fresh snapshot/delta bunu doğruladıktan SONRA yeniden etkinleştirir.
  - CLEAR, expired cert'leri DİRİLTMEZ (Layer-2 kısa ömür bağımsız geçerlidir).
  - CLEAR record'u da aynı (operator_id, subject_type, subject_id) scope'unda monoton sequence_number taşır;
    en yüksek sequence kazanır (REVOKE/SUSPEND/CLEAR arası LWW). Eşit sequence + farklı action = kriptografik anomali → alert.
```

> Alpha sadeleştirmesi isteğe bağlıdır: bir operatör "Alpha'da un-suspend yok" derse, CLEAR'ı desteklemez ve soft_suspend de hard block gibi davranır. Belge CLEAR'ı tanımlar; etkinleştirme operatör policy'sine bağlıdır. Default Alpha: CLEAR yalnızca SUSPEND için tanımlı, REVOKE için yasak.


### 6.2 RevocationSnapshot — magic `"SRSN"`

```
RevocationSnapshot {
  magic               : bytes4      # "SRSN"
  version             : u8          # 1
  operator_id         : bytes16
  issuer_key_id       : bytes16     # Intermediate CA key_id (checkpoint otoritesi)
  snapshot_sequence   : u64_be
  valid_from          : u64_be
  valid_until         : u64_be
  generated_at        : u64_be
  record_count        : u32_be
  records             : RevocationRecord[record_count]   # Alpha: inline
  records_merkle_root : bytes32     # §6.2.1
  signature           : bytes64     # Ed25519_Sign(sk_intermediate, canonical_revocation_snapshot_tbs)
}
```

Canonical TBS:

```
canonical_revocation_snapshot_tbs =
  "SRSN" || version || operator_id || issuer_key_id ||
  snapshot_sequence || valid_from || valid_until || generated_at ||
  record_count || records_merkle_root
# NOT: records (inline gövde) TBS'e records_merkle_root ÜZERİNDEN bağlanır (records'ın kendisi TBS'te tekrar edilmez).
```

#### 6.2.1 records_merkle_root (CANONICAL KARAR)

```
leaf_i  = SHA-256(canonical_revocation_record_tbs_i)
root    = ikili Merkle ağacı, hash = SHA-256, soldan sağa, tek kalan düğüm kendisiyle eşlenir (RFC 6962 stili).
boş set (record_count = 0): records_merkle_root = 32 byte sıfır.

Alpha: records inline taşınır; doğrulayıcı leaf'lerden root'u yeniden hesaplar ve eşitliği kontrol eder.
Beta: records inline yerine compact CRLite/filter encoding ile gelebilir; root + inclusion proof semantiği korunur.
```

Normatif:

```
Snapshot, o anki TAM revocation state'i temsil eder.
Client/hub daha düşük snapshot_sequence kabul ETMEZ (anti-rollback).
valid_until geçmişse: unknown/never-validated yeni trust decision FAIL-CLOSED; known-contact bounded-stale §7.5'e tabi (revoked/expired/mismatch → reject).
records_merkle_root future compacting / inclusion proof için ayrılmıştır; Alpha'da hesaplanır ve doğrulanır.
```

### 6.3 RevocationDelta — magic `"SRDL"`

```
RevocationDelta {
  magic               : bytes4      # "SRDL"
  version             : u8          # 1
  operator_id         : bytes16
  issuer_key_id       : bytes16     # Intermediate CA key_id
  base_snapshot_seq   : u64_be
  delta_sequence      : u64_be
  valid_from          : u64_be
  valid_until         : u64_be
  generated_at        : u64_be
  record_count        : u32_be
  records             : RevocationRecord[record_count]
  signature           : bytes64     # Ed25519_Sign(sk_intermediate, canonical_revocation_delta_tbs)
}
```

Canonical TBS:

```
canonical_revocation_delta_tbs =
  "SRDL" || version || operator_id || issuer_key_id ||
  base_snapshot_seq || delta_sequence || valid_from || valid_until || generated_at ||
  record_count || SHA-256(concat(canonical_revocation_record_tbs_0..N-1))
```

Normatif:

```
Delta yalnız base_snapshot_seq üstüne uygulanır.
delta_sequence rollback → reject.
Delta gap (beklenen delta_sequence eksik) → full snapshot fetch ZORUNLU.
Delta expired (valid_until < now) → unknown/never-validated yeni trust decision FAIL-CLOSED; known-contact bounded-stale §7.5'e tabi.
```

### 6.4 RevocationPush — magic `"SRPU"`

```
RevocationPush {
  magic               : bytes4      # "SRPU"
  version             : u8          # 1
  operator_id         : bytes16
  issuer_key_id       : bytes16     # RA Identity key_id — push imzasını deterministik doğrular (key rotation / çoklu RA cert)
  push_sequence       : u64_be
  issued_at           : u64_be
  record_count        : u16_be
  records             : RevocationRecord[record_count]   # her biri RA-imzalı, bağımsız doğrulanabilir
  signature           : bytes64     # Ed25519_Sign(sk_ra_identity, canonical_revocation_push_tbs)
}
```

Canonical TBS:

```
canonical_revocation_push_tbs =
  "SRPU" || version || operator_id || issuer_key_id ||
  push_sequence || issued_at ||
  record_count || SHA-256(concat(canonical_revocation_record_tbs_0..N-1))
```

> `issuer_key_id` olmadan verifier, RA key rotation veya birden fazla RA Identity cert durumunda hangi public key ile doğrulayacağını bilemezdi. Bu alan push imza doğrulamasını açık ve deterministik kılar; key, `issuer_key_id` → RA Identity cert lookup ile bulunur ve `ROOT_CA_SPKI_HASH`'e kadar verify_chain ile doğrulanır.

Normatif:

```
PUSH low-latency propagation içindir.
PUSH alan hub/client her record'u BAĞIMSIZ doğrular (RA imzası) ve immediate deny-cache'e koyar.
PUSH daha sonra snapshot/delta ile reconcile edilir (kalıcı state).
push_sequence rollback → reject.
PUSH kaybolursa snapshot/delta fetch backstop'tur (PUSH güvenilir teslim garantisi vermez).
```

### 6.5 RevocationFetchRequest — magic `"SRFQ"` / RevocationFetchResponse — magic `"SRFR"`

Client (veya anti-entropy yapan hub), elindeki sequence durumunu bildirip eksik state'i çeker. Taşıma `REVOCATION_FETCH_SNAPSHOT` / `REVOCATION_FETCH_DELTA` / `REVOCATION_STATUS` client↔hub RPC'leridir (§10); bu yapı imzasızdır (taşınan Snapshot/Delta zaten Intermediate-imzalı, otorite onlardadır).

```
RevocationFetchRequest {
  magic                 : bytes4      # "SRFQ"
  version               : u8          # 1
  operator_id           : bytes16
  client_snapshot_seq   : u64_be      # elindeki en yüksek snapshot_sequence (yoksa 0)
  client_delta_seq      : u64_be      # elindeki en yüksek delta_sequence (yoksa 0)
  want_full_snapshot    : bool_u8     # 0x01 = delta gap/şüphe → full snapshot iste
}

RevocationFetchResponse {
  magic                 : bytes4      # "SRFR"
  version               : u8          # 1
  operator_id           : bytes16
  response_type         : u8          # 0x01 snapshot, 0x02 delta, 0x03 up_to_date
  snapshot_present       : bool_u8    # response_type=0x01 ise 1
  snapshot              : RevocationSnapshot      # snapshot_present=1 ise (Intermediate-imzalı)
  delta_count           : u16_be
  deltas                : RevocationDelta[delta_count]   # response_type=0x02 ise; her biri Intermediate-imzalı
}
```

Normatif:

```
Sunucu davranışı:
  client_snapshot_seq < current snapshot_seq VEYA want_full_snapshot=1 → response_type=0x01 (full snapshot).
  client snapshot güncel ama eksik delta var → response_type=0x02 (delta zinciri base_snapshot_seq üstüne).
  client tamamen güncel → response_type=0x03 (up_to_date; snapshot/deltas boş).

Client davranışı:
  alınan snapshot/delta imzasını Intermediate ile (ROOT_CA_SPKI_HASH'e kadar) doğrular.
  daha düşük sequence reddedilir (anti-rollback).
  delta gap tespit edilirse want_full_snapshot=1 ile yeniden ister.
  hiçbir source güncel state vermezse → unknown/never-validated yeni trust kararları FAIL-CLOSED (§7); known-contact bounded-stale §7.5'e tabi; infra/control-plane fail-closed.
```

### 6.6 AdminServerCertRevocationDirective — magic `"SRAD"` (break-glass infra control-plane)

Break-glass path'inde (§9.5), kurumun güvenilir Admin Server'ı compromise/unavailable olduğunda, RoftCore-iç CA/Revocation otoritesi RA'ya bu **imzalı directive**'i gönderir. Bu bir **altyapı control-plane** object'idir — client/hub hostile hot-path object'i DEĞİL; yalnız RoftCore-iç iyi-ağ control-plane'inde taşınır.

```
AdminServerCertRevocationDirective {
  magic                  : bytes4      # "SRAD"
  version                : u8          # 1
  operator_id            : bytes16
  admin_server_id        : bytes16
  admin_server_cert_hash : bytes32     # SHA-256(canonical_admin_server_ccr_tbs)
  action                 : u8          # 0x01 REVOKE, 0x02 SUSPEND
  reason_code            : u16_be
  issued_at              : u64_be
  sequence_number        : u64_be      # (operator_id, issuer_key_id) başına monoton
  issuer_key_id          : bytes16     # Intermediate CA (Alpha) veya RoftCore Emergency Revocation Authority (Beta)
  signature              : bytes64
}
```

Canonical TBS:

```
admin_server_revoke_directive_tbs =
  "SRAD" || version || operator_id || admin_server_id ||
  admin_server_cert_hash || action || reason_code ||
  issued_at || sequence_number || issuer_key_id

signature = Ed25519_Sign(sk_intermediate_or_authorized_emergency_authority, admin_server_revoke_directive_tbs)
```

İmza otorite politikası (NORMATİF):

```
Allowed directive signer:
  Option A (Alpha default): per-org Intermediate CA imzalar.
  Option B (Beta/ops): Root-signed RoftCore Emergency Revocation Authority imzalar.

Root CA bu runtime aksiyonu için ONLINE OLMAMALIDIR (offline kalır).

RA signer authorization doğrular:
  - signer chain → ROOT_CA_SPKI_HASH
  - signer o operator_id için authorized
  - issuer_key_id, çözülen signer cert ile eşleşir

Compromised Admin Server bu directive'i İMZALAYAMAZ (kendi kendini revoke edemez).
```

---

## 7. Freshness / Fail-Closed Policy (NORMATİF)

```
Yeni trust kararı için fresh revocation state ZORUNLUDUR.

"Yeni trust kararı" örnekleri:
  - yeni SESSION_INIT için CompactDeviceCert doğrulama
  - DiscoveryRecord sonucu doğrulama
  - yeni Client↔Hub trust kurma
  - ANNOUNCE kabul
  - RoutingRecord DHT STORE kabul
  - Hub Noise cert / IKfallback kabul
  - RA / Admin kanalı kabul
  - SIGNED_CONTROL sender cert context doğrulama

Revocation state bayat/erişilemez ise (İKİ SINIF — §7.5 Bounded-Stale Policy):
  - UNKNOWN / never-validated contact:
      yeni konuşma/session başlatma     = FAIL-CLOSED
      yeni cert kabulü                  = FAIL-CLOSED
  - KNOWN contact (§7.5 tanımı: daha önce FRESH revocation state altında doğrulanmış +
    cache entry hash-eşleşmeli):
      yeni SESSION_INIT                 = BOUNDED-STALE ALLOW (risk-flag'li, §7.5 matrix)
  - yeni RoutingRecord/AHS kabulü       = FAIL-CLOSED (her durumda; hub control-plane)
  - hâlihazırda kurulmuş SESSION_DATA   = policy izin verirse sınırlı grace ile devam edebilir;
    Alpha default (muhafazakâr): önceden kabul edilmiş ciphertext'i geriye dönük
    decrypt etme/düşürme YAPMA (zaten kurulmuş oturum bounded grace ile akmaya devam eder).
```

Staleness hedefleri:

```
Alpha freshness hedefi: max_staleness <= 24h

Önerilen operasyonel değerler (daha sıkı):
  hub revocation state    : <= 15 dk
  client revocation state : <= 24h
  emergency push          : saniye/dakika

if snapshot.valid_until < now            → state stale.
if sequence rollback                     → reject.
if delta gap                             → fetch snapshot.
if hiçbir source erişilemez              → unknown/never-validated yeni trust kararları FAIL-CLOSED;
                                           known-contact bounded-stale §7.5; infra/control-plane fail-closed.
```

---

### 7.5 Known Contact Revocation Cache / Bounded-Stale Revocation Policy (NORMATİF — KAT1-P1-01 Option C)

> **Amaç (availability/security tradeoff):** Hostile/blackout operasyonda (revocation refresh >24h imkânsız)
> Kama'nın TÜM yeni known-contact konuşma yeteneğini kaybetmesini önler; unknown contact fail-closed kalır.
> "Signal/WhatsApp'ın çalışamadığı yerde çalışır" değer önerisinin revocation güvenliğiyle dengelendiği nokta.

**"Known contact" tanımı (NORMATİF — telefon rehberi kaydı DEĞİL):**

```
Bir contact YALNIZCA şu koşullarla known sayılır:
  1. DiscoveryRecordV2 daha önce FRESH revocation state altında doğrulanmış.
  2. DeviceCertBundle / CompactDeviceCert daha önce FRESH revocation state altında doğrulanmış.
  3. KnownContactTrustCache entry şunları içeriyor:
       identity_hash, dest_hash,
       current_device_cert_hash,
       recipient_static_x25519_hash,
       discovery_record_hash,
       revocation_snapshot_sequence, revocation_snapshot_hash,
       validated_at, valid_until,
       operator_id / namespace context (varsa)
  4. Yeni trust kararındaki cert/device hash cached entry ile TAM eşleşiyor.
```

**Bounded-stale allow matrix (NORMATİF):**

```
fresh revocation VAR   + not revoked                                  → ALLOW (normal)
fresh revocation VAR   + revoked                                      → REJECT
fresh YOK + unknown contact                                           → REJECT (fail-closed)
fresh YOK + known + cached snapshot REVOKED diyor                     → REJECT
fresh YOK + known + cached NOT-revoked + cert not-expired + hash eş   → ALLOW_WITH_STALE_REVOCATION_FLAG
fresh YOK + known AMA yeni/rotated device_cert_hash / cache mismatch  → REJECT (fresh revocation gerekene dek)
```

**Function-level policy:**

```
validate_revocation_for_trust_decision(subject, context) -> allow | allow_with_stale_flag | reject
  context ∈ { FreshOnline, KnownContactBoundedStale, UnknownOffline }
```

**Bounded-stale ŞUNLARI BYPASS ETMEZ (hepsi yine zorunlu):**

```
- cert expiry (not_after)                    - CompactDeviceCert imza doğrulaması
- local cache'te revoked işareti             - chain-to-pinned-Root doğrulaması
- device_cert_hash tam eşleşmesi             - operator routing authorization (§ routing)
- DiscoveryRecordV2 imza doğrulaması
```

**Audit event (zorunlu):**

```
STALE_REVOCATION_TRUST_DECISION {
  dest_hash, device_cert_hash,
  last_snapshot_sequence, last_validated_at, age,
  decision = ALLOW_BOUNDED_STALE
}
```

**Ek kurallar:** Stale-allow kararı risk-flag/audit event üretir; ağ düzelince revocation refresh ZORUNLU
(ilk fırsatta); yeni/rotated device cert hash görülürse bounded-stale cache onu ASLA yetkilendiremez
(fresh revocation şart); unknown contact HER DURUMDA fail-closed.

**Bounded-stale invariant'ları (NORMATİF):**

```
INV-REV-NO-ABSOLUTE-STALE-ALLOW:
  Bounded-stale mode genel bir offline trust bypass'ı DEĞİLDİR. Yalnızca daha önce fresh-validate edilmiş,
  exact cached binding eşleşmesi olan known contact'lara uygulanır. Unknown contact, görülmemiş device cert,
  infrastructure cert ve control-plane authorization object'leri fresh revocation state olmadan fail-closed kalır.

INV-REV-REVOCATION-WINS:
  Bilinen herhangi bir revoked durum — fresh state'ten VEYA cached snapshot'tan — HER ZAMAN reject eder.
  Bounded-stale mode yalnızca "son bilinen not-revoked" known contact'lar için availability koruyabilir.
```

---

## 8. Enforcement Points

### 8.1 Client Enforcement

Client şu noktalarda revocation kontrolü yapar:

```
- DiscoveryRecord validation
- CompactDeviceCert validation
- DeviceCertBundle validation
- SESSION_INIT sender cert/ref resolution
- SIGNED_CONTROL sender context validation
- HubListRecord / DiscoveryEndpointRecord validation
- RA / Discovery / Hub transport endpoint cert validation
```

Client davranışı:

```
Revoked device_cert_hash:
  CompactDeviceCert reject.
  PeerTrustCache entry invalidate.
  o cihaza yeni SESSION_INIT gönderme.
  gelen INIT/DATA sender context revoked cert'e çözülüyorsa reject.

Revoked identity_hash:
  o identity altındaki tüm device cert'leri invalidate.
  o identity için DiscoveryRecord reject.
  yeni session bloke.

Revoked hub cert (identity/noise):
  ANNOUNCE / HubList / Hub Noise endpoint reject.
  varsa başka hub'a reconnect (multi-hub failover).

> **Cert-bound revocation (KAT5-P1-02):** HubEntryV3 artık full Hub Identity Cert + Hub Noise Cert taşıdığı için (ccr §21.5, raw key değil), revocation subject_id doğrudan **cert_hash** üzerinden işler. Revoked Hub Identity Cert veya Hub Noise Cert: (a) HubList/BootstrapBundle validation'da reject (validate_hub_entry §21.5.4 CRLite kontrolü), (b) ANNOUNCE validation'da reject (verify_announce fresh_crl, routing §3.4), (c) Client↔Hub Noise setup reject, (d) hub-mesh Noise setup reject. Aynı disiplin RA Identity/Noise Cert (RAServerEntry, §21.8) için de geçerlidir.
> **AC-REV-KAT5-1:** Revoked Hub Identity Cert veya Hub Noise Cert, HubEntryV3 bootstrap validation'da (§21.5.4) reject edilir.
> **AC-REV-KAT5-2:** Revoked Hub Identity Cert veya Hub Noise Cert, ANNOUNCE / hub-mesh validation'da (§3.4) reject edilir.

> **Client kendi provisioned hub bağlantısı — hostile carve-out (NORMATİF, KAT4-P2-02):** Client, KENDİ provisioned hub'larına (validated HubListRecord'daki HubEntry'ler) olan bağlantısını, hostile ortamda **fresh per-connection hub-revocation state fetch edemedi diye self-disconnect ETMEZ**. Hub revocation client-side per-connection CRLite ile DEĞİL, control-plane mekanizmalarıyla enforce edilir: (1) Layer-1 mesh push (revoked hub mesh'ten düşer, routing durur), (2) HubListRecord re-issue (sequence+1, revoked hub çıkarılır). Stale-cache'lenmiş revoked hub'a bağlanma riski yalnızca **suppression**'dır (E2E korunur — hub forge/decrypt yapamaz); bounded-rebind + AHS/priority failover (CAD-46) telafi eder. Bu carve-out yalnızca client'ın KENDİ altyapı-bağlantısı içindir; peer device cert revocation freshness §7.5'e (known-contact bounded-stale) tabidir.

Stale revocation state:
  unknown / never-validated contact             → FAIL-CLOSED
  known contact + exact cached binding match
    + cached snapshot not-revoked + cert valid  → ALLOW_WITH_STALE_REVOCATION_FLAG (§7.5)
  known contact ama new/rotated device_cert_hash
    / cache mismatch                            → FAIL-CLOSED (fresh revocation gerekene dek)
  cached snapshot revoked diyor                 → REJECT
  infra / control-plane certs                   → FAIL-CLOSED
```

### 8.2 Hub Enforcement

Hub şu noktalarda revocation kontrolü yapar:

```
- client registration / mailbox binding
- Client↔Hub Noise/identity validation
- RoutingRecord publish/store
- AuthorizedHubSet acceptance
- ANNOUNCE acceptance
- Hub↔Hub Noise
- FORWARD_MESSAGE delivery policy
- local mailbox delivery
```

Hub davranışı:

```
Revoked device:
  o cihazdan yeni mailbox binding kabul etme.
  o cihaza kuyruktaki mesajları teslim ETME.
  encrypted kuyruk mesajları policy TTL'e dek tutulabilir ama revoked cert'e RELEASE edilmez.

Revoked identity / suspend:
  dest_hash için routing/teslim dur.
  local mailbox binding invalidate/remove.
  yeni RoutingRecord publish ETME.
  hard_revoke ise mevcut RoutingRecord'u da reject.

Revoked hub identity/noise:
  k-bucket'tan evict.
  ANNOUNCE reject.
  o hub'ın RoutingRecord imzalarını reject.
  o hub ile hub-mesh Noise reject.
  cached pk_noise purge.

AHS invalidation:
  RoutingRecord DHT STORE reject.
  expired RoutingRecord fallback block.
```

---

## 9. Propagation Model

```
RA / Admin aksiyonu:
  Admin revoke/suspend/device-removal talep eder (admin-control-plane-tdd.md).
  RA admin yetkisini + license/policy'yi doğrular.
  RA RevocationRecord yayınlar (RA Identity imzalı).
  Revocation Service snapshot/delta sequence'ini günceller (Intermediate-imzalı checkpoint).
  Hub'lar RevocationPush alır.
  Client'lar hub-proxy üzerinden snapshot/delta fetch eder.

Layer-1 PUSH:
  RA / Revocation Service → hub'lar.
  Hub'lar ANINDA enforce eder.
  Hub'lar client'lara revocation availability'sini announce edebilir (fetch tetikler).

Client double-check:
  Client periyodik snapshot/delta fetch eder.
  Yeni trust kararından ÖNCE freshness kontrol eder.
  Client tek başına hub dürüstlüğüne güvenmez.

Anti-entropy:
  Hub'lar revocation sequence'lerini reconcile eder (hub-mesh SYNC backstop).
  Eksik delta → snapshot fetch.
```

Akış diyagramı:

```
Admin ──talep──▶ RA ──[Gate: admin authority + license]──▶ RevocationRecord (RA-signed)
                          │
                          ├──▶ Revocation Service: snapshot/delta seq++ (Intermediate-signed checkpoint)
                          │
                          ├──RevocationPush──▶ Hub mesh ──(in-memory deny-cache, ANINDA enforce)
                          │                         │
                          │                         └──SYNC anti-entropy──▶ diğer hub'lar
                          │
                          └── Client ──FETCH (hub-proxy)──▶ Snapshot/Delta ──(double-check, fail-closed)
```

---

## 9.5 Admin Server Cert Revocation (Normal + Break-Glass + OCSP-like + Enforcement)

Admin Server Cert (ccr §17.6, cert_type 0x0B) RA admin-plane mTLS peer kimliğidir. İki ayrı revoke path tanımlıdır: **normal** (güvenilir Admin Server hâlâ varken) ve **break-glass** (Admin Server compromise/unavailable).

### 9.5.1 Normal path — ADMIN_REVOKE_ADMIN_SERVER

Operatör için hâlâ geçerli/güvenilir bir Admin Server varken kullanılır. admin-control-plane-tdd.md §8.11 ADMIN_REVOKE_ADMIN_SERVER buna map edilir.

```
Input:
  operator_id
  target_admin_server_id
  target_admin_server_cert_hash = SHA-256(canonical_admin_server_ccr_tbs)
  reason_code

RA davranışı:
  1. Request gönderen Admin Server mTLS peer cert'i validated olmalı.
  2. Request gönderen Admin Server revoked OLMAMALI.
  3. admin_actor_id RBAC yetkisi doğrulanır.
  4. Target Admin Server cert_hash aynı operator_id altında registered olmalı.
  5. RevocationRecord oluşturulur:
       subject_type      = ADMIN_SERVER_CERT (0x0B)
       subject_id        = target_admin_server_cert_hash
       revocation_action = REVOKE
       severity          = hard_revoke veya emergency_revoke
       reason_code       = admin_server_compromised / admin_server_decommissioned / admin_revoked
       signer            = RA Identity
  6. RA registry'de target admin_server_id / subject_key_id DISABLED yapılır.
  7. RevocationPush yayılır.
  8. Snapshot/Delta checkpoint güncellenir.
  9. RA_REVOCATION_ACK Admin'e döner.
```

> Normal path, tek Admin Server compromise/unavailable ise YETERLİ DEĞİLDİR; o durumda break-glass path otoritedir (compromised Admin Server kendini revoke etmesi için güvenilmez).

### 9.5.2 Break-glass path — RoftCore-authorized Admin Server Cert revocation

```
Güvenilir Admin Server compromise/unavailable ise veya kurum admin-plane kontrolünü kaybettiyse,
kurum ADMIN_REVOKE_ADMIN_SERVER'a güvenemez.

Kurum, RoftCore Teknoloji A.Ş.'ye out-of-band emergency kanalla ulaşır:
  - sözleşmesel emergency contact
  - önceden register edilmiş yetkili kurum temsilcisi
  - imzalı kurumsal talep
  - ses/video doğrulama + ticket
  - M-of-N RoftCore operatör onayı
  - gereken yerde offline/legal/compliance süreci

Bu operasyonel doğrulama revocation wire format kapsamı DIŞINDADIR; ama RA'ya teknik handoff burada tanımlıdır
(§6.6 AdminServerCertRevocationDirective "SRAD").

NORMATİF: Break-glass path compromised/unavailable Admin Server için OTORİTEDİR.
ADMIN_REVOKE_ADMIN_SERVER tek revoke path DEĞİLDİR.
```

### 9.5.3 RA validation — AdminServerCertRevocationDirective

```
RA, directive'i uygulamadan ÖNCE doğrular:
  1. Exact parse; magic == "SRAD", version == 1, trailing byte yok.
  2. issuer_key_id çöz.
  3. signer chain → ROOT_CA_SPKI_HASH.
  4. signer bu operator_id için authorized (Alpha default: per-org Intermediate CA).
  5. Ed25519 imza, admin_server_revoke_directive_tbs üzerinde doğrulanır.
  6. issued_at freshness penceresi içinde.
  7. sequence_number (operator_id, issuer_key_id) başına monoton.
  8. admin_server_id, operator_id altında registered.
  9. admin_server_cert_hash, registered Admin Server Cert ile eşleşir.
  10. action ∈ {REVOKE, SUSPEND}.
  11. registry disable/suspend uygulanır.
  12. ADMIN_SERVER_CERT revocation kaydı yayınlanır/işlenir.
  13. RevocationPush + Snapshot/Delta pipeline tetiklenir.

Geçerli break-glass directive etkisi:
  - RA registered_admin_server entry'yi disable eder.
  - RA o cert'ten gelen gelecek Admin↔RA mTLS'i reddeder.
  - RA o cert'ten gelen tüm AdminControlRequest method'larını reddeder.
  - O Admin Server'a RA_AUDIT_PUSH callback'leri durur.
  - mTLS chain hâlâ verify etse bile RA o peer için fail-closed döner.
```

### 9.5.4 OCSP-like infrastructure status path

```
Admin Server, RA ve CA/Revocation servisleri iyi-ağ server-to-server control-plane'de çalıştığı için,
ALTYAPI cert'leri için OCSP-benzeri status check İZİNLİDİR.

Kapsam:
  - Admin Server Cert
  - RA Identity / RA Noise
  - server-to-server infra context'inde kullanılırsa Hub Identity / Hub Noise
  - hostile client hot-path için DEĞİL.

Admin Server Cert status response:
  target_cert_hash : bytes32
  status           : u8      # GOOD / REVOKED / SUSPENDED / UNKNOWN
  produced_at      : u64_be
  valid_until      : u64_be
  sequence_number  : u64_be
  issuer_key_id    : bytes16
  signature        : bytes64

NORMATİF:
  RA, local status cache bayatsa Admin↔RA mTLS kabul etmeden önce OCSP-benzeri status sorgulayabilir.
  status == REVOKED/SUSPENDED → registered_admin_server disable + Admin request fail-closed reject.
  status == UNKNOWN ve freshness gerekli → fail-closed.
  Bu OCSP-benzeri path YALNIZ RoftCore/Admin/RA altyapı control-plane'i içindir;
  hostile-path client/hub snapshot/delta/push revocation modelini DEĞİŞTİRMEZ.
```

### 9.5.5 RA / Admin-Control enforcement

```
RA, Admin↔RA admin-plane request kabul etmeden önce:
  1. Admin Server mTLS peer cert parse edilir.
  2. cert_type == 0x0B Admin Server.
  3. EKU id-kama-admin-server.
  4. chain: Admin Server Cert → per-org Intermediate → ROOT_CA_SPKI_HASH.
  5. cert_hash = SHA-256(canonical_admin_server_ccr_tbs) hesaplanır.
  6. Revocation state fresh olmalı.
  7. ADMIN_SERVER_CERT subject_id == cert_hash revoked ise:
       - mTLS peer authorization fail-closed.
       - AdminControlRequest reject.
       - RA registered_admin_server entry disabled/removed.
  8. subject_key_id/admin_server_id RA registry'de operator_id için registered.
  9. operator_id request scope ile eşleşir.
  10. Opsiyonel OCSP-benzeri status check yapılabilir/required olabilir.

Revoked Admin Server Cert etkisi — TÜM admin method'ları reddedilir:
  ADMIN_ALLOWLIST_SYNC / ADMIN_PREPARE_HUBLIST / ADMIN_PREPARE_AHS / ADMIN_PREPARE_DISCOVERY /
  ADMIN_EMERGENCY_REVOKE / ADMIN_REVOKE_DEVICE / ADMIN_ROTATE_ADMIN_SERVER_CERT (özel recovery path hariç) /
  ADMIN_REVOKE_ADMIN_SERVER → hepsi reject.
```

### 9.5.6 Semantic boundary — cascade invalidate YASAK (NORMATİF)

```
Admin Server Cert revocation YALNIZ gelecek admin-plane yetkisini etkiler.

OTOMATİK OLARAK geçersiz KILMAZ:
  - önceden Intermediate-imzalı HubListRecord
  - önceden Intermediate-imzalı AuthorizedHubSetRecord
  - önceden Intermediate-imzalı DiscoveryEndpointRecord
  - önceden RA-imzalı RevocationRecord
  - önceden Intermediate-checkpoint'li Snapshot/Delta

Bu object'ler kendi validity / sequence / freshness / revocation semantiğine tabidir.
Compromise HubList/AHS/DiscoveryEndpoint geçersizleştirmeyi gerektiriyorsa, o object'ler için AYRI
revocation/update record'ları çıkarılmalıdır (yanlış cascade-invalidate bug'ı engellenir).
```

---

## 10. App Protocol / Method Namespace Notu (NORMATİF)

Revocation, iki ayrı protokol düzleminden yararlanır. **Hub-mesh method_id tablosu client↔hub app protokolü için TEKRAR KULLANILMAZ.** Parser namespace'leri ayrıdır (threat model, auth boundary, rate-limit, parser state, logging tamamen farklı).

```
Hub↔Hub Mesh RPC (method_id u8, routing-rendezvous-architecture.md §7):
  0x01 ANNOUNCE
  0x02 FIND_NODE
  0x03 FIND_VALUE
  0x04 STORE
  0x05 FORWARD_MESSAGE
  0x06 SIGNALING_RELAY
  0x07 SYNC            ← revocation anti-entropy backstop bu kanalı kullanır
  0x08 PING

Client↔Hub App RPC (ayrı namespace; application-protocol-and-rpc-spec.md):
  REGISTER_OR_RA_PROXY
  STORE_MESSAGE
  FETCH_MAILBOX
  ACK_RANGE
  DELIVERY_RECEIPT
  DISCOVERY_PROXY
  HUBLIST_FETCH
  SESSION_INIT_REF_REBIND       ← route-miss recovery
  ── revocation-specific ──
  REVOCATION_FETCH_SNAPSHOT
  REVOCATION_FETCH_DELTA
  REVOCATION_STATUS
```

> Hub-mesh RevocationPush (RA → hub) hub-mesh transport (Noise) üzerinden taşınır; ayrı bir hub-mesh method_id (örn. SYNC genişlemesi veya yeni `REVOCATION_PUSH` method_id) `application-protocol-and-rpc-spec.md` + routing belgesinde finalize edilir. Bu belge wire object'i (SRPU) tanımlar; taşıma method_id'sini app-protocol belgesi atar.

---

## 11. Delivery Model Etkileşimi

```
Client delivery modeli (Session Model V3, transport-layer-tdd.md ile uyumlu):
  online    = uzun-ömürlü Client↔Hub Noise session; hub kuyruktaki mesajı server-initiated push eder
  reconnect = FETCH_MAILBOX (cursor ile)
  ACK_RANGE = kuyruğu temizler
  DELIVERY_RECEIPT = non-repudiation gerekiyorsa SIGNED_CONTROL (SSC1) olabilir

Revocation etkileşimi:
  hub, reconnect sonrası kuyruğu RELEASE etmeden ÖNCE revocation kontrol eder.
  revoked device mailbox fetch EDEMEZ.
  revoked identity yeni routed mesaj ALAMAZ.
  established SESSION_DATA hot-path her mesajda revocation kontrol ETMEZ (transport hot-path imzasız/AEAD-authenticated);
  kontrol yeni session/trust kararında ve cache refresh anında yapılır.
```

---

## 12. File Transfer Alpha/Beta Notu

```
File transfer revocation etkileşimi:
  Alpha PoC file transfer IMPLEMENTE ETMEZ.
  File-transfer RPC/chunking schema reserved + feature-flagged Beta.
  Etkinleşince: her chunk/bulk session start ve resume'da gönderen/alıcı revocation state'i doğrulanmalı.
  FEC decoder yalnız authenticated shard almalı (transport-layer-tdd.md; AEAD-verify FEC-decode'dan ÖNCE).
```

---

## 13. HSM / Issuer Notu

```
Alpha HSM:
  YubiHSM2 / doğrulanmış Ed25519-capable path Alpha hedefidir.
  RA Identity key, Intermediate CA key ve Root CA key bu HSM'de (PKCS#11) tutulur.
  documented ceremony + export policy + audit log + key labels + backup/recovery (pki-ca-ra-hsm-tdd.md).

Beta/Open:
  DİRAK HSM Ed25519/API desteği TEYİTSİZ → Beta/open-gap.
  Ed25519/API desteği doğrulanmadan "DİRAK Alpha destekliyor" İDDİA EDİLMEZ.
```

---

## 14. Threat Model

```
| ID       | Tehdit                                                    | Statü                  | Mitigasyon |
|----------|-----------------------------------------------------------|------------------------|------------|
| R-REV-1  | CompactDeviceCert serial mismatch / eski X.509 serial varsayımı | Closed          | CompactDeviceCert primary handle = device_cert_hash; serial yasak. |
| R-REV-2  | Hub revocation push'u suppress eder                       | Partially mitigated    | client double-check snapshot/delta; freshness fail-closed. |
| R-REV-3  | Stale snapshot replay                                     | Closed                 | sequence_number + valid_until + pinned Intermediate imzası. |
| R-REV-4  | Delta gap                                                 | Closed                 | full snapshot fetch zorunlu. |
| R-REV-5  | Discovery cache'te stale revoked device kalır            | Closed                 | DiscoveryRecord/CompactDeviceCert validation'da revocation check; PeerTrustCache invalidation. |
| R-REV-6  | Revoked hub ANNOUNCE'a devam eder                         | Closed (fresh state)   | ANNOUNCE validation hub cert revocation fail-closed (C kararı). |
| R-REV-7  | AHS expired fallback revocation'ı bypass eder             | Closed                 | expired RoutingRecord fallback geçerli AHS + fresh revocation ister. |
| R-REV-8  | RA compromise → kötücül revocation yayını                 | Tier-0 risk            | HSM, audit, separation of duties, append-only log, CT/audit, ops kontrolleri. |
| R-REV-9  | Intermediate compromise                                   | Emergency/out-of-band  | root-signed emergency ceremony, client trust update, operator rollover. |
| R-REV-10 | Revoked Admin Server admin-plane method göndermeye devam eder | Closed             | ADMIN_SERVER_CERT subject_type; RA mTLS peer cert_hash revocation check; registry disable; fail-closed freshness. |
| R-REV-11 | Admin Server Cert generic CCR path'e düşmez / parser bilmez | Closed               | explicit ADMIN_SERVER_CERT (0x0B) subject type + matching semantiği. |
| R-REV-12 | Admin Server compromise; normal ADMIN_REVOKE_ADMIN_SERVER güvenilmez | Closed       | RoftCore break-glass AdminServerCertRevocationDirective (SRAD) + opsiyonel OCSP-benzeri status. |
| R-REV-13 | Admin cert revoke geçmiş imzalı HubList/AHS'yi yanlışlıkla geçersiz sanılır | Closed | Admin yalnız draft; canonical record Intermediate-signed; cert revoke yalnız gelecek admin-plane yetkisini keser, mevcut object'ler kendi semantiğine tabi (§9.5.6). |
```

---

## 15. Self-Audit

```
| Kontrol                                                          | Statü | Bölüm |
|------------------------------------------------------------------|-------|-------|
| CompactDeviceCert revocation primary handle = device_cert_hash   | ✅    | §5    |
| CompactDeviceCert serial matching yasak                          | ✅    | §5    |
| Alpha CRLite = signed snapshot + delta + push                    | ✅    | §1,§6 |
| Filter-cascade Beta optimizasyonu                                | ✅    | §2    |
| Stale revocation'da yeni trust kararı fail-closed                | ✅    | §7    |
| Device revoke + identity suspend destekli                        | ✅    | §4    |
| Hub Identity / Hub Noise revoke destekli (C kararı)              | ✅    | §4    |
| AHS scope invalidation destekli                                  | ✅    | §4,§5 |
| Client double-check kötücül/suppress eden hub'a karşı korur      | ✅    | §9    |
| PUSH tek otorite değil; snapshot/delta backstop                  | ✅    | §6.4  |
| Sequence rollback reddedilir                                     | ✅    | §6.2,§6.3 |
| Delta gap full snapshot fetch zorlar                             | ✅    | §6.3  |
| Revoked device'da DiscoveryRecord/PeerTrustCache invalidate      | ✅    | §8.1  |
| RoutingRecord/AHS validation revocation fail-closed kullanır     | ✅    | §8.2  |
| İki katmanlı imza otoritesi (RA record / Intermediate checkpoint)| ✅    | §3    |
| not_after GC semantiği canonical                                 | ✅    | §6.1.3|
| revocation_action REVOKE/SUSPEND/CLEAR canonical                 | ✅    | §6.1.4|
| RevocationPush issuer_key_id ile deterministik imza doğrulama    | ✅    | §6.4  |
| RevocationFetchRequest/Response (SRFQ/SRFR) tanımlı              | ✅    | §6.5  |
| Generic CCR primary = cert_hash (serial = opsiyonel scope_hint)  | ✅    | §4,§5 |
| ROOT_CA_SPKI_HASH terim hizalaması (belgeler arası tutarlı)      | ✅    | §3    |
| ADMIN_SERVER_CERT (0x0B) revocation subject type tanımlı         | ✅    | §4,§5 |
| ADMIN_SERVER_CERT cert_hash = SHA-256(canonical_ccr_tbs) (serial/device_cert_hash değil) | ✅ | §5 |
| RA, Admin↔RA method'ları öncesi ADMIN_SERVER_CERT revocation kontrol eder | ✅ | §9.5.5 |
| Revoked Admin Server RA registered_admin_server entry'yi disable eder | ✅ | §9.5.1,§9.5.5 |
| Normal + Break-glass (SRAD) Admin Server revoke path'leri tanımlı  | ✅    | §9.5.1,§9.5.2 |
| AdminServerCertRevocationDirective ("SRAD") tanımlı + signer policy | ✅   | §6.6,§9.5.3 |
| OCSP-benzeri infra status yalnız iyi-ağ control-plane (hostile path değil) | ✅ | §9.5.4 |
| Admin Server revoke Intermediate-imzalı record'ları cascade-invalidate ETMEZ | ✅ | §9.5.6 |
| Hub-mesh method_id app-protokolde tekrar kullanılmaz             | ✅    | §10   |
| DİRAK Alpha için iddia edilmiyor                                 | ✅    | §13   |
```

---

## 16. Acceptance Criteria

```
AC-REV-1:  CompactDeviceCertV1 revocation matching yalnız device_cert_hash kullanır.
           Serial-based CompactDeviceCert revocation talebi reddedilir.

AC-REV-2:  Alpha revocation state signed snapshot + signed delta olarak temsil edilebilir.
           CRLite/filter-cascade Alpha implementasyonu için zorunlu değildir.

AC-REV-3:  Revocation state stale/erişilemez ise unknown/never-validated yeni trust kararları fail-closed olur.
           Known contact kararları yalnız §7.5 bounded-stale policy koşulları sağlanırsa
           ALLOW_WITH_STALE_REVOCATION_FLAG ile kabul edilebilir.
           Infra/control-plane trust kararları fail-closed kalır.

AC-REV-4:  RevocationPush anlık enforcement sağlar ama signed snapshot/delta ile reconcile edilir.

AC-REV-5:  Hub Identity ve Hub Noise cert revocation desteklenir ve ANNOUNCE/hub-mesh validation'da enforce edilir.

AC-REV-6:  Identity suspend yeni session, discovery trust, routing delivery ve mailbox fetch'i bloke eder.

AC-REV-7:  AHS invalidation RoutingRecord DHT STORE ve expired fallback'i engeller.

AC-REV-8:  Client PeerTrustCache, current_device_cert_hash'i revoke olan entry'leri invalidate eder.

AC-REV-9:  Delta sequence gap full snapshot fetch gerektirir.

AC-REV-10: DİRAK HSM, Ed25519/API desteği teyit edilmedikçe Beta/open-gap'tir.

AC-REV-11: RevocationRecord RA Identity ile, Snapshot/Delta Intermediate ile imzalanır; her imza
           recursive verify_chain ile `ROOT_CA_SPKI_HASH`'e doğrulanır.

AC-REV-12: not_after, intrinsic-expiry'li subject'lerde subject cert'in kendi not_after'ına eşittir;
           expiry'siz suspend'de 0 (superseded olana dek).

AC-REV-13: revocation_action REVOKE/SUSPEND/CLEAR taşır; CLEAR yalnız SUSPEND'i geri alır,
           hard_revoke/emergency REVOKE CLEAR edilemez, CLEAR expired cert'i diriltmez.

AC-REV-14: RevocationFetchRequest/Response (SRFQ/SRFR) client'ın eksik snapshot/delta'yı çekmesini sağlar;
           up_to_date / snapshot / delta yanıt tipleri ayrıdır; anti-rollback uygulanır.

AC-REV-15: RevocationPush issuer_key_id taşır; push imzası issuer_key_id → RA Identity cert lookup ile
           deterministik doğrulanır.

AC-REV-16: RevocationSubjectType ADMIN_SERVER_CERT (0x0B) içerir.

AC-REV-17: Admin Server Cert revocation subject_id = cert_hash = SHA-256(canonical_admin_server_ccr_tbs);
           serial primary matching handle DEĞİL, device_cert_hash KULLANILMAZ.

AC-REV-18: RA, revoked Admin Server Cert'ten gelen Admin↔RA admin-plane method'larını reddeder.

AC-REV-19: ADMIN_SERVER_CERT revocation kabul edilince RA registered_admin_server entry'yi disable/remove eder.

AC-REV-20: Admin Server Cert revocation yalnız gelecek admin-plane yetkisini etkiler;
           önceden Intermediate-imzalı canonical record'lar kendi validity/revocation semantiğine tabidir.

AC-REV-21: Güvenilir Admin Server compromise/unavailable ise RoftCore break-glass path,
           o Admin Server'a güvenmeden Admin Server Cert'i revoke edebilir.

AC-REV-22: AdminServerCertRevocationDirective per-org Intermediate CA veya Root-signed Emergency Revocation Authority
           tarafından imzalanır; compromised Admin Server tarafından DEĞİL.

AC-REV-23: OCSP-benzeri status yalnız altyapı control-plane içindir ve hostile-path snapshot/delta/push modelini değiştirmez.
```

---

## 17. Forbidden Wording / Cleanup

```
YASAK (active) ifadeler:
  - CompactDeviceCertV1 için device_cert_serial
  - CompactDeviceCert için serial-based device revocation
  - "CRLite cascade Alpha'da zorunlu"
  - "revocation freshness opsiyonel"
  - "hub cert revocation desteklenmiyor"
  - "client hub push'a double-check olmadan güvenir"
  - "DİRAK Alpha destekleniyor"
  - "Admin Server Cert revocation generic/undefined"
  - "Admin Server Cert device_cert_hash kullanır"
  - "Admin Server revoke by CompactDeviceCert serial"
  - "Revoked Admin Server hâlâ RA admin method çağırabilir"
  - "Compromised Admin Server kendini revoke etmeli"
  - "ADMIN_REVOKE_ADMIN_SERVER tek Admin Server revoke path'i"
  - "Admin Server Cert revoke önceki Intermediate-imzalı tüm record'ları otomatik geçersiz kılar"
  - "OCSP hostile-path revocation modelini değiştirir"

İZİNLİ:
  - Generic CCR serial revocation
  - CompactDeviceCert device_cert_hash revocation
  - Alpha signed snapshot/delta
  - CRLite/filter-cascade Beta
  - PUSH + client double-check
  - Hub cert revocation destekli (C kararı)
  - ADMIN_SERVER_CERT subject_type + cert_hash revocation
  - RA registry disable on Admin Server revoke
  - RoftCore break-glass AdminServerCertRevocationDirective (SRAD)
  - OCSP-benzeri infra status (Admin/RA/CA iyi-ağ control-plane)
  - mevcut Intermediate-imzalı record'lar ayrıca revoke/expire olmadıkça geçerli
```

---

## 18. Final Verdict

```
Final Status:
  Revocation Model Alpha için freeze-ready.

Frozen (kilitli):
  - CompactDeviceCert hash-based revocation (device_cert_hash primary, serial yasak)
  - Alpha signed snapshot + delta + push mimarisi
  - fresh revocation unavailable durumda:
    unknown/never-validated trust decisions fail-closed;
    known-contact decisions §7.5 bounded-stale policy'ye tabi;
    infra/control-plane fail-closed
  - device / identity / hub / RA / AHS / discovery subject types (C kararı: hub dahil)
  - iki katmanlı imza otoritesi (RA record / Intermediate checkpoint)
  - client double-check + hub enforcement points
  - canonical magic: SRRV (record) / SRSN (snapshot) / SRDL (delta) / SRPU (push) / SRFQ (fetch-req) / SRFR (fetch-resp) / SRAD (admin-server-revoke-directive)
  - ADMIN_SERVER_CERT (0x0B) revocation: normal + break-glass (SRAD) + OCSP-benzeri infra status
  - Admin Server cert_hash matching; cascade-invalidate yasağı (mevcut Intermediate-imzalı record'lar korunur)

Deferred (ertelenen):
  - CRLite/filter-cascade compact encoding (Beta)
  - root/intermediate emergency ceremony detayı (Ops dokümanı)
  - DİRAK HSM profili (Ed25519/API teyidi sonrası)
  - file transfer revocation semantiği (session-start/resume kontrolü ötesinde)
  - Root-signed Emergency Revocation Authority (SRAD Option B, Beta/ops)

Cross-doc consistency:
  - ccr-format-spec.md cert_type 0x0B Admin Server (§17.6) ile uyumlu.
  - pki-ca-ra-hsm-tdd.md Admin Server Cert ceremony/lifecycle (§5.5) ile uyumlu.
  - admin-control-plane-tdd.md ADMIN_REVOKE_ADMIN_SERVER + break-glass requirement ile uyumlu.
```

Bu belge yazıldıktan sonra `identity-and-trust-model.md`, `ccr-format-spec.md`, `routing-rendezvous-architecture.md`, `discovery-architecture.md` ve `transport-layer-tdd.md` ile çelişmez. CompactDeviceCertV1 için serial kalıntısı yoktur; primary revoke handle her yerde `device_cert_hash`'tir.
