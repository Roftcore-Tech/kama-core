//! B1.3 smoke test — **ported from feature/app (Hayri)**: `MockTransport` Application crate'inden
//! kullanılabiliyor mu. Sözleşmenin tam kanıtı (S1 §6 H-* satırları) hedef-agnostik süittedir:
//! `crates/kama-integration/tests/seam_conformance_core` — AYNI sahte dosyasına karşı koşar.
//!
//! Olay beklentileri S1 F0.2 (2026-10-06) ile hizalı: `Pending` döndüğü anda
//! `ConnectProgress(ticket, Measuring)` (ii); `close(h)` → tam bir `Closed(h, UserClosed)` (iii).

mod common;

use common::mock_transport::MockTransport;
use kama_contracts::handles::{CloseReason, PeerRef};
use kama_contracts::s1::{
    CompressionPolicy, ConnectPhase, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit,
    SessionEvent, TransportCapabilities, TransportError,
};

/// Smoke testin yer tutucu kapasiteleri — normatif değil (gerçek değerler K5 zincirinden).
const SMOKE_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
const SMOKE_PREFERRED_PLAINTEXT_UNIT_BYTES: u32 = 512;
const SMOKE_MAX_EARLY_UNIT_BYTES: u32 = 256;
const SMOKE_RELIABLE: bool = true;

fn caps() -> TransportCapabilities {
    TransportCapabilities {
        max_plaintext_unit_bytes: SMOKE_MAX_PLAINTEXT_UNIT_BYTES,
        preferred_plaintext_unit_bytes: SMOKE_PREFERRED_PLAINTEXT_UNIT_BYTES,
        max_early_unit_bytes: SMOKE_MAX_EARLY_UNIT_BYTES,
        reliable: SMOKE_RELIABLE,
    }
}

#[test]
fn connect_completes_and_roundtrips() {
    let mut transport = MockTransport::new(caps());
    let peer = PeerRef::from_dest_hash([0xB0; 32]);

    let ConnectResult::Pending(ticket) = transport.connect(peer, None).unwrap() else {
        panic!("expected Pending on first connect to an unknown peer");
    };
    // F0.2(ii): Pending döndüğü anda ilk olay Measuring.
    assert_eq!(
        transport.poll_event(),
        Some(SessionEvent::ConnectProgress(
            ticket,
            ConnectPhase::Measuring
        ))
    );
    assert_eq!(transport.poll_event(), None);

    let session = transport.complete_connect(&peer);
    assert_eq!(
        transport.poll_event(),
        Some(SessionEvent::Ready(session, peer))
    );
    assert_eq!(transport.poll_event(), None);

    let unit = PreparedUnit {
        bytes: vec![1, 2, 3],
        compression_policy: CompressionPolicy::Never,
    };
    transport
        .send(session, unit.clone(), DeliveryPolicy::Reliable)
        .unwrap();

    let delivered = transport.recv().expect("unit should be delivered");
    assert_eq!(delivered.session, session);
    assert_eq!(delivered.unit, unit.bytes);

    // F0.2(iii): close(h) → tam bir Closed(h, UserClosed); sonra tutamaç bilinmez; ikinci close
    // olay üretmez.
    transport.close(session);
    assert_eq!(
        transport.poll_event(),
        Some(SessionEvent::Closed(session, CloseReason::UserClosed))
    );
    assert_eq!(transport.poll_event(), None);
    assert_eq!(
        transport.send(session, unit, DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession)
    );
    transport.close(session);
    assert_eq!(transport.poll_event(), None);
}
