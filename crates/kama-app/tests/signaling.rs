//! K4G-14a — sinyalleşme (`kama_app::signaling`) uçtan uca: `SIGNALING_BLOB` (0x09) gönder/al.
//!
//! Beklenen davranışın kaynağı: plan K4G-14 NE (1) (`send(peer, bytes) -> Result<SignalingId>`,
//! `Reliable`, A2'nin 0x09 sınırı → `TooLarge`, gelen 0x09 → `SignalingReceived { peer, bytes }`,
//! içerik opak; 0x09 `msg_id` taşımaz → dedup yok), AMP §3 (0x09 alan listesi; K4G-14a yaması
//! 2026-10-12: dedup yokluğu ve oturum değişiminde yeniden sunum), AMP §4 A2 ve §5 kural 6, S1 K7
//! (Transport `h`'de teslim edilmemiş birimi `h'`ye taşımaz) / K8 / §4 ve signaling-seam-v0 §3
//! (at-least-once, duplicate mümkün, sıra garanti edilmez). Yeniden sunum kaydının TUTMA KURALI
//! (ömür + eş başına tavan) ÖNERİDİR — sahip kararı bekliyor (DUR-K4G-14a); testler uygulanan öneriyi
//! sabitler.
//!
//! Düzenek `tests/ptt.rs`'teki iki uçlu çiftin (`Link`) kırpılmış KOPYASIDIR: bir uçtan gönderilen
//! birim karşı ucun teslim kuyruğuna düşer; oturum kurma, supersession, oturum sonu, yoldaki birimi
//! kaybettirme ya da geç verme, olayları düşürme, tek atımlık gönderim reddi ve ham kayıt enjeksiyonu
//! testin elindedir. Her uç, Transport'un KABUL ettiği gönderimlerin tutamacını, çözülmüş kaydını,
//! `DeliveryPolicy`'sini, `CompressionPolicy`'sini ve bayt uzunluğunu kaydeder (motordan bağımsız
//! kanıt: "oturum başına bir yeniden sunum", politika, boyut). Her uçta ürün zinciri kuruludur:
//! `Ptt::poll` (Messenger'ın tek okuyucusu) → `FileTransfer::handle` → `Signaling::handle`; turun
//! sonunda `FileTransfer::pump` + `Signaling::pump`. Zaman, iki ucun paylaştığı elle ilerletilen bir
//! saattir (gerçek uyku yok). Eşler arası kimlik testi ortak yansıtıcı sahteyle
//! (`tests/common/mock_transport.rs`; değiştirilmedi).
//!
//! # Testlerin `ea0d3909`'daki (K4G-14a öncesi main) durumu — üç sınıf
//!
//! `kama_app::signaling` o commit'te YOKTU (gönderim API'si yoktu; Messenger gelen 0x09'u
//! `MessengerEvent::Other` olarak aynen geçiriyordu): dosya eski kodda DERLENMEZ. Her testin belgesi,
//! test MANTIĞININ eski koda göre sınıfını yazar (K4G-08/09'un sınıfları):
//!
//! * **KIRMIZI (uyarlansa da)** — ham `codec::encode` + `transport_mut().send` ve `Other` okumasıyla
//!   uyarlanınca da bir doğrulama düşer (ör. oturum değişiminde yeniden sunum yoktu);
//! * **UYARLANAMAZ** — sınadığı yüzey eski kodda yok (`SignalingId`, `SignalingError`,
//!   `max_blob_bytes`, yeniden sunum kaydı);
//! * **AYNI DAVRANIŞ** — uyarlanınca eski kodda da geçerdi; regresyon kilidi.

mod common;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::mock_transport::MockTransport;
use kama_app::clock::MonotonicClock;
use kama_app::codec::limits::{APP_RECORD_HEADER_BYTES, SIGNALING_BLOB_PREFIX_BYTES};
use kama_app::codec::{decode, encode, AmpLimits, AppMessage, MsgType, ReceiptKind};
use kama_app::file_transfer::FileTransfer;
use kama_app::messaging::{ConnectState, Messenger, MessengerEvent};
use kama_app::ptt::{Ptt, PttEvent};
use kama_app::signaling::{
    max_blob_bytes, Signaling, SignalingError, SignalingEvent, SignalingId, SignalingStats,
    FIRST_SIGNALING_ID, SIGNALING_RESEND_PER_PEER, SIGNALING_RESEND_TTL,
};
use kama_app::store::{AppStore, MemoryAppStore};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport,
    PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};

/// Test parametresi (normatif değil): codec vektörlerinin ve öbür motor testlerinin değeri.
const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Çiftin birim tavanı (`max_plaintext_unit_bytes`, `usize`).
const LINK_MAX_UNIT_BYTES: usize = TEST_MAX_PLAINTEXT_UNIT_BYTES as usize;
/// Çiftin yön başına teslim kuyruğu tavanı (sahte sınır; aşımı `Backpressure`).
const LINK_QUEUE_CAP: usize = 64;
/// Uç başına olay kuyruğu tavanı (sahte sınır).
const LINK_EVENT_CAP: usize = 16;
/// Uç başına gönderim kaydı tavanı (sahte sınır; eş başına tavan testi 257 + 256 gönderim yapar).
const LINK_LOG_CAP: usize = 1024;
/// Bir `drain`'in olay tavanı.
const DRAIN_CAP: usize = 1024;
/// `Duo::settle` tur tavanı.
const ROUND_CAP: usize = 64;
/// "Yeniden yoklansa da" tur sayısı.
const IDLE_ROUNDS: usize = 8;
/// Saatin en küçük adımı (ömür sınırı denetimi).
const TICK: Duration = Duration::from_nanos(1);
/// Saatin başlangıcı (sıfır DEĞİL): ömrün başı (ilk kabul anı) sıfır damgasından ayırt edilsin.
const CLOCK_START: Duration = Duration::from_secs(1000);

const ALICE: PeerRef = PeerRef::from_dest_hash([0xA1; 32]);
const BOB: PeerRef = PeerRef::from_dest_hash([0xB0; 32]);
const CAROL: PeerRef = PeerRef::from_dest_hash([0xC0; 32]);
const DAVE: PeerRef = PeerRef::from_dest_hash([0xD0; 32]);

fn caps() -> TransportCapabilities {
    TransportCapabilities {
        max_plaintext_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
        preferred_plaintext_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
        max_early_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
        reliable: true,
    }
}

fn limits() -> AmpLimits {
    AmpLimits::from_capabilities(&caps()).expect("test limitleri geçerli")
}

fn record(message: &AppMessage) -> Vec<u8> {
    encode(message, &limits()).expect("kodlanabilir").bytes
}

/// `n` numaralı test blob'u (opak; `u16_be` sıra numarası).
fn blob(n: usize) -> Vec<u8> {
    u16::try_from(n)
        .expect("test sayısı u16'ya sığar")
        .to_be_bytes()
        .to_vec()
}

fn received(peer: PeerRef, bytes: &[u8]) -> SignalingEvent {
    SignalingEvent::SignalingReceived {
        peer,
        bytes: bytes.to_vec(),
    }
}

fn session(event: SessionEvent) -> SignalingEvent {
    SignalingEvent::Messenger(MessengerEvent::Session(event))
}

/// Taze bellek içi depo — kalıcı durumu olmayan bir cihaz (sinyalleşme depoya bağlanmaz).
fn fresh_store() -> Arc<dyn AppStore> {
    Arc::new(MemoryAppStore::new())
}

// ── saat ─────────────────────────────────────────────────────────────────────────────────────

