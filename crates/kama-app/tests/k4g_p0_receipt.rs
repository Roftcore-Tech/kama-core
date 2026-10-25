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
//! * **`Link` uzantısı (K4G-05d rev3):** `Link::supersede_on_next_recv` geçişin `recv` yüzüdür —
//!   ucun sıradaki `recv`'i supersession'ı öğrenir: eski tutamaç hemen eskir (`send` →
//!   `SessionSuperseded`), yeni tutamaç aktif olur ve uca `Closed(eski)` SONRA `Ready(yeni)`
//!   sıralanır (ters sıra; sahte Transport). `Link::supersede` zincirlenebilir: eski tutamaç uca son
//!   duyurulandır (`Endpoint::announced`), uç olayları henüz okumamış olabilir.
//!
//! # K4G-05d testleri (not `docs/design/K4G-05d-RECEIPT.md` rev3 §5, Ek B)
//!
//! İki P0 testi düzeltmeyle yeşile döner; gövdeleri ve iddiaları DEĞİŞMEDİ. Değişmeyen dal
//! testleri: T2, T5. rev3 ile değişen: T3 (taşınan RECEIPT + tekrar TEXT'in RECEIPT'i: `h'`'de 2
//! kabul), T4 (DUR-05d-5 (a) tut-ve-dur). Çıkan: T1' (rev2 düşürme yolu). Yeni (rev3 Ek B.1):
//! (a1) oturum değişmeden iki yönlü doygunluk, (a2) ortada supersede, (b) 300 TEXT tek yön, (b1)
//! pencere içinde > 256 yükümlülük, (b2) pencere ihlali, (c) öncelik kapısı ve muafiyetler, (d)
//! oturum değişiminde kuyruk {a, b, c}, (e) kapanış borcu {bir kez hata, yeniden başlatma}.
//!
//! Yeni API'yi okuyan birim iddiaları (`Messenger::pending_receipts`, sayaç değişmezi, kapı,
//! `adopt` sırası) `src/messaging.rs`'tedir.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kama_app::clock::MonotonicClock;
use kama_app::codec::{decode, encode, AmpLimits, AppMessage, PttCodec, ReceiptKind};
use kama_app::file_transfer::{FileTransfer, MAX_REASSEMBLY_BYTES};
use kama_app::messaging::{
    Messenger, MessengerError, MessengerEvent, MsgId, MAX_OUTBOX_UNACKED, MSG_DEDUP_WINDOW,
};
use kama_app::ptt::{Ptt, PttError, PttEvent, StreamId};
use kama_app::signaling::{Signaling, SignalingError, SignalingEvent};
use kama_app::store::{
    AppStore, AppStoreError, CounterKind, DedupWindow, MemoryAppStore, OutboxUnit,
    FIRST_COUNTER_VALUE,
};
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
/// Uç başına `send` denemesi kaydının tavanı (düzenek sınırı; aşımı senaryo hatasıdır). rev3 (a)
/// tur tavanında (`A_ROUND_CAP`) tur başına ret denemeleri dahil sığacak değer.
const SEND_LOG_CAP: usize = 16_384;
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
/// PTT `bundle_ms` (yorumlanmaz; değeri önemsiz).
const BUNDLE_MS: u8 = 20;
/// rev3 (a): her uçtan karşıya TEXT sayısı — gönderen outbox tavanı (S7).
const A_TEXTS: usize = MAX_OUTBOX_UNACKED;
/// rev3 (a): uç başına PTT demeti.
const A_BUNDLES: usize = 32;
/// rev3 (a), (c): demet yükü (bayt; değeri önemsiz).
const BUNDLE_BYTES: usize = 8;
/// Dosyanın kayıt sayısı üst sınırı: parça yükü birim tavanının yarısından büyüktür (A2: kayıt
/// başlığı + 18 B önek yarım birimden küçük), dolayısıyla ⌈FILE_BYTES / (birim/2)⌉ + BEGIN + END.
const FILE_RECORDS_BOUND: usize = FILE_BYTES.div_ceil(LINK_MAX_UNIT_BYTES / 2) + 2;
/// rev3 (a): tur tavanı. Yön başına taşınacak birim ≤ 2·`A_TEXTS` (TEXT + RECEIPT) + dosya + demet
/// + 2 (`PTT_OPEN`, `FILE_RECEIPT`); yön başına turda ≤ `LINK_QUEUE_CAP` birim akar. Tavan bu alt
/// sınırın 4 katıdır: aşılırsa ilerleme durmuştur (kilit) ya da hizmet adaletsizdir — ikisi de
/// bulgudur, tavan büyütülmez.
const A_ROUND_CAP: usize =
    (2 * A_TEXTS + FILE_RECORDS_BOUND + A_BUNDLES + 2).div_ceil(LINK_QUEUE_CAP) * 4;
