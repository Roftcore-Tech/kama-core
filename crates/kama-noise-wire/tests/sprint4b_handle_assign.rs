//! Sprint-4b: HANDLE_ASSIGN (§2.55) codec + golden + hub-sim sıralama/idempotency/bounded-retry.

use kama_noise_wire::handle_assign::{HandleAssignAck, HANDLE_ASSIGN_ACK_LEN, HANDLE_ASSIGN_V1};
use kama_noise_wire::hub_sim::HANDLE_ASSIGN_MAX_RETRIES;
use kama_noise_wire::{HandleAssign, HubSim, ResponderDelivery, HANDLE_ASSIGN_LEN};

// ── §2.55 golden: exact 25B big-endian layout ──
#[test]
fn handle_assign_golden_bytes() {
    let ha = HandleAssign {
        session_attempt_id: 0x0102_0304_0506_0708,
        init_counter: 0x1112_1314_1516_1718,
        h_legacy: 0xAABB_CCDD,
        h_ext: 0xEEFF_0011,
    };
    let mut want = Vec::new();
    want.push(HANDLE_ASSIGN_V1); // 0x01
    want.extend_from_slice(&0x0102_0304_0506_0708u64.to_be_bytes());
    want.extend_from_slice(&0x1112_1314_1516_1718u64.to_be_bytes());
    want.extend_from_slice(&0xAABB_CCDDu32.to_be_bytes());
    want.extend_from_slice(&0xEEFF_0011u32.to_be_bytes());
    assert_eq!(ha.encode().as_slice(), want.as_slice());
    assert_eq!(want.len(), HANDLE_ASSIGN_LEN);
    assert_eq!(HANDLE_ASSIGN_LEN, 25);
}

#[test]
fn handle_assign_roundtrip() {
    let ha = HandleAssign {
        session_attempt_id: 42,
        init_counter: 7,
        h_legacy: 100,
        h_ext: 101,
    };
    assert_eq!(HandleAssign::decode(&ha.encode()).unwrap(), ha);
    assert_eq!(ha.replay_identity(), (42, 7));
}

#[test]
fn handle_assign_fail_closed() {
    // Kısa buffer
    assert!(HandleAssign::decode(&[0u8; 24]).is_err());
    // Yanlış version
    let mut bad = HandleAssign {
        session_attempt_id: 1,
        init_counter: 1,
        h_legacy: 1,
        h_ext: 1,
    }
    .encode();
    bad[0] = 0x02;
    assert!(HandleAssign::decode(&bad).is_err());
}

#[test]
fn handle_assign_ack_roundtrip() {
    let ack = HandleAssignAck {
        session_attempt_id: 0xDEAD_BEEF_CAFE_0001,
    };
    assert_eq!(ack.encode().len(), HANDLE_ASSIGN_ACK_LEN);
    assert_eq!(HandleAssignAck::decode(&ack.encode()).unwrap(), ack);
    assert!(HandleAssignAck::decode(&[0u8; 7]).is_err());
}

// ── hub-sim SIRALAMA-GARANTİSİ: HANDLE_ASSIGN responder'a extended'den ÖNCE ──
#[test]
fn hub_ordering_handle_before_extended() {
    let mut hub = HubSim::new(1000);
    let assign = hub.admit_init(0xAAA, 1); // session_attempt_id, init_counter
                                           // h_ext teslim edildi → extended forward edilebilir.
    hub.forward_extended(assign.h_ext, vec![0xEE; 10]).unwrap();
    hub.forward_extended(assign.h_ext, vec![0xFF; 10]).unwrap();
    let stream = hub.responder_stream();
    // İlk teslim HANDLE_ASSIGN olmalı.
    assert!(matches!(stream[0], ResponderDelivery::HandleAssign(_)));
    assert!(matches!(stream[1], ResponderDelivery::Extended { .. }));
    assert!(matches!(stream[2], ResponderDelivery::Extended { .. }));
    // HANDLE_ASSIGN'ın h_ext'i ile forward-h_ext eşleşir.
    if let ResponderDelivery::HandleAssign(ha) = &stream[0] {
        assert_eq!(ha.h_ext, assign.h_ext);
    }
}

// ── hub-sim: HANDLE_ASSIGN-teslim-edilmeden extended-forward REDDEDİLİR ──
#[test]
fn hub_ordering_rejects_premature_extended() {
    let mut hub = HubSim::new(2000);
    // admit_init YAPMADAN forward → sıralama-ihlali reddi.
    assert!(hub.forward_extended(9999, vec![0x11; 4]).is_err());
    assert!(hub.responder_stream().is_empty());
}

// ── hub-sim IDEMPOTENCY: aynı-attempt-tekrarı AYNI-HandleAssign (yeni-atama YOK) ──
#[test]
fn hub_admit_idempotent() {
    let mut hub = HubSim::new(3000);
    let a1 = hub.admit_init(0x55, 9);
    let a2 = hub.admit_init(0x55, 9); // aynı (attempt, counter)
    assert_eq!(a1, a2, "idempotent: aynı-içerik");
    // Farklı-attempt → yeni-handle.
    let a3 = hub.admit_init(0x56, 9);
    assert_ne!(a3.h_ext, a1.h_ext);
    // responder-stream'de HANDLE_ASSIGN yalnız-2-kez (idempotent tekrar enqueue-etmez).
    let ha_count = hub
        .responder_stream()
        .iter()
        .filter(|d| matches!(d, ResponderDelivery::HandleAssign(_)))
        .count();
    assert_eq!(ha_count, 2);
}

// ── hub-sim BOUNDED-RETRY: ACK'e-kadar retry, cap'te durur ──
#[test]
fn hub_bounded_retry() {
    let mut hub = HubSim::new(4000);
    let a = hub.admit_init(0x77, 1);
    // ACK gelmeden: cap'e kadar retry.
    let mut count = 0;
    while hub.do_retry(a.session_attempt_id) {
        count += 1;
        assert!(count <= HANDLE_ASSIGN_MAX_RETRIES + 1, "sonsuz-retry-YASAK");
    }
    assert_eq!(count, HANDLE_ASSIGN_MAX_RETRIES);
    assert!(
        !hub.should_retry(a.session_attempt_id),
        "cap sonrası retry-YOK"
    );
}

#[test]
fn hub_ack_stops_retry() {
    let mut hub = HubSim::new(5000);
    let a = hub.admit_init(0x88, 2);
    assert!(hub.should_retry(a.session_attempt_id));
    hub.record_ack(a.session_attempt_id);
    assert!(
        !hub.should_retry(a.session_attempt_id),
        "ACK sonrası retry-durur"
    );
    assert!(!hub.do_retry(a.session_attempt_id));
}
