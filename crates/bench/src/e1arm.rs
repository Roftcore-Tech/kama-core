//! [YOL3 · Ö] E1 teşhis kolu (`--e1-arm`, `--e1-edar`; CC §4.1 "teşhis kolları bench komut
//! satırından seçilir", KK-Ö-05). Yalnız `--transport kama-product`; iki uca AYNI verilir.
//!
//! [YOL3 · E2a kıyası] Aynı `--e1-arm` iki ek kol taşır (M1 `adaptive_transport::loss::E2aFlags`,
//! ISEMRI-YOL3-M1): `K1a` = K1 + E2a, `K0a` = yalnız E2a. E2a seti `tt_rfc` (g = 1 ms) +
//! `rtt_rule_v11` + `iw_idle_hold` + `pto_probes = 2`; `gap_ack` kapalı (uygulanmadı). Seçim
//! `kama_product::runtime_cfg`'de `RuntimeConfig.e2a_flags`'e yazılır, `profile_delta` listeler.
//! Diğer kolların E2a seti varsayılandır (kapalı = bugünkü davranış).
//!
//! [YOL3 · A paketi merdiveni] Q1 (E2aFlags: `gap_ack` A-1, `return_prior_gated` A-2, `pto_rfc`
//! A-5) ve Q2 (E1Flags: `a3_burst_floor` A-3, `a4_probe_srtt` A-4) kolları, hepsi K1 üstüne:
//! `K1g` = K1 + gap_ack · `K1gr` = K1g + return_prior_gated · `K1grp` = K1gr + pto_rfc +
//! pto_probes = 2 · `K1A` = K1 + E2a'nın hepsi (K1a'nın dördü + gap_ack + return_prior_gated +
//! pto_rfc) + a3_burst_floor + a4_probe_srtt. Ürün değerleri (kapalı) diğer bütün kollarda.
//!
//! [YOL3 · A-6 gece] `K1A6` = K1A + Q2 A-6 `a6_iw_floor` (E1Flags; Startup bw-terimi tabanı
//! 10·MTU/srtt, deney, varsayılan kapalı). E2a seti K1A'nınki.
//!
//! [YOL3 · gösterim ablasyonu] Gösterim seti = `K1A6` (sahip kararı 2026-11-10 madde 3; kama-node
//! `goster::GOSTER_KOL`). R3 koruma hücresi tutmazsa bırak-birini-çıkar kolları (madde 3/4):
//! `K1A6mA1` = K1A6 − gap_ack · `K1A6mA2` = − return_prior_gated · `K1A6mA3` = − a3_burst_floor ·
//! `K1A6mA4` = − a4_probe_srtt · `K1A6mA5` = − pto_rfc (`pto_probes = 2` M1 E2a setinde kalır) ·
//! K1A6 − a6_iw_floor = `K1A` (yeni ad yok). Ad deseni `K2mc` = K2−c ile aynı (`m` = eksi).
//!
//! Kol → bayrak tablosu M2'nin teşhis manifestindeki tablonun aynısıdır (`docs/design/
//! E1-TESHIS-G2.md`, KK-M2-09); `crates/kama-integration/examples/f0_flows.rs` `E1Arm::set` ile
//! aynı. Seçim süreç başına bir kez [`set`] edilir (main; `rival::set` deseni) ve
//! `kama_product::runtime_cfg` onu `RuntimeConfig.e1_flags`'e (M2, `adaptive_transport::rate::
//! E1Flags`) yazar; `profile_delta` sapmayı listeler (INV-009, KK-Ö-26). Bayraklı kol
//! `kama-product` dışında koşu başlatmaz ([`E1Sel::refuse_unavailable`]).

use std::sync::OnceLock;

use adaptive_transport::loss::{E2aFlags, E2aFlagsError, E2A_TT_GRANULARITY_MIN_NS};
use adaptive_transport::rate::E1Flags;
use clap::ValueEnum;