/// rev3 (a2): supersession'ın yapıldığı tur (iki yön doluyken).
const A2_SUPERSEDE_ROUND: usize = 4;
/// rev3 (b), (b1): tek yönde TEXT sayısı (`MAX_OUTBOX_UNACKED`'ten fazla).
const B_TEXTS: usize = 300;
/// rev3 (b): tur tavanı (`A_ROUND_CAP` ile aynı türetim, tek yön).
const B_ROUND_CAP: usize = (2 * B_TEXTS + FILE_RECORDS_BOUND).div_ceil(LINK_QUEUE_CAP) * 4;
/// rev3 (b1): kapasite açıldıktan sonraki tur sayısı: her tur Bob→Alice yönünde `LINK_QUEUE_CAP`
/// yer açar ve turun ilk `Messenger::poll`'u bekleyen RECEIPT'leri o kadar kabul ettirir (dosya
/// pump'ı kapıda bekler); +2 tur kuyruk boşaldıktan sonra yeniden sunum olmadığını gösterir.
const B1_ROUNDS: usize = B_TEXTS.div_ceil(LINK_QUEUE_CAP) + 2;
/// rev3 (b2): pencere ihlali — ilk partiden sonra eşten gelen, `MSG_DEDUP_WINDOW` ileri `msg_id`.
/// İlk parti `0 .. LINK_QUEUE_CAP`; pencere `B2_FAR − (MSG_DEDUP_WINDOW − 1) ..= B2_FAR` olur ve
/// `0 .. B2_PRUNED` pencere altına düşer.
const B2_FAR: MsgId = MSG_DEDUP_WINDOW + 4;
/// rev3 (b2): budanması beklenen kayıt sayısı (`0 ..= B2_FAR − MSG_DEDUP_WINDOW`).
const B2_PRUNED: MsgId = B2_FAR - MSG_DEDUP_WINDOW + 1;
/// rev3 (b2): kapasite açıldıktan sonraki tur sayısı: kalan kayıtlar tek turda biter (≤
/// `LINK_QUEUE_CAP`); +2 tur yeniden sunum olmadığını gösterir.
const B2_ROUNDS: usize = 3;
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
// (b2): budanan kayıtlar ilk partinin içindedir.
const _: () = assert!((B2_PRUNED as usize) < LINK_QUEUE_CAP);
// (b), (b1): tek yönde gönderen outbox tavanından fazla TEXT.
const _: () = assert!(B_TEXTS > MAX_OUTBOX_UNACKED);

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
    /// [K4G-05d rev3] Uç dizisindeki yeri (Alice 0, Bob 1).
    const fn index(self) -> usize {
        match self {
            Self::Alice => 0,
            Self::Bob => 1,
        }
    }

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
    /// [K4G-05d rev3] Bu uca son duyurulan (olayı sıralanmış) tutamaç: `Link::supersede` zinciri
    /// eski tutamacı buradan alır (uç olayları henüz okumamış olabilir).
    announced: Option<SessionHandle>,
    /// [K4G-05d rev3] `Some(yeni)`: bu ucun sıradaki `recv`'i supersession'ı öğrenir — aktif
    /// tutamaç hemen eskir (`send` → `SessionSuperseded`), `yeni` aktif olur ve uca
    /// `Closed(eski, Superseded)` SONRA `Ready(yeni)` sıralanır (ters sıra; sahte Transport).
    supersede_on_recv: Option<SessionHandle>,
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
            endpoint.announced = Some(handle);
            endpoint.push_event(SessionEvent::Ready(handle, side.remote()));
        }
        handle
    }

    /// [K4G-05d] Supersession (S1 K7): her uca `Ready(h')` sonra `Closed(h, Superseded)` sıralanır;
    /// `h'` döner. Geçiş uç `Ready(h')`'yi `poll_event` ile alırken yapılır (`End::poll_event`); o
    /// ana dek `h` o uçta aktif kalır. [rev3] `h` uca son duyurulan tutamaçtır: iki supersession
    /// arka arkaya yapılabilir.
    fn supersede(&mut self) -> SessionHandle {
        let handle = self.fresh_handle();
        for side in [Side::Alice, Side::Bob] {
            let endpoint = self.endpoint(side);
            let old = endpoint
                .announced
                .expect("supersede: duyurulmuş oturum yok");
            endpoint.announced = Some(handle);
            endpoint.push_event(SessionEvent::Ready(handle, side.remote()));
            endpoint.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
        }
        handle
    }

    /// [K4G-05d rev3] Supersession'ı `side` ucu `recv` anında öğrenir (`Endpoint::supersede_on_recv`);
    /// karşı uca `Link::supersede` gibi `Ready(h')` sonra `Closed(h)` sıralanır. `h'` döner.
    fn supersede_on_next_recv(&mut self, side: Side) -> SessionHandle {
        let handle = self.fresh_handle();
        for each in [Side::Alice, Side::Bob] {
            let endpoint = self.endpoint(each);
            let old = endpoint
                .announced
                .expect("supersede: duyurulmuş oturum yok");
            endpoint.announced = Some(handle);
            if each.index() == side.index() {
                endpoint.supersede_on_recv = Some(handle);
            } else {
                endpoint.push_event(SessionEvent::Ready(handle, each.remote()));
                endpoint.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
            }
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

    /// [K4G-05d rev3] `from` ucunun `session`'a kabul ettirdiği `RECEIPT(delivered)`'ların
    /// `ref_msg_id`'leri, kabul sırasıyla.
    fn accepted_receipt_ids(&self, from: Side, session: SessionHandle) -> Vec<MsgId> {
        let limits = limits();
        self.endpoint_ref(from)
            .sends
            .iter()
            .filter(|attempt| attempt.session == session && attempt.result.is_ok())
            .filter_map(|attempt| match decode(&attempt.unit, &limits) {
                Ok(AppMessage::Receipt {
                    ref_msg_id,
                    kind: ReceiptKind::Delivered,
                }) => Some(ref_msg_id),
                _ => None,
            })
            .collect()
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

    /// [K4G-05d rev3] `from` ucunun `since` konumundan sonraki denemeleri: kayıt türü ve sonuç.
    fn kinds_since(
        &self,
        from: Side,
        since: usize,
    ) -> Vec<(&'static str, Result<(), TransportError>)> {
        let limits = limits();
        self.endpoint_ref(from).sends[since..]
            .iter()
            .map(|attempt| {
                let kind = decode(&attempt.unit, &limits).map_or("ÇÖZÜLEMEZ", |m| kind_of(&m));
                (kind, attempt.result)
            })
            .collect()
    }

    /// [K4G-05d rev3] `from` ucunun `Backpressure` alan RECEIPT sunumları.
    fn receipt_backpressure(&self, from: Side) -> usize {
        self.kinds_since(from, 0)
            .iter()
            .filter(|&&(kind, result)| {
                kind == "RECEIPT" && result == Err(TransportError::Backpressure)
            })
            .count()
    }
}

/// [K4G-05d rev3] Kaydın AMP tür adı (deneme kayıtlarını sınıflamak için).
const fn kind_of(message: &AppMessage) -> &'static str {
    match message {
        AppMessage::Text { .. } => "TEXT",
        AppMessage::FileBegin { .. } => "FILE_BEGIN",
        AppMessage::FileChunk { .. } => "FILE_CHUNK",
        AppMessage::FileEnd { .. } => "FILE_END",
        AppMessage::FileAbort { .. } => "FILE_ABORT",
        AppMessage::PttOpen { .. } => "PTT_OPEN",
        AppMessage::PttClose { .. } => "PTT_CLOSE",
        AppMessage::Receipt { .. } => "RECEIPT",
        AppMessage::SignalingBlob { .. } => "SIGNALING_BLOB",
        AppMessage::MediaBundle { .. } => "MEDIA_BUNDLE",
        AppMessage::FileReceipt { .. } => "FILE_RECEIPT",
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
        if let Some(new) = endpoint.supersede_on_recv.take() {
            // [K4G-05d rev3] Bu sürüş supersession'ı öğrenir: eski tutamaç hemen eskir, yenisi
            // aktif olur; olaylar ters sırayla sıralanır (Closed önce).
            if let Some(old) = endpoint.active.replace(new) {
                endpoint.superseded.push(old);
                endpoint.push_event(SessionEvent::Closed(old, CloseReason::Superseded));
            }
            endpoint.push_event(SessionEvent::Ready(new, self.side.remote()));
        }
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
        Self::connected_with(fresh_store())
    }

    /// [K4G-05d rev3] [`Rig::connected`], Alice'in deposu verilmiş biçimde (kapanış borcu, (e)).
    fn connected_with(alice_store: Arc<dyn AppStore>) -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let clock = ManualClock::started();
        let alice = Messenger::with_store(End::new(&link, Side::Alice), alice_store)
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

    /// [K4G-05d] [`Rig::open_capacity_and_run_turns`]'ün tur sayısı verilen biçimi.
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

// ── K4G-05d rev3 düzenek ekleri ──────────────────────────────────────────────────────────────

/// [K4G-05d rev3 (e)] Bellek içi depo: `msg_id` sayacı testin koyduğu değerden sürer (ileri atlama
/// depo sözleşmesine uyar: değerler kesin artar; tükenme kuralı ürün depolarıyla aynı,
/// [`CounterKind::advance`]) ve `outbox_remove(_, id)` istenen sayıda `Persist` döndürür (hiçbir
/// şey silinmez). Geri kalan her şey bellek içi depoya gider.
struct FlakyRemoveStore {
    inner: MemoryAppStore,
    next_msg_id: Mutex<u64>,
    /// (`msg_id`, kalan hata sayısı).
    failing: Mutex<Option<(MsgId, u32)>>,
}

impl FlakyRemoveStore {
    fn new() -> Self {
        Self {
            inner: MemoryAppStore::new(),
            next_msg_id: Mutex::new(FIRST_COUNTER_VALUE),
            failing: Mutex::new(None),
        }
    }

    /// `msg_id`'nin kalıcı silmesi sıradaki `times` denemede başarısız olur.
    fn fail_remove(&self, msg_id: MsgId, times: u32) {
        *self.failing.lock().expect("kilit zehirlenmedi") = Some((msg_id, times));
    }

    /// Depo iyileşir: silme hatası kalmaz.
    fn heal(&self) {
        *self.failing.lock().expect("kilit zehirlenmedi") = None;
    }

    /// Sıradaki `msg_id` `next` olur (yalnız ileri).
    fn jump_msg_id(&self, next: u64) {
        let mut guard = self.next_msg_id.lock().expect("kilit zehirlenmedi");
        assert!(next >= *guard, "sayaç geri gitmez");
        *guard = next;
    }

    /// Eşin kalıcı outbox'ındaki `msg_id`'ler.
    fn outbox_ids(&self, peer: PeerRef) -> Vec<MsgId> {
        self.inner
            .outbox_list(peer)
            .expect("bellek içi depo okunur")
            .iter()
            .map(OutboxUnit::msg_id)
            .collect()
    }
}

impl AppStore for FlakyRemoveStore {
    fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
        if kind != CounterKind::MsgId {
            return self.inner.reserve_next(peer, kind);
        }
        let mut next = self.next_msg_id.lock().expect("kilit zehirlenmedi");
        let value = *next;
        *next = kind.advance(value)?;
        Ok(value)
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
        {
            let mut failing = self.failing.lock().expect("kilit zehirlenmedi");
            if let Some((id, remaining)) = failing.as_mut() {
                if *id == msg_id && *remaining > 0 {
                    *remaining -= 1;
                    return Err(AppStoreError::Persist);
                }
            }
        }
        self.inner.outbox_remove(peer, msg_id)
    }

    fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
        self.inner.outbox_list(peer)
    }
}

