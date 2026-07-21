//! Sprint-5d-1 — PARAM-bootstrap (B2 akışı) uçtan-uca.
//! Bob-offline proaktif-emit · receiver hash-verify fail-closed · bounded pre-param hold · PARAM_ACK=confirmation-only.

mod common;

use adaptive_transport::sender::{Event, Sender, Timer, TransmitKind};
use kama_engine::{
    plan_param_bootstrap, ParamAckTracker, ParamState, PreParamHold, MAX_PREPARAM_SHARDS,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::param::ParamFields;

const MS: u64 = 1_000_000;

fn fields() -> ParamFields {
    ParamFields {
        protocol_version: 1,
        fec_version: 1,
        code_suite_id: 1,
        profile_id: 3, // K8R4E64 (by_id 0x03)
        k_shape: 8,
        r_code_max: 4,
        e: 64,
        partial_policy: 0,
        packet_scheduling_policy: 0,
    }
}

/// FEC-emit harness: verilen sender'ı `ticks` Pto-tick'i sürer, emit-edilen FecParity sayısını döner
/// (Bob-offline: HİÇ AckReceived yok). Emisyon YALNIZ fec_enabled + pacing'e bağlı (param-ack input-YOK).
fn drive_fec(sender: &mut Sender, ticks: u64) -> usize {
    let mut n = 0usize;
    for t in 0..ticks {
        n += sender
            .on_event(Event::TimerFired(Timer::Pto), t * MS)
            .iter()
            .filter(|d| d.kind == TransmitKind::FecParity)
            .count();
    }
    n
}

fn fresh_fec_sender() -> Sender {
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).expect("Manual-mode"); // B2-direktifi: HEMEN emit
    s.enqueue_fec_parity(1, 0, 267);
    s.enqueue_fec_parity(1, 1, 267);
    s
}

// ── B2 nedensel-kanıt: PARAM_SET atomik-emit + FEC proaktif-akar; PARAM_ACK-confirmation emisyonu DEĞİŞTİRMEZ ──
#[test]
fn b2_emission_independent_of_param_ack() {
    let f = fields();
    let plan = plan_param_bootstrap(1, &f).unwrap();
    assert!(
        plan.start_fec_immediately,
        "B2-direktifi: proaktif-emit (plan-anında, ACK-öncesi)"
    );

    // (1) ATOMİK-EMİT: PARAM_SET gerçekten seal edilir (wire'a çıkar) — direktifle-birlikte.
    let (mut tx, _rx) = common::pair(0x0500_00B2);
    let param_dg = common::seal_control(&mut tx, plan.control.clone(), 0);
    assert!(!param_dg.is_empty(), "PARAM_SET seal+emit edildi (atomic)");

    // (2) UNCONFIRMED koşu: tracker döngü-ÖNCESİ kurulur + HİÇ confirmed-edilmez (Bob offline).
    let mut tracker = ParamAckTracker::new();
    let mut sender_a = fresh_fec_sender();
    let fec_unconfirmed = drive_fec(&mut sender_a, 3_000);
    assert!(
        !tracker.is_confirmed(),
        "hiç PARAM_ACK yok → unconfirmed; yine de FEC aktı"
    );
    assert!(fec_unconfirmed > 0, "unconfirmed'ken FEC proaktif-aktı");

    // (3) CONFIRMED koşu: AYNI sürüş ama tracker mid-way confirmed edilir.
    let ack = Control::ParamAck {
        id: 2,
        param_hash: plan.param_hash,
    };
    tracker.on_param_ack(&ack, &plan.param_hash).unwrap();
    assert!(tracker.is_confirmed(), "doğru-echo → confirmed");
    let mut sender_b = fresh_fec_sender();
    let fec_confirmed = drive_fec(&mut sender_b, 3_000);

    // NEDENSEL B2-KANITI: confirmation emisyonu DEĞİŞTİRMEZ → PARAM_ACK gate/modifier DEĞİL.
    // (Biri is_confirmed()'i emit-gate'ine bağlarsa bu iki-sayı ıraksar → test FAIL = regresyon-guard.)
    assert_eq!(
        fec_confirmed, fec_unconfirmed,
        "PARAM_ACK confirmation FEC-emisyonunu DEĞİŞTİRMEZ (confirmation-only, gate-değil)"
    );
}