/// E1 teşhis kolu. Adlar `f0_cell.sh` `ARM_<kol>_SRV` değişkenine uysun diye K2−c → `K2mc`,
/// K2−e → `K2me`. Varsayılan `K0` = bayraklar kapalı (bugünkü davranış).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum E1Arm {
    #[default]
    #[value(name = "K0")]
    K0,
    #[value(name = "K0r")]
    K0r,
    #[value(name = "K1")]
    K1,
    #[value(name = "K2")]
    K2,
    #[value(name = "K2mc")]
    K2mc,
    #[value(name = "K2me")]
    K2me,
    /// [E2a kıyası] K1 + E2a seti (K1 ↔ K1a tek değişken: yalnız E2a).
    #[value(name = "K1a")]
    K1a,
    /// [E2a kıyası] Yalnız E2a seti (E1 kapalı; K0 ↔ K0a tek değişken).
    #[value(name = "K0a")]
    K0a,
    /// [A merdiveni] K1 + gap_ack (Q1 A-1).
    #[value(name = "K1g")]
    K1g,
    /// [A merdiveni] K1g + return_prior_gated (Q1 A-2).
    #[value(name = "K1gr")]
    K1gr,
    /// [A merdiveni] K1gr + pto_rfc + pto_probes = 2 (Q1 A-5).
    #[value(name = "K1grp")]
    K1grp,
    /// [A merdiveni] K1 + E2a'nın hepsi + a3_burst_floor + a4_probe_srtt (Q1 + Q2).
    #[value(name = "K1A")]
    K1A,
    /// [A-6 gece] K1A + a6_iw_floor (Q2 A-6).
    #[value(name = "K1A6")]
    K1A6,
    /// [Ablasyon] K1A6 − gap_ack (A-1).
    #[value(name = "K1A6mA1")]
    K1A6mA1,
    /// [Ablasyon] K1A6 − return_prior_gated (A-2).
    #[value(name = "K1A6mA2")]
    K1A6mA2,
    /// [Ablasyon] K1A6 − a3_burst_floor (A-3).
    #[value(name = "K1A6mA3")]
    K1A6mA3,
    /// [Ablasyon] K1A6 − a4_probe_srtt (A-4).
    #[value(name = "K1A6mA4")]
    K1A6mA4,
    /// [Ablasyon] K1A6 − pto_rfc (A-5; `pto_probes = 2` kalır).
    #[value(name = "K1A6mA5")]
    K1A6mA5,
}

/// `--e1-edar on|off` (varsayılan `on`): `off` → ön test geçmediyse (CC §4.2) kolun setinden
/// (e-dar) `e1_probe_keep` düşer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum E1Edar {
    #[default]
    On,
    Off,
}

/// Bir kolun açtığı E1 bayrakları (CC §4.1 adları; M2 `E1Flags` alanlarının sırası).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E1Set {
    /// (c) `e1_startup_cwnd`.
    pub startup_cwnd: bool,
    /// (d) `e1_applimited_plateau`.
    pub applimited_plateau: bool,
    /// DrainRtprop `e1_drainrt_gate`.
    pub drainrt_gate: bool,
    /// (e-dar) `e1_probe_keep`.
    pub probe_keep: bool,
    /// `retx_first`.
    pub retx_first: bool,
    /// [Q2 A-3] `a3_burst_floor`.
    pub a3_burst_floor: bool,
    /// [Q2 A-4] `a4_probe_srtt`.
    pub a4_probe_srtt: bool,
    /// [Q2 A-6] `a6_iw_floor`.
    pub a6_iw_floor: bool,
}

