# Kama — Routing & Rendezvous Architecture (Hub-Mesh DHT)

**Durum:** Implementation-ready / freeze-ready (Alpha) — yalnız hub-to-hub RPC payload byte-level test vektörleri bekliyor.
**Semantik otorite:** `identity-and-trust-model.md` v1.5 + `ccr-format-spec.md`
**Bu belge:** Hub-to-hub DHT, RoutingRecord, ANNOUNCE, hub-mesh Noise, senkron rendezvous (relay-only).

> Bu belge `identity-and-trust-model.md` (semantik otorite) ve `ccr-format-spec.md` (byte-level cert/record projeksiyonu) ile **birlikte** okunur. Sertifika formatları, verify_chain, dest_hash/routing_key türetimi, CompactDeviceCert ve DiscoveryRecordV2 oradan gelir; bu belge onların **üzerine** routing/rendezvous mekaniğini kurar. Hiçbir kimlik/PKI kararı burada yeniden tasarlanmaz.

---

## 0. Magic Registry Eklemesi

Bu belge üç canonical yapı ekler. Domain-separation disiplini (ccr-spec Section 1) aynen geçerli — her TBS ilk alan olarak kendi magic'ini içerir:

```
canonical_routing_record_tbs       magic "SRRR"   (RoutingRecord; bu belge Section 5)
canonical_announce_tbs             magic "SRAN"   (ANNOUNCE; bu belge Section 3)
canonical_authorized_hubset_tbs    magic "SRAH"   (AuthorizedHubSetRecord; bu belge Section 5.6)
```

Mevcut registry (ccr-spec): SRCR (CCR), SRDR (DiscoveryRecord), SRHL (HubListRecord), SROE (OrgEntitlementRecord), **SSI1/SSR1/SSD1/SSC1 (Session Model V3 — SESSION_INIT/SESSION_INIT_RESPONSE/SESSION_DATA/SIGNED_CONTROL)**. [SRAM + eski Async V2 SAI1/SAA1/SAD1/SAC1 DEPRECATED — identity v1.5 E-12/E-13, ccr-spec §35-DEPRECATED/V2.] Bu belge ekler: **SRRR, SRAN, SRAH**. Tüm magic'ler 4-byte ASCII, cross-protocol signature confusion'ı önler (bir yapının imzası başka yapı olarak doğrulanamaz). ccr-spec Section 6 magic tablosu SRRR/SRAN/SRAH'ı zaten rezerve eder (AC-19).

---

## 1. Scope ve Non-Goals

### 1.1 Bu Belgenin Sahibi (owns)

- Hub-to-hub DHT topolojisi (Kademlia, k=5)
- ANNOUNCE yapısı + doğrulama (recursive verify_chain)
- RoutingRecord byte formatı + yetki modeli
- Hub-mesh Noise transport (KK/IK/IKfallback, per-recipient)
- Hub-to-hub RPC envanteri (FIND_VALUE, STORE, vb.)
- Senkron rendezvous: Noise pattern + relay-only taşıma iskeleti (Alpha)

### 1.2 Bu Belgenin Sahibi DEĞİL

| Konu | Sahip belge |
|------|-------------|
| Cert formatları (Hub Identity/Noise CCR, CompactDeviceCert) | `ccr-format-spec.md` |
| verify_chain / verify_chain_to_root algoritması | `identity-and-trust-model.md` §15.2 |
| dest_hash / routing_key türetimi | `identity-and-trust-model.md` CAD-16 |
| DiscoveryRecordV2 (contact discovery) | `ccr-format-spec.md` §21 + `discovery-architecture.md` |
| VOPRF / telefon→dest_hash | `discovery-architecture.md` |
| Custom ARQ/FEC/AEAD wire transport | `transport-layer-tdd.md` |
| Session Model V3 payload formatı (SESSION_INIT/SESSION_INIT_RESPONSE/SESSION_DATA) | `ccr-format-spec.md` §35 |
| CRLite binary format | `revocation-model.md` |
| SDP/ICE derin signaling (Beta) | `signaling-architecture.md` (gelecek) |

### 1.3 Non-Goals (Alpha)

- **Client DHT katılımı YOK.** Client hiçbir zaman DHT node'u değildir; yalnızca kendi hub'ına RPC atar (Section 2).
- **Client RoutingRecord okumaz/görmez** (CAD-53). Konum bilgisi hub-mesh içinde kalır.
- **P2P media YOK.** Senkron çağrı dahil her şey relay (hub) üzerinden — NAT traversal sorunu yok (Section 8).
- **Derin SDP/ICE/TURN müzakere YOK (Alpha).** Senkron rendezvous Alpha'da yalnızca Noise pattern + relay-only iskelet; tam SDP/ICE Beta (`signaling-architecture.md`).
- **Cross-operator routing YOK (Alpha).** Operator A hub'ı Operator B dest_hash'ini resolve etmez (identity v1.5 cross-operator kararı; Beta Model B).

> **NORMATİF (operator isolation katmanı — KAT1-P2-02):** RoutingRecord / AuthorizedHubSet, operator-scoped routing authorization'ı enforce eder. **CompactDeviceCert'in chain-to-Root geçerliliği cross-operator authorization için YETERLİ DEĞİLDİR.** Başka bir operatörden gelen geçerli bir device cert kriptografik olarak valid'dir ama local operator namespace'inde routable/authorized DEĞİLDİR (discovery/routing policy izin vermedikçe). Alpha cross-operator routing kapalıdır. Client yine RoutingRecord veya AHS OKUMAZ (CAD-53).

### 1.4 Trust Boundary (NORMATİF — kilit çizgi)

```
AuthorizedHubSetRecord = RA/Intermediate-signed control-plane authorization.
                         (Admin Server hazırlar → RA gate → Intermediate imzalar; KULLANICI imzalamaz.)
RoutingRecord          = authorized Hub Identity-signed dynamic location record.
                         (sk_hub_identity; örtük Noise IK possession; client-signed YOK.)
Client                 = DHT / RoutingRecord / AuthorizedHubSet GÖRMEZ; hostile hot-path +0 byte.
```

> Bu üç satır routing yetki sınırının özüdür: **"kim nerede route edebilir"** kararı control-plane'de (RA/Intermediate), **"şu an fiilen neredeyim"** kaydı hub'da (Hub Identity), istemci ise yalnızca `dest_hash`'i kendi hub'ına verir ve routing topolojisini hiç görmez (CAD-53/58).

---

## 2. Mimari Konum — Client Sadece dest_hash Bilir

Kama dağıtık bir sistemdir ama **karmaşıklık client'tan tamamen gizlenir.** Client'ın routing dünyası tek cümledir: *"dest_hash'i kendi hub'ıma ver."*

```
KATMANLAR:

  Client (hostile link, 0.025 kbps, %80 kayıp)
     │  tek yük: kendi primary hub'ına 1 RPC round-trip
     │  (dest_hash + Session Model V3 envelope; hub_id BİLMEZ, DHT GÖRMEZ — CAD-53)
     ▼
  Hub Mesh (datacenter, düşük-gecikme/düşük-kayıp)
     │  DHT (Kademlia k=5), ANNOUNCE, RoutingRecord, hub-to-hub Noise
     │  "dest_hash → hangi hub" çözümü BURADA (client değil)
     ▼
  Hedef Hub → Bob (Bob'un kendi hostile linki)
```

