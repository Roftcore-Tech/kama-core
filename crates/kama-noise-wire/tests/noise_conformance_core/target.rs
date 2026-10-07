//! Hedef katmanı — S3 sözleşme süiti "hangi Crypto'ya karşı koşuyorum" sorusu.
//!
//! Süit yalnız S3 yüzeyini (`SessionControl` / `PacketProtection`) ve aşağıdaki FİKSTÜR kancalarını
//! kullanır. Kancalar sözleşmenin parçası değildir: SSR1'i üretecek karşı ucu ve gözlem noktalarını
//! (deneme sayacı, state parmak izi) hedefe özgü biçimde sağlarlar. Gerçek hedef aynı kancaları
//! gerçek el sıkışmayla doldurur; satır gövdeleri değişmez.
//!
//! # İki aile [2026-10-08, L-A5b]
//!
//! * [`ProtectionTarget`] — yalnız `PacketProtection` satırları (H-CTX, H-PN-OWN, H-FEC-ORDER,
//!   H-NO-MUTATE'in paket yarısı, H-REPLAY-POISON). Kurulu uç çifti hedefin kendi fikstürüdür
//!   ([`ProtectionTarget::established`]).
//! * [`S3Target`] — buna `SessionControl` satırlarını ekler (H-RETRY, H-HANDLE, H-DEMUX,
//!   H-NO-MUTATE'in kontrol yarısı).
//!
//! `mock` iki aileyi de taşır. Gerçek hedef (`NoiseWireProtection`, iki zarf profiliyle) iki
//! biçimde kayıtlıdır: paket hedefleri (`RealLegacyTarget` / `RealExtendedTarget`; oturum
//! doğrudan kurulur, PacketProtection ailesi) ve kontrol hedefleri (`RealLegacyControlTarget` /
//! `RealExtendedControlTarget`; ürün kurulumu `with_control`, gerçek el sıkışma + HANDLE_ASSIGN,
//! SessionControl ailesi + H-DEMUX). ~~`SessionControl` L-A5c'de (A5.5) gelir; o güne dek kontrol
//! ailesi gerçek hedef için BEYAN EDİLMİŞ boşluktur~~ [2026-10-08, L-A5c PR-2] boşluk kapandı.

#![allow(dead_code)]

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SessionControl, SessionEvent,
};
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::entropy::OsEntropy;
use kama_noise_wire::envelope::EnvelopeProfile;
use kama_noise_wire::handshake::{dh, seal_ssr1, session_context_v1, Ssr1Seal};
use kama_noise_wire::kdf::{self, SESSION_ID_INTERNAL_LEN};
use kama_noise_wire::peer_dir::{LocalIdentity, PeerDirectory, PeerEntry};
use kama_noise_wire::protector::{NoiseWireProtection, SessionKeyMaterial};
use kama_noise_wire::seam_mock::{self, MockCrypto};
use kama_noise_wire::session_control::{self, ControlConfig, EnvelopeKind};
use kama_noise_wire::{LegacyDirection, KEY_LEN};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::s3_support::{peer, INITIATOR, PEER};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Mock,
    Real,
}

/// Bir hedefin süite kaydedildiği satır ailesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Yalnız `PacketProtection` satırları.
    PacketProtection,
    /// Yalnız `SessionControl` satırları + H-DEMUX (paket satırları aynı uygulamanın paket
    /// hedefinde koşar).
    SessionControl,
    /// `PacketProtection` + `SessionControl` satırları.
    Full,
}

/// Süite kayıtlı bir hedef (beyan): `main.rs`'teki süit makrosu çağrılarıyla birebir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registered {
    pub id: &'static str,
    pub kind: TargetKind,
    pub family: Family,
}

/// Hedef kurulamadı — typed; panik değil, `todo!()` değil (CLAUDE.md §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnavailable {
    NotImplemented { gap: &'static str },
}

/// Pending kümesi yeni INIT kabul etmedi (ccr §35.15: istemci pending INIT tavanı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRefused;

