//! FINAL BUILD S3-R1 — ExactDeficit regression + reverse-path integration/pairing + 0.80C source-offer
//! pacing (measured grounding). DIAGNOSTIC (result-bearing-DEĞİL; manifest-DIŞI; 360-budget-harcamaz).
//! sender.rs/loss.rs DOKUNULMAZ; run_spine değişiklikleri ADDITIVE (burst-yolu + ExactDeficit + structural-null
//! DEĞİŞMEZ; S1/S2 pinleri yeşil kalır).
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_spine.rs"]
mod fs_spine;
use adaptive_transport::fs_trace::ReverseSpec;
use fs_spine::*;
use std::collections::BTreeSet;

const LINK: u64 = 1_000_000; // bottleneck capacity C
const OWD: u64 = 20 * MS;

// ── PACING PREDECLARED TOLERANCE (§16; observed-output'tan TÜRETİLMEZ) ──────────────────────────────
// expected inter-departure = sealed(101)×8×1e9 / pacer(800000) = 1_010_000 ns (1.01 ms).
// dt = 10_000 ns; elig[g] = g×1_010_000 → hepsi dt-katı (1_010_000/10_000 = 101 tam) → offer-quantization = 0.
// serialize(101@1M) = 808_000 ns SABİT/paket → (N−1)-span'da sadeleşir. max-teorik-hata = 0 (deterministik).
// frozen-tolerance = 1×dt / span × rate ≈ 10_000/47_470_000 × 800_000 ≈ 169 bps → güvenli TOL = 500 bps.
const PACER_0P80C: u64 = 800_000;
const PACE_DT: u64 = 10_000; // 10 µs (elig-time'lar dt-katı → quantization 0)
const PACE_OWD: u64 = 100_000; // 100 µs (küçük RTT → ACK feed'den hızlı → app-limited → paced)
const PACING_TOL_BPS: u64 = 500;

fn paced_a0_cfg() -> SpineCfg {
    SpineCfg::diag(LINK, PACE_OWD, &[])
        .with_pacer(PACER_0P80C)
        .with_dt(PACE_DT)
}

// ── PIN 1: EXACT_DEFICIT_VARIANT_REGRESSION (§10) ──────────────────────────────────────────────────
// typed-impairment refactor (R0) run_spine'ı DEĞİŞTİRMEDİ → ExactDeficit davranışı AYNEN. d∈{1,2,4} = tam-d
// global InitialSource-drop (per-block-çarpım-YOK; d-app-unit/d-repair/d-retransmit DEĞİL).
#[test]
fn exact_deficit_variant_regression() {
    // d=1 (tail g47 → yalnız FEC kurtarır), d=2 (g2,5), d=4 (g2,5,10,20).
    for (drops, d) in [
        (vec![47u16], 1usize),
        (vec![2, 5], 2),
        (vec![2, 5, 10, 20], 4),
    ] {
        let r = run_spine(SpineArm::F0 { r: 4 }, &SpineCfg::diag(LINK, OWD, &drops));
        assert_eq!(r.deficit_count, d, "deficit_count == |D| = {d}");
        assert_eq!(
            r.dropped_global_ids, drops,
            "dropped-set = configured exact InitialSource-ids"
        );
        assert!(
            r.faithful,
            "realized InitialSource-drops == d = {d} (çarpım-YOK)"
        );
    }
    // ExactDeficit forward-channel HÂLÂ yalnız InitialSource-drop (redundancy/retransmit bağımsız): faithful.
    let a0 = run_spine(SpineArm::A0, &SpineCfg::diag(LINK, OWD, &[2, 5]));
    assert!(a0.faithful && a0.deficit_count == 2);
}

// ── PIN 2: GLOBAL_DEFICIT_ARM_INVARIANCE_S3 + DEFICIT_NO_MULTIPLICATION_S3 (§10) ────────────────────
#[test]
fn global_deficit_arm_invariance_and_no_multiplication_s3() {
    let c = SpineCfg::diag(LINK, OWD, &[2, 5]);
    let arms = [
        run_spine(SpineArm::A0, &c),
        run_spine(SpineArm::A1 { r: 2 }, &c),
        run_spine(SpineArm::F0 { r: 2 }, &c),
        run_spine(SpineArm::F1 { r: 2 }, &c),
        run_spine(SpineArm::F3 { r: 2 }, &c),
        run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(250 * MS)),
    ];
    let ident = arms[0].configured_schedule_hash;
    for a in &arms {
        assert_eq!(
            a.dropped_global_ids,
            vec![2, 5],
            "{}: global-deficit farklı",
            a.arm
        );
        assert_eq!(
            a.configured_schedule_hash, ident,
            "{}: configured-identity farklı",
            a.arm
        );
        assert!(
            a.faithful,
            "{}: realized-drops != d (per-block çarpım?)",
            a.arm
        );
        assert_eq!(a.deficit_count, 2);
    }
    // F4 K2/K4/K8 across: same global deficit (per-workload; per-block DEĞİL).
    for base in [30u64, 100, 250].map(|m| m * MS) {
        let f4 = run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(base));
        assert_eq!(f4.dropped_global_ids, vec![2, 5]);
        assert!(f4.faithful);
    }
}

