//! TV-SEQ golden vektörleri — (expected, wire) → full_counter (engine spec §2).
//!
//! Değerler bu commit'in PR açıklamasında adım adım türetildi (RFC 9000 A.3 @ 16-bit).

use adaptive_transport::seq::resolve_seq;

#[test]
fn tv_seq_golden() {
    // (expected, wire, beklenen full_counter)
    let cases: &[(u64, u16, u64)] = &[
        (65536, 0xFFFF, 65535),   // boundary-altı reorder (131071 DEĞİL)
        (65536, 0x0002, 65538),   // ileri küçük adım
        (65535, 0x0001, 65537),   // boundary-üstü ileri (1 DEĞİL)
        (65536, 0x8000, 98304),   // tam-hwin tie → ileri/yüksek aday
        (196608, 0xFFFE, 196606), // yüksek boundary'de reorder
        (65536, 0x0000, 65536),   // tam isabet
        (0, 0x0000, 0),           // session başı, underflow yok
    ];
    for &(expected, wire, want) in cases {
        assert_eq!(
            resolve_seq(expected, wire),
            want,
            "expected={expected} wire={wire:#06x}"
        );
    }
}
