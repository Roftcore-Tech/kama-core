# A5-hiz-cc — hız, uçuş ve kayıp tepkisi denetimi (LineGuard · pacer · SRC · BtlBw · RTT · cc-port · bütçe)

Taban: `125b9afa87e731c1e666ecf63f33b7cc555dc9b1` (ürün hattı; aday `1a4f9efc` bunun içinde). Salt okuma: kod değiştirilmedi, cargo koşturulmadı. Satır numaraları bu tabandandır.

## Kapsam

| dosya | okunan satır | satır |
|---|---|---|
| `crates/adaptive-transport/src/line_guard.rs` | 1–3058 | 3058 |
| `crates/adaptive-transport/src/pacer.rs` | 1–331 | 331 |
| `crates/adaptive-transport/src/rate.rs` | 1–325 | 325 |
| `crates/adaptive-transport/src/bw.rs` | 1–329 | 329 |
| `crates/adaptive-transport/src/rtt.rs` | 1–248 | 248 |
| `crates/adaptive-transport/src/cc/mod.rs` | 1–90 | 90 |
| `crates/adaptive-transport/src/cc/bbr.rs` | 1–796 | 796 |
| `crates/adaptive-transport/src/cc/min_max.rs` | 1–121 | 121 |
| `crates/adaptive-transport/src/cc/bw_estimation.rs` | 1–116 | 116 |
| `crates/adaptive-transport/src/budget_kernel.rs` | 1–778 | 778 |
| `crates/adaptive-transport/src/budget.rs` | 1–54 | 54 |
| **toplam** | | **6246** |

Bağlam için (envantere girmedi, yalnız çağrı yeri doğrulaması): `sender.rs` 1770–1828 (`FeedbackStarvedRule`), 2755–2790 (`cc_window`, `cc_feedback_floor`, `cc_feedback_cap`), 3160–3180 (port pacing hızı), 4225–4240 (`set_external_bw`), 5040–5090 (§8.5 clamp çağrısı), 7654, 7860–7900 (bütçe çekirdeği bağlantısı); `udp_runner.rs` 100–130; `Cargo.toml` özellik satırları.

## Davranış envanteri

### `cc/mod.rs` (quinn `pacing.rs` izdüşümü)
- `BURST_INTERVAL_NS = 2 ms`, `MIN_BURST_SIZE = 10`, `MAX_BURST_SIZE = 256`, `PACING_N_PM = 1250` — quinn 0.11.18 `pacing.rs` sabitleri (türetilmiş değil, kaynaktan aynen).
- `PacerParams::from_window(window, srtt_ns, mtu)`: `rate_bps = window·1,25/srtt`, `burst_cap = clamp(window·2 ms/srtt, 10·MTU, 256·MTU)`; srtt 0 → 1 ns. Durum değiştirmez.
- Testler: quinn formülleri (12 000 B / 300 ms → 50 000 B/s, tavan 12 000), burst tavanının 10·MTU altına inmemesi.
- Uyumlu: 4 davranış (engine-spec §13 YAMA 2026-11-11 burst formülü; §0.3 madde 6).

### `cc/min_max.rs` (quinn `MinMax` birebir)
- `MinMax` tur sayısı pencereli (varsayılan 10) üç örnekli max filtresi: `update_max`, `subwin_update`, `get`, `reset`.
- Test: quinn'in kendi referans dizisi.
- Uyumlu: 4 davranış (birebir port).

### `cc/bw_estimation.rs` (quinn `BandwidthEstimation`)
- `on_sent` / `on_ack`: kümülatif gönderilen/teyitli bayt ve zamanları; örnek = `min(send_rate, ack_rate)`; uygulama sınırlı örnek filtreye girmez; filtre yalnız örnek mevcut max'tan büyükse güncellenir (quinn ile aynı).
- `bytes_acked_this_window` / `end_acks`: çerçeve başına teyitli bayt.
- `bw_from_delta`: `bytes·1e9/delta` (u128 ara tip), delta 0 → `None`.
- Testler: min(send, ack), uygulama sınırlı örneğin dışlanması.
- Uyumlu: 5 davranış. Bant tahmini olarak yalnız `external_bw` yokken okunur (U6).

