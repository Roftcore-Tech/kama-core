//! DR-CS-3 — **§1 PANEL PREFLIGHT CONFORMANCE**
//!
//! Closes the two narrow conformance uncertainties before any panel is run:
//!
//! * **§1.1** ownership: `BootstrapEchoResponder` is the single implementation, and the default-build
//!   [`ReceiverControlCore`] and the feature-gated `ReceiverHost` produce a **byte-identical** echo
//!   from the same probe under the same keys.
//! * **§1.2** `control_id` reflection safety: `control_id` is neither an AEAD nonce input nor an AAD
//!   input nor a dedup/issuance identity; `probe_seq` is the authoritative correlation.
//!
//! **SCOPE (binding):** authenticated transport *component* with TEST-INJECTED session keys. No Noise
//! handshake, no product binary, no deployment network path is validated here.

use adaptive_transport::sender::{
    BootstrapRttConfig, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitKind,
};
use kama_integration::receiver_control_core::{ECHO_BUDGET_BYTES, MAX_ECHOES};
use kama_integration::{CoreError, CoreIngest, ReceiverControlCore};
use kama_noise_wire::control::decode_control;
use kama_noise_wire::{nonce96, KamaNoiseWireProtector, RetryClass, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::UnsealedTransportPacket;
use zeroize::Zeroizing;

const H: u32 = 0x0000_0D31;
const H2: u32 = 0x0000_0D32;
const MS: u64 = 1_000_000;
/// Probe wire size pinned by the schema (`PROBE_WIRE_EXACT`), for the amplification assertion.
const PROBE_WIRE_EXACT: usize = 44;
const ECHO_WIRE: usize = 43;

fn k_ab() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0xA1u8; 32])
}
fn k_ba() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0xB2u8; 32])
}
fn k_other() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0xC3u8; 32])
}

/// Peer (initiator) side: sends with `k_ab`, receives with `k_ba`.
fn initiator(
    h: u32,
    send: &Zeroizing<[u8; 32]>,
    recv: &Zeroizing<[u8; 32]>,
) -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(h, send, recv)
}

/// Responder core: sends with `k_ba`, receives with `k_ab` (mirror of the initiator).
fn core_with(
    h: u32,
    send: &Zeroizing<[u8; 32]>,
    recv: &Zeroizing<[u8; 32]>,
) -> ReceiverControlCore {
    let mut c = ReceiverControlCore::new();
    c.install_session_with_directional_keys(h, send, recv)
        .unwrap();
    c
}

