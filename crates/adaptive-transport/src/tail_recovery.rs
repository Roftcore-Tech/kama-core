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
        //
        // NOTE these bounds are NOT the validated ones: the DR-ARQ-PRODUCT-TAIL-1 grid ran
        // [`Self::validated_v1`] (cap 3, 10 ms / 10 s / 30 s), and this default (cap 2, 1 ms /
        // 60 s / 120 s) was never driven by any campaign. Enabling tail recovery therefore goes
        // through `Sender::set_tail_recovery_config(TailRecoveryConfig::validated_v1())` — never
        // by flipping `enabled` here, which would ship an unvalidated configuration.
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

impl TailRecoveryConfig {
    /// KAMA-TAIL-V1 — the exact configuration the product validation ran, promoted to a named
    /// product constant so enabling tail recovery cannot silently ship different bounds.
    ///
    /// Provenance: DR-ARQ-PRODUCT-TAIL-1, faithful two-netns/netem, 2290/2290 rows, verdict
    /// `GO — MECHANISM CORRECT, POLICY/TUNING PENDING` (frozen grid config in
    /// `examples/dr_arqtail_product.rs`; freeze record
    /// `reports/dr-arq-product-tail-1/FIDELITY-AND-FREEZE.md`, source `c46e8fa3`, manifest
    /// `bf849aba…`). Terminal tail: OFF 8/360 (2.2%) vs ON 329/360 (91.4%); global max
    /// `probe_sent` = 3 = this cap; `inv_fail` = 0; `spurious` = 0.
    ///
    /// Known open tuning (recorded, NOT fixed here — the comparison campaign runs this config
    /// as-is, per the handoff rule "do not post-hoc tune Tail-v1 before the comparison"):
    /// `terminal-tail,10,med` exhausts the fixed cap before packet-threshold ARQ engages (0/30);
    /// adaptive cap/backoff is product-v2 backlog.
    #[must_use]
    pub fn validated_v1() -> Self {
        Self {
            enabled: true,
            min_pto_ns: 10_000_000,             // 10 ms floor
            max_pto_ns: 10_000_000_000,         // 10 s base ceiling
            initial_pto_ns: 1_000_000_000,      // 1 s when no RTT sample (Karn)
            max_backoff_pto_ns: 30_000_000_000, // 30 s absolute backoff ceiling
            max_ack_delay_ns: 25_000_000,       // 25 ms
            max_tail_probes: 3,
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
    /// Times `on_timer` returned `ProbeDue` (probe eligible/scheduled). `probe_due_count - probe_count`
    /// = probes that were due but not emitted (token-starved / ACK-race cancelled). Observation-only.
    probe_due_count: u64,
    /// DR-CS-2 (observation-only): `not-due → due` TRANSITIONS. `probe_due_count` is level-triggered
    /// (it grows with the timer poll rate while a probe stays due-but-unemitted) and therefore MUST NOT
    /// be read as "probes wanted"; this edge counter is the demand signal.
    probe_due_edge_count: u64,
    /// Whether the previous `on_timer` returned `ProbeDue` (edge detection state).
    was_due: bool,
    /// [Q1 A-5] RFC 9002 §6.2 / quinn-proto 0.11.18 PTO mode (default false = shipped behaviour):
    /// anchor = last ack-eliciting send, no per-window probe cap, base `srtt + max(4·var, 1 ms) + mad`.
    rfc_mode: bool,
    /// [Q1 A-5] Time of the most recent ack-eliciting send (`Sender::track_sent`). Written always,
    /// read only in `rfc_mode` (quinn `time_of_last_ack_eliciting_packet`).
    last_ack_eliciting_send_ns: Option<u64>,
}

/// [Q1 A-5] RFC 9002 kGranularity (1 ms) — quinn-proto 0.11.18 `lib.rs:334` `TIMER_GRANULARITY`.
const PTO_RFC_GRANULARITY_NS: u64 = 1_000_000;

impl TailRecovery {
    /// [FAZ-3 TELEMETRİ] Yürürlükteki yapılandırma (salt gözlem; "yapılandırıldı" olgusu
    /// "ateşledi" olgusundan AYRIDIR).
    #[must_use]
    pub fn config(&self) -> TailRecoveryConfig {
        self.cfg
    }

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
            probe_due_count: 0,
            probe_due_edge_count: 0,
            was_due: false,
            rfc_mode: false,
            last_ack_eliciting_send_ns: None,
        }
    }

