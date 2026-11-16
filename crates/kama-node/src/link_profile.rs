//! [TELEFON-PİLOTU · 2026-11-16] Yön hızı profili (asimetrik hat) dağıtım ayarından okunur ve
//! başlangıçta gerçekten seçilen taşıma yapılandırması veri dizinine yazılır.
//!
//! Neden: ürün profilinde (`ClientTransportConfig::product`) `RuntimeConfig::link_rates` daima
//! `None`'dı ve düğümün onu verecek bir girdisi yoktu. Bu yüzden asimetrik hat korumaları
//! (`client_core::runtime::link_feedback_starved`: PTT ileri kopyası, PTT dışı şeritlerde ACK'siz
//! FEC ön oranı, ACK saatsiz uçuş) telefonda hiç devreye giremezdi.
//!
//! Kaynak sırası:
//! 1. `KAMA_LINK_RATES` ortam değişkeni;
//! 2. yoksa `<data_dir>/link-profile` dosyası;
//! 3. yoksa profil yok (`None`, ürün varsayılanı; davranış bugünküyle aynı).
//!
//! Biçim: `<yukarı_B/s>,<aşağı_B/s>` — iki pozitif tamsayı. Boşluk ve sondaki satır sonu yok
//! sayılır. Yukarı = bu düğümün gönderdiği yön (kendi verisi ve gönderdiği ACK'ler); aşağı = bu
//! düğümün aldığı yön. Örnek: dar yukarı yönlü saha cihazı `62,12500`, karşısındaki baz
//! `12500,62` (bazın kendi verisinin ACK'leri 62 B/s'lik yoldan gelir; koruma bazda açılır).
//! Geçersiz değer yapılandırma hatasıdır; düğüm sessizce profilsiz başlamaz.
//!
//! Her başlangıçta `<data_dir>/transport-start.txt` (0600) yazılır. İçinde: motor, şeritler, yön
//! hızları, kaynağı ve koruma kipi. Anahtar ya da kimlik bilgisi YOK.
//!
//! Mobil FFI, cihaz dosyası, `KamaNode::start` imzası ve tel düzeni değişmez.

use std::io;
use std::path::Path;

use client_core::runtime::{link_feedback_starved, LinkRates, RuntimeConfig, PTT_FORWARD_COPIES};
use client_core::TransportEngine;

/// Ortam değişkeni adı.
pub(crate) const ENV_VAR: &str = "KAMA_LINK_RATES";
/// Veri dizinindeki profil dosyasının adı.
pub(crate) const PROFILE_FILE: &str = "link-profile";
/// Veri dizinindeki başlangıç kaydının adı.
pub(crate) const START_RECORD_FILE: &str = "transport-start.txt";

/// Profilin nereden geldiği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// `KAMA_LINK_RATES`.
    Env,
    /// `<data_dir>/link-profile`.
    File,
    /// Profil yok.
    None,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::Env => ENV_VAR,
            Self::File => PROFILE_FILE,
            Self::None => "yok",
        }
    }
}

/// Okuma hatası.
#[derive(Debug)]
pub(crate) enum LoadError {
    /// Değer biçime uymuyor.
    Invalid,
    /// Dosya var ama okunamadı.
    Io(io::Error),
}

/// `<yukarı>,<aşağı>` → [`LinkRates`]. İki değer de > 0 olmalı.
pub(crate) fn parse(text: &str) -> Option<LinkRates> {
    let (up, down) = text.trim().split_once(',')?;
    let uplink_bps: u64 = up.trim().parse().ok()?;
    let downlink_bps: u64 = down.trim().parse().ok()?;
    (uplink_bps > 0 && downlink_bps > 0).then_some(LinkRates {
        uplink_bps,
        downlink_bps,
    })
}

/// Profili kaynak sırasıyla okur (`env` = `KAMA_LINK_RATES`'in değeri, varsa).
pub(crate) fn load(
    data_dir: &Path,
    env: Option<&str>,
) -> Result<(Option<LinkRates>, Source), LoadError> {
    if let Some(value) = env {
        return parse(value)
            .map(|rates| (Some(rates), Source::Env))
            .ok_or(LoadError::Invalid);
    }
    match std::fs::read_to_string(data_dir.join(PROFILE_FILE)) {
        Ok(text) => parse(&text)
            .map(|rates| (Some(rates), Source::File))
            .ok_or(LoadError::Invalid),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((None, Source::None)),
        Err(e) => Err(LoadError::Io(e)),
    }
}

