//! FS-FUSION-CAL-1 — versioned calibration + pure fusion policy + independent safety validator.
//!
//! Clean-architecture separation (the sender never holds membership breakpoints, thresholds, rule
//! weights, defuzzification, or calibration constants):
//!
//! ```text
//! runtime telemetry -> FusionObservation (prefix-causal only)
//!                   -> FusionPolicy (pure, deterministic)  -> FusionAdvice
//!                   -> FusionSafetyValidator (independent)  -> FusionAction (bounded)
//!                   -> existing ARQ / FEC mechanisms
//! ```
//!
//! **DEFAULT OFF.** `FusionPolicy::disabled()` yields `FusionAction::ArqOnly` for every input, so the
//! product path is byte-identical to pre-calibration behaviour. A calibration is adopted only by an
//! explicit, versioned opt-in; any version mismatch, unsupported region, low confidence, stale or
//! invalid telemetry, or cold-start without observability degrades to `FallbackArq`.
//!
//! The policy is ADVISORY. It never mutates ARQ/ACK/loss/RTT/PTO state, never bypasses pacing or
//! congestion control, and can never raise a hard cap: caps live in [`FusionSafetyConfig`], which is
//! independent of the calibration artifact (a calibration can only request less, never more).
//!
//! Evidence: reports/fs-fusion-cal-1/{calibration-v1.json, decision-regions.csv}. The calibrated loss
//! membership corrects a concrete defect — the uncalibrated fixture classifies 200_000 ppm (20 % loss)
//! as fully LOW, firing R1 (fec_zero) in exactly the region where the authoritative data shows FEC
//! Pareto-dominates.

use crate::fusion::FusionInputs;

/// Calibration artifact identity. A policy only accepts telemetry it was calibrated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationVersion {
    /// Calibration artifact id, e.g. `"FS-FUSION-CAL-1"`.
    pub id: &'static str,
    /// Monotonic version of that artifact.
    pub version: u32,
    /// Feature-schema version the calibration was fitted against. Mismatch ⇒ `FallbackArq`.
    pub feature_schema: u32,
}

/// The feature schema this build produces observations for.
pub const FEATURE_SCHEMA_V1: u32 = 1;

/// Reason a bounded fallback was taken (observation-only; every variant ⇒ `ArqOnly` behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// Policy not enabled (default).
    PolicyDisabled,
    /// Calibration/feature-schema version mismatch.
    VersionMismatch,
    /// Telemetry older than the staleness bound.
    StaleTelemetry,
    /// Flow has not converged (no RTT samples) — jitter/PTO inputs untrustworthy.
    ColdStart,
    /// Observation outside the calibrated supported region.
    UnsupportedRegion,
    /// Observation inside the region but with insufficient confidence.
    LowConfidence,
    /// Malformed/out-of-domain input.
    InvalidInput,
    /// Requested action failed an independent safety check.
    SafetyRejected,
    /// FEC actuation is not available in this build/product path.
    FecUnavailable,
}

/// Prefix-causal runtime observation. Every field MUST be computable by the sender at decision time
/// from past observations only — no outcome, no ground-truth impairment label, no future event.
#[derive(Debug, Clone, Copy)]
pub struct FusionObservation {
    /// Feature schema this observation was built with.
    pub feature_schema: u32,
    /// The engine's crisp inputs (already prefix-causal by construction).
    pub inputs: FusionInputs,
    /// Number of RTT samples taken so far (0 ⇒ cold start; 1 ⇒ jitter bootstrap artifact).
    pub rtt_sample_count: u32,
    /// Nanoseconds since the newest telemetry update feeding `inputs`.
    pub telemetry_age_ns: u64,
    /// True when the sender has converged out of the cold-start phase.
    pub converged: bool,
    /// True when a product FEC send path is actually available to actuate.
    pub fec_actuation_available: bool,
}

/// What the policy would like to do (advisory; not yet safety-checked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionAdvice {
    /// Rely on ARQ alone.
    ArqOnly,
    /// Add bounded proactive redundancy.
    FecAssist {
        /// Parity ratio (per-mille) requested by the calibrated engine.
        redundancy_permille: u32,
    },
}

