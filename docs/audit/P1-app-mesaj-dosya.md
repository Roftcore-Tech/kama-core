# P1-app-mesaj-dosya — kama-app metin, dosya, depo, saat, kök

Taban: `125b9afa` (ürün hattı; aday `1a4f9efc` bunun içinde). Salt okuma. Satır numaraları bu tabandandır.

## Kapsam

| Dosya | Okunan aralık | Satır |
|---|---|---|
| `crates/kama-app/src/lib.rs` | 1–41 | 41 |
| `crates/kama-app/src/clock.rs` | 1–65 | 65 |
| `crates/kama-app/src/store.rs` | 1–533 | 533 |
| `crates/kama-app/src/messaging.rs` | 1–1744 | 1744 |
| `crates/kama-app/src/file_transfer.rs` | 1–2790 | 2790 |
| **Toplam** | | **5173** |

Bağlam için ayrıca bakılan (kapsam dışı, bulgu bu dosyalara yazılmadı): `codec/compress.rs` 1–30, `codec/mod.rs` 615–645, `codec/limits.rs` 72, 175–243, `kama-contracts/src/s1.rs` 184–224, `kama-node/src/node.rs` 462–468, 885–925, `kama-node/src/lib.rs` 538–550, `kama-app/Cargo.toml` 19–29, `scripts/ci/layer_check.py` (blake2 satırları).

## Davranış envanteri

### lib.rs (1–41)
- Crate kökü: `#![forbid(unsafe_code)]` (ADR-01), modüller `clock, codec, file_transfer, messaging, playout, ptt, signaling, store`. `playout` "[YOL3-T1] PTT oynatma tamponu (gösterim)" olarak bildirilmiş (PTT denetçisinin kapsamı).
- Modül belgesi katman kuralını yazar: Application yalnız `KamaTransport`'u çağırır; tek ham primitif `blake2` (DAR istisna); Application retry yapmaz (S1 K8). Belgenin "Dosya, PTT ve bağlantı akışları B2.3–B2.5 kartlarıyla buraya gelir" cümlesi bayat (akışlar zaten burada) — davranış değil.
- Uyumlu: 3 (unsafe yasağı, katman kuralı, S1 seam'inin `kama-contracts`'ta olması).

