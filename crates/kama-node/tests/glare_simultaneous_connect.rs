//! Eşzamanlı connect — ÜRÜN yolu (`KamaNode::start`, süreç-içi röle): iki düğüm iki iş
//! parçacığından bir engelle AYNI ANDA `connect` eder.
//!
//! **Kapsam sınırı:** zamanlama belirsizdir — iki INIT'in gerçekten çaprazlanıp çaprazlanmadığı
//! (glare) bu testte DENETLENMEZ ve iddia edilmez. Test yalnız SONUCU sınar: her düğümde eş için
//! tam bir `Connected`, `Closed` / `ConnectAbandoned` yok, iki yönde metin tam bir kez + alındısı.
//! Glare'in kendisi (iki taraf bekleyen INIT taşırken çözülme, roller, tek RESPONSE'un yapısal
//! karşılığı, erken metinler) belirlenimli birim testindedir: `node::tests::glare`. Erken metin
//! ürün API'sinde YOKTUR (`KamaNode` `connect_with_text` sunmaz; `send_text` oturum ister) — bu
//! test metinleri bağlandıktan sonra verir.

mod common;

use std::sync::Barrier;
use std::thread;

use kama_app::messaging::MsgId;
use kama_node::{KamaNode, LinkState, NodeEvent};

use common::{provision, Device, Events, TempDir};

const ALICE: &str = "alice";
const BOB: &str = "bob";
/// Aynı anda `connect` eden iş parçacığı sayısı (iki düğüm).
const CONNECTORS: usize = 2;

/// Ürün yolu: `KamaNode::start`.
fn start(device: &Device) -> (KamaNode, Events) {
    let (listener, events) = common::listener();
    let node = KamaNode::start(&device.config, &device.data, listener).expect("düğüm kurulmalı");
    (node, events)
}

fn wait_connected(events: &mut Events, peer: &str) {
    events.wait(&format!("Connected({peer})"), |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == peer).then_some(())
    });
}

fn text_of(name: &str) -> String {
    format!("{name}: eşzamanlı bağlantıdan sonra")
}

fn wait_text(events: &mut Events, from: &str, msg_id: MsgId) {
    let text = text_of(from);
    events.wait(&format!("TextReceived({from}, {msg_id})"), |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, text: body }
                if peer_id == from && *got == msg_id && *body == text
        )
        .then_some(())
    });
}

fn wait_receipt(events: &mut Events, from: &str, msg_id: MsgId) {
    events.wait(&format!("Receipt({from}, {msg_id})"), |event| {
        matches!(
            event,
            NodeEvent::Receipt { peer_id, msg_id: got } if peer_id == from && *got == msg_id
        )
        .then_some(())
    });
}

/// İki düğüm aynı anda `connect`: sonuçta her tarafta eş için tek `Connected`, kapanış ya da terk
/// yok, iki yönde metin tam bir kez teslim ve alındılı; ölçüde eş bağlı.
#[test]
fn simultaneous_connect_from_two_threads_ends_in_one_session_with_traffic_both_ways() {
    let root = TempDir::new("glare-product");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let (alice, mut alice_events) = start(&provisioned.devices[ALICE]);
    let (bob, mut bob_events) = start(&provisioned.devices[BOB]);

    let barrier = Barrier::new(CONNECTORS);
    thread::scope(|scope| {
        scope.spawn(|| {
            barrier.wait();
            alice.connect(BOB).expect("alice connect");
        });
        scope.spawn(|| {
            barrier.wait();
            bob.connect(ALICE).expect("bob connect");
        });
    });
    wait_connected(&mut alice_events, BOB);
    wait_connected(&mut bob_events, ALICE);

    let to_bob = alice.send_text(BOB, &text_of(ALICE)).expect("alice metni");
    let to_alice = bob.send_text(ALICE, &text_of(BOB)).expect("bob metni");
    wait_text(&mut bob_events, ALICE, to_bob);
    wait_text(&mut alice_events, BOB, to_alice);
    wait_receipt(&mut alice_events, BOB, to_bob);
    wait_receipt(&mut bob_events, ALICE, to_alice);

    for (node, events, peer) in [
        (&alice, &mut alice_events, BOB),
        (&bob, &mut bob_events, ALICE),
    ] {
        let rest = events.drain_all();
        assert!(
            !rest.iter().any(|event| matches!(
                event,
                NodeEvent::Connected { .. } | NodeEvent::Closed { .. }
            )),
            "ikinci Connected ya da Closed / ConnectAbandoned yok: {rest:?}"
        );
        let duplicate_text = rest
            .iter()
            .any(|event| matches!(event, NodeEvent::TextReceived { .. }));
        assert!(!duplicate_text, "metin tam bir kez: {rest:?}");
        let metrics = node.metrics();
        assert!(
            metrics
                .connect_state
                .iter()
                .any(|state| state.peer_id == peer && state.state == LinkState::Connected),
            "{metrics:?}"
        );
    }
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}
