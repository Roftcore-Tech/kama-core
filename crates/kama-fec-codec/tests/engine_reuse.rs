//! Sprint-3: RsSimdEngine (engine-reuse) == RsSimdCodec (stateless) == golden byte-eşitliği +
//! profil-değişimli reset (allocation-reuse doğruluğu) + E=1024 profil roundtrip.

use kama_fec_codec::{
    by_id, FecCodec, ReceivedSymbol, RsSimdCodec, RsSimdEngine, SourceSymbol, PROFILES,
};

fn mk_sources(k: usize, e: usize) -> Vec<Vec<u8>> {
    (0..k)
        .map(|i| {
            (0..e)
                .map(|j| (i as u8).wrapping_mul(31).wrapping_add((j & 0xFF) as u8))
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

// ── reuse == stateless: her profil encode byte-eş ──
#[test]
fn reuse_encode_equals_stateless_all_profiles() {
    let stateless = RsSimdCodec;
    let engine = RsSimdEngine::new();
    for p in PROFILES {
        let data = mk_sources(p.k_shape_usize(), p.e_usize());
        let a = stateless.encode(p, &src_syms(&data)).unwrap();
        let b = engine.encode(p, &src_syms(&data)).unwrap();
        assert_eq!(a, b, "profil 0x{:02x} reuse!=stateless", p.profile_id);
    }
}

// ── profil-değişimli reset: A→B→A aynı engine, hepsi doğru (reset allocation-reuse) ──
#[test]
fn reuse_across_profile_switches() {
    let engine = RsSimdEngine::new();
    let stateless = RsSimdCodec;
    let ids = [0x01u8, 0x03, 0x05, 0x02, 0x01, 0x06, 0x03];
    for id in ids {
        let p = by_id(id).unwrap();
        let data = mk_sources(p.k_shape_usize(), p.e_usize());
        let want = stateless.encode(&p, &src_syms(&data)).unwrap();
        let got = engine.encode(&p, &src_syms(&data)).unwrap();
        assert_eq!(got, want, "reset sonrası profil 0x{id:02x} byte-eş değil");
    }
}

// ── reuse reconstruct == stateless reconstruct (erasure recovery byte-eş) ──
#[test]
fn reuse_reconstruct_equals_stateless() {
    let stateless = RsSimdCodec;
    let engine = RsSimdEngine::new();
    for id in [0x03u8, 0x05] {
        let p = by_id(id).unwrap();
        let k = p.k_shape_usize();
        let e = p.e_usize();
        let data = mk_sources(k, e);
        let reps = stateless.encode(&p, &src_syms(&data)).unwrap();
        // esi 1,2 düş; 2 repair admit.
        let mut recv: Vec<ReceivedSymbol> = data
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 1 && *i != 2)
            .map(|(i, b)| ReceivedSymbol {
                esi: i as u16,
                is_repair: false,
                bytes: b,
            })
            .collect();
        recv.push(ReceivedSymbol {
            esi: 0,
            is_repair: true,
            bytes: &reps[0].bytes,
        });
        recv.push(ReceivedSymbol {
            esi: 1,
            is_repair: true,
            bytes: &reps[1].bytes,
        });
        let a = stateless.reconstruct(&p, &recv, 0).unwrap();
        let b = engine.reconstruct(&p, &recv, 0).unwrap();
        assert_eq!(a, b, "profil 0x{id:02x} reconstruct reuse!=stateless");
        // recovered doğru byte
        assert_eq!(a.iter().find(|r| r.esi == 1).unwrap().bytes, data[1]);
        assert_eq!(a.iter().find(|r| r.esi == 2).unwrap().bytes, data[2]);
    }
}

// ── E=1024 profil (0x05 K8R4E1024): sub-kbps büyük-E roundtrip + erasure recovery ──
#[test]
fn e1024_profile_roundtrip() {
    let engine = RsSimdEngine::new();
    let p = by_id(0x05).unwrap();
    assert_eq!(p.e_usize(), 1024);
    let data = mk_sources(8, 1024);
    let reps = engine.encode(&p, &src_syms(&data)).unwrap();
    assert_eq!(reps.len(), 4);
    assert_eq!(reps[0].bytes.len(), 1024);
    // 3 source düş, 3 repair ile kurtar (K=8, R=4).
    let mut recv: Vec<ReceivedSymbol> = data
        .iter()
        .enumerate()
        .filter(|(i, _)| ![0usize, 3, 6].contains(i))
        .map(|(i, b)| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: b,
        })
        .collect();
    for (j, rep) in reps.iter().take(3).enumerate() {
        recv.push(ReceivedSymbol {
            esi: j as u16,
            is_repair: true,
            bytes: &rep.bytes,
        });
    }
    let rec = engine.reconstruct(&p, &recv, 0).unwrap();
    assert_eq!(rec.len(), 3);
    for &i in &[0usize, 3, 6] {
        assert_eq!(
            rec.iter().find(|r| r.esi == i as u16).unwrap().bytes,
            data[i]
        );
    }
}

// ── E=1024 wire-overhead oranı: sealed FEC-shard = 26+11+1024 = 1061B, sabit-overhead %3.5 ──
#[test]
fn e1024_overhead_ratio() {
    let e = 1024usize;
    let sealed = 26 + 11 + e; // outer + inner-hdr + symbol
    let overhead = 26 + 11;
    let ratio = overhead as f64 / sealed as f64;
    assert!(
        ratio < 0.04,
        "E=1024 sabit-overhead %4'ün altında olmalı: {ratio}"
    );
    // E=64 karşılaştırması (%36.6)
    let ratio64 = 37.0 / (26.0 + 11.0 + 64.0);
    assert!(ratio64 > 0.36);
}
