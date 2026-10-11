//! B2.4 — PTT yaşam döngüsü (`kama_app::ptt`) uçtan uca.
//!
//! Beklenen davranışın kaynağı: AMP §3 (`PTT_OPEN` / `PTT_CLOSE` / `MEDIA_BUNDLE`; iş emrindeki
//! `PTT_START` / `PTT_END` bunlardır), sahip kararları S7 (`PTT_OPEN` öncesi `MEDIA_BUNDLE`
//! düşer), S8 (vektörsüz alıcı kararı fail-closed DROP: olay yok, panik yok), S9 (PTT sesi
//! `MEDIA_BUNDLE` BestEffort; codec / `bundle_ms` farkı DROP), S1 K7 ve §4 (supersession,
//! sıralama).
//!
//! Düzenek `tests/text_receipt.rs`'teki iki uçlu çiftin (`Link`) kırpılmış KOPYASIDIR: bir uçtan
//! gönderilen birim karşı ucun teslim kuyruğuna düşer; oturum kurma, supersession, oturum sonu ve
//! ham kayıt enjeksiyonu testin elindedir. Ek olarak her uç, Transport'un KABUL ettiği
//! gönderimlerin tipini, `DeliveryPolicy`'sini ve `CompressionPolicy`'sini kaydeder (S9
//! denetimi). Ortak sahte (`tests/common/mock_transport.rs`) politika kaydetmez ve S1 sözleşme
//! süitiyle paylaşıldığı için değiştirilmedi.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::iter;
use std::rc::Rc;
use std::sync::Arc;

use kama_app::codec::limits::{APP_RECORD_HEADER_BYTES, MEDIA_BUNDLE_PREFIX_BYTES};
use kama_app::codec::{decode, encode, AmpLimits, AppMessage, MsgType, PttCodec};
use kama_app::messaging::{Messenger, MessengerEvent};
use kama_app::ptt::{
    max_bundle_payload_bytes, Ptt, PttError, PttEvent, PttStats, StreamId, FIRST_STREAM_ID,
};
use kama_app::store::MemoryAppStore;
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport,
    PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};

/// Test parametresi (normatif değil): codec vektörlerinin üretildiği değerle aynı.
const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Çiftin birim tavanı (`max_plaintext_unit_bytes`, `usize`).
const LINK_MAX_UNIT_BYTES: usize = TEST_MAX_PLAINTEXT_UNIT_BYTES as usize;
/// Çiftin yön başına teslim kuyruğu tavanı (sahte sınır; aşımı `Backpressure`).
const LINK_QUEUE_CAP: usize = 64;
/// Uç başına olay kuyruğu tavanı (sahte sınır).
const LINK_EVENT_CAP: usize = 16;
/// Uç başına gönderim kaydı tavanı (sahte sınır).
const LINK_LOG_CAP: usize = 256;
/// `drain` döngüsünün tavanı.
const DRAIN_CAP: usize = 1024;
/// Testlerin demet süresi (ms; normatif değil — `bundle_ms` media ekibinindir).
const TEST_BUNDLE_MS: u8 = 20;
/// Uyuşmazlık vakalarındaki ikinci demet süresi (ms).
const OTHER_BUNDLE_MS: u8 = 40;
/// Bir test basışındaki demet sayısı.
const TEST_SPURT_BUNDLES: u8 = 3;
/// Test ses çerçevesinin boyu (bayt; içerik opak).
const TEST_FRAME_BYTES: usize = 3;

const ALICE: PeerRef = PeerRef::from_dest_hash([0xA1; 32]);
const BOB: PeerRef = PeerRef::from_dest_hash([0xB0; 32]);

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

fn open_record(stream_id: StreamId, codec: PttCodec, bundle_ms: u8) -> Vec<u8> {
    record(&AppMessage::PttOpen {
        stream_id,
        codec,
        bundle_ms,
    })
}

fn bundle_record(stream_id: StreamId, codec: u8, bundle_ms: u8, payload: &[u8]) -> Vec<u8> {
    record(&AppMessage::MediaBundle {
        stream_id,
        codec,
        bundle_ms,
        payload: payload.to_vec(),
    })
}

fn close_record(stream_id: StreamId) -> Vec<u8> {
    record(&AppMessage::PttClose { stream_id })
}

/// `n` numaralı test ses çerçevesi (opak baytlar).
fn frame(n: u8) -> Vec<u8> {
    vec![n; TEST_FRAME_BYTES]
}

