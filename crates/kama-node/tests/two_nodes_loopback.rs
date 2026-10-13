//! K4G-11 CORE testi (plan kart NE 4) — iki `KamaNode` aynı süreçte, loopback UDP.
//!
//! ÜRÜN yolu (`KamaNode::start`): `kama-provision` kütüphanesiyle test içinde üretilmiş iki cihaz
//! dosyası (elle yazılmış fikstür DEĞİL; `root.json` cihaz dizinlerine girmez), gerçek anahtarlar,
//! gerçek saatler (Crypto + Application ayrı), OS entropisi, `kama-store::RedbStore` (aynı nesne
//! `AppStore` + `CounterStore`), gerçek `ClientTransport` + `NoiseWireProtection` + UDP veri yolu,
//! `kama-app` Messenger / Ptt / FileTransfer motorları ve düğüm döngüsü.
//!
//! Zarf taşıyıcısı (sahip kararı DUR-K4G-11 → B): süreç-içi zarf rölesi (`LoopbackEnvelopeRelay`):
//! ürün taşıyıcısı değildir — iki cihaz arası taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti.
//! Veri datagramları UDP loopback'ten, el sıkışma zarfları röleden geçer. Röle kayıtsız eşe zarf
//! saklamaz: iki düğüm de bağlanmadan ÖNCE başlatılır.
//!
//! Akış: TEXT ↔ RECEIPT (iki yön) · 1 dosya (`FILE_RECEIPT(ok)` dahil; yol olayla, içerik diskte
//! bayt-özdeş) · 1 PTT akışı (OPEN, sıralı demetler, CLOSE) + kapanmış tutamaçların tipli hatası ·
//! ölçüler (epoch-1 iki uçta, `ctx_mismatch_drops == 0`, `delivery_channel_drops == 0`; anlık
//! görüntü ilgili aşamadan SONRA alınır) · Bob kapalıyken Alice'in metni outbox'ta kalır · iki
//! düğüm kapatılıp AYNI `data_dir` ile yeniden açılır ve bağlantıyı arka plan boşaltması
//! (`drain_background`, PoC K6 / EK-2) kurar: outbox'taki metin AYNI `msg_id`'yle teslim olur,
//! yeni `msg_id` ve `init_counter` artmıştır.
//!
//! Zamanlama: gerçek uyku yok; her bekleme bir olayın ÜST SINIRIDIR (`common::WAIT`).

mod common;

use std::fs;

use kama_app::codec::PttCodec;
use kama_app::messaging::MsgId;
use kama_app::store::AppStore;
use kama_node::{
    FileStage, KamaNode, LinkState, NodeError, NodeEvent, RECEIVED_FILES_DIR, STORE_FILE_NAME,
};
use kama_noise_wire::CounterStore;
use kama_store::RedbStore;

use common::{peer_ref_of, provision, Device, Events, TempDir};

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// PoC fotoğrafı ≈150 KB (PoC-KAPSAM-001 §2 "Fotoğraf": 1 MB → ≈150 KB).
const PHOTO_BYTES: usize = 150 * 1024;
/// Fotoğraf içeriğinin deseni: asal modül, parça sınırlarıyla hizalanmaz (yer değiştiren parça
/// görünür olur).
const PHOTO_PATTERN: usize = 251;
/// Test basışı: demet sayısı, süresi ve boyu. Değerler media'nındır (AMP §3 0x06; düğüm
/// yorumlamaz); burada yalnız gidiş-dönüşün sınanacağı kadar.
const BUNDLES: usize = 8;
const BUNDLE_MS: u8 = 40;
const BUNDLE_LEN: usize = 64;

/// Ürün yolu: `KamaNode::start`.
fn start(device: &Device) -> (KamaNode, Events) {
    let (listener, events) = common::listener();
    let node = KamaNode::start(&device.config, &device.data, listener).expect("düğüm kurulmalı");
    (node, events)
}

