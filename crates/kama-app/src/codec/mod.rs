//! AMP codec (B2.1) — `specs/application-message-protocol.md` §2–§6.
//!
//! [`encode`] bir [`AppMessage`]'ı tek bir `AppMessageRecord`'a (S1 [`PreparedUnit`]) çevirir;
//! [`decode`] Transport'un teslim ettiği birimi (`Delivered.unit`) mesaja çevirir ya da kaydı
//! [`DecodeError`] ile atar. Kayıt `max_plaintext_unit_bytes`'ı aşmaz: sınırlar [`AmpLimits`]
//! içinde `TransportCapabilities`'ten türetilir (A2) ve her çağrıya verilir.
//!
//! # Alıcı davranışı
//!
//! [`DecodeError::Drop`] ve [`DecodeError::Reject`] alıcıda aynı gözlemlenebilir sonucu verir:
//! kayıt atılır, karşı tarafa sinyal gitmez, retry yoktur (§5 kural 3, LAW-5). Ayrım teşhis
//! içindir; [`DecodeError::reason`] ret vektörlerinin (`vectors/amp-record-reject-v1.json`)
//! `reason` dizgesini AYNEN döndürür.
//!
//! # Sıkıştırma
//!
//! Bugün gönderici sıkıştırmaz ve alıcı sıkıştırılmış kaydı açamaz (zstd bağımlılığı kararı
//! bekliyor; `compress.rs` modül belgesi). Sıkıştırılmış kayıtta beyan edilen uzunluklara bakan
//! §5 denetimleri yine de uygulanır.
//!
//! # Gizlilik
//!
//! `text`, `name`, `payload`, `blob` ve `whole_file_hash` içeren [`AppMessage`]'ın `Debug`
//! çıktısı bu alanların içeriğini değil yalnız uzunluğunu yazar.

mod body;
mod compress;
mod header;
pub mod limits;

use core::fmt;

use kama_contracts::s1::{CompressionPolicy, PreparedUnit};

pub use header::MsgType;
pub use limits::{AmpLimits, LimitsError};

use limits::{FLAGS_NONE, FLAG_COMPRESSED, WHOLE_FILE_HASH_BYTES};

/// §3 0x05 `FILE_ABORT.reason`. Bilinmeyen değer → DROP (`unknown-enum-value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FileAbortReason {
    /// 0x01 `user_cancel`.
    UserCancel = 0x01,
    /// 0x02 `hash_mismatch`.
    HashMismatch = 0x02,
    /// 0x03 `storage_full`.
    StorageFull = 0x03,
    /// 0x04 `timeout`.
    Timeout = 0x04,
}

impl FileAbortReason {
    /// Registry'deki bütün değerler.
    pub const ALL: [Self; 4] = [
        Self::UserCancel,
        Self::HashMismatch,
        Self::StorageFull,
        Self::Timeout,
    ];

    /// Teldeki kod.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Kayıtlı koddan değer; bilinmeyen → `None`.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.code() == code)
    }
}

/// §3 0x06 `PTT_OPEN.codec`. Bilinmeyen değer → DROP (`unknown-enum-value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PttCodec {
    /// 0x01 opus.
    Opus = 0x01,
    /// 0x02 lyra.
    Lyra = 0x02,
}

impl PttCodec {
    /// Registry'deki bütün değerler.
    pub const ALL: [Self; 2] = [Self::Opus, Self::Lyra];

    /// Teldeki kod.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Kayıtlı koddan değer; bilinmeyen → `None`.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.code() == code)
    }
}

/// §3 0x08 `RECEIPT.kind`. Bilinmeyen değer → DROP (`unknown-enum-value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReceiptKind {
    /// 0x01 `delivered` — uygulama record'u işledi (A4: M3a).
    Delivered = 0x01,
    /// 0x02 `read` (A4: M3b).
    Read = 0x02,
}

impl ReceiptKind {
    /// Registry'deki bütün değerler.
    pub const ALL: [Self; 2] = [Self::Delivered, Self::Read];

    /// Teldeki kod.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Kayıtlı koddan değer; bilinmeyen → `None`.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.code() == code)
    }
}

