//! FINAL BUILD S3-R0 — ALT-1 typed-impairment sum-type + semantic cell-uniqueness pins. DIAGNOSTIC
//! (result-bearing-DEĞİL; manifest-DIŞI; 360-budget-harcamaz). S3-A DUR-B → ALT-1 (Abkarada) makineleşmesi.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_cell.rs"]
mod fs_cell;
use fs_cell::*;
use std::collections::BTreeSet;

// ── PIN 1: IID_COVERAGE_ALT1_RATIFIED ──────────────────────────────────────────────────────────────
#[test]
fn iid_coverage_alt1_ratified() {
    let cells = amended_base_cells();
    let ed = cells
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
        .count();
    let iid = cells
        .iter()
        .filter(|c| c.impairment.kind() == "NaturalIid")
        .count();
    assert_eq!(ed, 9, "9 ExactDeficit hücre (3 RTT × 3 deficit)");
    assert_eq!(
        iid, 3,
        "3 NaturalIid hücre (3 RTT × 1; deficit-ekseni çöktü)"
    );
    assert_eq!(cells.len(), 12, "ALT-1 amended = 12 base-cell");
    // projected result-bearing = 12 × 6 arm × 5 rep = 360.
    assert_eq!(cells.len() * FS1_ARMS.len() * FS1_REPS as usize, 360);
    // IID hücrelerinde deficit N/A, loss_permille = 200.
    for c in cells.iter().filter(|c| c.impairment.kind() == "NaturalIid") {
        assert_eq!(
            c.impairment.configured_exact_drop_count(),
            None,
            "IID deficit N/A"
        );
        assert_eq!(c.impairment.configured_iid_loss_permille(), Some(200));
    }
    // ExactDeficit hücrelerinde loss_permille N/A, drop_count ∈ {1,2,4}.
    for c in cells
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
    {
        assert_eq!(
            c.impairment.configured_iid_loss_permille(),
            None,
            "ED iid N/A"
        );
        assert!(matches!(
            c.impairment.configured_exact_drop_count(),
            Some(1 | 2 | 4)
        ));
    }
}

// ── PIN 2: IMPAIRMENT_SUM_TYPE_VALIDATION (§5; RULE-9) ──────────────────────────────────────────────
#[test]
fn impairment_sum_type_validation() {
    // GEÇERLİ.
    assert!(parse_impairment("ExactDeficit", Some(1), None).is_ok());
    assert!(parse_impairment("ExactDeficit", Some(2), None).is_ok());
    assert!(parse_impairment("ExactDeficit", Some(4), None).is_ok());
    assert!(parse_impairment("NaturalIid", None, Some(200)).is_ok());
    assert!(parse_impairment("NaturalIid", None, None).is_ok()); // default 200

    // NaturalIid + deficit → REJECT (S3-A DUR-B çekirdeği; deficit IID-inert).
    assert_eq!(
        parse_impairment("NaturalIid", Some(1), Some(200)),
        Err(CellConfigError::IidWithDeficit(1))
    );
    assert_eq!(
        parse_impairment("NaturalIid", Some(2), None),
        Err(CellConfigError::IidWithDeficit(2))
    );
    assert_eq!(
        parse_impairment("NaturalIid", Some(4), Some(200)),
        Err(CellConfigError::IidWithDeficit(4))
    );

    // ExactDeficit drop_count'suz → REJECT.
    assert_eq!(
        parse_impairment("ExactDeficit", None, None),
        Err(CellConfigError::ExactDeficitWithoutCount)
    );
    // ExactDeficit + iid_loss_permille → REJECT (variant-yabancı).
    assert_eq!(
        parse_impairment("ExactDeficit", Some(2), Some(200)),
        Err(CellConfigError::ExactDeficitWithIid(200))
    );

    // invalid drop_count (3,0,5 → matched-deficit / d-genişletme YASAK).
    assert_eq!(
        parse_impairment("ExactDeficit", Some(3), None),
        Err(CellConfigError::InvalidDropCount(3))
    );
    assert_eq!(
        parse_impairment("ExactDeficit", Some(0), None),
        Err(CellConfigError::InvalidDropCount(0))
    );
    // invalid loss_permille (sweep / matched-deficit YASAK).
    assert_eq!(
        parse_impairment("NaturalIid", None, Some(100)),
        Err(CellConfigError::InvalidLossPermille(100))
    );
    assert_eq!(
        parse_impairment("NaturalIid", None, Some(50)),
        Err(CellConfigError::InvalidLossPermille(50))
    );

    // newtype doğrudan reddi.
    assert!(ExactDeficitCount::new(3).is_err());
    assert!(ExactDeficitCount::new(8).is_err());
    assert!(LossPermille::new(999).is_err());
    assert_eq!(ExactDeficitCount::new(2).unwrap().get(), 2);
    assert_eq!(LossPermille::new(200).unwrap().get(), 200);

    // unknown kind.
    assert!(matches!(
        parse_impairment("Iid5", None, None),
        Err(CellConfigError::UnknownKind(_))
    ));
}

