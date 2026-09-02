//! DR-CS-3 §2 — **DEFAULT-BUILD AUTHENTICATED TRANSPORT COMPONENT PANEL
//! WITH TEST-INJECTED SESSION KEYS**
//!
//! A panel harness **separate from `fs_cal2_spine`** (which has no `ReceiverControlCore` and is NOT
//! silently converted into a product-path harness). This one drives:
//!
//! * the real product `Sender` (default features),
//! * the real default-build [`ReceiverControlCore`],
//! * the real `kama-transport-schema` control codec,
//! * real AEAD seal/open (`KamaNoiseWireProtector`) with **test-injected directional session keys**,
//! * a deterministic forward/reverse loss model,
//! * real probe/echo ingest, the real RTT estimator, the real PreConverge exit and the real
//!   startup-pacing handoff.
//!
//! **SCOPE (binding).** This panel does NOT verify the Noise handshake, does NOT verify a product
//! binary, does NOT verify a deployment network path, and does NOT discharge the product-runtime
//! debt (`PRODUCT-RUNTIME-INTEGRATION-PENDING`).
//!
//! **SEED/LOSS IDENTITY (V2).** The draw primitives are the FS-CAL-2 §5.3 ones re-used verbatim
//! from `fs_cal2_gen` (`draw_u64`, `ppm_hit`, `ge_transitions`, `markov_bd_transitions`), but the
//! per-class precomputed streams are GONE. Contract `DR-CS-3-PANEL-CONTRACT-V2` (md `0abbb114…`)
//! fixes ONE chain per DIRECTION, advanced once per traversing packet in chronological emission
//! order: forward carries DATA + ARQ retransmissions + Probe, reverse carries ACK + Echo. There is
//! no packet-class stream and no treatment-only impairment.
//!
//! The workload, channel, warm-up length, offer pacing, virtual tick and horizon constants are the
//! frozen FS-CAL-2 / R2A identity values, restated here so this module owns no dependency on the
//! spine.
#![allow(dead_code)]

#[path = "fs_cal2_gen.rs"]
pub mod fs_cal2_gen;

use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use fs_cal2_gen::{sha256_hex, LossModel};
use kama_integration::receiver_control_core::{ECHO_BUDGET_BYTES, MAX_ECHOES};
use kama_integration::{CoreIngest, ReceiverControlCore};
use kama_noise_wire::control::decode_control;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::UnsealedTransportPacket;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroizing;

// ── frozen FS-CAL-2 / R2A workload + channel identity (restated, not imported) ───────────────────
/// ns per ms.
pub const MS: u64 = 1_000_000;
/// Sealed wire unit (E=64 + 37 seal overhead).
pub const SEALED: u64 = 101;
/// Warm-up length (original source packets), PINNED.
pub const WARMUP_UNITS: u64 = 384;
/// Source unit-id base.
pub const BASE_UNIT_ID: u64 = 1000;
/// Bottleneck link (bits/s).
pub const LINK_BPS: u64 = 1_000_000;
/// Source-offer eligibility pacer (bits/s) — 0.80C.
pub const OFFER_PACER_BPS: u64 = 800_000;
/// Virtual tick (ns).
pub const DT_NS: u64 = 10_000;
/// Session handle used by the panel (component-local; not a wire allocation policy).
pub const PANEL_H_EXT: u32 = 0x0000_C530;
/// Pinned probe wire size (schema `PROBE_WIRE_EXACT`).
pub const PROBE_WIRE_EXACT: u64 = 44;
/// Pinned echo wire size (schema `ECHO_WIRE_MAX`).
pub const ECHO_WIRE_MAX: u64 = 43;
/// Sender-side bootstrap wire budget (`BS_TX_HARD_BUDGET`).
pub const SENDER_PROBE_BUDGET: u64 = 200;

/// Frozen per-RTT observation horizon (ms).
#[must_use]
pub fn horizon_ms(rtt_ms: u64) -> u64 {
    match rtt_ms {
        50 => 699,
        250 => 2499,
        1000 => 9249,
        other => panic!("unsupported RTT cell {other} (frozen cells: 50/250/1000)"),
    }
}

/// FROZEN product tail-recovery ON config (DR-ARQ-PRODUCT-TAIL-1 verbatim values).
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

/// Egress-fair FIFO bottleneck (every offered packet serializes — dropped included; FIFO ⇒ no
/// reordering). Identical semantics to the FS-CAL-2 channel.
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

/// **V2 CHANNEL (contract `DR-CS-3-PANEL-CONTRACT-V2`, md `0abbb114…`).**
///
/// ONE channel state per DIRECTION, advanced **chronologically, once per packet that actually
/// traverses it**. There is no per-class stream and no treatment-only impairment: DATA, ARQ
/// retransmissions and Probes all consume the FORWARD chain in emission order; ACKs and Echoes all
/// consume the REVERSE chain in emission order.
///
/// GE stays **packet-domain**: wall-clock gaps never synthesise draws. Two packets 24 s apart
/// consume two consecutive steps exactly as two packets 1 ms apart would — the model says nothing
/// about elapsed time, and inventing a time-decay here would be a post-hoc change made after seeing
/// V1's outcome.
pub struct GeChain {
    key: String,
    model: LossModel,
    idx: u64,
    /// GE bad-state / MARKOV-BD previous-loss carrier.
    bad: bool,
}

impl GeChain {
    #[must_use]
    pub fn new(key: String, model: LossModel) -> Self {
        Self {
            key,
            model,
            idx: 0,
            bad: false,
        }
    }

    /// Advance EXACTLY one packet-domain step; `true` = this packet is dropped.
    ///
    /// The transition algebra is the frozen FS-CAL-2 §3.5/§6.0 one, stepped incrementally instead of
    /// precomputed, so a single chain reproduces the frozen generator draw for draw.
    pub fn next_drop(&mut self) -> bool {
        let i = self.idx;
        self.idx += 1;
        match self.model {
            LossModel::None => false,
            LossModel::Iid { p_ppm } => {
                fs_cal2_gen::ppm_hit(fs_cal2_gen::draw_u64(&self.key, i), p_ppm)
            }
            LossModel::MarkovBd { p_ppm, b_ppm } => {
                let (p11, p01) = fs_cal2_gen::markov_bd_transitions(p_ppm, b_ppm);
                let d = fs_cal2_gen::draw_u64(&self.key, i);
                self.bad = if i == 0 {
                    fs_cal2_gen::ppm_hit(d, p_ppm)
                } else if self.bad {
                    fs_cal2_gen::ppm_hit(d, p11)
                } else {
                    fs_cal2_gen::ppm_hit(d, p01)
                };
                self.bad
            }
            LossModel::Ge { p_ppm, l } => {
                let (pgb, pbg) = fs_cal2_gen::ge_transitions(p_ppm, l);
                let d = fs_cal2_gen::draw_u64(&self.key, i);
                self.bad = if i == 0 {
                    fs_cal2_gen::ppm_hit(d, p_ppm)
                } else if self.bad {
                    !fs_cal2_gen::ppm_hit(d, pbg)
                } else {
                    fs_cal2_gen::ppm_hit(d, pgb)
                };
                self.bad
            }
        }
    }

    /// Packets that have traversed this direction so far (observation-only).
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.idx
    }

    /// The chain's carried GE bad-state / MARKOV-BD previous-loss bit (observation-only).
    ///
    /// A pure `&self` read of already-existing state: it consumes no draw, advances no index and
    /// cannot change any fate. It exists so the channel-event log can record `ge_state_before` and
    /// `ge_state_after` around a real decision (addendum §2 pre-treatment prefix list). For
    /// [`LossModel::None`] and [`LossModel::Iid`] the bit is structurally always `false` — those
    /// models carry no memory — and the field is honest about that rather than synthesising one.
    #[must_use]
    pub fn bad_state(&self) -> bool {
        self.bad
    }
}

/// Direction keys. Distinct per DIRECTION only — never per packet class, never per treatment arm.
#[must_use]
pub fn forward_key(domain: &str, cell_id: &str, repetition: u32) -> String {
    format!("{domain}|{cell_id}|rep{repetition}|V2Forward")
}
#[must_use]
pub fn reverse_key(domain: &str, cell_id: &str, repetition: u32) -> String {
    format!("{domain}|{cell_id}|rep{repetition}|V2Reverse")
}

/// Panel configuration (one run). Exactly ONE treatment axis: [`PanelCfg::bootstrap_on`].
#[derive(Debug, Clone)]
pub struct PanelCfg {
    /// Cell identity.
    pub cell_id: String,
    /// Repetition index (seed).
    pub repetition: u32,
    /// §5.3 seed domain.
    pub seed_domain: String,
    /// RTT cell (ms).
    pub rtt_ms: u64,
    /// **V2**: the ONE forward channel model — DATA, ARQ retransmissions and Probes all traverse it.
    pub forward_model: LossModel,
    /// **V2**: the ONE reverse channel model — ACKs and Echoes both traverse it.
    /// `LossModel::None` = the primary continuity panel (reverse lossless, matching the frozen cell).
    pub reverse_model: LossModel,
    /// PANEL axis (held fixed within a panel): tail recovery OFF (P0) / ON (P1).
    pub tail_on: bool,
    /// **TREATMENT axis** — the only thing that differs between the two arms of a panel.
    pub bootstrap_on: bool,
    /// Warm-up drain timeout (ns).
    pub warmup_timeout_ns: u64,
    /// §6 targeted override: explicit forward-drop script per probe generation.
    pub probe_drop_script: Option<Vec<bool>>,
    /// §6 targeted override: explicit reverse-drop script per echo generation.
    pub echo_drop_script: Option<Vec<bool>>,
    /// §6 targeted: additionally deliver each echo N extra times (byte-identical duplicates).
    pub echo_wire_duplicates: u32,
    /// §6 targeted: additionally inject N direct duplicate `CsProbeEcho` events (models a duplicate
    /// that survived the AEAD layer), to exercise the SENDER-side single-consumption guard.
    pub echo_event_duplicates: u32,
}

impl PanelCfg {
    /// Bootstrap-arm variant of this configuration (the single treatment axis).
    #[must_use]
    pub fn with_bootstrap(&self, on: bool) -> Self {
        let mut c = self.clone();
        c.bootstrap_on = on;
        c
    }
}

/// One panel run's observations.
///
/// `PartialEq`/`Eq` are derived for ONE purpose: the logger-off vs logger-on neutrality assertion
/// (addendum §7). Comparing two `PanelOut`s compares the whole observable trace — wire digest,
/// source-prefix digest, every emission counter, both directional draw counts, the terminal
/// classification and the full lineage — so a behavioural difference outside the logger's own
/// records cannot hide behind a field nobody thought to name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelOut {
    /// `DRAIN` | `TIMEOUT`.
    pub epoch_trigger: &'static str,
    /// Virtual time at the decision epoch.
    pub observation_epoch_ns: u64,
    /// First instant the sender left `PreConverge`.
    pub preconverge_exit_ns: Option<u64>,
    /// **Primary outcome:** the sender never left PreConverge within the observation window.
    pub cold_start_stall: bool,
    /// All 384 warm-up sources drained before the timeout.
    pub warmup_complete: bool,
    /// Distinct delivered warm-up sources at the epoch.
    pub delivered_src_at_epoch: u64,
    /// `TransmitKind::Data` emissions.
    pub tx_data: u64,
    /// `TransmitKind::Retransmit` emissions.
    pub tx_retx: u64,
    /// `TransmitKind::Duplicate` emissions.
    pub tx_dup: u64,
    /// ACK batches ingested.
    pub ack_batches: u64,
    /// First ACK ingest.
    pub first_ack_ns: Option<u64>,
    /// First delivery.
    pub first_delivery_ns: Option<u64>,
    /// DATA wire bytes.
    pub data_wire_bytes: u64,
    /// Retransmission wire bytes.
    pub retx_wire_bytes: u64,

    // ── bootstrap lineage (§4 exact chain) ──────────────────────────────────────────────────
    /// `bs_armed_ns` — the bootstrap edge (first DATA wire emission in PreConverge, srtt=None).
    pub bootstrap_edge_ns: Option<u64>,
    /// Probes emitted by the product sender (BOTH origins — the two are byte-indistinguishable on
    /// the wire and share `TransmitKind::CsProbe`).
    pub probes_emitted: u64,
    /// SELECTION-BUILD-AXIS-ADDENDUM §3/§4 origin attribution: probes the SHIPPING bootstrap
    /// emitter claims (`bootstrap_rtt_diag().probe_emitted`). `probes_emitted - this` is the LEGACY
    /// cold-start ladder's share, which exists only where `experimental-cold-start` is compiled and
    /// is gated to the bootstrap-OFF arm (`sender.rs:2996-2999`). A non-zero value of THIS counter
    /// in the OFF arm would be a shipping-gate leak — a product finding, never excusable.
    pub shipping_probes_emitted: u64,
    /// Probe wire bytes emitted.
    pub probe_wire_bytes: u64,
    /// Probes that arrived and **authenticated** at the receiver core.
    pub probes_authenticated: u64,
    /// Echo generations produced by the bounded responder.
    pub echoes_generated: u64,
    /// Echo wire bytes sealed.
    pub echo_wire_bytes: u64,
    /// Sealed echoes that arrived at the sender and decoded.
    pub echoes_arrived: u64,
    /// `echo_match_valid` from the product diag.
    pub echo_match_valid: u64,
    /// `echo_match_rejected` from the product diag.
    pub echo_match_rejected: u64,
    /// The matched local RTT sample (ns).
    pub rtt_sample_ns: u64,
    /// Instant `srtt` became `Some`.
    pub srtt_set_ns: Option<u64>,
    /// Startup pacing floor active immediately after the PreConverge exit (B/s).
    pub startup_floor_at_exit_bps: u64,
    /// Full lineage of the successful chain, in order.
    pub lineage: Vec<String>,

    // ── safety / accounting ────────────────────────────────────────────────────────────────
    /// Sender probe budget exceeded (must be 0).
    pub sender_budget_violation: u64,
    /// Receiver echo budget or generation cap exceeded (must be 0).
    pub receiver_budget_violation: u64,
    /// AEAD/authentication failures on a legitimately delivered frame (must be 0).
    pub auth_failures: u64,
    /// Echo frames rejected by the AEAD replay window (expected for wire duplicates).
    pub echo_replay_rejected: u64,
    /// More than one accepted RTT sample per bootstrap epoch (must be 0).
    pub duplicate_rtt_samples: u64,
    /// Probe emissions after the bootstrap already succeeded (must be 0).
    pub probe_after_success: u64,
    /// Tail-recovery diagnostics `(due, sent, cancelled)`.
    pub tail_probe: (u64, u64, u64),
    /// Tick indices at which a DATA emission occurred (for the paired displacement check).
    pub data_emit_ticks: Vec<u64>,
    /// Tick index of the first accepted echo (`u64::MAX` if none) — echo-bounded, see the metric
    /// contract: consumers must report `N/A — NO ECHO` rather than treating MAX as a horizon.
    pub first_echo_tick: u64,
    /// Tick index of the first Probe reaching the wire (`u64::MAX` if none). This, not the echo, is
    /// the boundary between "arms must be identical" and "divergence is legitimate physics".
    pub first_probe_tick: u64,
    /// Canonical wire trace digest.
    pub wire_digest: String,
    /// Canonical source-prefix digest (DATA emissions only).
    pub source_prefix_digest: String,
    /// ACKs dropped by the REVERSE channel (0 on the primary continuity panel).
    pub acks_reverse_dropped: u64,
    /// ACKs OFFERED to the reverse channel (delivered + dropped).
    pub acks_generated: u64,
    /// 2.1 chain cardinality: the ONE forward chain key used for the whole run.
    pub forward_chain_key: String,
    /// 2.1 chain cardinality: the ONE reverse chain key used for the whole run.
    pub reverse_chain_key: String,
    /// 2.3: a datagram type that bypassed both directional chains (MUST stay 0).
    pub channel_bypass_violations: u64,
    /// 2.2 tie-break trace: `(kind, forward chain index)` per forward emission, bounded.
    pub forward_order: Vec<(char, u64)>,
    /// Packets that traversed the forward chain (observation-only).
    pub forward_steps: u64,
    /// Packets that traversed the reverse chain (observation-only).
    pub reverse_steps: u64,
    /// V2 outcome class: `RTT-OPPORTUNITY` | `NETWORK-CENSORED` | `NO-BOOTSTRAP`.
    pub outcome_class: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Logical {
    Src(u16),
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// DIRECTIONAL CHANNEL EVENT LOG — OBSERVATION-ONLY INSTRUMENT
//
// Frozen contract: `reports/dr-cs-3/panel-v2/DR-CS-3-CHANNEL-FATE-INTERPRETATION-ADDENDUM.md`
// sha256 `c08254d0f598c111ecc1fb7c5895e574e318d5e0914419f87f89f24a61859b77`.
// Companion: `DR-CS-3-SELECTION-BUILD-AXIS-ADDENDUM.md` sha256 `1e56423e…`.
//
// **INSTRUMENT NEUTRALITY (addendum §7) — the whole design constraint.** The logger:
//   * produces no packet — it never calls `Chan::offer`, `Sender::on_event` or any emit path;
//   * advances no channel state and consumes no draw — it only calls `GeChain::steps()` and
//     `GeChain::bad_state()`, both `&self`;
//   * changes no event ordinal and no packet order — every record is appended AFTER the
//     `next_drop()` that produced it and BEFORE the surrounding statement sequence continues;
//   * invokes no callback and touches no scheduler or pacer state;
//   * uses no wall-clock and no worker-completion order (`run_panel` is single-threaded and its
//     only clock is the virtual `now`);
//   * never truncates — a cap breach is reported as `EVENT-LOG-OVERFLOW`, never as a dropped
//     record.
// It is OPT-IN: [`run_panel`] runs with the logger absent (`Option::None`) and keeps its exact
// pre-instrumentation signature and behaviour; [`run_panel_logged`] runs the identical code path
// with the logger present. The two are asserted byte-equal on the whole [`PanelOut`].
//
// **CANONICAL LOG ORDER (explicit).** `ChannelEventLog::events` is a plain `Vec` appended in the
// single-threaded `run_panel` loop, at the instant of each `GeChain::next_drop()` call, in that
// call's execution order. The total order it realises is exactly
//
//     (virtual tick, `run_panel` loop-step index, position within the decision `Vec` being consumed)
//
// and nothing else participates: there is no `HashMap`/`HashSet` on the path, no thread, no
// `rayon`, no wall-clock, and no RNG (`GeChain::next_drop` is the pure keyed hash
// `fs_cal2_gen::draw_u64(key, ordinal)`, ADR-17). No sort is performed and none is needed.
// Within a direction the record index equals `directional_event_ordinal`; that density is
// asserted by [`ChannelEventLog::dense_ordinal_check`], not assumed.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// Which arm of a panel pair produced a record. Derived from [`PanelCfg::bootstrap_on`] — the ONE
/// treatment axis — never from an outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PanelArm {
    /// `bootstrap_on == false` — the no-treatment control.
    Baseline,
    /// `bootstrap_on == true` — the bootstrap-RTT treatment.
    Treatment,
}

