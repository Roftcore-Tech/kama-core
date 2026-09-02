//! N8-YENİDEN-DOĞRULAMA-v2 (5d-çıkış) — RECOVERED-wiring AÇIKKEN, canlı-pipeline (dış-API-only).
//!
//! K2-TAM-KAPANIŞ: (güvenlik-yarısı) recovered BtlBw/delivered-EAI'ye BİT-EKLEMEZ — fires-vs-noop
//! bit-özdeşlik; (muhasebe-yarısı, S5-3'te-ertelenmişti → ARTIK-BAĞLI) recovered → confirmed_logical
//! defterine ConfirmSource::Recovered ile GİRER + bekleyen-retransmit İPTAL edilir.
//!
//! Zincir: sender FEC-parity-emisyonu (send_record → RecoveredMap.register) → parity-kayıp →
//! receiver gerçek-FEC-decode-recovery (admit_and_decode) → Control::Recovered seal→open→decode
//! (wire-roundtrip) → engine resolve (block/esi→unit_id, ÇEVRİM-ENGINE'DE) → sender RecoveredControl.

#![allow(deprecated)] // enqueue_fec_parity: legacy-harness-yolu (Commit-K deprecation) — bu-dosya tam-o-yolu-test-eder
mod common;

use adaptive_transport::sender::{
    AckInfo, ConfirmSource, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, Timer,
    TransmitKind,
};
use kama_engine::{admit_and_decode, RecoveredMap};
use kama_noise_wire::control::decode_control;
use kama_noise_wire::OpenedTransportShard;
use kama_transport_schema::control::Control;
use std::time::Duration;

const MS: u64 = 1_000_000;

fn ack(seqs: &[u64]) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: seqs.to_vec(),
        ack_delay: Duration::ZERO,
    })
}

/// Canlı-senaryo (dış-API-only). `fires` → zincirin-sonunda RECOVERED sender'a verilir.
/// Döner: (btlbw, recovered_cancels, recovered-marker'lı-defter-girişi, wire-roundtrip-ok).
fn drive(fires: bool) -> (u64, u64, usize, bool) {
    // ── SENDER-tarafı: warm-up + FEC-parity-emisyonu (gerçek send_record'dan RecoveredMap.register) ──
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).expect("Manual-mode");
    let mut rmap = RecoveredMap::new();

    for i in 0..20u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit { id: i, size: 1200 }),
            i * 10 * MS,
        );
        let acks: Vec<u64> = out.iter().map(|d| d.seq).collect();
        if !acks.is_empty() {
            s.on_event(ack(&acks), i * 10 * MS + 5 * MS);
        }
    }
    // FEC-parity enqueue (block=1, esi 1 ve 3) → emit-anı send_record'dan RecoveredMap.register.
    s.enqueue_fec_parity(1, 1, 1200);
    s.enqueue_fec_parity(1, 3, 1200);
    let mut parity_seqs: Vec<u64> = Vec::new();
    for t in 0..40u64 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), (200 + t) * MS);
        for d in &out {
            if d.kind == TransmitKind::FecParity {
                let hint = d.send_record.expect("FecParity send_record taşır");
                let adaptive_transport::sender::SrLogicalRef::Fec { block_id, esi } =
                    hint.logical_ref;
                rmap.register(block_id, esi, d.unit_id); // ÇEVRİM-DEFTERİ: emit-anı, gerçek-karardan
                parity_seqs.push(d.seq);
            }
        }
    }
    assert_eq!(parity_seqs.len(), 2, "iki parity emit edildi (canlı)");
    // Parity-seq'ler ACK'LENMEZ (kayıp). TEK yüksek-seq DATA+ACK gap-detection'ı tetikler →
    // parity'ler pending_lost'a girer; RECOVERED hemen-ertesinde gelir (drenaj-fırsatı-olmadan —
    // gerçek-dünya-sırası: receiver kurtardı-bildirdi, sender retransmit-etmeden iptal-etti).
    let mut top: Vec<u64> = Vec::new();
    for j in 0..4u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id: 500 + j,
                size: 1200,
            }),
            (300 + j) * MS,
        );
        top.extend(
            out.iter()
                .map(|d| d.seq)
                .filter(|sq| !parity_seqs.contains(sq)),
        );
    }
    if !top.is_empty() {
        s.on_event(ack(&top), 306 * MS);
    }

    // ── RECEIVER-tarafı: GERÇEK FEC-decode-recovery (ayrı protector-çifti; kayıp source{1,3}) ──
    let (mut tx, mut rx) = common::pair(0x0700_00A1);
    let p = kama_fec_codec::by_id(0x03).unwrap();
    let app = vec![0x33u8; common::PAYLOAD_CAP * 5];
    let (sealed, k_actual) = common::encode_seal_block(&mut tx, &p, &app, 1, 0x4444, 0);
    let kept: Vec<OpenedTransportShard> = sealed
        .iter()
        .filter(|sh| sh.is_repair || !(sh.esi == 1 || sh.esi == 3))
        .map(|sh| rx.open(&sh.datagram).expect("authenticated"))
        .collect();
    let out = admit_and_decode(kept, 3, k_actual).expect("recovery");
    assert_eq!(out, app, "receiver GERÇEK-recovered (app-bytes tam)");

    // Receiver kurtardığı-esi'leri RECOVERED-control ile bildirir → WIRE-roundtrip (seal→open→decode).
    let recovered_ctl = Control::Recovered {
        id: 30,
        block_id: 1,
        esi_ranges: vec![(1, 1), (3, 3)],
    };
    let ctl_dg = common::seal_control(&mut tx, recovered_ctl.clone(), 5000);
    let opened_ctl = rx.open(&ctl_dg).expect("control authenticated");
    let got = decode_control(opened_ctl.inner())
        .expect("control-frame")
        .expect("decode");
    let wire_ok = got == recovered_ctl;

    // ── ENGINE-çevrim + SENDER-teslim (yalnız fires) ──
    if fires {
        let (resolved, unresolved) = rmap.resolve(&got).expect("Recovered çözümü");
        assert_eq!(
            resolved.len(),
            2,
            "iki logical çözüldü (emit-anı-defterinden)"
        );
        assert_eq!(unresolved, 0);
        let units: Vec<RecoveredUnit> = resolved
            .iter()
            .map(|r| RecoveredUnit {
                unit_id: r.unit_id,
                block_id: r.block_id,
                esi: r.esi,
            })
            .collect();
        let emit = s.on_event(Event::RecoveredControl(RecoveredInfo { units }), 400 * MS);
        assert!(emit.is_empty(), "saf-state: RECOVERED emit-üretmez");
    }
    // Post-tick'ler (ACK-yok → BtlBw iki-kolda-da değişmemeli).
    for t in 0..5u64 {
        s.on_event(Event::TimerFired(Timer::Pto), (450 + t * 10) * MS);
    }

    let recovered_entries = s
        .take_confirmed_logical()
        .iter()
        .filter(|(_, _, src)| *src == ConfirmSource::Recovered)
        .count();
    (
        s.diag_rate().1,
        s.diag_recovered_cancels(),
        recovered_entries,
        wire_ok,
    )
}

