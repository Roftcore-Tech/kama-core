# N-kama-node — davranış envanteri ve otorite karşılaştırması

Taban: `125b9afa87e731c1e666ecf63f33b7cc555dc9b1` (ürün hattı; aday `1a4f9efc` bunun içinde). Satır numaraları bu tabandandır. Salt okuma: kaynak dosyalara yazılmadı, cargo çalıştırılmadı, git durumu değiştirilmedi.

## Kapsam

| dosya | satır aralığı | okunan satır |
|---|---|---|
| `crates/kama-node/Cargo.toml` | 1–52 | 52 |
| `crates/kama-node/src/lib.rs` | 1–989 | 989 |
| `crates/kama-node/src/node.rs` | 1–1186 | 1 186 |
| `crates/kama-node/src/carrier.rs` | 1–712 | 712 |
| `crates/kama-node/src/a5.rs` | 1–313 | 313 |
| `crates/kama-node/src/error.rs` | 1–104 | 104 |
| `crates/kama-node/src/event.rs` | 1–404 | 404 |
| `crates/kama-node/src/files.rs` | 1–309 | 309 |
| `crates/kama-node/src/handle.rs` | 1–279 | 279 |
| `crates/kama-node/src/hub_mode.rs` | 1–333 | 333 |
| `crates/kama-node/src/link_profile.rs` | 1–387 | 387 |
| `crates/kama-node/src/peers.rs` | 1–92 | 92 |
| `crates/kama-node/src/pilot_record.rs` | 1–266 | 266 |
| `crates/kama-node/src/probe.rs` | 1–207 | 207 |
| `crates/kama-node/src/ptt_pilot.rs` | 1–173 | 173 |
| `crates/kama-node/src/test_support.rs` | 1–313 | 313 |
| `crates/kama-node/src/wiring.rs` | 1–198 | 198 |
| `crates/kama-node/src/node/tests.rs` | 1–1116 | 1 116 |
| `crates/kama-node/src/node/signaling_tests.rs` | 1–192 | 192 |
| `crates/kama-node/src/node/tests/glare.rs` | 1–325 | 325 |
| `crates/kama-node/src/node/tests/hub_relink.rs` | 1–245 | 245 |
| `crates/kama-node/src/node/tests/intent_t3.rs` | 1–247 | 247 |
| `crates/kama-node/src/node/tests/ptt_pilot.rs` | 1–323 | 323 |
| `crates/kama-node/tests/common/mod.rs` | 1–253 | 253 |
| `crates/kama-node/tests/api_paths.rs` | 1–299 | 299 |
| `crates/kama-node/tests/glare_simultaneous_connect.rs` | 1–124 | 124 |
| `crates/kama-node/tests/signaling_loopback.rs` | 1–243 | 243 |
| `crates/kama-node/tests/start_config.rs` | 1–228 | 228 |
| `crates/kama-node/tests/two_nodes_loopback.rs` | 1–317 | 317 |
| **toplam** | | **10 229** |

Kapsam dışından yalnız bağlam için okunanlar (bulgu bu dosyalara atanmadı): `client-core/src/runtime.rs` 320–345, 405–432, 680–735; `client-core/src/transport.rs` 315–345; `client-core/src/hub/endpoint.rs` 200–330; `client-core/src/hub/link.rs` 722–745, 870–877; `kama-app/src/ptt.rs` 262–270, 1186–1196.

## Davranış envanteri

### Cargo.toml (52)

- Bağımlılıklar: workspace crate'leri + `thiserror` + `ed25519-dalek` (beyanlı istisna "[S2-A3-2] BindSigner"). Yeni async runtime yok (düğüm döngüsü `std::thread`).
- `test-util` özelliği yalnız ölçüm kurucularını, sondayı ve client-core test kancalarını açar; ürün bağımlılığında açılmaz. dev-dependency kendi `test-util`'ini açar.
- Uyumlu: 2 (ADR-02 tokio client-core içinde; D11 bağımlılık disiplini beyanlı istisnayla). dalek kullanımı → N-07.

### lib.rs (989)

