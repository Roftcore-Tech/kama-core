//! TV-RTT: RTT / RTprop / queue-delay estimator (engine spec §6).
//!
//! srtt/rttvar EWMA (RFC 6298 türevi, α=1/8 β=1/4 `[STD]`); RTprop min-filter
//! (BBR-türevi, W_rtprop `[STD]`); queue_delay = srtt − RTprop (congestion sinyali,
//! loss DEĞİL). Saf/numerik + deterministik (ADR-17): integer nanosecond, tek-bölmeli
//! exact EWMA form. `jitter_score` / `reordering_score` fuzzy adımına (§8) ertelendi
//! (`reordering_score` `[CAL]` içerir).

use std::time::Duration;

/// srtt EWMA payda: α = 1/8 `[STD: RFC 6298]`.
const SRTT_ALPHA_DEN: u128 = 8;
/// rttvar EWMA payda: β = 1/4 `[STD: RFC 6298]`.
const RTTVAR_BETA_DEN: u128 = 4;
/// RTprop min-filter penceresi `[STD: BBR 10s]`.
const W_RTPROP: Duration = Duration::from_secs(10);

/// `Duration` → u64 nanosecond (transport süreleri saniyeler altında).
fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

/// Kayan zaman penceresinde min-filter (RTprop için).
#[derive(Debug, Clone, Default)]
struct MinFilter {
    /// (time_ns, value_ns) örnekleri; pencere dışı at.
    samples: Vec<(u64, u64)>,
}

impl MinFilter {
    fn observe(&mut self, now_ns: u64, value_ns: u64, window_ns: u64) {
        self.samples.push((now_ns, value_ns));
        let cutoff = now_ns.saturating_sub(window_ns);
        self.samples.retain(|&(t, _)| t >= cutoff);
    }

    fn min(&self) -> Option<u64> {
        self.samples.iter().map(|&(_, v)| v).min()
    }
}

/// RTT / RTprop / queue-delay estimator (engine spec §6).
#[derive(Debug, Clone, Default)]
pub struct RttEstimator {
    /// Smoothed RTT (ns); `None` = henüz sample yok.
    srtt_ns: Option<u64>,
    /// RTT varyansı (ns).
    rttvar_ns: u64,
    /// RTprop min-filter.
    rtprop: MinFilter,
}

impl RttEstimator {
    /// Boş estimator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `now_ns` anında ölçülen bir `sample`'ı işler (RFC 6298 sırası: rttvar ÖNCE
    /// eski srtt ile, srtt SONRA).
    pub fn observe(&mut self, now_ns: u64, sample: Duration) {
        let s = dur_ns(sample);
        match self.srtt_ns {
            None => {
                // İlk sample: srtt = s, rttvar = s/2.
                self.srtt_ns = Some(s);
                self.rttvar_ns = s / 2;
            }
            Some(srtt) => {
                let diff = srtt.abs_diff(s);
                // rttvar = (1−β)·rttvar + β·|srtt_old − s|  → tek bölme, exact.
                self.rttvar_ns = (((RTTVAR_BETA_DEN - 1) * u128::from(self.rttvar_ns)
                    + u128::from(diff))
                    / RTTVAR_BETA_DEN) as u64;
                // srtt = (1−α)·srtt + α·s
                self.srtt_ns = Some(
                    (((SRTT_ALPHA_DEN - 1) * u128::from(srtt) + u128::from(s)) / SRTT_ALPHA_DEN)
                        as u64,
                );
            }
        }
        self.rtprop.observe(now_ns, s, dur_ns(W_RTPROP));
    }

    /// Smoothed RTT; henüz sample yoksa `None`.
    #[must_use]
    pub fn srtt(&self) -> Option<Duration> {
        self.srtt_ns.map(Duration::from_nanos)
    }

    /// RTT varyansı.
    #[must_use]
    pub fn rttvar(&self) -> Duration {
        Duration::from_nanos(self.rttvar_ns)
    }

    /// RTprop (pencereli min RTT); henüz sample yoksa `None`.
    #[must_use]
    pub fn rtprop(&self) -> Option<Duration> {
        self.rtprop.min().map(Duration::from_nanos)
    }

    /// queue_delay = srtt − RTprop (≥0; congestion sinyali, loss DEĞİL). Veri yoksa 0.
    #[must_use]
    pub fn queue_delay(&self) -> Duration {
        match (self.srtt_ns, self.rtprop.min()) {
            (Some(srtt), Some(rtprop)) => Duration::from_nanos(srtt.saturating_sub(rtprop)),
            _ => Duration::ZERO,
        }
    }
}

/// rtt_sample = raw_rtt − ack_delay (engine §6). `ack_delay` raw'ı aşarsa untrusted
/// → `None` (negatif sample invalid; §7.3 ack_delay_untrusted).
#[must_use]
pub fn rtt_sample(raw_rtt: Duration, ack_delay: Duration) -> Option<Duration> {
    raw_rtt.checked_sub(ack_delay)
}