/// `PacketProtection` satırlarının hedefi.
pub trait ProtectionTarget {
    type Crypto: PacketProtection;
    const ID: &'static str;
    const KIND: TargetKind;

    /// Hedefte `Session` ve `HubChannel` ayrı anahtarla mı mühürlenir (H-CTX'in anahtar yarısı).
    /// `false` = HubChannel PoC no-op'tur (`IdentitySeam` paritesi; H-CTX'in no-op yarısı).
    const CONTEXTS_ARE_KEY_SEPARATED: bool;

    /// Kurulu uç çifti `(a, b, h)`: `h` iki uçta da AYNI oturumu adlandırır; `a`'nın mühürlediğini
    /// `b` açar ve tersi.
    fn established() -> (Self::Crypto, Self::Crypto, SessionHandle);

    /// `seal()`'e en son verilen bağlam — seam'in kendi kaydı (K1 kayıt kancası).
    fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext>;

    /// Oturum/replay/pending/anahtar state'inin parmak izi; salt-gözlem alanları HARİÇ.
    fn state_fingerprint(c: &Self::Crypto) -> String;

    /// Oturumu epoch-1'e geçirir (gönderim tarafı). Hedef geçişi yapamıyorsa `false`.
    fn enter_epoch1(c: &mut Self::Crypto, session: SessionHandle) -> bool;

    /// Crypto → Transport olay kuyruğunun sıradaki olayı (S3 `poll_event`'in gözlem kancası).
    fn poll_event(c: &mut Self::Crypto) -> Option<SessionEvent>;
}

/// `SessionControl` satırlarının da hedefi.
pub trait S3Target: ProtectionTarget<Crypto: SessionControl> {
    /// ccr §35.15 `PENDING_RESPONSE_TRIAL_CAP` — hedefin ADLANDIRILMIŞ sabitinden gelir.
    const TRIAL_CAP: usize;

    /// # Errors
    /// Hedef bu derlemede yoksa [`TargetUnavailable`] (beyan edilmiş boşluk).
    fn build() -> Result<Self::Crypto, TargetUnavailable>;

    /// Kimliği `identity` olan uç [s3 v1.1]: bu ucun INIT'ini kabul eden karşı uç
    /// `EstablishedAndReply.peer` olarak bu kimliği görür.
    ///
    /// # Errors
    /// Hedef bu derlemede yoksa [`TargetUnavailable`].
    fn build_as(identity: PeerRef) -> Result<Self::Crypto, TargetUnavailable>;

    /// Yanıt bekleyen bir INIT kaydeder; `tag` yalnız fikstürün o INIT'i sonradan anmasına yarar.
    ///
    /// # Errors
    /// Pending kümesi tavandaysa [`PendingRefused`].
    fn add_pending(c: &mut Self::Crypto, tag: u64) -> Result<(), PendingRefused>;

    /// `tag`'li INIT'e karşı ucun üreteceği GEÇERLİ RESPONSE zarfı.
    fn response_for(tag: u64) -> Vec<u8>;

    /// Geçerli bir RESPONSE zarfının doğrulama alanı (tag) bozulmuş kopyası.
    fn corrupt(valid: &[u8]) -> Vec<u8>;

    /// Son `AcceptResponse`'ta yapılan aday denemesi sayısı (K7 gözlem noktası).
    fn last_trials(c: &Self::Crypto) -> usize;

    fn pending_len(c: &Self::Crypto) -> usize;

    /// Pending kümesi + oturum/replay state'inin parmak izi (H-NO-MUTATE karşılaştırmaları).
    /// Deneme sayacı gibi salt-gözlem alanları da içindedir; karşılaştıran taraf bunu bilir.
    fn fingerprint(c: &Self::Crypto) -> String;
}

