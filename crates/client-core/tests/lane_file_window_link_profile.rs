//! [ASİMETRİK-YOL · ŞERİT-W] Yön hızları açıkken dosya şeridinin penceresi göndericinin profil
//! penceresini doldurur (`lanes::file_window_floor`, `runtime::link_profile_window_bytes`).
//!
//! Sorun: şerit katmanı FILE penceresini `BtlBw × RTprop`'tan kurar (taban 5 birim). Yön hızları
//! verilince alıcı ACK'leri dar dönüşün bütçesine göre aralıklar (62 B/s dönüşte ≈ 5,4 s);
//! teyitsiz birim pencerede bir aralık kalır, dosya `5 · birim / aralık` hızına iner. Gönderici
//! bu aralığı ilk penceresine (≈ 67 kB) zaten katmıştır; şerit penceresi onu dolduramıyordu.
//! Ölçüm (S1 yolu, aşağıdaki hücre, 40 × 900 B, tohum 1–3): düzeltmeden önce alıcıda son teslim
//! 26,9–64,8 s, dosya şeridinin uçuşu en çok 6–11 birim; şeritsiz aynı hücre 13,4–16,8 s.
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması, şeritler açık), zarflar `MemoryHub`.
//! Alice → Bob veri yolu kullanıcı-alanı röleden geçer: ileri 12,5 kB/s, 64 paket FIFO, 300 ms,
//! %20 tohumlu kayıp; dönüş 62 B/s, 64 paket FIFO, 300 ms, kayıpsız.
//!
//! Kapılar: (1) profil açık: 40 birim verildiği anda dosya şeridinin uçuşu 40 (eski kural: 5) ve
//! aktarım ≤ 40 s'de 40/40 (gerçek zaman; eşik geniş); (2) profil kapalı: uçuş yapılandırma
//! tabanında (5), kuyruk şerit tavanında (2 × 5) ve sonraki birim `Backpressure` — bugünkü yol.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::{LaneConfig, DEFAULT_FILE_WINDOW_UNITS};
use client_core::memory_link::MemoryHub;
use client_core::runtime::LinkRates;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit, TrafficClass,
    TransportError,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
const FWD_BPS: u64 = 12_500;
const REV_BPS: u64 = 62;
const QUEUE_PKTS: usize = 64;
const ONE_WAY: Duration = Duration::from_millis(300);
const LOSS_PERMILLE: u64 = 200;
const UNITS: u64 = 40;
const UNIT_BYTES: usize = 900;
const UDP_IP_OVERHEAD: u64 = 28;

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4D;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn crypto(me: u8) -> NoiseWireProtection {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0
}

fn cfg(link: Option<LinkRates>) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.link_rates = link;
    cfg.lanes = Some(LaneConfig::default());
    cfg
}

/// Serileştirme + sınırlı FIFO + sabit gecikme + tohumlu bağımsız kayıp.
struct Shaper {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    loss_permille: u64,
    rng: u64,
    busy: VecDeque<Instant>,
    wire: VecDeque<(Instant, Vec<u8>)>,
}

impl Shaper {
    fn new(rate_bps: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            limit: QUEUE_PKTS,
            delay: ONE_WAY,
            loss_permille,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
        }
    }

    fn lost(&mut self) -> bool {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) % 1000 < self.loss_permille
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start
            + Duration::from_nanos(
                (pkt.len() as u64 + UDP_IP_OVERHEAD) * 1_000_000_000 / self.rate_bps,
            );
        self.busy.push_back(done);
        if self.loss_permille > 0 && self.lost() {
            return;
        }
        self.wire.push_back((done + self.delay, pkt));
    }

    fn take_due(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|(t, _)| *t <= now) {
            out.push(self.wire.pop_front().expect("var").1);
        }
        out
    }
}

fn relay(front: std::net::UdpSocket, back: std::net::UdpSocket, stop: Arc<AtomicBool>) {
    front.set_nonblocking(true).expect("nonblocking");
    back.set_nonblocking(true).expect("nonblocking");
    let mut fwd = Shaper::new(FWD_BPS, LOSS_PERMILLE, 1);
    let mut rev = Shaper::new(REV_BPS, 0, 1 ^ 0x5A5A);
    let mut client: Option<SocketAddr> = None;
    let mut buf = vec![0u8; 2048];
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        while let Ok((n, from)) = front.recv_from(&mut buf) {
            client = Some(from);
            fwd.offer(now, buf[..n].to_vec());
        }
        while let Ok(n) = back.recv(&mut buf) {
            rev.offer(now, buf[..n].to_vec());
        }
        let now = Instant::now();
        for p in fwd.take_due(now) {
            let _ = back.send(&p);
        }
        if let Some(to) = client {
            for p in rev.take_due(now) {
                let _ = front.send_to(&p, to);
            }
        }
        std::thread::sleep(Duration::from_micros(200));
    }
}

