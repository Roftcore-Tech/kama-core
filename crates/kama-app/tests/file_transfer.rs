//! B2.3 / K4G-09 — dosya aktarımı (`kama_app::file_transfer`) uçtan uca.
//!
//! Beklenen davranışın kaynağı: AMP §3/§4 (`FILE_*`, `MAX_FILE_CHUNK_BYTES`; K4G-09 yaması
//! 2026-10-12: 0x0B `FILE_RECEIPT`, `FILE_IDLE_TTL` / `FILE_RECEIPT_TTL`, gönderen-tek `FILE_ABORT`,
//! oturum değişiminde `FILE_ABORT(superseded)`), plan D5 (`transfer_id` kalıcı) ve D6, S7
//! (`MAX_REASSEMBLY_BYTES` = 64 MiB), S8 (vektörsüz alıcı kararı → fail-closed DROP), BL B2.3 Done
//! (`hash(kaynak) == hash(hedef)`; Application'da retransmit yolu yok — H-APP-RETRY-0). Dosya motoru
//! `Messenger::poll`'u okumaz: her uçta testin döngüsü motorun olaylarını `FileTransfer::handle`'a
//! verir (B2.4'te bu yeri `Ptt::poll` alır), sonra `pump` ve `poll_event`. İki düzenek:
//!
//! * **`Link` (iki uç):** `tests/text_receipt.rs`'teki çiftin dosya için genişletilmiş kopyası.
//!   Transport'un içinde ilk iletimi "kaybolup" sonra kurtarılan birimler (`hold_sends` +
//!   `recover`), sırasızlık (`reverse_inbox_after`), yoldaki birimi alıp sonra geç verme
//!   (`take_inbox` + `deliver_late` — eski oturumun yalnız-alım drain'inin karşılığı), yolda bozma
//!   (`map_inbox`), tek atımlık gönderim reddi (`refuse_next_sends`) ve oturum değişimi testin
//!   elindedir — bu, S1 §4'ün aynı-oturum sıra garantisinden SERTTİR. Uç başına Transport'un kabul
//!   ettiği gönderimler sayılır (`accepted`) ve tipleriyle kaydedilir (`sent_types`, `sent_on`):
//!   Application'ın kabul edilmiş bir kaydı yeniden vermediğinin ve alıcının `FILE_ABORT`
//!   göndermediğinin motordan bağımsız kanıtı. Zaman, iki ucun paylaştığı elle ilerletilen bir
//!   saattir (gerçek uyku yok).
//! * **`MockTransport` (yansıtıcı, `tests/common/mock_transport.rs`):** kayıp + kurtarma anahtarıyla
//!   (H-APP-RETRY-0) yankı eşi: BOB'a gönderilen dosya aynı motora BOB'dan gelmiş gibi döner;
//!   giden ve gelen yarı aynı (`PeerRef`, `transfer_id`) için ayrı durumlardır ve alıcı yarının
//!   `FILE_RECEIPT`'i gönderen yarıya döner.
//!
//! # Testlerin `d3bc7407`'deki (K4G-09 öncesi main) durumu — üç sınıf
//!
//! Düzenek `FileTransfer::new(saat)` ve 0x0B'yi kullandığı için dosya eski kodda DERLENMEZ. Her
//! testin belgesi, test MANTIĞININ eski koda göre sınıfını yazar:
//!
//! * **KIRMIZI (uyarlansa da)** — eski API'ye uyarlanınca da bir doğrulama düşer;
//! * **UYARLANAMAZ** — sınadığı yüzey eski kodda yok (ör. `FILE_RECEIPT`, `FILE_IDLE_TTL`,
//!   `FileSendError::Store`);
//! * **AYNI DAVRANIŞ** — eski kodda da geçerdi; regresyon kilidi.

mod common;

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::mock_transport::MockTransport;
use kama_app::clock::MonotonicClock;
use kama_app::codec::limits::{FILE_NAME_MAX_BYTES, WHOLE_FILE_HASH_BYTES};
use kama_app::codec::{
    decode, encode, AmpLimits, AppMessage, EncodeError, FileAbortReason, FileReceiptStatus, MsgType,
};
use kama_app::file_transfer::{
    whole_file_hash, FileEvent, FileFailure, FileSendError, FileTransfer, SendFailure, TransferId,
    FILE_IDLE_TTL, FILE_RECEIPT_TTL, FIRST_TRANSFER_ID, MAX_OUTGOING_TRANSFERS,
    MAX_REASSEMBLY_BYTES,
};
use kama_app::messaging::{Messenger, MessengerEvent, MsgId};
use kama_app::store::{
    AppStore, AppStoreError, CounterKind, DedupWindow, MemoryAppStore, OutboxUnit,
};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport, PreparedUnit,
    SessionEvent, TransportCapabilities, TransportError,
};

/// Test parametresi (normatif değil): codec vektörlerinin ve `text_receipt.rs`'in değeri.
const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Çiftin birim tavanı (`max_plaintext_unit_bytes`, `usize`).
const LINK_MAX_UNIT_BYTES: usize = TEST_MAX_PLAINTEXT_UNIT_BYTES as usize;
/// Yön başına Transport kuyruğu tavanı: teslim kuyruğu + kurtarılmayı bekleyenler (aşımı
/// `Backpressure`).
const LINK_QUEUE_CAP: usize = 64;
/// Uç başına olay kuyruğu tavanı (sahte sınır).
const LINK_EVENT_CAP: usize = 16;
/// Uç başına gönderim kaydı tavanı (sahte sınır).
const LINK_LOG_CAP: usize = 512;
/// Bir turda okunan motor olayı tavanı.
const DRAIN_CAP: usize = 1024;
/// `Duo::settle` tur tavanı.
const ROUND_CAP: usize = 256;
/// "Zaman geçse de" tur sayısı (H-APP-RETRY-0).
const IDLE_ROUNDS: usize = 16;
/// Zamanlayıcı sınırının bir adım altı.
const TTL_MARGIN: Duration = Duration::from_secs(1);
/// Örnek dosyanın tam parça sayısı ve son (kısa) parçasının uzunluğu (`< MAX_FILE_CHUNK_BYTES`).
const FULL_CHUNKS: usize = 6;
const TAIL_BYTES: usize = 123;
/// Örnek dosya adı (çok baytlı UTF-8).
const NAME: &str = "rapor-ş.pdf";
/// Elle kurulan kayıtların kimliği ve geometrisi: 10 B, `L = 6` → parçalar `[0,6)` ve `[6,10)`.
const INJECTED_ID: TransferId = 7;
const SMALL_SIZE: usize = 10;
const SMALL_CHUNK: usize = 6;

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

/// Konuma bağlı içerik: kayan ya da yer değiştiren parça özette görünür.
fn sample_file(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn chunk(transfer_id: TransferId, offset: usize, payload: &[u8]) -> AppMessage {
    AppMessage::FileChunk {
        transfer_id,
        offset: u64::try_from(offset).expect("küçük konum"),
        payload: payload.to_vec(),
    }
}

fn file_receipt(transfer_id: TransferId, status: FileReceiptStatus) -> AppMessage {
    AppMessage::FileReceipt {
        transfer_id,
        status,
    }
}

fn received(peer: PeerRef, transfer_id: TransferId, bytes: Vec<u8>) -> FileEvent {
    FileEvent::Received {
        peer,
        transfer_id,
        name: NAME.to_owned(),
        bytes,
    }
}

fn sent(peer: PeerRef, transfer_id: TransferId) -> FileEvent {
    FileEvent::Sent { peer, transfer_id }
}

fn delivered(peer: PeerRef, transfer_id: TransferId) -> FileEvent {
    FileEvent::Delivered { peer, transfer_id }
}

fn send_failed(peer: PeerRef, transfer_id: TransferId, reason: SendFailure) -> FileEvent {
    FileEvent::SendFailed {
        peer,
        transfer_id,
        reason,
    }
}

/// Elle kurulan küçük aktarımın kayıtları: `FILE_BEGIN`, parça 0, parça 1, `FILE_END`.
fn small_transfer(content: &[u8], hash: [u8; WHOLE_FILE_HASH_BYTES]) -> [AppMessage; 4] {
    [
        AppMessage::FileBegin {
            transfer_id: INJECTED_ID,
            file_size: u64::try_from(content.len()).expect("küçük boy"),
            whole_file_hash: hash,
            chunk_count: 2,
            name: NAME.to_owned(),
        },
        chunk(INJECTED_ID, 0, &content[..SMALL_CHUNK]),
        chunk(INJECTED_ID, SMALL_CHUNK, &content[SMALL_CHUNK..]),
        AppMessage::FileEnd {
            transfer_id: INJECTED_ID,
        },
    ]
}

fn is_file_record(message: &AppMessage) -> bool {
    matches!(
        message.msg_type(),
        MsgType::FileBegin
            | MsgType::FileChunk
            | MsgType::FileEnd
            | MsgType::FileAbort
            | MsgType::FileReceipt
    )
}

/// Bir ucun turu: motorun bütün olaylarını dosya motoruna verir (`FILE_*` tüketilir, gerisi aynen
/// geri döner), sonra `pump` (zamanlayıcılar + giden kuyruk). Dosya olaylarını doğuş sırasıyla
/// döndürür.
fn turn<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    files: &mut FileTransfer,
) -> Vec<FileEvent> {
    let mut polled = 0;
    while let Some(event) = messenger.poll() {
        polled += 1;
        assert!(polled <= DRAIN_CAP, "olay akışı bitmiyor");
        if let Some(MessengerEvent::Other { message, .. }) = files.handle(messenger, event) {
            assert!(!is_file_record(&message), "FILE_* kaydı geri verilmemeli");
        }
    }
    files.pump(messenger);
    std::iter::from_fn(|| files.poll_event()).collect()
}

