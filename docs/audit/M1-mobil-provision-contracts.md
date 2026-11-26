# M1 — kama-mobile · kama-provision · kama-contracts denetimi

Taban: `125b9afa` (ürün hattı; aday `1a4f9efc` bunun içinde). Salt okuma; kaynak değiştirilmedi, cargo çalıştırılmadı.
Satır numaraları `125b9afa`'dandır.

## 1. Kapsam

Bütün dosyalar baştan sona okundu (Read aracıyla, büyük dosyalar parça parça).

| Dosya | Satır aralığı | Satır |
|---|---|---|
| crates/kama-contracts/src/lib.rs | 1–39 | 39 |
| crates/kama-contracts/src/handles.rs | 1–148 | 148 |
| crates/kama-contracts/src/s1.rs | 1–359 | 359 |
| crates/kama-contracts/src/s3.rs | 1–363 | 363 |
| crates/kama-contracts/src/s4.rs | 1–1526 | 1526 |
| crates/kama-provision/src/lib.rs | 1–25 | 25 |
| crates/kama-provision/src/main.rs | 1–8 | 8 |
| crates/kama-provision/src/cert.rs | 1–252 | 252 |
| crates/kama-provision/src/encoding.rs | 1–248 | 248 |
| crates/kama-provision/src/format.rs | 1–706 | 706 |
| crates/kama-provision/src/provision.rs | 1–815 | 815 |
| crates/kama-provision/src/cli.rs | 1–352 | 352 |
| crates/kama-mobile/src/lib.rs | 1–1041 | 1041 |
| crates/kama-mobile/src/error.rs | 1–598 | 598 |
| crates/kama-mobile/src/event.rs | 1–957 | 957 |
| crates/kama-mobile/src/test_support.rs | 1–113 | 113 |
| crates/kama-mobile/src/bin/uniffi-bindgen.rs | 1–58 | 58 |
| **Toplam** | | **7608** |

Bağlam için ayrıca okunanlar (kapsam dışı, bulgu üretmez): üç crate'in `Cargo.toml`'u.

## 2. Davranış envanteri

### 2.1 kama-contracts

**lib.rs (1–39).** Dört modülü (`handles`, `s1`, `s3`, `s4`) dışa açar; `#![forbid(unsafe_code)]`. Belge yalnız S1/S3/handles'ı sayar, `s4`'ü anmaz (bkz. B-13). Kural: crate'in bağımlılığı yok (Cargo.toml'da bağımlılık yok; yalnız `test-util` özelliği).

**handles.rs (1–148).**
- `SessionHandle(u64)`: opak tutamaç; `from_raw` herkese açık `const fn`; `to_raw_for_mock` yalnız `test-util` ile. Wire'a çıkmaz (belge).
- `PeerRef([u8;32])`: `dest_hash` sarmalayıcısı; `from_dest_hash`, `as_bytes` (pub), `to_raw_for_mock` (test-util). `Debug` ürün derlemesinde `PeerRef(<redacted>)`, test derlemesinde ilk 4 bayt.
- `CloseReason {Superseded, UserClosed, PeerClosed, KeyHygiene}` kapalı küme.
- Testler: kapalı küme pini; `PeerRef` eşitlik/`as_bytes`; Debug'ın 4 bayttan fazlasını basmaması.

**s1.rs (1–359).** S1 (Application↔Transport) tipleri ve `KamaTransport` trait'i.
- `UnitBytes = Vec<u8>` (spec `Bytes`; bilinçli fark, belgeli).
- `TransportCapabilities {max_plaintext_unit_bytes, preferred_plaintext_unit_bytes, max_early_unit_bytes, reliable}`.
- `PreparedUnit`, `CompressionPolicy {Never, AppCompressed}`, `DeliveryPolicy {Reliable, DeadlineBounded{deadline: Instant}, BestEffort}`.
- `TrafficClass {Be, Ctrl, Sig, Bk, Vo, VoRel}` + `ALL` + `code()` = 0..5 (EK-1 D-08 numaraları; telde yok).
- `Delivered`, `SessionEvent {Ready(h,peer), RenewRequired, Closed, ConnectAbandoned, ConnectProgress}`, `AbandonReason` (4), `ConnectPhase {Measuring, Suspended, LongOutage}`, `ConnectResult {Ready, Pending}`, `ConnectTicket(u64)` (`from_raw` pub).
- `TransportError` 7 varyant, elle `Display` + `Error` (bağımlılıksız kural).
- `KamaTransport`: `capabilities`, `connect`, `send(.., class)`, `recv`, `poll_event`, `close -> Result`, `cancel`.
- Testler: kapalı kümeler, D-08 numaraları, trait nesne-uyumu, kararlı hata metinleri.

