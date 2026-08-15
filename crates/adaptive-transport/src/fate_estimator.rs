//! FS-CAL-2 v3 §3 — prefix-causal loss-structure fate estimator (pure, deterministic, integer-only).
//!
//! Normative source: `docs/design/FS-CAL-2-EXPERIMENT-DESIGN-v3 (1).md`
//! (sha256 `115fa266a40f744013d209d36711a698096a28509ede11dd417f23aa5cdb3393`), §3.1–§3.4.
//!
//! Per original source packet a three-state fate is tracked: `UNKNOWN` → (`SUCCESS` | `LOSS`).
//! * `SUCCESS`: receiver ACK evidence that the ORIGINAL delivery needed no recovery action.
//! * `LOSS`: sender loss/recovery lineage or receiver missing/recovered-source evidence — but a
//!   packet may NOT be finalized `LOSS` before the reordering guard elapses (§3.1). A late
//!   original-ACK inside the guard corrects the provisional loss to `SUCCESS` and increments
//!   `spurious_loss_corrections`. An ACK arriving after `LOSS` finalization cannot mutate the
//!   window (immutable once fate-known) and increments `late_ack_corrections` (observability).
//! * A logical source id can enter the window at most once (§3.1). Packets still unresolved at
//!   the horizon stay `UNKNOWN` and never enter the estimator denominator (§3.1).
//!
//! Reordering guard (§3.1): `clamp(srtt + 4·rttvar + max_ack_delay, min_pto, max_pto)` — the SAME
//! bounded-PTO base as the validated product tail probe. This module does not re-derive it: callers
//! obtain it from the product symbol [`crate::tail_recovery::TailRecovery::base_pto_ns`] (see
//! [`reordering_guard_ns`]). The guard is snapshotted at loss-declaration time.
//!
//! Window/counters (§3.2): the last [`FATE_WINDOW`] fate-known bits in logical source order;
//! `N, lost, n00, n01, n10, n11`; a transition is counted only when BOTH consecutive logical ids
//! (`id`, `id+1`) are fate-known members of the window (§3.2). Formulas (§3.3, Laplace-smoothed,
//! integer floor, ppm): `p11 = (n11+1)/(n10+n11+2)`, `p01 = (n01+1)/(n00+n01+2)`,
//! `burst_delta = p11 − p01`. Eligibility thresholds (§3.4) are PINNED for FS-CAL-2 v3; changing
//! any of them requires a new design/calibration version (design §3.4).
//!
//! This module is observation-only: nothing in the product engine calls it; it never mutates
//! ARQ/ACK/loss/RTT/PTO state. RNG-free, wall-clock-free, overflow-safe.

use crate::tail_recovery::{TailRecovery, TailRecoveryConfig};
use std::collections::BTreeMap;

/// §3.2 window size (fate-known bits), PINNED for FS-CAL-2 v3.
pub const FATE_WINDOW: usize = 256;
/// §3.4 minimum loss-adjacent transitions `n10 + n11`, PINNED.
pub const MIN_LOSS_TRANSITIONS: u32 = 8;
/// §3.4 minimum non-loss transitions `n00 + n01`, PINNED.
pub const MIN_NONLOSS_TRANSITIONS: u32 = 32;
/// ppm scale.
pub const PPM: u64 = 1_000_000;

/// §3.1 reordering guard from the PRODUCT bounded-PTO base symbol (`TailRecovery::base_pto_ns`):
/// `clamp(srtt + 4·rttvar + max_ack_delay, min_pto, max_pto)`; no RTT sample ⇒ `initial_pto_ns`
/// (the same conservative product branch — an unconverged flow is estimator-ineligible anyway).
#[must_use]
pub fn reordering_guard_ns(cfg: &TailRecoveryConfig, srtt_ns: Option<u64>, rttvar_ns: u64) -> u64 {
    TailRecovery::new(*cfg).base_pto_ns(srtt_ns, rttvar_ns)
}

