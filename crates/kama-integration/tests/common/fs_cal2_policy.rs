//! FS-CAL-2 v3 §7.3 — FEC-activation subpolicy inference (exact integer; canonical threshold form).
//!
//! Normative: FS-CAL-2-EXPERIMENT-DESIGN-v3 (sha 115fa266…3393) §3.4b (support bitmap + runtime
//! interlocks) + §7.3 (membership form, SCALE=1e6, checked integer arithmetic, boundary branches).
//! Feature allowlist (compile-enforced by this module's API surface — the ONLY inputs are):
//! `measured_loss_ppm, burst_delta_ppm, estimator_eligible, support_eligible, workload_supported`.
//! Configured ground-truth (structure/RTT/impairment labels) CANNOT reach this API.

use std::collections::BTreeSet;

/// §7.3 SCALE.
pub const SCALE: u64 = 1_000_000;
/// §3.4b support bin widths (PINNED).
pub const LOSS_BIN_WIDTH_PPM: u64 = 50_000;
/// §3.4b burst bin width (PINNED).
pub const BURST_BIN_WIDTH_PPM: i64 = 100_000;
/// §3.4b burst clamp low (PINNED).
pub const BURST_CLAMP_LO: i64 = -200_000;
/// §3.4b burst clamp high (PINNED).
pub const BURST_CLAMP_HI: i64 = 900_000;

/// Typed fallback reasons (§3.4b + §10 fallback_reason enum). Every non-R2 path carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyFallback {
    /// R2 requested (no fallback).
    None,
    /// §3.4 estimator eligibility failed.
    EstimatorIneligible,
    /// §3.4b observed feature bin outside the frozen support bitmap.
    OutOfSupport,
    /// §2.2 workload profile mismatch (mechanical interlock BEFORE policy inference).
    UnsupportedWorkload,
    /// Activation score below threshold (legitimate R0 region).
    BelowActivation,
    /// Static arm — policy not consulted.
    StaticArm,
}

impl PolicyFallback {
    /// Canonical §10 string code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::EstimatorIneligible => "ESTIMATOR_INELIGIBLE",
            Self::OutOfSupport => "OUT_OF_SUPPORT",
            Self::UnsupportedWorkload => "UNSUPPORTED_WORKLOAD",
            Self::BelowActivation => "BELOW_ACTIVATION",
            Self::StaticArm => "STATIC_ARM",
        }
    }
}

/// §3.4b support bins for an observed snapshot. `burst_bin` after clamp is in `0..=11`.
#[must_use]
pub fn support_bins(measured_loss_ppm: u64, burst_delta_ppm: i64) -> (u32, u32) {
    let loss_bin = (measured_loss_ppm / LOSS_BIN_WIDTH_PPM) as u32;
    let clamped = burst_delta_ppm.clamp(BURST_CLAMP_LO, BURST_CLAMP_HI);
    let burst_bin = ((clamped - BURST_CLAMP_LO) / BURST_BIN_WIDTH_PPM) as u32;
    (loss_bin, burst_bin)
}

/// §7.3 `mu_high_loss(x;a,b)` — exact integer, boundary branches pinned by the design.
#[must_use]
pub fn mu_high_loss(x_ppm: u64, a_ppm: u64, b_ppm: u64) -> u64 {
    debug_assert!(a_ppm < b_ppm, "loss_a < loss_b");
    if x_ppm <= a_ppm {
        0
    } else if x_ppm >= b_ppm {
        SCALE
    } else {
        (x_ppm - a_ppm)
            .checked_mul(SCALE)
            .expect("mu_high_loss overflow")
            / (b_ppm - a_ppm)
    }
}

/// §7.3 `mu_nonburst(y;a,b)` — exact integer over SIGNED ppm burst-delta.
#[must_use]
pub fn mu_nonburst(y_ppm: i64, a_ppm: i64, b_ppm: i64) -> u64 {
    debug_assert!(a_ppm < b_ppm, "burst_a < burst_b");
    if y_ppm <= a_ppm {
        SCALE
    } else if y_ppm >= b_ppm {
        0
    } else {
        let num = (b_ppm - y_ppm) as u128 * u128::from(SCALE);
        let den = (b_ppm - a_ppm) as u128;
        (num / den) as u64
    }
}

