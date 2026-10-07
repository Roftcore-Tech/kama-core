//! ccr §35.14 `BatchPayload` codec'i — Transport'un `seal()`'e verdiği plaintext birimin canonical
//! çerçevesi (KM-SEAM-TC-001 KARAR-1 = (a)). ADR-05: fixed-width big-endian, VARINT YOK.
//!
//! ```text
//! BatchPayload    { submsg_count : u8 ; submsg[i] : BatchSubMessage[submsg_count] }
//! BatchSubMessage { len_mode : u8
//!                     0x00..0xEF = inline length 0..239
//!                     0xF0       = extended u16_be length follows
//!                     0xF1..0xFF = reserved → reject
//!                   if len_mode == 0xF0: length : u16_be
//!                   payload : bytes[length] }
//! ```
//!
//! Kurallar (ccr §35.14 notu ve [K3 — 2026-10-04] amendment'ı):
//!
//! * **Boş batch geçerlidir:** `submsg_count = 0`, plaintext tam bir bayttır ([`EMPTY_BATCH`]).
//!   AEAD'den geçer ve PN tüketir; Application'a teslim YOKTUR. Ardından gelen her bayt reject.
//! * **Tek canonical encoding:** spec iki geçerli encoding'e izin vermez. Bu yüzden
//!   [`INLINE_LENGTH_MAX`]'a sığan bir uzunluğun `0xF0` ile (extended) yazılması reject edilir
//!   ([`BatchError::NonCanonicalLength`]); kodlayıcı her uzunluğu tek biçimde yazar.
//! * **Artık bayt reject:** son alt-mesajdan sonra bayt kalırsa batch reddedilir.
//! * **Sıfır uzunluklu alt-mesaj:** spec inline aralığını `0..239` diye tanımlar; `len_mode = 0x00`
//!   geçerli bir alt-mesajdır (boş `payload`). Bu katman payload içine bakmaz (AMP'nin işi).
//! * **Bounds-before-alloc (SR-AUDIT-001):** çözücü tahsisten ÖNCE `submsg_count`'u kalan bayta
//!   karşı sınar (alt-mesaj başına en az [`LEN_MODE_LEN`] bayt); her uzunluk, dilim alınmadan önce
//!   kalan bayta karşı sınanır. Çözücü payload kopyalamaz — girdiye ödünç dilim döndürür.
//!
//! Bu modül runtime'a BAĞLI DEĞİLDİR (L-D D1b'nin bağımsız parçası): interim `wire.rs` DATA
//! çerçevesinin yerine geçişi, ayrım ve sıra-kimliği kararlarını bekler.

use std::fmt;

/// `submsg_count` alanının genişliği (u8).
pub const SUBMSG_COUNT_LEN: usize = 1;
/// `submsg_count`'un taşıyabileceği en büyük değer (u8 tavanı).
pub const SUBMSG_COUNT_MAX: usize = u8::MAX as usize;
/// `len_mode` alanının genişliği (u8).
pub const LEN_MODE_LEN: usize = 1;
/// Inline uzunluk aralığının son `len_mode` değeri (`0x00..=0xEF` = uzunluğun kendisi).
pub const LEN_MODE_INLINE_MAX: u8 = 0xEF;
/// Extended uzunluk işareti: ardından `length : u16_be` gelir.
pub const LEN_MODE_EXTENDED: u8 = 0xF0;
/// Inline biçimle yazılabilen en büyük alt-mesaj uzunluğu (239).
pub const INLINE_LENGTH_MAX: usize = LEN_MODE_INLINE_MAX as usize;
/// Extended `length` alanının genişliği (u16_be).
pub const EXTENDED_LENGTH_LEN: usize = 2;
/// Extended biçimle yazılabilen en büyük alt-mesaj uzunluğu (u16 tavanı).
pub const EXTENDED_LENGTH_MAX: usize = u16::MAX as usize;
/// Boş batch'in `submsg_count` değeri.
pub const EMPTY_BATCH_SUBMSG_COUNT: u8 = 0;
/// Boş batch plaintext'i: tam bir bayt, `submsg_count = 0` (ccr §35.14 [K3 — 2026-10-04]).
pub const EMPTY_BATCH: [u8; SUBMSG_COUNT_LEN] = [EMPTY_BATCH_SUBMSG_COUNT];

