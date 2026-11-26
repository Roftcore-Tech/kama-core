# H-hub-store — `hub-server` ve `kama-store` kod denetimi

Taban: ürün hattı `125b9afa`. Satır numaraları bu tabandandır. Salt okuma yapıldı; kaynak dosyalara yazılmadı, cargo çalıştırılmadı.

## Kapsam

Okunan kod dosyaları. Her satır Read aracıyla okundu:

| Dosya | Satır aralığı | Satır |
|---|---|---|
| `crates/hub-server/src/lib.rs` | 1–420 | 420 |
| `crates/hub-server/src/main.rs` | 1–268 | 268 |
| `crates/hub-server/src/config.rs` | 1–482 | 482 |
| `crates/hub-server/src/wire.rs` | 1–919 | 919 |
| `crates/hub-server/src/bind.rs` | 1–647 | 647 |
| `crates/hub-server/src/table.rs` | 1–748 | 748 |
| `crates/hub-server/src/relay.rs` | 1–357 | 357 |
| `crates/hub-server/src/rate.rs` | 1–132 | 132 |
| `crates/hub-server/src/stats.rs` | 1–525 | 525 |
| `crates/hub-server/src/probe.rs` | 1–380 | 380 |
| `crates/kama-store/src/lib.rs` | 1–673 | 673 |
| **Toplam** | | **5 551** |

Bağlam için ayrıca okundu (envantere girmedi): iki crate'in `Cargo.toml`'u, `kama-contracts/src/s4.rs` başlığı ve genel API listesi, `kama-app/src/store.rs` `AppStore` trait'i, `kama-noise-wire` `verify_strict` başı, redb 4.3.0 `Durability` varsayılanı (`InternalDurability::Immediate`, transactions.rs:982).

## Davranış envanteri

### `lib.rs` — IO'suz hub çekirdeği

- **Lint kapısı (28–40).** `forbid(unsafe_code)`. Test dışında `indexing_slicing`, `unwrap/expect`, `panic`, `unreachable`, `todo`, `unimplemented` yasak. Dayanak S4 H12 ("hiçbir girdi hub'ı düşürmez") ve CLAUDE.md §6 (reachable `todo!()` yok).
- **`Clock` / `SystemClock` (71–100).** Monoton saat, `Instant::elapsed`. Cookie dakikası, kovalar, tombstone ve kilit süreleri yalnız bu saatten okunur.
- **`EntropyUnavailable` (103–112).** Açılışta sır üretilemezse tipli hata.
- **`Sink` (115–120).** `send` ve `log`. Kuyruk yok; gönderilemeyen datagram düşer.
- **`Directory` (123–168).** `hub.json` cihazları, `cert_ref → Device` ve `dest_hash → Device` indeksleri. Açılıştan sonra değişmez (S4 H2).
- **`Hub::new` / `with_limits` (194–227).** `Limits::S4` ile kurar. Cookie sırrı ve handle sırrı 32'şer B rastgeledir. Tamponlar `max_datagram` kapasiteyle bir kez ayrılır. `with_limits` genel API'dir ve "yalnız test" diye belgelenmiştir; ürün ikilisi `new`'i çağırır.
- **`on_datagram` (230–246).** Boy `>1280` ya da `<5` ise `drop_size`. Sonra `classify`: veri → `on_data`; kontrol `0x1B` → heartbeat; `0x1D` → RPC; başka tip → `drop_unknown_frame`; çözme hatası → `drop_malformed`. Uyar: S4 H1, TDD §3 `recv`.
- **`on_rpc` (248–264).** `flags ≠ 0` ya da bit7'li `method_id` → `drop_malformed`. `0x01`, `0x0B`, `0x05` dağıtılır; başkası `drop_unknown_method` alır ve cevapsız kalır.
- **`on_tick` (268–290).** 60 s'de bir tombstone ve kilit süpürmesi, saatlik cookie yenilemesi, 10 s'de bir `hub stats` satırı. Datagram yollamaz (S4 H10).
- **`reply_rpc` / `reply_error` / `reply_frame` / `encode_rpc` / `reply_control` (323–419).** Yanıt aynı `method_id` ve `request_id` ile gider, `flags` 0x01 ya da 0x03'tür. Hata yükü `ErrorPayload{code, 0}`'dır. Bind ACK'i saklanmak için ayrı bir vektöre kodlanır.

Uyumlu: 9 davranış. En önemlileri: demux sırası (H1), `flags` ve bit7 reddi (T-R4), datagramsız zamanlayıcı (H10).

### `main.rs` — ikili

- **Sabitler (30–37).** `TICK = 1 s` okuma zaman aşımıdır. `ERROR_BACKOFF = 50 ms`. `MAX_CONSECUTIVE_FAILURES = 200`: yaklaşık 10 s sürekli soket hatasından sonra çıkış kodu 1. Çıkış kodları: 1 hata, 2 kullanım.
- **`UdpSink` (39–65).** `send_to` hatasında yeniden deneme yok. Hata sayılır, en çok `STATS_PERIOD`'da bir satırla loglanır ve satır hedef `ip:port`'u taşır. `log` stdout'a yazar; yazılamayan satır düşer, panik yok.
- **`flags` / `parse_args` (79–119).** `--config` ve isteğe bağlı `--listen`; `probe --device` ve isteğe bağlı `--hub`. Bilinmeyen, yinelenen ya da değersiz bayrak kullanım hatasıdır.
- **`run_probe` (132–164).** `--hub` yoksa cihaz dosyasındaki `hub.addr` kullanılır. DNS çözümü `to_socket_addrs` ile yapılır. Sonuç `println!` ile basılır.
- **`serve` (166–243).** Yapılandırma okunamazsa tek satır yazılır ve çıkış 1'dir (T-C3). `--listen` hub.json adresini ezer. Döngü tek iş parçacıklı ve engelleyicidir: `recv_from` → `on_datagram` → `on_tick`. Tampon `MAX_DATAGRAM + 1` B'tır; daha büyük datagram kesik gelir ve boyundan tanınır.
- **Test (245–268).** Gönderim hatasının sayıldığını ve aralık başına bir kez loglandığını sabitler.

