# A3-sender-testler — `sender.rs` test modülleri denetimi

Taban: `125b9afa` (ürün hattı; aday `1a4f9efc` içinde). Salt okuma; kaynak, test ve spec değiştirilmedi.

## Kapsam

| dosya | satır aralığı | modül | test sayısı |
|---|---|---|---|
| `crates/adaptive-transport/src/sender.rs` | 9600–12767 | `mod tests` (gövde) | 80 |
| aynı | 12768–13392 | `tests::e2a_goster` | 17 |
| aynı | 13394–16220 | `tests::k4g05br_teyit` | 32 |
| aynı | 16222–16285 | `profile_pacing_cap_tests` | 2 |
| aynı | 16287–16359 | `fate_key_tests` | 2 |
| aynı | 16361–16722 | `feedback_starved_tests` | 7 |

Okunan kod satırı: 9601–16722 = **7 122 satır**, hepsi Read aracıyla parça parça (atlama yok). Bağlam için ayrıca okunan
üretim satırları: `sender.rs` 2700–2760 (`with_cc_port`, `cc_on`, `fate_key`), 1817–1853 (`FeedbackStarvedRule`,
`DupRec`, `DUP_SRC_*`), 1183–1191 (`StartupPacingConfig::default`), 5306–5320 (Drain çıkış hedefi), sabit tanımları
(`BS_FIRES_NS` 1120, `BS_TX_HARD_BUDGET` 1124, `CONFIRMED_LOGICAL_CAP` 1362, `CONFIRMED_UNITS_CAP` 1374,
`K_CONFIRMED_RELEASE` 1406, `SENT_LOGICAL_CAP` 1416, `STARTUP_PLATEAU_ROUNDS` 1909, `K_TOMB` 1946, `K_ARQ_SHARE` 1972,
`K_CONF_EAI` 1982, `TOMBSTONE_MAX` 1990).

Toplam 140 test. Ortak düzenekler: `unit(id)` = 1200 B, `small(id)` = 100 B opak birim; `MS` = 1 ms; `ack(seqs)`
= `ack_delay` 0 SACK. Sahte saat; RNG yalnız düzeneklerde (Knuth MMIX LCG, kanal kaderi), motor girdisinde değil.

## Davranış envanteri

### A. `mod tests` gövdesi (9601–12767)

**Yardımcılar.** `warmup_btlbw(map_acked)`, `warmup_recovered(fires)`, `warmup_data_register(fires)`: aynı olay
dizisini iki kolda sürüp (BtlBw, sayaçlar) döndürür; N8 bit-özdeşlik kıyasının tabanı. `tail_cfg_on()`: kuyruk
kurtarması açık yapılandırma (min_pto 1 ms, max 1 s, initial 1 ms, backoff tavanı 100 s, max_ack_delay 0,
`max_tail_probes` 2) — test sabitleri, türetilmemiş. `floor_on()`, `ramping()`: DR-RATE-RAMP-2 başlangıç tabanı
açık Startup rampası. `bs_on()`, `bs_stalled()`, `probes()`: DR-CS-3 bootstrap merdiveni, hiç ACK almayan
PreConverge akışı (384 × 101 B). `E1Sim`: tek darboğaz (FIFO seri hız, sabit tek yön gecikmesi, anında tek-seq ACK,
isteğe bağlı tampon `buf_ns` ve tıkanıklık düşmesi), 1 ms `Pto` + 200 ms `RtpropProbe` pompası; her adımın
(an, durum, pending, çıktı, `arq_share_tick`, tx_queue) kaydı. `e1_startup_fixture`, `e1_complete_round`: Startup
kurulumu ve tur işareti taklidi.

**Cold-start / durum makinesi (§3, §3.3).**
- `idle_to_preconverge_then_startup_on_first_ack`: ilk AppData → PreConverge, 1 DATA, inflight 1200; ilk SACK +
  iş → Startup.
- `preconverge_clean_no_spurious_recovery`: temiz cold-start'ta Duplicate/Retransmit yok; ilk SACK PreConverge'i bitirir.
- `drains_to_idle_when_quiescent`, `ack_frees_inflight_and_updates_rtt`: ACK uçuşu boşaltır, srtt kurulur, iş yoksa Idle.
- `startup_reentry_resets_plateau_counter`: Idle→Startup taze girişte plato sayacı/round bayrağı sıfır, marker =
  giriş anı `next_seq` (1).
- `plateau_round_is_supply_independent`: marker öncesi ACK olayları plato sayacını ilerletmez.
- `startup_exit_enters_drain_then_steady_on_bdp`: plato → Drain; uçuş > hedef iken Drain sürer; ACK sonrası Steady.
- `drain_rtprop_contract_dwell_and_exit_invariant`: rtprop yaşı < W/2 iken giriş yok; ≥ W/2 (12 s) giriş; aynı olayda
  çıkış yok; min_dwell ∧ qualifying → Steady; qualifying'siz 3×epoch → Steady.
- `probe_up_contract_entry_success_and_gates`: periyot (4×epoch) dolmadan giriş yok; dolunca ProbeUp, aynı olayda çıkış
  yok; donuk snapshot küçültülünce success → Steady, `diag_probe` = (1,0,0,0).
- `probe_up_timeout_and_preempt_bookkeeping`: timeout ayrı sayılır (0,0,1,0); pending doluşu ProbeUp'ı RecoverArq'la
  keser, preempt 4. tür; giriş-öncesi kayıp probe'a yazılmaz.

