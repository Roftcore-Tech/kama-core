//! FS-CAL-2 v3 §3 estimator wiring — it must OBSERVE, and it must change NOTHING.
//!
//! Q17 (preregistered holdout) and FS-FUSION-CELL-1 both concluded that loss RATE cannot gate FEC
//! and loss STRUCTURE is the missing input. The estimator that computes structure existed but had
//! no product consumer. These pins guard the properties the wiring has to have, including the
//! lifecycle wired per docs/design/ESTIMATOR-LIFECYCLE.md (advance() at the engine clock).
use adaptive_transport::fate_estimator::{FateEstimator, FATE_WINDOW};
use adaptive_transport::sender::*;
use std::time::Duration;

const MS: u64 = 1_000_000;

fn feed(s: &mut Sender, units: u64) {
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
}

/// V1 — NON-VACUITY on a real lossy flow: the window must hold FINALIZED LOSSES, not merely fill.
///
/// The first version of this test asserted only `n > 0`, which successes alone satisfy — and that
/// is exactly the state the product was in while `advance()` had no caller (S-ANSWERS: every loss
/// stayed provisional forever, the window held successes only, `n10 + n11` was structurally zero).
/// A test written to prevent vacuous passes was itself vacuous. The binding assertion is therefore
/// on the LOSS-transition counters: they can only be non-zero if a loss was finalized into the
/// window AND sits adjacent to another fate-known id.
#[test]
fn estimator_observes_real_loss() {
    let mut s = Sender::new();
    feed(&mut s, 24);
    // Emit under the PreConverge floor, then CONVERGE the flow with an early ACK of seq 0: the RTT
    // sample establishes srtt, which exits PreConverge in BOTH builds. Without this the
    // experimental-cold-start build never converges under a timer-only drive, declares no loss,
    // and the strengthened assertion below is unsatisfiable there — the review caught exactly
    // that failure (n=1, window = one SUCCESS) on the first version of this drive.
    for t in 1..=20u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    // Converged: normal pacing emits the rest. Then ACK a sparse subset so the gapped originals
    // below the ACK horizon are declared lost.
    for t in 21..=40u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![2, 4, 6, 8],
            ack_delay: Duration::ZERO,
        }),
        9_000 * MS,
    );
    // Timer events flow through on_event_core, whose advance() finalizes the declared losses once
    // the reordering guard elapses — 18 s is far past any bounded-PTO guard this flow can hold.
    for t in 41..=90u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }

    let f = s.diag_loss_structure();
    assert!(
        f.n > 0,
        "estimator window is empty after a lossy flow: {f:?}"
    );
    assert!(
        f.n10 + f.n11 > 0,
        "no finalized loss transition after a lossy flow — losses are stuck provisional, the \
         lifecycle is not wired: {f:?}"
    );
}

/// V2 — deterministic fate sequence through the REAL lifecycle yields EXACT transition counters.
///
/// Drives evidence + guard + advance (the product-shaped path, not `features_from_fate_bits`) with
/// a hand-computed sequence, then asserts the counters equal the hand count. Cannot pass vacuously:
/// a severed `advance` leaves every L provisional and the expected counters are unreachable; a
/// wrong transition count fails the exact equality.
#[test]
fn deterministic_fate_sequence_yields_exact_transition_counters() {
    // ids 0..=9, fates S S L L S L S S L L, hand-counted transitions:
    // SS SL LL LS SL LS SS SL LL  ->  n00=2  n01=3  n10=2  n11=2  (9 transitions, n=10)
    let losses = [2u64, 3, 5, 8, 9];
    let mut e = FateEstimator::new();
    for id in 0..10u64 {
        if losses.contains(&id) {
            e.on_loss_evidence(id, 100, 50); // expiry 150
        } else {
            e.on_original_ack(id);
        }
    }
    assert_eq!(
        e.pending(),
        5,
        "all five losses must sit provisional before the guard elapses"
    );
    e.advance(149);
    assert_eq!(
        e.pending(),
        5,
        "guard not elapsed — nothing may finalize early"
    );
    e.advance(150);
    assert_eq!(
        e.pending(),
        0,
        "expiry <= now must finalize every provisional loss"
    );

    let f = e.snapshot();
    assert_eq!(
        (f.n, f.n00, f.n01, f.n10, f.n11),
        (10, 2, 3, 2, 2),
        "transition counters must equal the hand count for S S L L S L S S L L: {f:?}"
    );

    // Guard-correction branches, on isolated instances so the counters above stay untouched.
    // Spurious: an original-ACK INSIDE the guard corrects the provisional loss to SUCCESS.
    let mut sp = FateEstimator::new();
    sp.on_loss_evidence(7, 1_000, 500);
    sp.on_original_ack(7); // before expiry 1_500
    sp.advance(u64::MAX);
    assert_eq!(
        sp.corrections(),
        (0, 1),
        "in-guard ACK must count as a spurious-loss correction"
    );
    let f = sp.snapshot();
    assert_eq!(
        (f.n, f.n10 + f.n11),
        (1, 0),
        "corrected fate must be SUCCESS: {f:?}"
    );

    // Late: an original-ACK AFTER finalization cannot mutate the window.
    let mut la = FateEstimator::new();
    la.on_loss_evidence(7, 0, 50);
    la.advance(50); // finalized LOSS
    la.on_original_ack(7);
    assert_eq!(
        la.corrections(),
        (1, 0),
        "post-finalization ACK must count as late"
    );
    assert_eq!(la.snapshot().lost, 1, "the finalized LOSS is immutable");

    // Bound (SR-AUDIT-001): the lifecycle makes `finalized` grow in product, so it carries its own
    // bound. Snapshot equivalence: the evicted ids are exactly the ones snapshot already ignored.
    let mut b = FateEstimator::new();
    for id in 0..(FATE_WINDOW as u64 + 44) {
        b.on_original_ack(id);
    }
    assert_eq!(
        b.fate_known(),
        FATE_WINDOW,
        "finalized must stay bounded at FATE_WINDOW"
    );
    let f = b.snapshot();
    assert_eq!(
        f.n, FATE_WINDOW as u32,
        "window must still be exactly FATE_WINDOW"
    );
    assert_eq!(
        (f.window_ids.first(), f.window_ids.last()),
        (Some(&44), Some(&(FATE_WINDOW as u64 + 43))),
        "window must be the LAST FATE_WINDOW ids in logical order"
    );
}