**s3.rs (1–363).** S3 (Transport↔Crypto).
- `HubHandle(u32)`: `from_raw`/`raw`; `SessionHandle`'a dönüşüm yok.
- `ProtectionContext {Session(h), HubChannel}`, `ProtectionClass`, `ProtectionLimits {wire_overhead_bytes, max_plaintext_bytes}`, `SealedPacketMeta {full_wire_pn u64, dh_epoch u8, sealed_len u32}`, `SealedUnit`, `Opened`.
- `SealError {FrameTooLarge, UnknownSession, CapReached}` + `Display`.
- `PacketProtection: Send + 'static` (`seal`, `open -> Option`, `sealed_len`, `conservative_limits`, `limits`).
- `SessionCommand {Initiate, AcceptInit, AcceptResponse, AssignHandle, Renew, Close}`, `SessionResult {Initiated, Established, EstablishedAndReply{peer}, ProvisionalAndReply, NoOutput, Failed}`, `SessionEvent {Ready, RenewRequired, Closed, Confirmed}`, `SessionControl`.
- Testler: kapalı kümeler, nesne-uyumu, hata metinleri, `HubHandle` gidiş-dönüş; derleme zamanı sondasıyla `HubHandle ↔ SessionHandle` dönüşümünün yokluğu.

**s4.rs (1–1526).** S4 tel biçimi (KM-SEAM-TH-001 v1.3.1 §2): sabitler, kod çözücüler, TBS/cookie/tahsis-anahtarı birleştiricileri; kripto yok.
- Sabitler (hepsi S4 §2/§6'dan, türetilenler formülle): `HUB_CHANNEL_HANDLE=0`, `FRAME_* 0x1B/0x1C/0x1D`, `METHOD_* 0x01/0x05/0x0B/0x81/0x84`, `METHOD_HUB_INITIATED=0x80`, `FLAG_RESPONSE=1`, `FLAG_ERROR=2`, `ENVELOPE_* 0x01/0x02/0x05`, durum `0x0000/0x0003/0x0301/0x0302`, hata `0x0105/0x0106/0x0501/0x0502/0x0601`, `MAX_DATAGRAM=1280`, `RELAY_OPEN_MAX_ENVELOPE=64`, `HUB_TRAILER_LEN=16`, türetilmiş `CONTROL_MIN_LEN=21`, `MAX_OPENED_FRAME=1264`, `RPC_HEADER_LEN=10`, `RPC_OVERHEAD=31`, `MAX_RPC_PAYLOAD=1249`, `STORE_REQ_FIXED_LEN=DELIVER_FIXED_LEN=41`, `BIND_TBS_LEN=112`, `ACK_TBS_LEN=83`, `ALLOCATION_KEY_LEN=77`, etiketler `KAMA-HUB-BIND-V1`/`KAMA-HUB-ACK-V1`.
- `S4Error` (15 varyant, `non_exhaustive`, kararlı `as_str`).
- `Reader` (özel): `take/array/u8/u16/u32/u64`, `tail_with_len` (önce `len ≤ max`, sonra `len == remaining`, ancak sonra `to_vec` — bounds-before-alloc), `finish` (artık bayt reddi).
- `hub_frame_encode/decode`: `[0;4]‖frame_type‖body`; kuyruk eklenmez/çıkarılmaz; ilk 4 bayt ≠ 0 → `NotControl`; > 1264 → `TooLong`.
- `Heartbeat` gövde 0 ya da 4 B; `HubFrame` tipli çözme; bilinmeyen tip → `UnknownFrameType`.
- `AppRpcFrame` encode/decode (`payload_len ≤ 1249`).
- Yükler: `ClientBindReq` (50 B, mod/presence 0 değilse `BadField`), `ClientBindResp` (35 B, sabit alanlar + `CHALLENGE`), `BindProofReq` (128 B), `BindProofResp` (83 B, `bind_ok=1`), `valid_combo`, `StoreMessageReq` (+`validate`: birleşim + RELAY_OPEN ≤ 64), `StoreMessageResp` (OK ise handle ≠ 0), `Deliver` (handle ≠ 0, tip ∈ {1,2,5}), `RouteMiss` (reason = 1), `ErrorPayload` (yalnız hata sütunu kodları), `RelayOpenEnvelope` (16 B).
- `RpcBody`/`RpcMessage::to_frame/from_frame`: method → flags → request_id sırası; hub başlatımlıda flags 0 ve request_id 0.
- `domain_sep` (const; > 16 B ya da ASCII değil → `BadLength`), `DOMAIN_SEP_HUB_BIND/ACK` (const değerlendirmede `panic!`, çalışma zamanında erişilemez), `bind_tbs`, `ack_tbs`, `concat_into` (özel), `CookieSrc` (IPv4-eşlemeli IPv6 normalizasyonu), `cookie_input` (63/75 B), `AllocationKey::encode` (77 B).
- Testler: her satırın gidiş-dönüşü, S4 §3 H10 boyları, sınır boyları, büyük `payload_len`'in ayırmasız reddi, heartbeat, birleşimler, hub başlatımlı kurallar, normalizasyon, etiket boyu, kod sütunları.

### 2.2 kama-provision

**lib.rs / main.rs.** Beş modül; `main` → `cli::run(args_os)`. `forbid(unsafe_code)`.

**cert.rs (1–252).**
- Sabitler: `KEY_LEN=32`, `ISSUER_KEY_ID_LEN=16`, `NOT_AFTER_LEN=8`, `SIGNATURE_LEN=64`, `DOMAIN_SEP_LEN=16`, `DOMAIN_SEP_CDC_V1 = "SR_CDC_V1"+7×0` (ccr §13.2 YAMA 2026-10-11), `SIGNED_FIELDS_LEN=88`, `TBS_LEN=104`, `SPKI_ALG_ED25519=0x01` (ccr §6.3), `DEVICE_CERT_VALIDITY_SECS=90 gün` (ccr §6.8/§13.1); derleme zamanı düzen iddiaları (152 B).
- `CertFields::signed_fields/tbs`; `sign_compact_device_cert` (ed25519-dalek `try_sign`); `split_compact_device_cert` (yapısal, doğrulamasız); `verify_compact_device_cert` (yalnız ccr §13.8 adım 9, `kama_noise_wire::ed25519_strict::verify_strict`); `issuer_key_id = SHA-256(0x01‖pub)[0..16]`; `identity_hash = SHA-256(pub)`; `dest_hash = SHA-256(identity_hash)`.
- Testler: etiket dolgusu, ofsetler, imzanın yalnız kendi kökünde doğrulanması ve bayt bozulmasında düşmesi, spki ≠ ham özet.

**encoding.rs (1–248).** Std-only küçük harf hex (`hex_encode`, `hex_decode<N>`, `hex_decode_into` — gizli tampona doğrudan) ve RFC 4648 kanonik base64 (`base64_len`, `base64_encode`, `base64_decode<N>`; dolgu yeri, alfabe, kullanılmayan bitler sıfır). Hatalar yalnız uzunluk taşır. Testler: RFC 4648 §10 vektörleri, kanonik olmayan girdi reddi, 0/31/32/33/152 B gidiş-dönüş.

**format.rs (1–706).**
- Sabitler: `FORMAT_VERSION=1`, `HUB_FILE_NAME`, `PROBE_NAME="probe"`, `HUB_LISTEN_HOST="0.0.0.0"`, `ROOT_FILE_NAME`, `.gitignore`=`*\n`, `MIN_DEVICES=2`, `MAX_DEVICES=64`, `MAX_NAME_LEN=32`, `MAX_FILE_BYTES=1 MiB` (uygulama sınırı, PROVISIONING-FORMAT-v1 §7).
- `DeviceName` (`[a-z0-9][a-z0-9_-]{0,31}`), `Public32` (hex; Debug basmaz), `Secret32` (öbekte, Clone değil, Debug maskeli, Drop'ta `zeroize`, JSON'dan doğrudan öbek tamponuna), `CertBytes` (base64, `cert_ref = cert_ref_of`).
- Dosya tipleri (hepsi `deny_unknown_fields`): `KeyPair`, `RootFile`, `DeviceConfig`, `PeerConfig`, `HubConfig {addr, x25519_pub: Option, ed25519_pub}`, `HubDevice`, `HubFile` (+`validate`: sürüm, `listen` soket adresi, anahtar çifti, 1–65 cihaz, tekillik).
- `FormatError` (JSON hatası yalnız kategori+satır/sütun).
- `load_device_config` → `read_bounded` (önce metadata boyu, sonra `take(MAX+1)`; `Zeroizing` tampon) → serde → `validate`.
- `DeviceConfig::validate`: sürüm; üç anahtar çiftinin priv↔pub tutarlılığı; kimlik ≠ cihaz anahtarı; `identity_hash`/`dest_hash` türetmesi; kendi sertifikasının taşıdığı anahtarlar; eş sayısı (probe dışı 1–63, probe 0 + hub zorunlu); ad/`dest_hash`/`cert_ref` tekilliği; eş x25519 tüm-sıfır reddi; eş `dest_hash = SHA-256(identity_hash)`; eş sertifikasındaki x25519 = `x25519_pub`; `direct_addrs` bilinen ad ve port ≠ 0; `hub.addr` host:port. İmza, `issuer_key_id`, `not_after` DENETLENMEZ.
- `local_identity()`, `peer_directory()` → kama-noise-wire tipleri.
- Testler: ad kuralı, Debug'da gizli/kimlik baytı yok, `Secret32` JSON, host:port kuralı, `root.json`'ın cihaz dosyası olarak reddi.

**provision.rs (1–815).**
- `InitRequest`, `InitOptions {probe}`, `InitReport`, `RequestError` (7), `InitError` (serde_json hatası yalnız sınıf).
- `validate_request(_with)`: 2–64 cihaz, tekillik, `--direct` adları ve port, `--hub` host:port, `--probe` → `--hub` zorunlu ve `probe` adı serbest.
- `init_with`: istek denetimi → `not_after = created_at + 90 gün` (`checked_add`) → kök + cihaz başına üç anahtar (x25519, cihaz ed25519, kimlik ed25519) + kökle imzalı sertifika → isteğe bağlı probe ve hub anahtarı → her cihaz dosyası için öz-denetim (`validate` + kök imzasının katı doğrulaması) → `hub.json` + `validate` → `root.json` → yazım (`.gitignore` önce). Girdi/entropi/öz-denetim hatasında dosya sistemine dokunulmaz.
- `random_key`: tüm-sıfır tohum → `ZeroKey`.
- `to_json`: uzunluğu önce sayar, tamponu tek seferde tam kapasiteyle ayırır (gizli içerikli ara tampon serbest bırakılmasın).
- `Output`: dizin yoksa `0700` oluşturur, varsa boş ve sembolik bağ olmayan dizin; dosyalar `O_EXCL` + `0600`; hata yolunda gizli dosyaları ters sırayla siler, biri silinemezse `.gitignore` ve dizini korur, kalan yolları raporlar; Drop'ta aynı temizlik (panik yok).
- Testler: girdi kuralları dosya yazmadan, entropi arızası/tüm-sıfır, tek ayırma, silinemeyen gizli dosyada `.gitignore`'un korunması, `commit` olmadan düşen çıktının temizliği, `O_EXCL`.

**cli.rs (1–352).** `parse_args` (komut `init`/`--help`; `--out/--devices/--hub/--direct` tek değerli ve tek kez, `--probe` değersiz), `parse_direct` (IP sabiti zorunlu, ad tekrarı reddi), `run` (çıkış 0/1/2; standart çıktıya yalnız yollar ve iki hatırlatma). Testler: tam komut, kullanım hataları.

### 2.3 kama-mobile

**lib.rs (1–1041).** UniFFI `KamaMobile` nesnesi: `kama_node::KamaNode`'un birebir sarmalayıcısı.
- `EngineListener` callback interface (`on_event -> Result<(), ListenerError>`).
- Yuva `Arc<RwLock<Option<KamaNode>>>` + `Lifecycle` (Mutex + Condvar; `Running → Requested → Stopping → Detached → Stopped`).
- Yöntemler: `start` (constructor), `connect`, `cancel_connect`, `send_text`, `send_file` (bloklamayan), `cancel_file`, `ptt_open/push/close`, `send_signaling` (`SignalingId::get`), `on_platform_signal` (bloklamayan), `drain_background`, `metrics` (bloklamayan), `shutdown`.
- `with_node`: JVM iş parçacığında okuma kilidini bekler, dinleyici içinde `try_read`; `with_node_now`: her yerde `try_read`, yazar tutuyor/bekliyorsa `Stopped`.
- `shutdown`: JVM'den `stop_and_wait` (yazma kilidi, düğümü al, kilidi bırak, `KamaNode::shutdown`, ilk kapanışın bitişini bekle); dinleyiciden `try_write`, meşgulse `request_stop` → kısa ömürlü `kama-stop` iş parçacığı (yalnız ilk istek), başlatılamazsa `Spawn` ve istek geri alınır.
- `ListenerBridge`: olayı aynaya çevirir, thread-local `IN_LISTENER` kapsamıyla çağırır, dinleyici hatasını yutar; Drop'u döngü bitişi sinyalidir (`loop_ended(thread::panicking())`).
- Sabit: `STOP_THREAD_NAME = "kama-stop"` (Linux 15 B sınırı).
- Testler: hatalı dinleyicinin yutulması ve bayrağın geri gelmesi; Detached → Stopped; bekleyen Condvar'ın uyanması; isteğin tek kez açılması ve geri alınması; dinleyici çağrılarının yuva kilidini beklememesi; JVM bloklamayan çağrıların beklememesi; dinleyiciden asenkron kapanış ve yeniden açılış; meşgul yuvada `kama-stop` devri; eşzamanlı iki JVM kapanışının tam duruşu beklemesi.

**error.rs (1–598).** `NodeError` (kama-node aynası, `_` kolsuz tam eşleme; kurulum hatalarında `reason = Display`), `wide(usize→u64)` (doyurmalı), `ListenerError::Unexpected` (`UnexpectedUniFFICallbackError` metni atılır), motor hata aynaları `TransportError`, `EncodeError`, `AppStoreError`, `MessengerError`, `PttError`, `FileSendError`, `SignalingError`. Testler: tipli neden korunur, `TooLarge` alanları, dinleyici istisna metni sızmaz.

**event.rs (1–957).** `EngineEvent` (13 varyant) ve elle `Debug` (metin/ses/blob yerine uzunluk); `From<NodeEvent>`; `path_text` (kayıplı UTF-8, panik yok); `io_kind_name` (ErrorKind Debug); tutamaç kayıtları `StreamHandle`/`FileHandle`; `PlatformSignal {LinkChanged}`; `PttCodec {Opus, Lyra}`; `ConnectPhase`, `CloseCause`, `CloseReason`, `AbandonReason`, `FileStage`, `FileRef`, `FileFailReason`, `FileFailure`, `FileAbortReason`, `SendFailure`, `NodeMetrics` (`delivery_channel_full` FFI'ya taşınmaz), `PeerConnectState`, `LinkState`. Testler: Debug sansürü, tutamaç/codec gidiş-dönüşü, ölçü aynası.

**test_support.rs (1–113).** Yalnız `cfg(test)`: `serial()` kilidi, `TempDir`, `provision()` (gerçek `kama-provision init` CLI'ı, iki boş UDP portu; telefona yalnız `device-alice.json`), `WAIT = 60 s`.

**bin/uniffi-bindgen.rs (1–58).** Yalnız `bindgen` özelliğiyle: `uniffi::generate` (Kotlin, kütüphane kipi, `crate_filter = kama_mobile`, yapılandırma crate kökündeki `uniffi-bindgen.toml`, biçimlendirici yok). Çıkış 0/1/2.

### 2.4 Uyumlu sayımı (dosya başına)

| Dosya | Uyumlu | Öne çıkanlar |
|---|---|---|
| kama-contracts/lib.rs | 2 | `forbid(unsafe_code)`, bağımlılıksız crate (S1/S3 belgelerindeki "Bytes→Vec<u8>" farkı beyanlı) |
| handles.rs | 6 | `CloseReason` kapalı kümesi S1 §1 ile birebir; `PeerRef` alanı özel, `Debug` sansürlü (CLAUDE.md §4 dest_hash) |
| s1.rs | 15 | trait imzaları KM-SEAM-AT-001 §1 (S1 v2, `send(.., class)`) ile birebir; `TrafficClass` D-08 numaraları; `TransportError` kapalı küme; `ConnectPhase::LongOutage` = KS-UI-1 |
| s3.rs | 13 | KM-SEAM-TC-001 §1.1/§1.2 + v1.1 YAMA'sı (Initiated, peer, AssignHandle, Close); `HubHandle ≠ SessionHandle` tip düzeyinde (ccr §35.7) |
| s4.rs | 38 | bütün sabitler S4 §2/§6 ile; sabit genişlik big-endian (ADR-05, varint yok); bounds-before-alloc (SR-AUDIT-001); fail-closed çözücüler; birleşim tablosu §2.4(c); TBS 112/83 B §2.4(b); cookie girdisi §2.4(a) |
| cert.rs | 9 | 152 B düzen ccr §13.1; TBS ccr §13.2 + YAMA 2026-10-11; `issuer_key_id` ccr §8.1/§13.4; `identity_hash`/`dest_hash` ccr §13.3/§13.5 (cihaz anahtarından türetilmez); katı Ed25519 ccr §5.2; 90 gün ccr §6.8 |
| encoding.rs | 6 | RFC 4648 kanonik; hata içerik taşımaz |
| format.rs | 21 | `hub.x25519_pub` otorite değil (CLAUDE.md §4 cert-bound); `root.json` cihaz dosyası olamaz (PoC-KAPSAM-001 §2 EK-1); tüm-sıfır x25519 reddi (ccr §5.1); boyut önce (SR-AUDIT-001); kapalı-sistem dizini (ccr §35.4) |
| provision.rs | 18 | kök gizli anahtarı yalnız `root.json`'da (EK-1); sertifika test köküyle (PoC-KAPSAM-001 §2 "Anahtarlar"); öz-denetimde katı imza; `0600`/`0700`, `O_EXCL`, `.gitignore` önce; Drop'ta panik yok |
| cli.rs | 8 | standart çıktıda gizli değer yok; kullanım hatasında dosya yazılmaz |
| main.rs | 1 | — |
| mobile/lib.rs | 20 | yüzey KM-SEAM-AK-001 §1 ile birebir; ADR-15 kaba taneli (çağrı/olay başına geçiş, datagram geçmez — AK-001 §0); ADR-01 mobil binding `unsafe` istisnası (kendi kodunda `unsafe` yok); dinleyiciden kilitlenme yerine tipli `ReentrantCall`; panik FFI'ı geçmez |
| error.rs | 10 | bütün aynalar `_` kolsuz tam eşleme; dinleyici istisna metni atılır |
| event.rs | 19 | olay kümesi AK-001 §2 ile birebir; Debug sansürü; sinyalleşme blob'u opak, alındısız/dedupsuz (signaling-seam-v0 §3 "duplicate mümkün", §4 opak) |
| test_support.rs | 3 | fikstür elle yazılmaz, `root.json` telefon dizinine girmez (D9) |
| bin/uniffi-bindgen.rs | 2 | `cargo metadata`/clap yok |

Sahip kararıyla spec'ten sapan ama karara uyan davranışlar (bulgu sayılmadı): sertifikayı Identity/User Cert yerine test kökünün imzalaması ve `issuer_key_id`'nin kökten türemesi (ccr §13.1/§13.4 yerine PoC-KAPSAM-001 §2 "Anahtarlar: CompactDeviceCert test köküyle"); çalışma zamanında zincir/iptal doğrulamasının yokluğu (ccr §35.4 KAPALI-SİSTEM KİPİ YAMA 2026-10-08); gizli anahtarların düz JSON'da diskte durması (identity-and-trust-model A-07 "plaintext diske asla" yerine PoC-KAPSAM-001 §2 EK-2 "kabul edilmiş risk").

## 3. Bulgular

| # | Tür | Önem | Yer | Davranış | Otorite | Düzeltme türü |
|---|---|---|---|---|---|---|
| B-01 | UYUŞMAZ | orta | kama-provision/src/cert.rs:21,103–113; format.rs:21,25,483–491; provision.rs:22,26,354–402 | Ed25519 imza/anahtar üretimi ve X25519 türetmesi yalnız `ed25519-dalek`/`x25519-dalek` ile; libsodium yolu ve iki uygulama arası çapraz denetim yok. Desen depo geneli (hub-server, kama-node, kama-noise-wire da dalek). | ADR-03: "libsodium (FFI, tek onaylı C bağımlılığı): Ed25519, X25519 … dalek ailesi … libsodium ile cross-check testleri"; AMENDMENT/AMENDMENT-2 yalnız AEAD ve BLAKE2s'i değiştirir: "Ed25519/X25519/BLAKE2s/ristretto kararları DEĞİŞMEDİ". | karar-gerekir (proje saf Rust'a evrildi, DR-5 emsali; Ed25519/X25519 için karşılığı olan bir ADR değişikliği depoda yok) |
| B-02 | UYUŞMAZ | orta | kama-provision/src/cert.rs:77–137 | CompactDeviceCertV1 (imzalı canonical yapı) kodlayıcısı ve ayrıştırıcısı ccr-codec dışında yazılmış; ccr-codec'te karşılığı yok. | ADR-05: "Tüm imzalı/canonical yapılar … `ccr-codec` crate tek kaynak"; tech-stack-adr §4: "Canonical yapı üreten/parse eden TEK crate: ccr-codec (kopya parser YASAK — divergence riski)". Kod ve PROVISIONING-FORMAT-v1 §4.3 "plan Ek 2026-10-11 (3) ile onaylandı" der; o plan eki depodaki karar kayıtlarında bulunamadı → onay DOĞRULANAMADI. | karar-gerekir (kodlayıcıyı ccr-codec'e taşımak bir kod işi; ama onayın varlığı/kapsamı sahipte) |
| B-03 | UYUŞMAZ | orta | kama-contracts/src/s4.rs:456–1102, 1104–1187 | İmzalanan S4 TBS'leri (`bind_tbs`, `ack_tbs`) ve S4 tel kod çözücüsü ccr-codec dışında, kama-contracts'ta. | ADR-05 (yukarıdaki alıntı). Çelişen kural: kama-contracts'ın bağımlılıksız olması (`scripts/ci/layer_check.py` C1, crate belgesi) — S4'ü ccr-codec'e taşımak ya da kama-contracts'ı ccr-codec'e bağlamak bu kuralı ya da ADR-05'i bozar. | karar-gerekir |
| B-04 | DOĞRULANAMADI | orta | kama-contracts/src/s4.rs:1–20 (modülün tamamı) | Modül KM-SEAM-TH-001 v1.3.1'i uygular ve onu "baytların tek otoritesi" sayar. | KM-SEAM-TH-001 başlığı: "v1.3.1 taslak · Durum: G-S4 öncesi — 'G-S4 GEÇTİ' mesajıyla FROZEN olur"; PoC-KAPSAM-001 §2: "hub sprintinde G-S4 ile FROZEN"; KARAR-KAYDI-2026-11 §2: kapı sözleri "yalnız sahibin ayrı mesajı … belgeye gerçekleşmiş olarak yazılmaz". Depoda "G-S4 GEÇTİ" kaydı bulunamadı (aranan: docs/decisions/*.md, verilen #175 yorumları). | karar-gerekir |
| B-05 | EKSİK | orta | kama-provision/src/format.rs:500–595 (`DeviceConfig::validate`), 444–449 | Yükleyici eş sertifikalarının (ve kendi sertifikasının) `issuer_signature`'ını dosyadaki `root_pub`'a karşı doğrulamaz; `issuer_key_id`'yi denetlemez. Tek doğrulama `init` anındaki öz-denetimdir (provision.rs:257). Sonuç: aktarım sırasında tutarlı biçimde değiştirilmiş bir eş kaydı (x25519 + sertifika takası) dizine girer; PoC'de `dest_hash` sertifikaya kriptografik olarak bağlı olmadığından (ccr §35.4 adım 3'ün karşılığı yok) dizin bütünlüğü yalnız dosya bütünlüğüdür. | ccr §35.4 KAPALI-SİSTEM KİPİ: "Dizine giren her kayıt aşağıdaki 1–4. adımları dizin kurulurken (bant dışı) geçmiştir … hub-untrusted iddiası dizinin bütünlüğüne dayanır". Aynı metin: "zincir … ve iptal … çalışma zamanında YAPILMAZ". Yüklemede imza denetimi "çalışma zamanı" sayılır mı belirsiz; PROVISIONING-FORMAT-v1 §5 (tasarım belgesi, otorite değil) "imza denetlenmez" der. | karar-gerekir (kod tarafı küçük: yüklemede her sertifika için `verify_compact_device_cert(cert, root_pub)` + `issuer_key_id == issuer_key_id(root_pub)`) |
| B-06 | TANIMSIZ | düşük | kama-provision/src/cert.rs:47, provision.rs:205–207; çalışma zamanında hiçbir yerde | `not_after` (oluşturma + 90 gün) yazılır ama hiçbir tüketici denetlemez; süresi geçmiş kurulum çalışmayı sürdürür. | ccr §13.8 adım 10: "now <= not_after # bounded-stale dahil HİÇBİR context expiry'yi bypass etmez". ccr §35.4 KAPALI-SİSTEM yalnız zincir ve iptali muaf tutar. Spec'te bulunamadı (aranan: kapalı-sistem kipinde süre denetiminden muafiyet). | karar-gerekir |
| B-07 | ÖLÜ | düşük | kama-provision/src/format.rs:317 (`root_pub`), 341 (`HubConfig.x25519_pub`) | `DeviceConfig.root_pub` yüklenir, doğrulanmaz ve kama-provision dışında hiçbir ürün kodu okumaz (depo taraması: yalnız hub-server testinde alan adı). `hub.x25519_pub` okunur, kullanılmaz (bu kasıtlı: cert-bound). | B-05 ile bağlı. `x25519_pub` için CLAUDE.md §4 "Raw hub_noise_x25519_pub authoritative DEĞİL" — kullanılmaması uyumlu, alanın varlığı tasarım belgesinde. | karar-gerekir (B-05 kararına bağlı) |
| B-08 | UYUŞMAZ | düşük | kama-contracts/src/s4.rs:923–932 (`ErrorPayload::decode`), 909–918 | `detail_len > 0` olan hata gövdesi kabul edilir (en çok 1245 B). | KM-SEAM-TH-001 §2.3: "`ErrorPayload { error_code u16; detail_len u16 = 0 }` (… `detail` boş)"; modülün kendi kuralı (s4.rs:18–19): "PoC'de sabit olan alanın farklı değeri … → S4Error". `vectors/hub/s4-apprpc-v1.json` yalnız boş `detail` taşır. | kod-duzeltmesi (`tail_with_len`'den sonra `!detail.is_empty()` → `BadField`; kodlamada da boş olmayanı reddet) |
| B-09 | TANIMSIZ | düşük | kama-contracts/src/s4.rs:755–768 (`StoreMessageResp::decode`) | `PEER_UNBOUND` ve `UNKNOWN_HANDLE` durumunda `assigned_handle ≠ 0` kabul edilir. | S4 §2.4(c) ve §2.6 tabloları bu durumları `{PEER_UNBOUND, 0}` / `{UNKNOWN_HANDLE, 0}` yazar; H6 (mevcut hat) yanıttaki handle'ı belirtmez. Spec'te bulunamadı (aranan: mevcut-hat UNKNOWN_HANDLE/PEER_UNBOUND yanıtında `assigned_handle` değeri). | karar-gerekir |
| B-10 | TANIMSIZ | düşük | kama-contracts/src/s3.rs:143–145 | `seal` belgesi Transport'a davranış yükler: "Yapısal seal-reddi — çağıran sayar, emisyonu kayıp-sayar (INV-RATE-1)". | KM-SEAM-TC-001 §1.1'de ve CLAUDE.md INV-RATE-1 metinlerinde (2026-11-11, 2026-11-16) seal reddinin kayıp sayılması yok. Spec'te bulunamadı (aranan: "kayıp-say", seal reddi muhasebesi). INV-RATE-1 2026-11-11 metninde kayıp `recovery_window` ile uçuş tavanını sınırlar; seal reddini kayıp saymak CC girdisini değiştirir. | kod-duzeltmesi (sözleşme belgesindeki yan cümleyi kaldırmak ya da otoriteye bağlamak; trait imzası değişmez) |
| B-11 | UYUŞMAZ | düşük | kama-contracts/src/s1.rs:40–42 | `max_early_unit_bytes` belgesi: "Crypto `conservative_limits`'ten INIT zarfı için türetilir". | KM-SEAM-AT-001 YAMA KAYDI 2026-10-08: "`max_early_unit_bytes` bu sprint INIT zarfından değil birim tavanından türer". (Seam belgesinin §1 bloğu da eski metni taşır; belge kendi içinde çelişkili.) | kod-duzeltmesi (yalnız belge yorumu; alan değişmez) |
| B-12 | TANIMSIZ | orta | kama-mobile/src/lib.rs:630–644 (`ListenerBridge::drop`), 525–553 | Dinleyiciden başlatılan kapanışın "tamamlandı" sinyali, köprünün kama-node `NodeLoop` alanları arasında transport ve depodan SONRA düşmesine dayanır; bu sıra kama-node'un sözleşmesi değildir, test sırayı garanti etmez (kodun kendi notu). Sıra bozulursa JVM `shutdown` kaynaklar bırakılmadan döner, aynı `data_dir`/port ile yeniden `start` yarışa girer. | Dayanak olarak "DUR-K4G-15-E CEVAP (a), CEVAP PAKETİ 6" anılıyor; depodaki karar kayıtlarında bulunamadı → DOĞRULANAMADI. KM-SEAM-AK-001 §1 `shutdown` sözü ("dönüşte depo kilidi, UDP soketi ve süreç-içi röle kaydı bırakılmıştır") bu sıraya dayanır. | karar-gerekir (kama-node'da açık kapanış kancası — kodun "KATMAN 6" dediği iş) |
| B-13 | TANIMSIZ | düşük | kama-mobile/src/lib.rs:141; error.rs:20; kama-provision/src/format.rs:54; kama-contracts/src/lib.rs:7–10 ve Cargo.toml | Belge kayması: mobil `start` ve `NodeError::Config` "PROVISIONING-FORMAT-v0" der (yükleyici v1'dir ve v0'ı reddeder, format.rs:33); `MAX_FILE_BYTES` yorumu "v0 dosyası" der; kama-contracts belgesi ve manifest yorumu yalnız S1/S3'ü sayar, `s4` modülü anılmaz. | PROVISIONING-FORMAT-v1 §8 (v1 v0'ın yerine geçer); KM-SEAM-TH-001 (S4). | kod-duzeltmesi (yalnız yorum) |
| B-14 | TANIMSIZ | düşük | kama-mobile/src/event.rs:776–779 | `kama_node::NodeMetrics.delivery_channel_full` FFI aynasına taşınmaz ("[F4, 2026-11-11] … Aralık"). F4 = PR #183'ün düzeltmesi; sahip kararı değil. | KM-SEAM-AK-001 §2 `NodeMetrics` alan listesi bu alanı içermez (seam ile tutarlı). ADR-15 yüzeyin kaba taneli olmasını ister, bir sayaç alanını yasaklamaz. Yüzeyden alan çıkarma kararı otoritede bulunamadı (aranan: delivery_channel_full, F4). | karar-gerekir |

Kilitli bulgu yok: PTT/metin sarmalayıcıları (`ptt_open/push/close`, `send_text`) yalnız tip çevirir ve çağrıyı iletir; davranış kama-app/kama-node'dadır (karar 3 dondurması o katmanlara aittir). `receiver.rs` ve #262 yolu bu kapsamda yoktur.

## 4. Okunan otorite bölümleri

- CLAUDE.md §4 (cert-bound, dest_hash, INV-ENGINE-SOC, INV-RATE-1 2026-11-11 metni ve 2026-11-16 profil istisnası), §5, §6, §8, §9, YAMA KAYDI.
- specs/tech-stack-adr.md: ADR-01, ADR-03 + AMENDMENT + AMENDMENT-2, ADR-05, ADR-09, ADR-12, ADR-15, §3 karar matrisi, §4 bağımlılık politikası.
- specs/ccr-format-spec.md: §5.1, §5.2, §6.1, §6.3, §6.8, §7, §8, §8.1, §8.3, §13.1–§13.8, §35.4 (KAPALI-SİSTEM KİPİ YAMA 2026-10-08).
- specs/pki-ca-ra-hsm-tdd.md §13 (kripto registry: Ed25519, X25519, SHA-256, BLAKE2s — kullanılan bütün primitifler registry'de).
- specs/identity-and-trust-model.md: E-4, CAD-42, anahtar tablosu (sk_user_identity), A-07 (grep ile ilgili satırlar).
- specs/signaling-seam-v0.md (tamamı).
- docs/decisions/PoC-KAPSAM-001.md §2 (Provisioning, Anahtarlar, Kalıcılık EK-1/EK-2, Sözleşmeler), K4, K6, K9, §5; docs/decisions/KARAR-KAYDI-2026-11.md (G-S4 ve kapı sözleri için arama).
- #175 sahip kararları 7171705736 (dokuz karar), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 — hiçbiri bu üç crate'e doğrudan hüküm koymaz; karar 3 (PTT/metin dondurması) sarmalayıcıların davranışını değiştirmeyi yasaklar, bu denetim değişiklik önermiyor.
- Sözleşme belgeleri (otorite sırasında spec'in altında, crate'lerin kendi kaynağı): docs/seams/KM-SEAM-AT-001.md (başlık, YAMA kayıtları, §1 bloğu, §4), KM-SEAM-TC-001.md (başlık, YAMA kayıtları, §0, §1.1, §1.2 başı), KM-SEAM-TH-001.md (başlık, §0 KR-1..8, §2–§6), KM-SEAM-AK-001.md (başlık, §0–§3), docs/provisioning/PROVISIONING-FORMAT-v1.md §4.2–§8.