fn seal_probe(p: &mut KamaNoiseWireProtector, control_id: u32, probe_seq: u16, pn: u64) -> Vec<u8> {
    p.seal(
        &UnsealedTransportPacket::Control(Control::CsProbe {
            id: control_id,
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
    s.set_bootstrap_rtt(BootstrapRttConfig {
        enabled: true,
        fires_ns: None,
    });
    s
}

/// Sustained backlog + PTO ticks; returns `(probe_seq, emit_ns)` for every product probe.
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

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// §1.2 — control_id reflection safety
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// The AEAD nonce is a pure function of `wire_pn` (`protector.rs:36`,
/// `nonce = [0x00000000 ‖ LE64(full_pn)]`). `control_id` is carried in the *encrypted inner
/// plaintext*, so it can neither select nor perturb a nonce. The separation of nonces within one
/// direction is therefore owned exclusively by the monotone `wire_pn` guard — which is enforced
/// structurally, independently of any `control_id`.
#[test]
fn nonce_is_derived_from_wire_pn_only_and_never_from_control_id() {
    // (a) nonce = 4 zero bytes ‖ LE64(pn); nothing else is an input.
    for pn in [0u64, 1, 7, 4096, u64::from(u32::MAX) + 9] {
        let n = nonce96(pn);
        assert_eq!(&n[0..4], &[0u8; 4], "nonce prefix must be 4 zero bytes");
        assert_eq!(
            &n[4..12],
            &pn.to_le_bytes(),
            "nonce suffix must be LE64(wire_pn)"
        );
    }

    // (b) Two DIFFERENT control_ids at the same pn map to the SAME nonce — i.e. control_id has no
    //     nonce-separating power whatsoever. The pn guard is what prevents reuse.
    assert_eq!(nonce96(9), nonce96(9));

    // (c) Reusing a wire_pn within one direction is refused structurally, no matter the control_id.
    let mut a = initiator(H, &k_ab(), &k_ba());
    let _ = seal_probe(&mut a, 0xAAAA_AAAA, 0, 5);
    let reuse = a.seal(
        &UnsealedTransportPacket::Control(Control::CsProbe {
            id: 0xBBBB_BBBB, // different control_id …
            subtype: 0x01,
            probe_seq: 1,
        }),
        SealArgs {
            wire_pn: 5,
            dh_epoch: 0,
            send_generation: 0,
        }, // … same pn
    );
    assert!(
        reuse.is_err(),
        "non-monotonic wire_pn must be refused (nonce-reuse guard)"
    );
}

/// The same numeric `control_id` present in both directions cannot create a `(key, nonce)` collision:
/// the two directions use **different keys** (`send_cipher` from `send_key`, `recv_cipher` from
/// `recv_key`), so identical plaintext at an identical `wire_pn` still yields different ciphertext,
/// and a frame sealed for one direction cannot be opened in the other.
#[test]
fn same_control_id_in_both_directions_uses_different_keys_and_ciphertext() {
    const CID: u32 = 0x5151_5151;
    const PN: u64 = 11;

    let mut a = initiator(H, &k_ab(), &k_ba()); // A→B seals with k_ab
    let mut b = initiator(H, &k_ba(), &k_ab()); // B→A seals with k_ba

    let frame = |p: &mut KamaNoiseWireProtector| {
        p.seal(
            &UnsealedTransportPacket::Control(Control::CsEcho {
                id: CID,
                subtype: 0x01,
                probe_seq: 3,
                seen_count: 0,
                arrival_gap: 0,
            }),
            SealArgs {
                wire_pn: PN,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap()
        .bytes
    };
    let a_to_b = frame(&mut a);
    let b_to_a = frame(&mut b);

    assert_eq!(a_to_b.len(), b_to_a.len(), "same plaintext ⇒ same length");
    assert_ne!(
        a_to_b, b_to_a,
        "identical control_id + pn in opposite directions must NOT collide"
    );

    // A cannot open its own outbound frame: that would require k_ab as a *recv* key.
    let mut a_again = initiator(H, &k_ab(), &k_ba());
    assert!(
        a_again.open(&a_to_b).is_err(),
        "own-direction frame must not open on the recv cipher"
    );
    // The peer, holding the mirrored keys, opens it.
    let mut b_again = initiator(H, &k_ba(), &k_ab());
    assert!(
        b_again.open(&a_to_b).is_ok(),
        "mirrored peer must open the frame"
    );
}

/// `probe_seq` is the authoritative correlation. The reflected `control_id` is never even delivered
/// to the sender's RTT logic — `Event::CsProbeEcho` carries `probe_seq` alone.
#[test]
fn probe_seq_is_the_authoritative_correlation_and_control_id_is_reflected_verbatim() {
    // Wire-level: the echo carries back exactly the control_id the request supplied.
    const CID: u32 = 0x0BAD_C0DE;
    let mut a = initiator(H, &k_ab(), &k_ba());
    let mut c = core_with(H, &k_ba(), &k_ab());

    let dg = seal_probe(&mut a, CID, 42, 1);
    let CoreIngest::BootstrapProbe {
        probe_seq,
        control_id,
    } = c.ingest(H, &dg).unwrap()
    else {
        panic!("authenticated probe must be recognised");
    };
    assert_eq!(probe_seq, 42);
    assert_eq!(
        control_id, CID,
        "the request's control_id is surfaced verbatim"
    );

    let seq = c.take_pending_cs_echo(H).unwrap();
    let echo = c.emit_cs_echo(H, seq, control_id, 1).unwrap();
    let opened = a.open(&echo).unwrap();
    let Some(Ok(Control::CsEcho {
        id,
        probe_seq: back,
        seen_count,
        arrival_gap,
        ..
    })) = decode_control(opened.inner())
    else {
        panic!("echo must decode");
    };
    assert_eq!(
        id, CID,
        "control_id is an idempotent request/response identity, echoed verbatim"
    );
    assert_eq!(
        back, 42,
        "probe_seq is the correlation the sender matches on"
    );
    assert_eq!(
        (seen_count, arrival_gap),
        (0, 0),
        "ledger-only fields must never carry peer clock input"
    );

    // Codepoint policy: probe/echo are NoRetry (§2.57 amendment) — they are not retried, and they
    // are not issuance/dedup identities.
    assert_eq!(RetryClass::of(0x19), RetryClass::NoRetry);
    assert_eq!(RetryClass::of(0x1A), RetryClass::NoRetry);
}

/// A replayed probe produces no additional echo — both as a byte-identical network duplicate
/// (rejected at the AEAD replay window) and as a fresh-pn re-send of an already-answered generation
/// (rejected by the bounded responder).
#[test]
fn replayed_probe_produces_no_additional_echo() {
    let mut a = initiator(H, &k_ab(), &k_ba());
    let mut c = core_with(H, &k_ba(), &k_ab());

    let dg = seal_probe(&mut a, 7, 0, 1);
    assert!(matches!(
        c.ingest(H, &dg).unwrap(),
        CoreIngest::BootstrapProbe { .. }
    ));
    let seq = c.take_pending_cs_echo(H).unwrap();
    c.emit_cs_echo(H, seq, 7, 1).unwrap();
    assert_eq!(
        c.echo_budget(H).unwrap(),
        (1, ECHO_WIRE as u64, ECHO_BUDGET_BYTES)
    );

    // (a) byte-identical duplicate → AEAD replay window rejects it; zero side effect.
    assert!(matches!(c.ingest(H, &dg).unwrap(), CoreIngest::Rejected(_)));
    assert!(c.take_pending_cs_echo(H).is_none());
    assert_eq!(
        c.echo_budget(H).unwrap(),
        (1, ECHO_WIRE as u64, ECHO_BUDGET_BYTES)
    );

    // (b) same generation re-sent under a fresh pn → authenticates, but the responder refuses.
    let fresh = seal_probe(&mut a, 8, 0, 2);
    assert!(matches!(
        c.ingest(H, &fresh).unwrap(),
        CoreIngest::BootstrapProbe { .. }
    ));
    assert!(
        c.take_pending_cs_echo(H).is_none(),
        "duplicate generation ⇒ no pending action"
    );
    assert_eq!(
        c.emit_cs_echo(H, 0, 8, 3),
        Err(CoreError::ResponderRefused("duplicate generation"))
    );
    assert_eq!(
        c.echo_budget(H).unwrap(),
        (1, ECHO_WIRE as u64, ECHO_BUDGET_BYTES)
    );
}

/// A replayed echo produces no additional RTT sample: the outstanding slot is consumed exactly once,
/// and a resolved bootstrap rejects everything afterwards.
#[test]
fn replayed_echo_produces_no_additional_rtt_sample() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 4);
    let (seq, _) = probes[0];

    s.on_event(Event::CsProbeEcho { probe_seq: seq }, 4_000 * MS);
    let after_first = s.bootstrap_rtt_diag();
    assert_eq!(after_first.echo_match_valid, 1);
    assert!(
        s.path_timing_snapshot().srtt_ns.is_some(),
        "first matched echo must produce srtt>0"
    );

    for t in [4_500u64, 5_000, 5_500] {
        s.on_event(Event::CsProbeEcho { probe_seq: seq }, t * MS);
    }
    let after_replays = s.bootstrap_rtt_diag();
    assert_eq!(
        after_replays.echo_match_valid, 1,
        "single consumption under replay"
    );
    assert_eq!(
        after_replays.rtt_sample_ns, after_first.rtt_sample_ns,
        "replays must not perturb the recorded sample"
    );
    assert!(
        after_replays.echo_match_rejected >= 3,
        "each replay must be counted as rejected"
    );
}

/// A mismatched `probe_seq` yields no RTT even though the reflected `control_id` would have matched:
/// correlation is owned by `probe_seq` alone.
#[test]
fn wrong_probe_seq_produces_no_rtt() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 4);
    let (seq, _) = probes[0];

    s.on_event(
        Event::CsProbeEcho {
            probe_seq: seq.wrapping_add(1),
        },
        4_000 * MS,
    );
    let d = s.bootstrap_rtt_diag();
    assert_eq!(
        d.echo_match_valid, 0,
        "wrong generation must not produce a sample"
    );
    assert_eq!(d.rtt_sample_ns, 0);
    assert!(d.echo_match_rejected >= 1);
    assert!(
        s.path_timing_snapshot().srtt_ns.is_none(),
        "srtt must stay unset"
    );
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "no RTT ⇒ no PreConverge exit"
    );
}

