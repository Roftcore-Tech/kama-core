//! B2.4 / K4G-08 — PTT yaşam döngüsü (`kama_app::ptt`) uçtan uca.
//!
//! Beklenen davranışın kaynağı: AMP §3 (`PTT_OPEN` / `PTT_CLOSE` / `MEDIA_BUNDLE`; K4G-08 yaması
//! 2026-10-12: PTT-R0…PTT-R5), plan D5 (aktif akışın `PTT_OPEN`'ı her yeni oturumda bir kez aynı
//! `stream_id` ile; `stream_id` kalıcı) ve D7 ek (PTT kayıtlarının üçü de `Reliable`), sahip
//! kararları S7 (`PTT_OPEN` öncesi `MEDIA_BUNDLE` düşer), S8 (vektörsüz alıcı kararı fail-closed
//! DROP: olay yok, panik yok), S9 (codec / `bundle_ms` farkı DROP; S9'un "`MEDIA_BUNDLE`
//! `BestEffort`" kararı K4G-08 ile kalktı), S1 K2/K7 ve §4 (idempotent `connect`, supersession,
//! sıralama). İş emrindeki `PTT_START` / `PTT_END` AMP'nin `PTT_OPEN` / `PTT_CLOSE`'udur.
//!
//! Düzenek `tests/text_receipt.rs`'teki iki uçlu çiftin (`Link`) kırpılmış KOPYASIDIR: bir uçtan
//! gönderilen birim karşı ucun teslim kuyruğuna düşer; oturum kurma, supersession, oturum sonu,
//! çökme + yeniden bağlanma, yoldaki birimi geciktirme ya da kaybettirme ve ham kayıt enjeksiyonu
//! testin elindedir. Ek olarak her uç, Transport'un KABUL ettiği gönderimlerin tutamacını, tipini,
//! `DeliveryPolicy`'sini ve `CompressionPolicy`'sini kaydeder (PTT-R0 ve "oturum başına bir yeniden
//! `PTT_OPEN`" denetimi). Zaman, iki ucun paylaştığı elle ilerletilen bir saattir (PTT-R4; gerçek
//! uyku yok). Eşler arası `stream_id` benzersizliği ortak yansıtıcı sahteyle
//! (`tests/common/mock_transport.rs`) sınanır; o sahte politika kaydetmez ve S1 sözleşme süitiyle
//! paylaşıldığı için değiştirilmedi.
//!
//! # Testlerin `0904f723`'teki (K4G-08 öncesi main) durumu — üç sınıf
//!
//! Düzenek `Ptt::new(saat)` ve depo-bağlı `stream_id` kullandığı için dosya eski kodda DERLENMEZ.
//! K4G-08'in eklediği ya da anlamını değiştirdiği her testin belgesi, test MANTIĞININ eski koda göre
//! sınıfını yazar:
//!
//! * **KIRMIZI (uyarlansa da)** — eski API'ye uyarlanınca da bir doğrulama düşer;
//! * **UYARLANAMAZ** — sınadığı yüzey eski kodda yok (ör. `PttError::Store`, `ptt_orphan_bundles`,
//!   `PTT_STREAM_IDLE_TTL`);
//! * **AYNI DAVRANIŞ** — eski kodda da geçerdi; regresyon kilidi.
//!
//! İnceleme turlarında eklenen testler, sınıflarının ardından bu PR'ın inceleme öncesi
//! başlarındaki (`b1cce203`, `dad269e7`) durumu da yazar.

mod common;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::iter;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::mock_transport::MockTransport;
use kama_app::clock::MonotonicClock;
use kama_app::codec::limits::{APP_RECORD_HEADER_BYTES, MEDIA_BUNDLE_PREFIX_BYTES};
use kama_app::codec::{
    bundle_ms_from_code, decode, encode, AmpLimits, AppMessage, MsgType, PttCodec,
};
use kama_app::messaging::{Messenger, MessengerEvent, MsgId};
use kama_app::ptt::{
    max_bundle_payload_bytes, Ptt, PttError, PttEvent, PttStats, SpurtCapabilityRecord, StreamId,
    FIRST_STREAM_ID, PTT_MAX_ACTIVE_STREAMS, PTT_STREAM_IDLE_TTL, SPURT_FORMAT_EPOCH,
    SPURT_PILOT_BUNDLE_CODE, SPURT_PILOT_BUNDLE_MS,
};
use kama_app::store::{
    AppStore, AppStoreError, CounterKind, DedupWindow, MemoryAppStore, OutboxUnit,
};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy,
    KamaTransport, PreparedUnit, SessionEvent, TrafficClass, TransportCapabilities, TransportError,
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
/// "Yeniden yoklansa da" tur sayısı.
const IDLE_ROUNDS: usize = 8;
/// Testlerin demet süresi (ms; normatif değil — `bundle_ms` media ekibinindir).
const TEST_BUNDLE_MS: u8 = 20;
/// Uyuşmazlık vakalarındaki ikinci demet süresi (ms).
const OTHER_BUNDLE_MS: u8 = 40;
/// Bir test basışındaki demet sayısı.
const TEST_SPURT_BUNDLES: u8 = 3;
/// Test ses çerçevesinin boyu (bayt; içerik opak).
const TEST_FRAME_BYTES: usize = 3;
/// PTT-R4 sınırının bir adım altı.
const TTL_MARGIN: Duration = Duration::from_secs(1);

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

/// Taze bellek içi depo — kalıcı durumu olmayan bir cihaz.
fn fresh_store() -> Arc<dyn AppStore> {
    Arc::new(MemoryAppStore::new())
}

/// `ptt.poll` `None` verene kadar olayları toplar.
fn drain_events<T: KamaTransport>(ptt: &mut Ptt, messenger: &mut Messenger<T>) -> Vec<PttEvent> {
    let mut events = Vec::new();
    while let Some(event) = ptt.poll(messenger) {
        assert!(events.len() < DRAIN_CAP, "olay akışı bitmiyor");
        events.push(event);
    }
    events
}

// ── saat ve depo ─────────────────────────────────────────────────────────────────────────────

/// Elle ilerletilen saat: kopyalar AYNI zamanı paylaşır (test bir kopyayı tutar, motorlar
/// diğerlerini); zaman yalnız ileri gider (u64 nanosaniyede doyar).
#[derive(Clone, Default)]
struct ManualClock {
    nanos: Arc<AtomicU64>,
}

impl ManualClock {
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

/// Arıza enjekte edilebilen depo: anahtarlar kapalıyken her işlem bellek içi depoya gider.
/// `fail_reserve` açıkken `reserve_next` `Persist`, `exhaust_reserve` açıkken `Exhausted` döner ve
/// hiçbir şey ayırmaz.
#[derive(Default)]
struct FlakyStore {
    inner: MemoryAppStore,
    fail_reserve: AtomicBool,
    exhaust_reserve: AtomicBool,
}

impl AppStore for FlakyStore {
    fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
        if self.fail_reserve.load(Ordering::SeqCst) {
            return Err(AppStoreError::Persist);
        }
        if self.exhaust_reserve.load(Ordering::SeqCst) {
            return Err(AppStoreError::Exhausted);
        }
        self.inner.reserve_next(peer, kind)
    }

    fn dedup_load(&self, peer: PeerRef) -> Result<DedupWindow, AppStoreError> {
        self.inner.dedup_load(peer)
    }

    fn dedup_save(&self, peer: PeerRef, window: &DedupWindow) -> Result<(), AppStoreError> {
        self.inner.dedup_save(peer, window)
    }

    fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError> {
        self.inner.outbox_put(peer, unit)
    }

    fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError> {
        self.inner.outbox_remove(peer, msg_id)
    }

    fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
        self.inner.outbox_list(peer)
    }
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

    /// Karşı uç.
    const fn other(self) -> Self {
        match self {
            Self::Alice => Self::Bob,
            Self::Bob => Self::Alice,
        }
    }
}

/// Transport'un kabul ettiği bir gönderim: tutamaç, kaydın tipi ve beyan edilen iki politika.
type Accepted = (SessionHandle, MsgType, DeliveryPolicy, CompressionPolicy);
/// [`Accepted`]'in tutamaçsız hâli.
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
    sent: Vec<Accepted>,
    /// Tek atımlık düğme: bu uçtan, oturum ve boy denetimlerini geçen sıradaki bu kadar gönderim
    /// `Backpressure` ile reddedilir — kuyruk boş olsa da (düğme kuyruk denetiminden önce gelir;
    /// kuyruk doluysa ret yine düğmeden sayılır). Reddedilen gönderim kaydedilmez, karşıya gitmez.
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

    /// `side` ucunun oturumu `reason` ile biter: yalnız o uca `Closed(h, reason)`. Karşı uç
    /// kapanıştan habersizdir; oradan gelmeye devam eden birimler kapanmış tutamaçla teslim edilir.
    fn end(&mut self, side: Side, reason: CloseReason) {
        let endpoint = self.endpoint(side);
        let handle = endpoint.active.take().expect("end: aktif oturum yok");
        endpoint.push_event(SessionEvent::Closed(handle, reason));
    }

    /// `side` ucunun süreci öldü: Transport durumu (oturum, kuyruklar, olaylar, kayıt) yok olur.
    /// Karşı uç bunu bilmez; eski oturumu hâlâ aktiftir.
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

    /// `to` ucuna, aktif oturumu üzerinden eşten gelmiş gibi ham kayıt koyar.
    fn inject(&mut self, to: Side, unit: Vec<u8>) {
        let endpoint = self.endpoint(to);
        let session = endpoint.active.expect("inject: aktif oturum yok");
        assert!(endpoint.inbox.len() < LINK_QUEUE_CAP, "inject: kuyruk dolu");
        endpoint.inbox.push_back(Delivered { session, unit });
    }

    /// `to` ucunun teslim kuyruğunu tavanına dek boş kayıtla doldurur (codec'te düşerler, olay
    /// yok): eşin o uca sonraki gönderimi `Backpressure` alır.
    fn fill_inbox(&mut self, to: Side) {
        let endpoint = self.endpoint(to);
        let session = endpoint.active.expect("fill_inbox: aktif oturum yok");
        while endpoint.inbox.len() < LINK_QUEUE_CAP {
            endpoint.inbox.push_back(Delivered {
                session,
                unit: Vec::new(),
            });
        }
    }

    /// `to` ucunun teslim kuyruğundaki birimleri yoldan alır — motora verilmemiştir, tutamaçları
    /// korunur: test onları kaybettirebilir ya da sonra [`Link::deliver_late`] ile verebilir.
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
    /// olayların karşılığı; ortak sahte de böyle düşürür). Supersede edilmiş tutamaçların kaydı da
    /// silinir: onların `Closed`'ı artık hiç okunmayacaktır.
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
        _class: TrafficClass,
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
        me.sent
            .push((session, message.msg_type(), policy, unit.compression_policy));
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

/// Bir uç: Messenger + PTT motoru, ikisi AYNI depoyla (stream_id depodan). Olaylar yalnız PTT
/// motorundan okunur (Messenger'ın tek okuyucusu).
struct Node {
    messenger: Messenger<End>,
    ptt: Ptt,
}

impl Node {
    fn new(
        link: &Rc<RefCell<Link>>,
        side: Side,
        store: Arc<dyn AppStore>,
        clock: &ManualClock,
    ) -> Self {
        let end = End {
            link: Rc::clone(link),
            side,
        };
        Self {
            messenger: Messenger::with_store(end, store).expect("limitler geçerli"),
            ptt: Ptt::new(Arc::new(clock.clone())),
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
        drain_events(&mut self.ptt, &mut self.messenger)
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
        Self::unconnected_with(fresh_store())
    }

    /// Alice'in motorları `alice_store` ile (yeniden başlatma testleri aynı depoyu yeniden verir),
    /// Bob'unkiler taze depoyla.
    fn unconnected_with(alice_store: Arc<dyn AppStore>) -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let clock = ManualClock::default();
        let alice = Node::new(&link, Side::Alice, alice_store, &clock);
        let bob = Node::new(&link, Side::Bob, fresh_store(), &clock);
        Self {
            link,
            clock,
            alice,
            bob,
        }
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
            duo.alice.drain(),
            vec![session(SessionEvent::Ready(handle, BOB))]
        );
        assert_eq!(
            duo.bob.drain(),
            vec![session(SessionEvent::Ready(handle, ALICE))]
        );
        (duo, handle)
    }

    /// Alice'in süreci ölür ve `store` ile yeniden başlar: eski motorlar ve Transport durumu yok
    /// olur; Bob'un ucu bunu bilmez. Yeni oturumu test kurar ([`Link::reconnect_after_crash`]).
    fn restart_alice(&mut self, store: Arc<dyn AppStore>) {
        self.link.borrow_mut().crash(Side::Alice);
        self.alice = Node::new(&self.link, Side::Alice, store, &self.clock);
    }

    fn inject(&self, to: Side, unit: Vec<u8>) {
        self.link.borrow_mut().inject(to, unit);
    }

    fn fill_inbox(&self, to: Side) {
        self.link.borrow_mut().fill_inbox(to);
    }

    fn take_inbox(&self, to: Side) -> Vec<Delivered> {
        self.link.borrow_mut().take_inbox(to)
    }

    fn deliver_late(&self, to: Side, units: Vec<Delivered>) {
        self.link.borrow_mut().deliver_late(to, units);
    }

    fn drop_events(&self, side: Side) {
        self.link.borrow_mut().drop_events(side);
    }

    /// `side` ucundan sıradaki `count` gönderim `Backpressure` ile reddedilir (tek atımlık).
    fn refuse_next_sends(&self, side: Side, count: usize) {
        self.link.borrow_mut().endpoint(side).refuse_next = count;
    }

    /// `side` ucundan Transport'un kabul ettiği gönderimler, sırasıyla.
    fn sent(&self, side: Side) -> Vec<Sent> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .map(|&(_, msg_type, policy, compression)| (msg_type, policy, compression))
            .collect()
    }

    /// `side` ucundan `handle` tutamacına kabul edilen kayıtların tipleri, sırasıyla.
    fn sent_on(&self, side: Side, handle: SessionHandle) -> Vec<MsgType> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .filter(|accepted| accepted.0 == handle)
            .map(|accepted| accepted.1)
            .collect()
    }
}

// ── yaşam döngüsü ve gönderen ────────────────────────────────────────────────────────────────

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
    assert!(duo.bob.ptt.incoming_streams(ALICE).is_empty());
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats::default(),
        "hiçbir kayıt düşmedi"
    );
    // PTT kayıtları alındı üretmez: gönderene hiçbir şey dönmez.
    assert!(duo.alice.drain().is_empty());
    // Sıradaki basış eşin kalıcı sayacından yeni `stream_id` alır ve kendi parametreleriyle açılır.
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

