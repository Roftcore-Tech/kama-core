//! TV-BW golden vektörleri — delivery-rate sampler + max-filter BtlBw (§7).
//!
//! "ana yetenek" kanıtı: bandwidth delivered-rate'ten ölçülür, loss'tan bağımsız
//! (INV-ENGINE-BW). Değerler PR açıklamasında adım adım türetildi.

use adaptive_transport::bw::{bw_bps, BtlBwFilter, DeliveryRateSampler, RateSample};
use std::time::Duration;

/// ms → ns (u64 zaman damgası).
fn ns(ms: u64) -> u64 {
    ms * 1_000_000
}
/// ms → Duration.
fn dms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn tv_bw_sampler_golden() {
    // 1250B paket, 2-in-flight, bottleneck 20ms/pkt, RTT 40ms (engine §7.1).
    let mut s = DeliveryRateSampler::new();
    let p0 = s.on_sent(ns(0), 1250, false, true);
    let p1 = s.on_sent(ns(0), 1250, false, true);
    let r0 = s.on_ack(ns(40), p0);
    let p2 = s.on_sent(ns(40), 1250, false, true);
    let r1 = s.on_ack(ns(60), p1);
    let p3 = s.on_sent(ns(60), 1250, false, true);
    let r2 = s.on_ack(ns(80), p2);
    let p4 = s.on_sent(ns(80), 1250, false, true);
    let r3 = s.on_ack(ns(100), p3);
    let r4 = s.on_ack(ns(120), p4);

    // NEDEN r1/r3 = 41666 ama BtlBw yine 62500 (KAYIT):
    // r1 (ve r3) ÖRTÜŞEN-INTERVAL sample'ıdır. P1/P3'ün delivered_time snapshot'ı 0'dı
    // (ilk ACK'ten önce alındı) → ack_elapsed tüm 60ms'yi kapsar; bu interval'in ilk
    // 40ms'si P0 sample'ında ZATEN sayıldı (double-count) → düşük 41666. on_ack'teki
    // `first_sent = P.sent` güncellemesi sonraki interval'leri düzeltir → P2/P4 temiz
    // 40ms → 62500. Max-filter örtüşen 41666'yı ELER; BtlBw = 62500 (bottleneck).
    assert_eq!((r0.delivered, r0.interval), (1250, dms(40))); // 31250 B/s
    assert_eq!((r1.delivered, r1.interval), (2500, dms(60))); // 41666 (örtüşen-interval)
    assert_eq!((r2.delivered, r2.interval), (2500, dms(40))); // 62500 (bottleneck)
    assert_eq!((r3.delivered, r3.interval), (2500, dms(60))); // 41666 (örtüşen-interval)
    assert_eq!((r4.delivered, r4.interval), (2500, dms(40))); // 62500 (bottleneck)

    assert_eq!(bw_bps(r0.delivered, r0.interval), 31250);
    assert_eq!(bw_bps(r1.delivered, r1.interval), 41666); // truncation
    assert_eq!(bw_bps(r2.delivered, r2.interval), 62500);

    let mut f = BtlBwFilter::new();
    let mut btlbw = 0;
    for (i, r) in [r0, r1, r2, r3, r4].into_iter().enumerate() {
        btlbw = f.update(r, ns(40) * (i as u64 + 1), true);
    }
    assert_eq!(btlbw, 62500); // BtlBw = max delivered-rate; örtüşen 41666 elendi
}

