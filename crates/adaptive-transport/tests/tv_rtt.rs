//! TV-RTT golden vektörleri — rtt_sample dizisi → srtt/rttvar/RTprop/queue_delay (§6).
//!
//! Değerler PR açıklamasında adım adım türetildi (RFC 6298 EWMA; ns'de exact).

use adaptive_transport::rtt::{apply_ack_delay, rtt_sample, RttEstimator};
use std::time::Duration;

fn dms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn tv_rtt_golden() {
    // Girdi [100, 100, 140, 120] ms; now_ns'ler (0,50,100,150 ms) W_rtprop(10s) içinde
    // → RTprop = min(tüm sample). Kural: ilk sample srtt=s, rttvar=s/2; sonra RFC 6298
    // (rttvar ÖNCE eski srtt ile, srtt SONRA); queue_delay = srtt − RTprop.
    let mut e = RttEstimator::new();

    e.observe(0, dms(100));
    assert_eq!(e.srtt(), Some(dms(100)));
    assert_eq!(e.rttvar(), dms(50));
    assert_eq!(e.rtprop(), Some(dms(100)));
    assert_eq!(e.queue_delay(), Duration::ZERO);

    e.observe(50_000_000, dms(100));
    assert_eq!(e.srtt(), Some(dms(100)));
    assert_eq!(e.rttvar(), Duration::from_nanos(37_500_000)); // 37.5 ms
    assert_eq!(e.rtprop(), Some(dms(100)));
    assert_eq!(e.queue_delay(), Duration::ZERO);

    e.observe(100_000_000, dms(140));
    assert_eq!(e.srtt(), Some(dms(105)));
    assert_eq!(e.rttvar(), Duration::from_nanos(38_125_000)); // 38.125 ms
    assert_eq!(e.rtprop(), Some(dms(100)));
    assert_eq!(e.queue_delay(), dms(5));

    e.observe(150_000_000, dms(120));
    assert_eq!(e.srtt(), Some(Duration::from_nanos(106_875_000))); // 106.875 ms
    assert_eq!(e.rttvar(), Duration::from_nanos(32_343_750)); // 32.34375 ms
    assert_eq!(e.rtprop(), Some(dms(100)));
    assert_eq!(e.queue_delay(), Duration::from_nanos(6_875_000)); // 6.875 ms
}

#[test]
fn tv_rtt_sample_ack_delay_untrusted() {
    assert_eq!(rtt_sample(dms(40), dms(10)), Some(dms(30)));
    assert_eq!(rtt_sample(dms(40), dms(50)), None); // ack_delay > raw → untrusted (§7.3)
}

#[test]
fn tv_rtt_apply_ack_delay_min_rtt_guard() {
    // [TUR-1x] RFC 9002 §5.3 semantiği: düzeltme yalnız sonuç gözlenen-min'in altına inmiyorsa.
    // Normal düzeltme: 40−10=30 ≥ min(20) → uygulanır.
    assert_eq!(
        apply_ack_delay(dms(40), dms(10), Some(dms(20))),
        (dms(30), true)
    );
    // min_rtt yok (ilk örnek) → düzeltme uygulanır.
    assert_eq!(apply_ack_delay(dms(40), dms(10), None), (dms(30), true));
    // Düzeltme min-rtt ALTINA iter (40−25=15 < 20) → RED, RAW işlenir (örnek atlanmaz).
    assert_eq!(
        apply_ack_delay(dms(40), dms(25), Some(dms(20))),
        (dms(40), false)
    );
    // Underflow (delay > raw) → RED, RAW işlenir.
    assert_eq!(
        apply_ack_delay(dms(40), dms(50), Some(dms(20))),
        (dms(40), false)
    );
    // Sub-ms gerçek RTT (loopback sınıfı): mutlak-taban YOK → düzeltme geçerli (ZM-2).
    let us = Duration::from_micros(200);
    assert_eq!(apply_ack_delay(us, Duration::ZERO, None), (us, true));
}
