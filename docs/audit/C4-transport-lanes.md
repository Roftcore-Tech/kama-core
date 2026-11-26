# C4-transport-lanes — S1 adaptörü (`transport.rs`) ve ince şeritler (`lanes.rs`) davranış denetimi

Taban: ürün hattı `125b9afa` (yerel/kod-denetimi). Salt okuma; kod değiştirilmedi, cargo koşulmadı.
Satır numaraları `125b9afa`'dandır.

## 1. Kapsam

| dosya | okunan aralık | satır |
|---|---|---|
| `crates/client-core/src/transport.rs` | 1–2576 (tamamı; test modülü 2330–2576 dahil) | 2576 |
| `crates/client-core/src/lanes.rs` | 1–1375 (tamamı; test modülü 819–1375 dahil) | 1375 |
| **toplam** | | **3951** |

Karşılaştırma için ayrıca okunan (denetim kapsamı dışı, yalnız sözleşme doğrulaması):
`crates/kama-contracts/src/s1.rs` 1–300; `crates/client-core/src/runtime.rs` 535–560, 2830–2960, 3440–3520, 4320–4375
(uç tutamacı erişimcileri, teyit akışı, PTT bırakma); `crates/client-core/src/unit_id.rs` 1–60;
`crates/adaptive-transport/src/receiver.rs` 83–139; `crates/adaptive-transport/src/sender.rs` 1791–1817;
`crates/kama-noise-wire/src/session_control.rs` 82–85; `crates/client-core/src/hub/link.rs` 600–660, 1070–1100.

## 2. Davranış envanteri

### 2.1 `transport.rs`

**Modül belgesi (1–103).** S1 `KamaTransport`'un gerçek adaptörü: veri yolu = oturum başına bir `ClientEndpoint`,
düğüm başına bir `ServerEndpoint`; oturum kontrolü = S3 `SessionControl`. Direct-mode topoloji; INIT/RESPONSE
zarfları `EnvelopeLink` üzerinden. Senkron S1 yüzeyi, her çağrıda `drive()`.

**Sabitler.**
- `PENDING_INIT_CAP = 16` (136–138): ccr §35.15 KS-DOS-1'den; test 2335–2341 noise-wire `PENDING_RESPONSE_TRIAL_CAP` ile eşitliği sabitler. Türetilmiş.
- `MAX_EVENTS = 256` (140–142): okunmamış S1 olayı tavanı; türetme yazılı değil.
- `MAX_INBOX = 1024` (144–147): teslim kutusu tavanı; türetme yazılı değil.
- `PTT_SKIPPED_CAP = 256` (533–534), `PTT_HOLE_SKIP = 100 ms` (536–542, PTT demet aralığının yarısı), `PTT_LANE = 0` (543–544).
- `UNIT_TRACE_CAP = 65 536` (692–695, yalnız test-util).

**`receiver_buffer_bound_units` (149–179).** Alıcı tarafı tampon üst sınırı = `delivery_queue + MAX_INBOX + sessions × Σ admit_widths`
(şeritler kapalıyken `max_inflight_units`). Saf hesap, durum değiştirmez. Ürün değeri tek oturumda 6 912 birim (doc ile aritmetik tutarlı: 1024+1024+768+4096).

**Zarf taşıyıcısı tipleri (181–281).** `LinkAttempt`, `LinkEvent{HandleAssigned, IncomingInit, Response}`, `LinkError`,
`EnvelopeLink` trait (`send_init`, `send_response`, `poll`, varsayılanı boş `cancel_init`), `RelinkOutcome{Started, Nothing, Deferred, Stale}`,
`ActiveLink{NoSession, Unknown, On(h)}`.

**Yapılandırma (283–347).** `ClientTransportConfig{runtime, data_bind, peer_addrs, recovery, renew_drain_cap, lanes}`;
`product()` → `RuntimeConfig::default()`, `recovery = true`, `renew_drain_cap = runtime.drain_cap` (3 s), `lanes = Some(LaneConfig::default())`.
`TransportBuildError{RecoveryOff, FragmenterOn, Runtime, Io, Lanes}` (thiserror).

**Crypto dikişleri (349–427).** `lock()` zehirli Mutex'te panik etmez, içeriği okur. `CryptoSeam` ortak Crypto'ya yönlendirir.
`EndpointSeam` aynı yönlendirme + (yalnız test-util) ilk emisyonu artan kimlikte işaretli birimin son baytını bozarak kaybettirir.

**Durum tipleri (429–531).** `Phase{Active, Superseded, Closed}`; `Sess{peer, phase, endpoint, sent[4], link, drop_ids}`;
`Intent{ticket, peer, pending, early, attempt}`; `Renewal{old, peer, pending, held, attempt}`;
`Reorder{lanes[4] LaneOrder, ptt_skip}`; `LaneOrder{next, held BTreeMap, hole_since, skipped BTreeSet}`.

**`Reorder::push_bounded` (579–602).** Birimi şeridine (kimliğin üst 2 biti) koyar. `seq < next` ise geç kopya: PTT şeridinde,
DATA-çerçeveli ve `skipped` kümesindeyse sırasız teslim edilir (`next` değişmez), aksi hâlde yok sayılır. Değilse `held`'e
(ilk kopya kazanır) ve `release`. Döner: şeridin yeni etiketli sıra başı.

