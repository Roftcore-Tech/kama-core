//! DR-ARQ-TAIL-1 — EXPERIMENT-ONLY bounded tail probe (NOT a product mechanism; NOT an RFC 9002 adaptive-PTO
//! compliance claim). Pure deterministic state machine (RNG-free; replay-identical). Fills the tail-loss gap the
//! current ACK-driven ARQ cannot recover (last source lost → no later ACK reveals the gap).
//!
//! Executability basis (§1): the frozen model exposes send timestamps, ACK progress, outstanding un-acked source,
//! a deterministic known RTT (2·owd), and NO reordering (FIFO per-packet drop) → tail-loss is unambiguous.
//!
//! Invariants (§2): arms ONLY while outstanding source data exists · cancels/resets on ACK progress · BOUNDED total
//! probes (never unbounded) · not tied to FEC · measures spurious probes · does NOT change normal-flow send behavior
//! (fires only on tail idle). Timers are cancelled/re-armed deterministically from live ACK-progress state.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
pub struct TailProbeCfg {
    /// PTO = pto_mult · rtt_ns + max_ack_delay_ns (conservative; experiment-only).
    pub pto_mult: u64,
    pub max_ack_delay_ns: u64,
    /// hard upper bound on total probe rounds (never unbounded).
    pub max_probes: u32,
}
impl TailProbeCfg {
    #[must_use]
    pub fn conservative() -> Self {
        // 2·RTT + max_ack_delay, at most 2 probe rounds (bounded; experiment-only).
        Self {
            pto_mult: 2,
            max_ack_delay_ns: 0,
            max_probes: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeDecision {
    /// no action this tick (disarmed, not yet due, cancelled, or budget exhausted).
    Idle,
    /// fire a bounded tail-probe retransmission round for the outstanding source set.
    Probe { round: u32 },
}

/// Deterministic tail-probe state machine. Driven per tick with (now, rtt_ns, outstanding_count, acked_progressed).
#[derive(Debug, Clone)]
pub struct TailProbe {
    cfg: TailProbeCfg,
    armed_since: Option<u64>, // when the current idle interval started (last progress or arm)
    probes_sent: u32,
    spurious_probes: u32,
    last_progress: u64,
    total_retransmitted: u64,
}
impl TailProbe {
    #[must_use]
    pub fn new(cfg: TailProbeCfg) -> Self {
        Self {
            cfg,
            armed_since: None,
            probes_sent: 0,
            spurious_probes: 0,
            last_progress: 0,
            total_retransmitted: 0,
        }
    }

    #[must_use]
    pub fn pto_ns(&self, rtt_ns: u64) -> u64 {
        self.cfg.pto_mult.saturating_mul(rtt_ns) + self.cfg.max_ack_delay_ns
    }

    /// ACK progress cancels/resets the timer (RFC-flavored: any newly-acked ack-eliciting data resets PTO).
    pub fn on_ack_progress(&mut self, now: u64) {
        self.last_progress = now;
        self.armed_since = Some(now); // re-arm from the progress point
    }

    /// Per-tick decision. `outstanding` = count of ack-eliciting source units still un-acked/un-delivered.
    /// Returns whether to fire a bounded probe this tick. Arms ONLY when outstanding>0; disarms at outstanding==0.
    pub fn on_tick(&mut self, now: u64, rtt_ns: u64, outstanding: u32) -> ProbeDecision {
        if outstanding == 0 {
            self.armed_since = None; // nothing outstanding → never fires (normal-flow untouched)
            return ProbeDecision::Idle;
        }
        // arm if not already armed for this idle interval.
        let armed = *self.armed_since.get_or_insert(now.max(self.last_progress));
        if self.probes_sent >= self.cfg.max_probes {
            return ProbeDecision::Idle; // bounded — never unbounded
        }
        if now.saturating_sub(armed) >= self.pto_ns(rtt_ns) {
            self.probes_sent += 1;
            // re-arm the idle window from now (exponential-flavored: next probe waits another PTO).
            self.armed_since = Some(now);
            return ProbeDecision::Probe {
                round: self.probes_sent,
            };
        }
        ProbeDecision::Idle
    }

    /// Record the outcome of a probe round: how many units were retransmitted and how many were SPURIOUS
    /// (the unit was not actually lost — it would have arrived without the probe).
    pub fn record_probe(&mut self, retransmitted: u64, spurious: u32) {
        self.total_retransmitted += retransmitted;
        self.spurious_probes += spurious;
    }

    #[must_use]
    pub fn probes_sent(&self) -> u32 {
        self.probes_sent
    }
    #[must_use]
    pub fn spurious_probes(&self) -> u32 {
        self.spurious_probes
    }
    #[must_use]
    pub fn total_retransmitted(&self) -> u64 {
        self.total_retransmitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TailProbeCfg {
        TailProbeCfg::conservative()
    }
    const RTT: u64 = 100_000_000; // 100ms

    /// §3 last-source-packet loss: outstanding tail persists → probe fires after PTO (bounded).
    #[test]
    fn last_source_loss_triggers_bounded_probe() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        // before PTO: idle.
        assert_eq!(tp.on_tick(RTT, RTT, 1), ProbeDecision::Idle);
        // at 2·RTT since progress: first probe.
        assert_eq!(
            tp.on_tick(2 * RTT, RTT, 1),
            ProbeDecision::Probe { round: 1 }
        );
        // after re-arm, another PTO → second probe.
        assert_eq!(
            tp.on_tick(4 * RTT, RTT, 1),
            ProbeDecision::Probe { round: 2 }
        );
        // budget exhausted (max_probes=2) → idle forever (BOUNDED; never unbounded).
        assert_eq!(tp.on_tick(10 * RTT, RTT, 1), ProbeDecision::Idle);
        assert_eq!(tp.on_tick(100 * RTT, RTT, 1), ProbeDecision::Idle);
        assert_eq!(tp.probes_sent(), 2);
    }

    /// §3 last-ACK loss: still looks like outstanding tail → probe fires (same mechanism; correct behavior).
    #[test]
    fn last_ack_loss_behaves_as_outstanding_tail() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        assert_eq!(
            tp.on_tick(2 * RTT, RTT, 1),
            ProbeDecision::Probe { round: 1 }
        );
    }

    /// §3 long-but-valid RTT: PTO scales with RTT → no premature probe.
    #[test]
    fn long_valid_rtt_no_premature_probe() {
        let mut tp = TailProbe::new(cfg());
        let long = 900_000_000u64; // 900ms
        tp.on_ack_progress(0);
        // at 2·RTT(100ms)=200ms but RTT is 900ms → PTO=1.8s → NOT due.
        assert_eq!(tp.on_tick(200_000_000, long, 1), ProbeDecision::Idle);
        // due only at 2·900ms.
        assert_eq!(
            tp.on_tick(1_800_000_000, long, 1),
            ProbeDecision::Probe { round: 1 }
        );
    }

    /// §3 no-loss flow: outstanding drops to 0 → NEVER fires (normal-flow send behavior unchanged).
    #[test]
    fn no_loss_flow_never_probes() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        // everything acked → outstanding 0 forever.
        for t in 0..20 {
            assert_eq!(tp.on_tick(t * RTT, RTT, 0), ProbeDecision::Idle);
        }
        assert_eq!(tp.probes_sent(), 0);
    }

    /// §3 timer cancellation: ACK progress before PTO resets the timer → no probe.
    #[test]
    fn ack_progress_cancels_timer() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        assert_eq!(tp.on_tick(RTT, RTT, 2), ProbeDecision::Idle); // 1·RTT, not due
        tp.on_ack_progress(RTT + 1); // progress resets
                                     // now 2·RTT since start but only 1·RTT-ish since progress → not due.
        assert_eq!(tp.on_tick(2 * RTT, RTT, 1), ProbeDecision::Idle);
        // due 2·RTT after the progress point.
        assert_eq!(
            tp.on_tick(RTT + 1 + 2 * RTT, RTT, 1),
            ProbeDecision::Probe { round: 1 }
        );
    }