/// PTT-R0 (plan D7 ek, K4G-08 §0): `PTT_OPEN`, `MEDIA_BUNDLE` ve `PTT_CLOSE` Transport'a
/// `Reliable` verilir; hiçbir PTT kaydı sıkıştırılmaz.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): `MEDIA_BUNDLE` `BestEffort` gidiyordu (`ptt.rs:433`).
/// [PTT-BANT · kol A] Kompakt demetler (2 B başlık) alıcıda akışın `codec` / `bundle_ms`'iyle
/// açılır: olaylar tam biçimle aynıdır. Akışı olmayan kompakt demet PTT-R3 (yetim) sayılır.
#[test]
fn compact_bundles_deliver_like_full_bundles() {
    let (mut duo, _) = Duo::connected();
    duo.alice.ptt.set_compact_bundles(true);
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
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((0..TEST_SPURT_BUNDLES).map(|n| audio(ALICE, stream, &frame(n))));
    want.push(ended(ALICE, stream));
    assert_eq!(duo.bob.drain(), want);
    assert_eq!(duo.bob.ptt.stats(), PttStats::default());
    // Akış kapandı: aynı etiketli kompakt demet yetimdir.
    let orphan = encode(
        &AppMessage::MediaBundleCompact {
            stream_tag: kama_app::codec::compact_stream_tag(stream),
            payload: frame(9),
        },
        &limits(),
    )
    .expect("kodlanır")
    .bytes;
    duo.inject(Side::Bob, orphan);
    assert_eq!(duo.bob.drain(), Vec::new());
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);
}

#[test]
fn r0_all_ptt_records_are_reliable() {
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
    // [YAMA 2026-11-22] Açılış ve kapanış Reliable; demet son tarihli (1 s).
    let deadline = |p: DeliveryPolicy| matches!(p, DeliveryPolicy::DeadlineBounded { .. });
    let expected: Vec<(MsgType, bool, CompressionPolicy)> =
        iter::once((MsgType::PttOpen, false, CompressionPolicy::Never))
            .chain(iter::repeat_n(
                (MsgType::MediaBundle, true, CompressionPolicy::Never),
                usize::from(TEST_SPURT_BUNDLES),
            ))
            .chain(iter::once((
                MsgType::PttClose,
                false,
                CompressionPolicy::Never,
            )))
            .collect();
    let sent: Vec<(MsgType, bool, CompressionPolicy)> = duo
        .sent(Side::Alice)
        .into_iter()
        .map(|(t, p, c)| (t, deadline(p), c))
        .collect();
    assert_eq!(sent, expected);
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
    // Akışı açılmamış demet düşer (S7, PTT-R3) — ör. `PTT_OPEN`'ı eski oturumla kaybolmuş bir
    // akışın demeti. Aynı oturumda Reliable sıra demetin kendi `PTT_OPEN`'ını geçmesine izin vermez.
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
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);
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
    // Akışın `PTT_CLOSE`'undan sonra işlenen demet (aynı oturumda Reliable sıra bunu engeller; eski
    // bir oturumun drain'inden gelebilir) akışsızdır ve düşer (PTT-R3).
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
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);
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
    // Yeni akış etkinken kapanmış eski akışın geç demeti de bilinmeyen akıştır.
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
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 2);
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
        assert_eq!(stats.ptt_orphan_bundles, 2);
        assert_eq!(stats.unknown_closes, 1);
        assert_eq!(stats.opens_without_session, 1);
        assert!(duo.bob.ptt.incoming_streams(ALICE).is_empty());
    }
}

/// Gönderen hataları tiplidir ve reddedilen `open` akış açmaz. `NoSession` depoya dokunmadan döner;
/// Transport reddinde (`Backpressure`) ayrılmış `stream_id` YANAR (boşluk; modül belgesi —
/// alıcı ardışıklık zorlamaz).
///
/// `0904f723`'te KIRMIZI (uyarlansa da): reddedilen `open` değer tüketmiyordu (`stream ==
/// FIRST_STREAM_ID`); K4G-08 kalıcı ayırmada hata yolunun değer yaktığını seçti.
#[test]
fn sender_errors_are_typed_and_a_refused_open_opens_nothing() {
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
    // Bob'un kuyruğu dolu: Transport'un `Backpressure`'ı aynen; akış açılmaz.
    duo.fill_inbox(Side::Bob);
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
        stream,
        FIRST_STREAM_ID + 1,
        "NoSession değer ayırmadı; reddedilen PTT_OPEN'ın ayırdığı değer yandı"
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

/// `close` giden akışı HER DURUMDA kapatır ve `PTT_CLOSE` yeniden denenmez (S1 K8). Bob'da
/// `PTT_CLOSE`'u gelmeyen akış, Alice'in sıradaki basışıyla BİTMEZ (en yeni kazanır kalktı): yeni
/// akışla birlikte etkin kalır (PTT-R5) ve PTT-R4 ile biter.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): sıradaki `PTT_OPEN` eski akışı örtük bitiriyordu
/// (`PttEnded` + `PttStarted`); PTT-R4 yoktu.
#[test]
fn close_always_ends_the_outgoing_stream_and_is_never_retried() {
    let (mut duo, handle) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    // Bob'un kuyruğu `PTT_OPEN`'ın arkasından dolar.
    duo.fill_inbox(Side::Bob);
    // Reliable demet de Transport'un geri basıncına tabidir; ret aynen döner, motor yeniden denemez.
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
    // Motor `PTT_CLOSE`'u kendiliğinden yeniden göndermez. Alice'in sıradaki basışı Bob'da YENİ
    // akıştır; eski akış onu bekleyen alıcıda yanında etkin kalır.
    let next = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(next, stream + 1);
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, next, PttCodec::Lyra, TEST_BUNDLE_MS)]
    );
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![stream, next]);
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
    // Bob'da `PTT_CLOSE`'u gelmeyen iki akış PTT-R4 ile, yerel olarak biter.
    duo.clock.advance(PTT_STREAM_IDLE_TTL);
    assert_eq!(
        duo.bob.drain(),
        vec![ended(ALICE, stream), ended(ALICE, next)]
    );
    assert_eq!(duo.bob.ptt.stats().streams_ended_by_idle, 2);
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

// ── oturum değişimi: yeniden `PTT_OPEN` (D5, K4G-08 §1) ──────────────────────────────────────

/// Akış eşindir: supersession akışı bitirmez; aynı `stream_id` yeni tutamaçla sürer ve yeni oturuma
/// önce akışın `PTT_OPEN`'ı gider (D5). Eski tutamaçla yoldaki demet de akışa bağlanır.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeni oturumda `PTT_OPEN` yoktu (`sent_on(h')` = demet +
/// kapanış); Bob'da `duplicate_opens` 0'dı.
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
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle, MsgType::PttClose]
    );
    // Bob: oturum olayları önce; supersession akışı bitirmez; eski tutamaçtan gelen demet de akışa
    // bağlanır; yeniden `PTT_OPEN` etkin akışın tekrarıdır (PTT-R1).
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
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats {
            duplicate_opens: 1,
            ..PttStats::default()
        }
    );
}

/// D5 / K4G-08 §1 (olay yolu; DUR #107): asıl `PTT_OPEN` eski oturumla yolda kaybolur (S1 K7:
/// Transport teslim edilmemiş birimi yeni oturuma taşımaz). Açık akışın `PTT_OPEN`'ı yeni oturuma
/// AYNI `stream_id` ile, `Reliable`, TAM BİR KEZ yeniden gider — `Ready(h')` geçirilirken, o
/// oturumdaki demetlerden önce; sonraki yoklamalar ve demetler onu yinelemez. Bob'da akış yeniden
/// `PTT_OPEN`'la başlar, demetler yetim kalmaz.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu — yeni oturuma yalnız demetler
/// gider, Bob'da hepsi akışsız düşer (`PttStarted` / `PttAudio` yok).
#[test]
fn reopen_after_supersession_is_sent_exactly_once() {
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.take_inbox(Side::Bob).len(),
        1,
        "PTT_OPEN yolda kayboldu"
    );
    let new = duo.link.borrow_mut().supersede();
    // `Ready(h')` uygulamaya verildiğinde yeniden `PTT_OPEN` h'ye çoktan sunulmuştur.
    assert_eq!(
        duo.alice.ptt.poll(&mut duo.alice.messenger),
        Some(session(SessionEvent::Ready(new, BOB)))
    );
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen],
        "Ready(h') geçirilirken"
    );
    assert_eq!(
        duo.alice.drain(),
        vec![session(SessionEvent::Closed(old, CloseReason::Superseded))]
    );
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.drain().is_empty());
    }
    for n in 0..TEST_SPURT_BUNDLES {
        duo.alice
            .send_bundle(stream, &frame(n))
            .expect("gönderilir");
    }
    let on_new: Vec<MsgType> = iter::once(MsgType::PttOpen)
        .chain(iter::repeat_n(
            MsgType::MediaBundle,
            usize::from(TEST_SPURT_BUNDLES),
        ))
        .collect();
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        on_new,
        "PTT_OPEN tam bir kez"
    );
    assert!(duo.sent(Side::Alice).iter().all(|&(msg_type, policy, _)| {
        if msg_type == MsgType::MediaBundle {
            matches!(policy, DeliveryPolicy::DeadlineBounded { .. })
        } else {
            policy == DeliveryPolicy::Reliable
        }
    }));
    let alice_stats = duo.alice.ptt.stats();
    assert_eq!(alice_stats.opens_resent, 1);
    assert_eq!(alice_stats.opens_resend_refused, 0);
    let expected: Vec<PttEvent> = [
        session(SessionEvent::Ready(new, ALICE)),
        session(SessionEvent::Closed(old, CloseReason::Superseded)),
        started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
    ]
    .into_iter()
    .chain((0..TEST_SPURT_BUNDLES).map(|n| audio(ALICE, stream, &frame(n))))
    .collect();
    assert_eq!(duo.bob.drain(), expected);
    assert_eq!(duo.bob.ptt.stats(), PttStats::default(), "yetim demet yok");
}

/// D5 (senkron yol): yeni oturum `Ready` olayı okunmadan benimsenebilir — idempotent
/// `Messenger::connect` mevcut tutamacı `ConnectResult::Ready(h')` ile olaysız döndürür (S1 K2).
/// Yeniden `PTT_OPEN` yine TAM BİR KEZ gider: `send_bundle`, `close` ya da `poll` — hangisi önce
/// gelirse — onu demetten / kapanıştan önce verir; sonra okunan `Ready(h')` onu yinelemez;
/// `Ready(h')` hiç okunmasa da yoklamanın başındaki denetim onu verir.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu (`sent_on(h')`'de `PTT_OPEN`
/// yok).
#[test]
fn reopen_follows_a_session_adopted_without_an_event() {
    // (a) `send_bundle` önce.
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle]
    );
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    duo.alice
        .send_bundle(stream, &frame(2))
        .expect("gönderilir");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle, MsgType::MediaBundle],
        "sonra okunan Ready(h') PTT_OPEN'ı yinelemedi"
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            audio(ALICE, stream, &frame(1)),
            audio(ALICE, stream, &frame(2)),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().duplicate_opens, 1);

    // (b) `close` önce: `PTT_OPEN`, sonra `PTT_CLOSE`.
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.alice.close(stream).expect("kapanır");
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::PttClose],
        "kapanmış akış için yeniden PTT_OPEN yok"
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            ended(ALICE, stream),
        ]
    );

    // (c) `poll` önce: yoklamanın başında, olaylar okunmadan.
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    duo.alice
        .send_bundle(stream, &frame(3))
        .expect("gönderilir");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle]
    );
    assert_eq!(duo.alice.ptt.stats().opens_resent, 1);

    // (d) `Ready(h')` olayı hiç okunmaz (ör. taşan olay kuyruğunda düştü; tutamaç yalnız
    // `connect` dönüşünden öğrenilir): yoklamanın başındaki denetim `PTT_OPEN`'ı yine verir.
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    let new = duo.link.borrow_mut().supersede();
    duo.drop_events(Side::Alice);
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    assert!(duo.alice.drain().is_empty());
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::PttOpen]);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.drain().is_empty());
    }
    duo.alice
        .send_bundle(stream, &frame(4))
        .expect("gönderilir");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle]
    );
}

/// D5 (ret yolu): Transport yeniden `PTT_OPEN`'ı reddederse (`Backpressure`) tutamaç işaretlenmez
/// ve kayıt sonraki denetimde yeniden sunulur; kabul edildikten sonra o oturuma bir daha verilmez —
/// toplam TAM BİR kabul. Ret tek atımlık düğmeyle verilir (kuyruk durumundan bağımsız). (a) Yoklama
/// yolu: `Ready(h')` geçirilirken ret, sonraki yoklamada kabul. (b) `send_bundle` yolu: yeniden
/// `PTT_OPEN` bir kez reddedilir; demet KABUL EDİLEBİLİR durumdayken (kuyruk boş) bile h''ye
/// `PTT_OPEN`'dan önce GİTMEZ, `send_bundle` hata döner.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu; h''ye yalnız demet giderdi.
/// `send_bundle` yeniden `PTT_OPEN` reddini yutsaydı (`let _ = reopen(..)`) (b) KIRMIZI: demet
/// `PTT_OPEN`'sız giderdi.
#[test]
fn refused_reopen_is_retried_until_accepted_once() {
    // (a) Yoklama yolu.
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    let new = duo.link.borrow_mut().supersede();
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen],
        "ret sonraki yoklamada yeniden sunuldu"
    );
    let stats = duo.alice.ptt.stats();
    assert_eq!((stats.opens_resend_refused, stats.opens_resent), (1, 1));
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.drain().is_empty());
    }
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle]
    );
    assert_eq!(duo.alice.ptt.stats().opens_resent, 1);
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            audio(ALICE, stream, &frame(1)),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().duplicate_opens, 1);

    // (b) `send_bundle` yolu: yeni oturum olaysız benimsenir; kuyruk boş, demet kabul edilebilir.
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(
        duo.alice.send_bundle(stream, &frame(2)),
        Err(PttError::Transport(TransportError::Backpressure))
    );
    assert!(
        duo.sent_on(Side::Alice, new).is_empty(),
        "demet PTT_OPEN'dan önce gitmedi"
    );
    assert_eq!(duo.alice.ptt.stats().opens_resend_refused, 1);
    duo.alice
        .send_bundle(stream, &frame(3))
        .expect("gönderilir");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle]
    );
    assert_eq!(duo.alice.ptt.stats().opens_resent, 1);
}

/// D5 (kapanış yolu; beyanlı istisna 7): `close`'ta yeniden `PTT_OPEN` reddedilirse `PTT_CLOSE`
/// YİNE sunulur (alıcıdaki akışı bitiren odur), ret `opens_resend_refused`'ta sayılır, akış kapanır
/// ve kapanmış akışın `PTT_OPEN`'ı bir daha sunulmaz.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu; düğme `close`'un tek gönderimini
/// (`PTT_CLOSE`) reddeder, `close` hata döner ve h''ye hiçbir şey gitmez. `close` yeniden `PTT_OPEN`
/// reddini döndürseydi (`reopen(..)?`) de KIRMIZI: `PTT_CLOSE` hiç gitmez, Bob'da akış PTT-R4'e dek
/// etkin kalırdı.
#[test]
fn refused_reopen_in_close_still_sends_the_close() {
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(duo.alice.close(stream), Ok(()));
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), None, "akış kapandı");
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::PttClose]);
    let stats = duo.alice.ptt.stats();
    assert_eq!((stats.opens_resend_refused, stats.opens_resent), (1, 0));
    // Kapanmış akışın `PTT_OPEN`'ı sonraki yoklamalarda sunulmaz.
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice.drain().is_empty());
    }
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::PttClose]);
    // Akışı bilen Bob'da `PTT_CLOSE` akışı bitirir.
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            ended(ALICE, stream),
        ]
    );
}

