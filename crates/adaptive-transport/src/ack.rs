//! TV-ACK: SACK feedback frame (engine spec §2), fixed-width big-endian (varint YASAK).
//!
//! Wire layout (9 B sabit + 4 B × range_count):
//! ```text
//! frame_type u8=0x10 · ack_epoch u8 · largest_seq u16 · ack_delay u16 ·
//! block0_len u16 · range_count u8 · ranges[]{ gap u16, len u16 }
//! ```
//! Descending reconstruction (engine §2):
//! ```text
//! cursor = largest_seq − block0_len + 1                 (block0'ın alt ucu)
//! her range: cursor −= gap;  block = [cursor−len+1 .. cursor];  cursor −= len
//! ```
//! u16 wrap: aritmetik mod 2^16 (wrapping); `ack_epoch` yalnız rekey sınırını
//! disambiguate eder — u16 sarması epoch değişimi DEĞİLDİR.

use crate::error::TransportError;
use crate::read::Reader;

/// Transport-control frame_type: ACK (engine spec §1.1).
pub const FRAME_TYPE_ACK: u8 = 0x10;

/// ACK range üst sınırı. `[CAL]` — netem baseline'dan (ADR-18) kalibre edilecek;
/// magic-number olmasın diye isimlendirildi (engine spec §2.1).
pub const MAX_ACK_RANGES: usize = 16;

/// Önceki bloğun altındaki ek (gap, len) acknowledgement bloğu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckRange {
    /// Bir önceki bloğun altındaki kayıp aralık ölçüsü (u16, wire mod 2^16).
    pub gap: u16,
    /// Ardışık-alınan blok uzunluğu.
    pub len: u16,
}

/// u16 hop-seq uzayında selective-acknowledgement frame'i.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckFrame {
    /// Rekey / counter-wrap epoch'u (`largest_seq` bu epoch'ta).
    pub ack_epoch: u8,
    /// Alınan en yüksek seq (bu epoch, u16 wire).
    pub largest_seq: u16,
    /// µs — `largest_seq` alımı → bu ACK gönderimi (RTT düzeltmesi).
    pub ack_delay: u16,
    /// En üstteki ardışık-alınan blok uzunluğu (`largest_seq` dahil).
    pub block0_len: u16,
    /// Ek gap+blok çiftleri (descending); en çok [`MAX_ACK_RANGES`].
    pub ranges: Vec<AckRange>,
}

impl AckFrame {
    /// Alınan full-counter seq kümesinden bir ACK kurar.
    ///
    /// `received_full` 64-bit monoton counter'lardır ([`crate::seq::resolve_seq`]
    /// ile çözülmüş). Gruplama full-counter uzayında yapılır → u16 wire sarması
    /// doğal olarak ele alınır; alanlar u16 wire değeri olarak yazılır. Range'ler
    /// en yüksek seq'ler korunacak şekilde [`MAX_ACK_RANGES`] ile clamp'lenir
    /// (engine spec §2.1). Boş girdide `None`.
    #[must_use]
    pub fn from_received(received_full: &[u64], ack_epoch: u8, ack_delay: u16) -> Option<Self> {
        if received_full.is_empty() {
            return None;
        }
        let mut v: Vec<u64> = received_full.to_vec();
        v.sort_unstable();
        v.dedup();

        // Full-counter uzayında contiguous ascending bloklar [low, high].
        let mut blocks: Vec<(u64, u64)> = Vec::new();
        let mut low = v[0];
        let mut high = v[0];
        for &x in &v[1..] {
            if x == high + 1 {
                high = x;
            } else {
                blocks.push((low, high));
                low = x;
                high = x;
            }
        }
        blocks.push((low, high));

        // En yüksek blok = block0.
        let (b0_low, b0_high) = *blocks.last().expect("blocks boş olamaz (v boş değil)");
        let largest_seq = (b0_high & 0xFFFF) as u16;
        let block0_len = block_len_u16(b0_low, b0_high);

        // Alt blokları descending yay; en yüksek seq'leri koruyarak clamp'le.
        let mut ranges = Vec::new();
        let mut cursor = b0_low; // yayılmış son bloğun full-counter alt ucu
        for &(low, high) in blocks.iter().rev().skip(1) {
            if ranges.len() == MAX_ACK_RANGES {
                break;
            }
            let gap = (cursor - high) as u16; // full-uzay farkı; wire'da mod 2^16
            let len = block_len_u16(low, high);
            ranges.push(AckRange { gap, len });
            cursor = low.saturating_sub(1);
        }

        Some(AckFrame {
            ack_epoch,
            largest_seq,
            ack_delay,
            block0_len,
            ranges,
        })
    }