**`Reorder::release` (606–641).** `out.len() < cap` oldukça sıra başı geldiyse teslim eder. Sıra başı eksikse: PTT dışında ya da
`ptt_skip` kapalıyken bekler; PTT + `ptt_skip` iken `held_n ≥ 2` ya da `hole_since` 100 ms'yi geçtiyse deliği atlar,
atlanan sıraları `skipped`'a ekler (≤ 256, en küçüğü düşer). Saat `Instant::now()`.

**Telemetri erişimcileri (910–1036).** `data_addr`, `add_peer_addr` (sınırsız HashMap girişi), `ctx_dispatch_stats`,
`delivery_dropped`, `data_not_admitted_full`, `data_not_admitted_window`, `fec_blocks_decoded`, `peer_ptt_coalesce`,
`ptt_coalesce_enabled`, `session_live_diags`, `retired_live_diag`, `reorder_diags`. Hepsi salt okuma.

**`drive` (1038–1047).** Sıra: `drive_link` → `release_held_renewals` → `drive_crypto_events` → `drive_deliveries` → `sync_expected` → `pump_lanes`.

**`sync_expected` (1049–1080).** Veri alıcısının beklenen kümesini yeniden yayımlar: bekleyen INIT/yenileme/provisional/yanıtlayan → `NotReady`;
`phase != Closed` oturumlar → `Ready` (sonra eklendiği için kazanır).

**Zarf yolu (1092–1211).** `drive_link` olay tavanı doluncaya dek `poll`: `HandleAssigned` → `AssignHandle`, başarıda `hub_of`;
`IncomingInit` → `AcceptInit`; `EstablishedAndReply` → hat bağı + `responder_peer` + `insert_pending` + `send_response` (hata yok sayılır);
`ProvisionalAndReply` → `provisional` + `insert_pending` + `send_response` (hata yok sayılır); diğer sonuçlar sessiz.
`on_response`: bekleyen yenilemeyle eşleşirse RESPONSE tutulur (`held`), değilse `AcceptResponse`.
`release_held_renewals`: eski oturumun ucu `outstanding() == 0` VE şeritleri boşsa ya da `renew_drain_cap` dolduysa tutulan RESPONSE'u Crypto'ya verir.

**Crypto olayları (1215–1319).** `on_crypto_ready`: (a) kendi yenilemesi → eski `Superseded`, eşin etkin oturumu varsa yenileme terk edilir, yoksa `activate`;
(b) niyet → `activate(early)`; (c) yanıtlayan → aynı eşin niyeti varsa INIT'i terk edilir, `early` devralınır, `activate`.
`on_confirmed`: provisional onayı → `old` için bekleyen yenileme terk edilir, `old` `Superseded`, eşin etkin oturumu varsa yeni kapatılır, yoksa `activate`.
`on_renew_required`: yalnız `Active` için `RenewRequired` olayı (oda denetimi yok) + yenileme yoksa `start_renewal`.

**Yenileme yardımcıları (1321–1399).** `start_renewal`: `Renew` → `insert_pending` → `send_init`; taşıyıcı reddederse bekleyen kapatılır, `Deferred`.
`close_unopened`, `abandon_renewal` (`cancel_init` + kapat), `drop_intent_init`, `active_session_of`, `active_link`.

**Hat kaybı (1401–1470).** `relink`: etkin oturum yoksa ya da yenilemesi bekliyorsa `Nothing`, değilse `start_renewal`.
`relink_line`: kaybolan hat etkin hattan farklıysa `Stale`, değilse `relink`. `restart_renewal`: RESPONSE'u gelmemiş yenilemeyi terk edip yenisini başlatır.

**`on_crypto_closed` (1472–1536).** (1) niyetin bekleyeni düştü → deneme iptal, `reinitiate` (taze INIT); (2) yenilemenin bekleyeni düştü → iptal, eski oturum sürer;
(3) provisional → sessiz; (4) yerelde kapatılan → yutulur; (5) S1 oturumu → `Active` ise `Closed`, uç bırakılır, `Closed` ise reorder silinir,
`Closed(h, reason)` olayı, eski oturumun yenilemesi terk edilir. `Superseded` evrede reorder `Closed` okunana dek kalır.

**`reinitiate` (1538–1567).** `Initiate{early_unit: None}`; başarıda `insert_pending` + `send_init(...).ok()` (taşıyıcı hatası sessiz: deneme `None`);
başarısızsa niyet `ConnectAbandoned(LocalPermanent)`.

**`activate` (1569–1623).** Eşin diğer provisional'larını kapatır; `open_endpoint`; `early` varsa TEXT şeridine (`lane_of(Be)`) doğrudan `send_in_lane`
(şerit katmanını atlar), başarıda `sent[lane] = 1`; `Sess` eklenir; `Ready(h, peer)`. Uç kurulamazsa oturum ucu olmadan eklenir, `early` düşer.

**`open_endpoint` (1625–1654).** Eş adresi yoksa ya da bağlantı kurulamazsa `None` (hata yutulur); kurulursa kurtarma anahtarı uygulanır.