/// D5 "her yeni oturumda bir kez": aynı açık akış art arda İKİ supersession görür; her yeni tutamaca
/// akışın `PTT_OPEN`'ı TAM BİR KEZ, o tutamaçtaki demetten önce gider; önceki tutamaca bir daha
/// verilmez (`opens_resent` == 2). Bob'da iki tekrar da PTT-R1'dir.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu (her yeni tutamaçta yalnız demet).
#[test]
fn each_new_session_gets_exactly_one_reopen() {
    let (mut duo, first) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    let mut previous = first;
    for n in 1..=2 {
        let current = duo.link.borrow_mut().supersede();
        assert_eq!(
            duo.alice.drain(),
            vec![
                session(SessionEvent::Ready(current, BOB)),
                session(SessionEvent::Closed(previous, CloseReason::Superseded)),
            ]
        );
        for _ in 0..IDLE_ROUNDS {
            assert!(duo.alice.drain().is_empty());
        }
        duo.alice
            .send_bundle(stream, &frame(n))
            .expect("gönderilir");
        assert_eq!(
            duo.sent_on(Side::Alice, current),
            vec![MsgType::PttOpen, MsgType::MediaBundle],
            "{n}. yeni oturum"
        );
        assert_eq!(
            duo.bob.drain(),
            vec![
                session(SessionEvent::Ready(current, ALICE)),
                session(SessionEvent::Closed(previous, CloseReason::Superseded)),
                audio(ALICE, stream, &frame(n)),
            ]
        );
        previous = current;
    }
    assert_eq!(
        duo.sent_on(Side::Alice, first),
        vec![MsgType::PttOpen],
        "ilk tutamaca yalnız asıl PTT_OPEN"
    );
    let stats = duo.alice.ptt.stats();
    assert_eq!((stats.opens_resent, stats.opens_resend_refused), (2, 0));
    assert_eq!(duo.bob.ptt.stats().duplicate_opens, 2);
}

// ── alıcı kuralları PTT-R0…R5 (AMP §3) ──────────────────────────────────────────────────────

/// PTT-R0 "geç gelen parçayı düşürmez": telde demet sıra numarası yoktur; motor demeti teslim
/// sırasıyla verir ve geç geldiği için düşürmez. (i) Aynı akışın eski oturumda yolda kalmış demeti,
/// yeni oturumdaki demetten SONRA gelir: ikisi de verilir (oturumlar arası sıra yükün işidir).
/// (ii) Eski akışın eski oturumda kalmış demeti ve `PTT_CLOSE`'u, aynı eşin yeni oturumdaki YENİ
/// akışının `PTT_OPEN`'ından SONRA gelir: eski akış yenisiyle birlikte etkindir (PTT-R5), demet
/// `PttAudio` olur, akış kendi `PTT_CLOSE`'uyla biter.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): (ii)'de "en yeni kazanır" yeni `PTT_OPEN`'da eski akışı
/// bitiriyordu (`PttEnded` + `PttStarted`); geç demet `bundles_unknown_stream`'e, `PTT_CLOSE`
/// `unknown_closes`'a düşüyordu. (i) AYNI DAVRANIŞ.
#[test]
fn r0_late_bundles_from_a_retired_session_are_delivered_not_dropped() {
    let (mut duo, first_session) = Duo::connected();
    let first = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, first, PttCodec::Opus, TEST_BUNDLE_MS)]
    );

    // (i) Demet 1 eski oturumda yolda kalır; demet 2 yeni oturumdan önce varır.
    duo.alice.send_bundle(first, &frame(1)).expect("gönderilir");
    let in_flight = duo.take_inbox(Side::Bob);
    let second_session = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(second_session, BOB)),
            session(SessionEvent::Closed(first_session, CloseReason::Superseded)),
        ]
    );
    duo.alice.send_bundle(first, &frame(2)).expect("gönderilir");
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(second_session, ALICE)),
            session(SessionEvent::Closed(first_session, CloseReason::Superseded)),
            audio(ALICE, first, &frame(2)),
        ]
    );
    duo.deliver_late(Side::Bob, in_flight);
    assert_eq!(
        duo.bob.drain(),
        vec![audio(ALICE, first, &frame(1))],
        "geç demet düşmedi"
    );

    // (ii) Demet 3 ve `PTT_CLOSE` ikinci oturumda yolda kalır; üçüncü oturumda yeni akış açılır.
    duo.alice.send_bundle(first, &frame(3)).expect("gönderilir");
    duo.alice.close(first).expect("kapanır");
    let in_flight = duo.take_inbox(Side::Bob);
    let third_session = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(third_session, BOB)),
            session(SessionEvent::Closed(
                second_session,
                CloseReason::Superseded
            )),
        ]
    );
    let second = duo
        .alice
        .open(BOB, PttCodec::Lyra, OTHER_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(third_session, ALICE)),
            session(SessionEvent::Closed(
                second_session,
                CloseReason::Superseded
            )),
            started(ALICE, second, PttCodec::Lyra, OTHER_BUNDLE_MS),
        ]
    );
    assert_eq!(
        duo.bob.ptt.incoming_streams(ALICE),
        vec![first, second],
        "yeni akış eskisini bitirmedi"
    );
    duo.deliver_late(Side::Bob, in_flight);
    assert_eq!(
        duo.bob.drain(),
        vec![audio(ALICE, first, &frame(3)), ended(ALICE, first)]
    );
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![second]);
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats {
            duplicate_opens: 1,
            ..PttStats::default()
        },
        "tek tekrar (i)'deki yeniden PTT_OPEN; hiçbir demet düşmedi"
    );
}

/// PTT-R1: eşin etkin akışlarından HERHANGİ birinin `stream_id`'siyle tekrar gelen `PTT_OPEN`
/// idempotenttir — yeni akış yok, hata yok, olay yok; akışın parametreleri ilk `PTT_OPEN`'la sabit
/// kalır (S9). Gönderenin yeni oturumdaki yeniden `PTT_OPEN`'ı (D5) alıcıda bu yoldan söner.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): iki etkin akış olamıyordu; ikinci `PTT_OPEN` birinciyi
/// bitiriyor, birincinin tekrar `PTT_OPEN`'ı da ikinciyi bitirip birinciyi YENİDEN başlatıyordu.
#[test]
fn r1_repeated_open_for_an_active_stream_is_idempotent() {
    let (mut duo, _) = Duo::connected();
    let (older, newer): (StreamId, StreamId) = (10, 11);
    duo.inject(
        Side::Bob,
        open_record(older, PttCodec::Opus, TEST_BUNDLE_MS),
    );
    duo.inject(
        Side::Bob,
        open_record(newer, PttCodec::Lyra, OTHER_BUNDLE_MS),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, older, PttCodec::Opus, TEST_BUNDLE_MS),
            started(ALICE, newer, PttCodec::Lyra, OTHER_BUNDLE_MS),
        ]
    );
    // Aynı parametrelerle, farklı parametrelerle ve öbür akış için tekrar.
    duo.inject(
        Side::Bob,
        open_record(older, PttCodec::Opus, TEST_BUNDLE_MS),
    );
    duo.inject(
        Side::Bob,
        open_record(older, PttCodec::Lyra, OTHER_BUNDLE_MS),
    );
    duo.inject(
        Side::Bob,
        open_record(newer, PttCodec::Lyra, OTHER_BUNDLE_MS),
    );
    assert!(duo.bob.drain().is_empty(), "olay yok");
    assert_eq!(
        duo.bob.ptt.incoming_streams(ALICE),
        vec![older, newer],
        "yeni akış yok"
    );
    // Parametreler ilk `PTT_OPEN`'ınki: akışın parametreleriyle demet geçer, tekrarınkiyle düşer.
    duo.inject(
        Side::Bob,
        bundle_record(older, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(1)),
    );
    duo.inject(
        Side::Bob,
        bundle_record(older, PttCodec::Lyra.code(), OTHER_BUNDLE_MS, &frame(2)),
    );
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, older, &frame(1))]);
    let stats = duo.bob.ptt.stats();
    assert_eq!(stats.duplicate_opens, 3);
    assert_eq!(stats.bundles_mismatched, 1);
}

/// PTT-R1, kapanmış akış (inceleme bulgusu "hayalet akış"): supersession'da akışın asıl `PTT_OPEN`'ı
/// ve bir demeti eski oturumun drain'inde kalır (S1 K7); yeni oturumdaki yeniden `PTT_OPEN`, demet
/// ve `PTT_CLOSE` önce işlenir. Sonra gelen asıl `PTT_OPEN` kapanmış akışı yeniden BAŞLATMAZ: ikinci
/// `PttStarted` yok, etkin akış yok (PTT-R5 yuvası boş). Aynı drain'in geç demeti PTT-R3'tür.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu (`sent_on(h')` = demet + kapanış).
/// `b1cce203`'te (K4G-08'in inceleme öncesi hâli) de KIRMIZI: geç `PTT_OPEN` aynı basışa ikinci
/// `PttStarted` üretiyor, geç demet o hayalet akışa `PttAudio` oluyor ve akış PTT-R4'e dek etkin
/// kalıyordu.
#[test]
fn r1_late_open_from_a_retired_session_does_not_restart_a_closed_stream() {
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    duo.alice
        .send_bundle(stream, &frame(0))
        .expect("gönderilir");
    let in_flight = duo.take_inbox(Side::Bob);
    assert_eq!(
        in_flight.len(),
        2,
        "PTT_OPEN ve demet eski oturumun drain'inde"
    );
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    duo.alice.close(stream).expect("kapanır");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle, MsgType::PttClose]
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
            audio(ALICE, stream, &frame(1)),
            ended(ALICE, stream),
        ]
    );
    duo.deliver_late(Side::Bob, in_flight);
    assert!(duo.bob.drain().is_empty(), "ikinci PttStarted yok");
    assert!(
        duo.bob.ptt.incoming_streams(ALICE).is_empty(),
        "hayalet akış yok"
    );
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats {
            opens_after_close: 1,
            ptt_orphan_bundles: 1,
            ..PttStats::default()
        },
        "geç PTT_OPEN PTT-R1, geç demet PTT-R3"
    );
}

/// PTT-R1, kapanmış akış — ikinci yol (son tur inceleme bulgusu): gönderende `close`'un yeniden
/// `PTT_OPEN`'ı reddedilir ve yeni oturuma yalnız `PTT_CLOSE` gider (beyanlı istisna 7). Asıl
/// `PTT_OPEN`'ı ve demeti eski oturumun drain'inde olan Bob'da bu `PTT_CLOSE` PTT-R2'ye düşer (olay
/// yok) ama kimlik kapanmış akış kaydına girer: sonra gelen asıl `PTT_OPEN` akış BAŞLATMAZ, etkin
/// akış kalmaz; geç demet PTT-R3'tür.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): yeniden `PTT_OPEN` yoktu; düğme `close`'un tek gönderimini
/// (`PTT_CLOSE`) reddeder, `close` hata döner. `dad269e7`'te de KIRMIZI: PTT-R2 ile yok sayılan
/// `PTT_CLOSE` kayda girmiyordu, geç `PTT_OPEN` hayalet bir `PttStarted` üretiyordu.
#[test]
fn r1_late_open_after_an_ignored_close_starts_no_ghost_stream() {
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    duo.alice
        .send_bundle(stream, &frame(0))
        .expect("gönderilir");
    let in_flight = duo.take_inbox(Side::Bob);
    assert_eq!(
        in_flight.len(),
        2,
        "PTT_OPEN ve demet eski oturumun drain'inde"
    );
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.messenger.connect(BOB),
        Ok(ConnectResult::Ready(new))
    );
    duo.refuse_next_sends(Side::Alice, 1);
    assert_eq!(duo.alice.close(stream), Ok(()));
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::PttClose]);
    // Bob: `PTT_CLOSE` etkin olmayan bir akışındır (PTT-R2) — olay yok, kimlik kayda girer.
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    duo.deliver_late(Side::Bob, in_flight);
    assert!(duo.bob.drain().is_empty(), "hayalet PttStarted yok");
    assert!(duo.bob.ptt.incoming_streams(ALICE).is_empty());
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats {
            unknown_closes: 1,
            opens_after_close: 1,
            ptt_orphan_bundles: 1,
            ..PttStats::default()
        },
        "CLOSE PTT-R2, geç PTT_OPEN PTT-R1, geç demet PTT-R3"
    );
}

/// Gerileme kilidi (hayalet akış düzeltmesinin sınırı): PTT-R4 ile ya da oturum sonuyla biten akış
/// (`PTT_CLOSE` gelmedi) kapanmış akış kaydına GİRMEZ — gönderen onu hâlâ açık tutuyor olabilir; yeni
/// oturumdaki yeniden `PTT_OPEN` (D5) akışı yeniden başlatır ve demetleri yine `PttAudio` olur.
///
/// `0904f723`'te UYARLANAMAZ ((a): saat ve `PTT_STREAM_IDLE_TTL` yoktu); (b) uyarlansa KIRMIZI:
/// yeniden `PTT_OPEN` yoktu, akış yeni oturumda yeniden başlamaz. `b1cce203`'te AYNI DAVRANIŞ (kayıt
/// yoktu; gerileme kilidi): kayıt bu kapanışları da tutsaydı KIRMIZI.
#[test]
fn reopen_restarts_a_stream_ended_by_idle_or_session_end() {
    // (a) PTT-R4.
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    duo.clock.advance(PTT_STREAM_IDLE_TTL);
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, stream)]);
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(new, BOB)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    );
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
            audio(ALICE, stream, &frame(1)),
        ]
    );
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![stream]);
    assert_eq!(duo.bob.ptt.stats().opens_after_close, 0);

    // (b) Oturum sonu: iki uçta da oturum biter, sonra yenisi kurulur.
    let (mut duo, old) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    duo.link
        .borrow_mut()
        .end(Side::Bob, CloseReason::PeerClosed);
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Closed(old, CloseReason::PeerClosed)),
            ended(ALICE, stream),
        ]
    );
    duo.link
        .borrow_mut()
        .end(Side::Alice, CloseReason::PeerClosed);
    let new = duo.link.borrow_mut().establish();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Closed(old, CloseReason::PeerClosed)),
            session(SessionEvent::Ready(new, BOB)),
        ]
    );
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::PttOpen]);
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().opens_after_close, 0);
}

/// PTT-R2: bilinmeyen `stream_id`'nin `PTT_CLOSE`'u sessizce yok sayılır — etkin akış yokken, başka
/// akışlar etkinken ve akış zaten kapanmışken; `PTT_CLOSE` yalnız kendi akışını bitirir.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): iki etkin akış olamıyordu (ikinci `PTT_OPEN` birinciyi
/// bitirirdi). Tek akışlı kısmı (bilinmeyen kapanış yok sayılır) AYNI DAVRANIŞ.
#[test]
fn r2_close_for_an_unknown_stream_is_ignored() {
    let (mut duo, _) = Duo::connected();
    duo.inject(Side::Bob, close_record(5));
    assert!(duo.bob.drain().is_empty());
    let (first, second, unknown): (StreamId, StreamId, StreamId) = (10, 11, 12);
    for stream in [first, second] {
        duo.inject(
            Side::Bob,
            open_record(stream, PttCodec::Opus, TEST_BUNDLE_MS),
        );
    }
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, first, PttCodec::Opus, TEST_BUNDLE_MS),
            started(ALICE, second, PttCodec::Opus, TEST_BUNDLE_MS),
        ]
    );
    duo.inject(Side::Bob, close_record(unknown));
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![first, second]);
    duo.inject(Side::Bob, close_record(first));
    duo.inject(Side::Bob, close_record(first));
    duo.inject(
        Side::Bob,
        bundle_record(second, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(1)),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![ended(ALICE, first), audio(ALICE, second, &frame(1))]
    );
    assert_eq!(duo.bob.ptt.stats().unknown_closes, 3);
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![second]);
}

