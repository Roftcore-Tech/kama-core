//! FS-CAL-2 v3 — product-path flow runner (warm-up 384 → estimator snapshot → decision block R0/R2).
//!
//! Normative: FS-CAL-2-EXPERIMENT-DESIGN-v3 (sha 115fa266…3393) §1.1–§2.4, §3, §4.1–§4.3, §5.3.
//!
//! GROUNDING (verbatim product symbols; DEFAULT cargo features — no experimental-cold-start /
//! experimental-fec-budget; the same compile configuration as the DR-ARQ-PRODUCT-TAIL-1 validation):
//! * ARQ base (§1.1): real `adaptive_transport::sender::Sender::new()` + the FROZEN product tail
//!   config (DR-ARQ-PRODUCT-TAIL-1 `tail_on_config`, examples/dr_arqtail_product.rs:35-46):
//!   enabled, min_pto 10ms, max_pto 10s, initial 1s, max_backoff 30s, max_ack_delay 25ms, 3 probes.
//! * FEC_R2 (§1.2/§4.1): the FS-1/R2A authoritative coding identity — K=8, profile `by_id(0x03)`
//!   (K=8/E=64), real `RsSimdCodec`, r=2, parity sealed length == source sealed length == 101 B
//!   (`fs_sealed_len(64) = 64+37`, fs_harness.rs:51-59) — EMITTED through the PRODUCT Sender FEC
//!   authority (`open_fec_block_authority` + `close_fec_block_manual` → `plan_fec_parity` →
//!   `TransmitKind::FecParity`; sender.rs:2400-2505, 2827-2912), i.e. paced by the product pacer
//!   and seq/inflight-tracked. Decode: FS-1/R2A counting-MDS rule + `RsSimdCodec.reconstruct` +
//!   `Event::RecoveredControl` feedback (fs_spine.rs:799-864 identity).
//! * Channel: egress-fair FIFO bottleneck (serializes dropped packets too) — the fs_harness
//!   `BottleneckChannel::offer` semantics (fs_harness.rs:355-365) re-stated here without the
//!   feature-gated dependency; NO reordering (FIFO cursor); per-class deterministic drop streams
//!   from `fs_cal2_gen` (§5.3 InitialSource/Parity/ArqRetransmission).
//! * Estimator (§3): `adaptive_transport::fate_estimator` (product crate, observation-only), guard
//!   from the product `TailRecovery::base_pto_ns` symbol via live `path_timing_snapshot()`.
//! * Workload (§2.2): warm-up 384 + decision block 8 source units of 101 B; offers eligibility-paced
//!   at 800 kbit/s over a 1 Mbit/s bottleneck (R2A `cfg_of` identity: link 1_000_000, pacer 800_000,
//!   dt 10_000 ns); frozen per-RTT horizons {50→699, 250→2499, 1000→9249} ms and (secondary)
//!   deadlines {199, 599, 2099} ms (DR-ARQ-PRODUCT-TAIL-1 / R2A identity).
//!
//! RESULT-BEARING: this module never writes artifacts; callers decide persistence. M1 outcome-blind
//! mode is implemented by the caller simply not serializing the returned output.
#![allow(dead_code)]

#[path = "fs_cal2_gen.rs"]
pub mod fs_cal2_gen;
#[path = "fs_cal2_policy.rs"]
pub mod fs_cal2_policy;

use adaptive_transport::fate_estimator::{reordering_guard_ns, FateEstimator, WindowFeatures};
use adaptive_transport::fusion_policy::FusionSafetyConfig;
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, SrLogicalRef, Timer, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use fs_cal2_gen::{loss_bits, stream_key, LossModel, StreamClass};
use fs_cal2_policy::{FecActivationPolicyV1, PolicyFallback};
use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// ns per ms.
pub const MS: u64 = 1_000_000;
/// Sealed wire unit (E=64 + 37 seal overhead → 101 B; fs_harness `fs_sealed_len` identity).
pub const SEALED: u64 = 101;
/// Symbol payload length E (profile 0x03).
pub const E_LEN: usize = 64;
/// FEC block K (FS-1/R2A authoritative geometry).
pub const K: u16 = 8;
/// Authoritative FEC_R2 profile id (K=8/E=64 catalog entry).
pub const R2_PROFILE_ID: u8 = 0x03;
/// FEC_R2 parity count per decision block.
pub const R2_PARITY: u16 = 2;
/// §2.3 warm-up length (original source packets), PINNED.
pub const WARMUP_UNITS: u64 = 384;
/// Decision-bearing block length = K.
pub const BLOCK_LEN: u64 = K as u64;
/// Decision block id (first block boundary after warm-up: 384/8).
pub const DECISION_BLOCK_ID: u32 = 48;
/// Source unit-id base (avoids unit_id collision with parity esi in `OpaqueUnit.id`).
pub const BASE_UNIT_ID: u64 = 1000;
/// Bottleneck link (bits/s) — R2A identity.
pub const LINK_BPS: u64 = 1_000_000;
/// Source-offer eligibility pacer (bits/s) — R2A identity (0.80C).
pub const OFFER_PACER_BPS: u64 = 800_000;
/// Virtual tick (ns) — R2A identity.
pub const DT_NS: u64 = 10_000;
/// Frozen workload profile id (§2.2 interlock; freeze-record identity).
pub const WORKLOAD_PROFILE_ID: &str = "FSCAL2-BULK-W384-K8-U101-v1";
/// Frozen ARQ contract id (§4.0b; freeze-record identity).
pub const ARQ_CONTRACT_ID: &str = "FSCAL2-ARQ-DEFAULTFEAT-TAILV1-v1";

/// FROZEN product tail-recovery ON config — DR-ARQ-PRODUCT-TAIL-1 `tail_on_config` VERBATIM values.
#[must_use]
pub fn tail_on_config() -> TailRecoveryConfig {
    TailRecoveryConfig {
        enabled: true,
        min_pto_ns: 10 * MS,
        max_pto_ns: 10_000 * MS,
        initial_pto_ns: 1_000 * MS,
        max_backoff_pto_ns: 30_000 * MS,
        max_ack_delay_ns: 25 * MS,
        max_tail_probes: 3,
    }
}

/// §3.2 estimator window length, as a u64 (mirror of `fate_estimator::FATE_WINDOW`).
pub const FATE_WINDOW_LEN: u64 = 256;

