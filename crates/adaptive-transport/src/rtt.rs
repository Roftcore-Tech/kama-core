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
        // [TUR-3a D-2/2] KEEP-LAST: budama filtreyi boşaltacaksa en-yeni örnek yaşar.
        // (push-sonra-buda sırası nedeniyle bugün yapısal olarak boşalamaz — savunma katmanı;
        // gelecek çağrı-sırası değişikliklerine karşı invariant kilitlenir.)
        let last = self.samples.last().copied();
        self.samples.retain(|&(t, _)| t >= cutoff);
        if self.samples.is_empty() {
            if let Some(l) = last {
                self.samples.push(l);
            }
        }
    }

    fn min(&self) -> Option<u64> {
        self.samples.iter().map(|&(_, v)| v).min()
    }
}

/// RTT / RTprop / queue-delay estimator (engine spec §6).
///
/// [TUR-3a D-2/1] RTprop penceresi KANAL-TÜREVİ: sender `set_rtprop_window` ile
/// `max(10s, K_rtprop × corrected_srtt)` yazar (ZM-2 — 10s wall-clock sub-kbps'te 1-4 örneğe
/// düşüyordu).
#[derive(Debug, Clone)]
pub struct RttEstimator {
    /// Smoothed RTT (ns); `None` = henüz sample yok.
    srtt_ns: Option<u64>,
    /// RTT varyansı (ns).
    rttvar_ns: u64,
    /// RTprop min-filter.
    rtprop: MinFilter,
    /// RTprop penceresi (ns) — sender kanal-türevi yazar; taban W_RTPROP [STD].
    rtprop_window_ns: u64,
    /// [TUR-3a.5 commit-α, srtt-clock-v1 §2.2] qd-payı KISA-pencere min-filter'ı: qd artık
    /// srtt'den DEĞİL buradan (qd = min_W_qd(rtt) − rtprop = ayakta-kuyruk). Kuyruk-fiziği:
    /// RTT örneği kuyruğu YAŞAMIŞ paketten gelir, gerçek transitin altına inemez → gürültü
    /// tek-taraflı-yukarı → kısa-min anlamsal-doğru + salıverme ANINDA (gain-mıhı ilacı).
    qd_min: MinFilter,
    /// qd kısa-penceresi (ns) — sender K_qd×ack_epoch yazar; [STD] taban BİLİNÇLİ YOK
    /// (keep-last boşalmayı önler; taban hızlı-decade'de soft-drain'i köreltir).
    qd_window_ns: u64,
}

impl Default for RttEstimator {
    fn default() -> Self {
        Self {
            srtt_ns: None,
            rttvar_ns: 0,
            rtprop: MinFilter::default(),
            rtprop_window_ns: dur_ns(W_RTPROP),
            qd_min: MinFilter::default(),
            qd_window_ns: dur_ns(W_RTPROP), // ilk yazıma dek muhafazakâr (rtprop-penceresi)
        }
    }
}

impl RttEstimator {
    /// Boş estimator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// [TUR-3a D-2/1] Kanal-türevi RTprop penceresi: `max(10s, K_rtprop × corrected_srtt)`.
    pub fn set_rtprop_window(&mut self, w: Duration) {
        self.rtprop_window_ns = dur_ns(w).max(dur_ns(W_RTPROP)); // taban [STD]: küçültme yok
    }

    /// [TUR-3a.5 commit-α] qd kısa-penceresi: `W_qd = K_qd × ack_epoch` (kanal-türevi;
    /// W_qd ≤ W_rtprop invariantı çağıranda — kısa-min ≥ uzun-min yapısal kalsın).
    pub fn set_qd_window(&mut self, w: Duration) {
        self.qd_window_ns = dur_ns(w).min(self.rtprop_window_ns).max(1);
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
        self.rtprop.observe(now_ns, s, self.rtprop_window_ns);
        self.qd_min.observe(now_ns, s, self.qd_window_ns); // [3a.5] aynı örnek, kısa pencere
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

    /// [TUR-3a.5 commit-α, NORMATİF-değişim] queue_delay = min_W_qd(rtt) − RTprop
    /// (≥0; congestion sinyali, loss DEĞİL). ESKİ tanım (srtt − rtprop) CANCELLED:
    /// srtt-EWMA lag'ı boş kuyruğu dakikalarca dolu gösteriyordu (ölçüldü, 42-kilidi yarı-1);
    /// ayakta-kuyruk için doğru istatistik kısa-min (srtt-clock-v1 §2.2). Veri yoksa 0.
    #[must_use]
    pub fn queue_delay(&self) -> Duration {
        match (self.qd_min.min(), self.rtprop.min()) {
            (Some(short_min), Some(rtprop)) => {
                Duration::from_nanos(short_min.saturating_sub(rtprop))
            }
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

/// [TUR-1x, Seçenek-A min-rtt guard — RFC 9002 §5.3 semantiği] ack_delay düzeltmesini
/// GÜVENLİYSE uygular: `corrected = raw − ack_delay` yalnız gözlenen min-RTT'nin (rtprop)
/// altına İNMİYORSA kullanılır; underflow ya da min-rtt-altı → **raw kullanılır, örnek
/// ATLANMAZ** (sample-skip estimator açlığı üretir — K-1 sınıfı; raw kendi saatimizde ölçülü
/// güvenilir üst-sınırdır). Dönüş: `(sample, correction_applied)`.
///
/// NOT: mutlak wall-clock taban (1ms) 4-decade'de GEÇERSİZ çıktı — loopback/LAN gerçek RTT
/// sub-ms; ölçümle yanlışlandı (bench loopback testi) → gözlenen-min guard'ıyla değiştirildi
/// (ZM-2: decade-bağımlı sabit yasak).
#[must_use]
pub fn apply_ack_delay(
    raw_rtt: Duration,
    ack_delay: Duration,
    min_rtt: Option<Duration>,
) -> (Duration, bool) {
    match raw_rtt.checked_sub(ack_delay) {
        Some(c) if min_rtt.is_none_or(|m| c >= m) => (c, true),
        _ => (raw_rtt, false),
    }
}
