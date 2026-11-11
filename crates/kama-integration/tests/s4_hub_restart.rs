//! [HUB-RESTART] Gerçek `hub-server` SÜRECİ veri uçuştayken `SIGKILL` ile öldürülür ve aynı
//! yapılandırma, aynı port ile yeniden başlatılır; iki `KamaNode` (hub kipi, ürün kurucusu
//! `KamaNode::start`) uygulama müdahalesi olmadan toparlanmalıdır. Loopback, netem yok.
//!
//! # Spec dayanağı (iddia edilen yalnız bunlar)
//!
//! * S4 §1 madde 9 + beyanlı sınır 3 (`docs/seams/KM-SEAM-TH-001.md`): hub'daki her tablo
//!   bellektedir; yeniden başlayan hub BOŞ başlar, telefonlar yeniden bind olur ve oturumlarını
//!   yeniden kurar. Hub'ın kalıcı durumu yalnız `hub.json`'dur (dizin + hub anahtarı); mesaj
//!   saklamaz (S4 §1 madde 3, 10). Bu yüzden "aynı depolama" = aynı `hub.json`; hub'ın başka kalıcı
//!   deposu yoktur (depolama kaybıyla yeniden başlama ikinci değişkeni burada ANLAMSIZDIR).
//! * S4 H10: yeniden başlayan hub için uçlar bağsızdır; bağsız kaynağın veri ve heartbeat
//!   datagramları CEVAPSIZDIR → istemci "koptu"yu yalnız cevapsız aktif yoklamayla öğrenir (C6) ve
//!   yeniden bind olur (C7); eski handle'a veri `ROUTE_MISS` alır, oturum yeni INIT'le yenilenir
//!   (C5, C7 (b)).
//! * Deploy TDD §6 süre kuralı: "hub yeniden başladığında ... eski handle ilk veride `ROUTE_MISS`
//!   alır ve o oturum bir el sıkışmayla yeniden kurulur". Süre SAYISI spec'te yoktur; burada yalnız
//!   gözlem olarak basılır (`HUB-RESTART-OLCUM`), üst sınır koşul döngüsünün deneme tavanıdır.
//! * AMP §3 `msg_id` (+ kama-app `messaging` belgesi): gönderen `RECEIPT`'i gelmeyen TEXT'i kalıcı
//!   outbox'tan sıradaki oturumda AYNI `msg_id` ile yeniden gönderir; alıcı dedup penceresinde ilk
//!   kez görülen `msg_id`'yi bir kez teslim eder; `Receipt` `(eş, msg_id)` başına tam bir kez. Alıcı
//!   teslim SIRASINI `msg_id` ile ZORLAMAZ (sıra yalnız oturum içinde Transport Reliable'dan gelir):
//!   bu test sırayı İDDİA ETMEZ, gözlem olarak sayar.
//! * Tek etkin oturum: S1 KARAR-2 (`docs/seams/KM-SEAM-AT-001.md`, atomik cutover — eşin TEK güncel
//!   oturumu). "Bekleyen yenileme 0, onaysız provisional 0" spec metninde YOKTUR: #180'in uygulama
//!   sözleşmesidir (S1 YAMA ADAYI 1) — burada BEYANLI VARSAYIM olarak sınanır.
//! * Dosya: kama-app `file_transfer` "Oturum değişimi" (plan D6) — yarım aktarım oturum
//!   değişiminde `FILE_ABORT(superseded)` ile kapanır, kaldığı yerden devam YOKTUR. Bu yüzden dosya
//!   yalnız TOPARLANMADAN SONRA gönderilir (yeni oturumda dosya akışı); uçuştaki dosya test edilmedi.
//!
//! # Düzenek
//!
//! Hub ayrı OS sürecidir (`KAMA_HUB_SERVER_BIN`, `s4_hub_smoke` ile aynı kural; PR CI'da koşmaz).
//! Sabit port: süreç başlamadan boş bir port seçilir, provisioning ve iki başlatma onu kullanır.
//! Hub'ın standart çıktısı (log satırları) geçici dizinde `hub-<n>.log`'a yazılır ve kırmızıda
//! basılır.
//!
//! Akış: bağlan → (öncesi) alice→bob `PRE` metin, her biri teslim beklenerek → (uçuşta) `BURST`
//! metin beklemeden + hemen `SIGKILL` → (sırası) hub ölüyken `DURING` metin + bob→alice bir metin →
//! yeniden başlat → (sonrası) `AFTER` metin → hepsinin teslimi ve alındısı → bob→alice ikinci tur +
//! alice→bob dosya.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::net::UdpSocket;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kama_app::messaging::MsgId;
use kama_node::{FileStage, KamaNode, NodeEvent, NodeListener};
use kama_noise_wire::entropy::OsEntropy;
use kama_provision::format::{DeviceName, HUB_FILE_NAME};
use kama_provision::provision::{init_with, InitOptions, InitRequest};

