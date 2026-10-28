//! **K4G-05g — sahte `Ready`** (CEVAP PAKETİ 14 §5) kabul testleri.
//!
//! KUSUR (main 1b1425d2): `ClientTransport::activate` oturum ucu açılamasa da (`open_endpoint` →
//! `None`: eşin veri adresi yok ya da `ClientEndpoint::connect_session_counted` hatası) oturumu
//! `Active` kaydedip `Ready(h, eş)` yayınlıyordu: uçsuz oturumun her `send`'i `NotConnected`,
//! `early_unit` tele çıkmadığı hâlde Application'da biletine göre "h'ye gönderildi" sayılıyordu
//! (kama-app `adopt`: niyet bu oturumla çözüldüyse `sent_on = Some(h)`).
//!
//! DÜZENEK — iki gerçek S1 adaptörü (`ClientTransport`, ürün builder'ı ve ürün veri profili; yalnız
//! kapanış boşaltması kısaltılmış), gerçek `NoiseWireProtection` (TEST anahtarları
//! `kama_noise_wire::seam_mock::fixture_*`, GİZLİ DEĞİL), UDP loopback veri yolu ve elle sürülen
//! `MemoryHub` rölesi. Soket kurulum hatası `set_endpoint_open_failures_for_test` ile belirlenimli
//! üretilir (adres denetiminden SONRA; ürün derlemesinde kanca yoktur); adres eksikliği gerçek
//! yoldur (test 2). Bekleme süreleri yalnız beklenen olayın ÜST SINIRIDIR (ARQ yeniden gönderimi
//! gerçek saatle işler).
//!
//! Testler (kabul 1–3): (1) başlatan niyet — uç açılamaz → `ConnectAbandoned(bilet,
//! LocalPermanent)`, `Ready` yok, erken metin tele çıkmaz, Crypto oturumu kapanır; yeniden
//! `connect` normal `Ready` ve yeni erken metnin teslimiyle biter. (2) yanıtlayan, eşin adresi yok
//! → S1 olayı yok (tutamaç Application'a hiç verilmedi), sayaç, Crypto oturumu kapanır; adres
//! eklenince `connect` toparlar. (3) yenileme cutover'ı — uç açılamazken yanıt tutulur, `Ready(h')`
//! yok, eski oturum İKİ yönde çalışır; uç açılınca tek `Ready(h')`. (4) glare kaybeden — bilet
//! bilinir → `ConnectAbandoned`; yeniden `connect` toparlar.
//!
//! [DUR-05g-1 (a), CEVAP PAKETİ 15 §3 — S1 K7'nin TEK istisnası] Yerine geçen oturumun ucu, Crypto
//! eski oturumu zaten drain'e aldıktan sonra açılamazsa: (5) provisional onayı ve (6) yenilemenin
//! yedek kolu — yeni oturum için `Ready` yok, yeni Crypto oturumu kapalı, eski oturum tam BİR
//! `Closed(eski, Superseded)` ile biter (etkin ardıl olmadan); gerçek yeniden bağlanma
//! (`connect` → `Pending` → `Ready`) toparlar. (6)'nın düzeneği: karşı uç HAM eştir (`RawPeer`,
//! Crypto'yu test sürer) ve yenilemenin RESPONSE'unu yabancı bir hub tutamacıyla yollar — adaptör
//! yanıtı yenilemeyle eşleyemez (tutmaz, ucu önceden açmaz), Crypto adayı yine bulur.
//!
//! [DUR-05g-3 (B), CEVAP PAKETİ 16 §2 — INIT öncesi yerel soket sınaması; yardımcı önlem, kaynak
//! sınırı DEĞİL] (7) kalıcı yerel soket hatası → INIT yola çıkmaz (rölede ve karşı uçta hiçbir
//! zarf yok), niyet `ConnectAbandoned(_, LocalPermanent)` ile biter; hata kalkınca gerçek yeniden
//! bağlanma. (8) sınama geçer, açılış düşer → açılış hatası kolu aynen (sınama onun yerine
//! geçmez). (9) normal yol değişmez. (10) kalıcı niyetin taze INIT'i (`HALF_OPEN_TTL` sonrası) da
//! sınanır. Sınama hatası `set_socket_probe_failures_for_test` ile belirlenimli üretilir; uç
//! açılışını etkilemez.
//!
//! Kabul 2'nin kama-app yarısı: `ConnectAbandoned(bilet, _)` erken metni outbox'a "hiçbir tutamaca
//! gönderilmemiş" (`sent_on: None`) koyar ve eşin sıradaki oturumunda aynı `msg_id`'yle yollar
//! (`crates/kama-app/src/messaging.rs` `on_session_event` → `release_early`); kama-app'te
//! değişiklik gerekmez — sahte `Ready` kalkınca o yol kendiliğinden çalışır.
//!
//! BEYANLI BOŞLUK (DUR-05g-2): yanıtlayanın ucu açılamadığında RESPONSE zaten yoldadır; karşı uç
//! `Ready` görür ve yanıtlayan Crypto oturumunu kapattığı için yolladıkları ACK almaz. Bu testler
//! karşı ucun o oturumu hakkında İDDİA KURMAZ (olayları yalnız tüketilir); toparlanma iddiası
//! kurar.
//!
//! main 1b1425d2'de KIRMIZI: kanca o kodda yoktur; mutasyon dosyaları
//! (`k4g05g/mutasyon/`) eski davranışı bu kodun üstüne geri getirir ve hangi testin hangi
//! iddiasının kırmızı olduğunu yazar.

