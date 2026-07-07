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
    let p0 = s.on_sent(ns(0), 1250, false);
    let p1 = s.on_sent(ns(0), 1250, false);
    let r0 = s.on_ack(ns(40), p0);
    let p2 = s.on_sent(ns(40), 1250, false);
    let r1 = s.on_ack(ns(60), p1);
    let p3 = s.on_sent(ns(60), 1250, false);
    let r2 = s.on_ack(ns(80), p2);
    let p4 = s.on_sent(ns(80), 1250, false);
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
    };
    let s62500 = RateSample {
        delivered: 2500,
        interval: dms(40),
        is_app_limited: false,
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
        },
        ns(2),
        false,
    );
    assert_eq!(f.btlbw(), 62500);
}