/// Kurulum (hub açılışı + bağlantı) deneme tavanı.
const SETUP: Duration = Duration::from_secs(60);
/// Yeniden başlatmadan sonra toparlanma deneme tavanı (gözlem değil, koşul döngüsünün sınırı).
const RECOVERY: Duration = Duration::from_secs(180);
/// Öldürme ile yeniden başlatma arası (hub yokken gönderim penceresi).
const OUTAGE: Duration = Duration::from_secs(2);
/// Öldürmeden sonra eski hub üstünden teslim edilmiş iletilerin olaylarının okunduğu süre (hub
/// öldükten sonra yeni teslim OLAMAZ; bu yalnız düğüm olay kanalının boşaltılmasıdır).
const KILL_DRAIN: Duration = Duration::from_millis(500);
/// Koşul döngüsünün adımı.
const POLL: Duration = Duration::from_millis(20);
/// Öncesi / uçuşta / sırası / sonrası alice→bob metin sayıları.
const PRE: usize = 10;
const BURST: usize = 40;
const DURING: usize = 10;
const AFTER: usize = 10;
const TOTAL: usize = PRE + BURST + DURING + AFTER;
/// Hub ölüyken ve yeniden başladıktan sonra metinler arası boşluk.
const SEND_GAP: Duration = Duration::from_millis(100);
/// Metin dolgusu (TEXT ≤ 1002 B; birden çok paket uçuşta kalsın diye büyük).
const PAD: usize = 600;
/// Toparlanmadan sonraki bob→alice metin sayısı.
const B2A_AFTER: usize = 5;
/// Toparlanmadan sonra gönderilen dosyanın boyu (birden çok `FILE_CHUNK`).
const FILE_BYTES: usize = 48 * 1024;
const ALICE: &str = "alice";
const BOB: &str = "bob";
/// Hub ikilisinin açılış satırı (`hub-server` `serve`: soket bağlandıktan sonra yazılır).
const HUB_READY_LINE: &str = "hub listening addr=";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "kama-s4-hub-restart-{}-{nanos}",
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

fn hub_bin() -> PathBuf {
    PathBuf::from(
        std::env::var_os("KAMA_HUB_SERVER_BIN")
            .expect("KAMA_HUB_SERVER_BIN tanımlı değil (hub-server ikilisinin yolu)"),
    )
}

fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .and_then(|s| s.local_addr())
        .expect("boş port")
        .port()
}

/// Gerçek hub süreci: aynı ikili, aynı `hub.json`, aynı `--listen`; her başlatma ayrı log dosyası.
struct HubProcess {
    bin: PathBuf,
    config: PathBuf,
    listen: String,
    log_dir: PathBuf,
    child: Option<Child>,
    generation: u32,
}

impl HubProcess {
    fn log_path(&self, generation: u32) -> PathBuf {
        self.log_dir.join(format!("hub-{generation}.log"))
    }

    fn log(&self, generation: u32) -> String {
        fs::read_to_string(self.log_path(generation)).unwrap_or_default()
    }

    /// Süreci başlatır ve açılış satırını bekler (soket bağlandı).
    fn spawn(&mut self, deadline: Instant) {
        assert!(self.child.is_none(), "hub zaten çalışıyor");
        self.generation += 1;
        let out = File::create(self.log_path(self.generation)).expect("hub log dosyası");
        let err = out.try_clone().expect("hub log dosyası");
        let child = Command::new(&self.bin)
            .arg("--config")
            .arg(&self.config)
            .arg("--listen")
            .arg(&self.listen)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("hub-server başladı");
        self.child = Some(child);
        loop {
            if self.log(self.generation).contains(HUB_READY_LINE) {
                return;
            }
            if let Some(status) = self
                .child
                .as_mut()
                .and_then(|c| c.try_wait().expect("hub durumu"))
            {
                panic!(
                    "hub-server açılmadan çıktı ({status}): {}",
                    self.log(self.generation)
                );
            }
            assert!(Instant::now() < deadline, "hub-server açılış satırı yok");
            thread::sleep(POLL);
        }
    }