/// Stable name for a product sender state (diagnostic labelling only).
#[must_use]
pub fn state_name(s: adaptive_transport::sender::SenderState) -> &'static str {
    use adaptive_transport::sender::SenderState as S;
    match s {
        S::Idle => "Idle",
        S::PreConverge => "PreConverge",
        S::Startup => "Startup",
        S::Drain => "Drain",
        S::ProbeUp => "ProbeUp",
        S::Steady => "Steady",
        S::RecoverArq => "RecoverArq",
        S::RecoverFwd => "RecoverFwd",
        S::DrainRtprop => "DrainRtprop",
    }
}

/// Frozen per-RTT observation horizon (ms) — DR-ARQ-PRODUCT-TAIL-1 / R2A identity.
#[must_use]
pub fn horizon_ms(rtt_ms: u64) -> u64 {
    match rtt_ms {
        50 => 699,
        250 => 2499,
        1000 => 9249,
        other => panic!("unsupported RTT cell {other} (frozen cells: 50/250/1000)"),
    }
}

/// Frozen per-RTT secondary deadline (ms) — same identity chain.
#[must_use]
pub fn deadline_ms(rtt_ms: u64) -> u64 {
    match rtt_ms {
        50 => 199,
        250 => 599,
        1000 => 2099,
        other => panic!("unsupported RTT cell {other}"),
    }
}

/// Deterministic canonical E=64 symbol bytes for a global source id (content is transport-opaque;
/// only length + RS algebra matter; byte-equality is asserted against this canonical).
#[must_use]
pub fn symbol_bytes(gid: u16) -> Vec<u8> {
    let h1 = fs_cal2_gen::draw_hex(&format!("FSFC2-SYM|{gid}|0"));
    let h2 = fs_cal2_gen::draw_hex(&format!("FSFC2-SYM|{gid}|1"));
    let mut v = Vec::with_capacity(E_LEN);
    for hx in [h1, h2] {
        for i in 0..32 {
            v.push(u8::from_str_radix(&hx[2 * i..2 * i + 2], 16).unwrap());
        }
    }
    v
}

/// Egress-fair FIFO bottleneck (fs_harness `BottleneckChannel::offer` semantics: EVERY offered
/// packet serializes — dropped included; arrival = serialize-end + owd; FIFO ⇒ no reordering).
struct Chan {
    link_bps: u64,
    owd_ns: u64,
    link_free_ns: u64,
}

impl Chan {
    fn offer(&mut self, len: u64, now_ns: u64, drop: bool) -> Option<u64> {
        let ser = len.saturating_mul(8).saturating_mul(1_000_000_000) / self.link_bps.max(1);
        let start = self.link_free_ns.max(now_ns);
        self.link_free_ns = start.saturating_add(ser);
        if drop {
            None
        } else {
            Some(self.link_free_ns.saturating_add(self.owd_ns))
        }
    }
}

/// Treatment arm (§5.1/§5.2).
#[derive(Debug, Clone)]
pub enum Arm {
    /// ARQ (tail-capable) + 0 parity.
    StaticR0,
    /// Same ARQ + unconditional authoritative FEC_R2 on the decision block.
    StaticR2,
    /// Frozen policy artifact decides R0/R2 from the pre-treatment snapshot.
    Calibrated(FecActivationPolicyV1),
}

impl Arm {
    /// Canonical arm tag (§10 `arm`).
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::StaticR0 => "STATIC_R0",
            Self::StaticR2 => "STATIC_R2",
            Self::Calibrated(_) => "CALIBRATED_POLICY",
        }
    }
}

/// Flow configuration (one run).
#[derive(Debug, Clone)]
pub struct FlowCfg {
    /// Cell identity (manifest key).
    pub cell_id: String,
    /// Repetition index.
    pub repetition: u32,
    /// §5.3 seed domain string (namespace-disjoint per phase).
    pub seed_domain: String,
    /// RTT cell (ms): 50 | 250 | 1000.
    pub rtt_ms: u64,
    /// InitialSource-stream loss model (shared by the R0/R2 pairing group).
    pub source_model: LossModel,
    /// Parity-stream loss model.
    pub parity_model: LossModel,
    /// ArqRetransmission-stream loss model.
    pub retx_model: LossModel,
    /// Terminal-tail overlay: drop the FIRST transmission of the last N decision-block sources.
    pub tail_len: usize,
    /// Treatment arm.
    pub arm: Arm,
    /// §4.2b estimator-only mode: end the run AT the decision epoch (no treatment, no outcome —
    /// diagnostic pre-check rows are never result-bearing).
    pub stop_at_epoch: bool,
    /// DR-CS-2 PANEL axis: `true` = P1 final FS-CAL tail-capable ARQ baseline (the FROZEN FS-CAL ARQ
    /// contract); `false` = P0 shipped product baseline (tail recovery default OFF). PANEL axis, never a
    /// treatment axis — it is held fixed within a panel.
    pub tail_on: bool,
    /// DR-RATE-RAMP-2 (§FAZ-1) treatment axis OVERRIDE. `None` ⇒ use the PRODUCT DEFAULT (now ON).
    /// `Some(false)` = RR-BASELINE arm, `Some(true)` = RR-STARTUP-PACING-INIT arm. ONE binary, one axis.
    pub startup_pacing_init: Option<bool>,
    /// v3.2 cold-start DIAGNOSTIC capture (observation-only). `None` ⇒ byte-identical legacy path:
    /// no sampling, frozen 2×horizon warm-up timeout. NEVER set for result-bearing runs.
    pub diag: Option<DiagOpts>,
}

/// v3.2 diagnostic options (observation-only; never alters sender inputs or the loss streams).
#[derive(Debug, Clone, Default)]
pub struct DiagOpts {
    /// Capture the sampled timeline + event lineage.
    pub capture: bool,
    /// Override the warm-up drain timeout (ns). `None` ⇒ frozen `2 × horizon`.
    /// Used ONLY by the FSFC2-DIAG-CS-V32 liveness diagnostic to separate
    /// HARNESS-TIME-BUDGET from PRODUCT-COLD-START-LIVENESS.
    pub warmup_timeout_override_ns: Option<u64>,
    /// Virtual-time sampling period (ns) for the timeline. 0 ⇒ no periodic samples.
    pub sample_period_ns: u64,
}