/// Finalized fate of one original source packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Original delivered without any recovery action.
    Success,
    /// Original required recovery (retransmit lineage / recovered-source evidence), guard elapsed.
    Loss,
}

/// §3.2/§3.3 window features at a decision epoch. All integer; `window_ids`/`window_fates` are the
/// raw fate snapshot (serialized per §10 so every feature is recomputable from scratch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFeatures {
    /// Fate-known count in the window (≤ [`FATE_WINDOW`]).
    pub n: u32,
    /// LOSS count in the window.
    pub lost: u32,
    /// SUCCESS→SUCCESS transitions.
    pub n00: u32,
    /// SUCCESS→LOSS transitions.
    pub n01: u32,
    /// LOSS→SUCCESS transitions.
    pub n10: u32,
    /// LOSS→LOSS transitions.
    pub n11: u32,
    /// `floor(lost·1e6/N)`; 0 when `N == 0`.
    pub measured_loss_ppm: u64,
    /// Laplace `floor((n11+1)·1e6/(n10+n11+2))`.
    pub p11_ppm: u64,
    /// Laplace `floor((n01+1)·1e6/(n00+n01+2))`.
    pub p01_ppm: u64,
    /// `p11_ppm − p01_ppm` (signed).
    pub burst_delta_ppm: i64,
    /// Window logical source ids, ascending.
    pub window_ids: Vec<u64>,
    /// Window fates aligned with `window_ids`; `true` = LOSS.
    pub window_fates: Vec<bool>,
}

impl WindowFeatures {
    /// §3.4 estimator eligibility (thresholds PINNED for FS-CAL-2 v3). Runtime flags are supplied
    /// by the caller (warm-up completion, telemetry staleness, flow convergence).
    #[must_use]
    pub fn eligible(
        &self,
        warmup_complete: bool,
        telemetry_stale: bool,
        flow_converged: bool,
    ) -> bool {
        self.n as usize == FATE_WINDOW
            && self.n10 + self.n11 >= MIN_LOSS_TRANSITIONS
            && self.n00 + self.n01 >= MIN_NONLOSS_TRANSITIONS
            && warmup_complete
            && !telemetry_stale
            && flow_converged
    }
}

/// Compute §3.2/§3.3 features from an ordered fate-bit slice (`true` = LOSS) whose logical ids are
/// `id0..id0+bits.len()` and are ALL fate-known. Used by S0 (synthetic traces, fates known by
/// definition) and by the analysis pipeline's from-scratch recomputation. The window is the LAST
/// [`FATE_WINDOW`] bits (§2.3 "son 256 fate-known").
#[must_use]
pub fn features_from_fate_bits(id0: u64, bits: &[bool]) -> WindowFeatures {
    let start = bits.len().saturating_sub(FATE_WINDOW);
    let win = &bits[start..];
    let ids: Vec<u64> = (0..win.len() as u64)
        .map(|k| id0 + start as u64 + k)
        .collect();
    features_from_window(&ids, win)
}

/// Core counter/feature computation over an explicit (ids, fates) window. Transitions count only
/// consecutive logical ids (`id+1` present). Pure, deterministic.
#[must_use]
pub fn features_from_window(ids: &[u64], fates: &[bool]) -> WindowFeatures {
    debug_assert_eq!(ids.len(), fates.len());
    let n = ids.len() as u32;
    let lost = fates.iter().filter(|&&l| l).count() as u32;
    let (mut n00, mut n01, mut n10, mut n11) = (0u32, 0u32, 0u32, 0u32);
    for i in 0..ids.len() {
        // consecutive logical id present as the NEXT window entry?
        if i + 1 < ids.len() && ids[i + 1] == ids[i] + 1 {
            match (fates[i], fates[i + 1]) {
                (false, false) => n00 += 1,
                (false, true) => n01 += 1,
                (true, false) => n10 += 1,
                (true, true) => n11 += 1,
            }
        }
    }
    let measured_loss_ppm = if n > 0 {
        u64::from(lost) * PPM / u64::from(n)
    } else {
        0
    };
    let p11_ppm = (u64::from(n11) + 1) * PPM / (u64::from(n10) + u64::from(n11) + 2);
    let p01_ppm = (u64::from(n01) + 1) * PPM / (u64::from(n00) + u64::from(n01) + 2);
    WindowFeatures {
        n,
        lost,
        n00,
        n01,
        n10,
        n11,
        measured_loss_ppm,
        p11_ppm,
        p01_ppm,
        burst_delta_ppm: p11_ppm as i64 - p01_ppm as i64,
        window_ids: ids.to_vec(),
        window_fates: fates.to_vec(),
    }
}

