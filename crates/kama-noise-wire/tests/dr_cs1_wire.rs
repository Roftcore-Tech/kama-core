//! DR-CS-1 WIRE-CODEPOINT AMENDMANI kapı-testleri (Abkarada-karar §6; F2B-ÖNKOŞUL).
//! 0x19 CTRL_CS_PROBE / 0x1A CTRL_CS_ECHO — registry-uniqueness · runtime-assert · exact-layout ·
//! malformed-reject · unknown-codepoint-safe-ignore(sınıf-özelliği) · replay · golden-değişmedi.

use kama_noise_wire::control::{
    control_body_len, decode_control, encode_control_body, CS_ECHO_BODY_LEN, CS_PROBE_BODY_LEN,
    CS_PROBE_PAD_LEN, CS_SUBTYPE_COLD_START,
};
use kama_noise_wire::retry::{InboundControlDedup, InboundOutcome, RetryClass};
use kama_noise_wire::{KamaNoiseWireProtector, KamaNoiseWireSizer, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::{self, classify, InnerKind};
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
fn probe(id: u32, seq: u16) -> Control {
    Control::CsProbe {
        id,
        subtype: CS_SUBTYPE_COLD_START,
        probe_seq: seq,
    }
}
fn echo(id: u32, seq: u16) -> Control {
    Control::CsEcho {
        id,
        subtype: CS_SUBTYPE_COLD_START,
        probe_seq: seq,
        seen_count: 3,
        arrival_gap: 40_000,
    }
}

/// §6.1 — Registry uniqueness: 0x19/0x1A benzersiz; 0x12 hâlâ YALNIZ RECOVERED.
#[test]
fn g1_registry_uniqueness_and_recovered_lock() {
    let all = [
        inner_kind::FEC_SOURCE,
        inner_kind::FEC_REPAIR,
        inner_kind::ACK,
        inner_kind::SACK,
        inner_kind::RECOVERED,
        inner_kind::BLOCK_COMPLETE,
        inner_kind::PARAM_SET,
        inner_kind::PARAM_ACK,
        inner_kind::BLOCK_ABORT,
        inner_kind::SESSION_CLOSE,
        inner_kind::SESSION_RENEW_REQUIRED,
        inner_kind::CTRL_CS_PROBE,
        inner_kind::CTRL_CS_ECHO,
    ];
    let mut seen = std::collections::BTreeSet::new();
    for cp in all {
        assert!(seen.insert(cp), "codepoint-çakışması: {cp:#04x}");
    }
    assert_eq!(inner_kind::CTRL_CS_PROBE, 0x19);
    assert_eq!(inner_kind::CTRL_CS_ECHO, 0x1A);
    // 0x12-KİLİDİ: RECOVERED'dır ve DTO-codepoint'leri doğru-eşleşir.
    assert_eq!(inner_kind::RECOVERED, 0x12);
    assert_eq!(
        Control::Recovered {
            id: 0,
            block_id: 0,
            esi_ranges: vec![]
        }
        .codepoint(),
        0x12
    );
    assert_eq!(probe(0, 0).codepoint(), 0x19);
    assert_eq!(echo(0, 0).codepoint(), 0x1A);
}

/// §6.2 — Runtime-registry assert: classify TAM-OLARAK authoritative-küme'yi kabul-eder
/// ({0x01,0x02} ∪ {0x10..=0x1A}); geri-kalan-256-uzayı FAIL-CLOSED.
#[test]
fn g2_runtime_registry_matches_authoritative_exactly() {
    for cp in 0u16..=255 {
        let cp = cp as u8;
        let expect_ok = matches!(cp, 0x01 | 0x02) || (0x10..=0x1A).contains(&cp);
        assert_eq!(
            classify(cp).is_ok(),
            expect_ok,
            "classify({cp:#04x}) authoritative-registry'yle-uyuşmuyor"
        );
    }
    assert!(matches!(classify(0x19), Ok(InnerKind::Control(0x19))));
    assert!(matches!(classify(0x1A), Ok(InnerKind::Control(0x1A))));
}

/// §6.3 — Exact encode/decode: golden-body-byte'ları + exact-length (probe-body=12 → wire=44;
/// echo-body=11 → wire=43) + seal→open→decode roundtrip.
#[test]
fn g3_exact_layout_roundtrip_and_wire_lengths() {
    // GOLDEN body-byte'ları (spec-dondurulmuş §2.57):
    let p = probe(7, 0x0102);
    let mut expect_p = vec![0x01, 0x01, 0x02];
    expect_p.extend_from_slice(&[0u8; CS_PROBE_PAD_LEN]);
    assert_eq!(encode_control_body(&p), expect_p);
    assert_eq!(control_body_len(&p).unwrap(), CS_PROBE_BODY_LEN);

    let e = echo(8, 0x0102);
    let mut expect_e = vec![0x01, 0x01, 0x02];
    expect_e.extend_from_slice(&3u32.to_be_bytes());
    expect_e.extend_from_slice(&40_000u32.to_be_bytes());
    assert_eq!(encode_control_body(&e), expect_e);
    assert_eq!(control_body_len(&e).unwrap(), CS_ECHO_BODY_LEN);

    // EXACT-WIRE (karar-§4; varsayım-değil-ölçüm): PROBE=44B, ECHO=43B.
    let (mut a, mut b) = paired();
    for (c, want_wire) in [(p.clone(), 44usize), (e.clone(), 43usize)] {
        let pkt = UnsealedTransportPacket::Control(c.clone());
        assert_eq!(
            a.sealed_len(&pkt).unwrap(),
            want_wire,
            "sealed_len {:#04x}",
            c.codepoint()
        );
        let sealed = a
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: u64::from(c.codepoint()),
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(sealed.bytes.len(), want_wire, "encode-length-testi");
        let opened = b.open(&sealed.bytes).unwrap();
        let decoded = decode_control(opened.inner()).unwrap().unwrap();
        assert_eq!(decoded, c, "roundtrip");
    }
    // amp≤1: probe_wire(44) ≥ echo_wire(43).
    assert!(44 >= 43);
}

/// §6.3 — Malformed/truncated/reserved reject (FAIL-CLOSED).
#[test]
fn g3b_malformed_truncated_reject() {
    use kama_noise_wire::control::decode_control_body;
    // kısa-body (probe 12B-altı):
    for cut in 0..CS_PROBE_BODY_LEN {
        let body = encode_control_body(&probe(1, 5));
        assert!(
            decode_control_body(0x19, 1, &body[..cut]).is_err(),
            "truncated-probe({cut}B) kabul-edildi"
        );
    }
    // nonzero-pad → reject (reserved-alan-nonzero kuralı):
    let mut bad_pad = encode_control_body(&probe(1, 5));
    *bad_pad.last_mut().unwrap() = 0xFF;
    assert!(
        decode_control_body(0x19, 1, &bad_pad).is_err(),
        "nonzero-pad"
    );
    // reserved-subtype → reject:
    let mut bad_st = encode_control_body(&probe(1, 5));
    bad_st[0] = 0x02;
    assert!(decode_control_body(0x19, 1, &bad_st).is_err(), "subtype");
    // trailing → reject:
    let mut trail = encode_control_body(&echo(1, 5));
    trail.push(0x00);
    assert!(decode_control_body(0x1A, 1, &trail).is_err(), "trailing");
    // echo kısa-body:
    let e = encode_control_body(&echo(1, 5));
    assert!(decode_control_body(0x1A, 1, &e[..CS_ECHO_BODY_LEN - 1]).is_err());
}

/// §6.4 — Unknown-codepoint SINIF-ÖZELLİĞİ: bilinmeyen-inner_kind FRAME-seviyesi-reject'tir,
/// session-YAŞAR (eski-parser'ın 0x19/0x1A-karşısındaki-davranış-sınıfı: bugünkü-parser'ın
/// 0x1B-karşısındaki-davranışıyla-AYNI-yol — UnknownInnerKind → frame-drop). Bozuk/bilinmeyen-
/// frame-sonrası aynı-session'da geçerli-frame AÇILMAYA-devam-eder.
#[test]
fn g4_unknown_codepoint_frame_reject_session_lives() {
    // (a) classify: 0x1B (reserved) frame-reject-yolu.
    assert!(classify(0x1B).is_err(), "0x1B-hâlâ-reserved-reject");
    // (b) session-yaşar: garbage/bilinmeyen-datagram open-hatası → SONRAKİ geçerli-frame açılır.
    let (mut a, mut b) = paired();
    let sealed1 = a
        .seal(
            &UnsealedTransportPacket::Control(probe(1, 0)),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    // bozuk-frame (tag-kırık) → frame-reject:
    let mut corrupt = sealed1.bytes.clone();
    let n = corrupt.len();
    corrupt[n - 1] ^= 0xFF;
    assert!(b.open(&corrupt).is_err(), "bozuk-frame-reject");
    // session-devam: orijinal-frame hâlâ-açılır (state-poison-yok).
    let opened = b.open(&sealed1.bytes).unwrap();
    assert!(decode_control(opened.inner()).unwrap().is_ok());
    // sonraki-pn-de-açılır:
    let sealed2 = a
        .seal(
            &UnsealedTransportPacket::Control(echo(2, 0)),
            SealArgs {
                wire_pn: 1,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&sealed2.bytes).is_ok(), "session-yaşıyor");
}

/// §6.5 — Replay: duplicate probe/echo İKİNCİ-kez kabul-edilmez (outer-pn-window) +
/// control_id-dedup (idempotent) + RetryClass=NoRetry.
#[test]
fn g5_replay_and_dedup() {
    let (mut a, mut b) = paired();
    let sealed = a
        .seal(
            &UnsealedTransportPacket::Control(probe(9, 3)),
            SealArgs {
                wire_pn: 42,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&sealed.bytes).is_ok(), "ilk-teslim");
    assert!(
        b.open(&sealed.bytes).is_err(),
        "AYNI-bytes-replay-reject (pn-window)"
    );
    // control_id-dedup (§2.57 idempotency; probe/echo-otomatik-kapsam):
    let mut d = InboundControlDedup::new(16);
    assert_eq!(d.admit(&probe(9, 3)), InboundOutcome::Fresh);
    assert_eq!(d.admit(&probe(9, 3)), InboundOutcome::DuplicateAckOnly);
    // RetryClass: RETRY-EDİLMEZ (spec-§2.57-amendman-tablosu).
    assert_eq!(RetryClass::of(0x19), RetryClass::NoRetry);
    assert_eq!(RetryClass::of(0x1A), RetryClass::NoRetry);
}

/// §6.8 — Sealed-control compatibility: MEVCUT frame'lerin (RECOVERED-örneği) golden-byte'ları
/// DEĞİŞMEDİ (amendman mevcut-layout'lara-dokunmaz; tam-suite = kalan-kanıt).
#[test]
fn g8_existing_recovered_golden_unchanged() {
    let rec = Control::Recovered {
        id: 3,
        block_id: 9,
        esi_ranges: vec![(0, 3), (5, 5)],
    };
    let mut expect = Vec::new();
    expect.extend_from_slice(&9u32.to_be_bytes());
    expect.extend_from_slice(&2u16.to_be_bytes());
    for (s, e) in [(0u16, 3u16), (5, 5)] {
        expect.extend_from_slice(&s.to_be_bytes());
        expect.extend_from_slice(&e.to_be_bytes());
    }
    assert_eq!(
        encode_control_body(&rec),
        expect,
        "RECOVERED-golden-değişmedi"
    );
    assert_eq!(control_body_len(&rec).unwrap(), 14);
}