fn started(peer: PeerRef, stream_id: StreamId, codec: PttCodec, bundle_ms: u8) -> PttEvent {
    PttEvent::PttStarted {
        peer,
        stream_id,
        codec,
        bundle_ms,
    }
}

fn audio(peer: PeerRef, stream_id: StreamId, payload: &[u8]) -> PttEvent {
    PttEvent::PttAudio {
        peer,
        stream_id,
        payload: payload.to_vec(),
    }
}

fn ended(peer: PeerRef, stream_id: StreamId) -> PttEvent {
    PttEvent::PttEnded { peer, stream_id }
}

fn session(event: SessionEvent) -> PttEvent {
    PttEvent::Messenger(MessengerEvent::Session(event))
}

// ── iki uçlu çift (`tests/text_receipt.rs`'ten kırpılmış kopya + gönderim kaydı) ─────────────

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

/// Transport'un kabul ettiği bir gönderim: kaydın tipi ve beyan edilen iki politika.
type Sent = (MsgType, DeliveryPolicy, CompressionPolicy);

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
    sent: Vec<Sent>,
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

    /// `side` ucunun oturumu `reason` ile biter: yalnız o uca `Closed(h, reason)`. Karşı uç
    /// kapanıştan habersizdir; oradan gelmeye devam eden birimler kapanmış tutamaçla teslim edilir.
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
        if remote.inbox.len() >= LINK_QUEUE_CAP {
            return Err(TransportError::Backpressure);
        }
        assert!(
            me.sent.len() < LINK_LOG_CAP,
            "senaryo gönderim kaydı tavanını aştı"
        );
        let message = decode(&unit.bytes, &limits()).expect("motor geçerli kayıt üretir");
        me.sent
            .push((message.msg_type(), policy, unit.compression_policy));
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

/// Bir uç: Messenger + PTT motoru. Olaylar yalnız PTT motorundan okunur (Messenger'ın tek
/// okuyucusu).
struct Node {
    messenger: Messenger<End>,
    ptt: Ptt,
}

impl Node {
    fn new(link: &Rc<RefCell<Link>>, side: Side) -> Self {
        let end = End {
            link: Rc::clone(link),
            side,
        };
        Self {
            messenger: Messenger::with_store(end, Arc::new(MemoryAppStore::new()))
                .expect("limitler geçerli"),
            ptt: Ptt::new(),
        }
    }

    fn open(
        &mut self,
        peer: PeerRef,
        codec: PttCodec,
        bundle_ms: u8,
    ) -> Result<StreamId, PttError> {
        self.ptt.open(&mut self.messenger, peer, codec, bundle_ms)
    }

    fn send_bundle(&mut self, stream_id: StreamId, payload: &[u8]) -> Result<(), PttError> {
        self.ptt
            .send_bundle(&mut self.messenger, stream_id, payload)
    }

    fn close(&mut self, stream_id: StreamId) -> Result<(), PttError> {
        self.ptt.close(&mut self.messenger, stream_id)
    }

    /// `poll` `None` verene kadar olayları toplar.
    fn drain(&mut self) -> Vec<PttEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.ptt.poll(&mut self.messenger) {
            assert!(events.len() < DRAIN_CAP, "olay akışı bitmiyor");
            events.push(event);
        }
        events
    }
}

/// İki uçlu düzenek: Alice'in ve Bob'un uçları + testin `Link` denetimi.
struct Duo {
    link: Rc<RefCell<Link>>,
    alice: Node,
    bob: Node,
}

impl Duo {
    fn unconnected() -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let alice = Node::new(&link, Side::Alice);
        let bob = Node::new(&link, Side::Bob);
        Self { link, alice, bob }
    }

    /// Oturum kurulmuş, iki tarafın `Ready` olayı okunmuş düzenek.
    fn connected() -> (Self, SessionHandle) {
        let mut duo = Self::unconnected();
        let handle = duo.link.borrow_mut().establish();
        assert_eq!(
            duo.alice.drain(),
            vec![session(SessionEvent::Ready(handle, BOB))]
        );
        assert_eq!(
            duo.bob.drain(),
            vec![session(SessionEvent::Ready(handle, ALICE))]
        );
        (duo, handle)
    }

    fn inject(&self, to: Side, unit: Vec<u8>) {
        self.link.borrow_mut().inject(to, unit);
    }

    /// `side` ucundan Transport'un kabul ettiği gönderimler, sırasıyla.
    fn sent(&self, side: Side) -> Vec<Sent> {
        self.link.borrow_mut().endpoint(side).sent.clone()
    }
}

