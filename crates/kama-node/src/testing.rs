//! Düğüm testlerinin PAYLAŞILAN yardımcıları (`test-util` özelliği; ürün grafiğine girmez).
//!
//! `kama-node`'un kendi entegrasyon testleri (`tests/common`) ve INT-04 (`kama-integration`) aynı
//! kurulumu kullanır: cihaz dosyaları `kama-provision` KÜTÜPHANESİYLE geçici dizinde üretilir
//! (elle yazılmış fikstür DEĞİL), otorite çıktısı ayrı dizinde kalır, her "telefon"un dizinine
//! YALNIZ kendi `device-<ad>.json`'u kopyalanır (D9: `root.json` cihaz dizinine girmez).
//!
//! ENTROPİ BURADA DEĞİL (layer_check C8): bileşim kökünün ürün kaynağında (`src/**/*.rs`) Crypto
//! katmanının adı yalnız kurma modülünde (`src/wiring.rs`) geçebilir — `EntropySource` ve
//! `OsEntropy` o katmanın adlarıdır. Bu modül provisioning İSTEĞİNİ kurar ([`init_request`]) ve
//! üretilmiş otorite çıktısını telefonlara dağıtır ([`devices_from`]); `provision::init` çağrısını
//! ÇAĞIRAN yapar (testler C8 kapsamı dışındadır). Kapsam dışı bırakılan tek adım budur.

use std::collections::BTreeMap;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kama_contracts::handles::PeerRef;
use kama_provision::format::{load_device_config, DeviceName, ROOT_FILE_NAME};
use kama_provision::provision::InitRequest;

use crate::{NodeEvent, NodeListener};

/// Beklenen bir olayın ÜST SÜRE SINIRI (aşılırsa kırmızı; sessiz geçiş yok). Loopback'te olaylar
/// milisaniyeler içinde gelir; sınır yavaş CI koşucusu içindir. netem altında (INT-04) bozulma
/// profili bu sınırın altında kalmalıdır — kalmıyorsa test kırmızıdır, sessiz geçmez.
pub const WAIT: Duration = Duration::from_secs(60);

/// `std::env::temp_dir()` altında bu teste özgü dizin; Drop'ta silinir (panik etmez).
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("kama-node-{label}-{pid}-{seq}-{nanos}"));
        fs::create_dir(&path).expect("geçici dizin");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Bir "telefon": yalnız kendi cihaz dosyası + veri dizini.
pub struct Device {
    /// `<telefon>/config/device-<ad>.json`.
    pub config: PathBuf,
    /// `<telefon>/data` (düğüm oluşturur).
    pub data: PathBuf,
    /// Direct-mode veri adresi (verildiyse).
    pub addr: Option<SocketAddr>,
}

/// Provisioning çıktısı.
pub struct Provisioned {
    /// Otorite dizini (`kama-provision init --out`): `root.json` YALNIZ burada.
    pub authority: PathBuf,
    /// Ad → telefon.
    pub devices: BTreeMap<String, Device>,
}

/// `names` cihazları için provisioning isteği; `addrs[i]` `names[i]`'nin direct-mode adresidir
/// (eksikse o cihazın adresi yok). Çağıran bunu `kama_provision::provision::init`'e kendi entropi
/// kaynağıyla verir (modül belgesi: C8), sonra [`devices_from`] ile telefonları kurar.
pub fn init_request(root: &Path, names: &[&str], addrs: &[SocketAddr]) -> InitRequest {
    let devices: Vec<DeviceName> = names
        .iter()
        .map(|name| DeviceName::new(name).expect("cihaz adı"))
        .collect();
    InitRequest {
        out_dir: root.join("authority"),
        devices: devices.clone(),
        hub: None,
        direct: devices.into_iter().zip(addrs.iter().copied()).collect(),
    }
}

/// `init` çalıştıktan SONRA: otorite çıktısından her telefonun dizinini kurar ve yalnız kendi
/// cihaz dosyasını kopyalar. `root.json`'un cihaz dizinine girmediği DENETLENİR (D9).
pub fn devices_from(root: &Path, request: &InitRequest, addrs: &[SocketAddr]) -> Provisioned {
    let authority = request.out_dir.clone();
    let mut phones = BTreeMap::new();
    for (index, name) in request.devices.iter().enumerate() {
        let phone = root.join(name.as_str());
        let config_dir = phone.join("config");
        fs::create_dir_all(&config_dir).expect("telefon dizini");
        let config = config_dir.join(name.file_name());
        fs::copy(authority.join(name.file_name()), &config).expect("cihaz dosyası");
        assert!(
            !config_dir.join(ROOT_FILE_NAME).exists(),
            "D9: root.json cihaz dizinine girmez"
        );
        let device = Device {
            config,
            data: phone.join("data"),
            addr: addrs.get(index).copied(),
        };
        phones.insert(name.as_str().to_owned(), device);
    }
    Provisioned {
        authority,
        devices: phones,
    }
}

