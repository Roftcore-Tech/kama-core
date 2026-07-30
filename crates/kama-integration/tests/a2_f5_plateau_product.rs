//! A2-F5 — PLATEAU (F-A/F-B/F-C) + PRODUCT W-3-65 RERUN (pre-registration sha256=32ada1ad…, FROZEN).
//! Deterministik-birim (gerçek ReceiverHost + Sender). Beklentiler koşudan-ÖNCE donduruldu.

#![cfg(feature = "experimental-cold-start")]

use kama_integration::{IngestOutcome, ReceiverHost};
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const H: u32 = 0x0000_0077;
const GLOBAL_COLD_BYTE_CAP: u64 = 15 * 1024 * 1024;
const PER_SESSION_CAP: u64 = 480 * 1024;

fn key() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0x33u8; 32])
}
fn snd(h: u32) -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(h, &key(), &key())
}
fn seal_source(s: &mut KamaNoiseWireProtector, block: u32, esi: u16, pn: u64) -> Vec<u8> {
    let pfx = SourcePrefix {
        message_id: 1,
        chunk_offset: 0,
        actual_payload_len: 10,
        source_flags: 0,
    };
    let mut sym = vec![0u8; 64];
    pfx.serialize_symbol(&[0xAB; 10], &mut sym).unwrap();
    s.seal(
        &UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Source,
                profile_id: 0x03,
                block_id: block,
                esi,
                k_actual: 8,
            },
            symbol: sym,
        },
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}
fn seal_probe(s: &mut KamaNoiseWireProtector, seq: u16, pn: u64) -> Vec<u8> {
    s.seal(
        &UnsealedTransportPacket::Control(Control::CsProbe {
            id: 0,
            subtype: 0x01,
            probe_seq: seq,
        }),
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}
fn seal_close(s: &mut KamaNoiseWireProtector, id: u32, pn: u64) -> Vec<u8> {
    s.seal(
        &UnsealedTransportPacket::Control(Control::SessionClose { id, reason_code: 0 }),
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

/// F-A PROBE-FLOOD: probe cold-deftere DOKUNMAZ; working-set sabit (cs_echo, retained=0).
#[test]
fn f5_fa_probe_flood_zero_cold_delta() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H, &key()).unwrap();
    let mut s = snd(H);
    for seq in 0..2000u16 {
        let dg = seal_probe(&mut s, seq, u64::from(seq));
        assert_eq!(
            host.on_datagram(0, &dg).unwrap(),
            IngestOutcome::ProbeEchoed
        );
    }
    assert_eq!(host.cs_echo_count(), 2000);
    assert_eq!(
        host.global_cold().res.retained_bytes,
        0,
        "probe cold-deftere dokundu"
    );
}

/// F-B COLD-BLOCK-FLOOD: incomplete-blok seli → OPEN_BLOCK-cap-reject; retained ≤ per-session + global.
#[test]
fn f5_fb_cold_block_flood_bounded_and_typed_reject() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H, &key()).unwrap();
    let mut s = snd(H);
    let mut rejects = 0u32;
    // OPEN_BLOCK_CAP=8; 40 farklı-blok (her-biri 1-shard) → 8 kabul, kalan cap-reject.
    for block in 0..40u32 {
        let dg = seal_source(&mut s, block, 0, u64::from(block));
        match host.on_datagram(0, &dg).unwrap() {
            IngestOutcome::ShardAdmitted { .. } => {}
            IngestOutcome::Dropped("cold cap exceeded") => rejects += 1,
            other => panic!("beklenmeyen: {other:?}"),
        }
    }
    assert!(rejects > 0, "cap-aşımı typed-reject üretmedi");
    assert!(
        host.global_cold().res.retained_bytes <= PER_SESSION_CAP,
        "session cold-retained per-session cap'i aştı"
    );
    assert!(host.global_cold().res.retained_bytes <= GLOBAL_COLD_BYTE_CAP);
    // Plateau: daha-fazla-blok cap-dolu → retained SABİT (büyüme durur).
    let plateau = host.global_cold().res.retained_bytes;
    for block in 40..60u32 {
        let dg = seal_source(&mut s, block, 0, 1000 + u64::from(block));
        let _ = host.on_datagram(0, &dg);
    }
    assert_eq!(
        host.global_cold().res.retained_bytes,
        plateau,
        "plateau bozuldu (büyüme sürdü)"
    );
}

