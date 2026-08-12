//! FS-CAL-2 v3 YETKİ-1 — pin/golden suite: generators (§6.0), policy inference (§7.3),
//! actuator/product-path goldens (§4.3), estimator-snapshot pairing identity (§7.2).
//!
//! DEFAULT cargo features ON PURPOSE (no experimental-*): the FS-CAL-2 ARQ contract binds to the
//! same compile configuration as the DR-ARQ-PRODUCT-TAIL-1 product validation.
//! Normative: FS-CAL-2-EXPERIMENT-DESIGN-v3 (sha 115fa266…3393).
#![allow(clippy::duplicate_mod)]

#[path = "common/fs_cal2_schema.rs"]
mod fs_cal2_schema;

use adaptive_transport::fusion_policy::FusionSafetyConfig;
use fs_cal2_schema::fs_cal2_spine::fs_cal2_gen::{self, LossModel, StreamClass};
use fs_cal2_schema::fs_cal2_spine::fs_cal2_policy::fixture_policy;
use fs_cal2_schema::fs_cal2_spine::*;

fn cfg(cell: &str, arm: Arm, model: LossModel, tail: usize, rtt: u64) -> FlowCfg {
    FlowCfg {
        cell_id: cell.into(),
        repetition: 0,
        seed_domain: "FSFC2-PINS".into(),
        rtt_ms: rtt,
        source_model: model,
        parity_model: LossModel::None,
        retx_model: LossModel::None,
        tail_len: tail,
        arm,
        stop_at_epoch: false,
        tail_on: true,
        startup_pacing_init: None,
        diag: None,
    }
}

// ── gen surface ───────────────────────────────────────────────────────────────
#[test]
fn gen_surface_smoke() {
    let key = fs_cal2_gen::stream_key("FSFC2-SMOKE", "c0", 0, StreamClass::InitialSource);
    let bits = fs_cal2_gen::loss_bits(&key, LossModel::Iid { p_ppm: 100_000 }, 384);
    assert_eq!(bits.len(), 384);
    assert_eq!(
        fs_cal2_gen::markov_bd_transitions(200_000, 350_000),
        (480_000, 130_000)
    );
}

// ── §4.3: R0 determinism (identical wire digest across runs) ─────────────────
#[test]
fn r0_determinism_wire_digest_exact() {
    let c = cfg(
        "pin-iid20",
        Arm::StaticR0,
        LossModel::Iid { p_ppm: 200_000 },
        0,
        50,
    );
    let a = run_flow(&c);
    let b = run_flow(&c);
    assert_eq!(
        a.wire_digest, b.wire_digest,
        "R0 trace digest must be reproducible"
    );
    assert_eq!(a.snapshot.features, b.snapshot.features);
    assert_eq!(a.complete, b.complete);
    assert_eq!(a.total_wire_bytes, b.total_wire_bytes);
    assert_eq!(a.inv_fail, 0);
}

// ── §4.3: CALIBRATED_POLICY that resolves to R0 == STATIC_R0 wire-exact ──────
#[test]
fn calibrated_r0_equals_static_r0_wire_exact() {
    // no-loss warm-up → measured_loss 0 → mu_high_loss=0 → BELOW_ACTIVATION → effective R0.
    let base = cfg("pin-noloss", Arm::StaticR0, LossModel::None, 0, 50);
    let mut pol_cfg = base.clone();
    pol_cfg.arm = Arm::Calibrated(fixture_policy(WORKLOAD_PROFILE_ID, ARQ_CONTRACT_ID));
    let r0 = run_flow(&base);
    let pol = run_flow(&pol_cfg);
    assert_eq!(pol.effective_r, "R0");
    // no-loss warm-up ⇒ n10+n11 = 0 < 8 ⇒ §3.4 estimator ineligibility (design H2: no-loss → R0).
    assert_eq!(pol.fallback_reason, "ESTIMATOR_INELIGIBLE");
    assert!(!pol.snapshot.estimator_eligible);
    assert_eq!(
        pol.wire_digest, r0.wire_digest,
        "policy plumbing at R0 must be wire-exact vs STATIC_R0"
    );
    assert_eq!(pol.parity_bytes, 0);
    assert_eq!(pol.r0_parity_leak, 0);
}

