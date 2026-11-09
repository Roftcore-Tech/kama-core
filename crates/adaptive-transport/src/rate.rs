//! TV-SRC: SRC (Kama Rate Control) çekirdek denklemleri (engine §8.2).
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

// ── E1 gösterim bayrakları (CC §4.1, CC-K5; ISEMRI-YOL3-M2 KK-M2-01) ─────────────────────────────
//
// EKLEMELİ: yukarıdaki imzalar (`SrcConfig`, `evaluate`, `effective_send_rate`) DEĞİŞMEZ. Bayraklar
// kapalıyken motor yalnız eski kod yolunu çağırır (bit-aynı); aşağıdaki işlevler yalnız bayrak açıkken
// çağrılır.

/// E1 gösterim bayrakları — çalışma zamanı yapılandırması (Cargo feature DEĞİL).
///
/// Adlar CC §4.1 tablosuyla **aynen**. `Default` = beşi de `false` = bugünkü davranış bit-aynı (ürün
/// varsayılanı). 32 birleşimin hepsi geçerlidir; birleşim kuralı yoktur, tipli hata gerekmez. Yalnız
/// gösterim düğümü (`transport-v1.x-goster`) ve G2 teşhis kolları açar.
///
/// | bayrak | kural (CC §4.1) |
/// |---|---|
/// | `e1_startup_cwnd` | (c) Startup cwnd kazancı 2,885 (`PacingPhase::Startup`'a bağlı; `cwnd/srtt` ve kalkış tabanı aynı kazancı kullanır; Drain hedefi 2×BDP kalır) |
/// | `e1_applimited_plateau` | (d) uygulamayla sınırlı tur plato sayılmaz (ayrı tur mandalı) |
/// | `e1_drainrt_gate` | DrainRtprop girişi yalnız rtprop bayat ∧ qd ≥ [`E1_DRAINRT_QD_MIN_PERMILLE`] |
/// | `e1_probe_keep` | (e-dar) bekleyen kayıp ProbeUp'ı kesmez; atıflı kayıp iptal ettirmez; qd iptali kalır; retx koşulu `!pending_lost.is_empty() ∧ state ≠ RecoverFwd` |
/// | `retx_first` | ARQ-önce koşulu `retx_first ∨ tick % K_ARQ_SHARE == 0` |
///
/// Hiçbir bayrak kaybı hız girdisi yapmaz (INV-RATE-1); `diag_*` sayaçları karar girdisi olmaz (INV-007).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct E1Flags {
    /// (c) Startup cwnd kazancı 2,885 (CC §2.1 (c)).
    pub e1_startup_cwnd: bool,
    /// (d) uygulamayla sınırlı turlar plato sayılmaz (CC §2.1 (d)).
    pub e1_applimited_plateau: bool,
    /// DrainRtprop girişi yalnız rtprop bayat ∧ qd ≥ 250‰ (CC §2.1).
    pub e1_drainrt_gate: bool,
    /// (e-dar) bekleyen/atıflı kayıp ProbeUp'ı kesmez (CC §2.1 (e-dar), CC-K5).
    pub e1_probe_keep: bool,
    /// ARQ-önce her izinde (CC §4.1, CC-K6).
    pub retx_first: bool,
}

/// [E1 DrainRtprop kapısı] DrainRtprop girişi için asgari kuyruk gecikmesi oranı (‰, qd/RTprop).
/// Sahip sayısı (CC §4.1: "giriş yalnız rtprop bayat ∧ qd ≥ 250‰"); [CAL] DEĞİL.
pub const E1_DRAINRT_QD_MIN_PERMILLE: u64 = 250;

/// [E1 (d) qd freni] Uygulamayla sınırlı işaretli tur yine de değerlendirilir ⇔ tur işaretinde
/// qd ≥ bu oran (‰). CC §2.1 (d) "turda qd ≥ 250‰ ise tur plato sayılır"; G1 model kararı (KK-M2-08)
/// GİRER. Kayıp girdi DEĞİL (INV-RATE-1).
pub const E1_PLATEAU_QD_BRAKE_PERMILLE: u64 = 250;

/// [E1 (c)] cwnd kazancı (Startup fazında, bayrak açıkken): `Gain::STARTUP` = 2885/1000 (CC §2.1 (c)).
pub const E1_STARTUP_CWND_GAIN: Gain = Gain::STARTUP;

