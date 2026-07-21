//! M4 — encode/decode µs/block + reset-reuse ölçümü → RsSimdEngine TUT/AT karar-verisi.
//! ÖLÇÜM-testi (#[ignore]; `--ignored --nocapture` ile koşulur; release-mode önerilir).
//! Teşhis-seviyesi (yerel-makine; PR-gate DEĞİL — ölçüm-teşhis-doğrulama-ayrımı).

use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, RsSimdEngine, SourceSymbol};
use std::time::Instant;

fn mk_sources(k: usize, e: usize) -> Vec<Vec<u8>> {
    (0..k)
        .map(|i| {
            (0..e)
                .map(|j| (i as u8).wrapping_mul(31).wrapping_add((j & 0xFF) as u8))
                .collect()
        })
        .collect()
}

fn pct(v: &[u128], p: f64) -> u128 {
    let mut s = v.to_vec();
    s.sort_unstable();
    s[((s.len() as f64 - 1.0) * p) as usize]
}

fn bench_encode<C: FecCodec>(codec: &C, profile_id: u8, iters: usize) -> (u128, u128, u128) {
    let p = by_id(profile_id).unwrap();
    let data = mk_sources(p.k_shape as usize, p.e_usize());
    let srcs: Vec<SourceSymbol> = data
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let mut times = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t = Instant::now();
        let r = codec.encode(&p, &srcs).unwrap();
        times.push(t.elapsed().as_nanos());
        std::hint::black_box(r);
    }
    (pct(&times, 0.50), pct(&times, 0.95), pct(&times, 0.99))
}

fn bench_reconstruct<C: FecCodec>(codec: &C, profile_id: u8, iters: usize) -> (u128, u128, u128) {
    let p = by_id(profile_id).unwrap();
    let k = p.k_shape as usize;
    let data = mk_sources(k, p.e_usize());
    let srcs: Vec<SourceSymbol> = data
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let repairs = RsSimdCodec.encode(&p, &srcs).unwrap();
    // 2-source-kayıp (esi 1,3) + 2-repair → recovery-decode (tam-blok k_actual=K, known_zero=0).
    let mut recv: Vec<ReceivedSymbol> = Vec::new();
    for (i, b) in data.iter().enumerate() {
        if i != 1 && i != 3 {
            recv.push(ReceivedSymbol {
                esi: i as u16,
                is_repair: false,
                bytes: b,
            });
        }
    }
    for rep in repairs.iter().take(2) {
        recv.push(ReceivedSymbol {
            esi: rep.esi,
            is_repair: true,
            bytes: &rep.bytes,
        });
    }
    let mut times = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t = Instant::now();
        let r = codec.reconstruct(&p, &recv, 0).unwrap();
        times.push(t.elapsed().as_nanos());
        std::hint::black_box(r);
    }
    (pct(&times, 0.50), pct(&times, 0.95), pct(&times, 0.99))
}

#[test]
#[ignore]
fn m4_timing_table() {
    const N: usize = 2000;
    println!("profil,op,motor,p50_us,p95_us,p99_us");
    for pid in [0x03u8, 0x05u8] {
        // K8R4E64 · K8-E1024
        let fresh_e = bench_encode(&RsSimdCodec, pid, N);
        let eng = RsSimdEngine::new();
        let reuse_e = bench_encode(&eng, pid, N);
        let fresh_d = bench_reconstruct(&RsSimdCodec, pid, N);
        let eng2 = RsSimdEngine::new();
        let reuse_d = bench_reconstruct(&eng2, pid, N);
        for (op, motor, (a, b, c)) in [
            ("encode", "fresh", fresh_e),
            ("encode", "reuse", reuse_e),
            ("decode2loss", "fresh", fresh_d),
            ("decode2loss", "reuse", reuse_d),
        ] {
            println!(
                "0x{pid:02x},{op},{motor},{:.1},{:.1},{:.1}",
                a as f64 / 1000.0,
                b as f64 / 1000.0,
                c as f64 / 1000.0
            );
        }
    }
}