// ── testler ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn open_bundles_close_round_trip_in_order() {
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(stream, FIRST_STREAM_ID);
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), Some(stream));
    for n in 0..TEST_SPURT_BUNDLES {
        duo.alice
            .send_bundle(stream, &frame(n))
            .expect("gönderilir");
    }
    duo.alice.close(stream).expect("kapanır");
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), None);
    let expected: Vec<PttEvent> =
        iter::once(started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS))
            .chain((0..TEST_SPURT_BUNDLES).map(|n| audio(ALICE, stream, &frame(n))))
            .chain(iter::once(ended(ALICE, stream)))
            .collect();
    assert_eq!(duo.bob.drain(), expected);
    assert_eq!(duo.bob.ptt.incoming_stream(ALICE), None);
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats::default(),
        "hiçbir kayıt düşmedi"
    );
    // PTT kayıtları alındı üretmez: gönderene hiçbir şey dönmez.
    assert!(duo.alice.drain().is_empty());
    // Sıradaki basış yeni `stream_id` alır (gönderen başına monoton) ve kendi parametreleriyle
    // açılır.
    let next = duo
        .alice
        .open(BOB, PttCodec::Lyra, OTHER_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(next, stream + 1);
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, next, PttCodec::Lyra, OTHER_BUNDLE_MS)]
    );
}

#[test]
fn lifecycle_records_are_reliable_and_bundles_best_effort() {
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    for n in 0..TEST_SPURT_BUNDLES {
        duo.alice
            .send_bundle(stream, &frame(n))
            .expect("gönderilir");
    }
    duo.alice.close(stream).expect("kapanır");
    // S9: ses `BestEffort`; yaşam döngüsü kaydı `Reliable`. Hiçbir PTT kaydı sıkıştırılmaz.
    let lifecycle = |msg_type| (msg_type, DeliveryPolicy::Reliable, CompressionPolicy::Never);
    let bundle = (
        MsgType::MediaBundle,
        DeliveryPolicy::BestEffort,
        CompressionPolicy::Never,
    );
    let expected: Vec<Sent> = iter::once(lifecycle(MsgType::PttOpen))
        .chain(iter::repeat_n(bundle, usize::from(TEST_SPURT_BUNDLES)))
        .chain(iter::once(lifecycle(MsgType::PttClose)))
        .collect();
    assert_eq!(duo.sent(Side::Alice), expected);
}

#[test]
fn bundle_with_another_codec_is_dropped() {
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    // `MEDIA_BUNDLE.codec` opak `u8`'dir: registry'deki başka kodlayıcı da, registry dışı kod da
    // farktır.
    for codec in [PttCodec::Lyra.code(), u8::MAX] {
        duo.inject(
            Side::Bob,
            bundle_record(stream, codec, TEST_BUNDLE_MS, &frame(1)),
        );
    }
    assert!(duo.bob.drain().is_empty(), "S9: codec farkı DROP");
    assert_eq!(duo.bob.ptt.stats().bundles_mismatched, 2);
    // Akış sürer: akışın parametreleriyle gelen demet iletilir.
    duo.alice
        .send_bundle(stream, &frame(2))
        .expect("gönderilir");
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, stream, &frame(2))]);
}

#[test]
fn bundle_with_another_bundle_ms_is_dropped() {
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    for bundle_ms in [OTHER_BUNDLE_MS, u8::MAX] {
        duo.inject(
            Side::Bob,
            bundle_record(stream, PttCodec::Opus.code(), bundle_ms, &frame(1)),
        );
    }
    assert!(duo.bob.drain().is_empty(), "S9: bundle_ms farkı DROP");
    assert_eq!(duo.bob.ptt.stats().bundles_mismatched, 2);
    duo.alice
        .send_bundle(stream, &frame(2))
        .expect("gönderilir");
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, stream, &frame(2))]);
}

#[test]
fn bundle_before_open_is_dropped() {
    let (mut duo, _) = Duo::connected();
    // `BestEffort` demet `Reliable` `PTT_OPEN`'ı geçebilir (S1 §4): akışı açılmamış demet düşer
    // (S7).
    duo.inject(
        Side::Bob,
        bundle_record(
            FIRST_STREAM_ID,
            PttCodec::Opus.code(),
            TEST_BUNDLE_MS,
            &frame(0),
        ),
    );
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.stats().bundles_without_stream, 1);
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(stream, FIRST_STREAM_ID);
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    // Erken demet sonradan teslim edilmez.
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
            audio(ALICE, stream, &frame(1)),
        ]
    );
}

