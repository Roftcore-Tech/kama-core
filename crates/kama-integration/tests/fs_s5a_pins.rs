//! FINAL BUILD S5-A — observation-horizon (frozen v1.1 §B0.3) + execution-order authority. DIAGNOSTIC.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_manifest.rs"]
mod fs_manifest;
use fs_manifest::*;
use std::collections::BTreeSet;

// ── PIN 1: observation horizon (§4; grounded, arm-independent, > deadline) ──────────────────────────
#[test]
fn s5a_observation_horizon() {
    // frozen v1.1 §B0.3: 249 + 9×RTT.
    assert_eq!(observation_horizon_ms(50), 699);
    assert_eq!(observation_horizon_ms(250), 2499);
    assert_eq!(observation_horizon_ms(1000), 9249);
    // horizon > deadline (deadline horizon DEĞİL).
    for rtt in [50u64, 250, 1000] {
        assert!(
            observation_horizon_ms(rtt) > deadline_ms_for(rtt),
            "horizon > deadline (RTT{rtt})"
        );
    }
    // arm-independent (fonksiyon yalnız RTT alır — arm-girdisi YOK; yapısal).
    // 5099 FS-1-horizon-değeri DEĞİL (RTT2500 kapsam-dışı; hiçbir FS-1 RTT → 5099).
    for rtt in [50u64, 250, 1000] {
        assert_ne!(observation_horizon_ms(rtt), 5099);
        assert_ne!(deadline_ms_for(rtt), 5099);
    }
}

// ── PIN 2: execution order (§7; deterministic; every row once; operational-metadata) ───────────────
#[test]
fn s5a_execution_order() {
    let e = execution_order_entries();
    assert_eq!(e.len(), 240, "240 execution-order entry");
    // 60 distinct pairing-group.
    let groups: BTreeSet<String> = e.iter().map(|(_, g, _, _, _, _)| g.clone()).collect();
    assert_eq!(groups.len(), 60, "60 pairing-group");
    // her run_id tam-bir-kez (240 unique).
    let rids: BTreeSet<String> = e.iter().map(|(_, _, _, _, _, r)| r.clone()).collect();
    assert_eq!(rids.len(), 240, "240 unique run_id (her-satır bir-kez)");
    // order_index 0..239 ardışık.
    for (i, entry) in e.iter().enumerate() {
        assert_eq!(entry.0, i, "order_index ardışık");
    }
    // deterministik (2× özdeş).
    let e2 = execution_order_entries();
    let rids2: Vec<String> = e2.iter().map(|(_, _, _, _, _, r)| r.clone()).collect();
    let rids1: Vec<String> = e.iter().map(|(_, _, _, _, _, r)| r.clone()).collect();
    assert_eq!(rids1, rids2, "execution-order deterministik");
    // always-A0-first DEĞİL (arm-permütasyonu gerçek): en-az-bir-grupta ilk-arm ≠ A0.
    let first_arms: BTreeSet<&str> = (0..60).map(|g| e[g * 4].4).collect();
    assert!(
        first_arms.len() > 1 || first_arms.iter().next() != Some(&"A0"),
        "arm-order permuted (not always-A0-first)"
    );
}

// ── PIN 3: execution order covers manifest exactly ─────────────────────────────────────────────────
#[test]
fn s5a_execution_order_covers_manifest() {
    let order_rids: BTreeSet<String> = execution_order_entries()
        .iter()
        .map(|(_, _, _, _, _, r)| r.clone())
        .collect();
    let mut manifest_rids = BTreeSet::new();
    for c in amended_base_cells() {
        for arm in FS1_RESULT_BEARING_ARMS {
            for rep in 0..FS1_REPS {
                manifest_rids.insert(row_hashes(&c, arm, rep).run_id);
            }
        }
    }
    assert_eq!(
        order_rids, manifest_rids,
        "execution-order run_id-seti = manifest run_id-seti (240)"
    );
}

// ── PIN 4: execution-order.json consistency (read-only; drift-YOK) ──────────────────────────────────
const EXEC_ORDER_JSON: &str =
    include_str!("../../../reports/decision-requests/FS-1-EXECUTION-ORDER-v1.json");
#[test]
fn s5a_execution_order_artifact_consistency() {
    assert_eq!(
        EXEC_ORDER_JSON,
        execution_order_json(),
        "execution-order.json drift"
    );
    assert!(EXEC_ORDER_JSON.contains("\"total_rows\": 240"));
    assert!(EXEC_ORDER_JSON.contains("\"pairing_groups\": 60"));
}
