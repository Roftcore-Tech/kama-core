//! DR-CS-3 — **AUTHENTICATED TRANSPORT INTEGRATION WITH TEST-INJECTED SESSION KEYS**
//!
//! This exercises the real shipping bootstrap RTT path end to end:
//! real `Sender` (default build) → real `TransmitKind::CsProbe` → real `Control::CsProbe` codec →
//! real AEAD seal → real `ReceiverHost::on_datagram` (verify-before-admission) → typed pending echo
//! action → real `ReceiverHost::emit_cs_echo` (`Control::CsEcho`, sealed) → real decode → real
//! `Event::CsProbeEcho` ingest → real `RttEstimator` → real PreConverge exit.
//!
//! **SCOPE LIMIT (binding, do not overstate):** the directional session keys are TEST-INJECTED.
//! A real Noise handshake does NOT exist in this repository (`crates/crypto` is a skeleton), so this
//! test does **not** claim "full product end-to-end", "Noise handshake PASS", or
//! "key establishment PASS". Key establishment remains Sprint-6 debt. Everything *after* key
//! installation — authentication, session binding, codec, seal/open, budgets — is the real product path.
//!
//! **SECOND SCOPE LIMIT (recorded, not worked around):** `kama_integration::receiver_host` is itself
//! `#[cfg(feature = "experimental-cold-start")]` (`lib.rs:14-18`), so the receiver composition root is
//! NOT in the shipped default build today. The SENDER emitter added by DR-CS-3 *is* in the default
//! build. This file is therefore gated exactly like every other existing ReceiverHost test. Promoting
//! the responder to the shipped default requires un-gating `receiver_host`, which is a separate
//! decision and is NOT taken here.
#![cfg(feature = "experimental-cold-start")]

use adaptive_transport::sender::{
    BootstrapRttConfig, Event, OpaqueUnit, Sender, SenderState, StartupPacingConfig, Timer,
    TransmitKind,
};
use kama_integration::{IngestOutcome, ReceiverHost};
use kama_noise_wire::control::decode_control;
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::UnsealedTransportPacket;
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0000_00C3;
const MS: u64 = 1_000_000;
const UNIT: u64 = 101;

fn key() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0x5Au8; 32])
}

/// The sender's wire protector (test-injected keys; mirrors `install_session_with_keys`).
fn sender_protector() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H_EXT, &key(), &key())
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
    // DR-RATE-RAMP-2B: startup pacing is a SEPARATE axis and is now default-OFF; the lineage
    // below asserts the handoff INTO it, so enable it explicitly.
    s.set_startup_pacing_init(StartupPacingConfig { enabled: true });
    s
}

/// Feed a sustained backlog so the sender is genuinely PreConverge/token-starved, and drive the
/// tick. Returns every probe the product sender emitted, as `(probe_seq, emit_ns)`.
fn drive(s: &mut Sender, until_s: u64) -> Vec<(u16, u64)> {
    let mut probes = Vec::new();
    for i in 1..=384u64 {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: UNIT,
            }),
            0,
        );
    }
    for t in 1..=until_s {
        let now = t * 1_000 * MS;
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            if d.kind == TransmitKind::CsProbe {
                probes.push((d.seq as u16, now));
            }
        }
    }
    probes
}