use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::time::{Duration, Instant};

use client_core::memory_link::{MemoryHub, MemoryLink};
use client_core::transport::{ClientTransport, ClientTransportConfig, EnvelopeLink, LinkEvent};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    AbandonReason, CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered,
    DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent, TransportError,
};
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SessionCommand, SessionControl, SessionResult,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::epoch_machine::GRACE_T_G;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::{EnvelopeKind, HALF_OPEN_TTL, SESSION_MAX_AGE};

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

/// Beklenen bir olayın (S1 olayı, teslim) ÜST SINIRI. ARQ yeniden gönderimi gerçek saatle işler
/// (kuruluş probu 0,5 s'den, PTO 1 s'den başlar ve katlanır).
const WAIT: Duration = Duration::from_secs(15);
/// "Gelmedi" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// Yoklama adımı.
const TICK: Duration = Duration::from_millis(2);
/// (6): ham eşin yenileme RESPONSE'unu yolladığı yabancı hub tutamacı — `MemoryHub` tutamaçları
/// 1'den artarak atar, bu değere test boyunca ulaşılmaz.
const FOREIGN_HUB: HubHandle = HubHandle::from_raw(0x7E57_F0F0);

type Node = ClientTransport<NoiseWireProtection>;

// ── Fikstür ──────────────────────────────────────────────────────────────────────────────────

/// Sentetik test kimliği.
fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

/// İki kimliği de bilen dizinle ürün kriptosu (`with_control`) + saatin test kopyası.
fn crypto(me: u8) -> (NoiseWireProtection, ManualClock) {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy)
}

/// Ürün builder'ı + ürün veri profili (kapanış boşaltması kısaltılmış) + saatin test kopyası.
fn node(me: u8, hub: &MemoryHub) -> (Node, ManualClock) {
    let (crypto, clock) = crypto(me);
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    let t = ClientTransport::new(cfg, crypto, Box::new(hub.link(id(me)))).expect("düğüm");
    (t, clock)
}

/// İki düğüm; `bob_knows_alice = false` iken Bob'un adres defterinde Alice yoktur.
fn pair(hub: &MemoryHub, bob_knows_alice: bool) -> (Node, ManualClock, Node) {
    let (mut alice, clock) = node(ALICE, hub);
    let (mut bob, _) = node(BOB, hub);
    alice.add_peer_addr(id(BOB), bob.data_addr());
    if bob_knows_alice {
        bob.add_peer_addr(id(ALICE), alice.data_addr());
    }
    (alice, clock, bob)
}

/// İki düğüm, iki yönde adresli, iki saat de testte: (Alice, Alice'in saati, Bob, Bob'un saati).
fn pair_with_clocks(hub: &MemoryHub) -> (Node, ManualClock, Node, ManualClock) {
    let (mut alice, alice_clock) = node(ALICE, hub);
    let (mut bob, bob_clock) = node(BOB, hub);
    alice.add_peer_addr(id(BOB), bob.data_addr());
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    (alice, alice_clock, bob, bob_clock)
}

/// HAM karşı uç (test (6)): Crypto'yu (S3) test sürer — S1 adaptörünün zarf yolundaki komutların
/// aynısı —; zarfları aynı `MemoryHub` üzerinden taşınır; veri yolu yoktur, Alice'in bu eşe açtığı
/// uçlar `sink`'e yazar.
struct RawPeer {
    crypto: NoiseWireProtection,
    link: MemoryLink,
    sink: StdUdpSocket,
}

impl RawPeer {
    fn new(me: u8, hub: &MemoryHub) -> Self {
        Self {
            crypto: crypto(me).0,
            link: hub.link(id(me)),
            sink: StdUdpSocket::bind("127.0.0.1:0").expect("UDP soketi"),
        }
    }

    fn sink_addr(&self) -> SocketAddr {
        self.sink.local_addr().expect("adres")
    }

    /// Gelen INIT'leri adaptörün zarf yoluyla AYNI komutlarla yanıtlar (`AcceptInit` +
    /// `AssignHandle` gerçek tutamaçla); RESPONSE `reply_handle` verildiyse O tutamaçla yollanır.
    /// Yanıtlanan oturumları sırayla döner.
    fn serve(&mut self, reply_handle: Option<HubHandle>) -> Vec<SessionHandle> {
        let mut out = Vec::new();
        while let Some(ev) = self.link.poll() {
            let LinkEvent::IncomingInit {
                hub_handle, bytes, ..
            } = ev
            else {
                continue; // ham eş başlatmaz: HandleAssigned / Response gelmez
            };
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
                .send_response(
                    peer,
                    reply_handle.unwrap_or(hub_handle),
                    envelope_type,
                    reply,
                )
                .expect("zarf taşıyıcısı");
            out.push(session);
        }
        while self.crypto.poll_event().is_some() {}
        out
    }

