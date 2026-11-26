# A1-sender-1 — `sender.rs` satır 1–4800 davranış denetimi

Taban: `125b9afa`. Satır numaraları bu tabandandır. Salt okuma; kaynakta değişiklik yok.

## Kapsam

| dosya | satır | okunan |
|---|---|---|
| `crates/adaptive-transport/src/sender.rs` | 1–4800 (üretim kodu; testler 9600'den sonra, kapsam dışı) | 4800 |
| `specs/adaptive-transport-engine-spec.md` | 1–110, 250–1247 | 1108 |
| `specs/fec-core-tdd-v3.0.md` | 1–140 | 140 |
| `specs/transport-layer-tdd.md` | 925–960 + başlık/YAMA taraması | ≈ 36 |
| `docs/decisions/KARAR-KAYDI-2026-11.md` | 1–148 | 148 |
| #175 sahip kararları | 7171705736, 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 | tamamı |

Kapsam sınırı: `fuzzy_evaluate` (4822), `update_state` (5245), `compute_send_rate` (5548), `schedule_transmit` (5639), `drain_establish_probe` (8579), `queue_priority_head_hole` (8890) ve `queue_losses`'ın 4800 sonrası kuyruğu bu belgenin dışındadır. Bu işlevlere dayanan bulgular "DOĞRULANAMADI" ya da çapraz-modül notu olarak yazıldı. Bağlam için okunan dış satırlar: `tail_recovery.rs` 172–256, `bw.rs` 224–304, `rtt.rs` 113–125, `budget.rs` 18–33, `loss.rs` 899–911, `client-core/src/runtime.rs` 3076–3100 ve 3860–3995 (yalnız çağrı sözleşmesi).

## Davranış envanteri

### 1–50 · modül belgesi ve içe aktarma
- Modül belgesi: deterministik çekirdek (ADR-17), opak birim (INV-ENGINE-SOC), INV-RATE-1'in **eski** metni ("loss send_rate'i DÜŞÜRMEZ; recovery durumları pacing_gain'i Steady tutar"), RS+AEAD'in güvenlik katmanında olduğu, receiver'ın "sonraki artım" olduğu yazılı. Bu üç cümle bayattır (bulgu A1S1-15).

### 51–293 · olay ve durum tipleri
- `SenderState` (9 durum): §3 tablosuyla aynı küme (Idle, PreConverge, Startup, Drain, ProbeUp, Steady, RecoverArq, RecoverFwd, DrainRtprop).
- `OpaqueUnit {id, size}`: yalnız kimlik ve boy. Bayt, AEAD ya da anahtar yok (INV-ENGINE-SOC uyumlu).
- `Event`: AppData, AckReceived, TimerFired, RecoveredControl (salt-mantıksal), Tick (yalnız `experimental-cold-start`), CsProbeEcho (koşulsuz derlenir).
- `CsTimerKind`, `CsTerminal`, `PostTimeoutCause`, `PostTimeoutNotice`, `CsSrttSource`: DR-CS-1 deneysel cold-start yüzeyi, `experimental-cold-start` özelliği altında.
- Sabitler 192–198: `CS_PROBE_FIRES_NS` = 3/9/21/45 s, `CS_PROBE_WIRE_MAX` = 44 B, `CS_PROBE_TX_HARD_BUDGET` = 200 B, `CS_GIVEUP_NS` = 60 s. Hepsi [CAL]; 44 B noise-wire §2.57 `PROBE_WIRE_EXACT` ile uyumlu.
- `RecoveredInfo` / `RecoveredUnit`: FEC kurtarma bildirimi (unit_id, block_id, esi).
- `PathTimingState`: srtt, rtprop, rttvar, qd salt-okuma görüntüsü.
- `FecActuationMode` (Manual{enabled} | Controller): tek FEC aktüasyon otoritesi.
- `GateOutcome` + `GATE_OUTCOME_KINDS = 7`: yapı-gate teşhis kovaları.

### 294–641 · deneysel FEC/IW kolları
- `InitialWindowConfig {bytes}`: varsayılan 0. `>0` olduğunda PreConverge burst tabanını yükseltir (kullanımı 3213–3234).
- `FecLossRiskConfig`: Suspected kayıplardan spekülatif FEC kolu. Varsayılan kapalı. Sabitler: 125‰ / 250‰ tavan, 0,5×srtt persistence, ρ = 1,25, Q95, 10 000 ppm eşikler, 8 birim kanıt, 500‰ indirim, 2 500‰ sıkı bütçe, 63‰ taban, 300‰ reorder bastırma (bounded_v1). Hepsi [CAL]; ölçüm raporlarına atıflı ama spec'te yok.
- `bounded_v1()`: ürün varsayılanı. client-core `runtime.rs:372` bunu kurar.
- `learned_persist_min_ns`: `max(floor, q·ρ/1000)`, tavanlı; saf işlev.
- `SpeculativeLevel`, `SpeculativeProposal`, `FecArbiterReason`, `TsSample`, `ARBITER_REASON_KINDS = 7`.
- `fec_arbiter`: `max(confirmed, structure, speculative)`. Toplama yapmaz. Eşitlikte confirmed kazanır.
- `LossConfirmHorizon` (CadenceLegacy varsayılan | ChannelDerived{k}): ledger onay ufku ölçüm kolu. Varsayılan §16.1 "confirm = K_c×EAI" satırıyla aynı.
- `FecStructurePolicy` (Q18Iid varsayılan | BurstFusion). Belgesi yanlış öğeye bağlanmış (A1S1-16).

### 643–1031 · otorite tanığı, teşhis tipleri, deneysel bütçe
- `FecBlockAuthority`: tek kullanımlık tanık (Clone/Copy yok; derleme düzeyinde kanıtlı doc-testler). Yabancı Sender typed hata ile reddedilir.
- `FecAuthorityError`: thiserror, NotInManualMode / ForeignAuthority.
- `ControllerDecisionSnapshot`, `ActivationDiag`, `RecoveryTimersDiag`, `UnitRecoveryDiag`, `FecSuppressionReason`, `FecLatchResult`, `FecActuationDiag`, `FecTokenDenialSnapshot`: salt gözlem yapıları. Hiçbir karar yolu okumaz (belgeye göre).
- 912–1021 `experimental-fec-budget` özelliği: `BudgetPolicy`, `FloorTicket`, `BudgetExt`, `BudgetDiag`. Özellik kapalıyken derlenmez.
- `compute_requested_r(ratio, k, r_max)`: `(ratio·k + 500)/1000`, yarım-yukarı yuvarlama, `min(r_code_max)`. §10.2 `r = round(fec_parity_ratio × k)` ile uyumlu.

### 1033–1162 · ACK girdisi, zamanlayıcı, bootstrap RTT
- `ConfirmSource`, `AckInfo {acked_seqs, ack_delay}`.
- `Timer` yalnız `Pto` ve `RtpropProbe`. §12'deki IDLE_TIMER / PATH_DEAD_TIMER yok (A1S1-13).
- `BootstrapRttConfig`: DR-CS-3. Varsayılan kapalı. `establishment_fast` = 0,2/0,6/1,8/5,4 s merdiven, "KALİBRE EDİLMEMİŞ".
- `BS_FIRES_NS` = 3/9/21/45 s (`CS_PROBE_FIRES_NS` ile aynı değer, ikinci sabit), `BS_PROBE_WIRE_EXACT` = 44 B, `BS_TX_HARD_BUDGET` = 200 B.
- `BootstrapRttDiag`: sayaçlar. `budget_violation` ve `probe_after_success_violation` hiç artırılmaz (A1S1-17).
- `rate_per_srtt`: u128, doygun.

### 1163–1302 · startup pacing, sched teşhisi, emisyon kararı
- `StartupPacingConfig`: Default `enabled: true` (1226). Yapı belgesi (1163–1164) ve alan yorumları (2045, 2477, 2884) "default OFF" der (A1S1-15).
- `SchedDiag` (`diag-sched-telemetry`): salt gözlem.
- `TransmitDecision {kind, seq, unit_id, size, send_record}`: bayt yok.
- `SendRecordHint`, `SrLogicalRef::Fec`, `SrPacketClass`, `ParityTry`, `PendingFecParity` (blok/esi/boy + teşhis etiketi + `release_after_unit`).

### 1359–1541 · sınırlı defterler ve iz
- `CONFIRMED_LOGICAL_CAP = 4096`, `CONFIRMED_UNITS_CAP = 4096` (sahip kararı 2026-11-11), `ConfirmedCancelDiag`, `K_CONFIRMED_RELEASE = K_TOMB`, `SENT_LOGICAL_CAP = 16384`: bounded (SR-AUDIT-001 uyumlu).
- `TransmitKind`: Data, Retransmit, Duplicate, FecParity, CsProbe. FecParity belgesi "shard baytlarını güvenlik katmanı encode eder" der; AMENDMENT-5'ten sonra bayat (A1S1-15).
- `InflightPacket` (`confirmed_release_ns`, `data_ord`), `LostRec`, `TombstoneRec`: §5 TUR-2b tombstone kaydının alanlarıyla birebir.
- `EmitCause`, `EmitDiag`, `EMIT_LOG_CAP = 65 536`: iz; dolunca yazmaz.
- `COPY_GUARD_CAP = 64`, `SINGLE_SHOT_CAP = 64`, `LINE_GUARD_TRACE_CAP = 64`: bounded.

### 1547–1711 · hat koruması izi
- `ForwardIntervalNote`, `LineGuardAckDecision`, `LineGuardAckTrace`, `LineGuardSnap`, `LineGuardTraceRing` (halka, 64).
- `line_guard_trace_record`: önce/sonra gözlem farkından karar sınıfı türetir. Salt gözlem.

### 1712–1861 · PTT kopya kuralları ve geri bildirimi aç kural
- `COPY_LINE_BUDGET_NS` = 200 ms, `COPY_STALE_NS` = 1 s, `IDLE_RETX_MIN_AGE_NS` = 1 s, `IDLE_RETX_PER_TURN` = 4, `IDLE_RETX_LINE_SHARE_DEN` = 2: duvar saati sabitleri. PTT davranışı, sahip kararı 3 ile dondurulmuş (A1S1-19).
- `ForwardCopyRule {below_id, priority_below_id, copies, spacing_ns}`: kimlik sırasına göre önlemsel kopya.
- `ForwardCopyLineScale::copies_for`: `n = clamp(round((copies+1)·ölçülen/beyan), min+1, copies+1)` (sahip kararı 2026-11-21 m. 9).
- `FeedbackStarvedRule {pacing_cap_bps, feedback_ns, line, return_line}`: 2026-11-16 profil istisnasının taşıyıcısı. Belgesi (1809–1815) bu yolda pacing hızının "hattın kendisi" olduğunu, BBR'ın `kazanç × BtlBw` hızının okunmadığını yazar (A1S1-01).
- `DupRec {due_ns, unit, interval_ns, src}`: son tarih alanı yok (A1S1-13). `DUP_SRC_*` 1..8: iz kodları.
- `ProbeExit` (Abort/Timeout).

### 1862–2000 · motor sabitleri
- `INITIAL_RTT` = 100 ms [CAL initial]. Ek-B'nin iptal ettiği "sabit INITIAL_RTT-seed" modeli değil; geri dönüş değeri olarak kullanılır.
- `STRUCTURE_GATE_LOSS_MIN_PPM` = 100 000: kodun kendi belgesi "DAYANAK BORCU, geçerli kampanya yok, unsanctioned magic constant" der. `STRUCTURE_GATE_MIN_N` = 32, `STRUCTURE_GATE_IID_MAX_DELTA_PPM` = 50 000, `BURST_FUSION_MARGIN` = 3/2 (A1S1-20).
- `STARTUP_PLATEAU_ROUNDS` = 3, `STARTUP_PLATEAU_EPS_PERMILLE` = 50, `ACK_SILENCE_SRTT_MULT` = 3 (§5.1 ~3 ile uyumlu), `DEFAULT_DUP_FACTOR` = 2 (§11 D = 2..3), `DEFAULT_DUP_SPACING` = 20 ms (A1S1-14).
- `PRE_CONVERGE_FLOOR_BW` = 31 (§16.1 31,25 B/s).
- `ESTABLISH_PROBE_INITIAL_NS` = 500 ms, `ESTABLISH_PROBE_CAP_NS` = 8 s (A1S1-06).
- `EXPECTED_ACK_FREQ_N` = 2, `EAI_SACK_WIRE_BYTES` = 65 (§16.1 sack_wire 65 B), `EAI_RETURN_BW_PRIOR` = 31, `CADENCE_KC` = 3/2 (§16.1 K_c = 1,5).
- `K_WL` = 8, `K_TOMB` = 8, `TOMBSTONE_MAX` = 4096, `K_BW` = 8, `K_RTPROP` = 8, `K_QD` = 2, `K_CONV` = 4, `K_ARQ_SHARE` = 4, `DRAIN_*_DWELL_EPOCH` = 1/3/3, `K_PROBE_PERIOD_EPOCH` = 4, `PROBE_MAX_DWELL_EPOCH` = 4, `PROBE_VOLUME_EPOCH` = 2, `PROBE_SUCCESS_EPS` = 25‰, `PROBE_QD_ABORT` = 500‰, `K_CONF_EAI` = 2, `UNIT_SIZE_HINT` = 267: §16.1 tablosuyla değer değer aynı.
- `CC_MAX_UNIT_COPIES_IN_FLIGHT` = 8: CC-PORT U10 sınırı. Spec'te bulunamadı (aranan: uçuştaki kopya tavanı).
- `PC_SACK_WIRE` = 65, `PC_KBASE` = 3/2 (§16.1 1,3–1,5), `PC_COPY_MULTIPLIER` = 2 (§16.1 [STD] 2), `BURST_WINDOW_NS` = 40 ms (§16.1 R3).
- `NEXT_SENDER_INSTANCE`: süreç-yerel atomik sayaç. Yalnız yabancı tanık reddinde okunur; tel ya da karar çıktısına girmez (ADR-17 bozulmaz).

### 2003–2704 · `Sender` alanları ve kurucu
- Alan kümesi: ölçüm (loss, rtt, bw, btlbw), karar (fuzzy, budget, emergency), icra (pacer, arq), kuyruklar (`tx_queue`, `pending_lost`, `dup_queue`, `pending_fec`), tombstone, cold-start, FEC teşhisi, port motoru (`cc`, `cc_feedback_starved`, `cc_line_guard`, `cc_profile_pacing_cap`, `cc_line_floor`).
- Bounded olmayan iki kuyruk bu aralıkta sınırsız görünür: `tx_queue` (uygulama geri basıncı çağıranda) ve `dup_queue` (birim başına tek kayıt kuralları dışarıda). Sınır §14 `TX_POOL_MAX` ile çağıran katmandadır. DOĞRULANAMADI.
- `new()` / `try_with_fuzzy_config()` / `new_internal()`: fixture yapılandırması. Pacer `TokenBucket::new(min_rate_bps = 31, DEFAULT_BURST_CAP)`, dolu kova. `establish_probe_enabled = true`, `fec_mode = Manual{false}`.

### 2706–2840 · port motoru
- `with_cc_port()`: `Bbr::new(MTU, rfc9002_initial_window)` (10·MTU), `RetxConfig::rfc9002_cc_port` (yeniden iletim tavanı yok), `TailRecoveryConfig::rfc9002_cc_port` (PTO tavansız), `E2aFlags::new(true, 1 ms, rtt_rule_v11 = true, iw_idle_hold = true, 2 prob, false).expect(..)` (A1S1-08, A1S1-09, A1S1-21, A1S1-05).
- `fate_key` / `fate_key_for`: port açıkken `data_ord`, değilse birim kimliği.
- `cc_window()`: port yoksa `u64::MAX`. Kural yokken `cc.window()` (recovery_window dahil). Kural varken `window_without_recovery().max(cc_feedback_floor)` (A1S1-02).
- `cc_feedback_floor`: `cc_feedback_cap × (feedback_ns + rtprop|INITIAL_RTT)`.
- `cc_feedback_cap`: LineGuard `pace_bps` profil hattının altındaysa `pacing_cap × pace/line_rate`, [1, pacing_cap] aralığına kırpılmış. Değilse profil tavanı.
- `cc_fits(size)`: `inflight + size ≤ cc_window`.
- `diag_cc()`: salt okuma.

### 2842–3037 · ayarlayıcılar ve teşhis okuyucuları
- `set_bootstrap_rtt`, `set_cs_probe_lane` (özellik yokken no-op), `set_startup_pacing_init`, `set_e1_flags` (mandalı temizler), `set_tail_recovery_config` (bekleyen probu siler), `set_e2a_flags` (ikinci prob yuvasını siler).
- Diğerleri salt okuma: `bootstrap_rtt_diag`, `bootstrap_budget_remaining`, `startup_pacing_floor_bps`, `diag_pacer`, `diag_bw_accept`, `tail_probe_due_edge_count`, `diag_cc_*`, `diag_e2a`, `diag_loss_structure`, `diag_fate_corrections`, `tail_recovery_enabled`, `tail_probe_diag(_ext)`.

### 3039–3321 · ana döngü ve hız
- `on_event` (özellik kapalı): onay birikimini kırp, RecoveredControl'ü saatsiz işle ve boş dön (N8). Kalan olaylarda `on_event_core` + `schedule_transmit`. Özellik açıkken batch-of-1.
- `on_event_core`: pacer refill (önceki hızla), `apply_cc_line_floor`, LineGuard `tick` (süreli yoklama dolduysa hat modeli tavana döner), `fate_estimator.advance`. Olay dağıtımı: AppData kuyruğa girer. Port açıkken iş yokken gelen birim `first_send_ns`'i sıfırlar. Idle'dan çıkış: srtt yoksa PreConverge, varsa Startup. Ack, Timer, Echo (iki bootstrap kolu da besler). Ardından `fuzzy_evaluate`, `update_state`, startup tabanı kurma (PreConverge çıkışında bir kez).
  - Port yolu: hız = `cc.pacing_rate()` (> 0 ise) yoksa `1,25·window/srtt` (srtt yoksa INITIAL_RTT). Profil tavanı: `min(rate, max(cap, measured_target))` (A1S1-03). Geri bildirimi aç kural varsa hız = `cc_feedback_cap` (A1S1-01). Burst tavanı quinn formülü. PreConverge çıkışında kova doldurulur. Fuzzy kazancı okunmaz (§5.4 YAMA uyumlu).
  - Eski yol `apply_legacy_rate`: `max(natural, min(startup_floor, cwnd/srtt))`. Burst = `clamp(rate·40 ms, max(front, iw), max(front, 2·MTU, iw))`. iw yalnız PreConverge'de (ya da E2a `iw_idle_hold` ile ölçümsüz Idle'da). PreConverge çıkışında kova doldurulur. Startup tabanı rampa bitince ya da doğal hız yetişince kalıcı olarak bırakılır.
