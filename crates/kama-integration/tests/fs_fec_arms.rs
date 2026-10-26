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
/// SPURIOUS retransmit sızdırır [K4G-05b öncesi sıralamada — DUR-44]. İKİ maliyet AYRI: INTERVENTION-BUDGET
/// (C-P4 = proaktif parity sealed-wire) vs TOTAL-OUTCOME-COST (§8 = arq_retransmit_bytes +
/// spurious_retransmits). Spurious ≠ BUDGET_MISMATCH.
///
/// [K4G-05b DUR-44 (a) · DUR-8 biçimi · sahip kararı bekliyor] NEDEN DEĞİŞTİ (teşhis; ölçüm
/// `~/kama-k4g-work/k4g05b/kod-run/r10`; bisect: 35a02fcf ve 9e0ea164 YEŞİL, bc913d49'dan 20a92477'ye
/// KIRMIZI). K4G-05b rev6'nın K3 ARQ payı rezervi (CEVAP PAKETİ 15 v3 §2.3) bu fikstürde ARQ yeniden
/// gönderimini son orijinal DATA'nın token dilimine alır: t_arq_wire 124 → 115 ms, t_arq_arrival
/// 144.808 → 135.808 ms < t_repair_usable 136.0 ms. Alıcı 136 ms'de 5 orijinal + yeniden gönderim + 2
/// parite ile çözer; yeniden gönderim çözülen K sembolünden biridir, israf değildir → fikstürün
/// sınıflaması (`t_repair_usable ≤ t_arq_arrival`) doğru olarak 0 verir. Sayı, bayt ve tamamlanma aynı
/// (retx 1, parite 2 × 101 B, ARQ 101 B, 136 ms). `spurious_retransmits > 0` satırı bir maliyet kanalının
/// VARLIĞINI belgeliyordu, güvenlik özelliğini değil (SPURIOUS-RETRANSMIT-SUPERSESSION md `7718b94f…`);
/// kanal DATA'nın dilimi kazandığı sıralamada hâlâ vardır, bu fikstür onu artık kurmaz. YENİ: spurious
/// İDDİA EDİLMEZ, basılır — t_arq_arrival − t_repair_usable işaretiyle (eski sıralama +8.808 ms /
/// spurious 1; K3 −0.192 ms / spurious 0). Maliyet ayrımı iddiaları aynen; eklenen: retransmits ≥ 1 (ARQ
/// kolu çalıştı), delivered == 8, tamamlanma var. Eser: SPURIOUS-RETRANSMIT-SUPERSESSION-K4G05B-DUR44
/// (md `3faadfce…`, bu düzenlemeden ÖNCE commit'lendi); tasarım notu Ek B.1d DUR-44 ve B.2.8 #10.
/// 2×2: eski kod + eski test YEŞİL (ölçüldü, r10: 9e0ea164) · yeni kod + eski test KIRMIZI (ölçüldü, r10)
/// · yeni kod + yeni test YEŞİL, işaret − [çıkarım] · eski kod + yeni test YEŞİL, işaret +, spurious 1
/// basılır [çıkarım]. K3 kapalı mutasyonla (DUR-44): YEŞİL, t_arq_wire 124 ms, işaret +, spurious 1
/// basılır [çıkarım].
#[test]
fn f0_cost_split_intervention_vs_total_outcome() {
    // DR-RATE-RAMP-2B SUPERSESSION (md `7718b94f…`): these constants were measured on the
    // startup-pacing-INACTIVE path and are PRESERVED VERBATIM here, re-scoped to it. The floor
    // removes the late-parity race that produced the spurious retransmit; the product-default
    // behaviour is sealed by `f0_cost_split_under_product_default` below. ~~Nothing weakened.~~
    // [K4G-05b DUR-44 (a)] The `spurious_retransmits > 0` assertion was removed (printed instead): see the
    // doc comment above and SPURIOUS-RETRANSMIT-SUPERSESSION-K4G05B-DUR44 (md `3faadfce…`).
    let sealed = fs_sealed_len(64); // 101B grounded
    let r = run_f0_paced(2, &[2, 5], 1_000_000, 20 * MS, Some(false));
    assert_eq!(r.delivered, 8, "blok teslim edilmeli");
    assert!(r.completion_ns.is_some(), "tamamlanma anı olmalı");
    assert_eq!(r.fec_decoded_sources, 2, "FEC tamamlamalı");
    // INTERVENTION-BUDGET (C-P4): YALNIZ proaktif parity sealed-wire (2 × 101).
    assert_eq!(r.realized_sealed_wire_bytes, 2 * sealed);
    // TOTAL-OUTCOME-COST (§8 AYRI): arq_retransmit_bytes accounting-identity.
    assert_eq!(
        r.arq_retransmit_bytes,
        r.retransmits * sealed,
        "arq_retransmit_bytes = retransmits × sealed (accounting-identity)"
    );
    assert!(
        r.retransmits >= 1,
        "D2/R2'de ARQ kolu çalışmalı (en az bir yeniden gönderim)"
    );
    // [K4G-05b DUR-44 (a)] Spurious sınıflaması basılır, iddia edilmez: işaret + → yeniden gönderim FEC
    // çözümünden sonra vardı (israf, spurious); − → çözümden önce vardı (çözüme girdi).
    let arrival = r.timing.t_arq_arrival;
    let usable = r.timing.t_repair_usable;
    let lead_ns = match (arrival, usable) {
        (Some(a), Some(u)) => Some(i128::from(a) - i128::from(u)),
        _ => None,
    };
    let verdict = match lead_ns {
        Some(d) if d >= 0 => "ARQ FEC çözümünden sonra: yeniden gönderim israf (spurious)",
        Some(_) => "ARQ FEC çözümünden önce: yeniden gönderim çözüme girdi",
        None => "ölçülmedi",
    };
    eprintln!(
        "[f0 cost-split] retx={} spurious={} t_arq_wire={:?} t_arq_arrival={arrival:?} \
         t_repair_usable={usable:?} t_arq_arrival−t_repair_usable={lead_ns:?} ns ({verdict}); completion={:?}",
        r.retransmits, r.spurious_retransmits, r.timing.t_arq_wire, r.completion_ns
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
    assert_eq!(
        r.fec_decoded_sources, 2,
        "FEC must still complete the block"
    );
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