#[test]
fn bundle_after_close_or_from_an_old_stream_is_dropped() {
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    duo.alice.close(stream).expect("kapanır");
    // `BestEffort` demet `PTT_CLOSE`'tan sonra varabilir (S1 §4): akış bitmiştir, demet düşer.
    duo.inject(
        Side::Bob,
        bundle_record(stream, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(1)),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
            ended(ALICE, stream),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().bundles_without_stream, 1);
    // Gönderende kapanmış akış bilinmez; hiçbir şey gönderilmez.
    assert_eq!(
        duo.alice.send_bundle(stream, &frame(2)),
        Err(PttError::UnknownStream)
    );
    assert_eq!(duo.alice.close(stream), Err(PttError::UnknownStream));
    assert_eq!(
        duo.sent(Side::Alice).len(),
        2,
        "yalnız PTT_OPEN ve PTT_CLOSE"
    );
    // Yeni akış etkinken eski akışın geç demeti bilinmeyen akıştır.
    let next = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    duo.inject(
        Side::Bob,
        bundle_record(stream, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(3)),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, next, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    assert_eq!(duo.bob.ptt.stats().bundles_unknown_stream, 1);
}

#[test]
fn close_for_an_unknown_stream_is_dropped() {
    let (mut duo, _) = Duo::connected();
    duo.inject(Side::Bob, close_record(FIRST_STREAM_ID));
    assert!(duo.bob.drain().is_empty());
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    // Etkin akış varken başka `stream_id`'nin `PTT_CLOSE`'u akışı bitirmez.
    duo.inject(Side::Bob, close_record(stream + 1));
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
            audio(ALICE, stream, &frame(1)),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().unknown_closes, 2);
    assert_eq!(duo.bob.ptt.incoming_stream(ALICE), Some(stream));
}

#[test]
fn second_open_ends_the_active_stream_and_starts_the_new_one() {
    let (mut duo, _) = Duo::connected();
    let first = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    // Gönderen: eş başına tek giden akış.
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Lyra, OTHER_BUNDLE_MS),
        Err(PttError::StreamActive { stream_id: first })
    );
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, first, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    // Alıcı: `PTT_CLOSE`'u gelmemiş akış varken başka `stream_id`'li `PTT_OPEN` (kapanışı
    // kaybolmuş ya da yeniden başlamış gönderici) — en yeni kazanır: önce eskinin sonu.
    let second = first + 1;
    duo.inject(
        Side::Bob,
        open_record(second, PttCodec::Lyra, OTHER_BUNDLE_MS),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            ended(ALICE, first),
            started(ALICE, second, PttCodec::Lyra, OTHER_BUNDLE_MS),
        ]
    );
    // Eski akışın demeti artık bilinmeyen akıştır; yenisininki iletilir.
    duo.inject(
        Side::Bob,
        bundle_record(first, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(1)),
    );
    duo.inject(
        Side::Bob,
        bundle_record(second, PttCodec::Lyra.code(), OTHER_BUNDLE_MS, &frame(2)),
    );
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, second, &frame(2))]);
    // Etkin akışın `stream_id`'siyle yeniden `PTT_OPEN` düşer — parametreleri farklı olsa da
    // (akışın codec / bundle_ms'i ilk `PTT_OPEN`'la sabit, S9).
    duo.inject(
        Side::Bob,
        open_record(second, PttCodec::Lyra, OTHER_BUNDLE_MS),
    );
    duo.inject(
        Side::Bob,
        open_record(second, PttCodec::Opus, TEST_BUNDLE_MS),
    );
    assert!(duo.bob.drain().is_empty());
    let stats = duo.bob.ptt.stats();
    assert_eq!(stats.streams_replaced, 1);
    assert_eq!(stats.bundles_unknown_stream, 1);
    assert_eq!(stats.duplicate_opens, 2);
    assert_eq!(duo.bob.ptt.incoming_stream(ALICE), Some(second));
}

