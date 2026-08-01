//! FINAL BUILD S3-R2 — NaturalIid plumbing (deficit-free IID surface) + six-arm pairing + determinism +
//! role-admissibility + no-arm/no-wire-ordinal keying. DIAGNOSTIC (result-bearing-DEĞİL; manifest-DIŞI;
//! 360-budget-harcamaz). NaturalIid channel/hash DEĞİŞTİRİLMEDİ (mevcut fs_trace); yalnız spine'e plumb-edildi.
//! sender.rs/loss.rs DOKUNULMAZ. loss_permille = 200 (p=0.20 SABİT; deficit-free).
#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_spine.rs"]
mod fs_spine;
use adaptive_transport::fs_trace::{TraceChannel, TracePacketId, TraceSpec, TransmissionRole};
use fs_spine::*;

const LINK: u64 = 1_000_000;
const OWD: u64 = 20 * MS;
const IID_HORIZON: u64 = 3000 * MS; // tüm-48-InitialSource offer'ından sonra yeterli (cwnd ~birkaç-RTT)

// deterministik decide-drop sayacı (direct channel; emit_time role-key'e GİRMEZ → sadece dağılım-mekaniği).
fn count_drops(role: TransmissionRole, n: u32, permille: u32, seed: u64) -> u64 {
    let mut ch = TraceChannel::new(TraceSpec::NaturalIid {
        loss_permille: permille,
        seed,
    });
    let attempt = matches!(
        role,
        TransmissionRole::RetransmitSource | TransmissionRole::Duplicate
    );
    let mut drops = 0u64;
    for i in 0..n {
        let id = TracePacketId {
            source_id: (i, 0),
            role,
            attempt: u16::from(attempt),
        };
        if ch.decide(id, u64::from(i) * 1000) {
            drops += 1;
        }
    }
    drops
}

// ── PIN 1: IID_ROLE_ADMISSIBILITY (§19) ────────────────────────────────────────────────────────────
// InitialSource/Retransmit/Duplicate/Repair loss-eligible (garantili-teslim YOK); Control muaf (drop-YOK).
#[test]
fn iid_role_admissibility() {
    for role in [
        TransmissionRole::InitialSource,
        TransmissionRole::RetransmitSource,
        TransmissionRole::Duplicate,
        TransmissionRole::Repair,
    ] {
        let drops = count_drops(role, 1000, 200, 0x51D);
        assert!(
            drops > 0,
            "{role:?}: loss-eligible olmalı (garantili-teslim YOK)"
        );
        assert!(drops < 1000, "{role:?}: hepsi düşmez (p=0.20)");
    }
    // Control: forward-loss MUAF (reverse ACK/control AYRI kanal — decide_reverse).
    assert_eq!(
        count_drops(TransmissionRole::Control, 1000, 200, 0x51D),
        0,
        "Control forward-loss'tan muaf"
    );
    // p=1000 → repair/dup/retransmit bile hepsi düşer (garantili-teslim-imtiyazı YOK).
    for role in [
        TransmissionRole::Repair,
        TransmissionRole::Duplicate,
        TransmissionRole::RetransmitSource,
    ] {
        assert_eq!(
            count_drops(role, 200, 1000, 1),
            200,
            "{role:?}: p=1000 → hepsi düşer"
        );
    }
}

// ── PIN 2: IID_NO_WIRE_ORDINAL_KEYING (§18) ────────────────────────────────────────────────────────
// decide yalnız (role, source_id, attempt, seed)'e bağlı — emit_time / çağrı-sırası / ordinal'e DEĞİL.
#[test]
fn iid_no_wire_ordinal_keying() {
    for esi in 0..24u16 {
        let id = TracePacketId::initial_source(0, esi);
        // (a) aynı id, FARKLI emit_time → aynı karar.
        let mut c1 = TraceChannel::new(TraceSpec::NaturalIid {
            loss_permille: 200,
            seed: 42,
        });
        let mut c2 = TraceChannel::new(TraceSpec::NaturalIid {
            loss_permille: 200,
            seed: 42,
        });
        let d_early = c1.decide(id, 1);
        let d_late = c2.decide(id, 9_999_999_999);
        assert_eq!(d_early, d_late, "emit_time kararı DEĞİŞTİRMEZ (esi {esi})");
        // (b) önce başka-id'ler karar-alır (log/ordinal ilerler) → aynı-id kararı DEĞİŞMEZ.
        let mut c3 = TraceChannel::new(TraceSpec::NaturalIid {
            loss_permille: 200,
            seed: 42,
        });
        for j in 100..130u16 {
            let _ = c3.decide(TracePacketId::initial_source(0, j), u64::from(j));
        }
        let d_after = c3.decide(id, 5);
        assert_eq!(
            d_early, d_after,
            "önceki-kararlar/ordinal aynı-id kararını DEĞİŞTİRMEZ (esi {esi})"
        );
    }
}

