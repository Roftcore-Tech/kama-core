//! Gerçek `hub-server` ikilisi, gerçek UDP (loopback): `main.rs`'in kablolaması, `probe` alt komutu
//! (KM-DEPLOY-TDD-001 §4, 6.4) ve T-C3 (okunamayan `hub.json` → tek satır, çıkış 1). Davranış testleri
//! IO'suz çekirdeğe karşı `tests/tdd/`'dedir. Geçici dosyalar `std::env::temp_dir()` altında, silinir.

#[allow(dead_code)]
#[path = "tdd/harness.rs"]
mod harness;

use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use harness::{
    base64, data, deliver, hex, hub_json, hub_public, reply, store_response, Phone, HUB_SEED,
};
use hub_server::wire::{
    classify, code, envelope, frame, method, ClientBindResponse, Datagram, Payload,
};

const BIN: &str = env!("CARGO_BIN_EXE_hub-server");
const TIMEOUT: Duration = Duration::from_secs(10);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("kama-hub-{label}-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("geçici dizin");
        Self(path)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).expect("geçici dosya");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn probe_phone() -> Phone {
    Phone::new("probe", 0x4d, [127, 0, 0, 1], 1)
}

/// Dizin + probe cihazı.
fn hub_config(dir: &TempDir) -> PathBuf {
    let mut devices = Phone::directory();
    devices.push(probe_phone());
    dir.write("hub.json", &hub_json(&devices))
}

/// Provisioning v1 cihaz dosyasının probe'un okuduğu alanları (geri kalanı yok sayılır).
fn device_file(phone: &Phone, hub_addr: &str) -> String {
    format!(
        r#"{{ "format_version": 1, "device_id": "{}", "dest_hash": "{}",
  "ed25519": {{ "priv": "{}", "pub": "{}" }},
  "cert": "{}",
  "hub": {{ "addr": "{}", "x25519_pub": null, "ed25519_pub": "{}" }} }}"#,
        phone.name,
        hex(&phone.id),
        hex(&phone.key.to_bytes()),
        hex(&phone.key.verifying_key().to_bytes()),
        base64(&phone.cert),
        hub_addr,
        hex(&hub_public())
    )
}

struct RunningHub {
    child: Child,
    addr: SocketAddr,
    stdout: BufReader<std::process::ChildStdout>,
}

impl RunningHub {
    fn start(config: &Path) -> Self {
        let mut child = Command::new(BIN)
            .arg("--config")
            .arg(config)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("hub-server başlamalı");
        let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut line = String::new();
        stdout.read_line(&mut line).expect("açılış satırı");
        let addr = line
            .split_whitespace()
            .find_map(|w| w.strip_prefix("addr="))
            .unwrap_or_else(|| panic!("açılış satırı: {line:?}"))
            .parse()
            .expect("adres");
        Self {
            child,
            addr,
            stdout,
        }
    }
}

impl Drop for RunningHub {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Device {
    phone: Phone,
    socket: UdpSocket,
    hub: SocketAddr,
}

impl Device {
    fn new(phone: Phone, hub: SocketAddr) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("soket");
        socket.set_read_timeout(Some(TIMEOUT)).expect("zaman aşımı");
        Self { phone, socket, hub }
    }

    fn send(&self, datagram: &[u8]) {
        self.socket.send_to(datagram, self.hub).expect("gönder");
    }

    fn receive(&self) -> Vec<u8> {
        let mut buffer = [0u8; 2048];
        let (n, from) = self
            .socket
            .recv_from(&mut buffer)
            .expect("hub'dan datagram bekleniyordu");
        assert_eq!(from, self.hub);
        buffer[..n].to_vec()
    }

    fn bind(&mut self, hub_pub: &[u8; 32], cn: [u8; 16]) {
        let rid = self.phone.rid();
        self.send(&self.phone.bind1(rid, cn));
        let r = reply(&self.receive());
        let hn = ClientBindResponse::decode(&r.payload)
            .expect("CHALLENGE")
            .hub_nonce;
        let rid = self.phone.rid();
        self.send(&self.phone.bind2(rid, hub_pub, cn, hn));
        let r = reply(&self.receive());
        assert_eq!(
            (r.method_id, r.flags),
            (method::BIND_PROOF, 0x01),
            "bind_ok"
        );
    }
}