**Kayıp tespiti, tombstone, ARQ (§5, §9).**
- `low_loss_with_healthy_acks_retransmits`, `retransmit_references_same_unit_id`: paket eşiğiyle (3 üst ACK) kayıp →
  RecoverArq, aynı `unit_id` yeni seq'le yeniden iletilir.
- `tail_loss_liveness_without_loss_classification`: saf sessizlik kayıp sayacını artırmaz (0,0,0; ppm 0), canlılık
  RecoverFwd Duplicate'inden gelir.
- `clean_pair_flow_no_time_threshold_loss`: geç ama tam SACK'te zaman eşiği kayıp yazmaz.
- `unloss_cancels_pending_retransmit_and_raises_tolerance`: geç ACK pending'i iptal eder, Spurious windowed ppm'e
  girmez, `recent_max_reorder` ≥ 4.
- `tombstone_recovers_bw_and_rtt_from_late_ack`: kol-1 geç ACK → BW örneği + RTT örneği, plato round'u beslenir.
- `karn_superseded_late_ack_credits_without_samples`: kol-2 → RTT yok, hız örneği yok, delivered kredisi var,
  ambiguous +1, `note_spurious` mesafesi ≥ 9.
- `karn_both_transmissions_acked_wire_byte_semantics`: retx kendi ACK'inden sonra orijinal geç ACK: kredili, örneksiz.
- `tombstone_expiry_yields_no_sample`: TTL sonrası prune, çok geç ACK örneksiz; prune sonrası `diag_pif` = gerçek uçuş.
- `arq_share_emits_retx_under_token_starvation`: DATA birikimi doluyken her `K_ARQ_SHARE` (4) iznide retx öne geçer.
- `ack_confidence_low_enters_recover_fwd_and_duplicates`: ACK kesilince (≈14 s+) RecoverFwd + ACK-bağımsız Duplicate +
  emergency escalate.
- `ack_confidence_eai_floor_prevents_false_low`: eşik `max(3×srtt, K_CONF_EAI(2)×EAI)`; EAI içinde 1000‰, 4×EAI'de < 1000.

**Kuyruk probu (DR-ARQ-PRODUCT-TAIL-1).**
- `tail_probe_off_never_probes`: varsayılan kapalı → 0 prob.
- `tail_probe_on_no_loss_zero_probe`: her şey ACK'lenince 0 prob, 0 bayt.
- `tail_probe_mid_stream_loss_no_same_tick_double_plan`: bir tikte aynı birim iki kez retx edilmez; prob ≤ 2.
- `tail_probe_terminal_tail_bounded_recovery`: kapalıda saf kuyruk 0 retx; açıkta 1..=2 prob, her biri Retransmit,
  spurious 0.
- `tail_probe_token_unavailable_cap_not_consumed`: **PreConverge'de** (srtt yok) prob vadesi gelir, jeton yok → sayaç
  tüketilmez, `tail_probe_pending` dolu.
- `tail_probe_ack_race_cancels_without_consuming_cap`: PreConverge'de bekleyen prob ACK yarışında iptal.
- `tail_probe_ack_is_karn_excluded_from_rtt`: eski motorda Retransmit kökenli prob ACK'i srtt'yi değiştirmez.
- `tail_probe_determinism_two_senders`: aynı olay dizisi → bayt-özdeş iz (ADR-17).

**INV-RATE-1 ve başlangıç tabanı (DR-RATE-RAMP-2).**
- `inv_rate_1_loss_declaration_preserves_phase_gain`: taban KAPALI kolda kayıp ilanı sonrası hız = 2,885×BtlBw (tam
  eşitlik), recovery çıkışı Startup tabanına.
- `inv_rate_1_active_floor_loss_does_not_reduce_rate`: taban AÇIK; hız = `max(STARTUP×btlbw, min(floor, cwnd/srtt))`.
- `startup_floor_arms_only_on_preconverge_exit`, `_not_armed_or_raised_by_loss`, `_never_exceeds_cwnd_over_srtt`,
  `_writes_no_synthetic_btlbw_samples`, `_release_is_deterministic_and_permanent`: taban yalnız PreConverge çıkışında
  kurulur, kayıpla yükselmez, BtlBw filtresine örnek yazmaz, plato/Drain'de kalıcı bırakılır.

**Bootstrap RTT probu (DR-CS-3).** `bootstrap_off_is_inert`, `_ladder_is_edge_only_and_bounded` (tek kurulum, adım başına
≤ 1 prob, tekil `probe_seq`), `_sender_budget_never_exceeds_200_bytes` (≤ 200 B, ≤ 4×44 B),
`_probe_is_additive_not_paced` (DATA önekini kaydırmaz; yalnız `experimental-cold-start` kapalı derlemede),
`_unsolicited_echo_yields_no_rtt`, `_wrong_generation_echo_yields_no_rtt`, `_valid_echo_yields_rtt_from_local_timestamps_only`
(RTT = yerel varış − yerel gönderim = 1 s; ilk geçerli örnek merdiveni iptal eder), `_duplicate_echo_yields_at_most_one_sample`,
`_no_probe_after_success`, `_does_not_relax_karn_or_touch_arq_timing` (yalnız `diag_loss` eşitliği).

**DR-S5 mantıksal defter, RECOVERED (N8).**
- `dr_s5_3_mapping_does_not_change_btlbw`, `dr_s5_6_n8_data_register_does_not_change_btlbw`,
  `dr_s5_4_n8_recovered_does_not_change_btlbw`: send-record eşlemesi / DATA kaydı / RECOVERED BtlBw'ye bit-sızmaz.
