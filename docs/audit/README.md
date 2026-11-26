# Kod denetimi sentezi (taban 125b9afa)

Taban: ürün hattı `125b9afa` (aday `1a4f9efc` bunun içinde). Bütün satır numaraları bu tabandandır. Denetim salt okumadır: kaynak dosyalara yazılmadı, cargo koşulmadı, testler koşulmadı. Aşağıdaki davranış iddialarının hepsi kod okumasından gelir; hiçbiri ölçümle doğrulanmadı.

Bu belgeyi hazırlarken şunlar okundu:

- **Okuyucu raporları:** A1, A2 ve A5 baştan sona okundu. Öteki 16 raporun kapsam ve davranış envanteri bölümleri okundu. Bulgu satırları yapılandırılmış teslimden alındı.
- **Kendim açtığım satırlar (§3):** cc-port ↔ LineGuard ↔ pacer ↔ sender bölümünde alıntılanan her satır kodda açılıp okundu. Bunlar `sender.rs` 1790–1830, 2706–2840, 3071–3090, 3150–3245, 4222–4245, 4325–4384, 4805–4818, 5842–5860 ve 6185–6200; `cc/bbr.rs` 330–430 ve 586–652; `line_guard.rs` 1–130, 390–477, 720–890, 1015–1045, 1225–1410, 1448–1530, 1740–1800 ve 1855–1905; `pacer.rs` 20–130 ve 230–270.
- **Sahip kararları:** #175 yorumlarından 7171705736, 7172579876, 7186813951 ve 6920942098 doğrudan okundu.

Okuyucunun DOĞRULANAMADI dediği hiçbir kalem burada kesinleştirilmedi. Sentezcinin kendi bulguları §8'de ayrıca işaretlidir.

## 1. Kapsam

| rapor | kapsanan kod | okunan satır |
|---|---|---:|
| A1-sender-1 | `adaptive-transport/src/sender.rs` 1–4800 (+ spec satırları) | 6 232 |
| A2-sender-2 | `sender.rs` 4801–9600 (+ bağlam) | 5 040 |
| A3-sender-testler | `sender.rs` 9601–16722 (140 test) | 7 122 |
| A4-alici | `receiver.rs`, `ack.rs`, `arrival_rate.rs`, `return_path.rs`, `seq.rs`, `read.rs`, `clock.rs` | 5 641 |
| A5-hiz-cc | `line_guard.rs`, `pacer.rs`, `rate.rs`, `bw.rs`, `rtt.rs`, `cc/*`, `budget*.rs` | 6 246 |
| A6-transport-kalan | `loss.rs`, `fusion*.rs`, `emergency.rs`, `arq.rs`, `tail_recovery.rs`, `fate_estimator.rs`, `fec.rs`, `fs_*`, `udp_runner.rs`, `sim.rs`, `lib.rs`, `error.rs` | 7 905 |
| C1-runtime-1 | `client-core/src/runtime.rs` 1–3300 | 3 300 |
| C2-runtime-2 | `runtime.rs` 3301–6461 | 3 161 |
| C3-runtime-testler-kucukler | `runtime.rs` 6462–8717, `memory_link.rs`, `seam.rs`, `lib.rs`, `unit_id.rs`, `wire.rs`, `batch.rs` | 4 460 |
| C4-transport-lanes | `client-core/src/transport.rs`, `lanes.rs` | 3 951 |
| C5-hub-istemci | `client-core/src/hub/*` | 7 044 |
| N-kama-node | `kama-node` (src + tests) | 10 229 |
| P1-app-mesaj-dosya | `kama-app` lib/clock/store/messaging/file_transfer | 5 173 |
| P2-app-ptt-kodek | `kama-app` ptt/playout/signaling/codec | 6 086 |
| W-noise-wire | `kama-noise-wire/src/*` | 8 386 |
| H-hub-store | `hub-server/src/*`, `kama-store/src/lib.rs` | 5 551 |
| M1-mobil-provision-contracts | `kama-contracts`, `kama-provision`, `kama-mobile` | 7 608 |
| M2-kucuk-crateler | `kama-vectors`, `kama-engine`, `kama-fec-codec`, `kama-transport-schema`, `kama-integration/src`, dört boş crate, bütün `Cargo.toml` | 5 165 |
| B-bench | `bench/src/*` | 10 876 |
| **toplam** | | **119 176** |

Toplama A1'in okuduğu spec satırları da girdi. Okuyucular 19 raporda 434 ham bulgu yazdı. Bunlar §4'te 92 birleşik kaleme indirildi.

## 2. Ürün davranış haritası

### 2.1 Crate → modül → görev

| crate | ürün yolunda | modül → görev |
|---|---|---|
| `adaptive-transport` | evet | `sender.rs`: gönderici durum makinesi. ACK/zamanlayıcı işler, kayıp ilan eder; ARQ, PTO, FEC paritesi ve kopyayı planlar; pacer hızını kurar. `cc/` (bbr, bw_estimation, min_max, mod): quinn-proto 0.11.18 BBR portu, ürün motoru. `line_guard.rs`: geri bildirimi aç profilde gerçek ileri hattı kestirir, gönderim tavanını yönetir. `pacer.rs`: jeton kovası, hat modeli ve çift kipi. `bw.rs`: BtlBw örnekleyici ve max filtresi. `rtt.rs`: srtt, rttvar, RTprop ve qd. `loss.rs`: RFC 9002 eşikleri, kayıp defteri, spekülatif risk. `arq.rs`: son tarihli yeniden iletim, ReorderTracker. `tail_recovery.rs`: PTO prob makinesi. `fusion.rs`: fuzzy FEC/ARQ kuralları. `emergency.rs`: histerezisli acil seviye. `fate_estimator.rs`: kayıp yapısı; Q18 kapısı okur. `budget.rs`: §8.5 kıskaçları. `receiver.rs`: alıcı ve ACK zamanlaması. `ack.rs`: 0x10/0x14/0x15–0x17 kodeği. `arrival_rate.rs`: ilk varış penceresi ve daralma alarmı. `return_path.rs`: yalnız gözlem. Deneysel ya da ölü: `budget_kernel.rs`, `fs_*`, `udp_runner.rs`, `fusion_policy.rs` (bench), `fec.rs` (deprecated). |
| `client-core` | evet | `runtime.rs`: `client_task` ve `server_task`. Çerçeveleme, mühür, PN↔seq, RS encode/decode, PARAM/CAPS, yalın PTT, bağlam-önce dağıtım, teyit drenajı. `transport.rs`: S1 adaptörü; oturum yaşam döngüsü, yenileme, Reorder. `lanes.rs`: 4 şerit, FILE penceresi. `wire.rs`: interim çerçeve ailesi 0x00–0x09. `unit_id.rs`: şerit etiketi üst 2 bitte. `seam.rs`: IdentitySeam stub. `batch.rs`: ccr §35.14 kodeği, veri yolunda değil. `hub/`: HubLink, bind, heartbeat, T3, relink, endpoint, tek-açılış deposu. `lib.rs`: ClientSession. |
| `kama-noise-wire` | evet | El sıkışma SSI1/SSR1, KDF D1–D7, legacy 22 B ve extended 26 B zarf, PN/replay, `session_control`, `epoch_machine`. HubChannel no-op. Eski API'ler (`KamaNoiseWireProtector`, `SessionManager`, `HubSim` vb.) yalnız deneysel ya da test yolunda. |
| `kama-app` | evet | `messaging` (TEXT/RECEIPT), `file_transfer`, `ptt`, `signaling`, `codec` (AMP v1 ve işaretli PTT biçimleri), `store` trait. `playout` ölü. |
| `kama-node` | evet | Bileşim kökü. Tek iş parçacığında düğüm döngüsü, komut kuyruğu (64), dağıtım anahtarları (ortam değişkeni ve dosya), `hub_mode`, `a5` metrikleri, `files`. |
| `kama-mobile` | evet | UniFFI sarmalayıcısı, kapanış serileştirmesi. |
| `kama-contracts` | evet | S1 `KamaTransport`, S3 `PacketProtection`/`SessionControl`, S4 kodeği. |
| `kama-provision`, `kama-store` | evet | Provisioning CLI ve yükleyicisi; redb deposu (`AppStore`, `CounterStore`). |
| `hub-server` | PoC | Tek-hub röle: iki turlu bind, STORE/DELIVER, veri rölesi, ROUTE_MISS. |
| `kama-engine`, `kama-fec-codec`, `kama-transport-schema` | kısmen | Üründe yalnız `plan_param_bootstrap`, `ParamState`, `RsSimdCodec` + `by_id` ve şemanın tamamı kullanılıyor. |
| `kama-integration`, `kama-vectors`, `bench` | hayır | Deneysel alıcı, test vektörleri, benchmark düzeneği. |
| `ccr-codec`, `discovery`, `ra-ca`, `admin-server` | hayır | 3 satırlık yorum; davranış yok (§7). |

### 2.2 Uçtan uca akış

**İleri yön (uygulama → tel):**

