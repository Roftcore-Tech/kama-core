//! Canonical-AAD builder (§2.8 KİLİTLİ, MADDE-13):
//! `AAD = ASCII("KAMA-WIRE-EXT-V1") || h_ext_u32_be || wire_format_u8 || wire_flags_u8 || pn_trunc_u32_be`
//! Alan-sırası-DEĞİŞMEZ · big-endian · native-struct/serde/padding YASAK · tag AAD'ye-DAHİL-DEĞİL ·
//! ciphertext-length UDP-length-üzerinden-implicit (AAD'de-YOK).

use crate::outer::ExtendedOuter;
use kama_transport_schema::EXTENDED_V1;

/// AAD domain-separator (§2.8; 16 ASCII byte).
pub const AAD_DS: &[u8; 16] = b"KAMA-WIRE-EXT-V1";
/// AAD toplam uzunluğu: 16 (DS) + 4 (h_ext) + 1 (wire_format) + 1 (wire_flags) + 4 (pn_trunc) = 26.
pub const AAD_LEN: usize = 26;

/// Outer-header alanlarından canonical-AAD üretir (deterministik; golden-vector ile ayrıca assert edilir).
#[must_use]
pub fn build_aad(outer: &ExtendedOuter) -> [u8; AAD_LEN] {
    let mut aad = [0u8; AAD_LEN];
    aad[0..16].copy_from_slice(AAD_DS);
    aad[16..20].copy_from_slice(&outer.h_ext.to_be_bytes());
    aad[20] = EXTENDED_V1;
    aad[21] = outer.wire_flags();
    aad[22..26].copy_from_slice(&outer.pn_trunc.to_be_bytes());
    aad
}