- `dr_s5_3_confirmed_logical_take` (WireAck kaynaklı defter, drenaj), `dr_s5_3_confirmed_logical_bounded`
  (`CONFIRMED_LOGICAL_CAP` 4096, en eski düşer), `dr_s5_6_register_bounded_evict_miss` (`SENT_LOGICAL_CAP` 16384,
  evict sayacı, miss artefakt sayacı).
- `dr_s5_4_recovered_cancels_and_ledger`: RECOVERED emisyon üretmez, pending retx iptal, dup kaydı serbest, defter
  `ConfirmSource::Recovered`; bilinmeyen birim NO-OP.

**FEC eyleyici otoritesi (DR-1a, Commit-D…N).**
- `dr_1a_ratio_to_r_golden_k8`, `_property_exhaustive`: `r = (ratio×k + 500)/1000` (yarım-yukarı), monoton, `r_max` kırpması,
  k ∈ {1..8, 255}, taşma yok.
- `dr_1a_authority_api_and_manual_reject`: varsayılan `Manual{false}`; Controller kipinde manuel toggle tipli
  `NotInManualMode` + sayaç.
- `dr_1a_latch_guards_initial_mismatch_norule_emergency`: karar yok → 0 (`NoEligibleDecision`); bayat olay indeksi → 0
  (`DecisionEventMismatch`); no_rule → 0; emergency → computed görünür, planned 0; normal → planned = clamped; Manual'da 0.
- `dr_1a_ordering_fuzzy_before_emission_and_recovered_cannot_close`: fuzzy emisyondan önce; RecoveredControl fuzzy
  koşturmaz, emisyon üretmez.
- `dr_1a_counter_saturation_boundary`: sayaçlar doygun (wrap yok).
- `dr_1a_last_latch_full_overwrite_no_carryover`, `commit_k_foreign_authority_and_manual_close`,
  `commit_k_enqueue_permission_gate_and_generation`, `commit_n_token_denial_attribution_and_semantics`,
  `commit_n_interleaved_denial_overwrite_and_saturation`: yabancı tanık reddi, izin kapısı, monoton generation, jeton
  reddi atfı (blok/olay/generation), RECOVERED iptali red sayılmaz, doygunluk.
- `fusion_snapshot_equals_controller_input`: snapshot → `decide` = `last_decision` (bit-özdeş).
- `dr_1e_default_constructor_equivalence`: `new()` ≡ `try_with_fuzzy_config(fixture)` (karar dizisi, hız, kayıp, no_rule).

