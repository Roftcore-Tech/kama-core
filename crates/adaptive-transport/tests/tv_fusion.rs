//! TV-FUZZY golden vektörleri — fuzzy karar motoru (§8.4/§15). Eşikler fixture
//! (KALİBRE DEĞİL, netem §598); golden motor matematiğini + kural tablosunu kilitler.
//! Değerler PR açıklamasında adım adım türetildi.

use adaptive_transport::fusion::{
    trap_permille, DeterministicFuzzyController, FusionController, FusionInputs, FuzzyConfig, Mode,
};
use adaptive_transport::rate::{effective_send_rate, Gain, SrcConfig};
use std::time::Duration;

fn dms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn tv_fusion_a_trapezoidal_membership() {
    // GOLDEN A: loss fixture membership (ppm) → (LOW, MED, HIGH) per-mille.
    let loss = FuzzyConfig::fixture().loss;
    let cases = [
        (100_000u64, 1000u32, 0u32, 0u32),
        (250_000, 500, 500, 0),
        (400_000, 0, 1000, 0),
        (550_000, 0, 500, 500),
        (700_000, 0, 0, 1000),
    ];
    for (x, low, med, high) in cases {
        let d = loss.fuzzify(x);
        assert_eq!((d.low, d.med, d.high), (low, med, high), "loss={x}");
    }
    // Shoulder: sol-omuz (a==b) ve sağ-omuz (c==d).
    assert_eq!(trap_permille(0, 0, 0, 200_000, 300_000), 1000); // LOW x=0
    assert_eq!(
        trap_permille(u64::MAX, 500_000, 600_000, u64::MAX, u64::MAX),
        1000
    ); // HIGH plateau
}

#[test]
fn tv_fusion_b_decision_s1() {
    // GOLDEN B: weighted-average karar. loss=550k(MED500,HIGH500) bw=175k(MED500,HIGH500)
    // jitter=50k(LOW1000) qd=0 ackconf=1000(HIGH).
    let ctrl = DeterministicFuzzyController::fixture();
    let d = ctrl.decide(&FusionInputs {
        loss_ppm: 550_000,
        bandwidth_bps: 175_000,
        jitter_ppm: 50_000,
        queue_delay: Duration::ZERO,
        ack_confidence_permille: 1000,
    });
    assert_eq!(d.fec_parity_ratio_permille, 650); // (500·800 + 500·500)/1000
    assert_eq!(d.arq_aggressiveness_permille, 550); // (500·600 + 500·500)/1000
    assert_eq!(d.send_rate_gain_permille, 1000); // bw not LOW → throttle yok
    assert!(!d.compress);
    assert_eq!(d.mode, Mode::Normal);
}

#[test]
fn tv_fusion_c_loss_decoupling() {
    // GOLDEN C: loss 550k→100k → FEC değişir AMA send_rate_gain SABİT (§5.4).
    let ctrl = DeterministicFuzzyController::fixture();
    let base = FusionInputs {
        loss_ppm: 550_000,
        bandwidth_bps: 175_000,
        jitter_ppm: 50_000,
        queue_delay: Duration::ZERO,
        ack_confidence_permille: 1000,
    };
    let s1 = ctrl.decide(&base);
    let s1p = ctrl.decide(&FusionInputs {
        loss_ppm: 100_000,
        ..base
    });

    assert_eq!(s1.fec_parity_ratio_permille, 650); // loss FEC'i etkiler
    assert_eq!(s1p.fec_parity_ratio_permille, 0);
    // send_rate_gain loss'a kör (bw+qd aynı):
    assert_eq!(s1.send_rate_gain_permille, s1p.send_rate_gain_permille);
    assert_eq!(s1.send_rate_gain_permille, 1000);
}

#[test]
fn tv_fusion_d_emergency_trigger() {
    // GOLDEN D: loss HIGH ∧ bw LOW ∧ ackconf LOW → emergency; send_rate_gain throttled.
    let ctrl = DeterministicFuzzyController::fixture();
    let d = ctrl.decide(&FusionInputs {
        loss_ppm: 650_000,
        bandwidth_bps: 25_000,
        jitter_ppm: 50_000,
        queue_delay: dms(500),
        ack_confidence_permille: 100,
    });
    assert_eq!(d.mode, Mode::EmergencyForwardRedundancy);
    assert_eq!(d.send_rate_gain_permille, 500); // bw LOW ∧ qd HIGH (loss-kör throttle)
    assert!(d.compress);
}

#[test]
fn tv_fusion_composition_with_src() {
    // send_rate_gain × SRC composition: min(pacing·gain·BtlBw, cwnd/srtt).
    let cfg = SrcConfig::default();
    // gain=1000 → throttle yok
    assert_eq!(
        effective_send_rate(&cfg, 62500, dms(100), dms(100), Gain::STEADY, 1000),
        62500
    );
    // gain=500 → fuzzy soft-drain
    assert_eq!(
        effective_send_rate(&cfg, 62500, dms(100), dms(100), Gain::STEADY, 500),
        31250
    );
    // gain=1000 ama srtt=500ms → cwnd/srtt HARD ceiling son söz
    assert_eq!(
        effective_send_rate(&cfg, 62500, dms(100), dms(500), Gain::STEADY, 1000),
        25000
    );
}

#[test]
fn tv_fusion_via_trait_object() {
    // Port temizliği: dyn FusionController üzerinden.
    let ctrl = DeterministicFuzzyController::fixture();
    let c: &dyn FusionController = &ctrl;
    let d = c.decide(&FusionInputs {
        loss_ppm: 550_000,
        bandwidth_bps: 175_000,
        jitter_ppm: 50_000,
        queue_delay: Duration::ZERO,
        ack_confidence_permille: 1000,
    });
    assert_eq!(d.fec_parity_ratio_permille, 650);
}
