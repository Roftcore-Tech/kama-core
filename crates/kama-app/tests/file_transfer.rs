//! B2.3 — dosya aktarımı (`kama_app::file_transfer`) uçtan uca.
//!
//! Beklenen davranışın kaynağı: AMP §3/§4 (`FILE_*`, `MAX_FILE_CHUNK_BYTES`), S7
//! (`MAX_REASSEMBLY_BYTES` = 64 MiB), S8 (vektörsüz alıcı kararı → fail-closed DROP), BL B2.3 Done
//! (`hash(kaynak) == hash(hedef)`; Application'da retransmit yolu yok — H-APP-RETRY-0). Dosya motoru
//! `Messenger::poll`'u okumaz: her uçta testin döngüsü motorun olaylarını `FileTransfer::handle`'a
//! verir (B2.4'te bu yeri `Ptt::poll` alır), sonra `pump` ve `poll_event`. İki düzenek:
//!
//! * **`Link` (iki uç):** `tests/text_receipt.rs`'teki çiftin dosya için genişletilmiş kopyası.
//!   Transport'un içinde ilk iletimi "kaybolup" sonra kurtarılan birimler (`hold_sends` +
//!   `recover`) ve sırasızlık (`reverse_inbox_after`) testin elindedir — bu, S1 §4'ün aynı-oturum
//!   sıra garantisinden SERTTİR: alıcının sıra-bağımsızlığı sınanır. Uç başına Transport'un kabul
//!   ettiği gönderim sayılır (`accepted`): Application'ın kabul edilmiş bir kaydı yeniden
//!   vermediğinin motordan bağımsız kanıtı.
//! * **`MockTransport` (yansıtıcı, `tests/common/mock_transport.rs`):** kayıp + kurtarma anahtarıyla
//!   (H-APP-RETRY-0) yankı eşi: BOB'a gönderilen dosya aynı motora BOB'dan gelmiş gibi döner;
//!   giden ve gelen yarı aynı (`PeerRef`, `transfer_id`) için ayrı durumlardır.

mod common;

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

use common::mock_transport::MockTransport;
use kama_app::codec::limits::{FILE_NAME_MAX_BYTES, WHOLE_FILE_HASH_BYTES};
use kama_app::codec::{
    decode, encode, AmpLimits, AppMessage, EncodeError, FileAbortReason, MsgType,
};
use kama_app::file_transfer::{
    whole_file_hash, FileEvent, FileFailure, FileSendError, FileTransfer, TransferId,
    FIRST_TRANSFER_ID, MAX_OUTGOING_TRANSFERS, MAX_REASSEMBLY_BYTES,
};
use kama_app::messaging::{Messenger, MessengerEvent};
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
/// Bir turda okunan motor olayı tavanı.
const DRAIN_CAP: usize = 1024;
/// `Duo::settle` tur tavanı.
const ROUND_CAP: usize = 256;
/// "Zaman geçse de" tur sayısı (H-APP-RETRY-0).
const IDLE_ROUNDS: usize = 16;
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

fn received(peer: PeerRef, transfer_id: TransferId, bytes: Vec<u8>) -> FileEvent {
    FileEvent::Received {
        peer,
        transfer_id,
        name: NAME.to_owned(),
        bytes,
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
        MsgType::FileBegin | MsgType::FileChunk | MsgType::FileEnd | MsgType::FileAbort
    )
}

/// Bir ucun turu: motorun bütün olaylarını dosya motoruna verir (`FILE_*` tüketilir, gerisi aynen
/// geri döner), sonra giden kuyruğu sunar. Dosya olaylarını doğuş sırasıyla döndürür.
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
    /// Bu ucun motoruna teslim edilen birimler.
    delivered: u64,
    /// İlk iletimi kaybolacak gönderimlerin kabul sıraları (0'dan).
    hold: BTreeSet<u64>,
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
        if remote.inbox.len() + remote.held.len() >= LINK_QUEUE_CAP {
            return Err(TransportError::Backpressure);
        }
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