/// Başlangıç kaydının metni. `lanes` = şerit katmanı açık mı (`ClientTransportConfig::lanes`).
pub(crate) fn start_record(runtime: &RuntimeConfig, lanes: bool, source: Source) -> String {
    let engine = match runtime.engine {
        TransportEngine::CcPort => "cc-port",
        TransportEngine::Legacy => "legacy",
    };
    let rates = runtime.link_rates.map_or_else(
        || "yok".to_string(),
        |l| format!("yukarı {} B/s, aşağı {} B/s", l.uplink_bps, l.downlink_bps),
    );
    // Koruma kipi runtime'ın kendi yüklemiyle hesaplanır (aynı kural, ayrı kopya yok). PTT kopyası
    // ayrıca şerit kimliği ister (`runtime.rs`: `cfg.lane_ids && link_feedback_starved(&cfg)`).
    let protection = if link_feedback_starved(runtime) {
        if lanes {
            format!(
                "geri-bildirim-aç (PTT {PTT_FORWARD_COPIES}× ileri kopya, PTT dışı şeritlerde ACK'siz FEC ön oranı, ACK saatsiz uçuş)"
            )
        } else {
            "geri-bildirim-aç, şeritsiz (ACK'siz FEC ön oranı, ACK saatsiz uçuş; PTT kopyası YOK)"
                .to_string()
        }
    } else {
        "standart".to_string()
    };
    format!(
        "motor={engine}\nşeritler={}\nyön_hızları={rates}\nkaynak={}\nkoruma={protection}\n",
        if lanes { "açık" } else { "kapalı" },
        source.label(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn parse_accepts_two_positive_rates_and_rejects_the_rest() {
        assert_eq!(
            parse(" 62 , 12500\n"),
            Some(LinkRates {
                uplink_bps: 62,
                downlink_bps: 12_500
            })
        );
        for bad in [
            "",
            "62",
            "62,",
            ",62",
            "0,12500",
            "62,0",
            "a,b",
            "62,12500,1",
            "-1,5",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn env_wins_over_file_and_no_input_means_no_profile() {
        let dir = TempDir::new("link-profile");
        assert!(matches!(load(dir.path(), None), Ok((None, Source::None))));
        std::fs::write(dir.path().join(PROFILE_FILE), "62,12500\n").expect("profil dosyası");
        let (file, src) = load(dir.path(), None).expect("dosyadan");
        assert_eq!((file.map(|l| l.uplink_bps), src), (Some(62), Source::File));
        let (env, src) = load(dir.path(), Some("12500,62")).expect("ortamdan");
        assert_eq!(
            (env.map(|l| l.uplink_bps), src),
            (Some(12_500), Source::Env)
        );
        assert!(matches!(
            load(dir.path(), Some("bozuk")),
            Err(LoadError::Invalid)
        ));
        std::fs::write(dir.path().join(PROFILE_FILE), "bozuk").expect("bozuk dosya");
        assert!(matches!(load(dir.path(), None), Err(LoadError::Invalid)));
    }

    #[test]
    fn start_record_names_engine_lanes_rates_and_protection() {
        let mut runtime = RuntimeConfig::default();
        let plain = start_record(&runtime, true, Source::None);
        assert!(plain.contains("motor=cc-port\n"), "{plain}");
        assert!(plain.contains("yön_hızları=yok\n"), "{plain}");
        assert!(plain.contains("koruma=standart\n"), "{plain}");
        // Saha cihazı: aşağı yön (ACK'lerin geldiği yön) geniş, yukarı dar → kendi verisinin
        // ACK'leri hızlı gelir: geri bildirim aç DEĞİL.
        runtime.link_rates = Some(LinkRates {
            uplink_bps: 62,
            downlink_bps: 12_500,
        });
        let field = start_record(&runtime, true, Source::File);
        assert!(field.contains("kaynak=link-profile\n"), "{field}");
        assert!(field.contains("koruma=standart\n"), "{field}");
        // Baz: kendi verisinin ACK'leri 62 B/s'lik yoldan gelir → geri bildirim aç.
        runtime.link_rates = Some(LinkRates {
            uplink_bps: 12_500,
            downlink_bps: 62,
        });
        let base = start_record(&runtime, true, Source::Env);
        assert!(
            base.contains("koruma=geri-bildirim-aç (PTT 5× ileri kopya"),
            "{base}"
        );
        assert!(base.contains("kaynak=KAMA_LINK_RATES\n"), "{base}");
        let no_lanes = start_record(&runtime, false, Source::Env);
        assert!(no_lanes.contains("PTT kopyası YOK"), "{no_lanes}");
        runtime.engine = TransportEngine::Legacy;
        let legacy = start_record(&runtime, true, Source::Env);
        assert!(legacy.contains("motor=legacy\n") && legacy.contains("koruma=standart\n"));
    }

    /// Uçtan uca: gerçek kurulum gövdesi (`KamaNode::build`) profil dosyasını `RuntimeConfig`'e
    /// uygular ve başlangıç kaydını yazar; profilsiz cihazda kayıt "yok / standart" der.
    #[test]
    fn node_build_applies_the_profile_file_and_writes_the_start_record() {
        let _serial = crate::test_support::serial();
        let root = TempDir::new("link-profile-node");
        let phones = crate::test_support::provision(root.path(), &["base", "field"]);
        let base = &phones["base"];
        crate::files::create_private_dir(base.data_dir()).expect("veri dizini");
        std::fs::write(base.data_dir().join(PROFILE_FILE), "12500,62\n").expect("profil");
        let (built, _events) =
            crate::test_support::manual(base, crate::test_support::system_clock());
        let record =
            std::fs::read_to_string(base.data_dir().join(START_RECORD_FILE)).expect("kayıt");
        assert!(
            record.contains("yön_hızları=yukarı 12500 B/s, aşağı 62 B/s\n"),
            "{record}"
        );
        assert!(record.contains("kaynak=link-profile\n"), "{record}");
        assert!(record.contains("şeritler=açık\n"), "{record}");
        assert!(record.contains("koruma=geri-bildirim-aç (PTT"), "{record}");
        drop(built);
        let field = &phones["field"];
        let (built, _events) =
            crate::test_support::manual(field, crate::test_support::system_clock());
        let record =
            std::fs::read_to_string(field.data_dir().join(START_RECORD_FILE)).expect("kayıt");
        assert!(record.contains("yön_hızları=yok\n"), "{record}");
        assert!(record.contains("koruma=standart\n"), "{record}");
        drop(built);
    }
}
