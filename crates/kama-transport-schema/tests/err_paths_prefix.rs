//! TC-05 — `prefix.rs` hata yolları (SR-STR-CORE-001 §5.1, BUL-05).
//!
//! **Kapsanan bulgu:** BUL-05. STR §3.4 `prefix.rs`'te dört sıfır-sayımlı construct-site raporlar:
//! `SymbolLenMismatch`, `BufferTooShort`, `ReservedFlagsSet` ve `PayloadTooLong`'un ikinci site'ları.
//! `prefix.rs` satır kapsamı %78,33 ile workspace'in en düşük ikinci değeridir.
//!
//! **Gerekçe:** Canonical prefix, ADR-05 fixed-width big-endian kodlamasının tek kaynağıdır
//! (§2.6 MADDE-9). Buradaki bir kabul hatası — reserved-bit'li bir `source_flags`'ın geçmesi veya
//! `actual_payload_len`'in kapasiteyi aşmasına izin verilmesi — kayıp-source reconstruct yolunda
//! metadata-recovery'yi bozar ve tüm üst katmanlara yayılır. Bu dosya her iki yönü de (serialize +
//! parse) fail-closed olarak sabitler.
//!
//! **Oracle (tüm vakalar):** Her yol tipli `SchemaError` döndürür; panik yok; kısmi/bozuk
//! `SourcePrefix` üretilmez ve `out` tamponuna byte yazılmaz.

use kama_transport_schema::prefix::{SourcePrefix, SOURCE_FLAG_FINAL};
use kama_transport_schema::{SchemaError, SOURCE_PREFIX_LEN};

const E: usize = 64;
/// §2.6: prefix + payload + zero_pad == E → payload kapasitesi E − 11.
const CAP: usize = E - SOURCE_PREFIX_LEN;

fn ok_prefix(payload_len: u16) -> SourcePrefix {
    SourcePrefix {
        message_id: 0x1122_3344,
        chunk_offset: 0x5566_7788,
        actual_payload_len: payload_len,
        source_flags: SOURCE_FLAG_FINAL,
    }
}

// ── (a) serialize_symbol: ReservedFlagsSet ──────────────────────────────────

#[test]
fn serialize_reserved_flags_set_is_rejected() {
    // §2.6 MADDE-9: SOURCE_FLAG_FINAL dışındaki her bit reserved → reject.
    let mut out = [0u8; E];
    for bit in 1..8u8 {
        let p = SourcePrefix {
            source_flags: 1u8 << bit,
            ..ok_prefix(4)
        };
        let got = p.serialize_symbol(&[0xAA; 4], &mut out);
        assert_eq!(
            got,
            Err(SchemaError::ReservedFlagsSet(1u8 << bit)),
            "reserved bit {bit} kabul edildi"
        );
    }
}

#[test]
fn serialize_reserved_flags_writes_no_bytes() {
    // Fail-closed: reddedilen çağrı tamponu KİRLETMEZ (kısmi-serialize yok).
    let mut out = [0x5Au8; E];
    let p = SourcePrefix {
        source_flags: 0b1000_0000,
        ..ok_prefix(4)
    };
    assert!(p.serialize_symbol(&[0xAA; 4], &mut out).is_err());
    assert!(out.iter().all(|&b| b == 0x5A), "tampon kısmen yazıldı");
}

// ── (b) serialize_symbol: PayloadTooLong ────────────────────────────────────

#[test]
fn serialize_payload_capacity_boundary_is_exact() {
    // Off-by-one kapısı: cap KABUL, cap+1 REDDEDİLİR — ikisi tek testte.
    // NOT: `golden_v0::payload_too_long_fail_closed` PayloadTooLong site'ını zaten kapatıyor
    // (E=16, cap=5). Buradaki değer sınırın İKİ YANINI birlikte pinlemek ve dönen alan
    // değerlerini ({len, cap}) `{ .. }` yerine tam olarak assert etmek.
    let mut out = [0u8; E];

    // (a) cap → Ok, payload aynen yazılır
    let payload = vec![0xAAu8; CAP];
    let p = ok_prefix(CAP as u16);
    assert_eq!(p.serialize_symbol(&payload, &mut out), Ok(()));
    assert_eq!(&out[SOURCE_PREFIX_LEN..], &payload[..]);

    // (b) cap+1 → PayloadTooLong, alan değerleri tam
    let over = vec![0xAAu8; CAP + 1];
    let q = ok_prefix((CAP + 1) as u16);
    assert_eq!(
        q.serialize_symbol(&over, &mut out),
        Err(SchemaError::PayloadTooLong {
            len: CAP + 1,
            cap: CAP
        })
    );
}

