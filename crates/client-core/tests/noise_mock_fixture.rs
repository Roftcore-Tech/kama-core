//! **mock-fixture** (S3 / A4.2) — sahte Crypto, Transport'u yanıltmıyor mu.
//!
//! `MockCrypto` `kama-noise-wire`'ın `test-util` özelliğindedir (sahte, gerçeğinin sahibi olan
//! crate'te yaşar); tüketicisi Transport olduğu için fixture testleri burada, tüketici tarafında
//! durur — f9 gerçek Transport runtime'ını sahte Crypto ile uçtan uca koşturur.
//!
//! Sahtenin sözleşmeye UYDUĞU conformance-core'da ölçülür (A4.4). Burada ölçülen ayrı bir şeydir:
//! `MockCrypto`'nun kendi iddialarına sadık kalması ve gerçek seam'in Transport'a gösterdiği
//! davranışı (ardışık PN, sessiz `open`, replay reddi, cap'li demux) DOĞRU taklit etmesi.

use std::time::Duration;

use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint};
use client_core::seam::{
    CloseReason, PacketProtection, PeerRef, ProtectionClass, ProtectionContext, SealError,
    SessionCommand, SessionControl, SessionEvent, SessionHandle, SessionResult,
};
use kama_noise_wire::seam_mock::{
    MockCrypto, ENVELOPE_INIT, ENVELOPE_INIT_RESPONSE, MOCK_MAX_SESSIONS,
    PENDING_RESPONSE_TRIAL_CAP,
};

fn s(v: u64) -> ProtectionContext {
    ProtectionContext::Session(SessionHandle::from_raw(v))
}

/// Test eşi: `dest_hash`'in ilk sekiz baytı `n` (big-endian). Sahte, oturum tutamacını bu sekiz
/// bayttan türetir (`MockCrypto::session_of`), yani `peer(n)` ile kurulan oturum `s(n)`'dir.
fn peer(n: u64) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[..8].copy_from_slice(&n.to_be_bytes());
    PeerRef::from_dest_hash(dest_hash)
}

/// İki uç: A `Initiate` ile INIT zarfını üretir, B aynı tutamacı o zarftan `AcceptInit` ile öğrenir.
fn pair(session: u64) -> (MockCrypto, MockCrypto) {
    let (mut a, mut b) = (MockCrypto::new(), MockCrypto::new());
    let SessionResult::OutboundEnvelope {
        envelope_type,
        bytes,
    } = a.command(SessionCommand::Initiate {
        peer: peer(session),
        early_unit: None,
    })
    else {
        panic!("Initiate bir INIT zarfı üretmeli");
    };
    assert_eq!(envelope_type, ENVELOPE_INIT);
    assert_eq!(bytes, MockCrypto::init_envelope(session, 0));
    assert!(matches!(
        b.command(SessionCommand::AcceptInit { wire: bytes }),
        SessionResult::EstablishedAndReply { session: h, envelope_type: ENVELOPE_INIT_RESPONSE, .. }
            if h == SessionHandle::from_raw(session)
    ));
    (a, b)
}

/// K2: PN'i sahte sayar, bağlam başına ardışık; K1: `Session` ve `HubChannel` ayrı uzay.
#[test]
fn f1_pn_is_consecutive_per_context_and_owned_by_the_mock() {
    let (mut a, _b) = pair(7);
    let pns: Vec<u64> = (0..4)
        .map(|_| a.seal(s(7), b"x").unwrap().meta.full_wire_pn)
        .collect();
    assert_eq!(pns, [0, 1, 2, 3]);
    assert_eq!(
        a.seal(ProtectionContext::HubChannel, b"x")
            .unwrap()
            .meta
            .full_wire_pn,
        0,
        "HubChannel ayrı PN uzayı"
    );
    assert_eq!(a.seal(s(7), b"x").unwrap().meta.full_wire_pn, 4);
}