    /// `sink`'e gelen datagramlardan biri `session` bağlamında AEAD-geçerli açılır (üst sınır
    /// `WAIT`): Alice'in o oturum için açtığı uç gerçekten mühürlüyor ve yolluyor.
    fn opens_in(&mut self, session: SessionHandle) -> bool {
        let deadline = Instant::now() + WAIT;
        self.sink
            .set_read_timeout(Some(QUIET))
            .expect("okuma zaman sınırı");
        let mut buf = [0u8; 2048];
        while Instant::now() < deadline {
            let Ok(n) = self.sink.recv(&mut buf) else {
                continue;
            };
            if let Some(opened) = self.crypto.open(&buf[..n]) {
                if opened.ctx == ProtectionContext::Session(session) {
                    return true;
                }
            }
        }
        false
    }
}

fn unit(bytes: &[u8]) -> PreparedUnit {
    PreparedUnit {
        bytes: bytes.to_vec(),
        compression_policy: CompressionPolicy::Never,
    }
}

fn pending(r: Result<ConnectResult, TransportError>) -> ConnectTicket {
    match r {
        Ok(ConnectResult::Pending(ticket)) => ticket,
        other => panic!("Pending bekleniyordu, gelen {other:?}"),
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

/// `QUIET` boyunca S1 olayı gelmez.
fn assert_no_event(t: &mut Node, why: &str) {
    let deadline = Instant::now() + QUIET;
    while Instant::now() < deadline {
        if let Some(ev) = t.poll_event() {
            panic!("{why}: beklenmeyen olay {ev:?}");
        }
        std::thread::sleep(TICK);
    }
}

/// Olayları `QUIET` boyunca yeni olay gelmeyene dek tüketir (iddia kurulmayan karşı uç için).
fn drain_events(t: &mut Node) {
    let mut deadline = Instant::now() + QUIET;
    while Instant::now() < deadline {
        if t.poll_event().is_some() {
            deadline = Instant::now() + QUIET;
        }
        std::thread::sleep(TICK);
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
fn assert_no_delivery(t: &mut Node, why: &str) {
    let deadline = Instant::now() + QUIET;
    while Instant::now() < deadline {
        if let Some(d) = t.recv() {
            panic!("{why}: beklenmeyen teslim {d:?}");
        }
        std::thread::sleep(TICK);
    }
}

/// Ucu açılamayan tutamacın S1'de ve Crypto'da izi yok: `send` → `UnknownSession` (S1 hiç
/// bilmedi), Crypto'da kurulu oturum yok (`session_epoch` → `None`; zombi kripto oturumu kalsaydı
/// `Some(_)`).
fn assert_not_activated(t: &mut Node, failed: SessionHandle) {
    assert_eq!(
        t.send(failed, unit(b"x"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession),
        "ucu açılamayan tutamaç S1'e verilmedi"
    );
    assert_eq!(
        t.session_epoch(failed),
        None,
        "ucu açılamayan oturum Crypto'da kapatılmalı (zombi kripto oturumu yok)"
    );
}

// ── Kabul testleri ───────────────────────────────────────────────────────────────────────────

/// **Kabul 1 + 2 + 3, başlatan niyet:** Alice'in ucu RESPONSE'tan sonra açılamaz.
///
/// Beklenen: Alice `Measuring` → `ConnectAbandoned(t1, LocalPermanent)` görür, `Ready` YOK; sayaç
/// 1; ucu açılamayan tutamaç S1'e verilmedi ve Crypto'da kapatıldı; `early_unit` ("erken-1") tele
/// çıkmadı (Bob hiçbir şey teslim etmez). Yeniden `connect` (yeni bilet, yeni erken metin) normal
/// `Ready(a1)` ile biter ve Bob "erken-2"yi Alice'in yeni oturumundan TEK kez teslim eder; sayaç
/// artmaz.
///
/// Eski davranışta (mutasyon M1) Alice `Ready(h, Bob)` görür: ikinci olay iddiası kırmızı.
#[test]
fn initiator_endpoint_failure_abandons_the_ticket_and_reconnect_recovers() {
    let hub = MemoryHub::new();
    let (mut alice, _clock, mut bob) = pair(&hub, true);

    alice.set_endpoint_open_failures_for_test(1);
    let t1 = pending(alice.connect(id(BOB), Some(unit(b"erken-1"))));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let (b0, _) = ready(&mut bob, id(ALICE));
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");

    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectProgress(t1, ConnectPhase::Measuring)
    );
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectAbandoned(t1, AbandonReason::LocalPermanent),
        "uç açılamadı: niyet kapalı nedenle biter, sahte Ready yok"
    );
    assert_no_event(&mut alice, "ConnectAbandoned'dan sonra olay yok");
    assert_eq!(alice.endpoint_open_failures(), 1);
    assert_eq!(alice.active_session_for_test(id(BOB)), None);
    let failed = alice
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_not_activated(&mut alice, failed);
    assert_no_delivery(&mut bob, "erken metin tele çıkmamalı");

    // Toparlanma: yeniden `connect`. Bob'da `b0` var → yeni INIT provisional; Alice'in ilk epoch-1
    // paketi onu onaylar.
    let t2 = pending(alice.connect(id(BOB), Some(unit(b"erken-2"))));
    assert_ne!(t2, t1, "yeni niyet yeni bilet");
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    bob.drive();
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a1, before) = ready(&mut alice, id(BOB));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(t2, ConnectPhase::Measuring)]
    );
    assert_ne!(a1, failed);
    let d = expect_delivery(&mut bob);
    assert_eq!(d.unit, b"erken-2");
    assert_ne!(d.session, b0, "yeni oturumdan teslim");
    assert_no_delivery(&mut bob, "erken-1 hiç, erken-2 tek kez");
    assert_eq!(
        alice.endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
}

/// **Kabul 1 + 3, yanıtlayan (niyet yok), gerçek yol:** Bob'un adres defterinde Alice yok.
///
/// Beklenen: Bob INIT'i yanıtlar ama ucu açılamaz → Bob'da HİÇBİR S1 olayı yok (tutamaç
/// Application'a hiç verilmedi: S1 K2/K7), sayaç 1, Crypto oturumu kapatıldı. Adres eklenince
/// Bob'un `connect`'i normal `Ready` ile biter ve erken metni Alice'in yeni oturumunda teslim
/// edilir.
///
/// Eski davranışta (mutasyon M1) Bob `Ready(h, Alice)` görür: ilk olay iddiası kırmızı; mutasyon
/// M2'de (Crypto kapatılmaz) `session_epoch` iddiası kırmızı.
#[test]
fn responder_without_the_peer_address_emits_no_event_and_connect_recovers() {
    let hub = MemoryHub::new();
    let (mut alice, _clock, mut bob) = pair(&hub, false);

    let _ = pending(alice.connect(id(BOB), Some(unit(b"alice-erken"))));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    bob.drive();
    assert_no_event(&mut bob, "ucu açılamayan yanıtlayan oturum S1'e açılmaz");
    assert_eq!(bob.endpoint_open_failures(), 1);
    assert_eq!(bob.active_session_for_test(id(ALICE)), None);
    let failed = bob
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_not_activated(&mut bob, failed);

    // Karşı uç (DUR-05g-2): RESPONSE yoldaydı; Alice'in o oturumu hakkında iddia yok.
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    drain_events(&mut alice);
    let a0 = alice.active_session_for_test(id(BOB));
    assert_no_delivery(&mut bob, "kapatılan oturumdan teslim yok");

    // Toparlanma: adres eklenir, Bob bağlanır.
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let tb = pending(bob.connect(id(ALICE), Some(unit(b"bob-geri"))));
    assert_eq!(hub.relay(), 1, "INIT → Alice, HANDLE_ASSIGN → Bob");
    alice.drive();
    assert_eq!(hub.relay(), 1, "RESPONSE → Bob");
    let (b1, before) = ready(&mut bob, id(ALICE));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(tb, ConnectPhase::Measuring)]
    );
    assert_ne!(b1, failed);
    let d = expect_delivery(&mut alice);
    assert_eq!(d.unit, b"bob-geri");
    assert_ne!(Some(d.session), a0, "yeni oturumdan teslim");
    assert_eq!(
        bob.endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
}

