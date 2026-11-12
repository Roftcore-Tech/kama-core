//! [T3-GLARE] Bileşim düzeyi: niyet yolunda terk edilen INIT'in TEL üzerindeki T3 tekrarı
//! (S4 C3 / KR-18; #180 "Riskler": "Niyet yolunda glare'i kaybedenin hub denemesi
//! `ATTEMPT_MAX_AGE`'e dek T3 yapar").
//!
//! DÜZENEK — iki gerçek `ClientTransport` (gerçek `NoiseWireProtection`, TEST anahtarları
//! `kama_noise_wire::seam_mock::fixture_*`, GİZLİ DEĞİL; veri yolu UDP loopback, direct-mode). Zarf
//! taşıyıcısı ürünün hub kipi bileşimidir (`kama-node` `hub_mode.rs`): `RecordingLink(
//! HubEnvelopeLink(HubLink))` + `KnownHandles`; S1 `Ready` → `HubLink::peer_ready(peer,
//! known.handle_of(peer))` (`HubSide::peer_ready`, KK-H58). Hub, testin içindeki küçük bir röledir:
//! `STORE{0, INIT}` → `{OK, h}` + `DELIVER`; aynı `(gönderen, request_id)` → aynı `h` + yeniden
//! `DELIVER` (`hub-server` `relay.rs` idempotent tahsis); `STORE{h, RESPONSE}` → `{OK, h}` +
//! `DELIVER`; heartbeat → ACK. `HubLink`'in saati sahtedir (`FakeClock`): T3, heartbeat ve deneme
//! yaşı sahte saatte vadeden vadeye ilerler, tele çıkan `STORE{0, INIT}`'ler uç başına sayılır.
//! Crypto saati ayrıdır ve ilerletilmez; veri yolunun bekleme süreleri yalnız beklenen olayın ÜST
//! sınırıdır. Kimlikler: `id(ALICE) < id(BOB)` → S3 glare'ini (K10) Alice kazanır.

use std::time::Duration;

use kama_contracts::handles::SessionHandle;
use kama_contracts::s1::{
    AbandonReason, CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit,
    SessionEvent, TrafficClass,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

use super::*;
use crate::hub::endpoint::{KnownHandles, RecordingLink};
use crate::transport::{ActiveLink, ClientTransport, ClientTransportConfig, RelinkOutcome};

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
const A: usize = 0;
const B: usize = 1;

/// Veri yolunda beklenen olayın üst sınırı (gerçek zaman; ARQ yeniden gönderimi PTO ile).
const WAIT: Duration = Duration::from_secs(15);
/// Yoklama adımı.
const TICK: Duration = Duration::from_millis(2);
/// Testin yenileme boşaltma tavanı.
const RENEW_DRAIN_CAP: Duration = Duration::from_millis(200);
/// Hub turunun iç tur tavanı (her tur bir yanıt/teslim dalgası).
const HUB_ROUNDS: usize = 64;
/// Sahte saatte vadeden vadeye ilerlemenin adım tavanı (bir saatte ≤ 30 s'lik T3 ve ≥ 8 s'lik
/// heartbeat aralığı bunun çok altında kalır).
const MAX_STEPS: usize = 20_000;

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn guard(m: &Mutex<HubLink>) -> std::sync::MutexGuard<'_, HubLink> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Side {
    t: ClientTransport<NoiseWireProtection>,
    link: Arc<Mutex<HubLink>>,
    known: KnownHandles,
    me: PeerRef,
    /// Okunmuş, testin henüz tüketmediği S1 olayları.
    seen: Vec<SessionEvent>,
}

fn side(me: u8, link: HubLink, clock: &Arc<FakeClock>) -> Side {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    let (crypto, _crypto_clock) = fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy);
    let mut cfg = ClientTransportConfig::product(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    cfg.renew_drain_cap = RENEW_DRAIN_CAP;
    let link = Arc::new(Mutex::new(link));
    let known = KnownHandles::new();
    let clock: Arc<dyn HubClock> = clock.clone();
    let envelope = RecordingLink::new(
        HubEnvelopeLink::with_clock(Arc::clone(&link), clock),
        known.clone(),
    );
    let t = ClientTransport::new(cfg, crypto, Box::new(envelope)).expect("düğüm");
    Side {
        t,
        link,
        known,
        me: id(me),
        seen: Vec::new(),
    }
}