/// Elle ilerletilen saat: kopyalar AYNI zamanı paylaşır (test bir kopyayı tutar, motorlar
/// diğerlerini); zaman yalnız ileri gider (u64 nanosaniyede doyar).
#[derive(Clone, Default)]
struct ManualClock {
    nanos: Arc<AtomicU64>,
}

impl ManualClock {
    /// [`CLOCK_START`]'tan başlayan saat.
    fn started() -> Self {
        let clock = Self::default();
        clock.advance(CLOCK_START);
        clock
    }

    fn advance(&self, by: Duration) {
        let step = u64::try_from(by.as_nanos()).unwrap_or(u64::MAX);
        let advanced = self
            .nanos
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| {
                Some(now.saturating_add(step))
            });
        assert!(advanced.is_ok(), "kapanış her zaman Some döner");
    }
}

impl MonotonicClock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
    }
}

// ── iki uçlu çift (`tests/ptt.rs`'ten kırpılmış kopya + gönderim kaydı) ────────────────────────

#[derive(Debug, Clone, Copy)]
enum Side {
    Alice,
    Bob,
}

impl Side {
    /// Bu ucun gördüğü eş.
    const fn remote(self) -> PeerRef {
        match self {
            Self::Alice => BOB,
            Self::Bob => ALICE,
        }
    }
}

/// Transport'un kabul ettiği bir gönderim.
struct Accepted {
    session: SessionHandle,
    message: AppMessage,
    policy: DeliveryPolicy,
    compression: CompressionPolicy,
    len: usize,
}

/// Bir ucun Transport durumu.
#[derive(Default)]
struct Endpoint {
    /// Eşten gelmiş, motorun henüz okumadığı birimler (≤ `LINK_QUEUE_CAP`).
    inbox: VecDeque<Delivered>,
    /// Bu uca sıralanmış oturum olayları (≤ `LINK_EVENT_CAP`).
    events: VecDeque<SessionEvent>,
    /// Gönderim kabul eden tutamaç.
    active: Option<SessionHandle>,
    /// Supersede edilmiş, `Closed`'ı bu uçta henüz okunmamış tutamaçlar: `send` →
    /// `SessionSuperseded` (S1 F0.2(i)); `Closed` okununca `UnknownSession`.
    superseded: Vec<SessionHandle>,
    /// Bu uçtan KABUL edilen gönderimler, sırasıyla (≤ `LINK_LOG_CAP`).
    sent: Vec<Accepted>,
    /// Tek atımlık düğme: bu uçtan, oturum ve boy denetimlerini geçen sıradaki bu kadar gönderim
    /// `Backpressure` ile reddedilir — kuyruk boş olsa da. Reddedilen gönderim kaydedilmez, karşıya
    /// gitmez.
    refuse_next: usize,
}

impl Endpoint {
    fn push_event(&mut self, event: SessionEvent) {
        assert!(
            self.events.len() < LINK_EVENT_CAP,
            "senaryo olay tavanını aştı"
        );
        self.events.push_back(event);
    }
}

/// İki ucun paylaştığı durum.
#[derive(Default)]
struct Link {
    alice: Endpoint,
    bob: Endpoint,
    /// Basılan son tutamacın ham değeri.
    last_handle: u64,
}

impl Link {
    fn endpoint(&mut self, side: Side) -> &mut Endpoint {
        match side {
            Side::Alice => &mut self.alice,
            Side::Bob => &mut self.bob,
        }
    }

    fn fresh_handle(&mut self) -> SessionHandle {
        self.last_handle += 1;
        SessionHandle::from_raw(self.last_handle)
    }

    /// İki uçta aynı tutamaçla oturum: her uca `Ready(h, eş)`.
    fn establish(&mut self) -> SessionHandle {
        let handle = self.fresh_handle();
        for side in [Side::Alice, Side::Bob] {
            let endpoint = self.endpoint(side);
            assert!(endpoint.active.is_none(), "establish: aktif oturum var");
            endpoint.active = Some(handle);
            endpoint.push_event(SessionEvent::Ready(handle, side.remote()));
        }
        handle
    }

    /// Supersession onayı (S1 K7): her uca `Ready(h')` sonra `Closed(h, Superseded)`. Eski
    /// tutamaçla yoldaki birimler karşı kuyrukta KALIR (yalnız-alım drain'in karşılığı).
    fn supersede(&mut self) -> SessionHandle {
        let handle = self.fresh_handle();
        for side in [Side::Alice, Side::Bob] {
            let endpoint = self.endpoint(side);
            let old = endpoint
                .active
                .replace(handle)
                .expect("supersede: aktif oturum yok");
            endpoint.superseded.push(old);
            endpoint.push_event(SessionEvent::Ready(handle, side.remote()));
            endpoint.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
        }
        handle
    }

    /// `side` ucunun oturumu `reason` ile biter: yalnız o uca `Closed(h, reason)`.
    fn end(&mut self, side: Side, reason: CloseReason) {
        let endpoint = self.endpoint(side);
        let handle = endpoint.active.take().expect("end: aktif oturum yok");
        endpoint.push_event(SessionEvent::Closed(handle, reason));
    }

    /// `to` ucuna, aktif oturumu üzerinden eşten gelmiş gibi ham kayıt koyar.
    fn inject(&mut self, to: Side, unit: Vec<u8>) {
        let endpoint = self.endpoint(to);
        let session = endpoint.active.expect("inject: aktif oturum yok");
        assert!(endpoint.inbox.len() < LINK_QUEUE_CAP, "inject: kuyruk dolu");
        endpoint.inbox.push_back(Delivered { session, unit });
    }

    /// `to` ucunun teslim kuyruğundaki birimleri yoldan alır — motora verilmemiştir: test onları
    /// kaybettirebilir ya da sonra [`Link::deliver_late`] ile verebilir.
    fn take_inbox(&mut self, to: Side) -> Vec<Delivered> {
        self.endpoint(to).inbox.drain(..).collect()
    }

    /// Yolda tutulan birimleri `to` ucunun teslim kuyruğunun SONUNA koyar: eski oturumun yalnız-alım
    /// drain'inden geç gelen birimlerin karşılığı (S1 K7).
    fn deliver_late(&mut self, to: Side, units: Vec<Delivered>) {
        let endpoint = self.endpoint(to);
        assert!(
            endpoint.inbox.len() + units.len() <= LINK_QUEUE_CAP,
            "deliver_late: kuyruk tavanı"
        );
        endpoint.inbox.extend(units);
    }

    /// `side` ucuna sıralanmış, henüz okunmamış oturum olaylarını atar (olay kuyruğu taşınca düşen
    /// olayların karşılığı). Supersede edilmiş tutamaçların kaydı da silinir: onların `Closed`'ı artık
    /// hiç okunmayacaktır.
    fn drop_events(&mut self, side: Side) {
        let endpoint = self.endpoint(side);
        endpoint.events.clear();
        endpoint.superseded.clear();
    }
}

/// Bir uç: `KamaTransport`'u paylaşılan `Link` üzerinde uygular.
struct End {
    link: Rc<RefCell<Link>>,
    side: Side,
}

impl KamaTransport for End {
    fn capabilities(&self) -> TransportCapabilities {
        caps()
    }

