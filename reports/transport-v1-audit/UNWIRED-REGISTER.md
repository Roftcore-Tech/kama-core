# KABLOSUZ / ÖLÜ KOD SİCİLİ

CODE→SPEC süpürmesi: **839 kalem** — Counter({'KEEP-EXPERIMENTAL': 407, 'SHIP': 249, 'DELETE': 94, 'WIRE': 89}).
Çağrılma kaynağı: {'product': 454, 'feature-gated': 57, 'test': 143, 'nowhere': 54, 'bench': 100, 'diagnostic-only': 31}

## DELETE (94) — ölü, duplicate, superseded veya yanıltıcı

| sembol | yol | yalnız | modül |
|---|---|---|---|
| `FuzzyConfig.sr_med` | crates/adaptive-transport/src/fusion.rs:236 | nowhere | at-controllers (crates |
| `FusionInputSnapshot::to_inputs` | crates/adaptive-transport/src/fusion.rs:414 | test | at-controllers (crates |
| `FusionPolicy::calibration_v1` | crates/adaptive-transport/src/fusion_policy.rs:272 | test | at-controllers (crates |
| `FusionPolicy::with_safety` | crates/adaptive-transport/src/fusion_policy.rs:380 | test | at-controllers (crates |
| `FusionAction::is_arq_only` | crates/adaptive-transport/src/fusion_policy.rs:167 | test | at-controllers (crates |
| `FallbackReason::SafetyRejected` | crates/adaptive-transport/src/fusion_policy.rs:76 | nowhere | at-controllers (crates |
| `FEATURE_SCHEMA_V1` | crates/adaptive-transport/src/fusion_policy.rs:41 | test | at-controllers (crates |
| `PolicyKind::BoundedDebtFloor` | crates/adaptive-transport/src/budget_kernel.rs:33 | test | at-controllers (crates |
| `FloorTiming::SuspendedNoOpportunity` | crates/adaptive-transport/src/budget_kernel.rs:217 | nowhere | at-controllers (crates |
| `BudgetDecision.data_token_limit / .floor_grants / .next_wakeup_hint_ns` | crates/adaptive-transport/src/budget_kernel.rs:127 | nowhere | at-controllers (crates |
| `lib.rs doc: fate_estimator 'observation-only: no product caller'` | crates/adaptive-transport/src/lib.rs:31 | product | at-controllers (crates |
| `fate_estimator::reordering_guard_ns` | crates/adaptive-transport/src/fate_estimator.rs:47 | test | at-controllers (crates |
| `features_from_fate_bits` | crates/adaptive-transport/src/fate_estimator.rs:114 | test | at-controllers (crates |
| `FateEstimator::drop_unresolved_as_unknown` | crates/adaptive-transport/src/fate_estimator.rs:295 | test | at-controllers (crates |
| `Framing::SegmentBlock` | crates/adaptive-transport/src/emergency.rs:58 | nowhere | at-controllers (crates |
| `RecoveryAction::MinimalParity` | crates/adaptive-transport/src/emergency.rs:76 | nowhere | at-controllers (crates |
| `EmergencyConfig.block_parity_dup2_permille / .block_parity_dup3_permil` | crates/adaptive-transport/src/emergency.rs:110 | nowhere | at-controllers (crates |
| `Sender::last_recovery accessor` | crates/adaptive-transport/src/sender.rs:5980 | nowhere | at-controllers (crates |
| `TailRecovery::spurious_probe_count / ::spurious_probe_bytes` | crates/adaptive-transport/src/tail_recovery.rs:278 | diagnostic-only | at-controllers (crates |
| `DeadlineArqScheduler::plan — arq_aggressiveness==0 suppression branch` | crates/adaptive-transport/src/arq.rs:106 | nowhere | at-controllers (crates |
| `ReorderTracker::recent_max_reorder accessor` | crates/adaptive-transport/src/arq.rs:160 | nowhere | at-controllers (crates |
| `rate::evaluate + SrcDecision` | crates/adaptive-transport/src/rate.rs:137 | test | at-controllers (crates |
| `BtlBwFilter::diag_accept` | crates/adaptive-transport/src/bw.rs:263 | nowhere | at-controllers (crates |
| `DeliveryRateSampler::packets_in_flight accessor` | crates/adaptive-transport/src/bw.rs:126 | nowhere | at-controllers (crates |
| `rtt::rtt_sample` | crates/adaptive-transport/src/rtt.rs:225 | test | at-controllers (crates |
| `LossDetector::loss_pre_fec_ppm` | crates/adaptive-transport/src/loss.rs:150 | test | at-controllers (crates |
| `ThresholdLossDetector::fec_loss_risk_ppm` | crates/adaptive-transport/src/loss.rs:476 | test | at-controllers (crates |
| `fec.rs — FecShardFrame / FRAME_TYPE_FEC_SHARD (0x20) / SHARD_TAG_LEN` | crates/adaptive-transport/src/fec.rs:36 | test | at-controllers (crates |
| `RxState + Receiver::state accessor` | crates/adaptive-transport/src/receiver.rs:22 | test | at-controllers (crates |
| `RxOutput.was_duplicate` | crates/adaptive-transport/src/receiver.rs:72 | nowhere | at-controllers (crates |
| `udp_runner::UNIT` | crates/adaptive-transport/src/udp_runner.rs:17 | nowhere | at-harness |
| `RunnerCfg::ticket_cap` | crates/adaptive-transport/src/udp_runner.rs:68 | diagnostic-only | at-harness |
| `RunnerCfg::to_policy ("f10"/"f15"/"f20" aliases)` | crates/adaptive-transport/src/udp_runner.rs:123 | feature-gated | at-harness |
| `fs_harness::PacketSink (trait)` | crates/adaptive-transport/src/fs_harness.rs:62 | test | at-harness |
| `fs_harness::A1EarlyEmitter (+ new/target_set/maybe_emit_duplicate/cost` | crates/adaptive-transport/src/fs_harness.rs:111-163 | test | at-harness |
| `fs_harness::RedundancyEmissionPolicy` | crates/adaptive-transport/src/fs_harness.rs:167 | test | at-harness |
| `fs_harness::a1_emission_order` | crates/adaptive-transport/src/fs_harness.rs:175 | test | at-harness |
| `fs_trace::TraceSpec::NaturalBurst` | crates/adaptive-transport/src/fs_trace.rs:82 | test | at-harness |
| `fs_trace::RedundancyPolicy::Iid` | crates/adaptive-transport/src/fs_trace.rs:68 | test | at-harness |
| `EngineError::FeatureNotImplemented` | crates/kama-engine/src/error.rs:33-36 | nowhere | at-harness |
| `EngineError::ParamNotConfirmed` | crates/kama-engine/src/error.rs:18-20 | nowhere | at-harness |
| `EngineError::ParamNeverArrived` | crates/kama-engine/src/error.rs:46-49 | nowhere | at-harness |
| `ParamSetPlan::start_fec_immediately` | crates/kama-engine/src/param_bootstrap.rs:18 | nowhere | at-harness |
| `kama_engine::ParamAckTracker (+ on_param_ack, is_confirmed)` | crates/kama-engine/src/param_bootstrap.rs:47,62,80 | test | at-harness |
| `ParamState::bound_profile_id` | crates/kama-engine/src/param_state.rs:77 | test | at-harness |
| `ParamState::bound / BoundParam` | crates/kama-engine/src/param_state.rs:83,14 | nowhere | at-harness |
| `kama_engine::PreParamHold + MAX_PREPARAM_SHARDS` | crates/kama-engine/src/preparam_hold.rs:17,13 | test | at-harness |
| `kama_engine::RecoveredMap + ResolvedRecovered + RECOVERED_MAP_CAP` | crates/kama-engine/src/recovered_map.rs:26,18,13 | test | at-harness |
| `BootstrapEchoResponder::reset` | crates/kama-integration/src/receiver_control_core.rs:159 | nowhere | at-harness |
| `HostError::FeatureNotImplemented` | crates/kama-integration/src/receiver_host.rs:76-78 | nowhere | at-harness |
| `ReceiverHost::insert_active_session_for_test` | crates/kama-integration/src/receiver_host.rs:244 | test | at-harness |
| `ReceiverHost::tombstone_grace_deadline_ns` | crates/kama-integration/src/receiver_host.rs:238 | nowhere | at-harness |
| `mod kama (stub transport arm)` | crates/bench/src/kama.rs:1 | bench | bench (crates/bench/sr |
| `enum KamaArm { Arq, TailV1 } + KamaArm::label` | crates/bench/src/transport.rs:34 | bench | bench (crates/bench/sr |
| `CLI arg `serve --ts-out`` | crates/bench/src/main.rs:87 | nowhere | bench (crates/bench/sr |
| `ServerCfg::ts_out` | crates/bench/src/transport.rs:269 | nowhere | bench (crates/bench/sr |
| `struct ClientRun + fn drive + fn handle_datagram (stub arm)` | crates/bench/src/kama.rs:85 | bench | bench (crates/bench/sr |
| `TransmitKind catch-all `_ => {}` in stub-arm tx accounting` | crates/bench/src/kama.rs:234 | bench | bench (crates/bench/sr |
| `env knob KAMA_BENCH_TRACE (ClientRun.trace)` | crates/bench/src/kama.rs:131 | diagnostic-only | bench (crates/bench/sr |
| `async fn serve_loop (stub arm) + RTRAJ diag line` | crates/bench/src/kama.rs:541 | bench | bench (crates/bench/sr |
| `stub arm silently ignores 13 CLI flags` | crates/bench/src/kama.rs:256 | bench | bench (crates/bench/sr |
| `struct KamaTransport (unit)` | crates/bench/src/transport.rs:296 | bench | bench (crates/bench/sr |
| `pub fn BudgetGovernor::is_armed` | crates/bench/src/budget.rs:135 | bench | bench (crates/bench/sr |
| `FateWindowDiag::rtt_samples` | crates/client-core/src/runtime.rs:435 | nowhere | client-core |
| `FateWindowDiag::spurious_units vs ledger_spurious_units` | crates/client-core/src/runtime.rs:464 | diagnostic-only | client-core |
| `ClientStats::{tx_datagrams, rx_datagrams, unknown_frames, fec_repair_m` | crates/client-core/src/runtime.rs:527 | nowhere | client-core |
| `ServerStats::{fec_repairs_wasted_complete, fec_repairs_wasted_evicted,` | crates/client-core/src/runtime.rs:617 | nowhere | client-core |
| `Delivery::peer` | crates/client-core/src/runtime.rs:664 | nowhere | client-core |
| `LEGACY_SESSION_DATA_OVERHEAD` | crates/kama-transport-schema/src/lib.rs:45 | nowhere | schema-codec (crates/k |
| `ShardKind::from_codepoint` | crates/kama-transport-schema/src/inner_kind.rs:46 | nowhere | schema-codec (crates/k |
| `FecCodec::recompute_repair` | crates/kama-fec-codec/src/codec.rs:85 | bench | schema-codec (crates/k |
| `RsSimdEngine` | crates/kama-fec-codec/src/codec.rs:334 | bench | schema-codec (crates/k |
| `RsSimdEngine::new` | crates/kama-fec-codec/src/codec.rs:351 | bench | schema-codec (crates/k |
| `impl Debug for RsSimdEngine` | crates/kama-fec-codec/src/codec.rs:339 | test | schema-codec (crates/k |
| `CsTimerKind::TestHook` | crates/adaptive-transport/src/sender.rs:122 | test | sender-a (crates/adapt |
| `BootstrapRttDiag::duplicate_step_denied` | crates/adaptive-transport/src/sender.rs:1033 | nowhere | sender-a (crates/adapt |
| `BootstrapRttDiag::probe_after_success_violation` | crates/adaptive-transport/src/sender.rs:1049 | nowhere | sender-a (crates/adapt |
| `BootstrapRttDiag::budget_violation` | crates/adaptive-transport/src/sender.rs:1051 | diagnostic-only | sender-a (crates/adapt |
| `SchedDiag write-only fields (8)` | crates/adaptive-transport/src/sender.rs:1151, 1165, 1167, 11 | nowhere | sender-a (crates/adapt |
| `Sender::sched_diag` | crates/adaptive-transport/src/sender.rs:1979 | nowhere | sender-a (crates/adapt |
| `Sender::tail_probe_due_edge_count` | crates/adaptive-transport/src/sender.rs:2050 | nowhere | sender-a (crates/adapt |
| `Sender::diag_bw_accept` | crates/adaptive-transport/src/sender.rs:2043 | nowhere | sender-a (crates/adapt |
| `Sender::bootstrap_budget_remaining` | crates/adaptive-transport/src/sender.rs:2014 | nowhere | sender-a (crates/adapt |
| `cs_test_arm_hook_timer / cs_test_hook_fired` | crates/adaptive-transport/src/sender.rs:2870 / :2879 | test | sender-a (crates/adapt |
| `dead binding `let _ = ack_epoch;` in on_ack_received` | crates/adaptive-transport/src/sender.rs:2912 | diagnostic-only | sender-b (crates/adapt |
| `diag_qd_no_evidence` | crates/adaptive-transport/src/sender.rs:1678 | nowhere | sender-b (crates/adapt |
| `diag_last_obs / diag_obs_bw_range / diag_obs_loss_range (diag_last_fus` | crates/adaptive-transport/src/sender.rs:3498 | diagnostic-only | sender-b (crates/adapt |
| `persist_cap_srtt_permille (learned-horizon ceiling)` | crates/adaptive-transport/src/sender.rs:368 | nowhere | sender-b (crates/adapt |
| `Sender::enqueue_fec_parity` | crates/adaptive-transport/src/sender.rs:4717 | test | sender-b (crates/adapt |
| `bs_diag.budget_violation vacuous increment` | crates/adaptive-transport/src/sender.rs:3196 | nowhere | sender-b (crates/adapt |
| `gain_ge1 send-time gain stamp (TUR-D1 F3)` | crates/adaptive-transport/src/sender.rs:5763-5769 | nowhere | sender-c (crates/adapt |
| `Sender::last_recovery` | crates/adaptive-transport/src/sender.rs:5980 | nowhere | sender-c (crates/adapt |
| `lib.rs doc claim on `pub mod fate_estimator`: "observation-only: no pr` | crates/adaptive-transport/src/lib.rs:34-36 | product | sender-c (crates/adapt |
| `lib.rs crate-doc roadmap line: "Sıradaki: bench `KamaTransport`'u moto` | crates/adaptive-transport/src/lib.rs:16 | nowhere | sender-c (crates/adapt |

## Hiçbir yerden çağrılmayan, DELETE dışı (11)

| sembol | yol | sınıf |
|---|---|---|
| `CapExceedPolicy::ConnectionFail` | crates/adaptive-transport/src/arq.rs:32 | WIRE |
| `TransportError::FeatureNotImplemented` | crates/adaptive-transport/src/error.rs:53 | KEEP-EXPERIMENTAL |
| `ServerStats::{rx_datagrams, unknown_frames, fec_repairs_rx, fec_decode` | crates/client-core/src/runtime.rs:601 | WIRE |
| `SealError::{NotReady, PnExhausted, EpochKeysMissing, Internal}` | crates/client-core/src/seam.rs:56 | KEEP-EXPERIMENTAL |
| `OpenReject::{Replay, UnknownPeer, EpochNotAccepted}` | crates/client-core/src/seam.rs:73 | KEEP-EXPERIMENTAL |
| `SealedDatagram::{full_wire_pn, dh_epoch}` | crates/client-core/src/seam.rs:40 | WIRE |
| `FecProfile 0x04 (K=16,R=8,E=64) and 0x06 (K=16,R=8,E=1024)` | crates/kama-fec-codec/src/profile.rs:76-82, 92-98 | KEEP-EXPERIMENTAL |
| `PathTimingState / Sender::path_timing_snapshot` | crates/adaptive-transport/src/sender.rs:228 (type) / :5903 ( | KEEP-EXPERIMENTAL |
| `Sender::next_wakeup_ns (+ earlier, next_timer_deadline_ns, tail_probe_` | crates/adaptive-transport/src/sender.rs:2433 / :2438 / :2453 | WIRE |
| `RetxDecision::CapExceeded { policy: CapExceedPolicy::ConnectionFail }` | crates/adaptive-transport/src/sender.rs:5387 | WIRE |
| `impl Default for Sender` | crates/adaptive-transport/src/sender.rs:5990-5994 | SHIP |