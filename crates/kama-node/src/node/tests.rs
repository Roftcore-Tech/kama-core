//! Elle sürülen düğüm döngüsüyle birim testleri (`crate::test_support`): komut grubunu tur ARASINA
//! girmeden koşan, saati elle ilerleten ve döngünün iç durumunu (tutamaç kaydı, motor tabloları,
//! ölçü anlık görüntüsü) okuyan sınamalar. Kurulum `KamaNode::start` ile AYNI gövdedir (`build`):
//! gerçek provisioning, kripto, depo ve UDP; zarflar `KamaNode::start`'ın kullandığı süreç-içi
//! röleyle (her test kendi kimliklerini üretir).

use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use kama_app::codec::{AppMessage, EncodeError, PttCodec};
use kama_app::file_transfer::{
    FileFailure, FileSendError, SendFailure, TransferId, FILE_IDLE_TTL, FILE_RECEIPT_TTL,
};
use kama_app::messaging::{ConnectState, MessengerError, MessengerEvent};
use kama_app::ptt::{PttError, PttEvent, PTT_STREAM_IDLE_TTL};
use kama_contracts::handles::{CloseReason, PeerRef};
use kama_contracts::s1::{AbandonReason, ConnectPhase, KamaTransport, TransportError};

use super::{lock, Command, NodeLoop, Reply, COMMAND_QUEUE_CAP, REPLY_CAPACITY};
use crate::error::NodeError;
use crate::event::{
    CloseCause, FileFailReason, FileRef, FileStage, LinkState, NodeEvent, NodeMetrics,
};
use crate::handle::FileHandle;
use crate::test_support::{self as support, Events, ManualClock, TempDir};
use crate::{KamaNode, NodeListener};

/// Eşzamanlı connect (glare) — DR-CONNECT-1 #4 / K2 (`tests/glare.rs`).
mod glare;

const ALICE: &str = "alice";
const BOB: &str = "bob";
const CAROL: &str = "carol";

/// Test dosyası: bu kadar TAM parça + bir yarım parça (parça sınırına hizalanmaz; iptal edilen ve
/// kuyrukta bekleyen PARÇALAR olsun diye birden çok).
const FULL_CHUNKS: usize = 2;
/// Bir aktarımın parça dışı kayıtları: `FILE_BEGIN` + `FILE_END`.
const BEGIN_AND_END: usize = 2;
/// Dosya içeriğinin deseni: asal modül, parça boyuyla hizalanmaz (yer değiştiren parça görünür).
const PATTERN: usize = 251;

/// Test basışının demet süresi ve yükü — değerler media'nındır (AMP §3 0x06; düğüm yorumlamaz),
/// burada yalnız taşınacak kadar.
const BUNDLE_MS: u8 = 40;
const BUNDLE_LEN: usize = 64;
const BUNDLE_BYTE: u8 = 0x5A;
/// Taşma testinde bir adımda verilen en çok demet (geri basınca dek): adımı kısa tutar; değer
/// sonucu belirlemez, koşul her adımda yeniden denetlenir.
const PUSH_BURST: usize = 256;

/// Zamanlayıcı kontrolü: eşiğin bu kadar ALTINDA süre dolmamış olmalı. Saat elle ilerler;
/// değer yalnız eşikten ayırt edilebilir olmalı.
const CONTROL_MARGIN: Duration = Duration::from_secs(1);

/// Teslim sırası engeli olarak gönderilen metin.
const BARRIER_TEXT: &str = "engel";
/// Epoch geçişini tetikleyen ilk veri paketi.
const EPOCH_TEXT: &str = "epoch-1";
/// Zincir sırası testinde alındının ARDINDAN aynı oturumda gelen metin.
const AFTER_RECEIPT_TEXT: &str = "alindidan-sonra";
/// Zincir sırası testinde Alice'in teslim kuyruğundaki kayıtlar: `FILE_RECEIPT(ok)` + metin.
const RECEIPT_AND_TEXT: usize = 2;

/// Plan D14: tek TEXT ≤ 1002 B — düğümün türettiği tavana karşı BAĞIMSIZ kaynak (ürün profili:
/// birim 1013 B − AMP başlığı 5 B = `MAX_BODY_BYTES` 1008 B; − TEXT öneki 6 B).
const D14_MAX_TEXT_BYTES: usize = 1002;
/// Metin testlerinin karakteri (tek baytlık UTF-8: bayt boyu = karakter sayısı).
const TEXT_CHAR: &str = "a";

/// Oturumsuz `send_file`'ın kaynağı: düğüm oturum yokken dosyayı OKUMAZ (ad yeter).
const UNREAD_SOURCE: &str = "okunmaz.bin";
/// Kapanıştan önce kuyruğa konan yanıtsız komut sayısı — kuyruk tavanının (`COMMAND_QUEUE_CAP`)
/// altında: `Shutdown` hepsinin ardına sığar.
const QUEUED_BEFORE_SHUTDOWN: usize = 8;
const _: () = assert!(QUEUED_BEFORE_SHUTDOWN < COMMAND_QUEUE_CAP);

// ── Yardımcılar ──────────────────────────────────────────────────────────────────────────────

/// Yanıtlı bir komutu ürün döngüsünün komut grubundaki gibi koşar (tur YOK); yanıt komut
/// işlendiği anda gelir.
fn call<T>(node: &mut NodeLoop, make: impl FnOnce(Reply<T>) -> Command) -> Result<T, NodeError> {
    let (reply, answer) = mpsc::sync_channel(REPLY_CAPACITY);
    node.execute(make(reply));
    answer.try_recv().expect("komut işlendiği anda yanıtlanır")
}

fn peer_of(node: &NodeLoop, name: &str) -> PeerRef {
    node.peers
        .resolve(name)
        .expect("eş provisioning dosyasında")
}

