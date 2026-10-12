//! `kama-node` testlerinin ortak yardımcıları.
//!
//! * Provisioning: cihaz dosyaları `kama-provision` KÜTÜPHANESİYLE (`provision::init`, OS
//!   entropisi) geçici dizinde üretilir — elle yazılmış fikstür DEĞİL. Otorite çıktısı (`root.json`
//!   dahil) ayrı bir dizinde kalır; her "telefon"un dizinine YALNIZ kendi `device-<ad>.json`'u
//!   kopyalanır (D9: `root.json` cihaz dizinine girmez — denetlenir).
//! * Olaylar: dinleyici olayları kanala yazar; [`Events::wait`] beklenen olayı ÜST SÜRE SINIRIYLA
//!   bekler (gerçek uyku yok; koşul olaydır). Eşleşmeyen olaylar sırasıyla saklanır, sonraki
//!   beklemeler önce onlara bakar (olay sırası iddiası gerekmez). Periyodik `Metrics` olayları
//!   saklanmaz: ölçü beklemesi yalnız taze anlık görüntüye bakar.
//! * Zarf taşıyıcısı (DUR-K4G-11 bekleniyor): [`RelayLink`] — `client_core::memory_link::MemoryHub`
//!   (YALNIZ test-util) her gönderim ve yoklamada röle eder; arka plan iş parçacığı ve uyku yok.

// Bu ortak modül birden fazla test ikilisinde ayrı derlenir; her ikili yardımcıların hepsini
// kullanmaz.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use client_core::memory_link::{MemoryHub, MemoryLink};
use client_core::transport::{EnvelopeLink, LinkAttempt, LinkError, LinkEvent};
use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;
use kama_node::{NodeEvent, NodeListener};
use kama_noise_wire::entropy::OsEntropy;
use kama_provision::format::{load_device_config, DeviceName, ROOT_FILE_NAME};
use kama_provision::provision::{init, InitRequest};

/// Beklenen bir olayın ÜST SÜRE SINIRI (aşılırsa kırmızı; sessiz geçiş yok). Loopback'te olaylar
/// milisaniyeler içinde gelir; sınır yavaş CI koşucusu içindir.
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

/// `names` cihazlarını kama-provision kütüphanesiyle üretir. `direct` doğruysa her cihaza
/// 127.0.0.1'de boş bir UDP portu verilir (direct-mode).
pub fn provision(root: &Path, names: &[&str], direct: bool) -> Provisioned {
    let authority = root.join("authority");
    let devices: Vec<DeviceName> = names
        .iter()
        .map(|name| DeviceName::new(name).expect("cihaz adı"))
        .collect();
    let addrs: Vec<SocketAddr> = if direct {
        free_ports(names.len())
            .into_iter()
            .map(|port| SocketAddr::from(([127, 0, 0, 1], port)))
            .collect()
    } else {
        Vec::new()
    };
    let request = InitRequest {
        out_dir: authority.clone(),
        devices: devices.clone(),
        hub: None,
        direct: devices.iter().cloned().zip(addrs.iter().copied()).collect(),
    };
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("sistem saati")
        .as_secs();
    init(&request, &mut OsEntropy::new(), created_at).expect("provisioning");
    let mut phones = BTreeMap::new();
    for (index, name) in devices.iter().enumerate() {
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

/// Olay bekleyici (bkz. modül belgesi).
pub struct Events {
    rx: Receiver<NodeEvent>,
    backlog: Vec<NodeEvent>,
}

impl Events {
    /// `pick`'in kabul ettiği ilk olayı döner: önce saklananlar (geliş sırasıyla), sonra yeniler —
    /// en çok [`WAIT`] boyunca. Süre dolarsa ya da düğüm kapanırsa görülen olaylarla panik.
    pub fn wait<T>(&mut self, what: &str, mut pick: impl FnMut(&NodeEvent) -> Option<T>) -> T {
        let saved = self
            .backlog
            .iter()
            .enumerate()
            .find_map(|(index, event)| pick(event).map(|found| (index, found)));
        if let Some((index, found)) = saved {
            self.backlog.remove(index);
            return found;
        }
        let deadline = Instant::now() + WAIT;
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
                        "{what}: {WAIT:?} içinde gelmedi; görülen: {:?}",
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

/// Test zarf rölesi (DUR-K4G-11 bekleniyor): `MemoryHub` her gönderimde ve her yoklamada röle eder
/// — düğümün turu taşıyıcıyı her `NODE_TICK`'te yokladığından zarf en geç bir tur bekler.
pub struct RelayLink {
    hub: MemoryHub,
    link: MemoryLink,
}

impl RelayLink {
    /// `me` kimliğiyle hub'a kayıtlı taşıyıcı.
    pub fn boxed(hub: &MemoryHub, me: PeerRef) -> Box<dyn EnvelopeLink> {
        Box::new(Self {
            hub: hub.clone(),
            link: hub.link(me),
        })
    }
}

impl EnvelopeLink for RelayLink {
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        let attempt = self.link.send_init(peer, envelope_type, bytes);
        self.hub.relay();
        attempt
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        let sent = self
            .link
            .send_response(peer, hub_handle, envelope_type, bytes);
        self.hub.relay();
        sent
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        self.hub.relay();
        self.link.poll()
    }
}
