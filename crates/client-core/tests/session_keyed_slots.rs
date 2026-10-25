//! **K4G-01b — eş yuvası ve `Reorder` oturuma anahtarlanır** (plan Ek 2026-10-11 (3)) regresyon
//! testi. Gerçek `NoiseWireProtection` (ürün kurulumu `with_control`; TEST anahtarları
//! `kama_noise_wire::seam_mock::fixture_*`, GİZLİ DEĞİL) + gerçek veri yolu (UDP loopback).
//!
//! KAPATILAN BULGULAR (#114 incelemesi 4 ve 5, #114 beyanlı istisnası):
//! * (i) `ClientTransport`'ın sıralama durumu (`Reorder`) KAYNAK ADRESE anahtarlıydı ve hiç
//!   silinmiyordu → aynı adresten gelen yeni oturumun ilk birimleri eski oturumun `next`'inin
//!   altında kalıp sessizce atılıyordu (veri alıcısı onları ACK'lemişti: ACK'li kayıp);
//! * (ii) hattaki saldırgan geçerli datagramı sahte adresten önce iletince ek eş yuvası + adres
//!   anahtarlı `Reorder` → S1'de çift teslim;
//! * (iii) veri alıcısının eş yuvası ADRESE anahtarlıydı ve ilk oturuma bağlı kalıyordu → aynı
//!   adresten (efemeral port yeniden kullanımı) gelen yeni oturum `idle_hold` (105 s) dolana dek
//!   `ctx_mismatch_drops` ile düşüyordu.
//!
//! DÜZENEK — belirlenimli. Alice gerçek S1 adaptörüdür (`ClientTransport`, ürün builder'ı ve ürün
//! veri profili). Karşı uç Bob HAM bir eştir (`RawPeer`): Crypto'su (S3 `SessionControl` +
//! `PacketProtection`) test tarafından sürülür — S1 adaptörünün `connect` / `cancel` / zarf yolunda
//! Crypto'ya verdiği komutların aynısı —, zarfları aynı `MemoryHub` üzerinden ve YALNIZ `relay()`
//! çağrısında taşınır, veri datagramlarını test mühürler ve TEK bir UDP soketinden yollar. Bob'un
//! bütün oturumları AYNI adresten gelir: sahadaki efemeral port yeniden kullanımının belirlenimli
//! karşılığı (port kancası gerekmez, ürün grafiği değişmez). Bob'da ARQ zamanlayıcısı yoktur:
//! "yeniden gönderim" testin açık bir adımıdır (yeni PN'li yeni mühür). Bekleme süreleri yalnız
//! beklenen olayın (S1 olayı, teslim) ÜST SINIRIDIR; sıra iddiaları aynı soketten sırayla işlenen
//! bir bariyer datagramına dayanır, zamana değil.
//!
//! Testler (kart NE 3): (a) cancel+connect aynı porttan; (b) yenileme sonrası aynı port; (c)
//! adres-sahteli kopya; (d) `max_peers` yeni anahtarla. Her testin belgesi 5dbd0b80'de (K4G-01
//! merge noktası) neden KIRMIZI olduğunu yazar. Ek (e): kapanış emekliliğinin bakım-tick yolu, dolu
//! tablo baskısı olmadan (orkestratör incelemesi; yalnız tick çağrısı silinince kırmızı).
//!
//! BEYANLI KALAN AÇIK (INT-02; PoC-KAPSAM-001 §5): hattaki saldırgan gözlediği geçerli datagramı
//! sahte adresten ÖNCE ileterek `max_peers` içinde ek yuva açabilir (test (c)'de açar, (d) bunun
//! tavanla sınırlı olduğunu gösterir); kapanış M4 / adres doğrulama.

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::time::{Duration, Instant};

use client_core::memory_link::{MemoryHub, MemoryLink};
use client_core::runtime::{
    CtxDispatchCounters, CtxDispatchStats, Delivery, ExpectedSessions, Readiness, RuntimeConfig,
    ServerEndpoint, ServerHandle,
};
use client_core::transport::{
    ClientTransport, ClientTransportConfig, EnvelopeLink, LinkAttempt, LinkEvent,
};
use client_core::wire::encode_data;
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{ConnectResult, Delivered, KamaTransport, SessionEvent};
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SessionCommand, SessionControl, SessionResult,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::epoch_machine::GRACE_T_G;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::{EnvelopeKind, SESSION_MAX_AGE};
use tokio::net::UdpSocket;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
const CAROL: u8 = 0xC0;

/// Sunucu düzeyi testlerin (d), (e) hub (wire) tutamaçları.
const HUB_AB: HubHandle = HubHandle::from_raw(0x7E57_0AB0);
const HUB_AC: HubHandle = HubHandle::from_raw(0x7E57_0AC0);

/// (e): sonda turları arasında kapanan oturumun BEKLENMEDİĞİ aralık — bakım tick'inin (runtime
/// `PATIENCE_TICK` = 1 s) neredeyse her zaman düştüğü pencere. Doğruluk buna bağlı değildir: koşul
/// olaydır (sondanın teslimi), üst sınır `WAIT`.
const PROBE_PACE: Duration = Duration::from_millis(250);

/// Beklenen bir olayın (S1 olayı, teslim) ÜST SINIRI — aşılırsa kırmızı, sessiz geçiş yok. (b)'nin
/// anlamı gereği `idle_hold`'dan (105 s) çok küçüktür (test ayrıca doğrular).
const WAIT: Duration = Duration::from_secs(10);
/// "Gelmedi" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// Yoklama adımı.
const TICK: Duration = Duration::from_millis(2);

// ── Ortak fikstür ────────────────────────────────────────────────────────────────────────────

/// Sentetik test kimliği.
fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

/// Üç kimliği de bilen dizinle ürün kurulumu (`with_control`) + saatin test kopyası.
fn node(me: u8) -> (NoiseWireProtection, ManualClock) {
    let ids = [ALICE, BOB, CAROL].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy)
}

/// Birimin içeriği: hangi oturum kuşağından geldiği (`tag`) ve birim kimliği.
fn payload(tag: &str, unit: u64) -> Vec<u8> {
    format!("{tag}-{unit}").into_bytes()
}

fn delivered(session: SessionHandle, tag: &str, unit: u64) -> Delivered {
    Delivered {
        session,
        unit: payload(tag, unit),
    }
}

fn std_udp() -> StdUdpSocket {
    StdUdpSocket::bind("127.0.0.1:0").expect("UDP soketi")
}

// ── S1 düzeyi: Alice = gerçek `ClientTransport`, Bob = ham eş ────────────────────────────────

type Node = ClientTransport<NoiseWireProtection>;

