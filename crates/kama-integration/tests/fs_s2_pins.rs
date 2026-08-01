//! FINAL BUILD S2 — F1/F3/F4 closed-loop adapters + structural-null + F4 real-codec + six-arm pins. DIAGNOSTIC.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_spine.rs"]
mod fs_spine;
use fs_spine::*;

const LINK: u64 = 1_000_000;
const OWD: u64 = 20 * MS;

// ── §7-8 F1: real decode + full-workload + BULK STRUCTURAL-NULL ───────────────────────────────────
#[test]
fn f1_real_decode_full_workload() {
    let r = run_spine(SpineArm::F1 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[47]));
    assert!(r.completed && r.reassembly_equal, "F1 full-workload 2400B");
    assert!(
        r.fec_decoded_sources >= 1,
        "F1 tail: gerçek FEC-decode (ARQ-imkânsız)"
    );
    assert!(r.reconstruct_byte_equal && r.estimator_pure);
    assert_eq!(r.selected_k, 8, "F1 K=8 (6 blok)");
}

/// §8/§21 F1_BULK_PARITY_WIRE_NULL — paired F0/F1 deterministic (no-loss): parity-wire final-source-SONRASI +
/// F1 first-parity-wire F0'ı GEÇMEZ (deterministik eşitlik; advancement → DUR-A).
#[test]
fn f1_bulk_parity_wire_null() {
    let c = SpineCfg::diag(LINK, OWD, &[]);
    let f0 = run_spine(SpineArm::F0 { r: 2 }, &c);
    let f1 = run_spine(SpineArm::F1 { r: 2 }, &c);
    // A: parity final-source-wire'dan ÖNCE olamaz.
    assert!(f0.t_first_parity_wire.unwrap() >= f0.t_final_source_wire_complete.unwrap());
    assert!(f1.t_first_parity_wire.unwrap() >= f1.t_final_source_wire_complete.unwrap());
    // C: F1 first-parity-wire F0'ı GEÇMEZ (deterministik; advancement → S0-B FALSIFIED → DUR-A).
    assert_eq!(
        f1.t_first_parity_wire, f0.t_first_parity_wire,
        "F1 parity-wire F0'dan-erken → STRUCTURAL FALSIFIED (DUR-A)"
    );
    assert_eq!(
        f1.structural_class,
        StructuralClass::BulkParityWireNullConfirmed
    );
}

// ── §9-11 F3: K4/12-block real decode + full-workload + STRUCTURAL-NULL + cost ──────────────────
#[test]
fn f3_k4_12block_real_decode_full_workload() {
    let r = run_spine(SpineArm::F3 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[46, 47]));
    assert_eq!(r.selected_k, 4, "F3 K'=4");
    assert_eq!(r.source_block_count, 12, "48/4 = 12 blok");
    assert!(
        r.completed && r.reassembly_equal,
        "F3 full 2400B (workload azalmaz)"
    );
    assert!(r.fec_decoded_sources >= 1, "F3 tail: gerçek FEC-decode");
    assert!(r.reconstruct_byte_equal);
}

#[test]
fn f3_bulk_parity_wire_null() {
    let c = SpineCfg::diag(LINK, OWD, &[]);
    let f0 = run_spine(SpineArm::F0 { r: 2 }, &c);
    let f3 = run_spine(SpineArm::F3 { r: 2 }, &c);
    assert!(f3.t_first_parity_wire.unwrap() >= f3.t_final_source_wire_complete.unwrap());
    assert_eq!(
        f3.t_first_parity_wire, f0.t_first_parity_wire,
        "F3 parity-wire F0'dan-erken → S0-B FALSIFIED (DUR-A)"
    );
    assert_eq!(
        f3.structural_class,
        StructuralClass::BulkParityWireNullConfirmed
    );
}

/// §11 F3 repair-count/cost (per-block R; block-count↑ → repair↑; visible, normalize-YOK).
#[test]
fn f3_repair_count_and_cost_distinct_from_f0() {
    let f0 = run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[]));
    let f3 = run_spine(SpineArm::F3 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[]));
    assert_eq!(f0.repair_count, 12, "F0: 6 blok × R=2 = 12");
    assert_eq!(f3.repair_count, 24, "F3: 12 blok × R=2 = 24");
    assert_eq!(f0.intervention_bytes, 12 * 101);
    assert_eq!(
        f3.intervention_bytes,
        24 * 101,
        "F3 intervention 2× F0 (BUDGET_MISMATCH-DEĞİL: gerçek-fark)"
    );
    assert_eq!(
        f0.base_source_bytes, f3.base_source_bytes,
        "base source aynı (4848B)"
    );
}

// ── §12-15 F4: direct-K real codec + geometry + no-leakage ────────────────────────────────────
#[test]
fn f4_k2_real_codec_24block() {
    let r = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[47]).with_f4_base_rtt(30 * MS),
    );
    assert_eq!(r.selected_k, 2, "base 30ms → K2");
    assert_eq!(r.selected_profile_id, 0x01);
    assert_eq!(r.source_block_count, 24, "48/2 = 24 blok");
    assert!(r.completed && r.reassembly_equal && r.reconstruct_byte_equal);
}

#[test]
fn f4_k4_real_codec_12block() {
    let r = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[47]).with_f4_base_rtt(100 * MS),
    );
    assert_eq!(r.selected_k, 4, "base 100ms → K4");
    assert_eq!(r.selected_profile_id, 0x02);
    assert_eq!(r.source_block_count, 12);
    assert!(r.completed && r.reassembly_equal && r.reconstruct_byte_equal);
}

