//! Transport katmanı typed hataları (thiserror).
//!
//! Reachable `todo!()` YASAK (CLAUDE.md §6); tamamlanmamış özellikler panik yerine
//! [`TransportError::FeatureNotImplemented`] döner.

use thiserror::Error;

/// Transport codec / engine tarafından yüzeye çıkan hatalar.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    /// Fixed-width bir alan okunmadan buffer bitti (PARSER-02: bounds-before-read).
    #[error("truncated frame: offset {offset} için {needed} byte eksik")]
    Truncated {
        /// Okumanın denendiği ofset.
        offset: usize,
        /// Kaç byte daha gerekiyordu.
        needed: usize,
    },

    /// Tam bir frame çözüldükten sonra artan byte var (canonical form trailing'i reddeder).
    #[error("frame sonrası trailing byte: {count}")]
    TrailingBytes {
        /// Artan byte sayısı.
        count: usize,
    },

    /// Transport-control frame için bilinmeyen `frame_type` byte'ı.
    #[error("bilinmeyen transport-control frame_type 0x{0:02x}")]
    UnknownFrameType(u8),

    /// Bir özellik tanımlı ama henüz uygulanmadı (typed stub, asla panik değil).
    #[error("feature not implemented: {0}")]
    FeatureNotImplemented(&'static str),
}