/// Taze bellek içi depo — kalıcı durumu olmayan bir cihaz.
fn fresh_store() -> Arc<dyn AppStore> {
    Arc::new(MemoryAppStore::new())
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
}

/// Bir ucun Transport durumu.
#[derive(Default)]
struct Endpoint {
    /// Eşten gelmiş, motorun henüz okumadığı birimler.
    inbox: VecDeque<Delivered>,
    /// Bu uca giden, ilk iletimi kaybolmuş ve Transport'un kendi içinde kurtaracağı birimler
    /// (`inbox` ile birlikte ≤ `LINK_QUEUE_CAP`).
    held: VecDeque<Delivered>,
    /// Bu uca sıralanmış oturum olayları (≤ `LINK_EVENT_CAP`).
    events: VecDeque<SessionEvent>,
    /// Gönderim kabul eden tutamaç.
    active: Option<SessionHandle>,
    /// Supersede edilmiş, `Closed`'ı bu uçta henüz okunmamış tutamaçlar.
    superseded: Vec<SessionHandle>,
    /// Bu uçtan KABUL edilen gönderimler (ilk iletimi kaybolanlar dahil).
    accepted: u64,
    /// Bu uçtan kabul edilen gönderimlerin tutamacı ve tipi, sırasıyla (≤ `LINK_LOG_CAP`).
    sent: Vec<(SessionHandle, MsgType)>,
    /// Bu ucun motoruna teslim edilen birimler.
    delivered: u64,
    /// İlk iletimi kaybolacak gönderimlerin kabul sıraları (0'dan).
    hold: BTreeSet<u64>,
    /// Tek atımlık düğme: bu uçtan, oturum ve boy denetimlerini geçen sıradaki bu kadar gönderim
    /// `Backpressure` ile reddedilir — kuyruk boş olsa da. Reddedilen gönderim kaydedilmez.
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

    /// `side` ucunun kabul sırası `ordinals` olan gönderimlerinin ilk iletimi kaybolur.
    fn hold_sends(&mut self, side: Side, ordinals: &[u64]) {
        self.endpoint(side).hold.extend(ordinals.iter().copied());
    }

    /// Transport `side` ucuna giden kayıp birimleri kurtarır: SONRA gönderilenlerin ARKASINDAN
    /// teslim edilirler.
    fn recover(&mut self, side: Side) {
        let endpoint = self.endpoint(side);
        endpoint.inbox.append(&mut endpoint.held);
    }

    /// Sırasızlık: `side` ucunun teslim kuyruğunda ilk `keep` birim yerinde kalır, kalanı ters
    /// sıraya döner.
    fn reverse_inbox_after(&mut self, side: Side, keep: usize) {
        let inbox = &mut self.endpoint(side).inbox;
        let tail = inbox.split_off(keep.min(inbox.len()));
        inbox.extend(tail.into_iter().rev());
    }

    /// `to` ucuna, aktif oturumu üzerinden eşten gelmiş gibi ham kayıt koyar.
    fn inject(&mut self, to: Side, unit: Vec<u8>) {
        let endpoint = self.endpoint(to);
        let session = endpoint.active.expect("inject: aktif oturum yok");
        assert!(endpoint.inbox.len() < LINK_QUEUE_CAP, "inject: kuyruk dolu");
        endpoint.inbox.push_back(Delivered { session, unit });
    }

    /// Durulma ölçüsü: iki uçtan kabul edilen gönderimler + iki uca teslim edilen birimler.
    fn activity(&self) -> u64 {
        self.alice.accepted + self.bob.accepted + self.alice.delivered + self.bob.delivered
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
        if me.refuse_next > 0 {
            me.refuse_next -= 1;
            return Err(TransportError::Backpressure);
        }
        if remote.inbox.len() + remote.held.len() >= LINK_QUEUE_CAP {
            return Err(TransportError::Backpressure);
        }
        let message = decode(&unit.bytes, &limits()).expect("motor geçerli kayıt üretir");
        assert!(
            me.sent.len() < LINK_LOG_CAP,
            "senaryo gönderim kaydı tavanını aştı"
        );
        me.sent.push((session, message.msg_type()));
        let delivered = Delivered {
            session,
            unit: unit.bytes,
        };
        if me.hold.remove(&me.accepted) {
            remote.held.push_back(delivered);
        } else {
            remote.inbox.push_back(delivered);
        }
        me.accepted += 1;
        Ok(())
    }

    fn recv(&mut self) -> Option<Delivered> {
        let mut link = self.link.borrow_mut();
        let endpoint = link.endpoint(self.side);
        let unit = endpoint.inbox.pop_front()?;
        endpoint.delivered += 1;
        Some(unit)
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

/// İki uçlu düzenek: Alice'in ve Bob'un motorları, dosya motorları, ortak saat ve testin `Link`
/// denetimi.
struct Duo {
    link: Rc<RefCell<Link>>,
    clock: ManualClock,
    alice: Messenger<End>,
    bob: Messenger<End>,
    alice_files: FileTransfer,
    bob_files: FileTransfer,
}

impl Duo {
    /// Oturum kurulmuş, iki tarafın `Ready` olayı okunmuş düzenek; iki uç da taze depolu.
    fn connected() -> Self {
        Self::connected_with(fresh_store())
    }

    /// [`Duo::connected`]; Alice'in deposu verilir.
    fn connected_with(alice_store: Arc<dyn AppStore>) -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let clock = ManualClock::default();
        let mut duo = Self {
            alice: Messenger::with_store(End::new(&link, Side::Alice), alice_store)
                .expect("limitler geçerli"),
            bob: Messenger::with_store(End::new(&link, Side::Bob), fresh_store())
                .expect("limitler geçerli"),
            alice_files: FileTransfer::new(Arc::new(clock.clone())),
            bob_files: FileTransfer::new(Arc::new(clock.clone())),
            clock,
            link,
        };
        let handle = duo.link.borrow_mut().establish();
        assert!(duo.alice_turn().is_empty());
        assert!(duo.bob_turn().is_empty());
        assert_eq!(duo.alice.session_of(BOB), Some(handle));
        assert_eq!(duo.bob.session_of(ALICE), Some(handle));
        duo
    }

    fn alice_turn(&mut self) -> Vec<FileEvent> {
        turn(&mut self.alice, &mut self.alice_files)
    }

    fn bob_turn(&mut self) -> Vec<FileEvent> {
        turn(&mut self.bob, &mut self.bob_files)
    }

    /// İki uç sırayla döner; bir tur ne dosya olayı ne etkinlik (kabul, teslim) üretmeyince durur.
    /// Kurtarılmayı bekleyen birimler (`held`) yalnız `recover` ile ilerler.
    fn settle(&mut self) -> (Vec<FileEvent>, Vec<FileEvent>) {
        let (mut at_alice, mut at_bob) = (Vec::new(), Vec::new());
        for _ in 0..ROUND_CAP {
            let before = self.link.borrow().activity();
            let alice = self.alice_turn();
            let bob = self.bob_turn();
            let idle =
                alice.is_empty() && bob.is_empty() && self.link.borrow().activity() == before;
            at_alice.extend(alice);
            at_bob.extend(bob);
            if idle {
                return (at_alice, at_bob);
            }
        }
        panic!("düzenek {ROUND_CAP} turda durulmadı");
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

    /// `side` ucunun teslim kuyruğundaki birimleri yoldan alır — motora verilmemiştir, tutamaçları
    /// korunur: test onları sonra [`Duo::deliver_late`] ile verebilir.
    fn take_inbox(&self, side: Side) -> Vec<Delivered> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .inbox
            .drain(..)
            .collect()
    }

    /// Yolda tutulan birimleri `side` ucunun teslim kuyruğunun SONUNA koyar: eski oturumun yalnız-alım
    /// drain'inden geç gelen birimlerin karşılığı (S1 K7).
    fn deliver_late(&self, side: Side, units: Vec<Delivered>) {
        let mut link = self.link.borrow_mut();
        let inbox = &mut link.endpoint(side).inbox;
        assert!(
            inbox.len() + units.len() <= LINK_QUEUE_CAP,
            "deliver_late: kuyruk tavanı"
        );
        inbox.extend(units);
    }

    /// `side` ucunun teslim kuyruğundaki her birime `f`'yi uygular (yolda bozma).
    fn map_inbox(&self, side: Side, f: impl FnMut(&mut Delivered)) {
        self.link
            .borrow_mut()
            .endpoint(side)
            .inbox
            .iter_mut()
            .for_each(f);
    }

    /// `side` ucundan sıradaki `count` gönderim `Backpressure` ile reddedilir (tek atımlık).
    fn refuse_next_sends(&self, side: Side, count: usize) {
        self.link.borrow_mut().endpoint(side).refuse_next = count;
    }

    /// `side` ucundan Transport'un kabul ettiği gönderim sayısı.
    fn accepted(&self, side: Side) -> u64 {
        self.link.borrow_mut().endpoint(side).accepted
    }

    /// `side` ucundan Transport'un kabul ettiği kayıtların tipleri, sırasıyla.
    fn sent_types(&self, side: Side) -> Vec<MsgType> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .map(|&(_, msg_type)| msg_type)
            .collect()
    }