/// The bounded action the actuator may apply. Produced ONLY by the safety validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionAction {
    /// ARQ only — the safe baseline, byte-identical to pre-calibration behaviour.
    ArqOnly,
    /// Bounded additive FEC assist. Never defers, cancels or weakens ARQ.
    FecAssist {
        /// Clamped parity ratio (per-mille).
        redundancy_permille: u32,
        /// Hard byte ceiling for this decision.
        byte_budget: u64,
    },
    /// Safe fallback with an attributed reason.
    FallbackArq(FallbackReason),
}

/// [TEŞHİS] `FusionAction` histogram kovaları. "Politika neden ateşlemedi" sorusu, ret
/// sebebi sayılmadan cevaplanamaz — bu repoda aynı hata yapı-gate'te bir kez yaşandı
/// (tek kovaya yığılan ret, vakum sayaç).
pub const FUSION_ACTION_KINDS: usize = 10;

impl FusionAction {
    /// Histogram indeksi: 0=ArqOnly · 1=FecAssist · 2..=9 fallback sebepleri.
    #[must_use]
    pub fn index(&self) -> usize {
        match self {
            Self::ArqOnly => 0,
            Self::FecAssist { .. } => 1,
            Self::FallbackArq(r) => 2 + Self::reason_index(*r),
        }
    }

    fn reason_index(r: FallbackReason) -> usize {
        match r {
            FallbackReason::PolicyDisabled => 0,
            FallbackReason::VersionMismatch => 1,
            FallbackReason::UnsupportedRegion => 2,
            FallbackReason::LowConfidence => 3,
            FallbackReason::StaleTelemetry => 4,
            FallbackReason::ColdStart => 5,
            FallbackReason::InvalidInput => 6,
            FallbackReason::SafetyRejected | FallbackReason::FecUnavailable => 7,
        }
    }
}

impl FusionAction {
    /// True when the action leaves the wire byte-identical to ARQ-only behaviour.
    #[must_use]
    pub fn is_arq_only(&self) -> bool {
        matches!(self, Self::ArqOnly | Self::FallbackArq(_))
    }
}

/// Independent hard bounds. NOT part of the calibration artifact — a calibration can request less,
/// never more. Changing these is a deliberate safety-configuration change.
#[derive(Debug, Clone, Copy)]
pub struct FusionSafetyConfig {
    /// Absolute ceiling on parity ratio (per-mille) regardless of calibration.
    pub hard_max_redundancy_permille: u32,
    /// Absolute ceiling on FEC bytes for a single decision.
    pub hard_fec_byte_budget: u64,
    /// Absolute ceiling on combined added overhead for a single decision.
    pub hard_combined_byte_cap: u64,
    /// Telemetry older than this is stale.
    pub max_telemetry_age_ns: u64,
    /// RTT samples required before jitter-derived inputs are trusted (bootstrap artifact guard).
    pub min_rtt_samples: u32,
}

impl Default for FusionSafetyConfig {
    fn default() -> Self {
        Self {
            hard_max_redundancy_permille: 800,
            hard_fec_byte_budget: 4096,
            hard_combined_byte_cap: 8192,
            max_telemetry_age_ns: 2_000_000_000, // 2 s
            // rtt.rs bootstraps rttvar = srtt/2 on the FIRST sample => jitter_ppm = 500_000, which the
            // fixture places in the jitter HIGH shoulder. Require a second sample before trusting it.
            min_rtt_samples: 2,
        }
    }
}

/// Calibrated supported region (from the calibration artifact). Outside ⇒ `UnsupportedRegion`.
#[derive(Debug, Clone, Copy)]
pub struct SupportedRegion {
    /// Inclusive upper bound of calibrated loss evidence (ppm).
    pub loss_ppm_max: u64,
    /// Inclusive bandwidth bounds (bytes/sec) of the tested regime.
    pub bandwidth_bps_min: u64,
    /// Inclusive bandwidth bounds (bytes/sec) of the tested regime.
    pub bandwidth_bps_max: u64,
}

/// Calibrated loss membership boundary (per calibration-v1). Integer, deterministic, no RNG.
#[derive(Debug, Clone, Copy)]
pub struct LossBoundary {
    /// Fully LOW at or below this (ppm).
    pub low_full_ppm: u64,
    /// LOW reaches zero here (ppm).
    pub low_zero_ppm: u64,
    /// HIGH starts rising here (ppm).
    pub high_start_ppm: u64,
    /// Fully HIGH at or above this (ppm).
    pub high_full_ppm: u64,
}

