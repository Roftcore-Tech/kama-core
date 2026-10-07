//! L-A5c PR-2 — gerçek `SessionControl`'ün S3 conformance süitinin ÖRTMEDİĞİ davranışları
//! (`crate::session_control`; KM-SEAM-TC-001 K10, s3 v1.1; ccr §35.15; G1).
//!
//! * K10 provisional supersession: onay = provisional oturumda ilk AEAD-geçerli epoch-1 paketi →
//!   `Confirmed(new)`; eski oturum yalnız-alım drain → `Closed(eski, Superseded)`; onaylanmayan
//!   provisional `PROVISIONAL_TTL` ile SESSİZ düşer.
//! * Renew cutover: `Ready(h')` ve eski oturum drain → `Closed(h, Superseded)`.
//! * Glare: tek oturum doğar.
//! * Zaman: bekleyen INIT `HALF_OPEN_TTL` → `Closed(h, KeyHygiene)`; `SESSION_MAX_AGE` → tek
//!   `RenewRequired`.
//! * Opak retler (G1): dizinde olmayan gönderici · başka alıcıya INIT · eski format · değiştirilmiş
//!   yeniden gönderim · `early_unit` (sahip kararı S1).

#![cfg(feature = "test-util")]

use core::time::Duration;

use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SealError, SessionCommand, SessionControl,
    SessionEvent, SessionResult,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::entropy::OsEntropy;
use kama_noise_wire::epoch_machine::GRACE_T_G;
use kama_noise_wire::peer_dir::{LocalIdentity, PeerDirectory, PeerEntry};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::session_control::{
    ControlConfig, EnvelopeKind, HALF_OPEN_TTL, PROVISIONAL_TTL, SESSION_MAX_AGE,
};
use sha2::{Digest, Sha256};
use x25519_dalek::StaticSecret;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
const CAROL: u8 = 0xC0;
/// Dizinde OLMAYAN kimlik.
const MALLORY: u8 = 0xEE;

fn peer(n: u8) -> PeerRef {
    PeerRef::from_dest_hash([n; 32])
}

fn derive(label: &[u8], n: u8) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(label);
    h.update([n]);
    h.finalize().into()
}

fn identity(n: u8) -> LocalIdentity {
    LocalIdentity::new(
        StaticSecret::from(derive(b"a5c-static", n)),
        [n; 32],
        derive(b"a5c-cert-ref", n),
    )
}

/// Dizin: Alice, Bob, Carol (Mallory YOK).
fn directory() -> PeerDirectory {
    PeerDirectory::new([ALICE, BOB, CAROL].map(|n| {
        let id = identity(n);
        PeerEntry {
            dest_hash: id.dest_hash,
            static_x25519_pub: *id.static_public().as_bytes(),
            cert_ref: id.cert_ref,
        }
    }))
    .unwrap()
}

struct Node {
    c: NoiseWireProtection,
    clock: ManualClock,
}

fn node(n: u8, envelope: EnvelopeKind) -> Node {
    let clock = ManualClock::new();
    let c = NoiseWireProtection::with_control(ControlConfig {
        identity: identity(n),
        directory: directory(),
        clock: Box::new(clock.clone()),
        entropy: Box::new(OsEntropy::new()),
        envelope,
    });
    Node { c, clock }
}

impl Node {
    fn initiate(&mut self, to: u8) -> (SessionHandle, Vec<u8>) {
        match self.c.command(SessionCommand::Initiate {
            peer: peer(to),
            early_unit: None,
        }) {
            SessionResult::Initiated { session, bytes, .. } => (session, bytes),
            other => panic!("Initiate: {other:?}"),
        }
    }

    fn assign(&mut self, h: SessionHandle, hub: u32) {
        assert_eq!(
            self.c.command(SessionCommand::AssignHandle {
                session: h,
                hub_handle: HubHandle::from_raw(hub),
            }),
            SessionResult::NoOutput
        );
    }

    fn events(&mut self) -> Vec<SessionEvent> {
        std::iter::from_fn(|| self.c.poll_event()).collect()
    }

