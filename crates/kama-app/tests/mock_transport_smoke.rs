//! B1.3 smoke test — **ported from feature/app (Hayri)**: `MockTransport` Application crate'inden
//! kullanılabiliyor mu. Sözleşmenin tam kanıtı (S1 §6 H-* satırları) hedef-agnostik süittedir:
//! `crates/kama-integration/tests/seam_conformance_core` — AYNI sahte dosyasına karşı koşar.

mod common;

use common::mock_transport::MockTransport;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent,
    TransportCapabilities,
};

fn caps() -> TransportCapabilities {
    TransportCapabilities {
        max_plaintext_unit_bytes: 1024,
        preferred_plaintext_unit_bytes: 512,
    }
}

#[test]
fn connect_completes_and_roundtrips() {
    let mut transport = MockTransport::new(caps());
    let peer = PeerRef::from_dest_hash([0xB0; 32]);

    let ConnectResult::Pending(_ticket) = transport.connect(peer, None).unwrap() else {
        panic!("expected Pending on first connect to an unknown peer");
    };

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
}
