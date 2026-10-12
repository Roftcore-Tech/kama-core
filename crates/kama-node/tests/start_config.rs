//! `KamaNode` kurulum testleri: tipli yapılandırma hataları, `root.json`'un okunmaması (D9), ürün
//! zarf taşıyıcısının tipli bekleme noktası (DUR-K4G-11), hata yolunda kaynakların bırakılması, tek
//! açılış, dinleyiciden yeniden giriş ve taze düğümün ölçüleri.

mod common;

use std::fs;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock, Weak};

use client_core::memory_link::MemoryHub;
use kama_node::{
    KamaNode, LinkState, NodeError, NodeEvent, NodeListener, ENVELOPE_LINK_PENDING, STORE_FILE_NAME,
};
use kama_provision::format::{FormatError, ROOT_FILE_NAME};
use kama_store::{RedbStore, StoreOpenError};

use common::{provision, RelayLink, TempDir, WAIT};

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// Ürün kurucusu bugün zarf taşıyıcısında TİPLİ hatayla durur (DUR-K4G-11; `todo!` yok) ve o
/// noktaya dek açtığını (depo) bırakır.
#[test]
fn product_start_stops_at_the_envelope_carrier_with_a_typed_error() {
    let root = TempDir::new("carrier");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let alice = &provisioned.devices[ALICE];
    let (listener, _events) = common::listener();
    let error = KamaNode::start(&alice.config, &alice.data, listener).expect_err("DUR-K4G-11");
    assert!(
        matches!(error, NodeError::FeatureNotImplemented(name) if name == ENVELOPE_LINK_PENDING),
        "{error:?}"
    );
    // Depo taşıyıcıdan ÖNCE açılmıştı: hata yolunda kapandı (redb kilidi bırakıldı).
    RedbStore::open(&alice.data.join(STORE_FILE_NAME)).expect("depo kilidi bırakılmalı");
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
    let hub = MemoryHub::new();
    let (listener, _events) = common::listener();
    let node = KamaNode::start_with_envelope_link(&alice.config, &alice.data, listener, |me| {
        RelayLink::boxed(&hub, me)
    })
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
    let hub = MemoryHub::new();
    let open = || {
        let (listener, _events) = common::listener();
        KamaNode::start_with_envelope_link(&alice.config, &alice.data, listener, |me| {
            RelayLink::boxed(&hub, me)
        })
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
    let hub = MemoryHub::new();
    let node = Arc::new(
        KamaNode::start_with_envelope_link(&alice.config, &alice.data, listener, |me| {
            RelayLink::boxed(&hub, me)
        })
        .expect("düğüm"),
    );
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
