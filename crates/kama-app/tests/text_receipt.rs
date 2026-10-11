//! B2.2 — metin + alındı (`kama_app::messaging`) uçtan uca; K4G-07 — kalıcı `msg_id` ve outbox.
//!
//! Beklenen davranışın kaynağı: AMP §3 (`msg_id`, `RECEIPT`; S2 yaması 2026-10-08, K4G-07 yaması
//! 2026-10-12), S1 K2/K7/K8/§4, S7 (`MSG_DEDUP_WINDOW`, `MAX_OUTBOX_UNACKED`), plan D4 (v1.1).
//! Her motor bir `AppStore` ile kurulur; yeniden başlatma, AYNI depoyu yeni bir motora vermektir.
//! İki düzenek:
//!
//! * **`Link` (iki uç):** ortak sahte iki ayrı motoru birbirine bağlayamaz (yansıtıcıdır), bu
//!   yüzden bu dosyada küçük, sınırlı bir çift var: bir uçtan gönderilen birim karşı ucun teslim
//!   kuyruğuna düşer. Oturum kurma, supersession, kayıp, ham kayıt enjeksiyonu ve bir ucun çöküp
//!   yeniden bağlanması testin elindedir. Supersession'da eski tutamaçla yoldaki birimler karşı
//!   kuyrukta KALIR (yalnız-alım drain'in karşılığı); iki uç aynı tutamaç değerini görür
//!   (sadeleştirme).
//! * **`MockTransport` (yansıtıcı, `tests/common/mock_transport.rs`):** gönderilen birimi AYNI uca
//!   "eşten gelmiş" gibi geri verir. Burada yankı eşi olarak kullanılır: motorun gönderen yarısı
//!   (sayaç, outbox) ile alan yarısı (dedup) aynı `PeerRef` anahtarında ama AYRI durumlardır —
//!   TEXT(n) geri döner, alan yarı onu işleyip RECEIPT(n) üretir, RECEIPT de döner ve gönderen yarı
//!   `Delivered(n)` yayınlar. Kayıp enjeksiyonu + recovery anahtarı (H-APP-RETRY-0) ve peer
//!   supersession bu sahtenin.

mod common;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use common::mock_transport::{MockTransport, PROBE_INTERVAL};
use kama_app::codec::limits::{MSG_TYPE_RESERVED_HIGH_START, MSG_VERSION_V1, TEXT_PREFIX_BYTES};
use kama_app::codec::{decode, encode, AmpLimits, AppMessage, EncodeError, ReceiptKind};
use kama_app::messaging::{
    Messenger, MessengerError, MessengerEvent, MsgId, FIRST_MSG_ID, MAX_OUTBOX_UNACKED,
    MSG_DEDUP_WINDOW,
};
use kama_app::store::{
    AppStore, AppStoreError, CounterKind, DedupWindow, MemoryAppStore, OutboxUnit,
};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy,
    KamaTransport, PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};

/// Test parametresi (normatif değil): codec vektörlerinin üretildiği değerle aynı.
const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Çiftin birim tavanı (`max_plaintext_unit_bytes`, `usize`).
const LINK_MAX_UNIT_BYTES: usize = TEST_MAX_PLAINTEXT_UNIT_BYTES as usize;
/// Çiftin yön başına teslim kuyruğu tavanı (sahte sınır; aşımı `Backpressure`).
const LINK_QUEUE_CAP: usize = 64;
/// Uç başına olay kuyruğu tavanı (sahte sınır).
const LINK_EVENT_CAP: usize = 16;
/// `drain` döngüsünün tavanı.
const DRAIN_CAP: usize = 1024;
/// "Zaman geçse de" tur sayısı (H-APP-RETRY-0).
const IDLE_ROUNDS: usize = 32;
/// UTF-8'de hiçbir konumda geçerli olmayan bayt.
const INVALID_UTF8_BYTE: u8 = 0xFF;

// Transport'un geri basıncı outbox tavanından ÖNCE görünür (`send_errors_…` buna dayanır).
const _: () = assert!(LINK_QUEUE_CAP < MAX_OUTBOX_UNACKED);

const ALICE: PeerRef = PeerRef::from_dest_hash([0xA1; 32]);
const BOB: PeerRef = PeerRef::from_dest_hash([0xB0; 32]);
const CAROL: PeerRef = PeerRef::from_dest_hash([0xC0; 32]);

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

fn text_record(msg_id: MsgId, text: &str) -> Vec<u8> {
    record(&AppMessage::Text {
        msg_id,
        text: text.to_owned(),
    })
}

fn receipt(ref_msg_id: MsgId) -> AppMessage {
    AppMessage::Receipt {
        ref_msg_id,
        kind: ReceiptKind::Delivered,
    }
}

fn receipt_record(ref_msg_id: MsgId) -> Vec<u8> {
    record(&receipt(ref_msg_id))
}

fn received(peer: PeerRef, msg_id: MsgId, text: &str) -> MessengerEvent {
    MessengerEvent::TextReceived {
        peer,
        msg_id,
        text: text.to_owned(),
    }
}

fn delivered(peer: PeerRef, msg_id: MsgId) -> MessengerEvent {
    MessengerEvent::Delivered { peer, msg_id }
}

fn session(event: SessionEvent) -> MessengerEvent {
    MessengerEvent::Session(event)
}

/// Taze bellek içi depo — kalıcı durumu olmayan bir cihaz.
fn fresh_store() -> Arc<dyn AppStore> {
    Arc::new(MemoryAppStore::new())
}

/// Depodaki kalıcı outbox'ın `msg_id`'leri, artan sırayla.
fn outbox_ids(store: &dyn AppStore, peer: PeerRef) -> Vec<MsgId> {
    store
        .outbox_list(peer)
        .expect("depo okunur")
        .iter()
        .map(OutboxUnit::msg_id)
        .collect()
}

/// Arıza enjekte edilebilen depo: işlem başına bir anahtar; anahtar kapalıyken her işlem bellek
/// içi depoya gider, açıkken `Persist` döner ve HİÇBİR şey yazmaz.
#[derive(Default)]
struct FlakyStore {
    inner: MemoryAppStore,
    fail_reserve: AtomicBool,
    fail_outbox_put: AtomicBool,
    fail_outbox_remove: AtomicBool,
    fail_outbox_list: AtomicBool,
}

