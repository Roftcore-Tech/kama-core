//! DR-CS-3 — **PAIRED STRUCTURAL ORACLE** for the gate-5 displacement component.
//!
//! Classification: `STRUCTURAL REGRESSION INVARIANT — PACER/GRANT DISPLACEMENT EXPECTED 0 BY
//! ADDITIVE, UNPACED, DATA-FIRST DESIGN`; semantics `A — contended grant/token`.
//!
//! **Grounded accessor semantics** (from signatures, not tuple position):
//! * `diag_loss() = (diag_loss_pkt, diag_loss_time, pending_lost.len())` — `.0`/`.1` are CUMULATIVE
//!   loss-declared counters; `.2` is an INSTANTANEOUS depth. Loss is declared *and* drained inside
//!   one `on_event(AckReceived)`, so post-call depth is NOT an activation oracle.
//! * `diag_pacer() = (rate_bps, tokens, burst_cap)`; tokens are BYTES.
//! * `try_send(size)`: `tokens >= size` then `tokens -= size` ⇒ **grant cost = packet bytes**.
//! * pacer debt: `N/A — REMOVED BY DR-CS-2` (no debt concept exists in the product).
//!
//! Shipping `Sender` has no `Clone` and none is added: both arms are built from identical frozen
//! inputs and asserted equal before the target event.

use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, OpaqueUnit, Sender, StartupPacingConfig, Timer,
    TransmitDecision, TransmitKind,
};
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const UNIT: u64 = 101; // DATA size, and therefore its exact grant cost in tokens

// ── the ONE shared oracle ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleInput {
    /// Ordered NORMAL-packet plan (Probe suffix excluded by construction).
    plan: Vec<(String, u64, u64, u64)>,
    tokens: u64,
    burst_cap: u64,
    rate: u64,
    inflight: u64,
    sent_logical_len: usize,
    pending_lost: u64,
}

#[derive(Debug, Default)]
struct DisplacementReport {
    missing_or_replaced: Vec<(String, u64, u64, u64)>,
    order_divergence: usize,
    length_divergence: i64,
    pacer_divergence: bool,
    inflight_or_sentlogical_divergence: bool,
    /// Total direct-grant displacement verdict.
    total: usize,
}

/// The single comparator used by BOTH the shipping cells and the synthetic control.
/// Never `zip`-only: it sees length, unmatched prefix/suffix, identity, kind and order.
fn evaluate_grant_displacement(off: &OracleInput, on: &OracleInput) -> DisplacementReport {
    let mut r = DisplacementReport {
        length_divergence: on.plan.len() as i64 - off.plan.len() as i64,
        pacer_divergence: off.tokens != on.tokens
            || off.burst_cap != on.burst_cap
            || off.rate != on.rate,
        inflight_or_sentlogical_divergence: off.inflight != on.inflight
            || off.sent_logical_len != on.sent_logical_len
            || off.pending_lost != on.pending_lost,
        ..DisplacementReport::default()
    };
    // Any OFF normal packet not present in the ON plan was replaced or dropped.
    for p in &off.plan {
        if !on.plan.contains(p) {
            r.missing_or_replaced.push(p.clone());
        }
    }
    // Order divergence over the common prefix.
    for (a, b) in off.plan.iter().zip(on.plan.iter()) {
        if a != b {
            r.order_divergence += 1;
        }
    }
    r.total = r.missing_or_replaced.len()
        + r.order_divergence
        + r.length_divergence.unsigned_abs() as usize
        + usize::from(r.pacer_divergence)
        + usize::from(r.inflight_or_sentlogical_divergence);
    r
}

fn normal_plan(out: &[TransmitDecision]) -> Vec<(String, u64, u64, u64)> {
    out.iter()
        .filter(|d| d.kind != TransmitKind::CsProbe)
        .map(|d| (format!("{:?}", d.kind), d.seq, d.unit_id, d.size))
        .collect()
}

fn input(s: &Sender, out: &[TransmitDecision]) -> OracleInput {
    let (rate, tokens, burst_cap) = s.diag_pacer();
    let (_st, _bw, _sr, _r, inflight) = s.diag_rate();
    let (sl, _ev, _m) = s.diag_sent_logical();
    OracleInput {
        plan: normal_plan(out),
        tokens,
        burst_cap,
        rate,
        inflight,
        sent_logical_len: sl,
        pending_lost: s.diag_loss().2,
    }
}

/// `(probe_due, token_regime, queued_data, recovery_active, parity_active, target_event_kind)`
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    probe_due: bool,
    grants_available: u64,
    queued_data: u64,
    recovery_active: bool,
    parity_active: bool,
    target_event: &'static str,
}