#[test]
fn f4_k8_real_codec_6block() {
    let r = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[47]).with_f4_base_rtt(250 * MS),
    );
    assert_eq!(r.selected_k, 8, "base 250ms → K8");
    assert_eq!(r.selected_profile_id, 0x03);
    assert_eq!(r.source_block_count, 6);
    assert!(r.completed && r.reassembly_equal && r.reconstruct_byte_equal);
}

/// §15 F4 no-leakage: f4_decide_k YALNIZ base_rtt (pre-decision) alır; deterministik; no future/outcome.
/// K seçimi drop-set/outcome'dan BAĞIMSIZ (aynı base_rtt, farklı-deficit → aynı K).
#[test]
fn f4_no_leakage_predecision_only() {
    let base = 250 * MS;
    let k_a = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[]).with_f4_base_rtt(base),
    )
    .selected_k;
    let k_b = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[2, 5]).with_f4_base_rtt(base),
    )
    .selected_k;
    let k_c = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[47]).with_f4_base_rtt(base),
    )
    .selected_k;
    assert_eq!(k_a, k_b, "K deficit-outcome'dan bağımsız (no-leakage)");
    assert_eq!(k_b, k_c);
    // deterministik: aynı base → aynı K (pure).
    assert_eq!(f4_decide_k(base).k(), 8);
    assert_eq!(f4_decide_k(30 * MS).k(), 2);
    assert_eq!(f4_decide_k(100 * MS).k(), 4);
    assert_eq!(f4_decide_k(0).k(), 4, "cold → K_COLD=K4");
    // §14: yalnız {2,4,8}; K1/K3 asla.
    for base in [0u64, 10, 49, 50, 149, 150, 300, 5000].map(|m| m * MS) {
        assert!(
            matches!(f4_decide_k(base).k(), 2 | 4 | 8),
            "K1/K3 ÜRETİLMEZ"
        );
    }
}

// ── §20 SIX-ARM EQUIVALENCE PINS ────────────────────────────────────────────────────────────────
fn all_arms(c: &SpineCfg) -> Vec<SpineResult> {
    vec![
        run_spine(SpineArm::A0, c),
        run_spine(SpineArm::A1 { r: 2 }, c),
        run_spine(SpineArm::F0 { r: 2 }, c),
        run_spine(SpineArm::F1 { r: 2 }, c),
        run_spine(SpineArm::F3 { r: 2 }, c),
        run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(250 * MS)),
    ]
}

#[test]
fn s2_canonical_source_and_schedule_equality() {
    let c = SpineCfg::diag(LINK, OWD, &[]);
    let arms = all_arms(&c);
    let sh = arms[0].source_sequence_hash;
    let sc = arms[0].source_offer_schedule_hash;
    for a in &arms {
        assert_eq!(a.source_sequence_hash, sh, "{}: source-seq farklı", a.arm);
        assert_eq!(
            a.source_offer_schedule_hash, sc,
            "{}: schedule farklı",
            a.arm
        );
        assert_eq!(
            a.base_source_bytes,
            48 * 101,
            "{}: base-source 4848B",
            a.arm
        );
        assert_eq!(a.source_symbol_count, 48);
    }
    // F4 K2/K4/K8 also share source seq/schedule.
    for base in [30u64, 100, 250].map(|m| m * MS) {
        let f4 = run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(base));
        assert_eq!(f4.source_sequence_hash, sh);
        assert_eq!(f4.source_offer_schedule_hash, sc);
        assert_eq!(f4.base_source_bytes, 48 * 101);
    }
}

#[test]
fn s2_global_deficit_arm_invariance_no_multiplication() {
    let c = SpineCfg::diag(LINK, OWD, &[2, 5]);
    let arms = all_arms(&c);
    let ident = arms[0].configured_schedule_hash;
    for a in &arms {
        assert_eq!(
            a.dropped_global_ids,
            vec![2, 5],
            "{}: dropped-ids farklı",
            a.arm
        );
        assert_eq!(
            a.configured_schedule_hash, ident,
            "{}: configured-identity farklı",
            a.arm
        );
        assert!(
            a.faithful,
            "{}: realized-drops != d (per-block-çarpım?)",
            a.arm
        );
        assert_eq!(a.deficit_count, 2);
    }
    // F4 across K: same global deficit (per-workload; not per-block).
    for base in [30u64, 100, 250].map(|m| m * MS) {
        let f4 = run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(base));
        assert_eq!(
            f4.dropped_global_ids,
            vec![2, 5],
            "F4 K{}: deficit farklı",
            f4.selected_k
        );
        assert!(f4.faithful);
    }
}

#[test]
fn s2_multiblock_reassembly_byte_equality() {
    // F1/F3/F4-K2/K4/K8 recoverable → 2400B (tail via FEC; ARQ-imkânsız → FEC-guaranteed).
    assert!(run_spine(SpineArm::F1 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[47])).reassembly_equal);
    assert!(run_spine(SpineArm::F3 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[47])).reassembly_equal);
    for base in [30u64, 100, 250].map(|m| m * MS) {
        let r = run_spine(
            SpineArm::F4 { r: 2 },
            &SpineCfg::diag(LINK, OWD, &[47]).with_f4_base_rtt(base),
        );
        assert!(r.reassembly_equal, "F4 K{} reassembly", r.selected_k);
    }
}
