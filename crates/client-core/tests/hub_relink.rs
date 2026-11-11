//! [HLR] S4 C5 — hub hattı kaybında kurulu oturumun yeni INIT'le yenilenmesi
//! (`ClientTransport::relink`, `ClientTransport::restart_renewal`) ve eş başına TEK etkin oturum.
//!
//! DÜZENEK — belirlenimli zarf sırası. İki gerçek `ClientTransport` (ürün builder'ı; gerçek
//! `NoiseWireProtection`, TEST anahtarları `kama_noise_wire::seam_mock::fixture_*`, GİZLİ DEĞİL) +
//! gerçek veri yolu (UDP loopback, direct-mode). INIT/RESPONSE zarfları `MemoryHub` üzerinden YALNIZ
//! `relay()` çağrısında taşınır: glare ve geç INIT sıraları testin elindedir. `ROUTE_MISS`'in
//! kendisi burada yoktur — `relink` bileşim kökünün `HubNotice::HandleLost`'a cevabıdır; gerçek
//! hub üstünden uçtan uca yol `kama-integration/tests/s4_hub_relink.rs`'tedir.
//!
//! FEC iki uçta KAPALI: yeni oturum ucu kendiliğinden PARAM_SET yollamaz, eşteki provisional'ın
//! onayı (ilk epoch-1 birimi) testin açık bir `send`'idir — Application'ın oturum değişiminde
//! RECEIPT'siz kaydı yeni oturumdan yeniden yollamasının karşılığı. Kimlikler: `id(ALICE) <
//! id(BOB)` → S3 glare'ini (K10) Alice kazanır. Bekleme süreleri yalnız beklenen olayın ÜST
//! sınırıdır.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use client_core::hub::relink::{NoticeRoute, RelinkGovernor, RelinkOp, RELINK_SEED_LEN};
use client_core::hub::HubNotice;
use client_core::memory_link::MemoryHub;
use client_core::transport::{ActiveLink, ClientTransport, ClientTransportConfig, RelinkOutcome};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent,
    TrafficClass, TransportError,
};
use kama_contracts::s3::HubHandle;
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::epoch_machine::GRACE_T_G;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::{EnvelopeKind, SESSION_MAX_AGE};

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
/// Dizinde olan ama oturumu olmayan eş.
const CAROL: u8 = 0xC0;

/// Beklenen olayın (S1 olayı, teslim) üst sınırı — gerçek zaman (ARQ yeniden gönderimi PTO ile).
const WAIT: Duration = Duration::from_secs(15);
/// Yoklama adımı.
const TICK: Duration = Duration::from_millis(2);
/// "Olmadı" iddialarının penceresi.
const QUIET: Duration = Duration::from_millis(300);
/// Testin yenileme boşaltma tavanı (kısa: eski oturumda teyitsiz birim kalsa da cutover hızlı).
const RENEW_DRAIN_CAP: Duration = Duration::from_millis(200);
/// `relay` + olay okuma turlarının tavanı (`settle`).
const SETTLE_ROUNDS: usize = 16;

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn unit(bytes: &[u8]) -> PreparedUnit {
    PreparedUnit {
        bytes: bytes.to_vec(),
        compression_policy: CompressionPolicy::Never,
    }
}

struct Node {
    t: ClientTransport<NoiseWireProtection>,
    clock: ManualClock,
    me: PeerRef,
    /// Okunmuş, testin henüz tüketmediği S1 olayları.
    events: Vec<SessionEvent>,
    /// `establish`'ten sonra okunan bütün `Ready`'lerin oturumları (sayım iddiaları).
    readies: Vec<SessionHandle>,
}

fn node(me: u8, hub: &MemoryHub) -> Node {
    let ids = [ALICE, BOB, CAROL].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    let (crypto, clock) = fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy);
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    cfg.renew_drain_cap = RENEW_DRAIN_CAP;
    let t = ClientTransport::new(cfg, crypto, Box::new(hub.link(id(me)))).expect("düğüm");
    Node {
        t,
        clock,
        me: id(me),
        events: Vec::new(),
        readies: Vec::new(),
    }
}

