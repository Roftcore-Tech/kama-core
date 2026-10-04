//! Sprint-1 çekirdek testleri (noise-wire §2.5/§2.6/§2.8/§4/§6/§0.5):
//! outer/AAD golden · reject-matrisi · PN reconstruct/accept · seal/open roundtrip ·
//! sizer eşitliği (seal.len==sealed_len) · epoch-ayrımı · AEAD-fail-window-ilerlemez.

use kama_noise_wire::inner::{InnerPlain, FEC_INNER_HDR_LEN};
use kama_noise_wire::outer::{ExtendedOuter, FLAG_DH_EPOCH, OUTER_HDR_LEN, TAG_LEN};
use kama_noise_wire::pn::{reconstruct, ReplayWindow, EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};
use kama_noise_wire::{
    KamaNoiseWireProtector, KamaNoiseWireSizer, PacketClass, PnReject, SealArgs, WireError,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket, EXTENDED_OUTER_OVERHEAD};
use zeroize::Zeroizing;

fn key(seed: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([seed; 32])
}

fn mk_protector(h_ext: u32) -> KamaNoiseWireProtector {
    // send/recv anahtarları ayrı seed (tek-taraf testte kendi-kendine open için send==recv gerekir;
    // burada iki-protector kullanacağız — A.send == B.recv).
    KamaNoiseWireProtector::new(h_ext, &key(0x11), &key(0x22))
}

/// Bir yön için eşleşen çift: A gönderir (send=k1), B alır (recv=k1).
fn paired(h_ext: u32) -> (KamaNoiseWireProtector, KamaNoiseWireProtector) {
    let a = KamaNoiseWireProtector::new(h_ext, &key(0xAA), &key(0xBB));
    let b = KamaNoiseWireProtector::new(h_ext, &key(0xBB), &key(0xAA));
    (a, b)
}

fn fec_packet(block_id: u32, esi: u16, k_actual: u8, e: usize) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id,
            esi,
            k_actual,
        },
        symbol: vec![0x5Au8; e],
    }
}

// ── §2.8 AAD golden ──
#[test]
fn aad_golden_bytes() {
    let outer = ExtendedOuter {
        h_ext: 0x0102_0304,
        dh_epoch: 1,
        pn_trunc: 0xAABB_CCDD,
    };
    let aad = kama_noise_wire::aad::build_aad(&outer);
    let mut expected = Vec::new();
    expected.extend_from_slice(b"KAMA-WIRE-EXT-V1");
    expected.extend_from_slice(&0x0102_0304u32.to_be_bytes());
    expected.push(0x01); // wire_format EXTENDED_V1
    expected.push(0x01); // wire_flags bit0=dh_epoch=1
    expected.extend_from_slice(&0xAABB_CCDDu32.to_be_bytes());
    assert_eq!(aad.as_slice(), expected.as_slice());
    assert_eq!(aad.len(), 26);
}

// ── §2.6 outer serialize/parse roundtrip + reject-matrisi ──
#[test]
fn outer_serialize_parse_roundtrip() {
    let outer = ExtendedOuter {
        h_ext: 0xDEAD_BEEF,
        dh_epoch: 0,
        pn_trunc: 0x0000_1234,
    };
    let hdr = outer.serialize_hdr();
    // ct+tag ekle (26B minimum): 0 ct + 16 tag
    let mut dg = hdr.to_vec();
    dg.extend_from_slice(&[0u8; TAG_LEN]);
    let (parsed, ct, tag) = ExtendedOuter::parse(&dg).unwrap();
    assert_eq!(parsed, outer);
    assert_eq!(ct.len(), 0);
    assert_eq!(tag.len(), TAG_LEN);
}

#[test]
fn outer_reject_short_datagram() {
    let dg = [0u8; EXTENDED_OUTER_OVERHEAD - 1];
    assert!(matches!(
        ExtendedOuter::parse(&dg),
        Err(WireError::DatagramTooShort { .. })
    ));
}

#[test]
fn outer_reject_unknown_wire_format() {
    let mut dg = vec![0u8; EXTENDED_OUTER_OVERHEAD];
    dg[4] = 0x02; // EXTENDED_V1 değil
    assert!(matches!(
        ExtendedOuter::parse(&dg),
        Err(WireError::UnknownWireFormat(0x02))
    ));
}

#[test]
fn outer_reject_reserved_flags() {
    let mut dg = vec![0u8; EXTENDED_OUTER_OVERHEAD];
    dg[4] = 0x01;
    dg[5] = 0b0000_0010; // bit1 reserved set
    assert!(matches!(
        ExtendedOuter::parse(&dg),
        Err(WireError::ReservedWireFlags(_))
    ));
}