impl PanelArm {
    /// The arm this configuration runs.
    #[must_use]
    pub fn of(cfg: &PanelCfg) -> Self {
        if cfg.bootstrap_on {
            Self::Treatment
        } else {
            Self::Baseline
        }
    }
    /// Short label used in reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Baseline => "OFF",
            Self::Treatment => "ON",
        }
    }
}

/// Channel direction. **Load-bearing in the identity key** (addendum §5.1): without it a reverse
/// ACK key derived from a delivered unit is numerically identical to that unit's forward DATA key,
/// and a reverse-channel fate would be scored as a forward DATA fate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChanDirection {
    /// DATA + ARQ retransmissions + duplicates + Probes.
    Forward,
    /// ACKs + Echoes.
    Reverse,
}

impl ChanDirection {
    /// Report label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "FORWARD",
            Self::Reverse => "REVERSE",
        }
    }
}

/// The packet kind as seen AT the channel decision point (never reconstructed later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChanPacketKind {
    /// `TransmitKind::Data`.
    Data,
    /// `TransmitKind::Retransmit` (ARQ or tail probe — indistinguishable in `TransmitDecision`;
    /// separating them would need a new shipping accessor, which is out of scope).
    Retransmit,
    /// `TransmitKind::Duplicate`.
    Duplicate,
    /// `TransmitKind::CsProbe` — carries `unit_id = 0`, which is NEVER a DATA identity (§5.1).
    Probe,
    /// Harness-synthesised reverse ACK for one delivered forward admission.
    Ack,
    /// Sealed `CsEcho` from the real `ReceiverControlCore`.
    Echo,
}

impl ChanPacketKind {
    /// Whether this kind carries a stable `OpaqueUnit.id` identity in its own direction.
    #[must_use]
    pub fn carries_unit_identity(self) -> bool {
        matches!(
            self,
            Self::Data | Self::Retransmit | Self::Duplicate | Self::Ack
        )
    }
    /// §5.3 `class` metadata. `None` for kinds outside `{Data, Retransmit, Duplicate}`.
    #[must_use]
    pub fn transmission_class(self) -> Option<TransmissionClass> {
        match self {
            Self::Data => Some(TransmissionClass::Data),
            Self::Retransmit => Some(TransmissionClass::Retransmit),
            Self::Duplicate => Some(TransmissionClass::Duplicate),
            _ => None,
        }
    }
    /// An event class that exists ONLY because a bootstrap-probe mechanism is active.
    ///
    /// In the authoritative build (`experimental-cold-start` NOT enabled) that means the shipping
    /// treatment, so such an event can only occur in the ON arm. Under `experimental-cold-start`
    /// the LEGACY ladder fires in the OFF arm instead (SELECTION-BUILD-AXIS-ADDENDUM §1); the flag
    /// then surfaces on a baseline record, which the cap report classifies as
    /// `EVENT-CAP-CONFIG-MISMATCH` rather than hiding it.
    #[must_use]
    pub fn is_treatment_exclusive(self) -> bool {
        matches!(self, Self::Probe | Self::Echo)
    }
    /// Report label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Data => "DATA",
            Self::Retransmit => "RETRANSMIT",
            Self::Duplicate => "DUPLICATE",
            Self::Probe => "PROBE",
            Self::Ack => "ACK",
            Self::Echo => "ECHO",
        }
    }
}

/// §5.3 transmission class. **METADATA ONLY** — see [`ATTEMPT_CLASS_VERDICT`]. It is never folded
/// into the matched key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransmissionClass {
    /// First channel admission of a unit.
    Data,
    /// ARQ / tail-probe retransmission.
    Retransmit,
    /// Forward-recovery or PreConverge geometric copy.
    Duplicate,
}

/// The frozen §5.3 answer, decided from source lineage BEFORE any outcome was seen.
pub const ATTEMPT_CLASS_VERDICT: &str = "\
CLASS IS METADATA — it CAN differ across arms for the same (direction, unit_id, attempt_ordinal).
Invariant sub-result: attempt_ordinal == 0 is ALWAYS class Data. TransmitKind::Data is emitted at
exactly one site (sender.rs:4175) reached only via drain_new_data popping tx_queue (sender.rs:4166),
whose sole writer is the Event::AppData arm (sender.rs:1489-1490); every other class sources its
unit from pending_lost / dup_queue / inflight, all populated only from already-emitted units.
For attempt_ordinal >= 1 the class is NOT arm-invariant, by two independent grounded routes:
  L1 channel re-phasing — the Probe consumes one forward chain step (dr_cs3_panel.rs Probe arm), so
     every later forward packet draws at a shifted ordinal and possibly a different bad-state; that
     changes deliveries -> ACK set -> pending_lost / state / dup_queue (dup_queue.retain on ACK,
     sender.rs:2334), hence whether a unit's 2nd admission is Retransmit, Duplicate or absent;
  L2 direct product path, no channel involvement — Event::CsProbeEcho (sender.rs:1515-1518) reaches
     bs_on_echo -> self.rtt.observe (sender.rs:2503), setting srtt=Some, which flips the
     drain_new_data PreConverge branch (sender.rs:4188) so the geometric-copy DupRec seeding
     (sender.rs:4201-4205) stops being scheduled, and changes cadence_loss_floor /
     expected_ack_interval and therefore when loss is declared.
Therefore class stays OUT of the matched key; a class difference on a matched attempt is counted
separately as attempt_class_divergence and is never summed with anything (addendum §5.3, §6).";

/// Channel fate, read STRAIGHT from the decision — never inferred from an ACK, a terminal outcome
/// or an emission tick (addendum §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChannelFate {
    /// The channel admitted the packet.
    Delivered,
    /// The channel dropped the packet.
    Dropped,
}

impl ChannelFate {
    /// `true` = dropped, matching `GeChain::next_drop`'s return convention.
    #[must_use]
    pub fn from_drop(drop: bool) -> Self {
        if drop {
            Self::Dropped
        } else {
            Self::Delivered
        }
    }
    /// Report label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Dropped => "dropped",
        }
    }
}

/// Where the fate the channel APPLIED came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FateSource {
    /// The `GeChain::next_drop()` return value, unmodified.
    GeChainDecision,
    /// A §6 targeted `probe_drop_script` / `echo_drop_script` entry replaced it. The chain was
    /// still advanced unconditionally, and `ge_decision_fate` still records what it said.
    ScriptOverride,
}

/// The channel key and domain inputs in force for one direction of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainDomainInputs {
    /// The exact `GeChain` key string.
    pub key: String,
    /// §5.3 seed domain.
    pub seed_domain: String,
    /// Cell identity.
    pub cell_id: String,
    /// Repetition index (seed).
    pub repetition: u32,
    /// The loss model this direction runs.
    pub model: LossModel,
}

/// ONE real GE decision, recorded observation-only.
///
/// Every field is captured in the same statement sequence as the `GeChain::next_drop()` call that
/// produced it, before any delivery, ACK or terminal code can run — which is the structural reason
/// `actual_fate` cannot have been inferred from an ACK, a terminal outcome or an emission tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEvent {
    /// Which arm produced this record.
    pub arm: PanelArm,
    /// Position in the CANONICAL LOG ORDER (append index across both directions). Observation-only
    /// and never an identity input; it exists so "the treatment had already acted by then" is a
    /// well-defined, direction-independent statement.
    pub global_ordinal: u64,
    /// Which directional chain this decision was drawn from.
    pub direction: ChanDirection,
    /// The chain index this packet consumed (`GeChain::steps()` immediately before the draw).
    pub directional_event_ordinal: u64,
    /// **DIAGNOSTIC SCHEDULING TIMESTAMP ONLY** (addendum §5.4). Never the identity, never the
    /// attempt ordinal, never the matching key, never a source of fate. Multiple emissions share a
    /// tick by construction.
    pub simulation_tick: u64,
    /// Packet kind at the decision point.
    pub packet_kind: ChanPacketKind,
    /// Stable DATA identity component (`OpaqueUnit.id`). `None` for Probe and Echo — a Probe's
    /// wire `unit_id` is `0` and `0` is NEVER a DATA identity (§5.1).
    pub unit_id: Option<u64>,
    /// ARM-LOCAL monotone attempt ordinal for `(direction, unit_id)`.
    pub attempt_ordinal: Option<u32>,
    /// §5.3 class metadata (out of the key).
    pub transmission_class: Option<TransmissionClass>,
    /// This event class exists only because a bootstrap-probe mechanism is active.
    pub treatment_exclusive_event: bool,
    /// Chain bad-state immediately BEFORE the draw.
    pub ge_state_before: bool,
    /// Chain bad-state immediately AFTER the draw.
    pub ge_state_after: bool,
    /// The fate the channel applied to this packet.
    pub actual_fate: ChannelFate,
    /// The raw `GeChain::next_drop()` result, always recorded even when overridden.
    pub ge_decision_fate: ChannelFate,
    /// Provenance of `actual_fate`.
    pub fate_source: FateSource,
    /// Channel key / domain inputs for this direction.
    pub chain: Arc<ChainDomainInputs>,
}

impl ChannelEvent {
    /// Boundary comparison projection: channel-admitted **membership** + **packet semantic
    /// record** (addendum §2). Deliberately excludes fate, GE state, attempt ordinal and
    /// directional ordinal — those belong to the pre-treatment PREFIX check, which is what makes
    /// `PRE-TREATMENT-PREFIX-MISMATCH` non-vacuous.
    #[must_use]
    pub fn semantic_record(&self) -> (ChanPacketKind, Option<u64>) {
        (self.packet_kind, self.unit_id)
    }
    /// Pre-treatment prefix projection (addendum §2, the items the boundary does not already fix).
    #[must_use]
    pub fn prefix_record(&self) -> (u64, Option<u32>, bool, bool, ChannelFate) {
        (
            self.directional_event_ordinal,
            self.attempt_ordinal,
            self.ge_state_before,
            self.ge_state_after,
            self.actual_fate,
        )
    }
    /// Matched key (addendum §5.2), when this event carries a stable identity.
    #[must_use]
    pub fn attempt_key(&self) -> Option<AttemptKey> {
        Some(AttemptKey {
            direction: self.direction,
            unit_id: self.unit_id?,
            attempt_ordinal: self.attempt_ordinal?,
        })
    }
}

/// The authoritative matched key (addendum §5.2). `class` is NOT part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptKey {
    /// Load-bearing (§5.1).
    pub direction: ChanDirection,
    /// `OpaqueUnit.id`.
    pub unit_id: u64,
    /// ARM-LOCAL monotone ordinal, starting at 0.
    pub attempt_ordinal: u32,
}

/// ARM-LOCAL monotone attempt ordinals, per `(direction, unit_id)`.
///
/// The ONLY inputs are the key and the table's own prior state: never queue position, never the
/// tick, never the global event ordinal (addendum §5.2). Overflow is fatal, not wrapping.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttemptOrdinalTable {
    next: BTreeMap<(ChanDirection, u64), u32>,
    /// `u32` attempt-ordinal overflow occurred — FATAL.
    pub overflow: bool,
}

impl AttemptOrdinalTable {
    /// Issue the next ordinal for this identity. Increments only on an actual channel admission.
    pub fn issue(&mut self, direction: ChanDirection, unit_id: u64) -> u32 {
        let slot = self.next.entry((direction, unit_id)).or_insert(0);
        let issued = *slot;
        match slot.checked_add(1) {
            Some(v) => *slot = v,
            None => self.overflow = true,
        }
        issued
    }
    /// CONTROL-ONLY: pre-position the counter so overflow is reachable in a test.
    pub fn control_preset(&mut self, direction: ChanDirection, unit_id: u64, next: u32) {
        self.next.insert((direction, unit_id), next);
    }
}

/// One real GE decision, handed to the logger at the decision site.
#[derive(Debug, Clone, Copy)]
pub struct GeDecision {
    /// Direction of the chain that was drawn.
    pub direction: ChanDirection,
    /// `GeChain::steps()` captured immediately before the draw.
    pub directional_event_ordinal: u64,
    /// Diagnostic scheduling timestamp (§5.4).
    pub simulation_tick: u64,
    /// Packet kind at the decision point.
    pub packet_kind: ChanPacketKind,
    /// Stable identity, or `None` when the kind carries none.
    pub unit_id: Option<u64>,
    /// `GeChain::bad_state()` before the draw.
    pub ge_state_before: bool,
    /// `GeChain::bad_state()` after the draw.
    pub ge_state_after: bool,
    /// The raw `next_drop()` return.
    pub ge_decision_drop: bool,
    /// The drop the channel actually applied (differs only under a §6 script override).
    pub applied_drop: bool,
    /// A §6 script supplied `applied_drop`.
    pub script_overridden: bool,
}

// ── EVENT CAP — derived per compile configuration from the producer table ───────────────────────

/// Whether a crate feature is on, off, or not observable from this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureState {
    /// Compiled in.
    On,
    /// Compiled out.
    Off,
    /// The feature belongs to another crate and this crate's `cfg!` cannot see it.
    Unobservable,
}

impl FeatureState {
    /// Report label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::On => "ON",
            Self::Off => "OFF",
            Self::Unobservable => "UNOBSERVABLE-FROM-THIS-CRATE",
        }
    }
}

/// ARQ retransmission cap per unit — `arq.rs:73` `RetxConfig::fixture().arq_retx_cap`.
pub const ARQ_RETX_CAP: u64 = 3;
/// `sender.rs:896` `DEFAULT_DUP_FACTOR`.
pub const DUP_FACTOR: u64 = 2;
/// `sender.rs:965` `PC_SACK_WIRE`.
pub const PC_SACK_WIRE: u64 = 65;
/// `sender.rs:900` `PRE_CONVERGE_FLOOR_BW` (B/s).
pub const PRE_CONVERGE_FLOOR_BW: u64 = 31;
/// `sender.rs:966` `PC_KBASE_NUM`.
pub const PC_KBASE_NUM: u64 = 3;
/// `sender.rs:967` `PC_KBASE_DEN`.
pub const PC_KBASE_DEN: u64 = 2;
/// `sender.rs:968` `PC_COPY_MULTIPLIER`.
pub const PC_COPY_MULTIPLIER: u64 = 2;
/// `sender.rs:595` `BS_FIRES_NS.len()` — shipping bootstrap ladder steps.
pub const BS_LADDER_STEPS: u64 = 4;
/// `sender.rs:192` `CS_PROBE_FIRES_NS.len()` — LEGACY ladder steps; compiled ONLY under
/// `experimental-cold-start`, and gated to the bootstrap-OFF arm (`sender.rs:2996-2999`).
pub const CS_LEGACY_LADDER_STEPS: u64 = 4;

/// The exact product first-copy delay for a PreConverge geometric duplicate of one sealed unit,
/// reproducing `sender.rs:4196-4200` integer arithmetic verbatim.
#[must_use]
pub fn pc_first_copy_delay_ns() -> u64 {
    let rtt_floor = (SEALED + PC_SACK_WIRE).saturating_mul(1_000_000_000) / PRE_CONVERGE_FLOOR_BW;
    rtt_floor.saturating_mul(PC_KBASE_NUM) / PC_KBASE_DEN
}

/// PreConverge geometric copies of ONE unit that can become due within `horizon_ns`.
///
/// Derived from the producer's own scheduling law (`sender.rs:4143-4147`: `due = now + interval`,
/// `interval *= PC_COPY_MULTIPLIER`), i.e. the k-th copy is due at `d0 * (2^k - 1)` after the
/// seeding emission. This producer has **no finite product bound** — the source says so
/// (`sender.rs:4139-4141`, *"sayı-tavanı YOK, deadline'a dek"*) — so its bound is a harness bound
/// obtained by intersecting that law with the run's own configured observation horizon. It is NOT
/// a tick count and NOT an observed count, and it carries no headroom.
#[must_use]
pub fn pc_copies_per_unit(horizon_ns: u64) -> u64 {
    let d0 = pc_first_copy_delay_ns();
    if d0 == 0 {
        return 0;
    }
    let (mut k, mut interval, mut cum) = (0u64, d0, 0u64);
    while let Some(next) = cum.checked_add(interval) {
        if next > horizon_ns {
            break;
        }
        cum = next;
        k = k.saturating_add(1);
        let Some(i2) = interval.checked_mul(PC_COPY_MULTIPLIER) else {
            break;
        };
        interval = i2;
    }
    k
}

/// Tail probes one run can emit within `horizon_ns`.
///
/// `max_tail_probes` is a PER-EPISODE cap (`tail_recovery.rs:179`) and `on_ack_progress` resets
/// `probe_index` (`tail_recovery.rs:134-138`), so it is NOT a per-run bound. The per-run bound
/// comes from the machine's own spacing law: every emission sets `armed_at_ns = now`
/// (`on_probe_sent`), every ACK progress sets `armed_at_ns = now` (never earlier), and a probe
/// requires `now - armed >= pto >= min_pto_ns` — so two consecutive probe EMISSIONS are separated
/// by at least `min_pto_ns`, whatever the ACK trajectory does. Again a harness bound, not a
/// product bound, and again headroom-free.
#[must_use]
pub fn tail_probes_in_horizon(horizon_ns: u64, min_pto_ns: u64) -> u64 {
    if min_pto_ns == 0 {
        return 0;
    }
    horizon_ns / min_pto_ns + 1
}

/// One producer's contribution to the cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerBound {
    /// Producer id from the inventory (`F1`..`F9`, `R1`..`R3`).
    pub id: &'static str,
    /// Human name.
    pub producer: &'static str,
    /// Which chain it consumes.
    pub direction: ChanDirection,
    /// Compiled in this configuration.
    pub compiled: bool,
    /// Compiled AND reachable under this run's configuration.
    pub reachable: bool,
    /// Bound on channel events this producer can contribute to this run.
    pub bound: u64,
    /// `PRODUCT` = a closed product bound; `HARNESS` = a harness bound for a producer with no
    /// finite product bound; `ABSENT` = structurally impossible here.
    pub basis: &'static str,
    /// Exact source lineage.
    pub lineage: &'static str,
}

/// The derived event cap for one run in one compile configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCapPlan {
    /// Complete enumeration of this crate's features plus explicitly-unobservable rows.
    pub feature_fingerprint: Vec<(&'static str, FeatureState)>,
    /// Whether `adaptive-transport/experimental-cold-start` is compiled (hence the legacy ladder).
    pub legacy_ladder_compiled: bool,
    /// Every producer, compiled or not.
    pub producers: Vec<ProducerBound>,
    /// Sum of forward producer bounds.
    pub forward_cap: u64,
    /// Sum of reverse producer bounds.
    pub reverse_cap: u64,
    /// `forward_cap + reverse_cap`.
    pub total_cap: u64,
    /// Bound on forward Probe events in this configuration (0 ⇒ a Probe here is a config mismatch).
    pub probe_cap: u64,
    /// Bound on reverse Echo events in this configuration.
    pub echo_cap: u64,
}

