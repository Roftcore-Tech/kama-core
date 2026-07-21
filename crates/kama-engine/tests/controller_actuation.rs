//! Commit-H — CONTROLLER-ACTUATION ENGINE-WIRING + variable-r ENGINE-EMISSION matrisi (§14/§15).
//!
//! Zincir (GERÇEK-emisyon-yolu): controller_decision_snapshot (same-event) → fec_controller_latch →
//! planned_r × enqueue_fec_parity → plan_fec_parity (token; Pto-tick) → FecParity-decisions
//! (send_record-hint) → seal → receiver admit_and_decode. Ratio-ekseni testlerde snapshot-ratio-
//! override'ıyla sürülür (event_index GERÇEK sender-index'i — same-event-guard-korunur; fixture-fuzzy
//! canlı-loss'tan hedef-ratio-üretmek kırılgan-olurdu). Manual-mode non-regression: mevcut-tüm-harness
//! (sprint5c/n8/m1/n8_v2) Manual-yolda; Controller default-DEĞİL.

mod common;

use adaptive_transport::sender::{
    ControllerDecisionSnapshot, Event, FecActuationMode, FecSuppressionReason, OpaqueUnit, Sender,
    SrLogicalRef, Timer, TransmitKind,
};
use kama_engine::admit_and_decode;
use kama_fec_codec::{by_id, FecCodec, RsSimdCodec, SourceSymbol};
use kama_noise_wire::{KamaNoiseWireProtector, OpenedTransportShard};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};

const MS: u64 = 1_000_000;
const PID: u8 = 0x03; // K8 R4 E64 → r_max=4

