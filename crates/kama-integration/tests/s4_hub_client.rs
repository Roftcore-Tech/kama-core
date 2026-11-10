//! S4 (KM-SEAM-TH-001 v1.3.1) — gerçek hub istemcisi (`client_core::hub::HubLink`) ↔ gerçek hub
//! çekirdeği (`hub_server::Hub`). HB1-M3; KK-H45.
//!
//! İki `HubLink` + bir `Hub`, sahte saat, sabit rastgelelik, tek süreç, soketsiz. Aradaki her datagram
//! gerçek tel baytıdır: istemci çerçevesi PoC no-op mühürle (`frame ‖ 0×16`, S4 §2.1) hub'a, hub'ın
//! çıkışı açılarak (`son 16 bayt sıfır`) istemciye. Pompa seçili hub→istemci datagramlarını düşürerek
//! S4 §7 K1, K2, K3'ü üretir.
//!
//! Kapsam: bu test **duman değildir** (D-14 eki "Mock yeşili dumanın yerine geçmez"). Transport ve
//! kripto oturumu burada yoktur: sahte transport `EnvelopeLink` olaylarını S1'in yapacağı gibi tüketir
//! (`IncomingInit` → `send_response`, `Response` → `peer_ready`). Zarf baytları opak sabitlerdir.
//! "İki yönde veri" hub düzeyindedir: `[h][yük]` datagramı hub'dan bayt bayt karşı uca geçer.
//! Uçtan uca TEXT gidiş-dönüşü HB2'nin `s4_hub_smoke`'udur.
//!
//! Cihaz imzası `ed25519-dalek` ile (Cargo.lock'ta mevcut; yalnız dev-dep). Anahtarlar test sabitidir.

use std::cell::Cell;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::rc::Rc;
use std::time::{Duration, Instant};

use client_core::hub::{BindSigner, HubConfig, HubLink, HubNotice};
use client_core::transport::LinkEvent;
use ed25519_dalek::{Signer, SigningKey};
use hub_server::{Clock, Hub, Sink};
use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;
use kama_noise_wire::entropy::{EntropyError, EntropySource};
use kama_noise_wire::handshake::cert_ref_of;
use kama_provision::cert::{self, CertFields};
use kama_provision::encoding::{base64_encode, hex_encode};

// ── sabitler ve yardımcılar ──────────────────────────────────────────────────────────────────

const HUB_SEED: [u8; 32] = [0x11; 32];
const ROOT_SEED: [u8; 32] = [0x12; 32];
const INIT_BYTES: &[u8] = b"opaque SSI1 envelope (test)";
const RESP_BYTES: &[u8] = b"opaque SSR1 envelope (test)";

fn hub_addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::new(10, 0, 0, 100), 7000))
}