/// Alice: ürün builder'ı + ürün veri profili (yalnız kapanış boşaltması kısaltılmış) + saatinin
/// kopyası.
fn alice_node(hub: &MemoryHub) -> (Node, ManualClock) {
    alice_node_with_queue(hub, None)
}

/// [`alice_node`]; `delivery_queue` verilirse veri alıcısının teslim kanalı o kapasitededir (ürün
/// 1024; yalnız dolu kanal kolları).
fn alice_node_with_queue(hub: &MemoryHub, delivery_queue: Option<usize>) -> (Node, ManualClock) {
    let (crypto, clock) = node(ALICE);
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    if let Some(q) = delivery_queue {
        cfg.runtime.delivery_queue = q;
    }
    let t = ClientTransport::new(cfg, crypto, Box::new(hub.link(id(ALICE)))).expect("düğüm");
    (t, clock)
}

/// HAM karşı uç (bkz. modül belgesi): Crypto'yu test sürer; veri yolu yok, tek veri soketi.
struct RawPeer {
    crypto: NoiseWireProtection,
    link: MemoryLink,
    /// Bütün oturumların veri datagramlarının çıktığı TEK soket — her oturum aynı adresten.
    sock: StdUdpSocket,
    /// Alice'in bu eşe açtığı oturum uçlarının yazdığı adres (okunmaz).
    sink: StdUdpSocket,
    /// Yola çıkmış INIT → bekleyen oturum (hub'ın `HandleAssigned`'ı için).
    attempts: BTreeMap<LinkAttempt, SessionHandle>,
}

impl RawPeer {
    fn new(me: u8, hub: &MemoryHub) -> Self {
        Self {
            crypto: node(me).0,
            link: hub.link(id(me)),
            sock: std_udp(),
            sink: std_udp(),
            attempts: BTreeMap::new(),
        }
    }

    fn sink_addr(&self) -> SocketAddr {
        self.sink.local_addr().expect("adres")
    }

    /// S1 `connect`'in Crypto karşılığı: `Initiate` + INIT zarfı yola çıkar (hub'da röleyi bekler).
    fn initiate(&mut self, peer: PeerRef) -> SessionHandle {
        let SessionResult::Initiated {
            session,
            envelope_type,
            bytes,
        } = self.crypto.command(SessionCommand::Initiate {
            peer,
            early_unit: None,
        })
        else {
            panic!("Initiate bekleyen oturum + INIT üretmeli");
        };
        let attempt = self
            .link
            .send_init(peer, envelope_type, bytes)
            .expect("zarf taşıyıcısı");
        self.attempts.insert(attempt, session);
        session
    }

    /// S1 `cancel` (bekleyen oturum) ve `close`'un (kurulu oturum) Crypto karşılığı.
    fn close(&mut self, session: SessionHandle) {
        let _ = self.crypto.command(SessionCommand::Close { session });
    }