/// §3.1 prefix-causal fate tracker. One instance per flow. Deterministic: identical event sequences
/// (ids, times, guards) produce identical state; iteration orders are `BTreeMap`-stable.
#[derive(Debug, Default, Clone)]
pub struct FateEstimator {
    /// Finalized fates (immutable once inserted): id → LOSS?
    finalized: BTreeMap<u64, bool>,
    /// Provisional loss declarations: id → guard expiry (ns). Earliest declaration wins (§3.1: no
    /// re-arming — the guard clock starts at first loss evidence).
    provisional: BTreeMap<u64, u64>,
    /// ACK-of-original arriving AFTER `LOSS` finalization (window immutable; observability only).
    late_ack_corrections: u64,
    /// Provisional loss corrected to `SUCCESS` by an original-ACK inside the guard (§3.1).
    spurious_loss_corrections: u64,
}

impl FateEstimator {
    /// New empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// SUCCESS evidence: receiver ACK proves the ORIGINAL transmission of `id` was delivered with
    /// no recovery action pending. Corrects a provisional loss (spurious++). After `LOSS`
    /// finalization it cannot mutate the window (late_ack_corrections++).
    pub fn on_original_ack(&mut self, id: u64) {
        if let Some(&is_loss) = self.finalized.get(&id) {
            if is_loss {
                self.late_ack_corrections = self.late_ack_corrections.saturating_add(1);
            }
            return; // immutable once fate-known (§3.1)
        }
        if self.provisional.remove(&id).is_some() {
            self.spurious_loss_corrections = self.spurious_loss_corrections.saturating_add(1);
        }
        self.finalized.insert(id, false);
    }

    /// LOSS lineage evidence at `now_ns` (sender loss declaration, ARQ/tail retransmit emission, or
    /// receiver missing/recovered-source evidence). Starts the reordering guard; finalization
    /// happens only via [`Self::advance`] once `now ≥ declaration + guard` (§3.1). Repeated
    /// evidence for the same id keeps the EARLIEST expiry.
    pub fn on_loss_evidence(&mut self, id: u64, now_ns: u64, guard_ns: u64) {
        if self.finalized.contains_key(&id) {
            return;
        }
        let expiry = now_ns.saturating_add(guard_ns);
        self.provisional.entry(id).or_insert(expiry);
    }

    /// Advance virtual time: finalize every provisional loss whose guard has elapsed
    /// (`expiry ≤ now`). Deterministic (id order).
    pub fn advance(&mut self, now_ns: u64) {
        let due: Vec<u64> = self
            .provisional
            .iter()
            .filter(|&(_, &exp)| exp <= now_ns)
            .map(|(&id, _)| id)
            .collect();
        for id in due {
            self.provisional.remove(&id);
            self.finalized.insert(id, true);
        }
    }

    /// Horizon/decision-epoch cutoff (§3.1): whatever is still provisional stays `UNKNOWN` and is
    /// dropped from consideration (never enters the denominator). Call AFTER a final
    /// [`Self::advance`] at the cutoff instant.
    pub fn drop_unresolved_as_unknown(&mut self) {
        self.provisional.clear();
    }

    /// Number of finalized (fate-known) packets.
    #[must_use]
    pub fn fate_known(&self) -> usize {
        self.finalized.len()
    }

