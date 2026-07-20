//! kama-transport-schema — primitive tipler + logical enum'lar + UNSEALED DTO'lar + canonical
//! serialization + wire-length contracts + protocol errors (kama-noise-wire-tdd §11.5, P0-8).
//!
//! **İÇERMEZ (NORMATİF):** Noise/AEAD/RS/CC/socket/identity/trust. Bu crate kripto BİLMEZ;
//! yalnız byte-genişlikleri, logical-kimlikler ve canonical-serialization taşır. Herkes bağımlı
//! olabilir (schema ← {fec-codec, recovery-controller, noise-wire} ← engine; döngü YASAK).
//!
//! **BYTE-OTORİTESİ:** kama-noise-wire-tdd-v2.0 §2.6 (byte-layout), §2.56 (inner_kind registry),
//! §2.57 (control body), §8 (PARAM canonical-hash). Başka yerde byte tanımlanmaz.
//!
//! ADR-05: fixed-width big-endian canonical; VARINT YASAK.

#![forbid(unsafe_code)]

pub mod inner_kind;
pub mod param;
pub mod prefix;

use thiserror::Error;

// ── ID genişlikleri (§2.6 ID-SCOPE/WRAP + D0-B gate "ID widths") ──
/// FEC profile kimliği (§2.6: profile_id u8).
pub type ProfileId = u8;
/// Code-suite kimliği (RS-matematiği/generator/ESI — fec-core §5).
pub type CodeSuiteId = u8;
/// Blok kimliği (§2.6: session+direction-scoped monoton u32_be, NO-REUSE).
pub type BlockId = u32;
/// Mesaj kimliği (§2.6: session+direction-scoped monoton u32_be, NO-REUSE).
pub type MessageId = u32;
/// Encoding Symbol ID — source veya repair (§2.6: esi u16_be).
pub type Esi = u16;
/// Bir blokta gerçekten var olan kaynak-shard sayısı (§2.6: k_actual u8; B4: 1≤K_actual≤K_shape≤255).
pub type KActual = u8;

/// Canonical-source-symbol-prefix sabit boyutu (§2.6): message_id 4 + chunk_offset 4 +
/// actual_payload_len 2 + source_flags 1 = 11B. Prefix + payload + zero_pad == E her zaman.
pub const SOURCE_PREFIX_LEN: usize = 11;

/// EXTENDED-outer sabit overhead (§2.6): h_ext 4 + wire_format 1 + wire_flags 1 + pn_trunc 4 +
/// tag 16 = 26B. sealed_len(EXTENDED) = 26 + encrypted_inner_len.
pub const EXTENDED_OUTER_OVERHEAD: usize = 26;

/// Legacy SESSION_DATA overhead (§2.6): hub_handle 4 + seq 2 + tag 16 = 22B. sealed_len(legacy)
/// = 22 + plaintext_payload_len (AEAD-expansion=yalnız-tag varsayımı; N7-raporunda exact-teyit).
pub const LEGACY_SESSION_DATA_OVERHEAD: usize = 22;

/// EXTENDED_V1 wire_format kodu (§2.5/§2.6).
pub const EXTENDED_V1: u8 = 0x01;
/// UNIFIED-INNER-HEADER inner_version (§2.56: INNER_V1).
pub const INNER_V1: u8 = 0x01;

/// FEC-shard mantıksal başlığı — UNSEALED DTO (§12 ortak-tipler: LogicalShardHeader). Block-common
/// metadata (§2.6 FEC-inner header). Kripto/nonce/tag TAŞIMAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalShardHeader {
    pub kind: inner_kind::ShardKind,
    pub profile_id: ProfileId,
    pub block_id: BlockId,
    pub esi: Esi,
    pub k_actual: KActual,
}

/// UNSEALED transport-paket DTO'su (§12): herkes üretebilir; sealed/Opened DEĞİL (o noise-wire-owned).
/// FEC source/repair + listed secure control'lerin logical-formu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsealedTransportPacket {
    /// FEC source/repair shard: header + opaque symbol bytes (== profile.E; §2.6 symbol_len-YOK).
    FecShard {
        header: LogicalShardHeader,
        symbol: Vec<u8>,
    },
    /// Secure-control frame (§2.56 inner_kind 0x10..0x18 + §2.57 body). Body logical-formda.
    Control(control::Control),
}

/// Protocol-level hatalar (crate-genel; typed, thiserror — reachable todo!() YASAK).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchemaError {
    /// actual_payload_len prefix-sınırını aşıyor (§2.6: ≤ E−11).
    #[error("payload len {len} exceeds symbol capacity {cap}")]
    PayloadTooLong { len: usize, cap: usize },
    /// source_flags reserved-bit nonzero (§2.6/MADDE-9: reject).
    #[error("source_flags reserved bits set: {0:#04x}")]
    ReservedFlagsSet(u8),
    /// symbol_bytes uzunluğu profile.E ile uyuşmuyor (§2.6 symbol_len-KİLİDİ).
    #[error("symbol length {got} != profile E {expected}")]
    SymbolLenMismatch { got: usize, expected: usize },
    /// B4-invariant ihlali (§8: 1≤K_actual≤K_shape≤255, E≥11).
    #[error("profile bound violation: {0}")]
    ProfileBound(&'static str),
    /// Bilinmeyen/reserved inner_kind codepoint (§2.56: fail-closed).
    #[error("unknown inner_kind codepoint: {0:#04x}")]
    UnknownInnerKind(u8),
    /// Serileştirme tampon-boyutu yetersiz.
    #[error("buffer too short: need {need}, have {have}")]
    BufferTooShort { need: usize, have: usize },
}

pub mod control;
