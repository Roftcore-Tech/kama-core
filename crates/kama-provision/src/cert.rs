//! CompactDeviceCertV1 (ccr §13.1 / §13.2) — PoC kodlayıcısı ve kimlik türetmeleri
//! (PROVISIONING-FORMAT-v0 §4.3–§4.5).
//!
//! ```text
//! [  0.. 32) device_ed25519_pub   [ 32.. 64) device_x25519_pub   [ 64.. 80) issuer_key_id
//! [ 80.. 88) not_after (u64_be)   [ 88..152) issuer_signature = Ed25519_Sign(issuer, tbs)
//! tbs = domain_sep("SR_CDC_V1") (16 B) ‖ bayt [0..88)
//! ```
//!
//! PoC'de Identity/User Cert katmanı yoktur: sertifikayı test kökü DOĞRUDAN imzalar (D9; PoC-KAPSAM-001
//! §2 "Anahtarlar"). Çalışma zamanında imza doğrulanmaz (ccr §35.4 KAPALI-SİSTEM KİPİ);
//! [`verify_compact_device_cert`] testler ve çevrim dışı araçlar içindir.
//!
//! **DUR-K4G-12 CEVAP (plan Ek 2026-10-11 (3)):** `kama-noise-wire` CompactDeviceCert kodlayıcısı ve
//! `dest_hash` türetmesi sunmaz (`handshake.rs` sertifikayı opak 152 B taşır); ikisi burada ccr §13'ten
//! yazılır. (a) `dest_hash` cihaz anahtarından DEĞİL, cihazdan ayrı kimlik Ed25519 anahtarından türer
//! ([`identity_hash`], [`dest_hash`]; ccr §13.3 / §13.5). (b) [`DOMAIN_SEP_CDC_V1`] ccr §13.2 YAMA KAYDI
//! 2026-10-11 tanımıdır. Altın vektörler: `vectors/ccr-dest-hash-v1/`, `vectors/ccr-cdc-v1/`
//! (`ref/crypto/gen_vectors.py`; tüketici `tests/ccr_cert_vectors.rs`).

use ed25519_dalek::{Signer, SigningKey};
use kama_noise_wire::ed25519_strict::{verify_strict, Ed25519Reject};
use kama_noise_wire::handshake::COMPACT_DEVICE_CERT_LEN;
use sha2::{Digest, Sha256};

/// 32 B anahtar / özet.
pub const KEY_LEN: usize = 32;
/// `issuer_key_id` (ccr §13.4: imzalayanın `subject_key_id`'sinin ilk 16 baytı).
pub const ISSUER_KEY_ID_LEN: usize = 16;
/// `not_after` (u64_be, Unix saniyesi).
pub const NOT_AFTER_LEN: usize = 8;
/// Ed25519 imzası (`R ‖ S`).
pub const SIGNATURE_LEN: usize = 64;
/// `domain_sep(...)` etiketi: "16B sabit etiket" (ccr §13.2).
pub const DOMAIN_SEP_LEN: usize = 16;
/// `domain_sep("SR_CDC_V1")` — ccr §13.2 YAMA KAYDI 2026-10-11 (K4G-12; DUR-K4G-12 (b)): ASCII etiket
/// (≤ 16 B) + `0x00` dolgu → `53525f4344435f5631` + `00`×7. Altın vektör
/// `ccr-cdc-v1.domain-sep.sr-cdc-v1`.
pub const DOMAIN_SEP_CDC_V1: [u8; DOMAIN_SEP_LEN] = *b"SR_CDC_V1\0\0\0\0\0\0\0";
/// İmzalanan alanlar: `device_ed25519_pub ‖ device_x25519_pub ‖ issuer_key_id ‖ not_after` (88 B).
pub const SIGNED_FIELDS_LEN: usize = KEY_LEN + KEY_LEN + ISSUER_KEY_ID_LEN + NOT_AFTER_LEN;
/// `compact_device_cert_tbs` (104 B).
pub const TBS_LEN: usize = DOMAIN_SEP_LEN + SIGNED_FIELDS_LEN;
/// `spki_encoding` algoritma kodu: Ed25519 (ccr §6.3).
pub const SPKI_ALG_ED25519: u8 = 0x01;
/// Cihaz sertifikası geçerliliği: 90 gün (ccr §13.1 "max +90 gün").
pub const DEVICE_CERT_VALIDITY_SECS: u64 = 90 * 24 * 60 * 60;

const OFF_X25519: usize = KEY_LEN;
const OFF_ISSUER_KEY_ID: usize = OFF_X25519 + KEY_LEN;
const OFF_NOT_AFTER: usize = OFF_ISSUER_KEY_ID + ISSUER_KEY_ID_LEN;
const OFF_SIGNATURE: usize = OFF_NOT_AFTER + NOT_AFTER_LEN;

// Düzen ccr §13.1 ile birebir: 32 + 32 + 16 + 8 + 64 = 152 (kama-noise-wire'ın tel uzunluğu).
const _: () = assert!(OFF_SIGNATURE == SIGNED_FIELDS_LEN);
const _: () = assert!(SIGNED_FIELDS_LEN + SIGNATURE_LEN == COMPACT_DEVICE_CERT_LEN);

