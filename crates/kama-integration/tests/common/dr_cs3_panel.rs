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
//! **SEED/LOSS IDENTITY.** Seed derivation and loss generation are the FS-CAL-2 §5.3 primitives
//! re-used verbatim from `fs_cal2_gen` (`stream_key`, `loss_bits`). The workload, channel, warm-up
//! length, offer pacing, virtual tick and horizon constants are the frozen FS-CAL-2 / R2A identity
//! values, restated here so this module owns no dependency on the spine. The three existing stream
//! classes are untouched; the bootstrap path draws from two ADDITIONAL, disjoint stream keys.
#![allow(dead_code)]

#[path = "fs_cal2_gen.rs"]
pub mod fs_cal2_gen;

use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use fs_cal2_gen::{loss_bits, sha256_hex, stream_key, LossModel, StreamClass};
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

/// Bootstrap stream classes — DISJOINT from the three FS-CAL-2 classes, so the existing
/// InitialSource / Parity / ArqRetransmission streams are bit-for-bit unchanged.
const CLASS_PROBE_FWD: &str = "DRCS3ProbeForward";
const CLASS_ECHO_REV: &str = "DRCS3EchoReverse";

fn bootstrap_key(domain: &str, cell_id: &str, repetition: u32, class: &str) -> String {
    format!("{domain}|{cell_id}|rep{repetition}|{class}")
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
    /// InitialSource loss model.
    pub source_model: LossModel,
    /// ArqRetransmission loss model.
    pub retx_model: LossModel,
    /// Forward bootstrap-probe loss model.
    pub probe_model: LossModel,
    /// Reverse bootstrap-echo loss model.
    pub echo_model: LossModel,
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

    // ── §5.3 deterministic per-class loss streams (existing three classes untouched) ─────────
    let src_key = stream_key(
        &cfg.seed_domain,
        &cfg.cell_id,
        cfg.repetition,
        StreamClass::InitialSource,
    );
    let rtx_key = stream_key(
        &cfg.seed_domain,
        &cfg.cell_id,
        cfg.repetition,
        StreamClass::ArqRetransmission,
    );
    let src_bits = loss_bits(&src_key, cfg.source_model, WARMUP_UNITS as usize);
    let rtx_bits = loss_bits(&rtx_key, cfg.retx_model, 4096);
    // Disjoint bootstrap streams.
    let probe_bits = loss_bits(
        &bootstrap_key(&cfg.seed_domain, &cfg.cell_id, cfg.repetition, CLASS_PROBE_FWD),
        cfg.probe_model,
        64,
    );
    let echo_bits = loss_bits(
        &bootstrap_key(&cfg.seed_domain, &cfg.cell_id, cfg.repetition, CLASS_ECHO_REV),
        cfg.echo_model,
        64,
    );
    let mut rtx_idx = 0usize;

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
                        let drop = src_bits[gid as usize];
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
                        let drop = rtx_bits[rtx_idx.min(rtx_bits.len() - 1)];
                        rtx_idx += 1;
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
                        let drop = cfg
                            .probe_drop_script
                            .as_ref()
                            .and_then(|v| v.get(probe_gen).copied())
                            .unwrap_or_else(|| probe_bits[probe_gen.min(probe_bits.len() - 1)]);
                        probe_gen += 1;
                        wire_lines.push(format!("B|{probe_seq}|{}|{}", d.seq, u8::from(drop)));
                        // The frame is sealed EXACTLY ONCE, at emission, and carried through the
                        // channel verbatim — as a real datagram is.
                        if let Some(arr) = ch.offer(sealed.len() as u64, now, drop) {
                            probe_deliveries.push((arr, sealed));
                        }
                    }
                    _ => {}
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
            acks.push((arr.saturating_add(owd_ns), seq));
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
                                let drop = cfg
                                    .echo_drop_script
                                    .as_ref()
                                    .and_then(|v| v.get(echo_gen).copied())
                                    .unwrap_or_else(|| {
                                        echo_bits[echo_gen.min(echo_bits.len() - 1)]
                                    });
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