/// İki uçlu düzenek: Alice'in ve Bob'un motorları, dosya motorları ve testin `Link` denetimi.
struct Duo {
    link: Rc<RefCell<Link>>,
    alice: Messenger<End>,
    bob: Messenger<End>,
    alice_files: FileTransfer,
    bob_files: FileTransfer,
}

impl Duo {
    /// Oturum kurulmuş, iki tarafın `Ready` olayı okunmuş düzenek.
    fn connected() -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let mut duo = Self {
            alice: Messenger::new(End::new(&link, Side::Alice)).expect("limitler geçerli"),
            bob: Messenger::new(End::new(&link, Side::Bob)).expect("limitler geçerli"),
            alice_files: FileTransfer::new(),
            bob_files: FileTransfer::new(),
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

    /// `side` ucundan Transport'un kabul ettiği gönderim sayısı.
    fn accepted(&self, side: Side) -> u64 {
        self.link.borrow_mut().endpoint(side).accepted
    }
}

// ── yansıtıcı sahte ──────────────────────────────────────────────────────────────────────────

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

// ── testler ──────────────────────────────────────────────────────────────────────────────────

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
    assert_eq!(
        duo.alice_turn(),
        vec![FileEvent::Sent {
            peer: BOB,
            transfer_id: id,
        }]
    );
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
    // Her kayıt Transport'a tam bir kez kabul ettirildi; alıcı hiçbir şey göndermedi (ACK yok).
    assert_eq!(duo.accepted(Side::Alice), records);
    assert_eq!(duo.alice_files.stats().records_accepted, records);
    assert_eq!(duo.accepted(Side::Bob), 0);
}

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
    assert_eq!(
        at_alice,
        vec![FileEvent::Sent {
            peer: BOB,
            transfer_id: id,
        }]
    );
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

#[test]
fn empty_file_round_trips_without_chunks() {
    let mut duo = Duo::connected();
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, Vec::new())
        .expect("kuyruğa alınır");
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(
        at_alice,
        vec![FileEvent::Sent {
            peer: BOB,
            transfer_id: id,
        }]
    );
    assert_eq!(at_bob, vec![received(ALICE, id, Vec::new())]);
    // FILE_BEGIN + FILE_END.
    assert_eq!(duo.accepted(Side::Alice), 2);
}

#[test]
fn echo_peer_reassembles_through_mock_loss_and_recovery() {
    let mut messenger = Messenger::new(MockTransport::new(caps())).expect("limitler geçerli");
    let mut files = FileTransfer::new();
    connect_mock(&mut messenger, &mut files, BOB);
    let chunk_len = messenger.limits().max_file_chunk_bytes();
    let source = sample_file(4 * chunk_len + TAIL_BYTES);
    // FILE_BEGIN ile parça 0'ın ilk iletimi kaybolur; sahtenin Transport'u onları kurtarır ve aynı
    // oturumda sırayı koruyarak (arkalarındakileri bekleterek) teslim eder.
    messenger.transport_mut().drop_next_units(2);
    let id = files
        .send_file(&messenger, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert_eq!(
        turn(&mut messenger, &mut files),
        vec![FileEvent::Sent {
            peer: BOB,
            transfer_id: id,
        }]
    );
    assert_eq!(
        files.incoming_count(),
        0,
        "kurtarılana kadar hiçbir şey gelmedi"
    );
    messenger.transport_mut().pump();
    assert_eq!(
        turn(&mut messenger, &mut files),
        vec![received(BOB, id, source)]
    );
    let stats = messenger.transport().stats();
    assert_eq!((stats.units_dropped, stats.units_recovered), (2, 2));
    // Yedi kayıt (1 + 5 + 1), her biri bir kez; kurtarma Transport'undur.
    assert_eq!(files.stats().records_accepted, 7);
}

#[test]
fn lost_chunk_is_never_resent_and_the_transfer_waits() {
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
    assert_eq!(
        at_alice,
        vec![FileEvent::Sent {
            peer: BOB,
            transfer_id: id,
        }]
    );
    assert!(at_bob.is_empty(), "eksik parçayla dosya teslim edilmez");
    // Zaman geçse de Application yeniden göndermez ve kayıp tespit etmez (H-APP-RETRY-0).
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice_turn().is_empty());
        assert!(duo.bob_turn().is_empty());
    }
    assert_eq!(duo.accepted(Side::Alice), records);
    assert_eq!(duo.accepted(Side::Bob), 0);
    // Zamanlayıcı yok: yarım aktarım yalnız yerel kararla bırakılır; eşe sinyal gitmez.
    assert_eq!(duo.bob_files.incoming_count(), 1);
    assert!(duo.bob_files.abandon_incoming(ALICE, id));
    assert_eq!(duo.bob_files.incoming_count(), 0);
    assert!(duo.raw_inbox(Side::Alice).is_empty());
}

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
    // Hiçbir tekrar karşıya sinyal üretmedi (parça ACK'i yok).
    assert!(duo.raw_inbox(Side::Alice).is_empty());
}

