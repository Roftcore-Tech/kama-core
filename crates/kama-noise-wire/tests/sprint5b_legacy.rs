//! Sprint-5b: N1-legacy-interop (ccr §35.6/§35.9 AKTİF-format; mock-key). Legacy 22B codec + AAD-golden +
//! datagram-golden + DR-3-legacy-sabit-seti(extended'ten-AYRI) + mixed-traffic-demux + cross-direction-replay.

use kama_noise_wire::pn::{EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};
use kama_noise_wire::{
    build_legacy_aad, HandleClass, HandleRouter, KamaNoiseWireProtector, LegacyDirection,
    LegacySessionProtector, SealArgs, WireError, LEGACY_AAD_LEN, LEGACY_OVERHEAD,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn legacy_pair(hub_handle: u32) -> (LegacySessionProtector, LegacySessionProtector) {
    let a = LegacySessionProtector::new(
        hub_handle,
        &key(0x11),
        &key(0x22),
        0x01,
        0x0001,
        1,
        [0x33; 16],
        LegacyDirection::AToB,
    );
    // B: send=0x22 recv=0x11, send_direction=B→A.
    let b = LegacySessionProtector::new(
        hub_handle,
        &key(0x22),
        &key(0x11),
        0x01,
        0x0001,
        1,
        [0x33; 16],
        LegacyDirection::BToA,
    );
    (a, b)
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// ── §35.6 roundtrip: A→B seal → B open ──
#[test]
fn legacy_roundtrip() {
    let (mut a, mut b) = legacy_pair(0x0000_1000);
    let dg = a.seal(b"hot-path", 0).unwrap();
    // Wire: [hub_handle 4][seq16 2][ct][tag 16] = 22 + 8 = 30.
    assert_eq!(dg.len(), LEGACY_OVERHEAD + 8);
    assert_eq!(&dg[0..4], &0x0000_1000u32.to_be_bytes());
    assert_eq!(&dg[4..6], &0u16.to_be_bytes()); // seq16=0
    assert_eq!(b.open(&dg).unwrap(), b"hot-path");
}

// ── §35.9 AAD golden (28B, exact byte-layout) ──
#[test]
fn legacy_aad_golden() {
    let aad = build_legacy_aad(0x01, 0x0001, 0x01, 1, 5, &[0x33; 16]);
    assert_eq!(aad.len(), LEGACY_AAD_LEN);
    assert_eq!(
        aad.to_vec(),
        hex("53534431010001010001000533333333333333333333333333333333"),
        "SSD1 || ver || suite || dir || epoch || seq16 || session_id_internal"
    );
}

// ── KATMAN-4 legacy full-wire golden (45B pinned; LE64-nonce §4.5) ──
#[test]
fn legacy_datagram_golden() {
    let mut a = LegacySessionProtector::new(
        0xAABB_CCDD,
        &key(0x11),
        &key(0x22),
        0x01,
        0x0001,
        1,
        [0x33; 16],
        LegacyDirection::AToB,
    );
    let dg = a.seal(b"legacy hot-path payload", 5).unwrap();
    assert_eq!(
        dg,
        hex("aabbccdd0005bcd0644447bfb57076ca5f1cf2c399df9e1753118671bd7a0e3ef2f0a1a75d09866a752a2bf1fd"),
        "legacy wire-golden drift-gate"
    );
}

// ── DR-3: legacy iki-sabit-seti extended'ten AYRI (WINDOW/FUTURE_GAP/REORDER) ──
#[test]
fn legacy_constants_separate_from_extended() {
    assert_eq!(LEGACY_PN_PARAMS.window, 1024);
    assert_eq!(LEGACY_PN_PARAMS.future_gap_max, 4096);
    assert_eq!(LEGACY_PN_PARAMS.max_reorder, Some(8192));
    assert_eq!(LEGACY_PN_PARAMS.low_width_bits, 16);
    // Extended AYRI: 2^14 / 2^13 / reorder-yok / 32-bit.
    assert_eq!(EXTENDED_PN_PARAMS.window, 1 << 14);
    assert_eq!(EXTENDED_PN_PARAMS.future_gap_max, 1 << 13);
    assert_eq!(EXTENDED_PN_PARAMS.max_reorder, None);
    assert_eq!(EXTENDED_PN_PARAMS.low_width_bits, 32);
    // İki-yol-iki-sabit: eşit-DEĞİL.
    assert_ne!(LEGACY_PN_PARAMS.window, EXTENDED_PN_PARAMS.window);
}

// ── Legacy reorder legacy-window-içi + replay-reddi ──
#[test]
fn legacy_reorder_and_replay() {
    let (mut a, mut b) = legacy_pair(0x0000_2000);
    let d0 = a.seal(b"m0", 0).unwrap();
    let d1 = a.seal(b"m1", 1).unwrap();
    let d2 = a.seal(b"m2", 2).unwrap();
    // Reorder: 2, 0, 1.
    assert_eq!(b.open(&d2).unwrap(), b"m2");
    assert_eq!(b.open(&d0).unwrap(), b"m0"); // legacy-window-içi geç-paket
    assert_eq!(b.open(&d1).unwrap(), b"m1");
    // Replay d2 → DUPLICATE.
    assert!(matches!(b.open(&d2), Err(WireError::PnReject(_))));
}

// ── Cross-direction: yanlış-yön AAD → AEAD-fail (§35.9 direction-replay-engeli) ──
#[test]
fn legacy_cross_direction_rejected() {
    // A→B seal; ama B-yönünde-A-yönü-AAD-beklemez. Aynı-yönlü-iki-A ise open-edemez (direction-mismatch).
    let mut a = LegacySessionProtector::new(
        0x0000_3000,
        &key(0x11),
        &key(0x22),
        0x01,
        0x0001,
        1,
        [0x33; 16],
        LegacyDirection::AToB,
    );
    // Yanlış: alıcı da AToB-send (→ recv-dir = BToA), ama gönderen AToB → AAD-direction uyuşmaz.
    let mut wrong = LegacySessionProtector::new(
        0x0000_3000,
        &key(0x22),
        &key(0x11),
        0x01,
        0x0001,
        1,
        [0x33; 16],
        LegacyDirection::AToB,
    );
    let dg = a.seal(b"x", 0).unwrap();
    // wrong.recv_dir = AToB.opposite = BToA; ama datagram AToB(0x01)-AAD'yle mühürlü → mismatch → AEAD-fail.
    assert!(matches!(wrong.open(&dg), Err(WireError::AeadFail)));
}

// ── MIXED-TRAFFIC DEMUX: h_legacy→Legacy, h_ext→Extended; cross-handle-karışma-YOK ──
#[test]
fn mixed_traffic_demux() {
    let h_legacy = 0x0000_4000u32;
    let h_ext = 0x0000_4001u32;
    let mut router = HandleRouter::new();
    router.register_legacy(h_legacy);
    router.register_extended(h_ext);

    // Legacy datagram.
    let mut leg = LegacySessionProtector::new(
        h_legacy,
        &key(0x11),
        &key(0x22),
        0x01,
        0x0001,
        1,
        [0x33; 16],
        LegacyDirection::AToB,
    );
    let leg_dg = leg.seal(b"data", 0).unwrap();

    // Extended datagram.
    let mut ext = KamaNoiseWireProtector::new(h_ext, &key(0xAA), &key(0xBB));
    let fec = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 3,
            block_id: 1,
            esi: 0,
            k_actual: 1,
        },
        symbol: vec![0u8; 64],
    };
    let ext_dg = ext
        .seal(
            &fec,
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap()
        .bytes;

    // DEMUX: handle-uzayından doğru-parser (decrypt-öncesi).
    assert_eq!(router.classify(&leg_dg), Some(HandleClass::Legacy));
    assert_eq!(router.classify(&ext_dg), Some(HandleClass::Extended));
    // Bilinmeyen-handle → None (drop).
    let mut unknown = ext_dg.clone();
    unknown[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
    assert_eq!(router.classify(&unknown), None);

    // Cross-handle-karışma-YOK: legacy-datagram extended-parser'a verilirse → hata (h_ext-mismatch/format).
    assert!(
        ext.open(&leg_dg).is_err(),
        "legacy-datagram extended-parser'da açılamaz"
    );
}
