//! [HLR] S4 C5 uçtan uca — hub kurulu bir oturumun hattını emekli edince (veri → `ROUTE_MISS{h}`)
//! oturum yeni INIT'le yeniden kurulur: Application `Closed` görmez, RECEIPT'i gelmemiş metin yeni
//! oturumdan yeniden gider ve iki yönde metin akar. ESKİ DAVRANIŞ: `kama-node` `HandleLost`'u yok
//! sayıyordu — oturum kalıcı olarak ölüydü, metin hiç ulaşmıyordu.
//!
//! Düzenek: üç `KamaNode` (hub kipi, ürün kurucusu `KamaNode::start`; gerçek kripto, gerçek
//! `HubEndpoint`/`HubLink`, gerçek düğüm döngüsü) + SÜREÇ İÇİ gerçek hub çekirdeği
//! (`hub_server::Hub`, gerçek UDP soketi; `hub-server` ikilisinin `serve` döngüsünün karşılığı).
//! Hub, küresel hat tavanı 2 ile kurulur — `Limits::S4`'ten TEK fark; S4 §2.6 planlayıcısı aynıdır:
//! üçüncü hat açılınca en uzun süredir kullanılmayan hat emekli olur. Üretimde aynı yol uç başına
//! 64 / küresel 8192 hat dolunca işler (heartbeat hattın `last_activity`'sini tazelemez: sessiz ama
//! canlı oturumun hattı emekliye adaydır). Netem yok; zarflar ve veri hub üstünden geçer.
//!
//! Sıra: alice↔bob (L1) → bob↔carol (L2, L1'den sonra trafik) → alice↔carol (L3: tavan → L1
//! emekli; hub tablosundan doğrulanır) → alice↔bob metni L1'e gider → `ROUTE_MISS` → yenileme.

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hub_server::config::Limits;
use hub_server::{Hub, Sink, SystemClock};
use kama_node::{KamaNode, NodeEvent, NodeListener};
use kama_noise_wire::entropy::OsEntropy;
use kama_provision::format::{load_device_config, DeviceName, HUB_FILE_NAME};
use kama_provision::provision::{init_with, InitOptions, InitRequest};

/// Deneme tavanı (yenileme: ROUTE_MISS + hat tahsis bütçesi + boşaltma tavanı içinde kalır).
const ATTEMPT: Duration = Duration::from_secs(90);
/// Hub'ın küresel hat tavanı (bkz. modül belgesi).
const MAX_LINKS: usize = 2;
/// Hub soketinin okuma zaman aşımı (zamanlayıcının trafik yokken de koşması için).
const HUB_POLL: Duration = Duration::from_millis(20);
/// Adımlar arası boşluk: S4 `LINK_ALLOC_RATE` (1/s/uç) ve hatların `last_activity` sırası.
const STEP_GAP: Duration = Duration::from_millis(1500);
/// Hub'a gelen en büyük datagramın bir fazlası (büyük datagram boyundan tanınır).
const HUB_RECV_BUF: usize = hub_server::wire::MAX_DATAGRAM + 1;
const ALICE: &str = "alice";
const BOB: &str = "bob";
const CAROL: &str = "carol";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "kama-s4-hub-relink-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("geçici dizin");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

type HubCore = Hub<SystemClock, OsEntropy>;

/// Gerçek UDP soketine yazan çıkış; log satırları tutulur (hat açılış/emeklilik).
struct UdpSink<'a> {
    socket: &'a UdpSocket,
    log: &'a Mutex<Vec<String>>,
}

impl Sink for UdpSink<'_> {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) {
        let _ = self.socket.send_to(datagram, to);
    }
    fn log(&mut self, line: fmt::Arguments<'_>) {
        lock(self.log).push(line.to_string());
    }
}

