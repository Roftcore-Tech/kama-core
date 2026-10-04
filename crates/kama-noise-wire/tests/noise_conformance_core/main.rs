//! **noise conformance-core** — S3 sözleşmesinin gözlenebilir davranışı (KM-SEAM-TC-001 §6).
//!
//! Düzen S1'deki `seam_conformance_core` kalıbıyla aynıdır (R-7(c)): satır gövdeleri hedef-agnostik
//! generic `fn`'lerdir (`demux.rs`), makro onları hedef başına ayrı `#[test]` olarak sarar. Sahte
//! ile gerçek için "ayrı ama benzer" test yoktur — aynı koddur.
//!
//! # Bugün tek hedef koşuyor
//!
//! `mock` kayıtlı; `real` DEĞİL ([`target::GAP_REAL_CRYPTO`] — gerçek kripto A5'te). `RealTarget`
//! derlenir ama `build()` typed "uygulanmadı" döner ve süite kaydedilmez. A5.5 indiğinde eklenecek
//! tek satır:
//!
//! ```ignore
//! noise_demux_suite!(real_demux, crate::target::RealTarget);
//! ```
//!
//! Koşucu: `scripts/ci/noise-conformance.sh --target mock|real`.

#[macro_use]
mod demux;
mod target;

noise_demux_suite!(mock_demux, crate::target::MockTarget);

/// `real` hedefinin yokluğu BEYAN EDİLMİŞ bir boşluktur; sessiz atlama değil. Gerçek hedef
/// kaydedildiği an bu sınama kırmızıya döner — kapsam sessizce değişemez.
#[test]
fn missing_target_is_a_declared_gap() {
    use target::{S3Target, TargetKind, TargetUnavailable};
    assert!(
        !target::REGISTERED_TARGETS
            .iter()
            .any(|(_, k)| *k == TargetKind::Real),
        "gerçek hedef kaydedildi: bu sınamayı kaldır ve real_* süitlerini ekle"
    );
    assert!(matches!(
        target::RealTarget::build(),
        Err(TargetUnavailable::NotImplemented { gap }) if gap == target::GAP_REAL_CRYPTO
    ));
}