Uyumlu: 5 davranış (T-C3, tek iş parçacığı ve tek soket, sınır+1 tampon, kullanım kodu, yeniden denemesiz röle).

### `config.rs` — `hub.json` ve S4 §6 sabitleri

- **`Limits::S4` (75–93).** Değerler: `MAX_DATAGRAM` 1280 · `MAX_LINKS` 8192 · `MAX_LINKS_PER_ENDPOINT` 64 · `MAX_TOMBSTONES` 65 536 · `HANDLE_TOMBSTONE` 7 gün · `COOKIE_ROTATE` 1 saat · `VERIFY_BUDGET` 2000/s, patlama 2000 · `BIND_FAIL` 10 / 60 s / 4096 · `STORE_RATE` 10/s, patlama 20 · `LINK_ALLOC_RATE` 1/s, patlama 10 · `ROUTE_MISS_RATE` 1/s, patlama 1 · `STATS_PERIOD` 10 s · `sweep_period` 60 s. Hepsi S4 §6 ve TDD §3'te var. İki istisna türetilmemiştir: `VERIFY_BUDGET` patlaması ve `ROUTE_MISS` patlaması. İkincisi kod yorumunda gerekçelidir.
- **`HubSecret` (102–123).** 32 B tohum. `Debug` içeriği gizler, Drop sıfırlar.
- **`Device`, `Config`, `ConfigError` (126–182).** Hata metni dosya içeriği taşımaz.
- **`load` (222–232).** Yalnız G/Ç hatası `Unreadable`'dır. UTF-8 olmayan dosya `Invalid`'dir. Okunan baytlar sıfırlanır.
- **`parse` (238–309).** `deny_unknown_fields`. `format_version` 1 olmalı. `listen` bir SocketAddr olmalı. hex tam 64 küçük harf olmalı. `pub` değeri `priv`'den türemeli. `cert` kanonik base64 ve 152 B olmalı. Cihaz anahtarı `cert[0..32]`'dir ve katı kipte kullanılabilir olmalıdır. `dest_hash` ve `cert` yinelenemez. `cert_ref = SHA-256(cert)`. Boş `devices` listesi ve yinelenen `name` reddedilmez.
- **`usable_public_key` (314–322).** Sıfır imzayla `verify_strict`'e sorar. Anahtar kaynaklı ret kodları kullanılamaz sayılır.
- **`hex_decode` / `base64_decode` (325–398).** Katı ve kanonik çözücüler; panik yok.
- **Testler (400–482).** Base64 kanonikliğini, hex katılığını, açık anahtar kuralını ve `Debug` gizlemesini sabitler.

Uyumlu: 9 davranış (T-C1, T-C2, T-C3, S4 §6 değerleri, H2 türetimleri).

### `wire.rs` — S4 §2 codec'i (yerel kopya)

- **Sabitler (21–102).** `MAX_DATAGRAM` 1280 · `CONTROL_MIN_LEN` 21 · `DATA_MIN_LEN` 5 · `RPC_OVERHEAD` 31. Frame tipleri `0x1B`, `0x1C`, `0x1D`. Method'lar `0x01`, `0x05`, `0x0B`, `0x81`, `0x84`. Durum ve hata kodları `0x0000`, `0x0003`, `0x0105`, `0x0106`, `0x0301`, `0x0302`, `0x0501`, `0x0502`, `0x0601`. `RELAY_OPEN_MAX_ENVELOPE` 64. Değerler S4 §2.2–§2.5 ile aynıdır.
- **`classify` (152–168).** İlk 4 bayt sıfır değilse veridir ve yüke bakılmaz. Sıfırsa sondaki 16 B sıfır olmalı ve datagram en az 21 B olmalıdır.
- **`RpcFrame::decode/encode` (196–232).** `payload_len` kalan baytla tam tutmalı ve 1249'u aşmamalıdır.
- **Yük codec'leri (254–495).** `ClientBindRequest` sabit alanları denetler (`mode = 0`, `presence = 0`). `ClientBindResponse` ve `BindProofResponse` sabit alanları denetler. `ErrorPayload` için `detail_len = 0`. `StoreRequest` ve `Deliver` sondaki uzunluğu tam ister. `StoreResponse`, `RouteMiss`, `ErrorPayload` ve `Deliver` çözücüleri durum kodunu, `reason`'ı, handle 0'ı ve `envelope_type`'ı denetlemez (H-2 U-1…U-4).
- **TBS ve cookie birleştiricileri (505–598).** TBS boyları 112 B ve 83 B. Cookie girdisinde `fam` 4 ya da 6'dır ve IPv4-eşlemeli adres normalize edilir. Handle girdisi `secret ‖ counter_be64`'tür.
- **`AllocationKey` (604–611)** ve **`concat` (615–624).** `concat` sabit boya doldurur, panik yok.
- **Testler (695–919).** Boyları, sınıflamayı, uzunluk retlerini, sabit alanları ve bayt düzenlerini sabitler.

Uyumlu: 8 davranış (bütün bayt düzenleri S4 vektörleriyle eşit; H-2 belgesinde 93/93 pozitif vaka).

