//! F-CID-0 (DR-CS-1b/A2; Abkarada full-scale-directive §8) — CONTROL-ID MERKEZİ ISSUANCE-GATE +
//! HARD-BOUND'lar + CAL-10 dedup-window türetimi.
//!
//! PRODUCER-INVENTORY (control_id-tüketen üreticiler; `Control` §2.57 varyantları):
//!   Ack(0x10) · Sack(0x11) · Recovered(0x12) · BlockComplete(0x13) · ParamSet(0x14) ·
//!   ParamAck(0x15) · BlockAbort(0x16) · SessionClose(0x17) · SessionRenewRequired(0x18).
//! Atama-yolu: `control::split_for_send(c, next_id)` (control.rs:194) — bugüne kadar `next_id`
//! CALLER'dan (harness). Üretimde merkezi-atayıcı YOKtu → BU gate onu sağlar (A-3 monoton kontratı).
//! CsProbe(0x19)/CsEcho(0x1A) control_id-dedup'a GİRMEZ (A-2; stateless) → gate'e de girmez.
//!
//! HARD-değerler GÖZLEMSEL-ORTALAMA DEĞİL — return-supply kota/split-cap enforce-max'larından türetilir
//! (kod/spec). CAL-10 required-window bu sabitlerden hesaplanır (freeze-r2 CONTROL_DEDUP_WINDOW=4096
//! CANDIDATE'ini doğrular/mühürler).

use kama_transport_schema::control::ControlId;

// ── HARD-BOUND'lar (F-CID-0; CAL-10 girdileri) ───────────────────────────────────────────────────

/// Yön-başına HARD control-id issuance-rate üst-sınırı (id/s). Return-supply kotası (QUOTA_R_DEFAULT=4
/// epizot-başı) × control-tip-çeşitliliği + RECOVERED-return-supply payı → konservatif 32 id/s HARD.
pub const MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION: u32 = 32;

/// Tek emisyon-batch'inde üretilebilecek HARD max control-id (burst). Worst-case: ACK-split
/// (MAX_ACK_RANGES=16 → ceil chunks) + SACK-split (MAX_SACK ~32) + RECOVERED (16) → konservatif 64.
pub const MAX_CONTROL_ID_BURST: u32 = 64;

/// Ağ-reorder toleransı (control-datagram out-of-order derinliği).
pub const MAX_CONTROL_REORDER_ALLOWANCE: u32 = 16;

/// Retry + reorder zaman-horizonu (s). PARAM_SET/CLOSE bounded-retry + patience-penceresi (F0-(viii)
/// max 105s) → retry'lar bu horizon içinde aynı control_id'yi taşır (§2.57 retry=aynı-id).
pub const MAX_RETRY_REORDER_HORIZON_S: u32 = 105;

/// CAL-10: required_window = RATE × HORIZON + BURST + REORDER_ALLOWANCE (§8.3).
const REQUIRED_WINDOW_RAW: u32 = MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION
    * MAX_RETRY_REORDER_HORIZON_S
    + MAX_CONTROL_ID_BURST
    + MAX_CONTROL_REORDER_ALLOWANCE;

/// Uygun power-of-two dedup-window (≥ required_window). 3440 → 4096 (freeze-r2 CANDIDATE doğrulandı).
pub const CONTROL_DEDUP_WINDOW: u32 = REQUIRED_WINDOW_RAW.next_power_of_two();

// Derleme-zamanı: 4096 gerçekten required-window'u karşılıyor mu (freeze-r2 CANDIDATE mührü).
const _: () = assert!(CONTROL_DEDUP_WINDOW >= REQUIRED_WINDOW_RAW);
const _: () = assert!(CONTROL_DEDUP_WINDOW == 4096);

/// Issue-gate hataları (typed; reachable panic-YASAK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlIdIssueError {
    /// Bu emisyon-batch'i MAX_CONTROL_ID_BURST'ü aştı (fail-closed; caller batch'i bölmeli).
    BurstExceeded,
    /// control_id-uzayı wrap-eşiğine ulaştı → SESSION_RENEW gerekli (§2.57 wrap-öncesi-renewal).
    RenewalRequired,
}

/// F-CID-0 merkezi control-id atayıcı: MONOTON (A-3) + per-batch BURST-enforce + wrap→renewal.
/// Rate'in zaman-katmanı enforce'u emisyon-tick'inde (F4.5-kapısı); bu tip A-3 kontratını ve burst-hard-
/// sınırını yapısal sağlar. session+direction-scoped (her Session-yön için ayrı gate).
#[derive(Debug, Clone)]
pub struct ControlIdIssueGate {
    next_id: u32,
    burst_in_batch: u32,
    /// wrap-eşiği: bu id'ye ulaşınca RenewalRequired (§2.6 cap/wrap-öncesi session-renewal).
    wrap_threshold: u32,
}

impl Default for ControlIdIssueGate {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlIdIssueGate {
    /// Yeni gate (id=0'dan; wrap-eşiği u32::MAX — pratik session-ömrü içinde ulaşılmaz).
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 0,
            burst_in_batch: 0,
            wrap_threshold: u32::MAX,
        }
    }

    /// Erken wrap-eşiğiyle gate (renewal-yolu; production: gerçek session-cap/wrap eşiği).
    #[must_use]
    pub fn with_wrap_threshold(wrap_threshold: u32) -> Self {
        Self {
            next_id: 0,
            burst_in_batch: 0,
            wrap_threshold,
        }
    }

    /// YENİ logical-control için control_id atar (monoton). retry AYNI-id kullanır (bu fn ÇAĞRILMAZ).
    ///
    /// # Errors
    /// [`ControlIdIssueError::BurstExceeded`] · [`ControlIdIssueError::RenewalRequired`].
    pub fn issue(&mut self) -> Result<ControlId, ControlIdIssueError> {
        if self.burst_in_batch >= MAX_CONTROL_ID_BURST {
            return Err(ControlIdIssueError::BurstExceeded);
        }
        if self.next_id >= self.wrap_threshold {
            return Err(ControlIdIssueError::RenewalRequired);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.burst_in_batch += 1;
        Ok(id)
    }

    /// Emisyon-batch sınırı: burst-sayacını sıfırlar (her on_event_batch/emisyon-tick sonu).
    pub fn end_batch(&mut self) {
        self.burst_in_batch = 0;
    }

    /// Son atanan id'nin bir sonrası (dedup-window high-water hizalaması; teşhis).
    #[must_use]
    pub fn next_id(&self) -> u32 {
        self.next_id
    }
}
