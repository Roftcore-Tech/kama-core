//! §3 tip gövdeleri — A1: sabit-genişlik big-endian alanlar; her değişken-uzunluklu alan kendi
//! sabit-genişlik uzunluk alanıyla ÖNCE gelir; opsiyonel alan yok.
//!
//! Okuma tarafı sınır-önce-tahsis kuralını uygular: her uzunluk alanı, kopyalama yapılmadan önce
//! gövdenin kalan baytlarıyla karşılaştırılır; aşan alan → DROP `inner-field-exceeds-body-len`.

use super::limits::{
    AmpLimits, FILE_ABORT_BODY_BYTES, FILE_BEGIN_PREFIX_BYTES, FILE_CHUNK_PREFIX_BYTES,
    FILE_END_BODY_BYTES, FILE_NAME_MAX_BYTES, FILE_RECEIPT_BODY_BYTES, MEDIA_BUNDLE_PREFIX_BYTES,
    PTT_CLOSE_BODY_BYTES, PTT_OPEN_BODY_BYTES, RECEIPT_BODY_BYTES, SIGNALING_BLOB_PREFIX_BYTES,
    TEXT_PREFIX_BYTES, WHOLE_FILE_HASH_BYTES,
};
use super::{
    AppMessage, DecodeError, DropReason, EncodeError, FileAbortReason, FileReceiptStatus, MsgType,
    PttCodec, ReceiptKind,
};

/// Gövde okuyucusu: her okuma önce kalan bayt sayısını denetler.
pub(crate) struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(body: &'a [u8]) -> Self {
        Self { rest: body }
    }

    fn exceeds() -> DecodeError {
        DecodeError::Drop(DropReason::InnerFieldExceedsBodyLen)
    }

    /// `n` baytı ödünç verir; `n` kalan baytlardan büyükse hiçbir şey tüketmeden DROP.
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let rest: &'a [u8] = self.rest;
        if n > rest.len() {
            return Err(Self::exceeds());
        }
        let (head, tail) = rest.split_at(n);
        self.rest = tail;
        Ok(head)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let rest: &'a [u8] = self.rest;
        let Some((head, tail)) = rest.split_first_chunk::<N>() else {
            return Err(Self::exceeds());
        };
        self.rest = tail;
        Ok(*head)
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, DecodeError> {
        self.array().map(u8::from_be_bytes)
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16, DecodeError> {
        self.array().map(u16::from_be_bytes)
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, DecodeError> {
        self.array().map(u32::from_be_bytes)
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64, DecodeError> {
        self.array().map(u64::from_be_bytes)
    }

    /// Kalan baytların tamamı (sıkıştırılmış yük gibi örtük-uzunluklu kuyruk için).
    pub(crate) fn rest(self) -> &'a [u8] {
        self.rest
    }

    /// Gövde tam tüketilmiş olmalı; artık bayt → DROP `trailing-body-bytes` (tek kanonik biçim).
    fn finish(self) -> Result<(), DecodeError> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::Drop(DropReason::TrailingBodyBytes))
        }
    }
}

fn utf8(bytes: &[u8]) -> Result<String, DecodeError> {
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| DecodeError::Drop(DropReason::InvalidUtf8))
}

fn unknown_enum() -> DecodeError {
    DecodeError::Drop(DropReason::UnknownEnumValue)
}

