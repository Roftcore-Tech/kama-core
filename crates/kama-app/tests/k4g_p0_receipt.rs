//! K4G P0 şeridi "RECEIPT" — R3 bulgusunu gösteren belirlenimli testler. Düzeltme İÇERMEZ.
//!
//! Taban: main `3439fccd`. Karar: CEVAP PAKETİ 9 (C9) §3 K4G-05d ve §4 P0. Bulgu: dış inceleme
//! 2026-10-24 §4 R3; bağımsız doğrulama `verify-wf_f84f4d0e`, anahtar "R3" (DOĞRULANDI, iki
//! daraltmayla).
//!
//! # Bulgu (R3)
//!
//! `messaging.rs` `send_receipt` (:1110-1136) `RECEIPT(delivered)`'ı TEK KEZ `send(.., Reliable)`
//! ile sunar. `Err` dönerse yalnız `MessengerStats::receipts_unsent` artar; bekleyen kayıt tutulmaz
//! (`PeerState`, :858-878, böyle bir alan taşımaz). RECEIPT yalnız tekrar TEXT ile yeniden üretilir
//! (`on_text`, :801-822). Gönderen ise kabul edilmiş kaydı aynı tutamaca bir daha vermez
//! (`flush_peer`, :1093-1096); tekrar TEXT ancak oturum değişiminde (`Ready(h')`, yeniden
//! başlatma, kapanış + `connect`) gelir. Canlı oturumun yenilenmesi 7 günde ya da 2^31 PN'de
//! olduğundan `Backpressure` ile reddedilen RECEIPT, gönderendeki `Delivered`'ı oturum ömrü boyunca
//! geciktirir. Yanlış "teslim edildi" DOĞMAZ; eksik olan teslim sinyalidir (doğrulama daraltması
//! 1). Gerçek boşluk canlı oturumdaki `Backpressure`'dır; "oturum yok" ve `SessionSuperseded`
//! sonraki oturumda zaten düzelir (daraltma 2). Bu yüzden testler yalnız `Backpressure`'ı kurar ve
//! "yeni oturum YOK" koşulunu denetler.
//!
//! # Tetik (ürün yolu)
//!
//! `Backpressure` ürün profilinde oturum başına offer kanalı doluyken doğar (`runtime.rs`
//! `offer_queue` 1024 / `max_inflight_units` 4096 → `QueueFull` → `transport.rs:1212`
//! `TransportError::Backpressure`). Gerçekçi tetik: RECEIPT'i üreten taraf aynı eşe dosya
//! gönderiyor. `kama-node` turu (`node.rs:451-470`) önce `Ptt::poll` zinciriyle Messenger'ı okur
//! (RECEIPT burada sunulur); Messenger kuyruğu boşalınca `FileTransfer::pump` Transport kuyruğunu
//! reddedilene dek doldurur; sonraki turun RECEIPT'i dolu kuyrukla karşılaşır. Düzenek bu sırayı
//! birebir kurar.
//!
//! # Düzenek
//!
//! * **Neden ortak sahte değil:** `tests/common/mock_transport.rs` yansıtıcıdır (gönderilen birim
//!   aynı uca döner) ve `send`'i `inbox + in_recovery ≥ MAX_INBOX` iken reddeder. RECEIPT,
//!   `recv`'in TEXT'i kuyruktan aldığı çağrıda sunulur; o anda kuyrukta en az bir yer açılmıştır,
//!   dolayısıyla RECEIPT'in ilk sunumu orada `Backpressure` alamaz. Ortak sahteye dokunulmadı.
//!   `text_receipt.rs`'teki iki uçlu `Link` o test ikilisine özeldir; burada aynı modelin küçük bir
//!   kopyası var.
//! * **`Link` (iki uç):** bir uçtan kabul edilen birim karşı ucun teslim kuyruğuna düşer. Yön
//!   başına kabul edilmiş ama karşı Application'ın henüz okumadığı birim sayısı `LINK_QUEUE_CAP`
//!   ile sınırlıdır; doluyken `send` → `Backpressure` (ürünün dolu offer kanalının karşılığı).
//!   "Kapasite açılır" = alıcı (Alice) kuyruğunu okur; oturum ve tutamaç DEĞİŞMEZ. Her `send`
//!   denemesi sonucuyla kaydedilir (neyin sunulduğu, neyin kabul edildiği).
//! * **Bob = `kama-node` düğümünün motorları:** `Messenger` + `Ptt` + `FileTransfer` +
//!   `Signaling`; `Node::turn`, `NodeLoop::turn`'un çağrı sırasını kopyalar (komut işleme ve
//!   `drain_background` hariç; bu testlerde komut yalnız `send_file`'dır). Alice yalnız
//!   `Messenger`'dır: TEXT gönderir, RECEIPT'i gözler.
//! * Saat elle ilerler; RNG yok; her koşu aynı sırayı üretir.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kama_app::clock::MonotonicClock;
use kama_app::codec::{decode, encode, AmpLimits, AppMessage, ReceiptKind};
use kama_app::file_transfer::FileTransfer;
use kama_app::messaging::{Messenger, MessengerEvent, MsgId};
use kama_app::ptt::{Ptt, PttEvent};
use kama_app::signaling::{Signaling, SignalingEvent};
use kama_app::store::{AppStore, MemoryAppStore};
use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport, PreparedUnit,
    SessionEvent, TransportCapabilities, TransportError,
};