fn file_unit(n: u64) -> PreparedUnit {
    let mut bytes = vec![0xF1];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(UNIT_BYTES, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

struct Cell {
    alice: ClientTransport<NoiseWireProtection>,
    bob: ClientTransport<NoiseWireProtection>,
    h: SessionHandle,
    /// `Backpressure`'a dek kabul edilen dosya birimi.
    accepted: u64,
    stop: Arc<AtomicBool>,
    relay: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Cell {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(r) = self.relay.take() {
            let _ = r.join();
        }
    }
}

/// Oturum kurulur, `UNITS` dosya birimi ilk `Backpressure`'a dek dosya şeridine verilir.
fn start(profile: bool) -> Cell {
    let alice_link = profile.then_some(LinkRates {
        uplink_bps: FWD_BPS,
        downlink_bps: REV_BPS,
    });
    let bob_link = profile.then_some(LinkRates {
        uplink_bps: REV_BPS,
        downlink_bps: FWD_BPS,
    });
    let hub = MemoryHub::new();
    let mut alice = ClientTransport::new(
        cfg(alice_link),
        crypto(ALICE),
        Box::new(hub.link(id(ALICE))),
    )
    .expect("alice");
    let mut bob =
        ClientTransport::new(cfg(bob_link), crypto(BOB), Box::new(hub.link(id(BOB)))).expect("bob");
    let front = std::net::UdpSocket::bind("127.0.0.1:0").expect("röle ön");
    let back = std::net::UdpSocket::bind("127.0.0.1:0").expect("röle arka");
    back.connect(bob.data_addr()).expect("röle → Bob");
    let front_addr = front.local_addr().expect("adres");
    let stop = Arc::new(AtomicBool::new(false));
    let relay_stop = Arc::clone(&stop);
    let relay = std::thread::spawn(move || relay(front, back, relay_stop));
    alice.add_peer_addr(id(BOB), front_addr);
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let h = loop {
        hub.relay();
        while alice.poll_event().is_some() {}
        while bob.poll_event().is_some() {}
        if let (Some(h), Some(_)) = (
            alice.active_session_for_test(id(BOB)),
            bob.active_session_for_test(id(ALICE)),
        ) {
            break h;
        }
        assert!(Instant::now() < deadline, "oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(2));
    };
    let mut accepted = 0u64;
    for n in 0..UNITS {
        match alice.send(h, file_unit(n), DeliveryPolicy::Reliable, TrafficClass::Bk) {
            Ok(()) => accepted += 1,
            Err(TransportError::Backpressure) => break,
            Err(e) => panic!("dosya send: {e:?}"),
        }
    }
    Cell {
        alice,
        bob,
        h,
        accepted,
        stop,
        relay: Some(relay),
    }
}

/// (1) Profil açık: dosya şeridi profil penceresini doldurur ve aktarım tamamlanır.
#[test]
fn link_profile_lets_the_file_lane_fill_the_profile_window() {
    let mut c = start(true);
    assert_eq!(
        c.accepted, UNITS,
        "profil açıkken 40 birim şerit tavanına takıldı"
    );
    let (_, inflight) = c.alice.lane_backlog(c.h).expect("şeritler açık");
    // Profil penceresi 67 500 B ≥ 40 × 900 B: bütün birimler uca verilir (eski kural: 5).
    assert_eq!(
        inflight, UNITS as usize,
        "profil açıkken dosya şeridinin uçuşu {inflight} < {UNITS}: pencere BDP tabanında kaldı"
    );
    let t0 = Instant::now();
    let mut got = 0u64;
    while got < UNITS {
        while c.alice.poll_event().is_some() {}
        while c.bob.poll_event().is_some() {}
        while let Some(d) = c.bob.recv() {
            assert_eq!(d.unit[0], 0xF1);
            got += 1;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(40),
            "profil açıkken dosya 40 s içinde bitmedi: {got}/{UNITS} (düzeltmeden önce 26,9–64,8 s)"
        );
        std::thread::sleep(Duration::from_micros(500));
    }
    eprintln!(
        "ÖLÇÜM şerit-W profil: {UNITS} × {UNIT_BYTES} B · {:.2} s",
        t0.elapsed().as_secs_f64()
    );
}

/// (2) Profil kapalı: dosya şeridinin penceresi yapılandırma tabanında, şerit tavanı 2 × W
/// (bugünkü yol).
#[test]
fn without_link_profile_the_file_window_stays_at_the_configured_floor() {
    let mut c = start(false);
    while c.alice.poll_event().is_some() {}
    let w = DEFAULT_FILE_WINDOW_UNITS as usize;
    let (queued, inflight) = c.alice.lane_backlog(c.h).expect("şeritler açık");
    assert_eq!((queued, inflight), (2 * w, w), "uçuş W, kuyruk 2 × W");
    assert_eq!(
        c.accepted,
        3 * w as u64,
        "W + 2 × W birimden sonra Backpressure"
    );
}
