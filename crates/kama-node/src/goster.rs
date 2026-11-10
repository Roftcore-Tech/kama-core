//! [YOL3 · Ö · KK-Ö-28] Gösterim profili `transport-v1.x-goster` — düğüme uygulanan değerlerin TEK
//! kaynağı (A yolu: telefonun ürün kurucusu [`crate::KamaNode::start`], `goster` özelliğiyle; L yolu:
//! ölçüm düzeneği `f0_flows --goster on`, `test-util` kancasıyla). Değerler
//! `docs/transport-v1/PRODUCT-PROFILE.md` gösterim bölümüyle aynıdır; fark çıkarsa o bölüm kazanır ve
//! bu dosya düzeltilir (ISEMRI-YOL3-M2 KK-M2-15).
//!
//! Gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; sığ tamponlu ağ gösterim kapsamı
//! dışı.
//!
//! # Gösterim seti
//!
//! Sahip kararı 2026-11-10 madde 3: "Gösterim seti = K1 + A paketi (A-1…A-6) + şeritler". Gece
//! ölçümünün (2026-11-10) `K1A6s` kolunun aynısı: `f0_flows --e1-arm K1A6 --fec-off on --goster on`.
//! Bayraklar varsayılan kapalı deney kollarıdır; burada yalnız gösterim profilinde açılır (ürün
//! varsayılanları `KamaV1Profile`, `RuntimeConfig::default` değişmez). A-6 "ısınmadan öğrenilen
//! uygulamayla-sınırlı bant Startup'ı tohumlamaz" kuralı kodda `max` olarak okunur (sahip madde 3:
//! kabul). Sığ tamponlu ağda (netem `limit` ≈ 2 × BDP) bu set çöküyor (gece ölçümü §4); gösterim
//! derin kuyrukta koşar, sığ tamponlu ağ kapsam dışıdır (sahip madde 5).
//!
//! # Ne yazar ([`apply`])
//!
//! Yalnız dört alan; geri kalan ürün profili (`ClientTransportConfig::product`) aynen kalır:
//! * `runtime.fec = None` — PARAM_SET gönderilmez (sahip v4 Y1).
//! * `runtime.e1_flags = `[`GOSTER_E1`] — (c) + (d) + DrainRtprop + (e-dar) + A-3 + A-4 + A-6;
//!   `retx_first` KAPALI.
//! * `runtime.e2a_flags = `[`e2a_flags`]`()` — [`GOSTER_E2A`]: m1 `tt_rfc` (g = 1 ms) + m8
//!   `rtt_rule_v11` + m6 `iw_idle_hold` + `pto_probes = 2` + A-1 `gap_ack` + A-2
//!   `return_prior_gated` + A-5 `pto_rfc`. Tip `adaptive_transport::loss::E2aFlags` (alanları özel,
//!   kurucusu `E2aFlags::new` + `with_*`); kama-node bunun için `adaptive-transport`'a doğrudan yol
//!   bağımlılığı alır (bileşim kökü, `layer_check` C8; yeni dış paket yok; KK-Ö-38 kapandı).
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
//! Kol, bayraklar, W ve T bu dosyadaki sabitlerdir ([`GOSTER_KOL`], [`GOSTER_E1`], [`GOSTER_E2A`],
//! [`GOSTER_FILE_WINDOW_UNITS`], [`GOSTER_PLAYOUT_DELAY`]). Ölçüm düzeneğinin kol tablosu
//! (`f0_flows` / bench `--e1-arm K1A6`) aynı bayrakları yazar (f0_flows testi
//! `goster_on_writes_exactly_the_kama_node_goster_profile`). Paz 15 20:00 dondurmasından sonra
//! değişmez (sınıf M; KK-Ö-22).

use core::time::Duration;

use adaptive_transport::loss::{E2aFlags, E2aFlagsError, E2A_TT_GRANULARITY_MIN_NS};
use adaptive_transport::rate::E1Flags;
use client_core::lanes::{LaneConfig, DEFAULT_FILE_WINDOW_UNITS};
use client_core::transport::ClientTransportConfig;
use kama_app::playout::{PlayoutConfig, PLAYOUT_DELAY_DEFAULT};

/// Gösterim kolunun adı = f0_flows / bench `--e1-arm` değeri (koşu kimliği `kama-custom`). Gece
/// ölçümünün hücre kol etiketi `K1A6s` = bu kol + `--goster on` (şeritler + tampon); `s` kolun değil
/// `--goster on`'un parçasıdır, bench'te şerit olmadığından kol adına girmez.
pub const GOSTER_KOL: &str = "K1A6";

/// Gösterim kolunun E1 bayrakları (M2 + Q2 `E1Flags`; CC §4.1 adları yorumda).
pub const GOSTER_E1: E1Flags = E1Flags {
    // (c)
    e1_startup_cwnd: true,
    // (d)
    e1_applimited_plateau: true,
    // DrainRtprop
    e1_drainrt_gate: true,
    // (e-dar)
    e1_probe_keep: true,
    // retx_first KAPALI (sahip v4 EK-A §2).
    retx_first: false,
    // A-3: kova tavanı tabanı 2·MTU.
    a3_burst_floor: true,
    // A-4: ProbeUp atıflı kayıpla iptal edilmez; prob saati srtt.
    a4_probe_srtt: true,
    // A-6: Startup bw-terimi tabanı 10·MTU/srtt.
    a6_iw_floor: true,
};

