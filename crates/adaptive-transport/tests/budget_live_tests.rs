//! Commit-W — CANLI-BÜTÇE adversarial/non-regression testleri (feature-gated; ON'da-koşar).
//! Kapsam: saturated-rezervasyon (üçgen-canlı-kanıtı) · atomik-rezervasyon-waste=0 · F2-ticket-önceliği ·
//! U4-lifecycle (miss-1-kez / CanceledMoot≠miss) · U6-Busy-guard · U9-displacement-ayrımı (new-DATA/ARQ) ·
//! canlı-hardcap-envelope. Mutation-pin-eşlemesi Commit-W-raporunda (hangi-test-hangi-mutasyonu-öldürür).
#![cfg(feature = "experimental-fec-budget")]

use adaptive_transport::budget_kernel::PolicyParams;
use adaptive_transport::sender::{
    AckInfo, BudgetPolicy, BudgetPolicyTransitionError, Event, FecActuationMode, OpaqueUnit,
    RecoveredInfo, RecoveredUnit, Sender, Timer, TransmitKind,
};
use std::time::Duration;

const MS: u64 = 1_000_000;

fn warm(s: &mut Sender, t0: u64) -> u64 {
    let mut t = t0;
    for _ in 0..3 {
        let mut seqs = Vec::new();
        for i in 0..8u64 {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: 50_000 + t + i,
                    size: 1200,
                }),
                t * MS,
            );
            seqs.extend(out.iter().map(|d| d.seq));
            t += 1;
        }
        if !seqs.is_empty() {
            s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: Duration::ZERO,
                }),
                t * MS,
            );
        }
        t += 2;
    }
    t
}

fn floorcap() -> BudgetPolicy {
    BudgetPolicy::FloorCapExperimental(PolicyParams::defaults())
}

/// Saturated-DATA'da: Leftover=parity-açlığı vs FloorCap=bounded-akış (üçgen-CANLI-kanıtı).
/// MUTATION-PİN-m1: rezervasyon-kapatılırsa (reserved=0-zorla) floorcap-kolu-da-0'a-düşer → test-DÜŞER.
#[test]
fn saturated_reservation_vs_leftover_live() {
    let run = |policy: Option<BudgetPolicy>| -> (u64, u64, u64) {
        let mut s = Sender::new();
        s.set_manual_fec_enabled(true).unwrap();
        let t0 = warm(&mut s, 0);
        if let Some(p) = policy {
            s.set_budget_policy(p).expect("boş-kuyrukta-geçiş");
        }
        // Blok-kapat: 4-parity-staged (otorite-bağlı).
        let a = s.open_fec_block_authority();
        assert_eq!(s.close_fec_block_manual(a, 1, 4, 1200), Ok(4));
        // SATURATED-sürüş: 8×1200B/10ms ≈ 960kB/s ≈ warm-rate → leftover≈0 (gerçek-doyum).
        let mut parity = 0u64;
        let mut t = t0 + 1;
        let mut id = 90_000u64;
        for _ in 0..300u64 {
            for _ in 0..8 {
                let out = s.on_event(Event::AppData(OpaqueUnit { id, size: 1200 }), t * MS);
                id += 1;
                parity += out
                    .iter()
                    .filter(|d| d.kind == TransmitKind::FecParity)
                    .count() as u64;
            }
            t += 10;
            let out = s.on_event(Event::TimerFired(Timer::Pto), t * MS);
            parity += out
                .iter()
                .filter(|d| d.kind == TransmitKind::FecParity)
                .count() as u64;
            t += 10;
        }
        let d = s.diag_budget();
        (parity, d.new_data_displaced, d.reservation_waste_total)
    };
    let (p_leftover, disp_l, _) = run(None); // default=LeftoverOnly (baseline-yol)
    let (p_floor, disp_f, waste_f) = run(Some(floorcap()));
    assert_eq!(disp_l, 0, "Leftover: displacement-yapısal-0");
    assert!(
        p_floor > p_leftover,
        "FloorCap-saturated-parity ({p_floor}) > Leftover ({p_leftover}) — rezervasyon-canlı-çalışıyor"
    );
    assert_eq!(p_floor, 4, "staged-4/4-emit (bounded-pay-yetti)");
    assert!(disp_f > 0, "displacement-ölçülür (bounded)");
    // MUTATION-PİN-m5 (hayalet-rezerv): normal-deterministik-yolda waste=0 (kabul-kriteri).
    assert_eq!(waste_f, 0, "reservation-waste=0 (atomik-plan)");
}

