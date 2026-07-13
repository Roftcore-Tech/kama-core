//! TV-SRC: SRC (SafeRoom Rate Control) çekirdek denklemleri (engine §8.2).
//!
//! Bandwidth-model çekirdek: `send_rate = pacing_gain × BtlBw`, inflight-cap ≤ cwnd.
//! Delay-tepkisi YAPISAL (fuzzy DEĞİL): `send_rate = min(pacing_rate, cwnd/srtt)` —
//! §8.2 inflight-cap'in rate formu (Little yasası) + §0.3 "queue_delay + inflight_cap
//! ile pace". queue_delay (srtt) büyüyünce `cwnd/srtt` düşer → send_rate düşer; loss
//! srtt'ye GİRMEZ → send_rate'i etkilemez (INV-RATE-1: loss≠congestion).
//!
//! Sabitler [STD:BBR] / [STD:policy]. Probe MEKANİZMASI (faz→gain) burada; probe
//! ZAMANLAMASI (PROBE_BW_PERIOD [CAL] §8.3), state-machine geçişleri (§3), MIN_RATE
//! floor (§8.5 [CAL]) ve fuzzy/FEC/ARQ (§8.4/§9/§10) sonraki adımlarda.

use std::time::Duration;

/// cwnd gain [STD:BBR 2].
pub const CWND_GAIN: u64 = 2;
/// Default path MTU (bytes) [STD:policy §11].
pub const DEFAULT_MTU: u64 = 1200;

fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

/// Pacing gain — fixed-point rational (float yok, ADR-17): `rate = value · num/den`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gain {
    /// Pay.
    pub num: u64,
    /// Payda.
    pub den: u64,
}

impl Gain {
    /// STEADY 1.0 [STD:BBR].
    pub const STEADY: Gain = Gain { num: 1, den: 1 };
    /// ProbeBW up 1.25 [STD:BBR].
    pub const PROBE_UP: Gain = Gain { num: 5, den: 4 };
    /// ProbeBW down 0.75 [STD:BBR].
    pub const PROBE_DOWN: Gain = Gain { num: 3, den: 4 };
    /// STARTUP high_gain ≈ 2/ln2 [STD:BBR] (fixed-point yaklaşım).
    pub const STARTUP: Gain = Gain {
        num: 2885,
        den: 1000,
    };
    /// [TUR-3b] DRAIN = 1/high_gain [STD:BBR] — Startup'ın şişirdiği kuyruğu boşalt.
    pub const DRAIN: Gain = Gain {
        num: 1000,
        den: 2885,
    };

    /// `value`'ya gain uygular (integer, u128 ara-tip).
    #[must_use]
    pub fn apply(self, value: u64) -> u64 {
        (u128::from(value) * u128::from(self.num) / u128::from(self.den)) as u64
    }
}

/// Pacing fazı — probe MEKANİZMASI (faz→gain). Faz GEÇİŞLERİ (PROBE_BW_PERIOD [CAL]
/// §8.3, state machine §3) ertelendi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacingPhase {
    /// BtlBw hızlı keşif (yüksek gain).
    Startup,
    /// [TUR-3b] Startup-exit drain: 1/high_gain ile kuyruğu BDP'ye indir [STD:BBR].
    Drain,
    /// Kararlı: pace = BtlBw.
    Steady,
    /// BW-probe up (kısa).
    ProbeUp,
    /// Drain down (biriken kuyruğu boşalt).
    ProbeDown,
}

impl PacingPhase {
    /// Fazın pacing gain'i [STD:BBR].
    #[must_use]
    pub fn gain(self) -> Gain {
        match self {
            PacingPhase::Startup => Gain::STARTUP,
            PacingPhase::Drain => Gain::DRAIN,
            PacingPhase::Steady => Gain::STEADY,
            PacingPhase::ProbeUp => Gain::PROBE_UP,
            PacingPhase::ProbeDown => Gain::PROBE_DOWN,
        }
    }
}

