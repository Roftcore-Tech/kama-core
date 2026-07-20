//! Sprint-4d: INIT-flight (§0.7-B + MADDE-5 + DR-2). cached-byte-for-byte-retransmit + bounded +
//! anti-amplification + dup-INIT-map + farklı-cert-reject.

use kama_noise_wire::{
    AntiAmplification, InitAdmission, InitFlight, InitReceiver, InitReplayIdentity,
    InitRetryConfig, RetransmitOutcome,
};

fn dev(x: u8) -> [u8; 16] {
    [x; 16]
}
fn cert(x: u8) -> [u8; 32] {
    [x; 32]
}

// ── MADDE-5: retransmit AYNEN cached-byte (byte-for-byte; re-seal YOK) ──
#[test]
fn retransmit_is_byte_for_byte_cached() {
    let sealed = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03];
    let mut flight = InitFlight::new(sealed.clone(), 7, InitRetryConfig::alpha_default());
    // Her retransmit AYNI byte'lar (nonce-reuse-guard: cached, re-seal-imkansız).
    for _ in 0..3 {
        match flight.retransmit() {
            RetransmitOutcome::Send { datagram, .. } => assert_eq!(datagram, &sealed[..]),
            other => panic!("Send beklendi: {other:?}"),
        }
    }
    assert_eq!(flight.cached_datagram(), &sealed[..]);
    assert_eq!(flight.init_counter(), 7);
}

// ── Bounded: max_retries sonrası Exhausted (sonsuz-retry-YASAK) ──
#[test]
fn retransmit_bounded_by_count() {
    let cfg = InitRetryConfig {
        max_retries: 3,
        backoff_base_ms: 10,
        backoff_mult: 1,
        max_total_ms: 1_000_000,
    };
    let mut flight = InitFlight::new(vec![1, 2, 3], 1, cfg);
    let mut sends = 0;
    loop {
        match flight.retransmit() {
            RetransmitOutcome::Send { .. } => {
                sends += 1;
                assert!(sends <= 4, "count-cap aşıldı");
            }
            RetransmitOutcome::Exhausted => break,
            RetransmitOutcome::Confirmed => panic!("confirmed değil"),
        }
    }
    assert_eq!(sends, 3);
}

// ── Bounded: toplam-süre üst-sınırı (backoff büyüdükçe max_total'da durur) ──
#[test]
fn retransmit_bounded_by_total_duration() {
    let cfg = InitRetryConfig {
        max_retries: 100,
        backoff_base_ms: 200,
        backoff_mult: 2,
        max_total_ms: 1000, // 200+400+... → 200+400=600, +800>1000 → durur
    };
    let mut flight = InitFlight::new(vec![9], 1, cfg);
    let mut total = 0u64;
    while let RetransmitOutcome::Send { next_delay_ms, .. } = flight.retransmit() {
        total += next_delay_ms;
        assert!(total <= 1000, "toplam-süre-cap aşıldı: {total}");
    }
    // 200 + 400 = 600 (sonraki 800 → 600+800=1400 > 1000 → dur).
    assert_eq!(total, 600);
}

// ── Confirmed retry'yi durdurur (SENDER_CONFIRMED) ──
#[test]
fn confirmation_stops_retry() {
    let mut flight = InitFlight::new(vec![1], 1, InitRetryConfig::alpha_default());
    assert!(matches!(
        flight.retransmit(),
        RetransmitOutcome::Send { .. }
    ));
    flight.on_confirmation();
    assert_eq!(flight.retransmit(), RetransmitOutcome::Confirmed);
}

// ── MADDE-5 alım: ilk-INIT Fresh; duplicate(aynı-cert)→map+resend; app-2.-teslim-YOK ──
#[test]
fn init_admission_dedup() {
    let mut rx = InitReceiver::new();
    let id = InitReplayIdentity {
        sender_device_id: dev(1),
        recipient_device_id: dev(2),
        init_counter: 5,
    };
    assert_eq!(rx.admit(id, cert(0xAA)), InitAdmission::Fresh);
    // Duplicate (aynı-identity + aynı-cert) → yeni-session-YOK, confirmation-resend.
    assert_eq!(
        rx.admit(id, cert(0xAA)),
        InitAdmission::DuplicateResendConfirmation
    );
    assert_eq!(
        rx.admit(id, cert(0xAA)),
        InitAdmission::DuplicateResendConfirmation
    );
    assert_eq!(rx.established_count(), 1, "app/state 2.-kez-kurulmadı");
}

// ── MADDE-5: aynı-identity FARKLI-cert → REJECT ──
#[test]
fn init_same_counter_different_cert_rejected() {
    let mut rx = InitReceiver::new();
    let id = InitReplayIdentity {
        sender_device_id: dev(1),
        recipient_device_id: dev(2),
        init_counter: 9,
    };
    assert_eq!(rx.admit(id, cert(0xAA)), InitAdmission::Fresh);
    // Aynı-(device,counter) ama FARKLI-cert → reject (forged/cert-swap).
    assert_eq!(rx.admit(id, cert(0xBB)), InitAdmission::RejectCertMismatch);
}

// ── Farklı init_counter → ayrı-flight (yeni-attempt) ──
#[test]
fn different_counter_is_fresh() {
    let mut rx = InitReceiver::new();
    let id1 = InitReplayIdentity {
        sender_device_id: dev(1),
        recipient_device_id: dev(2),
        init_counter: 1,
    };
    let id2 = InitReplayIdentity {
        sender_device_id: dev(1),
        recipient_device_id: dev(2),
        init_counter: 2,
    };
    assert_eq!(rx.admit(id1, cert(0xAA)), InitAdmission::Fresh);
    assert_eq!(rx.admit(id2, cert(0xAA)), InitAdmission::Fresh);
    assert_eq!(rx.established_count(), 2);
}

// ── ANTI-AMPLIFICATION: unauthenticated-input sınırsız-response ÜRETMEZ ──
#[test]
fn anti_amplification_bounds_response() {
    let guard = AntiAmplification::new(3); // 3× klasik
                                           // 100B unauthenticated-request → max 300B response.
    assert_eq!(guard.max_response_bytes(100), 300);
    assert!(guard.allows(100, 300));
    assert!(
        !guard.allows(100, 301),
        "3×-üstü response reddedilir (amplification-guard)"
    );
    // Küçük-unauthenticated-input büyük-response-üretemez.
    assert!(
        !guard.allows(10, 1000),
        "10B-girdiden 1000B-response = amplification → YASAK"
    );
}
