//! TV-LOSS: loss detection (engine §5). loss = RECOVERY sinyali, RATE sinyali DEĞİL
//! (INV-RATE-1). Packet-threshold (K=3 [STD:BBR/QUIC RFC 9002]) + time-threshold
//! (9/8·srtt [STD], min_delay [CAL] parametre). loss_pre_fec (recovery ÖNCESİ) ve
//! residual_loss (recovery SONRASI) ayrı sayaçlar.
//!
//! Clean-arch: [`LossDetector`] trait = port (füzyon §8.4 bunun üzerinden loss_pre_fec
//! okuyacak); [`ThresholdLossDetector`] = RFC 9002-türevi adapter. FEC/ARQ execution
//! (§9/§10) + W_loss sliding window (§5 [CAL]) sonraki adımlarda.

use std::collections::BTreeMap;
use std::time::Duration;

/// Packet-threshold K [STD:BBR/QUIC RFC 9002 = 3].
pub const PACKET_THRESHOLD: u64 = 3;
/// Time-threshold pay [STD:RFC 9002 = 9].
pub const TIME_THRESH_NUM: u64 = 9;
/// Time-threshold payda [STD:RFC 9002 = 8].
pub const TIME_THRESH_DEN: u64 = 8;

/// Packet-threshold: `largest_acked − seq ≥ k` (RFC 9002 §6.1.1). K altı = reorder
/// (bekle), K üstü = kayıp.
#[must_use]
pub fn packet_threshold_lost(seq: u64, largest_acked: u64, k: u64) -> bool {
    largest_acked.saturating_sub(seq) >= k
}

/// Time-threshold: `elapsed > max(num/den·srtt, min_delay)` (RFC 9002 §6.1.2; §5).
/// `min_delay` [CAL] parametre — reordering tail + tiny-srtt koruması.
#[must_use]
pub fn time_threshold_lost(
    elapsed: Duration,
    srtt: Duration,
    min_delay: Duration,
    num: u64,
    den: u64,
) -> bool {
    elapsed > scale_duration(srtt, num, den).max(min_delay)
}

fn scale_duration(d: Duration, num: u64, den: u64) -> Duration {
    let ns = d.as_nanos() * u128::from(num) / u128::from(den);
    Duration::from_nanos(ns as u64)
}

fn ratio_ppm(num: u64, den: u64) -> u64 {
    if den == 0 {
        return 0;
    }
    (u128::from(num) * 1_000_000 / u128::from(den)) as u64
}

/// Loss detection port (clean-arch). Füzyon (§8.4) `loss_pre_fec`'i buradan okur;
/// loss sayaçları send_rate'e GİRMEZ (INV-RATE-1).
pub trait LossDetector {
    /// Paket gönderimini kaydet.
    fn on_sent(&mut self, seq: u64, send_ns: u64, units: u64);
    /// ACK işle; yeni packet-threshold kayıplarını (artan seq) döner.
    fn on_ack(&mut self, acked: &[u64]) -> Vec<u64>;
    /// Time-threshold kayıplarını `now`da ilan et; yeni-kayıp seq'leri döner.
    fn detect_time_losses(&mut self, now_ns: u64, srtt: Duration) -> Vec<u64>;
    /// Recovery sonrası kurtarılamayan birim bildir (residual_loss'u besler).
    fn on_unrecovered(&mut self, units: u64);
    /// loss_pre_fec = lost/sent (ppm; recovery ÖNCESİ).
    fn loss_pre_fec_ppm(&self) -> u64;
    /// residual_loss = unrecovered/sent (ppm; recovery SONRASI).
    fn residual_loss_ppm(&self) -> u64;
}

/// Loss detector yapılandırması.
#[derive(Debug, Clone, Copy)]
pub struct LossConfig {
    /// K [STD:3].
    pub packet_threshold: u64,
    /// Time-threshold pay [STD:9].
    pub time_num: u64,
    /// Time-threshold payda [STD:8].
    pub time_den: u64,
    /// min_delay [CAL] parametre (default = RFC kGranularity ~1ms floor).
    pub min_delay: Duration,
}