/// PTT-R3: `stream_id`'si eşin etkin akışlarından biri olmayan `MEDIA_BUNDLE` düşer ve
/// `ptt_orphan_bundles` artar — etkin akış yokken (S7), başka akışlar etkinken ve akışın
/// `PTT_CLOSE`'undan sonra. Başka hiçbir sayaç artmaz.
///
/// `0904f723`'te UYARLANAMAZ: sayaç yoktu (`bundles_without_stream` / `bundles_unknown_stream`
/// ikiye bölüyordu). Uyarlansa KIRMIZI: iki etkin akış olamıyordu.
#[test]
fn r3_bundle_for_an_unknown_stream_is_an_orphan() {
    let (mut duo, _) = Duo::connected();
    let (first, second, unknown): (StreamId, StreamId, StreamId) = (10, 11, 12);
    let bundle =
        |stream, n| bundle_record(stream, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(n));
    duo.inject(Side::Bob, bundle(first, 0));
    assert!(duo.bob.drain().is_empty(), "etkin akış yok");
    for stream in [first, second] {
        duo.inject(
            Side::Bob,
            open_record(stream, PttCodec::Opus, TEST_BUNDLE_MS),
        );
    }
    duo.inject(Side::Bob, bundle(unknown, 1));
    duo.inject(Side::Bob, close_record(first));
    duo.inject(Side::Bob, bundle(first, 2));
    duo.inject(Side::Bob, bundle(second, 3));
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, first, PttCodec::Opus, TEST_BUNDLE_MS),
            started(ALICE, second, PttCodec::Opus, TEST_BUNDLE_MS),
            ended(ALICE, first),
            audio(ALICE, second, &frame(3)),
        ]
    );
    assert_eq!(
        duo.bob.ptt.stats(),
        PttStats {
            ptt_orphan_bundles: 3,
            ..PttStats::default()
        }
    );
}

/// PTT-R4: akışa `PTT_STREAM_IDLE_TTL` (normatif 120 s) boyunca `MEDIA_BUNDLE` gelmezse alıcı akışı
/// yerel olarak kapatır: `PttEnded` üretir, eşe hiçbir şey göndermez (`PTT_CLOSE` yok). Süre
/// `PTT_OPEN`'da başlar ve kabul edilen her demette sıfırlanır; düşen (S9) demet ve tekrar
/// `PTT_OPEN` sıfırlamaz. PoC-KAPSAM-001 K8: 60 s demetsiz kesintiden sonra akış sürer ve gelen demet
/// `PttAudio` olur. Transport kuyruğunda bekleyen demet süre denetiminden ÖNCE işlenir. Saat elle
/// ilerletilir.
///
/// `0904f723`'te UYARLANAMAZ (saat ve `PTT_STREAM_IDLE_TTL` yoktu). Uyarlansa KIRMIZI: akış hiçbir
/// zaman kendiliğinden bitmiyordu.
#[test]
fn r4_idle_stream_is_closed_locally_after_the_ttl() {
    assert_eq!(
        PTT_STREAM_IDLE_TTL,
        Duration::from_secs(120),
        "AMP §3 PTT-R4"
    );
    let (mut duo, _) = Duo::connected();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, stream, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    // Süre `PTT_OPEN`'da başladı: sınırın bir adım altında akış sürer.
    duo.clock.advance(PTT_STREAM_IDLE_TTL - TTL_MARGIN);
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![stream]);
    // Kabul edilen demet süreyi sıfırlar.
    duo.alice
        .send_bundle(stream, &frame(1))
        .expect("gönderilir");
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, stream, &frame(1))]);
    // K8: 60 s demetsiz kesinti — akış sürer, sonra gelen demet `PttAudio` olur (ve süreyi sıfırlar).
    duo.clock.advance(Duration::from_secs(60));
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![stream]);
    duo.alice
        .send_bundle(stream, &frame(4))
        .expect("gönderilir");
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, stream, &frame(4))]);
    duo.clock.advance(PTT_STREAM_IDLE_TTL - TTL_MARGIN);
    // Düşen demet (S9) ve tekrar `PTT_OPEN` (PTT-R1) sıfırlamaz.
    duo.inject(
        Side::Bob,
        bundle_record(stream, PttCodec::Lyra.code(), TEST_BUNDLE_MS, &frame(9)),
    );
    duo.inject(
        Side::Bob,
        open_record(stream, PttCodec::Opus, TEST_BUNDLE_MS),
    );
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![stream]);
    // Son kabul edilen demetten tam `PTT_STREAM_IDLE_TTL` sonra: yerel kapanış.
    duo.clock.advance(TTL_MARGIN);
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, stream)]);
    assert_eq!(duo.bob.ptt.stats().streams_ended_by_idle, 1);
    assert!(duo.bob.ptt.incoming_streams(ALICE).is_empty());
    assert!(
        duo.sent(Side::Bob).is_empty(),
        "alıcı hiçbir şey göndermedi: PTT_CLOSE yok"
    );
    // Kapanmış akışın sonraki demeti bilinmeyen akıştır (PTT-R3).
    duo.alice
        .send_bundle(stream, &frame(2))
        .expect("gönderilir");
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);

    // Kuyrukta bekleyen demet akışı canlı tutar: süre, kuyruk işlendikten sonra denetlenir.
    duo.alice.close(stream).expect("kapanır");
    let next = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, next, PttCodec::Opus, TEST_BUNDLE_MS)]
    );
    duo.alice.send_bundle(next, &frame(3)).expect("gönderilir");
    duo.clock.advance(PTT_STREAM_IDLE_TTL * 2);
    assert_eq!(
        duo.bob.drain(),
        vec![audio(ALICE, next, &frame(3))],
        "bekleyen demet önce işlendi"
    );
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![next]);
    duo.clock.advance(PTT_STREAM_IDLE_TTL);
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, next)]);
    assert_eq!(duo.bob.ptt.stats().streams_ended_by_idle, 2);
}

/// PTT-R5: aynı eşten aynı anda en çok `PTT_MAX_ACTIVE_STREAMS` (2) akış; fazlası düşer ve
/// `opens_over_active_limit` artar. Yeni akış eskisini bitirmez; bir akış bitince yer açılır.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): "en yeni kazanır" — ikinci `PTT_OPEN` birinciyi bitirirdi
/// (`PttEnded` + `PttStarted`), üçüncüsü düşmez ikinciyi bitirirdi; eş başına tek akış vardı.
#[test]
fn r5_at_most_two_streams_per_peer_are_active() {
    assert_eq!(PTT_MAX_ACTIVE_STREAMS, 2);
    let (mut duo, _) = Duo::connected();
    let (first, second, third): (StreamId, StreamId, StreamId) = (10, 11, 12);
    let bundle =
        |stream, n| bundle_record(stream, PttCodec::Opus.code(), TEST_BUNDLE_MS, &frame(n));
    for stream in [first, second, third] {
        duo.inject(
            Side::Bob,
            open_record(stream, PttCodec::Opus, TEST_BUNDLE_MS),
        );
    }
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, first, PttCodec::Opus, TEST_BUNDLE_MS),
            started(ALICE, second, PttCodec::Opus, TEST_BUNDLE_MS),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().opens_over_active_limit, 1);
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![first, second]);
    // İki etkin akışın demeti geçer; düşen akışınki bilinmeyen akıştır (PTT-R3).
    for (stream, n) in [(first, 1), (second, 2), (third, 3)] {
        duo.inject(Side::Bob, bundle(stream, n));
    }
    assert_eq!(
        duo.bob.drain(),
        vec![
            audio(ALICE, first, &frame(1)),
            audio(ALICE, second, &frame(2)),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 1);
    // Bir akış bitince yer açılır.
    duo.inject(Side::Bob, close_record(first));
    duo.inject(
        Side::Bob,
        open_record(third, PttCodec::Opus, TEST_BUNDLE_MS),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            ended(ALICE, first),
            started(ALICE, third, PttCodec::Opus, TEST_BUNDLE_MS),
        ]
    );
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![second, third]);
    assert_eq!(duo.bob.ptt.stats().opens_over_active_limit, 1);
}

// ── `stream_id`: kalıcılık, benzersizlik, depo hatası ───────────────────────────────────────

/// D5 (K4G-08 §1, `CounterKind::StreamId`): `stream_id` eş başına kalıcıdır — Ptt ve Messenger AYNI
/// depoyla yeniden kurulunca aynı eşe sıradaki `stream_id` öncekilerin hepsinden büyüktür. Alice
/// basış sürerken ölür (`PTT_CLOSE` hiç gitmez, Bob'da akış etkin kalır); yeniden başlayan Alice'in
/// yeni basışı Bob'da YENİ akış olarak başlar, sesi eski akışa karışmaz.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): sayaç motorun belleğindeydi; yeni motor
/// `FIRST_STREAM_ID`'den başlar (`second == first`), Bob'da `PTT_OPEN` etkin akışın tekrarı
/// sayılıp düşer (`PttStarted` yok) ve yeni basışın demetleri eski akışa bağlanır.
#[test]
fn stream_ids_continue_after_a_restart_with_the_same_store() {
    let store = fresh_store();
    let (mut duo, old) = Duo::connected_with(Arc::clone(&store));
    let first = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, first, PttCodec::Opus, TEST_BUNDLE_MS)]
    );

    duo.restart_alice(Arc::clone(&store));
    let new = duo.link.borrow_mut().reconnect_after_crash(Side::Alice);
    assert_eq!(
        duo.alice.drain(),
        vec![session(SessionEvent::Ready(new, BOB))]
    );
    let second = duo
        .alice
        .open(BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    // Sayaç depodan sürdü: öncekinden büyük; çökme anında ayrılmış ama kullanılmamış değer
    // olmadığı için boşluk da yok.
    assert_eq!(second, first + 1);
    duo.alice
        .send_bundle(second, &frame(1))
        .expect("gönderilir");
    assert_eq!(
        duo.bob.drain(),
        vec![
            session(SessionEvent::Ready(new, ALICE)),
            session(SessionEvent::Closed(old, CloseReason::Superseded)),
            started(ALICE, second, PttCodec::Opus, TEST_BUNDLE_MS),
            audio(ALICE, second, &frame(1)),
        ]
    );
    assert_eq!(duo.bob.ptt.stats().duplicate_opens, 0);
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![first, second]);
}

/// K4G-08 kararı (K4G-11 API'sinde `ptt_push(stream_id)` / `ptt_close(stream_id)` eş almaz): eş
/// başına sayaçlar iki eşe aynı değeri önerebilir; `Ptt::open` açık akışlar arasında
/// benzersizliği korur — başka bir eşin açık akışının değeri atlanır (kalıcı olarak ayrılmıştı,
/// yanar) ve yenisi ayrılır. Demet ve kapanış `stream_id`'ye göre DOĞRU eşin oturumuna gider.
/// Düzenek ortak yansıtıcı sahtedir: iki eşe iki oturum; gönderilen her kayıt aynı tutamaçla
/// "eşten gelmiş" gibi geri döner ve tutamacın eşine bağlanır.
///
/// `0904f723`'te KIRMIZI (uyarlansa da): tek, eşten bağımsız bellek sayacı vardı ve depo
/// okunmuyordu — olay akışı aynı kalır ama depodaki eş sayaçları (`Bob` 1, `Carol` 2 beklenir) 0
/// döner. Atlama kuralı kaldırılırsa da KIRMIZI: Carol'ın akışı Bob'unkini ezer.
#[test]
fn open_stream_ids_are_unique_across_peers() {
    let store = fresh_store();
    let mut messenger = Messenger::with_store(MockTransport::new(caps()), Arc::clone(&store))
        .expect("limitler geçerli");
    let mut ptt = Ptt::new(Arc::new(ManualClock::default()));
    for peer in [BOB, CAROL] {
        let ConnectResult::Pending(ticket) = messenger.connect(peer).expect("connect") else {
            panic!("bilinmeyen eşe ilk connect Pending olmalı");
        };
        let handle = messenger.transport_mut().complete_connect(&peer);
        assert_eq!(
            drain_events(&mut ptt, &mut messenger),
            vec![
                session(SessionEvent::ConnectProgress(
                    ticket,
                    ConnectPhase::Measuring
                )),
                session(SessionEvent::Ready(handle, peer)),
            ]
        );
    }
    let to_bob = ptt
        .open(&mut messenger, BOB, PttCodec::Opus, TEST_BUNDLE_MS)
        .expect("açılır");
    let to_carol = ptt
        .open(&mut messenger, CAROL, PttCodec::Lyra, OTHER_BUNDLE_MS)
        .expect("açılır");
    assert_eq!(to_bob, FIRST_STREAM_ID);
    assert_eq!(
        to_carol,
        FIRST_STREAM_ID + 1,
        "Bob'un açık akışının değeri atlandı"
    );
    ptt.send_bundle(&mut messenger, to_bob, &frame(1))
        .expect("gönderilir");
    ptt.send_bundle(&mut messenger, to_carol, &frame(2))
        .expect("gönderilir");
    ptt.close(&mut messenger, to_bob).expect("kapanır");
    assert_eq!(ptt.outgoing_stream(BOB), None);
    assert_eq!(ptt.outgoing_stream(CAROL), Some(to_carol));
    assert_eq!(
        drain_events(&mut ptt, &mut messenger),
        vec![
            started(BOB, to_bob, PttCodec::Opus, TEST_BUNDLE_MS),
            started(CAROL, to_carol, PttCodec::Lyra, OTHER_BUNDLE_MS),
            audio(BOB, to_bob, &frame(1)),
            audio(CAROL, to_carol, &frame(2)),
            ended(BOB, to_bob),
        ]
    );
    // Sayaçlar depoda eş başına sürer; Carol'ın atlanan değeri de ayrılmıştı.
    assert_eq!(
        store.reserve_next(BOB, CounterKind::StreamId),
        Ok(u64::from(to_bob) + 1)
    );
    assert_eq!(
        store.reserve_next(CAROL, CounterKind::StreamId),
        Ok(u64::from(to_carol) + 1)
    );
}

/// Depo hatası (K4G-08 kararı): `stream_id` ayrılamazsa tipli hata döner, `PTT_OPEN` Transport'a
/// verilmez ve akış açılmaz — panik yok, sessiz yutma yok. Depo tükenmesi `StreamIdExhausted`'dır.
/// Ayrılamayan değer yoktur: depo düzelince eşin ilk değeri gelir.
///
/// `0904f723`'te UYARLANAMAZ: `Ptt` depoyu okumuyordu (`PttError::Store` yoktu; depo arızası
/// `open`'ı etkilemezdi).
#[test]
fn store_failure_opens_nothing_and_sends_no_open() {
    let flaky = Arc::new(FlakyStore::default());
    let (mut duo, _) = Duo::connected_with(Arc::clone(&flaky) as Arc<dyn AppStore>);
    flaky.fail_reserve.store(true, Ordering::SeqCst);
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Opus, TEST_BUNDLE_MS),
        Err(PttError::Store(AppStoreError::Persist))
    );
    flaky.fail_reserve.store(false, Ordering::SeqCst);
    flaky.exhaust_reserve.store(true, Ordering::SeqCst);
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Opus, TEST_BUNDLE_MS),
        Err(PttError::StreamIdExhausted)
    );
    flaky.exhaust_reserve.store(false, Ordering::SeqCst);
    assert_eq!(duo.alice.ptt.outgoing_stream(BOB), None);
    assert!(
        duo.sent(Side::Alice).is_empty(),
        "PTT_OPEN Transport'a verilmedi"
    );
    assert!(duo.bob.drain().is_empty());
    assert_eq!(
        duo.alice.open(BOB, PttCodec::Opus, TEST_BUNDLE_MS),
        Ok(FIRST_STREAM_ID)
    );
    assert_eq!(
        duo.bob.drain(),
        vec![started(
            ALICE,
            FIRST_STREAM_ID,
            PttCodec::Opus,
            TEST_BUNDLE_MS
        )]
    );
}

