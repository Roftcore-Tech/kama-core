//! H-HANDLE (çalışma zamanı yarısı) — kapanmış oturumun tutamacıyla `seal` → `UnknownSession`.
//!
//! Tip yarısı (`HubHandle` ≠ `SessionHandle`) derleme zamanında kanıtlanır: trybuild negatif testi
//! `tests/ui/hub_handle_is_not_session_handle.rs` (bkz. `tests/trybuild.rs`) ve toolchain'den
//! bağımsız karşılığı `kama-contracts` birim testi `hub_handle_does_not_convert_to_session_handle`.
//! Kapsama tablosunda ayrı satırdır ("H-HANDLE/type").

use kama_contracts::handles::{CloseReason, SessionHandle};
use kama_contracts::s3::{
    HubHandle, PacketProtection, SealError, SessionCommand, SessionControl, SessionEvent,
    SessionResult,
};

use crate::s3_support::{established, initiate_session, peer, session, INITIATOR, PEER};
use crate::target::S3Target;

pub fn closed_handle_cannot_seal<T: S3Target>() {
    let mut p = established::<T>();
    let ctx = session(p.h);
    let in_flight =
        p.a.seal(ctx, b"kapanmadan once")
            .expect("açık oturumda seal");

    assert_eq!(
        p.a.command(SessionCommand::Close { session: p.h }),
        SessionResult::NoOutput
    );
    assert!(matches!(
        p.a.poll_event(),
        Some(SessionEvent::Closed(h, _)) if h == p.h
    ));
    assert_eq!(
        p.a.seal(ctx, b"kapandiktan sonra").unwrap_err(),
        SealError::UnknownSession
    );
    assert_eq!(
        p.a.sealed_len(ctx, 1).unwrap_err(),
        SealError::UnknownSession
    );

    // Karşı uç da kapatınca uçuştaki paket sessizce düşer.
    let _ = p.b.command(SessionCommand::Close { session: p.h });
    assert!(p.b.open(&in_flight.bytes).is_none());
}

/// [s3 v1.1, 2026-10-08] Bekleyen oturumun tutamacı `Initiated{session}` ile gelir; `AssignHandle`
/// onu adlandırır (aynı bağın tekrarı idempotent; aynı hub tutamacını başka oturuma bağlamak
/// `Failed` ve state değişmez); `Close` bekleyen oturumu da kapatır (tam bir `Closed`, geç gelen
/// RESPONSE `Failed`; bilinmeyen tutamaçta olay yok, state değişmez).
pub fn pending_close_and_assign_handle<T: S3Target>() {
    let mut a = T::build_as(peer(INITIATOR)).expect("hedef kayıtlı olmalı");
    let mut b = T::build_as(peer(PEER)).expect("hedef kayıtlı olmalı");
    let (h, _, init) = initiate_session::<T>(&mut a, PEER);
    let (other, _, _) = initiate_session::<T>(&mut a, PEER + 1);
    assert_ne!(h, other, "iki bekleyen oturum ayrı tutamaç taşımalı");
    while a.poll_event().is_some() {}

    let hub = HubHandle::from_raw(0x0000_0A55);
    let assign = |a: &mut T::Crypto, session, hub_handle| {
        a.command(SessionCommand::AssignHandle {
            session,
            hub_handle,
        })
    };
    assert_eq!(assign(&mut a, h, hub), SessionResult::NoOutput);
    assert_eq!(
        assign(&mut a, h, hub),
        SessionResult::NoOutput,
        "aynı bağın tekrarı idempotent"
    );
    let bound = T::state_fingerprint(&a);
    assert_eq!(
        assign(&mut a, other, hub),
        SessionResult::Failed,
        "hub tutamacı başka oturumda"
    );
    assert_eq!(
        T::state_fingerprint(&a),
        bound,
        "başarısız bağ state değiştirdi"
    );
    assert_eq!(a.poll_event(), None, "bağ olay üretmez");

    // Bob yanıtlar; Alice RESPONSE'u görmeden bekleyen oturumu kapatır.
    let resp = match b.command(SessionCommand::AcceptInit { wire: init }) {
        SessionResult::EstablishedAndReply { bytes, .. } => bytes,
        other => panic!("AcceptInit RESPONSE üretmeli, gelen {other:?}"),
    };
    assert_eq!(
        a.command(SessionCommand::Close { session: h }),
        SessionResult::NoOutput
    );
    assert_eq!(
        a.poll_event(),
        Some(SessionEvent::Closed(h, CloseReason::UserClosed))
    );
    assert_eq!(a.poll_event(), None, "tam bir Closed");
    assert_eq!(
        a.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Failed,
        "kapatılmış bekleyen oturumun geç RESPONSE'u kabul edildi"
    );

    let before = T::state_fingerprint(&a);
    assert_eq!(
        a.command(SessionCommand::Close {
            session: SessionHandle::from_raw(0x0BAD)
        }),
        SessionResult::NoOutput
    );
    assert_eq!(a.poll_event(), None, "bilinmeyen tutamaçta olay yok");
    assert_eq!(T::state_fingerprint(&a), before);
}