/// `BatchPayload` kodlama/çözme hatası. Alanlar yalnız konum ve uzunluk taşır — payload baytı
/// hiçbir varyanta girmez (hassas veri Debug/log'a sızmaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BatchError {
    /// Girdi boş: `submsg_count` baytı yok.
    #[error("batch: boş girdi — submsg_count baytı yok")]
    MissingSubmsgCount,
    /// `submsg_count`, kalan bayta sığamaz (alt-mesaj başına en az bir `len_mode` baytı);
    /// tahsisten önce reddedilir.
    #[error(
        "batch: submsg_count {submsg_count} kalan {remaining} B'a sığmaz \
         (alt-mesaj başına ≥ {min} B len_mode)",
        min = LEN_MODE_LEN
    )]
    CountExceedsInput {
        /// Girdideki `submsg_count`.
        submsg_count: u8,
        /// `submsg_count`'tan sonra kalan bayt sayısı.
        remaining: usize,
    },
    /// Alt-mesaj kesik: `len_mode`, extended `length` ya da `payload` için bayt yetmiyor.
    #[error("batch: alt-mesaj {index} kesik — {need} B gerekli, {have} B var")]
    Truncated {
        /// Kesik alt-mesajın sırası (0 tabanlı).
        index: usize,
        /// Okunması gereken bayt sayısı.
        need: usize,
        /// Kalan bayt sayısı.
        have: usize,
    },
    /// `len_mode` reserved aralıkta (`0xF1..=0xFF`) → reject.
    #[error("batch: alt-mesaj {index} len_mode {len_mode:#04x} reserved → reject")]
    ReservedLenMode {
        /// Alt-mesajın sırası (0 tabanlı).
        index: usize,
        /// Okunan `len_mode`.
        len_mode: u8,
    },
    /// `0xF0` (extended) ile inline aralığa sığan bir uzunluk yazılmış: tek canonical encoding
    /// kuralı ihlali → reject.
    #[error(
        "batch: alt-mesaj {index} extended uzunluk {length} ≤ {max} — inline yazılmalı \
         (tek canonical encoding)",
        max = INLINE_LENGTH_MAX
    )]
    NonCanonicalLength {
        /// Alt-mesajın sırası (0 tabanlı).
        index: usize,
        /// Okunan extended `length`.
        length: u16,
    },
    /// Son alt-mesajdan sonra artık bayt var → reject.
    #[error("batch: {trailing} B artık bayt → reject")]
    TrailingBytes {
        /// Artık bayt sayısı.
        trailing: usize,
    },
    /// Kodlanacak alt-mesaj sayısı `submsg_count` tavanını aşıyor.
    #[error("batch: {count} alt-mesaj > submsg_count tavanı {max}", max = SUBMSG_COUNT_MAX)]
    TooManySubmsgs {
        /// İstenen alt-mesaj sayısı.
        count: usize,
    },
    /// Kodlanacak alt-mesaj extended uzunluk tavanını aşıyor.
    #[error(
        "batch: alt-mesaj {index} {length} B > extended uzunluk tavanı {max} B",
        max = EXTENDED_LENGTH_MAX
    )]
    SubmsgTooLong {
        /// Alt-mesajın sırası (0 tabanlı).
        index: usize,
        /// Alt-mesajın uzunluğu.
        length: usize,
    },
}

/// Çözülmüş `BatchPayload`: girdiye ödünç alt-mesaj dilimleri, tel sırasıyla.
///
/// `Debug` yalnız sayıyı ve uzunlukları gösterir; payload baytları gösterilmez.
#[derive(Clone, PartialEq, Eq)]
pub struct BatchPayload<'a> {
    submsgs: Vec<&'a [u8]>,
}

impl<'a> BatchPayload<'a> {
    /// Alt-mesajların `payload` dilimleri, tel sırasıyla.
    #[must_use]
    pub fn submsgs(&self) -> &[&'a [u8]] {
        &self.submsgs
    }

    /// Alt-mesaj sayısı (`submsg_count`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.submsgs.len()
    }

    /// `true` = boş batch (`submsg_count = 0`): geçerli, Application'a teslim YOK.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.submsgs.is_empty()
    }

    /// Alt-mesaj dilimlerini sahiplenerek döndürür.
    #[must_use]
    pub fn into_submsgs(self) -> Vec<&'a [u8]> {
        self.submsgs
    }
}

impl fmt::Debug for BatchPayload<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lens: Vec<usize> = self.submsgs.iter().map(|s| s.len()).collect();
        f.debug_struct("BatchPayload")
            .field("submsg_count", &self.submsgs.len())
            .field("submsg_lens", &lens)
            .finish_non_exhaustive()
    }
}

