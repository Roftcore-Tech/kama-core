//! TV-ACK: SACK feedback frame (engine spec §2), fixed-width big-endian (varint YASAK).
//!
//! Wire layout — V2 0x14 (EMIT, §2.2; 15 B sabit + 4 B × range_count):
//! ```text
//! frame_type u8=0x14 · ack_epoch u8 · largest_seq u16 · ack_delay u16(256µs) ·
//! block0_len u16 · range_count u8 · ranges[]{ gap u16, len u16 } ·
//! seen_count u32 · arrival_rate u16
//! ```
//! V1 0x10 (yalnız DECODE-uyumluluk; 9 B + 4 B × range): aynı baş, ack_delay µs, ölçüm eki YOK.
//! Descending reconstruction (engine §2):
//! ```text
//! cursor = largest_seq − block0_len + 1                 (block0'ın alt ucu)
//! her range: cursor −= gap;  block = [cursor−len+1 .. cursor];  cursor −= len
//! ```
//! u16 wrap: aritmetik mod 2^16 (wrapping); `ack_epoch` yalnız rekey sınırını
//! disambiguate eder — u16 sarması epoch değişimi DEĞİLDİR.

use std::time::Duration;

use crate::error::TransportError;
use crate::read::Reader;

/// Transport-control frame_type: ACK **V1** (engine spec §1.1) — `ack_delay` birimi **µs**.
/// [TUR-1x] DECODE-uyumluluk için tutulur; EMIT artık [`FRAME_TYPE_ACK_V2`] (µs u16 tavanı 65.5ms,
/// sub-kbps cadence-hold'unda anında satüre → RTT düzeltmesi etkisizdi).
pub const FRAME_TYPE_ACK: u8 = 0x10;
/// Transport-control frame_type: ACK **V2** (§2.2, 15B gövde) [TUR-1x, Seçenek-A]:
/// `ack_delay` birimi **256µs** (tavan 65535×256µs ≈ 16.78s > ACK_CADENCE_FLOOR 8.39s) +
/// ölçüm eki (`seen_count` u32, `arrival_rate` u16 — K3 cold-start yön-ayrımı).
pub const FRAME_TYPE_ACK_V2: u8 = 0x14;
/// V2 `ack_delay` birimi (µs) [STD: Seçenek-A kararı, time-model-v1 D-4/1].
pub const ACK_DELAY_UNIT_US: u64 = 256;

/// ACK range üst sınırı. `[CAL]` — netem baseline'dan (ADR-18) kalibre edilecek;
/// magic-number olmasın diye isimlendirildi (engine spec §2.1).
pub const MAX_ACK_RANGES: usize = 16;

/// [SR-AUDIT-001 / P3] Tek `acked_full` çağrısının döndürebileceği MAX seq sayısı (bounded transient).
/// 17 blok × u16-len yasal olarak ~1.1M üretebilir; gerçek inflight-penceresi bunun çok altında.
/// 65536 = bir tam u16-span: block0 tek başına asla aşamaz, yasal trafik yaklaşamaz, düşman `len`-
/// şişirmesi 512KB'ta durur (kümülatif SACK kaybı telafi eder). Bkz. `acked_full`.
pub const MAX_ACKED_SEQS: usize = 65536;

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
    /// RAW wire değeri — `largest_seq` alımı → bu ACK gönderimi (RTT düzeltmesi). Birim frame
    /// versiyonuna bağlı: V2 (0x14) → 256µs [TUR-1x], V1 (0x10) → µs. [`Self::ack_delay_duration`]
    /// ile çöz; raw değeri doğrudan µs sayma.
    pub ack_delay: u16,
    /// En üstteki ardışık-alınan blok uzunluğu (`largest_seq` dahil).
    pub block0_len: u16,
    /// Ek gap+blok çiftleri (descending); en çok [`MAX_ACK_RANGES`].
    pub ranges: Vec<AckRange>,
    /// [TUR-1x] wire versiyonu: `true` → 0x14 (ack_delay 256µs + V2 ölçüm eki), `false` → 0x10
    /// (µs, yalnız decode-uyumluluk).
    pub v2: bool,
    /// [§2.2 V2 ölçüm eki] Görülen toplam DATA datagram'ı (birim+kopya; saturating u32).
    /// Sender kendi gönderdiğiyle kıyaslar: seen≪sent = forward-loss, seen≈sent ama SACK yok =
    /// return-loss (K3 cold-start yön-ayrımı). V1 decode'da 0 (alan yok).
    pub seen_count: u32,
    /// [§2.2 V2 ölçüm eki] Receiver-gözlemli pencere-oranı (B/s). [`ARRIVAL_RATE_SENTINEL`] =
    /// ölçüm YOK ya da üst-bantta — cold-start ipucu olarak KULLANILMAZ. V1 decode'da sentinel.
    pub arrival_rate: u16,
}