/// `sealed_len` KESİN; `limits` ile tutarlı.
#[test]
fn f2_sealed_len_is_exact() {
    let (mut a, _b) = pair(1);
    for n in [0usize, 1, 37, 1013] {
        let frame = vec![0xA5u8; n];
        let u = a.seal(s(1), &frame).unwrap();
        assert_eq!(u.bytes.len(), a.sealed_len(s(1), n).unwrap());
        assert_eq!(u.meta.sealed_len as usize, u.bytes.len());
        let overhead = a.limits(s(1)).wire_overhead_bytes as usize;
        assert_eq!(u.bytes.len(), n + overhead);
    }
    assert_eq!(
        a.conservative_limits(ProtectionClass::Session),
        a.limits(s(1))
    );
}

/// Roundtrip: byte-özdeş çerçeve + doğru bağlam + mühürdeki PN alımda da görünür.
#[test]
fn f3_roundtrip_identifies_context_and_pn() {
    let (mut a, mut b) = pair(9);
    let u = a.seal(s(9), b"hello").unwrap();
    let o = b.open(&u.bytes).expect("geçerli datagram açılmalı");
    assert_eq!(o.frame, b"hello");
    assert_eq!(o.ctx, s(9));
    assert_eq!(o.meta.full_wire_pn, u.meta.full_wire_pn);
}

/// K5: `open` başarısızlığı sessizdir ve HİÇBİR state'i ilerletmez — bozuk kopya düşer, ardından
/// ASIL datagram hâlâ açılır (bozuk kopya PN'i "yakmış" olsaydı açılamazdı).
#[test]
fn f4_open_failure_is_silent_and_does_not_advance_state() {
    let (mut a, mut b) = pair(3);
    let u = a.seal(s(3), b"payload").unwrap();
    let mut bad = u.bytes.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert!(b.open(&bad).is_none(), "bozuk tag sessizce düşmeli");
    let mut bad2 = u.bytes.clone();
    bad2[20] ^= 1;
    assert!(b.open(&bad2).is_none(), "bozuk gövde sessizce düşmeli");
    assert!(b.open(&[]).is_none());
    assert!(b.open(&u.bytes[..10]).is_none(), "kırpılmış datagram");
    assert_eq!(b.open(&u.bytes).expect("asıl datagram").frame, b"payload");
}

/// Replay reddi + pencere içi sırasız teslim. Kabul eden bir sahte, Transport'un duplicate
/// testlerini anlamsız kılardı.
#[test]
fn f5_replay_is_rejected_and_reordering_within_window_is_accepted() {
    let (mut a, mut b) = pair(5);
    let pkts: Vec<Vec<u8>> = (0u8..4)
        .map(|i| a.seal(s(5), &[i]).unwrap().bytes)
        .collect();
    assert_eq!(b.open(&pkts[2]).unwrap().frame, [2]);
    assert_eq!(
        b.open(&pkts[0]).unwrap().frame,
        [0],
        "geç gelen, pencere içi"
    );
    assert!(b.open(&pkts[2]).is_none(), "duplicate");
    assert!(b.open(&pkts[0]).is_none(), "duplicate (geç gelen)");
    assert_eq!(b.open(&pkts[3]).unwrap().frame, [3]);
    assert_eq!(b.open(&pkts[1]).unwrap().frame, [1]);
}

/// Bilinmeyen oturum: `seal` yapısal hata, `open` sessiz.
#[test]
fn f6_unknown_session_is_typed_on_seal_and_silent_on_open() {
    let (mut a, _b) = pair(1);
    assert_eq!(a.seal(s(2), b"x").unwrap_err(), SealError::UnknownSession);
    assert_eq!(
        a.sealed_len(s(2), 1).unwrap_err(),
        SealError::UnknownSession
    );
    let u = a.seal(s(1), b"x").unwrap();
    let mut stranger = MockCrypto::new();
    assert!(
        stranger.open(&u.bytes).is_none(),
        "oturumu bilmeyen uç açamaz"
    );
    let too_big = vec![0u8; a.limits(s(1)).max_plaintext_bytes as usize + 1];
    assert_eq!(
        a.seal(s(1), &too_big).unwrap_err(),
        SealError::FrameTooLarge
    );
}

