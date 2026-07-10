//! Bench typed hataları (thiserror). reachable `todo!()` YOK — kama transport stub
//! bile [`BenchError::FeatureNotImplemented`] döner, panik değil.

use thiserror::Error;

/// Bench uygulamasının hata tipi.
#[derive(Debug, Error)]
pub enum BenchError {
    /// IO hatası (dosya, soket).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// JSONL serileştirme.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// QUIC (quinn) tarafı — detay string'e sarılır (typed BenchError kalır).
    #[error("quic: {0}")]
    Quic(String),
    /// TLS / cert kurulumu.
    #[error("tls: {0}")]
    Tls(String),
    /// Henüz uygulanmadı (kama transport stub) — typed, asla panik.
    #[error("feature not implemented: {0}")]
    FeatureNotImplemented(&'static str),
    /// Diğer.
    #[error("{0}")]
    Other(String),
}