**`drive_deliveries` (1667–1739).** Önce kutuda yer varsa her oturum/şerit için `release` (kutu `MAX_INBOX`'u aşmaz), sıra başı ilerledikçe
`advance_head`. Sonra teslim kanalından `try_next_delivery`: `Session(h)` değilse düşer; oturum yoksa Crypto olayları sürülür; `Closed`/bilinmeyen oturumun
teslimi düşer; PTT şeridinde DATA-çerçeveli işaret tüketilir; `push_bounded`; ilerlemede `advance_head`.

**Şerit kancası (1741–1925).** `lane_stats`, `lane_backlog` (salt okuma).
`send_via_lanes`: S1 hata denetimleri (bilinmeyen/supersede/kapalı, `UnitTooLarge`, geçmiş deadline → `DeadlineExpired`, uç yok → `NotConnected`),
sonra şeride ekler (tavan → `Backpressure`), başarıda o oturumun pompası. `pump_lanes`: her uçtan `try_acked` boşaltma, etkin olmayan oturumların şeritlerini
düşürme, iş bekleyen oturumları pompalama. `pump_session`: dosya penceresi = `file_window_for_path(file_window_floor(taban, birim, profil_penceresi), birim, btlbw, rtprop)`;
profil penceresi ve `link_rates` varsa motor birikim yeri = `file_backlog_bound_units − (Σ sent − send_queue_drained)`; açıklık = `min(lane_confirm_frontier, sent) + width − sent`.
`pump_lanes_of`: `pop_next` → `send` gövdesi; `Ok` → `on_given`, `Backpressure` → başa geri ve dur, `DeadlineExpired` → sayaç, diğer hata → `give_failed` (birim düşer).

**test-util kancaları (1927–2069).** Kurtarma anahtarı, canlı ayna, birim izi, ilk emisyon kaybı, gözlem erişimcileri. Ürün yolunda çağrılmaz.

**`session_epoch` (2072–2094).** Ürün kriptosunda gönderim DH epoch'u; belgede "per-INIT ephemeral confidentiality" etiketi (CLAUDE.md FS kuralına uygun).

**`KamaTransport` impl (2096–2328).**
- `connect`: etkin oturum → `Ready(h)` (erken birimle `EarlyUnitConflict`); bekleyen niyet → aynı `Pending` (erken birimle `EarlyUnitConflict`);
  erken birim tavanı → `UnitTooLarge`; niyet sayısı ≥ 16 → `Backpressure`; yeni bilet + `ConnectProgress(Measuring)`; eş adresi yoksa `ConnectAbandoned(LocalPermanent)`;
  `Initiate` başarılıysa `insert_pending` + `send_init(...).ok()` + niyet kaydı; başarısızsa `ConnectAbandoned(LocalPermanent)`.
- `send`: şeritler açıksa `send_via_lanes`; değilse hata denetimleri, `DeadlineBounded` → kalan ns, birim kimliği `tagged(lane, sent[lane])`, `send_in_lane`,
  `QueueFull` → `Backpressure`, diğer → `NotConnected`; başarıda `sent[lane] += 1`. `CompressionPolicy` uca geçirilmez.
- `recv` / `poll_event`: `drive` sonrası kutudan / olay kuyruğundan; `Closed(h)` okununca `sessions` ve `reorder` silinir, `sync_expected`.
- `close`: `Active` değilse typed hata; `Closed` evresi, uç bırakılır, reorder silinir, Crypto `Close`, yenileme terk, `Closed(h, UserClosed)` (oda denetimi yok).
- `cancel`: niyeti kaldırır, Crypto `Close`, INIT tekrarı durur, `ConnectAbandoned(Cancelled)` (oda denetimi yok).

**Testler (2330–2576).** Pending tavanı = trial cap; sıralayıcı atlamasız sıra; PTT son tarih kipinde delik atlama (yalnız açıkken);
PTT geç güvenilir kaydın bir kez sırasız teslimi; 10 000 tutulan birimde atlama yok; şeritlerin bağımsız sıralanması; taze ürün adaptöründe salt-okunur erişimciler.

### 2.2 `lanes.rs`

**Modül belgesi (1–41).** Oturum başına dört şerit; sınıf → şerit yalnız S1 parametresinden; şerit içi FIFO; kabul sırası PTT > SIG > TEXT > FILE;
FILE pencereli; tavanlı (SR-AUDIT-001); şerit başına kabul genişliği; profil açıkken motor birikim sınırı.

**Sabitler (49–104).**
- `DEFAULT_FILE_WINDOW_UNITS = 5` (v4 K6; KK-T1-09 ile alt sınır 4).
- `FILE_WINDOW_BDP_GAIN = 2` (BBR cwnd_gain'den gerekçeli), `MAX_FILE_WINDOW_UNITS = 1024`.
- `FILE_BACKLOG_MIN_UNITS = 2` (gerekçeli), `FILE_BACKLOG_HORIZON_NS = 40 ms` (`kama-node` `NODE_TICK`'in iki katı — başka crate'in iç sabitinden).
- `DEFAULT_RT_ADMIT_WIDTH = 256`, `DEFAULT_FILE_ADMIT_WIDTH = 4096` (= `max_inflight_units`), `DEFAULT_ADMIT_WIDTHS`.
- `DEFAULT_PTT_CAP = 64`, `DEFAULT_SIG_CAP = 64`, `DEFAULT_TEXT_CAP = 256` (`[KALİBRASYON]`), `GOSTER_BOTTLENECK_BYTES_PER_S = 12 500`, `GOSTER_RTT_MS = 300` (yalnız testte okunur).

**`Lane` (106–142).** Dört varyant; `of`: `Vo|VoRel → Ptt`, `Sig → Sig`, `Be|Ctrl → Text`, `Bk → File`; `index` 0..3.

**`LaneConfig` (144–214).** `with_file_window(W)`: tavanlar varsayılan, FILE tavanı `2·W`. `validate(offer_queue)`: W ≥ 1, tavan ≥ 1, genişlik ≥ 1,
Σ tavan ≤ `offer_queue` (typed `LaneConfigError`). `Default` = W 5.

**`file_window_for_path` (244–267).** `clamp(⌈2·btlbw·rtprop/unit⌉, min(1..1024), 1024)`; ölçüm yoksa taban.
**`file_window_floor` (269–297).** Profil penceresi varsa `max(yapılandırma, ⌈profil/unit⌉ ≤ 1024)`, yoksa yapılandırma.
**`file_backlog_bound_units` (299–322).** `clamp(⌈uplink·40 ms/unit⌉, 2, 1024)`.
**`file_window_covers_bdp` (324–336).** `W·S ≥ BDP` (yalnız testte okunur).

**`QueuedUnit`, `LaneFull`, `LaneStats` (338–383).** Birim S1 argümanlarını aynen taşır (bayt okunmaz); sayaçlar salt gözlem.

**`SessionLanes` (385–642).**
- `push`: tavan denetimi eklemeden ÖNCE; doluysa `LaneFull`.
- `push_front`: tavansız başa dönüş; birikim kipi açıkken FILE birimi bekletme kuyruğuna.
- `pop_next`: kabul sırasıyla; FILE birikim kipinde önce `admit_file` (pencere + açıklık), yer 0 ise bekletir, değilse bekletme kuyruğunun başını verir
  (pencere yeniden denetlenmez); kip kapalıyken önce bekletilenler, sonra pencere (`file_inflight < W`), açıklık (`span_room > 0`), FIFO; süresi dolan `DeadlineBounded` düşer ve sayılır.
- `set_file_window` (≥ 1; daralma uçtakini geri almaz), `set_span_rooms` (FILE için bekletilenler düşülür), `set_file_backlog_room`, `admit_file`,
  `on_given` (yer ve açıklık bir azalır; FILE kimliği pencereye), `on_acked` (pencereden düşer), gözlem erişimcileri.

**`LaneState` (644–817).** Oturum haritası (ilk birimde açılır), `enqueue` (sayaçlarla), `pop_next`, pencere/açıklık/yer ayarlayıcıları,
`note_pump_done`, `on_given`, `push_front`, `on_expired`, `on_give_failed`, `on_acked`, `retain_active` (düşen birimler + bekletilenler `lane_dropped`), `sessions_with_work`, `is_empty`, `session`.

**Testler (819–1375).** Kabul sırası ve şerit içi FIFO (T-L1); W penceresi ve teyitle tek birim açılması (T-L2); teyit kanalının her turda boşaltılmasıyla 5000 birimde takılma olmaması,
taşmadan sonra pencerenin kalıcı kilitlenmesi (T-L3 ve ek); şeridin yalnız sınıftan okunması (T-L4); tavanlı ret ve bellek sınırı (T-L5); BDP penceresi; profil tabanı;
birikim sınırı; bekletme kuyruğunun metni hiç tutmaması; pencere/açıklık/geri basma ile bekletme; bekletilenlerin süre dolumu ve düşümü; pencere ayarı; W=5 ≥ 1 BDP (T-L9); tipli doğrulama; süresi dolan birimin düşümü (T-L10); `retain_active`.

## 3. Bulgular

Uyumlu davranışlar (tek tek yazılmadı):
- `transport.rs`: **24 davranış uyumlu.** Başlıcaları: pending INIT tavanı 16 → `Backpressure` (ccr §35.15, S1 §3); idempotent `connect` ve `EarlyUnitConflict` (S1 KARAR-1, K6);
  birim tavanı `conservative_limits(Session) − HDR_DATA` (S1 K5); `recovery=false` ürün builder'ında ret (S1 F0.2(iv)); fragmenter ret (S1 K3 "Transport birim içini bölmez");
  `Superseded` / `UnknownSession` pencereleri ve `close` sonucu (S1 F0.2(i), F07.1); `cancel` → `ConnectAbandoned(Cancelled)` (H9); `Pending` anında `Measuring` (F0.2(ii));
  yarım-açık düşüşte niyetin taze INIT ile sürmesi (INV-NO-IDLE-TEARDOWN, H11); provisional onayda `Ready(h')` (K7, DR-CONNECT-1 K5); glare kaybedenin `early_unit`'inin
  kazanan oturumda gönderilmesi (DR-CONNECT-1 K2); birimin yeni oturuma taşınmaması (K7); `CompressionPolicy`'nin davranışı değiştirmemesi (K4, H-COMPRESS-BOUNDARY);
  aynı oturum + aynı sınıf içinde sıralı teslim (S1 §4 D-09 eki); bağlam-önce dağıtım (PoC K10); `session_epoch` belgesinde FS etiketi; reachable `todo!()` yok; typed hatalar.
- `lanes.rs`: **11 davranış uyumlu.** Sınıf → şerit eşlemesi ve bayt okumama (KK-T1-04, S1 K3/D-04); kabul sırası; W = 5 varsayılanı (v4 K6) ve KK-T1-09 aritmetiği;
  tavan doluyken `Backpressure` (KK-T1-10); tahsisten önce tavan denetimi (SR-AUDIT-001); tipli `validate`; şeritte süresi dolan birimin düşümü (S1 §4);
  etkin olmayan oturumda şerit düşümü (S1 K7); pacing hızına dokunmama (INV-RATE-1 2026-11-11 metni).
- Ürün profilinde şeritlerin açık olması, BDP'den dosya penceresi ve şerit etiketli birim kimliği: sahibin koşullu `MERGE 199` sözü (#175 6856692375, 2026-11-13:
  "iki uç aynı APK ve eşleşen şerit profiliyle çalışacak") ile onaylı. Tasarım notu `T1-SERITLER.md` §5/§10'daki "`product()` = `None`" satırı bu sözle aşıldı.

| fid | tür | önem | konum | davranış | otorite | düzeltme türü | düzeltme taslağı |
|---|---|---|---|---|---|---|---|
| C4-01 | EKSİK | orta | transport.rs:2157-2167, 1552-1556 | `connect` ve `reinitiate` taşıyıcının `send_init` hatasını `.ok()` ile yutar: niyet `attempt = None` ile kaydedilir, INIT tele hiç çıkmaz, adaptörde yeniden deneme yolu yoktur. Niyet, Crypto `HALF_OPEN_TTL` (7 gün) dolup bekleyeni düşürene dek INIT'siz bekler. `HubEnvelopeLink::send_init` geçici `QueueFull` hatası verebilir (hub/link.rs). Aynı durumda `start_renewal` bekleyeni kapatıp `Deferred` döner (1337-1340), `connect` ise dönmez. | DR-CONNECT-1: "NİYET zamanla ölmez; DENEME maliyeti sınırlanır"; transport-tdd §0 INV-NO-IDLE-TEARDOWN; S1 §3: `Backpressure` = "Kuyruk dolu ya da pending INIT tavanı dolu" | kod-duzeltmesi | `connect`: `send_init` hatasında bekleyeni `Close` et, `insert_pending`'i geri al, `Err(Backpressure)` dön (S1 §3 kümesi içinde). `reinitiate`: `attempt == None` niyetleri `drive`'da önbellekli INIT baytıyla yeniden sun (INV-INIT-KEY-1 bayt-özdeşliği korunur). |
| C4-02 | UYUSMAZ | orta | lanes.rs:282-297; transport.rs:1849-1860 | Geri bildirimi aç profilde dosya penceresinin tabanı `link_profile_window_bytes` = `uplink × budgeted_ack_interval` (tipik 81 B ACK, 62 B/s dönüşte 5,4 s → 67 birim). Motorun aynı profildeki uçuş tabanı `pacing_cap × (feedback_horizon_ns + RTT)` (en büyük ACK + dönüş serileşmesi: 62 B/s'de ≈ 10,7 s + RTT). Şerit penceresi motor tabanının yaklaşık yarısında kalır; ayrıca kayıp birim teyidine dek pencerede yer tutar. Dosya için bağlayıcı uçuş tavanı motorun değil şerit katmanınınki olur. | CLAUDE.md §4 INV-RATE-1 [PROFİL İSTİSNASI 2026-11-16]: "recovery_window uçuş tavanını SINIRLAMAZ; tavan en az hat × (geri bildirim ufku + RTT)"; sender.rs `FeedbackStarvedRule` belgesi | karar-gerekir | İstisnanın uygulama katmanındaki dosya penceresini kapsayıp kapsamadığı sahipçe belirtilmeli. Kapsıyorsa: taban `uplink × (feedback_horizon_ns(downlink) + rtprop)` olsun, motorun kullandığı değerle tek kaynaktan (`runtime` içinde bir yardımcı) okunsun. |
| C4-03 | UYUSMAZ | orta | lanes.rs:593-609; transport.rs:1819-1829, 2178-2190 | FILE penceresi yalnız kayıplı teyit kanalından (`try_acked`; kanal 4096, `try_send` dolunca olay düşer) boşalır. Düşen bir teyit, birimi pencerede kalıcı tutar (test `t_l3_late_drain_after_overflow_locks_the_window` bunu sabitliyor). Boşaltma yalnız `drive` içinde; `send` → `send_via_lanes` → `pump_session` yolu teyit okumaz, uç da birim verir. Uygulama `recv`/`poll_event` çağırmadan uzun süre `send` ederse kanal taşabilir. | `LaneConfig::file_window_units` tanımı: "uca verdiği ve teyidi gelmemiş en çok birim"; T1-SERITLER §3 KK-T1-07; runtime.rs `lane_confirm_frontier`: "teyit olay kanalı değil — o kanal taşınca olay düşer, bu değer düşmez" | kod-duzeltmesi | Her pompa turunda `file_inflight`'tan `tagged(FILE, lane_confirm_frontier(FILE))` altındaki kimlikleri düşür. Cephe kayıpsızdır ve cephe altındaki her birim teyitlidir. Kanal olayı yalnız hızlandırıcı kalır. |
| C4-04 | UYUSMAZ | dusuk | transport.rs:2134-2144, 2170-2173, 2306-2307, 2323-2326, 1314 | `MAX_EVENTS` yalnız `drive_link` ve `drive_crypto_events` döngülerinde zorlanır. `connect`, `close`, `cancel` ve `on_renew_required` olay kuyruğuna oda denetimi olmadan yazar. Eş adresi olmayan eşe her `connect` niyet kaydı açmadan iki olay ekler; uygulama okumazsa kuyruk sınırsız büyür. Sabitin belgesi "olay düşmez, kaynakta bekletir" der. | CLAUDE.md §9 SR-AUDIT-001 (bounded buffer); `MAX_EVENTS` belgesi (140-142) | kod-duzeltmesi | `connect` başında `!room_for_events()` → `Err(Backpressure)` (S1 §3). `close`/`cancel` tam bir olay üretmek zorunda (F07.1); onların oturum/niyet tablosuyla sınırlı olduğu belgeye yazılsın. |
| C4-05 | EKSİK | dusuk | transport.rs:1595-1609, 1625-1651, 1790-1792, 2216 | `open_endpoint` hatası `.ok()?` ile yutulur: oturum ucu olmadan `Active` kurulur, `Ready(h, peer)` yayımlanır, `send(h)` kalıcı olarak `NotConnected` döner. Saklanan `early_unit` bu durumda ya da `send_in_lane` reddinde sinyalsiz düşer. `NotConnected`, S1 `send`'in belgelenen hata listesinde yok (TransportError kümesinde var). | S1 K6 [2026-10-08]: "Transport `early_unit`'i SAKLAR ... `Ready(h)` olunca birimi `h` üzerinden ilk normal `send()` olarak yollar"; s1.rs `send` # Errors: "`UnknownSession`, `SessionSuperseded`, `UnitTooLarge`, `Backpressure`, `DeadlineExpired`" | karar-gerekir | S1 bu durum için olay tanımlamıyor. Seçenekler: uç kurulamazsa `Ready` verme, oturumu kapat (kapalı neden listesinde karşılığı yok) ya da `early_unit`'i ucun ilk kuruluşuna dek sakla. Sahip kararı gerekir. |
| C4-06 | TANIMSIZ | dusuk | transport.rs:1915-1923 | Şeride kabul edilen (uygulamaya `Ok` dönmüş) birim, uç `Backpressure` ve `DeadlineExpired` dışında bir hata verirse sessizce düşer, yalnız `give_failed` sayılır. | S1 K8: "`Ok(())` = kuyruğa alındı"; spec'te bulunamadı (aranan: şeritten uca verme hatasında birimin akıbeti) | karar-gerekir | Birimi başa geri koyup pompayı durdurmak ya da sayacı uygulamaya yansıtmak. Hangisinin S1 K8 ile uyumlu olduğu sahipçe seçilmeli. |
| C4-07 | TANIMSIZ | dusuk | transport.rs:38-45, 1173-1211, 315-319 | Yenilemede RESPONSE, eski oturum boşalana ya da `renew_drain_cap` dolana dek Crypto'ya verilmez. Bu sürede eski oturum `Active` kalır ve yeni `send` kabul eder; boşalma ölçüsü bu yüzden yeni gönderimlerle uzayabilir. `renew_drain_cap` = `RuntimeConfig::drain_cap` (3 s); türetilmemiş, kapanış boşaltmasından ödünç. | KM-SEAM-AT-001 KARAR-2: "h' hazır → ATOMİK CUTOVER: h = draining (yeni send YOK, outstanding boşalır) · h' = aktif" (boşalma cutover'dan SONRA); spec'te bulunamadı (aranan: cutover öncesi bekleme ve süresi) | karar-gerekir | Ya KARAR-2 metnine uyulur (cutover hemen, boşalma sonra; Crypto'nun yalnız-alım drain'inde eski oturumun yeniden iletimi mühürlenebiliyor mu doğrulanmalı), ya da ön-boşaltma ve 3 s değeri sahip kararıyla yazılır. |
| C4-08 | UYUSMAZ | dusuk | transport.rs:486-508, 1528-1530, 2266-2272 | Alıcı sıralayıcısı sınırı "en eskiyi düşür" ile değil, alıcının kabul penceresinin dışını ACK'lememekle sağlanır; ACK'lenmiş birim hiç atılmaz. Ancak supersede edilmiş oturumun `Closed` olayı okununca (ya da `close`'da) `reorder` silinir: eksik sıra başının arkasında tutulan, ACK'lenmiş birimler teslim edilmeden düşer. | engine-spec §14: "rx_reorder_pool: ≤ RX_POOL_MAX; fazla out-of-order → en eski drop (SACK ile kurtarılır)"; INV-ACK-DELIVER-1 (modül belgesi; donmuş spec'te bulunamadı, aranan: `INV-ACK-DELIVER-1` specs/ altında) | karar-gerekir | Mekanizma farkı (pencere dışını ACK'lememe ↔ en eskiyi düşürme) ve drain sonunda ACK'lenmiş birim kaybı sahipçe karara bağlanmalı. Kod tarafı: drain sonunda tutulan birim sayısını sayaçla görünür kılmak. |
| C4-09 | TANIMSIZ | dusuk | lanes.rs:53-91, 244-322, 519-582; transport.rs:1861-1895 | Şerit kuralları [ŞERİT-W] (BDP kazancı 2, tavan 1024), [ŞERİT-KUYRUK] (en az 2 birim, 40 ms ufuk), [ŞERİT-GENİŞLİK] (256/4096), [ŞERİT-AÇIKLIK] ve [ASİMETRİK-YOL] tabanı birleştirilmiş kodda var; kuralların metni hiçbir spec ya da tarihli kararda yok. `FILE_BACKLOG_HORIZON_NS`, başka bir crate'in (`kama-node` `NODE_TICK`) iç sabitine bağlı bir değer. | Kod onayı: `MERGE 199` (#175 6856692375) ve İŞ-26 birleştirmeleri; spec'te bulunamadı (aranan: dosya penceresi BDP kuralı, motor birikim sınırı, şerit kabul genişliği — transport-tdd, engine-spec, KM-SEAM-AT-001) | karar-gerekir | Kural ve sabitler bir tasarım notunda ya da karar kaydında türetmeleriyle yazılsın. 40 ms ufuk, düğüm turu yerine adaptörün kendi `drive` aralığından ya da yapılandırmadan türesin. |
| C4-10 | TANIMSIZ | dusuk | transport.rs:140-147, 826-829; lanes.rs:93-99 | Türetilmemiş sabitler: `MAX_EVENTS = 256`, `MAX_INBOX = 1024`, tokio `worker_threads(2)`, şerit tavanları 64/64/256 (`[KALİBRASYON]`, tasarım notu KK-T1-05). | CLAUDE.md §9; spec'te bulunamadı (aranan: S1 olay/teslim kutusu tavanı) | karar-gerekir | Kalibrasyon etiketiyle bir karar kaydına alınsın ya da kaynak sabitten (`delivery_queue`, `offer_queue`) türetilsin. |
| C4-11 | TANIMSIZ | dusuk | transport.rs:301-307, 840-852, 2217-2218; lanes.rs:152-164 | Şerit açık/kapalı ayarı ve kabul genişlikleri iki uçta aynı olmak zorunda: birim kimliğinin üst 2 biti şerittir ve alıcı sırayı ondan türetir. Uyuşmazlık çalışma anında algılanmaz; sonuç sessiz yanlış sıralama, takılma ya da kabul penceresi reddidir. Sınıf bilgisi kimlik bitleri yoluyla tele girmiş olur. | Sahip koşulu (#175 6856692375): "iki uç aynı APK ve eşleşen şerit profiliyle"; S1 v2 YAMA: sınıfın "telde karşılığı YOK" | karar-gerekir | Dağıtım kuralı yerine çalışma anı denetimi: el sıkışmada ya da ilk birimde şerit profilinin uyuşmazlığını sayaçla/olayla görünür kılmak. Tel değişikliği gerekirse D-32(iii) kapsamında. |
| C4-12 | UYUSMAZ | orta | transport.rs:486-531, 533-544, 579-641, 1714-1728 | Eş PTT'yi son tarihli gönderiyorsa PTT şeridinde eksik sıra başı 100 ms sonra ya da arkasında iki birim tutulunca atlanır. Atlanan güvenilir kayıt (PTT_OPEN/CLOSE) geç gelirse sırasız teslim edilir. | KM-SEAM-AT-001 §4: "aynı session içinde `Reliable` birimler gönderim sırasında"; PoC-KAPSAM-001 EK-2 madde 1 (PTT Reliable klip, "geç gelen ses düşürülmez"); s1.rs `VoRel` "D-10 B: Reliable-sıralı" | kilitli | Karar 3 (#175 7171705736): "PTT ve metin davranışını dondurun". Değişiklik transport etkisi ölçüldükten sonra sahip kararıyla. |
| C4-13 | UYUSMAZ | dusuk | transport.rs:1605-1606, 2229-2230 | Sınıftan türeyen şerit indeksi uca (`send_in_lane`) verilir; motor PTT şeridine özel davranış uygular (son tarih bırakma, ileri kopyalar; runtime.rs). Sınıf bu yolla sunum sırası/penceresi dışında motor davranışını da seçer. | s1.rs ve KM-SEAM-AT-001 S1 v2 YAMA: "Transport onu yalnız sunum sırası/penceresi için okur" | kilitli | Karar 3 ve 2026-11-16 PTT D×5 kararı. Metin ↔ kod farkı sahip kararıyla kapatılmalı. |
| C4-14 | UYUSMAZ | dusuk | lanes.rs:463-478, 507-511 | Birikim kipinde pencereye alınmış (bekletilen) dosya birimi, pencere daraldıktan sonra da uca verilir; `pop_next` bu dalda `file_inflight < window` denetlemez. Teyitsiz dosya birimi sayısı yeni W'yi aşabilir (en çok eski W'ye dek). | `LaneConfig::file_window_units`: "teyidi gelmemiş en çok birim"; `set_file_window` belgesi yalnız "uçtaki birimi geri ALMAZ" der, bekletileni değil | kod-duzeltmesi | Bekletme kuyruğunun başını vermeden önce `file_inflight.len() < window` denetle; aşan bekletilenler şerit kuyruğunun başına geri dönsün. |
| C4-15 | OLU | dusuk | transport.rs:26-29, 498-503, 767-768 | Bayat belge: (i) "Motor bir birimi hiçbir zaman terk etmediği için" — motor PTT son tarihinde birimi bırakır; (ii) `Reorder` [F3] "Eksik sıra başı HİÇBİR ZAMAN atlanmaz" — PTT kipinde atlanır; (iii) `lanes` alanı "`None` = şeritsiz (ürün)" — ürün `Some` kurar (317-324). | `ClientTransportConfig::product` (324); runtime.rs PTT bırakma (4341-4354) | kod-duzeltmesi | Yalnız yorumlar düzeltilsin; davranış değişmez (PTT kilidine dokunmaz). |
| C4-16 | OLU | dusuk | transport.rs:1793-1796; lanes.rs:480-484, 324-336, 101-104 | Ürün yolunda erişilemeyen dallar: `send_via_lanes`'in şeritsiz geri dönüşü (yalnız `lanes.is_some()` iken çağrılır); `pop_next`'in "sınır kalktıysa" dalı (yer `None`'a yalnız yapılandırma değişirse döner); `file_window_covers_bdp` ve `GOSTER_*` yalnız testte okunur. | CLAUDE.md §6 (yarım/ölü kod kuralı) | kod-duzeltmesi | Savunma dalları `debug_assert!` + typed hata ya da yorumlu bırakılsın; yalnız-test öğeleri `#[cfg(test)]` altına alınsın (pub yüzey değişeceği için D-32 sınıfı denetlenmeli). |
| C4-17 | DOGRULANAMADI | dusuk | transport.rs:1136-1138, 1152-1154 | Yanıtlayan rolde `send_response` hatası yutulur; oturum `responder_peer`/`provisional`'a girer ama RESPONSE tele çıkmayabilir. Başlatanın INIT tekrarının Crypto'da RESPONSE'u yeniden ürettiği bu kapsamda doğrulanamadı. | KM-SEAM-TC-001 / noise-wire H-RETRY (okunmadı) | karar-gerekir | Crypto'nun yinelenen INIT'e yanıtı yeniden verip vermediği doğrulansın; vermiyorsa `send_response` hatası sayaçla görünür olsun ve yeniden sunulsun. |

## 4. Okunan otorite bölümleri

- `CLAUDE.md` §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-ENGINE-SOC, FS), §6, §8, §9, YAMA KAYDI.
- `docs/decisions/KARAR-KAYDI-2026-11.md` §1–§10 (tamamı; D-23 bayrak kuralı, D-32(iii) tel/S1 imza sınıfı, D-CC-1).
- `docs/decisions/PoC-KAPSAM-001.md` (tamamı; §2 PTT satırı, EK-2 madde 1, EK-3, §5 beyanlı açıklar, K10).
- `docs/decisions/DR-CONNECT-1.md` (tamamı; H1, H2, H6, H11, tamamlama K2, K5).
- `docs/seams/KM-SEAM-AT-001.md` §0–§9 (S1: K1–K9, KARAR-1, KARAR-2, §3 hata semantiği, §4 zamanlama ve sıralama, §6 harness, YAMA KAYITLARI).
- `crates/kama-contracts/src/s1.rs` (S1'in derlenen hâli; `TrafficClass` D-08/D-09 belgesi, `send` hata listesi).
- `specs/transport-layer-tdd.md` §0 (INV-NO-IDLE-TEARDOWN, INV-PLAINTEXT-NOT-SEMANTICS), §0.1, §1 başı.
- `specs/adaptive-transport-engine-spec.md` §0.2 (INV-ENGINE-SOC), §4 (alıcı durum makinesi, teslim sırası), §14 (bellek tavanları).
- `specs/application-protocol-and-rpc-spec.md` §2 (3 düzlem) ve §6 başlıkları (grep düzeyinde: S1 adaptörü Plane 2 hot-path'ini sarmalamıyor; uyumlu).
- `specs/kama-noise-wire-tdd-v2.0.md` YAMA KAYDI (KS-GRACE-1 `N_g = 64`, `T_g = 30 s`).
- `docs/design/T1-SERITLER.md` §1–§10 (tasarım notu; otorite değil, KK-T1 kart atıfları için).
- Sahip kararları (#175): 7171705736 (dokuz karar; karar 3 PTT/metin dondurma), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074,
  7186813951 (receiver.rs alarm aralığı Web'de), 6856692375 (koşullu `MERGE 199`: ürün profilinde şeritler).