/// `KamaNode::send_file`'ın düğüm tarafı: tutamaç çağıranda verilir, komut grupta işlenir.
fn send_file(node: &mut NodeLoop, to: &str, path: &Path) -> FileHandle {
    let file = FileHandle::from_raw(node.shared.handles.issue().expect("tutamaç"));
    let peer = peer_of(node, to);
    node.execute(Command::SendFile {
        file,
        peer,
        path: path.to_owned(),
    });
    file
}

/// Açık giden tutamacın (eş, tel `transfer_id`) kaydı.
fn wire_of(node: &NodeLoop, file: FileHandle) -> (PeerRef, TransferId) {
    *node
        .outgoing
        .get(file.raw(), &node.shared.handles)
        .expect("açık tutamaç")
}

fn connected(name: &str) -> impl FnMut(&NodeEvent) -> Option<()> + '_ {
    move |event| matches!(event, NodeEvent::Connected { peer_id } if peer_id == name).then_some(())
}

fn progress(file: FileHandle, stage: FileStage) -> impl FnMut(&NodeEvent) -> Option<()> {
    move |event| {
        matches!(
            event,
            NodeEvent::FileProgress { file: got, stage: seen, .. } if *got == file && *seen == stage
        )
        .then_some(())
    }
}

fn failed_outgoing(file: FileHandle) -> impl FnMut(&NodeEvent) -> Option<(String, FileFailReason)> {
    move |event| match event {
        NodeEvent::FileFailed {
            file: FileRef::Outgoing(got),
            peer_id,
            reason,
        } if *got == file => Some((peer_id.clone(), *reason)),
        _ => None,
    }
}

fn peer_state(metrics: &NodeMetrics, peer_id: &str) -> (LinkState, Option<u8>) {
    let state = metrics
        .connect_state
        .iter()
        .find(|state| state.peer_id == peer_id)
        .expect("her eş ölçüde");
    (state.state, state.epoch)
}

/// Elle sürülen `from`, iş parçacıklı `to`'ya bağlanır; yalnız `from` turlanır.
fn connect_from_manual(from: &mut NodeLoop, events: &mut Events, to: &str) {
    let peer = peer_of(from, to);
    call(from, |reply| Command::Connect { peer, reply }).expect("connect");
    support::until(&format!("Connected({to})"), || {
        from.turn();
        events.take(connected(to))
    });
}

/// İş parçacıklı `from`, elle sürülen `to` ile bağlanır; yalnız `to` turlanır.
fn connect_to_manual(
    from: &KamaNode,
    from_events: &mut Events,
    from_name: &str,
    to: &mut NodeLoop,
    to_events: &mut Events,
    to_name: &str,
) {
    from.connect(to_name).expect("connect");
    support::until(&format!("{to_name}: Connected({from_name})"), || {
        to.turn();
        to_events.take(connected(from_name))
    });
    from_events.wait(
        &format!("{from_name}: Connected({to_name})"),
        connected(to_name),
    );
}

/// İki elle sürülen düğüm bağlanır; ikisi de turlanır.
fn connect_manuals(
    a: &mut NodeLoop,
    a_events: &mut Events,
    a_name: &str,
    b: &mut NodeLoop,
    b_events: &mut Events,
    b_name: &str,
) {
    let peer = peer_of(a, b_name);
    call(a, |reply| Command::Connect { peer, reply }).expect("connect");
    let (mut a_up, mut b_up) = (false, false);
    support::until("iki uç Connected", || {
        a.turn();
        b.turn();
        a_up |= a_events.take(connected(b_name)).is_some();
        b_up |= b_events.take(connected(a_name)).is_some();
        (a_up && b_up).then_some(())
    });
}

/// `FULL_CHUNKS` tam + bir yarım parçalık dosya.
fn file_of_chunks(chunk_len: usize) -> Vec<u8> {
    let len = FULL_CHUNKS * chunk_len + chunk_len / 2;
    (0..len)
        .map(|i| u8::try_from(i % PATTERN).expect("desen bir bayta sığar"))
        .collect()
}

/// Bir aktarımın kayıt sayısı: `FILE_BEGIN` + parçalar + `FILE_END`.
fn file_records(len: usize, chunk_len: usize) -> usize {
    len.div_ceil(chunk_len) + BEGIN_AND_END
}

/// Düğümün adaptörünü (Transport) yalnız SÜRER — Messenger'ı okumadan — `records` kayıt teslim
/// kuyruğuna girene dek: kayıtlar uygulamaya ulaşmış ama İŞLENMEMİŞ durumdadır.
fn queue_records(node: &mut NodeLoop, records: usize) {
    support::until("kayıtlar teslim kuyruğunda", || {
        let transport = node.messenger.transport_mut();
        transport.drive();
        (transport.inbox_len_for_test() >= records).then_some(())
    });
    assert_eq!(
        node.messenger.transport().inbox_len_for_test(),
        records,
        "kuyrukta yalnız bu aktarımın kayıtları"
    );
}

/// Kuyruğun İLK kaydını — `FILE_BEGIN` olmalı — olay zincirinden geçirir (dosya motoru aktarımı
/// kabul eder); kalan kayıtlar kuyrukta bekler. Aktarımın kimliğini döner.
fn take_begin(node: &mut NodeLoop) -> TransferId {
    let event = node
        .ptt
        .poll(&mut node.messenger)
        .expect("FILE_BEGIN teslim kuyruğunda");
    let PttEvent::Messenger(MessengerEvent::Other {
        message: AppMessage::FileBegin { transfer_id, .. },
        ..
    }) = &event
    else {
        panic!("ilk kayıt FILE_BEGIN değil: {event:?}");
    };
    let transfer_id = *transfer_id;
    node.on_ptt_event(event);
    assert_eq!(node.files.incoming_count(), 1, "aktarım kabul edildi");
    transfer_id
}

// ── Sahip şartı (1): tutamaç kaydı motor tablosuyla AYNI komutta kapanır ─────────────────────

