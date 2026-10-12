//! `KamaNode` kurulum testleri (ürün yolu, `KamaNode::start`): tipli yapılandırma hataları,
//! `root.json`'un okunmaması (D9), süreç-içi zarf rölesinde kimlik başına tek canlı düğüm, hata
//! yolunda kaynakların bırakılması, tek açılış, dinleyiciden yeniden giriş ve taze düğümün
//! ölçüleri.

mod common;

use std::fs;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock, Weak};

use kama_node::{KamaNode, LinkState, NodeError, NodeEvent, NodeListener, STORE_FILE_NAME};
use kama_provision::format::{FormatError, ROOT_FILE_NAME};
use kama_store::{RedbStore, StoreOpenError};

use common::{provision, provision_at, TempDir, WAIT};

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// Reddedilen ikizden sonra ilk düğümün gönderdiği metin.
const AFTER_TWIN_TEXT: &str = "ikizden sonra";
/// Testin kendi tuttuğu soketin adresi: 127.0.0.1, işletim sisteminin seçtiği boş port.
const LOOPBACK_ANY_PORT: &str = "127.0.0.1:0";

/// Süreç-içi zarf rölesi bir kimliği tek canlı düğüme bağlar: aynı cihaz dosyasıyla (başka veri
/// dizininde) ikinci düğüm `IdentityInUse` alır ve o noktaya dek açtığını (depo) bırakır; ilk düğüm
/// etkilenmez — eşiyle bağlanır ve metin alışverişi yapar.
#[test]
fn second_node_with_the_same_identity_is_refused_and_the_first_is_unaffected() {
    let root = TempDir::new("identity");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let alice = &provisioned.devices[ALICE];
    let bob = &provisioned.devices[BOB];
    let (listener, mut alice_events) = common::listener();
    let first = KamaNode::start(&alice.config, &alice.data, listener).expect("ilk düğüm");

    let twin_data = root.path().join("ikiz-veri");
    let (listener, _twin_events) = common::listener();
    let error = KamaNode::start(&alice.config, &twin_data, listener).expect_err("aynı kimlik");
    assert!(matches!(error, NodeError::IdentityInUse), "{error:?}");
    // Depo röle kaydından ÖNCE açılmıştı: hata yolunda kapandı (redb kilidi bırakıldı).
    RedbStore::open(&twin_data.join(STORE_FILE_NAME)).expect("depo kilidi bırakılmalı");

    // İlk düğüm etkilenmedi: kaydı duruyor; eşiyle zarf ve veri alışverişi yapar.
    let (listener, mut bob_events) = common::listener();
    let peer = KamaNode::start(&bob.config, &bob.data, listener).expect("eş");
    first.connect(BOB).expect("connect");
    alice_events.wait("alice: Connected(bob)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == BOB).then_some(())
    });
    bob_events.wait("bob: Connected(alice)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == ALICE).then_some(())
    });
    let msg_id = first.send_text(BOB, AFTER_TWIN_TEXT).expect("metin");
    bob_events.wait("bob: TextReceived(alice)", |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, .. }
                if peer_id == ALICE && *got == msg_id
        )
        .then_some(())
    });
    first.shutdown().expect("kapanış");
    peer.shutdown().expect("kapanış");
}

/// Röle kaydından SONRAKİ kurulum hatası (veri adresi başka bir sokette: `Transport` kurulamaz)
/// röle kaydını ve açılan her şeyi bırakır: adres boşalınca aynı cihaz dosyasıyla `start` başarılı
/// olur — `IdentityInUse` ya da depo kilidi DEĞİL. Test veri adresini kurulumdan önce KENDİ
/// soketiyle tutar (port yarışı yok).
#[test]
fn setup_failure_after_relay_registration_releases_the_identity_and_the_socket() {
    let root = TempDir::new("bind-busy");
    let _serial = common::serial();
    let held = UdpSocket::bind(LOOPBACK_ANY_PORT).expect("veri adresini tutan soket");
    let peer_socket = UdpSocket::bind(LOOPBACK_ANY_PORT).expect("eşin adresi için soket");
    let addrs = [
        held.local_addr().expect("adres"),
        peer_socket.local_addr().expect("adres"),
    ];
    let provisioned = provision_at(root.path(), &[ALICE, BOB], &addrs);
    let alice = &provisioned.devices[ALICE];

    let (listener, _events) = common::listener();
    let error = KamaNode::start(&alice.config, &alice.data, listener).expect_err("adres meşgul");
    assert!(matches!(error, NodeError::Transport(_)), "{error:?}");

    drop(held);
    let (listener, _events) = common::listener();
    let node = KamaNode::start(&alice.config, &alice.data, listener)
        .expect("röle kaydı, depo ve soket bırakılmıştı");
    node.shutdown().expect("kapanış");
    drop(peer_socket);
}

#[test]
fn missing_device_file_is_a_typed_config_error_and_creates_nothing() {
    let root = TempDir::new("missing");
    let data = root.path().join("data");
    let (listener, _events) = common::listener();
    let error = KamaNode::start(&root.path().join("device-yok.json"), &data, listener)
        .expect_err("cihaz dosyası yok");
    assert!(
        matches!(error, NodeError::Config(FormatError::Io(_))),
        "{error:?}"
    );
    assert!(!data.exists(), "yapılandırma hatasında veri dizini oluşmaz");
}