/// Versioned FEC-activation subpolicy artifact (§7.3 + §11.6). The support bitmap is embedded
/// (frozen with the training dataset hash before any holdout).
#[derive(Debug, Clone)]
pub struct FecActivationPolicyV1 {
    /// Calibration artifact version tag.
    pub calibration_version: String,
    /// §7.3 loss membership breakpoints (ppm; `loss_a < loss_b`).
    pub loss_a_ppm: u64,
    /// See `loss_a_ppm`.
    pub loss_b_ppm: u64,
    /// §7.3 burst membership breakpoints (signed ppm; `burst_a < burst_b`).
    pub burst_a_ppm: i64,
    /// See `burst_a_ppm`.
    pub burst_b_ppm: i64,
    /// Activation threshold scaled by [`SCALE`] (e.g. 0.70 → 700_000).
    pub activation_threshold_scaled: u64,
    /// §3.4b frozen support bitmap: allowed (loss_bin, burst_bin) pairs.
    pub support: BTreeSet<(u32, u32)>,
    /// sha256 of the canonical support bitmap serialization (traceability).
    pub support_bitmap_sha256: String,
    /// Frozen workload profile this policy is valid for (§2.2 interlock).
    pub workload_profile_id: String,
    /// Frozen ARQ contract this policy is valid under (§4.0b interlock).
    pub arq_contract_id: String,
}

/// Policy decision output (§7.3 inference + §3.4b interlocks, in the pinned order:
/// workload → estimator eligibility → support bitmap → activation threshold).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyDecision {
    /// True ⇒ R2 requested; false ⇒ R0.
    pub request_r2: bool,
    /// `min(mu_high_loss, mu_nonburst)` (0..=SCALE) — recorded even when interlocked (0 there).
    pub activation_score: u64,
    /// Reason when `request_r2 == false` (or `None` when R2).
    pub fallback: PolicyFallback,
}

impl FecActivationPolicyV1 {
    /// §7.3 inference. ONLY runtime-observable inputs (allowlist) — no configured labels.
    #[must_use]
    pub fn decide(
        &self,
        measured_loss_ppm: u64,
        burst_delta_ppm: i64,
        estimator_eligible: bool,
        workload_profile_id: &str,
        arq_contract_id: &str,
    ) -> PolicyDecision {
        // §2.2 mechanical interlock BEFORE inference.
        if workload_profile_id != self.workload_profile_id
            || arq_contract_id != self.arq_contract_id
        {
            return PolicyDecision {
                request_r2: false,
                activation_score: 0,
                fallback: PolicyFallback::UnsupportedWorkload,
            };
        }
        if !estimator_eligible {
            return PolicyDecision {
                request_r2: false,
                activation_score: 0,
                fallback: PolicyFallback::EstimatorIneligible,
            };
        }
        let bins = support_bins(measured_loss_ppm, burst_delta_ppm);
        if !self.support.contains(&bins) {
            return PolicyDecision {
                request_r2: false,
                activation_score: 0,
                fallback: PolicyFallback::OutOfSupport,
            };
        }
        let mu_l = mu_high_loss(measured_loss_ppm, self.loss_a_ppm, self.loss_b_ppm);
        let mu_b = mu_nonburst(burst_delta_ppm, self.burst_a_ppm, self.burst_b_ppm);
        let activation = mu_l.min(mu_b);
        if activation >= self.activation_threshold_scaled {
            PolicyDecision {
                request_r2: true,
                activation_score: activation,
                fallback: PolicyFallback::None,
            }
        } else {
            PolicyDecision {
                request_r2: false,
                activation_score: activation,
                fallback: PolicyFallback::BelowActivation,
            }
        }
    }
}

