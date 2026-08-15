//! FS-FUSION-CAL-1 Q18 — regression pin for a CONFIRMED calibration defect.
//!
//! FS-FUSION-CELL-1 measured, at exactly 20% IID loss, that tail-capable ARQ completes 0/12 while
//! adding coding completes 9/12. So 200_000 ppm is precisely where FEC stops being optional.
//!
//! The shipped fixture puts the loss-LOW shoulder at `shoulder_low(200_000, 300_000)`
//! (`fusion.rs`), i.e. `Trap { a: 0, b: 0, c: 200_000, d: 300_000 }`, so membership in LOW is a
//! full 1.0 all the way to 200_000. The controller therefore reads 20% loss as entirely LOW and
//! plans zero parity — in the one regime the measurement says coding is the whole mechanism.
//!
//! This pin does not fix that. It records the boundary so it cannot move unnoticed, and so a
//! future recalibration has a test that fails when the defect is actually repaired.
use adaptive_transport::fusion::*;

fn at(loss_ppm: u64, bandwidth_bps: u64) -> FusionDecision {
    DeterministicFuzzyController::fixture().decide(&FusionInputs {
        loss_ppm,
        bandwidth_bps,
        jitter_ppm: 100,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    })
}

#[test]
fn q18_shipped_controller_plans_zero_parity_where_fec_is_the_whole_mechanism() {
    // The measured regime: FS-FUSION-CELL-1 rtt50/rtt250/rtt1000-iid20, ~125 kB/s.
    let d = at(200_000, 125_000);
    assert_eq!(
        d.fec_parity_ratio_permille, 0,
        "Q18 DEFECT CHANGED: the controller now plans parity at 20% loss. If this is a deliberate \
         recalibration, update this pin and cite the campaign that justifies the new boundary."
    );

    // Not a bandwidth artefact — raising bw into the R2 HIGH region does not open FEC either.
    assert_eq!(at(200_000, 175_000).fec_parity_ratio_permille, 0);

    // The knee really is just above: 25% opens coding, 20% does not.
    assert!(
        at(250_000, 125_000).fec_parity_ratio_permille > 0,
        "the LOW shoulder should end by 250_000 ppm"
    );

    // The defect is a boundary placement, not a dead rule: the mechanism works further out.
    assert!(at(500_000, 125_000).fec_parity_ratio_permille > 0);
}