/// [§2.2] `arrival_rate` sentinel: ölçüm yok / üst-bantta → ipucu olarak kullanma.
pub const ARRIVAL_RATE_SENTINEL: u16 = 0xFFFF;

impl AckFrame {
    /// Alınan full-counter seq kümesinden bir ACK kurar.
    ///
    /// `received_full` 64-bit monoton counter'lardır ([`crate::seq::resolve_seq`]
    /// ile çözülmüş). Gruplama full-counter uzayında yapılır → u16 wire sarması
    /// doğal olarak ele alınır; alanlar u16 wire değeri olarak yazılır. Range'ler
    /// en yüksek seq'ler korunacak şekilde [`MAX_ACK_RANGES`] ile clamp'lenir
    /// (engine spec §2.1). Boş girdide `None`.
    #[must_use]
    pub fn from_received(
        received_full: &[u64],
        ack_epoch: u8,
        ack_delay: u16,
        seen_count: u32,
        arrival_rate: u16,
    ) -> Option<Self> {
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
            v2: true, // [TUR-1x] emit daima V2 (0x14, 15B gövde §2.2); V1 yalnız decode-uyumluluk.
            seen_count,
            arrival_rate,
        })
    }

    /// [TUR-1x] `ack_delay` raw wire değerini süreye çözer (V2 → ×256µs, V1 → ×1µs).
    /// Floor-encoding V2'de encode-tarafındadır → decode şişirme yapmaz (delay ≤ gerçek).
    #[must_use]
    pub fn ack_delay_duration(&self) -> Duration {
        let unit_us = if self.v2 { ACK_DELAY_UNIT_US } else { 1 };
        Duration::from_micros(u64::from(self.ack_delay) * unit_us)
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
        acked.truncate(MAX_ACKED_SEQS); // block0 tek başına (≤65535) tavanı aşamaz; savunmacı.
                                        // alt bloklar (descending, §2): cursor −= gap; block = [cursor−len+1 .. cursor]; cursor −= len.
        for r in &self.ranges {
            cursor = cursor.saturating_sub(u64::from(r.gap));
            if r.len == 0 {
                continue;
            }
            let low = cursor.saturating_sub(u64::from(r.len) - 1);
            push_inclusive(&mut acked, low, cursor);
            cursor = cursor.saturating_sub(u64::from(r.len));
            // [SR-AUDIT-001 / P3 derinlemesine-savunma] toplam çıktı tavanı: range_count decode'da
            // ≤16 olsa da her `len` u16 (≤65535) → 17 blok yasal olarak ~1.1M seq üretebilir. Tek
            // wire ACK'inin sender'a taşıdığı YENİ ack sayısı inflight-penceresiyle sınırlıdır;
            // SACK KÜMÜLATİF olduğundan tavana çarpınca kesmek KAYIP DEĞİL (en yüksek/en yeni seq'ler
            // önce — block0 descending — konur; kesilen en-eski seq'ler sonraki ACK'te yeniden kapsanır,
            // bkz. bu fn'in doc'u). Düşman `len`-şişirmesini bounded transient'e indirger.
            if acked.len() >= MAX_ACKED_SEQS {
                acked.truncate(MAX_ACKED_SEQS);
                break;
            }
        }
        acked
    }

    /// Canonical fixed-width big-endian byte'lara serialize eder.
    /// V2 (0x14): 15B sabit + 4B×range (§2.2 ölçüm eki dahil); V1 (0x10): 9B + 4B×range.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(
            self.ranges.len() <= MAX_ACK_RANGES,
            "range sayısı MAX_ACK_RANGES'i aşıyor"
        );
        let mut out = Vec::with_capacity(15 + 4 * self.ranges.len());
        out.push(if self.v2 {
            FRAME_TYPE_ACK_V2
        } else {
            FRAME_TYPE_ACK
        });
        out.push(self.ack_epoch);
        out.extend_from_slice(&self.largest_seq.to_be_bytes());
        out.extend_from_slice(&self.ack_delay.to_be_bytes());
        out.extend_from_slice(&self.block0_len.to_be_bytes());
        out.push(self.ranges.len() as u8);
        for r in &self.ranges {
            out.extend_from_slice(&r.gap.to_be_bytes());
            out.extend_from_slice(&r.len.to_be_bytes());
        }
        if self.v2 {
            // §2.2 V2 ölçüm eki (fixed-width; opsiyonel alan YOK).
            out.extend_from_slice(&self.seen_count.to_be_bytes());
            out.extend_from_slice(&self.arrival_rate.to_be_bytes());
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
        // [TUR-1x] V2 (0x14, 256µs) VE V1 (0x10, µs — uyumluluk) kabul; layout aynı.
        if ft != FRAME_TYPE_ACK && ft != FRAME_TYPE_ACK_V2 {
            return Err(TransportError::UnknownFrameType(ft));
        }
        let ack_epoch = r.u8()?;
        let largest_seq = r.u16_be()?;
        let ack_delay = r.u16_be()?;
        let block0_len = r.u16_be()?;
        let range_count = usize::from(r.u8()?);
        // [SR-AUDIT-001 / P3 sertleştirme] range_count PROTOKOL tavanıyla sınırlanır. Encode ≤16
        // garanti eder (from_received clamp + encode debug_assert); doğrulanmamış range_count (u8→255)
        // acked_full'de ~16.7M-girişlik Vec<u64> (~134MB) amplifikasyonuna yol açardı (fail-closed,
        // decode reddi — with_capacity zaten yoktu ama asıl amplifikasyon acked_full'deydi).
        if range_count > MAX_ACK_RANGES {
            return Err(TransportError::TooManyAckRanges {
                count: range_count,
                max: MAX_ACK_RANGES,
            });
        }
        let mut ranges = Vec::new(); // count doğrulandı → yine de with_capacity yok (savunmacı)
        for _ in 0..range_count {
            let gap = r.u16_be()?;
            let len = r.u16_be()?;
            ranges.push(AckRange { gap, len });
        }
        let v2 = ft == FRAME_TYPE_ACK_V2;
        // §2.2 V2 ölçüm eki; V1'de alan yok → nötr değerler (0 / sentinel).
        let (seen_count, arrival_rate) = if v2 {
            (r.u32_be()?, r.u16_be()?)
        } else {
            (0, ARRIVAL_RATE_SENTINEL)
        };
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
            v2,
            seen_count,
            arrival_rate,
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

    /// [P3 SR-AUDIT-001] Düşman çerçeve: range_count > MAX_ACK_RANGES decode'da typed reddedilir
    /// (denetim bulgusu: doğrulanmamış range_count → acked_full ~134MB amplifikasyonu).
    #[test]
    fn decode_rejects_range_count_over_max() {
        // 255 range bildiren ama gövdesi kısa/uzun fark etmez — reddin range_count'ta olması esas.
        let mut buf = vec![FRAME_TYPE_ACK_V2, 0]; // ft, epoch
        buf.extend_from_slice(&0xFFFFu16.to_be_bytes()); // largest_seq
        buf.extend_from_slice(&0u16.to_be_bytes()); // ack_delay
        buf.extend_from_slice(&0xFFFFu16.to_be_bytes()); // block0_len
        buf.push(255); // range_count = 255 (> 16)
        buf.extend(std::iter::repeat_n(0u8, 255 * 4)); // 255 range gövdesi
        buf.extend_from_slice(&0u32.to_be_bytes()); // seen_count
        buf.extend_from_slice(&0u16.to_be_bytes()); // arrival_rate
        match AckFrame::decode(&buf) {
            Err(TransportError::TooManyAckRanges {
                count: 255,
                max: 16,
            }) => {}
            other => panic!("255-range çerçeve reddedilmeliydi: {other:?}"),
        }
        // Sınır: tam 16 range GEÇERLİ (encode'un ürettiği maksimum).
        let recv: Vec<u64> = (0..40).map(|i| i * 3).collect(); // 40 ayrık blok → 16'ya clamp
        let f = AckFrame::from_received(&recv, 0, 0, 0, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        assert!(f.ranges.len() <= MAX_ACK_RANGES);
        assert!(
            AckFrame::decode(&f.encode()).is_ok(),
            "16-range çerçeve geçerli olmalı"
        );
    }

    /// [P3 SR-AUDIT-001] `acked_full` çıktısı, geçerli range sayısında bile dev `len` alanlarıyla
    /// MAX_ACKED_SEQS'i aşamaz (derinlemesine savunma; `len`-şişirme amplifikasyonu).
    #[test]
    fn acked_full_is_bounded_against_len_inflation() {
        // 16 range, her biri gap=0 len=0xFFFF; block0_len=0xFFFF; büyük expected tabanı.
        let ranges = vec![
            AckRange {
                gap: 0,
                len: 0xFFFF
            };
            MAX_ACK_RANGES
        ];
        let f = AckFrame {
            ack_epoch: 0,
            largest_seq: 0xFFFF,
            ack_delay: 0,
            block0_len: 0xFFFF,
            ranges,
            v2: true,
            seen_count: 0,
            arrival_rate: 0,
        };
        let acked = f.acked_full(u64::MAX / 2); // büyük taban → tüm bloklar dolar
        assert!(
            acked.len() <= MAX_ACKED_SEQS,
            "acked_full tavanı aşmamalı: {} > {MAX_ACKED_SEQS}",
            acked.len()
        );
    }

    #[test]
    fn acked_full_roundtrips_from_received() {
        // Alınan set (gaps 8,9,11): from_received → encode → decode → acked_full == set.
        let recv = [5u64, 6, 7, 10, 12, 13];
        let f = AckFrame::from_received(&recv, 0, 100, 6, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        let bytes = f.encode();
        let f2 = AckFrame::decode(&bytes).expect("decode");
        let mut got = f2.acked_full(14); // expected = largest+1
        got.sort_unstable();
        assert_eq!(got, recv, "acked_full, from_received'in tersi olmalı (§2)");
    }

    #[test]
    fn acked_full_single_contiguous_block() {
        let recv = [100u64, 101, 102, 103];
        let f = AckFrame::from_received(&recv, 0, 0, 4, ARRIVAL_RATE_SENTINEL).expect("nonempty");
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
            v2: true,
            seen_count: 0,
            arrival_rate: ARRIVAL_RATE_SENTINEL,
        };
        assert!(f.acked_full(1).is_empty(), "block0_len 0 → boş");
    }

    #[test]
    fn ack_delay_unit_per_version() {
        // [TUR-1x] Aynı raw değer: V2 → ×256µs, V1 → ×1µs (decode-uyumluluk).
        let mut f =
            AckFrame::from_received(&[1], 0, 100, 1, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        assert!(f.v2, "emit daima V2");
        assert_eq!(f.ack_delay_duration(), Duration::from_micros(100 * 256));
        f.v2 = false;
        assert_eq!(f.ack_delay_duration(), Duration::from_micros(100));
    }

    #[test]
    fn decode_v1_frame_type_compat() {
        // V1 golden byte'ları (0x10, 9B gövde — ölçüm eki YOK) hâlâ decode edilir; µs semantiği.
        let mut bytes = AckFrame::from_received(&[5, 6, 7], 0, 40, 3, ARRIVAL_RATE_SENTINEL)
            .expect("nonempty")
            .encode();
        assert_eq!(bytes[0], FRAME_TYPE_ACK_V2, "emit V2");
        bytes[0] = FRAME_TYPE_ACK;
        bytes.truncate(9); // V1 gövdesi 9B — V2 ölçüm eki V1'de taşınmaz.
        let f = AckFrame::decode(&bytes).expect("V1 decode-uyumlu");
        assert!(!f.v2);
        assert_eq!(f.ack_delay_duration(), Duration::from_micros(40));
        assert_eq!(f.seen_count, 0, "V1'de alan yok → nötr");
        assert_eq!(f.arrival_rate, ARRIVAL_RATE_SENTINEL);
    }
}
