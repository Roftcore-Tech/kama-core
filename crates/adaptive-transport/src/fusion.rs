//! TV-FUZZY: fuzzy FEC/ARQ/rate/mode karar motoru (engine §8.4/§15, TDD §5).
//!
//! Deterministik (ADR-17, RNG YOK): aynı input snapshot → aynı output. Trapezoidal
//! membership + Mamdani AND (min) + ağırlıklı-ortalama defuzzification (§15 centroid).
//! Membership eşikleri [CAL:netem] — burada [`FuzzyConfig::fixture`] KALİBRE DEĞİL
//! (gerçek değerler netem baseline'dan, §598); golden yalnız MOTOR MATEMATİĞİNİ +
//! kural tablosunu kilitler.
//!
//! **INV-RATE-1 fuzzy'ye gömülü (§5.4):** `send_rate_gain` YALNIZCA bandwidth +
//! queue_delay'e bağlıdır; loss girdisi DEĞİLDİR → loss send_rate'e giremez.
//!
//! Clean-arch: [`FusionController`] trait = port (SRC/engine loop bunun üzerinden
//! kararı okur); [`DeterministicFuzzyController`] = fuzzy adapter.

/// Per-mille birim (0..=1000) — membership derecesi ve oran çıktıları (float yok).
pub const PERMILLE: u32 = 1000;

/// Trapezoidal membership: μ(x) per-mille (0..=1000). μ = min(rise, fall).
/// Shoulder: `a==b` → sol-omuz (rise sabit 1000), `c==d` → sağ-omuz (fall sabit 1000).
#[must_use]
pub fn trap_permille(x: u64, a: u64, b: u64, c: u64, d: u64) -> u32 {
    let rise = if a >= b {
        // dejenere/sol-omuz: yükselen kenar yok, x ≥ b'de tam
        if x >= b {
            PERMILLE
        } else {
            0
        }
    } else if x <= a {
        0
    } else if x >= b {
        PERMILLE
    } else {
        // a < x < b : (x−a)/(b−a) per-mille
        ((u128::from(x - a) * u128::from(PERMILLE)) / u128::from(b - a)) as u32
    };

    let fall = if c >= d {
        // sağ-omuz: düşen kenar yok, x ≤ c'de tam
        if x <= c {
            PERMILLE
        } else {
            0
        }
    } else if x >= d {
        0
    } else if x <= c {
        PERMILLE
    } else {
        // c < x < d : (d−x)/(d−c) per-mille
        ((u128::from(d - x) * u128::from(PERMILLE)) / u128::from(d - c)) as u32
    };

    rise.min(fall)
}

/// Tek trapezoidal membership (a,b,c,d eşikleri).
#[derive(Debug, Clone, Copy)]
pub struct Trap {
    /// Yükselen kenar başı.
    pub a: u64,
    /// Yükselen kenar sonu / plateau başı.
    pub b: u64,
    /// Plateau sonu / düşen kenar başı.
    pub c: u64,
    /// Düşen kenar sonu.
    pub d: u64,
}

impl Trap {
    /// μ(x) per-mille.
    #[must_use]
    pub fn mu(&self, x: u64) -> u32 {
        trap_permille(x, self.a, self.b, self.c, self.d)
    }
}

/// Bir girdinin üç membership derecesi (per-mille).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Degrees {
    /// LOW derecesi.
    pub low: u32,
    /// MED derecesi.
    pub med: u32,
    /// HIGH derecesi.
    pub high: u32,
}

/// Bir girdi değişkeninin LOW/MED/HIGH trapezoidleri.
#[derive(Debug, Clone, Copy)]
pub struct VarThresholds {
    /// LOW membership (tipik sol-omuz: `Trap{0,0,c,d}`).
    pub low: Trap,
    /// MED membership.
    pub med: Trap,
    /// HIGH membership (tipik sağ-omuz: `Trap{a,b,MAX,MAX}`).
    pub high: Trap,
}

impl VarThresholds {
    /// `x`'i LOW/MED/HIGH derecelerine çevirir.
    #[must_use]
    pub fn fuzzify(&self, x: u64) -> Degrees {
        Degrees {
            low: self.low.mu(x),
            med: self.med.mu(x),
            high: self.high.mu(x),
        }
    }
}

/// Fuzzy motor yapılandırması — 5 girdinin membership eşikleri + consequent seviyeleri.
#[derive(Debug, Clone, Copy)]
pub struct FuzzyConfig {
    /// loss_rate (ppm) membership.
    pub loss: VarThresholds,
    /// bandwidth (bytes/sec) membership.
    pub bandwidth: VarThresholds,
    /// jitter (ppm) membership.
    pub jitter: VarThresholds,
    /// [TUR-3a önalım, D-3/1] qd_ratio (per-mille: queue_delay×1000/rtprop) membership —
    /// MUTLAK-ns eşikleri CANCELLED (sub-kbps'te fiziksel-taban qd saniyeler → kalıcı HIGH →
    /// gain 0.5× mıhlanması ÖLÇÜLDÜ; dürüst-rtprop'un zorunlu ikizi).
    pub queue_delay: VarThresholds,
    /// ack_confidence (per-mille) membership.
    pub ack_confidence: VarThresholds,