    fn connect(
        &mut self,
        _peer: PeerRef,
        _early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError> {
        // Oturumu test kurar (`Link::establish`); `connect` yalnız durumu bildirir (idempotent).
        let active = self.link.borrow_mut().endpoint(self.side).active;
        Ok(match active {
            Some(handle) => ConnectResult::Ready(handle),
            None => ConnectResult::Pending(ConnectTicket::from_raw(0)),
        })
    }

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        let mut guard = self.link.borrow_mut();
        let link = &mut *guard;
        let (me, remote) = match self.side {
            Side::Alice => (&mut link.alice, &mut link.bob),
            Side::Bob => (&mut link.bob, &mut link.alice),
        };
        if me.superseded.contains(&session) {
            return Err(TransportError::SessionSuperseded);
        }
        if me.active != Some(session) {
            return Err(TransportError::UnknownSession);
        }
        if unit.bytes.len() > LINK_MAX_UNIT_BYTES {
            return Err(TransportError::UnitTooLarge);
        }
        if me.refuse_next > 0 {
            me.refuse_next -= 1;
            return Err(TransportError::Backpressure);
        }
        if remote.inbox.len() >= LINK_QUEUE_CAP {
            return Err(TransportError::Backpressure);
        }
        assert!(
            me.sent.len() < LINK_LOG_CAP,
            "senaryo gönderim kaydı tavanını aştı"
        );
        let message = decode(&unit.bytes, &limits()).expect("motor geçerli kayıt üretir");
        me.sent.push(Accepted {
            session,
            message,
            policy,
            compression: unit.compression_policy,
            len: unit.bytes.len(),
        });
        remote.inbox.push_back(Delivered {
            session,
            unit: unit.bytes,
        });
        Ok(())
    }

    fn recv(&mut self) -> Option<Delivered> {
        self.link.borrow_mut().endpoint(self.side).inbox.pop_front()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        let mut link = self.link.borrow_mut();
        let endpoint = link.endpoint(self.side);
        let event = endpoint.events.pop_front()?;
        if let SessionEvent::Closed(handle, _) = event {
            endpoint.superseded.retain(|old| *old != handle);
        }
        Some(event)
    }

    fn close(&mut self, session: SessionHandle) -> Result<(), TransportError> {
        let mut link = self.link.borrow_mut();
        let endpoint = link.endpoint(self.side);
        if endpoint.superseded.contains(&session) {
            return Err(TransportError::SessionSuperseded);
        }
        if endpoint.active != Some(session) {
            return Err(TransportError::UnknownSession);
        }
        endpoint.active = None;
        endpoint.push_event(SessionEvent::Closed(session, CloseReason::UserClosed));
        Ok(())
    }

    fn cancel(&mut self, _ticket: ConnectTicket) {}
}

/// Bir uç: Messenger + ürün zinciri (Ptt → FileTransfer → Signaling), hepsi aynı saatle. Olaylar
/// yalnız PTT motorundan okunur (Messenger'ın tek okuyucusu); bu düzenekte PTT ve dosya trafiği
/// yoktur.
struct Node {
    messenger: Messenger<End>,
    ptt: Ptt,
    files: FileTransfer,
    signaling: Signaling,
}

impl Node {
    fn new(link: &Rc<RefCell<Link>>, side: Side, clock: &ManualClock) -> Self {
        let end = End {
            link: Rc::clone(link),
            side,
        };
        Self {
            messenger: Messenger::with_store(end, fresh_store()).expect("limitler geçerli"),
            ptt: Ptt::new(Arc::new(clock.clone())),
            files: FileTransfer::new(Arc::new(clock.clone())),
            signaling: Signaling::new(Arc::new(clock.clone())),
        }
    }

    fn send(&mut self, peer: PeerRef, bytes: &[u8]) -> Result<SignalingId, SignalingError> {
        self.signaling.send(&mut self.messenger, peer, bytes)
    }

    /// Yalnız sinyalleşme motorunun turu (yeniden sunum denetimi + budama).
    fn pump(&mut self) {
        self.signaling.pump(&mut self.messenger);
    }

    /// Zincirin bir adımı: `Ptt::poll` → `FileTransfer::handle` → `Signaling::handle`. Dosya motorunun
    /// tükettiği kayıtlar atlanır. `pump` YOK.
    fn next(&mut self) -> Option<SignalingEvent> {
        loop {
            let event = self.ptt.poll(&mut self.messenger)?;
            let PttEvent::Messenger(event) = event else {
                panic!("bu düzenekte PTT trafiği yok: {event:?}");
            };
            let Some(event) = self.files.handle(&mut self.messenger, event) else {
                continue;
            };
            return Some(self.signaling.handle(&mut self.messenger, event));
        }
    }

    /// Zincirde okunacak bir şey kalmayana dek olayları toplar. `pump` YOK.
    fn drain(&mut self) -> Vec<SignalingEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.next() {
            assert!(events.len() < DRAIN_CAP, "olay akışı bitmiyor");
            events.push(event);
        }
        events
    }

    /// Bir uygulama turu: zincir boşaltılır, sonra `FileTransfer::pump` + `Signaling::pump`.
    fn turn(&mut self) -> Vec<SignalingEvent> {
        let events = self.drain();
        self.files.pump(&mut self.messenger);
        self.pump();
        assert!(self.files.poll_event().is_none(), "dosya olayı yok");
        events
    }
}

/// İki uçlu düzenek: Alice'in ve Bob'un uçları, ortak saat + testin `Link` denetimi.
struct Duo {
    link: Rc<RefCell<Link>>,
    clock: ManualClock,
    alice: Node,
    bob: Node,
}

impl Duo {
    fn unconnected() -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let clock = ManualClock::started();
        let alice = Node::new(&link, Side::Alice, &clock);
        let bob = Node::new(&link, Side::Bob, &clock);
        Self {
            link,
            clock,
            alice,
            bob,
        }
    }

    /// Oturum kurulmuş, iki tarafın `Ready` olayı okunmuş düzenek.
    fn connected() -> (Self, SessionHandle) {
        let mut duo = Self::unconnected();
        let handle = duo.link.borrow_mut().establish();
        assert_eq!(
            duo.alice.turn(),
            vec![session(SessionEvent::Ready(handle, BOB))]
        );
        assert_eq!(
            duo.bob.turn(),
            vec![session(SessionEvent::Ready(handle, ALICE))]
        );
        (duo, handle)
    }

    fn supersede(&self) -> SessionHandle {
        self.link.borrow_mut().supersede()
    }

    fn take_inbox(&self, to: Side) -> Vec<Delivered> {
        self.link.borrow_mut().take_inbox(to)
    }

    fn deliver_late(&self, to: Side, units: Vec<Delivered>) {
        self.link.borrow_mut().deliver_late(to, units);
    }

    fn inject(&self, to: Side, unit: Vec<u8>) {
        self.link.borrow_mut().inject(to, unit);
    }

    fn drop_events(&self, side: Side) {
        self.link.borrow_mut().drop_events(side);
    }

    /// `side` ucundan sıradaki `count` gönderim `Backpressure` ile reddedilir (tek atımlık).
    fn refuse_next_sends(&self, side: Side, count: usize) {
        self.link.borrow_mut().endpoint(side).refuse_next = count;
    }

    /// `side` ucundan Transport'un kabul ettiği gönderimlerin tipi ve iki politikası, sırasıyla.
    fn sent(&self, side: Side) -> Vec<(MsgType, DeliveryPolicy, CompressionPolicy)> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .map(|accepted| {
                (
                    accepted.message.msg_type(),
                    accepted.policy,
                    accepted.compression,
                )
            })
            .collect()
    }

    /// `side` ucundan Transport'un kabul ettiği gönderimlerin bayt uzunlukları, sırasıyla.
    fn sent_lens(&self, side: Side) -> Vec<usize> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .map(|accepted| accepted.len)
            .collect()
    }

    /// `side` ucundan `handle` tutamacına kabul edilen `SIGNALING_BLOB`'ların blob'ları, sırasıyla.
    fn blobs_on(&self, side: Side, handle: SessionHandle) -> Vec<Vec<u8>> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .filter(|accepted| accepted.session == handle)
            .filter_map(|accepted| match &accepted.message {
                AppMessage::SignalingBlob { blob } => Some(blob.clone()),
                _ => None,
            })
            .collect()
    }

    /// Alice ile Bob'u ilerleme kalmayana dek sırayla döndürür; Bob'un olaylarını toplar (Bob'un
    /// teslim kuyruğu tavanı Alice'in yeniden sunumlarını geri basınçla böler).
    fn settle(&mut self) -> Vec<SignalingEvent> {
        let mut bob_events = Vec::new();
        for _ in 0..ROUND_CAP {
            let alice_events = self.alice.turn();
            let fresh = self.bob.turn();
            if alice_events.is_empty() && fresh.is_empty() {
                return bob_events;
            }
            bob_events.extend(fresh);
        }
        panic!("düzenek {ROUND_CAP} turda durulmadı");
    }
}

