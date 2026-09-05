//! [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi.
//!
//! Bu dosya üç şeyi savunur:
//!  (a) kol KAPALIYKEN davranış bugünküyle özdeş (kanonik kayıp tek girdi),
//!  (b) risk sinyali `Suspected` olaylara yaş-ağırlıklı güven verir ve `Spurious` gelince
//!      katkı KENDİLİĞİNDEN sıfırlanır,
//!  (c) sinyal ARQ/CC/pacing yollarına SIZMAZ — tek tüketici FEC oranı kararıdır.
//!
//! (c) bu kolun tüm meşruiyetidir: kanonik kayıp yeniden tanımlanmıyor, ayrı bir okuma ekleniyor.

use adaptive_transport::loss::{LossDetector, SuspectConfidence, ThresholdLossDetector};
use adaptive_transport::sender::*;
use std::time::Duration;

const MS: u64 = 1_000_000;

/// Akış sürücüsü: birim sun, RTT örneği ver, sonra timer pompala; `acked` kayıp deseni imal eder.
fn drive(mut s: Sender, units: u64, acked: Vec<u64>) -> Sender {
    s.set_establish_probe(false);
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=20u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        45_000 * MS,
    );
    for t in 221..=320u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s
}

fn gap_pattern() -> Vec<u64> {
    (1..400).filter(|q| q % 2 == 0).collect()
}

// ── (a) Kol KAPALI = sevk davranışı ──────────────────────────────────────────────────────────

#[test]
fn kol_varsayilan_olarak_kapalidir() {
    assert!(
        !Sender::new().fec_loss_risk_config().enabled,
        "deneysel kol default AÇIK olamaz — ölçülmemiş davranış sevk edilmez"
    );
}

/// Kol kapalıyken risk teşhisi hiç yazılmaz ve FEC oranı kanonik kayıptan gelir.
#[test]
fn kapali_kolda_risk_hesaplanmaz() {
    let s = drive(Sender::new(), 80, gap_pattern());
    assert_eq!(s.diag_risk_ppm(), (0, 0), "kapalı kolda risk ölçülmemeli");
    let (_, _, t_risk, _) = s.diag_fec_risk_times();
    assert_eq!(t_risk, 0, "kapalı kolda risk damgası basılmamalı");
}

// ── (b) Risk sinyali erken ve düzeltilebilir ─────────────────────────────────────────────────

/// Kolun ÇEKİRDEK iddiası: risk, onaydan ÖNCE sıfırdan çıkar. Aynı akış, tek eksen = bayrak.
#[test]
fn risk_onaydan_once_sifirdan_cikar() {
    let mut on = Sender::new();
    on.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let on = drive(on, 80, gap_pattern());

    let (t_susp, t_conf, t_risk, _) = on.diag_fec_risk_times();
    assert!(t_susp > 0, "önkoşul: kayıp kuşkusu ilan edilmeli");
    assert!(t_risk > 0, "risk sinyali sıfırdan ÇIKMALI");
    assert!(
        t_risk >= t_susp,
        "risk kuşkudan önce doğamaz (nedensellik): risk={t_risk} susp={t_susp}"
    );
    if t_conf > 0 {
        assert!(
            t_risk <= t_conf,
            "kolun TEK amacı bu: risk onaydan ÖNCE görünmeli (risk={t_risk} conf={t_conf})"
        );
    }
    assert!(on.diag_risk_ppm().1 > 0, "tepe risk sıfırdan büyük olmalı");
}

/// Ağırlık eğrisi monoton: yaşlanan kuşku daha çok güven taşır, ama onayın (1000‰) altında kalır.
#[test]
fn guven_egrisi_monoton_ve_onayin_altinda() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let a = w.for_age(srtt / 8, srtt); // < srtt/4
    let b = w.for_age(srtt / 3, srtt); // < srtt/2
    let c = w.for_age(3 * srtt / 4, srtt); // < srtt
    let d = w.for_age(2 * srtt, srtt); // >= srtt
    assert!(a < b && b < c, "eğri monoton artmalı: {a} {b} {c}");
    assert!(
        c <= d && d < 1000,
        "onaylanmamış kuşku ASLA tam güven (1000‰) almamalı: {d}"
    );
}