/// §7 scenarios 1-9: dropped first DATA → probe → authenticated echo → RTT → srtt>0 → PreConverge exit.
#[test]
fn authenticated_bootstrap_rtt_unlocks_preconverge() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut wire = sender_protector();
    let mut s = bootstrap_sender();

    // (1) every DATA emission is forward-dropped: nothing is handed to the receiver, so no ACK and
    //     therefore no RTT can come from the DATA path.
    let probes = drive(&mut s, 5);
    assert!(!probes.is_empty(), "the ladder must emit a probe by t=5s");
    assert_eq!(s.state(), SenderState::PreConverge, "still cold");
    assert!(
        s.path_timing_snapshot().srtt_ns.is_none(),
        "srtt is uninitialised"
    );
    let (probe_seq, emit_ns) = probes[0];

    // (2) the probe reaches the receiver on the REAL authenticated path
    let dg = seal_probe(&mut wire, probe_seq, 0);
    assert_eq!(
        host.on_datagram(0, &dg).unwrap(),
        IngestOutcome::ProbeEchoed,
        "authenticated probe must be recognised"
    );

    // (3) the composition root produced a typed pending echo ACTION
    let pending = host
        .take_pending_cs_echo(H_EXT)
        .expect("authenticated 0x19 must produce a typed echo action");
    assert_eq!(
        pending, probe_seq,
        "the action carries the request correlation"
    );

    // (4) the echo is emitted through the shipping outbound control path (sealed wire datagram)
    let echo_dg = host
        .emit_cs_echo(H_EXT, pending, 1)
        .expect("echo must seal");
    assert!(
        echo_dg.len() <= dg.len(),
        "amplification: echo {} B must not exceed probe {} B",
        echo_dg.len(),
        dg.len()
    );

    // (5) the sender decodes it and ingests the echo; RTT comes from LOCAL timestamps only
    let opened = wire.open(&echo_dg).expect("sender opens the sealed echo");
    let ctrl = decode_control(opened.inner())
        .expect("control body present")
        .expect("decodes");
    let Control::CsEcho {
        probe_seq: echoed,
        seen_count,
        arrival_gap,
        ..
    } = ctrl
    else {
        panic!("expected CsEcho");
    };
    assert_eq!(echoed, probe_seq, "correlation returned unchanged");
    assert_eq!(
        (seen_count, arrival_gap),
        (0, 0),
        "peer metadata is ledger-only"
    );

    let arrival_ns = emit_ns + 1_000 * MS; // one RTT later, local clock
    s.on_event(Event::CsProbeEcho { probe_seq: echoed }, arrival_ns);

    // (6) srtt goes 0 → positive, from (local arrival − local send)
    let d = s.bootstrap_rtt_diag();
    assert_eq!(d.echo_match_valid, 1, "exactly one valid match");
    assert_eq!(
        d.rtt_sample_ns,
        1_000 * MS,
        "RTT is the local timestamp delta"
    );
    assert!(
        s.path_timing_snapshot().srtt_ns.is_some(),
        "a valid echo must establish srtt"
    );

    // (7) the sender leaves PreConverge on the normal path, and (8) startup pacing arms
    let mut left = false;
    for t in 7..=12u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 1_000 * MS);
        if s.state() != SenderState::PreConverge {
            left = true;
            break;
        }
    }
    assert!(left, "PreConverge must be left once srtt is established");
    assert!(
        s.startup_pacing_floor_bps() > 0 || s.diag_rate().3 > 31,
        "FAZ-1A startup pacing must take over after the exit"
    );

    // (9) the ladder is cancelled: no further probes
    assert_eq!(d.cancelled, 1);
    let before = s.bootstrap_rtt_diag().probe_emitted;
    for t in 13..=60u64 {
        for x in s.on_event(Event::TimerFired(Timer::Pto), t * 1_000 * MS) {
            assert_ne!(x.kind, TransmitKind::CsProbe, "no probe after success");
        }
    }
    assert_eq!(s.bootstrap_rtt_diag().probe_emitted, before);
}

/// A lost echo must let the NEXT ladder step fire.
#[test]
fn lost_echo_advances_to_the_next_ladder_step() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 12); // steps at 3 s and 9 s are due
    assert!(
        probes.len() >= 2,
        "a lost echo must not stop the ladder (got {} probes)",
        probes.len()
    );
    assert_ne!(
        probes[0].0, probes[1].0,
        "each step uses a fresh generation"
    );
    assert!(s.path_timing_snapshot().srtt_ns.is_none(), "still no srtt");
}

/// All four probes lost: the budget holds and no fifth probe is ever emitted.
#[test]
fn all_probes_lost_keeps_budget_and_gives_up() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 120); // far beyond the last step (45 s)
    assert!(
        probes.len() <= 4,
        "at most four ladder steps, got {}",
        probes.len()
    );
    let d = s.bootstrap_rtt_diag();
    assert!(d.probe_wire_bytes <= 200, "sender budget ≤ 200 B");
    assert_eq!(d.budget_violation, 0);
    assert!(
        s.path_timing_snapshot().srtt_ns.is_none(),
        "no RTT without an echo"
    );
}

