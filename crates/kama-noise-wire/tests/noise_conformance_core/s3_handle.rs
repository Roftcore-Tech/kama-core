//! H-HANDLE (çalışma zamanı yarısı) — kapanmış oturumun tutamacıyla `seal` → `UnknownSession`.
//!
//! Tip yarısı (`HubHandle` ≠ `SessionHandle`) derleme zamanında kanıtlanır: trybuild negatif testi
//! `tests/ui/hub_handle_is_not_session_handle.rs` (bkz. `tests/trybuild.rs`) ve toolchain'den
//! bağımsız karşılığı `kama-contracts` birim testi `hub_handle_does_not_convert_to_session_handle`.
//! Kapsama tablosunda ayrı satırdır ("H-HANDLE/type").

use kama_contracts::s3::{
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
