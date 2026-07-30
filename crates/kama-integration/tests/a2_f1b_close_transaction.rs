//! A2-F1-B — İKİ-AŞAMALI CLOSE-TRANSACTION KABUL-PİNLERİ (Abkarada DUR-B düzeltmesi):
//! reserve-before-release · registry-full-no-mutation · precompute-failure-rollback ·
//! grace-expiry-full-cleanup · double-close/idempotency · reservation-token sızıntı-yok.

#![cfg(feature = "experimental-cold-start")]

use kama_integration::{CachedCloseAck, HostError, ReceiverHost};

fn ack(deadline: u64) -> CachedCloseAck {
    CachedCloseAck {
        grace_deadline_ns: deadline,
    }
}

/// Mutlu-yol: PHASE-A(reserve+precompute) → PHASE-B(release+commit+tombstone). Sayaç-geçişleri doğru.
#[test]
fn close_happy_path_reserve_before_tombstone() {
    let mut host = ReceiverHost::new();
    host.insert_active_session_for_test(0x0000_0001).unwrap();
    assert_eq!(host.session_count(), 1);
    assert_eq!(host.tombstone_count(), 0);

    host.close_session(0x0000_0001, || Ok(ack(1000))).unwrap();

    // Active→Tombstone ÇEVRİMİ: iki ayrı eksen (active-sayaç çıkar, tombstone-sayaç girer).
    assert_eq!(host.session_count(), 0, "Active-entry düşmedi");
    assert_eq!(host.tombstone_count(), 1, "Tombstone kurulmadı");
    assert_eq!(host.global_cold().res.tombstone_count, 1);
    assert_eq!(
        host.global_cold().res.session_count,
        0,
        "active-session sayacı çevrimde düşmeli (ayrı eksen)"
    );
}

/// REGISTRY-FULL → NO-MUTATION: 64 tombstone dolu; 65. close-denemesi Active/cold state'e DOKUNMAZ.
#[test]
fn close_registry_full_no_destructive_mutation() {
    let mut host = ReceiverHost::new();
    // 64 slot'u tombstone'a çevir (her biri ayrı session).
    for i in 0..64u32 {
        host.insert_active_session_for_test(i).unwrap();
        host.close_session(i, || Ok(ack(0))).unwrap();
    }
    assert_eq!(host.tombstone_count(), 64);

    // 65. Active session; registry dolu → close FAIL-CLOSED, Active KORUNUR.
    host.insert_active_session_for_test(999).unwrap();
    let before_sessions = host.session_count();
    let before_tomb = host.tombstone_count();
    let r = host.close_session(999, || Ok(ack(0)));
    assert_eq!(r, Err(HostError::TombstoneRegistryFull));
    assert_eq!(
        host.session_count(),
        before_sessions,
        "Active-session destructive-teardown gördü"
    );
    assert_eq!(host.tombstone_count(), before_tomb);
    // 999 hâlâ Active (yeniden-close denenebilir): registry açılınca çalışır.
    host.expire_tombstone(0).unwrap(); // bir slot aç
    host.close_session(999, || Ok(ack(0))).unwrap();
    assert_eq!(host.tombstone_count(), 64);
}

/// PRECOMPUTE-FAILURE → RESERVATION-ROLLBACK: rezerve slot geri verilir; Active-session KORUNUR; teardown YOK.
#[test]
fn close_precompute_failure_rolls_back_reservation() {
    let mut host = ReceiverHost::new();
    host.insert_active_session_for_test(0x55).unwrap();
    let before_tomb = host.global_cold().res.tombstone_count;

    let r = host.close_session(0x55, || Err(HostError::PrecomputeFailed("seal failed")));
    assert!(matches!(r, Err(HostError::PrecomputeFailed(_))));

    // Rollback: rezervasyon geri verildi (sızıntı yok) + Active-session dokunulmadı.
    assert_eq!(
        host.global_cold().res.tombstone_count,
        before_tomb,
        "rezervasyon sızdı"
    );
    assert_eq!(
        host.session_count(),
        1,
        "precompute-fail Active-session'ı yok etti"
    );
    assert_eq!(host.tombstone_count(), 0);
    // Sonraki başarılı close çalışır (state tutarlı):
    host.close_session(0x55, || Ok(ack(0))).unwrap();
    assert_eq!(host.tombstone_count(), 1);
}

/// GRACE-EXPIRY full-cleanup: tombstone düşer + committed registry-slot serbest; sayaç geri döner.
#[test]
fn grace_expiry_full_cleanup() {
    let mut host = ReceiverHost::new();
    host.insert_active_session_for_test(0x77).unwrap();
    host.close_session(0x77, || Ok(ack(500))).unwrap();
    assert_eq!(host.global_cold().res.tombstone_count, 1);

    host.expire_tombstone(0x77).unwrap();
    assert_eq!(host.tombstone_count(), 0);
    assert_eq!(
        host.global_cold().res.tombstone_count,
        0,
        "committed registry-slot serbest kalmadı"
    );
}

/// DOUBLE-CLOSE / idempotency: Active kapandıktan sonra ikinci-close → typed UnknownSession
/// (tombstone yeniden-kapatılamaz; sessiz-no-op DEĞİL).
#[test]
fn double_close_is_typed_not_silent() {
    let mut host = ReceiverHost::new();
    host.insert_active_session_for_test(0x88).unwrap();
    host.close_session(0x88, || Ok(ack(0))).unwrap();
    let r = host.close_session(0x88, || Ok(ack(0)));
    assert_eq!(r, Err(HostError::UnknownSession { h_ext: 0x88 }));
    assert_eq!(host.tombstone_count(), 1, "double-close tombstone'u bozdu");
}

/// DOUBLE-EXPIRY: grace-expiry sonrası ikinci-expiry → typed UnknownTombstone (registry-underflow yok).
#[test]
fn double_expiry_is_typed() {
    let mut host = ReceiverHost::new();
    host.insert_active_session_for_test(0x99).unwrap();
    host.close_session(0x99, || Ok(ack(0))).unwrap();
    host.expire_tombstone(0x99).unwrap();
    let r = host.expire_tombstone(0x99);
    assert_eq!(r, Err(HostError::UnknownTombstone { h_ext: 0x99 }));
    assert_eq!(host.global_cold().res.tombstone_count, 0);
}