/// Tek komut grubunda (araya tur girmeden): H1 → bob, H1 iptal (`FILE_BEGIN` öncesi: motor
/// aktarımı BU çağrıda bırakır), H2 → carol (carol'ın sayacı da aynı değerden başlar: AYNI tel
/// `transfer_id`'si), H1 yeniden iptal. İkinci iptal `HandleClosed` alır ve carol'ın aktarımına
/// DOKUNMAZ. Uçtan uca (üç cihaz): carol dosyayı bayt-özdeş alır, alice `Delivered` görür, tele
/// yalnız H2'nin kayıtları çıkar ve bob hiçbir dosya olayı görmez.
#[test]
fn cancel_of_a_closed_file_handle_never_reaches_another_peers_transfer() {
    let _serial = support::serial();
    let root = TempDir::new("closed-file-handle");
    let phones = support::provision(root.path(), &[ALICE, BOB, CAROL]);
    let (_bob, mut bob_events) = support::threaded(&phones[BOB]);
    let (_carol, mut carol_events) = support::threaded(&phones[CAROL]);
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], support::system_clock());
    let node = &mut alice.node;
    connect_from_manual(node, &mut alice_events, BOB);
    connect_from_manual(node, &mut alice_events, CAROL);
    let to_bob = peer_of(node, BOB);
    let to_carol = peer_of(node, CAROL);
    let chunk_len = node.messenger.limits().max_file_chunk_bytes();
    let bytes = file_of_chunks(chunk_len);
    let source = root.path().join("ortak.bin");
    fs::write(&source, &bytes).expect("kaynak dosya");

    // ── Tek komut grubu: araya tur GİRMEZ ──
    let h1 = send_file(node, BOB, &source);
    let (owner, wire) = wire_of(node, h1);
    assert_eq!(owner, to_bob);
    call(node, |reply| Command::CancelFile { file: h1, reply }).expect("FILE_BEGIN öncesi iptal");
    assert_eq!(
        alice_events.take(failed_outgoing(h1)),
        Some((
            BOB.to_owned(),
            FileFailReason::Outgoing(SendFailure::Cancelled)
        )),
        "iptalin son olayı aynı komutta"
    );
    let h2 = send_file(node, CAROL, &source);
    assert_eq!(
        wire_of(node, h2),
        (to_carol, wire),
        "senaryo ön koşulu: iki eşin aktarımı AYNI tel kimliğini aldı"
    );
    let again = call(node, |reply| Command::CancelFile { file: h1, reply });
    assert!(matches!(again, Err(NodeError::HandleClosed)), "{again:?}");
    assert_eq!(node.files.outgoing_count(), 1, "carol'ın aktarımı kuyrukta");
    assert_eq!(wire_of(node, h2), (to_carol, wire), "H2 açık");
    assert_eq!(
        alice_events.take(failed_outgoing(h2)),
        None,
        "H2 iptal edilmedi"
    );

    // ── Uçtan uca: carol'ın aktarımı etkilenmedi ──
    support::until("alice: Delivered(H2)", || {
        node.turn();
        alice_events.take(progress(h2, FileStage::Delivered))
    });
    let (path, size) = carol_events.wait("carol: FileReceived(alice)", |event| match event {
        NodeEvent::FileReceived {
            peer_id,
            path,
            size,
            ..
        } if peer_id == ALICE => Some((path.clone(), *size)),
        _ => None,
    });
    assert_eq!(size, u64::try_from(bytes.len()).expect("boy"));
    assert_eq!(fs::read(&path).expect("alınan dosya"), bytes);
    assert_eq!(
        node.files.stats().records_accepted,
        u64::try_from(file_records(bytes.len(), chunk_len)).expect("kayıt sayısı"),
        "tele yalnız H2'nin kayıtları çıktı (H1'in FILE_BEGIN'i bile sunulmadı)"
    );
    // Aynı oturumda SONRA gönderilen metin teslim sırasının engelidir (Reliable, sıralı): bob onu
    // işlediyse ondan önce gelmiş her kaydı da işlemiştir.
    let barrier = call(node, |reply| Command::SendText {
        peer: to_bob,
        text: BARRIER_TEXT.to_owned(),
        reply,
    })
    .expect("metin");
    bob_events.wait("bob: TextReceived(engel)", |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id, .. }
                if peer_id == ALICE && *msg_id == barrier
        )
        .then_some(())
    });
    let file_event = bob_events.take(|event| {
        matches!(
            event,
            NodeEvent::FileReceived { .. } | NodeEvent::FileFailed { .. }
        )
        .then(|| event.clone())
    });
    assert!(
        file_event.is_none(),
        "bob dosya olayı görmemeli: {file_event:?}"
    );
}

// ── Madde 7(d): iptal + yeni `connect` aynı grupta ───────────────────────────────────────────

/// Bekleyen niyet iptal edilir ve AYNI komut grubunda eşe yeniden `connect` gelir: iptalin
/// `Closed { Abandoned(Cancelled) }` olayı yutulmaz (biten niyetin bileti kendi son olayına dek
/// kayıtta kalır), yeni niyet sürer ve kayıtta yalnız onun bileti kalır.
#[test]
fn connect_after_cancel_in_one_batch_keeps_the_abandoned_event() {
    let _serial = support::serial();
    let root = TempDir::new("cancel-connect-batch");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    // Bob hiç başlamaz: süreç-içi rölede kayıtlı değildir — INIT'in yolu yok ve saklanmaz; niyet
    // bekler.
    let (mut alice, mut events) = support::manual(&phones[ALICE], support::system_clock());
    let node = &mut alice.node;
    let bob = peer_of(node, BOB);

    call(node, |reply| Command::Connect { peer: bob, reply }).expect("connect");
    let cancelled = call(node, |reply| Command::CancelConnect { peer: bob, reply });
    assert!(
        matches!(cancelled, Ok(true)),
        "bekleyen niyet vardı: {cancelled:?}"
    );
    call(node, |reply| Command::Connect { peer: bob, reply }).expect("yeniden connect");
    node.turn();

    let abandoned = events.take(|event| {
        matches!(
            event,
            NodeEvent::Closed {
                peer_id,
                reason: CloseCause::Abandoned(AbandonReason::Cancelled),
            } if peer_id == BOB
        )
        .then_some(())
    });
    assert!(abandoned.is_some(), "iptalin Closed olayı yutulmadı");
    let other =
        events.take(|event| matches!(event, NodeEvent::Closed { .. }).then(|| event.clone()));
    assert!(other.is_none(), "tek Closed: {other:?}");
    assert!(
        matches!(
            node.messenger.connect_state(bob),
            ConnectState::Connecting { .. }
        ),
        "yeni niyet sürüyor"
    );
    assert_eq!(
        node.tickets.len(),
        1,
        "biten niyetin bileti çıktı; yalnız yenisi kalır"
    );
}