/// The complete set of `kama-integration` features that forward
/// `adaptive-transport/experimental-cold-start` (grep-verified: `crates/kama-integration/Cargo.toml`
/// lines 20 and 30 are the only two occurrences in the whole workspace).
#[must_use]
pub fn legacy_ladder_compiled() -> bool {
    cfg!(feature = "experimental-cold-start") || cfg!(feature = "experimental-fs-harness")
}

/// Complete feature fingerprint (SELECTION-BUILD-AXIS-ADDENDUM §3 item 2).
#[must_use]
pub fn feature_fingerprint() -> Vec<(&'static str, FeatureState)> {
    let st = |on: bool| {
        if on {
            FeatureState::On
        } else {
            FeatureState::Off
        }
    };
    vec![
        (
            "kama-integration/experimental-cold-start",
            st(cfg!(feature = "experimental-cold-start")),
        ),
        (
            "kama-integration/diag-sched-telemetry",
            st(cfg!(feature = "diag-sched-telemetry")),
        ),
        (
            "kama-integration/experimental-fs-harness",
            st(cfg!(feature = "experimental-fs-harness")),
        ),
        // Forwarded state, derived from the two rows above (the only forwarders in the workspace).
        (
            "adaptive-transport/experimental-cold-start (forwarded)",
            st(legacy_ladder_compiled()),
        ),
        // `experimental-fec-budget` is an adaptive-transport-only feature with no kama-integration
        // forwarder, so this crate's `cfg!` cannot read it. It cannot change the producer table
        // either: F8 (FecParity) is 0 because the panel calls no FEC API at all, and the
        // fec-budget code paths all draw from `pending_fec`, which stays empty.
        (
            "adaptive-transport/experimental-fec-budget",
            FeatureState::Unobservable,
        ),
    ]
}

impl EventCapPlan {
    /// Derive the cap for this run from the producer table.
    ///
    /// The horizon used by the two producers that have no finite product bound is the run's own
    /// configured `warmup_timeout_ns` plus exactly one `DT_NS` — the loop's termination
    /// granularity (`now >= cfg.warmup_timeout_ns` is tested after `now += DT_NS`). That single
    /// tick is arithmetic, not headroom.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn derive(cfg: &PanelCfg) -> Self {
        let horizon = cfg.warmup_timeout_ns.saturating_add(DT_NS);
        let legacy = legacy_ladder_compiled();
        let tail_min_pto = tail_on_config().min_pto_ns;

        let f1 = WARMUP_UNITS;
        let f2 = WARMUP_UNITS.saturating_mul(ARQ_RETX_CAP);
        let f3 = WARMUP_UNITS;
        let f4 = WARMUP_UNITS.saturating_mul(DUP_FACTOR - 1);
        let f5 = WARMUP_UNITS.saturating_mul(pc_copies_per_unit(horizon));
        let f6 = if cfg.tail_on {
            tail_probes_in_horizon(horizon, tail_min_pto)
        } else {
            0
        };
        let f7 = if cfg.bootstrap_on { BS_LADDER_STEPS } else { 0 };
        let f9 = if legacy && !cfg.bootstrap_on {
            CS_LEGACY_LADDER_STEPS
        } else {
            0
        };
        let probe_cap = f7 + f9;
        // R1: exactly one reverse ACK per DELIVERED forward identity-bearing admission.
        let r1 = f1 + f2 + f3 + f4 + f5 + f6;
        let r2 = if probe_cap == 0 {
            0
        } else {
            (MAX_ECHOES as u64).min(probe_cap)
        };

        let producers = vec![
            ProducerBound { id: "F1", producer: "DATA", direction: ChanDirection::Forward, compiled: true, reachable: true, bound: f1, basis: "PRODUCT",
                lineage: "one AppData per id (dr_cs3_panel.rs feed loop) x WARMUP_UNITS; sole tx_queue writer sender.rs:1489-1490; emitted sender.rs:4175" },
            ProducerBound { id: "F2", producer: "ARQ retransmit", direction: ChanDirection::Forward, compiled: true, reachable: true, bound: f2, basis: "PRODUCT",
                lineage: "arq.rs:117 retx_count >= arq_retx_cap; arq.rs:73 arq_retx_cap=3; emitted sender.rs:3929" },
            ProducerBound { id: "F3", producer: "Duplicate (ARQ cap-exceeded handoff)", direction: ChanDirection::Forward, compiled: true, reachable: true, bound: f3, basis: "PRODUCT",
                lineage: "arq.rs:77 CapExceedPolicy::HandToForward; sender.rs:3944-3948 interval_ns=0 fire-once" },
            ProducerBound { id: "F4", producer: "Duplicate (RecoverFwd)", direction: ChanDirection::Forward, compiled: true, reachable: true, bound: f4, basis: "PRODUCT",
                lineage: "sender.rs:896 DEFAULT_DUP_FACTOR=2, factor-1 copies per DATA; sender.rs:4088-4094 / :4180-4187 interval_ns=0 fire-once" },
            ProducerBound { id: "F5", producer: "Duplicate (PreConverge geometric)", direction: ChanDirection::Forward, compiled: true, reachable: true, bound: f5, basis: "HARNESS",
                lineage: "UNBOUNDED-BY-PRODUCT (sender.rs:4139-4141). Seeded sender.rs:4201-4205, rescheduled sender.rs:4143 with interval x2; bound = WARMUP_UNITS * copies whose cumulative d0*(2^k-1) fits the run horizon" },
            ProducerBound { id: "F6", producer: "Retransmit (tail probe)", direction: ChanDirection::Forward, compiled: true, reachable: cfg.tail_on, bound: f6, basis: "HARNESS",
                lineage: "UNBOUNDED-BY-PRODUCT per run: tail_recovery.rs:179 caps per EPISODE, :134-138 on_ack_progress resets probe_index. Bound = horizon / min_pto_ns + 1 (min emission spacing, on_probe_sent re-arm)" },
            ProducerBound { id: "F7", producer: "CsProbe (shipping bootstrap)", direction: ChanDirection::Forward, compiled: true, reachable: cfg.bootstrap_on, bound: f7, basis: "PRODUCT",
                lineage: "sender.rs:595 BS_FIRES_NS[4] + monotone bs_next_step + BS_TX_HARD_BUDGET/BS_PROBE_WIRE_EXACT + bs_done latch; gated sender.rs:2436 on bootstrap_rtt.enabled" },
            ProducerBound { id: "F8", producer: "FecParity", direction: ChanDirection::Forward, compiled: true, reachable: false, bound: 0, basis: "ABSENT",
                lineage: "plan_fec_parity loops on pending_fec (sender.rs:3462); the panel calls no FEC API, so pending_fec stays empty. Asserted, not swallowed: channel_bypass_violations" },
            ProducerBound { id: "F9", producer: "CsProbe (LEGACY cold-start ladder)", direction: ChanDirection::Forward, compiled: legacy, reachable: legacy && !cfg.bootstrap_on, bound: f9, basis: if legacy { "PRODUCT" } else { "ABSENT" },
                lineage: "sender.rs:192 CS_PROBE_FIRES_NS[4], budget sender.rs:194-196, arm-once latch sender.rs:4219-4228; gated sender.rs:2996-2999 on !bootstrap_rtt.enabled, compiled ONLY under experimental-cold-start" },
            ProducerBound { id: "R1", producer: "ACK", direction: ChanDirection::Reverse, compiled: true, reachable: true, bound: r1, basis: "HARNESS",
                lineage: "one rev.next_drop() per delivered forward identity-bearing admission (dr_cs3_panel.rs delivery loop); probe deliveries generate no ACK" },
            ProducerBound { id: "R2", producer: "Echo", direction: ChanDirection::Reverse, compiled: true, reachable: probe_cap > 0, bound: r2, basis: "PRODUCT",
                lineage: "receiver_control_core.rs:29 MAX_ECHOES=4 + ECHO_BUDGET_BYTES=172; also bounded by probes_authenticated <= probe cap" },
            ProducerBound { id: "R3", producer: "Echo wire-duplicate", direction: ChanDirection::Reverse, compiled: true, reachable: false, bound: 0, basis: "ABSENT",
                lineage: "echo_wire_duplicates copies are appended AFTER the single rev.next_drop() and bypass the chain entirely — they consume no reverse event" },
        ];

        let forward_cap = f1 + f2 + f3 + f4 + f5 + f6 + f7 + f9;
        let reverse_cap = r1 + r2;
        Self {
            feature_fingerprint: feature_fingerprint(),
            legacy_ladder_compiled: legacy,
            producers,
            forward_cap,
            reverse_cap,
            total_cap: forward_cap + reverse_cap,
            probe_cap,
            echo_cap: r2,
        }
    }

    /// CONTROL-ONLY: a plan with hand-chosen caps, so the cap-boundary control can drive the
    /// production verdict path at an exactly reachable cap.
    #[must_use]
    pub fn control_plan(forward_cap: u64, reverse_cap: u64) -> Self {
        Self {
            feature_fingerprint: feature_fingerprint(),
            legacy_ladder_compiled: legacy_ladder_compiled(),
            producers: Vec::new(),
            forward_cap,
            reverse_cap,
            total_cap: forward_cap + reverse_cap,
            probe_cap: 0,
            echo_cap: 0,
        }
    }

    /// Ids of producers compiled in this configuration.
    #[must_use]
    pub fn compiled_producers(&self) -> Vec<&'static str> {
        self.producers
            .iter()
            .filter(|p| p.compiled)
            .map(|p| p.id)
            .collect()
    }
    /// Ids of producers absent in this configuration.
    #[must_use]
    pub fn absent_producers(&self) -> Vec<&'static str> {
        self.producers
            .iter()
            .filter(|p| !p.compiled || p.basis == "ABSENT")
            .map(|p| p.id)
            .collect()
    }
}

/// The cap verdict for one log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapVerdict {
    /// Observed counts within every derived bound.
    Pass,
    /// A producer produced events the configuration says it cannot.
    ConfigMismatch(String),
    /// Observed count exceeded a derived cap. Records were still kept — nothing is truncated.
    Overflow(String),
}

impl CapVerdict {
    /// Frozen verdict label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::ConfigMismatch(_) => "EVENT-CAP-CONFIG-MISMATCH — M2 RED",
            Self::Overflow(_) => "EVENT-LOG-OVERFLOW — M2 RED",
        }
    }
    /// `true` when the verdict is `PASS`.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}

/// The full per-configuration cap report (item 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCapReport {
    /// Complete feature fingerprint.
    pub feature_fingerprint: Vec<(&'static str, FeatureState)>,
    /// Legacy ladder compiled in this configuration.
    pub legacy_ladder_compiled: bool,
    /// Compiled producer ids.
    pub compiled_producers: Vec<&'static str>,
    /// Absent producer ids.
    pub absent_producers: Vec<&'static str>,
    /// Every producer with its bound.
    pub producers: Vec<ProducerBound>,
    /// Derived forward cap.
    pub forward_cap: u64,
    /// Derived reverse cap.
    pub reverse_cap: u64,
    /// Derived total cap.
    pub total_cap: u64,
    /// Observed forward channel events.
    pub observed_forward: u64,
    /// Observed reverse channel events.
    pub observed_reverse: u64,
    /// Observed total channel events.
    pub observed_total: u64,
    /// Observed forward Probe events.
    pub observed_probe: u64,
    /// Observed reverse Echo events.
    pub observed_echo: u64,
    /// Verdict.
    pub verdict: CapVerdict,
}

/// The full log for ONE arm of ONE run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEventLog {
    /// Arm that produced these records.
    pub arm: PanelArm,
    /// Forward chain key + domain inputs.
    pub forward_chain: Arc<ChainDomainInputs>,
    /// Reverse chain key + domain inputs.
    pub reverse_chain: Arc<ChainDomainInputs>,
    /// Derived cap for this run's compile configuration.
    pub caps: EventCapPlan,
    /// Every recorded GE decision, in canonical (append) order. NEVER truncated.
    pub events: Vec<ChannelEvent>,
    /// Arm-local attempt-ordinal issuer.
    pub attempts: AttemptOrdinalTable,
}

impl ChannelEventLog {
    /// Fresh log for one run.
    #[must_use]
    pub fn new(cfg: &PanelCfg) -> Self {
        let mk = |key: String, model: LossModel| {
            Arc::new(ChainDomainInputs {
                key,
                seed_domain: cfg.seed_domain.clone(),
                cell_id: cfg.cell_id.clone(),
                repetition: cfg.repetition,
                model,
            })
        };
        Self {
            arm: PanelArm::of(cfg),
            forward_chain: mk(
                forward_key(&cfg.seed_domain, &cfg.cell_id, cfg.repetition),
                cfg.forward_model,
            ),
            reverse_chain: mk(
                reverse_key(&cfg.seed_domain, &cfg.cell_id, cfg.repetition),
                cfg.reverse_model,
            ),
            caps: EventCapPlan::derive(cfg),
            events: Vec::new(),
            attempts: AttemptOrdinalTable::default(),
        }
    }

    /// CONTROL-ONLY: an empty log with hand-chosen caps and synthetic domain inputs, so the
    /// controls drive the SAME production `record` / comparator path as a real run.
    #[must_use]
    pub fn control_log(arm: PanelArm, caps: EventCapPlan) -> Self {
        let mk = |key: &str| {
            Arc::new(ChainDomainInputs {
                key: key.to_string(),
                seed_domain: "DRCS3-CONTROL".into(),
                cell_id: "CONTROL".into(),
                repetition: 0,
                model: LossModel::None,
            })
        };
        Self {
            arm,
            forward_chain: mk("DRCS3-CONTROL|CONTROL|rep0|V2Forward"),
            reverse_chain: mk("DRCS3-CONTROL|CONTROL|rep0|V2Reverse"),
            caps,
            events: Vec::new(),
            attempts: AttemptOrdinalTable::default(),
        }
    }

    /// Record ONE real GE decision. The only mutating entry point.
    ///
    /// Appends exactly one record and issues at most one attempt ordinal. It reads no channel, no
    /// sender and no clock; it calls nothing.
    pub fn record(&mut self, d: GeDecision) {
        let unit_id = if d.packet_kind.carries_unit_identity() {
            d.unit_id
        } else {
            None // §5.1: a Probe's wire unit_id (0) is NEVER a DATA identity.
        };
        let attempt_ordinal = unit_id.map(|uid| self.attempts.issue(d.direction, uid));
        let chain = match d.direction {
            ChanDirection::Forward => Arc::clone(&self.forward_chain),
            ChanDirection::Reverse => Arc::clone(&self.reverse_chain),
        };
        self.events.push(ChannelEvent {
            arm: self.arm,
            global_ordinal: self.events.len() as u64,
            direction: d.direction,
            directional_event_ordinal: d.directional_event_ordinal,
            simulation_tick: d.simulation_tick,
            packet_kind: d.packet_kind,
            unit_id,
            attempt_ordinal,
            transmission_class: d.packet_kind.transmission_class(),
            treatment_exclusive_event: d.packet_kind.is_treatment_exclusive(),
            ge_state_before: d.ge_state_before,
            ge_state_after: d.ge_state_after,
            actual_fate: ChannelFate::from_drop(d.applied_drop),
            ge_decision_fate: ChannelFate::from_drop(d.ge_decision_drop),
            fate_source: if d.script_overridden {
                FateSource::ScriptOverride
            } else {
                FateSource::GeChainDecision
            },
            chain,
        });
    }

    /// Events of one direction, in canonical order.
    #[must_use]
    pub fn direction_events(&self, direction: ChanDirection) -> Vec<&ChannelEvent> {
        self.events
            .iter()
            .filter(|e| e.direction == direction)
            .collect()
    }

    /// Recorded events in one direction.
    #[must_use]
    pub fn count(&self, direction: ChanDirection) -> u64 {
        self.events
            .iter()
            .filter(|e| e.direction == direction)
            .count() as u64
    }

    /// Recorded events of one kind.
    #[must_use]
    pub fn count_kind(&self, kind: ChanPacketKind) -> u64 {
        self.events.iter().filter(|e| e.packet_kind == kind).count() as u64
    }

    /// Per-direction ordinal density: record index == `directional_event_ordinal`. One chain, one
    /// step per record, no gap, no reuse.
    ///
    /// # Errors
    /// Returns the first violating `(direction, index, recorded ordinal)`.
    pub fn dense_ordinal_check(&self) -> Result<(), String> {
        for dir in [ChanDirection::Forward, ChanDirection::Reverse] {
            for (i, e) in self.direction_events(dir).iter().enumerate() {
                if e.directional_event_ordinal != i as u64 {
                    return Err(format!(
                        "{} record {i} carries directional_event_ordinal {} — the log is not dense",
                        dir.label(),
                        e.directional_event_ordinal
                    ));
                }
            }
        }
        Ok(())
    }

    /// Attempt-ordinal integrity (addendum §5.2): per `(direction, unit_id)` the ordinals seen in
    /// canonical order MUST be exactly `0, 1, 2, …` — start at 0, no duplicate, no gap. `u32`
    /// overflow is fatal.
    ///
    /// # Errors
    /// Returns the exact violation.
    pub fn attempt_ordinal_check(&self) -> Result<(), String> {
        if self.attempts.overflow {
            return Err("ATTEMPT-ORDINAL-OVERFLOW — FATAL".to_string());
        }
        let mut expect: BTreeMap<(ChanDirection, u64), u32> = BTreeMap::new();
        let mut seen: BTreeSet<AttemptKey> = BTreeSet::new();
        for e in &self.events {
            let (Some(uid), Some(ord)) = (e.unit_id, e.attempt_ordinal) else {
                if e.unit_id.is_some() != e.attempt_ordinal.is_some() {
                    return Err(format!(
                        "{} {} carries a half identity (unit_id {:?}, attempt {:?})",
                        e.direction.label(),
                        e.packet_kind.label(),
                        e.unit_id,
                        e.attempt_ordinal
                    ));
                }
                continue;
            };
            let key = AttemptKey {
                direction: e.direction,
                unit_id: uid,
                attempt_ordinal: ord,
            };
            if !seen.insert(key) {
                return Err(format!("DUPLICATE ATTEMPT KEY {key:?} — FATAL"));
            }
            let next = expect.entry((e.direction, uid)).or_insert(0);
            if ord != *next {
                return Err(format!(
                    "ATTEMPT ORDINAL GAP: {} unit {uid} expected {} got {ord} — FATAL",
                    e.direction.label(),
                    *next
                ));
            }
            *next = next.saturating_add(1);
        }
        Ok(())
    }

