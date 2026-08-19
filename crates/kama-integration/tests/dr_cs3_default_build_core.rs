//! DR-CS-3 — **DEFAULT-BUILD AUTHENTICATED TRANSPORT COMPONENT INTEGRATION
//! WITH TEST-INJECTED SESSION KEYS**
//!
//! Runs under **default features** (`experimental-cold-start` OFF). It exercises the real chain:
//! `Sender` → sealed `CsProbe` → `ReceiverControlCore` session lookup → AEAD open → control decode →
//! bounded responder → typed `CsEcho` → session seal → `Sender` echo ingest → matched local RTT
//! sample → `srtt>0` → PreConverge exit → startup pacing.
//!
//! **SCOPE (binding):** this validates the authenticated transport **component** AFTER key
//! establishment. Directional session keys are TEST-INJECTED; there is no Noise handshake in this
//! repository. It does NOT claim: Noise handshake verified · product binary verified · deployment
//! network path verified. There is also no product runtime consumer yet
//! (`PRODUCT-RUNTIME-INTEGRATION-PENDING`).

use adaptive_transport::sender::{
    BootstrapRttConfig, Event, OpaqueUnit, Sender, SenderState, StartupPacingConfig, Timer,
    TransmitKind,
};
use kama_integration::{CoreIngest, ReceiverControlCore};
use kama_noise_wire::control::decode_control;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::UnsealedTransportPacket;
use zeroize::Zeroizing;

const H: u32 = 0x0000_0C33;
const MS: u64 = 1_000_000;

fn key() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0x7Bu8; 32])
}
fn peer() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H, &key(), &key())
}
fn core() -> ReceiverControlCore {
    let mut c = ReceiverControlCore::new();
    c.install_session_with_directional_keys(H, &key(), &key())
        .unwrap();
    c
}
fn seal_probe(p: &mut KamaNoiseWireProtector, probe_seq: u16, pn: u64) -> Vec<u8> {
    p.seal(
        &UnsealedTransportPacket::Control(Control::CsProbe {
            id: u32::from(probe_seq) + 1,
            subtype: 0x01,
            probe_seq,
        }),
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}
fn bootstrap_sender() -> Sender {
    let mut s = Sender::new();
    s.set_bootstrap_rtt(BootstrapRttConfig { enabled: true });
    // DR-RATE-RAMP-2B: startup pacing is a SEPARATE axis and is now default-OFF. The bootstrap
    // lineage below asserts the handoff INTO it, so enable it explicitly rather than depending on
    // whatever the product default happens to be.
    s.set_startup_pacing_init(StartupPacingConfig { enabled: true });
    s
}
/// Sustained backlog + tick; returns `(probe_seq, emit_ns)` for every product probe.
fn drive(s: &mut Sender, until_s: u64) -> Vec<(u16, u64)> {
    let mut v = Vec::new();
    for i in 1..=384u64 {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=until_s {
        let now = t * 1_000 * MS;
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            if d.kind == TransmitKind::CsProbe {
                v.push((d.seq as u16, now));
            }
        }
    }
    v
}

/// §9 scenario 1 + the full lineage, entirely in the DEFAULT build.
#[test]
fn default_build_bootstrap_rtt_unlocks_preconverge() {
    let mut c = core();
    let mut wire = peer();
    let mut s = bootstrap_sender();

    let probes = drive(&mut s, 5);
    assert!(!probes.is_empty(), "ladder must fire by t=5s");
    assert_eq!(s.state(), SenderState::PreConverge);
    assert!(s.path_timing_snapshot().srtt_ns.is_none());
    let (probe_seq, emit_ns) = probes[0];

    let dg = seal_probe(&mut wire, probe_seq, 0);
    let CoreIngest::BootstrapProbe {
        probe_seq: got,
        control_id,
    } = c.ingest(H, &dg).unwrap()
    else {
        panic!("authenticated probe must be recognised by the default-build core");
    };
    assert_eq!(got, probe_seq);

    let pending = c.take_pending_cs_echo(H).expect("typed echo action");
    let echo = c
        .emit_cs_echo(H, pending, control_id, 1)
        .expect("sealed echo");
    assert!(echo.len() <= dg.len(), "amplification ≤ 1");

    let opened = wire.open(&echo).expect("peer opens the sealed echo");
    let Some(Ok(Control::CsEcho {
        probe_seq: echoed,
        seen_count,
        arrival_gap,
        ..
    })) = decode_control(opened.inner())
    else {
        panic!("expected CsEcho");
    };
    assert_eq!(echoed, probe_seq);
    assert_eq!(
        (seen_count, arrival_gap),
        (0, 0),
        "peer metadata ledger-only"
    );

    s.on_event(
        Event::CsProbeEcho { probe_seq: echoed },
        emit_ns + 1_000 * MS,
    );
    let d = s.bootstrap_rtt_diag();
    assert_eq!(d.echo_match_valid, 1);
    assert_eq!(
        d.rtt_sample_ns,
        1_000 * MS,
        "RTT from local timestamps only"
    );
    assert!(
        s.path_timing_snapshot().srtt_ns.is_some(),
        "srtt established"
    );

    let mut left = false;
    for t in 7..=12u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 1_000 * MS);
        if s.state() != SenderState::PreConverge {
            left = true;
            break;
        }
    }
    assert!(left, "PreConverge exit");
    assert!(
        s.startup_pacing_floor_bps() > 0 || s.diag_rate().3 > 31,
        "startup pacing handoff"
    );
    assert_eq!(d.cancelled, 1, "ladder cancelled after success");
}

