# C5-hub-istemci — `crates/client-core/src/hub/` denetimi

Taban: ürün hattı `125b9afa`. Salt okuma; kod değişmedi. Satır numaraları bu tabandandır.

## Kapsam

| dosya | satır | okunan |
|---|---|---|
| `crates/client-core/src/hub/mod.rs` | 1–194 | 194 |
| `crates/client-core/src/hub/signer.rs` | 1–27 | 27 |
| `crates/client-core/src/hub/t3.rs` | 1–61 | 61 |
| `crates/client-core/src/hub/heartbeat.rs` | 1–251 | 251 |
| `crates/client-core/src/hub/bind.rs` | 1–324 | 324 |
| `crates/client-core/src/hub/link.rs` | 1–1107 | 1107 |
| `crates/client-core/src/hub/link_tests.rs` | 1–1762 | 1762 |
| `crates/client-core/src/hub/link_glare_tests.rs` | 1–522 | 522 |
| `crates/client-core/src/hub/shared_crypto.rs` | 1–797 | 797 |
| `crates/client-core/src/hub/relink.rs` | 1–653 | 653 |
| `crates/client-core/src/hub/endpoint.rs` | 1–1346 | 1346 |
| **toplam kod** | | **7044** |

Çapraz okuma (bağlam için, denetim kapsamı dışı): `crates/kama-node/src/hub_mode.rs:150–184`, `crates/kama-contracts/src/s4.rs` (yalnız `RouteMiss::decode` ve sabit adları, grep), `crates/client-core/src/transport.rs:138` (`PENDING_INIT_CAP`).

Genel not — kod yorumlarındaki karar kimlikleri: kod `KK-H31…KK-H66` ve `[HLR]`, `[HLR-STALE]`, `[T3-GLARE]` etiketlerine dayanıyor. Depoda yalnız **KK-H36, KK-H52, KK-H54** bulundu (`docs/decisions/SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` §10) ve KK-H52'nin tasarım notu (`docs/design/HB2-ALIM-YOLU.md`). Geri kalanlar (KK-H33, H34, H35, H37, H38, H39, H40, H41, H42, H43, H44, H48, H51, H53, H57, H58, H66) **spec'te/depoda bulunamadı** (aranan: `KK-H3?`, `KK-H4?`, `KK-H5?`, `KK-H6?` bütün `.md` dosyalarında). Bunlara dayanan davranışlar aşağıda, varsa, S4 ya da spec metniyle karşılaştırıldı. Ayrıca S4'ün (`docs/seams/KM-SEAM-TH-001.md` v1.3.1) FROZEN olup olmadığı **DOĞRULANAMADI**: belgede "G-S4 öncesi" yazıyor, kapı sözü ("G-S4 GEÇTİ") yalnız sahibin ayrı mesajıyla geçerli (KARAR-KAYDI-2026-11 satır 53), depoda kaydı yok. Bu yüzden S4 ile dondurulmuş spec (transport-layer-tdd, app-rpc) çeliştiğinde bulgu `karar-gerekir` olarak işaretlendi.

## Davranış envanteri