    /// fec_parity_ratio consequent: fec yok.
    pub fec_zero: u32,
    /// fec_parity_ratio consequent: orta.
    pub fec_med: u32,
    /// fec_parity_ratio consequent: yüksek.
    pub fec_high: u32,
    /// arq_aggressiveness consequent: hafif.
    pub arq_light: u32,
    /// arq_aggressiveness consequent: orta.
    pub arq_med: u32,
    /// arq_aggressiveness consequent: moderate.
    pub arq_moderate: u32,
    /// send_rate_gain consequent: düşük (congestion throttle).
    pub sr_low: u32,
    /// send_rate_gain consequent: orta (ileride MED rate kuralı için).
    pub sr_med: u32,
    /// send_rate_gain consequent: tam (throttle yok).
    pub sr_high: u32,
}

impl FuzzyConfig {
    /// **KALİBRE DEĞİL** fixture — gerçek eşikler netem baseline'dan (§598). Golden
    /// yalnız motor matematiğini + kural tablosunu kilitler; hiçbir eşik iddia edilmez.
    #[must_use]
    pub fn fixture() -> Self {
        // Kısayol: sol-omuz LOW, orta MED, sağ-omuz HIGH.
        let shoulder_low = |c, d| Trap { a: 0, b: 0, c, d };
        let mid = |a, b, c, d| Trap { a, b, c, d };
        let shoulder_high = |a, b| Trap {
            a,
            b,
            c: u64::MAX,
            d: u64::MAX,
        };

        Self {
            loss: VarThresholds {
                low: shoulder_low(200_000, 300_000),
                med: mid(200_000, 300_000, 500_000, 600_000),
                high: shoulder_high(500_000, 600_000),
            },
            bandwidth: VarThresholds {
                low: shoulder_low(50_000, 100_000),
                med: mid(50_000, 100_000, 150_000, 200_000),
                high: shoulder_high(150_000, 200_000),
            },
            jitter: VarThresholds {
                low: shoulder_low(100_000, 200_000),
                med: mid(100_000, 200_000, 400_000, 500_000),
                high: shoulder_high(400_000, 500_000),
            },
            queue_delay: VarThresholds {
                // qd_ratio per-mille [CAL placeholder]: <0.25×rtprop rahat; >1.5-2×rtprop congested.
                low: shoulder_low(250, 500),
                med: mid(250, 500, 1500, 2000),
                high: shoulder_high(1500, 2000),
            },
            ack_confidence: VarThresholds {
                low: shoulder_low(300, 500),
                med: mid(300, 500, 700, 900),
                high: shoulder_high(700, 900),
            },
            fec_zero: 0,
            fec_med: 500,
            fec_high: 800,
            arq_light: 200,
            arq_med: 500,
            arq_moderate: 600,
            sr_low: 500,
            sr_med: 750,
            sr_high: 1000,
        }
    }
}

/// Fuzzy motorun ham (crisp) girdileri.
#[derive(Debug, Clone, Copy)]
pub struct FusionInputs {
    /// loss_pre_fec (ppm).
    pub loss_ppm: u64,
    /// BtlBw (bytes/sec).
    pub bandwidth_bps: u64,
    /// jitter score (ppm).
    pub jitter_ppm: u64,
    /// [TUR-3a D-3/1] qd_ratio = queue_delay×1000/rtprop (per-mille; rtprop yoksa 0 —
    /// kanıt yok → boğma yok, sayaçla izlenir).
    pub qd_ratio_permille: u64,
    /// ack_confidence = delivered-ACK / beklenen-ACK (per-mille).
    pub ack_confidence_permille: u64,
}

/// Recovery/pacing modu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal adaptif çalışma.
    Normal,
    /// ACK-bağımsız forward-recovery (§11) — davranış Fusion-2'de.
    EmergencyForwardRedundancy,
}

/// Fuzzy motor kararı (defuzzified).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FusionDecision {
    /// FEC parity oranı (per-mille) — POLİTİKA; RS-encode güvenlik katmanında (INV-FEC-1).
    pub fec_parity_ratio_permille: u32,
    /// ARQ agresifliği (per-mille) — §9 selective retransmit'i sürer.
    pub arq_aggressiveness_permille: u32,
    /// send_rate throttle gain (per-mille) — YALNIZ bw+queue_delay (§5.4 loss-decoupling).
    pub send_rate_gain_permille: u32,
    /// Sıkıştırma açık mı.
    pub compress: bool,
    /// Recovery modu.
    pub mode: Mode,
}