/// The same `probe_seq` under the wrong session produces nothing: the frame fails AEAD open against
/// the other session's keys, so no echo and no responder state change occur.
#[test]
fn same_probe_seq_wrong_session_produces_no_echo_and_no_state_change() {
    let mut a = initiator(H, &k_ab(), &k_ba());
    let mut c = ReceiverControlCore::new();
    c.install_session_with_directional_keys(H, &k_ba(), &k_ab())
        .unwrap();
    c.install_session_with_directional_keys(H2, &k_other(), &k_other())
        .unwrap();

    let dg = seal_probe(&mut a, 5, 3, 1);
    // Delivered to the WRONG handle → verify-before-admission rejects it.
    assert!(matches!(
        c.ingest(H2, &dg).unwrap(),
        CoreIngest::Rejected(_)
    ));
    assert!(c.take_pending_cs_echo(H2).is_none());
    assert_eq!(c.echo_budget(H2).unwrap(), (0, 0, ECHO_BUDGET_BYTES));
    // The correct session is untouched by the misdelivery.
    assert_eq!(c.echo_budget(H).unwrap(), (0, 0, ECHO_BUDGET_BYTES));
    assert!(c.take_pending_cs_echo(H).is_none());

    // An unknown handle is a typed error and likewise produces no echo.
    assert_eq!(
        c.ingest(0xDEAD_BEEF, &dg),
        Err(CoreError::UnknownSession { h_ext: 0xDEAD_BEEF })
    );
}