/// Sıkıştırılmamış gövde → mesaj. `body` tam `body_len` bayttır (başlık denetiminden geçmiş).
pub(crate) fn decode_body(msg_type: MsgType, body: &[u8]) -> Result<AppMessage, DecodeError> {
    let mut r = Reader::new(body);
    let message = match msg_type {
        MsgType::Text => {
            let msg_id = r.read_u32()?;
            let text_len = usize::from(r.read_u16()?);
            let text = utf8(r.take(text_len)?)?;
            AppMessage::Text { msg_id, text }
        }
        MsgType::FileBegin => {
            let transfer_id = r.read_u64()?;
            let file_size = r.read_u64()?;
            let whole_file_hash = r.array::<WHOLE_FILE_HASH_BYTES>()?;
            let chunk_count = r.read_u32()?;
            let name_len = usize::from(r.read_u8()?);
            let name = utf8(r.take(name_len)?)?;
            AppMessage::FileBegin {
                transfer_id,
                file_size,
                whole_file_hash,
                chunk_count,
                name,
            }
        }
        MsgType::FileChunk => {
            let transfer_id = r.read_u64()?;
            let offset = r.read_u64()?;
            let payload_len = usize::from(r.read_u16()?);
            let payload = r.take(payload_len)?.to_vec();
            AppMessage::FileChunk {
                transfer_id,
                offset,
                payload,
            }
        }
        MsgType::FileEnd => AppMessage::FileEnd {
            transfer_id: r.read_u64()?,
        },
        MsgType::FileAbort => {
            let transfer_id = r.read_u64()?;
            let reason = FileAbortReason::from_code(r.read_u8()?).ok_or_else(unknown_enum)?;
            AppMessage::FileAbort {
                transfer_id,
                reason,
            }
        }
        MsgType::PttOpen => {
            let stream_id = r.read_u32()?;
            let codec = PttCodec::from_code(r.read_u8()?).ok_or_else(unknown_enum)?;
            let bundle_ms = r.read_u8()?;
            AppMessage::PttOpen {
                stream_id,
                codec,
                bundle_ms,
            }
        }
        MsgType::PttClose => AppMessage::PttClose {
            stream_id: r.read_u32()?,
        },
        MsgType::Receipt => {
            let ref_msg_id = r.read_u32()?;
            let kind = ReceiptKind::from_code(r.read_u8()?).ok_or_else(unknown_enum)?;
            AppMessage::Receipt { ref_msg_id, kind }
        }
        MsgType::SignalingBlob => {
            let blob_len = usize::from(r.read_u16()?);
            let blob = r.take(blob_len)?.to_vec();
            AppMessage::SignalingBlob { blob }
        }
        MsgType::MediaBundle => {
            let stream_id = r.read_u32()?;
            let codec = r.read_u8()?;
            let bundle_ms = r.read_u8()?;
            let payload_len = usize::from(r.read_u16()?);
            let payload = r.take(payload_len)?.to_vec();
            AppMessage::MediaBundle {
                stream_id,
                codec,
                bundle_ms,
                payload,
            }
        }
        // K4G-09: sabit 9 B — kısa gövde `inner-field-exceeds-body-len`, artık bayt (aşağıdaki
        // `finish`) `trailing-body-bytes`.
        MsgType::FileReceipt => {
            let transfer_id = r.read_u64()?;
            let status = FileReceiptStatus::from_code(r.read_u8()?).ok_or_else(unknown_enum)?;
            AppMessage::FileReceipt {
                transfer_id,
                status,
            }
        }
    };
    r.finish()?;
    Ok(message)
}

/// Sıkıştırılmamış gövde uzunluğu (denetimli toplam). Taşma `usize::MAX` olarak döner ve her
/// `MAX_BODY_BYTES`'ı aşar.
fn plain_body_len(message: &AppMessage) -> Result<usize, EncodeError> {
    let variable = |prefix: usize, len: usize| prefix.saturating_add(len);
    Ok(match message {
        AppMessage::Text { text, .. } => variable(TEXT_PREFIX_BYTES, text.len()),
        AppMessage::FileBegin { name, .. } => {
            if name.len() > FILE_NAME_MAX_BYTES {
                return Err(EncodeError::NameTooLong {
                    name_len: name.len(),
                });
            }
            variable(FILE_BEGIN_PREFIX_BYTES, name.len())
        }
        AppMessage::FileChunk { payload, .. } => variable(FILE_CHUNK_PREFIX_BYTES, payload.len()),
        AppMessage::FileEnd { .. } => FILE_END_BODY_BYTES,
        AppMessage::FileAbort { .. } => FILE_ABORT_BODY_BYTES,
        AppMessage::PttOpen { .. } => PTT_OPEN_BODY_BYTES,
        AppMessage::PttClose { .. } => PTT_CLOSE_BODY_BYTES,
        AppMessage::Receipt { .. } => RECEIPT_BODY_BYTES,
        AppMessage::SignalingBlob { blob } => variable(SIGNALING_BLOB_PREFIX_BYTES, blob.len()),
        AppMessage::MediaBundle { payload, .. } => {
            variable(MEDIA_BUNDLE_PREFIX_BYTES, payload.len())
        }
        AppMessage::FileReceipt { .. } => FILE_RECEIPT_BODY_BYTES,
    })
}

