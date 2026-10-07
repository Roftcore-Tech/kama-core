//! Enjekte edilebilir entropi kaynağı ([`EntropySource`]) — Crypto'nun ephemeral anahtar üretimi
//! için (S3 F0.3, 2026-10-06: trait `kama-noise-wire`'ın public API'sinde tanımlanır; Transport
//! enjekte eder). Bu, S3 seam'inin trait'i DEĞİLDİR.
//!
//! Üretimde [`OsEntropy`] (işletim sisteminin CSPRNG'i, `rand_core::OsRng`). Testte deterministik
//! [`FixedEntropy`] verilir: SSI1/SSR1 exact vektörleri ancak ephemeral anahtar sabitlenebilirse
//! byte-for-byte doğrulanabilir.
//!
//! Tanım `crates/client-core/src/seam.rs`'teki ile AYNIDIR (taşıma; client-core tarafının buraya
//! yönlenmesi ayrı şeridin işidir).

use rand_core::{OsRng, RngCore};

/// Entropi kaynağı tükendi / kullanılamıyor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("entropy source unavailable")]
pub struct EntropyError;

/// Crypto'nun ephemeral anahtar üretimi için ENJEKTE EDİLEBİLİR entropi kaynağı.
///
/// Üretimde OS kaynağıdır ([`OsEntropy`]). Testte deterministik kaynak verilir: SSI1/SSR1 exact
/// vektörleri (A2.3/A2.4) ancak ephemeral anahtar sabitlenebilirse byte-for-byte doğrulanabilir.
pub trait EntropySource: Send + 'static {
    /// `dst`'yi TAMAMEN doldurur ya da hata döner (kısmi doldurma YOK).
    ///
    /// # Errors
    /// [`EntropyError`] — kaynak tükendi.
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError>;
}

/// Üretim kaynağı: işletim sisteminin CSPRNG'i (`rand_core::OsRng` → `getrandom`).
///
/// Hata durumunda `dst` SIFIRLANIR: işletim sistemi çağrısı yarıda kalmış olabilir ve kısmen
/// doldurulmuş tampon, hatayı yok sayan bir çağıranın elinde zayıf anahtar olurdu.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsEntropy;

impl OsEntropy {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl EntropySource for OsEntropy {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        OsRng.try_fill_bytes(dst).map_err(|_| {
            dst.fill(0);
            EntropyError
        })
    }
}

/// Önceden verilmiş byte'ları sırayla döndüren deterministik kaynak — YALNIZ TEST.
///
/// Tükenince hata verir (sessizce sıfır ya da tekrar ÜRETMEZ: vektör testinde fazladan bir
/// ephemeral çekilirse bu görünür olmalı).
#[cfg(any(test, feature = "test-util"))]
#[derive(Debug, Clone)]
pub struct FixedEntropy {
    bytes: Vec<u8>,
    pos: usize,
}

#[cfg(any(test, feature = "test-util"))]
impl FixedEntropy {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Henüz tüketilmemiş bayt sayısı.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}

#[cfg(any(test, feature = "test-util"))]
impl EntropySource for FixedEntropy {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        if dst.len() > self.remaining() {
            return Err(EntropyError);
        }
        dst.copy_from_slice(&self.bytes[self.pos..self.pos + dst.len()]);
        self.pos += dst.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_entropy_yields_bytes_in_order_and_refuses_partial_fill() {
        let mut e = FixedEntropy::new((0u8..10).collect());
        let mut a = [0u8; 4];
        e.fill(&mut a).unwrap();
        assert_eq!(a, [0, 1, 2, 3]);
        let mut b = [0xEEu8; 8];
        assert_eq!(
            e.fill(&mut b),
            Err(EntropyError),
            "kalan 6 bayt 8'i dolduramaz"
        );
        assert_eq!(b, [0xEE; 8], "başarısız çağrı tamponu değiştirmez");
        assert_eq!(e.remaining(), 6, "başarısız çağrı bayt tüketmez");
        let mut c = [0u8; 6];
        e.fill(&mut c).unwrap();
        assert_eq!(c, [4, 5, 6, 7, 8, 9]);
        assert_eq!(
            e.fill(&mut [0u8; 1]),
            Err(EntropyError),
            "tükenince tekrar üretmez"
        );
    }

    #[test]
    fn os_entropy_fills_the_whole_buffer() {
        // 2^-256 olasılıkla yanlış alarm; sıfır dönen bir kaynak burada yakalanır.
        let mut k = [0u8; 32];
        OsEntropy::new().fill(&mut k).unwrap();
        assert_ne!(k, [0u8; 32]);
    }
}
