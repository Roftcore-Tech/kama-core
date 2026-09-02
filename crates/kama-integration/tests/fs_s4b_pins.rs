//! FINAL BUILD S4-B — FS-1 candidate manifest (240 row) validators + pairing groups + 4-hash uniqueness.
//! DIAGNOSTIC (result-bearing-DEĞİL; v1.3-CANDIDATE-NOT-FROZEN). Generator-level validation (JSON-parse-YOK).
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_manifest.rs"]
mod fs_manifest;
use fs_manifest::*;
use std::collections::BTreeSet;

fn all_rows() -> Vec<(BaseCell, &'static str, u8)> {
    let mut v = Vec::new();
    for c in amended_base_cells() {
        for arm in FS1_RESULT_BEARING_ARMS {
            for rep in 0..FS1_REPS {
                v.push((c, arm, rep));
            }
        }
    }
    v
}

// ── PIN 1: row count + family + arm breakdown (§13/§17) ────────────────────────────────────────────
#[test]
fn manifest_row_and_family_counts() {
    let rows = all_rows();
    assert_eq!(rows.len(), 240, "12 × 4 × 5 = 240");
    assert_eq!(manifest_row_count(), 240, "JSONL satır-sayısı 240");
    let ed = rows
        .iter()
        .filter(|(c, _, _)| c.impairment.kind() == "ExactDeficit")
        .count();
    let iid = rows
        .iter()
        .filter(|(c, _, _)| c.impairment.kind() == "NaturalIid")
        .count();
    assert_eq!(ed, 180, "ExactDeficit 9×4×5 = 180");
    assert_eq!(iid, 60, "NaturalIid 3×4×5 = 60");
    for arm in FS1_RESULT_BEARING_ARMS {
        let n = rows.iter().filter(|(_, a, _)| *a == arm).count();
        assert_eq!(n, 60, "{arm}: 12×5 = 60");
    }
    // F1/F4 manifest'te YOK.
    assert!(
        !rows.iter().any(|(_, a, _)| *a == "F1" || *a == "F4"),
        "F1/F4 result-bearing-DIŞI"
    );
}

// ── PIN 2: unique run-id + row-hash + exec-hash (§15/§17) ───────────────────────────────────────────
#[test]
fn manifest_unique_ids_and_hashes() {
    let rows = all_rows();
    let mut run_ids = BTreeSet::new();
    let mut row_hs = BTreeSet::new();
    let mut exec_hs = BTreeSet::new();
    let mut base_hs = BTreeSet::new();
    for (c, arm, rep) in &rows {
        let h = row_hashes(c, arm, *rep);
        run_ids.insert(h.run_id.clone());
        row_hs.insert(h.row);
        exec_hs.insert(h.exec);
        base_hs.insert(h.base);
    }
    assert_eq!(run_ids.len(), 240, "240 unique run-id (collision-YOK)");
    assert_eq!(row_hs.len(), 240, "240 unique row_identity_hash");
    assert_eq!(
        exec_hs.len(),
        240,
        "240 unique run_execution_semantics_hash (identical-row-dup = 0)"
    );
    assert_eq!(base_hs.len(), 12, "12 unique base_cell_semantics_hash");
}

// ── PIN 3: pairing groups (§16) ────────────────────────────────────────────────────────────────────
#[test]
fn manifest_pairing_groups() {
    let mut groups: std::collections::BTreeMap<String, Vec<&'static str>> = Default::default();
    for c in amended_base_cells() {
        for arm in FS1_RESULT_BEARING_ARMS {
            for rep in 0..FS1_REPS {
                groups
                    .entry(pairing_group_id(&c, rep))
                    .or_default()
                    .push(arm);
            }
        }
    }
    assert_eq!(groups.len(), 60, "12 cell × 5 rep = 60 pairing-group");
    for (gid, arms) in &groups {
        assert_eq!(arms.len(), 4, "{gid}: 4 arm/group");
        let uniq: BTreeSet<_> = arms.iter().collect();
        assert_eq!(uniq.len(), 4, "{gid}: duplicate-arm YOK");
        for expect in FS1_RESULT_BEARING_ARMS {
            assert!(
                arms.contains(&expect),
                "{gid}: {expect} eksik (incomplete group)"
            );
        }
    }
}