    /// Number of still-provisional (guard-pending) packets.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.provisional.len()
    }

    /// §3.1 diagnostics: (late_ack_corrections, spurious_loss_corrections).
    #[must_use]
    pub fn corrections(&self) -> (u64, u64) {
        (self.late_ack_corrections, self.spurious_loss_corrections)
    }

    /// §3.2 snapshot: the last [`FATE_WINDOW`] fate-known entries in logical source order. Owned
    /// copy — later events can never mutate a taken snapshot (no future leakage).
    #[must_use]
    pub fn snapshot(&self) -> WindowFeatures {
        let total = self.finalized.len();
        let skip = total.saturating_sub(FATE_WINDOW);
        let mut ids = Vec::with_capacity(total.min(FATE_WINDOW));
        let mut fates = Vec::with_capacity(total.min(FATE_WINDOW));
        for (i, (&id, &loss)) in self.finalized.iter().enumerate() {
            if i >= skip {
                ids.push(id);
                fates.push(loss);
            }
        }
        features_from_window(&ids, &fates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TailRecoveryConfig {
        // frozen product ON config values (DR-ARQ-PRODUCT-TAIL-1 tail_on_config)
        TailRecoveryConfig {
            enabled: true,
            min_pto_ns: 10_000_000,
            max_pto_ns: 10_000_000_000,
            initial_pto_ns: 1_000_000_000,
            max_backoff_pto_ns: 30_000_000_000,
            max_ack_delay_ns: 25_000_000,
            max_tail_probes: 3,
        }
    }

    // §4.2 synthetic event replay — expected counter vector EXACT equality.
    #[test]
    fn replay_expected_vector_exact() {
        let mut e = FateEstimator::new();
        let g = 100;
        // ids 0..10: fates S S L L S L S S S S (losses at 2,3,5)
        for id in [0u64, 1] {
            e.on_original_ack(id);
        }
        e.on_loss_evidence(2, 1_000, g);
        e.on_loss_evidence(3, 1_010, g);
        e.on_original_ack(4);
        e.on_loss_evidence(5, 1_020, g);
        for id in [6u64, 7, 8, 9] {
            e.on_original_ack(id);
        }
        e.advance(2_000); // all guards elapsed
        let f = e.snapshot();
        assert_eq!((f.n, f.lost), (10, 3));
        // transitions over S S L L S L S S S S:
        // 0-1 SS, 1-2 SL, 2-3 LL, 3-4 LS, 4-5 SL, 5-6 LS, 6-7 SS, 7-8 SS, 8-9 SS
        assert_eq!((f.n00, f.n01, f.n10, f.n11), (4, 2, 2, 1));
        assert_eq!(f.measured_loss_ppm, 300_000);
        // p11 = (1+1)/(2+1+2) = 2/5 = 400000 ; p01 = (2+1)/(4+2+2) = 3/8 = 375000
        assert_eq!(f.p11_ppm, 400_000);
        assert_eq!(f.p01_ppm, 375_000);
        assert_eq!(f.burst_delta_ppm, 25_000);
    }

    // §3.1 guard boundary: no LOSS finalization before the guard elapses; exact boundary inclusive.
    #[test]
    fn guard_boundary_below_at_above() {
        let mut e = FateEstimator::new();
        e.on_loss_evidence(7, 1_000, 500); // expiry 1500
        e.advance(1_499);
        assert_eq!(e.fate_known(), 0, "guard not elapsed → not finalized");
        assert_eq!(e.pending(), 1);
        e.advance(1_500);
        assert_eq!(e.fate_known(), 1, "expiry ≤ now → finalized LOSS");
        assert_eq!(e.snapshot().lost, 1);
    }

    // §3.1 late original-ACK INSIDE the guard corrects to SUCCESS + spurious counter.
    #[test]
    fn late_ack_within_guard_corrects_spurious() {
        let mut e = FateEstimator::new();
        e.on_loss_evidence(3, 1_000, 500);
        e.on_original_ack(3); // within guard
        e.advance(10_000);
        let f = e.snapshot();
        assert_eq!((f.n, f.lost), (1, 0), "corrected to SUCCESS");
        assert_eq!(e.corrections(), (0, 1));
    }

    // §3.1 ACK after finalization: window immutable; late_ack_corrections increments.
    #[test]
    fn ack_after_finalization_immutable() {
        let mut e = FateEstimator::new();
        e.on_loss_evidence(3, 1_000, 500);
        e.advance(1_500); // finalized LOSS
        e.on_original_ack(3);
        let f = e.snapshot();
        assert_eq!((f.n, f.lost), (1, 1), "LOSS stays");
        assert_eq!(e.corrections(), (1, 0));
    }

    // §3.1 same logical id cannot enter the window twice.
    #[test]
    fn dedup_single_entry_per_id() {
        let mut e = FateEstimator::new();
        e.on_original_ack(5);
        e.on_original_ack(5);
        e.on_loss_evidence(5, 1_000, 100);
        e.advance(5_000);
        let f = e.snapshot();
        assert_eq!(
            (f.n, f.lost),
            (1, 0),
            "first finalization wins; id counted once"
        );
    }

    // §3.2 UNKNOWN breaks transition chains (gap ids are not adjacent).
    #[test]
    fn unknown_excluded_breaks_chain() {
        let mut e = FateEstimator::new();
        e.on_original_ack(1);
        e.on_original_ack(2);
        // id 3 never resolves (UNKNOWN)
        e.on_original_ack(4);
        e.on_original_ack(5);
        let f = e.snapshot();
        assert_eq!(f.n, 4);
        // transitions: only 1-2 and 4-5 (id 3 missing breaks 2-3, 3-4)
        assert_eq!(f.n00, 2);
    }

    // §2.3/§3.2 window slides: only the last 256 fate-known enter.
    #[test]
    fn window_slides_to_last_256() {
        let mut e = FateEstimator::new();
        for id in 0..300u64 {
            if id % 10 == 0 {
                e.on_loss_evidence(id, 1_000 + id, 10);
            } else {
                e.on_original_ack(id);
            }
        }
        e.advance(1_000_000);
        let f = e.snapshot();
        assert_eq!(f.n as usize, FATE_WINDOW);
        assert_eq!(*f.window_ids.first().unwrap(), 44, "ids 0..43 slid out");
        assert_eq!(*f.window_ids.last().unwrap(), 299);
        // losses at multiples of 10 within 44..=299: 50,60,...,290 → 25
        assert_eq!(f.lost, 25);
    }

    // No future leakage: a taken snapshot is unaffected by later events.
    #[test]
    fn snapshot_no_future_leakage() {
        let mut e = FateEstimator::new();
        for id in 0..10u64 {
            e.on_original_ack(id);
        }
        let snap = e.snapshot();
        e.on_loss_evidence(10, 1, 1);
        e.advance(100);
        assert_eq!(snap.n, 10, "old snapshot unchanged");
        assert_eq!(e.snapshot().n, 11);
    }

    // Horizon cutoff: pending provisional stays UNKNOWN and never enters the denominator.
    #[test]
    fn horizon_unresolved_stays_unknown() {
        let mut e = FateEstimator::new();
        e.on_original_ack(0);
        e.on_loss_evidence(1, 1_000, 1_000_000); // guard far in the future
        e.advance(2_000); // guard NOT elapsed
        e.drop_unresolved_as_unknown();
        let f = e.snapshot();
        assert_eq!(f.n, 1, "pending → UNKNOWN, excluded");
        assert_eq!(e.pending(), 0);
    }

    // §3.3 Laplace smoothing: zero-transition denominators never divide by zero; the no-loss
    // window yields a large positive burst_delta — which is exactly why §3.4 eligibility
    // (n10+n11 ≥ 8) must gate activation.
    #[test]
    fn laplace_no_loss_window_ineligible() {
        let bits = vec![false; FATE_WINDOW];
        let f = features_from_fate_bits(0, &bits);
        assert_eq!(f.p11_ppm, 500_000); // (0+1)/(0+2)
        assert_eq!(f.p01_ppm, 1_000_000 / 257); // (0+1)/(255+2)
        assert!(!f.eligible(true, false, true), "n10+n11=0 < 8 → ineligible");
    }

    // §3.4 eligibility boundary: exactly at the pinned thresholds.
    #[test]
    fn eligibility_exact_thresholds() {
        // construct a 256 window with exactly 8 loss transitions and rest non-loss:
        // 4 isolated losses => each contributes n01+n10 = 2 → n10+n11 = 4... need >= 8:
        // use 4 loss-pairs (LL): each pair contributes n01(1into)+n11(1)+n10(1 out) → n10+n11 = 8 total.
        let mut bits = vec![false; FATE_WINDOW];
        for k in 0..4 {
            let base = 20 + k * 30;
            bits[base] = true;
            bits[base + 1] = true;
        }
        let f = features_from_fate_bits(0, &bits);
        assert_eq!(f.n10 + f.n11, 8);
        assert!(f.n00 + f.n01 >= MIN_NONLOSS_TRANSITIONS);
        assert!(f.eligible(true, false, true));
        assert!(
            !f.eligible(false, false, true),
            "warm-up incomplete → ineligible"
        );
        assert!(
            !f.eligible(true, true, true),
            "stale telemetry → ineligible"
        );
        assert!(!f.eligible(true, false, false), "unconverged → ineligible");
    }

    // features_from_fate_bits ≡ event-replay equivalence.
    #[test]
    fn bits_vs_replay_equivalence() {
        let bits: Vec<bool> = (0..300).map(|i| i % 7 == 3 || i % 11 == 5).collect();
        let via_bits = features_from_fate_bits(0, &bits);
        let mut e = FateEstimator::new();
        for (id, &l) in bits.iter().enumerate() {
            if l {
                e.on_loss_evidence(id as u64, 1, 1);
            } else {
                e.on_original_ack(id as u64);
            }
        }
        e.advance(u64::MAX);
        let via_replay = e.snapshot();
        assert_eq!(via_bits, via_replay);
    }

    // Reordering vector: out-of-order evidence still yields id-ordered window (§3.2).
    #[test]
    fn reordered_evidence_id_ordered() {
        let mut e = FateEstimator::new();
        e.on_original_ack(5);
        e.on_original_ack(3);
        e.on_loss_evidence(4, 10, 10);
        e.advance(100);
        let f = e.snapshot();
        assert_eq!(f.window_ids, vec![3, 4, 5]);
        assert_eq!(f.window_fates, vec![false, true, false]);
        assert_eq!((f.n01, f.n10), (1, 1));
    }

    // Determinism: identical input → identical output, three runs.
    #[test]
    fn determinism_thrice() {
        let run = || {
            let mut e = FateEstimator::new();
            for id in 0..400u64 {
                if id % 5 == 0 {
                    e.on_loss_evidence(id, id * 10, 50);
                } else {
                    e.on_original_ack(id);
                }
                e.advance(id * 10);
            }
            e.advance(u64::MAX);
            e.snapshot()
        };
        let a = run();
        let b = run();
        let c = run();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    // Overflow safety at extremes.
    #[test]
    fn overflow_safe() {
        let mut e = FateEstimator::new();
        e.on_loss_evidence(u64::MAX, u64::MAX, u64::MAX);
        e.advance(u64::MAX);
        let _ = e.snapshot();
        let g = reordering_guard_ns(&cfg(), Some(u64::MAX), u64::MAX);
        assert!(g <= cfg().max_pto_ns, "guard clamped by product max_pto");
    }

    // §3.1 guard grounding: identical to the product TailRecovery::base_pto_ns symbol.
    #[test]
    fn guard_equals_product_base_pto() {
        let c = cfg();
        let t = TailRecovery::new(c);
        for (srtt, var) in [(None, 0), (Some(100_000_000), 5_000_000), (Some(1), 0)] {
            assert_eq!(reordering_guard_ns(&c, srtt, var), t.base_pto_ns(srtt, var));
        }
        // no sample → product initial_pto branch
        assert_eq!(reordering_guard_ns(&c, None, 0), c.initial_pto_ns);
    }
}
