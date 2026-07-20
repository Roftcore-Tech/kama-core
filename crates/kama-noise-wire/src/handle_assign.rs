//! HANDLE_ASSIGN (§2.55 WIRE-FROZEN) — hub→client hub-control mesajı (hub↔client-Noise-transport-kanalında
//! authenticated; E2E-endpoint-key DEĞİL). h_ext'i initiator'a hub-admission-sonrası, responder'a
//! forwarded-INIT'ten-ÖNCE/birlikte dağıtır. **NAMESPACE:** E2E inner_kind-registry'sine (§2.56) DAHİL-DEĞİL —
//! ayrı hub-control-namespace.
//!
//! Layout (§2.55, exact): version u8=0x01 · session_attempt_id u64_be · init_counter u64_be (echo) ·
//! h_legacy u32_be · h_ext u32_be = 25B. REPLAY-IDENTITY = (hub_channel_session, session_attempt_id,
//! init_counter) — aynı-attempt-tekrarı-AYNI-içerik (idempotent). HANDLE_ASSIGN_ACK: session_attempt_id u64_be echo.

use crate::error::WireError;

/// §2.55 HANDLE_ASSIGN version (WIRE-FROZEN).
pub const HANDLE_ASSIGN_V1: u8 = 0x01;
/// §2.55 sabit boyut: 1 + 8 + 8 + 4 + 4 = 25B.
pub const HANDLE_ASSIGN_LEN: usize = 25;
/// HANDLE_ASSIGN_ACK sabit boyut: session_attempt_id u64_be = 8B.
pub const HANDLE_ASSIGN_ACK_LEN: usize = 8;

/// hub→client HANDLE_ASSIGN (§2.55). h_ext'i taşır → extended-parser bunsuz-çalışmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleAssign {
    /// hub-assigned, hub-local-unique (attempt-bağı; collision-safe).
    pub session_attempt_id: u64,
    /// initiator-INIT'inden echo (client-doğrulaması: hangi-INIT'e-cevap).
    pub init_counter: u64,
    /// legacy SESSION_DATA-parser handle'ı.
    pub h_legacy: u32,
    /// extended (FEC+control) parser handle'ı.
    pub h_ext: u32,
}

impl HandleAssign {
    /// §2.55 fixed-width big-endian serialize (25B).
    #[must_use]
    pub fn encode(&self) -> [u8; HANDLE_ASSIGN_LEN] {
        let mut out = [0u8; HANDLE_ASSIGN_LEN];
        out[0] = HANDLE_ASSIGN_V1;
        out[1..9].copy_from_slice(&self.session_attempt_id.to_be_bytes());
        out[9..17].copy_from_slice(&self.init_counter.to_be_bytes());
        out[17..21].copy_from_slice(&self.h_legacy.to_be_bytes());
        out[21..25].copy_from_slice(&self.h_ext.to_be_bytes());
        out
    }

    /// §2.55 parse (fail-closed: kısa-buffer / bilinmeyen-version / trailing → reject).
    ///
    /// # Errors
    /// [`WireError::InnerParse`] — sınır/version ihlali.
    pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
        if buf.len() != HANDLE_ASSIGN_LEN {
            return Err(WireError::InnerParse("HANDLE_ASSIGN length"));
        }
        if buf[0] != HANDLE_ASSIGN_V1 {
            return Err(WireError::InnerParse("HANDLE_ASSIGN version"));
        }
        let session_attempt_id = u64::from_be_bytes(buf[1..9].try_into().expect("8B"));
        let init_counter = u64::from_be_bytes(buf[9..17].try_into().expect("8B"));
        let h_legacy = u32::from_be_bytes(buf[17..21].try_into().expect("4B"));
        let h_ext = u32::from_be_bytes(buf[21..25].try_into().expect("4B"));
        Ok(Self {
            session_attempt_id,
            init_counter,
            h_legacy,
            h_ext,
        })
    }

    /// REPLAY-IDENTITY (§2.55): aynı-attempt-tekrarı idempotent-eşit-içerik anahtarı.
    #[must_use]
    pub fn replay_identity(&self) -> (u64, u64) {
        (self.session_attempt_id, self.init_counter)
    }
}

/// HANDLE_ASSIGN_ACK (§2.55): client→hub, session_attempt_id echo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleAssignAck {
    pub session_attempt_id: u64,
}

impl HandleAssignAck {
    #[must_use]
    pub fn encode(&self) -> [u8; HANDLE_ASSIGN_ACK_LEN] {
        self.session_attempt_id.to_be_bytes()
    }

    /// # Errors
    /// [`WireError::InnerParse`] — kısa-buffer.
    pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
        if buf.len() != HANDLE_ASSIGN_ACK_LEN {
            return Err(WireError::InnerParse("HANDLE_ASSIGN_ACK length"));
        }
        Ok(Self {
            session_attempt_id: u64::from_be_bytes(buf.try_into().expect("8B")),
        })
    }
}