/// **Kabul 1, yenileme cutover'ı:** Alice'in yenileme ucu açılamaz.
///
/// Beklenen: RESPONSE Crypto'ya VERİLMEZ (uç `AcceptResponse`'tan önce açılır) → `Ready(h')` yok,
/// her `drive` bir deneme sayar; eski oturum (`a0`/`b0`) İKİ yönde teslim eder — Crypto onu drain'e
/// almadı. Uç açılabilir olunca tek `Ready(a1)`; `send(a0)` → `SessionSuperseded`; Bob yeni
/// oturumdan teslim eder.
///
/// Mutasyon M3'te (uç açılmadan `AcceptResponse`) Alice uçsuz `Ready(a1)` görür ve Crypto `a0`'ın
/// gönderimini kapatır: "Ready(h') yok" iddiası kırmızı.
#[test]
fn renewal_cutover_waits_for_the_endpoint_and_the_old_session_keeps_working() {
    let hub = MemoryHub::new();
    let (mut alice, clock, mut bob) = pair(&hub, true);
    let _ = pending(alice.connect(id(BOB), None));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let (b0, _) = ready(&mut bob, id(ALICE));
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a0, _) = ready(&mut alice, id(BOB));

    // Yenileme: Crypto `RenewRequired(a0)` → adaptör INIT'i yola çıkarır.
    alice.set_endpoint_open_failures_for_test(usize::MAX);
    clock.advance(SESSION_MAX_AGE);
    assert_eq!(next_event(&mut alice), SessionEvent::RenewRequired(a0));
    assert_eq!(
        hub.relay(),
        1,
        "yenileme INIT'i → Bob, HANDLE_ASSIGN → Alice"
    );
    bob.drive();
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice (tutulur)");
    // `a0` boş (teyitsiz birim yok) → her `drive` cutover'ı dener; uç açılamaz.
    alice.drive();
    alice.drive();
    assert_eq!(
        alice.endpoint_open_failures(),
        2,
        "her drive bir uç açma denemesi"
    );
    assert_no_event(&mut alice, "uç açılmadan cutover yok: Ready(h') yok");
    assert_eq!(alice.active_session_for_test(id(BOB)), Some(a0));

    // Eski oturum İKİ yönde çalışır (Crypto `a0`'ı drain'e almadı, Bob'da `b0` tam çalışıyor).
    alice
        .send(a0, unit(b"eski-ileri"), DeliveryPolicy::Reliable)
        .expect("eski oturum gönderim kabul eder");
    assert_eq!(
        expect_delivery(&mut bob),
        Delivered {
            session: b0,
            unit: b"eski-ileri".to_vec(),
        }
    );
    bob.send(b0, unit(b"eski-geri"), DeliveryPolicy::Reliable)
        .expect("Bob'un eski oturumu");
    assert_eq!(
        expect_delivery(&mut alice),
        Delivered {
            session: a0,
            unit: b"eski-geri".to_vec(),
        }
    );

    // Uç açılabilir: cutover tamamlanır.
    alice.set_endpoint_open_failures_for_test(0);
    let failures = alice.endpoint_open_failures();
    let (a1, before) = ready(&mut alice, id(BOB));
    assert!(before.is_empty(), "cutover tek Ready üretir: {before:?}");
    assert_ne!(a1, a0);
    assert_eq!(alice.endpoint_open_failures(), failures, "uç açıldı");
    assert_eq!(
        alice.send(a0, unit(b"x"), DeliveryPolicy::Reliable),
        Err(TransportError::SessionSuperseded)
    );
    alice
        .send(a1, unit(b"yeni"), DeliveryPolicy::Reliable)
        .expect("yeni oturum");
    let d = expect_delivery(&mut bob);
    assert_eq!(d.unit, b"yeni");
    assert_ne!(d.session, b0, "yeni oturumdan teslim");
}