### `bind.rs` — bind ve heartbeat

- **`CookieJar` (44–129).** Geçerli ve önceki sır tutulur. `rotate_if_due` süre dolunca yeni sır üretir; rastgelelik yoksa eskisi kalır ve sessizce `false` döner. `issue` bu dakikanın cookie'sini üretir. `check` 2 dakika × 2 sır = en çok 4 BLAKE2s hesaplar, erken çıkmaz, sabit zamanlı karşılaştırır.
- **`cookie` (138–153).** `BLAKE2s-256(cookie_input)[0..16]`.
- **`BindFailTable` (163–250).** Kaynak adres başına kayıt, LRU, `BIND_FAIL_ENTRIES` tavanı. Sayım penceresi ilk başarısızlıktan `BIND_FAIL_HOLD`'dur. Sınıra ulaşıldığı an kilit süresi baştan başlar. Süpürme süresi dolanı siler.
- **`HubIdentity` (253–294).** Ed25519 ile ACK TBS'ini imzalar (`ed25519-dalek`). `Debug` içeriği gizler.
- **`on_heartbeat` (306–325).** Gövde 0 ya da 4 B olmalıdır, değilse `drop_malformed`. Kaynak bağlı değilse `hb_unbound` sayılır ve cevap yoktur. Bağlıysa `last_activity` tazelenir ve aynı gövdeyle `0x1C` döner (H8).
- **`on_bind1` (328–348).** Dizinde olmayan `cert_ref` sessiz kalır. Ucun epoch'u okunur ve cookie döner. Hiçbir şey yazılmaz (H3).
- **`on_bind2` (351–461).** Sıra: (1) dizin, sessiz → (2) kayıp ACK tekrarı, bayt-özdeş → (3) cookie[epoch], yanlışsa `BAD_COOKIE` ve durum yazılmaz → (4) adres kilidi sessiz, ardından küresel bütçe (`RATE_LIMITED`) → (5) `verify_strict`, yanlışsa `bind_fail.hit` ve `POSSESSION_PROOF_FAIL` → (6) `accept_bind`, imzalı ACK, `bind ok dh=<8 hex> epoch=` logu. S4 §2.4(b) 1–6 ve TDD `on_bind2` ile birebir.
- **Testler (464–647).** T-B13 normalizasyonunu, cookie girdisinin bağını, 2 dakikalık pencereyi, önceki sırla kabulü, kilit tavanını ve LRU'yu, ACK TBS'ini sabitler.

Uyumlu: 11 davranış (H3, H8, H11, KR-5, KR-11, T-B4, T-B7, T-B8, T-B8b, T-B11, T-B13).

### `table.rs` — tablolar ve emeklilik planlayıcısı

- **Tipler (29–111).** `LastAccepted`, `Endpoint` (`addr: Option`, `bind_epoch`, `link_count`, üç kova), `Link` (`a`, `b`, `last_activity`, `key`), `Tombstone`, `BindChange`, `KeyState`, `Plan = Vec<u32>`.
- **`Table` (114–135).** Yedi HashMap ve `alloc_counter`. `Debug` sırrı ve sayacı basmaz.
- **Okuyucular (155–248).** `capacity_snapshot` HUB_CAPACITY'nin durumu değiştirmediğini kanıtlamak için var.
- **`check_invariants` (254–308).** `by_addr ⇔ endpoints`, `link_count`, tavanlar, `by_key = links`, `tomb_by_key = tombstones`, aktif handle tombstone'da değil ve 0 değil.
- **`accept_bind` (315–358).** Uç yoksa oluşturulur. Adres başka kimlikteyse o ucun adresi düşer (`displaced`). Eski adres taşınır (`moved`). `bind_epoch` `wrapping_add(1)` ile artar. `last_accepted` yazılır. Adresi değiştiren tek yol budur (H3, H4).
- **`key_state` (364–372).** Aktif / emekli / yok.
- **`plan_new_link` (376–412).** S4 §2.6'nın 4–7. adımları. Alıcı kotasında yalnız ortak hat seçilir, yoksa `None` döner. Ardından gönderen kotası, küresel tavan ve küme kuralı. Denetimde tombstone sığası `≤ MAX_TOMBSTONES`. Durum değiştirmez.
- **`pick_handle` (416–428).** `BLAKE2s(secret ‖ counter)[0..4]`. Her denemede sayaç ilerler. 0, aktif ve tombstone'daki değer atlanır. 8 denemede bulunamazsa `None` döner.
- **`commit_new_link` (432–459)**, **`retire` (462–480)**, **`sweep` (484–498).** Tombstone 7 günden önce silinmez. Zamanla silinen tek kayıt tombstone'dur.
- **`oldest_idle` (502–508).** Seçim ölçütü `(last_activity, handle)` en küçüğüdür, belirlenimlidir.
- **Testler (530–748).** Taşıma ve epoch artışını, gönderen ve alıcı kotasını, küresel tavanı ve küme kuralını, dolu tombstone'da değişmezliği, T-M5'i ve 7 günlük süreyi sabitler.

Uyumlu: 12 davranış (KR-9, KR-19, H3, H9, §2.6 adım 3–8, T-M1/1b/1c/1d, T-M4, T-M5, T-M9, T-S15).

### `relay.rs` — zarf rölesi ve veri