/// Süreç içi hub: `hub-server serve` döngüsü (recv → `on_datagram` → send; her turda `on_tick`).
struct InProcessHub {
    core: Arc<Mutex<HubCore>>,
    log: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InProcessHub {
    fn start(socket: UdpSocket, core: HubCore) -> Self {
        socket
            .set_read_timeout(Some(HUB_POLL))
            .expect("zaman aşımı");
        let core = Arc::new(Mutex::new(core));
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (core, log, stop) = (Arc::clone(&core), Arc::clone(&log), Arc::clone(&stop));
            thread::spawn(move || {
                let mut buf = vec![0u8; HUB_RECV_BUF];
                while !stop.load(Ordering::Acquire) {
                    let received = socket.recv_from(&mut buf);
                    let mut sink = UdpSink {
                        socket: &socket,
                        log: &log,
                    };
                    let mut hub = lock(&core);
                    match received {
                        Ok((n, src)) => hub.on_datagram(src, &buf[..n], &mut sink),
                        Err(e)
                            if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                        Err(e) => panic!("hub soketi: {e}"),
                    }
                    hub.on_tick(&mut sink);
                }
            })
        };
        Self {
            core,
            log,
            stop,
            thread: Some(thread),
        }
    }

    /// İki uç arasındaki canlı hat sayısı.
    fn links_between(&self, x: &[u8; 32], y: &[u8; 32]) -> usize {
        lock(&self.core)
            .table()
            .capacity_snapshot()
            .links
            .iter()
            .filter(|(_, a, b, _)| (a == x && b == y) || (a == y && b == x))
            .count()
    }

    fn route_misses(&self) -> u64 {
        lock(&self.core).stats().route_miss
    }

    fn retirements(&self) -> usize {
        lock(&self.log)
            .iter()
            .filter(|l| l.starts_with("link retire"))
            .count()
    }
}

impl Drop for InProcessHub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Listener(Sender<NodeEvent>);

impl NodeListener for Listener {
    fn on_event(&self, event: NodeEvent) {
        let _ = self.0.send(event);
    }
}

/// Bir düğüm ve okunmuş olayları.
struct Peer {
    node: KamaNode,
    rx: Receiver<NodeEvent>,
    seen: Vec<NodeEvent>,
    dest_hash: [u8; 32],
}

impl Peer {
    /// `pick` bir değer verene dek olayları okur; süre dolarsa kırmızı.
    fn wait_for<T>(
        &mut self,
        what: &str,
        deadline: Instant,
        pick: impl Fn(&NodeEvent) -> Option<T>,
    ) -> T {
        if let Some(v) = self.seen.iter().find_map(&pick) {
            return v;
        }
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "deneme tavanı doldu: {what}");
            if let Ok(event) = self.rx.recv_timeout(left.min(Duration::from_millis(200))) {
                let v = pick(&event);
                self.seen.push(event);
                if let Some(v) = v {
                    return v;
                }
            }
        }
    }

    fn connected(&mut self, peer: &str, deadline: Instant) {
        self.wait_for(&format!("Connected {peer}"), deadline, |e| {
            matches!(e, NodeEvent::Connected { peer_id } if peer_id == peer).then_some(())
        });
    }

    fn text(&mut self, from: &str, text: &str, deadline: Instant) {
        self.wait_for(&format!("{from}: {text}"), deadline, |e| {
            matches!(e, NodeEvent::TextReceived { peer_id, text: t, .. }
                if peer_id == from && t == text)
            .then_some(())
        });
    }

    /// Okunmuş olaylarda bu eş için `Closed` yok (hat kaybı Application'a görünmez).
    fn assert_never_closed(&mut self, peer: &str) {
        while let Ok(event) = self.rx.try_recv() {
            self.seen.push(event);
        }
        assert!(
            !self
                .seen
                .iter()
                .any(|e| matches!(e, NodeEvent::Closed { peer_id, .. } if peer_id == peer)),
            "{peer} için Closed olayı"
        );
    }
}

/// Alanlar bildirim sırasıyla düşer: önce düğümler, sonra hub, en son geçici dizin.
struct Net {
    alice: Peer,
    bob: Peer,
    carol: Peer,
    hub: InProcessHub,
    root: TempDir,
}

