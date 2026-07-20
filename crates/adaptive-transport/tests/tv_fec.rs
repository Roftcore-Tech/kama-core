//! TV-FEC golden vektörleri — §10.1 FEC_SHARD FRAMING codec.
//!
//! **RETIRED / DEPRECATED (D1 Sprint-3):** bu golden'lar legacy 0x20-cleartext-framing'i belgeler;
//! o yol noise-wire EXTENDED-seal'e (§2.6) MİGRE edildi (METADATA-LEAK-01). Production-FEC-wire golden'ı
//! artık `kama-noise-wire/tests/golden_wire.rs` (101B@E=64 EXTENDED-datagram). Bu test yalnız
//! deprecated-codec'in geriye-dönük-tutarlılığını korur (byte-değişmezliği); yeni-format DEĞİL.
#![allow(deprecated)] // deprecated legacy-codec bilinçli test ediliyor (retire-öncesi tutarlılık)

use adaptive_transport::error::TransportError;
use adaptive_transport::fec::{FecShardFrame, SHARD_TAG_LEN};

fn sample() -> FecShardFrame {
    // opaque_secured = ct(3B: AA BB CC) ‖ tag(16B: 0x11×16) = 19B (transport için opak blob)
    let mut opaque = vec![0xAA, 0xBB, 0xCC];
    opaque.extend_from_slice(&[0x11; SHARD_TAG_LEN]);
    FecShardFrame {
        block_id: 1,
        shard_index: 2,
        k: 8,
        r: 4,
        shard_len: 3,
        opaque_secured: opaque,
    }
}

#[test]
fn tv_fec_encode_golden() {
    // 10B header + 19B opaque = 29B. block_id u32_be, shard_len u16_be.
    let mut want = vec![0x20, 0x00, 0x00, 0x00, 0x01, 0x02, 0x08, 0x04, 0x00, 0x03];
    want.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
    want.extend_from_slice(&[0x11; 16]);
    assert_eq!(sample().encode(), want);
    assert_eq!(sample().encode().len(), 29);
}

#[test]
fn tv_fec_roundtrip() {
    let f = sample();
    let g = FecShardFrame::decode(&f.encode()).expect("decode ok");
    assert_eq!(f, g);
    // opaque TEK blob: shard_len + tag.
    assert_eq!(
        g.opaque_secured.len(),
        usize::from(g.shard_len) + SHARD_TAG_LEN
    );
}

#[test]
fn tv_fec_decode_rejects_truncated() {
    let bytes = sample().encode();
    match FecShardFrame::decode(&bytes[..bytes.len() - 1]) {
        Err(TransportError::Truncated { .. }) => {}
        other => panic!("Truncated bekleniyordu: {other:?}"),
    }
}

#[test]
fn tv_fec_decode_rejects_trailing() {
    let mut bytes = sample().encode();
    bytes.push(0xEE);
    match FecShardFrame::decode(&bytes) {
        Err(TransportError::TrailingBytes { count: 1 }) => {}
        other => panic!("TrailingBytes bekleniyordu: {other:?}"),
    }
}

#[test]
fn tv_fec_decode_rejects_bad_frame_type() {
    let mut bytes = sample().encode();
    bytes[0] = 0x10; // ACK, FEC_SHARD değil
    match FecShardFrame::decode(&bytes) {
        Err(TransportError::UnknownFrameType(0x10)) => {}
        other => panic!("UnknownFrameType bekleniyordu: {other:?}"),
    }
}

#[test]
fn tv_fec_decode_rejects_shard_count_overflow() {
    // k+r > 255 (GF(2^8)) — metadata invalid, opaque OKUNMADAN reddedilir.
    let f = FecShardFrame {
        block_id: 1,
        shard_index: 0,
        k: 200,
        r: 100,
        shard_len: 0,
        opaque_secured: vec![0x11; SHARD_TAG_LEN],
    };
    match FecShardFrame::decode(&f.encode()) {
        Err(TransportError::InvalidShardCount { k: 200, r: 100 }) => {}
        other => panic!("InvalidShardCount bekleniyordu: {other:?}"),
    }
}