#[test]
fn n8_v2_recovered_wiring_ledger_bound_btlbw_unchanged() {
    let (btlbw_noop, canc_noop, led_noop, wire_noop) = drive(false);
    let (btlbw_fires, canc_fires, led_fires, wire_fires) = drive(true);

    eprintln!("[N8-v2] noop : btlbw={btlbw_noop} cancels={canc_noop} recovered_ledger={led_noop}");
    eprintln!(
        "[N8-v2] fires: btlbw={btlbw_fires} cancels={canc_fires} recovered_ledger={led_fires}"
    );

    assert!(
        wire_noop && wire_fires,
        "RECOVERED wire-roundtrip (seal→open→decode) doğru"
    );
    // MUHASEBE-YARISI ARTIK-BAĞLI (S5-3'te-ertelenmişti): recovered → defter (Recovered-marker).
    assert_eq!(
        led_fires, 2,
        "fires: recovered→confirmed_logical GİRDİ (muhasebe-yarısı BAĞLI)"
    );
    assert_eq!(led_noop, 0, "noop: recovered-girişi yok");
    // Retransmit-cancel gerçekleşti (FEC ana-avantajı; M1-sinyali).
    assert!(
        canc_fires > 0,
        "fires: bekleyen-retransmit İPTAL (diag_recovered_cancels>0)"
    );
    assert_eq!(canc_noop, 0, "noop: cancel-yok");
    // GÜVENLİK-YARISI KORUNDU: recovered BtlBw'ye bit-eklemez (fires-vs-noop BİT-ÖZDEŞ).
    assert_eq!(
        btlbw_fires, btlbw_noop,
        "N8-v2: RECOVERED-wiring-AÇIKKEN BtlBw bit-özdeş ({btlbw_fires} == {btlbw_noop})"
    );
}
