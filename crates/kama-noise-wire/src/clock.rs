//! Monoton saat (sahip kararı K2, 2026-10-08): Crypto'nun zaman girdisi — epoch grace'inin `T_g`'si,
//! HALF_OPEN / PROVISIONAL TTL'leri ve SESSION_MAX_AGE buradan okunur. Kurulumda ENJEKTE edilir;
//! Crypto duvar saatine ve ağ zamanlayıcısına bakmaz (S3 K9: retransmisyon zamanlayıcısı
//! Transport'tadır).
//!
//! Üretimde [`SystemClock`] (`std::time::Instant` — işletim sisteminin monoton saati). Testte
//! [`ManualClock`]: elle ilerletilir, kopyaları AYNI zamanı paylaşır (test bir kopyayı tutar,
//! diğerini Crypto'ya verir).

use core::time::Duration;

/// Monoton zaman kaynağı: `now()` keyfi bir başlangıçtan bu yana geçen süredir ve ASLA geri gitmez.
pub trait MonotonicClock: Send + 'static {
    fn now(&self) -> Duration;
}

/// Üretim saati: kurulduğu andan bu yana geçen süre (`Instant`, monoton).
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: std::time::Instant,
}

impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: std::time::Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// Elle sürülen saat — YALNIZ TEST. Kopyalar aynı zamanı paylaşır; zaman yalnız ileri gider.
#[cfg(any(test, feature = "test-util"))]
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    nanos: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[cfg(any(test, feature = "test-util"))]
impl ManualClock {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Zamanı `by` kadar ilerletir (u64 nanosaniyede doyar; geri gitmez).
    pub fn advance(&self, by: Duration) {
        let step = u64::try_from(by.as_nanos()).unwrap_or(u64::MAX);
        let _ = self.nanos.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |now| Some(now.saturating_add(step)),
        );
    }
}

#[cfg(any(test, feature = "test-util"))]
impl MonotonicClock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(std::sync::atomic::Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_copies_share_time_and_never_go_back() {
        let test_side = ManualClock::new();
        let injected = test_side.clone();
        assert_eq!(injected.now(), Duration::ZERO);
        test_side.advance(Duration::from_secs(30));
        assert_eq!(injected.now(), Duration::from_secs(30));
        test_side.advance(Duration::MAX);
        assert_eq!(
            injected.now(),
            Duration::from_nanos(u64::MAX),
            "doyar, sarmaz"
        );
    }

    #[test]
    fn system_clock_is_monotonic() {
        let c = SystemClock::new();
        let a = c.now();
        let b = c.now();
        assert!(b >= a);
    }
}