    /// The per-configuration cap report (item 6).
    #[must_use]
    pub fn cap_report(&self) -> EventCapReport {
        let observed_forward = self.count(ChanDirection::Forward);
        let observed_reverse = self.count(ChanDirection::Reverse);
        let observed_probe = self.count_kind(ChanPacketKind::Probe);
        let observed_echo = self.count_kind(ChanPacketKind::Echo);
        let observed_total = observed_forward + observed_reverse;

        // Configuration mismatch takes priority: a producer the configuration says is absent, or a
        // producer over its own bound, is a build-axis error, not a sizing error.
        let verdict = if observed_probe > self.caps.probe_cap {
            CapVerdict::ConfigMismatch(format!(
                "{observed_probe} Probe channel events observed but the derived probe bound for \
                 this configuration is {} (legacy_ladder_compiled={})",
                self.caps.probe_cap, self.caps.legacy_ladder_compiled
            ))
        } else if observed_echo > self.caps.echo_cap {
            CapVerdict::ConfigMismatch(format!(
                "{observed_echo} Echo channel events observed but the derived echo bound is {}",
                self.caps.echo_cap
            ))
        } else if observed_forward > self.caps.forward_cap {
            CapVerdict::Overflow(format!(
                "forward {observed_forward} > cap {}",
                self.caps.forward_cap
            ))
        } else if observed_reverse > self.caps.reverse_cap {
            CapVerdict::Overflow(format!(
                "reverse {observed_reverse} > cap {}",
                self.caps.reverse_cap
            ))
        } else if observed_total > self.caps.total_cap {
            CapVerdict::Overflow(format!(
                "total {observed_total} > cap {}",
                self.caps.total_cap
            ))
        } else {
            CapVerdict::Pass
        };

        EventCapReport {
            feature_fingerprint: self.caps.feature_fingerprint.clone(),
            legacy_ladder_compiled: self.caps.legacy_ladder_compiled,
            compiled_producers: self.caps.compiled_producers(),
            absent_producers: self.caps.absent_producers(),
            producers: self.caps.producers.clone(),
            forward_cap: self.caps.forward_cap,
            reverse_cap: self.caps.reverse_cap,
            total_cap: self.caps.total_cap,
            observed_forward,
            observed_reverse,
            observed_total,
            observed_probe,
            observed_echo,
            verdict,
        }
    }
}

// ── COMPARATOR ─────────────────────────────────────────────────────────────────────────────────

/// The addendum §3 authoritative reporting sentence for a matched fate divergence.
pub const AUTHORITATIVE_MATCHED_FATE_CLAIM: &str =
    "The n-th channel-admission attempt of the same \
     semantic DATA unit received a different channel fate under the two treatment trajectories.";

/// Wording the report must never contain (addendum §3 forbidden language, §4 forbidden score).
pub const FORBIDDEN_CLAIM_FRAGMENTS: [&str; 6] = [
    "same physical packet",
    "same channel realisation",
    "same channel realization",
    "net harm",
    "net-harm",
    "direct effect",
];

/// Which side of the per-direction treatment boundary a matched attempt sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundarySide {
    /// Both arms' ordinals are strictly before the boundary.
    PreBoundary,
    /// At or after the boundary in at least one arm.
    PostBoundary,
}

/// Addendum §4 prior-attempt tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorPrefix {
    /// Attempts `0..n-1` exist in BOTH arms with the SAME actual fates.
    Aligned,
    /// Otherwise — `TRAJECTORY-CONDITIONED MATCHED-ATTEMPT FATE DIVERGENCE`.
    Diverged,
}

/// Addendum §4 ordinal tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdinalAlignment {
    /// Same directional event ordinal in both arms.
    Aligned,
    /// Different directional event ordinal — ACCEPTED post-boundary (§1 packet-domain re-phasing).
    Rephased,
}

/// One matched attempt, fully tagged (addendum §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedAttempt {
    /// The matched key.
    pub key: AttemptKey,
    /// Pre- or post-boundary.
    pub boundary_side: BoundarySide,
    /// Prior-attempt prefix tag.
    pub prior_prefix: PriorPrefix,
    /// Ordinal alignment tag.
    pub ordinal_alignment: OrdinalAlignment,
    /// Baseline-arm fate.
    pub off_fate: ChannelFate,
    /// Treatment-arm fate.
    pub on_fate: ChannelFate,
    /// Baseline-arm class metadata.
    pub off_class: Option<TransmissionClass>,
    /// Treatment-arm class metadata.
    pub on_class: Option<TransmissionClass>,
    /// Baseline directional event ordinal.
    pub off_ordinal: u64,
    /// Treatment directional event ordinal.
    pub on_ordinal: u64,
}

/// Addendum §4 directional fate counts. **NEVER collapsed into a summed net-harm score.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FateCrossTab {
    /// Baseline delivered, treatment dropped.
    pub off_delivered_on_dropped: u64,
    /// Baseline dropped, treatment delivered.
    pub off_dropped_on_delivered: u64,
    /// Both delivered.
    pub both_delivered: u64,
    /// Both dropped.
    pub both_dropped: u64,
}

/// The §6 metric set for ONE direction. The six counts are kept apart and are never summed into an
/// aggregate total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionReport {
    /// Which direction.
    pub direction: ChanDirection,
    /// Addendum §2 boundary for THIS direction, computed independently of the other direction.
    /// `None` = the two arms never differ in this direction.
    pub first_treatment_induced_channel_event: Option<u64>,
    /// §6 metric.
    pub matched_data_attempts: u64,
    /// §6 metric.
    pub matched_data_attempt_fate_divergences: u64,
    /// §6 metric.
    pub off_only_data_attempts: u64,
    /// §6 metric.
    pub on_only_data_attempts: u64,
    /// §6 metric — baseline arm (Probe/Echo carry no DATA identity).
    pub non_data_channel_events_off: u64,
    /// §6 metric — treatment arm.
    pub non_data_channel_events_on: u64,
    /// §6 metric.
    pub attempt_class_divergences: u64,
    /// §4 four directional fate counts, never collapsed.
    pub fate_cross_tab: FateCrossTab,
    /// Every matched attempt, tagged.
    pub matched: Vec<MatchedAttempt>,
    /// Tag count.
    pub pre_boundary_matched: u64,
    /// Tag count.
    pub post_boundary_matched: u64,
    /// Tag count.
    pub prior_prefix_aligned: u64,
    /// Tag count.
    pub prior_prefix_diverged: u64,
    /// Tag count.
    pub event_ordinal_aligned: u64,
    /// Tag count.
    pub event_ordinal_rephased: u64,
}

/// Pairing verdict. A void pairing reports NO fate metric (addendum §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingVerdict {
    /// Pre-treatment prefixes equal in both directions.
    Valid,
    /// `INVALID-BASELINE-CONFIG — ABORT SELECTION` (SELECTION-BUILD-AXIS-ADDENDUM §4). The
    /// baseline arm put a treatment-exclusive packet on a directional chain, so it is not a
    /// no-treatment control and the pair is not a paired comparison.
    InvalidBaselineConfig {
        /// Exact reason, including the §4 origin attribution.
        reason: String,
    },
    /// `PRE-TREATMENT-PREFIX-MISMATCH — INVALID PAIRING`.
    PreTreatmentPrefixMismatch {
        /// Which direction failed.
        direction: ChanDirection,
        /// Directional ordinal of the failure.
        ordinal: u64,
        /// Exact reason.
        reason: String,
    },
    /// The instrument itself is invalid (non-dense ordinals / attempt-ordinal integrity).
    InstrumentInvalid {
        /// Exact reason.
        reason: String,
    },
}

impl PairingVerdict {
    /// Frozen verdict label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Valid => "VALID PAIRING",
            Self::InvalidBaselineConfig { .. } => "INVALID-BASELINE-CONFIG — ABORT SELECTION",
            Self::PreTreatmentPrefixMismatch { .. } => {
                "PRE-TREATMENT-PREFIX-MISMATCH — INVALID PAIRING"
            }
            Self::InstrumentInvalid { .. } => "INSTRUMENT INVALID",
        }
    }
    /// `true` when fate metrics may be reported.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// The full paired comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelFateComparison {
    /// Pairing verdict.
    pub verdict: PairingVerdict,
    /// Forward metrics — `None` whenever the pairing is void.
    pub forward: Option<DirectionReport>,
    /// Reverse metrics — `None` whenever the pairing is void.
    pub reverse: Option<DirectionReport>,
}

impl ChannelFateComparison {
    /// The forward report, or a panic-free `None` accessor for controls.
    #[must_use]
    pub fn direction(&self, direction: ChanDirection) -> Option<&DirectionReport> {
        match direction {
            ChanDirection::Forward => self.forward.as_ref(),
            ChanDirection::Reverse => self.reverse.as_ref(),
        }
    }

    /// Render the report. The wording is fixed by addendum §3: it states the authoritative claim
    /// and never any physical-event causal claim, and it never prints a summed score.
    #[must_use]
    pub fn report_text(&self) -> String {
        let mut s = String::new();
        s.push_str("DR-CS-3 CHANNEL-FATE COMPARISON\n");
        s.push_str(&format!("verdict: {}\n", self.verdict.label()));
        if let PairingVerdict::PreTreatmentPrefixMismatch {
            direction,
            ordinal,
            reason,
        } = &self.verdict
        {
            s.push_str(&format!(
                "  {} ordinal {ordinal}: {reason}\n  pairing void — no fate metric is reported.\n",
                direction.label()
            ));
        }
        if let PairingVerdict::InstrumentInvalid { reason }
        | PairingVerdict::InvalidBaselineConfig { reason } = &self.verdict
        {
            s.push_str(&format!(
                "  {reason}\n  pairing void — no fate metric is reported.\n"
            ));
        }
        for r in [self.forward.as_ref(), self.reverse.as_ref()]
            .into_iter()
            .flatten()
        {
            s.push_str(&format!(
                "[{}] first_treatment_induced_channel_event={}\n",
                r.direction.label(),
                r.first_treatment_induced_channel_event
                    .map_or("NONE — arms never differ".to_string(), |v| v.to_string())
            ));
            s.push_str(&format!(
                "  matched_data_attempts={} | matched_data_attempt_fate_divergences={} | \
                 off_only_data_attempts={} | on_only_data_attempts={} | \
                 non_data_channel_events(off={},on={}) | attempt_class_divergences={}\n",
                r.matched_data_attempts,
                r.matched_data_attempt_fate_divergences,
                r.off_only_data_attempts,
                r.on_only_data_attempts,
                r.non_data_channel_events_off,
                r.non_data_channel_events_on,
                r.attempt_class_divergences
            ));
            s.push_str(&format!(
                "  fate counts (reported separately, never collapsed): \
                 off_delivered_on_dropped={} off_dropped_on_delivered={} both_delivered={} \
                 both_dropped={}\n",
                r.fate_cross_tab.off_delivered_on_dropped,
                r.fate_cross_tab.off_dropped_on_delivered,
                r.fate_cross_tab.both_delivered,
                r.fate_cross_tab.both_dropped
            ));
            s.push_str(&format!(
                "  tags: pre_boundary={} post_boundary={} | prior_attempt_prefix_aligned={} \
                 prior_attempt_prefix_diverged={} | event_ordinal_aligned={} \
                 event_ordinal_rephased={}\n",
                r.pre_boundary_matched,
                r.post_boundary_matched,
                r.prior_prefix_aligned,
                r.prior_prefix_diverged,
                r.event_ordinal_aligned,
                r.event_ordinal_rephased
            ));
        }
        s.push_str("interpretation: ");
        s.push_str(AUTHORITATIVE_MATCHED_FATE_CLAIM);
        s.push_str(
            "\nA post-boundary difference in directional event ordinal is EXPECTED and ACCEPTED: \
             the treatment consumes its own directional chain event and re-phases the \
             packet-to-draw pairing (addendum §1). This is a diagnostic, not a safety gate, not a \
             promotion threshold, and not a cost measure.\n",
        );
        s
    }
}

/// Per-direction boundary outcome.
struct BoundaryOutcome {
    boundary: Option<u64>,
    failure: Option<(u64, String)>,
}

/// Compute the per-direction boundary and validate the pre-treatment prefix.
///
/// The boundary is the addendum §2 definition applied literally: the first canonical directional
/// event at which the two arms' channel-admitted event MEMBERSHIP or PACKET SEMANTIC RECORD first
/// differ. The prefix check then asserts the remaining §2 items — attempt ordinal, GE state before
/// and after, actual fate, directional event ordinal — strictly before that boundary; those are not
/// part of the boundary projection, which is exactly why the check can fail.
///
/// One extra guard: a boundary is legitimate only if the treatment has already acted. If the arms
/// first differ at a point where the treatment arm has recorded NO treatment-exclusive channel
/// event yet (globally, in either direction) and the differing event is not itself one, the arms
/// diverged for a reason the treatment cannot explain — an instrument or pairing failure.
fn direction_boundary(
    off: &ChannelEventLog,
    on: &ChannelEventLog,
    direction: ChanDirection,
) -> BoundaryOutcome {
    let o = off.direction_events(direction);
    let t = on.direction_events(direction);
    // Global index of the treatment arm's first treatment-exclusive channel event (either
    // direction) — the instant the treatment first acted on any chain.
    let on_first_treatment_global = on
        .events
        .iter()
        .find(|e| e.treatment_exclusive_event)
        .map(|e| e.global_ordinal);

    let n = o.len().max(t.len());
    for i in 0..n {
        let (a, b) = (o.get(i).copied(), t.get(i).copied());
        let same = match (a, b) {
            (Some(x), Some(y)) => x.semantic_record() == y.semantic_record(),
            _ => false,
        };
        if same {
            continue;
        }
        let ordinal = i as u64;
        // Is this divergence attributable to the treatment having acted?
        let differing_is_treatment_exclusive = b.is_some_and(|e| e.treatment_exclusive_event)
            || a.is_some_and(|e| e.treatment_exclusive_event);
        let treatment_already_acted = match b {
            Some(e) => on_first_treatment_global.is_some_and(|f| f < e.global_ordinal),
            None => on_first_treatment_global.is_some(),
        };
        if !differing_is_treatment_exclusive && !treatment_already_acted {
            return BoundaryOutcome {
                boundary: None,
                failure: Some((
                    ordinal,
                    format!(
                        "the arms' {} channel-admitted membership/semantic record first differ at \
                         ordinal {ordinal} (off={:?}, on={:?}), but the treatment arm has recorded \
                         no treatment-exclusive channel event before that point — the divergence \
                         is not attributable to the treatment",
                        direction.label(),
                        a.map(ChannelEvent::semantic_record),
                        b.map(ChannelEvent::semantic_record),
                    ),
                )),
            };
        }
        // Prefix check, strictly before the boundary.
        for j in 0..i {
            let (x, y) = (o[j], t[j]);
            if x.prefix_record() != y.prefix_record() {
                return BoundaryOutcome {
                    boundary: None,
                    failure: Some((
                        j as u64,
                        format!(
                            "{} pre-treatment prefix differs at ordinal {j}: \
                             (directional_ordinal, attempt_ordinal, ge_before, ge_after, fate) \
                             off={:?} on={:?}",
                            direction.label(),
                            x.prefix_record(),
                            y.prefix_record()
                        ),
                    )),
                };
            }
        }
        return BoundaryOutcome {
            boundary: Some(ordinal),
            failure: None,
        };
    }
    // No divergence anywhere in this direction — the whole trace is the prefix.
    for j in 0..o.len() {
        if o[j].prefix_record() != t[j].prefix_record() {
            return BoundaryOutcome {
                boundary: None,
                failure: Some((
                    j as u64,
                    format!(
                        "{} pre-treatment prefix differs at ordinal {j}: \
                         (directional_ordinal, attempt_ordinal, ge_before, ge_after, fate) \
                         off={:?} on={:?}",
                        direction.label(),
                        o[j].prefix_record(),
                        t[j].prefix_record()
                    ),
                )),
            };
        }
    }
    BoundaryOutcome {
        boundary: None,
        failure: None,
    }
}

/// Full key-set membership scan for one direction. Zip-only comparison is forbidden (§6).
fn direction_report(
    off: &ChannelEventLog,
    on: &ChannelEventLog,
    direction: ChanDirection,
    boundary: Option<u64>,
) -> DirectionReport {
    let index = |log: &ChannelEventLog| -> BTreeMap<AttemptKey, ChannelEvent> {
        log.direction_events(direction)
            .into_iter()
            .filter_map(|e| e.attempt_key().map(|k| (k, e.clone())))
            .collect()
    };
    let oi = index(off);
    let ti = index(on);

    // FULL key-set membership scan in BOTH directions.
    let mut keys: BTreeSet<AttemptKey> = BTreeSet::new();
    keys.extend(oi.keys().copied());
    keys.extend(ti.keys().copied());

    let mut r = DirectionReport {
        direction,
        first_treatment_induced_channel_event: boundary,
        matched_data_attempts: 0,
        matched_data_attempt_fate_divergences: 0,
        off_only_data_attempts: 0,
        on_only_data_attempts: 0,
        non_data_channel_events_off: off
            .direction_events(direction)
            .iter()
            .filter(|e| e.attempt_key().is_none())
            .count() as u64,
        non_data_channel_events_on: on
            .direction_events(direction)
            .iter()
            .filter(|e| e.attempt_key().is_none())
            .count() as u64,
        attempt_class_divergences: 0,
        fate_cross_tab: FateCrossTab::default(),
        matched: Vec::new(),
        pre_boundary_matched: 0,
        post_boundary_matched: 0,
        prior_prefix_aligned: 0,
        prior_prefix_diverged: 0,
        event_ordinal_aligned: 0,
        event_ordinal_rephased: 0,
    };

    for key in keys {
        match (oi.get(&key), ti.get(&key)) {
            (Some(a), Some(b)) => {
                r.matched_data_attempts += 1;
                match (a.actual_fate, b.actual_fate) {
                    (ChannelFate::Delivered, ChannelFate::Dropped) => {
                        r.fate_cross_tab.off_delivered_on_dropped += 1;
                        r.matched_data_attempt_fate_divergences += 1;
                    }
                    (ChannelFate::Dropped, ChannelFate::Delivered) => {
                        r.fate_cross_tab.off_dropped_on_delivered += 1;
                        r.matched_data_attempt_fate_divergences += 1;
                    }
                    (ChannelFate::Delivered, ChannelFate::Delivered) => {
                        r.fate_cross_tab.both_delivered += 1;
                    }
                    (ChannelFate::Dropped, ChannelFate::Dropped) => {
                        r.fate_cross_tab.both_dropped += 1;
                    }
                }
                if a.transmission_class != b.transmission_class || a.packet_kind != b.packet_kind {
                    r.attempt_class_divergences += 1;
                }
                let boundary_side = match boundary {
                    Some(bd)
                        if a.directional_event_ordinal >= bd
                            || b.directional_event_ordinal >= bd =>
                    {
                        BoundarySide::PostBoundary
                    }
                    _ => BoundarySide::PreBoundary,
                };
                // prior_attempt_prefix_aligned: attempts 0..n-1 exist in BOTH arms with the SAME
                // actual fates (addendum §4).
                let mut prior = PriorPrefix::Aligned;
                for p in 0..key.attempt_ordinal {
                    let pk = AttemptKey {
                        attempt_ordinal: p,
                        ..key
                    };
                    match (oi.get(&pk), ti.get(&pk)) {
                        (Some(x), Some(y)) if x.actual_fate == y.actual_fate => {}
                        _ => {
                            prior = PriorPrefix::Diverged;
                            break;
                        }
                    }
                }
                let ordinal_alignment =
                    if a.directional_event_ordinal == b.directional_event_ordinal {
                        OrdinalAlignment::Aligned
                    } else {
                        OrdinalAlignment::Rephased
                    };
                match boundary_side {
                    BoundarySide::PreBoundary => r.pre_boundary_matched += 1,
                    BoundarySide::PostBoundary => r.post_boundary_matched += 1,
                }
                match prior {
                    PriorPrefix::Aligned => r.prior_prefix_aligned += 1,
                    PriorPrefix::Diverged => r.prior_prefix_diverged += 1,
                }
                match ordinal_alignment {
                    OrdinalAlignment::Aligned => r.event_ordinal_aligned += 1,
                    OrdinalAlignment::Rephased => r.event_ordinal_rephased += 1,
                }
                r.matched.push(MatchedAttempt {
                    key,
                    boundary_side,
                    prior_prefix: prior,
                    ordinal_alignment,
                    off_fate: a.actual_fate,
                    on_fate: b.actual_fate,
                    off_class: a.transmission_class,
                    on_class: b.transmission_class,
                    off_ordinal: a.directional_event_ordinal,
                    on_ordinal: b.directional_event_ordinal,
                });
            }
            (Some(_), None) => r.off_only_data_attempts += 1,
            (None, Some(_)) => r.on_only_data_attempts += 1,
            (None, None) => unreachable!("key came from one of the two indices"),
        }
    }
    r
}