/// K7: `AcceptResponse` EN YENİ ÖNCE ve en fazla cap deneme; başarısızlık opak ve iz bırakmaz.
#[test]
fn f7_accept_response_is_newest_first_capped_and_opaque() {
    let mut m = MockCrypto::new();
    for tok in 100..100 + PENDING_RESPONSE_TRIAL_CAP as u64 {
        m.add_pending(tok).unwrap();
    }
    assert_eq!(
        m.add_pending(999),
        Err(SealError::CapReached),
        "pending kümesi trial cap'ten büyük olamaz (ccr §35.15 tutarlılık zinciri)"
    );

    // Sahte RESPONSE: tam cap kadar deneme, sonuç opak, pending DEĞİŞMEZ.
    assert_eq!(
        m.command(SessionCommand::AcceptResponse {
            wire: MockCrypto::response_envelope(1)
        }),
        SessionResult::Failed
    );
    assert_eq!(m.last_response_trials, PENDING_RESPONSE_TRIAL_CAP);
    assert_eq!(m.pending_len(), PENDING_RESPONSE_TRIAL_CAP);
    assert_eq!(
        m.command(SessionCommand::AcceptResponse {
            wire: vec![1, 2, 3]
        }),
        SessionResult::Failed,
        "bozuk zarf da aynı opak sonuç"
    );
    assert!(m.poll_event().is_none(), "başarısızlık olay üretmez");

    // En yeni pending → 1 deneme.
    let newest = 100 + PENDING_RESPONSE_TRIAL_CAP as u64 - 1;
    assert_eq!(
        m.command(SessionCommand::AcceptResponse {
            wire: MockCrypto::response_envelope(newest)
        }),
        SessionResult::Established {
            session: SessionHandle::from_raw(newest)
        }
    );
    assert_eq!(m.last_response_trials, 1);
    // En eski pending → kalan kümenin tamamı kadar deneme; kümenin SONUNDAKİ meşru aday düşmez.
    assert_eq!(
        m.command(SessionCommand::AcceptResponse {
            wire: MockCrypto::response_envelope(100)
        }),
        SessionResult::Established {
            session: SessionHandle::from_raw(100)
        }
    );
    assert_eq!(m.last_response_trials, PENDING_RESPONSE_TRIAL_CAP - 1);
}

/// Olaylar ve sınırlar: `Ready` → `RenewRequired` → `Closed`; tablo sınırsız büyümez.
#[test]
fn f8_events_and_bounded_tables() {
    let (mut a, _b) = pair(4);
    let h = SessionHandle::from_raw(4);
    assert_eq!(a.poll_event(), Some(SessionEvent::Ready(h)));
    assert_eq!(
        a.command(SessionCommand::Renew { session: h }),
        SessionResult::Established { session: h }
    );
    assert_eq!(a.poll_event(), Some(SessionEvent::RenewRequired(h)));
    assert_eq!(
        a.command(SessionCommand::Close { session: h }),
        SessionResult::NoOutput
    );
    assert_eq!(
        a.poll_event(),
        Some(SessionEvent::Closed(h, CloseReason::UserClosed))
    );
    assert_eq!(
        a.seal(s(4), b"x").unwrap_err(),
        SealError::UnknownSession,
        "Closed sonrası tutamaç kullanılamaz (H-HANDLE)"
    );

    let mut m = MockCrypto::new();
    for i in 0..MOCK_MAX_SESSIONS as u64 {
        assert!(matches!(
            m.command(SessionCommand::AcceptInit {
                wire: MockCrypto::init_envelope(i, 0)
            }),
            SessionResult::EstablishedAndReply { .. }
        ));
    }
    assert_eq!(
        m.command(SessionCommand::AcceptInit {
            wire: MockCrypto::init_envelope(9_999, 0)
        }),
        SessionResult::Failed,
        "oturum tablosu dolu → yeni oturum reddedilir (sınırsız büyüme yok)"
    );
}