/// Port ayıran (direct-mode) testleri AYNI test ikilisi içinde sıraya sokar: [`free_ports`]'un
/// bıraktığı port, düğüm onu bağlayana dek kardeş bir testin `free_ports`'una düşemez (ikililer
/// cargo'da zaten sırayla koşar). Test boyunca tutulur.
pub fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `n` farklı boş UDP portu: soketler aynı anda açık tutulur ki aynı port iki kez verilmesin.
pub fn free_ports(n: usize) -> Vec<u16> {
    let sockets: Vec<UdpSocket> = (0..n)
        .map(|_| UdpSocket::bind("127.0.0.1:0").expect("UDP soketi"))
        .collect();
    sockets
        .iter()
        .map(|socket| socket.local_addr().expect("adres").port())
        .collect()
}

/// Cihaz dosyasındaki `name` eşinin `PeerRef`'i (depoyu doğrudan okuyan denetimler için).
pub fn peer_ref_of(config: &Path, name: &str) -> PeerRef {
    let config = load_device_config(config).expect("cihaz dosyası");
    let peer = config
        .peers
        .iter()
        .find(|peer| peer.name.as_str() == name)
        .expect("eş");
    PeerRef::from_dest_hash(*peer.dest_hash.bytes())
}

/// Olayları kanala yazan dinleyici.
pub struct ChannelListener(Sender<NodeEvent>);

impl NodeListener for ChannelListener {
    fn on_event(&self, event: NodeEvent) {
        // Test bittiyse alıcı yoktur; olay düşer.
        let _ = self.0.send(event);
    }
}

/// Dinleyici + bekleyicisi.
pub fn listener() -> (Box<dyn NodeListener>, Events) {
    let (tx, rx) = mpsc::channel();
    (
        Box::new(ChannelListener(tx)),
        Events {
            rx,
            backlog: Vec::new(),
        },
    )
}

/// Olay bekleyici: eşleşmeyen olaylar sırasıyla saklanır, sonraki beklemeler önce onlara bakar
/// (olay sırası iddiası gerekmez). Periyodik `Metrics` olayları saklanmaz.
pub struct Events {
    rx: Receiver<NodeEvent>,
    backlog: Vec<NodeEvent>,
}

impl Events {
    /// Kanalda ŞU ANA dek birikmiş olayları BLOKLAMADAN saklananlara taşır; `Metrics` olayları
    /// atılır. Sonraki `Metrics` beklemesi yalnız bu çağrıdan sonra kanala giren olaylara bakar.
    pub fn drain_now(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            if !matches!(event, NodeEvent::Metrics(_)) {
                self.backlog.push(event);
            }
        }
    }

    /// Saklananlar + kanalda ŞU ANA dek birikmiş olaylar (`Metrics` hariç), geliş sırasıyla;
    /// saklananlar boşalır. BLOKLAMAZ.
    pub fn drain_all(&mut self) -> Vec<NodeEvent> {
        self.drain_now();
        std::mem::take(&mut self.backlog)
    }

    /// `pick`'in kabul ettiği ilk olayı döner: önce saklananlar (geliş sırasıyla), sonra yeniler —
    /// en çok [`WAIT`] boyunca. Süre dolarsa ya da düğüm kapanırsa görülen olaylarla panik.
    pub fn wait<T>(&mut self, what: &str, pick: impl FnMut(&NodeEvent) -> Option<T>) -> T {
        self.wait_for(what, WAIT, pick)
    }

    /// [`Events::wait`], ÜST SINIR çağırandan: netem altında 60 s'lik kesinti gibi bilinen uzun
    /// beklemeler için (sınır yine ÜST sınırdır; gerçek uyku değildir).
    pub fn wait_for<T>(
        &mut self,
        what: &str,
        limit: Duration,
        mut pick: impl FnMut(&NodeEvent) -> Option<T>,
    ) -> T {
        let saved = self
            .backlog
            .iter()
            .enumerate()
            .find_map(|(index, event)| pick(event).map(|found| (index, found)));
        if let Some((index, found)) = saved {
            self.backlog.remove(index);
            return found;
        }
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(event) => {
                    if let Some(found) = pick(&event) {
                        return found;
                    }
                    if !matches!(event, NodeEvent::Metrics(_)) {
                        self.backlog.push(event);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    panic!(
                        "{what}: {limit:?} içinde gelmedi; görülen: {:?}",
                        self.backlog
                    )
                }
                Err(RecvTimeoutError::Disconnected) => {
                    panic!("{what}: düğüm kapandı; görülen: {:?}", self.backlog)
                }
            }
        }
    }
}
