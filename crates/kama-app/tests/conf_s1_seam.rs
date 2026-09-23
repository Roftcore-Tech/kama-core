//! S1 conformance-core (B1.4) — KM-SEAM-AT-001.md §6 harness, `MockTransport`'a karşı.
//! Her test dosyanın H-<AD> satırını birebir kanıtlıyor.

mod common;

use common::mock_transport::MockTransport;
use kama_app::{
    CloseReason, CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PeerRef,
    PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};

fn caps() -> TransportCapabilities {
    TransportCapabilities {
        max_plaintext_unit_bytes: 64,
        preferred_plaintext_unit_bytes: 32,
    }
}

fn unit(bytes: Vec<u8>) -> PreparedUnit {
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

/// H-ROUNDTRIP: `send(PreparedUnit)` → `recv()` byte-identical; `early_unit` byte-identical.
#[test]
fn h_roundtrip() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());

    // early_unit: yeni handshake'e eşlik eden opak veri, Ready ile birlikte teslim edilir.
    let early = unit(vec![9, 9, 9]);
    let ConnectResult::Pending(_) = t.connect(peer.clone(), Some(early.clone())).unwrap() else {
        panic!("expected Pending on first connect");
    };
    let session = t.complete_connect(&peer);
    assert_eq!(
        t.poll_event(),
        Some(SessionEvent::Ready(session, peer.clone()))
    );
    let delivered_early = t.recv().expect("early_unit should be delivered on Ready");
    assert_eq!(delivered_early.unit, early.bytes);

    let payload = unit(vec![1, 2, 3, 4]);
    t.send(session, payload.clone(), DeliveryPolicy::Reliable)
        .unwrap();
    let delivered = t.recv().expect("payload should be delivered");
    assert_eq!(delivered.unit, payload.bytes);
}

/// H-EVENTS: `Ready → RenewRequired → Ready(h') → Closed(h, Renewed)` sırası;
/// `Closed` + `close(h)` sonrası `send(h)` → `UnknownSession`.
#[test]
fn h_events() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());

    let ConnectResult::Pending(_) = t.connect(peer.clone(), None).unwrap() else {
        panic!("expected Pending");
    };
    let h = t.complete_connect(&peer);
    assert_eq!(t.poll_event(), Some(SessionEvent::Ready(h, peer.clone())));

    t.request_renew(h);
    assert_eq!(t.poll_event(), Some(SessionEvent::RenewRequired(h)));

    let h2 = t.complete_renew(h);
    assert_eq!(t.poll_event(), Some(SessionEvent::Ready(h2, peer.clone())));
    assert_eq!(
        t.poll_event(),
        Some(SessionEvent::Closed(h, CloseReason::Renewed))
    );
    assert_eq!(t.poll_event(), None);

    // Uygulama Closed'ı işleyip eski handle'ı kapatır (kaynak serbest bırakma).
    t.close(h);
    let err = t
        .send(h, unit(vec![1]), DeliveryPolicy::Reliable)
        .unwrap_err();
    assert_eq!(err, TransportError::UnknownSession);
}

/// H-CUTOVER: cutover öncesi `send(h)` kabul; cutover sonrası `send(h)` → `SessionSuperseded`;
/// `h'` üzerinden gönderim çalışmaya devam eder.
#[test]
fn h_cutover() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());

    let ConnectResult::Pending(_) = t.connect(peer.clone(), None).unwrap() else {
        panic!("expected Pending");
    };
    let h = t.complete_connect(&peer);
    t.poll_event(); // Ready(h)

    // Cutover öncesi: h hâlâ kabul eder.
    t.send(h, unit(vec![1]), DeliveryPolicy::Reliable).unwrap();
    t.recv();

    t.request_renew(h);
    t.poll_event(); // RenewRequired(h)
    let h2 = t.complete_renew(h);
    t.poll_event(); // Ready(h2)
    t.poll_event(); // Closed(h, Renewed)

    // Cutover sonrası: h reddedilir, h2 kabul eder.
    let err = t
        .send(h, unit(vec![2]), DeliveryPolicy::Reliable)
        .unwrap_err();
    assert_eq!(err, TransportError::SessionSuperseded);

    t.send(h2, unit(vec![3]), DeliveryPolicy::Reliable).unwrap();
    let delivered = t.recv().expect("h2 üzerinden gönderim teslim edilmeli");
    assert_eq!(delivered.session, h2);
    assert_eq!(delivered.unit, vec![3]);
}

/// H-CONNECT-IDEMP: aktif peer'a ikinci `connect` → aynı handle, YENİ olay yok; pending peer'a
/// ikinci `connect` → aynı ticket.
#[test]
fn h_connect_idemp() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());

    let ConnectResult::Pending(ticket1) = t.connect(peer.clone(), None).unwrap() else {
        panic!("expected Pending");
    };
    let ConnectResult::Pending(ticket2) = t.connect(peer.clone(), None).unwrap() else {
        panic!("expected Pending (idempotent)");
    };
    assert_eq!(ticket1, ticket2);

    let h = t.complete_connect(&peer);
    t.poll_event(); // Ready(h) tüketildi

    let ConnectResult::Ready(h_again) = t.connect(peer.clone(), None).unwrap() else {
        panic!("expected Ready for an already-active peer");
    };
    assert_eq!(h, h_again);
    assert_eq!(
        t.poll_event(),
        None,
        "idempotent connect yeni olay üretmemeli"
    );
}

