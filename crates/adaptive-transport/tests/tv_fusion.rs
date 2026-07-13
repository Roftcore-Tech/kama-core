//! TV-FUZZY golden vektörleri — fuzzy karar motoru (§8.4/§15). Eşikler fixture
//! (KALİBRE DEĞİL, netem §598); golden motor matematiğini + kural tablosunu kilitler.
//! Değerler PR açıklamasında adım adım türetildi.

use adaptive_transport::fusion::{
    log2_fp, trap_permille, DeterministicFuzzyController, FusionController, FusionInputs,
    FuzzyConfig, Mode, Trap, VarThresholds,
};
use adaptive_transport::rate::{effective_send_rate, Gain, SrcConfig};
use std::time::Duration;

fn dms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn tv_fusion_a_trapezoidal_membership() {
    // GOLDEN A: loss fixture membership (ppm) → (LOW, MED, HIGH) per-mille.
    let loss = FuzzyConfig::fixture().loss;
    let cases = [
        (100_000u64, 1000u32, 0u32, 0u32),
        (250_000, 500, 500, 0),
        (400_000, 0, 1000, 0),
        (550_000, 0, 500, 500),
        (700_000, 0, 0, 1000),
    ];
    for (x, low, med, high) in cases {
        let d = loss.fuzzify(x);
        assert_eq!((d.low, d.med, d.high), (low, med, high), "loss={x}");
    }
    // Shoulder: sol-omuz (a==b) ve sağ-omuz (c==d).
    assert_eq!(trap_permille(0, 0, 0, 200_000, 300_000), 1000); // LOW x=0
    assert_eq!(
        trap_permille(u64::MAX, 500_000, 600_000, u64::MAX, u64::MAX),
        1000
    ); // HIGH plateau
}

#[test]
fn tv_fusion_b_decision_s1() {
    // GOLDEN B: weighted-average karar. loss=550k(MED500,HIGH500) bw=175k(MED500,HIGH500)
    // jitter=50k(LOW1000) qd=0 ackconf=1000(HIGH).
    let ctrl = DeterministicFuzzyController::fixture();
    let d = ctrl.decide(&FusionInputs {
        loss_ppm: 550_000,
        bandwidth_bps: 175_000,
        jitter_ppm: 50_000,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    });
    assert_eq!(d.fec_parity_ratio_permille, 650); // (500·800 + 500·500)/1000
    assert_eq!(d.arq_aggressiveness_permille, 550); // (500·600 + 500·500)/1000
    assert_eq!(d.send_rate_gain_permille, 1000); // bw not LOW → throttle yok
    assert!(!d.compress);
    assert_eq!(d.mode, Mode::Normal);
}

#[test]
fn tv_fusion_c_loss_decoupling() {
    // GOLDEN C: loss 550k→100k → FEC değişir AMA send_rate_gain SABİT (§5.4).
    let ctrl = DeterministicFuzzyController::fixture();
    let base = FusionInputs {
        loss_ppm: 550_000,
        bandwidth_bps: 175_000,
        jitter_ppm: 50_000,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    };
    let s1 = ctrl.decide(&base);
    let s1p = ctrl.decide(&FusionInputs {
        loss_ppm: 100_000,
        ..base
    });

    assert_eq!(s1.fec_parity_ratio_permille, 650); // loss FEC'i etkiler
    assert_eq!(s1p.fec_parity_ratio_permille, 0);
    // send_rate_gain loss'a kör (bw+qd aynı):
    assert_eq!(s1.send_rate_gain_permille, s1p.send_rate_gain_permille);
    assert_eq!(s1.send_rate_gain_permille, 1000);
}

#[test]
fn tv_fusion_d_emergency_trigger() {
    // GOLDEN D: loss HIGH ∧ bw LOW ∧ ackconf LOW → emergency; send_rate_gain throttled.
    let ctrl = DeterministicFuzzyController::fixture();
    let d = ctrl.decide(&FusionInputs {
        loss_ppm: 650_000,
        bandwidth_bps: 25_000,
        jitter_ppm: 50_000,
        qd_ratio_permille: 2500, // 2.5×rtprop → HIGH (oran-uzayı, D-3/1)
        ack_confidence_permille: 100,
    });
    assert_eq!(d.mode, Mode::EmergencyForwardRedundancy);
    assert_eq!(d.send_rate_gain_permille, 500); // bw LOW ∧ qd HIGH (loss-kör throttle)
    assert!(d.compress);
}

#[test]
fn tv_fusion_composition_with_src() {
    // send_rate_gain × SRC composition: min(pacing·gain·BtlBw, cwnd/srtt).
    let cfg = SrcConfig::default();
    // gain=1000 → throttle yok
    assert_eq!(
        effective_send_rate(&cfg, 62500, dms(100), dms(100), Gain::STEADY, 1000),
        62500
    );
    // gain=500 → fuzzy soft-drain
    assert_eq!(
        effective_send_rate(&cfg, 62500, dms(100), dms(100), Gain::STEADY, 500),
        31250
    );
    // gain=1000 ama srtt=500ms → cwnd/srtt HARD ceiling son söz
    assert_eq!(
        effective_send_rate(&cfg, 62500, dms(100), dms(500), Gain::STEADY, 1000),
        25000
    );
}

#[test]
fn tv_fusion_via_trait_object() {
    // Port temizliği: dyn FusionController üzerinden.
    let ctrl = DeterministicFuzzyController::fixture();
    let c: &dyn FusionController = &ctrl;
    let d = c.decide(&FusionInputs {
        loss_ppm: 550_000,
        bandwidth_bps: 175_000,
        jitter_ppm: 50_000,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    });
    assert_eq!(d.fec_parity_ratio_permille, 650);
}

