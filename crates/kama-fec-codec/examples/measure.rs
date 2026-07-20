//! FEC-POC İLK-ÖLÇÜM harness'i (teşhis-seviyesi — netem/criterion PR-gate DEĞİL; ölçüm-disiplini).
//! encode/reconstruct throughput + recompute-vs-full-encode (§6 POC-ölçer). Tek-koşu, indicative.

use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, RsSimdEngine, SourceSymbol};
use std::time::Instant;

fn main() {
    let c = RsSimdCodec;
    let engine = RsSimdEngine::new();
    let iters = 20_000u32;
    println!("# FEC-POC ölçüm (indicative, {iters} iter/profil, tek-koşu)");
    println!(
        "profil        K  R  E  | enc(µs) enc-MB/s | recon-STATELESS(µs) | recon-REUSE(µs) | reuse-hızlanma | recompute/enc"
    );
    // Sprint-3: RsSimdEngine reset()-reuse → reconstruct decoder-inşa-baskınlığını giderir (D0-ölçüm).
    for pid in [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06] {
        let p = by_id(pid).unwrap();
        let k = p.k_shape_usize();
        let e = p.e_usize();
        let r = p.r_code_max as usize;
        let block: Vec<Vec<u8>> = (0..k)
            .map(|i| {
                (0..e)
                    .map(|j| (i as u8).wrapping_mul(31).wrapping_add(j as u8))
                    .collect()
            })
            .collect();
        let src: Vec<SourceSymbol> = block
            .iter()
            .enumerate()
            .map(|(i, b)| SourceSymbol {
                esi: i as u16,
                bytes: b,
            })
            .collect();

        // warmup (SIMD/once_cell tablo + cache ısıt)
        for _ in 0..500 {
            let _ = c.encode(&p, &src).unwrap();
        }
        // encode
        let t = Instant::now();
        let mut reps = Vec::new();
        for _ in 0..iters {
            reps = c.encode(&p, &src).unwrap();
        }
        let enc_ns = t.elapsed().as_nanos() as f64 / f64::from(iters);
        let block_bytes = (k * e) as f64;
        let enc_mbps = block_bytes / (enc_ns / 1e9) / 1e6;

        // reconstruct: R source düş, R repair admit (worst-case decode).
        let lost = r.min(k);
        let mut recv: Vec<ReceivedSymbol> = block
            .iter()
            .enumerate()
            .skip(lost)
            .map(|(i, b)| ReceivedSymbol {
                esi: i as u16,
                is_repair: false,
                bytes: b,
            })
            .collect();
        for rr in reps.iter().take(lost) {
            recv.push(ReceivedSymbol {
                esi: rr.esi,
                is_repair: true,
                bytes: &rr.bytes,
            });
        }
        // reconstruct STATELESS (her-çağrı taze-decoder).
        let t = Instant::now();
        for _ in 0..iters {
            let _ = c.reconstruct(&p, &recv, 0).unwrap();
        }
        let rec_ns = t.elapsed().as_nanos() as f64 / f64::from(iters);

        // reconstruct REUSE (RsSimdEngine reset()) — warmup ilk-reset kurar.
        for _ in 0..100 {
            let _ = engine.reconstruct(&p, &recv, 0).unwrap();
        }
        let t = Instant::now();
        for _ in 0..iters {
            let _ = engine.reconstruct(&p, &recv, 0).unwrap();
        }
        let reuse_ns = t.elapsed().as_nanos() as f64 / f64::from(iters);
        let speedup = rec_ns / reuse_ns;

        // recompute tek-ESI vs full-encode.
        let t = Instant::now();
        for _ in 0..iters {
            let _ = c.recompute_repair(&p, &src, 0).unwrap();
        }
        let recompute_ns = t.elapsed().as_nanos() as f64 / f64::from(iters);
        let ratio = recompute_ns / enc_ns;

        println!(
            "0x{pid:02x} K{k:<2}R{r}E{e:<4}| {:7.3} {enc_mbps:8.1} | {:19.3} | {:15.3} | {speedup:13.1}× | {ratio:12.2}x",
            enc_ns / 1000.0,
            rec_ns / 1000.0,
            reuse_ns / 1000.0
        );
    }
    // wire-byte tablosu (deterministik — §2.6 constants).
    println!("\n# sealed-wire-byte (§2.6: outer 26 + FEC-inner-hdr 11 + symbol E)");
    println!("profil  E  | sealed_FEC_shard(B) | app-source-payload(E-11) | IPv4-datagram | IPv6-datagram");
    for pid in [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06] {
        let p = by_id(pid).unwrap();
        let e = p.e_usize();
        let sealed = 26 + 11 + e;
        let payload = e - 11;
        println!(
            "0x{pid:02x}   {e:<3}| {sealed:19} | {payload:24} | {:13} | {:13}",
            28 + sealed,
            48 + sealed
        );
    }
}