    /// [Q1 A-5] RFC/quinn PTO mode switch (`E2aFlags::pto_rfc`). Off = shipped behaviour bit-for-bit.
    pub fn set_rfc_mode(&mut self, on: bool) {
        self.rfc_mode = on;
    }

    /// [Q1 A-5] Whether RFC/quinn PTO mode is on.
    #[must_use]
    pub fn rfc_mode(&self) -> bool {
        self.rfc_mode
    }

    /// [Q1 A-5] Record an ack-eliciting emission (data / retransmit / probe / duplicate). Pure
    /// bookkeeping; read only in `rfc_mode`.
    pub fn note_ack_eliciting_sent(&mut self, now_ns: u64) {
        self.last_ack_eliciting_send_ns = Some(now_ns);
    }

    /// EK4-A2 + [Q1 A-5]: absolute PTO deadline of the current round (read-only). Shipped mode:
    /// `armed_at + current_pto`. RFC mode: `last_ack_eliciting_send + current_pto` (quinn-proto
    /// 0.11.18 `connection/mod.rs` `pto_time_and_space`; MIT/Apache-2.0). `None` ⇒ disarmed.
    #[must_use]
    pub fn pto_deadline_ns(&self, srtt_ns: Option<u64>, rttvar_ns: u64) -> Option<u64> {
        let armed = self.armed_at_ns?;
        let anchor = if self.rfc_mode {
            self.last_ack_eliciting_send_ns.unwrap_or(armed)
        } else {
            armed
        };
        Some(anchor.saturating_add(self.current_pto_ns(srtt_ns, rttvar_ns)))
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
                // [Q1 A-5] RFC mode: max(4·rttvar, kGranularity) (quinn `paths.rs` `pto_base`).
                let var_term = if self.rfc_mode {
                    rttvar_ns.saturating_mul(4).max(PTO_RFC_GRANULARITY_NS)
                } else {
                    rttvar_ns.saturating_mul(4)
                };
                let raw = srtt
                    .saturating_add(var_term)
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

    /// EK4-A2 (event-loop liveness): armed idle-window başlangıcı (SALT-OKUMA; yeni state DEĞİL).
    /// `next_wakeup_ns()` PTO deadline'ını `armed_at_ns + current_pto_ns(..)` ile türetir.
    #[must_use]
    pub fn armed_at_ns(&self) -> Option<u64> {
        self.armed_at_ns
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
            self.was_due = false; // edge-detect: leaving the due state
            return ProbeDecision::Idle;
        }
        let Some(target) = oldest_inflight_seq else {
            // nothing outstanding → never fires; normal-flow send untouched.
            self.disarm();
            self.was_due = false; // edge-detect: leaving the due state
            return ProbeDecision::Idle;
        };
        // arm the idle window if not already armed for this outstanding data.
        let armed = *self
            .armed_at_ns
            .get_or_insert(now_ns.max(self.last_progress_ns));
        // [Q1 A-5] RFC mode: no per-window probe cap (RFC 9002 has backoff, not a cap; the
        // backed-off PTO stays bounded by `max_backoff_pto_ns`, so probes are rate-bounded).
        if !self.rfc_mode && self.probe_index >= self.cfg.max_tail_probes {
            self.was_due = false; // edge-detect: leaving the due state
            return ProbeDecision::Idle; // bounded — never unbounded retransmission.
        }
        let pto = self.current_pto_ns(srtt_ns, rttvar_ns);
        if self.rfc_mode {
            // [Q1 A-5] quinn-proto 0.11.18 `pto_time_and_space` (MIT/Apache-2.0): deadline =
            // last ack-eliciting send + PTO·2^count. The probe itself is an ack-eliciting send, so
            // the next round re-anchors on it (backoff). No separate ACK-progress guard: progress
            // resets `probe_index` (quinn `pto_count = 0`) via `on_ack_progress`.
            let anchor = self.last_ack_eliciting_send_ns.unwrap_or(armed);
            if now_ns.saturating_sub(anchor) < pto {
                self.was_due = false; // edge-detect: leaving the due state
                return ProbeDecision::Idle; // not yet due.
            }
        } else {
            // reordering / delayed-ACK guard: never probe within one PTO of the most recent ACK progress.
            if now_ns.saturating_sub(self.last_progress_ns) < pto {
                self.was_due = false; // edge-detect: leaving the due state
                return ProbeDecision::Idle;
            }
            if now_ns.saturating_sub(armed) < pto {
                self.was_due = false; // edge-detect: leaving the due state
                return ProbeDecision::Idle; // not yet due.
            }
        }
        // DUE — but do NOT consume the attempt here (§3 probe-due ≠ probe-sent). The sender consumes it via
        // `on_probe_sent` ONLY after a successful emission (token available + target still outstanding). While a
        // probe is due but un-emitted (no token), on_timer keeps returning ProbeDue without advancing the cap.
        self.probe_due_count = self.probe_due_count.saturating_add(1); // observation-only (§7 probe_due) — LEVEL.
        if !self.was_due {
            self.probe_due_edge_count = self.probe_due_edge_count.saturating_add(1);
            // EDGE (demand).
        }
        self.was_due = true;
        ProbeDecision::ProbeDue {
            target_seq: target,
            probe_index: self.probe_index,
        }
    }