- **`valid_combination` (26–31).** Geçerli birleşimler `(0, 0x01)`, `(0, 0x05)` ve `(h≠0, 0x02)`'dir (§2.4(c)).
- **`envelope_digest` (35–42).** `BLAKE2s(envelope)[0..8]`.
- **`on_store` (46–85).** Sıra: bağsız kaynak sessiz (`src_unbound`) → `last_activity` → `STORE_RATE` → çözme → geçersiz birleşim / RELAY_OPEN > 64 B / alıcı dizinde değil / alıcı kendisi → `BAD_REQUEST`.
- **`store_new_link` (88–167).** §2.6'nın 1–8. adımları: aktif anahtar → aynı h ve yeniden DELIVER (alıcı bağsızsa PEER_UNBOUND); emekli anahtar → `{UNKNOWN_HANDLE, 0}`; alıcı bağsız → PEER_UNBOUND; `LINK_ALLOC_RATE`; plan ve aday; uygula; `link open` logu; `{OK, h}`; DELIVER.
- **`store_existing_link` (170–199).** Hat yok → `UNKNOWN_HANDLE`. Gönderen ya da alıcı karşı uç değil → `BAD_REQUEST`. Karşı uç bağsız → PEER_UNBOUND. Değilse DELIVER ve `{OK, h}` (H6).
- **`on_data` (202–257).** Hat yoksa ve kaynak bağlıysa hız sınırlı `ROUTE_MISS{h, 0x01}`, değilse `drop_unknown_link`. Kaynak bağsız → `src_unbound`. Hattın ucu değil → `src_mismatch`. Karşı uç bağsız → `drop_peer_unbound`. Datagram bayt-özdeş iletilir (H4, H7).
- **`deliver`, `reply_store`, `peer_unbound`, `bad_request`, `send_hub_rpc` (259–330).** Hub'ın başlattığı mesajlarda `flags = 0` ve `request_id = 0`.
- **Testler (333–357).** Üç birleşimi ve özet tanımını sabitler.

Uyumlu: 10 davranış (H4–H7, KR-3, KR-4, KR-15, KR-19 tablosu, H10 yansıtma boyları: ROUTE_MISS 36 B, meydan okuma 66 B ≤ 81 B, BIND hata 35 B ≤ 159 B).

### `rate.rs` — token bucket

- **`Rate` / `TokenBucket` (13–80).** Jetonlar ×1000 tamsayıdır. Dolum ms çözünürlüklüdür, `burst` ile sınırlıdır. Geri giden saat jeton üretmez. Reddedilen `take` bir şey harcamaz. Taşma doyurulur.
- **Testler (82–132).** Patlamayı, doğrusal dolumu, sığayı ve saatin geri gitmesini sabitler.

Uyumlu: 3 davranış (TDD §4 token bucket, S4 §6 oranları).

### `stats.rs` — sayaçlar ve loglar

- **`ShortId` (34–55).** `dest_hash`'in ilk 4 baytı, 8 hex.
- **`Stats` (58–101).** 33 sayaç; `bump` ve `add` sarar.
- **`COUNTERS` (117–142).** Satıra giren 24 sayaç: TDD §5 listesi ve üç eklenti (`bind_moved`, `bind_displaced`, `rate_verify`). `rx`, `tx_reply`, `tx_forward`, `relay_*`, `verify_calls`, `bad_request`, `drop_unknown_method` ve `drop_peer_unbound` satıra girmez. `relay_*` satıra yalnız hız (`relay_pps`, `relay_bps`) olarak girer.
- **`RateWindow::emit` (161–205).** Sarma güvenli fark alır ve ortalama pps/bps üretir.
- **`SendErrors` (243–271).** Toplam ve bastırılan sayaçları tutar, en çok aralıkta bir satır.
- **Olay satırları (287–303).** `bind ok dh= epoch=` · `link open h= a= b= type=` · `link retire h=` · `cookie rotate`.
- **Testler (305–525).** Alan sırasını, tablo ile okuyucu eşlemesini, sarmayı, gönderim hatası kısmasını ve olay satırı biçimini sabitler.