/// Compare a baseline log with a treatment log (addendum §2/§4/§6).
///
/// Boundaries are computed INDEPENDENTLY per direction. On any pre-treatment prefix failure the
/// pairing is void and **no fate metric is reported** from it.
#[must_use]
pub fn compare_channel_fates(off: &ChannelEventLog, on: &ChannelEventLog) -> ChannelFateComparison {
    for (tag, log) in [("baseline", off), ("treatment", on)] {
        if let Err(e) = log.dense_ordinal_check() {
            return ChannelFateComparison {
                verdict: PairingVerdict::InstrumentInvalid {
                    reason: format!("{tag}: {e}"),
                },
                forward: None,
                reverse: None,
            };
        }
        if let Err(e) = log.attempt_ordinal_check() {
            return ChannelFateComparison {
                verdict: PairingVerdict::InstrumentInvalid {
                    reason: format!("{tag}: {e}"),
                },
                forward: None,
                reverse: None,
            };
        }
    }
    // SELECTION-BUILD-AXIS-ADDENDUM §4 halt rule, checked BEFORE any metric can be formed: a
    // treatment-exclusive packet on a directional chain in the BASELINE arm means the control arm
    // was not a no-treatment control. Under `experimental-cold-start` the LEGACY ladder is gated to
    // fire exactly when the bootstrap treatment is DISABLED (`sender.rs:2996-2999`), so this is the
    // expected verdict in that build — and §5 makes results from it diagnostic-only.
    let baseline_treatment_events = off
        .events
        .iter()
        .filter(|e| e.treatment_exclusive_event)
        .count();
    if baseline_treatment_events > 0 {
        let kinds: BTreeSet<&'static str> = off
            .events
            .iter()
            .filter(|e| e.treatment_exclusive_event)
            .map(|e| e.packet_kind.label())
            .collect();
        return ChannelFateComparison {
            verdict: PairingVerdict::InvalidBaselineConfig {
                reason: format!(
                    "the baseline arm put {baseline_treatment_events} treatment-exclusive \
                     channel event(s) {kinds:?} on a directional chain \
                     (legacy_ladder_compiled={}); the OFF arm is not a no-treatment control and \
                     the pairing is inadmissible",
                    off.caps.legacy_ladder_compiled
                ),
            },
            forward: None,
            reverse: None,
        };
    }
    let fwd = direction_boundary(off, on, ChanDirection::Forward);
    let rev = direction_boundary(off, on, ChanDirection::Reverse);
    for (dir, outcome) in [
        (ChanDirection::Forward, &fwd),
        (ChanDirection::Reverse, &rev),
    ] {
        if let Some((ordinal, reason)) = &outcome.failure {
            return ChannelFateComparison {
                verdict: PairingVerdict::PreTreatmentPrefixMismatch {
                    direction: dir,
                    ordinal: *ordinal,
                    reason: reason.clone(),
                },
                forward: None,
                reverse: None,
            };
        }
    }
    ChannelFateComparison {
        verdict: PairingVerdict::Valid,
        forward: Some(direction_report(
            off,
            on,
            ChanDirection::Forward,
            fwd.boundary,
        )),
        reverse: Some(direction_report(
            off,
            on,
            ChanDirection::Reverse,
            rev.boundary,
        )),
    }
}

/// **NEGATIVE FIXTURE — NOT the production comparator, never used for a result.**
///
/// A positional zip of the two arms' identity-bearing events in one direction. It has no concept of
/// an unmatched attempt, so a one-sided event silently shifts every later pairing and disappears
/// from the accounting. Exists only so a control can demonstrate that the production full key-set
/// membership scan catches what a zip misses (§6 "zip-only comparison is forbidden").
#[must_use]
pub fn zip_only_fate_divergences_negative_fixture(
    off: &ChannelEventLog,
    on: &ChannelEventLog,
    direction: ChanDirection,
) -> (u64, u64) {
    let o: Vec<&ChannelEvent> = off
        .direction_events(direction)
        .into_iter()
        .filter(|e| e.attempt_key().is_some())
        .collect();
    let t: Vec<&ChannelEvent> = on
        .direction_events(direction)
        .into_iter()
        .filter(|e| e.attempt_key().is_some())
        .collect();
    let mut compared = 0u64;
    let mut diverged = 0u64;
    for (a, b) in o.iter().zip(t.iter()) {
        compared += 1;
        if a.actual_fate != b.actual_fate {
            diverged += 1;
        }
    }
    (compared, diverged)
}

/// **NEGATIVE FIXTURE — direction-erased key.** Counts identity collisions that appear when
/// `direction` is dropped from the matched key (addendum §5.1). A non-zero result is RED: it means
/// a reverse-channel fate would have been scored against a forward DATA key.
#[must_use]
pub fn direction_erased_key_collisions(log: &ChannelEventLog) -> u64 {
    let mut seen: BTreeSet<(u64, u32)> = BTreeSet::new();
    let mut collisions = 0u64;
    for e in &log.events {
        if let (Some(uid), Some(ord)) = (e.unit_id, e.attempt_ordinal) {
            if !seen.insert((uid, ord)) {
                collisions += 1;
            }
        }
    }
    collisions
}

/// Collisions under the AUTHORITATIVE key, which includes direction. MUST be 0.
#[must_use]
pub fn authoritative_key_collisions(log: &ChannelEventLog) -> u64 {
    let mut seen: BTreeSet<AttemptKey> = BTreeSet::new();
    let mut collisions = 0u64;
    for e in &log.events {
        if let Some(k) = e.attempt_key() {
            if !seen.insert(k) {
                collisions += 1;
            }
        }
    }
    collisions
}

/// Run one panel flow.
///
/// **Logger OFF.** Byte-identical to the pre-instrumentation entry point: the channel-event log is
/// `None`, so no record is built and no attempt ordinal is issued. Every existing consumer keeps
/// this signature and this behaviour.
#[must_use]
pub fn run_panel(cfg: &PanelCfg) -> PanelOut {
    run_panel_inner(cfg, None).0
}

/// Run one panel flow **with the directional channel event log ON**.
///
/// Same code path, same statement order, same channel, same sender. The only difference is that
/// each real `GeChain::next_drop()` also appends one observation-only record. Asserted equal to
/// [`run_panel`] on the whole [`PanelOut`] by the neutrality controls.
#[must_use]
pub fn run_panel_logged(cfg: &PanelCfg) -> (PanelOut, ChannelEventLog) {
    let (out, log) = run_panel_inner(cfg, Some(ChannelEventLog::new(cfg)));
    (out, log.expect("logger was constructed"))
}