fn connect(alice: &KamaNode, alice_events: &mut Events, bob_events: &mut Events) {
    alice.connect(BOB).expect("connect");
    wait_connected(alice_events, bob_events);
}

fn wait_connected(alice_events: &mut Events, bob_events: &mut Events) {
    alice_events.wait("alice: Connected(bob)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == BOB).then_some(())
    });
    bob_events.wait("bob: Connected(alice)", |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == ALICE).then_some(())
    });
}

fn expect_text(events: &mut Events, from: &str, msg_id: MsgId, text: &str) {
    events.wait(&format!("TextReceived({from}, {msg_id})"), |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, text: body }
                if peer_id == from && *got == msg_id && body == text
        )
        .then_some(())
    });
}

fn expect_receipt(events: &mut Events, from: &str, msg_id: MsgId) {
    events.wait(&format!("Receipt({from}, {msg_id})"), |event| {
        matches!(
            event,
            NodeEvent::Receipt { peer_id, msg_id: got } if peer_id == from && *got == msg_id
        )
        .then_some(())
    });
}

/// Eşle oturum hazır ve epoch-1'de; bağlam-önce dağıtım ve teslim kanalı hiçbir şey düşürmedi.
///
/// Anlık görüntü bu çağrıdan SONRA hesaplanmış olmalıdır (önceki aşamanın bütün düşmelerini
/// kapsasın): kanalda bekleyen eski olaylar önce boşaltılır; boşaltmadan sonra gelen İLK `Metrics`
/// atlanır (boşaltmadan önce hesaplanıp sonra gönderilmiş olabilir); ondan sonrakiler düğüm
/// iş parçacığında ilkinin gönderiminden SONRA hesaplanmıştır.
fn expect_clean_metrics(events: &mut Events, peer: &str) {
    events.drain_now();
    events.wait("Metrics (boşaltmadan sonraki ilk; atlanır)", |event| {
        matches!(event, NodeEvent::Metrics(_)).then_some(())
    });
    let metrics = events.wait(
        &format!("Metrics: {peer} bağlı, epoch 1"),
        |event| match event {
            NodeEvent::Metrics(metrics)
                if metrics.connect_state.iter().any(|state| {
                    state.peer_id == peer
                        && state.state == LinkState::Connected
                        && state.epoch == Some(1)
                }) =>
            {
                Some(metrics.clone())
            }
            _ => None,
        },
    );
    assert_eq!(metrics.epoch, Some(1), "{metrics:?}");
    assert_eq!(metrics.ctx_mismatch_drops, 0, "{metrics:?}");
    assert_eq!(metrics.delivery_channel_drops, 0, "{metrics:?}");
    assert_eq!(metrics.hub_frames_dropped, 0, "{metrics:?}");
    // Salt-okunur kaynağı olmayan alanlar uydurulmaz; heartbeat PoC'de yok (D18).
    assert_eq!(metrics.rtt_ms, None);
    assert_eq!(metrics.loss_pct, None);
    assert_eq!(metrics.bitrate_kbps, None);
    assert_eq!(metrics.last_heartbeat_age_s, None);
}