/// H-EARLY-CONFLICT: aktif/pending peer'a `connect(peer, Some(X))` → `EarlyUnitConflict`;
/// X hiçbir yola girmez (recv'de byte izi yok).
#[test]
fn h_early_conflict() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());

    // Pending durumunda çakışma.
    t.connect(peer.clone(), None).unwrap();
    let err = t.connect(peer.clone(), Some(unit(vec![0xEE]))).unwrap_err();
    assert_eq!(err, TransportError::EarlyUnitConflict);
    assert_eq!(
        t.recv(),
        None,
        "reddedilen early_unit hiçbir yola girmemeli"
    );

    // Aktif durumunda çakışma.
    let h = t.complete_connect(&peer);
    t.poll_event();
    let err = t.connect(peer.clone(), Some(unit(vec![0xEE]))).unwrap_err();
    assert_eq!(err, TransportError::EarlyUnitConflict);
    assert_eq!(t.recv(), None);

    // early_unit olmadan idempotent davranış aynen sürer.
    let ConnectResult::Ready(h_again) = t.connect(peer, None).unwrap() else {
        panic!("expected Ready");
    };
    assert_eq!(h, h_again);
}

/// H-CAPS: `len == max` kabul, `len == max+1` → `UnitTooLarge`; sınır Transport'tan okunur.
#[test]
fn h_caps() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());
    t.connect(peer.clone(), None).unwrap();
    let h = t.complete_connect(&peer);
    t.poll_event();

    let max = t.capabilities().max_plaintext_unit_bytes as usize;
    t.send(h, unit(vec![0u8; max]), DeliveryPolicy::Reliable)
        .expect("tam sınırda kabul edilmeli");
    t.recv();

    let err = t
        .send(h, unit(vec![0u8; max + 1]), DeliveryPolicy::Reliable)
        .unwrap_err();
    assert_eq!(err, TransportError::UnitTooLarge);
}

/// H-OPAQUE-FAIL: farklı başarısızlık sebepleri (yanlış peer material, bozuk RESPONSE, Plane 1
/// timeout benzeri senaryolar) dışarıdan ayırt edilemez — hepsi aynı tip `ConnectFailed(ticket)`.
/// `ConnectFailed` yapısal olarak yalnız opak ticket taşır (B1.0 K9), bu test onu tüketici
/// tarafında da kanıtlar: sebep bilgisine erişecek bir alan YOK.
#[test]
fn h_opaque_fail() {
    let mut t = MockTransport::new(caps());
    let peer_a = PeerRef::from_bytes(b"alice".to_vec());
    let peer_b = PeerRef::from_bytes(b"carol".to_vec());

    let ConnectResult::Pending(ticket_a) = t.connect(peer_a.clone(), None).unwrap() else {
        panic!("expected Pending");
    };
    t.fail_connect(&peer_a);
    assert_eq!(t.poll_event(), Some(SessionEvent::ConnectFailed(ticket_a)));

    let ConnectResult::Pending(ticket_b) = t.connect(peer_b.clone(), None).unwrap() else {
        panic!("expected Pending");
    };
    t.fail_connect(&peer_b);
    assert_eq!(t.poll_event(), Some(SessionEvent::ConnectFailed(ticket_b)));

    // Peer artık ne aktif ne pending — yeniden connect temiz bir handshake başlatır.
    let ConnectResult::Pending(_) = t.connect(peer_a, None).unwrap() else {
        panic!("başarısız connect sonrası peer tamamen boşalmalı");
    };
}

/// H-COMPRESS-BOUNDARY: `CompressionPolicy` Transport davranışını değiştirmez; `Delivered.unit`
/// olduğu gibi geri gelir (decompress yalnız Application'da, AMP §5).
#[test]
fn h_compress_boundary() {
    let mut t = MockTransport::new(caps());
    let peer = PeerRef::from_bytes(b"bob".to_vec());
    t.connect(peer.clone(), None).unwrap();
    let h = t.complete_connect(&peer);
    t.poll_event();

    let never = PreparedUnit {
        bytes: vec![1, 2, 3],
        compression_policy: CompressionPolicy::Never,
    };
    let compressed = PreparedUnit {
        bytes: vec![1, 2, 3],
        compression_policy: CompressionPolicy::AppCompressed,
    };

    t.send(h, never.clone(), DeliveryPolicy::Reliable).unwrap();
    let d1 = t.recv().unwrap();
    t.send(h, compressed.clone(), DeliveryPolicy::Reliable)
        .unwrap();
    let d2 = t.recv().unwrap();

    // Aynı bytes, farklı CompressionPolicy → Transport'tan aynı şekilde geri gelir.
    assert_eq!(d1.unit, never.bytes);
    assert_eq!(d2.unit, compressed.bytes);
    assert_eq!(d1.unit, d2.unit);
}