/// **Kabul 1 + 2 + 3, glare kaybeden:** iki taraf aynı anda bağlanır; kaybeden Bob'un (dest_hash'i
/// büyük; identity §16) ucu açılamaz.
///
/// Beklenen: Bob `Measuring` → `ConnectAbandoned(tb, LocalPermanent)`, `Ready` YOK; sayaç 1; Crypto
/// oturumu kapatıldı; Bob'un erken metni ("bob-erken-1") tele çıkmadı. Yeniden `connect` normal
/// `Ready` ile biter ve yeni erken metin Alice'te tek kez teslim edilir.
///
/// Mutasyon M4'te (glare'de bilet olayı yok) ikinci olay iddiası (`ConnectAbandoned`) kırmızı.
#[test]
fn glare_loser_endpoint_failure_abandons_its_ticket_and_reconnect_recovers() {
    assert!(
        id(ALICE) < id(BOB),
        "tie-break: Alice kazanır, Bob kaybeder"
    );
    let hub = MemoryHub::new();
    let (mut alice, _clock, mut bob) = pair(&hub, true);

    bob.set_endpoint_open_failures_for_test(1);
    let _ = pending(alice.connect(id(BOB), Some(unit(b"alice-erken"))));
    let tb = pending(bob.connect(id(ALICE), Some(unit(b"bob-erken-1"))));
    assert_eq!(hub.relay(), 2, "iki INIT çapraz");
    alice.drive(); // kazanan: Bob'un INIT'i sessiz düşer
    bob.drive(); // kaybeden: bekleyeni silinir, Alice'in INIT'i yanıtlanır, uç açılamaz
    assert_eq!(
        next_event(&mut bob),
        SessionEvent::ConnectProgress(tb, ConnectPhase::Measuring)
    );
    assert_eq!(
        next_event(&mut bob),
        SessionEvent::ConnectAbandoned(tb, AbandonReason::LocalPermanent),
        "glare kaybında bilet bilinir: niyet kapalı nedenle biter"
    );
    assert_no_event(&mut bob, "sahte Ready yok");
    assert_eq!(bob.endpoint_open_failures(), 1);
    assert_eq!(bob.active_session_for_test(id(ALICE)), None);
    let failed = bob
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_not_activated(&mut bob, failed);

    // Karşı uç (DUR-05g-2): Alice'in o oturumu hakkında iddia yok.
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    drain_events(&mut alice);
    let a0 = alice.active_session_for_test(id(BOB));
    assert_no_delivery(&mut alice, "bob-erken-1 tele çıkmamalı");

    // Toparlanma: Bob yeniden bağlanır (Alice'te `a0` var → provisional).
    let tb2 = pending(bob.connect(id(ALICE), Some(unit(b"bob-erken-2"))));
    assert_ne!(tb2, tb);
    assert_eq!(hub.relay(), 1, "INIT → Alice, HANDLE_ASSIGN → Bob");
    alice.drive();
    assert_eq!(hub.relay(), 1, "RESPONSE → Bob");
    let (b1, before) = ready(&mut bob, id(ALICE));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(tb2, ConnectPhase::Measuring)]
    );
    assert_ne!(b1, failed);
    let d = expect_delivery(&mut alice);
    assert_eq!(d.unit, b"bob-erken-2");
    assert_ne!(Some(d.session), a0, "yeni oturumdan teslim");
    assert_no_delivery(&mut alice, "bob-erken-1 hiç, bob-erken-2 tek kez");
    assert_eq!(
        bob.endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
}

