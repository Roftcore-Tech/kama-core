//! CC-PORT (ISEMRI-CC-PORT, 2026-11-10): quinn-proto 0.11.18'in tıkanıklık denetimi ve
//! pacing mekanizmalarının Kama birim/çerçeve modeline taşınması.
//!
//! Modül her zaman derlenir; motora bağlanması **çalışma zamanında** olur:
//! [`crate::sender::Sender::with_cc_port`] + [`crate::receiver::Receiver::with_cc_port`].
//! `Sender::new()` / `Receiver::new()` bugünkü motordur (bit-eşit). Cargo özelliği bilinçli
//! olarak YOK: CI'nın `--all-features`'ı motoru değiştiremez ve iki motor aynı süreçte eşli
//! koşabilir (`tests/cc_port_sim.rs`).
//!
//! | dosya | quinn kaynağı | ne |
//! |---|---|---|
//! | `min_max.rs` | `congestion/bbr/min_max.rs` | tur-pencereli max filtresi |
//! | `bw_estimation.rs` | `congestion/bbr/bw_estimation.rs` | teslim hızı (min(send, ack)) |
//! | `bbr.rs` | `congestion/bbr/mod.rs` | durum makinesi, cwnd, recovery_window |
//! | `mod.rs` (`PacerParams`) | `connection/pacing.rs` | pencere tabanlı pacing parametreleri |
//!
//! Lisans: quinn-proto Apache-2.0 / MIT (atıf her dosyanın başında). quinn ürün
//! bağımlılığı DEĞİLDİR; kod taşınmıştır (ADR-06/ADR-14 "fork yok" korunur).

pub mod bbr;
mod bw_estimation;
mod min_max;

pub use bbr::{rfc9002_initial_window, Bbr, Mode};

/// quinn `connection/pacing.rs` sabitleri: 2 ms burst aralığı, 10..256 paket burst sınırı.
const BURST_INTERVAL_NS: u128 = 2_000_000;
const MIN_BURST_SIZE: u64 = 10;
const MAX_BURST_SIZE: u64 = 256;
/// RFC 9002 §7.7 "N = 1.25": kova bir RTT'de 1,25 pencere dolar (quinn `pacing.rs:92-95`).
const PACING_N_PM: u128 = 1250;

/// Pencere tabanlı pacer parametreleri — quinn `Pacer` davranışının Kama `TokenBucket`'a
/// (hız + burst tavanı) izdüşümü.
///
/// quinn'de BBR'ın `pacing_rate`'i yalnız metriktir (B5 bulgusu, QUINN-FARKI §1); fiili
/// pace `window · 1,25 / srtt` hızında, kapasitesi `clamp(window · 2 ms / srtt, 10·MTU,
/// 256·MTU)` olan kovadır (`optimal_capacity`, `pacing.rs:116-139`). Bu yapı aynısını verir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacerParams {
    /// Kova dolum hızı (bayt/s) = `window · 1,25 / srtt`.
    pub rate_bps: u64,
    /// Burst kapasitesi (bayt) = `clamp(window · 2 ms / srtt, 10·MTU, 256·MTU)`.
    pub burst_cap: u64,
}

impl PacerParams {
    /// `window` bayt, `srtt_ns` ns (0 → 1 ns; quinn `optimal_capacity` `.max(1)`), `mtu` bayt.
    #[must_use]
    pub fn from_window(window: u64, srtt_ns: u64, mtu: u64) -> Self {
        let rtt = u128::from(srtt_ns.max(1));
        let capacity = ((u128::from(window) * BURST_INTERVAL_NS) / rtt) as u64;
        let burst_cap = capacity.clamp(MIN_BURST_SIZE * mtu, MAX_BURST_SIZE * mtu);
        let rate_bps = (u128::from(window) * PACING_N_PM * 1_000_000_000 / (1000 * rtt)) as u64;
        Self {
            rate_bps,
            burst_cap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacer_params_match_quinn_formulas() {
        // window 12 000 B, srtt 300 ms: rate = 12 000 × 1,25 / 0,3 = 50 000 B/s;
        // capacity = 12 000 × 2 ms / 300 ms = 80 B → taban 10·MTU = 12 000 B.
        let p = PacerParams::from_window(12_000, 300_000_000, 1200);
        assert_eq!(p.rate_bps, 50_000);
        assert_eq!(p.burst_cap, 12_000);
        // Büyük pencere, küçük RTT: 1 MB × 2 ms / 10 ms = 200 kB (tavan 307 200 B'nin altında).
        let p = PacerParams::from_window(1_000_000, 10_000_000, 1200);
        assert_eq!(p.burst_cap, 200_000);
        // Daha da büyük: 10 MB × 2 ms / 10 ms = 2 MB → tavan 256·MTU.
        let p = PacerParams::from_window(10_000_000, 10_000_000, 1200);
        assert_eq!(p.burst_cap, 256 * 1200);
    }

    #[test]
    fn burst_cap_never_below_ten_mtu_so_parity_bursts_fit() {
        // ISEMRI-CC-PORT §1 #5: eski kova tavanı tek pakete düşüyordu (`size > cap → None`).
        for w in [1_000u64, 4_800, 12_000, 100_000] {
            for rtt in [1_000_000u64, 300_000_000, 2_000_000_000] {
                assert!(PacerParams::from_window(w, rtt, 1200).burst_cap >= 12_000);
            }
        }
    }
}
