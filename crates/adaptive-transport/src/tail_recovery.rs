//! DR-ARQ-PRODUCT-TAIL-1 — bounded tail-loss recovery state machine (product transport core).
//!
//! Pure, deterministic (RNG-free), overflow-safe. Implements the frozen INV-CS-4 supersession TDD
//! (docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD.md §4): a PTO-driven bounded probe that retransmits the OLDEST
//! outstanding ack-eliciting source when no ACK progress has occurred for a PTO interval. **Default OFF** — when
//! `enabled == false` the machine never fires, so the sender path and emitted wire are byte-identical to pre-decision.
//!
//! Not a product-deployment or RFC 9002 compliance claim. All arithmetic is checked/saturating; no wire/protocol
//! format change (reuses the existing ARQ retransmission path). FEC-independent: the machine reads no FEC state and
//! its firing cannot toggle FEC.
//!
//! `sender.rs` only: derives typed input from live sender/ACK state, calls `on_timer`, wires `ProbeDue` to the
//! existing ARQ retransmit path, and records metrics. Karn (no RTT sample from a retransmitted/probed packet) is
//! enforced at the sender: probe outputs are marked so their ACKs never feed the RTT estimator.

/// Typed, product-config-driven bounds (default OFF; conservative bounds).
#[derive(Debug, Clone, Copy)]
pub struct TailRecoveryConfig {
    /// Master gate. Default false → mechanism inert (byte-identical wire/behaviour).
    pub enabled: bool,
    /// Lower bound on the base PTO (ns).
    pub min_pto_ns: u64,
    /// Upper bound on the base PTO (ns), before backoff.
    pub max_pto_ns: u64,
    /// PTO used when no RTT sample exists yet (ns). NOT derived from a retransmitted sample.
    pub initial_pto_ns: u64,
    /// Hard cap on the backed-off PTO (ns), after exponential backoff.
    pub max_backoff_pto_ns: u64,
    /// Receiver max ACK delay budget added to the base PTO (ns).
    pub max_ack_delay_ns: u64,
    /// Hard cap on probe rounds per idle window (never unbounded).
    pub max_tail_probes: u32,
}

impl Default for TailRecoveryConfig {
    fn default() -> Self {
        // DEFAULT OFF. Bounds are conservative product defaults (ns).
        Self {
            enabled: false,
            min_pto_ns: 1_000_000,               // 1 ms floor
            max_pto_ns: 60_000_000_000,          // 60 s ceiling on base PTO
            initial_pto_ns: 1_000_000_000,       // 1 s when no RTT sample
            max_backoff_pto_ns: 120_000_000_000, // 120 s absolute backoff ceiling
            max_ack_delay_ns: 25_000_000,        // 25 ms
            max_tail_probes: 2,
        }
    }
}

/// Per-tick decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeDecision {
    /// No action (disarmed, guarded, not yet due, or budget exhausted).
    Idle,
    /// Retransmit the given oldest-outstanding source seq as probe round `probe_index` (0-based).
    ProbeDue { target_seq: u64, probe_index: u32 },
}

/// Bounded tail-recovery state machine. One instance per connection/sender.
#[derive(Debug, Clone)]
pub struct TailRecovery {
    cfg: TailRecoveryConfig,
    /// Start of the current idle window (last ACK progress or first arm). None ⇒ disarmed.
    armed_at_ns: Option<u64>,
    /// Probe rounds already fired in the current idle window.
    probe_index: u32,
    /// Time of the most recent ACK progress (for the reordering / delayed-ACK guard).
    last_progress_ns: u64,
    // ── observation-only metrics ──
    probe_count: u64,
    probe_bytes: u64,
    spurious_probe_count: u64,
    spurious_probe_bytes: u64,
    timer_fire_count: u64,
}

