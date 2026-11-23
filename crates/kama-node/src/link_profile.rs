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

use std::io::{self, Read};
use std::path::Path;

use client_core::runtime::{
    link_feedback_starved, LinkRates, RuntimeConfig, PTT_FORWARD_COPIES, PTT_FORWARD_COPIES_MIN,
};
use client_core::TransportEngine;

/// Ortam değişkeni adı.
pub(crate) const ENV_VAR: &str = "KAMA_LINK_RATES";
/// Veri dizinindeki profil dosyasının adı.
pub(crate) const PROFILE_FILE: &str = "link-profile";
/// Veri dizinindeki başlangıç kaydının adı.
pub(crate) const START_RECORD_FILE: &str = "transport-start.txt";
/// [SR-AUDIT-001] Profil değerinin (ortam ya da dosya) üst sınırı (B). Biçim `<u64>,<u64>` en çok
/// 41 B'dir; boşluk ve satır sonu payıyla 64. Dosyadan en çok bu + 1 bayt okunur, fazlası hata.
pub(crate) const MAX_PROFILE_BYTES: usize = 64;

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
        // Uzunluk ayrıştırmadan ÖNCE.
        if value.len() > MAX_PROFILE_BYTES {
            return Err(LoadError::Invalid);
        }
        return parse(value)
            .map(|rates| (Some(rates), Source::Env))
            .ok_or(LoadError::Invalid);
    }
    let file = match std::fs::File::open(data_dir.join(PROFILE_FILE)) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((None, Source::None)),
        Err(e) => return Err(LoadError::Io(e)),
    };
    // Sınır + 1 bayt: dosyanın tamamı okunmaz; fazlası varsa yapılandırma hatası.
    let mut bytes = Vec::with_capacity(MAX_PROFILE_BYTES + 1);
    file.take(MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(LoadError::Io)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err(LoadError::Invalid);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| LoadError::Invalid)?;
    parse(text)
        .map(|rates| (Some(rates), Source::File))
        .ok_or(LoadError::Invalid)
}

