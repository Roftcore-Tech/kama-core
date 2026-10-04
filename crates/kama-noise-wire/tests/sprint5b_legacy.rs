//! Sprint-5b: N1-legacy-interop (ccr §35.6/§35.9 AKTİF-format; mock-key). Legacy 22B codec + AAD-golden +
//! datagram-golden + DR-3-legacy-sabit-seti(extended'ten-AYRI) + mixed-traffic-demux + cross-direction-replay.

use kama_noise_wire::pn::{EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};
use kama_noise_wire::{
    build_legacy_aad, HandleClass, HandleRouter, KamaNoiseWireProtector, LegacyDirection,
    LegacySessionProtector, SealArgs, WireError, LEGACY_AAD_LEN, LEGACY_OVERHEAD,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use kama_vectors::hex;
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
        hex("53534431010001010001000533333333333333333333333333333333").unwrap(),
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
        hex("aabbccdd0005bcd0644447bfb57076ca5f1cf2c399df9e1753118671bd7a0e3ef2f0a1a75d09866a752a2bf1fd")
            .unwrap(),
        "legacy wire-golden drift-gate"
    );
}

// ── DR-3: legacy iki-sabit-seti extended'ten AYRI (WINDOW/FUTURE_GAP/REORDER) ──
#[test]
fn legacy_constants_separate_from_extended() {
    assert_eq!(LEGACY_PN_PARAMS.window, 1024);
    // [D-GAP-1] legacy'de far-future kapısı YOK (MAX_FUTURE_GAP=4096 ve MAX_REORDER_DISTANCE=8192 kaldırıldı).
    assert_eq!(LEGACY_PN_PARAMS.future_gap_max, None);
    assert_eq!(LEGACY_PN_PARAMS.low_width_bits, 16);
    // Extended AYRI: 2^14 / 2^13 (kapı durur) / 32-bit.
    assert_eq!(EXTENDED_PN_PARAMS.window, 1 << 14);
    assert_eq!(EXTENDED_PN_PARAMS.future_gap_max, Some(1 << 13));
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

// ── [D-GAP-1] ccr §35.8: AEAD-geçerli far-future paket KABUL edilir (uçtan uca, gerçek AEAD) ──
#[test]
fn legacy_far_future_aead_valid_is_accepted() {
    let (mut a, mut b) = legacy_pair(0x0000_3000);
    assert_eq!(b.open(&a.seal(b"first", 0).unwrap()).unwrap(), b"first");
    // Yolda geciken eski bir paket (gönderen monoton mühürler; teslim sonra denenecek).
    let stale = a.seal(b"stale", 1).unwrap();

    // İleri yön kesintisi: gönderen 10 000 sayaç yaktı, alıcıya hiçbiri ulaşmadı. Eskiden bu paket
    // FutureGap (4096) ile precheck'te düşüyordu ve resync yolu yoktu → oturum kalıcı ölüyordu.
    let far = a.seal(b"after-outage", 10_000).unwrap();
    assert_eq!(b.open(&far).unwrap(), b"after-outage");
    assert_eq!(b.recv_expected_pn(), 10_001, "highest sıçramalı");

    // Sıçranan paket replay edilemez; sıçramadan önceki sayaç artık çok eski.
    assert!(matches!(
        b.open(&far),
        Err(WireError::PnReject(kama_noise_wire::PnReject::Duplicate))
    ));
    assert!(matches!(
        b.open(&stale),
        Err(WireError::PnReject(kama_noise_wire::PnReject::TooOld))
    ));
}

// ── [D-GAP-1] YAPISAL SINIR: gerçek sayaç expected'ten 2^15'ten uzaksa seq16 yanlış kurulur ──
#[test]
fn legacy_beyond_structural_limit_fails_aead_without_advancing_state() {
    let (mut a, mut b) = legacy_pair(0x0000_3001);
    // Alıcıyı 100 001'e, her adımı 2^15'in ALTINDA tutarak yürüt (eski 4096/8192 kapılarının çok üstü).
    for pn in [0u64, 30_000, 60_000, 90_000, 100_000] {
        assert_eq!(b.open(&a.seal(b"x", pn).unwrap()).unwrap(), b"x");
    }
    let before = b.recv_expected_pn();
    assert_eq!(before, 100_001);

    // 100 001'den 40 000 ileri (> 2^15): reconstruction en yakın adayı seçer → YANLIŞ sayaç
    // (gerçek − 2^16 = 74 465, yani GERİDE). Sonuç sessiz kayıptır; hangi kuralın düşürdüğü yanlış
    // adayın nereye düştüğüne bağlıdır: pencere dışındaysa TooOld (burada öyle), pencere içindeyse
    // nonce uyuşmadığı için AeadFail. İkisinde de replay state'i İLERLEMEZ (P0-6).
    // Bunu meşru trafikte önlemek gönderenin işidir (KS-PN-1, K = 1024 ≪ 2^15).
    let too_far = a.seal(b"y", before + 40_000).unwrap();
    assert!(matches!(
        b.open(&too_far),
        Err(WireError::PnReject(kama_noise_wire::PnReject::TooOld))
    ));
    // Yanlış adayın PENCERE İÇİNE düştüğü uzaklık: 2^16 − 500 ileri → aday 500 geride → AEAD denenir, düşer.
    let in_window = a.seal(b"z", before + 65_536 - 500).unwrap();
    assert!(matches!(b.open(&in_window), Err(WireError::AeadFail)));
    assert_eq!(
        b.recv_expected_pn(),
        before,
        "AEAD başarısızlığı state'i ilerletmemeli"
    );
}
