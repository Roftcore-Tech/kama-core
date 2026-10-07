//! Encrypted-inner envelope codec (§2.56 UNIFIED-INNER-HEADER + §2.6 FEC-variant + §2.57 control-önü).
//!
//! FEC-inner  : inner_version u8=0x01 · inner_kind u8 (0x01/0x02) · flags u8 (reserved→reject) ·
//!              profile_id u8 · block_id u32_be · esi u16_be · k_actual u8 · symbol[E]   (hdr=11B)
//! Ctl-inner  : inner_version u8=0x01 · inner_kind u8 (0x12..=0x1A; 0x10/0x11 RESERVED → reject,
//!              D2 2026-10-08) · control_id u32_be · body
//! Cryptex (§2): FEC-semantiği TAMAMEN burada (ciphertext'te) — outer SIZDIRMAZ.

use crate::error::WireError;
use kama_transport_schema::inner_kind::{self, InnerKind};
use kama_transport_schema::{LogicalShardHeader, INNER_V1};
#[cfg(feature = "experimental-cold-start")]
use zeroize::Zeroize;

/// A-5 (DR-CS-1b/A2; spec-§0.9-amendmanı): plaintext FEC-symbol'ün TEK-KANONİK-SAHİBİ.
/// - Drop öncesinde payload allocation'ını (Vec-capacity dahil) ZEROIZE eder.
/// - `InnerPlain → OpenedTransportShard → admission/decode` yolunda MOVE edilir (KOPYALANMAZ).
/// - borrow yalnız `&[u8]` / `AsRef<[u8]>` biçiminde (ham mutable-Vec dışarı verilmez).
/// - **Clone DEĞİLDİR** (zeroize-edilmeyen-kopya yapısal-imkansız; source ve repair aynı sözleşme).
/// - Debug payload YAZMAZ (yalnız uzunluk).
/// - Zeroize (payload sıfırlama) `experimental-cold-start` arkasında; flag-OFF: önceki-davranış (INV-CS-5).
pub struct SensitiveSymbolBytes(Vec<u8>);

impl SensitiveSymbolBytes {
    pub(crate) fn from_vec(v: Vec<u8>) -> Self {
        Self(v)
    }
}

/// Vec ownership'ı İÇERİ-alır (dışarı-vermez); admission-reconstruct'ta recovered-symbol'ü sarmak için
/// (A-5: reconstructed-symbol da zeroize-sahibi olur). Ham-Vec dışarı-çıkışı bu tipte YOKtur.
impl From<Vec<u8>> for SensitiveSymbolBytes {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl core::ops::Deref for SensitiveSymbolBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for SensitiveSymbolBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for SensitiveSymbolBytes {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for SensitiveSymbolBytes {}

impl core::fmt::Debug for SensitiveSymbolBytes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Payload SIZDIRMAZ: yalnız metadata (uzunluk).
        f.debug_struct("SensitiveSymbolBytes")
            .field("len", &self.0.len())
            .finish()
    }
}

impl Drop for SensitiveSymbolBytes {
    fn drop(&mut self) {
        // A-5: terminal-state zeroize (parse-error/evict/deadline/release/decode-sonrası tüm-yollar).
        // flag-OFF: Vec normal-drop (önceki-davranış; INV-CS-5).
        #[cfg(feature = "experimental-cold-start")]
        self.0.zeroize();
    }
}

/// FEC-inner sabit header boyu (§2.6): 1+1+1+1+4+2+1 = 11B (symbol hariç).
pub const FEC_INNER_HDR_LEN: usize = 11;
/// Control-inner sabit önü (§2.57): inner_version 1 + inner_kind 1 + control_id 4 = 6B (body hariç).
pub const CONTROL_INNER_PREFIX_LEN: usize = 6;

/// Decrypt-sonrası sınıflandırılmış inner (§4-ORDER adım-7 çıktısı).
/// NOT (A-5): `Clone` DERIVE KALDIRILDI — `SensitiveSymbolBytes` non-Clone (zeroize-edilmeyen-kopya
/// yasağı). InnerPlain.clone() üretimde/testte HİÇ çağrılmıyordu (yalnız-derive; salt-API-daralması).
#[derive(Debug, PartialEq, Eq)]
pub enum InnerPlain {
    FecShard {
        header: LogicalShardHeader,
        symbol: SensitiveSymbolBytes,
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
                symbol: SensitiveSymbolBytes::from_vec(symbol),
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
