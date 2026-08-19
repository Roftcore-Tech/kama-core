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
    /// Those defaults are: startup pacing **ON**, bootstrap RTT **OFF**, tail recovery **OFF**,
    /// FEC actuation `Manual { enabled: false }`. Enabling tail recovery is
    /// `sender_mut().set_tail_recovery_config(TailRecoveryConfig::validated_v1())` — the named
    /// KAMA-TAIL-V1 constant the 2290-row product grid actually ran — never a flip of
    /// `TailRecoveryConfig::default()`, whose bounds were never driven by any campaign.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sender: Sender::new(),
        }
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

    /// Guards the defaults this crate documents, so the doc comment cannot drift from the code
    /// the way the startup-pacing rationale did (see sender.rs and commit 50cd8e35).
    ///
    /// The observation horizon is **derived from the shipped config**, not chosen. A literal
    /// here would encode a coverage claim that silently stops being true when a bound moves:
    /// an earlier draft of this test drove 30 s and claimed to be "well past every tail-probe
    /// deadline", while `max_backoff_pto_ns` is 120 s — it covered a quarter of the range it
    /// asserted. Deriving the horizon makes the test track the config instead of a guess.
    #[test]
    fn shipped_defaults_are_what_the_docs_say() {
        let cfg = TailRecoveryConfig::default();

        // Worst case the mechanism could reach if it were enabled: every permitted probe round
        // waiting the maximum backed-off PTO, after the initial no-RTT PTO, plus the receiver's
        // ACK-delay budget. Nothing here is a literal; each term names the bound it comes from.
        let horizon_ns = cfg
            .initial_pto_ns
            .saturating_add(cfg.max_ack_delay_ns)
            .saturating_add(
                cfg.max_backoff_pto_ns
                    .saturating_mul(u64::from(cfg.max_tail_probes) + 1),
            );

        // One tick per base-PTO floor, so the sweep cannot step over a deadline. Bounded by the
        // same arithmetic rather than by a chosen iteration count.
        let step_ns = cfg.min_pto_ns.max(1);
        let ticks = horizon_ns / step_ns;

        let mut s = ClientSession::new();
        // Payload size is a genuinely arbitrary fixture: this test asserts nothing about bytes,
        // only that a unit is outstanding so the tail path has something to probe for.
        s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 101 }), 0);
        for i in 1..=ticks {
            s.on_event(
                Event::TimerFired(adaptive_transport::sender::Timer::Pto),
                i.saturating_mul(step_ns),
            );
        }

        let (probes, probe_bytes, spurious, _) = s.sender().tail_probe_diag();
        assert_eq!(
            (probes, probe_bytes, spurious),
            (0, 0, 0),
            "tail recovery is default-OFF: a flow stalled for {horizon_ns} ns \
             ({ticks} ticks, the full worst-case probe horizon of the shipped config) \
             must emit no probe"
        );
    }
}
