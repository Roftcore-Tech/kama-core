//! [ŞERİT-W] Dosya penceresi yolun BDP'siyle büyür: şeritler açıkken (ürün profili) dosya
//! aktarımı bağlantıyı doldurur.
//!
//! Sabit pencere (gösterim varsayılanı W = 5 teyitsiz dosya birimi) dosya hızını `W · S / RTT`'de
//! tavanlar: bu düzenekte 5 × 900 B / 0,1 s ≈ 45 kB/s, bağlantı 400 kB/s. Pencere motorun ölçtüğü
//! `BtlBw × RTprop`'tan türetilince (`lanes::file_window_for_path`) dosya, bağlantı hızına yaklaşır;
//! metnin/PTT'nin önündeki dosya verisi yine en çok ≈ 2 BDP'dir.
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması, şeritler açık, FEC kapalı), zarflar
//! `MemoryHub`. Alice → Bob veri yolu kullanıcı-alanı bir röleden geçer: ileri yön [`RATE_BPS`]
//! hız, [`QUEUE_PKTS`] paketlik FIFO, [`ONE_WAY`] gecikme; dönüş yönü aynı gecikme, sınırsız hız.
//! Kayıp yok. Alice [`UNITS`] dosya birimi gönderir; Bob'un hepsini teslim aldığı süre ölçülür.
//!
//! Ek iddia: dosya akarken gönderilen metin, önündeki ≤ 2 BDP dosya verisi yüzünden en çok
//! ≈ tek yön + 2·RTprop gecikir (şerit kabul sırası TEXT > FILE; alıcıda şerit başına sıra).
//!
//! Gerçek zamanlıdır: eşikler geniş tutulur (yük hızı ≥ sabit-W tavanının 3 katı ≈ bağlantının
//! %34'ü; metin ≤ tek yön + 4 × RTT).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::LaneConfig;
use client_core::memory_link::MemoryHub;
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
/// İleri yön darboğaz hızı (UDP gövdesi + 28 B UDP/IP, bayt/s).
const RATE_BPS: u64 = 400_000;
/// İleri yön FIFO kuyruğu (paket).
const QUEUE_PKTS: usize = 256;
/// Tek yön gecikme (iki yönde aynı) → RTprop ≈ 100 ms.
const ONE_WAY: Duration = Duration::from_millis(50);
/// Dosya birimi sayısı ve yükü.
const UNITS: u64 = 400;
const UNIT_BYTES: usize = 900;
/// Gösterim varsayılanı (sabit pencere) — karşılaştırma tavanı için.
const FIXED_W: u64 = 5;
const WAIT: Duration = Duration::from_secs(60);
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

fn cfg() -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    // Ürün profili şeritleri zaten açar; açıkça yazılır ki test eski commit'te de aynı kurulumla koşsun.
    cfg.lanes = Some(LaneConfig::default());
    cfg
}

/// Bir yönün biçimlendiricisi: serileştirme (hız) + sınırlı FIFO + sabit gecikme.
struct Shaper {
    rate_bps: Option<u64>,
    limit: usize,
    delay: Duration,
    /// Serileştirmesi süren / bekleyen paketlerin bitiş anları (kuyruk doluluğu).
    busy: VecDeque<Instant>,
    /// Gecikme hattındaki paketler (varış anı, gövde).
    wire: VecDeque<(Instant, Vec<u8>)>,
    dropped: u64,
}

