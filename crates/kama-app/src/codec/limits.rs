//! AMP sabitleri ve çalışma-zamanı limitleri (`specs/application-message-protocol.md` §2–§5, A2).
//!
//! Sabitlerin adları spec'teki adlardır. Alan genişliklerinden türeyen önek boyları
//! (`…_PREFIX_BYTES`, `…_BODY_BYTES`) §3 alan listelerinin toplamıdır; sayı elle yazılmaz, alan
//! genişliklerinden hesaplanır.
//!
//! `MAX_BODY_BYTES`, `MAX_DECOMPRESSED_MESSAGE` ve `MAX_FILE_CHUNK_BYTES` derleme-zamanı sabiti
//! DEĞİLDİR: `TransportCapabilities::max_plaintext_unit_bytes`'tan türetilirler (S1 K5 — değeri
//! Transport verir, Application yalnız formülü uygular). Bu yüzden [`AmpLimits`] içinde taşınırlar.

use core::fmt;

use kama_contracts::s1::TransportCapabilities;

const U8_BYTES: usize = core::mem::size_of::<u8>();
const U16_BYTES: usize = core::mem::size_of::<u16>();
const U32_BYTES: usize = core::mem::size_of::<u32>();
const U64_BYTES: usize = core::mem::size_of::<u64>();

/// §2 — `msg_version` v1.
pub const MSG_VERSION_V1: u8 = 0x01;

/// §2 — `msg_version 1 + msg_type 1 + flags 1 + body_len 2`.
pub const APP_RECORD_HEADER_BYTES: usize = U8_BYTES + U8_BYTES + U8_BYTES + U16_BYTES;

/// §2 — `flags` sıfır: sıkıştırma yok, reserved bitler sıfır.
pub const FLAGS_NONE: u8 = 0x00;

/// §2 — `flags.bit0 = compressed` (§5).
pub const FLAG_COMPRESSED: u8 = 0x01;

/// §2 — `flags` bit1..7 reserved-zero; herhangi biri set → REJECT.
pub const FLAGS_RESERVED_MASK: u8 = !FLAG_COMPRESSED;

/// §2 — reserved `msg_type` (alt uç): `0x00` → REJECT.
pub const MSG_TYPE_RESERVED_ZERO: u8 = 0x00;

/// §2 — reserved `msg_type` aralığının başı: `0xF0..=0xFF` → REJECT.
pub const MSG_TYPE_RESERVED_HIGH_START: u8 = 0xF0;

/// §5 — `compression_id 0x01 = zstd` (v1'de TEK algoritma; `0x00` yasak, diğer → REJECT).
pub const COMPRESSION_ID_ZSTD: u8 = 0x01;

/// §5 — sıkıştırılmış gövde öneki: `compression_id u8 | uncompressed_len u32_be`.
pub const COMPRESSION_PREFIX_BYTES: usize = U8_BYTES + U32_BYTES;

/// §5 kural 4 / A2 — `uncompressed_len / body_len` bu oranı aşarsa DROP. Karşılaştırma TAM SAYI
/// ÇARPIMIYLA yapılır (`uncompressed_len > MAX_EXPANSION_RATIO × body_len`); bölme 64 < oran < 65
/// aralığını yanlışlıkla kabul ederdi.
pub const MAX_EXPANSION_RATIO: u64 = 64;

/// §5 kural 5 / A2 — `body_len_uncompressed` bundan küçükse sıkıştırma YOK.
pub const COMPRESS_MIN_BYTES: usize = 64;

/// §3 0x02 — `whole_file_hash [32]` (BLAKE2s-256 çıktı boyu; codec hash HESAPLAMAZ — hesap
/// `crate::file_transfer::whole_file_hash`'te).
pub const WHOLE_FILE_HASH_BYTES: usize = 32;

/// §3 0x02 — `name_len u8` ⇒ `name` en çok 255 bayt (UTF-8).
pub const FILE_NAME_MAX_BYTES: usize = u8::MAX as usize;

/// §3 0x01 TEXT gövde öneki: `msg_id u32 · text_len u16` (spec: "gövde öneki 6 B").
pub const TEXT_PREFIX_BYTES: usize = U32_BYTES + U16_BYTES;

/// §3 0x02 FILE_BEGIN gövde öneki: `transfer_id u64 · file_size u64 · whole_file_hash [32] ·
/// chunk_count u32 · name_len u8`.
pub const FILE_BEGIN_PREFIX_BYTES: usize =
    U64_BYTES + U64_BYTES + WHOLE_FILE_HASH_BYTES + U32_BYTES + U8_BYTES;

/// §3 0x03 FILE_CHUNK gövde öneki: `transfer_id u64 · offset u64 · payload_len u16`
/// (spec: "18 B sabit önek"; A2 `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − 18`).
pub const FILE_CHUNK_PREFIX_BYTES: usize = U64_BYTES + U64_BYTES + U16_BYTES;

