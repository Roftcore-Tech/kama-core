//! L-A5b — `NoiseWireProtection` (A5.2 seal/open + sayaç + nonce/AAD · A5.3 replay): S3
//! conformance süitinin (`noise_conformance_core`, `--target real`) ÖRTMEDİĞİ iddialar.
//!
//! * **S10** (sahip kararı 2026-10-08): extended 26 B zarfın Python vektörü yok → DİFERANSİYEL.
//!   Aynı anahtar / tutamaç / epoch / PN ile `NoiseWireProtection` ↔ mevcut
//!   `KamaNoiseWireProtector` mühürleri BAYT-ÖZDEŞ; aynı datagram dizisinde iki alıcının
//!   kabul/ret kararları, ret gerekçeleri ve pencereleri adım adım aynı.
//! * **S6**: `SEND_PN_CAP` = ccr §35.8 REKEY_THRESHOLD (2^31); tavanda `CapReached`, sayaç
//!   tüketilmez, TEK `RenewRequired`; `close` bekleyen `RenewRequired`'ı düşürür.
//! * `sealed_len` kesin (iki profil + HubChannel); hata sırası `seal` ile aynı, state değişmez.
//! * Replay: `PN = u64::MAX` reddi (iki genişliğin parametreleriyle).
//! * Yaşam döngüsü: `retire_send` sonrası seal yok, alım sürer; `close` demux kaydını siler.

#![cfg(feature = "test-util")]

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionClass, ProtectionContext, SealError, SessionEvent,
};
use kama_noise_wire::aad::build_aad;
use kama_noise_wire::envelope::EnvelopeProfile;
use kama_noise_wire::kdf::SESSION_ID_INTERNAL_LEN;
use kama_noise_wire::outer::OUTER_HDR_LEN;
use kama_noise_wire::protector::{NoiseWireProtection, SessionKeyMaterial};
use kama_noise_wire::replay::DirectionalReplay;
use kama_noise_wire::{
    nonce96, ExtendedOuter, KamaNoiseWireProtector, LegacyDirection, PnReject, SealArgs, WireError,
    EXTENDED_PN_PARAMS, KEY_LEN, LEGACY_PN_PARAMS, SEND_PN_CAP,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0000_7E57;
const A2B_E0: u8 = 0xA0;
const B2A_E0: u8 = 0xB0;
const A2B_E1: u8 = 0xA1;
const B2A_E1: u8 = 0xB1;

fn key(b: u8) -> Zeroizing<[u8; KEY_LEN]> {
    Zeroizing::new([b; KEY_LEN])
}

fn extended() -> EnvelopeProfile {
    EnvelopeProfile::Extended {
        h_ext: HubHandle::from_raw(H_EXT),
    }
}

fn legacy(send_direction: LegacyDirection) -> EnvelopeProfile {
    EnvelopeProfile::Legacy {
        hub_handle: HubHandle::from_raw(H_EXT),
        protocol_version: 1,
        cipher_suite_id: 1,
        session_id_internal: [0x51; SESSION_ID_INTERNAL_LEN],
        send_direction,
    }
}

/// Tek oturumlu uç: `send` ile mühürler, `recv` ile açar.
fn endpoint(send: u8, recv: u8, profile: EnvelopeProfile) -> (NoiseWireProtection, SessionHandle) {
    let mut p = NoiseWireProtection::new();
    let h = p
        .install_session_for_test(SessionKeyMaterial {
            profile,
            send_e0: key(send),
            recv_e0: key(recv),
        })
        .expect("taze uçta oturum kurulur");
    (p, h)
}

/// Mevcut protector'ın kodlayacağı geçerli bir iç çerçeve (boyu ve içeriği `i`'ye göre değişir).
fn packet(i: u64) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: u32::try_from(i % 1024).unwrap(),
            esi: u16::try_from(i % 4).unwrap(),
            k_actual: 4,
        },
        symbol: vec![u8::try_from(i % 251).unwrap(); 1 + usize::try_from(i % 64).unwrap()],
    }
}

