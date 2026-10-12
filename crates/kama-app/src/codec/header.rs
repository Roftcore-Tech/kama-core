//! §2 kayıt başlığı (5 B) ve §3 tip registry'si; alıcı başlık denetimlerinin SIRASI burada.

use super::limits::{
    APP_RECORD_HEADER_BYTES, FLAGS_RESERVED_MASK, FLAG_COMPRESSED, MSG_TYPE_RESERVED_HIGH_START,
    MSG_TYPE_RESERVED_ZERO, MSG_VERSION_V1,
};
use super::{DecodeError, DropReason, EncodeError, RejectReason};

/// Başlıkta `msg_version` alanının konumu.
pub(crate) const MSG_VERSION_OFFSET: usize = 0;
/// Başlıkta `msg_type` alanının konumu.
pub(crate) const MSG_TYPE_OFFSET: usize = 1;
/// Başlıkta `flags` alanının konumu.
pub(crate) const FLAGS_OFFSET: usize = 2;
/// Başlıkta `body_len u16_be` alanının ilk baytının konumu.
pub(crate) const BODY_LEN_OFFSET: usize = 3;

/// §3 tip registry (v1). Ayırt ediciler registry kodlarıdır.
///
/// `0x00` ve `0xF0..=0xFF` reserved'dır (REJECT); `0x0C..=0xEF` bilinmeyen-ama-reserved-olmayan
/// tiplerdir (DROP, ileri uyum — §6; K4G-09'dan önce `0x0B..=0xEF`). İkisi de bu enum'da temsil
/// edilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MsgType {
    /// 0x01 `TEXT`.
    Text = 0x01,
    /// 0x02 `FILE_BEGIN`.
    FileBegin = 0x02,
    /// 0x03 `FILE_CHUNK`.
    FileChunk = 0x03,
    /// 0x04 `FILE_END`.
    FileEnd = 0x04,
    /// 0x05 `FILE_ABORT`.
    FileAbort = 0x05,
    /// 0x06 `PTT_OPEN`.
    PttOpen = 0x06,
    /// 0x07 `PTT_CLOSE`.
    PttClose = 0x07,
    /// 0x08 `RECEIPT`.
    Receipt = 0x08,
    /// 0x09 `SIGNALING_BLOB`.
    SignalingBlob = 0x09,
    /// 0x0A `MEDIA_BUNDLE`.
    MediaBundle = 0x0A,
    /// 0x0B `FILE_RECEIPT` (K4G-09, plan D6).
    FileReceipt = 0x0B,
}

impl MsgType {
    /// Registry'deki bütün tipler, kod sırasıyla.
    pub const ALL: [Self; 11] = [
        Self::Text,
        Self::FileBegin,
        Self::FileChunk,
        Self::FileEnd,
        Self::FileAbort,
        Self::PttOpen,
        Self::PttClose,
        Self::Receipt,
        Self::SignalingBlob,
        Self::MediaBundle,
        Self::FileReceipt,
    ];

    /// Teldeki `msg_type` kodu.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Kayıtlı bir koddan tip; reserved ya da bilinmeyen kod → `None`.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.code() == code)
    }

    /// §2 — `0x00` ve `0xF0..=0xFF` reserved'dır.
    #[must_use]
    pub const fn is_reserved_code(code: u8) -> bool {
        code == MSG_TYPE_RESERVED_ZERO || code >= MSG_TYPE_RESERVED_HIGH_START
    }
}

/// Başlık denetimlerinden geçmiş kayıt: tip, sıkıştırma biti ve tam `body_len` baytlık gövde.
pub(crate) struct ParsedRecord<'a> {
    pub(crate) msg_type: MsgType,
    pub(crate) compressed: bool,
    pub(crate) body: &'a [u8],
}

/// Alıcı başlık denetimleri. Sıra spec'te adım adım yazılı değildir; ret vektörleri onu sabitler
/// (`vectors/amp-record-reject-v1.json`; ör. `unknown-msg-version` başlığın geri kalanından önce):
///
/// 1. kayıt boş → DROP `record-too-short`
/// 2. `msg_version ≠ 0x01` → DROP `unknown-msg-version` (başlığın geri kalanı yorumlanmaz, §6)
/// 3. kayıt 5 B'tan kısa → DROP `record-too-short`
/// 4. `msg_type` reserved → REJECT `reserved-msg-type`
/// 5. `flags` reserved bit → REJECT `reserved-flags-nonzero` (tipten bağımsız)
/// 6. `len − 5 ≠ body_len` → DROP `record-len-mismatch`
/// 7. `msg_type` kayıtlı değil → DROP `unknown-msg-type` (§6 ileri uyum)
pub(crate) fn parse_record(record: &[u8]) -> Result<ParsedRecord<'_>, DecodeError> {
    let Some(&msg_version) = record.get(MSG_VERSION_OFFSET) else {
        return Err(DecodeError::Drop(DropReason::RecordTooShort));
    };
    if msg_version != MSG_VERSION_V1 {
        return Err(DecodeError::Drop(DropReason::UnknownMsgVersion));
    }
    let Some((head, body)) = record.split_first_chunk::<APP_RECORD_HEADER_BYTES>() else {
        return Err(DecodeError::Drop(DropReason::RecordTooShort));
    };
    let type_code = head[MSG_TYPE_OFFSET];
    let flags = head[FLAGS_OFFSET];
    let body_len = u16::from_be_bytes([head[BODY_LEN_OFFSET], head[BODY_LEN_OFFSET + 1]]);
    if MsgType::is_reserved_code(type_code) {
        return Err(DecodeError::Reject(RejectReason::ReservedMsgType));
    }
    if flags & FLAGS_RESERVED_MASK != 0 {
        return Err(DecodeError::Reject(RejectReason::ReservedFlagsNonzero));
    }
    if body.len() != usize::from(body_len) {
        return Err(DecodeError::Drop(DropReason::RecordLenMismatch));
    }
    let Some(msg_type) = MsgType::from_code(type_code) else {
        return Err(DecodeError::Drop(DropReason::UnknownMsgType));
    };
    Ok(ParsedRecord {
        msg_type,
        compressed: flags & FLAG_COMPRESSED != 0,
        body,
    })
}

/// Başlık + gövde → `AppMessageRecord` baytları. `body` uzunluğu `body_len` alanına sığmalıdır.
pub(crate) fn write_record(
    msg_type: MsgType,
    flags: u8,
    body: &[u8],
) -> Result<Vec<u8>, EncodeError> {
    let body_len = u16::try_from(body.len()).map_err(|_| EncodeError::BodyTooLarge {
        body_len: body.len(),
        max_body_bytes: usize::from(u16::MAX),
    })?;
    let mut record = Vec::with_capacity(APP_RECORD_HEADER_BYTES + body.len());
    record.push(MSG_VERSION_V1);
    record.push(msg_type.code());
    record.push(flags);
    record.extend_from_slice(&body_len.to_be_bytes());
    record.extend_from_slice(body);
    Ok(record)
}