    /// §3 multiple outstanding: probe still bounded regardless of outstanding count.
    #[test]
    fn multiple_outstanding_bounded() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        assert_eq!(
            tp.on_tick(2 * RTT, RTT, 5),
            ProbeDecision::Probe { round: 1 }
        );
        assert_eq!(
            tp.on_tick(4 * RTT, RTT, 5),
            ProbeDecision::Probe { round: 2 }
        );
        assert_eq!(tp.on_tick(6 * RTT, RTT, 5), ProbeDecision::Idle);
    }

    /// §3 bounded retry: never exceeds max_probes even over a long horizon.
    #[test]
    fn bounded_retry_hard_cap() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        for t in 1..1000u64 {
            let _ = tp.on_tick(t * RTT, RTT, 3);
        }
        assert!(tp.probes_sent() <= cfg().max_probes);
    }

    /// §3 spurious recovery cost is accounted.
    #[test]
    fn spurious_cost_accounted() {
        let mut tp = TailProbe::new(cfg());
        tp.on_ack_progress(0);
        assert_eq!(
            tp.on_tick(2 * RTT, RTT, 2),
            ProbeDecision::Probe { round: 1 }
        );
        tp.record_probe(2, 1); // retransmitted 2, one of which was spurious (would have arrived)
        assert_eq!(tp.total_retransmitted(), 2);
        assert_eq!(tp.spurious_probes(), 1);
    }

    /// §3 RNG/order determinism: identical driving sequence → identical decisions × 3.
    #[test]
    fn determinism_thrice() {
        let run = || {
            let mut tp = TailProbe::new(cfg());
            tp.on_ack_progress(0);
            let mut trace = Vec::new();
            for t in 0..10u64 {
                trace.push(tp.on_tick(t * RTT, RTT, if t < 8 { 1 } else { 0 }));
            }
            (trace, tp.probes_sent())
        };
        let a = run();
        let b = run();
        let c = run();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }
}
