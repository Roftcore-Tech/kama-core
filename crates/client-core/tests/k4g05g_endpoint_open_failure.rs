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

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use client_core::memory_link::MemoryHub;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    AbandonReason, CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered,
    DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent, TransportError,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::{EnvelopeKind, SESSION_MAX_AGE};

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

/// Beklenen bir olayın (S1 olayı, teslim) ÜST SINIRI. ARQ yeniden gönderimi gerçek saatle işler
/// (kuruluş probu 0,5 s'den, PTO 1 s'den başlar ve katlanır).
const WAIT: Duration = Duration::from_secs(15);
/// "Gelmedi" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// Yoklama adımı.
const TICK: Duration = Duration::from_millis(2);

type Node = ClientTransport<NoiseWireProtection>;

// ── Fikstür ──────────────────────────────────────────────────────────────────────────────────

/// Sentetik test kimliği.
fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

/// Ürün builder'ı + ürün veri profili (kapanış boşaltması kısaltılmış) + saatin test kopyası.
fn node(me: u8, hub: &MemoryHub) -> (Node, ManualClock) {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    let (crypto, clock) = fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy);
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