#[test]
fn oversize_payload_is_a_typed_error_at_the_sender() {
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    let max = max_bundle_payload_bytes(duo.alice.messenger.limits());
    // Bağımsız türetim: birim − kayıt başlığı − `MEDIA_BUNDLE` öneki.
    assert_eq!(
        max,
        LINK_MAX_UNIT_BYTES - APP_RECORD_HEADER_BYTES - MEDIA_BUNDLE_PREFIX_BYTES
    );
    let over = vec![0u8; max + 1];
    assert_eq!(
        duo.alice.send_bundle(stream, &over),
        Err(PttError::PayloadTooLarge {
            payload_len: max + 1,
            max_payload_len: max,
        })
    );
    // Reddedilen demet Transport'a hiç verilmedi: yalnız `PTT_OPEN` kabul edildi.
    assert_eq!(duo.sent(Side::Alice).len(), 1);
    // Tam sınır: kayıt birime tam sığar ve karşıya ulaşır.
    let exact = vec![0u8; max];
    duo.alice.send_bundle(stream, &exact).expect("tam sığar");
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
            audio(ALICE, stream, &exact),
        ]
    );
}

#[test]
fn stream_survives_supersession() {
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    // Eski tutamaçla yolda kalan demet.
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    let new = duo.link.borrow_mut().supersede();
    // Alice `Ready(h')`'yi okumadan: eski tutamaç supersede edilmiştir (S1 F0.2(i)); ret aynen.
    assert_eq!(
        duo.alice.send_bundle(stream, &frame(2)),
        Err(PttError::Transport(TransportError::SessionSuperseded))
    );
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    // Akış eşindir: aynı `stream_id` yeni tutamaçla sürer.
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), Some(stream));
    duo.alice
        .send_bundle(stream, &frame(3))
        .expect("gönderilir");
    duo.alice.close(stream).expect("kapanır");
    // Bob: oturum olayları önce; supersession akışı bitirmez; eski tutamaçtan gelen demet de akışa
    // bağlanır.
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            audio(ALICE, stream, &frame(1)),
            audio(ALICE, stream, &frame(3)),
            ended(ALICE, stream),
        ]
    );
    assert_eq!(duo.bob.ptt.stats(), PttStats::default());
}

#[test]
fn session_end_ends_the_incoming_stream() {
    for reason in [
        CloseReason::UserClosed,
        CloseReason::PeerClosed,
        CloseReason::KeyHygiene,
    ] {
        let (mut duo, handle) = Duo::connected();
        let stream = duo
            .alice
            .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
            .expect("açılır");
        assert_eq!(
            duo.bob.drain(),
            vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
        );
        duo.link.borrow_mut().end(Side::Bob, reason);
        // Eşin son oturumu kapandı: `Closed` aynen geçer, hemen ardından akış biter.
        assert_eq!(
            duo.bob.drain(),
            vec![
                session(SessionEvent::Closed(handle, reason)),
                ended(ALICE, stream),
            ],
            "{reason:?}"
        );
        assert_eq!(duo.bob.ptt.stats().streams_ended_by_session, 1);
        // Kapanıştan habersiz Alice'ten geç gelen kayıtlar: demetler ve `PTT_CLOSE` akışsız, yeni
        // `PTT_OPEN` güncel oturumu olmayan eşten — hepsi düşer.
        duo.alice
            .send_bundle(stream, &frame(1))
            .expect("Alice'in oturumu açık");
        duo.alice.close(stream).expect("Alice'in oturumu açık");
        let next = duo
            .alice
            .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
            .expect("açılır");
        duo.alice.send_bundle(next, &frame(2)).expect("gönderilir");
        assert!(duo.bob.drain().is_empty(), "{reason:?}");
        let stats = duo.bob.ptt.stats();
        assert_eq!(stats.bundles_without_stream, 2);
        assert_eq!(stats.unknown_closes, 1);
        assert_eq!(stats.opens_without_session, 1);
        assert_eq!(duo.bob.ptt.incoming_stream(ALICE), None);
    }
}

