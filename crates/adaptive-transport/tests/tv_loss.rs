//! TV-LOSS golden vektörleri — loss detection (§5). loss = recovery sinyali, RATE
//! sinyali DEĞİL (INV-RATE-1). Değerler PR açıklamasında adım adım türetildi.

use adaptive_transport::loss::{
    packet_threshold_lost, time_threshold_lost, LossConfig, LossDetector, ThresholdLossDetector,
};
use adaptive_transport::rate::{evaluate, Gain, SrcConfig};
use std::time::Duration;

fn ns(ms: u64) -> u64 {
    ms * 1_000_000
}
fn dms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn tv_loss_g1_packet_threshold_reorder_vs_loss() {
    // K=3: gap<3 → reorder (bekle); gap≥3 → kayıp. seq 5 izlenir.
    assert!(!packet_threshold_lost(5, 6, 3)); // gap 1
    assert!(!packet_threshold_lost(5, 7, 3)); // gap 2
    assert!(packet_threshold_lost(5, 8, 3)); // gap 3 → lost

    let mut d = ThresholdLossDetector::new();
    for seq in 5..=8 {
        d.on_sent(seq, 0, 1);
    }
    assert!(d.on_ack(&[6]).is_empty()); // gap1 → bekle
    assert!(d.on_ack(&[7]).is_empty()); // gap2 → bekle
    assert_eq!(d.on_ack(&[8]), vec![5]); // gap3 → seq5 kayıp
    assert_eq!(d.lost_units(), 1);
}

#[test]
fn tv_loss_g2_time_threshold_and_min_delay() {
    // thresh = max(9/8·srtt, min_delay). min_delay [CAL] PARAMETRE (baked değil).
    // srtt=100ms → 9/8·100 = 112.5ms.
    assert!(!time_threshold_lost(dms(100), dms(100), dms(10), 9, 8)); // 100 < 112.5
    assert!(time_threshold_lost(dms(120), dms(100), dms(10), 9, 8)); // 120 > 112.5
                                                                     // tiny srtt=4ms → 9/8·4=4.5ms; min_delay=10ms korur → thresh=10ms.
    assert!(!time_threshold_lost(dms(8), dms(4), dms(10), 9, 8)); // 8 < 10
    assert!(time_threshold_lost(dms(12), dms(4), dms(10), 9, 8)); // 12 > 10

    // stateful: min_delay explicit parametre olarak geçilir.
    let cfg = LossConfig {
        min_delay: dms(10),
        ..LossConfig::default()
    };
    let mut d = ThresholdLossDetector::with_config(cfg);
    d.on_sent(10, 0, 1);
    assert!(d.detect_time_losses(ns(100), dms(100)).is_empty()); // 100 < 112.5
    assert_eq!(d.detect_time_losses(ns(120), dms(100)), vec![10]); // 120 > 112.5
}

#[test]
fn tv_loss_g3_pre_fec_vs_residual_separate_counters() {
    // 10 birim gönder; ack [4..=10] → seq {1,2,3} kayıp (10-1,10-2,10-3 hepsi ≥3).
    let mut d = ThresholdLossDetector::new();
    for seq in 1..=10 {
        d.on_sent(seq, 0, 1);
    }
    let acked: Vec<u64> = (4..=10).collect();
    assert_eq!(d.on_ack(&acked), vec![1, 2, 3]);
    assert_eq!(d.sent_units(), 10);
    assert_eq!(d.lost_units(), 3);
    assert_eq!(d.loss_pre_fec_ppm(), 300_000); // 3/10 = 0.3 (recovery ÖNCESİ)

    // recovery: 3'ten 2'si kurtarıldı → 1 unrecovered
    d.on_unrecovered(1);
    assert_eq!(d.residual_loss_ppm(), 100_000); // 1/10 = 0.1 (recovery SONRASI)
    assert_ne!(d.loss_pre_fec_ppm(), d.residual_loss_ppm()); // İKİ AYRI sayaç
}

#[test]
fn tv_loss_g4_orthogonal_to_rate_inv_rate_1() {
    // INV-RATE-1: loss sayacı ↑ iken send_rate ETKİLENMEZ. loss-detection ⟂ rate-control.
    let cfg = SrcConfig::default();
    let baseline = evaluate(&cfg, 62500, dms(100), dms(100), Gain::STEADY).send_rate;
    assert_eq!(baseline, 62500);

    let mut d = ThresholdLossDetector::new();
    for seq in 1..=10 {
        d.on_sent(seq, 0, 1);
    }
    let acked: Vec<u64> = (4..=10).collect();
    d.on_ack(&acked);
    assert_eq!(d.loss_pre_fec_ppm(), 300_000); // loss %30

    // Aynı (BtlBw, srtt) → loss'a bakmaksızın send_rate DEĞİŞMEZ.
    let after = evaluate(&cfg, 62500, dms(100), dms(100), Gain::STEADY).send_rate;
    assert_eq!(after, baseline); // 62500 — loss send_rate'e girmez
}

#[test]
fn tv_loss_via_trait_object() {
    // Port temizliği: dyn LossDetector üzerinden sürülebilir (füzyon §8.4 böyle tüketecek).
    let mut det = ThresholdLossDetector::new();
    let d: &mut dyn LossDetector = &mut det;
    for seq in 1..=10 {
        d.on_sent(seq, 0, 1);
    }
    let acked: Vec<u64> = (4..=10).collect();
    assert_eq!(d.on_ack(&acked), vec![1, 2, 3]);
    assert_eq!(d.loss_pre_fec_ppm(), 300_000);
}