1. `kama-app` motorları bir AMP kaydı kurar. `kama-node` API'si yük boyunu kuyruğa koymadan önce sınar; komut düğüm iş parçacığına gider.
2. S1 `ClientTransport::send` (`transport.rs`) birimi sınıfına göre şeride koyar (`lanes.rs`). `pump_session` FILE penceresini ve açıklığı hesaplar, birimi `ClientEndpoint::send_in_lane`'e verir.
3. `client_task` sunumu işler (`runtime.rs` 4122–4324): mesajı chunk'lara böler, kimliği `tagged(lane, sıra)` olarak verir, SourcePrefix ekler, birimi FEC bloğuna koyar ve `Event::AppData` üretir.
4. `Sender::on_event_core` (`sender.rs` 3071) sırasıyla şunları yapar: pacer dolumu, LineGuard `tick`, olay, `fuzzy_evaluate`, `update_state`, hız yolu (port: BBR ya da `cc_feedback_cap`; §3), `schedule_transmit`. `schedule_transmit` sırası: öncelikli şerit, ARQ, yeni veri, PTO, kuruluş probu, kopyalar, parite, boş hat kör yeniden iletimi.
5. `drive!` her `TransmitDecision`'ı çerçeveye çevirir (`unit_frame`, `encode_ptt`, `FEC_REPAIR`).
6. `seal_send!` çerçeveyi gönderir: `PacketProtection::seal` (Noise legacy 22 B) → egress kapısı → `UplinkShare::charge` → UDP.
7. Hub kipinde datagram `HubEndpoint` tek soketinden çıkar; `hub-server` onu bayt-özdeş iletir.

**Geri yön (alıcı → ACK → gönderici):**

1. `server_task` datagramı açar (`open`) ve bağlam-önce yönlendirir (`route_server`).
2. DATA için kabul penceresi ve teslim kanalı denetlenir, sonra `Receiver::on_datagram` çalışır: seq çözümü, tekilleştirme, PairRate, ilk varış penceresi, RateDropDetector, ACK zamanlaması (port alıcısında 25 ms ya da sırasız varışta anında, %25 bütçe kovasıyla).
3. Teslim `admit_chunk` ile yapılır. `fec_ingest` RS çözümü yapar; kurtarılan birimler RECOVERED ile bildirilir.
4. ACK (0x14 ya da 0x15) mühürlenip gönderilir.
5. İstemcide `handle_rx!` ACK'i çözer. `arrival_rate`, `pair_rate` ve ölçüm aralığı notları alınır, PN'ler seq'e çevrilir ve `Event::AckReceived` üretilir.
6. `Sender::on_ack_received` çalışır: Karn RTT, tombstone, un-loss, kayıp tespiti, ardından `cc.on_ack` ×n → `set_external_bw` → `on_end_acks` (4228–4237), sonra `cc_line_guard_on_ack` (4325). Bu adımda pacer'ın hat hızı ve kuyruğu yazılır.
7. `queue_losses` kayıpları işler, `cc.on_congestion_event` (4811–4815) çağrılır.
8. Teyit zinciri: `take_confirmed_units` → `drain_confirmed!` → uygulama ACK'i → `transport.rs` `drive_deliveries`/Reorder → `kama-app` olayı → `NodeEvent`.

## 3. Modüller arası etkileşim

### 3.1 cc-port (BBR) ↔ LineGuard ↔ pacer ↔ sender: görevleri kim yürütüyor

Geri bildirimi aç kural (`FeedbackStarvedRule`) yalnız şu koşulda kurulur: port motoru, `link_rates` verilmiş ve bütçeli ACK aralığı ≥ 1 s (`runtime.rs` 482–488, 3176–3192). Kural yoksa port yolu BBR'dır. Aşağıdaki tablo kural varken durumu gösterir. Satırların hepsi açılıp okundu.