impl Node {
    /// Bekleyen S1 olaylarını okur (Transport'u da sürer).
    fn pump(&mut self) {
        while let Some(ev) = self.t.poll_event() {
            if let SessionEvent::Ready(h, _) = ev {
                self.readies.push(h);
            }
            self.events.push(ev);
        }
    }

    /// `pick`'in seçtiği ilk olayı bekler ve tüketir.
    fn wait<T>(&mut self, what: &str, pick: impl Fn(&SessionEvent) -> Option<T>) -> T {
        let deadline = Instant::now() + WAIT;
        loop {
            self.pump();
            if let Some(i) = self.events.iter().position(|e| pick(e).is_some()) {
                let ev = self.events.remove(i);
                if let Some(v) = pick(&ev) {
                    return v;
                }
            }
            assert!(Instant::now() < deadline, "{what}");
            std::thread::sleep(TICK);
        }
    }

    fn wait_ready(&mut self, peer: PeerRef) -> SessionHandle {
        self.wait("Ready gelmedi", |e| match *e {
            SessionEvent::Ready(h, p) if p == peer => Some(h),
            _ => None,
        })
    }

    fn wait_event(&mut self, expected: SessionEvent) {
        self.wait(&format!("{expected:?} gelmedi"), |e| {
            (*e == expected).then_some(())
        });
    }

    fn send(&mut self, session: SessionHandle, bytes: &[u8]) -> Result<(), TransportError> {
        self.t.send(
            session,
            unit(bytes),
            DeliveryPolicy::Reliable,
            TrafficClass::Be,
        )
    }

    /// Sıradaki teslim `session`'da `bytes` olmalı.
    fn expect_delivery(&mut self, session: SessionHandle, bytes: &[u8]) {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(d) = self.t.recv() {
                assert_eq!(
                    (d.session, d.unit.as_slice()),
                    (session, bytes),
                    "teslim oturumu ve baytı"
                );
                return;
            }
            assert!(
                Instant::now() < deadline,
                "teslim gelmedi: {}",
                String::from_utf8_lossy(bytes)
            );
            std::thread::sleep(TICK);
        }
    }

    /// Okunmuş olaylarda `RenewRequired` ya da `Closed` yok (yenileme Application'a görünmez).
    fn assert_no_close_or_renew_event(&mut self) {
        self.pump();
        assert!(
            !self
                .events
                .iter()
                .any(|e| matches!(e, SessionEvent::RenewRequired(_) | SessionEvent::Closed(..))),
            "beklenmeyen olay: {:?}",
            self.events
        );
    }
}

fn pair(hub: &MemoryHub) -> (Node, Node) {
    let mut a = node(ALICE, hub);
    let mut b = node(BOB, hub);
    a.t.add_peer_addr(b.me, b.t.data_addr());
    b.t.add_peer_addr(a.me, a.t.data_addr());
    (a, b)
}