// ── §4.3 + §7.2: pairing identity — R0/R2 share source prefix + snapshot ─────
#[test]
fn r0_r2_source_prefix_and_snapshot_identical() {
    let c0 = cfg(
        "pin-pair-iid20",
        Arm::StaticR0,
        LossModel::Iid { p_ppm: 200_000 },
        0,
        50,
    );
    let mut c2 = c0.clone();
    c2.arm = Arm::StaticR2;
    let r0 = run_flow(&c0);
    let r2 = run_flow(&c2);
    assert_eq!(
        r0.source_prefix_digest, r2.source_prefix_digest,
        "R0/R2 pairing group must share the InitialSource prefix (§5.3)"
    );
    assert_eq!(
        r0.snapshot.features, r2.snapshot.features,
        "pre-treatment estimator snapshot must be arm-invariant (§7.2)"
    );
    assert_eq!(r0.parity_bytes, 0);
    assert!(r2.parity_bytes > 0, "STATIC_R2 must actually emit parity");
}

// ── §4.3: R2 parity wire proof (product-path emission; exact bytes) ──────────
#[test]
fn r2_parity_wire_proof_exact() {
    let c = cfg("pin-r2-clean", Arm::StaticR2, LossModel::None, 0, 50);
    let r = run_flow(&c);
    assert_eq!(r.effective_r, "R2");
    assert_eq!(
        r.parity_bytes,
        u64::from(R2_PARITY) * SEALED,
        "exactly r=2 parity units of sealed 101 B through the product queue"
    );
    assert_eq!(r.base_source_bytes, BLOCK_LEN * SEALED);
    assert_eq!(r.inv_fail, 0);
    assert!(r.complete, "clean channel completes");
}

// ── §4.3: R0 zero parity ─────────────────────────────────────────────────────
#[test]
fn r0_zero_parity() {
    let c = cfg("pin-r0-clean", Arm::StaticR0, LossModel::None, 0, 50);
    let r = run_flow(&c);
    assert_eq!(r.parity_bytes, 0);
    assert_eq!(r.r0_parity_leak, 0);
    assert!(r.complete);
    assert_eq!(r.retransmits, 0, "no-loss ⇒ no recovery emissions");
}

// ── §4.3: FEC decode success — exact reassembly via RsSimdCodec ──────────────
#[test]
fn fec_decode_success_recovers_tail() {
    // last 2 decision originals dropped; parity clean → counting-MDS decode recovers both.
    let c = cfg("pin-decode", Arm::StaticR2, LossModel::None, 2, 50);
    let r = run_flow(&c);
    assert!(r.complete, "decode path must complete the block");
    assert!(r.fec_decode_success);
    assert_eq!(
        r.fec_decoded_sources, 2,
        "both dropped sources recovered by RS"
    );
    assert!(
        r.reassembly_equal,
        "recovered bytes byte-equal to canonical"
    );
    assert_eq!(r.inv_fail, 0);
}

// ── §4.3: FEC decode FAILURE does not block ARQ recovery (liveness) ──────────
#[test]
fn fec_decode_failure_arq_liveness() {
    // last 2 decision originals dropped AND all parity dropped → decode impossible;
    // the tail-capable ARQ (bounded probe) must still complete the block.
    let mut c = cfg("pin-liveness", Arm::StaticR2, LossModel::None, 2, 50);
    c.parity_model = LossModel::Iid { p_ppm: 1_000_000 }; // drop every parity packet
    let r = run_flow(&c);
    assert!(r.complete, "ARQ liveness: completion without FEC decode");
    assert_eq!(r.fec_decoded_sources, 0);
    assert!(r.retransmits >= 1, "recovery emissions present");
    assert_eq!(r.inv_fail, 0);
}