/// Test parametresi (normatif değil): codec vektörlerinin ve `text_receipt.rs`'in değeriyle aynı.
const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// Düzeneğin birim tavanı (`max_plaintext_unit_bytes`, `usize`).
const LINK_MAX_UNIT_BYTES: usize = TEST_MAX_PLAINTEXT_UNIT_BYTES as usize;
/// Yön başına kabul edilmiş, karşı Application'ın okumadığı birim tavanı — düzenek sınırı,
/// normatif DEĞİL. Ürünün karşılığı oturum başına offer kanalı (1024) + unit-store (4096); küçük
/// değer yalnız düzeneği kısaltır, mekanizma aynıdır (dolu → `Backpressure`).
const LINK_QUEUE_CAP: usize = 16;
/// Uç başına olay kuyruğu tavanı (düzenek sınırı).
const LINK_EVENT_CAP: usize = 16;
/// Uç başına `send` denemesi kaydının tavanı (düzenek sınırı; aşımı senaryo hatasıdır).
const SEND_LOG_CAP: usize = 4096;
/// Alice'in `poll` döngüsünün tavanı.
const DRAIN_CAP: usize = 1024;
/// Turda işlenen en çok olay: `kama-node` `MAX_EVENTS_PER_TURN` (node.rs:93) =
/// `client_core::transport::MAX_INBOX` (1024). kama-app testi o crate'leri göremez
/// (`layer_check`); değer kopyadır, normatif değil.
const TURN_EVENT_BUDGET: usize = 1024;
/// Turlar arası sanal süre: `kama-node` `NODE_TICK` (lib.rs:123) kopyası, normatif değil. R3
/// zamanlayıcı içermez; süre yalnız "zaman geçse de" koşulunu kurar (toplam 160 ms).
const TURN_TICK: Duration = Duration::from_millis(20);
/// Saatin başlangıcı (sıfır anı kenar durumlarından uzak).
const CLOCK_START: Duration = Duration::from_secs(1);
/// Kapasite açıldıktan sonra koşulan tur sayısı (her turdan önce Alice kuyruğunu okur).
const RECOVERY_ROUNDS: usize = 8;
/// Bob'un Alice'e gönderdiği dosya: kayıt sayısı kuyruk tavanını aşar (aşağıdaki `const` denetimi).
const FILE_BYTES: usize = 64 * 1024;
/// Dosya içeriği (değeri önemsiz).
const FILE_FILL_BYTE: u8 = 0x5A;
/// Dosya adı (`FILE_BEGIN`'e sığar).
const FILE_NAME: &str = "buyuk.bin";
/// Alice'in metni.
const TEXT: &str = "dosya giderken metin";
/// Test 2: aynı `(Alice, msg_id)` için Bob'a gelen TEXT sayısı (ilk + tekrarlar).
const DUPLICATE_TEXTS: u64 = 4;