    /// `side` ucundan `handle` tutamacına kabul edilen kayıtların tipleri, sırasıyla.
    fn sent_on(&self, side: Side, handle: SessionHandle) -> Vec<MsgType> {
        self.link
            .borrow_mut()
            .endpoint(side)
            .sent
            .iter()
            .filter(|&&(session, _)| session == handle)
            .map(|&(_, msg_type)| msg_type)
            .collect()
    }
}

// ── yansıtıcı sahte ──────────────────────────────────────────────────────────────────────────

/// Yansıtıcı sahte üzerinde, `store` deposuyla bir uç.
fn echo_node(
    store: Arc<dyn AppStore>,
    clock: &ManualClock,
) -> (Messenger<MockTransport>, FileTransfer) {
    let messenger =
        Messenger::with_store(MockTransport::new(caps()), store).expect("limitler geçerli");
    (messenger, FileTransfer::new(Arc::new(clock.clone())))
}

/// Yeni eşe `connect` → sahte el sıkışmayı tamamlar → olaylar okunur, eşin oturumu hazır.
fn connect_mock(messenger: &mut Messenger<MockTransport>, files: &mut FileTransfer, peer: PeerRef) {
    assert!(matches!(
        messenger.connect(peer),
        Ok(ConnectResult::Pending(_))
    ));
    messenger.transport_mut().complete_connect(&peer);
    assert!(turn(messenger, files).is_empty());
    assert!(messenger.session_of(peer).is_some());
}

// ── teslim ve alındı ─────────────────────────────────────────────────────────────────────────

/// Kayıp, sırasızlık ve parçalardan önce gelen `FILE_END` altında dosya birleşir; alıcı TAM BİR
/// `FILE_RECEIPT(ok)` gönderir (parça ACK'i yok) ve gönderen `Delivered` görür.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): alıcı hiçbir şey göndermiyordu (`accepted(Bob) == 0`),
/// gönderen `Sent`'ten sonra hiçbir sinyal almazdı (`Delivered` yok).
#[test]
fn file_survives_transport_loss_reordering_and_end_before_chunks() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(FULL_CHUNKS * chunk_len + TAIL_BYTES);
    let records = u64::try_from(1 + FULL_CHUNKS + 1 + 1).expect("küçük sayı");
    // Kabul sırası: 0 = FILE_BEGIN, 1..=7 = parça 0..6, 8 = FILE_END. Parça 1, 2 ve 5'in ilk
    // iletimi kaybolur; Transport onları kendi içinde kurtarır (Reliable).
    duo.link.borrow_mut().hold_sends(Side::Alice, &[2, 3, 6]);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    // `Sent` yalnız son kaydın Transport'a kabulüdür; teslim değildir (S1 K8).
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    // Sırasızlık: FILE_BEGIN önde kalır, kalanı tersine döner — FILE_END parçalardan ÖNCE gelir.
    duo.link.borrow_mut().reverse_inbox_after(Side::Bob, 1);
    assert!(duo.bob_turn().is_empty(), "eksik parça varken tamamlanmaz");
    assert_eq!(duo.bob_files.incoming_count(), 1);
    // Transport kurtardı: kayıp birimler SONRA gönderilenlerin arkasından teslim edilir.
    duo.link.borrow_mut().recover(Side::Bob);
    let events = duo.bob_turn();
    assert_eq!(events, vec![received(ALICE, id, source.clone())]);
    // BL B2.3 Done: hash(kaynak) == hash(hedef).
    let [FileEvent::Received { bytes, .. }] = events.as_slice() else {
        panic!("tek Received bekleniyordu");
    };
    assert_eq!(whole_file_hash(bytes), whole_file_hash(&source));
    assert_eq!(duo.bob_files.incoming_count(), 0);
    // Her kayıt Transport'a tam bir kez kabul ettirildi; alıcı yalnız TEK alındı gönderdi.
    assert_eq!(duo.accepted(Side::Alice), records);
    assert_eq!(duo.alice_files.stats().records_accepted, records);
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
    // Teslim edildi: yalnız `FILE_RECEIPT(ok)` ile.
    assert_eq!(duo.alice_turn(), vec![delivered(BOB, id)]);
    assert_eq!(duo.alice_files.outgoing_count(), 0);
}

/// `d3bc7407`'de KIRMIZI (uyarlansa da): gönderen olayları `[Sent]`'ti (`Delivered` yok).
#[test]
fn backpressure_waits_and_no_accepted_record_is_offered_twice() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    // Transport kuyruğunun üç katı parça: gönderen geri basınçla bekler.
    let chunks = 3 * LINK_QUEUE_CAP;
    let source = sample_file(chunks * chunk_len);
    let records = u64::try_from(chunks + 2).expect("küçük sayı");
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty(), "kuyruk dolunca bekler");
    let queue = u64::try_from(LINK_QUEUE_CAP).expect("küçük sayı");
    assert_eq!(duo.accepted(Side::Alice), queue);
    assert!(duo.alice_files.stats().offers_refused > 0);
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(at_alice, vec![sent(BOB, id), delivered(BOB, id)]);
    assert_eq!(at_bob, vec![received(ALICE, id, source)]);
    assert_eq!(duo.accepted(Side::Alice), records);
    assert_eq!(duo.alice_files.stats().records_accepted, records);
    assert_eq!(duo.alice_files.outgoing_count(), 0);
    // Zaman geçse de, kaç kez sunulursa sunulsun: kabul edilmiş kayıt yeniden verilmez.
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice_turn().is_empty());
    }
    assert_eq!(duo.accepted(Side::Alice), records);
}

/// `d3bc7407`'de KIRMIZI (uyarlansa da): gönderen olayları `[Sent]`'ti, alıcı alındı göndermiyordu.
#[test]
fn empty_file_round_trips_without_chunks() {
    let mut duo = Duo::connected();
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, Vec::new())
        .expect("kuyruğa alınır");
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(at_alice, vec![sent(BOB, id), delivered(BOB, id)]);
    assert_eq!(at_bob, vec![received(ALICE, id, Vec::new())]);
    // FILE_BEGIN + FILE_END; karşıdan tek FILE_RECEIPT.
    assert_eq!(duo.accepted(Side::Alice), 2);
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
}