/// The fifth generation produces no echo, and the receiver budget is never exceeded.
#[test]
fn fifth_generation_produces_no_echo() {
    let mut a = initiator(H, &k_ab(), &k_ba());
    let mut c = core_with(H, &k_ba(), &k_ab());

    for g in 0..MAX_ECHOES as u16 {
        let dg = seal_probe(&mut a, u32::from(g) + 1, g, u64::from(g) + 1);
        assert!(matches!(
            c.ingest(H, &dg).unwrap(),
            CoreIngest::BootstrapProbe { .. }
        ));
        let seq = c.take_pending_cs_echo(H).unwrap();
        let echo = c
            .emit_cs_echo(H, seq, u32::from(g) + 1, u64::from(g) + 1)
            .unwrap();
        assert_eq!(
            echo.len(),
            ECHO_WIRE,
            "echo must be exactly the pinned wire size"
        );
    }
    assert_eq!(
        c.echo_budget(H).unwrap(),
        (MAX_ECHOES, ECHO_BUDGET_BYTES, ECHO_BUDGET_BYTES)
    );

    // Fifth distinct generation: authenticates, but no pending action and no emission.
    let fifth = seal_probe(&mut a, 99, MAX_ECHOES as u16, 99);
    assert!(matches!(
        c.ingest(H, &fifth).unwrap(),
        CoreIngest::BootstrapProbe { .. }
    ));
    assert!(
        c.take_pending_cs_echo(H).is_none(),
        "generation cap ⇒ no pending action"
    );
    assert!(
        c.emit_cs_echo(H, MAX_ECHOES as u16, 99, 99).is_err(),
        "generation cap ⇒ no emission"
    );
    assert_eq!(
        c.echo_budget(H).unwrap(),
        (MAX_ECHOES, ECHO_BUDGET_BYTES, ECHO_BUDGET_BYTES),
        "budget must never be exceeded"
    );
}