// ── [YOL3-T1 KK-T1-11] Oynatma tamponu (T-P1…T-P8; sahte saat) ───────────────────────────────

/// Test T'si: varsayılan (2,0 s).
const PLAYOUT_T: Duration = kama_app::playout::PLAYOUT_DELAY_DEFAULT;
/// Saatin en küçük adımı.
const NANO: Duration = Duration::from_nanos(1);

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Bob'un tamponu açık, Alice bir akış açmış düzenek; Bob `PttStarted`'ı hemen görmüş.
fn playout_duo() -> (Duo, StreamId) {
    let (mut duo, _handle) = Duo::connected();
    duo.bob
        .ptt
        .set_playout(Some(kama_app::playout::PlayoutConfig::default()));
    let s = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("akış açıldı");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS)],
        "PttStarted hemen"
    );
    (duo, s)
}

/// T-P1: T dolmadan demet verilmez; ilk demet a0 + T'de.
#[test]
fn t_p1_first_bundle_waits_for_t_after_its_arrival() {
    let (mut duo, s) = playout_duo();
    duo.clock.advance(ms(30)); // a0 = 30 ms
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    assert!(duo.bob.drain().is_empty(), "varışta verilmez");
    duo.clock.advance(PLAYOUT_T - NANO);
    assert!(duo.bob.drain().is_empty(), "T dolmadan verilmez");
    duo.clock.advance(NANO);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(0))], "a0 + T");
}

/// T-P2: sonraki demetler `bundle_ms` aralıkla.
#[test]
fn t_p2_following_bundles_are_paced_by_bundle_ms() {
    let (mut duo, s) = playout_duo();
    for n in 0..TEST_SPURT_BUNDLES {
        duo.alice.send_bundle(s, &frame(n)).expect("demet");
    }
    assert!(duo.bob.drain().is_empty());
    duo.clock.advance(PLAYOUT_T);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(0))]);
    for n in 1..TEST_SPURT_BUNDLES {
        duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)) - NANO);
        assert!(duo.bob.drain().is_empty(), "bundle_ms dolmadan verilmez");
        duo.clock.advance(NANO);
        assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(n))]);
    }
}

/// T-P3: geç demet atlanmaz, gelince verilir, takvim ona yeniden çapalanır; sayaç artar.
#[test]
fn t_p3_late_bundle_is_waited_for_not_skipped_and_schedule_reanchors() {
    let (mut duo, s) = playout_duo();
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    assert!(duo.bob.drain().is_empty(), "varış (damga) t = 0");
    duo.clock.advance(PLAYOUT_T);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(0))]);
    // Demet 1'in takvimi T + 20 ms; ağ durur, demet 30 ms geç (T + 50 ms) gelir.
    duo.clock.advance(ms(50));
    assert!(duo.bob.drain().is_empty(), "boşluk atlanmaz: bekler");
    duo.alice.send_bundle(s, &frame(1)).expect("demet");
    duo.alice.send_bundle(s, &frame(2)).expect("demet");
    assert_eq!(
        duo.bob.drain(),
        vec![audio(ALICE, s, &frame(1))],
        "geç demet gelince hemen"
    );
    let stats = duo.bob.ptt.playout_stats().expect("tampon açık");
    assert_eq!((stats.playout_stalls, stats.playout_stall_ms), (1, 30));
    // Yeniden çapa: demet 2, demet 1'in verilişinden 20 ms sonra.
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)) - NANO);
    assert!(duo.bob.drain().is_empty());
    duo.clock.advance(NANO);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(2))]);
}

/// T-P4: `PttEnded` son demetten sonra, tampon boşalınca (PTT_CLOSE erken gelse bile); en erken
/// son demetin verilişi + `bundle_ms` (KK-T1-17).
#[test]
fn t_p4_ended_comes_after_the_last_bundle_even_if_close_arrives_early() {
    let (mut duo, s) = playout_duo();
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    duo.alice.send_bundle(s, &frame(1)).expect("demet");
    duo.alice.close(s).expect("kapanış");
    assert!(
        duo.bob.drain().is_empty(),
        "PTT_CLOSE geldi ama tampon dolu"
    );
    duo.clock.advance(PLAYOUT_T);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(0))]);
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)));
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(1))]);
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)) - NANO);
    assert!(
        duo.bob.drain().is_empty(),
        "son demetin süresi dolmadan bitmez"
    );
    duo.clock.advance(NANO);
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, s)]);
    assert!(duo.bob.ptt.incoming_streams(ALICE).is_empty());
}

/// T-P5: tampon kapalı (varsayılan) → demet varışta, sayaç yok (mevcut testler de aynı yolu koşar).
#[test]
fn t_p5_playout_off_is_todays_behaviour() {
    let (mut duo, _handle) = Duo::connected();
    assert_eq!(duo.bob.ptt.playout_stats(), None);
    let s = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("akış");
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    duo.alice.close(s).expect("kapanış");
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
            audio(ALICE, s, &frame(0)),
            ended(ALICE, s),
        ]
    );
}

/// T-P6: T aralık dışı → tipli hata, panik yok.
#[test]
fn t_p6_out_of_range_delay_is_a_typed_error() {
    use kama_app::playout::{
        PlayoutConfig, PlayoutConfigError, PLAYOUT_DELAY_MAX, PLAYOUT_DELAY_MIN,
    };
    for delay in [
        PLAYOUT_DELAY_MIN - NANO,
        PLAYOUT_DELAY_MAX + NANO,
        Duration::ZERO,
    ] {
        assert_eq!(
            PlayoutConfig::new(delay),
            Err(PlayoutConfigError::DelayOutOfRange { delay })
        );
    }
    for delay in [PLAYOUT_DELAY_MIN, PLAYOUT_DELAY_MAX, ms(2000)] {
        assert_eq!(PlayoutConfig::new(delay).map(|c| c.delay()), Ok(delay));
    }
    assert_eq!(PlayoutConfig::default().delay(), ms(2000));
}

/// T-P7 (KK-T1-19; EK-A §10, KK-T1-EKA-2): akış tavanı aşımı = yerel sonlandırma — tampondaki
/// demetler ve taşıran demet atılır (sayılır), `PttEnded` HEMEN verilir, hiçbir demet erken
/// verilmez; gönderenin sonraki demetleri elenir (sayılır), Ptt'nin `PttEnded`'i yutulur (yaşam
/// döngüsü başına tek `PttEnded`). Sonraki akış normal tamponlanır.
///
/// v1'de (aynı dalın önceki commit'i) KIRMIZI: taşmada en eski demet planından erken veriliyordu.
#[test]
fn t_p7_overflow_terminates_the_stream_locally_and_releases_nothing_early() {
    use kama_app::playout::{PlayoutStats, PLAYOUT_MAX_BUNDLES_PER_STREAM};
    let (mut duo, s) = playout_duo();
    let payload = |i: usize| (i as u16).to_be_bytes().to_vec();
    let mut events = Vec::new();
    for i in 0..PLAYOUT_MAX_BUNDLES_PER_STREAM {
        duo.alice.send_bundle(s, &payload(i)).expect("demet");
        events.extend(duo.bob.drain());
        // Sahte bağın gönderim kaydı (LINK_LOG_CAP) bu senaryoda sınır değildir.
        duo.link.borrow_mut().endpoint(Side::Alice).sent.clear();
    }
    assert!(
        events.is_empty(),
        "tavana dek: T dolmadı, hiçbir şey verilmez"
    );
    duo.alice
        .send_bundle(s, &payload(PLAYOUT_MAX_BUNDLES_PER_STREAM))
        .expect("demet");
    assert_eq!(
        duo.bob.drain(),
        vec![ended(ALICE, s)],
        "taşma: yerel sonlandırma, PttEnded hemen"
    );
    // Gönderen akışı açık tutar: sonraki demetler elenir; PTT_CLOSE'un PttEnded'i yutulur.
    for i in 0..3 {
        duo.alice.send_bundle(s, &payload(i)).expect("demet");
        assert!(
            duo.bob.drain().is_empty(),
            "sonlandırılmış akışın demeti elenir"
        );
    }
    duo.alice.close(s).expect("kapanış");
    assert!(
        duo.bob.drain().is_empty(),
        "yaşam döngüsü başına tek PttEnded"
    );
    duo.clock.advance(PLAYOUT_T * 2);
    assert!(
        duo.bob.drain().is_empty(),
        "atılan demet sonradan da verilmez"
    );
    assert_eq!(
        duo.bob.ptt.playout_stats(),
        Some(PlayoutStats {
            playout_overflow: 1,
            playout_discarded: 257,
            playout_late_eliminated: 3,
            playout_peak_bundles: 256,
            ..PlayoutStats::default()
        })
    );
    // Sonraki akış normal: PttStarted hemen, demet a0 + T'de.
    let s2 = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("yeni akış");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, s2, PttCodec::Lyra, TEST_BUNDLE_MS)]
    );
    duo.alice.send_bundle(s2, &frame(9)).expect("demet");
    assert!(duo.bob.drain().is_empty(), "varış (damga) şimdi");
    duo.clock.advance(PLAYOUT_T - NANO);
    assert!(duo.bob.drain().is_empty());
    duo.clock.advance(NANO);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s2, &frame(9))]);
}

/// T-P8: iki eşzamanlı akış bağımsız tamponlanır.
#[test]
fn t_p8_two_concurrent_streams_are_buffered_independently() {
    let (mut duo, s1) = playout_duo();
    duo.alice.send_bundle(s1, &frame(1)).expect("demet");
    duo.alice.close(s1).expect("kapanış");
    assert!(
        duo.bob.drain().is_empty(),
        "s1: varış t = 0, PttEnded ertelendi"
    );
    duo.clock.advance(ms(10));
    let s2 = duo
        .alice
        .open(BOB, PttCodec::Lyra, OTHER_BUNDLE_MS)
        .expect("ikinci akış");
    duo.alice.send_bundle(s2, &frame(2)).expect("demet");
    assert_eq!(
        duo.bob.drain(),
        vec![started(ALICE, s2, PttCodec::Lyra, OTHER_BUNDLE_MS)]
    );
    duo.clock.advance(PLAYOUT_T - ms(10));
    assert_eq!(
        duo.bob.drain(),
        vec![audio(ALICE, s1, &frame(1))],
        "s1: 0 + T"
    );
    duo.clock.advance(ms(10));
    assert_eq!(
        duo.bob.drain(),
        vec![audio(ALICE, s2, &frame(2))],
        "s2: 10 ms + T"
    );
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)) - ms(10));
    assert_eq!(
        duo.bob.drain(),
        vec![ended(ALICE, s1)],
        "s1 kendi takvimiyle biter"
    );
    duo.alice.close(s2).expect("kapanış");
    duo.clock.advance(ms(u64::from(OTHER_BUNDLE_MS)));
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, s2)]);
}

/// Tampon kapatılırken bekleyen demetler düşmez ve planlarından ERKEN verilmez (EK-A §10):
/// tampondaki akış planıyla biter, kapatmadan sonra başlayan akış tamponlanmaz; tablo boşalınca
/// tampon kalkar.
///
/// v1'de (aynı dalın önceki commit'i) KIRMIZI: kapatmada bekleyen demetler beklemeden veriliyordu.
#[test]
fn playout_switched_off_keeps_buffered_plans_and_bypasses_new_streams() {
    let (mut duo, s) = playout_duo();
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    duo.alice.send_bundle(s, &frame(1)).expect("demet");
    duo.bob.ptt.set_playout(None);
    duo.alice.send_bundle(s, &frame(2)).expect("demet");
    assert!(duo.bob.drain().is_empty(), "plan sürer: erken verilmez");
    assert!(duo.bob.ptt.playout_stats().is_some(), "akış bitmedi");
    // Kapatmadan sonra açılan akış tamponsuz geçer (plan yok). Gönderen eşe en çok bir açık giden
    // akış tuttuğu için ikinci akış ham kayıtla gelir (PTT-R5: eşten iki etkin akış).
    let s2: StreamId = 100;
    let lyra = PttCodec::Lyra;
    duo.inject(Side::Bob, open_record(s2, lyra, OTHER_BUNDLE_MS));
    duo.inject(
        Side::Bob,
        bundle_record(s2, lyra.code(), OTHER_BUNDLE_MS, &frame(9)),
    );
    duo.inject(Side::Bob, close_record(s2));
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, s2, PttCodec::Lyra, OTHER_BUNDLE_MS),
            audio(ALICE, s2, &frame(9)),
            ended(ALICE, s2),
        ]
    );
    duo.clock.advance(PLAYOUT_T);
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(0))]);
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)));
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(1))]);
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)));
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, s, &frame(2))]);
    duo.alice.close(s).expect("kapanış");
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)));
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, s)]);
    assert_eq!(duo.bob.ptt.playout_stats(), None, "boşalınca tampon kalkar");
}

// ── [YOL3-T1 EK-A §3, §10] Erken verilmeme, yerel sonlandırma, oturum değişimi (T-P9…T-P12) ──────

/// Yaşam döngüsü denetimi (EK-A §3.1, KK-T1-20): anahtar (eş, `stream_id`) başına olaylar
/// `PttStarted (PttAudio)* PttEnded` dizisinin tekrarıdır — açık olmayan anahtara demet ya da
/// `PttEnded` yok, açık anahtara ikinci `PttStarted` yok.
fn assert_lifecycles(events: &[PttEvent]) {
    let mut open = std::collections::BTreeSet::new();
    for event in events {
        match event {
            PttEvent::PttStarted {
                peer, stream_id, ..
            } => assert!(
                open.insert((*peer, *stream_id)),
                "iç içe yaşam döngüsü: {event:?}"
            ),
            PttEvent::PttAudio {
                peer, stream_id, ..
            } => assert!(
                open.contains(&(*peer, *stream_id)),
                "yaşam döngüsü dışında demet: {event:?}"
            ),
            PttEvent::PttEnded { peer, stream_id } => assert!(
                open.remove(&(*peer, *stream_id)),
                "açılmamış akışın sonu: {event:?}"
            ),
            PttEvent::Messenger(_) => {}
        }
    }
}

/// PTT dışı olayları atar.
fn ptt_only(events: &[PttEvent]) -> Vec<PttEvent> {
    events
        .iter()
        .filter(|event| !matches!(event, PttEvent::Messenger(_)))
        .cloned()
        .collect()
}