// ── gidiş-dönüş, politika, boyut, oturumsuz gönderim ───────────────────────────────────────────

/// Gidiş-dönüş: blob'lar OPAK (boş, UTF-8 olmayan, 256 bayt değerinin hepsi) ve gönderim sırasıyla
/// aynen teslim edilir; her `SIGNALING_BLOB` `Reliable` ve sıkıştırmasız gider; alıcı hiçbir şey geri
/// göndermez (alındı yok); kimlikler `FIRST_SIGNALING_ID`'den kesin artar; iki yön de çalışır;
/// kabul edilen blob'lar yeniden sunum kaydında tutulur.
///
/// `ea0d3909`'da UYARLANAMAZ: `Signaling` motoru, `SignalingEvent` ve `SignalingId` yoktu. Yalnız
/// teslim kısmı, ham `codec::encode` + `transport_mut().send` ile uyarlanınca AYNI DAVRANIŞ olurdu
/// (Messenger 0x09'u `Other` olarak aynen geçiriyordu).
#[test]
fn round_trip_delivers_opaque_blobs_in_order_and_reliably() {
    let (mut duo, _) = Duo::connected();
    let all_values: Vec<u8> = (0..=u8::MAX).collect();
    let blobs = [Vec::new(), vec![0xFF, 0x00, 0xFE], all_values];
    let ids: Vec<SignalingId> = blobs
        .iter()
        .map(|bytes| duo.alice.send(BOB, bytes).expect("gönderilir"))
        .collect();
    assert_eq!(ids[0], FIRST_SIGNALING_ID);
    assert!(
        ids.windows(2)
            .all(|pair| pair[1].get() == pair[0].get() + 1),
        "kimlikler kesin artar: {ids:?}"
    );
    let expected: Vec<SignalingEvent> = blobs.iter().map(|bytes| received(ALICE, bytes)).collect();
    assert_eq!(duo.bob.turn(), expected);
    let reliable = (
        MsgType::SignalingBlob,
        DeliveryPolicy::Reliable,
        CompressionPolicy::Never,
    );
    assert_eq!(duo.sent(Side::Alice), vec![reliable; blobs.len()]);
    // Alındı yok: Bob hiçbir şey göndermedi, Alice hiçbir olay görmez.
    assert!(duo.sent(Side::Bob).is_empty());
    assert!(duo.alice.turn().is_empty());
    assert_eq!(duo.alice.signaling.pending_count(BOB), blobs.len());
    // Ters yön.
    let back = duo.bob.send(ALICE, b"cevap").expect("gönderilir");
    assert_eq!(back, FIRST_SIGNALING_ID, "kimlik motor-tekildir, uç başına");
    assert_eq!(duo.alice.turn(), vec![received(BOB, b"cevap")]);
    let blob_count = u64::try_from(blobs.len()).expect("küçük sayı");
    assert_eq!(
        duo.alice.signaling.stats(),
        SignalingStats {
            blobs_sent: blob_count,
            blobs_received: 1,
            ..SignalingStats::default()
        }
    );
    assert_eq!(
        duo.bob.signaling.stats(),
        SignalingStats {
            blobs_sent: 1,
            blobs_received: blob_count,
            ..SignalingStats::default()
        }
    );
}

/// A2'nin 0x09 sınırı: `max_blob_bytes` = birim − kayıt başlığı − `blob_len` öneki (bağımsız
/// türetim). Bir bayt fazlası tipli `TooLarge`; Transport'a hiçbir şey verilmez, kayda bir şey girmez,
/// kimlik tüketilmez; boyut oturum denetiminden ÖNCE denetlenir. Tam sınır: kayıt birime tam sığar ve
/// karşıya ulaşır. Denetim sırası: eşin yeniden sunulacak bir kaydı VARKEN de `TooLarge` hiçbir şey
/// göndermez (yeniden sunum denetiminden önce döner); kayıt sonraki `pump`'ta sunulur.
///
/// `ea0d3909`'da UYARLANAMAZ: `SignalingError::TooLarge` ve `max_blob_bytes` yoktu (codec'in
/// `EncodeError::BodyTooLarge`'ı vardı — o kısım AYNI DAVRANIŞ).
#[test]
fn oversize_blob_is_a_typed_error_and_reaches_no_transport() {
    let (mut duo, _) = Duo::connected();
    let max = max_blob_bytes(duo.alice.messenger.limits());
    assert_eq!(
        max,
        LINK_MAX_UNIT_BYTES - APP_RECORD_HEADER_BYTES - SIGNALING_BLOB_PREFIX_BYTES
    );
    let over = vec![0x5A; max + 1];
    let too_large = Err(SignalingError::TooLarge {
        blob_len: max + 1,
        max_blob_len: max,
    });
    assert_eq!(duo.alice.send(BOB, &over), too_large);
    assert_eq!(
        duo.alice.send(CAROL, &over),
        too_large,
        "boyut oturumdan önce"
    );
    assert!(duo.sent(Side::Alice).is_empty(), "Transport'a hiçbir şey");
    assert_eq!(duo.alice.signaling.pending_count(BOB), 0);
    assert_eq!(duo.alice.signaling.stats(), SignalingStats::default());
    let exact = vec![0xA5; max];
    let id = duo.alice.send(BOB, &exact).expect("tam sığar");
    assert_eq!(id, FIRST_SIGNALING_ID, "ret kimlik tüketmedi");
    assert_eq!(
        duo.sent_lens(Side::Alice),
        vec![LINK_MAX_UNIT_BYTES],
        "kayıt birime tam sığdı"
    );
    assert_eq!(duo.bob.turn(), vec![received(ALICE, &exact)]);
    // Denetim sırası: yeni oturum senkron benimsenir, `exact` yeniden sunulmayı bekler.
    let new = duo.supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    assert_eq!(duo.alice.send(BOB, &over), too_large);
    assert!(
        duo.blobs_on(Side::Alice, new).is_empty(),
        "boyut reddi bekleyen yeniden sunumu tetiklemedi"
    );
    duo.alice.pump();
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![exact]);
}