impl TailRecovery {
    #[must_use]
    pub fn new(cfg: TailRecoveryConfig) -> Self {
        Self {
            cfg,
            armed_at_ns: None,
            probe_index: 0,
            last_progress_ns: 0,
            probe_count: 0,
            probe_bytes: 0,
            spurious_probe_count: 0,
            spurious_probe_bytes: 0,
            timer_fire_count: 0,
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.cfg.enabled
    }

    /// Base PTO = clamp(srtt + 4·rttvar + max_ack_delay, MIN, MAX). No RTT sample ⇒ initial_pto (Karn: never from a
    /// retransmitted sample). Overflow-safe (saturating).
    #[must_use]
    pub fn base_pto_ns(&self, srtt_ns: Option<u64>, rttvar_ns: u64) -> u64 {
        match srtt_ns {
            None => self.cfg.initial_pto_ns,
            Some(srtt) => {
                let raw = srtt
                    .saturating_add(rttvar_ns.saturating_mul(4))
                    .saturating_add(self.cfg.max_ack_delay_ns);
                raw.clamp(self.cfg.min_pto_ns, self.cfg.max_pto_ns)
            }
        }
    }

    /// PTO for the current probe round = base · 2^probe_index, capped (bounded backoff; overflow-safe).
    #[must_use]
    pub fn current_pto_ns(&self, srtt_ns: Option<u64>, rttvar_ns: u64) -> u64 {
        let base = self.base_pto_ns(srtt_ns, rttvar_ns);
        let factor = 1u64.checked_shl(self.probe_index).unwrap_or(u64::MAX);
        base.saturating_mul(factor).min(self.cfg.max_backoff_pto_ns)
    }

    /// ACK progress: newly-acked ack-eliciting data resets/cancels the timer (RFC-flavored) and clears backoff.
    pub fn on_ack_progress(&mut self, now_ns: u64) {
        self.last_progress_ns = now_ns;
        self.armed_at_ns = Some(now_ns);
        self.probe_index = 0;
    }

    /// No outstanding ack-eliciting source ⇒ disarm (mechanism inert on a drained/no-loss flow).
    pub fn disarm(&mut self) {
        self.armed_at_ns = None;
        self.probe_index = 0;
    }

    /// Timer tick. `oldest_inflight_seq` = the oldest outstanding ack-eliciting SOURCE seq (None ⇒ nothing
    /// outstanding). Returns `ProbeDue` at most once per PTO; re-arms with exponential backoff; bounded by
    /// `max_tail_probes`. Disabled or drained ⇒ `Idle`.
    #[must_use]
    pub fn on_timer(
        &mut self,
        now_ns: u64,
        srtt_ns: Option<u64>,
        rttvar_ns: u64,
        oldest_inflight_seq: Option<u64>,
    ) -> ProbeDecision {
        self.timer_fire_count = self.timer_fire_count.saturating_add(1);
        if !self.cfg.enabled {
            return ProbeDecision::Idle;
        }
        let Some(target) = oldest_inflight_seq else {
            // nothing outstanding → never fires; normal-flow send untouched.
            self.disarm();
            return ProbeDecision::Idle;
        };
        // arm the idle window if not already armed for this outstanding data.
        let armed = *self
            .armed_at_ns
            .get_or_insert(now_ns.max(self.last_progress_ns));
        if self.probe_index >= self.cfg.max_tail_probes {
            return ProbeDecision::Idle; // bounded — never unbounded retransmission.
        }
        let pto = self.current_pto_ns(srtt_ns, rttvar_ns);
        // reordering / delayed-ACK guard: never probe within one PTO of the most recent ACK progress.
        if now_ns.saturating_sub(self.last_progress_ns) < pto {
            return ProbeDecision::Idle;
        }
        if now_ns.saturating_sub(armed) < pto {
            return ProbeDecision::Idle; // not yet due.
        }
        // DUE — but do NOT consume the attempt here (§3 probe-due ≠ probe-sent). The sender consumes it via
        // `on_probe_sent` ONLY after a successful emission (token available + target still outstanding). While a
        // probe is due but un-emitted (no token), on_timer keeps returning ProbeDue without advancing the cap.
        ProbeDecision::ProbeDue {
            target_seq: target,
            probe_index: self.probe_index,
        }
    }

    /// Consume one probe attempt AFTER a confirmed successful emission (token available, target still outstanding
    /// at emission time). Advances the round (exponential backoff) and re-arms the idle window. NOT called on a
    /// cancelled (ACK-race) or token-starved probe → the hard cap is spent only on real wire emissions (§3).
    pub fn on_probe_sent(&mut self, now_ns: u64) {
        self.probe_index = self.probe_index.saturating_add(1);
        self.armed_at_ns = Some(now_ns);
    }

    /// Record the outcome of a fired probe (observation-only). `spurious` = the unit was in fact deliverable
    /// (would have arrived without the probe).
    pub fn record_probe(&mut self, bytes: u64, spurious: bool) {
        self.probe_count = self.probe_count.saturating_add(1);
        self.probe_bytes = self.probe_bytes.saturating_add(bytes);
        if spurious {
            self.spurious_probe_count = self.spurious_probe_count.saturating_add(1);
            self.spurious_probe_bytes = self.spurious_probe_bytes.saturating_add(bytes);
        }
    }

    #[must_use]
    pub fn probe_count(&self) -> u64 {
        self.probe_count
    }
    #[must_use]
    pub fn probe_bytes(&self) -> u64 {
        self.probe_bytes
    }
    #[must_use]
    pub fn spurious_probe_count(&self) -> u64 {
        self.spurious_probe_count
    }
    #[must_use]
    pub fn spurious_probe_bytes(&self) -> u64 {
        self.spurious_probe_bytes
    }
    #[must_use]
    pub fn timer_fire_count(&self) -> u64 {
        self.timer_fire_count
    }
    #[must_use]
    pub fn probe_index(&self) -> u32 {
        self.probe_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_cfg() -> TailRecoveryConfig {
        TailRecoveryConfig {
            enabled: true,
            min_pto_ns: 1_000_000,
            max_pto_ns: 10_000_000_000,
            initial_pto_ns: 500_000_000,
            max_backoff_pto_ns: 60_000_000_000,
            max_ack_delay_ns: 0,
            max_tail_probes: 2,
        }
    }
    const RTT: u64 = 100_000_000; // 100 ms srtt
    const VAR: u64 = 0;

    #[test]
    fn disabled_never_fires() {
        let mut t = TailRecovery::new(TailRecoveryConfig::default()); // OFF
        t.on_ack_progress(0);
        for k in 0..50u64 {
            assert_eq!(
                t.on_timer(k * RTT * 10, Some(RTT), VAR, Some(7)),
                ProbeDecision::Idle
            );
        }
        assert_eq!(t.probe_index(), 0);
    }

    #[test]
    fn initial_disarmed_no_outstanding_idle() {
        let mut t = TailRecovery::new(on_cfg());
        assert_eq!(
            t.on_timer(10 * RTT, Some(RTT), VAR, None),
            ProbeDecision::Idle
        );
    }

    #[test]
    fn base_pto_clamp_and_no_sample() {
        let t = TailRecovery::new(on_cfg());
        // no sample → initial.
        assert_eq!(t.base_pto_ns(None, 0), 500_000_000);
        // srtt + 4·rttvar (+0 ack delay), clamped.
        assert_eq!(t.base_pto_ns(Some(RTT), 10_000_000), RTT + 40_000_000);
        // below floor → clamped to min.
        assert_eq!(t.base_pto_ns(Some(1), 0), 1_000_000);
        // above ceiling → clamped to max.
        assert_eq!(t.base_pto_ns(Some(u64::MAX), 0), 10_000_000_000);
    }

    #[test]
    fn arms_on_outstanding_fires_at_pto_boundary() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        // PTO = srtt = 100ms. Before boundary → idle.
        assert_eq!(
            t.on_timer(RTT - 1, Some(RTT), VAR, Some(3)),
            ProbeDecision::Idle
        );
        // at boundary → probe round 0 targets oldest.
        assert_eq!(
            t.on_timer(RTT, Some(RTT), VAR, Some(3)),
            ProbeDecision::ProbeDue {
                target_seq: 3,
                probe_index: 0
            }
        );
    }

    #[test]
    fn backoff_sequence_and_retry_cap() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        // round 0 at 1·PTO.
        assert!(matches!(
            t.on_timer(RTT, Some(RTT), VAR, Some(9)),
            ProbeDecision::ProbeDue { probe_index: 0, .. }
        ));
        t.on_probe_sent(RTT); // emission consumed the attempt.
                              // round 1 needs 2·PTO after re-arm at RTT → fires at RTT+2·RTT = 3·RTT.
        assert_eq!(
            t.on_timer(2 * RTT, Some(RTT), VAR, Some(9)),
            ProbeDecision::Idle
        );
        assert!(matches!(
            t.on_timer(3 * RTT, Some(RTT), VAR, Some(9)),
            ProbeDecision::ProbeDue { probe_index: 1, .. }
        ));
        t.on_probe_sent(3 * RTT);
        // cap = 2 → no more, forever (bounded).
        assert_eq!(
            t.on_timer(100 * RTT, Some(RTT), VAR, Some(9)),
            ProbeDecision::Idle
        );
        assert_eq!(
            t.on_timer(1000 * RTT, Some(RTT), VAR, Some(9)),
            ProbeDecision::Idle
        );
    }

