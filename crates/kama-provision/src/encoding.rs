//! std-only hex ve base64 (PROVISIONING-FORMAT-v0 §4.1). D11: harici kodlama crate'i yok.
//!
//! * **hex:** yalnız küçük harf `[0-9a-f]`, `0x` öneki yok; uzunluk tam olmalı.
//! * **base64:** RFC 4648 §4 standart alfabe, `=` dolgulu, KANONİK (kullanılmayan son bitler sıfır).
//!
//! Çözme hataları girdiden içerik TAŞIMAZ (yalnız uzunluk): gizli anahtar çözülürken hata mesajına ya
//! da log'a sızmasın.

/// Çözme hatası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("wrong encoded length: expected {expected} characters, got {actual}")]
    Length { expected: usize, actual: usize },
    #[error("invalid hex (lowercase 0-9a-f only)")]
    Hex,
    #[error("invalid or non-canonical base64")]
    Base64,
}

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_PAD: u8 = b'=';
/// Bir base64 dörtlüsünün taşıdığı bayt sayısı.
const BASE64_GROUP_BYTES: usize = 3;
/// Bir base64 dörtlüsünün karakter sayısı.
const BASE64_GROUP_CHARS: usize = 4;
/// Bir base64 karakterinin taşıdığı bit sayısı.
const BASE64_SEXTET_BITS: u32 = 6;
const BASE64_SEXTET_MASK: u32 = 0x3f;
/// 24 bitlik grubun dört altılısının kaydırma miktarları (soldan sağa).
const BASE64_SEXTET_SHIFTS: [u32; BASE64_GROUP_CHARS] = [18, 12, 6, 0];

/// Küçük harf hex.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Tam `2 * N` küçük harf hex karakterini `N` bayta çözer.
///
/// # Errors
/// [`DecodeError::Length`] · [`DecodeError::Hex`].
pub fn hex_decode<const N: usize>(text: &str) -> Result<[u8; N], DecodeError> {
    let digits = text.as_bytes();
    if digits.len() != N * 2 {
        return Err(DecodeError::Length {
            expected: N * 2,
            actual: digits.len(),
        });
    }
    let mut out = [0u8; N];
    for (slot, pair) in out.iter_mut().zip(digits.chunks_exact(2)) {
        *slot = (hex_value(pair[0])? << 4) | hex_value(pair[1])?;
    }
    Ok(out)
}

fn hex_value(digit: u8) -> Result<u8, DecodeError> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        _ => Err(DecodeError::Hex),
    }
}

/// `n` baytın base64 karakter sayısı (dolgu dahil).
#[must_use]
pub const fn base64_len(n: usize) -> usize {
    n.div_ceil(BASE64_GROUP_BYTES) * BASE64_GROUP_CHARS
}

/// RFC 4648 §4 base64 (standart alfabe, `=` dolgulu).
#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(base64_len(bytes.len()));
    for chunk in bytes.chunks(BASE64_GROUP_BYTES) {
        let mut padded = [0u8; BASE64_GROUP_BYTES];
        padded[..chunk.len()].copy_from_slice(chunk);
        let group = u32::from_be_bytes([0, padded[0], padded[1], padded[2]]);
        // `chunk.len() + 1` karakter veri taşır; dörtlünün kalanı dolgudur.
        for (position, shift) in BASE64_SEXTET_SHIFTS.into_iter().enumerate() {
            if position <= chunk.len() {
                let sextet = (group >> shift) & BASE64_SEXTET_MASK;
                out.push(char::from(BASE64_ALPHABET[sextet as usize]));
            } else {
                out.push(char::from(BASE64_PAD));
            }
        }
    }
    out
}

