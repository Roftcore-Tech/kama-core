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

/// Bounds-checked byte okuyucu (PARSER-02).
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], TransportError> {
        if self.remaining() < n {
            return Err(TransportError::Truncated {
                offset: self.pos,
                needed: n - self.remaining(),
            });
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, TransportError> {
        Ok(self.take(1)?[0])
    }

    fn u16_be(&mut self) -> Result<u16, TransportError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
}
