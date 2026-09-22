# Kama — Application Protocol & RPC Spec

> **YAMA KAYDI — 2026-09-20:** §8 dedup satırındaki replay penceresi "512-1024" → ccr §35.8 `REPLAY_WINDOW_SIZE = 1024` (normatif replay: identity §15.5).


> **YAMA KAYDI — 2026-09-18b:** §3 "transport ciphertext gördüğü için sıkıştıramaz" kalıntısı düzeltildi (tip-bilgisi gerekçesi); "CRIME E2E'de geçersiz" → kontrollü ifade (cross-request oracle yüzeyi yok; secret ile attacker-controlled plaintext aynı context'te değil; uzunluk sızıntısı residual); sıkıştırma wire sözleşmesi → application-message-protocol.md §5 (compression_id, uncompressed_len, MAX_DECOMPRESSED, expansion, sözlük yok, invalid→drop).


> **YAMA KAYDI — 2026-09-18 [AMENDMENT-6]:** §1 kapsam netleştirmesi (bu belge = Client↔Hub Kontrol RPC, Plane 1; E2E mesaj şeması application-message-protocol.md'de; Plane 1 istemcisi kama-core'un; KS-DEMUX-1 Crypto içinde) · AMENDMENT-4 gerekçe düzeltmesi · §14 dosya transferi PoC'de (SUPERSEDED) · §6.11 DELIVER/FETCH payload [OPEN] + INIT/RESPONSE zarflarının SessionControl'e yönlendirilmesi.


**Durum:** Implementation-ready / freeze-ready Alpha.

**Kritik mimari invariant (NORMATİF):** Kama üç ayrı **düzlem** (plane) çalıştırır. App-RPC Control Plane, kurulu (established) Session Model V3 hot-path'ini (`SESSION_DATA`) **ASLA sarmalamaz**. Hot-path raw Session Message Data Plane üzerinden gider ve `transport-layer-tdd.md`'deki thin overhead (established ≈ `hub_handle4 + seq16 + ciphertext + tag16`) korunur. App-RPC framing yalnızca control-plane içindir.

Bu belge `identity-and-trust-model.md` (trust), `ccr-format-spec.md` (§35 Session Model V3 wire otoritesi), `routing-rendezvous-architecture.md` (hub-mesh + AHS), `discovery-architecture.md` (VOPRF), `transport-layer-tdd.md` (hot-path byte muhasebesi) ve `revocation-model.md` (freshness) ile %100 uyumludur.

---

## 1. Scope / Non-Goals

> **[AMENDMENT-6 — kapsam netleştirmesi, 2026-09-18]** Bu belge **Client↔Hub Kontrol RPC'sidir (Plane 1)**; adına rağmen E2E uygulama mesaj şemasını (TEXT / FILE_* / RECEIPT / PTT_* — SESSION_DATA plaintext'inin içi) TANIMLAMAZ; o şema `application-message-protocol.md`'dedir (iskelet v0.1 mevcut; sahibi Application, alan kararları [KARAR — Hayri]). Katman grafiği transport-tdd §0 INV-NEIGHBOR-1: Application'ın tek komşusu Transport; Plane 1 istemcisi **kama-core'un** (Transport tarafının) parçasıdır, Application onu ancak `connect/send/recv` üzerinden dolaylı kullanır. SESSION_INIT/RESPONSE zarfları Transport tarafından Crypto'nun SessionControl'üne yönlendirilir; **candidate seçimi (KS-DEMUX-1) Crypto'nun içindedir** — Application ve Transport candidate bilmez.

Kapsam:

```
- Üç düzlem ayrımı (App-RPC Control / Session Message Data / Hub-Mesh)
- App-RPC framing (method_id, flags, request_id, payload_len)
- Client↔Hub method_id tablosu (client→hub + hub→client server-initiated)
- CLIENT_BIND auth boundary (sk_device possession)
- Her method için request/response şeması + normatif davranış
- Mesaj yaşam döngüsü: send → store → deliver → ack → receipt
- N-mesaj ACK penceresi + idempotency + dedup
- Discovery proxy alt-operasyonları
- Onboarding / RA proxy (bootstrap-direct istisna + post-onboarding hub-proxy)
- Error code taksonomisi (u16_be, sınıf-bazlı)
- Rate-limit / quota kancaları
- Revocation / freshness etkileşimi
- File-transfer Beta-reserved notu
```

Non-goals:

```
- Session Model V3 wire byte detayı (ccr-format-spec.md §35 otoritedir)
- Transport hot-path / FEC / KCP / fuzzy controller (transport-layer-tdd.md)
- Hub-mesh RPC iç mekaniği (routing-rendezvous-architecture.md §7)
- VOPRF kriptografisi (discovery-architecture.md)
- Revocation object byte formatları (revocation-model.md)
- Admin panel / RA-Admin kontratı (admin-control-plane-tdd.md)
- P2P media / signaling (Beta)
```

---

## 2. Üç Düzlem Modeli (NORMATİF)

```
Plane 1 — App-RPC Control Plane
  Amaç: kontrol/koordinasyon (bind, mailbox, ack, discovery proxy, RA proxy, revocation fetch, presence).
  Framing: method_id(u8) + flags(u8) + request_id(u32_be) + payload_len(u32_be) + payload.
  Taşıma: kurulu Client↔Hub Noise session (transport substrate).
  request/response korelasyonu request_id ile.

Plane 2 — Session Message Data Plane
  Amaç: E2E uçtan-uca mesaj taşıma (Session Model V3).
  Mesaj tipleri: SESSION_INIT (SSI1) / SESSION_INIT_RESPONSE (SSR1) / SESSION_DATA (SSD1) / SIGNED_CONTROL (SSC1).
  Framing: ccr-format-spec.md §35 / transport-layer-tdd.md canonical bytes.
  Established hot-path: hub_handle(4B) + seq(2B) + ciphertext + AEAD tag(16B).
  Hot-path SESSION_DATA'ya app-RPC method_id/request_id/len EKLENMEZ.

Plane 3 — Hub-Mesh Plane
  Amaç: hub-to-hub routing/discovery/forward (client görmez).
  method_id (u8): ANNOUNCE / FIND_NODE / FIND_VALUE / STORE / FORWARD_MESSAGE / SIGNALING_RELAY / SYNC / PING.
  Ayrı framing, ayrı parser, ayrı state machine (routing-rendezvous-architecture.md §7).
```