/// 2026-10-06 genişletmesi: INIT zarfı deterministik ve her `Initiate`'te farklı; SSR1 önbellekli;
/// bağlam kaydı ve epoch kancası sayacı etkilemez.
#[test]
fn f10_init_envelope_response_cache_and_hooks() {
    let mut a = MockCrypto::new();
    let init = |a: &mut MockCrypto| match a.command(SessionCommand::Initiate {
        peer: peer(7),
        early_unit: Some(vec![0xEE]),
    }) {
        SessionResult::OutboundEnvelope { bytes, .. } => bytes,
        other => panic!("INIT zarfı bekleniyordu: {other:?}"),
    };
    let (i0, i1) = (init(&mut a), init(&mut a));
    let mut want = MockCrypto::init_envelope(7, 0);
    want.push(0xEE);
    assert_eq!(
        i0, want,
        "INIT zarfı deterministik; early_unit opak olarak eklenir"
    );
    assert_ne!(i0, i1, "ikinci Initiate farklı init_counter taşır");
    assert_eq!(a.pending_len(), 2);

    let mut b = MockCrypto::new();
    let r0 = b.command(SessionCommand::AcceptInit { wire: i0.clone() });
    let fp = b.state_fingerprint();
    assert_eq!(b.command(SessionCommand::AcceptInit { wire: i0 }), r0);
    assert_eq!(
        b.state_fingerprint(),
        fp,
        "duplicate INIT state değiştirmez"
    );
    assert_ne!(b.command(SessionCommand::AcceptInit { wire: i1 }), r0);

    // Kancalar: bağlam kaydı ve epoch; sayaç epoch'ta sıfırlanmaz.
    let h = SessionHandle::from_raw(7);
    assert_eq!(a.last_seal_ctx(), None);
    let u0 = a.seal(s(7), b"x").unwrap();
    assert_eq!(a.last_seal_ctx(), Some(s(7)));
    assert!(a.mock_set_epoch(h, 1));
    assert!(!a.mock_set_epoch(SessionHandle::from_raw(8), 1));
    let u1 = a.seal(s(7), b"x").unwrap();
    assert_eq!((u0.meta.dh_epoch, u1.meta.dh_epoch), (0, 1));
    assert_eq!(u1.meta.full_wire_pn, u0.meta.full_wire_pn + 1);
}

/// Asıl kanıt: gerçek Transport runtime'ı (UDP, ARQ, ACK) sahte Crypto ile uçtan uca çalışır.
/// Sahte yanlış olsaydı (PN, sealed_len, bağlam demux'u) teslim tamamlanmazdı.
#[tokio::test(flavor = "multi_thread")]
async fn f9_real_transport_runtime_delivers_over_mock_crypto() {
    const N: u64 = 40;
    // Runtime'ın PoC bağlamı tek örtük oturumdur (tutamaç 0); iki uç da onu kurar.
    let (client_seam, server_seam) = pair(0);
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        RuntimeConfig::default(),
        Box::new(server_seam),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        RuntimeConfig::default(),
        Box::new(client_seam),
    )
    .await
    .expect("client connect");

    for i in 0..N {
        client
            .send(format!("mock-unit-{i}").into_bytes())
            .expect("offer");
    }
    let all = tokio::time::timeout(Duration::from_secs(30), async {
        let (mut delivered, mut acked) = (0u64, 0u64);
        while delivered < N || acked < N {
            tokio::select! {
                Some(d) = server.next_delivery(), if delivered < N => {
                    assert_eq!(d.payload, format!("mock-unit-{}", d.unit_id).into_bytes());
                    delivered += 1;
                }
                Some(_) = client.acked(), if acked < N => { acked += 1; }
            }
        }
        (delivered, acked)
    })
    .await
    .expect("30 s içinde tamamlanmalı");
    assert_eq!(all, (N, N));
    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    assert_eq!(cs.open_failures, 0, "sahte, geçerli datagramı reddetmemeli");
    assert_eq!(ss.open_failures, 0);
    assert_eq!(ss.delivered, N);
}