// ── cells ───────────────────────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Target {
    Tick,
    AckGap,
}

struct Cell {
    name: &'static str,
    units: u64,
    advance_to_ns: u64,
    probe_due_expected: bool,
    target: Target,
    parity_before_target: bool,
}

const CELLS: [Cell; 6] = [
    Cell { name: "1 probe NOT due + queued DATA",           units: 8,  advance_to_ns: SEC,       probe_due_expected: false, target: Target::Tick,   parity_before_target: false },
    Cell { name: "2 probe due + queued DATA + zero grant",  units: 8,  advance_to_ns: 3 * SEC - 100 * MS,  probe_due_expected: true,  target: Target::Tick,   parity_before_target: false },
    Cell { name: "3 probe due + queued DATA + one grant",   units: 8,  advance_to_ns: 9 * SEC - 100 * MS,  probe_due_expected: true,  target: Target::Tick,   parity_before_target: false },
    Cell { name: "4 probe due + queued DATA + many grants", units: 24, advance_to_ns: 21 * SEC - 100 * MS, probe_due_expected: true,  target: Target::Tick,   parity_before_target: false },
    Cell { name: "5 probe due + DATA + recovery in-turn",   units: 24, advance_to_ns: 21 * SEC - 100 * MS, probe_due_expected: true,  target: Target::AckGap, parity_before_target: false },
    Cell { name: "6 probe due + DATA + parity pending",     units: 24, advance_to_ns: 45 * SEC - 100 * MS, probe_due_expected: true,  target: Target::Tick,   parity_before_target: true },
];

/// Build one arm up to (but not including) the target event.
#[allow(deprecated)]
fn build(bootstrap_on: bool, c: &Cell) -> (Sender, Vec<TransmitDecision>) {
    let mut s = Sender::new();
    s.set_startup_pacing_init(StartupPacingConfig { enabled: false });
    s.set_bootstrap_rtt(BootstrapRttConfig { enabled: bootstrap_on });
    if c.parity_before_target {
        s.set_manual_fec_enabled(true).expect("manual FEC authority");
    }
    let mut pre = Vec::new();
    for i in 0..c.units {
        pre.extend(s.on_event(Event::AppData(OpaqueUnit { id: 1000 + i, size: UNIT }), 0));
    }
    let mut t = 100 * MS;
    while t <= c.advance_to_ns {
        pre.extend(s.on_event(Event::TimerFired(Timer::Pto), t));
        t += 100 * MS;
    }
    // Parity is staged IMMEDIATELY before the target turn, identically in both arms.
    if c.parity_before_target {
        for esi in 0..3u16 {
            s.enqueue_fec_parity(9, esi, 400);
        }
    }
    (s, pre)
}

