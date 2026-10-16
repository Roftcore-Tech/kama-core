# KM-SEAM-AK-001 — kama-app (Kotlin, `ui-api v0.3`) ↔ kama-core (Rust, UniFFI) seam'i — S5'in Rust karşılığı

| Alan | Değer |
|---|---|
| Belge | `docs/seams/KM-SEAM-AK-001.md` v0.1 · 2026-10-16 |
| Durum | **PoC** |
| Kart | K4G-15 (`KAMA-PLAN-4GUN-APP-KRIPTO` v1.1) — MOBILE-HANDOFF kapısı; icra Claude Code (L-FFI) |
| Sahip | Abkarada (Rust yüzeyi) · Hasan (Kotlin facade ve `ui-api`) |
| S5 kaynağı | `ui-api v0.3` — kama-engine `docs/ui-api-v0.md`, tag `ui-api-v0.3` = `5de9be16`, sha256 `7d409bdb15cc279612d5ac3ef09c7857569a4474a47745673701c01e8b1c2d14` |
| Sürüm beyanı | `ui-api-v0.4` ve `ui-api-v0.5` etiketleri mevcut; bu belge plan gereği **v0.3**'ü kapsar. |
| Rust yüzeyi | `crates/kama-mobile` (UniFFI 0.32.2, proc-macro) → `kama_node::KamaNode` (K4G-11). Kotlin bağlayıcısı: `uniffi.kama_mobile` paketi, `kama_mobile.kt` |
| Dayanak | Plan K4G-15, D9, D10, D11 · PoC-KAPSAM-001 §2 "Sözleşmeler", K6, EK-2, EK-3 · DUR-K4G-11 → B · DUR-K4G-11b → (b) |