/// T-P9 (EK-A §10, KK-T1-EKA-2/8): hiçbir `PttAudio` planından ERKEN verilmez — `i`'inci demet en
/// erken `a0 + T + i·bundle_ms`'te (`a0` = ilk demetin varışı); geç teslim mümkün kalır (durma).
/// Tarama: T ∈ {alt, varsayılan, üst} × `bundle_ms` ∈ {20, 40} × dört varış deseni × ızgarayla
/// hizasız 7 ms düğüm turu. Her koşuda bütün demetler sırayla ve eksiksiz, `PttEnded` son demetten
/// sonra. Deterministik; RNG yok.
///
/// v1'de (aynı dalın önceki commit'i) AYNI DAVRANIŞ: taşma dışı yollar zaten erken vermiyordu
/// (v1'in tek erken yolu taşmaydı, T-P7); regresyon kilidi.
#[test]
fn t_p9_no_bundle_is_released_before_its_plan() {
    use kama_app::playout::{PlayoutConfig, PLAYOUT_DELAY_MAX, PLAYOUT_DELAY_MIN};
    /// Akıştaki demet sayısı.
    const BUNDLES: u64 = 24;
    /// Bob'un yoklama aralığı (ms) — `bundle_ms` ızgarasıyla hizasız.
    const TICK_MS: u64 = 7;
    /// "durma" deseninde duran demet ve duraklama (ms): T üst sınırından uzun, ızgarayla hizasız —
    /// tampon onu örtemez, geç teslim zorunlu.
    const STALL_AT: u64 = 6;
    const STALL_MS: u64 = 2_707;
    /// "titreşim" deseninin ms kaymaları (< 20 ms: gönderim sırası korunur).
    const JITTER_MS: [u64; 8] = [0, 11, 3, 17, 5, 0, 13, 2];
    type Pattern = fn(u64, u64) -> u64;
    let patterns: [(&str, Pattern); 4] = [
        ("hepsi birden", |_, _| 0),
        ("zamanında", |i, b| i * b),
        ("durma", |i, b| {
            if i < STALL_AT {
                i * b
            } else {
                i * b + STALL_MS
            }
        }),
        ("titreşim", |i, b| {
            i * b + JITTER_MS[usize::try_from(i % 8).expect("küçük")]
        }),
    ];
    let payload = |i: u64| i.to_be_bytes().to_vec();
    for delay in [PLAYOUT_DELAY_MIN, PLAYOUT_T, PLAYOUT_DELAY_MAX] {
        for bundle_ms in [TEST_BUNDLE_MS, OTHER_BUNDLE_MS] {
            for (name, send_at) in patterns {
                let b = u64::from(bundle_ms);
                let (mut duo, _) = Duo::connected();
                duo.bob
                    .ptt
                    .set_playout(Some(PlayoutConfig::new(delay).expect("aralıkta")));
                let s = duo
                    .alice
                    .open(BOB, PttCodec::Lyra, bundle_ms)
                    .expect("akış");
                let sends: Vec<u64> = (0..BUNDLES).map(|i| send_at(i, b)).collect();
                let last_send = *sends.last().expect("boş değil");
                let delay_ms = u64::try_from(delay.as_millis()).expect("küçük");
                let end_ms = last_send + delay_ms + (BUNDLES + 2) * b + 200;
                // a0: Bob'un ilk demeti okuduğu tur (gönderimden sonraki ilk yoklama).
                let a0 = sends[0].div_ceil(TICK_MS) * TICK_MS;
                let mut log: Vec<(u64, PttEvent)> = Vec::new();
                let mut next = 0;
                for now in 0..=end_ms {
                    while sends.get(next).is_some_and(|&at| at == now) {
                        let i = u64::try_from(next).expect("küçük");
                        duo.alice.send_bundle(s, &payload(i)).expect("demet");
                        next += 1;
                        if next == sends.len() {
                            duo.alice.close(s).expect("kapanış");
                        }
                    }
                    if now % TICK_MS == 0 {
                        log.extend(duo.bob.drain().into_iter().map(|e| (now, e)));
                    }
                    duo.clock.advance(ms(1));
                }
                let ctx = format!("T={delay:?} bundle_ms={bundle_ms} desen={name}");
                let events: Vec<PttEvent> = log.iter().map(|(_, e)| e.clone()).collect();
                assert_lifecycles(&events);
                let mut want = vec![started(ALICE, s, PttCodec::Lyra, bundle_ms)];
                want.extend((0..BUNDLES).map(|i| audio(ALICE, s, &payload(i))));
                want.push(ended(ALICE, s));
                assert_eq!(ptt_only(&events), want, "{ctx}: sırayla, eksiksiz");
                let mut late = false;
                for (i, (at, _)) in log
                    .iter()
                    .filter(|(_, e)| matches!(e, PttEvent::PttAudio { .. }))
                    .enumerate()
                {
                    let plan = a0 + delay_ms + u64::try_from(i).expect("küçük") * b;
                    assert!(*at >= plan, "{ctx}: demet {i} erken ({at} < plan {plan})");
                    late |= *at > plan + TICK_MS;
                }
                if name == "durma" {
                    assert!(late, "{ctx}: durma geç teslim üretir");
                    let stats = duo.bob.ptt.playout_stats().expect("tampon açık");
                    assert!(stats.playout_stalls >= 1, "{ctx}: durma sayıldı");
                }
            }
        }
    }
}

/// T-P10 (KK-T1-20; EK-A §3.6, KK-T1-EKA-6): supersession'da eski oynatma durumu kapanır —
/// tampondaki eski oturum demetleri çalınmaz (atılır, sayılır), `PttEnded` hemen; eski oturumun
/// drain'inden geç gelen demet ve akışın yeni oturumdaki devamı (`seq`'siz ayırt edilemez) elenir
/// (sayılır); Ptt'nin sonraki `PttEnded`'i yutulur. Yeni oturumda açılan yeni akış normal
/// tamponlanır. Kullanıcıya hiçbir eski oturum demeti verilmez.
///
/// v1'de (aynı dalın önceki commit'i) KIRMIZI: eski oturum demetleri planlarıyla çalınıyor, geç
/// drain demeti yeni oturumunkinden sonra veriliyordu.
#[test]
fn t_p10_supersession_closes_old_playout_and_eliminates_late_bundles() {
    use kama_app::playout::{PlayoutConfig, PlayoutStats};
    let (mut duo, first) = Duo::connected();
    duo.bob.ptt.set_playout(Some(PlayoutConfig::default()));
    let s = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("akış");
    let mut bob = duo.bob.drain();
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    duo.alice.send_bundle(s, &frame(1)).expect("demet");
    bob.extend(duo.bob.drain());
    duo.alice.send_bundle(s, &frame(2)).expect("demet");
    let in_flight = duo.take_inbox(Side::Bob);
    let second = duo.link.borrow_mut().supersede();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Ready(second, BOB)),
            session(SessionEvent::Closed(first, CloseReason::Superseded)),
        ]
    );
    duo.alice
        .send_bundle(s, &frame(3))
        .expect("yeni oturumda devam (D5 yeniden PTT_OPEN)");
    bob.extend(duo.bob.drain());
    duo.deliver_late(Side::Bob, in_flight);
    bob.extend(duo.bob.drain());
    duo.clock.advance(PLAYOUT_T * 2);
    bob.extend(duo.bob.drain());
    duo.alice.close(s).expect("kapanış");
    bob.extend(duo.bob.drain());
    let s2 = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("yeni oturumda yeni akış");
    duo.alice.send_bundle(s2, &frame(7)).expect("demet");
    bob.extend(duo.bob.drain());
    duo.clock.advance(PLAYOUT_T);
    bob.extend(duo.bob.drain());
    duo.alice.close(s2).expect("kapanış");
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)));
    bob.extend(duo.bob.drain());

    assert_lifecycles(&bob);
    assert_eq!(
        ptt_only(&bob),
        vec![
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
            ended(ALICE, s),
            started(ALICE, s2, PttCodec::Lyra, TEST_BUNDLE_MS),
            audio(ALICE, s2, &frame(7)),
            ended(ALICE, s2),
        ],
        "eski oturum demeti (0, 1, geç 2) ve akışın devamı (3) kullanıcıya verilmez"
    );
    let ready_at = bob
        .iter()
        .position(|e| *e == session(SessionEvent::Ready(second, ALICE)))
        .expect("Ready(h')");
    let ended_at = bob
        .iter()
        .position(|e| *e == ended(ALICE, s))
        .expect("PttEnded");
    assert!(ready_at < ended_at, "eski durum oturum değişiminde kapanır");
    assert_eq!(
        duo.bob.ptt.playout_stats(),
        Some(PlayoutStats {
            playout_session_closes: 1,
            playout_discarded: 2,
            playout_late_eliminated: 2,
            bundles_released: 1,
            playout_peak_bundles: 2,
            ..PlayoutStats::default()
        })
    );
}

/// T-P11 (KK-T1-20): oturum sonu + yeniden bağlanma — eşin oturumu biter (Ptt akışı bitirir,
/// tamponda iki demet kalır), yeni oturumda gönderenin yeniden `PTT_OPEN`'ı (D5) AYNI `stream_id`
/// ile yeni yaşam döngüsü başlatır: eski yaşam döngüsü önce kapanır (`PttEnded`, tampondaki eski
/// oturum demetleri atılır), sonra yeni `PttStarted`; yeni yaşam döngüsü kendi `a0 + T`'siyle
/// tamponlanır. İki yaşam döngüsü iç içe geçmez.
///
/// v1'de (aynı dalın önceki commit'i) KIRMIZI: yeni `PttStarted` eski yaşam döngüsü kapanmadan
/// veriliyor, iki yaşam döngüsünün demetleri tek tampona karışıyordu.
#[test]
fn t_p11_reopen_after_session_end_starts_a_fresh_lifecycle() {
    use kama_app::playout::{PlayoutConfig, PlayoutStats};
    let (mut duo, first) = Duo::connected();
    duo.bob.ptt.set_playout(Some(PlayoutConfig::default()));
    let s = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("akış");
    let mut bob = duo.bob.drain();
    duo.alice.send_bundle(s, &frame(0)).expect("demet");
    duo.alice.send_bundle(s, &frame(1)).expect("demet");
    bob.extend(duo.bob.drain());
    duo.link
        .borrow_mut()
        .end(Side::Bob, CloseReason::PeerClosed);
    bob.extend(duo.bob.drain());
    assert_eq!(
        ptt_only(&bob),
        vec![started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS)],
        "oturum sonu: Ptt akışı bitirdi, tampon dolu — PttEnded bekler"
    );
    duo.clock.advance(ms(500));
    duo.link
        .borrow_mut()
        .end(Side::Alice, CloseReason::PeerClosed);
    let second = duo.link.borrow_mut().establish();
    assert_eq!(
        duo.alice.drain(),
        vec![
            session(SessionEvent::Closed(first, CloseReason::PeerClosed)),
            session(SessionEvent::Ready(second, BOB)),
        ]
    );
    duo.alice
        .send_bundle(s, &frame(2))
        .expect("yeni oturumda devam");
    bob.extend(duo.bob.drain());
    duo.clock.advance(PLAYOUT_T - NANO);
    bob.extend(duo.bob.drain());
    assert_eq!(
        ptt_only(&bob),
        vec![
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
            ended(ALICE, s),
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
        ],
        "eski yaşam döngüsü yenisinden önce kapanır; yeni demet T dolmadan verilmez"
    );
    duo.clock.advance(NANO);
    bob.extend(duo.bob.drain());
    duo.alice.close(s).expect("kapanış");
    duo.clock.advance(ms(u64::from(TEST_BUNDLE_MS)));
    bob.extend(duo.bob.drain());
    assert_lifecycles(&bob);
    assert_eq!(
        ptt_only(&bob),
        vec![
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
            ended(ALICE, s),
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
            audio(ALICE, s, &frame(2)),
            ended(ALICE, s),
        ]
    );
    assert_eq!(
        duo.bob.ptt.playout_stats(),
        Some(PlayoutStats {
            playout_session_closes: 1,
            playout_discarded: 2,
            bundles_released: 1,
            playout_peak_bundles: 2,
            ..PlayoutStats::default()
        })
    );
}

/// T-P12 (KK-T1-20): aynı oturumda aynı anahtara yeni yaşam döngüsü (kapanmış akış kaydı taştıktan
/// sonra gelen `PTT_OPEN`, PTT-R1'in beyanlı sınırı) — tampondaki önceki yaşam döngüsü önce
/// kapanır: `PttEnded` yeni `PttStarted`'dan ÖNCE verilir, önceki demet atılır ve sayılır.
///
/// v1'de (aynı dalın önceki commit'i) KIRMIZI: aynı anahtara ikinci `PttStarted` önceki yaşam
/// döngüsü kapanmadan veriliyordu.
#[test]
fn t_p12_same_key_restart_closes_the_previous_lifecycle_first() {
    use kama_app::playout::PlayoutConfig;
    let (mut duo, _) = Duo::connected();
    duo.bob.ptt.set_playout(Some(PlayoutConfig::default()));
    let lyra = PttCodec::Lyra;
    let (s, x, y): (StreamId, StreamId, StreamId) = (10, 11, 12);
    duo.inject(Side::Bob, open_record(s, lyra, TEST_BUNDLE_MS));
    duo.inject(
        Side::Bob,
        bundle_record(s, lyra.code(), TEST_BUNDLE_MS, &frame(1)),
    );
    duo.inject(Side::Bob, close_record(s));
    let mut bob = duo.bob.drain();
    assert_eq!(
        ptt_only(&bob),
        vec![started(ALICE, s, lyra, TEST_BUNDLE_MS)],
        "PTT_CLOSE geldi, demet tamponda"
    );
    for id in [x, y] {
        duo.inject(Side::Bob, open_record(id, lyra, TEST_BUNDLE_MS));
        duo.inject(Side::Bob, close_record(id));
    }
    duo.inject(Side::Bob, open_record(s, lyra, TEST_BUNDLE_MS));
    bob.extend(duo.bob.drain());
    assert_lifecycles(&bob);
    assert_eq!(
        ptt_only(&bob),
        vec![
            started(ALICE, s, lyra, TEST_BUNDLE_MS),
            started(ALICE, x, lyra, TEST_BUNDLE_MS),
            ended(ALICE, x),
            started(ALICE, y, lyra, TEST_BUNDLE_MS),
            ended(ALICE, y),
            ended(ALICE, s),
            started(ALICE, s, lyra, TEST_BUNDLE_MS),
        ]
    );
    let stats = duo.bob.ptt.playout_stats().expect("tampon açık");
    assert_eq!(
        (stats.playout_session_closes, stats.playout_discarded),
        (1, 1)
    );
}

// ── [PTT-BİRLEŞİK · deney, sahip onayı 2026-11-23; aday dışı] açılış/kapanış demetin içinde ─────
//
// Değişen AMP kuralları (yalnız deney açıkken ve yeteneğini bildirmiş eşe; spec metni
// değişmedi): PTT-R3 — birleşik demet bilmediği akışı AÇAR (yetim sayılmaz); PTT-R4 — birleşik
// akışın hareketsizlik sonu 120 s değil `spurt_gap_end(bundle_ms)` (1,1 s + 2 × demet süresi);
// PTT_OPEN / PTT_CLOSE tele hiç çıkmaz (PTT-R0'ın "açılış ve kapanış Reliable" yaması);
// PTT-R1'in kapanmış akış kaydına `END`'li demet de yazar. Ayrıntı: `kama-app` `ptt.rs`.

/// Birleşik deney için bağlanmış düzenek: iki uçta da birleşik biçim açık, Bob yetenekli eş.
fn coalesced_duo() -> Duo {
    let (mut duo, _) = Duo::connected();
    duo.alice.ptt.set_spurt_coalesce(true);
    duo.bob.ptt.set_spurt_coalesce(true);
    assert!(duo.alice.ptt.set_peer_spurt_provisioned(BOB, true));
    duo
}