| görev | BBR (`cc/bbr.rs`) | LineGuard (`line_guard.rs`) | gönderici kapısı (`sender.rs`) | kim belirliyor |
|---|---|---|---|---|
| pacing hızı (jeton) | `pacing_rate()` 620, `calculate_pacing_rate` 408–428 (gain × bw, Startup'ta düşmez) | `pace_bps()` 860 | 3162–3166 önce BBR hızını kurar; 3183–3185 `rate = self.cc_feedback_cap(rule)` ile ezer | **LineGuard** (BBR hesaplanır, okunmaz) |
| hat modeli hızı (tel, zarf dahil) | — | `pace_bps` | `pacer.set_line_rate(g.pace_bps())`: 3080 (`tick`), 4366 (ACK), 5855 (kurulum) | **LineGuard** |
| uçuş tavanı | `window_without_recovery()` 611–616 (ProbeRTT'de probe penceresi, aksi `cwnd`) | dolaylı: `cc_feedback_cap` | `cc_window` 2761–2770 `max(window_without_recovery, cc_feedback_floor)`; taban 2775–2780 `cap × (feedback_ns + rtprop)` | **ikisi birden** (`max`) |
| burst tavanı | `window()` 596–605 (recovery'de `min(cwnd, recovery_window)`) | — | 3155 `PacerParams::from_window(cc.window(), …)` → 3187 | **BBR** (§8 Y-01) |
| kayıp tepkisi | `on_congestion_event` 589 → `recovery_window`; kural varken uçuşa uygulanmaz, burst'e girer | `overflow_estimate` 1248–1305: teslim kesri tabanın 3/4'ünün altında ve kuyruk kanıtı varsa tavanı düşürür | 4811–4815 kayıp baytını BBR'a verir | BBR durumu + **LineGuard hız düşüşü** |
| bant yoklaması | ProbeBW kazanç döngüsü (hız okunmaz), ProbeRTT 10 s (pencere tabanın altında kalır) | `begin_probe` 1855, `begin_start_probe` 1866, `probe_to` 1876; `on_pair_rate` 1381–1407 (tek çiftle doğrudan kalıcı açma); pacer çift kipi (`PAIR_PERIOD_NS` 500 ms, `pacer.rs` 44, 246–266) | `tick` 3078–3083 | **LineGuard** |
| başlangıç | Startup 2,885; ilk RTT'de `init_cwnd/min_rtt` | başlangıç bütçesi (`start_budget_admits` 902), ilk pencere (`on_first_window` 1059), geçici evrede tavan P/2 (1787–1800); ilk tavan `conservative_start` = P × 1/1 (412–413, 764–766) | `start_budget_holds` | **LineGuard** |
| toparlanma (ARQ/PTO) | — | — | `plan_arq_retransmits`, `emit_tail_probe`; `tail_recovery.rs` | gönderici (ikisinden bağımsız) |

**Gözlem (kod okuması, ölçülmedi).** Kural varken BBR'ın durum makinesi çalışmaya devam ediyor: Startup, Drain, ProbeBW, ProbeRTT ve recovery hepsi işliyor. Ama çıktılarından yalnız ikisi okunuyor:

- uçuş tavanındaki `max` içinde `cwnd`. Dar hatta taban `cap × (≈ 10,7 s + RTT)` olduğu için bu terimin bağlaması beklenmez.
- burst tavanı. 10·MTU tabanı yüzünden dar hatta sabittir.

Kural varken pacing'i tek başına LineGuard belirliyor. Aynı değişkene iki yönetici şu üç yerde dokunuyor:

1. Jeton hızı önce BBR'dan kuruluyor, sonra eziliyor (3162–3185).
2. Uçuş tavanı iki ayrı mantığın `max`'ı (2766–2769). BBR Startup'ta `cwnd`'yi büyütürse uçuş tabanın üstüne çıkabiliyor; o sırada pacing yine LineGuard'da.
3. Kayıp iki ayrı yoldan iki ayrı etki üretiyor. BBR'da `recovery_window` var; bu yolda uçuşa uygulanmıyor ama burst'e giriyor. LineGuard'da `overflow_estimate` var ve hızı düşürüyor.

### 3.2 Otoriteyle uyum

| otorite | metin | kod | hüküm |
|---|---|---|---|
| CLAUDE.md §4 INV-RATE-1, 2026-11-11 | «pace = BBR pacing_gain × BtlBw; BtlBw loss'tan türetilmez» | Kural varken pace = `cc_feedback_cap` (3183–3185) | UYUŞMAZ (S-01), istisna kapsamına girmiyorsa |
| 2026-11-16 profil istisnası (CLAUDE.md YAMA; #175 6920942098) | «Pacing profil hattıyla sert sınırlı; ölçüm tavanı aşamaz» · «Taban: hat hızı × (yol RTT'si + en büyük bütçeli ACK aralığı + o ACK'in serileşmesi)» · «Loss yine ölçülür, FEC/ARQ'yu ve BBR durumunu besler» | Tavan bir sınır olarak yazılmış; kod onu kaynak yapıyor. Taban ölçülen hatta iniyor (2784–2800). Kayıp kesri hızı düşürüyor (`overflow_estimate`). | S-01, S-02, S-03 karar-gerekir |
| engine-spec §0.3 CC-PORT YAMA (1)–(8) | (2) pace = `pacing_gain × BtlBw` … (3) recovery'de `min(cwnd, recovery_window)` … (6) burst = `clamp(window·2 ms/srtt, …)` | Kural yokken uyumlu (A1). Kural varken (2) uygulanmıyor, (3) yerine 2026-11-16 istisnası, (6) uygulanıyor. | — |
| engine-spec başlık YAMA 2026-11-21 (§2.2) | «Gönderici … kendi gönderim kaydıyla iki nokta tahmincisine verir … sonraki ACK'lerin ölçüm aralığı (0x15, `rx_span`) ya da iki nokta tahmini düşürür, süreli yoklama yükseltir» | LineGuard'ın ilk pencere, aralık ve cephe ölçüsü ile süreli yoklaması burada tarif ediliyor | **kısmen tanımlı.** A5, LineGuard'ı adıyla aradığı için "spec'te bulunamadı" yazdı. Mekanizmanın bir kısmı bu YAMA'da adsız olarak var. Ölçünün pacing'in **kaynağı** olduğu yazılı değil. Taşma, çift alt sınırı, boşluk bütçesi ve çift kipi de yazılı değil. |
| engine-spec §7.4 INV-ENGINE-BW | «bandwidth DAİMA delivered-rate'ten (BtlBw); loss'tan ASLA türetilmez» | `overflow_estimate`, çift alt sınırı yokken büyüklüğü `(rx / taban kesri) / rx_span` olarak türetiyor | UYUŞMAZ (S-03) |
| ADR-06 / ADR-14 | tek adaptif motor; quinn fork yok, kod taşıma | Tek crate ve tek motor; fork yok | Metne uyumlu. Kural varken ürün artık quinn+BBR referans davranışında değil. ADR-18 / §4.4 kıyasında bu profil "BBR'a karşı" değil, LineGuard'a karşı ölçülür. Rapor dili buna dikkat etmeli. |
| transport-layer-tdd §14 INV-RATE-1 satırı, §5.4 CC-PORT YAMA | eski ve yeni metin | Kural yokken uyumlu | — |
| sahip 7171705736 m. 5; 7172579876 (K2′) | Paket çifti yoklama hedefini seçer; kalıcı tavan artışı sınırlı yoklamayla doğrulanır; doğrudan açma yalnız karşılaştırma için | `on_pair_rate` bugün doğrudan ve kalıcı açıyor; K2′ deney anahtarı bu tabanda yok | S-04 kilitli (#262 parkta, 7186813951) |

**Sonuç.** Bu bir uygulama hatası değil. Kod, belgesinde gerekçelendirdiği bilinçli bir tasarımı uyguluyor (`sender.rs` 1809–1815, 3174–3182; `line_guard.rs` 12–21): BtlBw seyrek ACK ve %20 kayıpta hattın üçte birini okuyor. Ama bu tasarım INV-RATE-1'in yazılı metninde yok. Sahip kararları (m. 5, K2′) LineGuard tavanını varsayıyor ve onu iyileştiriyor; hiçbiri pacing'in kaynağını değiştiren tarihli bir yama değil. Kod tek başına spec'e çekilemez. `rate = min(cc.pacing_rate(), cap)` yapmak, kodun kendi ölçümüne göre dar hatta hızı 2,8 → 1,1 kB/s'ye düşürür (3180–3181). Görev dağılımı sahip kararı ister. Kararın yönleri tarafsız olarak:

- **Yön A:** LineGuard tek kontrolcü olur. cc-port yalnız kayıp ve ACK muhasebesi yapar. INV-RATE-1'e profil için "pace = ölçülen hat; uçuş tabanı ölçülen hat × ufuk" YAMA'sı yazılır. Taşma (S-03) için ayrı bir istisna gerekir ya da taşma çıkarılır.
- **Yön B:** Tek hız tahmincisi kalır. LineGuard'ın hat ölçüsü BBR'a dış bant girdisi olarak verilir (`set_external_bw` benzeri). Pace = `pacing_gain × ölçülen hat`, yoklama BBR ProbeBW'den gelir. LineGuard'ın kendi yoklama, başlangıç ve çift mekanizmaları kalkar ya da BBR'a taşınır. Bu yön S-01, S-04, S-25, S-26 ve Y-01'i birlikte yeniden tanımlar.

### 3.3 Öteki sözleşme kopuklukları (okuyucuların çapraz-modül notlarından)

| # | iki uç | kopukluk | bulgu |
|---|---|---|---|
| X-1 | `client-core` `TransportEngine::parse("")` (runtime.rs 718–724) ↔ `kama-node` lib.rs 333–340 ↔ bench | Boş ortam değişkeni ürünü sessizce Legacy'ye düşürüyor; `Default` CcPort | S-07 |
| X-2 | `lanes.rs` `file_window_floor` (uplink × bütçeli ACK aralığı) ↔ `sender.rs` `cc_feedback_floor` (cap × (ufuk + RTT)) | İki ayrı taban; şerit penceresi motorun yaklaşık yarısında bağlıyor | S-36 |
| X-3 | `transport.rs` FILE penceresi ↔ `runtime.rs` `try_acked` (4096, taşınca düşüyor) | Kayıpsız kaynak `lane_confirm_frontier` kullanılmıyor | S-37 |
| X-4 | `kama-node` `note_up` → `HubSide::peer_ready(handle_of(peer))` ↔ `hub/link.rs` 722 `peer_ready` | Oturum nesli değil, eşe en son bağlanan handle kullanılıyor | S-39 |
| X-5 | `server_task` idle_hold 105 s ↔ istemci PARAM/CAPS'i bir kez gönderiyor | Yeni yuvada PARAM bağı ve bitmap kayboluyor | S-05 |
| X-6 | `receiver.rs` 0x15 alanları ↔ `line_guard::interval_sample` (`AlarmEvidence` aynası) | Aynı sabitler iki uçta elle tutuluyor (28 B, 256 µs, MTU) | S-69 (kilitli, Web) |
| X-7 | `kama-app` `PTT_BUNDLE_DEADLINE`, `SPURT_RX_HOLE_WAIT` ↔ `runtime::PTT_DEADLINE_NS`, `transport::PTT_HOLE_SKIP` | Derleme zamanı bağı yok | P2-07 (S-89) |
| X-8 | `kama-node` `PttOpened.bundle_ms` "ms" ↔ `kama-app` tel kodu (pilotta 210) | Birim karışıklığı | P2-04 (kilitli) |
| X-9 | `hub-server` kendi `wire.rs` S4 kopyası ↔ `kama-contracts::s4` (istemci) | İki kodek; çözücü katılığı farklı | S-49 |
| X-10 | `kama-engine` `MAX_PREPARAM_SHARDS` = 256 ↔ runtime `PRE_PARAM_FEC_CAP` = 64 | Yorum "hiza" diyor, değerler farklı | C2R2-12 / C3-12 (S-92) |
| X-11 | `sender.rs` 1122 `BS_PROBE_WIRE_EXACT` = 44 ↔ `wire.rs` CS gövdesi 28 B + stub 16 B | Gerçek dikişte (22/26 B) tutmuyor | S-59 |
| X-12 | `ack_epoch` alanı | Hiçbir modül okumuyor (depo çapında arandı) | A4-13 (S-92) |
| X-13 | `RuntimeConfig.fec_loss_risk = bounded_v1()` (runtime.rs 372) ↔ spekülatif kol | Ürün varsayılanı açık, spec'te yok | S-12 |
| X-14 | `s3.rs` 143–145 "seal reddi kayıp say (INV-RATE-1)" ↔ runtime `seal_send!` | Otoritesi olmayan bir yükümlülük | M1 B-10 (S-78) |

## 4. Birleşik bulgu listesi

Sütunlar: **tür** = UYUŞMAZ / TANIMSIZ / EKSİK / ÖLÜ; **düzeltme** = kod-duzeltmesi (K) / karar-gerekir (G) / kilitli (L). **Yazar** sütununda Yerel = `line_guard.rs`, `pacer.rs`, `sender.rs`; Web = `receiver.rs`; donmuş = PTT/metin (karar 3). Öteki dosyalar için yalnız dosya yazıldı (koordinatör atar).

### 4.1 Yüksek

| ID | başlık | tür | konum | kaynak | düzeltme | yazar |
|---|---|---|---|---|---|---|
| S-01 | Geri bildirimi aç yolda pacing kaynağı LineGuard; BBR `pacing_gain × BtlBw` okunmuyor; iki kontrolcü | UYUŞMAZ | `sender.rs` 3183–3185, 1809–1815; `line_guard.rs` 860 | A1S1-01, A5-03, A2S2-10 | G | Yerel |
| S-02 | Uçuş tabanı ölçülen hatta iniyor (`cap × (ufuk + rtprop)`); metindeki "hat" profil hattı | UYUŞMAZ | `sender.rs` 2775–2800 | A1S1-02, A2S2-10 | G | Yerel |
| S-03 | `overflow_estimate` kayıp kesrinden pacing tavanını düşürüyor | UYUŞMAZ | `line_guard.rs` 1248–1305, 1228–1233, 1757–1778 | A5-02 | G | Yerel |
| S-04 | `on_pair_rate` tek çiftle tavanı doğrudan ve kalıcı açıyor | UYUŞMAZ | `line_guard.rs` 1381–1407 | A5-01, A5-17 | L (#262 parkta, K2′ kapalı) | Yerel |
| S-05 | Sunucu 105 s sessiz yuvayı emekliye ayırıyor; PARAM/CAPS bağı kayboluyor, onarımlar çözülmüyor | UYUŞMAZ | `runtime.rs` 5879–5891, 6007–6020, 6221–6259 | C2R2-01 | G | `client-core/src/runtime.rs` |
| S-06 | Yalın PTT tek iletim + 1 s bırakma, DeadlineBounded demet, birleşik biçim, sıra başı atlama, 0x8A–0x8D işaretleri; PTT-R0 "Reliable" ile çelişiyor | UYUŞMAZ / TANIMSIZ | `runtime.rs` 456–474, 2185, 3283; `ptt.rs`; `transport.rs` 486–641; `codec/mod.rs` 659–703 | C1R1-03..05, C2R2-14/15, C4-12/13, P2-01..06, P2-08, P2-11, P2-18/19, A1S1-19, A2S2-18/19, A3T-10, A4-03, C3-04, C3-13 | L | donmuş |

### 4.2 Orta

| ID | başlık | tür | konum | kaynak | düzeltme | yazar |
|---|---|---|---|---|---|---|
| S-07 | Boş `KAMA_TRANSPORT_ENGINE` → Legacy; belgeler Legacy'yi ürün diyor | UYUŞMAZ | `runtime.rs` 715–724, 94–97; `kama-node/src/lib.rs` 329–340; `bench/src/kama_product.rs` 156–162; `client-core/src/lib.rs` 100–104 | C1R1-01/02, N-05, B-03, C3-08 | K | runtime.rs, kama-node, bench |
| S-08 | E2a v1.1: port yolunda RTT örneği atlanıyor | UYUŞMAZ | `sender.rs` 4165–4202, 2723 | A1S1-05, A3T-01 | G | Yerel |
| S-09 | Port: PreConverge'de PTO probu (U14 canlılık istisnası) | UYUŞMAZ | `sender.rs` 4465–4507 | A1S1-04, A3T-08 | G | Yerel |
| S-10 | Eski motor: srtt yokken PreConverge'de PTO ve tail prob | UYUŞMAZ | `sender.rs` 4465–4507; testler 10795–10848, 13155–13228 | A3T-02 | K (yalnız eski motor) | Yerel |
| S-11 | DR-EST-1 kuruluş kopyası jeton ve uçuş penceresinden muaf, varsayılan açık | UYUŞMAZ | `sender.rs` 8579–8644, 1920–1925 | A1S1-06, A2S2-04 | G | Yerel |
| S-12 | Spekülatif FEC kolu (`bounded_v1`) ve Q18 yapı kapısı ürün yolunda; kalibre değil, `eligible()` okunmuyor | TANIMSIZ | `sender.rs` 336–448, 4877–5032, 5152–5234; `loss.rs` 94–108; `fate_estimator.rs` | A1S1-20, A2S2-01/02/24, A6-05/06 | G | Yerel |
| S-13 | `tightest_deadline_ns` hiç sıfırlanmıyor; spekülatif L1 kalıcı olarak sıkı tavanda | UYUŞMAZ | `sender.rs` 5729–5745 | A2S2-03 | K | Yerel |
| S-14 | `pending_fec` tavansız; `manual_r` kırpılmıyor | EKSİK | `sender.rs` 6833–6877 | A2S2-06 | K | Yerel |
| S-15 | Son tarih / COPY_MAX_INTERVAL / RETRY_BUDGET / IDLE / PATH_DEAD zamanlayıcıları yok; `dup_queue` tavansız; `dup_spacing` 20 ms | EKSİK / UYUŞMAZ | `sender.rs` 1052–1059, 1830–1916, 8630–8726, 9223 | A1S1-13/14, A2S2-05/07/08 | G | Yerel |
| S-16 | KS-PN-1: hop-ACK'siz K = 1024 PN tavanı yok | EKSİK | `sender.rs` emisyon yolları; `runtime.rs` 3386–3431; `emergency.rs` | A1S1-12, A2S2-23, A3T-22, A6-03, C2R2-02 | G (spec "SPEC-ONLY, ayrı iş") | Yerel + runtime.rs |
| S-17 | Port motorunda ARQ yeniden iletim tavanı yok | UYUŞMAZ | `sender.rs` 2721; `arq.rs` 71–79 | A1S1-21, A3T-06 | G | Yerel |
| S-18 | ReorderTracker hiç azalmıyor; port paket eşiği 3 değil `max(3, 2·max)` | UYUŞMAZ | `arq.rs` 154–185; `sender.rs` 4095 | A6-07 | G | arq.rs |
| S-19 | Duplicate kaybı tombstone bırakmıyor, birim düşebiliyor | UYUŞMAZ | `sender.rs` 4650–4800; testler 13826–13875 | A3T-07 | G | Yerel |
| S-20 | `ack_confidence` eşiği `max(3·srtt, 2·EAI)` | UYUŞMAZ | `sender.rs` 9341–9344 | A2S2-12, A3T-03 | G | Yerel |
| S-21 | INV-RATE-1 kilit-golden testi varsayılan olmayan yolda koşuyor | UYUŞMAZ | `sender.rs` 11168–11278 | A3T-04 | G | Yerel |
| S-22 | Port motorunda "kayıp pacing hızını düşürmez" testi yok | EKSİK | `sender.rs` test modülleri | A3T-05 | K (yeni test) | Yerel |
| S-23 | ACK `from_received` boşluğu `as u16` ile kırpıyor; hiç alınmamış seq'ler teyit edilebiliyor | UYUŞMAZ | `ack.rs` 221, 600–602 | A4-01 | K | ack.rs |
| S-24 | Tanımsız tel kod noktaları: bitmap ACK 0x15–0x17, CAPS 0x08, yalın PTT 0x09, v0 interim aile (0x00–0x07), 0x05 DATA_FRAG, 0x1D PLANE1_RPC; noise-wire §2.56 ile çakışma | TANIMSIZ / UYUŞMAZ | `ack.rs` 18–75; `wire.rs`; `runtime.rs` 3737–3826, 5967–5992; `hub-server/src/wire.rs` 57 | A4-02, A6-17, C1R1-06/07, C2R2-03/04, C3-03, C3-05, H-HS-04 | G (D-32 (iii)) | çok dosya |
| S-25 | LineGuard'ın kural seti ve 20'den fazla sabiti türetilmemiş; pacer çift kipi; genel yoklama hükmü için ACK sınırı yok; boşluk bütçesi 3R/8 | TANIMSIZ | `line_guard.rs` 388–477, 1548–1564; `pacer.rs` 32–44, 112–127, 246–266 | A5-04, A5-08, A5-16, A3T-11 | G | Yerel |
| S-26 | `interval_sample` erken dönüşte aşağı yönlü aralık örneği sessizce atılıyor | TANIMSIZ | `line_guard.rs` 1460–1526 | A5-05 | G | Yerel |
| S-27 | `BtlBwFilter` ve `MinFilter` yalnız zamanla budanıyor, adet tavanı yok | UYUŞMAZ | `bw.rs` 182, 255, 264; `rtt.rs` 27–38 | A5-06 | K | bw.rs, rtt.rs |
| S-28 | §7.3 `estimator_confidence` ve `ack_compression` kuralı yok | EKSİK | `bw.rs` 228–271 | A5-07 | G | bw.rs |
| S-29 | Acil durum de-escalate dwell'i escalate'te başlamıyor; osilasyon serbest | UYUŞMAZ | `emergency.rs` 162–171 | A6-01 | K | emergency.rs |
| S-30 | ACK seyreltme (`ack_decimation`) tüketicisi yok; `ack_freq` sabit 2 | EKSİK | `emergency.rs` 205–207; `receiver.rs` 224 | A6-02, A4-14 | G | emergency.rs + Web |
| S-31 | "KALİBRE DEĞİL" fuzzy ve histerezis fixture'ları ürün yolunda; R4/SR1 eksik; acil durum tetiği spec içinde çelişkili | UYUŞMAZ / TANIMSIZ | `fusion.rs` 316–374, 503–548; `emergency.rs` 91–126 | A6-04, A6-13/14/15, A4-22 | G | fusion.rs, emergency.rs |
| S-32 | Deney düzenekleri eski motoru koşuyor (ürün port) | TANIMSIZ | `fs_harness.rs` 620; `udp_runner.rs` 639 | A6-08 | G | fs_harness.rs |
| S-33 | Dar uplink'te FEC sessizce kurulmuyor; parite 2 s zaman kapısı türetilmemiş | TANIMSIZ | `runtime.rs` 780–847, 3223–3233, 4419–4436 | C1R1-08/09, C3-10, C3-22 | G | runtime.rs |
| S-34 | `LinkRates` (0 hız) ve kuyruk boyları (0 → `mpsc::channel(0)` paniği) doğrulanmıyor | EKSİK | `runtime.rs` 746–778, 394–400, 2760 | C1R1-10/11 | K | runtime.rs |
| S-35 | Fragmenter birleştirici tahliyesi ACK'li chunk'ları kaybediyor (yalnız bench yolu) | UYUŞMAZ | `runtime.rs` 5207–5210, 5060–5078, 6137–6150 | C2R2-05 | K | runtime.rs |
| S-36 | Dosya penceresi tabanı motor uçuş tabanının yaklaşık yarısı | UYUŞMAZ | `lanes.rs` 282–297; `transport.rs` 1849–1860 | C4-02 | G | lanes.rs |
| S-37 | FILE penceresi yalnız kayıplı `try_acked` kanalından boşalıyor | UYUŞMAZ | `lanes.rs` 593–609; `transport.rs` 1819–1829 | C4-03 | K | transport.rs |
| S-38 | `send_init` hatası `.ok()` ile yutuluyor; niyet INIT'siz 7 gün bekliyor | EKSİK | `transport.rs` 2157–2167, 1552–1556 | C4-01 | K | transport.rs |
| S-39 | `peer_ready` eşin en son handle'ıyla çağrılıyor; yeni denemenin T3'ü erken durabiliyor | UYUŞMAZ | `kama-node/src/node.rs` 800–814; `hub_mode.rs` 162–166; `hub/endpoint.rs` 235–251 | N-01, C5-B4 | K (önce kırmızı dar test) | kama-node, hub/endpoint.rs |
| S-40 | Atanmamış denemenin tutulan RESPONSE'ları temizlenmiyor; sonraki deneme çakışmayla bitiyor | UYUŞMAZ | `hub/link.rs` 560–566, 629–632, 853–866 | C5-B2 | K | hub/link.rs |
| S-41 | StoreResp'ten önce gelen RESPONSE kriptoya verilmiyor, imzasız alanla eşleniyor | UYUŞMAZ | `hub/link.rs` 484–545 | C5-B1 | G | hub/link.rs |
| S-42 | Hub çerçevelerinde ve INIT metasında `dest_hash` ve `cert_ref` açık metin | UYUŞMAZ | `hub/link.rs` 1004–1019; `bind.rs`; `handshake.rs` 401–402 | C5-B3, W-NW-07 | G | hub/, noise-wire |
| S-43 | Yanıtlayanda kalıcı INIT tekrar koruması yok | EKSİK | `session_control.rs` 206–207, 713–836 | W-NW-01 | G (ara: bellek içi high-water K) | noise-wire |
| S-44 | HubChannel no-op (kimlik doğrulamasız); Hub düzlemi yalnız sayıp düşürüyor | EKSİK | `protector.rs` 938–1004; `seam.rs` 186–196 | W-NW-02, C3-16, C1R1-21 | G (M4) | noise-wire |
| S-45 | `s3::Opened` alanları pub; noise-wire §6 mühürlü tip kuralına aykırı | UYUŞMAZ | `kama-contracts/src/s3.rs` 107–113 | W-NW-03 | G | kama-contracts |
| S-46 | Eski `KamaNoiseWireProtector` API'si (çağıran PN'i) ve model tipler koşulsuz derleniyor | UYUŞMAZ / ÖLÜ | `protector.rs` 57–396; `legacy.rs`; `hub_sim.rs`; `init_flight.rs`; `restart.rs`; `retry.rs`; `session.rs` | W-NW-04/05/17/18 | K | noise-wire |
| S-47 | ADR-03: Ed25519/X25519 üretimde yalnız dalek; libsodium çapraz sınaması yok; BLAKE2s'in PRF rolü kayıtta yok; blake2 kama-app ve client-core'da | UYUŞMAZ | noise-wire, kama-provision, hub-server, kama-node `wiring.rs` 99–120 | W-NW-06, N-07, H-HS-14/15, M1 B-01, M2-06, C5-B19, P1-17 | G | çok crate |
| S-48 | ADR-05: `ccr-codec` boş; kanonik kodek beş crate'e dağılmış | UYUŞMAZ | `ccr-codec`; `adaptive-transport/read.rs`, `ack.rs`; `kama-provision/cert.rs`; `kama-contracts/s4.rs` | M2-02/03, A4-18, A6-29, M1 B-02/03 | G | çok crate |
| S-49 | `hub-server` kendi S4 kodek kopyasını kullanıyor | UYUŞMAZ | `hub-server/src/wire.rs` 1–6, 287–495 | H-HS-09 | K | hub-server |
| S-50 | Hub, her yeni hat için `a=`/`b=` dest_hash önekiyle kim-kimle kaydı yazıyor | UYUŞMAZ | `hub-server/src/stats.rs` 291–295; `relay.rs` 158–164 | H-HS-08 | K | hub-server |
| S-51 | S4 taslak (G-S4 kaydı yok); codepoint ve yapılar app-rpc / ccr / PoC K5 ile çakışıyor | UYUŞMAZ / DOĞRULANAMADI | `hub-server/src/wire.rs`, `table.rs`, `relay.rs`; `kama-contracts/s4.rs`; `hub/` | H-HS-01..07, H-HS-10, M1 B-04, C5-B5..B8, B16, B17, B20 | G | hub-server, hub/ |
| S-52 | hub-server ADR-02 (Tokio yok), ADR-13 (Storage trait yok), ADR-19 (JSON, sağlık uç noktası yok) dışında | UYUŞMAZ / EKSİK | `hub-server/src/main.rs`, `table.rs`, `config.rs` | H-HS-11..13, M2-35 | G | hub-server |
| S-53 | `kama-engine` admission: `k_actual`, `block_id`, `message_id`, `chunk_offset` ve `code_suite_id` denetlenmiyor (ürün yolunda değil) | EKSİK | `kama-engine/src/admission.rs` 42–102; `param_state.rs` 43–65 | M2-07/08/09/11 | K | kama-engine |
| S-54 | Deneysel `receiver_host`: başarısız çözüm defter kaydı sızdırıyor; tamamlanan blok yeniden açılıyor; `h_ext` yeniden kuruluşu sayacı şişiriyor | UYUŞMAZ | `kama-integration/src/receiver_host.rs` 265–292, 427–571 | M2-18/19/22 | K | kama-integration |
| S-55 | ADR-16: `reed-solomon-erasure` yerine `reed-solomon-simd`; trait adı `FecCodec` | UYUŞMAZ | `kama-fec-codec` | M2-05 | G | kama-fec-codec |
| S-56 | CLAUDE.md §2 / ADR-12 düzeni: `crypto` crate yok, adlar farklı, 20 üye | UYUŞMAZ | kök `Cargo.toml` 3–24 | M2-01 | G | — |
| S-57 | Provisioning yükleyicisi sertifika imzasını ve `issuer_key_id`'yi doğrulamıyor; `not_after` hiç denetlenmiyor | EKSİK / TANIMSIZ | `kama-provision/src/format.rs` 500–595; `cert.rs` 47 | M1 B-05/06/07 | G | kama-provision |
| S-58 | Kaynak sembolde `message_id = unit_id & 0xFFFF_FFFF`; şerit etiketi düşüyor, şeritler arası yeniden kullanım oluyor | UYUŞMAZ | `runtime.rs` 4156, 6070 | C3-02 | G (bayt değişir) | runtime.rs |
| S-59 | IdentitySeam 16 B ek yük (spec 22/26 B); CS probe gövdesi stub'a göre seçilmiş; bench bayt kıyasları eksik sayıyor | UYUŞMAZ | `seam.rs` 104–132; `wire.rs` 259–274; `bench/kama.rs` 35–68 | C3-09, C3-06, B-27 | G | seam.rs, bench |
| S-60 | Bench kıyas adaleti: motor ve risk alanları `profile_delta`'da yok; quinn `initial_mtu` 1452 (spec 1200); varsayılan Cubic; manifest CC/bütçe/warmup taşımıyor; TCP penceresi 60 s; qdgram ACK aralıkları eskiyi tutuyor; p50/p95/p99 yazılmıyor; quinn sürümü elle yazılmış | UYUŞMAZ / EKSİK | `bench/src/kama_product.rs`, `quic.rs`, `manifest.rs`, `tcp.rs`, `qdgram.rs`, `metrics.rs` | B-01/02/04/05/06/07/08/09/10/11/17 | K | bench |
| S-61 | §4.4 mekanizma metrikleri ve ikili blackout survival (BENCH-01) yok | EKSİK | `bench/src` | B-12, B-13 | G | bench |
| S-62 | kama-app: dedup kalıcı değil; RECEIPT tek deneme; MAX_PEERS 64 eşi çıkarmıyor; `connect_with_text` ürün yolunda değil | EKSİK / TANIMSIZ | `messaging.rs`; `store.rs` | P1-01..07 | L | donmuş (metin) |
| S-63 | zstd yok; TEXT→ON uygulanmıyor | EKSİK | `codec/compress.rs` | P2-12/13 | G | kama-app |
| S-64 | Signaling blob ≈ 1 KB tavanı; "deadline-bounded" belgesi Reliable davranışla çelişiyor; geçici ret kümesi tanımsız | EKSİK / UYUŞMAZ | `signaling.rs` 89–96, 214–224, 402–433, 614–622 | P2-15/16/17 | G | kama-app |
| S-65 | Ürün düğümünde `rtt_ms`/`loss_pct` daima None, `bitrate_kbps` hep None; `loss_pct` aslında yeniden iletim oranı | EKSİK / TANIMSIZ | `kama-node/src/node.rs` 1019–1034; `a5.rs` | N-03/04 | G | kama-node |
| S-66 | `PlatformSignal::LinkChanged` hub kipinde de etkisiz | EKSİK | `kama-node/src/node.rs` 541–547 | N-02 | G | kama-node |
| S-67 | Medya API'si, MEDIA_LINK ve RELAY_OPEN yok | EKSİK | `kama-node/src/lib.rs` 37–43; `hub/link.rs` 623–625 | N-14, C5-B16 | G | kama-node, hub/ |
| S-68 | `RateDropDetector` halkası adet tavansız | UYUŞMAZ | `arrival_rate.rs` 239–282 | A4-08 | K (Web ile eşgüdüm) | arrival_rate.rs |
| S-69 | Alarm ACK aralığı, kanıt ve çift saklama kuralları | TANIMSIZ | `receiver.rs` 306–545, 1474–1587; `arrival_rate.rs` 302–534 | A4-09, A5-17 | L (7186813951) | Web |
| S-70 | Port alıcısında 25 ms zamanlayıcı son emisyondan sayılıyor, bekleyen ilk varıştan değil | UYUŞMAZ | `receiver.rs` 1235–1242 | A4-11 | K | Web |
| S-71 | Alıcıda tanımsız kurallar: akış sonu yedeği, bilgisiz kopya ACK'inin bastırılması, yol verme, yeniden rapor, teşhis anahtarları, K_TAIL, RECOVERED'ın ACK'ten sonra gelmesi | TANIMSIZ | `receiver.rs` 205–208, 436–441, 697–702, 776–858, 975–1062, 1248–1269; `runtime.rs` 5468–5548 | A4-04/05/06/15/16/21, C2R2-16, C3-20 | G | Web, runtime.rs |
| S-72 | FEC `block_id` şerit içinde 30 bitte sessizce sarıyor | UYUŞMAZ | `unit_id.rs` 44–47, 102–136 | C3-01 | K | unit_id.rs |
| S-73 | Taban PTO formülü `clamp(srtt + 4·rttvar + mad, min_pto, …)`; spec `srtt + max(4·rttvar, min_pto) + mad`. Port motorunda geri çekilme tavanı 30 s | UYUŞMAZ | `tail_recovery.rs` 172–190 | A6-09, A6-10 | K (A6-09) · G (A6-10) | tail_recovery.rs |

### 4.3 Düşük (gruplanmış)

| ID | başlık | kaynak | düzeltme | yazar |
|---|---|---|---|---|
| S-74 | Erişilebilir panik yolları (`.expect` / `assert!` / indeks paniği / kapalı boruda `println!`) | A1S1-09, A2S2-25, C1R1-14, A5-09, A6-31, B-21, M2-27, H-HS-28 | K | sender.rs (Yerel) ve öteki dosyalar |
| S-75 | Sayaçsız sessiz düşüşler (teyit kanalı, alış/gönderim hatası, çözülemeyen ACK, `ConnectionFail`, MAX_EVENTS, hub tx kuyruğu, mühür hatası sayacı, manifest yazımı, cookie yenilemesi, `drain(..).take`) | C1R1-19, C2R2-10, A2S2-15, C4-04, C5-B11, C5-B14, B-22, H-HS-22, A6-30 | K | çeşitli |
| S-76 | Gönderici küçük sözleşme düzeltmeleri: boş ACK'te üç notun tüketilmemesi (A1S1-18); IW varsayılanının 10·MTU olması (A1S1-08); `w_epoch` sihirli sabiti ve ölü `ack_epoch` (A1S1-11); `probe_seq` çakışması (A1S1-23); §8.5'in fixture okuması (A2S2-16); eski motorda sessizlik çapası (A2S2-13); DrainRtprop W/2 (A3T-13); `Some(0)` sentinel (A3T-21); `parity_final` AMENDMENT-3 (A5-11) | aynı | K | Yerel (A5-11: budget.rs) |
| S-77 | Boş ya da ölü testler ve sayaçlar | A1S1-17, A5-12, A5-13, A3T-16/17/18, C3-18, N-15, A6-23, A6-26, M2-32, C5-B18, C2R2-11 | K (C2R2-11: G, pub yüzey) | çeşitli |
| S-78 | Belge ve yorum kayması | A1S1-15/16, A2S2-20, A5-14, A6-11, A6-18, A6-28, C1R1-23, C4-15, N-06, B-24, W-NW-26, M1 B-10/11/13, M2-36, P2-10, Y-03 | K | çeşitli |
| S-79 | Deneysel ya da ölü kolların tutulması ya da kaldırılması | A2S2-11/21/22, A3T-23, A5-10, A5-15, A6-24, A6-27, M2-13, M2-17, P2-14, C1R1-20 | G | çeşitli |
| S-80 | Hub istemcisi: BAD_REQUEST'e 5 s geri çekilme, sayaç ayrımı | C5-B9, B10, B14 | K | hub/ |
| S-81 | Öteki sınırsız yapılar (fate provisional, fs_trace, bench haritaları, noise-wire model tipleri, kama-vectors okuma, memory_link) | A6-22, A6-35, B-20, W-NW-18, M2-33, C3-17 | K | çeşitli |
| S-82 | `ControlDedupWindow` pencere içindeki id 0'ı "pencere altı" sayıyor (deneysel) | W-NW-08 | K | noise-wire |
| S-83 | noise-wire: RECOVERED sayısının `as u16` yazılması, KS-GRACE/KS-DOS sayaçları, CCR 8192 sınırı | W-NW-10, W-NW-13, W-NW-23 | K | noise-wire |
| S-84 | Küçük crate düzeltmeleri (ters ESI, tahliye sırası, bozuk 0x19, atomik serbest bırakma, uzunluk varyantı, katalog dışı profil, lint/lisans) | M2-15/16/24/26/28/31/34 | K | çeşitli |
| S-85 | Bekletilen dosya birimi pencere daraldıktan sonra da veriliyor | C4-14 | K | lanes.rs |
| S-86 | Kopya `EntropySource`; `encode_recovered` sessiz kırpma; `release_outstanding` doyması | C3-07, C3-14, C3-19 | K | client-core |
| S-87 | `ErrorPayload` `detail_len > 0` kabul ediliyor | M1 B-08 | K | kama-contracts |
| S-88 | kama-node: `restart_attempt` hatası yutuluyor; sabitler türetilmemiş | N-10, N-11 | K | kama-node |
| S-89 | Davranış-nötr bağlar: `thiserror`; PTT sabitleri için kama-node'da `const assert` | P1-18, P2-07 | K | kama-app, kama-node |
| S-90 | Probe: çözülemeyen yanıt Timeout sayılıyor; `format_version` denetlenmiyor | H-HS-27 | K | hub-server |
| S-91 | Geri bildirimi aç yolda burst tavanı `cc.window()`'dan (recovery dahil) hesaplanıyor | Y-01 (yeni) | G (S-01 ile birlikte) | Yerel |
| S-92 | Öteki düşük karar-gerekir kalemleri | A1S1-03/07/10/22, A2S2-14/17/22, A3T-09/12/14/15/19/20/24, A4-05/07/10/12/13/17/19/22, A6-10/12/16/19/20/21/25/32/33/34, C1R1-12/13/15/16/17/18/22, C2R2-06/07/08/09/12/13, C3-11/12/15/21, C4-05/06/07/08/09/10/11/16/17, C5-B12/13/15, N-08/09/12/13, P1-08..17, W-NW-09/11/12/14/15/16/19..22/24/25/27/28, H-HS-17..21/23..26, H-KS-01, M1 B-09/12/14, M2-10/12/14/20/21/23/25/29/30, B-14..16/18/19/23/25/26/28 | G / K / L (fid'e göre; okuyucu raporundaki sınıf geçerli) | çeşitli |

Not: S-92 bir sepettir. A4-19, A6-16/19, C4-16, C1R1-18, C3-15 gibi kod-duzeltmesi sınıfındaki küçük kalemler de içinde; sınıfları okuyucu raporlarındaki gibidir. M2-30 ve B-28 kilitlidir.

### 4.4 kod-duzeltmesi: uygulama sırası ve doğrulayan var olan testler

Sıra, davranışı değiştirmeyenden değiştirene doğru ilerliyor. Ürün davranışını değiştiren her adım ayrı bir PR olmalı ve aynı senaryo ile tohumla önce/sonra gösterilmeli (sahip 7171705736, uygulama sırası).

| adım | kalem | bağımlılık | doğrulayan var olan test |
|---|---|---|---|
| 0.1 | S-22 port INV-RATE-1 birim testi (kayıp ilanı öncesi ve sonrası pacer hızı = `pacing_gain × BtlBw`, kural yokken) | — | yeni test; ayrıca `adaptive-transport/tests/tv_src.rs`, `n8_reverify_emit.rs`, `cc_port_sim.rs` |
| 0.2 | S-77 boş testler (A5-12 ProbeRTT, A3T-16 Karn, A3T-17 Drain, A3T-18 monoton saat, C3-18, N-15) ve ölü sayaçlar (A1S1-17) | — | `cc/bbr.rs` iç testleri; `tests/dr_cs3_structural_oracle.rs`; `kama-node` `hub_mode` testleri |
| 0.3 | S-78 belge düzeltmeleri, S-89 `thiserror` ve `const assert` | — | `cargo doc`, derleme |
| 1.1 | S-74 panik yolları → tipli hata | — | `tests/e2a_goster.rs`, `sprint5c_fec_emit.rs`, `budget_kernel_sim.rs`, `kama-transport-schema/tests` |
| 1.2 | S-34 `LinkRates` ve kuyruk boyu doğrulaması; A3T-21 `NonZero` | — | `client-core/tests/product_profile_v1.rs`, `cc_port_runtime.rs`; `sender.rs` `profile_pacing_cap_tests` |
| 1.3 | A4-20 ACK `encode` tipli (S-92 içinde) | — | `tests/tv_ack.rs`, `ack_ref_codec.rs`; `kama-integration/tests/ack_ref_vectors.rs`, `ack_ref_wire_size.rs` |
| 2.1 | S-07 boş motor değeri → CcPort ya da hata (runtime + kama-node + bench birlikte) | 0.3 | `client-core/tests/cc_port_runtime.rs:103` (beklenti değişir); `kama-node/tests/start_config.rs`; bench `kama_product` birim testleri |
| 3.1 | A1S1-18 boş ACK'te üç notun tüketilmesi | — | `tests/first_ack_window.rs`, `start_budget_sim.rs`, `line_guard_sim.rs`, `starved_line_sim.rs`; ürün: `kama-integration/tests/asym_product_outcome.rs` |
| 3.2 | S-13 en sıkı son tarihin canlı kümeden hesaplanması | — | `tests/fec_loss_risk.rs` |
| 3.3 | S-14 `pending_fec` tavanı ve `manual_r` kırpması | — | `tests/sprint5c_fec_emit.rs`, `fec_repair_timing.rs`; `kama-engine/tests/controller_actuation.rs` |
| 3.4 | A2S2-16 `self.budget_cfg`; A5-11 `parity_final` | — | `tests/budget_equivalence_pin.rs`, `budget_live_tests.rs` |
| 3.5 | A1S1-08 kurucu varsayılanı 4·MTU (ürün runtime zaten eziyor) | — | `tests/initial_window.rs`; `client-core/tests/cc_port_runtime.rs` |
| 3.6 | A1S1-11 `w_epoch` sabitten türetme (bit-aynı) | — | `tests/sender_state_thresholds.rs`, `sim_determinism.rs` |
| 3.7 | A1S1-23 tek `probe_seq` sayacı (`--all-features`) | — | `tests/dr_cs1_acceptance.rs`, `dr_cs3_structural_oracle.rs`; `kama-integration/tests/dr_cs3_*` |
| 3.8 | A2S2-15 `ConnectionFail` tipli olay | — | `tests/tv_arq.rs` |
| 3.9 | Eski motor: S-10 PreConverge PTO, A2S2-13 çapa, A3T-13 DrainRtprop (geri alma yolu; ürün değişmez) | — | `tests/cold_start_liveness.rs`, `establish_probe.rs`, `ack_confidence_idle.rs`, `sender_state_matrix.rs`; `sender.rs` test beklentileri spec'e çekilir |
| 4.1 | S-27 tekdüze deque + adet tavanı (`btlbw()` / `rtprop()` bit-aynı) | — | `tests/tv_bw.rs`, `tv_rtt.rs`, `cc_port_sim.rs`, `sim_determinism.rs`, `soak_long_session.rs` |
| 4.2 | S-81 fate provisional tavanı ve öteki sınırlar | — | `tests/estimator_sim_integration.rs`, `loss_structure_observation.rs` |
| 4.3 | S-68 `RateDropDetector` adet tavanı (alarm yolu Web'de, eşgüdüm) | Web | `tests/ack_alarm_interval.rs`, `ack_alarm_planned.rs` |
| 5.1 | S-29 de-escalate dwell çapası | — | `tests/tv_emergency.rs`, `emergency_and_accessors.rs` |
| 5.2 | S-73 (A6-09) taban PTO formülü (port PTO zamanlaması değişir) | 3.x | `tests/cc_port_sim.rs`, `e2a_goster.rs`, `tail_recovery.rs` iç testleri; ürün: `asym_product_outcome.rs` |
| 5.3 | S-23 ACK boşluğu > u16 iken eski aralıkları kesme | — | `tests/tv_ack.rs`, `ack_ref_delivery.rs`, `kani_seq_proofs.rs` |
| 5.4 | S-70 port alıcısında zamanlayıcının ilk bekleyen varıştan sayılması | Web | `tests/receiver_ack_policy.rs`, `ack_uplink_yield_once.rs` |
| 6.1 | S-37 FILE penceresinin `lane_confirm_frontier`'dan boşalması; S-85 | — | `client-core/tests/lane_file_window_link_profile.rs`, `lane_file_window_bdp.rs`, `lanes_e2e.rs`, `lane_finite_head_loss.rs` |
| 6.2 | S-38 `send_init` hatası → Backpressure ya da yeniden sunum; C4-04 olay odası | — | `client-core/tests/hub_relink.rs`, `lanes_renewal.rs`; `kama-node/tests/api_paths.rs` |
| 6.3 | S-35 fragmenter kapasitesinin ACK'ten önce denetlenmesi | — | `client-core/tests/runtime_e2e.rs`, `ack_after_admit.rs`, `fec_admit_full.rs` |
| 6.4 | S-39 `peer_ready`'nin oturumun hub hattıyla çağrılması (`ActiveLink::On(h)`) | 6.2 | `kama-node/src/node/tests/intent_t3.rs`, `hub_relink.rs`; `kama-integration/tests/s4_hub_relink.rs` |
| 6.5 | S-40 atanmamış denemenin tutulan RESPONSE'larının `retire`'ı; S-80 | — | `client-core/src/hub/link_tests.rs`, `link_glare_tests.rs`; `kama-integration/tests/s4_hub_client.rs` |
| 6.6 | S-72 `block_id` tavanında tipli ret | — | `client-core/tests/transport_v1_freeze.rs` (bayt-aynı), `fec_recovered_partial_admit.rs` |
| 6.7 | S-86, S-88, S-75 (client-core / kama-node payı) | — | `client-core/tests/runtime_e2e.rs`; `kama-node/tests/two_nodes_loopback.rs` |
| 7.1 | S-49 `hub-server` → `kama_contracts::s4` | — | `kama-integration/tests/s4_vectors.rs`, `s4_hub_smoke.rs`; `hub-server/tests/tdd`, `udp_loopback.rs` |
| 7.2 | S-50 log satırından dest_hash öneklerinin çıkarılması; S-90; H-HS-22 | — | `hub-server` `stats.rs` `event_lines` testi; `hub-server/tests/udp_loopback.rs` |
| 7.3 | S-87 `ErrorPayload` katılığı | 7.1 | `kama-integration/tests/s4_vectors.rs` |
| 7.4 | S-82, S-83 | — | `kama-noise-wire/tests/a2_f3_control_dedup_window.rs`, `err_paths_control.rs`, `a5c_handshake_vectors.rs` |
| 7.5 | S-46 eski API'nin test-util / deneysel bayrak arkasına alınması | 7.4 | `kama-integration/tests/a2_f0_receiver_host.rs`, `a2_f3_control_ingest.rs`; `kama-noise-wire/tests/sprint1_core.rs` |
| 7.6 | S-53 admission denetimleri; S-84 | — | `kama-engine/tests/admission_5d2.rs`, `param_bootstrap_5d1.rs` |
| 7.7 | S-54 `receiver_host` | 7.5 | `kama-integration/tests/a2_f0_receiver_host.rs`, `a2_c1b_unified_heal.rs`, `a2_f2_product_ingest.rs` |
| 8.1 | S-60 bench kıyas adaleti. §4.4 kapsamında bir quinn kıyas iddiasından önce yapılmalı. | 2.1 | bench birim testleri (`manifest.rs`, `kama_product.rs`, `quic.rs`, `qdgram.rs`, `tcp.rs`, `metrics.rs` test modülleri); `scripts/netem` kapı manifesti |

**Yerel dosyalarında (line_guard.rs, pacer.rs, sender.rs) kod-duzeltmesi olanlar:** S-10, S-13, S-14, S-22, S-76 (A5-11 hariç), S-74 (sender payı), S-77'den A1S1-17 ve A5-13. `line_guard.rs` ve `pacer.rs`'te kod-duzeltmesi yalnız A5-13 (ölü test yardımcısı). Bu iki dosyadaki öteki her bulgu karar-gerekir ya da kilitli; S-01 kararı verilmeden dokunulmamalı.

## 5. Sahip kararı gereken sorular

1. **Kontrolcü yönü.** Geri bildirimi aç profilde pacing ve uçuşu kim belirleyecek? Seçenekler: bugünkü tasarım (INV-RATE-1'e YAMA ile), Yön A ya da Yön B (§3.2). Bu karar S-01, S-02, S-25, S-26 ve S-91'i birlikte kapatır.
2. **Taşma kuralı.** `overflow_estimate`'in kayıp kesrinden hız düşürmesi INV-RATE-1'e tarihli bir istisna mı olacak, yoksa kayıp kesri hız yolundan çıkacak mı (S-03)?
3. **K2′ sonrası.** `on_pair_rate` doğrudan açma yolunun ürün durumu ne olacak (S-04)? #262 park dalı ve Web PR'ı geldikten sonra verilir.
4. **Sunucu yuva emekliliği.** INV-NO-IDLE-TEARDOWN ile uyumlu biçim hangisi (S-05)? Seçenekler: emeklilik yalnız oturum kapanınca, PARAM/CAPS bağı oturum başına ya da yeni yuvada yeniden bağ kurma.
5. **CC-PORT YAMA'sında listelenmeyen port farkları yamaya alınsın mı?** E2a v1.1 örnek atlama (S-08), PreConverge PTO U14 (S-09), retx tavanının kaldırılması (S-17), ReorderTracker (S-18), 30 s PTO geri çekilme tavanı (A6-10), Duplicate tombstone (S-19), `ack_confidence` 2·EAI (S-20), INV-RATE-1 kilit-golden'ın yeniden kapsamlanması (S-21).
6. **DR-EST-1 kuruluş kopyası.** Pacer ve uçuş penceresi muafiyeti spec'e işlensin mi, yoksa pacer'a mı bağlansın (S-11)?
7. **Spekülatif FEC kolu ve Q18 yapı kapısı.** Spec'e mi alınsın, ürün varsayılanından mı çıksın (S-12)? Kalibrasyon kampanyası: fuzzy, histerezis ve LineGuard sabitleri (S-31, S-25).
8. **Zamanlama.** KS-PN-1 K = 1024 (S-16) ile son tarih ve IDLE/PATH_DEAD zamanlayıcıları (S-15) ne zaman yapılacak?
9. **Tel kod noktası tahsisi.** Bitmap ACK, CAPS, yalın PTT, v0 interim aile, 0x1D ve S4 Y1–Y12 için tahsis gerekiyor; ayrıca G-S4 kapısının durumu bilinmiyor (S-24, S-51, S-58).
10. **ADR sapmaları.** ADR-03 (dalek), ADR-05 (`ccr-codec`), ADR-16 (simd), hub-server için ADR-02/13/19 ve CLAUDE.md §2 crate düzeni (S-47, S-48, S-52, S-55, S-56) için ne yapılacak?
11. **Güvenlik açıkları.** HubChannel M4 (S-44), hub çerçevelerinde açık metin `dest_hash` (S-42), kalıcı INIT tekrar koruması (S-43), `s3::Opened` mühürü (S-45), provisioning'de imza doğrulaması (S-57).
12. **Pencere ve FEC.** Dosya penceresi tabanı motorla tek kaynaktan mı okunsun (S-36)? Dar uplink'te sessiz FEC kapanışında hangi kural geçerli (S-33)?
13. **Benchmark ve ürün metrikleri.** §4.4 mekanizma metrikleri ve BENCH-01 (S-61); kama-node RTT/kayıp/bitrate tanımı (S-65), LinkChanged (S-66), medya API'si (S-67).

## 6. Kilitli kalemler

| kalem | kilit dayanağı |
|---|---|
| S-06 (PTT ve metin politikası: yalın PTT, DeadlineBounded demet, birleşik biçim, kopya sayıları, sıra başı atlama, işaretli kodek biçimleri) | sahip 7171705736 madde 3: «PTT ve metin davranışını dondurun» |
| S-62 (kama-app metin davranışı) | aynı, madde 3 |
| S-04 (`on_pair_rate`), S-69 (alarm aralığı), A5-17 | #262 dalı parkta; `receiver.rs` alarm aralığının tek yazıcısı Web (7186813951); K2′ deneyi (7172579876) ürün varsayılanını değiştirme yetkisi vermiyor |
| M2-30 (E=1024 profilinin dar bantta kullanılması) | karar 3 sürerken ürün profil kimliği değişmez |
| B-28 (karışık akışın PTT yük varsayılanları) | karar 3 |

## 7. Eksik ve stub alanlar

**CLAUDE.md §2 crate'leri:**

- `ccr-codec`, `discovery`, `ra-ca`, `admin-server`: her biri 3 satırlık yorum. Pub öğe, trait, typed-error stub ya da bağımlı yok. `ccr-codec` için bu, CLAUDE.md §6 "API-first: crypto + ccr-codec trait imzalarını ÖNCE indir" kuralının karşılanmadığı anlamına geliyor.
- `crypto` crate'i workspace'te yok. Kripto `kama-noise-wire`'da (M2-01).
- `hub-server`, `client-core` ve `adaptive-transport` dolu.

**Spec'te olup kodda olmayan büyük alanlar:**

- discovery-architecture: VOPRF kişi keşfi yok.
- pki-ca-ra-hsm-tdd ve revocation-model: CA/RA/HSM yok; 3 katmanlı iptal ve hub deny-cache yok.
- admin-control-plane-tdd: Admin↔RA yok.
- routing-rendezvous-architecture: hub-mesh DHT, ANNOUNCE ve `verify_chain` yok; HubEntryV3 cert-bound hub kimliği yok.
- application-protocol-and-rpc-spec:
  - Noise IK CLIENT_BIND ve ROOT_CA zinciri yok (PoC bind iki turlu ve imzalı).
  - Mailbox fallback (DR-CONNECT-1 H7) yok.
  - HubChannel hop anahtarı (M4) yok.
  - FAILOVER yok.
  - S4 alt kümesi dışındaki RPC'lerin durumu ayrıntıda DOĞRULANAMADI.
- adaptive-transport-engine-spec:
  - §7.3 `estimator_confidence` yok.
  - §2.1/§11 ACK seyreltme yok.
  - §12 IDLE_TIMER, PATH_DEAD_TIMER ve RETRY_BUDGET yok; §16.1 COPY_MAX_INTERVAL yok.
- transport-layer-tdd: §12 KS-PN-1 yok; §4.4 mekanizma metrikleri yok; ikili blackout survival (BENCH-01) yok.
- noise-wire-tdd:
  - §0.7 kalıcı INIT tekrar koruması yok.
  - Bayt tabanlı yenileme tavanı yok.
  - KS-GRACE/KS-DOS sayaçları yok.
  - Extended zarf ürün yolunda yok (uç başına tek kip).
- Kodlama ve kanonik biçimler:
  - AMP §5 zstd sıkıştırması yok.
  - ccr §35.14 BatchPayload kodeği var ama veri yolunda değil.
  - Tek kanonik kodek crate'i yok (ADR-05).
- ADR'ler:
  - ADR-13 Storage trait hub'da yok.
  - ADR-19 sağlık ve metrik uç noktası yok.
  - ADR-03 libsodium Ed25519/X25519 yolu yok.
- Medya: MEDIA_LINK, RELAY_OPEN ve kama-node medya API'si yok. Medya ayrı depoda; bu yalnız seam tarafı.

## 8. Sentezcinin kendi bulguları ve okuyucu düzeltmeleri

- **Y-01 (yeni, TANIMSIZ, düşük).** Geri bildirimi aç yolda uçuş tavanı `window_without_recovery`'den gelir (2766–2769). Burst tavanı ise `PacerParams::from_window(cc.window(), …)` ile hesaplanır (3155, 3187); `cc.window()` recovery'de `min(cwnd, recovery_window)`, ProbeRTT'de probe penceresidir (`cc/bbr.rs` 596–605). Yani 2026-11-16'nın "recovery_window uygulanmaz" kuralı uçuşta uygulanıyor, burst'te uygulanmıyor. Dar hatta etkisi 10·MTU tabanı ve hat modeli yüzünden beklenmez; geniş profilde burst değişebilir. Kod okuması, ölçülmedi. S-01 kararıyla birlikte ele alınmalı (S-91).
- **Y-02 (gözlem).** Kural varken BBR'ın pacing hızı, ProbeBW kazanç döngüsü ve recovery'si hiçbir karar yolunda okunmuyor; ProbeRTT penceresi tabanın altında kalıyor. BBR fiilen pasif bir tahminci. Uçuşa yalnız `max` içindeki `cwnd` ile, hıza hiç girmiyor (§3.1). Kod okuması, ölçülmedi.
- **Y-03 (belge düzeltmesi; S-78 içinde).** `sender.rs` 2785–2787 ve 4364 yorumları "ilk ölçümden önce profil hattının yarısından başlar" diyor. Kodda `START_NUM/START_DEN = 1/1` (`line_guard.rs` 412–413). İlk tavan profil hattının kendisi. P/2 yalnız ilk ACK'ten sonraki geçici evrede kuruluyor (`line_guard.rs` 1787–1800) ve tek ölçümün alt sınırı olarak kalıyor (1765–1767). A1S1-01'in davranış metnindeki "yarım profil" ifadesi bu bayat yoruma dayanıyor; bulgunun kendisi geçerli.
- **Okuyucu düzeltmesi (A5-03 ve A5-04 "spec'te bulunamadı").** LineGuard'ın ölçüm mekanizmasının bir kısmı (ilk pencere, aralık ve iki nokta tahmini aşağı, süreli yoklama yukarı) engine-spec başlığındaki YAMA KAYDI 2026-11-21 (§2.2) içinde adsız olarak tanımlı. Tanımsız olanlar şunlar: ölçünün pacing'in kaynağı olması, taşma kuralı, çift alt sınırıyla doğrudan açma, boşluk bütçesi ve çift kipi. Bulguların sınıfı değişmiyor, gerekçe daralıyor.

## 9. Okunan otorite bölümleri

- CLAUDE.md §2–§10 ve YAMA KAYDI (INV-RATE-1 2026-11-11 metni ve 2026-11-16 profil istisnası).
- #175 sahip kararları (doğrudan okunanlar): 6920942098 (2026-11-16 profil paketi), 7171705736 (dokuz karar), 7172579876 (K2′), 7186813951 (#262 kapsamı). Okuyucuların okuduğu öbürleri: 7174289733, 7174845328, 7175487850, 7175498704, 7175888074.
- `specs/adaptive-transport-engine-spec.md`: başlık YAMA KAYDI 2026-11-21 (§2.2 ve §12 [R1]); §0.3 CC-PORT YAMA (1)–(8) ve SRC kutusu (77–104); §3 ve §5 YAMA (464–466); §13 YAMA (982). Okuyuculardan geçen bölümler: §0–§17, Ek, Ek-B.
- `specs/transport-layer-tdd.md` (§0, §4.4, §5, §10 AMENDMENT-10, §12 AMENDMENT-9/11, §14), `specs/tech-stack-adr.md` (ADR-01..19), `specs/ccr-format-spec.md` §35, `specs/application-protocol-and-rpc-spec.md`, noise-wire TDD, fec-core-tdd v3.0. Bunlar okuyucu raporları üzerinden kullanıldı.
- `docs/decisions/KARAR-KAYDI-2026-11.md` (§8 D-CC-1) ve `docs/design/CC-PORT-ESLEME.md` (normatif değil). Okuyucu raporları üzerinden kullanıldı.
