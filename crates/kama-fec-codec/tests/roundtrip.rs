//! RS-çekirdek POC (fec-core §15/§5.1): encode/decode roundtrip · source/repair/mixed-erasure ·
//! duplicate-ESI reject · wrong-count fail-closed · deterministik-repair (golden) · profile-alignment.

use kama_fec_codec::{
    by_id, FecCodec, FecError, FecProfile, ReceivedSymbol, RsSimdCodec, SourceSymbol, PROFILES,
};

fn mk_sources(k: usize, e: usize) -> Vec<Vec<u8>> {
    // Deterministik source-set: her shard esi ile damgalı, kalan pattern.
    (0..k)
        .map(|i| {
            (0..e)
                .map(|j| (i as u8).wrapping_mul(31).wrapping_add(j as u8))
                .collect()
        })
        .collect()
}

fn src_syms(data: &[Vec<u8>]) -> Vec<SourceSymbol<'_>> {
    data.iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect()
}

#[test]
fn roundtrip_full_no_loss() {
    let c = RsSimdCodec;
    for p in PROFILES {
        let data = mk_sources(p.k_shape_usize(), p.e_usize());
        let repairs = c.encode(p, &src_syms(&data)).unwrap();
        assert_eq!(
            repairs.len(),
            p.r_code_max as usize,
            "R_code_max repair üretildi"
        );
        // Tüm-source geldi → reconstruct boş (kayıp yok).
        let recv: Vec<_> = data
            .iter()
            .enumerate()
            .map(|(i, b)| ReceivedSymbol {
                esi: i as u16,
                is_repair: false,
                bytes: b,
            })
            .collect();
        assert!(c.reconstruct(p, &recv, 0).unwrap().is_empty());
    }
}

#[test]
fn source_erasure_recovered_by_repair() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8, R=4, E=64
    let data = mk_sources(8, 64);
    let repairs = c.encode(&p, &src_syms(&data)).unwrap();
    // 2 source düş (esi 2,5); yerine 2 repair admit → decode-koşulu 6+2 ≥ 8.
    let mut recv: Vec<ReceivedSymbol> = data
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 2 && *i != 5)
        .map(|(i, b)| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: b,
        })
        .collect();
    recv.push(ReceivedSymbol {
        esi: 0,
        is_repair: true,
        bytes: &repairs[0].bytes,
    });
    recv.push(ReceivedSymbol {
        esi: 1,
        is_repair: true,
        bytes: &repairs[1].bytes,
    });
    let rec = c.reconstruct(&p, &recv, 0).unwrap();
    assert_eq!(rec.len(), 2);
    // Reconstruct edilen source'lar orijinaliyle BYTE-BYTE aynı.
    assert_eq!(rec.iter().find(|r| r.esi == 2).unwrap().bytes, data[2]);
    assert_eq!(rec.iter().find(|r| r.esi == 5).unwrap().bytes, data[5]);
}

#[test]
fn mixed_erasure_all_repair() {
    let c = RsSimdCodec;
    let p = by_id(0x02).unwrap(); // K=4, R=4
    let data = mk_sources(4, 64);
    let repairs = c.encode(&p, &src_syms(&data)).unwrap();
    // Tüm-source düş, 4 repair ile decode (K=4 ≤ 4 repair).
    let recv: Vec<ReceivedSymbol> = repairs
        .iter()
        .map(|r| ReceivedSymbol {
            esi: r.esi,
            is_repair: true,
            bytes: &r.bytes,
        })
        .collect();
    let rec = c.reconstruct(&p, &recv, 0).unwrap();
    assert_eq!(rec.len(), 4);
    for (i, d) in data.iter().enumerate() {
        assert_eq!(&rec.iter().find(|r| r.esi == i as u16).unwrap().bytes, d);
    }
}

#[test]
fn deterministic_repair_golden() {
    // Aynı source-set → aynı repair-byte (iki-bağımsız-encode; golden-drift-gate temeli).
    let c = RsSimdCodec;
    let p = by_id(0x01).unwrap();
    let data = mk_sources(2, 64);
    let r1 = c.encode(&p, &src_syms(&data)).unwrap();
    let r2 = c.encode(&p, &src_syms(&data)).unwrap();
    assert_eq!(r1, r2, "deterministik repair (RS sistematik)");
}

#[test]
fn duplicate_esi_reject() {
    let c = RsSimdCodec;
    let p = by_id(0x02).unwrap();
    let data = mk_sources(4, 64);
    let mut recv: Vec<ReceivedSymbol> = data
        .iter()
        .enumerate()
        .map(|(i, b)| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: b,
        })
        .collect();
    recv.push(ReceivedSymbol {
        esi: 0,
        is_repair: false,
        bytes: &data[0],
    }); // dup
    assert!(matches!(
        c.reconstruct(&p, &recv, 0),
        Err(FecError::DuplicateEsi(0))
    ));
}

#[test]
fn wrong_source_count_fail_closed() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8
    let data = mk_sources(7, 64); // eksik
    assert!(matches!(
        c.encode(&p, &src_syms(&data)),
        Err(FecError::SourceCount {
            got: 7,
            expected: 8
        })
    ));
}

#[test]
fn wrong_symbol_len_fail_closed() {
    let c = RsSimdCodec;
    let p = by_id(0x01).unwrap(); // E=64
    let bad = vec![vec![0u8; 32], vec![0u8; 32]];
    assert!(matches!(
        c.encode(&p, &src_syms(&bad)),
        Err(FecError::SymbolLen {
            got: 32,
            expected: 64
        })
    ));
}

#[test]
fn insufficient_shards_fail_closed() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8
    let data = mk_sources(8, 64);
    // Yalnız 3 source, repair yok → 3 < 8 → Insufficient.
    let recv: Vec<ReceivedSymbol> = data
        .iter()
        .enumerate()
        .take(3)
        .map(|(i, b)| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: b,
        })
        .collect();
    assert!(matches!(
        c.reconstruct(&p, &recv, 0),
        Err(FecError::Insufficient { .. })
    ));
}

#[test]
fn profile_validation_bounds() {
    let bad = FecProfile {
        profile_id: 9,
        code_suite_id: 1,
        k_shape: 256,
        r_code_max: 2,
        e: 64,
    };
    assert!(matches!(bad.validate(), Err(FecError::ProfileBound(_))));
    let odd_e = FecProfile {
        profile_id: 9,
        code_suite_id: 1,
        k_shape: 4,
        r_code_max: 2,
        e: 63,
    };
    assert!(matches!(odd_e.validate(), Err(FecError::ProfileBound(_))));
    let tiny_e = FecProfile {
        profile_id: 9,
        code_suite_id: 1,
        k_shape: 4,
        r_code_max: 2,
        e: 10,
    };
    assert!(matches!(tiny_e.validate(), Err(FecError::ProfileBound(_))));
}