// ── Madde 2: teslim kanalı taşması ve epoch geçişi ölçüde ───────────────────────────────────

/// Bob'un döngüsü sürülmezken veri alıcısı birimleri teslim kanalına koyar; kanalı boşaltan
/// adaptör çağrılmaz. Alice kanal tavanından (`delivery_queue`) bir fazla demet sunar.
///
/// [K4G-05c · DUR-8 Ek B.4; eski ad `delivery_channel_overflow_reaches_node_metrics`] Fikstür aynı
/// (Bob sürülmez, Alice PTT demetleri basar, sonra Bob ürün döngüsüne girer). Eski iddia kanal
/// taşmasının ACK'li demeti düşürmesini (`delivery_channel_drops > 0`) beklenen davranış diye
/// sabitliyordu; R2 düzeltmesiyle (C9 §3, DUR-18; DUR-05c-10 (a)) dolu kanaldaki birim ACK'siz
/// reddedilir ve yeniden gönderilir: Bob döngüye girince `Ok` alan her demet tam bir kez gelir,
/// ölçüdeki taşma 0 kalır. Demet içerikleri aynı olduğundan sıra iddiası yoktur. Eski kodda bu
/// test KIRMIZIDIR (gelen demet < `Ok`; beklenen).
#[test]
fn delivery_channel_full_refuses_and_node_metrics_stay_zero() {
    let _serial = support::serial();
    let root = TempDir::new("delivery-overflow");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], support::system_clock());
    let (alice, mut alice_events) = support::threaded(&phones[ALICE]);
    connect_to_manual(
        &alice,
        &mut alice_events,
        ALICE,
        &mut bob.node,
        &mut bob_events,
        BOB,
    );
    assert_eq!(
        bob.node.snapshot().delivery_channel_drops,
        0,
        "taşma öncesi"
    );

    let stream = alice
        .ptt_open(BOB, PttCodec::Lyra, BUNDLE_MS)
        .expect("ptt_open");
    // Bob'un ürün profili teslim kanalı (`delivery_queue`) + 1: kanalı aşan en az bir demet.
    let target = client_core::runtime::RuntimeConfig::default().delivery_queue + 1;
    let mut pushed = 0usize;
    support::until(
        "alice: kanal tavanı + 1 demet sunuldu ya da geri basınç",
        || {
            for _ in 0..PUSH_BURST {
                if pushed >= target {
                    return Some(());
                }
                match alice.ptt_push(stream, vec![BUNDLE_BYTE; BUNDLE_LEN]) {
                    Ok(()) => pushed += 1,
                    // Geri basınç: Transport demeti almadı; sunum burada biter.
                    Err(NodeError::Ptt(PttError::Transport(TransportError::Backpressure))) => {
                        return Some(());
                    }
                    Err(other) => panic!("ptt_push: {other:?}"),
                }
            }
            (pushed >= target).then_some(())
        },
    );

    // Ürün döngüsü (`run`): `Ok` alan her demet tam bir kez gelir; ölçüde taşma yok.
    let bob = KamaNode::spawn(bob).expect("düğüm iş parçacığı");
    let mut bundles = 0usize;
    support::until("bob: Ok alan her demet teslim edildi", || {
        while bob_events
            .take(|event| matches!(event, NodeEvent::PttBundle { .. }).then_some(()))
            .is_some()
        {
            bundles += 1;
        }
        (bundles >= pushed).then_some(())
    });
    assert_eq!(bundles, pushed, "her demet tam bir kez teslim edilmeli");
    assert_eq!(
        bob.metrics().delivery_channel_drops,
        0,
        "dolu kanalda demet düşmemeli (ACK'siz ret)"
    );
}