- `grounded_startup_rate`: `min(2,885 × min_cwnd/srtt, cwnd/srtt)`. `cwnd_over_srtt`: `max(cwnd_gain·BDP, min_cwnd)/srtt`, E1 (c) bayrağıyla Startup kazancı.

### 3323–3916 · cold-start batch, uyanma, deneysel F2A kolu
- `on_event_batch` (deneysel): dört faz (inbound tip önceliği → çözüm → vadesi gelen zamanlayıcılar → tek `schedule_transmit`). E2 tam no-op. `debug_assert` ile C2 son koşulu.
- `next_wakeup_ns` (varsayılan derlemede): `next_timer_deadline_ns` (CS, ARQ zaman eşiği, dup vadesi ya da jeton anı, kuruluş denemesi, DR-CS-3 merdiveni, tail PTO, E2a ikinci prob, kararsız kopya bırakma), `pending_work_ready_ns` (tx/fec/lost en küçük birimin jeton anı) ve `line_probe_ready_ns` (hat modeli açıkken bekleyen kuyruk probu) değerlerinin en erkeni. Yeni zamanlayıcı icat etmez.
- `cs_on_probe_echo`, `cs_phase2_resolve`, `cs_phase3_fire_due(_once)` (64 tur güvenlik tavanı), `drain_cs_probes` (pacer'dan geçer), `diag_cold_start`, `take_cs_terminal`, `record_post_timeout_notice`, `take_post_timeout_notice`, `diag_cs_bootstrap`, test kancaları (`#[doc(hidden)]`).

### 3918–4319 · ACK işleme
- `on_ack_received`: kanıt aralığını tüketir. Boş ACK'te erken döner ve öteki üç ACK notunu tüketmez (A1S1-18). `idle_retx_units`'i temizler, `last_ack_ns`'i damgalar.
- Pencere ayarı: `btlbw.set_window(K_BW × w_epoch)`, `rtt.set_rtprop_window(K_RTPROP × srtt)`, `rtt.set_qd_window(K_QD × w_epoch)` (A1S1-10), `set_conv_interval(rtprop/K_CONV)`. `w_epoch = max(2·267 B/last_full_bw + 2,097 s, INITIAL_RTT)` (A1S1-11).
- Uçuş kaydı eşleşmesi: uçuş baytı düşer, CC kuyruğu, kader kanıtı (yalnız Data), onay kümesi (FecParity hariç), mantıksal defter, BW örneği. RTT adayı: en büyük Data seq (Karn).
- Tombstone eşleşmesi: kol-1 (retx = 0) BW örneği ve Data ise RTT adayı. Kol-2 RTT ve oran örneği yok, teslim kredisi var, `note_spurious` (§5 TUR-2c uyumlu).
- Paket eşiği `reorder_thresh` ile, `on_ack_with_evidence`.
- Un-loss: `pending_lost`'tan iptal, kader düzeltmesi, onay, `note_spurious`, kol-1 RTT adayı. Kol-1 orijinal tombstone eşleşmesi de reorder öğretir.
- E2a v1.1 açıkken RTT adayı yalnız çerçevenin en büyük seq'idir. Kural (d)/(e) örneği reddeder (A1S1-05).
- RTT gözlemi: `apply_ack_delay` (rtprop korumalı; düzeltme reddinde ham örnek). Örnek sayacı, telemetri damgası, DrainRtprop nitelik bayrağı.
- Onaylı birimlerin bekleyen işi iptal edilir, uçuştaki diğer kopyalar kararsızlaşır, onay çıktısı yazılır.
- Port: paket başına `cc.on_ack`, `set_external_bw(btlbw)`, `on_end_acks`.
- `cc_line_guard_on_ack_traced` → `cc_line_guard_on_ack`: ilk pencere, çift alt sınırı, `on_ack_measured`, `on_pair_rate`, başlangıç bütçesi, `set_line_rate`, `set_line_pairs`, kuyruk yazımı, boşluk boşaltması. İz isteğe bağlı.
- Kayıp kuyruğu: PreConverge ve srtt yoksa ilan yok (§3.3-C(1)/D uyumlu). Ardından `queue_priority_head_hole` (kapsam dışı). Tail: gerçek ilerlemede zamanlayıcı yenilenir, uçuş boşalınca söner.

### 4383–4510 · zamanlayıcılar
- `RtpropProbe`: yalnız Steady'den, rtprop min-örnek yaşı ≥ W/2 ise (E1 açıkken qd ≥ 250‰ da gerekir) DrainRtprop.
- `Pto`: PreConverge dışında zaman eşiği kaybı, tombstone TTL budaması (`forget_in_flight`), kararsız kopya bırakma. Ardından tail probu: port açıkken hedef uçuş ∪ `pending_lost` içindeki en eski kayıttır. Kopya korumasında beklenir. U14: port açık, srtt yok ve merdiven en eskiyi taşıyorsa beklenir. Bu koşul dışında PreConverge'de de prob kurulur (A1S1-04).

### 4512–4648 · DR-CS-3 ve kayıp eşiği girdileri
- `drain_bootstrap_probes`: PreConverge'de ve srtt yokken basamak başına bir prob. Pacer'dan GEÇMEZ ("additive"). 200 B toplam bütçe. Tek bekleyen yuva (A1S1-22).
- `bs_on_echo`: tek tüketim, yerel zaman damgasıyla RTT örneği, merdiven kalıcı olarak kapanır.
- `cadence_loss_floor` = 1,5 × EAI. `expected_ack_interval` = `2·ort_birim/pacer_hızı + 65/31 s` (§5(b) ve §16.1 ile aynı formül).
- `time_loss_inputs`: port ya da `tt_rfc` açıkken `(max(srtt, latest), 1 ms)`, değilse `(srtt, cadence_floor)`. `time_loss_threshold_ns` = `max(9/8·rtt, taban)`.

### 4650–4800 · `queue_losses` (ilk kısım)
- Uçuş baytı ve CC kayıp baytı düşer, prob atfı işaretlenir.
- Kararsız kopya, tek iletimli birim ve (port açıkken) daha taze Data/Retransmit kopyası uçuşta olan birim yeniden kuyruğa girmez.
- Duplicate kaybı: tombstone yok, `forget_in_flight`. Planlı kopya yoksa geometrik kopya yeniden kurulur (srtt aralıklı).
- Data kaybı kader kestiricisine gider (yalnız Data). FecParity kaybı ARQ'ya girmez (sayaç). Öteki kayıplar `pending_lost`'a girer.
- Tombstone: `TOMBSTONE_MAX` ile en eskisi atılır (audit sayacı). TTL = `K_TOMB × T_loss`.

### Uyumlu davranışlar
Toplam **48 davranış uyumlu**. Önemlileri:
- INV-ENGINE-SOC: `OpaqueUnit` / `TransmitDecision` bayt ve anahtar taşımaz. INV-FEC-1: transport parite baytı üretmez, yalnız `PendingFecParity` metadata'sını pace eder.
- §6 TUR-1x ack_delay koruması (eski yol). Karn RTT seçimi. §5 TUR-2b/2c tombstone kol-1/kol-2. §9.3 un-loss ve `note_spurious` kabloları.
- §3.3-C(1)/(D): PreConverge'de kayıp ilanı yok. İlk geçerli SACK'le kilit açılır.
- §16.1 sabitleri değer değer aynı: K_c, K_wl, K_TOMB, TOMBSTONE_MAX, K_bw, K_rtprop, K_QD, K_CONV, K_ARQ_SHARE, ProbeUp eşikleri, UNIT_SIZE_HINT, PRE_CONVERGE_FLOOR_BW, BURST_WINDOW, k_base, COPY_MULTIPLIER, sack_wire.
- §0.3 CC-PORT YAMA'sı (kural yokken): pace = `pacing_gain × BtlBw` (1), dış BtlBw Kama örnekleyicisinden (2), burst `clamp(window·2 ms/srtt, 10·MTU, 256·MTU)` (6), zaman eşiği `9/8·max(srtt, latest)`, taban 1 ms (4), PTO tavansız ve 2 prob (5), fuzzy kazancı hız yolunda değil (§5.4 YAMA).
- 2026-11-16 profil istisnası: kural varken `recovery_window` uçuş tavanını sınırlamaz.
- §10.2 `r = round(ratio × k)`. `fec_arbiter` toplama yapmaz. FecParity kaybı ARQ'ya girmez (fec-core §4/§6, engine §9). Duplicate kaybı yeniden kuyruğa girmez.
- SR-AUDIT-001: onay/mantıksal/iz/tombstone/kopya koruması/tek iletim defterleri tavanlı.

## Bulgular

| fid | tür | ciddiyet | konum | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| A1S1-01 | UYUŞMAZ | yüksek | 3183–3185; 1809–1815; 2784–2800 | Geri bildirimi aç kuralda pacing hızı `cc_feedback_cap` olur: LineGuard `pace_bps`'ten türeyen hat hızı, ilk ölçümden önce yarım profil. BBR'ın `pacing_gain × BtlBw` hızı hiç okunmaz. Hız denetimi BBR'dan alınıp LineGuard'a verilmiş; iki tahminci aynı değişkeni yönetiyor. | CLAUDE.md §4 INV-RATE-1 2026-11-11: "pace = BBR pacing_gain × BtlBw". 2026-11-16 istisnası: "pacing profil hattıyla sert sınırlı (ölçüm aşamaz)". İstisna tavan koyar, kaynağı değiştirmez. engine-spec §0.3 YAMA (2). Hız kaynağının LineGuard olması hiçbir YAMA KAYDI'nda yok. engine-spec 2026-11-21 YAMA'sı yalnız göndericinin hattı tahmin ettiğini söyler. Sahip kararı 7171705736 m. 5, LineGuard tavanı üzerinden işler. | karar-gerekir: ya `rate = min(cc.pacing_rate(), cap)` (metin) ya da INV-RATE-1'e "geri bildirimi aç profilde pace = ölçülen hat" yaması |
| A1S1-02 | UYUŞMAZ | orta | 2761–2780 | Kural varken uçuş tabanı `cc_feedback_cap × (feedback + rtprop)` olur. Hat LineGuard ile ölçülünce ya da başlangıçta yarı profilde taban da küçülür. RTT ölçülmeden `INITIAL_RTT` kullanılır. | CLAUDE.md §4 2026-11-16: "tavan en az hat × (geri bildirim ufku + RTT)"; "hat" = profil hattı. Ölçülen hatta inen taban metinde yok. | karar-gerekir |
| A1S1-03 | TANIMSIZ | orta | 3171–3173 | Kural yokken profil tavanı `min(rate, max(cap, measured_pacing_target))`: ölçüm tavanı aşabilir. Tavanın yalnız ölçümsüz payı kestiği belgede yazılı. | Spec'te bulunamadı (aranan: `cc_profile_pacing_cap`, profil tavanı, kural dışı). 2026-11-16 YAMA kapsamı yalnız geri bildirimi aç yol. | karar-gerekir |
| A1S1-04 | UYUŞMAZ | orta | 4465–4507 | PreConverge'de PTO probu kurulabilir. Port: merdiven en eski birimi taşımıyorsa (U14 canlılık istisnası). Eski yol: tail açıkken hiç PreConverge koruması yok; srtt yokken `initial_pto_ns` tohumu kullanılır (`tail_recovery.rs:174`). | engine-spec §3.3-C(5): "PTO ARM EDİLMEZ"; §12 NOT: "PRE_CONVERGE'de … PTO ARM DONDURULUR"; Ek-B: INITIAL_RTT-seed modeli MUST NOT IMPLEMENT. CC-PORT YAMA'sı §12'de yalnız "PTO tavanı"nı değiştirdi. | karar-gerekir (canlılık gerekçesi #175 Ö-CC; sahip yaması yok) |
| A1S1-05 | UYUŞMAZ | orta | 4180–4202; 2723 | Port ürün yolunda E2a v1.1 kural (d)/(e) açıktır: doygun `ack_delay` ya da düzeltme reddi + `ack_delay ≥ rtprop` olduğunda RTT örneği ATLANIR (`rtt_rule_refused`). | engine-spec §6 [TUR-1x]: "underflow ya da min-rtt-altı → RAW sample işlenir, örnek ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı YASAK)". Spec'te bulunamadı (aranan: v1.1, rtt_rule, KK-M1-06). | karar-gerekir |
| A1S1-06 | UYUŞMAZ | orta | 1920–1925; 2147–2155; 3519–3534 | DR-EST-1 kuruluş kopyası "token-muaf / token'sız kol" (pacer dışı) ve varsayılan AÇIK. Aralık 500 ms'den 8 s'ye duvar saati sabitleriyle katlanır. Emisyon kodu 8579'da (kapsam dışı). | engine-spec §3.3-B: "pacer'ı baypas eden gönderim yolu YOK (DATA/retransmit/kopya tek token bucket'ı paylaşır)". §16.2 ZM-2: duvar saati sabiti 4-decade doğrulaması ister. DR-EST-1 otorite belgelerde bulunamadı. | karar-gerekir |
| A1S1-07 | UYUŞMAZ | düşük | 3213–3234; 314–318 | Eski yolda `InitialWindowConfig.bytes > 0`, PreConverge burst tabanını yükseltir. Ürün runtime'ı 2 400 B kurar (`runtime.rs:323,3120`). Yalnız geri alma (legacy) yolu. | engine-spec §3.3-A [K-D2]: "PRE_CONVERGE'de burst tek-birime clamp'lenir". | karar-gerekir |
| A1S1-08 | UYUŞMAZ | düşük | 2715–2726 | `with_cc_port` ilk pencereyi RFC 9002 10·MTU kurar. Ürün runtime'ı `set_cc_initial_window` ile ezer; kurucunun varsayılanı ters. | engine-spec §0.3 (7): "ilk uçuş ürün profilinden (4·MTU; RFC 9002'nin 10·MTU'su yalnız yapılandırmayla)"; KARAR-KAYDI §8 D-CC-1. | kod-düzeltmesi (kurucu varsayılanı 4·MTU; 10·MTU açık yapılandırma) |
| A1S1-09 | TANIMSIZ | düşük | 2723–2724 | `E2aFlags::new(..).expect(..)`: sabit argümanlarla panik yolu. Fiilen ulaşılmaz ama typed hata yerine panik. | CLAUDE.md §9 (açık hata tipleri), §6 (ulaşılabilir panik yasak) | kod-düzeltmesi (const kurucu ya da doğrulanmış sabit) |
| A1S1-10 | UYUŞMAZ | düşük | 3954–3959 | `W_qd = K_QD × w_epoch` (delivered-EAI saati). | engine-spec §6 ve §16.1 K_QD satırı: "W_qd = K_qd × ack_epoch". fec-core-tdd §0.8: "FAZ-0 qd-canlanma (PR#41) — ack_epoch→w_epoch" mühürlü kazanım. Spec ↔ spec çelişkisi. | karar-gerekir (engine-spec metni) |
| A1S1-11 | TANIMSIZ / ÖLÜ | düşük | 3935–3951 | `w_epoch` içinde `2_097_000_000` sabiti, `EAI_SACK_WIRE_BYTES/EAI_RETURN_BW_PRIOR`'un elle yazılmış kopyası. `.max(INITIAL_RTT)` tabanı §16.1 Y3 satırında yok. `ack_epoch` hesaplanır ve `let _ =` ile atılır. | engine-spec §16.1 W_BTLBW Y3: `w_epoch = 2×unit/last_full_bw + return-prior(2.097s)`; §0 sabit disiplini (magic-number yasağı). | kod-düzeltmesi (sabitten türet; ölü hesabı sil; taban için karar) |
| A1S1-12 | EKSİK | orta | 1830–1840 (DupRec), §3.3-B/§11/§11.1 emisyon yolları | Hop-ACK almadan ardışık PN ≤ K = 1024 tavanı yok. | transport-layer-tdd §12 [AMENDMENT-9] + KS-PN-1 (K = 1024): "DURUM: SPEC-ONLY … kod bugün sınırsızdır. Uygulama … AYRI bir iş kalemidir". | karar-gerekir (altı beyanlı ayrı iş) |
| A1S1-13 | EKSİK | orta | 1052–1059; 1830–1840 | `Timer` yalnız Pto/RtpropProbe. IDLE_TIMER, PATH_DEAD_TIMER, RETRY_BUDGET, COPY_MAX_INTERVAL yok. `DupRec`'te son tarih yok. Geometrik kopya ölü kanalda tavansız yeniden kurulur. Idle geçişi 5271'de (kapsam dışı; DOĞRULANAMADI). | engine-spec §12 IDLE_TIMER / PATH_DEAD_TIMER / RETRY_BUDGET; §16.1 COPY_MAX_INTERVAL = deadline/8; §16.2 satırı "geometrik kopya deadline'sızlığı … kodda yok". | karar-gerekir (per-unit deadline motora taşınmıyor; §5 ZEYL) |
| A1S1-14 | UYUŞMAZ | düşük | 1916 | `DEFAULT_DUP_SPACING = 20 ms` duvar saati sabiti hâlâ kullanımda. | engine-spec §16.2: "dup_spacing=20ms · wall-clock · 20ms ≪ serialization → … dekorelasyon amacı ölür · gereken: k·birim-serialization"; ZM-2. | karar-gerekir (k değeri) |
| A1S1-15 | UYUŞMAZ (belge) | düşük | 7–9, 13–15, 23–24, 1163–1164, 1427–1428, 2045, 2477, 2884 | Modül belgesi INV-RATE-1'in üstü çizili metnini ve "recovery durumları pacing_gain'i Steady tutar"ı taşır. RS encode "güvenlik katmanında" der. Receiver "sonraki artım" der. `StartupPacingConfig` belgesi ve alan yorumları "default OFF" der, Default `true` döner. | CLAUDE.md §4 INV-RATE-1 2026-11-11; engine-spec §0.2 AMENDMENT-5 (RS execution transport'ta); `StartupPacingConfig::default` (1226). | kod-düzeltmesi (yorumlar) |
| A1S1-16 | TANIMSIZ (belge) | düşük | 320–335; 632 | `[BURST-FUSION]` belgesi `FecLossRiskConfig`'in belgesine yapışmış; `FecStructurePolicy` belgesiz. | CLAUDE.md §9 | kod-düzeltmesi |
| A1S1-17 | ÖLÜ | düşük | 4546; 1149–1152 | `budget_violation += 0` no-op. `budget_violation` ve `probe_after_success_violation` hiç artırılmaz; "MUST stay 0" testleri boş geçer (11480, 11601). | — (ölçüm hijyeni; boş sayaç) | kod-düzeltmesi (ihlal anında artır ya da alanları kaldır) |
| A1S1-18 | UYUŞMAZ | düşük | 3919–3924; 4326–4331; 4351–4353 | Boş `acked_seqs`'te erken dönüş `next_ack_evidence`'i tüketir ama `pending_arrival_rate`, `pending_pair_rate` ve `pending_forward_interval`'ı tüketmez. Bunlar sonraki ACK'e taşınır (sonraki çerçeve alanı taşımıyorsa bayat değer uygulanır). | Kodun kendi sözleşmesi 4351–4352: "önceki ACK'in değeri kullanılmaz". engine-spec 2026-11-21 YAMA (1): alan "yalnız oturumun ilk ACK'inde taşınır". | kod-düzeltmesi (erken dönüşte üç notu da `take`) |
| A1S1-19 | TANIMSIZ | düşük | 1712–1789; 1534–1541; 1725–1734 | PTT/öncelikli şerit kopyaları, hat ölçekli kopya sayısı, bayat kopya düşürme (1 s), hat kuyruğu bütçesi (200 ms), boş hat yeniden iletimi (1 s, 4/tur, ½ hat), kopya koruması, tek iletim. Spec'te yok; duvar saati sabitleri ZM-2 doğrulamasız. | Sahip kararı 7171705736 m. 3: "PTT ve metin davranışını dondurun"; 2026-11-21 m. 9; CLAUDE YAMA 2026-11-16 (PTT D×). | kilitli |
| A1S1-20 | TANIMSIZ | orta | 336–448; 575–605; 1865–1907 | FEC arbiter'in kanonik fuzzy dışındaki iki kolu: spekülatif Suspected kolu (`bounded_v1` ürün varsayılanı, client-core) ve Q18 yapı-gate'i (100 000 ppm, kodun kendi belgesine göre "unsanctioned magic constant"). [CAL] sabitlerin hiçbiri spec'te yok. | engine-spec §5: `loss_pre_fec = Confirmed_units / pencere`; §8.4 kanonik kural tablosu; §0 sabit disiplini ("UYDURULMAZ"). Spec'te bulunamadı (aranan: speculative, Suspected FEC, structure gate). | karar-gerekir |
| A1S1-21 | UYUŞMAZ | düşük | 2721 | Port kurucusu yeniden iletim tavanını kaldırır (`RetxConfig::rfc9002_cc_port`). | engine-spec §9.1: "ARQ_RETX_CAP: … aşılırsa FEC/forward'a devret veya connection-fail [CAL]". §0.3 YAMA'sı tavan kaldırmayı listelemez. | karar-gerekir |
| A1S1-22 | TANIMSIZ | düşük | 4512–4563; 1087–1098 | DR-CS-3 probu pacer dışında. Bütçe 200 B sabit; forward hıza bağlı değil. `establishment_fast` 5,4 s'de 176 B gönderir (≈ 33 B/s > 0,25 kbps hattın 31 B/s'i). Ürün varsayılanı kapalı (`bootstrap_rtt: false`). | engine-spec §16.1 PROBE_BUDGET_FRACTION: "probe forward ≤ Z·forward_bw" (0,10–0,20); §3.3-B pacer tek kapı. | karar-gerekir |
| A1S1-23 | TANIMSIZ | düşük | 2063, 2199, 3130–3133 | `experimental-cold-start` derlemesinde F2A ve DR-CS-3 kolları ayrı sayaçlardan (ikisi de 0'dan) `CsProbe` üretir. Aynı yankı iki işleyiciye de verilir. Runtime iki kolu da `bootstrap_rtt` ile açar; `--all-features` derlemesinde `probe_seq` çakışabilir. F2A kurulumu 9210'da (kapsam dışı; eşzamanlı kurulum DOĞRULANAMADI). | engine-spec §11.1 / INV-CS-7: "session-monoton; yeniden-kullanım-YOK". | kod-düzeltmesi (tek sayaç ya da kollar karşılıklı dışlayan) |
| A1S1-24 | DOĞRULANAMADI | düşük | 4633–4641 | Zaman eşiği tabanında `min_delay` terimi yok: `max(9/8·rtt, taban)`. `loss.rs` içinde olabilir. | engine-spec §5(b): `max(TIME_THRESH × srtt, min_delay, K_c × EAI)`. | — |
| A1S1-25 | DOĞRULANAMADI | düşük | 3099–3103 | `first_send_ns` etkin dönem yeniden tabanı yalnız port yolunda. Eski yolda bayat çapa kalabilir. ack_confidence hesabı kapsam dışı (`fuzzy_evaluate`). | engine-spec §16.2 "ack_confidence anchor `.or()` … gereken: spec formülü + aktif-dönem re-base". | — |

## Okunan otorite bölümleri

- CLAUDE.md §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §5 ADR-06/14/16/17/18, §8, §9; YAMA KAYDI 2026-11-11 ve 2026-11-16.
- `docs/decisions/KARAR-KAYDI-2026-11.md` §1–§10 (özellikle §8 D-CC-1).
- #175 sahip kararları: 7171705736 (dokuz karar; m. 3 PTT/metin dondurma, m. 5 çift yoklaması, m. 8 %3 pay yasağı), 7172579876 (K2′ tek yoklama, `KAMA_PAIR_PROBE_DATAGRAMS` 125b9afa'da YOK), 7174289733, 7174845328, 7175487850, 7175498704, 7175888074 (#262 deneyleri; park dalı bu tabanda değil), 7186813951 (receiver.rs alarm aralığı Web'de).
- `specs/adaptive-transport-engine-spec.md`: YAMA KAYDI başlıkları; §0.1–§0.3 (CC-PORT YAMA (1)–(8)); §3, §3.1–§3.3 (A–F); §4; §5, §5.1; §6; §7.1–§7.4; §8.1–§8.5; §9–§9.4; §10, §10.1, §10.2; §11 (AMENDMENT-3); §11.1; §12; §13 (CC-PORT YAMA); §14; §15; §16, §16.1, §16.2; §17; Ek; Ek-B.
- `specs/transport-layer-tdd.md`: YAMA KAYDI başlıkları; §5.4 CC-PORT YAMA; §12 [AMENDMENT-9] KS-PN-1; §14 INV-RATE-1 satırı.
- `specs/fec-core-tdd-v3.0.md`: statü, §0, §0.8 TRANSPORT-NON-REGRESSION (mühürlü kazanımlar ve paylaşılan kod yasak listesi), §0.5, §1, §1.5.
- `specs/tech-stack-adr.md`: CLAUDE.md §5 özeti üzerinden (ADR-06/14/16/17/18). Dosyanın kendisi bu turda satır satır açılmadı.
