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

/// [HAT-MODELİ] Bilinen darboğaz hattı: dağıtım profilinin yukarı yön hızı ve pacer'ın saymadığı
/// datagram zarfı. Ölçülmez, deterministiktir (ADR-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineModel {
    /// Hat hızı (B/s, zarf dahil).
    pub rate_bps: u64,
    /// Datagram başına zarf (B; UDP/IP), pacer'ın saydığı gövdenin üstüne.
    pub datagram_overhead: u64,
}

#[derive(Debug, Clone, Copy)]
struct LineState {
    model: LineModel,
    /// Hatta henüz serileşmemiş kendi baytlarımız (zarf dahil).
    backlog: u64,
    /// Boşaltma artık-kalanı (byte-ns, < 1e9; `refill_rem` ile aynı sabit nokta).
    rem: u64,
}

impl LineState {
    /// Bekletmeden kabul edilen kendi kuyruk payı: quinn pacer'ının burst aralığı
    /// (`cc::BURST_INTERVAL_NS`, 2 ms) kadar hat süresi. Dar hatta bu bir datagramdan azdır (bir
    /// sonraki datagram öncekinin serileşmesi bitince çıkar); hızlı hatta zamanlayıcı çözünürlüğü
    /// hattı boşta bırakmaz.
    fn allowance(&self) -> u64 {
        u64::try_from(
            u128::from(self.model.rate_bps) * crate::cc::BURST_INTERVAL_NS / 1_000_000_000,
        )
        .unwrap_or(u64::MAX)
    }