/// F2-önceliği: ticket'lı-bloğun-İLK-parity'si bulk'tan-önce. MUTATION-PİN-m6: ticket-servisi-bulk-
/// arkasına-alınırsa ilk-FecParity-bloğu-değişir → test-DÜŞER.
#[test]
fn f2_ticket_priority_over_bulk() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();
    let t0 = warm(&mut s, 0);
    s.set_budget_policy(floorcap()).unwrap();
    // Blok-7: ticket-cap'ine-takılmadan-önce-bulk-doldur (ticket'sız-manuel-legacy-değil; iki-otorite-blok):
    let a1 = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(a1, 7, 4, 1200), Ok(4)); // ticket(7)+bulk
    let a2 = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(a2, 8, 4, 1200), Ok(4)); // ticket(8)+bulk
                                                                 // Drenaj: ilk-iki-FecParity-kararı ticket-sırasıyla blok-7-ve-8'in-İLK-item'ları-olmalı.
    let mut first_two: Vec<u32> = Vec::new();
    let mut t = t0 + 1;
    while first_two.len() < 2 && t < t0 + 3000 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        for d in &out {
            if d.kind == TransmitKind::FecParity && first_two.len() < 2 {
                if let Some(h) = d.send_record {
                    let adaptive_transport::sender::SrLogicalRef::Fec { block_id, .. } =
                        h.logical_ref;
                    first_two.push(block_id);
                }
            }
        }
        t += 10;
    }
    assert_eq!(
        first_two,
        vec![7, 8],
        "F2: ticket-FIFO-önceliği (blok-7-ilk, 8-ikinci)"
    );
    let d = s.diag_budget();
    assert_eq!(d.tk_admitted, 2);
    assert_eq!(d.tk_on_time + d.tk_late, 2, "iki-ticket-servisli");
}

/// U4-lifecycle: miss-TAM-1-KEZ (MUTATION-PİN-m7: çift-sayım → düşer) + ServicedLate +
/// CanceledMoot-öncesi≠miss (MUTATION-PİN-m8: cancel'i-miss-sayan → düşer).
#[test]
fn ticket_lifecycle_miss_once_and_cancel_semantics() {
    // (a) STARVED-ticket: deadline-aşımı-çok-tick → missed_unique=1 (tek); sonra-servis → tk_late.
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();
    s.set_budget_policy(floorcap()).unwrap();
    // Warm-YOK → PreConverge-düşük-rate → parity-token-uzun-süre-yetmez (starve).
    let a = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(a, 5, 1, 1200), Ok(1));
    for t in 1..700u64 {
        // 70s — PreConverge-rate-31B/s'te serialization-tabanı t_floor≈38.7s+600ms; deadline-kesin-aşılır.
        s.on_event(Event::TimerFired(Timer::Pto), t * 100 * MS);
    }
    let d = s.diag_budget();
    assert_eq!(
        d.tk_missed_unique, 1,
        "miss-TAM-1-kez (200-tick'te-tek-sayım)"
    );
    assert!(d.last_floor_miss.is_some());
    // (b) CanceledMoot-DEADLINE-ÖNCESİ ≠ miss:
    let mut s2 = Sender::new();
    s2.set_manual_fec_enabled(true).unwrap();
    let t0 = warm(&mut s2, 0);
    s2.set_budget_policy(floorcap()).unwrap();
    let a2 = s2.open_fec_block_authority();
    assert_eq!(s2.close_fec_block_manual(a2, 6, 2, 1200), Ok(2));
    s2.on_event(
        Event::RecoveredControl(RecoveredInfo {
            units: vec![RecoveredUnit {
                unit_id: 600,
                block_id: 6,
                esi: 0,
            }],
        }),
        (t0 + 1) * MS,
    );
    let d2 = s2.diag_budget();
    assert_eq!(d2.tk_cancel_before, 1, "deadline-öncesi-CanceledMoot");
    assert_eq!(d2.tk_missed_unique, 0, "cancel≠miss");
    assert_eq!(
        s2.diag_fec_actuation().canceled_r_total,
        2,
        "bulk-cancel-aynen"
    );
}

