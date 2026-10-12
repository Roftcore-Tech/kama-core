//! `KamaNode` API yolları (PR beyanlı istisnalarının davranışları): `send_file`'ın hatası OLAY
//! olarak gelir, `drain_background` boştaki eşlere bağlanır, `cancel_connect` bekleyen niyeti
//! `Closed { Abandoned(Cancelled) }` ile bitirir, `TooLarge` sınırı tam limittedir ve `shutdown`
//! kendinden önce kuyruğa girmiş komutları işler.
//!
//! Kurulum `two_nodes_loopback` ile aynıdır: `kama-provision` kütüphanesiyle üretilmiş cihaz
//! dosyaları, gerçek kripto + depo + UDP; zarflar test rölesiyle (DUR-K4G-11 bekleniyor). Gerçek
//! uyku yok; her bekleme bir olayın ÜST SINIRIDIR (`common::WAIT`).

mod common;

use std::io::ErrorKind;
use std::path::Path;

use client_core::memory_link::MemoryHub;
use kama_app::codec::PttCodec;
use kama_app::file_transfer::FileSendError;
use kama_contracts::s1::AbandonReason;
use kama_node::{
    CloseCause, FileFailReason, FileHandle, FileRef, KamaNode, NodeError, NodeEvent, StreamHandle,
};

use common::{provision, Device, Events, RelayLink, TempDir};

const ALICE: &str = "alice";
const BOB: &str = "bob";

/// Provisioning dosyasında olmayan eş adı.
const STRANGER: &str = "yabanci";
/// Limit sondası: her AMP kaydından büyük bir girdi (`MAX_BODY_BYTES` bir birime sığar; birim
/// 1 KiB mertebesindedir) — dönen `TooLarge` düğümün limitini söyler.
const PROBE_LEN: usize = 64 * 1024;
/// Plan D14: tek TEXT ≤ 1002 B — sondadan okunan tavana karşı BAĞIMSIZ kaynak.
const D14_MAX_TEXT_BYTES: usize = 1002;
/// Test basışının demet süresi — media'nındır (AMP §3 0x06; düğüm yorumlamaz).
const BUNDLE_MS: u8 = 40;
/// Kapanıştan önce kuyruğa konan komut sayısı: komut kuyruğu tavanının (64) altında — hiçbiri
/// `Busy` almaz, `Shutdown` hepsinin ardına girer.
const QUEUED_BEFORE_SHUTDOWN: usize = 8;

/// DUR-K4G-11 bekleniyor: test rölesiyle kurulum.
fn start(device: &Device, hub: &MemoryHub) -> (KamaNode, Events) {
    let (listener, events) = common::listener();
    let node = KamaNode::start_with_envelope_link(&device.config, &device.data, listener, |me| {
        RelayLink::boxed(hub, me)
    })
    .expect("düğüm kurulmalı");
    (node, events)
}

fn wait_connected(events: &mut Events, peer: &str) {
    events.wait(&format!("Connected({peer})"), |event| {
        matches!(event, NodeEvent::Connected { peer_id } if peer_id == peer).then_some(())
    });
}

/// `file`'ın `FileFailed` olayı — çağıran tutamacı almadan önce gelmiş olabilir (saklanan olaylar
/// önce taranır).
fn wait_file_failed(events: &mut Events, file: FileHandle) -> (String, FileFailReason) {
    events.wait(&format!("FileFailed({file:?})"), |event| match event {
        NodeEvent::FileFailed {
            file: FileRef::Outgoing(got),
            peer_id,
            reason,
        } if *got == file => Some((peer_id.clone(), *reason)),
        _ => None,
    })
}

fn too_large(result: Result<impl std::fmt::Debug, NodeError>) -> (&'static str, usize, usize) {
    match result {
        Err(NodeError::TooLarge { what, len, max }) => (what, len, max),
        other => panic!("TooLarge bekleniyordu: {other:?}"),
    }
}