/// Alice başlatır, Bob yanıtlar; iki yönde birer birim. Olay kayıtları temizlenir.
fn establish(a: &mut Node, b: &mut Node, hub: &MemoryHub) -> (SessionHandle, SessionHandle) {
    let Ok(ConnectResult::Pending(_)) = a.t.connect(b.me, None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    assert_eq!(hub.relay(), 1, "INIT → Bob");
    let b0 = b.wait_ready(a.me);
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a0 = a.wait_ready(b.me);
    a.send(a0, b"a0").expect("gönder");
    b.expect_delivery(b0, b"a0");
    b.send(b0, b"b0").expect("gönder");
    a.expect_delivery(a0, b"b0");
    for n in [&mut *a, &mut *b] {
        n.pump();
        n.events.clear();
        n.readies.clear();
    }
    (a0, b0)
}

/// Röle ve iki ucun olay okuması sessizleşene dek.
fn settle(a: &mut Node, b: &mut Node, hub: &MemoryHub) {
    for _ in 0..SETTLE_ROUNDS {
        a.pump();
        b.pump();
        if hub.relay() == 0 {
            return;
        }
    }
    panic!("zarf rölesi sessizleşmedi");
}

/// Yeni çiftin iki yönde veri taşıdığı, eski oturumların drain sonunda `Superseded` kapandığı ve iki
/// uçta tek etkin oturum / bekleyen yenileme yok / onaysız provisional artığı yok olduğu.
fn assert_one_session(
    a: &mut Node,
    b: &mut Node,
    (a0, a1): (SessionHandle, SessionHandle),
    (b0, b1): (SessionHandle, SessionHandle),
) {
    b.send(b1, b"b1").expect("gönder");
    a.expect_delivery(a1, b"b1");
    a.send(a1, b"a1-2").expect("gönder");
    b.expect_delivery(b1, b"a1-2");
    assert_eq!(
        a.t.renewal_state_for_test(b.me),
        (1, 0, 0),
        "Alice: tek etkin oturum, artık yok"
    );
    assert_eq!(
        b.t.renewal_state_for_test(a.me),
        (1, 0, 0),
        "Bob: tek etkin oturum, artık yok"
    );
    a.clock.advance(GRACE_T_G);
    b.clock.advance(GRACE_T_G);
    a.wait_event(SessionEvent::Closed(a0, CloseReason::Superseded));
    b.wait_event(SessionEvent::Closed(b0, CloseReason::Superseded));
    a.assert_no_close_or_renew_event();
    b.assert_no_close_or_renew_event();
}

/// Kaybolan hat → `relink` → yenileme INIT'i → (Bob'da provisional) → RESPONSE → atomik cutover:
/// `Ready(a1)`, eski oturum `SessionSuperseded`; Bob'un yeni oturumu Alice'in ilk epoch-1 birimiyle
/// onaylanır; iki yönde veri yeni çiftte; eski oturumlar drain sonunda `Closed(_, Superseded)`.
/// Application olay görmez (`RenewRequired` yok). Oturumu olmayan eşe `relink` etkisiz; bekleyen
/// yenileme varken ikinci INIT yok.
#[test]
fn relink_renews_the_session_and_cuts_over() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let (a0, b0) = establish(&mut a, &mut b, &hub);

    assert_eq!(
        a.t.relink(id(CAROL)),
        RelinkOutcome::Nothing,
        "oturumu olmayan eş: yenilenecek oturum yok"
    );
    assert_eq!(a.t.relink(b.me), RelinkOutcome::Started);
    assert_eq!(
        a.t.relink(b.me),
        RelinkOutcome::Nothing,
        "bekleyen yenileme sürer"
    );
    assert_eq!(hub.relay(), 1, "TEK yenileme INIT'i");
    b.pump();
    assert!(b.events.is_empty(), "provisional: Bob'da S1 olayı yok");
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a1 = a.wait_ready(b.me);
    assert_ne!(a1, a0);
    assert_eq!(a.send(a0, b"eski"), Err(TransportError::SessionSuperseded));

    a.send(a1, b"a1").expect("yeni oturumda gönder");
    let b1 = b.wait_ready(a.me);
    assert_ne!(b1, b0);
    b.expect_delivery(b1, b"a1");
    assert_one_session(&mut a, &mut b, (a0, a1), (b0, b1));
    assert_eq!(
        (a.readies.as_slice(), b.readies.as_slice()),
        ([a1].as_slice(), [b1].as_slice())
    );
}

/// Bekleyen yenilemenin hub denemesi biterse (`AttemptEnded`) `restart_renewal` onu yeni
/// kriptografik INIT ile yeniden başlatır: eski INIT'in taşıyıcı tekrarı durdurulur, geç RESPONSE'u
/// hiçbir oturum açmaz (Crypto eski bekleyeni kapattı), yeni oturum TEK; Bob'da eski INIT'in
/// provisional'ı yeni oturum açılınca kapanır (artık kalmaz). Yenileme yokken etkisiz.
#[test]
fn restart_renewal_replaces_the_unanswered_renewal() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let (a0, b0) = establish(&mut a, &mut b, &hub);

    assert_eq!(
        a.t.restart_renewal(b.me),
        RelinkOutcome::Nothing,
        "yenileme yok"
    );
    assert_eq!(a.t.relink(b.me), RelinkOutcome::Started);
    assert_eq!(a.t.restart_renewal(b.me), RelinkOutcome::Started);
    assert_eq!(hub.cancelled_inits().len(), 1, "eski INIT'in tekrarı durdu");
    assert_eq!(hub.relay(), 2, "eski (gitmiş) ve yeni INIT → Bob");
    b.pump();
    assert!(b.events.is_empty(), "iki provisional: Bob'da S1 olayı yok");
    assert_eq!(hub.relay(), 2, "iki RESPONSE → Alice");
    let a1 = a.wait_ready(b.me);
    a.send(a1, b"a1").expect("gönder");
    let b1 = b.wait_ready(a.me);
    b.expect_delivery(b1, b"a1");
    assert_one_session(&mut a, &mut b, (a0, a1), (b0, b1));
    assert_eq!(
        (a.readies.as_slice(), b.readies.as_slice()),
        ([a1].as_slice(), [b1].as_slice())
    );
    assert_eq!(
        a.t.restart_renewal(b.me),
        RelinkOutcome::Nothing,
        "yenileme bitti"
    );
}