/// [K4G-05d rev3 (a)] Bir ucun gözlemi: gönderdiği TEXT'ler, gördüğü `Delivered` ve
/// `TextReceived` sayıları, PTT demeti, oturum olayı.
#[derive(Default)]
struct SideLog {
    sent: Vec<MsgId>,
    delivered: BTreeMap<MsgId, usize>,
    received: BTreeMap<MsgId, usize>,
    bundles: usize,
    session_events: usize,
}

impl SideLog {
    /// Turun zincir sonuna ulaşan Messenger olaylarını sayar.
    fn record(&mut self, side: Side, events: &[MessengerEvent]) {
        for event in events {
            match event {
                MessengerEvent::Delivered { peer, msg_id } => {
                    assert_eq!(*peer, side.remote(), "{side:?}: Delivered yalnız eşten");
                    *self.delivered.entry(*msg_id).or_default() += 1;
                }
                MessengerEvent::TextReceived { peer, msg_id, .. } => {
                    assert_eq!(*peer, side.remote(), "{side:?}: TEXT yalnız eşten");
                    *self.received.entry(*msg_id).or_default() += 1;
                }
                MessengerEvent::Session(_) => self.session_events += 1,
                MessengerEvent::Other { .. } => {}
            }
        }
    }

    /// Bu uç `A_TEXTS` TEXT gönderdi, hepsinin `Delivered`'ını ve eşin `A_TEXTS` TEXT'ini gördü.
    fn complete(&self) -> bool {
        self.sent.len() == A_TEXTS
            && self.delivered.len() == A_TEXTS
            && self.received.len() == A_TEXTS
    }
}

/// [K4G-05d rev3 (a)] İki uç da `kama-node` motorlarıdır (`Node`): ikisi de dosya gönderir, PTT akışı
/// açar ve karşılıklı TEXT yollar.
struct Duplex {
    link: Rc<RefCell<Link>>,
    clock: ManualClock,
    alice: Node,
    bob: Node,
    /// Kurulan ilk oturum.
    session: SessionHandle,
    /// Uç başına giden PTT akışı (`Side::index` sırasıyla).
    streams: [StreamId; 2],
}

impl Duplex {
    /// Oturum kurulu; iki uç PTT akışını boş kuyrukta açmış (`PTT_OPEN` kabul) ve karşıya
    /// `FILE_BYTES`'lık dosyayı kuyruğa almış. Dolum ilk turda başlar.
    fn started() -> Self {
        let link = Rc::new(RefCell::new(Link::default()));
        let clock = ManualClock::started();
        let node = |side: Side| {
            let messenger = Messenger::with_store(End::new(&link, side), fresh_store())
                .expect("limitler geçerli");
            Node::new(messenger, &clock)
        };
        let mut alice = node(Side::Alice);
        let mut bob = node(Side::Bob);
        let session = link.borrow_mut().establish();
        assert_eq!(
            alice.turn(),
            vec![MessengerEvent::Session(SessionEvent::Ready(session, BOB))]
        );
        assert_eq!(
            bob.turn(),
            vec![MessengerEvent::Session(SessionEvent::Ready(session, ALICE))]
        );
        let mut streams = [0; 2];
        for (node, side) in [(&mut alice, Side::Alice), (&mut bob, Side::Bob)] {
            streams[side.index()] = node
                .ptt
                .open(
                    &mut node.messenger,
                    side.remote(),
                    PttCodec::Opus,
                    BUNDLE_MS,
                )
                .expect("PTT_OPEN boş kuyrukta kabul edilir");
            node.files
                .send_file(
                    &node.messenger,
                    side.remote(),
                    FILE_NAME,
                    vec![FILE_FILL_BYTE; FILE_BYTES],
                )
                .expect("dosya kuyruğa alınır");
        }
        Self {
            link,
            clock,
            alice,
            bob,
            session,
            streams,
        }
    }

    fn node(&mut self, side: Side) -> &mut Node {
        match side {
            Side::Alice => &mut self.alice,
            Side::Bob => &mut self.bob,
        }
    }

    fn node_ref(&self, side: Side) -> &Node {
        match side {
            Side::Alice => &self.alice,
            Side::Bob => &self.bob,
        }
    }

    /// Bir tur: iki ucun uygulama adımı ([`app_step`]), saat, iki ucun `Node::turn`'u. Tur sırası
    /// her turda değişir (çift turda Alice önce): iki yön de "karşı taraf okumadan önce dolu" anını
    /// yaşar.
    fn round(&mut self, round: usize, logs: &mut [SideLog; 2]) {
        for side in [Side::Alice, Side::Bob] {
            let stream = self.streams[side.index()];
            app_step(self.node(side), side, stream, &mut logs[side.index()]);
        }
        self.clock.advance(TURN_TICK);
        let order = if round % 2 == 0 {
            [Side::Alice, Side::Bob]
        } else {
            [Side::Bob, Side::Alice]
        };
        for side in order {
            let events = self.node(side).turn();
            logs[side.index()].record(side, &events);
        }
    }

    /// İki uç tamam ve hiçbir RECEIPT beklemiyor.
    fn complete(&self, logs: &[SideLog; 2]) -> bool {
        [Side::Alice, Side::Bob].into_iter().all(|side| {
            logs[side.index()].complete()
                && self
                    .node_ref(side)
                    .messenger
                    .pending_receipts(side.remote())
                    == 0
        })
    }

    /// Tamamlanana ya da `A_ROUND_CAP`'e dek tur; `supersede_at` turunda supersession. Koşulan tur
    /// sayısını döndürür.
    fn run(&mut self, logs: &mut [SideLog; 2], supersede_at: Option<usize>) -> usize {
        let mut rounds = 0;
        while rounds < A_ROUND_CAP && !self.complete(logs) {
            if supersede_at == Some(rounds) {
                self.link.borrow_mut().supersede();
            }
            self.round(rounds, logs);
            rounds += 1;
        }
        rounds
    }