/// Hazır oturumu olmayan eşe gönderim tipli `NoSession`: (i) eş hiç görülmedi; (ii) eş GÖRÜLDÜ —
/// `Messenger::connect` bekleyen bir niyet açtı (`ConnectState::Connecting`) — ama oturumu henüz
/// kurulmadı; (iii) `Closed` okundu. Hiçbirinde Transport'a bir şey verilmez ve kimlik tüketilmez.
///
/// `ea0d3909`'da UYARLANAMAZ: `SignalingError` yoktu.
#[test]
fn send_without_a_ready_session_is_a_typed_error() {
    let mut duo = Duo::unconnected();
    assert_eq!(
        duo.alice.send(CAROL, b"x"),
        Err(SignalingError::NoSession),
        "(i) hiç görülmemiş eş"
    );
    assert!(matches!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Pending(_))
    ));
    assert!(
        matches!(
            duo.alice.messenger.connect_state(BOB),
            ConnectState::Connecting { .. }
        ),
        "eş görüldü: bekleyen niyet"
    );
    assert_eq!(
        duo.alice.send(BOB, b"x"),
        Err(SignalingError::NoSession),
        "(ii) görülmüş, oturumu kurulmamış eş"
    );
    let handle = duo.link.borrow_mut().establish();
    assert_eq!(
        duo.alice.turn(),
        vec![session(SessionEvent::Ready(handle, BOB))]
    );
    let id = duo.alice.send(BOB, b"ilk").expect("gönderilir");
    assert_eq!(id, FIRST_SIGNALING_ID, "hata yolları kimlik tüketmedi");
    duo.link
        .borrow_mut()
        .end(Side::Alice, CloseReason::PeerClosed);
    assert_eq!(
        duo.alice.turn(),
        vec![session(SessionEvent::Closed(
            handle,
            CloseReason::PeerClosed
        ))]
    );
    assert_eq!(
        duo.alice.send(BOB, b"x"),
        Err(SignalingError::NoSession),
        "(iii) Closed okundu"
    );
    assert_eq!(duo.sent(Side::Alice).len(), 1, "yalnız ilk blob");
    assert_eq!(duo.alice.signaling.stats().blobs_sent, 1);
}

/// Transport yeni blob'u reddederse (`Backpressure`) ret aynen döner (S1 §3), blob kayda girmez,
/// kimlik tüketilmez ve motor onu yeniden denemez (S1 K8: çağıran bekler, aynı blob'u yeniden verir).
///
/// `ea0d3909`'da UYARLANAMAZ: `SignalingError::Transport` ve `SignalingId` yoktu.
#[test]
fn a_transport_refusal_is_returned_and_consumes_nothing() {
    let (mut duo, _) = Duo::connected();
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(
        duo.alice.send(BOB, b"b1"),
        Err(SignalingError::Transport(TransportError::Backpressure))
    );
    assert_eq!(duo.alice.signaling.pending_count(BOB), 0, "kayda girmedi");
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.turn().is_empty());
    }
    assert!(duo.sent(Side::Alice).is_empty(), "motor yeniden denemedi");
    let id = duo.alice.send(BOB, b"b1").expect("çağıran yeniden verir");
    assert_eq!(id, FIRST_SIGNALING_ID, "ret kimlik tüketmedi");
    assert_eq!(duo.bob.turn(), vec![received(ALICE, b"b1")]);
}

// ── oturum değişimi: yeniden sunum (S1 K7; tutma kuralı ÖNERİ — DUR-K4G-14a) ─────────────────────

/// Olay yolu: blob eski oturumla yolda kaybolur (S1 K7: Transport onu yeni oturuma taşımaz). Kayıt
/// yeni oturuma AYNI baytlarla, `Reliable`, TAM BİR KEZ yeniden sunulur — `Ready(h')` zincirden
/// geçirilirken, olay uygulamaya verilmeden ÖNCE; sonraki yoklamalar ve turlar onu yinelemez. Bob'da
/// blob yeni oturumdan teslim edilir.
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): yeniden sunum yoktu — eski oturumla kaybolan blob Bob'a hiç
/// ulaşmaz (`blobs_on(h')` boş, Bob'da `SignalingReceived` yok).
#[test]
fn blob_lost_in_supersession_is_resent_once_before_ready_is_passed() {
    let (mut duo, old) = Duo::connected();
    duo.alice.send(BOB, b"offer").expect("gönderilir");
    assert_eq!(
        duo.take_inbox(Side::Bob).len(),
        1,
        "blob eski oturumla yolda kayboldu"
    );
    let new = duo.supersede();
    assert_eq!(
        duo.alice.next(),
        Some(session(SessionEvent::Ready(new, BOB)))
    );
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"offer".to_vec()],
        "Ready(h') geçirilirken"
    );
    assert_eq!(
        duo.alice.drain(),
        vec![session(SessionEvent::Closed(old, CloseReason::Superseded))]
    );
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.turn().is_empty());
    }
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"offer".to_vec()],
        "tam bir kez"
    );
    assert!(duo
        .sent(Side::Alice)
        .iter()
        .all(
            |&(_, policy, compression)| policy == DeliveryPolicy::Reliable
                && compression == CompressionPolicy::Never
        ));
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.resent_on_new_session, stats.resend_refused), (1, 0));
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, b"offer"),
        ]
    );
    assert_eq!(
        duo.alice.signaling.pending_count(BOB),
        1,
        "kayıt ömrü boyunca tutulur"
    );
}

/// Senkron yol: yeni oturum `Ready` olayı okunmadan benimsenebilir — idempotent `Messenger::connect`
/// mevcut tutamacı `ConnectResult::Ready(h')` ile olaysız döndürür (S1 K2). Yeniden sunum yine TAM BİR
/// KEZ gider: (a) `send` önce — eski blob yeni blob'dan ÖNCE; (b) `pump` önce; (c) `Ready(h')` hiç
/// okunmaz (olay kuyruğunda düştü) — yalnız `pump` verir. Sonra okunan `Ready(h')` yinelemez.
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): yeniden sunum yoktu (`blobs_on(h')`'de eski blob yok).
#[test]
fn resend_follows_a_session_adopted_without_an_event() {
    // (a) `send` önce. Eski blob eski oturumda TESLİM EDİLMİŞTİ: yine de yeniden sunulur ve Bob onu
    // ikinci kez alır (dedup yok — beyanlı).
    let (mut duo, old) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    assert_eq!(duo.bob.turn(), vec![received(ALICE, b"b1")]);
    let new = duo.supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.alice.send(BOB, b"b2").expect("gönderilir");
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"b1".to_vec(), b"b2".to_vec()],
        "yeniden sunum yeni blob'dan önce"
    );
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"b1".to_vec(), b"b2".to_vec()],
        "sonra okunan Ready(h') yinelemedi"
    );
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, b"b1"),
            received(ALICE, b"b2"),
        ]
    );

    // (b) `pump` önce.
    let (mut duo, old) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let new = duo.supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.alice.pump();
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"b1".to_vec()]);
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"b1".to_vec()]);

    // (c) `Ready(h')` hiç okunmaz: yalnız `pump`'ın denetimi yeniden sunar.
    let (mut duo, old) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let new = duo.supersede();
    duo.drop_events(Side::Alice);
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    assert!(duo.alice.drain().is_empty());
    assert!(
        duo.blobs_on(Side::Alice, new).is_empty(),
        "olay okunmadı, pump çağrılmadı"
    );
    duo.alice.pump();
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"b1".to_vec()]);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.turn().is_empty());
    }
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"b1".to_vec()],
        "tam bir kez"
    );
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, b"b1"),
        ]
    );
}