/// [HLR] Taşıyıcı INIT'i almazsa (hub deneme tavanı dolu) yenileme KAYDEDİLMEZ (`Deferred`):
/// bekleyen yenileme yok sayılmaz, sonraki `relink` yeni INIT başlatır. ESKİ DAVRANIŞ: taşıyıcısız
/// yenileme kaydediliyordu — hiçbir `AttemptEnded` gelmez, `relink` onu "bekliyor" sayardı; oturum
/// `HALF_OPEN_TTL`'ye dek yeni INIT'siz kalırdı.
#[test]
fn relink_whose_init_the_carrier_refuses_is_not_recorded() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let (a0, b0) = establish(&mut a, &mut b, &hub);

    hub.refuse_next_inits(1);
    assert_eq!(a.t.relink(b.me), RelinkOutcome::Deferred);
    assert_eq!(
        a.t.renewal_state_for_test(b.me),
        (1, 0, 0),
        "kayıt yok, eski oturum sürer"
    );
    assert_eq!(hub.relay(), 0, "INIT yok");
    assert_eq!(a.t.relink(b.me), RelinkOutcome::Started, "yeniden deneme");
    assert_eq!(hub.relay(), 1, "INIT → Bob");
    b.pump();
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a1 = a.wait_ready(b.me);
    a.send(a1, b"a1").expect("gönder");
    let b1 = b.wait_ready(a.me);
    b.expect_delivery(b1, b"a1");
    assert_one_session(&mut a, &mut b, (a0, a1), (b0, b1));
}

/// İki uç da aynı anda hattını kaybetti (iki uç da `ROUTE_MISS` aldı): iki yenileme INIT'i çapraz
/// gider. S3 glare'i (K10): Alice kazanır (Bob'un INIT'i sessiz düşer), Bob kaybeder (bekleyeni
/// silinir, Alice'in INIT'i provisional olur). Bob'un sahipsiz yenilemesi provisional onaylanınca
/// kapatılır. Sonuç: iki uçta tek etkin oturum, bekleyen yenileme yok, iki yönde veri.
#[test]
fn both_ends_relink_at_once_and_end_in_one_session() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let (a0, b0) = establish(&mut a, &mut b, &hub);

    assert_eq!(a.t.relink(b.me), RelinkOutcome::Started);
    assert_eq!(b.t.relink(a.me), RelinkOutcome::Started);
    assert_eq!(hub.relay(), 2, "iki INIT çapraz");
    a.pump();
    b.pump();
    assert_eq!(hub.relay(), 1, "yalnız glare'i kaybeden (Bob) yanıtlar");
    let a1 = a.wait_ready(b.me);
    a.send(a1, b"a1").expect("gönder");
    let b1 = b.wait_ready(a.me);
    b.expect_delivery(b1, b"a1");
    assert_one_session(&mut a, &mut b, (a0, a1), (b0, b1));
    assert_eq!(
        (a.readies.as_slice(), b.readies.as_slice()),
        ([a1].as_slice(), [b1].as_slice())
    );
    assert_eq!(
        hub.cancelled_inits().len(),
        1,
        "glare'i kaybedenin (Bob) terk edilen yenilemesinin tekrarı durdu"
    );
}

