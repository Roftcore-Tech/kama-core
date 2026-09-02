//! DR-FEC-WORKLOAD-1 (RATIFIED) — canonical 48-symbol workload pins (§3). HARNESS-ONLY; result-bearing-DEĞİL.
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_workload.rs"]
mod fs_workload;
use fs_workload::*;

/// §3 — EXACT offsets + actual_payload_len + canonical zero-pad + final-flag (300B → 6 symbol/unit).
#[test]
fn workload_fragment_offsets_lengths_padding() {
    let w = canonical_workload();
    assert_eq!(w.len(), TOTAL_SOURCE_SYMBOLS, "48 symbol beklenir");
    // İlk app-unit'in 6 fragmenti: offsets {0,53,106,159,212,265}; len {53×5, 35}.
    let want = [
        (0u32, 53u16),
        (53, 53),
        (106, 53),
        (159, 53),
        (212, 53),
        (265, 35),
    ];
    for (f, &(off, len)) in want.iter().enumerate() {
        let s = &w[f];
        assert_eq!(s.chunk_offset, off, "fragment {f} offset");
        assert_eq!(s.actual_payload_len, len, "fragment {f} len");
        assert_eq!(s.is_final, f == 5, "final-flag yalnız son-fragment");
        // canonical zero-pad: son-symbol'de payload-sonrası (11+len..64) hepsi 0.
        let pad_start = 11 + len as usize;
        assert!(
            s.symbol_bytes[pad_start..].iter().all(|&b| b == 0),
            "fragment {f} canonical_zero_pad sıfır-değil"
        );
        assert_eq!(s.symbol_bytes.len(), E, "symbol E=64 olmalı");
    }
}

/// §3 — EIGHT DISTINCT application message identities.
#[test]
fn workload_eight_distinct_message_ids() {
    let w = canonical_workload();
    let ids: std::collections::BTreeSet<u32> = w.iter().map(|s| s.message_id).collect();
    assert_eq!(ids.len(), APP_UNITS, "8 distinct message_id beklenir");
    // Her app-unit'in 6 fragmenti AYNI message_id (unit-major grup).
    for u in 0..APP_UNITS {
        let cnt = w
            .iter()
            .filter(|s| s.message_id == message_id_of(u))
            .count();
        assert_eq!(cnt, FRAGS_PER_UNIT, "unit {u} → 6 fragment");
    }
}

/// §3 — serialize/parse EQUALITY (production SourcePrefix round-trip).
#[test]
fn workload_fragment_serialize_parse_equality() {
    use kama_transport_schema::prefix::SourcePrefix;
    let w = canonical_workload();
    for s in &w {
        let pfx = SourcePrefix::parse(&s.symbol_bytes).unwrap();
        assert_eq!(pfx.message_id, s.message_id);
        assert_eq!(pfx.chunk_offset, s.chunk_offset);
        assert_eq!(pfx.actual_payload_len, s.actual_payload_len);
    }
}

/// §3 — FULL 2400-B reassembly byte-EQUALITY (fragment → reassemble == original app-units).
#[test]
fn workload_full_reassembly_byte_equality() {
    let w = canonical_workload();
    let original = app_units();
    let reassembled = reassemble(&w);
    assert_eq!(reassembled.len(), APP_UNITS);
    let total: usize = reassembled.iter().map(Vec::len).sum();
    assert_eq!(total, TOTAL_APP_BYTES, "toplam 2400B reassemble");
    for (u, (orig, rec)) in original.iter().zip(reassembled.iter()).enumerate() {
        assert_eq!(rec.len(), APP_UNIT_BYTES, "unit {u} 300B");
        assert_eq!(rec, orig, "unit {u} reassembly byte-equal DEĞİL");
    }
}

/// §5 / RULE-4 — global_source_fragment_id STABLE 0..47, unit-major, partition-independent.
#[test]
fn workload_global_ids_stable_and_dense() {
    let a = canonical_workload();
    let b = canonical_workload();
    assert_eq!(
        sequence_hash(&a),
        sequence_hash(&b),
        "canonical-sequence deterministik-değil"
    );
    let gids: Vec<u16> = a.iter().map(|s| s.global_source_fragment_id).collect();
    let want: Vec<u16> = (0..48u16).collect();
    assert_eq!(
        gids, want,
        "global_source_fragment_id 0..47 dense/sıralı olmalı"
    );
    // unit-major: global 0..5 → unit0, 6..11 → unit1, ...
    for (gid, s) in a.iter().enumerate() {
        assert_eq!(
            s.app_unit_index as usize,
            gid / FRAGS_PER_UNIT,
            "gid {gid} unit-major-değil"
        );
        assert_eq!(s.fragment_in_unit as usize, gid % FRAGS_PER_UNIT);
    }
}

/// §2 — capacity + geometry sabitleri (E-11=53; 8×6=48; total 2400).
#[test]
fn workload_geometry_constants() {
    assert_eq!(
        APP_CAP, 53,
        "app_capacity_per_symbol = E(64) - SOURCE_PREFIX_LEN(11)"
    );
    assert_eq!(FRAGS_PER_UNIT, 6);
    assert_eq!(TOTAL_SOURCE_SYMBOLS, 48);
    assert_eq!(TOTAL_APP_BYTES, 2400);
}
