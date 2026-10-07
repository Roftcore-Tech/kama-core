//! H-REPLAY-POISON — sahte paket replay penceresini ilerletmez; ardından AYNI sayaçla gelen geçerli
//! paket kabul edilir ve commit edilir (identity §15.5: precheck saf, commit yalnız AEAD başarısında).

use kama_contracts::s3::PacketProtection;

use crate::s3_support::{established, session, tamper};
use crate::target::ProtectionTarget;

pub fn forged_packet_does_not_poison_the_window<T: ProtectionTarget>() {
    let mut p = established::<T>();
    let ctx = session(p.h);
    // Pencereyi biraz ilerlet, sonra sayaç N'li paketi hazırla.
    for _ in 0..3 {
        let w = p.a.seal(ctx, b"on").unwrap().bytes;
        assert!(p.b.open(&w).is_some());
    }
    let valid = p.a.seal(ctx, b"asil paket").unwrap();
    let n = valid.meta.full_wire_pn;

    let s = T::state_fingerprint(&p.b);
    // Sahte paket: aynı sayaç N, bozuk tag.
    assert!(p.b.open(&tamper(&valid.bytes)).is_none());
    assert_eq!(
        T::state_fingerprint(&p.b),
        s,
        "sahte paket state'i ilerletti"
    );

    // Geçerli paket (sayaç N) hâlâ kabul edilir ve commit edilir.
    let opened =
        p.b.open(&valid.bytes)
            .expect("sahte paket pencereyi zehirledi");
    assert_eq!(opened.meta.full_wire_pn, n);
    assert_eq!(opened.frame, b"asil paket");
    assert_ne!(T::state_fingerprint(&p.b), s, "commit state'e yansımadı");
    assert!(
        p.b.open(&valid.bytes).is_none(),
        "commit sonrası aynı sayaç tekrar kabul edildi"
    );
}