/// İki uç + röle.
struct Net {
    sides: [Side; 2],
    clock: Arc<FakeClock>,
    /// Sahte saatin başlangıcı (iki `HubLink` de bundan önce bağlandı).
    start: Instant,
    /// `((gönderen, request_id), h)` — idempotent tahsis.
    allocs: Vec<((usize, u32), u32)>,
    next_h: u32,
    /// Uç başına tele çıkan `STORE{0, INIT}` sayısı (ilk gönderim + T3 tekrarları).
    init_stores: [usize; 2],
    /// `true` = o uca `DELIVER` ulaşmaz (kayıp); yanıtı yok, gönderen T3 ile tekrarlar.
    drop_deliver: [bool; 2],
    /// Bir hub turunda teslimler yanıtlardan ÖNCE uygulanır (varsayılan: yanıtlar önce).
    deliver_first: bool,
}

fn net() -> Net {
    let (la, ta) = {
        let t = bound();
        (t.link, t.t0)
    };
    let (lb, tb) = {
        let t = bound();
        (t.link, t.t0)
    };
    let start = ta.max(tb);
    let clock = Arc::new(FakeClock(Mutex::new(start)));
    let mut a = side(ALICE, la, &clock);
    let mut b = side(BOB, lb, &clock);
    a.t.add_peer_addr(b.me, b.t.data_addr());
    b.t.add_peer_addr(a.me, a.t.data_addr());
    Net {
        sides: [a, b],
        clock,
        start,
        allocs: Vec::new(),
        next_h: 0x100,
        init_stores: [0, 0],
        drop_deliver: [false, false],
        deliver_first: false,
    }
}

impl Net {
    fn now(&self) -> Instant {
        *self.clock.0.lock().unwrap()
    }

    fn me(&self, i: usize) -> PeerRef {
        self.sides[i].me
    }

    fn counters(&self, i: usize) -> HubCounters {
        guard(&self.sides[i].link).counters()
    }

    /// Hub: iki ucun bütün kontrol çıkışı işlenir, yanıtlar ve teslimler uygulanır; sessizleşene
    /// dek (sahte saat ilerlemez). Bir turda önce gönderenlere yanıtlar (`StoreResp`, heartbeat
    /// ACK), sonra teslimler (`DELIVER`) uygulanır; `deliver_first` bunu tersine çevirir.
    fn hub(&mut self) {
        let now = self.now();
        for _ in 0..HUB_ROUNDS {
            let mut replies: Vec<(usize, Vec<u8>)> = Vec::new();
            let mut delivers: Vec<(usize, Vec<u8>)> = Vec::new();
            for i in [A, B] {
                let frames: Vec<Vec<u8>> = {
                    let mut l = guard(&self.sides[i].link);
                    std::iter::from_fn(|| l.poll_transmit(now)).collect()
                };
                for f in frames {
                    self.route(i, &parse(&f), &mut replies, &mut delivers);
                }
            }
            if replies.is_empty() && delivers.is_empty() {
                return;
            }
            let order = if self.deliver_first {
                [delivers, replies]
            } else {
                [replies, delivers]
            };
            for (j, bytes) in order.into_iter().flatten() {
                guard(&self.sides[j].link).on_control(now, &bytes);
            }
        }
        panic!("hub sessizleşmedi");
    }

    fn route(
        &mut self,
        from: usize,
        o: &Out,
        replies: &mut Vec<(usize, Vec<u8>)>,
        delivers: &mut Vec<(usize, Vec<u8>)>,
    ) {
        if o.method == frame::HEARTBEAT {
            replies.push((from, hb_ack(o.rid)));
            return;
        }
        assert_eq!(
            o.method,
            method::STORE_MESSAGE,
            "bağlıyken STORE ya da heartbeat"
        );
        let p = &o.payload;
        let h = u32::from_be_bytes(p[..4].try_into().unwrap());
        let ty = p[4];
        let to = [A, B]
            .into_iter()
            .find(|&j| self.me(j).as_bytes()[..] == p[5..37])
            .expect("alıcı bilinen uç");
        let h = if h == 0 {
            assert_eq!(ty, envelope::SESSION_INIT);
            self.init_stores[from] += 1;
            let key = (from, o.rid);
            if let Some(&(_, h)) = self.allocs.iter().find(|(k, _)| *k == key) {
                h
            } else {
                self.next_h += 1;
                self.allocs.push((key, self.next_h));
                self.next_h
            }
        } else {
            assert_eq!(ty, envelope::SESSION_INIT_RESPONSE);
            h
        };
        replies.push((from, store_resp(o.rid, code::OK, h)));
        if !self.drop_deliver[to] {
            let sender = *self.me(from).as_bytes();
            delivers.push((to, deliver(h, sender, ty, &p[41..])));
        }
    }