/// SRC yapılandırması ([STD] varsayılanlar).
#[derive(Debug, Clone, Copy)]
pub struct SrcConfig {
    /// cwnd gain [STD:BBR 2].
    pub cwnd_gain: u64,
    /// MIN_CWND (bytes) [STD:policy 4×MTU]; [CAL] rafine sonra.
    pub min_cwnd: u64,
}

impl Default for SrcConfig {
    fn default() -> Self {
        Self {
            cwnd_gain: CWND_GAIN,
            min_cwnd: 4 * DEFAULT_MTU,
        }
    }
}

/// SRC çekirdek kararı (engine §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrcDecision {
    /// BDP = BtlBw × RTprop (bytes).
    pub bdp: u64,
    /// cwnd = max(cwnd_gain × BDP, MIN_CWND) (bytes).
    pub cwnd: u64,
    /// pacing_rate = pacing_gain × BtlBw (bytes/sec).
    pub pacing_rate: u64,
    /// send_rate = min(pacing_rate, cwnd/srtt) (bytes/sec; INV-RATE-1).
    pub send_rate: u64,
}

/// BDP = BtlBw[B/s] × RTprop[s] → bytes. u128 ara-tip → overflow yok.
#[must_use]
pub fn bdp_bytes(btlbw: u64, rtprop: Duration) -> u64 {
    (u128::from(btlbw) * u128::from(dur_ns(rtprop)) / 1_000_000_000) as u64
}

/// bytes / süre → bytes/sec (u128 ara-tip); süre 0 → `u64::MAX` (cap bağlamaz).
fn rate_from_window(bytes: u64, window: Duration) -> u64 {
    let ns = dur_ns(window);
    if ns == 0 {
        return u64::MAX;
    }
    (u128::from(bytes) * 1_000_000_000 / u128::from(ns)) as u64
}

/// SRC çekirdek denklemlerini değerlendirir (engine §8.2). `gain` bir GİRDİ (probe
/// faz→gain mekanizması), zamanlaması değil.
#[must_use]
pub fn evaluate(
    cfg: &SrcConfig,
    btlbw: u64,
    rtprop: Duration,
    srtt: Duration,
    gain: Gain,
) -> SrcDecision {
    let bdp = bdp_bytes(btlbw, rtprop);
    let cwnd = cfg.cwnd_gain.saturating_mul(bdp).max(cfg.min_cwnd);
    let pacing_rate = gain.apply(btlbw);
    let inflight_cap_rate = rate_from_window(cwnd, srtt);
    let send_rate = pacing_rate.min(inflight_cap_rate);
    SrcDecision {
        bdp,
        cwnd,
        pacing_rate,
        send_rate,
    }
}

/// Füzyon `send_rate_gain`'ini (per-mille) SRC pacing'e uygular (§5.4 composition):
/// `send_rate = min(pacing_gain × gain × BtlBw, cwnd/srtt)`.
///
/// Tek `min()` — füzyon gain'i proaktif soft-drain, `cwnd/srtt` yapısal HARD ceiling
/// (son söz). Dört girdi (pacing_gain, gain, BtlBw, cwnd/srtt) de loss-kör → INV-RATE-1
/// uçtan uca (gain kaynağı fuzzy'de yalnız bw+queue_delay, §5.4).
#[must_use]
pub fn effective_send_rate(
    cfg: &SrcConfig,
    btlbw: u64,
    rtprop: Duration,
    srtt: Duration,
    pacing_gain: Gain,
    send_rate_gain_permille: u32,
) -> u64 {
    let bdp = bdp_bytes(btlbw, rtprop);
    let cwnd = cfg.cwnd_gain.saturating_mul(bdp).max(cfg.min_cwnd);
    let pacing = pacing_gain.apply(btlbw);
    let throttled = (u128::from(pacing) * u128::from(send_rate_gain_permille) / 1000) as u64;
    throttled.min(rate_from_window(cwnd, srtt))
}
