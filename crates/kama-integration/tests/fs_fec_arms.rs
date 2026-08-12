//! DR-FEC-FS-1 FS-HARNESS-B — F1/F3 FEC-family sanity pins (GERÇEK RsSimdCodec + PIN-C sınıflandırma).
//! F4 mechanism pini adaptive-transport/fs_harness'ta (path_timing consumer; codec gerektirmez).
//! Harness-construction slice — result-bearing DEĞİL (FS-1 v1.2-freeze SONRASI).

#![cfg(feature = "experimental-fs-harness")]

#[path = "common/fs_f0.rs"]
mod fs_f0;

use adaptive_transport::fs_harness::{fs_sealed_len, pin_c_classify, ParityWireModel, PinCResult};
use fs_f0::{run_f0, run_f0_paced, MS};
use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, SourceSymbol};

/// PIN-C wire-model (harness-ortak sealed-boyut; F0 = 8-DATA-önce parity).
fn wire_model() -> ParityWireModel {
    ParityWireModel {
        link_bps: 1_000_000,
        sealed_len: fs_sealed_len(64), // 101B grounded
    }
}

/// **F1 — EARLY LOGICAL-CLOSE FEC.** Real repair decode = F0 (aynı K=8 tam-blok; gerçek RsSimdCodec).
/// PIN-C: early-close eligibility'yi ilerletir AMA DATA-first parity'yi block-DATA ardında tutar →
/// t_parity_wire ilerlemez → **STRUCTURALLY_NON_EFFECTIVE UNDER CURRENT ORDERING** (sahte-başarı YOK;
/// ordering-mührü gevşetilmez; sıradan-fail olarak gizlenmez).
#[test]
fn f1_real_decode_is_f0_and_pin_c_non_effective() {
    // (a) Real repair decode (F1 ≡ F0 full-block): D1/R1 gerçek reconstruct byte-equal.
    let r = run_f0(1, &[3], 1_000_000, 20 * MS);
    assert_eq!(r.delivered, 8);
    assert_eq!(r.fec_decoded_sources, 1);
    assert!(
        r.reconstruct_byte_equal,
        "F1 full-block real-decode byte-equal DEĞİL"
    );
    assert!(
        r.estimator_pure,
        "F1 RECOVERED estimator-purity (fake-ACK-YOK)"
    );
    // (b) t_repair_usable fixture: reconstruct-complete parity-arrival'dan SONRA (codec-latency causal).
    let (usable, parr) = (r.timing.t_repair_usable, r.timing.t_parity_arrival);
    assert!(usable.is_some() && parr.is_some());
    assert!(
        usable.unwrap() >= parr.unwrap(),
        "t_repair_usable < parity-arrival (imkânsız)"
    );
    // (c) PIN-C: F1 wire-time F0'ı GEÇEMEZ (early-close eligibility ilerletir, DATA-first wire'ı tutar).
    let m = wire_model();
    let (wire_f0, wire_f1) = (m.t_parity_wire(8), m.t_parity_wire(8)); // her-ikisi K=8 DATA-önce
    assert_eq!(
        pin_c_classify(wire_f0, wire_f1),
        PinCResult::StructurallyNonEffectiveUnderCurrentOrdering,
        "F1 EFEKTİF raporlandı (ordering-mührü altında olmamalı)"
    );
}

