//! [S2-A3-2 / HB2] D-14 dumanı (KK-H60): iki `KamaNode` (hub kipi) + `hub-server` ikilisi, tek
//! makine, netem'siz; CLIENT_BIND → BIND_PROOF → STORE_MESSAGE/DELIVER ile SESSION_INIT/RESPONSE →
//! oturum Ready → bir TEXT gidiş-dönüş.
//!
//! Bir koşu = bir deneme (taze geçici dizin, taze hub süreci). `#[ignore]`: PR CI'da koşmaz; 10/10
//! için iş emri §9 komutu ayrı süreçlerde 10 kez koşar. `hub-server` ikilisinin yolu
//! `KAMA_HUB_SERVER_BIN` ortam değişkenindendir (yoksa deneme açık bir iletiyle kırmızıdır).
//!
//! Kanıt: iki uçta `Connected`, iki yönde `TextReceived`, iki düğümde süreç-içi röleye verilen
//! zarf sayısı SIFIR (T11: zarflar hub'dan geçti), `hub_out_unknown_handle = 0` (KK-H66) ve
//! `hub_stash_miss = 0` (KK-H52; tek başına kanıt sayılmaz — üç vaka birim testlerindedir).
//! Mock yeşili bu dumanın yerine geçmez (D-14 eki).

use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kama_node::{KamaNode, NodeEvent, NodeListener};
use kama_noise_wire::entropy::OsEntropy;
use kama_provision::format::{DeviceName, HUB_FILE_NAME, PROBE_NAME};
use kama_provision::provision::{init_with, InitOptions, InitRequest};

/// Deneme tavanı (KK-H60).
const ATTEMPT: Duration = Duration::from_secs(60);
const ALICE: &str = "alice";
const BOB: &str = "bob";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path =
            std::env::temp_dir().join(format!("kama-s4-hub-smoke-{}-{nanos}", std::process::id()));
        fs::create_dir(&path).expect("geçici dizin");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Hub süreci; düşerken öldürülür (Drop panik etmez).
struct Hub(Child);

impl Drop for Hub {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Listener(Sender<NodeEvent>);

impl NodeListener for Listener {
    fn on_event(&self, event: NodeEvent) {
        let _ = self.0.send(event);
    }
}

fn hub_bin() -> PathBuf {
    PathBuf::from(
        std::env::var_os("KAMA_HUB_SERVER_BIN")
            .expect("KAMA_HUB_SERVER_BIN tanımlı değil (iş emri §9: target/debug/hub-server)"),
    )
}

fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .and_then(|s| s.local_addr())
        .expect("boş port")
        .port()
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// `pick` bir değer verene dek olayları okur; süre dolarsa kırmızı.
fn wait_for<T>(
    what: &str,
    events: &Receiver<NodeEvent>,
    deadline: Instant,
    mut pick: impl FnMut(&NodeEvent) -> Option<T>,
) -> T {
    loop {
        let left = remaining(deadline);
        assert!(!left.is_zero(), "deneme tavanı doldu: {what}");
        if let Ok(event) = events.recv_timeout(left.min(Duration::from_millis(200))) {
            if let Some(v) = pick(&event) {
                return v;
            }
        }
    }
}

/// `hub-server probe` çıkış 0 verene dek (hub açılıyor) dener.
fn probe_until_bound(bin: &Path, device: &Path, deadline: Instant) {
    loop {
        let status = Command::new(bin)
            .arg("probe")
            .arg("--device")
            .arg(device)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("hub-server probe çalıştı");
        if status.success() {
            return;
        }
        assert!(
            !remaining(deadline).is_zero(),
            "hub-server probe bind olmadı"
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn start(dir: &Path, name: &str, out: &Path) -> (KamaNode, Receiver<NodeEvent>) {
    let device = DeviceName::new(name).expect("ad");
    let config = dir.join(name).join("config").join(device.file_name());
    fs::create_dir_all(config.parent().expect("üst dizin")).expect("dizin");
    fs::copy(out.join(device.file_name()), &config).expect("cihaz dosyası");
    let (tx, rx) = mpsc::channel();
    let node = KamaNode::start(
        &config,
        &dir.join(name).join("data"),
        Box::new(Listener(tx)),
    )
    .expect("hub kipinde düğüm");
    (node, rx)
}

#[test]
#[ignore = "D-14 dumanı: hub-server ikilisi gerekir (KAMA_HUB_SERVER_BIN); iş emri §9"]
fn s4_hub_smoke_text_round_trip() {
    let deadline = Instant::now() + ATTEMPT;
    let bin = hub_bin();
    let root = TempDir::new();
    let out = root.0.join("authority");
    let port = free_udp_port();
    let hub_addr = format!("127.0.0.1:{port}");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("saat")
        .as_secs();
    init_with(
        &InitRequest {
            out_dir: out.clone(),
            devices: [ALICE, BOB]
                .map(|n| DeviceName::new(n).expect("ad"))
                .to_vec(),
            hub: Some(hub_addr.clone()),
            direct: Default::default(),
        },
        &InitOptions { probe: true },
        &mut OsEntropy::new(),
        now,
    )
    .expect("provisioning v1");

    let _hub = Hub(Command::new(&bin)
        .arg("--config")
        .arg(out.join(HUB_FILE_NAME))
        .arg("--listen")
        .arg(&hub_addr)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("hub-server başladı"));
    let probe_device = out.join(DeviceName::new(PROBE_NAME).expect("ad").file_name());
    probe_until_bound(&bin, &probe_device, deadline);

    let (alice, alice_rx) = start(&root.0, ALICE, &out);
    let (bob, bob_rx) = start(&root.0, BOB, &out);
    alice.connect(BOB).expect("connect");
    for (who, rx) in [(ALICE, &alice_rx), (BOB, &bob_rx)] {
        wait_for(&format!("{who} Connected"), rx, deadline, |e| {
            matches!(e, NodeEvent::Connected { .. }).then_some(())
        });
    }

    alice.send_text(BOB, "ping").expect("alice TEXT");
    let got = wait_for("bob TEXT", &bob_rx, deadline, |e| match e {
        NodeEvent::TextReceived { peer_id, text, .. } if peer_id == ALICE => Some(text.clone()),
        _ => None,
    });
    assert_eq!(got, "ping");
    bob.send_text(ALICE, "pong").expect("bob TEXT");
    let got = wait_for("alice TEXT", &alice_rx, deadline, |e| match e {
        NodeEvent::TextReceived { peer_id, text, .. } if peer_id == BOB => Some(text.clone()),
        _ => None,
    });
    assert_eq!(got, "pong");

    for (who, node) in [(ALICE, &alice), (BOB, &bob)] {
        let probe = node.probe_for_test().expect("sonda");
        assert_eq!(
            probe.envelopes,
            Default::default(),
            "{who}: zarf süreç-içi röleden geçmedi (T11)"
        );
        let hub = probe.hub.expect("hub kipi");
        assert_eq!(hub.hub_out_unknown_handle, 0, "{who}: KK-H66");
        assert_eq!(hub.stash.lost(), 0, "{who}: hub_stash_miss");
        eprintln!("DUMAN-SAYAÇ {who}: {hub:?}");
    }
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}
