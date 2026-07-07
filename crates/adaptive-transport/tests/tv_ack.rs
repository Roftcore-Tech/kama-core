//! TV-ACK golden vektörleri — rx (full-counter) → ACK_FRAME byte'ları (engine spec §2).

use adaptive_transport::ack::{AckFrame, AckRange};
use adaptive_transport::error::TransportError;

#[test]
fn tv_ack_encode_golden() {
    // contiguous {5,6,7} → tek blok, range yok
    let f = AckFrame::from_received(&[5, 6, 7], 0, 0).unwrap();
    #[rustfmt::skip]
    let want = vec![0x10, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x03, 0x00];
    assert_eq!(f.encode(), want);

    // gaps {7,5,3} → block0={7}, range'ler (gap=2,len=1)(gap=1,len=1)
    let f = AckFrame::from_received(&[7, 5, 3], 0, 0).unwrap();
    #[rustfmt::skip]
    let want = vec![
        0x10, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0x02,
        0x00, 0x02, 0x00, 0x01,
        0x00, 0x01, 0x00, 0x01,
    ];
    assert_eq!(f.encode(), want);

    // u16 WRAP: full counter'lar 65535,65536,65537 → wire {0xFFFF,0x0000,0x0001}
    let f = AckFrame::from_received(&[65535, 65536, 65537], 0, 0).unwrap();
    assert_eq!(f.largest_seq, 0x0001);
    assert_eq!(f.block0_len, 3);
    assert!(f.ranges.is_empty());
    #[rustfmt::skip]
    let want = vec![0x10, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00];
    assert_eq!(f.encode(), want);
}

#[test]
fn tv_ack_structure() {
    let f = AckFrame::from_received(&[7, 5, 3], 0, 0).unwrap();
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
        let f = AckFrame::from_received(&full, 2, 1234).unwrap();
        let bytes = f.encode();
        let g = AckFrame::decode(&bytes).expect("decode ok");
        assert_eq!(f, g, "roundtrip full={full:?}");
    }
}

#[test]
fn tv_ack_empty_input_is_none() {
    assert!(AckFrame::from_received(&[], 0, 0).is_none());
}

#[test]
fn tv_ack_decode_rejects_truncated() {
    let bytes = AckFrame::from_received(&[7, 5, 3], 0, 0).unwrap().encode();
    match AckFrame::decode(&bytes[..bytes.len() - 1]) {
        Err(TransportError::Truncated { .. }) => {}
        other => panic!("truncated bekleniyordu, geldi: {other:?}"),
    }
}

#[test]
fn tv_ack_decode_rejects_trailing() {
    let mut bytes = AckFrame::from_received(&[5, 6, 7], 0, 0).unwrap().encode();
    bytes.push(0xAB);
    match AckFrame::decode(&bytes) {
        Err(TransportError::TrailingBytes { count: 1 }) => {}
        other => panic!("trailing bekleniyordu, geldi: {other:?}"),
    }
}

#[test]
fn tv_ack_decode_rejects_bad_frame_type() {
    let mut bytes = AckFrame::from_received(&[5, 6, 7], 0, 0).unwrap().encode();
    bytes[0] = 0x20; // FEC_SHARD_HEADER, ACK değil
    match AckFrame::decode(&bytes) {
        Err(TransportError::UnknownFrameType(0x20)) => {}
        other => panic!("UnknownFrameType bekleniyordu, geldi: {other:?}"),
    }
}