fn seal_args(wire_pn: u64, dh_epoch: u8) -> SealArgs {
    SealArgs {
        wire_pn,
        dh_epoch,
        send_generation: 0,
    }
}

/// Mevcut protector'ın çıktısından iç çerçeveyi HAM AEAD ile çıkarır (üçüncü, bağımsız yol):
/// `NoiseWireProtection` çerçeveyi OPAK mühürler, iç kodlamayı bilmez.
fn inner_frame(wire: &[u8], key_byte: u8, pn: u64) -> Vec<u8> {
    let (outer, _, _) = ExtendedOuter::parse(wire).expect("dış başlık");
    let aad = build_aad(&outer);
    let k = key(key_byte);
    ChaCha20Poly1305::new(Key::from_slice(k.as_ref()))
        .decrypt(
            Nonce::from_slice(&nonce96(pn)),
            Payload {
                msg: &wire[OUTER_HDR_LEN..],
                aad: &aad,
            },
        )
        .expect("mevcut protector'ın çıktısı ham AEAD ile açılır")
}

/// Son baytın en düşük biti çevrilmiş kopya (tag bozulur).
fn tamper(wire: &[u8]) -> Vec<u8> {
    let mut w = wire.to_vec();
    let last = w.len() - 1;
    w[last] ^= 1;
    w
}

// ── S10: extended zarf diferansiyeli ─────────────────────────────────────────────────────────

/// Seal: her (epoch, PN) noktasında iki uygulamanın wire baytları özdeş; PN 16/32 bit sınırları,
/// epoch geçişi ve tavanın bir altı dahil. Tavanda ikisi de reddeder.
#[test]
fn s10_extended_seal_is_byte_identical_to_existing_protector() {
    let mut old = KamaNoiseWireProtector::new(H_EXT, &key(A2B_E0), &key(B2A_E0));
    let (mut new, h) = endpoint(A2B_E0, B2A_E0, extended());
    let ctx = ProtectionContext::Session(h);
    let schedule: [(u8, u64); 10] = [
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 0xFF),
        (0, 0x100),
        (0, 0xFFFF),
        (1, 0x1_0000),
        (1, 0x1_0001),
        (1, 0x00FF_FFFF),
        (1, SEND_PN_CAP - 1),
    ];
    let mut in_epoch1 = false;
    for (i, (epoch, pn)) in schedule.into_iter().enumerate() {
        if epoch == 1 && !in_epoch1 {
            old.install_epoch1_keys(&key(A2B_E1), &key(B2A_E1));
            assert!(new.install_epoch1_for_test(h, &key(A2B_E1), &key(B2A_E1)));
            assert!(new.set_send_epoch_for_test(h, 1));
            in_epoch1 = true;
        }
        let reference = old
            .seal(&packet(i as u64), seal_args(pn, epoch))
            .expect("mevcut protector mühürler");
        let send_key = if epoch == 0 { A2B_E0 } else { A2B_E1 };
        let frame = inner_frame(&reference.bytes, send_key, pn);
        assert!(
            new.set_next_send_pn_for_test(h, pn),
            "sayaç {pn}'e taşınamadı"
        );
        let unit = new.seal(ctx, &frame).expect("NoiseWireProtection mühürler");
        assert_eq!(
            unit.bytes, reference.bytes,
            "epoch {epoch} pn {pn}: mühürler ayrıştı"
        );
        assert_eq!(
            (unit.meta.full_wire_pn, unit.meta.dh_epoch),
            (pn, epoch),
            "meta kullanılan sayaç/epoch değil"
        );
        assert_eq!(
            usize::try_from(unit.meta.sealed_len).unwrap(),
            reference.meta.sealed_len
        );
    }
    assert_eq!(
        old.seal(&packet(99), seal_args(SEND_PN_CAP, 1))
            .unwrap_err(),
        WireError::PnCapReached
    );
    assert_eq!(new.seal(ctx, b"x").unwrap_err(), SealError::CapReached);
}

