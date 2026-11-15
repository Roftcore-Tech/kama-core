//! [ÖLÇÜM · yalnız `CC_S1_ASYM=1`] Ürünün S1 yolu (`ClientTransport`, şeritler) asimetrik hücrede.
//!
//! `tests/shaped_relay.rs` asimetrik hücresi veri yolunu doğrudan (`ClientHandle::send`, şerit 0)
//! sürer; şerit katmanı (`lanes`, dosya penceresi) orada devrede değildir. Bu araç aynı hücreyi
//! iki gerçek `ClientTransport` ile koşar: Alice dosya şeridine (`TrafficClass::Bk`) birim verir,
//! Bob teslim alır. Dosya penceresi `lanes::file_window_for_path` ile kurulur.
//!
//! Hücre: ileri `CC_S1_FWD` B/s (varsayılan 12 500), `CC_S1_Q` paket FIFO (64), tek yön
//! `CC_S1_DELAY_MS` (300), ileri kayıp `CC_S1_LOSS_PM` binde (200, tohumlu); dönüş `CC_S1_REV`
//! B/s (62), 64 paket FIFO, aynı gecikme, kayıpsız.
//!
//! Anahtarlar: `CC_S1_LINK=1` iki uca doğru yön hızları (`RuntimeConfig::link_rates`);
//! `CC_S1_LANES=0` şeritleri kapatır (eski yol, birim doğrudan uca); `CC_S1_SEED`, `CC_S1_UNITS`
//! (40), `CC_S1_UNIT_BYTES` (900), `CC_S1_TEXT_AT` (bu kadar dosya birimi teslim edilince bir
//! metin gönderilir; 0 = yok), `CC_S1_TMAX_S` (180).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::LaneConfig;
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
const UDP_IP_OVERHEAD: u64 = 28;

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

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

fn cfg(link: Option<LinkRates>, lanes: bool) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.link_rates = link;
    cfg.lanes = lanes.then(LaneConfig::default);
    cfg
}

/// Serileştirme (hız) + sınırlı FIFO + sabit gecikme + tohumlu bağımsız kayıp.
struct Shaper {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    loss_pm: u64,
    rng: u64,
    busy: VecDeque<Instant>,
    wire: VecDeque<(Instant, Vec<u8>)>,
    sent: u64,
    bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
}