    /// `SIGKILL` (Unix'te `Child::kill`) + süreç toplanır: dönüşte süreç ölüdür, soketi kapanmıştır.
    fn kill(&mut self) {
        let mut child = self.child.take().expect("çalışan hub");
        child.kill().expect("SIGKILL");
        let status = child.wait().expect("hub toplandı");
        assert_eq!(status.signal(), Some(9), "hub SIGKILL ile öldü: {status}");
    }
}

impl Drop for HubProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct Listener(Sender<NodeEvent>);

impl NodeListener for Listener {
    fn on_event(&self, event: NodeEvent) {
        let _ = self.0.send(event);
    }
}

/// Bir düğüm ve okunmuş olayları (okunma anıyla; periyodik ölçü olayı tutulmaz).
struct Peer {
    node: KamaNode,
    rx: Receiver<NodeEvent>,
    seen: Vec<(Instant, NodeEvent)>,
}

impl Peer {
    fn pump(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            if !matches!(event, NodeEvent::Metrics(_)) {
                self.seen.push((Instant::now(), event));
            }
        }
    }

    /// `from`'dan gelen metinler: (okunma anı, `msg_id`, metin).
    fn texts_from(&self, from: &str) -> Vec<(Instant, MsgId, &str)> {
        self.seen
            .iter()
            .filter_map(|(at, e)| match e {
                NodeEvent::TextReceived {
                    peer_id,
                    msg_id,
                    text,
                } if peer_id == from => Some((*at, *msg_id, text.as_str())),
                _ => None,
            })
            .collect()
    }

    fn receipts_from(&self, from: &str) -> Vec<MsgId> {
        self.seen
            .iter()
            .filter_map(|(_, e)| match e {
                NodeEvent::Receipt { peer_id, msg_id } if peer_id == from => Some(*msg_id),
                _ => None,
            })
            .collect()
    }

    fn count(&self, pick: impl Fn(&NodeEvent) -> bool) -> usize {
        self.seen.iter().filter(|(_, e)| pick(e)).count()
    }

    /// (yenileme yöneticisinin başlattığı, ertelediği), `other` ile (etkin, yenileme, provisional).
    fn relink_state(&self, other: &str) -> ((u64, u64), (usize, usize, usize)) {
        let probe = self.node.probe_for_test().expect("sonda");
        let peer = probe
            .peers
            .iter()
            .find(|p| p.peer_id == other)
            .map_or((0, 0, 0), |p| (p.active, p.renewals, p.provisional));
        ((probe.relink.started, probe.relink.deferred), peer)
    }
}

/// Alanlar bildirim sırasıyla düşer: önce düğümler, sonra hub, en son geçici dizin.
struct Net {
    alice: Peer,
    bob: Peer,
    hub: HubProcess,
    /// Hub log satırları okunma anıyla (gözlem: yeniden bind ve yeni hat zaman çizelgesi).
    hub_lines: Vec<(Instant, u32, String)>,
    /// Geçerli kuşağın okunmuş satır sayısı.
    hub_lines_read: usize,
    root: TempDir,
}

fn start_peer(root: &Path, out: &Path, name: &str) -> Peer {
    let device = DeviceName::new(name).expect("ad");
    let config = root.join(name).join("config").join(device.file_name());
    fs::create_dir_all(config.parent().expect("üst dizin")).expect("dizin");
    fs::copy(out.join(device.file_name()), &config).expect("cihaz dosyası");
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
    }
}

