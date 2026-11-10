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
