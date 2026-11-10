//! [YOL3-T1] T-L7 — yenileme ve ince şeritler (KK-T1-06 (iv), KK-T1-18; gösterim; kapılı PoC
//! ölçümü Aralık; ARQ yolu, FEC Aralık'ta).
//!
//! DÜZENEK `tests/session_keyed_slots.rs`'in kırpılmış kopyasıdır: Alice = gerçek `ClientTransport`
//! (şeritler açık, `runtime.fec = None`, Crypto saati testin elinde), Bob = ham eş (Crypto'yu test
//! sürer; veri yolu YOK → Alice'in dosya birimleri hiç teyit almaz, FILE penceresi dolu kalır,
//! şeritte birim bekler). Zarflar `MemoryHub` üzerinden, röle elle sürülür.
//!
//! İddialar sayaçlara ve bir ALT sınıra dayanır (zaman üst sınırları yalnız beklemenin tavanıdır):
//! şerit boşalmadığı için kesinti `renew_drain_cap`'ten önce OLAMAZ.

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::time::{Duration, Instant};

use client_core::lanes::{Lane, LaneConfig, DEFAULT_FILE_WINDOW_UNITS};
use client_core::memory_link::{MemoryHub, MemoryLink};
use client_core::transport::{
    ClientTransport, ClientTransportConfig, EnvelopeLink, LinkAttempt, LinkEvent,
};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent,
    TrafficClass,
};
use kama_contracts::s3::{SessionCommand, SessionControl, SessionResult};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::{EnvelopeKind, SESSION_MAX_AGE};

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

/// Beklenen olayın üst sınırı (gerçek zaman; loopback).
const WAIT: Duration = Duration::from_secs(10);
/// Yoklama adımı.
const TICK: Duration = Duration::from_millis(2);
/// Testin yenileme boşaltma tavanı: kesintinin ALT sınırı (şerit hiç boşalmaz).
const RENEW_DRAIN_CAP: Duration = Duration::from_millis(300);

type Node = ClientTransport<NoiseWireProtection>;

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn node(me: u8) -> (NoiseWireProtection, ManualClock) {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy)
}

/// Alice: ürün builder'ı, şeritler açık (varsayılan W), FEC kapalı, yenileme tavanı kısaltılmış.
fn alice_node(hub: &MemoryHub) -> (Node, ManualClock) {
    let (crypto, clock) = node(ALICE);
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    cfg.renew_drain_cap = RENEW_DRAIN_CAP;
    cfg.lanes = Some(LaneConfig::default());
    let t = ClientTransport::new(cfg, crypto, Box::new(hub.link(id(ALICE)))).expect("düğüm");
    (t, clock)
}

/// HAM karşı uç: yalnız zarf yolu (INIT/RESPONSE); veri soketi okunmaz (teyit yok).
struct RawPeer {
    crypto: NoiseWireProtection,
    link: MemoryLink,
    sink: StdUdpSocket,
    attempts: BTreeMap<LinkAttempt, SessionHandle>,
}

impl RawPeer {
    fn new(me: u8, hub: &MemoryHub) -> Self {
        Self {
            crypto: node(me).0,
            link: hub.link(id(me)),
            sink: StdUdpSocket::bind("127.0.0.1:0").expect("UDP soketi"),
            attempts: BTreeMap::new(),
        }
    }

    fn sink_addr(&self) -> SocketAddr {
        self.sink.local_addr().expect("adres")
    }

    /// Gelen kutusunu S1 adaptörünün zarf yoluyla AYNI komutlarla işler; INIT'i yanıtlanan
    /// oturumları sırayla döner (`session_keyed_slots.rs` `RawPeer::serve`).
    fn serve(&mut self) -> Vec<SessionHandle> {
        let mut out = Vec::new();
        while let Some(ev) = self.link.poll() {
            match ev {
                LinkEvent::HandleAssigned {
                    attempt,
                    hub_handle,
                } => {
                    if let Some(session) = self.attempts.remove(&attempt) {
                        let _ = self.crypto.command(SessionCommand::AssignHandle {
                            session,
                            hub_handle,
                        });
                    }
                }
                LinkEvent::IncomingInit {
                    hub_handle, bytes, ..
                } => {
                    let (session, peer, envelope_type, reply) = match self
                        .crypto
                        .command(SessionCommand::AcceptInit { wire: bytes })
                    {
                        SessionResult::EstablishedAndReply {
                            session,
                            peer,
                            envelope_type,
                            bytes,
                        } => (session, peer, envelope_type, bytes),
                        SessionResult::ProvisionalAndReply {
                            new,
                            peer,
                            envelope_type,
                            bytes,
                            ..
                        } => (new, peer, envelope_type, bytes),
                        other => panic!("INIT kabul edilmeli: {other:?}"),
                    };
                    assert_eq!(
                        self.crypto.command(SessionCommand::AssignHandle {
                            session,
                            hub_handle,
                        }),
                        SessionResult::NoOutput
                    );
                    self.link
                        .send_response(peer, hub_handle, envelope_type, reply)
                        .expect("zarf taşıyıcısı");
                    out.push(session);
                }
                LinkEvent::Response { bytes, .. } => {
                    if let SessionResult::Established { session } = self
                        .crypto
                        .command(SessionCommand::AcceptResponse { wire: bytes })
                    {
                        out.push(session);
                    }
                }
            }
        }
        while self.crypto.poll_event().is_some() {}
        out
    }
}