/// Birleşik basış: `n` demet (sonuncusu `END`'li), `lose` içindeki sıradakiler yolda kaybolur.
/// Döner: Bob'un olayları ve Bob'a giden birimlerin boyları (kaybolanlar dahil), gönderim sırasıyla.
fn coalesced_spurt(duo: &mut Duo, n: u8, lose: &[u8]) -> (StreamId, Vec<PttEvent>, Vec<usize>) {
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    assert!(duo.take_inbox(Side::Bob).is_empty(), "ayrı PTT_OPEN yok");
    let mut events = Vec::new();
    let mut sizes = Vec::new();
    for k in 0..n {
        let now = std::time::Instant::now();
        if k + 1 == n {
            duo.alice
                .ptt
                .send_last_bundle_from(&mut duo.alice.messenger, stream, &frame(k), now)
                .expect("son demet");
        } else {
            duo.alice
                .ptt
                .send_bundle_from(&mut duo.alice.messenger, stream, &frame(k), now)
                .expect("demet");
        }
        let units = duo.take_inbox(Side::Bob);
        assert_eq!(units.len(), 1, "demet başına tek birim");
        sizes.push(units[0].unit.len());
        if !lose.contains(&k) {
            duo.deliver_late(Side::Bob, units);
        }
        events.extend(duo.bob.drain());
        duo.clock
            .advance(Duration::from_millis(u64::from(TEST_BUNDLE_MS)));
    }
    duo.alice.close(stream).expect("kapanır");
    assert!(duo.take_inbox(Side::Bob).is_empty(), "ayrı PTT_CLOSE yok");
    events.extend(duo.bob.drain());
    (stream, events, sizes)
}

/// Kayıpsız birleşik basış: tele yalnız demetler çıkar (hepsi son tarihli); varsayılan kuralda
/// ilk üç demet açılış bilgisini taşır (7 B başlık), dördüncü kompakt `END`'li (0x8D, 2 B). Bob
/// akışı ilk demetten açar ve `END`'de bitirir; olaylar ayrı kayıtlı yolla aynıdır.
#[test]
fn coalesced_spurt_carries_open_and_close_inside_the_bundles() {
    let mut duo = coalesced_duo();
    let (stream, events, sizes) = coalesced_spurt(&mut duo, 4, &[]);
    let body = TEST_FRAME_BYTES;
    assert_eq!(sizes, vec![7 + body, 7 + body, 7 + body, 2 + body]);
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((0..4).map(|n| audio(ALICE, stream, &frame(n))));
    want.push(ended(ALICE, stream));
    assert_eq!(events, want);
    for (msg_type, policy, _) in duo.sent(Side::Alice) {
        assert_eq!(
            msg_type,
            MsgType::MediaBundle,
            "ayrı PTT_OPEN / PTT_CLOSE yok"
        );
        assert!(matches!(policy, DeliveryPolicy::DeadlineBounded { .. }));
    }
    let stats = duo.bob.ptt.stats();
    assert_eq!((stats.spurt_opens, stats.spurt_ends), (1, 1));
    assert_eq!(stats.ptt_orphan_bundles, 0);
    // `END`'den sonra aynı akışın geç açılış bilgili demeti akışı yeniden açmaz.
    duo.inject(
        Side::Bob,
        record(&AppMessage::MediaBundleOpen {
            stream_id: stream,
            codec: PttCodec::Lyra.code(),
            bundle_ms: TEST_BUNDLE_MS,
            end: false,
            payload: frame(0),
        }),
    );
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.stats().spurt_after_end, 1);
}

/// İLK datagram kaybolur: ikinci demet de açılış bilgisini taşıdığından Bob akışı ondan açar;
/// yetim demet yok, `END` akışı bitirir.
#[test]
fn coalesced_open_survives_the_loss_of_the_first_datagram() {
    let mut duo = coalesced_duo();
    let (stream, events, _) = coalesced_spurt(&mut duo, 4, &[0]);
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((1..4).map(|n| audio(ALICE, stream, &frame(n))));
    want.push(ended(ALICE, stream));
    assert_eq!(events, want);
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 0);
}

/// SON datagram kaybolur: `END` görülmez; Bob akışı son kabul edilen demetten tam
/// `spurt_gap_end(bundle_ms)` sonra (1 s + 100 ms + 2 × 20 ms) yerel olarak bitirir — bir adım
/// önce değil.
#[test]
fn coalesced_close_survives_the_loss_of_the_last_datagram_in_bounded_time() {
    let mut duo = coalesced_duo();
    let (stream, events, _) = coalesced_spurt(&mut duo, 4, &[3]);
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((0..3).map(|n| audio(ALICE, stream, &frame(n))));
    assert_eq!(events, want, "kapanış henüz yok");
    // Son kabul edilen demet (2.) işlendikten sonra saat 2 × 20 ms ilerledi.
    let gap = kama_app::ptt::spurt_gap_end(TEST_BUNDLE_MS);
    assert_eq!(gap, Duration::from_millis(1_140));
    let elapsed = Duration::from_millis(2 * u64::from(TEST_BUNDLE_MS));
    duo.clock.advance(gap - elapsed - Duration::from_nanos(1));
    assert!(duo.bob.drain().is_empty(), "sınırdan bir adım önce bitmez");
    duo.clock.advance(Duration::from_nanos(1));
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, stream)]);
    let stats = duo.bob.ptt.stats();
    assert_eq!((stats.streams_ended_by_gap, stats.spurt_ends), (1, 0));
    assert_eq!(stats.streams_ended_by_idle, 0, "PTT-R4 (120 s) değil");
}

/// İLK ve SON datagram birlikte kaybolur: akış ikinci demetten açılır, sessizlik sonuyla biter.
#[test]
fn coalesced_open_and_close_survive_losing_both_the_first_and_the_last_datagram() {
    let mut duo = coalesced_duo();
    let (stream, events, _) = coalesced_spurt(&mut duo, 4, &[0, 3]);
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((1..3).map(|n| audio(ALICE, stream, &frame(n))));
    assert_eq!(events, want);
    duo.clock
        .advance(kama_app::ptt::spurt_gap_end(TEST_BUNDLE_MS));
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, stream)]);
    let stats = duo.bob.ptt.stats();
    assert_eq!(stats.ptt_orphan_bundles, 0);
    assert_eq!(stats.streams_ended_by_gap, 1);
}

/// Ortadaki art arda iki kayıp akışı erken bitirmez (sınır 2 demetlik boşluğa pay bırakır).
#[test]
fn coalesced_stream_does_not_end_early_on_two_lost_middle_bundles() {
    let mut duo = coalesced_duo();
    let (stream, events, _) = coalesced_spurt(&mut duo, 6, &[2, 3]);
    assert_eq!(events.last(), Some(&ended(ALICE, stream)), "END ile biter");
    assert_eq!(duo.bob.ptt.stats().streams_ended_by_gap, 0);
}

/// `END`'li son demet verilmeden `close`: tek yüksüz `END` demeti gider (son tarihli).
#[test]
fn coalesced_close_without_a_last_bundle_sends_an_empty_end() {
    let mut duo = coalesced_duo();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    for k in 0..3 {
        duo.alice.send_bundle(stream, &frame(k)).expect("demet");
    }
    duo.alice.close(stream).expect("kapanır");
    let units = duo.take_inbox(Side::Bob);
    assert_eq!(units.len(), 4);
    assert_eq!(units[3].unit.len(), 2, "yüksüz kompakt END");
    duo.deliver_late(Side::Bob, units);
    let events = duo.bob.drain();
    assert_eq!(events.len(), 1 + 3 + 1);
    assert_eq!(events.last(), Some(&ended(ALICE, stream)));
}

/// Eski eş uyumu: yeteneğini bildirmemiş eşe bugünkü ayrı güvenilir kayıtlar gider; birleşik
/// biçimi kapalı alıcı birleşik demeti DROP eder (v1 alıcısı da `unknown-msg-version` ile atar).
#[test]
fn coalesced_form_is_used_only_toward_a_capable_peer() {
    let (mut duo, _) = Duo::connected();
    duo.alice.ptt.set_spurt_coalesce(true);
    duo.bob.ptt.set_spurt_coalesce(true);
    // Öğrenilmiş kaydın dönemi farklı: kullanılmaz (tutulmaz bile).
    assert!(!duo
        .alice
        .ptt
        .remember_spurt_capability(SpurtCapabilityRecord {
            peer: BOB,
            epoch: SPURT_FORMAT_EPOCH + 1,
        }));
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    duo.alice.send_bundle(stream, &frame(0)).expect("demet");
    duo.alice.close(stream).expect("kapanır");
    let types: Vec<MsgType> = duo.sent(Side::Alice).iter().map(|s| s.0).collect();
    assert_eq!(
        types,
        vec![MsgType::PttOpen, MsgType::MediaBundle, MsgType::PttClose]
    );
    assert_eq!(duo.bob.drain().len(), 3);
    // Alıcıda kapalı: birleşik demet düşer, olay yok.
    duo.bob.ptt.set_spurt_coalesce(false);
    duo.inject(
        Side::Bob,
        record(&AppMessage::MediaBundleOpen {
            stream_id: stream + 1,
            codec: PttCodec::Lyra.code(),
            bundle_ms: TEST_BUNDLE_MS,
            end: true,
            payload: frame(1),
        }),
    );
    assert!(duo.bob.drain().is_empty());
    assert_eq!(duo.bob.ptt.stats().spurt_refused, 1);
}

/// [#233 inceleme 1, tur 2] İLK İKİ datagram kaybolur: varsayılan kuralda (K = 3, her 4. demet)
/// üçüncü demet de açılış bilgisini taşıdığından akışı açar; ses o demetten başlar, `END` akışı
/// bitirir. Yalnız ilk K = 2 demet açılış taşısaydı (`set_spurt_open_every(0)`) bu kayıp basışın
/// tamamını yetim bırakırdı: aynı senaryo orada hiç olay vermez (karşılaştırma kolu aşağıda);
/// K = 3 kurtarır.
#[test]
fn coalesced_open_survives_losing_the_first_two_datagrams() {
    let mut duo = coalesced_duo();
    let (stream, events, sizes) = coalesced_spurt(&mut duo, 5, &[0, 1]);
    let (open, compact) = (7 + TEST_FRAME_BYTES, 2 + TEST_FRAME_BYTES);
    assert_eq!(
        sizes,
        vec![open, open, open, compact, open],
        "0, 1, 2 ve 4. demet"
    );
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((2..5).map(|n| audio(ALICE, stream, &frame(n))));
    want.push(ended(ALICE, stream));
    assert_eq!(events, want);
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 0);

    // Karşılaştırma: yalnız ilk K demet açılış taşır (eski kural). K = 2 → basış kaybolur.
    for (k, opens) in [(2u8, false), (3u8, true)] {
        let mut duo = coalesced_duo();
        duo.alice.ptt.set_spurt_open_every(0);
        duo.alice.ptt.set_spurt_open_repeat(k);
        let (_, events, sizes) = coalesced_spurt(&mut duo, 5, &[0, 1]);
        let open_carrying = sizes.iter().filter(|&&n| n == 7 + TEST_FRAME_BYTES).count();
        assert_eq!(open_carrying, usize::from(k), "K = {k}");
        assert_eq!(!events.is_empty(), opens, "K = {k}: {events:?}");
        if !opens {
            assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 3);
        }
    }
}

/// [#233 inceleme · küçük] Sessizlik sonu sınırı: araya iki kayıp girdiğinde iki kabul arasındaki
/// en kötü boşluk `2 × B + 1 s + 100 ms`'dir (önceki demet en erken, sonraki son tarihin ve
/// alıcının delik beklemesinin sonunda kabul edilir). Bu boşluktan 1 ns kısa bekleyişte akış
/// bitmez ve gelen demet sesi taşır. Eski türetme (1 s + 2 × B = 1,04 s) burada akışı erken
/// bitirip kalan demetleri yetim bırakıyordu.
#[test]
fn coalesced_stream_survives_the_worst_two_loss_acceptance_gap() {
    let mut duo = coalesced_duo();
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    let mut units = Vec::new();
    for k in 0..4u8 {
        duo.alice.send_bundle(stream, &frame(k)).expect("demet");
        units.push(duo.take_inbox(Side::Bob));
    }
    duo.deliver_late(Side::Bob, units[0].clone());
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS),
            audio(ALICE, stream, &frame(0))
        ]
    );
    let worst = Duration::from_millis(2 * u64::from(TEST_BUNDLE_MS) + 1_100);
    assert_eq!(kama_app::ptt::spurt_gap_end(TEST_BUNDLE_MS), worst);
    duo.clock.advance(worst - Duration::from_nanos(1));
    assert!(duo.bob.drain().is_empty(), "sınırdan önce bitmez");
    duo.deliver_late(Side::Bob, units[3].clone());
    assert_eq!(duo.bob.drain(), vec![audio(ALICE, stream, &frame(3))]);
    let stats = duo.bob.ptt.stats();
    assert_eq!(
        (stats.streams_ended_by_gap, stats.ptt_orphan_bundles),
        (0, 0)
    );
}

/// [#233 inceleme 3] Oynatma tamponu açıkken `END`'li demetin sesi tamponu 256 demette taşırır:
/// tampon yaşam döngüsünü yerel sonlandırır (`PttEnded` bir kez) ve Ptt'nin aynı kayıttan
/// `pending`'e koyduğu `PttEnded` yutulur. Eskiden `retain_terminated` işareti Ptt akışı
/// `incoming`'den sildiği için temizliyor ve ikinci `PttEnded` uygulamaya çıkıyordu.
#[test]
fn coalesced_end_overflowing_the_playout_queue_ends_the_stream_once() {
    use kama_app::playout::{PlayoutConfig, PLAYOUT_MAX_BUNDLES_PER_STREAM};
    let (mut duo, _) = Duo::connected();
    duo.bob.ptt.set_spurt_coalesce(true);
    duo.bob.ptt.set_playout(Some(PlayoutConfig::default()));
    let s: StreamId = 40;
    let lyra = PttCodec::Lyra.code();
    let mut events = Vec::new();
    for k in 0..PLAYOUT_MAX_BUNDLES_PER_STREAM {
        duo.inject(
            Side::Bob,
            record(&AppMessage::MediaBundleOpen {
                stream_id: s,
                codec: lyra,
                bundle_ms: TEST_BUNDLE_MS,
                end: false,
                payload: frame(u8::try_from(k % 200).expect("u8")),
            }),
        );
        events.extend(duo.bob.drain());
    }
    duo.inject(
        Side::Bob,
        record(&AppMessage::MediaBundleOpen {
            stream_id: s,
            codec: lyra,
            bundle_ms: TEST_BUNDLE_MS,
            end: true,
            payload: frame(1),
        }),
    );
    for _ in 0..4 {
        events.extend(duo.bob.drain());
    }
    let ends = events.iter().filter(|e| **e == ended(ALICE, s)).count();
    assert_eq!(ends, 1, "tek PttEnded: {:?}", ptt_only(&events).last());
    assert_lifecycles(&events);
    let stats = duo.bob.ptt.playout_stats().expect("tampon açık");
    assert_eq!(stats.playout_overflow, 1);
}

