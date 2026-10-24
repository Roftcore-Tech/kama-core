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
//! * **`Link` uzantısı (K4G-05d, not Ek B.2):** supersession ve kapanış (`Link::supersede`,
//!   `Link::close_at`) uca olayları sıralar; tutamaç geçişi uç olayı `poll_event` ile ALIRKEN
//!   yapılır. O ana dek eski tutamaç aktif kalır (yön kuyruğu doluysa `Backpressure`); `Ready(h')`
//!   alındıktan sonra eski tutamaca gönderim `SessionSuperseded`, `Closed(h)` alındıktan sonra
//!   `UnknownSession` döner (S1 F0.2(i)). client-core'un aynasıdır: faz geçişi `drive`'dadır ve
//!   `drive` ürün yolunda yalnız `recv` / `poll_event`'ten çağrılır. `Link::close_on_next_recv`
//!   aynı kuralın `recv` yüzüdür: ucun sıradaki `recv`'i kapanışı öğrenir (tutamaç hemen kapanır,
//!   kuyruktaki birim yine teslim edilir). `Link::lose_next`: uçtan Transport'un kabul ettiği
//!   sıradaki gönderim karşıya ulaşmaz. Uzantı P0 testlerinin yolunu değiştirmez (düğmeler
//!   kapalıyken `send` / `recv` / `poll_event` eskisiyle aynıdır).
//!
//! # K4G-05d testleri (not `docs/design/K4G-05d-RECEIPT.md` rev2 §5, Ek B.2)
//!
//! İki P0 testi düzeltmeyle yeşile döner; gövdeleri ve iddiaları DEĞİŞMEDİ. Aynı düzenekle beş
//! test daha var. Hepsi yalnız tabanda da bulunan API'yi kullanır: dosya tabanda (`3439fccd`)
//! DERLENİR ve sınıflar orada ölçülür.
//!
//! * **Tabanda KIRMIZI:** T1' taşma (`t1p_…`), T5 ret sınırı (`t5_…`).
//! * **Tabanda YEŞİL (bekçi):** T2 `Ok` sonrası (`t2_…`), T3 oturum değişimi (`t3_…`), T4 hata
//!   sınıfı (`t4_…`).
//!
//! Yeni API'yi okuyan iddialar (`MessengerStats::receipts_deferred` / `receipts_evicted` /
//! `receipts_pending`, `Messenger::pending_receipts`: T1 `evicted`, T3b, T4 `pending`, T6 sayaç
//! değişmezi) tabanda derlenmez; `src/messaging.rs` birim testlerindedir.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kama_app::clock::MonotonicClock;
use kama_app::codec::{decode, encode, AmpLimits, AppMessage, ReceiptKind};
use kama_app::file_transfer::FileTransfer;
use kama_app::messaging::{Messenger, MessengerEvent, MsgId, MAX_OUTBOX_UNACKED};
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
/// T1': Bob'a gelen farklı `msg_id` sayısı — eş başına bekleyen RECEIPT tavanının
/// (`MAX_OUTBOX_UNACKED`, outbox tavanıyla aynı; C9: yeni sayı yok) bir fazlası.
const OVERFLOW_TEXTS: usize = MAX_OUTBOX_UNACKED + 1;
/// T1': kapasite açıldıktan sonraki tur sayısı (not Ek B.2). Her turdan önce Alice kuyruğunu okur
/// ve Bob→Alice yönünde en çok `LINK_QUEUE_CAP` yer açar; turun ilk `Messenger::poll`'u bekleyen
/// RECEIPT'leri bu yer kadar kabul ettirir (dosya pump'ı turda sonra koşar). Tavandaki
/// `MAX_OUTBOX_UNACKED` kayıt ⌈`MAX_OUTBOX_UNACKED` / `LINK_QUEUE_CAP`⌉ turda biter; +2 tur
/// kuyruk boşaldıktan sonra hiçbir RECEIPT'in yeniden sunulmadığını ("tam bir kez") gösterir.
const OVERFLOW_ROUNDS: usize = MAX_OUTBOX_UNACKED.div_ceil(LINK_QUEUE_CAP) + 2;

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
    /// [K4G-05d] `Ready(h')`'si bu uçta alınmış, `Closed`'ı henüz alınmamış eski tutamaçlar:
    /// `send` → `SessionSuperseded` (S1 F0.2(i)); `Closed` alınınca `UnknownSession`.
    superseded: Vec<SessionHandle>,
    /// [K4G-05d] Bu uçtan Transport'un kabul ettiği sıradaki bu kadar gönderim KAYBOLUR: `Ok`
    /// döner ama karşı uca ulaşmaz (kurtarma yok).
    lose_next: usize,
    /// [K4G-05d] `Some(neden)`: bu ucun sıradaki `recv`'i oturumun kapandığını öğrenir — aktif
    /// tutamaç hemen kapanır (`send` → `UnknownSession`) ve uca `Closed(h, neden)` sıralanır;
    /// kuyruktaki birim yine teslim edilir (client-core `recv` → `drive` aynası).
    close_on_recv: Option<CloseReason>,
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

    /// Yeni tutamaç değeri.
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

    /// [K4G-05d] Supersession (S1 K7): her uca `Ready(h')` sonra `Closed(h, Superseded)` sıralanır;
    /// `h'` döner. Geçiş uç `Ready(h')`'yi `poll_event` ile alırken yapılır (`End::poll_event`); o
    /// ana dek `h` o uçta aktif kalır.
    fn supersede(&mut self) -> SessionHandle {
        let handle = self.fresh_handle();
        for side in [Side::Alice, Side::Bob] {
            let endpoint = self.endpoint(side);
            let old = endpoint.active.expect("supersede: aktif oturum yok");
            endpoint.push_event(SessionEvent::Ready(handle, side.remote()));
            endpoint.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
        }
        handle
    }

    /// [K4G-05d] Transport `side` ucunda aktif oturumu kapatır: uca `Closed(h, reason)` sıralanır;
    /// `h` döner. Geçiş olay `poll_event` ile alınırken yapılır; o ana dek `h` aktif kalır.
    fn close_at(&mut self, side: Side, reason: CloseReason) -> SessionHandle {
        let endpoint = self.endpoint(side);
        let handle = endpoint.active.expect("close_at: aktif oturum yok");
        endpoint.push_event(SessionEvent::Closed(handle, reason));
        handle
    }

    /// [K4G-05d] `side` ucunun sıradaki `recv`'i oturumun kapandığını öğrenir
    /// (`Endpoint::close_on_recv`).
    fn close_on_next_recv(&mut self, side: Side, reason: CloseReason) {
        self.endpoint(side).close_on_recv = Some(reason);
    }

    /// [K4G-05d] `side` ucundan Transport'un kabul ettiği sıradaki `n` gönderim kaybolur.
    fn lose_next(&mut self, side: Side, n: usize) {
        self.endpoint(side).lose_next += n;
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

    /// [K4G-05d] `from` ucunun `session`'a kabul ettirdiği `RECEIPT(delivered)` sayısı,
    /// `ref_msg_id` başına (kaydın tek geçişiyle).
    fn accepted_receipts(&self, from: Side, session: SessionHandle) -> BTreeMap<MsgId, usize> {
        let limits = limits();
        let mut accepted: BTreeMap<MsgId, usize> = BTreeMap::new();
        for attempt in &self.endpoint_ref(from).sends {
            if attempt.session != session || attempt.result.is_err() {
                continue;
            }
            if let Ok(AppMessage::Receipt {
                ref_msg_id,
                kind: ReceiptKind::Delivered,
            }) = decode(&attempt.unit, &limits)
            {
                *accepted.entry(ref_msg_id).or_default() += 1;
            }
        }
        accepted
    }

    /// [K4G-05d] `from` ucunun `send` denemesi sayısı (kayıt konumu).
    fn send_count(&self, from: Side) -> usize {
        self.endpoint_ref(from).sends.len()
    }

    /// [K4G-05d] `from` ucunun `since` konumundan sonraki `Backpressure` alan denemeleri.
    fn backpressure_since(&self, from: Side, since: usize) -> usize {
        self.endpoint_ref(from).sends[since..]
            .iter()
            .filter(|attempt| attempt.result == Err(TransportError::Backpressure))
            .count()
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
        let result = if me.superseded.contains(&session) {
            // [K4G-05d] `Ready(h')` alındı, `Closed(h)` alınmadı (S1 F0.2(i)).
            Err(TransportError::SessionSuperseded)
        } else if me.active != Some(session) {
            Err(TransportError::UnknownSession)
        } else if unit.bytes.len() > LINK_MAX_UNIT_BYTES {
            Err(TransportError::UnitTooLarge)
        } else if remote.inbox.len() >= LINK_QUEUE_CAP {
            // Yönün kuyruğu dolu: ürünün dolu offer kanalı (`QueueFull` → `Backpressure`).
            Err(TransportError::Backpressure)
        } else if me.lose_next > 0 {
            // [K4G-05d] Kabul edildi, yolda kayboldu: karşı uca ulaşmaz.
            me.lose_next -= 1;
            Ok(())
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
        let mut link = self.link.borrow_mut();
        let endpoint = link.endpoint(self.side);
        if let Some(reason) = endpoint.close_on_recv.take() {
            // [K4G-05d] Bu sürüş kapanışı öğrenir: tutamaç hemen kapanır, olay sıralanır; zaten
            // teslim kuyruğunda olan birim yine verilir.
            if let Some(handle) = endpoint.active.take() {
                endpoint.push_event(SessionEvent::Closed(handle, reason));
            }
        }
        endpoint.inbox.pop_front()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        let mut link = self.link.borrow_mut();
        let endpoint = link.endpoint(self.side);
        let event = endpoint.events.pop_front()?;
        // [K4G-05d] Tutamaç geçişi olay alınırken (`Link::supersede`, `Link::close_at`).
        match event {
            SessionEvent::Ready(handle, _) => {
                if let Some(old) = endpoint.active.replace(handle) {
                    if old != handle {
                        endpoint.superseded.push(old);
                    }
                }
            }
            SessionEvent::Closed(handle, _) => {
                endpoint.superseded.retain(|old| *old != handle);
                if endpoint.active == Some(handle) {
                    endpoint.active = None;
                }
            }
            SessionEvent::RenewRequired(_)
            | SessionEvent::ConnectAbandoned(..)
            | SessionEvent::ConnectProgress(..) => {}
        }
        Some(event)
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
    /// Kurulan ilk oturum. P0 testlerinde ve T1' / T2 / T5'te hiçbir adımda değişmez; oturum
    /// değişimini sınayan T3 / T4 yenisini kendisi tutar.
    session: SessionHandle,
}

impl Rig {
    /// [K4G-05d] Oturum kurulu ve iki taraf `Ready`'yi okumuş; yolda hiçbir şey yok.
    fn connected() -> Self {
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
        rig
    }

    /// Oturum kurulu ve iki taraf `Ready`'yi okumuş; Bob Alice'e büyük bir dosya gönderiyor ve
    /// dosya pump'ı Bob→Alice kuyruğunu reddedilene dek doldurmuş (`kama-node` sırası: `send_file`
    /// komutu → tur → kuyruk boş → `FileTransfer::pump`).
    fn with_full_bob_to_alice_queue() -> Self {
        let mut rig = Self::connected();
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
        self.run_rounds(RECOVERY_ROUNDS)
    }

    /// [K4G-05d] [`Rig::open_capacity_and_run_turns`]'ün tur sayısı verilen biçimi (T1':
    /// `OVERFLOW_ROUNDS`).
    fn run_rounds(&mut self, rounds: usize) -> AfterOpening {
        let accepted_before = self.link.borrow().accepted_sends(Side::Bob);
        let mut alice = Vec::new();
        let mut bob = Vec::new();
        for _ in 0..rounds {
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

// ── K4G-05d testleri (not rev2 §5, Ek B.2; tabanda derlenir) ────────────────────────────────

/// **T1' taşma (DUR-05d-1: en eski düşer) — tabanda (`3439fccd`) KIRMIZI beklenir.**
///
/// Amaç: eş başına bekleyen RECEIPT kuyruğu `MAX_OUTBOX_UNACKED` ile sınırlıdır ve taşmada EN ESKİ
/// kayıt düşer; geri kalanların her biri kapasite açılınca AYNI oturumda tam bir kez kabul edilir.
/// Senaryo (tek oturum): Bob→Alice kuyruğu dolu tutulur (Alice okumaz); Bob'a `OVERFLOW_TEXTS`
/// farklı `msg_id`'li (0, 1, …) ham TEXT `LINK_QUEUE_CAP`'lik partilerle gelir, her partiden sonra
/// Bob'un turu koşar — tavanı gönderende tutmayan uyumsuz eş. Sonra kapasite açılır ve Bob
/// `OVERFLOW_ROUNDS` tur çalışır. Beklenen: `msg_id` 0'ın RECEIPT'i hiç kabul edilmez; kalan
/// `MAX_OUTBOX_UNACKED` `msg_id`'nin her birininki tam bir kez kabul edilir. Sayaç yarısı
/// (`receipts_evicted` = 1) yeni API'dir: `src/messaging.rs` birim testi T1.
///
/// Tabanda neden kırmızı: bekleyen kayıt yoktur; her RECEIPT'in tek sunumu `Backpressure` alır ve
/// kaybolur, kapasite açılınca hiçbiri yeniden sunulmaz → "tam bir kez" denetimi 0 kabulle düşer.
/// Mutantlar (Ek B.2): sınır kaldırılırsa 0'ın RECEIPT'i de kabul edilir (ilk denetim düşer); en
/// yeni düşürülürse 0'ınki kabul edilir, en yeni `msg_id`'ninki edilmez.
#[test]
fn t1p_overflow_drops_only_the_oldest_receipt() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let ids: Vec<MsgId> = (0..OVERFLOW_TEXTS)
        .map(|n| MsgId::try_from(n).expect("küçük sayı"))
        .collect();
    for batch in ids.chunks(LINK_QUEUE_CAP) {
        for &msg_id in batch {
            rig.link
                .borrow_mut()
                .inject(Side::Bob, text_record(msg_id, TEXT));
        }
        let expected: Vec<MessengerEvent> = batch
            .iter()
            .map(|&msg_id| received(ALICE, msg_id, TEXT))
            .collect();
        assert_eq!(
            rig.bob.turn(),
            expected,
            "ön koşul: her TEXT bir kez teslim edilir"
        );
    }
    assert!(
        rig.link
            .borrow()
            .accepted_receipts(Side::Bob, session)
            .is_empty(),
        "ön koşul: Bob→Alice kuyruğu dolu kaldı, hiçbir RECEIPT kabul edilmedi"
    );

    let after = rig.run_rounds(OVERFLOW_ROUNDS);
    rig.assert_same_session_and_capacity_opened(&after);

    let accepted = rig.link.borrow().accepted_receipts(Side::Bob, session);
    let oldest = ids[0];
    assert_eq!(
        accepted.get(&oldest),
        None,
        "taşma: en eski RECEIPT({oldest}) düşmeliydi ama kabul edildi (sınır yok ya da en yeni \
         düşürülüyor)"
    );
    let not_once: Vec<(MsgId, usize)> = ids[1..]
        .iter()
        .map(|&msg_id| (msg_id, accepted.get(&msg_id).copied().unwrap_or(0)))
        .filter(|&(_, count)| count != 1)
        .collect();
    assert!(
        not_once.is_empty(),
        "T1' (tabanda beklenen kırmızı): en eski dışındaki {} RECEIPT'in her biri aynı oturumda \
         ({session:?}) tam bir kez kabul edilmeli; {} msg_id'de değil, ilk sekizi (msg_id, kabul): \
         {:?}. Bekleyen kayıt yoksa ilk sunum Backpressure alır ve RECEIPT kaybolur.",
        ids.len() - 1,
        not_once.len(),
        &not_once[..not_once.len().min(8)]
    );
}

/// **T2 `Ok` sonrası (C9: "`Ok` sonrası tekrar transport'un") — tabanda (`3439fccd`) YEŞİL
/// (bekçi).**
///
/// Amaç: bekleyen kuyruk ikinci bir ağ ARQ'su DEĞİLDİR — Transport'un kabul ettiği RECEIPT, yolda
/// kaybolsa da aynı oturuma bir daha sunulmaz. Senaryo: P0-1 düzeneği; RECEIPT(X)'in ilk sunumu
/// `Backpressure` alır. Bob'dan Transport'un kabul edeceği sıradaki gönderim kaybolacak biçimde
/// ayarlanır (`Link::lose_next`); kapasite açılır ve Bob `RECOVERY_ROUNDS` tur çalışır. Düzeltmeyle
/// ilk turun başındaki sunum RECEIPT(X)'i kabul ettirir (kaybolur) ve kayıt kuyruktan çıkar.
/// Beklenen: RECEIPT(X) en çok bir kez kabul edilir, kabulden sonra hiç sunulmaz ve Alice
/// `Delivered(X)` görmez — telafi oturum değişimidir (S2).
///
/// Tabanda neden yeşil: RECEIPT(X) hiç yeniden sunulmaz (0 kabul; kayıp düğmesini bir dosya
/// kaydı tüketir); denetimler orada boş geçer, test bekçidir. Değeri düzeltmeden sonradır:
/// `Ok`'ta kaydı çıkarmayan kuyruk RECEIPT(X)'i yeniden sunar (ikinci denetim düşer).
#[test]
fn t2_an_accepted_receipt_is_never_offered_again_even_if_lost() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
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

    rig.link.borrow_mut().lose_next(Side::Bob, 1);
    let after = rig.open_capacity_and_run_turns();
    rig.assert_same_session_and_capacity_opened(&after);

    let attempts = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    let accepted = accepted_on(&attempts, session);
    assert!(
        accepted <= 1,
        "RECEIPT({msg_id}) aynı oturumda {accepted} kez kabul edildi; en çok bir kez olmalı"
    );
    let after_ok = attempts
        .iter()
        .skip_while(|(_, result)| result.is_err())
        .skip(1)
        .count();
    assert_eq!(
        after_ok, 0,
        "Ok sonrası tekrar Transport'undur: kabul edilen RECEIPT({msg_id}) yolda kaybolsa da \
         yeniden sunulmamalı (zamanlayıcı yok, ikinci ağ ARQ'su yok)"
    );
    assert_eq!(
        count_delivered(&after.alice, msg_id),
        0,
        "kaybolan RECEIPT({msg_id}) için Delivered olamaz; telafi oturum değişimidir"
    );
}

/// **T3 oturum değişimi (DUR-05d-4: düşür) — tabanda (`3439fccd`) YEŞİL (bekçi).**
///
/// Amaç: eşin oturumu değişince bekleyen RECEIPT DÜŞER; telafi gönderenin yeni oturumdaki yeniden
/// gönderimidir ve onun tekrar TEXT'i RECEIPT'i yeniden üretir → `h'`'de RECEIPT(X) TAM BİR KEZ
/// kabul edilir. Senaryo: P0 düzeneği (Bob→Alice dolu); Alice X'i yollar, Bob'un RECEIPT(X) sunumu
/// `Backpressure` alır. Supersession `h` → `h'`, geçiş `poll_event`'te (Ek B.2). Sıra: (1) Bob
/// `Ready(h')`'yi okur — o poll'un başında RECEIPT(X) hâlâ aktif ve dolu `h`'ye sunulur ve
/// `Backpressure` alır, sonra `adopt` kuyruğu boşaltır; (2) Alice kuyruğunu okur (yer açılır),
/// `h'`'ye geçer ve X'i yeniden yollar; (3) Bob'un turu tekrar X'i işler.
///
/// Tabanda neden yeşil: RECEIPT(X) tabanda reddedilince kaybolur; `h'`'deki tek RECEIPT tekrar
/// TEXT'ten gelir. Boşaltmayan `adopt` mutantında X'in kaydı `h'`'de bekler: (3)'ün poll başı onu
/// kabul ettirir, ardından tekrar TEXT'in RECEIPT'i ikinci kez kabul edilir → `h'`'de 2 kabul.
#[test]
fn t3_session_change_drops_the_pending_receipt_and_the_repeat_text_receipts_once() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let old = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    assert_eq!(
        rig.bob.turn(),
        vec![received(ALICE, msg_id, TEXT)],
        "ön koşul: Bob TEXT'i teslim etmeli"
    );
    assert_eq!(
        rig.link
            .borrow()
            .receipt_attempts(Side::Bob, msg_id)
            .first(),
        Some(&(old, Err(TransportError::Backpressure))),
        "ön koşul: RECEIPT'in ilk sunumu eski oturumda Backpressure almalı"
    );

    let new = rig.link.borrow_mut().supersede();
    // (1) Bob oturum değişimini okur; Alice henüz okumadı, Bob→Alice yönü dolu.
    assert_eq!(
        rig.bob.turn(),
        vec![
            MessengerEvent::Session(SessionEvent::Ready(new, ALICE)),
            MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ],
        "(1) Bob Ready(h') ve Closed(h) okur"
    );
    assert_eq!(rig.bob.messenger.session_of(ALICE), Some(new));
    // (2) Alice okur: `h'`'ye geçer, X'i yeniden yollar, Bob'un dosya kayıtlarını okur.
    let alice_switch = rig.drain_alice();
    let switch_events = [
        MessengerEvent::Session(SessionEvent::Ready(new, BOB)),
        MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::Superseded)),
    ];
    assert_eq!(
        alice_switch.get(..switch_events.len()),
        Some(&switch_events[..]),
        "(2) Alice Ready(h') ve Closed(h) okur"
    );
    assert_eq!(
        rig.alice.stats().resent_on_new_session,
        1,
        "(2) X h' üzerinden yeniden gönderildi"
    );
    assert!(
        rig.link.borrow().endpoint_ref(Side::Alice).inbox.is_empty(),
        "(2) Alice okudu: Bob→Alice yönünde yer açıldı"
    );
    // (3) Bob'un turu: tekrar X → RECEIPT(X).
    rig.clock.advance(TURN_TICK);
    assert!(rig.bob.turn().is_empty(), "(3) tekrar TEXT olay üretmez");
    assert_eq!(
        rig.bob.messenger.stats().duplicate_texts,
        1,
        "(3) X h' üzerinden tekrar geldi"
    );
    let alice_after = rig.drain_alice();

    let attempts = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    assert_eq!(
        accepted_on(&attempts, old),
        0,
        "eski oturumda RECEIPT({msg_id}) kabul edilmez (yön doluydu)"
    );
    assert_eq!(
        accepted_on(&attempts, new),
        1,
        "oturum değişimi bekleyen RECEIPT({msg_id})'yi düşürmeli: h''de tek RECEIPT tekrar TEXT'in \
         olmalı (2 kabul = adopt kuyruğu boşaltmadı). Denemeler: {attempts:?}"
    );
    let delivered_count =
        count_delivered(&alice_switch, msg_id) + count_delivered(&alice_after, msg_id);
    assert_eq!(
        delivered_count, 1,
        "Alice tam bir Delivered({msg_id}) görmeli"
    );
}