/// Bir alt-mesajın uzunluk başlığı: canonical biçim uzunluktan tek türlü seçilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LenHeader {
    /// `len_mode` = uzunluğun kendisi (`0..=INLINE_LENGTH_MAX`).
    Inline(u8),
    /// `len_mode = LEN_MODE_EXTENDED` + `length : u16_be` (`INLINE_LENGTH_MAX+1..=u16::MAX`).
    Extended(u16),
}

impl LenHeader {
    /// Canonical başlığı seçer; extended tavanı aşan uzunluk hata.
    fn for_len(index: usize, length: usize) -> Result<Self, BatchError> {
        match u8::try_from(length) {
            Ok(inline) if inline <= LEN_MODE_INLINE_MAX => Ok(Self::Inline(inline)),
            _ => u16::try_from(length)
                .map(Self::Extended)
                .map_err(|_| BatchError::SubmsgTooLong { index, length }),
        }
    }

    /// Başlığın teldeki bayt sayısı.
    fn wire_len(self) -> usize {
        match self {
            Self::Inline(_) => LEN_MODE_LEN,
            Self::Extended(_) => LEN_MODE_LEN + EXTENDED_LENGTH_LEN,
        }
    }

    fn write(self, out: &mut Vec<u8>) {
        match self {
            Self::Inline(len_mode) => out.push(len_mode),
            Self::Extended(length) => {
                out.push(LEN_MODE_EXTENDED);
                out.extend_from_slice(&length.to_be_bytes());
            }
        }
    }
}

/// Doğrulama: `submsg_count` baytı ve toplam tel uzunluğu. Tahsisten önce çağrılır.
///
/// Toplam en çok `1 + 255 × (3 + 65535)` bayttır; `usize` taşmaz.
fn plan<T: AsRef<[u8]>>(submsgs: &[T]) -> Result<(u8, usize), BatchError> {
    let count = u8::try_from(submsgs.len()).map_err(|_| BatchError::TooManySubmsgs {
        count: submsgs.len(),
    })?;
    let mut total = SUBMSG_COUNT_LEN;
    for (index, submsg) in submsgs.iter().enumerate() {
        let length = submsg.as_ref().len();
        total += LenHeader::for_len(index, length)?.wire_len() + length;
    }
    Ok((count, total))
}

/// Verilen alt-mesajlar için canonical `BatchPayload`'ın bayt uzunluğu (kodlamadan).
///
/// Boş liste → [`EMPTY_BATCH`] uzunluğu. Tavan aşımı [`encode`] ile aynı hatayı verir.
pub fn encoded_len<T: AsRef<[u8]>>(submsgs: &[T]) -> Result<usize, BatchError> {
    plan(submsgs).map(|(_, total)| total)
}

/// Alt-mesajları canonical `BatchPayload` olarak kodlar.
///
/// Her uzunluk tek biçimde yazılır: `≤ INLINE_LENGTH_MAX` inline, üstü `0xF0` + u16_be.
/// Bütün tavanlar tahsisten ÖNCE sınanır; hata durumunda kısmi çıktı üretilmez. Boş liste →
/// [`EMPTY_BATCH`] (tam bir bayt).
pub fn encode<T: AsRef<[u8]>>(submsgs: &[T]) -> Result<Vec<u8>, BatchError> {
    let (count, total) = plan(submsgs)?;
    let mut out = Vec::with_capacity(total);
    out.push(count);
    for (index, submsg) in submsgs.iter().enumerate() {
        let payload = submsg.as_ref();
        LenHeader::for_len(index, payload.len())?.write(&mut out);
        out.extend_from_slice(payload);
    }
    Ok(out)
}