/// Open: aynı datagram dizisi (tekrar, sıra dışı, bozuk tag, far-future, pencere dışı, grace
/// sonrası epoch-0) iki alıcıya verilir; her adımda karar, gerekçe, kabul edilen PN/epoch ve
/// `expected_pn` aynı. Dizi sabit tohumlu (RNG yok); her ret sınıfının en az bir kez oluştuğu
/// ayrıca iddia edilir (vakum değil).
#[test]
fn s10_extended_open_decisions_match_existing_protector() {
    let gap = EXTENDED_PN_PARAMS
        .future_gap_max
        .expect("extended far-future kapısı var");
    let window = EXTENDED_PN_PARAMS.window;

    // Havuz — mevcut protector mühürler (çıktısı yukarıda NoiseWireProtection'ınkiyle özdeş).
    // Epoch 0: 0..40, epoch 1: 40..80 ve basamaklar (her biri bir öncekine `gap`'ten yakın; en
    // yükseği pencereyi küçük PN'lerin ötesine taşır).
    let rungs = 6;
    let step = gap - gap / 4;
    let mut tx = KamaNoiseWireProtector::new(H_EXT, &key(A2B_E0), &key(B2A_E0));
    let mut pns: Vec<u64> = (0..80).collect();
    pns.extend((1..=rungs).map(|k| 80 + k * step));
    assert!(
        80 + rungs * step > window + 80,
        "basamaklar pencereyi küçük PN'lerin ötesine taşımalı"
    );
    let mut pool = Vec::with_capacity(pns.len());
    for (i, &pn) in pns.iter().enumerate() {
        if pn == 40 {
            tx.install_epoch1_keys(&key(A2B_E1), &key(B2A_E1));
        }
        let epoch = u8::from(pn >= 40);
        let wire = tx
            .seal(&packet(i as u64), seal_args(pn, epoch))
            .expect("havuz mühürlenir")
            .bytes;
        pool.push((pn, wire));
    }

    let mut old_rx = KamaNoiseWireProtector::new(H_EXT, &key(B2A_E0), &key(A2B_E0));
    old_rx.install_epoch1_keys(&key(B2A_E1), &key(A2B_E1));
    let (mut new_rx, h) = endpoint(B2A_E0, A2B_E0, extended());
    assert!(new_rx.install_epoch1_for_test(h, &key(B2A_E1), &key(A2B_E1)));

    // Sabit tohumlu LCG (Knuth MMIX sabitleri) — yalnız teslim sırası için.
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        x >> 33
    };
    let steps = 1200;
    let mut accepted = 0usize;
    // Görülen ret sınıfları (`WireError` `Clone` değil; Debug biçimi sınıfı tam adlandırır).
    let mut seen: Vec<String> = Vec::new();
    for step_no in 0..steps {
        if step_no == steps / 2 {
            // Grace kapanışı iki alıcıda aynı anda.
            old_rx.discard_epoch0_recv();
            assert!(new_rx.discard_epoch0_recv_for_test(h));
        }
        let r = next();
        let (pn, wire) = &pool[usize::try_from(r).unwrap() % pool.len()];
        let datagram = if r % 5 == 0 {
            tamper(wire)
        } else {
            wire.clone()
        };

        let typed = new_rx.inspect_open_for_test(&datagram);
        let old_result = old_rx.open(&datagram);
        let new_result = new_rx.open(&datagram);
        match (&old_result, &new_result) {
            (Ok(o), Some(n)) => {
                assert_eq!(
                    (n.meta.full_wire_pn, n.meta.dh_epoch),
                    (o.full_pn(), o.dh_epoch()),
                    "adım {step_no} (pn {pn}): kabul edilen PN/epoch ayrıştı"
                );
                assert!(
                    typed.is_ok(),
                    "adım {step_no}: saf aşama ret dedi, open kabul"
                );
                accepted += 1;
            }
            (Err(e), None) => {
                assert_eq!(
                    typed.as_ref().err(),
                    Some(e),
                    "adım {step_no} (pn {pn}): ret gerekçesi ayrıştı"
                );
                let class = format!("{e:?}");
                if !seen.contains(&class) {
                    seen.push(class);
                }
            }
            _ => panic!(
                "adım {step_no} (pn {pn}): kararlar ayrıştı — mevcut {old_result:?}, yeni {:?}",
                new_result.as_ref().map(|o| o.meta.full_wire_pn)
            ),
        }
        assert_eq!(
            new_rx.recv_expected_pn_for_test(h),
            Some(old_rx.recv_expected_pn()),
            "adım {step_no}: pencereler ayrıştı"
        );
    }

    assert!(accepted > 0, "hiç kabul yok (vakum)");
    for class in [
        WireError::PnReject(PnReject::Duplicate),
        WireError::PnReject(PnReject::TooOld),
        WireError::PnReject(PnReject::FutureGap),
        WireError::AeadFail,
        WireError::EpochNotAccepted(0),
    ] {
        let class = format!("{class:?}");
        assert!(
            seen.contains(&class),
            "{class} sınıfı dizide hiç oluşmadı — sınama o yolu örtmüyor (görülen: {seen:?})"
        );
    }
}

