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
        Self { key, model, idx: 0, bad: false }
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
            LossModel::Iid { p_ppm } => fs_cal2_gen::ppm_hit(fs_cal2_gen::draw_u64(&self.key, i), p_ppm),
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
#[derive(Debug, Clone, Default)]
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
    /// Probes emitted by the product sender.
    pub probes_emitted: u64,
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
    /// Tick index of the first accepted echo (`u64::MAX` if none).
    pub first_echo_tick: u64,
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

/// Run one panel flow.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn run_panel(cfg: &PanelCfg) -> PanelOut {
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
                        let drop = fwd.next_drop(); // forward chain, chronological
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
                        let drop = fwd.next_drop(); // SAME forward chain as DATA
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
                        out.probe_wire_bytes += sealed.len() as u64;
                        if out.probe_wire_bytes > SENDER_PROBE_BUDGET {
                            out.sender_budget_violation += 1;
                        }
                        out.lineage
                            .push(format!("t={now} probe_emission seq={probe_seq}"));
                        // SAME forward chain as DATA. The chain is advanced unconditionally so a
                        // scripted override cannot desynchronise the shared channel state.
                        let probe_idx = fwd.steps();
                        let chan_drop = fwd.next_drop();
                        if out.forward_order.len() < 4096 {
                            out.forward_order.push(('B', probe_idx));
                        }
                        let drop = cfg
                            .probe_drop_script
                            .as_ref()
                            .and_then(|v| v.get(probe_gen).copied())
                            .unwrap_or(chan_drop);
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
            if rev.next_drop() {
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
                    out.lineage
                        .push(format!("t={now} authenticated_receiver_arrival seq={probe_seq}"));
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
                                let chan_drop = rev.next_drop();
                                let drop = cfg
                                    .echo_drop_script
                                    .as_ref()
                                    .and_then(|v| v.get(echo_gen).copied())
                                    .unwrap_or(chan_drop);
                                echo_gen += 1;
                                wire_lines.push(format!("E|{seq}|{}", u8::from(drop)));
                                if !drop {
                                    // Reverse path mirrors the ACK model: one OWD, no reverse
                                    // serialization (the DATA path's reverse model is unchanged).
                                    out.lineage.push(format!("t={now} sealed_echo_emission seq={seq}"));
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
    out
}

/// Paired DATA-displacement metric: tick indices where the BASELINE emitted DATA and the TREATMENT
/// did not, restricted to the window before the treatment's first accepted echo. The bootstrap probe
/// is append-only and consumes no pacer token, so this must be 0.
#[must_use]
pub fn data_displacement(baseline: &PanelOut, treatment: &PanelOut) -> u64 {
    let horizon = treatment.first_echo_tick;
    let t: BTreeSet<u64> = treatment
        .data_emit_ticks
        .iter()
        .copied()
        .filter(|&x| x <= horizon)
        .collect();
    baseline
        .data_emit_ticks
        .iter()
        .copied()
        .filter(|&x| x <= horizon && !t.contains(&x))
        .count() as u64
}
