//! Secure-control logical DTO'ları (kama-noise-wire-tdd §2.57). Wire-seal + AEAD noise-wire'da;
//! bu modül yalnız logical-form (schema-owned). Ortak alan: control_id u32_be (idempotent-dedup).

use crate::{BlockId, Esi};

/// Control-frame kimliği (§2.57): session+direction-scoped, retry'da AYNI, yeni-control'da YENİ.
pub type ControlId = u32;

/// §2.57 secure-control body'leri (logical). count'lar profile-bounded (wire-parse'ta enforce).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    // ~~0x10 Ack / 0x11 Sack~~ — [D2, 2026-10-08; K3 2026-10-04] kaldırıldı: ürün ACK'i engine
    // `ACK_V2` (kimlik `full_wire_pn`); codepoint'ler RESERVED → reject (`inner_kind::RESERVED_*`).
    /// 0x12 — logical-recovery (block_id, ESI-range); ACK/BtlBw-credit DEĞİL.
    Recovered {
        id: ControlId,
        block_id: BlockId,
        esi_ranges: Vec<(Esi, Esi)>,
    },
    /// 0x13 — block-complete.
    BlockComplete {
        id: ControlId,
        block_id: BlockId,
        final_source_count: u16,
    },
    /// 0x14 — param_hash + profile/suite (ACK-eliciting).
    ParamSet {
        id: ControlId,
        param_hash: [u8; 32],
        profile_id: u8,
        code_suite_id: u8,
    },
    /// 0x15 — param_hash echo (ACK-eliciting-DEĞİL).
    ParamAck { id: ControlId, param_hash: [u8; 32] },
    /// 0x16 — block-abort + reason.
    BlockAbort {
        id: ControlId,
        block_id: BlockId,
        reason_code: u8,
    },
    /// 0x17 — session-close + reason.
    SessionClose { id: ControlId, reason_code: u8 },
    /// 0x18 — renewal-required (cap/wrap → yeni-handshake §5).
    SessionRenewRequired {
        id: ControlId,
        reason_code: u8,
        renew_hint: u8,
    },
    /// 0x19 [DR-CS-1] — cold-start CS-probe (subtype=0x01; pad=9B-sıfır wire-codec'te; amp≤1:
    /// PROBE_WIRE_EXACT=44B). probe_seq: sender-session-monoton (INV-CS-7). RETRY-EDİLMEZ.
    CsProbe {
        id: ControlId,
        subtype: u8,
        probe_seq: u16,
    },
    /// 0x1A [DR-CS-1] — cold-start probe-echo (idempotent-stateless; ECHO_WIRE_MAX=43B).
    /// seen_count/arrival_gap YALNIZ-LEDGER (karar-girdisi-YAPILAMAZ — amendman-§3).
    CsEcho {
        id: ControlId,
        subtype: u8,
        probe_seq: u16,
        seen_count: u32,
        arrival_gap: u32,
    },
}

impl Control {
    /// inner_kind codepoint'i (§2.56).
    #[must_use]
    pub const fn codepoint(&self) -> u8 {
        use crate::inner_kind::*;
        match self {
            Control::Recovered { .. } => RECOVERED,
            Control::BlockComplete { .. } => BLOCK_COMPLETE,
            Control::ParamSet { .. } => PARAM_SET,
            Control::ParamAck { .. } => PARAM_ACK,
            Control::BlockAbort { .. } => BLOCK_ABORT,
            Control::SessionClose { .. } => SESSION_CLOSE,
            Control::SessionRenewRequired { .. } => SESSION_RENEW_REQUIRED,
            Control::CsProbe { .. } => CTRL_CS_PROBE,
            Control::CsEcho { .. } => CTRL_CS_ECHO,
        }
    }

    /// control_id (§2.57 idempotent-dedup anahtarı).
    #[must_use]
    pub const fn control_id(&self) -> ControlId {
        match self {
            Control::Recovered { id, .. }
            | Control::BlockComplete { id, .. }
            | Control::ParamSet { id, .. }
            | Control::ParamAck { id, .. }
            | Control::BlockAbort { id, .. }
            | Control::SessionClose { id, .. }
            | Control::SessionRenewRequired { id, .. }
            | Control::CsProbe { id, .. }
            | Control::CsEcho { id, .. } => *id,
        }
    }
}