/// Engine-path tek-blok: sender-Controller-latch→enqueue→EMİSYON; emit-edilen-parity'ler GERÇEK-RS-
/// repair-symbol'lerle seal-edilir; source'lar seal-edilir; kayıp-uygulanır; receiver-decode.
/// Döner: (decode-sonucu, requested_r, planned_r, emitted_this_block, k_actual).
#[allow(clippy::too_many_arguments)]
fn engine_block(
    s: &mut Sender,
    tx: &mut KamaNoiseWireProtector,
    rx: &mut KamaNoiseWireProtector,
    block_id: u32,
    k_units: usize,
    base_pn: u64,
    ratio_override: u32,
    drop_source_esi: &[u16],
    t0_ms: u64,
) -> (Result<Vec<u8>, kama_engine::EngineError>, u16, u16, u64, u8) {
    let p = by_id(PID).unwrap();
    let e = p.e_usize();
    let k_shape = p.k_shape as usize;
    let app = vec![0x33u8; common::PAYLOAD_CAP * k_units];

    // Canonical-block (source-symbol'ler + RS-repair'ler) — harness-codec (INV-FEC-1: seal-öncesi-encode).
    let chunks: Vec<&[u8]> = app.chunks(common::PAYLOAD_CAP).collect();
    let k_actual = chunks.len() as u8;
    let mut block: Vec<Vec<u8>> = Vec::new();
    for (i, c) in chunks.iter().enumerate() {
        let pfx = SourcePrefix {
            message_id: 0x6000 + block_id,
            chunk_offset: (i * common::PAYLOAD_CAP) as u32,
            actual_payload_len: c.len() as u16,
            source_flags: u8::from(i == chunks.len() - 1),
        };
        let mut sym = vec![0u8; e];
        pfx.serialize_symbol(c, &mut sym).unwrap();
        block.push(sym);
    }
    while block.len() < k_shape {
        block.push(vec![0u8; e]);
    }
    let srcs: Vec<SourceSymbol> = block
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let repairs = RsSimdCodec.encode(&p, &srcs).unwrap();

    // (1) Blok-DATA'sı sender'dan akar; BATCH-ACK (SACK-deseni — per-paket-anlık-ACK app-limited-
    // örnek-üretir, btlbw-0-kalır; m1-harness-kalıbı). PreConverge→Startup + btlbw-canlı.
    let mut pend: Vec<u64> = Vec::new();
    for i in 0..k_units as u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id: u64::from(block_id) * 100 + i,
                size: 1200,
            }),
            t0_ms * MS + i * MS,
        );
        pend.extend(out.iter().map(|d| d.seq));
        if pend.len() >= 4 {
            s.on_event(
                Event::AckReceived(adaptive_transport::sender::AckInfo {
                    acked_seqs: std::mem::take(&mut pend),
                    ack_delay: std::time::Duration::ZERO,
                }),
                t0_ms * MS + i * MS + MS / 2,
            );
        }
    }
    if !pend.is_empty() {
        s.on_event(
            Event::AckReceived(adaptive_transport::sender::AckInfo {
                acked_seqs: pend,
                ack_delay: std::time::Duration::ZERO,
            }),
            t0_ms * MS + k_units as u64 * MS,
        );
    }
    // (2) BLOK-TAMAM (aynı-event-bağlamı): snapshot → latch → planned_r × enqueue.
    let snap = s.controller_decision_snapshot().expect("fuzzy-koştu");
    let snap = ControllerDecisionSnapshot {
        ratio_permille: ratio_override,
        ..snap
    };
    let latch = s.fec_controller_latch(Some(snap), k_actual, p.r_code_max);
    for esi in 0..latch.planned_r {
        // repair-esi-uzayı 0-based (codec: esi<r_code_max; is_repair-ayrımı AYRI-uzay).
        s.enqueue_fec_parity(block_id, esi, 64);
    }
    // (3) EMİSYON (gerçek-yol: plan_fec_parity token'la) → FecParity-decisions.
    let mut emitted_esis: Vec<u16> = Vec::new();
    for t in 0..200u64 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), (t0_ms + 100 + t * 10) * MS);
        for d in &out {
            if d.kind == TransmitKind::FecParity {
                let hint = d.send_record.expect("hint");
                let SrLogicalRef::Fec { block_id: b, esi } = hint.logical_ref;
                if b == block_id {
                    emitted_esis.push(esi);
                }
            }
        }
        if emitted_esis.len() as u16 >= latch.planned_r {
            break;
        }
    }
    // (4) SEAL: source'lar + YALNIZ-emit-edilen-parity'ler (emisyon-yolu-sadakati).
    let mut pn = base_pn;
    let mut dgs: Vec<Vec<u8>> = Vec::new();
    for (i, sym) in block.iter().enumerate().take(usize::from(k_actual)) {
        if !drop_source_esi.contains(&(i as u16)) {
            let pkt = UnsealedTransportPacket::FecShard {
                header: LogicalShardHeader {
                    kind: ShardKind::Source,
                    profile_id: PID,
                    block_id,
                    esi: i as u16,
                    k_actual,
                },
                symbol: sym.clone(),
            };
            dgs.push(
                tx.seal(&pkt, adaptive_transport_seal_args(pn))
                    .unwrap()
                    .bytes,
            );
        }
        pn += 1;
    }
    for esi in &emitted_esis {
        let rep = repairs
            .iter()
            .find(|r| r.esi == *esi)
            .expect("emit-esi RS-repair-uzayında");
        let pkt = UnsealedTransportPacket::FecShard {
            header: LogicalShardHeader {
                kind: ShardKind::Repair,
                profile_id: PID,
                block_id,
                esi: rep.esi,
                k_actual,
            },
            symbol: rep.bytes.clone(),
        };
        dgs.push(
            tx.seal(&pkt, adaptive_transport_seal_args(pn))
                .unwrap()
                .bytes,
        );
        pn += 1;
    }
    // (5) RECEIVER.
    let opened: Vec<OpenedTransportShard> = dgs
        .iter()
        .map(|d| rx.open(d).expect("authenticated"))
        .collect();
    (
        admit_and_decode(opened, PID, k_actual),
        latch.computed_r,
        latch.planned_r,
        emitted_esis.len() as u64,
        k_actual,
    )
}