/// T4'ün iki durumu: eşin oturumu, Bob'un RECEIPT(X)'i üretmesinden önce ya da o sırada biter.
#[derive(Debug, Clone, Copy)]
enum SessionEnd {
    /// Kapanış olayı X'ten önce okunur (olaylar birimlerden önce): X eski tutamaçla, Bob'un
    /// oturumu yokken işlenir — "oturum yok".
    BeforeText,
    /// Bob'un Transport'u kapanışı X'i teslim eden `recv`'de öğrenir: RECEIPT(X)'in tek sunumu
    /// `UnknownSession` alır (Messenger olayı henüz okumadı).
    DuringRecv,
}

/// **T4 hata sınıfı (DUR-05d-3: yalnız `Backpressure` geçici) — tabanda (`3439fccd`) YEŞİL
/// (bekçi).**
///
/// Amaç: oturum yokken doğan ya da `Backpressure` dışı bir hatayla (burada `UnknownSession`)
/// reddedilen RECEIPT bırakılır ve AYNI oturuma bir daha sunulmaz; RECEIPT(X) yalnız gönderenin
/// yeni oturumdaki tekrar TEXT'inden gelir. Senaryo (dosya yok): Alice X'i yollar; oturum iki
/// durumdan biriyle biter ([`SessionEnd`]); Bob bir tur daha koşar; sonra Alice kapanışı okur,
/// yeni oturum kurulur, Alice X'i yeniden yollar ve Bob'un turu tekrar TEXT'i işler. Beklenen:
/// RECEIPT(X) denemeleri tam olarak şunlardır — (yalnız `DuringRecv`) eski tutamaçta bir
/// `UnknownSession`, sonra `h'`'de bir kabul; Alice tam bir `Delivered(X)` görür. Kuyruk boyu
/// yarısı (`pending` 0) yeni API'dir: `src/messaging.rs` T4.
///
/// Tabanda neden yeşil: `send_receipt` tek denemedir ve oturum yoksa sunmaz — beklenen sıranın
/// aynısı. Geçici sınıfı genişleten mutant (`UnknownSession` geçici sayılır) `DuringRecv`'de kaydı
/// tutar ve sıradaki poll başında eski tutamaca yeniden sunar → fazladan bir `UnknownSession`.
#[test]
fn t4_a_non_backpressure_end_leaves_the_receipt_to_the_repeat_text() {
    for case in [SessionEnd::BeforeText, SessionEnd::DuringRecv] {
        let mut rig = Rig::connected();
        let old = rig.session;
        let msg_id = rig
            .alice
            .send_text(BOB, TEXT)
            .expect("Alice→Bob yönünde yer var");
        let text = received(ALICE, msg_id, TEXT);
        let closed = MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::PeerClosed));
        let (bob_events, refused) = match case {
            SessionEnd::BeforeText => {
                rig.link
                    .borrow_mut()
                    .close_at(Side::Bob, CloseReason::PeerClosed);
                (vec![closed, text], Vec::new())
            }
            SessionEnd::DuringRecv => {
                rig.link
                    .borrow_mut()
                    .close_on_next_recv(Side::Bob, CloseReason::PeerClosed);
                (
                    vec![text, closed],
                    vec![(old, Err(TransportError::UnknownSession))],
                )
            }
        };
        assert_eq!(
            rig.bob.turn(),
            bob_events,
            "{case:?}: Bob X'i teslim eder ve kapanışı okur"
        );
        // Yeni oturumdan önce bir tur daha: geçici sınıf genişletilmiş olsaydı RECEIPT(X) burada
        // (ya da önceki turun ikinci poll'unda) eski tutamaca yeniden sunulurdu.
        rig.clock.advance(TURN_TICK);
        assert!(rig.bob.turn().is_empty(), "{case:?}: oturumsuz tur boştur");
        assert_eq!(
            rig.link.borrow().receipt_attempts(Side::Bob, msg_id),
            refused,
            "{case:?}: bırakılan RECEIPT({msg_id}) aynı oturuma yeniden sunulmamalı"
        );

        // Telafi (S2): Alice kapanışı okur, yeni oturum kurulur, Alice X'i yeniden yollar.
        rig.link
            .borrow_mut()
            .close_at(Side::Alice, CloseReason::PeerClosed);
        assert_eq!(
            rig.drain_alice(),
            vec![MessengerEvent::Session(SessionEvent::Closed(
                old,
                CloseReason::PeerClosed
            ))],
            "{case:?}"
        );
        let new = rig.link.borrow_mut().establish();
        assert_eq!(
            rig.drain_alice(),
            vec![MessengerEvent::Session(SessionEvent::Ready(new, BOB))],
            "{case:?}"
        );
        assert_eq!(
            rig.alice.stats().resent_on_new_session,
            1,
            "{case:?}: X h' üzerinden yeniden gönderildi"
        );
        assert_eq!(
            rig.bob.turn(),
            vec![MessengerEvent::Session(SessionEvent::Ready(new, ALICE))],
            "{case:?}: tekrar TEXT olay üretmez"
        );
        assert_eq!(rig.bob.messenger.stats().duplicate_texts, 1, "{case:?}");
        let mut expected = refused;
        expected.push((new, Ok(())));
        assert_eq!(
            rig.link.borrow().receipt_attempts(Side::Bob, msg_id),
            expected,
            "{case:?}: RECEIPT({msg_id}) yalnız tekrar TEXT'ten (h') gelmeli"
        );
        assert_eq!(
            rig.drain_alice(),
            vec![delivered(BOB, msg_id)],
            "{case:?}: Alice tam bir Delivered görür"
        );
    }
}

