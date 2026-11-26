# C2-runtime-2 — `crates/client-core/src/runtime.rs` 3301–6461 denetimi

Taban: ürün hattı `125b9afa` (satır numaraları bu tabandandır). Salt okuma; cargo koşulmadı, test koşulmadı.
Bütün davranış iddiaları kod okumasıdır; koşuyla doğrulanmadı.

## Kapsam

| dosya | satır aralığı | okunan satır |
|---|---|---|
| `crates/client-core/src/runtime.rs` | 3301–6461 (`client_task` gövdesinin ikinci yarısı: makrolar, ana döngü, kapanış; `ServerEndpoint`/`ServerHandle`; alıcı yardımcıları; `server_task` tamamı) | 3161 |
| bağlam için ayrıca bakılan (envantere girmez) | `runtime.rs` 73–86, 140–150, 261–387 (sabit/varsayılan satırları), 2040–2075, 3195–3210, 3234, 3278; `wire.rs` 1–14, 140–150, 205–215, 262–280, 365–380; `transport.rs` 810–830; `adaptive-transport/src/sender.rs` 6863–6890 | ≈ 170 |

Test modülleri 6462 ve sonrası bu kapsamın dışındadır (başka denetçi).

## Davranış envanteri

### A. `client_task` — durum ve makrolar (3301–3699)

