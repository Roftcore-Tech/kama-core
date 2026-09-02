//! [DR-EST-1] PreConverge kuruluş-denemesi pinleri.
//!
//! Kök-vaka (S2-REGION rt60d300): ilk-ölçüm-öncesi PRE_CONVERGE_FLOOR_BW=31 B/s pacer'ı §3.3-B
//! kopya-makinesini token-kapısında boğuyordu → sıkı-deadline + ağır-iki-yön-kayıpta teyit-zinciri
//! HİÇ kurulamıyordu (0/40). Bu pinler: (1) floor altında backoff'lu token-muaf kuruluş-kopyaları
//! AKAR; (2) ilk teyitle (PreConverge-çıkışı) kol SÖNER; (3) vade EK4-A2 uyandırma-kümesinde.

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use std::time::Duration;

const MS: u64 = 1_000_000;

fn pump_collect(s: &mut Sender, from_ms: u64, to_ms: u64, step_ms: u64) -> Vec<(u64, u64)> {
    // (t_ms, o tick'te emit edilen Duplicate sayısı)
    let mut out = Vec::new();
    let mut t = from_ms;
    while t <= to_ms {
        let d = s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        let dups = d
            .iter()
            .filter(|x| x.kind == TransmitKind::Duplicate)
            .count() as u64;
        if dups > 0 {
            out.push((t, dups));
        }
        t += step_ms;
    }
    out
}

/// (1) Floor-boğulmasında kuruluş-kopyaları backoff'la akar: 12 s ACK'siz PreConverge'de
/// kol-sayacı 4..=6 (vadeler ~0.5,1,2,4,8 s) ve tick-başına tek datagram (TEK-slot).
#[test]
fn establish_probes_flow_under_preconverge_floor() {
    let mut s = Sender::new();
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 101 }), 0);
    let emits = pump_collect(&mut s, 10, 12_000, 10);
    let n = s.diag_establish_probe_emitted();
    assert!(
        (4..=6).contains(&n),
        "backoff-serisi ~5 emisyon vermeli (0.5+1+2+4+8s), gelen: {n} ({emits:?})"
    );
    assert!(
        emits.iter().all(|&(_, k)| k == 1),
        "TEK-slot: tick başına en çok 1 kuruluş-kopyası: {emits:?}"
    );
    // Backoff gerçekten büyüyor: ardışık emisyon-aralıkları azalmıyor (±1 tick toleransı).
    let times: Vec<u64> = emits.iter().map(|&(t, _)| t).collect();
    for w in times.windows(3) {
        let (a, b) = (w[1] - w[0], w[2] - w[1]);
        assert!(b + 20 >= a, "aralıklar daralmamalı: {times:?}");
    }
}

/// (2) İlk teyit (PreConverge-çıkışı) kolu söndürür: ACK-sonrası sayaç DONAR.
#[test]
fn establish_probe_stops_on_first_confirmation() {
    let mut s = Sender::new();
    let d0 = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 101 }), 0);
    let first_seq = d0
        .iter()
        .find(|x| x.kind == TransmitKind::Data)
        .map(|x| x.seq)
        .expect("ilk DATA emit edilir (pacer quantum)");
    let _ = pump_collect(&mut s, 10, 700, 10); // ~0.5s vadesi ateşlesin
    let before = s.diag_establish_probe_emitted();
    assert!(before >= 1, "ilk vade ateşlemiş olmalı");
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![first_seq],
            ack_delay: Duration::ZERO,
        }),
        800 * MS,
    );
    let _ = pump_collect(&mut s, 810, 12_000, 10);
    assert_eq!(
        s.diag_establish_probe_emitted(),
        before,
        "teyit-sonrası kuruluş-kolu SÖNER (PreConverge-çıkışı)"
    );
}

/// (3) EK4-A2: armed-vade `next_wakeup_ns` kümesinde (event-loop uyanabilir).
#[test]
fn establish_due_is_in_wakeup_set() {
    let mut s = Sender::new();
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 101 }), 0);
    let _ = s.on_event(Event::TimerFired(Timer::Pto), 10 * MS); // lazy-arm
    let w = s.next_wakeup_ns().expect("armed-vade varken Some");
    assert!(
        w <= 520 * MS,
        "uyandırma ilk kuruluş-vadesini (≈510ms) kaçırmamalı: {w}"
    );
}