impl FlakyStore {
    fn check(switch: &AtomicBool) -> Result<(), AppStoreError> {
        if switch.load(Ordering::SeqCst) {
            Err(AppStoreError::Persist)
        } else {
            Ok(())
        }
    }
}

impl AppStore for FlakyStore {
    fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
        Self::check(&self.fail_reserve)?;
        self.inner.reserve_next(peer, kind)
    }

    fn dedup_load(&self, peer: PeerRef) -> Result<DedupWindow, AppStoreError> {
        self.inner.dedup_load(peer)
    }

    fn dedup_save(&self, peer: PeerRef, window: &DedupWindow) -> Result<(), AppStoreError> {
        self.inner.dedup_save(peer, window)
    }

    fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError> {
        Self::check(&self.fail_outbox_put)?;
        self.inner.outbox_put(peer, unit)
    }

    fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError> {
        Self::check(&self.fail_outbox_remove)?;
        self.inner.outbox_remove(peer, msg_id)
    }

    fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
        Self::check(&self.fail_outbox_list)?;
        self.inner.outbox_list(peer)
    }
}

/// `poll` `None` verene kadar olayları toplar.
fn drain<T: KamaTransport>(messenger: &mut Messenger<T>) -> Vec<MessengerEvent> {
    let mut events = Vec::new();
    while let Some(event) = messenger.poll() {
        assert!(events.len() < DRAIN_CAP, "olay akışı bitmiyor");
        events.push(event);
    }
    events
}

// ── iki uçlu çift ────────────────────────────────────────────────────────────────────────────

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

    /// Karşı uç.
    const fn other(self) -> Self {
        match self {
            Self::Alice => Self::Bob,
            Self::Bob => Self::Alice,
        }
    }
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
    /// Bu uçtan KABUL edilen gönderim sayısı (kaybolanlar dahil).
    accepted: u64,
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
    /// Sonraki bu kadar gönderim (hangi uçtan olursa) kabul edilir ve KAYBOLUR (kurtarma yok).
    lose_next: usize,
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

    /// Supersession onayı (S1 K7): her uca `Ready(h')` sonra `Closed(h, Superseded)`.
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

    /// `side` ucunun süreci öldü: Transport durumu (oturum, kuyruklar, olaylar) yok olur. Karşı uç
    /// bunu bilmez; eski oturumu hâlâ aktiftir.
    fn crash(&mut self, side: Side) {
        *self.endpoint(side) = Endpoint::default();
    }

    /// Çöküp yeniden başlayan `side` ucu eşle yeni oturum kurar: o uca yalnız `Ready(h')`; karşı
    /// uca eş-yeniden-başlatma supersession'ı (identity §16): `Ready(h')` sonra
    /// `Closed(h, Superseded)`.
    fn reconnect_after_crash(&mut self, side: Side) -> SessionHandle {
        let handle = self.fresh_handle();
        let restarted = self.endpoint(side);
        assert!(
            restarted.active.is_none(),
            "yeniden başlayan uçta oturum olamaz"
        );
        restarted.active = Some(handle);
        restarted.push_event(SessionEvent::Ready(handle, side.remote()));
        let peer_side = side.other();
        let peer = self.endpoint(peer_side);
        let old = peer
            .active
            .replace(handle)
            .expect("karşı uçta eski oturum yok");
        peer.superseded.push(old);
        peer.push_event(SessionEvent::Ready(handle, peer_side.remote()));
        peer.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
        handle
    }

    fn lose_next_units(&mut self, n: usize) {
        self.lose_next += n;
    }

    /// `to` ucuna, aktif oturumu üzerinden eşten gelmiş gibi ham kayıt koyar.
    fn inject(&mut self, to: Side, unit: Vec<u8>) {
        let endpoint = self.endpoint(to);
        let session = endpoint.active.expect("inject: aktif oturum yok");
        assert!(endpoint.inbox.len() < LINK_QUEUE_CAP, "inject: kuyruk dolu");
        endpoint.inbox.push_back(Delivered { session, unit });
    }
}

/// Bir uç: `KamaTransport`'u paylaşılan `Link` üzerinde uygular.
struct End {
    link: Rc<RefCell<Link>>,
    side: Side,
}

impl End {
    fn new(link: &Rc<RefCell<Link>>, side: Side) -> Self {
        Self {
            link: Rc::clone(link),
            side,
        }
    }
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
        _policy: DeliveryPolicy,
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
        if link.lose_next > 0 {
            link.lose_next -= 1;
            me.accepted += 1;
            return Ok(());
        }
        if remote.inbox.len() >= LINK_QUEUE_CAP {
            return Err(TransportError::Backpressure);
        }
        remote.inbox.push_back(Delivered {
            session,
            unit: unit.bytes,
        });
        me.accepted += 1;
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

/// İki uçlu düzenek: Alice'in ve Bob'un motorları + testin `Link` denetimi.
struct Duo {
    link: Rc<RefCell<Link>>,
    alice: Messenger<End>,
    bob: Messenger<End>,
}

impl Duo {
    fn unconnected() -> Self {
        Self::unconnected_with(fresh_store())
    }

    /// Alice'in motoru `alice_store` ile (yeniden başlatma testleri aynı depoyu yeniden verir),
    /// Bob'unki taze depoyla.
    fn unconnected_with(alice_store: Arc<dyn AppStore>) -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let alice = Messenger::with_store(End::new(&link, Side::Alice), alice_store)
            .expect("limitler geçerli");
        let bob = Messenger::with_store(End::new(&link, Side::Bob), fresh_store())
            .expect("limitler geçerli");
        Self { link, alice, bob }
    }

    /// Oturum kurulmuş, iki tarafın `Ready` olayı okunmuş düzenek.
    fn connected() -> (Self, SessionHandle) {
        Self::connected_with(fresh_store())
    }