- **Fragmenter durumu (3304–3307).** `frag_on = cfg.max_message_bytes.is_some()`; mesaj sayacı `next_message` (u32, sarar), `unit_msg` birim→mesaj, `msg_pending` mesaj→kalan chunk. Ürün taşıma katmanı fragmenteri reddeder (`transport.rs` `TransportBuildError::FragmenterOn`); yol yalnız doğrudan `RuntimeConfig` kullanan (bench) çağıranlarda açılır.
- **FEC blok birikimi (3314–3335).** Şerit başına açık blok (`cur_block`, `cur_block_id` = `first_fec_block_of_lane(i)`, `cur_block_wire`, `cur_block_ids`, `tail_copied`, `lane_last_offer_ns`). `repair_store` blok→(K_actual, onarımlar), `repair_order` giriş sırası; `REPAIR_STORE_BLOCKS = 64` (türetilmemiş tavan).
- **Birim deposu (3341–3350).** `store` birim→gövde (varlık = teyitsiz); `seq_to_unit` (yalnız teşhis sayacına uzunluk verir, hiçbir yolda okunmaz), `unit_seqs` (teyitte budama + kurtarma teşhisi).
- **PN çeviri tablosu (3356–3357).** `pn_seq` (PN, motor seq) artan; `max_sealed_pn`. Motor kararı olmayan çerçeveler (PARAM, CAPS, CS-probe) PN tüketir, tabloya girmez.
- **Gözlem sayaçları (3360–3366).** `ack_counts`, `ack_rx_types[5]`, `ptt_wire` (bayt, adet) — yalnız canlı aynaya yazılır.
- **Şerit sırası ve cephe (3368–3375).** `next_unit[lane]`; `unconfirmed[lane]` (BTreeSet); `publish_frontier` en küçük teyitsiz sırayı ya da sıradakini `PathEstimate::lane_frontier`'a yazar.
- **Zamanlayıcılar (3376–3378).** `pace` = `PACE_TICK` 1 ms, `rtprop` = `RTPROP_TICK` 200 ms; alış tamponu `CLIENT_RECV_BUF` 2048 B.
- **`seal_send!` (3386–3431).** Çerçeveyi `seam.seal(client_ctx, …)` ile mühürler; başarıda PN'i `max_sealed_pn`'e yazar, `tx_datagrams/tx_bytes` ve sınıf sayacını (Data/Retx/Dup/Fec/Ctrl) artırır; egress kapısı (`EgressPolicy::admit`) reddederse datagram çıkmaz, `egress_refused_*` sayılır; kabulde `UplinkShare::charge(n)` ve `sock.send`; gönderim hatası `send_errors`. Rate'e hiçbir yolda dokunmaz. Mühür reddinde `seal_failures`, değer `None`.
- **`map_pn!` (3433–3445).** (PN, seq) ekler; `PN_SEQ_WINDOW = 2^16` gerisini budar.
- **`unit_acked!` (3453–3473).** Fragmenter kapalı: `release_outstanding` + `acked_tx.try_send(unit)` (taşarsa olay düşer). Açık: mesajın kalan sayacını düşürür, sıfırda `messages_acked`, `release_outstanding`, mesaj kimliğini yollar.
- **`drain_confirmed!` (3486–3525).** Tek teyit yolu: `Sender::take_confirmed_units()`; birimin seq kayıtlarını budar; store'da yoksa atlar; PTT kayıtlarını siler; cepheyi yayımlar; PTT süresi dolmuşsa `ptt_expired` + `release_outstanding` (app olayı yok); değilse `ConfirmVia` türüne göre `fec_recovered_ctrl_units` ya da `acked`, sonra `unit_acked!`. Değer: düşen birim sayısı.
- **`drive!` (3527–3687).** Hat modeline yukarı yön birikimini verir (`cc_line_observe_backlog`), `session.on_event`, ardından teyit drenajı, yol kestirimi yayını (`btlbw_bps`, `rtprop_ns`, `send_queue_drained`). Kararlar: birim taşıyan karar store'daysa seq→birim kaydı; `FecParity` → kurtarma kapalıysa bastırılır (`recovery_suppressed`), onarım deposunda sembol yoksa atlanır (`fec_repair_missing` + `stale_skipped`), varsa `FEC_REPAIR` çerçevesi (`k_actual` başlıkta) mühürlenir; `CsProbe` → `encode_cs_probe` (PN tabloya girmez); öteki gövdesiz kararlar `stale_decisions`. Kurtarma kapalıyken Retransmit/Duplicate bastırılır. Birim kararı: PTT yalın kayıtlıysa `encode_ptt` (0x09), değilse `unit_frame`; gövde yoksa datagram üretilmez. PTT şeridi tel baytı (+28 B IP/UDP) gözlem sayacına.
- **`ts_sample!` (3690–3699).** `cfg.ts_sample_period_ns` açıksa, `TS_CAP` (4096) altında, periyot dolunca motor zaman serisi örneği.
- **`handle_rx!` (3702–4022).** `seam.open` başarısızsa `open_failures`; bağlam-önce yönlendirme (`route_client`): Hub → `hub.on_hub_frame`, NotReady/Mismatch → sayaç, yalnız Session yüke girer. `FRAME_CS_ECHO` → `Event::CsProbeEcho` (peer'in `seen/gap` alanları okunmaz). `FRAME_PARAM_ACK` → hash eşitse `param_confirmed` (yalnız yeniden denemeyi durdurur). `FRAME_RECOVERED` → ESI'ler `unit_of_fec(block, esi, K_shape)` ile birime → `Event::RecoveredControl` (yalnız listelenen birimler). ACK türleri: 0x10, 0x14 her zaman; 0x15/0x16 yalnız `ack_format == Bitmap`; 0x17 ayrıca `ptt_coalesce`; öteki `unknown_frames`. 0x15/0x16/0x17 ilk görülüşte `caps_confirmed`. Yalın PTT eşi: 0x16/0x17 görülünce (koşullar: `!frag_on`, şeritli, geri bildirim aç) `ptt_lean_peer`, ön-yalın kayıtlar devredilir, ileri kopya kuralı `(0,0)`. Yalın teklif varken `PTT_LEAN_FALLBACK_ACKS` (2) yalın olmayan ACK → eski kopya kuralına dönüş. Çözülen ACK: dönüş yolu gözlemi, ilk pencere `arrival_rate`, 0x15 `pair_rate`, kanıt aralığı (PN→seq en küçük/en büyük), ölçüm aralığı (`span_first..largest`, `rx_span × ACK_DELAY_UNIT_US`) → `note_ack_forward_interval`; PN'ler tabloyla seq'e çevrilip `Event::AckReceived` (ack_delay değiştirilmeden). Canlı ayna açıkken çerçeve teşhis kaydı. Çözülemeyen ACK sessizce düşer.

### B. `client_task` — ana döngü (4024–4499)

- **Canlı ayna (4041–4099).** `diag_emit_sink` varsa motorun emisyon kaydı; `live` varsa kontrol görüntüsü, `RECOVERY_DIAG_PERIOD` (200 ms) aralıklı kurtarma görüntüsü, durum değişikliği kaydı. Görev aynayı okumaz.
- **Kapanış (4101).** `shutdown_rx` → döngü biter.
- **t0 (4105–4120).** Egress kapısını kurar, tel sayaçlarını sıfırlar (motor durumu korunur; `send_errors`, `seal_failures` sıfırlanmaz), onay döner.
- **Sunum (4122–4324).** Yalnız `store.len() < max_inflight_units` iken alınır. Şerit `lane & 0b11`. PTT şeridi dışındaki son tarih `declare_delivery_deadline`'a gider. Mesaj `max_unit_bytes` chunk'lara bölünür; her chunk `tagged(lane, next_unit[lane])` kimliği, `SourcePrefix` (kapalıyken türetilmiş alanlar) ve `prefix‖payload` gövde; FEC sembolü gövde + sıfır dolgu (E). PTT: yalın eşte son tarihli birim `HDR_PTT` + `ptt_offered_at` + `mark_single_shot`; yalın teklif varken son tarihli birim ön-yalın kaydı; son tarihsiz PTT kaydı (açılış/kapanış) `PTT_RELIABLE_COPIES` (2) ileri kopya, aralık 20 ms, olmazsa bekleyen listesi (`PTT_COPY_PENDING_CAP` 16, taşarsa `ptt_copy_unprotected`). Bırakma anı `now + min(deadline, PTT_DEADLINE_NS 1 s)`. Store'a gövde, cepheye sıra, `Event::AppData{id, size=wire_len}`. FEC: blok dolunca (`k_shape`) `r_max` = yalın PTT şeridinde 0, değilse `fec_payload_r_max(...)`; manuel `r` ya da kontrolcü mandalı (`fec_controller_latch`, `fec_parity_payload_clamped` yalnız tavan kestiğinde); `planned_r > 0` ise `RsSimdCodec.encode` → `repair_store_insert(cap 64)`; blok sıfırlanır, sonraki blok kimliği.
- **`pace.tick` (4325–4397).** `Timer::Pto`; bekleyen PTT kopyalarını yeniden dener; süresi dolan yalın PTT birimlerini `abandon_units` + drenaj; `fec_starved_prior` varken şerit boştaysa (bütün gönderici boş ya da dosya dışı şeritte son sunumdan ≥ `LANE_IDLE_NS` 200 ms ve açık bloğun birimleri tele çıkmış) açık bloğun kopyalanmamış birimlerine `IDLE_LANE_TRANSMISSIONS − 1` (4) ileri kopya, aralık `IDLE_LANE_COPY_SPACING_NS` (40 ms).
- **`rtprop.tick` (4398–4491).** PARAM: `param_tries < 25` ve `param_retry_due` (200 ms'den ikiye katlanan, 5 s tavanlı geri çekilme; hat modeli varsa hat boşalması + taban PTO kapısı) ya da ileri kopya anı → `encode_param` mühürlenir (PN tabloya girmez), ilk denemede profil açıksa `param_first_ms`; hat modeli kuruluysa hat yüklenir, kapı güncellenir. CAPS: `!caps_confirmed && caps_tries < CAPS_MAX_TRIES (8)` ve aynı geri çekilme → bitler `(lane_ids, ptt_coalesce)` seçimiyle; ilk denemede iki kopya. Sonra `Timer::RtpropProbe`.
- **`sock.recv` (4492–4496).** Hata sessizce atlanır; başarıda `handle_rx!`.

### C. `client_task` — kapanış (4500–4716)

- **Kısmi/son blok kapanışı (4505–4582).** Her şeritte boş olmayan açık blok K_actual ile kapanır: `r_max` kuralı ana yolla aynı; kodlamaya `[K_actual, K_shape)` sıfır sembolleri katılır, tele çıkmaz; depo tavansız ekleme (en çok 64 + 4).
- **Kabul edilmemiş sunumlar (4585–4587)** `unaccepted_offers`'a sayılır.
- **Drenaj (4593–4611).** `flow_end_ns`; `drain_cap` süre tavanlı; yalnız `pace` ve `recv` kolları (PARAM/CAPS/RtpropProbe yok); çıkış: store boş ve `pif == 0` ya da süre.
- **Son teşhis (4612–4715).** `undelivered`, PTT tel sayacı, `FateWindowDiag` (motor teşhis alanlarının kopyası; durum adları dize).

### D. Sunucu API (4718–4938)

- `ServerHandle` alanları (teslim alıcısı, kapanış, t0, görev, adres, bağlam sayaçları, dört canlı ayna).
- `bind` → `bind_with_policy` → `bind_expecting(ExpectedSessions::only(IMPLICIT_SESSION))` → `bind_expecting_shared`: `validate_fec`, UDP bağlama, `delivery_queue` sınırlı kanal, t0 kanalı kapasite 1, görev başlatma.
- `local_addr`, `next_delivery`, `try_next_delivery`, `delivery_backlog`, `mark_t0` (görev işleyene dek bekler), `ctx_dispatch_stats`, `delivery_dropped` (hiç artmayan sayacın aynası), `data_not_admitted_full`, `data_not_admitted_window`, `fec_blocks_decoded`, `shutdown`.

### E. Alıcı yardımcıları (4940–5800)

- `DELIVERED_DEDUP_WINDOW = 8192`, `RX_BLOCKS_CAP = 32`, `MSG_ASM_CAP = 64` (türetilmemiş tavanlar).
- `busiest_lane` (eşitlikte büyük indeks), `fec_block_lane` (`>> FEC_BLOCK_LANE_SHIFT`), `rx_block_victim` (en kalabalık şeridin en küçük bloğu).
- `DedupWindow::{contains, insert}`: tavan aşımında en kalabalık şeridin en küçük kimliği düşer.
- `BlockAsm` (k, kaynaklar, onarımlar; BTreeMap = ESI tekilleştirme).
- `MsgAsm::insert`: `offset+len > cap` ya da `buffered > cap` → ret (ret yolunda `buffered` geri alınmaz); FINAL `total_len` kurar; aynı offset ilk-kazanır. `take_if_complete`: FINAL + boşluksuz kaplama.
- `push_delivery`: izinle teslim, `delivered` / `fec_recovered_units`.
- `permit_or_reserve`, `admit_chunk`: kapalıyken birim dedup → izin → teslim → dedup'a yazım → `note_admitted`; açıkken mesaj dedup, birleştirici, tamamlanınca izin ayrılmadan silmez; tamamlanmamışsa `msgs.len() > 64` iken `pop_first` (en küçük mesaj kimliği) tahliye.
- `PeerSlot` (alıcı motoru, son görülme, dedup, bloklar, mesajlar, bağlam, PARAM durumu, ön-PARAM bekletmesi, yankı aralığı, PARAM_ACK bekletmesi, ACK sayacı); `PARAM_ACK_HOLD_NS = 5 s`, `PARAM_ACK_AFTER_ACKS = 2`. `PeerSlot::new`: motor türüne göre `Receiver`, `quota_r`, dönüş bütçesi, yankı aralığı `min(budgeted_ack_interval_ns(bw), 30 s)` yalnız port + bütçe. `note_non_eliciting` yalnız bütçeli yolda. `param_ack_due`: bütçeli yolda bağlı oturumun yinelenen PARAM'ı en çok bir aralıkta bir yanıt.
- `retire_closed_slots`: beklenen kümede olmayan oturumların yuvaları.
- `PRE_PARAM_FEC_CAP = 64`; `fec_shard_fits` (tür ∈ {SOURCE, REPAIR}, profil, E, ESI aralığı, 1 ≤ k_actual ≤ K_shape).
- `CAPS_MAX_TRIES = 8`; `param_retry_due` (yukarıda); `PARAM_FORWARD_COPY_OFFSETS_MS = [200, 400, 800, 1600]`; `param_forward_copies_on` (geri bildirim aç profil ve PARAM serileşmesi < 200 ms); `PARAM_FORWARD_COPY_SLACK_MS = RTPROP_TICK/2`; `param_forward_copy_due`.
- `wire_len`: `sealed_len` reddinde düz boy.
- `SrvTx` (seam, egress, saat, yukarı yön, bekletilen denetim kuyruğu); `MAX_YIELDED_CTRL = 16`; `control_should_yield`, `line_ahead_ns`.
- `srv_send_or_yield`: hat veriye borçluysa RECOVERED kuyruğa (taşarsa en eskisi düşer, `yielded_ctrl_dropped`).
- `srv_ack_tick`: önce her yuvanın zamanlayıcı ACK'i (hat önü süresi ve yol verme bayrağı alıcıya), sonra bekletilen denetim, sonra süresi dolan ya da iki ACK sonrası PARAM_ACK.
- `drain_yielded`, `srv_seal_send` (mühür, egress kapısı, hat yükü, `send_to` hatası sayılmaz).
- `repair_store_insert`: giriş sırasına göre tahliye.
- `fec_ingest`: onarımda k_actual tutarlılığı (fail-closed, `unknown_frames`); blok K_shape ile açılır, onarım k'yı belirler; k'yı aşan kaynak onarımları geriye dönük siler; tam blok → silinir (boşa onarım sayacı); hazırsa `reconstruct(known_zero = K_shape − k)`; kurtarılan sembol prefix'iyle ayrıştırılır, şerit kabul sınırı dışındaysa atlanır, `admit_chunk` girerse ESI RECOVERED listesine; çözüm hatasında blok korunur; `RX_BLOCKS_CAP` aşımında tahliye.

### F. `server_task` (5802–6460)

- Kurulum: Hub dağıtıcısı, `SrvTx`, `frag_on`, `msg_cap`, FEC profili, PARAM beklenen alanları, yuva tablosu, `ACK_TICK` (20 ms), `PATIENCE_TICK` (1 s), `SERVER_RECV_BUF` 64 KiB, `idle_hold` (varsayılan 105 s).
- Döngü başı: dört sayaç aynaya.
- t0: egress kurulur, tel sayaçları sıfır.
- `ack_timer` → `srv_ack_tick`.
- `patience`: kapanmış oturum yuvaları + `idle_hold` boyunca sessiz yuvalar emekli (`retired_idle_peers`).
- Alış: `open` → bağlam-önce yönlendirme (`route_server`); çerçeve ayrıştırma sırası DATA, PTT, DATA_FRAG, FEC, CS_PROBE, PARAM, CAPS; bilinmeyen `unknown_frames`. Yeni yuva yalnız Session kolunda; tablo doluysa önce kapanmış yuvalar, yer yoksa `dropped_new_peers`. Yuvaya `ack_rereport`, teşhis bütçesi.
- PTT 0x09: kabul penceresinden alt 16 bitle tam sıra; bulunamazsa `unknown_frames`; her yalın çerçeve `mark_ptt_deadline`.
- DATA/DATA_FRAG: kabul penceresi dışı → motora verilmez (`data_not_admitted_window`); kanal yeri yoksa motora verilmez (`data_not_admitted_full`); motor (`RxKind::Data`, PN kimlik) → yeni birimse (PTT şeridi güvenilir kaydı `mark_ptt_data_framed`) `admit_chunk` (dönüş değeri okunmaz); ACK mühürlenir; FEC açıksa kaynak sembol türetilir (0x00) ya da gövdeden (0x05; E'yi aşarsa `unknown_frames`) → `fec_ingest` → RECOVERED (`srv_send_or_yield`).
- FEC: profil yoksa `unknown_frames`; PARAM bağlı değilse uyan shard bekletilir (64'e kadar, taşan `fec_pre_param_dropped`), uymayan sayılır, motor-ACK her ikisinde verilir; bağlıysa uymayan shard motora girmeden düşer; uyan → motor → ACK → `fec_ingest` → RECOVERED.
- CS_PROBE: PN `note_non_eliciting`; her probe'a durumsuz yankı (`seen=0, gap=0`), hız sınırı yok.
- CAPS: bitmap yalnız eş bildirdiyse ve `ack_format == Bitmap`, port motoru, profil varsa; çift sıkışma hattı aşağı yön; 0x16 / 0x17 türleri; yalın PTT başlangıcı `mark_ptt_deadline`.
- PARAM: PN `note_non_eliciting`; FEC kapalı uçta `unknown_frames`; `ParamState::on_param_set` → bağ (ilkse `param_bound_peers`), PARAM_ACK (bütçeli yolda seyreltme ve ilk iki ACK'e dek bekletme, en geç 5 s), bekletilen shard'lar boşaltılır; uyuşmazlıkta yalnız `param_rejects`.

## Bulgular

| fid | tür | önem | yer | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| C2R2-01 | UYUŞMAZ | yüksek | 5879–5891, 6007–6020, 6221–6259; istemci 3750–3752, 4417–4422, 4455 | Sunucu, `idle_hold` (105 s) sessiz kalan yuvayı emekliye ayırır; oturum açık kalır. Geç DATA yeni yuva açar: PARAM bağı, CAPS/bitmap seçimi, dedup penceresi ve FEC blokları sıfırlanır. İstemci `param_confirmed` / `caps_confirmed` sonrasında PARAM ve CAPS'i bir daha göndermez → yeni yuvada onarımlar ön-PARAM bekletmesinde kalır (64'e kadar, hiç çözülmez), ACK'ler 0x14'e döner (0x15 ölçüm aralığı ve çift ölçüsü kaybolur). Kod okumasıyla; koşuyla doğrulanmadı. | transport-layer-tdd §0 INV-NO-IDLE-TEARDOWN: «Bir oturum ya da bekleyen niyet, ZAMAN GEÇTİĞİ İÇİN sonlandırılmaz»; noise-wire §8 PARAM bağı oturum başınadır. 105 s değeri spec'te bulunamadı (aranan: SESSION_IDLE_HOLD; yalnız engine-spec ZM-3 kaydında [CAL]-aday). | karar-gerekir |
| C2R2-02 | EKSİK | orta | 3386–3431 (`seal_send!`) | Hop-ACK almadan ardışık tüketilen PN için tavan yok; PreConverge kopyası, CS-probe, RecoverFwd kopyası ve PARAM/CAPS sınırsız PN yakabilir. | transport-layer-tdd §12 [AMENDMENT-9] + «KS-PN-1 KAPANDI — K = 1024»: «hop-ACK almadan 1024 PN tüketilince yeni Session-ctx emisyonu DURUR». Spec kendisi SPEC-ONLY diyor (uygulama ayrı iş kalemi, profil kimliği `transport-v1.x`). | kod-duzeltmesi |
| C2R2-03 | UYUŞMAZ | orta | 3737–3793, 5967–5992 | Session bağlamındaki düz metnin ilk baytı v0 çerçeve türü: 0x00 DATA, 0x01 FEC, 0x02 RECOVERED, 0x03 PARAM, 0x04 PARAM_ACK, 0x05 DATA_FRAG, 0x06/0x07 CS, 0x08 CAPS, 0x09 PTT, 0x10/0x14–0x17 ACK. Kayıt defterindeki anlamlar farklı (0x02 = FEC_REPAIR, 0x12 RECOVERED, 0x14 PARAM_SET, 0x15 PARAM_ACK, 0x16 BLOCK_ABORT, 0x17 SESSION_CLOSE, 0x19/0x1A CS; 0x00 ve 0x03–0x0F reddedilir). `wire.rs` başlığı bunu S2 değişimine bırakılmış v0 kabuğu olarak açıklıyor. | noise-wire §2.56 «INNER_KIND-REGISTRY [NORMATİF]» + «Bilinmeyen-inner_kind → FAIL-CLOSED-reject» | karar-gerekir |
| C2R2-04 | TANIMSIZ | orta | 3784–3826, 4452–4489, 6330–6365 | ACK bitmap çerçeveleri 0x15/0x16/0x17 ve CAPS 0x08 (bitler 0x1/0x2/0x4), CAPS ilk denemede iki kopya, `CAPS_MAX_TRIES = 8`, bitmap açma koşulu (`Bitmap` + port + profil). Sahip, birleşik kabulde bitmap'i adıyla anıyor (7171705736: «iki cc-port uç, bitmap…»), ama tel biçimi hiçbir donmuş kayıtta yok. | Spec'te bulunamadı (aranan: engine-spec §1.1 frame_type tablosu yalnız 0x10, 0x11, 0x13, 0x14, 0x1B, 0x1C, 0x20; §2.2 «0x14-0x1F boştu»; noise-wire §2.56). | karar-gerekir |
| C2R2-05 | UYUŞMAZ | orta | 5207–5210, 5060–5078, 6137–6150 | Fragmenter açıkken chunk alıcı motorunca ACK'lendikten sonra birleştiriciye girer. Tamamlanmamış mesaj sayısı 64'ü aşınca `pop_first` en küçük mesaj kimliğini (sarmadan sonra en yenisini) siler; ACK'lenmiş chunk'ları göndericide yeniden gönderilmez → mesaj kalıcı kaybolur. `MsgAsm::insert` reddi de ACK'lenmiş chunk'ı düşürür (`admit_chunk` dönüşü okunmuyor). Ürün taşıma katmanı fragmenteri reddettiğinden yol yalnız bench'te erişilir. | Sahip kararı 2026-11-11 (kodda 6064–6065 alıntı): «ACK'lenmiş birim kaybolmaz»; INV-ACK-DELIVER-1 (F3/F4) | kod-duzeltmesi |
| C2R2-06 | EKSİK | düşük | 6312–6327 | CS-probe yankısı her probe'a üretilir; oturum başına hız sınırı yok (bench `udp_runner` 500 ms uyguluyor, ürün sunucusu uygulamıyor). Amp ≤ 1 korunuyor. | noise-wire §2.57 CTRL_CS_ECHO: «rate-limit T_ECHO_MIN/akış»; değer engine-spec ZM-3'te [CAL]-aday | karar-gerekir |
| C2R2-07 | EKSİK | düşük | 6386–6452 | Hash uyuşmazlığında yalnız `param_rejects` artar; SESSION_CLOSE yok (v0 telinde de yok), gönderici 25 denemede durur, ön-PARAM shard'ları bağlanmadan kalır. | noise-wire §8 STATE-MACHINE: «yalnız-KALICI-hash-mismatch → SESSION_CLOSE»; §2.57 PARAM_SET satırı | karar-gerekir |
| C2R2-08 | UYUŞMAZ | düşük | 6162–6186, 5655–5679 | Kaynak semboller DATA çerçevesiyle gider ve K_actual taşımaz; alıcı K_shape varsayar, onarımın k'sını aşan kaynak gelince onarımları geriye dönük siler. | fec-core §6 PATCH-4: «K_actual HER-FEC-source-ve-repair-shard'ın … inner-header'ında-AÇIKÇA-taşınır … dolaylı-türetime-BAĞIMLI-DEĞİLDİR» | karar-gerekir |
| C2R2-09 | TANIMSIZ | düşük | 4400–4451, 5363–5426, 5249–5256, 6391–6411, 5449–5451 | Duvar saati sabitleri kanal zamanından türetilmemiş ve saat-sınıfı envanterinde yok: PARAM geri çekilmesi 200 ms ×2 → 5 s, 25 deneme (satır içi sayı), PARAM ileri kopyaları 200/400/800/1600 ms + 100 ms pay, PARAM_ACK bekletmesi 5 s / 2 ACK, `MAX_YIELDED_CTRL = 16`. Gerekçeler #175 yorumlarına atıflı; o yorumlar bu denetimin otorite listesinde yok. | engine-spec §16.2 ZM-2: «RTT/kanal-zamanı ölçen her eşik srtt/rtprop/serialization-türevi olmak ZORUNDA»; ZM-3: «Zaman ölçen her YENİ mekanizma bu tabloya saat-sınıfıyla eklenir». noise-wire §8: «bounded-retry … until PARAM_ACK-veya-deadline» (sayı yok). | karar-gerekir |
| C2R2-10 | TANIMSIZ | düşük | 4493, 4606, 5893, 5575, 3857, 4273, 4527, 3597/4283/4311/5636/6187 | Sessiz yollar: alış hatası sayaçsız `continue`; sunucu `send_to` hatası sayılmaz (istemci `send_errors` sayar); çözülemeyen ACK `unknown_frames`'e girmez; `close_fec_block_manual(...).unwrap_or(0)` `FecAuthorityError`'ı yutar; `u8::try_from(k_shape).unwrap_or(255)` geri dönüşü. | CLAUDE.md §9 «açık hata tipleri»; bu denetimin TANIMSIZ tanımı (sessiz geri dönüş) | kod-duzeltmesi |
| C2R2-11 | ÖLÜ | düşük | 5096–5099, 5857, 4906–4908; 3349, 3566 | `ServerStats::delivery_dropped` hiçbir yolda artmaz (yorum bunu yazıyor), ama her turda aynaya yazılır ve `ServerHandle::delivery_dropped` ile sunulur (daima 0). `seq_to_unit` yazılır, hiçbir kararda okunmaz (yalnız uzunluğu teşhise). | CLAUDE.md §6: «Deprecated içerik → appendix'e taşı + "MUST NOT IMPLEMENT"» | karar-gerekir (genel API yüzeyi pinli) |
| C2R2-12 | UYUŞMAZ | düşük | 5339–5340, 6226–6230 | `PRE_PARAM_FEC_CAP = 64`; yorum «MAX_PREPARAM_SHARDS hizası» diyor, oysa `kama-engine` `MAX_PREPARAM_SHARDS = 256`; oturum başına toplam soğuk tavan yok. Aşım reddi sayaçlı (A-1 ile uyumlu). | noise-wire §8 DR-CS-1b-A2 A-1: «RX_PREPARAM_SHARD_CAP_PER_SESSION = min(RX_COLD_TOTAL_SHARD_CAP_PER_SESSION, MAX_PREPARAM_SHARDS)» | karar-gerekir |
| C2R2-13 | TANIMSIZ | düşük | 4268–4274, 4522–4527 | Manuel FEC kipi (`fec_manual_r`, yalnız bench/örnek) `r_max`'ı uygulamaz: yalın PTT şeridinde onarım yok kuralı ve kaynak baytı tavanı atlanır. | Spec'te bulunamadı (aranan: manuel r ile parite tavanı ilişkisi); PTT kuralı karar 3 kapsamında | karar-gerekir |
| C2R2-14 | TANIMSIZ | orta | 3654–3662, 3827–3851, 4126–4134, 4177–4236, 4260–4266, 4328–4355, 4517–4521, 6024–6050, 6124–6136 | Yalın PTT politikası: 0x09 çerçevesi, yalın eş geçişi, 2 yalın olmayan ACK'te eski kopyaya dönüş, tek iletim (`mark_single_shot`), `min(deadline, 1 s)` bırakma + `abandon_units`, güvenilir kayda 2 kopya / 20 ms / bekleyen listesi 16, yalın PTT şeridine onarım yok, PTT son tarihinin oturum son tarihine girmemesi, alıcıda alt-16-bit açma ve geç güvenilir işaret; `ptt_coalesce` (0x17, varsayılan kapalı) deney kolu. Spec'te tanımlı değil. | Spec'te bulunamadı (aranan: 0x09, PTT, yalın, `PTT_DEADLINE`); sahip kararı 7171705736 madde 3: «PTT ve metin davranışını dondurun» | kilitli |
| C2R2-15 | TANIMSIZ | düşük | 4356–4396 | Boştaki şeridin açık kısmi bloğuna ileri kopyalar: `LANE_IDLE_NS = 200 ms`, `IDLE_LANE_TRANSMISSIONS = 5`, 40 ms aralık, dosya şeridi hariç. Yorum sahip kararı 2026-11-16'ya atıf yapıyor; CLAUDE.md YAMA KAYDI 2026-11-16 bu kopyayı adıyla anmıyor (DOĞRULANAMADI). Metin gönderim politikasıdır. | sahip kararı 7171705736 madde 3 (metin gönderim politikası donmuş); engine-spec ZM-2 | kilitli |
| C2R2-16 | DOGRULANAMADI | düşük | 5492–5548, 5468–5488 | ACK zamanlayıcısında önce ACK, sonra hat boşken bekletilen RECOVERED; RECOVERED veriye yol verir. Bunun karar 7'nin «Sınırlı ACK önceliği ayrı transport denemesi» kapsamındaki K10 mı, yoksa onun öncesindeki ürün davranışı mı olduğu bu okumadan belirlenemedi. | sahip kararı 7171705736 madde 7 | karar-gerekir |

Uyumlu davranış sayımı (bölüm başına; tek tek yazılmadı):

- A–C `client_task`: **uyumlu 24.** En önemlileri: INV-RATE-1 (egress reddi / gönderim hatası kayıp sayılır, rate'e dokunulmaz); INV-FEC-1 (sembol düz metin `prefix‖payload‖sıfır`, her onarım ayrı mühür; kodla-sonra-şifrele); K4G-05b-R sahip kararı 2026-11-11 (payload yalnız motor teyidiyle düşer; gövdesiz karar datagram üretmez, PN harcanmaz); PARAM_ACK yalnız onay, emisyon onayı beklemez (noise-wire §8 B2); CS-echo `seen_count/arrival_gap` karar girdisi değil (§2.57); ACK kimliği `full_wire_pn` (engine-spec K3 yaması, transport-layer YAMA 2026-10-06); `ack_delay` değiştirilmeden motora (karar 2); Model-A sıfır dolgu kodlamaya girer, tele çıkmaz (fec-core §6); bağlam-önce dağıtım (K4G-01); drenaj süre tavanlı (D8); sınırlı tamponlar (`TS_CAP`, onarım deposu 64, `max_inflight_units`).
- D sunucu API: **uyumlu 10** (bağlama typed hata, sınırlı teslim kanalı, t0 sözleşmesi, salt okuma aynalar).
- E yardımcılar: **uyumlu 14.** Sınırlı dedup / blok / mesaj tavanları (SR-AUDIT-001); `fec_shard_fits` bilinmeyen türü reddeder (§2.56 fail-closed); onarım k_actual tutarlılık kapısı ve 1 ≤ K_actual ≤ K_shape (§8 B4); bilinen sıfır enjeksiyonu (fec-core §6); çözüm hatasında blok korunur; RECOVERED yalnız teslime giren birimi listeler (INV-ACK-DELIVER-1 F4); tamamlanan mesaj izin ayrılmadan silinmez.
- F `server_task`: **uyumlu 13.** Kanal yeri ve kabul penceresi motordan önce (F3/F4); ön-PARAM sınırlı bekletme, taşma sayaçlı ret (§8 A-1); doğrulamadan önce çözme yok (AEAD `open` → ayrıştırma → FEC; authenticate-before-decode); CS yankısı < probe (amp ≤ 1); PARAM fail-closed doğrulama, uyuşmazlıkta ACK yok; yuva tablosu tavanı fail-closed; `unreachable!` / `expect` yalnız yapısal olarak erişilemeyen dallarda; varint ve `todo!()` yok.

## Okunan otorite bölümleri

- Sahip kararları (#175): 7171705736 (dokuz karar; madde 2, 3, 7 bu kapsamla ilgili), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (bu satır aralığına doğrudan dokunan yok; LineGuard / `receiver.rs` kapsamında).
- `docs/decisions/KARAR-KAYDI-2026-11.md` §3, §4, §8 (D-CC-1).
- CLAUDE.md §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §6, §8, §9, YAMA KAYDI.
- `specs/kama-noise-wire-tdd-v2.0.md` §2.56, §2.57, §8 (B2, B4, A-1).
- `specs/adaptive-transport-engine-spec.md` YAMA KAYITLARI (2026-11-21, 2026-10-04b), §1.1, §2, §2.1, §2.2, §16.2 ZM-1..4 ve ZM-3 kaydı.
- `specs/transport-layer-tdd.md` YAMA KAYITLARI, §0 INV-NO-IDLE-TEARDOWN, §12 [AMENDMENT-8], [AMENDMENT-9], KS-PN-1.
- `specs/fec-core-tdd-v3.0.md` §6 (Model-A, PATCH-4).
- `specs/identity-and-trust-model.md` §28.1-A (PN terim tablosu; PARAM_SET ≤ 25 yalnız ölçüm kaydı).
- `specs/application-protocol-and-rpc-spec.md`, `specs/signaling-seam-v0.md`: bu satır aralığındaki davranışlarla eşleşen bölüm bulunamadı (aranan: ACK, PARAM, FEC, teslim kanalı; app-rpc teslim bilgisini AMP RECEIPT'e bağlar, hop ACK'ten türetmez).
- `specs/ccr-format-spec.md`: bu aralıkta ccr §35 SESSION_DATA çerçevesi kullanılmıyor (v0 kabuğu; `wire.rs` başlığı). Ayrıca okunmadı.
