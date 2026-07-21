//! 5d-1 bounded pre-param hold (§8 + SR-AUDIT-001 bounds-before-alloc).
//!
//! Authenticated-AMA-PARAM_SET-öncesi gelen `OpenedTransportShard`'ları (AEAD-geçmiş; forged-DEĞİL)
//! bounded-tutar. **BOUNDED:** `len ≥ MAX_PREPARAM_SHARDS` → en-eski-drop + audit-sayaç (unbounded-alloc
//! YOK). Authenticated-PARAM_SET bind edilince [`PreParamHold::drain`] ile decode-yoluna release.
//!
//! NOT: yalnız AUTHENTICATED shard tutulur (open() AEAD-verify sonrası); unauthenticated-buffering DEĞİL.

use kama_noise_wire::OpenedTransportShard;
use std::collections::VecDeque;

/// Pre-param hold üst-sınırı (bounded; loss.rs LEDGER_MAX/sender.rs TOMBSTONE_MAX kalıbı).
pub const MAX_PREPARAM_SHARDS: usize = 256;

/// PARAM-öncesi authenticated-shard'lar için bounded tampon (en-eski-drop).
#[derive(Debug, Default)]
pub struct PreParamHold {
    hold: VecDeque<OpenedTransportShard>,
    dropped: u64,
}

impl PreParamHold {
    #[must_use]
    pub fn new() -> Self {
        Self {
            hold: VecDeque::new(),
            dropped: 0,
        }
    }

    /// PARAM-öncesi authenticated-shard'ı tut. **Bounded:** `len ≥ MAX` → en-eski-drop + audit++.
    pub fn hold(&mut self, shard: OpenedTransportShard) {
        if self.hold.len() >= MAX_PREPARAM_SHARDS {
            self.hold.pop_front(); // en-eski-drop (SR-AUDIT-001: bounds-before-alloc)
            self.dropped += 1;
        }
        self.hold.push_back(shard);
    }

    /// PARAM confirmed → tutulan tüm shard'ları release (decode-yoluna). Tampon boşalır.
    pub fn drain(&mut self) -> Vec<OpenedTransportShard> {
        self.hold.drain(..).collect()
    }

    /// Tutulan-shard sayısı.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hold.len()
    }

    /// Tampon boş mu?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hold.is_empty()
    }

    /// Aşımda düşürülen-shard sayısı (audit; bounded-kanıtı).
    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped
    }
}
