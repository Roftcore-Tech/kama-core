//! TV-EMERGENCY (§11): ACK-bağımsız forward-recovery davranışı + retreat hysteresis.
//!
//! Fusion-1 `mode=Emergency` ÜRETİR; burası EYLEME çevirir (§11 tiny→duplication D×
//! zaman-aralıklı, segment-block→minimal parity, ACK decimation).
//!
//! **Retreat hysteresis (spec §11 amended — osilasyon önlemi, [CAL] marjlar netem'e):**
//! - **escalate (redundancy ARTIR): ANINDA, dwell YOK** — recovery-obsesif ("veri
//!   ulaşsın"; geç escalate = korumasız kayıp). Schmitt bandı (health < escalate_threshold)
//!   gürültüyü zaten soğurur.
//! - **de-escalate (redundancy AZALT): asimetrik Schmitt (health > deescalate_threshold >
//!   escalate_threshold) + minimum dwell** — osilasyonun asıl kaynağı erken de-escalate.
//!
//! Clean-arch: [`RecoveryModeController`] trait (port) + [`HysteresisController`] impl.

use std::time::Duration;

fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

/// Forward-recovery seviyesi. Sıralı: `Arq < Dup2 < Dup3` (redundancy artan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EmergencyLevel {
    /// Forward redundancy yok — normal ARQ.
    Arq,
    /// 2× duplication / düşük minimal parity.
    Dup2,
    /// 3× duplication / daha yüksek minimal parity.
    Dup3,
}

impl EmergencyLevel {
    /// Bir üst seviye (Dup3'te doygun).
    fn escalated(self) -> Self {
        match self {
            Self::Arq => Self::Dup2,
            Self::Dup2 | Self::Dup3 => Self::Dup3,
        }
    }
    /// Bir alt seviye (Arq'ta doygun).
    fn deescalated(self) -> Self {
        match self {
            Self::Dup3 => Self::Dup2,
            Self::Dup2 | Self::Arq => Self::Arq,
        }
    }
}

/// Payload framing (SoC: uygulama rolü değil, boyut/çerçeveleme).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// Tek-datagram / tiny (block/shard yok).
    Tiny,
    /// Segment-block / multi-shard.
    SegmentBlock,
}

/// Emergency'de uygulanacak forward-recovery eylemi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Forward redundancy yok — normal ARQ.
    Arq,
    /// Tiny: aynı payload'ı `factor`× **zaman-aralıklı** (`spacing`) gönder, ACK beklemeden.
    Duplication {
        /// Kopya sayısı (D).
        factor: u8,
        /// Kopyalar arası aralık (burst-loss'a dayanıklılık).
        spacing: Duration,
    },
    /// Segment-block: düşük-oranlı forward FEC parity.
    MinimalParity {
        /// Parity oranı (per-mille).
        permille: u32,
    },
}

/// Retreat hysteresis yapılandırması. **[CAL] fixture** (netem'e).
#[derive(Debug, Clone, Copy)]
pub struct HysteresisConfig {
    /// health < bu → escalate (ANINDA, dwell yok).
    pub escalate_permille: u32,
    /// health > bu → de-escalate (dwell'li). `> escalate_permille` (Schmitt bandı).
    pub deescalate_permille: u32,
    /// de-escalate'ler arası minimum süre (escalate'e uygulanmaz).
    pub deescalate_dwell: Duration,
}

impl HysteresisConfig {
    /// **KALİBRE DEĞİL** fixture.
    #[must_use]
    pub fn fixture() -> Self {
        Self {
            escalate_permille: 300,
            deescalate_permille: 700,
            deescalate_dwell: Duration::from_millis(1000),
        }
    }
}

