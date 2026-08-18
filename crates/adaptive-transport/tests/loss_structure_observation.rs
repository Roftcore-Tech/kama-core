//! FS-CAL-2 v3 §3 estimator wiring — it must OBSERVE, and it must change NOTHING.
//!
//! Q17 (preregistered holdout) and FS-FUSION-CELL-1 both concluded that loss RATE cannot gate FEC
//! and loss STRUCTURE is the missing input. The estimator that computes structure existed but had
//! no product consumer. These pins guard the two properties that wiring has to have.
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

/// NON-VACUITY: under real loss the estimator's window must actually fill.
///
/// Without this the wiring could compile, be read by a diagnostic, and always report an empty
/// window — the failure mode this repo has hit repeatedly, where a mechanism is named but never
/// fires.
#[test]
fn estimator_observes_real_loss() {
    let mut s = Sender::new();
    feed(&mut s, 24);
    // Drive traffic and acknowledge a gapped pattern so loss is declared on the originals.
    for t in 1..=40u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    // ACK a sparse subset: the unacked originals become loss evidence.
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0, 2, 4, 6, 8],
            ack_delay: Duration::ZERO,
        }),
        9_000 * MS,
    );
    for t in 41..=90u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }

    let f = s.diag_loss_structure();
    assert!(
        f.n > 0,
        "estimator window is empty after a lossy flow — the wiring observes nothing: {f:?}"
    );
}

/// NEUTRALITY: the estimator must not be reachable from any decision.
///
/// Proved structurally rather than by comparing byte streams: the field is read at exactly one
/// place. A source-level check is stronger here than an output comparison, because an output
/// comparison only samples the paths the test happens to drive.
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
                || u.contains(".snapshot()"),
            "unexpected estimator use — this would make it decision-carrying: {u}"
        );
    }
    // Exactly the wiring: two writes, one read.
    assert_eq!(uses.len(), 3, "estimator use count changed: {uses:?}");
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