### mod.rs
- `HubConfig` (47–56): `hub_addr`, `hub_ed25519_pub` (ham anahtar, provisioning'den), `device_cert_ref`, `self_dest_hash`. `Debug` (58–67) kimlik özetlerini `<redacted>` basar.
- `HubError` (71–87): thiserror; `NotImplemented` (yalnız tanım, kullanılmıyor), `Entropy`, `QueueFull`, `FrameTooLong`, `BadEnvelopeType`.
- `HubNotice` (91–102): `Bound`, `Disconnected`, `HandleLost{peer,h}`, `AttemptEnded{peer}`.
- `HubCounters` (107–194): 40 sayaç, hepsi doyarak artar (`bump`).
- Uyumlu: 4 (kimlik Debug'a düşmez; tipli hata; sayaçlar doyar; çerçeve sınırı belgesi S4 §2.1 ile aynı).

### signer.rs
- `BIND_TBS_LEN = 112` (16+32+32+16+16, S4 §2.4(b)'den türetilmiş), `BIND_SIGNATURE_LEN = 64`.
- `BindSigner::sign_bind(&[u8;112]) -> [u8;64]`: yalnız sabit TBS görür, imza kâhini yok; anahtar client-core'da yok.
- Uyumlu: 3 (INV-ENGINE-SOC: anahtar tutulmaz; TBS sabit boy; imza anahtarı cihaz anahtarı, S4 KR-5).

### t3.rs
- `T3_INITIAL 1 s`, `T3_FACTOR 2`, `T3_CAP 30 s`, `BACKOFF_MIN 5 s`, `ATTEMPT_MAX_AGE 3600 s` — S4 C3, §6'dan aynen.
- `t3_delay(n)` (27–33): `1 s·2ⁿ`, tavan 30 s, `checked_pow` ile taşmasız. `with_backoff(d)` (37–39): `max(d, 5 s)` (yalnız testte kullanılıyor; `link.rs` doğrudan `BACKOFF_MIN` kullanıyor).
- Testler (41–61): dizi `1,2,4,8,16,30,30,30`; `u32::MAX` → tavan; geri çekilme ≥ 5 s.
- Uyumlu: 5.

### heartbeat.rs
- Sabitler: `HEARTBEAT_FOREGROUND 8 s`, `HEARTBEAT_BACKGROUND 25 s` (kullanılmıyor), `ACK_WAIT 1 s`, `PROBE_COUNT 3`, `PROBE_INTERVAL 1 s` (KS-HB-1 ve S4 C6).
- `jitter(seed, n, step)` (45–57): `BLAKE2s-256(seed ‖ u32_be(n))[0..4] (u32 BE) mod (step/4 ms)`; pencere 0 ise sıfır. RNG yok.
- `Heartbeat` (81–187): `Idle` → vade `last_tx + 8 s + jitter(seq)`; vadede `HEARTBEAT{token}` + `Waiting{outstanding=[token], probes=0, next=now+1 s}`. `Waiting`'de vade gelince `probes < 3` ise yeni token'lı yoklama (`next=now+1 s`), `probes ≥ 3` ise `Disconnected` ve `Idle`. `outstanding` ≤ 4 token. `on_ack(token)`: bekleyen herhangi bir token eşleşirse `Idle`. `start_probe(now)`: `Idle` ise hemen yoklama dizisi (3 yoklama, sonra koptu). `reset()`: `Idle`.
- Akış süresi: normal yolda 1 heartbeat + 3 yoklama, ilk heartbeat'ten 4 s sonra "koptu"; soket hatası yolunda 3 yoklama, 3 s sonra.
- Testler: jitter sınırlı ve deterministik; yoklama dizisi ve koptu; herhangi bir bekleyen ACK yoklamayı bitirir.
- Uyumlu: 7 (yalnız sessizlikte; kaçırma sayacı yok; 3 × 1 s; yoklamada jitter yok; token aynen; soket hatası tetiği; RNG yok, ADR-17 determinizmi).

### bind.rs
- `BIND_ROUND_SENDS 3`, `BIND_RESEND 1 s` (S4 C2), `ENTROPY_RETRY 1 s` (türetilmemiş).
- `RequestIds` (39–52): başlangıç entropiden, her istekte +1, sarmalar (S4 §2.3).
- `BindMachine` durumları `Round1 / Round2 / Backoff / Bound`. `new_round` (143–167): yeni `client_nonce` (entropi hatasında `Backoff` 1 s), yeni `request_id`, CLIENT_BIND çerçevesi.
- `on_tick` (170–208): turda en çok 3 bayt-özdeş gönderim, 1 s arayla; tükenince `Backoff{t3_delay(exhausted) + jitter(o turun nonce'u)}`, sonra yeni tur (yeni nonce).
- `on_response` (211–303): Round1 + CLIENT_BIND hata → ipucu sayacı; Round1 + CHALLENGE (aynı rid) → TBS kur, `BindSigner`, BIND_PROOF (yeni rid), Round2; Round2 + `BAD_COOKIE` → hemen yeni Round1 (eski kanıt bir daha gitmez); Round2 + diğer hata → ipucu; Round2 + yanıt → `ack_tbs(cert_ref, nonce, hub_nonce, 1, OK)` üstünde `verify_strict`; geçmezse sayaç, durum değişmez; geçerse `Bound{nonce}`. Eşleşmeyen her şey `bind_unmatched`.
- `rpc_frame_checked` / `rpc_frame` (309–317): `kama_contracts::s4` codec; `rpc_frame` hata halinde boş çerçeve verir (yalnız sabit boylu yükler). `heartbeat_frame(token)` (320–324).
- Uyumlu: 10 (her denemede yeni nonce; 3 × 1 s bayt-özdeş; BAD_COOKIE → 1. tur; katı imza doğrulaması; imzasız hata bind'i düşürmez; tek deneme; rid kuralı; TBS alan sırası; geç meydan okuma atanmaz; tükenişte T3 + jitter).

### link.rs (HubLink çekirdeği)
- Tablo tavanları (49–69): `MAX_ATTEMPTS 16`, `MAX_QUEUED_RESPONSES 16`, `MAX_HELD_RESPONSES 16`, `MAX_RESPONDER_RECORDS 64`, `MAX_KNOWN_LINKS 64` (S4 `MAX_LINKS_PER_ENDPOINT`'ten), `MAX_RETIRED_HANDLES 64`, `MAX_RESPONSE_STORE_IDS 64`, `MAX_TX_QUEUE 64`, `MAX_EVENTS 256`, `MAX_NOTICES 64`, `NOTICE_BACKOFF = BACKOFF_MIN (5 s)`.
- `Deferred` (98–145): eş başına ertelenen bildirimler; `AttemptEnded` en çok bir, `HandleLost` kaybolan her hat için bir (≤ 64, taşarsa en eski düşer). Verirken sıra: önce `AttemptEnded`, sonra `HandleLost`'lar.
- `new` (201–250): entropiden rid tabanı + token tabanı + ilk nonce; bind hemen başlar.
- `on_control` (261–283): `last_rx=now` (her çerçevede); `HubFrame::decode`; bilinmeyen tip ve tel→hub `HEARTBEAT` → `hub_frames_dropped`; bozuk → `rx_malformed`; `HEARTBEAT_ACK` bağlıyken eşleşirse `heartbeat_acks`; `PLANE1_RPC` → `on_rpc`; sonunda `on_tick`.
- `on_rpc` (285–326): DELIVER, ROUTE_MISS, STORE yanıtı/hatası, bind yanıtları dağıtılır; tel→hub satırları `hub_frames_dropped`.
- `on_bound` (328–349): heartbeat sıfırlanır; yeniden bind ise gönderilmiş (`sends>0`) her deneme biter + `AttemptEnded`; hiç gönderilmemişler kalır; `Bound` bildirimi; bind öncesi kuyruktaki RESPONSE'lar gönderilir.
- `on_store_response` (351–420): `request_id` ile denemeyi bulur, yoksa RESPONSE STORE yanıtı ya da geç yanıt sayılır. `OK, h≠0`: atanmamışsa ve tutulan yanıt çelişiyorsa deneme biter; değilse `HandleAssigned` + tutulan RESPONSE bırakılır, **T3 sürer**; aynı h → yinelenen; farklı h → çelişki, deneme biter. `UNKNOWN_HANDLE` → deneme biter. `PEER_UNBOUND`/`RATE_LIMITED`/`HUB_CAPACITY` → `next_send ≥ now+5 s`. Diğer (BAD_REQUEST, `OK` ile `h=0`, bilinmeyen) → yine `now+5 s` ve `store_backoffs`.
- `release_held` (423–435): atanan h için tutulan RESPONSE `HandleAssigned`'dan hemen sonra verilir.
- `on_deliver` / `on_deliver_init` (437–482): INIT → aynı baytlı kayıt varsa: farklı h → `init_handle_conflict`, aynı h → saklı RESPONSE çerçevesi yeniden ya da `init_duplicate_pending`; yoksa kayıt (≤ 64, FIFO) + `IncomingInit`. RELAY_OPEN → yalnız sayaç.
- `on_deliver_response` (484–545): atanmış h'si eşleşen deneme → aynı bayt ikinci kez verilmez, yeni bayt `Response`; eski/emekli/bilinen hat → `late_response`; aksi halde **DELIVER'ın `sender_dest_hash`'iyle** eşin açık denemesi bulunur: deneme atanmışsa (farklı h) → çelişki, deneme biter; atanmamış ve gönderilmişse RESPONSE **tutulur** (≤ 16, en eski düşer).
- `held_conflicts` (548–553), `is_old_handle` (555–557), `take_attempt` (560–566: yalnız atanmış handle'ı emekli halkasına yazar), `retire` (568–576).
- `on_route_miss` (578–607): h'si atanmış deneme biter (`AttemptEnded`), kurulu hat silinir (`HandleLost`), h emekli edilir, yanıtlayan kaydı silinir; ikisi de yoksa `route_miss_unknown`. `reason` alanına bakılmaz (codec 0x01 dışını reddeder).
- `send_init` (616–653): yalnız `0x01`; boy denetimi; aynı eşin eski denemesi biter (`attempt_superseded`); tavan 16 (`QueueFull`); yeni rid + bayt-özdeş çerçeve.
- `send_response` (660–697): yalnız `0x02` ve h≠0; bind yokken kuyruk 16; çerçeve en yeni eşleşen `(h, INIT)` kaydına bağlanır (yoksa `response_unrecorded`); rid halkası; hat hatırlanır; gönderilir ya da kuyruğa alınır.
- `peer_ready` (722–735): eş + atanmış h eşleşirse deneme biter (`attempt_completed`); değilse `ready_unmatched` ve h kurulu hat olarak hatırlanır.
- `cancel_attempt` (740–745): deneme biter, bildirim yok.
- `poll_transmit` (751–757): `on_tick`, sonra kuyruktan bir çerçeve; `last_tx=now`.
- `next_deadline` (762–783): bind vadesi, bağlıyken heartbeat ve T3 vadeleri, deneme yaşı, tutulan yanıt yaşı, ertelenmiş bildirim vadeleri.
- `on_tick` (786–834): ertelenmiş bildirimler; yaş denetimi; bağlı değilse yalnız bind; bağlıysa heartbeat (koptu → `on_disconnected`) ve vadesi gelen INIT'ler (`next_send = now + t3_delay(sends)`).
- `on_disconnected` (836–851): sayaç, heartbeat sıfırla, bind yeniden (yeni nonce), `Disconnected`.
- `expire` (853–866): `ATTEMPT_MAX_AGE` geçen deneme biter (`AttemptEnded`); tutulan yanıtlar da aynı yaşta silinir.
- `on_session_traffic` (870–877): yalnız `last_tx`/`last_rx`; T3'ü durdurmaz (KK-H36).
- `on_socket_error` (880–885): bağlıysa yoklama.
- `push_tx` (912–918): kuyruk doluysa çerçeve **düşer** (yalnız `bounded_drops`), bind/INIT/RESPONSE ayrımı yok. `push_event` aynı. `push_notice` doluysa en eskiyi atar.
- `peer_notice` (939–969) / `release_deferred_notices` (971–982): eş başına ≥ 5 s; eş tablosu 64, doluysa ertelenmişi olmayan en eski (yoksa en eski) atılır ve ertelenmişleri hemen verilir.
- `remember_link` (984–1000): 64 tavanı; doluyken önce yeri yenisiyle dolmuş eşin en eski hattı, yoksa en eski hat düşer.
- `store_frame` (1004–1019): `StoreMessageReq::validate` + codec.
- `HubClock`/`SystemHubClock`/`HubEnvelopeLink` (1027–1103): `Mutex` zehirlenirse iç değerle sürer; bütün `HubError`'lar birimli `LinkError`'a çevrilir.
- Uyumlu: 20 (C2 bind öncesi gönderim yok; C3 aynı rid + bayt-özdeş T3; `OK` T3'ü durdurmaz; `UNKNOWN_HANDLE` biter; çelişki biter, zorla yazmaz; yaş yalnız o denemeyi bitirir; aynı eşe yeni INIT eskisini bitirir; ≥ 5 s geri çekilme; geç `StoreResp` sayılır; C4 yanıtlayan RESPONSE'u yeniden gönderir; aynı INIT farklı h → yok say; C5 h bırakılır, eş başına aralık; C7 gönderilmiş denemeler biter, kurulu hatlar sürer; C8 dağıtım; C9 `last_rx`; KK-H36 `peer_ready`; bütün tablolar sınırlı; glare kaybedeninin terki identity §16 + S4 C3 "deneme açıkça sonlandırıldı" ile; ROUTE_MISS reason codec'te; `HandleLost` eski hattı adlandırır).

### link_tests.rs ve link_glare_tests.rs (test modülleri)
Sabitlenen davranışlar: C2 iki tur + TBS alan sırası + katı imza (`c2_bind_two_rounds_tbs_and_verify_strict`); kanıtın 3 × 1 s bayt-özdeş tekrarı ve tükenişte `1 s + %25` jitter; CLIENT_BIND turu ve geç meydan okuma; BAD_COOKIE → yeni nonce, eski kanıt asla yeniden gitmez; imzasız bind hataları bind'i düşürmez; bind öncesi yalnız CLIENT_BIND ve 17. deneme `QueueFull`; C6 sessizlik + jitter, cevapsız yoklama → koptu → yeni nonce, yanıtlanan yoklama, 6 saatlik sessizlik, soket hatası; C3 STORE yerleşimi, T3 dizisi `1,2,4,8,16,30,30`, `peer_ready` eşleşmesi, PEER_UNBOUND/RATE_LIMITED/HUB_CAPACITY ≥ 5 s, `UNKNOWN_HANDLE`, yaş, çelişki, yerine geçme, kötü girdiler (`çerçeve + 16 = 1280`); C4 yanıtlayan tekrarı, başlatanda bayt başına bir RESPONSE; K3 tutma ve bırakma sırası; tutulanların sınırı; C5 `HandleLost` ve ertelenmiş bildirimler; HLR/HLR-STALE serisi (sessiz eşin tek hattı korunur, terk edilen deneme bildirim üretmez, eşleşmeyen Ready hattı hatırlanır, eski hattın kaybı onu adlandırır, `RelinkGovernor` süzgeci); C7 yeniden bind; C8/C9 sayaçları; tablo sınırları; Debug; cephe; fix1 (çelişki iki yönde, K9 iki vaka, başka eşin yanıtı çelişki değil); T3-GLARE (kaybedenin denemesi terkle durur, yalnız o deneme). `link_glare_tests.rs`: iki gerçek `ClientTransport` + gerçek `HubLink` + test rölesiyle glare'in iki sırası, niyet iptali, yanıtsız niyetin T3'ü, glare sonrası hat kaybında yenileme; ölçüt "iki uçta aynı handle + iki yönde veri" (S4 §7).
Eksik test: **atanmamış (StoreResp'i hiç gelmemiş) denemenin tutulan RESPONSE'u yeni denemeyi bitirir mi** — yok (bkz. B2).

### shared_crypto.rs
- `StashConfig` varsayılanı (71–80): `per_handle_cap 32`, `total_cap 256`, `ring_interval 20 ms`, `dead_consumer_ttl 10 s` — `[KALİBRASYON]`, HB2-ALIM-YOLU ile aynı değerler.
- `OpenedStash` (128–294): `reserve(h)` (yer yoksa `backpressure`, açma yok); `fill`; `cancel`; `take(wire)` tam bayt eşleşmesiyle (yoksa `doorbell_dup`); `due_rings(now)`: oluşturulalı ≥ 10 s olan kayıt `dropped_expired`, diğerleri 20 ms'de bir yeniden zil; `drop_session` → `dropped_closed`. `lost() = dropped_closed + dropped_expired`.
- `clamp` (299–305): hub kipinde `max_plaintext ≤ 1280 − wire_overhead` (KK-H54).
- `HubCrypto` (314–469): `Direct` saf geçiş; `Hub`'da `seal`/`sealed_len` kırpılmış sınırla, `open` kriptoyu çağırmadan kanaldan alır, `poll_event` `Closed`'da kanal kayıtlarını düşürür; kilitler birlikte tutulmaz.
- `HubCryptoTap` (344–359): `HubEndpoint`'in tek açması ve `seal(HubChannel)`.
- Testler: geçiş eşitliği, tek açma, taşmada açmadan red (kripto parmak izi değişmez), duran/ölü tüketici, kapanan oturum, 1280 kırpması.
- Uyumlu: 9 (KK-H52 (a) seçeneğinin yedi adımı ve üç vakası; KK-H54; INV-ENGINE-SOC — kripto S3 trait arkasında kalır).

### relink.rs
- Sabitler: `RELINK_FREE_STARTS 3` (app-rpc §6.11.1 `2..3`), `RELINK_BACKOFF_MIN = 5 s` (S4 C3'ten), `RELINK_BACKOFF_MAX 300 s`, `RELINK_QUIET_RESET 600 s`, `RELINK_RETRY 1 s` (üçü `[KALİBRASYON]`, türetilmemiş), `MAX_RELINK_PEERS 256`.
- `on_notice` (157–187): `HandleLost`: etkin hat başka ise `Stale`; bilinmiyorsa koşulsuz `Relink`; aksi `RelinkLine(h)`. `AttemptEnded`: oturum varsa `Restart`, yoksa `RestartIntent`.
- `drive` (196–252): önce `Restart`, `Nothing` dönerse bekleyen `Relink`; `Started` → `not_before = now + backoff(starts)`; `Deferred` → `1 s·2^(d−1)` tavan 300 s; `Stale` isteği bitirir. İsteği ve yakın sayacı olmayan girdi silinir.
- `backoff` (327–336): ilk 3 için 0; sonra `5 s·2^k`, tavan 300 s, + `jitter(seed, draw, base)`. Tohum `new(seed)`'den (üründe OS entropisi).
- Testler: ROUTE_MISS fırtınası sınırlı, sessizlikte sıfırlama, ertelenen istek kaybolmaz, yönlendirme, tablo sınırı, HLR-STALE iki an, koşulsuz yenileme.
- Uyumlu: 6.

### endpoint.rs
- Sabitler: `MAX_DATAGRAM 1280` (S4 §6), `HUB_POLL 20 ms`, `RELAY_POLL 50 ms`, `CLIENT_PORT_CAP 256`, `CONTROL_BURST 64`, `RECV_BUF 65536`, `KNOWN_HANDLE_CAP 1024`, `ATTEMPT_CAP 64` (hepsi türetilmemiş tavanlar).
- `role_of` (115–129): istemci ucu `0x02,0x04,0x07,0x10,0x14,0x15,0x16,0x17`; veri alıcısı `0x00,0x01,0x03,0x05,0x06,0x08,0x09`; diğer `None`.
- `HubControl` trait + `impl for HubLink` (135–175): saf iletim.
- `KnownHandles` (182–252): sınırlı küme (1024, en eski düşer); `bind_peer` eş → **en son bağlanan** handle (doluysa yeni eş sessizce yok sayılır); `handle_of`.
- `RecordingLink` (256–327): `EnvelopeLink`'i değiştirmeden sarar; `send_response` ve `HandleAssigned` eşi bağlar, `IncomingInit`/`Response` kümeye ekler.
- `send_to_hub` (440–460): > 1280 B → `DatagramTooLarge` tipli hata, tele çıkmaz.
- `Core::on_hub_datagram` (480–498): kaynak hub adresi değilse `hub_rx_foreign`; > 1280 düşer; ilk 4 bayt 0 → kontrol, değilse oturum (`on_session_traffic(inbound)` açma öncesi).
- `on_control_datagram` (501–512): açma, bağlam `HubChannel` değilse `hub_ctx_mismatch`, sonra `on_control`.
- `on_session_datagram` (515–558): yer ayır → tek açma → bağlam `Session` → rol → hedef (veri alıcısı adresi ya da h'nin öğrenilmiş istemci portu) → kanala taşı → zil (gönderim hatası yok sayılır).
- `on_relay_datagram` (561–590): yalnız geri döngü kaynağı ve h≠0; veri alıcısı dışındaki kaynaktan h → port öğrenilir; bind yoksa düşer (`hub_tx_unbound`); bilinmeyen h sayılır ama gönderilir; soket hatası yoklama tetikler.
- `on_tick` (593–621): çekirdeğin `on_tick`'i, en çok 64 kontrol çerçevesi mühürlenip gönderilir; mühür hatası `hub_oversize_drops`; zil yeniden çalınır.
- `hub_loop`/`relay_loop` (646–687): `std::thread`, okuma zaman aşımı `min(next_deadline, 20 ms)`, en az 1 ms.
- `HubEndpoint::start`/`Drop` (737–815): iki soket, iki iş parçacığı; Drop panik etmeden durdurur.
- Testler: rol tablosu (256 tip), 1281 B tipli hata, tek kaynak adres + bayt aynen, tek açma + rol dağıtımı, 0x17 istemci ucuna, yeniden zil, yabancı kaynak / bağsız / bilinmeyen h, kontrol mührü, dolu kanalda açmadan düşme, Drop, `RecordingLink` şeffaflığı.
- Uyumlu: 11 (C1 tek hub soketi; MAX_DATAGRAM iki yönde; ilk 4 bayt demux; bağlam doğrulaması; tek açma; giden bayt değişmez; C2 bind yokken veri gitmez; yabancı kaynak sessiz; KK-H54 yedeği; KK-H66 ölçüsü; Drop panik etmez).

Mimari değişmezlerle eşleşme (CLAUDE.md §4): DHT hub-mesh only — modül DHT'ye hiç dokunmuyor (uyumlu). dest_hash hot-path — veri datagramı `[h][şifreli yük]`, dest_hash yok (uyumlu). cert-bound — `hub_ed25519_pub` ham anahtar, sertifikadan türemiyor; PoC-KAPSAM-001 satır 40 PKI/zincir doğrulamasını kapsam dışı bıraktığı ve S4 KR-13 / C2 "provisioning dosyasından" dediği için beyanlı PoC sınırı (bulgu değil). INV-ENGINE-SOC — anahtar baytı yok, kripto S3 trait arkasında (uyumlu).

## Bulgular

| fid | tür | ciddiyet | yer | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| B1 | UYUŞMAZ | orta | `link.rs:484-545`, `375-381`, `548-553` | `DELIVER{RESPONSE}` StoreResp'ten önce gelirse transport'a verilmez, **tutulur**; hangi denemeye ait olduğu imzasız `sender_dest_hash` ile seçilir ve çelişki kararı (deneme bitirme) bu alana dayanır. RESPONSE ancak T3 tekrarının getirdiği StoreResp ile bırakılır (1–30 s gecikme). | S4 C4: "eşleşme yoksa `AcceptResponse{wire}` (KS-DEMUX-1 kriptoda) → kabul eden oturum için `AssignHandle(session, h)` … STORE yanıtı kaybolmuş ya da gecikmişken gelen RESPONSE böyle eşlenir"; §7 K3 "RESPONSE'taki h doğru oturuma atama olur"; §2.4(d) "hiçbir güvenlik kararı bu alana dayanmaz". KK-H37 depoda yok. | karar-gerekir |
| B2 | UYUŞMAZ | orta | `link.rs:560-566`, `629-632`, `333-342`, `402-407`, `853-866`, `548-553` | Atanmamış bir deneme (StoreResp'i gelmemiş) yerine geçme, iptal, yeniden bind, `UNKNOWN_HANDLE` ya da yaşla bittiğinde onun tutulan RESPONSE'ları silinmez ve handle'ları emekli halkasına yazılmaz. Aynı eşe sonraki denemenin `StoreResp{OK, h2}`'si `held_conflicts` ile çelişki sayılır, **yeni deneme biter** (`attempt_handle_conflict`, `AttemptEnded`). Ayrıca eski denemenin RESPONSE'u yeni deneme beklerken gelirse aynı sonuç. | S4 §7 K9: "eski denemenin gecikmiş `StoreResp`/`RESPONSE`'u … yeni denemeye atanmaz (sayaç); deneme 2 kendi h'siyle kurulur"; S4 C3 "sonlandırılmış denemelerin geç yanıtları … yeni denemeye atanmaz, sayılır". | kod-duzeltmesi (ilk alt vaka: `take_attempt` atanmamış denemede o eşin tutulanlarını düşürüp handle'larını `retire` etsin; ikinci alt vaka B1'e bağlı) |
| B3 | UYUŞMAZ | orta | `link.rs:1004-1019` (STORE `recipient_dest_hash`), `bind.rs:153-159`, `244-252` (`device_cert_ref`), `link.rs:443` (DELIVER `sender_dest_hash`) | HubChannel mührü no-op olduğundan RESPONSE STORE'unun `recipient_dest_hash`'i, DELIVER'ın `sender_dest_hash`'i ve CLIENT_BIND/BIND_PROOF'taki statik `device_cert_ref` telde açık. PoC-KAPSAM-001 satır 78 yalnız **INIT** meta alanlarının açık olduğunu beyan eder; diğerleri beyan listesinde yok. Statik `device_cert_ref` cihazı IP değişimleri boyunca bağlar (AMENDMENT-10 madde 5'in tohum gerekçesinin önlemek istediği şey). | CLAUDE.md §4 dest_hash: "WIRE'da cleartext TAŞINMAZ"; PoC-KAPSAM-001:78 "INIT meta alanları (`recipient_dest_hash`, …) telde açık — M4". | karar-gerekir (beyan listesinin genişletilmesi ya da M4) |
| B4 | DOGRULANAMADI | orta | `endpoint.rs:235-251` (+ `kama-node/src/hub_mode.rs:160-166`) | `KnownHandles::handle_of(peer)` eşe **en son bağlanan** handle'ı verir (`HandleAssigned` ya da `send_response`, hangisi sonsa); bileşim kökü S1 `Ready`'de `peer_ready(peer, handle_of(peer))` çağırır. Ready'yi veren oturumun handle'ı ile son bağlanan handle farklıysa T3 ya doğrulanmamış bir denemede durur ya da tamamlanmış denemede sürer. Glare testleri iki sırada geçiyor; genel durum doğrulanamadı. | KK-H36 (SAHIP-KARARI v4 EK-A §10): "T3 yalnız o denemeye/oturum nesline ait doğrulanmış RESPONSE kabulüyle durur: `peer_ready(peer, session_handle)`". | karar-gerekir (Ready olayının oturum handle'ını taşıması) |
| B5 | UYUŞMAZ | düşük | `heartbeat.rs:12-17`, `102-104`; `bind.rs:186-194`; `relink.rs:138-147`, `327-336` | Jitter tohumu bind sonrası son kabul edilen bind'in `client_nonce`'u, bind öncesi o turun `client_nonce`'u; relink'te OS entropisinden ayrı tohum. | transport-layer-tdd §10 [AMENDMENT-10] madde 5 [K4]: "`seed` = `hub_handle` (bind sonrası, bacak başına) / hop handshake'inin yerel ephemeral public anahtarı (bind öncesi)"; DR-CONNECT-1 K4. PoC'de hop el sıkışması yok; KK-H48 depoda yok. Gizlilik gerekçesi (statik kimlik tohum olmasın) korunuyor. | karar-gerekir |
| B6 | UYUŞMAZ | düşük | `bind.rs:186-197` + `t3.rs:17` | Bind turu tükenince geri çekilme T3 dizisiyle 30 s'ye çıkar (+ %25 jitter); kopma sonrası hub kanalı yoklaması (yeniden bind) ön planda 8 s tavanını aşar; 500 ms × 2 merdiveni uygulanmaz (`heartbeat.rs:17-18` bunu açıkça yazar). | transport-layer-tdd §10 [AMENDMENT-10] madde 1 "ilk vade 500 ms, her emisyonda ×2", madde 2 "Tavan … ön plan 8 s". S4 C8 "yeniden deneme T3 kadansıyla" (S4 FROZEN doğrulanamadı). | karar-gerekir |
| B7 | UYUŞMAZ | düşük | `heartbeat.rs:102-104` | Heartbeat aralığı `8 s + jitter[0, 2 s)` = 8–10 s; ön plan tavanı 8 s'nin üstüne çıkabilir. | AMENDMENT-10 madde 2 "Tavan BAĞLAMA göre: ön plan 8 s"; DR-CONNECT-1 H3 "tavan … 8 s". Jitter'in tavana eklenip eklenmeyeceği spec'te bulunamadı (aranan: "tavan" + "jitter" birlikte). | karar-gerekir |
| B8 | EKSİK | düşük | `heartbeat.rs:8-9`, `31-33`; `link.rs:880-885` | Yoklama tetikleri yalnız heartbeat cevapsızlığı ve soket hatası; OS sinyali ve ARQ RTO yedeklemesi yok; 25 s arka plan kadansı tanımlı ama kullanılmıyor; "anında prob" (link gelince / ön plan) yok. | S4 C6 "yoklamayı OS sinyali, soket hatası, ARQ RTO yedeklemesi ya da heartbeat cevapsızlığı tetikler"; AMENDMENT-10 madde 4. OS sinyali ve arka plan için DR-CONNECT-1 tamamlama 6 (K5 → PARK) ve AMENDMENT-10 madde 6 ertelemeyi kapsar; ARQ RTO yedeklemesi için erteleme kaydı depoda bulunamadı (KK-H42 yok). | karar-gerekir |
| B9 | TANIMSIZ | düşük | `link.rs:413-418` | STORE `BAD_REQUEST`, `OK` + `h=0` ve bilinmeyen durumlar da ≥ 5 s geri çekilme alır ve `store_backoffs` sayılır; yorum "T3 kendi kadansıyla" der, kod 5 s uygular. | S4 C3 geri çekilmeyi yalnız `PEER_UNBOUND`/`RATE_LIMITED`/`HUB_CAPACITY` için tanımlar; C8 "hata çerçeveleri yalnız yeniden deneme ipucudur … yeniden deneme T3 kadansıyla". | kod-duzeltmesi (bu kolda `next_send`'e dokunma, ayrı sayaç) |
| B10 | UYUŞMAZ | düşük | `link.rs:265-267`, `278`, `324`, `447` | `hub_frames_dropped` bilinmeyen tipin yanında tel→hub `HEARTBEAT`'i, bilinmeyen `method_id`'yi, hub'dan gelen istek satırlarını ve bilinmeyen zarf tipini de sayar. | S4 C9 "`hub_frames_dropped` yalnız bilinmeyen tip için". | kod-duzeltmesi (ayrı sayaçlar) |
| B11 | TANIMSIZ | düşük | `link.rs:912-918`, `694-696`, `699-707` | Gönderim kuyruğu (64) doluysa bind, INIT ve RESPONSE çerçeveleri sessizce düşer (yalnız `bounded_drops`); bağlıyken `send_response` çerçeve düşse de `Ok` döner. | SR-AUDIT-001 sınır ister, düşme politikasını tanımlamaz; spec'te bulunamadı (aranan: "MAX_TX", gönderim kuyruğu taşması). | kod-duzeltmesi (`send_response`'ta `QueueFull` dön; ya da sınıf başına politika kararı) |
| B12 | TANIMSIZ | düşük | `link.rs:49-69`, `864-865`; `relink.rs:44-53`; `shared_crypto.rs:71-80`; `endpoint.rs:75-89`, `180`, `265` | Türetilmemiş sabitler: tablo tavanları (16/64/256), `NOTICE_BACKOFF` = C3'ün `BACKOFF_MIN`'i (C5 için yeniden kullanım), tutulan RESPONSE ömrü = `ATTEMPT_MAX_AGE`, `RELINK_BACKOFF_MAX 300 s`, `RELINK_QUIET_RESET 600 s`, `RELINK_RETRY 1 s`, kanal 32/256/20 ms/10 s, `HUB_POLL 20 ms`, `RELAY_POLL 50 ms`, `CLIENT_PORT_CAP 256`, `KNOWN_HANDLE_CAP 1024`. `MAX_ATTEMPTS = 16` `transport::PENDING_INIT_CAP`'e bağlanmamış, aynı sayı ikinci kez yazılmış. | SR-AUDIT-001 (sınır zorunlu, değer yok); S4 §6'da bu adlar yok; kanal değerleri HB2-ALIM-YOLU'nda `[KALİBRASYON]`. | karar-gerekir (değerler); kod-duzeltmesi (`MAX_ATTEMPTS = PENDING_INIT_CAP`) |
| B13 | TANIMSIZ | düşük | `endpoint.rs:561-572` | Röle soketi her geri döngü kaynağını kabul eder; veri alıcısı dışındaki her kaynaktan gelen `h`, o h'nin istemci portunu "son yazan kazanır" ile değiştirir. Aynı cihazdaki başka bir süreç istemci rolü kayıtlarını kendine yönlendirebilir; gerçek tüketici zil almaz, kayıt 10 s sonra `dropped_expired`. | Spec'te bulunamadı (aranan: geri döngü röle güven modeli); modül belgesi soketleri "beyanlı uygulama ayrıntısı" sayar, tehdidi beyan etmez. | karar-gerekir |
| B14 | UYUŞMAZ | düşük | `endpoint.rs:607-609`, `490-491` | Mühür hatası `hub_oversize_drops` olarak, 4 bayttan kısa datagram (açma denenmeden) `hub_open_fail` olarak sayılır; sayaç adı ile olay farklı. | S4 H12/C9 sayaç adları; `HubEndpointCounters` alan belgeleri ("`MAX_DATAGRAM`'ı aştığı için", "Kripto açmadı"). | kod-duzeltmesi |
| B15 | TANIMSIZ | düşük | `link.rs:1074-1103` | `HubEnvelopeLink` bütün `HubError`'ları (`QueueFull`, `FrameTooLong`, `BadEnvelopeType`, `Entropy`) birimli `LinkError`'a çevirir; transport hata türünü göremez. Zehirli `Mutex` iç değerle sürer. | CLAUDE.md §9 "açık hata tipleri"; `EnvelopeLink` sözleşmesi `transport.rs`'te (kapsam dışı). | karar-gerekir (sözleşme değişikliği) |
| B16 | EKSİK | düşük | `link.rs:623-625`, `446`; `mod.rs:21-22` | RELAY_OPEN açılışı, `MEDIA_LINK`, C10 akışı yok: `send_init` `0x05`'i reddeder, `DELIVER{RELAY_OPEN}` yalnız sayılır (sayma kısmı C10 ile uyumlu). | S4 C1 (medya handle'ları), C10, KR-10, KR-17. Erteleme KK-H42'ye dayanıyor, depoda yok; CLAUDE.md §2 medyayı ayrı depoya verir. | karar-gerekir |
| B17 | UYUŞMAZ | düşük | `relink.rs:20-23`, `327-336` | `RELINK_FREE_STARTS`'tan sonra aynı hub'a geri çekilmeyle yeniden INIT basılmaya devam edilir; failover yok. | app-rpc §6.11.1 "K ROUTE_MISS sonrası … client aynı hub'a tekrar SESSION_INIT_REF BASMAZ … FAILOVER". S4 C5 "app-rpc §6.11.1 ruhu" ile tek hub'lı PoC'ye uyarlıyor; S4 FROZEN doğrulanamadı. | karar-gerekir |
| B18 | OLU | düşük | `mod.rs:72-74`; `heartbeat.rs:33`; `link.rs:870-871`; `t3.rs:37-39` | `HubError::NotImplemented` hiçbir yerde üretilmiyor; `HEARTBEAT_BACKGROUND` kullanılmıyor (PARK); `on_session_traffic`'in `h` parametresi yok sayılıyor; `with_backoff` yalnız testte. | CLAUDE.md §6 (yarım kod typed-error stub — artık stub yok); DR-CONNECT-1 tamamlama 6 (K5 PARK). | kod-duzeltmesi (`NotImplemented` ve `with_backoff` kaldır ya da kullan) |
| B19 | DOGRULANAMADI | düşük | `heartbeat.rs:23` | Jitter için `blake2::Blake2s256` doğrudan client-core'da kullanılıyor. | tech-stack-adr ADR-03 AMENDMENT-2: BLAKE2s "Üretim-home = `kama-noise-wire` (kripto-katmanı)" — madde `param_hash` için yazılmış; kripto dışı jitter kullanımını kapsayıp kapsamadığı doğrulanamadı. | karar-gerekir |
| B20 | DOGRULANAMADI | düşük | `mod.rs:1-22`, `link.rs` ve `endpoint.rs` yorumları | Davranışların çoğunun gerekçesi `KK-H33…H66` kimliklerine dayanıyor; depoda yalnız KK-H36/H52/H54 var. S4'ün FROZEN durumu kayıtlı değil. | CLAUDE.md §3 (yetki yalnız sahibin tarihli kararı); KARAR-KAYDI-2026-11 satır 53. | karar-gerekir (KK-H kayıtlarının depoya girmesi) |

Kilitli kapsam: bu dosyalarda PTT/metin politikası (karar 3) ve `receiver.rs` alarm aralığı yok; `endpoint.rs`'teki `0x09` yalın PTT ve `0x17` bitmap ACK rol eşlemesi yalnız yönlendirmedir, PTT davranışını değiştirmez — dokunulmaz, bulgu açılmadı.

## Okunan otorite bölümleri

- `docs/seams/KM-SEAM-TH-001.md` (S4 v1.3.1) — tamamı (1–364): KR-1…KR-19, §1–§9, ekler.
- `docs/decisions/DR-CONNECT-1.md` — tamamı (1–53): H1–H13, tamamlama 1–6, K1–K5.
- `docs/decisions/SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` — tamamı (1–110); KK-H36, KK-H52, KK-H54 (§10).
- `docs/design/HB2-ALIM-YOLU.md` — tamamı (1–63).
- `docs/decisions/PoC-KAPSAM-001.md` — satır 30, 34, 40, 50, 78–80, 90 (grep).
- `docs/decisions/KARAR-KAYDI-2026-11.md` — satır 53 (kapı sözleri, grep).
- `specs/transport-layer-tdd.md` — YAMA KAYDI 9–13; §10 AMENDMENT-10 (755–815).
- `specs/application-protocol-and-rpc-spec.md` — §6.11, §6.11.1 (395–446).
- `specs/tech-stack-adr.md` — ADR-03 AMENDMENT-2 (98–113); ADR-03 BLAKE2s satırları (grep).
- `specs/pki-ca-ra-hsm-tdd.md` — §13 registry BLAKE2s satırları 456–457, 603 (grep).
- `specs/routing-rendezvous-architecture.md` — `hub_handle` locality ve `recipient_dest_hash` normatifleri 111–157 (grep).
- `specs/identity-and-trust-model.md` — §16 glare kuralı satır 2208 (grep), YAMA KAYDI satır 11.
- `specs/kama-noise-wire-tdd-v2.0.md` — YAMA KAYDI 9, 11; §2.55/§2.56 başlıkları (grep).
- CLAUDE.md §2, §4 (cert-bound, DHT, dest_hash, INV-ENGINE-SOC), §5 ADR-03/05/17, §6, §8, §9.
- #175 sahip yorumları (7171705736, 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951): hub istemcisiyle ilgili madde bulunamadı (aranan: hub, HubLink, relink, heartbeat, bind, ROUTE_MISS, T3).