// ── §6 receiver-side security / amplification / budget ───────────────────────────────────────

#[test]
fn receiver_rejects_unauthenticated_and_unknown_session() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut wire = sender_protector();

    // unknown session id
    let dg = seal_probe(&mut wire, 1, 0);
    let other = 0x0000_0099u32;
    assert!(
        host.take_pending_cs_echo(other).is_none(),
        "unknown session must never hold a pending echo"
    );
    assert!(
        host.emit_cs_echo(other, 1, 0).is_err(),
        "unknown session must not emit an echo"
    );

    // corrupted ciphertext ⇒ AEAD verify fails ⇒ dropped, no action
    let mut bad = dg.clone();
    let n = bad.len();
    bad[n - 1] ^= 0xFF;
    assert_eq!(
        host.on_datagram(0, &bad).unwrap(),
        IngestOutcome::Dropped("aead verify failed")
    );
    assert!(
        host.take_pending_cs_echo(H_EXT).is_none(),
        "unauthenticated probe must produce NO echo action"
    );

    // malformed/truncated datagram
    assert!(host.on_datagram(0, &[0u8; 4]).is_err() || host.take_pending_cs_echo(H_EXT).is_none());
}

#[test]
fn receiver_echo_is_bounded_by_generation_and_budget() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut wire = sender_protector();

    // four unique generations → four echoes
    let mut total = 0usize;
    for g in 0..4u16 {
        let dg = seal_probe(&mut wire, g, u64::from(g));
        host.on_datagram(0, &dg).unwrap();
        let p = host.take_pending_cs_echo(H_EXT).expect("fresh generation");
        let e = host
            .emit_cs_echo(H_EXT, p, u64::from(g) + 100)
            .expect("echo");
        assert!(e.len() <= dg.len(), "echo must not exceed the probe");
        total += e.len();
    }
    let (count, bytes, budget) = host.cs_echo_budget(H_EXT).unwrap();
    assert_eq!(count, 4, "four unique echoes");
    assert!(bytes <= budget, "receiver budget {bytes} > {budget}");
    assert_eq!(budget, 172, "4 x ECHO_WIRE_MAX(43)");
    assert!(
        total <= 4 * 44,
        "aggregate echo wire stays under 4 x probe size"
    );

    // fifth generation → refused
    let dg = seal_probe(&mut wire, 4, 200);
    host.on_datagram(0, &dg).unwrap();
    assert!(
        host.take_pending_cs_echo(H_EXT).is_none(),
        "fifth generation must not produce an action"
    );
    assert!(
        host.emit_cs_echo(H_EXT, 4, 201).is_err(),
        "fifth generation must be refused at the emitter too"
    );

    // duplicate/replay of an already-echoed generation → refused
    let dup = seal_probe(&mut wire, 0, 202);
    host.on_datagram(0, &dup).unwrap();
    assert!(
        host.take_pending_cs_echo(H_EXT).is_none(),
        "replayed generation must not produce a second action"
    );
    assert!(
        host.emit_cs_echo(H_EXT, 0, 203).is_err(),
        "duplicate generation refused"
    );
    let (count2, bytes2, _) = host.cs_echo_budget(H_EXT).unwrap();
    assert_eq!((count2, bytes2), (count, bytes), "refusals change nothing");
}

#[test]
fn receiver_recognition_semantics_are_unchanged() {
    // The pre-existing contract (probe/echo never enters the dedup window; cs_echo_count counts
    // every probe) must survive: only the RESPONSE is newly bounded.
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut wire = sender_protector();
    for g in 0..50u16 {
        let dg = seal_probe(&mut wire, g, u64::from(g));
        assert_eq!(
            host.on_datagram(0, &dg).unwrap(),
            IngestOutcome::ProbeEchoed
        );
    }
    assert_eq!(
        host.cs_echo_count(),
        50,
        "recognition counting is unchanged"
    );
    let (count, _, _) = host.cs_echo_budget(H_EXT).unwrap();
    assert!(count <= 4, "but at most four responses were ever prepared");
}