    /// [`Duo::connected`]; Alice'in deposu verilir.
    fn connected_with(alice_store: Arc<dyn AppStore>) -> (Self, SessionHandle) {
        let mut duo = Self::unconnected_with(alice_store);
        let handle = duo.link.borrow_mut().establish();
        assert_eq!(
            drain(&mut duo.alice),
            vec![session(SessionEvent::Ready(handle, BOB))]
        );
        assert_eq!(
            drain(&mut duo.bob),
            vec![session(SessionEvent::Ready(handle, ALICE))]
        );
        (duo, handle)
    }

    /// Alice'in süreci ölür ve `store` ile yeniden başlar: eski motor ve Transport durumu (oturum,
    /// kuyruklar, olaylar) yok olur; Bob'un ucu bunu bilmez. Yeni oturumu test kurar
    /// ([`Link::reconnect_after_crash`]).
    fn restart_alice(&mut self, store: Arc<dyn AppStore>) {
        self.link.borrow_mut().crash(Side::Alice);
        self.alice = Messenger::with_store(End::new(&self.link, Side::Alice), store)
            .expect("limitler geçerli");
    }

    fn inject(&self, to: Side, unit: Vec<u8>) {
        self.link.borrow_mut().inject(to, unit);
    }

    /// `side` ucunun teslim kuyruğunu motoruna vermeden boşaltır ve çözer (telde ne var).
    fn raw_inbox(&self, side: Side) -> Vec<AppMessage> {
        let limits = limits();
        let units: Vec<Delivered> = self
            .link
            .borrow_mut()
            .endpoint(side)
            .inbox
            .drain(..)
            .collect();
        units
            .iter()
            .map(|unit| decode(&unit.unit, &limits).expect("geçerli kayıt"))
            .collect()
    }
}

// ── yansıtıcı sahte ──────────────────────────────────────────────────────────────────────────

fn mock_messenger() -> Messenger<MockTransport> {
    Messenger::with_store(MockTransport::new(caps()), fresh_store()).expect("limitler geçerli")
}

/// Yeni eşe `connect` → sahte el sıkışmayı tamamlar → iki olay okunur: F0.2(ii) `Measuring`,
/// sonra `Ready`.
fn connect_mock(messenger: &mut Messenger<MockTransport>, peer: PeerRef) -> SessionHandle {
    let ConnectResult::Pending(ticket) = messenger.connect(peer).expect("connect") else {
        panic!("bilinmeyen eşe ilk connect Pending olmalı");
    };
    let handle = messenger.transport_mut().complete_connect(&peer);
    assert_eq!(
        drain(messenger),
        vec![
            session(SessionEvent::ConnectProgress(
                ticket,
                ConnectPhase::Measuring
            )),
            session(SessionEvent::Ready(handle, peer)),
        ]
    );
    assert_eq!(messenger.session_of(peer), Some(handle));
    handle
}

// ── testler ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn text_is_delivered_once_and_receipted_once() {
    let (mut duo, _) = Duo::connected();
    let msg_id = duo.alice.send_text(BOB, "merhaba").expect("gönderilir");
    // Transport'un kabulü teslim değildir (S1 K8): Bob işlemeden Delivered yok.
    assert!(drain(&mut duo.alice).is_empty());
    assert_eq!(duo.alice.unacked_count(BOB), 1);
    assert_eq!(
        drain(&mut duo.bob),
        vec![received(ALICE, msg_id, "merhaba")]
    );
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, msg_id)]);
    assert_eq!(duo.alice.unacked_count(BOB), 0);
    // Aynı RECEIPT'in tekrarı ve bilinmeyen bir msg_id'nin RECEIPT'i olay üretmez.
    duo.inject(Side::Alice, receipt_record(msg_id));
    duo.inject(Side::Alice, receipt_record(msg_id + 1));
    assert!(drain(&mut duo.alice).is_empty());
    assert_eq!(duo.alice.stats().receipts_ignored, 2);
    assert_eq!(duo.bob.stats().receipts_sent, 1);
}

#[test]
fn receiver_accepts_out_of_order_msg_ids() {
    let (mut duo, _) = Duo::connected();
    let order: [MsgId; 3] = [5, 3, 4];
    for msg_id in order {
        duo.inject(Side::Bob, text_record(msg_id, "sırasız"));
    }
    let expected: Vec<MessengerEvent> = order
        .iter()
        .map(|&msg_id| received(ALICE, msg_id, "sırasız"))
        .collect();
    assert_eq!(drain(&mut duo.bob), expected);
    // Her biri geliş sırasıyla RECEIPT'lendi.
    let receipts: Vec<AppMessage> = order.iter().map(|&msg_id| receipt(msg_id)).collect();
    assert_eq!(duo.raw_inbox(Side::Alice), receipts);
}

#[test]
fn duplicate_text_is_not_redelivered_but_receipted_again() {
    let (mut duo, _) = Duo::connected();
    duo.inject(Side::Bob, text_record(7, "bir kez"));
    duo.inject(Side::Bob, text_record(7, "bir kez"));
    // `msg_id` kimliktir: aynı `msg_id`'li farklı metin de tekrardır.
    duo.inject(Side::Bob, text_record(7, "başka metin"));
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, 7, "bir kez")]);
    assert_eq!(duo.bob.stats().duplicate_texts, 2);
    // Her TEXT bir RECEIPT(7) aldı (S2: tekrar TEXT → RECEIPT yeniden).
    assert_eq!(duo.raw_inbox(Side::Alice), vec![receipt(7); 3]);
}