/// §3 — bir `AppMessageRecord`'un çözülmüş hâli. Alan adları spec'tekilerle birebir; uzunluk
/// alanları (`text_len`, `name_len`, `payload_len`, `blob_len`) taşınan içerikten türetilir.
#[derive(Clone, PartialEq, Eq)]
pub enum AppMessage {
    /// 0x01 `TEXT`.
    Text {
        /// Gönderen tarafında oturum-scope monoton (§3 `msg_id` semantiği).
        msg_id: u32,
        /// UTF-8 metin (`text_len u16`).
        text: String,
    },
    /// 0x02 `FILE_BEGIN`.
    FileBegin {
        /// Aktarım kimliği.
        transfer_id: u64,
        /// Dosya boyu (bayt).
        file_size: u64,
        /// BLAKE2s-256 bütün-dosya özeti — bu crate'te HESAPLANMAZ, opak taşınır.
        whole_file_hash: [u8; WHOLE_FILE_HASH_BYTES],
        /// Parça sayısı.
        chunk_count: u32,
        /// UTF-8 dosya adı, en çok 255 bayt (`name_len u8`).
        name: String,
    },
    /// 0x03 `FILE_CHUNK`.
    FileChunk {
        /// Aktarım kimliği.
        transfer_id: u64,
        /// Parçanın dosyadaki konumu.
        offset: u64,
        /// Parça baytları (`payload_len u16`; en çok `MAX_FILE_CHUNK_BYTES`).
        payload: Vec<u8>,
    },
    /// 0x04 `FILE_END`.
    FileEnd {
        /// Aktarım kimliği.
        transfer_id: u64,
    },
    /// 0x05 `FILE_ABORT`.
    FileAbort {
        /// Aktarım kimliği.
        transfer_id: u64,
        /// İptal nedeni.
        reason: FileAbortReason,
    },
    /// 0x06 `PTT_OPEN`.
    PttOpen {
        /// Konuşma akışı kimliği.
        stream_id: u32,
        /// Ses kodlayıcısı (talk-spurt boyunca sabit).
        codec: PttCodec,
        /// Demet süresi (ms; talk-spurt boyunca sabit).
        bundle_ms: u8,
    },
    /// 0x07 `PTT_CLOSE`.
    PttClose {
        /// Konuşma akışı kimliği.
        stream_id: u32,
    },
    /// 0x08 `RECEIPT`.
    Receipt {
        /// Onaylanan `TEXT.msg_id`.
        ref_msg_id: u32,
        /// Onay türü.
        kind: ReceiptKind,
    },
    /// 0x09 `SIGNALING_BLOB` — opak; core okumaz, parse etmez.
    SignalingBlob {
        /// Opak signaling baytları (`blob_len u16`).
        blob: Vec<u8>,
    },
    /// 0x0A `MEDIA_BUNDLE` — kap; yük içi yerleşim media ekibinin. `codec` burada yorumlanmaz.
    MediaBundle {
        /// Konuşma akışı kimliği.
        stream_id: u32,
        /// Kodlayıcı kodu (opak `u8`; bu belge yalnız kabı verir).
        codec: u8,
        /// Demet süresi (ms).
        bundle_ms: u8,
        /// Opak medya yükü (`payload_len u16`).
        payload: Vec<u8>,
    },
}

impl AppMessage {
    /// Kaydın `msg_type`'ı.
    #[must_use]
    pub const fn msg_type(&self) -> MsgType {
        match self {
            Self::Text { .. } => MsgType::Text,
            Self::FileBegin { .. } => MsgType::FileBegin,
            Self::FileChunk { .. } => MsgType::FileChunk,
            Self::FileEnd { .. } => MsgType::FileEnd,
            Self::FileAbort { .. } => MsgType::FileAbort,
            Self::PttOpen { .. } => MsgType::PttOpen,
            Self::PttClose { .. } => MsgType::PttClose,
            Self::Receipt { .. } => MsgType::Receipt,
            Self::SignalingBlob { .. } => MsgType::SignalingBlob,
            Self::MediaBundle { .. } => MsgType::MediaBundle,
        }
    }
}

