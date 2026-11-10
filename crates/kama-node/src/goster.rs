//! [YOL3 · Ö · KK-Ö-28] Gösterim profili `transport-v1.x-goster` — düğüme uygulanan değerlerin TEK
//! kaynağı (A yolu: telefonun ürün kurucusu [`crate::KamaNode::start`], `goster` özelliğiyle; L yolu:
//! ölçüm düzeneği `f0_flows --goster on`, `test-util` kancasıyla). Değerler
//! `docs/transport-v1/PRODUCT-PROFILE.md` gösterim bölümüyle aynıdır; fark çıkarsa o bölüm kazanır ve
//! bu dosya düzeltilir (ISEMRI-YOL3-M2 KK-M2-15).
//!
//! Gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.
//!
//! # Ne yazar ([`apply`])
//!
//! Yalnız dört alan; geri kalan ürün profili (`ClientTransportConfig::product`) aynen kalır:
//! * `runtime.fec = None` — PARAM_SET gönderilmez (sahip v4 Y1).
//! * `runtime.e1_flags` — gösterim kolu [`GOSTER_KOL`] (sahip v4 EK-A §2: "Gösterim seti = K1":
//!   (c) + (d) + DrainRtprop + (e-dar); retx_first KAPALI). Alanlar tek tek yazılır.
//! * `runtime.e2a_flags` — DOKUNULMAZ (ürün değeri, kapalı; K1 kolunun ölçüldüğü değer). v4 III.1
//!   "PTO prob sayısı 2" E2a setindedir; tipi (`adaptive_transport::loss::E2aFlags`, alanları özel,
//!   kurucusu `E2aFlags::new`) bu crate'ten adlandırılamaz (kama-node'un `adaptive-transport`
//!   bağımlılığı yok, D11) → E2a ya da A paketinin E2a bayrakları gösterim profiline girecekse
//!   client-core'un tipi yeniden dışa aktarması gerekir (açık kalem; bkz. KK-Ö-38).
//! * `lanes = Some(LaneConfig::with_file_window(GOSTER_FILE_WINDOW_UNITS))` — S1 şeritleri
//!   (YOL3-T1 KK-T1-05; W = 5 = `LaneConfig::default()`).
//!
//! [`playout`] PTT oynatma tamponunu verir (T = [`GOSTER_PLAYOUT_DELAY`], YOL3-T1 KK-T1-11;
//! "ölçüm yoksa 2,0 s", v4 III.1). `data_bind`, `peer_addrs`, zarf taşıyıcısı ve hub kipi seçimine
//! dokunulmaz (HB2 KK-H50, KK-H51).
//!
//! # Ne zaman etkin (KK-Ö-39)
//!
//! Ürün kurucusunda yalnız `goster` özelliği AÇIK ve `test-util` KAPALI iken (telefon `.so`'su:
//! `--features kama-node/goster`; kama-mobile `test-util` açmaz). Gerekçe: CI'ın test satırı
//! `cargo test --workspace --all-features` bütün özellikleri açar; profil `test-util` derlemelerinde
//! de etkin olsaydı kama-node testleri ürün yolunu değil gösterim profilini sınardı (ölçüldü: elle
//! saatli PTT-R4 testi oynatma tamponunda kalıyor). `test-util` ile birlikte derlenen bir `.so`'da
//! profil ETKİSİZDİR — gösterim ikilisi `test-util` açmadan derlenir.
//!
//! # Değiştirme
//!
//! Kol, W ve T bu dosyadaki sabitlerdir ([`GOSTER_E1`], [`GOSTER_FILE_WINDOW_UNITS`],
//! [`GOSTER_PLAYOUT_DELAY`]). A paketinin E1 bayrakları (`a3_burst_floor`, `a4_probe_srtt`; Q2) o
//! alanlar entegrasyon ağacına girince [`GosterE1`]'e eklenir. Paz 15 20:00 dondurmasından sonra
//! değişmez (sınıf M; KK-Ö-22).

use core::time::Duration;

use client_core::lanes::{LaneConfig, DEFAULT_FILE_WINDOW_UNITS};
use client_core::transport::ClientTransportConfig;
use kama_app::playout::{PlayoutConfig, PLAYOUT_DELAY_DEFAULT};

/// Gösterim kolunun adı (f0_flows / bench `--e1-arm` değeriyle aynı; koşu kimliği `kama-custom`).
pub const GOSTER_KOL: &str = "K1";

/// Gösterim kolunun E1 bayrakları (CC §4.1 adları; M2 `E1Flags` alanlarıyla aynı sıra).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GosterE1 {
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

/// K1 = (c) + (d) + DrainRtprop + (e-dar); retx_first kapalı (sahip v4 EK-A §2).
pub const GOSTER_E1: GosterE1 = GosterE1 {
    startup_cwnd: true,
    applimited_plateau: true,
    drainrt_gate: true,
    probe_keep: true,
    retx_first: false,
};