// ── S6: gönderim sayacı tavanı ───────────────────────────────────────────────────────────────

/// Tavan = ccr §35.8 REKEY_THRESHOLD; son PN mühürlenir, sonra `CapReached` (sayaç tüketilmez,
/// sarmaz) ve YALNIZ ilk seferde `RenewRequired(h)`. İki profilde.
#[test]
fn s6_send_pn_cap_is_rekey_threshold_and_signals_renew_once() {
    assert_eq!(
        SEND_PN_CAP,
        1u64 << 31,
        "S6: SEND_PN_CAP = ccr §35.8 REKEY_THRESHOLD (2^31)"
    );
    for profile in [legacy(LegacyDirection::AToB), extended()] {
        let (mut p, h) = endpoint(A2B_E0, B2A_E0, profile);
        let ctx = ProtectionContext::Session(h);
        assert!(p.set_next_send_pn_for_test(h, SEND_PN_CAP - 1));
        assert_eq!(
            p.seal(ctx, b"son").unwrap().meta.full_wire_pn,
            SEND_PN_CAP - 1
        );
        assert_eq!(p.take_event_for_test(), None, "tavanın altında olay yok");

        assert_eq!(p.seal(ctx, b"tavan").unwrap_err(), SealError::CapReached);
        assert_eq!(
            p.take_event_for_test(),
            Some(SessionEvent::RenewRequired(h))
        );
        assert_eq!(p.take_event_for_test(), None);
        let after_first = p.state_fingerprint();

        assert_eq!(p.seal(ctx, b"tavan").unwrap_err(), SealError::CapReached);
        assert_eq!(p.take_event_for_test(), None, "RenewRequired tek sefer");
        assert_eq!(
            p.state_fingerprint(),
            after_first,
            "ikinci ret state'i değiştirdi"
        );
        assert!(
            !p.set_next_send_pn_for_test(h, SEND_PN_CAP + 1),
            "sayaç tavanın ötesine taşınamaz"
        );
    }
}

/// `close`: bekleyen `RenewRequired` düşer, oturum mühürlemez, wire tutamacı demux'tan çıkar
/// (kapanmış oturumun datagramı `None` — HubChannel'a da düşmez).
#[test]
fn close_drops_pending_renew_and_demux_entry() {
    let (mut a, h) = endpoint(A2B_E0, B2A_E0, extended());
    let (mut b, hb) = endpoint(B2A_E0, A2B_E0, extended());
    let ctx = ProtectionContext::Session(h);
    let w = a.seal(ctx, b"bir kez").unwrap().bytes;
    assert!(a.set_next_send_pn_for_test(h, SEND_PN_CAP));
    assert_eq!(a.seal(ctx, b"x").unwrap_err(), SealError::CapReached);
    assert!(a.close_for_test(h));
    assert_eq!(
        a.take_event_for_test(),
        None,
        "close bekleyen RenewRequired'ı düşürmeli"
    );
    assert_eq!(a.seal(ctx, b"x").unwrap_err(), SealError::UnknownSession);
    assert!(!a.close_for_test(h), "kapanmış oturum ikinci kez kapanmaz");

    assert!(b.close_for_test(hb));
    assert!(b.open(&w).is_none(), "kapanmış oturumun datagramı açıldı");
}