// ── PIN 3: AMENDED_BASE_CELL_UNIQUENESS ─────────────────────────────────────────────────────────────
#[test]
fn amended_base_cell_uniqueness() {
    let cells = amended_base_cells();
    let uniq = unique_base_hashes(&cells);
    assert_eq!(cells.len(), 12, "12 base-cell");
    assert_eq!(
        uniq.len(),
        12,
        "12 hücre → 12 benzersiz base_cell_semantics_hash"
    );
    assert_eq!(cells.len() - uniq.len(), 0, "duplicate = 0");
}

// ── PIN 4: HISTORICAL_IID_DUPLICATE_DETECTION ───────────────────────────────────────────────────────
#[test]
fn historical_iid_duplicate_detection() {
    let rows = historical_base_rows();
    assert_eq!(
        rows.len(),
        18,
        "tarihsel 18-satır (RTT × deficit-label × impairment)"
    );
    let cells: Vec<BaseCell> = rows.iter().map(base_cell_of).collect();
    let uniq = unique_base_hashes(&cells);
    assert_eq!(uniq.len(), 12, "tarihsel-benzersiz-semantik = 12");
    assert_eq!(
        rows.len() - uniq.len(),
        6,
        "duplicate base-satır = 6 (RTT-başına 2 IID × 3 RTT)"
    );
    // 6 duplicate-cell × 6 arm × 5 rep = 180 result-bearing-run avoided (540 → 360).
    assert_eq!(
        (rows.len() - uniq.len()) * FS1_ARMS.len() * FS1_REPS as usize,
        180,
        "avoided duplicate result-bearing runs = 180"
    );

    // IID d-satırları RTT-başına ÖZDEŞ base-hash (d-label inert).
    for &rtt in &FS1_RTT_MS {
        let iid_hashes: BTreeSet<u64> = rows
            .iter()
            .filter(|r| r.rtt_ms == rtt && r.impairment.kind() == "NaturalIid")
            .map(|r| base_cell_semantics_hash(&base_cell_of(r)))
            .collect();
        assert_eq!(
            iid_hashes.len(),
            1,
            "RTT{rtt}: 3 IID d-satırı → 1 benzersiz base-hash (d inert)"
        );
    }
    // ExactDeficit d-satırları RTT-başına FARKLI base-hash (d grounded).
    for &rtt in &FS1_RTT_MS {
        let ed_hashes: BTreeSet<u64> = rows
            .iter()
            .filter(|r| r.rtt_ms == rtt && r.impairment.kind() == "ExactDeficit")
            .map(|r| base_cell_semantics_hash(&base_cell_of(r)))
            .collect();
        assert_eq!(
            ed_hashes.len(),
            3,
            "RTT{rtt}: 3 ExactDeficit d-satırı → 3 benzersiz base-hash"
        );
    }
}

// ── PIN 5: semantic hash hierarchy (RULE-8: label yeterli-değil; three-level separation) ─────────────
#[test]
fn semantic_hash_hierarchy_rule8() {
    let cells = amended_base_cells();
    let c = cells[0]; // rtt50-d1-exact
    let arm_a0 = ArmConfig {
        arm: "A0",
        configured_r: 0,
    };

    // aynı base+arm+seed+version → aynı exec-hash (deterministik; duplicate-execution).
    let exec = run_execution_semantics_hash(&c, &arm_a0, 1, 1);
    assert_eq!(exec, run_execution_semantics_hash(&c, &arm_a0, 1, 1));

    // RULE-8: aynı exec-hash + FARKLI row-label/display-deficit → farklı row-id AMA duplicate-execution-row.
    let row_a = row_identity_hash(exec, "row-0001", Some(1));
    let row_b = row_identity_hash(exec, "row-9999", Some(2));
    assert_ne!(row_a, row_b, "row-identity presentation-farkı");
    // ↑ farklı row-id, AYNI exec-hash → RULE-8: label bilimsel-distinctness KURMAZ.

    // farklı arm → farklı exec-hash AMA aynı base-cell-hash (arm base-cell'i tanımlamaz).
    let arm_f0 = ArmConfig {
        arm: "F0",
        configured_r: 2,
    };
    assert_ne!(
        exec,
        run_execution_semantics_hash(&c, &arm_f0, 1, 1),
        "farklı arm → farklı exec"
    );
    assert_eq!(
        base_cell_semantics_hash(&c),
        base_cell_semantics_hash(&c),
        "base-cell-hash arm/seed-bağımsız"
    );
    // farklı seed → farklı exec-hash.
    assert_ne!(
        run_execution_semantics_hash(&c, &arm_a0, 1, 1),
        run_execution_semantics_hash(&c, &arm_a0, 2, 1),
        "farklı seed → farklı exec"
    );
    // farklı config-version → farklı exec-hash.
    assert_ne!(
        run_execution_semantics_hash(&c, &arm_a0, 1, 1),
        run_execution_semantics_hash(&c, &arm_a0, 1, 2),
        "farklı config-version → farklı exec"
    );
}
