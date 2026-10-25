# Kama — Transport Layer TDD (Adaptive Hostile-Aware Transport)

> **YAMA KAYDI — 2026-10-25 [v1.1 · K4G-05c — K_MAP budama seçim kuralı ve geç-ACK tanıma garantisi; sahip kararı CEVAP PAKETİ 14, 2026-10-25, DUR-05c-14 (A, koşullu) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §12 [AMENDMENT-12] "Kayıt sınırı ve geç-ACK tanıma" paragrafında budama cümlesi (K4G-05b'de budama KAPALI) ve tanıma garantisi üstü çizildi; yerine: budama açık (runtime'ın birim başına O(1) kaydıyla aynı değişiklikte); birim başına uçuş + tombstone kaydı TOPLAMI ≤ K_MAP, orijinal dahil; aşılırken orijinal Data kaydı DIŞINDAKİ en eski kayıt budanır. Orijinal yalnız K_MAP budamasından muaftır; tombstone TTL'i ve tombstone tavanı tahliyesi ona da uygulanır, daha geniş saklama garantisi yoktur. Motorun hâlâ tuttuğu kaydın geç ACK'i birimi teyit eder; unutulmuş (budanmış, süresi dolmuş, tahliye edilmiş) kaydınki teyit etmez, payload korunur. K_MAP = 8 değişmedi; tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-24 [v1.1 · K4G-05b — yeniden-gönderim sahipliği; sahip kararları CEVAP PAKETİ 4 ve 5 (2026-10-16), CEVAP PAKETİ 6, 7 ve 7-EK (2026-10-18), CEVAP PAKETİ 8 (2026-10-19), CEVAP PAKETİ 9 (2026-10-24), CEVAP PAKETİ 10, 11, 11-EK, 12 ve 13 (2026-10-24) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`]:** §12'ye [AMENDMENT-12]: INV-NO-ORPHAN (sahip metni) ve üç kabul koşulu (kayıtlı · payload mevcut · hizmet verilebilir bütçede yeniden seçilebilir) · motorun **teyit çıktısı** (payload yalnız onunla silinir — runtime tarafı K4G-05b; kaydı unutan hiçbir yol motor ile runtime arasında payload sahipliği ayrışması yaratmaz) · ARQ_RETX_CAP aşımı ve kuyruk-probu bütçesinin tükenmesi yolu değiştirir, birimi düşürmez; prob bütçesi tükenince prob kapsamındaki birimler aynı turda devredilir · **RTO_MAX = 30 s** normatif sabit (koddan belgeye: `TailRecoveryConfig::validated_v1().max_backoff_pto_ns`; yeni sayı yok) · kesinti sonrası ilk taze ACK'te kopya geri çekilmesi sıfırlanır · FEC onarım sembolünün ACK'i kaynak birimi teyit etmez, onarım sembolü sahiplenilmez · K_MAP = 8 birim başına iletim kaydı için ADET sınırı, süre garantisi değil (budama ve birim başına O(1) runtime kaydı K4G-05c'de birlikte açılır; tanınmayan geç ACK teyit etmez, veri kaybı değildir) · kimlik `kama-v1-full` → `kama-v1.1-full`, `transport-v1` → `transport-v1.1` (AMENDMENT-11'in beyan ettiği `transport-v1.x` de bununla sabitlere işlenir). §5.3 "RTO↑" satırına tavan işareti. Engine-spec §5/§6 (RTT kabul kuralı + tazelik ölçütü), §13 (kopya kuyruğu sırası, BURST_CAP tur başı tabanı, sınıflar arası kopya payı ve rezervasyonu) ve §16.2 (tek saat-sınıfı satırı) v1.1 yamaları aynı kartta; yetki dışındaki belgelere tek satırlık AMENDMENT-12 işaretçileri, DR-CS-1 belgesine kabul fikstürü YAMA KAYDI. Tel, codec ve altın vektörler (tv_*) DEĞİŞMEDİ. Bu belgede metin silinmedi (yalnız ekleme).

> **YAMA KAYDI — 2026-10-08 [AMENDMENT-11 — WS-D (BL D1/D2/D3) UYGULANDI, L-D şeridi; iş emri 2026-10-06b §5]:** §12'ye [AMENDMENT-11]: ARQ/ACK kimliği = `meta.full_wire_pn` KODDA (çeviri katmanı `client-core` runtime'ında; `adaptive-transport` motoru değişmedi; ACK_FRAME_V2 formatı ve `ack_epoch` dokunulmadı) · noise-wire `Control::Ack/Sack` (0x10/0x11) kod silimi, codepoint'ler RESERVED → reject · golden etkisi listesi · altı beyan. Ara 11 B çerçeve veri yolunda KALIR (S3 kararı: beyanlı KARŞILANMAZ). Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-05 [PN SÜREKLİ artıkları — Abkarada kararı]:** §6 NONCE/SEQ: sayaç geri kazanımından ve `replay identity`'den `epoch` ÇIKTI (ccr §35.8 ile birebir) → kimlik `(session_id_internal, direction, full_counter)`; tek pencere (identity §15.5, 2026-10-06). Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-06 [PN SÜREKLİ — Abkarada kararı; WS-D1 `dh_epoch` sorusu KAPANDI]:** §12'ye ACK kimliği satırı: ACK_FRAME_V2 kimliği = `full_wire_pn`; `dh_epoch` kimliğe girmez. [AMENDMENT-9]'daki "sayım (session, direction, dh_epoch) başınadır" → "(session, direction) başınadır" (noise-wire §2.5 ile birebir). Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-04b [K4 — Abkarada kararı]:** §10 [AMENDMENT-10] madde 5 jitter formülü kesinleşti: `BLAKE2s-256(seed ‖ u32_be(deneme))[0..4] mod pencere`; tohum **dest_hash DEĞİL** — `hub_handle` (bind sonrası, bacak başına) / hop handshake'inin yerel ephemeral public anahtarı (bind öncesi). Önceki `hash(client_id ‖ …)` ifadesi üstü çizildi. SPEC-ONLY.

> **YAMA KAYDI — 2026-10-04 [DR-CONNECT-1 tamamlama — sahip kararı]:** §10 [AMENDMENT-10] genişletildi: **H4 hop heartbeat YENİ CODEPOINT ile** (`HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C`, HubChannel ctx, sıfır payload + opsiyonel u32 echo token, kendi sayacı yok, REFRESH-ONLY, CS probe bütçesi ve INV-CS-7 kapsamı dışında, RTT observation-only) — CS_PROBE'a bindirilmedi · **KS-HB-1 KAPANDI** (8 s / 25 s / OS) · **jitter RNG'siz** (`hash(client_id ‖ deneme_sayacı) mod pencere`, pencere = backoff adımının %25'i) · link/power sinyali gelene kadar ön-plan davranışı notu (K5 → PARK). Aşağıdaki 2026-10-03 kaydındaki "H4 UYGULANMADI" ifadesi bu kayıtla aşılmıştır. Rust ürün koduna dokunulmadı; üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-03 [DR-CONNECT-1 — sahip kararı; indeks `docs/decisions/DR-CONNECT-1.md`]:** §0'a **INV-NO-IDLE-TEARDOWN** (oturum/niyet zamanla sonlandırılmaz; sonlandırma nedenleri kapalı liste) eklendi (H1/H2) · §12 [AMENDMENT-9] **KS-PN-1 KAPANDI: K = 1024** + tavana ulaşınca davranış (REBIND → floor prob → hop-ACK'le devam) (H5) · §10'a **[AMENDMENT-10]** hub-kanalı prob kadansı: bağlama göre tavan (8 s / 25 s / OS), link yokken askıya alma, anında prob, zorunlu jitter (H3; K3/K5 kanıtıyla; SPEC-ONLY, altı beyan içinde). H4 (hop heartbeat taşıyıcısı) UYGULANMADI — K1 kanıtı sonrası sahip kararı bekliyor. Rust'a dokunulmadı; üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-09-18b:** §7 Pipeline C "Outer AEAD (Noise transport key)" → session E2E anahtarı (§0.1 ile tutarlı); alıcı adımı "seam.open → transport reconstruct"; §0'a INV-PLAINTEXT-NOT-SEMANTICS (plaintext görmek ≠ semantik bağ; serileştirme sınırı PreparedUnit→UnsealedTransportUnit→SealedWireUnit; CCR/cert canonical-codec-only, protobuf re-encode yasak, compressor yasak; private key API'den geçmez).


> **YAMA KAYDI — 2026-09-23 [AMENDMENT-8 + AMENDMENT-9]:** §7 Pipeline A ve §12'deki "N-mesaj ACK penceresi" ifadeleri temizlendi (AMENDMENT-8; 2026-09-22 üçe-ayırma kararının izi, **davranış değişikliği yok**, `kama-v1-full` korunur). §12'ye AMENDMENT-9 eklendi: D-GAP-2 — hop-ACK almadan ardışık tüketilen PN ≤ K (K ≪ 2^15); üç sayı-tavansız emisyon yolu buna bağlandı; K için KS-PN-1 kalibrasyon slotu açıldı. **Uygulandığında davranış değişir** → EVOLUTION §22'nin altı beyanı amendment içinde yazılı, kimlik `transport-v1.x`. §11 REKEY invariants satırı D-GAP-1'e bağlandı. SPEC-ONLY: kod değişmedi.


> **YAMA KAYDI — 2026-09-18 [AMENDMENT-6, Abkarada kararı + Claude Code kanıtı]:** §0'a INV-NEIGHBOR-1 (kanonik katman grafiği App→Transport→Crypto→UDP; degree(App)=degree(Crypto)=1; Transport→Crypto iki arayüz ailesi PacketProtection + SessionControl typed; SessionHandle ≠ HubHandle invariantları; zaman sahipliği ayrımı; early_unit istisnası) · §0.1 shard AEAD anahtarı "Noise transport key" → session E2E anahtarı; ProtectionContext S3 [OPEN] · §2 22B formatı = seam çıktısı notu (kod: legacy.rs:126, wire.rs:15-30, IdentitySeam no-op) · §8 AMENDMENT-4 gerekçesi düzeltildi ("transport ciphertext görür" → plaintext görür; app'te kalma gerekçesi tip bilgisi + aktüatör yokluğu, fusion.rs:551 / Cargo.lock negatif). INV-FEC-1 ve L-modeli DEĞİŞMEDİ.


> **Tasarım ilkesi (TLS 1.3 dersi, NORMATİF):** Handshake sonrası bağ **byte'larda değil, anahtarda** kurulur. TLS established record'unda `connection_id`/`session_id`/`handshake_hash`/`cert_hash`/imza **yoktur** — yalnızca 16B AEAD tag vardır; tag handshake'ten türeyen key ile üretildiği için bağ kriptografiktir. Kama aynı modeli benimser: established mesajda **yalnızca demux handle + seq + ciphertext + AEAD tag** bulunur. Per-mesaj imza, link_id, cert-ref, routing-info, length **TAŞINMAZ.**
> **Bağımlılık:** identity-and-trust-model.md (trust), ccr-format-spec.md (**§35 Session Model V3 — SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL canonical app-envelope byte'ları ve wire/session canonical semantiği orada**), routing-rendezvous-architecture.md (CAD-53/56/57), discovery-architecture.md.
> **Çekirdek:** Kama transport "KCP over UDP" değil, "QUIC clone" değildir. Async hot-path = **UDP üzerinde E2E-authenticated message envelope** + hub-routing handle. FEC/ARQ + SRC seçili modların araçlarıdır (adaptif motor koşula göre seçer), her mesajı saran zorunlu zarf değildir. **KCP çıkarıldı** (§4). QUIC = zorunlu benchmark referansı, bağımlılık değil (§4.1).

---

## 0. Katman Modeli (NORMATİF — omurga)

> **[AMENDMENT-6 — KANONİK KATMAN GRAFİĞİ, 2026-09-18, Abkarada] INV-NEIGHBOR-1 (NORMATİF):**
> ```
> Application → Transport (bu belge; kama-core) → Crypto (tel kenarı) → UDP
> degree(Application) = 1   tek alt komşusu Transport
> degree(Crypto)      = 1   tek üst komşusu Transport
> ```
> Application, Crypto'yu DOĞRUDAN ÇAĞIRAMAZ ve ondan callback ALAMAZ; Crypto, Application'ı çağıramaz.
> Tüm Application↔security etkileşimi Transport ÜZERİNDEN geçer ("mediated" = çağrı grafiğinin sahibi
> Transport'tur; Transport kripto semantiğini YORUMLAMAZ). Transport → Crypto sınırı İKİ arayüz ailesi taşır,
> üçüncü seam değildir: **PacketProtection** `seal(datagram, ProtectionContext) / open(wire)` ve
> **SessionControl** `command(SessionCommand) → SessionResult` (typed komut: Initiate{peer, early_unit?} ·
> AcceptInit{wire} · AcceptResponse{wire} · Renew{session}; zarf payload'ı opak, komut opak DEĞİL).
> Lifecycle olayları (`Ready(h) · RenewRequired(h) · Closed(h, reason)`) typed'dır; Transport yorumlamadan
> Application'a iletir. **`SessionHandle`** Crypto tarafından basılır, Application için opak, Transport
> içeriğini yorumlamaz, tek crypto session state'e çözülür, epoch DEĞİLDİR, wire'da gönderilmez, restart
> sonrası geçerli olmak zorunda değildir, `Closed` sonrası kullanılamaz; **`HubHandle`** (wire routing, ccr
> §35.7) ile AYRI tiptir ve birleştirilemez. Zaman sahipliği: Transport teslim/yeniden-iletim deadline'larını,
> Crypto güvenlik-durumu ömürlerini (candidate expiry, key lifetime, replay retirement, grace, cap) tutar;
> Crypto ağ retransmisyon timer'ı TUTMAZ. `early_unit` istisnası: Application plaintext'i bir kez data-path
> dışında SessionControl'e girer; Crypto onu OPAK byte olarak işler, anlamını yorumlamaz.
>
> **[DR-CONNECT-1 — 2026-10-03, Abkarada] INV-NO-IDLE-TEARDOWN (NORMATİF):**
> ```
> Bir oturum ya da bekleyen niyet, ZAMAN GEÇTİĞİ İÇİN sonlandırılmaz.
> ```
> NİYET (bir eşle oturum kurma / mesaj iletme isteği) zamanla ölmez; DENEME (prob, yeniden gönderim) maliyeti
> sınırlanır (§12 [AMENDMENT-9] K tavanı · §10 [AMENDMENT-10] prob kadansı); sonlandırma yalnız olay-tetiklidir.
> **Sonlandırma nedenleri aşağıda sayılan olaylarla sınırlıdır** (kapalı liste; "zaman" listede YOKTUR, başka hiçbir
> şey oturum kapatamaz): **(a)** kullanıcı/uygulama cancel · **(b)** karşı tarafın close'u · **(c)** anahtar hijyeni —
> `SEND_PN_CAP`, `SESSION_MAX_AGE` (KS-AGE-1), T-18c ephemeral ömrü · **(d)** kalıcı yerel hata — peer material yok,
> sertifika iptal/expire. Tanı durumları (`PATH_DEAD`, §9) sonlandırma nedeni DEĞİLDİR; Application'a yalnız
> `ConnectProgress` olarak görünür (KM-SEAM-AT-001 §4).
>
> **INV-PLAINTEXT-NOT-SEMANTICS (NORMATİF):** *Plaintext görmek ≠ semantik bağ.* Transport, plaintext byte dizisi
> üzerinde **yapısal** işlemler yapar — uzunluk, segment/batch, FEC, deadline, öncelik ipucu — ve bunların hiçbiri
> "bu TEXT mi / FILE_CHUNK mı / protobuf alan #7 mi" bilgisini gerektirmez. Semantik (tip, şema, alan anlamı,
> "bu sıkıştırılabilir mi") YALNIZ Application'ındır. Ayrım korunduğu sürece Transport'un plaintext işlemesi katman
> ihlali DEĞİLDİR. **Serileştirme sınırı:** Application nesneleri → AppMessageCodec → `PreparedUnit{bytes,
> compression_policy}`; Transport → batch/segment/FEC → `UnsealedTransportUnit`; Crypto → `SealedWireUnit`. Crypto
> protobuf bilmez, Transport protobuf bilmez, FEC TEXT bilmez. **CCR / sertifika / public key** yalnız canonical
> kripto codec'iyle (ccr §3) serileştirilir; protobuf ile YENİDEN kodlanmaz (iki canonicalization katmanı yasak),
> generic compressor'dan GEÇMEZ (yüksek entropi, sıfır kazanç, gereksiz transform + side-channel yüzeyi — "byte
> bozulur" diye değil); başka mesajda taşınacaksa opak `bytes` blob'udur. **Private key** Application/Transport
> API'sinden HİÇ geçmez; Crypto/keystore/HSM sınırı içinde kalır.

```
L1  IP/UDP                       UDP/IPv4 = 28B, UDP/IPv6 = 48B base
L2  Hub routing (cleartext)      hub_handle (4B) — hub okur+route eder; E2E DEĞİL
L3  E2E Session Model V3 envelope SESSION_INIT / SESSION_INIT_RESPONSE / SESSION_DATA / SIGNED_CONTROL
                                 (SESSION_DATA: seq16 2B + ciphertext + AEAD tag16) — hub OKUYAMAZ
L4  Session crypto               Session Model V3 key schedule (ccr §35): INIT ss_e+ss_s authenticated
                                 one-way; RESPONSE responder ephemeral upgrade; established epoch/dir/
                                 counter-bound AEAD (split() — Kama-specific Noise-inspired, standart
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
Segment-block/Control framing (Pipeline C/D): per-shard AEAD — ANAHTAR = **session E2E anahtarı** (shard'lar session verisidir; hop anahtarı DEĞİL) [AMENDMENT-6: eski "Noise transport key" ifadesi düzeltildi; hop anahtarı yalnız transport'un hub-yönelik kontrol çerçeveleri ve Plane 1 RPC için]. ProtectionContext'i Transport verir (Session(h) | HubChannel) — bugün SealArgs'ta ctx alanı YOK (kod: {wire_pn, dh_epoch, send_generation}); S3 seam'inde eklenecek [OPEN]. YALNIZCA burada (FEC shard auth).
Hub↔Hub mesh                 : Noise KK (datacenter; routing §6).
```
> **Neden hot-path'te outer Noise yok ama dest_hash yine gizli:** Kimlik-bağlı alan (dest_hash) setup'ta ClientHint ile gizlenir; hot-path'te yalnız opaque hub_handle görünür (kimliğe bağlanamaz). Per-mesaj outer Noise 0.025 kbps'de +16B tag/mesaj = kabul edilemez maliyet; E2E envelope zaten içeriği korur. Bu, "thin hot-path" (TLS-dersi) ile "dest_hash gizli" (Noise ClientHint) kararlarının birlikte tutarlı halidir — çelişki yok, KİLİTLİ.

---

## 1. Mesaj Taksonomisi — Session Model V3 aktif mesajları

> **Canonical app-envelope byte layout'u ccr §35'tedir;** burada transport görünümü + handle + accounting. Dört aktif Session Model V3 mesaj tipi:

```
1. SESSION_INIT
   - zero-RTT N-türevi authenticated one-way bootstrap; Bob online olmak zorunda DEĞİL
   - SESSION_INIT logical olarak recipient_dest_hash'e bağlıdır; fakat client→hub WIRE üzerinde
     recipient_dest_hash CLEARTEXT taşınMAZ — routing hint şifreli ClientHint içinde (hub'a ECDH-encrypted,
     §0.1) taşınır ve hub tarafından hub_handle/mailbox mapping'e çevrilir. Hot-path SESSION_DATA'da
     dest_hash HİÇBİR biçimde taşınmaz. early_data taşıyabilir
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

> **[AMENDMENT-6 not]** Bu 22 B format **Crypto seam'inin ÇIKTISIdır**: `hub_handle`'ı seam yazar (kod: kama-noise-wire legacy.rs:126, AAD dışı; HubHandle atama → HandleAssign, bugün yalnız hub_sim.rs:53-62), transport'a giren birim plaintext'tir (kod: client-core wire.rs:15-30 DATA çerçevesi `[type][seq u16][unit_id u64]` 11 B — interim). Ürün seam'i (IdentitySeam) bugün hub_handle yazmaz — PoC no-op ile tutarlı. Transport bu formatı ÜRETMEZ, yorumlamaz.

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
  ss_e = DH(sender_ephemeral, recipient_static)   → per-INIT ephemeral confidentiality (taze sender ephemeral).
                                                     TAM FS DEĞİL: recipient_static ileride ele geçer + kaydedilmiş
                                                     epoch-0 INIT (sender_ephemeral_pub içerir) → ss_e yeniden
                                                     hesaplanıp epoch-0 trafiği çözülebilir (0-RTT/one-way doğal sınırı).
  ss_s = DH(sender_static,    recipient_static)   → sender auth (statik possession; per-mesaj imza yok). FS SAĞLAMAZ.
  → TAM forward secrecy EPOCH-1'de başlar: SESSION_INIT_RESPONSE responder ephemeral'ini (ee DH) katıp
    ephemeral'ler silindikten SONRA (aşağıdaki FS-upgrade notu). Epoch-0 tek-yön mesajı bu sınırı taşır.
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

> **İlke:** "5 bps'de çalışır" gibi bir **bps-floor** iddiası anlamsızdır — yeterince beklenirse her store-and-forward sistem teorik olarak 0.00001 bps'de bile bir şey ulaştırır (kurulum haftalar/aylar sürer). Doğru metrik **bounded operational envelope**'dur: belirli byte + loss + timeout + kullanıcı beklentisi içinde teslim. Kama **bps floor yarışı yapmaz**; bounded store-and-forward teslim iddia eder.

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
           anonim/otoritesiz. Kama RNS'i physical-layer minimum bitrate yarışında GEÇMEYİ HEDEFLEMEZ.
Kama:  enterprise-controlled hostile-IP communication. Farklılaşma baseband DEĞİL:
           - PKI + revocation + admin control + auditability
           - always-online hub store-and-forward
           - app-level delivery semantics (offline recipient)
           RNS bu ekseni tasarım gereği hedeflemez (sovereign/anonim = ters yön).

Signal/WhatsApp:  TCP+TLS live-session. Yüksek loss'ta TCP çöker (HoL blocking, retransmit storm,
           handshake tamamlanamaz). Kama'nın ASIL kıyası budur: "Signal/WhatsApp sınıfı uygulamaların
           bağlantı kuramadığı hostile-IP rejimlerde bounded store-and-forward teslim." Bu SAVUNULABİLİR.
```
> **Tek cümlelik konum:** *Kama, RNS'i raw baseband'de geçmeyi hedeflemez; iddiası, kurumsal PKI/revocation/audit/admin-control gerektiren hostile-IP ortamlarında Signal/WhatsApp sınıfı uygulamaların bağlantı kuramadığı rejimlerde bounded store-and-forward teslim sağlamaktır.*

---

## 4.4 QUIC Full-Scale Karşılaştırmalı Değerlendirme — Test Metodolojisi (NORMATİF)

> §4.1'in tam açılımı. Amaç: DCP (Dufisson Capital Partners), ASELSAN, TÜBİTAK denetimi ve bağımsız bir transport mühendisinin itirazına karşı **savunulabilir, tekrar-üretilebilir kanıt** üretmek. Kazanma tanımı **kategoriktir (yetenek eşiği)**, marjinal değil: "QUIC'ten %X hızlı" değil, **"QUIC'in çalışamadığı koşulda haberleşmeyi tamamlamak."** Her değerlendirici sorusu → test → metrik → geçme ölçütü olarak eşlenir (checklist: §4.4.12).

### 4.4.0 Değerlendirme Felsefesi — Kategorik vs Marjinal (NORMATİF çerçeve)

```
İDDİA TÜRÜ (kilitli):
  RED  : marjinal üstünlük ("aynı işi %30 daha iyi") — BBR sürümüne kırılgan, operasyonel olarak anlamsız.
  KABUL: yetenek eşiği ("QUIC=0% iken Kama≥%90 bir koşul aralığı var").
         QUIC orada FONKSİYON YOK (0); Kama FONKSİYON VAR (1). Oran değil, ikili durum.

KAZANMA (kategorik):
  Öyle bir koşul bölgesi [X, Y] var ki:
     quinn+BBR completion ≈ 0%   (handshake kuramıyor / oturum sürdüremiyor)
     Kama  completion ≥ %90
  [X,Y] BOŞ DEĞİLSE → GO. Bölge GENİŞLİĞİ = value proposition'ın büyüklüğü (satılan şey bu).

DÜŞÜRÜLEBİLİRLİK (dürüstlük şartı):
  Nerede KAYBEDECEĞİMİZ ÖNCEDEN ilan edilir (§4.4.5): düşük-loss/düşük-jitter = BBR bölgesi.
  Orada QUIC kazanabilir → KABUL → o bölgede quinn adopte (ADR-14). Bu başarısızlık DEĞİL.

ÖLÇÜM > VARSAYIM:
  Tüm eşikler netem ground-truth'a karşı ölçülür. Magic-number YOK:
  birincil ölçüt kategorik VARLIK (aralık boş mu değil mi); ikincil metrik barları
  ilk baseline run'dan KALİBRE edilir ve rapora ÖLÇÜLMÜŞ değerle yazılır (placeholder yasak).
```

### 4.4.1 Karşı Taraf Kurulumu — Adil Kıyas (methodology fairness)

> **Değerlendirici sorusu:** *"QUIC'i dürüst mü kurdun, yoksa strawman mı yendin?"* (İlk sorulan soru; yanlış kurulum tüm benchmark'ı değersiz kılar.)

```
QUIC baseline — PRIMARY LAUNCH GATE (best-available; sakat kurulmaz):
  - best-available production-grade QUIC implementation(s) (ör. quinn / quiche); versiyon PİNLİ + Cargo.lock.
  - En güçlü mevcut congestion control: default CC + destekliyorsa BBR/BBRv2 profili
    (Cubic/Reno'ya karşı kazanmak anlamsız — QUIC'in EN İYİ hâli ölçülür).
  - Stream mode ve gerekiyorsa datagram mode AYRI ölçülür.
  - Kullanılan QUIC impl + CC profile RAPORDA açıkça yazılır (reproduce + "strawman yendin" itirazına karşı).
  NOT: tek bir library/CC varsayımına KİLİTLENME — mevcut en olgun profil seçilir ve belgelenir
       (BBRv2 entegrasyonu bir stack'te olgun değilse benchmark kırılgan olmasın).

Aynı fiziksel koşul (tek değişken = transport):
  - Aynı netem/tc script, aynı MTU=1200, aynı payload dağılımı, aynı ölçüm penceresi,
    aynı donanım/çekirdek/NIC, aynı container image digest. Confound YOK.

SECONDARY EVIDENCE / APPENDIX (GO kapısını BLOKE ETMEZ — scope disiplini):
  - TCP+TLS 1.3, ham KCP, Reticulum (RNS), Signal/WhatsApp sınıfı → ürün konumlandırma + appendix.
  - Layer-matched (RNS disiplini §4.3); "yendik" iddiası yalnız ÖLÇÜLMÜŞ delta ile.
  - PRIMARY launch gate = Kama custom UDP adaptive transport vs best-available QUIC — YALNIZCA bu.
  - Ek baseline'lar Alpha freeze'i GECİKTİRMEZ (destekleyici kanıt, gate değil).
```

### 4.4.2 Test Ekseni ve Profil Matrisi — Koşulu Kırılana Kadar Kötüleştir

> **Değerlendirici sorusu:** *"Profilleri cherry-pick mi ettin? Sınır nerede?"* Cevap: koşul, her iki transport da kırılana kadar süpürülür; sabit "kolay" profil yok.

```
3-EKSEN SWEEP (ADR-18 + genişletme):
  loss        : 0% → 2% → 5% → 20% → 50% → 80% → 95%
  jitter      : düşük → orta → yüksek (+ burst-jitter)
  bandwidth   : çok-düşük (~0.025 kbps) → düşük (~0.1) → orta (~0.2) → normal
  blackout    : yok → 2s → 10s → 30s → intermittent (EW jamming profili)
  reverse-path: sağlam → ACK-yolu bozuk (ARQ-only'nin İKİNCİ cehennemi)
  queue-delay : nominal → bufferbloat inflation

Kartezyen tam-matris pahalı → iki katman:
  (a) FULL grid: loss × jitter (birincil eksen; completion eğrisi buradan çıkar).
  (b) SPOT profiller: her köşe-senaryo (P_worst, P_high, P_bbr, blackout, reverse-path) derin ölçüm.

ÖNCEDEN İLAN (hipotez — düşürülebilir):
  P_worst {loss %80–95, jitter yüksek, bandwidth var} → Kama NET üstün (ARQ-only cehennemi).
  P_high  {loss %20–50}                                → Kama üstün (FEC recovery, RTT-beklemeden).
  P_blackout {intermittent kesinti}                    → Kama üstün (session survival, QUIC re-handshake).
  P_bbr   {loss %2, düşük bant, jitter normal}         → QUIC kazanabilir; KABUL (§4.4.5).
```

### 4.4.3 Birincil Metrikler — KATEGORİK ("kazandık mı?")

> **Değerlendirici sorusu:** *"Hostile ağda hangi metrik anlamlı?"* Cevap: latency/goodput DEĞİL (onlar iyi-ağ metrikleri) — **completion + survival + floor** (operatörün umursadığı: "mesaj gitti mi?").

```
① DEADLINE-BOUNDED COMPLETION RATE (%) — birincil
     VERİLEN DEADLINE İÇİNDE tam teslim edilen mesaj/dosya oranı. Bounded-delivery iddiası zaten zaman
     sınırı içerir → completion deadline'sız ölçülMEZ (2 dk'da giden ile 15 sn'de giden AYNI "success"
     kovasına düşmemeli; aksi halde Kama'nın gerçek avantajı ölçülemez).
     QUIC handshake kuramıyorsa / deadline'ı aşıyorsa → o deadline'da completion = 0%.

② BLACKOUT SURVIVAL (ikili) — birincil
     N-sn TAM kesinti sonrası oturum YAŞIYOR mu (Noise session + link_id + hub route cache 7g, no-keepalive).
     QUIC: idle-timeout → yeni handshake (hostile'da çoğu kez tamamlanamaz) → oturum ölür.
     Kama: aynı oturumdan devam (state korunur).

③ OPERATIONAL FLOOR — birincil (asıl kanıt)
     completion'ın %90 altına düştüğü koşul noktası.
        quinn+BBR floor  : loss = X_quic
        Kama  floor  : loss = Y_sr    (hedef: Y_sr ≫ X_quic)
     Kazanç bölgesi = [X_quic, Y_sr]: "QUIC'in öldüğü ama Kama'nın çalıştığı aralık." Value proposition = bu aralık.

④ HANDSHAKE / FIRST-DELIVERY SUCCESS @ loss — birincil
     QUIC 1-RTT+ handshake %X loss'ta KURULABİLİYOR mu (kurulamıyorsa hiçbir metrik anlamlı değil).
     Kama zero-RTT first-message delivery (§3) — handshake-round beklemez.

SECONDARY-BUT-REQUIRED (Tier-2/Tier-3 kazanımını ölçer — §4.4.10; latency TAMAMEN dışlanMAZ):
     time-to-first-delivery; deadline-altı p50/p95 delivery latency; bytes-per-delivered-message;
     ACK/retransmit overhead; enerji/mesaj. Bunlar "completion=success" kovasını AYRIŞTIRIR
     (15 sn vs 2 dk farkı) ve operasyonel/verim üstünlüğünü niceler. Birincil değil ama ZORUNLU.

GO kararı birincil (kategorik) + secondary-but-required (deadline/verim) metriklerin ÜÇ-TIER kazanma
modeline dayanır (§4.4.10) — yalnız Tier-1'e DEĞİL.
```

### 4.4.4 İkincil Metrikler — AÇIKLAYICI ("neden kazandık" — mekanizma kanıtı)

> **Değerlendirici sorusu (mühendis, black-magic şüphesi):** *"Neden çalışıyor? Ölçümün doğru mu, yoksa şans mı?"* Bu metrikler "kazandık mı"yı DEĞİL, üstünlüğün **mekanizmasını** kanıtlar — iddianın temeli.

```
ESTIMATOR ACCURACY (netem ground-truth'a karşı — çekirdek tez kanıtı):
  bandwidth error : |B_est − B_netem| / B_netem
  RTT/jitter error: |X_est − X_netem| / X_netem
  → Üstünlük "ölçüme dayalı FEC/ARQ kararına" bağlı; ölçüm YANLIŞSA karar da yanlış → QUIC'ten KÖTÜ olurdun.
    Bu yüzden estimator doğruluğu GO'nun ön-şartı (§4.4.10). Bar baseline'dan kalibre, rapora ölçülmüş yazılır.

LOSS ≠ CONGESTION CONFUSION MATRIX (senin tezinin doğrudan kanıtı):
  gerçek-loss'u congestion sanma oranı (false-congestion) — QUIC'in yapısal hatası, senin çözdüğün.
  gerçek-congestion'ı loss sanma oranı (false-loss).
  → "loss ile bandwidth'i ayırıyoruz" iddiası bu matrisle YA DOĞRULANIR ya ÇÜRÜR.

FEC EFFICIENCY (recovery bedava değil):
  overhead ratio : gönderilen parity byte / kurtarılan payload byte
  NET goodput    : parity DAHİL (brüt recovery değil).
  → Recovery kazanımı FEC overhead'ini goodput'ta GERİ ÖDEMELİ. Ödemiyorsa o profilde FEC kapatmak
    DOĞRU karardır (fuzzy bunu SEÇMELİ — §5.3). Bu, fuzzy controller'ın doğruluğunu da test eder.

ARQ EFFICIENCY:
  spurious/gereksiz retransmit oranı; reverse-path-impaired altında davranış (ACK-yolu bozukken çökmüyor mu).

RECOVERY TIME (üstünlüğün fiziksel kaynağı):
  kayıp → reconstruct süresi.
  QUIC: kayıp paketi RTT-bekle + retransmit. Kama: FEC ile RTT-BEKLEMEDEN reconstruct → fark buradan.

FUZZY DETERMINISM (debuggability — ADR-17):
  aynı input snapshot → aynı output (golden vector). Non-deterministik ise transport debug edilemez.
```

### 4.4.5 İyi-Ağ Dürüstlüğü — QUIC'in Kazandığı Bölge (graceful degradation)

> **Değerlendirici sorusu:** *"Normal ağda ne kadar kötüsün? Nerede kaybediyorsun?"* Kaybettiğin yeri SEN gösterirsin — cherry-pick suçlamasının tek panzehiri.

```
P_bbr {loss ~%2, düşük bant, jitter normal}:
  BBR yıllarca optimize + ML-tabanlı varyantlar → QUIC marjinal daha iyi olabilir. KABUL.
  Ölçüt (magic-number DEĞİL — ölçülür ve rapora yazılır):
    - Kama "graceful": ÇÖKME YOK (completion yüksek kalır); QUIC'e göre fark ÖLÇÜLÜR, gizlenmez.
    - O bölgede ADR-14 tetiklenir: quinn adopte (custom motor ısrarı YOK; doktrin "en iyiyi kullan").
  Amaç: iki bölge farklı savaş — Kama bölgesi [X,Y] (hostile), QUIC bölgesi [0,X] (iyi-ağ).
        Değerlendiriciye "her yerde kazanıyoruz" YALANINI söylemeyiz; sınırı biz çizeriz → güven.
```

> **QUIC selected olduğunda semantik (NORMATİF — Option A):** Bir profilde QUIC adopte edilirse Session Model V3 **canonical semantics DEĞİŞMEZ**; değişen YALNIZCA substrate/reliability layer'dır. E2E envelope (Session V3 AEAD), replay/counter, hub_handle/session semantics **KORUNUR**. QUIC **FORK yapılmaz** — production QUIC library as-is kullanılır (ADR-14). Kama kimlik/güven/rendezvous katmanı QUIC'ten bağımsız aynen çalışır; QUIC yalnız reliable taşımayı sağlar (adaptif custom motorun yerine geçer, semantiğin değil).

### 4.4.6 Kaynak Maliyeti — CPU / Bellek / Batarya / ARM (mobil & UAV bütçesi)

> **Değerlendirici sorusu (Baykar UAV, Android mobil):** *"Custom transport ARM'da pahalı mı? Bataryayı/CPU'yu yiyor mu?"* Savunma sahasında güç bütçesi operasyonel kısıt.

```
Ölçülen (QUIC/quinn ile AYNI eksende — adil):
  - CPU cycles/byte : FEC encode/decode (reed-solomon-erasure, ADR-16), fuzzy eval, AEAD.
  - Bellek          : peak/steady buffer pool, MAX_SKIP skipped-key store, jitter buffer.
  - Enerji          : mobil profil (mJ/mesaj, mJ/MB) — Android ARM 8.0+ hedef.
  - Throughput/core ve concurrent-session ölçeği (hub tarafı).
Not: reed-solomon-erasure Alpha'da DOĞRULUK-öncelikli (ADR-16); reed-solomon-simd Beta.
     Alpha maliyeti HEDEF değil ama ÖLÇÜLÜR ve rapora girer (regresyon izlenir).
```

### 4.4.7 Güvenlik Pariteliği — Transport Kriptoyu ZAYIFLATMAZ (defense audit)

> **Değerlendirici sorusu (en sert, savunma):** *"Adaptif FEC/ARQ E2E garantilerini bozuyor mu? Yeni yan kanal açıyor mu?"* Transport, §0.1 kripto invariant'larını koruMAK zorunda.

```
CODE-THEN-ENCRYPT-AND-AUTHENTICATE-SHARDS (kilitli — §0.1, §7 Pipeline B, INV-FEC-1):
  FEC (Reed-Solomon erasure) PLAINTEXT block üzerinde encode edilir → data+parity plaintext shard'lar →
  HER shard AYRI AEAD encrypt+authenticate → wire'da yalnız CIPHERTEXT shard görünür (plaintext/parity görünmez).
  Alıcı: shard al → ÖNCE AEAD verify/decrypt → YALNIZCA authenticated plaintext shard FEC decoder'a girer;
         unauthenticated/tampered shard decoder'a ASLA girmez (authenticate-before-decode, SR-AUDIT-001).
  Bu "ciphertext üstünde FEC" DEĞİLDİR. Güvenlik invariant'ı: wire confidentiality + authenticate-before-decode.
  Test: (a) parity shard'lardan plaintext/uzunluk-ötesi bilgi çıkarılamaz (differential);
        (b) unauthenticated/tampered shard'ın FEC decoder'a GİRMEDİĞİ fuzz ile kanıtlanır (INV-FEC-1).

TRAFFIC-ANALYSIS YAN KANALI (adaptif davranışın riski):
  Adaptif FEC ratio / send-rate içerik TÜRÜNÜ ele veriyor mu? (fuzzy çıktısı gözlemlenebilir mi.)
  Test: FEC-on/off ve rate değişimi ile paket-boyut/zamanlama korelasyonu ölçülür; içerik-ayırt-edilebilirliği sınırı.
  (Sealed-sender Alpha'da yok; hub metadata kabul — ama transport EK sızıntı yaratMAMALI.)
  ⚠️ İSTİSNA (bilinçli, kayıtlı): FEC_SHARD framing metadata (frame_type/block_id/shard_index/k/r/shard_len)
     wire'da PLAINTEXT → EK metadata sızıntısı (k/r = emergency-durumu; block_id = shard linkability).
     Kısmen kaçınılmaz (hub reconstruction için framing bilmeli). ALPHA'DA KABUL; minimizasyon Beta/Beta+.
     Tam analiz + Beta yönü: engine spec §10.1 AUDIT-FINDING METADATA-LEAK-01. Benchmark'ı geciktirmez.

REPLAY / INJECTION @ transport:
  seq + AEAD tag; malformed → sessiz drop (§15); replay window AEAD başarısından SONRA (INV-REPLAY-1).

AMPLIFICATION / DoS:
  Saldırgan FEC-on veya EMERGENCY_FORWARD_REDUNDANCY (§5.6) zorlayıp kaynak tüketebilir mi?
  Test: budget clamp (§5.5) + emergency clamp saldırgan-tetikli inflation'ı sınırlıyor mu.
```

### 4.4.8 Gerçek-Dünya Geçerliliği — netem Ötesi (procurement: saha kanıtı)

> **Değerlendirici sorusu:** *"Bu netem lab kurgusu mu, gerçek ağda çalışıyor mu?"* Savunma alımı saha kanıtı ister.

```
SIDE-BY-SIDE FIELD POC (1–2 gün, ölçülmüş rapor — satış metodolojisi):
  Gerçek hostile ağ: kırsal LTE/kenar hücre, jammer-sim, sınır/dağ senaryosu.
  Signal/WhatsApp + QUIC vs Kama PARALEL, aynı anda, aynı mesaj seti → ölçülmüş completion raporu.

MIDDLEBOX / DPI SURVIVAL:
  Kama throttle/blok yeniyor mu; QUIC-benzeri fingerprint mi (DPI kategorize edip kısabilir mi).

NAT TRAVERSAL + MTU DISCOVERY hostile path (fragmentation davranışı, §11).

MEDYA-TRANSPORT SEAM INTEROP (cross-repo):
  Signaling (SDP offer/ICE candidate) SENİN transport'undan geçer → medya stack (WebRTC fork) devralır.
  Test: signaling teslimi hostile'da tamamlanıyor → çağrı kurulabiliyor (seam kontratı çalışıyor).
```

### 4.4.9 İstatistiksel Sağlamlık + Tekrar-Üretilebilirlik (audit reproducibility)

> **Değerlendirici sorusu:** *"Tek atış mı? Ben bağımsız tekrar koşabilir miyim?"* Denetim = bağımsız doğrulanabilirlik.

```
İSTATİSTİK:
  Her hücre N ≥ 30 run; median + p95/p99 + %95 güven aralığı; variance raporu.
  Fuzzy determinism variance'ı AYRI raporlanır (ADR-17: aynı input → aynı output; ağ variance'ından ayrık).

TEKRAR-ÜRETİLEBİLİRLİK (tek komut):
  Seeded RNG; versioned harness (git commit); netem/tc script commit; quinn/lib versiyon pin;
  container image digest. `make bench PROFILE=...` bağımsız reproduce eder.

CI (ADR-18 kalite kapısı):
  Nightly full-matris; regresyon (bir profilde düşüş) build'i kırar.
  Ham veri + script + grafik üretici YAYIMLANIR → denetçi (DCP/ASELSAN/TÜBİTAK) bağımsız doğrular.
```

### 4.4.10 GO / NO-GO Karar Kapısı (launch criteria — NORMATİF)

```
GO (custom transport TUTULUR) — (A) EN AZ BİR kazanma tier'ı + (B) TÜM guardrail'ler:

  (A) KAZANMA — üç tier (en az biri; Tier-1 en güçlü):
      Tier-1 CATEGORICAL WIN:
        [X,Y] boş DEĞİL — QUIC completion≈0 iken Kama deadline-bounded completion ≥%90
        (P_worst / P_high / P_blackout). En güçlü iddia; killer-chart bölgesi (§4.4.11).
      Tier-2 OPERATIONAL WIN:
        QUIC completion >0 olsa bile deadline/SLA İÇİNDE başarısız (ör. 2 dk'da teslim = geç),
        Kama deadline içinde ≥%90. VEYA QUIC handshake kuruyor ama blackout-survival /
        offline-recipient store-and-forward semantiğini SAĞLAMIYOR, Kama sağlıyor.
      Tier-3 EFFICIENCY WIN:
        İkisi de deadline içinde teslim ediyor ama Kama aynı bounded envelope'ta ÖLÇÜLEBİLİR
        biçimde daha düşük byte/ACK/energy bütçesiyle.
        NOT: Tier-3-ALONE custom motoru YALNIZCA verim OPERASYONEL olarak belirleyiciyse gerekçeler
        (ör. tactical bandwidth/energy bütçesi QUIC overhead'iyle AŞILIYOR, düşük-güç UAV/mobil kısıt) —
        aksi halde salt marjinal verim custom-motor karmaşasını haklı çıkarmaz → ADR-14 quinn
        (marjinal-üstünlük tuzağı; §4.4.0 kategorik ilke). Tier-1 veya Tier-2 tek başına yeterli.

  (B) GUARDRAIL'ler (kazanılan tier ne olursa olsun HEPSİ gerekli):
      [G1] MEKANİZMA: estimator accuracy + loss/congestion confusion baseline barını tutar
                      (üstünlük TEMELLENDİRİLMİŞ; şans değil).
      [G2] GÜVENLİK : §4.4.7 tüm testler geçer (transport §0.1 kripto invariant'larını BOZMAZ; yeni yan kanal yok).
      [G3] İYİ-AĞ   : §4.4.5 graceful (çökme yok); kaybedilen bölgede ADR-14 fallback tanımlı.

NO-GO (bir koşul SINIFINDA):
  O sınıfta quinn adopte (ADR-14). Başarısızlık DEĞİL — önceden kabul (§4.4.0 düşürülebilirlik).

GLOBAL NO-GO (custom motor TÜMDEN):
  [X,Y] BOŞSA (hiçbir hostile bölgede QUIC kategorik yenilemiyor) → tüm reliable transport quinn'e devredilir.
  Doktrin dürüstlüğü: bu sonucu da KABUL ederiz. Custom motor bir inanç değil, ÖLÇÜLMÜŞ bir gerekçedir.
```

### 4.4.11 DCP / ASELSAN / TÜBİTAK Sunum Artefaktı — The Killer Chart

```
GRAFİK 1 — completion-vs-loss (iki eğri, İTİRAZ EDİLEMEZ):
  x = loss% (0→95), y = completion%. quinn+BBR eğrisi bir noktada UÇURUMDAN düşer; Kama eğrisi DEVAM eder.
  İki eğri arasındaki ALAN = value proposition (görsel, tek slayt, biten tartışma).

GRAFİK 2 — operational envelope map (2D):
  x = loss, y = jitter; renk = hangi transport %90 completion'ı SÜRDÜRÜYOR.
  Kama zarfı QUIC zarfını KAPSAR; fark alanı = Kama'nın tek başına ayakta olduğu bölge.

EKLER:
  - per-profile sonuç tablosu (completion/survival/floor + estimator accuracy + FEC efficiency).
  - reproducibility appendix (§4.4.9) + ham veri linki.
  - güvenlik-paritelik özeti (§4.4.7) — savunma denetçisi için.
```

### 4.4.12 Değerlendirici Soru → Test Eşleme Tablosu (audit checklist)

| Değerlendirici / Mühendis sorusu | Cevaplayan test | Metrik | Geçme ölçütü |
|---|---|---|---|
| QUIC'i dürüst mü kurdun? | §4.4.1 | quinn+BBRv2, versiyon pin, aynı netem/MTU/payload | Karşı-taraf spec'e uygun; confound yok |
| Profilleri cherry-pick mi ettin? | §4.4.2 | tam sweep + önceden ilan hipotez | Kaybedilen bölge önceden ilan edilmiş |
| Hostile'da hangi metrik önemli? | §4.4.3 | completion / survival / floor | Kategorik metrikler tanımlı |
| Nerede QUIC'i yeniyorsun? | §4.4.3 ③ | operational floor [X,Y] | [X,Y] boş değil |
| Neden çalışıyor (mekanizma)? | §4.4.4 | estimator accuracy + confusion matrix | Bar tutuyor (ölçülmüş) |
| Ölçümün doğru mu? | §4.4.4 | estimator error vs netem ground-truth | Baseline bar altında |
| FEC'i bedava mı sandın? | §4.4.4 | FEC efficiency (net goodput) | Recovery, overhead'i geri ödüyor |
| Normal ağda ne kadar kötüsün? | §4.4.5 | P_bbr fark ölçümü | Graceful; ADR-14 fallback |
| ARM'da pahalı mı (UAV/mobil)? | §4.4.6 | CPU/byte, bellek, enerji | quinn ekseninde ölçülmüş |
| E2E'yi/kriptoyu bozuyor mu? | §4.4.7 | code-then-encrypt-auth-shards, authenticate-before-decode, yan kanal, replay, DoS | §0.1/INV-FEC-1 invariant korunur |
| netem mi, gerçek ağ mı? | §4.4.8 | Side-by-Side field POC | Gerçek hostile ağda completion>0 |
| Tek atış mı, reproduce edebilir miyim? | §4.4.9 | N≥30, CI, versioned harness | Bağımsız reproduce |
| GO/NO-GO nasıl kararlaşır? | §4.4.10 | 4-şart launch criteria | Kategorik + mekanizma + güvenlik + iyi-ağ |

---

## 5. Fuzzy Controller — FEC/ARQ/CC Karar Mekanizması (NORMATİF çekirdek)

> Sert eşikli state machine yerine fuzzy controller: eşik uçurumu ve osilasyon yok, yumuşak geçiş.

### 5.1 Girdiler (fuzzified) — bandwidth ≠ loss
```
loss_rate      ∈ {LOW, MED, HIGH}   (loss_pre_fec; residual_after_fec ayrıca)
jitter         ∈ {LOW, MED, HIGH}   (rttvar / reordering_score)
bandwidth      ∈ {LOW, MED, HIGH}   (delivered-rate sampling — OBSESİF ölçüm, loss'tan TÜRETİLMEZ)
queue_delay    ∈ {LOW, MED, HIGH}   (srtt - rtprop_min; congestion sinyali)
ack_confidence ∈ {LOW, MED, HIGH}   (delivered-ACK / beklenen-ACK örneklemesi = dönüş yolu sağlığı; §5.6'da EMERGENCY karar girdisi)
```
> **NORMATİF (delivered-rate sampling validity):** ACK compression / app-limited / reverse-path impaired sample'lar bw_est'i ŞİŞİREMEZ; bunlarda estimator_confidence düşer (Section 8). bandwidth daima delivered-bytes/time'dan, loss'tan DEĞİL.

### 5.2 Çıktılar (defuzzified)
```
fec_parity_ratio     [0..1]   (0 = FEC yok; Reed-Solomon erasure parity oranı)
arq_aggressiveness   [0..1]
send_rate (pacing)   bps      (token-bucket)
compress             {on/off}   [AMENDMENT-4: RETIRED — transport sıkıştırmaz; alan kalıntı, tüketilmez]
```

### 5.3 Kural Tabanı (anahtar kurallar)
```
loss LOW                          → fec=0,    arq=light,  rate↑,            compress=—(retired, AMENDMENT-4)
loss HIGH, bw HIGH, jitter LOW    → fec=high, arq=moderate (RS erasure),    compress=—(retired, AMENDMENT-4)
loss MED,  bw MED                 → fec=med,  arq=med
loss LOW,  jitter HIGH            → fec=0,    arq=reorder-tolerant (RTO↑, fast-rtx↓)   [v1.1: RTO↑ ≤ RTO_MAX = 30 s — §12 AMENDMENT-12]
bw LOW, queue_delay HIGH          → rate↓ (congestion; fec CAP)
loss HIGH, bw LOW, jitter HIGH    → mode = EMERGENCY_FORWARD_REDUNDANCY (§5.6), rate=minimal-reliable,
  (EN KÖTÜ — NORMATİF)              (compress: transport dışı, AMENDMENT-4).  Doktrin: "ulaştırmak > hızlı ulaştırmak."
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
    → GEÇİŞ KARARLILIĞI: asimetrik retreat-hysteresis ZORUNLU — Schmitt eşikleri (escalate health<X,
      de-escalate health>Y, Y>X; bant gürültüyü soğurur) + ASİMETRİK dwell (escalate ANINDA —
      geç koruma = kayıp; de-escalate min_dwell [CAL] — erken iniş→re-escalate osilasyonunu keser).
      Gerekçe+detay: engine spec §11 (KİLİTLİ). Marjlar [CAL:netem].
```
> Bu mod ne "hep ARQ" ne "hep FEC"dir; **ACK-bağımsız forward kurtarma**dır ve path iyileşince fuzzy tarafından geri çekilir — geri çekilme **asimetrik hysteresis'le** (yukarıda; tek-eşik salınımı yasak). `parity/duplication` yine `available_redundancy_budget` ile clamp'lenir (§5.5). **[AMENDMENT-3]:** bu cümle §11 MinimalParity istisnası için BAĞLAYICI DEĞİLDİR — istisna kendi üç kapısıyla sınırlıdır ve §5.5 bütçesine tabi değildir; normatif metin engine-spec §11 AMENDMENT-3 ve `docs/transport-v1/adr/ADR-minimal-parity-exception.md`.

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
     counter'ı session_id_internal + direction + ~~epoch +~~ replay window + expected counter ile reconstruct eder.
     ~~replay identity = (session_id_internal, direction, epoch, full_counter)~~
     [2026-10-05] replay identity = (session_id_internal, direction, full_counter); replay window AEAD
     başarısından ÖNCE güncellenmez. `epoch` kimlikten ve geri kazanımdan ÇIKTI: sayaç epoch'lar boyunca
     sürekli, replay penceresi yön başına TEK (identity §15.5, 2026-10-06; ccr §35.8); `epoch` yalnız
     anahtar ve AAD seçimine girer.
REKEY invariants (ccr §35 ile birebir): MAX_MESSAGES_PER_EPOCH = 2^32; REKEY_THRESHOLD <= 2^31;
     ~~future gap / reorder pencereleri < 2^15~~ **[D-GAP-1 2026-09-23]** far-future/reorder KAPILARI ccr §35.8'de KALDIRILDI.
     Kalan kısıt SABİTE değil GERÇEKLEŞEN MESAFEYE uygulanır: |full_counter − expected| < 2^15 (seq16 reconstruction
     tek-anlamlı kalsın) ve bunu korumak GÖNDERENİN işidir → [AMENDMENT-9] / KS-PN-1. Değerler ccr §35 authoritative.
```

---

## 7. Pipeline'lar (framing-driven — NORMATİF)

> **NOT (SoC):** Pipeline ayrımı **framing/boyut** kaynaklıdır (tek datagram mı, segment-block mı) — uygulama rolü (chat/file/"hot-path") DEĞİL. Recovery stratejisi (ARQ/FEC/karma/none) HER pipeline'da fuzzy controller'dan gelir (§5); transport hangi uygulama olduğunu bilmez.

```
A — Tek-datagram payload (MTU'ya sığar):  envelope → SRC pacing/batching/~~N-msg ACK window~~ → UDP   [AMENDMENT-8]
    outer AEAD/FEC/prefix = 0B; auth = inner E2E tag; demux = hub_handle.
    Recovery: fuzzy — loss düşük/bant dar → ARQ (selective retransmit); tek küçük mesajda FEC uygulanamaz
    (block yok) → yüksek loss'ta duplication/ARQ. Strateji koşuldan çıkar.
B — SESSION_INIT/resync:    Noise handshake+early_data → retry scheduling → SRC pacing → UDP
C — Segment-block payload (MTU'yu aşar → shard'lara bölünür):
    Gönderen: block_plaintext → (fuzzy FEC seçtiyse) Reed-Solomon encode → plaintext shard'lar → her shard AYRI
              AEAD-encrypt/authenticate → gönder.
    Alıcı:    şifreli shard al → her shard'ı ÖNCE seam.open (session E2E anahtarı) → yalnızca authenticated shard'lar FEC
              decoder'a girer (transport reconstruct) → eksik shard'lar reconstruct.
    NORMATİF: Authenticated olmayan byte FEC decoder'a GİRMEZ (SR-AUDIT-001). Recovery: fuzzy —
              bant var+loss yüksek → FEC ağır; bant dar+loss düşük → FEC=0, ARQ retransmit; karma mümkün.
    Per-shard AEAD — anahtar = **session E2E anahtarı** (AMENDMENT-6; ~~Noise transport key~~ ifadesi yanlıştı: shard session verisidir) — block framing gereği (FEC shard auth); tek-datagram'da YOK. RS encode transport'ta, seal seam'de (AMENDMENT-5).
D — CONTROL/HOP:            control → per-hop Noise/TLS/AEAD → SRC → UDP/QUIC (deployment)
```
> Tek generic pipeline tüm payload'lara KÖR uygulanmaz. FEC/outer-AEAD **framing-specific** (block'ta mümkün; tek-datagram'da değil). Tek adaptif transport motoru tüm pipeline'larda; ayrı segment-block motoru YOK.

---

## 8. Sıkıştırma — TRANSPORT'UN İŞİ DEĞİLDİR [AMENDMENT-4]

> **[AMENDMENT-4] (sahip kararı 2026-09-17; gerekçesi AMENDMENT-6 ile düzeltildi — ~~iki-katmanlı kripto topolojisi~~ iptal):**
> **Sıkıştırma transport'un işi DEĞİLDİR.** ~~transport ciphertext görür (E2E kripto transport'un ÜSTÜNDE)~~
> [AMENDMENT-6 gerekçe düzeltmesi: kanonik mimaride Transport PLAINTEXT görür, dolayısıyla sıkıştırma
> transport'ta katman ihlali olmazdı; karar yine app'te kalır, gerekçe farklı: (1) ses/video "çift sıkıştırma
> yok" kuralı TİP bilgisi ister, Transport tip bilmez (INV-ENGINE-SOC); (2) kodda aktüatör YOK — Claude Code
> kanıtı: FusionDecision.compress üretici fusion.rs:551, tüketici yok, Cargo.lock'ta sıkıştırma crate'i yok,
> ON/OFF anahtarı yok → TDD §8'in eski "default ON, fuzzy çıktısı" metni hiç uygulanmamıştı.]
> Sahibi **application/RPC mesaj-hazırlama
> alt-katmanıdır** (`application-protocol-and-rpc-spec.md` §3, karşı-pointer orada). **Fuzzy
> controller sıkıştırma kararı VERMEZ.** Bu bölümün önceki normatif metni (zstd, metin için default
> ON, fuzzy çıktısı) ve CRIME analizi ile INV-COMPRESS-1 **oraya TAŞINMIŞTIR**; burada normatif
> hüküm kalmamıştır. Donmuş transport kodunda sıkıştırma YOKTUR (zstd bağımlılığı yok);
> `FusionDecision.compress` alanı üretilen ama HİÇBİR yerde tüketilmeyen kalıntı bir çıktıdır —
> ADR-17 golden-vektör kararlılığı için v1'de bırakılmıştır, kaldırılması v1.x temizliğidir
> (`docs/transport-v1/EVOLUTION.md` §9.8).

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

> **[AMENDMENT-10 — 2026-10-03, DR-CONNECT-1 H3: HUB-KANALI PROB KADANSI · NORMATİF · UYGULANDIĞINDA DAVRANIŞ DEĞİŞTİRİR]**
>
> **Kural (NORMATİF):** Hub-kanalı prob'u backoff'la SÜRER ve pes etmez (INV-NO-IDLE-TEARDOWN, §0) — ama deneme
> maliyeti sınırlıdır:
> 1. **Backoff:** ilk vade 500 ms, her emisyonda ×2 (mevcut davranış).
> 2. **Tavan BAĞLAMA göre:** ön plan **8 s** (mevcut) · arka plan **25 s** · ekran kapalı/doze **OS takvimi**.
> 3. **Askıya alma:** link (arayüz) YOKSA prob tamamen askıya alınır — **0 emisyon**. Prob, ancak ölçülecek bir yol
>    varken anlamlıdır.
> 4. **Anında prob:** link gelince · uygulama ön plana gelince · kullanıcı dokununca → backoff beklenmeden HEMEN.
> 5. **Jitter ZORUNLU:** backoff vadelerine jitter eklenir (thundering herd: hub geri gelince çok sayıda istemci aynı
>    saniyede koşmasın). **[2026-10-04 — RNG'siz, sahip kararı]** ~~`jitter = hash(client_id ‖ deneme_sayacı) mod
>    jitter_penceresi`~~ **[K4 — 2026-10-04b]** `jitter = BLAKE2s-256(seed ‖ u32_be(deneme))[0..4] mod pencere`
>    (ilk 4 bayt u32 big-endian okunur; ADR-05). **`seed`** = `hub_handle` (bind sonrası, bacak başına) /
>    hop handshake'inin yerel ephemeral public anahtarı (bind öncesi). **Tohum `dest_hash` DEĞİLDİR:** statik
>    kimlik tohum olursa cihaz her ağda aynı prob zamanlama desenini üretir → hostile gözlemci IP değişimleri
>    boyunca cihazı bağlayabilir; `hub_handle` tasarım gereği correlation-resistant'tır, oturumlar arası bağlanamaz.
>    `pencere` (`jitter_penceresi`) = backoff adımının **%25'i** `[KALİBRASYON — bu değerle KAPALI]`.
>    İstemciler arası dekorelasyon sağlar, aynı istemcide tekrarlanabilirdir; transport determinizmi (aynı girdi → aynı
>    çıktı) korunur, golden testler yaşar. Motor RNG TUTMAZ.
> 6. **[2026-10-04] Sinyal yokken:** link/power sinyali ürün/Android işidir ve bugün yoktur (K5; `docs/PARK-LISTESI.md`
>    P-4). O gelene kadar kadans ÖN-PLAN davranışıdır (8 s); askıya alma sinyali yoksa askıya alma da YOKTUR.
> `PATH_DEAD` (§9, bu bölümdeki "tekrarlı no-ACK → PATH_DEAD") bir TANI durumudur, sonlandırma değil: prob sürer.
>
> **HOP HEARTBEAT — DR-CONNECT-1 H4 (2026-10-04, sahip kararı: YENİ CODEPOINT · NORMATİF · SPEC-ONLY):**
> Hub-kanalı prob'unun taşıyıcısı ayrı bir çerçevedir; tek paket üç iş görür: **prob = hop heartbeat = NAT keepalive**.
> İstemci tek bir hub'a bağlıdır; UDP NAT eşlemesi client→hub yönünde tektir ve ~30 s'de düşer; inbound DELIVER ancak
> NAT açıksa ulaşır. `CS_PROBE`/`CS_ECHO`'ya BİNDİRİLMEZ: o soğuk-başlangıç ölçüm merdivenidir (bütçeli, 4 atış, u16
> sayaç sarmaz — INV-CS-7); heartbeat sürekli canlılık + NAT keepalive'dır (sınırsız). İki mekanizma, iki invariant.
> ```
> HEARTBEAT      frame_type = 0x1B   body: boş (0 B)  VEYA  echo_token u32_be (4 B)
> HEARTBEAT_ACK  frame_type = 0x1C   body: gelen HEARTBEAT ile AYNI uzunlukta; echo_token varsa AYNEN yansıtılır
> body_len ∉ {0, 4} → reject. Fixed-width big-endian (ADR-05; varint yok). ACK boyutu = istek boyutu (amplifikasyon ≤ 1).
> Bağlam : ProtectionContext::HubChannel (KM-SEAM-TC-001 K1) — hop anahtarı + hop PN uzayı.
>          Session ctx PN'i TÜKETMEZ → [AMENDMENT-9] K tavanına sayılmaz.
> Sayaç  : KENDİ SAYACI YOKTUR — hop mührünün PN'i yeterlidir.
> Token  : opsiyonel u32 echo_token yalnız RTT eşlemesi içindir; sarması serbesttir, replay anlamı YOKTUR.
> ```
> **Kurallar:** **(a) REFRESH-ONLY** — heartbeat hub'da yalnız MEVCUT PeerSlot'u yeniler; bilinmeyen eşten gelirse SESSİZ
> düşer; slot YARATMAZ, dolayısıyla sahte heartbeat DoS yüzeyi değildir. (Bugünkü alıcının "ilk geçerli datagram slot
> yaratır" davranışı — `runtime.rs:2258-2278` — heartbeat için GEÇERLİ DEĞİLDİR; bu ayrı bir kuraldır.) · **(b)** CS probe
> bütçesinin (INV-CS-2) ve INV-CS-7'nin kapsamı DIŞINDADIR. · **(c)** heartbeat RTT'si OBSERVATION-ONLY'dir; fuzzy
> kontrolcüye ve kalibrasyona GİRMEZ. · **(d)** ölçüm KANALA aittir, oturuma değil: 16 bekleyen niyet = 1 heartbeat akışı.
> · **(e)** E2E DEĞİLDİR: eşin (Bob'un) canlılığı hakkında bilgi vermez (identity OQ-14 AÇIK kalır).
> **Kadans = madde 2 → [KS-HB-1 KAPANDI: ön plan 8 s · arka plan 25 s · doze OS takvimi]** (25 s = tipik UDP NAT tavanının
> altı; WireGuard persistent-keepalive mantığı).
> **Codepoint çakışma denetimi (2026-10-04, tüm registry'ler tarandı):** `0x1B`/`0x1C` engine-spec §1.1'de boştu ·
> noise-wire §2.56 E2E `inner_kind` registry'sinde `0x1B..0xFF reserved(reject)` aralığında KALIR (heartbeat E2E
> inner_kind DEĞİLDİR; Session ctx'te görülürse fail-closed reject) · client-core v0 kabuğu (`0x00–0x07`),
> `kama-transport-schema` ve app-rpc `method_id`'leriyle çakışmaz.
> **Kod durumu:** `HubChannel` ctx kodda yok (PoC no-op; KM-SEAM-TC-001 §8) → spec burada HubChannel yazar, kod A4.1/D1
> ile yetişir.
>
> **Kanıt (2026-10-03, kod okuması — `docs/sprint_plan/A1-KALIBRASYON-KARAR-FOYU.md` KANIT PAKETİ 2):**
> - **K3 — jitter bugün YOK.** Establish backoff'unda jitter yok (`sender.rs:5860-5925`); `adaptive-transport`
>   crate'inde RNG bağımlılığı hiç yok (motor deterministik). Jitter eklenirken determinizm korunacaksa jitter değeri
>   ~~motora dışarıdan GİRDİ olarak verilmelidir — uygulama kartının tasarım kararı, burada SEÇİLMEDİ.~~
>   **[2026-10-04 KARAR]** RNG'siz türetilir: madde 5.
> - **K5 — link/power sinyali bugün YOK.** `client-core` ve `adaptive-transport`'ta link-state / power-state girdisi
>   yok; repoda UniFFI bağlaması yok. Madde 2–4 yeni bir girdi yüzeyi gerektirir (ADR-15: coarse-grained UniFFI).
> - Bugünkü "prob" bağımsız bir çerçeve DEĞİLDİR: PreConverge'de ilk DATA'nın §3.3-B kopyasıdır; kopya kuyruğu boşsa
>   hiç kurulmaz (`sender.rs:5871`). Bekleyen veri yokken prob'un taşıyıcısı = ~~DR-CONNECT-1 **H4 (AÇIK — sahip kararı)**~~
>   **[2026-10-04 KARAR]** `HEARTBEAT 0x1B` / `HEARTBEAT_ACK 0x1C` (yukarıdaki HOP HEARTBEAT bloğu).
>
> **EVOLUTION §22 — ALTI BEYAN (bu amendment UYGULANDIĞINDA):**
> 1. Donmuş invaryant: INV-RATE-1 DEĞİŞMEZ. Motor determinizmi (aynı girdi → aynı çıktı) KORUNMALIDIR (bkz. K3).
> 2. `KamaV1Profile`: yeni alanlar (bağlama göre prob tavanı, jitter, heartbeat) EKLENİR — bugünkü profilde YOKTUR.
> 3. Profil kimliği: `kama-v1-full` KORUNAMAZ → `transport-v1.x`.
> 4. Geçersiz olan T-hücreleri: [AMENDMENT-9] ile aynı küme (T1a, T1b, T6 + PreConverge'in uzun kaldığı her hücre);
>    ön plan tavanı değişmediği için fark jitter kaynaklıdır.
> 5. Yeniden koşulacak doğrulama: ADR-hostile-validation-contract ile T0 (regresyon) + T1a/T1b/T6, N ≥ 30, ön-kayıtlı.
> 6. Sürüm: `transport-v1.x` (v2 DEĞİL — mimari değişmiyor).
>
> **DURUM: SPEC-ONLY.** Bu turda Rust'a DOKUNULMADI.

---

## 11. MTU — Hostile-Safe (NORMATİF)
```
default path_mtu_target = 1200B (fragmentation kaçınımı); probe 576/768B; lowband 512–768B;
1900B YALNIZCA path validation sonrası. Küçük paket > fragmentation. Artış opt-in; azaltma ANINDA.
```

---

## 12. ACK Stratejisi (windowed — per-paket ACK DEĞİL)

> **[2026-10-06 — ACK KİMLİĞİ, Abkarada kararı; WS-D1]** ACK_FRAME_V2 kimliği = `full_wire_pn`; `dh_epoch` kimliğe girmez; dayanak: sayaç epoch'lar boyunca sürekli (noise-wire §2.5, §4 EPOCH-PN-INIT, 2026-10-06). Hub ACK'i cleartext PN'den üretir ve epoch'u bilmez; sürekli sayaçta bir PN değeri (session, direction) içinde tek bir datagrama karşılık gelir.

> **[AMENDMENT-12 — 2026-10-24, v1.1 · K4G-05b: YENİDEN-GÖNDERİM SAHİPLİĞİ + RTO_MAX · NORMATİF · DAVRANIŞ DEĞİŞTİRİR · TEL DEĞİŞMEZ]**
>
> **Dayanak:** sahip kararları CEVAP PAKETİ 4 ve 5 (2026-10-16), CEVAP PAKETİ 6, 7 ve 7-EK (2026-10-18), CEVAP PAKETİ 8
> (2026-10-19), CEVAP PAKETİ 9 (2026-10-24), CEVAP PAKETİ 10, 11, 11-EK, 12 ve 13 (2026-10-24) — `docs/decisions/K4G-CEVAP-PAKETLERI-4-7.md`; tasarım notu
> `docs/design/K4G-05b-RETX-FIX.md`.
> **INV-NO-ORPHAN (sahip metni, CEVAP PAKETİ 5, 2026-10-16):** Onaysız hiçbir Reliable birim sahipsiz kalamaz: her anda ya yeniden gönderim
> zamanlayıcısı, ya kopya kuyruğu, ya kuyruk-probu onu izler; izleyen yol kaybı ilan edebilmeli ve birimi yeniden
> gönderebilmelidir. Kuyruk-probu kopyası da kayıp ilan edilebilir ve yeniden planlanabilir.
> **Kabul (NORMATİF, CEVAP PAKETİ 9, 2026-10-24):** her onaysız Reliable birim için üç koşul birlikte sağlanır — (i) birim
> kayıtlıdır (bir izleyen yolun kaydı vardır); (ii) payload'u mevcuttur; (iii) hizmet verilebilir ağ ve gönderim
> bütçesinde yeniden seçilebilir (kaydı BURST_CAP tabanınca kapsanır, sıra başına geldiğinde rezervasyon ve token birikimiyle
> gönderilir — engine §13 v1.1 kaydı). (i) ve (ii) (iii)'ü sağlamaz: token reddi altında bekleyen kopya kaydı ilerleme
> kanıtı değildir. Payload yalnız teyit çıktısıyla silinir — runtime tarafı 05b; birim başına O(1) kayıt ve K_MAP
> budaması 05c (kartlar K4G-05b / K4G-05c; teyit çıktısı aşağıda; CEVAP PAKETİ 11-EK, 2026-10-24, §3; CEVAP PAKETİ 13,
> 2026-10-24, §3). Başka hiçbir yol silmez; ara durum yoktur. Motorun kaydı unutabildiği her yol (K_MAP budaması,
> tombstone süre dolumu, kapasite tahliyesi, başka varsa) için geç ACK, motor ile runtime arasında payload sahipliği
> ayrışması yaratmaz.
> **Okuma (NORMATİF):** kapsam motorun taşıdığı her birimdir. Sessizlik kayıp İLAN ETMEZ (INV-001): sessizlikte izleyen
> yol birimi canlılık için yeniden gönderir (kopya; loss sayılmaz); later-ACK kanıtı gelince izlenen kayıt — kuyruk-probu
> kopyası dahil — mevcut dedektörle kayıp ilan edilir ve yeniden planlanır. ARQ_RETX_CAP aşımı (her politika) birimi
> kopya kuyruğuna devreder, ASLA düşürmez. **Prob kapsamı** = kuyruk-probu zamanlayıcısı kurulu ve bütçesi tükenmemişken
> uçuşta kaydı olan her teyitsiz birim; bütçenin tükendiği planlama turunda kapsamdaki, geometrik kopya kaydı olmayan
> her birim kopya kuyruğuna hemen devredilir (kaydın vadesi = devir anı; birim o turda ikinci kez gönderilmez); devir ACK
> sessizliğine DAYANMAZ. **Sahipsizlik süresi** H = max(ACK_SILENCE_SRTT_MULT·srtt, K_CONF_EAI·EAI) × 1000 /
> `HysteresisConfig::escalate_permille` (kodun ack_confidence eşiği; saf sessizlikte RECOVER_FWD'nin girdiği an; eşik
> yeniden kalibre edilirse H onu izler): prob kapsamı DIŞINDAKİ birim (prob kapalı ya da bütçe tükendikten sonra
> uçuşa giren), ilerleme olmadan (ilerleme = en az bir iletim kaydını ilk kez onaylayan ACK; bayat ACK sayılmaz) son
> iletiminden beri H geçince kopya kuyruğuna alınır. H'nin çapası ilerlemedir: alıcının sessizlikte tekrarladığı bayat
> SACK RECOVER_FWD girişini erteleyebilir, H sahiplenmesini ertelemez. Değişmez RECOVER_FWD girişine ve ACK sessizliğine
> DAYANMAZ. İzleme teyide ya da §0 kapalı listesindeki bir sonlandırmaya dek sürer (INV-NO-IDLE-TEARDOWN). Teyit =
> birimin bir kaynak iletim kaydının canlıyken ACK'i ya da FEC RECOVERED bildirimi; FEC onarım sembolünün ACK'i kaynak
> birimi teyit ETMEZ (INV-004). Birim kimliği, iletim seq'i ve FEC (blok, ESI) ayrı tutulur; ACK, tombstone, un-loss,
> kuyruk-probu hedefi ve RECOVERED eşleşmelerinin HEPSİ bu ayrıma uyar. FEC onarım sembolü Reliable birim DEĞİLDİR:
> sahiplenilmez, ARQ'ya ve kuyruk-probuna girmez; kaydı kendi ACK'iyle kapanır, kayıp ilanında tombstone'a geçip mevcut
> TTL'le (K_TOMB × T_loss) düşer, ilan yoksa gönderiminden K_MAP × RTO_MAX sonra Expired olarak düşer (parite kaydı
> için bellek sınırı; birim geç-ACK garantisi değildir). Teyit edilmiş
> birimin kalan kayıtları yalnız muhasebeye girer (dedektör, ledger ve kader-kanıtı v1 gibi); ARQ, kuyruk-probu (hedef
> seçimi ve emisyon), canlılık, sahiplenme ve kopya yolları teyitli birim için karar ÜRETMEZ. Birim başına ≤ 1
> geometrik kopya kaydı ve ≤ D − 1 bekleyen tek seferlik kopya kaydı (ekleme anında sayılır; D ≤ D_max = 3); birim
> geometrik kayda sahipken sahiplenme yolları kaydın vadesine ve aralığına dokunmaz, vadeyi yalnız later-ACK kayıp
> kanıtı öne çeker. Bu kayıtların getirdiği iletimler ortak token kovasındandır (INV-RATE-1); PRE_CONVERGE dışında
> vadesi gelmemiş kopya kaydı DATA örneğini app-limited dışı saydırmaz.
> **Teyit çıktısı (NORMATİF):** motor, birimin teyit kümesine İLK kez girdiği olayı tek çıktıyla bildirir: son
> bildirimden bu yana teyit edilen birim kimlikleri, teyit sırasıyla, her birim oturumda en çok bir kez. Kaynaklar YALNIZ:
> canlı kayıtla eşleşen ACK (uçuştaki kayıt, tombstone, kayıp ilan edilmiş ama yeniden planı bekleyen kaydın un-loss
> eşleşmesi — tanınan geç ACK dahil) ve FEC RECOVERED. Teyit ÜRETMEYENLER: FEC onarım sembolünün ACK'i; canlı kaydı
> olmayan iletimin ACK'i (tombstone TTL'i dolmuş, tombstone tavanından taşmış, birim başına kayıt sınırıyla budanmış (K4G-05c) ya
> da tombstone bırakmayan kopya kökenli kayıp kaydı) — Expired/Unknown, birim sahipli kalır ve yeniden gönderilir;
> kayıt yokken dedektör defterindeki sınıflama; teyitli birimin sonraki ACK'leri; zamanlayıcı ve süre dolumu. Çıktı her
> olaydan sonra boşaltılır; birikimi sınırlıdır. Runtime payload'u ve uygulamaya teslim teyidini YALNIZ bu çıktıya
> bağlar; iletim→birim eşlemesi silme kararı vermez (runtime tarafı K4G-05b; birim başına O(1) kayıt K4G-05c).
> **Uygunluk sınırı (BEYANLI; uygunluk = vadesi gelmiş kaydı olan izleyen yol; emisyon token'a bağlıdır, ayrı ölçülür,
> gecikme sözü VERİLMEZ):** kuyruk-probu zamanlayıcısı t_arm = max(son ilerleme, birimin uçuşta görüldüğü ilk
> zamanlayıcı tiki) anında kurulur; t_arm'dan sonra ilerleme olmaz ve prob emisyonları token beklemezse son prob ve devir
> ≤ t_arm + Σ_{i=0..2} min(taban-PTO·2^i, RTO_MAX) ≤ t_arm + 10 + 20 + 30 s = t_arm + 60 s (taban-PTO ≤ `max_pto_ns`
> 10 s; `max_tail_probes` 3) — prob kapsamındaki her birim için; vadesi gelen ama token bulamayan prob hakkı harcamaz,
> sonraki PTO gönderimden sayılır — bu bekleme toplama eklenir; ilerleme zamanlayıcıyı yeniden kurar. Prob kapsamı
> dışındaki birimde ilk uygunluk ≤ son ilerleme ya da iletim + H + bir planlama tiki. Sonraki uygunluklar ≤ RTO_MAX
> arayla.
> **RTO_MAX = 30 s (NORMATİF sabit — koddan belgeye, yeni sayı DEĞİL; CEVAP PAKETİ 5, 2026-10-16, madde 2) [CAL — DR-ARQ-PRODUCT-TAIL-1, tuning pending]:**
> kaynak `TailRecoveryConfig::validated_v1().max_backoff_pto_ns` (PRODUCT-PROFILE "backoff tavanı 30 s", SHIP; pin
> `crates/client-core/tests/transport_v1_freeze.rs`). Üst sınırı olduğu aralıklar: (i) kuyruk-probu PTO geri
> çekilmesi (v1'de de); (ii) PRE_CONVERGE dışında kopya kuyruğunun geometrik aralığı ve tabanı (v1'de tavansızdı;
> PRE_CONVERGE çıkışında bekleyen kopyalar RTO_MAX'a kırpılır). Parite kaydı süre dolumunun (K_MAP × RTO_MAX) ve RTT tazelik
> ölçütünün tabanıdır (engine §6). Taban srtt-türevidir; taban-PTO tavanı `max_pto_ns` = 10 s ayrıdır; H kanal-türevidir
> ve RTO_MAX'la sınırlanmaz. ZM-2 4-decade doğrulaması BEKLİYOR (beyanlı: v1.1 T-hücreleri D = 20 s ile bu tavana
> ulaşmaz; uzun pencereli hücreler DEMO-01 öncesi ön-kayıtlı gece kartında — CEVAP PAKETİ 7, 2026-10-18, DUR-3 (b)).
> Değeri değiştirmek ayrı sahip kararıdır.
> **Sıfırlama (NORMATİF):** (i) kuyruk-probu: uçuştaki kaydı azaltan ilk ACK probe_index'i sıfırlar (v1 kodu, sender
> `on_ack_received`; DEĞİŞMEDİ); (ii) kopya kuyruğu: ilerlemesiz süre H'yi aştıktan sonra en az bir kaydı ilk kez
> onaylayan İLK ACK bütün geometrik kopya aralıklarını tabana döndürür (sonraki kopya en geç son kopya + taban); bunun
> doğurduğu emisyon ortak pace'e ve BURST_CAP'e tabidir (INV-RATE-1), sıra engine §13 kopya kuyruğu içi sırasıdır. Bayat
> ACK sıfırlamaz.
> **Beyanlı istisna (CEVAP PAKETİ 7-EK, 2026-10-18, B1):** PRE_CONVERGE'de BURST_CAP tabanı v1'dir; ilk-uçuş bütçesi
> kaydın boyutundan küçükken BURST_CAP'i aşan kayıt izlenir, ama token'a yetişmeyebilir (engine §13 v1.1 kaydı).
> **Kayıt sınırı ve geç-ACK tanıma (NORMATİF, CEVAP PAKETİ 9, 2026-10-24, DUR-20; CEVAP PAKETİ 11, 2026-10-24, DUR-28
> ve DUR-30; CEVAP PAKETİ 11-EK, 2026-10-24, §3–§4):** K_MAP = 8, birim başına iletim eşlemesi ve zamanlama kaydı
> (sender uçuş/tombstone tablosu ve kayıp dedektörü haritası) için bir ADET sınırıdır; süre garantisi DEĞİLDİR.
> ~~Budama — hangi kaydın seçileceği ve uçuş muhasebesi dahil — K4G-05c'de tanımlanır ve runtime'ın birim başına
> O(1) kaydıyla aynı değişiklikte açılır; K4G-05b'de `K_MAP` ve teyit çıktısı tanımlıdır,
> budama KAPALIDIR.~~ **[2026-10-25 — K4G-05c, CEVAP PAKETİ 14, 2026-10-25, DUR-05c-14 (A)] Budama AÇIKTIR**
> (runtime'ın birim başına O(1) kaydıyla aynı değişiklikte): birimin uçuş ve tombstone kayıtlarının TOPLAMI
> ≤ K_MAP, orijinal dahil; aşılırken birimin ORİJİNAL Data kaydı DIŞINDAKİ en eski kaydı ilan edilmeden budanır
> (uçuş muhasebesinden ve kayıp dedektörü haritasından düşer; ledger ve kayıp sayaçları değişmez). Orijinal Data
> kaydı YALNIZ K_MAP budamasının hedefi olmaz; tombstone TTL'i ve tombstone tavanı tahliyesi ona da uygulanır —
> daha geniş bir saklama garantisi yoktur. ~~Geç ACK, seq'i birimin son K_MAP iletimi arasındaysa tanınır — o kayıt
> TTL/tahliyeyle daha önce silinmemişse; tanıma garantisi bütün silme yollarıyla birlikte tanımlıdır (kayıp ilan
> edilmiş kayıt için tombstone TTL'i (K_TOMB × T_loss) ve tombstone tavanı v1 aynen; K_MAP budaması K4G-05c).~~
> **[2026-10-25 — K4G-05c]** Geç ACK, motorun o iletim için hâlâ tuttuğu kayıtla eşleşirse birimi teyit eder:
> seq birimin orijinal Data kaydıysa ya da orijinal dışındaki en yeni kayıtları arasındaysa (toplam ≤ K_MAP) ve o
> kayıt tombstone TTL'iyle (K_TOMB × T_loss) ya da tombstone tavanı tahliyesiyle daha önce silinmemişse (v1 aynen,
> orijinal dahil); tanıma garantisi bütün silme yollarıyla birlikte tanımlıdır. Unutulmuş (budanmış, süresi dolmuş
> ya da tahliye edilmiş) kaydın geç ACK'i birimi teyit etmez. Tanınmayan geç ACK veri kaybı
> değildir; ek yeniden gönderimlere yol açabilir; payload sahipliği korunur: Expired/Unknown, birimi TEYİT ETMEZ,
> birim sahipli kalır ve yeniden gönderilir, RTT örneği VERMEZ. Gerçek gönderim dizilerinde tutulan kayıt yaşı
> ölçülür ve INT-02 raporunda yazılır. Diğer sınırlar: bekleyen kopya işi birim başına ≤ D_max = 3; payload sayısı
> ≤ birim deposu; alıcı tamponu ve runtime eşleme tabloları bu kaydın kapsamı dışındadır (K4G-05c).
> **Beyanlı sınırlar (CEVAP PAKETİ 7-EK, 2026-10-18, A1; CEVAP PAKETİ 9, 2026-10-24):** v1'de aralık tavansız olduğu için
> kayıt büyümesi logaritmikti; K4G-05b'de (budama kapalı) birim başına kayıt zincir emisyonuyla büyür (aşağıdaki
> biçim; tombstone kaydı TTL ve tavanla sınırlı), K4G-05c'de K_MAP budamasıyla ≤ K_MAP. Emisyon sessizlikte sürer — canlılık
> için bilinçli takas (INV-NO-IDLE-TEARDOWN): bir kopya zinciri Δ sürede birim başına ≤ m + ⌊Δ/RTO_MAX⌋ iletim üretir
> (rampadan sonra birim başına 30 s'de ≤ 1); m = ⌈log2(RTO_MAX/b)⌉, b srtt-türevi kopya tabanı; ilk m iletimin rampası
> v1 ile aynıdır; zincir dışı iletimler (orijinal, RECOVER_FWD ek kopyaları, ARQ ve kuyruk-probu yeniden gönderimleri)
> Δ ile büyümez. [AMENDMENT-9] K = 1024 hop-ACK'siz ardışık PN tavanıdır, bellek ya da tarihçe sınırı DEĞİLDİR
> (tavanda hub fallback ister; bugün SPEC-ONLY); uygulandığında bu yolların emisyonları ona TABİDİR (izleme sürer).
> CEVAP PAKETİ 7-EK (2026-10-18) A1'in "AMENDMENT-9 uygulama kartı DEMO-01 ön koşulu" kararı CEVAP PAKETİ 9
> (2026-10-24) ile geri çekildi. Bayatlayan atıflar ve EVOLUTION §22 altı beyanı: K4G-05b PR açıklaması; yetki dışı
> belgelerde tek satırlık AMENDMENT-12 işaretçileri (CEVAP PAKETİ 7, 2026-10-18, DUR-4 (b)).

> **[AMENDMENT-11 — 2026-10-08, WS-D (BL D1/D2/D3) UYGULANDI · TEL ANLAMI DEĞİŞTİ · MOTOR DEĞİŞMEDİ]**
>
> **D1 — kimlik (D1a: runtime çeviri katmanı).** Gönderen her motor kararını (DATA · retransmit · duplication · FEC
> repair) ayrı bir `seal()` ile mühürler → her (yeniden) gönderim yeni PN alır (KM-SEAM-TC-001 K2/K4). Çeviri tablosu
> PN → motor seq → mantıksal birim (`crates/client-core/src/runtime.rs`); PN ufku `PN_SEQ_WINDOW = 2^16`, yani bir u16
> sarması: ACK_V2 `largest_seq` u16'dır ve daha gerideki bir PN telden doğru geri kurulamaz. Alıcı
> `RxDatagram.seq = Opened.meta.full_wire_pn` besler → ACK_V2 aralıkları PN uzayındadır. ACK_FRAME_V2 formatı
> DEĞİŞMEDİ; `ack_epoch` alanına dokunulmadı (2026-10-05 notu). `adaptive-transport` motoru DEĞİŞMEDİ: motor kendi seq
> uzayında kalır, ACK motor seq'ine çevrilerek beslenir.
> Motor kararı taşımayan çerçeveler (PARAM, CS-probe) PN tüketir; alıcıya beslenmez ve çeviri tablosuna girmez →
> alıcının SACK'inde boşluk olarak görünürler. Motora giden teyitli motor-seq kümesi aynı kalır; tek fark,
> `MAX_ACK_RANGES` kırpmasının bu boşlukları da sayması (FEC açıkken PARAM, bootstrap açıkken CS-probe; akış
> başında). Bu farkın etkisi ÖLÇÜLMEDİ.
> Ara çerçevedeki u16 `seq` alanı YERİNDE: S3 kararı (2026-10-08) gereği "interim çerçeve kodda yok" bu sprint
> beyanlı KARŞILANMAZ (`BatchPayload` codec'i var, `crates/client-core/src/batch.rs`; veri yolu geçişi sonraki
> sprint). Alan motor seq'inin alt 16 bitini taşımaya devam eder ama alıcı OKUMAZ — kimlik değildir.
> PoC `IdentitySeam` (S4): Session sahte etiketinin ilk 8 baytı PN'dir (BE), HubChannel etiketi sıfırdır; `open` PN'i
> oradan okur ve her datagramı tek örtük oturuma atfeder.
> KM-SEAM-TC-001 K2'deki "ACK'te gelen full-wire-PN aralığı (noise-wire §2.57)" ifadesi K3 (2026-10-04) ile aşılmıştır:
> PN aralığını taşıyan çerçeve engine ACK_V2'dir.
> Kanıt: `client-core/tests/runtime_e2e.rs::arq_identity_is_the_pn_not_the_interim_seq_field` (alıcı seam'i u16 alanını
> sıfırlar, gönderen seam'i ilk mühürden önce 5 PN yakar, bir birim kalıcı silinir). İki hedefli mutasyon — gönderen
> PN'i motor seq'i sayar / alıcı u16 alanını kimlik sayar — ikisi de 3/3 koşuda kırmızı; mutasyonsuz 5/5 yeşil.
>
> **D2 — ikinci ACK ailesi söküldü.** noise-wire `Control::Ack/Sack` (0x10/0x11) kodda YOK: `kama-transport-schema`
> (`Control` varyantları, `inner_kind`), `kama-noise-wire` (kontrol codec'i, `ACK_MAX_RANGES`/`SACK_MAX_GAP_BLOCKS`,
> retry sınıfı), `kama-engine` (`recovered_map` testi). Codepoint'ler RESERVED → reject: şema `classify` →
> `UnknownInnerKind`; noise-wire kontrol çözücüsü → `ControlDecode`. Ürün ACK'i = engine ACK_V2 (K3, 2026-10-04).
>
> **D3 — golden etkisi.** `adaptive-transport/tests/tv_*.rs`: DEĞİŞMEDİ (ACK_V2 ve ara çerçeve baytları aynı).
> `kama-transport-schema/tests/golden_v0.rs::inner_kind_registry`: DEĞİŞTİ — ~~`classify(0x10) = Control(0x10)`~~ →
> 0x10/0x11 fail-closed RESERVED listesinde, kontrol aralığının alt ucu `classify(0x12) = Control(0x12)`; prefix,
> PARAM-canonical ve overhead golden'ları aynı. noise-wire: `sprint2_control.rs::control_body_golden` içindeki Ack gövde golden'ı
> KALDIRILDI, yerine RECOVERED golden'ı geldi — codec yerleşiminden elle türetildi (`block_id u32 · count u16 ·
> (esi_start u16, esi_end u16)×count` → `0A0B0C0D 0001 000A 0014`); `m3_sealed_bytes.rs`'te "Control-Ack-4range"
> vakası "Control-Recovered-16range" oldu (DR-1 tavanındaki en büyük meşru kontrol gövdesi; dosya bayt golden'ı
> değil ≤1200 B PMTU sınırını sınar); `control_split_and_state_mutants.rs`'te bölme/tavan mekanizması yalnız
> RECOVERED üzerinde pinli. Kayıt dışı kalan tarihî rapor satırı (`reports/d1-exit/M3-M4-M5.md`, "Control-Ack")
> değiştirilmedi.
> `IdentitySeam` mühürlü baytları PN > 0'da etiketin ilk 8 baytında değişti (PoC stub; golden değil, PN 0'da v1 ile
> özdeş).
>
> **EVOLUTION §22 — ALTI BEYAN:**
> 1. Donmuş invaryant: INV-RATE-1, INV-FEC-1, INV-ENGINE-SOC DEĞİŞMEZ (kimlik Crypto'nun `meta`'sından okunur;
>    transport anahtar tutmaz, PN üretmez).
> 2. `KamaV1Profile`: alan eklenmedi, değişmedi.
> 3. Profil kimliği: tel anlamı değişti (ACK aralıkları PN uzayında; alıcı u16 alanını okumaz) → AMENDMENT-11 öncesi
>    bir uçla birlikte ÇALIŞMAZ → `transport-v1.x`.
> 4. Geçersiz olan T-hücreleri: karar yolu aynı girdiyi alır; FEC ya da bootstrap AÇIK hücrelerde ACK range kırpması
>    farklılaşabilir (ölçülmedi) — bu hücrelerin bankadaki sonuçları AMENDMENT-11 öncesi koda aittir.
> 5. Yeniden doğrulama: engine + integration testleri yeşil (iş emri §5 kabul ölçütü). KVT tek netem profili
>    NON-GATING — KOŞULMADI (`sch_netem` yüklü değil; T5 sahibin).
> 6. Sürüm: `transport-v1.x` (v2 DEĞİL — mimari değişmedi, kimlik kaynağı değişti).

> **[AMENDMENT-8 — 2026-09-23, N-PENCERESİ TEMİZLİĞİ · DAVRANIŞ DEĞİŞİKLİĞİ YOK]** Bu belgedeki "N-mesaj ACK penceresi"
> ifadeleri (§7 Pipeline A, §12) üstü çizildi. Gerekçe: 2026-09-22 sahip kararıyla N-mesaj ACK penceresi ÜÇE AYRILDI —
> hub tarafı sınır **mailbox kotası** olarak app-rpc §8'de KALIR (`0x0505 MAILBOX_QUOTA_EXCEEDED`), **istemci/gönderen
> penceresi KALKTI** (Application ACK beklemez; identity E-9, CAD-45), teslim bilgisi AMP `RECEIPT(delivered)`'a geçti.
> Transport'un ACK stratejisi (range/SACK, piggyback, decimation, per-profil bütçe) DEĞİŞMEDİ; burada yalnız
> transport'a ait OLMAYAN bir uygulama-katmanı mekanizmasının adı temizlendi. **Runtime kararı değişmez** → profil
> kimliği `kama-v1-full` KORUNUR, T0–T7 hücreleri geçerli kalır, yeniden doğrulama GEREKMEZ.

> **[AMENDMENT-9 — 2026-09-23, D-GAP-2: HOP-ACK'SİZ ARDIŞIK PN TAVANI · NORMATİF · UYGULANDIĞINDA DAVRANIŞ DEĞİŞTİRİR]**
>
> **Kural (NORMATİF):** Gönderen, peer'dan hop-ACK almadan ARDIŞIK olarak en fazla **K** paket-numarası (PN) tüketebilir.
> Tavana ulaşıldığında yeni datagram üretilmez; tüketim ancak ilerleyen bir hop-ACK geldikten sonra devam eder.
> Sayım ~~(session, direction, dh_epoch)~~ **(session, direction) [2026-10-06 — PN SÜREKLİ: sayaç epoch'ta sıfırlanmaz, noise-wire §2.5]** başınadır ve **mühürlenen her sınıfı** kapsar (DATA · retransmit · tail probe ·
> duplication · FEC repair · E2E kontrol) — çünkü noise-wire §2.5 hepsini TEK PN uzayına koyar.
>
> **Gerekçe — gap değil İSRAF:** boşluğa giden her datagram bant ve `SEND_PN_CAP` yakar. (Far-future reddi D-GAP-1 ile
> kaldırıldı; bu tavanın amacı replay kapısı DEĞİLDİR.) İkincil olarak tavan, ccr §35.8'in yapısal sınırını
> (|full_counter − expected| < 2^15) meşru trafikte korur → **K ≪ 2^15 ZORUNLUDUR**.
>
> **Bağlanan üç yol (bugün sayı-tavanı YOK — ölçülü, identity §28.1-A):** engine-spec §3.3-B PreConverge geometrik
> kopya · §11.1 cold-start probe/echo (establish probe) · §11 emergency forward redundancy (RecoverFwd sessizlik
> duplikasyonu). Üçü de bu tavana TABİDİR.
>
> ~~**[KARAR-SLOTU KS-PN-1 — 2026-09-23] K'nin DEĞERİ [KALİBRASYON — Abkarada].** Sayı bu belgede UYDURULMAZ.~~ *[2026-10-03: aşağıdaki KAPANIŞ ile sayıya dönüştü]*
> Kısıt: `K ≪ 2^15`. Kalibrasyon girdileri: hedef hostile zarf (T1b sıfır-ACK hücresi), `SEND_PN_CAP` (noise-wire §2.5)
> ve üç yolun ölçülen emisyon hızı.
> **[2026-09-23 — ikincil etki, sahip notu]** `K ≤ 8192 = 2^13` seçilirse gönderen, noise-wire extended-pn32 yolundaki
> `FUTURE_GAP_MAX = 2^13` kapısına da hiç ulaşamaz → o kapı konusuz kalır (identity OQ-16 PARK). `K > 8192` seçilirse
> OQ-16 yeniden açılır ve pn32 kapısı için ayrı karar gerekir.
>
> **[KS-PN-1 KAPANDI — 2026-10-03, DR-CONNECT-1 H5 · sahip kararı] `K = 1024`.**
> Session ctx'te hop-ACK almadan 1024 PN tüketilince yeni Session-ctx emisyonu DURUR; sonra sırayla:
> (1) `SESSION_REBIND` denenir (hub PeerSlot'u emekliye ayrılmış olabilir) → (2) floor kadansta hub-kanalı prob'una
> düşülür (§10 [AMENDMENT-10]) → (3) hop-ACK gelince ARQ kaldığı yerden devam eder. Bu bir vazgeçme değil, yakmayı
> kesmedir (INV-NO-IDLE-TEARDOWN, §0: oturum kapanmaz).
> Gerekçe: `cwnd`'nin kod-türevi tavanı yoktur; zarfın üst köşesinde ≈494 datagram (kanıt:
> `docs/sprint_plan/A1-KALIBRASYON-KARAR-FOYU.md` KANIT PAKETİ K4). K bunun altında olsa meşru yüksek-BDP akışı
> boğardı; 1024 = iki kat pay. `K = 1024 ≤ 8192` olduğundan gönderen pn32 `FUTURE_GAP_MAX = 2^13` kapısına ulaşamaz →
> identity OQ-16 kalibrasyonla KAPANIR. Aşağıdaki altı beyan ve SPEC-ONLY durumu AYNEN geçerlidir (kod bugün sınırsızdır).
>
> **EVOLUTION §22 — ALTI BEYAN (bu amendment UYGULANDIĞINDA):**
> 1. Donmuş invaryant: INV-RATE-1 DEĞİŞMEZ (tavan kayıp sinyaliyle değil, geri-bildirim yokluğuyla tetiklenir); yeni bir
>    emisyon-tavanı invaryantı EKLENİR.
> 2. `KamaV1Profile`: yeni alan (ör. `max_unacked_pn`) EKLENİR — bugünkü profilde YOKTUR.
> 3. Profil kimliği: `kama-v1-full` KORUNAMAZ → `transport-v1.x`.
> 4. Geçersiz olan T-hücreleri: sıfır/az-ACK hücreleri — T1b (18/30 sıfır-ACK), T1a, T6; ayrıca PreConverge'in uzun
>    kaldığı her hücre.
> 5. Yeniden koşulacak doğrulama: ADR-hostile-validation-contract ile T0 (regresyon) + T1a/T1b/T6, N ≥ 30, ön-kayıtlı.
> 6. Sürüm: `transport-v1.x` (v2 DEĞİL — mimari değişmiyor, sınır ekleniyor).
>
> **DURUM: SPEC-ONLY.** Bu turda Rust'a DOKUNULMADI; kod bugün sınırsızdır. Uygulama, yukarıdaki altı beyanla ve
> KS-PN-1 değeriyle birlikte AYRI bir iş kalemidir.

```
~~N-mesaj ACK penceresi;~~ ACK range/SACK compression; piggyback; ACK decimation; per-profil ACK budget.   [AMENDMENT-8]
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
Kama SESSION_DATA/UDP/IPv4 = 50B + payload → per-mesaj ~12B avantaj.
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
| INV-FUZZY-1 | FEC/ARQ/rate kararı (compress: AMENDMENT-4 ile transport'tan ÇIKARILDI) fuzzy controller (loss/jitter/bw/queue → defuzzified); parity budget-clamp'li. |
| INV-CLASS-2 | Loss rate MUST NOT be a direct bandwidth estimator (delivered-bytes/time). |
| INV-FEC-1 | FEC plaintext shard'lar üzerinde çalışır AMA her alınan shard FEC decoder'a girmeden ÖNCE AEAD-authenticate edilir; authenticated olmayan byte decoder'a GİRMEZ (SR-AUDIT-001). FEC yalnızca block/shard framing MEVCUTSA uygulanır — framing-driven, application-role-driven DEĞİL; transport baytların chat/file/control olduğunu bilmez. |
| INV-FEC-2 | FEC erasure coding (Reed-Solomon), XOR değil; on-demand (loss eşiği + ARQ kurtaramıyor/yavaş). |
| INV-ARQ-1 | TEK custom adaptif transport motoru (Rust-native, C++/FFI yok); ARQ + FEC mekanizmalarını İÇERİR; fuzzy controller ölçülen bw/jitter/loss'a göre seçer (ne ARQ ne FEC default). Transport uygulama rolünü (hot-path/bulk) BİLMEZ (SoC). KCP çıkarıldı. |
| INV-QUIC-1 | Her transport testi QUIC ile kıyaslanır; bir rejimde QUIC iyiyse QUIC seçilir; custom motor yalnız QUIC'in çöktüğü lowband/high-loss rejiminde (QUIC'ten düşük banta iner) gerekçelenir. QUIC fork NO-GO. |
| INV-QBENCH-1 | QUIC kıyası ÜÇ-TIER kazanma modeli (Tier-1 categorical / Tier-2 operational-deadline / Tier-3 efficiency) ile karara bağlanır — GO yalnız Tier-1'e DEĞİL; salt marjinal verim (Tier-3-alone, operasyonel-belirleyici değilse) custom motoru haklı çıkarmaz; karşı taraf best-available QUIC (en güçlü mevcut CC) adil kurulur; kaybedilen bölge önceden ilan + ADR-14 fallback (QUIC selected'ta Session V3 semantics korunur). |
| INV-QBENCH-2 | Üstünlük iddiası estimator-accuracy + loss/congestion confusion matrix ile mekanizma-kanıtlı olmadan yazılmaz (ölçüm > varsayım; magic-number placeholder YASAK; barlar baseline'dan kalibre). |
| INV-CRYPTO-1 | Client↔Hub Noise IK/KK (setup: dest_hash ClientHint'te gizli + hub_handle atama); single-datagram outer Noise YOK (thin); outer AEAD yalnız block/shard framing + control; hub-mesh Noise KK. |
| INV-NONCE-1 | Transport counter monoton; 2B seq wire'da; receiver reconstruct; wrap→rekey; reuse imkansız. |
| INV-REPLAY-1 | Replay window AEAD başarısından ÖNCE güncellenmez. |
| INV-COMPRESS-1 | **TAŞINDI → app-rpc §3 (AMENDMENT-4).** Transport sıkıştırmaz; CRIME/çift-sıkıştırma/uzunluk-leak hükümleri mesaj-hazırlama alt-katmanında. |
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
| AC-5 | Fuzzy controller: loss/jitter/bw/queue → fec/arq/rate (compress transport dışı, AMENDMENT-4); rate loss'a bağlı DEĞİL. |
| AC-6 | bandwidth obsesif sampling (validity-gated); ACK-compression/app-limited bw_est'i şişirmez. |
| AC-7 | FEC on-demand Reed-Solomon erasure (XOR değil); yalnızca block/shard framing MEVCUTSA (framing-driven, app-role değil); AEAD-verify önce. |
| AC-8 | En kötü senaryo (loss high+bw low+ack_confidence low): mode=EMERGENCY_FORWARD_REDUNDANCY (§5.6) — tiny→2×/3× duplication, block→minimal parity, sparse cumulative ACK; pure-ARQ DEĞİL; path düzelince fuzzy redundancy'yi ASİMETRİK HYSTERESIS ile geri çeker (Schmitt bant + de-escalate dwell; escalate anında; tek-eşik salınımı YASAK — §5.6, engine §11). rate=minimal-reliable (compress: transport dışı, AMENDMENT-4). |
| AC-9 | KCP tamamen ÇIKARILDI; tek custom adaptif transport motoru (Rust-native, ARQ+FEC, fuzzy-seçili; uygulama rolüne kör); per-paket ACK yok (storm engeli). |
| AC-QUIC | Her test QUIC ile yan yana ölçülür; QUIC daha iyiyse QUIC seçilir; custom transport QUIC'in çöktüğü lowband/high-loss'ta QUIC'ten düşük banta iner (kanıt = Side-by-Side). QUIC fork NO-GO. |
| AC-QBENCH-1 | QUIC baseline best-available production-grade QUIC + en güçlü mevcut CC (destekliyorsa BBR/BBRv2; Cubic/Reno anlamsız), versiyon-pinli, kullanılan profil raporda; aynı netem/MTU/payload/donanım; tek değişken = transport (§4.4.1) |
| AC-QBENCH-2 | Ek baselineler (TCP+TLS, KCP, RNS, Signal/WhatsApp) SECONDARY/appendix — GO kapısını bloke ETMEZ; primary gate = vs best-available QUIC; layer-matched, "yendik" yalnız ölçülmüş delta ile (§4.4.1, §4.3) |
| AC-QBENCH-3 | Kazanma ÜÇ-TIER (Tier-1 categorical [X,Y] / Tier-2 operational-deadline+survival+offline / Tier-3 efficiency); GO yalnız Tier-1'e bağlı DEĞİL; birincil metrik deadline-bounded completion/floor/survival/first-delivery + secondary-but-required latency/bytes/ACK/energy (§4.4.3, §4.4.10) |
| AC-QBENCH-4 | Estimator accuracy + loss/congestion confusion matrix netem ground-truth'a karşı ölçülür; üstünlük mekanizma-kanıtlı (§4.4.4) |
| AC-QBENCH-5 | Kaynak maliyeti (CPU/byte, bellek, enerji, ARM) quinn ekseninde ölçülür (§4.4.6) |
| AC-QBENCH-6 | Transport §0.1 kripto invariant'larını bozmaz: CODE-THEN-ENCRYPT-AUTHENTICATE-SHARDS (FEC plaintext-domain + per-shard AEAD + authenticate-before-decode, §7/INV-FEC-1); parity plaintext sızdırmaz; replay/injection dirençli (§4.4.7) |
| AC-QBENCH-7 | Adaptif davranış yeni traffic-analysis yan kanalı yaratmaz; FEC-on zorlamalı DoS budget/emergency clamp ile sınırlı (§4.4.7, §5.5/5.6) |
| AC-QBENCH-8 | N≥30 run + p95/p99 + CI + versioned harness + yayımlanan ham veri → bağımsız reproduce (§4.4.9) |
| AC-QBENCH-9 | GO = (A) en az bir kazanma tier'ı [Tier-1/2/3] + (B) guardrail'ler [mekanizma+güvenlik+iyi-ağ]; Tier-3-alone yalnız operasyonel-belirleyici verimle; global NO-GO → quinn devri kabul; QUIC selected'ta Session V3 semantics değişmez (§4.4.5, §4.4.10) |
| AC-CRYPTO | Kripto katmanlama §0.1: Noise IK/KK setup (dest_hash ClientHint gizli) + thin single-datagram (outer Noise yok) + outer AEAD yalnız block/shard framing + control + hub-mesh Noise KK. |
| AC-10 | TAŞINDI → app-rpc §3 (AMENDMENT-4): sıkıştırma transport kabul ölçütü DEĞİLDİR. |
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
| QUIC full-scale eval: üç-tier kazanma (categorical/operational/efficiency); adil best-available QUIC; deadline-bounded completion; mekanizma-kanıtı (estimator/confusion); güvenlik-pariteliği (code-then-encrypt-auth-shards); reproduce | ✅ | 4.4, AC-QBENCH-*, INV-QBENCH-* |
| Kaybedilen bölge (P_bbr) önceden ilan; global NO-GO ([X,Y] boş → quinn) kabul (doktrin dürüstlüğü) | ✅ | 4.4.5, 4.4.10 |
| Kripto katmanlama kilitli (Noise IK/KK setup + thin single-datagram + outer-AEAD block/shard-only) | ✅ | 0.1, INV-CRYPTO-1 |
| Paket kaybı → fuzzy-seçili recovery (ARQ/FEC/karma, koşula göre; ne ARQ ne FEC default) | ✅ | 5.3, 12 |
| Sıkıştırma — transport dışı; app-rpc §3'e taşındı (AMENDMENT-4) | ➡ | 8 |
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
  Bu belge = SRC/pacing + fuzzy controller + FEC/ARQ adaptif seçim (tek adaptif transport motoru; ne ARQ ne FEC default) + QUIC benchmark + MTU/ACK (compression: app-rpc §3, AMENDMENT-4) +
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
  - QUIC FULL-SCALE eval methodology (§4.4: kategorik yetenek eşiği [X,Y]; adil quinn+BBRv2 + TCP/KCP/RNS baselines; completion/blackout-survival/floor birincil; estimator-accuracy + loss/congestion confusion mekanizma-kanıtı; kaynak/güvenlik-pariteliği/reproduce; GO-NO/GO launch criteria; DCP/ASELSAN/TÜBİTAK killer-chart)
  - compression policy   [AMENDMENT-4: transport dışı → app-rpc §3]
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

> **Sonuç (NORMATİF):** Kama transport "KCP over UDP" / "QUIC clone" değildir; KCP ÇIKARILDI (tek adaptif transport motoru, Rust-native, ARQ+FEC içerir). Async hot-path = UDP üzerinde E2E-authenticated message envelope + kısa hub handle; bağ TLS gibi anahtarda; kripto katmanlama §0.1'de kilitli (Noise IK/KK setup + thin hot-path). FEC/ARQ/SRC seçili modların araçları; FEC/ARQ/CC kararı fuzzy (loss/jitter/bw). Paket kaybı fuzzy controller'ın ölçülen koşula göre SEÇTİĞİ recovery ile (ARQ / FEC / karma) kurtarılır — ne ARQ ne FEC sabittir; transport uygulama rolünü bilmez (SoC). QUIC = zorunlu benchmark referansı (iyiyse seçilir; custom motor yalnız QUIC'in çöktüğü lowband/high-loss rejiminde, QUIC'ten düşük banta inerek gerekçelenir; fork NO-GO). Tek savunulabilir lowband mimarisi: thin layer-collapsed envelope + tek adaptif ARQ/FEC motoru (koşula göre seçen) + fuzzy karar + QUIC-kıyaslı dürüstlük.
