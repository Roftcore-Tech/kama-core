//! FINAL BUILD S4-C — 240/240 NON-RESULT executability + result-bearing firewall + coverage. DIAGNOSTIC.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_dryrun.rs"]
mod fs_dryrun;
use fs_dryrun::*;
use std::collections::BTreeSet;

// ── PIN 1: 240/240 DISPATCHABLE (§22) ──────────────────────────────────────────────────────────────
#[test]
fn executability_all_dispatchable() {
    let s = executability_sweep();
    assert_eq!(s.total, 240, "candidate rows = 240");
    assert_eq!(
        s.dispatchable, 240,
        "DISPATCHABLE = 240 (aksi NO-S4-PASS → DUR-B)"
    );
    assert_eq!(s.unsupported, 0, "unsupported = 0");
    assert_eq!(s.ambiguous, 0, "ambiguous = 0");
    assert_eq!(s.invalid, 0, "invalid = 0");
    assert_eq!(s.duplicate, 0, "duplicate = 0");
    assert!(s.failures.is_empty(), "failures: {:?}", s.failures);
    // by-family.
    assert_eq!(
        s.by_family.get("ExactDeficit").copied(),
        Some(180),
        "ED 270→180 (OPT-A)"
    );
    assert_eq!(
        s.by_family.get("NaturalIid").copied(),
        Some(60),
        "IID 90→60 (OPT-A)"
    );
    // by-arm.
    for arm in ["A0", "A1-EARLY", "F0", "F3"] {
        assert_eq!(s.by_arm.get(arm).copied(), Some(60), "{arm} = 60/60");
    }
    assert!(s.output_paths_unique, "output-path collision-YOK");
}

// ── PIN 2: S4_ZERO_RESULT_BEARING_EXECUTION (§20) ──────────────────────────────────────────────────
#[test]
fn s4_zero_result_bearing_execution() {
    assert_eq!(DRYRUN_MODE, "EXECUTABILITY_ONLY");
    // raw-dataset namespace HENÜZ-YOK (v1.3-freeze PENDING) → result-bearing-firewall.
    assert!(
        RAW_ARTIFACT_NAMESPACE.contains("PENDING-v1.3-FREEZE"),
        "raw-namespace = pending-freeze"
    );
    // manifest'in HER satırı result_bearing=false + candidate.
    for line in manifest_jsonl().lines() {
        assert!(line.contains("\"result_bearing\":false"));
        assert!(line.contains("\"prereg_status\":\"CANDIDATE_NOT_FROZEN\""));
    }
    // executability report firewall alanları.
    let j = executability_report_json();
    assert!(j.contains("\"result_bearing\": false"));
    assert!(j.contains("\"raw_result_files_written\": 0"));
    assert!(j.contains("\"channel_loop_advanced\": false"));
    assert!(j.contains("\"outcome_fields_populated\": false"));
    assert!(j.contains("\"fs1_run_ids_spent\": 0"));
    // ExecSummary'de outcome-alanı YOK (yalnız dispatchability-metadata; yapısal firewall).
}

// ── PIN 3: IID seed coverage (§27; mechanism-diagnostic) ───────────────────────────────────────────
#[test]
fn iid_seed_coverage_unique() {
    let mut seeds = BTreeSet::new();
    let mut patterns = BTreeSet::new();
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "NaturalIid")
    {
        for rep in 0..FS1_REPS {
            let seed = paired_trace_seed(c, rep);
            seeds.insert(seed);
            patterns.insert(iid_admission_pattern_hash(seed));
        }
    }
    assert_eq!(seeds.len(), 15, "15 unique IID trace-seed (3 cell × 5 rep)");
    assert_eq!(
        patterns.len(),
        15,
        "15 unique InitialSource admission-pattern (collision-YOK)"
    );
}

// ── PIN 4: ExactDeficit tail coverage (§26; diagnostic) ────────────────────────────────────────────
#[test]
fn exact_deficit_tail_coverage() {
    let any_tail = amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
        .any(|c| {
            let d = c.impairment.configured_exact_drop_count().unwrap();
            let rs = exact_rank_seed(c);
            (0..FS1_REPS).any(|rep| exact_deficit_target_set(d, rs, rep).contains(&47))
        });
    assert!(
        any_tail,
        "tail(g47) en-az-bir ExactDeficit-rep'te temsil-edilmeli (strata coverage)"
    );
}

// ── PIN 5: executability report consistency (read-only; drift-YOK) ─────────────────────────────────
const EXEC_JSON: &str = include_str!(
    "../../../reports/decision-requests/FS-1-MANIFEST-EXECUTABILITY-v1.3-CANDIDATE.json"
);
const EXEC_MD: &str = include_str!(
    "../../../reports/decision-requests/FS-1-MANIFEST-EXECUTABILITY-v1.3-CANDIDATE.md"
);
const POS_COV: &str =
    include_str!("../../../reports/decision-requests/FS-1-EXACTDEFICIT-POSITION-COVERAGE.md");
const SEED_COV: &str = include_str!("../../../reports/decision-requests/FS-1-IID-SEED-COVERAGE.md");
#[test]
fn executability_reports_consistency() {
    assert_eq!(EXEC_JSON, executability_report_json(), "exec .json drift");
    assert_eq!(EXEC_MD, executability_report_md(), "exec .md drift");
    assert_eq!(
        POS_COV,
        exact_deficit_position_coverage_md(),
        "position-coverage drift"
    );
    assert_eq!(SEED_COV, iid_seed_coverage_md(), "seed-coverage drift");
    assert!(EXEC_JSON.contains("\"dispatchable\": 240"));
}
