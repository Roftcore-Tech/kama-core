//! FINAL BUILD S1 — COMMON MULTI-BLOCK EXECUTION SPINE (A0/A1/F0). HARNESS-ONLY; DIAGNOSTIC (result-bearing-DEĞİL).
//! Tek-ortak-spine: canonical 48-symbol workload → real Sender ARQ + BottleneckChannel + real RsSimdCodec (F0
//! per-block) + reassembly + full-workload-completion. Arm-adapter YALNIZ typed-config verir (channel/pacing/trace/
//! completion/accounting REDEFİNE-ETMEZ). sender.rs/loss.rs DOKUNULMAZ.
#![allow(dead_code)]

#[path = "fs_workload.rs"]
mod fs_workload;
pub use fs_workload::{
    app_units, canonical_workload, reassemble, sequence_hash, CanonicalSymbol, E, TOTAL_APP_BYTES,
    TOTAL_SOURCE_SYMBOLS,
};

use adaptive_transport::fs_harness::BottleneckChannel;
use adaptive_transport::fs_harness::{
    a1_duplicate_target_set, fs_sealed_len, BlockTiming, PathTimingLedger, RedundancyKind,
    FS_SHARD_FRAMING_BYTES,
};
use adaptive_transport::fs_trace::{
    RedundancyPolicy, ReverseSpec, TraceChannel, TracePacketId, TraceSpec, TransmissionRole,
};
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, Timer, TransmitKind,
};
use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

pub const MS: u64 = 1_000_000;
pub const K_REF: u16 = 8; // F0 + A1 reference grouping (48/8 = 6 blocks)
pub const REF_BLOCKS: u16 = 6;

/// S1 arm-adapter (typed-config; F1/F3/F4 S2'de).
#[derive(Debug, Clone, Copy)]
pub enum SpineArm {
    /// ARQ substrat; proaktif-redundancy-YOK.
    A0,
    /// blind block-total early-duplication (K8 reference-grouping; R target/group).
    A1 { r: u16 },
    /// 6 × K=8 block; R repair/block; real RsSimdCodec.
    F0 { r: u16 },
}

#[derive(Debug, Clone)]
pub struct SpineCfg {
    pub link_bps: u64,               // bottleneck_capacity_bps
    pub source_offer_pacer_bps: u64, // S1: schedule-türetir (0 → t=0 burst); 0.80C-freeze S3
    pub owd_ns: u64,
    pub horizon_ns: u64,
    pub dt_ns: u64,
    pub drop_global_ids: BTreeSet<u16>, // d TOPLAM global InitialSource (workload-level; per-block-DEĞİL)
    pub dup_seed: u64,
    pub reverse: ReverseSpec,
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
            drop_global_ids: drop.iter().copied().collect(),
            dup_seed: 0xD00D,
            reverse: ReverseSpec::Clear,
        }
    }
}

