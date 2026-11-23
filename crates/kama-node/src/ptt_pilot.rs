//! [PTT-BİRLEŞİK · pilot · #233, sahip kararı 2026-11-23] Birleşik PTT pilotunun dağıtım ayarı.
//!
//! İsteğe bağlıdır ve ürün varsayılanı KAPALIDIR: ayar yokken düğümün taşıma yapılandırması, PTT
//! motoru ve teli bugünküyle aynıdır (ACK biçimi V2, ayrı güvenilir `PTT_OPEN` / `PTT_CLOSE`).
//!
//! Kaynak sırası (`link_profile` ile aynı kalıp):
//! 1. `KAMA_PTT_PILOT` ortam değişkeni;
//! 2. yoksa `<data_dir>/ptt-pilot` dosyası;
//! 3. yoksa pilot kapalı.
//!
//! Biçim: `0` (kapalı) · `1` (açık; yönetilen eş yok, yetenek yalnız bant içi 0x17 ile öğrenilir)
//! · `1:<eş>[,<eş>…]` (açık; sayılan eşlerin desteği dağıtımca DOĞRULANMIŞTIR — yönetilen
//! yetenek). `<eş>` cihaz dosyasındaki eş adıdır; dizinde olmayan ad yapılandırma hatasıdır
//! (düğüm sessizce pilotsuz başlamaz). Boşluk ve sondaki satır sonu yok sayılır.
//!
//! Açıkken düğüm (`KamaNode::build`):
//! * Transport'ta `RuntimeConfig::ptt_coalesce` bayrağını açar (CAPS bit 2, ACK türü 0x17) ve ACK
//!   biçimini bitmap yapar — pilotun kabul kipi cc-port + bitmap + doğrulanmış 300 ms PTT'dir
//!   (yalın PTT çerçevesi 0x09 ve 0x17 yalnız bitmap ACK ile vardır; yetenek anlaşması
//!   olmadığından İKİ uç da pilotla başlatılmalıdır). `KAMA_ACK_FORMAT=bitmap` ayrıca verilebilir;
//!   açıkça `v2` verilmesi pilotla çelişir ve yapılandırma hatasıdır. CAPS / 0x17 yalnız geri
//!   bildirimi aç yön hızı profilinde çıkar (`KAMA_LINK_RATES`; `runtime::link_feedback_starved`):
//!   profil yokken yalnız yönetilen yetenek işler. V2 ürün yolu pilot kapalıyken aynen kalır;
//! * PTT motoruna yönetilen eşleri verir (`Ptt::set_peer_spurt_provisioned`);
//! * her turda Transport'un bant içi bildirimini (`ClientTransport::peer_ptt_coalesce`) oturumuyla
//!   motora iletir (`Ptt::on_peer_spurt_capability`): 0x17 SONRAKİ basışlar için kaydedilir.
//!
//! Basışın biçimi açılışta seçilir ve basış boyunca değişmez: desteği basıştan ÖNCE doğrulanmamış
//! eşe o basış bugünkü uyumlu biçimle gider (iyimser başlangıç yok).

use std::io::{self, Read};
use std::path::Path;

/// Ortam değişkeni adı.
pub(crate) const ENV_VAR: &str = "KAMA_PTT_PILOT";
/// Veri dizinindeki ayar dosyasının adı.
pub(crate) const PILOT_FILE: &str = "ptt-pilot";
/// [SR-AUDIT-001] Ayar değerinin üst sınırı (B): 63 eş adı (kama-provision tavanı) × 64 B pay.
/// Dosyadan en çok bu + 1 bayt okunur, fazlası hata.
pub(crate) const MAX_PILOT_BYTES: usize = 4096;

/// Pilot ayarı.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PttPilot {
    /// Pilot açık mı.
    pub(crate) enabled: bool,
    /// Desteği dağıtımca doğrulanmış eşlerin adları (cihaz dosyasındaki adlar).
    pub(crate) provisioned: Vec<String>,
}

/// Okuma hatası.
#[derive(Debug)]
pub(crate) enum LoadError {
    /// Değer biçime uymuyor.
    Invalid,
    /// Dosya var ama okunamadı.
    Io(io::Error),
}