fn adaptive_transport_seal_args(pn: u64) -> kama_noise_wire::SealArgs {
    kama_noise_wire::SealArgs {
        wire_pn: pn,
        dh_epoch: 0,
        send_generation: 0,
    }
}

fn controller_sender() -> Sender {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    s
}

// ── §15 VARIABLE-R ENGINE-MATRİSİ: A r=0 · B r=1 · C r=Rmax · D r=2 (ratio 0/125/500/250) ──
#[test]
fn engine_matrix_consecutive_variable_r() {
    let mut s = controller_sender();
    let (mut tx, mut rx) = common::pair(0x0900_0001);
    let expect = vec![0x33u8; common::PAYLOAD_CAP * 8];
    // A: ratio=0 → r=0; kayıpsız source-only.
    let (out, req, plan, emit, _) = engine_block(&mut s, &mut tx, &mut rx, 1, 8, 0, 0, &[], 0);
    assert_eq!((req, plan, emit), (0, 0, 0), "A: r=0 zinciri");
    assert_eq!(out.unwrap(), expect);
    // B: ratio=125 → r=1; 1-kayıp recoverable.
    let (out, req, plan, emit, _) =
        engine_block(&mut s, &mut tx, &mut rx, 2, 8, 100, 125, &[3], 3000);
    assert_eq!((req, plan), (1, 1));
    assert_eq!(emit, 1, "B: emisyon-yolu 1-parity");
    assert_eq!(out.unwrap(), expect);
    // C: ratio=500 → r=4=Rmax; 2-kayıp recoverable.
    let (out, req, plan, emit, _) =
        engine_block(&mut s, &mut tx, &mut rx, 3, 8, 200, 500, &[1, 5], 6000);
    assert_eq!((req, plan), (4, 4));
    assert_eq!(emit, 4);
    assert_eq!(out.unwrap(), expect);
    // D: ratio=250 → r=2; out-of-order + duplicate zaten preflight'ta; burada kayıpsız.
    let (out, req, plan, emit, _) =
        engine_block(&mut s, &mut tx, &mut rx, 4, 8, 300, 250, &[], 9000);
    assert_eq!((req, plan, emit), (2, 2, 2));
    assert_eq!(out.unwrap(), expect);
    // Diag-zinciri: enqueued==emitted-toplamı (0+1+4+2=7); requested→emitted-ayrımı-görünür.
    let d = s.diag_fec_actuation();
    assert_eq!(d.enqueued_r_total, 7);
    assert_eq!(d.emitted_r_total, 7);
}

// ── unrecoverable + r>emit-yarışı + clamp (engine-yolu) ──
#[test]
fn engine_matrix_loss_and_clamp() {
    let mut s = controller_sender();
    let (mut tx, mut rx) = common::pair(0x0900_0002);
    // ratio=125→r=1; 2-kayıp → UNRECOVERABLE typed (stuck-yok).
    let (out, ..) = engine_block(&mut s, &mut tx, &mut rx, 1, 8, 0, 125, &[2, 6], 0);
    assert!(out.is_err(), "unrecoverable typed-hata");
    // ratio=1000 → computed=8 → CLAMP r_max=4 (engine-yolunda).
    let (out, req, plan, emit, _) =
        engine_block(&mut s, &mut tx, &mut rx, 2, 8, 100, 1000, &[0, 7], 3000);
    assert_eq!((req, plan), (8, 4), "clamp: computed=8→planned=4");
    assert_eq!(emit, 4);
    assert_eq!(out.unwrap(), vec![0x33u8; common::PAYLOAD_CAP * 8]);
    // Sonraki-blok r=0 (r>0→r=0 ardışık; contamination-yok).
    let (out, _, plan, emit, _) = engine_block(&mut s, &mut tx, &mut rx, 3, 8, 200, 0, &[], 6000);
    assert_eq!((plan, emit), (0, 0));
    assert!(out.is_ok());
}