/// Emergency davranış yapılandırması. **[CAL] fixture** (netem'e).
#[derive(Debug, Clone, Copy)]
pub struct EmergencyConfig {
    /// Dup2 duplication faktörü.
    pub dup2_factor: u8,
    /// Dup3 duplication faktörü.
    pub dup3_factor: u8,
    /// Duplication kopyaları arası aralık.
    pub dup_spacing: Duration,
    /// Dup2 segment-block minimal parity (per-mille).
    pub block_parity_dup2_permille: u32,
    /// Dup3 segment-block minimal parity (per-mille).
    pub block_parity_dup3_permille: u32,
}

impl EmergencyConfig {
    /// **KALİBRE DEĞİL** fixture.
    #[must_use]
    pub fn fixture() -> Self {
        Self {
            dup2_factor: 2,
            dup3_factor: 3,
            dup_spacing: Duration::from_millis(20),
            block_parity_dup2_permille: 100,
            block_parity_dup3_permille: 200,
        }
    }
}

/// Retreat mode controller portu (clean-arch).
pub trait RecoveryModeController {
    /// `health` (ack_confidence↑ / residual_loss↓, per-mille) + `now` → güncel level.
    fn update(&mut self, health_permille: u32, now_ns: u64) -> EmergencyLevel;
    /// Güncel level.
    fn level(&self) -> EmergencyLevel;
}

/// Asimetrik Schmitt + de-escalate dwell hysteresis controller (§11 amended).
#[derive(Debug, Clone, Copy)]
pub struct HysteresisController {
    cfg: HysteresisConfig,
    level: EmergencyLevel,
    last_deescalate_ns: Option<u64>,
}

impl HysteresisController {
    /// Config + başlangıç seviyesi.
    #[must_use]
    pub fn new(cfg: HysteresisConfig, initial: EmergencyLevel) -> Self {
        Self {
            cfg,
            level: initial,
            last_deescalate_ns: None,
        }
    }
}

impl RecoveryModeController for HysteresisController {
    fn update(&mut self, health_permille: u32, now_ns: u64) -> EmergencyLevel {
        if health_permille < self.cfg.escalate_permille && self.level < EmergencyLevel::Dup3 {
            // ESCALATE — ANINDA, dwell YOK (recovery-obsesif: hemen koru).
            self.level = self.level.escalated();
        } else if health_permille > self.cfg.deescalate_permille && self.level > EmergencyLevel::Arq
        {
            // DE-ESCALATE — dwell'li (osilasyon önlemi).
            let locked = self
                .last_deescalate_ns
                .is_some_and(|t| now_ns < t + dur_ns(self.cfg.deescalate_dwell));
            if !locked {
                self.level = self.level.deescalated();
                self.last_deescalate_ns = Some(now_ns);
            }
        }
        self.level
    }

    fn level(&self) -> EmergencyLevel {
        self.level
    }
}

/// Level + framing → forward-recovery eylemi (§11).
#[must_use]
pub fn action(level: EmergencyLevel, framing: Framing, cfg: &EmergencyConfig) -> RecoveryAction {
    match (level, framing) {
        (EmergencyLevel::Arq, _) => RecoveryAction::Arq,
        (EmergencyLevel::Dup2, Framing::Tiny) => RecoveryAction::Duplication {
            factor: cfg.dup2_factor,
            spacing: cfg.dup_spacing,
        },
        (EmergencyLevel::Dup3, Framing::Tiny) => RecoveryAction::Duplication {
            factor: cfg.dup3_factor,
            spacing: cfg.dup_spacing,
        },
        (EmergencyLevel::Dup2, Framing::SegmentBlock) => RecoveryAction::MinimalParity {
            permille: cfg.block_parity_dup2_permille,
        },
        (EmergencyLevel::Dup3, Framing::SegmentBlock) => RecoveryAction::MinimalParity {
            permille: cfg.block_parity_dup3_permille,
        },
    }
}

/// ACK decimation aktif mi (§11: emergency'de sparse ACK). Arq dışında `true`.
#[must_use]
pub fn ack_decimation(level: EmergencyLevel) -> bool {
    level != EmergencyLevel::Arq
}