#[test]
fn bind_store_deliver_and_data_over_real_udp() {
    let dir = TempDir::new("udp");
    let mut hub = RunningHub::start(&hub_config(&dir));
    let hub_pub = hub_public();
    let mut alice = Device::new(Phone::alice(), hub.addr);
    let mut bob = Device::new(Phone::bob(), hub.addr);
    alice.bind(&hub_pub, [0xa1; 16]);
    bob.bind(&hub_pub, [0xb0; 16]);

    // Heartbeat.
    alice.send(&harness::heartbeat(&[1, 2, 3, 4]));
    assert_eq!(
        classify(&alice.receive()),
        Ok(Datagram::Control {
            frame_type: frame::HEARTBEAT_ACK,
            body: &[1, 2, 3, 4]
        })
    );

    // INIT → {OK, h} + DELIVER; RESPONSE → DELIVER.
    let bob_phone = bob.phone.clone();
    let (_, init) = alice.phone.init(&bob_phone, b"SSI1 zarf");
    alice.send(&init);
    let response = store_response(&alice.receive());
    assert_eq!(response.status, code::OK);
    let h = response.assigned_handle;
    assert_eq!(
        deliver(&bob.receive()),
        (
            h,
            alice.phone.id,
            envelope::SESSION_INIT,
            b"SSI1 zarf".to_vec()
        )
    );
    let rid = bob.phone.rid();
    bob.send(&bob.phone.store(
        rid,
        h,
        envelope::SESSION_INIT_RESPONSE,
        &alice.phone,
        b"SSR1",
    ));
    assert_eq!(store_response(&bob.receive()).status, code::OK);
    assert_eq!(deliver(&alice.receive()).0, h);

    // Veri: en büyük datagram, iki yönde, bayt-özdeş; sınırın bir bayt üstü düşer, döngü sürer.
    let payload: Vec<u8> = (0..1276u32).map(|n| (n % 251) as u8).collect();
    let datagram = data(h, &payload);
    alice.send(&datagram);
    assert_eq!(bob.receive(), datagram);
    bob.send(&datagram);
    assert_eq!(alice.receive(), datagram);
    alice.send(&data(h, &[0; 1277]));
    let after = data(h, b"sinir asimindan sonra");
    alice.send(&after);
    assert_eq!(bob.receive(), after);

    let mut seen = String::new();
    for _ in 0..3 {
        hub.stdout.read_line(&mut seen).expect("log");
    }
    assert!(seen.contains("bind ok dh=a1b2c3d4 epoch=1"), "{seen}");
    assert!(
        seen.contains(&format!(
            "link open h={h:08x} a=a1b2c3d4 b=c3d4e5f6 type=01"
        )),
        "{seen}"
    );
    assert!(!seen.contains(&hex(&HUB_SEED)));
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(BIN)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("hub-server koşmalı")
}

/// KM-DEPLOY-TDD-001 6.4 — `hub-server probe --device device-probe.json --hub <h:p>` → `bind ok … heartbeat 3/3`.
#[test]
fn probe_binds_and_gets_three_heartbeats() {
    let dir = TempDir::new("probe");
    let hub = RunningHub::start(&hub_config(&dir));
    let device = dir.write(
        "device-probe.json",
        &device_file(&probe_phone(), "127.0.0.1:47000"),
    );
    let target = hub.addr.to_string();
    let output = run(&[
        "probe".as_ref(),
        "--device".as_ref(),
        device.as_os_str(),
        "--hub".as_ref(),
        target.as_ref(),
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{stdout} / {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.starts_with("bind ok rtt=") && stdout.trim_end().ends_with("ms · heartbeat 3/3"),
        "{stdout}"
    );

    // Dizinde olmayan cihaz: hub sessiz → probe kırmızı.
    let stranger = dir.write(
        "device-mallory.json",
        &device_file(&Phone::mallory(), "127.0.0.1:47000"),
    );
    let output = run(&[
        "probe".as_ref(),
        "--device".as_ref(),
        stranger.as_os_str(),
        "--hub".as_ref(),
        target.as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("CLIENT_BIND: no answer"));
}

/// T-C3 — okunamayan dosya → tek satır `config unreadable: <yol>`, çıkış 1, başka çıktı yok; bozuk ve
/// bilinmeyen alanlı dosya da başlamaz; kullanım hatası 2.
#[test]
fn t_c3_unreadable_or_bad_config_refuses_to_start() {
    let dir = TempDir::new("cfg");
    let missing = dir.0.join("yok.json");
    let output = run(&["--config".as_ref(), missing.as_os_str()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("config unreadable: {}\n", missing.display())
    );
    // Dizin de okunamaz.
    let output = run(&["--config".as_ref(), dir.0.as_os_str()]);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("config unreadable: {}\n", dir.0.display())
    );

    let valid = hub_json(&Phone::directory());
    let unknown = dir.write(
        "unknown.json",
        &valid.replacen(
            r#""format_version": 1,"#,
            r#""format_version": 1, "debug": true,"#,
            1,
        ),
    );
    let output = run(&["--config".as_ref(), unknown.as_os_str()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(&hex(&HUB_SEED)[..16]));

    let good = dir.write("hub.json", &valid);
    for args in [
        vec![],
        vec!["--help".as_ref()],
        vec!["--config".as_ref()],
        vec!["--config".as_ref(), good.as_os_str(), "x".as_ref()],
        vec!["probe".as_ref(), "--hub".as_ref()],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage: hub-server"));
    }
}

/// YAAZ-EK-1 (INT-02H) — yerelde tek komut: `--listen` hub.json'daki adresi ezer; `probe` `--hub`
/// verilmezse cihaz dosyasındaki `hub.addr`'a bağlanır.
#[test]
fn local_run_with_listen_override_and_probe_from_the_device_file() {
    let dir = TempDir::new("local");
    let port = {
        let free = UdpSocket::bind("127.0.0.1:0").expect("boş port");
        free.local_addr().expect("adres").port()
    };
    let listen = format!("127.0.0.1:{port}");
    let mut child = Command::new(BIN)
        .args([
            "--config".as_ref(),
            hub_config(&dir).as_os_str(),
            "--listen".as_ref(),
            listen.as_ref(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("hub-server başlamalı");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let mut line = String::new();
    stdout.read_line(&mut line).expect("açılış satırı");
    assert!(line.contains(&format!("addr={listen}")), "{line}");
    let device = dir.write("device-probe.json", &device_file(&probe_phone(), &listen));
    let output = run(&["probe".as_ref(), "--device".as_ref(), device.as_os_str()]);
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("heartbeat 3/3"));
}