/// **DUR-05g-1 (a), provisional onayı kolu:** Alice yeniler; Bob'da provisional `b1` doğar ve
/// Alice'in ilk epoch-1 paketiyle Crypto'da onaylanır (eski `b0` Crypto'da drain'e düşer) — ama
/// Bob'un `b1` ucu açılamaz.
///
/// Beklenen (Bob): `Ready(b1)` YOK; `b0` tam BİR `Closed(b0, Superseded)` ile biter (Crypto'nun
/// drain sınırı `T_g` geçse de ikincisi yok); sayaç 1; `b1` S1'e verilmedi ve Crypto'da kapalı;
/// `b0` da Crypto'da kapalı. Toparlanma: Bob `connect` → `Pending` → `Ready(b2)` ve erken metni
/// Alice'in yeni oturumundan teslim edilir.
///
/// Mutasyon M5'te (uçsuz `Ready` geri) ilk olay `Ready(b1, Alice)`: `Closed` iddiası kırmızı;
/// M7'de (eski oturum kapatılmaz) `Closed` gelmez: "S1 olayı gelmedi".
#[test]
fn provisional_confirm_endpoint_failure_ends_the_old_session_and_reconnect_recovers() {
    let hub = MemoryHub::new();
    let (mut alice, alice_clock, mut bob, bob_clock) = pair_with_clocks(&hub);
    let _ = pending(alice.connect(id(BOB), None));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let (b0, _) = ready(&mut bob, id(ALICE));
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a0, _) = ready(&mut alice, id(BOB));

    // Alice yeniler: Bob'da `b0` var → yenileme INIT'i provisional `b1` doğurur.
    alice_clock.advance(SESSION_MAX_AGE);
    assert_eq!(next_event(&mut alice), SessionEvent::RenewRequired(a0));
    assert_eq!(
        hub.relay(),
        1,
        "yenileme INIT'i → Bob, HANDLE_ASSIGN → Alice"
    );
    bob.drive();
    // Bob'un sıradaki uç açma denemesi = `b1`'in onayı.
    bob.set_endpoint_open_failures_for_test(1);
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a1, _) = ready(&mut alice, id(BOB));
    alice
        .send(a1, unit(b"onay"), DeliveryPolicy::Reliable)
        .expect("Alice'in yeni oturumu");

    assert_eq!(
        next_event(&mut bob),
        SessionEvent::Closed(b0, CloseReason::Superseded),
        "ardılın ucu açılamadı: eski oturum etkin ardıl olmadan Superseded ile biter"
    );
    assert_no_event(&mut bob, "yeni oturum için Ready yok");
    assert_eq!(bob.endpoint_open_failures(), 1);
    assert_eq!(bob.active_session_for_test(id(ALICE)), None);
    let b1 = bob
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_ne!(b1, b0);
    assert_not_activated(&mut bob, b1);
    assert_eq!(
        bob.send(b0, unit(b"x"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession)
    );
    assert_eq!(bob.session_epoch(b0), None, "eski oturum Crypto'da kapandı");
    bob_clock.advance(GRACE_T_G);
    assert_no_event(&mut bob, "tam bir Closed: drain sınırı ikincisini üretmez");

    // Gerçek yeniden bağlanma: Alice'te `a1` var → Bob'un INIT'i orada provisional.
    let tb = pending(bob.connect(id(ALICE), Some(unit(b"bob-geri"))));
    assert_eq!(hub.relay(), 1, "INIT → Alice, HANDLE_ASSIGN → Bob");
    alice.drive();
    assert_eq!(hub.relay(), 1, "RESPONSE → Bob");
    let (b2, before) = ready(&mut bob, id(ALICE));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(tb, ConnectPhase::Measuring)]
    );
    assert_ne!(b2, b1);
    let d = expect_delivery(&mut alice);
    assert_eq!(d.unit, b"bob-geri");
    assert_ne!(d.session, a1, "Alice'in yeni oturumundan teslim");
    assert_eq!(
        bob.endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
}