/// Teslim edildi yalnız `ok`'tur, yalnız `FILE_END` kabul edildikten sonra ve tam bir kez
/// bildirilir; her başka status aktarımı kendi nedeniyle bitirir; erken, tekrar ya da geç alındı yok
/// sayılır.
///
/// `d3bc7407`'de UYARLANAMAZ: `FILE_RECEIPT` (0x0B) kayıtsız tipti (codec DROP), `Delivered` /
/// `SendFailure` yoktu.
#[test]
fn delivered_only_on_receipt_ok_and_every_other_status_fails() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(2 * LINK_QUEUE_CAP * chunk_len);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty(), "aktarım yarım");
    // `ok` `FILE_END`'den önce anlamsızdır: yok sayılır, aktarım sürer.
    let ok = record(&file_receipt(id, FileReceiptStatus::Ok));
    duo.inject(Side::Alice, ok.clone());
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.alice_files.stats().receipts_ignored, 1);
    assert_eq!(duo.alice_files.outgoing_count(), 1);
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(at_alice, vec![sent(BOB, id), delivered(BOB, id)]);
    assert_eq!(at_bob, vec![received(ALICE, id, source)]);
    // Tekrar alındı (ör. yinelenmiş bir sahte kayıt) ikinci bir `Delivered` üretmez.
    duo.inject(Side::Alice, ok);
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.alice_files.stats().receipts_ignored, 2);

    let failures = [
        (
            FileReceiptStatus::AbortedByReceiver,
            SendFailure::AbortedByReceiver,
        ),
        (FileReceiptStatus::HashMismatch, SendFailure::HashMismatch),
        (FileReceiptStatus::Timeout, SendFailure::ReceiverTimeout),
        (FileReceiptStatus::Rejected, SendFailure::Rejected),
    ];
    for (status, reason) in failures {
        let id = duo
            .alice_files
            .send_file(&duo.alice, BOB, NAME, sample_file(SMALL_SIZE))
            .expect("kuyruğa alınır");
        assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
        // Kayıtlar yolda kalır; alındı testin elinden gelir.
        assert_eq!(duo.take_inbox(Side::Bob).len(), 3);
        duo.inject(Side::Alice, record(&file_receipt(id, status)));
        assert_eq!(
            duo.alice_turn(),
            vec![send_failed(BOB, id, reason)],
            "{status:?}"
        );
        assert_eq!(duo.alice_files.outgoing_count(), 0);
    }
}

/// Alıcının kabul ettiği alındıyı Transport geçici reddederse alındı KAYBOLMAZ: bekler ve sonraki
/// `pump`'ta sunulur (ilki tele çıkmamıştı; yeniden iletim değil). İki yönlü aktarımda alıcının kendi
/// parçaları kuyruğu doldurduğunda gönderende yanlış başarısızlık doğmaz.
///
/// `d3bc7407`'de UYARLANAMAZ: alındı yoktu; alıcının tek kaydı (`FILE_ABORT`) tek denemelikti.
#[test]
fn a_receipt_refused_by_the_transport_waits_and_is_not_lost() {
    let mut duo = Duo::connected();
    let source = sample_file(SMALL_SIZE);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    // Bob'un turunda alındı iki kez sunulur (`handle` ve `pump`); ikisi de reddedilir.
    duo.refuse_next_sends(Side::Bob, 2);
    assert_eq!(duo.bob_turn(), vec![received(ALICE, id, source)]);
    assert_eq!(duo.accepted(Side::Bob), 0, "alındı tele çıkmadı");
    assert_eq!(duo.bob_files.stats().offers_refused, 2);
    assert!(duo.alice_turn().is_empty(), "henüz alındı yok");
    assert!(duo.bob_turn().is_empty());
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
    assert_eq!(duo.bob_files.stats().receipts_sent, 1);
    assert_eq!(duo.alice_turn(), vec![delivered(BOB, id)]);
}

#[test]
fn echo_peer_reassembles_through_mock_loss_and_recovery() {
    // `d3bc7407`'de KIRMIZI (uyarlansa da): ikinci tur `[Received]`'tı — alındı yoktu.
    let clock = ManualClock::default();
    let (mut messenger, mut files) = echo_node(fresh_store(), &clock);
    connect_mock(&mut messenger, &mut files, BOB);
    let chunk_len = messenger.limits().max_file_chunk_bytes();
    let source = sample_file(4 * chunk_len + TAIL_BYTES);
    // FILE_BEGIN ile parça 0'ın ilk iletimi kaybolur; sahtenin Transport'u onları kurtarır ve aynı
    // oturumda sırayı koruyarak (arkalarındakileri bekleterek) teslim eder.
    messenger.transport_mut().drop_next_units(2);
    let id = files
        .send_file(&messenger, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(turn(&mut messenger, &mut files), vec![sent(BOB, id)]);
    assert_eq!(
        files.incoming_count(),
        0,
        "kurtarılana kadar hiçbir şey gelmedi"
    );
    messenger.transport_mut().pump();
    // Yansıyan dosya alıcı yarıda birleşir; alıcı yarının alındısı gönderen yarıya döner.
    assert_eq!(
        turn(&mut messenger, &mut files),
        vec![received(BOB, id, source), delivered(BOB, id)]
    );
    let stats = messenger.transport().stats();
    assert_eq!((stats.units_dropped, stats.units_recovered), (2, 2));
    // Yedi kayıt (1 + 5 + 1), her biri bir kez; kurtarma Transport'undur. Alındı ayrıca sayılır.
    assert_eq!(files.stats().records_accepted, 7);
    assert_eq!(files.stats().receipts_sent, 1);
}

// ── zamanlayıcılar ───────────────────────────────────────────────────────────────────────────

/// Kayıp parça yeniden GÖNDERİLMEZ ve kayıp tespit edilmez (H-APP-RETRY-0); yarım aktarım alıcıda
/// `FILE_IDLE_TTL` ile biter: yuva boşalır, `FILE_RECEIPT(timeout)` gider, gönderen kendi
/// `FILE_RECEIPT_TTL`'i dolarken kuyruğunda bekleyen bu alındıyı ÖNCE işler (`ReceiverTimeout`).
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): zamanlayıcı yoktu — yarım aktarım `abandon_incoming`'e
/// dek tabloda kalırdı, alındı yoktu.
#[test]
fn lost_chunk_is_never_resent_and_the_receiver_times_out() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(FULL_CHUNKS * chunk_len + TAIL_BYTES);
    let records = u64::try_from(1 + FULL_CHUNKS + 1 + 1).expect("küçük sayı");
    // Parça 2'nin ilk iletimi kaybolur ve Transport onu hiç kurtarmaz (`recover` yok).
    duo.link.borrow_mut().hold_sends(Side::Alice, &[3]);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(at_alice, vec![sent(BOB, id)]);
    assert!(at_bob.is_empty(), "eksik parçayla dosya teslim edilmez");
    // Zaman geçse de (sınırın bir adım altına dek) Application yeniden göndermez.
    duo.clock.advance(FILE_IDLE_TTL - TTL_MARGIN);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice_turn().is_empty());
        assert!(duo.bob_turn().is_empty());
    }
    assert_eq!(duo.accepted(Side::Alice), records);
    assert_eq!(duo.bob_files.incoming_count(), 1);
    // Sınır: alıcı yuvayı boşaltır ve gönderene `timeout` bildirir.
    duo.clock.advance(TTL_MARGIN);
    assert_eq!(
        duo.bob_turn(),
        vec![FileEvent::Failed {
            peer: ALICE,
            transfer_id: id,
            reason: FileFailure::IdleTimeout,
        }]
    );
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
    // Gönderenin süresi de doldu, ama kuyruktaki alındı önce işlenir.
    assert_eq!(
        duo.alice_turn(),
        vec![send_failed(BOB, id, SendFailure::ReceiverTimeout)]
    );
    assert_eq!(duo.alice_files.stats().receipt_timeouts, 0);
    assert_eq!(duo.accepted(Side::Alice), records);
}

