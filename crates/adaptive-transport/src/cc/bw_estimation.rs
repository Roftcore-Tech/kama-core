//! Teslim hızı tahmini — **quinn-proto 0.11.18 `src/congestion/bbr/bw_estimation.rs`
//! portu** (CC-PORT, 2026-11-10). Lisans: Apache-2.0 / MIT.
//!
//! Uyarlama: `Instant`/`Duration` yerine `u64` nanosaniye (Kama motor saati); `Display`
//! (f32) çıkarıldı (ADR-17: float yok). Aritmetik birebir: örnek bant genişliği =
//! `min(gönderim hızı, ACK hızı)`; uygulamayla sınırlı örnek filtreye girmez.

use super::min_max::MinMax;

/// quinn `BandwidthEstimation` (bayt/s, u64 ns saat).
#[derive(Clone, Debug, Default)]
pub(crate) struct BandwidthEstimation {
    total_acked: u64,
    prev_total_acked: u64,
    acked_time_ns: Option<u64>,
    prev_acked_time_ns: Option<u64>,
    total_sent: u64,
    prev_total_sent: u64,
    sent_time_ns: Option<u64>,
    prev_sent_time_ns: Option<u64>,
    max_filter: MinMax,
    acked_at_last_window: u64,
}

impl BandwidthEstimation {
    pub(crate) fn on_sent(&mut self, now_ns: u64, bytes: u64) {
        self.prev_total_sent = self.total_sent;
        self.total_sent += bytes;
        self.prev_sent_time_ns = self.sent_time_ns;
        self.sent_time_ns = Some(now_ns);
    }

    pub(crate) fn on_ack(&mut self, now_ns: u64, bytes: u64, round: u64, app_limited: bool) {
        self.prev_total_acked = self.total_acked;
        self.total_acked += bytes;
        self.prev_acked_time_ns = self.acked_time_ns;
        self.acked_time_ns = Some(now_ns);

        let Some(prev_sent_time) = self.prev_sent_time_ns else {
            return;
        };

        let send_rate = match self.sent_time_ns {
            Some(sent_time) if sent_time > prev_sent_time => Self::bw_from_delta(
                self.total_sent - self.prev_total_sent,
                sent_time - prev_sent_time,
            )
            .unwrap_or(0),
            // min(send, ack) alınacağı için bu yalnız "atla" demektir (quinn ile aynı).
            _ => u64::MAX,
        };

        let ack_rate = match self.prev_acked_time_ns {
            Some(prev_acked_time) => Self::bw_from_delta(
                self.total_acked - self.prev_total_acked,
                now_ns.saturating_sub(prev_acked_time),
            )
            .unwrap_or(0),
            None => 0,
        };

        let bandwidth = send_rate.min(ack_rate);
        if !app_limited && self.max_filter.get() < bandwidth {
            self.max_filter.update_max(round, bandwidth);
        }
    }

    pub(crate) fn bytes_acked_this_window(&self) -> u64 {
        self.total_acked - self.acked_at_last_window
    }

    pub(crate) fn end_acks(&mut self, _current_round: u64, _app_limited: bool) {
        self.acked_at_last_window = self.total_acked;
    }

    /// Max filtreden teslim hızı tahmini (bayt/s); örnek yoksa 0.
    pub(crate) fn get_estimate(&self) -> u64 {
        self.max_filter.get()
    }

