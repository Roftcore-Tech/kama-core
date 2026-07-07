# SafeRoom — Transport Layer TDD (Adaptive Hostile-Aware Transport)

> **Tasarım ilkesi (TLS 1.3 dersi, NORMATİF):** Handshake sonrası bağ **byte'larda değil, anahtarda** kurulur. TLS established record'unda `connection_id`/`session_id`/`handshake_hash`/`cert_hash`/imza **yoktur** — yalnızca 16B AEAD tag vardır; tag handshake'ten türeyen key ile üretildiği için bağ kriptografiktir. SafeRoom aynı modeli benimser: established mesajda **yalnızca demux handle + seq + ciphertext + AEAD tag** bulunur. Per-mesaj imza, link_id, cert-ref, routing-info, length **TAŞINMAZ.**
> **Bağımlılık:** identity-and-trust-model.md (trust), ccr-format-spec.md (**§35 Session Model V3 — SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL canonical app-envelope byte'ları ve wire/session canonical semantiği orada**), routing-rendezvous-architecture.md (CAD-53/56/57), discovery-architecture.md.
> **Çekirdek:** SafeRoom transport "KCP over UDP" değil, "QUIC clone" değildir. Async hot-path = **UDP üzerinde E2E-authenticated message envelope** + hub-routing handle. FEC/ARQ + SRC seçili modların araçlarıdır (adaptif motor koşula göre seçer), her mesajı saran zorunlu zarf değildir. **KCP çıkarıldı** (§4). QUIC = zorunlu benchmark referansı, bağımlılık değil (§4.1).

---

## 0. Katman Modeli (NORMATİF — omurga)

```
L1  IP/UDP                       UDP/IPv4 = 28B, UDP/IPv6 = 48B base
L2  Hub routing (cleartext)      hub_handle (4B) — hub okur+route eder; E2E DEĞİL
L3  E2E Session Model V3 envelope SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL
                                 (SESSION_DATA: seq16 2B + ciphertext + AEAD tag16) — hub OKUYAMAZ
L4  Session crypto               Session Model V3 key schedule (ccr §35): INIT ss_e+ss_s authenticated
                                 one-way; RESPONSE responder ephemeral upgrade; established epoch/dir/
                                 counter-bound AEAD (split() — SafeRoom-specific Noise-inspired, standart
                                 Noise N/IK/KK transcript DEĞİL)
L5  Adaptive Recovery            FEC / ARQ / hybrid / none — fuzzy controller ölçülen bw/loss/jitter/queue/
                                 delivered-rate-confidence/path-health'e göre seçer. Transport application-role-blind.
```
> L2 (handle) hub'ın demux/route bilgisi; L3 (E2E envelope) içerik. Established mesaj L5'in (FEC/ARQ/outer-AEAD) zarfına **default girmez** — L3'teki inner AEAD tag zaten authentication'dır.

### 0.1 Kripto Katmanlama (NORMATİF — KİLİTLİ, identity §11/§14 + ccr §35)

Şifreleme katmanları AYRI ve hepsi zaten kararlı; taşıma güncellemeleri bunları DEĞİŞTİRMEZ:

```
Client↔Hub Noise handshake  : IK (yeni kayıt, sk_device possession) / KK (kayıtlı, 0-RTT).
                              Link auth + dest_hash'i Noise prologue ClientHint'te GİZLER (AEAD, key=ECDH(
                              client_ephemeral, hub_noise_pk)) + hub_handle atar. Konuşma başına 1 kez (setup).
Hot-path SESSION_DATA        : THIN — hub_handle cleartext (opaque, PRF-türevi, correlation-resistant, dest_hash'ten
                              türemez) + E2E envelope (Session Model V3 AEAD: ss_e+ss_s, Noise-inspired one-way).
                              Per-mesaj OUTER Noise AEAD YOK. dest_hash (kimlik-bağlı) hot-path'te TAŞINMAZ.
Segment-block/Control framing (Pipeline C/D): OUTER AEAD (Noise transport key) per-shard — YALNIZCA burada (FEC shard auth).
Hub↔Hub mesh                 : Noise KK (datacenter; routing §6).
```
> **Neden hot-path'te outer Noise yok ama dest_hash yine gizli:** Kimlik-bağlı alan (dest_hash) setup'ta ClientHint ile gizlenir; hot-path'te yalnız opaque hub_handle görünür (kimliğe bağlanamaz). Per-mesaj outer Noise 0.025 kbps'de +16B tag/mesaj = kabul edilemez maliyet; E2E envelope zaten içeriği korur. Bu, "thin hot-path" (TLS-dersi) ile "dest_hash gizli" (Noise ClientHint) kararlarının birlikte tutarlı halidir — çelişki yok, KİLİTLİ.

---

## 1. Mesaj Taksonomisi — Session Model V3 aktif mesajları

> **Canonical app-envelope byte layout'u ccr §35'tedir;** burada transport görünümü + handle + accounting. Dört aktif Session Model V3 mesaj tipi:

```
1. SESSION_INIT
   - zero-RTT N-türevi authenticated one-way bootstrap; Bob online olmak zorunda DEĞİL
   - recipient_dest_hash routing için YALNIZCA bu mesajda görünür; early_data taşıyabilir
   - konuşma/session başlatma mesajı; hub burada session + hub_handle atar, mailbox açar

2. SESSION_INIT_RESPONSE
   - Bob online olunca üretilen async FS-upgrade / bidirectional activation mesajı
   - ilk async mesajı okumak için ZORUNLU DEĞİL; epoch-0 init_root → epoch-1 session_root geçişi
   - hot-path mesajı DEĞİL

3. SESSION_DATA
   - established hot-path; L2 hub_handle + L3 seq16+ciphertext+tag16
   - routing info / cert / signature / explicit length TAŞIMAZ

4. SIGNED_CONTROL
   - SESSION_DATA semantiği + nadir Ed25519 non-repudiation imza (64B)
   - kritik komut / teslim makbuzu / audit-control; normal chat/file hot-path DEĞİL
```
> **NORMATİF:** SESSION_INIT_RESPONSE aktif bir Session Model V3 mesaj tipidir, ama ilk async mesaj teslim/okunmadan önce GEREKLİ DEĞİLDİR; hot-path data formatı da değildir.

---

## 2. SESSION_DATA Hot-Path Wire (TEK form — NORMATİF)

```
┌─ L2 transport (hub okur) ─┐┌─ L3 E2E app-envelope (hub okuyamaz; ccr §35) ─┐
│ hub_handle : 4B           ││ seq : 2B │ ciphertext : N │ AEAD tag : 16B     │
└───────────────────────────┘└───────────────────────────────────────────────┘
  length IMPLICIT (UDP datagram sınırı — wire'da 0B)

Overhead = handle 4B + seq 2B + tag 16B = 22B + payload
UDP/IPv4 = 22 + 28 = 50B + payload   |   UDP/IPv6 = 22 + 48 = 70B + payload
```

**Alan gerekçeleri (TLS gözüyle):**
```
hub_handle 4B : hub-local OPAQUE L2 routing/demux handle (authentication DEĞİL), SESSION_INIT'te atanır.
                dest_hash/session identity'den DOĞRUDAN türemez. NAT-robust. unknown handle →
                cheap-drop / LINK_ROUTE_MISS (AEAD/session key authentication sınırı kalır).
                → TLS connection_id taşımaz (TCP 5-tuple); biz store-and-forward + offline mailbox için.
                Allocation (collision-safe):
                  hub_handle = Trunc32(PRF(hub_secret, internal_session_id || hub_handle_epoch || allocation_attempt))
                  allocation_attempt 0'dan başlar; hub active table'da collision kontrol → attempt++;
                  attempt > MAX_HANDLE_ALLOC_ATTEMPTS (=8) → epoch rollover veya 8B handle profili.
seq 2B        : nonce resync + gap/replay. UDP+kayıpta zorunlu (TLS 0B taşır çünkü TCP in-order).
                Tam transport counter receiver'da seq ile reconstruct edilir (Section 6).
length 0B     : UDP datagram = mesaj sınırı (TLS 2B taşır çünkü TCP byte-stream).
AEAD tag 16B  : ChaCha20-Poly1305; tüm authenticity+integrity+confidentiality. Bağ anahtarda.
imza 0B       : per-mesaj imza YOK (Ed25519 yalnızca cert'i imzalar, discovery'de 1 kez doğrulanır;
                N-türevi schedule ss_s=DH(sender_static, recipient_static) ile authenticate eder).
```

> **NORMATİF:** Established SESSION_DATA'ya outer per-hop AEAD tag, FEC shard header, full transport prefix, session_id, routing-info, imza **EKLENMEZ** (default). Authentication = inner E2E AEAD; demux = hub_handle; replay/order = seq.

### 2.1 Batching ↔ implicit-length çelişkisi (çözüm — VARINT YOK)
Düşük bantta batching için: **bir batch tek AEAD birimi olarak şifrelenir**; alt-mesaj çerçeveleme plaintext'te (şifrelemeden önce). Dış length implicit (datagram). **Canonical Alpha batch framing fixed-width deterministik alanlar kullanır — LEB128/protobuf varint DEĞİL** (ccr §35 varint yasağı; şifreli payload içinde bile implementer drift yaratır).
```
batch_count : u8                # >= 1
tekrar:
  msg_len   : u16_be            # > 0
  msg_bytes : bytes[msg_len]
Kısıt: toplam plaintext <= path_mtu_payload_limit; decrypt sonrası trailing bytes → reject.
```
> Yalnızca TEK canonical Alpha batch formatı aktiftir; alternatif kodlamalar Beta/non-normative ve Alpha parser tarafından KABUL EDİLMEZ. Tek seq16, tek tag.

---

## 3. Zero-RTT Handshake (SESSION_INIT)

Discovery Alice'e Bob'un static X25519 + Ed25519'unu verir → handshake round-trip YOK.
```
N-TÜREVİ TEK-YÖN authenticated schedule (interaktif KK/IK DEĞİL — tek mesaj, Bob offline olabilir):
  ss_e = DH(sender_ephemeral, recipient_static)   → forward secrecy
  ss_s = DH(sender_static,    recipient_static)   → sender auth (possession; per-mesaj imza yok)
  CANONICAL app-envelope byte'ları (ccr §35.3 — OTORİTER):
    SESSION_INIT_REF            = 121B + early_data   (UDP/IPv4 149B)
    SESSION_INIT_INLINE_COMPACT = 241B + early_data   (UDP/IPv4 269B)
  NON-NORMATIVE Noise-core sezgisi (canonical DEĞİL): cert-ref ≈ ephemeral32 + tag16 ~48B [K-benzeri],
    cert-inline ≈ core + 152B CompactDeviceCert [X-benzeri]. "~48B core" canonical wire/app-envelope
    boyutu olarak SUNULMAZ — canonical boyutlar ccr §35'ten gelir.
Transport (handshake sonrası):  split() → k_send/k_recv; her mesaj = payload + 16B tag (nonce implicit counter)
```
> SESSION_INIT_RESPONSE = Bob online olunca kendi ephemeral'ını ekleyen asenkron FS-upgrade (epoch-0 → epoch-1); ilk mesaj RESPONSE olmadan teslim+okunur. Senkron PTT/voice (interaktif IK/KK) = Beta, ayrı konu.
> early_data (ilk gerçek mesaj) handshake paketinde gider → konuşma başlatma maliyeti tek pakette amortize. Bob offline olabilir (hub mailbox'a koyar); Bob online olunca INIT'i işler, session aktive, handle teyit.

---

## 4. Substrate Rolleri (NORMATİF)
```
TLS:   connection-merkezli reliable byte-stream. RA/onboarding/admin/API/mTLS/gRPC için mature/default.
       ASYNC hot-path'in yerine KONMAZ. "TLS kötü" DEĞİL — farklı problem (stream vs store-forward envelope).
QUIC:  as-is hub↔hub/RA/onboarding/normal-internet MAY (iyi ağda QUIC mükemmel — orada onu KULLAN).
       Streams hot-path default OLMAZ. QUIC fork = NO-GO (TLS 1.3'ü söküp reliable-transport'u ayıklamak =
       standartlık + library olgunluğu kaybı; fork karmaşası maliyeti kazancı aşar). QUIC = BENCHMARK REFERANSI
       (§4.1 zorunlu), bağımlılık değil.
KCP:   ÇIKARILDI (Haziran 2026). Ne single-datagram ne segment-block framing'de. Gerekçe: (a) KCP C/C++ tabanlı → Rust hub/client core ile
       FFI/unsafe mismatch (SR-AUDIT-001 memory-safety doktrinine aykırı); sağlıklı fork-edilebilir Rust tabanı yok.
       (b) KCP'den geriye kalan tek işlev (segment-block retransmit/window) zaten adaptif transport motorunda var. Tarihsel not:
       §18 Deprecated (MUST-NOT değil — "kullanılmadı, kalan işlev adaptif motorda").
ENGINE: TEK custom **adaptif transport motoru** (Rust-native, Tokio; C++/FFI bağımlılığı YOK). Motor İKİ recovery
        mekanizmasını da İÇERİR — ARQ (windowed/SACK/selective-retransmit) VE FEC (Reed-Solomon erasure). Hangisinin
        (veya karmasının, veya hiçbirinin) kullanılacağı **fuzzy controller tarafından ölçülen bw/jitter/loss/queue'ya
        göre seçilir** (§5). Ne ARQ-default ne FEC-default. Segment-block framing'de aynı motor daha büyük pencere +
        block-FEC parametreleriyle çalışır.
```

> **SEPARATION OF CONCERNS (NORMATİF):** Transport motoru "hot-path" / "bulk" / "SESSION_DATA" gibi **uygulama-katmanı kavramlarını BİLMEZ.** Girdisi: gönderilecek baytlar + ölçülen ağ durumu (bw/jitter/loss/queue) + payload boyutu (framing için). Çıktısı: rate + recovery stratejisi (ARQ/FEC/karma/none). Kararı **yalnızca ölçülen ağ koşulundan** çıkar — kimin, ne amaçla gönderdiğinden değil. Uygulama katmanı mesajın rolüne (chat/file/control) karar verir; transport buna kör kalır.

### 4.1 QUIC Benchmark Zorunluluğu (NORMATİF)
```
- HER transport testi QUIC ile YAN YANA ölçülür (aynı ağ profili: bw/jitter/loss/RTT).
- Bir profil/rejimde QUIC daha iyiyse → O REJİMDE QUIC SEÇİLİR (custom motor ısrarı yok; doktrin: "en iyi olanı kullan").
- Custom transport'un varlık gerekçesi TEK ölçüt: QUIC'in ÇALIŞAMADIĞI/çöktüğü düşük bant + yüksek loss rejiminde
  (hedef: 0.025–0.2 kbps, %50–80 loss) QUIC'ten DAHA DÜŞÜK bant genişliğinde ayakta kalmak. Bunu kanıtlayamazsa
  custom motor gerekçesini kaybeder → QUIC'e düşülür.
- Ölçüm dürüstlüğü (RNS/TLS disiplini): "normal ağda QUIC kazanır" AÇIKÇA kabul edilir; üstünlük yalnız hostile-lowband
  rejiminde iddia edilir ve Side-by-Side ölçümle (PoC) gösterilir — slaytla değil.
- Karşılaştırma metrikleri: goodput @ verilen loss/bw, ilk-mesaj gecikmesi (zero-RTT vs QUIC 1-RTT+),
  teslim başarı oranı @ %80 loss, ACK/overhead bytes, offline-alıcı desteği (QUIC'te yok).
```

### 4.2 Worst-Case Delivery Envelope (NORMATİF DÜRÜSTLÜK — KAT2-P2-04)

> **İlke:** "5 bps'de çalışır" gibi bir **bps-floor** iddiası anlamsızdır — yeterince beklenirse her store-and-forward sistem teorik olarak 0.00001 bps'de bile bir şey ulaştırır (kurulum haftalar/aylar sürer). Doğru metrik **bounded operational envelope**'dur: belirli byte + loss + timeout + kullanıcı beklentisi içinde teslim. SafeRoom **bps floor yarışı yapmaz**; bounded store-and-forward teslim iddia eder.

```
Rejim                          | Kurulum (SESSION_INIT_REF 149B) | Mesaj (SESSION_DATA ~65B) | Beklenti
-------------------------------|----------------------------------|---------------------------|------------------
0.025 kbps / %80 loss / yük.gec | ~4 dk (EMERGENCY_FWD_REDUNDANCY) | ~1.5–2 dk/mesaj           | real-time DEĞİL;
  (EN KÖTÜ)                     |                                  |                           | bounded-delay S&F teslim
0.2 kbps / düşük-orta loss     | ~6–10 sn                         | ~3–5 sn/mesaj             | kullanılabilir mesajlaşma
  (ORTALAMA)                    |                                  |                           |
iyi ağ                         | onboarding/discovery/prefetch    | anlık                     | tam işlevsel
-------------------------------|----------------------------------|---------------------------|------------------
İlk-temas (cert inline 269B) worst-case: ~7 dk. Chain prefetch'liyse (discovery §8.0) REF 149B'ye düşer.
```
> Değerler ADR-18 netem/tc kalite kapısında ölçülerek doğrulanır (nightly, %80 loss profili dahil). Ürün bu rejimlerde **real-time VAAT ETMEZ**; iddia bounded teslimdir.

### 4.3 RNS / Signal-WhatsApp Comparison Boundary (NORMATİF — KAT2-P1-01)

```
RNS:       sovereign low-bitrate mesh/networking stack; physical-layer agnostic; ~5 bps floor;
           anonim/otoritesiz. SafeRoom RNS'i physical-layer minimum bitrate yarışında GEÇMEYİ HEDEFLEMEZ.
SafeRoom:  enterprise-controlled hostile-IP communication. Farklılaşma baseband DEĞİL:
           - PKI + revocation + admin control + auditability
           - always-online hub store-and-forward
           - app-level delivery semantics (offline recipient)
           RNS bu ekseni tasarım gereği hedeflemez (sovereign/anonim = ters yön).

Signal/WhatsApp:  TCP+TLS live-session. Yüksek loss'ta TCP çöker (HoL blocking, retransmit storm,
           handshake tamamlanamaz). SafeRoom'un ASIL kıyası budur: "Signal/WhatsApp sınıfı uygulamaların
           bağlantı kuramadığı hostile-IP rejimlerde bounded store-and-forward teslim." Bu SAVUNULABİLİR.
```
> **Tek cümlelik konum:** *SafeRoom, RNS'i raw baseband'de geçmeyi hedeflemez; iddiası, kurumsal PKI/revocation/audit/admin-control gerektiren hostile-IP ortamlarında Signal/WhatsApp sınıfı uygulamaların bağlantı kuramadığı rejimlerde bounded store-and-forward teslim sağlamaktır.*

---

## 5. Fuzzy Controller — FEC/ARQ/CC Karar Mekanizması (NORMATİF çekirdek)

> Sert eşikli state machine yerine fuzzy controller: eşik uçurumu ve osilasyon yok, yumuşak geçiş.

### 5.1 Girdiler (fuzzified) — bandwidth ≠ loss
```
loss_rate    ∈ {LOW, MED, HIGH}   (loss_pre_fec; residual_after_fec ayrıca)
jitter       ∈ {LOW, MED, HIGH}   (rttvar / reordering_score)
bandwidth    ∈ {LOW, MED, HIGH}   (delivered-rate sampling — OBSESİF ölçüm, loss'tan TÜRETİLMEZ)
queue_delay  ∈ {LOW, MED, HIGH}   (srtt - rtprop_min; congestion sinyali)
```
> **NORMATİF (delivered-rate sampling validity):** ACK compression / app-limited / reverse-path impaired sample'lar bw_est'i ŞİŞİREMEZ; bunlarda estimator_confidence düşer (Section 8). bandwidth daima delivered-bytes/time'dan, loss'tan DEĞİL.

### 5.2 Çıktılar (defuzzified)
```
fec_parity_ratio     [0..1]   (0 = FEC yok; Reed-Solomon erasure parity oranı)
arq_aggressiveness   [0..1]
send_rate (pacing)   bps      (token-bucket)
compress             {on/off}
```

### 5.3 Kural Tabanı (anahtar kurallar)
```
loss LOW                          → fec=0,    arq=light,  rate↑,            compress=policy
loss HIGH, bw HIGH, jitter LOW    → fec=high, arq=moderate (RS erasure),    compress=policy
loss MED,  bw MED                 → fec=med,  arq=med
loss LOW,  jitter HIGH            → fec=0,    arq=reorder-tolerant (RTO↑, fast-rtx↓)
bw LOW, queue_delay HIGH          → rate↓ (congestion; fec CAP)
loss HIGH, bw LOW, jitter HIGH    → mode = EMERGENCY_FORWARD_REDUNDANCY (§5.6), rate=minimal-reliable,
  (EN KÖTÜ — NORMATİF)              compress=ON.  Doktrin: "ulaştırmak > hızlı ulaştırmak."
                                    (pure-ARQ DEĞİL — %80 bidirectional loss'ta ACK dönüş yolu da ~%80
                                     kayıplı → round-trip ~%4 → ARQ sürünür; ACK'e bağımsız forward kurtarma.)
```

> **NORMATİF (adaptif seçim — çekirdek doktrin):** Recovery stratejisi ölçülen koşuldan çıkar; **ne ARQ ne FEC sabit/default değildir.** İki kanonik gerekçe:
> ```
> bant VAR + loss YÜKSEK  → ARQ İŞE YARAMAZ (retransmit'ler de kaybolur/gecikir; RTT şişer) → FEC AĞIR BAS.
> bant YOK  + loss DÜŞÜK  → FEC İŞE YARAMAZ (parity zaten dar bandı israf eder)            → ARQ (retransmit ucuz, nadir).
> ```
> ARQ ve FEC farklı başarısızlık rejimlerini kapatır; fuzzy controller ölçüp doğru olanı (veya karmasını) seçer. Sabit strateji lowband-hostile'da çöker — belgenin varlık gerekçesi budur.

### 5.6 EMERGENCY_FORWARD_REDUNDANCY (NORMATİF — worst-case köşe, KAT2-P1-02)

> **Neden pure-ARQ değil:** %80 bidirectional loss'ta `P(data ulaşır ∧ ACK döner) = 0.2 × 0.2 ≈ %4` → pure-ARQ ~25× round-trip overhead'e sürünür. Bu köşede kurtarma **ACK dönüş yoluna bağımsız (forward)** olmalıdır. Karar tamamen **transport fuzzy'sinindir** — ölçülen `loss/bw/jitter/ack_confidence`'a göre; uygulama katmanı DEĞİL (SoC).

```
GİRDİ (yeni): ack_confidence = delivered-ACK / beklenen-ACK örneklemesi (dönüş yolu sağlığı, LOW/MED/HIGH)

IF loss HIGH  AND  bandwidth LOW  AND  ack_confidence LOW:
  mode = EMERGENCY_FORWARD_REDUNDANCY

  # framing/boyut'a göre (uygulama rolüne göre DEĞİL — SoC):
  tek-datagram / tiny payload (block/shard yok):
    → preemptive DUPLICATION 2×/3× (zaman-aralıklı; spaced intervals)
      (RS-erasure tek shard'a uygulanamaz; duplication doğru araç)
  segment-block / multi-shard payload:
    → minimal parity shards (low-rate forward FEC)

  ACK:
    → sparse cumulative / range ACK yalnız (ACK decimation)
    → per-paket ACK YOK, ACK storm YOK, per-paket ACK bağımlılığı YOK

  controller (kapalı çevrim):
    → teslimi ölç; path düzelirse redundancy'yi AZALT (delivered-rate confidence yükselince 3×→2×→ARQ'ya iniş)
```
> Bu mod ne "hep ARQ" ne "hep FEC"dir; **ACK-bağımsız forward kurtarma**dır ve path iyileşince fuzzy tarafından geri çekilir. `parity/duplication` yine `available_redundancy_budget` ile clamp'lenir (§5.5).

### 5.4 Loss-Decoupling YAPISAL (INV-RATE-1 fuzzy'ye gömülü)
> **NORMATİF:** `send_rate` çıktısı YALNIZCA bandwidth + queue_delay membership'ine bağlıdır; **loss_rate, rate çıktısının girdisi DEĞİLDİR.** Böylece "loss alone MUST NOT reduce rate" fuzzy tasarımın içine gömülüdür (kanal kaybı FEC/ARQ'yu etkiler, rate'i değil). queue_delay/RTprop inflation/delivery collapse rate'i düşürür.

### 5.5 Budget clamp
`fec_parity_ratio` defuzzified çıktısı **available_redundancy_budget** ile clamp'lenir (düşük bantta yüksek loss bile parity patlatamaz): `parity_final = min(fuzzy_parity, budget_cap, profile_cap)`.

---

## 6. SRC — Rate Control + Nonce/Seq

```
SRC: delivered-rate sampling (validity-gated) + RTprop/queue_delay + token-bucket pacing.
     rate kararı fuzzy controller'dan (Section 5); loss rate'i düşürmez.
NONCE/SEQ (ccr §35 OTORİTER): seq16 nonce DEĞİLDİR — düşük 16 bit / resync hint'tir. Receiver tam
     counter'ı session_id_internal + direction + epoch + replay window + expected counter ile reconstruct eder.
     replay identity = (session_id_internal, direction, epoch, full_counter); replay window AEAD
     başarısından ÖNCE güncellenmez.
REKEY invariants (ccr §35 ile birebir): MAX_MESSAGES_PER_EPOCH = 2^32; REKEY_THRESHOLD <= 2^31;
     future gap / reorder pencereleri < 2^15 (seq16 reconstruction tek-anlamlı kalsın). Değerler ccr §35 authoritative.
```

---

## 7. Pipeline'lar (framing-driven — NORMATİF)

> **NOT (SoC):** Pipeline ayrımı **framing/boyut** kaynaklıdır (tek datagram mı, segment-block mı) — uygulama rolü (chat/file/"hot-path") DEĞİL. Recovery stratejisi (ARQ/FEC/karma/none) HER pipeline'da fuzzy controller'dan gelir (§5); transport hangi uygulama olduğunu bilmez.

```
A — Tek-datagram payload (MTU'ya sığar):  envelope → SRC pacing/batching/N-msg ACK window → UDP
    outer AEAD/FEC/prefix = 0B; auth = inner E2E tag; demux = hub_handle.
    Recovery: fuzzy — loss düşük/bant dar → ARQ (selective retransmit); tek küçük mesajda FEC uygulanamaz
    (block yok) → yüksek loss'ta duplication/ARQ. Strateji koşuldan çıkar.
B — SESSION_INIT/resync:    Noise handshake+early_data → retry scheduling → SRC pacing → UDP
C — Segment-block payload (MTU'yu aşar → shard'lara bölünür):
    Gönderen: block_plaintext → (fuzzy FEC seçtiyse) Reed-Solomon encode → plaintext shard'lar → her shard AYRI
              AEAD-encrypt/authenticate → gönder.
    Alıcı:    şifreli shard al → her shard'ı ÖNCE AEAD verify/decrypt → yalnızca authenticated shard'lar FEC
              decoder'a girer → eksik shard'lar reconstruct.
    NORMATİF: Authenticated olmayan byte FEC decoder'a GİRMEZ (SR-AUDIT-001). Recovery: fuzzy —
              bant var+loss yüksek → FEC ağır; bant dar+loss düşük → FEC=0, ARQ retransmit; karma mümkün.
    Outer AEAD (Noise transport key) per-shard = block framing gereği (FEC shard auth); tek-datagram'da YOK.
D — CONTROL/HOP:            control → per-hop Noise/TLS/AEAD → SRC → UDP/QUIC (deployment)
```
> Tek generic pipeline tüm payload'lara KÖR uygulanmaz. FEC/outer-AEAD **framing-specific** (block'ta mümkün; tek-datagram'da değil). Tek adaptif transport motoru tüm pipeline'larda; ayrı segment-block motoru YOK.

---

## 8. Sıkıştırma (yeniden açıldı — NORMATİF)

> Eski "CRIME → kapalı" kararı GERİ ALINDI. CRIME = saldırganın seçili-plaintext'i sırla **aynı sıkıştırma bağlamına** enjekte edip ciphertext uzunluğunu gözlemesi + HTTP cookie oracle. E2E mesajda böyle oracle YOK, HTTP yüzeyi YOK → CRIME geçersiz.

```
compress = zstd; metin mesaj için default ON (düşük bantta büyük kazanç); fuzzy çıktısı (Section 5).
Ses/video: codec'te zaten sıkıştırılı → ÇİFT sıkıştırma YOK.
Residual: sıkıştırma uzunluğu içeriğe bağlı → uzunluk-tabanlı trafik analizi (CRIME DEĞİL, metadata).
  Bu zaten kabul edilen hub-metadata katmanında (sealed-sender Beta+); padding Beta+ knob.
```

---

## 9. NetworkClassifier Metrikleri (fuzzy girdileri + tanı)

```
send_rate, delivered_rate, ack_spacing_rate; loss_pre_fec, fec_recovered, residual_after_fec;
srtt, rttvar, rtprop_min, queue_delay, queue_delay_p95; jitter_score, reordering_score;
inflight_bytes, tx_queue_bytes/delay; ack_loss_rate, ack_compression_score; fec_recovery_rate, arq_retransmit_rate.

PATH_DEAD vs CHANNEL_LOSS vs LOW_BW vs REVERSE_PATH ayrımı (tanı):
  no_ack(T) → PATH_DEAD;  ack_loss high + forward unclear → REVERSE_PATH;
  queue_delay high + delivered plateau → LOW_BANDWIDTH;  loss high + queue low + ack stable → CHANNEL_LOSS.
NORMATİF: Loss rate MUST NOT be a direct bandwidth estimator. No-ACK ≠ high-loss (forward/reverse/dead/delay ayrılır).
```

---

## 10. Cold-Start (PROFILE_PROBE)
> Bant **en kötü** varsayılır (LOW_BANDWIDTH_STRICT pacing, küçük MTU) **ama FEC YOK** (kör FEC ağı öldürür). İlk paketler + SESSION_INIT FEC'siz; kayıp ölçülür; sonra (block/shard framing mevcutsa) fuzzy FEC. No-ACK → probe rate/MTU düşür + retry budget; tekrarlı no-ACK → PATH_DEAD (FEC kör escalate ETME).

---

## 11. MTU — Hostile-Safe (NORMATİF)
```
default path_mtu_target = 1200B (fragmentation kaçınımı); probe 576/768B; lowband 512–768B;
1900B YALNIZCA path validation sonrası. Küçük paket > fragmentation. Artış opt-in; azaltma ANINDA.
```

---

## 12. ACK Stratejisi (windowed — per-paket ACK DEĞİL)
```
N-mesaj ACK penceresi; ACK range/SACK compression; piggyback; ACK decimation; per-profil ACK budget.
Per-paket ACK KULLANILMAZ (ACK storm + bant israfı — KCP'nin bu davranışı reddedilme gerekçelerindendi).
Selective retransmit: SACK boşluğu → yalnız eksik seq'ler yeniden gönderilir (paket kaybı burada kurtarılır).
0.025–0.2 kbps'de ACK overhead payload'dan büyük olabilir (açıkça kabul; ACK decimation + piggyback ile azaltılır).
```
> **Paket kaybı → ne olur (NORMATİF):** Recovery **her zaman aynı değildir** — fuzzy controller o anki bw/jitter/loss'a göre stratejiyi seçmiş olur (§5): (a) FEC seçildiyse kayıp shard'lar **parity'den yeniden kurulur** (retransmit-round beklenmez — bant var + loss yüksek durumu); (b) ARQ seçildiyse SACK boşluğu → **selective retransmit** (bant dar + loss düşük durumu); (c) karma seçildiyse ikisi birlikte. Bu ACK stratejisi (windowed/SACK) yalnızca ARQ *seçildiğinde* devrededir; FEC-baskın rejimde retransmit minimaldir. Recovery katmanının kendisi her zaman vardır ama **hangi mekanizmayı çalıştırdığı adaptiftir** — "hep ARQ" veya "hep FEC" YANLIŞTIR.

---

## 13. Layer Accounting (NORMATİF)

| Mesaj | App-envelope | UDP payload overhead | UDP/IPv4 | Not |
|---|---:|---:|---:|---|
| SESSION_INIT_REF | 121B + early_data | 121B + early_data | 149B + early_data | bootstrap; Bob offline olabilir |
| SESSION_INIT_INLINE_COMPACT | 241B + early_data | 241B + early_data | 269B + early_data | sender CompactDeviceCert inline |
| SESSION_INIT_RESPONSE | 57B + response_data | 57B + response_data | 85B + response_data | async FS-upgrade/bidirectional activation |
| SESSION_DATA | 18B + payload | 22B + payload | 50B + payload | hot-path; hub_handle L2'de dahil |
| SIGNED_CONTROL | 82B + payload | 86B + payload | 114B + payload | nadir non-repudiation |

```
TLS/TCP/IPv4 established baseline ≈ 62B + payload.
SafeRoom SESSION_DATA/UDP/IPv4 = 50B + payload → per-mesaj ~12B avantaj.
Canonical app-envelope byte otoritesi: ccr §35. Boyutlar ccr §35 ile birebir.
```
> **Dürüst çerçeve:** per-mesaj byte üstünlüğü TLS'e karşı mütevazı (~12B); asıl üstünlük yapısal — zero-RTT (handshake round-trip yok), store-and-forward (offline alıcı), always-on session (re-handshake/keepalive yok), per-mesaj imza yok. "TLS'i byte'ta ezdik" diye SATILMAZ.

---

## 14. Protocol Invariants

| ID | Statement |
|----|-----------|
| INV-BIND-1 | Established mesaj bağı anahtardadır; wire'da görünür bağ (link_id/session_id/cert-ref/imza) YOK — yalnızca hub_handle (demux) + seq + AEAD tag. |
| INV-WIRE-1 | SESSION_DATA = hub_handle(4B) + seq(2B) + ct + tag(16B); length implicit (UDP). Outer AEAD/FEC/prefix default YOK. |
| INV-WIRE-2 | Per-mesaj imza YOK (Ed25519 yalnızca cert; N-türevi ss_s static-DH auth; SIGNED_CONTROL istisna). |
| INV-RATE-1 | Loss alone MUST NOT reduce rate — fuzzy'de yapısal (loss, rate çıktısının girdisi değil). |
| INV-RATE-2 | Rate = SRC + fuzzy controller; recovery (ARQ/FEC/karma/none) da fuzzy-seçili; sabit strateji YOK. |
| INV-FUZZY-1 | FEC/ARQ/rate/compress kararı fuzzy controller (loss/jitter/bw/queue → defuzzified); parity budget-clamp'li. |
| INV-CLASS-2 | Loss rate MUST NOT be a direct bandwidth estimator (delivered-bytes/time). |
| INV-FEC-1 | FEC plaintext shard'lar üzerinde çalışır AMA her alınan shard FEC decoder'a girmeden ÖNCE AEAD-authenticate edilir; authenticated olmayan byte decoder'a GİRMEZ (SR-AUDIT-001). FEC yalnızca block/shard framing MEVCUTSA uygulanır — framing-driven, application-role-driven DEĞİL; transport baytların chat/file/control olduğunu bilmez. |
| INV-FEC-2 | FEC erasure coding (Reed-Solomon), XOR değil; on-demand (loss eşiği + ARQ kurtaramıyor/yavaş). |
| INV-ARQ-1 | TEK custom adaptif transport motoru (Rust-native, C++/FFI yok); ARQ + FEC mekanizmalarını İÇERİR; fuzzy controller ölçülen bw/jitter/loss'a göre seçer (ne ARQ ne FEC default). Transport uygulama rolünü (hot-path/bulk) BİLMEZ (SoC). KCP çıkarıldı. |
| INV-QUIC-1 | Her transport testi QUIC ile kıyaslanır; bir rejimde QUIC iyiyse QUIC seçilir; custom motor yalnız QUIC'in çöktüğü lowband/high-loss rejiminde (QUIC'ten düşük banta iner) gerekçelenir. QUIC fork NO-GO. |
| INV-CRYPTO-1 | Client↔Hub Noise IK/KK (setup: dest_hash ClientHint'te gizli + hub_handle atama); single-datagram outer Noise YOK (thin); outer AEAD yalnız block/shard framing + control; hub-mesh Noise KK. |
| INV-NONCE-1 | Transport counter monoton; 2B seq wire'da; receiver reconstruct; wrap→rekey; reuse imkansız. |
| INV-REPLAY-1 | Replay window AEAD başarısından ÖNCE güncellenmez. |
| INV-COMPRESS-1 | Sıkıştırma izinli (CRIME E2E'de geçersiz); ses/video çift-sıkıştırma yok; uzunluk-leak = kabul edilen metadata. |
| INV-MTU-1 | Default 1200B (fragmentation kaçın); yüksek MTU validation sonrası opt-in; azaltma anında. |
| INV-ACK-1 | Windowed/batched ACK; per-paket ACK YOK (ACK storm engeli); ACK-only budgeted. |
| INV-LAYER-1 | hub_handle (L2) cleartext/hub-route; E2E envelope (L3) hub'a opak; FEC/ARQ (L5) profile-specific. |

---

## 15. Parser / Güvenlik
```
- SESSION_DATA: hub_handle ile demux (hub session key bilmez → geçerli mesaj üretemez);
  inner AEAD verify (ccr §35) fail → drop, replay window güncellenmez.
- block/shard + control framing: reject kuralları; outer AEAD verify FAIL → FEC decoder'a shard GİRMEZ.
- PARSER-02: untrusted length ile asla with_capacity; take(MAX); bounds-check alloc'tan önce.
- counter wrap → rekey; trailing/truncated → reject.
```

---

## 16. Acceptance Criteria

| AC | Kriter |
|----|--------|
| AC-1 | SESSION_DATA = handle4+seq2+tag16 = 22B; length implicit; outer AEAD/FEC/imza YOK. |
| AC-2 | Established bağ anahtarda; wire'da link_id/session_id/cert-ref/imza yok (INV-BIND-1). |
| AC-3 | Aktif Session Model V3 taksonomisi 4 tip: SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL. |
| AC-4b | SESSION_INIT zero-RTT authenticated one-way + early_data; Bob offline olabilir. SESSION_INIT_RESPONSE ilk mesajı okumak için gerekli değil; epoch-1 upgrade. |
| AC-14 | SESSION_INIT/RESPONSE/DATA/SIGNED_CONTROL canonical byte boyutları ccr §35 ile birebir; "~48B core" canonical SESSION_INIT boyutu olarak SUNULMAZ. |
| AC-15 | Batch iç framing tek deterministik fixed-width Alpha formatı; LEB128/protobuf varint YASAK. |
| AC-16 | hub_handle allocation allocation_attempt + collision check içerir (MAX_HANDLE_ALLOC_ATTEMPTS=8). |
| AC-17 | Section 18 ccr §35 rewrite "pending" İDDİA ETMEZ (ccr §35 canonical authority). |
| AC-4 | Zero-RTT N-türevi one-way + early_data; konuşma başına 1 handshake; Bob offline tolere. |
| AC-5 | Fuzzy controller: loss/jitter/bw/queue → fec/arq/rate/compress; rate loss'a bağlı DEĞİL. |
| AC-6 | bandwidth obsesif sampling (validity-gated); ACK-compression/app-limited bw_est'i şişirmez. |
| AC-7 | FEC on-demand Reed-Solomon erasure (XOR değil); yalnızca block/shard framing MEVCUTSA (framing-driven, app-role değil); AEAD-verify önce. |
| AC-8 | En kötü senaryo (loss high+bw low+ack_confidence low): mode=EMERGENCY_FORWARD_REDUNDANCY (§5.6) — tiny→2×/3× duplication, block→minimal parity, sparse cumulative ACK; pure-ARQ DEĞİL; path düzelince fuzzy redundancy'yi geri çeker. compress=on, rate=minimal-reliable. |
| AC-9 | KCP tamamen ÇIKARILDI; tek custom adaptif transport motoru (Rust-native, ARQ+FEC, fuzzy-seçili; uygulama rolüne kör); per-paket ACK yok (storm engeli). |
| AC-QUIC | Her test QUIC ile yan yana ölçülür; QUIC daha iyiyse QUIC seçilir; custom transport QUIC'in çöktüğü lowband/high-loss'ta QUIC'ten düşük banta iner (kanıt = Side-by-Side). QUIC fork NO-GO. |
| AC-CRYPTO | Kripto katmanlama §0.1: Noise IK/KK setup (dest_hash ClientHint gizli) + thin single-datagram (outer Noise yok) + outer AEAD yalnız block/shard framing + control + hub-mesh Noise KK. |
| AC-10 | Sıkıştırma izinli (CRIME N/A); ses/video çift-sıkıştırma yok. |
| AC-11 | MTU default 1200B; 1900B validation sonrası. |
| AC-12 | Layer-matched accounting: SESSION_DATA/UDP/IPv4 = 50B ≤ TLS/TCP/IPv4 62B; üstünlük yapısal sunulur. |
| AC-13 | counter wrap→rekey; replay window AEAD'den sonra; seq 2B nonce resync. |

---

## 17. Self-Audit

| Kontrol | Statü | Bölüm |
|---------|-------|-------|
| TLS dersi: bağ anahtarda, wire'da görünür bağ yok | ✅ | 0, 2, INV-BIND-1 |
| SESSION_DATA thin wire (22B, length implicit, imza yok) | ✅ | 2 |
| Taksonomi 3 tür (collapse) | ✅ | 1 |
| Zero-RTT N-türevi one-way + early_data; offline tolere | ✅ | 3 |
| Fuzzy controller; loss-decoupling yapısal | ✅ | 5 |
| Adaptif recovery: ne ARQ ne FEC default; koşul ölçülür strateji çıkar (2 kanonik durum) | ✅ | 5.3 |
| bandwidth ≠ loss (obsesif sampling, validity-gated) | ✅ | 5.1, 9 |
| FEC on-demand Reed-Solomon, plaintext-domain, AEAD-önce, block/shard-framing-only | ✅ | 5, 7, INV-FEC-1/2 |
| KCP çıkarıldı; tek adaptif transport motoru (ARQ+FEC, fuzzy-seçili, SoC: rolü bilmez) | ✅ | 4, 7, 12, INV-ARQ-1 |
| QUIC benchmark zorunlu; QUIC iyiyse seçilir; lowband'de QUIC'ten iyi (fork NO-GO) | ✅ | 4.1, INV-QUIC-1, AC-QUIC |
| Kripto katmanlama kilitli (Noise IK/KK setup + thin single-datagram + outer-AEAD block/shard-only) | ✅ | 0.1, INV-CRYPTO-1 |
| Paket kaybı → fuzzy-seçili recovery (ARQ/FEC/karma, koşula göre; ne ARQ ne FEC default) | ✅ | 5.3, 12 |
| Sıkıştırma izinli (CRIME N/A) + uzunluk-leak notu | ✅ | 8 |
| MTU 1200B hostile-safe; nonce/seq/replay | ✅ | 11, 6 |
| Layer accounting + dürüst TLS kıyas | ✅ | 13 |
| Katmanlar ayrık (L1–L5) | ✅ | 0 |
| ccr §35 Session Model V3 ile hizalı; no paired rewrite pending | ✅ | 1, 3, 13, 18 |
| SESSION_INIT_RESPONSE aktif Session Model V3 mesaj tipi olarak tanımlı | ✅ | 1 |
| SESSION_INIT exact sizes ccr §35 ile uyumlu | ✅ | 3, 13 |
| Batch framing varint kullanmıyor | ✅ | 2.1 |
| hub_handle PRF allocation_attempt ile collision-safe | ✅ | 2 |
| FEC decoder yalnız authenticated shard alır | ✅ | 7 |

---

## 18. Status & Cross-Doc

```
Transport Layer TDD Status: Implementation-ready / freeze-ready (Session Model V3 terminoloji reconciliation sonrası).

Cross-doc status:
  ccr-format-spec.md §35 = SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL +
    AAD/session_context + replay/counter reconstruction + hub_handle binding için CANONICAL byte-level OTORİTE.
  Bu belge = SRC/pacing + fuzzy controller + FEC/ARQ adaptif seçim (tek adaptif transport motoru; ne ARQ ne FEC default) + QUIC benchmark + MTU/ACK/compression +
    layer accounting view için TRANSPORT POLICY/PROFILE otoritesi.
  PAIRED REWRITE BEKLEMİYOR — ccr §35 zaten Session Model V3 canonical authority.

Kalan (freeze dışı): implementation test vektörleri; fuzzy eşik kalibrasyonu; FEC parity budget kalibrasyonu;
  path MTU empirik tuning.
```

```
Final Status:
  Transport Layer TDD is freeze-ready for Alpha.

Frozen:
  - layer model + crypto layering (§0.1: Noise IK/KK setup + thin single-datagram + outer-AEAD block/shard-only)
  - SESSION_DATA single-datagram transport view (application-role-agnostic)
  - Session Model V3 message taxonomy (4 active types)
  - KCP removed — single custom adaptive transport engine (Rust-native, no C++/FFI; contains ARQ+FEC, fuzzy-selected; application-role-agnostic)
  - FEC/ARQ framing-driven selection (single adaptive engine; recovery = fuzzy-selected; no app-role awareness)
  - fuzzy controller architecture
  - QUIC benchmark mandate (§4.1: every test vs QUIC; QUIC if better; beat QUIC only in lowband; fork NO-GO)
  - compression policy
  - MTU/ACK strategy
  - TLS comparison framing

Delegated canonical byte authority:
  - ccr-format-spec.md §35 for Session Model V3 bytes/AAD/replay/counter
  - this document for transport policy and profile selection

Remaining non-freeze work:
  - empirical fuzzy membership thresholds
  - FEC parity calibration
  - network simulation test corpus
  - implementation test vectors
```

> **Sonuç (NORMATİF):** SafeRoom transport "KCP over UDP" / "QUIC clone" değildir; KCP ÇIKARILDI (tek adaptif transport motoru, Rust-native, ARQ+FEC içerir). Async hot-path = UDP üzerinde E2E-authenticated message envelope + kısa hub handle; bağ TLS gibi anahtarda; kripto katmanlama §0.1'de kilitli (Noise IK/KK setup + thin hot-path). FEC/ARQ/SRC seçili modların araçları; FEC/ARQ/CC kararı fuzzy (loss/jitter/bw). Paket kaybı fuzzy controller'ın ölçülen koşula göre SEÇTİĞİ recovery ile (ARQ / FEC / karma) kurtarılır — ne ARQ ne FEC sabittir; transport uygulama rolünü bilmez (SoC). QUIC = zorunlu benchmark referansı (iyiyse seçilir; custom motor yalnız QUIC'in çöktüğü lowband/high-loss rejiminde, QUIC'ten düşük banta inerek gerekçelenir; fork NO-GO). Tek savunulabilir lowband mimarisi: thin layer-collapsed envelope + tek adaptif ARQ/FEC motoru (koşula göre seçen) + fuzzy karar + QUIC-kıyaslı dürüstlük.