#[test]
fn tv_bw_loss_does_not_reduce_btlbw() {
    // INV-ENGINE-BW / INV-RATE-1: loss BtlBw'i DÜŞÜRMEZ. P1,P3 (41666 örtüşen sample'lar)
    // kaybolursa surviving set [31250, 62500, 62500] → BtlBw yine 62500. bottleneck-rate
    // sample'ları hayatta olduğu sürece pencereli max düşmez.
    let s31250 = RateSample {
        delivered: 1250,
        interval: dms(40),
        is_app_limited: false,
        gain_ge1: true,
        ack_elapsed: Duration::ZERO,
        sent_ns: 0,
    };
    let s62500 = RateSample {
        delivered: 2500,
        interval: dms(40),
        is_app_limited: false,
        gain_ge1: true,
        ack_elapsed: Duration::ZERO,
        sent_ns: 0,
    };

    let mut f = BtlBwFilter::new();
    for (i, r) in [s31250, s62500, s62500].into_iter().enumerate() {
        f.update(r, ns(40) * (i as u64 + 1), true);
    }
    assert_eq!(f.btlbw(), 62500);
}

#[test]
fn tv_bw_app_limited_rule() {
    let mut f = BtlBwFilter::new();
    // Taban 62500.
    f.update(
        RateSample {
            delivered: 2500,
            interval: dms(40),
            is_app_limited: false,
            gain_ge1: true,
            ack_elapsed: Duration::ZERO,
            sent_ns: 0,
        },
        0,
        true,
    );
    assert_eq!(f.btlbw(), 62500);

    // app-limited 20000 (< BtlBw) → max-filter'a GİRMEZ.
    f.update(
        RateSample {
            delivered: 1000,
            interval: dms(50),
            is_app_limited: true,
            gain_ge1: true,
            ack_elapsed: Duration::ZERO,
            sent_ns: 0,
        },
        ns(10),
        true,
    );
    assert_eq!(f.btlbw(), 62500);

    // app-limited 80000 (≥ BtlBw) → BtlBw yükselir (geçerli alt-sınır).
    f.update(
        RateSample {
            delivered: 4000,
            interval: dms(50),
            is_app_limited: true,
            gain_ge1: true,
            ack_elapsed: Duration::ZERO,
            sent_ns: 0,
        },
        ns(20),
        true,
    );
    assert_eq!(f.btlbw(), 80000);
}

#[test]
fn tv_bw_min_interval_and_external_invalid() {
    let mut f = BtlBwFilter::new();
    f.update(
        RateSample {
            delivered: 2500,
            interval: dms(40),
            is_app_limited: false,
            gain_ge1: true,
            ack_elapsed: Duration::ZERO,
            sent_ns: 0,
        },
        0,
        true,
    );
    // interval 0.5ms < MIN_RATE_INTERVAL(1ms) → excluded.
    f.update(
        RateSample {
            delivered: 100,
            interval: Duration::from_micros(500),
            is_app_limited: false,
            gain_ge1: true,
            ack_elapsed: Duration::ZERO,
            sent_ns: 0,
        },
        ns(1),
        true,
    );
    assert_eq!(f.btlbw(), 62500);

    // valid=false (reverse_path_impaired / ack_delay_untrusted dış flag) → excluded.
    f.update(
        RateSample {
            delivered: 9999,
            interval: dms(40),
            is_app_limited: false,
            gain_ge1: true,
            ack_elapsed: Duration::ZERO,
            sent_ns: 0,
        },
        ns(2),
        false,
    );
    assert_eq!(f.btlbw(), 62500);
}

#[test]
fn tv_bw_credit_delivered_no_sample_but_future_delta() {
    // [TUR-2c D-1/3 kol-2] credit_delivered: örnek üretmez ama kümülatif delivered'a girer →
    // SONRAKİ gerçek örneğin delta'sına dürüst yansır (teslimat gerçekti).
    use adaptive_transport::bw::DeliveryRateSampler;
    let mut s = DeliveryRateSampler::new();
    let p0 = s.on_sent(0, 1000, false, true);
    let p1 = s.on_sent(ns(10), 1000, false, true);
    let r0 = s.on_ack(ns(20), p0);
    assert_eq!(r0.delivered, 1000);
    // kol-2 kredisi: 500B superseded-iletim geç-ACK'i — örnek yok, kümülatif +500.
    s.credit_delivered(ns(30), 500);
    // Sonraki gerçek örnek: delta = C.delivered(2500) − p1.snapshot(0) = r0'ın 1000'i +
    // kredi 500 + p1'in kendi 1000'i — kredi delta'da görünür (p1 gönderiminden beri teslim).
    let r1 = s.on_ack(ns(40), p1);
    assert_eq!(
        r1.delivered, 2500,
        "kredi sonraki örneğin delivered-delta'sında görünmeli"
    );
}

