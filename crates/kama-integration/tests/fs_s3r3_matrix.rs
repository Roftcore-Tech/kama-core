//! FINAL BUILD S3-R3 — candidate 12-cell matrix artifact validation (§26). DIAGNOSTIC (v1.3-CANDIDATE;
//! NON-FROZEN; result-bearing-DEĞİL; 360 result-row ÜRETİLMEZ; S4-manifest DEĞİL). Committed artifact
//! (FS-1-CELL-MATRIX-v1.3-CANDIDATE.json) = candidate_matrix_json() deterministik çıktısı (read-only pin).
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_cell.rs"]
mod fs_cell;
use fs_cell::*;

const COMMITTED: &str =
    include_str!("../../../reports/decision-requests/FS-1-CELL-MATRIX-v1.3-CANDIDATE.json");

// ── PIN: candidate-matrix artifact consistency + §26 validation ────────────────────────────────────
#[test]
fn candidate_matrix_artifact() {
    // (1) committed artifact = deterministik generator çıktısı (drift-YOK).
    assert_eq!(
        COMMITTED,
        candidate_matrix_json(),
        "committed FS-1-CELL-MATRIX-v1.3-CANDIDATE.json fs_cell çıktısıyla UYUŞMUYOR (regenerate)"
    );
    // (2) §26 structural validation.
    let cells = amended_base_cells();
    assert_eq!(cells.len(), 12, "count = 12");
    assert_eq!(
        unique_base_hashes(&cells).len(),
        12,
        "unique base-semantics = 12"
    );
    assert_eq!(
        cells.len() - unique_base_hashes(&cells).len(),
        0,
        "duplicate = 0"
    );
    let ed = cells
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
        .count();
    let iid = cells
        .iter()
        .filter(|c| c.impairment.kind() == "NaturalIid")
        .count();
    assert_eq!(ed, 9, "9 ExactDeficit");
    assert_eq!(iid, 3, "3 NaturalIid");
    // (3) projected 360; NON-frozen; S4-manifest DEĞİL.
    assert!(COMMITTED.contains("\"projected_result_bearing_runs\": 360"));
    assert!(COMMITTED.contains("\"frozen\": false"));
    assert!(COMMITTED.contains("\"invalid_variant_combos\": 0"));
    // (4) hiçbir invalid-variant-combo: her hücre variant-local param taşır (§5/RULE-9).
    for c in &cells {
        match c.impairment {
            FsImpairment::ExactDeficit { drop_count } => {
                assert!(matches!(drop_count.get(), 1 | 2 | 4));
            }
            FsImpairment::NaturalIid { loss_permille } => {
                assert_eq!(loss_permille.get(), 200);
            }
        }
    }
}
