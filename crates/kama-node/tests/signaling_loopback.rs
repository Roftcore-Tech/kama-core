//! K4G-14b testi (plan K4G-14 NE (2)) — iki `KamaNode` loopback'te sinyalleşme blob'u
//! (`SIGNALING_BLOB`, AMP §3 0x09) gidiş-dönüşü; K4G-11'in CORE testinden (`two_nodes_loopback.rs`)
//! AYRI.
//!
//! ÜRÜN yolu (`KamaNode::start`), kurulum `two_nodes_loopback` ile aynıdır: `kama-provision`
//! kütüphanesiyle üretilmiş cihaz dosyaları, gerçek kripto + depo + UDP; zarflar süreç-içi röleden.
//! Röle kayıtsız eşe zarf saklamaz: iki düğüm de bağlanmadan ÖNCE başlatılır.
//!
//! (i) blob iki yönde gider, baytlar özdeş gelir; (ii) aynı oturumda gönderim sırası korunur ve her
//! blob bir kez gelir (oturum değişimi yok: yeniden sunum yok); (iii) boyu aşan blob tipli hatadır
//! (`TooLarge`; komut kuyruğuna girmez) ve eşe hiçbir şey ulaşmaz — limit boyundaki blob ulaşır;
//! (iv) bilinmeyen eş tipli hatadır (`UnknownPeer`); oturumsuz eşe gönderim motorun tipli hatasıdır
//! (`Signaling(NoSession)`).
//!
//! Zamanlama: gerçek uyku yok; her bekleme bir olayın ÜST SINIRIDIR (`common::WAIT`).

mod common;

use kama_app::signaling::{SignalingError, SignalingId};
use kama_node::{KamaNode, NodeError, NodeEvent};

use common::{provision, Device, Events, TempDir};

const ALICE: &str = "alice";
const BOB: &str = "bob";
/// Provisioning dosyasında olmayan eş adı.
const STRANGER: &str = "yabanci";

/// Bir yönde art arda (teslim beklenmeden) gönderilen blob sayısı: sıra sınaması için birden çok.
const BLOBS: usize = 8;
/// Blob boyları: ilk blob'un boyu ve blob'dan blob'a artış — her blob farklı boyda (yer değiştiren
/// ya da kırpılan blob görünür). Değerler yalnız birbirinden ayırt edilebilir olmalı.
const FIRST_LEN: usize = 16;
const LEN_STEP: usize = 37;
/// İçerik deseni: asal modül, boylarla hizalanmaz.
const PATTERN: usize = 251;
/// Yön başına desen başlangıcı: iki yönün blob'ları birbirinden ayırt edilir.
const ALICE_SEED: usize = 0;
const BOB_SEED: usize = 101;
/// Limit sondası: her AMP kaydından büyük bir girdi — dönen `TooLarge` düğümün limitini söyler.
const PROBE_LEN: usize = 64 * 1024;
/// AMP §3 0x09 (`blob_len u16_be · blob`): ürün profilinde `MAX_BODY_BYTES` 1008 B (plan D14: TEXT
/// ≤ 1002 B + TEXT öneki 6 B) − `blob_len` 2 B — düğümün limitine karşı BAĞIMSIZ kaynak.
const PRODUCT_MAX_BLOB_BYTES: usize = 1006;
/// Engel metni: aynı oturumda blob'lardan SONRA gider (Reliable, sıralı) — alıcı onu işlediyse
/// ondan önce gelmiş her kaydı da işlemiştir.
const BARRIER_TEXT: &str = "engel";
/// Hata yolu sınamasının blob'u (boyu limitin altında; içerik önemsiz).
const SMALL_BLOB: &[u8] = &[0x0A, 0x0B, 0x0C];

/// Ürün yolu: `KamaNode::start`.
fn start(device: &Device) -> (KamaNode, Events) {
    let (listener, events) = common::listener();
    let node = KamaNode::start(&device.config, &device.data, listener).expect("düğüm kurulmalı");
    (node, events)
}

