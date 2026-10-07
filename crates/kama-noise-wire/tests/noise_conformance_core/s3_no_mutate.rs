//! H-NO-MUTATE — her başarısız yolda replay penceresi, pending kümesi ve anahtar/oturum state'i
//! byte-for-byte aynı kalır (S3 §3; P0-6 / P0-13).
//!
//! İki yarı [2026-10-08, L-A5b]: paket yarısı (bozuk tag, replay, kesik datagram, bilinmeyen
//! oturuma seal) yalnız `PacketProtection` ister ve her hedefte koşar; kontrol yarısı (yanlış aday,
//! bozuk RESPONSE, bozuk INIT) `SessionControl` ister — gerçek hedefte L-A5c'de.

use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{
    PacketProtection, ProtectionContext, SealError, SessionCommand, SessionControl, SessionResult,
};

use crate::s3_support::{established, initiate, session, tamper, Pair, PEER};
use crate::target::{ProtectionTarget, S3Target};

/// Paket yarısının başarısızlıkları. Döner: henüz açılmamış geçerli paket + başarısızlıklardan
/// ÖNCEKİ state parmak izi (kontrol yarısı aynı izle karşılaştırır).
fn packet_failures<T: ProtectionTarget>(p: &mut Pair<T>) -> (Vec<u8>, String) {
    let ctx = session(p.h);
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
    // bilinmeyen oturuma seal
    let unknown = ProtectionContext::Session(SessionHandle::from_raw(0x0BAD));
    assert_eq!(
        p.b.seal(unknown, b"x").unwrap_err(),
        SealError::UnknownSession
    );
    check(&p.b, "bilinmeyen oturum");
    (w2, s0)
}

/// Paket yarısı — her hedefte.
pub fn packet_failures_leave_state_byte_identical<T: ProtectionTarget>() {
    let mut p = established::<T>();
    let (w2, _) = packet_failures::<T>(&mut p);
    assert!(
        T::poll_event(&mut p.b).is_none(),
        "başarısızlık olay üretmez"
    );
    // Hiçbir başarısızlık geçerli paketi tüketmedi.
    assert!(p.b.open(&w2).is_some());
}

/// Tam satır: paket yarısı + kontrol yarısı.
pub fn failures_leave_state_byte_identical<T: S3Target>() {
    let mut p = established::<T>();
    // Bob'da pending de olsun: yanlış aday yolu boş kümede sınanmasın.
    let _ = initiate::<T>(&mut p.b, PEER + 1);
    while p.b.poll_event().is_some() {}

    let (w2, s0) = packet_failures::<T>(&mut p);
    let check = |b: &T::Crypto, what: &str| {
        assert_eq!(T::state_fingerprint(b), s0, "{what}: state değişti");
    };
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
    assert!(p.b.poll_event().is_none(), "başarısızlık olay üretmez");

    // Hiçbir başarısızlık geçerli paketi tüketmedi.
    assert!(p.b.open(&w2).is_some());
}