/// Gösterim kolunun E2a bayrakları (M1 + Q1 `E2aFlags` alanlarının sırası). `E2aFlags`'in alanları
/// özel ve kurucusu `const` değil; değerler burada, tip [`e2a_flags`]'te kurulur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GosterE2a {
    /// m1 `tt_rfc`.
    pub tt_rfc: bool,
    /// m1 `g` (ns), [1 ms, 200 ms].
    pub tt_granularity_ns: u64,
    /// m8 `rtt_rule_v11`.
    pub rtt_rule_v11: bool,
    /// m6 `iw_idle_hold`.
    pub iw_idle_hold: bool,
    /// PTO turu başına prob sayısı, {1, 2}.
    pub pto_probes: u8,
    /// A-1 (m5) `gap_ack`.
    pub gap_ack: bool,
    /// A-2 `return_prior_gated`.
    pub return_prior_gated: bool,
    /// A-5 `pto_rfc`.
    pub pto_rfc: bool,
}

/// m1 + m8 + m6 + PTO 2 (M1 seti) + A-1 + A-2 + A-5 (Q1); g = 1 ms.
pub const GOSTER_E2A: GosterE2a = GosterE2a {
    tt_rfc: true,
    tt_granularity_ns: E2A_TT_GRANULARITY_MIN_NS,
    rtt_rule_v11: true,
    iw_idle_hold: true,
    pto_probes: 2,
    gap_ack: true,
    return_prior_gated: true,
    pto_rfc: true,
};

impl GosterE2a {
    /// Doğrulamalı tip (`E2aFlags::new` + `with_return_prior_gated` + `with_pto_rfc`).
    ///
    /// # Errors
    /// [`E2aFlagsError`] — `pto_probes ∉ {1, 2}` ya da `g ∉ [1 ms, 200 ms]`.
    pub fn flags(self) -> Result<E2aFlags, E2aFlagsError> {
        E2aFlags::new(
            self.tt_rfc,
            self.tt_granularity_ns,
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
}

/// Gösterim E2a bayrakları ([`GOSTER_E2A`]). Sabitler geçerli aralıktadır (birim testi
/// `goster_values_are_the_profile_rows`); aralık dışına çıkarılırsa kurucu tipli hata verir ve burada
/// ürün değerine (`E2aFlags::default()`, hepsi kapalı) düşülür — test bunu kırmızıyla yakalar.
#[must_use]
pub fn e2a_flags() -> E2aFlags {
    GOSTER_E2A.flags().unwrap_or_default()
}

/// W: FILE şeridinin teyitsiz birim tavanı (v4 K6; YOL3-T1 gösterim varsayılanı).
pub const GOSTER_FILE_WINDOW_UNITS: u32 = DEFAULT_FILE_WINDOW_UNITS;

/// T: PTT oynatma tamponu (v4 III.1 "ölçüm yoksa 2,0 s"; mini-Ö okumasıyla kesinleşir).
pub const GOSTER_PLAYOUT_DELAY: Duration = PLAYOUT_DELAY_DEFAULT;

/// Gösterim profilini Transport yapılandırmasına yazar (yalnız modül belgesindeki alanlar).
pub fn apply(cfg: &mut ClientTransportConfig) {
    cfg.runtime.fec = None;
    cfg.runtime.e1_flags = GOSTER_E1;
    cfg.runtime.e2a_flags = e2a_flags();
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

    /// Profil satırları: FEC yok, gösterim E1 seti (K1 + A-3 + A-4 + A-6), gösterim E2a seti (M1 +
    /// A-1 + A-2 + A-5), şeritler W = 5, T = 2,0 s.
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
        // E1: alan alan (sahip 2026-11-10 madde 3; gece K1A6s).
        let e1 = cfg.runtime.e1_flags;
        assert!(e1.e1_startup_cwnd && e1.e1_applimited_plateau && e1.e1_drainrt_gate);
        assert!(
            e1.e1_probe_keep && !e1.retx_first,
            "K1 tabanı: retx_first kapalı"
        );
        assert!(
            e1.a3_burst_floor && e1.a4_probe_srtt && e1.a6_iw_floor,
            "A-3, A-4, A-6"
        );
        assert_eq!(
            product.runtime.e1_flags,
            E1Flags::default(),
            "ürün E1 kapalı"
        );
        // E2a: alan alan; kurucu sabitleri kabul etmeli (ürün değerine düşmemeli).
        assert!(GOSTER_E2A.flags().is_ok(), "E2a sabitleri geçerli aralıkta");
        let f = cfg.runtime.e2a_flags;
        assert!(
            f.tt_rfc() && f.rtt_rule_v11() && f.iw_idle_hold(),
            "M1: m1, m8, m6"
        );
        assert_eq!(
            (f.tt_granularity_ns(), f.pto_probes()),
            (1_000_000, 2),
            "g = 1 ms, PTO 2"
        );
        assert!(
            f.gap_ack() && f.return_prior_gated() && f.pto_rfc(),
            "A-1, A-2, A-5"
        );
        assert_eq!(
            product.runtime.e2a_flags,
            E2aFlags::default(),
            "ürün E2a kapalı"
        );
        let lanes = cfg.lanes.expect("şeritler açık");
        assert_eq!(
            lanes,
            LaneConfig::default(),
            "W = 5 = LaneConfig::default()"
        );
        assert_eq!(lanes.file_window_units, 5);
        assert_eq!(GOSTER_KOL, "K1A6");
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
        cfg.runtime.e2a_flags = product.runtime.e2a_flags;
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
