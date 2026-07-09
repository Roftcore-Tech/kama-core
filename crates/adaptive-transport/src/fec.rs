//! TV-FEC (§10.1): FEC_SHARD wire framing codec.
//!
//! Transport yalnız FRAMING metadata okur/yazar (block_id/shard_index/k/r/shard_len —
//! reliability/reconstruction TRACKING için). `opaque_secured` (= ct ‖ aead_tag) transport
//! için TEK OPAK BLOB'dur: transport ASLA parse/verify ETMEZ. RS-encode + per-shard AEAD
//! EXECUTION güvenlik katmanında (Noise+kompakt); **bu crate kripto İÇERMEZ** (INV-ENGINE-SOC,
//! INV-FEC-1). Fixed-width big-endian (ADR-05, varint yasak).

use crate::error::TransportError;
use crate::read::Reader;

/// Transport-control frame_type: FEC_SHARD (spec §10.1 / engine §1.1).
pub const FRAME_TYPE_FEC_SHARD: u8 = 0x20;

/// Güvenlik katmanının her shard'a eklediği AEAD trailer boyutu (spec §10.1: aead_tag 16B).
///
/// WIRE FORMAT sabiti (kripto parametresi DEĞİL). Transport tag'i yorumlamaz, yalnız boyutlar
/// (tıpkı block_id'nin 4B olması gibi). Tag boyutu değişirse → wire-format değişimi → yeni
/// frame_type/versiyon. INV-ENGINE-SOC korunur.
pub const SHARD_TAG_LEN: usize = 16;

/// FEC shard wire frame (spec §10.1): 10B framing header + `opaque_secured` (shard_len + tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FecShardFrame {
    /// Blok kimliği (hop-scoped, wrap→rekey).
    pub block_id: u32,
    /// Bu shard'ın indeksi (0..k+r−1).
    pub shard_index: u8,
    /// Data shard sayısı (bu blok).
    pub k: u8,
    /// Parity shard sayısı (bu blok).
    pub r: u8,
    /// Ciphertext payload uzunluğu (opaque_secured'ın tag hariç kısmı).
    pub shard_len: u16,
    /// = ciphertext ‖ aead_tag (shard_len + [`SHARD_TAG_LEN`] bytes). Transport için TEK
    /// OPAK BLOB — ASLA parse/verify ETMEZ. AEAD güvenlik katmanında; bu crate kripto İÇERMEZ.
    pub opaque_secured: Vec<u8>,
}

impl FecShardFrame {
    /// Canonical fixed-width big-endian byte'lara serialize eder.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(10 + self.opaque_secured.len());
        out.push(FRAME_TYPE_FEC_SHARD);
        out.extend_from_slice(&self.block_id.to_be_bytes());
        out.push(self.shard_index);
        out.push(self.k);
        out.push(self.r);
        out.extend_from_slice(&self.shard_len.to_be_bytes());
        out.extend_from_slice(&self.opaque_secured); // opak — transport içeriğe BAKMAZ
        out
    }

    /// Canonical byte'ları çözer. `opaque_secured` = shard_len + tag bytes olarak TEK blob
    /// okunur (İÇERİĞE BAKILMAZ). PARSER-02 bounds-check.
    ///
    /// # Errors
    /// [`TransportError::UnknownFrameType`] (frame_type≠0x20),
    /// [`TransportError::InvalidShardCount`] (k+r>255, GF(2^8)),
    /// [`TransportError::Truncated`] (kısa buffer),
    /// [`TransportError::TrailingBytes`] (sonda artan).
    pub fn decode(buf: &[u8]) -> Result<Self, TransportError> {
        let mut rd = Reader::new(buf);
        let ft = rd.u8()?;
        if ft != FRAME_TYPE_FEC_SHARD {
            return Err(TransportError::UnknownFrameType(ft));
        }
        let block_id = rd.u32_be()?;
        let shard_index = rd.u8()?;
        let k = rd.u8()?;
        let r = rd.u8()?;
        let shard_len = rd.u16_be()?;
        // GF(2^8): k+r ≤ 255 (spec §10.2 RS_MAX_SHARDS).
        if u16::from(k) + u16::from(r) > 255 {
            return Err(TransportError::InvalidShardCount { k, r });
        }
        // opaque_secured = ct(shard_len) ‖ tag(SHARD_TAG_LEN); TEK blob, İÇERİĞE BAKILMAZ.
        // PARSER-02: take bounds-check yapar (buffer kısaysa Truncated).
        let blob_len = usize::from(shard_len) + SHARD_TAG_LEN;
        let opaque_secured = rd.take(blob_len)?.to_vec();
        if rd.remaining() != 0 {
            return Err(TransportError::TrailingBytes {
                count: rd.remaining(),
            });
        }
        Ok(FecShardFrame {
            block_id,
            shard_index,
            k,
            r,
            shard_len,
            opaque_secured,
        })
    }
}
