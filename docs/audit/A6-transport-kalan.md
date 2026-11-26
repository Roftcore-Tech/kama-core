# A6-transport-kalan — adaptive-transport kalan modüller denetimi

Taban: `125b9afa` (dal `yerel/kod-denetimi`). Satır numaraları bu tabandandır. Salt okuma; kaynak dosyalara yazılmadı, cargo çalıştırılmadı.

## Kapsam

| Dosya | Okunan aralık | Satır |
|---|---|---|
| `crates/adaptive-transport/src/fs_harness.rs` | 1–1601 | 1601 |
| `crates/adaptive-transport/src/udp_runner.rs` | 1–1119 | 1119 |
| `crates/adaptive-transport/src/loss.rs` | 1–1075 | 1075 |
| `crates/adaptive-transport/src/fusion_policy.rs` | 1–811 | 811 |
| `crates/adaptive-transport/src/fs_trace.rs` | 1–744 | 744 |
| `crates/adaptive-transport/src/tail_recovery.rs` | 1–634 | 634 |
| `crates/adaptive-transport/src/fate_estimator.rs` | 1–612 | 612 |
| `crates/adaptive-transport/src/fusion.rs` | 1–579 | 579 |
| `crates/adaptive-transport/src/arq.rs` | 1–214 | 214 |
| `crates/adaptive-transport/src/emergency.rs` | 1–207 | 207 |
| `crates/adaptive-transport/src/fec.rs` | 1–109 | 109 |
| `crates/adaptive-transport/src/sim.rs` | 1–75 | 75 |
| `crates/adaptive-transport/src/lib.rs` | 1–66 | 66 |
| `crates/adaptive-transport/src/error.rs` | 1–59 | 59 |
| **Toplam** | | **7905** |

Bağlam için (denetim kapsamı dışı, yalnız çağrı yerini doğrulamak üzere) okunan parçalar: `sender.rs` 2425–2440, 2700–2730, 4086–4100, 4870–4900, 5170–5215; `bench/src/kama_product.rs` 245–262; `client-core/src/runtime.rs` (grep: `fec_loss_risk`, satır 372); `docs/design/CC-PORT-ESLEME.md` 30–70.

## Davranış envanteri

### lib.rs (66)
- Modül ağacı. `#![forbid(unsafe_code)]` (ADR-01 ile uyumlu). `fs_harness`, `fs_trace`, `udp_runner` yalnız `experimental-fec-budget` özelliğiyle derlenir; geri kalanlar her yapıda.
- Belge yorumu: "Hepsi INV-RATE-1 (loss ⟂ rate)" (11), "fate_estimator … no product caller" (35–36), "Sıradaki: bench KamaTransport'u motora bağla" (16), ADR-05 ertelenmiş not: ACK/FEC çerçeveleri ccr-codec yerine yerel `read` modülüyle kodlanır (18–21).