    /// Bitmeme teşhisi (kilit / açlık): uç başına sayılar.
    fn diagnosis(&self, logs: &[SideLog; 2]) -> String {
        [Side::Alice, Side::Bob]
            .into_iter()
            .map(|side| {
                let log = &logs[side.index()];
                let stats = self.node_ref(side).messenger.stats();
                format!(
                    "{side:?}: gönderilen {}, Delivered {}, alınan {}, bekleyen RECEIPT {}, kapı {}",
                    log.sent.len(),
                    log.delivered.len(),
                    log.received.len(),
                    stats.receipts_pending,
                    stats.gated_by_receipts
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// [K4G-05d rev3 (a)] Bir ucun uygulama adımı: `A_TEXTS`'e dek TEXT, ret gelene kadar; bir PTT
/// demeti (`A_BUNDLES`'a dek; reddedilen demet sonraki turda yeniden verilir). Geçici retler
/// (`Backpressure` — Transport ya da RECEIPT öncelik kapısı — ve outbox tavanı) beklenir; başka her
/// hata senaryo hatasıdır.
fn app_step(node: &mut Node, side: Side, stream: StreamId, log: &mut SideLog) {
    while log.sent.len() < A_TEXTS {
        match node.messenger.send_text(side.remote(), TEXT) {
            Ok(msg_id) => log.sent.push(msg_id),
            Err(
                MessengerError::Transport(TransportError::Backpressure)
                | MessengerError::OutboxFull,
            ) => break,
            Err(other) => panic!("{side:?}: beklenmeyen TEXT hatası {other:?}"),
        }
    }
    if log.bundles < A_BUNDLES {
        match node
            .ptt
            .send_bundle(&mut node.messenger, stream, &[0x5B; BUNDLE_BYTES])
        {
            Ok(()) => log.bundles += 1,
            Err(PttError::Transport(TransportError::Backpressure)) => {}
            Err(other) => panic!("{side:?}: beklenmeyen demet hatası {other:?}"),
        }
    }
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

/// **T3 oturum değişimi (rev3 M1: kayıt taşınır; not Ek B.2).**
///
/// Amaç: eşin oturumu değişince bekleyen RECEIPT DÜŞMEZ, yeni tutamaca taşınır; gönderenin yeni
/// oturumdaki yeniden gönderimi ve onun tekrar TEXT'i RECEIPT'i bir kez daha üretir → `h'`'de
/// RECEIPT(X) TAM İKİ kez kabul edilir, Alice tam bir `Delivered(X)` görür. Senaryo: P0 düzeneği
/// (Bob→Alice dolu); Alice X'i yollar, Bob'un RECEIPT(X) sunumu `Backpressure` alır. Supersession
/// `h` → `h'`, geçiş `poll_event`'te. Sıra: (1) Bob `Ready(h')`'yi okur — RECEIPT(X) önce hâlâ
/// aktif ve dolu `h`'ye, sonra `adopt`'ta dolu `h'`'ye sunulur; ikisi de `Backpressure` alır ve
/// kayıt kuyrukta kalır; (2) Alice kuyruğunu okur (yer açılır), `h'`'ye geçer ve X'i yeniden
/// yollar; (3) Bob'un turu: poll başı taşınan RECEIPT(X)'i kabul ettirir, ardından tekrar X'in
/// RECEIPT'i (bekleyen yok) bir kez daha kabul edilir.
///
/// Öldürdüğü mutant: `adopt`'ta kuyruğu boşaltan (rev2): `h'`'de yalnız tekrar TEXT'in RECEIPT'i
/// (1 kabul) ve (1)'den sonra `pending` 0.
#[test]
fn t3_session_change_carries_the_pending_receipt_to_the_new_session() {
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
    assert_eq!(
        rig.bob.messenger.pending_receipts(ALICE),
        1,
        "(1) RECEIPT(X) yeni oturuma taşındı, bekliyor"
    );
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
    // (3) Bob'un turu: taşınan RECEIPT(X), sonra tekrar X → RECEIPT(X) bir kez daha.
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
        2,
        "taşınan RECEIPT({msg_id}) + tekrar TEXT'in RECEIPT'i: h''de 2 kabul (1 = adopt kuyruğu \
         boşalttı). Denemeler: {attempts:?}"
    );
    let delivered_count =
        count_delivered(&alice_switch, msg_id) + count_delivered(&alice_after, msg_id);
    assert_eq!(
        delivered_count, 1,
        "Alice tam bir Delivered({msg_id}) görmeli"
    );
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);
}

/// T4'ün iki durumu: eşin oturumu, Bob'un RECEIPT(X)'i üretmesinden önce ya da o sırada biter.
#[derive(Debug, Clone, Copy)]
enum SessionEnd {
    /// Kapanış olayı X'ten önce okunur (olaylar birimlerden önce): X eski tutamaçla, Bob'un
    /// oturumu yokken işlenir — "oturum yok".
    BeforeText,
    /// Bob'un Transport'u kapanışı X'i teslim eden `recv`'de öğrenir: RECEIPT(X)'in ilk sunumu
    /// `UnknownSession` alır (Messenger olayı henüz okumadı).
    DuringRecv,
}

/// **T4 hata sınıfı (rev3, DUR-05d-5 (a): tut ve dur; not Ek B.2).**
///
/// Amaç: oturum yokken doğan RECEIPT kuyruğa girer; `Backpressure` dışı oturum sonu hatası (burada
/// `UnknownSession`) kaydı BIRAKMAZ, tutar. Kayıt eşin sıradaki tutamacına `adopt`'ta sunulur ve
/// tekrar TEXT'in RECEIPT'i bir kez daha kabul edilir. Senaryo (dosya yok): Alice X'i yollar;
/// oturum iki durumdan biriyle biter ([`SessionEnd`]); Bob bir tur daha koşar; sonra Alice
/// kapanışı okur, yeni oturum kurulur, Alice X'i yeniden yollar. Beklenen RECEIPT(X) denemeleri:
/// (yalnız `DuringRecv`) eski tutamaçta iki `UnknownSession` — biri TEXT'i teslim eden `poll`'da,
/// biri kapanış olayını okuyan sonraki `poll`'un başında (poll başına eş başına ≤ 1 ret) —, sonra
/// `h'`'de iki kabul (taşınan + tekrar TEXT'in). Oturumsuz turda hiçbir sunum yok. Alice tam bir
/// `Delivered(X)` görür.
///
/// Öldürdüğü mutantlar: yalnız `Backpressure`'ı geçici sayan sınıf (rev2; `DuringRecv`'de tek
/// `UnknownSession`, `h'`'de tek kabul); oturumsuz borcu bırakan `owe_receipt` (`BeforeText`'te
/// `h'`'de tek kabul).
#[test]
fn t4_a_session_end_holds_the_receipt_for_the_next_session() {
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
                    vec![
                        (old, Err(TransportError::UnknownSession)),
                        (old, Err(TransportError::UnknownSession)),
                    ],
                )
            }
        };
        assert_eq!(
            rig.bob.turn(),
            bob_events,
            "{case:?}: Bob X'i teslim eder ve kapanışı okur"
        );
        assert_eq!(
            rig.bob.messenger.pending_receipts(ALICE),
            1,
            "{case:?}: RECEIPT(X) tutuldu"
        );
        // Yeni oturumdan önce bir tur daha: oturumsuz eşe hiçbir sunum yapılmaz.
        rig.clock.advance(TURN_TICK);
        assert!(rig.bob.turn().is_empty(), "{case:?}: oturumsuz tur boştur");
        assert_eq!(
            rig.link.borrow().receipt_attempts(Side::Bob, msg_id),
            refused,
            "{case:?}: oturum kapandıktan sonra RECEIPT({msg_id}) sunulmaz"
        );

        // Alice kapanışı okur, yeni oturum kurulur, Alice X'i yeniden yollar.
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
        expected.push((new, Ok(())));
        assert_eq!(
            rig.link.borrow().receipt_attempts(Side::Bob, msg_id),
            expected,
            "{case:?}: RECEIPT({msg_id}) h''ye önce kuyruktan (adopt), sonra tekrar TEXT'ten"
        );
        assert_eq!(
            rig.drain_alice(),
            vec![delivered(BOB, msg_id)],
            "{case:?}: Alice tam bir Delivered görür"
        );
        assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0, "{case:?}");
        assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0, "{case:?}");
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

// ── K4G-05d rev3 testleri (not rev3 §5, Ek B.1) ─────────────────────────────────────────────

/// **(a1) Oturum değişmeden iki yönlü doygunluk — ZORUNLU (C14 §4).**
///
/// Senaryo ([`Duplex`]): iki uç da `kama-node` motorlarıdır; ikisi de karşıya dosya gönderir, PTT
/// akışı açar ve her turda demet verir; ikisi de karşıya `A_TEXTS` (= `MAX_OUTBOX_UNACKED`) TEXT
/// yollar. Yön kuyrukları dosya pump'ı, TEXT ve RECEIPT'lerle dolar; tur sırası her turda değişir.
/// Beklenen (değişmez (ii)–(iv)): `A_ROUND_CAP` içinde iki uç da bütün TEXT'lerini teslim eder,
/// her TEXT için tam bir `Delivered` görür; her RECEIPT AYNI oturumda tam bir kez kabul edilir;
/// `receipts_unsent` = 0, bekleyen RECEIPT 0; oturum olayı yok. Doygunluk kanıtı: iki uç da
/// `Backpressure` aldı, RECEIPT'ler en az bir kez `Backpressure` aldı ve öncelik kapısı en az bir
/// kez toplu üreticiyi tuttu. ACK yolu `Link`'te yoktur (M4'ün ACK yarısı INT-02'de ölçülür).
#[test]
fn a1_two_way_saturation_completes_on_the_same_session() {
    let mut duplex = Duplex::started();
    let mut logs = [SideLog::default(), SideLog::default()];
    let rounds = duplex.run(&mut logs, None);
    assert!(
        duplex.complete(&logs),
        "(a1) kilit ya da açlık: {A_ROUND_CAP} turda bitmedi — {}",
        duplex.diagnosis(&logs)
    );
    let link = duplex.link.borrow();
    let mut receipt_backpressure = 0;
    let mut gated = 0;
    for side in [Side::Alice, Side::Bob] {
        let log = &logs[side.index()];
        let messenger = &duplex.node_ref(side).messenger;
        assert_eq!(log.session_events, 0, "{side:?}: oturum değişmemeli");
        assert_eq!(messenger.session_of(side.remote()), Some(duplex.session));
        assert!(
            log.delivered.values().all(|&n| n == 1),
            "{side:?}: her TEXT için tam bir Delivered"
        );
        assert!(
            log.received.values().all(|&n| n == 1),
            "{side:?}: her TEXT bir kez teslim"
        );
        let stats = messenger.stats();
        assert_eq!(stats.receipts_unsent, 0, "{side:?}: bırakılan RECEIPT yok");
        assert_eq!(stats.receipts_pending, 0, "{side:?}");
        let accepted = link.accepted_receipts(side, duplex.session);
        assert_eq!(
            accepted.len(),
            A_TEXTS,
            "{side:?}: her RECEIPT aynı oturumda kabul edildi"
        );
        assert!(
            accepted.values().all(|&n| n == 1),
            "{side:?}: her RECEIPT tam bir kez"
        );
        assert!(
            link.backpressure_since(side, 0) > 0,
            "{side:?} yönü doymadı (düzenek)"
        );
        receipt_backpressure += link.receipt_backpressure(side);
        gated += stats.gated_by_receipts;
    }
    assert!(
        receipt_backpressure > 0,
        "düzenek: RECEIPT'ler hiç Backpressure almadı ({rounds} tur)"
    );
    assert!(
        gated > 0,
        "düzenek: öncelik kapısı hiç çalışmadı ({rounds} tur)"
    );
}

