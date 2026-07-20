//! Canonical-source-symbol-PREFIX serializer (kama-noise-wire-tdd §2.6).
//!
//! RS-kodlanan canonical-source-symbol'ün ilk 11 byte'ı (kayıpta metadata reconstruct):
//! `message_id u32_be · chunk_offset u32_be · actual_payload_len u16_be · source_flags u8`.
//! Ardından `application_payload[actual_payload_len]` + `canonical_zero_pad[E − 11 − len]`.
//! symbol_bytes == profile.E her zaman (§2.6 symbol_len-KİLİDİ).
//!
//! **PREFIX-DOĞRULAMA (§2.6 MADDE-9, yalnız prefix'te BULUNAN alanlar):** actual_payload_len ≤ E−11 ·
//! source_flags reserved-nonzero → reject · chunk_offset block-tutarlılığı (çağıran-context).
//! block_id/esi/profile_id PREFIX'TE YOK — decoder-context'ten (imkânsız-eşitlik-kaldırıldı).

use crate::{MessageId, SchemaError, SOURCE_PREFIX_LEN};

/// source_flags anlamlı-bit maskesi (§2.6: final/end-of-message). Kalan bitler reserved → reject.
pub const SOURCE_FLAG_FINAL: u8 = 0b0000_0001;
const SOURCE_FLAGS_RESERVED_MASK: u8 = !SOURCE_FLAG_FINAL;

/// Bir source-symbol'ün prefix + payload alanları (block-common metadata AYRI: inner-header §2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePrefix {
    pub message_id: MessageId,
    pub chunk_offset: u32,
    pub actual_payload_len: u16,
    pub source_flags: u8,
}

impl SourcePrefix {
    /// Canonical source-symbol'ü E-byte tampona serialize eder: prefix(11B) + payload +
    /// canonical_zero_pad. `out.len()` == E olmalı (symbol_len-KİLİDİ).
    ///
    /// # Errors
    /// - [`SchemaError::ReservedFlagsSet`] source_flags reserved-bit set ise.
    /// - [`SchemaError::PayloadTooLong`] payload E−11'e sığmıyorsa.
    /// - [`SchemaError::SymbolLenMismatch`] out.len() != E (payload+prefix+pad tutmuyorsa).
    pub fn serialize_symbol(&self, payload: &[u8], out: &mut [u8]) -> Result<(), SchemaError> {
        if self.source_flags & SOURCE_FLAGS_RESERVED_MASK != 0 {
            return Err(SchemaError::ReservedFlagsSet(self.source_flags));
        }
        let e = out.len();
        let cap = e.saturating_sub(SOURCE_PREFIX_LEN);
        if payload.len() > cap {
            return Err(SchemaError::PayloadTooLong {
                len: payload.len(),
                cap,
            });
        }
        if usize::from(self.actual_payload_len) != payload.len() {
            return Err(SchemaError::SymbolLenMismatch {
                got: payload.len(),
                expected: usize::from(self.actual_payload_len),
            });
        }
        // prefix (11B, big-endian, sıra-sabit §2.6)
        out[0..4].copy_from_slice(&self.message_id.to_be_bytes());
        out[4..8].copy_from_slice(&self.chunk_offset.to_be_bytes());
        out[8..10].copy_from_slice(&self.actual_payload_len.to_be_bytes());
        out[10] = self.source_flags;
        // payload + canonical zero-pad
        out[SOURCE_PREFIX_LEN..SOURCE_PREFIX_LEN + payload.len()].copy_from_slice(payload);
        for b in &mut out[SOURCE_PREFIX_LEN + payload.len()..] {
            *b = 0;
        }
        Ok(())
    }

    /// E-byte canonical symbol'den prefix'i geri okur (kayıp-source reconstruct'ında metadata-recovery).
    ///
    /// # Errors
    /// - [`SchemaError::BufferTooShort`] symbol < 11B ise.
    /// - [`SchemaError::ReservedFlagsSet`] / [`SchemaError::PayloadTooLong`] doğrulama.
    pub fn parse(symbol: &[u8]) -> Result<Self, SchemaError> {
        if symbol.len() < SOURCE_PREFIX_LEN {
            return Err(SchemaError::BufferTooShort {
                need: SOURCE_PREFIX_LEN,
                have: symbol.len(),
            });
        }
        let message_id = u32::from_be_bytes([symbol[0], symbol[1], symbol[2], symbol[3]]);
        let chunk_offset = u32::from_be_bytes([symbol[4], symbol[5], symbol[6], symbol[7]]);
        let actual_payload_len = u16::from_be_bytes([symbol[8], symbol[9]]);
        let source_flags = symbol[10];
        if source_flags & SOURCE_FLAGS_RESERVED_MASK != 0 {
            return Err(SchemaError::ReservedFlagsSet(source_flags));
        }
        let cap = symbol.len() - SOURCE_PREFIX_LEN;
        if usize::from(actual_payload_len) > cap {
            return Err(SchemaError::PayloadTooLong {
                len: usize::from(actual_payload_len),
                cap,
            });
        }
        Ok(SourcePrefix {
            message_id,
            chunk_offset,
            actual_payload_len,
            source_flags,
        })
    }

    /// Recovered symbol'den application-payload dilimi (prefix sonrası actual_payload_len byte).
    #[must_use]
    pub fn payload_slice<'a>(&self, symbol: &'a [u8]) -> &'a [u8] {
        let end = SOURCE_PREFIX_LEN + usize::from(self.actual_payload_len);
        &symbol[SOURCE_PREFIX_LEN..end.min(symbol.len())]
    }
}
