//! YALNIZ birim testleri (`#[cfg(test)]`): gerçek kurulum gövdesini ([`KamaNode::build`]) ELLE
//! sürülen düğüm döngüsüyle ([`crate::node::NodeLoop`]) kullanan yardımcılar.
//!
//! İlkeler `tests/common` ile aynıdır: cihaz dosyaları gerçek provisioning yolundan (kama-provision
//! kütüphanesi, OS entropisi; otorite dizini ayrı, telefona yalnız kendi `device-<ad>.json`'u),
//! gerçek kripto + depo + UDP; zarflar `KamaNode::start`'ın kullandığı süreç-içi röleyle
//! (`src/carrier.rs` — röle süreç-geneldir, her test kendi kimliklerini provisioning'le üretir);
//! her bekleme bir koşulun ÜST SINIRIDIR.
//!
//! Elle sürülen düğüm testin iki şeyi denetlemesini sağlar: (1) bir komut grubunu tur ARASINA
//! girmeden koşmak (`NodeLoop::execute` art arda — ürün döngüsü `run` bir grubu turdan önce aynı
//! biçimde işler), (2) zamanı: Application saati [`ManualClock`] olabilir (`Ptt` / `FileTransfer`
//! zamanlayıcıları gerçek 60/120 s beklenmeden sınanır). Ürün döngüsü aynı `execute` / `turn` /
//! `publish_metrics` adımlarından oluşur; elle kurulmuş düğüm `KamaNode::spawn` ile ürün döngüsüne
//! de geçirilebilir.

use std::collections::BTreeMap;
use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kama_app::clock::{MonotonicClock, SystemClock};
use kama_provision::format::DeviceName;
use kama_provision::provision::InitRequest;

use crate::{Built, KamaNode, NodeEvent, NodeListener, NODE_TICK};

/// Beklenen bir koşulun ÜST SÜRE SINIRI (aşılırsa kırmızı; sessiz geçiş yok). Loopback'te koşullar
/// milisaniyeler içinde sağlanır; sınır yavaş CI koşucusu içindir.
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
        let path = std::env::temp_dir().join(format!("kama-node-unit-{label}-{pid}-{seq}-{nanos}"));
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

/// Port ayıran testleri bu test ikilisi içinde sıraya sokar: bırakılan boş port, düğüm onu
/// bağlayana dek kardeş bir testin port aramasına düşemez. Test boyunca tutulur.
pub(crate) fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Bir "telefon": yalnız kendi cihaz dosyası + veri dizini.
pub(crate) struct Phone {
    config: PathBuf,
    data: PathBuf,
}

/// `names` cihazlarını gerçek provisioning yoluyla üretir; her cihaza 127.0.0.1'de boş bir UDP
/// portu (direct-mode). Soketler birlikte açılıp kapatılır: aynı port iki cihaza verilmez.
pub(crate) fn provision(root: &Path, names: &[&str]) -> BTreeMap<String, Phone> {
    let authority = root.join("authority");
    let devices: Vec<DeviceName> = names
        .iter()
        .map(|name| DeviceName::new(name).expect("cihaz adı"))
        .collect();
    let sockets: Vec<UdpSocket> = devices
        .iter()
        .map(|_| UdpSocket::bind("127.0.0.1:0").expect("UDP soketi"))
        .collect();
    let direct = devices
        .iter()
        .cloned()
        .zip(
            sockets
                .iter()
                .map(|socket| socket.local_addr().expect("adres")),
        )
        .collect();
    drop(sockets);
    crate::wiring::provision_for_test(&InitRequest {
        out_dir: authority.clone(),
        devices: devices.clone(),
        hub: None,
        direct,
    });
    devices
        .iter()
        .map(|name| {
            let phone = root.join(name.as_str());
            let config_dir = phone.join("config");
            fs::create_dir_all(&config_dir).expect("telefon dizini");
            let config = config_dir.join(name.file_name());
            fs::copy(authority.join(name.file_name()), &config).expect("cihaz dosyası");
            let phone = Phone {
                config,
                data: phone.join("data"),
            };
            (name.as_str().to_owned(), phone)
        })
        .collect()
}

/// Elle ilerletilen Application saati (`kama_app::clock::MonotonicClock`). Kopyalar aynı zamanı
/// paylaşır: test bir kopyayı tutar, diğerini düğüme verir. Zaman yalnız ileri gider.
#[derive(Debug, Clone, Default)]
pub(crate) struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    /// Zamanı `by` kadar ilerletir (u64 nanosaniyede doyar).
    pub(crate) fn advance(&self, by: Duration) {
        let step = u64::try_from(by.as_nanos()).unwrap_or(u64::MAX);
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| {
                Some(now.saturating_add(step))
            });
    }
}

impl MonotonicClock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.0.load(Ordering::SeqCst))
    }
}