/// Ölçünün epoch yolu: yanıtlayanın oturumu hazır olduğunda epoch-0'dadır (başlatanın epoch-1
/// paketi henüz gelmedi; noise-wire §4-B3 adım 1), başlatan RESPONSE'u işleyince epoch-1'dedir
/// (adım 2), başlatanın ilk epoch-1 paketiyle yanıtlayan da epoch-1'e geçer (adım 3–4). Eş başına
/// değer ve en düşük değer (`epoch`) aynı geçişi izler. Yenilemenin (yeni oturum, epoch yeniden 0)
/// uçtan uca sınanması bu düzenekte YOKTUR: tetik Crypto'nun saatine bağlıdır (oturum yaşı) ve
/// ürün kurulumu o saati sahip kararıyla gerçek saat olarak kurar (CEVAP PAKETİ 2 madde 3).
#[test]
fn epoch_telemetry_follows_the_responders_zero_to_one_transition() {
    let _serial = support::serial();
    let root = TempDir::new("epoch");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], support::system_clock());
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], support::system_clock());
    let to_bob = peer_of(&alice.node, BOB);
    call(&mut alice.node, |reply| Command::Connect {
        peer: to_bob,
        reply,
    })
    .expect("connect");

    // Yalnız Bob sürülür: INIT'i kabul eder, RESPONSE'u yollar; oturumu hazırdır.
    support::until("bob: Connected(alice)", || {
        bob.node.turn();
        bob_events.take(connected(ALICE))
    });
    let bob_view = bob.node.snapshot();
    assert_eq!(
        peer_state(&bob_view, ALICE),
        (LinkState::Connected, Some(0)),
        "{bob_view:?}"
    );
    assert_eq!(bob_view.epoch, Some(0), "{bob_view:?}");
    let alice_view = alice.node.snapshot();
    assert_eq!(
        peer_state(&alice_view, BOB),
        (LinkState::Connecting(ConnectPhase::Measuring), None),
        "{alice_view:?}"
    );
    assert_eq!(alice_view.epoch, None, "hazır oturum yok: {alice_view:?}");

    // Alice RESPONSE'u işler.
    support::until("alice: Connected(bob)", || {
        alice.node.turn();
        alice_events.take(connected(BOB))
    });
    let alice_view = alice.node.snapshot();
    assert_eq!(
        peer_state(&alice_view, BOB),
        (LinkState::Connected, Some(1)),
        "{alice_view:?}"
    );
    assert_eq!(alice_view.epoch, Some(1), "{alice_view:?}");

    // Alice'in ilk epoch-1 veri paketi.
    call(&mut alice.node, |reply| Command::SendText {
        peer: to_bob,
        text: EPOCH_TEXT.to_owned(),
        reply,
    })
    .expect("metin");
    support::until("bob: epoch-1", || {
        alice.node.turn();
        bob.node.turn();
        (peer_state(&bob.node.snapshot(), ALICE) == (LinkState::Connected, Some(1))).then_some(())
    });
    assert_eq!(bob.node.snapshot().epoch, Some(1));
}

// ── Madde 5: zamanlayıcılar düğüm turlarıyla, sahte saatle ──────────────────────────────────

/// (a) K4G-09 sözleşmesi: dosya zamanlayıcıları Messenger kuyruğu boşaltıldıktan SONRA denetlenir.
/// Bob'un uygulamasına ulaşmış ama işlenmemiş parçalar `FILE_IDLE_TTL` kadar beklese de tur onları
/// ÖNCE işler ve aktarım tamamlanır. (b) Ayırt edici kontrol: aynı bekleme kuyrukta parça YOKKEN
/// (kalan kayıtlar dosya motoru atlanarak tüketildi — gönderenin kaybolması benzetimi) trafiksiz
/// bir turda `IdleTimeout` verir; gönderen eşin `FILE_RECEIPT(timeout)`'unu alır.
#[test]
fn queued_chunks_outlive_file_idle_ttl_and_a_stalled_transfer_expires_on_a_turn() {
    let _serial = support::serial();
    let root = TempDir::new("file-idle");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let clock = ManualClock::default();
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], Arc::new(clock.clone()));
    let (alice, mut alice_events) = support::threaded(&phones[ALICE]);
    let node = &mut bob.node;
    connect_to_manual(&alice, &mut alice_events, ALICE, node, &mut bob_events, BOB);
    let chunk_len = node.messenger.limits().max_file_chunk_bytes();
    let bytes = file_of_chunks(chunk_len);
    let records = file_records(bytes.len(), chunk_len);
    let source = root.path().join("parcali.bin");
    fs::write(&source, &bytes).expect("kaynak dosya");

    // (a) Bütün kayıtlar teslim kuyruğunda; yalnız FILE_BEGIN işlendi.
    let first = alice.send_file(BOB, &source).expect("send_file");
    queue_records(node, records);
    let first_id = take_begin(node);
    clock.advance(FILE_IDLE_TTL);
    node.turn();
    let (received_id, size) = bob_events
        .take(|event| match event {
            NodeEvent::FileReceived {
                peer_id,
                transfer_id,
                size,
                ..
            } if peer_id == ALICE => Some((*transfer_id, *size)),
            _ => None,
        })
        .expect("kuyruktaki parçalar zamanlayıcıdan ÖNCE işlendi");
    assert_eq!(received_id, first_id);
    assert_eq!(size, u64::try_from(bytes.len()).expect("boy"));
    assert_eq!(node.files.stats().transfers_timed_out, 0);
    alice_events.wait("alice: Delivered", progress(first, FileStage::Delivered));

    // (b) Aynı bekleme, kuyrukta parça yokken.
    let second = alice.send_file(BOB, &source).expect("send_file");
    queue_records(node, records);
    let second_id = take_begin(node);
    // `FILE_BEGIN` işlendi; kalan kayıtlar "hiç gelmedi": dosya motoru atlanarak tüketilir.
    let remaining = records - 1;
    for _ in 0..remaining {
        let skipped = node.messenger.poll();
        assert!(
            matches!(skipped, Some(MessengerEvent::Other { .. })),
            "kalan kayıt: {skipped:?}"
        );
    }
    node.turn();
    assert_eq!(
        node.files.incoming_count(),
        1,
        "süre dolmadan aktarım sürer"
    );
    clock.advance(FILE_IDLE_TTL);
    node.turn();
    let reason = bob_events
        .take(|event| match event {
            NodeEvent::FileFailed {
                file: FileRef::Incoming(id),
                peer_id,
                reason,
            } if peer_id == ALICE && *id == second_id => Some(*reason),
            _ => None,
        })
        .expect("FILE_IDLE_TTL trafiksiz turda doldu");
    assert_eq!(reason, FileFailReason::Incoming(FileFailure::IdleTimeout));
    let (peer_id, reason) =
        alice_events.wait("alice: FILE_RECEIPT(timeout)", failed_outgoing(second));
    assert_eq!(peer_id, BOB);
    assert_eq!(
        reason,
        FileFailReason::Outgoing(SendFailure::ReceiverTimeout)
    );
}