    /// Transport'ları sürer (S1 olayları okunur); `Ready` → `peer_ready` (bileşim kökü, KK-H58).
    fn pump(&mut self) {
        for s in &mut self.sides {
            while let Some(ev) = s.t.poll_event() {
                if let SessionEvent::Ready(_, peer) = ev {
                    if let Some(h) = s.known.handle_of(peer) {
                        guard(&s.link).peer_ready(peer, h);
                    }
                }
                s.seen.push(ev);
            }
        }
    }

    /// Sahte saati vadeden vadeye `until`'e (HARİÇ) dek ilerletir; her vadede hub turu (heartbeat
    /// yanıtlanır: iki uç bağlı kalır, "INIT çıkmadı" bağsızlıktan boş geçmez). Dönen değer: bu
    /// aralıkta uç başına tele çıkan `STORE{0, INIT}` sayısı.
    fn advance_until(&mut self, until: Instant) -> [usize; 2] {
        let before = self.init_stores;
        for _ in 0..MAX_STEPS {
            self.hub();
            for s in &self.sides {
                assert!(guard(&s.link).is_bound(), "bağ sürdü");
            }
            let next = self
                .sides
                .iter()
                .filter_map(|s| guard(&s.link).next_deadline())
                .min();
            match next {
                Some(d) if d < until => {
                    let now = self.now();
                    *self.clock.0.lock().unwrap() = d.max(now);
                }
                _ => {
                    return [
                        self.init_stores[A] - before[A],
                        self.init_stores[B] - before[B],
                    ]
                }
            }
        }
        panic!("adım tavanı aşıldı");
    }

    /// `pick`'in seçtiği ilk olayı bekler ve tüketir (hub + transport'lar sürülür).
    fn wait<T>(&mut self, i: usize, what: &str, pick: impl Fn(&SessionEvent) -> Option<T>) -> T {
        let deadline = Instant::now() + WAIT;
        loop {
            self.hub();
            self.pump();
            let seen = &mut self.sides[i].seen;
            if let Some(k) = seen.iter().position(|e| pick(e).is_some()) {
                if let Some(v) = pick(&seen.remove(k)) {
                    return v;
                }
            }
            assert!(Instant::now() < deadline, "{what}");
            std::thread::sleep(TICK);
        }
    }

    fn wait_ready(&mut self, i: usize, peer: PeerRef) -> SessionHandle {
        self.wait(i, "Ready gelmedi", |e| match *e {
            SessionEvent::Ready(h, p) if p == peer => Some(h),
            _ => None,
        })
    }

    fn send(&mut self, i: usize, h: SessionHandle, bytes: &[u8]) {
        let unit = PreparedUnit {
            bytes: bytes.to_vec(),
            compression_policy: CompressionPolicy::Never,
        };
        self.sides[i]
            .t
            .send(h, unit, DeliveryPolicy::Reliable, TrafficClass::Be)
            .expect("gönder");
    }

    fn expect_delivery(&mut self, i: usize, h: SessionHandle, bytes: &[u8]) {
        let deadline = Instant::now() + WAIT;
        loop {
            self.hub();
            if let Some(d) = self.sides[i].t.recv() {
                assert_eq!((d.session, d.unit.as_slice()), (h, bytes), "teslim");
                return;
            }
            self.pump();
            assert!(Instant::now() < deadline, "teslim gelmedi");
            std::thread::sleep(TICK);
        }
    }

    /// İki yönde birer birim (`a` Alice'in, `b` Bob'un oturumu).
    fn both_ways(&mut self, a: SessionHandle, b: SessionHandle, tag: &str) {
        let (to_b, to_a) = (format!("{tag}: a→b"), format!("{tag}: b→a"));
        self.send(A, a, to_b.as_bytes());
        self.expect_delivery(B, b, to_b.as_bytes());
        self.send(B, b, to_a.as_bytes());
        self.expect_delivery(A, a, to_a.as_bytes());
    }

    /// İki uç aynı anda `connect` eder: iki INIT de tele çıkar, hiçbiri teslim edilmeden (gerçek
    /// glare). Bob kaybeder (bekleyeni silinir, Alice'in INIT'ini yanıtlar), Alice kazanır.
    fn glare(&mut self) -> (SessionHandle, SessionHandle) {
        let (a, b) = (self.me(A), self.me(B));
        assert!(a < b, "Alice glare'i kazanır (S3 K10: küçük dest_hash)");
        let Ok(ConnectResult::Pending(_)) = self.sides[A].t.connect(b, None) else {
            panic!("Alice beklemede başlamalı");
        };
        let Ok(ConnectResult::Pending(_)) = self.sides[B].t.connect(a, None) else {
            panic!("Bob beklemede başlamalı");
        };
        let b1 = self.wait_ready(B, a);
        let a1 = self.wait_ready(A, b);
        assert_eq!(self.init_stores, [1, 1], "iki INIT çapraz, birer kez");
        (a1, b1)
    }
}