#[test]
fn counter_and_dedup_survive_supersession_and_resend_keeps_msg_id() {
    let (mut duo, old) = Duo::connected();
    // (1) `h` üzerinde teslim edildi ve RECEIPT'lendi.
    let first = duo.alice.send_text(BOB, "bir").expect("gönderilir");
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, first, "bir")]);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, first)]);
    // (2) `h` üzerinde kaybolur: Transport kabul etti (`Ok`), Bob hiç görmez.
    duo.link.borrow_mut().lose_next_units(1);
    let lost = duo.alice.send_text(BOB, "iki").expect("kabul edildi");
    // (3) Bob'a ulaşır ama RECEIPT'i `h` ile birlikte kaybolur.
    let unreceipted = duo.alice.send_text(BOB, "üç").expect("gönderilir");
    duo.link.borrow_mut().lose_next_units(1);
    assert_eq!(
        drain(&mut duo.bob),
        vec![received(ALICE, unreceipted, "üç")]
    );
    assert!(
        drain(&mut duo.alice).is_empty(),
        "RECEIPT kayboldu: Delivered olamaz"
    );
    assert_eq!(duo.alice.unacked_count(BOB), 2);

    // Supersession: Transport `h`'nin teslim edilmemiş birimlerini `h'`ye TAŞIMAZ; Alice
    // RECEIPT'siz iki kaydı `h'` üzerinden ESKİ msg_id'leriyle yeniden gönderir (S1 K7).
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        drain(&mut duo.alice),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(duo.alice.stats().resent_on_new_session, 2);
    // Bob: kaybolan → yeni, teslim edilir; RECEIPT'i kaybolan → tekrar, teslim YOK ama RECEIPT
    // yeniden gider (S2).
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, lost, "iki"),
        ]
    );
    assert_eq!(duo.bob.stats().duplicate_texts, 1);
    // Alice: her kayıt için tam bir Delivered.
    assert_eq!(
        drain(&mut duo.alice),
        vec![delivered(BOB, lost), delivered(BOB, unreceipted)]
    );
    assert_eq!(duo.alice.unacked_count(BOB), 0);

    // Sayaç supersession'da sıfırlanmadı.
    let after = duo.alice.send_text(BOB, "dört").expect("gönderilir");
    assert_eq!(after, unreceipted + 1);
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, after, "dört")]);
    // Dedup supersession'da sıfırlanmadı: `h`'de teslim edilmiş ilk mesajın `h'`deki tekrarı
    // teslim edilmez.
    duo.inject(Side::Bob, text_record(first, "bir"));
    assert!(drain(&mut duo.bob).is_empty());
    assert_eq!(duo.bob.stats().duplicate_texts, 2);
    // Alice'in iletimleri: dört kullanıcı gönderimi + iki K7 yeniden gönderimi; başka iletim yok.
    assert_eq!(duo.link.borrow().alice.accepted, 6);
}

#[test]
fn late_unit_on_retired_handle_still_reaches_its_peer() {
    let (mut duo, old) = Duo::connected();
    let msg_id = duo.alice.send_text(BOB, "yolda").expect("gönderilir");
    let new = duo.link.borrow_mut().supersede();
    // Bob olayları birimlerden ÖNCE okur: TEXT eski tutamaçla, `Closed(h)` okunduktan sonra gelir
    // (yalnız-alım drain) ve yine Alice'e bağlanır. RECEIPT güncel tutamaçtan (`h'`) gider.
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, msg_id, "yolda"),
        ]
    );
    assert_eq!(duo.bob.stats().units_unknown_session, 0);
    // Alice RECEIPT'i okumadan `Ready(h')`'yi okur ve kaydı yeniden gönderir: fazlalık iletim
    // Bob'un dedup'ında söner (teslim yok, RECEIPT yeniden); Delivered yine tek.
    assert_eq!(
        drain(&mut duo.alice),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            delivered(BOB, msg_id),
        ]
    );
    assert_eq!(duo.alice.stats().resent_on_new_session, 1);
    assert!(drain(&mut duo.bob).is_empty());
    assert_eq!(duo.bob.stats().duplicate_texts, 1);
    assert!(drain(&mut duo.alice).is_empty());
    assert_eq!(duo.alice.stats().receipts_ignored, 1);
}

#[test]
fn connect_result_ready_is_adopted_without_an_event() {
    let mut duo = Duo::unconnected();
    assert_eq!(
        duo.alice.send_text(BOB, "erken"),
        Err(MessengerError::NoSession)
    );
    let handle = duo.link.borrow_mut().establish();
    // `Ready` olayı henüz okunmadı; idempotent `connect` mevcut tutamacı olaysız döndürür (S1 K2)
    // ve motor onu benimser.
    assert_eq!(duo.alice.connect(BOB), Ok(ConnectResult::Ready(handle)));
    assert_eq!(duo.alice.session_of(BOB), Some(handle));
    let msg_id = duo
        .alice
        .send_text(BOB, "hemen")
        .expect("tutamaç biliniyor");
    assert_eq!(msg_id, FIRST_MSG_ID, "NoSession msg_id tüketmedi");
    // Sonra okunan `Ready` aynı tutamaçtır: yeniden gönderim YOK.
    assert_eq!(
        drain(&mut duo.alice),
        vec![session(SessionEvent::Ready(handle, BOB))]
    );
    assert_eq!(duo.alice.stats().resent_on_new_session, 0);
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(handle, ALICE)),
            received(ALICE, msg_id, "hemen"),
        ]
    );
}

#[test]
fn malformed_records_produce_no_event_and_no_receipt() {
    let (mut duo, _) = Duo::connected();
    let mut invalid_utf8 = text_record(9, "a");
    *invalid_utf8.last_mut().expect("kayıt boş değil") = INVALID_UTF8_BYTE;
    let mut truncated = text_record(9, "kesik");
    truncated.pop();
    // RECEIPT'te `kind` gövdenin son baytıdır.
    let mut unknown_kind = receipt_record(9);
    *unknown_kind.last_mut().expect("kayıt boş değil") = u8::MAX;
    let malformed = [
        Vec::new(),
        invalid_utf8,
        truncated,
        unknown_kind,
        vec![MSG_VERSION_V1 + 1],
        // Başlık: sürüm · reserved `msg_type` · flags · body_len (u16).
        vec![MSG_VERSION_V1, MSG_TYPE_RESERVED_HIGH_START, 0, 0, 0],
    ];
    let count = u64::try_from(malformed.len()).expect("küçük sayı");
    for unit in malformed {
        duo.inject(Side::Bob, unit);
    }
    assert!(drain(&mut duo.bob).is_empty());
    assert_eq!(duo.bob.stats().records_dropped, count);
    // Atılan kayda sinyal gitmez (AMP §5 kural 3): RECEIPT yok.
    assert!(duo.raw_inbox(Side::Alice).is_empty());
    // Atılan kayıt dedup penceresine dokunmadı: aynı `msg_id`'li geçerli TEXT teslim edilir.
    duo.inject(Side::Bob, text_record(9, "geçerli"));
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, 9, "geçerli")]);
}