/// **T5 ret sınırı (not §3.3 "Sonuç") — tabanda (`3439fccd`) KIRMIZI beklenir (taban: 2 ret).**
///
/// Amaç: `Backpressure` sürerken bir `Messenger::poll` çağrısı eş başına en çok BİR reddedilen
/// sunum yapar: dolu kuyruğu yalnız poll başı sunar ve ilk `Backpressure`'da durur; kuyruk doluyken
/// yeni RECEIPT hemen sunulmaz, sona eklenir; aynı `msg_id`'nin tekrarı bekleyenle birleşir.
/// Senaryo: P0 düzeneği (Bob→Alice dolu); Alice X1 ve X2'yi yollar, Bob'un turu ikisini de teslim
/// eder (RECEIPT'ler bekler). Sonra Bob'a önce X1'in tekrarı, ardından yeni Y gelir ve Bob'un
/// Messenger'ı TEK `poll` çağrısıyla ikisini de işler (tekrar olay üretmez, aynı çağrıda atlanır).
/// Beklenen: o çağrıda Bob'un `Backpressure` alan denemesi ≤ 1 VE o tek sunum dolu kuyruğun başı
/// X1'indir — dolu kuyruk poll BAŞINDA sunulur: X1 o çağrıda tam bir kez sunulur ve
/// `Backpressure` alır.
///
/// Tabanda neden kırmızı: her TEXT kendi RECEIPT'ini hemen sunar — X1'in tekrarı ve Y için iki
/// `Backpressure` (2 > 1; ilk denetim düşer). Mutantlar (Ek B.2): kuyruk doluyken de hemen sunan
/// (poll başı X1 + Y) ve `Backpressure`'da sunumu sürdüren (poll başı X1 + X2) kuyruk da 2 ret
/// yapar (ilk denetim). Poll başı sunumu kaldıran ya da poll'un sonuna (boşta, `None` dönüşüne)
/// taşıyan kuyruk bu çağrıda X1'i hiç sunmaz — 0 ret ilk denetimi geçer, X1 denetimi düşer.
#[test]
fn t5_at_most_one_backpressure_per_poll_while_receipts_wait() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let first = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    let second = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    assert_eq!(
        rig.bob.turn(),
        vec![received(ALICE, first, TEXT), received(ALICE, second, TEXT)],
        "ön koşul: Bob iki TEXT'i teslim etmeli"
    );
    assert_eq!(
        rig.link.borrow().receipt_attempts(Side::Bob, first).first(),
        Some(&(session, Err(TransportError::Backpressure))),
        "ön koşul: X1'in RECEIPT'inin ilk sunumu Backpressure almalı"
    );
    assert!(
        rig.link
            .borrow()
            .accepted_receipts(Side::Bob, session)
            .is_empty(),
        "ön koşul: kuyruk doluyken RECEIPT kabul edilmemeli"
    );

    // Aynı poll'da: önce X1'in tekrarı, sonra yeni Y.
    rig.link
        .borrow_mut()
        .inject(Side::Bob, text_record(first, TEXT));
    let third = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    let since = rig.link.borrow().send_count(Side::Bob);
    let first_offered_before = rig.link.borrow().receipt_attempts(Side::Bob, first).len();
    let duplicates_before = rig.bob.messenger.stats().duplicate_texts;
    assert_eq!(
        rig.bob.messenger.poll(),
        Some(received(ALICE, third, TEXT)),
        "ön koşul: tek poll X1'in tekrarını söndürür ve Y'yi teslim eder"
    );
    assert_eq!(
        rig.bob.messenger.stats().duplicate_texts,
        duplicates_before + 1,
        "ön koşul: X1'in tekrarı aynı poll'da işlendi"
    );
    let refused = rig.link.borrow().backpressure_since(Side::Bob, since);
    assert!(
        refused <= 1,
        "T5 (tabanda beklenen kırmızı): Backpressure sürerken tek poll çağrısında Bob {refused} \
         reddedilen sunum yaptı; eş başına en çok 1 olmalı (dolu kuyruk yalnız poll başı ve ilk \
         Backpressure'a dek sunulur, yeni RECEIPT sona eklenir, tekrar birleşir)"
    );
    let first_offered_in_poll =
        rig.link.borrow().receipt_attempts(Side::Bob, first)[first_offered_before..].to_vec();
    assert_eq!(
        first_offered_in_poll,
        vec![(session, Err(TransportError::Backpressure))],
        "dolu kuyruk poll BAŞINDA sunulur: bu poll'da kuyruğun başı RECEIPT({first}) tam bir kez \
         sunulmalı ve Backpressure almalı (poll başı sunumu kaldıran ya da poll sonuna taşıyan \
         kuyruk onu hiç sunmaz)"
    );
}
