//! Commit-P — FUZZY RULE-COVERAGE PREPASS / 11.1 MATEMATİKSEL-INPUT-SPACE-MAP (coverage-AUDIT;
//! KALİBRASYON-DEĞİL; eşik/kural-DEĞİŞTİRİLMEZ; network-realisability-İDDİASI-YOK — 11.2-realizability
//! ayrı-tablo, MS1+fuzzy_e2e-ölçümlerinden).
//!
//! YÖNTEM-ŞERHİ: `decide()` private → harita public-`fuzzify`/`log2_fp` + kaynaktan-birebir-türetilmiş
//! kural-birleşim-formülleriyle (R1..R4/Mamdani-min/weighted-avg/no-rule-fallback/emergency-min3)
//! hesaplanır. SÜRÜKLENME-RİSKİ 3-GERÇEK-YOL-ÇAPA-NOKTASIYLA-sınırlandı (`anchor_validation`-testi:
//! fuzzy_e2e-ölçülen-inputs → beklenen-motor-çıktısı birebir). Kalıcı-çözüm (decide-pub-görünürlüğü)
//! ayrı-küçük-DR-adayı — bu-turda-üretim-koduna-DOKUNULMADI.

use adaptive_transport::fusion::{log2_fp, FuzzyConfig};
use std::io::Write as _;

const PERMILLE: u32 = 1000;

fn min3(a: u32, b: u32, c: u32) -> u32 {
    a.min(b).min(c)
}

fn weighted_avg(pairs: &[(u32, u32)]) -> u32 {
    let (mut num, mut den) = (0u64, 0u64);
    for &(w, v) in pairs {
        num += u64::from(w) * u64::from(v);
        den += u64::from(w);
    }
    num.checked_div(den)
        .map_or(0, |v| u32::try_from(v).unwrap())
}

/// Kaynak-formüllerin-birebir-kopyası (fusion.rs decide §2/§4): (r1..r4, no_rule, ratio, emergency_w).
fn eval(cfg: &FuzzyConfig, loss_ppm: u64, bw_bps: u64, jitter_ppm: u64, ackconf: u64) -> Eval {
    let l = cfg.loss.fuzzify(loss_ppm);
    let b = cfg.bandwidth.fuzzify(log2_fp(bw_bps));
    let j = cfg.jitter.fuzzify(jitter_ppm);
    let a = cfg.ack_confidence.fuzzify(ackconf);
    let r1 = l.low;
    let r2 = min3(l.high, b.high, j.low);
    let r3 = l.med.min(b.med);
    let r4 = l.low.min(j.high);
    let no_rule = r1 == 0 && r2 == 0 && r3 == 0 && r4 == 0;
    let ratio = if no_rule {
        cfg.fec_zero
    } else {
        weighted_avg(&[
            (r1, cfg.fec_zero),
            (r2, cfg.fec_high),
            (r3, cfg.fec_med),
            (r4, cfg.fec_zero),
        ])
    };
    let emergency_w = min3(l.high, b.low, a.low);
    Eval {
        r: [r1, r2, r3, r4],
        no_rule,
        ratio,
        emergency: emergency_w > PERMILLE / 2,
    }
}

struct Eval {
    r: [u32; 4],
    no_rule: bool,
    ratio: u32,
    emergency: bool,
}

/// ÇAPA-DOĞRULAMA (CI'da-koşar): fuzzy_e2e-GERÇEK-YOL-ölçüm-noktalarında analitik-harita == motor-çıktısı.
#[test]
fn anchor_validation_against_real_path() {
    let cfg = FuzzyConfig::fixture();
    // Çapa-1 (fuzzy_e2e-4.1 ölçülen): loss=592145 bw=800000 jit=1312 ackconf=1000 → ratio=800, R2-tek.
    let e = eval(&cfg, 592_145, 800_000, 1_312, 1000);
    assert!(!e.no_rule && e.r[1] > 0 && e.r[0] == 0 && e.r[2] == 0);
    assert_eq!(e.ratio, 800, "çapa-1: motor-ratio=800 (E2E-ölçümü)");
    assert!(!e.emergency);
    // Çapa-2 (4.3): loss=659090 bw=80000 → no_rule (motor: no_rule=true, LITERAL-0-latch).
    let e = eval(&cfg, 659_090, 80_000, 100_000, 1000);
    assert!(e.no_rule, "çapa-2: no_rule-sınıfı");
    // Çapa-3 (4.4): loss=787878 bw=26666 ackconf=393 → emergency ∧ no_rule (örtüşme-bulgusu).
    let e = eval(&cfg, 787_878, 26_666, 100_000, 393);
    assert!(
        e.emergency && e.no_rule,
        "çapa-3: emergency∧no_rule-birlikte"
    );
}

