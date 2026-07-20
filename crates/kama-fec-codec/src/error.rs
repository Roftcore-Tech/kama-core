//! Typed FEC hataları (reachable todo!() YASAK — CLAUDE.md §6).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FecError {
    /// Profil-bound ihlali (K_shape/R_code_max/E; B4-invariant, §8).
    #[error("profile bound: {0}")]
    ProfileBound(&'static str),
    /// Encode'a verilen source sayısı K_shape ile uyuşmuyor (Model-A: zero-slot dahil K_shape).
    #[error("source count {got} != K_shape {expected}")]
    SourceCount { got: usize, expected: usize },
    /// symbol uzunluğu profile.E değil (§2.6 symbol_len-KİLİDİ).
    #[error("symbol length {got} != E {expected}")]
    SymbolLen { got: usize, expected: usize },
    /// Aynı ESI iki kez admitted (logical-dedup; K-FEC-RSPACE-1).
    #[error("duplicate ESI: {0}")]
    DuplicateEsi(u16),
    /// ESI profil-code-space dışında (source ≥ K_shape veya repair ≥ R_code_max).
    #[error("ESI {esi} out of range for {kind}")]
    EsiOutOfRange { esi: u16, kind: &'static str },
    /// Decode-koşulu sağlanmadı: admitted < K_shape (known_zero dahil).
    #[error(
        "insufficient shards: admitted {admitted} + known_zero {known_zero} < K_shape {k_shape}"
    )]
    Insufficient {
        admitted: usize,
        known_zero: usize,
        k_shape: usize,
    },
    /// reed-solomon-simd motor hatası (opak — RS-crate).
    #[error("rs engine: {0}")]
    RsEngine(String),
}
