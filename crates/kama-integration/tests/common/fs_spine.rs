//! FINAL BUILD S1+S2 — COMMON MULTI-BLOCK EXECUTION SPINE (A0/A1/F0/F1/F3/F4). HARNESS-ONLY; DIAGNOSTIC.
//! Tek-ortak-spine: canonical 48-symbol workload → real Sender ARQ + BottleneckChannel + real RsSimdCodec
//! (K-parameterized per-block) + reassembly + full-workload-completion. Arm-adapter YALNIZ typed-config
//! (channel/pacing/trace/completion/accounting REDEFİNE-ETMEZ). sender.rs/loss.rs DOKUNULMAZ.
//! PARITY-EMISSION: TÜM-source offered SONRA (sender-wide DATA-first faithful; S0-B) → F1/F3/F4 parity-wire
//! F0'ı GEÇEMEZ (structural-null). t_first_parity_wire = GERÇEK channel wire-emission (arr − owd).
#![allow(dead_code, unused_imports)] // re-export'lar test-binary'lerine göre farklı-alt-küme kullanır
#![allow(clippy::needless_range_loop)] // block-index b çoklu-vec (source/repair/repairs_by_block) indeksler

#[path = "fs_workload.rs"]
mod fs_workload;
pub use fs_workload::{
    app_units, canonical_workload, reassemble, sequence_hash, E, TOTAL_APP_BYTES,
    TOTAL_SOURCE_SYMBOLS,
};

use adaptive_transport::fs_harness::BottleneckChannel;
use adaptive_transport::fs_harness::{
    a1_duplicate_target_set, f4_select_k, fs_effective_source_bps, fs_sealed_len,
    fs_t_serialize_source_ns, BlockTiming, CodecK, PathTimingLedger, RedundancyKind,
};
use adaptive_transport::fs_trace::{
    RedundancyPolicy, ReverseSpec, TraceChannel, TracePacketId, TraceSpec, TransmissionRole,
};
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, PathTimingState, RecoveredInfo, RecoveredUnit, Sender, Timer,
    TransmitKind,
};
use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

pub const MS: u64 = 1_000_000;
pub const K_REF: u16 = 8; // A1 reference grouping (48/8 = 6)
pub const REF_BLOCKS: u16 = 6;

/// S1+S2 arm-adapter (typed-config; tek-spine).
#[derive(Debug, Clone, Copy)]
pub enum SpineArm {
    A0,
    A1 {
        r: u16,
    },
    F0 {
        r: u16,
    },
    /// full-K early-logical-close (K=8; S0-B: parity-wire structural-null vs F0).
    F1 {
        r: u16,
    },
    /// K'=4 partial-block (12 blok; S0-B: parity-wire structural-null vs F0).
    F3 {
        r: u16,
    },
    /// adaptive direct-K (K∈{2,4,8}; f4_select_k(base_rtt)); PathTimingState-consumer.
    F4 {
        r: u16,
    },
}

/// **S3-R2: forward-impairment SUM-TYPE (RULE-9; execution-layer).** Geçersiz-kombinasyon (NaturalIid+deficit)
/// temsil-EDİLEMEZ. NaturalIid = deficit-free surface (S3-A DUR-B → ALT-1). loss_permille SABİT 200 (p=0.20).
#[derive(Debug, Clone)]
pub enum ForwardImpairment {
    ExactDeficit { drop_global_ids: BTreeSet<u16> },
    NaturalIid { loss_permille: u32, seed: u64 },
}