/// **DUR-05g-1 (a), yenilemenin yedek kolu:** Alice'in yenileme RESPONSE'u adaptörde yenilemeyle
/// eşleşmez (ham eş onu yabancı hub tutamacıyla yollar) → tutulmaz, uç önceden açılmaz; Crypto
/// adayı bulur, `Established` der ve `a0`'ı drain'e alır; `on_crypto_ready` ucu açamaz.
///
/// Beklenen (Alice): `Ready(h')` YOK; `a0` tam BİR `Closed(a0, Superseded)` ile biter; sayaç 1
/// (tek deneme); `h'` S1'e verilmedi ve Crypto'da kapalı, `a0` da. Toparlanma: `connect` →
/// `Pending` → `Ready(a2)`; Alice'in yeni ucu Bob'un yeni oturumu bağlamında mühürler.
///
/// Mutasyon M6'da (uçsuz `Ready` geri) ilk olay `Ready(h', Bob)`: `Closed` iddiası kırmızı.
#[test]
fn renewal_fallback_endpoint_failure_ends_the_old_session_and_reconnect_recovers() {
    let hub = MemoryHub::new();
    let (mut alice, clock) = node(ALICE, &hub);
    let mut bob = RawPeer::new(BOB, &hub);
    alice.add_peer_addr(id(BOB), bob.sink_addr());
    let _ = pending(alice.connect(id(BOB), None));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    assert_eq!(bob.serve(None).len(), 1, "Bob INIT'i yanıtlar");
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a0, _) = ready(&mut alice, id(BOB));

    alice.set_endpoint_open_failures_for_test(usize::MAX);
    clock.advance(SESSION_MAX_AGE);
    assert_eq!(next_event(&mut alice), SessionEvent::RenewRequired(a0));
    assert_eq!(
        hub.relay(),
        1,
        "yenileme INIT'i → Bob, HANDLE_ASSIGN → Alice"
    );
    assert_eq!(
        bob.serve(Some(FOREIGN_HUB)).len(),
        1,
        "Bob yenilemeyi yanıtlar (provisional), RESPONSE yabancı tutamaçla"
    );
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");

    assert_eq!(
        next_event(&mut alice),
        SessionEvent::Closed(a0, CloseReason::Superseded),
        "ardılın ucu açılamadı: eski oturum etkin ardıl olmadan Superseded ile biter"
    );
    assert_no_event(&mut alice, "yeni oturum için Ready yok");
    assert_eq!(alice.endpoint_open_failures(), 1, "yedek kol tek deneme");
    assert_eq!(alice.active_session_for_test(id(BOB)), None);
    let failed = alice
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_ne!(failed, a0);
    assert_not_activated(&mut alice, failed);
    assert_eq!(
        alice.send(a0, unit(b"x"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession)
    );
    assert_eq!(
        alice.session_epoch(a0),
        None,
        "eski oturum Crypto'da kapandı"
    );
    clock.advance(GRACE_T_G);
    assert_no_event(
        &mut alice,
        "tam bir Closed: drain sınırı ikincisini üretmez",
    );

    // Gerçek yeniden bağlanma.
    alice.set_endpoint_open_failures_for_test(0);
    let t = pending(alice.connect(id(BOB), None));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let served = bob.serve(None);
    assert_eq!(served.len(), 1, "Bob yeni INIT'i yanıtlar");
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a2, before) = ready(&mut alice, id(BOB));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(t, ConnectPhase::Measuring)]
    );
    assert_ne!(a2, failed);
    alice
        .send(a2, unit(b"yeni"), DeliveryPolicy::Reliable)
        .expect("yeni oturum gönderim kabul eder");
    assert!(
        bob.opens_in(served[0]),
        "yeni uç Bob'un yeni oturumu bağlamında mühürlemeli"
    );
    assert_eq!(
        alice.endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
}

// ── DUR-05g-3 (B): INIT öncesi yerel soket sınaması ─────────────────────────────────────────

/// **(7) Kalıcı yerel soket hatası → INIT yok; hata kalkınca gerçek yeniden bağlanma.**
///
/// Beklenen: Alice `Measuring` → `ConnectAbandoned(t1, LocalPermanent)`; röle ve bütün kutular
/// boş (INIT ve HANDLE_ASSIGN yok: karşı uca hiçbir zarf gitmedi, karşı uçta kayıt oluşamaz),
/// Bob'da olay yok; `local_probe_failures` 1, `endpoint_open_failures` 0, Crypto oturumu hiç
/// açılmadı. Yeniden `connect` → `Pending` → `Ready`, erken metin ("erken-2") tek kez teslim.
///
/// Mutasyon M8'de (`connect` sınamayı atlar) INIT yola çıkar: `ConnectAbandoned` gelmez — ikinci
/// `next_event` WAIT içinde düşer; M10'da (sınama hatası olaysız) aynı iddia kırmızı.
#[test]
fn local_socket_probe_failure_sends_no_init_and_reconnect_recovers() {
    let hub = MemoryHub::new();
    let (mut alice, _clock, mut bob) = pair(&hub, true);

    alice.set_socket_probe_failures_for_test(1);
    let t1 = pending(alice.connect(id(BOB), Some(unit(b"erken-1"))));
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectProgress(t1, ConnectPhase::Measuring)
    );
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectAbandoned(t1, AbandonReason::LocalPermanent),
        "yerel soket sınaması düştü: niyet kapalı nedenle biter"
    );
    assert_no_event(&mut alice, "ConnectAbandoned'dan sonra olay yok");
    assert!(
        hub.is_idle(),
        "INIT yola çıkmadı: rölede ve kutularda zarf yok"
    );
    assert_eq!(hub.relay(), 0, "taşınacak zarf yok");
    assert_no_event(&mut bob, "karşı uca hiçbir şey ulaşmadı");
    assert_eq!(bob.active_session_for_test(id(ALICE)), None);
    assert_eq!(alice.local_probe_failures(), 1);
    assert_eq!(alice.endpoint_open_failures(), 0, "uç açılışı denenmedi");
    assert_eq!(
        alice.last_failed_activation_for_test(),
        None,
        "Crypto oturumu hiç açılmadı"
    );

    // Hata kalktı (kanca tek seferlikti): gerçek yeniden bağlanma.
    let t2 = pending(alice.connect(id(BOB), Some(unit(b"erken-2"))));
    assert_ne!(t2, t1);
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let (b0, _) = ready(&mut bob, id(ALICE));
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (_, before) = ready(&mut alice, id(BOB));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(t2, ConnectPhase::Measuring)]
    );
    assert_eq!(
        expect_delivery(&mut bob),
        Delivered {
            session: b0,
            unit: b"erken-2".to_vec(),
        }
    );
    assert_no_delivery(&mut bob, "erken-1 hiç, erken-2 tek kez");
    assert_eq!(
        alice.local_probe_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
}

