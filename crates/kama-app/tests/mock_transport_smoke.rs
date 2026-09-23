//! B1.3 smoke test: `MockTransport`'un kendisi kullanılabilir mi. Sözleşmenin tam kanıtı
//! (H-* testleri) B1.4'te `conf_s1_*.rs` dosyalarında.

mod common;

use common::mock_transport::MockTransport;
use kama_app::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PeerRef, PreparedUnit,
    SessionEvent, TransportCapabilities,
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
    let peer = PeerRef::from_bytes(b"bob".to_vec());

    let ConnectResult::Pending(_ticket) = transport.connect(peer.clone(), None).unwrap() else {
        panic!("expected Pending on first connect to an unknown peer");
    };

    let session = transport.complete_connect(&peer);
    assert_eq!(
        transport.poll_event(),
        Some(SessionEvent::Ready(session, peer.clone()))
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