/// Sayaç tabanlı deterministik entropi (her `fill` farklı, tekrarlanabilir).
struct CounterEntropy(u64);
impl EntropySource for CounterEntropy {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        for b in dst.iter_mut() {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *b = (self.0 >> 56) as u8;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct SharedClock(Rc<Cell<Duration>>);
impl Clock for SharedClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}

struct DalekSigner(SigningKey);
impl BindSigner for DalekSigner {
    fn sign_bind(&self, tbs: &[u8; 112]) -> [u8; 64] {
        self.0.sign(tbs).to_bytes()
    }
}

#[derive(Default)]
struct VecSink(Vec<(SocketAddr, Vec<u8>)>);
impl Sink for VecSink {
    fn send(&mut self, to: SocketAddr, datagram: &[u8]) {
        self.0.push((to, datagram.to_vec()));
    }
    fn log(&mut self, _line: fmt::Arguments<'_>) {}
}

struct Device {
    name: &'static str,
    signing: SigningKey,
    cert: [u8; 152],
    dest_hash: [u8; 32],
    addr: SocketAddr,
}

fn device(name: &'static str, seed: u8, ip_last: u8) -> Device {
    let root = SigningKey::from_bytes(&ROOT_SEED);
    let signing = SigningKey::from_bytes(&[seed; 32]);
    let identity = SigningKey::from_bytes(&[seed ^ 0x80; 32]);
    let fields = CertFields {
        device_ed25519_pub: signing.verifying_key().to_bytes(),
        device_x25519_pub: [seed; 32],
        issuer_key_id: cert::issuer_key_id(&root.verifying_key().to_bytes()),
        not_after: 4_102_444_800, // 2100-01-01
    };
    let cert = cert::sign_compact_device_cert(&fields, &root).expect("sertifika");
    let dest_hash = cert::dest_hash(&cert::identity_hash(&identity.verifying_key().to_bytes()));
    Device {
        name,
        signing,
        cert,
        dest_hash,
        addr: SocketAddr::from((Ipv4Addr::new(10, 0, 0, ip_last), 5000)),
    }
}

fn hub_json(devices: &[&Device]) -> String {
    let hub = SigningKey::from_bytes(&HUB_SEED);
    let devs: Vec<String> = devices
        .iter()
        .map(|d| {
            format!(
                r#"{{"name":"{}","dest_hash":"{}","cert":"{}"}}"#,
                d.name,
                hex_encode(&d.dest_hash),
                base64_encode(&d.cert)
            )
        })
        .collect();
    format!(
        r#"{{"format_version":1,"listen":"{}","hub_ed25519":{{"priv":"{}","pub":"{}"}},"devices":[{}]}}"#,
        hub_addr(),
        hex_encode(&HUB_SEED),
        hex_encode(&hub.verifying_key().to_bytes()),
        devs.join(",")
    )
}

/// Hub→istemci datagramının sınıfı (düşürme seçimi için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    StoreResp,
    DeliverInit,
    DeliverResponse,
    Other,
}

fn kind(dg: &[u8]) -> Kind {
    // [0;4] 0x1D method flags rid(4) len(4) payload … [0;16]
    if dg.len() < 21 || dg[..4] != [0; 4] || dg[4] != 0x1D {
        return Kind::Other;
    }
    match (dg[5], dg[6]) {
        (0x05, 0x01) => Kind::StoreResp,
        (0x81, 0x00) => match dg.get(15 + 4 + 32) {
            Some(0x01) => Kind::DeliverInit,
            Some(0x02) => Kind::DeliverResponse,
            _ => Kind::Other,
        },
        _ => Kind::Other,
    }
}

/// Sahte transport'un bir uçtaki görüşü.
#[derive(Default)]
struct Side {
    assigned: Option<HubHandle>,
    incoming: Vec<HubHandle>,
    responses: Vec<HubHandle>,
    ready: Option<HubHandle>,
    notices: Vec<HubNotice>,
    data_rx: Vec<Vec<u8>>,
}

struct Net {
    t0: Instant,
    now: Duration,
    clock: Rc<Cell<Duration>>,
    hub: Hub<SharedClock, CounterEntropy>,
    alice: HubLink,
    bob: HubLink,
    alice_dev: Device,
    bob_dev: Device,
    a: Side,
    b: Side,
    /// (tür, kaç tanesi daha düşürülecek, alıcı adres)
    drops: Vec<(Kind, u32, SocketAddr)>,
    dropped: Vec<Kind>,
}

fn link_for(d: &Device, entropy_seed: u64, t0: Instant) -> HubLink {
    let cfg = HubConfig {
        hub_addr: hub_addr(),
        hub_ed25519_pub: SigningKey::from_bytes(&HUB_SEED).verifying_key().to_bytes(),
        device_cert_ref: cert_ref_of(&d.cert),
        self_dest_hash: d.dest_hash,
    };
    HubLink::new(
        cfg,
        Box::new(DalekSigner(d.signing.clone())),
        Box::new(CounterEntropy(entropy_seed)),
        t0,
    )
    .expect("HubLink")
}

impl Net {
    fn new() -> Self {
        let alice_dev = device("alice", 0x21, 1);
        let bob_dev = device("bob", 0x22, 2);
        let config =
            hub_server::config::parse(&hub_json(&[&alice_dev, &bob_dev])).expect("hub.json");
        let clock = Rc::new(Cell::new(Duration::ZERO));
        let hub =
            Hub::new(config, SharedClock(clock.clone()), CounterEntropy(0xABCD)).expect("hub");
        let t0 = Instant::now();
        Self {
            t0,
            now: Duration::ZERO,
            clock,
            hub,
            alice: link_for(&alice_dev, 1, t0),
            bob: link_for(&bob_dev, 2, t0),
            alice_dev,
            bob_dev,
            a: Side::default(),
            b: Side::default(),
            drops: Vec::new(),
            dropped: Vec::new(),
        }
    }

    fn peer_alice(&self) -> PeerRef {
        PeerRef::from_dest_hash(self.alice_dev.dest_hash)
    }