/// **(a2) İki yönlü doygunluk, ortada supersession (geçiş kolu; C14 §4).**
///
/// (a1) ile aynı düzenek; `A2_SUPERSEDE_ROUND` turunda iki yön doluyken `h` → `h'`. Beklenen:
/// `A_ROUND_CAP` içinde iki uç da bütün TEXT'lerini teslim eder; her TEXT tam bir kez teslim edilir
/// ve tam bir `Delivered` görür (yeniden gönderimler dedup'ta söner); `receipts_unsent` = 0,
/// bekleyen RECEIPT 0; uç başına iki oturum olayı (`Ready(h')`, `Closed(h)`). RECEIPT'lerin hangi
/// oturumda kabul edildiği iddia edilmez.
#[test]
fn a2_two_way_saturation_survives_a_supersession() {
    let mut duplex = Duplex::started();
    let mut logs = [SideLog::default(), SideLog::default()];
    duplex.run(&mut logs, Some(A2_SUPERSEDE_ROUND));
    assert!(
        duplex.complete(&logs),
        "(a2) kilit ya da açlık: {A_ROUND_CAP} turda bitmedi — {}",
        duplex.diagnosis(&logs)
    );
    for side in [Side::Alice, Side::Bob] {
        let log = &logs[side.index()];
        let messenger = &duplex.node_ref(side).messenger;
        assert_eq!(log.session_events, 2, "{side:?}: Ready(h') + Closed(h)");
        assert_ne!(
            messenger.session_of(side.remote()),
            Some(duplex.session),
            "{side:?}: yeni oturumda"
        );
        assert!(
            log.delivered.values().all(|&n| n == 1),
            "{side:?}: her TEXT için tam bir Delivered"
        );
        assert!(
            log.received.values().all(|&n| n == 1),
            "{side:?}: her TEXT bir kez teslim"
        );
        let stats = messenger.stats();
        assert_eq!(stats.receipts_unsent, 0, "{side:?}: bırakılan RECEIPT yok");
        assert_eq!(stats.receipts_pending, 0, "{side:?}");
    }
}

/// **(b) 300 TEXT tek yön, gönderen outbox tavanını aşmadan (rev3; not Ek B.1).**
///
/// Senaryo: P0 düzeneği (Bob Alice'e dosya gönderiyor, Bob→Alice dolu). Alice her turda yer
/// buldukça TEXT yollar (outbox tavanı ve `Backpressure` beklenir), Bob `kama-node` turunu koşar,
/// Alice okur. Beklenen: `B_ROUND_CAP` içinde `B_TEXTS` TEXT'in her biri tam bir kez teslim edilir
/// ve tam bir `Delivered` görür; her RECEIPT aynı oturumda tam bir kez kabul edilir; Bob'da
/// `receipts_unsent` = 0. Doygunluk kanıtı: RECEIPT'ler en az bir kez `Backpressure` aldı.
#[test]
fn b_three_hundred_texts_one_way_are_all_delivered_and_none_dropped() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let mut sent: Vec<MsgId> = Vec::new();
    let mut received_count: BTreeMap<MsgId, usize> = BTreeMap::new();
    let mut delivered_count: BTreeMap<MsgId, usize> = BTreeMap::new();
    let mut rounds = 0;
    while rounds < B_ROUND_CAP
        && !(delivered_count.len() == B_TEXTS && rig.bob.messenger.pending_receipts(ALICE) == 0)
    {
        while sent.len() < B_TEXTS {
            match rig.alice.send_text(BOB, TEXT) {
                Ok(msg_id) => sent.push(msg_id),
                Err(
                    MessengerError::Transport(TransportError::Backpressure)
                    | MessengerError::OutboxFull,
                ) => break,
                Err(other) => panic!("beklenmeyen TEXT hatası {other:?}"),
            }
        }
        rig.clock.advance(TURN_TICK);
        for event in rig.bob.turn() {
            match event {
                MessengerEvent::TextReceived { msg_id, .. } => {
                    *received_count.entry(msg_id).or_default() += 1;
                }
                other => panic!("Bob'da beklenmeyen olay {other:?}"),
            }
        }
        for event in rig.drain_alice() {
            if let MessengerEvent::Delivered { msg_id, .. } = event {
                *delivered_count.entry(msg_id).or_default() += 1;
            }
        }
        rounds += 1;
    }
    assert_eq!(
        delivered_count.len(),
        B_TEXTS,
        "(b) {B_ROUND_CAP} turda bitmedi: gönderilen {}, alınan {}, Delivered {}",
        sent.len(),
        received_count.len(),
        delivered_count.len()
    );
    assert!(
        received_count.values().all(|&n| n == 1),
        "her TEXT bir kez teslim"
    );
    assert!(
        delivered_count.values().all(|&n| n == 1),
        "her TEXT için tam bir Delivered"
    );
    let link = rig.link.borrow();
    let accepted = link.accepted_receipts(Side::Bob, session);
    assert_eq!(
        accepted.len(),
        B_TEXTS,
        "her RECEIPT aynı oturumda kabul edildi"
    );
    assert!(
        accepted.values().all(|&n| n == 1),
        "her RECEIPT tam bir kez"
    );
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
    assert!(
        link.receipt_backpressure(Side::Bob) > 0,
        "düzenek: RECEIPT'ler hiç Backpressure almadı ({rounds} tur)"
    );
}

/// **(b1) Pencere içinde `MAX_OUTBOX_UNACKED`'ten fazla yükümlülük düşmez (rev3 M1; not Ek B.1).**
///
/// Senaryo: P0 düzeneği; Bob→Alice dolu tutulur (Alice okumaz). Bob'a `B_TEXTS` farklı `msg_id`'li
/// (0, 1, …) ham TEXT `LINK_QUEUE_CAP`'lik partilerle gelir — gönderen outbox tavanı yerine
/// gönderen penceresine uyan eş. Beklenen: `B_TEXTS` RECEIPT bekler (hiçbiri düşmez, ≤
/// `MSG_DEDUP_WINDOW`); kapasite açılınca `B1_ROUNDS` turda her biri AYNI oturumda tam bir kez ve
/// `msg_id` sırasıyla kabul edilir. Öldürdüğü mutantlar: rev2'nin 256 tavanı ve en eskiyi düşürmesi
/// (`msg_id` 0..44 hiç kabul edilmez); `msg_id` sırası yerine bit konumu sırası.
#[test]
fn b1_more_obligations_than_the_outbox_cap_wait_without_drops() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let ids: Vec<MsgId> = (0..B_TEXTS)
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
    assert_eq!(
        rig.bob.messenger.pending_receipts(ALICE),
        B_TEXTS,
        "pencere içinde {B_TEXTS} yükümlülük bekler; hiçbiri düşmedi"
    );
    assert!(
        rig.link
            .borrow()
            .accepted_receipt_ids(Side::Bob, session)
            .is_empty(),
        "ön koşul: Bob→Alice dolu kaldı"
    );
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);

    let after = rig.run_rounds(B1_ROUNDS);
    rig.assert_same_session_and_capacity_opened(&after);
    assert_eq!(
        rig.link.borrow().accepted_receipt_ids(Side::Bob, session),
        ids,
        "her RECEIPT aynı oturumda tam bir kez, msg_id sırasıyla"
    );
    let stats = rig.bob.messenger.stats();
    assert_eq!(stats.receipts_pending, 0);
    assert_eq!(stats.receipts_unsent, 0);
    assert_eq!(stats.receipts_window_pruned, 0);
}

