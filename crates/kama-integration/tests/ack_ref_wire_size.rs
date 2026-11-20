//! ACK REFERANS DÜZENEĞİ · (e) ACK datagramının TEL boyu (zarf dahil) ≤ bugünkü en büyük değer.
//!
//! # Neden var
//!
//! Dönüş hattı 62 B/s olan hatta her ACK baytı saniyedir. ACK'i sıkıştıracak bir PR'ın boyu
//! BÜYÜTMEDİĞİ makinece görülmeli. Bu test bugünkü en büyük ACK datagramını sabitler; davranış ve
//! tel biçimi değişmez.
//!
//! # Düzenek
//!
//! İki gerçek `ClientTransport` (ürün yapılandırması, gerçek kripto `NoiseWireProtection`, ürünün
//! `Legacy` zarfı) ve üstünde `kama-app` `Messenger`. Veri yolu kullanıcı-alanı röleden geçer:
//! ileri yönde (Alice → Bob) tohumlu %30 rastgele kayıp — çok boşluk, dolayısıyla 16 aralık
//! tavanında ACK; dönüş kayıpsız. Alice metin yollar (400; profil kolunda 30). Bob'un kriptosu salt-okur bir
//! sarmalayıcıyla ([`Tap`]) sarılır: mühürlenen her ACK çerçevesinin (0x10/0x14) mühürlü bayt
//! parmak izi kaydedilir; röle dönüş yönünde gördüğü her datagramın boyunu ve parmak izini yazar.
//! Tel boyu = röledeki UDP gövdesi + 28 B UDP/IP.
//!
//! İki kol: ürün varsayılanı (yön profili yok) ve yön profili açık (dönüş bütçeli ACK yolu).
//!
//! # İddialar
//!
//! * Mühürlenen her ACK telde görüldü (kayıt eksiksiz); ürün varsayılanı kolunda en az bir ACK 16
//!   aralıklıdır (vakum).
//! * Her ACK datagramı: UDP gövdesi ≤ [`ACK_DATAGRAM_MAX_UDP_BYTES`], tel ≤
//!   [`ACK_DATAGRAM_MAX_LINE_BYTES`].

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use client_core::memory_link::MemoryHub;
use client_core::runtime::LinkRates;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use kama_app::messaging::{Messenger, MessengerEvent};
use kama_app::store::MemoryAppStore;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::ConnectResult;
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit, SessionCommand, SessionControl, SessionEvent as CryptoEvent, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

/// Bugünkü en büyük ACK çerçevesi: V2 15 B + 16 aralık × 4 B (elle; koddan türetilmez).
const ACK_FRAME_MAX_BYTES: usize = 79;
/// Ürünün `Legacy` Noise zarfı (handle 4 + seq 2 + etiket 16).
const LEGACY_ENVELOPE_BYTES: usize = 22;
/// UDP/IP zarfı (hattın serileştirdiği).
const UDP_IP_BYTES: usize = 28;
/// Bugünkü en büyük ACK datagramının UDP gövdesi: 79 + 22 = 101 B (elle).
const ACK_DATAGRAM_MAX_UDP_BYTES: usize = 101;
/// Bugünkü en büyük ACK datagramının tel boyu: 101 + 28 = 129 B (elle). ACK biçimini değiştiren PR
/// bu sabiti BİLEREK değiştirir (yalnız aşağı).
const ACK_DATAGRAM_MAX_LINE_BYTES: usize = 129;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
/// Metin sayısı: ürün varsayılanında 400; profil kolunda 30 (dönüş bütçesi ACK'i ≈ saniyelerde
/// bir çıkarır; kol yalnız boyu ölçer, süreyi değil).
const TEXTS: usize = 400;
const TEXTS_PROFILE: usize = 30;
const FWD_LOSS_PERMILLE: u64 = 300;
const RUN_CAP: Duration = Duration::from_secs(90);

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4E;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Mühürlenen ACK çerçeveleri: parmak izi → (çerçeve boyu, mühürlü boy).
type AckLog = Arc<Mutex<HashMap<u64, (usize, usize)>>>;

struct Tap {
    inner: NoiseWireProtection,
    acks: AckLog,
}