const ALICE: PeerRef = PeerRef::from_dest_hash([0xA1; 32]);
const BOB: PeerRef = PeerRef::from_dest_hash([0xB0; 32]);

// Parça yükü birim tavanından küçüktür; dolayısıyla dosyanın kayıt sayısı en az
// `FILE_BYTES / LINK_MAX_UNIT_BYTES`'tır ve tek pump turu kuyruğu doldurur.
const _: () = assert!(FILE_BYTES / LINK_MAX_UNIT_BYTES > LINK_QUEUE_CAP);
// Test 2'nin bütün TEXT'leri Bob'un teslim kuyruğuna sığar.
const _: () = assert!(DUPLICATE_TEXTS as usize <= LINK_QUEUE_CAP);

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

fn text_record(msg_id: MsgId, text: &str) -> Vec<u8> {
    let message = AppMessage::Text {
        msg_id,
        text: text.to_owned(),
    };
    encode(&message, &limits()).expect("kodlanabilir").bytes
}

fn receipt(ref_msg_id: MsgId) -> AppMessage {
    AppMessage::Receipt {
        ref_msg_id,
        kind: ReceiptKind::Delivered,
    }
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

/// Taze bellek içi depo.
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

// ── iki uçlu düzenek ─────────────────────────────────────────────────────────────────────────

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

/// Bir `send` denemesi: hangi tutamaca, hangi baytlar, sonuç.
struct SendAttempt {
    session: SessionHandle,
    unit: Vec<u8>,
    result: Result<(), TransportError>,
}

/// Bir RECEIPT sunumunun özeti: tutamaç ve Transport'un sonucu.
type Attempt = (SessionHandle, Result<(), TransportError>);

/// Bir ucun Transport durumu.
#[derive(Default)]
struct Endpoint {
    /// Eşten kabul edilmiş, bu ucun motorunun henüz okumadığı birimler (≤ `LINK_QUEUE_CAP`).
    inbox: VecDeque<Delivered>,
    /// Bu uca sıralanmış oturum olayları (≤ `LINK_EVENT_CAP`).
    events: VecDeque<SessionEvent>,
    /// Gönderim kabul eden tutamaç.
    active: Option<SessionHandle>,
    /// Bu ucun bütün `send` denemeleri, sırayla (≤ `SEND_LOG_CAP`).
    sends: Vec<SendAttempt>,
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

    fn endpoint_ref(&self, side: Side) -> &Endpoint {
        match side {
            Side::Alice => &self.alice,
            Side::Bob => &self.bob,
        }
    }

    /// İki uçta aynı tutamaçla oturum: her uca `Ready(h, eş)`.
    fn establish(&mut self) -> SessionHandle {
        self.last_handle += 1;
        let handle = SessionHandle::from_raw(self.last_handle);
        for side in [Side::Alice, Side::Bob] {
            let endpoint = self.endpoint(side);
            assert!(endpoint.active.is_none(), "establish: aktif oturum var");
            endpoint.active = Some(handle);
            endpoint.push_event(SessionEvent::Ready(handle, side.remote()));
        }
        handle
    }

    /// `to` ucuna, aktif oturumu üzerinden eşten gelmiş gibi ham kayıt koyar.
    fn inject(&mut self, to: Side, unit: Vec<u8>) {
        let endpoint = self.endpoint(to);
        let session = endpoint.active.expect("inject: aktif oturum yok");
        assert!(endpoint.inbox.len() < LINK_QUEUE_CAP, "inject: kuyruk dolu");
        endpoint.inbox.push_back(Delivered { session, unit });
    }

    /// `from` ucunun `RECEIPT(delivered, msg_id)` sunumları, sırayla.
    fn receipt_attempts(&self, from: Side, msg_id: MsgId) -> Vec<Attempt> {
        let limits = limits();
        let expected = receipt(msg_id);
        self.endpoint_ref(from)
            .sends
            .iter()
            .filter(|attempt| {
                decode(&attempt.unit, &limits).is_ok_and(|message| message == expected)
            })
            .map(|attempt| (attempt.session, attempt.result))
            .collect()
    }

    /// `from` ucunun Transport'a kabul ettirdiği gönderim sayısı.
    fn accepted_sends(&self, from: Side) -> usize {
        self.endpoint_ref(from)
            .sends
            .iter()
            .filter(|attempt| attempt.result.is_ok())
            .count()
    }

    /// `from` ucunun son `send` denemesinin sonucu.
    fn last_send_result(&self, from: Side) -> Option<Result<(), TransportError>> {
        self.endpoint_ref(from)
            .sends
            .last()
            .map(|attempt| attempt.result)
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
        let result = if me.active != Some(session) {
            Err(TransportError::UnknownSession)
        } else if unit.bytes.len() > LINK_MAX_UNIT_BYTES {
            Err(TransportError::UnitTooLarge)
        } else if remote.inbox.len() >= LINK_QUEUE_CAP {
            // Yönün kuyruğu dolu: ürünün dolu offer kanalı (`QueueFull` → `Backpressure`).
            Err(TransportError::Backpressure)
        } else {
            remote.inbox.push_back(Delivered {
                session,
                unit: unit.bytes.clone(),
            });
            Ok(())
        };
        assert!(
            me.sends.len() < SEND_LOG_CAP,
            "gönderim kaydı tavanı aşıldı"
        );
        me.sends.push(SendAttempt {
            session,
            unit: unit.bytes,
            result,
        });
        result
    }

    fn recv(&mut self) -> Option<Delivered> {
        self.link.borrow_mut().endpoint(self.side).inbox.pop_front()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.link
            .borrow_mut()
            .endpoint(self.side)
            .events
            .pop_front()
    }

    fn close(&mut self, session: SessionHandle) -> Result<(), TransportError> {
        let mut link = self.link.borrow_mut();
        let endpoint = link.endpoint(self.side);
        if endpoint.active != Some(session) {
            return Err(TransportError::UnknownSession);
        }
        endpoint.active = None;
        endpoint.push_event(SessionEvent::Closed(session, CloseReason::UserClosed));
        Ok(())
    }

    fn cancel(&mut self, _ticket: ConnectTicket) {}
}

// ── Bob: kama-node düğümünün motorları ve turu ──────────────────────────────────────────────

/// `kama-node` `AppEngines`'in karşılığı: tek Messenger ve onu kullanan üç motor.
struct Node {
    messenger: Messenger<End>,
    ptt: Ptt,
    files: FileTransfer,
    signaling: Signaling,
}

impl Node {
    fn new(messenger: Messenger<End>, clock: &ManualClock) -> Self {
        Self {
            messenger,
            ptt: Ptt::new(Arc::new(clock.clone())),
            files: FileTransfer::new(Arc::new(clock.clone())),
            signaling: Signaling::new(Arc::new(clock.clone())),
        }
    }

    /// `kama-node` `NodeLoop::turn` (node.rs:451-470) ile AYNI çağrı sırası: `Ptt::poll`
    /// Messenger'ın tek okuyucusudur → `PttEvent::Messenger` olayı `FileTransfer::handle`'a → dosya
    /// motorunun geçirdiği olay `Signaling::handle`'a; Messenger kuyruğu boşaldıysa
    /// `FileTransfer::pump` ve `Signaling::pump`; dosya olayları her adımdan sonra boşaltılır.
    /// Zincirin sonuna ulaşan Messenger olaylarını döndürür (düğümün `on_messenger_event`'ine
    /// gidecek olanlar). PTT olayları bu testlerde doğmaz.
    fn turn(&mut self) -> Vec<MessengerEvent> {
        let mut passed = Vec::new();
        let mut budget = TURN_EVENT_BUDGET;
        let drained = loop {
            if budget == 0 {
                break false;
            }
            budget -= 1;
            let Some(event) = self.ptt.poll(&mut self.messenger) else {
                break true;
            };
            if let PttEvent::Messenger(event) = event {
                if let Some(event) = self.files.handle(&mut self.messenger, event) {
                    if let SignalingEvent::Messenger(event) =
                        self.signaling.handle(&mut self.messenger, event)
                    {
                        passed.push(event);
                    }
                }
                self.drain_file_events();
            }
        };
        if drained {
            self.files.pump(&mut self.messenger);
            self.signaling.pump(&mut self.messenger);
        }
        self.drain_file_events();
        passed
    }

    /// Düğümün `drain_file_events`'i: dosya motorunun olay kuyruğu boşaltılır (içerik bu testlerin
    /// konusu değil).
    fn drain_file_events(&mut self) {
        while self.files.poll_event().is_some() {}
    }
}

// ── senaryo ──────────────────────────────────────────────────────────────────────────────────

/// Kapasite açıldıktan sonraki turların gözlemi.
struct AfterOpening {
    /// Alice'in bu turlarda okuduğu olaylar.
    alice: Vec<MessengerEvent>,
    /// Bob'un turlarında zincirin sonuna ulaşan Messenger olayları.
    bob: Vec<MessengerEvent>,
    /// Bu turlarda Bob'un Transport'a kabul ettirdiği gönderim sayısı.
    bob_accepted: usize,
}

/// Alice (TEXT gönderen, yalnız Messenger) + Bob (`kama-node` motorları) + düzenek.
struct Rig {
    link: Rc<RefCell<Link>>,
    clock: ManualClock,
    alice: Messenger<End>,
    bob: Node,
    /// Testin tek oturumu; hiçbir adımda değişmez.
    session: SessionHandle,
}

impl Rig {
    /// Oturum kurulu ve iki taraf `Ready`'yi okumuş; Bob Alice'e büyük bir dosya gönderiyor ve
    /// dosya pump'ı Bob→Alice kuyruğunu reddedilene dek doldurmuş (`kama-node` sırası: `send_file`
    /// komutu → tur → kuyruk boş → `FileTransfer::pump`).
    fn with_full_bob_to_alice_queue() -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let clock = ManualClock::started();
        let alice = Messenger::with_store(End::new(&link, Side::Alice), fresh_store())
            .expect("limitler geçerli");
        let bob_messenger = Messenger::with_store(End::new(&link, Side::Bob), fresh_store())
            .expect("limitler geçerli");
        let bob = Node::new(bob_messenger, &clock);
        let session = link.borrow_mut().establish();
        let mut rig = Self {
            link,
            clock,
            alice,
            bob,
            session,
        };
        assert_eq!(
            rig.drain_alice(),
            vec![MessengerEvent::Session(SessionEvent::Ready(session, BOB))]
        );
        assert_eq!(
            rig.bob.turn(),
            vec![MessengerEvent::Session(SessionEvent::Ready(session, ALICE))]
        );
        rig.bob
            .files
            .send_file(
                &rig.bob.messenger,
                ALICE,
                FILE_NAME,
                vec![FILE_FILL_BYTE; FILE_BYTES],
            )
            .expect("dosya kuyruğa alınır");
        assert!(
            rig.bob.turn().is_empty(),
            "dolum turu olay üretmez (Bob'a gelen birim yok)"
        );
        let (queued, last) = {
            let link = rig.link.borrow();
            (
                link.endpoint_ref(Side::Alice).inbox.len(),
                link.last_send_result(Side::Bob),
            )
        };
        assert_eq!(
            queued, LINK_QUEUE_CAP,
            "düzenek: dosya pump'ı Bob→Alice kuyruğunu doldurmalı"
        );
        assert_eq!(
            last,
            Some(Err(TransportError::Backpressure)),
            "düzenek: pump kuyruğu reddedilene dek doldurur (son sunum Backpressure)"
        );
        rig
    }

    /// Alice'in `poll`'u `None` verene kadar olayları toplar. Kuyruğu okumak Bob→Alice yönünde
    /// kapasite açar.
    fn drain_alice(&mut self) -> Vec<MessengerEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.alice.poll() {
            assert!(events.len() < DRAIN_CAP, "Alice'in olay akışı bitmiyor");
            events.push(event);
        }
        events
    }

    /// Kapasite açılır ve Bob `RECOVERY_ROUNDS` tur çalışır: her turdan önce Alice kuyruğunu okur
    /// (yer açılır), saat `TURN_TICK` ilerler, Bob `kama-node` tur sırasıyla çalışır. Son turdan
    /// sonra Alice bir kez daha okur. Yeni oturum, `Ready` ya da `connect` YOK.
    fn open_capacity_and_run_turns(&mut self) -> AfterOpening {
        let accepted_before = self.link.borrow().accepted_sends(Side::Bob);
        let mut alice = Vec::new();
        let mut bob = Vec::new();
        for _ in 0..RECOVERY_ROUNDS {
            alice.extend(self.drain_alice());
            self.clock.advance(TURN_TICK);
            bob.extend(self.bob.turn());
        }
        alice.extend(self.drain_alice());
        let bob_accepted = self.link.borrow().accepted_sends(Side::Bob) - accepted_before;
        AfterOpening {
            alice,
            bob,
            bob_accepted,
        }
    }

    /// Ön koşul (iki koşulda da yeşil): kapasite gerçekten açıldı ve oturum değişmedi. Biri
    /// düşerse sonraki kırmızı R3'ü GÖSTERMEZ (düzenek hatası).
    fn assert_same_session_and_capacity_opened(&self, after: &AfterOpening) {
        assert!(
            after.bob_accepted > 0,
            "düzenek: kapasite açılmadı (turlarda Bob'un hiçbir sunumu kabul edilmedi); bu \
             durumda RECEIPT'in gidememesi R3'ü göstermez"
        );
        let session_events: Vec<&MessengerEvent> = after
            .alice
            .iter()
            .chain(&after.bob)
            .filter(|event| matches!(event, MessengerEvent::Session(_)))
            .collect();
        assert!(
            session_events.is_empty(),
            "düzenek: R3 'yeni oturum YOK' koşulu bozuldu; oturum olayları: {session_events:?}"
        );
        assert_eq!(
            self.alice.session_of(BOB),
            Some(self.session),
            "düzenek: Alice'in oturumu değişmemeli"
        );
        assert_eq!(
            self.bob.messenger.session_of(ALICE),
            Some(self.session),
            "düzenek: Bob'un oturumu değişmemeli"
        );
    }
}