    /// Gelen kutusunu S1 adaptörünün zarf yoluyla AYNI komutlarla işler; INIT'i yanıtlanan ya da
    /// RESPONSE ile kurulan oturumları sırayla döner.
    fn serve(&mut self) -> Vec<SessionHandle> {
        let mut out = Vec::new();
        while let Some(ev) = self.link.poll() {
            match ev {
                LinkEvent::HandleAssigned {
                    attempt,
                    hub_handle,
                } => {
                    if let Some(session) = self.attempts.remove(&attempt) {
                        // İptal edilmiş (kapatılmış) bekleyen oturumda opak `Failed` — beklenen.
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
                    // İptal edilmiş INIT'in geç yanıtı opak `Failed` (s3 v1.1) — beklenen.
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

    /// `session`'da mühürlü DATA çerçevesi (birim `unit`). Her çağrı yeni bir mühürdür (yeni PN):
    /// ARQ yeniden gönderiminin tel karşılığı.
    fn data(&mut self, session: SessionHandle, unit: u64, tag: &str) -> Vec<u8> {
        let frame = encode_data(0, unit, &payload(tag, unit));
        self.crypto
            .seal(ProtectionContext::Session(session), &frame)
            .expect("mühür")
            .bytes
    }

    /// Tek veri soketinden (her oturum için AYNI adres) yollar.
    fn send(&self, wire: &[u8], to: SocketAddr) {
        self.sock.send_to(wire, to).expect("gönder");
    }
}

/// Sıradaki S1 olayı (üst sınır `WAIT`; gelmezse kırmızı).
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

/// `peer` için sıradaki `Ready(h, peer)`'e dek S1 olaylarını okur; aradaki olayları da döner.
fn ready(t: &mut Node, peer: PeerRef) -> (SessionHandle, Vec<SessionEvent>) {
    let mut before = Vec::new();
    loop {
        match next_event(t) {
            SessionEvent::Ready(h, p) if p == peer => return (h, before),
            ev => before.push(ev),
        }
    }
}

/// Sıradaki S1 teslimi (üst sınır `WAIT`; gelmezse kırmızı).
fn expect_delivery(t: &mut Node) -> Delivered {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(d) = t.recv() {
            return d;
        }
        assert!(Instant::now() < deadline, "S1 teslimi gelmedi");
        std::thread::sleep(TICK);
    }
}

/// `QUIET` boyunca S1 teslimi gelmez.
fn assert_quiet(t: &mut Node, why: &str) {
    let deadline = Instant::now() + QUIET;
    while Instant::now() < deadline {
        if let Some(d) = t.recv() {
            panic!("{why}: beklenmeyen teslim {d:?}");
        }
        std::thread::sleep(TICK);
    }
}

/// Alice `connect` eder, Bob yanıtlar (röle elle sürülür) → Alice'te `Ready(a0)`, Bob'da `b0`.
fn establish(
    alice: &mut Node,
    bob: &mut RawPeer,
    hub: &MemoryHub,
) -> (SessionHandle, SessionHandle) {
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let served = bob.serve();
    assert_eq!(served.len(), 1, "Bob INIT'i yanıtlamalı");
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a0, _) = ready(alice, id(BOB));
    (a0, served[0])
}

/// Bob eski oturumda `n` birim yollar (aynı soketten) ve Alice hepsini sırayla teslim eder:
/// veri alıcısında `(S, a0)` yuvası, sıralamada `a0` için `next = n`.
fn old_session_traffic(
    alice: &mut Node,
    bob: &mut RawPeer,
    a0: SessionHandle,
    b0: SessionHandle,
    n: u64,
) {
    let to = alice.data_addr();
    for u in 0..n {
        let wire = bob.data(b0, u, "eski");
        bob.send(&wire, to);
    }
    for u in 0..n {
        assert_eq!(expect_delivery(alice), delivered(a0, "eski", u));
    }
}

/// **Kart NE 3(a) — cancel+connect aynı porttan:** yeni oturumun ilk birimi teslim olur,
/// `ctx_mismatch_drops == 0`.
///
/// Akış: `a0`/`b0` kurulur, Bob `b0`'da iki birim yollar (Alice'in veri alıcısında `(S, a0)`
/// yuvası; sıralamada `a0` için `next = 2`). Bob (K4: kurtarma = yeniden INIT) eski oturumu bırakır
/// (`Close(b0)`), `connect` eder (INIT1), `cancel` eder (`Close(b1)` — INIT1 zaten yolda) ve
/// yeniden `connect` eder (INIT2). Röle iki INIT'i de taşır: Alice'te iki provisional (`a1`, `a2`;
/// ikisi de `a0`'ın yerine), Bob'da yalnız `b2` kurulur (iptal edilenin yanıtı opak `Failed`). Bob
/// `b2`'nin ilk birimini AYNI adresten (S) yollar: epoch-1 paketi `a2`'yi onaylar → `Ready(a2)`.
/// Onay işlenmeden açılan datagram INV-ACK-DELIVER-1 gereği ACK'siz düşer (`not_ready_drops`); Bob
/// birimi yeniden yollar (ARQ'nun tel karşılığı) ve ardından birim 1'i. (Onay datagramı yarışta
/// kabul edilmişse yeniden gönderim yuvada tekilleşir — sonuç aynı.)
///
/// 5dbd0b80'de KIRMIZI: (iii) veri alıcısının yuvası ADRES anahtarlıydı ve `a0`'a bağlıydı →
/// `a2`'nin her datagramı `ctx_mismatch_drops` ile düşer (düşen datagram `last_seen_ns`'i
/// tazelemez; yuva `idle_hold` = 105 s sonra emekliye ayrılır) → teslim beklemesi `WAIT`'i aşar.
/// Yalnız yuva düzeltilse bile (i) adres anahtarlı `Reorder`'da `next = 2` olduğundan `a2`'nin 0 ve
/// 1 numaralı birimleri `unit < next` diye sessizce atılırdı (veri alıcısı ACK'lemişti: ACK'li
/// kayıp) — iki kusurdan her biri bu testi tek başına kırmızı yapar.
#[test]
fn a_cancel_connect_from_the_same_port_delivers_the_new_sessions_first_unit() {
    let hub = MemoryHub::new();
    let (mut alice, _clock) = alice_node(&hub);
    let mut bob = RawPeer::new(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.sink_addr());
    let to = alice.data_addr();
    let (a0, b0) = establish(&mut alice, &mut bob, &hub);
    old_session_traffic(&mut alice, &mut bob, a0, b0, 2);

    // Bob: close + connect + cancel + connect. İptal edilen INIT1 iptalden ÖNCE yola çıkmıştır.
    bob.close(b0);
    let b1 = bob.initiate(id(ALICE));
    bob.close(b1);
    let b2 = bob.initiate(id(ALICE));
    assert_eq!(hub.relay(), 2, "iki INIT → Alice, iki HANDLE_ASSIGN → Bob");
    alice.drive();
    assert_eq!(
        hub.relay(),
        2,
        "Alice iki INIT'i de yanıtlar (provisional): iki RESPONSE → Bob"
    );
    assert_eq!(
        bob.serve(),
        [b2],
        "yalnız yeni oturum kurulur; iptal edilenin yanıtı opak Failed"
    );

    // Yeni oturumun ilk birimi AYNI adresten: epoch-1 paketi `a2`'yi onaylar.
    let first = bob.data(b2, 0, "yeni");
    bob.send(&first, to);
    let (a2, skipped) = ready(&mut alice, id(BOB));
    assert_ne!(a2, a0);
    assert!(
        skipped.is_empty(),
        "provisional dönem S1'e görünmez: {skipped:?}"
    );

    let again = bob.data(b2, 0, "yeni");
    bob.send(&again, to);
    let next = bob.data(b2, 1, "yeni");
    bob.send(&next, to);
    assert_eq!(expect_delivery(&mut alice), delivered(a2, "yeni", 0));
    assert_eq!(expect_delivery(&mut alice), delivered(a2, "yeni", 1));
    assert_quiet(&mut alice, "tek teslim");
    assert_eq!(alice.active_session_for_test(id(BOB)), Some(a2));
    let ctx = alice.ctx_dispatch_stats();
    assert_eq!((ctx.ctx_mismatch_drops, ctx.hub_frames_dropped), (0, 0));
}

/// **Kart NE 3(b) — yenileme sonrası aynı port:** yeni oturumun birimi `idle_hold`'u (105 s)
/// beklemeden teslim edilir; eski oturumun yuvası (yalnız-alım drain) aynı adreste yan yana yaşar
/// ve oturum `Closed` olunca sıralama durumu silinir.
///
/// Akış: `a0`/`b0` kurulur, Bob `b0`'da iki birim yollar (`(S, a0)` yuvası). Alice'in saati
/// `SESSION_MAX_AGE` ilerler → Crypto `RenewRequired(a0)` → adaptör arka planda yeniler (INIT →
/// Bob'da provisional `b1`; RESPONSE tutulur, `a0` boş → atomik cutover) → `Ready(a1)`. Bob `b1`'in
/// birimini AYNI adresten (S) yollar → teslim. Ardından drain'deki `a0`'ın bir birimi de aynı
/// adresten gelir ve kendi oturumuyla teslim edilir (iki oturum, iki yuva, tek adres). Son olarak
/// drain `T_g` ile biter → `Closed(a0, Superseded)` okunur → `a0`'ın sıralama durumu silinir; `a1`
/// akmaya devam eder.
///
/// 5dbd0b80'de KIRMIZI: veri alıcısının yuvası ADRES anahtarlıydı ve `a0`'a bağlıydı → `a1`'in
/// datagramı `ctx_mismatch_drops` ile düşer; yuva `idle_hold` (105 s) dolmadan emekliye ayrılmaz ve
/// düşen datagram onu tazelemez → ilk teslim beklemesi (`WAIT` ≪ 105 s) aşılır.
/// `reorder_sessions_for_test` 5dbd0b80'de yoktur (bu kartın test-util kancası); o iki satır
/// çıkarılınca test o koda taşınarak koşturulabilir ve yine kırmızıdır.
#[test]
fn b_after_renewal_the_same_port_does_not_wait_for_idle_hold() {
    assert!(
        WAIT * 4 < RuntimeConfig::default().idle_hold,
        "teslim beklemesi idle_hold'dan çok küçük olmalı (testin anlamı)"
    );
    let hub = MemoryHub::new();
    let (mut alice, clock) = alice_node(&hub);
    let mut bob = RawPeer::new(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.sink_addr());
    let to = alice.data_addr();
    let (a0, b0) = establish(&mut alice, &mut bob, &hub);
    old_session_traffic(&mut alice, &mut bob, a0, b0, 2);

    // Yenileme (Alice başlatır): Crypto `RenewRequired(a0)` → adaptör INIT'i yola çıkarır.
    clock.advance(SESSION_MAX_AGE);
    assert_eq!(next_event(&mut alice), SessionEvent::RenewRequired(a0));
    assert_eq!(
        hub.relay(),
        1,
        "yenileme INIT'i → Bob, HANDLE_ASSIGN → Alice"
    );
    let served = bob.serve();
    assert_eq!(served.len(), 1, "Bob yenilemeyi yanıtlar (provisional)");
    let b1 = served[0];
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a1, skipped) = ready(&mut alice, id(BOB));
    assert_ne!(a1, a0);
    assert!(skipped.is_empty(), "cutover tek Ready üretir: {skipped:?}");

    // Yeni oturumun birimi AYNI adresten → yeni yuva `(S, a1)`; `idle_hold` beklenmez.
    let wire = bob.data(b1, 0, "yeni");
    bob.send(&wire, to);
    assert_eq!(expect_delivery(&mut alice), delivered(a1, "yeni", 0));

    // Eski oturum yalnız-alım drain'de: aynı adresten gelen birimi KENDİ oturumuyla teslim edilir.
    let wire = bob.data(b0, 2, "eski");
    bob.send(&wire, to);
    assert_eq!(expect_delivery(&mut alice), delivered(a0, "eski", 2));
    let mut both = vec![a0, a1];
    both.sort();
    assert_eq!(alice.reorder_sessions_for_test(), both);

    // Drain `T_g` ile biter → `Closed(a0, Superseded)` okunur → `a0`'ın sıralama durumu silinir.
    clock.advance(GRACE_T_G);
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::Closed(a0, CloseReason::Superseded)
    );
    assert_eq!(alice.reorder_sessions_for_test(), [a1]);
    let wire = bob.data(b1, 1, "yeni");
    bob.send(&wire, to);
    assert_eq!(expect_delivery(&mut alice), delivered(a1, "yeni", 1));
    assert_quiet(&mut alice, "tek teslim");
    assert_eq!(alice.ctx_dispatch_stats(), CtxDispatchStats::default());
}

/// [K4G-05c · not 3.1 / D.2 / D.5] **Oturum geçişinde sıra başı rezervi.** Yenilemede eski oturum
/// (`a0`, yalnız-alım drain) ve yeni oturum (`a1`) veri alıcısında BİRLİKTE beklenir; her biri
/// kendi sıra başı rezervini tutar. Alice'in teslim kanalı küçüktür (C = 4 → R = min(`max_peers`,
/// C − 1) = 3) ve uygulaması DURUR (S1 çağrısı yok; adaptör kanalı boşaltmaz). Bob önce `a0`'ın
/// sırasız birimlerini yollar: kanal F − 1 ≥ R − U sınırına dek dolar, kalanı ACK'siz reddedilir
/// (`full_refusals`). Sonra DOLU kanalda `a1`'in ve `a0`'ın sıra başı birimleri rezervle kabul
/// edilir. Her adımda U ≤ R ve F ≥ R − U (değişmez I). Uygulama dönünce iki oturum da sırayla
/// teslim edilir, reddedilenler yeniden gönderimle (ARQ'nun tel karşılığı) gelir; sonda U = 0,
/// F = C.
#[test]
fn renewal_old_and_new_sessions_keep_their_head_of_line_reserve() {
    const QUEUE: usize = 4;
    let hub = MemoryHub::new();
    let (mut alice, clock) = alice_node_with_queue(&hub, Some(QUEUE));
    let mut bob = RawPeer::new(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.sink_addr());
    let to = alice.data_addr();
    let (a0, b0) = establish(&mut alice, &mut bob, &hub);
    // `a0`'ın kabul tabanı 2 olur (birim 0 ve 1 teslim edildi).
    old_session_traffic(&mut alice, &mut bob, a0, b0, 2);
    clock.advance(SESSION_MAX_AGE);
    assert_eq!(next_event(&mut alice), SessionEvent::RenewRequired(a0));
    assert_eq!(hub.relay(), 1, "yenileme INIT'i → Bob");
    let served = bob.serve();
    assert_eq!(served.len(), 1, "Bob yenilemeyi yanıtlar");
    let b1 = served[0];
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a1, _) = ready(&mut alice, id(BOB));
    assert_ne!(a1, a0);