/// The versioned, pure fusion policy.
#[derive(Debug, Clone, Copy)]
pub struct FusionPolicy {
    enabled: bool,
    version: CalibrationVersion,
    loss: LossBoundary,
    region: SupportedRegion,
    /// Parity ratio requested when loss is fully HIGH (calibrated consequent `fec_high`).
    fec_high_permille: u32,
    safety: FusionSafetyConfig,
}

impl FusionPolicy {
    /// DEFAULT: disabled. Always yields `FallbackArq(PolicyDisabled)` ⇒ byte-identical product path.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            version: CalibrationVersion {
                id: "NONE",
                version: 0,
                feature_schema: FEATURE_SCHEMA_V1,
            },
            loss: LossBoundary {
                low_full_ppm: 0,
                low_zero_ppm: 0,
                high_start_ppm: u64::MAX,
                high_full_ppm: u64::MAX,
            },
            region: SupportedRegion {
                loss_ppm_max: 0,
                bandwidth_bps_min: 1,
                bandwidth_bps_max: 0,
            },
            fec_high_permille: 0,
            safety: FusionSafetyConfig::default(),
        }
    }

    /// Adopt `calibration-v1` (reports/fs-fusion-cal-1/calibration-v1.json). Opt-in and versioned.
    #[must_use]
    pub fn calibration_v1() -> Self {
        Self {
            enabled: true,
            version: CalibrationVersion {
                id: "FS-FUSION-CAL-1",
                version: 1,
                feature_schema: FEATURE_SCHEMA_V1,
            },
            // **[DAYANAK BORCU — 2026-09-04] BU BANT KENARLARI TÜRETİLEMEZ.**
            // Eski gerekçe: "ARQ_TAIL Pareto-dominant at 20_833..83_333 ppm; FEC_R2
            // Pareto-dominant at 200_000 ppm". İki sebeple çöktü:
            //  1. O kıyas (ARQ_TAIL r0) vs (FEC_R2, DONMUŞ tail-kör ARQ) — İKİ ORTOGONAL EKSENİ
            //     aynı anda değiştirir ve `FRAMING-CORRIGENDUM.md:24-28` uyarınca "not a valid
            //     contrast under any framing"; §2 decision-regions SUPERSEDED işaretli.
            //  2. Kaynak grid (R2A) loss ORANINI loss YAPISIYLA karıştırır: S1/S2 pozisyonel
            //     silme (48 symbol'ün %2-8'i), S3 IID %20. Nominal ppm'ler jeneratör
            //     artefaktıdır (20_833 = 1/48, 41_666 = 2/48, 83_333 = 4/48). Raporun kendi
            //     hükmü: "a production FEC-activation threshold cannot be derived from
            //     loss_ppm alone from R2A."
            // Sonuç: aşağıdaki dört kenar (90k/140k/160k/200k) ÖLÇÜLMÜŞ bir geçiş noktasını
            // temsil ETMEZ. Politika opt-in ve default `disabled()` olduğu için sevk edilen
            // davranış etkilenmez; bu kalibrasyon ancak ürün-katmanında FEC kollu, oran×yapı
            // çaprazlanmış bir grid'den YENİDEN türetildikten sonra açılabilir.
            loss: LossBoundary {
                low_full_ppm: 90_000,
                low_zero_ppm: 140_000,
                high_start_ppm: 160_000,
                high_full_ppm: 200_000,
            },
            region: SupportedRegion {
                loss_ppm_max: 200_000,
                bandwidth_bps_min: 100_000,
                bandwidth_bps_max: 125_000,
            },
            fec_high_permille: 800,
            safety: FusionSafetyConfig::default(),
        }
    }

    /// Override the independent safety bounds (safety configuration, not calibration).
    #[must_use]
    pub fn with_safety(mut self, safety: FusionSafetyConfig) -> Self {
        self.safety = safety;
        self
    }

    /// Calibration identity (for version-mismatch checks and telemetry).
    #[must_use]
    pub fn version(&self) -> CalibrationVersion {
        self.version
    }

    /// True when a calibration is adopted.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Calibrated HIGH-loss membership degree (per-mille). Deterministic integer ramp.
    #[must_use]
    pub fn loss_high_permille(&self, loss_ppm: u64) -> u32 {
        let (a, b) = (self.loss.high_start_ppm, self.loss.high_full_ppm);
        if loss_ppm >= b {
            return 1000;
        }
        if loss_ppm <= a || b <= a {
            return 0;
        }
        // integer-exact, overflow-safe ramp
        let num = (loss_ppm - a) as u128 * 1000u128;
        let den = (b - a) as u128;
        (num / den) as u32
    }

    /// PURE inference: advisory only, no safety clamping, no side effects.
    #[must_use]
    pub fn advise(&self, obs: &FusionObservation) -> FusionAdvice {
        if !self.enabled {
            return FusionAdvice::ArqOnly;
        }
        let hi = self.loss_high_permille(obs.inputs.loss_ppm);
        if hi == 0 {
            // Calibrated LOW/MED region: the authoritative data shows ARQ (tail-capable) Pareto-dominant.
            return FusionAdvice::ArqOnly;
        }
        // Scale the calibrated consequent by the HIGH membership degree (weighted-average consistent).
        let r = (u64::from(self.fec_high_permille) * u64::from(hi) / 1000) as u32;
        if r == 0 {
            FusionAdvice::ArqOnly
        } else {
            FusionAdvice::FecAssist {
                redundancy_permille: r,
            }
        }
    }

    /// Full decision: pure inference followed by the INDEPENDENT safety validator.
    /// Every failure mode degrades to `FallbackArq`, never to a more aggressive action.
    #[must_use]
    pub fn decide(&self, obs: &FusionObservation, payload_bytes: u64) -> FusionAction {
        FusionSafetyValidator::new(self.safety, self.region, self.version).validate(
            self,
            obs,
            payload_bytes,
        )
    }
}

