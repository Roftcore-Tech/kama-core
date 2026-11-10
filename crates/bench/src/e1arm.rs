//! [YOL3 · Ö] E1 teşhis kolu (`--e1-arm`, `--e1-edar`; CC §4.1 "teşhis kolları bench komut
//! satırından seçilir", KK-Ö-05). Yalnız `--transport kama-product`; iki uca AYNI verilir.
//!
//! [YOL3 · E2a kıyası] Aynı `--e1-arm` iki ek kol taşır (M1 `adaptive_transport::loss::E2aFlags`,
//! ISEMRI-YOL3-M1): `K1a` = K1 + E2a, `K0a` = yalnız E2a. E2a seti `tt_rfc` (g = 1 ms) +
//! `rtt_rule_v11` + `iw_idle_hold` + `pto_probes = 2`; `gap_ack` kapalı (uygulanmadı). Seçim
//! `kama_product::runtime_cfg`'de `RuntimeConfig.e2a_flags`'e yazılır, `profile_delta` listeler.
//! Diğer kolların E2a seti varsayılandır (kapalı = bugünkü davranış).
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
        }
    }

    /// Kol → bayraklar (M2 manifesti KK-M2-09 AYNEN): K0 hiçbiri · K0r retx_first ·
    /// K1 c+d+DrainRtprop+e-dar · K2 beşi · K2−c d+DrainRtprop+e-dar+retx_first ·
    /// K2−e c+d+DrainRtprop+retx_first. [E2a kıyası] K1a = K1'in E1 seti · K0a hiçbiri.
    pub fn set(self) -> E1Set {
        let (c, d, dr, e, r) = match self {
            Self::K0 | Self::K0a => (false, false, false, false, false),
            Self::K0r => (false, false, false, false, true),
            Self::K1 | Self::K1a => (true, true, true, true, false),
            Self::K2 => (true, true, true, true, true),
            Self::K2mc => (false, true, true, true, true),
            Self::K2me => (true, true, true, false, true),
        };
        E1Set {
            startup_cwnd: c,
            applimited_plateau: d,
            drainrt_gate: dr,
            probe_keep: e,
            retx_first: r,
        }
    }
}

impl E1Arm {
    /// [E2a kıyası] Kol → E2a seti: K1a ve K0a `tt_rfc` + `rtt_rule_v11` + `iw_idle_hold` +
    /// `pto_probes = 2`; diğer kollar varsayılan (kapalı, `pto_probes = 1`).
    pub fn e2a(self) -> E2aSet {
        match self {
            Self::K1a | Self::K0a => E2aSet {
                tt_rfc: true,
                rtt_rule_v11: true,
                iw_idle_hold: true,
                pto_probes: 2,
            },
            _ => E2aSet::default(),
        }
    }
}

/// [E2a kıyası] Bir kolun açtığı E2a bayrakları (M1 `E2aFlags` alanlarının sırası). `g`
/// (`tt_granularity_ns`) daima 1 ms (`E2A_TT_GRANULARITY_MIN_NS`, varsayılan); `gap_ack` kapalı.
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
}

impl Default for E2aSet {
    /// M1 `E2aFlags::default()` ile aynı: hepsi kapalı, `pto_probes = 1`.
    fn default() -> Self {
        Self {
            tt_rfc: false,
            rtt_rule_v11: false,
            iw_idle_hold: false,
            pto_probes: 1,
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
            false,
        )
    }

    /// Etiket: açık kurallar `+` ile (M1 alan adları); hiçbiri → `-`.
    pub fn label(self) -> String {
        let names: Vec<&str> = [
            (self.tt_rfc, "tt_rfc(g=1ms)"),
            (self.rtt_rule_v11, "rtt_rule_v11"),
            (self.iw_idle_hold, "iw_idle_hold"),
            (self.pto_probes == 2, "pto_probes=2"),
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
            if !matches!(arm, E1Arm::K1a | E1Arm::K0a) {
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
            }
        );
    }
}
