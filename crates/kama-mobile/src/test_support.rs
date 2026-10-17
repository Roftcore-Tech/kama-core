//! Birim testlerinin gerçek düğüm girdileri (yalnız `cfg(test)`). Cihaz dosyaları
//! `kama-provision init` CLI yoluyla (`kama_provision::cli::run`, OS entropisi) geçici dizinde
//! üretilir — smoke testiyle (`tests/smoke.rs`) aynı yol, elle yazılmış fikstür DEĞİL. Otorite
//! çıktısı (`root.json` dahil) ayrı dizinde kalır; "telefon" dizinine YALNIZ `device-alice.json`
//! kopyalanır (D9). Direct-mode adresleri 127.0.0.1'de boş UDP portlarıdır.

use std::ffi::OsString;
use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kama_provision::format::ROOT_FILE_NAME;

/// Test düğümünün cihazı.
pub(crate) const ALICE: &str = "alice";
/// Provisioning dosyasındaki tek eş (bu testlerde başlatılmaz).
pub(crate) const BOB: &str = "bob";
/// Beklenen bir olayın ÜST SÜRE SINIRI (aşılırsa kırmızı); yavaş CI koşucusu içindir (smoke
/// testiyle aynı). Düğüm her saniye `Metrics` olayı verir.
pub(crate) const WAIT: Duration = Duration::from_secs(60);

/// `std::env::temp_dir()` altında bu teste özgü dizin; Drop'ta silinir (panik etmez).
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let pid = std::process::id();
        let name = format!("kama-mobile-unit-{label}-{pid}-{seq}-{nanos}");
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).expect("geçici dizin");
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Bir "telefon": yalnız kendi cihaz dosyası + veri dizini (Kotlin'in `noBackupFilesDir`'i).
pub(crate) struct Phone {
    pub(crate) config: String,
    pub(crate) data: String,
}

/// Alice ve Bob'u `kama-provision init` ile üretir; telefona yalnız alice'in dosyası gider.
pub(crate) fn provision(root: &Path) -> Phone {
    let authority = root.join("authority");
    // İki boş UDP portu: soketler aynı anda açık tutulur ki aynı port iki kez verilmesin.
    let sockets = [bind_any(), bind_any()];
    let [alice_port, bob_port] = sockets.map(|socket| port(&socket));
    let direct = format!("{ALICE}=127.0.0.1:{alice_port},{BOB}=127.0.0.1:{bob_port}");
    let devices = format!("{ALICE},{BOB}");
    let out = utf8(&authority);
    let argv = [
        "kama-provision",
        "init",
        "--out",
        out.as_str(),
        "--devices",
        devices.as_str(),
        "--direct",
        direct.as_str(),
    ];
    let _ = kama_provision::cli::run(argv.map(OsString::from));
    let source = authority.join(format!("device-{ALICE}.json"));
    assert!(source.is_file(), "init cihaz dosyasını yazmadı");
    let config_dir = root.join(ALICE).join("config");
    fs::create_dir_all(&config_dir).expect("telefon dizini");
    let config = config_dir.join(format!("device-{ALICE}.json"));
    fs::copy(&source, &config).expect("cihaz dosyası");
    assert!(
        !config_dir.join(ROOT_FILE_NAME).exists(),
        "D9: root.json cihaz dizinine girmez"
    );
    Phone {
        config: utf8(&config),
        data: utf8(&root.join(ALICE).join("data")),
    }
}

fn bind_any() -> UdpSocket {
    UdpSocket::bind("127.0.0.1:0").expect("UDP soketi")
}

fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().expect("adres").port()
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("geçici yol UTF-8").to_owned()
}