// ── PIN 4: NO cross-arm behavioral collapse (§18-B; RULE-11) — OPT-A kanıtı ─────────────────────────
#[test]
fn manifest_no_behavioral_collapse() {
    // her pairing-group içinde 4 arm → 4 DISTINCT resolved_behavior_semantics_hash (collapse-YOK).
    for c in amended_base_cells() {
        for rep in 0..FS1_REPS {
            let behav: BTreeSet<u64> = FS1_RESULT_BEARING_ARMS
                .iter()
                .map(|arm| row_hashes(&c, arm, rep).behav)
                .collect();
            assert_eq!(
                behav.len(),
                4,
                "{}: pairing-group'ta behavioral-collapse (OPT-A ihlal)",
                pairing_group_id(&c, rep)
            );
        }
    }
    // global: 240 satır → 240 distinct resolved_behavior (hiçbir collapse; presentation-only-dup = 0).
    let all: BTreeSet<u64> = all_rows()
        .iter()
        .map(|(c, a, r)| row_hashes(c, a, *r).behav)
        .collect();
    assert_eq!(
        all.len(),
        240,
        "240 distinct resolved_behavior (collapse = 0)"
    );
}

// ── PIN 5: impairment validity + deadlines + pacing (§17) ───────────────────────────────────────────
#[test]
fn manifest_impairment_deadline_pacing() {
    for (c, _arm, _rep) in all_rows() {
        match c.impairment {
            FsImpairment::ExactDeficit { drop_count } => {
                assert!(matches!(drop_count.get(), 1 | 2 | 4));
                assert_eq!(
                    c.impairment.configured_iid_loss_permille(),
                    None,
                    "ED → iid N/A"
                );
            }
            FsImpairment::NaturalIid { loss_permille } => {
                assert_eq!(loss_permille.get(), 200);
                assert_eq!(
                    c.impairment.configured_exact_drop_count(),
                    None,
                    "IID → deficit N/A"
                );
            }
        }
        assert_eq!(c.bottleneck_bps, 1_000_000);
        assert_eq!(c.source_offer_pacer_bps, 800_000);
    }
    assert_eq!(deadline_ms_for(50), 199);
    assert_eq!(deadline_ms_for(250), 599);
    assert_eq!(deadline_ms_for(1000), 2099);
    assert_eq!(
        source_offer_envelope_ns(),
        48_480_000,
        "0.80C envelope 48.48ms"
    );
}

// ── PIN 6: regeneration determinism (§24) ──────────────────────────────────────────────────────────
#[test]
fn manifest_regeneration_deterministic() {
    assert_eq!(
        manifest_jsonl(),
        manifest_jsonl(),
        "manifest byte-deterministik (regen)"
    );
    // her satır tam-bir-JSON-obje + result_bearing:false (firewall).
    for line in manifest_jsonl().lines() {
        assert!(line.starts_with('{') && line.ends_with('}'));
        assert!(
            line.contains("\"result_bearing\":false"),
            "result-bearing-firewall"
        );
        assert!(line.contains("\"prereg_status\":\"CANDIDATE_NOT_FROZEN\""));
    }
}

// ── PIN 7: committed artifact == generator (read-only; drift-YOK) ──────────────────────────────────
const COMMITTED_MANIFEST: &str =
    include_str!("../../../reports/decision-requests/FS-1-MANIFEST-v1.3-CANDIDATE.jsonl");
#[test]
fn manifest_artifact_consistency() {
    assert_eq!(
        COMMITTED_MANIFEST,
        manifest_jsonl(),
        "committed FS-1-MANIFEST-v1.3-CANDIDATE.jsonl generator'la UYUŞMUYOR (regenerate)"
    );
    assert_eq!(COMMITTED_MANIFEST.lines().count(), 240);
    // F1/F4 committed-manifest'te HİÇBİR satırda YOK.
    assert!(!COMMITTED_MANIFEST.contains("--F1--") && !COMMITTED_MANIFEST.contains("--F4--"));
    assert!(
        !COMMITTED_MANIFEST.contains("\"arm\":\"F1\"")
            && !COMMITTED_MANIFEST.contains("\"arm\":\"F4\"")
    );
}