/// Kayıtlı hedefler — `real_covers_both_families` bu listeye karşı sınar.
pub const REGISTERED_TARGETS: &[Registered] = &[
    Registered {
        id: MockTarget::ID,
        kind: TargetKind::Mock,
        family: Family::Full,
    },
    Registered {
        id: RealLegacyTarget::ID,
        kind: TargetKind::Real,
        family: Family::PacketProtection,
    },
    Registered {
        id: RealExtendedTarget::ID,
        kind: TargetKind::Real,
        family: Family::PacketProtection,
    },
    Registered {
        id: RealLegacyControlTarget::ID,
        kind: TargetKind::Real,
        family: Family::SessionControl,
    },
    Registered {
        id: RealExtendedControlTarget::ID,
        kind: TargetKind::Real,
        family: Family::SessionControl,
    },
];

// ── Sahte hedef ──────────────────────────────────────────────────────────────────────────────

pub struct MockTarget;

impl ProtectionTarget for MockTarget {
    type Crypto = MockCrypto;
    const ID: &'static str = "mock";
    const KIND: TargetKind = TargetKind::Mock;
    const CONTEXTS_ARE_KEY_SEPARATED: bool = true;

    fn established() -> (Self::Crypto, Self::Crypto, SessionHandle) {
        crate::s3_support::handshake::<Self>()
    }

    fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext> {
        c.last_seal_ctx()
    }

    fn state_fingerprint(c: &Self::Crypto) -> String {
        c.state_fingerprint()
    }

    fn enter_epoch1(c: &mut Self::Crypto, session: SessionHandle) -> bool {
        c.mock_set_epoch(session, 1)
    }

    fn poll_event(c: &mut Self::Crypto) -> Option<SessionEvent> {
        SessionControl::poll_event(c)
    }
}

impl S3Target for MockTarget {
    const TRIAL_CAP: usize = seam_mock::PENDING_RESPONSE_TRIAL_CAP;

    fn build() -> Result<Self::Crypto, TargetUnavailable> {
        Ok(MockCrypto::new())
    }

    fn build_as(identity: PeerRef) -> Result<Self::Crypto, TargetUnavailable> {
        Ok(MockCrypto::with_identity(identity))
    }

    fn add_pending(c: &mut Self::Crypto, tag: u64) -> Result<(), PendingRefused> {
        c.add_pending(tag).map_err(|_| PendingRefused)
    }

    fn response_for(tag: u64) -> Vec<u8> {
        MockCrypto::response_envelope(tag)
    }

    fn corrupt(valid: &[u8]) -> Vec<u8> {
        // Sahtede RESPONSE'un tamamı doğrulama alanıdır (ayrı tag yok). En yüksek bit çevrilir:
        // en düşük bit çevrilseydi sonuç KOMŞU bir pending'in geçerli RESPONSE'u olurdu ve
        // "bozuk paket" yerine "başka aday" sınanırdı (ilk yazımda ölçüldü).
        let mut w = valid.to_vec();
        if let Some(first) = w.first_mut() {
            *first ^= 0x80;
        }
        w
    }

    fn last_trials(c: &Self::Crypto) -> usize {
        c.last_response_trials
    }

    fn pending_len(c: &Self::Crypto) -> usize {
        c.pending_len()
    }

    fn fingerprint(c: &Self::Crypto) -> String {
        format!("{c:?}")
    }
}

// ── Gerçek hedef: `NoiseWireProtection` (L-A5b; `SessionControl` L-A5c'de) ───────────────────

/// Fikstürün oturum parametreleri. Değerleri süitin iddialarına girmez: iki uç aynı parametreyle
/// kurulur ve satırlar yalnız uçlar arası tutarlılığı sınar.
const REAL_HUB_HANDLE: u32 = 0x0000_5A5A;
const REAL_PROTOCOL_VERSION: u8 = 1;
const REAL_CIPHER_SUITE_ID: u16 = 1;
const REAL_SESSION_ID_INTERNAL: [u8; SESSION_ID_INTERNAL_LEN] = [0x51; SESSION_ID_INTERNAL_LEN];
/// Fikstür trafik anahtarları (ccr §35.2.1 rol eşlemesi: `a` = initiator).
const KEY_A2B_E0: [u8; KEY_LEN] = [0xA0; KEY_LEN];
const KEY_B2A_E0: [u8; KEY_LEN] = [0xB0; KEY_LEN];
const KEY_A2B_E1: [u8; KEY_LEN] = [0xA1; KEY_LEN];
const KEY_B2A_E1: [u8; KEY_LEN] = [0xB1; KEY_LEN];