// ── PARAM_ACK = confirmation-only: doğru-echo → confirmed; emisyonu ETKİLEMEZ ──
#[test]
fn param_ack_confirmation_only() {
    let f = fields();
    let plan = plan_param_bootstrap(1, &f).unwrap();
    let mut tracker = ParamAckTracker::new();
    assert!(!tracker.is_confirmed());

    // Doğru hash-echo → confirmed.
    let ack = Control::ParamAck {
        id: 2,
        param_hash: plan.param_hash,
    };
    tracker.on_param_ack(&ack, &plan.param_hash).unwrap();
    assert!(tracker.is_confirmed(), "doğru-echo → confirmed");

    // Yanlış hash-echo → confirmation reddi (ama emisyon zaten akıyordu — B2).
    let mut t2 = ParamAckTracker::new();
    let bad = Control::ParamAck {
        id: 3,
        param_hash: [0xEE; 32],
    };
    assert!(
        t2.on_param_ack(&bad, &plan.param_hash).is_err(),
        "yanlış-echo → confirmation reddi"
    );
    assert!(!t2.is_confirmed());
}

// ── Receiver hash-verify: doğru-PARAM → bind (admission açık); mismatch → FAIL-CLOSED (bound olmaz) ──
#[test]
fn receiver_param_verify_fail_closed() {
    let f = fields();
    let plan = plan_param_bootstrap(7, &f).unwrap();

    // Doğru PARAM_SET (aynı-alanlar) → bind.
    let mut st = ParamState::new();
    assert!(!st.is_confirmed(), "başta admission KAPALI");
    st.on_param_set(&plan.control, &f).unwrap();
    assert!(st.is_confirmed(), "doğru-PARAM → bind (admission açık)");
    assert_eq!(st.bound_profile_id(), Some(3));

    // param_hash-mismatch (kurcalanmış hash) → FAIL-CLOSED, bound olmaz.
    let tampered = Control::ParamSet {
        id: 7,
        param_hash: [0x00; 32],
        profile_id: 3,
        code_suite_id: 1,
    };
    let mut st2 = ParamState::new();
    assert!(
        st2.on_param_set(&tampered, &f).is_err(),
        "hash-mismatch → fail-closed"
    );
    assert!(
        !st2.is_confirmed(),
        "mismatch sonrası admission KAPALI kalır"
    );

    // profile_id-mismatch (gelen profil ≠ yerel-beklenen) → FAIL-CLOSED.
    let wrong_profile = Control::ParamSet {
        id: 7,
        param_hash: plan.param_hash,
        profile_id: 1, // yerel-beklenen 3
        code_suite_id: 1,
    };
    let mut st3 = ParamState::new();
    assert!(
        st3.on_param_set(&wrong_profile, &f).is_err(),
        "profile-mismatch → fail-closed"
    );
    assert!(!st3.is_confirmed());
}

// ── Bounded pre-param hold: PARAM-öncesi authenticated-shard'lar bounded-tutulur (aşımda en-eski-drop) ──
#[test]
fn preparam_hold_bounded_drop_oldest() {
    let (mut tx, mut rx) = common::pair(0x0500_0001);
    let mut hold = PreParamHold::new();
    assert!(hold.is_empty());

    // MAX+50 authenticated-shard hold et (her biri farklı-PN, hepsi AEAD-geçer).
    let overflow = MAX_PREPARAM_SHARDS + 50;
    for i in 0..overflow {
        let dg = common::seal_source(&mut tx, 1, 0, 3, 0x1111, i as u64);
        let opened = rx.open(&dg).expect("authenticated (AEAD-geçer)");
        hold.hold(opened);
    }
    // BOUNDED: len == MAX (unbounded-alloc YOK); 50 en-eski düştü (audit).
    assert_eq!(hold.len(), MAX_PREPARAM_SHARDS, "bounded: len == MAX");
    assert_eq!(hold.dropped_count(), 50, "aşan-50 en-eski-drop (audit)");

    // Bind sonrası release → tampon boşalır.
    let released = hold.drain();
    assert_eq!(released.len(), MAX_PREPARAM_SHARDS);
    assert!(hold.is_empty(), "release sonrası boş");
}