    /// `bytes / delta_ns` → bayt/s; `delta_ns == 0` → `None`.
    pub(crate) const fn bw_from_delta(bytes: u64, delta_ns: u64) -> Option<u64> {
        if delta_ns == 0 {
            return None;
        }
        // u128 ara tip: bytes·1e9 taşmasın.
        let b_ns = (bytes as u128) * 1_000_000_000;
        Some((b_ns / (delta_ns as u128)) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_is_min_of_send_and_ack_rate() {
        let mut e = BandwidthEstimation::default();
        // 1200 B / 10 ms gönderim hızı = 120 kB/s; ACK hızı 1200 B / 100 ms = 12 kB/s.
        e.on_sent(0, 1200);
        e.on_sent(10_000_000, 1200);
        e.on_ack(300_000_000, 1200, 1, false);
        e.on_ack(400_000_000, 1200, 1, false);
        assert_eq!(e.get_estimate(), 12_000);
    }

    #[test]
    fn app_limited_sample_does_not_enter_filter() {
        let mut e = BandwidthEstimation::default();
        e.on_sent(0, 1200);
        e.on_sent(10_000_000, 1200);
        e.on_ack(300_000_000, 1200, 1, true);
        e.on_ack(400_000_000, 1200, 1, true);
        assert_eq!(e.get_estimate(), 0);
    }
}

/// [CC-PORT manşet incelemesi 2026-11-12] quinn'in tahmincisi ↔ Kama §7.1 örnekleyicisi, pacer
/// patlaması + ACK sıkışması altında. Hat 12,5 kB/s, 1200 B paketler; pacer iki paketi 1 ms arayla
/// çıkarır (kova patlaması), jitter iki ACK'i 2 ms arayla getirir. quinn: örnek = min(son paketin
/// baytı / son iki gönderim arası, son paketin baytı / son iki ACK olayı arası) = min(1,2 MB/s,
/// 600 kB/s) = 600 kB/s (hattın 48 katı) max filtreye girer. Kama: aralık = max(ack_elapsed,
/// send_elapsed), paketin gönderim anındaki `delivered_time`'a dayanır → örnek ≤ hat.
///
/// Bu fark, fork'suz quinn'in netem düzeninde (kayıp sınırlayıcıdan önce) hattı %99,9 doldurmasının
/// kaynağıdır: şişmiş BtlBw ile hattın ~1,4 katı gönderir; düşen paketler hat süresi tüketmediği
/// için kayıptan sağ kalanlar hattı doldurur. Port Kama örnekleyicisini kullandığından (Uyarlama 6)
/// BtlBw ≤ hat kalır ve pacing ≈ hat → netem düzeninde doluluk ≈ 1 − p. Telsiz düzeninde (kayıp
/// serileştirme sonrası) şişkin tahmin kuyruk ve ek yeniden iletim üretir (sim: `CC_JITTER_MS`).
#[cfg(test)]
mod ack_compression_contrast {
    use super::*;
    use crate::bw::{bw_bps, DeliveryRateSampler};

    const MS: u64 = 1_000_000;
    const LINK: u64 = 12_500;

    #[test]
    fn quinn_estimator_inflates_under_ack_compression_kama_sampler_does_not() {
        // Gönderim: iki 1200 B paket 1 ms arayla (pacer patlaması). İlk ACK 300 ms RTT ile, ikinci
        // ACK jitter yüzünden ilkinden yalnız 2 ms sonra (sıkışma).
        let mut q = BandwidthEstimation::default();
        let mut k = DeliveryRateSampler::new();
        let s0 = 0;
        let s1 = MS;
        let p0 = k.on_sent(s0, 1200, false, true);
        q.on_sent(s0, 1200);
        let p1 = k.on_sent(s1, 1200, false, true);
        q.on_sent(s1, 1200);
        let a0 = s0 + 300 * MS;
        let a1 = a0 + 2 * MS; // sıkışmış ACK
        q.on_ack(a0, 1200, 1, false);
        q.on_ack(a1, 1200, 1, false);
        let r0 = k.on_ack(a0, p0);
        let r1 = k.on_ack(a1, p1);
        let quinn_bw = q.get_estimate();
        let kama0 = bw_bps(r0.delivered, r0.interval);
        let kama1 = bw_bps(r1.delivered, r1.interval);
        assert!(
            quinn_bw >= 40 * LINK,
            "quinn tahmincisi sıkışmada şişer: {quinn_bw} B/s (hat {LINK})"
        );
        assert!(
            kama0 <= LINK && kama1 <= LINK,
            "Kama örnekleri hattı aşmaz: {kama0} / {kama1} B/s (hat {LINK})"
        );
    }
}