/// **(b2) Pencere ihlali: yalnız pencere altı budanır (rev3 M1, DUR-05d-6 (a); not Ek B.1).**
///
/// Senaryo: P0 düzeneği (Bob→Alice dolu). Bob'a önce `0 .. LINK_QUEUE_CAP` gelir (RECEIPT'leri
/// bekler), sonra pencereyi aşan `B2_FAR` (= `MSG_DEDUP_WINDOW` + 4). Beklenen: `0 .. B2_PRUNED`
/// pencere altına düşer ve sayılarak budanır (`receipts_window_pruned` = `B2_PRUNED`); pencere
/// içindeki kalan kayıtlar ve `B2_FAR` kapasite açılınca aynı oturumda tam bir kez, `msg_id`
/// sırasıyla kabul edilir. Öldürdüğü mutantlar: budamayan kuyruk (0..4 da kabul edilir); pencere
/// içini de budayan kuyruk; budamayı saymayan sayaç.
#[test]
fn b2_a_window_violation_prunes_only_below_the_window() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let session = rig.session;
    let cap = MsgId::try_from(LINK_QUEUE_CAP).expect("küçük sayı");
    for msg_id in 0..cap {
        rig.link
            .borrow_mut()
            .inject(Side::Bob, text_record(msg_id, TEXT));
    }
    assert_eq!(
        rig.bob.turn().len(),
        LINK_QUEUE_CAP,
        "ön koşul: ilk parti teslim"
    );
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), LINK_QUEUE_CAP);

    rig.link
        .borrow_mut()
        .inject(Side::Bob, text_record(B2_FAR, TEXT));
    assert_eq!(rig.bob.turn(), vec![received(ALICE, B2_FAR, TEXT)]);
    let pruned = usize::try_from(B2_PRUNED).expect("küçük sayı");
    assert_eq!(
        rig.bob.messenger.stats().receipts_window_pruned,
        u64::from(B2_PRUNED),
        "yalnız pencere altı sayılarak budandı"
    );
    assert_eq!(
        rig.bob.messenger.pending_receipts(ALICE),
        LINK_QUEUE_CAP - pruned + 1,
        "pencere içi kayıtlar + B2_FAR bekler"
    );

    let after = rig.run_rounds(B2_ROUNDS);
    rig.assert_same_session_and_capacity_opened(&after);
    let expected: Vec<MsgId> = (B2_PRUNED..cap).chain([B2_FAR]).collect();
    assert_eq!(
        rig.link.borrow().accepted_receipt_ids(Side::Bob, session),
        expected,
        "pencere içi kayıtlar tam bir kez, msg_id sırasıyla; budananlar hiç"
    );
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);
}

/// **(c) Öncelik kapısı ve muafiyetler (rev3 M2; not Ek B.1).**
///
/// Senaryo: Bob Alice'e PTT akışı açar (boş kuyruk, `PTT_OPEN` kabul) ve dosya gönderir; dosya
/// pump'ı Bob→Alice kuyruğunu doldurur. Alice TEXT X'i yollar; Bob'un RECEIPT(X)'i `Backpressure`
/// alır ve bekler. Beklenen: (1) aynı turda dosya pump'ı kapıda kalır — o turda `FILE_*` denemesi
/// yok; (2) kuyruk beklerken `MEDIA_BUNDLE`, `SIGNALING_BLOB` ve TEXT Transport'a hiç VERİLMEZ
/// (geçici `Backpressure`, `gated_by_receipts` her biri için artar); (3) muaf kayıtlar Transport'a
/// VERİLİR: `PTT_CLOSE` (çağrı), `FILE_ABORT` (kullanıcı iptali, sonraki turun pump'ı) ve
/// `FILE_RECEIPT` (Alice'ten gelen, sığmayan `FILE_BEGIN`'e `rejected`); (4) kapasite açılınca
/// Bob'un ilk kabul edilen kaydı RECEIPT(X)'tir ve Alice tam bir `Delivered(X)` görür.
///
/// Öldürdüğü mutantlar: kapıyı atlayan motor (ilgili tür denemelerde görünür); muafiyeti kaldıran
/// kapı (`PTT_CLOSE` / `FILE_ABORT` / `FILE_RECEIPT` denemesi yok); `FILE_ABORT`'u adımdan ayırt
/// etmeyen dosya sunumu.
#[test]
fn c_gate_holds_bulk_producers_and_passes_exempt_control_records() {
    let mut rig = Rig::connected();
    let stream = rig
        .bob
        .ptt
        .open(&mut rig.bob.messenger, ALICE, PttCodec::Opus, BUNDLE_MS)
        .expect("PTT_OPEN boş kuyrukta kabul edilir");
    let transfer = rig
        .bob
        .files
        .send_file(
            &rig.bob.messenger,
            ALICE,
            FILE_NAME,
            vec![FILE_FILL_BYTE; FILE_BYTES],
        )
        .expect("dosya kuyruğa alınır");
    assert!(rig.bob.turn().is_empty(), "dolum turu olay üretmez");
    assert_eq!(
        rig.link.borrow().endpoint_ref(Side::Alice).inbox.len(),
        LINK_QUEUE_CAP,
        "düzenek: Bob→Alice dolu"
    );

    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    let since_turn = rig.link.borrow().send_count(Side::Bob);
    assert_eq!(rig.bob.turn(), vec![received(ALICE, msg_id, TEXT)]);
    let in_turn = rig.link.borrow().kinds_since(Side::Bob, since_turn);
    assert_eq!(
        in_turn.first(),
        Some(&("RECEIPT", Err(TransportError::Backpressure))),
        "(1) RECEIPT(X) ilk sunumda Backpressure: {in_turn:?}"
    );
    assert!(
        !in_turn.iter().any(|(kind, _)| kind.starts_with("FILE_")),
        "(1) RECEIPT beklerken dosya pump'ı kapıda: {in_turn:?}"
    );
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 1);
    let gated_before = rig.bob.messenger.stats().gated_by_receipts;
    assert!(gated_before >= 1, "(1) pump kapıda sayıldı");

    // (2) Toplu üreticiler kapıda: hiçbiri Transport'a verilmez.
    let since_app = rig.link.borrow().send_count(Side::Bob);
    let payload = [0x5B; BUNDLE_BYTES];
    assert_eq!(
        rig.bob
            .ptt
            .send_bundle(&mut rig.bob.messenger, stream, &payload),
        Err(PttError::Transport(TransportError::Backpressure))
    );
    assert_eq!(
        rig.bob
            .signaling
            .send(&mut rig.bob.messenger, ALICE, &payload)
            .map(|_| ()),
        Err(SignalingError::Transport(TransportError::Backpressure))
    );
    assert_eq!(
        rig.bob.messenger.send_text(ALICE, TEXT),
        Err(MessengerError::Transport(TransportError::Backpressure))
    );
    assert!(
        rig.link
            .borrow()
            .kinds_since(Side::Bob, since_app)
            .is_empty(),
        "(2) kapıdaki kayıtlar Transport'a verilmedi"
    );
    assert_eq!(
        rig.bob.messenger.stats().gated_by_receipts,
        gated_before + 3,
        "(2) her toplu sunum kapıda sayıldı"
    );

    // (3) Muaf kayıtlar Transport'a verilir (yön dolu: `Backpressure` alırlar).
    assert_eq!(
        rig.bob.ptt.close(&mut rig.bob.messenger, stream),
        Err(PttError::Transport(TransportError::Backpressure)),
        "(3) PTT_CLOSE sunuldu; yön dolu"
    );
    rig.bob
        .files
        .cancel_send(transfer)
        .expect("aktarım sürüyor");
    let oversized = AppMessage::FileBegin {
        transfer_id: 1,
        file_size: MAX_REASSEMBLY_BYTES + 1,
        whole_file_hash: [0; 32],
        chunk_count: 1,
        name: FILE_NAME.to_owned(),
    };
    rig.link.borrow_mut().inject(
        Side::Bob,
        encode(&oversized, &limits()).expect("kodlanır").bytes,
    );
    rig.clock.advance(TURN_TICK);
    assert!(
        rig.bob.turn().is_empty(),
        "(3) FILE_BEGIN dosya motorunda tüketilir"
    );
    let control = rig.link.borrow().kinds_since(Side::Bob, since_app);
    for exempt in ["PTT_CLOSE", "FILE_ABORT", "FILE_RECEIPT"] {
        assert!(
            control.iter().any(|(kind, _)| *kind == exempt),
            "(3) muaf {exempt} Transport'a verilmeli: {control:?}"
        );
    }
    for bulk in [
        "TEXT",
        "MEDIA_BUNDLE",
        "SIGNALING_BLOB",
        "FILE_BEGIN",
        "FILE_CHUNK",
        "FILE_END",
    ] {
        assert!(
            !control.iter().any(|(kind, _)| *kind == bulk),
            "(3) toplu {bulk} RECEIPT beklerken verilmemeli: {control:?}"
        );
    }

    // (4) Kapasite açılır: Bob'un ilk kabul edilen kaydı RECEIPT(X).
    let since_open = rig.link.borrow().send_count(Side::Bob);
    let after = rig.open_capacity_and_run_turns();
    let opened = rig.link.borrow().kinds_since(Side::Bob, since_open);
    let first_accepted = opened
        .iter()
        .find(|(_, result)| result.is_ok())
        .map(|&(kind, _)| kind);
    assert_eq!(
        first_accepted,
        Some("RECEIPT"),
        "(4) kapasite açılınca önce RECEIPT(X): {opened:?}"
    );
    assert_eq!(count_delivered(&after.alice, msg_id), 1);
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
}