// ── §14: partial-block (k_actual=5) engine-yolu + emergency-suppression engine-yolu ──
#[test]
fn engine_partial_block_and_emergency() {
    let mut s = controller_sender();
    let (mut tx, mut rx) = common::pair(0x0900_0003);
    let expect5 = vec![0x33u8; common::PAYLOAD_CAP * 5];
    // partial k_actual=5, ratio=500 → r=(500×5+500)/1000=3; 1-kayıp recoverable.
    let (out, req, plan, emit, ka) = engine_block(&mut s, &mut tx, &mut rx, 1, 5, 0, 500, &[2], 0);
    assert_eq!(ka, 5);
    assert_eq!(
        (req, plan, emit),
        (3, 3, 3),
        "k_actual-formülü (nominal-K-değil)"
    );
    assert_eq!(out.unwrap(), expect5);
    // Emergency-suppression (elle-emergency-snapshot): requested-görünür, planned/emitted=0.
    s.on_event(
        Event::AppData(OpaqueUnit {
            id: 9000,
            size: 1200,
        }),
        5000 * MS,
    );
    let snap = s.controller_decision_snapshot().unwrap();
    let em = ControllerDecisionSnapshot {
        ratio_permille: 500,
        emergency: true,
        ..snap
    };
    let latch = s.fec_controller_latch(Some(em), 8, 4);
    assert_eq!(latch.computed_r, 4, "requested-telemetride-görünür");
    assert_eq!(latch.planned_r, 0, "emitted/enqueued=0 (interlock)");
    assert_eq!(
        latch.suppression,
        Some(FecSuppressionReason::EmergencyPendingArbitration)
    );
}

// ── §14 transitions: idle M→C · open-block-boundary-kontrat · C→M(false/true) · pending-yok ──
#[test]
fn engine_mode_transitions() {
    let mut s = Sender::new();
    // Idle M→C: hemen (blok-yok).
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    assert_eq!(
        s.diag_fec_actuation().authority_mode,
        FecActuationMode::Controller
    );
    // C→M(false): açık-belirtim; latch artık-ManualDisabled.
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap();
    let out = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 1200 }), 0);
    // DATA'yı-ACK'le (token'ı-parity-fazına-bırak; amaç-Manual-emit-yolu).
    let acks: Vec<u64> = out.iter().map(|d| d.seq).collect();
    if !acks.is_empty() {
        s.on_event(
            Event::AckReceived(adaptive_transport::sender::AckInfo {
                acked_seqs: acks,
                ack_delay: std::time::Duration::ZERO,
            }),
            MS,
        );
    }
    let snap = s.controller_decision_snapshot().unwrap();
    let r = s.fec_controller_latch(Some(snap), 8, 4);
    assert_eq!(r.suppression, Some(FecSuppressionReason::ManualDisabled));
    // C→M(true): manuel-sabit-akış (enqueue-yolu) devralır — plan_fec_parity Manual{true}-emit-eder.
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    s.enqueue_fec_parity(7, 8, 64);
    let mut emitted = 0;
    for t in 1..200u64 {
        emitted += s
            .on_event(Event::TimerFired(Timer::Pto), t * 10 * MS)
            .iter()
            .filter(|d| d.kind == TransmitKind::FecParity)
            .count();
        if emitted > 0 {
            break;
        }
    }
    assert_eq!(emitted, 1, "Manual-enabled: mevcut-fixed-FEC-yolu aynen");
    // Open-block-boundary-KONTRATI: mode-değişimi blok-sınırında-çağrılır (engine-kontratı) —
    // authority yalnız latch/enqueue-noktalarında-okunduğundan mid-block-karışım-yapısal-yok
    // (bu-testte: blok-içi-mode-değişimi-yapılmadı; kontrat-belge+engine-akışı).
}

// ── §13: Controller default-DEĞİL + Manual-disabled-baseline (varsayılan) ──
#[test]
fn controller_not_default_manual_baseline() {
    let s = Sender::new();
    assert_eq!(
        s.diag_fec_actuation().authority_mode,
        FecActuationMode::Manual { enabled: false },
        "default=Manual{{false}} — Controller-default-DEĞİL; FEC-off-baseline (C-kuralı) korunur"
    );
}