**Kritik tasarım sonucu (hafıza #18):** İteratif Kademlia lookup'ın çok-RTT maliyeti **client'ı etkilemez**, çünkü lookup hostile link üzerinde değil — hub mesh içinde (datacenter). Client'ın hostile link üzerindeki tek yükü kendi hub'ına bir RPC'dir. Bu, "hostile ağda DHT çalışmaz" itirazını çözer: client zaten DHT'de değil.

### 2.1 Akış (Asenkron Mesaj)

İki ayrı durum vardır — **konuşma başlatma** (SESSION_INIT, control-plane) ve **kurulu akış** (established SESSION_DATA, raw data-plane). Bu ayrım `application-protocol-and-rpc-spec.md` §2 (3-düzlem) ve `transport-layer-tdd.md` ile birebir uyumludur.

```
DURUM A — Konuşma başlatma (SESSION_INIT, konuşma başına 1 kez):
  Alice → Hub A: STORE_MESSAGE { recipient_dest_hash, session_envelope=SESSION_INIT }   # App-RPC Control Plane (Plane 1)
     # Alice yalnızca dest_hash verir; Bob'un hub'ını bilmez (CAD-53)
  Hub A:
     1. routing_key = recipient_dest_hash[0:16]
     2. Bob'un hub'ını biliyorsa (cache) → direkt; bilmiyorsa:
        FIND_VALUE(routing_key) → DHT mesh → AuthorizedHubSet + RoutingRecordSet → Bob'un hub_id'si (Hub B)
     3. Hub B'nin pk_noise'ını cache'ten al (ANNOUNCE'tan) veya ANNOUNCE çek
     4. Hub A, Alice↔HubA legi için KENDİ local handle'ını atar:
        outbound_session_route[alice_hub_handle_A] = { Hub B, recipient_dest_hash }   (hub-A-local, CAD-57)
     5. Hub A → Hub B: FORWARD_MESSAGE { recipient_dest_hash, session_envelope }  (hub-mesh Noise KK, Section 6, Plane 3)
        # Alice↔HubA hub_handle TAŞINMAZ — yalnız L3 envelope + dest_hash gider
  Hub B:
     6. recipient_dest_hash ile Bob'un mailbox'ını bulur; L3 session_envelope'ı koyar (store-and-forward).
        Hub B KENDİ local binding'ini kurar; Hub A'nın hub_handle'ını global key gibi KULLANMAZ.
     7. Bob bağlandığında çeker (FETCH_MAILBOX). Online push gerekiyorsa Bob↔HubB legi için
        Hub B KENDİ local bob_hub_handle_B'sini atar (Bob'un hostile linki, kendi RPC'si).

DURUM B — Kurulu akış (established SESSION_DATA, HER mesaj, hot-path):
  Alice → Hub A: raw Plane 2 frame = alice_hub_handle_A(4B) + seq2 + ciphertext + tag16   # App-RPC YOK
     # recipient_dest_hash TAŞINMAZ; Hub A route'u KENDİ local alice_hub_handle_A ile bilir (DURUM A'da bind edildi)
     # Bu frame transport substrate'ten geçer: windowed/batched ACK + selective retransmit (ARQ)
     #   + on-demand FEC (loss yüksekse) + fuzzy rate (transport-layer-tdd.md) → reliability BURADA
  Hub A:
     1. alice_hub_handle_A → outbound_session_route[alice_hub_handle_A] = { Hub B, dest_hash } (cache lookup, DHT YOK)
     2. Hub A, Alice↔HubA L2 handle'ını CONSUME eder; L3 SESSION_DATA envelope'ı çıkarır: seq16 || ciphertext || tag16
     3. Hub A → Hub B: FORWARD_MESSAGE { recipient_dest_hash, session_envelope = SESSION_DATA_L3 }  (Plane 3)
        # Alice↔HubA hub_handle Hub B'ye GEÇMEZ
  Hub B:
     4. recipient_dest_hash / kendi local session binding'i ile Bob'a teslim (online push) veya mailbox (offline).
        Online push için Bob↔HubB legi Hub B'nin KENDİ bob_hub_handle_B'sini kullanır.
  # LINK_ROUTE_MISS (Hub A/B'de local route binding düştü) → ROUTE_MISS → client SESSION_REBIND → SESSION_INIT_REF (DURUM A)

DURUM C — Dönüş / yanıt (SESSION_INIT_RESPONSE ve non-hot-path SIGNED_CONTROL, konuşma başına ~1 kez):
  # SESSION_INIT_RESPONSE routing açısından "Alice'e giden bir mesaj"dır — ters yönde bir SESSION_INIT gibi.
  # Bob, aldığı SESSION_INIT'ten (E2E decrypt) Alice'in kimliğini/dest_hash'ini ZATEN bilir.
  Bob → Hub B: STORE_MESSAGE { recipient_dest_hash = Alice_dest_hash, session_envelope = SESSION_INIT_RESPONSE }  # Plane 1
  Hub B:
     1. routing_key = Alice_dest_hash[0:16]
     2. Hub A'yı ZATEN cache'te bilir (SESSION_INIT'i Hub A'dan FORWARD_MESSAGE ile aldı); değilse FIND_VALUE (DHT).
     3. DÖNÜŞ hot-path route'unu bind eder (SIMETRİK — Hub A'nın DURUM A'daki yaptığının aynısı):
        outbound_session_route[bob_hub_handle_B] = { Hub A, Alice_dest_hash }
     4. Hub B → Hub A: FORWARD_MESSAGE { recipient_dest_hash = Alice_dest_hash, session_envelope = SESSION_INIT_RESPONSE }
  Hub A:
     5. Alice_dest_hash ile Alice'in mailbox'ını / local session binding'ini bulur → Alice'e teslim.
        (Hub A, DURUM A'da Alice↔HubA legini zaten bind etmişti; yanıt o binding'e düşer.)
  # Bundan sonra her iki yön de established: Alice→Bob (alice_hub_handle_A) ve Bob→Alice (bob_hub_handle_B), her biri DURUM B hot-path.
```

> **NORMATİF — control-plane route correlation (P1):** `recipient_dest_hash`, **tüm control-plane STORE_MESSAGE tipleri** için anlamlıdır — SESSION_INIT, **SESSION_INIT_RESPONSE** ve non-hot-path **SIGNED_CONTROL** — her biri alıcısının dest_hash'i ile cache/DHT üzerinden route edilir (INIT ile simetrik). `recipient_dest_hash` YALNIZCA established SESSION_DATA hot-path'te (raw Plane 2) TAŞINMAZ; orada hub-local `hub_handle` kullanılır. SESSION_INIT_RESPONSE anında yanıtı alan hub (Hub B) **dönüş hot-path route'unu bind eder** (initiator hub'ın INIT'te yaptığının simetriği); böylece iki yön de bağımsız hub-local handle'larla established olur. `route_ref` alanı GEREKMEZ (Alpha).
>
> **Beta metadata-hardening (opsiyonel, ertelendi):** İstenirse Hub B, SESSION_INIT aldığında opak bir `reply_route_ref = Trunc128(HMAC(hub_b_secret, source_hub_id || mailbox_item_seq || SHA256(init_l3)))` üretip Bob'un mailbox item metadata'sına ekleyebilir; Bob yanıtta bu opak ref'i echo'lar ve Hub B, Alice_dest_hash'i yeniden ifşa etmeden / yeniden resolve etmeden Hub A'ya route eder. Bu, initiator dest_hash'inin yanıt yolunda tekrar görünmesini önler — ama Alpha'da gerekli değildir (INIT zaten dest_hash taşır; RESPONSE'un taşıması simetrik, kabul edilen federated metadata modeli). ref eksik/expired/replay/unknown → LINK_ROUTE_MISS.

> **NORMATİF (plane ayrımı):** Client→hub yolunda **yalnız SESSION_INIT/SESSION_INIT_RESPONSE/SIGNED_CONTROL** App-RPC `STORE_MESSAGE` (Plane 1) ile gider. **Established SESSION_DATA ASLA STORE_MESSAGE'a sarılmaz** — raw Plane 2 (hub_handle+seq+ct+tag) olarak transport substrate üstünden gider; reliability (ARQ/FEC/batched-ACK) substrate'tedir, App-RPC'de değil. Hub-mesh `FORWARD_MESSAGE` (Plane 3, datacenter low-loss) her envelope tipini taşıyabilir — Plane 1/2 ayrımı yalnız client↔hub hostile link içindir.

> **NORMATİF — hub_handle locality (P1):** `hub_handle` **hub-local L2 routing/demux state**'idir. ŞU DEĞİLDİR: global session identifier; cross-hub mailbox key; hub'lar arası taşınabilir; E2E Session Model V3 envelope'unun parçası. Alice↔HubA hub_handle ile Bob↔HubB hub_handle **ayrı, birbirinden bağımsız local handle'lar**dır. Hub A, Hub B'den kendi client-facing hub_handle'ını yorumlamasını İSTEYEMEZ; Hub B kendi local Bob delivery handle/mailbox binding'ini atar/tutar. Hub-mesh `FORWARD_MESSAGE` yalnız L3 envelope (+ recipient_dest_hash) taşır — sender-tarafı L2 hub_handle'ı taşımaz.

Alice tüm bu süreçte yalnızca `recipient_dest_hash` verdi. Hangi hub, kaç hop, DHT — hiçbirini görmedi.

### 2.2 Akış (Senkron Çağrı — Alpha relay-only)

```
Alice → Hub A: CALL_SETUP { recipient_dest_hash, call_offer (Noise handshake init) }
Hub A: FIND_VALUE(routing_key) → AuthorizedHubSet ile filtrele → Hub B (Bob'un authorized hub'ı)
Hub A → Hub B: SIGNALING_RELAY { call_offer }
Hub B → Bob: call_offer teslim (Bob online ise)
Bob → Hub B → Hub A → Alice: call_answer
   # Media RELAY üzerinden akar (Alice↔Hub A↔Hub B↔Bob); p2p YOK, NAT sorunu YOK
   # Alpha: yalnızca Noise pattern + relay path; derin SDP/ICE Beta (Section 8)
```

---

## 3. ANNOUNCE — Hub Ağa Katılım (Kendini Kanıtlar)

### 3.1 Prensip: Kimse Kimseyi Tanıtmaz

Bir hub ağa katılırken **kendini kanıtlar**; başka bir hub onu "tanıtmaz". Hub A'nın Hub C'yi DHT'ye "tanıtması" bir yetki devridir (Hub A merkezi güven noktası olur) — **yanlış**. Bunun yerine:

```
Hub C → DHT'ye ANNOUNCE gönderir.
Her hub BAĞIMSIZ olarak doğrular:
   verify_chain(hub_C.hub_identity_cert, ROOT_CA_SPKI_HASH)
Kimse kimseye tanıklık etmiyor — Root CA zaten cert imzasıyla tanıklık etti.
```

Bu, hub-mesh'i permissionless DHT'lerden ayırır: katılım açık değildir, **Intermediate-imzalı Hub Identity cert** gerektirir (kurumun PKI'sı). Ama merkezi bir "onaylayıcı hub" da yoktur — her hub kriptografik olarak bağımsız doğrular.

### 3.2 ANNOUNCE Wire Format

```
ANNOUNCE {
  magic               : bytes4      # "SRAN"
  version             : u8          # 1
  hub_id              : bytes16     # bu hub'ın kimliği (HubListRecord HubEntry.hub_id ile eşleşir)
  addr_count          : u8          # adres sayısı (1..N)
  addresses           : AddrEntry[addr_count]
  sequence_number     : u64_be      # MONOTONIC (hub her ANNOUNCE'ta artırır; replay/stale koruması)
  timestamp           : u64_be      # unix saniye (replay-window kontrolü)
  hub_identity_cert   : varbytes16  # CCR (Hub Identity, cert_type=0x05); len-prefixed
  hub_noise_cert      : varbytes16  # CCR (Hub Noise, cert_type=0x06); len-prefixed
  signature           : bytes64     # Ed25519_Sign(sk_hub_identity, canonical_announce_tbs)
}

AddrEntry {
  ip_version          : u8          # 0x04=IPv4, 0x06=IPv6
  ip_bytes            : bytes4 | bytes16   # ip_version'a göre
  port                : u16_be
}
```

> **Boyut notu:** ANNOUNCE hub-mesh içinde dolaşır (datacenter, bant sorunu yok) — client'a gitmez. Bu yüzden tam cert'ler (hub_identity_cert + hub_noise_cert) inline taşınabilir; CompactDeviceCert'teki 152B bant kaygısı burada geçerli değil.

### 3.3 canonical_announce_tbs

```
canonical_announce_tbs =
  "SRAN" (magic, 4B) || version || hub_id || addr_count ||
  (her AddrEntry: ip_version || ip_bytes || port) ||
  sequence_number || timestamp ||
  hub_identity_cert (varbytes16: len || bytes) ||
  hub_noise_cert (varbytes16: len || bytes)

signature = Ed25519_Sign(sk_hub_identity, canonical_announce_tbs)
```

`signature` TBS'e **dahil değildir**. Magic-in-TBS disiplini (ccr-spec Section 1) geçerli — implementer imzayı üretirken/doğrularken magic'i TBS'ten çıkarmamalı.

### 3.4 ANNOUNCE Doğrulama — RECURSIVE verify_chain (KRİTİK SIRA)

Doğrulama **recursive verify_chain** kullanır (identity v1.5 §15.2; RFC 5280 §6 path validation). İteratif "adım adım bağımsız kontrol" YANLIŞTIR — adımlar arası kriptografik bağ kurmaz. Root public key lokal app bundle'dadır ve `ROOT_CA_SPKI_HASH` ile pinlenir; **kök anahtar/pin asla ağdan (ANNOUNCE'tan) gelmez** (hash, Ed25519 verify key gibi KULLANILMAZ).

```
# ROOT_CA_SPKI_HASH bir HASH/PIN değeridir — Ed25519 public key DEĞİLDİR.
# İmza gerçek Root public key ile doğrulanır; pin yalnızca bu key'in doğru root olduğunu teyit eder.
verify_chain(cert, ROOT_CA_SPKI_HASH):              # identity v1.5 §15.2
  root_cert = load_root_cert_from_local_app_bundle()           # ASLA ağdan/ANNOUNCE'tan değil
  root_pub  = root_cert.public_key
  if SHA256(spki_encoding(root_cert.public_key_alg, root_pub)) != ROOT_CA_SPKI_HASH:
      return FALSE                                             # pin tutmuyorsa → reddet
  return verify_chain_to_root(cert, root_cert)

verify_chain_to_root(cert, root_cert):
  if cert.issuer_key_id == root_cert.subject_key_id:           # Root tarafından imzalı (anchor)
      return Ed25519_Verify(root_cert.public_key, cert.tbs, cert.signature)
  issuer_cert = find_issuer_cert(cert.issuer_key_id)
  if issuer_cert is None: return FALSE
  if verify_chain_to_root(issuer_cert, root_cert) == FALSE: return FALSE
  return Ed25519_Verify(issuer_cert.public_key, cert.tbs, cert.signature)
```
> **Root pin flow (NORMATİF):** (1) Root public key lokal app bundle/root_anchor'dan gelir. (2) `SHA256(spki_encoding(root_public_key)) == ROOT_CA_SPKI_HASH` kontrol edilir. (3) Pin geçerse Ed25519 verify **root_public_key** ile yapılır. (4) Hash/pin değeri ASLA Ed25519 public key gibi kullanılmaz. (5) ANNOUNCE veya ağ payload'ı trust anchor olarak root sağlayamaz.
```


verify_announce(ann, ROOT_CA_SPKI_HASH, fresh_crl, now):
  # 1. FORMAT: magic=="SRAN", version==1, parse exact; trailing byte → DROP
  if ann.magic != "SRAN" or ann.version != 1: DROP
  # 2. ZİNCİR ÖNCE — hub_identity_cert Root'a recursive doğrulanır (DoS guard: zincir geçmezse imzaya bakma)
  if verify_chain(ann.hub_identity_cert, ROOT_CA_SPKI_HASH) == FALSE: DROP
  # 3. ANNOUNCE imzası (ZİNCİR SONRASI) — doğrulanmış hub_identity_cert.public_key ile
  if NOT Ed25519_Verify(ann.hub_identity_cert.public_key, canonical_announce_tbs(ann), ann.signature): DROP
  # 3a. HubListRecord binding — ZORUNLU (hub_id self-asserted authority DEĞİL):
  hub_entry = current_hublist.find_by_hub_id(ann.hub_id)
  if hub_entry is None: DROP                                                  # keyspace'te yetkisiz hub_id
  # HubEntryV3 cert-bound binding (raw hub_ed25519_pub authoritative DEĞİL):
  expected_identity_cert = parse(hub_entry.hub_identity_cert)
  expected_noise_cert    = parse(hub_entry.hub_noise_cert)
  if expected_identity_cert.public_key     != ann.hub_identity_cert.public_key: DROP
  if expected_identity_cert.subject_key_id != ann.hub_identity_cert.subject_key_id: DROP
  # Hub Noise cert aynı Hub Identity altında olmalı (rotation/overlap policy'ye tabi):
  if ann.hub_noise_cert.issuer_key_id != ann.hub_identity_cert.subject_key_id: DROP
  if expected_noise_cert.issuer_key_id != expected_identity_cert.subject_key_id: DROP
  if ann.hub_noise_cert.public_key != expected_noise_cert.public_key:
      if NOT valid_hub_noise_rotation_overlap(ann.hub_noise_cert, expected_noise_cert, hub_entry, now): DROP
  if ann.hub_identity_cert.public_key_alg != Ed25519: DROP
  # 4. hub_noise_cert zinciri (recursive; hub_identity → Intermediate → Root)
  if verify_chain(ann.hub_noise_cert, ROOT_CA_SPKI_HASH) == FALSE: DROP
  # 4a. hub_noise_cert parent/type/usage binding (aynı Hub Identity altında, X25519 keyAgreement):
  if ann.hub_noise_cert.cert_type != HUB_NOISE_CERT: DROP
  if ann.hub_noise_cert.public_key_alg != X25519: DROP
  if ann.hub_noise_cert.key_usage != keyAgreement: DROP
  if ann.hub_noise_cert.issuer_key_id != ann.hub_identity_cert.subject_key_id: DROP
  # 5. EKU: hub_identity_cert EKU id-kama-hub içermeli
  if id-kama-hub NOT in ann.hub_identity_cert.eku: DROP
  # 6. SAN + ADDRESS EŞLEŞME: cert SAN'daki IP'ler ANNOUNCE addresses ile eşleşmeli (Section 3.5)
  if NOT san_matches_addresses(ann.hub_identity_cert.san, ann.addresses): DROP
  # 7. CRL: hub_identity_cert ve hub_noise_cert revoke değil (fail-closed)
  if fresh_crl.contains(ann.hub_identity_cert) or fresh_crl.contains(ann.hub_noise_cert): DROP
  # 8. VALIDITY: her iki cert now içinde geçerli
  if now > ann.hub_identity_cert.not_after or now > ann.hub_noise_cert.not_after: DROP
  # 9. TIMESTAMP replay-window: |now - ann.timestamp| <= 5 dakika
  if abs(now - ann.timestamp) > 300: DROP
  # 10. SEQUENCE: cached_seq varsa, ann.sequence_number > cached_seq olmalı (Section 3.6)
  if cached(ann.hub_id) and ann.sequence_number <= cached_seq(ann.hub_id): DROP   # stale/replay
  # KABUL:
  cache[ann.hub_id] = { pk_noise: ann.hub_noise_cert.spki,
                        seq_num: ann.sequence_number,
                        addresses: ann.addresses,
                        identity_fingerprint: SHA-256(ann.hub_identity_cert.spki) }
  kademlia_insert(ann.hub_id, ann.addresses)
```

> **NORMATİF (hub_id binding):** `ANNOUNCE.hub_id` self-asserted authority DEĞİLDİR. Bir hub yalnızca Intermediate-imzalı HubListRecord'da kendisine atanan `hub_id`'yi announce edebilir; `ANNOUNCE.hub_identity_cert.public_key` o hub_id için `HubListRecord`'daki `HubEntryV3.hub_identity_cert.public_key`'e (türetilen değer, raw `hub_ed25519_pub` DEĞİL) EŞİT olmalıdır. Aksi halde geçerli bir hub cert'i keyfi Kademlia node ID claim edip eclipse/routing saldırı yüzeyini büyütürdü. Bu binding Kademlia node identity'yi control-plane HubListRecord'a bağlar.

> **NORMATİF (hub_noise_cert binding):** Hub Noise Cert, AYNI ANNOUNCE içindeki Hub Identity Cert tarafından imzalanmış olmalı (`issuer_key_id == hub_identity_cert.subject_key_id`), X25519 + keyAgreement olmalı. Geçerli ama ilgisiz bir X25519 cert bu hub'ın transport key'i olarak ikame EDİLEMEZ.

> **Neden sıra kritik:** Zincir **önce** doğrulanır — geçmezse ANNOUNCE imzasına bakılmaz bile. Bu hem (a) self-signed sahte cert'i eler (saldırgan kendi keypair'iyle cert üretip ANNOUNCE imzalasa, verify_chain ROOT_CA_SPKI_HASH'ya ulaşamaz → DROP), hem (b) DoS guard'dır (geçersiz zincir için gereksiz imza doğrulaması yapılmaz). ROOT_CA_SPKI_HASH app bundle'dan gelir; ANNOUNCE'tan root_ca almak kritik bir bug olurdu (saldırgan kendi root'unu koyar).

### 3.5 SAN + Address Eşleşmesi (Zorunlu)

```
Hub C cert SAN: IP:1.2.3.4
ANNOUNCE addresses: ["5.6.7.8:7777"]
→ EŞLEŞMİYOR → DROP

Garanti: Geçerli bir hub'ın cert'ini çalıp farklı bir IP'de hub gibi davranmak imkansız.
Cert, IP'ye kriptografik olarak bağlı (Intermediate cert SAN'da IP'yi imzaladı).
```

`san_matches_addresses`: ANNOUNCE'taki her address'in IP'si, hub_identity_cert SAN'daki IP kümesinde olmalı (port SAN'da olmayabilir; IP eşleşmesi zorunlu). Eşleşmeyen tek address bile → tüm ANNOUNCE DROP (kısmi kabul yok).

### 3.6 sequence_number — Replay/Stale Koruması

```
Senaryo: Hub B IP1'den IP2'ye taşındı.
  Yeni ANNOUNCE: {IP2, seq=43, sig}
  Saldırgan eski ANNOUNCE'u kaydetti: {IP1, seq=42, sig} (replay-window içinde olsa bile)
  Saldırgan eski ANNOUNCE'u yeniden yayar.

Koruma: cached_seq(Hub B) == 43 → gelen seq=42 <= 43 → DROP (stale).
  Eski IP1 cache'lenmez; Hub B'ye IP2'den ulaşılır.

Kural: ANNOUNCE her gönderimde sequence_number'ı MONOTONIC artırır.
  Yeni > cached → kabul + cache güncelle. Yeni <= cached → DROP.
```

### 3.7 Geçersiz Cert — Sessizce Reddet, IP BAN YOK

```
Geçersiz/sahte ANNOUNCE → sessizce DROP + log. IP BAN YOK.

Neden IP ban yok (kritik):
  Saldırgan, meşru Hub A'nın IP'sini spoof edip kasıtlı geçersiz cert gönderebilir.
  "Bu IP'den geçersiz geldi → ban" olsaydı → meşru Hub A banlanır (saldırgan hedefi).
  IP ban olmadan: saldırgan geçerli cert üretemez (Intermediate sk yok) →
    sahte ANNOUNCE hiçbir etki yapmaz (DROP) → saldırganın kazancı sıfır.
  Meşru hub geçerli cert ile tekrar dener → kabul.
```

---

### 3.8 HubListRecord ↔ ANNOUNCE — Aynı Cert-Bound Hub Identity Modeli (KAT5-P2-03)

Client-facing `HubEntryV3` / HubListRecord (ccr §21.5) ve hub-mesh ANNOUNCE **aynı cert-bound hub identity modelini** kullanır:

```
Hub Identity Cert (cert_type 0x05):
  - Ed25519; hub_id / SAN / EKU id-kama-hub binding
  - Intermediate → Root pin chain
Hub Noise Cert (cert_type 0x06):
  - X25519 keyAgreement; issuer = Hub Identity Cert
  - Hub Identity public key ile doğrulanır
```

Client-facing bootstrap `hub_noise_x25519_pub`'ı **Hub Noise Cert'ten** çıkarır (ccr §21.5.4); hub-mesh ANNOUNCE **aynı Hub Noise Cert'i** taşır (§3.2). Bu nedenle client-facing key ile mesh-facing key arasında **iki ayrı authoritative temsil YOKTUR** (raw-key divergence eliminated).

**Cross-check (HubEntryV3 full cert taşıdığı için yeterli):**
```
- ANNOUNCE.hub_identity_cert, current HubListRecord'daki ilgili HubEntryV3.hub_identity_cert ile
  aynı public_key / subject_key_id altında olmalı (veya geçerli overlap/rollover policy'ye uymalı).
- ANNOUNCE.hub_noise_cert, HubEntryV3.hub_noise_cert ile aynı issuer (Hub Identity) altında
  geçerli olmalı (veya geçerli overlap/rotation policy).
```
Not: §3.4 `verify_announce` artık HubEntryV3 cert-bound modelini kullanır:
  - `HubEntryV3.hub_identity_cert.public_key`, `ANNOUNCE.hub_identity_cert.public_key` ile eşleşir veya geçerli rollover policy'ye uyar.
  - `HubEntryV3.hub_noise_cert` ve `ANNOUNCE.hub_noise_cert` aynı Hub Identity altında olmalıdır veya geçerli rotation/overlap policy'ye uyar.
Raw `hub_ed25519_pub` / raw `hub_noise_x25519_pub` alanları authoritative DEĞİLDİR.

**Hub Noise rotation:**
```
- yeni Hub Noise Cert AYNI Hub Identity Cert altında imzalanır
- HubListRecord sequence+1 ile re-issue (ccr §21.5), ANNOUNCE sequence_number artar
- eski/yeni cert overlap grace izinli (in-flight KK kırılmasın)
- client en güncel validated HubListRecord'u kullanır
- stale client failover eder veya yeni HubListRecord fetch eder;
  UNAUTHENTICATED in-band key replacement ASLA kabul edilmez
```

**Acceptance Criteria:**
```
AC-RR-KAT5-1: ANNOUNCE ve HubListRecord aynı cert-bound Hub Identity / Hub Noise modelini kullanır.
AC-RR-KAT5-2: verify_announce, HubEntryV3.hub_identity_cert ve ANNOUNCE.hub_identity_cert binding'ini doğrular.
AC-RR-KAT5-3: verify_announce, HubEntryV3.hub_noise_cert ve ANNOUNCE.hub_noise_cert binding/rotation policy'sini doğrular.
AC-RR-KAT5-4: Raw hub_ed25519_pub / raw hub_noise_x25519_pub authoritative field olarak kullanılmaz.
```

## 4. Kademlia DHT — Hub Mesh

### 4.1 Topoloji

```
- Node = hub (yalnızca hub'lar; client ASLA node değil).
- Node ID = hub_id (bytes16, 128-bit; HubListRecord HubEntry.hub_id ile aynı).
- Distance = XOR(hub_id_a, hub_id_b) (standart Kademlia metriği).
- k = 5 (replikasyon faktörü; RoutingRecord 5 en-yakın node'da tutulur).
- Anahtar uzayı = routing_key (dest_hash[0:16], 128-bit) — RoutingRecord anahtarı.
```

> **NORMATİF — hub_id generation (P2, DHT kalitesi + eclipse direnci):** `hub_id` **uniform rastgele 128-bit** olmalı (CSPRNG) VEYA RA/Intermediate tarafından PRF ile atanmış olmalı. **Sequential / IP-türevi / org-prefix-türevi hub_id YASAK** — aksi halde saldırgan hedef `routing_key` etrafında hub_id kümelemesi yapıp (adaptif) bir dest_hash'in k=5 en-yakın node kümesini ele geçirebilir (eclipse/route-poisoning yüzeyi). hub_id, HubListRecord HubEntry.hub_id ile eşleşir ve Hub Identity Cert'e binding'lidir (ANNOUNCE doğrulaması); dolayısıyla saldırgan hub_id'sini serbestçe seçse bile geçerli cert olmadan DHT'ye giremez, ama yine de hub_id dağılımının uniform olması k-bucket dengesi ve eclipse direnci için gereklidir.

### 4.2 Kademlia State Machine

```
OFFLINE → BOOTSTRAPPING → JOINED → DEGRADED → REJOINING

OFFLINE:       hub başlatılmadı / mesh bağlantısı yok
BOOTSTRAPPING: bilinen seed hub'lara ANNOUNCE + bucket doldurma (FIND_NODE)
JOINED:        k-bucket'lar dolu, RoutingRecord STORE/FIND_VALUE aktif
DEGRADED:      bazı bucket'lar zayıf (hub'lar offline); çalışır ama replikasyon eksik
REJOINING:     bağlantı koptu, yeniden ANNOUNCE + bucket onarımı
```

Seed hub'lar: kurumun Admin Server'ı tarafından sağlanan başlangıç hub listesi (HubListRecord'daki ilk birkaç hub bootstrap seed olarak kullanılabilir). Tek bir seed yeterli (Kademlia oradan mesh'i keşfeder).

### 4.3 k-Bucket ve Eviction

```
- Her hub, mesafe bucket'larında (i-th bucket: 2^i..2^(i+1) mesafe) en fazla k=5 node tutar.
- Yeni node: bucket doluysa, en eski node ping'lenir; yanıt verirse yeni node atılır
  (Kademlia "en eski güvenilir node'u tut" politikası — Sybil/eclipse direnci).
- Hub offline → ping fail → bucket'tan düşer → o hub'a ait cache (pk_noise, seq) temizlenir.
```

### 4.4 Anti-Entropy SYNC

```
Anti-entropy SYNC — SCOPED LWW (CAD-58; global "en yüksek seq kazanır" YANLIŞ):

RoutingRecord reconciliation:
  - LWW YALNIZCA aynı scope içinde uygulanır: (operator_id, dest_hash, hub_id).
  - Aynı scope'ta daha yüksek sequence_number kazanır (per-hub rollback koruması).
  - FARKLI hub_id'lere ait RoutingRecord'lar birbirini EZMEZ → RoutingRecordSet içinde
    BİRLİKTE tutulur (her authorized hub için ≤1 güncel kayıt).
  - Her hub, sorumlu olduğu routing_key'ler için periyodik (1 saat) SYNC yapar.

AuthorizedHubSetRecord reconciliation:
  - LWW scope: (operator_id, dest_hash); scoped_sequence ile (RA/Intermediate-imzalı).
  - Daha yüksek scoped_sequence kazanır.

Route selection SYNC'te YAPILMAZ — sonradan AuthorizedHubSet role/priority + liveness +
  freshness ile yapılır (5.7). SYNC yalnızca replikalar arası tutarlılık (anti-entropy).

Split-brain (aynı scope + aynı sequence_number, FARKLI imza) → KRİPTOGRAFİK ANOMALİ → alert + log.
  Continuity (NORMATİF): ilgili hub_id record'u KARANTİNAYA alınır (otomatik kabul YOK), AMA routing DURMAZ —
  Bob'un diğer AHS-authorized hub'ları üzerinden devam eder (multi-hub failover 5.7). "Otomatik kabul yok"
  total-stall DEĞİLDİR; yalnız o çelişkili hub_id record'u devre dışı bırakılır.
  (Aynı seq iki farklı geçerli imza = ya key compromise ya bug; otomatik kabul YOK.)
```

---

## 5. RoutingRecord — dest_hash → hub_id (Konum)

### 5.1 Kavramsal Ayrım: RoutingRecord vs DiscoveryRecordV2

İki ayrı kayıt, iki ayrı soru, iki ayrı okuyucu — **karıştırılmamalı** (CAD-53):

| | **DiscoveryRecordV2** | **RoutingRecord** |
|--|----------------------|-------------------|
| Soru | "Bu kişi kim?" (kimlik) | "Bu dest_hash şu an nerede?" (konum) |
| İçerik | dest_hash + cert + static X25519 | dest_hash → hub_id |
| Nerede | Discovery Server (kurum) | DHT (hub mesh) |
| Kim okur | **Client** (Alice) | **Hub** (client ASLA) |
| İmzalayan | Bob (sk_user_identity) | Hub (sk_hub_identity) |
| Değişkenlik | Statik (cert yenilenince) | Dinamik (failover/bağlantı) |
| Magic | SRDR | SRRR |
| Belge | ccr-spec §21 + discovery | bu belge |

DiscoveryRecordV2 **konum taşımaz** (home_hub_id kaldırıldı, CAD-53). Konum tamamen RoutingRecord'da, DHT'de, hub-mesh içinde. Client routing topolojisini hiç görmez.

### 5.2 Yetki Modeli — Hub-İmzalı + Örtük Noise IK Possession

RoutingRecord'u **hub imzalar** (`sk_hub_identity`). Bob'un ayrı bir imzası YOKTUR. Bob'un "bu hub'tayım" onayı **örtüktür**: hub, RoutingRecord yayımlamadan önce Bob ile Noise IK handshake kurmuş ve Bob'un `sk_device` possession'ını kanıtlatmıştır (identity v1.5 CAD-46 + Client→Hub IK).

```
Neden örtük yeterli (açık Bob-imzası GEREKMİYOR):
  - Kötü Hub X "dest_hash_bob bende" diye sahte RoutingRecord yayabilir.
  - AMA Hub X, Bob'u gerçekten register EDEMEZ: Noise IK handshake'inde Bob'un
    sk_device possession'ı gerekir (Bob'un private key'i Hub X'te yok).
  - Hub X sahte RoutingRecord yaysa bile: Alice'in mesajı Hub X'e gider, ama
    Hub X mesajı OKUYAMAZ (Session Model V3 AEAD; ss_e/ss_s için Bob'un static X25519 sk'sı lazım — E2E).
  - Hub X yalnızca SUPPRESS edebilir (mesajı düşürür) = availability residual,
    İÇERİK SIZINTISI DEĞİL. Alice N-mesaj-ACK penceresi ile sessiz kaybı yakalar
    (identity v1.5 CAD-45) → çok-hub failover → başka hub üzerinden Bob'a ulaşır.

Maliyet kararı: Açık Bob-imzası +64B (RoutingRecord'a) + Bob'un katılımı getirirdi.
  Örtük model bunu gerektirmez — Noise IK possession zaten bağlantı anında kanıtlanır.
  (Tutarlı: ilk temasta 152B CompactDeviceCert, sonra 32B ref; +64B routing imzası gereksiz.)
```

> **NORMATİF (yetki):** RoutingRecord meşruiyeti hub'ın Bob ile **Client↔Hub Noise IK kurmuş olmasından** gelir (sk_device possession). Bir hub, Noise IK possession kanıtı olmadan bir dest_hash için RoutingRecord yayımlamamalıdır. Sahte RoutingRecord (kötü hub) içerik sızdıramaz (E2E Session Model V3 AEAD); yalnızca suppression yapar (ACK penceresi + çok-hub failover ile telafi). Bu, identity v1.5 split-brain kararıyla (CAD-46) birebir tutarlıdır.

### 5.3 RoutingRecord Wire Format

```
RoutingRecord {
  magic               : bytes4      # "SRRR"
  version             : u8          # 1
  operator_id         : bytes16     # kurum namespace (sequence scope + cross-operator izolasyon, CAD-58)
  routing_key         : bytes16     # = dest_hash[0:16] (DHT anahtarı; CAD-16)
  dest_hash           : bytes32     # tam dest_hash (routing_key türetim doğrulaması için)
  hub_id              : bytes16     # Bob'un (authorized) hub'ı (HubEntry.hub_id)
  sequence_number     : u64_be      # MONOTONIC, scope (operator_id, dest_hash, hub_id) — PER-HUB (CAD-58)
  ttl_expires_at      : u64_be      # unix saniye (kaydın geçerlilik sonu)
  issued_at           : u64_be      # unix saniye (yayım anı)
  signature           : bytes64     # Ed25519_Sign(sk_hub_identity, canonical_routing_record_tbs)
}
```

> **Boyut:** sabit 4+1+16+16+32+16+8+8+8+64 = **173 byte**. RoutingRecord hub-mesh içinde dolaşır (datacenter), client'a gitmez; bant kaygısı yok. (`operator_id` eklendi → 157B'den 173B'ye; per-hub sequence scope wire'da temsil edilir.)

### 5.4 canonical_routing_record_tbs

```
canonical_routing_record_tbs =
  "SRRR" (magic, 4B) || version || operator_id || routing_key || dest_hash || hub_id ||
  sequence_number || ttl_expires_at || issued_at

signature = Ed25519_Sign(sk_hub_identity, canonical_routing_record_tbs)
```

`signature` TBS'e **dahil değildir**. Magic-in-TBS disiplini geçerli.

### 5.5 RoutingRecord Doğrulama (Hub Tarafı)

```
verify_routing_record(rr, fresh_crl, now):
  # Yalnızca HUB çalıştırır (client ASLA — CAD-53). AuthorizedHubSet DHT STORE için ANA GÖVDEDE ZORUNLU.
  # 1. FORMAT: magic=="SRRR", version==1, exact 173B, trailing byte → DROP
  if rr.magic != "SRRR" or rr.version != 1 or len(rr) != 173: DROP

  # 2. routing_key türetim tutarlılığı
  if rr.routing_key != rr.dest_hash[0:16]: DROP

  # 3. AuthorizedHubSet ZORUNLU (DHT STORE precondition — yoksa hiçbir RoutingRecord kabul edilmez):
  ahs = dht_lookup_authorized_hubset(rr.operator_id, rr.dest_hash)
  if ahs is None: DROP

  # 4. verify_authorized_hubset(ahs):
  if ahs.magic != "SRAH" or ahs.version != 1: DROP
  if ahs.operator_id != rr.operator_id or ahs.dest_hash != rr.dest_hash: DROP
  if now > ahs.not_after: DROP
  if cached_ahs(ahs.operator_id, ahs.dest_hash) and ahs.scoped_sequence < cached_ahs_seq: DROP   # LWW
  intermediate_cert = resolve_intermediate_cert(ahs.issuer_key_id)
  if verify_chain(intermediate_cert, ROOT_CA_SPKI_HASH) == FALSE: DROP
  if NOT Ed25519_Verify(intermediate_cert.public_key, canonical_authorized_hubset_tbs(ahs), ahs.signature): DROP

  # 5. allowlist check: rr.hub_id AuthorizedHubSet içinde OLMALI:
  if rr.hub_id NOT IN {e.hub_id for e in ahs.hubs}: DROP                     # route poisoning reddi

  # 6. İmzalayan Hub Identity cert'ini çöz:
  hub_cert = resolve_hub_identity_cert(rr.hub_id)
  if hub_cert is None: DROP

  # 7. HubListRecord binding (HubEntryV3 cert-bound; raw hub_ed25519_pub yok):
  hub_entry = current_hublist.find_by_hub_id(rr.hub_id)
  if hub_entry is None: DROP
  if parse(hub_entry.hub_identity_cert).public_key != hub_cert.public_key: DROP

  # 8. Hub cert doğrula:
  if verify_chain(hub_cert, ROOT_CA_SPKI_HASH) == FALSE: DROP
  if id-kama-hub NOT in hub_cert.eku: DROP
  if fresh_crl.contains(hub_cert): DROP                                       # fail-closed
  if now > hub_cert.not_after: DROP

  # 9. RoutingRecord imzası (hub_cert.public_key ile):
  if NOT Ed25519_Verify(hub_cert.public_key, canonical_routing_record_tbs(rr), rr.signature): DROP

  # 10. TTL incoming validation: expired RoutingRecord KABUL EDİLMEZ (cache davranışı 5.6'da AYRI)
  if now > rr.ttl_expires_at: DROP

  # 11. scoped LWW: scope (operator_id, dest_hash, hub_id) — GLOBAL DEĞİL (CAD-58)
  scope = (rr.operator_id, rr.dest_hash, rr.hub_id)
  if cached(scope) and rr.sequence_number < cached_seq(scope): DROP          # per-hub rollback koruması

  # 12. KABUL: store/update RoutingRecordSet (aynı dest_hash için birden çok authorized hub)
  dht_store(rr.routing_key, rr)
```

> **NORMATİF (AHS precondition):** AuthorizedHubSet DHT STORE için ön-koşuldur. Geçerli AHS olmadan hiçbir RoutingRecord DHT depolamasına kabul edilmez. Doğrudan client↔hub kayıt sırasında oluşan lokal mailbox binding bir DHT RoutingRecord DEĞİLDİR ve AHS var olmadan replike EDİLEMEZ.

> **NORMATİF (rollback koruması — CAD-58):** `sequence_number` scope'u `(operator_id, dest_hash, hub_id)` — **global DEĞİL**. Aynı `(operator, dest_hash, hub)` üçlüsü için daha düşük `sequence_number` taşıyan RoutingRecord **reddedilir** (LWW). Scope per-hub olduğu için, bir authorized hub'ın yüksek sequence'i **başka** bir authorized hub'ın route'unu yanlışlıkla ezmez (multi-hub failover, 5.7, kritik). Bu, bir hub'ın kendi eski konum kaydını "replay" ederek Bob'u eski (artık erişilemez) hub'a yönlendirmesini engeller; her authorized hub kendi seq zincirini ayrı yürütür.

### 5.6 TTL Davranışı — Incoming Validation vs Cached Lookup (NORMATİF ayrım)

```
İKİ AYRI DAVRANIŞ (karıştırılmamalı):

(a) Incoming STORE validation (5.5 adım 8):
    expired RoutingRecord (now > ttl_expires_at) KABUL EDİLMEZ → DROP.
    DHT'ye bayat kayıt YAZILMAZ (yeni kayıt her zaman fresh olmalı).

(b) Cached lookup behavior:
    DHT'de ZATEN bulunan expired RoutingRecord hemen SİLİNMEZ:
      - Bant yok ki sürekli republish yapılsın (hostile-aware tasarım).
      - FRESH record VARSA: fresh kullanılır (expired'a düşülmez).
      - FRESH record YOKSA: expired record yalnızca BEST-EFFORT / DEGRADED fallback
        olarak kullanılabilir (son bilinen konum) — "stale" işaretiyle.
      - Yeni RoutingRecord (Bob yeniden bağlandı, daha yüksek seq) gelince üzerine yazılır (LWW).
      - Tamamen offline hub: Kademlia k-bucket'tan düşer → record doğal olarak temizlenir.
```

> **Özet:** Yeni kayıt store edilirken expired KABUL EDİLMEZ (a); ama cache'te halihazırda olan expired kayıt, daha iyisi yoksa best-effort fallback olarak tutulabilir (b). Bu ayrım §5.5 ile §5.6 arasındaki görünür çelişkiyi kapatır.

> **NORMATİF (expired fallback YALNIZCA geçerli AHS ile — patch):** Expired cached RoutingRecord fallback'i AŞAĞIDAKİ KOŞULLARIN TÜMÜ sağlanırsa kullanılabilir:
> 1. Aynı dest_hash için fresh RoutingRecord YOK.
> 2. `(operator_id, dest_hash)` için **geçerli AuthorizedHubSetRecord** var.
> 3. `now <= ahs.not_after`.
> 4. `rr.hub_id` hâlâ `ahs.hubs[].hub_id` içinde.
> 5. Hub hub-mesh liveness/PING/ANNOUNCE cache'ine göre erişilebilir.
> 6. Hub Identity cert hâlâ geçerli ve revoke değil.
> 7. Route STALE/DEGRADED işaretli; yalnızca best-effort denenir.
>
> AHS expired/eksikse expired RoutingRecord fallback olarak **KULLANILAMAZ**. Expired route fallback lisans expiry / suspend / revocation / AuthorizedHubSet policy'yi BYPASS EDEMEZ — AHS, degraded last-known-location routing'de bile authorization gate'idir (Threat R-13/R-14 ile bağlı).

### 5.7 Çok-Hub Failover ile Route Selection (NORMATİF — per-hub sequence)

Bob ~3 hub'a kayıtlı (primary/secondary/tertiary — identity v1.5 CAD-46). **DİKKAT:** `sequence_number` scope'u `(operator_id, dest_hash, hub_id)` PER-HUB olduğundan (CAD-58, 5.5), **farklı hub'ların sequence'leri KARŞILAŞTIRILAMAZ** — per-hub sequence yalnızca **aynı hub'ın eski kaydını** reddetmek içindir (rollback koruması). "seq büyük olan hub kazanır" mantığı YANLIŞTIR.

```
Her authorized hub kendi RoutingRecord'unu kendi seq zinciriyle STORE eder:
  - Hub B: RoutingRecord(→ Hub B, seq_B=N)    # Hub B'nin kendi seq zinciri
  - Hub C: RoutingRecord(→ Hub C, seq_C=M)    # Hub C'nin kendi seq zinciri (N ile KARŞILAŞTIRILMAZ)
  FIND_VALUE → RoutingRecordSet = {Hub B kaydı, Hub C kaydı, ...} (her authorized hub için ≤1 güncel kayıt)

ROUTE SELECTION (gönderen hub, deterministik — "seq büyük" DEĞİL):
  1. AuthorizedHubSet'te yetkili hub mı? (e.hub_id ∈ ahs.hubs)         # değilse aday değil (DROP, 5.8)
  2. RoutingRecord fresh mı? (now <= ttl_expires_at)                   # expired → degraded fallback (5.6)
  3. Hub reachable mı? (hub-mesh liveness / PING / ANNOUNCE cache)     # ulaşılamaz → sıradaki adaya geç
  4. AuthorizedHubEntry.role + priority ne diyor?                      # primary < secondary < tertiary; düşük priority önce
  5. (Aynı hub için) en yüksek sequence_number'lı kayıt seçilir.       # per-hub LWW (rollback koruması)

  → Sonuç: AuthorizedHubSet policy (role/priority) + liveness + freshness kazanır;
    farklı hub'ların sequence'i kıyaslanarak DEĞİL.
```

> **NORMATİF:** Failover'da hub seçimi `AuthorizedHubEntry.priority`/`role` (control-plane policy) + reachability + per-hub freshness ile yapılır. Sequence yalnızca **aynı (operator, dest_hash, hub)** üçlüsünde LWW/rollback için kullanılır — asla iki farklı hub arasında "hangisi güncel" kararı için değil. Alice hiçbir şey görmez: yalnızca dest_hash verir; hub-mesh bu seçimi yapar (CAD-53).

### 5.8 AuthorizedHubSet — Route Poisoning Koruması (CAD-58, NORMATİF)

RoutingRecord yetkisi örtük Noise IK possession'a dayanır (5.2). Ancak Bob ~3 hub'a kayıtlı olduğundan (CAD-46), **hangi hub'ların Bob için RoutingRecord yayımlamaya yetkili olduğunu** sınırlamak route poisoning'i (kötü bir hub'ın kendini Bob'un hub'ı ilan etmesi) kontrol-düzleminde daraltır. Çözüm: **AuthorizedHubSetRecord** — **control-plane authorization nesnesi**, **client 0 byte** (istemci görmez/taşımaz/imzalamaz).

> **NORMATİF (yetki kaynağı — CAD-58):** AuthorizedHubSetRecord'u **RA/Intermediate (control-plane) imzalar — KULLANICI DEĞİL.** Bu kasıtlıdır: hangi hub'ın kurum namespace'i içinde bir dest_hash için yetkili olduğu bir **routing/infra-authorization** kararıdır, kullanıcı kararı değil. Mekanizma HubListRecord'la birebir aynı (#E-10/24): **Admin Server hazırlar** (kullanıcının hangi hub'lara enroll olduğunu bilir) → **RA license/admin gate** → **Intermediate CA otomatik imzalar** (server-to-server mTLS). `sk_user_identity` ile imzalama **YASAKTIR** — aksi halde (1) kurum hub politikası delinir (Bob keyfi hub yetkilendirir gibi görünür), (2) RA/license/admin gate zayıflar, (3) user identity key rolü gereksiz büyür (role separation bozulur), (4) suspend/lisans-bitti durumunda Bob'un önceden imzaladığı uzun-TTL hub set'leri revocation'ı zorlar, (5) trust boundary yanlış yere kayar. **Client-signed RoutingRecord/HubSet YOK.**

```
AuthorizedHubEntry {
  hub_id              : bytes16     # yetkili hub
  role                : u8          # 0=primary, 1=secondary, 2=tertiary, 3=mailbox, 4=relay
  priority            : u8          # düşük = daha öncelikli (deterministik route selection)
  flags               : u16_be      # reserved (Beta: capability bitleri)
}                                   # sabit 20B/entry

AuthorizedHubSetRecord {
  magic               : bytes4      # "SRAH"
  version             : u8          # 1
  operator_id         : bytes16     # kurum namespace (cross-operator izolasyon)
  issuer_key_id       : bytes16     # imzalayan Intermediate'in subject_key_id (cert lookup hint; HubListRecord disiplini)
  dest_hash           : bytes32     # kapsanan kimlik
  hub_count           : u8          # yetkili hub sayısı (≤ küçük sabit, ör. ≤8)
  hubs                : AuthorizedHubEntry[hub_count]   # hub_id + role + priority + flags (her biri 20B)
  scoped_sequence     : u64_be      # MONOTONIC, (operator_id, dest_hash) scope'unda LWW
  issued_at           : u64_be
  not_after           : u64_be      # <= license_not_after (lisans bitince yenilenmez → doğal expiry)
  signature           : bytes64     # Ed25519_Sign(sk_intermediate, canonical_authorized_hubset_tbs)
                                    #   (Admin Server hazırlar → RA gate → Intermediate otomatik imzalar)
}

canonical_authorized_hubset_tbs =
  "SRAH" || version || operator_id || issuer_key_id ||
  dest_hash || hub_count || hubs || scoped_sequence ||
  issued_at || not_after
# issuer_key_id ile imzalayan Intermediate cert çözülür (resolve_intermediate_cert(issuer_key_id));
# sonra verify_chain(intermediate_cert → Root, PINNED) + Ed25519_Verify(intermediate.spki, tbs, signature).
# HubListRecord (ccr-spec §21.5) issuer_key_id disipliniyle birebir aynı.
```

> **Route selection deterministik (CAD-58):** `AuthorizedHubEntry.role` + `priority` çok-hub failover'da deterministik hub seçimi sağlar (bare `hub_id` listesi yetmezdi). Primary/secondary/tertiary ayrımı + mailbox/relay rolleri AuthorizedHubSet'te tanımlıdır; hub-mesh route'u bu policy'ye göre seçer (5.7).

> **NORMATİF (allowlist enforcement):** Bir hub, gelen RoutingRecord'u kabul etmeden önce AuthorizedHubSet'i kontrol eder: `rr.hub_id` AuthorizedHubSet'in `hubs[].hub_id` listesinde yoksa RoutingRecord **DROP** (route poisoning reddi). Kötü hub kendini Bob'un hub'ı ilan etse bile, RA/Intermediate onu authorized listeye koymadıysa reddedilir. `scoped_sequence` `(operator_id, dest_hash)` scope'unda LWW → eski AuthorizedHubSet replay edilemez. **Revocation/suspend:** Admin kullanıcıyı suspend ettiğinde veya lisans bittiğinde RA yeni AuthorizedHubSet **üretmez** → eski kayıt `not_after`'da (≤ license_not_after) doğal expire → yeni RoutingRecord kabul edilmez. (Kullanıcı imzalı uzun-TTL set olmadığı için revocation temiz.)

> **NORMATİF (entegrasyon):** AuthorizedHubSet doğrulaması artık `verify_routing_record` (5.5) **ana gövdesinde** (adım 3–5) zorunlu ön-koşuldur — ayrı/opsiyonel "ek adım" DEĞİL. Aşağıdaki kanonik akış 5.5 adım 3–5'in özetidir:
```
# verify_routing_record (5.5) adım 3-5 — AHS ana gövdede:
  ahs = dht_lookup_authorized_hubset(rr.operator_id, rr.dest_hash)   # RA/Intermediate-imzalı
  if ahs is None: DROP                                               # AHS YOKSA DHT STORE KABUL EDİLMEZ
  intermediate_cert = resolve_intermediate_cert(ahs.issuer_key_id)
  verify_chain(intermediate_cert, ROOT_CA_SPKI_HASH)                 # control-plane authority
  Ed25519_Verify(intermediate_cert.public_key, canonical_authorized_hubset_tbs(ahs), ahs.signature)
  if ahs.not_after < now: DROP                                       # lisans/suspend doğal expiry
  if rr.hub_id NOT IN {e.hub_id for e in ahs.hubs}: DROP             # route poisoning reddi
```

> **NORMATİF (AHS zorunlu — patch, route poisoning sertleştirme):** **DHT STORE için AuthorizedHubSet ZORUNLUDUR.** AHS yoksa RoutingRecord DHT'ye **kabul edilmez** (eski "AHS yoksa örtük Noise IK possession yeterli" fallback'i KALDIRILDI — kötü hub'a "AHS yoktu, implicit possession'a dayanıyorum" alanı açıyordu). Yalnızca **local registration** sırasında hub kendi **lokal mailbox binding**'ini oluşturabilir (Bob ona bağlanırken, sk_device possession ile); bu kayıt **DHT'ye replike edilmez** ve **kısa TTL**'lidir (yalnızca o hub kendi local client'ına teslim için). RA/Intermediate-signed AuthorizedHubSet DHT'ye geldikten **sonra** o dest_hash için DHT STORE açılır. Böylece route poisoning penceresi kapanır: DHT'deki her RoutingRecord mutlaka bir AHS allowlist'ine tabi.

**Maliyet/kapsam:** AuthorizedHubSet hub-mesh içinde dolaşır (DHT, datacenter); **istemciye 0 byte** (hostile link'e dokunmaz, istemci imzalamaz/görmez). Admin Server kayıt/failover anında (iyi internet, server-to-server) günceller. Client routing topolojisini hâlâ görmez (CAD-53 korunur); routing authority RoftCore/RA/Admin control-plane'inde kalır (RA/license/admin gate ile aynı güven sınırı, CAD-58).

> **RoutingRecord scope (CAD-58):** RoutingRecord `sequence_number` scope'u `(operator_id, dest_hash, hub_id)` — **global DEĞİL**. FIND_VALUE bir dest_hash için **RoutingRecordSet** dönebilir (Bob'un birden çok authorized hub'ı varsa); hub-mesh en güncel/uygun kaydı seçer. Bu, çok-hub failover (5.7) ile tutarlıdır.

---

## 6. Hub-Mesh Noise Transport

### 6.1 İlk ANNOUNCE Plaintext, Sonrası Şifreli

```
İlk ANNOUNCE: PLAINTEXT (key henüz yok — ilk tanışma).
  Kabul edilen sızıntı: passive observer "Hub C, IP:port ile mesh'e katıldı" görür.
  Bu kabul edilebilir: hub'lar PUBLIC altyapıdır (IP'leri HubListRecord'da zaten var),
  "yeni hub katıldı" operasyonel bir olaydır, gizli değildir. İÇERİK değil.

Sonraki TÜM hub-to-hub DHT/routing mesajları: Noise ile ŞİFRELİ.
  - KK (her iki taraf birbirinin pk_noise'ını ANNOUNCE'tan cache'ledi) → 0-RTT.
  - IK (gönderen alıcının pk_noise'ını biliyor, alıcı göndereni bilmiyor — yeni hub) → gönderen static gider.
  - IKfallback (alıcı key rotate etti, gönderen eski pk_noise ile başarısız) → Section 6.3.
```

### 6.2 Noise Pattern Seçimi (Hub→Hub)

```
Hub C → Hub A'ya mesaj:

Durum 1 — KK (her ikisi birbirini biliyor, 0-RTT):
  Hub C cache: hub_a_pk_noise (Hub A ANNOUNCE'undan)
  Hub A cache: hub_c_pk_noise (Hub C ANNOUNCE'undan)
  → Noise KK, sıfır handshake overhead, mesaj direkt şifreli.

Durum 2 — IK (Hub C biliyor, Hub A bilmiyor — yeni hub):
  Hub C cache: hub_a_pk_noise var
  Hub A cache: hub_c_pk_noise YOK
  → Noise IK; Hub C static key'ini de gönderir; Hub A cache'ler → sonraki KK.

Durum 3 — Hiçbiri bilmiyor:
  ANNOUNCE önce gelmiş olmalı (bu durum normalde oluşmaz).
  Oluşursa: ANNOUNCE bekle (FIND_NODE ile çek), sonra mesaj.
```

Noise suite hub-mesh için: `Noise_KK_25519_ChaChaPoly_BLAKE2s` (KK) / `Noise_IK_...` (IK). Client↔Hub ve Client↔Client ile aynı kripto ailesi (Curve25519/ChaChaPoly/BLAKE2s) — tutarlılık.

### 6.3 Hub→Hub IKfallback

```
Senaryo: Hub A, Hub B'nin eski pk_noise'ını cache'liyor.
  Hub B Noise key'ini rotate etti (yeni ANNOUNCE yaydı), Hub A henüz görmedi.
  Hub A → Hub B: Noise KK (ESKİ pk_noise ile) → Hub B decrypt EDEMEZ.

Çözüm (Client→Hub IKfallback ile AYNI mekanizma):
  Hub B, KK decrypt başarısız olunca → IKfallback ile yeni hub_noise_cert'i gönderir.
  Hub A: verify_chain(yeni hub_noise_cert, ROOT_CA_SPKI_HASH) → geçerliyse cache güncelle.
  Session yeni pk_noise ile devam eder.
```

### 6.4 Per-Recipient Şifreleme (k=5)

```
Hub C bir RoutingRecord'u k=5 en-yakın node'a replike edecek (STORE):

  for each target_hub in k_closest_nodes(routing_key):
    encrypted = Noise_KK_encrypt(
      sender_sk   = hub_c_sk_noise,
      receiver_pk = cache[target_hub.hub_id].pk_noise,
      payload     = RoutingRecord
    )
    send(target_hub.address, encrypted)

  Her alıcı AYRI şifreli mesaj alır (kendi pk_noise'ıyla).
  Compromised node: yalnızca KENDİNE gelen mesajı okur (diğer 4'ünkini değil).
  Passive observer: içeriği/dest_hash'i GÖREMEZ (Noise şifreli).
  Kaçınılmaz (UDP doğası, kabul): gönderen IP + alıcı IP görünür (ama içerik yok).
```

### 6.5 Kabul Edilen Metadata Sınırı

```
Kapatılan (hub-mesh Noise ile):
  - DHT/routing mesaj içeriği → şifreli.
  - dest_hash → şifreli payload içinde, dışarıdan görünmez.
  - RoutingRecord içeriği → şifreli.

Kaçınılmaz (DHT/UDP doğası — kabul edilen sınır, identity v1.5 §3.1/§3.2 ile tutarlı):
  - Hub→hub gönderen/alıcı IP (UDP).
  - Mesaj zamanlaması.
  - "Hub A, Hub B ile bir şey konuştu" (ne konuştuğu değil).
  Bu, Kama'nın kabul ettiği routing-level metadata sınırıdır (Signal/Tor da çözememiştir).
  Kama iddiası metadata gizliliği DEĞİL, extrem-düşük-bant + yüksek-kayıpta İLETİŞİMİN SÜRMESİ.
  Cover traffic Beta+.
```

---

## 7. Hub-to-Hub RPC Envanteri

Tüm hub-to-hub RPC'ler Section 6 Noise transport üzerinden taşınır (ilk ANNOUNCE hariç plaintext). RPC payload encoding: Kama-native fixed-width (ccr-spec primitive kuralları; JSON/protobuf değil). Her RPC bir `method_id` (u8) taşır.

### 7.1 method_id Tablosu

```
0x01  ANNOUNCE              hub → mesh    (Section 3; ilk plaintext, sonra Noise)
0x02  FIND_NODE            hub → hub     (Kademlia: hub_id'ye en yakın k node)
0x03  FIND_VALUE           hub → hub     (routing_key → AuthorizedHubSet+RoutingRecordSet; Kademlia lookup)
0x04  STORE                hub → hub     (RoutingRecord'u k=5 node'a replike et)
0x05  FORWARD_MESSAGE      hub → hub     (Session Model V3 envelope'ı hedef hub'a taşı; Section 2.1)
0x06  SIGNALING_RELAY      hub → hub     (senkron çağrı kurulumu relay; Section 8)
0x07  SYNC                 hub → hub     (anti-entropy; RoutingRecord LWW reconciliation; Section 4.4)
0x08  PING                 hub → hub     (Kademlia liveness; k-bucket eviction)
```

### 7.2 RPC Tanımları

```
FIND_NODE { target_hub_id: bytes16 }
  → { nodes: [ {hub_id, addresses} ]  (en yakın k=5) }
  Kademlia node lookup; bucket doldurma ve mesh keşfi.

FIND_VALUE { routing_key: bytes16 }
  → { authorized_hubset: AuthorizedHubSetRecord,           # RA/Intermediate-imzalı (5.8)
      routing_records: RoutingRecordSet }                  # dest_hash için authorized hub'ların kayıtları
  VEYA { nodes: [...] }                                    # bulunamazsa en yakın k node
  dest_hash → hub_id çözümü. CAD-58: tek RoutingRecord DEĞİL — bir dest_hash için birden çok
  authorized hub olabilir (multi-hub failover, 5.7) → RoutingRecordSet döner. İstemci tarafında
  hub-mesh AuthorizedHubSet ile filtreler (authorized olmayan hub kayıtları DROP, 5.8) ve en güncel/
  uygun route'u seçer. Bulunamazsa iteratif lookup için en yakın node'lar (hub mesh içinde, client değil).

STORE { record: RoutingRecord | AuthorizedHubSetRecord }
  → { ack: bool }
  - RoutingRecord: Bob'un (authorized) hub'ı, RoutingRecord'u k=5 en yakın node'a replike eder
    (per-recipient Noise, 6.4). Alıcı verify_routing_record (5.5, AuthorizedHubSet kontrolü dahil) çalıştırır.
  - AuthorizedHubSetRecord: RA/Intermediate-signed control-plane kaydı (5.8 — Admin Server hazırlar →
    RA gate → Intermediate otomatik imzalar; KULLANICI yayımlamaz/imzalamaz). Alıcı verify_authorized_hubset
    çalıştırır (signature → Intermediate → Root, PINNED). Geçerliyse DHT'ye yazar.
  (Her ikisi de aynı routing_key=dest_hash[0:16] anahtar uzayında, ayrı record tipleri.)

FORWARD_MESSAGE { recipient_dest_hash: bytes32, session_envelope: SessionModelV3Envelope }
    # session_envelope ∈ { SESSION_INIT, SESSION_INIT_RESPONSE, SESSION_DATA, SIGNED_CONTROL }
    # AsyncSessionV2Envelope deprecated terminoloji — aktif RPC tanımlarında KULLANILMAZ
  → { ack: bool | LINK_ROUTE_MISS }
  Hub A, session_envelope'ı (SESSION_INIT/SESSION_INIT_RESPONSE/SESSION_DATA/SIGNED_CONTROL) Bob'un hub'ına (Hub B) taşır. Hub B, Bob'un
  mailbox'ına koyar. Envelope E2E şifreli (ccr-spec §35) — Hub A/B içeriği OKUYAMAZ, yalnızca taşır/saklar.
  Hub B Bob için aktif route bulamazsa LINK_ROUTE_MISS döner → istemci SESSION_INIT_REF rebind ile
  session'ı yeniden kurar (ccr-spec §35.16; Alpha'da ayrı resume frame YOK).

  ROUTING ANAHTARI (NORMATİF, plane-bağımlı):
    - SESSION_INIT (konuşma başlatma): Hub A recipient_dest_hash ile route resolve eder (FIND_VALUE/DHT) +
      outbound_session_route[alice_hub_handle_A]={Hub B, dest_hash} bind eder (CAD-57; alice_hub_handle_A = HUB-A-LOCAL).
    - Established SESSION_DATA (hot-path): client wire'da recipient_dest_hash TAŞIMAZ; Hub A KENDİ local
      alice_hub_handle_A → outbound_session_route cache lookup ile route eder (DHT YOK). hub_handle correlation-dirençli
      (hub-local opaque, dest_hash'ten türemez) — hub hub_handle'dan kimlik çıkaramaz, yalnızca route eder.

  hub_handle LOCALITY (NORMATİF, P1):
    FORWARD_MESSAGE Session Model V3 L3 envelope/blob taşır (+ recipient_dest_hash).
    Sender-tarafı Client↔Hub L2 hub_handle'ı cross-hub handle olarak TAŞIMAZ.
    SESSION_DATA için:
      taşınan payload = seq16 || ciphertext || tag16   (L3 envelope)
      taşınmayan      = Alice↔HubA hub_handle          (Hub A consume eder; Hub B yorumlamaz)
    Hub B kendi local Bob delivery binding'ini kurar; online push'ta Bob↔HubB için KENDİ bob_hub_handle_B'sini kullanır.

SIGNALING_RELAY { recipient_dest_hash: bytes32, signaling_payload: varbytes }
  → { ack: bool }
  Senkron çağrı kurulumu (call_offer/call_answer) relay. Alpha: Noise handshake init/resp
  taşır; media RELAY path üzerinden akar (Section 8). Derin SDP/ICE Beta.

SYNC { routing_keys: [bytes16], digests: [bytes32] }
  → { updates: [RoutingRecord] }
  1-saatlik anti-entropy. Replikalar arası SCOPED LWW reconciliation (4.4):
  RoutingRecord LWW scope = (operator_id, dest_hash, hub_id) — farklı hub'lar EZMEZ (RoutingRecordSet);
  AuthorizedHubSet LWW scope = (operator_id, dest_hash), scoped_sequence.
  Aynı scope'ta digest mismatch → daha yüksek sequence kazanır; split-brain (aynı scope+seq, farklı sig) → alert (4.4).

PING { nonce: bytes8 }
  → { nonce: bytes8 }  (echo)
  Kademlia liveness. Bucket eviction kararı (en eski node ping'lenir, 4.3).
```

### 7.3 DHT vs Diğer RPC Ayrımı

```
SADECE routing/DHT mekaniği: FIND_NODE, FIND_VALUE, STORE, SYNC, PING, ANNOUNCE.
Taşıma (DHT değil, ama hub-mesh): FORWARD_MESSAGE (async), SIGNALING_RELAY (senkron).
  Bunlar aynı Noise transport'u kullanır ama farklı amaç:
  DHT = "nerede" çözümü; FORWARD/RELAY = "oraya taşı".
```

---

## 8. Senkron Rendezvous (Çağrı Kurulumu — Alpha Relay-Only)

> **Kapsam (NORMATİF):** Alpha'da senkron rendezvous yalnızca **Noise pattern + relay-only taşıma iskeleti** olarak tanımlıdır. Derin SDP/ICE müzakeresi, TURN yönetimi, codec negotiation Beta'dadır (`signaling-architecture.md`). Bu bölüm Alpha iskeletini verir; Beta bunu genişletir, değiştirmez.

### 8.1 Relay-Only — Neden NAT Sorunu Yok

```
P2P YOK. Her şey hub RELAY üzerinden:
  Alice ↔ Hub A ↔ Hub B ↔ Bob   (media dahil)

Sonuç: NAT traversal GEREKMİYOR.
  - Alice yalnızca kendi hub'ına (Hub A) bağlanır — outbound, NAT-dostu.
  - Bob yalnızca kendi hub'ına (Hub B) bağlanır — outbound, NAT-dostu.
  - Hub'lar public IP'li (datacenter), birbirine ulaşır.
  - İki NAT arkasındaki client'ın doğrudan buluşması (STUN/TURN/ICE hole-punching) GEREKMEZ.
```

Bu, hostile-network hedefiyle tutarlıdır: client'ın tek yapması gereken kendi hub'ına outbound bağlantı (zaten async için de öyle). P2P hole-punching hostile/kısıtlı ağlarda güvenilmezdir; relay deterministiktir.

### 8.2 Çağrı Kurulumu Akışı (Alpha)

```
1. Alice → Hub A: CALL_SETUP { recipient_dest_hash, call_offer }
     call_offer = Noise handshake init (Alice'in çağrı oturumu için ephemeral)
2. Hub A: FIND_VALUE(dest_hash_bob[0:16]) → Hub B (Bob'un hub'ı)
3. Hub A → Hub B: SIGNALING_RELAY { call_offer }
4. Hub B → Bob: call_offer teslim (Bob online ise; değilse çağrı düşer — timeout)
5. Bob → Hub B → Hub A → Alice: call_answer (Noise handshake resp)
6. Noise oturumu kurulur (Alice↔Bob, uçtan uca; hub'lar relay, içeriği göremez)
7. Media akışı: relay path üzerinden (Alice↔Hub A↔Hub B↔Bob), Noise transport şifreli
```

### 8.3 Senkron Media — Hub Aptal Boru (Relay)

```
Asenkron'da hub AKTİF transport (custom ARQ transport'u hop-bazlı sonlandırır: kendi ARQ/FEC state'iyle
her hop'u ayrı kurtarır — client↔hub ve hub↔hub bağımsız; KCP çıkarıldı, transport-layer-tdd.md §4).
Senkron'da (voice/video/PTT) hub APTAL BORU relay:
  - Her paketi hop-bazlı ARQ ile söküp-sarmak gerçek-zamanlı olmaz (RTT/bant/kayıp UÇLAR ARASINDA).
  - Hub paketi olduğu gibi relay eder (Noise şifreli içerik, uçtan uca).
  - Codec/jitter/loss adaptasyonu uçlarda (Alice↔Bob), hub'da değil.
```

> Bu ayrım `transport-layer-tdd.md` ile koordine olur: asenkron hop-bazlı custom ARQ/FEC vs senkron uçtan-uca relay. Senkron media transport optimizasyonu (codec switching, adaptif bitrate) Beta.

### 8.4 Alpha Sınırları (Açık)

```
Alpha senkron rendezvous:
  ✓ Noise handshake init/resp relay (SIGNALING_RELAY)
  ✓ Relay-only media path (NAT sorunu yok)
  ✓ Uçtan uca Noise şifreleme (hub göremez)
Beta (signaling-architecture.md):
  - Derin SDP/ICE candidate müzakeresi
  - Codec negotiation + adaptif bitrate switching
  - Çağrı durumu yönetimi (ringing/busy/timeout detayları)
  - Grup çağrı (multi-party relay)
```

---

## 9. Tehdit Modeli

| # | Tehdit | Durum | Mitigasyon |
|---|--------|-------|------------|
| R-1 | Sahte hub ANNOUNCE (self-signed cert) | ✓ Kapalı | verify_chain ROOT_CA_SPKI_HASH'ya ulaşamaz → DROP (3.4) |
| R-2 | Çalınan hub cert farklı IP'de | ✓ Kapalı | SAN + address eşleşme zorunlu (3.5) |
| R-3 | ANNOUNCE replay (eski IP) | ✓ Kapalı | sequence_number monotonic + timestamp window (3.6) |
| R-4 | IP spoof ile meşru hub'ı banlatma | ✓ Kapalı | IP ban YOK; geçersiz cert sessiz DROP (3.7) |
| R-5a | Unauthorized fake RoutingRecord / route poisoning | ✓ Kapalı | AuthorizedHubSetRecord DHT STORE için ZORUNLU; `rr.hub_id` AHS içinde değilse RoutingRecord DROP. AHS RA/Intermediate-imzalı — kullanıcı/kötü hub üretemez (5.5/5.8) |
| R-5b | Authorized-but-malicious hub suppress/delay | ⚠ Kabul | Authorized hub mesajı düşürebilir/geciktirebilir; içerik okuyamaz/değiştiremez (E2E Session Model V3 AEAD). N-mesaj ACK + çok-hub failover telafi (5.7) |
| R-6 | RoutingRecord rollback (eski konum replay) | ✓ Kapalı | LWW: düşük sequence_number reddedilir (5.5) |
| R-7 | DHT mesaj içeriği dinleme (passive) | ✓ Kapalı | Hub-mesh Noise (içerik+dest_hash şifreli) (6.4) |
| R-8 | Compromised DHT node tüm replikaları okur | ✓ Kapalı | Per-recipient şifreleme: node yalnızca kendine geleni okur (6.4) |
| R-9 | Hub→hub MITM (sahte pk_noise) | ✓ Kapalı | pk_noise ANNOUNCE'tan (cert-zincirli); IKfallback verify_chain (6.3) |
| R-10 | Eclipse attack (node izolasyonu) | ⚠ Kısmi (Alpha'da pratik değil) | **Alpha küçük mesh: tüm hub'lar org-kontrollü + cert-gated (Intermediate-imzalı) → dış eclipse pratik tehdit DEĞİL; residual = authorized-but-malicious hub suppress (R-5b, zaten kabul).** Kademlia "en eski güvenilir node" eviction (4.3). Eclipse direnci **Beta scale/cross-org**'da anlamlı (node diversity sertleştirme) |
| R-11 | Hub→hub gönderen/alıcı IP görünür | ⚠ Kabul | UDP doğası; routing-level metadata kabul edilen sınır (6.5); cover traffic Beta+ |
| R-12 | Split-brain RoutingRecord (aynı scope+seq, farklı sig) | ✓ Tespit + continuity | Kriptografik anomali → o hub_id record'u KARANTİNAYA alınır + alert + log; otomatik kabul yok. **Routing DURMAZ**: Bob'un diğer AHS-authorized hub'ları üzerinden devam (multi-hub failover 5.7); total stall YOK (4.4) |
| R-13 | Route poisoning (kötü hub kendini Bob'un hub'ı ilan eder) | ✓ Kapalı | Unauthorized route poisoning mandatory AHS ile KAPALI (RA/Intermediate-imzalı control-plane allowlist, CAD-58 — KULLANICI imzalamaz): AHS dışı hub için RoutingRecord DROP; AHS DHT STORE için ZORUNLU; scoped_sequence replay korur; suspend/lisans-bitti → not_after expiry. **Residual yalnızca authorized-but-malicious hub'dan availability (suppress/delay) — R-5b** (5.5/5.8) |
| R-14 | LINK_ROUTE_MISS exploitation (hub sürekli ROUTE_MISS spam'i → client bant DoS) | ✓ Kapalı (bounded) | ROUTE_MISS **advisory'dir, sonsuz retry DEĞİL** (application-protocol §6.11.1): per-(dest_hash,hub_id) max_rebind K=2-3 → primary DEGRADED → AHS role/priority ile secondary/tertiary'ye FAILOVER + exponential backoff+jitter. Agresif blacklist yok (önce per-dest_hash; global-degraded yalnız hub çok sayıda dest_hash için ROUTE_MISS'lerse). Client-side, wire'a 0 byte. Residual: bounded gecikme (süresiz DEĞİL) |

### 9.1 Kabul Edilen Sınırlar (Açık)

```
R-5b (authorized-but-malicious suppression): Authorized hub Bob'a giden mesajı düşürebilir (availability),
  AMA okuyamaz/değiştiremez (E2E). N-mesaj-ACK + çok-hub failover telafi eder. KABUL.
  (R-5a unauthorized fake RoutingRecord mandatory AHS ile KAPALI — residual değil.)
R-10 (eclipse): Küçük mesh'te (savunma sanayi 10-50 hub) Kademlia'nın büyük-ağ varsayımları
  tam geçerli değil. Kademlia eviction + Intermediate-imzalı katılım (Sybil zorlaştırır) kısmi.
  Beta: node diversity + bucket çeşitlendirme. KABUL (Alpha mesh küçük + Intermediate-gated).
R-11 (IP metadata): Signal/Tor da çözememiştir; Kama iddiası bu değil. KABUL.
```

---

## 10. Self-Audit

| Kriter | Durum | Bölüm |
|--------|-------|-------|
| Client DHT'ye katılmaz | ✅ | 1.3, 2 |
| Client RoutingRecord okumaz (CAD-53) | ✅ | 5.1 |
| Client yalnızca dest_hash verir (hub_id bilmez) | ✅ | 2, 2.1 |
| Hub ağa katılım = kendini kanıtlar (kimse tanıtmaz) | ✅ | 3.1 |
| ANNOUNCE recursive verify_chain (iteratif değil) | ✅ | 3.4 |
| ROOT_CA_SPKI_HASH ağdan gelmez | ✅ | 3.4 |
| Zincir önce, sonra ANNOUNCE imzası (DoS guard) | ✅ | 3.4 |
| SAN + address eşleşme zorunlu | ✅ | 3.5 |
| sequence_number replay koruması | ✅ | 3.6 |
| IP ban YOK | ✅ | 3.7 |
| RoutingRecord hub-imzalı + örtük Noise IK possession (client-signed YOK) | ✅ | 5.2 |
| RoutingRecord scope (operator_id,dest_hash,hub_id) global değil; FIND_VALUE→RoutingRecordSet | ✅ | 5.8 |
| AuthorizedHubSet route poisoning (CAD-58, RA/Intermediate-imzalı, client 0 byte, KULLANICI imzalamaz) | ✅ | 5.8 |
| LINK_ROUTE_MISS → SESSION_INIT_REF rebind (ccr §35.16) | ✅ | 7.2 |
| Session Model V3 (SESSION_INIT/SESSION_INIT_RESPONSE/SESSION_DATA) referansı (SRAM + Async V2 SA?1 DEPRECATED) | ✅ | 0, 2.1, 7.2 |
| RoutingRecord ≠ DiscoveryRecordV2 (ayrı kayıt) | ✅ | 5.1 |
| RoutingRecord rollback koruması (LWW) | ✅ | 5.5 |
| Hub-mesh Noise (KK/IK/IKfallback) | ✅ | 6.2, 6.3 |
| Per-recipient şifreleme (k=5) | ✅ | 6.4 |
| Kademlia k=5 + anti-entropy SYNC | ✅ | 4.1, 4.4 |
| Senkron rendezvous relay-only (NAT sorunu yok) | ✅ | 8.1 |
| Magic domain separation (SRRR, SRAN) | ✅ | 0 |
| identity/ccr ile tutarlı (verify_chain, dest_hash, cert) | ✅ | tüm |
| Root pin hash public key gibi kullanılmaz; local root cert ile verify | ✅ | 3.4 |
| ANNOUNCE hub_id HubListRecord HubEntry ile bağlı | ✅ | 3.4 |
| ANNOUNCE hub_identity_pub == HubListRecord.hub_ed25519_pub | ✅ | 3.4 |
| hub_noise_cert aynı Hub Identity altında + keyAgreement/X25519 | ✅ | 3.4 |
| DHT STORE için AHS ana doğrulama akışında zorunlu | ✅ | 5.5/5.8 |
| Expired RoutingRecord fallback valid AHS olmadan kullanılamaz | ✅ | 5.6 |
| Active RPC envelope adı SessionModelV3Envelope | ✅ | 7.2 |

---

## 10.1 Acceptance Criteria — Freeze Gates

```
AC-RR-1: ANNOUNCE verification ROOT_CA_SPKI_HASH'i Ed25519 public key gibi ASLA kullanmaz;
         root public key lokal root bundle'dan gelir, hash ile pinlenir.
AC-RR-2: ANNOUNCE yalnızca ann.hub_id mevcut Intermediate-imzalı HubListRecord'da varsa ve
         ann.hub_identity_cert.public_key == HubEntryV3.hub_identity_cert.public_key (türetilen) ise kabul.
AC-RR-3: ANNOUNCE'taki hub_noise_cert aynı Hub Identity cert tarafından imzalı X25519 keyAgreement cert olmalı.
AC-RR-4: RoutingRecord DHT STORE geçerli AuthorizedHubSetRecord gerektirir. AHS yok → DHT STORE yok.
AC-RR-5: rr.hub_id AuthorizedHubSetRecord.hubs[] içinde listelenmeli; aksi halde DROP.
AC-RR-6: Expired cached RoutingRecord fallback hâlâ-geçerli AHS + hâlâ-authorized hub_id gerektirir;
         expired/eksik AHS fallback'i bloklar.
AC-RR-7: Aktif RPC tanımları yalnız SessionModelV3Envelope kullanır; AsyncSessionV2Envelope yasak.
AC-RR-8: Unauthorized hub route poisoning AHS ile kapalı; residual yalnız authorized-but-malicious suppress/delay.
```

---

## 11. Açık Sorular (İleri Faz)

```
OQ-R1  Eclipse sertleştirme (node diversity, bucket çeşitlendirme) — Beta
OQ-R2  Derin SDP/ICE/codec negotiation — Beta (signaling-architecture.md)
OQ-R3  Grup çağrı (multi-party relay) — Beta
OQ-R4  Cover traffic (IP metadata) — Beta+
OQ-R5  Cross-operator routing (Model B cross-certification) — Beta
OQ-R6  Hub-to-hub RPC payload byte-level test vektörleri — implementasyon fazı
```

Protokol-kritik routing/rendezvous davranışı (ANNOUNCE doğrulama, RoutingRecord yetki/format, hub-mesh Noise, relay-only senkron iskelet) bu belgede tanımlıdır. Kalan OQ'lar ileri-faz (Beta) veya implementasyon-detayıdır.

---

```
Freeze status:
  Routing authority model: frozen
  ANNOUNCE validation: frozen
  RoutingRecord/AHS validation: frozen
  Client routing boundary: frozen (CAD-53)
  Session Model V3 envelope terminology: frozen
  Remaining work: implementation test vectors + RPC encoder/decoder corpus
```

---

**Doküman sonu — Routing & Rendezvous Architecture (Hub-Mesh DHT), Alpha, Haziran 2026**
**Semantik otorite:** identity-and-trust-model.md v1.5 + ccr-format-spec.md. Bu belge onların routing/rendezvous projeksiyonudur; hiçbir kimlik/PKI kararı yeniden tasarlanmamıştır.
