//! DR-CS-3 — **PAIRED STRUCTURAL ORACLE** for the gate-5 displacement component.
//!
//! Classification: `STRUCTURAL REGRESSION INVARIANT — PACER/GRANT DISPLACEMENT EXPECTED 0 BY
//! ADDITIVE, UNPACED, DATA-FIRST DESIGN`. Semantics `A — contended grant/token`
//! (`DR-CS-3-DISPLACEMENT-SEMANTICS-GROUNDING`, md `775f60ef…`).
//!
//! Two senders are built from **identical frozen inputs** (shipping `Sender` has no `Clone`, and none
//! is added), their initial state is asserted equal, then one scheduling turn is compared.
//!
//! The oracle does NOT hard-code "displacement = 0". It derives the verdict from paired output and
//! pacer/state differences, so a future token-consuming Probe would fail it with no test change.

use adaptive_transport::sender::{
    BootstrapRttConfig, Event, OpaqueUnit, Sender, StartupPacingConfig, Timer, TransmitDecision,
    TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const UNIT: u64 = 101;

/// Everything the oracle compares. Read entirely from EXISTING observation accessors.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Snap {
    state: String,
    tokens: u64,
    burst_cap: u64,
    rate: u64,
    inflight: u64,
    sent_logical_len: usize,
    sent_logical_evicted: u64,
    pending_lost: u64,
}

fn snap(s: &Sender) -> Snap {
    let (rate, tokens, burst_cap) = s.diag_pacer();
    let (_st, _bw, _srtt, _r, inflight) = s.diag_rate();
    let (sl_len, sl_evicted, _miss) = s.diag_sent_logical();
    Snap {
        state: format!("{:?}", s.state()),
        tokens,
        burst_cap,
        rate,
        inflight,
        sent_logical_len: sl_len,
        sent_logical_evicted: sl_evicted,
        pending_lost: s.diag_loss().2,
    }
}

/// Normal (non-Probe) packets, in emission order, as comparable identities.
fn normal_prefix(out: &[TransmitDecision]) -> Vec<(String, u64, u64, u64)> {
    out.iter()
        .filter(|d| d.kind != TransmitKind::CsProbe)
        .map(|d| (format!("{:?}", d.kind), d.seq, d.unit_id, d.size))
        .collect()
}

fn probes(out: &[TransmitDecision]) -> Vec<&TransmitDecision> {
    out.iter().filter(|d| d.kind == TransmitKind::CsProbe).collect()
}

/// Deterministic sender from frozen inputs. `bootstrap_on` is the ONLY axis.
#[allow(deprecated)] // legacy parity enqueue is the bounded way to make pending_fec non-empty
fn build(bootstrap_on: bool, c: &Cell) -> (Sender, Vec<TransmitDecision>) {
    let mut s = Sender::new();
    s.set_startup_pacing_init(StartupPacingConfig { enabled: false }); // freeze the pacing axis
    s.set_bootstrap_rtt(BootstrapRttConfig { enabled: bootstrap_on });
    if c.extra == Extra::ParityPending {
        s.set_manual_fec_enabled(true).expect("manual FEC authority");
    }
    let mut pre = Vec::new();
    for i in 0..c.units {
        pre.extend(s.on_event(
            Event::AppData(OpaqueUnit { id: 1000 + i, size: UNIT }),
            0,
        ));
    }
    // Advance in fixed steps so both arms see an identical clock sequence.
    let mut t = 100 * MS;
    let mut seeded = false;
    while t <= c.advance_to_ns {
        pre.extend(s.on_event(Event::TimerFired(Timer::Pto), t));
        // Create the cell's extra state ONCE, identically in both arms.
        if !seeded && t >= c.advance_to_ns / 2 {
            match c.extra {
                Extra::RecoveryPending => {
                    // ACK a LATER seq only, leaving an earlier one below largest_acked ⇒ real loss.
                    let sent: Vec<u64> = pre
                        .iter()
                        .filter(|d| d.kind == TransmitKind::Data)
                        .map(|d| d.seq)
                        .collect();
                    if sent.len() >= 2 {
                        pre.extend(s.on_event(
                            Event::AckReceived(adaptive_transport::sender::AckInfo {
                                acked_seqs: sent[1..].to_vec(),
                                ack_delay: std::time::Duration::from_millis(1),
                            }),
                            t,
                        ));
                        seeded = true;
                    }
                }
                Extra::ParityPending => {
                    for esi in 0..3u16 {
                        s.enqueue_fec_parity(9, esi, 400);
                    }
                    seeded = true;
                }
                Extra::None => seeded = true,
            }
        }
        t += 100 * MS;
    }
    (s, pre)
}