#[test]
fn records_of_other_lanes_pass_through_without_receipt() {
    let (mut duo, _) = Duo::connected();
    let pending = duo.bob.send_text(ALICE, "okundu mu").expect("gönderilir");
    let file_end = AppMessage::FileEnd { transfer_id: 1 };
    let read = AppMessage::Receipt {
        ref_msg_id: pending,
        kind: ReceiptKind::Read,
    };
    duo.inject(Side::Bob, record(&file_end));
    duo.inject(Side::Bob, record(&read));
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            MessengerEvent::Other {
                peer: ALICE,
                message: file_end,
            },
            MessengerEvent::Other {
                peer: ALICE,
                message: read,
            },
        ]
    );
    // `RECEIPT(read)` M3b'dir; bu motor onu `delivered` saymaz.
    assert_eq!(duo.bob.unacked_count(ALICE), 1);
    // Bu kayıtlar için RECEIPT üretilmedi: Alice'in kuyruğunda yalnız Bob'un TEXT'i var.
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![AppMessage::Text {
            msg_id: pending,
            text: "okundu mu".to_owned(),
        }]
    );
}

#[test]
fn send_errors_are_typed_and_consume_no_msg_id() {
    let (mut duo, handle) = Duo::connected();
    // Bir kayda sığmayan metin: codec hatası aynen (S1 K3; bölmek Application'ın işi).
    let too_long = "a".repeat(duo.alice.limits().max_body_bytes() - TEXT_PREFIX_BYTES + 1);
    assert!(matches!(
        duo.alice.send_text(BOB, &too_long),
        Err(MessengerError::Encode(EncodeError::BodyTooLarge { .. }))
    ));
    // Karşı kuyruk dolu: Transport'un `Backpressure`'ı aynen; kayıt outbox'a girmez.
    let ids: Vec<MsgId> = (0..LINK_QUEUE_CAP)
        .map(|_| duo.alice.send_text(BOB, "dolu").expect("kuyrukta yer var"))
        .collect();
    assert_eq!(
        duo.alice.send_text(BOB, "taşan"),
        Err(MessengerError::Transport(TransportError::Backpressure))
    );
    assert_eq!(duo.alice.unacked_count(BOB), LINK_QUEUE_CAP);
    // Hiçbir hata msg_id tüketmedi: ilk başarılı gönderim ilk msg_id'yi aldı, gerisi ardışık.
    assert_eq!(ids[0], FIRST_MSG_ID);
    assert!(ids.windows(2).all(|pair| pair[1] == pair[0] + 1));
    // Reddedilen metni motor kendisi yeniden DENEMEDİ: Bob yalnız kabul edilenleri görür; kuyruk
    // boşalınca sıradaki msg_id kesintisiz.
    assert_eq!(drain(&mut duo.bob).len(), LINK_QUEUE_CAP);
    assert_eq!(drain(&mut duo.alice).len(), LINK_QUEUE_CAP);
    let next = duo.alice.send_text(BOB, "sonra").expect("yer açıldı");
    assert_eq!(next, ids[LINK_QUEUE_CAP - 1] + 1);
    // Kullanıcı kapattı: `Closed` okununca eşin oturumu yok.
    duo.alice
        .transport_mut()
        .close(handle)
        .expect("aktif tutamaç");
    assert_eq!(
        drain(&mut duo.alice),
        vec![session(SessionEvent::Closed(
            handle,
            CloseReason::UserClosed
        ))]
    );
    assert_eq!(duo.alice.session_of(BOB), None);
    assert_eq!(
        duo.alice.send_text(BOB, "kapalı"),
        Err(MessengerError::NoSession)
    );
}

#[test]
fn msg_ids_strictly_increase_per_peer_and_match_the_wire() {
    let mut messenger = mock_messenger();
    let to_bob = connect_mock(&mut messenger, BOB);
    let to_carol = connect_mock(&mut messenger, CAROL);
    let (mut bob_ids, mut carol_ids) = (Vec::new(), Vec::new());
    for (peer, count) in [(BOB, 3), (CAROL, 2), (BOB, 2), (CAROL, 1)] {
        for _ in 0..count {
            let msg_id = messenger.send_text(peer, "sıra").expect("gönderilir");
            if peer == BOB {
                bob_ids.push(msg_id);
            } else {
                carol_ids.push(msg_id);
            }
        }
    }
    for ids in [&bob_ids, &carol_ids] {
        // Kesin artan ve ardışık: kabul edilen her TEXT bir msg_id tüketir; tekrar ve boşluk yok.
        assert!(ids.windows(2).all(|pair| pair[1] == pair[0] + 1), "{ids:?}");
    }
    // Sayaç eş başınadır: Carol'ın ilk msg_id'si Bob'unkinin devamı değil.
    assert_eq!(bob_ids[0], FIRST_MSG_ID);
    assert_eq!(carol_ids[0], FIRST_MSG_ID);
    // Telde: döndürülen msg_id kaydın kendisindedir (yansıtıcı sahte birimi aynı uca geri verir).
    let (mut wire_bob, mut wire_carol) = (Vec::new(), Vec::new());
    while let Some(unit) = messenger.transport_mut().recv() {
        let AppMessage::Text { msg_id, .. } = decode(&unit.unit, &limits()).expect("geçerli kayıt")
        else {
            panic!("TEXT bekleniyordu");
        };
        if unit.session == to_bob {
            wire_bob.push(msg_id);
        } else {
            assert_eq!(unit.session, to_carol);
            wire_carol.push(msg_id);
        }
    }
    assert_eq!(wire_bob, bob_ids);
    assert_eq!(wire_carol, carol_ids);
}