    fn peer_bob(&self) -> PeerRef {
        PeerRef::from_dest_hash(self.bob_dev.dest_hash)
    }

    fn instant(&self) -> Instant {
        self.t0 + self.now
    }

    fn drop_next(&mut self, k: Kind, to: SocketAddr) {
        self.drops.push((k, 1, to));
    }

    /// Bu anda sessizleşene dek: istemci çıkışı → hub → istemci girişi → sahte transport.
    fn settle(&mut self) {
        let now = self.instant();
        for _ in 0..64 {
            let mut moved = false;
            let mut sink = VecSink::default();
            for (link, addr) in [
                (&mut self.alice, self.alice_dev.addr),
                (&mut self.bob, self.bob_dev.addr),
            ] {
                while let Some(frame) = link.poll_transmit(now) {
                    assert!(frame.len() + 16 <= 1280, "çerçeve + 16 B ≤ MAX_DATAGRAM");
                    let mut dg = frame;
                    dg.extend_from_slice(&[0; 16]); // PoC no-op seal_hub
                    self.hub.on_datagram(addr, &dg, &mut sink);
                    moved = true;
                }
            }
            for (to, dg) in sink.0 {
                moved = true;
                let k = kind(&dg);
                if let Some(d) = self
                    .drops
                    .iter_mut()
                    .find(|d| d.0 == k && d.1 > 0 && d.2 == to)
                {
                    d.1 -= 1;
                    self.dropped.push(k);
                    continue;
                }
                let (link, side) = if to == self.alice_dev.addr {
                    (&mut self.alice, &mut self.a)
                } else if to == self.bob_dev.addr {
                    (&mut self.bob, &mut self.b)
                } else {
                    panic!("hub bilinmeyen adrese yolladı");
                };
                if dg[..4] == [0; 4] {
                    // open_hub: son 16 bayt sıfır → açılmış çerçeve.
                    assert!(dg[dg.len() - 16..].iter().all(|&b| b == 0));
                    link.on_control(now, &dg[..dg.len() - 16]);
                } else {
                    let h = HubHandle::from_raw(u32::from_be_bytes(dg[..4].try_into().unwrap()));
                    link.on_session_traffic(now, h, false);
                    side.data_rx.push(dg);
                }
            }
            moved |= self.transport_alice(now);
            moved |= self.transport_bob(now);
            if !moved {
                return;
            }
        }
        panic!("pompa sessizleşmedi");
    }

    /// Başlatan sahte transport: HandleAssigned → atama; Response → (doğrulanmış kabul) → Ready.
    fn transport_alice(&mut self, _now: Instant) -> bool {
        let mut moved = false;
        while let Some(ev) = self.alice.poll_event() {
            moved = true;
            match ev {
                LinkEvent::HandleAssigned { hub_handle, .. } => {
                    assert!(self.a.assigned.is_none(), "tek HandleAssigned");
                    self.a.assigned = Some(hub_handle);
                }
                LinkEvent::Response {
                    hub_handle, bytes, ..
                } => {
                    assert_eq!(bytes, RESP_BYTES);
                    assert_eq!(
                        Some(hub_handle),
                        self.a.assigned,
                        "RESPONSE atanmış h'de (K3 sırası)"
                    );
                    self.a.responses.push(hub_handle);
                    self.a.ready = Some(hub_handle);
                    let peer = self.peer_bob();
                    self.alice.peer_ready(peer, hub_handle);
                }
                LinkEvent::IncomingInit { .. } => panic!("alice INIT almaz"),
            }
        }
        while let Some(n) = self.alice.take_notice() {
            self.a.notices.push(n);
        }
        moved
    }

    /// Yanıtlayan sahte transport: IncomingInit → RESPONSE.
    fn transport_bob(&mut self, now: Instant) -> bool {
        let mut moved = false;
        while let Some(ev) = self.bob.poll_event() {
            moved = true;
            match ev {
                LinkEvent::IncomingInit {
                    hub_handle, bytes, ..
                } => {
                    assert_eq!(bytes, INIT_BYTES);
                    self.b.incoming.push(hub_handle);
                    let peer = self.peer_alice();
                    self.bob
                        .send_response(now, peer, hub_handle, 0x02, RESP_BYTES.to_vec())
                        .expect("send_response");
                    self.b.ready = Some(hub_handle);
                }
                other => panic!("bob beklenmeyen olay: {other:?}"),
            }
        }
        while let Some(n) = self.bob.take_notice() {
            self.b.notices.push(n);
        }
        moved
    }