/// Ürün Application saati (zamanın denetlenmediği testler için).
pub(crate) fn system_clock() -> Arc<dyn MonotonicClock> {
    Arc::new(SystemClock::new())
}

/// Olayları kanala yazan dinleyici.
struct ChannelListener(Sender<NodeEvent>);

impl NodeListener for ChannelListener {
    fn on_event(&self, event: NodeEvent) {
        // Test bittiyse alıcı yoktur; olay düşer.
        let _ = self.0.send(event);
    }
}

/// Olay bekleyici. Periyodik `Metrics` olayları SAKLANMAZ (birim testleri ölçüyü düğümün anlık
/// görüntüsünden okur); diğerleri geliş sırasıyla saklanır.
pub(crate) struct Events {
    rx: Receiver<NodeEvent>,
    backlog: Vec<NodeEvent>,
}

impl Events {
    fn absorb(&mut self, event: NodeEvent) {
        if !matches!(event, NodeEvent::Metrics(_)) {
            self.backlog.push(event);
        }
    }

    /// Şu ana dek gelmiş olaylardan `pick`'in kabul ettiği İLKİNİ (geliş sırasıyla) alır;
    /// BLOKLAMAZ. Elle sürülen düğümün olayları `execute` / `turn` dönmeden kanaldadır.
    pub(crate) fn take<T>(&mut self, mut pick: impl FnMut(&NodeEvent) -> Option<T>) -> Option<T> {
        while let Ok(event) = self.rx.try_recv() {
            self.absorb(event);
        }
        let (index, found) = self
            .backlog
            .iter()
            .enumerate()
            .find_map(|(index, event)| pick(event).map(|found| (index, found)))?;
        self.backlog.remove(index);
        Some(found)
    }

    /// Şu ana dek gelmiş BÜTÜN olaylar (`Metrics` hariç), geliş sırasıyla; saklananlar boşalır.
    /// BLOKLAMAZ. Olay sırası iddiaları içindir.
    pub(crate) fn drain_all(&mut self) -> Vec<NodeEvent> {
        while let Ok(event) = self.rx.try_recv() {
            self.absorb(event);
        }
        std::mem::take(&mut self.backlog)
    }

    /// İş parçacıklı düğüm için: [`Events::take`], bulunana dek en çok [`WAIT`].
    pub(crate) fn wait<T>(
        &mut self,
        what: &str,
        mut pick: impl FnMut(&NodeEvent) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(found) = self.take(&mut pick) {
                return found;
            }
            match self
                .rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(event) => self.absorb(event),
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

/// Olay kanalı: gönderen ucu dinleyiciye (ör. testin kendi dinleyicisi), bekleyici teste.
pub(crate) fn event_channel() -> (Sender<NodeEvent>, Events) {
    let (tx, rx) = mpsc::channel();
    let events = Events {
        rx,
        backlog: Vec::new(),
    };
    (tx, events)
}

fn listener() -> (Box<dyn NodeListener>, Events) {
    let (tx, events) = event_channel();
    (Box::new(ChannelListener(tx)), events)
}

/// Elle sürülen düğüm: `KamaNode::start` ile AYNI kurulum gövdesi (`build`; süreç-içi röle kaydı
/// dahil), iş parçacığı yok; Application saati `app_clock`. Döngü `Built::node`'dur.
pub(crate) fn manual(phone: &Phone, app_clock: Arc<dyn MonotonicClock>) -> (Built, Events) {
    let (listener, events) = listener();
    let built =
        KamaNode::build(&phone.config, &phone.data, listener, app_clock).expect("düğüm kurulmalı");
    (built, events)
}

/// İş parçacıklı düğüm: ürün yolu (`KamaNode::start`).
pub(crate) fn threaded(phone: &Phone) -> (KamaNode, Events) {
    let (listener, events) = listener();
    (threaded_with(phone, listener), events)
}

/// [`threaded`], testin verdiği dinleyiciyle.
pub(crate) fn threaded_with(phone: &Phone, listener: Box<dyn NodeListener>) -> KamaNode {
    KamaNode::start(&phone.config, &phone.data, listener).expect("düğüm kurulmalı")
}

/// `step` bir değer verene dek tekrarlar — en çok [`WAIT`]. Adımlar arasında [`NODE_TICK`] kadar
/// durur: ürün döngüsünün boşta bekleme aralığı (diğer iş parçacıkları ilerlesin); doğruluk süreye
/// bağlı değildir, koşul her adımda yeniden denetlenir.
pub(crate) fn until<T>(what: &str, mut step: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(found) = step() {
            return found;
        }
        assert!(Instant::now() < deadline, "{what}: {WAIT:?} içinde olmadı");
        std::thread::park_timeout(NODE_TICK);
    }
}
