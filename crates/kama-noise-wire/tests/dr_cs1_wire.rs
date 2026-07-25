//! DR-CS-1 WIRE-CODEPOINT AMENDMANI kapı-testleri (Abkarada-karar §6; F2B-ÖNKOŞUL).
//! 0x19 CTRL_CS_PROBE / 0x1A CTRL_CS_ECHO — registry-uniqueness · runtime-assert · exact-layout ·
//! malformed-reject · unknown-codepoint-safe-ignore(sınıf-özelliği) · replay · golden-değişmedi.

use kama_noise_wire::control::{
    control_body_len, decode_control, decode_control_body, encode_control_body, CS_ECHO_BODY_LEN,
    CS_PROBE_BODY_LEN, CS_PROBE_PAD_LEN, CS_SUBTYPE_COLD_START,
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
    // amp≤1: probe_wire ≥ echo_wire (runtime-sizer; sabit-literal-değil).
    let probe_wire = a.sealed_len(&UnsealedTransportPacket::Control(p)).unwrap();
    let echo_wire = a.sealed_len(&UnsealedTransportPacket::Control(e)).unwrap();
    assert!(
        probe_wire >= echo_wire,
        "amp≤1: {probe_wire} >= {echo_wire}"
    );
}

/// §6.3 — Malformed/truncated/reserved reject (FAIL-CLOSED).
#[test]
fn g3b_malformed_truncated_reject() {
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

/// FC5 — TAM MIXED-VERSION MATRİSİ (5-satır; codec/runtime-seviyesi = faithful-PRODUCTION-yolu; harness-
/// raw-UDP AEAD-sealed-DEĞİL, safe-ignore'u-modellemez). Her-satır İKİ-sonuç: wire_safety +
/// feature_effectiveness. "old receiver" = amendman-ÖNCESİ allowlist {0x01,0x02,0x10..0x18} (0x19/0x1A
/// reserved-reject); `old_classify` bunu-modeller (bugünkü-classify'ın-0x1B-davranışıyla-özdeş = g4).
#[test]
fn g9_mixed_version_matrix() {
    // Amendman-ÖNCESİ (old) allowlist: 0x19/0x1A YOK → reject (frame-drop, safe-ignore).
    let old_classify =
        |cp: u8| -> bool { matches!(cp, 0x01 | 0x02) || (0x10..=0x18).contains(&cp) };
    let (mut a, mut b) = paired();
    let mut pnc = 0u64;
    let mut pn = || {
        pnc += 1;
        pnc
    };

    // ── Satır 1: new-sender-ON → old-receiver ──
    // Sender 0x19/0x1A-emit-eder; old-rx AEAD-açar (authentic) ama inner-classify-reddeder → FRAME-drop
    // (session-yaşar); echo-üretmez → feature-effectiveness FAIL. wire_safety PASS.
    for c in [probe(1, 0), echo(2, 0)] {
        let cp = c.codepoint();
        assert!(
            !old_classify(cp),
            "old-rx 0x{cp:02x}'i-TANIMAZ (reserved-reject)"
        );
        // SEALED-frame old-rx'te de AEAD-açılır (kimlik-doğru); inner-reject = frame-drop, session-alive:
        let sealed = a
            .seal(
                &UnsealedTransportPacket::Control(c),
                SealArgs {
                    wire_pn: pn(),
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert!(
            b.open(&sealed.bytes).is_ok(),
            "AEAD-açılır (session-kill-YOK)"
        );
        // Sonraki-BİLİNEN-frame doğru-decode (parser-desync-YOK):
        let known = a
            .seal(
                &UnsealedTransportPacket::Control(Control::Ack {
                    id: 9,
                    acked_dh_epoch: 0,
                    ranges: vec![(1, 2)],
                }),
                SealArgs {
                    wire_pn: pn(),
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        let op = b.open(&known.bytes).unwrap();
        assert!(matches!(
            decode_control(op.inner()).unwrap().unwrap(),
            Control::Ack { .. }
        ));
    }

    // ── Satır 2: new-sender-OFF → old-receiver ──
    // OFF: 0x19/0x1A wire'a-ÇIKMAZ (sender-flag-OFF; adaptive-transport-INV-CS-5-kimliği). Codec-seviyesi:
    // yalnız-mevcut-frame'ler → old-rx tam-uyumlu. Kanıt: RECOVERED(0x12)-golden old-classify'da-da-geçerli.
    assert!(
        old_classify(0x12),
        "0x12=RECOVERED old-rx'te-de-BİLİNİR (değişmedi)"
    );

    // ── Satır 3: old-sender → new-receiver ──
    // Old-sender 0x19/0x1A-göndermez; new-rx tüm-eski-frame'leri-decode-eder + 0x12=RECOVERED-golden.
    let rec = Control::Recovered {
        id: 3,
        block_id: 9,
        esi_ranges: vec![(0, 3)],
    };
    let sealed = a
        .seal(
            &UnsealedTransportPacket::Control(rec.clone()),
            SealArgs {
                wire_pn: pn(),
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    let op = b.open(&sealed.bytes).unwrap();
    assert_eq!(
        decode_control(op.inner()).unwrap().unwrap(),
        rec,
        "new-rx eski-frame-decode"
    );
    assert_eq!(inner_kind::RECOVERED, 0x12);

    // ── Satır 4: new-sender-ON → new-receiver ──
    // 0x19/0x1A tam-roundtrip + classify-Ok → feature EFFECTIVE. wire_safety PASS.
    for c in [probe(4, 7), echo(5, 7)] {
        assert!(matches!(classify(c.codepoint()), Ok(InnerKind::Control(_))));
        let sealed = a
            .seal(
                &UnsealedTransportPacket::Control(c.clone()),
                SealArgs {
                    wire_pn: pn(),
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        let op = b.open(&sealed.bytes).unwrap();
        assert_eq!(
            decode_control(op.inner()).unwrap().unwrap(),
            c,
            "new/new roundtrip (feature-effective)"
        );
    }

    // ── Satır 5: new-sender-OFF → new-receiver ──
    // OFF → 0x19/0x1A wire'a-çıkmaz; mevcut-frame'ler byte-özdeş (g8). Codec: RECOVERED-golden-korunur.
    // (Sender-flag-OFF-byte-for-byte adaptive-transport-INV-CS-5-testinde; burada-codec-uyum.)
    assert!(matches!(classify(0x12), Ok(InnerKind::Control(0x12))));

    // Duplicate/replay 0x19/0x1A ikinci-kez-etkisiz (g5-tekrar; matris-satır-ortak):
    let s = a
        .seal(
            &UnsealedTransportPacket::Control(probe(6, 1)),
            SealArgs {
                wire_pn: pn(),
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&s.bytes).is_ok());
    assert!(b.open(&s.bytes).is_err(), "replay-reject (pn-window)");
}

// ─────────────────────────────────────────────────────────────────────────────
// DI-10 — RECEIVER RESOURCE-CAP / LIFECYCLE (cold-start ADDITIONS'un O(1)/bounded-kanıtı).
// Ürün-alıcı UYGULANMADI; bu testler DR-CS-1 WIRE-eklerinin (0x19/0x1A) alıcıya UNBOUNDED-durum
// DAYATMADIĞINI production-codec-seviyesinde kanıtlar (harness-artefaktı-değil).
// ─────────────────────────────────────────────────────────────────────────────

/// §DI-10.1 — Wire-ekleri NoRetry: gönderen probe/echo için retry-kuyruğu-durumu BİRİKTİRMEZ
/// (deadline-bounded UntilNetworkAck DEĞİL). → sender-tarafı O(1) (ladder-zamanlayıcı ayrı; INV-CS-7).
#[test]
fn g10_di10_wire_additions_are_noretry() {
    assert_eq!(
        RetryClass::of(inner_kind::CTRL_CS_PROBE),
        RetryClass::NoRetry
    );
    assert_eq!(
        RetryClass::of(inner_kind::CTRL_CS_ECHO),
        RetryClass::NoRetry
    );
    assert_eq!(RetryClass::of_control(&probe(1, 1)), RetryClass::NoRetry);
    assert_eq!(RetryClass::of_control(&echo(1, 1)), RetryClass::NoRetry);
}

/// §DI-10.2 — Fixed-width / amplifikasyon-yok: probe/echo body-uzunluğu ALAN-DEĞERLERİNDEN bağımsız
/// SABİT (length-prefix-YOK → wire UNBOUNDED-alloc İSTEYEMEZ). Ekstrem-değerler → aynı-boyut.
#[test]
fn g10b_di10_fixed_width_no_amplification() {
    let p_ext = Control::CsProbe {
        id: u32::MAX,
        subtype: CS_SUBTYPE_COLD_START,
        probe_seq: u16::MAX,
    };
    let e_ext = Control::CsEcho {
        id: u32::MAX,
        subtype: CS_SUBTYPE_COLD_START,
        probe_seq: u16::MAX,
        seen_count: u32::MAX,
        arrival_gap: u32::MAX,
    };
    assert_eq!(control_body_len(&p_ext).unwrap(), CS_PROBE_BODY_LEN);
    assert_eq!(control_body_len(&e_ext).unwrap(), CS_ECHO_BODY_LEN);
    assert_eq!(encode_control_body(&p_ext).len(), CS_PROBE_BODY_LEN);
    assert_eq!(encode_control_body(&e_ext).len(), CS_ECHO_BODY_LEN);
    // sealed-wire de sabit (44/43); girdi-ekstrem olsa da amplifikasyon-yok.
    let (mut a, _b) = paired();
    for (i, (c, want)) in [(p_ext.clone(), 44usize), (e_ext.clone(), 43usize)]
        .into_iter()
        .enumerate()
    {
        let s = a
            .seal(
                &UnsealedTransportPacket::Control(c),
                SealArgs {
                    wire_pn: (i + 1) as u64,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(s.bytes.len(), want, "sealed-wire sabit-değil");
    }
}

/// §DI-10.3 — ADVERSARIAL: probe'lar InboundControlDedup.admit()'e SOKULMAMALI. Kanıt: farklı-id'li
/// N-probe admit()'e girerse `seen` LİNEER-BÜYÜR (unbounded; Sprint-4-TTL-öncesi bilinen-sınır).
/// → alıcı probe'u STATELESS-echo-yoluyla ele-almalı (idempotent-yeniden-üretim; harness bu-modeli
/// [udp_runner.rs rx 0x19→43B fixed-buf, SIFIR-map-insert] kullanır). Bu test anti-pattern'i-sabitler.
#[test]
fn g10c_di10_probe_flood_must_not_dedup_unbounded() {
    let mut dedup = InboundControlDedup::new(64);
    const FLOOD: u32 = 1024;
    for i in 0..FLOOD {
        // farklı control_id (=id) → her-biri Fresh → seen'e-eklenir.
        assert_eq!(
            dedup.admit(&probe(i, (i & 0xFFFF) as u16)),
            InboundOutcome::Fresh
        );
    }
    // KANIT: admit()-yolu unbounded (N-probe → N-seen). Bu YÜZDEN probe admit()'ten-GEÇMEMELİ.
    assert_eq!(
        dedup.seen_count(),
        FLOOD as usize,
        "admit-yolu-unbounded (anti-pattern-kanıtı)"
    );
    // Karşıt: STATELESS-echo yolu — hiçbir-koleksiyon-tutmadan echo-body üretir (O(1); durum-birikmez).
    // Body sabit CS_ECHO_BODY_LEN (11B); sealed-wire eşdeğeri 43B (g3/g10b). Working-set probe-sayısından
    // BAĞIMSIZ (FLOOD-boyunca sabit) → O(1) bellek.
    let mut last_len = 0usize;
    for i in 0..FLOOD {
        let e = stateless_echo(
            &probe(i, (i & 0xFFFF) as u16),
            /*seen*/ 3,
            /*gap*/ 40_000,
        );
        assert_eq!(e.len(), CS_ECHO_BODY_LEN, "stateless-echo body sabit-11B");
        last_len = e.len();
    }
    assert_eq!(last_len, CS_ECHO_BODY_LEN); // working-set sabit; probe-sayısından bağımsız (O(1)).
}

/// §DI-10.4 — Echo idempotent-yeniden-üretim: aynı (probe_seq, ledger) → BYTE-ÖZDEŞ echo (saklı-durum
/// GEREKMEZ; replay'de yeniden-üretilebilir) + seal→open→decode roundtrip. Stateless-regen kanıtı.
#[test]
fn g10d_di10_echo_idempotent_regeneration() {
    let e1 = stateless_echo(&probe(9, 0x0102), 7, 12_345);
    let e2 = stateless_echo(&probe(9, 0x0102), 7, 12_345);
    assert_eq!(e1, e2, "aynı-girdi → byte-özdeş echo (idempotent-regen)");
    // decode-edilebilir + roundtrip (production-codec):
    let c = Control::CsEcho {
        id: 9,
        subtype: CS_SUBTYPE_COLD_START,
        probe_seq: 0x0102,
        seen_count: 7,
        arrival_gap: 12_345,
    };
    let body = encode_control_body(&c);
    assert_eq!(
        &e1[..],
        &body[..],
        "stateless_echo body == codec-encode (aynı-wire)"
    );
    let dec = decode_control_body(inner_kind::CTRL_CS_ECHO, 9, &body).unwrap();
    assert_eq!(dec, c, "roundtrip (regen → decode)");
}

/// STATELESS echo-üretici (DI-10 modeli): saklı-durum-YOK; girdi→11B-body (harness rx 43B-wire buf'un
/// codec-eşdeğeri). Yalnız test-yardımcısı; production alıcı-bu-saf-fonksiyonu-kullanmalı.
fn stateless_echo(p: &Control, seen_count: u32, arrival_gap: u32) -> Vec<u8> {
    let Control::CsProbe { id, probe_seq, .. } = *p else {
        panic!("probe-değil")
    };
    encode_control_body(&Control::CsEcho {
        id,
        subtype: CS_SUBTYPE_COLD_START,
        probe_seq,
        seen_count,
        arrival_gap,
    })
}
