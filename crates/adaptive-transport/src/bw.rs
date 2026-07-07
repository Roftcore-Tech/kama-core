//! TV-BW: delivered-rate bandwidth estimator (engine spec §7).
//!
//! **bandwidth = delivered-rate'ten ÖLÇÜLÜR, loss'tan ASLA türetilmez**
//! (INV-ENGINE-BW; INV-RATE-1 ailesi). BBR delivery-rate sampler (§7.1) + max-filter
//! BtlBw (§7.2). Kısa-interval / app-limited / dış-flag sample'lar validity-gate'lenir
//! (§7.3). Saf/numerik + deterministik (ADR-17): monoton u64 ns zaman damgaları,
//! integer bytes/sec.

use std::time::Duration;

/// BtlBw max-filter penceresi `[STD: BBR ~10 RTT]`.
const W_BTLBW: Duration = Duration::from_secs(10);
/// Çok kısa interval → güvenilmez sample `[STD: BBR ~1ms]`.
const MIN_RATE_INTERVAL: Duration = Duration::from_millis(1);

/// `Duration` → u64 nanosecond.
fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

/// Gönderim anında bir pakete iliştirilen delivery-rate snapshot'ı (BBR §7.1). Opak
/// token: alanları private, dışarıdan üretilemez.
#[derive(Debug, Clone, Copy)]
pub struct SentPacket {
    size: u64,
    delivered: u64,
    delivered_time_ns: u64,
    first_sent_ns: u64,
    sent_ns: u64,
    is_app_limited: bool,
}

/// Bir ACK'ten türeyen delivery-rate örneği (engine §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateSample {
    /// Bu interval'de teslim edilen byte.
    pub delivered: u64,
    /// Örnekleme interval'i = max(ack_elapsed, send_elapsed).
    pub interval: Duration,
    /// Snapshot app-limited miydi (§7.3).
    pub is_app_limited: bool,
}

/// BBR delivery-rate sampler (engine §7.1).
#[derive(Debug, Clone, Default)]
pub struct DeliveryRateSampler {
    delivered: u64,
    delivered_time_ns: u64,
    first_sent_ns: u64,
    packets_in_flight: u64,
}

impl DeliveryRateSampler {
    /// Boş sampler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Paket gönderimi → snapshot token (engine §7.1). Pipe boşken first_sent /
    /// delivered_time tazelenir.
    pub fn on_sent(&mut self, now_ns: u64, size: u64, app_limited: bool) -> SentPacket {
        if self.packets_in_flight == 0 {
            self.first_sent_ns = now_ns;
            self.delivered_time_ns = now_ns;
        }
        self.packets_in_flight += 1;
        SentPacket {
            size,
            delivered: self.delivered,
            delivered_time_ns: self.delivered_time_ns,
            first_sent_ns: self.first_sent_ns,
            sent_ns: now_ns,
            is_app_limited: app_limited,
        }
    }

    /// Paket ACK'i → delivery-rate örneği (engine §7.1).
    /// `interval = max(ack_elapsed, send_elapsed)`, `delivered = C.delivered − P.delivered`.
    pub fn on_ack(&mut self, now_ns: u64, pkt: SentPacket) -> RateSample {
        self.delivered += pkt.size;
        self.delivered_time_ns = now_ns;
        self.packets_in_flight = self.packets_in_flight.saturating_sub(1);

        let ack_elapsed = now_ns.saturating_sub(pkt.delivered_time_ns);
        let send_elapsed = pkt.sent_ns.saturating_sub(pkt.first_sent_ns);
        let interval_ns = ack_elapsed.max(send_elapsed);
        self.first_sent_ns = pkt.sent_ns; // sonraki interval'in tabanı

        RateSample {
            delivered: self.delivered - pkt.delivered,
            interval: Duration::from_nanos(interval_ns),
            is_app_limited: pkt.is_app_limited,
        }
    }
}

/// BtlBw max-filter (engine §7.2/§7.3): geçerli sample'ların pencereli max'ı.
#[derive(Debug, Clone, Default)]
pub struct BtlBwFilter {
    /// (time_ns, bw_bps) örnekleri; pencereli max.
    samples: Vec<(u64, u64)>,
}

impl BtlBwFilter {
    /// Boş filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sample'ı validity-gate'leyip max-filter'ı günceller; güncel BtlBw'i (bps) döner.
    ///
    /// `valid=false` (reverse_path_impaired / ack_delay_untrusted — §7.3 dış flag'ler)
    /// veya `interval < MIN_RATE_INTERVAL` → sample max-filter'a GİRMEZ. app-limited
    /// sample yalnız mevcut BtlBw'den ≥ ise girer (yüksek app-limited = geçerli
    /// alt-sınır; §7.3).
    pub fn update(&mut self, sample: RateSample, now_ns: u64, valid: bool) -> u64 {
        if valid && sample.interval >= MIN_RATE_INTERVAL {
            let bw = bw_bps(sample.delivered, sample.interval);
            let app_limited_ok = !sample.is_app_limited || bw >= self.btlbw();
            if app_limited_ok {
                self.samples.push((now_ns, bw));
            }
        }
        let cutoff = now_ns.saturating_sub(dur_ns(W_BTLBW));
        self.samples.retain(|&(t, _)| t >= cutoff);
        self.btlbw()
    }

    /// Güncel BtlBw (bytes/sec); sample yoksa 0.
    #[must_use]
    pub fn btlbw(&self) -> u64 {
        self.samples.iter().map(|&(_, v)| v).max().unwrap_or(0)
    }
}

/// bw_sample = delivered · 1e9 / interval (bytes/sec, integer truncation). interval 0
/// ise 0.
#[must_use]
pub fn bw_bps(delivered: u64, interval: Duration) -> u64 {
    let ns = dur_ns(interval);
    if ns == 0 {
        return 0;
    }
    (u128::from(delivered) * 1_000_000_000 / u128::from(ns)) as u64
}