fn key(bytes: [u8; KEY_LEN]) -> Zeroizing<[u8; KEY_LEN]> {
    Zeroizing::new(bytes)
}

/// İki uç, aynı oturum: `a` initiator (`send = A2B`), `b` responder (`send = B2A`). Taze uçta ilk
/// basılan tutamaç iki tarafta da aynıdır; satırlar bunu `established()` sözleşmesi olarak ister.
fn real_pair(
    profile_a: EnvelopeProfile,
    profile_b: EnvelopeProfile,
) -> (NoiseWireProtection, NoiseWireProtection, SessionHandle) {
    let mut a = NoiseWireProtection::new();
    let mut b = NoiseWireProtection::new();
    let ha = a
        .install_session_for_test(SessionKeyMaterial {
            profile: profile_a,
            send_e0: key(KEY_A2B_E0),
            recv_e0: key(KEY_B2A_E0),
        })
        .expect("taze uçta oturum kurulur");
    let hb = b
        .install_session_for_test(SessionKeyMaterial {
            profile: profile_b,
            send_e0: key(KEY_B2A_E0),
            recv_e0: key(KEY_A2B_E0),
        })
        .expect("taze uçta oturum kurulur");
    assert_eq!(ha, hb, "fikstür: iki uç aynı tutamacı adlandırmalı");
    (a, b, ha)
}

/// `a`'yı epoch-1'e geçirir: epoch-1 anahtarları kurulur, gönderim epoch'u ileri alınır.
fn real_enter_epoch1(c: &mut NoiseWireProtection, session: SessionHandle) -> bool {
    c.install_epoch1_for_test(session, &key(KEY_A2B_E1), &key(KEY_B2A_E1))
        && c.set_send_epoch_for_test(session, 1)
}

fn legacy_profile(send_direction: LegacyDirection) -> EnvelopeProfile {
    EnvelopeProfile::Legacy {
        hub_handle: HubHandle::from_raw(REAL_HUB_HANDLE),
        protocol_version: REAL_PROTOCOL_VERSION,
        cipher_suite_id: REAL_CIPHER_SUITE_ID,
        session_id_internal: REAL_SESSION_ID_INTERNAL,
        send_direction,
    }
}

/// Gerçek hedef, 22 B legacy zarf (ccr §35.6 / §35.9).
pub struct RealLegacyTarget;

impl ProtectionTarget for RealLegacyTarget {
    type Crypto = NoiseWireProtection;
    const ID: &'static str = "real-legacy";
    const KIND: TargetKind = TargetKind::Real;
    const CONTEXTS_ARE_KEY_SEPARATED: bool = false;

    fn established() -> (Self::Crypto, Self::Crypto, SessionHandle) {
        real_pair(
            legacy_profile(LegacyDirection::AToB),
            legacy_profile(LegacyDirection::BToA),
        )
    }

    fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext> {
        c.last_seal_ctx()
    }

    fn state_fingerprint(c: &Self::Crypto) -> String {
        c.state_fingerprint()
    }

    fn enter_epoch1(c: &mut Self::Crypto, session: SessionHandle) -> bool {
        real_enter_epoch1(c, session)
    }

    fn poll_event(c: &mut Self::Crypto) -> Option<SessionEvent> {
        c.take_event_for_test()
    }
}

/// Gerçek hedef, 26 B extended zarf (noise-wire §2.5 / §2.6 / §2.8).
pub struct RealExtendedTarget;

