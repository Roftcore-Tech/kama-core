//! FINAL BUILD S5-B — outcome-blind analysis validators (SYNTHETIC ONLY; §22). campaign-data INGEST-YOK.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_analysis.rs"]
mod fs_analysis;
use fs_analysis::*;

fn rec(arm: &'static str, pg: &str, attempt: u8) -> AttemptRecord {
    AttemptRecord {
        run_id: format!("fs1--{pg}--{arm}"),
        arm,
        pairing_group: pg.to_string(),
        attempt_index: attempt,
        provenance_ok: true,
        schema_ok: true,
        delivered_fragments: 48,
        delivered_bytes: 2400,
        completed_before_horizon: true,
        intervention_bytes: 0,
        total_bytes: 4848,
        tail_included: false,
    }
}

// ── §22-A complete four-arm pairing group ──────────────────────────────────────────────────────────
#[test]
fn synth_complete_four_arm_group() {
    let g = "pg--rtt50-exactd2--rep0";
    let recs = ["A0", "A1-EARLY", "F0", "F3"]
        .map(|a| rec(a, g, 0))
        .to_vec();
    for r in &recs {
        assert_eq!(classify_validity(r), ValidityClass::ValidComplete);
    }
    assert!(pairing_complete(&recs), "4-arm complete group → paired");
    assert!(!has_duplicate_output(&recs));
}

// ── §22-B censored A0 tail (VALID_CENSORED, INVALID-DEĞİL) ──────────────────────────────────────────
#[test]
fn synth_censored_a0_tail() {
    let mut r = rec("A0", "pg--rtt50-exactd1--rep0", 0);
    r.completed_before_horizon = false;
    r.delivered_fragments = 47;
    r.delivered_bytes = 2347;
    r.tail_included = true;
    assert_eq!(
        classify_validity(&r),
        ValidityClass::ValidCensored,
        "tail-stall = VALID_CENSORED (INVALID-DEĞİL)"
    );
    // censored ≠ retry-edilebilir.
    assert!(!retry_permitted(&r), "VALID_CENSORED retry-YASAK");
}

// ── §22-C invalid provenance ───────────────────────────────────────────────────────────────────────
#[test]
fn synth_invalid_provenance() {
    let mut r = rec("F0", "pg--rtt250-iid20--rep1", 0);
    r.provenance_ok = false; // freeze-lock/manifest/binary mismatch
    assert_eq!(classify_validity(&r), ValidityClass::Invalid);
    assert!(
        retry_permitted(&r),
        "INVALID retry-edilebilir (attempt 0 → 1)"
    );
    let mut r2 = r.clone();
    r2.attempt_index = 2;
    assert!(
        !retry_permitted(&r2),
        "max-3 (attempt 0,1,2) → index 2'de retry-YOK"
    );
}

// ── §22-D missing paired arm ───────────────────────────────────────────────────────────────────────
#[test]
fn synth_missing_paired_arm() {
    let g = "pg--rtt1000-exactd4--rep2";
    let recs = ["A0", "A1-EARLY", "F0"].map(|a| rec(a, g, 0)).to_vec(); // F3 eksik
    assert!(
        !pairing_complete(&recs),
        "eksik-arm → pairing incomplete (unpaired-satırla DEĞİŞTİRME)"
    );
}

// ── §22-E duplicate output ─────────────────────────────────────────────────────────────────────────
#[test]
fn synth_duplicate_output() {
    let g = "pg--rtt50-iid20--rep3";
    let recs = vec![rec("F0", g, 0), rec("F0", g, 0)]; // aynı run_id+attempt
    assert!(has_duplicate_output(&recs), "duplicate-output tespit");
}

// ── §22-F retry lineage ────────────────────────────────────────────────────────────────────────────
#[test]
fn synth_retry_lineage() {
    let mut inv = rec("F3", "pg--rtt250-exactd2--rep0", 0);
    inv.provenance_ok = false;
    assert!(retry_permitted(&inv), "INVALID attempt0 → retry (attempt1)");
    // VALID attempt retry-YASAK.
    let ok = rec("F3", "pg--rtt250-exactd2--rep0", 0);
    assert!(!retry_permitted(&ok), "VALID_COMPLETE retry-YASAK");
}