impl PacketProtection for Tap {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        let sealed = self.inner.seal(ctx, frame)?;
        if matches!(frame.first(), Some(&FRAME_TYPE_ACK | &FRAME_TYPE_ACK_V2)) {
            lock(&self.acks).insert(
                fingerprint(&sealed.bytes),
                (frame.len(), sealed.bytes.len()),
            );
        }
        Ok(sealed)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        self.inner.open(wire)
    }
    fn sealed_len(&self, ctx: ProtectionContext, n: usize) -> Result<usize, SealError> {
        self.inner.sealed_len(ctx, n)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        self.inner.conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        self.inner.limits(ctx)
    }
}

impl SessionControl for Tap {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        self.inner.command(cmd)
    }
    fn poll_event(&mut self) -> Option<CryptoEvent> {
        self.inner.poll_event()
    }
}

fn transport(
    me: u8,
    acks: &AckLog,
    hub: &MemoryHub,
    link: Option<LinkRates>,
) -> ClientTransport<Tap> {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    let tap = Tap {
        inner: fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0,
        acks: Arc::clone(acks),
    };
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.link_rates = link;
    ClientTransport::new(cfg, tap, Box::new(hub.link(id(me)))).expect("transport")
}

/// Röle: ileri yönde tohumlu kayıp; dönüşte her datagramın (parmak izi, boy) kaydı.
fn relay(
    front_a: UdpSocket,
    back_a: UdpSocket,
    front_b: UdpSocket,
    back_b: UdpSocket,
    stop: &AtomicBool,
) -> Vec<(u64, usize)> {
    for s in [&front_a, &back_a, &front_b, &back_b] {
        s.set_nonblocking(true).expect("nonblocking");
    }
    let mut lcg: u64 = 0x5EED_0001;
    let mut lossy = || {
        lcg = lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (lcg >> 33) % 1000 < FWD_LOSS_PERMILLE
    };
    let (mut alice_c, mut bob_c): (Option<SocketAddr>, Option<SocketAddr>) = (None, None);
    let mut rev_log = Vec::new();
    let mut buf = vec![0u8; 2048];
    while !stop.load(Ordering::Relaxed) {
        let mut idle = true;
        while let Ok((n, from)) = front_a.recv_from(&mut buf) {
            idle = false;
            alice_c = Some(from);
            if !lossy() {
                let _ = back_a.send(&buf[..n]);
            }
        }
        // Bob sunucu → Alice istemci: Bob'un ACK'leri (dönüş).
        while let Ok(n) = back_a.recv(&mut buf) {
            idle = false;
            rev_log.push((fingerprint(&buf[..n]), n));
            if let Some(to) = alice_c {
                let _ = front_a.send_to(&buf[..n], to);
            }
        }
        while let Ok((n, from)) = front_b.recv_from(&mut buf) {
            idle = false;
            bob_c = Some(from);
            rev_log.push((fingerprint(&buf[..n]), n));
            let _ = back_b.send(&buf[..n]);
        }
        while let Ok(n) = back_b.recv(&mut buf) {
            idle = false;
            if let Some(to) = bob_c {
                let _ = front_b.send_to(&buf[..n], to);
            }
        }
        if idle {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    rev_log
}

fn arm(name: &str, profile: bool) {
    let texts = if profile { TEXTS_PROFILE } else { TEXTS };
    let hub = MemoryHub::new();
    let alice_log: AckLog = Arc::default();
    let bob_log: AckLog = Arc::default();
    let link = |up, down| {
        profile.then_some(LinkRates {
            uplink_bps: up,
            downlink_bps: down,
        })
    };
    let mut alice_t = transport(ALICE, &alice_log, &hub, link(12_500, 62));
    let mut bob_t = transport(BOB, &bob_log, &hub, link(62, 12_500));
    let bind = || UdpSocket::bind("127.0.0.1:0").expect("röle soketi");
    let (front_a, back_a, front_b, back_b) = (bind(), bind(), bind(), bind());
    back_a.connect(bob_t.data_addr()).expect("röle → Bob");
    back_b.connect(alice_t.data_addr()).expect("röle → Alice");
    alice_t.add_peer_addr(id(BOB), front_a.local_addr().expect("adres"));
    bob_t.add_peer_addr(id(ALICE), front_b.local_addr().expect("adres"));
    let stop = Arc::new(AtomicBool::new(false));
    let relay_stop = Arc::clone(&stop);
    let relay_thread =
        std::thread::spawn(move || relay(front_a, back_a, front_b, back_b, &relay_stop));

    let mut alice = Messenger::with_store(alice_t, Arc::new(MemoryAppStore::new())).expect("AMP");
    let mut bob = Messenger::with_store(bob_t, Arc::new(MemoryAppStore::new())).expect("AMP");
    match alice.connect(id(BOB)).expect("connect") {
        ConnectResult::Pending(_) | ConnectResult::Ready(_) => {}
    }
    let deadline = Instant::now() + RUN_CAP;
    while alice.session_of(id(BOB)).is_none() || bob.session_of(id(ALICE)).is_none() {
        hub.relay();
        while alice.poll().is_some() {}
        while bob.poll().is_some() {}
        assert!(Instant::now() < deadline, "[{name}] oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(1));
    }
    let (mut sent, mut received) = (0usize, 0usize);
    let mut next_send = Instant::now();
    while received < texts && Instant::now() < deadline {
        hub.relay();
        if sent < texts && Instant::now() >= next_send {
            if alice
                .send_text(id(BOB), &format!("ack-ref metin {sent}"))
                .is_ok()
            {
                sent += 1;
            }
            next_send = Instant::now() + Duration::from_millis(5);
        }
        while alice.poll().is_some() {}
        while let Some(e) = bob.poll() {
            if matches!(e, MessengerEvent::TextReceived { .. }) {
                received += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // ACK'lerin tele çıkması için kısa boşaltma.
    let settle = Instant::now() + Duration::from_millis(500);
    while Instant::now() < settle {
        while alice.poll().is_some() {}
        while bob.poll().is_some() {}
        std::thread::sleep(Duration::from_millis(1));
    }
    stop.store(true, Ordering::Relaxed);
    let rev = relay_thread.join().expect("röle");
    let on_wire: HashMap<u64, usize> = rev.into_iter().collect();
    let acks = lock(&bob_log).clone();
    let mut seen = 0usize;
    let (mut max_frame, mut max_udp, mut at_cap) = (0usize, 0usize, 0usize);
    for (fp, (frame_len, sealed_len)) in &acks {
        assert_eq!(
            *sealed_len,
            frame_len + LEGACY_ENVELOPE_BYTES,
            "[{name}] Legacy zarfı 22 B olmalı"
        );
        max_frame = max_frame.max(*frame_len);
        if *frame_len == ACK_FRAME_MAX_BYTES {
            at_cap += 1;
        }
        if let Some(&udp) = on_wire.get(fp) {
            seen += 1;
            max_udp = max_udp.max(udp);
            assert!(
                udp <= ACK_DATAGRAM_MAX_UDP_BYTES,
                "[{name}] ACK datagramı UDP gövdesi {udp} B > {ACK_DATAGRAM_MAX_UDP_BYTES}"
            );
            assert!(
                udp + UDP_IP_BYTES <= ACK_DATAGRAM_MAX_LINE_BYTES,
                "[{name}] ACK datagramı tel boyu {} B > {ACK_DATAGRAM_MAX_LINE_BYTES}",
                udp + UDP_IP_BYTES
            );
        }
    }
    println!(
        "ACK-REF tel-boyu [{name}] metin={received}/{texts} ack_mühürlenen={} ack_telde={seen} \
         tavanda(16 aralık)={at_cap} en_büyük_çerçeve={max_frame}B en_büyük_udp={max_udp}B \
         en_büyük_tel={}B",
        acks.len(),
        max_udp + UDP_IP_BYTES
    );
    assert!(!acks.is_empty(), "[{name}] vakum: ACK mühürlenmedi");
    assert_eq!(
        seen,
        acks.len(),
        "[{name}] mühürlenen her ACK telde görülmeli"
    );
    assert!(max_frame <= ACK_FRAME_MAX_BYTES);
    // Tavan vakumu ürün varsayılanı kolunda bağlayıcıdır (yüzlerce ACK); profil kolu birkaç ACK
    // üretir ve yalnız boy tavanını sınar.
    if !profile {
        assert!(at_cap > 0, "[{name}] vakum: 16 aralıklı ACK görülmedi");
    }
}

#[test]
fn ack_ref_wire_size_product_default() {
    arm("ürün-varsayılanı", false);
}

#[test]
fn ack_ref_wire_size_link_profile() {
    arm("yön-profili", true);
}