/// AMP §3 K4G-09: `FILE_IDLE_TTL` (normatif 60 s) `FILE_BEGIN` kabul edilince başlar ve kabul edilen
/// her YENİ parçada sıfırlanır; tekrar parça, atılan parça ve `FILE_END` sıfırlamaz. Transport
/// kuyruğunda bekleyen parça süre denetiminden ÖNCE işlenir ve aktarımı canlı tutar. Zaman aşımından
/// sonra gelen kayıtlar bilinmeyen aktarımdır: ikinci alındı yok.
///
/// `d3bc7407`'de UYARLANAMAZ (saat ve `FILE_IDLE_TTL` yoktu). Uyarlansa KIRMIZI: aktarım hiçbir
/// zaman kendiliğinden bitmiyordu.
#[test]
fn receiver_idle_timeout_frees_the_slot_and_a_queued_chunk_keeps_it_alive() {
    assert_eq!(FILE_IDLE_TTL, Duration::from_secs(60), "AMP §3 K4G-09");
    let mut duo = Duo::connected();
    let content = sample_file(SMALL_SIZE);
    let [begin, first, second, end] = small_transfer(&content, whole_file_hash(&content));
    duo.inject(Side::Bob, record(&begin));
    duo.inject(Side::Bob, record(&first));
    assert!(duo.bob_turn().is_empty());
    // Tekrar parça, atılan (geometriye uymayan) parça ve eksik parçayla gelen `FILE_END` süreyi
    // sıfırlamaz.
    duo.clock.advance(FILE_IDLE_TTL - TTL_MARGIN);
    duo.inject(Side::Bob, record(&first));
    duo.inject(
        Side::Bob,
        record(&chunk(INJECTED_ID, 3, &content[3..SMALL_CHUNK])),
    );
    duo.inject(Side::Bob, record(&end));
    assert!(duo.bob_turn().is_empty());
    assert_eq!(duo.bob_files.incoming_count(), 1);
    duo.clock.advance(TTL_MARGIN);
    assert_eq!(
        duo.bob_turn(),
        vec![FileEvent::Failed {
            peer: ALICE,
            transfer_id: INJECTED_ID,
            reason: FileFailure::IdleTimeout,
        }]
    );
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert_eq!(duo.bob_files.stats().transfers_timed_out, 1);
    // Geç parça bilinmeyen aktarımdır: atılır, ikinci alındı yok.
    duo.inject(Side::Bob, record(&second));
    assert!(duo.bob_turn().is_empty());
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![file_receipt(INJECTED_ID, FileReceiptStatus::Timeout)]
    );

    // Kuyrukta bekleyen parça aktarımı canlı tutar: süre, kuyruk işlendikten sonra denetlenir.
    let other = INJECTED_ID + 1;
    let mut records = small_transfer(&content, whole_file_hash(&content));
    for message in &mut records {
        if let AppMessage::FileBegin { transfer_id, .. }
        | AppMessage::FileChunk { transfer_id, .. }
        | AppMessage::FileEnd { transfer_id } = message
        {
            *transfer_id = other;
        }
    }
    let [begin, first, second, _] = records;
    duo.inject(Side::Bob, record(&begin));
    duo.inject(Side::Bob, record(&first));
    assert!(duo.bob_turn().is_empty());
    duo.inject(Side::Bob, record(&second));
    duo.clock.advance(FILE_IDLE_TTL * 2);
    assert!(duo.bob_turn().is_empty(), "bekleyen parça önce işlendi");
    assert_eq!(duo.bob_files.incoming_count(), 1);
    duo.clock.advance(FILE_IDLE_TTL);
    assert_eq!(
        duo.bob_turn(),
        vec![FileEvent::Failed {
            peer: ALICE,
            transfer_id: other,
            reason: FileFailure::IdleTimeout,
        }]
    );
}

/// AMP §3 K4G-09: `FILE_RECEIPT_TTL` (normatif 60 s) `FILE_END` kabul edilince başlar; süre içinde
/// alındı gelmezse aktarım `ReceiptTimeout` ile biter, eşe bir şey gitmez; sonra gelen alındı yok
/// sayılır (gerçekte teslim edilmiş olabilir — beyanlı yanlış "başarısız"). Kuyrukta bekleyen alındı
/// süre denetiminden ÖNCE işlenir.
///
/// `d3bc7407`'de UYARLANAMAZ: alındı ve saat yoktu.
#[test]
fn sender_receipt_timeout_fails_the_transfer_and_a_queued_receipt_wins() {
    assert_eq!(FILE_RECEIPT_TTL, Duration::from_secs(60), "AMP §3 K4G-09");
    let mut duo = Duo::connected();
    let source = sample_file(SMALL_SIZE);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    let in_flight = duo.take_inbox(Side::Bob);
    duo.clock.advance(FILE_RECEIPT_TTL - TTL_MARGIN);
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.alice_files.outgoing_count(), 1);
    duo.clock.advance(TTL_MARGIN);
    assert_eq!(
        duo.alice_turn(),
        vec![send_failed(BOB, id, SendFailure::ReceiptTimeout)]
    );
    assert_eq!(duo.alice_files.outgoing_count(), 0);
    assert_eq!(
        duo.accepted(Side::Alice),
        3,
        "zaman aşımında tele bir şey gitmedi"
    );
    // Kayıtlar sonunda ulaşır; geç `ok` alındısı yok sayılır.
    duo.deliver_late(Side::Bob, in_flight);
    assert_eq!(duo.bob_turn(), vec![received(ALICE, id, source.clone())]);
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.alice_files.stats().receipts_ignored, 1);

    // Süre dolduğunda kuyrukta bekleyen alındı önce işlenir: `Delivered`, zaman aşımı değil.
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    assert_eq!(duo.bob_turn(), vec![received(ALICE, id, source)]);
    duo.clock.advance(FILE_RECEIPT_TTL * 2);
    assert_eq!(duo.alice_turn(), vec![delivered(BOB, id)]);
    assert_eq!(duo.alice_files.stats().receipt_timeouts, 1);
}

// ── alıcı kararları ──────────────────────────────────────────────────────────────────────────

/// `d3bc7407`'de KIRMIZI (uyarlansa da): alıcı karşıya hiçbir şey göndermiyordu (alındı yok).
#[test]
fn duplicate_records_are_idempotent() {
    let mut duo = Duo::connected();
    let content = sample_file(SMALL_SIZE);
    let [begin, first, second, end] = small_transfer(&content, whole_file_hash(&content));
    for message in [&begin, &begin, &first, &first, &end, &end, &second] {
        duo.inject(Side::Bob, record(message));
    }
    assert_eq!(duo.bob_turn(), vec![received(ALICE, INJECTED_ID, content)]);
    assert_eq!(duo.bob_files.stats().duplicate_records, 3);
    // Tamamlanmış aktarımın geç tekrarı bilinmeyen aktarımdır: atılır, olay yok.
    duo.inject(Side::Bob, record(&second));
    assert!(duo.bob_turn().is_empty());
    assert_eq!(duo.bob_files.stats().unknown_transfer_records, 1);
    // Tekrarlar karşıya sinyal üretmedi (parça ACK'i yok): tek sinyal aktarımın alındısı.
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![file_receipt(INJECTED_ID, FileReceiptStatus::Ok)]
    );
}

/// Özet tutmazsa alıcı `FILE_RECEIPT(hash_mismatch)` gönderir, `FILE_ABORT` GÖNDERMEZ; gönderen
/// aktarımı `HashMismatch` ile bitirir. Parça yeniden istemi YOK. (a) elle kurulan kayıtlar;
/// (b) uçtan uca, yolda bir parçanın bir baytı bozulur.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): alıcı `FILE_ABORT(hash_mismatch)` gönderiyordu, gönderen
/// `SendAborted` görüyordu.
#[test]
fn hash_mismatch_is_reported_with_a_receipt_never_an_abort() {
    // (a) Elle kurulan kayıtlar.
    let mut duo = Duo::connected();
    let content = sample_file(SMALL_SIZE);
    let mut wrong = whole_file_hash(&content);
    wrong[0] ^= 0x01;
    for message in small_transfer(&content, wrong) {
        duo.inject(Side::Bob, record(&message));
    }
    assert_eq!(
        duo.bob_turn(),
        vec![FileEvent::Failed {
            peer: ALICE,
            transfer_id: INJECTED_ID,
            reason: FileFailure::HashMismatch,
        }]
    );
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![file_receipt(INJECTED_ID, FileReceiptStatus::HashMismatch)]
    );

    // (b) Uçtan uca: yolda parça 0'ın ilk baytı bozulur.
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(2 * chunk_len + TAIL_BYTES);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    let limits = limits();
    duo.map_inbox(Side::Bob, |unit| {
        if let Ok(AppMessage::FileChunk {
            transfer_id,
            offset: 0,
            mut payload,
        }) = decode(&unit.unit, &limits)
        {
            payload[0] ^= 0x01;
            unit.unit = record(&AppMessage::FileChunk {
                transfer_id,
                offset: 0,
                payload,
            });
        }
    });
    assert_eq!(
        duo.bob_turn(),
        vec![FileEvent::Failed {
            peer: ALICE,
            transfer_id: id,
            reason: FileFailure::HashMismatch,
        }]
    );
    assert_eq!(
        duo.sent_types(Side::Bob),
        vec![MsgType::FileReceipt],
        "alıcı FILE_ABORT GÖNDERMEZ"
    );
    assert_eq!(
        duo.alice_turn(),
        vec![send_failed(BOB, id, SendFailure::HashMismatch)]
    );
    assert_eq!(duo.alice_files.outgoing_count(), 0);
}