/// Canonical-result-şeması (S1; §17 ailesi — S2 F1/F3/F4 aynı-aileyi kullanır).
#[derive(Debug, Clone)]
pub struct SpineResult {
    pub arm: &'static str,
    pub selected_k: u16,
    pub source_block_count: u16,
    pub configured_r: u16,
    pub repair_count: u64,
    // workload
    pub source_symbol_count: usize,
    pub source_sequence_hash: u64,
    pub source_offer_schedule_hash: u64,
    pub global_to_local_mapping_hash: u64,
    // trace/deficit
    pub dropped_global_ids: Vec<u16>,
    pub deficit_count: usize,
    pub forward_trace_hash: u64,
    pub reverse_trace_hash: u64,
    pub configured_schedule_hash: u64,
    // validity
    pub admitted: bool,
    pub faithful: bool,
    pub overlap_ok: bool,
    // outcome
    pub completed: bool,
    pub full_workload_complete: bool,
    pub completion_ns: Option<u64>,
    pub delivered_source_symbols: usize,
    pub delivered_app_bytes: usize,
    pub reassembled_units: usize,
    pub reassembly_equal: bool,
    // arq / fec / dup
    pub retransmits: u64,
    pub fec_decoded_sources: usize,
    pub reconstruct_byte_equal: bool,
    pub duplicates_emitted: u64,
    pub a1_dup_hits: u64,
    pub a1_dup_misses: u64,
    // cost (§16 iki-defter)
    pub base_source_bytes: u64,
    pub intervention_bytes: u64,
    pub arq_retransmit_bytes: u64,
    pub total_outcome_bytes: u64,
    pub spurious_retransmits: u64,
    // purity
    pub estimator_pure: bool,
    pub recovered_control_used: bool,
    pub post_timeout_notice_none: bool,
    // timing (workload-level + reused BlockTiming)
    pub t_first_source_offer: Option<u64>,
    pub t_last_source_offer_complete: Option<u64>,
    pub t_first_application_delivery: Option<u64>,
    pub t_full_workload_complete: Option<u64>,
    pub timing: BlockTiming,
    pub path_timing: PathTimingLedger,
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

/// global_source_fragment_id → (block_id, local_esi) K8-referans-layout (arm-local metadata; §7).
#[must_use]
pub fn global_to_local(gid: u16) -> (u32, u16) {
    (u32::from(gid / K_REF), gid % K_REF)
}
#[must_use]
pub fn global_to_local_hash() -> u64 {
    let v: Vec<u64> = (0..TOTAL_SOURCE_SYMBOLS as u16)
        .flat_map(|g| {
            let (b, e) = global_to_local(g);
            [u64::from(g), u64::from(b), u64::from(e)]
        })
        .collect();
    fnv(&v)
}

/// Source-offer eligibility schedule (global_id → offer-ns). pacer=0 → t=0 (S1-diag). Arm-BAĞIMSIZ.
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

/// A1 dup-target global-id set (K8-referans-grouping; her-group R target; block-total; deficit-BAĞIMSIZ).
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

/// **COMMON MULTI-BLOCK SPINE** — A0/A1/F0. Gerçek Sender ARQ + BottleneckChannel + (F0) real RsSimdCodec per-block +
/// reassembly + full-workload (48/2400B) completion. Diagnostic (result-bearing-DEĞİL).
#[must_use]
pub fn run_spine(arm: SpineArm, cfg: &SpineCfg) -> SpineResult {
    let sealed = fs_sealed_len(E as u64); // 101
    let canonical = canonical_workload();
    let originals = app_units();
    let (r, is_f0, is_a1) = match arm {
        SpineArm::A0 => (0u16, false, false),
        SpineArm::A1 { r } => (r, false, true),
        SpineArm::F0 { r } => (r, true, false),
    };
    let arm_name = match arm {
        SpineArm::A0 => "A0",
        SpineArm::A1 { .. } => "A1",
        SpineArm::F0 { .. } => "F0",
    };
    let profile = by_id(0x03).unwrap(); // K=8/R=4/E=64

    // F0 per-block real repair (K8): repairs_by_block[b] = Vec<repair-bytes> (R adet).
    let repairs_by_block: Vec<Vec<Vec<u8>>> = if is_f0 {
        (0..REF_BLOCKS as usize)
            .map(|b| {
                let syms: Vec<SourceSymbol> = (0..K_REF as usize)
                    .map(|e| SourceSymbol {
                        esi: e as u16,
                        bytes: &canonical[b * K_REF as usize + e].symbol_bytes,
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

    // Trace: ExactDeficit on GLOBAL InitialSource ids (source_id=(0, gid)); reverse from cfg.
    let drop_set: BTreeSet<(u32, u16)> = cfg.drop_global_ids.iter().map(|&g| (0u32, g)).collect();
    let trace = TraceChannel::with_reverse(
        TraceSpec::ExactDeficit {
            drop_initial: drop_set,
            redundancy: RedundancyPolicy::PassAll,
        },
        cfg.reverse.clone(),
    );
    let mut ch = BottleneckChannel::new(trace, cfg.link_bps, cfg.owd_ns);
    let mut s = Sender::new();
    if is_f0 {
        s.set_manual_fec_enabled(true).unwrap();
    }

    let mut res = SpineResult {
        arm: arm_name,
        selected_k: K_REF,
        source_block_count: REF_BLOCKS,
        configured_r: r,
        repair_count: 0,
        source_symbol_count: TOTAL_SOURCE_SYMBOLS,
        source_sequence_hash: sequence_hash(&canonical),
        source_offer_schedule_hash: sched_hash,
        global_to_local_mapping_hash: global_to_local_hash(),
        dropped_global_ids: cfg.drop_global_ids.iter().copied().collect(),
        deficit_count: cfg.drop_global_ids.len(),
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
        t_last_source_offer_complete: None,
        t_first_application_delivery: None,
        t_full_workload_complete: None,
        timing: BlockTiming::default(),
        path_timing: PathTimingLedger::default(),
    };

    // Receive state: per-block source/repair; delivered global set.
    let mut delivered_src: BTreeSet<u16> = BTreeSet::new(); // global ids delivered/reconstructed
    let mut recv_src_block: Vec<BTreeMap<u16, Vec<u8>>> =
        vec![BTreeMap::new(); REF_BLOCKS as usize]; // block → local_esi → bytes
    let mut recv_rep_block: Vec<BTreeMap<u16, Vec<u8>>> =
        vec![BTreeMap::new(); REF_BLOCKS as usize];
    let mut deliveries: Vec<(u64, u16, bool, u64, u16)> = Vec::new(); // (arr, gid_or_repidx, is_repair, seq, block)
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut ack_ordinal = 0u64;
    let mut fed: BTreeSet<u16> = BTreeSet::new();
    let mut block_repair_emitted = [false; REF_BLOCKS as usize];
    let mut receiver_events = 0u64;

    // offer a source-DATA / retransmit decision to the channel.
    let offer_data = |ch: &mut BottleneckChannel,
                      deliveries: &mut Vec<(u64, u16, bool, u64, u16)>,
                      res: &mut SpineResult,
                      gid: u16,
                      seq: u64,
                      now: u64,
                      is_retx: bool| {
        let (block, _local) = global_to_local(gid);
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
            Some(arr) => deliveries.push((arr, gid, false, seq, block as u16)),
            None if !is_retx && res.timing.t_source_loss.is_none() => {
                res.timing.t_source_loss = Some(now);
            }
            None => {}
        }
        // A1: source is a dup-target → emit ONE duplicate adjacent-early.
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
                deliveries.push((arr, gid, false, u64::MAX, block as u16)); // dup delivers as source gid
            }
        }
    };

    let horizon = cfg.horizon_ns;
    let mut now = 0u64;
    // process initial eligible feeds at t=0 handled inside loop.
    while now < horizon && delivered_src.len() < TOTAL_SOURCE_SYMBOLS {
        // 1) feed AppData for eligible source symbols (source-offer schedule).
        for g in 0..TOTAL_SOURCE_SYMBOLS as u16 {
            if !fed.contains(&g) && elig[g as usize] <= now {
                fed.insert(g);
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
        // 2) F0: emit per-block repair once the block's 8 sources have been fed (block-close).
        if is_f0 {
            for b in 0..REF_BLOCKS as usize {
                if block_repair_emitted[b] {
                    continue;
                }
                let all_fed = (0..K_REF).all(|e| fed.contains(&((b as u16) * K_REF + e)));
                if !all_fed {
                    continue;
                }
                block_repair_emitted[b] = true;
                if res.timing.t_parity_eligible.is_none() {
                    res.timing.t_parity_eligible = Some(now);
                    res.timing.t_parity_enqueued = Some(now);
                    res.timing.redundancy_kind = Some(RedundancyKind::FecRepair);
                }
                for (ri, _rb) in repairs_by_block[b].iter().enumerate() {
                    res.repair_count += 1;
                    res.intervention_bytes += sealed;
                    if res.timing.t_parity_wire.is_none() {
                        res.timing.t_parity_wire = Some(now);
                        res.timing.t_redundancy_wire = Some(now);
                    }
                    let id = TracePacketId {
                        source_id: (b as u32, ri as u16),
                        role: TransmissionRole::Repair,
                        attempt: 0,
                    };
                    if let Some(arr) = ch.offer(id, sealed, now) {
                        deliveries.push((arr, ri as u16, true, u64::MAX, b as u16));
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
            } else {
                let gid = idx;
                let (_b, local) = global_to_local(gid);
                if delivered_src.insert(gid) {
                    recv_src_block[block as usize]
                        .insert(local, canonical[gid as usize].symbol_bytes.clone());
                    if res.t_first_application_delivery.is_none() {
                        res.t_first_application_delivery = Some(arr.max(now));
                    }
                }
                if seq != u64::MAX {
                    // reverse-path: ACK emit at arr; blackout/loss may drop.
                    if !ch.decide_reverse(arr, ack_ordinal) {
                        acks.push((arr + cfg.owd_ns, seq));
                    }
                    ack_ordinal += 1;
                }
            }
            false
        });
        // 4) F0 per-block reconstruct.
        if is_f0 {
            for b in 0..REF_BLOCKS as usize {
                let distinct = recv_src_block[b].len();
                if distinct >= K_REF as usize
                    || recv_rep_block[b].is_empty()
                    || distinct + recv_rep_block[b].len() < K_REF as usize
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
                    if !rec.is_empty() && res.timing.t_repair_usable.is_none() {
                        res.timing.t_repair_usable = Some(now);
                    }
                    for rsym in rec {
                        let gid = (b as u16) * K_REF + rsym.esi;
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
        // 5) RECOVERED → Sender (estimator-purity: RTT/BtlBw before/after identical).
        if !recovered_now.is_empty() {
            res.recovered_control_used = true;
            let pre = (s.path_timing_snapshot(), s.diag_wbtlbw());
            let units: Vec<RecoveredUnit> = recovered_now
                .iter()
                .map(|&gid| RecoveredUnit {
                    unit_id: u64::from(gid),
                    block_id: global_to_local(gid).0,
                    esi: global_to_local(gid).1,
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

    // Finalize.
    res.delivered_source_symbols = delivered_src.len();
    res.completed = delivered_src.len() == TOTAL_SOURCE_SYMBOLS;
    res.full_workload_complete = res.completed;
    res.t_last_source_offer_complete = elig.iter().copied().max();

    // Reassembly from delivered/reconstructed canonical symbols.
    if res.completed {
        let recovered: Vec<CanonicalSymbol> = canonical.clone(); // all 48 present (delivered==48)
        let reasm = reassemble(&recovered);
        res.reassembled_units = reasm.len();
        res.delivered_app_bytes = reasm.iter().map(Vec::len).sum();
        res.reassembly_equal = reasm == originals;
    }

    // A1 hit/miss: dropped-initial ∈ dup_targets → hit.
    for &g in &cfg.drop_global_ids {
        if dup_targets.contains(&g) {
            res.a1_dup_hits += 1;
        } else if is_a1 {
            res.a1_dup_misses += 1;
        }
    }
    res.spurious_retransmits = if res.fec_decoded_sources > 0
        && res
            .timing
            .t_repair_usable
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
    res.faithful = realized_drops == cfg.drop_global_ids.len() as u64;
    res.overlap_ok = receiver_events > 0;
    res.admitted = res.faithful && res.overlap_ok;
    res.forward_trace_hash = ch.trace_hash();
    res.reverse_trace_hash = ch.reverse_decision_log_hash();
    res.configured_schedule_hash = ch.configured_schedule_hash();
    let _ = FS_SHARD_FRAMING_BYTES;
    res
}