- **Crate belgesi 1–67:** kurulum sırası, API listesi, tutamaç ve "KAPSAM DIŞI" bölümü. 64–67 "Hub, heartbeat (`last_heartbeat_age_s` daima `None`), `BestEffort`/`DeadlineBounded` yok" der — kod hub kipi kuruyor ve PTT demeti DeadlineBounded gidiyor (N-06).
- **Sabitler:** `STORE_FILE_NAME = "kama-node.redb"`; `NODE_TICK = 20 ms` ([KALİBRASYON], ACK_TICK'le "aynı mertebe" denir, sabite bağlı değil); `METRICS_TICK = 1 s` ([KALİBRASYON]); `NODE_THREAD_NAME`; `ACK_FORMAT_ENV_VAR = "KAMA_ACK_FORMAT"`.
- **`thread_local` test anahtarları 142–151:** `PTT_COALESCE_FOR_TEST`, `PTT_PILOT_FOR_TEST` (yalnız `cfg(test)`).
- **`NodeListener` / `PlatformSignal::LinkChanged`.**
- **`InputLimits::from_amp` 181–189:** `max_text = MAX_BODY_BYTES − TEXT_PREFIX_BYTES` (ürün 1002 B), `max_bundle = max_bundle_payload_bytes`, `max_blob = MAX_BODY_BYTES − 2` (ürün 1006 B). Türetilmiş.
- **`parse_ack_format` 223–227:** `""`/`v2` → V2, `bitmap` → Bitmap, diğer → `Config(Invalid)`.
- **`core_build_line` 231–237:** paket sürümü + derleme anı `KAMA_GIT_SHA` (yoksa "bilinmiyor").
- **`start` 254–261:** Application saati `SystemClock`, `build` → `spawn`.
- **`build_with_link` 282–494 (kurulumun tek gövdesi):**
  1. `load_device_config` → `PeerBook`; `link == None` ise `hub_mode::params` (hub kipi seçimi).
  2. Veri bağlama adresi: hub kipinde `127.0.0.1:0`, aksi halde `direct_addrs[me]` yoksa `NoDirectAddress`.
  3. `files::create_private_dir`, `ReceivedFiles::open`, `RedbStore::open` (tek `Arc`: `AppStore` + `CounterStore`).
  4. `wiring::protection` (gerçek anahtar, Crypto saati, OS entropisi, kalıcı `init_counter`), hub girdileri, `drop(config)`.
  5. Taşıyıcı: dış link → doğrudan kripto; hub kipi → `hub_mode::start`; aksi → süreç-içi röle.
  6. `ClientTransportConfig::product`; ortam anahtarları: `KAMA_TRANSPORT_ENGINE` (329–340; tanınmayan → hata; **boş değer → Legacy**, N-05), `KAMA_ACK_FORMAT` (346–351), `KAMA_LINK_RATES`/`link-profile` (354–364), `KAMA_PTT_PILOT`/`ptt-pilot` (375–388; yönetilen ad dizinde değilse hata; pilot açıkken `ack_format` açıkça `v2` ise hata, değilse `Bitmap` + `ptt_coalesce = true` — motor denetlenmez, N-09), `KAMA_PILOT_RECORD`/`pilot-record` (416–424; açıkken `live_diag = true`).
  7. Başlangıç kaydı metni (`transport-start.txt`, 0600), Transport kurulur, kayıt yazılır, hub tarafına veri adresi bildirilir.
  8. `Messenger::with_store`, `InputLimits`, `ptt_engine` (birleşik anahtar Transport bayrağından türer), `provision_ptt_pilot`, `FileTransfer`, `Signaling` (ortak Application saati), `Shared`, `PilotRecorder`, `NodeLoop`.
- **`spawn` 497–517:** `sync_channel(COMMAND_QUEUE_CAP)`, adlı iş parçacığı.
- **API 528–784:** her çağrı eş adını `PeerBook::resolve` ile çözer (`UnknownPeer`); `send_text`/`ptt_push`/`ptt_push_timed`/`send_signaling` yük boyunu kuyruktan ÖNCE sınar (`TooLarge`); `send_file` tutamacı çağıranda verir ve yanıtsız komut koyar; `ptt_open` her zaman uyumlu yol (`pilot: false`), `ptt_open_press` pilot kuralı; `ptt_push_timed` `audio_start = now − audio_age` (taşmada `now`); `on_platform_signal` yanıtsız; `drain_background` yanıtlı; `metrics` paylaşılan anlık görüntüyü kopyalar.
- **test-util kurucuları 798–900:** canlı ayna açık, recovery-OFF, dış link + birim izi; `probe_for_test` röle zarf sayaçlarını ekler.
- **`request`/`enqueue`/`stop`/`Drop` 924–970:** düğüm iş parçacığından yanıtlı çağrı `ReentrantCall`; kuyruk dolu `Busy`; kapanış kuyruk sonuna `Shutdown`, sığmazsa `stopping` bayrağı; Drop panik etmez.
- **Test 973–989:** `KAMA_ACK_FORMAT` çözümünü sabitler (bitmap yalnız açıkça; tanınmayan değer hata).
- Uyumlu: 14 — öne çıkanlar: D14 TEXT ≤ 1002 B; AMP 0x09 blob tavanı; D8/D9 (kimlik bellekte, `root.json` okunmaz); D-CC-1 geri alma anahtarı `legacy`; karar 9 "V2 genel varsayılan" (pilot ve bitmap yalnız açık ayarla); ADR-15 kaba taneli API; SR-AUDIT-001 girdi sınırı kuyruktan önce; reachable `todo!()` yok.

### node.rs (1186)

- **Sabitler:** `COMMAND_QUEUE_CAP = 64` ([KALİBRASYON]); `MAX_EVENTS_PER_TURN = MAX_INBOX` (türetilmiş); `HUB_NOTICES_PER_TURN = 16` (gerekçesiz, etiketsiz); `REPLY_CAPACITY = 1`.
- **`Command` 116–185**, **`Shared` 189–209** (metrics mutex, tek `HandleAllocator`, `stopping`).
- **`ptt_engine` 215–222:** `Ptt::set_spurt_coalesce(transport.ptt_coalesce_enabled())`. **`provision_ptt_pilot` 230–241:** tablo dolu → `Config`.
- **`NodeLoop::new` 300–337:** `RelinkGovernor::new(relink_seed())`, `A5::default()`.
- **Pilot özeti 340–407:** canlı ayna toplamı (açık + emekli oturumlar); her satır katmanını yazar; "sayaç yok" satırları; kimlik/yük yok.
- **`run_loop` 421–455:** en çok `COMMAND_QUEUE_CAP` komut → `stopping` denetimi → `turn` → `drive_hub` → `publish_metrics` → `pilot_tick` → bekleme (iş kaldıysa 0, yoksa `NODE_TICK`).
- **`execute` 459–501:** komut → motor çağrısı; yanıt işlendiği anda.
- **`connect` 505–515:** `Pending` biletini kaydeder, `Ready` ise `note_up`.
- **`drain_background` 525–539:** `ConnectState::Idle` eşlere `connect`; ilk hata döner.
- **`on_platform_signal` 543–547:** `LinkChanged` → hiçbir şey (D15) — hub kipinde de (N-02).
- **Dosya 551–584:** oturumsuz eşe dosya okunmaz (`NotQueued(NoSession)`), okuma → `FileTransfer::send_file`, tutamaç iki yönlü eşlenir; `cancel_file` → `cancel_send` + olay boşaltma (kayıt aynı komutta kapanır).
- **PTT 586–671:** tutamaç önce verilir; `feed_ptt_capability` açılıştan önce; pilot → `open_press`, aksi → `open_compatible`; `ptt_push` üç kip (`send_bundle`, `send_bundle_from`, `send_last_bundle_from`); `ptt_close` kaydı motor `close`'undan ÖNCE siler.
- **`feed_ptt_capability` 644–662:** yalnız birleşik anahtar açıkken; (eş, oturum, değer) başına bir kez motora.
- **`turn` 677–698:** ≤ `MAX_EVENTS_PER_TURN` olay `Ptt::poll` → zincir; kuyruk boşaldıysa `files.pump` + `signaling.pump`; dosya olayları.
- **Olay zinciri 708–797:** PTT → dosya → sinyalleşme → Messenger olayı; `Ready` → `note_up`, `Closed` → `note_down`, `ConnectProgress` biletle eşlenir, `ConnectAbandoned` → `Closed{Abandoned}`, `RenewRequired` yok sayılır.
- **`note_up` 800–814:** Messenger oturumu yoksa çık; eşin biletlerini sil; hub kipinde `hub.peer_ready(peer)` (oturum tutamacı kullanılmaz; N-01); ilk geçişte `Connected`.
- **`drive_hub`/`on_hub_notices` 831–862:** ≤ 16 bildirim; `RelinkGovernor::on_notice(has_session, active_link)` → `RestartIntent` ise `restart_attempt`; `drive` → `restart_renewal`/`relink`/`relink_line`.
- **`restart_attempt` 864–874:** bileti sil → `cancel_connect` → `connect`; `connect` hatası yok sayılır (N-10).
- **`note_down` 879–893:** oturumu kalmayan bağlı eşlere `Closed{Session(reason)}`; supersession olay üretmez.
- **Dosya olayları 895–983:** gelen dosya diske `<eş>-<transfer_id>`; disk hatası `FileFailed{Storage}` (eşe `FILE_RECEIPT(ok)` zaten gitmiştir; N-13); `Delivered`/`SendFailed` tutamacı kapatır.
- **`snapshot` 998–1044:** eş başına durum + epoch; `rtt_ms`/`loss_pct` canlı aynadan (`a5`); `bitrate_kbps: None` sabit (N-03); `epoch` = hazır oturumların en küçüğü; `last_heartbeat_age_s` hub kipinde `HubSide::last_rx_age_s`.
- **`emit_for` 1048–1052:** ad yoksa olay düşer. test-util sonda ve kancalar 1056–1150.
- Uyumlu: 16 — öne çıkanlar: DR-CONNECT-1 (connect = kalıcı niyet, idempotent; `cancel_connect` → `Abandoned(Cancelled)`); PoC K6 / EK-2 `background.drain()` (mailbox çekmez); K4G-09 zamanlayıcıları kuyruk boşaldıktan sonra; AMP §3 0x09 yeniden sunum pompası; CEVAP PAKETİ 2 madde 1 (tutamaç kaydı motor tablosuyla aynı anda kapanır); S4 C5 + HLR-STALE (eski hattın geç `ROUTE_MISS`'i çalışan oturumu yenilemez); S4 C9 heartbeat yaşı = son datagram yaşı; INV-NO-IDLE-TEARDOWN (zamanla kapatma yok).

### carrier.rs (712)

- **Süreç-içi zarf rölesi:** süreç-genel `static RELAY` (kimlik → kutu + jeton); aynı kimliğin ikinci kaydı `IdentityInUse`; jetonla sahiplik (geç Drop yeni kaydı silmez).
- **`route_init` 192–235:** kendine INIT, kaydı düşmüş uç, kayıtsız eş, dolu kutu (iki kutu da önceden), tükenmiş deneme/tutamaç → `Err(LinkError)`, yan etki yok; başarıda `HandleAssigned` + aynı tutamaçla `IncomingInit`. `route_response` yalnız hedefin kutusuna.
- Sabitler: `MAILBOX_CAP = 1024` ([KALİBRASYON], SR-AUDIT-001), `FIRST_HUB_HANDLE = 1` (0 verilmez, sarma yok), `FIRST_ATTEMPT = 1`.
- Kayıtsız eşe zarf saklanmaz (store-and-forward yok), T3 yok; geç başlayan eşte glare takılması belgeli.
- Testler 355–712: kayıtsız eşe ret ve iz yok; ikinci kayıt reddi; geç Drop; iki uç aynı tutamaç, INIT başına yeni tutamaç; RESPONSE yalnız hedefe; kutu tavanında hiçbir kutuya bir şey girmez; baytların opaklığı; kendine INIT; tutamaç ve deneme uzayının tükenmesinde yan etkisizlik.
- Uyumlu: 8 — DUR-K4G-11 → B (ürün taşıyıcısı değildir); PoC EK-2 store-and-forward yok; PoC §5 "INIT yeniden gönderim kadansı yok" beyanlı açık; S3 K6 opaklık; KR-2 tek handle iki uçta; SR-AUDIT-001 sınırlı kutu.

### a5.rs (313)

- `LOSS_WINDOW = 10 s` ("A5 kartı"), `SAMPLE_PERIOD = 1 s`, `MAX_SAMPLES = 12` (yorumda `10/1 + 2`, değer literal).
- **`LossWindow::observe` 38–68:** toplamlar geri giderse pencere sıfırlanır; saniyede bir örnek; 10 s'den eski taban düşer; `Δretransmits / Δtx_datagrams × 100`, gönderim yoksa `None`.
- **`rtt_ms` 83–92:** hazır + aynalı + `srtt_us > 0` oturumların en küçüğü, ms (aşağı yuvarlama).
- **`A5::loss_pct` 96–116:** ayna yoksa `None` (pencere ilerlemez); açık oturumlar + emekli toplam.
- Testler: 10 s pencere, azalan toplam, örnek sınırı, en küçük RTT, ayna şartı; uçtan uca ayna açık düğümde `Some`, ürün kurucusunda `None`.
- Uyumlu: 3 (uydurma değer yok; sınırlı örnek; doğrudan kipte heartbeat `None`). Anlam → N-04; ürün yolunda boşluk → N-03.

### error.rs (104)

- `NodeError` (thiserror): kurulum (`Config`, `PeerDirectory`, `NoDirectAddress`, `DataDir`, `Store`, `Transport`, `Limits`, `Spawn`, `IdentityInUse`) ve çağrı (`UnknownPeer`, `UnknownHandle`, `HandleClosed`, `HandlesExhausted`, `TooLarge`, `Busy`, `Stopped`, `ReentrantCall`, motor hataları, `NodeThreadPanicked`). Gizli değer taşımaz.
- Ortam/dosya dağıtım ayarlarının hataları da `Config(FormatError::Invalid)`'e konuyor; varyant belgesi "provisioning dosyası" der (N-08).
- Uyumlu: 2 (CLAUDE.md §9 tipli hata; gizli değer yok).

### event.rs (404)

- `NodeEvent` (13 varyant), `CloseCause`, `FileStage`, `FileRef`, `FileFailReason`, `NodeMetrics`, `PeerConnectState`, `LinkState`.
- Elle yazılmış `Debug`: metin, ses yükü, blob baytı yerine uzunluk.
- Alan belgeleri bayat: `rtt_ms`/`loss_pct` "transport dışarı açmıyor", `last_heartbeat_age_s` "PoC'de daima None", `PttBundle` "PTT-R0 Reliable; geç gelen düşmez" (N-06).
- Testler: Debug'ın metin/ses/blob baytı basmaması.
- Uyumlu: 4 (PoC §2 iki mesaj durumu: Sent / Delivered; `FileReceived` yolu düğümün ürettiği ad; Debug gizliliği; `deadline_expired_drops` yok — DUR-K4G-10).

### files.rs (309)

- `RECEIVED_FILES_DIR = "files"`, `PART_SUFFIX`, `NAME_ATTEMPTS = 16` ([KALİBRASYON]), `0700`/`0600`.
- **`ReceivedFiles::store` 84–116:** ad `<eş>-<transfer_id>` (çakışmada `-<n>`), var olan dosyanın üstüne yazmaz, `.part` + `sync_all` + `rename`; tükenince `AlreadyExists`.
- **`write_private` 120–138:** kes + yaz + eşitle, kalıntı dosyaya da `0600`.
- **`read_outgoing` 145–174:** ad UTF-8 değilse `SourceName`; düzenli dosya değilse `InvalidInput`; boy okumadan önce `MAX_REASSEMBLY_BYTES` ile; okuma `take(MAX + 1)`.
- Testler: adın telden türememesi ve üstüne yazmama; ad tükenmesi; izinler; sınırlı okuma (seyrek dosya); UTF-8 ad.
- Uyumlu: 5 (PoC EK-1 "alınan dosya diske yazılır, yolu olayla verilir"; yol enjeksiyonu yok; SR-AUDIT-001 bounds-before-alloc; AMP S7 64 MiB; at-rest şifreleme yok = PoC kabul edilmiş risk).

### handle.rs (279)

- `StreamHandle`/`FileHandle` (u64), tek `HandleAllocator` (0 verilmez, `checked_add`, sarma yok, `HandlesExhausted`), `missing` → `HandleClosed`/`UnknownHandle`, `Registry`. `PttPress { stream, bundle_ms, coalesced }`.
- Testler: kesin artan, eşzamanlı tekillik, tükenme, kapanmış tutamacın başka eşin akışına çözülmemesi, hiç verilmemiş değer, ham değer gidiş-dönüşü.
- Uyumlu: 3 (CEVAP PAKETİ 2 madde 1; sarma yok; ADR-15 FFI ham değer).

### hub_mode.rs (333)

- **`params` 51–67:** `hub` yoksa `None`; `hub.addr` bir kez çözülür, ilk IPv4; çözülemezse `Config("hub.addr unresolvable")`.
- **`inputs` 77–88:** `HubConfig` (adres, hub Ed25519, `device_cert_ref`, `self_dest_hash`) + imzacı, `drop(config)`'tan önce.
- **`start` 105–134:** `HubLink::new` (hata → `Transport(Io(other))`), `HubCrypto::hub`, `HubEndpoint::start(product)`, `RecordingLink(HubEnvelopeLink)`.
- `DATA_BIND = 127.0.0.1:0`. **`HubSide::peer_ready` 162–166:** `known.handle_of(peer)` (eşe EN SON bağlanan handle) ile `HubLink::peer_ready` — belge "T3 yedeği `on_session_traffic`" der (N-01, N-06). `take_notices(cap)`, `last_rx_age_s`, `counters` (test-util).
- Testler: hub alanına göre kip; çözülemeyen adres; dış link hub kipine girmez; hub kipinde doğrudan adres gerekmez (test `Ok` ya da `Err(Transport)` iki sonucu da kabul ediyor; N-15).
- Uyumlu: 6 (KK-H50 seçim, KK-H51/H63 tek soket ve IPv4, KK-H55 tipli hata, KK-H56 imzacı dosya düşmeden, S4 C1 tek soket, C9).

### link_profile.rs (387)

- `ENV_VAR = KAMA_LINK_RATES`, `PROFILE_FILE = link-profile`, `START_RECORD_FILE = transport-start.txt`, `MAX_PROFILE_BYTES = 64` (türetilmiş: `<u64>,<u64>` ≤ 41 B + pay).
- **`parse` 74–82:** `<yukarı>,<aşağı>`, iki pozitif tamsayı. **`load` 85–115:** ortam > dosya > yok; uzunluk ayrıştırmadan önce; dosyadan `take(64 + 1)`.
- **`local_uplink_narrow` 120–132:** aynı `link_feedback_starved` kuralını ters çevrilmiş hızlara uygular.
- **`start_record` 135–187:** motor, ACK biçimi (bitmap'te yöne göre etkinlik), şeritler, yön hızları, kaynak, koruma kipi (PTT kopya sayıları `PTT_FORWARD_COPIES`'tan).
- Testler: ayrıştırma, kaynak sırası, kayıt metni (yönler, motor, şeritsiz), girdi sınırı, `build` uçtan uca.
- Uyumlu: 3 (CLAUDE.md YAMA KAYDI 2026-11-16 — profil yalnız `link_rates` verilince, profil yoksa bugünkü davranış; SR-AUDIT-001; geçersiz değer sessizce profilsiz başlamaz). Ayar yüzeyi → N-08.

### peers.rs (92)

- Provisioning'den bellekte ad ↔ `PeerRef` ↔ direct-mode adres; çalışma zamanında büyümez (≤ 63 eş).
- Uyumlu: 2 (D8; gizli değer yok).

### pilot_record.rs (266)

- `KAMA_PILOT_RECORD` (`1`/`0`, başka değer hata) ya da `pilot-record` işaret dosyası; `SUMMARY_PERIOD = 5 s`; kapasite-1 kanal, meşgul yazıcıda özet atlanır ve sayılır; kapanışta son özet beklenerek; geçici dosya + `rename`; Drop panik etmez.
- Testler: anahtarın katılığı, hız sınırı ve son özet, `build` uçtan uca, meşgul yazıcıda atlama.
- Uyumlu: 3 (bounded kuyruk; yük/kimlik yazılmaz; ölçüm "sayaç yok" diye uydurulmaz). Ayar yüzeyi → N-08.

### probe.rs (207, yalnız test-util)

- `NodeProbe`, `PeerSessionProbe`, `SessionProbe`, `SessionEventCounts`, `EnvelopeCounts`; `EnvelopeLog::record` son `RECENT_ENVELOPES = 16` zarfın baytını tutar ve bayt-özdeş tekrarı sayar.
- Test: tür ayrımı, bayt-özdeş tekrar, kayıt sınırı.
- Uyumlu: 2 (salt okuma; ürün yolu derlemez).

### ptt_pilot.rs (173)

- `KAMA_PTT_PILOT` / `ptt-pilot`; biçim `0` · `1` · `1:<eş>[,<eş>…]` (boş ya da yinelenen ad geçersiz); `MAX_PILOT_BYTES = 4096` (63 ad × 64 B, türetilmiş); `start_line`.
- Testler: üç biçim ve retler; kaynak sırası ve sınır.
- Uyumlu: 3 (karar 9: pilot açıkça seçilir, V2 genel varsayılan; dizin dışı ad sessiz geçmez; SR-AUDIT-001).

### test_support.rs (313, yalnız `cfg(test)`)

- `TempDir`, `serial` (port yarışı kilidi), gerçek provisioning (`provision`), `ManualClock`, `Events` (Metrics saklanmaz, `WAIT = 60 s` üst sınır), `manual`/`try_manual`/`manual_with_link`/`threaded`/`until`.
- Uyumlu: 2 (gerçek kripto + depo + UDP; her bekleme üst sınır).

### wiring.rs (198)

- `Inner = NoiseWireProtection`, `Crypto = HubCrypto<Inner>`, `Transport = ClientTransport<Crypto>`; `ENVELOPE = Legacy` (22 B, ccr §35.6).
- **`protection` 65–80:** dizin, `SystemClock`, `OsEntropy`, `CounterStore`.
- **`DeviceBindSigner` 99–120:** cihaz Ed25519 anahtarıyla `ed25519_dalek::SigningKey::sign` (N-07); Debug gizli.
- `device_cert_ref = SHA-256(cert)`, `hub_entropy = OsEntropy`, **`relink_seed` 136–140:** OS CSPRNG, hata → sıfır tohum (N-12).
- Test: imza cihaz anahtarıyla katı doğrulanır, kimlik anahtarıyla doğrulanmaz, Debug gizli, `cert_ref`.
- Uyumlu: 4 (layer_check C8 — `kama_noise_wire` adı yalnız burada; S4 KR-5 cihaz anahtarı; PoC §2 22 B zarf; gizli anahtar Drop'ta sıfırlanır).

### node/tests.rs (1116) — sabitlenen davranışlar

- Kapanmış dosya tutamacıyla iptal, aynı tel `transfer_id`'li başka eşin aktarımına ulaşmaz (üç cihaz uçtan uca).
- İptal + yeni `connect` aynı grupta: `Closed{Abandoned(Cancelled)}` yutulmaz, tek bilet kalır.
- Teslim kanalı taşması: F4 — düşme yok, `delivery_channel_full` ölçüde.
- Epoch telemetrisi: yanıtlayan 0 → 1 geçişi; üst düzey epoch en küçük.
- `FILE_IDLE_TTL` kuyruk boşaldıktan sonra; `PTT_STREAM_IDLE_TTL` (PTT-R4) trafiksiz turda; `FILE_RECEIPT_TTL` tutamacı aynı turda kapatır.
- Kapanış: kuyrukta yer varken bayrak kalkmaz; doluyken grup sonunda biter, kalan olay üretmeden düşer.
- Biletler `Ready`'de ve yeniden bağlantıda çıkar; girdi tavanları motorun kendi sınırı (D14 1002 B); alındı olayı zincir sırasında; birleşik anahtar Transport bayrağından türer.

### node/signaling_tests.rs (192)

- `Signaling::pump` boşalan turda ömrü dolan kaydı budar; blob son halkada tüketilir, metin aynen ve sırasıyla geçer, PTT/dosya motoruna sızmaz.

### node/tests/glare.rs (325)

- Gerçek glare iki sıralamada: kaybeden ilk turunda yanıtlar (epoch 0), kazanan tek RESPONSE'la (epoch 1); tek oturum, erken metinler ve sonraki metinler tam bir kez + alındı; Transport ve Messenger aynı etkin oturum (DR-CONNECT-1 #4, K2).

### node/tests/hub_relink.rs (245)

- Eski hattın geç `ROUTE_MISS`'i (`HandleLost{h1}`) ve geç `AttemptEnded` yeni oturumu yenilemez; yeni hattın kaybı tek yenileme, tele tek INIT.

### node/tests/intent_t3.rs (247)

- Glare kaybedeninin niyet INIT'i `cancel_init` ile durur; `CancelConnect` T3'ü durdurur; glare'le çözülen oturumun hat kaybı yenilemesi terk edilmez.

### node/tests/ptt_pilot.rs (323)

- Yönetilen eşe ilk basış birleşik 300 ms (ayrı OPEN/CLOSE yok, açılış bilgili 3 demet + kompakt END); bilinmeyen eş uyumlu biçim (kayıt dizisi bayrak kapalıyla aynı), bant içi 0x17 sonrası sonraki basış birleşik; dizin dışı yönetilen ad yapılandırma hatası; eski `ptt_open` API'si pilot açıkken de ayrı kayıtlar.
- Karar 9 ile uyumlu (300 ms pilot açıkça seçilir; bilinmeyen eşin uyumluluk yolu ayrı). Not: uyumluluk yolunun süresi çağıranındır (testte 40 ms); "200 ms" düğümde sabitlenmez — karar 9'un 200 ms'i test/medya tarafının seçimi.

### tests/common/mod.rs (253) ve entegrasyon testleri

- `common`: kütüphaneyle provisioning, `root.json` cihaz dizinine girmez (D9), olay bekleyici üst sınırlı.
- `api_paths.rs`: `send_file` hataları olay olarak; `drain_background` boştaki eşe bağlanır; `cancel_connect`; `TooLarge` tam limitte; `shutdown` önceki komutları işler.
- `glare_simultaneous_connect.rs`: ürün yolunda eşzamanlı connect → tek `Connected`, kapanış yok, iki yönde tek teslim (glare'in kendisini iddia etmez).
- `signaling_loopback.rs`: blob iki yönde bayt-özdeş, sırasıyla, bir kez; 1006 B tavanı; bilinmeyen eş / oturumsuz eş tipli hata.
- `start_config.rs`: `IdentityInUse` ve kaynak bırakma; röle kaydından sonraki hata kimliği ve soketi bırakır; eksik dosya veri dizini yaratmaz; `root.json` reddi ve hiç açılmaması; `NoDirectAddress`; aynı `data_dir` kilidi; dinleyiciden `ReentrantCall`.
- `two_nodes_loopback.rs` (CORE): metin ↔ alındı, 150 KB dosya, PTT akışı, kapanmış tutamaç hataları, epoch-1 ve sıfır düşme ölçüleri, ürün yolunda `rtt_ms`/`loss_pct`/`bitrate_kbps`/`last_heartbeat_age_s` `None`, outbox kalıcılığı, yeniden başlatmada aynı `msg_id` ve artan `init_counter`.
- Not: entegrasyon ve birim testleri süreç ortamını okuyan ürün kurulumunu kullanıyor; ortamda `KAMA_PILOT_RECORD=1` ya da `KAMA_LINK_RATES` varsa sonuçları değişir (N-08).

## Bulgular

| fid | tür | ciddiyet | konum | davranış | otorite | düzeltme türü |
|---|---|---|---|---|---|---|
| N-01 | UYUŞMAZ | orta | `node.rs:512`, `node.rs:776`, `node.rs:800-814`; `hub_mode.rs:162-166` | `note_up` hem `connect()`'in `ConnectResult::Ready`'sinden hem her `SessionEvent::Ready(_, peer)`'den çağrılır, oturum tutamacı atılır; hub kipinde `HubSide::peer_ready(peer)` `KnownHandles::handle_of(peer)` = eşe EN SON atanmış handle'ı (bekleyen yenileme/relink denemesininki dahil; `RecordingLink::poll` `HandleAssigned`'da `bind_peer`) `HubLink::peer_ready`'ye verir, o da `assigned == h` olan denemeyi kapatır (`link.rs:722-729`). Senaryo: kurulu oturum, `HandleLost` → relink INIT, `StoreResp OK` → `h2`; UI `connect(peer)` → eski oturumun `Ready`'si → yeni denemenin T3'ü doğrulanmış RESPONSE olmadan durur; INIT DELIVER kaybolursa yenileme takılır. Statik okuma; dar testle üretilmedi. | SAHİP-KARARI-v4-EK-A §10 HB1 KK-H36: "T3 yalnız o denemeye/oturum nesline ait **doğrulanmış RESPONSE kabulüyle** durur: `peer_ready(peer, session_handle)` kama-node `note_up`'tan; eski oturumun Ready'si yeni denemeyi kapatmaz." S4 KR-18 / C3: "Tekrar yalnız … doğrulanmış RESPONSE kabul edildi … ile durur." | kod-duzeltmesi |
| N-02 | EKSİK | orta | `node.rs:541-547` | `PlatformSignal::LinkChanged` hub kipinde de etkisiz; kurulu oturumlar için yeni INIT / `SESSION_INIT_REF` yok. | PoC-KAPSAM-001 K4: "`PlatformSignal.LinkChanged` → hop bağı düşer → her aktif eş için `SESSION_INIT_REF` + `early_unit`"; S4 C7: "Yeni INIT kurulmuş bir oturum için yalnız (a) OS `LinkChanged` … ya da (b) `ROUTE_MISS`". Plan D15 bunu "hub sprintinde; NON-GATING" diye erteledi; hub kipi kodda var (S2-A3-2). | karar-gerekir |
| N-03 | EKSİK | orta | `node.rs:1019-1034`, `event.rs:193-198`, `a5.rs:1-12` | Ürün kurulumunda (`KamaNode::start`, canlı ayna kapalı) `rtt_ms` ve `loss_pct` daima `None`; `bitrate_kbps` her kipte `None`. | PoC-KAPSAM-001 §2 Android: "ölçüm overlay'i (RTT, kayıp, bitrate, …)"; KAMA-PLAN-4GUN Ek 2026-10-12 (5) / KATMAN 6: "motor sRTT / kayıp / bitrate salt-okunur erişimcileri + K8 overlay alanları (hub sprinti, **demo öncesi zorunlu**)". Ayna kapalı varsayılanı DUR-19 (metni depoda bulunamadı). | karar-gerekir |
| N-04 | TANIMSIZ | orta | `a5.rs:11-12`, `a5.rs:38-68`, `a5.rs:96-116` | `loss_pct` = 10 s penceresinde Δ`retransmits` / Δ`tx_datagrams` × 100 (açık + emekli oturumlar). `retransmits` tail-probe kararlarını da içerir (`node.rs:387` "tail-probe" alt kümesi) ve kayıp değil yeniden iletim KARARI sayar; FEC'le onarılan kayıp görünmez. Ad "kayıp" ama ölçülen yeniden iletim oranı. | "A5 kartı" ve KK-H59 metni depoda bulunamadı (aranan: `A5 kartı`, `KK-H59`, `loss_pct` — `docs/`, `specs/`); PoC overlay "kayıp"ı tanımlamaz. DOĞRULANAMADI. | karar-gerekir |
| N-05 | UYUŞMAZ | orta | `lib.rs:329-340` (+ `client-core/src/runtime.rs:719-720`) | `KAMA_TRANSPORT_ENGINE` boş değerle tanımlıysa `TransportEngine::parse("")` → `Legacy`; tanımsızsa ürün varsayılanı `CcPort`. Boş dışa aktarılan değişken düğümü sessizce eski motorla açar (yalnız başlangıç kaydı `motor=legacy` der). Yorum (331) "Boş/`legacy` = ürün (bit-eşit)" der. | KARAR-KAYDI-2026-11 §8 D-CC-1: "#176 (ürün varsayılanı port …; geri alma `KAMA_TRANSPORT_ENGINE=legacy`)"; CLAUDE.md YAMA KAYDI 2026-11-11: "Ürün varsayılanı port (`RuntimeConfig::engine`), geri alma `KAMA_TRANSPORT_ENGINE=legacy`." | kod-duzeltmesi |
| N-06 | UYUŞMAZ | düşük | `lib.rs:64-67`; `lib.rs:656`; `event.rs:108`, `event.rs:193-204`; `hub_mode.rs:160-161` | Belge-kod ayrışması: "hub ve `last_heartbeat_age_s` daima None", "`DeadlineBounded` bu sprintte yok", "`MEDIA_BUNDLE` Reliable — PTT-R0, geç gelen düşmez", "`rtt_ms`/`loss_pct` transport dışarı açmıyor", "T3 yedeği `on_session_traffic`". Kod: hub kipi var, C9 değeri dolu, demet `DeadlineBounded` (kama-app `ptt.rs:1189-1193`), A5 canlı aynadan doldurur, `on_session_traffic` T3'ü durdurmaz. | S4 C9; sahip kararı 2026-11-22 (PTT-YALIN, kama-app atfı); HB1 KK-H36 "'İlk giden oturum datagramı T3'ü durdurur' yedeği **kalkar**". | kod-duzeltmesi (yalnız belge; PTT davranışına dokunmaz) |
| N-07 | UYUŞMAZ | orta | `wiring.rs:29`, `wiring.rs:99-120`; `Cargo.toml:19-24` | Hub BIND imzası üretimde `ed25519_dalek::SigningKey::sign` ile; Ed25519 yolu libsodium değil (proje genelinde kama-noise-wire, kama-provision, hub-server da dalek). | ADR-03: "libsodium (FFI, tek onaylı C bağımlılığı): Ed25519, X25519, …; dalek ailesi …: pure-Rust yol; libsodium ile cross-check" ve "Ed25519/X25519/BLAKE2s/ristretto kararları DEĞİŞMEDİ". KAMA-PLAN-4GUN KATMAN 6 listesi: "ADR-03 libsodium/dalek amendment" (açık). | karar-gerekir |
| N-08 | TANIMSIZ | düşük | `lib.rs:219-227`, `lib.rs:346-424`; `link_profile.rs:33-115`; `ptt_pilot.rs:35-111`; `pilot_record.rs:27-48`; `lib.rs:231-237`; `error.rs:22-24` | Dağıtım anahtarları ortam değişkeni ve veri dizini dosyası olarak: `KAMA_ACK_FORMAT`, `KAMA_LINK_RATES`/`link-profile`, `KAMA_PTT_PILOT`/`ptt-pilot`, `KAMA_PILOT_RECORD`/`pilot-record`, derleme `KAMA_GIT_SHA`. Hataları `NodeError::Config(FormatError::Invalid)` ("provisioning file") içinde. Testler süreç ortamını okuyan ürün kurulumunu kullanır; ortamda bu değişkenler varsa sonuç değişir (ör. `two_nodes_loopback.rs:126-129` `rtt_ms == None`). | spec / ADR'de bulunamadı (aranan: dört değişken adı `specs/`, `docs/decisions/`, CLAUDE.md'de; yalnız `KAMA_TRANSPORT_ENGINE` D-CC-1'de). Kısmi dayanak: karar 9 (pilot açık seçim, V2 genel varsayılan), YAMA KAYDI 2026-11-16 (`link_rates`). | karar-gerekir |
| N-09 | TANIMSIZ | düşük | `lib.rs:398-413`; `ptt_pilot.rs:17-23`; `link_profile.rs:170-180` | Pilot açıkken motor denetlenmez: `KAMA_TRANSPORT_ENGINE=legacy` + `KAMA_PTT_PILOT=1` kabul edilir; başlangıç kaydının kendi kuralına göre bitmap alıcı yönü yalnız cc-port'ta etkin. ACK biçimi çelişkisi reddediliyor, motor çelişkisi reddedilmiyor. | `ptt_pilot.rs:18` "pilotun kabul kipi cc-port + bitmap + doğrulanmış 300 ms PTT"; karar 9 uygulama sırası "iki cc-port uç, bitmap, doğrulanmış 300 ms PTT". Ret kuralı otoritede yok. | karar-gerekir |
| N-10 | TANIMSIZ | düşük | `node.rs:864-874` | `AttemptEnded` sonrası `restart_attempt` bileti siler, niyeti iptal eder ve `connect` hatasını (`let _ =`) yok sayar: depo hatasında niyet olaysız biter (bilet silindiği için `ConnectAbandoned` da `Closed` doğurmaz), UI eşi `Idle` görür. | INV-NO-IDLE-TEARDOWN kapalı listesi (transport-layer-tdd §0): "(a) cancel · (b) karşı tarafın close'u · (c) anahtar hijyeni · (d) kalıcı yerel hata — peer material yok, sertifika iptal/expire"; depo hatası listede yok, olaysız bitiş tanımsız. | kod-duzeltmesi |
| N-11 | TANIMSIZ | düşük | `lib.rs:132`, `lib.rs:137`; `node.rs:99`, `node.rs:107`; `carrier.rs:74`; `files.rs:44`; `pilot_record.rs:34`; `a5.rs:25-28` | Yerel sabitler spec'ten türemiyor: `NODE_TICK = 20 ms` (ACK_TICK'e bağlı değil), `METRICS_TICK = 1 s`, `COMMAND_QUEUE_CAP = 64`, `HUB_NOTICES_PER_TURN = 16` (etiketsiz, gerekçesiz), `MAILBOX_CAP = 1024`, `NAME_ATTEMPTS = 16`, `SUMMARY_PERIOD = 5 s`, `MAX_SAMPLES = 12` (yorumdaki `LOSS_WINDOW / SAMPLE_PERIOD + 2` ifadesi kodda değil). Hepsi sınırlı (SR-AUDIT-001 uyumlu). | spec'te bulunamadı; çoğu `[KALİBRASYON]` diye beyanlı. | kod-duzeltmesi (`MAX_SAMPLES` ve `NODE_TICK`'i kaynağından türetmek, `HUB_NOTICES_PER_TURN`'e etiket) |
| N-12 | TANIMSIZ | düşük | `wiring.rs:133-140`; `node.rs:331` | `RelinkGovernor` jitter tohumu OS CSPRNG'den; kaynak hata verirse sessizce sıfır tohum. | DR-CONNECT-1 tamamlama 5 / K4: "Jitter RNG'siz … seed = `hub_handle` (bind sonrası) / hop ephemeral public (bind öncesi)" — bu kural prob kadansı içindir; hat-kaybı yenilemesine (S4 C5) uygulanıp uygulanmadığı DOĞRULANAMADI. Sıfır-tohum geri dönüşü hiçbir yerde tanımlı değil. | karar-gerekir |
| N-13 | TANIMSIZ | düşük | `node.rs:957-975`; `event.rs:173-175` | Doğrulanan gelen dosya diske yazılamazsa eşe `FILE_RECEIPT(ok)` gitmiş olur: gönderen `Delivered` görür, dosya alıcıda yoktur; yalnız yerel `FileFailed{Storage}`. | AMP §3: "`ok` = `FILE_END` görüldü, bütün parçalar geldi ve `whole_file_hash` tuttu" (depolamayı kapsamaz); PoC-KAPSAM-001 K8 "Fotoğraf … %100". İkisi arasındaki boşluk tanımsız. | karar-gerekir |
| N-14 | EKSİK | düşük | `lib.rs:37-43` (API listesi) | Medya hattı API'si (`media_open` / `media_close` / `media_send`, `MediaLinkOpened` / `MediaLinkClosed`) kama-node'da yok. | KM-SEAM-TH-001 C10 / KR-17: "`kama-node` API'si: `media_open(peer)` / `media_close(peer)` …". Belge v1.3.1 **taslak** (G-S4 öncesi). | karar-gerekir |
| N-15 | ÖLÜ | düşük | `hub_mode.rs:313-332` | `hub_mode_does_not_need_a_direct_address` hem `Ok` hem `Err(Transport)`'u kabul ediyor; "HB1 gövdesi gelene dek" gerekçesi bayat (`HubLink::new` var). Test yalnız `NoDirectAddress` olmadığını ayırt ediyor; hub kipinin kurulduğunu sabitlemiyor. | KK-H63 (hub kipinde `direct_addrs` gerekmez) — testin ayırt ediciliği. | kod-duzeltmesi |

Kilitli olanlar (bulgu tablosu dışında, yalnız kayıt): PTT ve metin davranışı (karar 3) — `ptt_push_timed` son tarih anlamı, birleşik 300 ms seçimi ve kama-app'ın `DeadlineBounded` demeti bu denetimde değiştirilecek kalem sayılmadı; N-06 yalnız belge cümlelerini kapsar. receiver.rs alarm aralığı (7186813951) ve #262 dalı bu dosyalarla ilgisizdir.

## Okunan otorite bölümleri

- CLAUDE.md §3–§9 ve YAMA KAYDI (2026-11-11 CC-PORT, 2026-11-16 İŞ-26 profil istisnası).
- #175 sahip kararları: 7171705736 (dokuz karar; 3 ve 9 doğrudan ilgili), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (kama-node'a doğrudan değmeyenler yalnız kilit kapsamı için).
- `docs/decisions/PoC-KAPSAM-001.md` tamamı (§2, §3, K1–K10, §5, EK-1..3).
- `docs/decisions/KARAR-KAYDI-2026-11.md` §1, §3, §6–§10 (D-CC-1).
- `docs/decisions/DR-CONNECT-1.md` tamamı.
- `docs/decisions/SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` §10 (HB1 KK-H36, HB2 KK-H52/H54, T1).
- `docs/seams/KM-SEAM-TH-001.md` başlık, §0 KR-1…KR-18, §4 C1–C10, §5.
- `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` D14, D15, D18, Ek 2026-10-12 (5), KATMAN 6 listesi, K4G-11 API satırı.
- `specs/tech-stack-adr.md` ADR-01, ADR-02, ADR-03 (+ AMENDMENT, AMENDMENT-2), ADR-13…ADR-19, §4 bağımlılık politikası.
- `specs/transport-layer-tdd.md` §0 (INV-NEIGHBOR-1, INV-NO-IDLE-TEARDOWN, INV-PLAINTEXT-NOT-SEMANTICS).
- `specs/application-message-protocol.md` tamamı (§2 kayıt başlığı, §3 tip tablosu ve PTT-R0…R5, K4G-09, K4G-14a, §4 boyut).
- `specs/application-protocol-and-rpc-spec.md`: kama-node'un hub RPC'si client-core'dadır; bu dosyada doğrudan karşılaştırılacak davranış bulunmadı (aranan: `CLIENT_BIND`, `SESSION_INIT_REF` kama-node çağrısı).
- Spec'te bulunamayanlar: "bileşim kökü INV-NEIGHBOR-1 istisnası" (aranan `specs/*.md`: "bileşim kökü", "composition root", "kama-node" — yalnız Cargo.toml beyanı var); "A5 kartı" / KK-H59 metni; DUR-19 metni.
