//! EXTENDED-outer codec (§2.5 YOL-1 + §2.6 KİLİTLİ):
//! `[h_ext u32_be][wire_format u8=0x01][wire_flags u8 bit0=dh_epoch, bits1..7=0→REJECT][pn_trunc u32_be][ct][tag 16]`
//! Sabit-26B non-ct overhead. ct-length UDP-length-üzerinden-implicit (§2.8). ADR-05 fixed-width big-endian.

use crate::error::WireError;
use kama_transport_schema::{EXTENDED_OUTER_OVERHEAD, EXTENDED_V1};

/// AEAD tag uzunluğu (ChaCha20-Poly1305; §2.6 tag 16B).
pub const TAG_LEN: usize = 16;
/// Outer-header uzunluğu (tag hariç): h_ext 4 + wire_format 1 + wire_flags 1 + pn_trunc 4 = 10B.
pub const OUTER_HDR_LEN: usize = 10;
/// wire_flags bit0 = dh_epoch (§2.5).
pub const FLAG_DH_EPOCH: u8 = 0b0000_0001;
const FLAGS_RESERVED_MASK: u8 = !FLAG_DH_EPOCH;

/// Parse edilmiş EXTENDED-outer header (cleartext, AAD-authenticated — §2 Cryptex).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedOuter {
    pub h_ext: u32,
    /// wire_flags.bit0 (0/1). §5: yalnız dh_epoch 0/1.
    pub dh_epoch: u8,
    /// 32-bit truncated PN (direction-local full-PN'in düşük 32 biti; §2.5).
    pub pn_trunc: u32,
}

impl ExtendedOuter {
    /// wire_flags byte'ını üretir (bit0=dh_epoch; kalan 0).
    #[must_use]
    pub fn wire_flags(&self) -> u8 {
        self.dh_epoch & FLAG_DH_EPOCH
    }

    /// Outer-header'ı 10B tampona serialize eder (ct/tag çağıranın; sıra §2.6-sabit).
    #[must_use]
    pub fn serialize_hdr(&self) -> [u8; OUTER_HDR_LEN] {
        let mut out = [0u8; OUTER_HDR_LEN];
        out[0..4].copy_from_slice(&self.h_ext.to_be_bytes());
        out[4] = EXTENDED_V1;
        out[5] = self.wire_flags();
        out[6..10].copy_from_slice(&self.pn_trunc.to_be_bytes());
        out
    }

    /// Datagram'dan outer-parse (§4-ORDER adım-1: fixed-cost). Döner: (header, ciphertext, tag).
    ///
    /// # Errors
    /// Kısa-datagram / bilinmeyen-format / reserved-flags → fail-closed reject.
    pub fn parse(datagram: &[u8]) -> Result<(Self, &[u8], &[u8; TAG_LEN]), WireError> {
        if datagram.len() < EXTENDED_OUTER_OVERHEAD {
            return Err(WireError::DatagramTooShort {
                got: datagram.len(),
                min: EXTENDED_OUTER_OVERHEAD,
            });
        }
        let h_ext = u32::from_be_bytes([datagram[0], datagram[1], datagram[2], datagram[3]]);
        let wire_format = datagram[4];
        if wire_format != EXTENDED_V1 {
            return Err(WireError::UnknownWireFormat(wire_format));
        }
        let wire_flags = datagram[5];
        if wire_flags & FLAGS_RESERVED_MASK != 0 {
            return Err(WireError::ReservedWireFlags(wire_flags));
        }
        let pn_trunc = u32::from_be_bytes([datagram[6], datagram[7], datagram[8], datagram[9]]);
        let ct_end = datagram.len() - TAG_LEN;
        let ciphertext = &datagram[OUTER_HDR_LEN..ct_end];
        let tag: &[u8; TAG_LEN] = datagram[ct_end..].try_into().expect("sabit-16B dilim");
        Ok((
            Self {
                h_ext,
                dh_epoch: wire_flags & FLAG_DH_EPOCH,
                pn_trunc,
            },
            ciphertext,
            tag,
        ))
    }
}