#[test]
fn outbox_cap_is_typed_backpressure() {
    let mut messenger = mock_messenger();
    let handle = connect_mock(&mut messenger, BOB);
    // Recovery KAPALI: kabul edilen birimler kaybolur, hiçbir RECEIPT gelmez.
    messenger.transport_mut().set_recovery(false);
    messenger
        .transport_mut()
        .drop_next_units(MAX_OUTBOX_UNACKED);
    let ids: Vec<MsgId> = (0..MAX_OUTBOX_UNACKED)
        .map(|_| {
            messenger
                .send_text(BOB, "bekliyor")
                .expect("tavanın altında")
        })
        .collect();
    assert_eq!(messenger.unacked_count(BOB), MAX_OUTBOX_UNACKED);
    assert_eq!(
        messenger.send_text(BOB, "taşan"),
        Err(MessengerError::OutboxFull)
    );
    // Reddedilen metin Transport'a hiç verilmedi.
    let dropped = u64::try_from(MAX_OUTBOX_UNACKED).expect("küçük sayı");
    assert_eq!(messenger.transport().stats().units_dropped, dropped);
    assert!(messenger.transport_mut().recv().is_none());
    // Eşin RECEIPT'i bir yer açar; ret msg_id tüketmediği için sıradaki msg_id kesintisiz.
    let receipt_unit = PreparedUnit {
        bytes: receipt_record(ids[0]),
        compression_policy: CompressionPolicy::Never,
    };
    messenger
        .transport_mut()
        .send(handle, receipt_unit, DeliveryPolicy::Reliable)
        .expect("yansıtılır");
    assert_eq!(drain(&mut messenger), vec![delivered(BOB, ids[0])]);
    let next = messenger.send_text(BOB, "yer açıldı").expect("yer var");
    assert_eq!(next, ids[MAX_OUTBOX_UNACKED - 1] + 1);
}

#[test]
fn unacked_span_is_bounded_by_the_dedup_window() {
    let mut messenger = mock_messenger();
    connect_mock(&mut messenger, BOB);
    messenger.transport_mut().set_recovery(false);
    messenger.transport_mut().drop_next_units(1);
    let oldest = messenger.send_text(BOB, "kayıp").expect("kabul edildi");
    // Yankı eşi: sonraki her mesaj geri döner, işlenir ve RECEIPT'lenir; `oldest` RECEIPT'siz
    // kalır.
    let window = usize::try_from(MSG_DEDUP_WINDOW).expect("u32 → usize");
    for _ in 1..window {
        let msg_id = messenger.send_text(BOB, "yankı").expect("pencere içinde");
        assert_eq!(
            drain(&mut messenger),
            vec![received(BOB, msg_id, "yankı"), delivered(BOB, msg_id)]
        );
    }
    assert_eq!(messenger.unacked_count(BOB), 1);
    // Sıradaki msg_id `oldest`'u alıcı penceresinin altına iterdi: tipli geri basınç.
    assert_eq!(
        messenger.send_text(BOB, "dışarıda"),
        Err(MessengerError::UnackedSpanFull { oldest })
    );
}

#[test]
fn no_resend_without_a_session_change() {
    let mut messenger = mock_messenger();
    let old = connect_mock(&mut messenger, BOB);
    messenger.transport_mut().set_recovery(false);
    messenger.transport_mut().drop_next_units(1);
    let msg_id = messenger.send_text(BOB, "kayıp").expect("kabul edildi");
    // Zaman geçse de, kaç kez sorulursa sorulsun Application yeniden göndermez (H-APP-RETRY-0).
    for _ in 0..IDLE_ROUNDS {
        assert_eq!(messenger.poll(), None);
        messenger.transport_mut().advance_clock(PROBE_INTERVAL);
    }
    assert!(
        messenger.transport_mut().recv().is_none(),
        "yeniden iletim yok"
    );
    assert_eq!(messenger.transport().stats().units_dropped, 1);
    assert_eq!(messenger.stats().resent_on_new_session, 0);
    assert_eq!(messenger.unacked_count(BOB), 1);
    // Tek yeniden gönderim yolu: eşin oturumu değişti (S1 K7) — kayıt ESKİ msg_id ile, bir kez.
    let new = messenger.transport_mut().inject_peer_supersession(old);
    assert_eq!(
        messenger.poll(),
        Some(session(SessionEvent::Ready(new, BOB)))
    );
    assert_eq!(messenger.stats().resent_on_new_session, 1);
    assert_eq!(
        messenger.poll(),
        Some(session(SessionEvent::Closed(old, CloseReason::Superseded)))
    );
    // Yankı eşi: kayıt geri döner, alan yarı teslim edip RECEIPT üretir, RECEIPT gönderen yarıyı
    // kapatır.
    assert_eq!(messenger.poll(), Some(received(BOB, msg_id, "kayıp")));
    assert_eq!(messenger.poll(), Some(delivered(BOB, msg_id)));
    assert_eq!(messenger.poll(), None);
    assert_eq!(messenger.stats().resent_on_new_session, 1);
}

// ── K4G-07: kalıcı `msg_id` ve outbox (aynı depoyla yeniden başlatma) ────────────────────────
//
// Taban `b7da69c4`'te (K4G-07 öncesi main) aşağıdaki testlerin hiçbiri DERLENMEZ:
// `Messenger::with_store`, `MessengerError::Store` ve `MessengerStats::store_errors` yoktur. Her
// testin belgesi, derlenecek biçime getirilse bile eski kodda neden KIRMIZI olacağını yazar.

