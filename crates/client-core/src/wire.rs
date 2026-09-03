//! P1 wire-v0 çerçeve codec'i (bench soyu; ADR-05: fixed-width big-endian, VARINT YOK).
//!
//! DATA çerçevesi: `[0x00][seq u16 BE][unit_id u64 BE][payload]` — 11 B başlık. ACK çerçeveleri
//! `adaptive_transport::ack` codec'inin kendisidir (0x10 V1 / 0x14 V2), burada YENİDEN tanımlanmaz.
//! Mühür (tag) BU katmanda YOK — çerçeve [`crate::seam::SecuritySeam`]'den geçerek wire'a çıkar
//! (bench'in TAG_STUB'ı encode içindeydi; P1 dikişi ayırır — SoC).
//!
//! ccr §35 SESSION_DATA finalizasyonu güvenlik-düzlemi entegrasyonunda yapılır (tasarım D5);
//! v0 çerçevesi dikişin arkasında olduğundan swap noktası tek yerdir.

use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::SOURCE_PREFIX_LEN;

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

// ── [S1B] Fragmentli DATA çerçevesi ──────────────────────────────────────────────────────────
//
// Neden ayrı bir codepoint: 0x00 DATA'da chunk metadata'sı YOKTUR, alıcı source-symbol'ün
// prefix'ini `unit_id`'den TÜRETİR (message_id = unit_id, chunk_offset = 0). Bu, bir mesajın tek
// bir symbol'e sığmasını zorunlu kılar (profil 0x03'te 53 B) — production fragmenter'ın önündeki
// duvar buydu. 0x05 çerçevesi canonical `SourcePrefix`'i (§2.6, 11 B) wire'da AÇIKÇA taşır:
//
//   [0x05][seq u16][unit_id u64][ SourcePrefix 11B ‖ payload ]
//                                └──────── gövde = canonical symbol'ün BAŞI ────────┘
//
// Alıcı symbol'ü `gövde ‖ canonical_zero_pad` olarak kurar — hiçbir alan yeniden türetilmez,
// dolayısıyla iki uçtaki symbol'lerin byte-eşitliği YAPISALDIR (türetme-uyumsuzluğu bug sınıfı
// kapanır; RS reconstruct'ın sessizce bozulduğu tek yer orasıydı). Fazladan maliyet 11 B/chunk
// ve YALNIZ fragmentasyon açıkken ödenir — kapalıyken wire 0x00 ile birebir aynıdır.

/// [S1B] Fragmentli DATA frame_type.
pub const FRAME_DATA_FRAG: u8 = 0x05;
/// [S1B] Fragmentli DATA başlık boyu: type(1)+seq u16(2)+unit_id u64(8) + SourcePrefix(11) = 22 B.
pub const HDR_DATA_FRAG: usize = HDR_DATA + SOURCE_PREFIX_LEN;

/// [S1B] Fragmentli DATA kodlar. `body` = `SourcePrefix::serialize_symbol` çıktısının
/// pad'siz başı (prefix ‖ payload) — çağıran onu canonical serializer'dan üretir, bu fonksiyon
/// alan düzenini YENİDEN YAZMAZ (tek-kaynak kuralı).
#[must_use]
pub fn encode_data_frag(seq: u16, unit_id: u64, body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HDR_DATA + body.len());
    b.push(FRAME_DATA_FRAG);
    b.extend_from_slice(&seq.to_be_bytes());
    b.extend_from_slice(&unit_id.to_be_bytes());
    b.extend_from_slice(body);
    b
}