/// V3 — NEUTRALITY: the estimator must not be reachable from any decision.
///
/// Proved structurally rather than by comparing byte streams: no in-process counterfactual exists
/// (no toggle, and a setter is forbidden), so the honest evidence is (a) this pin — the complete
/// use set is two evidence writes, the lifecycle advance, and one diagnostic read — plus (b) the
/// golden suites (fuzzy_e2e, q18_check, the lib pins) staying green.
#[test]
fn estimator_has_exactly_one_reader() {
    let src = include_str!("../src/sender.rs");
    // Join continuations first: rustfmt splits `self.fate_estimator\n    .on_loss_evidence(..)`
    // across lines, so a line-based scan misses the method and reports a bare field access. An
    // earlier draft of this pin did exactly that and failed on correct code.
    let flat = src.replace('\n', " ");
    let uses: Vec<&str> = flat
        .match_indices("self.fate_estimator")
        .map(|(i, _)| &flat[i..(i + 80).min(flat.len())])
        .collect();

    let snapshot_reads = uses.iter().filter(|u| u.contains(".snapshot()")).count();
    assert_eq!(
        snapshot_reads,
        1,
        "the estimator must be read in exactly one place (diag_loss_structure); found {} uses",
        uses.len()
    );
    for u in &uses {
        assert!(
            u.contains(".on_loss_evidence")
                || u.contains(".on_original_ack")
                || u.contains(".advance(")
                || u.contains(".snapshot()"),
            "unexpected estimator use — this would make it decision-carrying: {u}"
        );
    }
    // Exactly the wiring: two evidence writes, one lifecycle advance, one diagnostic read.
    assert_eq!(uses.len(), 4, "estimator use count changed: {uses:?}");
}

/// SOURCE-GROUNDING S-0.7 — ingest FIDELITY, not merely ingest presence.
///
/// Both feeding sites must admit ONLY `TransmitKind::Data`. The loss site sits inside a branch
/// that admits everything except `Duplicate`, so `Retransmit` and `FecParity` reach it unless the
/// narrower condition is present. Parity carries `unit_id: u64::from(p.esi)` — an ESI is a small
/// integer in a different namespace from source logical ids, so feeding it injects foreign ids into
/// a window defined over logical source order.
///
/// This is a source-level pin because the corruption is silent: the counters would still be
/// well-formed, the arithmetic would still be correct, and every existing fixture would still pass
/// while describing a fate sequence the product never produced.
#[test]
fn estimator_ingest_admits_only_originals() {
    let src = include_str!("../src/sender.rs");
    let flat = src.replace('\n', " ");
    for (site, needle) in [
        ("success", ".on_original_ack("),
        ("loss", ".on_loss_evidence("),
    ] {
        let i = flat
            .find(needle)
            .unwrap_or_else(|| panic!("{site} feeding site not found"));
        // The guard must appear within the enclosing block, shortly before the call.
        let window = &flat[i.saturating_sub(700)..i];
        assert!(
            window.contains("p.origin == TransmitKind::Data"),
            "{site} ingest is not restricted to originals — Retransmit/FecParity can reach it"
        );
    }
}
