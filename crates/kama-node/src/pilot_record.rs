//! [TELEFON-PİLOTU · İŞ-30] Pilot kaydı: açıkça etkinleştirilen, düğümü yavaşlatmayan periyodik
//! taşıma özeti (`<data_dir>/pilot-summary.txt`).
//!
//! Etkinleştirme (ikisinden biri; yoksa KAPALI ve hiçbir şey yazılmaz, iş parçacığı açılmaz):
//! 1. `KAMA_PILOT_RECORD=1` ortam değişkeni (`0` açıkça kapalı; başka değer yapılandırma hatasıdır);
//! 2. `<data_dir>/pilot-record` işaret dosyası (içeriği okunmaz).
//!
//! Açıkken Transport'un canlı teşhis aynası açılır (`RuntimeConfig::live_diag`, salt gözlem: motor
//! kararlarını değiştirmez) ve özet ondan okunur.
//!
//! Zamanlama: özet METNİ düğüm döngüsünün ölçü turunda en çok [`SUMMARY_PERIOD`]'da bir üretilir;
//! disk yazımı ayrı bir yazıcı iş parçacığındadır. Aradaki kanalın kapasitesi 1'dir: yazıcı bir
//! öncekini bitirmediyse yeni özet ATLANIR (sayılır, kuyruk birikmez). Dosya geçici dosya + yeniden
//! adlandırma ile atomik değişir. Kapanışta son özet bir kez, beklenerek gönderilir.
//!
//! Sayaç kapsamı (`node.rs` özeti): her cihaz kendi GÖNDERDİĞİ baytları yazar; iki cihazın TX
//! kayıtları birlikte iki yönü kapsar. Her sayacın yanında ölçtüğü katman yazılır; canlı kaynakta
//! olmayan sayaç "sayaç yok" diye yazılır (uydurulmaz). Örtüşen kategoriler toplanmaz.
//! Anahtar, kimlik ya da yük içeriği YAZILMAZ.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Ortam değişkeni adı.
pub(crate) const ENV_VAR: &str = "KAMA_PILOT_RECORD";
/// Veri dizinindeki işaret dosyasının adı.
pub(crate) const MARKER_FILE: &str = "pilot-record";
/// Özet dosyasının adı.
pub(crate) const SUMMARY_FILE: &str = "pilot-summary.txt";
/// İki özet arasındaki en kısa süre.
pub(crate) const SUMMARY_PERIOD: Duration = Duration::from_secs(5);

/// Etkinleştirme girdisi geçersiz.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InvalidSwitch;

/// Pilot kaydı açık mı (`env` = `KAMA_PILOT_RECORD`'un değeri, varsa).
pub(crate) fn enabled(data_dir: &Path, env: Option<&str>) -> Result<bool, InvalidSwitch> {
    match env.map(str::trim) {
        Some("1") => Ok(true),
        Some("0") => Ok(false),
        Some(_) => Err(InvalidSwitch),
        None => Ok(data_dir.join(MARKER_FILE).is_file()),
    }
}

/// Özeti hazırlayan uç (düğüm döngüsü) ile yazıcı iş parçacığı.
pub(crate) struct PilotRecorder {
    tx: Option<SyncSender<String>>,
    writer: Option<JoinHandle<()>>,
    started: Instant,
    last: Option<Instant>,
    seq: u64,
    skipped: u64,
}

impl PilotRecorder {
    /// Yazıcıyı başlatır. Özet `<data_dir>/pilot-summary.txt`'ye yazılır.
    pub(crate) fn start(data_dir: &Path) -> io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<String>(1);
        let target = data_dir.join(SUMMARY_FILE);
        let writer = thread::Builder::new()
            .name("kama-pilot-record".to_owned())
            .spawn(move || write_loop(&rx, &target))?;
        Ok(Self {
            tx: Some(tx),
            writer: Some(writer),
            started: Instant::now(),
            last: None,
            seq: 0,
            skipped: 0,
        })
    }

    /// Kayıt başladığından beri geçen süre.
    pub(crate) fn uptime(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started)
    }

    /// Şimdiye dek atlanan özet sayısı.
    pub(crate) fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Süre dolduysa `summary(sıra)`'yı üretip yazıcıya bırakır (bloklamaz). Yazıcı meşgulse özet
    /// atlanır. Dönüş: özet üretildi mi.
    pub(crate) fn tick(&mut self, now: Instant, summary: impl FnOnce(u64) -> String) -> bool {
        if self
            .last
            .is_some_and(|t| now.saturating_duration_since(t) < SUMMARY_PERIOD)
        {
            return false;
        }
        self.last = Some(now);
        self.seq += 1;
        let text = summary(self.seq);
        if let Some(tx) = &self.tx {
            match tx.try_send(text) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => self.skipped += 1,
                Err(TrySendError::Disconnected(_)) => self.tx = None,
            }
        }
        true
    }

    /// Kapanış: son özet bir kez, beklenerek yazılır; yazıcı katılır.
    pub(crate) fn finish(mut self, summary: impl FnOnce(u64) -> String) {
        self.seq += 1;
        let text = summary(self.seq);
        if let Some(tx) = self.tx.take() {
            // Kapasite 1: yazıcı öncekini bitirince yer açılır; yazıcı ölmüşse hata, yok sayılır.
            let _ = tx.send(text);
        }
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}