/// FIXTURE policy — DATA-PROCESSABLE/actuator-conformance testing ONLY. Clearly NOT calibrated;
/// never used in any result-bearing run (YETKİ-2 fits the real artifact from training data).
#[must_use]
pub fn fixture_policy(workload_profile_id: &str, arq_contract_id: &str) -> FecActivationPolicyV1 {
    let mut support = BTreeSet::new();
    // generous fixture support: loss bins 0..=6 (0..350k ppm) × burst bins 0..=6 (−200k..500k)
    for lb in 0..=6u32 {
        for bb in 0..=6u32 {
            support.insert((lb, bb));
        }
    }
    FecActivationPolicyV1 {
        calibration_version: "FIXTURE-NOT-CALIBRATED-v0".into(),
        loss_a_ppm: 100_000,
        loss_b_ppm: 200_000,
        burst_a_ppm: 150_000,
        burst_b_ppm: 350_000,
        activation_threshold_scaled: 700_000, // 0.70
        support,
        support_bitmap_sha256: "FIXTURE".into(),
        workload_profile_id: workload_profile_id.into(),
        arq_contract_id: arq_contract_id.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> FecActivationPolicyV1 {
        fixture_policy("WPID", "ACID")
    }

    // §7.3 exact-threshold + ±1-unit goldens (mandatory per design).
    #[test]
    fn mu_boundaries_exact_and_pm1() {
        // mu_high_loss over a=100k, b=200k
        assert_eq!(mu_high_loss(100_000, 100_000, 200_000), 0, "x == a → 0");
        assert_eq!(
            mu_high_loss(100_001, 100_000, 200_000),
            10,
            "a+1 → floor(1e6/100000)=10"
        );
        assert_eq!(mu_high_loss(150_000, 100_000, 200_000), 500_000, "midpoint");
        assert_eq!(mu_high_loss(199_999, 100_000, 200_000), 999_990, "b−1");
        assert_eq!(
            mu_high_loss(200_000, 100_000, 200_000),
            SCALE,
            "x == b → SCALE"
        );
        // mu_nonburst over a=150k, b=350k (signed domain)
        assert_eq!(
            mu_nonburst(150_000, 150_000, 350_000),
            SCALE,
            "y == a → SCALE"
        );
        assert_eq!(mu_nonburst(150_001, 150_000, 350_000), 999_995, "a+1");
        assert_eq!(mu_nonburst(350_000, 150_000, 350_000), 0, "y == b → 0");
        assert_eq!(mu_nonburst(349_999, 150_000, 350_000), 5, "b−1");
        assert_eq!(
            mu_nonburst(-200_000, 150_000, 350_000),
            SCALE,
            "negative burst → SCALE"
        );
    }

    #[test]
    fn activation_exact_threshold_boundary() {
        let pol = p();
        // choose loss so mu_l exactly equals threshold 700_000: x = a + 0.7*(b-a) = 170_000
        let d = pol.decide(170_000, -100_000, true, "WPID", "ACID");
        assert_eq!(d.activation_score, 700_000);
        assert!(d.request_r2, "activation == threshold → R2 (>= semantics)");
        // one ppm below the exact boundary
        let d2 = pol.decide(169_999, -100_000, true, "WPID", "ACID");
        assert_eq!(d2.activation_score, 699_990);
        assert!(!d2.request_r2);
        assert_eq!(d2.fallback, PolicyFallback::BelowActivation);
    }

    #[test]
    fn interlock_order_pinned() {
        let pol = p();
        // workload interlock fires FIRST even when everything else would pass
        let d = pol.decide(200_000, 0, true, "OTHER", "ACID");
        assert_eq!(d.fallback, PolicyFallback::UnsupportedWorkload);
        let d = pol.decide(200_000, 0, true, "WPID", "OTHER");
        assert_eq!(d.fallback, PolicyFallback::UnsupportedWorkload);
        // then estimator eligibility
        let d = pol.decide(200_000, 0, false, "WPID", "ACID");
        assert_eq!(d.fallback, PolicyFallback::EstimatorIneligible);
        // then support bitmap (loss bin 7 = 350k+ outside fixture support)
        let d = pol.decide(360_000, 0, true, "WPID", "ACID");
        assert_eq!(d.fallback, PolicyFallback::OutOfSupport);
    }

    #[test]
    fn support_bins_exact() {
        assert_eq!(support_bins(0, -200_000), (0, 0));
        assert_eq!(support_bins(49_999, -200_001), (0, 0), "clamp below");
        assert_eq!(support_bins(50_000, -100_001), (1, 0), "bin edges");
        assert_eq!(support_bins(50_000, -100_000), (1, 1));
        assert_eq!(support_bins(200_000, 900_000), (4, 11), "clamp top bin");
        assert_eq!(support_bins(200_000, 950_000), (4, 11), "clamp above");
    }

    #[test]
    fn overflow_and_extremes_safe() {
        let pol = p();
        let d = pol.decide(u64::MAX / SCALE, i64::MAX, true, "WPID", "ACID");
        assert!(!d.request_r2); // out of support at extreme bins
        let _ = mu_nonburst(i64::MIN + 1, -200_000, 900_000);
        let _ = mu_high_loss(1_000_000, 0, 1);
    }

    #[test]
    fn determinism() {
        let pol = p();
        let a = pol.decide(180_000, 100_000, true, "WPID", "ACID");
        for _ in 0..32 {
            assert_eq!(pol.decide(180_000, 100_000, true, "WPID", "ACID"), a);
        }
    }
}
