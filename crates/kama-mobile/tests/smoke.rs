//! K4G-15 smoke testi (kart NE 4): iki `KamaMobile` aynı süreçte — `start` → `connect` →
//! `send_text` → olay. Yüzey bir Kotlin tüketicisi gibi crate dışından çağrılır; dinleyici
//! (`EngineListener`) olayları test içinde bir kanala yazar.
//!
//! Cihaz dosyaları `kama-provision init` CLI yoluyla (`kama_provision::cli::run`, OS entropisi)
//! geçici dizinde üretilir — elle yazılmış fikstür DEĞİL. Otorite çıktısı (`root.json` dahil) ayrı
//! dizinde kalır; her "telefon"un dizinine YALNIZ kendi `device-<ad>.json`'u kopyalanır (D9).
//! Direct-mode adresleri 127.0.0.1'de boş UDP portlarıdır. Zarf taşıyıcısı ürün yoludur (süreç-içi
//! röle; DUR-K4G-11 → B): iki düğüm de bağlanmadan ÖNCE başlatılır.
//!
//! Akış: oturumsuz `send_text` → `Messenger(NoSession)` (DUR-K4G-11b: arayüz `Connected`'ı bekler)
//! · `connect` → iki uçta `Connected` · `send_text` → karşıda `TextReceived`, gönderende `Receipt`
//! · `metrics()` eş listesini provisioning'den verir, salt-okunur kaynağı olmayan alanlar `None` ·
//! `shutdown` idempotent, sonrasında çağrılar `Stopped`, aynı `data_dir` ile yeniden `start`.
//!
//! Zamanlama: gerçek uyku yok; her bekleme bir olayın ÜST SINIRIDIR ([`WAIT`]).

use std::ffi::OsString;
use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kama_mobile::{
    EngineEvent, EngineListener, KamaMobile, LinkState, ListenerError, MessengerError, NodeError,
};
use kama_provision::format::ROOT_FILE_NAME;

const ALICE: &str = "alice";
const BOB: &str = "bob";
/// Gönderilen metin.
const TEXT: &str = "merhaba bob";
/// Beklenen bir olayın ÜST SÜRE SINIRI (aşılırsa kırmızı). Loopback'te olaylar milisaniyeler
/// içinde gelir; sınır yavaş CI koşucusu içindir (`kama-node` testleriyle aynı).
const WAIT: Duration = Duration::from_secs(60);

/// `std::env::temp_dir()` altında bu teste özgü dizin; Drop'ta silinir (panik etmez).
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("kama-mobile-{label}-{pid}-{seq}-{nanos}"));
        fs::create_dir(&path).expect("geçici dizin");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Bir "telefon": yalnız kendi cihaz dosyası + veri dizini (Kotlin'in `noBackupFilesDir`'i).
struct Phone {
    config: String,
    data: String,
}

/// `n` farklı boş UDP portu: soketler aynı anda açık tutulur ki aynı port iki kez verilmesin.
fn free_ports(n: usize) -> Vec<u16> {
    let sockets: Vec<UdpSocket> = (0..n)
        .map(|_| UdpSocket::bind("127.0.0.1:0").expect("UDP soketi"))
        .collect();
    sockets
        .iter()
        .map(|socket| socket.local_addr().expect("adres").port())
        .collect()
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("geçici yol UTF-8").to_owned()
}

/// Alice ve Bob'u `kama-provision init` ile üretir; her telefona yalnız kendi dosyası gider.
fn provision(root: &Path) -> (Phone, Phone) {
    let authority = root.join("authority");
    let ports = free_ports(2);
    let (alice_port, bob_port) = (ports[0], ports[1]);
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
    let phone = |name: &str| {
        let source = authority.join(format!("device-{name}.json"));
        assert!(
            source.is_file(),
            "kama-provision init {name} dosyasını yazmadı"
        );
        let config_dir = root.join(name).join("config");
        fs::create_dir_all(&config_dir).expect("telefon dizini");
        let config = config_dir.join(format!("device-{name}.json"));
        fs::copy(&source, &config).expect("cihaz dosyası");
        assert!(
            !config_dir.join(ROOT_FILE_NAME).exists(),
            "D9: root.json cihaz dizinine girmez"
        );
        Phone {
            config: utf8(&config),
            data: utf8(&root.join(name).join("data")),
        }
    };
    (phone(ALICE), phone(BOB))
}

/// Olayları kanala yazan dinleyici (Kotlin'deki `onEvent` → `Channel.trySend` deseni).
struct ChannelListener(Sender<EngineEvent>);

impl EngineListener for ChannelListener {
    fn on_event(&self, ev: EngineEvent) -> Result<(), ListenerError> {
        // Test bittiyse alıcı yoktur; olay düşer.
        let _ = self.0.send(ev);
        Ok(())
    }
}