/// Bir `BatchPayload` plaintext'ini çözer (AEAD doğrulamasından SONRA çağrılır).
///
/// Reject sınıfları: boş girdi, sayının kalan bayta sığmaması, kesik alt-mesaj, reserved
/// `len_mode` (`0xF1..=0xFF`), canonical olmayan extended uzunluk, artık bayt. Ürün yolunda
/// panik yok.
pub fn decode(buf: &[u8]) -> Result<BatchPayload<'_>, BatchError> {
    let Some((&submsg_count, mut rest)) = buf.split_first() else {
        return Err(BatchError::MissingSubmsgCount);
    };
    let count = usize::from(submsg_count);
    // Bounds-before-alloc: her alt-mesaj en az bir `len_mode` baytı ister.
    if count.saturating_mul(LEN_MODE_LEN) > rest.len() {
        return Err(BatchError::CountExceedsInput {
            submsg_count,
            remaining: rest.len(),
        });
    }
    let mut submsgs = Vec::with_capacity(count);
    for index in 0..count {
        let Some((&len_mode, after_mode)) = rest.split_first() else {
            return Err(BatchError::Truncated {
                index,
                need: LEN_MODE_LEN,
                have: 0,
            });
        };
        let (length, after_len) = match len_mode {
            0..=LEN_MODE_INLINE_MAX => (usize::from(len_mode), after_mode),
            LEN_MODE_EXTENDED => {
                let Some((length_be, tail)) = after_mode.split_first_chunk::<EXTENDED_LENGTH_LEN>()
                else {
                    return Err(BatchError::Truncated {
                        index,
                        need: EXTENDED_LENGTH_LEN,
                        have: after_mode.len(),
                    });
                };
                let length = u16::from_be_bytes(*length_be);
                if usize::from(length) <= INLINE_LENGTH_MAX {
                    return Err(BatchError::NonCanonicalLength { index, length });
                }
                (usize::from(length), tail)
            }
            _ => return Err(BatchError::ReservedLenMode { index, len_mode }),
        };
        let Some((payload, tail)) = after_len.split_at_checked(length) else {
            return Err(BatchError::Truncated {
                index,
                need: length,
                have: after_len.len(),
            });
        };
        submsgs.push(payload);
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(BatchError::TrailingBytes {
            trailing: rest.len(),
        });
    }
    Ok(BatchPayload { submsgs })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testlerde payload içeriği: konuma bağlı, deterministik (beklenen bayt değil, girdi).
    fn payload(len: usize, seed: u8) -> Vec<u8> {
        (0..len)
            .map(|i| seed.wrapping_add(u8::try_from(i % 251).unwrap()))
            .collect()
    }

    fn roundtrip(submsgs: &[Vec<u8>]) -> Vec<u8> {
        let wire = encode(submsgs).unwrap();
        assert_eq!(encoded_len(submsgs).unwrap(), wire.len());
        let decoded = decode(&wire).unwrap();
        assert_eq!(decoded.len(), submsgs.len());
        for (got, want) in decoded.submsgs().iter().zip(submsgs) {
            assert_eq!(*got, want.as_slice());
        }
        wire
    }

    #[test]
    fn empty_batch_is_exactly_one_byte_and_delivers_nothing() {
        let none: [&[u8]; 0] = [];
        let wire = encode(&none).unwrap();
        assert_eq!(wire, EMPTY_BATCH);
        assert_eq!(wire.len(), SUBMSG_COUNT_LEN);
        assert_eq!(encoded_len(&none).unwrap(), SUBMSG_COUNT_LEN);
        let decoded = decode(&EMPTY_BATCH).unwrap();
        assert!(decoded.is_empty());
        assert_eq!(decoded.len(), 0);
        assert!(decoded.into_submsgs().is_empty());
    }

    #[test]
    fn empty_batch_with_trailing_byte_is_rejected() {
        for extra in [EMPTY_BATCH_SUBMSG_COUNT, LEN_MODE_EXTENDED, u8::MAX] {
            let mut wire = EMPTY_BATCH.to_vec();
            wire.push(extra);
            // submsg_count = 0 + fazladan bayt: canonical boş batch tam 1 bayttır.
            assert_eq!(
                decode(&wire),
                Err(BatchError::TrailingBytes { trailing: 1 })
            );
        }
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(decode(&[]), Err(BatchError::MissingSubmsgCount));
    }

    #[test]
    fn single_submsg_roundtrip_inline() {
        let msgs = vec![payload(1, 7)];
        let wire = roundtrip(&msgs);
        assert_eq!(wire.len(), SUBMSG_COUNT_LEN + LEN_MODE_LEN + 1);
        assert_eq!(usize::from(wire[0]), msgs.len());
        assert_eq!(usize::from(wire[SUBMSG_COUNT_LEN]), msgs[0].len());
    }

    #[test]
    fn zero_length_submsg_is_valid_inline_zero() {
        let msgs = vec![Vec::new()];
        let wire = roundtrip(&msgs);
        assert_eq!(wire.len(), SUBMSG_COUNT_LEN + LEN_MODE_LEN);
        let decoded = decode(&wire).unwrap();
        assert_eq!(decoded.len(), 1);
        assert!(decoded.submsgs()[0].is_empty());
    }

    #[test]
    fn inline_upper_bound_uses_one_byte_header() {
        let msgs = vec![payload(INLINE_LENGTH_MAX, 1)];
        let wire = roundtrip(&msgs);
        assert_eq!(wire[SUBMSG_COUNT_LEN], LEN_MODE_INLINE_MAX);
        assert_eq!(
            wire.len(),
            SUBMSG_COUNT_LEN + LEN_MODE_LEN + INLINE_LENGTH_MAX
        );
    }

    #[test]
    fn first_extended_length_uses_u16_be_header() {
        let len = INLINE_LENGTH_MAX + 1;
        let msgs = vec![payload(len, 2)];
        let wire = roundtrip(&msgs);
        assert_eq!(wire[SUBMSG_COUNT_LEN], LEN_MODE_EXTENDED);
        let at = SUBMSG_COUNT_LEN + LEN_MODE_LEN;
        let got = u16::from_be_bytes([wire[at], wire[at + 1]]);
        assert_eq!(usize::from(got), len);
        assert_eq!(wire.len(), at + EXTENDED_LENGTH_LEN + len);
    }

    #[test]
    fn extended_upper_bound_roundtrips_and_one_more_is_rejected() {
        let msgs = vec![payload(EXTENDED_LENGTH_MAX, 3)];
        let wire = roundtrip(&msgs);
        assert_eq!(
            wire.len(),
            SUBMSG_COUNT_LEN + LEN_MODE_LEN + EXTENDED_LENGTH_LEN + EXTENDED_LENGTH_MAX
        );
        let too_long = vec![Vec::new(), payload(EXTENDED_LENGTH_MAX + 1, 3)];
        let want = BatchError::SubmsgTooLong {
            index: 1,
            length: EXTENDED_LENGTH_MAX + 1,
        };
        assert_eq!(encode(&too_long), Err(want));
        assert_eq!(encoded_len(&too_long), Err(want));
    }

    #[test]
    fn many_submsgs_mixed_lengths_roundtrip() {
        let lens = [0, 1, INLINE_LENGTH_MAX, INLINE_LENGTH_MAX + 1, 1000, 17];
        let msgs: Vec<Vec<u8>> = lens
            .iter()
            .zip(0u8..)
            .map(|(&len, seed)| payload(len, seed))
            .collect();
        let wire = roundtrip(&msgs);
        let headers: usize = lens
            .iter()
            .map(|&len| {
                if len <= INLINE_LENGTH_MAX {
                    LEN_MODE_LEN
                } else {
                    LEN_MODE_LEN + EXTENDED_LENGTH_LEN
                }
            })
            .sum();
        assert_eq!(
            wire.len(),
            SUBMSG_COUNT_LEN + headers + lens.iter().sum::<usize>()
        );
    }

    #[test]
    fn submsg_count_max_roundtrips_and_one_more_is_rejected() {
        let msgs: Vec<Vec<u8>> = (0..SUBMSG_COUNT_MAX)
            .map(|i| payload(i % 3, u8::try_from(i).unwrap()))
            .collect();
        let wire = roundtrip(&msgs);
        assert_eq!(usize::from(wire[0]), SUBMSG_COUNT_MAX);

        let over: Vec<Vec<u8>> = vec![Vec::new(); SUBMSG_COUNT_MAX + 1];
        let want = BatchError::TooManySubmsgs {
            count: SUBMSG_COUNT_MAX + 1,
        };
        assert_eq!(encode(&over), Err(want));
        assert_eq!(encoded_len(&over), Err(want));
    }

    #[test]
    fn every_reserved_len_mode_is_rejected() {
        for len_mode in (LEN_MODE_EXTENDED + 1)..=u8::MAX {
            // Reserved bayttan sonra bol bayt olsa bile reject (uzunluk okunmaz).
            let mut wire = vec![1, len_mode];
            wire.extend_from_slice(&payload(EXTENDED_LENGTH_LEN + 4, 0));
            assert_eq!(
                decode(&wire),
                Err(BatchError::ReservedLenMode { index: 0, len_mode })
            );
        }
    }

    #[test]
    fn reserved_len_mode_after_valid_submsg_reports_its_index() {
        let mut wire = encode(&[payload(2, 9)]).unwrap();
        wire[0] = 2;
        wire.push(u8::MAX);
        assert_eq!(
            decode(&wire),
            Err(BatchError::ReservedLenMode {
                index: 1,
                len_mode: u8::MAX,
            })
        );
    }

    #[test]
    fn extended_form_for_inline_length_is_non_canonical() {
        let lengths: [u16; 2] = [0, u16::from(LEN_MODE_INLINE_MAX)];
        for length in lengths {
            let mut wire = vec![1, LEN_MODE_EXTENDED];
            wire.extend_from_slice(&length.to_be_bytes());
            wire.extend_from_slice(&payload(usize::from(length), 4));
            assert_eq!(
                decode(&wire),
                Err(BatchError::NonCanonicalLength { index: 0, length })
            );
        }
    }

    #[test]
    fn count_larger_than_remaining_bytes_is_rejected_before_alloc() {
        let wire = [u8::MAX];
        assert_eq!(
            decode(&wire),
            Err(BatchError::CountExceedsInput {
                submsg_count: u8::MAX,
                remaining: 0,
            })
        );
        let wire = [2, 0];
        assert_eq!(
            decode(&wire),
            Err(BatchError::CountExceedsInput {
                submsg_count: 2,
                remaining: 1,
            })
        );
    }

    #[test]
    fn truncated_len_mode_is_rejected() {
        // İki alt-mesaj ilan edilmiş; ilki kalan bütün baytı yiyor → ikincinin len_mode'u yok.
        let mut wire = encode(&[payload(3, 5)]).unwrap();
        wire[0] = 2;
        assert_eq!(
            decode(&wire),
            Err(BatchError::Truncated {
                index: 1,
                need: LEN_MODE_LEN,
                have: 0,
            })
        );
    }

    #[test]
    fn truncated_extended_length_is_rejected() {
        let wire = [1, LEN_MODE_EXTENDED, 1];
        assert_eq!(
            decode(&wire),
            Err(BatchError::Truncated {
                index: 0,
                need: EXTENDED_LENGTH_LEN,
                have: 1,
            })
        );
    }

    #[test]
    fn truncated_payload_is_rejected() {
        let wire = encode(&[payload(5, 6)]).unwrap();
        let cut = &wire[..wire.len() - 3];
        assert_eq!(
            decode(cut),
            Err(BatchError::Truncated {
                index: 0,
                need: 5,
                have: 2,
            })
        );
    }

    #[test]
    fn every_strict_prefix_of_a_valid_batch_is_rejected() {
        let msgs = vec![
            payload(0, 0),
            payload(INLINE_LENGTH_MAX + 1, 1),
            payload(2, 2),
        ];
        let wire = roundtrip(&msgs);
        for cut in 0..wire.len() {
            assert!(decode(&wire[..cut]).is_err(), "prefix {cut} kabul edildi");
        }
    }

    #[test]
    fn trailing_bytes_after_last_submsg_are_rejected() {
        let mut wire = encode(&[payload(4, 8), payload(INLINE_LENGTH_MAX + 2, 8)]).unwrap();
        wire.extend_from_slice(&[EMPTY_BATCH_SUBMSG_COUNT; 3]);
        assert_eq!(
            decode(&wire),
            Err(BatchError::TrailingBytes { trailing: 3 })
        );
    }

    #[test]
    fn leb128_length_attempt_is_rejected() {
        // ccr §35.14 INVALID: batch subframe LEB128 varint → reject. 300 = LEB128 [0xAC, 0x02];
        // fixed-width okuyucu 0xAC'ı inline uzunluk 172 sayar → geri kalan artık bayt olur.
        let length: usize = 300;
        let leb128 = [0xAC, 0x02];
        let mut wire = vec![1];
        wire.extend_from_slice(&leb128);
        wire.extend_from_slice(&payload(length, 0));
        let inline_len = usize::from(leb128[0]);
        let trailing = leb128.len() - LEN_MODE_LEN + length - inline_len;
        assert_eq!(decode(&wire), Err(BatchError::TrailingBytes { trailing }));
    }

    #[test]
    fn debug_does_not_print_payload_bytes() {
        let secret = vec![0xA5; 8];
        let wire = encode(&[secret.as_slice()]).unwrap();
        let decoded = decode(&wire).unwrap();
        let shown = format!("{decoded:?}");
        assert!(shown.contains("submsg_count: 1"), "{shown}");
        assert!(
            !shown.contains("165"),
            "payload baytı Debug'a sızdı: {shown}"
        );
    }

    #[test]
    fn encode_accepts_borrowed_and_owned_submsgs() {
        let owned = vec![payload(3, 1), payload(INLINE_LENGTH_MAX + 1, 2)];
        let borrowed: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
        assert_eq!(encode(&owned).unwrap(), encode(&borrowed).unwrap());
    }
}