/// `send_file` tutamacı hemen döner; kuyruğa alınamayan dosyanın hatası O tutamacın
/// `FileFailed` olayıdır (eşe hiçbir şey gitmez) ve tutamaç kapanır.
#[test]
fn send_file_failures_arrive_as_events_for_the_returned_handle() {
    let root = TempDir::new("send-file-errors");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let hub = MemoryHub::new();
    let (bob, mut bob_events) = start(&provisioned.devices[BOB], &hub);
    let (alice, mut alice_events) = start(&provisioned.devices[ALICE], &hub);
    let source = root.path().join("kaynak.bin");
    std::fs::write(&source, b"icerik").expect("kaynak dosya");

    // Eş adı çağıranda çözülür: bilinmeyen eş tutamaç almaz.
    assert!(matches!(
        alice.send_file(STRANGER, &source),
        Err(NodeError::UnknownPeer)
    ));

    // Oturum yok: dosya okunmaz, kuyruğa alınmaz.
    let no_session = alice.send_file(BOB, &source).expect("tutamaç");
    assert_eq!(
        wait_file_failed(&mut alice_events, no_session),
        (
            BOB.to_owned(),
            FileFailReason::NotQueued(FileSendError::NoSession)
        )
    );
    assert!(matches!(
        alice.cancel_file(no_session),
        Err(NodeError::HandleClosed)
    ));

    alice.connect(BOB).expect("connect");
    wait_connected(&mut alice_events, BOB);
    wait_connected(&mut bob_events, ALICE);

    // Kaynak yok / düzenli dosya değil: okuma hatası olaydır.
    let missing = alice
        .send_file(BOB, &root.path().join("yok.bin"))
        .expect("tutamaç");
    let directory = alice.send_file(BOB, root.path()).expect("tutamaç");
    assert!(directory.raw() > missing.raw(), "tutamaçlar kesin artan");
    assert_eq!(
        wait_file_failed(&mut alice_events, missing),
        (
            BOB.to_owned(),
            FileFailReason::SourceUnreadable(ErrorKind::NotFound)
        )
    );
    assert_eq!(
        wait_file_failed(&mut alice_events, directory),
        (
            BOB.to_owned(),
            FileFailReason::SourceUnreadable(ErrorKind::InvalidInput)
        )
    );
    for file in [missing, directory] {
        assert!(matches!(
            alice.cancel_file(file),
            Err(NodeError::HandleClosed)
        ));
    }
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}

/// `drain_background` oturumu ve bekleyen niyeti olmayan eşe `connect` tetikler (hub
/// mailbox yok: hiçbir şey "çekilmez"); hazır oturumlu eşte yeniden çağrı yeni niyet açmaz.
#[test]
fn drain_background_connects_idle_peers() {
    let root = TempDir::new("drain");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let hub = MemoryHub::new();
    let (bob, mut bob_events) = start(&provisioned.devices[BOB], &hub);
    let (alice, mut alice_events) = start(&provisioned.devices[ALICE], &hub);

    alice.drain_background().expect("drain_background");
    wait_connected(&mut alice_events, BOB);
    wait_connected(&mut bob_events, ALICE);
    alice
        .drain_background()
        .expect("hazır oturumlu eşte etkisiz");
    let text = "bosaltmadan sonra";
    let msg_id = alice.send_text(BOB, text).expect("oturum hazır");
    bob_events.wait("bob: TextReceived", |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, text: body }
                if peer_id == ALICE && *got == msg_id && body == text
        )
        .then_some(())
    });
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}

/// `cancel_connect` bekleyen niyeti bitirir: `true` ve `Closed { Abandoned(Cancelled) }`; niyet
/// bittikten sonra yeniden çağrı `false` (olay yok).
#[test]
fn cancel_connect_ends_the_pending_intent_with_closed() {
    let root = TempDir::new("cancel-connect");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    // Bob hiç başlamaz: INIT rölede bekler, niyet çözülmez.
    let hub = MemoryHub::new();
    let (alice, mut events) = start(&provisioned.devices[ALICE], &hub);

    alice.connect(BOB).expect("connect");
    assert!(
        alice.cancel_connect(BOB).expect("cancel_connect"),
        "bekleyen niyet vardı"
    );
    events.wait("Closed(bob, Abandoned(Cancelled))", |event| {
        matches!(
            event,
            NodeEvent::Closed {
                peer_id,
                reason: CloseCause::Abandoned(AbandonReason::Cancelled),
            } if peer_id == BOB
        )
        .then_some(())
    });
    // `Closed` niyetin son olayı işlendikten SONRA verilir: artık bekleyen niyet yok.
    assert!(!alice.cancel_connect(BOB).expect("cancel_connect"));
    assert!(matches!(
        alice.cancel_connect(STRANGER),
        Err(NodeError::UnknownPeer)
    ));
    alice.shutdown().expect("alice kapanış");
}