// ── PIN 3: FORWARD_REVERSE_DOMAIN_SEPARATION (§11) ─────────────────────────────────────────────────
#[test]
fn forward_reverse_domain_separation() {
    let base = SpineCfg::diag(LINK, OWD, &[2, 5]); // reverse=Clear
    let b = run_spine(SpineArm::F0 { r: 2 }, &base);
    let fwd0 = b.forward_schedule_id;
    let rev0 = b.reverse_schedule_id;

    // (a) YALNIZ reverse değişir (Clear → Scheduled) → reverse-id değişir, forward-id AYNI.
    let rev_sched = ReverseSpec::Scheduled {
        blackout_intervals_ns: vec![(5 * MS, 10 * MS)],
        loss_permille: 0,
        seed: 7,
    };
    let only_rev = run_spine(
        SpineArm::F0 { r: 2 },
        &base.clone().with_reverse(rev_sched.clone()),
    );
    assert_eq!(
        only_rev.forward_schedule_id, fwd0,
        "reverse-değişimi forward-id'yi DEĞİŞTİRMEZ"
    );
    assert_ne!(
        only_rev.reverse_schedule_id, rev0,
        "reverse-değişimi reverse-id'yi DEĞİŞTİRİR"
    );
    assert_eq!(only_rev.reverse_spec_kind, "Scheduled");
    assert_eq!(only_rev.reverse_blackout_intervals, vec![(5 * MS, 10 * MS)]);

    // (b) YALNIZ forward değişir (drop-set) → forward-id değişir, reverse-id AYNI.
    let only_fwd = run_spine(SpineArm::F0 { r: 2 }, &SpineCfg::diag(LINK, OWD, &[3, 7]));
    assert_ne!(
        only_fwd.forward_schedule_id, fwd0,
        "forward-değişimi forward-id'yi DEĞİŞTİRİR"
    );
    assert_eq!(
        only_fwd.reverse_schedule_id, rev0,
        "forward-değişimi reverse-id'yi DEĞİŞTİRMEZ"
    );

    // configured_schedule_hash İKİSİNE de commit eder → her iki değişimde de farklı.
    assert_ne!(
        only_rev.configured_schedule_hash,
        b.configured_schedule_hash
    );
    assert_ne!(
        only_fwd.configured_schedule_hash,
        b.configured_schedule_hash
    );
}

// ── PIN 4: REVERSE_CONFIGURED_SCHEDULE_PAIRING (§12) ───────────────────────────────────────────────
// paired-arm: AYNI configured reverse-schedule → configured-identity ÖZDEŞ; realized ACK-trafiği FARKLI-olabilir.
#[test]
fn reverse_configured_schedule_pairing() {
    let rev = ReverseSpec::Scheduled {
        blackout_intervals_ns: vec![(30 * MS, 60 * MS)],
        loss_permille: 100,
        seed: 0xBEEF,
    };
    // drop [2,5] → arm'lar FARKLI recovery (A0 retransmit, F0 FEC) → farklı realized reverse/forward trafiği.
    let c = SpineCfg::diag(LINK, OWD, &[2, 5]).with_reverse(rev);
    let arms = [
        run_spine(SpineArm::A0, &c),
        run_spine(SpineArm::A1 { r: 2 }, &c),
        run_spine(SpineArm::F0 { r: 2 }, &c),
        run_spine(SpineArm::F1 { r: 2 }, &c),
        run_spine(SpineArm::F3 { r: 2 }, &c),
    ];
    let cfg_hash = arms[0].configured_schedule_hash;
    let rev_id = arms[0].reverse_schedule_id;
    for a in &arms {
        // PAIRING = configured reverse-intervention-authority ÖZDEŞ (realized-ACK-eşitliği DEĞİL).
        assert_eq!(
            a.configured_schedule_hash, cfg_hash,
            "{}: configured-schedule farklı",
            a.arm
        );
        assert_eq!(
            a.reverse_schedule_id, rev_id,
            "{}: reverse-schedule-id farklı",
            a.arm
        );
        assert_eq!(a.reverse_blackout_intervals, vec![(30 * MS, 60 * MS)]);
    }
    // ...ama arm-ÇIKTILARI (realized) FARKLI-olabilir: forward realized-trace arm'a göre değişir (S1'de kanıtlı).
    let realized: BTreeSet<u64> = arms.iter().map(|a| a.forward_trace_hash).collect();
    assert!(
        realized.len() > 1,
        "paired-configured AMA realized-forward-trace arm'a göre FARKLI"
    );
}