// ── PIN 3: IID_NO_ARM_NAME_KEYING (§17) ────────────────────────────────────────────────────────────
// decide fonksiyonunda arm-girdisi YOK: aynı IID-cfg altında realized InitialSource-drop-set arm-BAĞIMSIZ.
#[test]
fn iid_no_arm_name_keying() {
    let c = SpineCfg::iid(LINK, OWD, 200, 0x5EED).with_horizon(IID_HORIZON);
    let a0 = run_spine(SpineArm::A0, &c);
    let f0 = run_spine(SpineArm::F0 { r: 2 }, &c);
    let a1 = run_spine(SpineArm::A1 { r: 2 }, &c);
    // farklı-arm, farklı-recovery, farklı-ACK-trafiği → AMA InitialSource-drop-set ÖZDEŞ (arm key'e girmez).
    assert_eq!(
        a0.realized_initial_source_drop_ids,
        f0.realized_initial_source_drop_ids
    );
    assert_eq!(
        a0.realized_initial_source_drop_ids,
        a1.realized_initial_source_drop_ids
    );
    assert!(
        !a0.realized_initial_source_drop_ids.is_empty(),
        "IID bazı InitialSource'ları düşürdü"
    );
}

// ── PIN 4: IID_PROBABILITY_DISTRIBUTION_SANITY (§22; MEKANİZMA-diagnostiği, FS-1-KANIT-DEĞİL) ─────────
// önceden-ilan-edilen sanity-bandı (observed'tan TÜRETİLMEZ; hash/threshold TUNE-EDİLMEZ):
//   p=200 → 10000-id drop-oranı ∈ [0.17, 0.23] (≈0.20 mekanizma-sanity, precision-iddia DEĞİL)
//   p=0 → 0 · p=1000 → hepsi.
#[test]
fn iid_probability_distribution_sanity() {
    let n = 10_000u32;
    let drops = count_drops(TransmissionRole::InitialSource, n, 200, 7);
    let frac = drops as f64 / f64::from(n);
    assert!(
        (0.17..=0.23).contains(&frac),
        "IID ≈20% sanity: frac={frac} (önceden-ilan-band [0.17,0.23]; tune-YOK)"
    );
    assert_eq!(
        count_drops(TransmissionRole::InitialSource, n, 0, 7),
        0,
        "p=0 → drop-YOK"
    );
    assert_eq!(
        count_drops(TransmissionRole::InitialSource, n, 1000, 7),
        u64::from(n),
        "p=1000 → hepsi düşer"
    );
}

// ── PIN 5: IID_INITIAL_SOURCE_ARM_INVARIANCE (§20) ─────────────────────────────────────────────────
// paired 6-arm (+ F4-K2/4/8) aynı IID-cfg → her global InitialSource 0..47 AYNI NaturalIid-kararını alır.
#[test]
fn iid_initial_source_arm_invariance() {
    let c = SpineCfg::iid(LINK, OWD, 200, 0xABCD).with_horizon(IID_HORIZON);
    let arms = [
        run_spine(SpineArm::A0, &c),
        run_spine(SpineArm::A1 { r: 2 }, &c),
        run_spine(SpineArm::F0 { r: 2 }, &c),
        run_spine(SpineArm::F1 { r: 2 }, &c),
        run_spine(SpineArm::F3 { r: 2 }, &c),
        run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(250 * MS)),
    ];
    let drop0 = &arms[0].realized_initial_source_drop_ids;
    assert!(!drop0.is_empty(), "IID InitialSource-drop-set boş-değil");
    for a in &arms {
        assert_eq!(
            &a.realized_initial_source_drop_ids, drop0,
            "{}: InitialSource-drop-set arm'a göre FARKLI (arm-invariance İHLAL)",
            a.arm
        );
    }
    // F4 K2/K4/K8 across: aynı InitialSource-drop-set (K-partition drop-kararını DEĞİŞTİRMEZ).
    for base in [30u64, 100, 250].map(|m| m * MS) {
        let f4 = run_spine(SpineArm::F4 { r: 2 }, &c.clone().with_f4_base_rtt(base));
        assert_eq!(&f4.realized_initial_source_drop_ids, drop0);
    }
}

// ── PIN 6: IID_REPLAY_X3 (§21) ─────────────────────────────────────────────────────────────────────
// aynı typed-cfg + arm ×3 → özdeş event-hash'ler + realized-drop-set + retransmit + completion + timing.
#[test]
fn iid_replay_x3() {
    let c = SpineCfg::iid(LINK, OWD, 200, 0x1234).with_horizon(IID_HORIZON);
    // tuple = (forward_trace, reverse_trace, configured_schedule, realized-drops, retransmits,
    //          fec_decoded, completed, completion_ns, base_source_bytes, intervention_bytes).
    let snap = |arm: SpineArm| {
        let r = run_spine(arm, &c);
        (
            r.forward_trace_hash,
            r.reverse_trace_hash,
            r.configured_schedule_hash,
            r.realized_initial_source_drop_ids.clone(),
            r.retransmits,
            r.fec_decoded_sources,
            r.completed,
            r.completion_ns,
            r.base_source_bytes,
            r.intervention_bytes,
        )
    };
    for arm in [SpineArm::A0, SpineArm::F0 { r: 2 }, SpineArm::A1 { r: 2 }] {
        let s1 = snap(arm);
        let s2 = snap(arm);
        let s3 = snap(arm);
        assert_eq!(s1, s2, "replay-2 farklı (deterministik-DEĞİL)");
        assert_eq!(s2, s3, "replay-3 farklı (deterministik-DEĞİL)");
    }
}
