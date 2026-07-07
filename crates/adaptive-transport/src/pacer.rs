//! TV-PACE: token-bucket pacing (engine §13). Deterministik (ADR-17).
//!
//! `pace_rate` (§8.2/§8.5 SRC çıktısı) hızında token doldurulur; paket ancak yeterli
//! token varsa gönderilir. BURST_CAP micro-burst'ü sınırlar.

use std::time::Duration;

/// BURST_CAP (bytes) ≈ 2×MTU [STD §13 "≈1-2×MTU"]; [CAL] rafine sonra.
pub const DEFAULT_BURST_CAP: u64 = 2 * crate::rate::DEFAULT_MTU;

fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

/// Token-bucket pacer (engine §13): `rate·Δt` refill, BURST_CAP cap, size-based drain.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    rate_bps: u64,
    tokens: u64,
    burst_cap: u64,
}

impl TokenBucket {
    /// `rate_bps` pacing hızı, `burst_cap` micro-burst sınırı. Token dolu başlar.
    #[must_use]
    pub fn new(rate_bps: u64, burst_cap: u64) -> Self {
        Self {
            rate_bps,
            tokens: burst_cap,
            burst_cap,
        }
    }

    /// pacing hızını günceller (SRC send_rate_final değişince).
    pub fn set_rate(&mut self, rate_bps: u64) {
        self.rate_bps = rate_bps;
    }

    /// `dt` süre geçişinde `rate·dt` token doldurur, BURST_CAP ile clamp.
    pub fn refill(&mut self, dt: Duration) {
        let add = (u128::from(self.rate_bps) * u128::from(dur_ns(dt)) / 1_000_000_000) as u64;
        self.tokens = self.tokens.saturating_add(add).min(self.burst_cap);
    }

    /// `size` byte'lık paket gönderilebilir mi; gönderilirse token düşer.
    pub fn try_send(&mut self, size: u64) -> bool {
        if self.tokens >= size {
            self.tokens -= size;
            true
        } else {
            false
        }
    }

    /// Güncel token seviyesi (bytes).
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.tokens
    }
}