/// İçerik alanları (metin, ad, yük, özet) YAZILMAZ; yalnız uzunlukları.
impl fmt::Debug for AppMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text { msg_id, text } => f
                .debug_struct("Text")
                .field("msg_id", msg_id)
                .field("text_len", &text.len())
                .finish_non_exhaustive(),
            Self::FileBegin {
                transfer_id,
                file_size,
                chunk_count,
                name,
                ..
            } => f
                .debug_struct("FileBegin")
                .field("transfer_id", transfer_id)
                .field("file_size", file_size)
                .field("chunk_count", chunk_count)
                .field("name_len", &name.len())
                .finish_non_exhaustive(),
            Self::FileChunk {
                transfer_id,
                offset,
                payload,
            } => f
                .debug_struct("FileChunk")
                .field("transfer_id", transfer_id)
                .field("offset", offset)
                .field("payload_len", &payload.len())
                .finish_non_exhaustive(),
            Self::FileEnd { transfer_id } => f
                .debug_struct("FileEnd")
                .field("transfer_id", transfer_id)
                .finish(),
            Self::FileAbort {
                transfer_id,
                reason,
            } => f
                .debug_struct("FileAbort")
                .field("transfer_id", transfer_id)
                .field("reason", reason)
                .finish(),
            Self::PttOpen {
                stream_id,
                codec,
                bundle_ms,
            } => f
                .debug_struct("PttOpen")
                .field("stream_id", stream_id)
                .field("codec", codec)
                .field("bundle_ms", bundle_ms)
                .finish(),
            Self::PttClose { stream_id } => f
                .debug_struct("PttClose")
                .field("stream_id", stream_id)
                .finish(),
            Self::Receipt { ref_msg_id, kind } => f
                .debug_struct("Receipt")
                .field("ref_msg_id", ref_msg_id)
                .field("kind", kind)
                .finish(),
            Self::SignalingBlob { blob } => f
                .debug_struct("SignalingBlob")
                .field("blob_len", &blob.len())
                .finish_non_exhaustive(),
            Self::MediaBundle {
                stream_id,
                codec,
                bundle_ms,
                payload,
            } => f
                .debug_struct("MediaBundle")
                .field("stream_id", stream_id)
                .field("codec", codec)
                .field("bundle_ms", bundle_ms)
                .field("payload_len", &payload.len())
                .finish_non_exhaustive(),
        }
    }
}

/// Kayıt sessizce atılır (sinyal yok). Token'lar [`DropReason::reason`]'da.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropReason {
    /// Kayıt boş ya da 5 B'lık başlıktan kısa.
    RecordTooShort,
    /// `msg_version` bilinmiyor (§2, §6 — ileri uyum; reject değil).
    UnknownMsgVersion,
    /// Kayıt uzunluğu `5 + body_len` değil.
    RecordLenMismatch,
    /// Kayıtlı olmayan, reserved da olmayan `msg_type` (§6 — ileri uyum).
    UnknownMsgType,
    /// A1 — sabit önek ya da değişken alan gövdeye sığmıyor.
    InnerFieldExceedsBodyLen,
    /// Gövde tüm alanlar okunduktan sonra artık bayt taşıyor.
    TrailingBodyBytes,
    /// §5 kural 1 — `uncompressed_len > MAX_DECOMPRESSED_MESSAGE`.
    UncompressedLenExceedsMaxDecompressed,
    /// §5 kural 4 — `uncompressed_len > MAX_EXPANSION_RATIO × body_len`.
    ExpansionRatioExceeded,
    /// §5 kural 3 — geçersiz ya da kesik zstd karesi.
    InvalidZstdFrame,
    /// §5 kural 2 — açılan çıktı `uncompressed_len`'den fazla ya da eksik.
    DecompressedLenMismatch,
    /// zstd çözücüsü bu derlemede yok (bağımlılık kararı bekliyor).
    CompressionUnavailable,
    /// `TEXT.text` ya da `FILE_BEGIN.name` geçerli UTF-8 değil.
    InvalidUtf8,
    /// `FILE_ABORT.reason`, `PTT_OPEN.codec` ya da `RECEIPT.kind` registry'de yok.
    UnknownEnumValue,
}

impl DropReason {
    /// Kararlı gerekçe dizgesi (ret vektörlerinin `reason` alanı).
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::RecordTooShort => "record-too-short",
            Self::UnknownMsgVersion => "unknown-msg-version",
            Self::RecordLenMismatch => "record-len-mismatch",
            Self::UnknownMsgType => "unknown-msg-type",
            Self::InnerFieldExceedsBodyLen => "inner-field-exceeds-body-len",
            Self::TrailingBodyBytes => "trailing-body-bytes",
            Self::UncompressedLenExceedsMaxDecompressed => {
                "uncompressed-len-exceeds-max-decompressed"
            }
            Self::ExpansionRatioExceeded => "expansion-ratio-exceeded",
            Self::InvalidZstdFrame => "invalid-zstd-frame",
            Self::DecompressedLenMismatch => "decompressed-len-mismatch",
            Self::CompressionUnavailable => "zstd-decoder-unavailable",
            Self::InvalidUtf8 => "invalid-utf8",
            Self::UnknownEnumValue => "unknown-enum-value",
        }
    }
}

