//! KATMAN-2 (K-WIRE-NONCE96-1): full_pn → EXACT nonce96 byte-mapping (§4.5, DR-4-CLOSED KOŞUL-A/LE64).
//! Karar-paketi BÖLÜM-2 elle-yazılmış örnek-byte'ları DOĞRUDAN assert edilir (self-golden-DEĞİL: spec-elle-yazımı).

use kama_noise_wire::nonce96;

// §4.5 ÖRNEK-BYTE'LAR (LE64; nonce[0..4]=zero, nonce[4..12]=LE64(full_pn)).
#[test]
fn nonce96_le64_exact_vectors() {
    assert_eq!(
        nonce96(0),
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        "full_pn=0"
    );
    assert_eq!(
        nonce96(1),
        [0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        "full_pn=1 → LE: 01 sonra 7×00"
    );
    assert_eq!(
        nonce96(1u64 << 32),
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
        "full_pn=2^32"
    );
    assert_eq!(
        nonce96(1u64 << 63),
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80],
        "full_pn=2^63"
    );
}

// Zero-prefix INVARIANT: ilk 4 byte HER ZAMAN sıfır (32-bit zero prefix).
#[test]
fn nonce96_zero_prefix_invariant() {
    for pn in [0u64, 1, 42, u32::MAX as u64, 1 << 40, u64::MAX] {
        assert_eq!(&nonce96(pn)[0..4], &[0, 0, 0, 0], "pn={pn} zero-prefix");
    }
}

// LE-ayrım: BE olsaydı full_pn=1 → son-byte=01; LE'de ilk-payload-byte=01 (BE-DEĞİL-teyidi).
#[test]
fn nonce96_is_le_not_be() {
    let n = nonce96(1);
    assert_eq!(n[4], 0x01, "LE: düşük-byte önce (offset 4)");
    assert_eq!(
        n[11], 0x00,
        "LE: yüksek-byte sonra (offset 11) — BE olsaydı 0x01 olurdu"
    );
}

// Mapping injective (farklı pn → farklı nonce; nonce-uniqueness temeli).
#[test]
fn nonce96_injective() {
    let a = nonce96(0x0102_0304_0506_0708);
    let b = nonce96(0x0102_0304_0506_0709);
    assert_ne!(a, b);
    // roundtrip: nonce[4..12] LE → full_pn geri
    let mut le = [0u8; 8];
    le.copy_from_slice(&a[4..12]);
    assert_eq!(u64::from_le_bytes(le), 0x0102_0304_0506_0708);
}