/// F-C CONTROL-FLOOD: farklı+geri control-id → dedup-bitmap O(1); below-window görünür.
#[test]
fn f5_fc_control_flood_bounded_dedup() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H, &key()).unwrap();
    let mut s = snd(H);
    // Wire-PN GLOBAL ardışık-monoton (receiver replay-window içinde kalır); control-id AYRI eksen.
    let mut pn = 0u64;
    // İleri 3000 control-id (ilerleyen) → Fresh.
    for id in 0..3000u32 {
        let dg = seal_close(&mut s, id, pn);
        pn += 1;
        assert!(matches!(
            host.on_datagram(0, &dg).unwrap(),
            IngestOutcome::ControlFresh { .. } | IngestOutcome::ControlDuplicate
        ));
    }
    // high-water'ı 10000'e taşı → 0..5904 window-altına düşer.
    let dg = seal_close(&mut s, 10000, pn);
    pn += 1;
    assert!(matches!(
        host.on_datagram(0, &dg).unwrap(),
        IngestOutcome::ControlFresh { .. }
    ));
    // Geri-eski control-id=1 (window-altı) → ControlDuplicate (bitmap sabit; taşma yok).
    let dg_old = seal_close(&mut s, 1, pn);
    assert_eq!(
        host.on_datagram(0, &dg_old).unwrap(),
        IngestOutcome::ControlDuplicate,
        "window-altı Fresh oldu (bitmap taşması)"
    );
}

/// PRODUCT W-3-65 (65B-sınıfı): tam ingest→delivery→release zinciri gerçek ReceiverHost'ta (3 tekrar).
#[test]
fn f5_product_w365_delivery_repeatable() {
    for rep in 0..3u32 {
        let h = H + rep;
        let mut host = ReceiverHost::new();
        host.install_session_with_keys(h, &key()).unwrap();
        let mut s = snd(h);
        let mut last = IngestOutcome::Dropped("init");
        for esi in 0..8u16 {
            let dg = seal_source(&mut s, 0, esi, u64::from(esi));
            last = host.on_datagram(1000, &dg).unwrap();
        }
        assert!(
            matches!(last, IngestOutcome::BlockDelivered { block_id: 0, .. }),
            "rep {rep}: delivery zinciri kırıldı"
        );
        assert_eq!(host.delivered_count(), 1);
        assert_eq!(
            host.global_cold().res.retained_bytes,
            0,
            "rep {rep}: teslim-sonrası release sızdı"
        );
    }
}

/// PRODUCT CAP-STRESS: OPEN_BLOCK-cap üstü → typed-reject + bounded (pre-reg §2 cap-stress).
#[test]
fn f5_product_cap_stress_typed_bounded() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H, &key()).unwrap();
    let mut s = snd(H);
    let mut admitted = 0u32;
    let mut rejected = 0u32;
    for block in 0..20u32 {
        let dg = seal_source(&mut s, block, 0, u64::from(block));
        match host.on_datagram(0, &dg).unwrap() {
            IngestOutcome::ShardAdmitted { .. } => admitted += 1,
            IngestOutcome::Dropped("cold cap exceeded") => rejected += 1,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(admitted, 8, "OPEN_BLOCK_CAP=8 kadar kabul olmalı");
    assert_eq!(rejected, 12, "cap-üstü typed-reject");
    assert!(host.global_cold().res.retained_bytes <= PER_SESSION_CAP);
}