#[test]
fn tv_bw_credit_anchors_next_send_window() {
    // [TUR-2c review-vektörü] Kredinin delivered_time_ns=now güncellemesi: krediden SONRA
    // gönderilen paketin snapshot'ı kredi-zamanını taban alır → örneğinin ack_elapsed'i
    // kredi-anından ölçülür (eşleşmiş pencere, yansız — güncellenmeseydi sahte-uzun interval).
    use adaptive_transport::bw::DeliveryRateSampler;
    let mut s = DeliveryRateSampler::new();
    let p0 = s.on_sent(0, 1000, false, true);
    s.on_ack(ns(20), p0); // delivered=1000 @20ms
    s.credit_delivered(ns(30), 500); // kümülatif 1500 @30ms; in-flight 0'a döner
                                     // Krediden sonra gönderim: pipe-boş → first_sent/delivered_time yeniden çıpalanır (40ms).
    let p1 = s.on_sent(ns(40), 1000, false, true);
    let r1 = s.on_ack(ns(60), p1);
    assert_eq!(
        r1.delivered, 1000,
        "delta yalnız p1 (kredi ÖNCEKİ pencerede kaldı)"
    );
    // interval = max(ack_elapsed=60−40, send_elapsed=0) = 20ms — kredi-sonrası çıpa dürüst.
    assert_eq!(r1.interval, std::time::Duration::from_millis(20));
}

#[test]
fn tv_bw_keep_last_and_channel_window() {
    // [TUR-3a D-2/2] KEEP-LAST: valid=false güncellemede budama filtreyi BOŞALTAMAZ —
    // son örnek yaşar (eski davranış: btlbw→0 flap'i; sub-kbps 10s+ ACK-boşluğu sınıfı).
    use adaptive_transport::bw::{BtlBwFilter, RateSample};
    use std::time::Duration;
    let mut f = BtlBwFilter::new();
    let s = |d: u64, ms: u64| RateSample {
        delivered: d,
        interval: Duration::from_millis(ms),
        is_app_limited: false,
        gain_ge1: true,
        ack_elapsed: Duration::ZERO,
        sent_ns: 0,
    };
    assert_eq!(f.update(s(1000, 100), 0, true), 10_000); // 10kB/s @t=0
                                                         // t=15s (>10s pencere), geçersiz güncelleme: ESKİ davranış 0'a düşerdi; keep-last 10k tutar.
    assert_eq!(
        f.update(s(0, 0), ns(15_000), false),
        10_000,
        "keep-last: son örnek expire edilmez (btlbw=0 flap'i yapısal bitti)"
    );
    // [TUR-3a D-2/1] kanal-türevi pencere: 40s pencereyle 15s'lik örnek pencere-İÇİ kalır,
    // yeni düşük örnek max'ı DÜŞÜRMEZ (max-filter).
    let mut g = BtlBwFilter::new();
    g.set_window(Duration::from_secs(40));
    g.update(s(1000, 100), 0, true); // 10k
    assert_eq!(
        g.update(s(100, 100), ns(15_000), true),
        10_000,
        "40s pencerede t=0 örneği yaşar → max 10k"
    );
    // set_window tabanı: 1s istense de 10s [STD] altına inmez (t=5s'te örnek hâlâ pencere-içi).
    let mut h = BtlBwFilter::new();
    h.set_window(Duration::from_secs(1));
    h.update(s(1000, 100), 0, true);
    assert_eq!(
        h.update(s(100, 100), ns(5_000), true),
        10_000,
        "pencere tabanı 10s: küçültme yok"
    );
}
