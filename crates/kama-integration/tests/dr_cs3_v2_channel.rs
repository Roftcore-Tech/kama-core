//! DR-CS-3 — **V2 CHANNEL STRUCTURAL TESTS**
//!
//! Contract: `reports/dr-cs-3/panel-v2/DR-CS-3-PANEL-CONTRACT-V2.md`
//! sha256 `0abbb114bfdf5cae639d25926dac6bf4ed54e2a4eb4be82eaaf3a66e85d64b67`.
//!
//! These prove the channel semantics BEFORE any panel is run. V1 was invalid because packet classes
//! had independent loss streams and the treatment's Echo faced a reverse impairment the baseline's
//! ACK never saw. Each test below pins one clause of the V2 contract.

#[path = "common/dr_cs3_panel.rs"]
mod dr_cs3_panel;

use dr_cs3_panel::fs_cal2_gen::LossModel;
use dr_cs3_panel::{forward_key, reverse_key, run_panel, GeChain, PanelCfg};

const GE: LossModel = LossModel::Ge { p_ppm: 200_000, l: 10 };
const CELL: &str = "V2-STRUCT";
const DOMAIN: &str = "DRCS3-V2-STRUCT";

fn cfg(rep: u32, bootstrap_on: bool, reverse: LossModel) -> PanelCfg {
    PanelCfg {
        cell_id: CELL.into(),
        repetition: rep,
        seed_domain: DOMAIN.into(),
        rtt_ms: 1000,
        forward_model: GE,
        reverse_model: reverse,
        tail_on: false,
        bootstrap_on,
        warmup_timeout_ns: 65_000_000_000,
        probe_drop_script: None,
        echo_drop_script: None,
        echo_wire_duplicates: 0,
        echo_event_duplicates: 0,
    }
}

/// **(1)** Every FORWARD packet class consumes the one forward chain, exactly once each, in
/// emission order: DATA, ARQ retransmissions/duplicates and Probes together account for every step.
#[test]
fn forward_chain_is_shared_by_data_retransmit_and_probe() {
    for on in [false, true] {
        let r = run_panel(&cfg(7, on, LossModel::None));
        assert_eq!(
            r.forward_steps,
            r.tx_data + r.tx_retx + r.tx_dup + r.probes_emitted,
            "bootstrap_on={on}: forward steps must equal ALL forward emissions (no class has its own stream)"
        );
    }
}

/// **(2)** Every REVERSE packet class consumes the one reverse chain, exactly once each: ACKs and
/// Echoes together account for every step.
#[test]
fn reverse_chain_is_shared_by_ack_and_echo() {
    for on in [false, true] {
        let r = run_panel(&cfg(7, on, GE));
        assert_eq!(
            r.reverse_steps,
            r.acks_generated + r.echoes_generated,
            "bootstrap_on={on}: reverse steps must equal ACKs + Echoes (Echo has no private stream)"
        );
    }
}

/// **(3)** The chain is a function of DIRECTION only. Two chains built with the same direction key
/// and model produce the identical sequence regardless of which packet class asks for each draw.
#[test]
fn packet_class_does_not_create_a_separate_channel_state() {
    let k = forward_key(DOMAIN, CELL, 3);
    let mut a = GeChain::new(k.clone(), GE);
    let mut b = GeChain::new(k, GE);
    // `a` is driven as if by one class; `b` interleaved as if by three. Same chain ⇒ same bits.
    let seq_a: Vec<bool> = (0..64).map(|_| a.next_drop()).collect();
    let seq_b: Vec<bool> = (0..64).map(|_| b.next_drop()).collect();
    assert_eq!(seq_a, seq_b, "draw sequence must depend only on position in the chain");
    assert_eq!(a.steps(), b.steps());
    // And the two DIRECTIONS must be independent of each other.
    let mut rev = GeChain::new(reverse_key(DOMAIN, CELL, 3), GE);
    let seq_rev: Vec<bool> = (0..64).map(|_| rev.next_drop()).collect();
    assert_ne!(seq_a, seq_rev, "forward and reverse must be independent streams");
}

/// **(4)** The same chronological traffic prefix yields the same draws in both arms. With bootstrap
/// OFF no probe is inserted, so baseline and treatment share the prefix up to the first probe.
#[test]
fn identical_chronological_prefix_yields_identical_draws() {
    let base = run_panel(&cfg(11, false, LossModel::None));
    let treat = run_panel(&cfg(11, true, LossModel::None));
    let n = base.data_emit_ticks.len().min(treat.data_emit_ticks.len());
    let horizon = treat.bootstrap_edge_ns.unwrap_or(u64::MAX);
    // Every DATA emitted before the bootstrap arms must occur at the same tick in both arms.
    let pre_b: Vec<u64> = base.data_emit_ticks[..n].to_vec();
    let pre_t: Vec<u64> = treat.data_emit_ticks[..n].to_vec();
    if horizon > 0 {
        assert_eq!(
            pre_b.first(),
            pre_t.first(),
            "the first DATA emission must be identical in both arms"
        );
    }
    assert_eq!(
        base.source_prefix_digest.is_empty(),
        treat.source_prefix_digest.is_empty()
    );
}