// ── PIN 5: SOURCE_OFFER_CONFIGURED_0P80C (§14/§15A) ────────────────────────────────────────────────
#[test]
fn source_offer_configured_0p80c() {
    let r = run_spine(SpineArm::A0, &paced_a0_cfg());
    // configured source-offer = 0.80 × bottleneck; bottleneck DÜŞÜRÜLMEDİ.
    assert_eq!(
        r.bottleneck_capacity_bps, 1_000_000,
        "bottleneck = C (düşürülmedi)"
    );
    assert_eq!(r.source_offer_pacer_bps, 800_000);
    assert_eq!(r.configured_source_offer_bps, 800_000, "configured = 0.80C");
    assert_eq!(
        r.configured_source_offer_bps,
        8 * r.bottleneck_capacity_bps / 10
    );
    // 48 × 101 × 8 / 800000 = 48.48 ms envelope.
    assert_eq!(r.source_offer_start_ns, 0);
    assert_eq!(
        r.source_offer_end_ns, 48_480_000,
        "source-offer envelope = 48.48 ms"
    );
}

// ── PIN 6: SOURCE_OFFER_SCHEDULE_EQUALITY_S3 (§16) ─────────────────────────────────────────────────
#[test]
fn source_offer_schedule_equality_s3() {
    let c = paced_a0_cfg();
    let arms = [
        run_spine(SpineArm::A0, &c),
        run_spine(SpineArm::A1 { r: 2 }, &c),
        run_spine(SpineArm::F0 { r: 2 }, &c),
        run_spine(SpineArm::F1 { r: 2 }, &c),
        run_spine(SpineArm::F3 { r: 2 }, &c),
        run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(250 * MS)),
    ];
    let sh = arms[0].source_offer_schedule_hash;
    for a in &arms {
        assert_eq!(
            a.source_offer_schedule_hash, sh,
            "{}: source-offer schedule farklı",
            a.arm
        );
        assert_eq!(
            a.configured_source_offer_bps, 800_000,
            "{}: configured-rate farklı",
            a.arm
        );
        assert_eq!(
            a.source_offer_end_ns, 48_480_000,
            "{}: envelope farklı",
            a.arm
        );
        assert_eq!(
            a.base_source_bytes,
            48 * 101,
            "{}: offered-source-bytes farklı",
            a.arm
        );
        assert_eq!(a.source_symbol_count, 48);
    }
    // F4 K2/K4/K8 de AYNI source-offer schedule (intervention source-offer'ı DEĞİŞTİRMEZ).
    for base in [30u64, 100, 250].map(|m| m * MS) {
        let f4 = run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(base));
        assert_eq!(f4.source_offer_schedule_hash, sh);
        assert_eq!(f4.source_offer_end_ns, 48_480_000);
    }
}

// ── PIN 7: SOURCE_OFFER_REALIZED_A0_NO_CONTENTION + PACING_TOLERANCE_PREDECLARED (§15B/§16) ─────────
#[test]
fn source_offer_realized_a0_no_contention() {
    // A0 · no-loss · clear-reverse · no-intervention · bottleneck C · dt=10µs → app-limited paced.
    let r = run_spine(SpineArm::A0, &paced_a0_cfg());
    assert!(
        r.completed && r.reassembly_equal,
        "A0 no-loss paced tamamlanır"
    );
    // realized source-wire ≈ configured 0.80C (predeclared TOL; observed-output'tan TÜRETİLMEDİ).
    let realized = r.realized_source_wire_bps;
    let diff = realized.abs_diff(800_000);
    assert!(
        diff <= PACING_TOL_BPS,
        "realized-source-wire {realized} bps, configured 800000 (|Δ|={diff} > TOL={PACING_TOL_BPS})"
    );
    // no-contention + dt-katı elig → deterministik: TAM 800000 beklenir (grounding-failure → DUR-B).
    assert_eq!(
        realized, 800_000,
        "no-contention deterministik realized = 0.80C (tam)"
    );
}

