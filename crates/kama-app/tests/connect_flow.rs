//! B2.5 — bağlantı akışı (`kama_app::messaging`): iptal, `ConnectProgress` UI durumu, erken metin.
//!
//! Beklenen davranışın kaynağı: S1 K6 (`early_unit` Transport'ta; glare/vazgeçme kuralı), §4
//! (`cancel` → `ConnectAbandoned(Cancelled)`, `early_unit` teslim edilmemiştir; `ConnectProgress`
//! hub yolunun durumudur), K7 (oturum değişiminde RECEIPT'siz kayıt eski `msg_id`'yle yeniden).
//!
//! Düzenek: ortak sahte (`tests/common/mock_transport.rs`) — YANSITICIDIR: gönderilen birim aynı
//! uca "eşten gelmiş" gibi döner; `complete_connect` bekleyen niyetin `early_unit`'ini teslim eder.
//! Motorun gönderen ve alan yarıları aynı `PeerRef` anahtarında ama ayrı durumlardır: erken TEXT
//! döner, alan yarı onu işleyip RECEIPT üretir, RECEIPT de döner ve gönderen yarı `Delivered` verir.
//!
//! K4G-07: erken metin kendi niyetinin biletine bağlıdır ve kalıcı outbox'a Transport'tan önce
//! yazılır; ilgili testler dosyanın sonunda.

mod common;

use std::sync::Arc;

use common::mock_transport::MockTransport;
use kama_app::messaging::{ConnectState, Messenger, MessengerError, MessengerEvent, MsgId};
use kama_app::store::{AppStore, MemoryAppStore, OutboxUnit};
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{
    AbandonReason, ConnectPhase, ConnectResult, SessionEvent, TransportCapabilities, TransportError,
};

/// Test parametresi (normatif değil): codec vektörlerinin üretildiği değerle aynı.
const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
/// `drain` döngüsünün tavanı.
const DRAIN_CAP: usize = 256;

const BOB: PeerRef = PeerRef::from_dest_hash([0xB0; 32]);

fn messenger() -> Messenger<MockTransport> {
    messenger_with(Arc::new(MemoryAppStore::new()))
}

/// Verilen depoyla motor (kalıcı outbox'ı gözlemek için).
fn messenger_with(store: Arc<dyn AppStore>) -> Messenger<MockTransport> {
    let transport = MockTransport::new(TransportCapabilities {
        max_plaintext_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
        preferred_plaintext_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
        max_early_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
        reliable: true,
    });
    Messenger::with_store(transport, store).expect("limitler geçerli")
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

/// Motoru, okunacak bir şey kalmayana dek sürer; olayları sırayla döndürür.
fn drain(m: &mut Messenger<MockTransport>) -> Vec<MessengerEvent> {
    let mut out = Vec::new();
    for _ in 0..DRAIN_CAP {
        match m.poll() {
            Some(ev) => out.push(ev),
            None => return out,
        }
    }
    panic!("{DRAIN_CAP} turda durulmadı");
}

/// Olayları `Ready` okunana dek sürer. Olaylar birimlerden ÖNCE okunduğu için hiçbir birim
/// (ör. erken metnin yankısı) işlenmez.
fn poll_through_ready(m: &mut Messenger<MockTransport>) -> Vec<MessengerEvent> {
    let mut out = Vec::new();
    for _ in 0..DRAIN_CAP {
        let ev = m.poll().expect("Ready okunmadan akış bitti");
        let ready = matches!(ev, MessengerEvent::Session(SessionEvent::Ready(..)));
        out.push(ev);
        if ready {
            return out;
        }
    }
    panic!("{DRAIN_CAP} turda Ready gelmedi");
}

fn texts(events: &[MessengerEvent]) -> Vec<(MsgId, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            MessengerEvent::TextReceived { msg_id, text, .. } => Some((*msg_id, text.clone())),
            _ => None,
        })
        .collect()
}

fn delivered(events: &[MessengerEvent]) -> Vec<MsgId> {
    events
        .iter()
        .filter_map(|e| match e {
            MessengerEvent::Delivered { msg_id, .. } => Some(*msg_id),
            _ => None,
        })
        .collect()
}