// ── [TUR-4] log2_fp + covers_domain + sıfır-ağırlık fallback vektörleri ──

#[test]
fn tv_log2_fp_exact_and_overflow_edges() {
    // Uçlar: 0-guard (ilog2(0) tanımsız), e<8 EXACT bölge, 2^n tam-değerler.
    assert_eq!(log2_fp(0), 0);
    assert_eq!(log2_fp(1), 0);
    assert_eq!(log2_fp(2), 256);
    assert_eq!(log2_fp(3), 384); // e=1, frac=(3<<7)&0xFF=128
    assert_eq!(log2_fp(255), 7 * 256 + 254);
    assert_eq!(log2_fp(256), 8 * 256); // süreklilik: 2046 → 2048
                                       // Workflow taşma-vakası: naif (x−2^e)·256>>e formülü e≥56'da u64 taşırırdı.
    assert_eq!(log2_fp(1u64 << 56), 56 * 256);
    assert_eq!(log2_fp((1u64 << 57) + (1u64 << 56)), 57 * 256 + 128); // mantissa=1.5
    assert_eq!(log2_fp(u64::MAX), 63 * 256 + 255); // domain-tavanı 16383
}

#[test]
fn tv_log2_fp_monotonic_nondecreasing() {
    // e≤8 bölgesi EXHAUSTIVE (kesir exact); üstü stride-örneklemeli + taşma-bölgesi çevresi.
    let mut prev = 0;
    for x in 1u64..=65_536 {
        let v = log2_fp(x);
        assert!(v >= prev, "monotonluk kırıldı x={x}");
        prev = v;
    }
    let mut prev = 0;
    let mut x = 1u64;
    while x < u64::MAX / 3 {
        let v = log2_fp(x);
        assert!(v >= prev, "monotonluk kırıldı x={x}");
        prev = v;
        x = x.saturating_mul(3) / 2 + 1; // deterministik geometrik tarama (2^56+ dahil)
    }
}

#[test]
fn tv_covers_domain_fixture_and_strict_boundary() {
    // Fixture'ın 5 değişkeni de domain-örtülü olmalı (membership-invariant, savunma-derinliği).
    let c = FuzzyConfig::fixture();
    for (name, v) in [
        ("loss", &c.loss),
        ("bandwidth", &c.bandwidth),
        ("jitter", &c.jitter),
        ("queue_delay", &c.queue_delay),
        ("ack_confidence", &c.ack_confidence),
    ] {
        assert!(v.covers_domain(), "{name} domain-örtüsü İHLAL");
        // Örneklemeli doğrulama: hiçbir x'te üç üyelik birden 0 olamaz.
        for x in [
            0,
            1,
            100,
            250,
            500,
            1000,
            1999,
            2000,
            5000,
            100_000,
            700_000,
            u64::MAX,
        ] {
            let d = v.fuzzify(x);
            assert!(d.low > 0 || d.med > 0 || d.high > 0, "{name} x={x} örtüsüz");
        }
    }
    // KESİN-> kanıtı: dokunan-ama-örtüşmeyen (low.d == med.a) tek-nokta deliği REDdedilir
    // (trap kenarları dışlayıcı: x=500'de μL=0 ∧ μM=0).
    let touching = VarThresholds {
        low: Trap {
            a: 0,
            b: 0,
            c: 300,
            d: 500,
        },
        med: Trap {
            a: 500,
            b: 700,
            c: 800,
            d: 900,
        },
        high: Trap {
            a: 850,
            b: 950,
            c: u64::MAX,
            d: u64::MAX,
        },
    };
    assert!(!touching.covers_domain(), "≥ tek-nokta deliğini kaçırır");
    let hole = touching.fuzzify(500);
    assert_eq!((hole.low, hole.med, hole.high), (0, 0, 0), "delik gerçek");
}

#[test]
fn tv_fusion_zero_weight_fallback() {
    // Ölçülmüş dejenerasyon-sınıfı: loss.low=0 (lppm≥600k) ∧ sub-kbps (bw.med/high=0) ∧
    // jitter.low=1000 (R2'yi bw keser) → 4 kural da 0-ağırlık → ESKİ: 0/0→arq=0 (pending
    // sonsuza dek drain edilmez); YENİ: fallback fec_zero + arq_light + no_rule bayrağı.
    let ctrl = DeterministicFuzzyController::fixture();
    let d = ctrl.decide(&FusionInputs {
        loss_ppm: 700_000,
        bandwidth_bps: 500, // log2_fp(500) < log2_fp(50_000) → bw.low=1000
        jitter_ppm: 0,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    });
    assert!(d.no_rule, "fallback bayrağı");
    assert_eq!(d.fec_parity_ratio_permille, 0, "fec_zero tabanı");
    assert_eq!(
        d.arq_aggressiveness_permille, 200,
        "arq_light — arq=0 yapısal İMKÂNSIZ"
    );
    // INV-RATE-1 yolu fallback'ten BAĞIMSIZ yaşar (payda yapısal >0).
    assert_eq!(d.send_rate_gain_permille, 1000);
    // Normal bölgede bayrak KAPALI (R1 ateşliyor) — fallback çıktısı R1-çıktısıyla bayt-özdeş
    // olduğundan ayrım YALNIZ bayrakta (gözlemlenebilirlik, workflow-bulgusu).
    let n = ctrl.decide(&FusionInputs {
        loss_ppm: 0,
        bandwidth_bps: 500,
        jitter_ppm: 0,
        qd_ratio_permille: 0,
        ack_confidence_permille: 1000,
    });
    assert!(!n.no_rule);
    assert_eq!(n.arq_aggressiveness_permille, 200);
}