S5'in kendisi `ui-api v0.3`'tür (Hasan; PoC-KAPSAM-001 §2 "Sözleşmeler": FROZEN sayılır, değişmez). Bu belge onu
**değiştirmez**; her çağrı ve alanın gerçek motordaki karşılığını tek satırda yazar (K6: "facade değişmez, dar
implemente edilir"). Satırın eşlemesi üç biçimden biridir:

* **Rust: `KamaMobile.xxx`** (ya da olay `EngineEvent.Xxx`) — gerçek motor bu Rust çağrısı/olayıyla karşılar;
* **PoC'de etkisiz gövde (K6)** — facade öğeyi korur, PoC'de üretilmez ya da çağrı hiçbir şey yapmaz;
* **Kotlin tarafı (Hasan)** — öğe facade'ın kendi işidir; Rust'ta karşılığı yoktur (varsa girdisi notta).

## 0. Sınır

```
kama-app (UI) ── ui-api v0.3 ──▶ RealKamaEngine (Kotlin facade, Hasan)
                                        │  UniFFI: kama_mobile.kt (üretilmiş), JNA
                                        ▼
                                 KamaMobile (crates/kama-mobile) ── birebir ──▶ KamaNode (crates/kama-node)
```

* UniFFI `Flow`/`StateFlow` vermez; olaylar callback interface `EngineListener.onEvent(ev: EngineEvent)` ile gelir.
  Akış ve durum yüzeyi (`StateFlow`, "conflated + replay=1") facade'ın inşasıdır (ui-api §2).
* `onEvent` düğümün kendi iş parçacığında çağrılır (hızlı dönmeli; içinden yanıt bekleyen çağrı
  `NodeException.ReentrantCall`). Kotlin'in attığı istisna Rust'ta yutulur; düğüm etkilenmez.
* FFI geçişi çağrı ve olay başınadır (metin, dosya, PTT demeti, sinyalleşme blob'u); ağ datagramları FFI'ı geçmez
  (ADR-15: paket başına geçiş yok).
* Bu sürümde iki telefon arası oturum yoktur: el sıkışma zarfı taşıyıcısı süreç-içi röledir (DUR-K4G-11 → B); iki
  cihaz arası taşıyıcı hub Plane 1 rölesidir (S4, hub sprinti).

## 1. Rust yüzeyi — `KamaMobile` (Kotlin adlarıyla)

| `KamaMobile` | `KamaNode` | Not |
|---|---|---|
| `KamaMobile.start(configPath, dataDir, listener)` | `KamaNode::start` | Kurulum; tokio/async bağlamından çağrılmaz, JVM iş parçacığından çağrılır. Hata: `NodeException.Config`, `NoDirectAddress`, `DataDir`, `Store`, `PeerDirectory`, `IdentityInUse`, `Transport`, `Limits`, `Spawn` |
| `connect(peerId)` | `connect` | Kalıcı niyet, idempotent; sonuç `ConnectProgress` / `Connected` / `Closed` |
| `cancelConnect(peerId): Boolean` | `cancel_connect` | `true` = niyet vardı |
| `sendText(peerId, text): UInt` | `send_text` | `msgId` (eş başına kalıcı); teslim `EngineEvent.Receipt`; oturumsuz → `Messenger(NoSession)` |
| `sendFile(peerId, path): FileHandle` | `send_file` | Yerel dosya yolu; tutamaç hemen döner; `FileProgress` / `FileFailed` |
| `cancelFile(file)` | `cancel_file` | Sonuç `FileFailed(Outgoing(Cancelled))` |
| `pttOpen(peerId, codec, bundleMs): StreamHandle` | `ptt_open` | `codec`/`bundleMs` çağıranın değeri |
| `pttPush(stream, payload)` | `ptt_push` | Reliable; reddedilen demeti çağıran yeniden verir |
| `pttClose(stream)` | `ptt_close` | Tutamaç her durumda kapanır |
| `sendSignaling(peerId, bytes): ULong` | `send_signaling` | Alındısız, dedupsuz; dönen değer motorun yerel kimliği |
| `onPlatformSignal(signal)` | `on_platform_signal` | Yalnız `PlatformSignal.LINK_CHANGED` (D15: protokol davranışı yok) |
| `drainBackground()` | `drain_background` | Outbox + yeniden gönderim pompası, oturumsuz eşe `connect` (EK-2) |
| `metrics(): NodeMetrics` | `metrics` | Bloklamaz. FFI farkı: `shutdown` sonrası `NodeException.Stopped` |
| `shutdown()` | `shutdown` | İdempotent; sonrasında her çağrı `Stopped`; aynı `dataDir` ile yeniden `start` edilebilir |
| `EngineListener.onEvent(ev)` | `NodeListener::on_event` | Olaylar: §2 |

Tutamaçlar (`StreamHandle`, `FileHandle`: `raw: ULong`) düğüm-tekildir, yeniden kullanılmaz; kapanmış tutamaç
`HandleClosed`, hiç verilmemiş değer `UnknownHandle` alır (tel kimliği API tutamacı değildir).

## 2. Olaylar ve hatalar

`EngineEvent` (`NodeEvent` aynası): `ConnectProgress(peerId, phase)` · `Connected(peerId)` · `Closed(peerId, reason:
CloseCause)` · `TextReceived(peerId, msgId, text)` · `Receipt(peerId, msgId)` · `FileProgress(file, peerId, stage:
SENT|DELIVERED)` · `FileReceived(peerId, transferId, path, size)` · `FileFailed(file: FileRef, peerId, reason:
FileFailReason)` · `PttOpened(peerId, streamId, codec, bundleMs)` · `PttBundle(peerId, streamId, payload)` ·
`PttClosed(peerId, streamId)` · `SignalingReceived(peerId, bytes)` · `Metrics(metrics: NodeMetrics)`.

`NodeMetrics`: `rttMs`/`lossPct`/`bitrateKbps` bu sürümde `null` (overlay, hub sprinti) · `epoch` ·
`lastHeartbeatAgeS` (`null`, D18) · `ctxMismatchDrops` · `hubFramesDropped` · `deliveryChannelDrops` ·
`connectState: List<PeerConnectState(peerId, state: LinkState, epoch)>` (provisioning'deki bütün eşler, ad sırasıyla).

`NodeException` (`NodeError` aynası; Kotlin'de sealed): kurulum hataları (`Config`, `PeerDirectory`, `NoDirectAddress`,
`DataDir`, `Store`, `Transport`, `Limits`, `Spawn`, `IdentityInUse`; `reason` yalnız sınıf/konum/neden taşır) · çağrı
hataları (`UnknownPeer`, `UnknownHandle`, `HandleClosed`, `HandlesExhausted`, `TooLarge(what, len, max)`, `Busy`,
`Stopped`, `ReentrantCall`, `NodeThreadPanicked`) · motor redleri tipli nedenle: `Messenger(MessengerError)`,
`Ptt(PttError)`, `File(FileSendError)`, `Signaling(SignalingError)` (iç `TransportError`, `EncodeError`,
`AppStoreError` dahil).

## 3. K6 listesi (PoC-KAPSAM-001 K6; plan K4G-15 NE 2, birebir)

| K6 maddesi | Eşleme | Not |
|---|---|---|
| Enrollment → provisioning içe aktar → `Ready` | Rust: `KamaMobile.start` | İçe aktarmanın UI'ı ve dosyanın `noBackupFilesDir`'e yazılması Kotlin'de; `start` başarısı = `Ready` (`FakeSeed.Mode.Enrolled` eşdeğeri) |
| Contacts = provisioning listesi | Rust: `KamaMobile.metrics` → `NodeMetrics.connectState[].peerId` | Provisioning dosyasındaki bütün eşler, ad sırasıyla |
| `syncDirectory`/`lookupUsername` etkisiz | PoC'de etkisiz gövde (K6) | §4.6 |
| Devices tek satır | Kotlin tarafı (Hasan) | Bu cihaz; §4.7 |
| Push etkisiz gövde | PoC'de etkisiz gövde (K6) | §4.8 |
| `retryAfterTrustFailure` etkisiz gövde | PoC'de etkisiz gövde (K6) | `TrustFailure` PoC'de üretilmez (zincir/iptal doğrulaması yok, ccr §35.4) |
| Gate-3 bildirimi etkisiz gövde | PoC'de etkisiz gövde (K6) | `deviceApprovedUnseen` hep `false` |
| `background.drain()` = `drain_background()` | Rust: `KamaMobile.drainBackground` | EK-2: yerel outbox ve Transport yeniden gönderim kuyruğu yeniden pompalanır, gerekirse `connect`; hub mailbox yok, periyodik worker yok |
| `notifications.state` kullanılır | Kotlin tarafı (Hasan) | Girdi `EngineEvent.TextReceived`; gösterim app'te |
| `attachments.open` kullanılır | Kotlin tarafı (Hasan) | Gelen ekin yolu `EngineEvent.FileReceived.path` (Rust); URI Kotlin'de |
| `onPlatformSignal` kullanılır | Rust: `KamaMobile.onPlatformSignal` | Yalnız `LINK_CHANGED` Rust'a geçer (D15) |
| `CallsApi` kullanılır (`CallsApi` sinyalleşmesi `send_signaling` üzerinden, medya Kotlin/medya motorunda) | Rust: `KamaMobile.sendSignaling` + `EngineEvent.SignalingReceived` | `CallsApi` v0.3'te tanımlı değil (v0.3 §7: E-12 KAPSAM DIŞI); yüzeyi bu belgenin kapsamı dışında. Canlı medya kama transport'tan geçmez (EK-3) |

## 4. `ui-api v0.3` satır eşlemesi

Her satırın ilk sütunu v0.3 belgesindeki adı (ve aynı öğenin belgedeki diğer yazımlarını) taşır. Kapsama yöntemi §5.

### 4.1 Motor ve yaşam döngüsü

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `KamaEngine` | Kotlin tarafı (Hasan) | Application-tekil facade; `RealKamaEngine` bir `KamaMobile` tutar |
| `RealKamaEngine` | Kotlin tarafı (Hasan) | `FakeKamaEngine`'den tek değişiklikle geçilen gerçek facade |
| `KamaEngines.create` · `create` | Rust: `KamaMobile.start` | Provisioning dosyası yolu + uygulamaya özel veri dizini + dinleyici; ana iş parçacığı dışında |
| `context` | Kotlin tarafı (Hasan) | Yalnız `noBackupFilesDir` yolunu türetmek için; Rust'a geçmez |
| `scope` | Kotlin tarafı (Hasan) | Coroutine kapsamı; Rust olayları callback ile verir |
| `seed` | Kotlin tarafı (Hasan) | Fake modu; gerçek motorda yerini provisioning dosyası alır |
| `close` | Rust: `KamaMobile.shutdown` | Ardından UniFFI nesnesinin `close()`'u; süreç kapanışı |
| `KamaEngine.self` · `SelfIdentity` | Kotlin tarafı (Hasan) | Provisioning dosyasının `device_id`'si (PROVISIONING-FORMAT-v0 §3.2); Rust API'sinde öz kimlik çağrısı yok |
| `username` (SelfIdentity) | Kotlin tarafı (Hasan) | `device_id` |
| `orgName` | Kotlin tarafı (Hasan) | PoC'de sertifikadan türemez (PKI yok); değer facade'ın |
| `Flow` · `StateFlow` | Kotlin tarafı (Hasan) | UniFFI akış vermez; callback → `StateFlow` facade'da (ui-api §2) |

### 4.2 Kayıt (Enrollment) — K6: provisioning içe aktar → `Ready`

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `EnrollmentApi` | Kotlin tarafı (Hasan) | İçe aktarma akışı facade'da; sonunda `KamaMobile.start` |
| `EnrollmentState` | Kotlin tarafı (Hasan) | PoC'de kullanılan değerler `Initializing`, `AwaitingInvite`, `Ready` (+ içe aktarma hatası) |
| `enrollment` | Kotlin tarafı (Hasan) | Durum akışı facade'da |
| `state` | Kotlin tarafı (Hasan) | `EnrollmentApi.state`; tek kanal |
| `Initializing` | Kotlin tarafı (Hasan) | İlk değer; kayıtlı cihazda `KamaMobile.start` dönene dek |
| `AwaitingInvite` | Kotlin tarafı (Hasan) | Provisioning dosyası henüz içe aktarılmamış |
| `submitInvite` · `String` | Kotlin tarafı (Hasan) | İçe aktarma girdisi Kotlin'de; Rust yalnız dosya yolunu alır (`KamaMobile.start`) |
| `InviteUnreadable` | Kotlin tarafı (Hasan) | İçe aktarma hatası; dosya `start`'ta doğrulanır → `NodeException.Config` |
| `InviteExpired` | PoC'de etkisiz gövde (K6) | Provisioning dosyasında son kullanma yok; üretilmez |
| `AwaitingPhoneNumber` | PoC'de etkisiz gövde (K6) | Telefon/SMS adımı PoC dışı (PoC-KAPSAM-001 §3); üretilmez |
| `submitPhoneNumber` · `E164` | PoC'de etkisiz gövde (K6) | Çağrı etkisiz |
| `AwaitingSmsCode` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `submitSmsCode` | PoC'de etkisiz gövde (K6) | Çağrı etkisiz |
| `resendSmsCode` | PoC'de etkisiz gövde (K6) | Çağrı etkisiz |
| `wrongAttempts` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `lockedForSec` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `Checking` | PoC'de etkisiz gövde (K6) | RA kapıları PoC dışı; üretilmez |
| `Offline` (EnrollmentState) | PoC'de etkisiz gövde (K6) | RA yok; üretilmez |
| `retry` | PoC'de etkisiz gövde (K6) | Çağrı etkisiz |
| `Rejected` · `EnrollmentState.Rejected` | PoC'de etkisiz gövde (K6) | Allowlist/seat kapısı yok; üretilmez |
| `AwaitingAdminApproval` | PoC'de etkisiz gövde (K6) | Gate-3 yok; üretilmez |
| `waitingSinceEpochSec` · `since` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `nudgeAdmin` | PoC'de etkisiz gövde (K6) | Çağrı etkisiz |
| `Preparing` | PoC'de etkisiz gövde (K6) | K6: içe aktarmadan doğrudan `Ready`; üretilmez |
| `Ready` | Rust: `KamaMobile.start` | `start` başarılı döndü (K6) |
| `TrustValidationFailed` · `EnrollmentState.TrustValidationFailed` | PoC'de etkisiz gövde (K6) | Pin/zincir doğrulaması yok (ccr §35.4 kapalı-sistem kipi); üretilmez |
| `code` | Kotlin tarafı (Hasan) | Tanılama kodu kümesi facade'ındır; Rust hataları tiplidir (`NodeException`), metin kodu yok |
| `restart` | PoC'de etkisiz gövde (K6) | Terminal durumlar üretilmez; yeniden kurulum = yeniden provisioning (D4) |

### 4.3 Bağlantı

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `ConnectionApi` | Kotlin tarafı (Hasan) | Bileşik durum facade'da |
| `ConnectionState` | Kotlin tarafı (Hasan) | Bileşik (bağlantı ∧ boş kuyruk, K-4); girdiler `EngineEvent.Connected`/`Closed`/`ConnectProgress`, `Receipt`, `NodeMetrics.connectState` |
| `Ok` · `ConnectionState.Ok` | Kotlin tarafı (Hasan) | `Connected` ve `Receipt` bekleyen gönderim yok (INV-10) |
| `Disconnected` | Kotlin tarafı (Hasan) | Hazır oturum yok (`Closed` ya da `connectState` ≠ `Connected`) |
| `Recovering` | Kotlin tarafı (Hasan) | Bağlı ve `Receipt` bekleyen gönderim var (P-39); bant kuralı UI'da (P-41) |
| `TrustFailure` · `ConnectionState.TrustFailure` | PoC'de etkisiz gövde (K6) | K6 "TrustFailure çıkışı etkisiz"; zincir/iptal doğrulaması yok; üretilmez |
| `connection.retryAfterTrustFailure` · `retryAfterTrustFailure` | PoC'de etkisiz gövde (K6) | Plan K6 listesi |
| `ConnectionApi.diagnostics` · `diagnostics` · `Diagnostics` | Kotlin tarafı (Hasan) | E-11; girdiler olaylar ve `KamaMobile.metrics` |
| `lastConnectedAtEpochMs` | Kotlin tarafı (Hasan) | Son `Connected` olayının anı (olaylar zaman damgası taşımaz) |
| `pendingMessageCount` | Kotlin tarafı (Hasan) | `Receipt` bekleyen `msgId` sayısı (facade'ın gönderim kaydı) |
| `diagnosticCode` · `Diagnostics.diagnosticCode` | Kotlin tarafı (Hasan) | Kod kümesi facade'ın |

### 4.4 Mesajlaşma ve konuşmalar

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `MessagingApi` | Kotlin tarafı (Hasan) | Gönderim `KamaMobile.sendText` |
| `sendText` | Rust: `KamaMobile.sendText` | `Connected` sonrası çağrılır (DUR-K4G-11b); oturumsuz → `NodeException.Messenger(NoSession)`. INV-5 (başarısız olamaz, yeniden deneme yok) facade'ın: `Connected`'a dek Kotlin'de bekletme; metin ≤ 1002 B (D14) ön denetimi. Kabul edilen metin kalıcı outbox + ARQ ile yeniden başlatmada da aynı `msgId`'yle gider |
| `MessageId` | Rust: `KamaMobile.sendText` | Dönüş `msgId` (UInt, eş başına kalıcı); facade kimliği `(peerId, msgId)`'den kurar |
| `MessageStatus` | Kotlin tarafı (Hasan) | PoC'de iki değer üretilir: `PENDING` → `DELIVERED` |
| `PENDING` | Rust: `KamaMobile.sendText` | `Ok` = kuyruğa alındı (teslim değil) |
| `AT_SERVER` | PoC'de etkisiz gövde (K6) | Üretilmez (PoC-KAPSAM-001 §2 Mesaj) |
| `DELIVERED` | Rust: `EngineEvent.Receipt` | `(peerId, msgId)` başına tam bir kez |
| `messages` | Kotlin tarafı (Hasan) | Geçmiş bellek içi (D8, EK-1); girdiler `sendText` ve `EngineEvent.TextReceived` |
| `Message` | Kotlin tarafı (Hasan) | Zaman çizelgesi tipi |
| `UserMessage` | Kotlin tarafı (Hasan) | Gelen: `EngineEvent.TextReceived(peerId, msgId, text)` |
| `fromSelf` | Kotlin tarafı (Hasan) | Gönderen tarafın bilgisi facade'da |
| `status` | Kotlin tarafı (Hasan) | `MessageStatus`; `Outbound(status)` |
| `timestampEpochMs` | Kotlin tarafı (Hasan) | Olaylar zaman damgası taşımaz; facade gönderim/geliş anını yazar |
| `SystemMessage` | PoC'de etkisiz gövde (K6) | v0.3'te üretilmez (E-12 kapsam dışı) |
| `markRead` · `id` | Kotlin tarafı (Hasan) | Yerel (INV-9); okundu bilgisi yok |
| `ConversationsApi` | Kotlin tarafı (Hasan) | Konuşma = provisioning eşi |
| `ConversationsApi.list` · `list` | Kotlin tarafı (Hasan) | Liste facade'da, son mesaj zamanına göre |
| `ConversationSummary` | Kotlin tarafı (Hasan) | Facade'da |
| `ConversationId` | Kotlin tarafı (Hasan) | `peerId`'den türetilir |
| `RowIndicator` | Kotlin tarafı (Hasan) | INV-8 önceliği facade'da |
| `Unread` · `count` | Kotlin tarafı (Hasan) | `TextReceived` sayımı |
| `Outbound` | Kotlin tarafı (Hasan) | Son giden mesajın `MessageStatus`'u |
| `None` | Kotlin tarafı (Hasan) | `RowIndicator.None` ve `MessageAlerts.None` |

### 4.5 Ekler

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `AttachmentsApi` | Kotlin tarafı (Hasan) | Gönderim `KamaMobile.sendFile` |
| `send` | Rust: `KamaMobile.sendFile` | URI içeriği önce uygulamaya özel bir dosyaya kopyalanır (v0.3: "kaynak `send()` içinde kopyalanır"); Rust yalnız yerel yol okur. Görsel yeniden kodlama Kotlin'de (PoC-KAPSAM-001 §2 Fotoğraf) |
| `AttachmentSource.File` | Kotlin tarafı (Hasan) | URI → kopya dosya → `sendFile` |
| `AttachmentSource.File.uri` · `uri` | Kotlin tarafı (Hasan) | `content://` Rust'a geçmez |
| `AttachmentSource.Voice` | Kotlin tarafı (Hasan) | Kayıt dosyası → `sendFile` |
| `durationSec` | Kotlin tarafı (Hasan) | Rust süre bilmez |
| `FILE` · `IMAGE` | Kotlin tarafı (Hasan) | Rust tür ayrımı yapmaz (dosya opak bayt) |
| `Accepted` | Rust: `KamaMobile.sendFile` | `Ok(FileHandle)`; gelişme `FileProgress`, teslim `FileProgress(DELIVERED)` |
| `TooLarge` | Kotlin tarafı (Hasan) | INV-5: mesaj oluşmadan facade denetler. Rust tavanı 64 MiB (`MAX_REASSEMBLY_BYTES`); aşımda `EngineEvent.FileFailed(NotQueued(TooLarge))` |
| `VoiceTooLong` · `maxSec` | Kotlin tarafı (Hasan) | Süre sınırı Kotlin'de |
| `AttachmentLimits` | Kotlin tarafı (Hasan) | Rust API'si sınır açmaz (yukarıdaki tavan sabittir) |
| `limits.maxBytes` · `maxBytes` | Kotlin tarafı (Hasan) | ≤ 64 MiB seçilmeli |
| `maxVoiceDurationSec` | Kotlin tarafı (Hasan) | Facade'ın değeri |
| `sizeBytes` | Kotlin tarafı (Hasan) | Facade ölçer |
| `AttachmentRef` | Kotlin tarafı (Hasan) | Gelen: `EngineEvent.FileReceived(peerId, transferId, path, size)` |
| `DownloadState.NotDownloaded` · `NotDownloaded` | PoC'de etkisiz gövde (K6) | Gelen dosya tamamlanmış olarak gelir (D8: diske yazılır, yol olayla); indirme adımı yok |
| `Downloading` · `p` | PoC'de etkisiz gövde (K6) | Gelen aktarımın ilerlemesi olaylanmaz; üretilmez |
| `Downloaded` | Rust: `EngineEvent.FileReceived` | Dosya `<dataDir>/files/<eş>-<transferId>` |
| `Failed` | Rust: `EngineEvent.FileFailed` | Gelen aktarım: `FileRef.Incoming(transferId)` |
| `download` | PoC'de etkisiz gövde (K6) | Dosya zaten iner |
| `cancelDownload` | PoC'de etkisiz gövde (K6) | Alıcı tarafı iptal `KamaNode` API'sinde yok |
| `attachments.open` · `open` | Kotlin tarafı (Hasan) | K6: kullanılır. Gelen ekin yolu `EngineEvent.FileReceived.path` (Rust); URI (FileProvider) Kotlin'de; giden ek için Kotlin'in kopyası |

### 4.6 Kişiler — K6: Contacts = provisioning listesi

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `ContactsApi` | Kotlin tarafı (Hasan) | Liste provisioning'den |
| `matches` | Rust: `KamaMobile.metrics` | `NodeMetrics.connectState[].peerId`: provisioning'deki bütün eşler, ad sırasıyla |
| `ContactId` | Kotlin tarafı (Hasan) | `peerId`'den türetilir |
| `openWith` · `contactId` | Kotlin tarafı (Hasan) | Konuşma yerel; oturum `KamaMobile.connect(peerId)` ile istenir |
| `syncDirectory` · `ContactsApi.syncDirectory` | PoC'de etkisiz gövde (K6) | K6 |
| `syncState` | PoC'de etkisiz gövde (K6) | Senkron yok; değer facade'ın sabiti |
| `SYNCING` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `SYNCED` | PoC'de etkisiz gövde (K6) | Senkron yok |
| `NEVER` | PoC'de etkisiz gövde (K6) | Senkron yok |
| `syncState.STALE` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `LocalDirectoryEntry` | PoC'de etkisiz gövde (K6) | `syncDirectory` girdisi |
| `LocalDirectoryEntry.phoneE164` | PoC'de etkisiz gövde (K6) | `syncDirectory` girdisi |
| `ref` | PoC'de etkisiz gövde (K6) | `syncDirectory` girdisi |
| `lookupUsername` | PoC'de etkisiz gövde (K6) | K6 |
| `Found` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `NotDiscoverable` · `UsernameLookupResult.NotDiscoverable` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `Unavailable` | PoC'de etkisiz gövde (K6) | `lookupUsername` ve `DevicesApi.refresh` etkisiz; üretilmez |
| `PeerIdentity` | Kotlin tarafı (Hasan) | Eş = provisioning eşi |
| `username` (PeerIdentity) | Rust: `EngineEvent` / `NodeMetrics` | Eşin `peerId`'si (provisioning adı) |
| `localRef` | Kotlin tarafı (Hasan) | Rehber eşleşmesi yok |
| `verification` · `ContactsApi.verification` | Kotlin tarafı (Hasan) | Kapalı-sistem kipi: dizindeki her eş önceden doğrulanmış (ccr §35.4); Rust'ta ayrı sinyal yok |
| `VerificationState` | Kotlin tarafı (Hasan) | Facade'da |
| `Verified` | Kotlin tarafı (Hasan) | Provisioning eşi için sabit |
| `Anomalous` | PoC'de etkisiz gövde (K6) | Üretilmez |
| `IllegalArgumentException` | Kotlin tarafı (Hasan) | Bilinmeyen kimlik; Rust karşılığı `NodeException.UnknownPeer` / `UnknownHandle` |

### 4.7 Ayarlar ve cihazlar — K6: Devices tek satır

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `SettingsApi` | Kotlin tarafı (Hasan) | Yerel ayarlar |
| `setNotificationContentVisible` · `notificationContentVisible` | Kotlin tarafı (Hasan) | Bildirim içeriği facade'da (`TextReceived.text`) |
| `setAutoDownloadMedia` · `autoDownloadMedia` | PoC'de etkisiz gövde (K6) | Gelen dosya her zaman bütün alınır ve diske yazılır (D8) |
| `DevicesApi` | Kotlin tarafı (Hasan) | K6: tek satır — bu cihaz |
| `isCurrent` | Kotlin tarafı (Hasan) | Tek satır, `true` |
| `refresh` | PoC'de etkisiz gövde (K6) | Cihaz yenileme yok |
| `DeviceInfo.renewal` | PoC'de etkisiz gövde (K6) | Yenileme yok |
| `DeviceRenewalState.Failed` | PoC'de etkisiz gövde (K6) | Üretilmez |

### 4.8 Bildirim, arka plan, platform sinyali, push

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `notifications` · `notifications.state` | Kotlin tarafı (Hasan) | K6: kullanılır; girdi `EngineEvent.TextReceived`; gösterim app'te |
| `HiddenUnread` · `MessageAlerts.HiddenUnread` | Kotlin tarafı (Hasan) | INV-13 facade'da |
| `Visible` · `MessageAlerts.Visible` | Kotlin tarafı (Hasan) | Gövde `TextReceived.text` |
| `deviceApprovedUnseen` · `notifications.state.deviceApprovedUnseen` | PoC'de etkisiz gövde (K6) | Gate-3 bildirimi; hep `false` |
| `background` · `background.drain` · `drain` | Rust: `KamaMobile.drainBackground` | K6 / EK-2 |
| `DrainResult` | Kotlin tarafı (Hasan) | `drainBackground` sonucu ve bağlantı durumundan |
| `Drained` | Rust: `KamaMobile.drainBackground` | `Ok` |
| `Blocked` | PoC'de etkisiz gövde (K6) | Güven hatası yok; üretilmez |
| `Offline` (DrainResult) | Kotlin tarafı (Hasan) | Boşaltmadan sonra hazır oturum yok |
| `onPlatformSignal` | Rust: `KamaMobile.onPlatformSignal` | K6: kullanılır |
| `LinkChanged` | Rust: `KamaMobile.onPlatformSignal` | `PlatformSignal.LINK_CHANGED`; D15: `SESSION_INIT_REF` hub sprintinde, bu sürümde protokol davranışı yok |
| `Foreground` | PoC'de etkisiz gövde (K6) | Rust'ta değer yok: heartbeat kadansı (8 s / 25 s) hub sprintinde (D18); `deviceApprovedUnseen` zaten `false` |
| `Background` (sinyal) | PoC'de etkisiz gövde (K6) | Rust'ta değer yok (D18) |
| `push` | PoC'de etkisiz gövde (K6) | K6 |
| `push.setPushEndpoint` · `e` | PoC'de etkisiz gövde (K6) | K6 |
| `PushEndpoint` | PoC'de etkisiz gövde (K6) | K6 |

### 4.9 Kotlin dosya grupları ve fake'e özel öğeler

| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `Ids` | Kotlin tarafı (Hasan) | Kimlik tipleri: §4.4, §4.6 |
| `Identity` | Kotlin tarafı (Hasan) | `PeerIdentity`, `SelfIdentity`: §4.1, §4.6 |
| `Enrollment` | Kotlin tarafı (Hasan) | §4.2 |
| `Messaging` | Kotlin tarafı (Hasan) | §4.4 |
| `Conversations` | Kotlin tarafı (Hasan) | §4.4 |
| `Attachments` | Kotlin tarafı (Hasan) | §4.5 |
| `Contacts` | Kotlin tarafı (Hasan) | §4.6 |
| `Settings` | Kotlin tarafı (Hasan) | §4.7 |
| `Devices` | Kotlin tarafı (Hasan) | §4.7 |
| `Notifications` | Kotlin tarafı (Hasan) | §4.8 |
| `Background` (dosya) | Kotlin tarafı (Hasan) | §4.8 |
| `FakeKamaEngine` | Kotlin tarafı (Hasan) | Yalnız fake; `KamaEngine`'de yok |
| `FakeKamaEngine.reset` · `reset` · `mode` | Kotlin tarafı (Hasan) | Yalnız fake |
| `FakeKamaEngine.contactsNetworkCalls` · `contactsNetworkCalls` · `FakeContacts.networkCalls` | Kotlin tarafı (Hasan) | Yalnız fake (test gözlemi) |
| `simulateIncoming` · `conversationId` · `body` | Kotlin tarafı (Hasan) | Yalnız fake; gerçekte `EngineEvent.TextReceived` |
| `simulateNetwork` · `available` | Kotlin tarafı (Hasan) | Yalnız fake |
| `simulateTrustFailure` | Kotlin tarafı (Hasan) | Yalnız fake |
| `Fresh` | Kotlin tarafı (Hasan) | Fake seed modu |
| `Enrolled` · `seedConversations` | Kotlin tarafı (Hasan) | Fake seed modu (K6'nın `Ready` eşdeğeri) |
| `refreshCount` | Kotlin tarafı (Hasan) | Yalnız fake |
| `Clock` · `CoroutineDispatcher` · `TestDispatcher` | Kotlin tarafı (Hasan) | Fake'in enjekte edilen saati |

## 5. Kapsama yöntemi (eksik satır = DONE değil)

1. **Çıkarım (programla).** v0.3 belgesinin (sha256 yukarıda) iki kaynağı taranır: satır içi `` `…` `` aralıkları ve
   ```` ``` ```` çitli kod blokları. Her kaynaktan Unicode-bilinçli tanımlayıcı düzeni
   `(?<![\w\-/.])([^\W\d]\w*(?:\.[^\W\d]\w*)*)` ile noktalı yollar dahil adaylar çıkarılır; yalnız ASCII adaylar
   tutulur (Türkçe düzyazı sözcükleri bütün olarak düşer). Sonuç: **352** aday.
2. **Sınıflama (açık liste).** Her aday tam bir sınıfa girer; sınıfsız aday kalırsa denetim KIRMIZI'dır. Öğe
   sayılmayanlar ve kategorileri: kaynak etiketleri ve ekran/kod kimlikleri (`TASARIM`, `KAPSAM`, `E-…`, `A1`…), belge /
   dosya / crate / paket adları, değişmez değerler ve fake girdileri (`"bad"`, `onur.k`, `true`, `null`…), kama-core /
   keşif iç adları (`dest_hash`, `VOPRF`, `send_message`…), Kotlin/Android platform adları (`applicationContext`,
   `when`, `internal`…), v0.3'te kaldırılmış adlar (`AwaitingFederationChoice`, `chooseIndividual`, `NotStarted`),
   v0.3'ün "YOK" dediği alan/metotlar (`muted`, `readAt`, `revoke`), Kotlin invariant test adları (`test_…`) ve
   ASCII Türkçe düzyazı sözcükleri (**143** ad). Kalan **209** öğe (çağrı, alan, tip, durum değeri, parametre,
   dosya grubu ve fake'e özel öğe) bu belgenin §4 tablolarında, ilk sütunda birebir yazımıyla geçer.
3. **Denetim (programla).** Denetleyici §4'ün her satırını ayrıştırır: (i) 209 öğenin her biri bir satırın ilk
   sütununda geçer; (ii) her satırın eşlemesi üç biçimden biridir; (iii) ilk sütundaki her ad v0.3 adayıdır (uydurma
   satır yok). Son koşu: 209/209 kapsandı, 0 eksik, 0 yabancı ad, §4'te 165 satır, §3'te 12 satır. Denetleyicinin
   kendisi üç bozmayla sınandı (satır silme, geçersiz eşleme, uydurma ad): üçü de KIRMIZI.
4. **K6 satırları** (§3) PoC-KAPSAM-001 K6 ve plan K4G-15 NE 2 metninden birebir alındı; `CallsApi` v0.3'te
   olmadığı için yalnız §3'tedir.

Denetleyici betik bu depoya girmedi: girdisi olan v0.3 belgesi kama-engine reposundadır. Yöntem, sınıf listesi ve
sayılar buradadır; v0.3 değişmediği sürece (sha256 sabit) sonuç yeniden üretilebilir.

## 6. Beyanlı sınırlar ve farklar

* **İki telefon arası oturum yok** (DUR-K4G-11 → B): bu `.so` API entegrasyonu ve süreç-içi loopback içindir;
  iki cihaz arası zarf taşıyıcısı hub Plane 1 rölesidir (hub sprinti). Paketteki örnek yalnız `device-alice.json`'dur.
* **`metrics()` FFI'da hata dönebilir:** `KamaNode::metrics` hatasızdır (düğüm `shutdown` ile tükenir); FFI nesnesi
  paylaşıldığı için kapanış sonrası çağrı `NodeException.Stopped` alır.
* **Dinleyicinin Rust imzası `Result` döner** (`ListenerError`): Kotlin'in attığı istisna Rust'ta panik yerine hataya
  çevrilip yutulur. Kotlin arayüzü yine `fun onEvent(ev: EngineEvent)`.
* **`onPlatformSignal`**: yalnız `LINK_CHANGED` Rust'a geçer; `Foreground` / `Background` için Rust değeri yok (D18).
* **Olaylar zaman damgası taşımaz**; zaman alanları facade'da yazılır.
* **`rttMs` / `lossPct` / `bitrateKbps` bu sürümde `null`**: salt-okunur kaynak hub sprintinde (KATMAN 6).
* **Kotlin `toString()`**: üretilen veri sınıfları metni, ses yükünü ve blob'u yazar (Rust `Debug` yazmaz); olay
  loglanmaz.