/// Füzyon karar portu (clean-arch). SRC/engine loop kararı bunun üzerinden okur.
pub trait FusionController {
    /// Ölçülen girdilerden füzyon kararını üretir.
    fn decide(&self, inputs: &FusionInputs) -> FusionDecision;
}

/// Deterministik fuzzy controller (§8.4/§15 adapter, ADR-17).
#[derive(Debug, Clone, Copy)]
pub struct DeterministicFuzzyController {
    cfg: FuzzyConfig,
}

impl DeterministicFuzzyController {
    /// Verilen config ile.
    #[must_use]
    pub fn new(cfg: FuzzyConfig) -> Self {
        Self { cfg }
    }

    /// Fixture config ile (KALİBRE DEĞİL — golden/motor testi için).
    #[must_use]
    pub fn fixture() -> Self {
        Self {
            cfg: FuzzyConfig::fixture(),
        }
    }
}

impl FusionController for DeterministicFuzzyController {
    fn decide(&self, inputs: &FusionInputs) -> FusionDecision {
        let c = &self.cfg;

        // 1) Fuzzification — her crisp girdi → LOW/MED/HIGH dereceleri.
        let loss = c.loss.fuzzify(inputs.loss_ppm);
        let bw = c.bandwidth.fuzzify(inputs.bandwidth_bps);
        let jitter = c.jitter.fuzzify(inputs.jitter_ppm);
        let qd = c.queue_delay.fuzzify(inputs.qd_ratio_permille);
        let ackc = c.ack_confidence.fuzzify(inputs.ack_confidence_permille);

        // 2) FEC/ARQ kuralları (§8.4/§5.3). Mamdani AND = min; her kural
        //    (güç, fec_seviye, arq_seviye) üretir.
        let rules: [(u32, u32, u32); 4] = [
            // R1: loss LOW → fec=0, arq=light
            (loss.low, c.fec_zero, c.arq_light),
            // R2: loss HIGH ∧ bw HIGH ∧ jitter LOW → fec=high, arq=moderate
            (
                min3(loss.high, bw.high, jitter.low),
                c.fec_high,
                c.arq_moderate,
            ),
            // R3: loss MED ∧ bw MED → fec=med, arq=med
            (loss.med.min(bw.med), c.fec_med, c.arq_med),
            // R4: loss LOW ∧ jitter HIGH → fec=0, arq=light (reorder-tolerant ≈ düşük agresiflik)
            (loss.low.min(jitter.high), c.fec_zero, c.arq_light),
        ];
        let fec = weighted_average(rules.iter().map(|&(w, level, _)| (w, level)));
        let arq = weighted_average(rules.iter().map(|&(w, _, level)| (w, level)));

        // 3) send_rate_gain — §5.4 loss-decoupling: YALNIZ bw + queue_delay.
        //    SR1: bw LOW ∧ queue_delay HIGH → sr_low ; tümleyen ağırlık → sr_high (throttle yok).
        let sr1 = bw.low.min(qd.high);
        let sr_default = PERMILLE.saturating_sub(sr1);
        let send_rate_gain =
            weighted_average([(sr1, c.sr_low), (sr_default, c.sr_high)].into_iter());

        // 4) Emergency (§11 trigger): loss HIGH ∧ bw LOW ∧ ack_confidence LOW.
        let emergency = min3(loss.high, bw.low, ackc.low);
        let mode = if emergency > PERMILLE / 2 {
            Mode::EmergencyForwardRedundancy
        } else {
            Mode::Normal
        };

        // 5) compress: bw LOW baskın veya emergency → ON.
        let compress = bw.low > PERMILLE / 2 || mode == Mode::EmergencyForwardRedundancy;

        FusionDecision {
            fec_parity_ratio_permille: fec,
            arq_aggressiveness_permille: arq,
            send_rate_gain_permille: send_rate_gain,
            compress,
            mode,
        }
    }
}

/// Üç değerin min'i (Mamdani AND).
fn min3(a: u32, b: u32, c: u32) -> u32 {
    a.min(b).min(c)
}

/// Ağırlıklı-ortalama defuzzification (§15 centroid/height): Σ(w·level) / Σ(w).
/// Toplam ağırlık 0 ise 0.
fn weighted_average(items: impl Iterator<Item = (u32, u32)>) -> u32 {
    let mut num: u64 = 0;
    let mut den: u64 = 0;
    for (weight, level) in items {
        num += u64::from(weight) * u64::from(level);
        den += u64::from(weight);
    }
    num.checked_div(den).unwrap_or(0) as u32
}