/// **(d-a) Kuyruktayken kapanış (rev3 M1; not Ek B.1).**
///
/// Bob'un RECEIPT(X)'i dolu Bob→Alice yönünde beklerken oturum iki uçta kapanır (`Closed(h)`);
/// Alice kuyruğunu okur, yeni oturum kurulur. Beklenen: kapanış kuyruğu boşaltmaz; Bob
/// `Ready(h')`'yi okuduğu turda RECEIPT(X)'i kuyruktan `h'`'ye kabul ettirir — Alice'in yeniden
/// gönderdiği tekrar X henüz işlenmemişken (`duplicate_texts` 0) — ve Alice tam bir
/// `Delivered(X)` görür. Öldürdüğü mutant: `retire`'da kuyruğu boşaltan (o turda `h'`'de kabul
/// yok).
#[test]
fn receipt_queue_survives_session_change_a() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let old = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    assert_eq!(rig.bob.turn(), vec![received(ALICE, msg_id, TEXT)]);
    assert_eq!(
        rig.bob.messenger.pending_receipts(ALICE),
        1,
        "ön koşul: RECEIPT(X) bekliyor"
    );

    let closed = MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::PeerClosed));
    for side in [Side::Alice, Side::Bob] {
        rig.link
            .borrow_mut()
            .close_at(side, CloseReason::PeerClosed);
    }
    assert_eq!(rig.bob.turn(), vec![closed.clone()]);
    assert_eq!(rig.bob.messenger.session_of(ALICE), None);
    assert_eq!(
        rig.bob.messenger.pending_receipts(ALICE),
        1,
        "kapanış kuyruğu boşaltmaz"
    );
    let alice_closed = rig.drain_alice();
    assert_eq!(alice_closed.first(), Some(&closed), "Alice kapanışı okur");
    assert!(rig.link.borrow().endpoint_ref(Side::Alice).inbox.is_empty());

    let new = rig.link.borrow_mut().establish();
    rig.clock.advance(TURN_TICK);
    assert_eq!(
        rig.bob.turn(),
        vec![MessengerEvent::Session(SessionEvent::Ready(new, ALICE))]
    );
    let attempts = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    assert_eq!(
        accepted_on(&attempts, new),
        1,
        "Ready(h')'nin turunda kuyruktan h''ye: {attempts:?}"
    );
    assert_eq!(
        rig.bob.messenger.stats().duplicate_texts,
        0,
        "tekrar X henüz işlenmedi"
    );
    assert_eq!(count_delivered(&rig.drain_alice(), msg_id), 1);
    rig.clock.advance(TURN_TICK);
    assert!(rig.bob.turn().is_empty(), "tekrar X olay üretmez");
    assert_eq!(count_delivered(&rig.drain_alice(), msg_id), 0);
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);
}

/// **(d-b) Geçiş `recv` anında, olaylar ters sırada (rev3 M1, DUR-05d-5 (a); not Ek B.1).**
///
/// Bob'un Transport'u supersession'ı X'i teslim eden `recv`'de öğrenir (`Link::supersede_on_next_recv`;
/// client-core `recv` → `drive` aynası): eski tutamaç hemen `SessionSuperseded` döner ve olaylar
/// `Closed(h)` önce, `Ready(h')` sonra gelir (`retire` → `adopt`). Beklenen RECEIPT(X) denemeleri:
/// `h`'de iki `SessionSuperseded` (TEXT'in `poll`'u ve sonraki `poll`'un başı), sonra `Ready(h')`'nin
/// `poll`'unda `h'`'de kabul — Alice'in yeniden gönderimi gelmeden. Alice tam bir `Delivered(X)`
/// görür. Öldürdüğü mutant: yalnız `Backpressure`'ı geçici sayan sınıf (DUR-05d-5 (b); `h'`'ye
/// kuyruktan sunum yok).
#[test]
fn receipt_queue_survives_session_change_b() {
    let mut rig = Rig::connected();
    let old = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    let new = rig.link.borrow_mut().supersede_on_next_recv(Side::Bob);
    assert_eq!(
        rig.bob.turn(),
        vec![
            received(ALICE, msg_id, TEXT),
            MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::Superseded)),
            MessengerEvent::Session(SessionEvent::Ready(new, ALICE)),
        ],
        "Bob X'i teslim eder, sonra Closed(h) ve Ready(h') okur"
    );
    assert_eq!(
        rig.link.borrow().receipt_attempts(Side::Bob, msg_id),
        vec![
            (old, Err(TransportError::SessionSuperseded)),
            (old, Err(TransportError::SessionSuperseded)),
            (new, Ok(())),
        ],
        "tut ve dur: h'de iki ret, sonra h''ye kuyruktan"
    );
    assert_eq!(rig.bob.messenger.stats().duplicate_texts, 0);
    let alice = rig.drain_alice();
    let switch_events = [
        MessengerEvent::Session(SessionEvent::Ready(new, BOB)),
        MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::Superseded)),
    ];
    assert_eq!(alice.get(..switch_events.len()), Some(&switch_events[..]));
    assert_eq!(count_delivered(&alice, msg_id), 1);
    rig.clock.advance(TURN_TICK);
    assert!(rig.bob.turn().is_empty(), "tekrar X olay üretmez");
    assert_eq!(count_delivered(&rig.drain_alice(), msg_id), 0);
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);
}