#[test]
fn connect_state_follows_progress_phases_until_ready() {
    let mut m = messenger();
    assert_eq!(m.connect_state(BOB), ConnectState::Idle);
    let ticket = match m.connect(BOB) {
        Ok(ConnectResult::Pending(t)) => t,
        other => panic!("Pending bekleniyordu: {other:?}"),
    };
    assert_eq!(
        m.connect_state(BOB),
        ConnectState::Connecting {
            ticket,
            phase: ConnectPhase::Measuring
        },
        "connect dönüşünde faz Measuring"
    );
    // Hub yolu düşer / gelir: faz olaylarla güncellenir (UI durumu; eşin canlılığı DEĞİL).
    m.transport_mut().set_link(false);
    let ev = drain(&mut m);
    assert!(
        ev.contains(&MessengerEvent::Session(SessionEvent::ConnectProgress(
            ticket,
            ConnectPhase::Suspended
        )))
    );
    assert_eq!(
        m.connect_state(BOB),
        ConnectState::Connecting {
            ticket,
            phase: ConnectPhase::Suspended
        }
    );
    m.transport_mut().set_link(true);
    drain(&mut m);
    assert_eq!(
        m.connect_state(BOB),
        ConnectState::Connecting {
            ticket,
            phase: ConnectPhase::Measuring
        }
    );
    let h = m.transport_mut().complete_connect(&BOB);
    drain(&mut m);
    assert_eq!(m.connect_state(BOB), ConnectState::Ready(h));
}

#[test]
fn cancel_connect_abandons_once_and_returns_to_idle() {
    let mut m = messenger();
    assert!(
        !m.cancel_connect(BOB),
        "bekleyen niyet yokken iptal etkisiz"
    );
    let ticket = match m.connect(BOB) {
        Ok(ConnectResult::Pending(t)) => t,
        other => panic!("Pending bekleniyordu: {other:?}"),
    };
    assert!(m.cancel_connect(BOB));
    let ev = drain(&mut m);
    assert!(
        ev.contains(&MessengerEvent::Session(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::Cancelled
        )))
    );
    assert_eq!(m.connect_state(BOB), ConnectState::Idle);
    assert!(
        !m.cancel_connect(BOB),
        "iptal edilmiş niyet ikinci kez iptal edilmez"
    );
}

/// K6: erken metin Transport'ta durur, motor onu KENDİSİ yollamaz; `Ready` sonrası TEXT bir kez
/// gelir, RECEIPT'i `Delivered` olur. Motor kaydı ikinci kez yollamadı (tekrar yok, yeniden
/// gönderim yok).
#[test]
fn early_text_rides_the_transport_and_is_receipted_once() {
    let mut m = messenger();
    let (result, msg_id) = m
        .connect_with_text(BOB, "ilk mesaj")
        .expect("connect_with_text");
    let ticket = match result {
        ConnectResult::Pending(t) => t,
        other => panic!("Pending bekleniyordu: {other:?}"),
    };
    assert_eq!(
        m.connect_state(BOB),
        ConnectState::Connecting {
            ticket,
            phase: ConnectPhase::Measuring
        }
    );
    // Oturum yokken motor hiçbir şey yollamaz: yalnız Measuring olayı.
    let before = drain(&mut m);
    assert!(
        texts(&before).is_empty(),
        "oturum yokken erken metin teslim edildi"
    );

    m.transport_mut().complete_connect(&BOB);
    let ev = drain(&mut m);
    assert_eq!(texts(&ev), [(msg_id, "ilk mesaj".to_owned())]);
    assert_eq!(
        delivered(&ev),
        [msg_id],
        "erken metnin RECEIPT'i tam bir Delivered"
    );
    let st = m.stats();
    assert_eq!(
        st.duplicate_texts, 0,
        "motor erken metni ikinci kez yolladı"
    );
    assert_eq!(
        st.resent_on_new_session, 0,
        "erken metin yeniden gönderildi"
    );
    assert_eq!(m.unacked_count(BOB), 0);
}