impl Shaper {
    fn new(rate_bps: Option<u64>, limit: usize, delay: Duration) -> Self {
        Self {
            rate_bps,
            limit,
            delay,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            dropped: 0,
        }
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        let done = match self.rate_bps {
            Some(rate) => {
                if self.busy.len() >= self.limit {
                    self.dropped += 1;
                    return;
                }
                let start = self.busy.back().copied().unwrap_or(now).max(now);
                let ser = Duration::from_nanos(
                    (pkt.len() as u64 + UDP_IP_OVERHEAD) * 1_000_000_000 / rate,
                );
                let done = start + ser;
                self.busy.push_back(done);
                done
            }
            None => now,
        };
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

/// Röle: `front` (Alice'in istemci ucu) ↔ `back` (Bob'un veri alıcısına bağlı). Engellemeyen
/// soketler, 200 µs'lik döngü (gecikme 50 ms'nin yanında ihmal edilir).
fn relay(
    front: std::net::UdpSocket,
    back: std::net::UdpSocket,
    stop: Arc<AtomicBool>,
) -> (u64, u64) {
    front.set_nonblocking(true).expect("nonblocking");
    back.set_nonblocking(true).expect("nonblocking");
    let mut fwd = Shaper::new(Some(RATE_BPS), QUEUE_PKTS, ONE_WAY);
    let mut rev = Shaper::new(None, usize::MAX, ONE_WAY);
    let mut client: Option<SocketAddr> = None;
    let mut buf = vec![0u8; 2048];
    let mut fwd_pkts = 0u64;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        while let Ok((n, from)) = front.recv_from(&mut buf) {
            client = Some(from);
            fwd.offer(now, buf[..n].to_vec());
            fwd_pkts += 1;
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
    (fwd_pkts, fwd.dropped)
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

#[test]
fn file_transfer_fills_the_link_when_the_file_window_tracks_the_bdp() {
    let hub = MemoryHub::new();
    let mut alice =
        ClientTransport::new(cfg(), crypto(ALICE), Box::new(hub.link(id(ALICE)))).expect("alice");
    let mut bob =
        ClientTransport::new(cfg(), crypto(BOB), Box::new(hub.link(id(BOB)))).expect("bob");

    let front = std::net::UdpSocket::bind("127.0.0.1:0").expect("röle ön");
    let back = std::net::UdpSocket::bind("127.0.0.1:0").expect("röle arka");
    back.connect(bob.data_addr()).expect("röle → Bob");
    let front_addr = front.local_addr().expect("adres");
    let stop = Arc::new(AtomicBool::new(false));
    let relay_stop = Arc::clone(&stop);
    let relay_thread = std::thread::spawn(move || relay(front, back, relay_stop));

    alice.add_peer_addr(id(BOB), front_addr);
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    let deadline = Instant::now() + WAIT;
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
    let mut last_seen: Option<u64> = None;
    // Dosyanın yarısı teslim edilince bir metin gönderilir; dosya akarken teslim gecikmesi ölçülür.
    let mut text_sent_at: Option<Instant> = None;
    let mut text_latency: Option<Duration> = None;
    while got < UNITS || text_latency.is_none() {
        while next < UNITS {
            match alice.send(
                h,
                file_unit(next),
                DeliveryPolicy::Reliable,
                TrafficClass::Bk,
            ) {
                Ok(()) => next += 1,
                Err(TransportError::Backpressure) => break,
                Err(e) => panic!("dosya send: {e:?}"),
            }
        }
        if text_sent_at.is_none() && got >= UNITS / 2 {
            let mut bytes = vec![0x7E];
            bytes.extend_from_slice(&0u64.to_be_bytes());
            bytes.resize(64, 0);
            alice
                .send(
                    h,
                    PreparedUnit {
                        bytes,
                        compression_policy: CompressionPolicy::Never,
                    },
                    DeliveryPolicy::Reliable,
                    TrafficClass::Be,
                )
                .expect("metin send");
            text_sent_at = Some(Instant::now());
        }
        while alice.poll_event().is_some() {}
        while bob.poll_event().is_some() {}
        while let Some(d) = bob.recv() {
            if d.unit[0] == 0x7E {
                text_latency = text_sent_at.map(|t| t.elapsed());
                continue;
            }
            let n = u64::from_be_bytes(d.unit[1..9].try_into().expect("8 B"));
            if let Some(prev) = last_seen {
                assert_eq!(n, prev + 1, "dosya şeridi içinde sıra korunur");
            }
            last_seen = Some(n);
            got += 1;
        }
        assert!(
            t0.elapsed() < WAIT,
            "dosya {WAIT:?} içinde bitmedi: {got}/{UNITS}"
        );
        std::thread::sleep(Duration::from_micros(500));
    }
    let elapsed = t0.elapsed();
    let text_latency = text_latency.expect("metin teslim edildi");
    stop.store(true, Ordering::Relaxed);
    let (fwd_pkts, fwd_dropped) = relay_thread.join().expect("röle");

    let goodput = (UNITS * UNIT_BYTES as u64) as f64 / elapsed.as_secs_f64();
    let rtt = 2.0 * ONE_WAY.as_secs_f64();
    let fixed_ceiling = (FIXED_W * UNIT_BYTES as u64) as f64 / rtt;
    eprintln!(
        "ÖLÇÜM şerit-W: {UNITS} × {UNIT_BYTES} B · {:.2} s · yük hızı {:.1} kB/s · bağlantı {:.0} kB/s \
         · sabit W={FIXED_W} tavanı {:.1} kB/s · dosya akarken metin gecikmesi {:.0} ms \
         · ileri paket {fwd_pkts} · kuyruk düşmesi {fwd_dropped}",
        elapsed.as_secs_f64(),
        goodput / 1000.0,
        RATE_BPS as f64 / 1000.0,
        fixed_ceiling / 1000.0,
        text_latency.as_secs_f64() * 1000.0,
    );
    assert_eq!(got, UNITS, "bütün dosya birimleri tam bir kez");
    assert!(
        goodput >= 3.0 * fixed_ceiling,
        "dosya penceresi BDP'yi izlemiyor: yük hızı {:.1} kB/s < 3 × sabit-W tavanı {:.1} kB/s",
        goodput / 1000.0,
        fixed_ceiling / 1000.0
    );
    // Metnin önündeki dosya verisi ≤ ~2 BDP: gecikme ≈ tek yön + 2·RTprop. Eşik geniş (gerçek zaman):
    // tek yön + 4 × RTT.
    let bound = ONE_WAY + 4 * (2 * ONE_WAY);
    assert!(
        text_latency <= bound,
        "dosya akarken metin {text_latency:?} > {bound:?}: dosya penceresi metnin önünde fazla kuyruk"
    );
}