/// **(d-c) Arka arkaya iki oturum değişimi (rev3 M1; not Ek B.1).**
///
/// Bob'un RECEIPT(X)'i dolu Bob→Alice yönünde beklerken iki supersession `h` → `h'` → `h''`; Bob
/// dört olayı tek turda okur (her `adopt`'ta dolu yeni tutamaca sunum `Backpressure` alır). Alice
/// okur, iki yeni tutamaçta da X'i yeniden yollar. Beklenen: RECEIPT(X)'in ilk kabulü `h''`'dedir;
/// `h` ve `h'`'de kabul yok; Alice tam bir `Delivered(X)` görür — hiçbir TEXT "teslim edildi"siz
/// kalmaz.
#[test]
fn receipt_queue_survives_session_change_c() {
    let mut rig = Rig::with_full_bob_to_alice_queue();
    let first = rig.session;
    let msg_id = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    assert_eq!(rig.bob.turn(), vec![received(ALICE, msg_id, TEXT)]);

    let second = rig.link.borrow_mut().supersede();
    let third = rig.link.borrow_mut().supersede();
    assert_eq!(
        rig.bob.turn(),
        vec![
            MessengerEvent::Session(SessionEvent::Ready(second, ALICE)),
            MessengerEvent::Session(SessionEvent::Closed(first, CloseReason::Superseded)),
            MessengerEvent::Session(SessionEvent::Ready(third, ALICE)),
            MessengerEvent::Session(SessionEvent::Closed(second, CloseReason::Superseded)),
        ],
        "Bob iki değişimi tek turda okur"
    );
    assert_eq!(rig.bob.messenger.session_of(ALICE), Some(third));
    assert_eq!(
        rig.bob.messenger.pending_receipts(ALICE),
        1,
        "RECEIPT(X) iki değişimden sağ çıktı"
    );
    let alice_switch = rig.drain_alice();
    assert_eq!(rig.alice.session_of(BOB), Some(third));
    rig.clock.advance(TURN_TICK);
    assert!(rig.bob.turn().is_empty(), "tekrar X'ler olay üretmez");
    let alice_after = rig.drain_alice();

    let attempts = rig.link.borrow().receipt_attempts(Side::Bob, msg_id);
    assert_eq!(
        accepted_on(&attempts, first) + accepted_on(&attempts, second),
        0,
        "eski tutamaçlarda kabul yok: {attempts:?}"
    );
    assert_eq!(
        attempts
            .iter()
            .find(|(_, result)| result.is_ok())
            .map(|&(on, _)| on),
        Some(third),
        "ilk kabul h'''de: {attempts:?}"
    );
    assert_eq!(
        count_delivered(&alice_switch, msg_id) + count_delivered(&alice_after, msg_id),
        1,
        "Alice tam bir Delivered({msg_id}) görür"
    );
    assert_eq!(rig.bob.messenger.pending_receipts(ALICE), 0);
    assert_eq!(rig.bob.messenger.stats().receipts_unsent, 0);
}

/// **(e1) Kapanış borcu — bir kez kalıcı silme hatası (rev3 M1, DUR-05d-6 (a) zorunlu kol).**
///
/// Alice X'in RECEIPT'ini alır ama kalıcı kopyanın silmesi iki kez başarısız olur (`on_receipt`'in
/// silmesi ve sonraki `poll`'un yeniden denemesi). Beklenen: teslim gerçektir (`Delivered`),
/// `store_errors` 1 (yeniden deneme hatası yeniden sayılmaz); borç açıkken gönderen penceresi
/// X'ten ölçülür — `X + MSG_DEDUP_WINDOW` ayrılamaz (`UnackedSpanFull { oldest: X }`); sıradaki
/// `poll` silmeyi başarır, kalıntı kalkar ve aynı `msg_id` (yedek) gönderilir ve teslim edilir
/// ("takılma yok"). Öldürdüğü mutantlar: borcu tutmayan gönderen (ilk `send_text` geçer); yeniden
/// denemeyen gönderen (ikinci `send_text` de `UnackedSpanFull`).
#[test]
fn e1_closure_debt_holds_the_window_until_the_retry_succeeds() {
    let store = Arc::new(FlakyRemoveStore::new());
    let mut rig = Rig::connected_with(store.clone());
    let first = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    assert_eq!(rig.bob.turn(), vec![received(ALICE, first, TEXT)]);
    store.fail_remove(first, 2);
    assert_eq!(rig.drain_alice(), vec![delivered(BOB, first)]);
    assert_eq!(
        rig.alice.stats().store_errors,
        1,
        "yeniden deneme hatası yeniden sayılmaz"
    );
    assert_eq!(store.outbox_ids(BOB), vec![first], "kalıntı depoda");

    let far = first + MSG_DEDUP_WINDOW;
    store.jump_msg_id(u64::from(far));
    assert_eq!(
        rig.alice.send_text(BOB, TEXT),
        Err(MessengerError::UnackedSpanFull { oldest: first }),
        "borç pencereyi tutar"
    );
    assert!(rig.drain_alice().is_empty(), "yeniden deneme olay üretmez");
    assert!(
        store.outbox_ids(BOB).is_empty(),
        "yeniden deneme kalıntıyı sildi"
    );
    assert_eq!(
        rig.alice.send_text(BOB, TEXT),
        Ok(far),
        "borç kapandı: yedek msg_id gönderilir"
    );
    assert_eq!(rig.bob.turn(), vec![received(ALICE, far, TEXT)]);
    assert_eq!(rig.drain_alice(), vec![delivered(BOB, far)]);
    assert_eq!(rig.alice.stats().store_errors, 1);
}

/// **(e2) Kapanış borcu — kalıcı silme hatası + yeniden başlatma (rev3 M1, DUR-05d-6 (a) zorunlu
/// kol).**
///
/// Alice X'in kalıcı kopyasını silemez (depo yeniden başlatmaya dek bozuk). Pencerenin son değeri
/// (`X + MSG_DEDUP_WINDOW − 1`) gönderilir ve teslim edilir; bir fazlası borç yüzünden ayrılmaz.
/// Alice aynı depoyla yeniden başlar (depo iyileşti), yeni oturum kurulur. Beklenen: kalıntı X yeni
/// oturumda yeniden gönderilir ve Bob'un penceresinin İÇİNDEDİR — tekrar sayılır
/// (`duplicate_texts` 1), pencere altı değildir (`below_window_texts` 0, `receipts_window_pruned`
/// 0); RECEIPT'i kalıntıyı kapatır. Öldürdüğü mutant: borcu tutmayan gönderen (bir fazlası
/// gönderilir ve Bob'un penceresi X'in üstüne çıkar: kalıntı pencere altı olur).
#[test]
fn e2_closure_debt_survives_a_restart_and_the_residue_stays_inside_the_window() {
    let store = Arc::new(FlakyRemoveStore::new());
    let mut rig = Rig::connected_with(store.clone());
    let first = rig
        .alice
        .send_text(BOB, TEXT)
        .expect("Alice→Bob yönünde yer var");
    assert_eq!(rig.bob.turn(), vec![received(ALICE, first, TEXT)]);
    store.fail_remove(first, u32::MAX);
    assert_eq!(rig.drain_alice(), vec![delivered(BOB, first)]);

    let last = first + MSG_DEDUP_WINDOW - 1;
    store.jump_msg_id(u64::from(last));
    assert_eq!(
        rig.alice.send_text(BOB, TEXT),
        Ok(last),
        "pencerenin son değeri"
    );
    assert_eq!(rig.bob.turn(), vec![received(ALICE, last, TEXT)]);
    assert_eq!(rig.drain_alice(), vec![delivered(BOB, last)]);
    assert_eq!(
        rig.alice.send_text(BOB, TEXT),
        Err(MessengerError::UnackedSpanFull { oldest: first }),
        "bir fazlası borç yüzünden ayrılmaz"
    );
    assert_eq!(store.outbox_ids(BOB), vec![first], "kalıntı depoda");

    // Yeniden başlatma: aynı depoyla yeni motor; depo iyileşti; yeni oturum.
    store.heal();
    rig.alice = Messenger::with_store(End::new(&rig.link, Side::Alice), store.clone())
        .expect("limitler geçerli");
    let old = rig.session;
    let new = rig.link.borrow_mut().supersede();
    let switch_events = |peer: PeerRef| {
        vec![
            MessengerEvent::Session(SessionEvent::Ready(new, peer)),
            MessengerEvent::Session(SessionEvent::Closed(old, CloseReason::Superseded)),
        ]
    };
    assert_eq!(rig.drain_alice(), switch_events(BOB));
    assert_eq!(
        rig.alice.stats().resent_on_new_session,
        1,
        "kalıntı yeni oturumda yeniden gönderildi"
    );
    rig.clock.advance(TURN_TICK);
    assert_eq!(
        rig.bob.turn(),
        switch_events(ALICE),
        "tekrar X olay üretmez"
    );
    let bob = rig.bob.messenger.stats();
    assert_eq!(bob.duplicate_texts, 1, "kalıntı tekrar sayıldı");
    assert_eq!(
        bob.below_window_texts, 0,
        "kalıntı alıcının penceresi içinde (budanmadı)"
    );
    assert_eq!(bob.receipts_window_pruned, 0);
    assert_eq!(
        rig.drain_alice(),
        vec![delivered(BOB, first)],
        "kalıntı kapandı (yeniden başlatma sonrası ikinci Delivered beyanlıdır)"
    );
    assert!(store.outbox_ids(BOB).is_empty());
    let next = rig.alice.send_text(BOB, TEXT).expect("pencere açık");
    assert!(next > last, "sayaç kaldığı yerden sürer");
}
