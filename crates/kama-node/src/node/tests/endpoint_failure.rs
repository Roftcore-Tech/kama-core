//! **K4G-05g — oturum ucu açılamazsa** (CEVAP PAKETİ 14 §5, 15 §3) ürün düğümünde uçtan uca.
//!
//! **Düzenek:** iki elle sürülen düğüm (`test_support::manual`: ürün kurulum gövdesi, gerçek
//! provisioning, kripto, depo, UDP ve süreç-içi röle). Uç açma hatası Transport'un test kancasıyla
//! üretilir (`ClientTransport::set_endpoint_open_failures_for_test`: `connect_session_counted`
//! hatasının belirlenimli karşılığı, eşin adres denetiminden SONRA; ürün derlemesinde yoktur).
//! Bekleme süreleri yalnız koşulun ÜST SINIRIDIR.
//!
//! * **(a) Provisional onayı kolu — DUR-05g-1 (a):** Alice yeniden başlar ve bağlanır (eş yeniden
//!   başlatma supersession'ı, identity §16) → Bob'da provisional oturum; Alice'in ilk epoch-1
//!   paketi onu Crypto'da onaylar (Bob'un eski oturumu drain'e düşer) ama Bob'un ucu açılamaz → Bob
//!   `NodeEvent::Closed { alice, Session(Superseded) }` görür (`note_down`: eşin güncel oturumu
//!   kalmadı), `Connected` görmez. Alice'in o oturumu KENDİ bakışında kuruludur (`Connected`
//!   verdi); karşı ucu kapalıdır — DUR-05g-2'nin "yarım açık" oturumu. Bob yeniden `connect`
//!   eder — Alice'teki yarım açık oturum dururken — ve BAŞARIR: `Connected`, iki yönde metin ve
//!   alındı; Alice'in yarım açık oturumda kaybolan metni yeni oturumda aynı `msg_id`'yle gider.
//! * **(b) Başlatan kolu — DUR-05g-2 (3):** Alice'in ucu RESPONSE'tan sonra açılamaz → Alice
//!   `Closed { bob, Abandoned(LocalPermanent) }`; Bob'un oturumu kendi bakışında kuruludur (yarım
//!   açık). Alice yeniden `connect` eder ve BAŞARIR; Bob'da `Closed` yok (eski oturumunun yerini
//!   yenisi aldı).

use kama_app::messaging::MsgId;
use kama_contracts::handles::{CloseReason, PeerRef};
use kama_contracts::s1::AbandonReason;

use super::{call, connect_manuals, connected, peer_of, Command};
use crate::event::{CloseCause, NodeEvent};
use crate::test_support::{self as support, Events, TempDir};
use crate::Built;

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// (a): Alice'in yarım açık oturumunda yolladığı metin (Bob'un o oturumu kapalı: teslim edilmez,
/// RECEIPT gelmez, Alice'in outbox'ında kalır).
const HALF_OPEN_TEXT: &str = "alice: yarım açık oturumda";
const ALICE_TEXT: &str = "alice: yeniden bağlandıktan sonra";
const BOB_TEXT: &str = "bob: yeniden bağlandıktan sonra";

fn closed(name: &str) -> impl FnMut(&NodeEvent) -> Option<CloseCause> + '_ {
    move |event| match event {
        NodeEvent::Closed { peer_id, reason } if peer_id == name => Some(*reason),
        _ => None,
    }
}

fn text_from<'a>(
    name: &'a str,
    msg_id: MsgId,
    text: &'a str,
) -> impl FnMut(&NodeEvent) -> Option<()> + 'a {
    move |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, text: body }
                if peer_id == name && *got == msg_id && body == text
        )
        .then_some(())
    }
}

fn receipt_from(name: &str, msg_id: MsgId) -> impl FnMut(&NodeEvent) -> Option<()> + '_ {
    move |event| {
        matches!(
            event,
            NodeEvent::Receipt { peer_id, msg_id: got } if peer_id == name && *got == msg_id
        )
        .then_some(())
    }
}

fn send_text(built: &mut Built, peer: PeerRef, text: &str) -> MsgId {
    call(&mut built.node, |reply| Command::SendText {
        peer,
        text: text.to_owned(),
        reply,
    })
    .expect("metin kuyruğa")
}

