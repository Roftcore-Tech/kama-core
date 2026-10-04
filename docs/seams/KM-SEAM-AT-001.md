# KM-SEAM-AT-001 — Application ↔ Transport Seam Sözleşmesi (S1)

**Belge:** `docs/seams/KM-SEAM-AT-001.md` v0.1 · **Tarih:** 2026-09-19
**İmzacılar:** Hayri (Application) · Abkarada (Transport / kama-core) — ikisi imzalamadan sözleşme değildir.
**Statü: TASARIM GO ✅ (K1–K9 + KARAR-1/2 kapalı, 2026-09-19) · SEAM-FROZEN HOLD 🟡** — FROZEN için: types compile + AMP codec conformance vektörleri + §6 harness + §7 stub'lar. **Kripto golden vektörü bu seam'in kapısı DEĞİLDİR** (sınırdan kripto byte'ı geçmez).
**Türetildiği kaynaklar (icat yok):** transport-tdd §0 INV-NEIGHBOR-1 / INV-PLAINTEXT-NOT-SEMANTICS · KM-SEAM-TC-001 (S3) §1.2 SessionControl/SessionEvent, K7, K8, K9 · app-rpc §2 (üç düzlem), AMENDMENT-7 (envelope_type) · application-message-protocol.md §1–§2, §5 · engine-spec §0.2 · ccr §35.6 BatchPayload (KARAR-1) · identity (multi-device limit=1)
**Yazım:** her karar **K** · **G** (kaynak atfıyla) · **R** (reddedilen alternatif) · **S** (sınır).

> **YAMA KAYDI — 2026-10-05 [B3 — Abkarada kararı]:** §3'e `DeadlineExpired` açıklaması eklendi: varyant KALIR; birim deadline'ıdır (`DeliveryPolicy::DeadlineBounded`), bağlantı deadline'ı değil. DR-CONNECT-1 yalnız bağlantı deadline'ını kaldırdı. Trait bloğu ve hata listesi DEĞİŞMEDİ (yalnız ekleme).

> **YAMA KAYDI — 2026-10-04b [K2 — Abkarada kararı]:** K6'ya glare kaybedeninin `early_unit` kuralı: Transport `early_unit`'in kopyasını oturum `Ready` ya da `ConnectAbandoned` olana kadar saklar; glare kaybında kazanan oturum `Ready` olunca ilk normal `send()` olarak yollar, Application'a olay çıkmaz. §8'deki açık kalem kapandı. API değişmedi.

> **YAMA KAYDI — 2026-10-06 [C1 test 4 — Abkarada kararı]:** §3'e pending tavanı cümlesi eklendi: istemci pending INIT tavanı (ccr §35.15 KS-DOS-1, `PENDING_RESPONSE_TRIAL_CAP` ile eşit) doluyken yeni `connect()` → `TransportError::Backpressure`. Trait bloğu ve hata listesi DEĞİŞMEDİ (yalnız ekleme).

> **YAMA KAYDI — 2026-10-04 [DR-CONNECT-1 tamamlama — Abkarada kararı]:** K7 supersession **PROVISIONAL** olarak yeniden yazıldı (olay sırası `Ready(h')` ONAYDA → yalnız-alım drain → `Closed(h, Superseded)`; Renew ile peer-restart aynı yol; glare'de tek `Ready`) · §4 `LongOutage` = heartbeat yanıtı 60 s'dir yok (KS-UI-1; yalnız UI) · `SESSION_MAX_AGE = 7 gün` · §6'ya **H-SILENT-REJECT** (H-OPAQUE-FAIL'in yerine) · §8 açık kalemler güncellendi. **İmza:** Hayri imzası S1 freeze'inin (G-S1) ön koşuludur, merge'in değil. Üstü çizili metin silinmedi.

> **YAMA KAYDI — 2026-10-03 [DR-CONNECT-1 — Abkarada kararı; indeks `docs/decisions/DR-CONNECT-1.md`]:** `connect()` semantiği "kalıcı niyet, sınırlı deneme"ye çevrildi. §1: `ConnectFailed` KALDIRILDI; `cancel(ticket)`, `ConnectAbandoned(ticket, reason)`, `ConnectProgress(ticket, phase)` eklendi; `CloseReason`'a `Superseded` (H9, H6). §3: `ConnectAbandoned.reason` opak DEĞİL (yerel yaşam-döngüsü durumu; opaklık kripto hatalarına aittir). §4: **KS-8 KAPANDI — deadline yoktur** (H10); "Plane 1 `Reliable` deadline'ı" cümlesi üstü çizildi (yapısal olarak yoktu); sonlandırma nedenleri kapalı liste (H2); T-18c ve SESSION_MAX_AGE ilişkisi (H11, H8). §6: H-OPAQUE-FAIL kalktı; H-CANCEL, H-PROGRESS eklendi; H-CUTOVER'a Superseded. Üstü çizili metin silinmedi. **İMZA: bu değişiklik tek imzalıdır (Abkarada); belge başlığı gereği Hayri imzalamadan sözleşme değildir.**

---

## 0. Sınır

```
Application (Hayri)                                        Transport / kama-core (Abkarada)
  ürün semantiği · mesaj şeması (AMP) · sıkıştırma kararı  ──►  connect/send/recv/events tek API
  Crypto'yu GÖRMEZ · Plane 1'i GÖRMEZ · retry YAPMAZ       ◄──  Plane 1 RPC istemcisi · batch/segment/ARQ/FEC/pacing
                                                                  · SessionControl'ü sürer · olayları typed iletir
```
Application'ın tek alt komşusu Transport'tur (INV-NEIGHBOR-1). Bu belge kama-core'un **app-facing API'sinin** tamamıdır; başka app-facing yüzey yoktur.

---

## 1. Arayüz / şema

```rust
pub trait KamaTransport {
    fn capabilities(&self) -> TransportCapabilities;
    fn connect(&mut self, peer: PeerRef, early_unit: Option<PreparedUnit>) -> Result<ConnectResult, TransportError>;
    fn send(&mut self, session: SessionHandle, unit: PreparedUnit, policy: DeliveryPolicy) -> Result<(), TransportError>;
    fn recv(&mut self) -> Option<Delivered>;                  // poll; sıralama garantisi §4
    fn poll_event(&mut self) -> Option<SessionEvent>;         // Ready | RenewRequired | Closed | ~~ConnectFailed~~ ConnectAbandoned | ConnectProgress  [DR-CONNECT-1]
    fn close(&mut self, session: SessionHandle);
    fn cancel(&mut self, ticket: ConnectTicket);              // [DR-CONNECT-1 H9] bekleyen niyetten TEK çıkış → ConnectAbandoned(ticket, Cancelled)
}
pub struct TransportCapabilities { pub max_plaintext_unit_bytes: u32, pub preferred_plaintext_unit_bytes: u32 }
pub struct PreparedUnit          { pub bytes: Bytes /* = AppMessageRecord, AMP §2 */, pub compression_policy: CompressionPolicy }
pub enum   CompressionPolicy     { Never, AppCompressed /* bit0 zaten set */ }   // §1 K4
pub enum   DeliveryPolicy        { Reliable, DeadlineBounded { deadline: Instant }, BestEffort }
pub struct Delivered             { pub session: SessionHandle, pub unit: Bytes /* AppMessageRecord */ }
pub enum   SessionEvent          { Ready(SessionHandle, PeerRef), RenewRequired(SessionHandle), Closed(SessionHandle, CloseReason),
                                   /* ~~ConnectFailed(ConnectTicket)~~ [DR-CONNECT-1 H9: KALDIRILDI — deadline yok] */
                                   ConnectAbandoned(ConnectTicket, AbandonReason), ConnectProgress(ConnectTicket, ConnectPhase) }
pub enum   AbandonReason         { Cancelled, PeerClosed, KeyHygiene, LocalPermanent }  // [DR-CONNECT-1] = sonlandırma nedenleri (a)–(d), §4; OPAK DEĞİL (§3)
pub enum   ConnectPhase          { Measuring, Suspended, LongOutage }                   // [DR-CONNECT-1] HUB YOLUNUN durumu; eşin canlılığı DEĞİL (§4)
// CloseReason ∋ { Renewed (KARAR-2), Superseded (DR-CONNECT-1 H6: peer-restart supersession), … }
pub enum   ConnectResult         { Ready(SessionHandle), Pending(ConnectTicket) }   // K2 KARAR-1: idempotent, olay üretmez
pub struct SessionHandle(/* opak */);   pub struct ConnectTicket(/* opak */);   pub struct PeerRef(/* opak; StaticPeerMaterialProvider anahtarı */);
```

**K1 — Application yalnız bu trait'i çağırır; Crypto'ya ve Plane 1'e erişimi YOKTUR.**
**G:** INV-NEIGHBOR-1 (`degree(Application)=1`). `connect()` içinde Transport, S3 SessionControl `Initiate` → `OutboundEnvelope` → Plane 1 `STORE_MESSAGE` zincirini kendisi sürer; `AcceptInit`/`AcceptResponse` yönlendirmesini DELIVER/`MailboxItem.envelope_type` ile yapar (app-rpc AMENDMENT-7). Application yalnız `Ready(h)` görür.
**R:** *App'in Plane 1 çerçevelerini kendisinin kurması* — reddedildi: Plane 1 hub-yönelik kontrol düzlemidir (kama-core'un); app'e açılırsa hub-RPC semantiği ürün koduna sızar ve INV-NEIGHBOR-1'in "mediated" kuralı bozulur. *App'in `initiate()` çağırması* — reddedildi (eski S2; app→crypto kenarı).
**S:** Plane 1 istemcisinin kodunu Hayri yazabilir — ama **kama-core'un içinde**, bu API'nin altında. Şerit adı ("Application RPC") ile katman adı ayrışır; bu belge katmanı esas alır.