/// PTT-R4: gelen akışa `PTT_STREAM_IDLE_TTL` boyunca demet gelmezse alıcının trafiksiz turu akışı
/// bitirir (`PttClosed`); eşiğin altında akış sürer.
#[test]
fn idle_incoming_ptt_stream_ends_on_a_turn_without_traffic() {
    let _serial = support::serial();
    let root = TempDir::new("ptt-r4");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let clock = ManualClock::default();
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], Arc::new(clock.clone()));
    let (alice, mut alice_events) = support::threaded(&phones[ALICE]);
    let node = &mut bob.node;
    connect_to_manual(&alice, &mut alice_events, ALICE, node, &mut bob_events, BOB);

    let stream = alice
        .ptt_open(BOB, PttCodec::Lyra, BUNDLE_MS)
        .expect("ptt_open");
    alice
        .ptt_push(stream, vec![BUNDLE_BYTE; BUNDLE_LEN])
        .expect("demet");
    let stream_id = support::until("bob: PttOpened(alice)", || {
        node.turn();
        bob_events.take(|event| match event {
            NodeEvent::PttOpened {
                peer_id, stream_id, ..
            } if peer_id == ALICE => Some(*stream_id),
            _ => None,
        })
    });
    support::until("bob: PttBundle(alice)", || {
        node.turn();
        bob_events.take(|event| {
            matches!(
                event,
                NodeEvent::PttBundle { peer_id, stream_id: got, .. }
                    if peer_id == ALICE && *got == stream_id
            )
            .then_some(())
        })
    });
    let ended = |events: &mut Events| {
        events.take(|event| {
            matches!(
                event,
                NodeEvent::PttClosed { peer_id, stream_id: got }
                    if peer_id == ALICE && *got == stream_id
            )
            .then_some(())
        })
    };

    clock.advance(PTT_STREAM_IDLE_TTL - CONTROL_MARGIN);
    node.turn();
    assert!(
        ended(&mut bob_events).is_none(),
        "eşiğin altında akış sürer"
    );
    clock.advance(CONTROL_MARGIN);
    node.turn();
    assert!(
        ended(&mut bob_events).is_some(),
        "PTT-R4: trafiksiz tur akışı bitirdi"
    );
    // PTT-R4 alıcıda yereldir: gönderenin tutamacı açık kalır.
    alice.ptt_close(stream).expect("ptt_close");
}

/// `FILE_RECEIPT_TTL`: son kaydı kabul edilmiş giden aktarıma alındı gelmezse gönderenin trafiksiz
/// turu aktarımı bitirir (`ReceiptTimeout`) ve düğüm tutamacı AYNI turda kapanır.
#[test]
fn receipt_timeout_closes_the_handle_on_a_turn_without_traffic() {
    let _serial = support::serial();
    let root = TempDir::new("receipt-ttl");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let clock = ManualClock::default();
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], Arc::new(clock.clone()));
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], support::system_clock());
    let node = &mut alice.node;
    connect_manuals(
        node,
        &mut alice_events,
        ALICE,
        &mut bob.node,
        &mut bob_events,
        BOB,
    );
    // Bob bundan sonra sürülmez: dosyayı uygulamasına almaz, `FILE_RECEIPT` göndermez.
    let chunk_len = node.messenger.limits().max_file_chunk_bytes();
    let source = root.path().join("alindisiz.bin");
    fs::write(&source, file_of_chunks(chunk_len)).expect("kaynak dosya");
    let file = send_file(node, BOB, &source);
    support::until("alice: Sent", || {
        node.turn();
        alice_events.take(progress(file, FileStage::Sent))
    });

    clock.advance(FILE_RECEIPT_TTL - CONTROL_MARGIN);
    node.turn();
    assert_eq!(
        alice_events.take(failed_outgoing(file)),
        None,
        "eşiğin altında bekler"
    );
    assert_eq!(wire_of(node, file).0, peer_of(node, BOB), "tutamaç açık");
    clock.advance(CONTROL_MARGIN);
    node.turn();
    assert_eq!(
        alice_events.take(failed_outgoing(file)),
        Some((
            BOB.to_owned(),
            FileFailReason::Outgoing(SendFailure::ReceiptTimeout)
        ))
    );
    let again = call(node, |reply| Command::CancelFile { file, reply });
    assert!(matches!(again, Err(NodeError::HandleClosed)), "{again:?}");
}

// ── Son tur (koordinatör 9a145b41 incelemesi) ────────────────────────────────────────────────

/// Bekletme kapısı: (girildi bildirimi, bırakma bekleyicisi).
type Gate = (SyncSender<()>, Receiver<()>);

/// Dinleyici: İLK `FileFailed` olayında düğüm iş parçacığını — komut grubunun içinde, o olayı
/// doğuran komutun yürütülmesi sırasında — test bırakana dek bekletir; bütün olayları kanala yazar.
struct GatedListener {
    events: Sender<NodeEvent>,
    gate: Mutex<Option<Gate>>,
}

impl NodeListener for GatedListener {
    fn on_event(&self, event: NodeEvent) {
        let gated = matches!(event, NodeEvent::FileFailed { .. });
        let _ = self.events.send(event);
        if !gated {
            return;
        }
        // Kilit beklemeden ÖNCE bırakılır.
        let gate = lock(&self.gate).take();
        if let Some((entered, release)) = gate {
            let _ = entered.send(());
            let _ = release.recv();
        }
    }
}

/// Oturumsuz `send_file`'ın düğümdeki sonucu.
fn not_queued_no_session() -> (String, FileFailReason) {
    (
        BOB.to_owned(),
        FileFailReason::NotQueued(FileSendError::NoSession),
    )
}