fn connected(name: &str) -> impl FnMut(&NodeEvent) -> Option<()> + '_ {
    move |event| matches!(event, NodeEvent::Connected { peer_id } if peer_id == name).then_some(())
}

/// İki düğüm de başlatılmış olarak: Alice bağlanır, iki uç `Connected` görür.
fn connect(alice: &KamaNode, alice_events: &mut Events, bob_events: &mut Events) {
    alice.connect(BOB).expect("connect");
    alice_events.wait("alice: Connected(bob)", connected(BOB));
    bob_events.wait("bob: Connected(alice)", connected(ALICE));
}

fn pattern_byte(value: usize) -> u8 {
    u8::try_from(value % PATTERN).expect("desen bir bayta sığar")
}

/// `seed`'den türeyen `BLOBS` blob: her biri farklı boy ve içerikte.
fn blobs(seed: usize) -> Vec<Vec<u8>> {
    (0..BLOBS)
        .map(|index| {
            let len = FIRST_LEN + index * LEN_STEP;
            (0..len)
                .map(|offset| pattern_byte(seed + index + offset))
                .collect()
        })
        .collect()
}

/// Blob'ları art arda, teslim beklemeden gönderir; kimlikleri gönderim sırasıyla.
fn send_all(node: &KamaNode, to: &str, blobs: &[Vec<u8>]) -> Vec<SignalingId> {
    blobs
        .iter()
        .map(|blob| node.send_signaling(to, blob.clone()).expect("blob kuyruğa"))
        .collect()
}

fn blob_from(from: &str) -> impl FnMut(&NodeEvent) -> Option<Vec<u8>> + '_ {
    move |event| match event {
        NodeEvent::SignalingReceived { peer_id, bytes } if peer_id == from => Some(bytes.clone()),
        _ => None,
    }
}

/// `from`'un `count` blob'u GELİŞ sırasıyla: her bekleme, saklananlar dahil, ilk eşleşeni alır.
fn receive(events: &mut Events, from: &str, count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|index| {
            events.wait(
                &format!("SignalingReceived({from}) #{index}"),
                blob_from(from),
            )
        })
        .collect()
}

/// Alıcının `from`'dan sıradaki olayı: fazladan bir blob mu, engel metni mi.
#[derive(Debug, PartialEq, Eq)]
enum Next {
    /// Engel metni.
    Barrier,
    /// Fazladan blob (boyu).
    ExtraBlob(usize),
}

/// `sender` engel metnini gönderir; alıcının `from`'dan İLK sıradaki blob'u ya da engel olayı
/// engel olmalıdır: önceki blob'lardan fazlası (yinelenen) gelmedi.
fn expect_barrier_next(sender: &KamaNode, to: &str, events: &mut Events, from: &str) {
    let barrier = sender.send_text(to, BARRIER_TEXT).expect("engel metni");
    let next = events.wait(&format!("{to}: engel ya da blob"), |event| match event {
        NodeEvent::SignalingReceived { peer_id, bytes } if peer_id == from => {
            Some(Next::ExtraBlob(bytes.len()))
        }
        NodeEvent::TextReceived {
            peer_id, msg_id, ..
        } if peer_id == from && *msg_id == barrier => Some(Next::Barrier),
        _ => None,
    });
    assert_eq!(next, Next::Barrier, "her blob bir kez gelir");
}

fn assert_strictly_increasing(ids: &[SignalingId]) {
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "düğüm-tekil kimlikler kesin artan: {ids:?}"
    );
}

fn too_large(result: Result<SignalingId, NodeError>) -> (&'static str, usize, usize) {
    match result {
        Err(NodeError::TooLarge { what, len, max }) => (what, len, max),
        other => panic!("TooLarge bekleniyordu: {other:?}"),
    }
}