/// One periodic observation of sender + estimator state (observation-only).
#[derive(Debug, Clone, Copy)]
pub struct DiagSample {
    /// Virtual time (ns).
    pub t_ns: u64,
    /// `Sender::state()`.
    pub state: &'static str,
    /// `diag_rate().3` — pacer rate (B/s).
    pub send_rate_bps: u64,
    /// `diag_rate().1` — BtlBw estimate (B/s).
    pub btlbw_bps: u64,
    /// `diag_rate().2` — srtt (µs); 0 when no sample.
    pub srtt_us: u64,
    /// `path_timing_snapshot().rttvar_ns`.
    pub rttvar_ns: u64,
    /// `diag_rate().4` — inflight bytes.
    pub inflight_bytes: u64,
    /// `FateEstimator::fate_known()` at this instant (the fate-known time series).
    pub fate_known: u64,
    /// `now − last_ack_progress_ns` (the telemetry-age time series).
    pub telemetry_age_ns: u64,
    /// Distinct delivered source gids so far.
    pub delivered_src: u64,
    /// Cumulative `TransmitKind::Data` emissions so far.
    pub tx_data: u64,
    /// DR-RATE-RAMP-1: pacer token level (bytes).
    pub tokens: u64,
    /// DR-RATE-RAMP-1: pacer burst cap (bytes) — the §5 burst-clamp predicate input.
    pub burst_cap: u64,
    /// DR-RATE-RAMP-1: BtlBw samples accepted into the max-filter so far.
    pub bw_accepted: u64,
    /// DR-RATE-RAMP-1: BtlBw samples offered to the filter so far.
    pub bw_offered: u64,
    /// DR-RATE-RAMP-1: samples rejected because app-limited AND below current BtlBw.
    pub bw_rej_app_limited: u64,
    /// DR-RATE-RAMP-1: samples rejected because `interval < MIN_RATE_INTERVAL`.
    pub bw_rej_interval: u64,
    /// DR-RATE-RAMP-1: samples rejected because the caller passed `valid = false`.
    pub bw_rej_invalid: u64,
    /// DR-RATE-RAMP-1: fuzzy `send_rate_gain_permille` in force.
    pub send_rate_gain_permille: u64,
    /// DR-RATE-RAMP-1: packets-in-flight per the bandwidth sampler.
    pub pif: u64,
    /// DR-RATE-RAMP-2: active bootstrap pacing floor (B/s); 0 = inactive.
    pub startup_floor_bps: u64,
}

/// FAZ-1 per-source-unit ACK-chain lineage (observation-only). One record per DATA wire emission,
/// carrying every hop from sender emission to sender ACK ingest, so an `acks=0` run can be attributed
/// to an EXACT hop rather than inferred from aggregates.
#[derive(Debug, Clone, Copy, Default)]
pub struct AckChainRec {
    /// Logical source id.
    pub gid: u16,
    /// Wire seq assigned by the product sender.
    pub seq: u64,
    /// Sender wire-emission instant (ns).
    pub emit_ns: u64,
    /// FORWARD impairment decision for this emission (true = dropped on the forward path).
    pub forward_drop: bool,
    /// Receiver arrival instant (ns); `None` ⇒ never arrived (forward loss).
    pub rx_arrival_ns: Option<u64>,
    /// Receiver accepted/parsed it as a fresh source unit (harness receiver model).
    pub rx_accepted: bool,
    /// Instant an ACK became eligible and was generated by the receiver model (ns).
    pub ack_gen_ns: Option<u64>,
    /// REVERSE impairment decision for the generated ACK (harness models NO reverse loss ⇒ always false).
    pub reverse_drop: bool,
    /// Instant the ACK arrived at the sender socket (ns).
    pub ack_rx_ns: Option<u64>,
    /// The sender parsed the ACK and mapped it back to an ORIGINAL source unit.
    pub sender_parsed_original: bool,
    /// The ACK was handed to the product `Event::AckReceived`.
    pub sender_ingested: bool,
}