// ── PIN 8: INTERVENTION_CONTENTION_OBSERVATION (§15B/§23-I) ─────────────────────────────────────────
// A1 (interleaved dup) realized-source-wire A0'dan FARKLI-olabilir (bottleneck-contention) AMA configured
// source-offer schedule AYNI kalır. (Bilimsel: realized-wire ≠ configured-offer eşitliği zorlanmaz.)
#[test]
fn intervention_contention_distinct_from_configured() {
    let a0 = run_spine(SpineArm::A0, &paced_a0_cfg());
    let a1 = run_spine(SpineArm::A1 { r: 2 }, &paced_a0_cfg());
    // configured source-offer AYNI (schedule + rate + envelope).
    assert_eq!(a0.source_offer_schedule_hash, a1.source_offer_schedule_hash);
    assert_eq!(
        a0.configured_source_offer_bps,
        a1.configured_source_offer_bps
    );
    // A1 dup'ları source ile bottleneck'te yarışır → realized-source-wire A0'a EŞİT-OLMAYABİLİR.
    // (A0 = tam 800000; A1 dup-contention ile ≤ 800000. Eşitlik ZORLANMAZ — realized arm'a göre değişebilir.)
    assert!(a1.realized_source_wire_bps <= a0.realized_source_wire_bps + PACING_TOL_BPS);
    assert!(
        a1.duplicates_emitted > 0,
        "A1 dup emit etti (contention kaynağı)"
    );
}

// ── PIN 9/10: F1/F3_BULK_NULL_REGRESSION_S3 (§24) ──────────────────────────────────────────────────
// S3-R1 spine-değişiklikleri (reverse/pacing ADDITIVE) sender-wide DATA-first sıralamasını BOZMADI:
// deterministik paired binding-fixture'da F1/F3 parity-wire F0'ı GEÇMEZ. Advancement → DUR-A.
#[test]
fn f1_bulk_null_regression_s3() {
    let c = SpineCfg::diag(LINK, OWD, &[]);
    let f0 = run_spine(SpineArm::F0 { r: 2 }, &c);
    let f1 = run_spine(SpineArm::F1 { r: 2 }, &c);
    assert!(f0.t_first_parity_wire.unwrap() >= f0.t_final_source_wire_complete.unwrap());
    assert!(f1.t_first_parity_wire.unwrap() >= f1.t_final_source_wire_complete.unwrap());
    assert_eq!(
        f1.t_first_parity_wire, f0.t_first_parity_wire,
        "S3-R1 sonrası F1 parity-wire F0'ı GEÇTİ → STRUCTURAL FALSIFIED (DUR-A)"
    );
    assert_eq!(
        f1.structural_class,
        StructuralClass::BulkParityWireNullConfirmed
    );
}

#[test]
fn f3_bulk_null_regression_s3() {
    let c = SpineCfg::diag(LINK, OWD, &[]);
    let f0 = run_spine(SpineArm::F0 { r: 2 }, &c);
    let f3 = run_spine(SpineArm::F3 { r: 2 }, &c);
    assert!(f3.t_first_parity_wire.unwrap() >= f3.t_final_source_wire_complete.unwrap());
    assert_eq!(
        f3.t_first_parity_wire, f0.t_first_parity_wire,
        "S3-R1 sonrası F3 parity-wire F0'ı GEÇTİ → DUR-A"
    );
    assert_eq!(
        f3.structural_class,
        StructuralClass::BulkParityWireNullConfirmed
    );
}

// ── PIN 11: F4_PREDECISION_FIREWALL_S3 (§23) ───────────────────────────────────────────────────────
// F4 K yalnız PRE-DECISION base-RTT'den; reverse/pacing/deficit/completion K'yı DEĞİŞTİRMEZ.
#[test]
fn f4_predecision_firewall_s3() {
    let base = 250 * MS;
    let rev = ReverseSpec::Scheduled {
        blackout_intervals_ns: vec![(5 * MS, 40 * MS)],
        loss_permille: 200,
        seed: 3,
    };
    // aynı base_rtt; değişen: deficit + reverse + pacing → K SABİT kalır (post-decision-input SIZINTI-YOK).
    let k_plain = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[]).with_f4_base_rtt(base),
    )
    .selected_k;
    let k_deficit = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[2, 5]).with_f4_base_rtt(base),
    )
    .selected_k;
    let k_reverse = run_spine(
        SpineArm::F4 { r: 2 },
        &SpineCfg::diag(LINK, OWD, &[2, 5])
            .with_reverse(rev)
            .with_f4_base_rtt(base),
    )
    .selected_k;
    let k_paced = run_spine(
        SpineArm::F4 { r: 2 },
        &paced_a0_cfg().with_f4_base_rtt(base),
    )
    .selected_k;
    assert_eq!(k_plain, 8, "base 250ms → K8");
    assert_eq!(k_deficit, k_plain, "deficit K'yı DEĞİŞTİRMEZ");
    assert_eq!(
        k_reverse, k_plain,
        "realized-reverse-blackout K'yı DEĞİŞTİRMEZ"
    );
    assert_eq!(k_paced, k_plain, "pacing K'yı DEĞİŞTİRMEZ");
    // farklı base_rtt → farklı K (pre-decision path-history TEK girdi).
    assert_eq!(f4_decide_k(30 * MS).k(), 2);
    assert_eq!(f4_decide_k(100 * MS).k(), 4);
}