/// §4: vazgeçilen niyetin erken metni teslim edilmemiştir; eşin sıradaki oturumunda AYNI
/// `msg_id`'yle yollanır (motor kendiliğinden bağlanmaz — oturumu uygulama ister).
#[test]
fn abandoned_early_text_goes_out_on_the_next_session_with_the_same_msg_id() {
    let mut m = messenger();
    let (_, msg_id) = m
        .connect_with_text(BOB, "ilk mesaj")
        .expect("connect_with_text");
    m.transport_mut()
        .abandon_connect(&BOB, AbandonReason::PeerClosed);
    let ev = drain(&mut m);
    assert!(
        texts(&ev).is_empty(),
        "vazgeçilen niyetin metni teslim edildi"
    );
    assert_eq!(m.connect_state(BOB), ConnectState::Idle);
    assert_eq!(m.unacked_count(BOB), 1, "erken metin outbox'ta bekler");

    assert!(matches!(m.connect(BOB), Ok(ConnectResult::Pending(_))));
    m.transport_mut().complete_connect(&BOB);
    let ev = drain(&mut m);
    assert_eq!(texts(&ev), [(msg_id, "ilk mesaj".to_owned())]);
    assert_eq!(delivered(&ev), [msg_id]);
    assert_eq!(m.stats().resent_on_new_session, 1);
    assert_eq!(m.unacked_count(BOB), 0);
}

/// İptal de bir vazgeçmedir: erken metin outbox'a geçer ve sonraki oturumda gider.
#[test]
fn cancelled_early_text_is_not_lost() {
    let mut m = messenger();
    let (_, msg_id) = m
        .connect_with_text(BOB, "iptal")
        .expect("connect_with_text");
    assert!(m.cancel_connect(BOB));
    drain(&mut m);
    assert_eq!(m.unacked_count(BOB), 1);
    assert!(matches!(m.connect(BOB), Ok(ConnectResult::Pending(_))));
    m.transport_mut().complete_connect(&BOB);
    let ev = drain(&mut m);
    assert_eq!(texts(&ev), [(msg_id, "iptal".to_owned())]);
    assert_eq!(delivered(&ev), [msg_id]);
}

/// K6: aktif ya da bekleyen eşe erken metin `EarlyUnitConflict`'tir ve `msg_id` TÜKETMEZ.
#[test]
fn early_text_to_a_connected_or_pending_peer_is_a_conflict_and_consumes_nothing() {
    let mut m = messenger();
    assert!(matches!(m.connect(BOB), Ok(ConnectResult::Pending(_))));
    assert_eq!(
        m.connect_with_text(BOB, "çakışan").unwrap_err(),
        MessengerError::Transport(TransportError::EarlyUnitConflict),
        "bekleyen eşe erken metin"
    );
    m.transport_mut().complete_connect(&BOB);
    drain(&mut m);
    assert_eq!(
        m.connect_with_text(BOB, "çakışan").unwrap_err(),
        MessengerError::Transport(TransportError::EarlyUnitConflict),
        "aktif eşe erken metin"
    );
    // Reddedilenler `msg_id` yakmadı: ilk normal gönderim ilk kimliği alır.
    let first = m.send_text(BOB, "normal").expect("send_text");
    assert_eq!(first, kama_app::messaging::FIRST_MSG_ID);
}

// ── K4G-07: erken metin kendi niyetinin biletine bağlı; kalıcı outbox ────────────────────────

/// K4G-07 (inceleme bulgusu; kök #110): iptal edilen niyetin `ConnectAbandoned`'ı OKUNMADAN eşe
/// yeni bir erken metin gönderilir. Önceki erken metin EZİLMEZ: Transport yeni metni yeni bir
/// biletle kabul ettiği için önceki niyet bitmiş sayılır ve metni outbox'a "gönderilmemiş" olarak
/// geçer; oturum kurulunca yeniden gönderimle gider. İki metin de tam bir kez teslim edilir, ikisi
/// de `Delivered` olur, kalıcı outbox boşalır.
///
/// Düzeltmeden önce (ve `b7da69c4`'te) KIRMIZI: ikinci `connect_with_text` erken metni ezerdi;
/// "a" ne gönderilir ne teslim edilir. K4G-07'de kalıcı kopyası depoda kalırdı ve yeniden
/// başlatmada alıcı penceresinin altına düşüp yanlış "teslim edildi" üretebilirdi.
#[test]
fn cancelled_early_text_survives_a_new_early_text_before_the_abandon_is_read() {
    let store: Arc<dyn AppStore> = Arc::new(MemoryAppStore::new());
    let mut m = messenger_with(Arc::clone(&store));
    let (_, first) = m.connect_with_text(BOB, "a").expect("ilk niyet");
    assert!(m.cancel_connect(BOB));
    // `ConnectAbandoned` henüz okunmadı; Transport niyeti ve erken metnini zaten bıraktı.
    let (result, second) = m.connect_with_text(BOB, "b").expect("ikinci niyet");
    assert!(matches!(result, ConnectResult::Pending(_)));
    assert_eq!(second, first + 1);
    assert_eq!(m.unacked_count(BOB), 1, "önceki erken metin outbox'a geçti");
    assert_eq!(outbox_ids(&*store, BOB), vec![first, second]);

    m.transport_mut().complete_connect(&BOB);
    let ev = drain(&mut m);
    let mut got = texts(&ev);
    got.sort_unstable();
    assert_eq!(got, [(first, "a".to_owned()), (second, "b".to_owned())]);
    let mut done = delivered(&ev);
    done.sort_unstable();
    assert_eq!(done, [first, second]);
    let st = m.stats();
    assert_eq!(
        st.resent_on_new_session, 1,
        "yalnız önceki erken metin yeniden gönderildi"
    );
    assert_eq!(st.duplicate_texts, 0, "ikisi de tam bir kez");
    assert_eq!(m.unacked_count(BOB), 0);
    assert!(outbox_ids(&*store, BOB).is_empty());
}