/// Ret yolu: Transport yeniden sunumu reddederse (`Backpressure`) tutamaç işaretlenmez, kayıt sonraki
/// denetimde yeniden sunulur; kabul edildikten sonra o oturuma bir daha verilmez — toplam TAM BİR
/// kabul. Ret tek atımlık düğmeyle verilir (kuyruk durumundan bağımsız). (a) Olay yolu: `Ready(h')`
/// geçirilirken ret, sonraki `pump`'ta kabul. (b) Senkron yol: `pump` reddedilir, sonraki `pump` kabul
/// eder. (c) `send` yolu: eski blob'un yeniden sunumu reddedilir, yeni blob YİNE gider (ret onu
/// durdurmaz; sıra garanti değildir — beyanlı), sonraki `pump` eskiyi sunar. (d) Eşin iki kaydında
/// ilkinin reddi ikincisini de bekletir: yeniden sunumlar kendi aralarında sırayı korur.
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): yeniden sunum yoktu; h''ye eski blob hiç gitmez.
#[test]
fn refused_resend_is_retried_later_and_accepted_once() {
    // (a) Olay yolu.
    let (mut duo, old) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let new = duo.supersede();
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert!(
        duo.blobs_on(Side::Alice, new).is_empty(),
        "ret: tele çıkmadı"
    );
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.resend_refused, stats.resent_on_new_session), (1, 0));
    duo.alice.pump();
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"b1".to_vec()]);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.turn().is_empty());
    }
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"b1".to_vec()],
        "tam bir kabul"
    );
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.resend_refused, stats.resent_on_new_session), (1, 1));
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, b"b1"),
        ]
    );

    // (b) Senkron yol.
    let (mut duo, _) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let new = duo.supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.refuse_next_sends(Side::Alice, 1);
    duo.alice.pump();
    assert!(duo.blobs_on(Side::Alice, new).is_empty());
    duo.alice.pump();
    duo.alice.pump();
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"b1".to_vec()]);
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.resend_refused, stats.resent_on_new_session), (1, 1));

    // (c) `send` yolu.
    let (mut duo, _) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let new = duo.supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.refuse_next_sends(Side::Alice, 1);
    duo.alice
        .send(BOB, b"b2")
        .expect("yeniden sunumun reddi yeni blob'u durdurmaz");
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"b2".to_vec()]);
    duo.alice.pump();
    duo.alice.pump();
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"b2".to_vec(), b"b1".to_vec()],
        "eski blob sonraki pump'ta, tam bir kez"
    );

    // (d) Eşin iki kaydı: ilkinin yeniden sunumu reddedilince ikincisi de bekler (sıra korunur);
    // sonraki `pump` ikisini gönderim sırasıyla sunar.
    let (mut duo, _) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.alice.send(BOB, b"b2").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let new = duo.supersede();
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(duo.alice.drain().len(), 2, "Ready(h') + Closed(h)");
    assert!(
        duo.blobs_on(Side::Alice, new).is_empty(),
        "ikinci kayıt birincinin önüne geçmedi"
    );
    duo.alice.pump();
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        vec![b"b1".to_vec(), b"b2".to_vec()]
    );
}

/// Eski tutamaç GEÇİCİ rettir: Alice'in Messenger'ı yeni oturumu (h2) senkron benimser, ama Transport o
/// arada bir supersession daha görmüştür (h3; olaylar henüz okunmadı). `pump`'ın h2'ye yeniden sunumu
/// `SessionSuperseded` alır — kayıt DÜŞMEZ (`records_failed` = 0); `Ready(h3)` okununca blob h3'e TAM
/// BİR KEZ sunulur ve Bob onu alır. Eski tutamaçtaki ret iki kezdir: `pump`'ta ve `Ready(h2)`
/// geçirilirken.
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): yeniden sunum yoktu (h3'e blob gitmez).
#[test]
fn a_resend_refused_by_a_stale_handle_waits_for_the_newer_session() {
    let (mut duo, first) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    let second = duo.supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(second))
    );
    let third = duo.supersede();
    duo.alice.pump();
    let stats = duo.alice.signaling.stats();
    assert_eq!(
        (stats.resend_refused, stats.records_failed),
        (1, 0),
        "SessionSuperseded geçici: kayıt kaldı"
    );
    assert_eq!(duo.alice.signaling.pending_count(BOB), 1);
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(second, BOB)),
            session(SessionEvent::Closed(first, CloseReason::Superseded)),
            session(SessionEvent::Ready(third, BOB)),
            session(SessionEvent::Closed(second, CloseReason::Superseded)),
        ]
    );
    assert!(duo.blobs_on(Side::Alice, second).is_empty());
    assert_eq!(duo.blobs_on(Side::Alice, third), vec![b"b1".to_vec()]);
    let stats = duo.alice.signaling.stats();
    assert_eq!(
        (
            stats.resend_refused,
            stats.resent_on_new_session,
            stats.records_failed
        ),
        (2, 1, 0)
    );
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(second, ALICE)),
            session(SessionEvent::Closed(first, CloseReason::Superseded)),
            session(SessionEvent::Ready(third, ALICE)),
            session(SessionEvent::Closed(second, CloseReason::Superseded)),
            received(ALICE, b"b1"),
        ]
    );
}

/// "Oturum başına bir kez": aynı kayıtlar art arda İKİ supersession görür; her yeni tutamaca her blob
/// TAM BİR KEZ gider, önceki tutamaca bir daha verilmez (`resent_on_new_session` = 2 blob × 2 oturum).
/// Bob her oturumdan blob'ları alır (dedup yok).
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): yeniden sunum yoktu (yeni tutamaçlara hiçbir blob gitmez).
#[test]
fn each_new_session_gets_one_resend_per_blob() {
    let (mut duo, first) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.alice.send(BOB, b"b2").expect("gönderilir");
    assert_eq!(
        duo.bob.turn(),
        vec![received(ALICE, b"b1"), received(ALICE, b"b2")]
    );
    let both = vec![b"b1".to_vec(), b"b2".to_vec()];
    let mut previous = first;
    for _ in 0..2 {
        let current = duo.supersede();
        assert_eq!(
            duo.alice.turn(),
            vec![
                session(SessionEvent::Ready(current, BOB)),
                session(SessionEvent::Closed(previous, CloseReason::Superseded)),
            ]
        );
        for _ in 0..IDLE_ROUNDS {
            assert!(duo.alice.turn().is_empty());
        }
        assert_eq!(duo.blobs_on(Side::Alice, current), both);
        assert_eq!(
            duo.bob.turn(),
            vec![
                session(SessionEvent::Ready(current, ALICE)),
                session(SessionEvent::Closed(previous, CloseReason::Superseded)),
                received(ALICE, b"b1"),
                received(ALICE, b"b2"),
            ]
        );
        previous = current;
    }
    assert_eq!(
        duo.blobs_on(Side::Alice, first),
        both,
        "ilk tutamaca yalnız asıl gönderimler"
    );
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.resent_on_new_session, stats.resend_refused), (4, 0));
}

/// Eşin oturumu kapanırsa (`Closed`, supersession değil) kayıt kalır ve eşin SIRADAKİ oturumunda aynı
/// kuralla sunulur.
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): yeniden sunum yoktu.
#[test]
fn a_closed_session_keeps_the_record_for_the_next_session() {
    let (mut duo, first) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    for side in [Side::Alice, Side::Bob] {
        duo.link.borrow_mut().end(side, CloseReason::PeerClosed);
    }
    let closed = vec![session(SessionEvent::Closed(
        first,
        CloseReason::PeerClosed,
    ))];
    assert_eq!(duo.alice.turn(), closed);
    assert_eq!(duo.bob.turn(), closed);
    assert_eq!(duo.alice.signaling.pending_count(BOB), 1, "kayıt kaldı");
    let second = duo.link.borrow_mut().establish();
    assert_eq!(
        duo.alice.turn(),
        vec![session(SessionEvent::Ready(second, BOB))]
    );
    assert_eq!(duo.blobs_on(Side::Alice, second), vec![b"b1".to_vec()]);
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(second, ALICE)),
            received(ALICE, b"b1"),
        ]
    );
}

// ── tutma kuralı: ömür ve tavan (ÖNERİ — DUR-K4G-14a, sahip kararı bekliyor) ────────────────────