/// İki düğümü turlar: Alice→Bob ve Bob→Alice metinleri teslim edilir ve alındıları gelir.
fn exchange_texts(
    alice: &mut Built,
    alice_events: &mut Events,
    bob: &mut Built,
    bob_events: &mut Events,
    to_bob: (MsgId, &str),
    to_alice: (MsgId, &str),
) {
    let mut seen = [false; 4];
    support::until("iki yönde metin + alındı", || {
        alice.node.turn();
        bob.node.turn();
        seen[0] |= bob_events
            .take(text_from(ALICE, to_bob.0, to_bob.1))
            .is_some();
        seen[1] |= alice_events.take(receipt_from(BOB, to_bob.0)).is_some();
        seen[2] |= alice_events
            .take(text_from(BOB, to_alice.0, to_alice.1))
            .is_some();
        seen[3] |= bob_events.take(receipt_from(ALICE, to_alice.0)).is_some();
        seen.iter().all(|done| *done).then_some(())
    });
}

/// `events`'te eş için `Connected` / `Closed` kalmadı (tüketerek).
fn assert_no_link_change(events: &mut Events, why: &str) {
    let rest = events.drain_all();
    assert!(
        !rest.iter().any(|event| matches!(
            event,
            NodeEvent::Connected { .. } | NodeEvent::Closed { .. }
        )),
        "{why}: {rest:?}"
    );
}

/// **(a) Provisional onayı kolu** (bkz. modül belgesi).
#[test]
fn provisional_endpoint_failure_closes_superseded_and_reconnect_beside_the_half_open_session() {
    let _serial = support::serial();
    let root = TempDir::new("k4g05g-provisional");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], support::system_clock());
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], support::system_clock());
    let to_bob = peer_of(&alice.node, BOB);
    let to_alice = peer_of(&bob.node, ALICE);
    connect_manuals(
        &mut alice.node,
        &mut alice_events,
        ALICE,
        &mut bob.node,
        &mut bob_events,
        BOB,
    );
    let b0 = bob
        .node
        .messenger
        .session_of(to_alice)
        .expect("Bob'un oturumu");

    // Bob'un sıradaki uç açma denemesi = Alice'in yeni oturumunun onayı.
    bob.node
        .messenger
        .transport_mut()
        .set_endpoint_open_failures_for_test(1);

    // Alice yeniden başlar (aynı cihaz dosyası, aynı veri dizini: kalıcı `init_counter` artar) ve
    // bağlanır. Bob'da `b0` var → yeni INIT provisional.
    drop(alice);
    drop(alice_events);
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], support::system_clock());
    call(&mut alice.node, |reply| Command::Connect {
        peer: to_bob,
        reply,
    })
    .expect("connect");
    support::until("alice: Connected(bob)", || {
        alice.node.turn();
        bob.node.turn();
        alice_events.take(connected(BOB))
    });
    // Alice'in epoch-1 paketi Bob'da onayı getirir; Bob'un ucu açılamaz.
    let lost = send_text(&mut alice, to_bob, HALF_OPEN_TEXT);
    let cause = support::until("bob: Closed(alice)", || {
        alice.node.turn();
        bob.node.turn();
        bob_events.take(closed(ALICE))
    });
    assert_eq!(
        cause,
        CloseCause::Session(CloseReason::Superseded),
        "ardılın ucu açılamadı: eski oturum etkin ardıl olmadan Superseded ile biter"
    );
    assert_no_link_change(&mut bob_events, "Bob: Connected yok, tek Closed");
    assert_eq!(bob.node.messenger.session_of(to_alice), None);
    let transport = bob.node.messenger.transport();
    assert_eq!(transport.endpoint_open_failures(), 1);
    let b1 = transport
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_ne!(b1, b0);
    assert_eq!(
        transport.session_epoch(b1),
        None,
        "yeni Crypto oturumu kapalı"
    );
    assert_eq!(
        transport.session_epoch(b0),
        None,
        "eski Crypto oturumu kapalı"
    );

    // Karşı uç: Alice'in oturumu KENDİ bakışında kurulu — yarım açık (DUR-05g-2).
    let a1 = alice
        .node
        .messenger
        .session_of(to_bob)
        .expect("Alice'in yarım açık oturumu");

    // Bob yeniden bağlanır — Alice'teki yarım açık oturum dururken.
    call(&mut bob.node, |reply| Command::Connect {
        peer: to_alice,
        reply,
    })
    .expect("yeniden connect");
    assert_eq!(
        alice.node.messenger.session_of(to_bob),
        Some(a1),
        "yarım açık oturum duruyor"
    );
    support::until("bob: Connected(alice)", || {
        alice.node.turn();
        bob.node.turn();
        bob_events.take(connected(ALICE))
    });
    let from_bob = send_text(&mut bob, to_alice, BOB_TEXT);
    // Alice'in yarım açık oturumda kaybolan metni yeni oturumda aynı `msg_id`'yle gider.
    exchange_texts(
        &mut alice,
        &mut alice_events,
        &mut bob,
        &mut bob_events,
        (lost, HALF_OPEN_TEXT),
        (from_bob, BOB_TEXT),
    );
    let after = send_text(&mut alice, to_bob, ALICE_TEXT);
    let mut seen = [false; 2];
    support::until("Alice → Bob yeni oturumda", || {
        alice.node.turn();
        bob.node.turn();
        seen[0] |= bob_events
            .take(text_from(ALICE, after, ALICE_TEXT))
            .is_some();
        seen[1] |= alice_events.take(receipt_from(BOB, after)).is_some();
        (seen[0] && seen[1]).then_some(())
    });
    assert_ne!(
        alice.node.messenger.session_of(to_bob),
        Some(a1),
        "Alice'in yarım açık oturumunun yerini yenisi aldı"
    );
    assert_eq!(
        bob.node.messenger.transport().endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
    assert_no_link_change(&mut alice_events, "Alice: Closed yok (yerini yenisi aldı)");
}

