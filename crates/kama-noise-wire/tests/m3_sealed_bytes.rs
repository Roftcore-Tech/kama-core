//! M3 — GERÇEK-sealed-byte tablosu (tahmin-değil; sealed_len + seal byte-eş assert'li).
//! PMTU-gate-kanıtı: tüm-class'lar ≤1200B. `--ignored --nocapture` tablo-yazdırır;
//! normal-modda assert'ler koşar (m3_pmtu_all_classes).

use kama_noise_wire::{
    KamaNoiseWireProtector, KamaNoiseWireSizer, SealArgs, RECOVERED_MAX_ESI_RANGES,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn fec_pkt(e: usize, kind: ShardKind, profile_id: u8) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind,
            profile_id,
            block_id: 1,
            esi: 0,
            k_actual: 8,
        },
        symbol: vec![0x5A; e],
    }
}

fn cases() -> Vec<(&'static str, UnsealedTransportPacket)> {
    vec![
        ("FecShard-Source-E64", fec_pkt(64, ShardKind::Source, 0x03)),
        ("FecShard-Repair-E64", fec_pkt(64, ShardKind::Repair, 0x03)),
        (
            "FecShard-Source-E1024",
            fec_pkt(1024, ShardKind::Source, 0x05),
        ),
        (
            "FecShard-Repair-E1024",
            fec_pkt(1024, ShardKind::Repair, 0x05),
        ),
        // [D2 2026-10-08] ~~Control-Ack-4range~~ kaldırıldı — Ack ailesi kodda yok (K3). Yerine en
        // büyük meşru control gövdesi: DR-1 tavanında RECOVERED (16 aralık; boyut sealed_len'den).
        (
            "Control-Recovered-16range",
            UnsealedTransportPacket::Control(Control::Recovered {
                id: 1,
                block_id: 7,
                esi_ranges: (0..RECOVERED_MAX_ESI_RANGES as u16)
                    .map(|i| (i, i))
                    .collect(),
            }),
        ),
        (
            "Control-ParamSet",
            UnsealedTransportPacket::Control(Control::ParamSet {
                id: 2,
                param_hash: [0xAB; 32],
                profile_id: 3,
                code_suite_id: 1,
            }),
        ),
        (
            "Control-Recovered-2range",
            UnsealedTransportPacket::Control(Control::Recovered {
                id: 3,
                block_id: 7,
                esi_ranges: vec![(1, 1), (3, 3)],
            }),
        ),
    ]
}

/// Tablo (ölçüm; gerçek-seal byte-eş doğrulamalı).
#[test]
#[ignore]
fn m3_sealed_byte_table() {
    let mut tx = KamaNoiseWireProtector::new(0x1, &key(0xA1), &key(0xB2));
    println!("class,sealed_bytes,payload_bytes,overhead_bytes,goodput_ratio");
    for (pn, (name, pkt)) in cases().into_iter().enumerate() {
        let want = tx.sealed_len(&pkt).unwrap();
        let sealed = tx
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: pn as u64,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(sealed.bytes.len(), want, "seal==sealed_len ({name})");
        let payload = match &pkt {
            UnsealedTransportPacket::FecShard { symbol, .. } => symbol.len().saturating_sub(11), // SourcePrefix 11B
            UnsealedTransportPacket::Control(_) => 0,
        };
        println!(
            "{name},{want},{payload},{},{:.4}",
            want - payload,
            payload as f64 / want as f64
        );
    }
}

/// PMTU-gate (normal-CI): TÜM-class'lar gerçek-seal-byte'la ≤1200 (E=1024 dahil).
#[test]
fn m3_pmtu_all_classes() {
    let mut tx = KamaNoiseWireProtector::new(0x2, &key(0xA1), &key(0xB2));
    for (pn, (name, pkt)) in cases().into_iter().enumerate() {
        let want = tx.sealed_len(&pkt).unwrap();
        let sealed = tx
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: pn as u64,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        assert_eq!(sealed.bytes.len(), want, "seal==sealed_len ({name})");
        assert!(want <= 1200, "PMTU: {name} sealed={want} ≤ 1200");
    }
}