/// W: FILE şeridinin teyitsiz birim tavanı (v4 K6; YOL3-T1 gösterim varsayılanı).
pub const GOSTER_FILE_WINDOW_UNITS: u32 = DEFAULT_FILE_WINDOW_UNITS;

/// T: PTT oynatma tamponu (v4 III.1 "ölçüm yoksa 2,0 s"; mini-Ö okumasıyla kesinleşir).
pub const GOSTER_PLAYOUT_DELAY: Duration = PLAYOUT_DELAY_DEFAULT;

/// Gösterim profilini Transport yapılandırmasına yazar (yalnız modül belgesindeki alanlar).
pub fn apply(cfg: &mut ClientTransportConfig) {
    cfg.runtime.fec = None;
    let e1 = &mut cfg.runtime.e1_flags;
    e1.e1_startup_cwnd = GOSTER_E1.startup_cwnd;
    e1.e1_applimited_plateau = GOSTER_E1.applimited_plateau;
    e1.e1_drainrt_gate = GOSTER_E1.drainrt_gate;
    e1.e1_probe_keep = GOSTER_E1.probe_keep;
    e1.retx_first = GOSTER_E1.retx_first;
    cfg.lanes = Some(LaneConfig::with_file_window(GOSTER_FILE_WINDOW_UNITS));
}

/// Gösterim oynatma tamponu (T = [`GOSTER_PLAYOUT_DELAY`]). T izinli aralıktadır (birim testi
/// `goster_values_are_the_profile_rows`); aralık dışına çıkarılırsa kurucu tipli hata verir ve
/// burada varsayılan T'ye (2,0 s) düşülür — test bunu kırmızıyla yakalar.
#[must_use]
pub fn playout() -> PlayoutConfig {
    PlayoutConfig::new(GOSTER_PLAYOUT_DELAY).unwrap_or_default()
}

/// PTT motoruna gösterim oynatma tamponunu kurar (ürün kurucusu, `goster` ∧ ¬`test-util`; ve birim
/// testi). Ölçüm düzeneği aynı değeri [`playout`] ile alır.
#[cfg(any(test, all(feature = "goster", not(feature = "test-util"))))]
pub(crate) fn install_playout(ptt: &mut kama_app::ptt::Ptt) {
    ptt.set_playout(Some(playout()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Profil satırları: FEC yok, K1 bayrakları, E2a ürün değeri, şeritler W = 5, T = 2,0 s.
    #[test]
    fn goster_values_are_the_profile_rows() {
        let addr = "127.0.0.1:1".parse().expect("adres");
        let product = ClientTransportConfig::product(addr);
        let mut cfg = ClientTransportConfig::product(addr);
        apply(&mut cfg);
        assert!(
            product.runtime.fec.is_some(),
            "ürün profilinde FEC açık (taban)"
        );
        assert!(cfg.runtime.fec.is_none(), "gösterim: runtime.fec = None");
        let e1 = cfg.runtime.e1_flags;
        assert!(e1.e1_startup_cwnd && e1.e1_applimited_plateau && e1.e1_drainrt_gate);
        assert!(e1.e1_probe_keep && !e1.retx_first, "K1: retx_first kapalı");
        assert_eq!(
            cfg.runtime.e2a_flags, product.runtime.e2a_flags,
            "E2a dokunulmaz"
        );
        let lanes = cfg.lanes.expect("şeritler açık");
        assert_eq!(
            lanes,
            LaneConfig::default(),
            "W = 5 = LaneConfig::default()"
        );
        assert_eq!(lanes.file_window_units, 5);
        assert_eq!(GOSTER_KOL, "K1");
        assert!(
            PlayoutConfig::new(GOSTER_PLAYOUT_DELAY).is_ok(),
            "T izinli aralıkta"
        );
        assert_eq!(playout().delay(), Duration::from_millis(2000));
    }

    /// `apply` yalnız dört alanı yazar: geri kalan ürün yapılandırması aynen.
    #[test]
    fn apply_leaves_the_rest_of_the_product_config() {
        let addr = "127.0.0.1:1".parse().expect("adres");
        let product = ClientTransportConfig::product(addr);
        let mut cfg = ClientTransportConfig::product(addr);
        apply(&mut cfg);
        cfg.runtime.fec = product.runtime.fec;
        cfg.runtime.e1_flags = product.runtime.e1_flags;
        assert!(product.lanes.is_none() && cfg.lanes.is_some());
        assert_eq!(
            format!("{:?}", cfg.runtime),
            format!("{:?}", product.runtime)
        );
        assert_eq!(cfg.data_bind, product.data_bind);
        assert_eq!(cfg.peer_addrs, product.peer_addrs);
        assert_eq!(cfg.recovery, product.recovery);
        assert_eq!(cfg.renew_drain_cap, product.renew_drain_cap);
    }
}