#[test]
fn sender_errors_are_typed_and_a_refused_open_changes_nothing() {
    let mut duo = Duo::unconnected();
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Opus, TEST_BUNDLE_MS),
        Err(PttError::NoSession)
    );
    assert_eq!(
        duo.alice.send_bundle(FIRST_STREAM_ID, &frame(0)),
        Err(PttError::UnknownStream)
    );
    assert_eq!(
        duo.alice.close(FIRST_STREAM_ID),
        Err(PttError::UnknownStream)
    );
    let handle = duo.link.borrow_mut().establish();
    assert_eq!(
        duo.alice.drain(),
        vec![session(SessionEvent::Ready(handle, BOB))]
    );
    assert_eq!(
        duo.bob.drain(),
        vec![session(SessionEvent::Ready(handle, ALICE))]
    );
    // Bob'un kuyruğu dolu: Transport'un `Backpressure`'ı aynen; akış açılmaz, `stream_id` yanmaz.
    for _ in 0..LINK_QUEUE_CAP {
        duo.inject(Side::Bob, Vec::new());
    }
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Opus, TEST_BUNDLE_MS),
        Err(PttError::Transport(TransportError::Backpressure))
    );
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), None);
    // Boş kayıtlar codec'te düşer (olay yok); kuyruk boşalır.
    assert!(duo.bob.drain().is_empty());
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("yer açıldı");
    assert_eq!(
        stream, FIRST_STREAM_ID,
        "reddedilen PTT_OPEN stream_id tüketmedi"
    );
    // Eş başına tek giden akış.
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Lyra, OTHER_BUNDLE_MS),
        Err(PttError::StreamActive { stream_id: stream })
    );
    assert_eq!(
        duo.sent(Side::Alice),
        vec![(
            MsgType::PttOpen,
            DeliveryPolicy::Reliable,
            CompressionPolicy::Never
        )]
    );
}

#[test]
fn close_always_ends_the_outgoing_stream_and_is_never_retried() {
    let (mut duo, handle) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    // Bob'un kuyruğu `PTT_OPEN`'ın arkasından dolar.
    for _ in 1..LINK_QUEUE_CAP {
        duo.inject(Side::Bob, Vec::new());
    }
    // `BestEffort` demet de Transport'un geri basıncına tabidir; ret aynen döner.
    assert_eq!(
        duo.alice.send_bundle(stream, &frame(0)),
        Err(PttError::Transport(TransportError::Backpressure))
    );
    // `PTT_CLOSE` Transport'a verilemedi ama basış bitti: akış kapanır.
    assert_eq!(
        duo.alice.close(stream),
        Err(PttError::Transport(TransportError::Backpressure))
    );
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), None);
    assert_eq!(
        duo.alice.send_bundle(stream, &frame(1)),
        Err(PttError::UnknownStream)
    );
    // Bob: akış başladı, `PTT_CLOSE` hiç gelmedi.
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    // Motor `PTT_CLOSE`'u kendiliğinden yeniden göndermez; Alice'in sıradaki basışı eski akışı
    // Bob'da örtük bitirir (en yeni `PTT_OPEN` kazanır).
    let next = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(next, stream + 1);
    assert_eq!(
        duo.bob.drain(),
        vec![
            ended(ALICE, stream),
            started(ALICE, next, PttCodec::Lyra, TEST_BUNDLE_MS),
        ]
    );
    let open_sent: Sent = (
        MsgType::PttOpen,
        DeliveryPolicy::Reliable,
        CompressionPolicy::Never,
    );
    assert_eq!(duo.sent(Side::Alice), vec![open_sent; 2]);
    // Oturumu kalmayan eşe kapanış: `NoSession`, akış yine de kapanır. Giden akış oturum sonunda
    // kendiliğinden kapanmamıştı (çağıranındır).
    duo.link
        .borrow_mut()
        .end(Side::Alice, CloseReason::PeerClosed);
    assert_eq!(
        duo.alice.drain(),
        vec![session(SessionEvent::Closed(
            handle,
            CloseReason::PeerClosed
        ))]
    );
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), Some(next));
    assert_eq!(
        duo.alice.send_bundle(next, &frame(2)),
        Err(PttError::NoSession)
    );
    assert_eq!(duo.alice.close(next), Err(PttError::NoSession));
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), None);
}

#[test]
fn non_ptt_events_pass_through_unchanged() {
    let (mut duo, _) = Duo::connected();
    let msg_id = duo
        .alice
        .messenger
        .send_text(BOB, "metin")
        .expect("gönderilir");
    let file_end = AppMessage::FileEnd { transfer_id: 1 };
    duo.inject(Side::Bob, record(&file_end));
    assert_eq!(
        duo.bob.drain(),
        vec![
            PttEvent::Messenger(MessengerEvent::TextReceived {
                peer: ALICE,
                msg_id,
                text: "metin".to_owned(),
            }),
            PttEvent::Messenger(MessengerEvent::Other {
                peer: ALICE,
                message: file_end,
            }),
        ]
    );
    assert_eq!(
        duo.alice.drain(),
        vec![PttEvent::Messenger(MessengerEvent::Delivered {
            peer: BOB,
            msg_id
        })]
    );
    assert_eq!(duo.bob.ptt.stats(), PttStats::default());
}
