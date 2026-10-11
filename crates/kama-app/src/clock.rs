//! Monoton saat — Application'ın zaman girdisi (K4G-08). Bugünkü tek tüketici PTT-R4'tür
//! (`ptt::PTT_STREAM_IDLE_TTL`); K4G-09'un dosya zamanlayıcıları (`FILE_IDLE_TTL`,
//! `FILE_RECEIPT_TTL`) aynı arayüzü kullanır. Saat kurulumda ENJEKTE edilir; motorlar duvar saatine
//! bakmaz ve kendileri uyumaz.
//!
//! Biçim `kama_noise_wire::MonotonicClock` ile AYNIDIR (`now()` keyfi bir başlangıçtan bu yana
//! geçen süre, asla geri gitmez), ama Application o crate'i göremez (INV-NEIGHBOR-1;
//! `scripts/ci/layer_check.py` L1/C2). Bu yüzden arayüz burada ayrıca tanımlıdır; bileşim kökü
//! (K4G-11) iki katmana kendi saatlerini ayrı ayrı verir.
//!
//! Üretimde [`SystemClock`] (`std::time::Instant` — işletim sisteminin monoton saati). Testler
//! arayüzü elle ilerletilen bir saatle gerçekler (`tests/ptt.rs`); bu crate'in test özelliği
//! (`test-util`) yoktur ve kütüphanede test saati tutulmaz.

use core::time::Duration;
use std::time::Instant;

/// Monoton zaman kaynağı: `now()` keyfi bir başlangıçtan bu yana geçen süredir ve ASLA geri gitmez.
pub trait MonotonicClock: Send + Sync {
    /// Başlangıçtan bu yana geçen süre.
    fn now(&self) -> Duration;
}

/// Üretim saati: kurulduğu andan bu yana geçen süre (`Instant`, monoton).
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// Başlangıcı şimdi olan saat.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
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
        // `Instant::elapsed` geri gitmez ve paniklemez: saat başlangıçtan gerideyse sıfır döner.
        self.origin.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_never_goes_back() {
        let clock = SystemClock::new();
        let first = clock.now();
        let second = clock.now();
        assert!(second >= first);
    }
}
