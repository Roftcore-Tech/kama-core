//! K4G-14b birim testleri — sinyalleşme motorunun düğüm turundaki yeri, elle sürülen düğüm
//! döngüsüyle (`crate::test_support`). Kurulum `node::tests` ile aynıdır: `KamaNode::start` ile
//! AYNI gövde (`build`) — gerçek provisioning, kripto, depo ve UDP; zarflar süreç-içi röleden (her
//! test kendi kimliklerini üretir).
//!
//! * `Signaling::pump` Messenger kuyruğu BOŞALAN turda çağrılır (`files.pump` ile aynı koşul):
//!   yeniden sunum kaydının ömrü trafiksiz turda budanır.
//! * Zincir: `SIGNALING_BLOB` son halkada tüketilir ve `SignalingReceived` olur — PTT ve dosya
//!   motoruna geçmez —; diğer olaylar aynen ve sırayla geçer.
//!
//! `Debug`'ın blob baytı basmadığı `crate::event` testlerindedir; ürün yolu (iki `KamaNode::start`)
//! `tests/signaling_loopback.rs`'te. Yardımcılar `node::tests`'tekilerin kopyasıdır: o modülün özel
//! öğeleri kardeş modülden görünmez.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use kama_app::signaling::SIGNALING_RESEND_TTL;
use kama_contracts::handles::PeerRef;

use super::{Command, NodeLoop, Reply, REPLY_CAPACITY};
use crate::error::NodeError;
use crate::event::NodeEvent;
use crate::test_support::{self as support, Events, ManualClock, TempDir};

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// Zamanlayıcı kontrolü: eşiğin bu kadar ALTINDA süre dolmamış olmalı. Saat elle ilerler; değer
/// yalnız eşikten ayırt edilebilir olmalı.
const CONTROL_MARGIN: Duration = Duration::from_secs(1);

/// Test blob'ları: opak, birbirinden ayırt edilebilir baytlar (içerik medya motorunundur).
const FIRST_BLOB: &[u8] = &[0x01, 0x02, 0x03, 0x04];
const SECOND_BLOB: &[u8] = &[0xF0, 0x0D, 0xBE];
/// Zincir testinde iki blob'un ARASINDA, aynı oturumda gelen metin.
const BETWEEN_BLOBS_TEXT: &str = "bloblar-arasinda";
/// Zincir testinde Alice'in teslim kuyruğundaki kayıtlar: blob + metin + blob.
const BLOB_TEXT_BLOB: usize = 3;

// ── Yardımcılar (`node::tests` kopyası) ─────────────────────────────────────────────────────

/// Yanıtlı bir komutu ürün döngüsünün komut grubundaki gibi koşar (tur YOK).
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

fn connected(name: &str) -> impl FnMut(&NodeEvent) -> Option<()> + '_ {
    move |event| matches!(event, NodeEvent::Connected { peer_id } if peer_id == name).then_some(())
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
        "kuyrukta yalnız bu testin kayıtları"
    );
}

// ── Pompa: kuyruğu boşalan turda ─────────────────────────────────────────────────────────────