**K2 — `SessionHandle`'ı yalnız Transport verir: yeni kuruluşta `Ready(h)` olayıyla, idempotent `connect()`'te mevcut handle `ConnectResult::Ready(h)` dönüşüyle. Application opak taşır; `Closed` sonrası kullanamaz.**
**G:** Crypto basar, Transport iletir (S3 K8); invariantlar transport-tdd §0 (epoch değil, wire'da yok, restart sonrası geçerli olmak zorunda değil, `HubHandle` ile ayrı tip).
**KARAR-1 — aynı peer'a ikinci `connect()`: İDEMPOTENT [kapandı 2026-09-19].** `connect()` → `ConnectResult`: aktif session varsa **`Ready(h)` döner (olay YAYINLAMAZ)**; pending varsa **aynı** `Pending(ticket)`; hiç yoksa yeni handshake + `Pending(ticket)`. `Ready` olayı yalnız gerçek state geçişinde çıkar — olay kuyruğu idempotency mekanizması **değildir** (duplicate `Ready` yok). Gerekçe: multi-device limit=1 (identity) ⇒ peer başına en fazla bir aktif session; paralel handshake KS-DOS-1 cap'ini boşa tüketir.
**S — KARAR-2 [kapandı 2026-09-19]: ATOMİK CUTOVER; cap'e bağlı DEĞİL.**
```
h aktif → RenewRequired(h) → arka planda h' kuruluyor (h hâlâ send kabul eder)
→ h' hazır → ATOMİK CUTOVER: h = draining (yeni send YOK, outstanding boşalır) · h' = aktif
→ Ready(h') → Closed(h, Renewed)
```
Cutover sonrası `send(h)` → **`SessionSuperseded`** (yapısal hata; oracle değil, app'in düzelteceği yerel lifecycle durumu). Reddedilen: *"eski h cap'e kadar kabul eder"* — davranışı Crypto'nun gizli sayaç durumuna bağlar (aynı `send` bazen çalışır, birkaç paket sonra çalışmaz) ve `h`/`h'` üzerinden eşzamanlı veri kabul edilirse cross-session sıralama bölünür (§4 yalnız aynı-session Reliable sıralaması garanti eder). **Cap Crypto'nun güvenlik sınırıdır, API semantiği değil** — App↔Transport sözleşmesine kripto iç runtime state'i sızdırılmaz.

**K3 — `PreparedUnit.bytes` bir `AppMessageRecord`'dur; Transport onu yapısal olarak işler, semantik olarak asla.**
**G:** INV-PLAINTEXT-NOT-SEMANTICS. Transport uzunluk/segment/batch (ccr §35.6 `BatchSubMessage.payload` = bir record; KARAR-1 S3) /FEC/deadline yapar; `msg_type`'ı okumaz.
**R:** *Transport'un tip-bazlı öncelik/sıkıştırma kararı vermesi* — reddedildi (tip bilgisi app'in). İhtiyaç varsa app **politika** verir (`DeliveryPolicy`, `CompressionPolicy`), semantik değil.
**S:** `bytes.len() ≤ max_plaintext_unit_bytes` (K5); aşarsa `UnitTooLarge`. Application record'u **kendisi böler** (FILE_CHUNK) — Transport birim içini bölmez; birden fazla record'u bir birime **paketler**.

**K4 — Sıkıştırma Application'da yapılır ve `PreparedUnit` içinde işaretlidir; Transport sıkıştırmaz.**
**G:** AMENDMENT-4/6 (transport-tdd §8, app-rpc §3): karar tip bilgisi ister; AMP §5 wire sözleşmesi (`flags.bit0`, `compression_id`, izole context, bounded decompress). `CompressionPolicy` alanı Transport için **bilgilendiricidir** (istatistik/ileride pacing ipucu), davranış değiştirmez.
**R:** *Transport'un `compressible` ipucuyla sıkıştırması* — kod kanıtı aktüatör yok (fusion.rs:551); karar app'te; alan ileride bu yönde genişlemeye açık bırakıldı (v2).
**S:** Alıcı tarafta decompress de Application'da (AMP §5 kuralları 1–4); Transport `Delivered.unit`'i olduğu gibi verir.

**K5 — `TransportCapabilities` salt-okunur; değerleri Transport türetir, Application hesaplamaz.**
**G:** Application yalnız Transport'u bilir; 18 B E2E kayıt ek yükü ve 4 B `hub_handle` (ccr AC-SM3-2) Transport'un **alt komşusundan** öğrendiği sayılardır. Kaynak S3 iki sorgu: **`conservative_limits(ProtectionClass::Session)`** — handle gerektirmez, tüm session profilleri (legacy/extended) için güvenli **minimum**; S1 `capabilities()` (global, session öncesi) bunu kullanır — ve **`limits(ProtectionContext::Session(h))`** — aktif session'ın gerçek profili; v2 `capabilities(session)` için. Transport kripto format matematiğini **hiçbir yerde tekrar etmez**. Zincir: `Crypto conservative_limits(class) → Transport MTU/framing/segment bütçesi → TransportCapabilities.max_plaintext_unit_bytes → Application`.
**R:** *Application'ın 18'i bilmesi* — reddedildi: Crypto'ya ait sabit app'e sızar, format değişince sessiz kırılma. Eski `SecurityCapabilities`/orkestratör composition modeli (X-dönemi) **iptal**.
**S:** `preferred_plaintext_unit_bytes` batching için ipucu; app record'ları ona göre boyutlandırırsa daha az birim; zorunlu değil.

**K6 — `early_unit` opak bir `PreparedUnit`'tir; Transport onu S3 `Initiate{early_unit}` ile geçirir.**
**G:** transport-tdd §0 istisna + ccr §35.3 AMENDMENT-6: Crypto opak işler. `first_message` adı bilinçli **reddedildi** (Crypto'nun TEXT bildiği izlenimi).
**S:** Boyutu `max_early_unit_bytes` `[KARAR — ccr §35.3 early_data sınırından türer]`; aşarsa `UnitTooLarge`. Teslimi RESPONSE gibi Plane 1 üzerinden; ACK'i **yoktur** (async), app receipt'le öğrenir (K8).
**İdempotent `connect` ile etkileşim [kapandı 2026-09-19]:** `early_unit` **yalnız yeni handshake yaratılırken tüketilir.** Pending INIT byte-for-byte cache'lendiği için (INV-INIT-KEY-1) mevcut handshake'e yeni early data eklenemez. Kural: aktif veya pending peer'a `connect(peer, Some(X))` → **`EarlyUnitConflict`** (yapısal hata; X **sessizce düşürülmez**, hiçbir byte tüketilmez); `connect(peer, None)` → K1 idempotent davranışı aynen. Application X'i `Ready(h)` sonrası normal `send()` ile yollar.
**Glare kaybında `early_unit` [K2 — kapandı 2026-10-04]:** Transport `early_unit`'in kopyasını oturum `Ready` ya da `ConnectAbandoned` olana kadar SAKLAR. Glare'de kendi INIT'i düşen (kaybeden) tarafta, kazanan oturum `Ready(h', peer)` olunca Transport saklanan birimi `h'` üzerinden **ilk normal `send()`** olarak yollar; Application'a olay çıkmaz ve Application yeniden vermez. Application `early_unit`'i YALNIZ `ConnectAbandoned` aldığında yeniden verir. Gerekçe: yeni handshake turu açmadan, tek kayıtla niyet korunur; nadir çift işlemeyi `msg_id` dedup'u önler. *(Bu, §4'teki "`cancel` ⇒ Transport onu saklamaz" cümlesiyle çelişmez: saklama `Ready`/`ConnectAbandoned`'a kadardır, sonrasında bırakılır.)*

**K7 — Olaylar typed'dır; Transport yorumlamadan iletir.**
**G:** S3 K6/K8; transport-tdd §0 ("opaque" DEĞİL). `Ready(h, peer)` peer'ı taşır ki app konuşmaya eşlesin; ~~`ConnectFailed(ticket)` opak sebeple (§3)~~ **[DR-CONNECT-1]** `ConnectAbandoned(ticket, reason)` typed sebeple, `ConnectProgress(ticket, phase)` hub yolunun durumuyla (§3, §4).
**S:** Olay sırası bir session için garanti: `Ready` → (`RenewRequired`)* → `Closed`. `Ready` gelmeden `send(h)` → `UnknownSession`.
~~**S — Peer-restart supersession [DR-CONNECT-1 H6]:** Mevcut oturumu olan bilinen bir eşten doğrulanmış YENİ bir INIT gelirse (eş durumunu kaybetmiş; identity §16) olay sırası: **`Ready(h', peer)` → `Closed(h, Superseded)`**. Sonrasında `send(h)` → `SessionSuperseded`. `h` üzerinde teslim edilmemiş birimleri Transport yeni oturuma TAŞIMAZ; Application `msg_id` ile `h'` üzerinden yeniden gönderir (bu retry değil, yeni oturumda yeni gönderimdir — K8'in "app retry yapmaz" kuralı aynı oturum içindir).~~ *[2026-10-04: AŞILDI — aşağıdaki provisional kuralla değişti.]*
**S — Supersession, PROVISIONAL [DR-CONNECT-1 H6, 2026-10-04]:** Mevcut oturumu (`h`) olan bilinen bir eşten doğrulanmış, yeni `init_counter`'lı bir INIT gelirse yeni oturum provisional kurulur; **provisional dönem Application'a GÖRÜNMEZ** ve `h` tam çalışır (`send(h)` kabul). Olay sırası: **`Ready(h', peer)`** (ONAYDA = `h'` üzerinde ilk AEAD-geçerli epoch-1 paketi) **→ `h` yalnız-alım drain** (KS-GRACE-1: `N_g = 64` / `T_g = 30 s`, hangisi önce; bu sürede `recv()` `h`'den teslim edebilir) **→ `Closed(h, Superseded)`**. `Ready(h')` anından itibaren `send(h)` → `SessionSuperseded`. `h` üzerinde teslim edilmemiş birimleri Transport yeni oturuma TAŞIMAZ; Application `msg_id` ile `h'` üzerinden yeniden gönderir (alıcıda `msg_id` dedup güvenli kılar; bu aynı-oturum retry'ı değildir, K8 korunur). Onaylanmayan provisional oturum hiçbir olay üretmeden düşer; `h`'ye dokunulmaz. Alıcı Renew ile peer-restart'ı ayırt etmez — ikisi bu yoldan geçer (identity §16).
**S — Glare [DR-CONNECT-1, 2026-10-04]:** iki taraf aynı anda `connect` ederse (identity §16 tie-break: `dest_hash`'i küçük olan kazanır) tek oturum doğar ve her iki Application da TEK `Ready(h, peer)` görür; kaybeden tarafın `Pending(ticket)`'i geçerli kalır ve o `Ready` ile çözülür.

**K8 — Application teslimi Transport'tan öğrenmez; receipt AMP semantiğidir.**
**G:** LAW-5 + INV-NEIGHBOR-1: transport ACK hop/E2E-ARQ'nun iç işi (S3), app'e sızmaz. "Bob'un uygulaması işledi" bilgisi yalnız `RECEIPT` record'uyla (AMP 0x08) gelir. `send()`'in `Ok(())` dönmesi = kuyruğa alındı, teslim değil.
**R:** *`send()`'in teslim callback'i vermesi* — reddedildi: E2E teslim tanımı (hub aldı mı, Bob aldı mı, Bob açtı mı) katmanlara göre farklı; tek bir "delivered" app'i yanıltır. Async yolda anlamsız.
**S:** Application **hiçbir retry yapmaz**; `send` başarısızsa (`Backpressure`, `DeadlineExpired`) yalnız kullanıcıya yansıtır ya da bekler.

---

## 2. Byte sözleşmesi ve vektörler
Bu seam **wire tanımlamaz**. Sınırdan geçen byte'lar: `PreparedUnit.bytes` = AMP `AppMessageRecord` (AMP §2/§5 tanımlı) — `Delivered.unit` aynı record byte-identical geri gelir. **Vektörler:** AMP §7 codec conformance seti (pozitif/negatif record'lar) + **seam-özel:** `send` → (MockTransport) → `recv` byte-özdeşlik; `early_unit` byte-özdeşlik; `UnitTooLarge` sınırı tam `max_plaintext_unit_bytes`'ta. Kripto vektörü bu seam'in kapısı değildir.

## 3. Hata semantiği
```
TransportError ∈ { UnknownSession, SessionSuperseded, EarlyUnitConflict, UnitTooLarge, Backpressure, DeadlineExpired, NotConnected }
~~ConnectFailed(ticket)  — sebep OPAK (kripto Failed'ı, Plane 1 deadline'ı, route miss aynı görünür)~~   [DR-CONNECT-1 H9: KALDIRILDI]
ConnectAbandoned(ticket, reason) — reason ∈ { Cancelled, PeerClosed, KeyHygiene, LocalPermanent }; OPAK DEĞİL   [DR-CONNECT-1 H9]
```
**K9 — Application'a kripto nedeni sızmaz.** S3 §3 (`Failed` opak, `open()→None` sessiz) burada da korunur; ~~`ConnectFailed` tek tip~~. Gerekçe: oracle yüzeyi yok; app güvenlik kararı veremez. Yapısal hatalar (boyut, backpressure) typed'dır çünkü app'in düzeltebileceği şeylerdir.
**[B3 — 2026-10-05] `DeadlineExpired` KALIR ve birim deadline'ıdır.** `DeliveryPolicy::DeadlineBounded` ile verilen birim (PTT/medya) iletilmeden süresi dolarsa `send()` bunu döner; dönüş anı ve sonradan sinyal olmaması §4'teki kuraldır (değişmedi). DR-CONNECT-1 bağlantı deadline'ını kaldırdı (`connect()` için süre sınırı yok, §4 H10), birim deadline'ını değil — ikisi ayrı şeylerdir.
**[DR-CONNECT-1 H9] `ConnectAbandoned.reason` OPAK DEĞİLDİR.** Bu yerel bir yaşam-döngüsü durumudur, kripto oracle'ı değil; yukarıdaki opaklık kuralı kripto hatalarına aittir, buna değil. `reason` yalnız §4'teki kapalı listeden gelir ve kripto ayrıntısı taşımaz. Kapalı listenin sonucu: bozuk/sahte RESPONSE, route miss ve yol kesintisi bir bekleyen niyeti SONLANDIRAMAZ — kripto `Failed`'ı hâlâ opaktır ve Application'a hiçbir olay olarak çıkmaz (pending yaşar; KM-SEAM-TC-001 K7).
**[2026-10-06] Pending tavanı dolu → `Backpressure`.** İstemci pending INIT tavanı (ccr §35.15 KS-DOS-1; tutarlılık zinciri gereği `PENDING_RESPONSE_TRIAL_CAP` ile eşit) doluyken yeni bir eşe `connect()` → `TransportError::Backpressure`; yeni pending OLUŞMAZ, mevcut pending kümesi ve state değişmez. Yapısal hatadır (K9): Application bekler ya da bir niyeti `cancel` eder. Tavanın ötesinde aday var olamayacağı için ona ait bir RESPONSE Crypto'da tavan kadar denemeden sonra opak `Failed` ile biter (KM-SEAM-TC-001 K7, H-DEMUX).

## 4. Zamanlama / deadline
- Application `DeliveryPolicy` **beyan eder**; Transport **uygular** (transport-tdd §0). `DeadlineBounded` süresi dolarsa birim sessizce düşer ve `DeadlineExpired` yalnız `send()` anında (kuyruk dolu) döner — teslim edilemeyen birim için sonradan sinyal **yoktur** (K8).
- ~~`connect()` süresi Transport'un Plane 1 `Reliable` deadline'ıdır; dolarsa `ConnectFailed(ticket)`. **Semantik (donmuş):** `ConnectFailed` ⇒ pending kripto state kapatılır (ephemeral zeroize — ancak T-18c'nin "RESPONSE 7 güne kadar gelebilir" kuralıyla çelişmemesi için bu deadline yalnız **online-direct** yoluna uygulanır; mailbox/async yolunda `connect` deadline'ı YOKTUR, pending mailbox TTL'iyle yaşar) · `early_unit` **teslim edilmemiştir** ve Application tarafından yeni `connect`'ten sonra normal `send()` ile yeniden verilmelidir (Transport onu saklamaz) · ticket geçersizleşir. Değeri `[KALİBRASYON — KS-8]`.~~ *[DR-CONNECT-1 H10, 2026-10-03: SİLİNDİ (üstü çizili) — aşağıdaki metinle değişti.]*
- **[DR-CONNECT-1 H10 — KS-8 KAPANDI: deadline YOKTUR; sayı yok.]** `connect()` için süre sınırı yoktur. Bekleyen niyet zaman geçtiği için sonlandırılmaz (INV-NO-IDLE-TEARDOWN, transport-tdd §0). `Pending(ticket)` yalnız üç yolla biter: `Ready(h, peer)` · `cancel(ticket)` · `ConnectAbandoned(ticket, reason)`. Kanıt (`docs/sprint_plan/A1-KALIBRASYON-KARAR-FOYU.md` KANIT PAKETİ K1–K3): transport hub kanalı kuruluşunda pes etmez; `DeliveryPolicy::Reliable`'ın deadline alanı yoktur ("Plane 1 `Reliable` deadline'ı" yapısal olarak yoktu); STORE_MESSAGE yanıtı yalnız `ACCEPTED_QUEUED`'dir, "doğrudan teslim / mailbox" ayrımı senkron kurulamaz.
- **Sonlandırma nedenleri [DR-CONNECT-1 H2 — kapalı liste; "zaman" yok]:** (a) kullanıcı/uygulama cancel → `Cancelled` · (b) karşı tarafın close'u → `PeerClosed` · (c) anahtar hijyeni (`SEND_PN_CAP`, `SESSION_MAX_AGE`, T-18c ephemeral ömrü) → `KeyHygiene` · (d) kalıcı yerel hata (peer material yok, sertifika iptal/expire) → `LocalPermanent`. Başka hiçbir şey bir oturumu ya da bekleyen niyeti kapatamaz.
- **`cancel(ticket)` [H9]:** kullanıcı/uygulama vazgeçince TEK çıkış. ⇒ pending kripto state kapatılır (ephemeral zeroize) · `early_unit` **teslim edilmemiştir** ve Application tarafından yeni `connect`'ten sonra normal `send()` ile yeniden verilmelidir (Transport onu saklamaz) · ticket geçersizleşir · `ConnectAbandoned(ticket, Cancelled)` yayınlanır.
- **`ConnectProgress(ticket, phase)` [H9]:** UI "gönderiliyor" ile "ağ yok, bekliyor"u ayırabilsin diye. `Measuring` = hub yolu ölçülüyor (prob sürüyor) · `Suspended` = link yok, prob askıda (transport-tdd §10 [AMENDMENT-10]) · `LongOutage` = ~~uzun kesinti `[eşik TANIMSIZ — §8 açık kalem]`~~ **[2026-10-04, KS-UI-1 = 60 s KAPANDI]** heartbeat yanıtı 60 s'dir yok (transport-tdd §10 [AMENDMENT-10] HOP HEARTBEAT); YALNIZ UI içindir, protokol parametresi DEĞİLDİR — hiçbir şeyi sonlandırmaz, kadansı değiştirmez. **Bu olaylar HUB YOLUNUN durumudur. Bob'un canlılığı YALNIZ AMP `RECEIPT(delivered)` ile bilinir** (K8); hop-ACK ve prob yanıtı eş hakkında bilgi vermez.
- **T-18c ile ilişki [H11]:** 7 gün sonunda pending ephemeral silinir; NİYET KORUNUR; Transport yeni ephemeral ile yeni INIT üretir. 7 gün bir deadline değil, anahtar hijyenidir (identity §21 T-18c, §16).
- **SESSION_MAX_AGE [H8, KS-AGE-1 = 7 gün, KAPANDI 2026-10-04]:** oturum yaşı aşılınca bir sonraki gönderimde `RenewRequired(h)` → KARAR-2 cutover. Timeout değildir; oturum ölmez, yenilenir (identity §16).
- `recv()` sıralaması: **aynı session içinde `Reliable` birimler gönderim sırasında** (transport reassembly + E2E replay penceresi); `BestEffort`/`DeadlineBounded` sırasız gelebilir. Application AMP `msg_id` ile kendi sırasını tutar.
- Application timer'ı yalnız UI/ürün amaçlı; **retransmit timer'ı YOKTUR** (LAW-5).

## 5. Sürümleme
`KamaTransport` API v1 · `PreparedUnit` içi AMP `msg_version` (bağımsız) · `TransportCapabilities` alan ekleme = minor, imza değişimi = v2. Plane 1/2 sürümleri (app-rpc `protocol_version`, ccr) app'e görünmez.

## 6. Conformance harness (= anlaşmanın kendisi)
| Test | Kanıtladığı |
|---|---|
| H-NO-CRYPTO-EDGE | Application crate'inin Crypto crate'ine **derleme bağımlılığı yok** (statik); Plane 1 tiplerine bağımlılığı yok |
| H-APP-RETRY-0 | Deterministik kayıp altında `recovery ON` → teslim; `recovery OFF` → kasıtlı düşürülen birim **kurtarılmaz**; app kodunda retransmit yolu yok |
| H-ROUNDTRIP | `send(PreparedUnit)` → `recv()` byte-identical; `early_unit` byte-identical |
| H-EVENTS | `Ready → RenewRequired → Ready(h') → Closed(h, Renewed)` sırası; `Closed` sonrası `send(h)` → `UnknownSession` |
| H-CUTOVER | Cutover öncesi `send(h)` kabul; cutover sonrası `send(h)` → `SessionSuperseded`; `h`'nin outstanding'i drain olur; `h'` üzerinden sıralama korunur; davranış Crypto cap değerinden BAĞIMSIZ (cap değiştirilse de aynı). ~~**[DR-CONNECT-1 H6]** + peer-restart supersession: bilinen eşten yeni INIT → `Ready(h') → Closed(h, Superseded)`; `send(h)` → `SessionSuperseded`; aynı `init_counter`'lı duplicate INIT supersession ÜRETMEZ~~ **[DR-CONNECT-1 H6, 2026-10-04 — provisional]** bilinen eşten yeni-sayaçlı INIT: onaydan ÖNCE `send(h)` kabul ve hiçbir olay yok; onayda `Ready(h')`, `send(h)` → `SessionSuperseded`, `h` yalnız-alım drain (N_g/T_g) → `Closed(h, Superseded)`; onaylanmayan (bayat) INIT `h`'yi ETKİLEMEZ ve olay üretmez; aynı `init_counter`'lı duplicate INIT supersession üretmez; glare → tek `Ready` |
| H-CONNECT-IDEMP | aktif peer → `Ready(h)` döner, olay YOK; pending → aynı ticket; olay kuyruğunda duplicate `Ready` yok |
| H-EARLY-CONFLICT | aktif/pending peer'a `connect(peer, Some(X))` → `EarlyUnitConflict`; X hiçbir yola girmez (MockTransport/Crypto'da byte izi yok); `connect(peer, None)` idempotent |
| H-CAPS | `len == max` kabul, `len == max+1` `UnitTooLarge`; değer Transport'tan, app'te sabit yok |
| ~~H-OPAQUE-FAIL~~ | ~~Yanlış peer material, bozuk RESPONSE, Plane 1 timeout → ayırt edilemez `ConnectFailed`~~ **[DR-CONNECT-1: KALKTI — `ConnectFailed` yok; kripto opaklığı K9'da sürer; yerine H-SILENT-REJECT]** |
| H-CANCEL | **[DR-CONNECT-1]** `Pending(ticket)` → `cancel(ticket)` → `ConnectAbandoned(ticket, Cancelled)`; pending kripto state kapanır; `early_unit` hiçbir yola girmez; ticket geçersiz, sonraki `connect(peer)` yeni ticket verir. Sanal saat ne kadar ilerletilirse ilerletilsin niyet KENDİLİĞİNDEN sonlanmaz (INV-NO-IDLE-TEARDOWN)~~; bozuk/sahte RESPONSE olay üretmez, pending yaşar~~ *(→ H-SILENT-REJECT)* |
| H-PROGRESS | **[DR-CONNECT-1]** link yok → `ConnectProgress(Suspended)` ve 0 prob emisyonu; link gelir → `Measuring` + anında prob; `ConnectProgress` hiçbir durumda eşin canlılığını bildirmez (yalnız hub yolu) |
| H-SILENT-REJECT | **[DR-CONNECT-1, 2026-10-04]** sahte/geçersiz RESPONSE hiçbir olay üretmez, pending yaşar: ticket geçerli kalır, `ConnectAbandoned` çıkmaz, sonradan gelen geçerli RESPONSE aynı ticket'ı `Ready`'ye çözer; kripto nedeni hiçbir olayda görünmez (K9) |
| H-NO-PLANE1 | Application hiçbir Plane 1 method_id/çerçeve üretmez (statik kontrol) |
| H-COMPRESS-BOUNDARY | `CompressionPolicy` Transport davranışını değiştirmez; decompress yalnız app'te (AMP §5 kuralları) |

## 7. Karşı-taraf stub'ları
**`MockTransport`** (Hayri için): deterministik kayıp/sırasız/duplicate enjeksiyonu, `Ready/Renew/Closed` senaryoları, kapasite sabitleri, ~~`ConnectFailed` enjeksiyonu~~ **[DR-CONNECT-1]** `ConnectAbandoned`/`ConnectProgress` enjeksiyonu + sanal saat + supersession senaryosu; Plane 1 ve Crypto **içermez**. **`MockApplication`** (Abkarada için): AMP record üretici/tüketici, `connect→send→recv` akışı, receipt üretimi; retry yok.

## 8. Açıklar
| # | Kalem | Sahip |
|---|---|---|
| ~~KARAR-1~~ | KAPANDI: idempotent, `ConnectResult::{Ready, Pending}`, duplicate olay yok | — |
| ~~KARAR-2~~ | KAPANDI: atomik cutover, `SessionSuperseded`; cap'e bağlı değil | — |
| ~~⬛ KS-8~~ | ~~`connect` Plane 1 deadline değeri~~ **KAPANDI 2026-10-03 — DR-CONNECT-1 H10: deadline yoktur, değer yok** | — |
| ~~⬛ DR-CONNECT-1~~ | ~~`LongOutage` eşiğinin tanımı · alıcı tarafta Renew ↔ peer-restart ayrımı (ikisi de "bilinen eşten yeni INIT"; eski oturumun outstanding'i drain mi olur, app mi yeniden gönderir) · eşzamanlı INIT (glare) · S3'te `CloseReason::Superseded` kaynağı (`AcceptInit`)~~ **KAPANDI 2026-10-04:** `LongOutage` = 60 s (KS-UI-1) · ayrım GEREKMİYOR (provisional supersession) · glare tie-break (dest_hash) · S3 K10 | — |
| ~~⬛ DR-CONNECT-1~~ | ~~glare kaybedeninin `early_unit`'i (kendi INIT'i düştü; Application'a olay çıkmıyor — yeniden kim verir?) · jitter'daki `client_id` ve hash fonksiyonunun adı~~ **KAPANDI 2026-10-04:** `early_unit`'i Transport yeniden gönderir (K6) · jitter tohumu `hub_handle` / hop ephemeral, BLAKE2s-256 (transport-tdd §10 [AMENDMENT-10]) | — |
| ⬛ İMZA | DR-CONNECT-1 API değişikliği (`cancel` / `ConnectAbandoned` / `ConnectProgress` / `ConnectFailed`'ın kalkması) | Hayri — **imza G-S1 (S1 freeze) ön koşuludur, merge'in değil** [2026-10-04] |
| ~~S3 ekleme~~ | KAPANDI: `conservative_limits(class)` + `limits(Session(h))` — KM-SEAM-TC-001 §1.1 | — |
| AMP v1 | A1–A5 kararları; §7 codec conformance vektörleri (bu seam'in de kapısı) | Hayri |
| — | §6 harness + §7 stub'lar | Hayri + Abkarada, Hasan koşar |

## 9. Bu belgenin sınırı
Kripto doğruluğunu tanımlamaz (ccr/identity/S3). Plane 1 wire'ını tanımlamaz (app-rpc). Mesaj şemasını tanımlamaz (AMP). Media'yı görmez. Yalnız "Application, Transport'a ne verir, ne alır, neyi asla görmez" sorusunu kapatır.
