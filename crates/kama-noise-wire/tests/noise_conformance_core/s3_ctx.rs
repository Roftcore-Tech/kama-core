//! H-CTX — bağlam çağırandan gelir ve seam'e DEĞİŞMEDEN ulaşır (S3 K1).
//!
//! İddia hedefe göre ayrılır (sahip kararı 2026-10-06):
//! * her hedefte: `seal()`'e verilen `ProtectionContext` seam'in kaydettiğiyle aynıdır; içerik aynı
//!   kalsa bile bağlam içerikten türetilmez;
//! * anahtar ayrımı olan hedefte (`mock`): `Session` çıktısı ≠ `HubChannel` çıktısı;
//! * PoC `IdentitySeam`: iki bağlamda da çıktı plaintext'tir (no-op); "Session ≠ HubChannel" bu
//!   hedefte UYGULANMAZ (gerçek anahtar M4).

use client_core::seam::{IdentitySeam, PacketProtection, ProtectionContext, IDENTITY_TAG};

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

/// PoC `IdentitySeam`: bağlam kaydı doğru; iki bağlamda da çıktı = plaintext (+ sıfır dolgu).
pub fn identity_seam_is_noop_in_both_contexts() {
    let mut seam = IdentitySeam::new();
    let mut outputs = Vec::new();
    for ctx in [
        ProtectionContext::Session(IdentitySeam::SESSION),
        ProtectionContext::HubChannel,
    ] {
        let sealed = seam.seal(ctx, FRAME).expect("identity seal");
        assert_eq!(seam.last_seal_ctx(), Some(ctx));
        assert_eq!(&sealed.bytes[..FRAME.len()], FRAME, "plaintext değişti");
        assert_eq!(sealed.bytes.len(), FRAME.len() + IDENTITY_TAG);
        assert!(
            sealed.bytes[FRAME.len()..].iter().all(|b| *b == 0),
            "sahte tag sıfır dolgu olmalı"
        );
        outputs.push(sealed.bytes);
    }
    assert_eq!(
        outputs[0], outputs[1],
        "no-op seam'de iki bağlam aynı byte'ları üretir"
    );
}