/// Kayıt tanımlı biçimde geçersiz (reserved alan / tanımsız sıkıştırma). Alıcıda gözlemlenebilir
/// sonuç DROP ile aynıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RejectReason {
    /// `msg_type` `0x00` ya da `0xF0..=0xFF`.
    ReservedMsgType,
    /// `flags` bit1..7'den biri set.
    ReservedFlagsNonzero,
    /// `compression_id` `0x01` değil.
    UnknownCompressionId,
}

impl RejectReason {
    /// Kararlı gerekçe dizgesi (ret vektörlerinin `reason` alanı).
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::ReservedMsgType => "reserved-msg-type",
            Self::ReservedFlagsNonzero => "reserved-flags-nonzero",
            Self::UnknownCompressionId => "unknown-compression-id",
        }
    }
}

/// [`decode`] hatası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodeError {
    /// Sessiz atım.
    Drop(DropReason),
    /// Tanımlı ret.
    Reject(RejectReason),
}

impl DecodeError {
    /// Kararlı gerekçe dizgesi — ret vektörlerinin `reason` alanıyla AYNEN aynı.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::Drop(r) => r.reason(),
            Self::Reject(r) => r.reason(),
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verdict = match self {
            Self::Drop(_) => "dropped",
            Self::Reject(_) => "rejected",
        };
        write!(f, "AMP record {verdict}: {}", self.reason())
    }
}

impl std::error::Error for DecodeError {}