/// (a) + (d) Glare (S3 K10, identity §16): Bob kaybeder, niyeti Alice'in oturumuyla çözülür. Bob'un
/// terk edilen INIT'inin T3'ü durur: glare çözüldükten sonra `ATTEMPT_MAX_AGE`'e dek iki uçtan da
/// tele INIT çıkmaz; Alice'e Bob'un INIT'i yeniden teslim edilmez. Kazananın (Alice) denemesi
/// eşleşen `Ready` ile biter (iptal değil). Tek oturum iki yönde veri taşır.
///
/// İki sıra: hub önce gönderene yanıt verir (`StoreResp` Bob'a, Alice'in INIT'inden önce: Bob'un
/// son bağladığı hat Alice'in INIT'inin hattıdır) ya da önce teslim eder (Bob'un son bağladığı hat
/// kendi denemesininkidir).
///
/// ESKİ DAVRANIŞ: `on_crypto_ready` (c) niyeti çözüyor ama denemeyi terk etmiyordu. Yanıt-önce
/// sırada Bob'un `Ready`'si Alice'in INIT'inin hattıyla geldiği için `peer_ready` onu durdurmuyordu
/// → Bob'un INIT'i bir saat boyunca T3 kadansıyla (≈ 30 s'de bir) tele çıkıyor ve Alice'e yeniden
/// teslim ediliyordu. Teslim-önce sırada T3'ü yalnız rastlantısal eşleşme durduruyordu.
fn glare_case(deliver_first: bool) {
    let mut n = net();
    n.deliver_first = deliver_first;
    let (a1, b1) = n.glare();
    n.both_ways(a1, b1, "glare");
    let dup = n.counters(A).init_duplicate_pending;
    let start = n.start;
    assert_eq!(
        n.advance_until(start + ATTEMPT_MAX_AGE),
        [0, 0],
        "glare çözüldükten sonra tele INIT yok"
    );
    assert_eq!(
        n.counters(A).init_duplicate_pending,
        dup,
        "kazanana kaybedenin INIT'i yeniden teslim edilmedi"
    );
    let (ca, cb) = (n.counters(A), n.counters(B));
    assert_eq!(
        (cb.attempt_cancelled, cb.attempt_completed),
        (1, 0),
        "kaybedenin denemesi terk edildi"
    );
    assert_eq!(
        (ca.attempt_cancelled, ca.attempt_completed),
        (0, 1),
        "kazananınki eşleşen Ready ile bitti"
    );
    let (a, b) = (n.me(A), n.me(B));
    assert_eq!(n.sides[A].t.renewal_state_for_test(b), (1, 0, 0));
    assert_eq!(n.sides[B].t.renewal_state_for_test(a), (1, 0, 0));
    n.both_ways(a1, b1, "bir saat sonra");
}

#[test]
fn t3g_glare_losers_init_stops_on_the_wire_reply_first() {
    glare_case(false);
}

#[test]
fn t3g_glare_losers_init_stops_on_the_wire_deliver_first() {
    glare_case(true);
}

/// (b) Niyet iptali (S4 C3 "bağlantı niyeti iptal edildi"; S1 `cancel`): `StoreResp OK`'tan sonra
/// sürmekte olan T3 iptalde durur; `ATTEMPT_MAX_AGE`'e dek tele INIT çıkmaz.
///
/// ESKİ DAVRANIŞ: `cancel` Crypto bekleyenini kapatıyordu, taşıyıcı denemesini değil → T3 bir saat.
#[test]
fn t3g_cancelled_intent_stops_its_init_on_the_wire() {
    let mut n = net();
    let b = n.me(B);
    n.drop_deliver[B] = true;
    let Ok(ConnectResult::Pending(ticket)) = n.sides[A].t.connect(b, None) else {
        panic!("beklemede başlamalı");
    };
    n.hub();
    n.pump();
    let start = n.start;
    assert_eq!(
        n.advance_until(start + s(8)),
        [3, 0],
        "StoreResp OK T3'ü durdurmaz (1, 3, 7 s)"
    );
    n.sides[A].t.cancel(ticket);
    n.wait(A, "ConnectAbandoned gelmedi", |e| {
        (*e == SessionEvent::ConnectAbandoned(ticket, AbandonReason::Cancelled)).then_some(())
    });
    assert_eq!(
        n.advance_until(start + ATTEMPT_MAX_AGE),
        [0, 0],
        "iptalden sonra tele INIT yok"
    );
    assert_eq!(n.counters(A).attempt_cancelled, 1);
}