/// Tam `base64_len(N)` karakterlik kanonik base64'ü `N` bayta çözer.
///
/// # Errors
/// [`DecodeError::Length`] · [`DecodeError::Base64`] (alfabe dışı karakter, yanlış yerde dolgu,
/// sıfır olmayan kullanılmayan bitler).
pub fn base64_decode<const N: usize>(text: &str) -> Result<[u8; N], DecodeError> {
    let chars = text.as_bytes();
    let expected = base64_len(N);
    if chars.len() != expected {
        return Err(DecodeError::Length {
            expected,
            actual: chars.len(),
        });
    }
    let mut out = [0u8; N];
    for (index, quad) in chars.chunks_exact(BASE64_GROUP_CHARS).enumerate() {
        let start = index * BASE64_GROUP_BYTES;
        let take = (N - start).min(BASE64_GROUP_BYTES);
        let mut group = 0u32;
        for (position, &c) in quad.iter().enumerate() {
            let sextet = if position <= take {
                base64_value(c)?
            } else if c == BASE64_PAD {
                0
            } else {
                return Err(DecodeError::Base64);
            };
            group = (group << BASE64_SEXTET_BITS) | u32::from(sextet);
        }
        // `group` 24 bittir: be baytlarının ilki daima 0; ilk `take` bayttan sonrası sıfır olmalı
        // (kanonik kodlama).
        let decoded = group.to_be_bytes();
        if decoded[1 + take..].iter().any(|&b| b != 0) {
            return Err(DecodeError::Base64);
        }
        out[start..start + take].copy_from_slice(&decoded[1..=take]);
    }
    Ok(out)
}

fn base64_value(c: u8) -> Result<u8, DecodeError> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(DecodeError::Base64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_non_canonical_text() {
        let bytes = [0x00, 0x01, 0x7f, 0x80, 0xab, 0xff];
        let text = hex_encode(&bytes);
        assert_eq!(text, "00017f80abff");
        assert_eq!(hex_decode::<6>(&text), Ok(bytes));
        assert_eq!(hex_decode::<6>("00017F80ABFF"), Err(DecodeError::Hex));
        assert_eq!(
            hex_decode::<6>("00017f80abf"),
            Err(DecodeError::Length {
                expected: 12,
                actual: 11
            })
        );
        assert_eq!(hex_decode::<1>("0x"), Err(DecodeError::Hex));
        assert_eq!(hex_decode::<1>(" 0"), Err(DecodeError::Hex));
    }

    /// RFC 4648 §10 test vektörleri.
    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_decode::<0>(""), Ok([]));
        assert_eq!(base64_decode::<1>("Zg=="), Ok(*b"f"));
        assert_eq!(base64_decode::<2>("Zm8="), Ok(*b"fo"));
        assert_eq!(base64_decode::<3>("Zm9v"), Ok(*b"foo"));
        assert_eq!(base64_decode::<4>("Zm9vYg=="), Ok(*b"foob"));
        assert_eq!(base64_decode::<5>("Zm9vYmE="), Ok(*b"fooba"));
        assert_eq!(base64_decode::<6>("Zm9vYmFy"), Ok(*b"foobar"));
    }

    #[test]
    fn base64_rejects_non_canonical_and_malformed_text() {
        // "f" = "Zg=="; "Zh==" aynı baytı taşır ama kullanılmayan bitleri sıfır değil.
        assert_eq!(base64_decode::<1>("Zh=="), Err(DecodeError::Base64));
        // Dolgu yerinde veri; veri yerinde dolgu.
        assert_eq!(base64_decode::<1>("Zg=A"), Err(DecodeError::Base64));
        assert_eq!(base64_decode::<2>("Zm=="), Err(DecodeError::Base64));
        // Alfabe dışı (url-safe `-`).
        assert_eq!(base64_decode::<3>("Zm9-"), Err(DecodeError::Base64));
        // Dolgusuz ve satır sonlu metin uzunlukta düşer.
        assert_eq!(
            base64_decode::<1>("Zg"),
            Err(DecodeError::Length {
                expected: 4,
                actual: 2
            })
        );
        assert_eq!(
            base64_decode::<3>("Zm9v\n"),
            Err(DecodeError::Length {
                expected: 4,
                actual: 5
            })
        );
    }

    fn round_trip<const N: usize>() {
        let mut bytes = [0u8; N];
        let mut value = 11u8;
        for byte in &mut bytes {
            *byte = value;
            value = value.wrapping_mul(37).wrapping_add(11);
        }
        let text = base64_encode(&bytes);
        assert_eq!(text.len(), base64_len(N));
        assert_eq!(base64_decode::<N>(&text), Ok(bytes));
    }

    #[test]
    fn base64_round_trips_every_padding_class_and_the_certificate_size() {
        round_trip::<0>();
        round_trip::<31>();
        round_trip::<32>();
        round_trip::<33>();
        round_trip::<152>();
    }
}