impl E1Arm {
    /// Kolun adı (komut satırı değeri).
    pub fn name(self) -> &'static str {
        match self {
            Self::K0 => "K0",
            Self::K0r => "K0r",
            Self::K1 => "K1",
            Self::K2 => "K2",
            Self::K2mc => "K2mc",
            Self::K2me => "K2me",
            Self::K1a => "K1a",
            Self::K0a => "K0a",
            Self::K1g => "K1g",
            Self::K1gr => "K1gr",
            Self::K1grp => "K1grp",
            Self::K1A => "K1A",
            Self::K1A6 => "K1A6",
            Self::K1A6mA1 => "K1A6mA1",
            Self::K1A6mA2 => "K1A6mA2",
            Self::K1A6mA3 => "K1A6mA3",
            Self::K1A6mA4 => "K1A6mA4",
            Self::K1A6mA5 => "K1A6mA5",
        }
    }

    /// [Ablasyon] K1A6 ailesi: K1A6 ve bırak-birini-çıkar kolları (a6_iw_floor hepsinde açık).
    fn k1a6_family(self) -> bool {
        matches!(
            self,
            Self::K1A6
                | Self::K1A6mA1
                | Self::K1A6mA2
                | Self::K1A6mA3
                | Self::K1A6mA4
                | Self::K1A6mA5
        )
    }

    /// Kol → bayraklar (M2 manifesti KK-M2-09 AYNEN): K0 hiçbiri · K0r retx_first ·
    /// K1 c+d+DrainRtprop+e-dar · K2 beşi · K2−c d+DrainRtprop+e-dar+retx_first ·
    /// K2−e c+d+DrainRtprop+retx_first. [E2a kıyası] K1a = K1'in E1 seti · K0a hiçbiri.
    /// [A merdiveni] K1g, K1gr, K1grp = K1'in E1 seti · K1A = K1 + a3_burst_floor + a4_probe_srtt.
    /// [A-6 gece] K1A6 = K1A + a6_iw_floor. [Ablasyon] K1A6mA3 a3'süz, K1A6mA4 a4'süz; diğer
    /// ablasyon kollarının E1 seti K1A6'nınki.
    pub fn set(self) -> E1Set {
        let (c, d, dr, e, r) = match self {
            Self::K0 | Self::K0a => (false, false, false, false, false),
            Self::K0r => (false, false, false, false, true),
            Self::K1
            | Self::K1a
            | Self::K1g
            | Self::K1gr
            | Self::K1grp
            | Self::K1A
            | Self::K1A6
            | Self::K1A6mA1
            | Self::K1A6mA2
            | Self::K1A6mA3
            | Self::K1A6mA4
            | Self::K1A6mA5 => (true, true, true, true, false),
            Self::K2 => (true, true, true, true, true),
            Self::K2mc => (false, true, true, true, true),
            Self::K2me => (true, true, true, false, true),
        };
        let a6 = self.k1a6_family();
        let q2 = self == Self::K1A || a6;
        E1Set {
            startup_cwnd: c,
            applimited_plateau: d,
            drainrt_gate: dr,
            probe_keep: e,
            retx_first: r,
            a3_burst_floor: q2 && self != Self::K1A6mA3,
            a4_probe_srtt: q2 && self != Self::K1A6mA4,
            a6_iw_floor: a6,
        }
    }
}

impl E1Arm {
    /// [E2a kıyası] Kol → E2a seti: K1a ve K0a `tt_rfc` + `rtt_rule_v11` + `iw_idle_hold` +
    /// `pto_probes = 2`. [A merdiveni] K1g gap_ack · K1gr + return_prior_gated · K1grp + pto_rfc +
    /// pto_probes = 2 · K1A yedisi. [Ablasyon] K1A6mA1 gap_ack'sız, K1A6mA2 return_prior_gated'sız,
    /// K1A6mA5 pto_rfc'siz (pto_probes = 2 kalır); K1A6mA3/mA4 yedisi. Diğer kollar varsayılan
    /// (kapalı, `pto_probes = 1`).
    pub fn e2a(self) -> E2aSet {
        let (tt, v11, iw, probes, gap, rpg, pto) = match self {
            Self::K1a | Self::K0a => (true, true, true, 2, false, false, false),
            Self::K1g => (false, false, false, 1, true, false, false),
            Self::K1gr => (false, false, false, 1, true, true, false),
            Self::K1grp => (false, false, false, 2, true, true, true),
            Self::K1A | Self::K1A6 | Self::K1A6mA3 | Self::K1A6mA4 => {
                (true, true, true, 2, true, true, true)
            }
            Self::K1A6mA1 => (true, true, true, 2, false, true, true),
            Self::K1A6mA2 => (true, true, true, 2, true, false, true),
            Self::K1A6mA5 => (true, true, true, 2, true, true, false),
            _ => return E2aSet::default(),
        };
        E2aSet {
            tt_rfc: tt,
            rtt_rule_v11: v11,
            iw_idle_hold: iw,
            pto_probes: probes,
            gap_ack: gap,
            return_prior_gated: rpg,
            pto_rfc: pto,
        }
    }
}

/// [E2a kıyası] Bir kolun açtığı E2a bayrakları (M1 + Q1 `E2aFlags` alanlarının sırası). `g`
/// (`tt_granularity_ns`) daima 1 ms (`E2A_TT_GRANULARITY_MIN_NS`, varsayılan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct E2aSet {
    /// m1 `tt_rfc` (g = 1 ms).
    pub tt_rfc: bool,
    /// m8 `rtt_rule_v11`.
    pub rtt_rule_v11: bool,
    /// m6 `iw_idle_hold`.
    pub iw_idle_hold: bool,
    /// PTO turu başına prob sayısı ({1, 2}).
    pub pto_probes: u8,
    /// [Q1 A-1] m5 `gap_ack`.
    pub gap_ack: bool,
    /// [Q1 A-2] `return_prior_gated`.
    pub return_prior_gated: bool,
    /// [Q1 A-5] `pto_rfc`.
    pub pto_rfc: bool,
}

