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
    assert!(d.on_ack(&[6], 0).is_empty()); // gap1 → bekle
    assert!(d.on_ack(&[7], 0).is_empty()); // gap2 → bekle
    assert_eq!(d.on_ack(&[8], 0), vec![5]); // gap3 → seq5 kayıp
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

    // stateful: min_delay explicit parametre + [TUR-1a] later-ACK preconditionı.
    let cfg = LossConfig {
        min_delay: dms(10),
        ..LossConfig::default()
    };
    let mut d = ThresholdLossDetector::with_config(cfg);
    d.on_sent(10, 0, 1);
    // [TUR-1a / D-1/1] SAF SESSİZLİK (daha yüksek seq ACK'lenmemiş): time-threshold KESİN loss
    // SINIFLANDIRAMAZ — eşik geçilse bile boş döner (tail-recovery liveness'ın işi, §5.1).
    assert!(d
        .detect_time_losses(ns(120), dms(100), Duration::ZERO)
        .is_empty()); // kanıt yok → loss yazılmaz
    assert_eq!(d.lost_units(), 0, "saf sessizlik loss metriğini artırmaz");
    // Kanıt gelsin: seq 11 gönderilip ACK'lenir → seq 10 için later-ACK kanıtı var.
    d.on_sent(11, 0, 1);
    d.on_ack(&[11], 0); // largest_acked=11 (packet-threshold: 11−10=1 < 3 → o yoldan kayıp değil)
    assert!(d
        .detect_time_losses(ns(100), dms(100), Duration::ZERO)
        .is_empty()); // 100 < 112.5 (eşik dolmadı)
    assert_eq!(
        d.detect_time_losses(ns(120), dms(100), Duration::ZERO),
        vec![10]
    ); // kanıt + eşik → kayıp
}

#[test]
fn tv_loss_g2b_cadence_floor_guards_evidence_path() {
    // [TUR-1b / D-1/2] Cadence-farkında taban: later-ACK kanıtı VARKEN bile elapsed,
    // K_c×EAI tabanını aşmadan sınıflandırma YOK ("SACK'i henüz gelmedi" ≠ "kayıp").
    // srtt=100ms → 9/8·srtt=112.5ms; cadence_floor=3s → efektif eşik 3s.
    let cfg = LossConfig {
        min_delay: dms(10),
        ..LossConfig::default()
    };
    let mut d = ThresholdLossDetector::with_config(cfg);
    d.on_sent(10, 0, 1);
    d.on_sent(11, 0, 1);
    d.on_ack(&[11], 0); // kanıt: largest_acked=11
    let floor = Duration::from_secs(3);
    assert!(
        d.detect_time_losses(ns(120), dms(100), floor).is_empty(),
        "120ms > 112.5ms AMA < 3s cadence-tabanı → sınıflandırma yok"
    );
    assert!(
        d.detect_time_losses(ns(2_900), dms(100), floor).is_empty(),
        "taban sınırının hemen altı → hâlâ yok"
    );
    assert_eq!(
        d.detect_time_losses(ns(3_100), dms(100), floor),
        vec![10],
        "taban aşıldı + kanıt var → kayıp"
    );
    // Taban srtt-kolunu KÜÇÜLTEMEZ: floor < 9/8·srtt ise eşik yine 9/8·srtt.
    let mut d2 = ThresholdLossDetector::with_config(cfg);
    d2.on_sent(20, 0, 1);
    d2.on_sent(21, 0, 1);
    d2.on_ack(&[21], 0);
    assert!(d2.detect_time_losses(ns(100), dms(100), dms(1)).is_empty()); // 100 < 112.5 (taban etkisiz-küçük)
}

#[test]
fn tv_loss_g5_ledger_unloss_and_windowed_decay() {
    // [TUR-2a D-1/3, D-1/5] Olay-ledger: Suspected → Spurious (un-loss) | Confirmed (ufuk geçti);
    // loss_windowed_ppm pencere-türetimli — ratchet YAPISAL ölür.
    let mut d = ThresholdLossDetector::new();
    for seq in 1..=10 {
        d.on_sent(seq, 0, 1); // t=0'da 10 birim (pencere-paydası)
    }
    // ack [4..=10] @t=0 → {1,2,3} packet-threshold Suspected.
    let acked: Vec<u64> = (4..=10).collect();
    assert_eq!(d.on_ack(&acked, 0), vec![1, 2, 3]);
    // Henüz Confirmed yok → windowed ppm 0 (Suspected sayılmaz — D-1/3 literal).
    d.ledger_tick(ns(100), dms(10_000), dms(3_000)); // 100ms < confirm(3s)
    assert_eq!(d.loss_windowed_ppm(), 0, "Suspected henüz sayılmaz");
    // UN-LOSS: seq 2'ye geç-ACK @t=1s (< confirm ufku) → Spurious + drain listesi.
    assert!(d.on_ack(&[2], ns(1_000)).is_empty());
    assert_eq!(d.take_unlossed(), vec![2], "geri-alınan seq drain edilir");
    assert_eq!(d.spurious_units_total(), 1);
    // Ufuk geçti (t=4s ≥ 3s): kalan {1,3} Confirmed → ppm = 2/10 = 200_000.
    d.ledger_tick(ns(4_000), dms(10_000), dms(3_000));
    assert_eq!(d.loss_windowed_ppm(), 200_000, "Confirmed/pencere-toplamı");
    // RATCHET-ÖLÜMÜ: pencere (10s) dışına yaşlanınca olaylar + paydalar düşer → ppm 0'a DÖNER.
    d.ledger_tick(ns(20_000), dms(10_000), dms(3_000));
    assert_eq!(
        d.loss_windowed_ppm(),
        0,
        "pencere-dışı decay (lifetime-ratchet öldü)"
    );
    // Lifetime sayaç (diag) monoton kalır — iki ayrı anlam.
    assert_eq!(d.lost_units(), 3);
}

#[test]
fn tv_loss_g5b_adaptive_packet_threshold() {
    // [TUR-2a §9.3] set_packet_threshold: k yükselince aynı gap artık kayıp SAYILMAZ (reorder-tolerans).
    let mut d = ThresholdLossDetector::new();
    d.set_packet_threshold(5);
    for seq in 5..=10 {
        d.on_sent(seq, 0, 1);
    }
    assert!(d.on_ack(&[8], 0).is_empty(), "gap3 < k=5 → bekle");
    assert_eq!(d.on_ack(&[10], 0), vec![5], "gap5 ≥ k=5 → kayıp");
}

#[test]
fn tv_loss_g3_pre_fec_vs_residual_separate_counters() {
    // 10 birim gönder; ack [4..=10] → seq {1,2,3} kayıp (10-1,10-2,10-3 hepsi ≥3).
    let mut d = ThresholdLossDetector::new();
    for seq in 1..=10 {
        d.on_sent(seq, 0, 1);
    }
    let acked: Vec<u64> = (4..=10).collect();
    assert_eq!(d.on_ack(&acked, 0), vec![1, 2, 3]);
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
    d.on_ack(&acked, 0);
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
    assert_eq!(d.on_ack(&acked, 0), vec![1, 2, 3]);
    assert_eq!(d.loss_pre_fec_ppm(), 300_000);
}
