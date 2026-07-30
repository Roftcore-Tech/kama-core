//! A2-F3 — ControlDedupWindow product-entegrasyonu (Abkarada §9): gerçek control-ingest üzerinden
//! probe/echo dedup'a GİRMEZ; non-probe control fixed-window dedup (Fresh/DuplicateAckOnly);
//! Closing'de pencere düşer (session-slot ile birlikte).

#![cfg(feature = "experimental-cold-start")]

use kama_integration::{IngestOutcome, ReceiverHost};
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::control::Control;
use kama_transport_schema::UnsealedTransportPacket;
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0000_0055;

fn key() -> Zeroizing<[u8; 32]> {
    Zeroizing::new([0x22u8; 32])
}
fn sender() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H_EXT, &key(), &key())
}
fn seal_control(s: &mut KamaNoiseWireProtector, c: Control, pn: u64) -> Vec<u8> {
    s.seal(
        &UnsealedTransportPacket::Control(c),
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .unwrap()
    .bytes
}

/// Probe → echo; ControlDedupWindow'a GİRMEZ (A-2). cs_echo_count artar.
#[test]
fn f3_probe_does_not_enter_dedup() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let probe = Control::CsProbe {
        id: 5,
        subtype: 0x01,
        probe_seq: 7,
    };
    let dg = seal_control(&mut snd, probe.clone(), 0);
    assert_eq!(
        host.on_datagram(0, &dg).unwrap(),
        IngestOutcome::ProbeEchoed
    );
    assert_eq!(host.cs_echo_count(), 1);
    // İkinci probe (aynı-id, yeni-pn): yine echo (dedup-Duplicate DEĞİL — pencereye girmedi).
    let dg2 = seal_control(&mut snd, probe, 1);
    assert_eq!(
        host.on_datagram(0, &dg2).unwrap(),
        IngestOutcome::ProbeEchoed
    );
    assert_eq!(host.cs_echo_count(), 2);
}

/// Non-probe control → fixed-window dedup: ilk Fresh, retransmit DuplicateAckOnly.
#[test]
fn f3_non_probe_control_deduped() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let close = Control::SessionClose {
        id: 42,
        reason_code: 0,
    };
    // İlk SESSION_CLOSE (id=42) → Fresh.
    let d0 = seal_control(&mut snd, close.clone(), 0);
    assert_eq!(
        host.on_datagram(0, &d0).unwrap(),
        IngestOutcome::ControlFresh { inner_kind: 0x17 }
    );
    // Retransmit (aynı-id=42, yeni-pn=1) → DuplicateAckOnly (idempotent).
    let d1 = seal_control(&mut snd, close, 1);
    assert_eq!(
        host.on_datagram(0, &d1).unwrap(),
        IngestOutcome::ControlDuplicate
    );
    // Yeni control-id (43) → Fresh.
    let d2 = seal_control(
        &mut snd,
        Control::SessionClose {
            id: 43,
            reason_code: 0,
        },
        2,
    );
    assert_eq!(
        host.on_datagram(0, &d2).unwrap(),
        IngestOutcome::ControlFresh { inner_kind: 0x17 }
    );
}

/// F3.5: Closing (close_session) sonrası dedup-penceresi düşer (session-slot ile birlikte);
/// tombstone-yolu devreye girer (grace-recv).
#[test]
fn f3_closing_drops_dedup_window() {
    let mut host = ReceiverHost::new();
    host.install_session_with_keys(H_EXT, &key()).unwrap();
    let mut snd = sender();
    let d0 = seal_control(
        &mut snd,
        Control::SessionClose {
            id: 1,
            reason_code: 0,
        },
        0,
    );
    assert!(matches!(
        host.on_datagram(0, &d0).unwrap(),
        IngestOutcome::ControlFresh { .. }
    ));
    // Kapat → session-slot (dedup dahil) düşer; tombstone kurulur.
    host.close_session(H_EXT, |_s| {
        Ok(kama_integration::CachedCloseAck {
            grace_deadline_ns: ReceiverHost::default_grace_deadline_ns(0),
        })
    })
    .unwrap();
    assert_eq!(host.session_count(), 0);
    assert_eq!(host.tombstone_count(), 1);
    // Tombstone-grace: datagram → recv-only drop (dedup-penceresi yok).
    let d1 = seal_control(
        &mut snd,
        Control::SessionClose {
            id: 2,
            reason_code: 0,
        },
        1,
    );
    assert_eq!(
        host.on_datagram(0, &d1).unwrap(),
        IngestOutcome::Dropped("tombstone grace (recv-only)")
    );
}
