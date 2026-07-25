//! Commit-W — LEFTOVER-EŞDEĞERLİK-PİNİ (feature-GATE'SİZ: hem-OFF hem-ON-derlemesinde-AYNI-sabitler).
//! Kanıt-modeli: deterministik-mini-senaryo → karar-dizisi/sayaç-SABİTLERİ. OFF-koşusu=baseline;
//! ON-koşusu (`--features experimental-fec-budget`; policy-dokunulmaz=LeftoverOnly-default) AYNI-
//! sabitleri-geçmeli → "ON+LeftoverOnly ≡ OFF-baseline" karar-düzeyi-kanıtı. (Ek-kanıt: tüm-mevcut-
//! suite — N8/golden/sprint5c/M1-spot — iki-modda-da-yeşil; Commit-V-kapıları.)

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use std::time::Duration;

const MS: u64 = 1_000_000;

#[test]
#[allow(deprecated)] // legacy-enqueue: baseline-yolun-parçası (pin-senaryosu)
fn leftover_equivalence_pin_constants() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();
    // Deterministik-mini-senaryo: 6-DATA (batch-ACK) + 3-parity-enqueue + PTO-drenajı.
    let mut decisions: Vec<(u8, u64, u64)> = Vec::new(); // (kind-tag, seq, size)
    let push = |out: &[adaptive_transport::sender::TransmitDecision],
                acc: &mut Vec<(u8, u64, u64)>| {
        for d in out {
            let k = match d.kind {
                TransmitKind::Data => 0u8,
                TransmitKind::Retransmit => 1,
                TransmitKind::Duplicate => 2,
                TransmitKind::FecParity => 3,
                #[cfg(feature = "experimental-cold-start")]
                TransmitKind::CsProbe => 4,
            };
            acc.push((k, d.seq, d.size));
        }
    };
    let mut seqs = Vec::new();
    for i in 0..6u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id: 100 + i,
                size: 1200,
            }),
            i * MS,
        );
        seqs.extend(out.iter().map(|d| d.seq));
        push(&out, &mut decisions);
    }
    let out = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: seqs,
            ack_delay: Duration::ZERO,
        }),
        7 * MS,
    );
    push(&out, &mut decisions);
    for esi in 0..3u16 {
        s.enqueue_fec_parity(9, esi, 400);
    }
    for t in 0..40u64 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), (10 + t * 10) * MS);
        push(&out, &mut decisions);
    }
    let d = s.diag_fec_actuation();
    // PİN-SABİTLERİ (feature-OFF-baseline'dan-ölçüldü; ON-koşusu-AYNI-değerleri-geçmeli):
    assert_eq!(d.enqueued_r_total, 3, "pin: enqueued");
    assert_eq!(d.emitted_r_total, 3, "pin: emitted (leftover-yolunda-3/3)");
    let parity_count = decisions.iter().filter(|x| x.0 == 3).count();
    let data_count = decisions.iter().filter(|x| x.0 == 0).count();
    assert_eq!((data_count, parity_count), (6, 3), "pin: karar-sayıları");
    // DATA-ÖNCE-sıra-pini: son-DATA-index < ilk-parity-index.
    let last_data = decisions.iter().rposition(|x| x.0 == 0).unwrap();
    let first_par = decisions.iter().position(|x| x.0 == 3).unwrap();
    assert!(last_data < first_par, "pin: DATA-önce-parity");
    // Karar-dizisi-özet-pini (toplam-byte + son-seq): rejim-drift'ini-yakalar.
    let total_bytes: u64 = decisions.iter().map(|x| x.2).sum();
    let max_seq = decisions.iter().map(|x| x.1).max().unwrap();
    assert_eq!(total_bytes, 6 * 1200 + 3 * 400, "pin: toplam-byte");
    assert_eq!(max_seq, 8, "pin: son-seq (6-DATA + 3-parity − 1)");
}
