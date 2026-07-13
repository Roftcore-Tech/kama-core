//! TV-ACK golden vektörleri — rx (full-counter) → ACK_FRAME byte'ları (engine spec §2).
//!
//! [TUR-1x] Golden'lar V2 (0x14, §2.2 15B gövde) emisyonuna güncellendi: frame_type + ack_delay
//! birimi (256µs) + ölçüm eki (seen_count u32, arrival_rate u16). V1 (0x10, 9B, µs) yalnız
//! DECODE-uyumluluk — ayrı vektörle korunur.

use adaptive_transport::ack::{AckFrame, AckRange, ARRIVAL_RATE_SENTINEL};
use adaptive_transport::error::TransportError;

#[test]
fn tv_ack_encode_golden() {
    // contiguous {5,6,7} → tek blok, range yok; seen=3, arrival_rate=sentinel.
    let f = AckFrame::from_received(&[5, 6, 7], 0, 0, 3, ARRIVAL_RATE_SENTINEL).unwrap();
    #[rustfmt::skip]
    let want = vec![
        0x14, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x03, 0xFF, 0xFF,
    ];
    assert_eq!(f.encode(), want);

    // gaps {7,5,3} → block0={7}, range'ler (gap=2,len=1)(gap=1,len=1)
    let f = AckFrame::from_received(&[7, 5, 3], 0, 0, 3, ARRIVAL_RATE_SENTINEL).unwrap();
    #[rustfmt::skip]
    let want = vec![
        0x14, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0x02,
        0x00, 0x02, 0x00, 0x01,
        0x00, 0x01, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x03, 0xFF, 0xFF,
    ];
    assert_eq!(f.encode(), want);

    // u16 WRAP: full counter'lar 65535,65536,65537 → wire {0xFFFF,0x0000,0x0001}
    let f =
        AckFrame::from_received(&[65535, 65536, 65537], 0, 0, 3, ARRIVAL_RATE_SENTINEL).unwrap();
    assert_eq!(f.largest_seq, 0x0001);
    assert_eq!(f.block0_len, 3);
    assert!(f.ranges.is_empty());
    #[rustfmt::skip]
    let want = vec![
        0x14, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x03, 0xFF, 0xFF,
    ];
    assert_eq!(f.encode(), want);
}

#[test]
fn tv_ack_decode_v1_compat_golden() {
    // [TUR-1x] V1 (0x10, 9B gövde) byte'ları decode-UYUMLU: ack_delay µs-birimle çözülür,
    // ölçüm eki alanları nötr (seen=0, arrival=sentinel).
    #[rustfmt::skip]
    let v1 = vec![0x10, 0x00, 0x00, 0x07, 0x00, 0x64, 0x00, 0x03, 0x00];
    let f = AckFrame::decode(&v1).expect("V1 decode");
    assert!(!f.v2);
    assert_eq!(f.ack_delay, 100);
    assert_eq!(
        f.ack_delay_duration(),
        std::time::Duration::from_micros(100)
    );
    assert_eq!(f.seen_count, 0);
    assert_eq!(f.arrival_rate, ARRIVAL_RATE_SENTINEL);
    // Aynı baş V2 başlığı + ölçüm ekiyle → 256µs semantiği.
    #[rustfmt::skip]
    let v2 = vec![
        0x14, 0x00, 0x00, 0x07, 0x00, 0x64, 0x00, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x2A, 0x01, 0xF4,
    ];
    let g = AckFrame::decode(&v2).expect("V2 decode");
    assert!(g.v2);
    assert_eq!(
        g.ack_delay_duration(),
        std::time::Duration::from_micros(100 * 256)
    );
    assert_eq!(g.seen_count, 42);
    assert_eq!(g.arrival_rate, 500);
}

#[test]
fn tv_ack_structure() {
    let f = AckFrame::from_received(&[7, 5, 3], 0, 0, 3, ARRIVAL_RATE_SENTINEL).unwrap();
    assert_eq!(f.largest_seq, 7);
    assert_eq!(f.block0_len, 1);
    assert_eq!(
        f.ranges,
        vec![AckRange { gap: 2, len: 1 }, AckRange { gap: 1, len: 1 },]
    );
}

#[test]
fn tv_ack_roundtrip() {
    for full in [
        vec![5u64, 6, 7],
        vec![7, 5, 3],
        vec![65535, 65536, 65537],
        vec![1, 2, 3, 10, 11, 20],
    ] {
        let f = AckFrame::from_received(&full, 2, 1234, 42, 500).unwrap();
        let bytes = f.encode();
        let g = AckFrame::decode(&bytes).expect("decode ok");
        assert_eq!(f, g, "roundtrip full={full:?}");
        assert_eq!(g.seen_count, 42, "ölçüm eki roundtrip");
        assert_eq!(g.arrival_rate, 500, "ölçüm eki roundtrip");
    }
}

#[test]
fn tv_ack_empty_input_is_none() {
    assert!(AckFrame::from_received(&[], 0, 0, 0, ARRIVAL_RATE_SENTINEL).is_none());
}

#[test]
fn tv_ack_decode_rejects_truncated() {
    let bytes = AckFrame::from_received(&[7, 5, 3], 0, 0, 3, ARRIVAL_RATE_SENTINEL)
        .unwrap()
        .encode();
    match AckFrame::decode(&bytes[..bytes.len() - 1]) {
        Err(TransportError::Truncated { .. }) => {}
        other => panic!("truncated bekleniyordu, geldi: {other:?}"),
    }
}

#[test]
fn tv_ack_decode_rejects_trailing() {
    let mut bytes = AckFrame::from_received(&[5, 6, 7], 0, 0, 3, ARRIVAL_RATE_SENTINEL)
        .unwrap()
        .encode();
    bytes.push(0xAB);
    match AckFrame::decode(&bytes) {
        Err(TransportError::TrailingBytes { count: 1 }) => {}
        other => panic!("trailing bekleniyordu, geldi: {other:?}"),
    }
}

#[test]
fn tv_ack_decode_rejects_bad_frame_type() {
    let mut bytes = AckFrame::from_received(&[5, 6, 7], 0, 0, 3, ARRIVAL_RATE_SENTINEL)
        .unwrap()
        .encode();
    bytes[0] = 0x20; // FEC_SHARD_HEADER, ACK değil
    match AckFrame::decode(&bytes) {
        Err(TransportError::UnknownFrameType(0x20)) => {}
        other => panic!("UnknownFrameType bekleniyordu, geldi: {other:?}"),
    }
}