/// Ömür ilk KABUL anından sayılır ve yeniden sunum UZATMAZ: blob ömrünün bir adım öncesine dek her yeni
/// oturuma sunulur; tam sınırda (ilk kabulden `SIGNALING_RESEND_TTL` sonra) budanır ve sunulmaz — o
/// arada yeniden sunulmuş olsa da. Oturum değişimi olmadan da `pump` ömrü dolanı budar. Değer kilidi:
/// öneri 60 s (DUR-K4G-14a; sahip kararı gelene dek değişmez).
///
/// `ea0d3909`'da UYARLANAMAZ: yeniden sunum kaydı, ömrü ve `pending_count` yoktu.
#[test]
fn resend_record_expires_after_the_ttl_from_first_acceptance() {
    assert_eq!(SIGNALING_RESEND_TTL, Duration::from_secs(60));
    let (mut duo, first) = Duo::connected();
    duo.alice.send(BOB, b"b1").expect("gönderilir");
    assert_eq!(duo.bob.turn(), vec![received(ALICE, b"b1")]);

    duo.clock.advance(SIGNALING_RESEND_TTL - TICK);
    let second = duo.supersede();
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(second, BOB)),
            session(SessionEvent::Closed(first, CloseReason::Superseded)),
        ]
    );
    assert_eq!(
        duo.blobs_on(Side::Alice, second),
        vec![b"b1".to_vec()],
        "sınırdan bir adım önce sunulur"
    );

    duo.clock.advance(TICK);
    let third = duo.supersede();
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(third, BOB)),
            session(SessionEvent::Closed(second, CloseReason::Superseded)),
        ]
    );
    assert!(
        duo.blobs_on(Side::Alice, third).is_empty(),
        "tam sınırda budandı; yeniden sunum ömrü uzatmadı"
    );
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.resent_on_new_session, stats.records_expired), (1, 1));
    assert_eq!(duo.alice.signaling.pending_count(BOB), 0);
    // Bob'un Messenger'ı olayları birimlerden önce okur; ikinci oturumdan gelen tek blob sonda.
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(second, ALICE)),
            session(SessionEvent::Closed(first, CloseReason::Superseded)),
            session(SessionEvent::Ready(third, ALICE)),
            session(SessionEvent::Closed(second, CloseReason::Superseded)),
            received(ALICE, b"b1"),
        ]
    );

    // Oturum değişimi olmadan: `pending_count` ömrü dolanı saymaz, `pump` budar.
    duo.alice.send(BOB, b"b2").expect("gönderilir");
    assert_eq!(duo.alice.signaling.pending_count(BOB), 1);
    duo.clock.advance(SIGNALING_RESEND_TTL);
    assert_eq!(duo.alice.signaling.pending_count(BOB), 0);
    duo.alice.pump();
    assert_eq!(duo.alice.signaling.stats().records_expired, 2);
}

/// Budama bir eşin süresi dolmuş BÜTÜN kayıtlarını düşürür, yalnız ilkini değil: aynı eşe üç blob ömrün
/// başında, bir blob yarı ömürde gönderilir ve hepsi eski oturumla yolda kaybolur; ilk üçünün ömrü
/// dolduğunda yeni oturuma YALNIZ canlı olan sunulur (`records_expired` = 3).
///
/// `ea0d3909`'da UYARLANAMAZ: yeniden sunum kaydı ve ömrü yoktu.
#[test]
fn every_expired_record_is_pruned_and_only_live_ones_are_resent() {
    let (mut duo, old) = Duo::connected();
    for bytes in [b"x1", b"x2", b"x3"] {
        duo.alice.send(BOB, bytes).expect("gönderilir");
    }
    let half = SIGNALING_RESEND_TTL / 2;
    duo.clock.advance(half);
    duo.alice.send(BOB, b"live").expect("gönderilir");
    duo.take_inbox(Side::Bob);
    // İlk üçü tam sınırda (ömür doldu), sonuncusu yarı ömürde.
    duo.clock.advance(SIGNALING_RESEND_TTL - half);
    assert_eq!(duo.alice.signaling.pending_count(BOB), 1);
    let new = duo.supersede();
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(duo.blobs_on(Side::Alice, new), vec![b"live".to_vec()]);
    let stats = duo.alice.signaling.stats();
    assert_eq!((stats.records_expired, stats.resent_on_new_session), (3, 1));
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, b"live"),
        ]
    );
}

/// Eş başına tavan: tavan + 1 blob gönderilir (hepsi eski oturumla yolda kaybolur); gönderim hiç
/// reddedilmez, EN ESKİ kayıt düşer (`records_evicted` = 1). Supersession'da yalnız kalan kayıtlar,
/// gönderim sırasıyla, her biri TAM BİR KEZ yeniden sunulur — Bob'un teslim kuyruğu tavanı yeniden
/// sunumu geri basınçla turlara bölse de. Değer kilidi: öneri 256 (DUR-K4G-14a; sahip kararı gelene
/// dek değişmez).
///
/// `ea0d3909`'da UYARLANAMAZ: yeniden sunum kaydı ve tavanı yoktu.
#[test]
fn resend_record_is_bounded_per_peer_and_the_oldest_goes_first() {
    assert_eq!(SIGNALING_RESEND_PER_PEER, 256);
    let (mut duo, old) = Duo::connected();
    for n in 0..=SIGNALING_RESEND_PER_PEER {
        duo.alice
            .send(BOB, &blob(n))
            .expect("tavan gönderimi durdurmaz");
        if n.is_multiple_of(LINK_QUEUE_CAP / 2) {
            duo.take_inbox(Side::Bob);
        }
    }
    duo.take_inbox(Side::Bob);
    assert_eq!(
        duo.alice.signaling.pending_count(BOB),
        SIGNALING_RESEND_PER_PEER
    );
    assert_eq!(duo.alice.signaling.stats().records_evicted, 1);

    let new = duo.supersede();
    let bob_events = duo.settle();
    let kept: Vec<Vec<u8>> = (1..=SIGNALING_RESEND_PER_PEER).map(blob).collect();
    assert_eq!(
        duo.blobs_on(Side::Alice, new),
        kept,
        "en eski (0) düştü; kalanlar sırayla, birer kez"
    );
    let expected: Vec<SignalingEvent> = [
        session(SessionEvent::Ready(new, ALICE)),
        session(SessionEvent::Closed(old, CloseReason::Superseded)),
    ]
    .into_iter()
    .chain(kept.iter().map(|bytes| received(ALICE, bytes)))
    .collect();
    assert_eq!(bob_events, expected);
    let stats = duo.alice.signaling.stats();
    assert_eq!(
        stats.resent_on_new_session,
        u64::try_from(SIGNALING_RESEND_PER_PEER).expect("küçük sayı")
    );
    assert!(
        stats.resend_refused > 0,
        "Bob'un kuyruğu yeniden sunumu geri bastı"
    );
}

// ── dedup yok, kimlik, geçirgenlik ─────────────────────────────────────────────────────────────