/// §9 scenarios 2/3: a lost probe or a lost echo lets the next step fire.
#[test]
fn default_build_lost_probe_or_echo_advances_ladder() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 12);
    assert!(probes.len() >= 2, "next ladder step must fire");
    assert_ne!(probes[0].0, probes[1].0, "fresh generation per step");
    assert!(s.path_timing_snapshot().srtt_ns.is_none());
}

/// §9 scenario 4: all four lost ⇒ sender budget holds, no fifth probe.
#[test]
fn default_build_all_probes_lost_budget_holds() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 120);
    assert!(probes.len() <= 4);
    let d = s.bootstrap_rtt_diag();
    assert!(d.probe_wire_bytes <= 200, "sender budget ≤200 B");
    assert_eq!(d.budget_violation, 0);
}

/// §9 scenarios 6-9: replay, unauthenticated, unknown session, malformed ⇒ no echo.
#[test]
fn default_build_security_boundary() {
    let mut c = core();
    let mut wire = peer();

    // unknown session
    assert!(matches!(
        c.ingest(0x0BAD, &[0u8; 64]),
        Err(kama_integration::CoreError::UnknownSession { .. })
    ));
    assert!(c.emit_cs_echo(0x0BAD, 1, 1, 0).is_err());

    // unauthenticated (corrupted ciphertext) ⇒ rejected, no action
    let dg = seal_probe(&mut wire, 1, 0);
    let mut bad = dg.clone();
    let n = bad.len();
    bad[n - 1] ^= 0xFF;
    assert_eq!(
        c.ingest(H, &bad).unwrap(),
        CoreIngest::Rejected("aead verify failed")
    );
    assert!(
        c.take_pending_cs_echo(H).is_none(),
        "no echo for unauthenticated"
    );

    // malformed/truncated
    assert!(matches!(
        c.ingest(H, &[0u8; 8]),
        Ok(CoreIngest::Rejected(_)) | Err(_)
    ));
    assert!(c.take_pending_cs_echo(H).is_none());

    // authenticated probe then REPLAY of the same generation ⇒ exactly one echo
    let CoreIngest::BootstrapProbe { control_id, .. } = c.ingest(H, &dg).unwrap() else {
        panic!("probe")
    };
    let p = c.take_pending_cs_echo(H).unwrap();
    c.emit_cs_echo(H, p, control_id, 1).unwrap();
    let replay = seal_probe(&mut wire, 1, 5);
    c.ingest(H, &replay).unwrap();
    assert!(
        c.take_pending_cs_echo(H).is_none(),
        "replay ⇒ no extra action"
    );
    assert!(
        c.emit_cs_echo(H, 1, control_id, 2).is_err(),
        "replay ⇒ no extra echo"
    );
}

/// §9 scenario 5 + receiver budget: four unique generations, fifth refused, ≤172 B.
#[test]
fn default_build_receiver_budget_and_generation_cap() {
    let mut c = core();
    let mut wire = peer();
    for g in 0..4u16 {
        let dg = seal_probe(&mut wire, g, u64::from(g));
        let CoreIngest::BootstrapProbe { control_id, .. } = c.ingest(H, &dg).unwrap() else {
            panic!("probe")
        };
        let p = c.take_pending_cs_echo(H).unwrap();
        let e = c
            .emit_cs_echo(H, p, control_id, u64::from(g) + 100)
            .unwrap();
        assert!(e.len() <= dg.len(), "echo ≤ probe");
    }
    let (count, bytes, budget) = c.echo_budget(H).unwrap();
    assert_eq!((count, budget), (4, 172));
    assert!(bytes <= budget);

    let dg = seal_probe(&mut wire, 9, 200);
    c.ingest(H, &dg).unwrap();
    assert!(
        c.take_pending_cs_echo(H).is_none(),
        "fifth generation refused"
    );
    assert!(c.emit_cs_echo(H, 9, 1, 201).is_err());
}

/// §9 scenario 12: with the runtime config OFF there is no bootstrap wire behaviour at all.
/// Scoped to the SHIPPING build: under `experimental-cold-start` the LEGACY ladder is a separate
/// (superseded) implementation that still fires when DR-CS-3 is OFF, so raw probe absence is not a
/// valid assertion there. The DR-CS-3 counters are asserted unconditionally.
#[cfg(not(feature = "experimental-cold-start"))]
#[test]
fn default_build_runtime_off_emits_nothing() {
    let mut s = Sender::new(); // BootstrapRttConfig default OFF
    let probes = drive(&mut s, 60);
    assert!(probes.is_empty(), "OFF ⇒ no probe on the wire");
    assert_eq!(s.bootstrap_rtt_diag().arm, 0);
}

/// §9 scenario 11: session close resets the bounded responder state (contract §26).
#[test]
fn default_build_close_resets_responder_state() {
    let mut c = core();
    let mut wire = peer();
    let dg = seal_probe(&mut wire, 3, 0);
    c.ingest(H, &dg).unwrap();
    assert!(c.take_pending_cs_echo(H).is_some());
    c.close_session(H);
    assert_eq!(c.session_count(), 0);
    assert!(
        c.take_pending_cs_echo(H).is_none(),
        "state cleared on close"
    );
    assert!(c.echo_budget(H).is_none());
}