/// (i) + (ii): blob'lar iki yönde, art arda gönderilir; alıcı onları bayt-özdeş, gönderim
/// sırasıyla ve birer kez alır. Dönen kimlikler düğüm başına kesin artandır.
#[test]
fn blobs_round_trip_both_ways_byte_identical_in_send_order_and_once() {
    let root = TempDir::new("signaling-round-trip");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let (bob, mut bob_events) = start(&provisioned.devices[BOB]);
    let (alice, mut alice_events) = start(&provisioned.devices[ALICE]);
    connect(&alice, &mut alice_events, &mut bob_events);

    // Alice → Bob.
    let to_bob = blobs(ALICE_SEED);
    let alice_ids = send_all(&alice, BOB, &to_bob);
    assert_eq!(
        receive(&mut bob_events, ALICE, to_bob.len()),
        to_bob,
        "bayt-özdeş, gönderim sırasıyla"
    );
    expect_barrier_next(&alice, BOB, &mut bob_events, ALICE);

    // Bob → Alice.
    let to_alice = blobs(BOB_SEED);
    let bob_ids = send_all(&bob, ALICE, &to_alice);
    assert_eq!(
        receive(&mut alice_events, BOB, to_alice.len()),
        to_alice,
        "bayt-özdeş, gönderim sırasıyla"
    );
    expect_barrier_next(&bob, ALICE, &mut alice_events, BOB);

    assert_strictly_increasing(&alice_ids);
    assert_strictly_increasing(&bob_ids);
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}

/// (iii): boyu aşan blob komut kuyruğuna girmeden `TooLarge` alır — sınır TAM limittir ve ürün
/// profilinin değeridir —; eşe hiçbir şey ulaşmaz: Bob'un Alice'ten aldığı İLK blob, reddedilen iki
/// girdiden SONRA gönderilen limit boyundaki blob'dur (bayt-özdeş).
#[test]
fn oversize_blob_is_a_typed_error_and_reaches_nothing_while_the_limit_itself_arrives() {
    let root = TempDir::new("signaling-too-large");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let (bob, mut bob_events) = start(&provisioned.devices[BOB]);
    let (alice, mut alice_events) = start(&provisioned.devices[ALICE]);
    connect(&alice, &mut alice_events, &mut bob_events);

    let (what, len, max) = too_large(alice.send_signaling(BOB, vec![0; PROBE_LEN]));
    assert_eq!((what, len), ("signaling blob", PROBE_LEN));
    assert_eq!(max, PRODUCT_MAX_BLOB_BYTES, "AMP §3 0x09, ürün profili");
    assert_eq!(
        too_large(alice.send_signaling(BOB, vec![0; max + 1])),
        ("signaling blob", max + 1, max)
    );
    let largest: Vec<u8> = (0..max).map(pattern_byte).collect();
    alice
        .send_signaling(BOB, largest.clone())
        .expect("limit boyundaki blob");
    assert_eq!(
        bob_events.wait("bob: alice'in ilk blob'u", blob_from(ALICE)),
        largest,
        "reddedilen girdilerden eşe hiçbir şey ulaşmadı"
    );
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}

/// (iv): bilinmeyen eş `UnknownPeer` alır (çağrı düğüme ulaşmaz); provisioning'deki ama oturumu
/// olmayan eşe gönderim motorun tipli hatasıdır (`Signaling(NoSession)`) — kuyruğa alınmaz.
#[test]
fn unknown_peer_and_peer_without_session_are_typed_errors() {
    let root = TempDir::new("signaling-errors");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let (alice, _alice_events) = start(&provisioned.devices[ALICE]);

    let unknown = alice.send_signaling(STRANGER, SMALL_BLOB.to_vec());
    assert!(
        matches!(unknown, Err(NodeError::UnknownPeer)),
        "{unknown:?}"
    );
    let no_session = alice.send_signaling(BOB, SMALL_BLOB.to_vec());
    assert!(
        matches!(
            no_session,
            Err(NodeError::Signaling(SignalingError::NoSession))
        ),
        "{no_session:?}"
    );
    alice.shutdown().expect("alice kapanış");
}