    // ── Durak: Alice'e S1 çağrısı YOK (gözlem `drive` çağırmaz). ──
    let base = alice.delivery_reserve_for_test();
    assert_eq!(
        (base.capacity, base.reserve, base.in_use, base.free),
        (QUEUE, QUEUE - 1, 0, QUEUE),
        "ön koşul: boş kanal, R = min(max_peers, C − 1)"
    );
    let invariant = |alice: &Node, step: &str| {
        let r = alice.delivery_reserve_for_test();
        assert!(
            r.in_use <= r.reserve && r.free >= r.reserve - r.in_use,
            "{step}: U ≤ R ve F ≥ R − U olmalı — {r:?}"
        );
        r
    };
    let wait_for = |alice: &Node, what: &str, done: &dyn Fn(&Node) -> bool| {
        let deadline = Instant::now() + WAIT;
        while !done(alice) {
            assert!(Instant::now() < deadline, "{what}: {WAIT:?} içinde olmadı");
            std::thread::sleep(TICK);
        }
    };
    let pushed = |n: u64| move |a: &Node| a.delivery_reserve_for_test().pushed == base.pushed + n;
    let refused = |n: u64| move |a: &Node| a.ctx_dispatch_stats().full_refusals == n;

    // `a0`'ın sırasız birimleri (taban 2 eksik): 3 kabul (F − 1 = 3 ≥ R − U = 3), 4 ve 5 reddedilir.
    let wire = bob.data(b0, 3, "eski");
    bob.send(&wire, to);
    wait_for(&alice, "a0 birim 3 kabul", &pushed(1));
    invariant(&alice, "a0/3");
    let wire = bob.data(b0, 4, "eski");
    bob.send(&wire, to);
    wait_for(&alice, "a0 birim 4 ret", &refused(1));
    let wire = bob.data(b0, 5, "eski");
    bob.send(&wire, to);
    wait_for(&alice, "a0 birim 5 ret", &refused(2));
    let r = invariant(&alice, "a0 sırasızlar");
    assert_eq!((r.in_use, r.free), (0, QUEUE - 1), "rezerv için yer kaldı");

