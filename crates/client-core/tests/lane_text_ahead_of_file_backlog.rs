//! [ŞERİT-KUYRUK · ürün hedefi] Akışlar birbirini bloklamaz: yön hızları dosya penceresini profil
//! penceresine büyüttüğünde, dosyanın başında sunulan metin dosyanın arkasında beklemez.
//!
//! # Hangi boşluğu kapatır
//!
//! Yön hızları (`RuntimeConfig::link_rates`) açıkken dosya şeridinin penceresi göndericinin profil
//! penceresini doldurur (`lanes::file_window_floor`; 12,5 kB/s ileri, 62 B/s dönüşte 67 birim).
//! Şerit katmanı pencereye giren her dosya birimini hemen motora veriyordu. Motorun gönderim
//! kuyruğu (`adaptive-transport` `Sender::tx_queue`) FIFO boşalır ve şerit sırasını bilmez; şerit
//! sırası (PTT > SIG > TEXT > FILE) yalnız şerit katmanında bekleyen birime uygulanır. Dosyanın
//! başında sunulan metin, motora verilmiş bütün dosya birimlerinin arkasına girdi. Ölçüm (S1
//! hücresi, 40 × 900 B, metin ilk dosya birimi teslim edilince): 3,19 / 0,31 / 2,48 s (tohum
//! 1 / 2 / 3).
//!
//! Düzeltme: profil pencereyi büyüttüğünde dosya birimi motora yalnız motorun gönderilmemiş
//! birikimi `lanes::file_backlog_bound_units`'ın (bu hücrede 2 birim) altındayken verilir;
//! pencereye alınan fazlası şerit katmanında bekler. Pencere boyu değişmez.
//!
//! # Düzenek
//!
//! İki gerçek `ClientTransport`, ürün yapılandırması (`ClientTransportConfig::product`: port motoru,
//! şeritler açık, uyarlamalı FEC), iki uca kendi yönünden yön hızları. Zarflar `MemoryHub`. Alice →
//! Bob veri yolu kullanıcı-alanı röleden geçer: ileri 12,5 kB/s, 64 paket FIFO, 300 ms tek yön;
//! dönüş 62 B/s, 64 paket FIFO, 300 ms, kayıpsız. İleri yönde [`LOSSY_ABOVE`] B'tan büyük her
//! datagram %20 tohumlu kayıpla düşer (dosya verisi, yeniden iletimi, probu, FEC onarımı); küçük
//! datagramlar (metin, PARAM) düşmez. Neden: metin kaybolursa teslimi bir sonraki ACK'i (bu dönüşte
//! ≈ 5,4 s'de bir) bekler; o bekleme bu testin sınadığı kuyruk sırasından bağımsızdır ve sonucu
//! tohuma bağlı kılardı. Dosya kayıplı kalır: pencere ve bekletme kayıp kurtarmasıyla birlikte
//! sınanır.
//!
//! Alice [`FILE_UNITS`] × [`UNIT_BYTES`] B'lik dosyayı dosya şeridine verir, aynı turda bir
//! [`TEXT_BYTES`] B'lik metin sunar.
//!
//! # Eşikler (ölçümden önce)
//!
//! * [`TEXT_BOUND`] = 2 s: metnin sunumdan teslime süresi. Bileşenler: tek yön 300 ms; metinden
//!   önce tele çıkabilecek dosya verisi = pacer'ın kredisi (en çok 10 · MTU = 12 000 B, port
//!   motorunun burst tabanı; profil pacer tavanı hattın hızına eşit olduğundan bu veri darboğaz
//!   kuyruğunda boşalmadan durur) + motor birikimi sınırı (2 birim) ≈ 14 300 B tel baytı, 12,5 kB/s'de
//!   ≈ 1,15 s; metnin kendi serileşmesi ≈ 10 ms. Toplam ≈ 1,45 s; kalan ≈ 0,55 s yüklü iki çekirdekli
//!   makinede iş parçacığı zamanlaması içindir. Düzeltme yokken metin motor kuyruğunda kalan
//!   ≈ 48 dosya biriminin (≈ 47 kB, ≈ 3,7 s) ve aynı darboğaz kuyruğunun arkasındadır: ≈ 5 s.
//! * [`FILE_BOUND`] = 60 s: dosyanın tamamı. Aynı hücrede 40 birim profil penceresiyle 7–16 s,
//!   pencere BDP tabanında (5 birim) kaldığında 27–65 s sürdü. 60 birimin kaybı birkaç ACK
//!   aralığında (≈ 5,4 s) yeniden gider; 60 s bekletmenin dosyayı pencere tabanına düşürmediğini
//!   ve aktarımın tamamlandığını yakalar.
//! * Dosyanın hepsi `Backpressure` olmadan kabul edilir ve sunulur sunulmaz pencerededir
//!   (uçuş + bekletilen = [`FILE_UNITS`]): bekletme pencereyi küçültmez.

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
const FWD_BPS: u64 = 12_500;
const REV_BPS: u64 = 62;
const QUEUE_PKTS: usize = 64;
const ONE_WAY: Duration = Duration::from_millis(300);
const LOSS_PERMILLE: u64 = 200;
/// Bu boydan (UDP gövdesi, B) büyük ileri datagramlar kayıplıdır; metin (≈ 120 B) ve PARAM değil.
const LOSSY_ABOVE: usize = 400;
const FILE_UNITS: u64 = 60;
const UNIT_BYTES: usize = 900;
const TEXT_BYTES: usize = 64;
const UDP_IP_OVERHEAD: u64 = 28;
const TEXT_BOUND: Duration = Duration::from_secs(2);
const FILE_BOUND: Duration = Duration::from_secs(60);
const FILE_TAG: u8 = 0xF1;
const TEXT_TAG: u8 = 0x7E;

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