/// `0` | `1` | `1:<eş>[,<eş>…]` → [`PttPilot`]. Boş ad ya da yinelenen ad geçersizdir.
pub(crate) fn parse(text: &str) -> Option<PttPilot> {
    let text = text.trim();
    match text {
        "0" => return Some(PttPilot::default()),
        "1" => {
            return Some(PttPilot {
                enabled: true,
                provisioned: Vec::new(),
            })
        }
        _ => {}
    }
    let names = text.strip_prefix("1:")?;
    let mut provisioned: Vec<String> = Vec::new();
    for name in names.split(',') {
        let name = name.trim();
        if name.is_empty() || provisioned.iter().any(|n| n == name) {
            return None;
        }
        provisioned.push(name.to_owned());
    }
    Some(PttPilot {
        enabled: true,
        provisioned,
    })
}

/// Ayarı kaynak sırasıyla okur (`env` = `KAMA_PTT_PILOT`'un değeri, varsa).
pub(crate) fn load(data_dir: &Path, env: Option<&str>) -> Result<PttPilot, LoadError> {
    if let Some(value) = env {
        // Uzunluk ayrıştırmadan ÖNCE.
        if value.len() > MAX_PILOT_BYTES {
            return Err(LoadError::Invalid);
        }
        return parse(value).ok_or(LoadError::Invalid);
    }
    let file = match std::fs::File::open(data_dir.join(PILOT_FILE)) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(PttPilot::default()),
        Err(e) => return Err(LoadError::Io(e)),
    };
    let mut bytes = Vec::with_capacity(MAX_PILOT_BYTES + 1);
    file.take(MAX_PILOT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(LoadError::Io)?;
    if bytes.len() > MAX_PILOT_BYTES {
        return Err(LoadError::Invalid);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| LoadError::Invalid)?;
    parse(text).ok_or(LoadError::Invalid)
}

/// Başlangıç kaydının (`transport-start.txt`) pilot satırı.
pub(crate) fn start_line(pilot: &PttPilot) -> String {
    if pilot.enabled {
        format!(
            "ptt_pilotu=açık (birleşik PTT, bitmap ACK; yönetilen eş: {})\n",
            if pilot.provisioned.is_empty() {
                "yok".to_owned()
            } else {
                pilot.provisioned.join(",")
            }
        )
    } else {
        "ptt_pilotu=kapalı\n".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_the_three_forms_and_rejects_the_rest() {
        assert_eq!(parse("0\n"), Some(PttPilot::default()));
        assert_eq!(
            parse(" 1 "),
            Some(PttPilot {
                enabled: true,
                provisioned: vec![]
            })
        );
        assert_eq!(
            parse("1:bob, carol\n"),
            Some(PttPilot {
                enabled: true,
                provisioned: vec!["bob".to_owned(), "carol".to_owned()]
            })
        );
        for bad in ["", "2", "on", "1:", "1:bob,,carol", "1:bob,bob", "0:bob"] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn load_prefers_env_then_file_then_off_and_bounds_the_value() {
        let dir = std::env::temp_dir().join(format!("kama-ptt-pilot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dizin");
        assert_eq!(load(&dir, None).expect("yok"), PttPilot::default());
        std::fs::write(dir.join(PILOT_FILE), "1:bob\n").expect("yaz");
        assert_eq!(
            load(&dir, None).expect("dosya").provisioned,
            vec!["bob".to_owned()]
        );
        assert_eq!(load(&dir, Some("0")).expect("ortam"), PttPilot::default());
        assert!(matches!(load(&dir, Some("x")), Err(LoadError::Invalid)));
        let long = format!("1:{}", "a".repeat(MAX_PILOT_BYTES));
        assert!(matches!(load(&dir, Some(&long)), Err(LoadError::Invalid)));
        std::fs::write(dir.join(PILOT_FILE), long).expect("yaz");
        assert!(matches!(load(&dir, None), Err(LoadError::Invalid)));
        std::fs::remove_dir_all(&dir).expect("sil");
    }
}
