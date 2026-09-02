//! Sprint-2 control-codec testleri (§2.57 + DR-1):
//! her-tip seal→open roundtrip · body-golden · DR-1 tavan (gönderme-böl / gelen-reject) ·
//! invalid-range/trailing/kısa-body fail-closed · control_id-dedup + tombstone · retry-sınıfı ·
//! RECOVERED-no-bandwidth-credit.

use kama_noise_wire::control::{
    control_body_len, decode_control, decode_control_body, encode_control_body,
};
use kama_noise_wire::retry::{
    control_produces_bandwidth_credit, InboundControlDedup, InboundOutcome, RetryClass,
};
use kama_noise_wire::{
    split_for_send, InnerPlain, KamaNoiseWireProtector, KamaNoiseWireSizer, SealArgs, WireError,
    ACK_MAX_RANGES, RECOVERED_MAX_ESI_RANGES, SACK_MAX_GAP_BLOCKS,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind;
use kama_transport_schema::UnsealedTransportPacket;
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}
fn paired() -> (KamaNoiseWireProtector, KamaNoiseWireProtector) {
    (
        KamaNoiseWireProtector::new(0x77, &key(0xAA), &key(0xBB)),
        KamaNoiseWireProtector::new(0x77, &key(0xBB), &key(0xAA)),
    )
}

fn all_controls() -> Vec<Control> {
    vec![
        Control::Ack {
            id: 1,
            acked_dh_epoch: 1,
            ack_delay_256us: 0,
            seen_count: 0,
            arrival_rate: 0xFFFF,
            ranges: vec![(10, 20), (30, 40)],
        },
        Control::Sack {
            id: 2,
            acked_dh_epoch: 0,
            base_pn: 100,
            blocks: vec![(3, 5), (2, 7)],
        },
        Control::Recovered {
            id: 3,
            block_id: 9,
            esi_ranges: vec![(0, 3), (5, 5)],
        },
        Control::BlockComplete {
            id: 4,
            block_id: 9,
            final_source_count: 8,
        },
        Control::ParamSet {
            id: 5,
            param_hash: [0x5A; 32],
            profile_id: 3,
            code_suite_id: 1,
        },
        Control::ParamAck {
            id: 6,
            param_hash: [0x5A; 32],
        },
        Control::BlockAbort {
            id: 7,
            block_id: 9,
            reason_code: 2,
        },
        Control::SessionClose {
            id: 8,
            reason_code: 1,
        },
        Control::SessionRenewRequired {
            id: 9,
            reason_code: 4,
            renew_hint: 1,
        },
    ]
}

// ── Her control-tipi: body encode→decode roundtrip + seal→open roundtrip ──
#[test]
fn all_controls_body_roundtrip() {
    for c in all_controls() {
        let body = encode_control_body(&c);
        let back = decode_control_body(c.codepoint(), c.control_id(), &body).unwrap();
        assert_eq!(back, c, "body roundtrip: {:#04x}", c.codepoint());
        // sealed_len == 26 + 6 + body
        assert_eq!(control_body_len(&c).unwrap(), body.len());
    }
}

#[test]
fn all_controls_seal_open_roundtrip() {
    let (mut a, mut b) = paired();
    for (i, c) in all_controls().into_iter().enumerate() {
        let pkt = UnsealedTransportPacket::Control(c.clone());
        let want = a.sealed_len(&pkt).unwrap();
        let sealed = a
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: i as u64,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(
            sealed.bytes.len(),
            want,
            "seal.len==sealed_len (control {:#04x})",
            c.codepoint()
        );
        let opened = b.open(&sealed.bytes).unwrap();
        let decoded = decode_control(opened.inner()).unwrap().unwrap();
        assert_eq!(decoded, c);
    }
}

// ── §2.57 body golden (ACK, PARAM_SET, RECOVERED byte-exact) ──
#[test]
fn control_body_golden() {
    // ACK: acked_dh_epoch=1 · count=1 · [pn_start=0x0A, pn_end=0x14]
    let ack = Control::Ack {
        id: 0,
        acked_dh_epoch: 1,
        ack_delay_256us: 0,
        seen_count: 0,
        arrival_rate: 0xFFFF,
        ranges: vec![(0x0A, 0x14)],
    };
    // v1.0 Ack-genişletmesi: epoch(1) + delay u16 + seen u32 + rate u16 + count u16.
    let mut expect = vec![
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x01,
    ];
    expect.extend_from_slice(&0x0Au64.to_be_bytes());
    expect.extend_from_slice(&0x14u64.to_be_bytes());
    assert_eq!(encode_control_body(&ack), expect);

    // SESSION_RENEW_REQUIRED: reason=4, hint=1
    let renew = Control::SessionRenewRequired {
        id: 0,
        reason_code: 4,
        renew_hint: 1,
    };
    assert_eq!(encode_control_body(&renew), vec![0x04, 0x01]);

    // PARAM_ACK: 32B hash echo
    let pa = Control::ParamAck {
        id: 0,
        param_hash: [0xAB; 32],
    };
    assert_eq!(encode_control_body(&pa), vec![0xAB; 32]);
}