impl Default for E2aSet {
    /// `E2aFlags::default()` ile aynı: hepsi kapalı, `pto_probes = 1`.
    fn default() -> Self {
        Self {
            tt_rfc: false,
            rtt_rule_v11: false,
            iw_idle_hold: false,
            pto_probes: 1,
            gap_ack: false,
            return_prior_gated: false,
            pto_rfc: false,
        }
    }
}

impl E2aSet {
    /// Varsayılandan farklı mı (en az bir E2a kuralı açık).
    pub fn any(self) -> bool {
        self != Self::default()
    }

    /// M1'in çalışma zamanı bayrak yapısı (`RuntimeConfig.e2a_flags`); doğrulama tip kurucusunda.
    ///
    /// # Errors
    /// [`E2aFlagsError`] (`E2aFlags::new`); kol tablosunun değerleri geçerlidir.
    pub fn flags(self) -> Result<E2aFlags, E2aFlagsError> {
        E2aFlags::new(
            self.tt_rfc,
            E2A_TT_GRANULARITY_MIN_NS,
            self.rtt_rule_v11,
            self.iw_idle_hold,
            self.pto_probes,
            self.gap_ack,
        )
        .map(|f| {
            f.with_return_prior_gated(self.return_prior_gated)
                .with_pto_rfc(self.pto_rfc)
        })
    }

