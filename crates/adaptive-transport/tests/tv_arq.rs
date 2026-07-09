//! TV-ARQ golden vektörleri — §9 selective retransmit. Değerler PR açıklamasında adım
//! adım türetildi. [CAL] fixture (netem'e).

use adaptive_transport::arq::{
    ArqScheduler, CapExceedPolicy, DeadlineArqScheduler, LostSeq, ReorderTracker, RetxConfig,
    RetxDecision,
};
use adaptive_transport::pacer::TokenBucket;
use std::time::Duration;

fn ms(n: u64) -> u64 {
    n * 1_000_000
}
fn rt(seq: u64) -> RetxDecision {
    RetxDecision::Retransmit { seq }
}

#[test]
fn tv_arq_g1_deadline_priority() {
    // GOLDEN G1: SACK gap seq'leri deadline'a göre (erken önce) sıralanır.
    let sched = DeadlineArqScheduler::new(RetxConfig::fixture());
    let lost = [
        LostSeq {
            seq: 10,
            deadline_ns: ms(300),
            retx_count: 0,
        },
        LostSeq {
            seq: 7,
            deadline_ns: ms(100),
            retx_count: 0,
        },
        LostSeq {
            seq: 5,
            deadline_ns: ms(200),
            retx_count: 0,
        },
    ];
    assert_eq!(sched.plan(&lost, 600), vec![rt(7), rt(5), rt(10)]);
}

#[test]
fn tv_arq_g2_spurious_raises_reorder_thresh() {
    // GOLDEN G2: reorder_thresh = max(K=3, ρ=2 × recent_max_reorder). Spurious → ↑, küçülmez.
    let cfg = RetxConfig::fixture();
    let mut tr = ReorderTracker::new();
    assert_eq!(tr.reorder_thresh(&cfg), 3); // max(3, 2×0)
    tr.note_spurious(4);
    assert_eq!(tr.reorder_thresh(&cfg), 8); // max(3, 2×4)
    tr.note_spurious(6);
    assert_eq!(tr.reorder_thresh(&cfg), 12); // max(3, 2×6)
    tr.note_spurious(2); // küçük gözlem → max korunur
    assert_eq!(tr.reorder_thresh(&cfg), 12);
}

#[test]
fn tv_arq_g3_retx_cap_and_policy() {
    // GOLDEN G3: retx_count ≥ cap → CapExceeded{policy}. Default HandToForward (recovery-obsesif).
    let sched = DeadlineArqScheduler::new(RetxConfig::fixture()); // cap=3
    let lost = [
        LostSeq {
            seq: 7,
            deadline_ns: ms(100),
            retx_count: 2,
        },
        LostSeq {
            seq: 8,
            deadline_ns: ms(200),
            retx_count: 3,
        },
    ];
    assert_eq!(
        sched.plan(&lost, 600),
        vec![
            rt(7),
            RetxDecision::CapExceeded {
                seq: 8,
                policy: CapExceedPolicy::HandToForward,
            },
        ]
    );

    // ConnectionFail politikası seçilebilir.
    let cfg = RetxConfig {
        cap_exceed: CapExceedPolicy::ConnectionFail,
        ..RetxConfig::fixture()
    };
    let sched2 = DeadlineArqScheduler::new(cfg);
    assert_eq!(
        sched2.plan(
            &[LostSeq {
                seq: 8,
                deadline_ns: ms(0),
                retx_count: 3,
            }],
            600
        ),
        vec![RetxDecision::CapExceeded {
            seq: 8,
            policy: CapExceedPolicy::ConnectionFail,
        }]
    );
}

#[test]
fn tv_arq_g4_fuzzy_arq_aggressiveness_gates() {
    // GOLDEN G4: arq_aggressiveness=0 → baskıla (boş); >0 → aktif. Fusion ARQ'yu sürer.
    let sched = DeadlineArqScheduler::new(RetxConfig::fixture());
    let lost = [
        LostSeq {
            seq: 7,
            deadline_ns: ms(100),
            retx_count: 0,
        },
        LostSeq {
            seq: 5,
            deadline_ns: ms(200),
            retx_count: 0,
        },
    ];
    assert!(sched.plan(&lost, 0).is_empty()); // fec-dominant baskılama
    assert_eq!(sched.plan(&lost, 600), vec![rt(7), rt(5)]); // aktif
}

#[test]
fn tv_arq_g5_retransmit_paced() {
    // GOLDEN G5: retransmit'ler token bucket'tan (§13) — burst'te storm YOK (INV-RATE-1).
    let mut tb = TokenBucket::new(62500, 2400);
    let mut sent = 0;
    for _ in 0..3 {
        if tb.try_send(1250) {
            sent += 1;
        } else {
            break;
        }
    }
    assert_eq!(sent, 1); // burst 2400 → sadece 1 retransmit (1250); kalanı paced

    tb.refill(Duration::from_millis(10)); // +625 → 1775
    assert!(tb.try_send(1250)); // 2. retransmit paced olarak gönderilir
}

#[test]
fn tv_arq_expired_deadline_not_dropped() {
    // NORMATİF: deadline yalnız öncelik; expired seq DROP EDİLMEZ (veri ULAŞSIN).
    let sched = DeadlineArqScheduler::new(RetxConfig::fixture());
    let lost = [LostSeq {
        seq: 5,
        deadline_ns: ms(50),
        retx_count: 0,
    }]; // deadline "geçmiş" varsay
    assert_eq!(sched.plan(&lost, 600), vec![rt(5)]); // planda KALIR (drop yok)

    // pace-beklerken deadline geçti → token gelince YİNE gönderilir.
    let mut tb = TokenBucket::new(62500, 2400);
    assert!(tb.try_send(1250)); // başka paket → tokens 1150
    assert!(!tb.try_send(1250)); // token yok → expired seq BEKLER (drop değil)
    tb.refill(Duration::from_millis(10)); // +625 → 1775
    assert!(tb.try_send(1250)); // geç ama gönderildi
}

#[test]
fn tv_arq_via_trait_object() {
    // Port temizliği: dyn ArqScheduler üzerinden.
    let sched = DeadlineArqScheduler::new(RetxConfig::fixture());
    let s: &dyn ArqScheduler = &sched;
    let lost = [LostSeq {
        seq: 7,
        deadline_ns: ms(100),
        retx_count: 0,
    }];
    assert_eq!(s.plan(&lost, 600), vec![rt(7)]);
}
