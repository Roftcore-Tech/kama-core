//! H-CTX — bağlam çağırandan gelir ve seam'e DEĞİŞMEDEN ulaşır (S3 K1).
//!
//! İddia hedefe göre ayrılır (sahip kararı 2026-10-06):
//! * her hedefte: `seal()`'e verilen `ProtectionContext` seam'in kaydettiğiyle aynıdır; içerik aynı
//!   kalsa bile bağlam içerikten türetilmez;
//! * anahtar ayrımı olan hedefte (`mock`): `Session` çıktısı ≠ `HubChannel` çıktısı;
//! * PoC `IdentitySeam`: iki bağlamda da çıktı plaintext'tir (no-op); "Session ≠ HubChannel" bu
//!   hedefte UYGULANMAZ (gerçek anahtar M4). `IdentitySeam` `client-core`'undur ve bu crate
//!   `client-core`'u göremez (katman kuralı C3); o yarı `client-core/tests/noise_identity_target.rs`
//!   içindedir, koşucu onu `--target identity` ile koşar.

use kama_contracts::s3::{PacketProtection, ProtectionContext};

use crate::s3_support::{established, session};
use crate::target::S3Target;

const FRAME: &[u8] = b"ayni cerceve, iki baglam";

/// Bağlam kaydı çağıranın verdiğiyle aynıdır — aynı içerik iki bağlamda da.
pub fn ctx_reaches_seam_unchanged<T: S3Target>() {
    let mut p = established::<T>();
    for ctx in [
        session(p.h),
        ProtectionContext::HubChannel,
        session(p.h),
        ProtectionContext::HubChannel,
    ] {
        p.a.seal(ctx, FRAME).expect("kurulu bağlamda seal");
        assert_eq!(
            T::last_seal_ctx(&p.a),
            Some(ctx),
            "seam'in gördüğü bağlam çağıranın verdiği değil"
        );
    }
}

/// Anahtar ayrımı olan hedef: aynı çerçeve iki bağlamda FARKLI mühürlenir ve bağlamlar ayrı PN
/// uzayıdır; alıcı her birini kendi bağlamına atfeder.
pub fn session_and_hub_seal_differently<T: S3Target>() {
    assert!(
        T::CONTEXTS_ARE_KEY_SEPARATED,
        "bu satır anahtar ayrımı olan hedef içindir"
    );
    let mut p = established::<T>();
    let s = p.a.seal(session(p.h), FRAME).unwrap();
    let hub = p.a.seal(ProtectionContext::HubChannel, FRAME).unwrap();
    assert_ne!(
        s.bytes, hub.bytes,
        "Session ve HubChannel aynı byte'ları üretti"
    );
    assert_eq!(
        s.meta.full_wire_pn, hub.meta.full_wire_pn,
        "bağlamlar ayrı PN uzayıdır: ikisi de kendi sayacının başındadır"
    );
    assert_eq!(p.b.open(&s.bytes).map(|o| o.ctx), Some(session(p.h)));
    assert_eq!(
        p.b.open(&hub.bytes).map(|o| o.ctx),
        Some(ProtectionContext::HubChannel)
    );
}