/// [`encode`] hatası — mesaj bir kayda sığmıyor; Application içeriği bölmelidir (S1 K3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// Gövde `MAX_BODY_BYTES`'ı aşıyor.
    BodyTooLarge {
        /// Kodlanacak gövde uzunluğu (taşmada `usize::MAX`).
        body_len: usize,
        /// Bu limitlerdeki `MAX_BODY_BYTES`.
        max_body_bytes: usize,
    },
    /// `FILE_BEGIN.name` 255 baytı aşıyor (`name_len u8`).
    NameTooLong {
        /// Adın bayt uzunluğu.
        name_len: usize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BodyTooLarge {
                body_len,
                max_body_bytes,
            } => write!(
                f,
                "AMP body of {body_len} bytes exceeds MAX_BODY_BYTES {max_body_bytes}"
            ),
            Self::NameTooLong { name_len } => {
                write!(
                    f,
                    "FILE_BEGIN name of {name_len} bytes exceeds the u8 length field"
                )
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Mesaj → `AppMessageRecord` (§2). Kayıt `5 + body_len ≤ max_plaintext_unit_bytes` sağlar.
///
/// Sıkıştırma kararı §5 gönderici kurallarıyla burada alınır; bugün hiçbir tip sıkıştırılmaz
/// (`compression_policy = Never`, `flags.bit0 = 0`).
///
/// # Errors
///
/// [`EncodeError`] — gövde `MAX_BODY_BYTES`'ı ya da `FILE_BEGIN.name` 255 baytı aşıyor.
pub fn encode(message: &AppMessage, limits: &AmpLimits) -> Result<PreparedUnit, EncodeError> {
    let msg_type = message.msg_type();
    let plain = body::encode_body(message, limits)?;
    let (flags, record_body, compression_policy) =
        match compress::compress_for_send(msg_type, &plain) {
            Some(packed) => (FLAG_COMPRESSED, packed, CompressionPolicy::AppCompressed),
            None => (FLAGS_NONE, plain, CompressionPolicy::Never),
        };
    let bytes = header::write_record(msg_type, flags, &record_body)?;
    Ok(PreparedUnit {
        bytes,
        compression_policy,
    })
}

/// `AppMessageRecord` → mesaj. Denetim sırası: başlık (`header.rs`), sıkıştırma (§5 alıcı
/// kuralları, `compress.rs`), tip gövdesi (A1, `body.rs`).
///
/// # Errors
///
/// [`DecodeError`] — kayıt atılır; gerekçe [`DecodeError::reason`].
pub fn decode(record: &[u8], limits: &AmpLimits) -> Result<AppMessage, DecodeError> {
    let parsed = header::parse_record(record)?;
    if parsed.compressed {
        let plain = compress::open_compressed(parsed.body, limits)?;
        body::decode_body(parsed.msg_type, &plain)
    } else {
        body::decode_body(parsed.msg_type, parsed.body)
    }
}

#[cfg(test)]
mod tests {
    //! Vektörsüz hükümler (ret dosyasında vakası olmayanlar) ve sınır davranışları. Beklenen
    //! bayt YOKTUR: girdiler geçerli bir kaydın kodlanıp adlandırılmış konumlarda bozulmasıyla
    //! kurulur, beklenen yalnız karardır.

    use super::header::{BODY_LEN_OFFSET, FLAGS_OFFSET, MSG_TYPE_OFFSET};
    use super::limits::{
        APP_RECORD_HEADER_BYTES, COMPRESSION_ID_ZSTD, COMPRESSION_PREFIX_BYTES, COMPRESS_MIN_BYTES,
        FILE_CHUNK_PREFIX_BYTES, FILE_NAME_MAX_BYTES, MAX_EXPANSION_RATIO,
        MSG_TYPE_RESERVED_HIGH_START, MSG_VERSION_V1, TEXT_PREFIX_BYTES,
    };
    use super::*;
    use kama_contracts::s1::TransportCapabilities;

    /// Test parametresi (normatif değil): vektörlerin üretildiği değerle aynı tutuldu.
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
    /// UTF-8'de hiçbir konumda geçerli olmayan bayt.
    const INVALID_UTF8_BYTE: u8 = 0xFF;
    /// Sıkıştırılmış test kaydının gövdesindeki yük uzunluğu (önek hariç).
    const TEST_COMPRESSED_PAYLOAD_LEN: usize = 5;

    fn caps(max_plaintext_unit_bytes: u32) -> TransportCapabilities {
        TransportCapabilities {
            max_plaintext_unit_bytes,
            preferred_plaintext_unit_bytes: max_plaintext_unit_bytes,
            max_early_unit_bytes: max_plaintext_unit_bytes,
            reliable: true,
        }
    }

    fn limits() -> AmpLimits {
        AmpLimits::from_capabilities(&caps(TEST_MAX_PLAINTEXT_UNIT_BYTES))
            .expect("test limitleri geçerli")
    }

    fn record(message: &AppMessage) -> Vec<u8> {
        encode(message, &limits()).expect("kodlanabilir").bytes
    }

    fn drop_of(record: &[u8]) -> DropReason {
        match decode(record, &limits()) {
            Err(DecodeError::Drop(reason)) => reason,
            other => panic!("DROP bekleniyordu, gelen {other:?}"),
        }
    }

    fn reject_of(record: &[u8]) -> RejectReason {
        match decode(record, &limits()) {
            Err(DecodeError::Reject(reason)) => reason,
            other => panic!("REJECT bekleniyordu, gelen {other:?}"),
        }
    }

    fn set_body_len(record: &mut [u8], body_len: usize) {
        let field = u16::try_from(body_len).expect("test gövdesi u16'ya sığar");
        record[BODY_LEN_OFFSET..APP_RECORD_HEADER_BYTES].copy_from_slice(&field.to_be_bytes());
    }

    /// `flags.bit0 = 1` kaydı: önek + `TEST_COMPRESSED_PAYLOAD_LEN` sıfır bayt (gerçek kare
    /// gerekmez; denetimler yalnız beyan edilen uzunluklara bakar).
    fn compressed_record(compression_id: u8, uncompressed_len: u32) -> Vec<u8> {
        let mut body = vec![compression_id];
        body.extend_from_slice(&uncompressed_len.to_be_bytes());
        body.resize(COMPRESSION_PREFIX_BYTES + TEST_COMPRESSED_PAYLOAD_LEN, 0);
        let mut rec = vec![MSG_VERSION_V1, MsgType::Text.code(), FLAG_COMPRESSED, 0, 0];
        set_body_len(&mut rec, body.len());
        rec.extend_from_slice(&body);
        rec
    }

    fn sample_messages() -> Vec<AppMessage> {
        vec![
            AppMessage::Text {
                msg_id: u32::MAX,
                text: "çok baytlı ünicode ✓".to_owned(),
            },
            AppMessage::Text {
                msg_id: 0,
                text: String::new(),
            },
            AppMessage::FileBegin {
                transfer_id: u64::MAX,
                file_size: 0,
                whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
                chunk_count: 0,
                name: "ş".repeat(FILE_NAME_MAX_BYTES / 2),
            },
            AppMessage::FileChunk {
                transfer_id: 1,
                offset: u64::MAX,
                payload: Vec::new(),
            },
            AppMessage::FileEnd { transfer_id: 2 },
            AppMessage::FileAbort {
                transfer_id: 3,
                reason: FileAbortReason::Timeout,
            },
            AppMessage::PttOpen {
                stream_id: 4,
                codec: PttCodec::Lyra,
                bundle_ms: u8::MAX,
            },
            AppMessage::PttClose { stream_id: 5 },
            AppMessage::Receipt {
                ref_msg_id: 6,
                kind: ReceiptKind::Read,
            },
            AppMessage::SignalingBlob { blob: Vec::new() },
            AppMessage::MediaBundle {
                stream_id: 7,
                codec: u8::MAX,
                bundle_ms: 0,
                payload: vec![0; 3],
            },
        ]
    }

    #[test]
    fn every_variant_round_trips() {
        let messages = sample_messages();
        let mut seen: Vec<MsgType> = messages.iter().map(AppMessage::msg_type).collect();
        seen.sort_by_key(|t| t.code());
        seen.dedup();
        assert_eq!(seen, MsgType::ALL, "her tip en az bir kez sınanmalı");
        for message in messages {
            let unit = encode(&message, &limits()).expect("kodlanabilir");
            assert_eq!(unit.compression_policy, CompressionPolicy::Never);
            assert_eq!(unit.bytes[FLAGS_OFFSET], FLAGS_NONE);
            assert_eq!(decode(&unit.bytes, &limits()), Ok(message));
        }
    }

    #[test]
    fn limits_follow_a2_formulas() {
        let l = limits();
        let unit = usize::try_from(TEST_MAX_PLAINTEXT_UNIT_BYTES).unwrap();
        assert_eq!(l.max_body_bytes(), unit - APP_RECORD_HEADER_BYTES);
        assert_eq!(l.max_decompressed_message(), l.max_body_bytes());
        assert_eq!(
            l.max_file_chunk_bytes(),
            l.max_body_bytes() - FILE_CHUNK_PREFIX_BYTES
        );

        let huge = AmpLimits::from_capabilities(&caps(u32::MAX)).unwrap();
        assert_eq!(huge.max_body_bytes(), usize::from(u16::MAX));

        let header_only = u32::try_from(APP_RECORD_HEADER_BYTES).unwrap();
        assert_eq!(
            AmpLimits::from_capabilities(&caps(header_only - 1)),
            Err(LimitsError::UnitTooSmallForHeader {
                max_plaintext_unit_bytes: header_only - 1
            })
        );
        let no_chunk = header_only + u32::try_from(FILE_CHUNK_PREFIX_BYTES).unwrap() - 1;
        assert_eq!(
            AmpLimits::from_capabilities(&caps(no_chunk)),
            Err(LimitsError::UnitTooSmallForFileChunk {
                max_plaintext_unit_bytes: no_chunk
            })
        );
        let exact_chunk = no_chunk + 1;
        let tight = AmpLimits::from_capabilities(&caps(exact_chunk)).unwrap();
        assert_eq!(tight.max_file_chunk_bytes(), 0);
    }

    #[test]
    fn encode_rejects_body_over_max_and_accepts_exact_fit() {
        let l = limits();
        let fits = "a".repeat(l.max_body_bytes() - TEXT_PREFIX_BYTES);
        let unit = encode(
            &AppMessage::Text {
                msg_id: 1,
                text: fits,
            },
            &l,
        )
        .unwrap();
        let unit_len = usize::try_from(TEST_MAX_PLAINTEXT_UNIT_BYTES).unwrap();
        assert_eq!(unit.bytes.len(), unit_len, "kayıt birime tam sığmalı");

        let over = "a".repeat(l.max_body_bytes() - TEXT_PREFIX_BYTES + 1);
        assert_eq!(
            encode(
                &AppMessage::Text {
                    msg_id: 1,
                    text: over
                },
                &l
            ),
            Err(EncodeError::BodyTooLarge {
                body_len: l.max_body_bytes() + 1,
                max_body_bytes: l.max_body_bytes(),
            })
        );

        let chunk = AppMessage::FileChunk {
            transfer_id: 1,
            offset: 0,
            payload: vec![0; l.max_file_chunk_bytes() + 1],
        };
        assert!(matches!(
            encode(&chunk, &l),
            Err(EncodeError::BodyTooLarge { .. })
        ));
    }

    #[test]
    fn encode_rejects_file_name_over_u8() {
        let begin = AppMessage::FileBegin {
            transfer_id: 1,
            file_size: 1,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count: 1,
            name: "a".repeat(FILE_NAME_MAX_BYTES + 1),
        };
        assert_eq!(
            encode(&begin, &limits()),
            Err(EncodeError::NameTooLong {
                name_len: FILE_NAME_MAX_BYTES + 1
            })
        );
    }

    #[test]
    fn sender_never_compresses_today() {
        let text = "tekrar ".repeat(COMPRESS_MIN_BYTES);
        let unit = encode(&AppMessage::Text { msg_id: 1, text }, &limits()).unwrap();
        assert_eq!(unit.compression_policy, CompressionPolicy::Never);
        assert_eq!(unit.bytes[FLAGS_OFFSET] & FLAG_COMPRESSED, 0);
    }

    #[test]
    fn short_records_drop() {
        assert_eq!(drop_of(&[]), DropReason::RecordTooShort);
        assert_eq!(drop_of(&[MSG_VERSION_V1]), DropReason::RecordTooShort);
        let full = record(&AppMessage::PttClose { stream_id: 1 });
        assert_eq!(
            drop_of(&full[..APP_RECORD_HEADER_BYTES - 1]),
            DropReason::RecordTooShort
        );
        // Sürüm başlığın geri kalanından önce denetlenir.
        assert_eq!(
            drop_of(&[MSG_VERSION_V1 + 1]),
            DropReason::UnknownMsgVersion
        );
    }

    #[test]
    fn reserved_high_msg_types_reject() {
        for code in [MSG_TYPE_RESERVED_HIGH_START, u8::MAX] {
            let mut rec = record(&AppMessage::PttClose { stream_id: 1 });
            rec[MSG_TYPE_OFFSET] = code;
            assert_eq!(
                reject_of(&rec),
                RejectReason::ReservedMsgType,
                "kod {code:#04x}"
            );
        }
    }

    #[test]
    fn unregistered_msg_type_drops() {
        let mut rec = record(&AppMessage::PttClose { stream_id: 1 });
        // Registry'nin son kodundan bir sonraki: kayıtlı değil, reserved da değil (§6).
        rec[MSG_TYPE_OFFSET] = MsgType::MediaBundle.code() + 1;
        assert_eq!(drop_of(&rec), DropReason::UnknownMsgType);
        rec[MSG_TYPE_OFFSET] = MSG_TYPE_RESERVED_HIGH_START - 1;
        assert_eq!(drop_of(&rec), DropReason::UnknownMsgType);
    }

    #[test]
    fn record_length_must_equal_header_plus_body_len() {
        let mut longer = record(&AppMessage::PttClose { stream_id: 1 });
        longer.push(0);
        assert_eq!(drop_of(&longer), DropReason::RecordLenMismatch);
        let mut shorter = record(&AppMessage::PttClose { stream_id: 1 });
        shorter.pop();
        assert_eq!(drop_of(&shorter), DropReason::RecordLenMismatch);
    }

    #[test]
    fn trailing_body_bytes_drop() {
        let mut rec = record(&AppMessage::FileEnd { transfer_id: 1 });
        rec.push(0);
        let body_len = rec.len() - APP_RECORD_HEADER_BYTES;
        set_body_len(&mut rec, body_len);
        assert_eq!(drop_of(&rec), DropReason::TrailingBodyBytes);
    }

    #[test]
    fn invalid_utf8_drops() {
        let mut text = record(&AppMessage::Text {
            msg_id: 1,
            text: "a".to_owned(),
        });
        *text.last_mut().unwrap() = INVALID_UTF8_BYTE;
        assert_eq!(drop_of(&text), DropReason::InvalidUtf8);

        let mut begin = record(&AppMessage::FileBegin {
            transfer_id: 1,
            file_size: 1,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count: 1,
            name: "a".to_owned(),
        });
        *begin.last_mut().unwrap() = INVALID_UTF8_BYTE;
        assert_eq!(drop_of(&begin), DropReason::InvalidUtf8);
    }

    #[test]
    fn unknown_enum_values_drop() {
        let carriers = [
            AppMessage::FileAbort {
                transfer_id: 1,
                reason: FileAbortReason::UserCancel,
            },
            AppMessage::Receipt {
                ref_msg_id: 1,
                kind: ReceiptKind::Delivered,
            },
        ];
        for message in carriers {
            // Bu iki tipte enum alanı gövdenin SON baytıdır.
            let mut rec = record(&message);
            *rec.last_mut().unwrap() = u8::MAX;
            assert_eq!(drop_of(&rec), DropReason::UnknownEnumValue, "{message:?}");
        }
        let mut open = record(&AppMessage::PttOpen {
            stream_id: 1,
            codec: PttCodec::Opus,
            bundle_ms: 1,
        });
        // PTT_OPEN: codec, bundle_ms'den (son bayt) hemen önce.
        let codec_at = open.len() - 2;
        open[codec_at] = u8::MAX;
        assert_eq!(drop_of(&open), DropReason::UnknownEnumValue);
    }

    #[test]
    fn compressed_header_checks_then_typed_unavailable() {
        let l = limits();
        let body_len = COMPRESSION_PREFIX_BYTES + TEST_COMPRESSED_PAYLOAD_LEN;
        let ratio_cap = MAX_EXPANSION_RATIO * u64::try_from(body_len).unwrap();
        let ratio_cap = u32::try_from(ratio_cap).unwrap();
        assert!(usize::try_from(ratio_cap).unwrap() <= l.max_decompressed_message());

        // Oran tam sınırda: başlık denetimleri geçer, açma tipli hata verir (todo!() yok).
        assert_eq!(
            drop_of(&compressed_record(COMPRESSION_ID_ZSTD, ratio_cap)),
            DropReason::CompressionUnavailable
        );
        // Sınırın bir üstü: tam sayı çarpımıyla yakalanır (bölme 64 < oran < 65'i kaçırırdı).
        assert_eq!(
            drop_of(&compressed_record(COMPRESSION_ID_ZSTD, ratio_cap + 1)),
            DropReason::ExpansionRatioExceeded
        );
        // compression_id "diğer" → REJECT.
        assert_eq!(
            reject_of(&compressed_record(COMPRESSION_ID_ZSTD + 1, ratio_cap)),
            RejectReason::UnknownCompressionId
        );
        // Önek sığmıyor.
        let mut short = vec![MSG_VERSION_V1, MsgType::Text.code(), FLAG_COMPRESSED, 0, 0];
        short.push(COMPRESSION_ID_ZSTD);
        let short_body = short.len() - APP_RECORD_HEADER_BYTES;
        set_body_len(&mut short, short_body);
        assert_eq!(drop_of(&short), DropReason::InnerFieldExceedsBodyLen);
    }

    #[test]
    fn debug_redacts_content() {
        let secret = "gizli-icerik";
        let messages = [
            AppMessage::Text {
                msg_id: 1,
                text: secret.to_owned(),
            },
            AppMessage::FileBegin {
                transfer_id: 1,
                file_size: 1,
                whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
                chunk_count: 1,
                name: secret.to_owned(),
            },
        ];
        for message in messages {
            let shown = format!("{message:?}");
            assert!(!shown.contains(secret), "Debug içerik sızdırdı: {shown}");
        }
    }

    #[test]
    fn reason_tokens_are_distinct() {
        let mut tokens: Vec<&str> = [
            DropReason::RecordTooShort,
            DropReason::UnknownMsgVersion,
            DropReason::RecordLenMismatch,
            DropReason::UnknownMsgType,
            DropReason::InnerFieldExceedsBodyLen,
            DropReason::TrailingBodyBytes,
            DropReason::UncompressedLenExceedsMaxDecompressed,
            DropReason::ExpansionRatioExceeded,
            DropReason::InvalidZstdFrame,
            DropReason::DecompressedLenMismatch,
            DropReason::CompressionUnavailable,
            DropReason::InvalidUtf8,
            DropReason::UnknownEnumValue,
        ]
        .into_iter()
        .map(DropReason::reason)
        .chain(
            [
                RejectReason::ReservedMsgType,
                RejectReason::ReservedFlagsNonzero,
                RejectReason::UnknownCompressionId,
            ]
            .into_iter()
            .map(RejectReason::reason),
        )
        .collect();
        let total = tokens.len();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), total, "iki gerekçe aynı dizgeyi paylaşıyor");
    }
}