    #[test]
    fn token_starved_probe_does_not_consume_cap() {
        // §3: probe DUE repeatedly but never emitted (no on_probe_sent) → cap NOT consumed; still due.
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        for k in 1..50u64 {
            assert!(matches!(
                t.on_timer(k * RTT, Some(RTT), VAR, Some(1)),
                ProbeDecision::ProbeDue { probe_index: 0, .. }
            ));
        }
        assert_eq!(
            t.probe_index(),
            0,
            "un-emitted probe must not consume the cap"
        );
    }

    #[test]
    fn ack_progress_resets_and_clears_backoff() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        assert!(matches!(
            t.on_timer(RTT, Some(RTT), VAR, Some(1)),
            ProbeDecision::ProbeDue { probe_index: 0, .. }
        ));
        t.on_probe_sent(RTT);
        // ACK progress resets window + backoff.
        t.on_ack_progress(RTT + 5);
        assert_eq!(t.probe_index(), 0);
        // not due until 1·PTO after progress.
        assert_eq!(
            t.on_timer(RTT + 5 + RTT - 1, Some(RTT), VAR, Some(2)),
            ProbeDecision::Idle
        );
        assert!(matches!(
            t.on_timer(RTT + 5 + RTT, Some(RTT), VAR, Some(2)),
            ProbeDecision::ProbeDue { probe_index: 0, .. }
        ));
    }

    #[test]
    fn drain_disarms() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        let _ = t.on_timer(RTT, Some(RTT), VAR, Some(1));
        // outstanding drained → disarm; re-arm fresh afterwards.
        assert_eq!(
            t.on_timer(2 * RTT, Some(RTT), VAR, None),
            ProbeDecision::Idle
        );
        assert_eq!(t.probe_index(), 0);
    }

    #[test]
    fn oldest_eligible_target_selected() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        // the sender supplies the oldest outstanding seq; the machine probes exactly that.
        assert_eq!(
            t.on_timer(RTT, Some(RTT), VAR, Some(41)),
            ProbeDecision::ProbeDue {
                target_seq: 41,
                probe_index: 0
            }
        );
    }

    #[test]
    fn delayed_ack_and_reordering_guard() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        // a late ACK (progress) at t just before a would-be probe must suppress it for one PTO.
        t.on_ack_progress(RTT - 10);
        assert_eq!(
            t.on_timer(RTT, Some(RTT), VAR, Some(2)),
            ProbeDecision::Idle
        );
        assert!(matches!(
            t.on_timer(RTT - 10 + RTT, Some(RTT), VAR, Some(2)),
            ProbeDecision::ProbeDue { .. }
        ));
    }

    #[test]
    fn rtt_step_up_scales_pto() {
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        let big = 900_000_000u64;
        // PTO now 900ms → not due at 100ms.
        assert_eq!(
            t.on_timer(RTT, Some(big), VAR, Some(1)),
            ProbeDecision::Idle
        );
        assert!(matches!(
            t.on_timer(big, Some(big), VAR, Some(1)),
            ProbeDecision::ProbeDue { .. }
        ));
    }

    #[test]
    fn variance_widens_pto() {
        let t = TailRecovery::new(on_cfg());
        assert_eq!(t.base_pto_ns(Some(RTT), 50_000_000), RTT + 200_000_000);
    }

    #[test]
    fn overflow_safe() {
        let mut t = TailRecovery::new(TailRecoveryConfig {
            enabled: true,
            min_pto_ns: 1,
            max_pto_ns: u64::MAX,
            initial_pto_ns: u64::MAX,
            max_backoff_pto_ns: u64::MAX,
            max_ack_delay_ns: u64::MAX,
            max_tail_probes: 40,
        });
        // no panic on extreme values.
        let _ = t.base_pto_ns(Some(u64::MAX), u64::MAX);
        t.on_ack_progress(u64::MAX);
        let _ = t.on_timer(u64::MAX, Some(u64::MAX), u64::MAX, Some(u64::MAX));
        let _ = t.current_pto_ns(Some(u64::MAX), u64::MAX);
    }

    #[test]
    fn determinism_thrice() {
        let run = || {
            let mut t = TailRecovery::new(on_cfg());
            t.on_ack_progress(0);
            let mut trace = Vec::new();
            for k in 0..12u64 {
                let d = t.on_timer(k * RTT, Some(RTT), VAR, if k < 10 { Some(5) } else { None });
                if let ProbeDecision::ProbeDue { .. } = d {
                    t.on_probe_sent(k * RTT);
                }
                trace.push(d);
            }
            (trace, t.probe_index())
        };
        let a = run();
        let b = run();
        let c = run();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn probe_loss_bounded_retry() {
        // probe itself lost → no progress → backoff re-probe up to cap, then bounded failure.
        let mut t = TailRecovery::new(on_cfg());
        t.on_ack_progress(0);
        let mut probes = 0;
        for k in 1..500u64 {
            if let ProbeDecision::ProbeDue { .. } = t.on_timer(k * RTT, Some(RTT), VAR, Some(1)) {
                probes += 1;
                t.on_probe_sent(k * RTT); // probe went to wire (then was lost — no progress).
            }
        }
        assert_eq!(probes, on_cfg().max_tail_probes);
    }

    #[test]
    fn spurious_accounting() {
        let mut t = TailRecovery::new(on_cfg());
        t.record_probe(101, false);
        t.record_probe(101, true);
        assert_eq!(t.probe_count(), 2);
        assert_eq!(t.probe_bytes(), 202);
        assert_eq!(t.spurious_probe_count(), 1);
        assert_eq!(t.spurious_probe_bytes(), 101);
    }
}
