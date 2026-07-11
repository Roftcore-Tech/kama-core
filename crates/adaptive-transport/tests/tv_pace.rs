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

#[test]
fn tv_pace_r1_sub_byte_precision_no_truncation() {
    // [R1/ADIM-6] floor rate 31 B/s @ 1ms tick = 0.031 token/tick. Eski tamsayı-truncation her tick 0
    // üretip pacer'ı KİLİTLİYORDU (PreConverge stall'ının kök nedeni). Artık kesir birikir.
    let mut tb = TokenBucket::new(31, DEFAULT_BURST_CAP);
    assert!(tb.try_send(2400)); // bucket'ı boşalt → tokens 0
    assert_eq!(tb.tokens(), 0);
    // 32 tick: 32×0.031=0.992 < 1 → hâlâ 0 (kalan birikiyor, atılmıyor).
    for _ in 0..32 {
        tb.refill(ms(1));
    }
    assert_eq!(
        tb.tokens(),
        0,
        "32 tick < 1 token ama kalan taşınıyor (eski kod: sonsuza dek 0)"
    );
    // 33. tick: kümülatif 1.023 → 1 tam token ÜRETİLİR (truncation-yok kanıtı).
    tb.refill(ms(1));
    assert_eq!(
        tb.tokens(),
        1,
        "[R1] 33. tick'te 1 token — truncation olsaydı hep 0 kalırdı"
    );
    // 1s toplam (33 zaten geçti) → ~31 token (31 B/s × 1s), truncation-kaybı yok.
    for _ in 0..967 {
        tb.refill(ms(1));
    }
    assert_eq!(
        tb.tokens(),
        31,
        "[R1] 1s @ 31 B/s = 31 token (kesir kaybı yok)"
    );
}

#[test]
fn tv_pace_r3_channel_derived_burst() {
    // [R3/ADIM-6] set_burst_cap kanal-türevli cap + token'ı cap'e clamp (idle-dump önle); fill_to_cap grant.
    let mut tb = TokenBucket::new(62500, DEFAULT_BURST_CAP);
    assert_eq!(tb.tokens(), 2400);
    tb.set_burst_cap(100); // sub-kbps: 1 birim → token 2400'den 100'e clamp (bant-kör dump YOK)
    assert_eq!(tb.tokens(), 100);
    tb.set_burst_cap(500); // cap yükselir ama token YALNIZ azalır (yukarı doldurmaz)
    assert_eq!(tb.tokens(), 100);
    tb.fill_to_cap(); // cold-start çıkış grant'ı → token = kanal-türevli cap
    assert_eq!(tb.tokens(), 500);
    tb.set_burst_cap(0); // cap≥1 (deadlock önle)
    assert_eq!(tb.tokens(), 1);
}
