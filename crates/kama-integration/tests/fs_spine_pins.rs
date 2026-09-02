//! FINAL BUILD S1 — common multi-block spine diagnostic fixtures + 10 pins (§11-14,§19). DIAGNOSTIC (result-bearing-DEĞİL).
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_spine.rs"]
mod fs_spine;
use fs_spine::*;

const LINK: u64 = 1_000_000;
const OWD: u64 = 20 * MS;

// ── §11 A0 ──────────────────────────────────────────────────────────────────────────────────────
#[test]
fn a0_no_loss_full_workload_equality() {
    let r = run_spine(SpineArm::A0, &SpineCfg::diag(LINK, OWD, &[]));
    assert!(r.completed, "A0 no-loss tamamlanmalı");
    assert_eq!(r.delivered_source_symbols, 48);
    assert!(r.reassembly_equal, "A0 no-loss 2400B byte-equal");
    assert_eq!(r.delivered_app_bytes, TOTAL_APP_BYTES);
    assert!(r.admitted);
}

#[test]
fn a0_normal_deficit_recovers_via_arq() {
    // non-tail middle loss (global 20; block 2 esi 4).
    let r = run_spine(SpineArm::A0, &SpineCfg::diag(LINK, OWD, &[20]));
    assert!(r.completed, "A0 middle-deficit ARQ ile tamamlanır");
    assert!(r.retransmits > 0, "A0 ARQ-retransmit gerekli");
    assert!(r.reassembly_equal);
}

#[test]
fn a0_tail_structural_erasure_retained() {
    // tail global 47 (last) → structural tail-erasure (B_REAL_TAIL_ERASURE); admitted ama stall.
    let r = run_spine(SpineArm::A0, &SpineCfg::diag(LINK, OWD, &[47]));
    assert!(r.admitted, "harness-geçerli (PTO sadık-servis)");
    assert!(!r.completed, "tail-erasure yapısal → tamamlanmaz");
    assert_eq!(r.retransmits, 0, "seq<largest_acked → tail retransmit-YOK");
}

// ── §12 A1 ──────────────────────────────────────────────────────────────────────────────────────
#[test]
fn a1_hit_and_miss_and_residual_arq() {
    let targets = a1_targets(2, 0xD00D);
    let hit = *targets.iter().next().unwrap();
    let miss = (0..48u16).find(|g| !targets.contains(g)).unwrap();
    let rh = run_spine(SpineArm::A1 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[hit]));
    assert_eq!(rh.a1_dup_hits, 1, "hit: dropped ∈ target");
    assert!(rh.completed && rh.reassembly_equal);
    let rm = run_spine(SpineArm::A1 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[miss]));
    assert_eq!(rm.a1_dup_misses, 1, "miss: dropped ∉ target");
    assert!(rm.retransmits > 0, "miss → residual ARQ");
    assert!(rm.completed && rm.reassembly_equal);
}

#[test]
fn a1_cost_intervention_vs_total() {
    let r = run_spine(SpineArm::A1 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[]));
    // block-total R=2 × 6 reference-group = 12 duplicate; each 101B.
    assert_eq!(r.duplicates_emitted, 12, "6 group × R=2 = 12 dup");
    assert_eq!(r.intervention_bytes, 12 * 101);
    assert_eq!(r.base_source_bytes, 48 * 101, "base source 4848B");
}

// ── §13 F0 ──────────────────────────────────────────────────────────────────────────────────────
#[test]
fn f0_no_loss_full_workload_equality() {
    let r = run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[]));
    assert!(r.completed && r.reassembly_equal);
    assert_eq!(r.repair_count, 12, "6 block × R=2 = 12 repair");
    assert_eq!(r.intervention_bytes, 12 * 101);
    assert!(r.estimator_pure, "RECOVERED estimator-purity");
}

#[test]
fn f0_tail_fec_recovers_where_arq_cannot() {
    // workload-tail global 47 (block5 esi7): ARQ yapısal-kurtaramaz (seq<largest_acked) → FEC RELIABLY decode.
    let r = run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[47]));
    assert!(r.completed, "F0 tail: FEC kurtarır (ARQ imkânsız)");
    assert!(
        r.fec_decoded_sources >= 1,
        "gerçek spine FEC-decode (tail; ARQ-yapısal-imkânsız)"
    );
    assert!(r.reconstruct_byte_equal && r.reassembly_equal);
}

#[test]
fn f0_nontail_completes_hybrid() {
    // non-tail same-block 2-loss (global 2,5 block0): FEC-veya-ARQ (S0-B: parity DATA-gated → race) → tamamlanır.
    let r = run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[2, 5]));
    assert!(
        r.completed && r.reassembly_equal,
        "same-block 2-loss FEC-or-ARQ ile tamamlanır"
    );
    assert!(r.reconstruct_byte_equal);
    // cross-block da tamamlanır (global 3 block0 + 44 block5).
    let rc = run_spine(SpineArm::F0 { r: 1 }, &SpineCfg::diag(LINK, OWD, &[3, 44]));
    assert!(
        rc.completed && rc.reassembly_equal,
        "cross-block tamamlanır"
    );
}

