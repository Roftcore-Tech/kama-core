//! §5 per-record sıkıştırma — wire sözleşmesi ve zstd yalıtımı.
//!
//! ```text
//! flags.bit0 = 1  ⇒  body = { compression_id u8 | uncompressed_len u32_be |
//!                             payload bytes[body_len − 5] }
//! ```
//!
//! # zstd bugün YOK (sahipte bekleyen DUR)
//!
//! Workspace'te zstd kodlayıcı/çözücü bağımlılığı yoktur ve yeni harici bağımlılık bu sprintin
//! kapsamı dışındadır. zstd'ye dokunan HER ŞEY tek iç fonksiyon çiftinin arkasındadır:
//! `zstd_compress` ve `zstd_decompress`. Karar gelene kadar:
//!
//! * Gönderici HİÇ sıkıştırmaz (`flags.bit0 = 0`, `CompressionPolicy::Never`). §5 kural 6'nın
//!   "`TEXT` → ON" varsayılanı zstd kararına kadar UYGULANMAZ (`compression_default_on`).
//! * Alıcı sıkıştırılmış kayıtta beyan edilen uzunluklara bakan bütün denetimleri yapar
//!   (`body_len < 5`, `compression_id`, kural 1, kural 4). Bunları geçen kaydın açılması tipli
//!   hata verir: `DropReason::CompressionUnavailable` (`zstd-decoder-unavailable`). Kayıt
//!   atılır; panik ya da `todo!()` yoktur.

use super::body::Reader;
use super::limits::{
    AmpLimits, COMPRESSION_ID_ZSTD, COMPRESSION_PREFIX_BYTES, COMPRESS_MIN_BYTES,
    MAX_EXPANSION_RATIO,
};
use super::{DecodeError, DropReason, MsgType, RejectReason};

/// §5 kural 6 — tip-türevi sıkıştırma varsayılanı. Bu karar YALNIZ burada alınır (Transport tip
/// bilmez — INV-PLAINTEXT-NOT-SEMANTICS).
///
/// `MEDIA_BUNDLE`, `SIGNALING_BLOB`, `FILE_CHUNK` → kapalı (spec). `TEXT` → spec'te AÇIK; zstd
/// kararı gelene kadar kapalı (bkz. modül belgesi). Spec'in adlandırmadığı tipler → kapalı.
const fn compression_default_on(msg_type: MsgType) -> bool {
    match msg_type {
        // zstd-pending: spec §5 kural 6 "TEXT → ON"; zstd bağımlılığı kararıyla açılır.
        MsgType::Text => false,
        MsgType::MediaBundle | MsgType::SignalingBlob | MsgType::FileChunk => false,
        MsgType::FileBegin
        | MsgType::FileEnd
        | MsgType::FileAbort
        | MsgType::PttOpen
        | MsgType::PttClose
        | MsgType::Receipt
        | MsgType::FileReceipt => false,
    }
}

/// zstd kodlayıcı yalıtımı: tek kare, sözlüksüz, kayıt başına taze bağlam (§5). Bugün kodlayıcı
/// yoktur → `None` (gönderici sıkıştırmadan yollar).
fn zstd_compress(_plain: &[u8]) -> Option<Vec<u8>> {
    None
}

/// zstd çözücü yalıtımı. Gerçek çözücünün sözleşmesi (bağımlılık kararıyla birlikte yazılacak):
/// çıktı tamponu `uncompressed_len + 1` kapasiteyle sınırlı (taşan çıktı kural 2), tek kare,
/// sözlüksüz; geçersiz/kesik kare → `DropReason::InvalidZstdFrame` (kural 3). Bugün çözücü
/// yoktur → `DropReason::CompressionUnavailable`.
fn zstd_decompress(_frame: &[u8], _uncompressed_len: usize) -> Result<Vec<u8>, DropReason> {
    Err(DropReason::CompressionUnavailable)
}

/// Gönderici kuralları 5–7: sıkıştırılmış gövdeyi döndürür ya da `None` (sıkıştırmadan gönder).
pub(crate) fn compress_for_send(msg_type: MsgType, plain: &[u8]) -> Option<Vec<u8>> {
    // Kural 5: küçük mesaj sıkıştırılmaz. Kural 6: tip-türevi varsayılan.
    if plain.len() < COMPRESS_MIN_BYTES || !compression_default_on(msg_type) {
        return None;
    }
    let uncompressed_len = u32::try_from(plain.len()).ok()?;
    let payload = zstd_compress(plain)?;
    let body_len = COMPRESSION_PREFIX_BYTES.checked_add(payload.len())?;
    // Kural 7: sıkıştırılmış çıktı sıkıştırılmamış gövdeden büyükse sıkıştırmadan gönder.
    if body_len > plain.len() {
        return None;
    }
    let mut body = Vec::with_capacity(body_len);
    body.push(COMPRESSION_ID_ZSTD);
    body.extend_from_slice(&uncompressed_len.to_be_bytes());
    body.extend_from_slice(&payload);
    Some(body)
}

/// Alıcı kuralları (fail-closed, bounded). `body` = başlıktaki `body_len` baytlık sıkıştırılmış
/// gövde (önek dahil). Sıra: önek sığıyor mu → `compression_id` → kural 1 → kural 4 → aç
/// (kural 3) → kural 2. Kural 1 kural 4'ten ÖNCE gelir (ikisini birden çiğneyen ret vektörü
/// `uncompressed-len-exceeds-max-decompressed` bekler). Kural 1 ve 4 decompress'e başlamadan,
/// yalnız beyan edilen uzunluklarla karar verir.
pub(crate) fn open_compressed(body: &[u8], limits: &AmpLimits) -> Result<Vec<u8>, DecodeError> {
    let mut r = Reader::new(body);
    let compression_id = r.read_u8()?;
    let declared = r.read_u32()?;
    if compression_id != COMPRESSION_ID_ZSTD {
        return Err(DecodeError::Reject(RejectReason::UnknownCompressionId));
    }
    // Kural 1 (bomb koruması). u32 → usize yalnız 16-bit hedefte taşar; orada değer zaten tavanı
    // aşar.
    let uncompressed_len = usize::try_from(declared).unwrap_or(usize::MAX);
    if uncompressed_len > limits.max_decompressed_message() {
        return Err(DecodeError::Drop(
            DropReason::UncompressedLenExceedsMaxDecompressed,
        ));
    }
    // Kural 4 — tam sayı çarpımı, u64: `uncompressed_len > MAX_EXPANSION_RATIO × body_len`.
    // Payda başlıktaki `body_len`'dir (5 B önek DAHİL).
    let body_len = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if u64::from(declared) > MAX_EXPANSION_RATIO.saturating_mul(body_len) {
        return Err(DecodeError::Drop(DropReason::ExpansionRatioExceeded));
    }
    // Kural 3: açma.
    let plain = zstd_decompress(r.rest(), uncompressed_len).map_err(DecodeError::Drop)?;
    // Kural 2: çıktı beyan edilenden fazla ya da eksik → DROP.
    if plain.len() != uncompressed_len {
        return Err(DecodeError::Drop(DropReason::DecompressedLenMismatch));
    }
    Ok(plain)
}