/// DUR #105 (plan D4): Alice'in süreci ölür ve AYNI depoyla yeniden başlar; Bob çalışmayı sürdürür
/// ve dedup penceresi Alice'in eski `msg_id`'lerini tutar. Yeniden başlayan Alice'in ilk
/// `msg_id`'si öncekilerin hepsinden büyüktür; Bob TEXT'i teslim eder ve RECEIPT doğru kaydı
/// kapatır.
///
/// `b7da69c4`'te KIRMIZI: sayaç motorun belleğindeydi; yeni motor `FIRST_MSG_ID`'den (0) başlar →
/// `after == last + 1` düşer. Bob 0'ı pencerede görülmüş sayar, `TextReceived` üretmez (teslim
/// beklentisi düşer) ama RECEIPT(0) yollar ve Alice `Delivered(0)` görür — #105'in yanlış "teslim
/// edildi"si.
#[test]
fn restarted_sender_continues_msg_ids_and_the_receiver_delivers() {
    let store = fresh_store();
    let (mut duo, old) = Duo::connected_with(Arc::clone(&store));
    let mut sent = Vec::new();
    for text in ["bir", "iki", "üç"] {
        let msg_id = duo.alice.send_text(BOB, text).expect("gönderilir");
        assert_eq!(drain(&mut duo.bob), vec![received(ALICE, msg_id, text)]);
        assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, msg_id)]);
        sent.push(msg_id);
    }
    let last = *sent.last().expect("üç gönderim");

    duo.restart_alice(Arc::clone(&store));
    let new = duo.link.borrow_mut().reconnect_after_crash(Side::Alice);
    assert_eq!(
        drain(&mut duo.alice),
        vec![session(SessionEvent::Ready(new, BOB))]
    );
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );

    let after = duo
        .alice
        .send_text(BOB, "yeniden başladım")
        .expect("gönderilir");
    // Sayaç depodan sürdü: öncekilerin hepsinden büyük; çökme anında ayrılmış ama kullanılmamış
    // değer olmadığı için boşluk da yok.
    assert_eq!(after, last + 1);
    assert_eq!(
        drain(&mut duo.bob),
        vec![received(ALICE, after, "yeniden başladım")]
    );
    assert_eq!(duo.bob.stats().duplicate_texts, 0);
    assert_eq!(duo.bob.stats().below_window_texts, 0);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, after)]);
    assert_eq!(duo.alice.unacked_count(BOB), 0);
    assert!(outbox_ids(&*store, BOB).is_empty());
}

/// K4G-07 "outbox AppStore'a taşınır": Transport'un kabul ettiği ama yolda kaybolan TEXT, Alice
/// oturum değişimi görmeden ölse de kaybolmaz. Aynı depoyla yeniden başlayan motor eşin kalıcı
/// outbox'ını eş ilk göründüğünde (`Ready`) yükler ve kaydı yeni oturumda ESKİ `msg_id` ile (aynı
/// bayt) bir kez gönderir — bugünkü oturum-değişimi yolu. Bob teslim eder; RECEIPT kaydı bellekten
/// ve depodan kapatır.
///
/// `b7da69c4`'te KIRMIZI: outbox motorun belleğindeydi ve çökmeyle yok oldu; yeni oturumda
/// yeniden gönderim olmaz (`resent_on_new_session` 0), Bob "kayıp"ı hiç görmez, Alice `Delivered`
/// almaz.
#[test]
fn unreceipted_text_survives_a_sender_restart_and_is_receipted_on_the_new_session() {
    let store = fresh_store();
    let (mut duo, old) = Duo::connected_with(Arc::clone(&store));
    let first = duo.alice.send_text(BOB, "teslim").expect("gönderilir");
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, first, "teslim")]);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, first)]);
    // Transport kabul etti, yolda kayboldu: Bob görmez, RECEIPT gelmez.
    duo.link.borrow_mut().lose_next_units(1);
    let lost = duo.alice.send_text(BOB, "kayıp").expect("kabul edildi");
    assert!(drain(&mut duo.bob).is_empty());
    assert_eq!(
        outbox_ids(&*store, BOB),
        vec![lost],
        "teslim edilen kapandı, kaybolan kalıcı outbox'ta"
    );

    duo.restart_alice(Arc::clone(&store));
    let new = duo.link.borrow_mut().reconnect_after_crash(Side::Alice);
    assert_eq!(
        drain(&mut duo.alice),
        vec![session(SessionEvent::Ready(new, BOB))]
    );
    assert_eq!(duo.alice.stats().resent_on_new_session, 1);
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            received(ALICE, lost, "kayıp"),
        ]
    );
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, lost)]);
    assert_eq!(duo.alice.unacked_count(BOB), 0);
    assert!(
        outbox_ids(&*store, BOB).is_empty(),
        "RECEIPT kalıcı kopyayı da kapattı"
    );
    // Sayaç da sürdü: sıradaki kayıt kesintisiz değer alır.
    assert_eq!(duo.alice.send_text(BOB, "sonra"), Ok(lost + 1));
}

/// K4G-07: depo hatası sessiz yutulmaz ve panik üretmez. `msg_id` ayrılamazsa ya da kayıt kalıcı
/// outbox'a yazılamazsa kayıt Transport'a VERİLMEZ, outbox'a (bellek ve depo) girmez ve çağırana
/// tipli `MessengerError::Store` döner. Depodan ayrılmış ama tele çıkmamış değer yanmaz: depo
/// düzelince sıradaki gönderim kesintisiz değer alır.
///
/// `b7da69c4`'te KIRMIZI: Messenger depo almıyordu, `MessengerError::Store` yoktu; o kodda depo
/// hatası yolu hiç bulunmadığı için test eski koda uyarlanamaz — bu kartın sözleşmesini sabitler.
#[test]
fn store_failure_sends_nothing_and_burns_no_msg_id() {
    let flaky = Arc::new(FlakyStore::default());
    let store: Arc<dyn AppStore> = flaky.clone();
    let (mut duo, _) = Duo::connected_with(Arc::clone(&store));
    let first = duo.alice.send_text(BOB, "bir").expect("gönderilir");
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, first, "bir")]);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, first)]);
    for (operation, switch) in [
        ("reserve_next", &flaky.fail_reserve),
        ("outbox_put", &flaky.fail_outbox_put),
    ] {
        switch.store(true, Ordering::SeqCst);
        assert_eq!(
            duo.alice.send_text(BOB, "yazılamaz"),
            Err(MessengerError::Store(AppStoreError::Persist)),
            "{operation}"
        );
        switch.store(false, Ordering::SeqCst);
        assert!(
            duo.raw_inbox(Side::Bob).is_empty(),
            "{operation}: Transport'a hiçbir şey verilmedi"
        );
        assert_eq!(
            duo.alice.unacked_count(BOB),
            0,
            "{operation}: bellek outbox'ına girmedi"
        );
        assert!(
            outbox_ids(&*store, BOB).is_empty(),
            "{operation}: kalıcı outbox'a girmedi"
        );
    }
    assert_eq!(
        duo.link.borrow().alice.accepted,
        1,
        "yalnız ilk gönderim Transport'a verildi"
    );
    assert_eq!(
        duo.alice.stats().store_errors,
        0,
        "geri alma başarılı; çağırana dönen hata sayılmaz"
    );
    let next = duo.alice.send_text(BOB, "düzeldi").expect("gönderilir");
    assert_eq!(
        next,
        first + 1,
        "ne ayrılamayan ne yazılamayan gönderim değer yaktı"
    );
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, next, "düzeldi")]);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, next)]);
}

