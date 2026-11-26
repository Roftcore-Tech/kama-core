# C1-runtime-1 — `crates/client-core/src/runtime.rs` satır 1–3300 denetimi

Taban: `125b9afa` (ürün hattı; aday `1a4f9efc` içinde). Salt okuma: kaynak, cargo, git yazımı yok. Satır numaraları `125b9afa`'dandır.

## Kapsam

| Dosya | Satır aralığı | Okunan satır |
|---|---|---|
| `crates/client-core/src/runtime.rs` | 1–3300 (dosya 8 717 satır; 3301 sonrası C1-runtime-2'nin) | 3 300 |

Kapsam dışı ama karşılaştırma için bakılan yerler (yalnız ilgili satırlar): `crates/client-core/src/wire.rs` sabitleri (15–377), `crates/adaptive-transport/src/ack.rs` 58–75, `crates/adaptive-transport/src/receiver.rs` 99–122, `crates/kama-node/src/lib.rs` 326–345, `crates/bench/src/kama_product.rs` 150–165, `crates/client-core/tests/cc_port_runtime.rs` 100–116, `runtime.rs` 3463/3470 (yalnız `acked_tx.try_send` satırları, grep).

## Davranış envanteri

### A. Modül başlığı ve sabitler (1–86, 264–265, 402–476, 517–567, 676–679)

- Modül belgesi (1–19): tek görev `select!`, PACE 1 ms, RTPROP 200 ms, ACK 20 ms; motor saf u64-ns; kripto ayrı düzlem (`PacketProtection`); gönderim hatası = kanal kaybı (INV-RATE-1); sınırlar: offer kanalı bounded, unit-store bounded, peer tablosu cap'li fail-closed, teslim kanalı bounded.
- `PACE_TICK` 1 ms (73), `RTPROP_TICK` 200 ms (75), `ACK_TICK` 20 ms (77; engine-spec §2.1/§12 `ACK_TIMER_T` [CAL], §0.3 (8) `max_ack_delay 25 ms` altında), `PATIENCE_TICK` 1 s (79): türetilmemiş, bench'ten devralınmış.
- `CLIENT_RECV_BUF` 2 048 B / `SERVER_RECV_BUF` 65 536 B (81–82): istemci yalnız dönüş yönünü (ACK/RECOVERED/ECHO/PARAM_ACK) alır; en büyük V2 ACK 15+4·16 = 79 B.
- `PN_SEQ_WINDOW` = 2¹⁶ (86): ACK_V2 `largest_seq` u16 sarmasından türetilmiş (engine-spec §2 seq-wrap).
- `TS_CAP` 4 096 (265): zaman serisi tavanı (SR-AUDIT-001).
- `FEEDBACK_STARVED_ACK_INTERVAL_NS` 1 s (405): CLAUDE.md INV-RATE-1 profil istisnasındaki "bütçeli ACK aralığı ≥ 1 s" ile aynı.
- PTT/şerit sabitleri: `PTT_FORWARD_COPIES` 7 (422), `PTT_FORWARD_COPIES_MIN` 6 (432), `IDLE_LANE_TRANSMISSIONS` 5 (437), `IDLE_LANE_COPY_SPACING_NS` 40 ms (441, türetilmiş), `LANE_IDLE_NS` 200 ms (447), `FILE_LANE_INDEX` 3 (450), `PTT_COPY_SPACING_NS` = 200 ms / 7 (454, türetilmiş), `PTT_DEADLINE_NS` 1 s (460), `PTT_RELIABLE_COPIES` 2 (465), `PTT_COPY_PENDING_CAP` 16 (469), `PTT_RELIABLE_COPY_SPACING_NS` 20 ms (474), `PTT_LANE_INDEX` 0 (476).
- `MAX_LINK_INITIAL_WINDOW_BYTES` 1 MiB (518), `LINK_UDP_IP_OVERHEAD` 28 B (558; IPv4+UDP), `MAX_ECHO_SPACING_NS` 30 s (679).

### B. `RuntimeConfig` ve ürün profili (91–390, 736–741)

- `RuntimeConfig` (93–262): 31 alan. Ürün değerleri `KamaV1Profile::runtime_config()` (332–389): `engine CcPort` (338), `max_inflight_units 4096`, `offer_queue 1024`, `delivery_queue 1024`, `max_unit_bytes 1013` (= 1024 − 11), `max_message_bytes None`, `max_peers 64`, `quota_r 4`, `link_rates None`, `drain_cap 3 s`, `idle_hold 105 s`, `initial_window_bytes = DEFAULT_BURST_CAP` (2·MTU), `e2a_flags` kapalı, `bootstrap_rtt false`, `bootstrap_fast_ladder false`, `fec Some{0x05, manual_r None}` (Controller), `fusion_policy None`, `loss_confirm_horizon CadenceLegacy`, `fec_loss_risk bounded_v1()`, `structure_gate_level_permille Some(250)`, `live_diag false`, `diag_emit_sink None`, `diag_return_budget_permille None`, `ack_rereport false`, `ack_format V2`, `lane_ids false`, `e1_flags` kapalı, `ptt_coalesce false`.
- `Default for RuntimeConfig` (736–741): ürün profiline devreder.
- `KamaV1Profile` sabitleri (315–327): `FEC_PROFILE_ID 0x05`, `MAX_UNIT_BYTES 1013`, `INITIAL_WINDOW_BYTES`, `STRUCTURE_GATE_LEVEL_PERMILLE 250`.
- `TRANSPORT_V1_PROFILE_ID "kama-v1-full"` (303), `TRANSPORT_V1_ARCHITECTURE "transport-v1"` (306): manifest kimlikleri; belgesi EVOLUTION §9.5'e atıf yapar.
- `FecRuntimeConfig {profile_id, manual_r}` (268–277): `manual_r Some` = statik-r, `None` = Controller.
- `LinkRates {uplink_bps, downlink_bps}` (394–400): B/s, UDP gövdesi + UDP/IP; doğrulanmaz.
- `AckFormat` (682–701): `V2` (varsayılan) / `Bitmap`; `parse("" | "v2") → V2`, `"bitmap"`, başka → `None`.
- `TransportEngine` (704–734): `#[default] CcPort`; `parse("" | "legacy") → Legacy`, `"cc-port" | "cc_port" → CcPort`, başka → `None`; `label_suffix`.

### C. Profil türetmeleri (478–567)

- `link_feedback_starved(cfg)` (482–488): `CcPort ∧ link_rates ∧ budgeted_ack_interval_ns(downlink) ≥ 1 s`. `downlink_bps = 0` iken aralık `u64::MAX` → `true`.
- `fec_feedback_starved_permille(p)` (495–498): `ceil(r_code_max·1000/k_shape)`; 0x05 → 500‰ (YAMA KAYDI 2026-11-16 "profilin en yüksek FEC oranı").
- `with_fec_prior` (503–515): anlık görüntü oranını en az `prior` yapar, `no_rule=false`.
- `port_base_initial_window` (522–525): `max(initial_window_bytes, 4·MTU)` (D-CC-1 "ilk uçuş 4·MTU").
- `link_profile_window_bytes` (541–554): yalnız `CcPort ∧ link_rates`; `uplink·t_ack`, tavan 1 MiB; tabanı aşarsa `Some`. `link_feedback_starved`'dan bağımsız (aç olmayan profilde de çalışır).
- `link_pacing_cap_bps(uplink)` (563–567): `uplink·MTU/(MTU+28)`, en az 1.

### D. `UplinkShare` — paylaşılan yukarı yön hat modeli (569–674)

- `for_config` (599–617): `CcPort ∧ link_rates ∧ uplink>0` → `Arc<UplinkLine{origin=Instant::now, rate, busy_until_ns, narrow=Sender::line_is_narrow}>`.
- `charge(udp_bytes)` (624–635): `(udp+28)·1e9/rate` ns serileşmeyi CAS ile `busy_until = max(busy, now)+ser`'e ekler.
- `ns_until_clear` (638–643), `backlog_bytes` (659–664): kalan serileşme / bayt.
- `control_should_yield` (654–656): yalnız dar hatta ve hat doluyken `true` (ivedi olmayan denetim — bütçeli ACK, RECOVERED — veriye yol verir). Engine-spec §13 öncelik sırası "yeni DATA > ACK-only" ile uyumlu; karar 7 (sınırlı ACK önceliği) deneyi ayrı ve #263 parkta.
- `Debug` (667–674).

### E. Doğrulama ve FEC tavanı (743–883)

- `validate_fec` (746–778): `max_unit_bytes ≠ 0`; `max_message_bytes ≥ max_unit_bytes`; `fec Some` ise profil bilinir, `max_unit_bytes + 11 ≤ E`, `manual_r ≤ r_code_max` (`r = 0` geçerli kontrol kolu). `offer_queue`, `max_inflight_units`, `delivery_queue`, `max_peers`, `link_rates`, `max_message_bytes` üst sınırı DOĞRULANMAZ.
- `fec_payload_r_max` (802–815): zaman kapısı açıksa `r_code_max`, değilse `min(payload_wire/parity_wire, r_code_max)`.
- `fec_parity_path_bw` (829–836): `min(BtlBw, son teslim hızı örneği)`; BtlBw yoksa `None`.
- `fec_parity_time_gate_open` (841–847): `parity_wire·1e9/bw ≤ 2 s` (`PARITY_SER_LIMIT_NS`, türetilmemiş).
- `record_fec_cap_decision` (851–857): salt sayaç.
- `fec_chunk_capacity` (866–868): `E − 11`.
- `param_fields_for` (871–883): `protocol_version 1, fec_version 1, code_suite_id, profile_id, k_shape, r_code_max, e, partial_policy 0, packet_scheduling_policy 0` (noise-wire PARAM canonical alan listesiyle aynı sıra).

### F. Hata, egress, sayaç tipleri (885–1313)

- `RuntimeError` (887–903): `QueueFull`, `UnitTooLarge{got,cap}`, `Closed`, `Io`, `FecConfig(&'static str)` — thiserror.
- `EgressPolicy` (916–926): `admit(len, now_ns) → bool` (ret = kanal kaybı, geciktirme yok), `arm(now_ns)` varsayılan no-op.
- `TxClass` (930–941): Data/Retx/Dup/Fec/Ctrl yalnız sayaç ayrımı.
- `FateWindowDiag` (951–1071), `ClientStats` (1075–1219), `ServerStats` (1223–1313): salt gözlem sayaçları. `ClientStats::tx_ctrl_bytes` belgesi "(PARAM_SET)" der, `TxClass::Ctrl` ise PARAM_SET/PARAM_ACK/RECOVERED/ACK'i kapsar. `ServerStats::delivery_dropped` belgeye göre yapısal 0 (geriye uyum alanı).

### G. Canlı teşhis aynası (1315–1926)

- `ClientLiveDiag` (1328–1401), `SenderRecoveryDiag` (1419–1444), `StuckUnitDiag` (1448–1465), `MotorAckView` (1577–1608), `AckFrameDiag` (1613–1642), `AckFramePre` (1648–1654), `WireEvidence` (1664–1701, `plus` doygun toplama), `AckFrameCounts` (1705–1735), `StateChangeDiag` (1742–1760), `SenderControlDiag` (1767–1823): salt okuma yapıları; yalnız `live_diag` açıkken yazılır.
- `LINE_GUARD_LOG_LEN` (1405, motor halkasıyla aynı), `RECOVERY_DIAG_PERIOD` 200 ms (1411, `[KALİBRASYON]`, normatif değil), `ACK_LOG_LEN` 32, `STATE_LOG_LEN` 32 (1521–1525), `PTT_WIRE_UDP_IP_BYTES` 28 (1684).
- `capture_recovery` (1469–1515): store'un en küçük kimlikli birimini, PN geçmişini (`pn_seq` ters doğrusal tarama), motor kayıtlarını okur.
- `DiagRing<T,N>` (1530–1570): sabit halka, ayırma yok; `N=0` güvenli.
- `ClientLiveDiag::refresh` (1828–1888), `mirror_line_guard_trace` (1892–1905; motor halkası yeniden kurulunca sıra sıfırlanır), `add_counters` (1911–1925).
- `saturating_us` (1657–1659).

### H. Teslim, çerçeve kurucuları (1928–2021)

- `Delivery {peer, unit_id, payload, ctx}` (1930–1937).
- `unit_frame` (1945–1958): store'da gövde yoksa `None` (karar atlanır); `seq & 0xFFFF` u16 ara alan; frag açıkken `encode_data_frag`, kapalıyken `encode_data` (HDR_DATA 11 B iç başlık).
- `PTT_LEAN_FALLBACK_ACKS` 2 (1964).
- `set_ptt_legacy_copies` (1969–1987): `ForwardCopyRule{below_id = şerit 1'in ilk kimliği, priority_below_id = FILE şeridinin ilk kimliği, copies 6, spacing 200/7 ms}` + `ForwardCopyLineScale{declared = uplink, min 5}`.
- `ptt_forward_copy_rule(copies, spacing)` (1989–2004).
- `repair_symbol` (2008–2017): repair-store'da yoksa `None`.
- `elapsed_ns` (2019–2021).

### I. Bağlam-önce dağıtım (2023–2510)

- Tasarım notu (2023–2071): bağlam (`Opened::ctx`) yükten önce dağıtıcıyı seçer; çerçeve-tipi→bağlam tablosu yok; beklenen tutamaç TOFU değil; yuva `(adres, SessionHandle)`; INV-ACK-DELIVER-1 (NotReady → ACK'siz düşüş).
- `IMPLICIT_SESSION = SessionHandle(0)` (2075).
- `CtxDispatchCounters` / `CtxDispatchStats` (2080–2133): üç atomik sayaç (Relaxed).
- `Readiness {Ready, NotReady}` (2137–2144).
- `ExpectedSessions` (2158–2414): `Arc<Mutex<BTreeMap<Handle,Readiness>>>` + `Arc<Mutex<AdmitWindow>>`. `only`, `insert_ready`, `insert_pending` (Ready'yi geri almaz), `replace` (pencere durumunu emekli oturumlardan budar, sonra kümeyi değiştirir), `set_admit_window(width)` (tüm şeritlere aynı, taban = sıra başı), `set_lane_admit_windows(widths)` (taban = max(sıra başı, kabul sınırı)), `note_admitted` (kabul sınırını dedup penceresi boyunca ilerletir), `mark_ptt_deadline` / `ptt_deadline`, `mark_ptt_data_framed` (oturum başına ≤ 64, doluysa en eski düşer) / `take_ptt_data_framed`, `advance_head` (yalnız ileri), `admit_limits` / `admit_limit`, `readiness`; kilitler zehirlenmede panik yapmaz.
- `AdmitWindow` (2175–2199), `PTT_DATA_FRAMED_CAP` 64 (2204), `LaneAdmitLimits` (2207).
- `session_admit_limit` (2418–2426), `beyond_lane_limits` (2430–2432), `beyond_admit_window` (2436–2438).
- `HubDispatch::on_hub_frame` (2446–2457): yük okunmaz, yalnız `hub_frames_dropped++`.
- `CtxRoute` (2462–2473), `route_client` (2477–2483: HubChannel → Hub; eşit Session → Session; başka → Mismatch), `route_server` (2494–2510: HubChannel → Hub; yuva bağı farklı → Mismatch; Ready → Session; NotReady → NotReady; beklenmiyor → Mismatch).

### J. İstemci ucu API'si (2512–3039)

- `PathEstimate` (2519–2539): btlbw, rtprop, şerit teyit cephesi, gönderim kuyruğu boşalması, eşin PTT birleşik yeteneği — görev yazar, tutamaç okur.
- `ClientHandle` (2542–2557), `ClientSender` (2562–2571).
- `ClientSender::send` / `send_with_deadline` / `send_in_lane` (2580–2635): `len > max_message_bytes` → `UnitTooLarge`; `admit_counted` ile sayaç önce artar; `try_send((payload, deadline, lane & 0b11))`; Full → `QueueFull`, Closed → `Closed`.
- `admit_counted` (2650–2656), `release_outstanding` (2661–2665): doygun.
- `ClientEndpoint::connect` → `connect_with_policy` → `connect_session(IMPLICIT_SESSION)` → `connect_session_counted` → `connect_session_shared` (2672–2807): `validate_fec`; IPv4/IPv6'ya göre `0.0.0.0:0` / `[::]:0` bind + connect; kanallar `offer_queue`, `max(offer_queue, max_inflight_units)` (acked), `t0` kapasite 1; `recovery` watch `true`; canlı ayna yalnız `live_diag`; `tokio::spawn(client_task)`.
- `ClientHandle` yöntemleri (2810–2993): `send*`, `sender`, `mark_t0` (görev işleyene dek bekler), `acked`, `try_acked`, `outstanding`, `lane_confirm_frontier(lane & 0b11)`, `peer_ptt_coalesce` (0/1/2 → None/Some), `path_estimate`, `send_queue_drained`, `set_recovery` (yalnız test), `ctx_dispatch_stats`, `live_diag`, `live_diag_enabled`, `shutdown` (`task.await.unwrap_or_default()`).
- `ClientTaskLinks` (2998–3019; üstündeki `#[allow(clippy::too_many_arguments)]` bir struct'a uygulanmış).
- `ConfirmVia::of` (3026–3039): `RecoveredControl` → Recovered, diğer her olay → Ack.

### K. `client_task` kuruluşu (3041–3300)

- 3063–3075: `HubDispatch`; `start = Instant::now()`; motor `Legacy → ClientSession::new()`, `CcPort → ClientSession::with_cc_port()`.
- 3077–3122: yapı-gate politikası, bootstrap-RTT (açıksa hızlı/varsayılan merdiven, `enabled=true`), `set_cs_probe_lane(bootstrap_rtt)`, confirm ufku, `fec_loss_risk`, gate seviyesi, `fusion_policy` (varsa), E2a bayrakları YALNIZ Legacy'de, `set_initial_window`.
- 3127–3160 (yalnız CcPort): `iw = port_base_initial_window`; `link_profile_window_bytes` varsa `iw = link_iw` + `set_cc_profile_pacing_cap(link_pacing_cap)`; `link_rates` varsa `set_cc_line(LineModel{uplink, 28})`; `set_cc_initial_window(iw)`.
- 3161–3168: `diag_emit_sink` → `set_diag_emit_log(true)`; `live_diag` → `set_diag_line_guard_trace(true)`.
- 3176–3192: geri bildirimi aç profilde `set_cc_feedback_starved(FeedbackStarvedRule{pacing_cap = link_pacing_cap(uplink), feedback_ns = feedback_horizon_ns(downlink), line, return_line})`.
- 3199–3215: `ptt_lean_offer = lane_ids ∧ starved ∧ Bitmap ∧ frag kapalı` → kopyasız kural (`copies 0`); değilse `lane_ids ∧ starved` → eski kopyalı kural; `non_lean_acks`, `ptt_copy_fallback`.
- 3217: `set_e1_flags`.
- 3222–3241: `fec = by_id(profile)`; CcPort ∧ link_rates iken parite sembolü uplink'te 2 s'de serileşmiyorsa `fec = None` (FEC sessizce kurulmaz); `fec_manual_r`; `fec_starved_prior` (starved ise 500‰); `fec_prior_for(lane)` = şerit kapalıyken hepsine, açıkken şerit 0 hariç.
- 3242–3252: FEC varsa `set_fec_actuation_mode(Manual{true} | Controller).expect(...)`.
- 3257–3299: `param_plan = plan_param_bootstrap(1, fields)`; `param_confirmed = plan yoksa true`; PARAM / CAPS yeniden deneme durumu; `caps_confirmed = !(Bitmap ∧ starved)`; yalın PTT durumu (`ptt_lean_peer`, `ptt_offered_at`, `ptt_pre_lean`, `ptt_expired`, `ptt_copy_pending`).
- 3304–3308: `frag_on = max_message_bytes.is_some()`, mesaj haritaları.

**Uyumlu: 47 davranış.** En önemlileri: INV-ENGINE-SOC — motor yalnız `OpaqueUnit`/`RxDatagram` metadata görür, mühür/açma `PacketProtection`'da, transport AEAD/anahtar tutmaz (8–10, 2446–2510); INV-RATE-1 — gönderim/egress reddi ve kurtarma bastırması kanal kaybıdır, hıza dokunmaz (9–10, 910–912, 2943–2950); INV-RATE-1 profil istisnası `FeedbackStarvedRule` kablolaması ve ≥ 1 s eşiği (405, 482–488, 3176–3192); 2026-11-16 500‰ ACK'siz FEC (495–515, 3237–3241); D-CC-1 ürün motoru port + ilk uçuş 4·MTU + E2a portta atlanır (338, 522–525, 3114–3128); PARAM canonical alanları ve "PARAM_ACK yoksa emisyon sürer" tabanı (871–883, 3257–3260); bağlam-önce dağıtım / TOFU yok / NotReady ACK'siz düşüş (2023–2510); `PN_SEQ_WINDOW` u16 sarmasından türetilmiş (86); bounded kanallar ve halkalar (SR-AUDIT-001); typed `RuntimeError` (887–903); `UplinkShare::control_should_yield` engine-spec §13 öncelik sırasıyla ("yeni DATA > ACK-only"). Kapsamda `dest_hash` ve FS etiketi geçmiyor (CLAUDE.md §4 dest_hash / FS satırları için ihlal yok). Kapsamda `todo!()` / `unimplemented!()` yok.

## Bulgular

| fid | tür | ağırlık | konum | davranış | otorite | düzeltme türü | düzeltme taslağı |
|---|---|---|---|---|---|---|---|
| C1R1-01 | UYUŞMAZ | orta | runtime.rs:718–724 (`TransportEngine::parse`), ilgili: kama-node lib.rs:333, tests/cc_port_runtime.rs:103 | `parse("")` → `Legacy`. Ürün düğümü ortam değişkeni TANIMLIYSA her değeri `parse`'a verir; `KAMA_TRANSPORT_ENGINE=` (boş) ürünü sessizce eski motora düşürür. `Default` ise `CcPort` (704–712). | KARAR-KAYDI §8 D-CC-1: "#176 (ürün varsayılanı port + ilk uçuş 4·MTU; geri alma `KAMA_TRANSPORT_ENGINE=legacy`)"; CLAUDE.md YAMA KAYDI 2026-11-11: "Ürün varsayılanı port (`RuntimeConfig::engine`), geri alma `KAMA_TRANSPORT_ENGINE=legacy`". Geri alma yalnız açık `legacy` sözcüğüyle. | kod-duzeltmesi | `"" => Some(Self::CcPort)`; 715–716 belgesi ve kama-node 331 yorumu güncellenir; `cc_port_runtime.rs:103` beklentisi `CcPort` olur. |
| C1R1-02 | UYUŞMAZ | dusuk | runtime.rs:94–97, 706 | `RuntimeConfig::engine` belgesi "`Legacy` (varsayılan, ürün profili) … `CcPort` … varsayılan DEĞİL (PR-2 kararına dek)" diyor; kod 338'de `CcPort`. | KARAR-KAYDI §8 D-CC-1 ("CC-PORT VARSAYILAN 399b573f"). | kod-duzeltmesi | Belge yorumu D-CC-1'e göre yeniden yazılır (davranış değişmez). |
| C1R1-03 | UYUŞMAZ | yuksek | runtime.rs:456–460 (`PTT_DEADLINE_NS`), 2185–2187, 2315–2325 (`ptt_deadline` atlama), 3283–3291 (son tarih/bırakma durumu) | Yalın kipte PTT demeti bir kez gider, 1 s sonra teyitsizse bırakılır (yeniden iletilmez), alıcı şeridi deliği bekleyip atlar. | application-message-protocol.md §3 PTT-R0: "`PTT_OPEN`, `MEDIA_BUNDLE`, `PTT_CLOSE` Reliable taşınır; alıcı `MEDIA_BUNDLE`'ları sırayla oynatır, eksik parça gelene kadar bekler, geç gelen parçayı **düşürmez**." PoC-KAPSAM-001 EK: "PTT satırı değişmedi (Reliable klip)". Kodun dayanağı sahip hedefi 2026-11-22 (PTT ≤ 4,5 kbit/s); spec için YAMA KAYDI bulunamadı (aranan: AMP PTT-R0 yaması, "PTT_DEADLINE"). | kilitli | Karar 3 (7171705736): "PTT ve metin davranışını dondurun … yeni değişiklik yapmayın." Kod değişmez; spec yaması ya da davranış kararı sahibin. |
| C1R1-04 | TANIMSIZ | orta | runtime.rs:407–432, 434–441, 452–474, 1960–1987 | PTT ileri kopya 7 (hat altıysa en az 6), boştaki şerit 5 iletim, güvenilir PTT kaydı 2 kopya / 20 ms, eski eşe dönüş 2 ACK. Eski kopyalı yolda PTT ≈ 3,99 kB/s (yorum 419). | CLAUDE.md YAMA KAYDI 2026-11-16: "PTT D× kopya kararı (#175 6919973033)" — D×5. 7'ye çıkış yorumda "sahip kararı 2026-11-21 m. 9"; bu yorum verilen otorite listesinde yok → DOĞRULANAMADI. Spec'te bulunamadı (aranan: PTT kopya sayısı, `PTT_FORWARD_COPIES`). | kilitli | Karar 3 gereği PTT tekrar/zaman aşımı sabitleri değişmez; 7 değerinin dayanağı sahipten teyit edilmeli. |
| C1R1-05 | UYUŞMAZ | orta | runtime.rs:1972–1977, 1995–2000 (`priority_below_id`) | FILE dışı şeritlerin (PTT/SIG/TEXT) YENİ birimleri yeniden iletim dalgasının önünde çıkar. | adaptive-transport-engine-spec.md §13: "priority: retransmit(deadline) > FEC parity > yeni DATA > ACK-only". Spec yaması bulunamadı (aranan: §13 öncelik YAMA KAYDI, "şerit öncelik"). | kilitli | Metin/PTT gönderim politikası (karar 3). Spec yaması ya da karar gerekir; kod şimdi değişmez. |
| C1R1-06 | TANIMSIZ | orta | runtime.rs:27–30 (0x15/0x16/0x17 ACK türleri), 235–241, 255–261, 3199–3215, 3271–3282 (CAPS 0x08, yalın PTT 0x09) | Bitmap ACK (0x15), yalın PTT'li bitmap (0x16), birleşik PTT'li bitmap (0x17), CAPS çerçevesi ve 0x09 yalın PTT çerçevesi kullanılır; kod kendi belgesinde "YALNIZ ÖLÇÜM PROTOTİPİ, biçim onayı yok" diyor (235). | engine-spec §1.1 frame_type tablosu yalnız 0x10–0x14, 0x1B, 0x1C, 0x20'yi tahsis eder; noise-wire §2.56 DR-CS-1 notu: "başka-codepoint/subtype/alan-ekleme = DUR-B". Karar 9 (7171705736) PoC'de bitmap'i kullanır, "V2 ürünün genel varsayılanı olarak kalır" der; kod tahsisi için spec yaması bulunamadı. | karar-gerekir | Kodlama noktası tahsisi (spec yaması, D-32 (iii) sınıfı) sahibin; o gelene dek varsayılan V2 kalır (bugünkü hâl). |
| C1R1-07 | UYUŞMAZ | orta | runtime.rs:1939–1958 (`unit_frame`, HDR_DATA 11 B), 59–66 (wire.rs çerçeve aileleri), 170–176 (CS-probe 44/43 B iddiası) | Ara çerçeve ailesi kullanılıyor: DATA iç başlığı 11 B (tip + seq16 + unit_id), RECOVERED 0x02, PARAM 0x03 / PARAM_ACK 0x04, CS_PROBE 0x06 / CS_ECHO 0x07 (düz metin 28/27 B). Kod "S3: interim çerçeve bu sprint beyanlı KARŞILANMAZ" diyor. | noise-wire §2.56: RECOVERED 0x12, PARAM_SET 0x14, PARAM_ACK 0x15, CS 0x19/0x1A, "0x12=RECOVERED-kilidi … runtime-assert"; §2.57 CS_PROBE "EXACT-WIRE = 44B". app-rpc §2: hot-path "thin overhead (established ≈ `hub_handle4 + seq16 + ciphertext + tag16`) korunur"; engine-spec §1: "SESSION_DATA hot-path'e ACK/FEC header, transport-prefix … EKLENMEZ". Mühürlü CS-probe boyu kapsamda ölçülemedi → 44 B iddiası DOĞRULANAMADI. | karar-gerekir | Bayt düzeni değişikliği talimat ister (CLAUDE.md §6, D-32 (iii)); beyanlı açık sahip takviminde. |
| C1R1-08 | TANIMSIZ | orta | runtime.rs:3223–3233 | Port motorunda yön hızı verilmişken parite sembolü uplink'te ≤ 2 s serileşmiyorsa FEC hiç kurulmaz (`fec = None`), PARAM çıkmaz; yapılandırma `fec: Some` iken sayaç ya da tipli sinyal yok. Profil aynı zamanda geri bildirimi aç ise 500‰ ACK'siz FEC kuralı da devre dışı kalır. | CLAUDE.md YAMA KAYDI 2026-11-16: "PTT dışı şeritlere profilin en yüksek FEC oranı … kod `runtime::fec_feedback_starved_permille`" — iki yönü dar hatta bu karar uygulanamaz. fec-core §11: "sub-kbps ≠ E=1024" (yönü destekler, eşiği tanımlamaz). Eşik spec'te bulunamadı (aranan: "parite serileşme", "2 s"). | karar-gerekir | Dar uplink + aç geri bildirim kesişiminde hangi kuralın geçerli olduğu sahip kararı; ayrıca kurulmayan FEC için bir `ClientStats` sayacı (salt gözlem) eklenebilir. |
| C1R1-09 | TANIMSIZ | dusuk | runtime.rs:780–847 | Kontrolcü yolunda parite tavanı: zaman kapısı `PARITY_SER_LIMIT_NS = 2 s` (sabit, srtt'den türetilmemiş), bayt kapısı `r·parity ≤ payload`, yol hızı `min(BtlBw, son teslim örneği)`. | fec-core §11 (yalnız yön); engine-spec §8.4/§10.2 (k, r fuzzy-driven) bu tavanı tanımlamaz. Spec'te bulunamadı (aranan: "payload r_max", "parite tavanı"). ADR-17 determinizmi korunuyor. | karar-gerekir | Eşik ve kural spec'e (CAL satırı) ya da sahip kararına bağlanmalı. |
| C1R1-10 | EKSİK | orta | runtime.rs:746–778 (`validate_fec`), 2760–2761 | `offer_queue = 0` ya da `offer_queue = max_inflight_units = 0` iken `tokio::sync::mpsc::channel(0)` `connect` içinde panik verir; doğrulamada denetim yok. | CLAUDE.md §9 "açık hata tipleri"; §6 typed-error; modül belgesi 885 "her sınır typed reddeder". | kod-duzeltmesi | `validate_fec` (ya da ayrı `validate_config`) sıfır kuyruk boylarını `RuntimeError::FecConfig`-benzeri mevcut varyantla reddeder (yeni varyant gerekmez: `FecConfig(&'static str)` metinle). |
| C1R1-11 | EKSİK | orta | runtime.rs:394–400, 482–488, 563–567, 3176–3191 | `LinkRates` doğrulanmaz: `downlink_bps = 0` → aralık `u64::MAX` → `link_feedback_starved = true`, `feedback_ns = u64::MAX`, `return_line.rate_bps = 0`; `uplink_bps = 0` → pacing tavanı 1 B/s ve `set_cc_line(rate 0)`. Taşıma sessizce kilitlenebilir. | CLAUDE.md §9 (typed error, bounds-before-run); YAMA KAYDI 2026-11-16 istisnası "bütçeli ACK aralığı ≥ 1 s" — sıfır hız bir ölçüm değil, tanımsız girdi. | kod-duzeltmesi | Kuruluşta `link_rates` için `uplink_bps > 0 ∧ downlink_bps > 0` typed reddi (`FecConfig` metniyle ya da ayrı doğrulama). |
| C1R1-12 | EKSİK | dusuk | runtime.rs:752–758 | `max_message_bytes` için üst sınır yok; alıcı birleştirici tavanı bu değere dayanır (server tarafı). | CLAUDE.md §9 SR-AUDIT-001 (bounds-before-alloc); engine-spec §14 "Tüm pool'lar sabit-üst-sınır". Üst sınır değeri spec'te bulunamadı (aranan: mesaj tavanı). | karar-gerekir | Üst sınır değeri (ör. signaling-seam "birkaç KB" / dosya parçalama) sahip/spec kararı. |
| C1R1-13 | TANIMSIZ | dusuk | runtime.rs:2629, 2900 | `lane & 0b11`: geçersiz şerit (ör. 4) sessizce 0'a (PTT) eşlenir; PTT kopya/son tarih politikası yanlış birime uygulanır. | CLAUDE.md §9 typed error; "sessiz geri dönüş" görev tanımında TANIMSIZ. | karar-gerekir | Tipli ret yeni bir hata varyantı ister (genel API imzası; CLAUDE.md §6 "imza değişikliği talimatsız YAPMA"). Ara çözüm: `lane > 3` için `UnitTooLarge` değil, ayrı varyant — sahip onayı. |
| C1R1-14 | TANIMSIZ | dusuk | runtime.rs:3242–3252 | `set_fec_actuation_mode(..).expect("taze Sender mode-set kabul eder")`: ürün görevinde panik yolu (bugün erişilemez varsayılıyor). | CLAUDE.md §6 / §9 (typed error; reachable panik yok). | kod-duzeltmesi | `expect` yerine hata hâlinde FEC'siz devam + sayaç (ya da görevi tipli kapanışla bitirme). |
| C1R1-15 | UYUŞMAZ | dusuk | runtime.rs:2987–2992 | `shutdown` görev paniğini `unwrap_or_default()` ile varsayılan istatistiğe çevirir; belgesi "panik yutulmaz: ayrı gözlemlenebilir" der. Çağıran paniği sıfır trafikten ayıramaz. | Kendi belgesi (2987–2988); CLAUDE.md §9 açık hata tipleri. | karar-gerekir | Dönüş tipini `Result<ClientStats, RuntimeError>` yapmak genel imza değişikliği; sahip onayı. Alternatif: `ClientStats`'a `task_panicked` bayrağı (alan ekleme de yüzey değişikliği). |
| C1R1-16 | UYUŞMAZ | dusuk | runtime.rs:300–306 | `TRANSPORT_V1_PROFILE_ID = "kama-v1-full"`, `TRANSPORT_V1_ARCHITECTURE = "transport-v1"`; ürün davranışı CC-PORT varsayılanı (338) ve İŞ-26 profil yollarıyla değişti, kimlikler değişmedi. | docs/transport-v1/EVOLUTION.md §9.5 (kod 304–305'te atıflı): "davranış değiştiren her değişiklik ya `transport-v1.x` ya da `transport-v2` olur … `kama-v1-full` adı KORUNAMAZ." D-32 (iii): ürün profili kimliği değişikliği sahip MERGE'ü ister. | karar-gerekir | Kimlik güncellemesi sahip kararıyla. |
| C1R1-17 | TANIMSIZ | dusuk | runtime.rs:517–554, 3127–3160 | `link_rates` verilip profil geri bildirimi aç DEĞİLKEN de: ilk pencere `uplink·t_ack`'e (≤ 1 MiB) büyür, pacer profil hattına sınırlanır, `set_cc_line` hat modeli kurulur. | YAMA KAYDI 2026-11-16: "Kapsam yalnız `link_rates` verilmiş port motoru + geri bildirim aç (`runtime::link_feedback_starved`)". engine-spec §0.3 (7) "ilk uçuş ürün profilinden (4·MTU; …yalnız yapılandırmayla)". Aç olmayan profil için kural ve 1 MiB tavanı spec'te bulunamadı. Kayıptan türemediği için INV-RATE-1 ihlali değil. | karar-gerekir | Aç olmayan profilde bu üç kolun kapsamı sahip kararıyla belirlenir. |
| C1R1-18 | TANIMSIZ | dusuk | runtime.rs:556–558, 1683–1684, 2754–2756 | Hat modeli ve PTT tel defteri datagram başına 28 B (IPv4+UDP) sayar; uç IPv6 ile de bağlanır (IPv6+UDP = 48 B). IPv6'da hat modeli datagram başına 20 B eksik sayar. Aynı değer iki ayrı sabitte. | CLAUDE.md §8 "ölçülmemiş byte tahmini" yasağı; fec-core §11 / K-FEC-MTU-1 "IPv4/IPv6+tunnel'de" ayrımı. | kod-duzeltmesi | Zarfı sunucu adres ailesinden türet (V4 28, V6 48), tek sabit kaynağı. |
| C1R1-19 | TANIMSIZ | dusuk | runtime.rs:2761, 2877–2882 (+ 3463/3470 `try_send`, C1-runtime-2 aralığı) | Teyit olay kanalı `max(offer_queue, max_inflight_units)` olaylık; dolunca `try_send` sonucu yok sayılır, olay sayaçsız düşer. | Modül belgesi 15–16 "taşan teslim sayaçla raporlanır — app drenaj sözleşmesi"; CLAUDE.md §9. | kod-duzeltmesi | Düşen teyit olayı için `ClientStats` sayacı (salt gözlem). |
| C1R1-20 | TANIMSIZ | dusuk | runtime.rs:221–241, 161–177, 178–198, 249–261 | Ürün yapılandırmasında spec'te tanımsız ölçüm kolları: `ack_rereport` (kendi belgesi: "spec sapmasıdır", §2.1), `diag_return_budget_permille`, `ack_format`, `e1_flags`, `e2a_flags` (KEEP-EXPERIMENTAL), `bootstrap_fast_ladder`, `loss_confirm_horizon ChannelDerived`, `fec_structure_policy BurstFusion`, `fusion_policy`, `ptt_coalesce`. Hepsi varsayılan kapalı. | engine-spec §2.1 "taşarsa en yeni bloklar öncelik"; D-23 "Her kural bayraklı; kapı geçince varsayılan açık … bayrak DEMO+1'e kadar". Kolların kapanış/silme tarihi bulunamadı. | karar-gerekir | Her kol için tut / kaldır kararı; tutulursa spec CAL satırı. |
| C1R1-21 | EKSİK | dusuk | runtime.rs:2440–2457 | HubChannel bağlamındaki her çerçeve sayılıp düşer; HEARTBEAT/HEARTBEAT_ACK (0x1B/0x1C) ve hub kanal mesajları işlenmez (PoC beyanı). | engine-spec §1.1: "0x1B HEARTBEAT … HubChannel ctx"; PoC-KAPSAM-001 K5: "Heartbeat yalnız mevcut PeerSlot'u yeniler, slot yaratmaz." | karar-gerekir | Hub sprinti kapsamı (beyanlı); imza korunarak eklenir. |
| C1R1-22 | TANIMSIZ | dusuk | runtime.rs:676–679 | `MAX_ECHO_SPACING_NS = 30 s` PARAM_ACK yineleme aralığı tavanı ("≤ 25 deneme, 5 s'de bir" varsayımından); kullanım C1-runtime-2 aralığında. | noise-wire §2.57: "PARAM_ACK … ACK-eliciting-DEĞİL"; yineleme aralığı spec'te bulunamadı (aranan: echo spacing, PARAM_ACK tekrar). | karar-gerekir | Sabit spec'e ya da kaldırma kararına bağlanır. |
| C1R1-23 | ÖLÜ | dusuk | runtime.rs:2995–2996, 3067–3068, 1163, 3300–3301 | `#[allow(clippy::too_many_arguments)]` bir struct'a (`ClientTaskLinks`) uygulanmış, etkisiz; 2995'teki görev belgesi struct'a yapışmış. 3067–3068 yorumu "FEC Manual-off" (ürün Controller). `tx_ctrl_bytes` belgesi "(PARAM_SET)" (Ctrl sınıfı ACK/RECOVERED/PARAM_ACK'i de kapsar). 3300 yorumu "mesaj tavanı birim tavanından BÜYÜK" der, kod `is_some()`. | CLAUDE.md §9 (precise engineering). | kod-duzeltmesi | Özniteliği ve belge yorumunu `client_task`'a taşı; üç yorumu koda göre düzelt (davranış değişmez). |

Doğrulanamayanlar (bulgu tablosunun dışında): INV-ACK-DELIVER-1 ("plan Ek 2026-10-11 (3)") `specs/` ve `docs/decisions/` altında bulunamadı (aranan: "INV-ACK-DELIVER-1") — NotReady ACK'siz düşüş davranışı (2065–2071, 2506) bu yüzden yalnız kodun kendi belgesine karşı okundu. Yuva anahtarı `(adres, SessionHandle)` PoC-KAPSAM-001 K5'in hub tarafı kuralının ("eş kimliği = hub_handle, SocketAddr değil") istemci-tarafı analoğu olarak beyan edilmiş; K5 hub içindir, eşleşme DOĞRULANAMADI.

## Modüller arası gözlemler

- kama-node `lib.rs:329–339`: boş `KAMA_TRANSPORT_ENGINE` değerini `parse`'a verir ve yorumu "Boş/`legacy` = ürün (bit-eşit)" der — PR-2 öncesi metin (C1R1-01).
- bench `kama_product.rs:156–162` ve `client-core/tests/cc_port_runtime.rs:103`: `parse("") == Legacy` sözleşmesini sabitler; C1R1-01 düzeltmesi bu testi değiştirir.
- adaptive-transport `receiver.rs:99–122` (`budgeted_ack_interval_ns`, `feedback_horizon_ns`): sıfır hızda `u64::MAX` döner; runtime bunu "aç" sayar (C1R1-11).
- adaptive-transport `sender` `FeedbackStarvedRule` / `line_guard`: runtime profil hattını üst sınır olarak verir; LineGuard'ın hattı ölçüyle aşağı çekmesi (3173–3175 yorumu) INV-RATE-1 2026-11-16 metniyle ("pacing profil hattıyla sert sınırlı (ölçüm aşamaz)") bu kapsamda değil, `line_guard.rs` denetiminde değerlendirilmeli.
- wire.rs çerçeve aileleri (0x00–0x09) noise-wire §2.56 kayıtlı kodlama noktalarıyla örtüşmüyor (C1R1-07); ACK ailesi 0x15–0x17 `ack.rs:68–75`'te (C1R1-06).
- `lanes` modülü (`file_window_floor`, `admit_widths`, `set_span_rooms`) `link_profile_window_bytes` ve `ExpectedSessions::set_lane_admit_windows` sözleşmesine dayanır.
- `transport::Reorder` / S1 adaptörü: `ExpectedSessions::advance_head`, `take_ptt_data_framed`, `ptt_deadline` ile PTT deliği atlama (C1R1-03, kilitli).
- C1-runtime-2: `acked_tx.try_send` (3463/3470) düşüşü (C1R1-19), `wire_len` (5431), `MAX_ECHO_SPACING_NS` kullanımı, `UplinkShare::control_should_yield` çağrı yerleri.

## Okunan otorite bölümleri

- Sahip kararları (#175): 7171705736 (dokuz karar; 2, 3, 7, 9 kapsamla ilgili), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (hepsi LineGuard / receiver / #262 / #263; kapsamdaki satırlarla doğrudan kesişen yalnız karar 3 ve 7).
- `docs/decisions/KARAR-KAYDI-2026-11.md` tamamı (§1–§10; özellikle §4 D-32 (iii), §8 D-CC-1).
- `docs/decisions/PoC-KAPSAM-001.md` K5–K7 ve EK satırları.
- CLAUDE.md §4 (INV-RATE-1 üç metni, INV-FEC-1, INV-ENGINE-SOC, FS, dest_hash), §6, §8, §9, YAMA KAYDI 2026-11-11 / 2026-11-16.
- `specs/adaptive-transport-engine-spec.md` §0.2–§0.3 (YAMA KAYDI 2026-11-11 dahil), §1, §1.1, §2, §2.1, §2.2, §12, §13 (YAMA KAYDI dahil), §14.
- `specs/kama-noise-wire-tdd-v2.0.md` §2.56, §2.57, PARAMETER-BINDING (B2, B4, state machine).
- `specs/application-protocol-and-rpc-spec.md` §1–§3 (üç düzlem, hot-path thin overhead).
- `specs/application-message-protocol.md` §3 PTT-R0…R5.
- `specs/signaling-seam-v0.md` tamamı.
- `specs/fec-core-tdd-v3.0.md` §11 (grep satırları 328–343).
- `specs/transport-layer-tdd.md` §5.4 ve invaryant tablosu (grep satırları 668–672, 992).
- `docs/transport-v1/EVOLUTION.md` §9.5.