impl Shaper {
    fn new(rate_bps: u64, limit: usize, delay: Duration, loss_pm: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            limit,
            delay,
            loss_pm,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            sent: 0,
            bytes: 0,
            dropped_queue: 0,
            dropped_random: 0,
        }
    }

    fn rand_pm(&mut self) -> u64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) % 1000
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let ser = Duration::from_nanos(
            (pkt.len() as u64 + UDP_IP_OVERHEAD) * 1_000_000_000 / self.rate_bps.max(1),
        );
        let done = start + ser;
        self.busy.push_back(done);
        self.sent += 1;
        self.bytes += pkt.len() as u64 + UDP_IP_OVERHEAD;
        if self.loss_pm > 0 && self.rand_pm() < self.loss_pm {
            self.dropped_random += 1;
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

struct RelayOut {
    fwd: (u64, u64, u64, u64),
    rev: (u64, u64, u64),
}

fn relay(
    front: std::net::UdpSocket,
    back: std::net::UdpSocket,
    mut fwd: Shaper,
    mut rev: Shaper,
    stop: Arc<AtomicBool>,
) -> RelayOut {
    front.set_nonblocking(true).expect("nonblocking");
    back.set_nonblocking(true).expect("nonblocking");
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
    RelayOut {
        fwd: (fwd.sent, fwd.bytes, fwd.dropped_queue, fwd.dropped_random),
        rev: (rev.sent, rev.bytes, rev.dropped_queue),
    }
}

fn unit(tag: u8, n: u64, size: usize) -> PreparedUnit {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(size, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

#[test]
fn s1_asymmetric_cell_measure() {
    if std::env::var("CC_S1_ASYM").is_err() {
        return;
    }
    let fwd_rate = env_u64("CC_S1_FWD", 12_500);
    let rev_rate = env_u64("CC_S1_REV", 62);
    let q = usize::try_from(env_u64("CC_S1_Q", 64)).unwrap_or(64);
    let delay = Duration::from_millis(env_u64("CC_S1_DELAY_MS", 300));
    let loss = env_u64("CC_S1_LOSS_PM", 200);
    let seed = env_u64("CC_S1_SEED", 1);
    let units = env_u64("CC_S1_UNITS", 40);
    let unit_bytes = usize::try_from(env_u64("CC_S1_UNIT_BYTES", 900)).unwrap_or(900);
    let text_at = env_u64("CC_S1_TEXT_AT", 0);
    let t_max = Duration::from_secs(env_u64("CC_S1_TMAX_S", 180));
    let link_on = std::env::var("CC_S1_LINK").is_ok();
    let lanes_on = env_u64("CC_S1_LANES", 1) != 0;
    // Her uca kendi bakış açısından: Alice (gönderici) yukarı = ileri; Bob (alıcı) yukarı = dönüş.
    let alice_link = link_on.then_some(LinkRates {
        uplink_bps: fwd_rate,
        downlink_bps: rev_rate,
    });
    let bob_link = link_on.then_some(LinkRates {
        uplink_bps: rev_rate,
        downlink_bps: fwd_rate,
    });

    let hub = MemoryHub::new();
    let mut alice = ClientTransport::new(
        cfg(alice_link, lanes_on),
        crypto(ALICE),
        Box::new(hub.link(id(ALICE))),
    )
    .expect("alice");
    let mut bob = ClientTransport::new(
        cfg(bob_link, lanes_on),
        crypto(BOB),
        Box::new(hub.link(id(BOB))),
    )
    .expect("bob");

    let front = std::net::UdpSocket::bind("127.0.0.1:0").expect("röle ön");
    let back = std::net::UdpSocket::bind("127.0.0.1:0").expect("röle arka");
    back.connect(bob.data_addr()).expect("röle → Bob");
    let front_addr = front.local_addr().expect("adres");
    let stop = Arc::new(AtomicBool::new(false));
    let relay_stop = Arc::clone(&stop);
    let fwd = Shaper::new(fwd_rate, q, delay, loss, seed);
    let rev = Shaper::new(rev_rate, q.max(64), delay, 0, seed ^ 0x5A5A);
    let relay_thread = std::thread::spawn(move || relay(front, back, fwd, rev, relay_stop));

    alice.add_peer_addr(id(BOB), front_addr);
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let h: SessionHandle = loop {
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

    let t0 = Instant::now();
    let mut next: u64 = 0;
    let mut got: u64 = 0;
    let mut last_deliv: Option<Duration> = None;
    let mut text_sent_at: Option<Instant> = None;
    let mut text_latency: Option<Duration> = None;
    let mut backpressure = 0u64;
    let want_text = text_at > 0;
    // Şeridin FILE penceresindeki (uca verilmiş, teyitsiz) birim sayısı: en büyüğü ve saniyelik iz.
    let mut file_inflight_max = 0usize;
    let mut trace: Vec<String> = Vec::new();
    let mut next_trace = Duration::ZERO;
    while (got < units || (want_text && text_latency.is_none())) && t0.elapsed() < t_max {
        hub.relay();
        while next < units {
            match alice.send(
                h,
                unit(0xF1, next, unit_bytes),
                DeliveryPolicy::Reliable,
                TrafficClass::Bk,
            ) {
                Ok(()) => next += 1,
                Err(TransportError::Backpressure) => {
                    backpressure += 1;
                    break;
                }
                Err(e) => panic!("dosya send: {e:?}"),
            }
        }
        if want_text && text_sent_at.is_none() && got >= text_at {
            alice
                .send(
                    h,
                    unit(0x7E, 0, 64),
                    DeliveryPolicy::Reliable,
                    TrafficClass::Be,
                )
                .expect("metin send");
            text_sent_at = Some(Instant::now());
        }
        while alice.poll_event().is_some() {}
        while bob.poll_event().is_some() {}
        if let Some((queued, fi)) = alice.lane_backlog(h) {
            file_inflight_max = file_inflight_max.max(fi);
            if t0.elapsed() >= next_trace {
                trace.push(format!("{:.0}s:{got}/{fi}/{queued}", t0.elapsed().as_secs_f64()));
                next_trace += Duration::from_secs(2);
            }
        }
        while let Some(d) = bob.recv() {
            if d.unit[0] == 0x7E {
                text_latency = text_sent_at.map(|t| t.elapsed());
                continue;
            }
            got += 1;
            last_deliv = Some(t0.elapsed());
        }
        std::thread::sleep(Duration::from_micros(500));
    }
    stop.store(true, Ordering::Relaxed);
    let r = relay_thread.join().expect("röle");
    let f = |d: Option<Duration>| d.map_or("YOK".to_string(), |d| format!("{:.2}", d.as_secs_f64()));
    println!(
        "S1-ÖZET tohum={seed} şerit={} profil={} birim={units}×{unit_bytes} alıcı_son_teslim={} teslim={got}/{units} metin_gecikme={} ileri={}/{}B kuyruk_düşen={} rastgele_düşen={} dönüş={}/{}B dönüş_düşen={} geri_basınç={backpressure} dosya_uçuş_en_çok={file_inflight_max} iz(t:teslim/uçuş/bekleyen)={}",
        if lanes_on { "açık" } else { "kapalı" },
        if link_on { "açık" } else { "kapalı" },
        if got == units { f(last_deliv) } else { "YOK".to_string() },
        f(text_latency),
        r.fwd.0,
        r.fwd.1,
        r.fwd.2,
        r.fwd.3,
        r.rev.0,
        r.rev.1,
        r.rev.2,
        trace.join(" "),
    );
}