    /// Etiket: açık kurallar `+` ile (M1 alan adları); hiçbiri → `-`.
    pub fn label(self) -> String {
        let names: Vec<&str> = [
            (self.tt_rfc, "tt_rfc(g=1ms)"),
            (self.rtt_rule_v11, "rtt_rule_v11"),
            (self.iw_idle_hold, "iw_idle_hold"),
            (self.pto_probes == 2, "pto_probes=2"),
            (self.gap_ack, "gap_ack"),
            (self.return_prior_gated, "return_prior_gated"),
            (self.pto_rfc, "pto_rfc"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        if names.is_empty() {
            "-".to_owned()
        } else {
            names.join("+")
        }
    }
}

impl E1Set {
    /// En az bir bayrak açık mı.
    pub fn any(self) -> bool {
        self != Self::default()
    }

    /// M2'nin çalışma zamanı bayrak yapısı (`RuntimeConfig.e1_flags`).
    pub fn flags(self) -> E1Flags {
        E1Flags {
            e1_startup_cwnd: self.startup_cwnd,
            e1_applimited_plateau: self.applimited_plateau,
            e1_drainrt_gate: self.drainrt_gate,
            e1_probe_keep: self.probe_keep,
            retx_first: self.retx_first,
            a3_burst_floor: self.a3_burst_floor,
            a4_probe_srtt: self.a4_probe_srtt,
            a6_iw_floor: self.a6_iw_floor,
        }
    }

    /// Etiket: açık bayraklar `+` ile (CC §4.2 adları); hiçbiri → `-`.
    pub fn label(self) -> String {
        let names: Vec<&str> = [
            (self.startup_cwnd, "c"),
            (self.applimited_plateau, "d"),
            (self.drainrt_gate, "DrainRtprop"),
            (self.probe_keep, "e-dar"),
            (self.retx_first, "retx_first"),
            (self.a3_burst_floor, "a3_burst_floor"),
            (self.a4_probe_srtt, "a4_probe_srtt"),
            (self.a6_iw_floor, "a6_iw_floor"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        if names.is_empty() {
            "-".to_owned()
        } else {
            names.join("+")
        }
    }
}

/// Komut satırı seçimi: kol + (e-dar) anahtarı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E1Sel {
    /// `--e1-arm`.
    pub arm: E1Arm,
    /// `--e1-edar`.
    pub edar: E1Edar,
}

impl E1Sel {
    /// Etkin bayrak seti (`--e1-edar off` ise `e1_probe_keep` düşmüş).
    pub fn set(self) -> E1Set {
        let mut set = self.arm.set();
        if self.edar == E1Edar::Off {
            set.probe_keep = false;
        }
        set
    }

    /// [E2a kıyası] Kolun E2a seti (`--e1-edar` E2a'yı etkilemez).
    pub fn e2a(self) -> E2aSet {
        self.arm.e2a()
    }

    /// Koşu ÖNCESİ ret: bayraklı kol (E1 ya da E2a) yalnız `kama-product`'ta (rakip kollarda
    /// yok; sessizce yok sayılmaz).
    pub fn refuse_unavailable(self, kama_product: bool) -> Result<(), String> {
        if !(self.set().any() || self.e2a().any()) || kama_product {
            return Ok(());
        }
        Err(format!(
            "--e1-arm {} (E1 {}; E2a {}) yalnız --transport kama-product ile",
            self.arm.name(),
            self.set().label(),
            self.e2a().label()
        ))
    }
}

/// Seçim ve doğrulanmış E2a bayrakları (`E2aFlags::new` [`set`]'te bir kez koşar).
static SEL: OnceLock<(E1Sel, E2aFlags)> = OnceLock::new();

/// Süreç başına bir kez (main). İkinci çağrı yok sayılır.
///
/// # Errors
/// Kolun E2a seti `E2aFlags::new` doğrulamasından geçmezse (koşu başlamaz; kol tablosu geçerlidir).
pub fn set(sel: E1Sel) -> Result<(), String> {
    let e2a = sel
        .e2a()
        .flags()
        .map_err(|e| format!("--e1-arm {}: E2a bayrakları geçersiz: {e}", sel.arm.name()))?;
    let _ = SEL.set((sel, e2a));
    Ok(())
}

/// Yürürlükteki seçim; hiç [`set`] edilmediyse K0 (bayraklar kapalı, bugünkü davranış).
pub fn current() -> E1Sel {
    SEL.get().map(|s| s.0).unwrap_or_default()
}

/// [E2a kıyası] Yürürlükteki E2a bayrakları; hiç [`set`] edilmediyse `E2aFlags::default()` (ürün).
pub fn current_e2a() -> E2aFlags {
    SEL.get().map(|s| s.1).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_table_matches_the_teshis_manifest() {
        let want = [
            ("K0", "-"),
            ("K0r", "retx_first"),
            ("K1", "c+d+DrainRtprop+e-dar"),
            ("K2", "c+d+DrainRtprop+e-dar+retx_first"),
            ("K2mc", "d+DrainRtprop+e-dar+retx_first"),
            ("K2me", "c+d+DrainRtprop+retx_first"),
            ("K1a", "c+d+DrainRtprop+e-dar"),
            ("K0a", "-"),
            ("K1g", "c+d+DrainRtprop+e-dar"),
            ("K1gr", "c+d+DrainRtprop+e-dar"),
            ("K1grp", "c+d+DrainRtprop+e-dar"),
            ("K1A", "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt"),
            (
                "K1A6",
                "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor",
            ),
            (
                "K1A6mA1",
                "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor",
            ),
            (
                "K1A6mA2",
                "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor",
            ),
            ("K1A6mA3", "c+d+DrainRtprop+e-dar+a4_probe_srtt+a6_iw_floor"),
            ("K1A6mA4", "c+d+DrainRtprop+e-dar+a3_burst_floor+a6_iw_floor"),
            (
                "K1A6mA5",
                "c+d+DrainRtprop+e-dar+a3_burst_floor+a4_probe_srtt+a6_iw_floor",
            ),
        ];
        for (name, label) in want {
            let arm = E1Arm::from_str(name, false).expect("kol adı ayrışmalı");
            assert_eq!(arm.name(), name);
            assert_eq!(arm.set().label(), label, "{name}");
        }
        assert_eq!(E1Arm::value_variants().len(), want.len());
        assert!(
            E1Arm::from_str("k2mc", false).is_err(),
            "adlar büyük/küçük harf duyarlı"
        );
    }

    #[test]
    fn edar_off_drops_probe_keep_only() {
        let k2 = E1Sel {
            arm: E1Arm::K2,
            edar: E1Edar::Off,
        };
        assert_eq!(k2.set(), E1Arm::K2me.set(), "(e-dar) çıkınca K2 = K2−e");
        let k0r = E1Sel {
            arm: E1Arm::K0r,
            edar: E1Edar::Off,
        };
        assert_eq!(k0r.set(), E1Arm::K0r.set());
        assert_eq!(E1Edar::from_str("off", false), Ok(E1Edar::Off));
    }

    #[test]
    fn flagged_arms_run_only_on_kama_product() {
        assert_eq!(E1Sel::default().refuse_unavailable(true), Ok(()));
        assert_eq!(E1Sel::default().refuse_unavailable(false), Ok(()));
        let k2mc = E1Sel {
            arm: E1Arm::K2mc,
            edar: E1Edar::On,
        };
        assert_eq!(k2mc.refuse_unavailable(true), Ok(()));
        assert!(k2mc
            .refuse_unavailable(false)
            .expect_err("rakip kolda E1 yok")
            .contains("kama-product"));
    }

    #[test]
    fn e2a_arms_differ_from_their_base_only_in_e2a() {
        let on = "tt_rfc(g=1ms)+rtt_rule_v11+iw_idle_hold+pto_probes=2";
        for (arm, base) in [(E1Arm::K1a, E1Arm::K1), (E1Arm::K0a, E1Arm::K0)] {
            assert_eq!(
                arm.set(),
                base.set(),
                "{}: E1 seti tabanla aynı",
                arm.name()
            );
            assert_eq!(arm.e2a().label(), on, "{}", arm.name());
            assert!(!base.e2a().any(), "{}: E2a kapalı", base.name());
            let f = arm.e2a().flags().expect("kol tablosu geçerli");
            assert!(f.tt_rfc() && f.rtt_rule_v11() && f.iw_idle_hold() && !f.gap_ack());
            assert_eq!((f.tt_granularity_ns(), f.pto_probes()), (1_000_000, 2));
        }
        for arm in E1Arm::value_variants() {
            if !matches!(
                arm,
                E1Arm::K1a
                    | E1Arm::K0a
                    | E1Arm::K1g
                    | E1Arm::K1gr
                    | E1Arm::K1grp
                    | E1Arm::K1A
                    | E1Arm::K1A6
                    | E1Arm::K1A6mA1
                    | E1Arm::K1A6mA2
                    | E1Arm::K1A6mA3
                    | E1Arm::K1A6mA4
                    | E1Arm::K1A6mA5
            ) {
                assert_eq!(arm.e2a().flags(), Ok(E2aFlags::default()), "{}", arm.name());
                assert_eq!(arm.e2a().label(), "-");
            }
        }
        let k1a = E1Sel {
            arm: E1Arm::K1a,
            edar: E1Edar::Off,
        };
        assert_eq!(
            k1a.set().label(),
            "c+d+DrainRtprop",
            "(e-dar) düşer, E2a kalır"
        );
        assert_eq!(k1a.e2a(), E1Arm::K1a.e2a());
        let k0a = E1Sel {
            arm: E1Arm::K0a,
            edar: E1Edar::On,
        };
        assert_eq!(k0a.refuse_unavailable(true), Ok(()));
        assert!(k0a
            .refuse_unavailable(false)
            .expect_err("rakip kolda E2a yok")
            .contains("kama-product"));
    }

    /// [A merdiveni] Her kolun E1 ve E2a seti tam olarak tanımdaki gibi; K1'den farkı yalnız
    /// sayılan bayraklar (tek ikili, `--e1-arm`).
    #[test]
    fn a_ladder_arms_match_the_definition() {
        let k1 = E1Arm::K1.set().flags();
        let want = [
            (E1Arm::K1g, "gap_ack", false),
            (E1Arm::K1gr, "gap_ack+return_prior_gated", false),
            (
                E1Arm::K1grp,
                "pto_probes=2+gap_ack+return_prior_gated+pto_rfc",
                false,
            ),
            (
                E1Arm::K1A,
                "tt_rfc(g=1ms)+rtt_rule_v11+iw_idle_hold+pto_probes=2+gap_ack+\
                 return_prior_gated+pto_rfc",
                true,
            ),
        ];
        for (arm, e2a_label, q2) in want {
            assert_eq!(arm.e2a().label(), e2a_label, "{}", arm.name());
            let f = arm.e2a().flags().expect("kol tablosu geçerli");
            assert_eq!(f.gap_ack(), arm.e2a().gap_ack, "{}", arm.name());
            assert_eq!(f.return_prior_gated(), arm.e2a().return_prior_gated);
            assert_eq!(f.pto_rfc(), arm.e2a().pto_rfc);
            assert_eq!(f.pto_probes(), arm.e2a().pto_probes);
            let e1 = arm.set().flags();
            assert_eq!(
                (e1.a3_burst_floor, e1.a4_probe_srtt),
                (q2, q2),
                "{}",
                arm.name()
            );
            assert_eq!(
                E1Flags {
                    a3_burst_floor: false,
                    a4_probe_srtt: false,
                    ..e1
                },
                k1,
                "{}: E1 tabanı K1",
                arm.name()
            );
        }
        let all = E1Arm::K1A.e2a().flags().expect("geçerli");
        assert!(all.tt_rfc() && all.rtt_rule_v11() && all.iw_idle_hold());
        assert_eq!(all.tt_granularity_ns(), 1_000_000);
    }

    /// [A-6 gece] K1A6 = K1A + a6_iw_floor: E1'de tek fark a6, E2a seti K1A'nınki; a6 yalnız K1A6'da.
    #[test]
    fn k1a6_is_k1a_plus_a6_only() {
        let a6 = E1Arm::K1A6.set().flags();
        assert!(a6.a6_iw_floor);
        assert_eq!(
            E1Flags {
                a6_iw_floor: false,
                ..a6
            },
            E1Arm::K1A.set().flags()
        );
        assert_eq!(E1Arm::K1A6.e2a(), E1Arm::K1A.e2a());
        for arm in E1Arm::value_variants() {
            assert_eq!(
                arm.set().flags().a6_iw_floor,
                arm.name().starts_with("K1A6"),
                "{}: a6 yalnız K1A6 ailesinde",
                arm.name()
            );
        }
    }

    /// [Ablasyon] Her K1A6mAi kolu K1A6'dan TAM BİR bayrakla ayrılır; K1A6 − A-6 = K1A.
    #[test]
    fn ablation_arms_drop_exactly_one_a_flag() {
        let d = E1Arm::K1A6;
        let (e1, e2a) = (d.set(), d.e2a());
        let want = [
            (
                E1Arm::K1A6mA1,
                e1,
                E2aSet {
                    gap_ack: false,
                    ..e2a
                },
            ),
            (
                E1Arm::K1A6mA2,
                e1,
                E2aSet {
                    return_prior_gated: false,
                    ..e2a
                },
            ),
            (
                E1Arm::K1A6mA3,
                E1Set {
                    a3_burst_floor: false,
                    ..e1
                },
                e2a,
            ),
            (
                E1Arm::K1A6mA4,
                E1Set {
                    a4_probe_srtt: false,
                    ..e1
                },
                e2a,
            ),
            (
                E1Arm::K1A6mA5,
                e1,
                E2aSet {
                    pto_rfc: false,
                    ..e2a
                },
            ),
            (
                E1Arm::K1A,
                E1Set {
                    a6_iw_floor: false,
                    ..e1
                },
                e2a,
            ),
        ];
        for (arm, want_e1, want_e2a) in want {
            assert_eq!(arm.set(), want_e1, "{}", arm.name());
            assert_eq!(arm.e2a(), want_e2a, "{}", arm.name());
            assert!(arm.e2a().flags().is_ok(), "{}", arm.name());
            assert_ne!((arm.set(), arm.e2a()), (e1, e2a), "{}: tek fark", arm.name());
        }
        assert_eq!(E1Arm::K1A6mA5.e2a().pto_probes, 2, "A-5 düşünce PTO 2 kalır");
    }

    #[test]
    fn flags_map_field_by_field() {
        assert_eq!(E1Arm::K0.set().flags(), E1Flags::default());
        assert_eq!(
            E1Arm::K2mc.set().flags(),
            E1Flags {
                e1_startup_cwnd: false,
                e1_applimited_plateau: true,
                e1_drainrt_gate: true,
                e1_probe_keep: true,
                retx_first: true,
                a3_burst_floor: false,
                a4_probe_srtt: false,
                a6_iw_floor: false,
            }
        );
        assert_eq!(
            E1Arm::K1.set().flags(),
            E1Flags {
                e1_startup_cwnd: true,
                e1_applimited_plateau: true,
                e1_drainrt_gate: true,
                e1_probe_keep: true,
                retx_first: false,
                a3_burst_floor: false,
                a4_probe_srtt: false,
                a6_iw_floor: false,
            }
        );
    }
}
