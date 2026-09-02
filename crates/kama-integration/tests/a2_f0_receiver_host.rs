//! A2-F0 kabul-pinleri — ReceiverHost iskeleti (DR-CS-1b/A2; D-1/K-8/K-9).

#![cfg(feature = "experimental-cold-start")]

use kama_integration::{HostError, ReceiverHost};

/// Pin-1: composition-root kurulur; K-9 global-defter-INSTANCE'ı host'ta ve boş başlar.
#[test]
fn a2_f0_host_constructs_with_owned_global_ledger() {
    let host = ReceiverHost::new();
    assert_eq!(host.session_count(), 0);
    assert_eq!(host.global_cold().res.retained_bytes, 0);
    assert_eq!(host.global_cold().res.session_count, 0);
    assert_eq!(host.tombstone_registry_count(), 0);
}

/// Pin-2 (F2-evrimi): session-siz ingest → unknown-handle drop (typed Ok; state değişmez; panic yok).
#[test]
fn a2_f2_ingest_unknown_handle_drop_no_mutation() {
    let mut host = ReceiverHost::new();
    assert_eq!(
        host.on_datagram(0, &[0u8; 8]),
        Ok(kama_integration::IngestOutcome::Dropped("unknown handle"))
    );
    assert_eq!(host.session_count(), 0);
    assert_eq!(host.global_cold().res.retained_bytes, 0);
}

/// Pin-3 (F1-B-evrimi): bilinmeyen-session kapatma → typed UnknownSession; state değişmez.
#[test]
fn a2_f1b_close_unknown_session_typed() {
    let mut host = ReceiverHost::new();
    let r = host.close_session(0xDEAD_BEEF, |_s| {
        Ok(kama_integration::CachedCloseAck {
            grace_deadline_ns: 0,
        })
    });
    assert_eq!(r, Err(HostError::UnknownSession { h_ext: 0xDEAD_BEEF }));
    assert_eq!(host.tombstone_registry_count(), 0);
}