    // DOLU kanalda iki oturumun sıra başı rezervle kabul edilir.
    let wire = bob.data(b1, 0, "yeni");
    bob.send(&wire, to);
    wait_for(&alice, "a1 sıra başı kabul", &pushed(2));
    let r = invariant(&alice, "a1/0");
    assert_eq!(r.in_use, 1, "a1'in rezervi kullanımda");
    let wire = bob.data(b0, 2, "eski");
    bob.send(&wire, to);
    wait_for(&alice, "a0 sıra başı kabul", &pushed(3));
    let r = invariant(&alice, "a0/2");
    assert_eq!(r.in_use, 2, "iki oturumun rezervi birlikte kullanımda");
    assert_eq!(
        alice.ctx_dispatch_stats().full_refusals,
        2,
        "sıra başı birimler reddedilmedi"
    );

    // ── Uygulama döner: iki oturum sırayla; reddedilenler yeniden gönderimle. ──
    assert_eq!(expect_delivery(&mut alice), delivered(a1, "yeni", 0));
    assert_eq!(expect_delivery(&mut alice), delivered(a0, "eski", 2));
    assert_eq!(expect_delivery(&mut alice), delivered(a0, "eski", 3));
    for u in [4, 5] {
        let wire = bob.data(b0, u, "eski");
        bob.send(&wire, to);
        assert_eq!(expect_delivery(&mut alice), delivered(a0, "eski", u));
    }
    assert_quiet(&mut alice, "tek teslim");
    let end = invariant(&alice, "son");
    assert_eq!(
        (end.in_use, end.free),
        (0, QUEUE),
        "tüketimden sonra rezerv serbest, kanal boş"
    );
    let stats = alice.ctx_dispatch_stats();
    assert_eq!(
        (stats.reorder_skip, stats.reorder_window_violations),
        (0, 0),
        "{stats:?}"
    );
}

/// **Kart NE 3(c) — adres-sahteli kopya datagram:** S1'de TEK teslim (çift teslim yok).
///
/// Saldırı (hattaki saldırgan): Bob'un geçerli datagramını (birim 0) gözler ve sahte bir adresten
/// (X) ÖNCE iletir. Kripto ilk kopyayı açar; oturum beklenen ve `Ready` → `(X, a0)` yuvası açılır
/// (beyanlı açık: `max_peers` içinde ek yuva), birim teslim edilir, ACK X'e gider. Gerçek kopya (S)
/// replay penceresinde düşer; ACK almayan Bob birimi yeniden yollar (yeni PN, S) → `(S, a0)` yuvası
/// birimi veri alıcısından İKİNCİ kez geçirir. Bariyer: Bob'un birim 1'i — aynı soketten (S)
/// sırayla işlendiği için teslim edildiğinde yeniden gönderim de işlenmiştir.
///
/// 5dbd0b80'de KIRMIZI: (ii) `Reorder` ADRES anahtarlıydı → X'in ve S'nin sıralaması ayrı;
/// S'ninkinde `next = 0` olduğundan yeniden gönderilen birim 0 S1'e İKİNCİ kez teslim edilir
/// (teslim sırası 0, 0, 1 — ikinci teslim iddiası kırmızı). Oturum anahtarlı sıralamada ikinci
/// kopya `unit < next` ile tekilleşir: 0, 1.
#[test]
fn c_an_address_spoofed_copy_is_delivered_to_s1_once() {
    let hub = MemoryHub::new();
    let (mut alice, _clock) = alice_node(&hub);
    let mut bob = RawPeer::new(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.sink_addr());
    let to = alice.data_addr();
    let (a0, b0) = establish(&mut alice, &mut bob, &hub);
    let spoofed = std_udp();

    let observed = bob.data(b0, 0, "veri");
    spoofed.send_to(&observed, to).expect("sahte adresten");
    assert_eq!(expect_delivery(&mut alice), delivered(a0, "veri", 0));
    // Gerçek kopya (replay penceresinde düşer), ACK almayan Bob'un yeniden gönderimi, bariyer.
    bob.send(&observed, to);
    let retransmit = bob.data(b0, 0, "veri");
    bob.send(&retransmit, to);
    let barrier = bob.data(b0, 1, "veri");
    bob.send(&barrier, to);
    assert_eq!(
        expect_delivery(&mut alice),
        delivered(a0, "veri", 1),
        "birim 0 S1'e ikinci kez teslim edilmemeli"
    );
    assert_quiet(&mut alice, "tek teslim");
    assert_eq!(alice.ctx_dispatch_stats(), CtxDispatchStats::default());
}

// ── Sunucu düzeyi: gerçek `ServerEndpoint`, oturum tablosunu test yayımlar ──────────────────

/// `SessionControl` el sıkışması (`a` başlatır, `b` yanıtlar) + iki uca aynı hub tutamacı.
/// Döner: (`a`'daki tutamaç, `b`'deki tutamaç).
fn handshake(
    a: &mut NoiseWireProtection,
    b: &mut NoiseWireProtection,
    b_id: PeerRef,
    hub: HubHandle,
) -> (SessionHandle, SessionHandle) {
    let SessionResult::Initiated {
        session: ha,
        bytes: init,
        ..
    } = a.command(SessionCommand::Initiate {
        peer: b_id,
        early_unit: None,
    })
    else {
        panic!("Initiate bekleyen oturum + INIT üretmeli");
    };
    let SessionResult::EstablishedAndReply {
        session: hb,
        bytes: resp,
        ..
    } = b.command(SessionCommand::AcceptInit { wire: init })
    else {
        panic!("AcceptInit oturum + RESPONSE üretmeli");
    };
    assert_eq!(
        a.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Established { session: ha }
    );
    for (c, h) in [(&mut *a, ha), (&mut *b, hb)] {
        assert_eq!(
            c.command(SessionCommand::AssignHandle {
                session: h,
                hub_handle: hub,
            }),
            SessionResult::NoOutput
        );
        while c.poll_event().is_some() {}
    }
    (ha, hb)
}

/// `crypto`'nun `h` oturumunda mühürlediği DATA çerçevesi (birim `unit`).
fn sealed_data(crypto: &mut NoiseWireProtection, h: SessionHandle, unit: u64) -> Vec<u8> {
    let frame = encode_data(0, unit, &payload("veri", unit));
    crypto
        .seal(ProtectionContext::Session(h), &frame)
        .expect("mühür")
        .bytes
}

async fn udp() -> UdpSocket {
    UdpSocket::bind("127.0.0.1:0").await.expect("UDP soketi")
}

async fn send(sock: &UdpSocket, wire: &[u8], to: SocketAddr) {
    sock.send_to(wire, to).await.expect("gönder");
}

async fn delivery(server: &mut ServerHandle) -> Delivery {
    tokio::time::timeout(WAIT, server.next_delivery())
        .await
        .expect("teslim zaman aşımı")
        .expect("teslim kanalı kapandı")
}