/// [S1B] Fragmentli DATA çözer: `(wire_seq, unit_id, prefix, body)`.
///
/// `body` = prefix ‖ payload (canonical symbol'ün başı; alıcı yalnız pad ekler).
/// FAIL-CLOSED: prefix parse edilemezse VEYA `actual_payload_len` gerçek payload boyuyla
/// UYUŞMAZSA `None` — uzunluk alanı gövdeyle çapraz-doğrulanır, körü körüne güvenilmez.
#[must_use]
pub fn decode_data_frag(buf: &[u8]) -> Option<(u16, u64, SourcePrefix, &[u8])> {
    if buf.len() < HDR_DATA_FRAG || buf[0] != FRAME_DATA_FRAG {
        return None;
    }
    let seq = u16::from_be_bytes([buf[1], buf[2]]);
    let unit_id = u64::from_be_bytes(buf[3..11].try_into().ok()?);
    let body = &buf[HDR_DATA..];
    let pfx = SourcePrefix::parse(body).ok()?;
    if usize::from(pfx.actual_payload_len) != body.len() - SOURCE_PREFIX_LEN {
        return None;
    }
    Some((seq, unit_id, pfx, body))
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

// ── [S1] RECOVERED-control çerçevesi ─────────────────────────────────────────────────────────
//
// Alıcı FEC-reconstruct ile kurtardığı ESI'leri bildirir; gönderici o birimleri ACK-cancel gibi
// iptal eder (`Event::RecoveredControl`, saf-logical, pacer/rate'e girmez). Alan-hizası
// kama-noise-wire `Control::Recovered{block_id, esi_ranges}` (0x12): burada esi-LİSTESİ (S2'de
// range'e eşlenir). Bounded: ≤ RECOVERED_MAX_ESI (noise-wire RECOVERED_MAX_ESI_RANGES=16 hizası).

/// RECOVERED-control frame_type.
pub const FRAME_RECOVERED: u8 = 0x02;
/// RECOVERED başlık boyu: type(1)+block_id u32(4)+count u8(1).
pub const HDR_RECOVERED: usize = 6;
/// Tek çerçevede bildirilebilen en çok ESI (bounded, fail-closed).
pub const RECOVERED_MAX_ESI: usize = 16;

/// RECOVERED çerçevesi kodlar; `esis` RECOVERED_MAX_ESI'ye kırpılır (bounded).
#[must_use]
pub fn encode_recovered(block_id: u32, esis: &[u16]) -> Vec<u8> {
    let n = esis.len().min(RECOVERED_MAX_ESI);
    let mut b = Vec::with_capacity(HDR_RECOVERED + 2 * n);
    b.push(FRAME_RECOVERED);
    b.extend_from_slice(&block_id.to_be_bytes());
    b.push(n as u8);
    for e in &esis[..n] {
        b.extend_from_slice(&e.to_be_bytes());
    }
    b
}

/// RECOVERED çerçevesi çözer: `(block_id, esis)`. Bozuk/aşırı/kısa → `None` (fail-closed).
#[must_use]
pub fn decode_recovered(buf: &[u8]) -> Option<(u32, Vec<u16>)> {
    if buf.len() < HDR_RECOVERED || buf[0] != FRAME_RECOVERED {
        return None;
    }
    let block_id = u32::from_be_bytes(buf[1..5].try_into().ok()?);
    let n = usize::from(buf[5]);
    if n > RECOVERED_MAX_ESI || buf.len() != HDR_RECOVERED + 2 * n {
        return None;
    }
    let esis = buf[HDR_RECOVERED..]
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    Some((block_id, esis))
}

// ── [S1→v1] PARAM bootstrap çerçeveleri ──────────────────────────────────────────────────────
//
// FEC-profil bağlaması: gönderici PARAM'ı yollar (B2: FEC-emisyonu ACK-BEKLEMEDEN akar), alıcı
// yerel-beklenen `ParamFields` ile `ParamState::on_param_set` doğrulaması yapar (BLAKE2s param_hash,
// §8 fail-closed) → admission açılır; ACK yalnız confirmation (retry-durdurur). Alan-hizası
// `Control::ParamSet{param_hash, profile_id, code_suite_id}` — S2'de sealed-control'e taşınır.

/// PARAM_SET frame_type.
pub const FRAME_PARAM: u8 = 0x03;
/// PARAM gövde boyu: type(1)+profile(1)+suite(1)+hash(32).
pub const LEN_PARAM: usize = 35;
/// PARAM_ACK frame_type (hash-echo; confirmation-only).
pub const FRAME_PARAM_ACK: u8 = 0x04;
/// PARAM_ACK boyu: type(1)+hash(32).
pub const LEN_PARAM_ACK: usize = 33;

/// PARAM çerçevesi kodlar.
#[must_use]
pub fn encode_param(profile_id: u8, code_suite_id: u8, param_hash: &[u8; 32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(LEN_PARAM);
    b.push(FRAME_PARAM);
    b.push(profile_id);
    b.push(code_suite_id);
    b.extend_from_slice(param_hash);
    b
}

/// PARAM çerçevesi çözer: `(profile_id, code_suite_id, param_hash)`. Bozuk → `None` (fail-closed).
#[must_use]
pub fn decode_param(buf: &[u8]) -> Option<(u8, u8, [u8; 32])> {
    if buf.len() != LEN_PARAM || buf[0] != FRAME_PARAM {
        return None;
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&buf[3..35]);
    Some((buf[1], buf[2], h))
}

/// PARAM_ACK kodlar (hash-echo).
#[must_use]
pub fn encode_param_ack(param_hash: &[u8; 32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(LEN_PARAM_ACK);
    b.push(FRAME_PARAM_ACK);
    b.extend_from_slice(param_hash);
    b
}

/// PARAM_ACK çözer: `param_hash`. Bozuk → `None`.
#[must_use]
pub fn decode_param_ack(buf: &[u8]) -> Option<[u8; 32]> {
    if buf.len() != LEN_PARAM_ACK || buf[0] != FRAME_PARAM_ACK {
        return None;
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&buf[1..33]);
    Some(h)
}