#[allow(clippy::too_many_lines)]
fn run_panel_inner(
    cfg: &PanelCfg,
    mut log: Option<ChannelEventLog>,
) -> (PanelOut, Option<ChannelEventLog>) {
    let owd_ns = cfg.rtt_ms * MS / 2;
    let mut out = PanelOut {
        epoch_trigger: "NONE",
        first_echo_tick: u64::MAX,
        ..PanelOut::default()
    };

    // ── V2: ONE chain per DIRECTION, advanced chronologically per traversing packet ──────────
    let fk = forward_key(&cfg.seed_domain, &cfg.cell_id, cfg.repetition);
    let rk = reverse_key(&cfg.seed_domain, &cfg.cell_id, cfg.repetition);
    out.forward_chain_key = fk.clone();
    out.reverse_chain_key = rk.clone();
    // EXACTLY ONE chain object per direction for the entire run (2.1).
    let mut fwd = GeChain::new(fk, cfg.forward_model);
    let mut rev = GeChain::new(rk, cfg.reverse_model);

    // ── real product sender ─────────────────────────────────────────────────────────────────
    let mut s = Sender::new();
    if cfg.tail_on {
        s.set_tail_recovery_config(tail_on_config());
    }
    s.set_bootstrap_rtt(BootstrapRttConfig {
        enabled: cfg.bootstrap_on,
    });

    // ── real authenticated control component with TEST-INJECTED directional keys ────────────
    // In production these come from the Noise handshake Split(); no handshake exists in this repo.
    let k_tx: Zeroizing<[u8; 32]> = Zeroizing::new([0x3Au8; 32]); // sender → receiver
    let k_rx: Zeroizing<[u8; 32]> = Zeroizing::new([0x5Cu8; 32]); // receiver → sender
    let mut peer = KamaNoiseWireProtector::new(PANEL_H_EXT, &k_tx, &k_rx);
    let mut core = ReceiverControlCore::new();
    core.install_session_with_directional_keys(PANEL_H_EXT, &k_rx, &k_tx)
        .expect("install panel session");

    let mut ch = Chan {
        link_bps: LINK_BPS,
        owd_ns,
        link_free_ns: 0,
    };
    let elig: Vec<u64> = (0..WARMUP_UNITS)
        .map(|g| g * SEALED * 8 * 1_000_000_000 / OFFER_PACER_BPS)
        .collect();

    // harness state
    let mut seq_map: BTreeMap<u64, (Logical, bool)> = BTreeMap::new();
    let mut deliveries: Vec<(u64, Logical, u64)> = Vec::new();
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut delivered_src: BTreeSet<u16> = BTreeSet::new();
    let mut fed = 0u64;
    let mut now: u64 = 0;
    let mut tick: u64 = 0;
    let mut last_state = "";
    let mut wire_lines: Vec<String> = Vec::new();
    let mut src_prefix_lines: Vec<String> = Vec::new();

    // bootstrap wire state
    let mut probe_deliveries: Vec<(u64, Vec<u8>)> = Vec::new(); // (arrival, sealed datagram)
    let mut echo_deliveries: Vec<(u64, Vec<u8>)> = Vec::new(); // (arrival, sealed bytes)
    let mut probe_gen = 0usize;
    let mut echo_gen = 0usize;
    let mut probe_pn = 1u64;
    let mut echo_pn = 1u64;
    let mut bootstrap_succeeded = false;

    macro_rules! consume {
        ($decisions:expr) => {
            for d in $decisions {
                #[allow(unreachable_patterns)]
                match d.kind {
                    TransmitKind::Data => {
                        out.tx_data += 1;
                        let gid = (d.unit_id - BASE_UNIT_ID) as u16;
                        let idx = fwd.steps();
                        let ge_before = fwd.bad_state();
                        let drop = fwd.next_drop(); // forward chain, chronological
                        if let Some(l) = log.as_mut() {
                            l.record(GeDecision {
                                direction: ChanDirection::Forward,
                                directional_event_ordinal: idx,
                                simulation_tick: tick,
                                packet_kind: ChanPacketKind::Data,
                                unit_id: Some(d.unit_id),
                                ge_state_before: ge_before,
                                ge_state_after: fwd.bad_state(),
                                ge_decision_drop: drop,
                                applied_drop: drop,
                                script_overridden: false,
                            });
                        }
                        if out.forward_order.len() < 4096 {
                            out.forward_order.push(('D', idx));
                        }
                        seq_map.insert(d.seq, (Logical::Src(gid), true));
                        wire_lines.push(format!("D|{gid}|{}|{}", d.seq, u8::from(drop)));
                        src_prefix_lines.push(format!("{gid}|{}", u8::from(drop)));
                        out.data_wire_bytes += SEALED;
                        out.data_emit_ticks.push(tick);
                        if let Some(arr) = ch.offer(SEALED, now, drop) {
                            deliveries.push((arr, Logical::Src(gid), d.seq));
                        }
                    }
                    TransmitKind::Retransmit | TransmitKind::Duplicate => {
                        if d.kind == TransmitKind::Retransmit {
                            out.tx_retx += 1;
                        } else {
                            out.tx_dup += 1;
                        }
                        let gid = (d.unit_id - BASE_UNIT_ID) as u16;
                        let idx = fwd.steps();
                        let ge_before = fwd.bad_state();
                        let drop = fwd.next_drop(); // SAME forward chain as DATA
                        if let Some(l) = log.as_mut() {
                            // `d.kind` distinguishes the two classes AT the decision point; only
                            // `wire_lines` collapses them into one 'R' prefix.
                            l.record(GeDecision {
                                direction: ChanDirection::Forward,
                                directional_event_ordinal: idx,
                                simulation_tick: tick,
                                packet_kind: if d.kind == TransmitKind::Retransmit {
                                    ChanPacketKind::Retransmit
                                } else {
                                    ChanPacketKind::Duplicate
                                },
                                unit_id: Some(d.unit_id),
                                ge_state_before: ge_before,
                                ge_state_after: fwd.bad_state(),
                                ge_decision_drop: drop,
                                applied_drop: drop,
                                script_overridden: false,
                            });
                        }
                        if out.forward_order.len() < 4096 {
                            out.forward_order.push(
                                (if d.kind == TransmitKind::Retransmit { 'R' } else { 'U' }, idx),
                            );
                        }
                        seq_map.insert(d.seq, (Logical::Src(gid), false));
                        wire_lines.push(format!("R|{gid}|{}|{}", d.seq, u8::from(drop)));
                        out.retx_wire_bytes += SEALED;
                        if let Some(arr) = ch.offer(SEALED, now, drop) {
                            deliveries.push((arr, Logical::Src(gid), d.seq));
                        }
                    }
                    TransmitKind::CsProbe => {
                        // ── REAL product probe → REAL schema codec → REAL AEAD seal ──────────
                        if bootstrap_succeeded {
                            out.probe_after_success += 1;
                        }
                        let probe_seq = d.seq as u16;
                        let sealed = peer
                            .seal(
                                &UnsealedTransportPacket::Control(Control::CsProbe {
                                    // Idempotent request identity; NOT a ControlIdIssueGate id.
                                    id: 0x0C53_0000 | u32::from(probe_seq),
                                    subtype: 0x01,
                                    probe_seq,
                                }),
                                SealArgs {
                                    wire_pn: probe_pn,
                                    dh_epoch: 0,
                                    send_generation: 0,
                                },
                            )
                            .expect("seal cs probe")
                            .bytes;
                        probe_pn += 1;
                        if sealed.len() as u64 != PROBE_WIRE_EXACT || d.size != PROBE_WIRE_EXACT {
                            out.sender_budget_violation += 1;
                        }
                        out.probes_emitted += 1;
                        if out.first_probe_tick == u64::MAX {
                            out.first_probe_tick = tick;
                        }
                        out.probe_wire_bytes += sealed.len() as u64;
                        if out.probe_wire_bytes > SENDER_PROBE_BUDGET {
                            out.sender_budget_violation += 1;
                        }
                        out.lineage
                            .push(format!("t={now} probe_emission seq={probe_seq}"));
                        // SAME forward chain as DATA. The chain is advanced unconditionally so a
                        // scripted override cannot desynchronise the shared channel state.
                        let probe_idx = fwd.steps();
                        let ge_before = fwd.bad_state();
                        let chan_drop = fwd.next_drop();
                        let ge_after = fwd.bad_state();
                        if out.forward_order.len() < 4096 {
                            out.forward_order.push(('B', probe_idx));
                        }
                        let scripted = cfg
                            .probe_drop_script
                            .as_ref()
                            .and_then(|v| v.get(probe_gen).copied());
                        let drop = scripted.unwrap_or(chan_drop);
                        if let Some(l) = log.as_mut() {
                            // §5.1: the wire `unit_id` here is 0 and 0 is NEVER a DATA identity, so
                            // the record carries `None` and the Probe can never enter the DATA
                            // denominator.
                            l.record(GeDecision {
                                direction: ChanDirection::Forward,
                                directional_event_ordinal: probe_idx,
                                simulation_tick: tick,
                                packet_kind: ChanPacketKind::Probe,
                                unit_id: None,
                                ge_state_before: ge_before,
                                ge_state_after: ge_after,
                                ge_decision_drop: chan_drop,
                                applied_drop: drop,
                                script_overridden: scripted.is_some(),
                            });
                        }
                        probe_gen += 1;
                        wire_lines.push(format!("B|{probe_seq}|{}|{}", d.seq, u8::from(drop)));
                        // The frame is sealed EXACTLY ONCE, at emission, and carried through the
                        // channel verbatim — as a real datagram is.
                        if let Some(arr) = ch.offer(sealed.len() as u64, now, drop) {
                            probe_deliveries.push((arr, sealed));
                        }
                    }
                    // 2.3 DATAGRAM INVENTORY: every TransmitKind must either traverse a directional
                    // chain or be proven impossible here. FecParity is the only remaining variant and
                    // the panel never enables FEC, so it must never appear; if it ever does it is
                    // counted rather than silently swallowed by a catch-all.
                    TransmitKind::FecParity => {
                        out.channel_bypass_violations += 1;
                    }
                }
            }
        };
    }

    loop {
        // 1) feed eligible AppData
        while fed < WARMUP_UNITS && elig[fed as usize] <= now {
            let o = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: BASE_UNIT_ID + fed,
                    size: SEALED,
                }),
                now,
            );
            fed += 1;
            consume!(o);
        }

        // 2) observation-only state tracking
        let st = match s.state() {
            SenderState::PreConverge => "PreConverge",
            _ => "Post",
        };
        if st != last_state {
            if st != "PreConverge" && out.preconverge_exit_ns.is_none() {
                out.preconverge_exit_ns = Some(now);
                out.startup_floor_at_exit_bps = s.startup_pacing_floor_bps();
                out.lineage.push(format!("t={now} preconverge_exit"));
                out.lineage.push(format!(
                    "t={now} startup_pacing floor={} B/s",
                    out.startup_floor_at_exit_bps
                ));
            }
            last_state = st;
        }
        if out.srtt_set_ns.is_none() && s.path_timing_snapshot().srtt_ns.is_some() {
            out.srtt_set_ns = Some(now);
            out.lineage.push(format!(
                "t={now} srtt_established={}ns",
                s.path_timing_snapshot().srtt_ns.unwrap_or(0)
            ));
        }
        if out.bootstrap_edge_ns.is_none() {
            let d = s.bootstrap_rtt_diag();
            if d.arm > 0 {
                out.bootstrap_edge_ns = Some(now);
                out.lineage.push(format!("t={now} bootstrap_edge armed"));
            }
        }

        // 3) advance virtual time
        now += DT_NS;
        tick += 1;

        // 4) DATA deliveries due
        let mut due: Vec<(u64, Logical, u64)> = Vec::new();
        deliveries.retain(|&(arr, l, q)| {
            if arr <= now {
                due.push((arr, l, q));
                false
            } else {
                true
            }
        });
        for (arr, logical, seq) in due {
            if out.first_delivery_ns.is_none() {
                out.first_delivery_ns = Some(now);
            }
            let Logical::Src(gid) = logical;
            delivered_src.insert(gid);
            // ACK traverses the SAME reverse chain as Echo, in chronological emission order.
            out.acks_generated += 1;
            let rev_idx = rev.steps();
            let ge_before = rev.bad_state();
            let ack_drop = rev.next_drop();
            if let Some(l) = log.as_mut() {
                // §5.1: keyed `(Reverse, unit_id)`. `gid` here is numerically identical to the
                // FORWARD DATA key of the same unit; direction is what keeps a reverse-channel
                // fate from being scored as a forward DATA fate.
                l.record(GeDecision {
                    direction: ChanDirection::Reverse,
                    directional_event_ordinal: rev_idx,
                    simulation_tick: tick,
                    packet_kind: ChanPacketKind::Ack,
                    unit_id: Some(BASE_UNIT_ID + u64::from(gid)),
                    ge_state_before: ge_before,
                    ge_state_after: rev.bad_state(),
                    ge_decision_drop: ack_drop,
                    applied_drop: ack_drop,
                    script_overridden: false,
                });
            }
            if ack_drop {
                out.acks_reverse_dropped += 1;
            } else {
                acks.push((arr.saturating_add(owd_ns), seq));
            }
        }

        // 5) BOOTSTRAP PROBE arrivals → REAL ReceiverControlCore (authenticated ingest)
        let mut probes_due: Vec<Vec<u8>> = Vec::new();
        probe_deliveries.retain(|(arr, b)| {
            if *arr <= now {
                probes_due.push(b.clone());
                false
            } else {
                true
            }
        });
        for dg in probes_due {
            match core.ingest(PANEL_H_EXT, &dg) {
                Ok(CoreIngest::BootstrapProbe {
                    probe_seq,
                    control_id,
                }) => {
                    out.probes_authenticated += 1;
                    out.lineage.push(format!(
                        "t={now} authenticated_receiver_arrival seq={probe_seq}"
                    ));
                    if let Some(seq) = core.take_pending_cs_echo(PANEL_H_EXT) {
                        match core.emit_cs_echo(PANEL_H_EXT, seq, control_id, echo_pn) {
                            Ok(bytes) => {
                                echo_pn += 1;
                                out.echoes_generated += 1;
                                out.echo_wire_bytes += bytes.len() as u64;
                                if bytes.len() as u64 > ECHO_WIRE_MAX
                                    || out.echo_wire_bytes > ECHO_BUDGET_BYTES
                                    || out.echoes_generated > MAX_ECHOES as u64
                                {
                                    out.receiver_budget_violation += 1;
                                }
                                out.lineage.push(format!(
                                    "t={now} echo_generation seq={seq} wire={}B",
                                    bytes.len()
                                ));
                                // SAME reverse chain as ACK, advanced unconditionally.
                                let rev_idx = rev.steps();
                                let ge_before = rev.bad_state();
                                let chan_drop = rev.next_drop();
                                let ge_after = rev.bad_state();
                                let scripted = cfg
                                    .echo_drop_script
                                    .as_ref()
                                    .and_then(|v| v.get(echo_gen).copied());
                                let drop = scripted.unwrap_or(chan_drop);
                                if let Some(l) = log.as_mut() {
                                    // An Echo is not a DATA unit: no stable identity, so it never
                                    // enters the DATA denominator.
                                    l.record(GeDecision {
                                        direction: ChanDirection::Reverse,
                                        directional_event_ordinal: rev_idx,
                                        simulation_tick: tick,
                                        packet_kind: ChanPacketKind::Echo,
                                        unit_id: None,
                                        ge_state_before: ge_before,
                                        ge_state_after: ge_after,
                                        ge_decision_drop: chan_drop,
                                        applied_drop: drop,
                                        script_overridden: scripted.is_some(),
                                    });
                                }
                                echo_gen += 1;
                                wire_lines.push(format!("E|{seq}|{}", u8::from(drop)));
                                if !drop {
                                    // Reverse path mirrors the ACK model: one OWD, no reverse
                                    // serialization (the DATA path's reverse model is unchanged).
                                    out.lineage
                                        .push(format!("t={now} sealed_echo_emission seq={seq}"));
                                    let arr = now.saturating_add(owd_ns);
                                    echo_deliveries.push((arr, bytes.clone()));
                                    for _ in 0..cfg.echo_wire_duplicates {
                                        echo_deliveries.push((arr, bytes.clone()));
                                    }
                                }
                            }
                            Err(_) => {
                                // Bounded refusal is CORRECT behaviour, not a violation.
                            }
                        }
                    }
                }
                Ok(CoreIngest::Rejected(_)) => out.auth_failures += 1,
                Ok(_) => {}
                Err(_) => out.auth_failures += 1,
            }
        }

        // 6) BOOTSTRAP ECHO arrivals → REAL AEAD open → REAL decode → REAL sender ingest
        let mut echoes_due: Vec<Vec<u8>> = Vec::new();
        echo_deliveries.retain(|(arr, b)| {
            if *arr <= now {
                echoes_due.push(b.clone());
                false
            } else {
                true
            }
        });
        for bytes in echoes_due {
            let Ok(opened) = peer.open(&bytes) else {
                // Byte-identical wire duplicates are refused by the AEAD replay window.
                out.echo_replay_rejected += 1;
                continue;
            };
            let Some(Ok(Control::CsEcho { probe_seq, .. })) = decode_control(opened.inner()) else {
                out.auth_failures += 1;
                continue;
            };
            out.echoes_arrived += 1;
            out.lineage
                .push(format!("t={now} sender_match seq={probe_seq}"));
            let before = s.bootstrap_rtt_diag().echo_match_valid;
            let o = s.on_event(Event::CsProbeEcho { probe_seq }, now);
            consume!(o);
            let d = s.bootstrap_rtt_diag();
            if d.echo_match_valid > before && !bootstrap_succeeded {
                bootstrap_succeeded = true;
                out.first_echo_tick = tick;
                out.rtt_sample_ns = d.rtt_sample_ns;
                out.lineage
                    .push(format!("t={now} local_rtt_sample={}ns", d.rtt_sample_ns));
            }
            // Sender-side single-consumption guard against duplicates that survived the AEAD layer.
            for _ in 0..cfg.echo_event_duplicates {
                let o = s.on_event(Event::CsProbeEcho { probe_seq }, now);
                consume!(o);
            }
        }

        // 7) due ACKs → sender
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
            out.ack_batches += 1;
            if out.first_ack_ns.is_none() {
                out.first_ack_ns = Some(now);
            }
            let o = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: due_acks,
                    ack_delay: Duration::ZERO,
                }),
                now,
            );
            consume!(o);
        }

        // 8) unconditional per-tick PTO (spine cadence identity)
        let o = s.on_event(Event::TimerFired(Timer::Pto), now);
        consume!(o);

        // 9) termination
        let drained = delivered_src.len() as u64 >= WARMUP_UNITS
            && (0..WARMUP_UNITS as u16).all(|g| delivered_src.contains(&g));
        let timed_out = now >= cfg.warmup_timeout_ns;
        if (fed >= WARMUP_UNITS && drained) || timed_out {
            out.epoch_trigger = if fed >= WARMUP_UNITS && drained {
                "DRAIN"
            } else {
                "TIMEOUT"
            };
            out.observation_epoch_ns = now;
            out.warmup_complete = drained && !timed_out;
            out.delivered_src_at_epoch = delivered_src.len() as u64;
            break;
        }
    }

    let d = s.bootstrap_rtt_diag();
    out.echo_match_valid = d.echo_match_valid;
    out.echo_match_rejected = d.echo_match_rejected;
    // §4 origin attribution (observation-only): what the SHIPPING emitter claims. Read here, next
    // to the other diag readbacks, so the two probe origins are never conflated.
    out.shipping_probes_emitted = d.probe_emitted;
    out.duplicate_rtt_samples = d.echo_match_valid.saturating_sub(1);
    if d.budget_violation > 0 || d.probe_wire_bytes > SENDER_PROBE_BUDGET {
        out.sender_budget_violation += 1;
    }
    if d.probe_after_success_violation > 0 {
        out.probe_after_success += d.probe_after_success_violation;
    }
    let tp = s.tail_probe_diag_ext();
    out.tail_probe = (tp.0, tp.1, tp.2);
    out.cold_start_stall = out.preconverge_exit_ns.is_none();
    if let Some((gens, bytes, budget)) = core.echo_budget(PANEL_H_EXT) {
        if gens > MAX_ECHOES || bytes > budget {
            out.receiver_budget_violation += 1;
        }
    }
    out.forward_steps = fwd.steps();
    out.reverse_steps = rev.steps();
    // C2 §4 outcome class. NETWORK-CENSORED is its own class and stays in the denominator; it is
    // never a PASS and never a mechanism defect.
    out.outcome_class = if !cfg.bootstrap_on {
        "NO-BOOTSTRAP"
    } else if out.echoes_arrived > 0 && out.probes_authenticated > 0 {
        "RTT-OPPORTUNITY"
    } else if out.probes_emitted > 0 {
        "NETWORK-CENSORED"
    } else {
        "NO-BOOTSTRAP"
    };
    out.wire_digest = sha256_hex(wire_lines.join("\n").as_bytes());
    out.source_prefix_digest = sha256_hex(src_prefix_lines.join("\n").as_bytes());
    (out, log)
}

/// **METRIC CONTRACT** `DR-CS-3-DISPLACEMENT-METRIC-CONTRACT` (md `61d64d55…`).
///
/// DATA the treatment genuinely failed to emit BECAUSE of the treatment, measured strictly before
/// the first Probe reaches the wire — the window where the two arms MUST be identical. Any non-zero
/// value is a real scheduler defect. Safety-gate input (C2 gate 5).
#[must_use]
pub fn scheduler_data_displacement(baseline: &PanelOut, treatment: &PanelOut) -> u64 {
    let horizon = treatment.first_probe_tick; // exclusive
    let t: BTreeSet<u64> = treatment
        .data_emit_ticks
        .iter()
        .copied()
        .filter(|&x| x < horizon)
        .collect();
    baseline
        .data_emit_ticks
        .iter()
        .copied()
        .filter(|&x| x < horizon && !t.contains(&x))
        .count() as u64
}

/// How often DATA fates differ AFTER an extra Probe entered the shared packet-domain chain.
///
/// **Expected non-zero and NOT a defect.** One extra packet advances the shared GE chain by one
/// step, which legitimately re-phases every subsequent fate. Descriptive only — never a gate, never
/// reported as displacement.
#[must_use]
pub fn channel_fate_divergence(baseline: &PanelOut, treatment: &PanelOut) -> u64 {
    let from = treatment.first_probe_tick;
    if from == u64::MAX {
        return 0; // no extra packet ever entered the chain
    }
    let b: BTreeSet<u64> = baseline
        .data_emit_ticks
        .iter()
        .copied()
        .filter(|&x| x >= from)
        .collect();
    let t: BTreeSet<u64> = treatment
        .data_emit_ticks
        .iter()
        .copied()
        .filter(|&x| x >= from)
        .collect();
    b.symmetric_difference(&t).count() as u64
}