/// **Kart NE 3(d) — `max_peers` yeni anahtarla da geçerlidir:** sayılan birim `(adres, oturum)`
/// yuvasıdır; aynı adres ikinci oturuma bedava yuva vermez, aynı oturum ikinci adrese bedava yuva
/// vermez; aktif yuva ASLA çıkarılmaz; kapanmış oturumun yuvası `idle_hold`'u beklemeden yer açar.
///
/// `max_peers: 1`. Bob'un oturumu `(A, h_ab)` yuvasını alır. Aynı adresten BEKLENEN başka bir
/// oturum (Carol, `h_ac`) yeni yuva ister → tablo dolu → fail-closed ret (`dropped_new_peers`;
/// teslim yok); aynı oturumun başka adresten (B — adres-sahteli kopya sınıfı) gelen datagramı da
/// yeni yuva ister → ret, B'ye ACK dönmez. Bariyer: mevcut yuva etkilenmez (Bob'un birimi 3 teslim
/// edilir). Bob'un oturumu kapanınca (oturum tablosu `h_ab`'siz yayımlanır) yuvası erişilemez olur
/// ve yer ihtiyacında emekliye ayrılır → Carol'ın AYNI adresten gelen datagramı teslim edilir.
///
/// 5dbd0b80'de KIRMIZI: yuva ADRES anahtarlıydı → aynı adresten gelen Carol datagramı Bob'un
/// yuvasına bağ uyuşmazlığıyla düşer (`ctx_mismatch_drops`; sayaç iddiası kırmızı); Bob'un oturumu
/// kapandıktan sonra da yuva `h_ab`'ye bağlı kalır → Carol'ın datagramı `idle_hold` dolana dek
/// düşer, teslim beklemesi aşılır; `dropped_new_peers` 2 değil 1 olur.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d_max_peers_bounds_session_keyed_slots() {
    let mut alice = node(ALICE).0;
    let mut bob = node(BOB).0;
    let mut carol = node(CAROL).0;
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let (h_ac, h_ca) = handshake(&mut alice, &mut carol, id(CAROL), HUB_AC);
    let expected = ExpectedSessions::new();
    expected.insert_ready(h_ab);
    expected.insert_ready(h_ac);
    let cfg = RuntimeConfig {
        max_peers: 1,
        ..RuntimeConfig::default()
    };
    let mut server = ServerEndpoint::bind_expecting(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        cfg,
        Box::new(alice),
        None,
        expected.clone(),
        CtxDispatchCounters::new(),
    )
    .await
    .expect("veri alıcısı");
    let to = server.local_addr();
    let shared_addr = udp().await;
    let other_addr = udp().await;

    // Tek yuva: `(A, h_ab)`.
    send(&shared_addr, &sealed_data(&mut bob, h_ba, 0), to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (0, ProtectionContext::Session(h_ab)));

    // Aynı adres, başka beklenen oturum → yeni yuva → tablo dolu → ret.
    send(&shared_addr, &sealed_data(&mut carol, h_ca, 1), to).await;
    // Aynı oturum, başka adres → yeni yuva → ret.
    send(&other_addr, &sealed_data(&mut bob, h_ba, 2), to).await;
    // Bariyer (aynı sunucu soketinden sırayla işlenir): mevcut yuva etkilenmez. Teslim 0 → 3.
    send(&shared_addr, &sealed_data(&mut bob, h_ba, 3), to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (3, ProtectionContext::Session(h_ab)));
    assert!(server.try_next_delivery().is_none());
    let mut buf = [0u8; 2048];
    let reply = tokio::time::timeout(QUIET, other_addr.recv_from(&mut buf)).await;
    assert!(reply.is_err(), "reddedilen yuvaya yanıt (ACK) dönmemeli");

    // Bob'un oturumu kapanır: tablo `h_ab`'siz yayımlanır → yuvası erişilemez, yer ihtiyacında (ya
    // da bakım tick'inde) emekliye ayrılır; Carol'ın AYNI adresten gelen datagramı teslim edilir.
    expected.replace([(h_ac, Readiness::Ready)]);
    send(&shared_addr, &sealed_data(&mut carol, h_ca, 4), to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (4, ProtectionContext::Session(h_ac)));
    assert!(server.try_next_delivery().is_none());
    assert_eq!(server.ctx_dispatch_stats(), CtxDispatchStats::default());
    let ss = server.shutdown().await;
    assert_eq!(ss.dropped_new_peers, 2, "iki yeni yuva tavanda reddedildi");
    assert_eq!(
        ss.retired_idle_peers, 1,
        "kapanan oturumun yuvası emekliye ayrıldı"
    );
    assert_eq!(ss.delivered, 3);
    // Reddedilen datagramlar ALINDI ve AÇILDI: ret kriptodan değil, tavandan.
    assert_eq!((ss.open_failures, ss.unknown_frames), (0, 0));
}