fn start_peer(root: &Path, out: &Path, name: &str) -> Peer {
    let device = DeviceName::new(name).expect("ad");
    let config = root.join(name).join("config").join(device.file_name());
    fs::create_dir_all(config.parent().expect("üst dizin")).expect("dizin");
    fs::copy(out.join(device.file_name()), &config).expect("cihaz dosyası");
    let dest_hash = *load_device_config(&config)
        .expect("cihaz dosyası okunur")
        .dest_hash
        .bytes();
    let (tx, rx) = mpsc::channel();
    let node = KamaNode::start(
        &config,
        &root.join(name).join("data"),
        Box::new(Listener(tx)),
    )
    .expect("hub kipinde düğüm");
    Peer {
        node,
        rx,
        seen: Vec::new(),
        dest_hash,
    }
}

impl Net {
    fn start(tag: &str) -> Self {
        let root = TempDir::new(tag);
        let out = root.0.join("authority");
        // Hub soketi provisioning'den ÖNCE bağlanır: cihaz dosyalarındaki adres budur (port yarışı yok).
        let socket = UdpSocket::bind("127.0.0.1:0").expect("hub soketi");
        let hub_addr = socket.local_addr().expect("hub adresi");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("saat")
            .as_secs();
        init_with(
            &InitRequest {
                out_dir: out.clone(),
                devices: [ALICE, BOB, CAROL]
                    .map(|n| DeviceName::new(n).expect("ad"))
                    .to_vec(),
                hub: Some(hub_addr.to_string()),
                direct: Default::default(),
            },
            &InitOptions { probe: false },
            &mut OsEntropy::new(),
            now,
        )
        .expect("provisioning v1");
        let config = hub_server::config::load(&out.join(HUB_FILE_NAME)).expect("hub.json");
        let limits = Limits {
            max_links: MAX_LINKS,
            ..Limits::S4
        };
        let core = Hub::with_limits(config, limits, SystemClock::new(), OsEntropy::new())
            .expect("hub çekirdeği");
        let hub = InProcessHub::start(socket, core);
        let alice = start_peer(&root.0, &out, ALICE);
        let bob = start_peer(&root.0, &out, BOB);
        let carol = start_peer(&root.0, &out, CAROL);
        Self {
            alice,
            bob,
            carol,
            hub,
            root,
        }
    }

    /// alice↔bob (L1), bob↔carol (L2), alice↔carol (L3) → tavan L1'i emekli eder.
    fn retire_alice_bob_link(&mut self, deadline: Instant) {
        self.alice.node.connect(BOB).expect("connect");
        self.alice.connected(BOB, deadline);
        self.bob.connected(ALICE, deadline);
        self.alice.node.send_text(BOB, "ping").expect("TEXT");
        self.bob.text(ALICE, "ping", deadline);
        self.bob.node.send_text(ALICE, "pong").expect("TEXT");
        self.alice.text(BOB, "pong", deadline);
        assert_eq!(
            self.hub
                .links_between(&self.alice.dest_hash, &self.bob.dest_hash),
            1
        );

        thread::sleep(STEP_GAP);
        self.bob.node.connect(CAROL).expect("connect");
        self.bob.connected(CAROL, deadline);
        self.carol.connected(BOB, deadline);
        self.bob.node.send_text(CAROL, "b->c").expect("TEXT");
        self.carol.text(BOB, "b->c", deadline);
        assert_eq!(self.hub.retirements(), 0, "tavan dolmadan emeklilik yok");

        thread::sleep(STEP_GAP);
        self.alice.node.connect(CAROL).expect("connect");
        self.alice.connected(CAROL, deadline);
        self.carol.connected(ALICE, deadline);
        assert_eq!(self.hub.retirements(), 1, "tavan: bir hat emekli");
        assert_eq!(
            self.hub
                .links_between(&self.alice.dest_hash, &self.bob.dest_hash),
            0,
            "en uzun süredir kullanılmayan alice↔bob hattı emekli"
        );
        assert_eq!(self.hub.route_misses(), 0);
        // Yenilemenin hat tahsisi `LINK_ALLOC_RATE` bütçesine takılmasın (takılsa da T3 sürer).
        thread::sleep(STEP_GAP);
    }