/// Olay bekleyici: eşleşmeyen olaylar sırasıyla saklanır; periyodik `Metrics` saklanmaz.
struct Events {
    rx: Receiver<EngineEvent>,
    backlog: Vec<EngineEvent>,
}

impl Events {
    /// `pick`'in kabul ettiği ilk olay: önce saklananlar, sonra yeniler — en çok [`WAIT`] boyunca.
    fn wait<T>(&mut self, what: &str, mut pick: impl FnMut(&EngineEvent) -> Option<T>) -> T {
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
                    if !matches!(event, EngineEvent::Metrics { .. }) {
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

/// Ürün yolu: `KamaMobile::start` (Kotlin'de `KamaMobile.start(configPath, dataDir, listener)`).
fn start(phone: &Phone) -> (Arc<KamaMobile>, Events) {
    let (tx, rx) = mpsc::channel();
    let node = KamaMobile::start(
        phone.config.clone(),
        phone.data.clone(),
        Box::new(ChannelListener(tx)),
    )
    .expect("KamaMobile.start");
    let events = Events {
        rx,
        backlog: Vec::new(),
    };
    (node, events)
}

fn connected(name: &str) -> impl FnMut(&EngineEvent) -> Option<()> + '_ {
    move |event| match event {
        EngineEvent::Connected { peer_id } if peer_id == name => Some(()),
        _ => None,
    }
}

#[test]
fn start_connect_send_text_and_events_across_two_kama_mobiles() {
    let root = TempDir::new("smoke");
    let (alice_phone, bob_phone) = provision(root.path());
    let (bob, mut bob_events) = start(&bob_phone);
    let (alice, mut alice_events) = start(&alice_phone);

    // DUR-K4G-11b: arayüz "Bağlandı" olayını bekler; oturumsuz `send_text` → `NoSession`.
    let early = alice.send_text(BOB.to_owned(), TEXT.to_owned());
    assert_eq!(
        early,
        Err(NodeError::Messenger {
            reason: MessengerError::NoSession
        })
    );
    assert_eq!(
        alice.send_text("yabanci".to_owned(), TEXT.to_owned()),
        Err(NodeError::UnknownPeer)
    );

    // start → connect → iki uçta Connected.
    alice.connect(BOB.to_owned()).expect("connect");
    alice_events.wait("alice: Connected(bob)", connected(BOB));
    bob_events.wait("bob: Connected(alice)", connected(ALICE));

    // send_text → karşıda TextReceived, gönderende Receipt (aynı msg_id).
    let msg_id = alice
        .send_text(BOB.to_owned(), TEXT.to_owned())
        .expect("send_text");
    bob_events.wait("bob: TextReceived(alice)", |event| {
        matches!(
            event,
            EngineEvent::TextReceived { peer_id, msg_id: got, text }
                if peer_id == ALICE && *got == msg_id && text == TEXT
        )
        .then_some(())
    });
    alice_events.wait("alice: Receipt(bob)", |event| {
        matches!(
            event,
            EngineEvent::Receipt { peer_id, msg_id: got } if peer_id == BOB && *got == msg_id
        )
        .then_some(())
    });

    // metrics(): eş listesi provisioning'den (K6 Contacts); bağlı eş görünür; salt-okunur kaynağı
    // olmayan alanlar uydurulmaz (README zorunlu satır 3).
    let metrics = alice.metrics().expect("metrics");
    let peers: Vec<&str> = metrics
        .connect_state
        .iter()
        .map(|state| state.peer_id.as_str())
        .collect();
    assert_eq!(peers, [BOB]);
    assert_eq!(metrics.connect_state[0].state, LinkState::Connected);
    assert_eq!(metrics.rtt_ms, None);
    assert_eq!(metrics.loss_pct, None);
    assert_eq!(metrics.bitrate_kbps, None);
    assert_eq!(metrics.last_heartbeat_age_s, None);
    assert_eq!(metrics.ctx_mismatch_drops, 0, "{metrics:?}");
    assert_eq!(metrics.delivery_channel_drops, 0, "{metrics:?}");

    // shutdown: idempotent; sonrasında her çağrı Stopped; aynı data_dir ile yeniden start edilir
    // (depo kilidi, UDP soketi ve süreç-içi röle kaydı bırakıldı).
    alice.shutdown().expect("shutdown");
    alice.shutdown().expect("ikinci shutdown etkisiz");
    assert_eq!(
        alice.send_text(BOB.to_owned(), TEXT.to_owned()),
        Err(NodeError::Stopped)
    );
    assert_eq!(alice.metrics(), Err(NodeError::Stopped));
    let (again, _again_events) = start(&alice_phone);
    again.shutdown().expect("yeniden açılan düğümün kapanışı");
    bob.shutdown().expect("bob kapanış");
}
