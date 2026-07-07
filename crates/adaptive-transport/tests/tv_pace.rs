//! TV-PACE golden vektörü — token-bucket pacing (engine §13). Deterministik.

use adaptive_transport::pacer::{TokenBucket, DEFAULT_BURST_CAP};
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn tv_pace_token_bucket() {
    // G5: rate=62500 B/s, packet=1250, burst_cap=2400 (2×MTU), token dolu başlar.
    assert_eq!(DEFAULT_BURST_CAP, 2400);
    let mut tb = TokenBucket::new(62500, DEFAULT_BURST_CAP);
    assert_eq!(tb.tokens(), 2400);

    assert!(tb.try_send(1250)); // 2400 → 1150
    assert_eq!(tb.tokens(), 1150);

    assert!(!tb.try_send(1250)); // 1150 < 1250 → deny
    assert_eq!(tb.tokens(), 1150);

    tb.refill(ms(10)); // +625 (62500·0.01) → 1775
    assert_eq!(tb.tokens(), 1775);

    assert!(tb.try_send(1250)); // 1775 → 525
    assert_eq!(tb.tokens(), 525);

    tb.refill(ms(20)); // +1250 (62500·0.02) → min(1775, 2400) = 1775
    assert_eq!(tb.tokens(), 1775);
}

#[test]
fn tv_pace_burst_cap_clamps() {
    let mut tb = TokenBucket::new(62500, DEFAULT_BURST_CAP);
    tb.refill(ms(1000)); // büyük refill → BURST_CAP ile clamp
    assert_eq!(tb.tokens(), 2400);
}