/// 11.1 MAP (ignored; RCM_OUT'a-CSV): global-coverage + loss×bw-dilimleri + örtüşme-nicelleştirmesi.
#[test]
#[ignore]
fn rule_coverage_map() {
    let out = std::env::var("RCM_OUT").expect("RCM_OUT");
    std::fs::create_dir_all(&out).unwrap();
    let cfg = FuzzyConfig::fixture();
    let losses: Vec<u64> = (0..=50).map(|i| i * 20_000).collect(); // 0..1M
    let bws: Vec<u64> = vec![
        500, 1_000, 2_000, 5_000, 10_000, 20_000, 30_000, 40_000, 50_000, 60_000, 75_000, 90_000,
        100_000, 120_000, 150_000, 175_000, 200_000, 250_000, 300_000, 500_000, 1_000_000,
    ];
    let jits: Vec<u64> = vec![
        0, 50_000, 100_000, 150_000, 200_000, 300_000, 400_000, 500_000, 600_000,
    ];
    let acks: Vec<u64> = vec![0, 200, 300, 400, 500, 700, 900, 1000];
    let (mut n, mut c_r): (u64, [u64; 4]) = (0, [0; 4]);
    let (mut c_norule, mut c_emerg, mut c_both, mut c_overlap2) = (0u64, 0u64, 0u64, 0u64);
    let mut c_emerg_nonzero_ratio = 0u64;
    let mut ratio_hist: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    for &lp in &losses {
        for &bw in &bws {
            for &jp in &jits {
                for &ac in &acks {
                    let e = eval(&cfg, lp, bw, jp, ac);
                    n += 1;
                    for (cnt, &w) in c_r.iter_mut().zip(e.r.iter()) {
                        if w > 0 {
                            *cnt += 1;
                        }
                    }
                    if e.r.iter().filter(|&&w| w > 0).count() >= 2 {
                        c_overlap2 += 1;
                    }
                    if e.no_rule {
                        c_norule += 1;
                    }
                    if e.emergency {
                        c_emerg += 1;
                        if e.no_rule {
                            c_both += 1;
                        } else if e.ratio > 0 {
                            c_emerg_nonzero_ratio += 1;
                        }
                    }
                    *ratio_hist.entry(e.ratio).or_insert(0) += 1;
                }
            }
        }
    }
    let mut g = std::fs::File::create(format!("{out}/rcm-global.csv")).unwrap();
    writeln!(g, "metric,count,fraction").unwrap();
    let frac = |c: u64| c as f64 / n as f64;
    writeln!(g, "points,{n},1.0").unwrap();
    for (i, c) in c_r.iter().enumerate() {
        writeln!(g, "R{}_active,{c},{:.4}", i + 1, frac(*c)).unwrap();
    }
    writeln!(g, "overlap_ge2_rules,{c_overlap2},{:.4}", frac(c_overlap2)).unwrap();
    writeln!(g, "no_rule,{c_norule},{:.4}", frac(c_norule)).unwrap();
    writeln!(g, "emergency_mode,{c_emerg},{:.4}", frac(c_emerg)).unwrap();
    writeln!(g, "no_rule_AND_emergency,{c_both},{:.4}", frac(c_both)).unwrap();
    writeln!(
        g,
        "emergency_frac_of_which_no_rule,,{:.4}",
        c_both as f64 / c_emerg.max(1) as f64
    )
    .unwrap();
    writeln!(
        g,
        "emergency_AND_nonzero_ratio,{c_emerg_nonzero_ratio},{:.4}",
        frac(c_emerg_nonzero_ratio)
    )
    .unwrap();
    for (r, c) in &ratio_hist {
        writeln!(g, "ratio_{r},{c},{:.4}", frac(*c)).unwrap();
    }
    // loss×bw-dilimi (nominal: jit=100k qd-FEC-kuralı-yok ack=1000) + emergency-dilimi (ack=300).
    let mut s = std::fs::File::create(format!("{out}/rcm-loss-bw-slice.csv")).unwrap();
    writeln!(
        s,
        "slice,loss_ppm,bw_bps,r1,r2,r3,r4,no_rule,ratio,emergency"
    )
    .unwrap();
    for (slice, ack) in [("nominal_ack1000", 1000u64), ("lowconf_ack300", 300)] {
        for &lp in &losses {
            for &bw in &bws {
                let e = eval(&cfg, lp, bw, 100_000, ack);
                writeln!(
                    s,
                    "{slice},{lp},{bw},{},{},{},{},{},{},{}",
                    e.r[0],
                    e.r[1],
                    e.r[2],
                    e.r[3],
                    u8::from(e.no_rule),
                    e.ratio,
                    u8::from(e.emergency)
                )
                .unwrap();
            }
        }
    }
    eprintln!("[rcm] yazıldı: {out}/rcm-global.csv + rcm-loss-bw-slice.csv (n={n})");
}