// ── DR-1: GELEN count > tavan → FAIL-CLOSED reject ──
#[test]
fn incoming_ack_over_cap_rejected() {
    // Elle 33-range'lik ACK-body kur (tavan 32).
    let n = ACK_MAX_RANGES + 1;
    let mut body = vec![0u8]; // acked_dh_epoch
    body.extend_from_slice(&0u16.to_be_bytes()); // ack_delay_256us
    body.extend_from_slice(&0u32.to_be_bytes()); // seen_count
    body.extend_from_slice(&0xFFFFu16.to_be_bytes()); // arrival_rate
    body.extend_from_slice(&(n as u16).to_be_bytes());
    for i in 0..n {
        body.extend_from_slice(&(i as u64).to_be_bytes());
        body.extend_from_slice(&(i as u64).to_be_bytes());
    }
    assert!(matches!(
        decode_control_body(inner_kind::ACK, 0, &body),
        Err(WireError::ControlCountExceedsCap { kind: inner_kind::ACK, count, cap: ACK_MAX_RANGES }) if count == n
    ));
}

#[test]
fn incoming_recovered_over_cap_rejected() {
    let n = RECOVERED_MAX_ESI_RANGES + 1;
    let mut body = Vec::new();
    body.extend_from_slice(&9u32.to_be_bytes());
    body.extend_from_slice(&(n as u16).to_be_bytes());
    for i in 0..n {
        body.extend_from_slice(&(i as u16).to_be_bytes());
        body.extend_from_slice(&(i as u16).to_be_bytes());
    }
    assert!(matches!(
        decode_control_body(inner_kind::RECOVERED, 0, &body),
        Err(WireError::ControlCountExceedsCap {
            cap: RECOVERED_MAX_ESI_RANGES,
            ..
        })
    ));
}

// ── DR-1: GÖNDERME tavan-aşımı → BÖL (çok-control, her-biri-kendi-control_id) ──
#[test]
fn outgoing_ack_over_cap_split() {
    let ranges: Vec<(u64, u64)> = (0..(ACK_MAX_RANGES as u64 * 2 + 5))
        .map(|i| (i, i))
        .collect();
    let big = Control::Ack {
        id: 100,
        acked_dh_epoch: 0,
        ack_delay_256us: 0,
        seen_count: 0,
        arrival_rate: 0xFFFF,
        ranges: ranges.clone(),
    };
    let mut next = 1000u32;
    let parts = split_for_send(&big, || {
        next += 1;
        next
    });
    // 65 range / 32 = 3 parça
    assert_eq!(parts.len(), 3);
    // İlk parça orijinal-id, sonrakiler yeni-id
    assert_eq!(parts[0].control_id(), 100);
    assert_ne!(parts[1].control_id(), 100);
    assert_ne!(parts[2].control_id(), parts[1].control_id());
    // Her parça tavan-içinde + toplam-range korunur
    let mut total = 0;
    for p in &parts {
        let len = control_body_len(p).unwrap();
        assert!(
            len <= control_body_len(&Control::Ack {
                id: 0,
                acked_dh_epoch: 0,
                ack_delay_256us: 0,
                seen_count: 0,
                arrival_rate: 0xFFFF,
                ranges: vec![(0, 0); ACK_MAX_RANGES]
            })
            .unwrap()
        );
        if let Control::Ack { ranges, .. } = p {
            total += ranges.len();
            assert!(ranges.len() <= ACK_MAX_RANGES);
        }
    }
    assert_eq!(total, ranges.len());
}

#[test]
fn sack_split_and_within_cap_noop() {
    // Tavan-içi SACK → tek-parça, aynı-id.
    let small = Control::Sack {
        id: 5,
        acked_dh_epoch: 0,
        base_pn: 0,
        blocks: vec![(1, 1)],
    };
    let parts = split_for_send(&small, || 999);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].control_id(), 5);
    // Tavan-aşan SACK → böl
    let blocks: Vec<(u16, u16)> = (0..(SACK_MAX_GAP_BLOCKS as u16 + 1))
        .map(|i| (i, i))
        .collect();
    let big = Control::Sack {
        id: 5,
        acked_dh_epoch: 0,
        base_pn: 0,
        blocks,
    };
    assert_eq!(split_for_send(&big, || 999).len(), 2);
}

// ── fail-closed: invalid-range / trailing / kısa-body ──
#[test]
fn invalid_range_rejected() {
    // ACK pn_start > pn_end
    let mut body = vec![0u8, 0x00, 0x01];
    body.extend_from_slice(&20u64.to_be_bytes());
    body.extend_from_slice(&10u64.to_be_bytes());
    assert!(matches!(
        decode_control_body(inner_kind::ACK, 0, &body),
        Err(WireError::ControlDecode(_))
    ));
}

