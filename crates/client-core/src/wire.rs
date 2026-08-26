//! P1 wire-v0 çerçeve codec'i (bench soyu; ADR-05: fixed-width big-endian, VARINT YOK).
//!
//! DATA çerçevesi: `[0x00][seq u16 BE][unit_id u64 BE][payload]` — 11 B başlık. ACK çerçeveleri
//! `adaptive_transport::ack` codec'inin kendisidir (0x10 V1 / 0x14 V2), burada YENİDEN tanımlanmaz.
//! Mühür (tag) BU katmanda YOK — çerçeve [`crate::seam::SecuritySeam`]'den geçerek wire'a çıkar
//! (bench'in TAG_STUB'ı encode içindeydi; P1 dikişi ayırır — SoC).
//!
//! ccr §35 SESSION_DATA finalizasyonu güvenlik-düzlemi entegrasyonunda yapılır (tasarım D5);
//! v0 çerçevesi dikişin arkasında olduğundan swap noktası tek yerdir.

/// DATA frame_type.
pub const FRAME_DATA: u8 = 0x00;
/// DATA başlık boyu: type(1) + seq u16(2) + unit_id u64(8).
pub const HDR_DATA: usize = 11;

/// DATA çerçevesi kodlar (mühürsüz düz çerçeve; seam.seal çağıranın işi).
#[must_use]
pub fn encode_data(seq: u16, unit_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HDR_DATA + payload.len());
    b.push(FRAME_DATA);
    b.extend_from_slice(&seq.to_be_bytes());
    b.extend_from_slice(&unit_id.to_be_bytes());
    b.extend_from_slice(payload);
    b
}

/// DATA çerçevesi çözer: `(wire_seq, unit_id, payload)`. Bozuk/farklı-tip → `None`.
#[must_use]
pub fn decode_data(buf: &[u8]) -> Option<(u16, u64, &[u8])> {
    if buf.len() < HDR_DATA || buf[0] != FRAME_DATA {
        return None;
    }
    let seq = u16::from_be_bytes([buf[1], buf[2]]);
    let unit_id = u64::from_be_bytes(buf[3..11].try_into().ok()?);
    Some((seq, unit_id, &buf[HDR_DATA..]))
}

// ── [S1] FEC-shard çerçevesi ──────────────────────────────────────────────────────────────────
//
// Alan-düzeni kama-noise-wire `encode_fec_inner`/`LogicalShardHeader` (§2.6) ile BİREBİR hizalı:
// kind-codepoint (0x01 SOURCE / 0x02 REPAIR = `kama_transport_schema::inner_kind`), profile_id,
// block_id u32, esi u16, k_actual u8 — + motor-ACK'i için wire-seq (noise-wire'da karşılığı PN).
// S2 noise-wire adopsiyonu bu çerçeveyi seam-arkasında `UnsealedTransportPacket::FecShard`'a
// alan-eşlemeli swap eder (tek-nokta, D5). VARINT YOK (ADR-05).

/// FEC-shard frame_type.
pub const FRAME_FEC: u8 = 0x01;
/// FEC başlık boyu: type(1)+kind(1)+profile(1)+block u32(4)+esi u16(2)+k_actual(1)+seq u16(2).
pub const HDR_FEC: usize = 12;

/// FEC-shard başlığı (çözülmüş) — alanlar §2.6 `LogicalShardHeader` hizası.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecFrameHeader {
    /// `kama_transport_schema::inner_kind` codepoint'i: 0x01 SOURCE / 0x02 REPAIR.
    pub kind: u8,
    /// FEC profil kimliği (`kama_fec_codec::by_id` uzayı).
    pub profile_id: u8,
    /// Blok kimliği (session+direction-scoped monoton; yeniden-kullanım YOK).
    pub block_id: u32,
    /// Encoding Symbol ID.
    pub esi: u16,
    /// Bloktaki gerçek source-shard sayısı (1..=k_shape).
    pub k_actual: u8,
    /// Motor wire-seq (ACK yolu; alt-16 bit).
    pub seq: u16,
}

/// FEC-shard çerçevesi kodlar (mühürsüz; seam.seal çağıranın işi). `symbol.len() == profile.e`.
#[must_use]
pub fn encode_fec(h: &FecFrameHeader, symbol: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HDR_FEC + symbol.len());
    b.push(FRAME_FEC);
    b.push(h.kind);
    b.push(h.profile_id);
    b.extend_from_slice(&h.block_id.to_be_bytes());
    b.extend_from_slice(&h.esi.to_be_bytes());
    b.push(h.k_actual);
    b.extend_from_slice(&h.seq.to_be_bytes());
    b.extend_from_slice(symbol);
    b
}

/// FEC-shard çerçevesi çözer: `(header, symbol)`. Bozuk/farklı-tip/k_actual=0 → `None`
/// (fail-closed; `parse_inner` k_actual≠0 kuralıyla hizalı).
#[must_use]
pub fn decode_fec(buf: &[u8]) -> Option<(FecFrameHeader, &[u8])> {
    if buf.len() < HDR_FEC || buf[0] != FRAME_FEC {
        return None;
    }
    let h = FecFrameHeader {
        kind: buf[1],
        profile_id: buf[2],
        block_id: u32::from_be_bytes(buf[3..7].try_into().ok()?),
        esi: u16::from_be_bytes([buf[7], buf[8]]),
        k_actual: buf[9],
        seq: u16::from_be_bytes([buf[10], buf[11]]),
    };
    if h.k_actual == 0 {
        return None;
    }
    Some((h, &buf[HDR_FEC..]))
}
