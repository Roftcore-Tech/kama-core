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
//! * gerçek hedef (`NoiseWireProtection`, L-A5b): HubChannel PoC no-op'tur (`IdentitySeam`
//!   paritesi) — no-op yarısı burada, [`hub_channel_is_identity_noop`]; Session gerçekten mühürlüdür.

use kama_contracts::s3::{PacketProtection, ProtectionContext};

use crate::s3_support::{established, session};
use crate::target::ProtectionTarget;

const FRAME: &[u8] = b"ayni cerceve, iki baglam";

/// Bağlam kaydı çağıranın verdiğiyle aynıdır — aynı içerik iki bağlamda da.
pub fn ctx_reaches_seam_unchanged<T: ProtectionTarget>() {
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
pub fn session_and_hub_seal_differently<T: ProtectionTarget>() {
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

/// No-op HubChannel hedefi (`CONTEXTS_ARE_KEY_SEPARATED = false`): gerçek hedefte HubChannel PoC
/// no-op'tur, `IdentitySeam` ile bayt/alan özdeş (gerçek hop anahtarı KAPSAM DIŞI, M4).
///
/// * HubChannel çıktısı `frame ‖ sıfır tag`; kendi sayacıyla 0'dan başlar (oturum sayacından
///   ayrı PN uzayı); `dh_epoch = 0`; boyu `sealed_len`'in söylediği.
/// * Session çıktısı MÜHÜRLÜDÜR: çerçeve wire'da açık görünmez.
/// * Alıcı her birini kendi bağlamına atfeder; oturum trafiği HubChannel'a ASLA düşmez (demux
///   önce): tag'i sıfırlanmış bir oturum datagramı `None`'dır, HubChannel değil.
pub fn hub_channel_is_identity_noop<T: ProtectionTarget>() {
    assert!(
        !T::CONTEXTS_ARE_KEY_SEPARATED,
        "bu satır no-op HubChannel hedefi içindir"
    );
    let mut p = established::<T>();
    let s = p.a.seal(session(p.h), FRAME).unwrap();
    let hub0 = p.a.seal(ProtectionContext::HubChannel, FRAME).unwrap();
    let hub1 = p.a.seal(ProtectionContext::HubChannel, FRAME).unwrap();

    let hub_len =
        p.a.sealed_len(ProtectionContext::HubChannel, FRAME.len())
            .expect("HubChannel sealed_len");
    for (i, hub) in [&hub0, &hub1].into_iter().enumerate() {
        assert_eq!(
            hub.bytes.len(),
            hub_len,
            "{i}. HubChannel boyu sealed_len değil"
        );
        assert_eq!(
            &hub.bytes[..FRAME.len()],
            FRAME,
            "{i}. HubChannel çerçeveyi değiştirdi"
        );
        assert!(
            hub.bytes[FRAME.len()..].iter().all(|&b| b == 0),
            "{i}. HubChannel tag'i sıfır değil"
        );
        assert_eq!(hub.meta.dh_epoch, 0, "{i}. HubChannel dh_epoch");
    }
    assert_eq!(
        (hub0.meta.full_wire_pn, hub1.meta.full_wire_pn),
        (0, 1),
        "HubChannel kendi sayacıyla 0'dan başlamalı (oturum sayacından ayrı)"
    );
    assert_eq!(
        s.meta.full_wire_pn, 0,
        "oturum sayacı HubChannel'dan etkilenmez"
    );
    assert!(
        !s.bytes.windows(FRAME.len()).any(|w| w == FRAME),
        "Session çıktısında çerçeve açık görünüyor"
    );

    let mut zero_tag = s.bytes.clone();
    let body = zero_tag.len() - (hub_len - FRAME.len());
    zero_tag[body..].fill(0);
    assert!(
        p.b.open(&zero_tag).is_none(),
        "tag'i sıfırlanmış oturum datagramı kabul edildi ya da HubChannel'a düştü"
    );
    let opened_s = p.b.open(&s.bytes).expect("Session açılır");
    assert_eq!(
        (opened_s.ctx, opened_s.frame.as_slice()),
        (session(p.h), FRAME)
    );
    let opened_hub = p.b.open(&hub0.bytes).expect("HubChannel açılır");
    assert_eq!(
        (opened_hub.ctx, opened_hub.frame.as_slice()),
        (ProtectionContext::HubChannel, FRAME)
    );
}