/// Cold-start / liveness diagnostic timeline (observation-only, v3.2 FSFC2-DIAG-CS-V32).
#[derive(Debug, Clone, Default)]
pub struct DiagTimeline {
    /// Warm-up drain timeout actually in force (ns).
    pub warmup_timeout_ns: u64,
    /// Frozen (unmodified) warm-up timeout for reference (ns) = 2×horizon.
    pub frozen_warmup_timeout_ns: u64,
    /// `(t_ns, state)` on every sender state CHANGE.
    pub state_changes: Vec<(u64, &'static str)>,
    /// First instant the sender was observed OUT of `PreConverge`.
    pub preconverge_exit_ns: Option<u64>,
    /// `Event::AppData` units handed to the sender (application enqueue count).
    pub app_enqueued: u64,
    /// Emission counts by kind (real datagram offers onto the channel).
    pub tx_data: u64,
    /// `TransmitKind::Retransmit` emissions.
    pub tx_retx: u64,
    /// `TransmitKind::Duplicate` emissions (§3.3-B geometric copies live here).
    pub tx_dup: u64,
    /// `TransmitKind::FecParity` emissions.
    pub tx_parity: u64,
    /// First real datagram emission (ns).
    pub first_tx_ns: Option<u64>,
    /// First source delivery at the receiver (ns).
    pub first_delivery_ns: Option<u64>,
    /// First ACK batch handed back to the sender (ns).
    pub first_ack_ns: Option<u64>,
    /// Number of ACK batches delivered to the sender.
    pub ack_batches: u64,
    /// Final `last_ack_progress_ns`.
    pub last_ack_progress_ns: u64,
    /// Periodic samples.
    pub samples: Vec<DiagSample>,
    /// Product tail-probe accounting at epoch: (due, sent, cancelled, bytes, spurious, timer_fires).
    pub tail_probe_diag: (u64, u64, u64, u64, u64, u64),
    /// What ENDED the warm-up phase: "DRAIN" (all 384 delivered) or "TIMEOUT".
    pub epoch_trigger: &'static str,
    /// `FateEstimator::fate_known()` at the decision epoch (pre-`drop_unresolved`).
    pub fate_known_at_epoch: u64,
    /// FIRST instant `fate_known() >= FATE_WINDOW(256)` — i.e. when a design-faithful
    /// "fate-known-ready" epoch trigger WOULD have fired. `None` ⇒ never reached.
    pub fate_known_ready_ns: Option<u64>,
    /// Distinct delivered source gids at the decision epoch.
    pub delivered_src_at_epoch: u64,
    /// Warm-up units actually offered to the sender by the offer pacer at epoch.
    pub fed_at_epoch: u64,
    /// `TailRecovery::probe_due_edge_count()` at epoch — the DEMAND signal (`probe_due` is a level poll).
    pub tail_probe_due_edge: u64,
    /// A2 disaggregated scheduler telemetry (feature `diag-sched-telemetry`), as name/value pairs.
    pub sched: Vec<(&'static str, u64)>,
    /// FAZ-1 per-DATA-emission ACK-chain lineage (bounded: warm-up emissions only).
    pub ack_chain: Vec<AckChainRec>,
}

/// Pre-treatment estimator snapshot (§10 "observed at decision epoch").
#[derive(Debug, Clone)]
pub struct EpochSnapshot {
    /// Virtual time of the decision epoch (ns).
    pub observation_epoch_ns: u64,
    /// §3.2/§3.3 window features (ids/fates + counters + derived ppm).
    pub features: WindowFeatures,
    /// §3.4 eligibility (all conditions).
    pub estimator_eligible: bool,
    /// Warm-up phase concluded by full drain (not timeout).
    pub warmup_complete: bool,
    /// Flow left PreConverge and has an RTT sample.
    pub flow_converged: bool,
    /// Telemetry staleness at epoch (grounded: FusionSafetyConfig::default().max_telemetry_age_ns).
    pub telemetry_stale: bool,
    /// `path_timing_snapshot().srtt_ns` at epoch.
    pub observed_srtt_ns: Option<u64>,
    /// `path_timing_snapshot().rttvar_ns` at epoch.
    pub observed_rttvar_ns: u64,
    /// Warm-up observables (realized, offline-only).
    pub warmup_initial_source_sent: u64,
    /// Realized warm-up initial-source drops.
    pub warmup_initial_source_lost: u64,
    /// Longest realized consecutive warm-up initial-source drop run.
    pub warmup_max_loss_run: u64,
    /// Mean realized drop-run length ×1000 (integer milli).
    pub warmup_mean_loss_run_milli: u64,
    /// Estimator correction diagnostics: (late_ack, spurious_loss).
    pub corrections: (u64, u64),
}

/// One flow's complete raw output (schema serialization happens in `fs_cal2_schema`).
#[derive(Debug, Clone)]
pub struct FlowOutput {
    /// Echo of the config identity.
    pub cfg: FlowCfg,
    /// Pre-treatment snapshot.
    pub snapshot: EpochSnapshot,
    /// Requested redundancy ("R0"/"R2").
    pub requested_r: &'static str,
    /// Effective redundancy after interlocks ("R0"/"R2").
    pub effective_r: &'static str,
    /// Policy activation score (Some only for CALIBRATED_POLICY).
    pub policy_activation_score: Option<u64>,
    /// Fallback reason code (§10).
    pub fallback_reason: &'static str,
    /// §3.4b support bins of the observed snapshot.
    pub support_bins: (u32, u32),
    /// Workload interlock outcome.
    pub workload_supported: bool,
    /// Support-bitmap membership (true for static arms only when policy absent → recorded false).
    pub support_eligible: bool,
    // ── decision-phase outcome (post-epoch) ──
    /// All 8 decision sources delivered within horizon.
    pub complete: bool,
    /// Absolute virtual completion time (ns) when complete.
    pub completion_ns: Option<u64>,
    /// Absolute censor time (epoch + horizon) when censored.
    pub censor_time_ns: Option<u64>,
    /// Horizon DURATION from epoch (ns).
    pub horizon_ns: u64,
    /// Secondary frozen deadline hit (completion − epoch ≤ deadline).
    pub deadline_hit: bool,
    /// Decision-block source wire bytes (offered).
    pub base_source_bytes: u64,
    /// Parity wire bytes (offered, FecParity kind).
    pub parity_bytes: u64,
    /// Recovery wire bytes post-epoch (Retransmit/Duplicate kinds).
    pub arq_retransmit_bytes: u64,
    /// Post-epoch recovery emission count.
    pub retransmits: u64,
    /// Tail probes actually sent post-epoch (product diag delta).
    pub probe_sent: u64,
    /// Modeled control bytes (0 — ACKs are timed events, not byte-accounted frames; fs_spine identity).
    pub control_bytes: u64,
    /// Total offered wire bytes post-epoch (egress-fair; dropped included).
    pub total_wire_bytes: u64,
    /// FEC-decoded (recovered) source count.
    pub fec_decoded_sources: u64,
    /// Any successful non-empty reconstruct.
    pub fec_decode_success: bool,
    /// All delivered decision bytes byte-equal to canonical.
    pub reassembly_equal: bool,
    /// Harness invariant failures (0 = clean).
    pub inv_fail: u64,
    /// Parity emissions observed in an R0-effective run (must be 0; invariant input).
    pub r0_parity_leak: u64,
    /// §4.3 wire-event digest (sha256 over canonical `kind|logical|seq|drop` lines, whole flow).
    pub wire_digest: String,
    /// §4.3 original-source prefix digest (sha256 over `gid|drop` lines, Data emissions only).
    pub source_prefix_digest: String,
    /// v3.2 observation-only diagnostic timeline (`Some` iff `cfg.diag.capture`).
    pub diag: Option<DiagTimeline>,
}

/// Logical identity of a wire packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Logical {
    Src(u16),
    Par(u16),
}

/// Run one flow end-to-end. Deterministic in `cfg` (virtual clock; RNG-free outside seed streams).
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn run_flow(cfg: &FlowCfg) -> FlowOutput {
    let owd_ns = cfg.rtt_ms * MS / 2;
    let horizon_ns = horizon_ms(cfg.rtt_ms) * MS;
    let frozen_warmup_timeout_ns = 2 * horizon_ns;
    // Observation-only: `None` ⇒ the frozen 2×horizon budget (legacy path, byte-identical).
    let warmup_timeout_ns = cfg
        .diag
        .as_ref()
        .and_then(|d| d.warmup_timeout_override_ns)
        .unwrap_or(frozen_warmup_timeout_ns);
    let dcap = cfg.diag.as_ref().is_some_and(|d| d.capture);
    let dperiod = cfg.diag.as_ref().map_or(0, |d| d.sample_period_ns);
    let mut dg = DiagTimeline {
        warmup_timeout_ns,
        frozen_warmup_timeout_ns,
        epoch_trigger: "NONE",
        ..DiagTimeline::default()
    };
    let mut d_last_state: &'static str = "";
    let mut d_next_sample: u64 = 0;
    let total_units = WARMUP_UNITS + BLOCK_LEN;
    let tail_cfg = tail_on_config();
    let safety = FusionSafetyConfig::default();

    // ── §5.3 deterministic per-class loss streams ─────────────────────────────
    let src_key = stream_key(
        &cfg.seed_domain,
        &cfg.cell_id,
        cfg.repetition,
        StreamClass::InitialSource,
    );
    let par_key = stream_key(
        &cfg.seed_domain,
        &cfg.cell_id,
        cfg.repetition,
        StreamClass::Parity,
    );
    let rtx_key = stream_key(
        &cfg.seed_domain,
        &cfg.cell_id,
        cfg.repetition,
        StreamClass::ArqRetransmission,
    );
    let mut src_bits = loss_bits(&src_key, cfg.source_model, total_units as usize);
    if cfg.tail_len > 0 {
        fs_cal2_gen::apply_terminal_tail(&mut src_bits, cfg.tail_len);
    }
    let par_bits = loss_bits(&par_key, cfg.parity_model, 4096);
    let rtx_bits = loss_bits(&rtx_key, cfg.retx_model, 4096);
    let mut par_idx = 0usize;
    let mut rtx_idx = 0usize;

    // ── decision-block canonical symbols + authoritative FEC_R2 pre-encode ──
    let profile = by_id(R2_PROFILE_ID).expect("profile 0x03 (K=8/E=64) in catalog");
    let block_syms: Vec<Vec<u8>> = (0..K)
        .map(|e| symbol_bytes(WARMUP_UNITS as u16 + e))
        .collect();
    let src_refs: Vec<SourceSymbol<'_>> = block_syms
        .iter()
        .enumerate()
        .map(|(e, b)| SourceSymbol {
            esi: e as u16,
            bytes: b,
        })
        .collect();
    let repairs: Vec<Vec<u8>> = RsSimdCodec
        .encode(&profile, &src_refs)
        .expect("RsSimdCodec encode (authoritative FEC_R2)")
        .into_iter()
        .take(R2_PARITY as usize)
        .map(|r| r.bytes)
        .collect();

    // ── product sender under the frozen ARQ contract ─────────────────────────
    let mut s = Sender::new();
    if cfg.tail_on {
        s.set_tail_recovery_config(tail_cfg);
    }
    if let Some(enabled) = cfg.startup_pacing_init {
        s.set_startup_pacing_init(adaptive_transport::sender::StartupPacingConfig { enabled });
    }

    let mut ch = Chan {
        link_bps: LINK_BPS,
        owd_ns,
        link_free_ns: 0,
    };
    // §2.2 offer eligibility: warm-up gids paced at 0.8C; decision gids eligible at epoch.
    let elig: Vec<u64> = (0..WARMUP_UNITS)
        .map(|g| g * SEALED * 8 * 1_000_000_000 / OFFER_PACER_BPS)
        .collect();

    // harness state
    let mut est = FateEstimator::new();
    let mut recovery_emitted: BTreeSet<u16> = BTreeSet::new();
    let mut seq_map: BTreeMap<u64, (Logical, bool /*original data*/)> = BTreeMap::new();
    let mut deliveries: Vec<(u64, Logical, u64 /*seq*/)> = Vec::new();
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut delivered_src: BTreeSet<u16> = BTreeSet::new();
    let mut blk_src: BTreeMap<u16, Vec<u8>> = BTreeMap::new(); // decision block: local esi → bytes
    let mut blk_rep: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut fed = 0u64; // next gid to feed (warm-up phase)
    let mut warmup_drop_bits: Vec<bool> = Vec::new(); // realized initial-source drops (gid order)
    let mut last_ack_progress_ns = 0u64;
    let mut now: u64 = 0;

    // decision-phase state
    let mut epoch: Option<u64> = None;
    let mut snapshot: Option<EpochSnapshot> = None;
    let mut requested_r: &'static str = "R0";
    let mut effective_r: &'static str = "R0";
    let mut policy_score: Option<u64> = None;
    let mut fallback: &'static str = PolicyFallback::StaticArm.code();
    let mut support_bins = (0u32, 0u32);
    let mut workload_supported = true;
    let mut support_eligible = false;
    let mut decision_fed = false;
    let mut block_data_emitted = 0u64;
    let mut block_closed = false;
    let mut probe_diag_at_epoch = 0u64;
    // post-epoch accounting
    let (mut base_bytes, mut parity_bytes, mut retx_bytes, mut retx_count) =
        (0u64, 0u64, 0u64, 0u64);
    let mut total_wire_post = 0u64;
    let mut r0_parity_leak = 0u64;
    let mut fec_decoded = 0u64;
    let mut fec_decode_success = false;
    let mut reassembly_equal = true;
    let mut inv_fail = 0u64;
    let mut complete_ns: Option<u64> = None;

    // guard evaluated from the live product path-timing at evidence time (§3.1)
    macro_rules! guard_now {
        () => {{
            let pt = s.path_timing_snapshot();
            reordering_guard_ns(&tail_cfg, pt.srtt_ns, pt.rttvar_ns)
        }};
    }

    // wire-event trace for §4.3 digests (canonical lines; sha256 at end)
    let mut wire_lines: Vec<String> = Vec::new();
    let mut src_prefix_lines: Vec<String> = Vec::new();

    // consume a batch of TransmitDecisions → wire offers + evidence + accounting
    macro_rules! consume {
        ($decisions:expr) => {
            for d in $decisions {
                let post = epoch.is_some();
                if dcap {
                    #[allow(unreachable_patterns)]
                    match d.kind {
                        TransmitKind::Data => dg.tx_data += 1,
                        TransmitKind::Retransmit => dg.tx_retx += 1,
                        TransmitKind::Duplicate => dg.tx_dup += 1,
                        TransmitKind::FecParity => dg.tx_parity += 1,
                        _ => {}
                    }
                    if dg.first_tx_ns.is_none() {
                        dg.first_tx_ns = Some(now);
                    }
                }
                // `_` arm: only reachable when cfg-gated TransmitKind variants (CsProbe) are compiled.
                #[allow(unreachable_patterns)]
                match d.kind {
                    TransmitKind::Data => {
                        let gid = (d.unit_id - BASE_UNIT_ID) as u16;
                        let drop = src_bits[gid as usize];
                        if (gid as u64) < WARMUP_UNITS {
                            warmup_drop_bits.push(drop);
                        }
                        if u64::from(gid) >= WARMUP_UNITS {
                            block_data_emitted += 1; // R2 close gate: decision-block Data emissions
                        }
                        seq_map.insert(d.seq, (Logical::Src(gid), true));
                        if dcap && dg.ack_chain.len() < 4096 {
                            dg.ack_chain.push(AckChainRec {
                                gid,
                                seq: d.seq,
                                emit_ns: now,
                                forward_drop: drop,
                                ..AckChainRec::default()
                            });
                        }
                        wire_lines.push(format!("D|{gid}|{}|{}", d.seq, u8::from(drop)));
                        src_prefix_lines.push(format!("{gid}|{}", u8::from(drop)));
                        if post {
                            base_bytes += SEALED;
                            total_wire_post += SEALED;
                        }
                        if let Some(arr) = ch.offer(SEALED, now, drop) {
                            deliveries.push((arr, Logical::Src(gid), d.seq));
                        }
                    }
                    TransmitKind::Retransmit | TransmitKind::Duplicate => {
                        // recovery lineage (§3.1 LOSS evidence) for source logicals
                        let logical = if d.unit_id >= BASE_UNIT_ID {
                            let gid = (d.unit_id - BASE_UNIT_ID) as u16;
                            recovery_emitted.insert(gid);
                            est.on_loss_evidence(u64::from(gid), now, guard_now!());
                            Logical::Src(gid)
                        } else {
                            Logical::Par(d.unit_id as u16) // parity retransmit (single decision block)
                        };
                        let drop = rtx_bits[rtx_idx.min(rtx_bits.len() - 1)];
                        rtx_idx += 1;
                        seq_map.insert(d.seq, (logical, false));
                        wire_lines.push(format!("R|{logical:?}|{}|{}", d.seq, u8::from(drop)));
                        if post {
                            retx_bytes += SEALED;
                            retx_count += 1;
                            total_wire_post += SEALED;
                        }
                        if let Some(arr) = ch.offer(SEALED, now, drop) {
                            deliveries.push((arr, logical, d.seq));
                        }
                    }
                    TransmitKind::FecParity => {
                        let esi = match d.send_record {
                            Some(h) => match h.logical_ref {
                                SrLogicalRef::Fec { esi, .. } => esi,
                            },
                            None => d.unit_id as u16,
                        };
                        if effective_r == "R0" {
                            r0_parity_leak += 1; // invariant: R0 must never emit parity
                        }
                        let drop = par_bits[par_idx.min(par_bits.len() - 1)];
                        par_idx += 1;
                        seq_map.insert(d.seq, (Logical::Par(esi), false));
                        wire_lines.push(format!("P|{esi}|{}|{}", d.seq, u8::from(drop)));
                        if post {
                            parity_bytes += SEALED;
                            total_wire_post += SEALED;
                        }
                        if let Some(arr) = ch.offer(SEALED, now, drop) {
                            deliveries.push((arr, Logical::Par(esi), d.seq));
                        }
                    }
                    _ => {}
                }
            }
        };
    }

    loop {
        // 1) feed eligible AppData
        if epoch.is_none() {
            while fed < WARMUP_UNITS && elig[fed as usize] <= now {
                let out = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id: BASE_UNIT_ID + fed,
                        size: SEALED,
                    }),
                    now,
                );
                fed += 1;
                if dcap {
                    dg.app_enqueued += 1;
                }
                consume!(out);
            }
        } else if !decision_fed {
            decision_fed = true;
            for g in WARMUP_UNITS..total_units {
                let out = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id: BASE_UNIT_ID + g,
                        size: SEALED,
                    }),
                    now,
                );
                consume!(out);
            }
        }

        // 1d) v3.2 observation-only diagnostic capture. Reads sender/estimator state ONLY — feeds no
        // event, consumes no loss stream, and is fully skipped when `dcap == false` (legacy path).
        if dcap {
            let st = state_name(s.state());
            if st != d_last_state {
                dg.state_changes.push((now, st));
                d_last_state = st;
                if st != "PreConverge" && dg.preconverge_exit_ns.is_none() {
                    dg.preconverge_exit_ns = Some(now);
                }
            }
            let fk = est.fate_known() as u64;
            if fk >= FATE_WINDOW_LEN && dg.fate_known_ready_ns.is_none() {
                dg.fate_known_ready_ns = Some(now);
            }
            if dperiod > 0 && now >= d_next_sample {
                d_next_sample = now + dperiod;
                let (rs, btlbw, srtt_us, rate, ifb) = s.diag_rate();
                let pt = s.path_timing_snapshot();
                let (_prate, tokens, burst_cap) = s.diag_pacer();
                #[cfg(feature = "diag-sched-telemetry")]
                let (bwo, bwa, bwri, bwrint, bwral) = s.diag_bw_accept();
                #[cfg(not(feature = "diag-sched-telemetry"))]
                let (bwo, bwa, bwri, bwrint, bwral) = (0u64, 0u64, 0u64, 0u64, 0u64);
                dg.samples.push(DiagSample {
                    t_ns: now,
                    state: state_name(rs),
                    send_rate_bps: rate,
                    btlbw_bps: btlbw,
                    srtt_us,
                    rttvar_ns: pt.rttvar_ns,
                    inflight_bytes: ifb,
                    fate_known: fk,
                    telemetry_age_ns: now.saturating_sub(last_ack_progress_ns),
                    delivered_src: delivered_src.len() as u64,
                    tx_data: dg.tx_data,
                    tokens,
                    burst_cap,
                    bw_accepted: bwa,
                    bw_offered: bwo,
                    bw_rej_app_limited: bwral,
                    bw_rej_interval: bwrint,
                    bw_rej_invalid: bwri,
                    send_rate_gain_permille: u64::from(
                        s.last_decision().send_rate_gain_permille,
                    ),
                    pif: s.diag_pif(),
                    startup_floor_bps: s.startup_pacing_floor_bps(),
                });
            }
        }

        // 2) R2 treatment: authoritative product close once all K decision sources were emitted
        if epoch.is_some()
            && effective_r == "R2"
            && !block_closed
            && block_data_emitted >= BLOCK_LEN
        {
            block_closed = true;
            s.set_manual_fec_enabled(true)
                .expect("manual FEC authority");
            let auth = s.open_fec_block_authority();
            let staged = s
                .close_fec_block_manual(auth, DECISION_BLOCK_ID, R2_PARITY, SEALED)
                .expect("close_fec_block_manual");
            if staged != R2_PARITY {
                inv_fail += 1;
            }
        }

        // 3) advance virtual time
        now += DT_NS;

        // 4) deliveries due
        let mut recovered_now: Vec<u16> = Vec::new();
        let mut due_deliv: Vec<(u64, Logical, u64)> = Vec::new();
        deliveries.retain(|&(arr, l, q)| {
            if arr <= now {
                due_deliv.push((arr, l, q));
                false
            } else {
                true
            }
        });
        for (arr, logical, seq) in due_deliv {
            if dcap && dg.first_delivery_ns.is_none() {
                dg.first_delivery_ns = Some(now);
            }
            if dcap {
                if let Some(r) = dg.ack_chain.iter_mut().find(|r| r.seq == seq) {
                    r.rx_arrival_ns = Some(now);
                    r.rx_accepted = true;
                    // harness receiver model: EVERY delivered packet is ACK-eligible immediately,
                    // and the ACK is generated one OWD later. NO reverse-path loss is modelled.
                    r.ack_gen_ns = Some(arr.saturating_add(owd_ns));
                    r.reverse_drop = false;
                }
            }
            match logical {
                Logical::Src(gid) => {
                    if delivered_src.insert(gid) && gid as u64 >= WARMUP_UNITS {
                        let e = gid - WARMUP_UNITS as u16;
                        blk_src.insert(e, block_syms[e as usize].clone());
                    }
                    // count decision-block Data emissions for the close gate (first arrival isn't
                    // the gate — emission is; tracked here via seq_map original flag at emit time)
                }
                Logical::Par(esi) => {
                    if (esi as usize) < repairs.len() {
                        blk_rep
                            .entry(esi)
                            .or_insert_with(|| repairs[esi as usize].clone());
                    }
                }
            }
            acks.push((arr.saturating_add(owd_ns), seq));
        }

        // decision-block counting-MDS decode (FS-1/R2A rule) — every tick
        if epoch.is_some() {
            let distinct = blk_src.len();
            if distinct < K as usize
                && !blk_rep.is_empty()
                && distinct + blk_rep.len() >= K as usize
            {
                let mut received: Vec<ReceivedSymbol<'_>> = Vec::new();
                for (e, b) in &blk_src {
                    received.push(ReceivedSymbol {
                        esi: *e,
                        is_repair: false,
                        bytes: b,
                    });
                }
                for (e, b) in &blk_rep {
                    received.push(ReceivedSymbol {
                        esi: *e,
                        is_repair: true,
                        bytes: b,
                    });
                }
                if let Ok(rec) = RsSimdCodec.reconstruct(&profile, &received, 0) {
                    if !rec.is_empty() {
                        fec_decode_success = true;
                    }
                    for r in rec {
                        let gid = WARMUP_UNITS as u16 + r.esi;
                        if r.bytes != block_syms[r.esi as usize] {
                            reassembly_equal = false;
                            inv_fail += 1;
                        }
                        if delivered_src.insert(gid) {
                            blk_src.insert(r.esi, r.bytes);
                            fec_decoded += 1;
                            recovered_now.push(gid);
                        }
                    }
                }
            }
        }

        // 5) recovered → estimator evidence + product RecoveredControl (pure-logical in sender)
        if !recovered_now.is_empty() {
            est_evidence_recovered(&mut est, &recovered_now, now, guard_now!());
            let units = recovered_now
                .iter()
                .map(|&gid| adaptive_transport::sender::RecoveredUnit {
                    unit_id: BASE_UNIT_ID + u64::from(gid),
                    block_id: DECISION_BLOCK_ID,
                    esi: gid - WARMUP_UNITS as u16,
                })
                .collect();
            let out = s.on_event(
                Event::RecoveredControl(adaptive_transport::sender::RecoveredInfo { units }),
                now,
            );
            consume!(out);
        }

        // 6) due ACKs → sender (one batch) + estimator SUCCESS evidence
        let mut due_acks: Vec<u64> = Vec::new();
        acks.retain(|&(t, q)| {
            if t <= now {
                due_acks.push(q);
                false
            } else {
                true
            }
        });
        if !due_acks.is_empty() {
            if dcap {
                dg.ack_batches += 1;
                if dg.first_ack_ns.is_none() {
                    dg.first_ack_ns = Some(now);
                }
            }
            last_ack_progress_ns = now;
            for q in &due_acks {
                if dcap {
                    if let Some(r) = dg.ack_chain.iter_mut().find(|r| r.seq == *q) {
                        r.ack_rx_ns = Some(now);
                        r.sender_ingested = true;
                        r.sender_parsed_original =
                            matches!(seq_map.get(q), Some(&(Logical::Src(_), true)));
                    }
                }
                if let Some(&(Logical::Src(gid), original)) = seq_map.get(q) {
                    if original {
                        est.on_original_ack(u64::from(gid));
                    }
                }
            }
            let out = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: due_acks,
                    ack_delay: Duration::ZERO,
                }),
                now,
            );
            consume!(out);
        }

        // 7) unconditional per-tick PTO (fs_spine cadence identity)
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        consume!(out);
        est.advance(now);

        // 8) phase transitions
        if epoch.is_none() {
            let drained = delivered_src.len() as u64 >= WARMUP_UNITS
                && (0..WARMUP_UNITS as u16).all(|g| delivered_src.contains(&g));
            let timed_out = now >= warmup_timeout_ns;
            if (fed >= WARMUP_UNITS && drained) || timed_out {
                if dcap {
                    dg.epoch_trigger = if fed >= WARMUP_UNITS && drained {
                        "DRAIN"
                    } else {
                        "TIMEOUT"
                    };
                    dg.fate_known_at_epoch = est.fate_known() as u64;
                    dg.delivered_src_at_epoch = delivered_src.len() as u64;
                    dg.fed_at_epoch = fed;
                    dg.tail_probe_diag = s.tail_probe_diag_ext();
                    dg.tail_probe_due_edge = s.tail_probe_due_edge_count();
                    #[cfg(feature = "diag-sched-telemetry")]
                    {
                        let d = s.sched_diag();
                        dg.sched = vec![
                            ("data_enqueued", d.data_enqueued),
                            ("data_selected", d.data_selected),
                            ("data_token_denied", d.data_token_denied),
                            ("data_wire_emitted", d.data_wire_emitted),
                            ("arq_retransmit_selected", d.arq_retransmit_selected),
                            ("arq_retransmit_token_denied", d.arq_retransmit_token_denied),
                            ("arq_retransmit_wire_emitted", d.arq_retransmit_wire_emitted),
                            ("dup_selected", d.dup_selected),
                            ("dup_token_denied", d.dup_token_denied),
                            ("dup_wire_emitted", d.dup_wire_emitted),
                            ("tail_probe_selected", d.tail_probe_selected),
                            ("tail_probe_token_denied", d.tail_probe_token_denied),
                            ("tail_probe_wire_emitted", d.tail_probe_wire_emitted),
                            ("tail_probe_enqueued", d.tail_probe_enqueued),
                            ("cs_probe_selected", d.cs_probe_selected),
                            ("cs_probe_token_denied", d.cs_probe_token_denied),
                            ("cs_probe_wire_emitted", d.cs_probe_wire_emitted),
                            ("cs_probe_enqueued", d.cs_probe_enqueued),
                        ];
                    }
                    dg.last_ack_progress_ns = last_ack_progress_ns;
                }
                // §2.3 decision epoch: finalize estimator state, take the pre-treatment snapshot.
                est.advance(now);
                est.drop_unresolved_as_unknown();
                let features = est.snapshot();
                let pt = s.path_timing_snapshot();
                let converged = s.state() != adaptive_transport::sender::SenderState::PreConverge
                    && pt.sample_present;
                let stale = now.saturating_sub(last_ack_progress_ns) > safety.max_telemetry_age_ns;
                let warmup_complete = drained && !timed_out;
                let eligible = features.eligible(warmup_complete, stale, converged);
                let (mlr, mml) = loss_runs(&warmup_drop_bits);
                let snap = EpochSnapshot {
                    observation_epoch_ns: now,
                    features: features.clone(),
                    estimator_eligible: eligible,
                    warmup_complete,
                    flow_converged: converged,
                    telemetry_stale: stale,
                    observed_srtt_ns: pt.srtt_ns,
                    observed_rttvar_ns: pt.rttvar_ns,
                    warmup_initial_source_sent: warmup_drop_bits.len() as u64,
                    warmup_initial_source_lost: warmup_drop_bits.iter().filter(|&&b| b).count()
                        as u64,
                    warmup_max_loss_run: mlr,
                    warmup_mean_loss_run_milli: mml,
                    corrections: est.corrections(),
                };
                support_bins = fs_cal2_policy::support_bins(
                    snap.features.measured_loss_ppm,
                    snap.features.burst_delta_ppm,
                );
                // arm resolution (§3.4b interlock order)
                match &cfg.arm {
                    Arm::StaticR0 => {
                        requested_r = "R0";
                        effective_r = "R0";
                        fallback = PolicyFallback::StaticArm.code();
                    }
                    Arm::StaticR2 => {
                        requested_r = "R2";
                        effective_r = "R2";
                        fallback = PolicyFallback::StaticArm.code();
                    }
                    Arm::Calibrated(pol) => {
                        let d = pol.decide(
                            snap.features.measured_loss_ppm,
                            snap.features.burst_delta_ppm,
                            eligible,
                            WORKLOAD_PROFILE_ID,
                            ARQ_CONTRACT_ID,
                        );
                        support_eligible = pol.support.contains(&support_bins);
                        workload_supported = pol.workload_profile_id == WORKLOAD_PROFILE_ID;
                        policy_score = Some(d.activation_score);
                        requested_r = if d.request_r2 { "R2" } else { "R0" };
                        // safety caps (grounded FusionSafetyConfig): parity 2×101 within budgets
                        let parity_total = u64::from(R2_PARITY) * SEALED;
                        let caps_ok = parity_total <= safety.hard_fec_byte_budget
                            && parity_total <= safety.hard_combined_byte_cap;
                        effective_r = if d.request_r2 && caps_ok { "R2" } else { "R0" };
                        fallback = if d.request_r2 && !caps_ok {
                            "SAFETY_CAP"
                        } else {
                            d.fallback.code()
                        };
                    }
                }
                probe_diag_at_epoch = s.tail_probe_diag_ext().1;
                epoch = Some(now);
                snapshot = Some(snap);
                if cfg.stop_at_epoch {
                    break; // §4.2b diagnostic: estimator telemetry only — no treatment/outcome
                }
            }
        } else if let Some(ep) = epoch {
            let done =
                (WARMUP_UNITS as u16..total_units as u16).all(|g| delivered_src.contains(&g));
            if done && complete_ns.is_none() {
                complete_ns = Some(now);
            }
            if done || now.saturating_sub(ep) >= horizon_ns {
                break;
            }
        }
        // absolute safety stop (never loops unbounded)
        if now > warmup_timeout_ns + horizon_ns + 2 * horizon_ns {
            inv_fail += 1;
            break;
        }
    }

    let ep = epoch.expect("epoch always reached");
    let snap = snapshot.expect("snapshot taken at epoch");
    let probe_sent = s
        .tail_probe_diag_ext()
        .1
        .saturating_sub(probe_diag_at_epoch);
    let complete = complete_ns.is_some();
    let deadline_hit = complete_ns
        .map(|t| t.saturating_sub(ep) <= deadline_ms(cfg.rtt_ms) * MS)
        .unwrap_or(false);
    if effective_r == "R0" && parity_bytes > 0 {
        inv_fail += 1;
    }
    // static support fields for static arms (no policy bitmap): recorded as computed bins, false
    FlowOutput {
        cfg: cfg.clone(),
        snapshot: snap,
        requested_r,
        effective_r,
        policy_activation_score: policy_score,
        fallback_reason: fallback,
        support_bins,
        workload_supported,
        support_eligible,
        complete,
        completion_ns: complete_ns,
        censor_time_ns: if complete {
            None
        } else {
            Some(ep + horizon_ns)
        },
        horizon_ns,
        deadline_hit,
        base_source_bytes: base_bytes,
        parity_bytes,
        arq_retransmit_bytes: retx_bytes,
        retransmits: retx_count,
        probe_sent,
        control_bytes: 0,
        total_wire_bytes: total_wire_post,
        fec_decoded_sources: fec_decoded,
        fec_decode_success,
        reassembly_equal,
        inv_fail: inv_fail + u64::from(!reassembly_equal),
        r0_parity_leak,
        wire_digest: fs_cal2_gen::sha256_hex(wire_lines.join("\n").as_bytes()),
        source_prefix_digest: fs_cal2_gen::sha256_hex(src_prefix_lines.join("\n").as_bytes()),
        diag: if dcap { Some(dg) } else { None },
    }
}

/// Receiver recovered-source evidence (§3.1): LOSS lineage with guard.
fn est_evidence_recovered(est: &mut FateEstimator, gids: &[u16], now: u64, guard: u64) {
    for &g in gids {
        est.on_loss_evidence(u64::from(g), now, guard);
    }
}

/// Realized drop-run statistics over the warm-up initial-source bit sequence:
/// (max run length, mean run length ×1000).
#[must_use]
pub fn loss_runs(bits: &[bool]) -> (u64, u64) {
    let (mut max, mut cur, mut runs, mut total) = (0u64, 0u64, 0u64, 0u64);
    for &b in bits {
        if b {
            cur += 1;
            max = max.max(cur);
        } else if cur > 0 {
            runs += 1;
            total += cur;
            cur = 0;
        }
    }
    if cur > 0 {
        runs += 1;
        total += cur;
    }
    let mean_milli = total.saturating_mul(1000).checked_div(runs).unwrap_or(0);
    (max, mean_milli)
}