fn cfg(link: LinkRates) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.link_rates = Some(link);
    cfg.lanes = Some(LaneConfig::default());
    cfg
}

/// Serileştirme + sınırlı FIFO + sabit gecikme + büyük datagramlarda tohumlu bağımsız kayıp.
struct Shaper {
    rate_bps: u64,
    loss_permille: u64,
    rng: u64,
    busy: VecDeque<Instant>,
    wire: VecDeque<(Instant, Vec<u8>)>,
}

impl Shaper {
    fn new(rate_bps: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            loss_permille,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
        }
    }

    fn lost(&mut self, len: usize) -> bool {
        if self.loss_permille == 0 || len <= LOSSY_ABOVE {
            return false;
        }
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) % 1000 < self.loss_permille
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= QUEUE_PKTS {
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start
            + Duration::from_nanos(
                (pkt.len() as u64 + UDP_IP_OVERHEAD) * 1_000_000_000 / self.rate_bps,
            );
        self.busy.push_back(done);
        if self.lost(pkt.len()) {
            return;
        }
        self.wire.push_back((done + ONE_WAY, pkt));
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

fn unit(tag: u8, n: u64, size: usize) -> PreparedUnit {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(size, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

struct Cell {
    alice: ClientTransport<NoiseWireProtection>,
    bob: ClientTransport<NoiseWireProtection>,
    h: SessionHandle,
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

fn start() -> Cell {
    let hub = MemoryHub::new();
    let mut alice = ClientTransport::new(
        cfg(LinkRates {
            uplink_bps: FWD_BPS,
            downlink_bps: REV_BPS,
        }),
        crypto(ALICE),
        Box::new(hub.link(id(ALICE))),
    )
    .expect("alice");
    let mut bob = ClientTransport::new(
        cfg(LinkRates {
            uplink_bps: REV_BPS,
            downlink_bps: FWD_BPS,
        }),
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
    Cell {
        alice,
        bob,
        h,
        stop,
        relay: Some(relay),
    }
}

#[test]
fn text_at_file_start_is_not_queued_behind_the_file_in_the_engine() {
    let mut c = start();
    let h = c.h;
    for n in 0..FILE_UNITS {
        match c.alice.send(
            h,
            unit(FILE_TAG, n, UNIT_BYTES),
            DeliveryPolicy::Reliable,
            TrafficClass::Bk,
        ) {
            Ok(()) => {}
            Err(TransportError::Backpressure) => {
                panic!("dosya birimi {n}: Backpressure — pencere profil penceresinin altında")
            }
            Err(e) => panic!("dosya send: {e:?}"),
        }
    }
    let (queued, window) = c.alice.lane_backlog(h).expect("şeritler açık");
    assert_eq!(
        (queued, window),
        (0, FILE_UNITS as usize),
        "bütün dosya penceredeydi (uçuş + bekletilen)"
    );
    let t0 = Instant::now();
    c.alice
        .send(
            h,
            unit(TEXT_TAG, 0, TEXT_BYTES),
            DeliveryPolicy::Reliable,
            TrafficClass::Be,
        )
        .expect("metin send");

    let mut text_latency: Option<Duration> = None;
    let mut got = 0u64;
    let mut window_max = window;
    while got < FILE_UNITS || text_latency.is_none() {
        while c.alice.poll_event().is_some() {}
        while c.bob.poll_event().is_some() {}
        if let Some((_, w)) = c.alice.lane_backlog(h) {
            window_max = window_max.max(w);
        }
        while let Some(d) = c.bob.recv() {
            match d.unit[0] {
                TEXT_TAG => text_latency = Some(t0.elapsed()),
                FILE_TAG => {
                    let n = u64::from_be_bytes(d.unit[1..9].try_into().expect("8 B"));
                    assert_eq!(n, got, "dosya şerit içinde sırayla ve tam bir kez");
                    got += 1;
                }
                t => panic!("bilinmeyen birim {t:#x}"),
            }
        }
        if text_latency.is_none() {
            assert!(
                t0.elapsed() <= TEXT_BOUND,
                "dosyanın başında sunulan metin {TEXT_BOUND:?} içinde gelmedi (dosya {got}/{FILE_UNITS}): \
                 metin motor kuyruğunda dosyanın arkasında"
            );
        }
        assert!(
            t0.elapsed() <= FILE_BOUND,
            "dosya {FILE_BOUND:?} içinde bitmedi: {got}/{FILE_UNITS}"
        );
        std::thread::sleep(Duration::from_micros(500));
    }
    let text = text_latency.expect("metin geldi");
    let stats = c.alice.lane_stats().expect("şeritler açık");
    eprintln!(
        "ÖLÇÜM şerit-kuyruk: metin {:.2} s · dosya {FILE_UNITS} × {UNIT_BYTES} B {:.2} s · pencere en çok \
         {window_max} · bekletme turu {}",
        text.as_secs_f64(),
        t0.elapsed().as_secs_f64(),
        stats.file_backlog_held,
    );
    assert!(text <= TEXT_BOUND, "metin {text:?} > {TEXT_BOUND:?}");
}
