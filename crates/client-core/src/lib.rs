//! Kama `client-core` — client-side composition root.
//!
//! # What this crate is for
//!
//! It is the first place in the workspace where the two halves of the transport meet in a
//! **production** dependency graph:
//!
//! * [`adaptive_transport`] decides *what and when to send* — [`Sender::on_event`] returns
//!   [`TransmitDecision`]s;
//! * [`kama_engine`] decides *what an arriving set of authenticated shards means* —
//!   [`admit_and_decode`] returns the reassembled payload.
//!
//! Until this crate acquired those dependencies, `adaptive-transport` had **no** internal
//! production dependents except `bench`, and `kama-engine` reached it only through
//! `[dev-dependencies]`. Every mechanism validated over the last months — bounded tail
//! recovery, the bootstrap RTT ladder, FEC actuation, the calibrated fusion policy — was
//! therefore unshippable by construction, because nothing could call it outside a test build.
//! `reports/EVIDENCE-LEDGER-2026-08-15.md` §5 records that finding in full.
//!
//! # What this crate is NOT, yet
//!
//! This is item 1 of `reports/dr-cs-3/PRODUCT-RUNTIME-INTEGRATION-PENDING.md` — *name the real
//! product crate/binary owner* — and **only** item 1. It deliberately does not attempt items
//! 2-9, and none of the claims that document forbids may be made on the strength of it:
//!
//! | still missing | item |
//! |---|---|
//! | UDP receive/send loop | 2 |
//! | connection lifecycle | 3 |
//! | Noise handshake | 4 |
//! | Split-derived directional keys | 5 |
//! | outbound datagram drain | 7 |
//! | process-level end-to-end test | 8 |
//! | shutdown / error / resource-bound validation | 9 |
//!
//! There is no socket here, no binary, no key derivation and no session lifecycle. What there
//! is, is a type that owns both sides and a compiled proof that they can be held together.

#![forbid(unsafe_code)]

pub use adaptive_transport::sender::{Event, OpaqueUnit, Sender, TransmitDecision, TransmitKind};
pub use adaptive_transport::tail_recovery::TailRecoveryConfig;
pub use kama_engine::{admit_and_decode, EngineError};
pub use kama_noise_wire::opened::OpenedTransportShard;

/// Owns the egress decision engine and drives the ingress decode path.
///
/// The two directions are deliberately asymmetric, because the underlying crates are:
/// egress is *stateful* (the [`Sender`] carries pacing, ARQ, FEC and RTT state across calls),
/// while ingress is a *pure function* over an already-authenticated shard set. This type does
/// not hide that difference behind a symmetric façade — doing so would invent a lifecycle that
/// item 3 has not yet designed.
pub struct ClientSession {
    sender: Sender,
}

impl ClientSession {
    /// Construct with the shipped defaults.
    ///
    /// Those defaults are: startup pacing **OFF** (DR-RATE-RAMP-2B revert; üçüncü blokör
    /// `fuzzy_e2e` floor-vs-parity), bootstrap RTT **OFF** (kilitli panel 2/3), tail recovery
    /// **ON** — `TailRecoveryConfig::validated_v1()`, the named KAMA-TAIL-V1 constant the
    /// 2290-row product grid ran, re-validated post-P0 on burst + p2 no-harm (G1,
    /// `reports/g1-tail-revalidation/G1-CLOSURE.md`) and flipped by Abkarada decision G2
    /// (2026-08-24, `docs/design/P3-DEFAULT-FLIP-DECISION.md`) — and FEC actuation
    /// `Manual { enabled: false }` (Q18 calibration defect).
    ///
    /// The flip is exactly this construction calling `validated_v1()`. It is deliberately
    /// **never** a flip of `TailRecoveryConfig::default()`, whose bounds were never driven by
    /// any campaign — the R1 rule: unvalidated default bounds do not ship.
    #[must_use]
    pub fn new() -> Self {
        let mut sender = Sender::new();
        sender.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
        Self { sender }
    }

    /// Egress: feed one event and take the resulting send decisions.
    ///
    /// The caller is responsible for actually putting the bytes on a wire — item 7. Nothing in
    /// this crate sends anything.
    pub fn on_event(&mut self, event: Event, now_ns: u64) -> Vec<TransmitDecision> {
        self.sender.on_event(event, now_ns)
    }

    /// Ingress: admit an authenticated shard set and reassemble the payload.
    ///
    /// The shards must already be AEAD-opened — this crate does not hold keys and does not
    /// perform the handshake (items 4 and 5). That ordering is not incidental: INV-FEC-1
    /// requires authenticate-before-decode, and [`OpenedTransportShard`] enforces it in the
    /// type system, since the only way to obtain one is `protector.open()`. An unauthenticated
    /// datagram cannot reach this function because it cannot be given the argument type.
    pub fn admit(
        &mut self,
        shards: Vec<OpenedTransportShard>,
        bound_profile_id: u8,
        k_actual: u8,
    ) -> Result<Vec<u8>, EngineError> {
        admit_and_decode(shards, bound_profile_id, k_actual)
    }

    /// Read-only access to the egress engine, for diagnostics and for the wiring still to come.
    #[must_use]
    pub fn sender(&self) -> &Sender {
        &self.sender
    }

