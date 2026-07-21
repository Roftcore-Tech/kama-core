//! Sprint-5c DR-S5-1: FecParity koşullu-emit + SealedPacketMeta-hook.
//! guard-OFF → emit-yok (hot-path byte-değişmez, C-kuralı) · guard-ON → emit + send_record · DATA-ÖNCE korunur.

use adaptive_transport::sender::{
    Event, OpaqueUnit, Sender, SrLogicalRef, SrPacketClass, Timer, TransmitDecision, TransmitKind,
};

const MS: u64 = 1_000_000;
const UNIT: u64 = 267;

/// n birim app-data enqueue et (emisyon-dönüşünü topla) + ticks-Pto sür → TÜM decisions.
fn run(s: &mut Sender, n_data: u64, ticks: u64) -> Vec<TransmitDecision> {
    let mut out = Vec::new();
    for i in 0..n_data {
        out.extend(s.on_event(Event::AppData(OpaqueUnit { id: i, size: UNIT }), 0));
    }
    for i in 0..ticks {
        out.extend(s.on_event(Event::TimerFired(Timer::Pto), i * MS));
    }
    out
}

// ── C-KURALI: guard-OFF → enqueue-edilse-de FecParity emit-EDİLMEZ (hot-path byte-değişmez) ──
#[test]
fn fec_off_no_emit() {
    let mut s = Sender::new();
    s.enqueue_fec_parity(7, 0, UNIT);
    s.enqueue_fec_parity(7, 1, UNIT);
    let decisions = run(&mut s, 1, 300);
    assert!(
        decisions.iter().all(|d| d.kind != TransmitKind::FecParity),
        "guard-OFF: FecParity emit-EDİLMEZ (allocation-yok)"
    );
    assert!(
        decisions.iter().all(|d| d.send_record.is_none()),
        "guard-OFF: send_record hep None"
    );
}

// ── guard-ON → FecParity emit + send_record(Repair, block_id/esi) ──
#[test]
fn fec_on_emits_with_send_record() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).expect("Manual-mode");
    s.enqueue_fec_parity(7, 0, UNIT);
    s.enqueue_fec_parity(7, 3, UNIT);
    let decisions = run(&mut s, 0, 800);
    let fec: Vec<_> = decisions
        .iter()
        .filter(|d| d.kind == TransmitKind::FecParity)
        .collect();
    assert_eq!(fec.len(), 2, "iki parity emit edildi");
    for d in &fec {
        let sr = d.send_record.expect("FecParity send_record taşır");
        assert_eq!(sr.packet_class, SrPacketClass::Repair);
        match sr.logical_ref {
            SrLogicalRef::Fec { block_id, esi } => {
                assert_eq!(block_id, 7);
                assert!(esi == 0 || esi == 3);
            }
        }
    }
    // DATA yolunda send_record None (yalnız FecParity Some).
    assert!(decisions
        .iter()
        .filter(|d| d.kind != TransmitKind::FecParity)
        .all(|d| d.send_record.is_none()));
}

// ── DATA-ÖNCE mühürlü: ilk-DATA ilk-FecParity'den ÖNCE (schedule DATA-önce sıralaması) ──
#[test]
fn data_before_fec_parity() {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).expect("Manual-mode");
    s.enqueue_fec_parity(1, 0, UNIT);
    // Uzun-sür (pacing sub-kbps olsa da tüm-birimler drain olsun; ordering pacing-bağımsız).
    let decisions = run(&mut s, 3, 60_000);
    let data_ct = decisions
        .iter()
        .filter(|d| d.kind == TransmitKind::Data)
        .count();
    let fec_ct = decisions
        .iter()
        .filter(|d| d.kind == TransmitKind::FecParity)
        .count();
    assert_eq!(data_ct, 3, "3 DATA emit (hepsi drain)");
    assert_eq!(fec_ct, 1, "1 FecParity emit");
    // DATA-ÖNCE mühürlü: plan_fec_parity drain_new_data'dan SONRA çağrılır → tüm-DATA FecParity'den ÖNCE.
    let last_data = decisions
        .iter()
        .rposition(|d| d.kind == TransmitKind::Data)
        .unwrap();
    let first_fec = decisions
        .iter()
        .position(|d| d.kind == TransmitKind::FecParity)
        .unwrap();
    assert!(
        last_data < first_fec,
        "DATA-ÖNCE mühürlü: tüm-DATA FecParity'den önce"
    );
}
