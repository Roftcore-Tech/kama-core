//! `BindSigner` — PoC-BIND kanıtının imzacısı (S4 §2.4(b), KK-H43).
//!
//! client-core'da anahtar tipi, anahtar alanı ya da anahtar baytına erişen kod YOKTUR: [`HubLink`]
//! yalnız `Box<dyn BindSigner>` tutar. Gerçekleme bileşim kökündedir (kama-node `wiring.rs`, HB2);
//! cihaz Ed25519 gizli anahtarı (sertifikadaki cihaz anahtarı, kimlik anahtarı DEĞİL; S4 KR-5) onun
//! içinde kalır ve buradan okunamaz (INV-ENGINE-SOC).
//!
//! İmza kâhini yoktur (D-14): imzacıya keyfi bayt verilmez. TBS'yi [`HubLink`] tipli alanlardan
//! kurar (`codec::bind_proof_tbs`, 112 B) ve imzacı yalnız o sabit boyutlu diziyi görür.
//!
//! [`HubLink`]: super::link::HubLink

/// BIND_PROOF TBS boyu (S4 §2.4(b): `domain_sep ‖ hub_ed25519_pub ‖ device_cert_ref ‖ client_nonce ‖
/// hub_nonce` = 16 + 32 + 32 + 16 + 16).
pub const BIND_TBS_LEN: usize = 112;

/// Ed25519 imza boyu.
pub const BIND_SIGNATURE_LEN: usize = 64;

/// Cihaz Ed25519 imzacısı.
///
/// `sign_bind` saf bir imzadır: aynı TBS → aynı imza (Ed25519 deterministik). Hata dönmez; anahtarı
/// olmayan bir bileşim kökü `HubLink`'i hiç kurmaz.
pub trait BindSigner: Send {
    /// `client_signature = Ed25519_Sign(device_ed25519_sk, tbs)`.
    fn sign_bind(&self, tbs: &[u8; BIND_TBS_LEN]) -> [u8; BIND_SIGNATURE_LEN];
}