fn next_event(t: &mut Node) -> SessionEvent {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(ev) = t.poll_event() {
            return ev;
        }
        assert!(Instant::now() < deadline, "S1 olayı gelmedi");
        std::thread::sleep(TICK);
    }
}

fn ready(t: &mut Node, peer: PeerRef) -> SessionHandle {
    loop {
        if let SessionEvent::Ready(h, p) = next_event(t) {
            if p == peer {
                return h;
            }
        }
    }
}

fn file_unit(n: u16) -> PreparedUnit {
    let mut bytes = vec![0u8; 200];
    bytes[0] = b'F';
    bytes[1..3].copy_from_slice(&n.to_be_bytes());
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

fn file_index() -> usize {
    Lane::ALL
        .iter()
        .position(|&lane| lane == Lane::File)
        .expect("FILE şeridi")
}

/// T-L7 — yenileme: şeritte birim varken yenilemenin RESPONSE'u tutulur ve kesinti en erken
/// `renew_drain_cap` sonra olur (şerit hiç boşalmaz: teyit yok); kesintide eski oturumun şeridinde
/// bekleyen birimler yeni oturuma TAŞINMAZ, `lane_dropped`'ta sayılır (KK-T1-18, S1 K7); yeni
/// oturumun şeridi boş başlar ve yeni birim yeni oturuma verilir.
#[test]
fn t_l7_renewal_waits_for_the_lane_and_carries_no_unit_to_the_new_session() {
    /// W (5) uca + 7 şeritte; FILE tavanı 2·W = 10 içinde.
    const FILE_UNITS: u16 = 12;
    let window = usize::try_from(DEFAULT_FILE_WINDOW_UNITS).expect("küçük");
    let queued = usize::from(FILE_UNITS) - window;
    let hub = MemoryHub::new();
    let (mut alice, clock) = alice_node(&hub);
    let mut bob = RawPeer::new(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.sink_addr());

    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    assert_eq!(bob.serve().len(), 1, "Bob INIT'i yanıtlar");
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a0 = ready(&mut alice, id(BOB));

    for n in 0..FILE_UNITS {
        alice
            .send(a0, file_unit(n), DeliveryPolicy::Reliable, TrafficClass::Bk)
            .expect("şerit kabul eder");
    }
    assert_eq!(
        alice.lane_backlog(a0),
        Some((queued, window)),
        "W birim uçta (teyitsiz), kalanı şeritte"
    );

    // Yenileme (Alice başlatır): Crypto `RenewRequired(a0)` → INIT → Bob provisional → RESPONSE.
    clock.advance(SESSION_MAX_AGE);
    assert_eq!(next_event(&mut alice), SessionEvent::RenewRequired(a0));
    assert_eq!(hub.relay(), 1, "yenileme INIT'i → Bob");
    assert_eq!(bob.serve().len(), 1, "Bob yenilemeyi yanıtlar");
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let response_relayed = Instant::now();
    let a1 = ready(&mut alice, id(BOB));
    let held = response_relayed.elapsed();
    assert_ne!(a1, a0);
    assert!(
        held >= RENEW_DRAIN_CAP,
        "şerit boşalmadan kesinti yok: {held:?} < {RENEW_DRAIN_CAP:?}"
    );

    // Kesinti: eski oturumun şeridi bırakılır, birimleri sayılır; hiçbiri yeni oturuma verilmez.
    let deadline = Instant::now() + WAIT;
    while alice.lane_stats().expect("şeritler açık").lane_dropped == 0 {
        assert!(Instant::now() < deadline, "eski şerit bırakılmadı");
        let _ = alice.poll_event();
        std::thread::sleep(TICK);
    }
    let stats = alice.lane_stats().expect("şeritler açık");
    let file = file_index();
    assert_eq!(
        stats.lane_dropped, queued as u64,
        "şeritteki birimler sayılarak düştü"
    );
    assert_eq!(stats.accepted[file], u64::from(FILE_UNITS));
    assert_eq!(
        stats.given[file], window as u64,
        "yeni oturuma şeritten birim verilmedi"
    );
    assert_eq!(
        alice.lane_backlog(a0),
        Some((0, 0)),
        "eski oturumun şerit durumu yok"
    );
    assert_eq!(
        alice.lane_backlog(a1),
        Some((0, 0)),
        "yeni oturumun şeridi boş başlar"
    );

    // Yeni birim yeni oturuma verilir.
    alice
        .send(
            a1,
            file_unit(FILE_UNITS),
            DeliveryPolicy::Reliable,
            TrafficClass::Bk,
        )
        .expect("yeni oturumda kabul");
    let stats = alice.lane_stats().expect("şeritler açık");
    assert_eq!(stats.given[file], window as u64 + 1);
    assert_eq!(alice.lane_backlog(a1), Some((0, 1)));
}