/// `attempts` içinde `session` tutamacına kabul ettirilen sunum sayısı.
fn accepted_on(attempts: &[Attempt], session: SessionHandle) -> usize {
    attempts
        .iter()
        .filter(|(on, result)| *on == session && result.is_ok())
        .count()
}

/// `events` içindeki `Delivered(BOB, msg_id)` sayısı.
fn count_delivered(events: &[MessengerEvent], msg_id: MsgId) -> usize {
    let expected = delivered(BOB, msg_id);
    events.iter().filter(|event| **event == expected).count()
}

// ── testler ──────────────────────────────────────────────────────────────────────────────────

/// **Bulgu R3 — tabanda (`3439fccd`) KIRMIZI beklenir.**
///
/// Senaryo (tek oturum, hiç `Ready(h')` yok): Bob'un dosya pump'ı Bob→Alice kuyruğunu doldurmuş;
/// Alice TEXT gönderir; Bob'un turunda TEXT teslim edilir (`TextReceived`) ve RECEIPT'in ilk
/// sunumu `Backpressure` alır. Ardından Alice kuyruğunu okur (kapasite açılır) ve Bob
/// `RECOVERY_ROUNDS` tur boyunca `kama-node` tur sırasıyla çalışır; dosya kayıtları akmaya devam
/// eder. Beklenen (C9 K4G-05d Done): RECEIPT AYNI oturumda Transport'a kabul ettirilir ve Alice tam
/// bir `Delivered` görür.
///
/// Tabanda neden kırmızı: `send_receipt` tek denemedir (`messaging.rs:1110-1136`). Reddedilen
/// RECEIPT için kayıt tutulmaz; turun hiçbir çağrısı onu yeniden sunmaz (`Messenger::poll` →
/// `flush_resends` yalnız outbox'taki TEXT'leri oturum değişiminde yeniden sunar; `Ptt::poll`,
/// `FileTransfer::pump`, `Signaling::pump` RECEIPT bilmez); yalnız `receipts_unsent` artar. Kırmızı
/// "RECEIPT aynı oturumda kabul edilmedi" denetiminde düşmelidir.
///
/// Ön koşul denetimleri (tabanda da düzeltmeden sonra da yeşil): TEXT teslim edildi; RECEIPT'in
/// ilk sunumu `Backpressure` aldı ve henüz kabul edilmedi; kapasite gerçekten açıldı; oturum
/// değişmedi. `receipts_unsent` sabitlenmez (düzeltme anlamını değiştirebilir), yalnız mesajda
/// raporlanır.
#[test]
fn p0_r3_receipt_survives_backpressure() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");

    // Bob'un turu: TEXT teslim edilir; RECEIPT'in ilk sunumu dolu kuyrukla karşılaşır.
    assert_eq!(
        rig.bob.turn(),
        vec![received(ALICE, msg_id, TEXT)],
        "ön koşul: Bob TEXT'i teslim etmeli"
    );
    let attempts_now = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    assert_eq!(
        attempts_now.first(),
        Some(&(session, Err(TransportError::Backpressure))),
        "ön koşul: RECEIPT'in ilk sunumu aynı oturumda Backpressure almalı"
    );
    assert!(
        !attempts_now.iter().any(|(_, result)| result.is_ok()),
        "ön koşul: kuyruk doluyken RECEIPT kabul edilmemeli"
    );

    let after = rig.open_capacity_and_run_turns();
    rig.assert_same_session_and_capacity_opened(&after);

    let attempts = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    let accepted = accepted_on(&attempts, session);
    let tries = attempts.len();
    let opened = after.bob_accepted;
    let unsent = rig.bob.messenger.stats().receipts_unsent;
    assert!(
        accepted >= 1,
        "R3: RECEIPT({msg_id}) aynı oturumda ({session:?}) Transport'a hiç kabul ettirilmedi. \
         İlk sunum Backpressure aldı; sonra kapasite açıldı (turlarda Bob'un {opened} sunumu kabul \
         edildi) ve oturum değişmedi. Reddedilen RECEIPT için bekleyen kayıt yok ve turun hiçbir \
         çağrısı onu yeniden sunmuyor (send_receipt tek deneme, messaging.rs:1110-1136). RECEIPT \
         sunum denemesi: {tries}, receipts_unsent: {unsent}. Alice'e Delivered gelmez; tekrar TEXT \
         ancak oturum değişiminde gelir (K4G-05d)."
    );
    let delivered_count = count_delivered(&after.alice, msg_id);
    assert_eq!(
        delivered_count, 1,
        "RECEIPT kabul edildi ama Alice tam bir Delivered({msg_id}) görmeli"
    );
}