/// `retire_send`: gönderim yarısı emekli (`seal`/`sealed_len` → `UnknownSession`), alım sürer.
#[test]
fn retired_send_half_keeps_receiving() {
    let (mut a, h) = endpoint(A2B_E0, B2A_E0, legacy(LegacyDirection::AToB));
    let (mut b, hb) = endpoint(B2A_E0, A2B_E0, legacy(LegacyDirection::BToA));
    let from_b = b
        .seal(ProtectionContext::Session(hb), b"b'den")
        .unwrap()
        .bytes;
    assert!(a.retire_send_for_test(h));
    let ctx = ProtectionContext::Session(h);
    assert_eq!(a.seal(ctx, b"x").unwrap_err(), SealError::UnknownSession);
    assert_eq!(a.sealed_len(ctx, 1), Err(SealError::UnknownSession));
    assert_eq!(
        a.open(&from_b).map(|o| o.frame),
        Some(b"b'den".to_vec()),
        "emekli gönderim yarısı alımı kesti"
    );
}

// ── sealed_len ───────────────────────────────────────────────────────────────────────────────

/// `seal(ctx, f).len() == sealed_len(ctx, f.len())` her profilde ve HubChannel'da; sınırın bir
/// üstü iki yolda da `FrameTooLarge` ve state değişmez; bilinmeyen oturumda sıra `seal` ile aynı
/// (sınıf minimumunun üstü `FrameTooLarge`, içi `UnknownSession`).
#[test]
fn sealed_len_is_exact_and_errors_match_seal() {
    for profile in [legacy(LegacyDirection::AToB), extended()] {
        let (mut p, h) = endpoint(A2B_E0, B2A_E0, profile);
        for ctx in [ProtectionContext::Session(h), ProtectionContext::HubChannel] {
            let max = usize::try_from(p.limits(ctx).max_plaintext_bytes).unwrap();
            for n in [0, 1, 100, max] {
                let unit = p.seal(ctx, &vec![0x5A; n]).expect("sınır içinde seal");
                assert_eq!(p.sealed_len(ctx, n), Ok(unit.bytes.len()), "{ctx:?} n={n}");
                assert_eq!(
                    usize::try_from(unit.meta.sealed_len).unwrap(),
                    unit.bytes.len()
                );
            }
            let before = p.state_fingerprint();
            assert_eq!(p.sealed_len(ctx, max + 1), Err(SealError::FrameTooLarge));
            assert_eq!(
                p.seal(ctx, &vec![0; max + 1]).unwrap_err(),
                SealError::FrameTooLarge
            );
            assert_eq!(
                p.state_fingerprint(),
                before,
                "{ctx:?}: ret state'i değiştirdi"
            );
        }
        let unknown = ProtectionContext::Session(SessionHandle::from_raw(0x0BAD));
        let floor = usize::try_from(
            p.conservative_limits(ProtectionClass::Session)
                .max_plaintext_bytes,
        )
        .unwrap();
        assert_eq!(p.sealed_len(unknown, 1), Err(SealError::UnknownSession));
        assert_eq!(
            p.seal(unknown, &[0]).unwrap_err(),
            SealError::UnknownSession
        );
        assert_eq!(
            p.sealed_len(unknown, floor + 1),
            Err(SealError::FrameTooLarge)
        );
        assert_eq!(
            p.seal(unknown, &vec![0; floor + 1]).unwrap_err(),
            SealError::FrameTooLarge
        );
    }
}

// ── Replay ───────────────────────────────────────────────────────────────────────────────────

/// `PN = u64::MAX` her genişlikte reddedilir (pn.rs'in kani-ispatlı formu); precheck saftır.
#[test]
fn replay_rejects_u64_max() {
    for params in [LEGACY_PN_PARAMS, EXTENDED_PN_PARAMS] {
        let w = DirectionalReplay::new(params);
        assert!(
            w.precheck(u64::MAX).is_err(),
            "{params:?}: u64::MAX kabul edildi"
        );
        assert_eq!(w.expected_pn(), 0);
    }
}
