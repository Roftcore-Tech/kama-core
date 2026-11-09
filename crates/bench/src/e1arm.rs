//! [YOL3 · Ö] E1 teşhis kolu (`--e1-arm`, `--e1-edar`; CC §4.1 "teşhis kolları bench komut
//! satırından seçilir", KK-Ö-05). Yalnız `--transport kama-product`; iki uca AYNI verilir.
//!
//! Kol → bayrak tablosu M2'nin teşhis manifestindeki tablonun aynısıdır (`docs/design/
//! E1-TESHIS-G2.md`, KK-M2-09); `crates/kama-integration/examples/f0_flows.rs` `E1Arm::set` ile
//! aynı. Seçim süreç başına bir kez [`set`] edilir (main; `rival::set` deseni) ve
//! `kama_product::runtime_cfg` onu `RuntimeConfig.e1_flags`'e (M2, `adaptive_transport::rate::
//! E1Flags`) yazar; `profile_delta` sapmayı listeler (INV-009, KK-Ö-26). Bayraklı kol
//! `kama-product` dışında koşu başlatmaz ([`E1Sel::refuse_unavailable`]).

use std::sync::OnceLock;

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
        }
    }

    /// Kol → bayraklar (M2 manifesti KK-M2-09 AYNEN): K0 hiçbiri · K0r retx_first ·
    /// K1 c+d+DrainRtprop+e-dar · K2 beşi · K2−c d+DrainRtprop+e-dar+retx_first ·
    /// K2−e c+d+DrainRtprop+retx_first.
    pub fn set(self) -> E1Set {
        let (c, d, dr, e, r) = match self {
            Self::K0 => (false, false, false, false, false),
            Self::K0r => (false, false, false, false, true),
            Self::K1 => (true, true, true, true, false),
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

    /// Koşu ÖNCESİ ret: bayraklı kol yalnız `kama-product`'ta (rakip kollarda E1 yok; sessizce
    /// yok sayılmaz).
    pub fn refuse_unavailable(self, kama_product: bool) -> Result<(), String> {
        if !self.set().any() || kama_product {
            return Ok(());
        }
        Err(format!(
            "--e1-arm {} ({}) yalnız --transport kama-product ile",
            self.arm.name(),
            self.set().label()
        ))
    }
}

static SEL: OnceLock<E1Sel> = OnceLock::new();

/// Süreç başına bir kez (main). İkinci çağrı yok sayılır.
pub fn set(sel: E1Sel) {
    let _ = SEL.set(sel);
}

/// Yürürlükteki seçim; hiç [`set`] edilmediyse K0 (bayraklar kapalı, bugünkü davranış).
pub fn current() -> E1Sel {
    SEL.get().copied().unwrap_or_default()
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
