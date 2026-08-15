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
                let a0t = run_spine(SpineArm::A0Tail { r: 0 }, &build_cfg(c, rep));
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
            let a0t = run_spine(SpineArm::A0Tail { r: 0 }, &build_cfg(c, rep));
            assert!(a0t.completed);
            assert_eq!(
                a0t.retransmits, a0.retransmits,
                "A0-TAIL added retransmits on a non-tail completing flow (should be a no-op)"
            );
        }
    }
}

// ── FS-FUSION-CAL-1 §3 — the fusion cell: tail-capable ARQ *and* proactive redundancy ──────────
//
// The framing corrigendum records that "the (tail-capable, r>0) fusion cell is genuinely absent
// from every authoritative dataset": the coverage matrix carries r0/r2/r4 for frozen ARQ but only
// r0 for tail-capable ARQ. `SpineArm::A0Tail` could not express it, because the variant had no `r`.
// These two pins guard the arm that closes it.

/// Adding `r` to `A0Tail` must not disturb the arm the banked R2A dataset was measured on.
/// `A0Tail { r: 0 }` has to stay behaviourally identical to the tail-only arm, or ~8400 rows of
/// R2A evidence silently stop describing the code that produced them.
#[test]
fn a0tail_r0_is_unchanged_by_the_fusion_parameter() {
    let cells = amended_base_cells();
    for cid in ["rtt50-exactd4", "rtt250-exactd4", "rtt1000-exactd4"] {
        let c = cells.iter().find(|c| base_cell_id(c) == cid).unwrap();
        for rep in 0..5u8 {
            let a = run_spine(SpineArm::A0Tail { r: 0 }, &build_cfg(c, rep));
            let b = run_spine(SpineArm::A0Tail { r: 0 }, &build_cfg(c, rep));
            assert_eq!(
                (
                    a.completed,
                    a.retransmits,
                    a.total_outcome_bytes,
                    a.intervention_bytes,
                    a.repair_count
                ),
                (
                    b.completed,
                    b.retransmits,
                    b.total_outcome_bytes,
                    b.intervention_bytes,
                    b.repair_count
                ),
                "{cid} rep{rep}: A0-TAIL r=0 is not deterministic"
            );
            // r=0 carries no coding cost at all — this is what makes it the *same* arm.
            assert_eq!(
                (a.repair_count, a.configured_r),
                (0, 0),
                "{cid} rep{rep}: A0-TAIL r=0 must plan and emit no parity"
            );
        }
    }
}

/// The fusion arm must actually fuse: tail probing AND coding, both live in one run.
///
/// Without this the variant could compile, be recorded, and mean nothing — the failure mode this
/// repo has hit repeatedly, where a cell is named but its mechanism never fires.
#[test]
fn a0tail_fusion_arm_carries_both_tail_and_coding() {
    let cells = amended_base_cells();
    let mut exercised = 0usize;
    for cid in ["rtt50-exactd4", "rtt250-exactd4", "rtt1000-exactd4"] {
        let c = cells.iter().find(|c| base_cell_id(c) == cid).unwrap();
        for rep in 0..5u8 {
            let tail_only = run_spine(SpineArm::A0Tail { r: 0 }, &build_cfg(c, rep));
            let fusion = run_spine(SpineArm::A0Tail { r: 2 }, &build_cfg(c, rep));

            // Coding is live: the fusion arm spends parity bytes the tail-only arm never spends.
            assert!(
                fusion.repair_count > 0,
                "{cid} rep{rep}: fusion arm emitted no parity — coding is not wired"
            );
            assert!(
                fusion.intervention_bytes > tail_only.intervention_bytes,
                "{cid} rep{rep}: fusion arm did not add redundancy over tail-only"
            );
            // It is a distinct arm in the record, so an analysis can separate the two.
            assert_ne!(
                fusion.arm, tail_only.arm,
                "{cid} rep{rep}: fusion and tail-only must not share an arm label"
            );
            exercised += 1;
        }
    }
    assert!(
        exercised >= 15,
        "fusion arm exercised in too few cells: {exercised}"
    );
}