#[test]
fn text_file_ptt_and_restart_persistence_over_loopback_udp() {
    let root = TempDir::new("loopback");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let alice_device = &provisioned.devices[ALICE];
    let bob_device = &provisioned.devices[BOB];

    // ── Koşu 1 ──────────────────────────────────────────────────────────────────────────────
    let (bob, mut bob_events) = start(bob_device);
    let (alice, mut alice_events) = start(alice_device);
    connect(&alice, &mut alice_events, &mut bob_events);
    alice_events.wait("alice: ConnectProgress(bob)", |event| {
        matches!(event, NodeEvent::ConnectProgress { peer_id, .. } if peer_id == BOB).then_some(())
    });

    // TEXT ↔ RECEIPT, iki yön.
    let m1 = alice.send_text(BOB, "merhaba bob").expect("metin");
    expect_text(&mut bob_events, ALICE, m1, "merhaba bob");
    expect_receipt(&mut alice_events, BOB, m1);
    let n1 = bob.send_text(ALICE, "merhaba alice").expect("metin");
    expect_text(&mut alice_events, BOB, n1, "merhaba alice");
    expect_receipt(&mut bob_events, ALICE, n1);

    // 1 dosya: alınan yol olayla, içerik bayt-özdeş, `FILE_RECEIPT(ok)` gönderende `Delivered`.
    let photo: Vec<u8> = (0..PHOTO_BYTES)
        .map(|i| u8::try_from(i % PHOTO_PATTERN).expect("desen bir bayta sığar"))
        .collect();
    let source = root.path().join("foto.jpg");
    fs::write(&source, &photo).expect("kaynak dosya");
    let file = alice.send_file(BOB, &source).expect("dosya kuyruğa");
    let (path, size) = bob_events.wait("bob: FileReceived(alice)", |event| match event {
        NodeEvent::FileReceived {
            peer_id,
            path,
            size,
            ..
        } if peer_id == ALICE => Some((path.clone(), *size)),
        _ => None,
    });
    assert_eq!(size, u64::try_from(PHOTO_BYTES).expect("boy"));
    assert_eq!(fs::read(&path).expect("alınan dosya"), photo);
    assert!(
        path.starts_with(bob_device.data.join(RECEIVED_FILES_DIR)),
        "{path:?}"
    );
    let stored_name = path.file_name().and_then(|name| name.to_str());
    assert!(
        stored_name.is_some_and(|name| name.starts_with("alice-") && !name.contains("foto")),
        "ad eşin provisioning adından ve transfer_id'den; FILE_BEGIN.name'den TÜREMEZ: {path:?}"
    );
    alice_events.wait("alice: FileProgress(Sent)", |event| {
        matches!(
            event,
            NodeEvent::FileProgress { file: got, stage: FileStage::Sent, .. } if *got == file
        )
        .then_some(())
    });
    alice_events.wait("alice: FILE_RECEIPT(ok)", |event| {
        matches!(
            event,
            NodeEvent::FileProgress { file: got, stage: FileStage::Delivered, .. } if *got == file
        )
        .then_some(())
    });
    assert!(
        matches!(alice.cancel_file(file), Err(NodeError::HandleClosed)),
        "biten aktarımın tutamacı kapalıdır"
    );

    // 1 PTT akışı: OPEN → sıralı demetler → CLOSE; kapanmış tutamaç tipli hata alır.
    let stream = alice
        .ptt_open(BOB, PttCodec::Lyra, BUNDLE_MS)
        .expect("ptt_open");
    assert!(stream.raw() > file.raw(), "tek düğüm sayacı, kesin artan");
    let bundles: Vec<Vec<u8>> = (0..BUNDLES)
        .map(|i| vec![u8::try_from(i).expect("demet sırası"); BUNDLE_LEN])
        .collect();
    for bundle in &bundles {
        alice.ptt_push(stream, bundle.clone()).expect("demet");
    }
    alice.ptt_close(stream).expect("ptt_close");
    let stream_id = bob_events.wait("bob: PttOpened(alice)", |event| match event {
        NodeEvent::PttOpened {
            peer_id,
            stream_id,
            codec: PttCodec::Lyra,
            bundle_ms,
        } if peer_id == ALICE && *bundle_ms == BUNDLE_MS => Some(*stream_id),
        _ => None,
    });
    for (index, expected) in bundles.iter().enumerate() {
        let got = bob_events.wait(&format!("bob: PttBundle #{index}"), |event| match event {
            NodeEvent::PttBundle {
                peer_id,
                stream_id: got,
                payload,
            } if peer_id == ALICE && *got == stream_id => Some(payload.clone()),
            _ => None,
        });
        assert_eq!(&got, expected, "demetler gönderim sırasıyla (PTT-R0)");
    }
    bob_events.wait("bob: PttClosed(alice)", |event| {
        matches!(
            event,
            NodeEvent::PttClosed { peer_id, stream_id: got }
                if peer_id == ALICE && *got == stream_id
        )
        .then_some(())
    });
    assert!(matches!(
        alice.ptt_push(stream, vec![0; BUNDLE_LEN]),
        Err(NodeError::HandleClosed)
    ));
    assert!(matches!(
        alice.ptt_close(stream),
        Err(NodeError::HandleClosed)
    ));
    let next = alice
        .ptt_open(BOB, PttCodec::Lyra, BUNDLE_MS)
        .expect("yeni basış");
    assert!(next.raw() > stream.raw(), "tutamaç yeniden kullanılmaz");
    alice.ptt_close(next).expect("ptt_close");

    expect_clean_metrics(&mut alice_events, BOB);
    expect_clean_metrics(&mut bob_events, ALICE);

    // Bob kapalıyken gönderilen metin Alice'in kalıcı outbox'ında bekler (hub mailbox yok, EK-2).
    bob.shutdown().expect("bob kapanış");
    let m2 = alice
        .send_text(BOB, "bob kapalıyken")
        .expect("metin kuyruğa");
    assert!(m2 > m1, "eş başına msg_id kesin artan");
    alice.shutdown().expect("alice kapanış");

    // Düğümler kapalı: depo dosyası kilitsiz; outbox'ta yalnız m2 var, init_counter ayrılmış.
    let bob_ref = peer_ref_of(&alice_device.config, BOB);
    let init_after_first = {
        let store = RedbStore::open(&alice_device.data.join(STORE_FILE_NAME))
            .expect("kapanan düğüm depo kilidini bırakır");
        let pending: Vec<MsgId> = AppStore::outbox_list(&store, bob_ref)
            .expect("outbox")
            .iter()
            .map(|unit| unit.msg_id())
            .collect();
        assert_eq!(pending, [m2], "RECEIPT'siz metin kalıcı outbox'ta");
        CounterStore::reserve_next(&store).expect("init_counter")
    };
    assert!(
        init_after_first >= 1,
        "koşu 1'in INIT'i kalıcı sayaçtan ayrıldı"
    );

    // ── Koşu 2: aynı cihaz dosyaları, AYNI data_dir ─────────────────────────────────────────
    let (bob, mut bob_events) = start(bob_device);
    let (alice, mut alice_events) = start(alice_device);
    // Arka plan boşaltması (PoC K6 / EK-2): oturumu ve bekleyen niyeti olmayan eşe `connect`
    // tetikler; eşin kalıcı outbox'ı yüklenir ve kayıt sıradaki oturumda AYNI `msg_id`'yle gider.
    alice.drain_background().expect("drain_background");
    wait_connected(&mut alice_events, &mut bob_events);
    expect_text(&mut bob_events, ALICE, m2, "bob kapalıyken");
    expect_receipt(&mut alice_events, BOB, m2);
    let m3 = alice.send_text(BOB, "yeniden").expect("metin");
    assert!(m3 > m2, "msg_id yeniden başlatmadan sonra da artar (D4)");
    expect_text(&mut bob_events, ALICE, m3, "yeniden");
    expect_receipt(&mut alice_events, BOB, m3);
    expect_clean_metrics(&mut alice_events, BOB);
    expect_clean_metrics(&mut bob_events, ALICE);
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");

    let store = RedbStore::open(&alice_device.data.join(STORE_FILE_NAME)).expect("depo kilitsiz");
    assert!(
        AppStore::outbox_list(&store, bob_ref)
            .expect("outbox")
            .is_empty(),
        "teslim edilen metin outbox'tan çıktı"
    );
    let init_after_second = CounterStore::reserve_next(&store).expect("init_counter");
    // Koşu 2'nin INIT'i (en az bir) testin ayırdığı değerden SONRA ayrıldı: sayaç yeniden
    // başlatmada sıfırlanmadı, kalıcı değerden devam etti.
    assert!(
        init_after_second >= init_after_first + 2,
        "init_counter: koşu 1 sonrası {init_after_first}, koşu 2 sonrası {init_after_second}"
    );
}