#[test]
fn wrong_whole_file_hash_fails_with_hash_mismatch() {
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
    // Alıcı eşe tek bir FILE_ABORT(hash_mismatch) gönderdi; parça yeniden istemi YOK.
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![AppMessage::FileAbort {
            transfer_id: INJECTED_ID,
            reason: FileAbortReason::HashMismatch,
        }]
    );
}

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
            stats.aborts_sent
        ),
        (1, 1, 1)
    );
    assert_eq!(
        duo.raw_inbox(Side::Alice),
        vec![AppMessage::FileAbort {
            transfer_id: INJECTED_ID,
            reason: FileAbortReason::StorageFull,
        }]
    );
}

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
    // Atılan kayıtlar durumu bozmadı: doğru parça ve FILE_END ile aktarım tamamlanır.
    duo.inject(Side::Bob, record(&second));
    duo.inject(Side::Bob, record(&end));
    assert_eq!(duo.bob_turn(), vec![received(ALICE, INJECTED_ID, content)]);
    // S8: hiçbir ret karşıya sinyal üretmedi.
    assert!(duo.raw_inbox(Side::Alice).is_empty());
}

#[test]
fn abort_drops_the_transfer_on_both_sides() {
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
    let abort = AppMessage::FileAbort {
        transfer_id: refused,
        reason: FileAbortReason::StorageFull,
    };
    duo.inject(Side::Alice, record(&abort));
    assert_eq!(
        duo.alice_turn(),
        vec![FileEvent::SendAborted {
            peer: BOB,
            transfer_id: refused,
            reason: FileAbortReason::StorageFull,
        }]
    );
    assert_eq!(duo.alice_files.outgoing_count(), 0);
    for _ in 0..IDLE_ROUNDS {
        assert!(duo.alice_turn().is_empty());
    }
    assert_eq!(duo.accepted(Side::Alice), 2 * queue + 1);
}

#[test]
fn session_change_mid_transfer_resends_nothing() {
    let mut duo = Duo::connected();
    let chunk_len = duo.alice.limits().max_file_chunk_bytes();
    let chunks = 2 * LINK_QUEUE_CAP;
    let source = sample_file(chunks * chunk_len);
    let id = duo
        .alice_files
        .send_file(&duo.alice, BOB, NAME, source.clone())
        .expect("kuyruğa alınır");
    assert!(duo.alice_turn().is_empty());
    // Supersession: eski tutamaçla kabul edilmiş birimler karşı kuyrukta kalır (yalnız-alım
    // drain), kalan kayıtlar yeni tutamaçtan gider. Dosya için S1 K7 yeniden gönderimi YOK.
    duo.link.borrow_mut().supersede();
    let (at_alice, at_bob) = duo.settle();
    assert_eq!(
        at_alice,
        vec![FileEvent::Sent {
            peer: BOB,
            transfer_id: id,
        }]
    );
    assert_eq!(at_bob, vec![received(ALICE, id, source)]);
    let records = u64::try_from(chunks + 2).expect("küçük sayı");
    assert_eq!(duo.accepted(Side::Alice), records);
}

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
