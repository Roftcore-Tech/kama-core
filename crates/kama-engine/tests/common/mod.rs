//! Ortak test-helper'ları (mock-key seal; kama-integration pipeline.rs kalıbı).
#![allow(dead_code)]

use kama_fec_codec::{FecCodec, FecProfile, RsSimdCodec, SourceSymbol};
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

pub const PAYLOAD_CAP: usize = 53; // E(64) - SOURCE_PREFIX_LEN(11)

pub fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

/// (tx, rx) protector çifti (aynı h_ext, ters-anahtar).
pub fn pair(h_ext: u32) -> (KamaNoiseWireProtector, KamaNoiseWireProtector) {
    (
        KamaNoiseWireProtector::new(h_ext, &key(0xA1), &key(0xB2)),
        KamaNoiseWireProtector::new(h_ext, &key(0xB2), &key(0xA1)),
    )
}

/// Tek source-symbol'ün canonical 64B'si (prefix + 0x33-payload).
pub fn source_symbol(esi: u16, message_id: u32) -> Vec<u8> {
    let payload = vec![0x33u8; PAYLOAD_CAP];
    let pfx = SourcePrefix {
        message_id,
        chunk_offset: u32::from(esi) * PAYLOAD_CAP as u32,
        actual_payload_len: PAYLOAD_CAP as u16,
        source_flags: 0,
    };
    let mut sym = vec![0u8; 64];
    pfx.serialize_symbol(&payload, &mut sym).unwrap();
    sym
}

/// Tek source FEC-shard'ı seal → datagram (belirli profile_id/wire_pn).
pub fn seal_source(
    tx: &mut KamaNoiseWireProtector,
    block_id: u32,
    esi: u16,
    profile_id: u8,
    message_id: u32,
    wire_pn: u64,
) -> Vec<u8> {
    let pkt = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id,
            block_id,
            esi,
            k_actual: 1,
        },
        symbol: source_symbol(esi, message_id),
    };
    tx.seal(
        &pkt,
        SealArgs {
            wire_pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

/// Bir control-frame'i seal → datagram (PARAM_SET-emit atomic-kanıtı için).
pub fn seal_control(tx: &mut KamaNoiseWireProtector, ctl: Control, wire_pn: u64) -> Vec<u8> {
    tx.seal(
        &UnsealedTransportPacket::Control(ctl),
        SealArgs {
            wire_pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

/// Bir sealed FEC-shard + logical-esi/kind.
pub struct SealedShard {
    pub datagram: Vec<u8>,
    pub esi: u16,
    pub is_repair: bool,
}

/// app-bytes → Model-A blok → source+repair seal. Döner: (shard'lar, k_actual).
pub fn encode_seal_block(
    tx: &mut KamaNoiseWireProtector,
    profile: &FecProfile,
    app_bytes: &[u8],
    block_id: u32,
    message_id: u32,
    base_pn: u64,
) -> (Vec<SealedShard>, u8) {
    let k_shape = profile.k_shape as usize;
    let e = profile.e_usize();
    let chunks: Vec<&[u8]> = app_bytes.chunks(PAYLOAD_CAP).collect();
    let k_actual = chunks.len();

    let mut block: Vec<Vec<u8>> = Vec::with_capacity(k_shape);
    for (i, chunk) in chunks.iter().enumerate() {
        let pfx = SourcePrefix {
            message_id,
            chunk_offset: (i * PAYLOAD_CAP) as u32,
            actual_payload_len: chunk.len() as u16,
            source_flags: u8::from(i == k_actual - 1),
        };
        let mut sym = vec![0u8; e];
        pfx.serialize_symbol(chunk, &mut sym).unwrap();
        block.push(sym);
    }
    while block.len() < k_shape {
        block.push(vec![0u8; e]);
    }

    let codec = RsSimdCodec;
    let src_syms: Vec<SourceSymbol> = block
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let repairs = codec.encode(profile, &src_syms).unwrap();

    let mut sent = Vec::new();
    let mut pn = base_pn;
    for (esi, sym) in block.iter().enumerate().take(k_actual) {
        let pkt = UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Source,
                profile_id: profile.profile_id,
                block_id,
                esi: esi as u16,
                k_actual: k_actual as u8,
            },
            symbol: sym.clone(),
        };
        let sealed = tx
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        sent.push(SealedShard {
            datagram: sealed.bytes,
            esi: esi as u16,
            is_repair: false,
        });
        pn += 1;
    }
    for rep in &repairs {
        let pkt = UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Repair,
                profile_id: profile.profile_id,
                block_id,
                esi: rep.esi,
                k_actual: k_actual as u8,
            },
            symbol: rep.bytes.clone(),
        };
        let sealed = tx
            .seal(
                &pkt,
                SealArgs {
                    wire_pn: pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .unwrap();
        sent.push(SealedShard {
            datagram: sealed.bytes,
            esi: rep.esi,
            is_repair: true,
        });
        pn += 1;
    }
    (sent, k_actual as u8)
}