/// **F3 — DEADLINE PARTIAL-BLOCK FEC.** GERÇEK RsSimdCodec (profile 0x02, K=4) partial-block
/// encode→erasure→reconstruct byte-equal (real repair decode). Deterministik-trigger (sabit-deadline;
/// K'=4 sources'ta close). PIN-C: partial-parity (4 DATA önce) < F0 (8 DATA önce) → **WireTimeAdvanced**
/// (ordering-mührü korunur; interleaved-YOK; sender-değişikliği-YOK — close API K-agnostik).
#[test]
fn f3_partial_block_real_decode_and_pin_c_advances() {
    let p = by_id(0x02).unwrap(); // K=4, R=4, E=64 (partial-block profili)
    let e = 64usize;
    let k_partial = 4usize;
    // Gerçek partial-block source (K'=4 symbol).
    let src: Vec<Vec<u8>> = (0..k_partial)
        .map(|i| (0..e).map(|j| (i * 7 + j) as u8).collect())
        .collect();
    let syms: Vec<SourceSymbol> = src
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    // GERÇEK ENCODE → repair (synthetic-DEĞİL).
    let repairs = RsSimdCodec.encode(&p, &syms).unwrap();
    assert!(!repairs.is_empty(), "partial-block real repair üretilmeli");
    // Deterministik erasure: source esi=1 düştü → 3 source + 1 repair ile reconstruct.
    let mut received: Vec<ReceivedSymbol> = Vec::new();
    for (i, b) in src.iter().enumerate() {
        if i != 1 {
            received.push(ReceivedSymbol {
                esi: i as u16,
                is_repair: false,
                bytes: b,
            });
        }
    }
    received.push(ReceivedSymbol {
        esi: repairs[0].esi,
        is_repair: true,
        bytes: &repairs[0].bytes,
    });
    // GERÇEK RECONSTRUCT → byte-equality.
    let rec = RsSimdCodec.reconstruct(&p, &received, 0).unwrap();
    let recovered = rec
        .iter()
        .find(|s| s.esi == 1)
        .expect("esi=1 reconstruct edilmeli");
    assert_eq!(
        recovered.bytes, src[1],
        "partial-block real reconstruct byte-equal DEĞİL"
    );
    // PIN-C: partial parity (K'=4 DATA önce) F0 tam-blok (8 DATA önce)'dan ERKEN wire'a çıkar.
    let m = wire_model();
    assert!(
        m.t_parity_wire(k_partial as u64) < m.t_parity_wire(8),
        "partial-block parity daha-erken wire'a çıkmalı"
    );
    assert_eq!(
        pin_c_classify(m.t_parity_wire(8), m.t_parity_wire(k_partial as u64)),
        PinCResult::WireTimeAdvanced,
        "F3 partial-block PIN-C WireTimeAdvanced değil"
    );
}

/// **§8 COST-SPLIT proof:** F0 D2/R2 → FEC blok'u ERKEN tamamlar AMA post-DATA-parity geç-varınca ARQ
/// SPURIOUS retransmit sızdırır. İKİ maliyet AYRI: INTERVENTION-BUDGET (C-P4 = proaktif parity sealed-wire)
/// vs TOTAL-OUTCOME-COST (§8 = arq_retransmit_bytes + spurious_retransmits). Spurious ≠ BUDGET_MISMATCH.
#[test]
fn f0_cost_split_intervention_vs_total_outcome() {
    // DR-RATE-RAMP-2B SUPERSESSION (md `7718b94f…`): these constants were measured on the
    // startup-pacing-INACTIVE path and are PRESERVED VERBATIM here, re-scoped to it. The floor
    // removes the late-parity race that produced the spurious retransmit; the product-default
    // behaviour is sealed by `f0_cost_split_under_product_default` below. Nothing weakened.
    let sealed = fs_sealed_len(64); // 101B grounded
    let r = run_f0_paced(2, &[2, 5], 1_000_000, 20 * MS, Some(false));
    assert_eq!(r.fec_decoded_sources, 2, "FEC tamamlamalı");
    // INTERVENTION-BUDGET (C-P4): YALNIZ proaktif parity sealed-wire (2 × 101).
    assert_eq!(r.realized_sealed_wire_bytes, 2 * sealed);
    // TOTAL-OUTCOME-COST (§8 AYRI): arq_retransmit_bytes accounting-identity + spurious leak gözlendi.
    assert_eq!(
        r.arq_retransmit_bytes,
        r.retransmits * sealed,
        "arq_retransmit_bytes = retransmits × sealed (accounting-identity)"
    );
    assert!(
        r.spurious_retransmits > 0,
        "D2/R2'de FEC-erken-tamamlar + post-DATA-parity-geç → spurious-ARQ beklenir"
    );
    assert!(
        r.spurious_retransmits <= r.retransmits,
        "spurious ≤ toplam-retransmit (mantık)"
    );
}

/// **§8 COST-SPLIT under the PRODUCT DEFAULT** (startup pacing ON). The split itself is unchanged —
/// INTERVENTION-BUDGET is still exactly the proactive parity sealed wire and the accounting identity
/// still holds — but the late-parity race no longer leaks a spurious ARQ retransmit, so the
/// TOTAL-OUTCOME-COST is strictly lower. See SPURIOUS-RETRANSMIT-SUPERSESSION.
#[test]
fn f0_cost_split_under_product_default() {
    let sealed = fs_sealed_len(64);
    let r = run_f0_paced(2, &[2, 5], 1_000_000, 20 * MS, None); // product default
    assert_eq!(r.fec_decoded_sources, 2, "FEC must still complete the block");
    assert_eq!(
        r.realized_sealed_wire_bytes,
        2 * sealed,
        "INTERVENTION-BUDGET unchanged: only proactive parity sealed wire"
    );
    assert_eq!(
        r.arq_retransmit_bytes,
        r.retransmits * sealed,
        "accounting identity must hold on the default path too"
    );
    assert_eq!(
        r.spurious_retransmits, 0,
        "startup pacing removes the D2/R2 late-parity spurious leak"
    );
}
