//! TV-SRC golden vektörleri — SRC çekirdek denklemleri + INV-RATE-1 (§8.2).
//!
//! Değerler PR açıklamasında adım adım türetildi. Probe cycle test EDİLMEZ (periyot
//! [CAL], netem); golden sabit-gain + INV-RATE-1'i doğrular.

use adaptive_transport::rate::{bdp_bytes, evaluate, Gain, PacingPhase, SrcConfig};
use std::time::Duration;

fn dms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn tv_src_bdp_and_cwnd() {
    // G1: BDP = BtlBw[B/s] × RTprop[s]; cwnd = max(2·BDP, MIN_CWND=4800=4×MTU).
    assert_eq!(bdp_bytes(62500, dms(100)), 6250); // 62500 × 0.1 s
    let cfg = SrcConfig::default();
    assert_eq!(cfg.min_cwnd, 4800);

    // normal: cwnd = 2·6250 = 12500 (MIN_CWND bağlamaz)
    let d = evaluate(&cfg, 62500, dms(100), dms(100), Gain::STEADY);
    assert_eq!((d.bdp, d.cwnd), (6250, 12500));

    // düşük BtlBw: MIN_CWND bağlar (max(2000, 4800) = 4800)
    assert_eq!(bdp_bytes(10000, dms(100)), 1000);
    let d2 = evaluate(&cfg, 10000, dms(100), dms(100), Gain::STEADY);
    assert_eq!((d2.bdp, d2.cwnd), (1000, 4800));

    // overflow-safety: u128 ara-tip (1e9 B/s × 1s = 1e9 bytes)
    assert_eq!(
        bdp_bytes(1_000_000_000, Duration::from_secs(1)),
        1_000_000_000
    );
}

#[test]
fn tv_src_pacing_gains() {
    // G2: [STD:BBR] gain'ler. srtt=RTprop=100ms → cwnd/srtt=125000 (cap bağlamaz).
    let cfg = SrcConfig::default();
    let steady = evaluate(&cfg, 62500, dms(100), dms(100), PacingPhase::Steady.gain());
    let up = evaluate(&cfg, 62500, dms(100), dms(100), PacingPhase::ProbeUp.gain());
    let down = evaluate(
        &cfg,
        62500,
        dms(100),
        dms(100),
        PacingPhase::ProbeDown.gain(),
    );

    assert_eq!(steady.pacing_rate, 62500); // ×1
    assert_eq!(up.pacing_rate, 78125); // ×5/4
    assert_eq!(down.pacing_rate, 46875); // ×3/4
    assert_eq!(
        (steady.send_rate, up.send_rate, down.send_rate),
        (62500, 78125, 46875)
    );
}

#[test]
fn tv_src_inv_rate_1a_loss_does_not_change_rate() {
    // G3 / INV-RATE-1 (a): loss send_rate'e GİRMEZ. loss evaluate()'in parametresi bile
    // DEĞİL; BtlBw (TV-BW: loss-invariant) ve srtt (loss≠delay) sabit → send_rate satır
    // satır sabit. loss_pre_fec 0.0 / 0.20 / 0.50 → aynı (BtlBw, srtt) → aynı send_rate.
    let cfg = SrcConfig::default();
    for _loss_pct in [0u32, 20, 50] {
        let d = evaluate(&cfg, 62500, dms(100), dms(100), Gain::STEADY);
        assert_eq!(d.send_rate, 62500);
    }
}

#[test]
fn tv_src_inv_rate_1b_queue_delay_reduces_rate() {
    // G4 / INV-RATE-1 (b): queue_delay artışı → send_rate DÜŞER (delay-based; cwnd/srtt).
    // BtlBw=62500, RTprop=100ms sabit → cwnd=12500. srtt = RTprop + queue_delay.
    let cfg = SrcConfig::default();
    let cases = [
        (dms(100), 62500u64), // queue 0   → cwnd/srtt=125000 → min=62500
        (dms(200), 62500),    // queue 100 → cwnd/srtt=62500  → min=62500 (sınır)
        (dms(400), 31250),    // queue 300 → cwnd/srtt=31250  → min=31250 ↓
        (dms(500), 25000),    // queue 400 → cwnd/srtt=25000  → min=25000 ↓
    ];
    for (srtt, want) in cases {
        let d = evaluate(&cfg, 62500, dms(100), srtt, Gain::STEADY);
        assert_eq!(d.send_rate, want, "srtt={srtt:?}");
    }
}
