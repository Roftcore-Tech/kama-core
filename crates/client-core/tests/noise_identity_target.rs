//! **noise conformance — `identity` hedefi** (S3 §6 H-CTX; sahip kararı 2026-10-06).
//!
//! H-CTX'in asıl iddiası "bağlam çağırandan gelir ve seam'e DEĞİŞMEDEN ulaşır"dır ve her hedefte
//! sınanır. PoC `IdentitySeam` no-op mühürdür: iki bağlamda da çıktı plaintext'tir; "Session ≠
//! HubChannel" (anahtar ayrımı) bu hedefte UYGULANMAZ — gerçek anahtarla (M4) sınanacak.
//! S4 (2026-10-08): Session sahte etiketinin ilk 8 baytı PN'dir (BE; alıcının ARQ kimliği, D1a) —
//! PN 0'da iki bağlamın baytları hâlâ özdeş, sonraki Session mühürlerinde etiket PN'i gösterir.
//!
//! Süitin geri kalanı `crates/kama-noise-wire/tests/noise_conformance_core` altındadır. Bu dosya
//! burada durur çünkü `IdentitySeam` bu crate'indir ve `kama-noise-wire` `client-core`'u göremez
//! (katman kuralı; `scripts/ci/layer_check.py` C3). Koşucu:
//! `scripts/ci/noise-conformance.sh --target identity`.

use client_core::seam::{IdentitySeam, IDENTITY_TAG};
use kama_contracts::s3::{PacketProtection, ProtectionContext};

const FRAME: &[u8] = b"ayni cerceve, iki baglam";

/// PoC `IdentitySeam`: bağlam kaydı doğru; iki bağlamda da çıktı = plaintext (+ sıfır dolgu).
#[test]
fn identity_h_ctx_seam_is_noop_in_both_contexts() {
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
        "no-op seam'de iki bağlam aynı byte'ları üretir (Session PN 0)"
    );
    let next = seam
        .seal(ProtectionContext::Session(IdentitySeam::SESSION), FRAME)
        .expect("identity seal");
    assert_eq!(
        &next.bytes[FRAME.len()..FRAME.len() + 8],
        &next.meta.full_wire_pn.to_be_bytes(),
        "S4: Session etiketinin ilk 8 baytı PN (BE)"
    );
    assert_eq!(next.meta.full_wire_pn, 1);
}