### clock.rs (1–65)
- `MonotonicClock: Send + Sync` — `now() -> Duration`, keyfi başlangıçtan geçen süre, geri gitmez.
- `SystemClock { origin: Instant }`: `new()`/`Default` başlangıcı şimdi; `now()` = `origin.elapsed()` (geri gitmez, paniklemez).
- Test `system_clock_never_goes_back`: art arda iki okuma azalmaz.
- Uyumlu: 3 (enjekte saat, Application'ın kripto katmanının saatini görmemesi — INV-NEIGHBOR-1, panik yok). Bulgu yok.

### store.rs (1–533)
- `FIRST_COUNTER_VALUE = 0`; derleme zamanı eşitliği `FIRST_MSG_ID`, `FIRST_STREAM_ID`, `FIRST_TRANSFER_ID` ile (türetilmiş; spec başlangıç koymaz).
- `DEDUP_WINDOW_WORDS = MSG_DEDUP_WINDOW/64 = 64` (türetilmiş); pencere tam kelime denetimi.
- `MAX_OUTBOX_RECORD_BYTES = 5 + 65535` (AMP §2 başlık + `u16` `body_len`; türetilmiş).
- `CounterKind {MsgId, StreamId, TransferId}`; `max_value` tel genişliği (`u32::MAX`, `u32::MAX`, `u64::MAX`); `advance(next)`: `next > max` ya da `next+1` taşarsa `Exhausted` (sarma yok; `u64::MAX` hiç dağıtılmaz).
- `AppStoreError {Exhausted, Persist, Corrupt, OutboxFull, RecordTooLarge{len}}` — elle yazılmış `Display` + `Error`.
- `DedupWindow {highest: Option<MsgId>, seen: [u64; 64]}`, `EMPTY`/`Default` — depo değeri yorumlamaz.
- `OutboxUnit::new(msg_id, unit)`: kayıt > `MAX_OUTBOX_RECORD_BYTES` → `RecordTooLarge` (bounds-before-store). `Debug` yalnız uzunluk + sıkıştırma politikası.
- `AppStore` trait: `reserve_next` (dönmeden önce kalıcı, aynı değer iki kez yok, hata = ayrılmadı), `dedup_load/dedup_save`, `outbox_put` (aynı `msg_id` yerine geçer; yeni ve dolu → `OutboxFull`), `outbox_remove`, `outbox_list` (artan `msg_id`).
- `MemoryAppStore` (test/geliştirme): `Mutex<MemoryState>`; zehirlenmede `into_inner`; kurallar ürün deposuyla aynı.
- Testler: tel genişliğinde tükenme; eş/tür başına bağımsız sayım; tükenmenin kalıcılığı ve başka eşi etkilememesi; 4 iş parçacığı × 500 çağrıda tekil değer; `OutboxUnit` tavanı; `Debug`'ın baytı basmaması; boş pencere/boş outbox yanıtı.
- Modül belgesi 16–20 "Messenger / Ptt / FileTransfer bu kartta depoya BAĞLANMAZ" bayat (bağlandılar) — davranış değil.
- Uyumlu: 12 (en önemlileri: `reserve_next` D2 sözleşmesi, `u32` tükenmesinde sarma yok — AMP §3 S2; outbox tavanı `MAX_OUTBOX_UNACKED` — S7; bounds-before-alloc — SR-AUDIT-001; gizlilik).

### messaging.rs (1–1744)
Sabitler: `FIRST_MSG_ID = 0` (uygulamanın, spec başlangıç koymaz) · `MSG_DEDUP_WINDOW = 4096` (S7) · `MAX_OUTBOX_UNACKED = 256` (S7) · `MAX_PEERS = 64` ([KALİBRASYON], türetilmemiş) · `MAX_RETIRED_SESSIONS = MAX_PEERS` (türetilmemiş) · `DEDUP_WORDS = 64` (türetilmiş).

- `MessengerEvent {Session, TextReceived, Delivered, Other}`; `Debug` metni değil uzunluğu yazar.
- `ConnectState {Idle, Connecting{ticket, phase}, Ready(h)}` — UI içindir.
- `MessengerError` 8 varyant; `Display`/`Error` elle.
- `MessengerStats` 11 gözlem sayacı (davranışı değiştirmez).
- `Messenger::with_store`: limitleri `capabilities()`'ten bir kez türetir (S1 K5); depoya dokunmaz. `new` yalnız `#[cfg(test)]`.
- `connect(peer)`: `materialize` (eş yoksa tavan denetimi + depodan outbox yükleme; > 256 kayıt → `Corrupt`) → `transport.connect(peer, None)`; `Ready(h)` → `adopt`, `Pending` → `note_pending`.
- `connect_with_text(peer, text)`: materialize → `next_send_id` → encode → `persist_unsent` (depoya önce yaz) → `transport.connect(peer, Some(unit))`; hata → `forget_unsent`. `Pending` → önceki erken metin `release_early` ile outbox'a "gönderilmemiş" geçer, yeni erken metin bilete bağlanır. `Ready` (sözleşme dışı) → outbox'a "gönderilmemiş" + adopt + flush.
- `cancel_connect(peer)`: bekleyen bilet varsa `transport.cancel`, `true`.
- `connect_state(peer)`: Ready > Connecting > Idle.
- `note_pending`: erken metin başka bilete aitse serbest bırak; bilet değiştiyse `Measuring`.
- `send_text(peer, text)`: tanınmayan eş → `PeerTableFull`/`NoSession`; oturum yok → `NoSession`; `next_send_id` (outbox ≥ 256 → `OutboxFull`; yedek `msg_id` ya da `reserve_next`; bekleyenden küçük-eşit değer → `Store(Corrupt)`; en eski bekleyenden fark ≥ 4096 → `UnackedSpanFull`) → encode → `persist_unsent` → `send(h, Reliable, Be)`; red → `forget_unsent`, `Err(Transport)`; kabul → `commit_send` (outbox aynası `sent_on = h`, yedek boşalır).
- `poll`: (1) `flush_resends` — `resend_pending` eşler için güncel tutamaca yeniden sunum, (2) Transport olayları birer birer (`on_session_event` sonra aynen iletilir), (3) birimler; olay üretmeyen birim aynı çağrıda atlanır.
- `on_session_event`: `Ready` → `adopt`; `Closed` → `retire`; `ConnectProgress` → faz güncelle; `ConnectAbandoned` → niyet ve aynı biletli erken metin serbest; `RenewRequired` → yok sayılır (cutover Transport'un; S1 KARAR-2).
- `adopt(h, peer)`: materialize (tavan → `peers_refused++`, depo → `store_errors++`, tutamaç benimsenmez); aynı tutamaç → etkisiz; çözülen bilet erken metnin biletiyse kayıt `sent_on = h` (Transport ilk `send` olarak yollar, K6), değilse `None`; önceki tutamaç emekliye; outbox doluysa `flush_peer`.
- `retire(h)`: güncel tutamaçsa eşin oturumu boşalır; emekli FIFO'ya eklenir (geç teslimler eşe bağlanır).
- `peer_of(h)`: güncel, sonra emekli tutamaçlar.
- `on_unit`: bilinmeyen tutamaç → `units_unknown_session++`; decode hatası (DROP/REJECT) → `records_dropped++`, sinyal yok (AMP §5 kural 3); TEXT → `on_text`; `RECEIPT(delivered)` → `on_receipt`; diğer her geçerli kayıt → `Other`.
- `on_text`: `DedupWindow::admit`; HER durumda `send_receipt` (güncel oturumdan, `Reliable`, `Ctrl`, tek deneme); `New` → `TextReceived`; `Duplicate`/`BelowWindow` → sayaç.
- `on_receipt`: outbox'ta varsa sil (bellek + depo), `Delivered` bir kez; yoksa `receipts_ignored++` ve (bekleyen erken metin değilse) depodan idempotent silme.
- `PeerState::restored` (depodan yükleme, `sent_on = None`, `resend_pending`), `next_send_id`, `commit_send`, `release_early`.
- `materialize`, `reserve_msg_id` (`Exhausted` → `MsgIdExhausted`; `u32`'ye sığmayan → `Corrupt`), `persist_unsent` (`OutboxFull` ayrı; diğer hata → geri alma + `Store`), `forget_unsent` (tek deneme; hata → `store_errors++`).
- `flush_peer`: `msg_id` sırasıyla, `sent_on != h` kayıtları `send(h, Reliable, Be)`; ilk retle durur (`resend_refused++`), kalan bekler.
- `send_receipt`: oturum yok ya da Transport reddi → `receipts_unsent++`, yeniden denenmez.
- `push_retired`: FIFO, tavanda en eski düşer.
- `DedupWindow` (özel, depodakinin ikizi): `admit` — ilk TEXT çapa; büyükse ilerle (≥ 4096 → tümünü sil, değilse düşen konumları temizle); `highest − id ≥ 4096` → `BelowWindow`; işaretliyse `Duplicate`; yoksa işaretle, `New` (sıra zorlanmaz).
- Testler: sırasız kabul + tekrar bastırma; pencere kenarları; kısmi/tam ilerlemede konum temizliği; `u32` uçlarında taşma yok; `msg_id` tükenmesi tipli ve kalıcı; açık aralığın pencere içinde kalması ve reddin değeri yakmaması; bekleyen `msg_id`'yi yineleyen deponun `Corrupt` sayılması ve hiçbir kaydı ezmemesi; tavanı aşan outbox listesinin eş kurmaması; erken metnin yalnız kendi biletiyle "gönderilmiş" sayılması (M3 mutasyonu); eş tablosu tavanı ve çıkarmasızlık; emekli FIFO'su; `Debug` gizliliği.
- Uyumlu: 25 (en önemlileri: eş başına kalıcı `msg_id`, sarma yok — AMP §3 S2/K4G-07; ESKİ `msg_id` ile oturum değişiminde yeniden gönderim, her tutamaca en çok bir kabul — AMP §3 S2, S1 K7/K8; tekrar TEXT'e RECEIPT yeniden — AMP 0x08 S2; RECEIPT Transport `Ok`'undan türemez — S1 K8; `MAX_OUTBOX_UNACKED`/`MSG_DEDUP_WINDOW` — S7; erken metin K6 biletine bağlı; `compression_id = NONE` — AMP §5 kural 6'nın "TEXT → ON"u PoC-KAPSAM-001 §3 "zstd sıkıştırma (`compression_id = NONE`)" sahip kararıyla askıda; RECEIPT(read) üretilmiyor — PoC-KAPSAM-001 §2).

### file_transfer.rs (1–2790)
Sabitler: `FIRST_TRANSFER_ID = 0` · `MAX_REASSEMBLY_BYTES = 64 MiB` (S7) · `FILE_IDLE_TTL = 60 s`, `FILE_RECEIPT_TTL = 60 s` (AMP §3 K4G-09) · `MAX_INCOMING_TRANSFERS_PER_PEER = 2`, `MAX_INCOMING_TRANSFERS = 4`, `MAX_OUTGOING_TRANSFERS = 4` ([KALİBRASYON], türetilmemiş) · `MAX_PENDING_FILE_EVENTS = 4 + 2·4 = 12` (türetilmiş) · `MAX_PENDING_RECEIPTS = MAX_INCOMING_TRANSFERS = 4` (kod "türetilmiş DEĞİL" der) · `MAX_EARLY_ABORTS = MAX_PEERS × 2 = 128` (türetilmiş) · `SEEN_WORD_BITS = 64`.

- `whole_file_hash`: anahtarsız BLAKE2s-256 (`blake2::Blake2s256`), crate'teki tek kullanım.
- `FileFailure {HashMismatch, IdleTimeout}`, `SendFailure` 8 varyant; `from_receipt` eşlemesi (`ok` → `None`).
- `FileEvent {Received, Failed, Aborted, Sent, Delivered, SendFailed}`; `slot()` → Incoming/Outgoing/Progress; `Debug` ad ve bayt basmaz.
- `FileSendError` 9 varyant; `FileTransferStats` 14 sayaç.
- `FileTransfer::new(clock)`; `stats`, `outgoing_count`, `incoming_count`, `poll_event` (FIFO).
- `send_file(m, peer, name, bytes)`: oturum yok → `NoSession`; giden yuva (tablo + son giden olaylar) ≥ 4 → `QueueFull`; boy > 64 MiB → `TooLarge` (özetten önce); `L = max_file_chunk_bytes` (= `MAX_BODY_BYTES − 18`, A2); boş değil ve `L = 0` → `NoChunkRoom`; `FILE_BEGIN` sonda yer tutucularla encode denemesi; ancak sonra `reserve_transfer_id`; özet; `Offering(Begin)`.
- `reserve_transfer_id`: en çok `outgoing.len()+1` deneme; açık bir giden aktarımın değeri atlanır (yanar); aşan → `Store(Corrupt)`.
- `cancel_send(id)`: `Begin` sunulmadıysa tablodan çıkar + `SendFailed(Cancelled)` hemen, tele bir şey yok; `Chunk/End` → `Abort(UserCancel)` + baytlar bırakılır; `Abort` zaten → etkisiz; `AwaitingReceipt` → `AlreadySent`.
- `cancel_incoming(m, peer, id)`: gelen aktarımı bırakır, `FILE_RECEIPT(aborted_by_receiver)` kuyruğa + hemen sunum; olay yok.
- `pump(m)`: `expire_incoming` → `expire_receipt_waits` → `flush_receipts` → tur döngüsü: eş başına en eski sunulan aktarımın sıradaki kaydı bir kez (`offer`); `Sent` ara olay; `Finished` → tablodan çıkar + son olay; kabul yoksa döner.
- `handle(m, ev)`: `FILE_*` taşıyan `Other` tüketilir (`on_record`), yeni alındı varsa hemen `flush_receipts`; diğer olay aynen döner.
- `on_begin`: aynı anahtar → aynı alanlar `duplicate`, farklı alanlar sessiz DROP; tutarsız beyan → sessiz DROP (S8); erken iptal kaydında → `rejected`; boy > 64 MiB / eş başına ≥ 2 / toplam yuva ≥ 4 → `rejected`; yoksa kabul (zamanlayıcı başlar).
- `on_chunk`: bilinmeyen → sayaç; `accept_chunk` `Stored` → `last_chunk_at = now`; tamamsa `finish`.
- `on_end`: tekrar → sayaç; `end_seen`; tamamsa `finish` (süre sıfırlanmaz).
- `finish`: özet tutarsa `FILE_RECEIPT(ok)` + `Received`, tutmazsa `hash_mismatch` + `Failed(HashMismatch)`.
- `on_abort`: izlenen gelen aktarım (neden ne olursa olsun) → `Aborted`; değilse `aborts_unmatched++` + erken iptal kaydı.
- `on_receipt`: yalnız o eşe GİDEN aktarıma `receipt_outcome`; değilse yok sayılır.
- `expire_incoming`: `now − last_chunk_at ≥ 60 s` → yuva boşalır, `timeout` alındısı, `Failed(IdleTimeout)`.
- `expire_receipt_waits`: `AwaitingReceipt` ve `≥ 60 s` → `SendFailed(ReceiptTimeout)`; tele bir şey yok.
- `queue_receipt`: kuyruk ≥ 4 → en eski düşer (`receipts_unsent++`).
- `flush_receipts`: ≥ 60 s eski düşer; oturum yoksa bekler; `send(Reliable, Ctrl)`; geçici ret bekler, kalıcı ret düşer.
- `remember_early_abort` (süresi dolan düşer; eş başına 2, toplam 128, en eski yer açar), `take_early_abort` (kayıt her durumda silinir, ömür 60 s).
- `Outgoing::message/chunk/chunk_step/release_bytes/is_awaiting_receipt/receipt_overdue`.
- `offer`: oturum yoksa bekle; `Chunk/End` ve `began_on ≠ güncel oturum` → `Abort(Superseded)` (`transfers_superseded++`); encode; `send(Reliable, Bk)`; kabul → adım ilerler (`Begin` `began_on`'u yazar; `End` → `AwaitingReceipt{since}` + `Sent`; `Abort` → `Finished`); geçici ret → bekle; kalıcı ret → `Abort` ise iptal olayı, değilse `SendFailed(Transport)`.
- `abort_event`: `Superseded` → `SendFailed(Superseded)`, `UserCancel` → `SendFailed(Cancelled)`, diğer → `None`.
- `receipt_outcome`: `Begin` evresinde ya da `ok` erken → yok say; `AwaitingReceipt + ok` → `Delivered`; `Abort` sürerken başarısız alındı → iptalin kendi olayı; diğer → `SendFailed(neden)`.
- `is_transient`: `Backpressure, UnknownSession, SessionSuperseded, NotConnected` geçici; `EarlyUnitConflict, UnitTooLarge, DeadlineExpired` kalıcı.
- `Declared::is_consistent` (`(size=0) ⇔ (count=0)` ve `count ≤ size`), `fits(L)` (`⌈size/L⌉ = count`).
- `Incoming::learn_chunk_len` (ilk kabul edilen parçadan `L`), `accept_chunk` (boş/taşan/sınır dışı → atılır; ilk kabulde `size` bayt + bit haritası ayrılır; tekrar → `Duplicate`).
- Testler: BLAKE2s("abc") RFC 7693 yanıtı; `handle` geçirgenliği; `L`'nin her ilk parçadan öğrenilmesi; geometri dışı parçalar; üç ret sebebinin tek `rejected` alındısı; alınmamış son olayların yuva tutması ve kuyruk tavanı; `FILE_ABORT`/`FILE_RECEIPT` yön ayrımı; eşler arası adil, eş içi sıralı pompa; geçici/kalıcı ret; erken iptal kaydı tavan ve ömrü; alındı kuyruğu tavan ve ömrü; `transfer_id` tükenmesi; sözleşme dışı depo; `Debug` gizliliği.
- Uyumlu: 35 (en önemlileri: `FILE_ABORT` yalnız göndereninin, `FILE_RECEIPT` yalnız alıcının — AMP §3 K4G-09 "Yön"; beş alındı durumu ve tetikleri; tutarsız `FILE_BEGIN` sessiz DROP — S8; iki zamanlayıcı ve sıfırlama kuralı; oturum değişiminde `FILE_ABORT(superseded)`, bekleyen `user_cancel`'in nedeninin korunması, `FILE_END`'i kabul edilmiş aktarımın alındı beklemesi; chunk retry/ACK/kayıp tespiti/per-chunk MAC yok — AMP §3 "SAHİP OLMADIKLARI"; `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18` — A2; S7 boyutu tahsisten önce; `FILE_BEGIN` öncesi iptalin tele çıkmaması — plan "K4G-09 kart §4 okuması" satırı; erken iptal kaydı → `rejected` — aynı satır (a); `FILE_CHUNK` sıkıştırmasız — §5 kural 6).

## Bulgular

| FID | Tür | Ciddiyet | Konum | Davranış | Otorite | Düzeltme |
|---|---|---|---|---|---|---|
| P1-01 | EKSİK | orta | messaging.rs:120-122, 872-873, 922; store.rs:278-285 | Alıcı dedup penceresi yalnız bellekte; `AppStore::dedup_load/dedup_save` hiçbir motorda çağrılmıyor. Yeniden başlayan alıcı, oturum değişiminde yeniden gönderilen ve önceden teslim ettiği TEXT'i ikinci kez gösterir. | PoC-KAPSAM-001 §2 "Kalıcılık (Rust)": "eş başına `msg_id` / `stream_id` / `transfer_id` sayaçları, `init_counter`, dedup penceresi, outbox" (EK-1 dedup'ı çizmedi); plan D8 v1.1 "`AppStore` … dedup penceresi". | kilitli (karar 3: metin davranışı dondu). Çizim: `materialize`'da `dedup_load`, `on_text`'te `admit` sonrası `dedup_save`; hata yolları `store_errors`. |
| P1-02 | ÖLÜ | düşük | store.rs:160-186, 273-285; messaging.rs:1168-1238 | `store::DedupWindow` ve dedup API'si ürün yolunda kullanılmıyor; Messenger alan alan aynı ikinci bir özel `DedupWindow` tutuyor. | PoC-KAPSAM-001 §2; plan D8 (P1-01 ile aynı). | kilitli (P1-01'e bağlı; tek tip, depo tipine geçiş). |
| P1-03 | TANIMSIZ | orta | messaging.rs:1117-1143, 806-812 | `RECEIPT(delivered)` tek denemedir: oturum yoksa ya da Transport GEÇİCİ reddederse (`Backpressure`) atılır. Gönderende TEXT `sent_on = h` ile kalır ve aynı oturumda bir daha sunulmaz; mesaj bir sonraki oturum değişimine dek "gönderiliyor"da takılır. Aynı motor ailesinde `FILE_RECEIPT` geçici retlerde bekleyip yeniden sunuluyor (file_transfer.rs:1274-1277). | S1 K8: "`send` başarısızsa … yalnız kullanıcıya yansıtır ya da bekler"; AMP §3 0x08 yalnız tekrar TEXT'e yeniden RECEIPT'i tanımlar; geçici retteki RECEIPT davranışı spec'te bulunamadı (aranan: AMP §3, S1 §3/§4, PoC-KAPSAM-001 K8). | kilitli (karar 3). |
| P1-04 | TANIMSIZ | düşük | messaging.rs:1229-1231, 819-822 | Pencere ALTINDAKİ `msg_id` (görülüp görülmediği bilinmez) teslim edilmez ama `RECEIPT(delivered)` alır. Gönderen durumunu yitirdiyse (D4) bu yanlış "teslim edildi"dir. | AMP §3 S2 yalnız "pencerede görülmüş" `msg_id`'yi tanımlar; pencere altı spec'te bulunamadı (aranan: AMP §3, §8, S7). | kilitli (karar 3). |
| P1-05 | TANIMSIZ | orta | messaging.rs:164-172, 709-720, 1010-1031, 1145-1155 | `MAX_PEERS = 64` (türetilmemiş, eş çıkarılmaz): 65. eşin oturumu süreç ömrü boyunca benimsenmez, birimleri sessiz atılır (`units_unknown_session`), RECEIPT gitmez. `MAX_RETIRED_SESSIONS = 64` taşınca eski tutamacın geç birimi atılır. | Spec'te bulunamadı (aranan: AMP §3, S1, PoC-KAPSAM-001, plan D8); kod kendisi `[KALİBRASYON]` der. | kilitli (karar 3). |
| P1-06 | TANIMSIZ | düşük | messaging.rs:958-967 | Gönderen ek geri basınç uygular: en eski RECEIPT'siz `msg_id` ile sıradaki arası ≥ 4096 → `UnackedSpanFull`. Dedup penceresinden türetilmiş, ama yazılı kural değil. | AMP §3 S2/S7 yalnız `MAX_OUTBOX_UNACKED = 256`'yı koyar; app-rpc §8 "İstemci gönderim penceresi: YOK" (2026-09-22) — AMP S7 (2026-10-08) sonraki sahip kararı olduğu için 256 tavanı geçerli; açıklık tavanı ikisinde de yok. | kilitli (karar 3). |
| P1-07 | ÖLÜ | düşük | messaging.rs:453-459, 484-550 | `connect_with_text` ve `unacked_count` ürün yolunda (kama-node) çağrılmıyor; ilk mesaj hiçbir zaman `early_unit` olarak gitmiyor. | PoC-KAPSAM-001 K3 "soğuk INIT = tek datagram, 0-RTT (`early_unit` ilk mesaj)", K4 "`SESSION_INIT_REF` + `early_unit`"; S1 K6 (bu sprint: `Ready` sonrası ilk `send`). | kilitli (karar 3; eksik kablo kama-node'da). |
| P1-08 | TANIMSIZ | düşük | file_transfer.rs:24-29, 790-834, 1553, 1560-1562, 1576-1596 | `FILE_BEGIN` kabul edildikten sonraki iptal, `FILE_ABORT` kabul edildiğinde ya da KALICI reddedildiğinde `SendFailed(Cancelled)` ile biter; oturum değişimi sürerken `cancel_send` etkisiz, `Superseded` kalır. Dayanak koddaki "orkestratör tutarlılık kararı 2026-10-12"dir, sahip kararı değil. (Kabul öncesi iptalin tele çıkmaması plan satırıyla uyumlu.) | AMP §3 K4G-09 gönderendeki iptal olayını tanımlamaz; spec'te bulunamadı (aranan: AMP §3 "Oturum değişimi", plan K4G-09 kartı ve Ek tablo). | karar-gerekir. |
| P1-09 | TANIMSIZ | düşük | file_transfer.rs:91-103, 287-291, 1216-1235 | Bekleyen alındı kuyruğu `MAX_PENDING_RECEIPTS = 4` (kod: "türetilmiş DEĞİL", `MAX_INCOMING_TRANSFERS` yeniden kullanıldı). Taşmada en eski alındı düşer → dosya alıcıdayken göndericide `ReceiptTimeout`. | Spec'te bulunamadı (aranan: AMP §3 K4G-09, §4); plan "K4G-09 alındı kuyruğu `MAX_PENDING_RECEIPTS` kalibrasyonu ve eş başına tahliye" KATMAN 6'ya ertelenmiş. | karar-gerekir. |
| P1-10 | TANIMSIZ | düşük | file_transfer.rs:269-281, 1030-1038, 718-720 | Gelen yuva tavanları 2/eş, 4 toplam; giden 4 — türetilmemiş `[KALİBRASYON]`. Tavan doluyken gelen `FILE_BEGIN` `rejected`, giden `QueueFull`. | AMP §3 yalnız "yuva yok" tetiğini adlandırır; sayı spec'te bulunamadı (aranan: AMP §3, §4, S7). | karar-gerekir. |
| P1-11 | TANIMSIZ | düşük | file_transfer.rs:53-63, 1718-1795 | Alıcı parça uzunluğu `L`'yi ilk kabul ettiği parçadan öğrenir ve bütün parçaların aynı `L` sınırında olmasını şart koşar; uymayan parça sessiz atılır, aktarım `FILE_IDLE_TTL`'de `timeout` ile biter. Başka bir uyumlu göndericinin değişken boylu parçaları reddedilir. | AMP §3 0x03 yalnız `offset`/`payload_len` verir, yerleşim koymaz; spec'te bulunamadı (aranan: AMP §3 0x02/0x03, §4 A2). | karar-gerekir. |
| P1-12 | TANIMSIZ | düşük | file_transfer.rs:1556-1568, 1636-1646 | Geçici/kalıcı `TransportError` ayrımı kodun seçimidir. Kalıcı retle biten yarım giden aktarım eşe `FILE_ABORT` göndermez; alıcı yuvayı 60 s tutar, `timeout` alındısı göndericide yok sayılır. | S1 §3 hata kümesi; kalıcılık sınıflaması ve yarım aktarımın kalıcı retteki kapanışı spec'te bulunamadı (aranan: S1 §3/§4, AMP §3 K4G-09). | karar-gerekir. |
| P1-13 | TANIMSIZ | düşük | file_transfer.rs:1507-1515, 708-762 | Oturumu olmayan eşe kuyruklanmış giden aktarım (`Offering`) süresiz bekler ve 4 giden yuvadan birini tutar; zamanlayıcı yalnız `FILE_END` kabulünden sonra başlar. | AMP §3 K4G-09 yalnız iki zamanlayıcıyı tanımlar; `Offering` evresi için süre spec'te bulunamadı. | karar-gerekir. |
| P1-14 | TANIMSIZ | düşük | file_transfer.rs:139-153, 293-295, 1298-1327 | Erken iptal kaydının tavanı (2/eş, 128 toplam) ve ömrü (`FILE_IDLE_TTL` yeniden kullanımı) koddan türetilmiş; taşmada geç `FILE_BEGIN` hayalet aktarım açar (≤ 60 s). Kural (BEGIN'den önce gelen ABORT → `rejected`) sahip satırıyla uyumlu; sayılar değil. | Plan "K4G-09 kart §4 okuması" (a); AMP §3 "rejected … `FILE_ABORT`'u `FILE_BEGIN`'inden önce geldi"; tavan/ömür spec'te bulunamadı. | karar-gerekir. |
| P1-15 | ÖLÜ | düşük | file_transfer.rs:836-853 | `cancel_incoming` ürün yolunda çağrılmıyor (yalnız testler); `FILE_RECEIPT(aborted_by_receiver)` üründe hiç üretilmez. | AMP §3 0x0B `aborted_by_receiver` = "alıcının kullanıcısı kabul edilmiş aktarımı iptal etti". | karar-gerekir (kama-node API'sine eklemek K4G-11 API listesini değiştirir). |
| P1-16 | DOĞRULANAMADI | düşük | messaging.rs:637, 1104, 1134; file_transfer.rs:1265, 1529 | Tip → `TrafficClass` eşlemesi: TEXT `Be`, RECEIPT `Ctrl`, `FILE_RECEIPT` `Ctrl`, `FILE_*` (ABORT dahil) `Bk`. | S1 YAMA 2026-11-09 (EK-1 D-08/D-09) yalnız sınıf kümesini koyar; tip başına eşleme bir tasarım/iş emri belgesindedir (KK-T1-03/04); sahibin tarihli metni depoda bulunamadı. | karar-gerekir (yalnız doğrulama). |
| P1-17 | DOĞRULANAMADI | düşük | file_transfer.rs:306-314; Cargo.toml 21-25 | `blake2` kama-app'te: "DAR istisna, 2026-10-08" metni yalnız `layer_check.py` ve `Cargo.toml` yorumunda alıntı; ADR-03 AMENDMENT-2 BLAKE2s üretim yerini `kama-noise-wire` diye yazar. Primitif registry'de (pki §13). | ADR-03 AMENDMENT-2; pki §13 satır 456-457; sahip kararı docs/decisions altında bulunamadı. | karar-gerekir (istisnanın KARAR-KAYDI'na işlenmesi). |
| P1-18 | UYUŞMAZ | düşük | store.rs:119-158; messaging.rs:262-326; file_transfer.rs:511-585 | Hata tipleri elle yazılmış `Display` + `Error`; `thiserror` kullanılmıyor (kama-app bağımlılığı değil). Davranış aynı. | CLAUDE.md §9 "açık hata tipleri (thiserror)". | kod-duzeltmesi (davranış değişmez; messaging kısmı metin davranışına dokunmaz). |

## Okunan otorite bölümleri

- `specs/application-message-protocol.md` 1–142 (tamamı: YAMA KAYITLARI, §1–§8; özellikle §3 tablo, `msg_id` semantiği, S2 bloğu, K4G-09 bloğu, §4 A2, §5 kural 1–7).
- `specs/application-protocol-and-rpc-spec.md` başlık dizini; 1–40 kapsam yamaları; §7 (449–497); §8 (501–535); §14 notu (669–681).
- `docs/seams/KM-SEAM-AT-001.md` YAMA KAYITLARI (9–27), §1 imzalar (50–73), K2/KARAR-1/KARAR-2 (82–91), K3–K6 (93–113), K7 (115–119), K8 (122–126), §3/§4 (140–155), §6 H-tablosu satırları.
- `docs/decisions/PoC-KAPSAM-001.md` tamamı (1–106).
- `docs/decisions/KARAR-KAYDI-2026-11.md`, `DR-CONNECT-1.md`, `SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` — kama-app ile ilgili satırlar (arama).
- `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md` D5, D6, D8, K4G-06/07/09 kartları, Ek tablo (satır 48–96, 221–279, 392, 482–534).
- #175 sahip kararları: 7171705736 (dokuz karar; karar 3 "PTT ve metin davranışını dondurun … metin gönderim politikasında yeni değişiklik yapmayın"), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (bu dosyalara doğrudan madde yok; karar 3 ve "PTT/metin davranışı … sabit" cümlesi messaging bulgularını kilitler).
- `specs/tech-stack-adr.md` ADR-03 ve AMENDMENT-1/2 (61–112); `specs/pki-ca-ra-hsm-tdd.md` §13 (450–457).
- CLAUDE.md §4, §5, §8, §9, YAMA KAYDI.
- `docs/design/T1-SERITLER.md` sınıf eşleme tablosu (21–40, 102–110) — tasarım belgesi, otorite sayılmadı.