    fn seal(&mut self, h: SessionHandle) -> Result<Vec<u8>, SealError> {
        self.c
            .seal(ProtectionContext::Session(h), b"veri")
            .map(|u| u.bytes)
    }
}

/// Alice → Bob kurulu oturum (hub tutamacı `hub`). Döner: (Alice'in, Bob'un) tutamacı.
fn establish(a: &mut Node, b: &mut Node, hub: u32) -> (SessionHandle, SessionHandle) {
    let (ha, init) = a.initiate(BOB);
    a.assign(ha, hub);
    let SessionResult::EstablishedAndReply {
        session: hb,
        peer: p,
        bytes: resp,
        ..
    } = b.c.command(SessionCommand::AcceptInit { wire: init })
    else {
        panic!("AcceptInit");
    };
    assert_eq!(p, peer(ALICE));
    b.assign(hb, hub);
    assert_eq!(
        a.c.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Established { session: ha }
    );
    assert_eq!(a.events(), vec![SessionEvent::Ready(ha)]);
    assert_eq!(b.events(), vec![SessionEvent::Ready(hb)]);
    (ha, hb)
}

fn both_envelopes(f: impl Fn(EnvelopeKind)) {
    f(EnvelopeKind::Legacy);
    f(EnvelopeKind::Extended);
}

#[test]
fn renew_cutover_and_provisional_supersession_drain_the_old_session() {
    both_envelopes(|kind| {
        let (mut a, mut b) = (node(ALICE, kind), node(BOB, kind));
        let (ha1, hb1) = establish(&mut a, &mut b, 0x0101);

        // Alice yeniler: yeni bekleyen INIT; Bob'da eşle etkin oturum var → PROVISIONAL (K10).
        let SessionResult::Initiated {
            session: ha2,
            bytes: init2,
            ..
        } = a.c.command(SessionCommand::Renew { session: ha1 })
        else {
            panic!("Renew yeni bekleyen INIT üretmeli");
        };
        a.assign(ha2, 0x0202);
        let SessionResult::ProvisionalAndReply {
            new: hb2,
            supersedes,
            peer: p,
            bytes: resp2,
            ..
        } = b.c.command(SessionCommand::AcceptInit { wire: init2 })
        else {
            panic!("eşle oturum varken ProvisionalAndReply beklenir");
        };
        assert_eq!((supersedes, p), (hb1, peer(ALICE)));
        b.assign(hb2, 0x0202);
        assert!(b.events().is_empty(), "provisional Ready üretmez");

        // Onaydan önce eski oturum TAM çalışır.
        let w = a.seal(ha1).unwrap();
        assert!(b.c.open(&w).is_some());
        assert!(b.seal(hb1).is_ok());

        // Alice: yeni oturum kuruldu → Ready(h'); eski oturum yalnız-alım drain.
        assert_eq!(
            a.c.command(SessionCommand::AcceptResponse { wire: resp2 }),
            SessionResult::Established { session: ha2 }
        );
        assert_eq!(a.events(), vec![SessionEvent::Ready(ha2)]);
        assert_eq!(a.seal(ha1).unwrap_err(), SealError::UnknownSession);

        // Bob: provisional'da ilk AEAD-geçerli epoch-1 paketi = onay.
        let w = a.seal(ha2).unwrap();
        assert!(b.c.open(&w).is_some());
        assert_eq!(b.events(), vec![SessionEvent::Confirmed(hb2)]);
        assert_eq!(b.seal(hb1).unwrap_err(), SealError::UnknownSession);
        assert!(b.seal(hb2).is_ok());

        // Drain T_g sonra kapanır.
        b.clock.advance(GRACE_T_G);
        assert_eq!(
            b.events(),
            vec![SessionEvent::Closed(hb1, CloseReason::Superseded)]
        );
        a.clock.advance(GRACE_T_G);
        assert_eq!(
            a.events(),
            vec![SessionEvent::Closed(ha1, CloseReason::Superseded)]
        );
    });
}