/// Yerel yukarı hat dar mı: karşı uç bu cihazın yukarı hattından gelen ACK'lerle geri bildirimi
/// aç kalır. Aynı kural (`runtime::link_feedback_starved`) yön hızları ters çevrilmiş
/// yapılandırmaya uygulanır; ayrı bir eşik yoktur.
fn local_uplink_narrow(runtime: &RuntimeConfig) -> bool {
    let Some(l) = runtime.link_rates else {
        return false;
    };
    let mirrored = RuntimeConfig {
        link_rates: Some(LinkRates {
            uplink_bps: l.downlink_bps,
            downlink_bps: l.uplink_bps,
        }),
        ..runtime.clone()
    };
    link_feedback_starved(&mirrored)
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
                "geri-bildirim-aç (PTT: 1 asıl gönderim + teyitsiz birim için en çok {} planlı ek kopya, hat beyanın altında ölçülürse orantılı az, en az {}; PTT dışı şeritlerde ACK'siz FEC ön oranı; ACK saatsiz uçuş)",
                PTT_FORWARD_COPIES.saturating_sub(1),
                PTT_FORWARD_COPIES_MIN.saturating_sub(1)
            )
        } else {
            "geri-bildirim-aç, şeritsiz (ACK'siz FEC ön oranı, ACK saatsiz uçuş; PTT kopyası YOK)"
                .to_string()
        }
    } else {
        "standart".to_string()
    };
    // Saha cihazı (yerel yukarı hat dar): karşı ucun korumasını YEREL kayıt hükme bağlamaz.
    let uplink = if local_uplink_narrow(runtime) {
        "yerel_yukarı_hat=dar — Yerel yukarı hat dar; karşı uç koruması karşı cihazın başlangıç kaydından doğrulanır.\n"
    } else {
        ""
    };
    // [ACK-BİÇİMİ] Kabul kipi başlangıç kaydından okunur (V2 = ürün varsayılanı; bitmap = pilot).
    // Bitmap yalnız geri bildirimi aç profilde etkindir (`runtime.rs`: CAPS ancak
    // `link_feedback_starved` iken çıkar): ayarlanan ile etkin olan ayrı yazılır. Telde gerçekten
    // koştuğunun kanıtı koşu sonundaki sayaçtır (`ClientStats::ack_bitmap_rx`), bu kayıt değil.
    let ack = match (runtime.ack_format, link_feedback_starved(runtime)) {
        (client_core::runtime::AckFormat::V2, _) => "v2",
        (client_core::runtime::AckFormat::Bitmap, true) => "bitmap etkin=evet",
        (client_core::runtime::AckFormat::Bitmap, false) => {
            "bitmap etkin=hayır (geri bildirim aç profil yok: telde v2)"
        }
    };
    format!(
        "motor={engine}\nack_biçimi={ack}\nşeritler={}\nyön_hızları={rates}\nkaynak={}\nkoruma={protection}\n{uplink}",
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
        assert!(
            plain.contains("ack_biçimi=v2\n"),
            "ürün varsayılanı V2: {plain}"
        );
        let bitmap = start_record(
            &RuntimeConfig {
                ack_format: client_core::runtime::AckFormat::Bitmap,
                ..RuntimeConfig::default()
            },
            true,
            Source::None,
        );
        assert!(
            bitmap.contains("ack_biçimi=bitmap etkin=hayır"),
            "yön hızı yokken bitmap ayarlı ama etkin değil: {bitmap}"
        );
        let active = start_record(
            &RuntimeConfig {
                ack_format: client_core::runtime::AckFormat::Bitmap,
                link_rates: Some(LinkRates {
                    uplink_bps: 12_500,
                    downlink_bps: 62,
                }),
                ..RuntimeConfig::default()
            },
            true,
            Source::Env,
        );
        assert!(
            active.contains("ack_biçimi=bitmap etkin=evet\n"),
            "{active}"
        );
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
        assert!(
            field.contains(
                "yerel_yukarı_hat=dar — Yerel yukarı hat dar; karşı uç koruması karşı cihazın başlangıç kaydından doğrulanır.\n"
            ),
            "{field}"
        );
        assert!(!plain.contains("yerel_yukarı_hat"), "profilsiz: {plain}");
        // Baz: kendi verisinin ACK'leri 62 B/s'lik yoldan gelir → geri bildirim aç.
        runtime.link_rates = Some(LinkRates {
            uplink_bps: 12_500,
            downlink_bps: 62,
        });
        let base = start_record(&runtime, true, Source::Env);
        assert!(
            base.contains(
                "koruma=geri-bildirim-aç (PTT: 1 asıl gönderim + teyitsiz birim için en çok 6 planlı ek kopya, hat beyanın altında ölçülürse orantılı az, en az 5;"
            ),
            "{base}"
        );
        assert!(
            !base.contains("yerel_yukarı_hat"),
            "baz: yerel yukarı hat geniş: {base}"
        );
        assert!(base.contains("kaynak=KAMA_LINK_RATES\n"), "{base}");
        let no_lanes = start_record(&runtime, false, Source::Env);
        assert!(no_lanes.contains("PTT kopyası YOK"), "{no_lanes}");
        runtime.engine = TransportEngine::Legacy;
        let legacy = start_record(&runtime, true, Source::Env);
        assert!(legacy.contains("motor=legacy\n") && legacy.contains("koruma=standart\n"));
    }

    /// [SR-AUDIT-001] Dosyadan en çok sınır + 1 bayt okunur; sınırı aşan dosya ve ortam değeri
    /// ayrıştırılmadan hata verir. Sınırdaki değer kabul edilir.
    #[test]
    fn profile_input_is_bounded_before_parsing() {
        let dir = TempDir::new("link-profile-bound");
        let at_limit = format!("{:<width$}", "62,12500", width = MAX_PROFILE_BYTES);
        assert_eq!(at_limit.len(), MAX_PROFILE_BYTES);
        std::fs::write(dir.path().join(PROFILE_FILE), &at_limit).expect("sınırda dosya");
        assert!(matches!(
            load(dir.path(), None),
            Ok((Some(_), Source::File))
        ));
        let over = format!("{at_limit} ");
        std::fs::write(dir.path().join(PROFILE_FILE), &over).expect("sınırı aşan dosya");
        assert!(matches!(load(dir.path(), None), Err(LoadError::Invalid)));
        // Büyük dosya: okunan bayt sınır + 1 ile kısıtlı, sonuç yine hata.
        std::fs::write(dir.path().join(PROFILE_FILE), vec![b' '; 1 << 20]).expect("büyük dosya");
        assert!(matches!(load(dir.path(), None), Err(LoadError::Invalid)));
        assert!(matches!(
            load(dir.path(), Some(&over)),
            Err(LoadError::Invalid)
        ));
        assert!(matches!(
            load(dir.path(), Some(&at_limit)),
            Ok((Some(_), Source::Env))
        ));
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