/// Eş, bizim yenileme INIT'imizi yanıtladıktan SONRA (provisional'ı henüz onaysız) kendi hattını
/// kaybedip `relink` eder; INIT'i, bizim RESPONSE'umuz tutulurken (Crypto'da bekleyenimiz varken)
/// gelir → glare. İki sıra da (önce glare'i kazanan Alice ya da kaybeden Bob yeniler) tek oturumda
/// biter: Alice'in yenilemesi kalır, Bob'un yenilemesi provisional onaylanınca kapatılır.
fn peer_relinks_after_answering_our_renewal(alice_first: bool) {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let (a0, b0) = establish(&mut a, &mut b, &hub);
    {
        let (x, y) = if alice_first {
            (&mut a, &mut b)
        } else {
            (&mut b, &mut a)
        };
        assert_eq!(x.t.relink(y.me), RelinkOutcome::Started);
        assert_eq!(hub.relay(), 1, "ilk yenileme INIT'i");
        y.pump();
        assert!(y.events.is_empty(), "provisional: S1 olayı yok");
        assert_eq!(
            y.t.relink(x.me),
            RelinkOutcome::Started,
            "provisional onaysızken eş de yeniler"
        );
    }
    settle(&mut a, &mut b, &hub);
    let a1 = a.wait_ready(b.me);
    a.send(a1, b"a1").expect("gönder");
    let b1 = b.wait_ready(a.me);
    b.expect_delivery(b1, b"a1");
    assert_one_session(&mut a, &mut b, (a0, a1), (b0, b1));
    assert_eq!(
        (a.readies.as_slice(), b.readies.as_slice()),
        ([a1].as_slice(), [b1].as_slice())
    );
    assert_eq!(
        hub.cancelled_inits().len(),
        1,
        "Bob'un terk edilen yenilemesinin tekrarı durdu"
    );
}

#[test]
fn peer_relinks_after_answering_our_renewal_glare_winner_first() {
    peer_relinks_after_answering_our_renewal(true);
}

#[test]
fn peer_relinks_after_answering_our_renewal_glare_loser_first() {
    peer_relinks_after_answering_our_renewal(false);
}

/// Eşin yenilemesi bizim cutover'ımızdan SONRA gelir: Alice yeniledi ve kesti (`a1`); Bob, Alice'in
/// onayı gelmeden kendi yenilemesini başlattı; Bob'un INIT'i Alice'e cutover'dan sonra ulaştı
/// (Alice'te `a1`'in yerini alacak provisional) ve Bob'un yenilemesi önce tamamlandı (`b1`).
/// Bob'daki (yerini `b1` almış `b0`'ın) provisional'ı AÇILMAMALI (eş başına tek etkin oturum): `b1`
/// açılınca kapanır, Alice'in `a1`'den gelen birimi onu onaylayamaz; Bob'un `b1`'den birimi Alice'i
/// `b1`'in çiftine geçirir. Hub kaybı olmadan da doğan sıra: iki uçta yaklaşık aynı anda dolan `SESSION_MAX_AGE`
/// (yaş yenilemesi; bu test onunla kurulur).
///
/// ESKİ DAVRANIŞ: onaylanan provisional, yerini yenisi almış oturum için de açılıyordu → Bob'da
/// ikinci `Ready` (aynı eşle iki etkin oturum; `connect` ve teslim hangisine?).
#[test]
fn a_peer_renewal_landing_after_our_cutover_opens_no_second_session() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let (a0, b0) = establish(&mut a, &mut b, &hub);
    // Bob'un saati yaş yenilemesinin hemen öncesine (provisional'ı yaşlandırmadan).
    b.clock.advance(SESSION_MAX_AGE - Duration::from_secs(1));
    b.pump();
    assert!(b.events.is_empty());

    // (1) Alice yeniler (yaş) ve keser.
    a.clock.advance(SESSION_MAX_AGE);
    a.wait_event(SessionEvent::RenewRequired(a0));
    assert_eq!(hub.relay(), 1, "Alice'in INIT'i → Bob");
    b.pump();
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a1 = a.wait_ready(b.me);

    // (2) Bob, onay gelmeden kendi yenilemesini başlatır; INIT'i Alice'e cutover'dan sonra varır.
    b.clock.advance(Duration::from_secs(1));
    b.wait_event(SessionEvent::RenewRequired(b0));
    assert_eq!(hub.relay(), 1, "Bob'un INIT'i → Alice");
    a.pump();
    assert_eq!(hub.relay(), 1, "RESPONSE → Bob");
    let b1 = b.wait_ready(a.me);
    assert_ne!(b1, b0);

    // (3) [HLR] b1 açılınca Bob'daki provisional (Alice'in a1'i; yerini b1 almış b0 için) kapandı:
    // Alice'in a1'den birimi onu onaylayamaz, ikinci oturum açılmaz.
    assert_eq!(
        b.t.renewal_state_for_test(a.me),
        (1, 0, 0),
        "Bob: b1 etkin, artık provisional yok"
    );
    a.send(a1, b"a1").expect("gönder");
    std::thread::sleep(QUIET);
    b.pump();
    assert_eq!(b.readies, vec![b1], "Bob'da ikinci etkin oturum açılmadı");

    // (4) Bob'un b1'den birimi Alice'teki provisional'ı onaylar → Alice b1'in çiftine geçer.
    b.send(b1, b"b1").expect("gönder");
    let a2 = a.wait_ready(b.me);
    a.expect_delivery(a2, b"b1");
    a.send(a2, b"a2").expect("gönder");
    b.expect_delivery(b1, b"a2");
    assert_eq!(a.readies, vec![a1, a2]);
    assert_eq!(b.readies, vec![b1]);
    assert_eq!(a.t.active_session_for_test(b.me), Some(a2));
    assert_eq!(b.t.active_session_for_test(a.me), Some(b1));
}