// ── §4 reconstruct: en-yakın-aday, wrap, underflow ──
#[test]
fn pn_reconstruct_basic_and_wrap() {
    let p = &EXTENDED_PN_PARAMS; // 32-bit
                                 // expected küçük, low_bits aynı → aynı.
    assert_eq!(reconstruct(p, 100, 100), 100);
    // expected büyük tabanda, low_bits ileri
    let expected = (3u64 << 32) + 50;
    assert_eq!(reconstruct(p, expected, 60), (3u64 << 32) + 60);
    // wrap: expected span-sınırında, low_bits küçük → bir-üst-tabana
    let near = (1u64 << 32) - 10;
    assert_eq!(reconstruct(p, near, 5), 1u64 << 32 | 5);
    // underflow-guard: center küçük, cand>center+half ama cand<span → çıkarma YOK
    assert_eq!(reconstruct(p, 5, u64::from(u32::MAX)), u64::from(u32::MAX));
}

// ── §4 accept: precheck + commit, too-old/dup/future ──
#[test]
fn pn_window_accept_sequence() {
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    // İlk paket pn=0
    assert!(w.precheck(0).is_ok());
    w.commit(0);
    assert_eq!(w.expected_pn(), 1);
    // İleri pn=5
    assert!(w.precheck(5).is_ok());
    w.commit(5);
    assert_eq!(w.expected_pn(), 6);
    // Geç ama pencere-içi pn=3 (ilk-görülüş) kabul
    assert!(w.precheck(3).is_ok());
    w.commit(3);
    // pn=3 tekrar → DUPLICATE
    assert_eq!(w.precheck(3), Err(PnReject::Duplicate));
    // pn=5 tekrar → DUPLICATE
    assert_eq!(w.precheck(5), Err(PnReject::Duplicate));
    // future-gap: expected=6, gap>2^13
    assert_eq!(w.precheck(6 + (1 << 13) + 1), Err(PnReject::FutureGap));
}

#[test]
fn pn_window_too_old() {
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    // expected'i WINDOW+100'e taşı
    let far = (1u64 << 14) + 100;
    w.commit(far);
    // candidate + WINDOW < expected → too-old
    assert_eq!(w.precheck(50), Err(PnReject::TooOld));
}

#[test]
fn legacy_params_reorder_bound() {
    // ccr-§35.8 seti [D-GAP-1 sonrası]: geriye doğru tek sınır eski-paket kuralı (window = 1024).
    // Eski MAX_REORDER_DISTANCE (8192) uzaklığı bu yüzden hâlâ TooOld — kapı kalktı, sonuç aynı.
    let mut w = ReplayWindow::new(LEGACY_PN_PARAMS);
    w.commit(20_000);
    assert_eq!(w.precheck(20_000 - 8193), Err(PnReject::TooOld));
    // İleri yönde kapı YOK.
    assert_eq!(w.precheck(20_000 + 8193), Ok(()));
}