/// Madde 7(c), kuyrukta yer VARKEN: `Shutdown` kuyruğun sonuna girer, kapanış bayrağı KALKMAZ ve
/// önündeki yanıtsız komutların hepsi işlenir. (Eski anlamda bayrak her kapanışta kalkıyordu.)
#[test]
fn shutdown_with_room_in_the_queue_never_raises_the_stop_flag() {
    let _serial = support::serial();
    let root = TempDir::new("shutdown-room");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (node, mut events) = support::threaded(&phones[ALICE]);
    let shared = Arc::clone(&node.shared);
    // Oturum yok: her komut düğümde `NoSession` olayına dönüşür.
    let files: Vec<FileHandle> = (0..QUEUED_BEFORE_SHUTDOWN)
        .map(|_| {
            node.send_file(BOB, Path::new(UNREAD_SOURCE))
                .expect("kuyruğa")
        })
        .collect();
    node.shutdown().expect("kapanış");
    assert!(
        !shared.stopping.load(Ordering::Acquire),
        "kuyrukta yer varken kapanış bayrağı kalkmaz"
    );
    for file in files {
        assert_eq!(
            events.take(failed_outgoing(file)),
            Some(not_queued_no_session()),
            "kapanıştan önce kuyruğa giren komut işlendi"
        );
    }
}

/// Madde 7(c), kuyruk DOLUYKEN: `Shutdown` sığmaz, bayrak kalkar; döngü bayrağı komut grubu
/// sonunda okur ve içinde bulunduğu grubun sonunda biter — grubun dışında kalan komut olay
/// üretmeden düşer. Yarışsız: döngü grubun İLK komutunun dinleyici çağrısında bekletilirken
/// kuyruk doldurulur ve kapanış istenir; döngü ancak bayrak kalktıktan sonra bırakılır.
#[test]
fn full_queue_shutdown_ends_after_the_current_batch_and_drops_the_rest_without_events() {
    let _serial = support::serial();
    let root = TempDir::new("shutdown-full");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (events_tx, mut events) = support::event_channel();
    let (entered_tx, entered) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let listener = GatedListener {
        events: events_tx,
        gate: Mutex::new(Some((entered_tx, release_rx))),
    };
    let node = support::threaded_with(&phones[ALICE], Box::new(listener));
    let shared = Arc::clone(&node.shared);
    let source = Path::new(UNREAD_SOURCE);

    let first = node.send_file(BOB, source).expect("kuyruğa");
    entered
        .recv_timeout(support::WAIT)
        .expect("döngü grubun ilk komutunun olayında bekliyor");
    let queued: Vec<FileHandle> = (0..COMMAND_QUEUE_CAP)
        .map(|_| node.send_file(BOB, source).expect("kuyrukta yer var"))
        .collect();
    assert!(
        matches!(node.send_file(BOB, source), Err(NodeError::Busy)),
        "kuyruk dolu"
    );
    let closing = thread::spawn(move || node.shutdown());
    support::until("kapanış bayrağı kalktı", || {
        shared.stopping.load(Ordering::Acquire).then_some(())
    });
    release.send(()).expect("döngüyü bırak");
    support::until("kapanış bitti", || closing.is_finished().then_some(()));
    closing
        .join()
        .expect("kapanış iş parçacığı")
        .expect("kapanış");

    // Grup en çok `COMMAND_QUEUE_CAP` komuttur: ilk komut + kuyruktan `COMMAND_QUEUE_CAP − 1`.
    let (processed, dropped) = queued.split_at(COMMAND_QUEUE_CAP - 1);
    assert_eq!(
        events.take(failed_outgoing(first)),
        Some(not_queued_no_session())
    );
    for &file in processed {
        assert_eq!(
            events.take(failed_outgoing(file)),
            Some(not_queued_no_session()),
            "grup içinde işlendi"
        );
    }
    for &file in dropped {
        assert_eq!(
            events.take(failed_outgoing(file)),
            None,
            "grubun dışında kaldı: olay üretmeden düştü"
        );
    }
}

/// Madde 7(d) karşı yüzü: bilet niyetin SON olayında çıkar — başarılı bağlantıda `Ready` ile.
/// Aksi hâlde her (yeniden) bağlantıda kayıt büyürdü.
#[test]
fn tickets_are_released_on_ready_and_after_a_reconnection() {
    let _serial = support::serial();
    let root = TempDir::new("tickets");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (_bob, _bob_events) = support::threaded(&phones[BOB]);
    let (mut alice, mut events) = support::manual(&phones[ALICE], support::system_clock());
    let node = &mut alice.node;

    connect_from_manual(node, &mut events, BOB);
    assert!(
        node.tickets.is_empty(),
        "Ready bileti çıkardı: {:?}",
        node.tickets
    );
    // Oturum biter (Transport'un yerel `close`'u), sonra YENİ bir niyetle yeniden bağlanılır.
    let to_bob = peer_of(node, BOB);
    let session = node.messenger.session_of(to_bob).expect("hazır oturum");
    node.messenger
        .transport_mut()
        .close(session)
        .expect("yerel kapanış");
    support::until("Closed(bob)", || {
        node.turn();
        events.take(|event| {
            matches!(
                event,
                NodeEvent::Closed {
                    peer_id,
                    reason: CloseCause::Session(CloseReason::UserClosed),
                } if peer_id == BOB
            )
            .then_some(())
        })
    });
    connect_from_manual(node, &mut events, BOB);
    assert!(
        node.tickets.is_empty(),
        "yeniden bağlantıda da: {:?}",
        node.tickets
    );
}