/// `FILE_BEGIN`'i kabul edilmeyen aktarım (`rejected`, ilk tetik): eşten zaten
/// `MAX_INCOMING_TRANSFERS_PER_PEER` aktarım birleştirilirken gelen yeni aktarım reddedilir; gönderen
/// `Rejected` ile biter. Reddedilen aktarımın parçaları bilinmeyen aktarımdır. (S7 boyutu:
/// `oversize_file_is_refused_before_any_allocation`; ikinci tetik — `FILE_ABORT`'u önce gelen
/// aktarım: `synchronous_supersession_aborts_and_an_overtaken_file_begin_is_rejected`.)
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): ret `FILE_ABORT(storage_full)` ile bildiriliyordu, gönderen
/// `SendAborted` görürdü.
#[test]
fn a_file_begin_without_a_free_slot_is_rejected() {
    let mut duo = Duo::connected();
    // Bob, Alice'ten iki yarım aktarımı birleştiriyor (eş başına tavan).
    for transfer_id in [100, 101] {
        let begin = AppMessage::FileBegin {
            transfer_id,
            file_size: 10,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count: 1,
            name: NAME.to_owned(),
        };
        duo.inject(Side::Bob, record(&begin));
    }
    assert!(duo.bob_turn().is_empty());
    let source = sample_file(2 * duo.alice.limits().max_file_chunk_bytes());
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    assert!(
        duo.bob_turn().is_empty(),
        "reddedilen aktarımın yerel olayı yok"
    );
    let stats = duo.bob_files.stats();
    assert_eq!(
        (stats.transfers_refused, stats.unknown_transfer_records),
        (1, 3)
    );
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
    assert_eq!(
        duo.alice_turn(),
        vec![send_failed(BOB, id, SendFailure::Rejected)]
    );
}

/// `d3bc7407`'de KIRMIZI (uyarlansa da): ret eşe `FILE_ABORT(storage_full)` olarak gidiyordu
/// (`aborts_sent`).
#[test]
fn oversize_file_is_refused_before_any_allocation() {
    let mut duo = Duo::connected();
    let oversize = MAX_REASSEMBLY_BYTES + 1;
    let chunk_len = u64::try_from(duo.bob.limits().max_file_chunk_bytes()).expect("küçük sayı");
    let begin = AppMessage::FileBegin {
        transfer_id: INJECTED_ID,
        file_size: oversize,
        whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
        chunk_count: u32::try_from(oversize.div_ceil(chunk_len)).expect("u32'ye sığar"),
        name: NAME.to_owned(),
    };
    duo.inject(Side::Bob, record(&begin));
    let first = chunk(INJECTED_ID, 0, &sample_file(SMALL_CHUNK));
    duo.inject(Side::Bob, record(&first));
    assert!(duo.bob_turn().is_empty());
    // Aktarım tabloya hiç girmedi (tampon da bit haritası da ayrılmadı); parçası bilinmeyen
    // aktarımındır.
    assert_eq!(duo.bob_files.incoming_count(), 0);
    let stats = duo.bob_files.stats();
    assert_eq!(
        (
            stats.transfers_refused,
            stats.unknown_transfer_records,
            stats.receipts_sent
        ),
        (1, 1, 1)
    );
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![file_receipt(INJECTED_ID, FileReceiptStatus::Rejected)]
    );
}

/// `d3bc7407`'de KIRMIZI (uyarlansa da): son satır — eskide alıcı karşıya hiçbir şey göndermiyordu
/// (tamamlanan aktarımın alındısı yok). S8 retlerinin sessizliği AYNI DAVRANIŞ.
#[test]
fn inconsistent_records_are_dropped_without_poisoning_the_transfer() {
    let mut duo = Duo::connected();
    let content = sample_file(SMALL_SIZE);
    let hash = whole_file_hash(&content);
    let [begin, first, second, end] = small_transfer(&content, hash);
    let other = INJECTED_ID + 1;
    let bad = [
        // Süren aktarımın kimliğiyle farklı alanlar.
        AppMessage::FileBegin {
            transfer_id: INJECTED_ID,
            file_size: 11,
            whole_file_hash: hash,
            chunk_count: 2,
            name: NAME.to_owned(),
        },
        // Parça sınırında değil · dosyanın dışına taşıyor · uzunluk `min(L, kalan)` değil · boş ·
        // konum + uzunluk taşıyor.
        chunk(INJECTED_ID, 3, &content[3..SMALL_CHUNK]),
        chunk(INJECTED_ID, SMALL_CHUNK, &sample_file(SMALL_CHUNK)),
        chunk(
            INJECTED_ID,
            SMALL_CHUNK,
            &content[SMALL_CHUNK..SMALL_SIZE - 1],
        ),
        chunk(INJECTED_ID, SMALL_CHUNK, &[]),
        AppMessage::FileChunk {
            transfer_id: INJECTED_ID,
            offset: u64::MAX,
            payload: vec![0],
        },
        // Yerleşimden bağımsız tutarsız FILE_BEGIN'ler: boş olmayan dosyada sıfır parça · parça
        // sayısı bayt sayısından büyük.
        AppMessage::FileBegin {
            transfer_id: other,
            file_size: 5,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count: 0,
            name: NAME.to_owned(),
        },
        AppMessage::FileBegin {
            transfer_id: other,
            file_size: 5,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count: 6,
            name: NAME.to_owned(),
        },
    ];
    duo.inject(Side::Bob, record(&begin));
    // Kabul edilen ilk parça `L = 6`'yı sabitler.
    duo.inject(Side::Bob, record(&first));
    for message in &bad {
        duo.inject(Side::Bob, record(message));
    }
    // Hiç kurulmamış aktarımın parçası.
    duo.inject(Side::Bob, record(&chunk(other, 0, &content[..SMALL_CHUNK])));
    assert!(duo.bob_turn().is_empty());
    let stats = duo.bob_files.stats();
    let dropped = u64::try_from(bad.len()).expect("küçük sayı");
    assert_eq!(
        (stats.inconsistent_records, stats.unknown_transfer_records),
        (dropped, 1)
    );
    // S8: hiçbir ret karşıya sinyal üretmedi.
    assert!(duo.raw_inbox(Side::Alice).is_empty());
    // Atılan kayıtlar durumu bozmadı: doğru parça ve FILE_END ile aktarım tamamlanır.
    duo.inject(Side::Bob, record(&second));
    duo.inject(Side::Bob, record(&end));
    assert_eq!(duo.bob_turn(), vec![received(ALICE, INJECTED_ID, content)]);
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![file_receipt(INJECTED_ID, FileReceiptStatus::Ok)]
    );
}

// ── iptal ────────────────────────────────────────────────────────────────────────────────────

