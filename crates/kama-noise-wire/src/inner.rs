//! Encrypted-inner envelope codec (§2.56 UNIFIED-INNER-HEADER + §2.6 FEC-variant + §2.57 control-önü).
//!
//! FEC-inner  : inner_version u8=0x01 · inner_kind u8 (0x01/0x02) · flags u8 (reserved→reject) ·
//!              profile_id u8 · block_id u32_be · esi u16_be · k_actual u8 · symbol[E]   (hdr=11B)
//! Ctl-inner  : inner_version u8=0x01 · inner_kind u8 (0x10..0x18) · control_id u32_be · body
//! Cryptex (§2): FEC-semantiği TAMAMEN burada (ciphertext'te) — outer SIZDIRMAZ.

use crate::error::WireError;
use kama_transport_schema::inner_kind::{self, InnerKind};
use kama_transport_schema::{LogicalShardHeader, INNER_V1};

/// FEC-inner sabit header boyu (§2.6): 1+1+1+1+4+2+1 = 11B (symbol hariç).
pub const FEC_INNER_HDR_LEN: usize = 11;
/// Control-inner sabit önü (§2.57): inner_version 1 + inner_kind 1 + control_id 4 = 6B (body hariç).
pub const CONTROL_INNER_PREFIX_LEN: usize = 6;

/// Decrypt-sonrası sınıflandırılmış inner (§4-ORDER adım-7 çıktısı).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerPlain {
    FecShard {
        header: LogicalShardHeader,
        symbol: Vec<u8>,
    },
    /// Control: typed-body-decode Sprint-2'de (§2.57); burada bounds-checked ham gövde.
    Control {
        inner_kind: u8,
        control_id: u32,
        body: Vec<u8>,
    },
}

/// FEC-shard'ı inner-plaintext'e serialize eder (seal-yolu).
#[must_use]
pub fn encode_fec_inner(header: &LogicalShardHeader, symbol: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FEC_INNER_HDR_LEN + symbol.len());
    out.push(INNER_V1);
    out.push(header.kind.codepoint());
    out.push(0u8); // flags: reserved, hep-0 yazılır (§2.6)
    out.push(header.profile_id);
    out.extend_from_slice(&header.block_id.to_be_bytes());
    out.extend_from_slice(&header.esi.to_be_bytes());
    out.push(header.k_actual);
    out.extend_from_slice(symbol);
    out
}

/// Control-frame'i inner-plaintext'e serialize eder (body-codec Sprint-2; ham-body burada).
#[must_use]
pub fn encode_control_inner(kind_codepoint: u8, control_id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(CONTROL_INNER_PREFIX_LEN + body.len());
    out.push(INNER_V1);
    out.push(kind_codepoint);
    out.extend_from_slice(&control_id.to_be_bytes());
    out.extend_from_slice(body);
    out
}

/// Decrypt-sonrası inner-parse + bounds (§4-ORDER adım-7). Fail-closed: bilinmeyen-version/kind/
/// reserved-flags/kısa-gövde → reject.
///
/// # Errors
/// [`WireError::InnerParse`] — sınır/registry ihlali (içerik-sızdırmayan sabit-mesajlar).
pub fn parse_inner(plain: &[u8]) -> Result<InnerPlain, WireError> {
    if plain.len() < 2 {
        return Err(WireError::InnerParse("inner too short"));
    }
    if plain[0] != INNER_V1 {
        return Err(WireError::InnerParse("unknown inner_version"));
    }
    let kind_cp = plain[1];
    match inner_kind::classify(kind_cp).map_err(|_| WireError::InnerParse("unknown inner_kind"))? {
        InnerKind::Fec(kind) => {
            if plain.len() < FEC_INNER_HDR_LEN {
                return Err(WireError::InnerParse("fec inner too short"));
            }
            if plain[2] != 0 {
                return Err(WireError::InnerParse("fec inner reserved flags"));
            }
            let profile_id = plain[3];
            let block_id = u32::from_be_bytes([plain[4], plain[5], plain[6], plain[7]]);
            let esi = u16::from_be_bytes([plain[8], plain[9]]);
            let k_actual = plain[10];
            if k_actual == 0 {
                return Err(WireError::InnerParse("k_actual zero (B4)"));
            }
            let symbol = plain[FEC_INNER_HDR_LEN..].to_vec();
            if symbol.is_empty() {
                return Err(WireError::InnerParse("empty symbol"));
            }
            Ok(InnerPlain::FecShard {
                header: LogicalShardHeader {
                    kind,
                    profile_id,
                    block_id,
                    esi,
                    k_actual,
                },
                symbol,
            })
        }
        InnerKind::Control(_) => {
            if plain.len() < CONTROL_INNER_PREFIX_LEN {
                return Err(WireError::InnerParse("control inner too short"));
            }
            let control_id = u32::from_be_bytes([plain[2], plain[3], plain[4], plain[5]]);
            Ok(InnerPlain::Control {
                inner_kind: kind_cp,
                control_id,
                body: plain[CONTROL_INNER_PREFIX_LEN..].to_vec(),
            })
        }
    }
}