// ── §4.3: hard caps grounded (FusionSafetyConfig) ────────────────────────────
#[test]
fn r2_within_grounded_hard_caps() {
    let caps = FusionSafetyConfig::default();
    let parity_total = u64::from(R2_PARITY) * SEALED;
    assert!(
        parity_total <= caps.hard_fec_byte_budget,
        "202 ≤ hard_fec_byte_budget(4096)"
    );
    assert!(
        parity_total <= caps.hard_combined_byte_cap,
        "202 ≤ hard_combined_byte_cap(8192)"
    );
}

// ── ARQ contract liveness: terminal tail recovered by the product tail probe ─
#[test]
fn terminal_tail_r0_probe_recovers() {
    let c = cfg("pin-tail-r0", Arm::StaticR0, LossModel::None, 1, 50);
    let r = run_flow(&c);
    assert!(r.complete, "tail-capable ARQ recovers a pure terminal tail");
    assert!(
        r.probe_sent >= 1,
        "recovery via the product bounded tail probe"
    );
    assert_eq!(r.parity_bytes, 0);
    assert_eq!(r.inv_fail, 0);
}

// ── estimator snapshot sanity on a lossy warm-up (IID 20%) ───────────────────
#[test]
fn estimator_snapshot_iid20_sane_and_eligible() {
    let c = cfg(
        "pin-est-iid20",
        Arm::StaticR0,
        LossModel::Iid { p_ppm: 200_000 },
        0,
        50,
    );
    let r = run_flow(&c);
    let f = &r.snapshot.features;
    assert_eq!(
        f.n as usize,
        adaptive_transport::fate_estimator::FATE_WINDOW
    );
    assert!(
        r.snapshot.estimator_eligible,
        "IID-20 warm-up must be estimator-eligible (n10+n11={}, n00+n01={}, warmup_complete={}, converged={}, stale={})",
        f.n10 + f.n11,
        f.n00 + f.n01,
        r.snapshot.warmup_complete,
        r.snapshot.flow_converged,
        r.snapshot.telemetry_stale
    );
    // measured loss within ±6pp of configured 20% (deterministic given the pinned seed)
    assert!(
        (140_000..=260_000).contains(&f.measured_loss_ppm),
        "measured_loss_ppm={} outside sanity band",
        f.measured_loss_ppm
    );
    // IID burst-delta near zero (loose deterministic band)
    assert!(
        f.burst_delta_ppm.abs() < 120_000,
        "IID burst_delta_ppm={} unexpectedly structured",
        f.burst_delta_ppm
    );
}

// ── DR-RATE-RAMP-2 §FAZ-1: permanent regression pins for the startup pacing initialization ──
// The two seeds below are the deterministic post-convergence rate-lock reproducers found by
// DR-RATE-RAMP-1 (self-clocked fixed point: burst_cap pinned at one sealed unit, BtlBw pinned at
// SEALED/srtt). With the product default they must NOT lock.

fn rr_cfg(rep: u32, startup_pacing_init: Option<bool>) -> FlowCfg {
    FlowCfg {
        cell_id: "P2-GE20L10-RTT1000".into(),
        repetition: rep,
        seed_domain: "FSFC2-DIAG-CS-V32".into(),
        rtt_ms: 1000,
        source_model: LossModel::Ge {
            p_ppm: 200_000,
            l: 10,
        },
        parity_model: LossModel::Ge {
            p_ppm: 200_000,
            l: 10,
        },
        retx_model: LossModel::Ge {
            p_ppm: 200_000,
            l: 10,
        },
        tail_len: 0,
        arm: Arm::StaticR0,
        stop_at_epoch: true,
        tail_on: true,
        startup_pacing_init,
        diag: Some(DiagOpts {
            capture: true,
            warmup_timeout_override_ns: Some(65_000_000_000),
            sample_period_ns: 100_000_000,
        }),
    }
}