    /// Saati `dt` adımlarla `total` kadar ilerletir; her adımda hub tick + settle.
    fn run(&mut self, total: Duration, dt: Duration) {
        let end = self.now + total;
        while self.now < end {
            self.now += dt;
            self.clock.set(self.now);
            let mut sink = VecSink::default();
            self.hub.on_tick(&mut sink);
            assert!(sink.0.is_empty(), "hub on_tick datagram yollamaz");
            self.settle();
        }
    }

    fn bind_both(&mut self) {
        self.settle();
        assert!(
            self.alice.is_bound() && self.bob.is_bound(),
            "iki uç bağlandı"
        );
        assert_eq!(
            self.a.notices.drain(..).collect::<Vec<_>>(),
            vec![HubNotice::Bound]
        );
        assert_eq!(
            self.b.notices.drain(..).collect::<Vec<_>>(),
            vec![HubNotice::Bound]
        );
    }

    fn start_init(&mut self) {
        let now = self.instant();
        let peer = self.peer_bob();
        self.alice
            .send_init(now, peer, 0x01, INIT_BYTES.to_vec())
            .expect("send_init");
    }

    /// Başarı ölçütü (S4 C4): iki uçta aynı h, açılış tamam, iki yönde hub üstünden veri.
    fn assert_established(&mut self) {
        let h = self.a.assigned.expect("alice h");
        assert_eq!(self.b.incoming, vec![h], "bob tek IncomingInit, aynı h");
        assert_eq!(self.a.responses, vec![h], "alice tek Response");
        assert_eq!(self.a.ready, Some(h));
        assert_eq!(self.b.ready, Some(h));
        // T3 durdu: başka INIT STORE'u gitmez.
        let sent = self.alice.counters().init_stores_sent;
        self.run(Duration::from_secs(40), Duration::from_millis(500));
        assert_eq!(
            self.alice.counters().init_stores_sent,
            sent,
            "peer_ready T3'ü durdurdu"
        );
        // İki yönde veri: [h][yük] hub'dan bayt bayt geçer.
        let mut sink = VecSink::default();
        let mut a2b = h.raw().to_be_bytes().to_vec();
        a2b.extend_from_slice(b"alice->bob");
        self.hub.on_datagram(self.alice_dev.addr, &a2b, &mut sink);
        let mut b2a = h.raw().to_be_bytes().to_vec();
        b2a.extend_from_slice(b"bob->alice");
        self.hub.on_datagram(self.bob_dev.addr, &b2a, &mut sink);
        assert_eq!(
            sink.0,
            vec![(self.bob_dev.addr, a2b), (self.alice_dev.addr, b2a)]
        );
    }
}

// ── testler ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn s4_both_bind_and_heartbeat_ack() {
    let mut n = Net::new();
    n.bind_both();
    let acks0 = n.alice.counters().heartbeat_acks;
    // 8–10 s sessizlik → heartbeat → gerçek hub ACK'i.
    n.run(Duration::from_secs(11), Duration::from_millis(250));
    assert!(n.alice.counters().heartbeats_sent >= 1);
    assert_eq!(
        n.alice.counters().heartbeat_acks,
        acks0 + n.alice.counters().heartbeats_sent
    );
    assert!(n.bob.counters().heartbeat_acks >= 1);
    assert!(n.alice.is_bound() && n.bob.is_bound());
    assert!(n.alice.last_rx_age(n.instant()).unwrap() < Duration::from_secs(3));
    // Bir saat sessizlik (pasif) + cevaplanan heartbeat'ler: bağ yerinde.
    n.run(Duration::from_secs(3600), Duration::from_secs(1));
    assert!(n.alice.is_bound() && n.bob.is_bound());
    assert_eq!(n.alice.counters().disconnects, 0);
}

#[test]
fn s4_init_deliver_response_deliver_same_handle() {
    let mut n = Net::new();
    n.bind_both();
    n.start_init();
    n.settle();
    assert_established_now(&mut n);
}

fn assert_established_now(n: &mut Net) {
    assert!(n.a.assigned.is_some(), "StoreResp{{OK,h}} → HandleAssigned");
    n.assert_established();
    assert!(n.a.notices.is_empty() && n.b.notices.is_empty());
}

