//! A2-F0 kabul-pinleri — ReceiverHost iskeleti (DR-CS-1b/A2; D-1/K-8/K-9).

#![cfg(feature = "experimental-cold-start")]

use kama_integration::{HostError, ReceiverHost};

/// Pin-1: composition-root kurulur; K-9 global-defter-INSTANCE'ı host'ta ve boş başlar.
#[test]
fn a2_f0_host_constructs_with_owned_global_ledger() {
    let host = ReceiverHost::new();
    assert_eq!(host.session_count(), 0);
    assert_eq!(host.global_cold().retained_bytes, 0);
    assert_eq!(host.global_cold().session_count, 0);
    assert_eq!(host.global_cold().tombstone_count, 0);
}

/// Pin-2: F0-stub'ları TYPED-error döner (reachable todo!() yok); state değişmez.
#[test]
fn a2_f0_host_stubs_return_typed_errors() {
    let mut host = ReceiverHost::new();
    assert_eq!(
        host.on_datagram(0, &[0u8; 8]),
        Err(HostError::FeatureNotImplemented("a2_f1_ingest"))
    );
    assert_eq!(
        host.close_session(1, 0),
        Err(HostError::FeatureNotImplemented("a2_f1_close_chain"))
    );
    assert_eq!(host.session_count(), 0);
    assert_eq!(host.global_cold().retained_bytes, 0);
}