    /// Acknowledge edilen full-counter seq kümesini rekonstrükte eder — [`Self::from_received`]'in
    /// TERSİ (§2 descending). `expected` = beklenen bir sonraki full counter; `largest_seq` (u16 wire)
    /// bununla [`crate::seq::resolve_seq`] üzerinden full'e kaldırılır, alt bloklar full-uzayda türetilir.
    ///
    /// Sender'ın `OnAckReceived` (§3.2) wire ACK'ten çözülmüş `acked_seqs`'i buradan alır. TAM'dır ancak
    /// encode'da hiçbir gap/blok 2^16'yı aşmadıysa (`from_received` gap'i `as u16` truncate, len'i clamp
    /// eder — normal çalışmada aşılmaz). `ranges` [`MAX_ACK_RANGES`] ile sınırlı → çok-eski bloklar bu
    /// ACK'te olmayabilir (SACK kümülatif: sonraki ACK'ler en yüksek seq'leri tekrar kapsar).
    #[must_use]
    pub fn acked_full(&self, expected: u64) -> Vec<u64> {
        let mut acked = Vec::new();
        if self.block0_len == 0 {
            return acked;
        }
        let largest_full = crate::seq::resolve_seq(expected, self.largest_seq);
        // block0 = [largest_full − block0_len + 1 ..= largest_full]
        let mut cursor = largest_full.saturating_sub(u64::from(self.block0_len) - 1);
        push_inclusive(&mut acked, cursor, largest_full);
        // alt bloklar (descending, §2): cursor −= gap; block = [cursor−len+1 .. cursor]; cursor −= len.
        for r in &self.ranges {
            cursor = cursor.saturating_sub(u64::from(r.gap));
            if r.len == 0 {
                continue;
            }
            let low = cursor.saturating_sub(u64::from(r.len) - 1);
            push_inclusive(&mut acked, low, cursor);
            cursor = cursor.saturating_sub(u64::from(r.len));
        }
        acked
    }

    /// Canonical fixed-width big-endian byte'lara serialize eder.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(
            self.ranges.len() <= MAX_ACK_RANGES,
            "range sayısı MAX_ACK_RANGES'i aşıyor"
        );
        let mut out = Vec::with_capacity(9 + 4 * self.ranges.len());
        out.push(FRAME_TYPE_ACK);
        out.push(self.ack_epoch);
        out.extend_from_slice(&self.largest_seq.to_be_bytes());
        out.extend_from_slice(&self.ack_delay.to_be_bytes());
        out.extend_from_slice(&self.block0_len.to_be_bytes());
        out.push(self.ranges.len() as u8);
        for r in &self.ranges {
            out.extend_from_slice(&r.gap.to_be_bytes());
            out.extend_from_slice(&r.len.to_be_bytes());
        }
        out
    }

    /// Canonical byte'ları çözer; truncated ve trailing girdiyi reddeder
    /// (PARSER-02: bounds-before-read, untrusted len ile with_capacity yok).
    ///
    /// # Errors
    /// Buffer kısa ([`TransportError::Truncated`]), frame_type yanlış
    /// ([`TransportError::UnknownFrameType`]) veya sonda artan byte varsa
    /// ([`TransportError::TrailingBytes`]).
    pub fn decode(buf: &[u8]) -> Result<Self, TransportError> {
        let mut r = Reader::new(buf);
        let ft = r.u8()?;
        if ft != FRAME_TYPE_ACK {
            return Err(TransportError::UnknownFrameType(ft));
        }
        let ack_epoch = r.u8()?;
        let largest_seq = r.u16_be()?;
        let ack_delay = r.u16_be()?;
        let block0_len = r.u16_be()?;
        let range_count = usize::from(r.u8()?);
        let mut ranges = Vec::new(); // count untrusted → with_capacity yok (PARSER-02)
        for _ in 0..range_count {
            let gap = r.u16_be()?;
            let len = r.u16_be()?;
            ranges.push(AckRange { gap, len });
        }
        if r.remaining() != 0 {
            return Err(TransportError::TrailingBytes {
                count: r.remaining(),
            });
        }
        Ok(AckFrame {
            ack_epoch,
            largest_seq,
            ack_delay,
            block0_len,
            ranges,
        })
    }
}

/// Full-uzay blok uzunluğunu u16'ya clamp'ler (degenerate >2^16 blok savunması).
fn block_len_u16(low: u64, high: u64) -> u16 {
    (high - low + 1).min(u64::from(u16::MAX)) as u16
}

/// [low, high] kapsayıcı full-counter aralığını çıkışa ekler.
fn push_inclusive(out: &mut Vec<u64>, low: u64, high: u64) {
    for s in low..=high {
        out.push(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acked_full_roundtrips_from_received() {
        // Alınan set (gaps 8,9,11): from_received → encode → decode → acked_full == set.
        let recv = [5u64, 6, 7, 10, 12, 13];
        let f = AckFrame::from_received(&recv, 0, 100).expect("nonempty");
        let bytes = f.encode();
        let f2 = AckFrame::decode(&bytes).expect("decode");
        let mut got = f2.acked_full(14); // expected = largest+1
        got.sort_unstable();
        assert_eq!(got, recv, "acked_full, from_received'in tersi olmalı (§2)");
    }

    #[test]
    fn acked_full_single_contiguous_block() {
        let recv = [100u64, 101, 102, 103];
        let f = AckFrame::from_received(&recv, 0, 0).expect("nonempty");
        assert!(f.ranges.is_empty(), "tek blok → range yok");
        let mut got = f.acked_full(104);
        got.sort_unstable();
        assert_eq!(got, recv);
    }

    #[test]
    fn acked_full_empty_on_zero_block0() {
        let f = AckFrame {
            ack_epoch: 0,
            largest_seq: 0,
            ack_delay: 0,
            block0_len: 0,
            ranges: Vec::new(),
        };
        assert!(f.acked_full(1).is_empty(), "block0_len 0 → boş");
    }
}
