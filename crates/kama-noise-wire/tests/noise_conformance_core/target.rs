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
//! `mock` iki aileyi de taşır. Gerçek hedef (`NoiseWireProtection`, iki zarf profiliyle) bugün
//! yalnız ilk aileyi taşır: `SessionControl` L-A5c'de (A5.5) gelir; o güne dek kontrol ailesi
//! gerçek hedef için BEYAN EDİLMİŞ boşluktur ([`GAP_REAL_SESSION_CONTROL`]).

#![allow(dead_code)]

use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{
    HubHandle, PacketProtection, ProtectionContext, SessionControl, SessionEvent,
};
use kama_noise_wire::envelope::EnvelopeProfile;
use kama_noise_wire::protector::{NoiseWireProtection, SessionKeyMaterial};
use kama_noise_wire::seam_mock::{self, MockCrypto};
use kama_noise_wire::{LegacyDirection, KEY_LEN};
use zeroize::Zeroizing;

/// Gerçek hedefte henüz olmayan ailenin kimliği (koşucu ile paylaşılır): bugün yalnız
/// `SessionControl` ailesi (A5.5, L-A5c).
pub const GAP_REAL_SESSION_CONTROL: &str = "GAP-S3-REAL-A5";

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

/// Kayıtlı hedefler — `session_control_on_real_is_a_declared_gap` bu listeye karşı sınar.
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
const REAL_SESSION_ID_INTERNAL: [u8; 16] = [0x51; 16];
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