    /// Mutable access, so a future runtime can install a validated tail-recovery or FEC
    /// configuration without this crate having to mirror every setter.
    pub fn sender_mut(&mut self) -> &mut Sender {
        &mut self.sender
    }
}

impl Default for ClientSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The point of this crate: both halves are reachable from one production graph.
    ///
    /// This is a compile-and-link proof, not a behavioural claim. If the dependency edge is
    /// ever removed, this test stops building rather than silently passing.
    #[test]
    fn both_halves_are_reachable_from_one_production_graph() {
        let mut s = ClientSession::new();

        // Egress side is live: a queued unit produces a real decision.
        let out = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 101 }), 0);
        assert!(
            out.iter().any(|d| d.kind == TransmitKind::Data),
            "egress must plan the queued unit"
        );

        // Ingress side is reachable and rejects an unknown profile rather than panicking.
        let err = s.admit(Vec::new(), 0xFF, 0).unwrap_err();
        assert!(
            matches!(err, EngineError::UnknownProfile(0xFF)),
            "ingress must reject an unknown profile: {err:?}"
        );
    }

    /// Converge a flow (first unit ACKed → RTT sample exists), then stall a second unit and
    /// sweep the full worst-case probe horizon of the given config.
    ///
    /// The horizon is **derived from the config**, not chosen. A literal would encode a
    /// coverage claim that silently stops being true when a bound moves: an earlier draft
    /// drove 30 s and claimed to be "well past every tail-probe deadline", while
    /// `max_backoff_pto_ns` was 120 s — it covered a quarter of the range it asserted.
    fn probes_after_converge_then_stall(
        drive: &mut dyn FnMut(Event, u64) -> Vec<TransmitDecision>,
        cfg: &TailRecoveryConfig,
    ) {
        use adaptive_transport::sender::{AckInfo, Timer};
        use std::time::Duration;

        const MS: u64 = 1_000_000;
        // Converge: unit 1 emitted, ACKed one RTT later (probe-fresh after converge — the same
        // shape the validated product harness used: warmup → converge, then measure).
        let out = drive(Event::AppData(OpaqueUnit { id: 1, size: 101 }), 0);
        let seq = out
            .iter()
            .find(|d| d.kind == TransmitKind::Data)
            .expect("unit 1 must be planned")
            .seq;
        drive(
            Event::AckReceived(AckInfo {
                acked_seqs: vec![seq],
                ack_delay: Duration::ZERO,
            }),
            40 * MS,
        );
        // Stall: unit 2 emitted, never ACKed, no later packets → only a PTO tail probe can act
        // (time-threshold is blind at the tail: seq ≥ largest_acked).
        let t0 = 50 * MS;
        drive(Event::AppData(OpaqueUnit { id: 2, size: 101 }), t0);

        // Worst case the mechanism could reach: every permitted probe round waiting the maximum
        // backed-off PTO, after the initial no-RTT PTO, plus the receiver's ACK-delay budget.
        let horizon_ns = cfg
            .initial_pto_ns
            .saturating_add(cfg.max_ack_delay_ns)
            .saturating_add(
                cfg.max_backoff_pto_ns
                    .saturating_mul(u64::from(cfg.max_tail_probes) + 1),
            );
        // One tick per base-PTO floor, so the sweep cannot step over a deadline.
        let step_ns = cfg.min_pto_ns.max(1);
        let ticks = horizon_ns / step_ns;
        for i in 1..=ticks {
            drive(
                Event::TimerFired(Timer::Pto),
                t0.saturating_add(i.saturating_mul(step_ns)),
            );
        }
    }

    /// Guards the G2 shipped default (P3-DEFAULT-FLIP-DECISION §2.3, Abkarada 2026-08-24):
    /// `ClientSession::new()` ships tail recovery **ON** via `validated_v1()` — a converged
    /// flow with a stalled tail unit MUST probe. Twin guard below pins that the raw engine
    /// default stays OFF, so this flip lives in the construction, never in
    /// `TailRecoveryConfig::default()` (R1 rule).
    #[test]
    fn shipped_default_tail_recovery_is_on_via_validated_v1() {
        let mut s = ClientSession::new();
        probes_after_converge_then_stall(
            &mut |e, t| s.on_event(e, t),
            &TailRecoveryConfig::validated_v1(),
        );
        let (probes, probe_bytes, _, _) = s.sender().tail_probe_diag();
        assert!(
            probes >= 1 && probe_bytes > 0,
            "G2: ClientSession ships tail-ON (validated_v1); a converged flow with a stalled \
             tail unit must emit at least one probe, got {probes}"
        );
    }

    /// R1 rule, other direction: the ENGINE default (`Sender::new()`) keeps tail recovery OFF —
    /// `TailRecoveryConfig::default()` bounds were never campaign-driven and must not ship.
    /// The G2 flip is `ClientSession::new()` calling `validated_v1()`, nothing deeper.
    #[test]
    fn engine_default_tail_recovery_stays_off() {
        let mut s = Sender::new();
        probes_after_converge_then_stall(
            &mut |e, t| s.on_event(e, t),
            &TailRecoveryConfig::validated_v1(), // horizon: sweep the SAME range the ON test does
        );
        let (probes, probe_bytes, spurious, _) = s.tail_probe_diag();
        assert_eq!(
            (probes, probe_bytes, spurious),
            (0, 0, 0),
            "engine default must stay tail-OFF (R1): same converge-then-stall sweep, no probes"
        );
    }
}