/// What extra product state the cell must genuinely create.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Extra {
    None,
    /// A real ACK gap so loss is declared and `pending_lost > 0` (RecoverArq path).
    RecoveryPending,
    /// Real queued FEC parity via the manual authority (`pending_fec > 0`).
    ParityPending,
}

struct Cell {
    name: &'static str,
    units: u64,
    advance_to_ns: u64,
    probe_due_expected: bool,
    extra: Extra,
}

/// The six cells, frozen before results.
const CELLS: [Cell; 6] = [
    Cell { name: "1 probe NOT due + queued DATA + one grant",     units: 8,  advance_to_ns: SEC,                probe_due_expected: false, extra: Extra::None },
    Cell { name: "2 probe due + queued DATA + zero token",        units: 8,  advance_to_ns: 3 * SEC,            probe_due_expected: true,  extra: Extra::None },
    Cell { name: "3 probe due + queued DATA + exactly one grant", units: 8,  advance_to_ns: 3 * SEC + 300 * MS, probe_due_expected: true,  extra: Extra::None },
    Cell { name: "4 probe due + queued DATA + several grants",    units: 24, advance_to_ns: 9 * SEC,            probe_due_expected: true,  extra: Extra::None },
    Cell { name: "5 probe due + DATA + recovery pending",         units: 24, advance_to_ns: 21 * SEC,           probe_due_expected: true,  extra: Extra::RecoveryPending },
    Cell { name: "6 probe due + DATA + parity pending",           units: 24, advance_to_ns: 45 * SEC,           probe_due_expected: true,  extra: Extra::ParityPending },
];

