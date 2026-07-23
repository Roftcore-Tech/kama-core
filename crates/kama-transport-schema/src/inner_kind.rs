//! INNER_KIND registry (kama-noise-wire-tdd §2.56) — decrypt-sonrası tek-birleşik-alan;
//! packet_type↔control_type çakışması YOK. Bilinmeyen codepoint → FAIL-CLOSED reject.

use crate::SchemaError;

// ── FEC codepoint'leri (§2.56) ──
/// inner_kind = FEC source shard.
pub const FEC_SOURCE: u8 = 0x01;
/// inner_kind = FEC repair shard.
pub const FEC_REPAIR: u8 = 0x02;

// ── Control codepoint'leri (§2.56 allowlist; KEY_UPDATE/NACK YOK) ──
pub const ACK: u8 = 0x10;
pub const SACK: u8 = 0x11;
pub const RECOVERED: u8 = 0x12;
pub const BLOCK_COMPLETE: u8 = 0x13;
pub const PARAM_SET: u8 = 0x14;
pub const PARAM_ACK: u8 = 0x15;
pub const BLOCK_ABORT: u8 = 0x16;
pub const SESSION_CLOSE: u8 = 0x17;
pub const SESSION_RENEW_REQUIRED: u8 = 0x18;
/// DR-CS-1-AMENDMAN (Abkarada-onaylı; §2.56): cold-start minimal-bootstrap probe.
pub const CTRL_CS_PROBE: u8 = 0x19;
/// DR-CS-1-AMENDMAN: cold-start probe-echo (idempotent-stateless yanıt).
pub const CTRL_CS_ECHO: u8 = 0x1A;

/// FEC shard türü (source/repair) — §2.6 FEC-inner header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardKind {
    Source,
    Repair,
}

impl ShardKind {
    /// inner_kind codepoint'i (§2.56).
    #[must_use]
    pub const fn codepoint(self) -> u8 {
        match self {
            ShardKind::Source => FEC_SOURCE,
            ShardKind::Repair => FEC_REPAIR,
        }
    }

    /// Codepoint → ShardKind. FEC-dışı/reserved → None (çağıran control-branch'ine düşer).
    #[must_use]
    pub const fn from_codepoint(cp: u8) -> Option<Self> {
        match cp {
            FEC_SOURCE => Some(ShardKind::Source),
            FEC_REPAIR => Some(ShardKind::Repair),
            _ => None,
        }
    }
}

/// Birleşik inner_kind sınıflandırması (§2.56 tek-parser-seçimi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnerKind {
    Fec(ShardKind),
    Control(u8),
}

/// Codepoint → InnerKind (fail-closed: 0x00 reserved, 0x03..0x0F FEC-reserved, 0x1B..0xFF reserved).
/// Fail-closed = FRAME-seviyesi reject (session-yaşar) — eski-peer'ın yeni-codepoint'leri safe-ignore
/// etmesinin yapısal-tabanı (DR-CS-1 mixed-version).
///
/// # Errors
/// Reserved/bilinmeyen codepoint → [`SchemaError::UnknownInnerKind`].
pub fn classify(cp: u8) -> Result<InnerKind, SchemaError> {
    match cp {
        FEC_SOURCE => Ok(InnerKind::Fec(ShardKind::Source)),
        FEC_REPAIR => Ok(InnerKind::Fec(ShardKind::Repair)),
        ACK
        | SACK
        | RECOVERED
        | BLOCK_COMPLETE
        | PARAM_SET
        | PARAM_ACK
        | BLOCK_ABORT
        | SESSION_CLOSE
        | SESSION_RENEW_REQUIRED
        | CTRL_CS_PROBE
        | CTRL_CS_ECHO => Ok(InnerKind::Control(cp)),
        other => Err(SchemaError::UnknownInnerKind(other)),
    }
}