/// Sertifika hatası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CertError {
    #[error("ed25519 signing failed")]
    Sign,
    #[error("compact device cert signature rejected: {0}")]
    Signature(Ed25519Reject),
}

/// CompactDeviceCertV1'in imzalanan alanları (açık değerler).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertFields {
    pub device_ed25519_pub: [u8; KEY_LEN],
    pub device_x25519_pub: [u8; KEY_LEN],
    pub issuer_key_id: [u8; ISSUER_KEY_ID_LEN],
    /// Unix saniyesi (UTC).
    pub not_after: u64,
}

impl CertFields {
    /// İmzalanan 88 B, ccr §13.2 sırasıyla.
    #[must_use]
    pub fn signed_fields(&self) -> [u8; SIGNED_FIELDS_LEN] {
        let mut out = [0u8; SIGNED_FIELDS_LEN];
        out[..OFF_X25519].copy_from_slice(&self.device_ed25519_pub);
        out[OFF_X25519..OFF_ISSUER_KEY_ID].copy_from_slice(&self.device_x25519_pub);
        out[OFF_ISSUER_KEY_ID..OFF_NOT_AFTER].copy_from_slice(&self.issuer_key_id);
        out[OFF_NOT_AFTER..].copy_from_slice(&self.not_after.to_be_bytes());
        out
    }

    /// `compact_device_cert_tbs` = `domain_sep("SR_CDC_V1")` ‖ imzalanan alanlar (ccr §13.2).
    #[must_use]
    pub fn tbs(&self) -> [u8; TBS_LEN] {
        let mut out = [0u8; TBS_LEN];
        out[..DOMAIN_SEP_LEN].copy_from_slice(&DOMAIN_SEP_CDC_V1);
        out[DOMAIN_SEP_LEN..].copy_from_slice(&self.signed_fields());
        out
    }
}

/// Sertifikayı kodlar ve `issuer` ile imzalar (PoC: issuer = test kökü).
///
/// # Errors
/// [`CertError::Sign`].
pub fn sign_compact_device_cert(
    fields: &CertFields,
    issuer: &SigningKey,
) -> Result<[u8; COMPACT_DEVICE_CERT_LEN], CertError> {
    let tbs = fields.tbs();
    let signature = issuer.try_sign(&tbs).map_err(|_| CertError::Sign)?;
    let mut cert = [0u8; COMPACT_DEVICE_CERT_LEN];
    cert[..OFF_SIGNATURE].copy_from_slice(&fields.signed_fields());
    cert[OFF_SIGNATURE..].copy_from_slice(&signature.to_bytes());
    Ok(cert)
}

/// 152 B sertifikayı alanlarına ve imzasına ayırır. Yapısaldır; doğrulama YAPMAZ.
#[must_use]
pub fn split_compact_device_cert(
    cert: &[u8; COMPACT_DEVICE_CERT_LEN],
) -> (CertFields, [u8; SIGNATURE_LEN]) {
    let mut device_ed25519_pub = [0u8; KEY_LEN];
    device_ed25519_pub.copy_from_slice(&cert[..OFF_X25519]);
    let mut device_x25519_pub = [0u8; KEY_LEN];
    device_x25519_pub.copy_from_slice(&cert[OFF_X25519..OFF_ISSUER_KEY_ID]);
    let mut issuer_key_id = [0u8; ISSUER_KEY_ID_LEN];
    issuer_key_id.copy_from_slice(&cert[OFF_ISSUER_KEY_ID..OFF_NOT_AFTER]);
    let mut not_after = [0u8; NOT_AFTER_LEN];
    not_after.copy_from_slice(&cert[OFF_NOT_AFTER..OFF_SIGNATURE]);
    let mut signature = [0u8; SIGNATURE_LEN];
    signature.copy_from_slice(&cert[OFF_SIGNATURE..]);
    let fields = CertFields {
        device_ed25519_pub,
        device_x25519_pub,
        issuer_key_id,
        not_after: u64::from_be_bytes(not_after),
    };
    (fields, signature)
}

/// Sertifikanın imzasını `issuer_ed25519_pub`'a karşı ccr §5.2 katı kipiyle doğrular
/// (`kama_noise_wire::ed25519_strict::verify_strict`). Kapsam YALNIZ ccr §13.8 adım 9'dur (imza);
/// zincir, süre, iptal ve kimlik adımları PoC kapsamı dışıdır (PKI yok). Çalışma zamanında ÇAĞRILMAZ
/// (ccr §35.4 KAPALI-SİSTEM KİPİ); testler ve çevrim dışı araçlar içindir.
///
/// # Errors
/// [`CertError::Signature`] — katı kipin ilk düşen kuralı.
pub fn verify_compact_device_cert(
    cert: &[u8; COMPACT_DEVICE_CERT_LEN],
    issuer_ed25519_pub: &[u8; KEY_LEN],
) -> Result<(), CertError> {
    let (fields, signature) = split_compact_device_cert(cert);
    let tbs = fields.tbs();
    verify_strict(issuer_ed25519_pub, &tbs, &signature).map_err(CertError::Signature)
}