// ── [HLR-STALE] eski hattın geç hatası (sahip şartı, #180) ───────────────────────────────────

/// Yöneticinin jitter tohumu (testte sabit).
const SEED: [u8; RELINK_SEED_LEN] = [7; RELINK_SEED_LEN];

/// `kama-node` `NodeLoop::on_hub_notices` ile AYNI bileşim: bildirim, eşin etkin oturumunun hattıyla
/// (`ClientTransport::active_link`) yöneticiye verilir; yöneticinin vadesi gelen işi transport'ta
/// uygulanır (`Relink` → `relink`, `RelinkLine(h)` → `relink_line`, `Restart` →
/// `restart_renewal`).
fn govern(g: &mut RelinkGovernor, a: &mut Node, notice: HubNotice) -> NoticeRoute {
    let now = Instant::now();
    let (has_session, link) = match notice {
        HubNotice::AttemptEnded { peer } | HubNotice::HandleLost { peer, .. } => (
            a.t.active_session_for_test(peer).is_some(),
            a.t.active_link(peer),
        ),
        HubNotice::Bound | HubNotice::Disconnected => (false, ActiveLink::NoSession),
    };
    let route = g.on_notice(now, notice, has_session, link);
    g.drive(now, |p, op| match op {
        RelinkOp::Restart => a.t.restart_renewal(p),
        RelinkOp::Relink => a.t.relink(p),
        RelinkOp::RelinkLine(h) => a.t.relink_line(p, h),
    });
    route
}

/// Eşin etkin oturumunun hattı (`MemoryHub` her INIT'e handle atar: bilinmeli).
fn line(n: &Node, peer: PeerRef) -> HubHandle {
    match n.t.active_link(peer) {
        ActiveLink::On(h) => h,
        other => panic!("etkin oturumun hattı bilinmeli: {other:?}"),
    }
}

fn lost(peer: PeerRef, h: HubHandle) -> HubNotice {
    HubNotice::HandleLost { peer, h }
}