/// Scoped to the SHIPPING (default) build. Under `experimental-cold-start` the LEGACY cold-start
/// ladder emits paced probes that consume PreConverge tokens, which perturbs the trajectory and stops
/// the baseline arm from reproducing the burst-clamp lock — so the pin's premise does not hold there.
/// The property it guards (the shipped rate-ramp fix) is a default-build property.
#[cfg(not(feature = "experimental-cold-start"))]
#[test]
fn rate_lock_seeds_escape_the_fixed_point() {
    for rep in [2u32, 31] {
        let base = run_flow(&rr_cfg(rep, Some(false)));
        let fixed = run_flow(&rr_cfg(rep, Some(true)));
        let bd = base.diag.as_ref().unwrap();
        let fd = fixed.diag.as_ref().unwrap();
        let bcap = bd.samples.iter().map(|s| s.burst_cap).max().unwrap_or(0);
        let fcap = fd.samples.iter().map(|s| s.burst_cap).max().unwrap_or(0);
        let brate = bd.samples.iter().map(|s| s.send_rate_bps).max().unwrap_or(0);
        let frate = fd.samples.iter().map(|s| s.send_rate_bps).max().unwrap_or(0);
        // baseline reproduces the lock: burst clamp never leaves the one-sealed-unit floor
        assert_eq!(bcap, SEALED, "rep{rep}: baseline must reproduce the burst-clamp lock");
        // the shipped default must break it
        assert!(fcap > SEALED, "rep{rep}: startup pacing init must release the burst clamp");
        assert!(
            frate > 10 * brate,
            "rep{rep}: rate must leave the fixed point (base {brate} -> fix {frate})"
        );
        // and it must never write synthetic samples into the BtlBw max-filter
        let last = fd.samples.last().unwrap();
        assert!(
            last.bw_accepted <= last.bw_offered,
            "rep{rep}: accepted samples must never exceed offered (no synthetic writes)"
        );
    }
}

#[test]
fn startup_pacing_floor_is_bootstrap_only_and_released() {
    let out = run_flow(&rr_cfg(2, Some(true)));
    let d = out.diag.as_ref().unwrap();
    // armed at some point ...
    assert!(
        d.samples.iter().any(|s| s.startup_floor_bps > 0),
        "floor must arm at the PreConverge exit"
    );
    // ... and released before the run ends (never a permanent minimum rate)
    assert_eq!(
        d.samples.last().unwrap().startup_floor_bps,
        0,
        "floor must be released once startup completes / the measured rate catches up"
    );
}

/// Default-tracking pin: the PRODUCT DEFAULT path (`startup_pacing_init: None`) must be behaviourally
/// EQUIVALENT to the arm that matches the current default — same wire, same estimator snapshot.
///
/// DR-RATE-RAMP-2B §A3.2/§A4: the default was reverted **ON → OFF** (unresolved EK4-A2 deadline-exposure
/// uncertainty; see `reports/dr-rate-ramp-2b/`). This pin exists to keep the *default* honest, so its
/// reference arm follows the default. Nothing is weakened: it still asserts exact digest/epoch/window
/// equality, and `explicit_arm_differs_from_default` below still proves the axis is real.
#[test]
fn default_equals_matching_explicit_treatment() {
    for rep in [2u32, 16, 31] {
        let d = run_flow(&rr_cfg(rep, None)); // product default (now OFF)
        let e = run_flow(&rr_cfg(rep, Some(false))); // explicit arm matching the default
        assert_eq!(d.wire_digest, e.wire_digest, "rep{rep}: wire digest must match");
        assert_eq!(
            d.source_prefix_digest, e.source_prefix_digest,
            "rep{rep}: source prefix digest must match"
        );
        assert_eq!(
            d.snapshot.observation_epoch_ns, e.snapshot.observation_epoch_ns,
            "rep{rep}: decision epoch must match"
        );
        assert_eq!(
            d.snapshot.features.n, e.snapshot.features.n,
            "rep{rep}: estimator window must match"
        );
    }
}

/// The arm OPPOSITE to the default must still differ (the axis is real, not a no-op).
/// DR-RATE-RAMP-2B: default is now OFF, so the opposite arm is `Some(true)`. The assertion itself is
/// unchanged — the startup-pacing axis must produce a different wire.
#[test]
fn explicit_arm_differs_from_default() {
    let default_arm = run_flow(&rr_cfg(2, None));
    let opposite = run_flow(&rr_cfg(2, Some(true)));
    assert_ne!(
        default_arm.wire_digest, opposite.wire_digest,
        "the startup-pacing axis must actually change behaviour"
    );
}
