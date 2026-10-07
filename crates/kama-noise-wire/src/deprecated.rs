//! MUST-NOT-IMPLEMENT formatların reddi (ccr §35.12 / AC-SM3-12; §30.7 DEPRECATED-REJECT-1/2).
//!
//! Async Session V2 (`SAI1`/`SAA1`/`SAD1`/`SAC1`), eski tek-yapı AsyncMessage (`SRAM`) ve kaldırılmış
//! prekey yapıları (`SRPK`, `SRPB`) aktif wire'da görünürse ret, magic'ten verilir; geri kalan
//! baytlara BAKILMAZ. Aktif parser bunların hiçbiri için kabul döndüremez: el sıkışma girişi
//! ([`crate::handshake`]) zarfı ayrıştırmadan ÖNCE bu süzgeçten geçirir.

/// Reddedilen eski format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeprecatedFormat {
    /// `SAI1` — ASYNC_INIT (V2).
    AsyncInit,
    /// `SAA1` — ASYNC_ACCEPT (V2).
    AsyncAccept,
    /// `SAD1` — ASYNC_DATA (V2).
    AsyncData,
    /// `SAC1` — ASYNC_CONTROL (V2).
    AsyncControl,
    /// `SRAM` — tek-yapı AsyncMessage.
    AsyncMessage,
    /// `SRPK` — SignedNoisePrekey (§30.7 DEPRECATED-REJECT-1).
    SignedNoisePrekey,
    /// `SRPB` — PrekeyBundle (§30.7 DEPRECATED-REJECT-2).
    PrekeyBundle,
}

const MAGICS: [([u8; 4], DeprecatedFormat); 7] = [
    (*b"SAI1", DeprecatedFormat::AsyncInit),
    (*b"SAA1", DeprecatedFormat::AsyncAccept),
    (*b"SAD1", DeprecatedFormat::AsyncData),
    (*b"SAC1", DeprecatedFormat::AsyncControl),
    (*b"SRAM", DeprecatedFormat::AsyncMessage),
    (*b"SRPK", DeprecatedFormat::SignedNoisePrekey),
    (*b"SRPB", DeprecatedFormat::PrekeyBundle),
];

/// Girdi eski bir formatın magic'iyle başlıyorsa o format.
#[must_use]
pub fn deprecated_format(input: &[u8]) -> Option<DeprecatedFormat> {
    let magic = input.get(..4)?;
    MAGICS
        .iter()
        .find(|(m, _)| m.as_slice() == magic)
        .map(|(_, f)| *f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_listed_magics_are_flagged() {
        assert_eq!(
            deprecated_format(b"SAI1rest"),
            Some(DeprecatedFormat::AsyncInit)
        );
        assert_eq!(deprecated_format(b"SRP"), None, "kısa girdi");
        // Aktif magic'ler (AAD-only olsalar da) eski format sayılmaz.
        for active in [b"SSI1", b"SSR1", b"SSD1", b"SSC1", b"SRDB"] {
            assert_eq!(deprecated_format(active), None);
        }
    }
}