### error.rs (59)
- `TransportError` (thiserror): `Truncated{offset,needed}`, `TrailingBytes{count}`, `UnknownFrameType(u8)`, `InvalidShardCount{k,r}` (k+r>255), `TooManyAckRanges{count,max}` (MAX_ACK_RANGES=16 decode'da zorlanır), `InvalidAckBitmap(&str)` (0x15 ölçüm prototipi), `FeatureNotImplemented(&str)` (typed stub). Durum tutmaz.

### sim.rs (75)
- `Fate{Deliver(Duration),Drop}`, `Delivery{index,delay}`, `LinkSim{script,default}`.
- `LinkSim::fate(i)`: script dışı paket `default` kaderini alır.
- `LinkSim::deliver(clock,count)`: teslim edilecekleri gecikmeye göre (eşitlikte indeks) sıralar, `clock.sleep_until(start+delay)` ile bekler, varış sırasında döner. RNG yok (ADR-17).

### fec.rs (109) — DEPRECATED
- `FRAME_TYPE_FEC_SHARD=0x20`, `SHARD_TAG_LEN=16` (tel biçimi sabiti).
- `FecShardFrame{block_id u32, shard_index u8, k u8, r u8, shard_len u16, opaque_secured}`; `#[deprecated]`: D1 kararıyla noise-wire EXTENDED mühre taşındı (METADATA-LEAK-01).
- `encode`: 10 B başlık + opak blob, sabit genişlik big-endian. `decode`: tip 0x20 değilse `UnknownFrameType`; k+r>255 → `InvalidShardCount`; `shard_len+16` blob `take` ile sınırlı okunur (PARSER-02); artan bayt → `TrailingBytes`. `shard_index < k+r` denetlenmez.
- Kullanıcısı yalnız `tests/tv_fec.rs`.

### emergency.rs (207)
- `EmergencyLevel{Arq<Dup2<Dup3}`; `escalated`/`deescalated` doygun tek adım.
- `HysteresisConfig::fixture()`: escalate 300 ‰, de-escalate 700 ‰, dwell 1000 ms — "KALİBRE DEĞİL" [CAL].
- `EmergencyConfig::fixture()`: D 2/3, kopya aralığı 20 ms [CAL], Dup2/Dup3 parite 100/200 ‰ (spec §11 AMENDMENT-3'te adıyla geçen değerler).
- `HysteresisController::update(health,now)`: health < escalate ve Dup3 değilse ANINDA bir üst seviye; health > de-escalate ve Arq değilse, son de-escalate'ten bu yana dwell geçtiyse bir alt seviye ve `last_deescalate_ns=now`. Dwell çapası YALNIZ önceki de-escalate'tir; escalate dwell'i başlatmaz. `t + dur_ns(dwell)` denetimsiz toplama.
- `action(level,framing,cfg)`: Arq→`Arq`; Tiny→`Duplication{factor,spacing}`; SegmentBlock→`MinimalParity{permille}`.
- `ack_decimation(level)`: Arq dışında `true`. Ürün çağıranı yok (yalnız `tests/tv_emergency.rs`).
- Üründe `sender.rs:2464–2465` iki fixture ile kurulur.

### arq.rs (214)
- `LostSeq{seq,deadline_ns,retx_count}`, `CapExceedPolicy{HandToForward,ConnectionFail}`, `RetxDecision{Retransmit,CapExceeded}`.
- `RetxConfig::fixture()`: cap 3, K=3 [STD], ρ=2/1 [CAL], HandToForward. `RetxConfig::rfc9002_cc_port()`: cap `u8::MAX` = tavan yok (CC-PORT-ESLEME satır 41'de beyanlı).
- `DeadlineArqScheduler::plan(lost,arq_aggr)`: arq_aggr==0 → boş plan; aksi deadline'a göre sıralar, süresi geçen DÜŞÜRÜLMEZ; cap aşımı `CapExceeded` (cap==u8::MAX ise asla).
- `ReorderTracker`: `reorder_thresh = max(K, ρ·recent_max_reorder)`; `note_spurious` en büyüğü tutar, hiç azalmaz.
- Test `saturated_retx_count_is_still_retransmitted_under_cc_port`: port kurucusunda 255 sonrası da yeniden iletim; fixture'da `CapExceeded`.

### fusion.rs (579)
- `log2_fp(x)`: `ilog2·256 + 8 bit kesir`, x=0→0, taşmasız (spec §16.2 TUR-4 satırıyla uyumlu).
- `trap_permille`/`Trap::mu`: trapezoit, omuz durumları, u128 ara hesap.
- `VarThresholds::fuzzify`, `covers_domain` (sol omuz LOW, sağ omuz HIGH, kesin örtüşme).
- `FuzzyConfigError`, `ValidatedFuzzyConfig` (özel alan; `trusted_fixture()` doğrulamasız sarar, `Sender::new` tek kullanıcı), `FuzzyConfig::validate` (trapezoit sırası, kapsama, ppm/‰/log2 alan sınırı, consequent ≤1000; consequent sırası bilinçli denetlenmez).
- `FuzzyConfig::fixture()` — "KALİBRE DEĞİL": loss LOW 0–200k/300k, MED 200k–600k, HIGH ≥500k/600k ppm; bw log2(50k/100k/150k/200k B/s); jitter 100k/200k/400k/500k ppm; qd_ratio 250/500/1500/2000 ‰; ackconf 300/500/700/900 ‰; fec 0/500/800, arq 200/500/600, sr 500/750/1000.
- `DeterministicFuzzyController::decide`: R1 loss.low→(fec_zero,arq_light); R2 min(loss.high,bw.high,jitter.low)→(fec_high,arq_moderate); R3 min(loss.med,bw.med)→(fec_med,arq_med); R4 min(loss.low,jitter.high)→(fec_zero,arq_light). Σw=0 → (fec_zero,arq_light) + `no_rule`. Ağırlıklı ortalama. send_rate_gain = SR1 min(bw.low,qd.high)→sr_low, tümleyen→sr_high (loss girdi değil). Emergency = min(loss.high,bw.low,ackc.low) > 500 → `EmergencyForwardRedundancy`. compress = bw.low>500 ∨ emergency (kalıntı).
- `FusionInputSnapshot::to_inputs`, `weighted_average`, `min3`.

### fusion_policy.rs (811)
- `CalibrationVersion`, `FEATURE_SCHEMA_V1=1`, `FEATURE_SCHEMA_V2=2`, `FallbackReason` (11 varyant), `FusionObservation`, `FusionAdvice`, `FusionAction`, `FUSION_ACTION_KINDS=12`, `FusionAction::index/reason_index` (SafetyRejected ve FecUnavailable aynı kova 7), `is_arq_only`.
- `FusionSafetyConfig::default()`: tavan 800 ‰, FEC 4096 B, birleşik 8192 B, telemetri yaşı 2 s, min 2 RTT örneği.
- `FusionPolicy::disabled()` (varsayılan; her girdi `FallbackArq(PolicyDisabled)`); `calibration_v1()` (bant kenarları 90k/140k/160k/200k ppm, bölge 100–125 kB/s — kod yorumu "TÜRETİLEMEZ, DAYANAK BORCU"); `calibration_v2_deadline()` (kenarlar 50k/120k/150k/200k, bölge loss ≤250k, bw 1–80 000 B/s, bütçe/srtt ≤2500 ‰, fec_high 250 ‰).
- `loss_high_permille` (tamsayı rampa), `advise` (yalnız HIGH üyeliği × fec_high), `decide` → `FusionSafetyValidator::validate`: devre dışı → şema → alan → yaş → yakınsama → RTT örneği → bölge → bütçe ekseni → öneri → FEC aktüatör yok → tavanlarla kırpma.
- Ürün: `RuntimeConfig::fusion_policy` bench `PolicyV2Deadline` kolunda `calibration_v2_deadline`; diğer yollarda yok.
- Testler: devre dışı kesinliği, üyelik sınırları, bölge/şema/bayat/yakınsama düşüşleri, tavanlar, determinizm, taşma.

### loss.rs (1075)
- Sabitler: `PACKET_THRESHOLD=3` [STD RFC 9002], `LEDGER_MAX=4096` (spec §5 ile aynı), `TIME_THRESH 9/8` [STD], `SPURIOUS_AGE_RING=32`, `E2A_TT_GRANULARITY_MIN/MAX 1 ms/200 ms`, `E2A_ACK_DELAY_SATURATED = 256 µs × 65535`.
- `packet_threshold_lost` (largest−seq ≥ k), `time_threshold_lost` (`>=` eşik; RFC 9002 pseudo-code ile tutarlı).
- `SuspectConfidence::default()` 200/400/700/700 ‰ — "KALİBRE EDİLMEMİŞ, spec'e YAZILMADI"; `for_age` yaş/srtt basamağı.
- `LossDetector` trait; `ThresholdLossDetector`: `on_sent` (inflight BTreeMap'e ekler; `sent_window` LEDGER_MAX sınırlı), `on_ack_inner` (teyit edilen seq inflight'tan çıkar; ilan edilmiş olay Spurious'a döner, yaşı halkaya yazılır, `unlossed`'a eklenir; kanıt aralığı varsa kanıt kipi açılır, kapsam boşluğundaki teyitsiz kayıtlar eşiğe açılır; paket eşiği yalnız uygun kayıtlara), `detect_time_losses_inner` (yalnız `seq < largest_acked` ve uygun kayıt; eşik max(9/8·srtt, min_delay, cadence_floor)), `declare` (Suspected, LEDGER_MAX en-eski eviction), `ledger_tick` (W_loss dışı olay/gönderim düşer; confirm_age geçen Suspected→Confirmed), `loss_windowed_ppm` (Confirmed/pencere gönderimi), `fec_loss_risk_ppm`/`fec_loss_risk_split` (yaş ağırlıklı risk, kalıcı risk, reorder baskısı), `spurious_age_quantile`, `set_packet_threshold` (0 = varsayılan), `next_time_loss_deadline_ns`, teşhis erişimcileri.
- `loss_pre_fec_ppm()` trait yöntemi = ömür boyu lost/sent (Spurious'a dönen ilanlar düşülmez); `residual_loss_ppm()` = unrecovered/sent.
- `E2aFlags::new` doğrulamalı (pto_probes ∈{1,2}, g ∈[1,200] ms, gap_ack=true → `GapAckNotImplemented`); `E2aDiag` gözlem sayaçları.
- Ürün: `client-core/src/runtime.rs:372` `FecLossRiskConfig::bounded_v1()` ile spekülatif kol AÇIK; ağırlıklar `SuspectConfidence::default()`.
- Testler (`evidence_tests`): kanıt aralığı içi/dışı, kapsam boşluğu, zaman eşiği kanıtlı seq, kanıtsız yolun bugünkü kural, boş aralık.

### tail_recovery.rs (634)
- `TailRecoveryConfig::default()` (KAPALI; 1 ms/60 s/1 s/120 s/25 ms/cap 2 — doğrulanmamış), `validated_v1()` (10 ms/10 s/1 s/30 s/25 ms/cap 3), `rfc9002_cc_port()` (cap `u32::MAX`, `max_pto=u64::MAX`, geri çekilme tavanı 30 s kalır).
- `base_pto_ns`: `clamp(srtt + 4·rttvar + max_ack_delay, min_pto, max_pto)`; örnek yoksa `initial_pto`.
- `current_pto_ns`: `base·2^probe_index`, `max_backoff` ile kırpılır (probe_index=0'da da).
- `on_ack_progress`, `disarm`, `on_timer` (devre dışı/boş → Idle; cap dolu → Idle; son ilerlemeden bir PTO geçmeden prob yok; due ise `ProbeDue` ama cap tüketmez), `on_probe_sent` (cap tüketir, yeniden kurar), `record_probe`, sayaçlar.
- Ürün: eski motor `client-core` `ClientSession::new` → `validated_v1`; port motoru `Sender::with_cc_port` → `rfc9002_cc_port`.

### fate_estimator.rs (612)
- `FATE_WINDOW=256`, `MIN_LOSS_TRANSITIONS=8`, `MIN_NONLOSS_TRANSITIONS=32` (FS-CAL-2 v3 §3.4 PINNED).
- `reordering_guard_ns` = `TailRecovery::base_pto_ns`.
- `features_from_fate_bits`/`features_from_window`: N, lost, n00..n11 (yalnız ardışık id), Laplace p11/p01, burst_delta.
- `WindowFeatures::eligible` (N=256 ∧ geçiş eşikleri ∧ bayraklar).
- `FateEstimator`: `on_original_ack` (finalize LOSS ise late sayaç; provisional varsa spurious sayaç, SUCCESS), `insert_finalized` (FATE_WINDOW sınırlı, en düşük id atılır), `on_loss_evidence` (provisional, en erken süre korunur; `provisional` haritası sınırsız), `correct_original_delivered`, `advance`, `drop_unresolved_as_unknown`, `snapshot`.
- Ürün: `sender.rs` 3095/4012/4046/4125/4755 olayları besler; `sender.rs:5173–5203` Q18 yapı kapısı `snapshot().burst_delta_ppm`'i FEC aç/kapa kararında okur, `eligible()` çağrılmaz.

### fs_trace.rs (744) — deney düzeneği
- `TransmissionRole`, `TracePacketId`, `RedundancyPolicy`, `TraceSpec{ExactDeficit,NaturalIid,NaturalBurst}`, `ReverseSpec{Clear,Scheduled}`; FNV-1a 64 anahtarlı belirlenimli düşürme (`% 1000 < permille`).
- `TraceChannel::decide` (yalnız ilk kaynak iletimini siler; Control muaf), `decide_reverse`, karar günlüğü özetleri, `forward_decisions` (sınırsız `Vec`), şema kimlikleri.
- Testler: kollar arası özdeş açık, redundancy bağımsızlığı, determinizm, ileri/geri ayrı görünürlük.

### fs_harness.rs (1601) — deney düzeneği
- `FS_SHARD_FRAMING_BYTES=37` (26+11 elle yazılmış), `fs_sealed_len`, `domain_seed`, `a1_duplicate_target_set`, `A1EarlyEmitter`, `a1_emission_order`, `BlockTiming`, `ParityDelayClass`, `BottleneckChannel` (seri hale getirme, düşen de kaynak tüketir), `RunCfg`, `RunResult`, `run_arm` (gerçek `Sender::new()` × kanal, `dt_ns` adımlı döngü), `classify_deficit_position`, `run_config_hash`, `ParityWireModel`, `pin_c_classify`, `fs_deadline_ms` (N_RECOVERY=2, M_FIXED=50 ms), `CodecK`, `F4_K_COLD=K4`, `f4_select_k` (min_rtt <50/50–150/≥150 ms → K2/K4/K8).
- Testler: PIN-A/PIN-E, blok toplam bütçe, kapalı döngü A0/A1, kuyruk silinmesinin (son birim) eski göndericide kurtarılmadığını SABİTLER (`b2_tail_erasure_is_real_not_harness_defect`), PIN-C, §17/§18 geometri, F4.

### udp_runner.rs (1119) — deney düzeneği
- `UNIT=1200`, `K=8`, `cold_admit_new_block`, `CsBatchAdapter` (cold-start özelliği), `RunnerCfg` (+`canonical_dump`, `config_hash`, `to_policy`: leftover/p15/f15/f10/f20).
- `run_rx`: düz metin datagram `[kind][seq][unit][block][esi]`, FINISH 0xFE, CS-probe 0x19 → echo 0x1A (500 ms hız sınırı), test silmeleri (deficit/drop_repair/drop_first_sack/suppress_data_acks), 50 ms'de bir SACK 0xA0 (son 64 seq) ve RECOVERED 0xA1, CSV/işaret dosyaları.
- `run_tx`: `Sender::new()` + bütçe politikası, 40 ms'de bir uygulama birimi, 10 ms'de bir PTO tik, blok dolunca `close_fec_block_manual` (sonuç yok sayılır), geri yol çözümleme, yerleşme çıkışı, FINISH el sıkışması, CSV çıktıları.
- `emit`: TransmitKind → tel türü (0x00/0x01/0x02/0x03/0x19/0xFD); FecParity için `send_record.expect("hint")`.

## Bulgular

| fid | tür | önem | konum | davranış | otorite | düzeltme türü |
|---|---|---|---|---|---|---|
| A6-01 | UYUŞMAZ | orta | `emergency.rs:162-171` | De-escalate dwell çapası yalnız önceki de-escalate. Escalate dwell başlatmaz: Arq'tan Dup2'ye çıkıştan hemen sonra health>700 gelirse aynı anda geri inilir (ilk de-escalate kilitsiz; sonra her 1 s'de bir escalate→de-escalate çevrimi serbest). | engine §11 "de-escalate dwell = min_dwell … duplication'ın ölçüm etkisi sindirilene dek mode kilitli"; TDD §5.6 "erken iniş→re-escalate osilasyonunu keser" | kod-duzeltmesi |
| A6-02 | EKSİK | orta | `emergency.rs:205-207` | `ack_decimation()` üretilir, hiçbir ürün yolu tüketmez; acil kipte ACK seyrekleştirme yok. | TDD §5.6 "ACK: sparse cumulative / range ACK yalnız (ACK decimation)"; engine §2.1, §11, §12 | karar-gerekir (alıcı ACK davranışı; karar 2/7 ve receiver.rs kilidiyle etkileşir) |
| A6-03 | EKSİK | orta | `emergency.rs:183-201` (emisyon `sender.rs`) | Acil kip kopya/parite emisyonunda "hop-ACK almadan ardışık PN ≤ K" sayacı yok. | engine §11 AMENDMENT-9 bağı: "K = 1024 … Bugün bu yolda SAYI TAVANI YOKTUR — tavan SPEC-ONLY"; TDD §12 [AMENDMENT-9] | karar-gerekir (spec "uygulandığında davranış değişir, EVOLUTION §22 beyanları" diyor) |
| A6-04 | UYUŞMAZ | orta | `fusion.rs:316-374`, `fusion.rs:200-202`; `emergency.rs:91-97,116-126`; `arq.rs:71-79` | Ürün göndericisi (`Sender::new`, `with_cc_port`) "KALİBRE DEĞİL" fixture'larla kurulur: %20 kayıp (200 000 ppm) μ_LOW=1000 → R1 fec_zero; hysteresis 300/700 ‰ + 1 s, kopya aralığı 20 ms, reorder ρ=2 ölçümle türetilmemiş. | engine başlık "Sabit disiplini: [CAL] = netem baseline'dan kalibre edilir, UYDURULMAZ"; §15 "(a1,a2,b1,b2 netem'de kalibre; UYDURULMAZ.)"; §11 "Marjlar (X, Y, min_dwell) = [CAL]" | karar-gerekir (kalibrasyon kampanyası) |
| A6-05 | TANIMSIZ | orta | `loss.rs:94-108`, `loss.rs:423-473`, `loss.rs:383-398` | Spekülatif FEC kolu ürün çalışma zamanında açık (`runtime.rs:372` `bounded_v1`); Suspected ağırlıkları 200/400/700/700 ‰, `SPURIOUS_AGE_RING=32`, Q95 öğrenilmiş kalıcılık. Kod yorumu: "KALİBRE EDİLMEMİŞ … Spec'e YAZILMADI". | spec'te bulunamadı (aranan: Suspected güven ağırlığı, spekülatif kol, persist_min, reorder_pressure — engine §5, §8.4, §10.2) | karar-gerekir |
| A6-06 | TANIMSIZ | orta | `fate_estimator.rs:28-29,94-106`; tüketici `sender.rs:5173-5203` | Modül "observation-only: nothing in the product engine calls it" der; ürün Q18 yapı kapısı `snapshot().burst_delta_ppm`'i FEC aç/kapa kararında okur ve §3.4 `eligible()` sözleşmesini uygulamaz: geçişsiz pencerede Laplace önseli (delta=0) "IID" okur, FEC açılır. Gönderici yorumu "AÇIK BORÇ (owner kararı gerekir)". | FS-CAL-2 v3 §3.4 (modül başlığı: eşikler PINNED, uygunluk zorunlu); engine/TDD'de yapı kapısı spec'te bulunamadı (aranan: burst_delta, yapı kapısı, Q18) | karar-gerekir |
| A6-07 | UYUŞMAZ | orta | `arq.rs:154-185`; `loss.rs:328-330`; çağrı `sender.rs:4095-4096` | `ReorderTracker` hiç azalmaz (ömür boyu en büyük); port motorunda da paket eşiği `max(3, 2·recent_max_reorder)` olarak yükselir. Tek bir 500'lük sırasızlık eşiği kalıcı 1000 yapar, paket eşiği fiilen kapanır. | engine §0.3 CC-PORT YAMA (4) "kayıp ilanı RFC 9002 §6.1.2: paket eşiği 3"; §9.3 "reorder_thresh = max(K, ρ × recent_max_reorder)" ("recent" pencere tanımı yok) | karar-gerekir (iki otorite çakışıyor; CC-PORT-ESLEME'de beyan yok) |
| A6-08 | TANIMSIZ | orta | `fs_harness.rs:620`; `udp_runner.rs:639` | İki deney düzeneği eski motoru (`Sender::new()`) koşar; ürün varsayılanı port motoru (`with_cc_port`). Düzenek kanıtları ürün motorunu ölçmez; `b2_tail_erasure…` testi kuyruk kurtarmasız eski davranışı sabitler. | CLAUDE.md YAMA KAYDI 2026-11-11 "Ürün varsayılanı port (`RuntimeConfig::engine`)"; CLAUDE.md §8 "ölçülmemiş … kanıtsız iddia" | karar-gerekir (dondurulmuş deney düzenekleri) |
| A6-09 | UYUŞMAZ | düşük | `tail_recovery.rs:172-182` | Taban PTO `clamp(srtt+4·rttvar+mad, min_pto, max)`. Spec: min_pto rttvar teriminin içinde. srtt=100 ms, rttvar=0, mad=25 ms, min_pto=10 ms: spec 135 ms, kod 125 ms. RFC 9002 (kGranularity 1 ms) 126 ms. | engine §12 "PTO = srtt + max(RTTVAR_MULT × rttvar, min_pto) + max_ack_delay"; RFC 9002 §6.2.1 | kod-duzeltmesi |
| A6-10 | UYUŞMAZ | düşük | `tail_recovery.rs:89-99,186-190` | `rfc9002_cc_port` taban tavanını kaldırır ama `current_pto_ns` 30 s geri çekilme tavanını probe_index=0'da da uygular: etkin PTO tavanı 30 s. | engine §0.3 YAMA (5) "PTO tavansız"; CC-PORT-ESLEME satır 38 "etkin taban tavanı 30 s, beyanlı … U15 adayı" | karar-gerekir (beyanlı sapma) |
| A6-11 | UYUŞMAZ | düşük | `tail_recovery.rs:1-14,240` | Modül belgesi "Default OFF … byte-identical", "bounded — never unbounded retransmission" der; ürün eski motorda `validated_v1` (AÇIK), port motorunda prob sayısı tavansız. | CLAUDE.md YAMA KAYDI 2026-11-11; CC-PORT-ESLEME satır 38 | kod-duzeltmesi (belge yorumu) |
| A6-12 | ÖLÜ | düşük | `arq.rs:125-127` | `arq_aggressiveness==0` baskılama dalı erişilemez (fuzzy arq consequent'leri ≥ arq_light=200, Σw=0'da da arq_light). §9 "fec_dominant → retransmit BASKILA" hiçbir yerde uygulanmaz. | engine §9 "if fec_dominant (fuzzy fec yüksek ∧ ackconf düşük): retransmit BASKILA"; §16.2 TUR-4 satırı "arq.plan()'ın ==0 fec-dominant baskılama-dalı (§9) yapısal-erişilmez — sözleşme-notu" | karar-gerekir (spec iç çelişkisi) |
| A6-13 | EKSİK | düşük | `fusion.rs:503-516,537-540` | R4 "reorder-tolerant (RTO↑, fast-rtx↓)" yerine arq_light; SR1 kuralının "fec CAP" kolu yok (yalnız hız kazancı). | TDD §5.3; engine §8.4 | karar-gerekir |
| A6-14 | TANIMSIZ | düşük | `fusion.rs:544,551` | Acil kip kesin eşiği `min3 > 500 ‰`, compress `bw.low > 500 ‰`. Eşik türetilmemiş. | spec'te bulunamadı (aranan: kip seçimi için kesinleştirme eşiği — §8.4, §11, §15) | karar-gerekir |
| A6-15 | UYUŞMAZ | düşük | `fusion.rs:542-548` | Acil kip tetiği `loss HIGH ∧ bw LOW ∧ ackconf LOW` (§5.6 ile aynı). TDD §5.3 tablosu aynı kipi `jitter HIGH` ile tanımlar: otorite kendi içinde çelişik. | TDD §5.3 "loss HIGH, bw LOW, jitter HIGH → mode = EMERGENCY"; TDD §5.6, engine §8.4/§11 "ack_confidence LOW" | karar-gerekir |
| A6-16 | ÖLÜ | düşük | `fusion.rs:235-236,371` | `sr_med` hiçbir kuralda kullanılmaz ("ileride MED rate kuralı için"). `compress` kalıntı (AMENDMENT-4 ile uyumlu, bulgu değil). | engine §8.4 AMENDMENT-4; TDD §8 | kod-duzeltmesi (alanı kalıntı olarak işaretle) |
| A6-17 | TANIMSIZ | düşük | `loss.rs:681-764` | ACK-bitmap kanıt kipi ve kapsam boşluğu kuralı (aralık dışı kaydı bilinmiyor say; boşluktakini eşiğe aç). 0x15 çerçeve kodu §1.1 kayıt tablosunda yok. Sahip kararı 7171705736 birleşik kabule "bitmap"i dahil eder; engine başlık YAMA 2026-11-21 "0x15, `rx_span`" diye anar. | engine §1.1 (0x15 yok); §2 | karar-gerekir (kayıt tablosu yaması) |
| A6-18 | UYUŞMAZ | düşük | `loss.rs:155-156,667-669` | `LossDetector::loss_pre_fec_ppm()` ömür boyu lost/sent döner (Spurious'a dönen ilanlar düşülmez); spec `loss_pre_fec` = pencereli Confirmed oranı. Ürün fuzzy yolu `loss_windowed_ppm` kullanır; yöntem yalnız testlerde. | engine §5 "loss_pre_fec (fuzzy girdisi) = loss_windowed_ppm … ömür-boyu sayaç yalnız diag" | kod-duzeltmesi (adı/belgeyi diag olarak düzelt) |
| A6-19 | TANIMSIZ | düşük | `loss.rs:302-305,636-640` | `ledger_evicted_total` iki ayrı tahliyeyi (olay defteri ve gönderim paydası) tek sayaçta toplar; denetim sayacı hangi sınırın dolduğunu ayırt etmez. | engine §5 "LEDGER_MAX=4096, en-eski-önce eviction + audit" | kod-duzeltmesi |
| A6-20 | DOĞRULANAMADI | düşük | `loss.rs:219,624-633` | `inflight` BTreeMap içte sınırsız; sınır çağıranın (uçuş/havuz) sorumluluğunda. | engine §14 "tx_unacked_pool ≤ TX_POOL_MAX … unbounded alloc YOK"; CLAUDE.md §9 | karar-gerekir |
| A6-21 | DOĞRULANAMADI | düşük | `loss.rs:836-837` | `g` üst sınırı 200 ms "YOL PLANI v2 §5.1" atfıyla; belge depoda bulunamadı. | spec'te bulunamadı (aranan: tt_granularity üst sınırı) | karar-gerekir |
| A6-22 | TANIMSIZ | düşük | `fate_estimator.rs:173,228-234` | `provisional` haritası sınırsız (finalize haritası FATE_WINDOW ile sınırlı). Ürün olayları beslediği için sınır yalnız kayıp hızı × koruma süresi. | CLAUDE.md §9 (SR-AUDIT-001 bounded buffer); engine §14 | kod-duzeltmesi |
| A6-23 | ÖLÜ | düşük | `fate_estimator.rs:295-297` | `drop_unresolved_as_unknown` yalnız modül testinde çağrılır. | yok | kod-duzeltmesi (yalnız test/analiz olarak işaretle) |
| A6-24 | ÖLÜ | düşük | `fusion_policy.rs:272-311` | `calibration_v1` bant kenarları kod yorumunca "TÜRETİLEMEZ"; ürün kullanmaz, yalnız testler. Kaldırma/işaretleme yok. | CLAUDE.md §6 "Deprecated içerik → appendix'e taşı + MUST NOT IMPLEMENT" | kod-duzeltmesi |
| A6-25 | TANIMSIZ | düşük | `fusion_policy.rs:188-200,330-368` | FS-FUSION-CAL-1 politikası, güvenlik tavanları (800 ‰/4096/8192 B/2 s/2 örnek) ve v2 bölgesi (tek kampanya, tek nokta) spec'te yok; bench `PolicyV2Deadline` kolunda açık. | spec'te bulunamadı (aranan: kalibrasyon sürümü, deadline/srtt ekseni — engine §8.4/§8.5, TDD §5) | karar-gerekir |
| A6-26 | ÖLÜ | düşük | `fusion_policy.rs:76,148-161,219-228` | `FallbackReason::SafetyRejected` hiç üretilmez; histogramda FecUnavailable ile aynı kova (7); `LossBoundary.low_full_ppm/low_zero_ppm` kararlarda okunmaz. | yok | kod-duzeltmesi |
| A6-27 | ÖLÜ | düşük | `fec.rs:1-109` | 0x20 FEC_SHARD codec'i deprecated, yalnız testte. Engine §1.1 ve §10.1 0x20'yi hâlâ normatif listeler, üstünleme notu yok. `decode` `shard_index < k+r` denetlemez. | engine §1.1, §10.1; CLAUDE.md §6 | karar-gerekir (spec üstünleme notu) |
| A6-28 | UYUŞMAZ | düşük | `lib.rs:11,16,35-36` | Belge yorumu "Hepsi INV-RATE-1 (loss ⟂ rate)" (2026-11-11 metninden önceki), "fate_estimator no product caller" (yanlış), "Sıradaki: bench KamaTransport'u motora bağla" (bayat). | CLAUDE.md §4 INV-RATE-1 2026-11-11 metni | kod-duzeltmesi (belge yorumu) |
| A6-29 | UYUŞMAZ | düşük | `lib.rs:18-21`; `fec.rs` | ACK/FEC çerçeveleri ccr-codec yerine crate içinde (`read`) kodlanır; lib.rs bunu "ertelenmiş" diye anar. | ADR-05 "`ccr-codec` crate tek kaynak" | karar-gerekir |
| A6-30 | UYUŞMAZ | düşük | `udp_runner.rs:490-499` | `recovered.drain(..).take(100)` bütün kuyruğu boşaltır, yalnız ilk 100'ü yollar; 100'ün üstündeki RECOVERED kayıtları sessizce kaybolur, gönderici o birimler için ARQ'yu iptal etmez. | CLAUDE.md §9 (sessiz kayıp yok); spec'te harness için hüküm bulunamadı | kod-duzeltmesi |
| A6-31 | UYUŞMAZ | düşük | `udp_runner.rs:1089`; `udp_runner.rs:760` | `send_record.expect("hint")` erişilebilir panik; `close_fec_block_manual` sonucu `let _` ile yutulur (sessiz hata). | CLAUDE.md §6/§9 (typed error, panik yok) | kod-duzeltmesi |
| A6-32 | TANIMSIZ | düşük | `udp_runner.rs:120-125,163-172,253-255,401,447,711` | Düzenek tel biçimi (düz metin kind/seq/block/esi; 0xA0/0xA1/0xFE/0xFD), 40 ms uygulama kadansı, 50 ms SACK, 1500 ms ilk-uçuş boşluğu, 105 s / 8 s boşta çıkış, `f10/f15/f20` aynı parametrelere eşlenir. | spec'te bulunamadı (harness; engine §17 karşılaştırma düzeneği sabit tanımlamaz) | karar-gerekir |
| A6-33 | TANIMSIZ | düşük | `fs_harness.rs:825-851` | `run_config_hash` `horizon_ns`'i içermez: yalnız ufku farklı iki koşu aynı kimliği alır (B2 testi ufku ×3 yapar). | düzenek kendi sözleşmesi ("binary/arg provenance") | kod-duzeltmesi |
| A6-34 | TANIMSIZ | düşük | `fs_harness.rs:51,508-522,601-611,659-660` | `FS_SHARD_FRAMING_BYTES=37` noise-wire sabitlerinden türetilmez (elle 26+11); `unit as u16` 65 535 üstünde takma ad üretir; `dt_ns=0` döngüyü sonsuz yapar. `A1EarlyEmitter`/`a1_emission_order` yalnız modül testinde. | CLAUDE.md §8 "ölçülmemiş byte tahmini"; §9 | kod-duzeltmesi |
| A6-35 | TANIMSIZ | düşük | `fs_trace.rs:227,295-302`; `emergency.rs:167` | `forward_decisions` sınırsız `Vec` (düzenek); `emergency` dwell kontrolünde `t + dur_ns(dwell)` denetimsiz toplama (u64 sınırında debug panik). | CLAUDE.md §9 | kod-duzeltmesi |

### Uyumlu davranış sayısı (dosya başına)
- lib.rs: 2 (forbid(unsafe), özellik kapısı).
- error.rs: 7 (typed varyantlar; MAX_ACK_RANGES decode zorlaması).
- sim.rs: 3 (belirlenimli script, sıralı teslim, RNG yok — ADR-17).
- fec.rs: 4 (sabit genişlik BE, k+r ≤255, PARSER-02 sınırlı okuma, opak blob — INV-ENGINE-SOC).
- emergency.rs: 6 (escalate anında, Schmitt bandı, tiny→D kopya aralıklı, segment→minimal parite, Dup2/Dup3 100/200 ‰ AMENDMENT-3, seviye sırası).
- arq.rs: 6 (deadline önceliği, süresi geçen düşmez, cap politikası iki seçenekli §9.1, reorder formülü §9.3, port kurucusunda tavansız yeniden iletim CC-PORT-ESLEME beyanlı, 255 doygunluk düzeltmesi).
- fusion.rs: 10 (trapezoit, Mamdani min, ağırlıklı ortalama, R1–R3 kuralları, send_rate_gain'e loss girmez — INV-RATE-1, compress kalıntı AMENDMENT-4, no_rule §16.2, log2 alanı §16.2, config doğrulama, determinizm).
- fusion_policy.rs: 6 (varsayılan kapalı, yalnız aşağı kırpma, alan dışı girdide düşüş, sürüm uyuşmazlığında düşüş, FEC aktüatörü yoksa düşüş, tamsayı determinizm).
- loss.rs: 11 (paket eşiği 3, zaman eşiği 9/8, later-ACK ön şartı §5 TUR-1a, cadence tabanı TUR-1b, olay defteri Suspected/Confirmed/Spurious §5 TUR-2a, LEDGER_MAX 4096, pencereli loss, residual_loss, un-loss, E2a typed stub, kayıp hızı ölçmez — INV-RATE-1).
- tail_recovery.rs: 7 (örneksizken initial PTO/Karn, üstel geri çekilme, ACK ilerlemesinde sıfırlama, ilerlemeden bir PTO bekleme, due≠sent, validated_v1 sabitlenmesi, taşmasız aritmetik).
- fate_estimator.rs: 7 (Laplace formülleri, ardışık id geçişi, koruma = ürün base_pto, finalize sonrası değişmezlik, tek giriş, pencere 256 sınırlı, determinizm).
- fs_trace.rs: 5 (ilk iletim silmesi, Control muaf, belirlenimli FNV, ileri/geri ayrı özet, şema kimliği).
- fs_harness.rs: 6 (blok toplam bütçe, düşen paket kaynak tüketir, mühürlü bayt tek alan, kapalı döngü, PIN-C sınıflaması, §18 geometri).
- udp_runner.rs: 4 (özellik kapılı, kök gerektirmez, RECOVERED ile ARQ iptali, soğuk blok sınırı bounds-before-alloc).

## Okunan otorite bölümleri
- `specs/adaptive-transport-engine-spec.md`: başlık YAMA kayıtları (1–28), §0–§2.2 (31–250), §5–§6 (462–604), §8.4–§16 başı (700–1039), §16.2 TUR-4 satırları (1174–1175).
- `specs/transport-layer-tdd.md`: YAMA kayıtları (7–18), §5.1–§9 (596–755).
- `specs/tech-stack-adr.md`: ADR-05, ADR-06 (122–140), ADR-16, ADR-17, ADR-18 başı (247–280).
- `specs/fec-core-tdd-v3.0.md`: profil/sealed_len satırları (grep).
- `docs/design/CC-PORT-ESLEME.md`: eşleme tablosu ve U1–U13 (30–70).
- `docs/decisions/KARAR-KAYDI-2026-11.md`, `SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` (ilgili konu taraması).
- Sahip kararları (#175 yorumları): 7171705736 (dokuz karar; karar 3 PTT/metin dondurma; birleşik kabulde "bitmap"), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (receiver.rs alarm aralığı Web'de). Bu kararların hiçbiri bu dosyalardaki davranışı doğrudan kilitlemiyor; A6-02 alıcı tarafına dokunduğu için kilitli alanla etkileşir.
- `CLAUDE.md` §4 (INV-RATE-1 2026-11-11 + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §5, §6, §8, §9, YAMA KAYDI.