impl Net {
    fn start(deadline: Instant) -> Self {
        let root = TempDir::new();
        let out = root.0.join("authority");
        let listen = format!("127.0.0.1:{}", free_udp_port());
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
                hub: Some(listen.clone()),
                direct: Default::default(),
            },
            &InitOptions { probe: false },
            &mut OsEntropy::new(),
            now,
        )
        .expect("provisioning v1");
        let mut hub = HubProcess {
            bin: hub_bin(),
            config: out.join(HUB_FILE_NAME),
            listen,
            log_dir: root.0.clone(),
            child: None,
            generation: 0,
        };
        hub.spawn(deadline);
        let alice = start_peer(&root.0, &out, ALICE);
        let bob = start_peer(&root.0, &out, BOB);
        Self {
            alice,
            bob,
            hub,
            hub_lines: Vec::new(),
            hub_lines_read: 0,
            root,
        }
    }

    fn pump(&mut self) {
        self.alice.pump();
        self.bob.pump();
        let generation = self.hub.generation;
        let log = self.hub.log(generation);
        let complete = log.rsplit_once('\n').map_or("", |(head, _)| head);
        let lines: Vec<&str> = complete.lines().collect();
        let now = Instant::now();
        for line in lines.iter().skip(self.hub_lines_read) {
            self.hub_lines.push((now, generation, (*line).to_owned()));
        }
        self.hub_lines_read = self.hub_lines_read.max(lines.len());
    }

    /// Hub kuşağını değiştirir (öldürme / yeniden başlatma): satır sayacı yeni dosyadan başlar.
    fn respawn_hub(&mut self, deadline: Instant) {
        self.pump();
        self.hub.spawn(deadline);
        self.hub_lines_read = 0;
    }

    /// `cond` doğru olana dek iki düğümün olaylarını okur; süre dolarsa hub loglarıyla kırmızı.
    fn wait_until(&mut self, what: &str, deadline: Instant, cond: impl Fn(&Self) -> bool) {
        loop {
            self.pump();
            if cond(self) {
                return;
            }
            if Instant::now() >= deadline {
                self.dump(what);
                panic!("deneme tavanı doldu: {what}");
            }
            thread::sleep(POLL);
        }
    }

    /// Kırmızıda teşhis: sondalar ve hub logları.
    fn dump(&self, what: &str) {
        eprintln!("HUB-RESTART-DUMP neden: {what}");
        for (name, peer) in [(ALICE, &self.alice), (BOB, &self.bob)] {
            match peer.node.probe_for_test() {
                Ok(p) => eprintln!(
                    "HUB-RESTART-DUMP {name}: hub={:?} relink={:?} peers={:?} events={:?} \
                     messenger={:?} outstanding_units={}",
                    p.hub, p.relink, p.peers, p.events, p.messenger, p.outstanding_units
                ),
                Err(e) => eprintln!("HUB-RESTART-DUMP {name}: sonda hatası {e}"),
            }
            let closed = peer.count(|e| matches!(e, NodeEvent::Closed { .. }));
            eprintln!("HUB-RESTART-DUMP {name}: closed_events={closed}");
        }
        for generation in 1..=self.hub.generation {
            eprintln!(
                "HUB-RESTART-DUMP hub-{generation}.log:\n{}",
                self.hub.log(generation)
            );
        }
    }

    fn shutdown(self) {
        let Self {
            alice,
            bob,
            hub,
            root,
            ..
        } = self;
        for (name, peer) in [(ALICE, alice), (BOB, bob)] {
            peer.node
                .shutdown()
                .unwrap_or_else(|e| panic!("{name} kapanış: {e}"));
        }
        drop(hub);
        drop(root);
    }
}

fn a2b(i: usize) -> String {
    format!("a2b-{i:03}|{}", "x".repeat(PAD))
}

fn b2a(i: usize) -> String {
    format!("b2a-{i:03}")
}

/// Alice'in gönderdiği `i`. metin Bob'da kaç kez teslim edildi.
fn delivered_count(bob: &Peer, text: &str) -> usize {
    bob.texts_from(ALICE)
        .iter()
        .filter(|(_, _, t)| *t == text)
        .count()
}