/// Otoritenin kök dosyası cihaz dosyası DEĞİLDİR (D9): yükleyici onu tipli hatayla reddeder.
#[test]
fn root_json_given_as_device_file_is_a_typed_config_error() {
    let root = TempDir::new("rootfile");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let (listener, _events) = common::listener();
    let error = KamaNode::start(
        &provisioned.authority.join(ROOT_FILE_NAME),
        &root.path().join("data"),
        listener,
    )
    .expect_err("root.json cihaz dosyası değil");
    assert!(
        matches!(error, NodeError::Config(FormatError::Json { .. })),
        "{error:?}"
    );
}

/// D9: düğüm cihaz dosyasının yanındaki `root.json`'u AÇMAZ. Yanına okunamayan bir `root.json`
/// (dizin) konur — açılmaya kalkılsaydı kurulum düşerdi; düğüm kurulur, taze ölçüleri verir ve
/// kapanır.
#[test]
fn start_never_opens_a_root_json_next_to_the_device_file() {
    let root = TempDir::new("noroot");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let alice = &provisioned.devices[ALICE];
    let config_dir = alice.config.parent().expect("cihaz dizini");
    fs::create_dir(config_dir.join(ROOT_FILE_NAME)).expect("okunamayan root.json");
    let (listener, _events) = common::listener();
    let node = KamaNode::start(&alice.config, &alice.data, listener)
        .expect("kurulum root.json'a bakmadan ilerlemeli");
    let metrics = node.metrics();
    assert_eq!(metrics.connect_state.len(), 1, "{metrics:?}");
    assert_eq!(metrics.connect_state[0].peer_id, BOB);
    assert_eq!(metrics.connect_state[0].state, LinkState::Idle);
    assert_eq!(metrics.epoch, None);
    assert_eq!(metrics.ctx_mismatch_drops, 0);
    assert_eq!(metrics.delivery_channel_drops, 0);
    assert!(matches!(node.connect("yok"), Err(NodeError::UnknownPeer)));
    node.shutdown().expect("kapanış");
}

#[test]
fn device_without_a_direct_address_is_a_typed_error() {
    let root = TempDir::new("nodirect");
    let provisioned = provision(root.path(), &[ALICE, BOB], false);
    let alice = &provisioned.devices[ALICE];
    let (listener, _events) = common::listener();
    let error = KamaNode::start(&alice.config, &alice.data, listener).expect_err("adres yok");
    assert!(matches!(error, NodeError::NoDirectAddress), "{error:?}");
}

/// Tek açılış: aynı `data_dir`'de ikinci düğüm depo kilidiyle reddedilir; ilki kapanınca açılır.
#[test]
fn second_node_on_the_same_data_dir_is_refused_until_the_first_shuts_down() {
    let root = TempDir::new("twice");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let alice = &provisioned.devices[ALICE];
    let open = || {
        let (listener, _events) = common::listener();
        KamaNode::start(&alice.config, &alice.data, listener)
    };
    let first = open().expect("ilk düğüm");
    let second = open().expect_err("ikinci açılış");
    assert!(
        matches!(second, NodeError::Store(StoreOpenError::AlreadyOpen)),
        "{second:?}"
    );
    first.shutdown().expect("kapanış");
    open()
        .expect("kapanıştan sonra yeniden açılır")
        .shutdown()
        .expect("kapanış");
}

/// Dinleyici düğüm iş parçacığında çağrılır: oradan yanıt bekleyen çağrı kilitlenmez, tipli hata
/// alır.
#[test]
fn blocking_call_from_the_listener_is_a_typed_error_not_a_deadlock() {
    struct Reentrant {
        node: Arc<OnceLock<Weak<KamaNode>>>,
        result: mpsc::Sender<Result<(), NodeError>>,
    }

    impl NodeListener for Reentrant {
        fn on_event(&self, _event: NodeEvent) {
            if let Some(node) = self.node.get().and_then(Weak::upgrade) {
                let _ = self.result.send(node.connect(BOB));
            }
        }
    }

    let root = TempDir::new("reentrant");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let alice = &provisioned.devices[ALICE];
    let slot = Arc::new(OnceLock::new());
    let (result_tx, result_rx) = mpsc::channel();
    let listener = Box::new(Reentrant {
        node: Arc::clone(&slot),
        result: result_tx,
    });
    let node = Arc::new(KamaNode::start(&alice.config, &alice.data, listener).expect("düğüm"));
    assert!(slot.set(Arc::downgrade(&node)).is_ok(), "tek atama");
    // Dinleyici en geç bir `METRICS_TICK` sonra (periyodik `Metrics` olayı) çağrılır.
    let result = result_rx.recv_timeout(WAIT).expect("dinleyici çağrılmalı");
    assert!(
        matches!(result, Err(NodeError::ReentrantCall)),
        "{result:?}"
    );
    // Düşüş düğümü kapatır (dinleyici geçici bir referans tutuyorsa onun düşüşü kapatır).
    drop(node);
}
