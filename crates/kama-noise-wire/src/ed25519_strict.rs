//! Ed25519 katı doğrulama kipi (ccr §5.2; RFC 8032 §5.1.7) — TÜM Ed25519 doğrulamalarının tek
//! yolu.
//!
//! RFC 8032 iki denkleme (kofaktörlü / kofaktörsüz) izin verir ve küçük mertebeli bileşenler
//! hakkında sessizdir; kip seçilmezse iki implementasyon aynı imzayı farklı sınıflandırır. Bu kip:
//! `0 ≤ S < ℓ` · A ve R kanonik kodlanmış (`y < p`; `x = 0` iken işaret biti 0) · A ve R küçük
//! mertebeli DEĞİL · denklem KOFAKTÖRSÜZ (`[S]B = R + [k]A`).
//!
//! **Ret önceliği (vektör sözlüğü, ccr-ed25519-neg-v1 `precedence`):** uzunluk → `S < ℓ` → nokta
//! kodlaması (A, sonra R) → küçük mertebeli A → küçük mertebeli R → denklem. `ed25519-dalek`'in
//! `verify_strict`'i küçük mertebe ve kofaktörsüz denklemi kapsar ama gerekçeyi ayırt etmez ve
//! kanonik olmayan `y ≥ p`'yi indirger; kip bu yüzden kontrolleri açıkça ve bu sırayla yapar
//! (ccr §5.2: "kütüphane bir kuralı kapsamıyorsa açık kontrol eklenir").

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

/// Genel anahtar boyu.
pub const PUBLIC_KEY_LEN: usize = 32;
/// İmza boyu (`R ‖ S`).
pub const SIGNATURE_LEN: usize = 64;
const POINT_LEN: usize = 32;

/// ℓ = 2^252 + 27742317777372353535851937790883648493 (küçük-endian).
const GROUP_ORDER_LE: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];
/// p = 2^255 − 19 (küçük-endian).
const FIELD_PRIME_LE: [u8; 32] = [
    0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];
/// p − 1 (küçük-endian) — `x = 0` olan iki noktadan biri (`y = −1`).
const FIELD_PRIME_MINUS_ONE_LE: [u8; 32] = [
    0xec, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];
/// Sıkıştırılmış noktada işaret biti (bayt 31, bit 7).
const SIGN_BIT: u8 = 0x80;

/// Ed25519 reddi — ilk düşen kural.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Ed25519Reject {
    #[error("ed25519 key or signature length")]
    Length,
    #[error("ed25519 S not below group order")]
    SNotBelowGroupOrder,
    #[error("ed25519 non-canonical point encoding")]
    NonCanonicalPointEncoding,
    #[error("ed25519 point does not decode")]
    PointDecoding,
    #[error("ed25519 small-order public key")]
    SmallOrderPublicKey,
    #[error("ed25519 small-order R")]
    SmallOrderR,
    #[error("ed25519 signature equation failed")]
    Equation,
}

/// Küçük-endian 256-bit karşılaştırma: `a < b`. Girdiler genel veridir (imza ve anahtar) —
/// sabit zaman gerekmez.
fn lt_le(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in (0..32).rev() {
        if a[i] != b[i] {
            return a[i] < b[i];
        }
    }
    false
}

/// RFC 8032 §5.1.3 kanonik kodlama: `y < p` ve `x = 0` iken (`y = ±1`) işaret biti 0.
fn is_canonical_point(encoded: &[u8; POINT_LEN]) -> bool {
    let mut y = *encoded;
    y[31] &= !SIGN_BIT;
    if !lt_le(&y, &FIELD_PRIME_LE) {
        return false;
    }
    let sign = encoded[31] & SIGN_BIT != 0;
    let mut one = [0u8; 32];
    one[0] = 1;
    !(sign && (y == one || y == FIELD_PRIME_MINUS_ONE_LE))
}

/// ccr §5.2 katı doğrulama. Başarı = imza `public_key` altında `message` için geçerli.
///
/// # Errors
/// İlk düşen kural ([`Ed25519Reject`]); sıra modül belgesinde.
pub fn verify_strict(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), Ed25519Reject> {
    let a: &[u8; PUBLIC_KEY_LEN] = public_key.try_into().map_err(|_| Ed25519Reject::Length)?;
    let sig: &[u8; SIGNATURE_LEN] = signature.try_into().map_err(|_| Ed25519Reject::Length)?;
    let mut r = [0u8; POINT_LEN];
    r.copy_from_slice(&sig[..POINT_LEN]);
    let mut s = [0u8; 32];
    s.copy_from_slice(&sig[POINT_LEN..]);

    if !lt_le(&s, &GROUP_ORDER_LE) {
        return Err(Ed25519Reject::SNotBelowGroupOrder);
    }
    if !is_canonical_point(a) || !is_canonical_point(&r) {
        return Err(Ed25519Reject::NonCanonicalPointEncoding);
    }
    let vk_a = VerifyingKey::from_bytes(a).map_err(|_| Ed25519Reject::PointDecoding)?;
    let vk_r = VerifyingKey::from_bytes(&r).map_err(|_| Ed25519Reject::PointDecoding)?;
    if vk_a.is_weak() {
        return Err(Ed25519Reject::SmallOrderPublicKey);
    }
    if vk_r.is_weak() {
        return Err(Ed25519Reject::SmallOrderR);
    }
    vk_a.verify_strict(message, &Signature::from_bytes(sig))
        .map_err(|_| Ed25519Reject::Equation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn valid_signature_verifies_and_any_bit_flip_fails() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let msg = b"kama ed25519 kati kip";
        let sig = sk.sign(msg).to_bytes();
        let pk = sk.verifying_key().to_bytes();
        assert_eq!(verify_strict(&pk, msg, &sig), Ok(()));
        let mut bad = sig;
        bad[10] ^= 1;
        assert!(verify_strict(&pk, msg, &bad).is_err());
        assert_eq!(
            verify_strict(&pk, b"baska mesaj", &sig),
            Err(Ed25519Reject::Equation)
        );
    }

    #[test]
    fn length_is_checked_first() {
        assert_eq!(
            verify_strict(&[0; 31], b"", &[0; 64]),
            Err(Ed25519Reject::Length)
        );
        assert_eq!(
            verify_strict(&[0; 32], b"", &[0; 63]),
            Err(Ed25519Reject::Length)
        );
    }

    #[test]
    fn field_constants_are_consistent() {
        // p − 1 < p ve ℓ < p (sabitlerin kendileri yazım hatasına karşı).
        assert!(lt_le(&FIELD_PRIME_MINUS_ONE_LE, &FIELD_PRIME_LE));
        assert!(lt_le(&GROUP_ORDER_LE, &FIELD_PRIME_LE));
    }
}