> **MUTLAK KURAL:** App-RPC framing (`method_id + flags + request_id + payload_len`) yalnızca Plane 1 içindir. Established Session Model V3 hot-path `SESSION_DATA` frame'lerine **ÖN-EKLENMEZ**. Hot-path raw Plane 2 üzerinden akar; `transport-layer-tdd.md` byte muhasebesi korunur.

Üç düzlem aynı method_id sayı uzayını kullansa bile çakışmaz: farklı plane, farklı parser, farklı session. (Örn. mesh `0x05 FORWARD_MESSAGE` ile app `0x05 STORE_MESSAGE` ayrı düzlemdedir.)

---

## 3. App-RPC Framing (Plane 1)

```
AppRpcFrame {
  method_id     : u8          # §4 tablosu; bit7=0 client→hub, bit7=1 hub→client (server-initiated)
  flags         : u8          # bit0: response (1=response, 0=request); bit1: error; bit2: more (streaming); bit3-7 reserved
  request_id    : u32_be      # req/resp korelasyon + idempotency anahtarı
  payload_len   : u32_be      # ≤ MAX_APP_PAYLOAD (default 1 MiB; method-bazlı daha sıkı limit §11)
  payload       : bytes[payload_len]
}
```

> **[AMENDMENT-4] Sıkıştırma — bu alt-katmanın işidir (karşı-pointer: `transport-layer-tdd.md` §8).** [AMENDMENT-6 gerekçe düzeltmesi: "transport ciphertext görür" öncülü kanonik mimaride geçersiz — Transport plaintext görür; sıkıştırmanın app'te kalma gerekçesi TİP bilgisi (ses/video çift sıkıştırma yasağı) + kodda aktüatör yokluğu (fusion.rs:551 üretici, tüketici yok, sıkıştırma crate'i yok).]
> Sahip kararı (2026-09-17): sıkıştırma **mesaj-hazırlama** adımıdır ve **düz-metin payload'a, E2E
> mühürden ÖNCE** uygulanır; ~~transport ciphertext gördüğü için sıkıştıramaz~~ [AMENDMENT-6: transport PLAINTEXT görür; app'te kalma sebebi TİP bilgisidir — "ses/video zaten codec-sıkıştırılı" kararını yalnız Application verebilir] ve fuzzy controller
> sıkıştırma kararı vermez. Bu adım **App-RPC FRAMING DEĞİLDİR**: yukarıdaki MUTLAK KURAL aynen
> geçerlidir (framing yalnız Plane 1; hot-path SESSION_DATA sarmalanmaz). Sıkıştırma, framing'in
> değil payload hazırlığının parçası olduğundan **her iki düzleme** (Plane 1 control, Plane 2
> hot-path) uygulanabilir. TDD §8'den taşınan hükümler: `compress = zstd; metin mesaj için default
> ON` · ses/video codec'te sıkıştırılı → ÇİFT sıkıştırma YOK · ~~CRIME E2E'de geçersiz~~ [AMENDMENT-6 kontrollü ifade: HTTP/TLS-tarzı cross-request compression-oracle YÜZEYİ mevcut değildir; bununla birlikte attacker-controlled plaintext ile secret-bearing plaintext AYNI compression context'inde sıkıştırılmaz (per-message/per-batch izole context, ccr §35.6 P1-7 ile aynı); ciphertext uzunluk sızıntısı residual metadata olarak KABUL EDİLİR; küçük mesajlar sıkıştırılmaz — wire sözleşmesi application-message-protocol.md §5] · residual uzunluk-tabanlı trafik analizi = kabul edilen hub-metadata
> (padding Beta+ knob). Bu paragraf **INV-COMPRESS-1**'in yeni evidir. Karar mekanizması (hangi
> mesaj sıkıştırılır) app-rpc'nindir, transport ölçümlerine BAĞLI DEĞİLDİR.

`request_id` semantiği (NORMATİF):

```
request_id:
  - request/response korelasyonu
  - idempotent method'lar için retry idempotency anahtarı
  - kriptografik session id DEĞİLDİR
  - E2E Session Model V3 AAD'ye DAHİL EDİLMEZ (aksi açıkça belirtilmedikçe)
  - client tarafından monoton/benzersiz üretilir (session-local)
```

> Bu framing yalnızca App-RPC Control Plane içindir. Established Session Model V3 hot-path `SESSION_DATA` frame'lerine ÖN-EKLENMEZ.

flags.response/error kullanımı:

```
request : flags.response=0
response: flags.response=1; error ise flags.error=1, payload = ErrorPayload (§11)
```

---

## 4. Client↔Hub Method_id Tablosu (Plane 1)

```
Client → Hub (request/response), method_id 0x01..0x7F:

0x01 CLIENT_BIND               device cert + Noise IK possession → mailbox binding + presence
0x02 REGISTER_OR_RA_PROXY      post-onboarding RA proxy (cert renewal/enroll; hub RA adresini gizler)
0x03 HUBLIST_FETCH             HubListRecord refresh (Intermediate-imzalı)
0x04 DISCOVERY_PROXY           alt-op: VOPRF_BLIND_BATCH | LOOKUP_BATCH | PUBLISH → Discovery Server
0x05 STORE_MESSAGE             control-plane submission: SESSION_INIT/RESPONSE/SIGNED_CONTROL + offline mailbox submit
0x06 FETCH_MAILBOX             cursor-based pull (reconnect drain)
0x07 ACK_RANGE                 teslim alınan range ack → kuyruk temizle (N-mesaj ACK penceresi)
0x08 SESSION_REBIND            LINK_ROUTE_MISS recovery → yeni SESSION_INIT_REF (ccr §35.16)
0x09 REVOCATION_FETCH_SNAPSHOT (SRFQ → SRFR snapshot; revocation-model.md §6.5)
0x0A REVOCATION_FETCH_DELTA    (SRFQ → SRFR delta)
0x0B REVOCATION_STATUS         freshness sorgu
0x0C PRESENCE_KEEPALIVE        app-level presence (online-push yeterliliği; transport keepalive'den AYRI)

Hub → Client (server-initiated), method_id 0x80..0xFF (bit7=1):

0x81 DELIVER                   mailbox/reconnect/control-plane delivery notification veya non-hot-path envelope push
0x82 DELIVERY_STATUS           transport-seviye durum: "mailbox'a kondu" / "alıcı fetch etti" (opsiyonel, best-effort)
0x83 REVOCATION_NOTIFY         revocation availability → client fetch tetikle
0x84 ROUTE_MISS               Hub route bulamadı bildirimi (LINK_ROUTE_MISS)
```

> **Hot-path AYRIMI (NORMATİF):**
> `0x05 STORE_MESSAGE` ve `0x81 DELIVER` **control-plane / non-hot-path** içindir.
> Established `SESSION_DATA` hot-path bunların İÇİNE SARILMAZ — raw Plane 2 üzerinden gider.

---

## 5. CLIENT_BIND — Auth Boundary (NORMATİF)

`CLIENT_BIND`, app-RPC güvenlik omurgasıdır. Bootstrap/RA-proxy istisnaları dışında tüm app-RPC method'larından **önce** gerçekleşmelidir.

```
CLIENT_BIND:
  - Client↔Hub Noise IK handshake ile sk_device possession kanıtlar (kötü hub Bob'u onun sk_device'ı olmadan register edemez).
  - şunları bind eder (session-bound yetki):
      identity_hash
      dest_hash
      current_device_cert_hash
      device static x25519 / identity context
      mailbox binding
      presence state
  - hub: CompactDeviceCert chain'i ROOT_CA_SPKI_HASH'e kadar doğrular + revocation freshness (fail-closed).

CLIENT_BIND hub'a E2E mesaj plaintext erişimi VERMEZ.
Yalnızca o device/session için mailbox/routing operasyonlarını yetkilendirir.
```

Request/Response:

```
CLIENT_BIND request {
  device_cert_mode  : u8            # 0=ref(device_cert_hash 32B), 1=full(CompactDeviceCert 152B)
  device_cert       : bytes32 | bytes152
  presence_flags    : u8            # online/push-capable bits
}
  (Noise IK handshake transport-substrate seviyesinde; possession kanıtı handshake'tedir.)

CLIENT_BIND response {
  bind_ok           : bool_u8
  mailbox_cursor    : u64_be        # mevcut mailbox high-water (FETCH için başlangıç)
  revocation_seq    : u64_be        # hub'ın bildiği snapshot_sequence (client freshness karşılaştırması)
  error_code        : u16_be        # §11
}
```

---

## 6. Method Şemaları

Tüm integer big-endian fixed-width; len-prefixed alanlar `u16_be`/`u32_be` uzunluk taşır.

### 6.1 STORE_MESSAGE (0x05) — control-plane submission

```
STORE_MESSAGE request {
  envelope_type     : u8            # 0x01 SESSION_INIT, 0x02 SESSION_INIT_RESPONSE, 0x04 SIGNED_CONTROL
                                    #  (0x03 SESSION_DATA control-plane'de YASAK — hot-path raw Plane 2)
  recipient_dest_hash : bytes32     # TÜM control-plane tiplerinde anlamlı = bu mesajın ALICISININ dest_hash'i
                                    #  (SESSION_INIT→Bob, SESSION_INIT_RESPONSE→Alice, SIGNED_CONTROL→alıcı); route resolution
  envelope_len      : u32_be
  envelope          : bytes[envelope_len]   # ccr §35 canonical bytes; hub için OPAK E2E ciphertext
}

STORE_MESSAGE response {
  status            : u16_be        # 0x0001 ACCEPTED_QUEUED | 0x03xx route miss | 0x07xx revoked/stale
  assigned_handle   : u32_be        # SESSION_INIT / SESSION_INIT_RESPONSE kabul edilince hub_handle atanır (o legin hot-path'i için)
}
```

Normatif:

```
- STORE_MESSAGE control-plane submission / compatibility path'tir.
- SESSION_INIT / SESSION_INIT_RESPONSE / SIGNED_CONTROL gibi non-hot-path veya request/response gerektiren submitlerde kullanılır.
- MUST NOT: established SESSION_DATA hot-path'i app-RPC STORE_MESSAGE içine sarmalamaz.
- recipient_dest_hash TÜM control-plane tiplerinde alıcının dest_hash'idir; hub bununla route resolve eder (cache/DHT).
  Yalnız established SESSION_DATA (raw Plane 2) recipient_dest_hash TAŞIMAZ → hub-local hub_handle kullanılır.
- SESSION_INIT kabul edilince initiator legi için hub_handle atanır; SESSION_INIT_RESPONSE kabul edilince
  yanıtı alan hub DÖNÜŞ legi için KENDİ hub_handle'ını atar (simetrik; routing-rendezvous-architecture.md §2.1 DURUM C).
  Her leg bağımsız hub-local handle; hub_handle hub'lar arası TAŞINMAZ (routing §2.1 hub_handle locality).
- envelope hub için OPAK; hub yalnız recipient_dest_hash okur (routing), gövdeyi okuyamaz.
```

### 6.2 Established SESSION_DATA gönderimi (Plane 2 — app-RPC DEĞİL)

```
Established hot-path data app-RPC üzerinden GİTMEZ:
  raw Session Message Data Plane frame:
    hub_handle(4B) + seq(2B) + ciphertext + AEAD tag(16B)
  hub, hub_handle ile route eder (SESSION_INIT'te bound edilmiş outbound_session_route).
  recipient_dest_hash hot-path'te TAŞINMAZ.
```

### 6.3 FETCH_MAILBOX (0x06)

```
FETCH_MAILBOX request {
  cursor            : u64_be        # son alınan high-water (0 = baştan)
  max_items         : u16_be        # batch limiti
}

FETCH_MAILBOX response {
  item_count        : u16_be
  next_cursor       : u64_be
  items             : MailboxItem[item_count]
}

MailboxItem {
  item_seq          : u64_be        # mailbox-local monoton
  envelope_type     : u8            # [AMENDMENT-7, 2026-09-18] STORE_MESSAGE ile AYNI registry (0x01 INIT, 0x02 INIT_RESPONSE,
                                    #  0x04 SIGNED_CONTROL). Hub STORE'dan kopyalar, yorumlamaz. Transport bunu okuyarak
                                    #  SessionControl komutunu seçer (AcceptInit / AcceptResponse) — Crypto magic parse ETMEZ
                                    #  (KM-SEAM-TC-001 K6). Reserved/unknown → item DROP.
  envelope_len      : u32_be
  envelope          : bytes[envelope_len]   # OPAK Session Model V3 frame (hub parse etmez, blob taşır)
}
```

Normatif:

```
- FETCH_MAILBOX reconnect drain'idir (steady-state hot-path değil); offline biriken envelope'ları çeker.
- Hub, kuyruğu RELEASE etmeden ÖNCE revocation kontrol eder (revoked device fetch edemez, revocation-model §8.2).
- items OPAK blob; double-framing semantiği yok (her item length-delimited opak envelope).
```

### 6.4 ACK_RANGE (0x07)

```
ACK_RANGE request {
  ack_from          : u64_be        # mailbox item_seq range başı (dahil)
  ack_to            : u64_be        # range sonu (dahil)
}

ACK_RANGE response {
  cleared_count     : u32_be
  new_cursor        : u64_be
}
```

Normatif:

```
- ACK_RANGE acked range'i kuyruktan temizler.
- N-mesaj ACK penceresi: gönderen ACK almadan > N mesaj gönderemez (sessiz kayıp tespiti + ratchet counter DoS sınırı).
- ACK_RANGE zorunludur; DELIVERY_STATUS opsiyonel UX sinyalidir.
```

### 6.5 DISCOVERY_PROXY (0x04)

```
DISCOVERY_PROXY request {
  sub_op            : u8            # 0x01 VOPRF_BLIND_BATCH | 0x02 LOOKUP_BATCH | 0x03 PUBLISH
  payload_len       : u32_be
  payload           : bytes[payload_len]   # discovery-architecture.md formatı (OPAK; hub yalnız proxy)
}

DISCOVERY_PROXY response {
  status            : u16_be
  payload_len       : u32_be
  payload           : bytes[payload_len]   # Discovery Server cevabı (OPAK)
}
```

Normatif:

```
- Hub yalnız PROXY'dir: client'ın gerçek adresini Discovery Server'dan gizler (RA-proxy mantığı).
- Hub OPRF key TUTMAZ, telefon GÖRMEZ (blinding); payload'ı parse etmez.
- VOPRF_BLIND_BATCH/LOOKUP_BATCH/PUBLISH semantiği discovery-architecture.md'de tanımlıdır.
- Rate-limit ZORUNLU (online enumeration; §12).
```

### 6.6 REGISTER_OR_RA_PROXY (0x02)

```
REGISTER_OR_RA_PROXY request {
  ra_op             : u8            # 0x01 cert_renew | 0x02 device_enroll | 0x03 status
  payload_len       : u32_be
  payload           : bytes[payload_len]   # RA protokolü (OPAK; admin-control-plane / pki-ca-ra-hsm otoritesi)
}

REGISTER_OR_RA_PROXY response {
  status            : u16_be
  payload_len       : u32_be
  payload           : bytes[payload_len]
}
```

Normatif:

```
- Post-onboarding: client RA'ya DOĞRUDAN bağlanmaz → Client → Hub → RA (hub proxy, gerçek adres gizli).
- İlk onboarding istisnası §10'da (bootstrap-direct; hub listesi henüz yok).
- RA 3-gate (license gate + admin allowlist gate + device approval / SIM-swap mitigation gate) RA tarafında uygulanır (pki-ca-ra-hsm-tdd.md).
```

### 6.7 HUBLIST_FETCH (0x03)

```
HUBLIST_FETCH request  { current_hublist_seq : u64_be }
HUBLIST_FETCH response { status : u16_be; hublist_present : bool_u8; hublist : HubListRecord }
```

Intermediate-imzalı HubListRecord (ccr §21.5); client ROOT_CA_SPKI_HASH'e kadar doğrular.

### 6.8 SESSION_REBIND (0x08)

```
SESSION_REBIND request  { link_ref : bytes16; reason : u8 }       # LINK_ROUTE_MISS sonrası
SESSION_REBIND response { status : u16_be; new_handle : u32_be }  # yeni SESSION_INIT_REF tetikler (ccr §35.16)
```

### 6.9 REVOCATION_FETCH_SNAPSHOT (0x09) / _DELTA (0x0A) / REVOCATION_STATUS (0x0B)

```
REVOCATION_FETCH_* request  : RevocationFetchRequest (SRFQ, revocation-model.md §6.5)
REVOCATION_FETCH_* response : RevocationFetchResponse (SRFR)
REVOCATION_STATUS  response : { snapshot_seq : u64_be; delta_seq : u64_be; valid_until : u64_be }
```

### 6.10 PRESENCE_KEEPALIVE (0x0C)

```
PRESENCE_KEEPALIVE request  { presence_flags : u8 }   # push-capable / foreground / device-active
PRESENCE_KEEPALIVE response { ack : bool_u8; pending_count : u32_be }
```

Normatif: app-level presence (transport keepalive'den ayrı). Hub bununla "client online → server-initiated push yapabilirim" kararını verir.

> **[AMENDMENT-7 — 2026-09-18] `0x81 DELIVER` ve `MailboxItem` yerleşimine `envelope_type:u8` eklendi (STORE_MESSAGE registry'si); geri kalan alanlar mevcut. Exact byte vektörleri app-rpc freeze kapısı [OPEN].** Kanonik mimaride DELIVER'ı Transport tarafı (kama-core) alır; `envelope_type` INIT/RESPONSE ise zarf Crypto SessionControl'e (AcceptInit/AcceptResponse) yönlendirilir, Application'a yalnız `Ready(SessionHandle)` olayı çıkar. Payload exact layout S3/S1 ile birlikte kapanır.

### 6.11 Server-initiated: DELIVER (0x81), DELIVERY_STATUS (0x82), REVOCATION_NOTIFY (0x83), ROUTE_MISS (0x84)

```
DELIVER (0x81):
  mailbox/reconnect/control-plane delivery notification veya non-hot-path envelope push.
  Established hot-path server-initiated message DELIVER wrapper'a SARILMAZ — raw Plane 2 ile push edilir.
  payload: { envelope_type u8; envelope_len u32_be; envelope bytes[] }  (OPAK Session Model V3 frame)
  [AMENDMENT-7] envelope_type STORE_MESSAGE registry'siyle aynı; Transport'un SessionControl dispatch anahtarı (AcceptInit/AcceptResponse);
  Crypto zarf içini parse ederek tür ÇIKARMAZ. Bu alan Plane 1 kontrol-düzlemi metadata'sıdır; hot-path'e byte eklemez (LAW-7 dışı).

DELIVERY_STATUS (0x82) — OPSIYONEL/best-effort:
  payload: { ref u64_be; state u8 }   # 0x01 queued, 0x02 delivered_to_hub_b, 0x03 fetched_by_recipient
  non-repudiation SAĞLAMAZ (hub üretir). Gerçek E2E receipt = SIGNED_CONTROL (Plane 2).

REVOCATION_NOTIFY (0x83):
  payload: { latest_snapshot_seq u64_be }  # client REVOCATION_FETCH tetikler

ROUTE_MISS (0x84):
  payload: { link_ref bytes16 }  # client SESSION_REBIND başlatır (BOUNDED — §6.11.1)
```

#### 6.11.1 ROUTE_MISS Bounded Rebind Policy (NORMATİF — KAT3-P1-01)

> **İlke:** `ROUTE_MISS` **advisory'dir, sonsuz retry tetikleyicisi DEĞİL.** Client, hub başına rebind denemelerini SINIRLAMAK ve K başarısızlıktan sonra sıradaki authorized hub'a FAILOVER etmek ZORUNDADIR. Aksi halde kötü niyetli primary hub, sürekli ROUTE_MISS ile client'ın kıt hostile bandını (her rebind = SESSION_INIT_REF 149B) süresiz yakabilir (availability DoS). Bu policy client-side'dır — **wire'a 0 byte ekler.**

```
per (dest_hash, hub_id):
  max_rebind_attempts = 2..3
  K ROUTE_MISS sonrası:
    - primary hub = DEGRADED (bu dest_hash için) işaretlenir
    - client aynı hub'a tekrar SESSION_INIT_REF BASMAZ
    - AuthorizedHubSet role/priority sırasına göre secondary/tertiary hub'a FAILOVER (routing §5.7)
  Backoff:
    - degraded hub için exponential backoff + jitter (aynı hub'a erken dönüşü engeller)

Degraded kapsamı (agresif blacklist YAPMA):
  - önce PER-DEST_HASH local degraded ("bu route gerçekten yoktu" olabilir — hub'ı komple gömme)
  - GLOBAL-DEGRADED yalnızca aynı hub BİRÇOK farklı dest_hash için tekrar tekrar ROUTE_MISS dönüyorsa
    (anomali sayacı) → client o hub'ın primary tercihini GEÇİCİ döndürür (local, client-side)
```
> Transport EMERGENCY_FORWARD_REDUNDANCY (transport-layer-tdd §5.6) ile birlikte: kötü hub tespit → başka authorized hub + forward-redundancy mod. Bu, routing R-14'ü "residual süresiz gecikme"den "**bounded gecikme + otomatik failover**"a çevirir.

---

## 7. Mesaj Yaşam Döngüsü (send → store → deliver → ack → receipt)

```
1. SEND
   - Mesaj SESSION_INIT (SSI1) ise:
       Alice → Hub A: STORE_MESSAGE { envelope_type=SESSION_INIT, recipient_dest_hash, envelope }
       Hub A: parse + revocation freshness (fail-closed) + AHS doğrula.
       Hub A SESSION_INIT'ten recipient_dest_hash OKUR ve route resolve eder.
       Hub A hub_handle atar → STORE_MESSAGE response { ACCEPTED_QUEUED, assigned_handle }.
   - Mesaj established SESSION_DATA (SSD1) ise:
       Alice mevcut session route binding / hub_handle kullanır.
       raw Plane 2 frame: hub_handle4 + seq16 + ct + tag16 (app-RPC YOK).
       Hot-path'te recipient_dest_hash TAŞINMAZ.

2. RESOLVE (hub-mesh)
   Hub A → mesh: FIND_VALUE(routing_key) → AuthorizedHubSet + RoutingRecordSet → Hub B (authorized).
   Hub A → Hub B: FORWARD_MESSAGE { envelope }  (mesh 0x05, Noise KK).

3. STORE / DELIVER
   Hub B:
     Bob offline → mailbox store (TTL 7 gün).
     Bob online  → server-initiated push:
        SESSION_DATA hot-path → raw Plane 2 frame (Bob'un inbound hub_handle'ı).
        non-hot-path/notification → DELIVER (0x81).

4. DELIVER (alıcı)
   Bob online  → raw Plane 2 frame alır.
   Bob reconnect → FETCH_MAILBOX (cursor) ile drain.
   Bob: Session Model V3 decrypt (E2E; hub okuyamaz).

5. ACK
   Bob → Hub B: ACK_RANGE → kuyruk temizle.
   (opsiyonel) Hub B → Alice yönüne DELIVERY_STATUS (0x82, best-effort).
   N-mesaj ACK penceresi: Alice ACK almadan > N mesaj gönderemez.

6. RECEIPT (opsiyonel, E2E non-repudiation)
   Bob → SIGNED_CONTROL (SSC1) → STORE_MESSAGE → ters yol (Hub B → mesh → Hub A) → Alice.
   Bu gerçek kriptografik receipt'tir; hub gövdeyi okuyamaz, DELIVERY_STATUS'tan farklıdır.
```

State machine (özet):

```
PENDING_SUBMIT → (STORE_MESSAGE ACCEPTED_QUEUED) → QUEUED_AT_HUB_B
QUEUED_AT_HUB_B → (online push | FETCH_MAILBOX) → DELIVERED_TO_CLIENT
DELIVERED_TO_CLIENT → (ACK_RANGE) → ACKED → (kuyruktan temizlenir)
ROUTE_MISS herhangi aşamada → SESSION_REBIND → yeni SESSION_INIT_REF
```

---

## 8. N-Mesaj ACK Penceresi + Idempotency + Dedup

```
N-mesaj ACK penceresi:
  Alice, ACK almadan en fazla N established mesaj gönderebilir (default N = profile-bağımlı, lowband'de küçük).
  Pencere dolarsa Alice gönderimi durdurur → 0x05xx ACK_WINDOW_EXCEEDED (kontrol akışı).
  Çözdüğü 3 sorun: ratchet counter DoS sınırı + sessiz kayıp tespiti + MAX_SKIP doğal sınırı.

Idempotency:
  Control-plane: aynı request_id ile retry idempotent (hub aynı response'u döner; çift işlem yok).
  Data-plane: STORE_MESSAGE / SESSION_DATA message_counter + (handle/link) ile dedup → hostile retry güvenli.

Dedup / replay:
  Alıcı: message_counter (monoton) + sliding-window bitmap (~~512-1024~~ 1024 = ccr §35.8 `REPLAY_WINDOW_SIZE`; normatif replay: identity §15.5) + timestamp penceresi (max 7 gün, S&F TTL).
  İlk mesaj ATOMIK counter-check (saldırgan ilk mesajı N kez gönderse de counter=1 bir kez işlenir).
  (replay detayları transport-layer-tdd.md / identity-and-trust-model.md ile uyumlu.)
```

---

## 9. Discovery Proxy Alt-Operasyonları

```
DISCOVERY_PROXY sub_op:
  0x01 VOPRF_BLIND_BATCH  : Alice blinded private_input_1..N gönderir → Discovery Server OPRF eval → evaluated_1..N.
  0x02 LOOKUP_BATCH       : Alice unblind → discovery_lookup_key_1..N → DiscoveryRecord + full CompactDeviceCert(152B) döner.
  0x03 PUBLISH            : Bob kendi DiscoveryRecordV2'sini sk_user_identity ile imzalayıp publish eder (Seçenek B).

NORMATİF:
  - Tüm sub_op hub-PROXY üzerinden Discovery Server'a gider (Client → Hub → Discovery).
  - Hub OPRF key tutmaz, telefon görmez (blinding), payload'ı parse etmez.
  - LOOKUP_BATCH tam CompactDeviceCert döndürür → Alice ilk SESSION_INIT'te cert'i inline koyabilir (cert_mode=1).
  - Lazy discovery YOK: toplu çekim + cache (discovery-architecture.md).
  - Rate-limit ZORUNLU (§12).
```

---

## 10. Onboarding / RA Proxy

```
İlk onboarding (bootstrap-direct İSTİSNA):
  - App bundle'da: ROOT_CA_SPKI_HASH + ROFTCORE_ONBOARDING_ENDPOINT (well-known, sabit).
  - Client → RA: DOĞRUDAN (hub listesi henüz YOK → hub proxy yapılamaz, tavuk-yumurta çözümü).
  - Client org_id iddia eder (kurum kodu); allowlist bypass YOK (Admin Server allowlist'te yoksa RA reddeder).
  - RA: allowlist + license gate → Intermediate CSR → imzalı cert.
  - RA ilk bundle döner: Identity/Device cert + HubListRecord (Intermediate-imzalı IP SAN) + Discovery endpoint.

Post-onboarding (hub-proxy DAİMA):
  - Client artık hub'ları bilir → HubListRecord'dan hub seç → CLIENT_BIND.
  - Tüm RA temasları REGISTER_OR_RA_PROXY (0x02) ile hub-proxy üzerinden (gerçek adres gizli).
  - well-known onboarding endpoint'e DÖNÜLMEZ (kayıtlı client) → endpoint down/blocked mevcut kullanıcıları etkilemez.
```

---

## 11. Error Code Taksonomisi (u16_be, sınıf = high byte)

```
ErrorPayload {
  error_code   : u16_be
  detail_len   : u16_be
  detail       : bytes[detail_len]   # opsiyonel insan-okur/debug (güvenlik kararı detail'e dayanmaz)
}
```

```
0x00xx  OK / info
  0x0000 OK
  0x0001 ACCEPTED_QUEUED
  0x0002 UP_TO_DATE

0x01xx  Auth / trust
  0x0101 BIND_REQUIRED
  0x0102 CERT_INVALID
  0x0103 CERT_CHAIN_FAIL
  0x0104 NOT_AUTHORIZED
  0x0105 POSSESSION_PROOF_FAIL

0x02xx  Rate / quota
  0x0201 RATE_LIMITED
  0x0202 QUOTA_EXCEEDED
  0x0203 BATCH_TOO_LARGE

0x03xx  Routing / delivery
  0x0301 ROUTE_MISS
  0x0302 RECIPIENT_UNKNOWN
  0x0303 MAILBOX_FULL
  0x0304 NO_AUTHORIZED_HUBSET

0x04xx  Protocol / parse
  0x0401 BAD_MAGIC
  0x0402 BAD_VERSION
  0x0403 MALFORMED
  0x0404 OVERSIZE
  0x0405 UNKNOWN_METHOD
  0x0406 WRONG_PLANE          # hot-path frame app-RPC'de veya tersi

0x05xx  State / sequence
  0x0501 REPLAY
  0x0502 ROLLBACK
  0x0503 DELTA_GAP
  0x0504 OUT_OF_WINDOW
  0x0505 ACK_WINDOW_EXCEEDED

0x06xx  License / policy
  0x0601 LICENSE_EXPIRED
  0x0602 SEAT_LIMIT
  0x0603 FEATURE_DISABLED      # örn. file-transfer Alpha'da kapalı

0x07xx  Revocation / freshness
  0x0701 REVOCATION_STALE      # fail-closed (yeni trust kararı)
  0x0702 SUBJECT_REVOKED

0x0Fxx  Internal
  0x0F01 UNAVAILABLE
  0x0F02 DISCOVERY_DOWN
  0x0F03 RA_UNREACHABLE
```

---

## 12. Rate-Limit / Quota Kancaları

```
- DISCOVERY_PROXY (online enumeration): per-user + per-org quota + anomali tespiti (Signal modeli); sabit batch B=256 padding.
- STORE_MESSAGE (spam/flood): per-device gönderim hızı + N-mesaj ACK penceresi doğal sınır.
- REGISTER_OR_RA_PROXY (onboarding abuse): per-org seat limit RA'da (cryptographic enforcement); hub-seviye soft rate-limit.
- FETCH_MAILBOX: per-device fetch hızı.
- Aşım → 0x02xx error sınıfı.
```

---

## 13. Revocation / Freshness Etkileşimi

```
Yeni trust kararı = fresh revocation state gerektirir (fail-closed; revocation-model.md §7):
  - CLIENT_BIND: hub device cert revocation kontrol eder.
  - STORE_MESSAGE (SESSION_INIT): sender/recipient cert revocation kontrol.
  - FETCH_MAILBOX: hub kuyruğu RELEASE etmeden önce revocation kontrol (revoked device fetch edemez).
  - DISCOVERY_PROXY (LOOKUP): revoked device cert reject; PeerTrustCache invalidate.

Hot-path (established SESSION_DATA):
  her mesajda revocation kontrol ETMEZ (imzasız/AEAD-authenticated thin hot-path).
  kontrol yeni session/trust kararında + cache refresh (REVOCATION_NOTIFY/STATUS) anında yapılır.
```

---

## 14. File-Transfer Beta-Reserved Notu

> **[SUPERSEDED — 2026-09-18, PoC kapsamı]** Dosya transferi PoC KAPSAMINDADIR: `FILE_BEGIN / FILE_CHUNK / FILE_END / FILE_ABORT` mesaj tipleri `application-message-protocol.md`'de tanımlanır (Application sahibi: transfer_id, offset, size, bütün-dosya hash, birleştirme; **chunk retry/ACK/FEC/kayıp tespiti/per-chunk MAC YASAK** — hepsi Transport ve Crypto'nun). Chunk boyutu transport bütçesine göre HESAPLANMAZ: Application plaintext birimi verir, Transport böler/batch'ler. Aşağıdaki Beta notu tarihseldir.

```
- Alpha PoC file transfer IMPLEMENTE ETMEZ (PoC = mesajlaşma direnci, Side-by-Side ölçüm).
- File-transfer RPC schema RESERVED + feature-flagged Beta.
- Feature disabled iken file-transfer payload → 0x0603 FEATURE_DISABLED reject.
- Beta: chunked bulk profili (SESSION_DATA/BULK framing), Reed-Solomon FEC, authenticated shards FEC-decode'dan ÖNCE,
  resumable transfer cursor; her chunk/bulk session start ve resume'da revocation kontrol.
```

---

## 15. Threat Model

```
| ID        | Tehdit                                                  | Statü               | Mitigasyon |
|-----------|---------------------------------------------------------|---------------------|------------|
| R-APP-1   | App-RPC framing hot-path'i şişirir (lowband bozulur)    | Closed              | 3-düzlem ayrımı; SESSION_DATA raw Plane 2; STORE/DELIVER control-only (§2). |
| R-APP-2   | Kötü hub başka device adına bind eder                   | Closed              | CLIENT_BIND Noise IK + sk_device possession; possession yoksa 0x0105. |
| R-APP-3   | Hub E2E içeriği okur                                     | Closed              | envelope OPAK; hub yalnız SESSION_INIT'te recipient_dest_hash görür. |
| R-APP-4   | Hostile retry duplicate mesaj/session                   | Closed              | request_id idempotency + message_counter dedup + atomic ilk-mesaj. |
| R-APP-5   | Revoked device mailbox fetch eder                       | Closed              | FETCH_MAILBOX öncesi revocation kontrol (§13). |
| R-APP-6   | Discovery online enumeration                             | Mitigated           | hub-proxy + rate-limit + sabit batch padding (§12). |
| R-APP-7   | Plane confusion (hot-path frame app-RPC parser'ına)     | Closed              | 0x0406 WRONG_PLANE; ayrı parser/state machine. |
| R-APP-8   | DELIVERY_STATUS non-repudiation sanılır                 | Closed              | DELIVERY_STATUS best-effort; gerçek receipt = SIGNED_CONTROL (E2E). |
| R-APP-9   | ACK olmadan sınırsız gönderim (DoS/counter)             | Closed              | N-mesaj ACK penceresi → 0x0505. |
| R-APP-10  | Onboarding endpoint sansürü mevcut kullanıcıyı keser    | Closed              | kayıtlı client hub-proxy kullanır; well-known endpoint yalnız yeni kayıt. |
```

---

## 16. Self-Audit

```
| Kontrol                                                              | Statü | Bölüm |
|----------------------------------------------------------------------|-------|-------|
| App-RPC control plane SESSION_DATA hot-path'i sarmalamaz             | ✅    | §2,§6.2 |
| Established hot-path = hub_handle4 + seq16 + ct + tag16 (raw Plane 2) | ✅    | §6.2  |
| App-plane namespace ayrı (bit7 server-initiated)                     | ✅    | §4    |
| Mesh namespace ayrı parser/state machine                             | ✅    | §2    |
| STORE_MESSAGE control-only (SESSION_DATA YASAK)                      | ✅    | §6.1  |
| DELIVER control/non-hot-path; hot-path raw push                      | ✅    | §6.11 |
| CLIENT_BIND sk_device possession; E2E plaintext erişimi YOK         | ✅    | §5    |
| request_id idempotency anahtarı, crypto session id DEĞİL, AAD'de YOK | ✅    | §3    |
| SEND: SESSION_INIT dest_hash okur; SESSION_DATA hot-path dest_hash YOK| ✅    | §7    |
| ACK_RANGE zorunlu; DELIVERY_STATUS opsiyonel                         | ✅    | §4,§6.4 |
| E2E receipt = SIGNED_CONTROL (DELIVERY_STATUS'tan ayrı)             | ✅    | §6.11,§7 |
| PRESENCE_KEEPALIVE app-presence (transport keepalive'den ayrı)      | ✅    | §6.10 |
| Discovery proxy: hub OPRF key tutmaz, telefon görmez                 | ✅    | §9    |
| Bootstrap-direct RA istisnası + post-onboarding hub-proxy           | ✅    | §10   |
| N-mesaj ACK penceresi → ACK_WINDOW_EXCEEDED                          | ✅    | §8    |
| Revocation freshness fail-closed (yeni trust noktaları)             | ✅    | §13   |
| File-transfer Beta-reserved / FEATURE_DISABLED                      | ✅    | §14   |
| Error taksonomisi u16_be sınıf-bazlı                                | ✅    | §11   |
```

---

## 17. Acceptance Criteria

```
AC-APP-1:  App-RPC framing yalnız Plane 1; established SESSION_DATA hot-path'e ÖN-EKLENMEZ.

AC-APP-2:  Established hot-path raw Session Message Data Plane (hub_handle4+seq16+ct+tag16) ile gider;
           transport-layer-tdd.md byte muhasebesi korunur.

AC-APP-3:  Client→hub method_id 0x01..0x7F, server-initiated 0x80..0xFF; mesh namespace ayrıdır.

AC-APP-4:  STORE_MESSAGE yalnız SESSION_INIT/RESPONSE/SIGNED_CONTROL + offline submit taşır;
           SESSION_DATA hot-path'i sarmalaması REDDEDİLİR (0x0406 WRONG_PLANE).

AC-APP-5:  CLIENT_BIND, app-RPC method'larından önce sk_device possession (Noise IK) kanıtlar;
           hub'a E2E plaintext erişimi vermez.

AC-APP-6:  request_id req/resp korelasyonu + idempotency anahtarıdır; E2E AAD'ye dahil edilmez.

AC-APP-7:  SESSION_INIT recipient_dest_hash taşır ve hub route resolve eder;
           established SESSION_DATA hot-path'te recipient_dest_hash YOKTUR.

AC-APP-8:  ACK_RANGE zorunlu; N-mesaj ACK penceresi aşımı 0x0505 döner.

AC-APP-9:  E2E delivery receipt SIGNED_CONTROL (Plane 2) ile sağlanır; DELIVERY_STATUS best-effort UX sinyalidir.

AC-APP-10: DISCOVERY_PROXY hub-proxy üzerinden çalışır; hub OPRF key tutmaz, telefonu görmez.

AC-APP-11: Post-onboarding tüm RA temasları hub-proxy; yalnız ilk onboarding bootstrap-direct.

AC-APP-12: Yeni trust kararları (bind/init/fetch/lookup) revocation freshness fail-closed kontrol eder.

AC-APP-13: File-transfer Alpha'da disabled; payload 0x0603 FEATURE_DISABLED ile reddedilir.
```

---

## 18. Forbidden Wording / Cleanup

```
YASAK (active):
  - "established SESSION_DATA STORE_MESSAGE içine sarılır"
  - "app-RPC framing hot-path'e eklenir"
  - "hub mesaj gövdesini okur"
  - "DELIVERY_STATUS non-repudiation sağlar"
  - "client RA'ya doğrudan bağlanır" (post-onboarding)
  - "PRESENCE_KEEPALIVE = transport keepalive"
  - "mesh method_id app-plane ile aynı parser"

İZİNLİ:
  - SESSION_DATA raw Plane 2 (hub_handle4+seq16+ct+tag16)
  - STORE_MESSAGE = control-plane submission (SESSION_INIT/RESPONSE/SIGNED_CONTROL)
  - SIGNED_CONTROL = E2E receipt
  - hub-proxy RA/discovery erişimi
  - bootstrap-direct yalnız ilk onboarding
```

---

## 19. Final Verdict

```
Final Status:
  Application Protocol & RPC Spec Alpha için freeze-ready.

Frozen (kilitli):
  - 3-düzlem ayrımı (App-RPC Control / Session Message Data / Hub-Mesh)
  - App-RPC framing yalnız Plane 1; hot-path SESSION_DATA raw Plane 2 (byte muhasebesi korunur)
  - method_id tablosu (client 0x01-0x0C, server-initiated 0x81-0x84) ayrı namespace
  - CLIENT_BIND auth boundary (sk_device possession; E2E erişimi yok)
  - STORE_MESSAGE/DELIVER control-only; SESSION_DATA hot-path sarılmaz
  - mesaj yaşam döngüsü send→store→deliver→ack→receipt + N-mesaj ACK penceresi
  - error taksonomisi (u16_be, sınıf-bazlı)
  - discovery proxy / RA proxy / revocation fetch hub-proxy
  - E2E receipt = SIGNED_CONTROL; DELIVERY_STATUS opsiyonel

Deferred (ertelenen):
  - file-transfer implementasyonu (Beta; Alpha'da FEATURE_DISABLED)
  - signaling/media call setup (Beta)
  - PUSH delivery-attestation (Beta operasyonel)
```

Bu belge yazıldıktan sonra `transport-layer-tdd.md` (hot-path byte muhasebesi), `ccr-format-spec.md` §35 (Session Model V3), `routing-rendezvous-architecture.md` (mesh + AHS), `discovery-architecture.md` (VOPRF proxy) ve `revocation-model.md` (freshness) ile çelişmez. Kritik invariant korunur: App-RPC Control Plane established SESSION_DATA hot-path'ini sarmalamaz.