/// `issuer_key_id` = `SHA-256(spki_encoding(0x01, issuer_pub))[0..16]` — `spki_encoding` = `alg (u8) ‖
/// key (32 B)` (ccr §8.1), ilk 16 bayt (ccr §13.4). Ham anahtarın özeti DEĞİLDİR (ccr §8.1 ayrımı).
#[must_use]
pub fn issuer_key_id(issuer_ed25519_pub: &[u8; KEY_LEN]) -> [u8; ISSUER_KEY_ID_LEN] {
    let mut hasher = Sha256::new();
    hasher.update([SPKI_ALG_ED25519]);
    hasher.update(issuer_ed25519_pub);
    let digest = hasher.finalize();
    let mut id = [0u8; ISSUER_KEY_ID_LEN];
    id.copy_from_slice(&digest[..ISSUER_KEY_ID_LEN]);
    id
}

/// `identity_hash = SHA-256(user_identity_ed25519_pub)` (ccr §13.3 / §13.5, CAD-16) — HAM 32 B anahtar
/// üzerinden, spki değil (ccr §8.1). Girdi KİMLİK anahtarıdır: cihaz Ed25519 anahtarından türetmek ccr
/// §13.3'te yasaktır (DUR-K4G-12 (a) — o öneri reddedildi, plan Ek 2026-10-11 (3)).
#[must_use]
pub fn identity_hash(identity_ed25519_pub: &[u8; KEY_LEN]) -> [u8; KEY_LEN] {
    Sha256::digest(identity_ed25519_pub).into()
}

/// `dest_hash = SHA-256(identity_hash)` (ccr §13.5, CAD-16).
#[must_use]
pub fn dest_hash(identity_hash: &[u8; KEY_LEN]) -> [u8; KEY_LEN] {
    Sha256::digest(identity_hash).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fields(issuer: &SigningKey) -> CertFields {
        CertFields {
            device_ed25519_pub: SigningKey::from_bytes(&[3; KEY_LEN])
                .verifying_key()
                .to_bytes(),
            device_x25519_pub: [5; KEY_LEN],
            issuer_key_id: issuer_key_id(&issuer.verifying_key().to_bytes()),
            not_after: 1_791_676_800 + DEVICE_CERT_VALIDITY_SECS,
        }
    }

    #[test]
    fn domain_separation_label_is_ascii_then_zero_padding_to_sixteen_bytes() {
        assert_eq!(&DOMAIN_SEP_CDC_V1[..9], b"SR_CDC_V1");
        assert!(DOMAIN_SEP_CDC_V1[9..].iter().all(|&b| b == 0));
        assert_eq!(TBS_LEN, 104);
    }

    #[test]
    fn layout_follows_ccr_13_1_offsets() {
        let issuer = SigningKey::from_bytes(&[7; KEY_LEN]);
        let fields = sample_fields(&issuer);
        let cert = sign_compact_device_cert(&fields, &issuer).expect("imza");
        assert_eq!(cert.len(), 152);
        assert_eq!(cert[0..32], fields.device_ed25519_pub);
        assert_eq!(cert[32..64], fields.device_x25519_pub);
        assert_eq!(cert[64..80], fields.issuer_key_id);
        assert_eq!(cert[80..88], fields.not_after.to_be_bytes());
        let (parsed, _) = split_compact_device_cert(&cert);
        assert_eq!(parsed, fields);
        let tbs = fields.tbs();
        assert_eq!(tbs[..16], DOMAIN_SEP_CDC_V1);
        assert_eq!(tbs[16..], cert[..88]);
    }

    #[test]
    fn signed_certificate_verifies_against_its_issuer_only() {
        let issuer = SigningKey::from_bytes(&[7; KEY_LEN]);
        let fields = sample_fields(&issuer);
        let cert = sign_compact_device_cert(&fields, &issuer).expect("imza");
        let issuer_pub = issuer.verifying_key().to_bytes();
        assert_eq!(verify_compact_device_cert(&cert, &issuer_pub), Ok(()));

        let other_signer = SigningKey::from_bytes(&[8; KEY_LEN]);
        let other = other_signer.verifying_key().to_bytes();
        assert!(verify_compact_device_cert(&cert, &other).is_err());
        for index in [0, 40, 70, 85, 100, 151] {
            let mut tampered = cert;
            tampered[index] ^= 0x01;
            assert!(
                verify_compact_device_cert(&tampered, &issuer_pub).is_err(),
                "bayt {index} değişince imza düşmeli"
            );
        }
    }

    #[test]
    fn identity_derivations_hash_what_ccr_names() {
        let signer = SigningKey::from_bytes(&[9; KEY_LEN]);
        let key = signer.verifying_key().to_bytes();
        // spki (33 B) özeti ≠ ham anahtar özeti (ccr §8.1).
        let raw = identity_hash(&key);
        assert_ne!(issuer_key_id(&key), raw[..ISSUER_KEY_ID_LEN]);
        // dest_hash kimlik özetinin özetidir; kimlik özetinin kendisi değildir.
        assert_ne!(dest_hash(&raw), raw);
    }
}