/// (c) Koruma: glare'siz, yanıtsız (DELIVER kaybı) meşru deneme `StoreResp OK`'tan sonra T3'ü
/// sürdürür (KR-18; 1, 3, 7, 15, 31 s); kayıp bitince sıradaki tekrar (61 s) eşe ulaşır, oturum
/// kurulur ve tekrar YALNIZ eşleşen `Ready` ile durur (iptal değil). İki yönde veri.
#[test]
fn t3g_unanswered_intent_keeps_t3_after_store_ok_until_its_ready() {
    let mut n = net();
    let (a, b) = (n.me(A), n.me(B));
    n.drop_deliver[B] = true;
    let Ok(ConnectResult::Pending(_)) = n.sides[A].t.connect(b, None) else {
        panic!("beklemede başlamalı");
    };
    n.hub();
    n.pump();
    let start = n.start;
    assert_eq!(n.advance_until(start + s(32)), [5, 0], "T3 sürer");
    n.drop_deliver[B] = false;
    assert_eq!(n.advance_until(start + s(62)), [1, 0], "61 s tekrarı eşe");
    let b1 = n.wait_ready(B, a);
    let a1 = n.wait_ready(A, b);
    n.both_ways(a1, b1, "glare'siz");
    assert_eq!(
        n.advance_until(start + ATTEMPT_MAX_AGE),
        [0, 0],
        "eşleşen Ready T3'ü durdurdu"
    );
    let c = n.counters(A);
    assert_eq!((c.attempt_completed, c.attempt_cancelled), (1, 0));
}

/// (e) DR-CONNECT-1 / INV-NO-IDLE-TEARDOWN gerilemesi yok: glare'le çözülen niyetin oturumunun hub
/// hattı SONRADAN kaybolur (ROUTE_MISS → `HandleLost`; S4 C5) → kurulu oturum yolu yeni INIT'le
/// yeniler. Bu yeni denemenin T3'ü sürer (önceki terk aynı eşi zehirlemez), eşe ulaşınca tamamlanır
/// ve eşleşen `Ready` ile durur; yeni oturum iki yönde veri taşır.
#[test]
fn t3g_glare_resolved_session_is_renewed_when_its_line_is_lost() {
    let mut n = net();
    let (a, b) = (n.me(A), n.me(B));
    let (a1, b1) = n.glare();
    n.both_ways(a1, b1, "glare");

    let ActiveLink::On(h) = n.sides[B].t.active_link(a) else {
        panic!("Bob'un etkin oturumunun hattı bilinmeli");
    };
    let now = n.now();
    guard(&n.sides[B].link).on_control(now, &route_miss(h.raw()));
    assert_eq!(
        guard(&n.sides[B].link).take_notice(),
        Some(HubNotice::HandleLost { peer: a, h }),
        "çözülen oturumun hattı kurulu hat"
    );
    n.drop_deliver[A] = true;
    assert_eq!(n.sides[B].t.relink_line(a, h), RelinkOutcome::Started);
    n.hub();
    n.pump();
    let start = n.start;
    assert_eq!(
        n.advance_until(start + s(4)),
        [0, 2],
        "yenileme INIT'inin T3'ü sürer (1, 3 s)"
    );
    n.drop_deliver[A] = false;
    assert_eq!(n.advance_until(start + s(8)), [0, 1], "7 s tekrarı eşe");

    let b2 = n.wait_ready(B, a);
    assert_ne!(b2, b1);
    n.send(B, b2, b"yeni hat: b->a");
    let a2 = n.wait_ready(A, b);
    assert_ne!(a2, a1);
    n.expect_delivery(A, a2, b"yeni hat: b->a");
    n.send(A, a2, b"yeni hat: a->b");
    n.expect_delivery(B, b2, b"yeni hat: a->b");
    assert_eq!(
        n.advance_until(start + ATTEMPT_MAX_AGE),
        [0, 0],
        "yenileme eşleşen Ready ile durdu"
    );
    assert_eq!(n.sides[A].t.renewal_state_for_test(b), (1, 0, 0));
    assert_eq!(n.sides[B].t.renewal_state_for_test(a), (1, 0, 0));
    let cb = n.counters(B);
    assert_eq!(
        (cb.attempt_cancelled, cb.attempt_completed),
        (1, 1),
        "glare'deki terk + yenilemenin eşleşen Ready'si"
    );
}