impl Drop for PilotRecorder {
    fn drop(&mut self) {
        // `finish` çağrılmadıysa: kanal kapanır, yazıcı biter. Panik yok.
        self.tx = None;
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}

/// Yazıcı: her özeti geçici dosyaya yazıp yeniden adlandırır. Hata özeti düşürür (düğüm etkilenmez).
fn write_loop(rx: &Receiver<String>, target: &Path) {
    let tmp: PathBuf = target.with_extension("txt.tmp");
    while let Ok(text) = rx.recv() {
        if crate::files::write_private(&tmp, text.as_bytes()).is_ok() {
            let _ = std::fs::rename(&tmp, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn switch_is_explicit_and_strict() {
        let dir = TempDir::new("pilot-switch");
        assert_eq!(enabled(dir.path(), None), Ok(false), "varsayılan kapalı");
        assert_eq!(enabled(dir.path(), Some("1")), Ok(true));
        assert_eq!(enabled(dir.path(), Some("evet")), Err(InvalidSwitch));
        assert_eq!(enabled(dir.path(), Some("2")), Err(InvalidSwitch));
        std::fs::write(dir.path().join(MARKER_FILE), b"").expect("işaret");
        assert_eq!(enabled(dir.path(), None), Ok(true), "işaret dosyası açar");
        assert_eq!(
            enabled(dir.path(), Some("0")),
            Ok(false),
            "ortam '0' işareti ezer"
        );
    }

    /// Süre dolmadan ikinci özet üretilmez; kapanış özeti beklenerek yazılır ve dosyada kalır.
    #[test]
    fn summaries_are_rate_limited_and_the_final_one_is_written() {
        let dir = TempDir::new("pilot-rate");
        let mut r = PilotRecorder::start(dir.path()).expect("yazıcı");
        let t0 = Instant::now();
        assert!(r.tick(t0, |n| format!("özet {n}\n")));
        assert!(!r.tick(t0 + Duration::from_secs(1), |_| unreachable!(
            "süre dolmadı"
        )));
        assert!(r.tick(t0 + SUMMARY_PERIOD, |n| format!("özet {n}\n")));
        r.finish(|n| format!("son {n}\n"));
        let text = std::fs::read_to_string(dir.path().join(SUMMARY_FILE)).expect("özet dosyası");
        assert_eq!(text, "son 3\n");
        assert!(
            !dir.path().join("pilot-summary.txt.tmp").exists(),
            "geçici dosya kalmaz"
        );
    }

    /// Uçtan uca: gerçek kurulum gövdesi (`KamaNode::build`). İşaret dosyası olan cihazda başlangıç
    /// kaydı `pilot_kaydı=açık` der, döngünün özet turu ve kapanışı özet dosyasını beklenen
    /// satırlarla yazar. İşaretsiz cihazda aynı turlar özet dosyası OLUŞTURMAZ.
    #[test]
    fn node_build_writes_the_pilot_summary_only_when_enabled() {
        let _serial = crate::test_support::serial();
        let root = TempDir::new("pilot-node");
        let phones = crate::test_support::provision(root.path(), &["base", "field"]);
        let base = &phones["base"];
        crate::files::create_private_dir(base.data_dir()).expect("veri dizini");
        std::fs::write(base.data_dir().join(MARKER_FILE), b"").expect("işaret");
        let (mut built, _events) =
            crate::test_support::manual(base, crate::test_support::system_clock());
        let start =
            std::fs::read_to_string(base.data_dir().join(crate::link_profile::START_RECORD_FILE))
                .expect("başlangıç kaydı");
        assert!(start.contains("pilot_kaydı=açık"), "{start}");
        assert!(start.contains("çekirdek_sürümü="), "{start}");
        assert!(start.contains("çekirdek_git="), "{start}");
        built.node.pilot_tick(Instant::now());
        built.node.pilot_finish();
        let text = std::fs::read_to_string(base.data_dir().join(SUMMARY_FILE)).expect("özet");
        for line in [
            "pilot_özeti sıra=",
            "son=true",
            "tx_toplam_bayt=",
            "[mühürlü UDP gövdesi, UDP/IP zarfı hariç]",
            "tx_yeniden_iletim_bayt=",
            "tx_ileri_kopya_karar=",
            "tx_fec_parite_datagram=",
            "tx_asıl_veri_bayt=sayaç yok",
            "tx_ack_kontrol=sayaç yok",
            "rx_bayt=sayaç yok",
            "çekirdek_git=",
        ] {
            assert!(text.contains(line), "{line:?} yok:\n{text}");
        }
        drop(built);
        let field = &phones["field"];
        let (mut built, _events) =
            crate::test_support::manual(field, crate::test_support::system_clock());
        let start = std::fs::read_to_string(
            field
                .data_dir()
                .join(crate::link_profile::START_RECORD_FILE),
        )
        .expect("başlangıç kaydı");
        assert!(start.contains("pilot_kaydı=kapalı"), "{start}");
        built.node.pilot_tick(Instant::now());
        built.node.pilot_finish();
        assert!(
            !field.data_dir().join(SUMMARY_FILE).exists(),
            "kapalıyken özet dosyası oluşmaz"
        );
        drop(built);
    }

    /// Yazıcı meşgulken özet bekletilmez, atlanır ve sayılır (kuyruk birikmez); üretici bloklanmaz.
    #[test]
    fn a_busy_writer_skips_summaries_instead_of_queueing() {
        let (tx, rx) = mpsc::sync_channel::<String>(1);
        let mut r = PilotRecorder {
            tx: Some(tx),
            writer: None,
            started: Instant::now(),
            last: None,
            seq: 0,
            skipped: 0,
        };
        let t0 = Instant::now();
        for i in 0..5u32 {
            r.tick(t0 + SUMMARY_PERIOD * i, |n| format!("{n}"));
        }
        assert_eq!(
            r.skipped(),
            4,
            "yazıcı okumadı: ilki kanalda, kalan dördü atlandı"
        );
        assert_eq!(rx.try_recv().as_deref(), Ok("1"));
        assert!(rx.try_recv().is_err(), "kanalda tek özet");
    }
}