impl ProtectionTarget for RealExtendedTarget {
    type Crypto = NoiseWireProtection;
    const ID: &'static str = "real-extended";
    const KIND: TargetKind = TargetKind::Real;
    const CONTEXTS_ARE_KEY_SEPARATED: bool = false;

    fn established() -> (Self::Crypto, Self::Crypto, SessionHandle) {
        let profile = || EnvelopeProfile::Extended {
            h_ext: HubHandle::from_raw(REAL_HUB_HANDLE),
        };
        real_pair(profile(), profile())
    }

    fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext> {
        c.last_seal_ctx()
    }

    fn state_fingerprint(c: &Self::Crypto) -> String {
        c.state_fingerprint()
    }

    fn enter_epoch1(c: &mut Self::Crypto, session: SessionHandle) -> bool {
        real_enter_epoch1(c, session)
    }

    fn poll_event(c: &mut Self::Crypto) -> Option<SessionEvent> {
        c.take_event_for_test()
    }
}

// ── Gerçek KONTROL hedefi: ürün kurulumu (`with_control`) + gerçek el sıkışma (L-A5c PR-2) ────

/// `build()`'in varsayılan ucu (satırların andığı eşlerden farklı).
const DEFAULT_ID: u64 = 0x00DE_FA17;
/// Fikstür dizinindeki kimlikler: varsayılan uç + satırların andığı eşler.
const FIXTURE_IDS: [u64; 4] = [DEFAULT_ID, INITIATOR, PEER, PEER + 1];
const LABEL_STATIC: &[u8] = b"kama-fixture-static-v1";
const LABEL_CERT_REF: &[u8] = b"kama-fixture-cert-ref-v1";
const LABEL_INIT_EPH: &[u8] = b"kama-fixture-init-ephemeral-v1";
const LABEL_RESP_EPH: &[u8] = b"kama-fixture-response-ephemeral-v1";

fn derive(label: &[u8], input: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(label);
    h.update(input);
    h.finalize().into()
}

/// Fikstür kimliği: `dest_hash`'ten deterministik statik X25519 ve `cert_ref` (TEST anahtarları).
pub fn fixture_identity(p: PeerRef) -> LocalIdentity {
    LocalIdentity::new(
        StaticSecret::from(derive(LABEL_STATIC, p.as_bytes())),
        *p.as_bytes(),
        derive(LABEL_CERT_REF, p.as_bytes()),
    )
}

/// G1 eş dizini: fikstür kimliklerinin hepsi (bant dışı "doğrulanmış" sayılır).
fn fixture_directory() -> PeerDirectory {
    PeerDirectory::new(FIXTURE_IDS.iter().map(|&n| {
        let id = fixture_identity(peer(n));
        PeerEntry {
            dest_hash: id.dest_hash,
            static_x25519_pub: *id.static_public().as_bytes(),
            cert_ref: id.cert_ref,
        }
    }))
    .expect("fikstür dizini kurulmalı")
}

/// Ürün kurulumu; saatin test kopyası dışarıda kalır (zaman hücreleri ilerletir).
pub fn real_endpoint(
    identity: PeerRef,
    envelope: EnvelopeKind,
) -> (NoiseWireProtection, ManualClock) {
    let clock = ManualClock::new();
    let p = NoiseWireProtection::with_control(ControlConfig {
        identity: fixture_identity(identity),
        directory: fixture_directory(),
        clock: Box::new(clock.clone()),
        entropy: Box::new(OsEntropy::new()),
        envelope,
    });
    (p, clock)
}

fn ephemeral_for(label: &[u8], tag: u64) -> [u8; 32] {
    derive(label, &tag.to_be_bytes())
}