/// Mesaj → sıkıştırılmamış gövde. Boyut `MAX_BODY_BYTES`'a karşı TAHSİSTEN ÖNCE denetlenir.
pub(crate) fn encode_body(
    message: &AppMessage,
    limits: &AmpLimits,
) -> Result<Vec<u8>, EncodeError> {
    let body_len = plain_body_len(message)?;
    let max_body_bytes = limits.max_body_bytes();
    if body_len > max_body_bytes {
        return Err(EncodeError::BodyTooLarge {
            body_len,
            max_body_bytes,
        });
    }
    // Uzunluk alanları gövdenin parçasıdır; gövde ≤ MAX_BODY_BYTES ≤ 65535 olduğundan sığarlar.
    let len_u16 = |len: usize| {
        u16::try_from(len).map_err(|_| EncodeError::BodyTooLarge {
            body_len,
            max_body_bytes,
        })
    };
    let mut out = Vec::with_capacity(body_len);
    match message {
        AppMessage::Text { msg_id, text } => {
            out.extend_from_slice(&msg_id.to_be_bytes());
            out.extend_from_slice(&len_u16(text.len())?.to_be_bytes());
            out.extend(text.bytes());
        }
        AppMessage::FileBegin {
            transfer_id,
            file_size,
            whole_file_hash,
            chunk_count,
            name,
        } => {
            let name_len = u8::try_from(name.len()).map_err(|_| EncodeError::NameTooLong {
                name_len: name.len(),
            })?;
            out.extend_from_slice(&transfer_id.to_be_bytes());
            out.extend_from_slice(&file_size.to_be_bytes());
            out.extend_from_slice(whole_file_hash);
            out.extend_from_slice(&chunk_count.to_be_bytes());
            out.push(name_len);
            out.extend(name.bytes());
        }
        AppMessage::FileChunk {
            transfer_id,
            offset,
            payload,
        } => {
            out.extend_from_slice(&transfer_id.to_be_bytes());
            out.extend_from_slice(&offset.to_be_bytes());
            out.extend_from_slice(&len_u16(payload.len())?.to_be_bytes());
            out.extend_from_slice(payload);
        }
        AppMessage::FileEnd { transfer_id } => {
            out.extend_from_slice(&transfer_id.to_be_bytes());
        }
        AppMessage::FileAbort {
            transfer_id,
            reason,
        } => {
            out.extend_from_slice(&transfer_id.to_be_bytes());
            out.push(reason.code());
        }
        AppMessage::PttOpen {
            stream_id,
            codec,
            bundle_ms,
        } => {
            out.extend_from_slice(&stream_id.to_be_bytes());
            out.push(codec.code());
            out.push(*bundle_ms);
        }
        AppMessage::PttClose { stream_id } => {
            out.extend_from_slice(&stream_id.to_be_bytes());
        }
        AppMessage::Receipt { ref_msg_id, kind } => {
            out.extend_from_slice(&ref_msg_id.to_be_bytes());
            out.push(kind.code());
        }
        AppMessage::SignalingBlob { blob } => {
            out.extend_from_slice(&len_u16(blob.len())?.to_be_bytes());
            out.extend_from_slice(blob);
        }
        AppMessage::MediaBundle {
            stream_id,
            codec,
            bundle_ms,
            payload,
        } => {
            out.extend_from_slice(&stream_id.to_be_bytes());
            out.push(*codec);
            out.push(*bundle_ms);
            out.extend_from_slice(&len_u16(payload.len())?.to_be_bytes());
            out.extend_from_slice(payload);
        }
        AppMessage::FileReceipt {
            transfer_id,
            status,
        } => {
            out.extend_from_slice(&transfer_id.to_be_bytes());
            out.push(status.code());
        }
    }
    debug_assert_eq!(
        out.len(),
        body_len,
        "plain_body_len ile yazılan gövde ayrıştı"
    );
    Ok(out)
}
