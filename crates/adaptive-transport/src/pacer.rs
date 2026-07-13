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
    /// [R1/ADIM-6] refill artık-kalanı (byte-ns birimi, <1e9). Sub-byte-per-tick birikimini taşır:
    /// `rate·dt < 1e9` (rate<1000 B/s @ 1ms tick) durumunda tamsayı-truncation ile atılan kesir burada
    /// birikir → floor pacing (31 B/s) gerçekten token üretir; PreConverge stall'ının kök nedeni kalkar.
    refill_rem: u64,
}

impl TokenBucket {
    /// `rate_bps` pacing hızı, `burst_cap` micro-burst sınırı. Token dolu başlar.
    #[must_use]
    pub fn new(rate_bps: u64, burst_cap: u64) -> Self {
        Self {
            rate_bps,
            tokens: burst_cap,
            burst_cap,
            refill_rem: 0,
        }
    }

    /// pacing hızını günceller (SRC send_rate_final değişince).
    pub fn set_rate(&mut self, rate_bps: u64) {
        self.rate_bps = rate_bps;
    }

    /// `dt` süre geçişinde `rate·dt` token doldurur, BURST_CAP ile clamp.
    ///
    /// [R1/ADIM-6] SUB-BYTE PRECISION: `numer = rate·dt_ns + kalan` (byte-ns); tam token = `numer/1e9`,
    /// kalan `numer%1e9` SONRAKİ refill'e taşınır (<1e9, bounded). Eski `rate·dt/1e9` per-tick
    /// truncation'ı rate<1000 B/s'te (31 B/s @ 1ms = 0.031→0) her tick 0 üretip pacer'ı kilitliyordu.
    pub fn refill(&mut self, dt: Duration) {
        let numer =
            u128::from(self.rate_bps) * u128::from(dur_ns(dt)) + u128::from(self.refill_rem);
        let add = (numer / 1_000_000_000) as u64;
        self.refill_rem = (numer % 1_000_000_000) as u64;
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

    /// [R3/ADIM-6] Burst kapasitesini KANAL-TÜREVLİ değere ayarlar + token'ı cap'e clamp (idle-dump önle).
    /// Bant-kör 2×MTU=2400B sabiti yerine çağıran `max(1 birim, rate·pencere)` verir → sub-kbps'te 1-birim
    /// (backlog-spike yok), hızlı kanalda micro-burst. cap≥1 (deadlock önle).
    pub fn set_burst_cap(&mut self, cap: u64) {
        self.burst_cap = cap.max(1);
        self.tokens = self.tokens.min(self.burst_cap);
    }

    /// [R3/ADIM-6] Cold-start çıkışında micro-burst grant'ı: token'ı (kanal-türevli) burst_cap'e doldurur.
    /// PRE_CONVERGE→STARTUP (§3.3-D): kuyruktaki birimler kanal-uygun küçük burst'le boşalır (eski
    /// `restore_burst`'ün bant-kör 2400 dump'ı yerine — set_burst_cap kanal-türevli sınırı belirler).
    pub fn fill_to_cap(&mut self) {
        self.tokens = self.burst_cap;
    }

    /// Güncel token seviyesi (bytes).
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    /// Güncel pacing rate (bytes/sec) — SALT-GÖZLEM (teşhis).
    #[must_use]
    pub fn rate(&self) -> u64 {
        self.rate_bps
    }
}