/// **(8) Sınama geçer, açılış düşer → açılış hatası kolu aynen.**
///
/// Beklenen: sınama başarılı (`local_probe_failures` 0) → INIT yola çıkar (röle 1); uç açılamaz →
/// `ConnectAbandoned(t, LocalPermanent)` tam bir kez, `Ready` yok; `endpoint_open_failures` 1;
/// Crypto oturumu kapalı. Sınama açılış hatası kolunun YERİNE geçmez.
///
/// Mutasyon M1'de (açılış kolunda uçsuz `Ready`) ikinci olay `Ready`: kırmızı.
#[test]
fn probe_passes_but_endpoint_open_fails_keeps_the_open_failure_path() {
    let hub = MemoryHub::new();
    let (mut alice, _clock, mut bob) = pair(&hub, true);

    alice.set_endpoint_open_failures_for_test(1);
    let t = pending(alice.connect(id(BOB), Some(unit(b"erken"))));
    assert_eq!(hub.relay(), 1, "sınama geçti: INIT yola çıktı");
    let _ = ready(&mut bob, id(ALICE));
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectProgress(t, ConnectPhase::Measuring)
    );
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectAbandoned(t, AbandonReason::LocalPermanent)
    );
    assert_no_event(&mut alice, "tam bir kapanış, Ready yok");
    assert_eq!(alice.local_probe_failures(), 0, "sınama geçti");
    assert_eq!(alice.endpoint_open_failures(), 1, "açılış düştü");
    let failed = alice
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_not_activated(&mut alice, failed);
}

/// **(9) Normal yol değişmez.**
///
/// Beklenen: `Measuring` → `Ready(a0)` (arada başka olay yok), iki sayaç 0, veri iki yönde teslim.
/// Sınama her `connect`'te bir kez koşar ve normal yolda gözlenebilir hiçbir şey değiştirmez.
#[test]
fn normal_connect_with_a_passing_probe_is_unchanged() {
    let hub = MemoryHub::new();
    let (mut alice, _clock, mut bob) = pair(&hub, true);
    let t = pending(alice.connect(id(BOB), None));
    assert_eq!(hub.relay(), 1, "INIT → Bob, HANDLE_ASSIGN → Alice");
    let (b0, _) = ready(&mut bob, id(ALICE));
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let (a0, before) = ready(&mut alice, id(BOB));
    assert_eq!(
        before,
        [SessionEvent::ConnectProgress(t, ConnectPhase::Measuring)]
    );
    assert_eq!(
        (alice.local_probe_failures(), alice.endpoint_open_failures()),
        (0, 0)
    );
    alice
        .send(a0, unit(b"ileri"), DeliveryPolicy::Reliable)
        .expect("gönder");
    assert_eq!(
        expect_delivery(&mut bob),
        Delivered {
            session: b0,
            unit: b"ileri".to_vec(),
        }
    );
    bob.send(b0, unit(b"geri"), DeliveryPolicy::Reliable)
        .expect("gönder");
    assert_eq!(
        expect_delivery(&mut alice),
        Delivered {
            session: a0,
            unit: b"geri".to_vec(),
        }
    );
    assert_no_event(&mut alice, "normal yolda ek olay yok");
}

/// **(10) Kalıcı niyetin taze INIT'i de sınanır.** Alice'in INIT'i röleden hiç geçmez; Crypto
/// `HALF_OPEN_TTL` sonra bekleyen kaydı düşürür → adaptör niyeti sürdürmek için taze INIT
/// başlatacakken yerel sınama düşer.
///
/// Beklenen: `Measuring` → `ConnectAbandoned(t, LocalPermanent)`; rölede yalnız İLK INIT vardır
/// (taze INIT yok); `local_probe_failures` 1. Mutasyon M9'da (`reinitiate` sınamayı atlar) taze
/// INIT yola çıkar ve niyet sürer: `ConnectAbandoned` gelmez, kırmızı.
#[test]
fn persistent_intent_reinit_probe_failure_abandons_without_a_fresh_init() {
    let hub = MemoryHub::new();
    let (mut alice, clock, mut bob) = pair(&hub, true);
    let t = pending(alice.connect(id(BOB), None));
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectProgress(t, ConnectPhase::Measuring)
    );

    alice.set_socket_probe_failures_for_test(1);
    clock.advance(HALF_OPEN_TTL);
    assert_eq!(
        next_event(&mut alice),
        SessionEvent::ConnectAbandoned(t, AbandonReason::LocalPermanent),
        "taze INIT öncesi sınama düştü: niyet kapalı nedenle biter"
    );
    assert_no_event(&mut alice, "ConnectAbandoned'dan sonra olay yok");
    assert_eq!(alice.local_probe_failures(), 1);
    assert_eq!(hub.relay(), 1, "rölede yalnız ilk INIT: taze INIT yok");
    // Bob bayat INIT'i işler; yanıtı gelse de Alice'te bekleyen kayıt yok (opak `Failed`).
    bob.drive();
    let _ = hub.relay();
    assert_no_event(&mut alice, "bayat yanıt niyeti diriltmez");
    assert_eq!(alice.active_session_for_test(id(BOB)), None);
}
