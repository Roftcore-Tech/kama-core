//! Sprint-2 control-codec testleri (§2.57 + DR-1):
//! her-tip seal→open roundtrip · body-golden · DR-1 tavan (gönderme-böl / gelen-reject) ·
//! invalid-range/trailing/kısa-body fail-closed · control_id-dedup + tombstone · retry-sınıfı ·
//! RECOVERED-no-bandwidth-credit · [D2 2026-10-08] 0x10/0x11 (eski Ack/Sack) RESERVED → reject.

use kama_noise_wire::control::{
    control_body_len, decode_control, decode_control_body, encode_control_body,
};
use kama_noise_wire::retry::{
    control_produces_bandwidth_credit, InboundControlDedup, InboundOutcome, RetryClass,
};
use kama_noise_wire::{
    split_for_send, InnerPlain, KamaNoiseWireProtector, KamaNoiseWireSizer, SealArgs, WireError,
    RECOVERED_MAX_ESI_RANGES,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind;
use kama_transport_schema::{SchemaError, UnsealedTransportPacket};
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
    // [D2 2026-10-08] Ack (id 1) / Sack (id 2) kaldırıldı — aile kodda yok (K3: ürün ACK'i engine
    // `ACK_V2`); 0x10/0x11 RESERVED → reject (bkz. `reserved_ack_sack_codepoints_rejected`).
    vec![
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

// ── [D2 2026-10-08] RESERVED → reject: eski Ack/Sack codepoint'leri (0x10/0x11) ──
// Aile kodda yok (K3 2026-10-04: ürün ACK'i engine `ACK_V2`). Hiçbir varyant bu codepoint'leri
// üretmez; gelen 0x10/0x11 schema sınıflandırmasında ve body-decoder'da gövdeden bağımsız düşer.
#[test]
fn reserved_ack_sack_codepoints_rejected() {
    assert_eq!(inner_kind::RESERVED_ACK, 0x10);
    assert_eq!(inner_kind::RESERVED_SACK, 0x11);
    let bodies: [&[u8]; 2] = [&[], &[0u8; 64]];
    for cp in [inner_kind::RESERVED_ACK, inner_kind::RESERVED_SACK] {
        assert!(
            all_controls().iter().all(|c| c.codepoint() != cp),
            "cp={cp:#04x} bir varyant tarafından üretiliyor"
        );
        assert_eq!(
            inner_kind::classify(cp),
            Err(SchemaError::UnknownInnerKind(cp))
        );
        for body in bodies {
            assert_eq!(
                decode_control_body(cp, 1, body),
                Err(WireError::ControlDecode("non-control codepoint")),
                "cp={cp:#04x} body={}B",
                body.len()
            );
        }
        // parse_inner baypas edilip elle kurulmuş bir control-inner da decode'da reddedilir.
        let ctl = InnerPlain::Control {
            inner_kind: cp,
            control_id: 1,
            body: vec![0u8; 8],
        };
        assert_eq!(
            decode_control(&ctl),
            Some(Err(WireError::ControlDecode("non-control codepoint")))
        );
    }
}

// ── §2.57 body golden (RECOVERED, SESSION_RENEW_REQUIRED, PARAM_ACK byte-exact) ──
#[test]
fn control_body_golden() {
    // [D2 2026-10-08] Ack golden kaldırıldı — aile kodda yok (K3). Yerine count+aralık yapısını
    // taşıyan kalan tek tür RECOVERED; beklenen byte'lar codec yerleşiminden elle türetildi:
    // block_id u32_be · count u16_be · count × (esi_start u16_be, esi_end u16_be).
    // RECOVERED: block_id=0x0A0B0C0D · count=1 · [esi_start=0x0A, esi_end=0x14]
    let rec = Control::Recovered {
        id: 0,
        block_id: 0x0A0B_0C0D,
        esi_ranges: vec![(0x0A, 0x14)],
    };
    let expect = vec![0x0A, 0x0B, 0x0C, 0x0D, 0x00, 0x01, 0x00, 0x0A, 0x00, 0x14];
    assert_eq!(encode_control_body(&rec), expect);

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
// [D2 2026-10-08] ACK kolu (`incoming_ack_over_cap_rejected`) kaldırıldı — aile kodda yok (K3).
// Aynı gelen-tavan mekanizması RECOVERED'a yeniden hedeflendi: kind/count/cap TAM pinlenir. Sayaç
// u16::MAX beyan edilir, aralık byte'ı verilmez → tavan kapısı okumadan/ayırmadan ÖNCE düşmeli.
#[test]
fn incoming_recovered_max_count_rejected_before_read() {
    let mut body = Vec::new();
    body.extend_from_slice(&9u32.to_be_bytes()); // block_id
    body.extend_from_slice(&u16::MAX.to_be_bytes()); // count (aralık byte'ı YOK)
    assert_eq!(
        decode_control_body(inner_kind::RECOVERED, 0, &body),
        Err(WireError::ControlCountExceedsCap {
            kind: inner_kind::RECOVERED,
            count: usize::from(u16::MAX),
            cap: RECOVERED_MAX_ESI_RANGES,
        })
    );
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
// [D2 2026-10-08] ACK kolu (`outgoing_ack_over_cap_split`) kaldırıldı — aile kodda yok (K3); aynı
// bölme mekanizması RECOVERED'a yeniden hedeflendi (tavanlı/bölünen tek tür).
#[test]
fn outgoing_recovered_over_cap_split() {
    let ranges: Vec<(u16, u16)> = (0..(RECOVERED_MAX_ESI_RANGES as u16 * 2 + 5))
        .map(|i| (i, i))
        .collect();
    let big = Control::Recovered {
        id: 100,
        block_id: 9,
        esi_ranges: ranges.clone(),
    };
    let mut next = 1000u32;
    let parts = split_for_send(&big, || {
        next += 1;
        next
    });
    // 37 aralık / 16 = 3 parça
    assert_eq!(parts.len(), 3);
    // İlk parça orijinal-id, sonrakiler yeni-id
    assert_eq!(parts[0].control_id(), 100);
    assert_ne!(parts[1].control_id(), 100);
    assert_ne!(parts[2].control_id(), parts[1].control_id());
    // Her parça tavan-içinde + toplam-aralık korunur
    let mut total = 0;
    for p in &parts {
        let len = control_body_len(p).unwrap();
        assert!(
            len <= control_body_len(&Control::Recovered {
                id: 0,
                block_id: 0,
                esi_ranges: vec![(0, 0); RECOVERED_MAX_ESI_RANGES]
            })
            .unwrap()
        );
        if let Control::Recovered { esi_ranges, .. } = p {
            total += esi_ranges.len();
            assert!(esi_ranges.len() <= RECOVERED_MAX_ESI_RANGES);
        }
    }
    assert_eq!(total, ranges.len());
}

// [D2 2026-10-08] SACK kolu (`sack_split_and_within_cap_noop`) kaldırıldı — aile kodda yok (K3);
// aynı tavan-içi/tavan-aşımı davranışı RECOVERED'a yeniden hedeflendi.
#[test]
fn recovered_split_and_within_cap_noop() {
    // Tavan-içi RECOVERED → tek-parça, aynı-id.
    let small = Control::Recovered {
        id: 5,
        block_id: 0,
        esi_ranges: vec![(1, 1)],
    };
    let parts = split_for_send(&small, || 999);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].control_id(), 5);
    // Tavan-aşan RECOVERED → böl
    let esi_ranges: Vec<(u16, u16)> = (0..(RECOVERED_MAX_ESI_RANGES as u16 + 1))
        .map(|i| (i, i))
        .collect();
    let big = Control::Recovered {
        id: 5,
        block_id: 0,
        esi_ranges,
    };
    assert_eq!(split_for_send(&big, || 999).len(), 2);
}

// ── fail-closed: invalid-range / trailing / kısa-body ──
#[test]
fn invalid_range_rejected() {
    // ~~ACK pn_start > pn_end~~ [D2 2026-10-08: ACK kolu kaldırıldı, K3] → RECOVERED esi_start >
    // esi_end (range-doğrulaması olan kalan tek tür).
    let mut body = Vec::new();
    body.extend_from_slice(&9u32.to_be_bytes()); // block_id
    body.extend_from_slice(&1u16.to_be_bytes()); // count
    body.extend_from_slice(&20u16.to_be_bytes()); // esi_start
    body.extend_from_slice(&10u16.to_be_bytes()); // esi_end < esi_start
    assert!(matches!(
        decode_control_body(inner_kind::RECOVERED, 0, &body),
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
    // [D2 2026-10-08] ~~ACK/SACK~~ tablodan çıktı; 0x10/0x11 RESERVED → registry-dışı fallback
    // NoRetry (fail-safe: reserved codepoint retry edilmez).
    assert_eq!(
        RetryClass::of(inner_kind::RESERVED_ACK),
        RetryClass::NoRetry
    );
    assert_eq!(
        RetryClass::of(inner_kind::RESERVED_SACK),
        RetryClass::NoRetry
    );
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
    // [D2 2026-10-08] ~~ACK~~ → RECOVERED (Ack ailesi kodda yok, K3). Gövde bilinçli kısa: ayrım
    // inner-türünden yapılır, gövde-geçerliliğinden değil (Some(Err) da Some'dır).
    let ctl = InnerPlain::Control {
        inner_kind: inner_kind::RECOVERED,
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
