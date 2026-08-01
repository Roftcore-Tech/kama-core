//! FINAL BUILD S3-R3 — common result-schema completion (§25) + deadline-candidate eligibility (§17).
//! DIAGNOSTIC (result-bearing-DEĞİL; manifest-DIŞI). run_spine değişiklikleri ADDITIVE (S1/S2/R0/R1/R2 yeşil).
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_spine.rs"]
mod fs_spine;
use adaptive_transport::fs_harness::{
    fs_deadline_ms, fs_effective_source_bps, fs_t_serialize_source_ns,
};
use fs_spine::*;

const LINK: u64 = 1_000_000;
const OWD: u64 = 20 * MS;

// ── PIN 1: COMMON_RESULT_SCHEMA_S3 (§25; typed None/N-A, sahte-sıfır YOK) ───────────────────────────
#[test]
fn common_result_schema_s3() {
    // ExactDeficit run: impairment-variant + realized-loss + reverse-event + pairing alanları.
    let ed = run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[2, 5]));
    assert_eq!(ed.impairment_kind, "ExactDeficit");
    assert_eq!(
        ed.configured_exact_drop_count,
        Some(2),
        "ExactDeficit → configured drop-count"
    );
    assert_eq!(
        ed.configured_iid_loss_permille, None,
        "ExactDeficit → iid N/A (sahte-sıfır YOK)"
    );
    assert_eq!(
        ed.realized_initial_source_losses, 2,
        "realized InitialSource-loss == |D|"
    );
    assert_eq!(
        ed.realized_retransmission_losses, 0,
        "ExactDeficit retransmit'i düşürmez"
    );
    assert_eq!(
        ed.realized_repair_losses, 0,
        "ExactDeficit repair'i düşürmez"
    );
    assert_eq!(ed.realized_duplicate_losses, 0);
    assert_eq!(
        ed.reverse_event_hash, ed.reverse_trace_hash,
        "reverse_event_hash = realized reverse-log"
    );
    assert!(ed.pairing_gate_passed, "admitted → pairing-gate passed");
    assert_eq!(ed.pairing_failure_reason, None);

    // NaturalIid run: variant-local flip (exact N/A, iid Some(200)); realized InitialSource-loss > 0.
    let iid = run_spine(
        SpineArm::A1 { r: 2 },
        &SpineCfg::iid(LINK, OWD, 200, 0xABCD).with_horizon(3000 * MS),
    );
    assert_eq!(iid.impairment_kind, "NaturalIid");
    assert_eq!(
        iid.configured_exact_drop_count, None,
        "NaturalIid → deficit N/A (sahte-sıfır YOK)"
    );
    assert_eq!(iid.configured_iid_loss_permille, Some(200));
    assert!(
        iid.realized_initial_source_losses > 0,
        "IID bazı InitialSource'ları düşürdü"
    );
    // realized per-role loss alanları POPULATED (IID → retransmit/dup da düşebilir; ≥0, typed).
    let total_role_losses = iid.realized_initial_source_losses as u64
        + iid.realized_retransmission_losses
        + iid.realized_duplicate_losses
        + iid.realized_repair_losses;
    assert!(total_role_losses >= iid.realized_initial_source_losses as u64);
}

// ── PIN 2: DEADLINE_CANDIDATE_ELIGIBILITY (§17; v1.3-freeze YOK — yalnız eligible-candidate) ─────────
#[test]
fn deadline_candidate_eligibility() {
    let total_source = 48u64 * 101; // 4848 B
    let eff = fs_effective_source_bps(800_000, 1_000_000); // 0.80C = 800000 bps
    assert_eq!(eff, 800_000);
    assert_eq!(total_source, 4848, "common source bytes");
    // effective source rate = 100000 B/s (= 800000 bps).
    assert_eq!(eff / 8, 100_000, "effective source rate = 100000 B/s");
    // source serialization = 48.48 ms.
    assert_eq!(
        fs_t_serialize_source_ns(total_source, eff),
        48_480_000,
        "serialization 48.48 ms"
    );
    // candidate deadlines (ELIGIBLE; v1.3'te DONDURULMADI).
    assert_eq!(
        fs_deadline_ms(total_source, eff, 50),
        199,
        "RTT50 → 199 ms candidate"
    );
    assert_eq!(
        fs_deadline_ms(total_source, eff, 250),
        599,
        "RTT250 → 599 ms candidate"
    );
    assert_eq!(
        fs_deadline_ms(total_source, eff, 1000),
        2099,
        "RTT1000 → 2099 ms candidate"
    );
    assert_eq!(
        fs_deadline_ms(total_source, eff, 2500),
        5099,
        "future RTT2500 → 5099 ms candidate"
    );
}
