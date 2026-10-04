//! H-NO-MUTATE — her başarısız yolda replay penceresi, pending kümesi ve anahtar/oturum state'i
//! byte-for-byte aynı kalır (S3 §3; P0-6 / P0-13).

use client_core::seam::{
    PacketProtection, ProtectionContext, SealError, SessionCommand, SessionControl, SessionHandle,
    SessionResult,
};

use crate::s3_support::{established, initiate, session, tamper, PEER};
use crate::target::S3Target;

pub fn failures_leave_state_byte_identical<T: S3Target>() {
    let mut p = established::<T>();
    let ctx = session(p.h);
    // Bob'da pending de olsun: yanlış aday yolu boş kümede sınanmasın.
    let _ = initiate::<T>(&mut p.b, PEER + 1);
    while p.b.poll_event().is_some() {}

    let w1 = p.a.seal(ctx, b"bir").unwrap().bytes;
    let w2 = p.a.seal(ctx, b"iki").unwrap().bytes;
    assert!(p.b.open(&w1).is_some(), "pencere boş olmasın");
    let s0 = T::state_fingerprint(&p.b);
    let check = |b: &T::Crypto, what: &str| {
        assert_eq!(T::state_fingerprint(b), s0, "{what}: state değişti");
    };

    // bozuk tag
    assert!(p.b.open(&tamper(&w2)).is_none());
    check(&p.b, "bozuk tag");
    // replay
    assert!(p.b.open(&w1).is_none());
    check(&p.b, "replay");
    // malformed datagram
    assert!(p.b.open(&[]).is_none());
    assert!(p.b.open(&w2[..w2.len() / 2]).is_none());
    check(&p.b, "kesik datagram");
    // yanlış aday / bozuk RESPONSE / bozuk INIT
    let stray = T::response_for(0xDEAD);
    assert_eq!(
        p.b.command(SessionCommand::AcceptResponse { wire: stray }),
        SessionResult::Failed
    );
    check(&p.b, "yanlış aday");
    assert_eq!(
        p.b.command(SessionCommand::AcceptResponse { wire: vec![7; 3] }),
        SessionResult::Failed
    );
    check(&p.b, "bozuk RESPONSE");
    assert_eq!(
        p.b.command(SessionCommand::AcceptInit { wire: vec![7; 3] }),
        SessionResult::Failed
    );
    check(&p.b, "bozuk INIT");
    // bilinmeyen oturuma seal
    let unknown = ProtectionContext::Session(SessionHandle::from_raw(0x0BAD));
    assert_eq!(
        p.b.seal(unknown, b"x").unwrap_err(),
        SealError::UnknownSession
    );
    check(&p.b, "bilinmeyen oturum");
    assert!(p.b.poll_event().is_none(), "başarısızlık olay üretmez");

    // Hiçbir başarısızlık geçerli paketi tüketmedi.
    assert!(p.b.open(&w2).is_some());
}