impl Default for LossConfig {
    fn default() -> Self {
        Self {
            packet_threshold: PACKET_THRESHOLD,
            time_num: TIME_THRESH_NUM,
            time_den: TIME_THRESH_DEN,
            min_delay: Duration::from_millis(1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sent {
    send_ns: u64,
    units: u64,
}

/// RFC 9002-türevi packet/time-threshold loss detector ([`LossDetector`] adapter).
#[derive(Debug, Clone, Default)]
pub struct ThresholdLossDetector {
    cfg: LossConfig,
    inflight: BTreeMap<u64, Sent>,
    largest_acked: u64,
    have_acked: bool,
    sent_units: u64,
    lost_units: u64,
    unrecovered_units: u64,
}

impl ThresholdLossDetector {
    /// Default config ([STD] K=3, 9/8; min_delay 1ms).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Verilen config ile.
    #[must_use]
    pub fn with_config(cfg: LossConfig) -> Self {
        Self {
            cfg,
            ..Self::default()
        }
    }

    /// Toplam gönderilen birim.
    #[must_use]
    pub fn sent_units(&self) -> u64 {
        self.sent_units
    }
    /// Kayıp ilan edilen toplam birim (recovery öncesi).
    #[must_use]
    pub fn lost_units(&self) -> u64 {
        self.lost_units
    }
    /// Kurtarılamayan toplam birim (recovery sonrası).
    #[must_use]
    pub fn unrecovered_units(&self) -> u64 {
        self.unrecovered_units
    }
}

impl LossDetector for ThresholdLossDetector {
    fn on_sent(&mut self, seq: u64, send_ns: u64, units: u64) {
        self.inflight.insert(seq, Sent { send_ns, units });
        self.sent_units += units;
    }

    fn on_ack(&mut self, acked: &[u64]) -> Vec<u64> {
        for &seq in acked {
            self.inflight.remove(&seq);
            if !self.have_acked || seq > self.largest_acked {
                self.largest_acked = seq;
                self.have_acked = true;
            }
        }
        if !self.have_acked {
            return Vec::new();
        }
        let la = self.largest_acked;
        let k = self.cfg.packet_threshold;
        // Önce kayıp seq'leri topla (immutable borç), sonra kaldır.
        let lost: Vec<u64> = self
            .inflight
            .keys()
            .copied()
            .filter(|&seq| packet_threshold_lost(seq, la, k))
            .collect();
        for seq in &lost {
            if let Some(s) = self.inflight.remove(seq) {
                self.lost_units += s.units;
            }
        }
        lost
    }

    fn detect_time_losses(&mut self, now_ns: u64, srtt: Duration) -> Vec<u64> {
        // [TUR-1a / D-1/1] LATER-ACK PRECONDITION (RFC 9002 §6.1'in atlanmış yarısı): time-threshold
        // KESİN kayıp sınıflandırması YALNIZ "daha yüksek seq ACK'lendi" kanıtıyla (seq < largest_acked).
        // Saf sessizlik loss metriğini ARTIRMAZ — sessizlikte tail-recovery liveness'ın işi (§5.1
        // ack_confidence → RECOVER_FWD spekülatif kopya; loss-sınıflandırması DEĞİL). Bu olmadan
        // SACK-cadence gecikmesi (K-1) temiz kanalda sistematik false-loss üretiyordu (ölçüldü: 27/27).
        if !self.have_acked {
            return Vec::new();
        }
        let cfg = self.cfg;
        let la = self.largest_acked;
        let lost: Vec<u64> = self
            .inflight
            .iter()
            .filter(|(&seq, s)| {
                let elapsed = Duration::from_nanos(now_ns.saturating_sub(s.send_ns));
                seq < la
                    && time_threshold_lost(elapsed, srtt, cfg.min_delay, cfg.time_num, cfg.time_den)
            })
            .map(|(&seq, _)| seq)
            .collect();
        for seq in &lost {
            if let Some(s) = self.inflight.remove(seq) {
                self.lost_units += s.units;
            }
        }
        lost
    }

    fn on_unrecovered(&mut self, units: u64) {
        self.unrecovered_units += units;
    }

    fn loss_pre_fec_ppm(&self) -> u64 {
        ratio_ppm(self.lost_units, self.sent_units)
    }

    fn residual_loss_ppm(&self) -> u64 {
        ratio_ppm(self.unrecovered_units, self.sent_units)
    }
}