/// K4G-07 (inceleme bulgusu; kök #110), `connect` varyantı: iptal edilen niyetin olayı okunmadan
/// eşe erken metinsiz yeni bir niyet açılır. Erken metin yeni niyetin `Ready`'sinde "gönderilmiş"
/// diye YANLIŞ etiketlenmez (Transport onu iptalle bıraktı): yeni bilet gelince outbox'a
/// "gönderilmemiş" olarak geçer ve yeni oturumda tam bir kez gider.
///
/// Düzeltmeden önce (ve `b7da69c4`'te) KIRMIZI: metin yeni oturumda "gönderilmiş" sayılır ve hiç
/// gönderilmezdi — teslim yok, RECEIPT yok, kayıt bir sonraki oturum değişimine dek takılı.
#[test]
fn cancelled_early_text_survives_a_plain_connect_before_the_abandon_is_read() {
    let store: Arc<dyn AppStore> = Arc::new(MemoryAppStore::new());
    let mut m = messenger_with(Arc::clone(&store));
    let (_, msg_id) = m.connect_with_text(BOB, "iptal edildi").expect("niyet");
    assert!(m.cancel_connect(BOB));
    assert!(matches!(m.connect(BOB), Ok(ConnectResult::Pending(_))));
    assert_eq!(m.unacked_count(BOB), 1, "erken metin outbox'a geçti");

    m.transport_mut().complete_connect(&BOB);
    let ev = drain(&mut m);
    assert_eq!(texts(&ev), [(msg_id, "iptal edildi".to_owned())]);
    assert_eq!(delivered(&ev), [msg_id]);
    assert_eq!(m.stats().resent_on_new_session, 1);
    assert_eq!(m.unacked_count(BOB), 0);
    assert!(outbox_ids(&*store, BOB).is_empty());
}

/// K4G-07: başarılı `connect_with_text` ayırdığı `msg_id`'yi TÜKETİR — sıradaki gönderim aynı
/// değeri yedekten yeniden almaz (alsaydı iki kayıt aynı `msg_id`'yi taşır, ikincisi birincinin
/// outbox kaydının yerine geçerdi). Depoda iki AYRI kayıt durur; ikisi de teslim edilir.
///
/// `b7da69c4`'te derlenmez (`with_store` yok); orada sayaç `connect_with_text`'te ilerletildiği
/// için davranışça yeşil olurdu — test bu kartın yedek `msg_id` (`spare_msg_id`) mekanizmasını
/// sabitler.
#[test]
fn successful_early_text_consumes_its_msg_id() {
    let store: Arc<dyn AppStore> = Arc::new(MemoryAppStore::new());
    let mut m = messenger_with(Arc::clone(&store));
    let (_, early) = m.connect_with_text(BOB, "erken").expect("niyet");
    let handle = m.transport_mut().complete_connect(&BOB);
    let ev = poll_through_ready(&mut m);
    assert!(ev.contains(&MessengerEvent::Session(SessionEvent::Ready(handle, BOB))));
    let next = m.send_text(BOB, "sonra").expect("oturum hazır");
    assert_eq!(next, early + 1);
    assert_eq!(
        outbox_ids(&*store, BOB),
        vec![early, next],
        "iki ayrı kalıcı kayıt"
    );

    let ev = drain(&mut m);
    assert_eq!(
        texts(&ev),
        [(early, "erken".to_owned()), (next, "sonra".to_owned())]
    );
    assert_eq!(delivered(&ev), [early, next]);
    assert!(outbox_ids(&*store, BOB).is_empty());
}