/// Alıcının kullanıcısı kabul edilmiş aktarımı iptal eder: eşe `FILE_RECEIPT(aborted_by_receiver)`
/// gider (DUR #108: alıcı iptali artık bildirilir), gönderen `AbortedByReceiver` ile biter ve kalan
/// parçaları SUNMAZ. Alıcı `FILE_ABORT` göndermez.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): `abandon_incoming` sessizdi — eşe sinyal gitmiyordu,
/// gönderen kalan parçaları göndermeye devam ederdi.
#[test]
fn receiver_cancel_sends_aborted_by_receiver_and_the_sender_stops() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(3 * LINK_QUEUE_CAP * chunk_len);
    let queue = u64::try_from(LINK_QUEUE_CAP).expect("küçük sayı");
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty());
    assert!(
        duo.bob_turn().is_empty(),
        "yarım aktarım: FILE_BEGIN + parçalar"
    );
    assert_eq!(duo.bob_files.incoming_count(), 1);
    assert!(duo.bob_files.cancel_incoming(&mut duo.bob, ALICE, id));
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
    // Gönderen alındıyı turun başında okur: aktarım biter, `pump` başka parça sunmaz.
    assert_eq!(
        duo.alice_turn(),
        vec![send_failed(BOB, id, SendFailure::AbortedByReceiver)]
    );
    assert_eq!(duo.accepted(Side::Alice), queue);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice_turn().is_empty());
        assert!(duo.bob_turn().is_empty());
    }
    assert_eq!(duo.accepted(Side::Alice), queue);
    assert!(
        !duo.bob_files.cancel_incoming(&mut duo.bob, ALICE, id),
        "izlenmeyen aktarım: eşe bir şey gitmez"
    );
    assert_eq!(duo.sent_types(Side::Bob), vec![MsgType::FileReceipt]);
}

/// Gönderen iptali (`FILE_ABORT(user_cancel)`) iki uçta da aktarımı bitirir (gönderende olay yok);
/// alıcının reddi (`FILE_RECEIPT(rejected)`) gönderende aktarımı bitirir ve kalan kayıtlar sunulmaz;
/// son kaydı kabul edilmiş aktarım iptal edilemez (`AlreadySent`) ve teslim yine bildirilir.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): alıcının reddi `FILE_ABORT(storage_full)` idi
/// (`SendAborted`); 0x0B kayıtsız tipti (DROP); `AlreadySent` yoktu (`FILE_END` kabulüyle aktarım
/// tablodan çıkıyordu → `UnknownTransfer`). İlk yarı (gönderen iptali) AYNI DAVRANIŞ.
#[test]
fn sender_cancel_and_receiver_rejection_end_the_transfer_on_both_sides() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(3 * LINK_QUEUE_CAP * chunk_len);
    let queue = u64::try_from(LINK_QUEUE_CAP).expect("küçük sayı");

    // Gönderen iptali: kalan kayıtların yerine tek FILE_ABORT(user_cancel).
    let cancelled = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.accepted(Side::Alice), queue);
    duo.alice_files
        .cancel_send(cancelled)
        .expect("izlenen aktarım");
    assert!(
        duo.bob_turn().is_empty(),
        "yarım aktarım: FILE_BEGIN + parçalar"
    );
    assert_eq!(duo.bob_files.incoming_count(), 1);
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.accepted(Side::Alice), queue + 1);
    assert_eq!(duo.alice_files.outgoing_count(), 0);
    assert_eq!(
        duo.bob_turn(),
        vec![FileEvent::Aborted {
            peer: ALICE,
            transfer_id: cancelled,
            reason: FileAbortReason::UserCancel,
        }]
    );
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert!(duo.sent_types(Side::Bob).is_empty(), "iptale alındı yok");
    assert_eq!(
        duo.alice_files.cancel_send(cancelled),
        Err(FileSendError::UnknownTransfer)
    );

    // Alıcının reddi: gönderen kalan kayıtları sunmaz.
    let refused = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.accepted(Side::Alice), 2 * queue + 1);
    duo.inject(
        Side::Alice,
        record(&file_receipt(refused, FileReceiptStatus::Rejected)),
    );
    assert_eq!(
        duo.alice_turn(),
        vec![send_failed(BOB, refused, SendFailure::Rejected)]
    );
    assert_eq!(duo.alice_files.outgoing_count(), 0);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice_turn().is_empty());
    }
    assert_eq!(duo.accepted(Side::Alice), 2 * queue + 1);

    // Son kaydı kabul edilmiş aktarım iptal edilemez; teslim yine bildirilir.
    let mut duo = Duo::connected();
    let small = sample_file(SMALL_SIZE);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, small.clone())
        .expect("kuyruğa alınır");
    assert_eq!(duo.alice_turn(), vec![sent(BOB, id)]);
    assert_eq!(
        duo.alice_files.cancel_send(id),
        Err(FileSendError::AlreadySent)
    );
    assert_eq!(duo.bob_turn(), vec![received(ALICE, id, small)]);
    assert_eq!(duo.alice_turn(), vec![delivered(BOB, id)]);
}

// ── oturum değişimi ──────────────────────────────────────────────────────────────────────────

/// D6 (olay yolu): yarım aktarımın oturumu değişince gönderen kalan kayıtları GÖNDERMEZ; aktarım yeni
/// oturumdan `FILE_ABORT(superseded)` ile kapanır (`Superseded`). Alıcı eski oturumdan gelenleri
/// eşine bağlar, sonra `FILE_ABORT`'la aktarımı bitirir; alıcıdan alındı gitmez. Kaldığı yerden devam
/// YOK.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): aktarım yeni oturumda sürüyordu (`Sent` + `Received`).
#[test]
fn supersession_closes_a_half_transfer_with_file_abort_superseded() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(2 * LINK_QUEUE_CAP * chunk_len);
    let queue = u64::try_from(LINK_QUEUE_CAP).expect("küçük sayı");
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty());
    // Supersession: eski tutamaçla kabul edilmiş birimler karşı kuyrukta kalır (yalnız-alım drain).
    let new = duo.link.borrow_mut().supersede();
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(
        at_alice,
        vec![send_failed(BOB, id, SendFailure::Superseded)]
    );
    assert_eq!(
        at_bob,
        vec![FileEvent::Aborted {
            peer: ALICE,
            transfer_id: id,
            reason: FileAbortReason::Superseded,
        }]
    );
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::FileAbort]);
    assert_eq!(duo.accepted(Side::Alice), queue + 1);
    assert_eq!(duo.alice_files.stats().transfers_superseded, 1);
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert!(duo.sent_types(Side::Bob).is_empty());
}

/// D6 (senkron yol + oturumlar arası sırasızlık): yeni oturum `Ready` olayı okunmadan benimsenir
/// (`Messenger::connect` → `ConnectResult::Ready`); `pump` yine `FILE_ABORT(superseded)` verir.
/// Alıcıda yeni oturumun `FILE_ABORT`'u eski oturumun drain'inden ÖNCE işlenir: geç gelen
/// `FILE_BEGIN` hayalet aktarım başlatmaz, `FILE_RECEIPT(rejected)` alır (ikinci `rejected` tetiği);
/// parçaları bilinmeyen aktarımdır; iki `FILE_IDLE_TTL` sonra da sahte zaman aşımı olayı yok.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): aktarım yeni oturumda sürüyordu (h'ye parçalar);
/// UYARLANAMAZ kısım: erken iptal kaydı yoktu — geç `FILE_BEGIN` aktarımı başlatırdı.
#[test]
fn synchronous_supersession_aborts_and_an_overtaken_file_begin_is_rejected() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let source = sample_file(2 * LINK_QUEUE_CAP * chunk_len);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source)
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty());
    let in_flight = duo.take_inbox(Side::Bob);
    let parts = u64::try_from(in_flight.len() - 1).expect("küçük sayı");
    let new = duo.link.borrow_mut().supersede();
    assert_eq!(duo.alice.connect(BOB), Ok(ConnectResult::Ready(new)));
    // Olaylar okunmadan.
    duo.alice_files.pump(&mut duo.alice);
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::FileAbort]);
    assert_eq!(
        duo.alice_files.poll_event(),
        Some(send_failed(BOB, id, SendFailure::Superseded))
    );
    // Olaylar sonra okunur: hiçbir şey yinelenmez.
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.sent_on(Side::Alice, new), vec![MsgType::FileAbort]);

    // Bob: `FILE_ABORT` (yeni oturum) drain'den ÖNCE.
    assert!(duo.bob_turn().is_empty());
    assert_eq!(duo.bob_files.stats().aborts_unmatched, 1);
    duo.deliver_late(Side::Bob, in_flight);
    assert!(duo.bob_turn().is_empty(), "hayalet aktarım yok");
    assert_eq!(duo.bob_files.incoming_count(), 0);
    let stats = duo.bob_files.stats();
    assert_eq!(
        (stats.begins_after_abort, stats.unknown_transfer_records),
        (1, parts)
    );
    assert_eq!(
        duo.sent_types(Side::Bob),
        vec![MsgType::FileReceipt],
        "rejected; FILE_ABORT yok"
    );
    duo.clock.advance(FILE_IDLE_TTL * 2);
    assert!(duo.bob_turn().is_empty(), "sahte zaman aşımı yok");
    // Gönderen aktarımı çoktan bitirmişti: geç `rejected` yok sayılır.
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.alice_files.stats().receipts_ignored, 1);
}

