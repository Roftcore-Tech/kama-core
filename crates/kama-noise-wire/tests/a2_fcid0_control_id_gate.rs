//! F-CID-0 — ControlIdIssueGate + CAL-10 dedup-window KABUL-PİNLERİ (Abkarada §8).

#![cfg(feature = "experimental-cold-start")]

use kama_noise_wire::{
    ControlIdIssueError, ControlIdIssueGate, CONTROL_DEDUP_WINDOW, MAX_CONTROL_ID_BURST,
    MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION, MAX_CONTROL_REORDER_ALLOWANCE,
    MAX_RETRY_REORDER_HORIZON_S,
};

/// A-3: issue() MONOTON control_id atar (0,1,2,...).
#[test]
fn fcid0_issue_monotonic() {
    let mut g = ControlIdIssueGate::new();
    for expected in 0..10u32 {
        assert_eq!(g.issue().unwrap(), expected);
    }
    assert_eq!(g.next_id(), 10);
}

/// BURST-enforce: tek-batch MAX_CONTROL_ID_BURST'ü aşarsa fail-closed; end_batch reset.
#[test]
fn fcid0_burst_enforced_and_reset() {
    let mut g = ControlIdIssueGate::new();
    for _ in 0..MAX_CONTROL_ID_BURST {
        g.issue().unwrap();
    }
    assert_eq!(g.issue(), Err(ControlIdIssueError::BurstExceeded));
    // Yeni batch: burst reset → tekrar issue eder (monotonluk korunur).
    g.end_batch();
    let id = g.issue().unwrap();
    assert_eq!(
        id, MAX_CONTROL_ID_BURST,
        "monotonluk batch-sınırında bozuldu"
    );
}

/// WRAP → RenewalRequired (§2.57 wrap-öncesi-session-renewal; test-wrap-eşiği).
#[test]
fn fcid0_wrap_triggers_renewal() {
    let mut g = ControlIdIssueGate::with_wrap_threshold(3);
    assert_eq!(g.issue().unwrap(), 0);
    assert_eq!(g.issue().unwrap(), 1);
    assert_eq!(g.issue().unwrap(), 2);
    // next_id=3 = wrap_threshold → RenewalRequired.
    assert_eq!(g.issue(), Err(ControlIdIssueError::RenewalRequired));
}

/// CAL-10 dedup-window: required = RATE×HORIZON + BURST + REORDER; window = next-pow2 = 4096.
#[test]
fn cal10_dedup_window_derivation() {
    let required = MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION * MAX_RETRY_REORDER_HORIZON_S
        + MAX_CONTROL_ID_BURST
        + MAX_CONTROL_REORDER_ALLOWANCE;
    assert_eq!(required, 32 * 105 + 64 + 16); // 3440
    assert_eq!(required, 3440);
    assert!(
        CONTROL_DEDUP_WINDOW >= required,
        "window required'ı karşılamıyor"
    );
    assert_eq!(
        CONTROL_DEDUP_WINDOW, 4096,
        "freeze-r2 CANDIDATE 4096 doğrulanmadı"
    );
    assert!(CONTROL_DEDUP_WINDOW.is_power_of_two());
}