/// Dedup YOK (AMP §3, K4G-14a): 0x09 `msg_id` taşımaz; alıcı aynı baytlarla gelen blob'u her seferinde
/// teslim eder — aynı oturumdan iki kez gelse de, eski oturumun drain'inden geç gelip yeni oturumdaki
/// yeniden sunumuyla iki kez görülse de. Yinelenen blob'u medya motoru idempotent işler (beyanlı).
///
/// `ea0d3909`'da KIRMIZI (uyarlansa da): (iii)'te yeni oturumdan gelen iki kopya eksik kalır —
/// yeniden sunum yoktu. (i) ve (ii)'nin teslim kısmı AYNI DAVRANIŞ: Messenger her 0x09 kaydını
/// dedup'sız `Other` olarak geçiriyordu (regresyon kilidi: dedup eklenmedi).
#[test]
fn receiver_has_no_dedup_and_every_copy_is_delivered() {
    let (mut duo, old) = Duo::connected();
    // (i) Aynı kayıt aynı oturumdan iki kez.
    let same = record(&AppMessage::SignalingBlob {
        blob: b"ice".to_vec(),
    });
    duo.inject(Side::Bob, same.clone());
    duo.inject(Side::Bob, same);
    assert_eq!(
        duo.bob.turn(),
        vec![received(ALICE, b"ice"), received(ALICE, b"ice")]
    );
    // (ii) Uygulama aynı baytları iki kez gönderir: iki kimlik, iki teslim.
    let first = duo.alice.send(BOB, b"offer").expect("gönderilir");
    let second = duo.alice.send(BOB, b"offer").expect("gönderilir");
    assert_ne!(first, second);
    // (iii) Eski oturumda yolda kalan iki kopya sonra drain'den geç gelir; yeni oturumdaki yeniden
    // sunumlar da gelir: dört teslim.
    let in_flight = duo.take_inbox(Side::Bob);
    assert_eq!(in_flight.len(), 2);
    let new = duo.supersede();
    assert_eq!(
        duo.alice.turn(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    duo.deliver_late(Side::Bob, in_flight);
    assert_eq!(
        duo.bob.turn(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, b"offer"),
            received(ALICE, b"offer"),
            received(ALICE, b"offer"),
            received(ALICE, b"offer"),
        ]
    );
    assert_eq!(duo.bob.signaling.stats().blobs_received, 6);
}

/// Eşin güncel oturumu kalmasa da gelen blob teslim edilir: Bob'un oturumu `Closed` ile bitti,
/// kapanıştan habersiz Alice'in blob'u kapanmış tutamaçla gelir ve eşine bağlanır. İçerik opaktır,
/// geçerliliğine medya motoru karar verir (beyanlı; PTT'nin `opens_without_session` gibi bir düşürme
/// kuralı plan metninde yok).
///
/// `ea0d3909`'da AYNI DAVRANIŞ (uyarlanınca): Messenger emekli tutamaçtan gelen 0x09'u eşine bağlayıp
/// `Other` olarak geçiriyordu.
#[test]
fn a_blob_after_the_session_ended_is_still_delivered() {
    let (mut duo, handle) = Duo::connected();
    duo.link
        .borrow_mut()
        .end(Side::Bob, CloseReason::UserClosed);
    assert_eq!(
        duo.bob.turn(),
        vec![session(SessionEvent::Closed(
            handle,
            CloseReason::UserClosed
        ))]
    );
    assert_eq!(duo.bob.messenger.session_of(ALICE), None);
    duo.alice.send(BOB, b"late").expect("Alice'in oturumu açık");
    assert_eq!(duo.bob.turn(), vec![received(ALICE, b"late")]);
}

/// `SignalingId` motor-tekildir (eş başına sayaç DEĞİL): iki eşe karışık gönderimde kimlikler
/// `FIRST_SIGNALING_ID`'den kesin artar; hata yolları (`NoSession`, `TooLarge`) kimlik tüketmez;
/// kayıtlar ömürle düştükten sonra da hiçbir kimlik yeniden verilmez. Ortak yansıtıcı sahte: her blob
/// kendi eşinin oturumundan, gönderim sırasıyla geri döner.
///
/// `ea0d3909`'da UYARLANAMAZ: `SignalingId` yoktu.
#[test]
fn signaling_ids_are_engine_unique_strictly_increasing_and_never_reused() {
    let mut messenger =
        Messenger::with_store(MockTransport::new(caps()), fresh_store()).expect("limitler geçerli");
    let clock = ManualClock::started();
    let mut signaling = Signaling::new(Arc::new(clock.clone()));
    for p in [BOB, CAROL] {
        assert!(matches!(
            messenger.connect(p),
            Ok(ConnectResult::Pending(_))
        ));
    }
    messenger.transport_mut().pump();
    let mut ready = Vec::new();
    while let Some(event) = messenger.poll() {
        if let SignalingEvent::Messenger(MessengerEvent::Session(SessionEvent::Ready(_, p))) =
            signaling.handle(&mut messenger, event)
        {
            ready.push(p);
        }
    }
    assert_eq!(ready, vec![BOB, CAROL]);

    let mut ids = Vec::new();
    let mut targets = Vec::new();
    let mut send = |signaling: &mut Signaling, messenger: &mut Messenger<MockTransport>, p| {
        let id = signaling.send(messenger, p, b"s").expect("gönderilir");
        ids.push(id);
        targets.push(p);
    };
    send(&mut signaling, &mut messenger, BOB);
    send(&mut signaling, &mut messenger, CAROL);
    send(&mut signaling, &mut messenger, BOB);
    assert_eq!(
        signaling.send(&mut messenger, DAVE, b"s"),
        Err(SignalingError::NoSession)
    );
    let over = vec![0; max_blob_bytes(messenger.limits()) + 1];
    assert!(matches!(
        signaling.send(&mut messenger, CAROL, &over),
        Err(SignalingError::TooLarge { .. })
    ));
    send(&mut signaling, &mut messenger, CAROL);
    clock.advance(SIGNALING_RESEND_TTL);
    signaling.pump(&mut messenger);
    assert_eq!(signaling.pending_count(BOB), 0);
    assert_eq!(signaling.pending_count(CAROL), 0);
    send(&mut signaling, &mut messenger, BOB);

    let expected: Vec<u64> = (0..5).map(|n| FIRST_SIGNALING_ID.get() + n).collect();
    let got: Vec<u64> = ids.iter().map(|id| id.get()).collect();
    assert_eq!(got, expected, "motor-tekil, kesin artan, boşluksuz");
    // Yansıtıcı: her blob kendi eşinin oturumundan döner.
    let mut echoed = Vec::new();
    while let Some(event) = messenger.poll() {
        if let SignalingEvent::SignalingReceived { peer, bytes } =
            signaling.handle(&mut messenger, event)
        {
            assert_eq!(bytes, b"s");
            echoed.push(peer);
        }
    }
    assert_eq!(echoed, targets);
}

/// `SIGNALING_BLOB` olmayan her olay zincirden AYNEN geçer: metin, alındı, `RECEIPT(read)` (Messenger,
/// Ptt ve FileTransfer'ın geçirdiği `Other`), oturum olayları. Sinyalleşme sayaçları kıpırdamaz.
///
/// `ea0d3909`'da AYNI DAVRANIŞ (uyarlanınca): zincirin geri kalanı aynı olayları veriyordu; yalnız
/// `Signaling` halkası yoktu.
#[test]
fn non_signaling_events_pass_through_the_chain_unchanged() {
    let (mut duo, _) = Duo::connected();
    let msg_id = duo
        .alice
        .messenger
        .send_text(BOB, "metin")
        .expect("gönderilir");
    let read = AppMessage::Receipt {
        ref_msg_id: msg_id,
        kind: ReceiptKind::Read,
    };
    duo.inject(Side::Bob, record(&read));
    assert_eq!(
        duo.bob.turn(),
        vec![
            SignalingEvent::Messenger(MessengerEvent::TextReceived {
                peer: ALICE,
                msg_id,
                text: "metin".to_owned(),
            }),
            SignalingEvent::Messenger(MessengerEvent::Other {
                peer: ALICE,
                message: read,
            }),
        ]
    );
    assert_eq!(
        duo.alice.turn(),
        vec![SignalingEvent::Messenger(MessengerEvent::Delivered {
            peer: BOB,
            msg_id
        })]
    );
    assert_eq!(duo.alice.signaling.stats(), SignalingStats::default());
    assert_eq!(duo.bob.signaling.stats(), SignalingStats::default());
}
