//! F3 (DR-CS-1b/A2; full-scale-directive §9) — CONTROL-ID FIXED-WINDOW DEDUP.
//!
//! Session+direction-scoped bounded dedup: `highest_control_id` (high-water) + sabit-boyut bitmap
//! (CONTROL_DEDUP_WINDOW; CAL-10-SEALED=4096; pn.rs ReplayWindow modular-index deseni). session-scoped-
//! HashSet (unbounded) YERİNE (Abkarada D-5). Probe/echo (0x19/0x1A) BU pencereye GİRMEZ (A-2 caller-
//! dispatch). Closing'de pencere düşer; CLOSE-retry tombstone-yoluna gider (F3.5).

use crate::control_id_gate::CONTROL_DEDUP_WINDOW;
use kama_transport_schema::control::ControlId;

const WINDOW: u32 = CONTROL_DEDUP_WINDOW;
const WORDS: usize = (CONTROL_DEDUP_WINDOW as usize) / 64;

/// Dedup kararı (Abkarada §9 semantiği).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupDecision {
    /// İlk-görülüş: side-effect uygulanır (yeni-logical-control).
    Fresh,
    /// Görülmüş/eski (window-içi-marked VEYA window-altı): idempotent — yalnız-ACK-yeniden-üret.
    DuplicateAckOnly,
}

/// Fixed-window control-id dedup (per session/direction).
#[derive(Debug, Clone)]
pub struct ControlDedupWindow {
    highest: u32,
    initialized: bool,
    bitmap: Box<[u64; WORDS]>,
    /// A-3 teşhis: window-altına düşen id sayısı (ControlIdBelowDedupWindow — sizing-varsayımı-ihlali).
    below_window_count: u64,
}

impl Default for ControlDedupWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlDedupWindow {
    #[must_use]
    pub fn new() -> Self {
        Self {
            highest: 0,
            initialized: false,
            bitmap: Box::new([0u64; WORDS]),
            below_window_count: 0,
        }
    }

    /// window-altı-düşen id sayısı (teşhis; sizing-varsayımı-ihlali göstergesi).
    #[must_use]
    pub fn below_window_count(&self) -> u64 {
        self.below_window_count
    }

    fn idx(id: u32) -> (usize, u64) {
        let i = (id % WINDOW) as usize;
        (i / 64, 1u64 << (i % 64))
    }

    fn is_marked(&self, id: u32) -> bool {
        let (w, b) = Self::idx(id);
        self.bitmap[w] & b != 0
    }

    fn mark(&mut self, id: u32) {
        let (w, b) = Self::idx(id);
        self.bitmap[w] |= b;
    }

    /// high-water'ı `new_high`'a ilerletirken aradaki (eski+1 ..= new_high) slot'ları TEMİZLE (yeni-aralık
    /// Fresh olmalı; modular-bitmap yeniden-kullanım). Atlama ≥ WINDOW ise tüm-bitmap temizlenir.
    fn advance_to(&mut self, new_high: u32) {
        let span = new_high.wrapping_sub(self.highest);
        if span >= WINDOW {
            self.bitmap.iter_mut().for_each(|w| *w = 0);
        } else {
            let mut c = self.highest.wrapping_add(1);
            while c != new_high.wrapping_add(1) {
                let (w, b) = Self::idx(c);
                self.bitmap[w] &= !b;
                c = c.wrapping_add(1);
            }
        }
        self.highest = new_high;
    }

    /// Dedup-kararı (Abkarada §9):
    /// - id > highest → Fresh (high-water ilerler, bitmap işaretlenir)
    /// - window-içi marked → DuplicateAckOnly · window-içi unmarked → Fresh
    /// - window-altı → DuplicateAckOnly + below-window teşhis
    pub fn check(&mut self, id: ControlId) -> DedupDecision {
        if !self.initialized {
            self.initialized = true;
            self.highest = id;
            self.mark(id);
            return DedupDecision::Fresh;
        }
        if id > self.highest {
            self.advance_to(id);
            self.mark(id);
            return DedupDecision::Fresh;
        }
        // id ≤ highest: window-içi mi altı mı?
        if id <= self.highest.saturating_sub(WINDOW - 1) {
            // window-altı (evict-edilmiş aralık): fail-safe idempotent + teşhis.
            self.below_window_count = self.below_window_count.saturating_add(1);
            return DedupDecision::DuplicateAckOnly;
        }
        if self.is_marked(id) {
            DedupDecision::DuplicateAckOnly
        } else {
            self.mark(id);
            DedupDecision::Fresh
        }
    }
}