/// **(b) Başlatan kolu** (bkz. modül belgesi).
#[test]
fn initiator_endpoint_failure_abandons_and_reconnect_beside_the_half_open_session() {
    let _serial = support::serial();
    let root = TempDir::new("k4g05g-initiator");
    let phones = support::provision(root.path(), &[ALICE, BOB]);
    let (mut alice, mut alice_events) = support::manual(&phones[ALICE], support::system_clock());
    let (mut bob, mut bob_events) = support::manual(&phones[BOB], support::system_clock());
    let to_bob = peer_of(&alice.node, BOB);
    let to_alice = peer_of(&bob.node, ALICE);

    alice
        .node
        .messenger
        .transport_mut()
        .set_endpoint_open_failures_for_test(1);
    call(&mut alice.node, |reply| Command::Connect {
        peer: to_bob,
        reply,
    })
    .expect("connect");
    let (mut bob_up, mut alice_closed) = (false, None);
    support::until("bob: Connected(alice) + alice: Closed(bob)", || {
        alice.node.turn();
        bob.node.turn();
        bob_up |= bob_events.take(connected(ALICE)).is_some();
        if alice_closed.is_none() {
            alice_closed = alice_events.take(closed(BOB));
        }
        (bob_up && alice_closed.is_some()).then_some(())
    });
    assert_eq!(
        alice_closed,
        Some(CloseCause::Abandoned(AbandonReason::LocalPermanent)),
        "uç açılamadı: niyet kapalı nedenle biter"
    );
    assert_no_link_change(&mut alice_events, "Alice: Connected yok");
    assert_eq!(alice.node.messenger.session_of(to_bob), None);
    let transport = alice.node.messenger.transport();
    assert_eq!(transport.endpoint_open_failures(), 1);
    let failed = transport
        .last_failed_activation_for_test()
        .expect("ucu açılamayan tutamaç kaydedilmeli");
    assert_eq!(
        transport.session_epoch(failed),
        None,
        "Crypto oturumu kapalı"
    );

    // Karşı uç: Bob'un oturumu KENDİ bakışında kurulu — yarım açık (DUR-05g-2).
    let b0 = bob
        .node
        .messenger
        .session_of(to_alice)
        .expect("Bob'un yarım açık oturumu");

    // Alice yeniden bağlanır — Bob'daki yarım açık oturum dururken.
    call(&mut alice.node, |reply| Command::Connect {
        peer: to_bob,
        reply,
    })
    .expect("yeniden connect");
    assert_eq!(
        bob.node.messenger.session_of(to_alice),
        Some(b0),
        "yarım açık oturum duruyor"
    );
    support::until("alice: Connected(bob)", || {
        alice.node.turn();
        bob.node.turn();
        alice_events.take(connected(BOB))
    });
    let from_alice = send_text(&mut alice, to_bob, ALICE_TEXT);
    let from_bob = send_text(&mut bob, to_alice, BOB_TEXT);
    exchange_texts(
        &mut alice,
        &mut alice_events,
        &mut bob,
        &mut bob_events,
        (from_alice, ALICE_TEXT),
        (from_bob, BOB_TEXT),
    );
    assert_ne!(
        bob.node.messenger.session_of(to_alice),
        Some(b0),
        "Bob'un yarım açık oturumunun yerini yenisi aldı"
    );
    assert_eq!(
        alice.node.messenger.transport().endpoint_open_failures(),
        1,
        "toparlanmada yeni hata yok"
    );
    assert_no_link_change(&mut bob_events, "Bob: Closed yok (yerini yenisi aldı)");
}