/// Echo-bounded reporting helper: `None` renders as `N/A — NO ECHO`, never as a whole-trace horizon.
#[must_use]
pub fn echo_bounded_window(treatment: &PanelOut) -> Option<u64> {
    (treatment.first_echo_tick != u64::MAX).then_some(treatment.first_echo_tick)
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// THIRTEEN CONTROLS — every one drives the SAME production logger / comparator path
//
// Compiled only into a test target (`cfg(test)` is set by rustc's `--test`, which cargo passes for
// `tests/*.rs`); the example binary that also `#[path]`-includes this module never builds them.
// Each control asserts an EXACT expected count. Nothing is aggregated, and no control is allowed to
// pass vacuously — every one first asserts its fixture actually produced the situation it targets.
// ═══════════════════════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod channel_fate_controls {
    use super::*;

    const U: u64 = BASE_UNIT_ID;

    /// Push ONE synthetic decision through the production `record` path.
    #[allow(clippy::too_many_arguments)]
    fn push(
        log: &mut ChannelEventLog,
        direction: ChanDirection,
        ordinal: u64,
        tick: u64,
        kind: ChanPacketKind,
        unit_id: Option<u64>,
        drop: bool,
    ) {
        log.record(GeDecision {
            direction,
            directional_event_ordinal: ordinal,
            simulation_tick: tick,
            packet_kind: kind,
            unit_id,
            ge_state_before: false,
            ge_state_after: drop,
            ge_decision_drop: drop,
            applied_drop: drop,
            script_overridden: false,
        });
    }

    fn control_pair() -> (ChannelEventLog, ChannelEventLog) {
        (
            ChannelEventLog::control_log(
                PanelArm::Baseline,
                EventCapPlan::control_plan(4096, 4096),
            ),
            ChannelEventLog::control_log(
                PanelArm::Treatment,
                EventCapPlan::control_plan(4096, 4096),
            ),
        )
    }

    /// Boundary-at-forward-ordinal-1 fixture: OFF = three DATA; ON = the same three DATA with one
    /// Probe spliced in after the first, exactly as an extra forward packet does.
    fn probe_boundary_fixture() -> (ChannelEventLog, ChannelEventLog) {
        let (mut off, mut on) = control_pair();
        let f = ChanDirection::Forward;
        push(&mut off, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut off, f, 1, 11, ChanPacketKind::Data, Some(U + 1), false);
        push(&mut off, f, 2, 12, ChanPacketKind::Data, Some(U + 2), false);
        push(&mut on, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut on, f, 1, 11, ChanPacketKind::Probe, Some(0), false);
        push(&mut on, f, 2, 11, ChanPacketKind::Data, Some(U + 1), false);
        push(&mut on, f, 3, 12, ChanPacketKind::Data, Some(U + 2), false);
        (off, on)
    }

    /// A cheap real panel: rep16 of the frozen locked cell stalls, so the whole 12 s virtual
    /// horizon costs a fraction of a second and the bootstrap ladder still fires at 3 s and 9 s.
    fn real_cfg(bootstrap_on: bool) -> PanelCfg {
        PanelCfg {
            cell_id: "P2-GE20L10-RTT1000".into(),
            repetition: 16,
            seed_domain: "FSFC2-DIAG42B".into(),
            rtt_ms: 1000,
            forward_model: LossModel::Ge {
                p_ppm: 200_000,
                l: 10,
            },
            reverse_model: LossModel::None,
            tail_on: false,
            bootstrap_on,
            warmup_timeout_ns: 12_000_000_000,
            probe_drop_script: None,
            echo_drop_script: None,
            echo_wire_duplicates: 0,
            echo_event_duplicates: 0,
        }
    }

    /// A cheap CONVERGING panel: no loss at RTT 50 ms drains all 384 sources long before the
    /// horizon, so BOTH directions carry real traffic (the stalling `real_cfg` produces no ACK at
    /// all, which would let a reverse assertion pass vacuously).
    fn converging_cfg(bootstrap_on: bool) -> PanelCfg {
        PanelCfg {
            cell_id: "CTRL-NOLOSS-RTT50".into(),
            repetition: 3,
            seed_domain: "DRCS3-CONTROL-V2".into(),
            rtt_ms: 50,
            forward_model: LossModel::None,
            reverse_model: LossModel::None,
            tail_on: false,
            bootstrap_on,
            warmup_timeout_ns: 12_000_000_000,
            probe_drop_script: None,
            echo_drop_script: None,
            echo_wire_duplicates: 0,
            echo_event_duplicates: 0,
        }
    }

    fn fwd(c: &ChannelFateComparison) -> &DirectionReport {
        c.forward.as_ref().expect("valid pairing reports forward")
    }
    fn rev(c: &ChannelFateComparison) -> &DirectionReport {
        c.reverse.as_ref().expect("valid pairing reports reverse")
    }

    // ── CONTROL 1 ──────────────────────────────────────────────────────────────────────────────
    /// Identical trace ⇒ matched fate divergence 0, off-only 0, on-only 0.
    #[test]
    fn control_01_identical_trace_has_no_divergence() {
        let cfg = converging_cfg(false);
        let (out, a) = run_panel_logged(&cfg);
        let (_, b) = run_panel_logged(&cfg);
        assert_eq!(out.epoch_trigger, "DRAIN", "fixture must actually converge");
        assert!(
            out.acks_generated > 0,
            "fixture must produce reverse traffic"
        );
        assert_eq!(a.events, b.events, "the panel must be reproducible");
        let c = compare_channel_fates(&a, &b);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        for r in [fwd(&c), rev(&c)] {
            assert!(
                r.matched_data_attempts > 0,
                "{} control must not pass vacuously",
                r.direction.label()
            );
            assert_eq!(r.matched_data_attempt_fate_divergences, 0);
            assert_eq!(r.off_only_data_attempts, 0);
            assert_eq!(r.on_only_data_attempts, 0);
            assert_eq!(r.first_treatment_induced_channel_event, None);
            assert_eq!(r.event_ordinal_rephased, 0);
        }
    }

    // ── CONTROL 2 ──────────────────────────────────────────────────────────────────────────────
    /// Exactly ONE matched fate flip ⇒ matched fate divergence == 1, and the four directional fate
    /// counts localise it.
    #[test]
    fn control_02_one_matched_fate_flip_counts_exactly_one() {
        let (off, mut on) = probe_boundary_fixture();
        let base = compare_channel_fates(&off, &on);
        assert_eq!(fwd(&base).matched_data_attempt_fate_divergences, 0);
        // flip exactly one POST-boundary matched attempt: unit U+2, attempt 0, treatment arm.
        let target = on
            .events
            .iter_mut()
            .find(|e| e.unit_id == Some(U + 2))
            .expect("fixture has the attempt");
        target.actual_fate = ChannelFate::Dropped;
        target.ge_decision_fate = ChannelFate::Dropped;

        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        let r = fwd(&c);
        assert_eq!(r.matched_data_attempts, 3);
        assert_eq!(r.matched_data_attempt_fate_divergences, 1);
        assert_eq!(r.fate_cross_tab.off_delivered_on_dropped, 1);
        assert_eq!(r.fate_cross_tab.off_dropped_on_delivered, 0);
        assert_eq!(r.fate_cross_tab.both_delivered, 2);
        assert_eq!(r.fate_cross_tab.both_dropped, 0);
        assert_eq!(r.off_only_data_attempts, 0);
        assert_eq!(r.on_only_data_attempts, 0);
        assert_eq!(r.attempt_class_divergences, 0);
    }

    // ── CONTROL 3 ──────────────────────────────────────────────────────────────────────────────
    /// Trajectory-only divergence: an attempt present in ONE arm only. Matched fate divergence
    /// stays 0 and the one-sided attempt is counted separately — never folded into the matched
    /// count (addendum §6).
    #[test]
    fn control_03_trajectory_only_is_never_a_matched_fate_divergence() {
        let (mut off, mut on) = control_pair();
        let f = ChanDirection::Forward;
        push(&mut off, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut off, f, 1, 11, ChanPacketKind::Data, Some(U + 1), false);
        push(
            &mut off,
            f,
            2,
            20,
            ChanPacketKind::Retransmit,
            Some(U + 1),
            true,
        );
        push(&mut on, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut on, f, 1, 11, ChanPacketKind::Probe, Some(0), false);
        push(&mut on, f, 2, 11, ChanPacketKind::Data, Some(U + 1), false);

        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        let r = fwd(&c);
        assert_eq!(r.matched_data_attempts, 2);
        assert_eq!(r.matched_data_attempt_fate_divergences, 0);
        assert_eq!(r.off_only_data_attempts, 1);
        assert_eq!(r.on_only_data_attempts, 0);
        assert_eq!(r.non_data_channel_events_off, 0);
        assert_eq!(r.non_data_channel_events_on, 1);
    }

    // ── CONTROL 4 ──────────────────────────────────────────────────────────────────────────────
    /// Attempts 0 and 1 of the SAME unit match separately and correctly — and the ordinal is not
    /// the tick, not the directional event ordinal and not the global event ordinal.
    #[test]
    fn control_04_attempt_ordinals_match_separately_and_are_not_derived() {
        let (mut off, mut on) = control_pair();
        let f = ChanDirection::Forward;
        for log in [&mut off, &mut on] {
            push(log, f, 0, 500, ChanPacketKind::Data, Some(U + 9), false);
            push(log, f, 1, 500, ChanPacketKind::Data, Some(U + 8), false);
            push(
                log,
                f,
                2,
                733,
                ChanPacketKind::Retransmit,
                Some(U + 9),
                true,
            );
        }
        push(&mut on, f, 3, 900, ChanPacketKind::Probe, Some(0), false);

        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        let r = fwd(&c);
        assert_eq!(r.matched_data_attempts, 3);
        assert_eq!(r.matched_data_attempt_fate_divergences, 0);
        let a0 = r
            .matched
            .iter()
            .find(|m| m.key.unit_id == U + 9 && m.key.attempt_ordinal == 0)
            .expect("attempt 0 of U+9 matched on its own key");
        let a1 = r
            .matched
            .iter()
            .find(|m| m.key.unit_id == U + 9 && m.key.attempt_ordinal == 1)
            .expect("attempt 1 of U+9 matched on its own key");
        assert_eq!(a0.off_fate, ChannelFate::Delivered);
        assert_eq!(a0.on_fate, ChannelFate::Delivered);
        assert_eq!(a1.off_fate, ChannelFate::Dropped);
        assert_eq!(a1.on_fate, ChannelFate::Dropped);
        assert_eq!(a0.off_class, Some(TransmissionClass::Data));
        assert_eq!(a1.off_class, Some(TransmissionClass::Retransmit));
        // NOT derived from queue position / tick / global event ordinal.
        let e_u8 = off
            .events
            .iter()
            .find(|e| e.unit_id == Some(U + 8))
            .expect("fixture");
        assert_eq!(e_u8.attempt_ordinal, Some(0));
        assert_eq!(e_u8.directional_event_ordinal, 1, "ordinal 1, attempt 0");
        assert_eq!(e_u8.global_ordinal, 1);
        assert_eq!(e_u8.simulation_tick, 500);
        let e_r9 = off
            .events
            .iter()
            .find(|e| e.packet_kind == ChanPacketKind::Retransmit)
            .expect("fixture");
        assert_eq!(e_r9.attempt_ordinal, Some(1));
        assert_eq!(e_r9.directional_event_ordinal, 2, "ordinal 2, attempt 1");
        assert_eq!(e_r9.simulation_tick, 733);
        // Two emissions can share a tick, which is exactly why the tick can never be an identity.
        assert_eq!(off.events[0].simulation_tick, off.events[1].simulation_tick);
    }

    // ── CONTROL 5 ──────────────────────────────────────────────────────────────────────────────
    /// A duplicate key or an ordinal gap is FATAL RED — and the comparator then reports NO metric.
    #[test]
    fn control_05_duplicate_or_gap_is_fatal_red() {
        let f = ChanDirection::Forward;
        // clean baseline: integrity holds.
        let (mut clean, _) = control_pair();
        push(&mut clean, f, 0, 1, ChanPacketKind::Data, Some(U), false);
        push(
            &mut clean,
            f,
            1,
            2,
            ChanPacketKind::Retransmit,
            Some(U),
            false,
        );
        assert!(clean.attempt_ordinal_check().is_ok());

        // duplicate key
        let (mut dup, _) = control_pair();
        push(&mut dup, f, 0, 1, ChanPacketKind::Data, Some(U), false);
        dup.attempts.control_preset(f, U, 0); // force the issuer to re-issue 0
        push(
            &mut dup,
            f,
            1,
            2,
            ChanPacketKind::Retransmit,
            Some(U),
            false,
        );
        let e = dup.attempt_ordinal_check().expect_err("must be fatal");
        assert!(e.contains("DUPLICATE ATTEMPT KEY"), "{e}");
        assert_eq!(authoritative_key_collisions(&dup), 1);

        // ordinal gap
        let (mut gap, _) = control_pair();
        gap.attempts.control_preset(f, U + 1, 5);
        push(&mut gap, f, 0, 1, ChanPacketKind::Data, Some(U + 1), false);
        let e = gap.attempt_ordinal_check().expect_err("must be fatal");
        assert!(e.contains("ATTEMPT ORDINAL GAP"), "{e}");

        // and the pairing is void in both cases — no fate metric is reported.
        for bad in [&dup, &gap] {
            let (ok, _) = control_pair();
            let c = compare_channel_fates(bad, &ok);
            assert!(matches!(
                c.verdict,
                PairingVerdict::InstrumentInvalid { .. }
            ));
            assert!(c.forward.is_none() && c.reverse.is_none());
        }
    }

    /// Overflow of the `u32` attempt ordinal is fatal, not wrapping.
    #[test]
    fn control_05b_attempt_ordinal_overflow_is_fatal() {
        let f = ChanDirection::Forward;
        let (mut log, _) = control_pair();
        log.attempts.control_preset(f, U, u32::MAX);
        push(&mut log, f, 0, 1, ChanPacketKind::Data, Some(U), false);
        assert!(log.attempts.overflow, "overflow must be flagged");
        let e = log.attempt_ordinal_check().expect_err("must be fatal");
        assert!(e.contains("ATTEMPT-ORDINAL-OVERFLOW"), "{e}");
    }

    // ── CONTROL 6 ──────────────────────────────────────────────────────────────────────────────
    /// Direction isolation: the SAME `unit_id` at the SAME attempt ordinal exists once forward
    /// (DATA) and once reverse (its ACK). With `direction` in the key they are two attempts;
    /// removing it collides them and the collision control goes RED.
    #[test]
    fn control_06_direction_isolation() {
        let (mut off, mut on) = control_pair();
        let (f, r) = (ChanDirection::Forward, ChanDirection::Reverse);
        push(&mut off, f, 0, 10, ChanPacketKind::Data, Some(U + 7), false);
        push(&mut off, r, 0, 20, ChanPacketKind::Ack, Some(U + 7), false);
        push(&mut on, f, 0, 10, ChanPacketKind::Data, Some(U + 7), false);
        push(&mut on, f, 1, 11, ChanPacketKind::Probe, Some(0), false);
        push(&mut on, r, 0, 19, ChanPacketKind::Echo, None, false);
        push(&mut on, r, 1, 20, ChanPacketKind::Ack, Some(U + 7), true);

        // AUTHORITATIVE key (direction included): no collision anywhere.
        assert_eq!(authoritative_key_collisions(&off), 0);
        assert_eq!(authoritative_key_collisions(&on), 0);
        // DIRECTION-ERASED key: the forward DATA and the reverse ACK of unit U+7 collide — RED.
        assert_eq!(direction_erased_key_collisions(&off), 1);
        assert_eq!(direction_erased_key_collisions(&on), 1);

        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        // The reverse-channel fate difference is scored REVERSE, never against the forward DATA.
        assert_eq!(fwd(&c).matched_data_attempts, 1);
        assert_eq!(fwd(&c).matched_data_attempt_fate_divergences, 0);
        assert_eq!(rev(&c).matched_data_attempts, 1);
        assert_eq!(rev(&c).matched_data_attempt_fate_divergences, 1);
        assert_eq!(rev(&c).fate_cross_tab.off_delivered_on_dropped, 1);
    }

    // ── CONTROL 7 ──────────────────────────────────────────────────────────────────────────────
    /// A Probe NEVER enters the DATA denominator. Synthetic half: even when the wire `unit_id` 0 is
    /// handed to the logger it is discarded. Real half: a live panel's Probes and Echoes all land
    /// in `non_data_channel_events` and none of them contributes a matched key.
    #[test]
    fn control_07_probe_never_enters_the_data_denominator() {
        let (mut off, mut on) = control_pair();
        let f = ChanDirection::Forward;
        push(&mut off, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut on, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut on, f, 1, 11, ChanPacketKind::Probe, Some(0), false);
        let probe = on
            .events
            .iter()
            .find(|e| e.packet_kind == ChanPacketKind::Probe)
            .expect("fixture");
        assert_eq!(probe.unit_id, None, "unit_id 0 is NEVER a DATA identity");
        assert_eq!(probe.attempt_ordinal, None);
        assert_eq!(probe.attempt_key(), None);
        let c = compare_channel_fates(&off, &on);
        assert_eq!(fwd(&c).matched_data_attempts, 1);
        assert_eq!(fwd(&c).non_data_channel_events_on, 1);

        // Real panel with Probes AND Echoes on the wire.
        let mut cfg = real_cfg(true);
        cfg.probe_drop_script = Some(vec![false; 4]); // force delivery so echoes exist
        let (out, log) = run_panel_logged(&cfg);
        assert!(out.probes_emitted > 0, "control must not pass vacuously");
        assert!(out.echoes_generated > 0, "control must not pass vacuously");
        for e in &log.events {
            if matches!(e.packet_kind, ChanPacketKind::Probe | ChanPacketKind::Echo) {
                assert_eq!(e.unit_id, None);
                assert_eq!(e.attempt_ordinal, None);
                assert_eq!(e.transmission_class, None);
                assert!(e.treatment_exclusive_event);
            } else {
                assert!(e.unit_id.is_some_and(|u| u >= BASE_UNIT_ID));
                assert!(!e.treatment_exclusive_event);
            }
        }
        assert_eq!(
            log.count_kind(ChanPacketKind::Probe),
            out.probes_emitted,
            "every Probe that traversed the chain is recorded exactly once"
        );
        assert_eq!(log.count_kind(ChanPacketKind::Echo), out.echoes_generated);
    }

    // ── CONTROL 8 ──────────────────────────────────────────────────────────────────────────────
    /// Class-only divergence: the matched key is PRESERVED (the attempt stays matched, is not split
    /// into two unmatched attempts) and only `attempt_class_divergences` increments.
    #[test]
    fn control_08_class_only_divergence_keeps_the_matched_key() {
        let (mut off, mut on) = control_pair();
        let f = ChanDirection::Forward;
        push(&mut off, f, 0, 10, ChanPacketKind::Data, Some(U + 3), false);
        push(
            &mut off,
            f,
            1,
            30,
            ChanPacketKind::Retransmit,
            Some(U + 3),
            true,
        );
        push(&mut on, f, 0, 10, ChanPacketKind::Data, Some(U + 3), false);
        push(&mut on, f, 1, 11, ChanPacketKind::Probe, Some(0), false);
        push(
            &mut on,
            f,
            2,
            30,
            ChanPacketKind::Duplicate,
            Some(U + 3),
            true,
        );

        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        let r = fwd(&c);
        assert_eq!(
            r.matched_data_attempts, 2,
            "the key survived the class change"
        );
        assert_eq!(r.off_only_data_attempts, 0);
        assert_eq!(r.on_only_data_attempts, 0);
        assert_eq!(r.matched_data_attempt_fate_divergences, 0);
        assert_eq!(r.attempt_class_divergences, 1);
        let m = r
            .matched
            .iter()
            .find(|m| m.key.attempt_ordinal == 1)
            .expect("attempt 1 matched");
        assert_eq!(m.off_class, Some(TransmissionClass::Retransmit));
        assert_eq!(m.on_class, Some(TransmissionClass::Duplicate));
    }

    // ── CONTROL 9 ──────────────────────────────────────────────────────────────────────────────
    /// A SINGLE pre-boundary fate / GE-state / identity change ⇒
    /// `PRE-TREATMENT-PREFIX-MISMATCH — INVALID PAIRING`, and no fate metric is reported from it.
    #[test]
    fn control_09_pre_treatment_prefix_mismatch() {
        let f = ChanDirection::Forward;
        let build = || {
            let (mut off, mut on) = control_pair();
            for log in [&mut off, &mut on] {
                push(log, f, 0, 10, ChanPacketKind::Data, Some(U), false);
                push(log, f, 1, 11, ChanPacketKind::Data, Some(U + 1), false);
                push(log, f, 2, 12, ChanPacketKind::Data, Some(U + 2), false);
            }
            push(&mut on, f, 3, 13, ChanPacketKind::Probe, Some(0), false);
            (off, on)
        };
        // sanity: the untouched fixture is a VALID pairing with the boundary at ordinal 3.
        let (off, on) = build();
        let ok = compare_channel_fates(&off, &on);
        assert!(ok.verdict.is_valid());
        assert_eq!(fwd(&ok).first_treatment_induced_channel_event, Some(3));

        // (a) one pre-boundary FATE change
        let (off, mut on) = build();
        on.events[1].actual_fate = ChannelFate::Dropped;
        // (b) one pre-boundary GE-STATE change
        let (off_b, mut on_b) = build();
        on_b.events[1].ge_state_after = true;
        // (c) one pre-boundary IDENTITY change (a different unit, with no treatment cause)
        let (off_c, mut on_c) = build();
        on_c.events[1].unit_id = Some(U + 42);
        // (d) an attempt-ordinal identity change is structurally impossible without a gap or a
        //     duplicate; a gap is caught one step earlier as INSTRUMENT INVALID, still void.
        let (mut off_d, mut on_d) = control_pair();
        push(&mut off_d, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        on_d.attempts.control_preset(f, U, 1);
        push(&mut on_d, f, 0, 10, ChanPacketKind::Data, Some(U), false);

        for (tag, a, b) in [
            ("fate", &off, &on),
            ("ge-state", &off_b, &on_b),
            ("identity", &off_c, &on_c),
            ("attempt-ordinal", &off_d, &on_d),
        ] {
            let c = compare_channel_fates(a, b);
            assert!(
                matches!(
                    c.verdict,
                    PairingVerdict::PreTreatmentPrefixMismatch { .. }
                        | PairingVerdict::InstrumentInvalid { .. }
                ),
                "{tag}: expected an invalid pairing, got {}",
                c.verdict.label()
            );
            assert!(
                c.forward.is_none() && c.reverse.is_none(),
                "{tag}: a void pairing must report NO fate metric"
            );
            assert!(
                c.report_text().contains("pairing void") || c.report_text().contains("INVALID")
            );
        }
    }

    // ── CONTROL 10 ─────────────────────────────────────────────────────────────────────────────
    /// A POST-boundary re-phase (different directional event ordinal for the same matched attempt)
    /// is ACCEPTED — the attempt stays matched, it is tagged `event_ordinal_rephased`, and the
    /// report emits no physical-event causal wording.
    #[test]
    fn control_10_post_boundary_rephase_is_accepted() {
        let (off, on) = probe_boundary_fixture();
        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        let r = fwd(&c);
        assert_eq!(r.first_treatment_induced_channel_event, Some(1));
        assert_eq!(r.matched_data_attempts, 3);
        assert_eq!(r.event_ordinal_aligned, 1, "only the pre-boundary attempt");
        assert_eq!(r.event_ordinal_rephased, 2, "both post-boundary attempts");
        assert_eq!(r.pre_boundary_matched, 1);
        assert_eq!(r.post_boundary_matched, 2);
        let m = r
            .matched
            .iter()
            .find(|m| m.key.unit_id == U + 1)
            .expect("fixture");
        assert_eq!(m.boundary_side, BoundarySide::PostBoundary);
        assert_eq!(m.ordinal_alignment, OrdinalAlignment::Rephased);
        assert_eq!(m.off_ordinal, 1);
        assert_eq!(m.on_ordinal, 2);
        assert_eq!(m.prior_prefix, PriorPrefix::Aligned);

        let text = c.report_text().to_lowercase();
        for bad in FORBIDDEN_CLAIM_FRAGMENTS {
            assert!(
                !text.contains(bad),
                "report emitted forbidden wording: {bad}"
            );
        }
        assert!(c.report_text().contains(AUTHORITATIVE_MATCHED_FATE_CLAIM));
        assert!(c.report_text().contains("ACCEPTED"));
    }

    // ── CONTROL 11 ─────────────────────────────────────────────────────────────────────────────
    /// FULL key-set membership scan. A zip-only comparator both LOSES the unmatched attempt and
    /// FABRICATES a fate divergence by mis-pairing everything after it. The production comparator
    /// does neither.
    #[test]
    fn control_11_zip_only_comparator_misses_an_unmatched_event() {
        let (mut off, mut on) = control_pair();
        let f = ChanDirection::Forward;
        push(&mut off, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut off, f, 1, 11, ChanPacketKind::Data, Some(U + 1), false);
        push(&mut on, f, 0, 10, ChanPacketKind::Data, Some(U), false);
        push(&mut on, f, 1, 11, ChanPacketKind::Probe, Some(0), false);
        push(&mut on, f, 2, 12, ChanPacketKind::Duplicate, Some(U), true);
        push(&mut on, f, 3, 13, ChanPacketKind::Data, Some(U + 1), false);

        let c = compare_channel_fates(&off, &on);
        assert!(c.verdict.is_valid(), "{}", c.verdict.label());
        let r = fwd(&c);
        assert_eq!(r.matched_data_attempts, 2);
        assert_eq!(r.matched_data_attempt_fate_divergences, 0);
        assert_eq!(r.on_only_data_attempts, 1);
        assert_eq!(r.off_only_data_attempts, 0);
        let total_keys =
            r.matched_data_attempts + r.off_only_data_attempts + r.on_only_data_attempts;
        assert_eq!(total_keys, 3);

        let (compared, diverged) =
            zip_only_fate_divergences_negative_fixture(&off, &on, ChanDirection::Forward);
        assert_eq!(compared, 2, "the zip never sees the third key");
        assert!(
            compared < total_keys,
            "the zip's denominator must be short by the unmatched attempt"
        );
        assert_eq!(
            diverged, 1,
            "the zip fabricates a divergence by mis-pairing after the unmatched attempt"
        );
        assert_ne!(
            diverged, r.matched_data_attempt_fate_divergences,
            "the control must actually catch the zip"
        );
    }

    // ── CONTROL 12 ─────────────────────────────────────────────────────────────────────────────
    /// Event-cap boundary: exactly at the cap is PASS, cap+1 is `EVENT-LOG-OVERFLOW — M2 RED`, and
    /// nothing is ever truncated. A producer the configuration says is absent is
    /// `EVENT-CAP-CONFIG-MISMATCH — M2 RED`.
    #[test]
    fn control_12_event_cap_boundary() {
        let f = ChanDirection::Forward;
        const CAP: u64 = 8;
        let mut log =
            ChannelEventLog::control_log(PanelArm::Baseline, EventCapPlan::control_plan(CAP, 0));
        for i in 0..CAP {
            push(&mut log, f, i, i, ChanPacketKind::Data, Some(U + i), false);
        }
        let rep = log.cap_report();
        assert_eq!(rep.observed_forward, CAP);
        assert_eq!(rep.forward_cap, CAP);
        assert_eq!(rep.verdict, CapVerdict::Pass, "exactly at cap must PASS");
        assert_eq!(rep.verdict.label(), "PASS");

        push(
            &mut log,
            f,
            CAP,
            CAP,
            ChanPacketKind::Data,
            Some(U + CAP),
            false,
        );
        let rep = log.cap_report();
        assert!(matches!(rep.verdict, CapVerdict::Overflow(_)));
        assert_eq!(rep.verdict.label(), "EVENT-LOG-OVERFLOW — M2 RED");
        assert_eq!(rep.observed_forward, CAP + 1);
        assert_eq!(
            log.events.len() as u64,
            CAP + 1,
            "the logger must NEVER truncate — it reports overflow instead"
        );

        // config mismatch: a Probe where the derived probe bound is 0.
        let mut m =
            ChannelEventLog::control_log(PanelArm::Baseline, EventCapPlan::control_plan(64, 64));
        assert_eq!(m.caps.probe_cap, 0);
        push(&mut m, f, 0, 0, ChanPacketKind::Probe, Some(0), false);
        let rep = m.cap_report();
        assert!(matches!(rep.verdict, CapVerdict::ConfigMismatch(_)));
        assert_eq!(rep.verdict.label(), "EVENT-CAP-CONFIG-MISMATCH — M2 RED");
    }

    /// The derived cap is honoured by real runs in THIS compile configuration, and the report
    /// carries the full per-configuration inventory.
    #[test]
    fn control_12b_real_runs_stay_within_the_derived_caps() {
        for bootstrap_on in [false, true] {
            for tail_on in [false, true] {
                let mut cfg = real_cfg(bootstrap_on);
                cfg.tail_on = tail_on;
                let (_, log) = run_panel_logged(&cfg);
                let rep = log.cap_report();
                assert!(
                    rep.verdict.is_pass(),
                    "bootstrap_on={bootstrap_on} tail_on={tail_on}: {} ({:?})",
                    rep.verdict.label(),
                    rep.verdict
                );
                assert!(rep.observed_forward <= rep.forward_cap);
                assert!(rep.observed_reverse <= rep.reverse_cap);
                assert!(rep.observed_total <= rep.total_cap);
                assert_eq!(rep.feature_fingerprint.len(), 5);
                assert!(rep.producers.len() == 12);
                assert!(rep.absent_producers.contains(&"F8"));
                assert!(rep.absent_producers.contains(&"R3"));
                assert_eq!(rep.legacy_ladder_compiled, legacy_ladder_compiled());
                // The legacy CS-probe bound enters ONLY where it is actually compiled.
                let f9 = rep.producers.iter().find(|p| p.id == "F9").expect("F9 row");
                assert_eq!(f9.compiled, legacy_ladder_compiled());
                if !legacy_ladder_compiled() {
                    assert_eq!(f9.bound, 0);
                    assert_eq!(f9.basis, "ABSENT");
                }
            }
        }
    }

    // ── CONTROL 13 ─────────────────────────────────────────────────────────────────────────────
    /// With the logger ON, NO new packet, event, DATA, ACK, Probe, parity or recovery is produced.
    /// The whole `PanelOut` — every counter, both digests, both directional draw counts, the
    /// terminal classification and the full lineage — is identical to the logger-OFF run.
    #[test]
    fn control_13_logger_produces_no_packet_and_no_event() {
        for bootstrap_on in [false, true] {
            for tail_on in [false, true] {
                let mut cfg = real_cfg(bootstrap_on);
                cfg.tail_on = tail_on;
                let off_logger = run_panel(&cfg);
                let (on_logger, log) = run_panel_logged(&cfg);
                assert_eq!(
                    off_logger, on_logger,
                    "OBSERVATION-NEUTRALITY-FAILURE — INSTRUMENT INVALID \
                     (bootstrap_on={bootstrap_on} tail_on={tail_on})"
                );
                // named, so a future field rename cannot silently weaken the check
                assert_eq!(off_logger.tx_data, on_logger.tx_data);
                assert_eq!(off_logger.tx_retx, on_logger.tx_retx);
                assert_eq!(off_logger.tx_dup, on_logger.tx_dup);
                assert_eq!(off_logger.probes_emitted, on_logger.probes_emitted);
                assert_eq!(off_logger.echoes_generated, on_logger.echoes_generated);
                assert_eq!(off_logger.echoes_arrived, on_logger.echoes_arrived);
                assert_eq!(off_logger.acks_generated, on_logger.acks_generated);
                assert_eq!(off_logger.ack_batches, on_logger.ack_batches);
                assert_eq!(
                    off_logger.channel_bypass_violations,
                    on_logger.channel_bypass_violations
                );
                assert_eq!(off_logger.forward_steps, on_logger.forward_steps);
                assert_eq!(off_logger.reverse_steps, on_logger.reverse_steps);
                assert_eq!(off_logger.wire_digest, on_logger.wire_digest);
                assert_eq!(
                    off_logger.source_prefix_digest,
                    on_logger.source_prefix_digest
                );
                assert_eq!(off_logger.epoch_trigger, on_logger.epoch_trigger);
                assert_eq!(off_logger.outcome_class, on_logger.outcome_class);
                assert_eq!(off_logger.data_emit_ticks, on_logger.data_emit_ticks);
                assert_eq!(off_logger.forward_order, on_logger.forward_order);
                assert_eq!(off_logger.lineage, on_logger.lineage);
                // No echo, packet or event was invented by the instrument.
                assert_eq!(
                    log.count_kind(ChanPacketKind::Echo),
                    on_logger.echoes_generated
                );
                assert_eq!(
                    log.count_kind(ChanPacketKind::Probe),
                    on_logger.probes_emitted
                );
            }
        }
    }

    // ── SUPPORTING PINS ────────────────────────────────────────────────────────────────────────

    /// The logger records EVERY real GE decision and no more: forward records == forward chain
    /// steps, reverse records == reverse chain steps. It therefore consumed no draw of its own.
    #[test]
    fn logger_records_every_ge_decision_exactly_once() {
        for bootstrap_on in [false, true] {
            let mut cfg = real_cfg(bootstrap_on);
            cfg.reverse_model = LossModel::Ge {
                p_ppm: 200_000,
                l: 10,
            };
            let (out, log) = run_panel_logged(&cfg);
            assert_eq!(log.count(ChanDirection::Forward), out.forward_steps);
            assert_eq!(log.count(ChanDirection::Reverse), out.reverse_steps);
            assert_eq!(
                log.count(ChanDirection::Forward),
                out.tx_data + out.tx_retx + out.tx_dup + out.probes_emitted
            );
            assert_eq!(
                log.count(ChanDirection::Reverse),
                out.acks_generated + out.echoes_generated
            );
            assert!(log.dense_ordinal_check().is_ok());
            assert!(log.attempt_ordinal_check().is_ok());
            assert_eq!(authoritative_key_collisions(&log), 0);
        }
    }

    /// CANONICAL LOG ORDER: append order, monotone in `global_ordinal` and non-decreasing in the
    /// diagnostic tick; dense per direction. Reproducible across runs.
    #[test]
    fn canonical_log_order_is_deterministic_and_dense() {
        let cfg = real_cfg(true);
        let (_, a) = run_panel_logged(&cfg);
        let (_, b) = run_panel_logged(&cfg);
        assert_eq!(a.events, b.events, "canonical order must be reproducible");
        assert!(!a.events.is_empty());
        for (i, e) in a.events.iter().enumerate() {
            assert_eq!(e.global_ordinal, i as u64);
        }
        for w in a.events.windows(2) {
            assert!(
                w[0].simulation_tick <= w[1].simulation_tick,
                "canonical order is monotone in virtual time"
            );
        }
        assert!(a.dense_ordinal_check().is_ok());
    }

    /// `actual_fate` comes STRAIGHT from the GE decision — never from an ACK, a terminal outcome or
    /// an emission tick. With no §6 override script every applied fate IS the chain's own result.
    #[test]
    fn actual_fate_comes_straight_from_the_ge_decision() {
        let mut cfg = real_cfg(true);
        cfg.reverse_model = LossModel::Ge {
            p_ppm: 200_000,
            l: 10,
        };
        let (_, log) = run_panel_logged(&cfg);
        assert!(!log.events.is_empty());
        for e in &log.events {
            assert_eq!(e.fate_source, FateSource::GeChainDecision);
            assert_eq!(e.actual_fate, e.ge_decision_fate);
        }
        // With a §6 override script the chain result is still recorded verbatim next to the
        // applied fate, and the override is labelled.
        let mut scripted = real_cfg(true);
        scripted.probe_drop_script = Some(vec![false; 4]);
        let (_, log) = run_panel_logged(&scripted);
        let probes: Vec<&ChannelEvent> = log
            .events
            .iter()
            .filter(|e| e.packet_kind == ChanPacketKind::Probe)
            .collect();
        assert!(!probes.is_empty());
        for p in probes {
            assert_eq!(p.fate_source, FateSource::ScriptOverride);
            assert_eq!(p.actual_fate, ChannelFate::Delivered);
        }
        // every non-probe event is untouched by the probe script
        assert!(log
            .events
            .iter()
            .filter(|e| e.packet_kind != ChanPacketKind::Probe)
            .all(|e| e.fate_source == FateSource::GeChainDecision));
    }

    /// §5.3 invariant sub-result, on real data: `attempt_ordinal == 0` is ALWAYS class `Data`.
    #[test]
    fn attempt_ordinal_zero_is_always_class_data() {
        for bootstrap_on in [false, true] {
            let (_, log) = run_panel_logged(&real_cfg(bootstrap_on));
            let mut checked = 0u64;
            for e in log
                .events
                .iter()
                .filter(|e| e.direction == ChanDirection::Forward && e.attempt_ordinal == Some(0))
            {
                assert_eq!(e.transmission_class, Some(TransmissionClass::Data));
                checked += 1;
            }
            assert!(checked > 0, "must not pass vacuously");
        }
        assert!(ATTEMPT_CLASS_VERDICT.contains("CLASS IS METADATA"));
    }

    /// End-to-end: a REAL OFF/ON pairing is a VALID pairing, its per-direction boundaries are
    /// computed independently, and the treatment's own Probe is what opens the forward boundary.
    #[test]
    fn real_pairing_is_valid_with_independent_directional_boundaries() {
        let (out_off, off) = run_panel_logged(&real_cfg(false));
        let (out_on, on) = run_panel_logged(&real_cfg(true));
        assert!(out_on.probes_emitted > 0, "control must not pass vacuously");
        // SELECTION-BUILD-AXIS-ADDENDUM §3 item 5 / §4: the SHIPPING emitter must never fire in the
        // OFF arm, in ANY configuration. A non-zero value here is a product finding.
        assert_eq!(
            out_off.shipping_probes_emitted, 0,
            "the shipping bootstrap gate (sender.rs:2436) leaked into the control arm"
        );

        let c = compare_channel_fates(&off, &on);
        if legacy_ladder_compiled() {
            // §1: the LEGACY ladder is gated to fire exactly when the bootstrap treatment is
            // DISABLED, so under this build the OFF arm is NOT a no-treatment control. The
            // instrument must SAY so and refuse to produce a fate metric — §4 halt rule — instead
            // of quietly scoring an inadmissible pairing. §5: such results are diagnostic only.
            assert!(
                out_off.probes_emitted > 0,
                "the legacy ladder is compiled but did not fire — fixture no longer exercises §1"
            );
            assert_eq!(
                out_off.probes_emitted - out_off.shipping_probes_emitted,
                out_off.probes_emitted,
                "every OFF-arm probe is attributed LEGACY-COLD-START-LADDER"
            );
            assert!(off.count_kind(ChanPacketKind::Probe) > 0);
            assert_eq!(
                c.verdict.label(),
                "INVALID-BASELINE-CONFIG — ABORT SELECTION"
            );
            assert!(
                c.forward.is_none() && c.reverse.is_none(),
                "an inadmissible pairing must report NO fate metric"
            );
            return;
        }
        // AUTHORITATIVE BUILD (§2: Cargo default features, experimental-cold-start NOT enabled).
        assert_eq!(
            out_off.probes_emitted, 0,
            "the OFF arm is a no-treatment control"
        );
        assert_eq!(off.count_kind(ChanPacketKind::Probe), 0);
        assert!(
            c.verdict.is_valid(),
            "{}\n{}",
            c.verdict.label(),
            c.report_text()
        );
        let f = fwd(&c);
        let r = rev(&c);
        // The forward boundary is exactly the ordinal the treatment's first Probe consumed.
        let first_probe = on
            .events
            .iter()
            .find(|e| e.packet_kind == ChanPacketKind::Probe)
            .expect("treatment emitted a probe");
        assert_eq!(
            f.first_treatment_induced_channel_event,
            Some(first_probe.directional_event_ordinal)
        );
        // Independence: the reverse boundary is computed on its own chain and is NOT assumed to be
        // the forward one.
        assert!(
            r.first_treatment_induced_channel_event.is_none()
                || r.first_treatment_induced_channel_event
                    != f.first_treatment_induced_channel_event
                || r.matched_data_attempts > 0
        );
        // The six §6 metrics exist side by side and are never summed.
        assert!(f.matched_data_attempts > 0);
        assert_eq!(f.non_data_channel_events_off, 0);
        assert_eq!(f.non_data_channel_events_on, out_on.probes_emitted);
        // The pre-boundary prefix is genuinely non-empty — the control is not vacuous.
        assert!(f.pre_boundary_matched > 0);
    }

    /// The derived cap arithmetic itself, so a future constant drift is caught at the source.
    #[test]
    fn derived_cap_arithmetic_matches_the_producer_table() {
        let mut cfg = real_cfg(true);
        cfg.warmup_timeout_ns = 65_000_000_000;
        cfg.tail_on = false;
        let plan = EventCapPlan::derive(&cfg);
        let horizon = cfg.warmup_timeout_ns + DT_NS;
        assert_eq!(
            pc_first_copy_delay_ns(),
            (SEALED + 65) * 1_000_000_000 / 31 * 3 / 2
        );
        assert_eq!(pc_copies_per_unit(horizon), 3, "d0*(2^k-1) <= 65s ⇒ k=3");
        // F1 DATA + F2 ARQ retransmit + F3 cap-exceeded handoff duplicate + F4 RecoverFwd
        // duplicate + F5 PreConverge geometric duplicate (F6 tail probe = 0 here because
        // tail_on = false; F9 legacy ladder = 0 because it is gated to the OFF arm) + F7 probe.
        let expect_fwd = WARMUP_UNITS
            + WARMUP_UNITS * ARQ_RETX_CAP
            + WARMUP_UNITS
            + WARMUP_UNITS * (DUP_FACTOR - 1)
            + WARMUP_UNITS * 3
            + BS_LADDER_STEPS;
        assert_eq!(plan.forward_cap, expect_fwd);
        assert_eq!(plan.probe_cap, BS_LADDER_STEPS);
        assert_eq!(plan.echo_cap, MAX_ECHOES as u64);
        assert_eq!(
            plan.reverse_cap,
            expect_fwd - BS_LADDER_STEPS + MAX_ECHOES as u64
        );
        assert_eq!(plan.total_cap, plan.forward_cap + plan.reverse_cap);

        // tail-ON adds the harness-derived tail-probe bound and nothing else.
        cfg.tail_on = true;
        let plan_tail = EventCapPlan::derive(&cfg);
        let tail = tail_probes_in_horizon(horizon, tail_on_config().min_pto_ns);
        assert_eq!(plan_tail.forward_cap, plan.forward_cap + tail);

        // OFF arm: no shipping probe; the legacy ladder enters ONLY where it is compiled.
        cfg.tail_on = false;
        cfg.bootstrap_on = false;
        let plan_off = EventCapPlan::derive(&cfg);
        let legacy_bound = if legacy_ladder_compiled() {
            CS_LEGACY_LADDER_STEPS
        } else {
            0
        };
        assert_eq!(
            plan_off.forward_cap,
            plan.forward_cap - BS_LADDER_STEPS + legacy_bound
        );
        assert_eq!(plan_off.probe_cap, legacy_bound);
    }
}