#[test]
fn stale_provisional_is_dropped_silently_after_ttl() {
    let (mut a, mut b) = (
        node(ALICE, EnvelopeKind::Extended),
        node(BOB, EnvelopeKind::Extended),
    );
    let (ha1, hb1) = establish(&mut a, &mut b, 0x0303);
    let (_, init2) = a.initiate(BOB);
    let SessionResult::ProvisionalAndReply { new: hb2, .. } =
        b.c.command(SessionCommand::AcceptInit { wire: init2 })
    else {
        panic!("ProvisionalAndReply beklenir");
    };
    b.clock.advance(PROVISIONAL_TTL);
    // (Aynı süre SESSION_MAX_AGE'e de eşit: eski oturum için RenewRequired gelebilir — o, eski
    // oturumun yaş kuralıdır.) Provisional'a ait HİÇBİR olay yok.
    let about_provisional = |e: &SessionEvent| match *e {
        SessionEvent::Ready(h) | SessionEvent::Confirmed(h) | SessionEvent::RenewRequired(h) => {
            h == hb2
        }
        SessionEvent::Closed(h, _) => h == hb2,
    };
    assert!(
        !b.events().iter().any(about_provisional),
        "onaylanmayan provisional olay üretmez"
    );
    // Provisional gitti; eski oturum etkilenmedi.
    assert_eq!(
        b.c.command(SessionCommand::AssignHandle {
            session: hb2,
            hub_handle: HubHandle::from_raw(0x0404),
        }),
        SessionResult::Failed
    );
    let w = a.seal(ha1).unwrap();
    assert!(b.c.open(&w).is_some());
    assert!(b.seal(hb1).is_ok());
}

#[test]
fn glare_yields_a_single_session() {
    let (mut a, mut b) = (
        node(ALICE, EnvelopeKind::Legacy),
        node(BOB, EnvelopeKind::Legacy),
    );
    let (ha, init_a) = a.initiate(BOB);
    let (hb, init_b) = b.initiate(ALICE);
    // dest_hash'i küçük olan kazanır: Alice ([0xA1; 32]) < Bob ([0xB0; 32]).
    assert_eq!(
        a.c.command(SessionCommand::AcceptInit { wire: init_b }),
        SessionResult::NoOutput,
        "kazanan karşı INIT'i sessizce düşürür"
    );
    let SessionResult::EstablishedAndReply {
        session: hb_new,
        bytes: resp,
        ..
    } = b.c.command(SessionCommand::AcceptInit { wire: init_a })
    else {
        panic!("kaybeden kazananın INIT'ini işler");
    };
    assert_eq!(b.c.pending_len(), 0, "kaybedenin bekleyen INIT'i silindi");
    assert_ne!(hb_new, hb);
    assert_eq!(
        a.c.command(SessionCommand::AcceptResponse { wire: resp }),
        SessionResult::Established { session: ha }
    );
    assert_eq!(a.events(), vec![SessionEvent::Ready(ha)]);
    assert_eq!(b.events(), vec![SessionEvent::Ready(hb_new)], "tek Ready");
}

#[test]
fn pending_init_expires_with_key_hygiene() {
    let mut a = node(ALICE, EnvelopeKind::Extended);
    let (h, _) = a.initiate(BOB);
    a.clock.advance(HALF_OPEN_TTL - Duration::from_secs(1));
    assert!(a.events().is_empty());
    a.clock.advance(Duration::from_secs(1));
    assert_eq!(
        a.events(),
        vec![SessionEvent::Closed(h, CloseReason::KeyHygiene)]
    );
    assert_eq!(a.c.pending_len(), 0);
}

#[test]
fn session_max_age_signals_renew_exactly_once() {
    let (mut a, mut b) = (
        node(ALICE, EnvelopeKind::Legacy),
        node(BOB, EnvelopeKind::Legacy),
    );
    let (ha, _) = establish(&mut a, &mut b, 0x0505);
    a.clock.advance(SESSION_MAX_AGE);
    assert_eq!(a.events(), vec![SessionEvent::RenewRequired(ha)]);
    a.clock.advance(SESSION_MAX_AGE);
    assert!(a.events().is_empty(), "RenewRequired tek sefer");
    assert!(a.seal(ha).is_ok(), "yaş oturumu öldürmez (timeout DEĞİL)");
}

