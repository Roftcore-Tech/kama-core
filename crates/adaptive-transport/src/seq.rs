//! TV-SEQ: u16 wire `seq` → 64-bit full-counter reconstruction (engine spec §2).
//!
//! QUIC packet-number decode (RFC 9000 Appendix A.3) 16 bit'e özelleştirilmiş hali:
//! `wire`'ın düşük bitlerini paylaşan adaylardan `expected`'e en yakını seçilir.
//! Deterministik (RNG yok, ADR-17); tam yarım-pencere (`hwin`) tie'ı ileri/yüksek
//! adaya çözülür.

/// SESSION_DATA `seq` wire'da u16 taşınır (TDD §2).
const SEQ_BITS: u32 = 16;

/// Truncated 16-bit `wire` seq'i, beklenen bir sonraki full counter `expected`
/// verildiğinde, 64-bit monoton full counter'a çözer.
///
/// ```
/// use adaptive_transport::seq::resolve_seq;
/// assert_eq!(resolve_seq(65536, 0xFFFF), 65535); // boundary-altı reorder
/// assert_eq!(resolve_seq(65535, 0x0001), 65537); // boundary-üstü ileri
/// ```
#[must_use]
pub fn resolve_seq(expected: u64, wire: u16) -> u64 {
    let win: u64 = 1u64 << SEQ_BITS; // 65536
    let hwin: u64 = win / 2; // 32768
    let mask: u64 = win - 1; // 0xFFFF
    let candidate = (expected & !mask) | u64::from(wire);

    if expected >= hwin && candidate <= expected - hwin && candidate <= u64::MAX - win {
        // candidate, expected'in yarım pencere kadar gerisinde → aynı wire ileri epoch'ta
        candidate + win
    } else if candidate >= win && candidate - hwin > expected {
        // candidate, expected'in yarım pencere kadar ilerisinde → geri sar
        candidate - win
    } else {
        candidate
    }
}