### `cc/bbr.rs` (quinn `bbr/mod.rs` portu)
- Sabitler: Startup kazancı 2885‰, Drain 346‰ (=1 000 000/2885), ProbeBW döngüsü [1250, 750, 1000×6]‰, cwnd kazancı 2000‰, Startup büyüme eşiği 1250‰ / 3 tur, ProbeRTT 200 ms, min_rtt ömrü 10 s, ProbeRTT penceresi 0,75·BDP, `min_cwnd = 4·MTU` — hepsi quinn/BBR'dan (f32 → ‰).
- `PACKET_CONSERVATION_IN_STARTUP = true` (U9): Startup'ta da `min(cwnd, recovery_window)`; quinn'den sapma, Linux BBR v1'den.
- Durum makinesi: `update_recovery_state` (NotInRecovery → Conservation → Growth → çıkış), `update_gain_cycle_phase` (ofset daima 0, U3), `maybe_exit_startup_or_drain`, `maybe_enter_or_exit_probe_rtt`, `check_if_full_bw_reached` (kayıpta da tam bant kabulü), `calculate_pacing_rate` (Startup'ta düşmez; ilk RTT'de `init_cwnd/min_rtt`), `calculate_cwnd` (tam bantta `min(hedef + ack_height, cwnd + acked)`, Startup'ta yalnız büyür), `calculate_recovery_window` (quinn ile aynı aritmetik).
- Arayüz: `on_sent(now, bytes, seq)`, `on_ack(now, bytes, app_limited, rtt_min)` (U8: ilk min_rtt örneği ProbeRTT saatini başlatır), `on_end_acks`, `on_congestion_event(lost_bytes)` (yalnız biriktirir), `window()` (ProbeRTT → probe penceresi; recovery → `min(cwnd, recovery_window)`), `window_without_recovery()` (profil istisnası için), `pacing_rate()`, `measured_pacing_target()` (= `pacing_gain × bw_estimate`), `set_external_bw` (U6; 0 → `None`), `delivery_rate`, `min_rtt_ns`, `initial_window`, `in_recovery`, `diag`.
- `rfc9002_initial_window(mtu) = min(10·MTU, max(2·MTU, 14 720))`.
- Testler: IW 12 000; Startup → Drain → ProbeBW; ProbeBW penceresi ≈ 2·BDP; kayıpta recovery ve tavan; 10 s sonra ProbeRTT (zayıf, A5-12); iki örnek determinizmi.
- Uyumlu: 22 davranış (engine-spec §0.3 YAMA maddeleri 2–3; INV-RATE-1 2026-11-11 metni: kayıp yalnız `recovery_window` ile uçuş tavanını sınırlar, pacing hızına girmez — `calculate_pacing_rate` kayıp okumaz; ADR-17 determinizm, RNG yok).

### `bw.rs` (engine-spec §7)
- `SentPacket` (opak token; `is_app_limited`, TUR-D1 `gain_ge1`), `RateSample` (delivered, interval, app_limited, gain_ge1, ack_elapsed, sent_ns).
- `DeliveryRateSampler::on_sent / on_ack`: §7.1 — `interval = max(ack_elapsed, send_elapsed)`, `delivered = C − P`; uçuş boşken taban tazelenir. `forget_in_flight` (TUR-3a sızıntı düzeltmesi), `credit_delivered` (Karn: örneksiz teslim kredisi), `delivered`, `packets_in_flight`.
- `BtlBwFilter`: `W_BTLBW = 10 s` taban, `set_window` yalnız büyütür (§6 `max(10 s, K_bw × ack_epoch)`), `MIN_RATE_INTERVAL = 1 ms`, `valid = false` örnek girmez, uygulama sınırlı örnek yalnız `≥ BtlBw` ise girer, KEEP-LAST. `last_full_bw`, `last_offered_bw` (salt gözlem), `window_ns`, `samples_snapshot`, `diag_accept` (özellik `diag-sched-telemetry`).
- `bw_bps(delivered, interval)`: tamsayı, interval 0 → 0.
- Uyumlu: 7 davranış (§7.1, §7.2, §7.3 app-limited kuralı, §6 KEEP-LAST, INV-ENGINE-BW: kayıp girmez). Eksik/uyuşmaz: A5-06, A5-07.

### `rtt.rs` (engine-spec §6)
- `RttEstimator::observe(now, sample)`: RFC 6298 (α = 1/8, β = 1/4; ilk örnekte rttvar = s/2); kuantum döngüsü `n = clamp(Δt/T, 1, 64)` (TUR-3a.5-β); RTprop min filtresi (`W_RTPROP = 10 s` taban, `set_rtprop_window` yalnız büyütür); qd kısa min filtresi (`set_qd_window ≤ rtprop penceresi`).
- `queue_delay() = min_{W_qd}(rtt) − RTprop` (≥ 0); `rtprop_age_ns`, `rtprop_window_ns`, `srtt`, `rttvar`, `rtprop`.
- `rtt_sample(raw, ack_delay)` (negatifse `None`), `apply_ack_delay(raw, ack_delay, min_rtt)` (RFC 9002 §5.3 korumalı; altına inerse ham örnek, atlamaz).
- Uyumlu: 8 davranış (§6 tüm formüller, TUR-1x, TUR-3a D-2/1–2, TUR-3a.5 α/β). Uyuşmaz: A5-06 (sayısı sınırsız örnek vektörü).

### `rate.rs` (engine-spec §8.2, eski motor)
- `CWND_GAIN = 2`, `DEFAULT_MTU = 1200`; `Gain` (STEADY 1, PROBE_UP 5/4, PROBE_DOWN 3/4, STARTUP 2885/1000, DRAIN 1000/2885); `PacingPhase::gain`.
- `SrcConfig` (cwnd_gain 2, MIN_CWND 4·MTU); `bdp_bytes`; `evaluate` → `SrcDecision {bdp, cwnd = max(2·BDP, MIN_CWND), pacing_rate = gain·BtlBw, send_rate = min(pacing, cwnd/srtt)}`; `effective_send_rate` (füzyon ‰ kazancıyla).
- E1 gösterim bayrakları `E1Flags` (beşi de varsayılan kapalı), `E1_DRAINRT_QD_MIN_PERMILLE = 250`, `E1_PLATEAU_QD_BRAKE_PERMILLE = 250`, `E1_STARTUP_CWND_GAIN`, `cwnd_with_gain`, `effective_send_rate_cwnd_gain`.
- Testler: bayrak varsayılanı, 2/1 kazançta eski formülle eşitlik, 2,885 kazanç aritmetiği ve doyma.
- Uyumlu: 6 davranış (§8.2; eski motor için §0.3 YAMA'ya göre geçerli; kayıp girmez). Tanımsız: A5-15.

### `pacer.rs` (engine-spec §13)
- `DEFAULT_BURST_CAP = 2·MTU` (eski motor §13 "≈ 1–2 MTU").
- `TokenBucket::new / refill` (alt-bayt artığı `refill_rem` taşınır), `try_send`, `set_rate`, `set_burst_cap` (≥ 1), `fill_to_cap`, `tokens`, `burst_cap`, `rate`, `ns_until_sendable` (refill'in tersi; `size > burst_cap` ya da hız 0 → `None`).
- Hat modeli (`LineModel`, `set_line`, `set_line_rate`, `line_charge`, `line_raise_backlog`, `line_add_backlog`, `has_line`, `line_model`, `line_is_open`, `line_ns_until_open`, `line_ns_until_drained`): kendi baytlarımızın (zarf dahil) hattaki kuyruğu `rate × 2 ms` payını aşınca kova datagram vermez.
- Çift kipi (`set_line_pairs`, `PAIR_PERIOD_NS = 500 ms` [CAL]): 500 ms'de bir iki datagram arka arkaya; ikincisi hat kuyruğunu beklemez.
- Uyumlu: 7 davranış (§13 kova, refill, boyut düşümü; profil istisnasının "pacing profil hattıyla sert sınırlı" maddesi hat modeliyle; karar 8 "bütün tel baytı hat hesabına" — `line_charge` zarfı ekler). Tanımsız: A5-08.

### `budget.rs` (engine-spec §8.5)
- `BudgetConfig::fixture()` (profile_cap 900‰, MIN_RATE 31 B/s, budget_ref 125 000 B/s; [CAL] kalibre değil).
- `budget_cap_permille(btlbw) = min(profile_cap, btlbw·1000/ref)`, `parity_final = min(fuzzy, budget_cap, profile_cap)`, `send_rate_final = clamp(fuzzy_rate, min_rate, cap)`.
- Uyumlu: 3 davranış (formüller). AMENDMENT-3 çağıranda (A5-11).

### `budget_kernel.rs` (DR-1b-v2 deneysel çekirdek)
- `PolicyKind` (LeftoverOnly / ProtectedShare / FloorCap / BoundedDebtFloor), `PolicyParams::defaults` (share 200‰, entitlement 4·1200, hardcap 300‰ / 8·1200, borç 2·1200, floor 2×srtt ∈ [50 ms, 10 s], tanecik 10 ms, srtt önseli 300 ms; hepsi [CAL] sim).
- `admit` (tipli kabul/ret), `floor_timing` (hız 0 → askıda), `t_floor_ns`, `step` (sınırlı kredi biriktirme, rezervasyon, hard-cap, B sınıfı borç izni, floor bileti sayısı, uyanma ipucu), `settle` (iki fazlı muhasebe; `assert!` iki yerde), `BudgetPolicyState::reset`.
- Testler: 13 000+ durumlu kapsayıcı değişmez taraması, T_floor, zarf özelliği, askı, tipli kabul, sınıf anlamları.
- Ürün varsayılanında etkisiz (özellik `experimental-fec-budget`); A5-09, A5-10.

### `line_guard.rs` (HAT-KORUMASI; geri bildirimi aç profilde ileri hat tahmincisi)
- **Sabitler** (hepsi elle seçilmiş ya da benzetim/ürün testinden; netem tabanından türetilmiş değil): `SENT_CAP 8192`, `POINT_CAP 8`, `GROWTH_MIN_DIV 20` (%5), `VERIFY_MIN_SPAN_NS 3 s`, `VERIFY_GROWTH_DIV 40`, `VERIFY_MIN_PERMILLE 900`, `STEP_START/MIN/MAX 500/62/1000‰`, `START_NUM/START_DEN 1/1`, `BASE_WINDOW 4`, `START_BURST_NS 500 ms`, `FIRST_WINDOW_MIN_WAIT_NS 5 ms`, `PROBE_QUEUE_NS 80 ms`, `OVERFLOW_EXCESS 3/4`, `OVERFLOW_WINDOW_ACKS 3`, `OVERFLOW_MIN_MTUS 8`, `OVERFLOW_FLOOR_MIN/CAP_MTUS 16/256`, `UNMEASURED_GAP_ACKS 2`, `GAP_PRIORITY_WINDOW_NS 2 s`; ayrıca satır içi: öncelikli `3R/8` eşiği, çift `%5` aşım ve `%95` serbest bırakma eşiği, yoklamada en az 8 (başlangıç yoklamasında 2) datagram.
- `new`, `conservative_start` (profil × 1/1), `on_sent / on_sent_ahead / on_sent_classed` (kümülatif tel baytı, yoklama süresince bayt sayımı, başlangıç kovası düşümü, kayıt; tavan dolunca en eski düşer), `on_unsequenced`, `note_ack_wire`.
- Başlangıç bütçesi: `start_budget_admits`, `start_depth` (`max(P × 0,5 s, 1 MTU)`; boşlukta 1 MTU), `start_tokens_at` (P/2 dolum, boşta birikmez), `end_start_budget` (uygulandıysa noktalar silinir), `settle_start_budget`, `first_ack_trusted`, `on_first_ack`, `in_provisional`, `start_floor_bps` (P/2).
- İlk pencere: `on_first_window` (alıcının §2.2 `arrival_rate` alanı + gönderici kaydı → çıkış anlarıyla `Δs`, `Δτ = bayt/alan`; `Sample` ise tavan `max(örnek, P/2)`, `Verified`, `NoInfo`; sıkışık pencere bilgi yok).
- ACK işleme `on_ack` / `on_ack_measured`: ölçüm aralığı (`interval_sample`: `g = rx_span − send_span − ser farkı`; eşik `max(MTU ser, send_span/20)`; artışta örnek = tel baytı / rx_span; değilse `overflow_estimate`; değilse `Flat`), boşluk kuralı (iki ölçümsüz ACK → `R/2` kovası, öncelikli ≥ 3R/8 ise kurulmaz, tek seferlik boşaltma), cephe tahmincisi (`τ = varış − ack_delay − ACK serileşmesi`; noktalara göre en büyük `bayt/Δτ`; yalnız aralık kipi yokken aşağı yönlü örnek), doğrulama → `begin_probe`, süreli yoklama hükmü (mutlak ve fark ölçüleri; başarı → tavan kalıcı, adım ×2; başarısızlık → tavan tabanda, ret seviyesi, fazla boşaltma; ayrışma → sonuçsuz), geçici evre (`Flat` → tavan P/2, `begin_start_probe`), kuyruk tahmini (`min(bayt tahmini, son 8 noktanın en büyük cephe beklemesi × hız)`).
- `overflow_estimate`: teslim kesri `f = rx/wire` taban kesrin 3/4'ünün altında ∧ son 3 ölçülü ACK'te artış ∧ gönderim tavanın ≥ %90'ı → örnek = çift alt sınırı (tavanın altındaysa) ya da `(rx / taban) / rx_span`.
- `flat_tail_rate`, `received_wire`, `interval_diag`, `overflow_samples`.
- Çift alt sınırı: `on_pair_rate` (yalnız aşağı yönlü örneksiz aralık ACK'inden sonra; alt sınır tavanı ve ret seviyesini %5 aşarsa **tavanı doğrudan ve kalıcı açar**, yoklamayı iptal eder; ≥ %95 profil → koruma kalkar), `pair_wire_bps`, `note_pair_lower_bound` (her ACK'te tüketilir), `pair_pacing` (aralık kipi ∧ bütçe yok ∧ yoklama yok ∧ tavan < profil), `pair_raises`.
- `tick` (yoklama süresi dolunca tavan tabana döner), `probe_to` (süre `PROBE_QUEUE_NS × taban/(yoklama − taban)`, ∈ [min datagram süresi, 3 s]).
- Okuyucular: `rate_bps`, `samples`, `pace_bps`, `raises`, `probe_failures`, `start_budget_active`, `gap_budget_active`, `gap_arms`, `gap_skips`, `take_gap_drain`, `start_exit`, `first_window`, `first_window_detail`.
- Testler (27): dürüst hatta örnek yok; çift alt sınırıyla toparlanma ve karşı-olgu; boşluk bütçesinin öncelikli yükte kurulmaması; aralık kipinde cephe örneği yok; taşma için kuyruk kanıtı ve aşırı kayıp; çift alt sınırıyla taşma büyüklüğü; alt sınırın tek ACK'te tüketilmesi; bitişik olmayan alarm aralığı (Web'e verilmiş kapsam, kilitli); iki cephe geriden aralık; dar hattın %1 içinde ölçülmesi; kuyruk tahmininin gerçek kuyruğu aşmaması; şişkin aralığın hızı yükseltmemesi; bayat/doymuş ACK; dürüst hatta doğrulama; dar hatta başarısız yoklamaların kuyruğu sınırlaması; uygulama sınırlı yoklamanın sonuçsuzluğu; ret seviyesinin çift alt sınırını sınırlaması; dönüş gecikmesi basamağı; hızlanan hatta toparlanma; başlangıç bütçesi (patlama, saat, dar profil); ilk pencerenin beş biçimi.
- Uyumlu: 9 davranış — karar 2 (`ack_delay` anlamı korunur: yalnız çıkarılır, dönüş kuyruğu eklenmez), 2026-11-16 profil istisnası "ölçüm tavanı aşamaz" (her örnek `min(tavan, profil)`), karar 8 (tel baytı zarf dahil, motor dışı datagram `on_unsequenced` ile), SR-AUDIT-001 (`SENT_CAP`, `POINT_CAP`, `BASE_WINDOW`, iki cephe), ADR-17 (tamsayı, RNG yok), karar 6 (kova boyu varsayılmaz; yalnız FIFO seri hat), K2′ önkoşulu "bayat çift değeri göndericide tüketilir" (`note_pair_lower_bound`), taşmada `bound ≥ tavan → örnek yok` bastırması, ret seviyesi altındaki çiftin tavanı açmaması.

## Bulgular

| fid | tür | ağırlık | konum | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| A5-01 | UYUŞMAZ | yüksek | `line_guard.rs:1381-1407` (`on_pair_rate`); çağrı `sender.rs` | Tek bir çift alt sınırı ret seviyesini %5 aşınca tavan **doğrudan ve kalıcı** açılır (`pace_bps = up`, `probe = None`, `raises += 1`); süren yoklama hükümsüz iptal edilir. Kendi belgesi (1367-1369) bilinen sahte artışı yazar: alıcı boy sayımı +100 B'de alt sınır 6 863, gerçek 6 250, kuyruk 0,82–1,34 s. | #175 7171705736 karar 5: «Paket çifti yoklama hedefini seçsin. Kovalı hatta kalıcı tavan artışı için süre ve bayt bütçesi sınırlı bir yoklamadan doğrulama isteyin … Mevcut doğrudan tavan açma yolu yalnız karşılaştırma için kullanılabilir … Hat 6.250 B/s'de kalırsa sahte artış olmamalı.» 7172579876: «Bu onay ürün varsayılanını değiştirme veya adaya alma yetkisi vermez.» | kilitli (#262 dalı parkta, 7186813951; K2′ deneyi varsayılan kapalı). Ürün yolu sahip kararıyla değişene dek kod yok. |
| A5-02 | UYUŞMAZ | yüksek | `line_guard.rs:1248-1305` (`overflow_estimate`), sabitler 452-467; tüketim 1228-1233, 1757-1778 | Teslim kesri `f = alınan/gönderilen tel baytı`, rastgele kayıp tabanının 3/4'ünün altına düşünce (artış örneği son 3 ACK'te ve gönderim ≥ %90 iken) gönderim tavanı düşürülür. Çift alt sınırı yoksa büyüklük de kayıptan türer: `(rx / taban kesri) / rx_span`. Kayıp oranı hız girdisidir. | CLAUDE.md §4 INV-RATE-1 (2026-11-11): «loss PACING HIZINI düşürmez (pace = BBR pacing_gain × BtlBw; BtlBw loss'tan türetilmez)»; 2026-11-16 istisnası: «Loss yine ölçülür, FEC/ARQ'yu ve BBR durumunu besler». engine-spec §7.4 INV-ENGINE-BW: «bandwidth DAİMA delivered-rate'ten (BtlBw); loss'tan ASLA türetilmez.» | karar-gerekir: ya kuyruk kanıtlı taşma tıkanıklık sayılıp INV-RATE-1'e tarihli istisna yazılır, ya kayıp kesri hız yolundan çıkarılır (yalnız çift alt sınırı büyüklüğü, tetik kuyruk artışı). |
| A5-03 | UYUŞMAZ | yüksek | `line_guard.rs:519-602` (`rate_bps`, `pace_bps`), 1146-1160, 1757-1781, 1855-1901; bağlantı `sender.rs:2761-2790` (`cc_window` = `max(window_without_recovery, cap × (ufuk + RTT))`), `sender.rs:3160-3180` | Geri bildirimi aç profilde pacing hızı ve uçuş tabanı LineGuard'ın tavanıdır (profil hattı ya da LineGuard'ın ölçtüğü daha yavaş hat); BBR'ın `pacing_gain × BtlBw` hızı okunmaz. LineGuard kendi Startup'ını (başlangıç bütçesi), bant yoklamasını (`begin_probe`, adım 6,2–100 %) ve aşağı yönlü tepkisini yürütür; BBR aynı anda cwnd, recovery ve kendi ProbeBW/ProbeRTT döngüsünü hesaplar. Aynı değişkeni (hız, uçuş) iki denetleyici yönetir. | INV-RATE-1 2026-11-11: «pace = BBR pacing_gain × BtlBw». #175 6920942098 (2026-11-16): «Pacing profil hattıyla sert sınırlı; ölçüm tavanı aşamaz» — tavan, yerine geçme değil; «Taban: hat hızı × (yol RTT'si + en büyük bütçeli ACK aralığı + o ACK'in serileşmesi)» — hat = profil hattı; ölçülen hatta indirme yazılı değil. spec'te bulunamadı (aranan: `LineGuard`, `line_guard`, `HAT-KORUMASI`, `pair_rate`, "hat koruması" — `specs/` altında sıfır eşleşme). | karar-gerekir: görev dağılımı (tek hız tahmincisi mi, LineGuard ölçüsü BBR'a hat girdisi mi, LineGuard tek denetleyici mi) sahip kararı ister; kod tek başına spec'e çekilemez. |
| A5-04 | TANIMSIZ | orta | `line_guard.rs:388-477`, 1382-1398, 1501-1502, 1860, 1868 | LineGuard kural setinin tamamı (başlangıç bütçesi, ilk pencere, ikinci ölçüm, boşluk bütçesi, taşma, yoklama adımları, çift kipi) ve 20'den fazla sabiti ([CAL] işaretli olanlar dahil) benzetim/ürün testinden seçilmiştir; hiçbiri netem tabanından türetilmemiştir. `START_NUM/START_DEN = 1/1` "temkinli başlangıç" adıyla etkisizdir; belgesi (79-96) 17/20'nin «bedeliyle sahibe gider» dediği seçeneği açık bırakır, karar kaydı yok. | engine-spec başlık «Sabit disiplini (NORMATİF): Tunable sabitler `[CAL]` = netem baseline'dan (ADR-18) kalibre edilir, UYDURULMAZ». Kuralların otoritesi: spec'te bulunamadı (aranan yukarıdaki gibi); yalnız #175 kararları 5/6/8 ve K2′ kısmi çerçeve verir. | karar-gerekir: kural seti ve sabitler için tarihli sahip kaydı (ya da spec YAMA'sı); `START_NUM/DEN` için seçim kaydı. |
| A5-05 | TANIMSIZ | orta | `line_guard.rs:1460-1526` | `interval_sample` kayıtlar budanmadan önce hesaplanır ve durumu değiştirir (`interval_mode`, `growth_recent`, `floor_rx/sent`, `interval_diag.1` örnek sayacı, boşluk bütçesinin kurulması); ardından `pending_wire` yoksa, `saturated` ise, cephe ilerlemediyse, seq kayıtta yoksa ya da `τ` taşarsa erken `None` döner: aralığın aşağı yönlü örneği **sessizce atılır** (aralık ölçüsü `ack_delay`'den ve cepheden bağımsız olduğu hâlde), gözlem sayacı uygulanmamış örneği sayar; boşluk bütçesi kurulur ama «kurulurken tek seferlik boşaltma» (1537-1544) yazılmaz. | Modülün kendi kuralı (299-310: «kurulurken tek seferlik boşaltma: cephenin kuyruk beklemesi kadar süre hat modeline yazılır»; 252-256: aralık örneği dönüş gecikmesinden bağımsız). Spec karşılığı: bulunamadı. #262 için 7175487850 «Ölçüm taşımayan veya cephesi ilerlemeyen ACK de sayılır» — yalnız çift yoklaması kapsamı. | karar-gerekir (ürün davranışı değişir; LineGuard kuralları sahip alanında). Kanıt: dar testte cephesi ilerlemeyen ya da doymuş `ack_delay`'li aralık ACK'i. |
| A5-06 | UYUŞMAZ | orta | `bw.rs:182`, 255, 264; `rtt.rs:27`, 32, 38 | `BtlBwFilter.samples` ve `MinFilter.samples` yalnız zamanla budanan `Vec`; adet tavanı yok. Pencere `set_window` / `set_rtprop_window` ile üstten sınırsız büyüyebilir (`max(10 s, K × ack_epoch)`); yüksek ACK hızında on binlerce kayıt, `btlbw()` ve `min()` her çağrıda O(n). | CLAUDE.md §9 «bounded buffer (SR-AUDIT-001: unbounded alloc yok, bounds-before-alloc)»; engine-spec §14 «Tüm pool'lar sabit-üst-sınır (unbounded alloc YOK)». | kod-duzeltmesi: tekdüze (monotonic) deque ile pencereli max/min + sabit adet tavanı; KEEP-LAST korunur, `btlbw()`/`rtprop()` değeri bit-aynı kalır (yalnız `samples_snapshot` gözlemi değişir). |
| A5-07 | EKSİK | orta | `bw.rs:228-271` (yalnız dış `valid` bayrağı) | §7.3'ün `estimator_confidence` EWMA'sı ve `ack_compression_score > ACK_COMP_THRESH` geçersizlik kuralı kodda yok (crate genelinde arandı: eşleşme yok). | engine-spec §7.3 (NORMATİF): «estimator_confidence (EWMA, 0..1) [CAL] … düşük confidence → SRC muhafazakâr pace»; «ack_compression_score > ACK_COMP_THRESH : … GEÇERSİZ». §0.3 YAMA 2026-11-11: «BtlBw = BBR §7.1 … (bu belgenin §7'si, değişmedi)». | karar-gerekir ([CAL] değerleri ve muhafazakâr pace kuralı tanımsız; port motorunda hız yoluna girip girmeyeceği sahip kararı). |
| A5-08 | TANIMSIZ | orta | `pacer.rs:32-44`, 112-127, 246-266 | Çift kipi: 500 ms'de bir iki datagram arka arkaya çıkar, ikincisi hat kuyruğunu beklemez (darboğazda bir datagram fazla kuyruk); `PAIR_PERIOD_NS` [CAL] türetilmemiş (ACK aralığı ≥ 1 s'den akıl yürütme). | engine-spec §13: «inter-packet gap = pkt.size / pace_rate (düzgün pacing …)»; port YAMA'sı burst tavanını tanımlar, çift kipini değil. Belgedeki #175 7068773849 okunmadı: sahip onayı DOĞRULANAMADI. | karar-gerekir. |
| A5-09 | TANIMSIZ | düşük | `budget_kernel.rs:1-6`, 389-392, 402 | Modül belgesi «hiçbir üretim yolu çağırmaz» der; oysa `sender.rs:916-978`, `7654`, `7860-7900` `experimental-fec-budget` özelliği altında bağlar ve `udp_runner.rs:7,114` `PolicyParams` kullanır. `settle` iki `assert!` ile çalışma zamanında panikleyebilir (CI `--all-features` derler). | CLAUDE.md §6 «reachable todo!() YASAK (runtime panik)», §9 «açık hata tipleri (thiserror)». | kod-duzeltmesi: `settle` → `Result<u64, BudgetKernelError>`; belge cümlesi gerçek bağlantıyla düzeltilir. |
| A5-10 | ÖLÜ | düşük | `budget_kernel.rs` (tümü); `Cargo.toml` `experimental-fec-budget = []` (varsayılan değil) | Ürün derlemesinde etkisiz deneysel kol (dört politika, kabul, floor bileti). | CLAUDE.md §6 «Deprecated içerik → appendix'e taşı + "MUST NOT IMPLEMENT". Sessizce SİLME.» | karar-gerekir (kol sürecek mi, ek bölüme mi alınacak). |
| A5-11 | UYUŞMAZ | düşük | `budget.rs:43-47` | `parity_final` `budget_cap == 0` iken pariteyi sıfırlar; AMENDMENT-3 istisnası yalnız tek çağıranda (`sender.rs:5062-5070`) uygulanır, işlev ve belgesi istisnayı içermez. | engine-spec §8.5 [AMENDMENT-3]: «budget_cap YALNIZ KÜÇÜLTÜR, SIFIRLAMAZ: budget_cap == 0 ∧ fuzzy_parity > 0 ise clamp UYGULANMAZ». | kod-duzeltmesi: istisna işleve alınır (çağıranın sayacı korunur) ya da işlev belgesinde açıkça "çağıran uygular" yazılır. |
| A5-12 | ÖLÜ | düşük | `cc/bbr.rs:780-786` | `probe_rtt_after_ten_seconds_without_min_rtt_refresh` yalnız `probe_rtt_last_started_at_ns.is_some()` arar; U8 (524-530) bu alanı ilk min_rtt örneğinde kurduğu için test ProbeRTT'ye hiç girilmese de geçer (boş doğrulama). | Test amacı (kendi adı) ve engine-spec §0.3 YAMA «ProbeRTT 200 ms / 10 s». | kod-duzeltmesi: sürücü turlarında `mode() == ProbeRtt` görüldüğünü ya da alanın ilk değerinden büyüdüğünü sına. |
| A5-13 | ÖLÜ | düşük | `line_guard.rs:2567-2579` | Test yardımcısı `send_at` hiç çağrılmıyor (`#[allow(dead_code)]`). | yok | kod-duzeltmesi: sil ya da kullan. |
| A5-14 | TANIMSIZ | düşük | `cc/bbr.rs:7-34`, 57-61 | Başlıktaki uyarlama listesi 1, 2, 3, 4, 9, 10, 5 sırasıyla; 6 ve 8 yalnız satır içinde, 7 (pacer hızı) hiç yok. `PACKET_CONSERVATION_IN_STARTUP` «(deneniyor)» etiketli ama ürün varsayılanında sabit `true`, anahtarı yok. | KARAR-KAYDI §8 «U9 paket koruması, PR #179, azaltır»; CC-PORT-ESLEME U1–U9 listesi. | kod-duzeltmesi (yalnız belge): listeyi U1–U17 eşlemesiyle hizala, "deneniyor" etiketini kaldır. |
| A5-15 | TANIMSIZ | düşük | `rate.rs:179-252` | E1 gösterim bayrakları ve 250‰ eşikleri spec dışı tasarım belgesine (`docs/design/E1-HIZ.md`, «CC §4.1») dayanır; yalnız eski motor ve gösterim/teşhis kolları açar. | spec'te bulunamadı (aranan: `E1Flags`, `e1_drainrt_gate`, "DrainRtprop kapısı" — engine-spec §8.3 yalnız `RTprop_stamp > RTPROP_WINDOW` koşulunu yazar). | karar-gerekir (gösterim kolu sürecek mi). |
| A5-16 | TANIMSIZ | düşük | `line_guard.rs:1548-1564`, 1643-1649 | Genel adım yoklamasının hükmü yalnız cephesi yoklama bitiminden sonra gönderilmiş ACK'te verilir; arada gelen ACK'ler sayılmaz, hüküm için ACK sınırı yoktur (hüküm 4. ve sonraki ACK'e kayabilir). | Çift yoklaması için sahip 3. ACK sınırı koydu (7174289733 madde 1, 7175487850 madde 1); genel adım yoklaması için kural: bulunamadı. | karar-gerekir. |
| A5-17 | DOĞRULANAMADI | orta | `line_guard.rs:1287-1296`, 1424-1426 | K2′ önkoşulu: «Eski çift ölçüsü sızıyorsa … aşağı yönlü taşma hesabını bozmasını engelleyin». Gönderici her ACK'in alt sınırını tüketir ve `bound ≥ tavan → örnek yok` bastırması vardır; alıcıda bayat değerin sızıp sızmadığı bu dosyalardan doğrulanamaz. | #175 7172579876 önkoşul. | kilitli (#256 Bulut-B, alıcı `receiver.rs` Web'de, 7186813951). |

## Kod ile otorite arasındaki ana gözlem (hız / uçuş / kayıp tepkisi görev dağılımı)

| görev | cc-port (BBR) | LineGuard | çakışma |
|---|---|---|---|
| hız | `pacing_rate`, `measured_pacing_target` hesaplanır; geri bildirimi aç profilde okunmaz | `pace_bps` (profil ya da ölçülen hat) pacing'in kendisi | A5-03 |
| uçuş | `window_without_recovery` | `cap × (ufuk + RTT)` tabanı, `max` ile birleşir | A5-03 |
| kayıp tepkisi | `recovery_window` (profil yolunda uygulanmaz), Startup çıkışı | `overflow_estimate` hız düşürür | A5-02 |
| bant yoklaması | ProbeBW 1,25/0,75, ProbeRTT | `begin_probe`, `begin_start_probe`, `on_pair_rate` | A5-01, A5-03 |
| başlangıç | Startup 2,885 | başlangıç bütçesi `max(P × 0,5 s, P/2 × t)` | A5-03, A5-04 |

## Okunan otorite bölümleri

- CLAUDE.md §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §5 ADR, §6, §8, §9, YAMA KAYDI.
- #175 sahip kararları: 7171705736 (dokuz karar; 2, 5, 6, 8 bu dosyalarla ilgili), 7172579876 (K2′), 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951; ek olarak 6920942098 (2026-11-16 profil kararı, YAMA KAYDI'nın kaynağı).
- `specs/adaptive-transport-engine-spec.md`: başlık YAMA kayıtları ve sabit disiplini (1–29), §0.2–§0.3 (44–104), §6 (559–602), §7.1–§7.4 (606–661), §8.1–§8.5 (665–728), §13 (980–994), §14 (998–1008).
- `specs/transport-layer-tdd.md`: §5.1–§5.6 (596–676), §6 (679–695), §14 invaryant tablosu INV-RATE-1 satırı (992).
- `specs/tech-stack-adr.md`: ADR-06 (131–142), ADR-17 (261–272), ADR-18 (274–290).
- `docs/decisions/KARAR-KAYDI-2026-11.md` (tümü; §8 D-CC-1).
- `docs/design/CC-PORT-ESLEME.md` (tümü; normatif değil, uyarlama beyanı U1–U17).
- `specs/` altında arandı, bulunamadı: `LineGuard`, `line_guard`, `HAT-KORUMASI`, `pair_rate`, `estimator_confidence` uygulaması, `E1Flags`, çift kipi.