/// §3 0x04 FILE_END gövdesi: `transfer_id u64`.
pub const FILE_END_BODY_BYTES: usize = U64_BYTES;

/// §3 0x05 FILE_ABORT gövdesi: `transfer_id u64 · reason u8`.
pub const FILE_ABORT_BODY_BYTES: usize = U64_BYTES + U8_BYTES;

/// §3 0x06 PTT_OPEN gövdesi: `stream_id u32 · codec u8 · bundle_ms u8`.
pub const PTT_OPEN_BODY_BYTES: usize = U32_BYTES + U8_BYTES + U8_BYTES;

/// §3 0x07 PTT_CLOSE gövdesi: `stream_id u32`.
pub const PTT_CLOSE_BODY_BYTES: usize = U32_BYTES;

/// §3 0x08 RECEIPT gövdesi: `ref_msg_id u32 · kind u8`.
pub const RECEIPT_BODY_BYTES: usize = U32_BYTES + U8_BYTES;

/// §3 0x09 SIGNALING_BLOB gövde öneki: `blob_len u16`.
pub const SIGNALING_BLOB_PREFIX_BYTES: usize = U16_BYTES;

/// §3 0x0A MEDIA_BUNDLE gövde öneki: `stream_id u32 · codec u8 · bundle_ms u8 · payload_len u16`.
pub const MEDIA_BUNDLE_PREFIX_BYTES: usize = U32_BYTES + U8_BYTES + U8_BYTES + U16_BYTES;

/// [PTT-BANT · aday, kol A; sahip onayı 2026-11-22 "AMP biçim değişikliği test için"] Kompakt
/// `MEDIA_BUNDLE` kaydının ilk baytı — `msg_version` konumunda, v1 (`0x01`) değil: v1 alıcısı onu
/// başlığın geri kalanına bakmadan `unknown-msg-version` ile DROP eder (§6 ileri uyum; panik ve
/// sinyal yok). Kayıt: `0x8A · stream_tag u8 · payload` (2 B başlık; tam biçim 13 B). `codec` ve
/// `bundle_ms` akışın `PTT_OPEN`'ından, yük uzunluğu birim uzunluğundan gelir; `stream_tag` =
/// `stream_id`'nin düşük 8 biti.
pub const COMPACT_MEDIA_MARKER: u8 = 0x8A;

/// [PTT-BANT · kol A] Kompakt `MEDIA_BUNDLE` başlığı: işaret 1 + `stream_tag` 1.
pub const COMPACT_MEDIA_HEADER_BYTES: usize = U8_BYTES + U8_BYTES;

/// [PTT-BANT · aday, sahip onayı 2026-11-22 "AMP biçim değişikliği test için"] `bundle_ms u8`
/// alanının (`PTT_OPEN`, `MEDIA_BUNDLE`) kodlaması: kod ≤ bu değer ise demet süresi kodun kendisidir
/// (ms; bugünkü bütün değerler, vektörler dahil, bayt-aynı); üstündeki kodlar
/// [`BUNDLE_MS_COARSE_STEP`] adımlıdır. Bkz. [`bundle_ms_from_code`].
pub const BUNDLE_MS_EXACT_MAX: u8 = 200;

/// [PTT-BANT · aday] [`BUNDLE_MS_EXACT_MAX`]'ın üstündeki kodların adımı (ms): kod `c` →
/// `200 + (c − 200) × 10` ms; en büyük kod 255 → 750 ms.
pub const BUNDLE_MS_COARSE_STEP: u16 = 10;

/// [PTT-BANT · aday] `bundle_ms` kodu → demet süresi (ms). Kod ≤ 200 → aynen; üstü 10 ms adımlı
/// (201 → 210 ms … 222 → 420 ms … 255 → 750 ms). Eski alıcı kodu ms okur: motor yalnız eşitlik
/// denetler (etkilenmez), oynatma tamponu (varsayılan kapalı) 200 ms üstü demeti kod kadar ms
/// aralıkla yerleştirir.
#[must_use]
pub const fn bundle_ms_from_code(code: u8) -> u16 {
    if code <= BUNDLE_MS_EXACT_MAX {
        code as u16
    } else {
        BUNDLE_MS_EXACT_MAX as u16 + (code - BUNDLE_MS_EXACT_MAX) as u16 * BUNDLE_MS_COARSE_STEP
    }
}

/// [PTT-BANT · aday] Demet süresi (ms) → `bundle_ms` kodu; 200 ms'nin üstünde 10 ms'nin katı
/// olmayan ya da 750 ms'yi aşan süre → `None`.
#[must_use]
pub const fn bundle_code_from_ms(ms: u16) -> Option<u8> {
    let exact = BUNDLE_MS_EXACT_MAX as u16;
    if ms <= exact {
        return Some(ms as u8);
    }
    let over = ms - exact;
    if !over.is_multiple_of(BUNDLE_MS_COARSE_STEP) {
        return None;
    }
    let steps = over / BUNDLE_MS_COARSE_STEP;
    if steps > (u8::MAX - BUNDLE_MS_EXACT_MAX) as u16 {
        return None;
    }
    Some(BUNDLE_MS_EXACT_MAX + steps as u8)
}

