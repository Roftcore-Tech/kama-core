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