#[test]
fn opaque_rejects_leave_state_untouched() {
    let mut b = node(BOB, EnvelopeKind::Extended);
    let fp = b.c.state_fingerprint();
    let refuse = |b: &mut Node, wire: Vec<u8>, what: &str| {
        assert_eq!(
            b.c.command(SessionCommand::AcceptInit { wire }),
            SessionResult::Failed,
            "{what}"
        );
    };

    // Dizinde olmayan gönderici (G1): Mallory'nin dizini Bob'u tanır, Bob Mallory'yi tanımaz.
    let mut m = {
        let clock = ManualClock::new();
        let dir = [PeerEntry {
            dest_hash: identity(BOB).dest_hash,
            static_x25519_pub: *identity(BOB).static_public().as_bytes(),
            cert_ref: identity(BOB).cert_ref,
        }];
        Node {
            c: NoiseWireProtection::with_control(ControlConfig {
                identity: identity(MALLORY),
                directory: PeerDirectory::new(dir).unwrap(),
                clock: Box::new(clock.clone()),
                entropy: Box::new(OsEntropy::new()),
                envelope: EnvelopeKind::Extended,
            }),
            clock,
        }
    };
    let (_, from_mallory) = m.initiate(BOB);
    refuse(&mut b, from_mallory, "dizinde olmayan gönderici");

    // Başka alıcıya (Carol) adresli INIT.
    let mut a = node(ALICE, EnvelopeKind::Extended);
    let (_, to_carol) = a.initiate(CAROL);
    refuse(&mut b, to_carol, "başka alıcı");

    // Eski format (magic'ten ret).
    refuse(&mut b, b"SAI1 eski async init".to_vec(), "eski format");

    assert_eq!(b.c.state_fingerprint(), fp, "ret yolları state'e dokundu");
    assert!(b.events().is_empty());

    // Değiştirilmiş yeniden gönderim: ilk INIT işlendi; aynı sayaçlı FARKLI bayt → ret.
    let (_, init) = a.initiate(BOB);
    assert!(matches!(
        b.c.command(SessionCommand::AcceptInit { wire: init.clone() }),
        SessionResult::EstablishedAndReply { .. }
    ));
    let _ = b.events();
    let fp = b.c.state_fingerprint();
    let mut changed = init;
    let last = changed.len() - 1;
    changed[last] ^= 1;
    refuse(&mut b, changed, "aynı sayaç, farklı bayt");
    assert_eq!(b.c.state_fingerprint(), fp);
}

#[test]
fn early_unit_is_refused_this_sprint() {
    let mut a = node(ALICE, EnvelopeKind::Legacy);
    assert_eq!(
        a.c.command(SessionCommand::Initiate {
            peer: peer(BOB),
            early_unit: Some(b"erken".to_vec()),
        }),
        SessionResult::Failed,
        "sahip kararı S1: early_unit Transport'ta saklanır"
    );
    assert_eq!(a.c.pending_len(), 0);
    assert!(matches!(
        a.c.command(SessionCommand::Initiate {
            peer: peer(BOB),
            early_unit: Some(Vec::new()),
        }),
        SessionResult::Initiated { .. }
    ));
}

#[test]
fn unknown_peer_and_unbound_session_cannot_be_used() {
    let mut a = node(ALICE, EnvelopeKind::Extended);
    assert_eq!(
        a.c.command(SessionCommand::Initiate {
            peer: peer(MALLORY),
            early_unit: None,
        }),
        SessionResult::Failed,
        "dizinde olmayan eşe INIT yok"
    );
    let (h, _) = a.initiate(BOB);
    assert_eq!(
        a.seal(h).unwrap_err(),
        SealError::UnknownSession,
        "hub tutamacı bağlanmadan oturum mühürlemez"
    );
    a.assign(h, 0x0606);
    assert!(a.seal(h).is_ok(), "bağdan sonra epoch-0 (0-RTT) mühürlenir");
}