/// srtt ölçülmemişken kuşkuya güven VERİLMEZ → sonuç kanonikle özdeş. Ölçüm yokken risk uydurulmaz
/// (bu, düzeltmeye çalıştığımız "prior'dan eşik türetme" kusurunun tekrarı olurdu).
#[test]
fn srtt_yokken_risk_kanonikle_ozdes() {
    let mut d = ThresholdLossDetector::default();
    d.on_sent(1, 0, 1);
    d.on_sent(2, 0, 1);
    d.on_sent(3, 0, 1);
    d.on_sent(4, 0, 1);
    let _ = d.on_ack(&[4], 0);
    let _ = d.detect_time_losses(10 * MS, Duration::from_millis(1), Duration::from_millis(1));
    let canon = d.loss_windowed_ppm();
    let risk = d.fec_loss_risk_ppm(20 * MS, 0, SuspectConfidence::default());
    assert_eq!(risk, canon, "srtt yokken risk kanonik değerden SAPAMAZ");
}

/// Geç ACK gelince (`Spurious`) katkı sıfırlanır — tahmin kendi hatasını düzeltir.
/// Bu, reorder'lı kanalda yanlış-pozitif parity bedelinin ÜST SINIRINI belirleyen mekanizmadır.
#[test]
fn spurious_katkiyi_sifirlar() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    // 8'e ACK → 1..7 arasındaki boşluklar paket-eşiğiyle kayıp ilan edilir.
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    let before = d.fec_loss_risk_ppm(50 * MS, srtt, w);
    assert!(before > 0, "önkoşul: kuşku riski sıfırdan büyük olmalı");

    // Geç ACK'ler gelir → olaylar Spurious'a döner.
    let _ = d.on_ack(&[1, 2, 3, 4, 5, 6, 7], 60 * MS);
    let after = d.fec_loss_risk_ppm(70 * MS, srtt, w);
    assert_eq!(after, 0, "geç ACK gelen olaylar riske katkı VERMEMELİ");
}

// ── (c) İZOLASYON — sinyal FEC dışına sızmıyor ───────────────────────────────────────────────

/// Kolun meşruiyet kanıtı: bayrak AÇIK/KAPALI arasında ARQ agresifliği, mod, pacing hızı,
/// bant tahmini ve state-machine fazı DEĞİŞMEZ. Değişmesine izin verilen tek şey FEC oranıdır.
#[test]
fn risk_sinyali_arq_cc_ve_pacing_yollarina_sizmaz() {
    let off = drive(Sender::new(), 80, gap_pattern());
    let mut on_s = Sender::new();
    on_s.set_fec_loss_risk(FecLossRiskConfig::experimental_v1());
    let on = drive(on_s, 80, gap_pattern());

    // Önkoşul: kol gerçekten çalıştı (aksi hâlde test vakum olurdu).
    assert!(on.diag_risk_ppm().1 > 0, "önkoşul: risk sinyali üretilmeli");

    assert_eq!(
        off.state(),
        on.state(),
        "state-machine fazı risk sinyalinden ETKİLENMEMELİ"
    );
    assert_eq!(
        off.diag_rate(),
        on.diag_rate(),
        "hız düzlemi (state, pacing_rate, btlbw, cwnd, inflight) risk sinyalinden \
         ETKİLENMEMELİ — INV-RATE-1'in mekanik kanıtı"
    );
    assert_eq!(
        off.diag_ledger(),
        on.diag_ledger(),
        "kanonik kayıp defteri (unloss, loss_ppm, spurious) DEĞİŞMEMELİ"
    );
    assert_eq!(
        off.diag_confirm_path(),
        on.diag_confirm_path(),
        "onay yolu (T_confirm, confirmed_total) DEĞİŞMEMELİ — ARQ semantiği korunur"
    );
}

/// Kanonik metrik yeniden tanımlanmadı: `loss_windowed_ppm` hâlâ YALNIZ `Confirmed` sayar.
#[test]
fn kanonik_kayip_yeniden_tanimlanmadi() {
    let w = SuspectConfidence::default();
    let srtt = 200 * MS;
    let mut d = ThresholdLossDetector::default();
    for seq in 1..=8u64 {
        d.on_sent(seq, 0, 1);
    }
    let _ = d.on_ack(&[8], 0);
    let _ = d.detect_time_losses(MS, Duration::from_millis(1), Duration::from_millis(1));
    assert_eq!(
        d.loss_windowed_ppm(),
        0,
        "kanonik metrik Suspected'ı SAYMAMALI (ARQ semantiği)"
    );
    assert!(
        d.fec_loss_risk_ppm(50 * MS, srtt, w) > 0,
        "risk metriği ise Suspected'ı görmeli — iki metrik AYRI sorular sorar"
    );
}
