//! DR-ARQ-TAIL-1 §3 — integration: A0-TAIL recovers a tail erasure that frozen A0 censors (bounded, non-spurious),
//! and is a no-op on non-tail / no-loss flows (regression). Deconfounding-arm mechanism check (separate from FS-1).
#![cfg(feature = "experimental-fs-harness")]
#![allow(clippy::duplicate_mod)]

#[path = "common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;

#[test]
fn a0tail_recovers_tail_erasure_that_a0_censors() {
    let cells = amended_base_cells();
    // ExactDeficit d=4 cells put erasures late enough to include the terminal id in some reps → A0 tail-stall.
    let mut found = false;
    for cid in ["rtt50-exactd4", "rtt250-exactd4", "rtt1000-exactd4"] {
        let c = cells.iter().find(|c| base_cell_id(c) == cid).unwrap();
        for rep in 0..5u8 {
            let a0 = run_spine(SpineArm::A0, &build_cfg(c, rep));
            // only interested in reps where A0 fails to complete AND the terminal id 47 was erased (true tail).
            if !a0.completed && a0.realized_initial_source_drop_ids.contains(&47) {
                let a0t = run_spine(SpineArm::A0Tail, &build_cfg(c, rep));
                assert!(
                    a0t.completed,
                    "{cid} rep{rep}: A0-TAIL failed to recover a real tail erasure that A0 censored"
                );
                // recovery used ARQ retransmits (bounded); the completing cause is ARQ.
                assert!(
                    a0t.retransmits > a0.retransmits,
                    "tail probe produced no retransmit"
                );
                found = true;
            }
        }
    }
    assert!(
        found,
        "no real tail-erasure (terminal-id) A0-censored case found to exercise the probe"
    );
}

#[test]
fn a0tail_is_noop_on_no_loss_and_matches_a0_when_no_tail() {
    let cells = amended_base_cells();
    // a cell/rep where A0 already completes (no tail loss) → A0-TAIL must match A0 exactly (no extra retransmit).
    let c = cells
        .iter()
        .find(|c| base_cell_id(c) == "rtt50-exactd1")
        .unwrap();
    for rep in 0..5u8 {
        let a0 = run_spine(SpineArm::A0, &build_cfg(c, rep));
        if a0.completed && !a0.realized_initial_source_drop_ids.contains(&47) {
            let a0t = run_spine(SpineArm::A0Tail, &build_cfg(c, rep));
            assert!(a0t.completed);
            assert_eq!(
                a0t.retransmits, a0.retransmits,
                "A0-TAIL added retransmits on a non-tail completing flow (should be a no-op)"
            );
        }
    }
}
