//! TV-EMERGENCY golden vektörleri — §8.5 budget clamp + §11 emergency davranışı +
//! retreat hysteresis. Değerler PR açıklamasında adım adım türetildi. [CAL] fixture.

use adaptive_transport::budget::{
    budget_cap_permille, parity_final, send_rate_final, BudgetConfig,
};
use adaptive_transport::emergency::{
    ack_decimation, action, EmergencyConfig, EmergencyLevel, Framing, HysteresisConfig,
    HysteresisController, RecoveryAction, RecoveryModeController,
};
use std::time::Duration;

fn ms(n: u64) -> u64 {
    n * 1_000_000
}

#[test]
fn tv_emergency_g1_parity_clamp() {
    // GOLDEN G1: dar bantta yüksek loss parity'yi PATLATAMAZ. budget_cap = min(900, BtlBw·1000/125000).
    let cfg = BudgetConfig::fixture();
    assert_eq!(budget_cap_permille(25_000, &cfg), 200);
    assert_eq!(budget_cap_permille(62_500, &cfg), 500);
    assert_eq!(budget_cap_permille(200_000, &cfg), 900); // 1600 → profile_cap

    let cap = cfg.profile_cap_permille;
    assert_eq!(
        parity_final(800, budget_cap_permille(25_000, &cfg), cap),
        200
    ); // budget bağlar (dar bant)
    assert_eq!(
        parity_final(800, budget_cap_permille(62_500, &cfg), cap),
        500
    ); // budget bağlar
    assert_eq!(
        parity_final(800, budget_cap_permille(200_000, &cfg), cap),
        800
    ); // fuzzy (bant yeter)
    assert_eq!(
        parity_final(1000, budget_cap_permille(200_000, &cfg), cap),
        900
    ); // profile_cap
}

#[test]
fn tv_emergency_g2_min_rate_orthogonal_to_loss() {
    // GOLDEN G2: MIN_RATE clamp ⟂ INV-RATE-1. fuzzy_rate bw+queue_delay'den (loss-kör).
    let cfg = BudgetConfig::fixture();
    let cap = 125_000;
    for _loss_pct in [0u32, 50, 80] {
        let fuzzy_rate = 31_250; // effective_send_rate(bw+qd) — loss girdi DEĞİL
        assert_eq!(send_rate_final(fuzzy_rate, cfg.min_rate_bps, cap), 31_250);
    }
    // floor: rate düşükse (queue throttle) MIN_RATE floor — delay-driven, loss'tan DEĞİL.
    assert_eq!(send_rate_final(5_000, cfg.min_rate_bps, cap), 10_000);
}

#[test]
fn tv_emergency_g3_behavior() {
    // GOLDEN G3: framing-ayrımlı davranış (§11). tiny→duplication, block→minimal parity.
    let cfg = EmergencyConfig::fixture();
    let sp = Duration::from_millis(20);

    assert_eq!(
        action(EmergencyLevel::Dup3, Framing::Tiny, &cfg),
        RecoveryAction::Duplication {
            factor: 3,
            spacing: sp
        }
    );
    assert_eq!(
        action(EmergencyLevel::Dup2, Framing::Tiny, &cfg),
        RecoveryAction::Duplication {
            factor: 2,
            spacing: sp
        }
    );
    assert_eq!(
        action(EmergencyLevel::Arq, Framing::Tiny, &cfg),
        RecoveryAction::Arq
    );
    assert_eq!(
        action(EmergencyLevel::Dup3, Framing::SegmentBlock, &cfg),
        RecoveryAction::MinimalParity { permille: 200 }
    );
    assert_eq!(
        action(EmergencyLevel::Dup2, Framing::SegmentBlock, &cfg),
        RecoveryAction::MinimalParity { permille: 100 }
    );

    // ACK decimation: Arq dışında sparse ACK.
    assert!(ack_decimation(EmergencyLevel::Dup3));
    assert!(ack_decimation(EmergencyLevel::Dup2));
    assert!(!ack_decimation(EmergencyLevel::Arq));
}

#[test]
fn tv_emergency_g4_hysteresis_no_oscillation() {
    // GOLDEN G4a: gürültülü health bant [300,700] İÇİNDE → flip YOK.
    let mut ctrl = HysteresisController::new(HysteresisConfig::fixture(), EmergencyLevel::Dup3);
    for (t, h) in [
        (100u64, 400u32),
        (200, 600),
        (300, 350),
        (400, 650),
        (500, 500),
    ] {
        assert_eq!(ctrl.update(h, ms(t)), EmergencyLevel::Dup3, "t={t} h={h}");
    }
}

#[test]
fn tv_emergency_g4_deescalate_dwell() {
    // GOLDEN G4b: de-escalate'ler ≥ dwell(1000ms) aralıklı.
    let mut ctrl = HysteresisController::new(HysteresisConfig::fixture(), EmergencyLevel::Dup3);
    assert_eq!(ctrl.update(750, ms(0)), EmergencyLevel::Dup2); // de-escalate
    assert_eq!(ctrl.update(750, ms(500)), EmergencyLevel::Dup2); // dwell locked (500<1000)
    assert_eq!(ctrl.update(750, ms(1000)), EmergencyLevel::Arq); // dwell açıldı
    assert_eq!(ctrl.update(750, ms(1200)), EmergencyLevel::Arq); // tekrar locked
}

#[test]
fn tv_emergency_g4_escalate_instant_bypasses_dwell() {
    // GOLDEN G4c: kötüleşme (health 750→250) → dwell BEKLEMEDEN escalate (recovery-obsesif).
    let mut ctrl = HysteresisController::new(HysteresisConfig::fixture(), EmergencyLevel::Dup3);
    assert_eq!(ctrl.update(750, ms(0)), EmergencyLevel::Dup2); // de-escalate (last_deescalate=0)
                                                               // t=100ms < dwell(1000ms) AMA escalate anında (dwell yalnız de-escalate'e uygulanır):
    assert_eq!(ctrl.update(250, ms(100)), EmergencyLevel::Dup3);
}

#[test]
fn tv_emergency_via_trait_object() {
    // Port temizliği: dyn RecoveryModeController üzerinden.
    let mut ctrl = HysteresisController::new(HysteresisConfig::fixture(), EmergencyLevel::Arq);
    let c: &mut dyn RecoveryModeController = &mut ctrl;
    assert_eq!(c.update(250, ms(0)), EmergencyLevel::Dup2); // escalate
    assert_eq!(c.level(), EmergencyLevel::Dup2);
}