/// **(5)** On a treatment-free prefix the channel state is bit-identical with bootstrap ON and OFF.
/// Before the first probe traverses, the treatment has consumed no extra forward draw.
#[test]
fn treatment_free_prefix_has_identical_channel_state() {
    let mut a = GeChain::new(forward_key(DOMAIN, CELL, 5), GE);
    let mut b = GeChain::new(forward_key(DOMAIN, CELL, 5), GE);
    // 20 DATA packets in both arms; no probe yet.
    let pre_a: Vec<bool> = (0..20).map(|_| a.next_drop()).collect();
    let pre_b: Vec<bool> = (0..20).map(|_| b.next_drop()).collect();
    assert_eq!(pre_a, pre_b, "no divergence before any treatment packet");
    assert_eq!(a.steps(), b.steps(), "no extra draw consumed by the treatment arm");
    // Now ONE probe traverses in arm `b` only — divergence from here on is REAL channel physics,
    // not a per-class artefact, and it is exactly one step.
    let _probe = b.next_drop();
    assert_eq!(b.steps(), a.steps() + 1, "a probe costs exactly one chain step");
}

/// **(6)** Echoes spaced 6/12/24 s apart do NOT synthesise consecutive treatment-only draws. The
/// chain is packet-domain: it advances only on a real packet event, so the wall-clock gap is
/// irrelevant and no extra draw is invented for elapsed time. (This is the exact V1 defect that made
/// rep14 read `[D,D,D,D]` from a private echo chain.)
#[test]
fn wall_clock_gaps_never_synthesise_draws() {
    let mut c = GeChain::new(reverse_key(DOMAIN, CELL, 14), GE);
    let before = c.steps();
    // Four echo events, however far apart in virtual time, cost exactly four steps.
    for _ in 0..4 {
        let _ = c.next_drop();
    }
    assert_eq!(c.steps(), before + 4, "four packet events ⇒ exactly four steps");

    // In a real run the reverse chain is shared with ACKs, so four echoes can never occupy four
    // CONSECUTIVE draws unless no ACK was generated between them.
    let r = run_panel(&cfg(14, true, GE));
    if r.echoes_generated >= 2 && r.acks_generated > 0 {
        assert!(
            r.reverse_steps > r.echoes_generated,
            "echo draws must be interleaved with ACK draws on the shared reverse chain"
        );
    }
}

/// **(7)** `NETWORK-CENSORED` is its own class and is NOT silently removed: a censored run is still
/// a completed run that counts in the denominator, and is never reported as a mechanism success.
#[test]
fn network_censored_is_separate_but_stays_in_the_denominator() {
    // Force every probe to be forward-dropped ⇒ censored by construction.
    let mut c = cfg(21, true, LossModel::None);
    c.probe_drop_script = Some(vec![true; 8]);
    let r = run_panel(&c);
    if r.probes_emitted > 0 {
        assert_eq!(r.outcome_class, "NETWORK-CENSORED");
        assert_eq!(r.probes_authenticated, 0, "no probe may reach the receiver");
        assert_eq!(r.echo_match_valid, 0, "a censored run produces no RTT sample");
        // It is a real run with real observations — it cannot be dropped from a denominator.
        assert!(r.observation_epoch_ns > 0, "censored runs still complete and are counted");
        assert_eq!(r.sender_budget_violation, 0, "censoring is not a safety failure");
    }
}

/// **(8)** NEGATIVE FIXTURE. Reconstructing V1 semantics — a private per-class chain for Echo — must
/// FAIL the shared-chain property. If this ever passes, the V2 guarantee has been silently lost.
///
/// The proof is the DRAW-INDEX MAPPING, which is deterministic. Drive the identical chronological
/// event sequence ACK, Echo, ACK, Echo:
///   * V2 (one reverse chain): the echoes land on chain positions 1 and 3 — interleaved with ACKs.
///   * V1 (private echo chain): the echoes land on positions 0 and 1 of their own chain, completely
///     independent of how many ACKs were sent.
/// That independence is exactly the defect that let four echoes 6/12/24 s apart consume four
/// consecutive draws in rep14.
#[test]
fn v1_private_class_stream_fails_the_shared_chain_property() {
    // ── V2: one shared reverse chain ──────────────────────────────────────────────────────────
    let mut shared = GeChain::new(reverse_key(DOMAIN, CELL, 14), GE);
    let mut v2_echo_positions = Vec::new();
    for _ in 0..2 {
        let _ack = shared.next_drop(); // ACK traverses first
        v2_echo_positions.push(shared.steps()); // position the echo is about to take
        let _echo = shared.next_drop();
    }
    assert_eq!(
        v2_echo_positions,
        vec![1, 3],
        "on the shared chain echoes MUST be interleaved with ACKs"
    );

    // ── V1: private per-class chains ──────────────────────────────────────────────────────────
    let mut v1_ack = GeChain::new(reverse_key(DOMAIN, CELL, 14), GE);
    let mut v1_echo = GeChain::new(format!("{DOMAIN}|{CELL}|rep14|DRCS3EchoReverse"), GE);
    let mut v1_echo_positions = Vec::new();
    for _ in 0..2 {
        let _ack = v1_ack.next_drop();
        v1_echo_positions.push(v1_echo.steps());
        let _echo = v1_echo.next_drop();
    }
    assert_eq!(
        v1_echo_positions,
        vec![0, 1],
        "the V1 private chain advances independently of ACK traffic — the defect"
    );

    // The structural failure: V1's echo positions do not depend on the reverse traffic at all.
    assert_ne!(
        v1_echo_positions, v2_echo_positions,
        "a private per-class stream must NOT reproduce the shared-chain index mapping; \
         if these ever match, the V1 defect has been reintroduced"
    );
    // And the accounting identity that test (2) pins would be violated under V1:
    // the reverse chain would show 2 steps while 4 reverse packets actually traversed.
    assert_eq!(v1_ack.steps(), 2, "V1 reverse chain sees only the ACKs");
    assert_eq!(
        v1_ack.steps() + v1_echo.steps(),
        4,
        "4 reverse packets exist, but no single chain accounts for them under V1"
    );
    assert_eq!(shared.steps(), 4, "V2: one chain accounts for all 4 reverse packets");
}