#[derive(Debug, Clone)]
pub struct SpineCfg {
    pub link_bps: u64,
    pub source_offer_pacer_bps: u64, // 0 → t=0 burst (S1-diag); >0 → paced (structural-null fixture)
    pub owd_ns: u64,
    pub horizon_ns: u64,
    pub dt_ns: u64,
    pub forward: ForwardImpairment,
    pub dup_seed: u64,
    pub reverse: ReverseSpec,
    /// F4 PRE-DECISION base-RTT (path-history; ns). 0 → cold (f4→K_COLD=K4). Only F4 consumes.
    pub f4_base_rtt_ns: u64,
}
impl SpineCfg {
    #[must_use]
    pub fn diag(link_bps: u64, owd_ns: u64, drop: &[u16]) -> Self {
        Self {
            link_bps,
            source_offer_pacer_bps: 0,
            owd_ns,
            horizon_ns: 40_000 * MS,
            dt_ns: MS,
            forward: ForwardImpairment::ExactDeficit {
                drop_global_ids: drop.iter().copied().collect(),
            },
            dup_seed: 0xD00D,
            reverse: ReverseSpec::Clear,
            f4_base_rtt_ns: 0,
        }
    }
    /// S3-R2: NaturalIid forward-channel (deficit-free; loss_permille=200 SABİT; seed replicate-ekseni).
    #[must_use]
    pub fn iid(link_bps: u64, owd_ns: u64, loss_permille: u32, seed: u64) -> Self {
        let mut c = Self::diag(link_bps, owd_ns, &[]);
        c.forward = ForwardImpairment::NaturalIid {
            loss_permille,
            seed,
        };
        c
    }
    /// ExactDeficit configured drop-set (NaturalIid → boş; per-workload configured-deficit YOK).
    #[must_use]
    pub fn exact_drop_ids(&self) -> BTreeSet<u16> {
        match &self.forward {
            ForwardImpairment::ExactDeficit { drop_global_ids } => drop_global_ids.clone(),
            ForwardImpairment::NaturalIid { .. } => BTreeSet::new(),
        }
    }
    #[must_use]
    pub fn with_pacer(mut self, bps: u64) -> Self {
        self.source_offer_pacer_bps = bps;
        self
    }
    #[must_use]
    pub fn with_f4_base_rtt(mut self, ns: u64) -> Self {
        self.f4_base_rtt_ns = ns;
        self
    }
    /// S3-R1: reverse-path spesifikasyonu (Clear | Scheduled{blackout,loss}). configured — realized-DEĞİL.
    #[must_use]
    pub fn with_reverse(mut self, reverse: ReverseSpec) -> Self {
        self.reverse = reverse;
        self
    }
    /// S3-R1: virtual-time çözünürlüğü. Paced-fixture'da 0.80C eligibility-time'ları dt-katı olmalı (10µs).
    #[must_use]
    pub fn with_dt(mut self, dt_ns: u64) -> Self {
        self.dt_ns = dt_ns;
        self
    }
    /// S3-R2: horizon (IID-fixture'ında koşu-süresini sınırlar; tüm-48-InitialSource offer'ından SONRA yeterli).
    #[must_use]
    pub fn with_horizon(mut self, ns: u64) -> Self {
        self.horizon_ns = ns;
        self
    }
}

/// F1/F3 structural-null sınıflandırması (parity-wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralClass {
    NotApplicable,
    BulkParityWireNullConfirmed,
    StructuralAnalysisFalsified,
}

#[derive(Debug, Clone)]
pub struct SpineResult {
    pub arm: &'static str,
    pub selected_k: u16,
    pub selected_profile_id: u8,
    pub source_block_count: u16,
    pub configured_r: u16,
    pub repair_count: u64,
    pub source_symbol_count: usize,
    pub source_sequence_hash: u64,
    pub source_offer_schedule_hash: u64,
    pub global_to_local_mapping_hash: u64,
    pub dropped_global_ids: Vec<u16>,
    pub deficit_count: usize,
    pub forward_trace_hash: u64,
    pub reverse_trace_hash: u64,
    pub configured_schedule_hash: u64,
    pub admitted: bool,
    pub faithful: bool,
    pub overlap_ok: bool,
    pub completed: bool,
    pub full_workload_complete: bool,
    pub completion_ns: Option<u64>,
    pub delivered_source_symbols: usize,
    pub delivered_app_bytes: usize,
    pub reassembled_units: usize,
    pub reassembly_equal: bool,
    pub retransmits: u64,
    pub fec_decoded_sources: usize,
    pub reconstruct_byte_equal: bool,
    pub duplicates_emitted: u64,
    pub a1_dup_hits: u64,
    pub a1_dup_misses: u64,
    pub base_source_bytes: u64,
    pub intervention_bytes: u64,
    pub arq_retransmit_bytes: u64,
    pub total_outcome_bytes: u64,
    pub spurious_retransmits: u64,
    pub estimator_pure: bool,
    pub recovered_control_used: bool,
    pub post_timeout_notice_none: bool,
    // workload/parity wire timing (GERÇEK channel wire-emission = arr − owd).
    pub t_first_source_offer: Option<u64>,
    pub t_final_source_wire_complete: Option<u64>,
    pub t_first_parity_eligible: Option<u64>,
    pub t_first_parity_wire: Option<u64>,
    pub t_first_repair_usable: Option<u64>,
    pub t_full_workload_complete: Option<u64>,
    pub timing: BlockTiming,
    pub path_timing: PathTimingLedger,
    // F4 + structural
    pub f4_base_rtt_ns: u64,
    pub structural_class: StructuralClass,
    // ── S3-R1: reverse identity/separation (configured vs realized) ──
    pub forward_schedule_id: u64,
    pub reverse_schedule_id: u64,
    pub reverse_spec_kind: &'static str,
    pub reverse_event_count: u64,
    pub reverse_dropped_count: u64,
    pub reverse_blackout_intervals: Vec<(u64, u64)>,
    // ── S3-R1: source-offer pacing (bottleneck vs source-offer vs realized-wire; §14/§15) ──
    pub bottleneck_capacity_bps: u64,
    pub source_offer_pacer_bps: u64,
    pub configured_source_offer_bps: u64,
    pub source_offer_start_ns: u64,
    pub source_offer_end_ns: u64,
    pub t_first_source_wire_complete: Option<u64>,
    pub realized_source_wire_bps: u64,
    // ── S3-R2: NaturalIid realized InitialSource drops (arm-invariance/pairing/determinism) ──
    pub realized_initial_source_drop_ids: Vec<u16>,
    // ── S3-R3: common result-schema completion (§25; typed None/N-A, sahte-sıfır YOK) ──
    pub impairment_kind: &'static str,
    pub configured_exact_drop_count: Option<u8>,
    pub configured_iid_loss_permille: Option<u16>,
    pub realized_initial_source_losses: usize,
    pub realized_retransmission_losses: u64,
    pub realized_duplicate_losses: u64,
    pub realized_repair_losses: u64,
    pub reverse_event_hash: u64,
    pub pairing_gate_passed: bool,
    pub pairing_failure_reason: Option<&'static str>,
}

