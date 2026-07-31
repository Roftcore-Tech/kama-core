//! DR-FEC-FS-1 FS-HARNESS-A — F0 closed-loop (SHARED; test + fs_runner CLI #[path]-include eder).
//! Gerçek Sender FEC-authority + GERÇEK RsSimdCodec encode/decode + RECOVERED + 9-timestamp + instrument.
#![allow(dead_code)] // CLI ve test farklı-alt-kümeler kullanır.

use adaptive_transport::fs_harness::BottleneckChannel;
use adaptive_transport::fs_harness::{
    BlockTiming, PathTimingLedger, PerRunInstrument, RedundancyKind,
};
use adaptive_transport::fs_trace::{
    RedundancyPolicy, TraceChannel, TracePacketId, TraceSpec, TransmissionRole,
};
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, Timer, TransmitKind,
};
use kama_fec_codec::{by_id, FecCodec, FecProfile, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

pub const K: u16 = 8;
pub const E: usize = 64;
pub const MS: u64 = 1_000_000;

pub fn profile() -> FecProfile {
    by_id(0x03).unwrap() // K=8, R=4, E=64
}
pub fn source_bytes() -> Vec<Vec<u8>> {
    (0..K as usize)
        .map(|i| {
            (0..E)
                .map(|j| (i as u8).wrapping_mul(31).wrapping_add(j as u8))
                .collect()
        })
        .collect()
}
/// GERÇEK codec ENCODE → repair-bytes (pin-3: synthetic-DEĞİL).
pub fn real_repairs(src: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let p = profile();
    let syms: Vec<SourceSymbol> = src
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    RsSimdCodec
        .encode(&p, &syms)
        .unwrap()
        .into_iter()
        .map(|r| r.bytes)
        .collect()
}

pub struct F0Result {
    pub delivered: usize,
    pub completion_ns: Option<u64>,
    pub fec_decoded_sources: usize,
    pub retransmits: u64,
    pub parity_emitted: u64,
    pub post_timeout_notice_none: bool,
    pub reconstruct_byte_equal: bool,
    pub serialized_bytes: u64,
    /// RECOVERED estimator-purity: RTT/BtlBw sample RECOVERED-öncesi/sonrası ÖZDEŞ (fake-ACK-YOK kanıtı).
    pub estimator_pure: bool,
    pub timing: BlockTiming,
    pub path_timing: PathTimingLedger,
    pub instrument: PerRunInstrument,
    pub redundancy_payload_bytes: u64,
    pub redundancy_framing_bytes: u64,
    pub realized_sealed_wire_bytes: u64,
}

/// F0 kapalı-döngü. `parity_wire_frame` = parity paketinin sealed-wire boyu (payload E + framing).
pub fn run_f0(
    manual_r: u16,
    drop_initial: &[u16],
    link_bps: u64,
    owd_ns: u64,
    parity_frame: u64,
) -> F0Result {
    let src = source_bytes();
    let repairs = real_repairs(&src);
    let p = profile();
    let drop_set: BTreeSet<(u32, u16)> = drop_initial.iter().map(|&e| (0u32, e)).collect();
    let trace = TraceChannel::new(TraceSpec::ExactDeficit {
        drop_initial: drop_set,
        redundancy: RedundancyPolicy::PassAll,
    });
    let mut ch = BottleneckChannel::new(trace, link_bps, owd_ns);
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap(); // F0 PURITY: Manual fixed-R

    let mut delivered_src: BTreeSet<u16> = BTreeSet::new();
    let mut recv_src: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut recv_rep: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut deliveries: Vec<(u64, u16, bool, u64)> = Vec::new(); // (arrival, esi, is_repair, seq)
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut receiver_events = 0u64;
    let mut timing = BlockTiming {
        redundancy_kind: Some(RedundancyKind::FecRepair),
        ..BlockTiming::default()
    };
    let mut res = F0Result {
        delivered: 0,
        completion_ns: None,
        fec_decoded_sources: 0,
        retransmits: 0,
        parity_emitted: 0,
        post_timeout_notice_none: true,
        reconstruct_byte_equal: true,
        serialized_bytes: 0,
        estimator_pure: true,
        timing: BlockTiming::default(),
        path_timing: PathTimingLedger::default(),
        instrument: PerRunInstrument::default(),
        redundancy_payload_bytes: 0,
        redundancy_framing_bytes: 0,
        realized_sealed_wire_bytes: 0,
    };

    // t=0: K source DATA (esi=unit_id).
    for u in 0..K as u64 {
        let d = s.on_event(
            Event::AppData(OpaqueUnit {
                id: u,
                size: E as u64,
            }),
            0,
        );
        for tx in d {
            let (role, is_retx) = match tx.kind {
                TransmitKind::Data => (TransmissionRole::InitialSource, false),
                TransmitKind::Retransmit => {
                    res.retransmits += 1;
                    (TransmissionRole::RetransmitSource, true)
                }
                _ => continue,
            };
            let esi = tx.unit_id as u16;
            let id = TracePacketId {
                source_id: (0, esi),
                role,
                attempt: u16::from(is_retx),
            };
            match ch.offer(id, E as u64, 0) {
                Some(arr) => deliveries.push((arr, esi, false, tx.seq)),
                None if timing.t_source_loss.is_none() => timing.t_source_loss = Some(0),
                None => {}
            }
        }
    }
    // Block-close authority (Manual fixed-R) → parity (gerçek repair-bytes).
    timing.t_parity_eligible = Some(0);
    timing.t_parity_enqueued = Some(0);
    let auth = s.open_fec_block_authority();
    let authorized = s
        .close_fec_block_manual(auth, 0, manual_r, E as u64)
        .unwrap();
    res.parity_emitted = u64::from(authorized);
    timing.record_redundancy(RedundancyKind::FecRepair, 0, 0, 0, None);
    for esi in 0..authorized {
        let id = TracePacketId {
            source_id: (0, esi),
            role: TransmissionRole::Repair,
            attempt: 0,
        };
        // Accounting (aynı-domain): payload E + framing (parity_frame − E).
        res.realized_sealed_wire_bytes += parity_frame;
        res.redundancy_payload_bytes += E as u64;
        res.redundancy_framing_bytes += parity_frame.saturating_sub(E as u64);
        if let Some(arr) = ch.offer(id, parity_frame, 0) {
            deliveries.push((arr, esi, true, u64::MAX));
        }
    }

    let mut now = 0u64;
    let horizon = 20_000 * MS;
    while now < horizon && delivered_src.len() < K as usize {
        now += MS;
        let mut recovered_now: Vec<u16> = Vec::new();
        deliveries.retain(|&(arr, esi, is_repair, seq)| {
            if arr <= now {
                receiver_events += 1;
                if is_repair {
                    if timing.t_parity_arrival.is_none() {
                        timing.t_parity_arrival = Some(arr);
                        timing.t_redundancy_arrival = Some(arr);
                    }
                    recv_rep
                        .entry(esi)
                        .or_insert_with(|| repairs[esi as usize].clone());
                } else if delivered_src.insert(esi) {
                    recv_src.insert(esi, src[esi as usize].clone());
                    acks.push((arr + owd_ns, seq));
                    if seq == u64::MAX {
                        // retransmit-delivered-as-source path zaten seq'li; MAX yalnız-parity.
                    }
                    if delivered_src.len() == K as usize {
                        res.completion_ns = Some(now);
                        timing.t_block_complete = Some(now);
                    }
                }
                false
            } else {
                true
            }
        });
        // GERÇEK DECODE: distinct-source + repair ≥ K → reconstruct.
        let distinct = recv_src.len();
        if distinct < K as usize && distinct + recv_rep.len() >= K as usize && !recv_rep.is_empty()
        {
            let mut received: Vec<ReceivedSymbol> = Vec::new();
            for (esi, b) in &recv_src {
                received.push(ReceivedSymbol {
                    esi: *esi,
                    is_repair: false,
                    bytes: b,
                });
            }
            for (esi, b) in &recv_rep {
                received.push(ReceivedSymbol {
                    esi: *esi,
                    is_repair: true,
                    bytes: b,
                });
            }
            if let Ok(rec) = RsSimdCodec.reconstruct(&p, &received, 0) {
                // **t_repair_usable = reconstruct-TAMAMLANDI anı** (parity-arrival DEĞİL; codec-latency causal).
                if !rec.is_empty() && timing.t_repair_usable.is_none() {
                    timing.t_repair_usable = Some(now);
                }
                for r in rec {
                    if r.bytes != src[r.esi as usize] {
                        res.reconstruct_byte_equal = false; // pin-3 byte-equality
                    }
                    if delivered_src.insert(r.esi) {
                        recv_src.insert(r.esi, r.bytes.clone());
                        res.fec_decoded_sources += 1;
                        recovered_now.push(r.esi);
                        if delivered_src.len() == K as usize {
                            res.completion_ns = Some(now);
                            timing.t_block_complete = Some(now);
                        }
                    }
                }
            }
        }
        // RECOVERED → Sender (pin-2). ESTIMATOR-PURITY: RTT/BtlBw RECOVERED-öncesi/sonrası ÖZDEŞ.
        if !recovered_now.is_empty() {
            let pre = (s.path_timing_snapshot(), s.diag_wbtlbw());
            let units: Vec<RecoveredUnit> = recovered_now
                .iter()
                .map(|&esi| RecoveredUnit {
                    unit_id: u64::from(esi),
                    block_id: 0,
                    esi,
                })
                .collect();
            let _ = s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
            let post = (s.path_timing_snapshot(), s.diag_wbtlbw());
            if pre != post {
                res.estimator_pure = false; // RECOVERED estimator'a-dokundu (fake-ACK ihlali)
            }
        }
        // ACK due → Sender (yalnız gerçek-alınan-source; parity seq=MAX ACK-etmez).
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
            let (role, is_retx) = match tx.kind {
                TransmitKind::Data => (TransmissionRole::InitialSource, false),
                TransmitKind::Retransmit => {
                    res.retransmits += 1;
                    if timing.t_arq_eligible.is_none() {
                        timing.t_arq_eligible = Some(now);
                        timing.t_arq_wire = Some(now);
                    }
                    (TransmissionRole::RetransmitSource, true)
                }
                _ => continue,
            };
            let esi = tx.unit_id as u16;
            let id = TracePacketId {
                source_id: (0, esi),
                role,
                attempt: u16::from(is_retx),
            };
            if let Some(arr) = ch.offer(id, E as u64, now) {
                deliveries.push((arr, esi, false, tx.seq));
                if is_retx && timing.t_arq_arrival.is_none() {
                    // arrival capture: en-erken retransmit-teslim (yaklaşık; delivery-loop kesinleştirir).
                    timing.t_arq_arrival = Some(arr);
                }
            }
        }
    }
    res.post_timeout_notice_none = s.take_post_timeout_notice().is_none();
    res.delivered = delivered_src.len();
    res.serialized_bytes = ch.serialized_bytes();
    let pt = s.path_timing_snapshot();
    res.path_timing = PathTimingLedger {
        path_srtt_ns: pt.srtt_ns,
        path_min_rtt_ns: pt.min_rtt_ns,
        path_rttvar_ns: pt.rttvar_ns,
        path_queue_delay_ns: pt.queue_delay_ns,
        path_sample_present: pt.sample_present,
        path_rtprop_fresh: pt.rtprop_fresh,
    };
    res.instrument = PerRunInstrument {
        faithful: ch.trace_counts().1 == drop_initial.len() as u64,
        overlap_ok: receiver_events > 0,
        config_hash: 0, // F0 config-hash CLI-tarafında (arm+cfg)
        trace_hash: ch.trace_hash(),
        arm_intervention: RedundancyKind::FecRepair,
        configured_source_drops: drop_initial.len() as u64,
        realized_source_drops: ch.trace_counts().1,
        receiver_events,
    };
    res.timing = timing;
    res
}