/// Independent safety validator — the final authority. The policy is only advisory.
#[derive(Debug, Clone, Copy)]
pub struct FusionSafetyValidator {
    cfg: FusionSafetyConfig,
    region: SupportedRegion,
    expected: CalibrationVersion,
}

impl FusionSafetyValidator {
    /// New validator bound to a safety config, supported region and expected calibration identity.
    #[must_use]
    pub fn new(
        cfg: FusionSafetyConfig,
        region: SupportedRegion,
        expected: CalibrationVersion,
    ) -> Self {
        Self {
            cfg,
            region,
            expected,
        }
    }

    /// Validate an observation and the policy's advice into a bounded action.
    #[must_use]
    pub fn validate(
        &self,
        policy: &FusionPolicy,
        obs: &FusionObservation,
        payload_bytes: u64,
    ) -> FusionAction {
        use FallbackReason as R;
        if !policy.is_enabled() {
            return FusionAction::FallbackArq(R::PolicyDisabled);
        }
        if obs.feature_schema != self.expected.feature_schema {
            return FusionAction::FallbackArq(R::VersionMismatch);
        }
        // domain validity (integer inputs: reject out-of-domain rather than saturating silently)
        if obs.inputs.loss_ppm > 1_000_000 || obs.inputs.ack_confidence_permille > 1000 {
            return FusionAction::FallbackArq(R::InvalidInput);
        }
        if obs.telemetry_age_ns > self.cfg.max_telemetry_age_ns {
            return FusionAction::FallbackArq(R::StaleTelemetry);
        }
        if !obs.converged {
            return FusionAction::FallbackArq(R::ColdStart);
        }
        if obs.rtt_sample_count < self.cfg.min_rtt_samples {
            return FusionAction::FallbackArq(R::LowConfidence);
        }
        if obs.inputs.loss_ppm > self.region.loss_ppm_max
            || obs.inputs.bandwidth_bps < self.region.bandwidth_bps_min
            || obs.inputs.bandwidth_bps > self.region.bandwidth_bps_max
        {
            return FusionAction::FallbackArq(R::UnsupportedRegion);
        }
        match policy.advise(obs) {
            FusionAdvice::ArqOnly => FusionAction::ArqOnly,
            FusionAdvice::FecAssist {
                redundancy_permille,
            } => {
                if !obs.fec_actuation_available {
                    // No product FEC send path exists today (POLICY-ONLY) — never pretend otherwise.
                    return FusionAction::FallbackArq(R::FecUnavailable);
                }
                let r = redundancy_permille.min(self.cfg.hard_max_redundancy_permille);
                if r == 0 {
                    return FusionAction::ArqOnly;
                }
                let want = payload_bytes.saturating_mul(u64::from(r)) / 1000;
                let budget = want
                    .min(self.cfg.hard_fec_byte_budget)
                    .min(self.cfg.hard_combined_byte_cap);
                if budget == 0 {
                    return FusionAction::ArqOnly;
                }
                FusionAction::FecAssist {
                    redundancy_permille: r,
                    byte_budget: budget,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(loss_ppm: u64) -> FusionObservation {
        FusionObservation {
            feature_schema: FEATURE_SCHEMA_V1,
            inputs: FusionInputs {
                loss_ppm,
                bandwidth_bps: 110_000,
                jitter_ppm: 10_000,
                qd_ratio_permille: 100,
                ack_confidence_permille: 900,
            },
            rtt_sample_count: 8,
            telemetry_age_ns: 1_000_000,
            converged: true,
            fec_actuation_available: true,
        }
    }

    // ── default-OFF exactness ──────────────────────────────────────────────────
    #[test]
    fn disabled_always_arq_only() {
        let p = FusionPolicy::disabled();
        for l in [0u64, 1, 50_000, 90_000, 200_000, 999_999] {
            assert_eq!(
                p.decide(&obs(l), 4848),
                FusionAction::FallbackArq(FallbackReason::PolicyDisabled)
            );
            assert!(p.decide(&obs(l), 4848).is_arq_only());
        }
    }

    // ── calibrated membership boundaries (below / at / above) ──────────────────
    #[test]
    fn loss_membership_boundaries_exact() {
        let p = FusionPolicy::calibration_v1();
        assert_eq!(p.loss_high_permille(159_999), 0, "just below HIGH start");
        assert_eq!(p.loss_high_permille(160_000), 0, "exactly at HIGH start");
        // one ppm above the start is 1*1000/40000 = 0.025 permille, which truncates to 0 (integer
        // ramp, no rounding-up): the first measurable step is one full permille at +40 ppm.
        assert_eq!(
            p.loss_high_permille(160_001),
            0,
            "just above HIGH start (sub-permille)"
        );
        assert_eq!(
            p.loss_high_permille(160_040),
            1,
            "first measurable permille step"
        );
        assert_eq!(p.loss_high_permille(180_000), 500, "midpoint");
        assert_eq!(p.loss_high_permille(199_999), 999, "just below full HIGH");
        assert_eq!(p.loss_high_permille(200_000), 1000, "exactly full HIGH");
        assert_eq!(p.loss_high_permille(1_000_000), 1000, "saturates");
    }

    #[test]
    fn calibration_corrects_the_fixture_defect() {
        // THE calibration finding: the uncalibrated fixture classifies 200_000 ppm as fully LOW and
        // fires R1 (fec_zero). The calibrated policy classifies it fully HIGH and recommends FEC —
        // the region where the authoritative data shows FEC_R2 Pareto-dominates.
        let p = FusionPolicy::calibration_v1();
        assert_eq!(p.loss_high_permille(200_000), 1000);
        match p.decide(&obs(200_000), 4848) {
            FusionAction::FecAssist {
                redundancy_permille,
                byte_budget,
            } => {
                assert_eq!(redundancy_permille, 800);
                assert!(byte_budget > 0 && byte_budget <= 4096);
            }
            other => panic!("expected FecAssist at 200k ppm, got {other:?}"),
        }
    }

    #[test]
    fn sparse_tail_region_stays_arq_only() {
        // ARQ_TAIL Pareto-dominant evidence band: 20_833..83_333 ppm (drop 1..4 of 48 units).
        let p = FusionPolicy::calibration_v1();
        for l in [20_833u64, 41_667, 83_333, 90_000] {
            assert_eq!(
                p.decide(&obs(l), 4848),
                FusionAction::ArqOnly,
                "loss {l} must stay ARQ-only"
            );
        }
    }

    // ── safety validator: every invalid/unknown/unsupported path ⇒ ArqOnly ─────
    #[test]
    fn unsupported_region_falls_back() {
        let p = FusionPolicy::calibration_v1();
        let mut o = obs(200_001); // above calibrated evidence
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::UnsupportedRegion)
        );
        o = obs(200_000);
        o.inputs.bandwidth_bps = 10_000; // outside tested regime
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::UnsupportedRegion)
        );
    }

