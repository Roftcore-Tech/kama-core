//! Sprint-4a: session-state + cross-session-admission (§3/§2.55/§5).
//! session-demux h_ext-routing · cross-session-reddi (eski-h_ext→UnknownHandle) · closed-session-reddi ·
//! renewal (yeni-h_ext ayrı-session) · anahtar-farkı ikinci-savunma.

use kama_noise_wire::{
    Direction, KamaNoiseWireProtector, SealArgs, Session, SessionId, SessionLifecycle,
    SessionManager, WireError,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}
fn fec(block_id: u32) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id,
            esi: 0,
            k_actual: 1,
        },
        symbol: vec![0x5Au8; 64],
    }
}

/// Bir yön için eşleşen sealer(A→send) + manager(B→recv) session çifti kur.
fn make_pair(h_ext: u32, sid: u64, s: u8, r: u8) -> (Session, Session) {
    let a = Session::new(
        SessionId(sid),
        Direction::Initiator,
        KamaNoiseWireProtector::new(h_ext, &key(s), &key(r)),
    );
    let b = Session::new(
        SessionId(sid),
        Direction::Responder,
        KamaNoiseWireProtector::new(h_ext, &key(r), &key(s)),
    );
    (a, b)
}

// ── Temel: doğru-session seal→open (demux h_ext-routing) ──
#[test]
fn session_demux_routes_by_handle() {
    let (mut a, b) = make_pair(0x1111_1111, 1, 0xAA, 0xBB);
    let mut mgr = SessionManager::new();
    mgr.install(b);
    let sealed = a
        .seal(
            &fec(7),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    let (sid, opened) = mgr.open(&sealed.bytes).unwrap();
    assert_eq!(sid, SessionId(1));
    assert_eq!(opened.h_ext(), 0x1111_1111);
}

// ── CROSS-SESSION REDDİ: eski-h_ext yeni-session-manager'da yok → UnknownHandle ──
#[test]
fn cross_session_old_handle_rejected() {
    let (mut a_old, _b_old) = make_pair(0xAAAA_0000, 1, 0xAA, 0xBB);
    // Eski-session'dan datagram üret.
    let old_dg = a_old
        .seal(
            &fec(1),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();

    // Yeni-session YENİ-h_ext alır (§2.55 RESTART); manager yalnız yeni-session'ı tanır.
    let (_a_new, b_new) = make_pair(0xBBBB_0000, 2, 0xCC, 0xDD);
    let mut mgr = SessionManager::new();
    mgr.install(b_new);

    // Eski-h_ext datagramı → UnknownHandle (cross-session admission-alamaz).
    assert!(matches!(
        mgr.open(&old_dg.bytes),
        Err(WireError::UnknownHandle(0xAAAA_0000))
    ));
}

// ── Renewal: eski-session remove edilince eski-datagram UnknownHandle ──
#[test]
fn renewal_removes_old_session() {
    let (mut a, b) = make_pair(0x2222_2222, 5, 0xAA, 0xBB);
    let mut mgr = SessionManager::new();
    mgr.install(b);
    let dg = a
        .seal(
            &fec(1),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    // İlk açılış OK.
    assert!(mgr.open(&dg.bytes).is_ok());
    // Renewal: eski-session kaldırıldı.
    assert!(mgr.remove(0x2222_2222).is_some());
    assert_eq!(mgr.active_count(), 0);
    // Sonraki eski-h_ext datagramı (yeni-PN) → UnknownHandle.
    let dg2 = a
        .seal(
            &fec(2),
            SealArgs {
                wire_pn: 1,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(matches!(
        mgr.open(&dg2.bytes),
        Err(WireError::UnknownHandle(_))
    ));
}

// ── Closed-session (SESSION_CLOSE/cap→renewal): data-admit-ETMEZ ──
#[test]
fn closed_session_rejects_data() {
    let (mut a, mut b) = make_pair(0x3333_3333, 9, 0xAA, 0xBB);
    b.close();
    assert_eq!(b.lifecycle(), SessionLifecycle::Closed);
    let mut mgr = SessionManager::new();
    mgr.install(b);
    let dg = a
        .seal(
            &fec(1),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert!(matches!(
        mgr.open(&dg.bytes),
        Err(WireError::SessionClosed(0x3333_3333))
    ));

    // Closed-session seal de reddedilir.
    assert!(a
        .seal(
            &fec(2),
            SealArgs {
                wire_pn: 1,
                dh_epoch: 0,
                send_generation: 0
            }
        )
        .is_ok()); // a hâlâ Active
    let mut a_closed = a;
    a_closed.close();
    assert!(matches!(
        a_closed.seal(
            &fec(3),
            SealArgs {
                wire_pn: 2,
                dh_epoch: 0,
                send_generation: 0
            }
        ),
        Err(WireError::SessionClosed(_))
    ));
}

// ── İkinci-savunma: h_ext-collision olsa bile anahtar-farkı → AEAD-fail ──
#[test]
fn key_mismatch_second_defense() {
    // İki session AYNI h_ext ama FARKLI anahtar (varsayımsal-collision senaryosu).
    let (mut a, _b) = make_pair(0x4444_4444, 1, 0xAA, 0xBB);
    // Manager'a FARKLI-anahtarlı ama aynı-h_ext'li session kur.
    let b_wrong = Session::new(
        SessionId(2),
        Direction::Responder,
        KamaNoiseWireProtector::new(0x4444_4444, &key(0x99), &key(0x88)),
    );
    let mut mgr = SessionManager::new();
    mgr.install(b_wrong);
    let dg = a
        .seal(
            &fec(1),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    // h_ext eşleşir ama anahtar farklı → AEAD-fail.
    assert!(matches!(mgr.open(&dg.bytes), Err(WireError::AeadFail)));
}

// ── Çok-session bir-arada (migration/renewal geçişi): her-handle kendi-session'ına ──
#[test]
fn multiple_sessions_coexist() {
    let (mut a1, b1) = make_pair(0x0000_0001, 1, 0x11, 0x22);
    let (mut a2, b2) = make_pair(0x0000_0002, 2, 0x33, 0x44);
    let mut mgr = SessionManager::new();
    mgr.install(b1);
    mgr.install(b2);
    assert_eq!(mgr.active_count(), 2);
    let d1 = a1
        .seal(
            &fec(1),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    let d2 = a2
        .seal(
            &fec(2),
            SealArgs {
                wire_pn: 0,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap();
    assert_eq!(mgr.open(&d1.bytes).unwrap().0, SessionId(1));
    assert_eq!(mgr.open(&d2.bytes).unwrap().0, SessionId(2));
}