// ── §0/§4 seal→open roundtrip (FEC-shard, epoch-0) ──
#[test]
fn seal_open_roundtrip_epoch0() {
    let (mut a, mut b) = paired(0x1234_5678);
    let pkt = fec_packet(7, 3, 8, 64);
    let sealed = a
        .seal(
            &pkt,
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert_eq!(sealed.meta.packet_class, PacketClass::Source);
    assert_eq!(sealed.meta.full_wire_pn, 0);
    let opened = b.open(&sealed.bytes).unwrap();
    assert_eq!(opened.full_pn(), 0);
    assert_eq!(opened.dh_epoch(), 0);
    match opened.inner() {
        InnerPlain::FecShard { header, symbol } => {
            assert_eq!(header.block_id, 7);
            assert_eq!(header.esi, 3);
            assert_eq!(header.k_actual, 8);
            assert_eq!(symbol.len(), 64);
        }
        InnerPlain::Control { .. } => panic!("FEC beklendi"),
    }
}

// ── §0.5 P0-3: seal(p).len() == sealed_len(p) ──
#[test]
fn sealed_len_exact_all_e() {
    let (mut a, _b) = paired(0x1);
    for (i, e) in [11usize, 32, 64, 512, 1024].into_iter().enumerate() {
        let pkt = fec_packet(1, 0, 1, e);
        let want = a.sealed_len(&pkt).unwrap();
        let got = a
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: i as u64,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(got.bytes.len(), want, "seal-len != sealed_len (E={e})");
        assert_eq!(want, EXTENDED_OUTER_OVERHEAD + FEC_INNER_HDR_LEN + e);
    }
}

// ── §4: AEAD-fail window-İLERLETMEZ ──
#[test]
fn aead_fail_does_not_advance_window() {
    let (mut a, mut b) = paired(0x9);
    let pkt = fec_packet(1, 0, 1, 64);
    let mut sealed = a
        .seal(
            &pkt,
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    let before = b.recv_expected_pn(0);
    // ciphertext'i boz → AEAD fail
    let n = sealed.bytes.len();
    sealed.bytes[n - 1] ^= 0xFF;
    assert!(matches!(b.open(&sealed.bytes), Err(WireError::AeadFail)));
    assert_eq!(
        b.recv_expected_pn(0),
        before,
        "window AEAD-fail'de ilerlememeli"
    );
    // Şimdi geçerli paket hâlâ pn=0'da kabul edilebilmeli
    let good = a.seal(
        &fec_packet(1, 1, 1, 64),
        SealArgs {
            wire_pn: 0,
            dh_epoch: 0,
            send_generation: 0,
        },
    );
    // pn=0 zaten seal-edildi (non-monotonic guard) → yeni pn kullan
    assert!(good.is_err());
    let good = a
        .seal(
            &fec_packet(1, 1, 1, 64),
            SealArgs {
                wire_pn: 1,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&good.bytes).is_ok());
}

// ── §4: replay (aynı datagram iki kez open) → ikinci DUPLICATE ──
#[test]
fn replay_same_datagram_rejected() {
    let (mut a, mut b) = paired(0x7);
    let s0 = a
        .seal(
            &fec_packet(1, 0, 1, 64),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&s0.bytes).is_ok());
    // Aynı datagram tekrar → PN precheck DUPLICATE
    assert!(matches!(
        b.open(&s0.bytes),
        Err(WireError::PnReject(PnReject::Duplicate))
    ));
}

// ── §4: epoch-0/1 ayrı-state, aynı-PN-değeri karışmaz ──
#[test]
fn epoch_isolation_same_pn() {
    let mut a = KamaNoiseWireProtector::new(0x5, &key(0xAA), &key(0xBB));
    let mut b = KamaNoiseWireProtector::new(0x5, &key(0xBB), &key(0xAA));
    a.install_epoch1_keys(&key(0xCC), &key(0xDD));
    b.install_epoch1_keys(&key(0xDD), &key(0xCC));
    // epoch-0 pn=0
    let s0 = a
        .seal(
            &fec_packet(1, 0, 1, 64),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    // epoch-1 pn=0 (ayrı cipher-state, aynı PN değeri)
    let s1 = a
        .seal(
            &fec_packet(2, 0, 1, 64),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 1,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&s0.bytes).is_ok());
    assert!(
        b.open(&s1.bytes).is_ok(),
        "epoch-1 pn=0 ayrı-state'te kabul"
    );
    assert_eq!(b.recv_expected_pn(0), 1);
    assert_eq!(b.recv_expected_pn(1), 1);
}

// ── §4: epoch-1 anahtarı yokken epoch-1 open reddi ──
#[test]
fn epoch1_not_installed_rejected() {
    let mut a = KamaNoiseWireProtector::new(0x5, &key(0xAA), &key(0xBB));
    a.install_epoch1_keys(&key(0xCC), &key(0xDD));
    let s1 = a
        .seal(
            &fec_packet(2, 0, 1, 64),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 1,
                send_generation: 0,
            },
        )
        .unwrap();
    // B epoch-1 kurmadı
    let mut b = KamaNoiseWireProtector::new(0x5, &key(0xBB), &key(0xAA));
    assert!(matches!(
        b.open(&s1.bytes),
        Err(WireError::EpochNotAccepted(1))
    ));
}

// ── nonce-reuse guard: aynı-PN ikinci-seal reddi ──
#[test]
fn nonce_reuse_guard() {
    let mut a = mk_protector(0x3);
    let _ = a
        .seal(
            &fec_packet(1, 0, 1, 64),
            SealArgs {
                wire_pn: 5,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    // aynı pn=5 → reddet
    assert!(matches!(
        a.seal(
            &fec_packet(1, 1, 1, 64),
            SealArgs {
                wire_pn: 5,
                dh_epoch: 0,
                send_generation: 0
            }
        ),
        Err(WireError::SealInput(_))
    ));
    // pn=4 (geri) → reddet
    assert!(a
        .seal(
            &fec_packet(1, 1, 1, 64),
            SealArgs {
                wire_pn: 4,
                dh_epoch: 0,
                send_generation: 0
            }
        )
        .is_err());
}

// ── h_ext mismatch open reddi ──
#[test]
fn h_ext_mismatch_rejected() {
    let mut a = KamaNoiseWireProtector::new(0x1111, &key(0xAA), &key(0xBB));
    let mut b = KamaNoiseWireProtector::new(0x2222, &key(0xBB), &key(0xAA));
    let s = a
        .seal(
            &fec_packet(1, 0, 1, 64),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(b.open(&s.bytes).is_err());
}

#[test]
fn flag_dh_epoch_mask_is_bit0() {
    assert_eq!(FLAG_DH_EPOCH, 1);
    assert_eq!(OUTER_HDR_LEN, 10);
}
