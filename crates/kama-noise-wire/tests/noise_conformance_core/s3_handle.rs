//! H-HANDLE (çalışma zamanı yarısı) — kapanmış oturumun tutamacıyla `seal` → `UnknownSession`.
//!
//! Tip yarısı (`HubHandle` ≠ `SessionHandle`, trybuild negatif testi) `kama-contracts` işine (B1)
//! taşındı; kapsama tablosunda "H-HANDLE/type: pending B1".

use client_core::seam::{
    PacketProtection, SealError, SessionCommand, SessionControl, SessionEvent, SessionResult,
};

use crate::s3_support::{established, session};
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