Uyumlu: 5 davranış (TDD §5 satırının ilk 28 alanı sırasıyla, T-L1'in yük ve anahtar kısmı, sarma).

### `probe.rs` — `hub-server probe`

- **Sabitler (33–36).** `ATTEMPTS = 3`, `WAIT = 1 s`, `HEARTBEATS = 3` (S4 C2).
- **`ProbeDevice`, `ProbeError`, `Outcome` (39–123).** Çıktı `bind ok rtt=… ms · heartbeat n/3`. Yeşil sayılması için 3/3 heartbeat gerekir.
- **`parse_device` (138–171).** Gevşek JSON okur, bilinmeyen alanları yok sayar (TDD §5). `ed25519.priv`, `cert` ve `hub.ed25519_pub` okunur. Özel anahtar sertifikadaki anahtarla eşleşmelidir. `format_version` denetlenmez.
- **`run` (177–219).** Bağlı UDP soketi açar. Bind `BAD_COOKIE` ya da `RATE_LIMITED` alırsa en çok 3 deneme, aralarında 1 s. Diğer hata kodlarında hemen kırmızı. Ardından 3 heartbeat, token `request_id + n`.
- **`bind_once` (222–275).** Rastgele `request_id` ve nonce. İki tur. Hub imzası `verify_strict` ile denetlenir (C2). Yanıt çözülemezse `Timeout(step)` döner.
- **`exchange` / `heartbeat` / `receive` (278–357).** Yeniden gönderim 3 kez, 1 s arayla. Eşleşme `method_id` ve `request_id` ile yapılır.
- **Test (359–380).** Soket hatası türünün korunmasını sabitler.

Uyumlu: 6 davranış (S4 C2'nin 3 × 1 s kuralı, hub imzası doğrulaması, BAD_COOKIE'de 1. tura dönüş, Deploy TDD çıktı satırı, kendi cihazıyla çalışma notu, alan listesi).

### `kama-store/src/lib.rs` — redb deposu

- **Şema v0 (24–39, 79–113).** Dört tablo: `app_counters` (anahtar `dest_hash ‖ tür`), `init_counter`, `dedup` (517 B), `outbox` (`dest_hash ‖ msg_id_be`). Anahtarlar sabit genişlikli big-endian'dır (ADR-05). Dosyada şema sürümü işareti yoktur.
- **`StoreOpenError`, `open`, `with_database` (116–174).** Kilitli dosya `AlreadyOpen` hatası verir. Tablolar tek işlemde hazırlanır.
- **`AppStore::reserve_next` (177–195).** Tek yazma işleminde oku → `advance` → yaz → commit. Değer yalnız commit `Ok` olduktan sonra döner. Taşmada `Exhausted` ve geri alma.
- **`dedup_load` / `dedup_save` (197–216).** Kayıt yoksa `EMPTY`. Boy tam olmalı. Kanonik olmayan "yok" bozuk sayılır.
- **`outbox_put` (218–233).** Tavan denetimi ve ekleme aynı işlemdedir. Yeni `msg_id` için en çok 256 kayıt, aşımda `OutboxFull`. Aynı `msg_id` yerine yazılır.
- **`outbox_remove` (235–250).** Kayıt yoksa commit yapılmaz.
- **`outbox_list` (252–272).** `msg_id` artan sırayla döner. 256'dan fazla kayıt `Corrupt` sayılır ve ayırmadan önce reddedilir (SR-AUDIT-001).
- **`CounterStore::reserve_next` (276–293).** `u64::MAX` dağıtılmaz. Commit'ten sonra döner.
- **Yardımcılar (298–431).** Anahtar kurucular; `held_units` sayımı tavanda keser; kodlama ve çözme. Boy kopyalamadan önce denetlenir.
- **Testler (433–673).** Tükenmenin kalıcı olduğunu, commit hatasında değer dönmediğini ve `fail-closed` davranışını, v0 dışı satırın panik değil `Corrupt` olduğunu, dedup'ın kanonik big-endian kodlamasını sabitler.

Uyumlu: 10 davranış. En önemlileri: PoC-KAPSAM-001 §2 "Kalıcılık (Rust)" dört kalemi ve EK-1 (kimlik ve eş dizini depoda yok). EK-2'ye göre at-rest şifreleme yok; bu kabul edilmiş PoC riskidir. ADR-13 "redb, trait arkasında" (`AppStore`, `CounterStore`). ADR-05. SR-AUDIT-001. Commit sonrası dönüş: redb 4.3.0 varsayılan dayanıklılığı `Immediate` olarak kaynaktan doğrulandı.

## Bulgular

Sıra önce önem derecesine göre (yüksek → düşük), aynı derecede bulgu numarasına göredir. `kilitli` sınıfına giren bulgu yok: bu dosyalar PTT/metin davranışına, `receiver.rs`'e ya da #262'ye dokunmuyor.

| fid | tür | önem | yer | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| H-HS-01 | UYUŞMAZ | orta | wire.rs:64, lib.rs:260 | `BIND_PROOF` kodu `method_id 0x0B`. | app-rpc §4: "0x0B REVOCATION_STATUS freshness sorgu". S4 Y3 YAMA'sı spec'e işlenmemiş; app-rpc'de YAMA KAYDI yok. | karar-gerekir (tel biçimi) |
| H-HS-02 | UYUŞMAZ | orta | wire.rs:87–97 | `RATE_LIMITED = 0x0502`, `HUB_CAPACITY = 0x0501`, `BAD_REQUEST = 0x0601`, `PEER_UNBOUND = 0x0301`, `UNKNOWN_HANDLE = 0x0302`. | app-rpc §11'de 0x0201 RATE_LIMITED, 0x0501 REPLAY, 0x0502 ROLLBACK, 0x0601 LICENSE_EXPIRED, 0x0301 ROUTE_MISS, 0x0302 RECIPIENT_UNKNOWN. §12: "Aşım → 0x02xx error sınıfı". S4 Y6 işlenmemiş. | karar-gerekir (tel biçimi) |
| H-HS-03 | UYUŞMAZ | orta | wire.rs:254–495 | STORE isteğinin başında `hub_handle u32` var. DELIVER'ın başında `hub_handle` ve `sender_dest_hash` var. ROUTE_MISS `{hub_handle u32, reason u8}`. CLIENT_BIND `client_nonce` / `hub_nonce` / `CHALLENGE` taşıyor. STORE başarısı `0x0000`. | app-rpc §6.1: istek `envelope_type · recipient · len · envelope`, durum `0x0001 ACCEPTED_QUEUED`. §6.11: DELIVER `{envelope_type; envelope_len; envelope}`, ROUTE_MISS `{link_ref bytes16}`. §5 CLIENT_BIND. S4 Y2–Y5 işlenmemiş. | karar-gerekir (tel biçimi) |
| H-HS-04 | UYUŞMAZ | orta | wire.rs:57, lib.rs:241 | Plane 1 RPC `frame_type 0x1D PLANE1_RPC` içinde taşınıyor. | engine-spec §1.1 registry'sinde 0x1D yok; listede yalnız 0x10–0x14, 0x1B, 0x1C ve 0x20 var. S4 Y1 işlenmemiş. | karar-gerekir |
| H-HS-05 | UYUŞMAZ | orta | relay.rs:170–199, table.rs:432–459 | Tek hat iki yönde aynı handle'ı kullanıyor. RESPONSE mevcut hatta gidiyor ve yeni handle verilmiyor. | app-rpc §6.1: "SESSION_INIT_RESPONSE kabul edilince yanıtı alan hub DÖNÜŞ legi için KENDİ hub_handle'ını atar … Her leg bağımsız hub-local handle". routing §2.1 DURUM C. S4 KR-2 bunu taslak YAMA ile değiştiriyor. | karar-gerekir |
| H-HS-06 | UYUŞMAZ | orta | table.rs:416–428, 521–528 | Handle `BLAKE2s(handle_secret ‖ alloc_counter)[0..4]` ile üretiliyor. `hub_handle_epoch` yok. 8 başarısız deneme `HUB_CAPACITY` veriyor. | ccr §35.7: "`hub_handle = Trunc32(PRF(hub_secret, internal_session_id ‖ hub_handle_epoch ‖ allocation_attempt))` … attempt > 8 → hub_handle_epoch rollover VEYA 8B handle profili … yüksek collision pressure → hub_handle_epoch rollover ZORUNLU". | karar-gerekir |
| H-HS-07 | UYUŞMAZ | orta | relay.rs:211–229 | Bilinmeyen ya da emekli handle'a bağlı kaynaktan veri gelince `ROUTE_MISS` dönüyor (1/s/uç). | PoC-KAPSAM-001 K5 (sahip, KESİN): "Bilinmeyen handle → sessiz drop." S4 Y9 bunun için "EK-4" öneriyor, ama PoC-KAPSAM-001'de EK-4 yok (yalnız EK-1…EK-3). İstemci C5/C7 bu mesaja dayandığı için kod tek başına geri alınamaz. | karar-gerekir |
| H-HS-08 | UYUŞMAZ | orta | stats.rs:291–295, relay.rs:158–164 | `link open h=… a=<4 B dest_hash> b=<4 B dest_hash> type=` her yeni hatta journald'a yazılıyor. Bu, kim-kimle konuştuğunun kalıcı kaydıdır; küçük dizinde 4 B önek cihazı tek başına belirler. | identity E-7: "Hub'lar kim-kimle metadata DEFAULT tutmaz (minimize)". TDD §3 yalnız `log("link open h=…")` diyor; `a=`/`b=` alanları kodun eklentisi. | kod-duzeltmesi |
| H-HS-09 | UYUŞMAZ | orta | wire.rs:1–6, 287–495; Cargo.toml (kama-contracts bağlı değil) | Hub kendi S4 codec'ini kullanıyor. Kodun kendi koşulu ("`kama-contracts::s4` main'e girene dek") sağlandığı halde geçiş yapılmamış: `s4.rs` şimdi var ve `client-core` onu kullanıyor. Çözücüler `StoreResponse`, `ErrorPayload`, `Deliver` ve `RouteMiss`'te gevşek (H-2 U-1…U-4 açık). | TDD §5: "`wire.rs` (yalnız `kama_contracts::s4` codec'ini çağırır)". S4 KR-12 bağımlılığı `kama-contracts`. ADR-05: "codec crate tek kaynak: hub-server, client-core … aynı crate'i kullanır". | kod-duzeltmesi (`pub use kama_contracts::s4`; baytlar eşit, H-2 93/93) |
| H-HS-10 | DOĞRULANAMADI | orta | crate geneli | Hub, "v1.3.1 taslak · G-S4 öncesi" damgalı bir sözleşmeyi uyguluyor. "G-S4 GEÇTİ" sözü depoda bulunamadı. Aranan: "G-S4 GEÇTİ", "S4 FROZEN". KARAR-KAYDI §4'e göre bu söz belgeye yazılmaz, yalnız sahibin mesajıdır. S4 §8'in Y1–Y6 yamalarının spec'te olmadığı doğrulandı. H-HS-01…07 bu duruma bağlıdır. | PoC-KAPSAM-001 §2 Sözleşmeler: "hub sprintinde G-S4 ile FROZEN"; S4 §7 kapı. | karar-gerekir |
| H-HS-11 | EKSİK | düşük | table.rs:114–124, lib.rs:171–187 | Hub tabloları doğrudan bellek içi HashMap'ler. Storage trait yok; yeniden başlatmada her şey kaybolur. | ADR-13: "Storage trait ZORUNLU … `put_handle_binding / expire_handle_binding` …". S4 §1 madde 9 bunu beyanlı PoC sınırı sayıyor (taslak). | karar-gerekir |
| H-HS-12 | UYUŞMAZ | düşük | main.rs:121–243 | Engelleyici `std::net::UdpSocket`, tek iş parçacığı, Tokio yok. adaptive-transport, ccr-codec, redb ve quinn kullanılmıyor. | ADR-02: "Tüm Rust servisleri Tokio üzerinde". ADR §3 karar matrisi: "hub-server Rust/Tokio · adaptive-transport, ccr-codec, redb, quinn". TDD §5 aksini söylüyor ("async/tokio yok"). | karar-gerekir |
| H-HS-13 | UYUŞMAZ | düşük | main.rs, config.rs:3–8 | Yapılandırma JSON'dur (`--config <hub.json>`). Sağlık ve metrik uç noktası yok; yerlerini `probe` alt komutu ve 10 s'lik log satırı tutuyor. | ADR-19 NORMATİF: "--config /etc/kama/<service>.toml · healthcheck endpoint · metrics endpoint". S4 §9 (taslak): "TLS/HTTP/metrik sunucusu" kapsam dışı. | karar-gerekir |
| H-HS-14 | UYUŞMAZ | düşük | bind.rs:25, 279–293; relay.rs:12, 35; table.rs:19, 521 | Ed25519 imzalama yalnız `ed25519-dalek`, BLAKE2s yalnız RustCrypto `blake2` ile yapılıyor. libsodium yolu ve çapraz denetim yok. | ADR-03: "libsodium … primary: Ed25519, X25519, …, BLAKE2s · dalek … cross-check testleri". DR-5 yalnız AEAD backend'ini değiştirdi. | karar-gerekir |
| H-HS-15 | TANIMSIZ | düşük | bind.rs:138–153, table.rs:521–528, relay.rs:35–42 | BLAKE2s-256 iki rolde kullanılıyor: anahtarlı PRF olarak (`secret ‖ girdi`, kısaltılmış; cookie ve handle adayı) ve anahtarsız kısa özet olarak (tahsis anahtarı). | pki §13 registry'si BLAKE2s'i yalnız KDF rolünde sayıyor ("BLAKE2s (KDF)"). Bu yapı yalnız S4 taslağında tanımlı. Primitif registry'de; rol tanımı yok. | karar-gerekir |
| H-HS-16 | EKSİK | düşük | bind.rs:328–461, config.rs:270–302, relay.rs | Eksik olanlar: app-rpc §5 CLIENT_BIND (Noise IK possession, ROOT_CA_SPKI_HASH'e zincir, revocation freshness fail-closed), revocation Layer-2 hub deny-cache'i, cert-bound hub kimliği (HubEntryV3 yok; hub anahtarı provisioning'den ham Ed25519), DR-CONNECT-1 H7 mailbox fallback'i, routing §8.3 hop-ARQ, DHT/ANNOUNCE. | Spec'ler bunları istiyor. PoC-KAPSAM-001 §2/§3/§5 ve EK-2 (sahip) dışarıda bırakıyor: "PKI / sertifika zinciri ve iptal doğrulaması · … DHT, hub-mesh · hub mailbox (EK-2) · HubChannel gerçek hop anahtarı"; EK-2 (2026-10-11) H7'den (2026-10-03) yenidir. | karar-gerekir (PoC sonrası) |
| H-HS-17 | TANIMSIZ | düşük | main.rs:179–182; stats.rs:14–15, 139–141 | `--listen` bayrağı ve satırdaki üç ek sayaç "INT-02H; YAAZ-EK-1"e atıf yapıyor. | Atıf depoda bulunamadı. Aranan: "YAAZ-EK-1", "INT-02H", bütün `.md` dosyalarında. TDD §5 satır listesi `hb_unbound`'da bitiyor. | karar-gerekir |
| H-HS-18 | UYUŞMAZ | düşük | main.rs:32, 190 | Okuma zaman aşımı `TICK = 1 s`. | TDD §5: "`recv_from` (okuma zaman aşımı `STATS_PERIOD`)" = 10 s. Kodun değeri zamanlayıcıyı daha düzenli koşturuyor. | karar-gerekir (belge mi, kod mu) |
| H-HS-19 | TANIMSIZ | düşük | main.rs:33–35, 229–239 | `MAX_CONSECUTIVE_FAILURES = 200` ve `ERROR_BACKOFF = 50 ms`: yaklaşık 10 s sürekli `recv` hatasında süreç çıkıyor. | S4, TDD ve Deploy TDD'de bulunamadı. Aranan: art arda hata, backoff, çıkış. | karar-gerekir |
| H-HS-20 | TANIMSIZ | düşük | main.rs:47–59, stats.rs:236–285 | `send error kind= to=<ip:port> total= suppressed=` satırı TDD §5 olay listesinde yok. Satır bağlı ucun adresini journald'a yazıyor. | TDD §5 olayları yalnız `bind ok`, `link open`, `link retire`, `cookie rotate`. S4 H12: "kimlik … loglanmaz". | karar-gerekir |
| H-HS-21 | TANIMSIZ | düşük | config.rs:82; bind.rs:188–227 | `VERIFY_BUDGET` patlaması 2000. bind_fail sayım penceresi ilk başarısızlıktan `HOLD` kadar; kilit sınıra ulaşılan andan sayılıyor. | S4 §6 yalnız "2000 doğrulama/s" ve "10 / 60 s / 4096" diyor. Patlama ve pencere tanımı yok. | karar-gerekir |
| H-HS-22 | TANIMSIZ | düşük | bind.rs:68–84, lib.rs:275–280 | Cookie yenilemesinde rastgelelik yoksa eski sır sessizce kalıyor ve her tick'te yeniden deneniyor. Sayaç ya da log yok. | S4 §2.4(a): "`cookie_secret` … saatte bir yenilenir". Başarısızlık davranışı tanımsız. | kod-duzeltmesi (sayaç ya da log satırı; davranış aynı kalır) |
| H-HS-23 | ÖLÜ | düşük | stats.rs:59–101 ile 117–142 | `drop_peer_unbound`, `bad_request`, `drop_unknown_method`, `verify_calls`, `rx`, `tx_reply`, `tx_forward` sayılıyor ama hiçbir satıra girmiyor; yalnız `Hub::stats()` ile okunabiliyor. `drop_peer_unbound` S4 H3 ve TDD T-D5'te adıyla geçiyor. | TDD §5 satır listesi. | karar-gerekir (satır biçimi TDD'de) |
| H-HS-24 | ÖLÜ | düşük | table.rs:44; config.rs:128, 138 | `Endpoint.last_activity` dört yerde yazılıyor, hub mantığında okuyan yok. `Device.name` ve `Config.hub_pub` doğrulamadan sonra kullanılmıyor. | S4 H3/H8 `last_activity`'yi tazelemeyi istiyor ama bir tüketici tanımlamıyor. | karar-gerekir |
| H-HS-25 | TANIMSIZ | düşük | config.rs:270–302 | Boş `devices` listesi ve yinelenen `name` kabul ediliyor. Dizini boş hub hiçbir bind'e cevap vermeden başlıyor. | TDD §6 ve T-C1'de tanımlı değil. | karar-gerekir |
| H-HS-26 | UYUŞMAZ | düşük | probe.rs:193–205, 311–313 | Probe, imzasız `POSSESSION_PROOF_FAIL` ve diğer hata çerçevelerini kesin ret sayıp kırmızı veriyor. | S4 C8: "imzasız BIND hatası bind'i **düşürmez** (yoldaki saldırgan üretebilir) — yeniden deneme T3 kadansıyla". Probe bir teşhis aracı; C8 istemci yükümlülüğü. | karar-gerekir |
| H-HS-27 | TANIMSIZ | düşük | probe.rs:244–246, 263–264, 138–171 | Çözülemeyen yanıt `Timeout(step)` olarak raporlanıyor; hata sınıfı yanlış. Cihaz dosyasında `format_version` denetlenmiyor. | TDD §5: "v1 = v0 anahtar adları + `format_version: 1` + `hub.ed25519_pub`". | kod-duzeltmesi |
| H-HS-28 | TANIMSIZ | düşük | main.rs:152, 136, 145, 160 | `run_probe` sonucu `println!`/`eprintln!` ile basıyor. Bu makrolar kapalı boruda panikler. Aynı dosya `UdpSink::log`'da bu riski yorumla kapatmış (62–63). | S4 H12 ve CLAUDE.md §9 (açık hata, panik yok). Probe yolu hub değil; risk düşük. | kod-duzeltmesi |
| H-KS-01 | TANIMSIZ | düşük | kama-store lib.rs:24–39, 89–113 | Şema v0 dosyada sürüm işareti taşımıyor. `DEDUP_WINDOW_WORDS` ya da `MAX_OUTBOX_RECORD_BYTES` değişirse eski dosya `Corrupt` verir; davranış fail-closed ama sessiz. | PoC-KAPSAM-001 §2 "Kalıcılık" yalnız içeriği tanımlıyor; göç kuralı yok. Crate belgesi "göç YOK" diyor. | karar-gerekir |

**Yukarıda sayılmayanlar.** Şu ayrılıklar sahip kararıyla kapsam dışıdır ve uyumlu sayıldı:

- Bind sonrası kontrol mesajları imzasız ve kaynak adrese dayanıyor (PoC-KAPSAM-001 §5 "Hop'ta kimlik doğrulama yok · M4"; S4 beyanlı sınır 1).
- HubChannel no-op 16 B kuyruk kullanıyor (PoC-KAPSAM §3 "HubChannel gerçek hop anahtarı (no-op kalır)").
- Zincir ve iptal doğrulaması yok (§2 "Anahtarlar"; ccr §35.4 kapalı-sistem kipi YAMA'sı).

CLAUDE.md §4'teki DHT değişmezi ihlal edilmiyor: istemci DHT'de değil, hub'da da DHT yok. `verify_chain`, enrollment PoP ve HubEntryV3 için hub-server'da kod yok (H-HS-16). INV-NO-IDLE-TEARDOWN tutuyor: hiçbir zamanlayıcı hat ya da uç silmiyor (H9, T-M6).

## Okunan otorite bölümleri

- `docs/seams/KM-SEAM-TH-001.md` (S4 v1.3.1, taslak): tamamı, 1–364.
- `docs/hub/KM-HUB-TDD-001.md` v0.6.1: tamamı, 1–228.
- `docs/hub/H-2-WIRE-RS-KARSILASTIRMA-2026-11-11.md`: 1–60 ve 130–180 (yöntem, uyuşmazlık listesi U-1…U-4, geçiş önerisi).
- `docs/decisions/PoC-KAPSAM-001.md`: tamamı, 1–105.
- `docs/decisions/KARAR-KAYDI-2026-11.md`: tamamı, 1–148.
- `docs/decisions/DR-CONNECT-1.md`: tamamı, 1–53.
- #175 sahip yorumları 7171705736, 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951. Hub ya da depo ile ilgili madde yok; `hub`, `redb`, `store`, `relay` ile arandı, 7171705736 ve 7186813951 baştan sona okundu.
- CLAUDE.md §4 (cert-bound, enrollment, DHT), §5, §8, §9 ve YAMA KAYDI.
- `specs/application-protocol-and-rpc-spec.md`: YAMA bandı 1–16, §3–§6.2 (91–250), §6.11–§6.11.1 (396–446), §11–§12 (575–650).
- `specs/routing-rendezvous-architecture.md`: §1–§2.2 (28–172), §8–§9.1 (896–987), başlık dizini.
- `specs/tech-stack-adr.md`: ADR-01, ADR-02, ADR-03, ADR-05, ADR-13, ADR-14, ADR-19 ve §3 karar matrisi.
- `specs/ccr-format-spec.md`: YAMA bandı satır 5 ve 15, §35.7 (2734–2752), `init_counter` geçerliliği (1947–1949).
- `specs/adaptive-transport-engine-spec.md`: YAMA bandı satır 7, §1.1 registry (120–150).
- `specs/pki-ca-ra-hsm-tdd.md`: §13 registry (450–474).
- `specs/identity-and-trust-model.md`: E-7 (95) ve `hub` araması.
- `specs/revocation-model.md` ve `specs/admin-control-plane-tdd.md`: `hub` araması. Layer-2 hub enforcement ve ADMIN_ALLOWLIST_HUB, PoC dışı.
- `specs/transport-layer-tdd.md`: "NAT-robust" (156).