/// **R3 sınırlılık şartı (tekilleştirme) — tabanda (`3439fccd`) KIRMIZI beklenir; kırmızının
/// nedeni R3'ün kendisidir, tekilleştirme DEĞİL.**
///
/// Senaryo: test 1 ile aynı düzenek. Aynı `(Alice, msg_id)` için `DUPLICATE_TEXTS` TEXT (biri
/// Alice'in gönderimi, gerisi aynı kaydın tekrarları; `text_receipt.rs`
/// `duplicate_text_is_not_redelivered_but_receipted_again` deseni) Bob'a kuyruk doluyken gelir.
/// S2 gereği her TEXT bir RECEIPT sunumu doğurur; hepsi `Backpressure` alır. Kapasite açılınca
/// beklenen (C9 K4G-05d: `(peer, msg_id)` ile tekilleştirilen kuyruk; `Ok` sonrası tekrar
/// Transport'un, ikinci ağ ARQ'su yok): RECEIPT(msg_id) TAM BİR KEZ kabul ettirilir ve ondan sonra
/// yeniden sunulmaz.
///
/// Tabanda durum: bekleyen-RECEIPT kuyruğu YOK; `DUPLICATE_TEXTS` ret iz bırakmaz → kabul sayısı
/// 0 → ilk denetim (R3) düşer. Tekilleştirme yarısı tabanda BOŞ-GEÇİŞTİR (tekilleştirilecek
/// kayıt yok); bu test tabanda tekilleştirme hakkında kanıt DEĞİLDİR. Değeri düzeltmeden sonradır:
/// tekilleştiren kuyruk → 1 (yeşil); tekilleştirmeyen kuyruk → `DUPLICATE_TEXTS` kabul (ikinci
/// denetim düşer); `Ok` sonrası yeniden sunan kuyruk → üçüncü denetim düşer.
///
/// Yazılmayan: eş başına tavan (C9: outbox sınırı, `MAX_OUTBOX_UNACKED` = 256) testi. Tabanda
/// boş-geçiştir (kuyruk yok) ve taşma politikası (hangi kayıt düşer, hangi sayaç) C9'da tanımlı
/// değildir.
#[test]
fn p0_r3_receipt_dedup_bounded() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    let record = text_record(msg_id, TEXT);
    for _ in 1..DUPLICATE_TEXTS {
        rig.link.borrow_mut().inject(Side::Bob, record.clone());
    }

    // Bob'un turu: ilk TEXT teslim edilir, tekrarlar söner; her biri bir RECEIPT sunumu doğurur ve
    // hepsi dolu kuyrukla karşılaşır.
    assert_eq!(
        rig.bob.turn(),
        vec![received(ALICE, msg_id, TEXT)],
        "ön koşul: Bob TEXT'i bir kez teslim etmeli"
    );
    assert_eq!(
        rig.bob.messenger.stats().duplicate_texts,
        DUPLICATE_TEXTS - 1,
        "ön koşul: tekrarlar dedup penceresinde sönmeli"
    );
    let attempts_now = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    assert_eq!(
        attempts_now.first(),
        Some(&(session, Err(TransportError::Backpressure))),
        "ön koşul: RECEIPT'in ilk sunumu aynı oturumda Backpressure almalı"
    );
    assert!(
        !attempts_now.iter().any(|(_, result)| result.is_ok()),
        "ön koşul: kuyruk doluyken RECEIPT kabul edilmemeli"
    );

    let after = rig.open_capacity_and_run_turns();
    rig.assert_same_session_and_capacity_opened(&after);

    let attempts = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    let accepted = accepted_on(&attempts, session);
    let tries = attempts.len();
    let unsent = rig.bob.messenger.stats().receipts_unsent;
    assert_ne!(
        accepted, 0,
        "R3 (tabanda beklenen kırmızı): {DUPLICATE_TEXTS} TEXT'in RECEIPT({msg_id}) sunumlarının \
         hepsi Backpressure aldı ve kapasite açıldıktan sonra hiçbiri yeniden sunulmadı (bekleyen \
         kayıt yok, send_receipt tek deneme). RECEIPT sunum denemesi: {tries}, receipts_unsent: \
         {unsent}. Tekilleştirme şartı tabanda ölçülemez (kuyruk yok)."
    );
    assert_eq!(
        accepted, 1,
        "sınırlılık/tekilleştirme: aynı (Alice, {msg_id}) için bekleyen RECEIPT tek olmalı \
         (K4G-05d); {DUPLICATE_TEXTS} tekrar TEXT'ten sonra {accepted} kez kabul ettirildi"
    );
    let after_ok = attempts
        .iter()
        .skip_while(|(_, result)| result.is_err())
        .skip(1)
        .count();
    assert_eq!(
        after_ok, 0,
        "Ok sonrası tekrar Transport'undur (K4G-05d, ikinci ağ ARQ'su yok): kabul edilen \
         RECEIPT({msg_id}) yeniden sunulmamalı"
    );
    assert_eq!(
        count_delivered(&after.alice, msg_id),
        1,
        "Alice tam bir Delivered({msg_id}) görmeli"
    );
}