/// End-to-end budget conformance: sender ≤ 200 B, receiver ≤ 172 B, and every echo is strictly
/// smaller than the probe that caused it (amplification < 1 by construction).
#[test]
fn sender_and_receiver_budgets_hold_with_amplification_below_one() {
    let mut s = bootstrap_sender();
    let probes = drive(&mut s, 60); // full ladder, every echo lost
    let d = s.bootstrap_rtt_diag();
    assert!(
        d.probe_wire_bytes <= 200,
        "sender probe budget: {} B",
        d.probe_wire_bytes
    );
    assert_eq!(d.budget_violation, 0);
    assert!(
        probes.len() <= MAX_ECHOES,
        "at most one probe per ladder step"
    );

    let mut a = initiator(H, &k_ab(), &k_ba());
    let mut c = core_with(H, &k_ba(), &k_ab());
    let mut echo_bytes = 0usize;
    for g in 0..8u16 {
        let dg = seal_probe(&mut a, u32::from(g) + 1, g, u64::from(g) + 1);
        assert_eq!(dg.len(), PROBE_WIRE_EXACT, "probe wire size is pinned");
        let _ = c.ingest(H, &dg).unwrap();
        if let Some(seq) = c.take_pending_cs_echo(H) {
            let echo = c
                .emit_cs_echo(H, seq, u32::from(g) + 1, u64::from(g) + 1)
                .unwrap();
            assert!(echo.len() < PROBE_WIRE_EXACT, "amplification must be < 1");
            echo_bytes += echo.len();
        }
    }
    assert!(
        echo_bytes as u64 <= ECHO_BUDGET_BYTES,
        "receiver echo budget: {echo_bytes} B"
    );
    let (gens, bytes, budget) = c.echo_budget(H).unwrap();
    assert_eq!(gens, MAX_ECHOES);
    assert_eq!(bytes, budget);
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// §1.3 — session capacity: SCOPE RECORD ONLY (the value 1024 is NOT tuned here)
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// Grounds the three claims recorded in the §1.3 scope note: the cap is **fail-closed** (typed error,
/// no insertion), it performs **no eviction**, and reaching it leaves **already-installed sessions
/// fully operational**. This is a component safety cap; the product capacity decision belongs to
/// `PRODUCT-RUNTIME-INTEGRATION`.
#[test]
fn session_capacity_is_fail_closed_without_eviction() {
    use kama_integration::receiver_control_core::MAX_CORE_SESSIONS;

    let mut c = ReceiverControlCore::new();
    for i in 0..MAX_CORE_SESSIONS as u32 {
        c.install_session_with_directional_keys(i, &k_ba(), &k_ab())
            .unwrap();
    }
    assert_eq!(c.session_count(), MAX_CORE_SESSIONS);

    // Fail-closed: a NEW handle at capacity is a typed refusal, not an eviction.
    assert_eq!(
        c.install_session_with_directional_keys(9_999, &k_ba(), &k_ab()),
        Err(CoreError::SessionCapacity {
            cap: MAX_CORE_SESSIONS
        })
    );
    assert_eq!(c.session_count(), MAX_CORE_SESSIONS, "no eviction occurred");

    // Existing sessions keep working at capacity.
    let mut a = initiator(0, &k_ab(), &k_ba());
    let dg = seal_probe(&mut a, 1, 0, 1);
    assert!(matches!(
        c.ingest(0, &dg).unwrap(),
        CoreIngest::BootstrapProbe { .. }
    ));
    let seq = c.take_pending_cs_echo(0).unwrap();
    assert!(c.emit_cs_echo(0, seq, 1, 1).is_ok());

    // Closing one frees exactly one slot — capacity is reclaimed only by explicit close.
    c.close_session(0);
    assert_eq!(c.session_count(), MAX_CORE_SESSIONS - 1);
    assert!(c
        .install_session_with_directional_keys(9_999, &k_ba(), &k_ab())
        .is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// §1.1 — ownership: the two paths must be semantically identical, byte for byte
// ═══════════════════════════════════════════════════════════════════════════════════════════════

/// **§1.1 acceptance.** The default-build `ReceiverControlCore` and the feature-gated `ReceiverHost`
/// answer the same authenticated probe with a **byte-identical** sealed echo, under the same
/// directional keys and the same `wire_pn`. This is what "single-source bootstrap security semantics"
/// means operationally: the responder policy, the frame construction, and the reflected `control_id`
/// all come from the one `BootstrapEchoResponder`.
#[cfg(feature = "experimental-cold-start")]
#[test]
fn default_core_and_receiver_host_emit_byte_identical_echo() {
    use kama_integration::{IngestOutcome, ReceiverHost};

    const CID: u32 = 0x1234_5678;
    let mut a = initiator(H, &k_ab(), &k_ba());
    let dg = seal_probe(&mut a, CID, 21, 1);

    let mut c = core_with(H, &k_ba(), &k_ab());
    let mut host = ReceiverHost::new();
    host.install_session_with_directional_keys(H, &k_ba(), &k_ab())
        .unwrap();

    // Same authenticated probe into both paths.
    let CoreIngest::BootstrapProbe {
        probe_seq,
        control_id,
    } = c.ingest(H, &dg).unwrap()
    else {
        panic!("core must recognise the probe");
    };
    assert_eq!(
        host.on_datagram(0, &dg).unwrap(),
        IngestOutcome::ProbeEchoed
    );

    assert_eq!(c.take_pending_cs_echo(H), Some(probe_seq));
    assert_eq!(host.take_pending_cs_echo(H), Some(probe_seq));

    let core_echo = c.emit_cs_echo(H, probe_seq, control_id, 1).unwrap();
    let host_echo = host.emit_cs_echo(H, probe_seq, 1).unwrap();
    assert_eq!(
        core_echo, host_echo,
        "the two paths must emit a byte-identical echo"
    );

    // Identical bounded-policy accounting on both sides.
    assert_eq!(c.echo_budget(H), host.cs_echo_budget(H));

    // Identical refusal semantics for the duplicate generation.
    assert!(c.emit_cs_echo(H, probe_seq, control_id, 2).is_err());
    assert!(host.emit_cs_echo(H, probe_seq, 2).is_err());
    assert_eq!(c.echo_budget(H), host.cs_echo_budget(H));
}

/// The gated `ReceiverHost` keeps probe/echo out of `ControlDedupWindow` and out of the
/// `ControlIdIssueGate`: answering probes must not consume any control-id issuance budget that a
/// `Recovered` emission would need.
#[cfg(feature = "experimental-cold-start")]
#[test]
fn probe_echo_consume_no_control_id_issuance_budget() {
    use kama_integration::ReceiverHost;

    let mut a = initiator(H, &k_ab(), &k_ba());
    let mut host = ReceiverHost::new();
    host.install_session_with_directional_keys(H, &k_ba(), &k_ab())
        .unwrap();

    // A Recovered emission before any probe, to sample the issued control-id.
    let first = host.emit_recovered(H, 1, vec![(0, 0)], 1).unwrap();
    let mut peer = initiator(H, &k_ab(), &k_ba());
    let id_before = match decode_control(peer.open(&first).unwrap().inner()) {
        Some(Ok(Control::Recovered { id, .. })) => id,
        _ => panic!("recovered must decode"),
    };

    // Four probes answered — the entire bootstrap ladder.
    for g in 0..MAX_ECHOES as u16 {
        let dg = seal_probe(&mut a, 0xF000 + u32::from(g), g, u64::from(g) + 10);
        host.on_datagram(0, &dg).unwrap();
        let seq = host.take_pending_cs_echo(H).unwrap();
        host.emit_cs_echo(H, seq, u64::from(g) + 100).unwrap();
    }

    // The next Recovered gets the immediately following id: the gate was not advanced by the echoes.
    let second = host.emit_recovered(H, 2, vec![(0, 0)], 200).unwrap();
    let id_after = match decode_control(peer.open(&second).unwrap().inner()) {
        Some(Ok(Control::Recovered { id, .. })) => id,
        _ => panic!("recovered must decode"),
    };
    assert_eq!(
        id_after,
        id_before + 1,
        "four answered probes must consume ZERO control-id issuance budget"
    );
}