// ── §22-G F0/A1 equal-budget (matched-wire → NonDominated; coding-causal-uygun) ─────────────────────
#[test]
fn synth_f0_a1_equal_budget() {
    // F0 r=2 = 12×101 = 1212 ; A1 r=2 = 12×101 = 1212 (matched).
    assert_eq!(
        coding_causal_fairness(1212, 1212),
        FairnessClass::NonDominated
    );
}

// ── §22-H F3 budget mismatch (2424 vs 1212 → BUDGET_MISMATCH; coding-causal-YASAK) ──────────────────
#[test]
fn synth_f3_budget_mismatch() {
    assert_eq!(
        coding_causal_fairness(2424, 1212),
        FairnessClass::BudgetMismatch
    );
    // O2 (FEC-specific) BUDGET_MISMATCH altında verilemez.
    let o = oracle_class(true, true, true, FairnessClass::BudgetMismatch, true);
    assert_ne!(
        o,
        OracleClass::FecSpecificValue,
        "budget-mismatch → FEC-specific-value YASAK"
    );
    assert_eq!(o, OracleClass::RedundancyValue);
}

// ── §22-I NaturalIid realized strata (exploratory; primary'yi değiştirmez / lineage-düşürmez) ───────
#[test]
fn synth_naturaliid_realized_strata() {
    // realized tail-inclusion yalnız metadata: aynı delivered/completion → aynı validity (strata primary'yi değiştirmez).
    let mut a = rec("F0", "pg--rtt50-iid20--rep0", 0);
    a.completed_before_horizon = false;
    a.delivered_fragments = 40;
    a.delivered_bytes = 2000;
    let mut b = a.clone();
    b.tail_included = true; // yalnız realized-strata metadata farkı
    assert_eq!(
        classify_validity(&a),
        classify_validity(&b),
        "realized-strata validity'yi DEĞİŞTİRMEZ"
    );
    assert_eq!(
        classify_validity(&a),
        ValidityClass::ValidCensored,
        "censored-IID lineage-DÜŞÜRÜLMEZ"
    );
}

// ── §22-J F4 derived-policy non-independence (RULE-17) ──────────────────────────────────────────────
#[test]
fn synth_f4_non_independence() {
    assert!(
        !f4_is_independent_evidence(),
        "F4 independent-campaign-evidence DEĞİL"
    );
    assert!(!is_result_bearing_arm("F4"), "F4 result-bearing-DIŞI");
}

// ── §22-K F1 diagnostic-only rejection ─────────────────────────────────────────────────────────────
#[test]
fn synth_f1_diagnostic_only_rejection() {
    assert!(
        !is_result_bearing_arm("F1"),
        "F1 result-dataset'ten reddedilir (diagnostic-only)"
    );
    for a in ["A0", "A1-EARLY", "F0", "F3"] {
        assert!(is_result_bearing_arm(a), "{a} result-bearing");
    }
}

// ── leakage firewall (§20) + oracle O0..O3 ─────────────────────────────────────────────────────────
#[test]
fn synth_leakage_firewall_and_oracle() {
    // pre-decision path-history ALLOWED; realized-outcome FORBIDDEN.
    for allowed in ["base_rtt_ns", "min_rtt_ns", "srtt_ns", "queue_delay_ns"] {
        assert!(
            is_allowed_decision_feature(allowed),
            "{allowed} pre-decision → allowed"
        );
    }
    for forbidden in [
        "realized_loss_count",
        "completion_ns",
        "deadline_hit",
        "delivered_bytes",
        "validity_class",
    ] {
        assert!(
            !is_allowed_decision_feature(forbidden),
            "{forbidden} leakage → forbidden"
        );
    }
    // oracle classes.
    assert_eq!(
        oracle_class(false, false, false, FairnessClass::NonDominated, true),
        OracleClass::NoRedundancyValue
    );
    assert_eq!(
        oracle_class(true, false, false, FairnessClass::NonDominated, true),
        OracleClass::RedundancyValue
    );
    assert_eq!(
        oracle_class(true, true, true, FairnessClass::NonDominated, true),
        OracleClass::FecSpecificValue
    );
    assert_eq!(
        oracle_class(true, true, true, FairnessClass::NonDominated, false),
        OracleClass::Insufficient
    );
    // FEC>A0 tek-başına (fec_beats_a1=false) → O2-DEĞİL.
    assert_eq!(
        oracle_class(false, true, false, FairnessClass::NonDominated, true),
        OracleClass::RedundancyValue
    );
}