/// §19.6 F0_REAL_CODEC_MULTIBLOCK — 6 K8-blok'un HEPSİ gerçek encode/decode (direct; deterministik; ARQ-race-YOK).
#[test]
fn f0_real_codec_all_six_blocks_direct() {
    use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, SourceSymbol};
    let p = by_id(0x03).unwrap(); // K8/R4/E64
    let canonical = canonical_workload();
    for b in 0..6usize {
        let syms: Vec<SourceSymbol> = (0..8)
            .map(|e| SourceSymbol {
                esi: e as u16,
                bytes: &canonical[b * 8 + e].symbol_bytes,
            })
            .collect();
        let reps = RsSimdCodec.encode(&p, &syms).unwrap();
        // drop esi 1,4; reconstruct 6 source + 2 repair.
        let mut recv: Vec<ReceivedSymbol> = Vec::new();
        for e in 0..8 {
            if e != 1 && e != 4 {
                recv.push(ReceivedSymbol {
                    esi: e as u16,
                    is_repair: false,
                    bytes: &canonical[b * 8 + e].symbol_bytes,
                });
            }
        }
        recv.push(ReceivedSymbol {
            esi: reps[0].esi,
            is_repair: true,
            bytes: &reps[0].bytes,
        });
        recv.push(ReceivedSymbol {
            esi: reps[1].esi,
            is_repair: true,
            bytes: &reps[1].bytes,
        });
        let rec = RsSimdCodec.reconstruct(&p, &recv, 0).unwrap();
        for want in [1usize, 4] {
            let got = rec
                .iter()
                .find(|s| s.esi == want as u16)
                .expect("block reconstruct");
            assert_eq!(
                got.bytes,
                canonical[b * 8 + want].symbol_bytes,
                "block {b} esi {want} byte-eq"
            );
        }
    }
}

// ── §19 PINS ──────────────────────────────────────────────────────────────────────────────────
#[test]
fn pin_canonical_source_equality_s1() {
    let (a0, a1, f0) = (
        run_spine(SpineArm::A0, &SpineCfg::diag(LINK, OWD, &[])),
        run_spine(SpineArm::A1 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[])),
        run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[])),
    );
    assert_eq!(a0.source_sequence_hash, a1.source_sequence_hash);
    assert_eq!(a1.source_sequence_hash, f0.source_sequence_hash);
}

#[test]
fn pin_source_schedule_equality_s1() {
    let c = SpineCfg::diag(LINK, OWD, &[]);
    let (a0, a1, f0) = (
        run_spine(SpineArm::A0, &c),
        run_spine(SpineArm::A1 { r: 2 }, &c),
        run_spine(SpineArm::F0 { r: 2 }, &c),
    );
    assert_eq!(a0.source_offer_schedule_hash, a1.source_offer_schedule_hash);
    assert_eq!(a1.source_offer_schedule_hash, f0.source_offer_schedule_hash);
    assert_eq!(
        a0.global_to_local_mapping_hash,
        f0.global_to_local_mapping_hash
    );
}

#[test]
fn pin_global_deficit_arm_invariance_and_no_multiplication_s1() {
    let c = SpineCfg::diag(LINK, OWD, &[2, 5]);
    let (a0, a1, f0) = (
        run_spine(SpineArm::A0, &c),
        run_spine(SpineArm::A1 { r: 2 }, &c),
        run_spine(SpineArm::F0 { r: 2 }, &c),
    );
    // GLOBAL_DEFICIT_ARM_INVARIANCE: same global dropped ids + same CONFIGURED schedule identity across arms.
    assert_eq!(a0.dropped_global_ids, vec![2, 5]);
    assert_eq!(a0.dropped_global_ids, a1.dropped_global_ids);
    assert_eq!(a1.dropped_global_ids, f0.dropped_global_ids);
    assert_eq!(
        a0.configured_schedule_hash, f0.configured_schedule_hash,
        "configured trace identity arm-invariant (realization/forward_trace_hash farklı-olabilir)"
    );
    assert_eq!(a1.configured_schedule_hash, f0.configured_schedule_hash);
    // DEFICIT_NO_MULTIPLICATION: realized InitialSource drops = d=2 (not 2×6 blocks) → faithful.
    assert!(
        a0.faithful && a1.faithful && f0.faithful,
        "realized drops == d (per-workload; per-block-çarpım-YOK)"
    );
    assert_eq!(a0.deficit_count, 2);
}

#[test]
fn pin_base_source_wire_equality_s1() {
    for arm in [SpineArm::A0, SpineArm::A1 { r: 2 }, SpineArm::F0 { r: 2 }] {
        let r = run_spine(arm, &SpineCfg::diag(LINK, OWD, &[]));
        assert_eq!(r.base_source_bytes, 48 * 101, "base source 4848B");
    }
}

#[test]
fn pin_multiblock_reassembly_byte_equality_s1() {
    // A0/A1/F0 recoverable diagnostic → 2400B.
    assert!(run_spine(SpineArm::A0, &SpineCfg::diag(LINK, OWD, &[20])).reassembly_equal);
    assert!(run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[2, 5])).reassembly_equal);
}