/// [E1 (c)] Kesirli cwnd kazancıyla cwnd: `max(cwnd_gain × BDP, min_cwnd)`. u128 ara-tip; taşma
/// `u64::MAX`'e doyar (`SrcConfig.cwnd_gain.saturating_mul` deseniyle aynı yön).
#[must_use]
pub fn cwnd_with_gain(cfg: &SrcConfig, cwnd_gain: Gain, bdp: u64) -> u64 {
    let den = u128::from(cwnd_gain.den.max(1));
    let scaled = u128::from(bdp) * u128::from(cwnd_gain.num) / den;
    u64::try_from(scaled).unwrap_or(u64::MAX).max(cfg.min_cwnd)
}

/// [E1 (c)] [`effective_send_rate`]'in kesirli cwnd kazançlı eşi: cwnd = [`cwnd_with_gain`].
/// Geri kalan aritmetik [`effective_send_rate`] ile birebir aynıdır (tek `min()`, `cwnd/srtt` son söz).
#[must_use]
pub fn effective_send_rate_cwnd_gain(
    cfg: &SrcConfig,
    cwnd_gain: Gain,
    btlbw: u64,
    rtprop: Duration,
    srtt: Duration,
    pacing_gain: Gain,
    send_rate_gain_permille: u32,
) -> u64 {
    let bdp = bdp_bytes(btlbw, rtprop);
    let cwnd = cwnd_with_gain(cfg, cwnd_gain, bdp);
    let pacing = pacing_gain.apply(btlbw);
    let throttled = (u128::from(pacing) * u128::from(send_rate_gain_permille) / 1000) as u64;
    throttled.min(rate_from_window(cwnd, srtt))
}

#[cfg(test)]
mod e1_tests {
    use super::*;

    #[test]
    fn e1_flags_default_all_off() {
        let f = E1Flags::default();
        assert!(
            !f.e1_startup_cwnd
                && !f.e1_applimited_plateau
                && !f.e1_drainrt_gate
                && !f.e1_probe_keep
                && !f.retx_first
        );
    }

    #[test]
    fn e1_cwnd_gain_integer_two_matches_legacy_rate() {
        // Kazanç 2/1 ile eklemeli işlev eski `effective_send_rate` ile aynı sonucu verir (aritmetik
        // eşdeğerlik; bayrak kapalıyken motor zaten eski işlevi çağırır).
        let cfg = SrcConfig::default();
        let two = Gain {
            num: CWND_GAIN,
            den: 1,
        };
        for &(bw, rtp_ms, srtt_ms, g) in &[
            (12_500u64, 300u64, 450u64, 1000u32),
            (1_000, 1_000, 1_400, 1000),
            (250_000, 50, 90, 750),
            (0, 300, 300, 1000),
        ] {
            let rtp = Duration::from_millis(rtp_ms);
            let srtt = Duration::from_millis(srtt_ms);
            for pg in [Gain::STARTUP, Gain::STEADY, Gain::PROBE_UP] {
                assert_eq!(
                    effective_send_rate_cwnd_gain(&cfg, two, bw, rtp, srtt, pg, g),
                    effective_send_rate(&cfg, bw, rtp, srtt, pg, g)
                );
            }
        }
    }

    #[test]
    fn e1_cwnd_gain_startup_raises_cap_above_min_cwnd() {
        // 12,5 kB/s × 300 ms = 3750 B BDP: 2×BDP = 7500 B, 2,885×BDP = 10818 B (> 4800 B MIN_CWND).
        let cfg = SrcConfig::default();
        let bdp = bdp_bytes(12_500, Duration::from_millis(300));
        assert_eq!(bdp, 3750);
        assert_eq!(cwnd_with_gain(&cfg, E1_STARTUP_CWND_GAIN, bdp), 10_818);
        assert_eq!(
            cwnd_with_gain(
                &cfg,
                Gain {
                    num: CWND_GAIN,
                    den: 1
                },
                bdp
            ),
            7_500
        );
        // Küçük BDP'de MIN_CWND bağlar: 2,885 × 1000 B < 4800 B.
        assert_eq!(
            cwnd_with_gain(&cfg, E1_STARTUP_CWND_GAIN, 1000),
            4 * DEFAULT_MTU
        );
        // Taşma doyar.
        assert_eq!(
            cwnd_with_gain(&cfg, E1_STARTUP_CWND_GAIN, u64::MAX),
            u64::MAX
        );
    }
}