/// §3 0x0B FILE_RECEIPT gövdesi (K4G-09; §4 A2 `FILE_RECEIPT_BODY_BYTES`): `transfer_id u64`
/// (`FILE_BEGIN.transfer_id` ile aynı genişlik ve kodlama) `· status u8` — sabit 9 B.
pub const FILE_RECEIPT_BODY_BYTES: usize = U64_BYTES + U8_BYTES;

/// A2 limitleri — `TransportCapabilities`'ten türetilmiş, salt-okunur.
///
/// * `MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes − APP_RECORD_HEADER_BYTES)`
/// * `MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES`
/// * `MAX_FILE_CHUNK_BYTES = MAX_BODY_BYTES − FILE_CHUNK_PREFIX_BYTES`
///
/// Çıkarmaların ikisi de denetlidir: taşan kapasite [`LimitsError`] verir, sessizce sıfıra
/// sabitlenmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmpLimits {
    max_body_bytes: usize,
    max_decompressed_message: usize,
    max_file_chunk_bytes: usize,
}

impl AmpLimits {
    /// A2 formüllerini `caps.max_plaintext_unit_bytes`'a uygular.
    ///
    /// # Errors
    ///
    /// * [`LimitsError::UnitTooSmallForHeader`] — birim kayıt başlığını (5 B) taşıyamıyor.
    /// * [`LimitsError::UnitTooSmallForFileChunk`] — `MAX_BODY_BYTES` FILE_CHUNK önekinden küçük.
    pub fn from_capabilities(caps: &TransportCapabilities) -> Result<Self, LimitsError> {
        let unit = caps.max_plaintext_unit_bytes;
        // u32 → usize yalnız 16-bit hedefte taşar; orada değer zaten 65535 tavanının üstündedir.
        let unit_bytes = usize::try_from(unit).unwrap_or(usize::MAX);
        let after_header = unit_bytes.checked_sub(APP_RECORD_HEADER_BYTES).ok_or(
            LimitsError::UnitTooSmallForHeader {
                max_plaintext_unit_bytes: unit,
            },
        )?;
        // `body_len` alanı u16'dır: u16'ya sığmayan değer 65535'e iner = min(65535, …).
        let max_body_bytes = usize::from(u16::try_from(after_header).unwrap_or(u16::MAX));
        let max_file_chunk_bytes = max_body_bytes.checked_sub(FILE_CHUNK_PREFIX_BYTES).ok_or(
            LimitsError::UnitTooSmallForFileChunk {
                max_plaintext_unit_bytes: unit,
            },
        )?;
        Ok(Self {
            max_body_bytes,
            max_decompressed_message: max_body_bytes,
            max_file_chunk_bytes,
        })
    }

    /// `MAX_BODY_BYTES` (A2).
    #[must_use]
    pub const fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }

    /// `MAX_DECOMPRESSED_MESSAGE` (A2; `= MAX_BODY_BYTES`).
    #[must_use]
    pub const fn max_decompressed_message(&self) -> usize {
        self.max_decompressed_message
    }

    /// `MAX_FILE_CHUNK_BYTES` (A2; `= MAX_BODY_BYTES − 18`).
    #[must_use]
    pub const fn max_file_chunk_bytes(&self) -> usize {
        self.max_file_chunk_bytes
    }
}

/// [`AmpLimits::from_capabilities`] hatası: Transport'un beyan ettiği birim AMP için çok küçük.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitsError {
    /// `max_plaintext_unit_bytes < APP_RECORD_HEADER_BYTES`.
    UnitTooSmallForHeader {
        /// Transport'un beyan ettiği değer.
        max_plaintext_unit_bytes: u32,
    },
    /// `MAX_BODY_BYTES < FILE_CHUNK_PREFIX_BYTES` (`MAX_FILE_CHUNK_BYTES` negatif olurdu).
    UnitTooSmallForFileChunk {
        /// Transport'un beyan ettiği değer.
        max_plaintext_unit_bytes: u32,
    },
}

impl fmt::Display for LimitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnitTooSmallForHeader {
                max_plaintext_unit_bytes,
            } => write!(
                f,
                "max_plaintext_unit_bytes {max_plaintext_unit_bytes} cannot carry the AMP record \
                 header"
            ),
            Self::UnitTooSmallForFileChunk {
                max_plaintext_unit_bytes,
            } => write!(
                f,
                "max_plaintext_unit_bytes {max_plaintext_unit_bytes} leaves MAX_BODY_BYTES below \
                 the FILE_CHUNK prefix"
            ),
        }
    }
}

impl std::error::Error for LimitsError {}
