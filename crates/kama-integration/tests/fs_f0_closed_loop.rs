//! DR-FEC-FS-1 FS-HARNESS-A — F0 CLOSED-LOOP (gerçek Sender FEC-authority + GERÇEK RsSimdCodec + real decode).
//!
//! Abkarada 3-bağlayıcı-pin:
//! 1. Crate: kama-integration test (production-dep-graph GENİŞLEMEZ; kama-fec-codec zaten-prod-dep,
//!    adaptive-transport dev-only).
//! 2. Decode→sender closure = **RECOVERED** (fake-ACK YASAK); F0_RECOVERED_SCOPE: steady-F0 → notice==none.
//! 3. Sender-side repair de **gerçek codec çıktısı**: K-source → RsSimdCodec.encode → repair-bytes →
//!    channel → RsSimdCodec.reconstruct → reconstructed==original (byte-equality). Count-based/synthetic YASAK.
//!
//! F0 = ARQ + fixed-R block-close FEC (Manual; PathTiming/controller-YOK). Historical control.

#![cfg(feature = "experimental-fs-harness")]

use adaptive_transport::fs_harness::BottleneckChannel;
use adaptive_transport::fs_trace::{
    RedundancyPolicy, TraceChannel, TracePacketId, TraceSpec, TransmissionRole,
};
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, Timer, TransmitKind,
};
use kama_fec_codec::{by_id, FecCodec, FecProfile, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const K: u16 = 8;
const E: usize = 64;
const MS: u64 = 1_000_000;

fn profile() -> FecProfile {
    by_id(0x03).unwrap() // K=8, R=4, E=64
}
/// Deterministik K source-symbol (E-byte; esi-damgalı).
fn source_bytes() -> Vec<Vec<u8>> {
    (0..K as usize)
        .map(|i| {
            (0..E)
                .map(|j| (i as u8).wrapping_mul(31).wrapping_add(j as u8))
                .collect()
        })
        .collect()
}
/// GERÇEK codec ENCODE → repair-bytes (pin-3: synthetic-DEĞİL).
fn real_repairs(src: &[Vec<u8>]) -> Vec<Vec<u8>> {
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

struct F0Result {
    delivered: usize,
    completion_ns: Option<u64>,
    fec_decoded_sources: usize,
    retransmits: u64,
    parity_emitted: u64,
    post_timeout_notice_none: bool,
    reconstruct_byte_equal: bool,
    serialized_bytes: u64,
}

/// F0 kapalı-döngü: gerçek Sender (source+ARQ) + Sender-authority-fixed-R + GERÇEK encode/reconstruct + RECOVERED.
fn run_f0(manual_r: u16, drop_initial: &[u16], link_bps: u64, owd_ns: u64) -> F0Result {
    let src = source_bytes();
    let repairs = real_repairs(&src); // pin-3: gerçek encode
    let p = profile();
    let drop_set: BTreeSet<(u32, u16)> = drop_initial.iter().map(|&e| (0u32, e)).collect();
    let trace = TraceChannel::new(TraceSpec::ExactDeficit {
        drop_initial: drop_set,
        redundancy: RedundancyPolicy::PassAll,
    });
    let mut ch = BottleneckChannel::new(trace, link_bps, owd_ns);
    let mut s = Sender::new();
    // F0 PURITY: Manual fixed-R (Controller/PathTiming-YOK).
    s.set_manual_fec_enabled(true).unwrap();

    let mut delivered_src: BTreeSet<u16> = BTreeSet::new();
    let mut recv_src: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut recv_rep: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    let mut deliveries: Vec<(u64, u16, bool, u64)> = Vec::new(); // (arrival, esi, is_repair, seq)
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut res = F0Result {
        delivered: 0,
        completion_ns: None,
        fec_decoded_sources: 0,
        retransmits: 0,
        parity_emitted: 0,
        post_timeout_notice_none: true,
        reconstruct_byte_equal: true,
        serialized_bytes: 0,
    };

    // t=0: K source DATA (esi=unit_id).
    let emit = |decisions: Vec<adaptive_transport::sender::TransmitDecision>,
                now: u64,
                ch: &mut BottleneckChannel,
                deliveries: &mut Vec<(u64, u16, bool, u64)>,
                res: &mut F0Result| {
        for d in decisions {
            let (role, is_repair) = match d.kind {
                TransmitKind::Data => (TransmissionRole::InitialSource, false),
                TransmitKind::Retransmit => {
                    res.retransmits += 1;
                    (TransmissionRole::RetransmitSource, false)
                }
                _ => continue,
            };
            let esi = d.unit_id as u16;
            let id = TracePacketId {
                source_id: (0, esi),
                role,
                attempt: u16::from(d.kind == TransmitKind::Retransmit),
            };
            if let Some(arr) = ch.offer(id, E as u64, now) {
                deliveries.push((arr, esi, is_repair, d.seq));
            }
        }
    };
    let mut d0 = Vec::new();
    for u in 0..K as u64 {
        d0.extend(s.on_event(
            Event::AppData(OpaqueUnit {
                id: u,
                size: E as u64,
            }),
            0,
        ));
    }
    emit(d0, 0, &mut ch, &mut deliveries, &mut res);

    // Block-close: authority (Manual fixed-R) → parity emit (gerçek repair-bytes).
    let auth = s.open_fec_block_authority();
    let authorized = s
        .close_fec_block_manual(auth, 0, manual_r, E as u64)
        .unwrap();
    res.parity_emitted = u64::from(authorized);
    for esi in 0..authorized {
        // pin-3: GERÇEK codec-repair channel'a.
        let id = TracePacketId {
            source_id: (0, esi),
            role: TransmissionRole::Repair,
            attempt: 0,
        };
        if let Some(arr) = ch.offer(id, E as u64, 0) {
            deliveries.push((arr, esi, true, u64::MAX));
        }
    }

    let mut now = 0u64;
    let horizon = 20_000 * MS;
    while now < horizon && delivered_src.len() < K as usize {
        now += MS;
        // Forward delivery.
        let mut recovered_now: Vec<u16> = Vec::new();
        deliveries.retain(|&(arr, esi, is_repair, seq)| {
            if arr <= now {
                if is_repair {
                    recv_rep
                        .entry(esi)
                        .or_insert_with(|| repairs[esi as usize].clone());
                } else if delivered_src.insert(esi) {
                    recv_src.insert(esi, src[esi as usize].clone());
                    acks.push((arr + owd_ns, seq)); // gerçek-alınan-source → normal ACK
                    if delivered_src.len() == K as usize {
                        res.completion_ns = Some(now);
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
                for r in rec {
                    // pin-3 byte-equality: reconstructed == original.
                    if r.bytes != src[r.esi as usize] {
                        res.reconstruct_byte_equal = false;
                    }
                    if delivered_src.insert(r.esi) {
                        recv_src.insert(r.esi, r.bytes.clone());
                        res.fec_decoded_sources += 1;
                        recovered_now.push(r.esi);
                        if delivered_src.len() == K as usize {
                            res.completion_ns = Some(now);
                        }
                    }
                }
            }
        }
        // RECOVERED → Sender (pin-2: fake-ACK DEĞİL; residual-ARQ-cancel).
        if !recovered_now.is_empty() {
            let units: Vec<RecoveredUnit> = recovered_now
                .iter()
                .map(|&esi| RecoveredUnit {
                    unit_id: u64::from(esi),
                    block_id: 0,
                    esi,
                })
                .collect();
            let _ = s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
        }
        // ACK due → Sender.
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
        if !due.is_empty() {
            let d = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: due,
                    ack_delay: Duration::from_nanos(0),
                }),
                now,
            );
            emit(d, now, &mut ch, &mut deliveries, &mut res);
        }
        // PTO → residual ARQ (hybrid).
        let d = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(d, now, &mut ch, &mut deliveries, &mut res);
    }
    // F0_RECOVERED_SCOPE: steady-F0 → PostTimeoutNotice YOK (Sender hiç TimedOut olmadı).
    res.post_timeout_notice_none = s.take_post_timeout_notice().is_none();
    res.delivered = delivered_src.len();
    res.serialized_bytes = ch.serialized_bytes();
    res
}

/// F0_LOOPBACK: loss-yok → block-tamamlanır (normal delivery); parity substrate'i BOZMAZ; cold-start-sızmaz.
/// NOT: parity yavaş-source'tan-önce-varırsa FEC erken-decode-edebilir (kayıp-olmasa-da; meşru FEC-erken-
/// tamamlama) — bu normal-delivery'yi BOZMAZ; decode/retransmit-sayısı pacing'e-bağlı-değişebilir.
#[test]
fn f0_loopback() {
    let r = run_f0(2, &[], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8, "loopback tüm-source teslim etmeli");
    assert!(r.completion_ns.is_some(), "block tamamlanmalı");
    assert!(
        r.reconstruct_byte_equal,
        "reconstruct (varsa) byte-özdeş olmalı"
    );
    assert!(
        r.post_timeout_notice_none,
        "cold-start sızmamalı (RECOVERED_SCOPE)"
    );
}

/// F0_EXACT_D1_R1 + F0_ENCODER_REAL: 1-source-erased + 1-real-repair → GERÇEK reconstruct == original.
#[test]
fn f0_exact_d1_r1_real_decode_byte_equal() {
    let r = run_f0(1, &[3], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8, "FEC + hybrid ile tamamlanır");
    assert_eq!(
        r.fec_decoded_sources, 1,
        "1 source FEC-decode ile kurtarıldı"
    );
    assert!(
        r.reconstruct_byte_equal,
        "reconstructed != original (fake/bozuk decode)"
    );
    assert!(
        r.post_timeout_notice_none,
        "steady-F0 → PostTimeoutNotice YOK (RECOVERED_SCOPE)"
    );
}

/// F0_EXACT_D2_R2: 2-erasure + 2-repair → GERÇEK 2-source reconstruct == original.
#[test]
fn f0_exact_d2_r2_two_erasure_real_decode() {
    let r = run_f0(2, &[2, 5], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8);
    assert_eq!(r.fec_decoded_sources, 2, "2 source FEC-decode");
    assert!(r.reconstruct_byte_equal);
    assert!(r.post_timeout_notice_none);
}

/// F0_EXACT_D2_R1 (HYBRID): FEC-yetersiz (2-kayıp, 1-repair) → residual-ARQ tamamlar (ARQ-KAPALI-DEĞİL).
#[test]
fn f0_exact_d2_r1_insufficient_fec_residual_arq() {
    let r = run_f0(1, &[2, 5], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8, "hybrid: FEC-yetersiz olsa da ARQ tamamlar");
    assert!(
        r.retransmits > 0,
        "residual-ARQ çalışmalı (harness ARQ'yu-kapatmadı)"
    );
    assert!(r.reconstruct_byte_equal);
    assert!(r.post_timeout_notice_none);
}

/// F0_PURITY: manual-R sabit; PathTimingState/controller etkisi YOK (parity-count == manual_r).
#[test]
fn f0_purity_fixed_r_no_controller() {
    for &rr in &[1u16, 2, 4] {
        let r = run_f0(rr, &[], 1_000_000, 20 * MS);
        assert_eq!(
            r.parity_emitted,
            u64::from(rr),
            "manual-R sabit değil (controller sızdı?)"
        );
    }
}