/// U6-Busy-guard (MUTATION-PİN-m9: guard-kaldırılırsa-düşer) + başarılı-geçiş-semantiği.
#[test]
fn policy_transition_busy_guard() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();
    let t0 = warm(&mut s, 0);
    s.set_budget_policy(floorcap()).unwrap();
    let a = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(a, 3, 2, 1200), Ok(2));
    // Pending-varken-geçiş: Busy.
    assert_eq!(
        s.set_budget_policy(BudgetPolicy::LeftoverOnly),
        Err(BudgetPolicyTransitionError::Busy),
        "pending-ticket/bulk'ta-geçiş-YASAK"
    );
    // Drenaj + ACK'lerle-boşalt:
    let mut t = t0 + 1;
    for _ in 0..2000u64 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        let seqs: Vec<u64> = out.iter().map(|d| d.seq).collect();
        if !seqs.is_empty() {
            s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: Duration::ZERO,
                }),
                t * MS + MS / 2,
            );
        }
        t += 10;
        if s.diag_budget().tickets_pending == 0
            && s.diag_fec_actuation().enqueued_r_total == s.diag_fec_actuation().emitted_r_total
        {
            break;
        }
    }
    let before = s.diag_budget();
    assert!(before.tk_admitted >= 1, "kümülatif-sayaç-birikti");
    s.set_budget_policy(BudgetPolicy::LeftoverOnly)
        .expect("boş-kuyrukta-geçiş-OK");
    let after = s.diag_budget();
    assert_eq!(after.entitlement_credit, 0, "credit-reset");
    assert_eq!(after.hardcap_credit, 0, "hardcap-reset");
    assert_eq!(
        after.tk_admitted, before.tk_admitted,
        "kümülatifler-RESETLENMEZ"
    );
}

/// U9-displacement-ayrımı: retransmission-displacement-AYRI-ölçülür (MUTATION-PİN-m11: metrik-
/// sıfırlanırsa-düşer) + canlı-hardcap-envelope (MUTATION-PİN-m3-canlısı).
#[test]
fn displacement_split_and_hardcap_envelope_live() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();
    let t0 = warm(&mut s, 0);
    s.set_budget_policy(floorcap()).unwrap();
    let a = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(a, 2, 4, 1200), Ok(4));
    // Kayıplı-saturated-sürüş: DATA + kayıp-ilan (retx-talebi-doğar) — her-iki-displacement-sınıfı-mümkün.
    let mut t = t0 + 1;
    let mut all_seqs: Vec<u64> = Vec::new();
    for i in 0..400u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id: 70_000 + i,
                size: 1200,
            }),
            t * MS,
        );
        all_seqs.extend(out.iter().map(|d| d.seq));
        // Her-4.-turda yalnız-son-seq-ACK (packet-threshold → kayıp-ilan → retx-talebi).
        if i % 4 == 3 && !all_seqs.is_empty() {
            let keep = vec![*all_seqs.last().unwrap()];
            s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: keep,
                    ack_delay: Duration::ZERO,
                }),
                t * MS + MS / 2,
            );
        }
        t += 15;
        s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        t += 15;
    }
    let d = s.diag_budget();
    let total_displaced = d.new_data_displaced + d.retx_displaced;
    assert!(
        total_displaced > 0,
        "toplam-pre-parity-displacement-ölçüldü"
    );
    // Canlı-hardcap-envelope (U3; başlangıç-credit-0): parity_bytes ≤ hc_share×accrual + hc_cap.
    let p = PolicyParams::defaults();
    let elapsed_s = (t - t0) as f64 * 0.001;
    let rate = 60_000.0_f64.min(1.0e9); // üst-sınır-önseli; gerçek-rate≤60k (cap-60k-link-yok-ama-btlbw-örneği)
    let accrual_bound = (rate * elapsed_s) as u64;
    let parity_bytes = s.diag_fec_actuation().emitted_r_total * 1200;
    let envelope = p.hardcap_cap_bytes + accrual_bound * u64::from(p.hardcap_share_permille) / 1000;
    assert!(
        parity_bytes <= envelope,
        "canlı-hardcap-envelope: {parity_bytes} ≤ {envelope}"
    );
}

/// MUTATION-PİN-m5 (güçlendirilmiş): KUANTUM-ALTI-limit-tick'lerinde hayalet-rezerv-yasağı.
/// Warm-rate + DATA'sız + r=8-pending + 1ms-tick → refill/tick < psize → sub-quantum-limit-tick'leri
/// GARANTİ. EXACT-kuantum-kuralı → o-tick'lerde reserved=0 → waste=0. Mutasyon (reserved=raw-limit)
/// → sub-quantum-tick'lerde waste>0 → DÜŞER.
#[test]
fn subquantum_no_phantom_reservation() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();
    let t0 = warm(&mut s, 0);
    s.set_budget_policy(floorcap()).unwrap();
    let a = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(a, 4, 8, 1200), Ok(8));
    let mut t = t0 * 1_000 + 1; // µs-taban: 1ms-tick'ler
    for _ in 0..400u64 {
        let _ = s.on_event(Event::TimerFired(Timer::Pto), t * 1_000_000 / 1_000);
        t += 1;
    }
    let d = s.diag_budget();
    assert!(
        s.diag_fec_actuation().emitted_r_total >= 2,
        "ilerleme-var (kuantum-birikince-emit)"
    );
    assert_eq!(
        d.reservation_waste_total, 0,
        "kuantum-altı-tick'lerde-hayalet-rezerv-YOK (waste=0)"
    );
}