// ── `transfer_id` (D5) ───────────────────────────────────────────────────────────────────────

/// D5 (`CounterKind::TransferId`): `transfer_id` eş başına kalıcıdır — FileTransfer ve Messenger AYNI
/// depoyla yeniden kurulunca aynı eşe sıradaki `transfer_id` öncekilerin hepsinden büyüktür; aktarım
/// uçtan uca yürür.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): sayaç motorun belleğindeydi; yeni motor
/// `FIRST_TRANSFER_ID`'den başlardı (`second == first`).
#[test]
fn transfer_ids_continue_after_a_restart_with_the_same_store() {
    let store = fresh_store();
    let clock = ManualClock::default();
    let (mut messenger, mut files) = echo_node(Arc::clone(&store), &clock);
    connect_mock(&mut messenger, &mut files, BOB);
    let first = files
        .send_file(&messenger, BOB, NAME, sample_file(TAIL_BYTES))
        .expect("kuyruğa alınır");
    // Süreç ölür: motorlar yok olur, depo kalır.
    drop((messenger, files));
    let (mut messenger, mut files) = echo_node(Arc::clone(&store), &clock);
    connect_mock(&mut messenger, &mut files, BOB);
    let source = sample_file(TAIL_BYTES);
    let second = files
        .send_file(&messenger, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(second, first + 1);
    assert_eq!(turn(&mut messenger, &mut files), vec![sent(BOB, second)]);
    assert_eq!(
        turn(&mut messenger, &mut files),
        vec![received(BOB, second, source), delivered(BOB, second)]
    );
}

/// K4G-09 kararı (`cancel_send(transfer_id)` ve K4G-11'in `cancel_file(transfer_id)`'si eş almaz):
/// eş başına sayaçlar iki eşe aynı değeri önerebilir; `send_file` açık aktarımlar arasında
/// benzersizliği korur — başka bir eşin açık aktarımının değeri atlanır (kalıcı olarak ayrılmıştı,
/// yanar). Kayıtlar ve alındılar doğru eşe gider. Düzenek yansıtıcı sahtedir: iki eşe iki oturum.
///
/// `d3bc7407`'de KIRMIZI (uyarlansa da): tek, eşten bağımsız bellek sayacı vardı ve depo okunmuyordu
/// — kimlikler (0, 1) aynı çıkar ama depodaki eş sayaçları (`BOB` 1, `CAROL` 2 beklenir) 0 döner.
#[test]
fn open_transfer_ids_are_unique_across_peers() {
    let store = fresh_store();
    let clock = ManualClock::default();
    let (mut messenger, mut files) = echo_node(Arc::clone(&store), &clock);
    for peer in [BOB, CAROL] {
        connect_mock(&mut messenger, &mut files, peer);
    }
    let source = sample_file(TAIL_BYTES);
    let to_bob = files
        .send_file(&messenger, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    let to_carol = files
        .send_file(&messenger, CAROL, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(to_bob, FIRST_TRANSFER_ID);
    assert_eq!(
        to_carol,
        FIRST_TRANSFER_ID + 1,
        "Bob'un açık aktarımının değeri atlandı"
    );
    assert_eq!(
        turn(&mut messenger, &mut files),
        vec![sent(BOB, to_bob), sent(CAROL, to_carol)]
    );
    assert_eq!(
        turn(&mut messenger, &mut files),
        vec![
            received(BOB, to_bob, source.clone()),
            received(CAROL, to_carol, source),
            delivered(BOB, to_bob),
            delivered(CAROL, to_carol),
        ]
    );
    assert_eq!(
        store.reserve_next(BOB, CounterKind::TransferId),
        Ok(to_bob + 1)
    );
    assert_eq!(
        store.reserve_next(CAROL, CounterKind::TransferId),
        Ok(to_carol + 1)
    );
}

/// Depo hatası (D5): `transfer_id` ayrılamazsa tipli hata döner, `FILE_BEGIN` Transport'a verilmez
/// ve aktarım kuyruğa girmez — panik yok, sessiz yutma yok. Depo tükenmesi `TransferIdExhausted`'dır.
/// Ayrılamayan değer yoktur: depo düzelince eşin ilk değeri gelir.
///
/// `d3bc7407`'de UYARLANAMAZ: motor depoyu okumuyordu (`FileSendError::Store` yoktu).
#[test]
fn store_failure_queues_nothing_and_sends_no_file_begin() {
    let flaky = Arc::new(FlakyStore::default());
    let mut duo = Duo::connected_with(Arc::clone(&flaky) as Arc<dyn AppStore>);
    flaky.fail_reserve.store(true, Ordering::SeqCst);
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, BOB, NAME, sample_file(1)),
        Err(FileSendError::Store(AppStoreError::Persist))
    );
    flaky.fail_reserve.store(false, Ordering::SeqCst);
    flaky.exhaust_reserve.store(true, Ordering::SeqCst);
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, BOB, NAME, sample_file(1)),
        Err(FileSendError::TransferIdExhausted)
    );
    flaky.exhaust_reserve.store(false, Ordering::SeqCst);
    assert_eq!(duo.alice_files.outgoing_count(), 0);
    assert!(duo.alice_turn().is_empty());
    assert_eq!(
        duo.accepted(Side::Alice),
        0,
        "FILE_BEGIN Transport'a verilmedi"
    );
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, BOB, NAME, sample_file(1)),
        Ok(FIRST_TRANSFER_ID)
    );
}

/// `d3bc7407`'de AYNI DAVRANIŞ (hiçbir hata `transfer_id` tüketmez; `FILE_BEGIN`'i kabul edilmemiş
/// aktarımın iptali sessizdir).
#[test]
fn send_errors_are_typed_and_consume_no_transfer_id() {
    let mut duo = Duo::connected();
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, CAROL, NAME, sample_file(1)),
        Err(FileSendError::NoSession)
    );
    let long_name = "a".repeat(FILE_NAME_MAX_BYTES + 1);
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, BOB, &long_name, sample_file(1)),
        Err(FileSendError::Encode(EncodeError::NameTooLong {
            name_len: FILE_NAME_MAX_BYTES + 1
        }))
    );
    // S7 tavanı özetten ÖNCE denetlenir (sıfırlanmış tamponun sayfalarına dokunulmaz).
    let oversize = usize::try_from(MAX_REASSEMBLY_BYTES + 1).expect("64 MiB usize'a sığar");
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, BOB, NAME, vec![0; oversize]),
        Err(FileSendError::TooLarge {
            file_size: MAX_REASSEMBLY_BYTES + 1
        })
    );
    // Giden yuva tavanı.
    let ids: Vec<TransferId> = (0..MAX_OUTGOING_TRANSFERS)
        .map(|_| {
            duo.alice_files
                .send_file(&duo.alice, BOB, NAME, sample_file(1))
                .expect("yuva var")
        })
        .collect();
    assert_eq!(
        duo.alice_files
            .send_file(&duo.alice, BOB, NAME, sample_file(1)),
        Err(FileSendError::QueueFull)
    );
    // Hiçbir hata transfer_id tüketmedi: ilk başarılı gönderim ilk kimliği aldı, gerisi ardışık.
    assert_eq!(ids[0], FIRST_TRANSFER_ID);
    assert!(ids.windows(2).all(|pair| pair[1] == pair[0] + 1));
    // FILE_BEGIN'i henüz kabul edilmemiş aktarımın iptali sessizdir: tele hiçbir şey çıkmaz.
    for id in ids {
        duo.alice_files.cancel_send(id).expect("izlenen aktarım");
    }
    assert!(duo.alice_turn().is_empty());
    assert_eq!(duo.accepted(Side::Alice), 0);
    assert_eq!(duo.alice_files.outgoing_count(), 0);
}