/// `TooLarge`: metin ve PTT yükü komut kuyruğuna girmeden reddedilir; sınır TAM limittir — limit
/// boyundaki girdi kabul edilir ve eşe bayt-özdeş ulaşır, bir bayt fazlası reddedilir.
#[test]
fn oversize_inputs_are_typed_errors_and_the_limit_itself_fits() {
    let root = TempDir::new("too-large");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let hub = MemoryHub::new();
    let (bob, mut bob_events) = start(&provisioned.devices[BOB], &hub);
    let (alice, mut alice_events) = start(&provisioned.devices[ALICE], &hub);
    alice.connect(BOB).expect("connect");
    wait_connected(&mut alice_events, BOB);
    wait_connected(&mut bob_events, ALICE);

    // Metin: limit sondadan okunur.
    let (what, len, max_text) = too_large(alice.send_text(BOB, &"a".repeat(PROBE_LEN)));
    assert_eq!((what, len), ("text", PROBE_LEN));
    assert_eq!(max_text, D14_MAX_TEXT_BYTES, "plan D14");
    assert_eq!(
        too_large(alice.send_text(BOB, &"a".repeat(max_text + 1))),
        ("text", max_text + 1, max_text)
    );
    let longest = "a".repeat(max_text);
    let msg_id = alice
        .send_text(BOB, &longest)
        .expect("limit boyundaki metin");
    bob_events.wait("bob: en uzun TEXT", |event| {
        matches!(
            event,
            NodeEvent::TextReceived { peer_id, msg_id: got, text }
                if peer_id == ALICE && *got == msg_id && *text == longest
        )
        .then_some(())
    });

    // PTT yükü: denetim tutamaçtan ÖNCE yapılır (bilinmeyen tutamaçla da `TooLarge`).
    let unknown = StreamHandle::from_raw(u64::MAX);
    let (what, len, max_bundle) = too_large(alice.ptt_push(unknown, vec![0; PROBE_LEN]));
    assert_eq!((what, len), ("ptt payload", PROBE_LEN));
    assert!(matches!(
        alice.ptt_push(unknown, vec![0; max_bundle]),
        Err(NodeError::UnknownHandle)
    ));
    let stream = alice
        .ptt_open(BOB, PttCodec::Lyra, BUNDLE_MS)
        .expect("ptt_open");
    assert_eq!(
        too_large(alice.ptt_push(stream, vec![0; max_bundle + 1])),
        ("ptt payload", max_bundle + 1, max_bundle)
    );
    let largest: Vec<u8> = (0..max_bundle)
        .map(|i| u8::try_from(i % usize::from(u8::MAX)).expect("bir bayt"))
        .collect();
    alice
        .ptt_push(stream, largest.clone())
        .expect("limit boyundaki demet");
    bob_events.wait("bob: en büyük demet", |event| {
        matches!(
            event,
            NodeEvent::PttBundle { peer_id, payload, .. } if peer_id == ALICE && *payload == largest
        )
        .then_some(())
    });
    alice.ptt_close(stream).expect("ptt_close");
    alice.shutdown().expect("alice kapanış");
    bob.shutdown().expect("bob kapanış");
}

/// `shutdown` kuyruğun SONUNA `Shutdown` koyar: kendinden önce kuyruğa girmiş yanıtsız komutlar
/// (`send_file`) işlenir ve olaylarını doğurur; dönüşte düğüm kapanmıştır.
#[test]
fn shutdown_processes_the_commands_queued_before_it() {
    let root = TempDir::new("shutdown-queue");
    let _serial = common::serial();
    let provisioned = provision(root.path(), &[ALICE, BOB], true);
    let hub = MemoryHub::new();
    let (alice, mut events) = start(&provisioned.devices[ALICE], &hub);

    // Oturum yok: her komut düğümde `NoSession` olayına dönüşür (dosya okunmaz).
    let files: Vec<FileHandle> = (0..QUEUED_BEFORE_SHUTDOWN)
        .map(|_| {
            alice
                .send_file(BOB, Path::new("okunmaz.bin"))
                .expect("kuyruğa")
        })
        .collect();
    alice.shutdown().expect("kapanış");
    for file in files {
        assert_eq!(
            wait_file_failed(&mut events, file),
            (
                BOB.to_owned(),
                FileFailReason::NotQueued(FileSendError::NoSession)
            ),
            "kapanıştan önce kuyruğa giren komut işlendi"
        );
    }
}