    #[test]
    fn invalid_input_falls_back() {
        let p = FusionPolicy::calibration_v1();
        let mut o = obs(1_000_001);
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::InvalidInput)
        );
        o = obs(200_000);
        o.inputs.ack_confidence_permille = 1001;
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::InvalidInput)
        );
    }

    #[test]
    fn stale_cold_start_and_low_confidence_fall_back() {
        let p = FusionPolicy::calibration_v1();
        let mut o = obs(200_000);
        o.telemetry_age_ns = 5_000_000_000;
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::StaleTelemetry)
        );
        o = obs(200_000);
        o.converged = false;
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::ColdStart)
        );
        o = obs(200_000);
        o.rtt_sample_count = 1; // jitter bootstrap artifact guard
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::LowConfidence)
        );
    }

    #[test]
    fn version_mismatch_falls_back() {
        let p = FusionPolicy::calibration_v1();
        let mut o = obs(200_000);
        o.feature_schema = FEATURE_SCHEMA_V1 + 1;
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::VersionMismatch)
        );
    }

    #[test]
    fn fec_unavailable_falls_back() {
        // Product reality today: no product FEC send path ⇒ never emit a FEC action.
        let p = FusionPolicy::calibration_v1();
        let mut o = obs(200_000);
        o.fec_actuation_available = false;
        assert_eq!(
            p.decide(&o, 4848),
            FusionAction::FallbackArq(FallbackReason::FecUnavailable)
        );
    }

    // ── hard caps are independent of calibration ──────────────────────────────
    #[test]
    fn hard_caps_clamp_and_calibration_cannot_raise_them() {
        let tight = FusionSafetyConfig {
            hard_max_redundancy_permille: 100,
            hard_fec_byte_budget: 64,
            ..Default::default()
        };
        let p = FusionPolicy::calibration_v1().with_safety(tight);
        match p.decide(&obs(200_000), 1_000_000) {
            FusionAction::FecAssist {
                redundancy_permille,
                byte_budget,
            } => {
                assert_eq!(redundancy_permille, 100, "clamped by hard cap");
                assert_eq!(byte_budget, 64, "clamped by hard byte budget");
            }
            other => panic!("expected clamped FecAssist, got {other:?}"),
        }
    }

    #[test]
    fn combined_cap_bounds_budget() {
        let cfg = FusionSafetyConfig {
            hard_combined_byte_cap: 32,
            ..Default::default()
        };
        let p = FusionPolicy::calibration_v1().with_safety(cfg);
        match p.decide(&obs(200_000), 1_000_000) {
            FusionAction::FecAssist { byte_budget, .. } => assert!(byte_budget <= 32),
            other => panic!("expected FecAssist, got {other:?}"),
        }
    }

    // ── determinism + overflow safety ─────────────────────────────────────────
    #[test]
    fn determinism_identical_input_identical_decision() {
        let p = FusionPolicy::calibration_v1();
        for l in [0u64, 90_000, 160_001, 200_000] {
            let a = p.decide(&obs(l), 4848);
            for _ in 0..64 {
                assert_eq!(p.decide(&obs(l), 4848), a);
            }
        }
    }

    #[test]
    fn overflow_safe_extremes() {
        let p = FusionPolicy::calibration_v1();
        let mut o = obs(1_000_000);
        o.inputs.bandwidth_bps = u64::MAX;
        o.telemetry_age_ns = u64::MAX;
        // must not panic and must be a bounded fallback
        assert!(p.decide(&o, u64::MAX).is_arq_only());
        let _ = p.loss_high_permille(u64::MAX);
    }

    #[test]
    fn exhaustive_boundary_scan_is_bounded_and_monotone() {
        // scan the calibrated loss domain: no panic, bounded output, monotone non-decreasing
        let p = FusionPolicy::calibration_v1();
        let mut prev = 0u32;
        let mut l = 0u64;
        while l <= 1_000_000 {
            let m = p.loss_high_permille(l);
            assert!(m <= 1000, "membership bounded");
            assert!(m >= prev, "monotone non-decreasing at {l}");
            prev = m;
            let act = p.decide(&obs(l), 4848);
            if let FusionAction::FecAssist {
                redundancy_permille,
                byte_budget,
            } = act
            {
                assert!(redundancy_permille <= 800);
                assert!(byte_budget <= 4096);
            }
            l += 137; // dense non-aligned step to hit non-breakpoint interiors
        }
    }

    #[test]
    fn advice_is_pure_and_never_more_aggressive_than_action() {
        // the safety validator may only downgrade, never upgrade
        let p = FusionPolicy::calibration_v1();
        for l in (0..=1_000_000u64).step_by(9_973) {
            let mut o = obs(l);
            o.fec_actuation_available = false;
            let act = p.decide(&o, 4848);
            assert!(
                act.is_arq_only(),
                "no FEC actuation available ⇒ always ARQ-only"
            );
        }
    }
}