#[test]
fn s4_k1_first_deliver_init_lost() {
    let mut n = Net::new();
    n.bind_both();
    let bob = n.bob_dev.addr;
    n.drop_next(Kind::DeliverInit, bob);
    n.start_init();
    n.settle();
    assert_eq!(n.dropped, vec![Kind::DeliverInit]);
    assert!(n.a.assigned.is_some(), "StoreResp ulaştı");
    assert!(n.b.incoming.is_empty());
    // T3 (1 s) → hub aynı anahtar → aynı h ile yeniden DELIVER.
    n.run(Duration::from_millis(1100), Duration::from_millis(100));
    assert_established_now(&mut n);
}

#[test]
fn s4_k2_first_deliver_response_lost() {
    let mut n = Net::new();
    n.bind_both();
    let alice = n.alice_dev.addr;
    n.drop_next(Kind::DeliverResponse, alice);
    n.start_init();
    n.settle();
    assert_eq!(n.dropped, vec![Kind::DeliverResponse]);
    assert_eq!(n.b.incoming.len(), 1);
    assert!(n.a.responses.is_empty());
    // Alice T3 ile INIT'i yineler (StoreResp durdurmamıştır) → bob aynı INIT için saklı
    // RESPONSE'u yeniden gönderir (yeni oturum açmaz).
    n.run(Duration::from_millis(1100), Duration::from_millis(100));
    assert_eq!(n.bob.counters().init_duplicate_resend, 1);
    assert_established_now(&mut n);
}

#[test]
fn s4_k3_store_resp_lost_response_first() {
    let mut n = Net::new();
    n.bind_both();
    let alice = n.alice_dev.addr;
    n.drop_next(Kind::StoreResp, alice);
    n.start_init();
    n.settle();
    assert_eq!(n.dropped, vec![Kind::StoreResp]);
    assert!(n.a.assigned.is_none());
    assert_eq!(
        n.alice.counters().response_held,
        1,
        "RESPONSE tutuldu (KK-H37)"
    );
    // T3 → aynı anahtar → hub aktif hatta aynı h → HandleAssigned, ardından tutulan Response.
    n.run(Duration::from_millis(1100), Duration::from_millis(100));
    assert_established_now(&mut n);
}

#[test]
fn s4_route_miss_after_unknown_handle_data() {
    let mut n = Net::new();
    n.bind_both();
    n.start_init();
    n.settle();
    let h = n.a.assigned.unwrap();
    // Hub'ın bilmediği bir handle'a veri → bağlı kaynağa ROUTE_MISS → HubLink bilinen h değilse sayar.
    let mut sink = VecSink::default();
    let bogus = h.raw().wrapping_add(1).max(1);
    let mut dg = bogus.to_be_bytes().to_vec();
    dg.extend_from_slice(b"x");
    n.hub.on_datagram(n.alice_dev.addr, &dg, &mut sink);
    assert_eq!(sink.0.len(), 1);
    let (to, rm) = sink.0.remove(0);
    assert_eq!(to, n.alice_dev.addr);
    n.alice.on_control(n.instant(), &rm[..rm.len() - 16]);
    assert_eq!(n.alice.counters().route_miss_unknown, 1);
    assert!(n.alice.take_notice().is_none());
}

#[test]
fn s4_hub_restart_rebinds_and_ends_open_attempt() {
    let mut n = Net::new();
    n.bind_both();
    // Bob yokmuş gibi: alice INIT'i gönderir, hub PEER_UNBOUND değil (bob bağlı) → kurulmadan önce
    // hub yeniden başlar (boş tablolar): heartbeat cevapsız → koptu → yeniden bind → açılış biter.
    let bob = n.bob_dev.addr;
    n.drops.push((Kind::DeliverInit, u32::MAX, bob));
    n.start_init();
    n.settle();
    let config = hub_server::config::parse(&hub_json(&[&n.alice_dev, &n.bob_dev])).unwrap();
    n.hub = Hub::new(config, SharedClock(n.clock.clone()), CounterEntropy(0x5555)).unwrap();
    // T3 tekrarları sessizliği bozar; heartbeat ilk ≥ 8 s boşlukta (16 s T3 aralığı) gider.
    n.run(Duration::from_secs(60), Duration::from_millis(250));
    assert!(n.alice.counters().disconnects >= 1);
    assert!(n.alice.is_bound(), "yeniden bind");
    assert!(n.a.notices.contains(&HubNotice::Disconnected));
    assert!(n
        .a
        .notices
        .contains(&HubNotice::AttemptEnded { peer: n.peer_bob() }));
}