    /// Observation-only: `not-due → due` TRANSITION count (the demand signal). Unlike `probe_due_count`,
    /// this does NOT grow with the timer poll rate.
    #[must_use]
    pub fn probe_due_edge_count(&self) -> u64 {
        self.probe_due_edge_count
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
    /// §7 probe_due: times a probe was eligible (`ProbeDue`), independent of whether it was emitted.
    #[must_use]
    pub fn probe_due_count(&self) -> u64 {
        self.probe_due_count
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

    /// KAMA-TAIL-V1 pin: the named product constant must stay byte-equal to the configuration the
    /// 2290-row validation grid actually ran (frozen in examples/dr_arqtail_product.rs). If this
    /// fails, someone re-tuned the constant without a campaign — the exact drift the name exists
    /// to prevent. The default stays distinct and OFF: enabling goes through validated_v1, never
    /// through flipping the (never-validated) default bounds.
    #[test]
    fn validated_v1_pins_the_grid_config_and_default_stays_off() {
        let v = TailRecoveryConfig::validated_v1();
        assert!(v.enabled);
        assert_eq!(
            (
                v.min_pto_ns,
                v.max_pto_ns,
                v.initial_pto_ns,
                v.max_backoff_pto_ns,
                v.max_ack_delay_ns,
                v.max_tail_probes,
            ),
            (
                10_000_000,
                10_000_000_000,
                1_000_000_000,
                30_000_000_000,
                25_000_000,
                3,
            ),
            "KAMA-TAIL-V1 drifted from the validated grid config"
        );
        let d = TailRecoveryConfig::default();
        assert!(!d.enabled, "shipping default must remain OFF");
        assert_ne!(
            d.max_tail_probes, v.max_tail_probes,
            "the default (cap 2) was never validated; it must not silently converge on v1"
        );
    }
}

// ── [Q1 A-5] RFC 9002 / quinn-proto 0.11.18 PTO mode: anchor = last ack-eliciting send, no cap ──
#[cfg(test)]
mod q1_pto_rfc_tests {
    use super::*;

    const RTT: u64 = 100_000_000; // 100 ms srtt
    const VAR: u64 = 0;

    fn cfg() -> TailRecoveryConfig {
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

    fn machine(rfc: bool) -> TailRecovery {
        let mut t = TailRecovery::new(cfg());
        t.set_rfc_mode(rfc);
        t
    }

    #[test]
    fn off_is_default_and_base_pto_unchanged() {
        let t = TailRecovery::new(cfg());
        assert!(!t.rfc_mode());
        // Shipped base: srtt + 4·var (+mad). RFC: srtt + max(4·var, 1 ms) (+mad).
        assert_eq!(t.base_pto_ns(Some(RTT), 0), RTT);
        assert_eq!(machine(true).base_pto_ns(Some(RTT), 0), RTT + 1_000_000);
        assert_eq!(
            machine(true).base_pto_ns(Some(RTT), 10_000_000),
            RTT + 40_000_000,
            "4·var > 1 ms → same as shipped"
        );
    }

    #[test]
    fn anchor_is_last_send_not_ack_progress() {
        // ACK progress at 0, last ack-eliciting send at 50 ms.
        for rfc in [false, true] {
            let mut t = machine(rfc);
            t.on_ack_progress(0);
            t.note_ack_eliciting_sent(50_000_000);
            let pto = t.current_pto_ns(Some(RTT), VAR);
            let due_at = if rfc { 50_000_000 + pto } else { pto };
            assert_eq!(
                t.pto_deadline_ns(Some(RTT), VAR),
                Some(due_at),
                "rfc={rfc}: deadline"
            );
            assert_eq!(
                t.on_timer(due_at - 1, Some(RTT), VAR, Some(4)),
                ProbeDecision::Idle,
                "rfc={rfc}: before deadline"
            );
            assert_eq!(
                t.on_timer(due_at, Some(RTT), VAR, Some(4)),
                ProbeDecision::ProbeDue {
                    target_seq: 4,
                    probe_index: 0
                },
                "rfc={rfc}: at deadline"
            );
        }
    }

    #[test]
    fn rfc_has_no_round_cap_and_backs_off_from_each_probe() {
        let mut off = machine(false);
        let mut on = machine(true);
        for t in [&mut off, &mut on] {
            t.on_ack_progress(0);
            t.note_ack_eliciting_sent(0);
        }
        // Drive 6 rounds: each probe is itself an ack-eliciting send (re-anchor) and doubles PTO.
        let mut now = 0u64;
        let mut fired_on = Vec::new();
        for _ in 0..6 {
            let dl = on.pto_deadline_ns(Some(RTT), VAR).expect("armed");
            now = now.max(dl);
            if let ProbeDecision::ProbeDue { probe_index, .. } =
                on.on_timer(now, Some(RTT), VAR, Some(1))
            {
                fired_on.push((now, probe_index));
                on.on_probe_sent(now);
                on.note_ack_eliciting_sent(now);
            }
        }
        // PTO_k = (RTT + 1 ms)·2^k; deadlines 101, 101+202, ... ms.
        let base = RTT + 1_000_000;
        let mut expect = Vec::new();
        let mut t = 0u64;
        for k in 0..6u32 {
            t += base << k;
            expect.push((t, k));
        }
        assert_eq!(fired_on, expect, "uncapped rounds with doubling backoff");
        // Shipped mode stops after max_tail_probes = 2 rounds.
        let mut n_off = 0;
        let mut now = 0u64;
        for k in 0..60u64 {
            now += RTT * (1 << (k % 8));
            if let ProbeDecision::ProbeDue { .. } = off.on_timer(now, Some(RTT), VAR, Some(1)) {
                off.on_probe_sent(now);
                off.note_ack_eliciting_sent(now);
                n_off += 1;
            }
        }
        assert_eq!(n_off, 2, "shipped cap = 2 rounds");
    }

    #[test]
    fn rfc_backoff_bounded_by_ceiling_and_progress_resets() {
        let mut t = machine(true);
        t.on_ack_progress(0);
        for _ in 0..100 {
            t.on_probe_sent(0);
        }
        assert_eq!(
            t.current_pto_ns(Some(RTT), VAR),
            cfg().max_backoff_pto_ns,
            "probe_index ≫ 64 → saturating, capped (overflow-safe)"
        );
        t.on_ack_progress(5);
        assert_eq!(
            t.probe_index(),
            0,
            "ACK progress resets the round (quinn pto_count = 0)"
        );
    }
}