/// Alice yenilemesini keser, Bob yeni oturumu Alice'in ilk biriminde onaylar; iki uçta tek etkin
/// oturum (eski oturumlar `Superseded` kapanır). Yeni çifti döner.
fn cut_over(
    a: &mut Node,
    b: &mut Node,
    hub: &MemoryHub,
    (a0, b0): (SessionHandle, SessionHandle),
) -> (SessionHandle, SessionHandle) {
    assert_eq!(hub.relay(), 1, "yenileme INIT'i → Bob");
    b.pump();
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a1 = a.wait_ready(b.me);
    a.send(a1, b"yeni").expect("yeni oturumda gönder");
    let b1 = b.wait_ready(a.me);
    b.expect_delivery(b1, b"yeni");
    assert_one_session(a, b, (a0, a1), (b0, b1));
    (a1, b1)
}

/// [HLR-STALE] Sahip şartı (#180), transport düzeyi, iki handle: h1'de kurulu oturum → h1 kaybı →
/// yenileme → h2'de yeni oturum. Sonra ESKİ hattın geç hataları:
///
/// * `ROUTE_MISS{h1}` (`HandleLost{bob, h1}`) bildirim anında `Stale` (`stale_handle_lost` +1);
///   yöneticide bekleyen bir istek uygulama anında da aynı kuralla süzülür (`relink_line(bob, h1)`
///   = `Stale`);
/// * eski hatta ait `UNKNOWN_HANDLE`'ın bileşim köküne ulaşabilen tek biçimi `AttemptEnded{bob}`
///   → `restart_renewal` = `Nothing` (yeniden başlatılacak bekleyen yenileme yok).
///
/// Hiçbiri `started`'ı artırmaz, tele INIT çıkmaz, etkin oturum h2'de. Ardından `ROUTE_MISS{h2}`
/// tam bir yenileme: yeni INIT, cutover, iki yönde veri, tek etkin oturum.
///
/// ESKİ DAVRANIŞ: `HandleLost{bob, h1}` handle'a bakılmadan `relink(bob)`'a gidiyordu → h2'deki
/// çalışan oturum için ikinci INIT (`started` 2).
#[test]
fn late_errors_of_the_old_line_do_not_renew_the_new_session() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let first = establish(&mut a, &mut b, &hub);
    let mut g = RelinkGovernor::new(SEED);
    let h1 = line(&a, b.me);

    assert_eq!(
        govern(&mut g, &mut a, lost(b.me, h1)),
        NoticeRoute::Governed
    );
    assert_eq!(g.stats().started, 1, "h1 kaybı → yenileme");
    let second = cut_over(&mut a, &mut b, &hub, first);
    let h2 = line(&a, b.me);
    assert_ne!(h2, h1);

    assert_eq!(
        govern(&mut g, &mut a, lost(b.me, h1)),
        NoticeRoute::Stale,
        "eski hattın ROUTE_MISS'i"
    );
    assert_eq!(
        govern(&mut g, &mut a, HubNotice::AttemptEnded { peer: b.me }),
        NoticeRoute::Governed,
        "eski hatta ait UNKNOWN_HANDLE"
    );
    assert_eq!(a.t.relink_line(b.me, h1), RelinkOutcome::Stale);
    assert_eq!(a.t.restart_renewal(b.me), RelinkOutcome::Nothing);
    let st = g.stats();
    assert_eq!(
        (st.requests, st.started, st.stale_handle_lost, st.nothing),
        (2, 1, 1, 1),
        "{st:?}"
    );
    assert_eq!(hub.relay(), 0, "tele INIT çıkmadı");
    assert_eq!(a.t.active_session_for_test(b.me), Some(second.0));
    assert_eq!(a.t.renewal_state_for_test(b.me), (1, 0, 0));
    assert_eq!(line(&a, b.me), h2, "etkin oturum h2'de");

    assert_eq!(
        govern(&mut g, &mut a, lost(b.me, h2)),
        NoticeRoute::Governed
    );
    assert_eq!(g.stats().started, 2, "h2 kaybı → yenileme");
    let third = cut_over(&mut a, &mut b, &hub, second);
    assert_ne!(line(&a, b.me), h2);
    assert_eq!(
        (a.readies.as_slice(), b.readies.as_slice()),
        (
            [second.0, third.0].as_slice(),
            [second.1, third.1].as_slice()
        )
    );
}