/// Madde 3: düğümün girdi tavanları motorların KENDİ sınırıdır ve metin tavanı plan D14'ün
/// değeridir. Ön denetim atlanarak (komut doğrudan) limit + 1 motora verilir: metni codec, demeti
/// PTT motoru reddeder; limit boyunu ikisi de kabul eder.
#[test]
fn input_limits_are_the_engines_own_limits() {
    let _serial = support::serial();
    let root = TempDir::new("limits");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (_bob, _bob_events) = support::threaded(&phones[BOB]);
    let (mut alice, mut events) = support::manual(&phones[ALICE], support::system_clock());
    let limits = alice.limits;
    let node = &mut alice.node;
    connect_from_manual(node, &mut events, BOB);
    let to_bob = peer_of(node, BOB);
    assert_eq!(limits.max_text_bytes, D14_MAX_TEXT_BYTES, "plan D14");

    let over = call(node, |reply| Command::SendText {
        peer: to_bob,
        text: TEXT_CHAR.repeat(limits.max_text_bytes + 1),
        reply,
    });
    assert!(
        matches!(
            over,
            Err(NodeError::Messenger(MessengerError::Encode(
                EncodeError::BodyTooLarge { .. }
            )))
        ),
        "{over:?}"
    );
    call(node, |reply| Command::SendText {
        peer: to_bob,
        text: TEXT_CHAR.repeat(limits.max_text_bytes),
        reply,
    })
    .expect("limit boyundaki metni motor kabul eder");

    let stream = call(node, |reply| Command::PttOpen {
        peer: to_bob,
        codec: PttCodec::Lyra,
        bundle_ms: BUNDLE_MS,
        reply,
    })
    .expect("ptt_open");
    let over = call(node, |reply| Command::PttPush {
        stream,
        payload: vec![BUNDLE_BYTE; limits.max_bundle_bytes + 1],
        reply,
    });
    assert!(
        matches!(
            over,
            Err(NodeError::Ptt(PttError::PayloadTooLarge { payload_len, max_payload_len }))
                if payload_len == limits.max_bundle_bytes + 1
                    && max_payload_len == limits.max_bundle_bytes
        ),
        "{over:?}"
    );
    call(node, |reply| Command::PttPush {
        stream,
        payload: vec![BUNDLE_BYTE; limits.max_bundle_bytes],
        reply,
    })
    .expect("limit boyundaki demeti motor kabul eder");
}

/// Madde 4: üst düzey `epoch` hazır oturumların EN DÜŞÜĞÜDÜR. Alice bob'a başlatandır (epoch 1),
/// carol'a yanıtlayandır ve carol henüz veri yollamadı (epoch 0) → `Some(0)`; carol'ın ilk
/// epoch-1 paketinden sonra `Some(1)`.
#[test]
fn top_level_epoch_is_the_lowest_ready_session_epoch() {
    let _serial = support::serial();
    let root = TempDir::new("epoch-min");
    let phones = support::provision(root.path(), &[ALICE, BOB, CAROL]);
    let (_bob, _bob_events) = support::threaded(&phones[BOB]);
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], support::system_clock());
    let (mut carol, mut carol_events) = support::manual(&phones[CAROL], support::system_clock());

    connect_from_manual(&mut alice.node, &mut alice_events, BOB);
    // carol başlatır; carol SÜRÜLMEZ: RESPONSE'u işlemez, veri yollayamaz.
    let to_alice = peer_of(&carol.node, ALICE);
    call(&mut carol.node, |reply| Command::Connect {
        peer: to_alice,
        reply,
    })
    .expect("connect");
    support::until("alice: Connected(carol)", || {
        alice.node.turn();
        alice_events.take(connected(CAROL))
    });
    let view = alice.node.snapshot();
    assert_eq!(
        peer_state(&view, BOB),
        (LinkState::Connected, Some(1)),
        "{view:?}"
    );
    assert_eq!(
        peer_state(&view, CAROL),
        (LinkState::Connected, Some(0)),
        "{view:?}"
    );
    assert_eq!(view.epoch, Some(0), "en düşük değer: {view:?}");

    support::until("carol: Connected(alice)", || {
        carol.node.turn();
        carol_events.take(connected(ALICE))
    });
    call(&mut carol.node, |reply| Command::SendText {
        peer: to_alice,
        text: EPOCH_TEXT.to_owned(),
        reply,
    })
    .expect("metin");
    support::until("alice: carol oturumu epoch-1", || {
        alice.node.turn();
        carol.node.turn();
        (peer_state(&alice.node.snapshot(), CAROL) == (LinkState::Connected, Some(1))).then_some(())
    });
    assert_eq!(alice.node.snapshot().epoch, Some(1));
}

/// Madde 5: alındıyla (`FILE_RECEIPT(ok)`) biten giden aktarımın son olayı zincir sırasıyla gelir —
/// aynı turda, ondan SONRA işlenen kaydın olayından önce (`handle`'dan hemen sonra boşaltma).
#[test]
fn receipt_completion_is_reported_in_chain_order_within_the_turn() {
    let _serial = support::serial();
    let root = TempDir::new("chain-order");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (bob, mut bob_events) = support::threaded(&phones[BOB]);
    let (mut alice, mut events) = support::manual(&phones[ALICE], support::system_clock());
    let node = &mut alice.node;
    connect_from_manual(node, &mut events, BOB);
    let chunk_len = node.messenger.limits().max_file_chunk_bytes();
    let source = root.path().join("sirali.bin");
    fs::write(&source, file_of_chunks(chunk_len)).expect("kaynak dosya");
    let file = send_file(node, BOB, &source);
    support::until("alice: Sent", || {
        node.turn();
        events.take(progress(file, FileStage::Sent))
    });

    // Bob dosyayı aldı: `FILE_RECEIPT(ok)` olaydan ÖNCE Transport'una verildi; metin aynı oturumda
    // ondan sonra gider. Alice ikisini de işlemeden teslim kuyruğunda tutar.
    bob_events.wait("bob: FileReceived(alice)", |event| {
        matches!(event, NodeEvent::FileReceived { peer_id, .. } if peer_id == ALICE).then_some(())
    });
    let text_id = bob.send_text(ALICE, AFTER_RECEIPT_TEXT).expect("metin");
    queue_records(node, RECEIPT_AND_TEXT);
    let _ = events.drain_all();
    node.turn();
    let order = events.drain_all();
    let delivered = order
        .iter()
        .position(|event| {
            matches!(
                event,
                NodeEvent::FileProgress { file: got, stage: FileStage::Delivered, .. }
                    if *got == file
            )
        })
        .expect("Delivered aynı turda");
    let text = order
        .iter()
        .position(|event| {
            matches!(
                event,
                NodeEvent::TextReceived { peer_id, msg_id, .. }
                    if peer_id == BOB && *msg_id == text_id
            )
        })
        .expect("metin aynı turda");
    assert!(delivered < text, "zincir sırası: {order:?}");
}