/// `add_pending(tag)` ile varsayılan uçtan `PEER`'e açılan INIT'e Bob'un vereceği GEÇERLİ SSR1 —
/// Crypto'ya bakmadan, aynı türetimle bağımsız hesaplanır.
fn real_response_for(tag: u64) -> Vec<u8> {
    let alice = fixture_identity(peer(DEFAULT_ID));
    let bob = fixture_identity(peer(PEER));
    let eph = StaticSecret::from(ephemeral_for(LABEL_INIT_EPH, tag));
    let ss_e = dh(&eph, bob.static_public()).expect("DH");
    let ss_s = dh(alice.static_secret(), bob.static_public()).expect("DH");
    let ctx = session_context_v1(&alice.cert_ref, &bob.dest_hash, bob.static_public());
    let root = kdf::d1_init_root(&ss_e, &ss_s, &ctx).expect("D1");
    let sid = kdf::d3_session_id_internal(&root, &ctx).expect("D3");
    let resp_eph = StaticSecret::from(ephemeral_for(LABEL_RESP_EPH, tag));
    seal_ssr1(&Ssr1Seal {
        init_root: &root,
        session_id_internal: &sid,
        initiator_ephemeral: &PublicKey::from(&eph),
        initiator_static: alice.static_public(),
        responder_ephemeral: &resp_eph,
        response_counter: 0,
        response_data: &[],
    })
    .expect("SSR1")
    .0
}

macro_rules! real_control_target {
    ($name:ident, $id:literal, $envelope:expr) => {
        pub struct $name;

        impl ProtectionTarget for $name {
            type Crypto = NoiseWireProtection;
            const ID: &'static str = $id;
            const KIND: TargetKind = TargetKind::Real;
            const CONTEXTS_ARE_KEY_SEPARATED: bool = false;

            fn established() -> (Self::Crypto, Self::Crypto, SessionHandle) {
                crate::s3_support::handshake::<Self>()
            }

            fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext> {
                c.last_seal_ctx()
            }

            fn state_fingerprint(c: &Self::Crypto) -> String {
                c.state_fingerprint()
            }

            /// Epoch geçişi bu hedefte makinenin işidir (paket satırları paket hedeflerinde).
            fn enter_epoch1(_: &mut Self::Crypto, _: SessionHandle) -> bool {
                false
            }

            fn poll_event(c: &mut Self::Crypto) -> Option<SessionEvent> {
                SessionControl::poll_event(c)
            }
        }

        impl S3Target for $name {
            const TRIAL_CAP: usize = session_control::PENDING_RESPONSE_TRIAL_CAP;

            fn build() -> Result<Self::Crypto, TargetUnavailable> {
                Ok(real_endpoint(peer(DEFAULT_ID), $envelope).0)
            }

            fn build_as(identity: PeerRef) -> Result<Self::Crypto, TargetUnavailable> {
                Ok(real_endpoint(identity, $envelope).0)
            }

            fn add_pending(c: &mut Self::Crypto, tag: u64) -> Result<(), PendingRefused> {
                c.initiate_with_ephemeral_for_test(peer(PEER), ephemeral_for(LABEL_INIT_EPH, tag))
                    .map(|_| ())
                    .ok_or(PendingRefused)
            }

            fn response_for(tag: u64) -> Vec<u8> {
                real_response_for(tag)
            }

            fn corrupt(valid: &[u8]) -> Vec<u8> {
                // AEAD etiketinin son biti.
                let mut w = valid.to_vec();
                if let Some(last) = w.last_mut() {
                    *last ^= 1;
                }
                w
            }

            fn last_trials(c: &Self::Crypto) -> usize {
                c.last_response_trials()
            }

            fn pending_len(c: &Self::Crypto) -> usize {
                c.pending_len()
            }

            fn fingerprint(c: &Self::Crypto) -> String {
                format!(
                    "{}|trials={}",
                    c.state_fingerprint(),
                    c.last_response_trials()
                )
            }
        }
    };
}

real_control_target!(
    RealLegacyControlTarget,
    "real-legacy-control",
    EnvelopeKind::Legacy
);
real_control_target!(
    RealExtendedControlTarget,
    "real-extended-control",
    EnvelopeKind::Extended
);
