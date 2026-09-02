//! FINAL BUILD S4-A — repetition-lineage uniqueness (RULE-10) + OPT-A result-bearing arm-set. DIAGNOSTIC
//! (result-bearing-DEĞİL; manifest-DIŞI). Frozen outcome-blind seed/target selection.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_cell.rs"]
mod fs_cell;
use fs_cell::*;
use std::collections::BTreeSet;

// ── PIN 1: OPT_A_RESULT_BEARING_ARM_SET ────────────────────────────────────────────────────────────
#[test]
fn opt_a_result_bearing_arm_set() {
    assert_eq!(FS1_RESULT_BEARING_ARMS, ["A0", "A1-EARLY", "F0", "F3"]);
    assert_eq!(
        FS1_DIAGNOSTIC_ARMS,
        ["F1", "F4"],
        "F1/F4 → diagnostic (collapse)"
    );
    assert_eq!(FS1_RESULT_BEARING_RUNS, 240, "12 × 4 × 5 = 240 (OPT-A)");
    assert_eq!(
        amended_base_cells().len() * FS1_RESULT_BEARING_ARMS.len() * FS1_REPS as usize,
        240
    );
    // F1/F4 result-bearing-arm-set'te DEĞİL (collapse → diagnostic).
    for diag in FS1_DIAGNOSTIC_ARMS {
        assert!(
            !FS1_RESULT_BEARING_ARMS.contains(&diag),
            "{diag} result-bearing-DIŞI olmalı"
        );
    }
}

// ── PIN 2: EXACT_DEFICIT_REPETITION_LINEAGE_UNIQUE (§5) ─────────────────────────────────────────────
#[test]
fn exact_deficit_repetition_lineage_unique() {
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
    {
        let d = c.impairment.configured_exact_drop_count().unwrap();
        let mut seeds = BTreeSet::new();
        let mut sets = BTreeSet::new();
        let rank_seed = exact_rank_seed(c);
        for rep in 0..FS1_REPS {
            let seed = paired_trace_seed(c, rep);
            let target = exact_deficit_target_set(d, rank_seed, rep);
            // her rep: set-size == drop_count, dup-YOK, ∈ [0,47].
            assert_eq!(target.len(), d as usize, "target-size == drop_count");
            assert_eq!(
                target.iter().collect::<BTreeSet<_>>().len(),
                target.len(),
                "dup-id YOK"
            );
            assert!(target.iter().all(|&g| g < 48), "global-id ∈ [0,47]");
            seeds.insert(seed);
            sets.insert(target);
        }
        // 5-rep → 5 distinct seed + 5 distinct target-set (REPETITION SEMANTICS DUPLICATION → DUR-B aksi halde).
        assert_eq!(
            seeds.len(),
            FS1_REPS as usize,
            "{}: 5-rep 5-distinct-seed",
            base_cell_id(c)
        );
        assert_eq!(
            sets.len(),
            FS1_REPS as usize,
            "{}: 5-rep 5-distinct-drop-set",
            base_cell_id(c)
        );
    }
}

// ── PIN 3: EXACT_DEFICIT_TARGET_DETERMINISTIC_ARM_INDEPENDENT (§7) ──────────────────────────────────
#[test]
fn exact_deficit_target_deterministic_arm_independent() {
    // deterministik: aynı (cell,rep) → aynı target (arm GİRDİ-DEĞİL; target arm-bağımsız).
    let c = amended_base_cells()[1]; // rtt50-exactd2
    let rs = exact_rank_seed(&c);
    assert_eq!(
        exact_deficit_target_set(2, rs, 3),
        exact_deficit_target_set(2, rs, 3),
        "deterministik"
    );
    assert_eq!(
        exact_drop_set_hash(&exact_deficit_target_set(2, rs, 3)),
        exact_drop_set_hash(&exact_deficit_target_set(2, rs, 3))
    );
    // non-overlapping window: farklı-rep farklı-set (garantili).
    assert_ne!(
        exact_deficit_target_set(2, rs, 0),
        exact_deficit_target_set(2, rs, 1)
    );
    // farklı cell (farklı base-hash) → farklı seed.
    let c0 = amended_base_cells()[0]; // rtt50-exactd1
    assert_ne!(
        paired_trace_seed(&c0, 3),
        paired_trace_seed(&c, 3),
        "farklı-cell farklı-seed"
    );
}

// ── PIN 4: IID_REPETITION_LINEAGE_UNIQUE (§5/§8) ───────────────────────────────────────────────────
#[test]
fn iid_repetition_lineage_unique() {
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "NaturalIid")
    {
        assert_eq!(c.impairment.configured_iid_loss_permille(), Some(200));
        assert_eq!(
            c.impairment.configured_exact_drop_count(),
            None,
            "IID → deficit-label YOK"
        );
        let mut seeds = BTreeSet::new();
        for rep in 0..FS1_REPS {
            seeds.insert(paired_trace_seed(c, rep));
        }
        assert_eq!(
            seeds.len(),
            FS1_REPS as usize,
            "{}: 5-rep 5-distinct trace-seed",
            base_cell_id(c)
        );
    }
}

// ── PIN 5: base_cell_id NO historical-deficit-in-IID (§8) ───────────────────────────────────────────
#[test]
fn iid_cell_id_no_historical_deficit() {
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "NaturalIid")
    {
        let id = base_cell_id(c);
        assert!(
            !id.contains("exactd")
                && !id.contains("-d1")
                && !id.contains("-d2")
                && !id.contains("-d4"),
            "IID cell-id'de historical-deficit-label YOK: {id}"
        );
        assert!(id.contains("iid20"));
    }
    // ExactDeficit cell-id drop_count taşır (grounded).
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
    {
        assert!(base_cell_id(c).contains("exactd"));
    }
}