**E1 gösterim bayrakları (KEEP-EXPERIMENTAL, varsayılan kapalı).**
`e1_default_flags_are_off_and_setter_round_trips`; `e1_pretest1_retx_continues_during_probe_up` (kapalı: kayıp ProbeUp'ı
keser; açık `e1_probe_keep`: ProbeUp'ta retx); `e1_pretest2_probe_keep_alone_p0_wire_identical` (p = 0'da tel dizisi
bit-aynı); `e1_retx_first_arq_first_on_every_grant`; `e1_c_startup_cwnd_gain_shared_by_rate_ceiling_and_floor`
(Startup'ta 2,885 / diğer 2); `e1_d_app_limited_round_is_not_counted`, `e1_d_qd_brake_evaluates_marked_round_under_standing_queue`
(qd ≥ 250‰ freni), `e1_d_ptt_cadence_does_not_count_as_plateau`; `e1_drainrt_gate_requires_standing_queue`;
`e1_e_dar_preconverge_plans_no_retx`; `e1_e_dar_emergency_still_preempts_probe_up`;
`e1_v2_iid30_backlog_probe_up_does_not_close_successfully`, `e1_v2_congestion_loss_probe_aborts_on_qd_and_queue_bounded`
(srtt/rtprop ≤ 2).

### B. `e2a_goster` (12768–13392)

E2a bayrakları (`tt_rfc`, `g`, `rtt_rule_v11`, `iw_idle_hold`, `pto_probes`, `gap_ack`); `with_cc_port()` hepsini
(gap_ack hariç) açar (`sender.rs:2723`).
- `e2a_flags_default_is_today_and_new_validates`: varsayılan kapalı; `pto_probes` ∈ {1,2}, `g` ∈ [1 ms, 200 ms];
  `gap_ack=true` → tipli `GapAckNotImplemented` (todo!/panik yok).
- m6: `e2a_m6_idle_first_timer_keeps_initial_window` (kapalı: Idle'da ilk tik krediyi 2400→1200 kırpar, 4 birim; açık:
  8 birim), `e2a_m6_does_not_touch_measured_idle`.
- m1: `e2a_m1_time_threshold_rfc_vs_cadence_floor` (açık: eşik `max(9/8·max(srtt,latest), g)`, TTL = `K_TOMB`×yeni T_loss;
  kapalı: `K_c×EAI` tabanı ≥ 3,145 s), `_pure_silence_declares_no_time_loss_in_either_arm`, `_granularity_floors_the_threshold`.
- PTO 2: `e2a_pto2_two_units_two_distinct_probes_per_round`, `_single_unit_probed_twice_with_fresh_seqs`,
  `e2a_pto_probes_one_is_todays_decision_sequence`, `e2a_pto2_second_probe_waits_for_token_then_emits` (eski motor,
  ACK'siz PreConverge, prob ≥ 60 s), `_second_probe_cancelled_when_target_acked`, `_waiting_second_probe_is_a_wakeup_source`.
- m8 (v1.1 RTT kuralı): (a) çerçevenin en büyüğü zaten onaylıysa örnek yok, küçüğe düşüş yok; (b) açıkta Retransmit /
  Duplicate kökenli ilk ACK örnek verir (Duplicate ACK'i PreConverge'den çıkarır; kapalıda "PC tuzağı"); (d) doygun
  `ack_delay` → örnek yok; (e) guard reddi ∧ `ack_delay ≥ rtprop` → örnek yok, `ack_delay < rtprop` → RAW örnek.

### C. `k4g05br_teyit` (13394–16220)

Düzenek `Rig` (olay sürücüsü, isteğe bağlı drenajsız ikiz; her olayda karar ve `next_wakeup_ns` eşitliği),
`spend_tokens`, `seq_of`, `pump_until`, `warm`; `evict_tombstone` (tavan döngüsünün tek adımını beyaz kutu uygular);
`kol1_setup`, `kol2_setup(_on)`, `row7_setup_on`; `engines()` (legacy, cc_port), `tail_engines()` (legacy+`validated_v1`,
cc_port), `ghost_engines()`; `has_decision_work`, `marked_copies`, `decisions_since`, `engine_owns`. Sabitler test
düzeneğidir (`UID` 1000, pompa adımları, ufuklar, `TRACE_*`, `GHOST_*`); motor sabiti değildir.
- Teyit çıktısı (`take_confirmed_units`): birim kimliği taşır (seq/ESI değil), olay içinde tekil + artan kimlik; uçuş
  (Data/Retransmit/Duplicate), tombstone kol-1/kol-2, un-loss eşleşmesi canlı kayıttır; budanmış/tahliye edilmiş
  tombstone, Duplicate kökenli kayıp, bilinmeyen seq, yinelenen ACK teyit etmez; parite ACK'i ESI adaşını teyit
  etmez ve onun kopyasını iptal etmez; RECOVERED listelenen birimleri teyit eder.
- `k4g05br_confirmed_units_call_output_never_dropped`: tek çağrı tavandan (`CONFIRMED_UNITS_CAP` 4096) fazla teyit
  üretebilir, hiçbiri düşmez; tavan birikime sonraki çağrının girişinde uygulanır, taşma sayacı.
- Karar-nötrlük: `neutrality_trace` × 3 tohum × 2 motor (8000 adım, %15 düşme, %15 geç, kesinti sıçraması, iki sentetik
  büyük RECOVERED): drenajlı ve drenajsız motor her adımda aynı karar ve uyanma; son durum yüzeyleri özdeş.
- Teyit sonrası iş iptali ("[sahip kararı 2026-11-11]"): pending_lost iptal, dup_queue iptal, tx_queue'dan çıkarma,
  uçuştaki kopya `confirmed_release_ns` ile kararsız işaretlenir (uçuş baytı aynı), sonra kayıp ilanında yeniden
  kuyruksuz (port: tam tıkanıklık olayı), sessizlikte bırakma anında (`K_CONFIRMED_RELEASE` = `K_TOMB` × T_loss) kayıp
  ilansız düşer; kuyruk probu kararsız kopyayı atlar/iptal eder; ikinci prob yuvası boşaltılır; bırakma anı uyanma
  kaynağıdır; işaretli Data kaydının spurious kaybı reorder'ı besler; teyitli birime hiçbir karar çıkmaz.
- Hayalet ölçümü `k4g05br_ghost_after_skipped_stale_decision` (3 motor × "atla"/"başlık" kolu, 120 s): teyit sonrası
  hedefe 0 karar, birim ufukta bırakılır, Idle, kuyruklar ≤ 4096, BBR durumu ve diğer birimlerin gecikmesi kollar arasında aynı.
- CC-PORT uyarlama testleri: `k4g05br_tail_probe_retx_count_saturates` (u8 doygun; port hedefi uçuşta kalır — U10),
  `u14_no_pto_probe_before_first_rtt_sample_on_port` (port: örneksizken PTO probu yok; eski+`validated_v1`: prob çıkar),
  `u14_pto_probe_resumes_when_no_ladder_entry_serves_the_oldest_unit`, `u16_duplicate_copy_does_not_suppress_arq_requeue_on_port`,
  `u16_pto_probe_targets_smallest_unit_id_not_smallest_seq`, `u17_pto_probe_carries_queued_unit_with_smaller_id_than_target`,
  `u17_pto_probe_keeps_u16_target_when_queued_ids_are_larger`, `arq_retransmit_at_saturated_retx_count_does_not_overflow`
  (port: 255'teki kayıt yeniden iletilir, "tavan yok"), `k4g05br_tail_probe_never_targets_parity`.

### D. `profile_pacing_cap_tests` (16222–16285)
`profile_cap_paces_the_enlarged_initial_window`: IW 67 500 B; tavansız 200 ms'de 40 birim, pacer > 500 kB/s
(1,25·pencere/100 ms); tavanlı (12 200 B/s) 2–5 birim, pacer = tavan. `profile_cap_none_is_the_default_path`:
`None` ve `Some(0)` bugünkü yolla aynı.

### E. `fate_key_tests` (16287–16359)
Port motoru kader penceresini gönderim sırasıyla (son 256 orijinal), eski motor kimlik sırasıyla (en büyük 256 kimlik)
tutar; şerit etiketli kimlikte (`3<<62` dosya, `2<<62` metin) ayrışır, şeritsizde aynı.

### F. `feedback_starved_tests` (16361–16722)
`forward_copy_line_scale_keeps_the_line_share` (kopya sayısı beyan altında hatla orantılı, en yakın tamsayı,
`min_copies` alt sınırı); `forward_copy_rule_copies_only_units_below_the_bound` (kimlik < sınır → ek `copies` kopya;
`copies == 0` kural kurmaz); `rule_units_leave_before_queued_bulk` (öncelikli kimlik kuyruğun önüne geçer);
`copies_are_planned_only_for_unconfirmed_inflight_units`; `due_copies_alternate_between_units_on_the_starved_profile`
(aç profilde vade sırası 7,8,9,7,8,9; kuralsız kuyruk sırası); `feedback_starved_rule_sends_up_to_the_floor_without_acks`
(ACK'siz 100–130 kB ≈ tavan × (10 s + 100 ms), hız ≤ tavan; kuralsız < 30 kB); `parity_does_not_count_as_priority_for_the_gap_budget`
(LineGuard boşluk bütçesi: paritede kurulur (1,0), öncelikli veride kurulmaz (0,1)).

### Uyumlu (sayım)

140 testin sabitlediği davranışların **~95'i** otoriteyle uyumlu bulundu. Önemlileri: §3.3 PreConverge giriş/çıkışı ve
spurious kurtarma yasağı; §5 TUR-2a un-loss ve TUR-2b kol-1/kol-2 tombstone kuralları (RTT yok, örnek yok, kredi var,
`note_spurious`); §5(b) kanıt şartı (saf sessizlik kayıp yazmaz); §10.2 `r = round(ratio×k)` ve §8.5 profil kırpması;
N8 (RECOVERED/defter BtlBw'ye sızmaz); SR-AUDIT-001 sınırlı defterler (4096 / 16384, eviction + sayaç); §0.3 INV-RATE-1
faz kazancı kilit-golden'ı (taban kapalı kolda); §16.1 ProbeUp kapıları ve `K_ARQ_SHARE` = 4; ADR-17 determinizm (iki
gönderici bayt-özdeş, karar-nötrlük izi); §9 tipli hata (`GapAckNotImplemented`, `NotInManualMode`, `ForeignAuthority`);
INV-ENGINE-SOC/INV-FEC-1 (testler yalnız opak `id`+`size` ve parite metadata'sı görür, AEAD yok); CC-PORT §0.3 (4)
zaman eşiği 9/8·max(srtt, latest), taban 1 ms; INV-RATE-1 2026-11-16 profil istisnası (ACK'siz uçuş tabanı, hız tavanı aşılmaz).

## Bulgular

| fid | tür | önem | konum (125b9afa) | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| A3T-01 | UYUŞMAZ | orta | `sender.rs:13348-13391` (`e2a_m8_d_*`, `e2a_m8_e_*`); kural `sender.rs:4165-4190`, port açar `:2723` | `rtt_rule_v11` açıkken (ürün varsayılanı port motoru) doygun `ack_delay` ve "guard reddi ∧ ack_delay ≥ rtprop" durumunda RTT örneği ATLANIR | engine-spec §6 [TUR-1x]: "underflow ya da min-rtt-altı → RAW sample işlenir, örnek ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı YASAK)"; §16.1 min-rtt guard satırı "RAW işlenir (örnek atlanmaz; §6/§7.3, RFC 9002 §5.3)". §0.3 CC-PORT YAMA'sı §6'yı listelemez. Kuralın dayanağı (ISEMRI-YOL3-M1 KK-M1-06, "C5.md") depoda bulunamadı | karar-gerekir |
| A3T-02 | UYUŞMAZ | orta | `sender.rs:10795-10848`, `:13155-13176`, `:13192-13228`, `:14152-14156` | eski motorda (geri alma profili `validated_v1` dahil) RTT örneği yokken (PreConverge) PTO/kuyruk probu kurulur ve `Retransmit` çıkar; testler bunu "eski yol değişmez, prob çıkar" diye sabitler | engine-spec §3.3-C(5): "PTO ARM EDİLMEZ … Retransmit/probe TEK sürücüsü §3.3-B geometrik kopya"; §12 NOT; §0.3 YAMA: eski metin "eski motor için geçerli kalır" | kod-duzeltmesi |
| A3T-03 | UYUŞMAZ | orta | `sender.rs:11707-11724`, yorum `:10607`; sabit `K_CONF_EAI` = 2 (`:1982`) | ack_confidence sessizlik eşiği `max(3×srtt, 2×EAI)` | engine-spec §5.1: `threshold = ACK_SILENCE_MULT × srtt`. Spec'te bulunamadı (aranan: `K_CONF_EAI`, "EAI-tabanı", "2×EAI" — §5.1, §16.1) | karar-gerekir |
| A3T-04 | UYUŞMAZ | orta | `sender.rs:11168-11211`, `:11234-11278`; varsayılan `StartupPacingConfig` AÇIK (`:1183-1191`) | §0.3'ün adıyla andığı kilit-golden `inv_rate_1_loss_declaration_preserves_phase_gain` artık başlangıç tabanını KAPATIP koşuyor; varsayılan (taban açık) yol yalnız `max(phase, min(floor, cwnd/srtt))` eşitliğiyle sabit; "loss must never CUT the rate" iddiası `rate ≥ min(rate_before, expect_after)` biçiminde (önceki eşitlikten sonra totoloji). Yeniden kapsamlama kaydı `reports/fs-cal-2/v3.5/INV-RATE-1-STARTUP-FLOOR-SUPERSESSION.md` (rapor), spec/CLAUDE.md YAMA KAYDI'nda yok | engine-spec §0.3 [TUR-7 S1 ek-cümle]: "kayıp-ilanı faz-gain'ini değiştiremez — kilit-golden: `inv_rate_1_loss_declaration_preserves_phase_gain`"; CLAUDE.md §3 "Listede olmayan donmuş metin değişmez" | karar-gerekir |
| A3T-05 | EKSİK | orta | `sender.rs` test modülleri bütünü; `inv_rate_1_*` yalnız `Sender::new` | ürün varsayılanı port motorunda (`with_cc_port`) "kayıp pacing hızını düşürmez" kuralını doğrudan sabitleyen test bu aralıkta yok; INV-RATE-1 adıyla anılan `tests/n8_reverify_emit.rs`, `tv_src.rs`, `tv_loss.rs`, `tv_bw.rs`, `fec_loss_risk.rs` de `with_cc_port` kullanmıyor (grep). `cc/bbr.rs` iç testleri bu denetimde okunmadı → orada var mı DOĞRULANAMADI | CLAUDE.md §4 INV-RATE-1 (2026-11-11): "loss PACING HIZINI düşürmez (pace = BBR pacing_gain × BtlBw)"; transport-tdd §14 INV-RATE-1 satırı | kod-duzeltmesi |
| A3T-06 | UYUŞMAZ | orta | `sender.rs:14077-14118`, `:14407-14438` | port motorunda ARQ yeniden iletim tavanı yok (`arq_retx_cap = u8::MAX`, sayaç doygun, 255'teki kayıt yine yeniden iletilir) | engine-spec §9.1: "ARQ_RETX_CAP: … aşılırsa FEC/forward'a devret veya connection-fail"; §0.3 YAMA yerine geçen bölümleri sayar (§3, §5, §8, §12, §13), §9'u saymaz. "Tavan kalktı" yalnız `docs/design/CC-PORT-ESLEME.md` (belgenin kendi ifadesiyle NORMATİF DEĞİL) | karar-gerekir |
| A3T-07 | UYUŞMAZ | orta | `sender.rs:13826-13875` (`k4g05br_late_ack_without_live_record_does_not_confirm` (c)); U16 yorumu `:14200-14203` | `Duplicate` kökenli kayıp ilanı tombstone bırakmaz ve yeniden kuyruğa girmez; geç ACK'i teyit/kredi vermez. Birimin bütün kopyaları bu yoldan kaybolursa motordan düşebilir (ESLEME U14 notu, iki motor; port için U16 (a) kısmi) | engine-spec §5 [TUR-2b]: kayıp ilanında gönderim kaydı "{unit, send_ns, bw_sample, retx_count, origin} SAKLANIR"; ZEYL-2 "superseded-işaret TEKDÜZEDİR"; CLAUDE.md §2 recovery-obsesif taban. ESLEME: "Düzeltme adayı (iki motoru da değiştirir → sahip kararı)" | karar-gerekir |
| A3T-08 | DOGRULANAMADI | düşük | `sender.rs:14178-14198` (`u14_pto_probe_resumes_when_no_ladder_entry_serves_the_oldest_unit`) | port motorunda merdiven kaydı yoksa RTT örneği olmadan PTO probu kurulur (canlılık istisnası) | engine-spec §3.3-C(5) "PTO ARM EDİLMEZ"; §0.3 YAMA §3 "faz makinesi"nin yerini BBR+RFC 9002'nin aldığını söyler ama port motorunda PreConverge durumu sürüyor → §3.3-C'nin portu bağlayıp bağlamadığı belirsiz | karar-gerekir |
| A3T-09 | TANIMSIZ | orta | `sender.rs:14249-14405` (U16 (b), U17), `:16321-16348` (`fate_key`) | PTO probu hedefi ve yeni veri seçimi `unit_id` sırasına dayanır; testler client-core'un şerit etiketini kimliğin üst bitlerine yazdığını (`3<<62` dosya, `2<<62` metin) ve "küçük kimlik = öncelikli şerit (PTT)" anlamını varsayar | transport-tdd §14 INV-ARQ-1: "Transport uygulama rolünü (hot-path/bulk) BİLMEZ (SoC)"; CLAUDE.md §4 INV-ENGINE-SOC "opak birim". Kimlik sırasının öncelik anlamı spec'te bulunamadı (aranan: "şerit", "lane", "kimlik sırası" — engine-spec, transport-tdd). #211 "sahibin koşullu sözüyle" birleşti; ESLEME hâlâ "aday — sahip kararı bekler" yazıyor | karar-gerekir |
| A3T-10 | TANIMSIZ | düşük | `sender.rs:16389-16416`, `:16533-16582` | `ForwardCopyLineScale`: beyan altı ölçülen hatta kopya sayısı orantılı (en yakın tamsayı, `min_copies`); KOPYA-SIRA: aç profilde kopyalar birimler arası vade sırasıyla | sahip kararı 2026-11-16 (CLAUDE.md YAMA KAYDI: PTT D× kopya `runtime::PTT_FORWARD_COPIES`) sayıyı tanımlar; ölçekleme ve sıralama kuralı spec'te bulunamadı (aranan: "ForwardCopyLineScale", "hatla orantılı", "KOPYA-SIRA"). PTT davranışı sahip kararı 3 (7171705736) ile donduruldu | kilitli |
| A3T-11 | TANIMSIZ | orta | `sender.rs:16630-16721` | LineGuard boşluk bütçesi: aralık kipinde 2 ölçümsüz ACK (`UNMEASURED_GAP_ACKS`), 2 s öncelik penceresi, öncelikli hız < 3R/8 ise kurulur; parite öncelik sayılmaz | spec'te bulunamadı (aranan: "boşluk bütçesi", "3R/8", "LineGuard", "hat koruması" — `specs/` tümü). Sabitlerin türetimi yalnız `line_guard.rs` başlığında | karar-gerekir |
| A3T-12 | TANIMSIZ | düşük | `sender.rs:11387-11622`; `BS_FIRES_NS` [3, 9, 21, 45 s] (`:1120`), `BS_TX_HARD_BUDGET` 200 B (`:1124`) | DR-CS-3 bootstrap prob merdiveni ve 200 B bütçe (varsayılan kapalı) | dayanak `reports/dr-cs-3/DR-CS-3-BOOTSTRAP-RTT-CONTRACT.md` (rapor). engine-spec §11.1 merdiveni tanımlamaz; §16.1 `PROBE_BUDGET_FRACTION`/`MINI_BURST_COUNT` formülleri bant-türevli, sabit 200 B ve 3/9/21/45 s değil | karar-gerekir |
| A3T-13 | UYUŞMAZ | düşük | `sender.rs:11768-11843`, `:12559-12594` | DrainRtprop girişi rtprop min-örnek yaşı ≥ W_rtprop/2 iken (testte 12 s) | engine-spec §8.3 "RTprop_stamp > RTPROP_WINDOW eskiyse"; §12 "RTPROP_PROBE_TIMER: RTPROP_WINDOW (§8.3) dolunca DRAIN_RTPROP". W/2 spec'te bulunamadı (aranan: "W/2", "W_rtprop/2") | kod-duzeltmesi |
| A3T-14 | UYUŞMAZ | düşük | `sender.rs:13319-13346` (kapalı kol), `:10850-10880` | eski motorda Duplicate kökenli kaydın (PreConverge kuruluş kopyası) geçerli SACK'i RTT kilidini açmaz ("PC tuzağı (eski)"); Retransmit kökenli kaydın kendi ACK'i RTT örneği vermez | engine-spec §3.3-D "İlk geçerli Noise-authenticated SACK/echo → RTT + loss kilidi açılır"; §5 TUR-2b ZEYL "Retransmit-seq'in KENDİ ACK'i inflight-yolundan tam örnek verir (belirsizlik yok)" ("tam örnek"in RTT'yi kapsadığı ikinci kısım için DOĞRULANAMADI) | karar-gerekir |
| A3T-15 | UYUŞMAZ | düşük | `sender.rs:11624-11654`, `:12269-12336` | eski motorda RecoverArq'ta DATA-önce; retx yalnız her 4. izinde önde | engine-spec §13 "priority: retransmit(deadline) > FEC parity > yeni DATA"; §16.1 `K_ARQ_SHARE` satırı "DATA-önce ölçülü-kararı diğer grant'lerde korunur" — spec içi çelişki; sahip v4 EK-A §2 `retx_first`'i gösterimde kapalı tuttu | karar-gerekir |
| A3T-16 | EKSİK | düşük | `sender.rs:11604-11622` | `bootstrap_does_not_relax_karn_or_touch_arq_timing` hiç ACK vermez; PreConverge'de kayıp ilanı zaten yok → `diag_loss` eşitliği boş geçer; adındaki Karn iddiası sınanmıyor | engine-spec §3.3-F/§11.1 "hızlandırıcı; hiçbir şeyi bloklamaz"; Karn kuralı §5 TUR-2b | kod-duzeltmesi |
| A3T-17 | EKSİK | düşük | `sender.rs:11312-11329`, `:11726-11766` | `startup_floor_never_exceeds_cwnd_over_srtt` ikinci iddiası `floor.min(c) ≤ c` totoloji; `startup_exit_enters_drain_then_steady_on_bdp` yorumu "inflight ≤ BDP" der, kod `CWND_GAIN × BDP` (`:5306-5320`) kullanır, test iki eşiği ayırt etmez | engine-spec §3 "DRAIN --inflight≤cwnd_gain×BDP ∨ dwell≥3×ack_epoch--> STEADY" | kod-duzeltmesi |
| A3T-18 | TANIMSIZ | düşük | `sender.rs:9851-9889` (`warmup_recovered`) | düzenek saati geri sarar: ACK 315 ms'de, sonra RECOVERED 305 ms'de ve PTO 310 ms'de verilir; motorun azalan `now` davranışı test sonucuna girer | engine-spec §16.2 saat-sınıfı envanteri azalan zaman girdisini tanımlamaz (aranan: "monoton", "saat geri") | kod-duzeltmesi |
| A3T-19 | TANIMSIZ | düşük | `sender.rs:14539-14613` | teyit çıktısının tek çağrıdaki boyu `CONFIRMED_UNITS_CAP + olay boyu`; tavan yalnız birikime sonraki çağrıda uygulanır | CLAUDE.md §9 SR-AUDIT-001 "unbounded alloc yok, bounds-before-alloc"; engine-spec §14 "Tüm pool'lar sabit-üst-sınır". Üst sınır çağıranın verdiği listeye bağlı (ACK çerçevesi `MAX_ACK_RANGES` ile sınırlı; RECOVERED listesinin sınırı bu aralıkta görülmedi) | karar-gerekir |
| A3T-20 | TANIMSIZ | düşük | `sender.rs:13900-14066`, `:14448-14503`, `:15930-15931` | `FecParity` kararında `unit_id` = ESI (kaynak birim ad-uzayıyla aynı alan); testler sızıntıya karşı korur (ESI adaşı teyit/iptal edilmez, prob pariteyi hedeflemez) | engine-spec §10.1 FEC metadata'sı `block_id`/`shard_index`; `TransmitDecision.unit_id`'nin paritedeki anlamı spec'te bulunamadı. Tip ayrımı imza değişikliğidir (CLAUDE.md §6: talimatsız yapma) | karar-gerekir |
| A3T-21 | TANIMSIZ | düşük | `sender.rs:16269-16284` | `set_cc_profile_pacing_cap(Some(0))` "tavan yok" sayılır (sessiz sentinel; `None` zaten var) | spec'te bulunamadı (aranan: profil pacing tavanı sentinel'i); CLAUDE.md §9 açık hata tipleri | kod-duzeltmesi |
| A3T-22 | EKSİK | düşük | `sender.rs:11399-11415`, `:13192-13228`, `:16584-16628` | hop-ACK'siz ardışık PN tavanı (K = 1024) uygulanmadı; testler ACK'siz sınırsız emisyon yollarını (geometrik merdiven, RecoverFwd kopyaları, aç profil ACK'siz taban) tavansız sabitler | transport-tdd §12 [AMENDMENT-9] "K = 1024 … DURUM: SPEC-ONLY … AYRI bir iş kalemidir" | karar-gerekir |
| A3T-23 | OLU | düşük | `sender.rs:12135-12762` (E1), `:11437-11442`, `:11484-11488` (`experimental-cold-start`); `#[allow(deprecated)] enqueue_fec_parity` kullanımları | E1 bayrakları ürün profilinde kapalı; port motorunda `e1_probe_keep`/`retx_first` etkisini `cc_on()` zaten verir (ESLEME); gösterim penceresi (9–17 Kas) kapandı. `experimental-cold-start` eski merdiveni ve deprecated `enqueue_fec_parity` yalnız test düzeneğinde yaşıyor | PRODUCT-PROFILE `e1_flags` KEEP-EXPERIMENTAL; CLAUDE.md §6 "Deprecated içerik → appendix'e taşı + MUST NOT IMPLEMENT" | karar-gerekir |
| A3T-24 | DOGRULANAMADI | düşük | `sender.rs:13730-15807` ("[sahip kararı 2026-11-11]" etiketli testler); `K_CONFIRMED_RELEASE` = `K_TOMB` (`:1406`) | teyit sonrası iş iptali, kararsız kopya işareti ve bırakma anı | atıf verilen sahip metni `docs/decisions/` ve okunan #175 yorumlarında yok; yalnız `docs/design/K4G-05b-RUNTIME-ENVANTER.md` (tasarım belgesi). `K_CONFIRMED_RELEASE` için ayrı türetim spec'te bulunamadı | karar-gerekir |

Kilitli kapsam notu: PTT/metin davranışı (karar 3) yalnız A3T-10'da dokunuluyor. `receiver.rs` alarm aralığı (Web'e
verildi, 7186813951) ve #262 dalı bu aralıkta test edilmiyor; ilgili bulgu yok.

## Okunan otorite bölümleri

- `specs/adaptive-transport-engine-spec.md`: §0.3 (CC-PORT YAMA, INV-RATE-1 TUR-7 ek-cümle), §3 (durumlar, geçişler,
  öncelik, AMENDMENT-1), §3.1–§3.3 (A–F, yasaklar C(1)–(5), kademeli tahliye D), §4, §5 (CC-PORT YAMA, (a)/(b),
  TUR-2a ledger, TUR-2b tombstone, ZEYL-2/3), §5.1, §6 (TUR-1x min-rtt guard, RTprop, qd), §8.1–§8.5, §9–§9.4,
  §10.2, §11 (AMENDMENT-3), §11.1, §12, §13 (CC-PORT YAMA), §14, §15, §16.1 tablo; başlık YAMA KAYDI 2026-11-21.
- `specs/transport-layer-tdd.md`: §12 AMENDMENT-9 (K = 1024, SPEC-ONLY), §14 invaryant tablosu (INV-RATE-1/2,
  INV-FEC-1/2, INV-ARQ-1, INV-ACK-1).
- `CLAUDE.md`: §4 (INV-RATE-1 2026-11-11 + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §6, §8, §9,
  YAMA KAYDI.
- `docs/decisions/KARAR-KAYDI-2026-11.md` §1–§10 (D-01, D-23, D-CC-1); `docs/decisions/SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md`
  (E1 K1 seti, `retx_first` kapalı) — ilgili satırlar.
- #175 sahip kararları: 7171705736 (dokuz karar; karar 3 PTT/metin dondurma), 7172579876, 7174289733, 7174845328,
  7175487850, 7175498704, 7175888074, 7186813951.
- Normatif olmayan, bağlam için: `docs/design/CC-PORT-ESLEME.md` (eşleme tablosu, U1–U17), `docs/transport-v1/PRODUCT-PROFILE.md`
  (`e2a_flags`, `e1_flags` satırları); #211 squash mesajı (U17 birleşme yetkisi).
- `specs/fec-core-tdd-v3.0.md` ve `specs/tech-stack-adr.md`: bu test aralığındaki davranışlar için ADR-16/17'nin
  CLAUDE.md §5 özeti yeterli görüldü; ayrıntılı bölüm okunmadı (FEC kodlayıcı bu testlerde yok).