    fn shutdown(self) {
        let Self {
            alice,
            bob,
            carol,
            hub,
            root,
        } = self;
        for (name, peer) in [(ALICE, alice), (BOB, bob), (CAROL, carol)] {
            peer.node
                .shutdown()
                .unwrap_or_else(|e| panic!("{name} kapanış: {e}"));
        }
        drop(hub);
        drop(root);
    }
}

/// Tek uç (alice) emekli hatta veri yollar → `ROUTE_MISS` → `HandleLost` → yenileme: metin bob'a
/// ulaşır, bob'un cevabı alice'e; iki uçta `Closed` yok; alice metni yeni oturumdan yeniden yolladı.
#[test]
fn s4_retired_link_route_miss_renews_the_session() {
    let deadline = Instant::now() + ATTEMPT;
    let mut net = Net::start("one");
    net.retire_alice_bob_link(deadline);

    let sent = Instant::now();
    net.alice.node.send_text(BOB, "after-retire").expect("TEXT");
    net.bob.text(ALICE, "after-retire", deadline);
    // Salt gözlem (iddia değil): emekli hatta gönderimden teslime süre — yenileme + KARAR-2 boşaltma
    // tavanı (`renew_drain_cap`, ürün 3 s) + yeniden gönderim; loopback, netem yok.
    eprintln!(
        "HLR-OLCUM retired->delivered_ms={}",
        sent.elapsed().as_millis()
    );
    net.bob.node.send_text(ALICE, "reply").expect("TEXT");
    net.alice.text(BOB, "reply", deadline);

    assert!(
        net.hub.route_misses() >= 1,
        "emekli hatta veri → ROUTE_MISS"
    );
    assert_eq!(
        net.hub
            .links_between(&net.alice.dest_hash, &net.bob.dest_hash),
        1,
        "yenileme yeni bir alice↔bob hattı açtı"
    );
    let probe = net.alice.node.probe_for_test().expect("sonda");
    assert!(
        probe.messenger.resent_on_new_session >= 1,
        "RECEIPT'siz metin yeni oturumdan yeniden gitti: {:?}",
        probe.messenger
    );
    net.alice.assert_never_closed(BOB);
    net.bob.assert_never_closed(ALICE);
    net.shutdown();
}

/// İki uç da aynı anda emekli hatta veri yollar → ikisi de `ROUTE_MISS` alır, ikisi de yeniler
/// (S3 glare). Sonuç tek oturum: iki metin de ulaşır, ardından iki yönde ikinci tur da ulaşır; iki
/// uçta `Closed` yok.
#[test]
fn s4_both_ends_route_miss_at_once_end_in_one_session() {
    let deadline = Instant::now() + ATTEMPT;
    let mut net = Net::start("both");
    net.retire_alice_bob_link(deadline);

    net.alice.node.send_text(BOB, "a-1").expect("TEXT");
    net.bob.node.send_text(ALICE, "b-1").expect("TEXT");
    net.bob.text(ALICE, "a-1", deadline);
    net.alice.text(BOB, "b-1", deadline);
    assert!(net.hub.route_misses() >= 2, "iki uç da ROUTE_MISS aldı");

    for round in ["2", "3"] {
        let (a, b) = (format!("a-{round}"), format!("b-{round}"));
        net.alice.node.send_text(BOB, &a).expect("TEXT");
        net.bob.node.send_text(ALICE, &b).expect("TEXT");
        net.bob.text(ALICE, &a, deadline);
        net.alice.text(BOB, &b, deadline);
    }
    net.alice.assert_never_closed(BOB);
    net.bob.assert_never_closed(ALICE);
    net.shutdown();
}