    /// Kuyruk `target` bayta inene dek geçecek süre (ns; tamsayı, yukarı yuvarlanır).
    fn ns_until_backlog(&self, target: u64) -> u64 {
        if self.backlog <= target {
            return 0;
        }
        let need = u128::from(self.backlog - target) * 1_000_000_000u128;
        let deficit = need.saturating_sub(u128::from(self.rem));
        u64::try_from(deficit.div_ceil(u128::from(self.model.rate_bps))).unwrap_or(u64::MAX)
    }
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
    /// [HAT-MODELİ] Kurulu hat (`None` = model yok, bugünkü kova aynen).
    line: Option<LineState>,
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
            line: None,
        }
    }

    /// [HAT-MODELİ] Bilinen darboğaz hattını kurar: kovanın verdiği her datagram (gövde + zarf)
    /// hatta kendi sırasıyla serileşir ve kendi baytlarımızın hattaki kuyruğu bir burst aralığını
    /// aşarken kova datagram vermez. Kova hızı ve burst'ü bu sınırın altında aynen işler; model
    /// yalnız kovanın hattın taşıyabileceğinden fazlasını darboğaz kuyruğuna bırakmasını keser.
    /// `None` ya da hız 0 = model yok. Kurulum kuyruğu sıfırlar (oturum başında çağrılır).
    pub fn set_line(&mut self, model: Option<LineModel>) {
        self.line = model.filter(|m| m.rate_bps > 0).map(|model| LineState {
            model,
            backlog: 0,
            rem: 0,
        });
    }

    /// [HAT-MODELİ] Kovanın dışında (jetondan muaf) tele çıkan datagramı hat kuyruğuna yazar.
    pub fn line_charge(&mut self, size: u64) {
        if let Some(l) = self.line.as_mut() {
            l.backlog = l
                .backlog
                .saturating_add(size.saturating_add(l.model.datagram_overhead));
        }
    }

    /// [HAT-MODELİ] Hat modeli kurulu mu.
    #[must_use]
    pub fn has_line(&self) -> bool {
        self.line.is_some()
    }

    /// [HAT-MODELİ] Hat yeni datagram alabilir mi (model yoksa daima).
    #[must_use]
    pub fn line_is_open(&self) -> bool {
        self.line.is_none_or(|l| l.backlog <= l.allowance())
    }

    /// [HAT-MODELİ] Hattın yeni datagram alacağı ana kalan süre (ns; model yoksa 0).
    #[must_use]
    pub fn line_ns_until_open(&self) -> u64 {
        self.line.map_or(0, |l| l.ns_until_backlog(l.allowance()))
    }

    /// [HAT-MODELİ] Kendi baytlarımızın hatta serileşmesinin biteceği ana kalan süre (ns; model
    /// yoksa 0).
    #[must_use]
    pub fn line_ns_until_drained(&self) -> u64 {
        self.line.map_or(0, |l| l.ns_until_backlog(0))
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
        if let Some(l) = self.line.as_mut() {
            // Hat kendi hızında boşalır; boş hat kredi biriktirmez (artık sıfırlanır).
            let numer = u128::from(l.model.rate_bps) * u128::from(dur_ns(dt)) + u128::from(l.rem);
            let drained = u64::try_from(numer / 1_000_000_000).unwrap_or(u64::MAX);
            if drained >= l.backlog {
                l.backlog = 0;
                l.rem = 0;
            } else {
                l.backlog -= drained;
                l.rem = (numer % 1_000_000_000) as u64;
            }
        }
    }

    /// `size` byte'lık paket gönderilebilir mi; gönderilirse token düşer.
    pub fn try_send(&mut self, size: u64) -> bool {
        if !self.line_is_open() {
            return false;
        }
        if self.tokens >= size {
            self.tokens -= size;
            self.line_charge(size);
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

    /// Güncel burst kapasitesi (bytes) — SALT-GÖZLEM (teşhis).
    #[must_use]
    pub fn burst_cap(&self) -> u64 {
        self.burst_cap
    }

    /// Güncel pacing rate (bytes/sec) — SALT-GÖZLEM (teşhis).
    #[must_use]
    pub fn rate(&self) -> u64 {
        self.rate_bps
    }

    /// EK4-A2 (event-loop liveness): `size` byte'ın gönderilebilir olmasına kalan süre (ns).
    ///
    /// **SALT-OKUMA — yeni timer DEĞİL.** Mevcut `tokens/rate/refill_rem/burst_cap` durumundan
    /// `refill()`'in TAM tersini hesaplar (aynı sub-byte fixed-point aritmetiği; ADR-17: float yok):
    /// `refill` `tokens += (rate·dt + rem) / 1e9` yaptığı için, gereken `dt` en küçük tamsayıdır ki
    /// `rate·dt + rem ≥ need·1e9`.
    ///
    /// * `Some(0)`  — şimdi gönderilebilir.
    /// * `Some(ns)` — token yenilenmesiyle bu kadar sonra gönderilebilir.
    /// * `None`     — ASLA gönderilemez (rate 0, ya da `size > burst_cap` ⇒ token hiç yetişemez).
    ///   Çağıran bunu deadline olarak KULLANMAMALI; ilerleme başka bir olaya bağlıdır.
    ///
    /// [HAT-MODELİ] Kurulu hat varken yanıt hattın açılmasını da bekler (iki koşulun geç olanı).
    #[must_use]
    pub fn ns_until_sendable(&self, size: u64) -> Option<u64> {
        let line = self.line_ns_until_open();
        if self.tokens >= size {
            return Some(line);
        }
        if self.rate_bps == 0 || size > self.burst_cap {
            return None; // token bu boyuta hiçbir zaman yetişmez → zaman-güdümlü ilerleme yok
        }
        let need = u128::from(size - self.tokens) * 1_000_000_000u128;
        let rem = u128::from(self.refill_rem);
        let deficit = need.saturating_sub(rem);
        let rate = u128::from(self.rate_bps);
        // ceil(deficit / rate) — tamsayı, yukarı yuvarlama (erken uyanma yok).
        let dt = deficit.div_ceil(rate);
        Some(u64::try_from(dt).unwrap_or(u64::MAX).max(line))
    }
}
