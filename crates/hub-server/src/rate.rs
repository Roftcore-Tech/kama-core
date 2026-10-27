//! Hız sınırı: token bucket — sayaç + son doldurma zamanı (KM-HUB-TDD-001 §4).
//!
//! Kullanıldığı yerler (adları S4 §6'dan): `VERIFY_BUDGET` (küresel), `STORE_RATE`, `LINK_ALLOC_RATE`,
//! `ROUTE_MISS_RATE` (uç başına; kovalar uç kaydında durur, uç tablosu dizinle sınırlıdır). Kesirli sayı
//! yok: jetonlar binde bir birimle tutulur.

use core::time::Duration;

/// Bir jetonun iç birim karşılığı.
const MILLI: u64 = 1000;

/// Bir hız sınırı: saniyede `per_second` jeton, en çok `burst` birikir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    pub per_second: u32,
    pub burst: u32,
}

impl Rate {
    #[must_use]
    pub const fn new(per_second: u32, burst: u32) -> Self {
        Self { per_second, burst }
    }
}

/// Token bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenBucket {
    /// Kalan jeton × 1000.
    milli_tokens: u64,
    /// Son dolumun zamanı (ms).
    filled_at_ms: u64,
}

impl TokenBucket {
    /// Dolu kova (`burst` jeton).
    #[must_use]
    pub fn full(rate: Rate, now: Duration) -> Self {
        Self {
            milli_tokens: u64::from(rate.burst).saturating_mul(MILLI),
            filled_at_ms: millis(now),
        }
    }

    /// Doldurur, sonra bir jeton harcar; kova boşsa `false` ve hiçbir şey harcanmaz.
    pub fn take(&mut self, rate: Rate, now: Duration) -> bool {
        self.refill(rate, now);
        match self.milli_tokens.checked_sub(MILLI) {
            Some(rest) => {
                self.milli_tokens = rest;
                true
            }
            None => false,
        }
    }

    /// Kalan tam jeton sayısı (yalnız gözlem; test ve istatistik).
    #[must_use]
    pub fn tokens(&self, rate: Rate, now: Duration) -> u64 {
        let mut copy = *self;
        copy.refill(rate, now);
        copy.milli_tokens / MILLI
    }

    fn refill(&mut self, rate: Rate, now: Duration) {
        let now_ms = millis(now);
        let elapsed = now_ms.saturating_sub(self.filled_at_ms);
        let capacity = u64::from(rate.burst).saturating_mul(MILLI);
        // 1 ms'de `per_second / 1000` jeton = `per_second` iç birim.
        self.milli_tokens = self
            .milli_tokens
            .saturating_add(elapsed.saturating_mul(u64::from(rate.per_second)))
            .min(capacity);
        self.filled_at_ms = self.filled_at_ms.max(now_ms);
    }
}

fn millis(at: Duration) -> u64 {
    u64::try_from(at.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORE: Rate = Rate::new(10, 20);
    const ALLOC: Rate = Rate::new(1, 10);

    #[test]
    fn burst_then_linear_refill() {
        let t0 = Duration::from_secs(5);
        let mut bucket = TokenBucket::full(STORE, t0);
        for n in 0..20 {
            assert!(bucket.take(STORE, t0), "{n}");
        }
        assert!(!bucket.take(STORE, t0));
        assert!(!bucket.take(STORE, t0 + Duration::from_millis(99)));
        assert!(bucket.take(STORE, t0 + Duration::from_millis(100)));
        assert!(!bucket.take(STORE, t0 + Duration::from_millis(100)));
    }

    #[test]
    fn slow_rate_with_a_large_burst() {
        let t0 = Duration::ZERO;
        let mut bucket = TokenBucket::full(ALLOC, t0);
        for _ in 0..10 {
            assert!(bucket.take(ALLOC, t0));
        }
        assert!(!bucket.take(ALLOC, t0 + Duration::from_millis(999)));
        assert!(bucket.take(ALLOC, t0 + Duration::from_secs(1)));
        assert_eq!(
            bucket.tokens(ALLOC, t0 + Duration::from_secs(3600)),
            10,
            "sığa aşılmaz"
        );
    }

    #[test]
    fn a_refused_take_spends_nothing_and_time_never_goes_back() {
        let mut bucket = TokenBucket::full(Rate::new(1, 1), Duration::from_secs(10));
        assert!(bucket.take(Rate::new(1, 1), Duration::from_secs(10)));
        assert!(
            !bucket.take(Rate::new(1, 1), Duration::from_secs(5)),
            "geri giden saat jeton üretmez"
        );
        assert!(bucket.take(Rate::new(1, 1), Duration::from_secs(11)));
        assert!(
            TokenBucket::full(Rate::new(u32::MAX, u32::MAX), Duration::MAX)
                .take(Rate::new(u32::MAX, u32::MAX), Duration::MAX)
        );
    }
}
