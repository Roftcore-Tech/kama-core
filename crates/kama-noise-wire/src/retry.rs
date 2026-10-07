//! Control idempotent-dedup + retry-state-machine (§2.57 IDEMPOTENCY + RETRY tablosu).
//!
//! ALIM-tarafı: `InboundControlDedup` — aynı control_id ikinci-kez app/state-side-effect ÜRETMEZ;
//!   yalnız-ACK yeniden-üretilebilir (idempotent). SESSION_CLOSE için bounded-TOMBSTONE.
//! GÖNDERME-tarafı: `RetryClass` — hangi control retry-edilir, hangi-koşulda-durur (§2.57):
//!   PARAM_SET → PARAM_ACK-hash-echo'ya-kadar (deadline→durur, emission-SÜRER B2)
//!   ABORT/COMPLETE/CLOSE/RENEW → kendi-wire-PN network-ACK'ine-kadar (deadline-bounded)
//!   ~~ACK/SACK~~ [D2 kaldırıldı] RECOVERED → RETRY-YOK (pure-feedback, en-güncel-durum)

use kama_transport_schema::control::{Control, ControlId};
use kama_transport_schema::inner_kind;
use std::collections::HashSet;

/// §2.57 retry-sınıfı (gönderme-tarafı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryClass {
    /// PARAM_SET: PARAM_ACK-hash-echo gelene-kadar (deadline→retry-durur, emission-SÜRER).
    UntilParamAck,
    /// ABORT/COMPLETE/CLOSE/RENEW: kendi-wire-PN network-ACK'ine-kadar (deadline-bounded).
    UntilNetworkAck,
    /// RECOVERED (~~ACK/SACK~~ D2'de kaldırıldı): retry-EDİLMEZ (pure-feedback).
    NoRetry,
}

impl RetryClass {
    /// Control-codepoint → retry-sınıfı (§2.57 tablosu; tek-kaynak).
    #[must_use]
    pub fn of(codepoint: u8) -> Self {
        use inner_kind::*;
        match codepoint {
            PARAM_SET => RetryClass::UntilParamAck,
            BLOCK_ABORT | BLOCK_COMPLETE | SESSION_CLOSE | SESSION_RENEW_REQUIRED => {
                RetryClass::UntilNetworkAck
            }
            RECOVERED | PARAM_ACK => RetryClass::NoRetry,
            // DR-CS-1 (§2.57-amendman): probe-retry = merdivenin-kendisi; echo idempotent-stateless.
            CTRL_CS_PROBE | CTRL_CS_ECHO => RetryClass::NoRetry,
            _ => RetryClass::NoRetry,
        }
    }

    #[must_use]
    pub fn of_control(c: &Control) -> Self {
        Self::of(c.codepoint())
    }
}

/// **RECOVERED bandwidth-credit üretMEZ (N8-invariant).** Bu fonksiyon derleme-zamanı-belgelemesi +
/// runtime-assert için: hiçbir control-tipi "delivered/BtlBw-credit" taşımaz; RECOVERED yalnız-logical.
/// (Gerçek muhasebe Sprint-5 engine-tarafında; burada kripto-katmanın-credit-üretmediği sabittir.)
#[must_use]
pub fn control_produces_bandwidth_credit(_c: &Control) -> bool {
    // Hiçbir control-tipi wire-katmanında BtlBw/delivered-credit ÜRETMEZ (§2.57 ACK-eliciting≠credit;
    // RECOVERED açıkça ACK/BtlBw-credit-DEĞİL). ACK'in gerçek-wire-teslim-muhasebesi SealedPacketMeta
    // üzerinden engine'de yapılır; RECOVERED oraya-GİRMEZ.
    false
}

/// Alım-tarafı idempotent-dedup + SESSION_CLOSE-tombstone (§2.57).
#[derive(Debug, Default)]
pub struct InboundControlDedup {
    seen: HashSet<ControlId>,
    /// SESSION_CLOSE görülen id'ler: state-silinse-de retry'lara-ACK üretmek için bounded-tutulur.
    close_tombstones: HashSet<ControlId>,
    tombstone_cap: usize,
}

/// Bir gelen-control'un işlenme-sonucu (idempotent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundOutcome {
    /// İlk-görülüş: app/state-side-effect UYGULANIR.
    Fresh,
    /// Tekrar (aynı control_id): side-effect YOK; ACK yeniden-üretilebilir.
    DuplicateAckOnly,
}

impl InboundControlDedup {
    #[must_use]
    pub fn new(tombstone_cap: usize) -> Self {
        Self {
            seen: HashSet::new(),
            close_tombstones: HashSet::new(),
            tombstone_cap: tombstone_cap.max(1),
        }
    }

    /// Gelen-control'u idempotent işle. İlk-görülüş → Fresh (side-effect uygula); tekrar → DuplicateAckOnly.
    /// SESSION_CLOSE ayrıca tombstone'a yazılır (bounded).
    pub fn admit(&mut self, c: &Control) -> InboundOutcome {
        let id = c.control_id();
        if c.codepoint() == inner_kind::SESSION_CLOSE {
            if self.close_tombstones.len() >= self.tombstone_cap
                && !self.close_tombstones.contains(&id)
            {
                // Bounded: en-basit politika — cap dolu + yeni-id → yine ACK-üret ama tombstone-ekleme.
                // (Gerçek-eviction Sprint-4 session-TTL'iyle; burada bounded-guard yeterli.)
            } else {
                self.close_tombstones.insert(id);
            }
        }
        if self.seen.insert(id) {
            InboundOutcome::Fresh
        } else {
            InboundOutcome::DuplicateAckOnly
        }
    }

    /// SESSION_CLOSE-tombstone'u var mı (state-silindikten-sonra-bile ACK üretebilmek için).
    #[must_use]
    pub fn has_close_tombstone(&self, id: ControlId) -> bool {
        self.close_tombstones.contains(&id)
    }

    #[must_use]
    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }
}
