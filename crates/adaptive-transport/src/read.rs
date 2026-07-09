//! Internal bounds-checked byte okuyucu (PARSER-02: bounds-check-before-read).
//!
//! ack / fec codec'lerinin ortak fixed-width big-endian okuyucusu (DRY). ADR-05 deferred:
//! ccr-codec public API'si tasarlanınca oraya çıkarılacak.

use crate::error::TransportError;

/// Her okumadan önce sınır kontrol eden byte okuyucu.
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], TransportError> {
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

    pub(crate) fn u8(&mut self) -> Result<u8, TransportError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16_be(&mut self) -> Result<u16, TransportError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32_be(&mut self) -> Result<u32, TransportError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}