/// K4G-07: çağırana dönemeyen depo hataları sayılır ve hiçbiri yanlış "teslim edildi" üretmez.
/// (1) RECEIPT'in kalıcı kapanışı başarısız: teslim gerçektir (`Delivered`), kayıt depoda kalır.
/// (2) Yeniden başlatmada eşin outbox'ı yüklenemez: olayla gelen oturum benimsenmez, `connect`
/// hatayı tipli döndürür. Depo düzelince `connect` yüklemeyi yineler; kalan kayıt bir kez daha
/// gider, Bob'un dedup'ında söner (ikinci teslim YOK) ve RECEIPT'i onu kapatır.
///
/// `b7da69c4`'te KIRMIZI: depo, kalıcı outbox ve `MessengerStats::store_errors` yoktu; test eski
/// koda uyarlanamaz.
#[test]
fn background_store_errors_are_counted_and_never_fake_a_delivery() {
    let flaky = Arc::new(FlakyStore::default());
    let store: Arc<dyn AppStore> = flaky.clone();
    let (mut duo, old) = Duo::connected_with(Arc::clone(&store));
    let msg_id = duo.alice.send_text(BOB, "bir").expect("gönderilir");
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, msg_id, "bir")]);
    flaky.fail_outbox_remove.store(true, Ordering::SeqCst);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, msg_id)]);
    flaky.fail_outbox_remove.store(false, Ordering::SeqCst);
    assert_eq!(duo.alice.stats().store_errors, 1);
    assert_eq!(duo.alice.unacked_count(BOB), 0);
    assert_eq!(outbox_ids(&*store, BOB), vec![msg_id], "kalıcı kopya kaldı");

    duo.restart_alice(Arc::clone(&store));
    let new = duo.link.borrow_mut().reconnect_after_crash(Side::Alice);
    flaky.fail_outbox_list.store(true, Ordering::SeqCst);
    assert_eq!(
        drain(&mut duo.alice),
        vec![session(SessionEvent::Ready(new, BOB))]
    );
    assert_eq!(
        duo.alice.stats().store_errors,
        1,
        "olay yolunda yükleme hatası"
    );
    assert_eq!(duo.alice.session_of(BOB), None, "tutamaç benimsenmedi");
    assert_eq!(
        duo.alice.send_text(BOB, "olmaz"),
        Err(MessengerError::NoSession)
    );
    assert_eq!(
        duo.alice.connect(BOB),
        Err(MessengerError::Store(AppStoreError::Persist))
    );
    flaky.fail_outbox_list.store(false, Ordering::SeqCst);

    assert_eq!(duo.alice.connect(BOB), Ok(ConnectResult::Ready(new)));
    assert_eq!(duo.alice.stats().resent_on_new_session, 1);
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(
        duo.bob.stats().duplicate_texts,
        1,
        "Bob kaydı ikinci kez teslim etmedi"
    );
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, msg_id)]);
    assert!(outbox_ids(&*store, BOB).is_empty());
}

/// D4 BEYANLI SINIRI — bu test hatayı ONAYLAMAZ; sınırın hâlâ var olduğunu sabitler ("çözüldü"
/// denmez; PROVISIONING-FORMAT-v0 §1, PoC-KAPSAM-001 §5 EK-1). Kalıcı durum tamamen kaybolur
/// (Alice boş bir depoyla yeniden başlar) ve AYNI kimlik yeniden kullanılırsa `msg_id` baştan
/// başlar: Bob TEXT'i pencerede görülmüş sayar, teslim etmez ama RECEIPT yollar ve Alice yanlış
/// `Delivered` görür. Kural: kalıcı durum kaybında yeniden provisioning (yeni kimlik → Bob'da yeni
/// eş). Sınır kapatılırsa (ör. sunucu tarafı kimlik epoch'u) bu test bilinçli olarak değişir.
///
/// `b7da69c4`'te yalnız derlenmez (`Messenger::with_store` yok); davranış orada da aynıdır — o
/// kodda HER yeniden başlatma bu durumdu (DUR #105).
#[test]
fn state_loss_under_the_same_identity_is_the_declared_d4_limit() {
    let (mut duo, old) = Duo::connected();
    let first = duo.alice.send_text(BOB, "bir").expect("gönderilir");
    assert_eq!(drain(&mut duo.bob), vec![received(ALICE, first, "bir")]);
    assert_eq!(drain(&mut duo.alice), vec![delivered(BOB, first)]);

    duo.restart_alice(fresh_store());
    let new = duo.link.borrow_mut().reconnect_after_crash(Side::Alice);
    assert_eq!(
        drain(&mut duo.alice),
        vec![session(SessionEvent::Ready(new, BOB))]
    );
    assert_eq!(
        drain(&mut duo.bob),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    let reused = duo.alice.send_text(BOB, "yeni metin").expect("gönderilir");
    assert_eq!(reused, first, "boş depo: sayaç baştan");
    assert!(drain(&mut duo.bob).is_empty(), "Bob teslim ETMEDİ");
    assert_eq!(duo.bob.stats().duplicate_texts, 1);
    assert_eq!(
        drain(&mut duo.alice),
        vec![delivered(BOB, reused)],
        "yanlış \"teslim edildi\" (beyanlı sınır)"
    );
}