fn fnv(vals: &[u64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &v in vals {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// global_source_fragment_id → (block_id, local_esi) for block-K `k` (arm-local layout; §7).
#[must_use]
pub fn global_to_local_k(gid: u16, k: u16) -> (u16, u16) {
    (gid / k, gid % k)
}
#[must_use]
pub fn global_to_local_hash_k(k: u16) -> u64 {
    let v: Vec<u64> = (0..TOTAL_SOURCE_SYMBOLS as u16)
        .flat_map(|g| {
            let (b, e) = global_to_local_k(g, k);
            [u64::from(g), u64::from(b), u64::from(e)]
        })
        .collect();
    fnv(&v)
}

#[must_use]
pub fn profile_for_k(k: u16) -> u8 {
    match k {
        2 => 0x01,
        4 => 0x02,
        _ => 0x03, // 8
    }
}

#[must_use]
pub fn source_offer_schedule(cfg: &SpineCfg, sealed: u64) -> Vec<u64> {
    (0..TOTAL_SOURCE_SYMBOLS as u64)
        .map(|g| {
            if cfg.source_offer_pacer_bps == 0 {
                0
            } else {
                g.saturating_mul(sealed)
                    .saturating_mul(8)
                    .saturating_mul(1_000_000_000)
                    / cfg.source_offer_pacer_bps.max(1)
            }
        })
        .collect()
}

#[must_use]
pub fn a1_targets(r: u16, dup_seed: u64) -> BTreeSet<u16> {
    let mut out = BTreeSet::new();
    for group in 0..REF_BLOCKS {
        for local in a1_duplicate_target_set(u32::from(group), dup_seed, r, K_REF) {
            out.insert(group * K_REF + local);
        }
    }
    out
}

/// F4 pre-decision K selection (PathTimingState'ten; yalnız F4 tüketir; no-future/outcome).
#[must_use]
pub fn f4_decide_k(base_rtt_ns: u64) -> CodecK {
    let pt = PathTimingState {
        srtt_ns: if base_rtt_ns > 0 {
            Some(base_rtt_ns)
        } else {
            None
        },
        min_rtt_ns: if base_rtt_ns > 0 {
            Some(base_rtt_ns)
        } else {
            None
        },
        rttvar_ns: 0,
        queue_delay_ns: 0,
        sample_present: base_rtt_ns > 0,
        rtprop_fresh: base_rtt_ns > 0,
    };
    f4_select_k(&pt)
}

/// **COMMON MULTI-BLOCK SPINE.** Gerçek Sender ARQ + BottleneckChannel + (FEC) real RsSimdCodec per-block +
/// reassembly + full-workload completion. Parity TÜM-source-SONRASI emit (sender-wide-gate faithful).
#[must_use]
pub fn run_spine(arm: SpineArm, cfg: &SpineCfg) -> SpineResult {
    let sealed = fs_sealed_len(E as u64); // 101
    let canonical = canonical_workload();
    let originals = app_units();

    // Arm geometry.
    let (arm_name, r, is_a1, is_fec, is_f1, is_f3, is_f4) = match arm {
        SpineArm::A0 => ("A0", 0, false, false, false, false, false),
        SpineArm::A1 { r } => ("A1", r, true, false, false, false, false),
        SpineArm::F0 { r } => ("F0", r, false, true, false, false, false),
        SpineArm::F1 { r } => ("F1", r, false, true, true, false, false),
        SpineArm::F3 { r } => ("F3", r, false, true, false, true, false),
        SpineArm::F4 { r } => ("F4", r, false, true, false, false, true),
    };
    let selected_codec_k = if is_f4 {
        Some(f4_decide_k(cfg.f4_base_rtt_ns))
    } else {
        None
    };
    let k: u16 = match arm {
        SpineArm::F3 { .. } => 4,
        SpineArm::F4 { .. } => selected_codec_k.unwrap().k(),
        _ => 8, // A0/A1 reference, F0/F1
    };
    let nblocks = (TOTAL_SOURCE_SYMBOLS as u16) / k;
    let profile = by_id(profile_for_k(k)).unwrap();

    // Per-block real repair (FEC arms).
    let repairs_by_block: Vec<Vec<Vec<u8>>> = if is_fec {
        (0..nblocks as usize)
            .map(|b| {
                let syms: Vec<SourceSymbol> = (0..k as usize)
                    .map(|e| SourceSymbol {
                        esi: e as u16,
                        bytes: &canonical[b * k as usize + e].symbol_bytes,
                    })
                    .collect();
                RsSimdCodec
                    .encode(&profile, &syms)
                    .unwrap()
                    .into_iter()
                    .take(r as usize)
                    .map(|rp| rp.bytes)
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };

    let dup_targets = if is_a1 {
        a1_targets(r, cfg.dup_seed)
    } else {
        BTreeSet::new()
    };
    let elig = source_offer_schedule(cfg, sealed);
    let sched_hash = fnv(&elig);

    // S3-R2: forward-channel = ExactDeficit (drop_initial) VEYA NaturalIid (deficit-free hash-drop).
    let forward_spec = match &cfg.forward {
        ForwardImpairment::ExactDeficit { drop_global_ids } => TraceSpec::ExactDeficit {
            drop_initial: drop_global_ids.iter().map(|&g| (0u32, g)).collect(),
            redundancy: RedundancyPolicy::PassAll,
        },
        ForwardImpairment::NaturalIid {
            loss_permille,
            seed,
        } => TraceSpec::NaturalIid {
            loss_permille: *loss_permille,
            seed: *seed,
        },
    };
    let trace = TraceChannel::with_reverse(forward_spec, cfg.reverse.clone());
    let mut ch = BottleneckChannel::new(trace, cfg.link_bps, cfg.owd_ns);
    let mut s = Sender::new();
    if is_fec {
        s.set_manual_fec_enabled(true).unwrap();
    }

    let mut res = SpineResult {
        arm: arm_name,
        selected_k: k,
        selected_profile_id: profile_for_k(k),
        source_block_count: nblocks,
        configured_r: r,
        repair_count: 0,
        source_symbol_count: TOTAL_SOURCE_SYMBOLS,
        source_sequence_hash: sequence_hash(&canonical),
        source_offer_schedule_hash: sched_hash,
        global_to_local_mapping_hash: global_to_local_hash_k(k),
        dropped_global_ids: cfg.exact_drop_ids().iter().copied().collect(),
        deficit_count: cfg.exact_drop_ids().len(),
        forward_trace_hash: 0,
        reverse_trace_hash: 0,
        configured_schedule_hash: 0,
        admitted: false,
        faithful: false,
        overlap_ok: false,
        completed: false,
        full_workload_complete: false,
        completion_ns: None,
        delivered_source_symbols: 0,
        delivered_app_bytes: 0,
        reassembled_units: 0,
        reassembly_equal: false,
        retransmits: 0,
        fec_decoded_sources: 0,
        reconstruct_byte_equal: true,
        duplicates_emitted: 0,
        a1_dup_hits: 0,
        a1_dup_misses: 0,
        base_source_bytes: 0,
        intervention_bytes: 0,
        arq_retransmit_bytes: 0,
        total_outcome_bytes: 0,
        spurious_retransmits: 0,
        estimator_pure: true,
        recovered_control_used: false,
        post_timeout_notice_none: true,
        t_first_source_offer: None,
        t_final_source_wire_complete: None,
        t_first_parity_eligible: None,
        t_first_parity_wire: None,
        t_first_repair_usable: None,
        t_full_workload_complete: None,
        timing: BlockTiming::default(),
        path_timing: PathTimingLedger::default(),
        f4_base_rtt_ns: cfg.f4_base_rtt_ns,
        structural_class: StructuralClass::NotApplicable,
        forward_schedule_id: 0,
        reverse_schedule_id: 0,
        reverse_spec_kind: "Clear",
        reverse_event_count: 0,
        reverse_dropped_count: 0,
        reverse_blackout_intervals: Vec::new(),
        bottleneck_capacity_bps: 0,
        source_offer_pacer_bps: 0,
        configured_source_offer_bps: 0,
        source_offer_start_ns: 0,
        source_offer_end_ns: 0,
        t_first_source_wire_complete: None,
        realized_source_wire_bps: 0,
        realized_initial_source_drop_ids: Vec::new(),
        impairment_kind: "ExactDeficit",
        configured_exact_drop_count: None,
        configured_iid_loss_permille: None,
        realized_initial_source_losses: 0,
        realized_retransmission_losses: 0,
        realized_duplicate_losses: 0,
        realized_repair_losses: 0,
        reverse_event_hash: 0,
        pairing_gate_passed: false,
        pairing_failure_reason: None,
    };

    let mut delivered_src: BTreeSet<u16> = BTreeSet::new();
    let mut recv_src_block: Vec<BTreeMap<u16, Vec<u8>>> = vec![BTreeMap::new(); nblocks as usize];
    let mut recv_rep_block: Vec<BTreeMap<u16, Vec<u8>>> = vec![BTreeMap::new(); nblocks as usize];
    // deliveries: (arr, gid_or_repidx, is_repair, seq, block)
    let mut deliveries: Vec<(u64, u16, bool, u64, u16)> = Vec::new();
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut ack_ordinal = 0u64;
    let mut fed: BTreeSet<u16> = BTreeSet::new();
    let mut parity_emitted_flag = false;
    let mut receiver_events = 0u64;

    let owd = cfg.owd_ns;
    let offer_data = |ch: &mut BottleneckChannel,
                      deliveries: &mut Vec<(u64, u16, bool, u64, u16)>,
                      res: &mut SpineResult,
                      gid: u16,
                      seq: u64,
                      now: u64,
                      is_retx: bool| {
        let (block, _local) = global_to_local_k(gid, k);
        let role = if is_retx {
            TransmissionRole::RetransmitSource
        } else {
            TransmissionRole::InitialSource
        };
        let id = TracePacketId {
            source_id: (0, gid),
            role,
            attempt: u16::from(is_retx),
        };
        if is_retx {
            res.retransmits += 1;
            res.arq_retransmit_bytes += sealed;
            if res.timing.t_arq_eligible.is_none() {
                res.timing.t_arq_eligible = Some(now);
                res.timing.t_arq_wire = Some(now);
            }
        } else {
            res.base_source_bytes += sealed;
            if res.t_first_source_offer.is_none() {
                res.t_first_source_offer = Some(now);
            }
        }
        match ch.offer(id, sealed, now) {
            Some(arr) => {
                if !is_retx {
                    // source wire-emission = channel serialization completion (arr − owd).
                    let wire = arr.saturating_sub(owd);
                    if wire > res.t_final_source_wire_complete.unwrap_or(0) {
                        res.t_final_source_wire_complete = Some(wire);
                    }
                    // S3-R1: first source wire (realized-pacing spanı için min-wire).
                    if res.t_first_source_wire_complete.is_none_or(|w| wire < w) {
                        res.t_first_source_wire_complete = Some(wire);
                    }
                }
                deliveries.push((arr, gid, false, seq, block));
            }
            None => {
                if is_retx {
                    // S3-R3: realized retransmission-loss (IID → retransmit de düşebilir; role-admissibility).
                    res.realized_retransmission_losses += 1;
                } else {
                    // S3-R2: realized InitialSource-drop (ExactDeficit → configured; NaturalIid → hash-drop).
                    res.realized_initial_source_drop_ids.push(gid);
                    if res.timing.t_source_loss.is_none() {
                        res.timing.t_source_loss = Some(now);
                    }
                }
            }
        }
        if is_a1 && !is_retx && dup_targets.contains(&gid) {
            let did = TracePacketId {
                source_id: (0, gid),
                role: TransmissionRole::Duplicate,
                attempt: 1,
            };
            res.duplicates_emitted += 1;
            res.intervention_bytes += sealed;
            if res.timing.t_redundancy_wire.is_none() {
                res.timing.redundancy_kind = Some(RedundancyKind::Duplicate);
                res.timing.t_redundancy_eligible = Some(now);
                res.timing.t_redundancy_enqueued = Some(now);
                res.timing.t_redundancy_wire = Some(now);
            }
            if let Some(arr) = ch.offer(did, sealed, now) {
                deliveries.push((arr, gid, false, u64::MAX, block));
            } else {
                // S3-R3: realized duplicate-loss (IID → dup da düşebilir; garantili-teslim YOK).
                res.realized_duplicate_losses += 1;
            }
        }
    };

    let horizon = cfg.horizon_ns;
    let mut now = 0u64;
    while now < horizon && delivered_src.len() < TOTAL_SOURCE_SYMBOLS {
        // 1) feed AppData for eligible source symbols.
        for g in 0..TOTAL_SOURCE_SYMBOLS as u16 {
            if !fed.contains(&g) && elig[g as usize] <= now {
                fed.insert(g);
                // F1 early-logical-close: parity ELIGIBLE as soon as block sources ENQUEUED.
                if is_fec && res.t_first_parity_eligible.is_none() {
                    let block_of = global_to_local_k(g, k).0;
                    let block_fully_fed = (0..k).all(|e| fed.contains(&(block_of * k + e)));
                    if (is_f1 || is_f3 || is_f4 || block_of == 0) && block_fully_fed {
                        res.t_first_parity_eligible = Some(now);
                        res.timing.t_parity_eligible = Some(now);
                    }
                }
                let d = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id: u64::from(g),
                        size: sealed,
                    }),
                    now,
                );
                for tx in d {
                    match tx.kind {
                        TransmitKind::Data => offer_data(
                            &mut ch,
                            &mut deliveries,
                            &mut res,
                            tx.unit_id as u16,
                            tx.seq,
                            now,
                            false,
                        ),
                        TransmitKind::Retransmit => offer_data(
                            &mut ch,
                            &mut deliveries,
                            &mut res,
                            tx.unit_id as u16,
                            tx.seq,
                            now,
                            true,
                        ),
                        _ => {}
                    }
                }
            }
        }
        // 2) PARITY EMISSION — AFTER all 48 source offered (sender-wide DATA-first faithful; S0-B).
        //    F0/F1/F3/F4 uniform → parity-wire F0'ı GEÇEMEZ (structural-null). block-close eligibility ise
        //    F1/F3 için ERKEN (yukarıda) — AMA wire burada, tüm-source-sonrası.
        // GATE: parity YALNIZ TÜM-48-source CHANNEL'a OFFER edildikten sonra (base_source_bytes==48×sealed).
        // (fed.len()==48 YETMEZ: Sender cwnd ile source'u kademeli emit-eder; sender-wide-DATA-first faithful.)
        if is_fec
            && !parity_emitted_flag
            && res.base_source_bytes >= (TOTAL_SOURCE_SYMBOLS as u64) * sealed
        {
            parity_emitted_flag = true;
            res.timing.redundancy_kind = Some(RedundancyKind::FecRepair);
            if res.t_first_parity_eligible.is_none() {
                res.t_first_parity_eligible = Some(now);
                res.timing.t_parity_eligible = Some(now);
            }
            res.timing.t_parity_enqueued = Some(now);
            for b in 0..nblocks as usize {
                for ri in 0..repairs_by_block[b].len() {
                    res.repair_count += 1;
                    res.intervention_bytes += sealed;
                    let id = TracePacketId {
                        source_id: (b as u32, ri as u16),
                        role: TransmissionRole::Repair,
                        attempt: 0,
                    };
                    if let Some(arr) = ch.offer(id, sealed, now) {
                        // parity wire-emission captured at OFFER (loop may exit on source-completion before arrival).
                        let wire = arr.saturating_sub(owd);
                        if res.t_first_parity_wire.is_none_or(|w| wire < w) {
                            res.t_first_parity_wire = Some(wire);
                            res.timing.t_parity_wire = Some(wire);
                        }
                        deliveries.push((arr, ri as u16, true, u64::MAX, b as u16));
                    } else {
                        // S3-R3: realized repair-loss (IID → parity da düşebilir; garantili-teslim YOK).
                        res.realized_repair_losses += 1;
                    }
                }
            }
        }
        now += cfg.dt_ns;
        // 3) deliveries due.
        let mut recovered_now: Vec<u16> = Vec::new();
        deliveries.retain(|&(arr, idx, is_repair, seq, block)| {
            if arr > now {
                return true;
            }
            receiver_events += 1;
            if is_repair {
                recv_rep_block[block as usize]
                    .entry(idx)
                    .or_insert_with(|| repairs_by_block[block as usize][idx as usize].clone());
                if res.timing.t_parity_arrival.is_none() {
                    res.timing.t_parity_arrival = Some(arr);
                }
            } else {
                let gid = idx;
                let (_b, local) = global_to_local_k(gid, k);
                if delivered_src.insert(gid) {
                    recv_src_block[block as usize]
                        .insert(local, canonical[gid as usize].symbol_bytes.clone());
                }
                if seq != u64::MAX {
                    if !ch.decide_reverse(arr, ack_ordinal) {
                        acks.push((arr + owd, seq));
                    }
                    ack_ordinal += 1;
                }
            }
            false
        });
        // 4) per-block reconstruct (FEC arms).
        if is_fec {
            for b in 0..nblocks as usize {
                let distinct = recv_src_block[b].len();
                if distinct >= k as usize
                    || recv_rep_block[b].is_empty()
                    || distinct + recv_rep_block[b].len() < k as usize
                {
                    continue;
                }
                let mut received: Vec<ReceivedSymbol> = Vec::new();
                for (e, by) in &recv_src_block[b] {
                    received.push(ReceivedSymbol {
                        esi: *e,
                        is_repair: false,
                        bytes: by,
                    });
                }
                for (e, by) in &recv_rep_block[b] {
                    received.push(ReceivedSymbol {
                        esi: *e,
                        is_repair: true,
                        bytes: by,
                    });
                }
                if let Ok(rec) = RsSimdCodec.reconstruct(&profile, &received, 0) {
                    if !rec.is_empty() && res.t_first_repair_usable.is_none() {
                        res.t_first_repair_usable = Some(now);
                        res.timing.t_repair_usable = Some(now);
                    }
                    for rsym in rec {
                        let gid = (b as u16) * k + rsym.esi;
                        if rsym.bytes != canonical[gid as usize].symbol_bytes {
                            res.reconstruct_byte_equal = false;
                        }
                        if delivered_src.insert(gid) {
                            recv_src_block[b].insert(rsym.esi, rsym.bytes.clone());
                            res.fec_decoded_sources += 1;
                            recovered_now.push(gid);
                        }
                    }
                }
            }
        }
        // 5) RECOVERED → Sender (estimator-purity).
        if !recovered_now.is_empty() {
            res.recovered_control_used = true;
            let pre = (s.path_timing_snapshot(), s.diag_wbtlbw());
            let units: Vec<RecoveredUnit> = recovered_now
                .iter()
                .map(|&gid| {
                    let (b, e) = global_to_local_k(gid, k);
                    RecoveredUnit {
                        unit_id: u64::from(gid),
                        block_id: u32::from(b),
                        esi: e,
                    }
                })
                .collect();
            let _ = s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
            let post = (s.path_timing_snapshot(), s.diag_wbtlbw());
            if pre != post {
                res.estimator_pure = false;
            }
        }
        // 6) ACKs due → Sender → new tx.
        let due: Vec<u64> = {
            let mut v = Vec::new();
            acks.retain(|&(t, seq)| {
                if t <= now && seq != u64::MAX {
                    v.push(seq);
                    false
                } else {
                    t > now
                }
            });
            v
        };
        let mut emit_after = Vec::new();
        if !due.is_empty() {
            emit_after.extend(s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: due,
                    ack_delay: Duration::from_nanos(0),
                }),
                now,
            ));
        }
        emit_after.extend(s.on_event(Event::TimerFired(Timer::Pto), now));
        for tx in emit_after {
            match tx.kind {
                TransmitKind::Data => offer_data(
                    &mut ch,
                    &mut deliveries,
                    &mut res,
                    tx.unit_id as u16,
                    tx.seq,
                    now,
                    false,
                ),
                TransmitKind::Retransmit => offer_data(
                    &mut ch,
                    &mut deliveries,
                    &mut res,
                    tx.unit_id as u16,
                    tx.seq,
                    now,
                    true,
                ),
                _ => {}
            }
        }
        if delivered_src.len() == TOTAL_SOURCE_SYMBOLS && res.completion_ns.is_none() {
            res.completion_ns = Some(now);
            res.timing.t_block_complete = Some(now);
            res.t_full_workload_complete = Some(now);
        }
    }

    res.delivered_source_symbols = delivered_src.len();
    res.completed = delivered_src.len() == TOTAL_SOURCE_SYMBOLS;
    res.full_workload_complete = res.completed;
    if res.completed {
        let reasm = reassemble(&canonical);
        res.reassembled_units = reasm.len();
        res.delivered_app_bytes = reasm.iter().map(Vec::len).sum();
        res.reassembly_equal = reasm == originals;
    }
    for &g in &cfg.exact_drop_ids() {
        if dup_targets.contains(&g) {
            res.a1_dup_hits += 1;
        } else if is_a1 {
            res.a1_dup_misses += 1;
        }
    }
    res.spurious_retransmits = if res.fec_decoded_sources > 0
        && res
            .t_first_repair_usable
            .zip(res.timing.t_arq_arrival)
            .is_none_or(|(u, a)| u <= a)
    {
        res.retransmits
    } else {
        0
    };
    res.total_outcome_bytes =
        res.base_source_bytes + res.intervention_bytes + res.arq_retransmit_bytes;
    res.post_timeout_notice_none = s.take_post_timeout_notice().is_none();
    let pt = s.path_timing_snapshot();
    res.path_timing = PathTimingLedger {
        path_srtt_ns: pt.srtt_ns,
        path_min_rtt_ns: pt.min_rtt_ns,
        path_rttvar_ns: pt.rttvar_ns,
        path_queue_delay_ns: pt.queue_delay_ns,
        path_sample_present: pt.sample_present,
        path_rtprop_fresh: pt.rtprop_fresh,
    };
    let (_, realized_drops) = ch.trace_counts();
    res.realized_initial_source_drop_ids.sort_unstable();
    res.faithful = match &cfg.forward {
        // ExactDeficit: realized total-drop == configured |D| (redundancy PassAll → yalnız InitialSource düşer).
        ForwardImpairment::ExactDeficit { drop_global_ids } => {
            realized_drops == drop_global_ids.len() as u64
        }
        // NaturalIid: configured-deficit YOK → harness-valid (determinism/pairing AYRI pinlerde kanıtlanır).
        ForwardImpairment::NaturalIid { .. } => true,
    };
    res.overlap_ok = receiver_events > 0;
    res.admitted = res.faithful && res.overlap_ok;
    // S3-R3: common result-schema completion (§25). configured = variant-local (typed None/N-A).
    match &cfg.forward {
        ForwardImpairment::ExactDeficit { drop_global_ids } => {
            res.impairment_kind = "ExactDeficit";
            res.configured_exact_drop_count = u8::try_from(drop_global_ids.len()).ok();
            res.configured_iid_loss_permille = None;
        }
        ForwardImpairment::NaturalIid { loss_permille, .. } => {
            res.impairment_kind = "NaturalIid";
            res.configured_exact_drop_count = None;
            res.configured_iid_loss_permille = u16::try_from(*loss_permille).ok();
        }
    }
    res.realized_initial_source_losses = res.realized_initial_source_drop_ids.len();
    res.pairing_gate_passed = res.admitted;
    res.pairing_failure_reason = if res.admitted {
        None
    } else {
        Some("not-admitted")
    };
    res.forward_trace_hash = ch.trace_hash();
    res.reverse_trace_hash = ch.reverse_decision_log_hash();
    // S3-R3 schema: reverse_event_hash = realized reverse decision-log (reverse_trace_hash ile AYNI; §25-alias).
    res.reverse_event_hash = ch.reverse_decision_log_hash();
    res.configured_schedule_hash = ch.configured_schedule_hash();
    // S3-R1: reverse identity (configured, forward'dan AYRI) + realized reverse events.
    res.forward_schedule_id = ch.forward_schedule_id();
    res.reverse_schedule_id = ch.reverse_schedule_id();
    let (rev_events, rev_dropped) = ch.reverse_counts();
    res.reverse_event_count = rev_events;
    res.reverse_dropped_count = rev_dropped;
    let (rev_kind, rev_blackout) = match &cfg.reverse {
        ReverseSpec::Clear => ("Clear", Vec::new()),
        ReverseSpec::Scheduled {
            blackout_intervals_ns,
            ..
        } => ("Scheduled", blackout_intervals_ns.clone()),
    };
    res.reverse_spec_kind = rev_kind;
    res.reverse_blackout_intervals = rev_blackout;
    // S3-R1: source-offer pacing — bottleneck vs configured-offer vs realized-wire (§14/§15).
    res.bottleneck_capacity_bps = cfg.link_bps;
    res.source_offer_pacer_bps = cfg.source_offer_pacer_bps;
    if cfg.source_offer_pacer_bps > 0 {
        let eff = fs_effective_source_bps(cfg.source_offer_pacer_bps, cfg.link_bps);
        res.configured_source_offer_bps = eff; // min(pacer, bottleneck) = 0.80C
        res.source_offer_start_ns = 0;
        res.source_offer_end_ns =
            fs_t_serialize_source_ns(TOTAL_SOURCE_SYMBOLS as u64 * sealed, eff);
    }
    // realized source wire rate: (N-1) inter-departure spanı (first-wire serialize-offset'i düşürür).
    if let (Some(first), Some(last)) = (
        res.t_first_source_wire_complete,
        res.t_final_source_wire_complete,
    ) {
        if last > first {
            res.realized_source_wire_bps = (TOTAL_SOURCE_SYMBOLS as u64 - 1)
                .saturating_mul(sealed)
                .saturating_mul(8)
                .saturating_mul(1_000_000_000)
                / (last - first);
        }
    }
    // F1/F3 structural-null (per-run): parity-wire, final-source-wire'DAN ÖNCE OLAMAZ (sender-wide-gate).
    // Paired F1/F3-vs-F0 t_first_parity_wire eşitliği testte (deterministik). Advancement → FALSIFIED (DUR-A).
    if is_f1 || is_f3 {
        res.structural_class = match (res.t_first_parity_wire, res.t_final_source_wire_complete) {
            (Some(pw), Some(sw)) if pw >= sw => StructuralClass::BulkParityWireNullConfirmed,
            (Some(_), Some(_)) => StructuralClass::StructuralAnalysisFalsified,
            _ => StructuralClass::NotApplicable,
        };
    }
    res
}