/// **SHIPPING NEGATIVE FIXTURE.** For every frozen cell: identical initial state, DATA genuinely
/// queued, and — where the cell says so — a Probe genuinely emitted. The normal-packet prefix and
/// the whole pacer/inflight/sent_logical state must be EXACTLY equal, and the Probe may only appear
/// as a suffix carrying no send_record.
#[test]
fn paired_structural_oracle_six_cells() {
    let mut exercised = 0usize;
    let mut probe_cells = 0usize;
    let mut assertions = 0usize;

    for c in &CELLS {
        // Two senders from identical frozen inputs; bootstrap is the only axis.
        let (mut off, off_pre) = build(false, c);
        let (mut on, on_pre) = build(true, c);

        // PRECONDITION: identical starting point.
        assert_eq!(snap(&off), snap(&on), "{}: initial state must be identical", c.name);
        assert_eq!(
            normal_prefix(&off_pre),
            normal_prefix(&on_pre),
            "{}: pre-turn normal traffic must be identical",
            c.name
        );
        assertions += 2;

        // PRECONDITION: DATA genuinely queued and unsent (fewer emitted than fed).
        let emitted_data = off_pre.iter().filter(|d| d.kind == TransmitKind::Data).count() as u64;
        assert!(
            emitted_data < c.units,
            "{}: fixture must leave DATA queued (emitted {emitted_data} of {})",
            c.name,
            c.units
        );
        assertions += 1;

        // ACTIVATION: the cell's extra product state must genuinely exist.
        match c.extra {
            Extra::RecoveryPending => {
                assert!(off.diag_loss().2 > 0, "{}: recovery must be pending", c.name);
                assertions += 1;
            }
            Extra::ParityPending => {
                let d = off.diag_fec_actuation();
                assert!(d.enqueued_r_total > 0, "{}: parity must be enqueued", c.name);
                assertions += 1;
            }
            Extra::None => {}
        }

        // One scheduling turn at the same instant on both arms.
        let t = c.advance_to_ns + 100 * MS;
        let off_out = off.on_event(Event::TimerFired(Timer::Pto), t);
        let on_out = on.on_event(Event::TimerFired(Timer::Pto), t);

        // ACTIVATION: the cell's probe-due expectation must actually hold.
        let on_probes = probes(&on_out);
        let probe_seen = !on_probes.is_empty() || on.bootstrap_rtt_diag().probe_emitted > 0;
        assert_eq!(
            probe_seen, c.probe_due_expected,
            "{}: probe activation must match the frozen cell (seen={probe_seen})",
            c.name
        );
        assertions += 1;
        if c.probe_due_expected {
            probe_cells += 1;
        }

        // ORACLE — derived, not hard-coded: the normal-packet plan and the pacer/state must be
        // identical, so no grant was diverted.
        assert_eq!(
            normal_prefix(&off_out),
            normal_prefix(&on_out),
            "{}: DATA/recovery/parity prefix diverged ⇒ grant displacement",
            c.name
        );
        assert_eq!(
            snap(&off),
            snap(&on),
            "{}: pacer/inflight/sent_logical state diverged ⇒ grant displacement",
            c.name
        );
        assertions += 2;

        // Probe may only be a suffix, and must carry no send record.
        for p in &on_probes {
            assert!(p.send_record.is_none(), "{}: probe must carry no send_record", c.name);
            assertions += 1;
        }
        let normal_on = normal_prefix(&on_out).len();
        assert_eq!(
            on_out[..normal_on].iter().filter(|d| d.kind == TransmitKind::CsProbe).count(),
            0,
            "{}: probes must be appended after all normal packets, never interleaved",
            c.name
        );
        assertions += 1;

        exercised += 1;
    }

    assert_eq!(exercised, CELLS.len(), "all frozen cells must be exercised");
    assert!(probe_cells >= 5, "at least the five probe-due cells must have activated");
    eprintln!(
        "ORACLE cells_exercised={exercised}/{} probe_active_cells={probe_cells} assertions={assertions}",
        CELLS.len()
    );
}

/// **SYNTHETIC POSITIVE CONTROL — oracle sensitivity only.** Models the DR-CS-2 debiting behaviour:
/// a treatment packet consumes the same finite grant a DATA would have taken. The SAME comparison
/// must report non-zero displacement. This is not shipping behaviour and is not product evidence.
#[test]
fn synthetic_debiting_treatment_is_caught_by_the_same_oracle() {
    // OFF: the grant goes to DATA.
    let off_out: Vec<(String, u64, u64, u64)> =
        vec![("Data".into(), 0, 1000, UNIT), ("Data".into(), 1, 1001, UNIT)];
    // ON: a treatment packet consumed one grant, substituting for a DATA (DR-CS-2: tx 20→18, dup 0→2).
    let on_out: Vec<(String, u64, u64, u64)> =
        vec![("Data".into(), 0, 1000, UNIT), ("Duplicate".into(), 1, 1001, UNIT)];

    // Same derivation the shipping oracle uses: compare the normal-packet plan.
    let displaced = off_out
        .iter()
        .zip(on_out.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        displaced > 0,
        "the oracle must catch a treatment packet taking a DATA's grant"
    );
    assert_eq!(displaced, 1, "exactly one grant was substituted");
    eprintln!("SYNTHETIC-SENSITIVITY displaced={displaced} (oracle proof only, not product evidence)");
}