/// **(e) Kapanış emekliliğinin BAKIM-TICK yolu — dolu tablo baskısı YOK** (orkestratör incelemesi).
/// Üründe `max_peers = 64` olduğundan kapanan oturumun yuvasını pratikte emekliye ayıran TEK yol
/// bakım tick'idir (runtime `PATIENCE_TICK` = 1 s → `retire_closed_slots`); (d) yalnız dolu-tablo
/// yolunu ölçer.
///
/// Düzenek: `max_peers: 3` ve en çok İKİ yuva (Bob'un `(A, h_ab)`'si — sonra onun taze hâli — ve
/// Carol'ın bariyer yuvası `(B, h_ac)`) → tablo HİÇ dolmaz, dolu-tablo yolu HİÇ çalışmaz
/// (`dropped_new_peers == 0` doğrular); `idle_hold` ürün değeri (105 s) → idle emekliliği de
/// çalışmaz. Bob'un birimi 0 teslim edilir (oturumun kabul durumuna girer — K4G-05c: oturum
/// anahtarlı, yuvalarıyla AYNI adımda silinir; eski belge: "yuvanın teslim-dedup penceresi"), sonra oturum
/// tablosu `h_ab`'siz yayımlanır (oturum kapandı); bundan sonra Bob'un yuvasını emekliye
/// ayırabilecek tek yol bakım tick'idir.
///
/// Gözlem — olay koşullu, sabit uyku YOK: `ServerStats` yalnız kapanışta okunur; yuvanın varlığı
/// tekrarlanan bir SONDAYLA olaydan okunur. Her turda `h_ab` YALNIZ sonda süresince yeniden
/// yayımlanır, Bob birim 0'ı yeni PN'le yeniden yollar, ardından Carol'ın yuvasından bir bariyer
/// gelir (aynı sunucu soketinden sırayla işlenir: bariyer teslim edildiğinde sonda işlenmiştir).
/// Yuva hâlâ duruyorsa birim 0 oturumun kabul durumunda Duplicate olarak yutulur (teslim yok) ve `h_ab` yeniden
/// kaldırılır; yuva emekliye ayrılmışsa sonda TAZE bir yuva açar ve birim 0 yeniden teslim edilir —
/// bekleme koşulu budur (üst sınır `WAIT` ≫ 1 s). Turlar arasındaki `PROBE_PACE` aralığında `h_ab`
/// beklenmez; sonda penceresi bir bariyer gidiş-dönüşü kadar kısadır. Sonda taze yuvayı açtıktan
/// sonra `h_ab` beklenmeye devam eder (taze yuva emekliye ayrılmaz) → `retired_idle_peers == 1`
/// kesindir.
///
/// YALNIZ bakım-tick çağrısı silinince KIRMIZI: yuva hiçbir yoldan emekliye ayrılmaz (tablo dolmaz,
/// `idle_hold` dolmaz) → her sonda yutulur → üst sınır aşılır. Dolu-tablo yolundaki çağrı silinirse
/// YEŞİL kalır (o yol bu testte hiç çalışmaz). 5dbd0b80'de de KIRMIZI (kapanış emekliliği yoktu).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e_maintenance_tick_retires_a_closed_session_slot_without_table_pressure() {
    let mut alice = node(ALICE).0;
    let mut bob = node(BOB).0;
    let mut carol = node(CAROL).0;
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let (h_ac, h_ca) = handshake(&mut alice, &mut carol, id(CAROL), HUB_AC);
    let expected = ExpectedSessions::new();
    expected.insert_ready(h_ab);
    expected.insert_ready(h_ac);
    let cfg = RuntimeConfig {
        max_peers: 3,
        ..RuntimeConfig::default()
    };
    let mut server = ServerEndpoint::bind_expecting(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        cfg,
        Box::new(alice),
        None,
        expected.clone(),
        CtxDispatchCounters::new(),
    )
    .await
    .expect("veri alıcısı");
    let to = server.local_addr();
    let bob_addr = udp().await;
    let carol_addr = udp().await;

    // Bob'un yuvası `(A, h_ab)`: birim 0 teslim edilir ve oturumun kabul durumuna girer.
    send(&bob_addr, &sealed_data(&mut bob, h_ba, 0), to).await;
    let d = delivery(&mut server).await;
    assert_eq!((d.unit_id, d.ctx), (0, ProtectionContext::Session(h_ab)));

    // Bob'un oturumu kapanır: tablo yalnız Carol'la yayımlanır. Yeni datagram yollanmadan bakım
    // tick'i yuvayı emekliye ayırmalı; sonda turları bunu olaydan okur.
    let carol_only = [(h_ac, Readiness::Ready)];
    expected.replace(carol_only);
    let deadline = Instant::now() + WAIT;
    let mut barrier = 0;
    loop {
        tokio::time::sleep(PROBE_PACE).await;
        expected.insert_ready(h_ab);
        send(&bob_addr, &sealed_data(&mut bob, h_ba, 0), to).await;
        barrier += 1;
        send(&carol_addr, &sealed_data(&mut carol, h_ca, barrier), to).await;
        let mut fresh_slot = false;
        loop {
            let d = delivery(&mut server).await;
            if d.ctx == ProtectionContext::Session(h_ac) {
                assert_eq!(d.unit_id, barrier, "bariyer sırası");
                break;
            }
            assert_eq!((d.unit_id, d.ctx), (0, ProtectionContext::Session(h_ab)));
            fresh_slot = true;
        }
        if fresh_slot {
            break;
        }
        expected.replace(carol_only);
        assert!(
            Instant::now() < deadline,
            "kapanan oturumun yuvası bakım tick'inde emekliye ayrılmadı (sonda hep yutuldu)"
        );
    }
    assert!(server.try_next_delivery().is_none());
    // Sondalar `h_ab` beklenirken işlendi (bariyer bekleniyor): hiçbir datagram kapıda düşmedi.
    assert_eq!(server.ctx_dispatch_stats(), CtxDispatchStats::default());
    let ss = server.shutdown().await;
    assert_eq!(
        ss.retired_idle_peers, 1,
        "kapanan oturumun yuvası bakım tick'inde emekliye ayrıldı"
    );
    assert_eq!(
        ss.dropped_new_peers, 0,
        "tavan hiç dolmadı: dolu-tablo yolu çalışmadı"
    );
    assert_eq!((ss.open_failures, ss.unknown_frames), (0, 0));
}

// ── K4G-05c · R2: sunucu düzeyinde kabul kuralı (not 3.1, D.2, D.5) ──────────────────────────

/// Dönüş soketinde bekleyen datagramları `QUIET` sessizliğe dek boşaltır.
async fn drain_socket(sock: &UdpSocket) {
    let mut buf = [0u8; 2048];
    while let Ok(Ok(_)) = tokio::time::timeout(QUIET, sock.recv_from(&mut buf)).await {}
}

/// `sock`'a `WAIT` içinde gelen ilk datagram `crypto` ile açılır ve ACK çerçevesi mi (V1/V2).
async fn next_is_ack(sock: &UdpSocket, crypto: &mut NoiseWireProtection) -> bool {
    let mut buf = [0u8; 2048];
    let Ok(Ok((n, _))) = tokio::time::timeout(WAIT, sock.recv_from(&mut buf)).await else {
        return false;
    };
    crypto.open(&buf[..n]).is_some_and(|o| {
        o.frame.first().is_some_and(|&t| {
            t == adaptive_transport::ack::FRAME_TYPE_ACK
                || t == adaptive_transport::ack::FRAME_TYPE_ACK_V2
        })
    })
}