/// [HLR-STALE] (iii) Yenileme uçuştayken (RESPONSE gelmedi; yeni hat etkin oturumun hattı değil,
/// etkin oturum hâlâ h1'de) gelen kayıp: etkin oturumun KENDİ hattına (h1) gelen bildirim SÜZÜLMEZ,
/// yöneticiye girer; `relink_line` bekleyen yenilemeyi görür, ikinci INIT yok (`Nothing`;
/// yenilemenin INIT'i yeni bir hatta gider). Etkin oturumun hattı olmayan handle `Stale`.
#[test]
fn line_loss_while_a_renewal_is_in_flight() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    let first = establish(&mut a, &mut b, &hub);
    let mut g = RelinkGovernor::new(SEED);
    let h1 = line(&a, b.me);

    // Hat kaybından bağımsız başlamış yenileme; INIT henüz rölede (yeni hattın handle'ı yok).
    assert_eq!(a.t.relink(b.me), RelinkOutcome::Started);
    assert_eq!(a.t.renewal_state_for_test(b.me), (1, 1, 0));
    assert_eq!(line(&a, b.me), h1, "uçuşta: etkin oturum h1'de");
    assert_eq!(
        govern(&mut g, &mut a, lost(b.me, h1)),
        NoticeRoute::Governed,
        "etkin oturumun kendi hattı süzülmez"
    );
    let other = HubHandle::from_raw(h1.raw().wrapping_add(0x100));
    assert_eq!(
        govern(&mut g, &mut a, lost(b.me, other)),
        NoticeRoute::Stale
    );
    let st = g.stats();
    assert_eq!(
        (st.requests, st.started, st.nothing, st.stale_handle_lost),
        (1, 0, 1, 1),
        "{st:?}"
    );
    cut_over(&mut a, &mut b, &hub, first);
}

/// [HLR-STALE] (iii) Etkin oturumun hattı BİLİNMİYORSA (taşıyıcı o oturumun INIT'ine handle
/// bildirmedi) kayıp DÜŞÜRÜLMEZ: hangi hattın kaybolduğu doğrulanamaz, düşürmek meşru bir kaybı
/// yutabilirdi (`HubLink` kaybolan hattı unutur; ikinci bildirim gelmez, oturum yeni INIT'siz
/// kalırdı). `relink_line` yeniler; yöneticide bildirim koşulsuz yenilemeye gider ve görünür
/// sayılır (`handle_lost_unverified`). Hattı bildirilmemiş oturum Crypto'da hub tutamacına
/// bağlanmadığı için (`AssignHandle` yok) veri taşımaz; burada yalnız el sıkışma kurulur, veri yeni
/// (hattı bilinen) oturumda akar.
#[test]
fn line_loss_with_an_unknown_active_line_still_renews() {
    let hub = MemoryHub::new();
    let (mut a, mut b) = pair(&hub);
    hub.withhold_next_assignments(1);
    let Ok(ConnectResult::Pending(_)) = a.t.connect(b.me, None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    assert_eq!(hub.relay(), 1, "INIT → Bob (Alice'e HandleAssigned yok)");
    let b0 = b.wait_ready(a.me);
    assert_eq!(hub.relay(), 1, "RESPONSE → Alice");
    let a0 = a.wait_ready(b.me);
    for n in [&mut a, &mut b] {
        n.pump();
        n.events.clear();
        n.readies.clear();
    }
    let first = (a0, b0);
    assert_eq!(a.t.active_link(b.me), ActiveLink::Unknown);
    let mut g = RelinkGovernor::new(SEED);
    let any = HubHandle::from_raw(0x99);

    assert_eq!(a.t.relink_line(b.me, any), RelinkOutcome::Started);
    assert_eq!(
        govern(&mut g, &mut a, lost(b.me, any)),
        NoticeRoute::Governed
    );
    let st = g.stats();
    assert_eq!(
        (
            st.requests,
            st.handle_lost_unverified,
            st.stale_handle_lost,
            st.nothing
        ),
        (1, 1, 0, 1),
        "{st:?}: koşulsuz Relink, yenileme zaten bekliyor"
    );
    cut_over(&mut a, &mut b, &hub, first);
    assert!(
        matches!(a.t.active_link(b.me), ActiveLink::On(_)),
        "yenilemenin hattı bilinir"
    );
}