/// `Signaling::pump` Messenger kuyruğu BOŞALAN turda çağrılır. Gözlem: kabul edilen blob'un
/// yeniden sunum kaydı ilk kabulden `SIGNALING_RESEND_TTL` sonra ancak bir DENETİMDE budanır
/// (`records_expired`) ve bu sınamada `pump`'tan başka denetim yoktur — yeni gönderim yok, oturum
/// değişimi (`Ready`) yok: trafiksiz turda budayan `pump`'tır. Eşiğin altında kayıt kalır.
#[test]
fn signaling_pump_prunes_an_expired_record_on_a_drained_turn() {
    let _serial = support::serial();
    let root = TempDir::new("signaling-pump");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let clock = ManualClock::default();
    let (_bob, _bob_events) = support::threaded(&phones[BOB]);
    let (mut alice, mut events) = support::manual(&phones[ALICE], Arc::new(clock.clone()));
    let node = &mut alice.node;
    connect_from_manual(node, &mut events, BOB);
    let to_bob = peer_of(node, BOB);
    call(node, |reply| Command::SendSignaling {
        peer: to_bob,
        bytes: FIRST_BLOB.to_vec(),
        reply,
    })
    .expect("blob kuyruğa");
    assert_eq!(
        node.signaling.pending_count(to_bob),
        1,
        "kabul edilen blob yeniden sunum kaydında"
    );

    clock.advance(SIGNALING_RESEND_TTL - CONTROL_MARGIN);
    assert!(node.turn(), "trafiksiz tur: Messenger kuyruğu boşaldı");
    assert_eq!(
        node.signaling.stats().records_expired,
        0,
        "eşiğin altında kayıt kalır"
    );
    assert_eq!(node.signaling.pending_count(to_bob), 1);

    clock.advance(CONTROL_MARGIN);
    assert!(node.turn(), "trafiksiz tur: Messenger kuyruğu boşaldı");
    assert_eq!(
        node.signaling.stats().records_expired,
        1,
        "boşalan turun pump'ı ömrü dolan kaydı budadı"
    );
    assert_eq!(node.signaling.pending_count(to_bob), 0);
}

// ── Zincir: son halka ────────────────────────────────────────────────────────────────────────

/// Zincir `Ptt::poll` → `FileTransfer::handle` → `Signaling::handle`. Aynı oturumdan art arda
/// gelen blob · metin · blob AYNI turda, gönderim sırasıyla, her biri TAM bir düğüm olayı olur:
/// blob'lar `SignalingReceived` (bayt-özdeş), metin `TextReceived` olarak aynen geçer. Blob PTT ya
/// da dosya motoruna sızmaz — iki motorun sayaçları değişmez, gelen tabloları boş, PTT / dosya
/// olayı yok — ve düğümün `Other` koluna düşüp yutulmaz.
#[test]
fn signaling_blob_is_consumed_last_in_the_chain_and_other_events_pass_unchanged() {
    let _serial = support::serial();
    let root = TempDir::new("signaling-chain");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (bob, _bob_events) = support::threaded(&phones[BOB]);
    let (mut alice, mut events) = support::manual(&phones[ALICE], support::system_clock());
    let node = &mut alice.node;
    connect_from_manual(node, &mut events, BOB);
    let to_bob = peer_of(node, BOB);
    let ptt_before = node.ptt.stats();
    let files_before = node.files.stats();

    // Bob'un üç kaydı aynı oturumda (Reliable, sıralı); Alice onları işlemeden kuyrukta tutar.
    bob.send_signaling(ALICE, FIRST_BLOB.to_vec())
        .expect("blob");
    let text_id = bob.send_text(ALICE, BETWEEN_BLOBS_TEXT).expect("metin");
    bob.send_signaling(ALICE, SECOND_BLOB.to_vec())
        .expect("blob");
    queue_records(node, BLOB_TEXT_BLOB);
    let _ = events.drain_all();
    assert!(node.turn(), "üç kayıt tek turda işlendi");

    assert_eq!(
        events.drain_all(),
        [
            NodeEvent::SignalingReceived {
                peer_id: BOB.to_owned(),
                bytes: FIRST_BLOB.to_vec(),
            },
            NodeEvent::TextReceived {
                peer_id: BOB.to_owned(),
                msg_id: text_id,
                text: BETWEEN_BLOBS_TEXT.to_owned(),
            },
            NodeEvent::SignalingReceived {
                peer_id: BOB.to_owned(),
                bytes: SECOND_BLOB.to_vec(),
            },
        ],
        "zincir sırası; turun bütün olayları"
    );
    assert_eq!(node.signaling.stats().blobs_received, 2);
    assert_eq!(node.ptt.stats(), ptt_before, "PTT motoru blob'u saymadı");
    assert!(node.ptt.incoming_streams(to_bob).is_empty());
    assert_eq!(
        node.files.stats(),
        files_before,
        "dosya motoru blob'u saymadı"
    );
    assert_eq!(node.files.incoming_count(), 0);
}