/// Bob'a teslim edilen farklı alice metni sayısı.
fn delivered_distinct(bob: &Peer) -> usize {
    let mut ids: Vec<MsgId> = bob.texts_from(ALICE).iter().map(|(_, id, _)| *id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.len()
}

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

#[test]
#[ignore = "HUB-RESTART: hub-server ikilisi gerekir (KAMA_HUB_SERVER_BIN); PR CI'da koşmaz"]
fn s4_hub_process_killed_mid_flight_restarts_and_sessions_recover() {
    let setup = Instant::now() + SETUP;
    let mut net = Net::start(setup);
    net.alice.node.connect(BOB).expect("connect");
    net.wait_until("iki uçta Connected", setup, |n| {
        n.alice
            .count(|e| matches!(e, NodeEvent::Connected { peer_id } if peer_id == BOB))
            == 1
            && n.bob
                .count(|e| matches!(e, NodeEvent::Connected { peer_id } if peer_id == ALICE))
                == 1
    });

    // msg_id → gönderim sırası (alice'in kalıcı `msg_id`'si; `Receipt` eşlemesi için).
    let mut sent: BTreeMap<MsgId, usize> = BTreeMap::new();
    let send = |net: &mut Net, sent: &mut BTreeMap<MsgId, usize>, i: usize| {
        let id = net
            .alice
            .node
            .send_text(BOB, &a2b(i))
            .unwrap_or_else(|e| panic!("alice send_text {i}: {e}"));
        assert!(sent.insert(id, i).is_none(), "msg_id tekil: {id}");
    };

    // Öncesi: her metin teslim beklenerek (eski hub üstünden teslim > 0).
    for i in 0..PRE {
        send(&mut net, &mut sent, i);
        let text = a2b(i);
        net.wait_until(&format!("öncesi teslim {i}"), setup, |n| {
            delivered_count(&n.bob, &text) == 1
        });
    }

    // Uçuşta: beklemeden BURST metin, ardından hemen SIGKILL.
    for i in PRE..PRE + BURST {
        send(&mut net, &mut sent, i);
    }
    let killed = Instant::now();
    net.hub.kill();
    let kill_call = killed.elapsed();
    let sent_at_kill = sent.len();
    // Hub ölü: bundan sonra eski hub üstünden teslim olamaz; kanaldakiler okunur.
    thread::sleep(KILL_DRAIN);
    net.pump();
    let delivered_at_kill = delivered_distinct(&net.bob);
    let alice_probe = net.alice.node.probe_for_test().expect("sonda");
    eprintln!(
        "HUB-RESTART-OLCUM kill: sent={sent_at_kill} delivered={delivered_at_kill} \
         in_flight={} alice_outstanding_units={} kill_call_ms={}",
        sent_at_kill - delivered_at_kill,
        alice_probe.outstanding_units,
        ms(kill_call)
    );
    // Düzenek ön koşulu (ürün iddiası değil): öldürme anında teslim > 0, < N ve en az bir ileti
    // gönderilmiş ama teslim edilmemiş.
    assert!(
        delivered_at_kill > 0 && delivered_at_kill < TOTAL,
        "ön koşul: 0 < teslim ({delivered_at_kill}) < N ({TOTAL})"
    );
    assert!(
        delivered_at_kill < sent_at_kill,
        "ön koşul: öldürme anında uçuşta ileti var (gönderilen {sent_at_kill}, teslim \
         {delivered_at_kill})"
    );

    // Sırası: hub ölüyken alice DURING metin, bob bir metin.
    let b2a_during = net
        .bob
        .node
        .send_text(ALICE, &b2a(0))
        .unwrap_or_else(|e| panic!("bob send_text (hub ölü): {e}"));
    for i in PRE + BURST..PRE + BURST + DURING {
        send(&mut net, &mut sent, i);
        thread::sleep(SEND_GAP);
        net.pump();
    }
    let left = OUTAGE.saturating_sub(killed.elapsed());
    thread::sleep(left);

    // Yeniden başlat: aynı ikili, aynı hub.json, aynı port.
    let restart_at = Instant::now();
    net.respawn_hub(restart_at + SETUP);
    let restarted = Instant::now();
    eprintln!(
        "HUB-RESTART-OLCUM restart: outage_ms={} spawn_ms={}",
        ms(restarted.duration_since(killed)),
        ms(restarted.duration_since(restart_at))
    );

    // Sonrası: AFTER metin (toparlanma beklenmeden).
    for i in PRE + BURST + DURING..TOTAL {
        send(&mut net, &mut sent, i);
        thread::sleep(SEND_GAP);
        net.pump();
    }
    assert_eq!(sent.len(), TOTAL);

    // (a)+(b)+(e): uygulama müdahalesi yok (connect yok, yeniden gönderim yok) — bütün metinler
    // teslim, bütün alındılar, bob'un hub ölüyken yolladığı metin alice'te.
    let recovery = restarted + RECOVERY;
    net.wait_until(
        "toparlanma: bütün alice→bob metinleri teslim",
        recovery,
        |n| delivered_distinct(&n.bob) == TOTAL,
    );
    let all_delivered = Instant::now();
    net.wait_until("toparlanma: bob→alice (hub ölüyken)", recovery, |n| {
        n.alice.texts_from(BOB).iter().any(|(_, _, t)| *t == b2a(0))
    });
    let b2a_delivered = Instant::now();
    net.wait_until("toparlanma: bütün alındılar alice'te", recovery, |n| {
        n.alice.receipts_from(BOB).len() >= TOTAL
    });
    let all_receipts = Instant::now();
    let first_after = net
        .bob
        .texts_from(ALICE)
        .iter()
        .map(|(at, _, _)| *at)
        .filter(|at| *at > restarted)
        .min()
        .expect("yeniden başlatmadan sonra teslim");
    eprintln!(
        "HUB-RESTART-OLCUM recovery (yeniden başlatmadan): first_delivery_ms={} \
         all_delivered_ms={} b2a_during_ms={} all_receipts_ms={} (öldürmeden: all_delivered_ms={})",
        ms(first_after.duration_since(restarted)),
        ms(all_delivered.duration_since(restarted)),
        ms(b2a_delivered.duration_since(restarted)),
        ms(all_receipts.duration_since(restarted)),
        ms(all_delivered.duration_since(killed)),
    );

    // (e) ikinci tur: bob→alice B2A_AFTER metin + alice→bob dosya (yeni oturumda).
    for i in 1..=B2A_AFTER {
        net.bob
            .node
            .send_text(ALICE, &b2a(i))
            .unwrap_or_else(|e| panic!("bob send_text {i}: {e}"));
    }
    let payload: Vec<u8> = (0..FILE_BYTES)
        .map(|i| u8::try_from(i % 251).expect("< 256"))
        .collect();
    let file_path = net.root.0.join("payload.bin");
    fs::write(&file_path, &payload).expect("dosya");
    let file = net
        .alice
        .node
        .send_file(BOB, &file_path)
        .expect("send_file");
    net.wait_until("ikinci tur: bob→alice metinler", recovery, |n| {
        (1..=B2A_AFTER).all(|i| n.alice.texts_from(BOB).iter().any(|(_, _, t)| *t == b2a(i)))
    });
    net.wait_until("ikinci tur: dosya teslim + alındı", recovery, |n| {
        n.bob
            .count(|e| matches!(e, NodeEvent::FileReceived { peer_id, .. } if peer_id == ALICE))
            == 1
            && n.alice.count(|e| {
                matches!(e, NodeEvent::FileProgress { file: f, stage: FileStage::Delivered, .. }
                    if *f == file)
            }) == 1
    });
    let received_path = net
        .bob
        .seen
        .iter()
        .find_map(|(_, e)| match e {
            NodeEvent::FileReceived { path, .. } => Some(path.clone()),
            _ => None,
        })
        .expect("dosya yolu");
    assert_eq!(
        fs::read(&received_path).expect("alınan dosya"),
        payload,
        "dosya bayt-özdeş"
    );

    // (c) iki uçta tek etkin oturum (S1 KARAR-2), bekleyen yenileme ve onaysız provisional yok
    // (beyanlı varsayım, #180 sözleşmesi; yenileme eşzamansız biter: koşul döngüsü).
    net.wait_until(
        "iki uçta (etkin 1, yenileme 0, provisional 0)",
        recovery,
        |n| n.alice.relink_state(BOB).1 == (1, 0, 0) && n.bob.relink_state(ALICE).1 == (1, 0, 0),
    );
    let settled = Instant::now();
    net.pump();

    // (b) tam bir kez: her alice metni Bob'da TAM BİR KEZ, fazladan teslim yok; her `msg_id`'nin
    // alındısı alice'te tam bir kez.
    for i in 0..TOTAL {
        assert_eq!(
            delivered_count(&net.bob, &a2b(i)),
            1,
            "a2b-{i:03} tam bir kez"
        );
    }
    let bob_texts = net.bob.texts_from(ALICE);
    assert_eq!(bob_texts.len(), TOTAL, "bob'da fazladan TextReceived yok");
    for (_, id, text) in &bob_texts {
        let i = *sent
            .get(id)
            .unwrap_or_else(|| panic!("bilinmeyen msg_id {id}"));
        assert_eq!(*text, a2b(i), "msg_id {id} ↔ metin");
    }
    let mut receipts: HashMap<MsgId, usize> = HashMap::new();
    for id in net.alice.receipts_from(BOB) {
        *receipts.entry(id).or_default() += 1;
    }
    assert_eq!(receipts.len(), TOTAL, "her msg_id için alındı");
    assert!(
        sent.keys().all(|id| receipts.get(id) == Some(&1)),
        "Receipt (eş, msg_id) başına tam bir kez: {receipts:?}"
    );
    // (e) bob→alice: hub ölüyken yollanan + ikinci tur, her biri tam bir kez; bob'da alındı.
    let alice_texts = net.alice.texts_from(BOB);
    for i in 0..=B2A_AFTER {
        let n = alice_texts.iter().filter(|(_, _, t)| *t == b2a(i)).count();
        assert_eq!(n, 1, "b2a-{i:03} tam bir kez");
    }
    assert_eq!(
        alice_texts.len(),
        B2A_AFTER + 1,
        "alice'te fazladan metin yok"
    );
    assert!(
        net.bob.receipts_from(ALICE).contains(&b2a_during),
        "bob hub ölüyken yolladığı metnin alındısını aldı"
    );

    // (a) iki uçta Closed yok; Connected bir kez (oturum kaybı Application'a görünmedi, düğüm
    // yeniden `connect` gerektirmedi).
    for (name, me, other) in [(ALICE, &net.alice, BOB), (BOB, &net.bob, ALICE)] {
        let closed =
            me.count(|e| matches!(e, NodeEvent::Closed { peer_id, .. } if peer_id == other));
        let connected =
            me.count(|e| matches!(e, NodeEvent::Connected { peer_id } if peer_id == other));
        assert_eq!((closed, connected), (0, 1), "{name}: (Closed, Connected)");
    }

    // Gözlem (iddia değil): sıra tersinmeleri, yeniden gönderim ve yenileme sayaçları.
    let order: Vec<usize> = bob_texts
        .iter()
        .map(|(_, id, _)| *sent.get(id).expect("msg_id"))
        .collect();
    let inversions = order.windows(2).filter(|w| w[1] < w[0]).count();
    let alice_probe = net.alice.node.probe_for_test().expect("sonda");
    let bob_probe = net.bob.node.probe_for_test().expect("sonda");
    eprintln!(
        "HUB-RESTART-OLCUM settled_ms={} order_inversions={inversions} \
         alice_resent_on_new_session={} alice_relink={:?} bob_relink={:?} \
         alice_events={:?} bob_events={:?}",
        ms(settled.duration_since(restarted)),
        alice_probe.messenger.resent_on_new_session,
        alice_probe.relink,
        bob_probe.relink,
        alice_probe.events,
        bob_probe.events,
    );
    eprintln!(
        "HUB-RESTART-OLCUM drops: alice ctx_mismatch={} delivery_channel={} not_ready={} | bob \
         ctx_mismatch={} delivery_channel={} not_ready={}",
        alice_probe.ctx_mismatch_drops,
        alice_probe.delivery_channel_drops,
        alice_probe.not_ready_drops,
        bob_probe.ctx_mismatch_drops,
        bob_probe.delivery_channel_drops,
        bob_probe.not_ready_drops,
    );
    eprintln!(
        "HUB-RESTART-OLCUM hub: alice={:?} bob={:?}",
        alice_probe.hub, bob_probe.hub
    );
    for (at, generation, line) in net.hub_lines.iter().filter(|(_, g, _)| *g > 1) {
        eprintln!(
            "HUB-RESTART-OLCUM hub-{generation} +{}ms {line}",
            ms(at.saturating_duration_since(restarted))
        );
    }
    net.shutdown();
}