#[test]
fn trailing_bytes_rejected() {
    // SESSION_CLOSE 1B bekler; 2B verilirse trailing → reject
    assert!(matches!(
        decode_control_body(inner_kind::SESSION_CLOSE, 0, &[1, 2]),
        Err(WireError::ControlDecode("trailing bytes"))
    ));
}

#[test]
fn short_body_rejected() {
    // PARAM_ACK 32B bekler; 10B → too short
    assert!(matches!(
        decode_control_body(inner_kind::PARAM_ACK, 0, &[0u8; 10]),
        Err(WireError::ControlDecode(_))
    ));
}

// ── control_id idempotent-dedup + SESSION_CLOSE tombstone ──
#[test]
fn dedup_idempotent() {
    let mut d = InboundControlDedup::new(16);
    let c = Control::BlockComplete {
        id: 42,
        block_id: 1,
        final_source_count: 4,
    };
    assert_eq!(d.admit(&c), InboundOutcome::Fresh);
    assert_eq!(d.admit(&c), InboundOutcome::DuplicateAckOnly);
    assert_eq!(d.admit(&c), InboundOutcome::DuplicateAckOnly);
    assert_eq!(d.seen_count(), 1);
}

#[test]
fn session_close_tombstone() {
    let mut d = InboundControlDedup::new(16);
    let close = Control::SessionClose {
        id: 7,
        reason_code: 0,
    };
    d.admit(&close);
    assert!(
        d.has_close_tombstone(7),
        "CLOSE-retry'larına ACK için tombstone tutulmalı"
    );
    // tekrar-CLOSE → duplicate ama tombstone korunur
    assert_eq!(d.admit(&close), InboundOutcome::DuplicateAckOnly);
    assert!(d.has_close_tombstone(7));
}

// ── §2.57 retry-sınıfı tablosu ──
#[test]
fn retry_class_table() {
    assert_eq!(
        RetryClass::of(inner_kind::PARAM_SET),
        RetryClass::UntilParamAck
    );
    assert_eq!(
        RetryClass::of(inner_kind::BLOCK_ABORT),
        RetryClass::UntilNetworkAck
    );
    assert_eq!(
        RetryClass::of(inner_kind::BLOCK_COMPLETE),
        RetryClass::UntilNetworkAck
    );
    assert_eq!(
        RetryClass::of(inner_kind::SESSION_CLOSE),
        RetryClass::UntilNetworkAck
    );
    assert_eq!(
        RetryClass::of(inner_kind::SESSION_RENEW_REQUIRED),
        RetryClass::UntilNetworkAck
    );
    assert_eq!(RetryClass::of(inner_kind::ACK), RetryClass::NoRetry);
    assert_eq!(RetryClass::of(inner_kind::SACK), RetryClass::NoRetry);
    assert_eq!(RetryClass::of(inner_kind::RECOVERED), RetryClass::NoRetry);
    assert_eq!(RetryClass::of(inner_kind::PARAM_ACK), RetryClass::NoRetry);
}

// ── N8-invariant: hiçbir control (özellikle RECOVERED) bandwidth-credit üretMEZ ──
#[test]
fn no_control_produces_bandwidth_credit() {
    for c in all_controls() {
        assert!(
            !control_produces_bandwidth_credit(&c),
            "control {:#04x} credit üretmemeli",
            c.codepoint()
        );
    }
    // RECOVERED yapısal olarak PN/credit alanı taşımaz — yalnız logical (block_id, esi_ranges).
    let rec = Control::Recovered {
        id: 1,
        block_id: 5,
        esi_ranges: vec![(0, 2)],
    };
    match rec {
        Control::Recovered {
            block_id,
            esi_ranges,
            ..
        } => {
            assert_eq!(block_id, 5);
            assert_eq!(esi_ranges, vec![(0, 2)]);
        }
        _ => unreachable!(),
    }
}

// ── decode_control: control-inner → Some, FEC-inner → None (tip-güvenliği) ──
#[test]
fn decode_control_type_discrimination() {
    let ctl = InnerPlain::Control {
        inner_kind: inner_kind::ACK,
        control_id: 1,
        body: vec![0, 0, 0],
    };
    assert!(decode_control(&ctl).is_some());

    let fec = InnerPlain::FecShard {
        header: kama_transport_schema::LogicalShardHeader {
            kind: kama_transport_schema::inner_kind::ShardKind::Source,
            profile_id: 3,
            block_id: 1,
            esi: 0,
            k_actual: 1,
        },
        symbol: kama_noise_wire::SensitiveSymbolBytes::from(vec![0u8; 64]),
    };
    assert!(
        decode_control(&fec).is_none(),
        "FEC-shard control-decode → None"
    );
}