#[test]
fn paired_structural_oracle_six_cells() {
    let mut fingerprints: Vec<(&str, Fingerprint)> = Vec::new();
    let (mut assertions, mut activations) = (0usize, 0usize);

    for c in &CELLS {
        let (mut off, off_pre) = build(false, c);
        let (mut on, on_pre) = build(true, c);

        // PRECONDITION: identical starting point.
        assert_eq!(input(&off, &off_pre), input(&on, &on_pre), "{}: identical start", c.name);
        assertions += 1;

        // Measured token regime at the target instant (grant cost = UNIT bytes).
        let (_rate, tokens, _cap) = off.diag_pacer();
        let grants = tokens / UNIT;
        let emitted = off_pre.iter().filter(|d| d.kind == TransmitKind::Data).count() as u64;
        let queued = c.units - emitted;
        assert!(queued > 0, "{}: DATA must remain queued (emitted {emitted}/{})", c.name, c.units);
        assertions += 1;

        // Cell-2/3/4 token-regime claims must be MEASURED, not named.
        eprintln!("PRE {} tokens={tokens} grants={grants} queued={queued}", c.name);
        match c.name.chars().next().unwrap() {
            '2' => { assert_eq!(grants, 0, "{}: must have zero grants (tokens={tokens})", c.name); assertions += 1; }
            '3' => { assert_eq!(grants, 1, "{}: must have exactly one grant (tokens={tokens})", c.name); assertions += 1; }
            '4' => { assert!(grants >= 2, "{}: must have >=2 grants (tokens={tokens})", c.name); assertions += 1; }
            _ => {}
        }

        let loss_before = off.diag_loss();
        let t = c.advance_to_ns + 100 * MS;

        // The TARGET event, delivered identically to both arms.
        let (off_out, on_out) = match c.target {
            Target::Tick => (
                off.on_event(Event::TimerFired(Timer::Pto), t),
                on.on_event(Event::TimerFired(Timer::Pto), t),
            ),
            Target::AckGap => {
                let sent: Vec<u64> = off_pre
                    .iter()
                    .filter(|d| d.kind == TransmitKind::Data)
                    .map(|d| d.seq)
                    .collect();
                assert!(sent.len() >= 3, "{}: need >=3 sent DATA for a gap", c.name);
                let gap: Vec<u64> = sent[1..].to_vec(); // withhold the FIRST ⇒ real gap
                (
                    off.on_event(Event::AckReceived(AckInfo { acked_seqs: gap.clone(), ack_delay: Duration::from_millis(1) }), t),
                    on.on_event(Event::AckReceived(AckInfo { acked_seqs: gap, ack_delay: Duration::from_millis(1) }), t),
                )
            }
        };

        // ACTIVATION — proven from the TARGET turn, never from a stale cumulative alone.
        let probe_active = on_out.iter().any(|d| d.kind == TransmitKind::CsProbe);
        assert_eq!(probe_active, c.probe_due_expected, "{}: probe activation", c.name);
        assertions += 1;
        if probe_active { activations += 1; }

        let loss_after = off.diag_loss();
        let recovery_active = c.target == Target::AckGap
            && (loss_after.0 > loss_before.0
                || loss_after.1 > loss_before.1
                || off_out.iter().any(|d| d.kind == TransmitKind::Retransmit));
        if c.target == Target::AckGap {
            assert!(
                recovery_active,
                "{}: recovery must activate in the target turn (loss {loss_before:?} -> {loss_after:?})",
                c.name
            );
            assertions += 1;
            activations += 1;
        }

        let parity_active = off_out.iter().any(|d| d.kind == TransmitKind::FecParity);
        if c.parity_before_target {
            assert!(parity_active, "{}: parity must emit in the target turn", c.name);
            assertions += 1;
            activations += 1;
        }

        // THE ORACLE — one shared comparator, verdict derived not hard-coded.
        let rep = evaluate_grant_displacement(&input(&off, &off_out), &input(&on, &on_out));
        assert_eq!(
            rep.total, 0,
            "{}: shipping displacement must be 0, got {rep:?}",
            c.name
        );
        assertions += 1;

        // Probe may only be a suffix and must carry no send record.
        for p in on_out.iter().filter(|d| d.kind == TransmitKind::CsProbe) {
            assert!(p.send_record.is_none(), "{}: probe carries no send_record", c.name);
            assertions += 1;
        }
        let n = normal_plan(&on_out).len();
        assert!(
            on_out[..n].iter().all(|d| d.kind != TransmitKind::CsProbe),
            "{}: probes must be appended after all normal packets",
            c.name
        );
        assertions += 1;

        fingerprints.push((c.name, Fingerprint {
            probe_due: probe_active,
            grants_available: grants,
            queued_data: queued,
            recovery_active,
            parity_active,
            target_event: match c.target { Target::Tick => "Pto", Target::AckGap => "AckGap" },
        }));
    }

    // RULE-8 — semantic cell uniqueness.
    for i in 0..fingerprints.len() {
        for j in (i + 1)..fingerprints.len() {
            assert_ne!(
                fingerprints[i].1, fingerprints[j].1,
                "RULE-8 SEMANTIC CELL UNIQUENESS FAILURE: {} and {} share a fingerprint",
                fingerprints[i].0, fingerprints[j].0
            );
        }
    }

    for (n, f) in &fingerprints {
        eprintln!("CELL {n} -> {f:?}");
    }
    eprintln!(
        "ORACLE exercised={} unreachable=0 harness_limited=0 activations={activations} assertions={assertions} shipping_displacement_per_cell=0",
        CELLS.len()
    );
}

/// **SYNTHETIC POSITIVE CONTROL** — DR-CS-2 debiting, evaluated by the SAME helper.
#[test]
fn synthetic_debiting_is_caught_by_the_same_oracle() {
    let base = |plan: Vec<(String, u64, u64, u64)>, tokens: u64| OracleInput {
        plan, tokens, burst_cap: 1200, rate: 31, inflight: 0, sent_logical_len: 0, pending_lost: 0,
    };
    // OFF: the single grant goes to DATA.
    let off = base(vec![("Data".into(), 0, 1000, UNIT)], 0);
    // ON: a treatment packet consumed that same grant instead (tx 20->18, dup 0->2).
    let on = base(vec![("Duplicate".into(), 0, 1000, UNIT)], 0);

    let rep = evaluate_grant_displacement(&off, &on);
    assert_eq!(rep.missing_or_replaced.len(), 1, "the DATA was replaced");
    assert_eq!(rep.total, 2, "one replacement + one order divergence"); // exact, derived
    eprintln!("SYNTHETIC displacement_total={} report={rep:?}", rep.total);
}