/// [Sahip kararı 2026-11-23 · ilk temas] Yetenek kaynakları: (1) yönetilen bilgi, (2) önceki
/// oturumdan öğrenilen — yalnız aynı eş ve aynı biçim dönemi, (3) bu oturumdaki bant içi
/// bildirim. Bilinmeyen eşe bugünkü uyumlu ayrı kayıtlar gider. Bant içi "çözmüyorum" (0x16)
/// öğrenilmiş kaydı siler ve o oturumda yönetilen bilgiyi de geçersiz kılar.
#[test]
fn spurt_capability_sources_provisioned_learned_and_in_band() {
    let open_types = |duo: &mut Duo| -> Vec<MsgType> {
        let before = duo.sent(Side::Alice).len();
        let stream = duo
            .alice
            .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
            .expect("açılır");
        duo.alice.send_bundle(stream, &frame(0)).expect("demet");
        duo.alice.close(stream).expect("kapanır");
        let _ = duo.bob.drain();
        duo.sent(Side::Alice)[before..]
            .iter()
            .map(|s| s.0)
            .collect()
    };
    let separate = vec![MsgType::PttOpen, MsgType::MediaBundle, MsgType::PttClose];
    let coalesced = vec![MsgType::MediaBundle, MsgType::MediaBundle];
    let (mut duo, h) = Duo::connected();
    duo.alice.ptt.set_spurt_coalesce(true);
    duo.bob.ptt.set_spurt_coalesce(true);
    // Bilinmiyor → ayrı kayıtlar.
    assert_eq!(open_types(&mut duo), separate);
    // Öğrenilmiş (aynı eş, aynı dönem) → birleşik.
    assert!(duo
        .alice
        .ptt
        .remember_spurt_capability(SpurtCapabilityRecord {
            peer: BOB,
            epoch: SPURT_FORMAT_EPOCH,
        }));
    assert_eq!(open_types(&mut duo), coalesced);
    // Başka eşin öğrenilmiş kaydı BOB için kullanılmaz.
    let (mut other, _) = Duo::connected();
    other.alice.ptt.set_spurt_coalesce(true);
    assert!(other
        .alice
        .ptt
        .remember_spurt_capability(SpurtCapabilityRecord {
            peer: CAROL,
            epoch: SPURT_FORMAT_EPOCH,
        }));
    assert_eq!(open_types(&mut other), separate);
    // Bant içi "çözmüyorum" bu oturumda öğrenilmiş ve yönetilen bilgiyi geçersiz kılar.
    assert!(duo.alice.ptt.set_peer_spurt_provisioned(BOB, true));
    assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, h, false));
    assert_eq!(open_types(&mut duo), separate);
    assert!(duo.alice.ptt.learned_spurt_capabilities().is_empty());
    // Bant içi "çözerim" öğrenilir (kalıcılaştırılacak kayıt) ve kullanılır.
    assert!(duo.alice.ptt.set_peer_spurt_provisioned(BOB, false));
    assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, h, true));
    assert_eq!(
        duo.alice.ptt.learned_spurt_capabilities(),
        vec![SpurtCapabilityRecord {
            peer: BOB,
            epoch: SPURT_FORMAT_EPOCH
        }]
    );
    assert_eq!(open_types(&mut duo), coalesced);
}

/// [#233 inceleme · küçük · D5] Oturum değişince yetenek yeniden sınanır: yeni oturumda eş
/// "çözmüyorum" dediyse açık birleşik akış o oturumdan itibaren ayrı kayıtlara döner (güvenilir
/// `PTT_OPEN`, demet, `PTT_CLOSE`); eskiden ilk K demeti yine birleşik gönderiyordu.
#[test]
fn coalesced_stream_rechecks_the_capability_after_a_session_change() {
    let (mut duo, old) = Duo::connected();
    duo.alice.ptt.set_spurt_coalesce(true);
    duo.bob.ptt.set_spurt_coalesce(true);
    assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, old, true));
    let stream = duo
        .alice
        .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
        .expect("açılır");
    duo.alice.send_bundle(stream, &frame(0)).expect("demet");
    assert_eq!(duo.sent_on(Side::Alice, old), vec![MsgType::MediaBundle]);
    let new = duo.link.borrow_mut().supersede();
    let _ = duo.alice.drain();
    assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, new, false));
    duo.alice.send_bundle(stream, &frame(1)).expect("demet");
    duo.alice.close(stream).expect("kapanır");
    assert_eq!(
        duo.sent_on(Side::Alice, new),
        vec![MsgType::PttOpen, MsgType::MediaBundle, MsgType::PttClose]
    );
}

/// [#233 inceleme · küçük] `END` biçimler arası kapalıdır: ayrı `PTT_OPEN`'la açılmış akışı
/// `END`'li demet bitirmez (sesi teslim edilir, işaret sayılıp yok sayılır).
#[test]
fn end_marker_does_not_close_a_separately_opened_stream() {
    let (mut duo, _) = Duo::connected();
    duo.bob.ptt.set_spurt_coalesce(true);
    let s: StreamId = 7;
    duo.inject(Side::Bob, open_record(s, PttCodec::Lyra, TEST_BUNDLE_MS));
    duo.inject(
        Side::Bob,
        record(&AppMessage::MediaBundleEnd {
            stream_tag: kama_app::codec::compact_stream_tag(s),
            payload: frame(2),
        }),
    );
    assert_eq!(
        duo.bob.drain(),
        vec![
            started(ALICE, s, PttCodec::Lyra, TEST_BUNDLE_MS),
            audio(ALICE, s, &frame(2))
        ]
    );
    assert_eq!(duo.bob.ptt.incoming_streams(ALICE), vec![s]);
    assert_eq!(duo.bob.ptt.stats().spurt_refused, 1);
    duo.inject(Side::Bob, close_record(s));
    assert_eq!(duo.bob.drain(), vec![ended(ALICE, s)]);
}

/// [#233 inceleme 7087771388 · 1, tur 2] Basış ortasında `spurt_gap_end`'den uzun kesinti (20 ms
/// demette 60 demet = 1,2 s > 1,14 s): akış sessizlik sonuyla biter; kesintiden sonra ilk
/// açılışlı demet (her 4. demet: 64.) akışı yeniden açar ve sesin kalanı teslim edilir; aradaki
/// kompakt demet (63.) yetimdir — en çok üç demet. Yalnız ilk K demet açılış taşısaydı
/// (`set_spurt_open_every(0)`) kesinti sonrası bütün demetler yetim kalırdı (karşılaştırma kolu).
/// Bedel: kesintili basış iki yaşam döngüsüdür.
#[test]
fn coalesced_stream_resumes_after_an_outage_longer_than_the_silence_end() {
    let lost: Vec<u8> = (3..=62).collect();
    let mut duo = coalesced_duo();
    let (stream, events, _) = coalesced_spurt(&mut duo, 70, &lost);
    let mut want = vec![started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS)];
    want.extend((0..3).map(|n| audio(ALICE, stream, &frame(n))));
    want.push(ended(ALICE, stream));
    want.push(started(ALICE, stream, PttCodec::Lyra, TEST_BUNDLE_MS));
    want.extend((64..70).map(|n| audio(ALICE, stream, &frame(n))));
    want.push(ended(ALICE, stream));
    assert_eq!(events, want);
    let stats = duo.bob.ptt.stats();
    assert_eq!(stats.ptt_orphan_bundles, 1, "yalnız 63. demet");
    assert_eq!((stats.streams_ended_by_gap, stats.spurt_ends), (1, 1));

    let mut duo = coalesced_duo();
    duo.alice.ptt.set_spurt_open_every(0);
    let (_, events, _) = coalesced_spurt(&mut duo, 70, &lost);
    assert!(
        !events
            .iter()
            .any(|e| *e == audio(ALICE, stream, &frame(63))),
        "eski kural: kesinti sonrası ses yetim"
    );
    assert_eq!(duo.bob.ptt.stats().ptt_orphan_bundles, 7);
}

// ── [PTT-BİRLEŞİK · pilot · #233] 300 ms, doğrulanmış eş ─────────────────────────────────────

/// Bob'a giden birimler: (bayt, teslim politikası), gönderim sırasıyla.
type Wire = Vec<(Vec<u8>, DeliveryPolicy)>;

/// Pilot basışı: `n` demet (sonuncusu `send_last_bundle_from`), sonra `close`. `via_open` →
/// bugünkü [`Ptt::open`] çağıranın süresiyle; değilse [`Ptt::open_press`] (çağıranın süresi
/// [`TEST_BUNDLE_MS`]). Demetlerin ses başlangıcı `audio_start` sabittir: son tarihler koşular
/// arasında eşit olur, politika da bayt bayt kıyaslanır. Döner: `stream_id`, akışın kodu, tel.
fn pilot_press(
    duo: &mut Duo,
    n: u8,
    audio_start: std::time::Instant,
    via_open: bool,
) -> (StreamId, u8, Wire) {
    let (stream, code) = if via_open {
        let stream = duo
            .alice
            .open(BOB, PttCodec::Lyra, TEST_BUNDLE_MS)
            .expect("açılır");
        (stream, TEST_BUNDLE_MS)
    } else {
        duo.alice
            .ptt
            .open_press(
                &mut duo.alice.messenger,
                BOB,
                PttCodec::Lyra,
                TEST_BUNDLE_MS,
            )
            .expect("açılır")
    };
    for k in 0..n {
        let ptt = &mut duo.alice.ptt;
        let messenger = &mut duo.alice.messenger;
        if k + 1 == n {
            ptt.send_last_bundle_from(messenger, stream, &frame(k), audio_start)
        } else {
            ptt.send_bundle_from(messenger, stream, &frame(k), audio_start)
        }
        .expect("demet");
    }
    duo.alice.close(stream).expect("kapanır");
    let units: Vec<Vec<u8>> = duo
        .take_inbox(Side::Bob)
        .into_iter()
        .map(|d| d.unit)
        .collect();
    let policies: Vec<DeliveryPolicy> = duo.sent(Side::Alice).iter().map(|s| s.1).collect();
    assert_eq!(units.len(), policies.len());
    (stream, code, units.into_iter().zip(policies).collect())
}

/// [PTT-BİRLEŞİK · pilot] Bayrak açık ve eşin yeteneği DOĞRULANMIŞ (yönetilen, aynı dönemle
/// öğrenilmiş ya da bu oturumda bant içi 0x17) → `open_press` çağıranın süresini (20 ms) değil
/// pilotun 300 ms'sini seçer; tele ayrı `PTT_OPEN` / `PTT_CLOSE` çıkmaz, açılışsız demetler kompakt
/// (2 B) ve Bob akışı 300 ms koduyla açar. Doğrulanmamış eşte (yetenek bilinmiyor, bant içi 0x16)
/// çağıranın süresi ve ayrı güvenilir `PTT_OPEN`.
#[test]
fn pilot_opens_a_verified_peer_at_300_ms_with_the_compact_header() {
    assert_eq!(SPURT_PILOT_BUNDLE_MS, 300);
    assert_eq!(bundle_ms_from_code(SPURT_PILOT_BUNDLE_CODE), 300);
    for source in ["yönetilen", "öğrenilmiş", "bant içi 0x17"] {
        let (mut duo, handle) = Duo::connected();
        duo.alice.ptt.set_spurt_coalesce(true);
        duo.bob.ptt.set_spurt_coalesce(true);
        match source {
            "yönetilen" => assert!(duo.alice.ptt.set_peer_spurt_provisioned(BOB, true)),
            "öğrenilmiş" => {
                assert!(duo
                    .alice
                    .ptt
                    .remember_spurt_capability(SpurtCapabilityRecord {
                        peer: BOB,
                        epoch: SPURT_FORMAT_EPOCH,
                    }))
            }
            _ => assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, handle, true)),
        }
        let (stream, code, wire) = pilot_press(&mut duo, 4, std::time::Instant::now(), false);
        assert_eq!(code, SPURT_PILOT_BUNDLE_CODE, "{source}: pilot süresi");
        let msgs: Vec<AppMessage> = wire
            .iter()
            .map(|(u, _)| decode(u, &limits()).expect("çözülür"))
            .collect();
        assert_eq!(msgs.len(), 4, "{source}: yalnız dört demet");
        for m in &msgs[..3] {
            assert!(
                matches!(m, AppMessage::MediaBundleOpen { bundle_ms, .. } if *bundle_ms == SPURT_PILOT_BUNDLE_CODE),
                "{source}: ilk üç demet açılışı 300 ms koduyla taşır: {m:?}"
            );
        }
        assert!(
            matches!(msgs[3], AppMessage::MediaBundleEnd { .. }),
            "{source}: son demet kompakt END"
        );
        assert_eq!(
            wire[3].0.len(),
            wire[2].0.len() - 5,
            "{source}: kompakt başlık 2 B"
        );
        for (unit, _) in wire {
            duo.inject(Side::Bob, unit);
        }
        let events = duo.bob.drain();
        assert_eq!(
            events.first(),
            Some(&started(
                ALICE,
                stream,
                PttCodec::Lyra,
                SPURT_PILOT_BUNDLE_CODE
            )),
            "{source}"
        );
        assert_eq!(events.last(), Some(&ended(ALICE, stream)), "{source}");
    }
    for source in ["yetenek bilinmiyor", "bant içi 0x16"] {
        let (mut duo, handle) = Duo::connected();
        duo.alice.ptt.set_spurt_coalesce(true);
        duo.bob.ptt.set_spurt_coalesce(true);
        if source == "bant içi 0x16" {
            assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, handle, false));
        }
        let (stream, code, wire) = pilot_press(&mut duo, 4, std::time::Instant::now(), false);
        assert_eq!(code, TEST_BUNDLE_MS, "{source}: çağıranın süresi");
        assert_eq!(
            wire[0].0,
            open_record(stream, PttCodec::Lyra, TEST_BUNDLE_MS),
            "{source}: ayrı PTT_OPEN"
        );
    }
}

/// [PTT-BİRLEŞİK · pilot · ret koşulu] Doğrulanmamış eş ve bayrak kapalı yol bugünküyle BAYT-AYNI:
/// bugünkü `open` (bayrak kapalı) basışının tel birimleri ve teslim politikaları; `open_press` ile
/// (a) bayrak kapalı, (b) bayrak açık ve yetenek bilinmiyor, (c) bayrak açık ve eş bant içinde
/// çözmediğini bildirmiş (0x16) kollarında birebir aynıdır. Taşıma katmanının CAPS çerçevesi
/// (bit 2) bu kıyasın dışındadır: o PTT datagramı değildir.
#[test]
fn pilot_toward_an_unknown_or_refusing_peer_is_byte_identical_to_the_flag_off_path() {
    let audio_start = std::time::Instant::now();
    let reference = {
        let (mut duo, _) = Duo::connected();
        pilot_press(&mut duo, 4, audio_start, true)
    };
    let types: Vec<MsgType> = reference
        .2
        .iter()
        .map(|(u, _)| decode(u, &limits()).expect("çözülür").msg_type())
        .collect();
    assert_eq!(
        types,
        vec![
            MsgType::PttOpen,
            MsgType::MediaBundle,
            MsgType::MediaBundle,
            MsgType::MediaBundle,
            MsgType::MediaBundle,
            MsgType::PttClose
        ]
    );
    for (arm, flag, in_band) in [
        ("bayrak kapalı, open_press", false, None),
        ("bayrak açık, yetenek bilinmiyor", true, None),
        ("bayrak açık, bant içi 0x16", true, Some(false)),
    ] {
        let (mut duo, handle) = Duo::connected();
        duo.alice.ptt.set_spurt_coalesce(flag);
        duo.bob.ptt.set_spurt_coalesce(flag);
        if let Some(v) = in_band {
            assert!(duo.alice.ptt.on_peer_spurt_capability(BOB, handle, v));
        }
        let got = pilot_press(&mut duo, 4, audio_start, false);
        assert_eq!(got, reference, "{arm}: tel bayt-aynı olmalı");
    }
}