#[test]
fn serialize_into_undersized_buffer_saturates_capacity_to_zero() {
    // out.len() < SOURCE_PREFIX_LEN → cap saturating_sub ile 0; boş-olmayan payload → PayloadTooLong.
    // (Bu, `out` çok kısa olduğunda panik yerine tipli-hata alındığının kanıtıdır.)
    let mut out = [0u8; 4];
    let p = ok_prefix(1);
    assert_eq!(
        p.serialize_symbol(&[0xAA], &mut out),
        Err(SchemaError::PayloadTooLong { len: 1, cap: 0 })
    );
}

// ── (c) serialize_symbol: SymbolLenMismatch ─────────────────────────────────

#[test]
fn serialize_declared_len_below_actual_is_mismatch() {
    let mut out = [0u8; E];
    let p = ok_prefix(3); // beyan 3
    assert_eq!(
        p.serialize_symbol(&[0xAA; 8], &mut out), // gerçek 8
        Err(SchemaError::SymbolLenMismatch {
            got: 8,
            expected: 3
        })
    );
}

#[test]
fn serialize_declared_len_above_actual_is_mismatch() {
    let mut out = [0u8; E];
    let p = ok_prefix(8);
    assert_eq!(
        p.serialize_symbol(&[0xAA; 3], &mut out),
        Err(SchemaError::SymbolLenMismatch {
            got: 3,
            expected: 8
        })
    );
}

// ── (d) parse: BufferTooShort ───────────────────────────────────────────────

#[test]
fn parse_below_prefix_len_is_buffer_too_short() {
    // 0..10 byte'ın tamamı reddedilir; 11 (prefix-only, payload 0) kabul edilir.
    for have in 0..SOURCE_PREFIX_LEN {
        let sym = vec![0u8; have];
        assert_eq!(
            SourcePrefix::parse(&sym),
            Err(SchemaError::BufferTooShort {
                need: SOURCE_PREFIX_LEN,
                have
            }),
            "len={have} reddedilmedi"
        );
    }
    let exact = [0u8; SOURCE_PREFIX_LEN];
    assert!(
        SourcePrefix::parse(&exact).is_ok(),
        "tam-prefix (payload 0) kabul edilmeli"
    );
}

// ── (e) parse: ReservedFlagsSet ─────────────────────────────────────────────

#[test]
fn parse_reserved_flags_set_is_rejected() {
    let mut sym = [0u8; E];
    for bit in 1..8u8 {
        sym[10] = 1u8 << bit;
        assert_eq!(
            SourcePrefix::parse(&sym),
            Err(SchemaError::ReservedFlagsSet(1u8 << bit)),
            "parse reserved bit {bit} kabul etti"
        );
    }
}

// ── (f) parse: PayloadTooLong (ikinci site) ─────────────────────────────────

#[test]
fn parse_declared_len_over_capacity_is_too_long() {
    // actual_payload_len = CAP+1 → symbol'e sığmaz. Bu, düşman/bozuk gönderenin
    // payload_slice()'ı taşırmaya çalıştığı yoldur (bounds-before-slice kanıtı).
    let mut sym = [0u8; E];
    sym[8..10].copy_from_slice(&((CAP + 1) as u16).to_be_bytes());
    assert_eq!(
        SourcePrefix::parse(&sym),
        Err(SchemaError::PayloadTooLong {
            len: CAP + 1,
            cap: CAP
        })
    );
}

#[test]
fn parse_declared_len_u16_max_is_too_long_not_panic() {
    // En-kötü-köşe: 0xFFFF beyan. Tipli-hata, panik değil.
    let mut sym = [0u8; E];
    sym[8..10].copy_from_slice(&u16::MAX.to_be_bytes());
    assert_eq!(
        SourcePrefix::parse(&sym),
        Err(SchemaError::PayloadTooLong {
            len: usize::from(u16::MAX),
            cap: CAP
        })
    );
}

#[test]
fn parse_declared_len_exactly_at_capacity_succeeds() {
    let mut sym = [0u8; E];
    sym[8..10].copy_from_slice(&(CAP as u16).to_be_bytes());
    let p = SourcePrefix::parse(&sym).expect("cap sınırı kabul edilmeli");
    assert_eq!(p.actual_payload_len as usize, CAP);
    assert_eq!(p.payload_slice(&sym).len(), CAP);
}

// ── (g) roundtrip + alan-sırası kilidi ──────────────────────────────────────