/// [K4G-05c · not D.2 / D.5 — "kanal rezervi" + "kontrol yolu"] Bir oturumun seli teslim kanalını
/// doldururken BAŞKA bir oturumun sıra başı birimi kabul edilir (sıra başı rezervi); dolu kanalda
/// kontrol yolu çalışır (kabul edilmiş birimin tekrarı ACK'lenir, `mark_t0` döner).
///
/// C = 4 → R = min(`max_peers`, C − 1) = 3; tüketici durur. Bob 0..8 yollar: 0 sıra başı
/// (rezervle), 1 kabul (F − 1 = 2 ≥ R − U = 2), 2..7 ACK'siz ret (`full_refusals`). Carol'ın
/// sıra başı birimi 0 DOLU kanalda rezervle kabul edilir. Eski davranışta Bob'un seli kanalın
/// tamamını alırdı. Her adımda U ≤ R ve F ≥ R − U.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_channel_keeps_another_sessions_head_and_the_control_path() {
    const QUEUE: usize = 4;
    const FLOOD: u64 = 8;
    let mut alice = node(ALICE).0;
    let mut bob = node(BOB).0;
    let mut carol = node(CAROL).0;
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let (h_ac, h_ca) = handshake(&mut alice, &mut carol, id(CAROL), HUB_AC);
    let expected = ExpectedSessions::new();
    expected.insert_ready(h_ab);
    expected.insert_ready(h_ac);
    let cfg = RuntimeConfig {
        delivery_queue: QUEUE,
        ..RuntimeConfig::default()
    };
    let mut server = ServerEndpoint::bind_expecting(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        cfg,
        Box::new(alice),
        None,
        expected.clone(),
        CtxDispatchCounters::new(),
    )
    .await
    .expect("veri alıcısı");
    let to = server.local_addr();
    let bob_addr = udp().await;
    let carol_addr = udp().await;
    let invariant = |server: &ServerHandle, step: &str| {
        let r = server.delivery_reserve_for_test();
        assert!(
            r.in_use <= r.reserve && r.free >= r.reserve - r.in_use,
            "{step}: U ≤ R ve F ≥ R − U olmalı — {r:?}"
        );
        r
    };
    async fn settle(server: &ServerHandle, done: impl Fn(&ServerHandle) -> bool, what: &str) {
        let deadline = Instant::now() + WAIT;
        while !done(server) {
            assert!(Instant::now() < deadline, "{what}: {WAIT:?} içinde olmadı");
            tokio::time::sleep(TICK).await;
        }
    }

    // Bob'un seli (tüketici durur).
    for u in 0..FLOOD {
        send(&bob_addr, &sealed_data(&mut bob, h_ba, u), to).await;
    }
    settle(
        &server,
        |s| s.ctx_dispatch_stats().full_refusals == FLOOD - 2,
        "Bob'un seli kanalı rezerv sınırına dek doldurmalı",
    )
    .await;
    let r = invariant(&server, "sel");
    assert_eq!((r.pushed, r.in_use), (2, 1), "Bob: sıra başı + bir sırasız");

    // Carol'ın sıra başı DOLU kanalda kabul edilir.
    send(&carol_addr, &sealed_data(&mut carol, h_ca, 0), to).await;
    settle(
        &server,
        |s| s.delivery_reserve_for_test().pushed == 3,
        "Carol'ın sıra başı kabul edilmeli",
    )
    .await;
    let r = invariant(&server, "carol/0");
    assert_eq!(r.in_use, 2, "iki oturumun rezervi kullanımda");
    assert_eq!(
        server.ctx_dispatch_stats().full_refusals,
        FLOOD - 2,
        "sıra başı reddedilmedi"
    );

    // Kontrol yolu: kabul edilmiş birimin tekrarı (yeni PN) ACK'lenir; `mark_t0` döner.
    drain_socket(&bob_addr).await;
    send(&bob_addr, &sealed_data(&mut bob, h_ba, 0), to).await;
    assert!(
        next_is_ack(&bob_addr, &mut bob).await,
        "dolu kanalda kabul edilmiş birimin tekrarı ACK'lenmeli (kontrol yolu)"
    );
    tokio::time::timeout(WAIT, server.mark_t0())
        .await
        .expect("dolu kanalda mark_t0 dönmeli");

    // Tüketici döner: FIFO sırası; sonra rezerv serbest.
    for (unit, ctx) in [
        (0, ProtectionContext::Session(h_ab)),
        (1, ProtectionContext::Session(h_ab)),
        (0, ProtectionContext::Session(h_ac)),
    ] {
        let d = delivery(&mut server).await;
        assert_eq!((d.unit_id, d.ctx), (unit, ctx));
    }
    assert!(server.try_next_delivery().is_none());
    let r = invariant(&server, "son");
    assert_eq!((r.in_use, r.free), (0, QUEUE));
    let stats = server.ctx_dispatch_stats();
    assert_eq!(stats.reorder_skip, 0, "{stats:?}");
    let ss = server.shutdown().await;
    assert_eq!(ss.delivery_dropped, 0, "{ss:?}");
}

/// [K4G-05c · not 3.1 / D.5 — "pencere ihlali"] Oturumun kimlik penceresinin dışındaki birim
/// (`unit − floor > max_inflight_units`) ACK'siz reddedilir (`reorder_skip`), alıcı motoruna ve
/// kanala girmez; kabul durumu onu kaydetmez — pencere içine girince aynı birim kabul edilir.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unit_outside_the_session_window_is_refused_without_ack_then_accepted() {
    const WINDOW: usize = 4;
    let mut alice = node(ALICE).0;
    let mut bob = node(BOB).0;
    let (h_ab, h_ba) = handshake(&mut alice, &mut bob, id(BOB), HUB_AB);
    let cfg = RuntimeConfig {
        max_inflight_units: WINDOW,
        ..RuntimeConfig::default()
    };
    let mut server = ServerEndpoint::bind_expecting(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        cfg,
        Box::new(alice),
        None,
        ExpectedSessions::only(h_ab),
        CtxDispatchCounters::new(),
    )
    .await
    .expect("veri alıcısı");
    let to = server.local_addr();
    let bob_addr = udp().await;
    let far = WINDOW as u64 + 1;

    send(&bob_addr, &sealed_data(&mut bob, h_ba, far), to).await;
    let deadline = Instant::now() + WAIT;
    while server.ctx_dispatch_stats().reorder_skip == 0 {
        assert!(Instant::now() < deadline, "pencere dışı birim reddedilmeli");
        tokio::time::sleep(TICK).await;
    }
    let mut buf = [0u8; 2048];
    let reply = tokio::time::timeout(QUIET, bob_addr.recv_from(&mut buf)).await;
    assert!(reply.is_err(), "pencere dışı birim ACK'lenmemeli");
    assert!(server.try_next_delivery().is_none(), "kanala girmemeli");

    for u in 0..far {
        send(&bob_addr, &sealed_data(&mut bob, h_ba, u), to).await;
    }
    for u in 0..far {
        let d = delivery(&mut server).await;
        assert_eq!(d.unit_id, u, "pencere içi birimler kabul edilir");
    }
    // Taban `far`'a geldi: aynı birim (yeni PN) artık pencere içinde.
    send(&bob_addr, &sealed_data(&mut bob, h_ba, far), to).await;
    assert_eq!(delivery(&mut server).await.unit_id, far);
    let stats = server.ctx_dispatch_stats();
    assert_eq!(
        (stats.reorder_skip, stats.full_refusals),
        (1, 0),
        "{stats:?}"
    );
    let ss = server.shutdown().await;
    assert_eq!(ss.delivered, far + 1, "{ss:?}");
}
