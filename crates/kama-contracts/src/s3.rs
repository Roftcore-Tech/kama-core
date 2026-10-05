//! S3 — Transport ↔ Crypto seam'i (`docs/seams/KM-SEAM-TC-001.md` §1.1 + §1.2'nin derlenen hâli).
//!
//! INV-ENGINE-SOC'un çalışma-zamanı sınırı. Transport datagram'ı OPAK taşır: dışarı çıkan her
//! çerçeve [`PacketProtection::seal`]'den, içeri giren her datagram [`PacketProtection::open`]'dan
//! geçer; oturum kurulumu [`SessionControl`]'ün typed komutlarıyla sürülür. Kripto bu iki trait'in
//! ARKASINDADIR; gerçek implementasyon `kama-noise-wire`'da yazılır ve aynı dikiş noktasına takılır.
//!
//!  * **K1** — [`ProtectionContext`] çağıranın verdiği TEK bağlamdır; Crypto çerçeve içeriğine
//!    bakarak anahtar seçmez.
//!  * **K2** — PN sayacını CRYPTO tutar. Transport PN üretmez; `seal()` dönüşündeki
//!    [`SealedPacketMeta::full_wire_pn`]'i kendi ARQ/ACK tablosuna kaydeder. Sayaç epoch'lar
//!    boyunca SÜREKLİDİR (noise-wire §4 EPOCH-PN-INIT): `dh_epoch` yalnız anahtarı seçer.
//!  * **K5** — `open()` başarısızlığı SESSİZDİR (`None`) ve state değiştirmez: "neden düştü"
//!    bilgisi demux/replay oracle'ıdır.
//!  * **K10 / DR-CONNECT-1 K5** — [`SessionResult::ProvisionalAndReply`] ve
//!    [`SessionEvent::Confirmed`]: mevcut oturumu olan eşten gelen yeni-sayaçlı INIT provisional
//!    kurulur; Transport Application'a `Ready`'yi `Confirmed`'de yayınlar.
//!
//! SPEC'TEN TEK FARK: spec `Bytes` yazar, burada `Vec<u8>` (bu crate'in harici bağımlılığı yoktur).
//!
//! Aynı adı taşıyan [`crate::s1::SessionEvent`] FARKLI bir tiptir (Application'ın gördüğü olay).

use core::fmt;

use crate::handles::{CloseReason, PeerRef, SessionHandle};

// ── Kimlikler ────────────────────────────────────────────────────────────────────────────────

/// Hub'ın yönlendirme tutamacı: wire'da cleartext taşınan `hub_handle` (ccr §35.7).
///
/// [`SessionHandle`] ile AYRI tiptir (S3 K3, ccr §35.7 AMENDMENT-6) ve bu ayrım TİP düzeyindedir:
/// biri diğerinin beklendiği yere konursa kod derlenmez (trybuild negatif testi
/// `kama-noise-wire/tests/ui/hub_handle_is_not_session_handle.rs`). `SessionHandle` yerel ve opaktır,
/// wire'a çıkmaz; `HubHandle` wire değeridir, hub onu paketi AÇMADAN yönlendirmek için okur.
/// Application bu tipi hiç görmez (yalnız S3'te vardır).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HubHandle(u32);

impl HubHandle {
    /// Wire değerinden kurar.
    #[must_use]
    pub const fn from_raw(v: u32) -> Self {
        Self(v)
    }

    /// Wire değeri (`hub_handle` alanına yazılan dört bayt).
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

// ── §1.1 PacketProtection ────────────────────────────────────────────────────────────────────

/// Çağıranın verdiği TEK koruma bağlamı (S3 K1).
///
/// `Session(h)` → o oturumun E2E cipher state'i ve PN uzayı. `HubChannel` → client↔hub hop anahtarı,
/// AYRI PN uzayı (engine ACK, HANDLE_ASSIGN, Plane 1 RPC, hop heartbeat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProtectionContext {
    Session(SessionHandle),
    HubChannel,
}

/// Tutamaç gerektirmeyen koruma sınıfı ([`PacketProtection::conservative_limits`] için).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtectionClass {
    Session,
    HubChannel,
}

/// Bir bağlamın/sınıfın boyut sınırları. Transport kripto format matematiğini TEKRAR ETMEZ; bu
/// sayıları Crypto'dan okur (S1 K5 zinciri).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionLimits {
    /// Mühürlemenin eklediği wire baytı (başlık + tag).
    pub wire_overhead_bytes: u32,
    /// Tek `seal()`'e verilebilecek azami plaintext.
    pub max_plaintext_bytes: u32,
}

/// Mühürlü paketin YALNIZ Crypto'ya ait meta alanları (noise-wire P0-4). `logical_ref` Transport'un
/// kendi tablosundadır (S3 K4), burada yoktur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedPacketMeta {
    /// Crypto'nun atadığı TAM sayaç (S3 K2). Transport bunu ARQ/ACK tablosuna yazar.
    pub full_wire_pn: u64,
    /// Mühürde/açmada kullanılan DH epoch'u.
    pub dh_epoch: u8,
    /// Wire'daki kesin bayt sayısı.
    pub sealed_len: u32,
}

/// Mühürlü birim: wire'a gidecek byte'lar (ek çerçeveleme YOK) + meta.
#[derive(Debug)]
pub struct SealedUnit {
    pub bytes: Vec<u8>,
    pub meta: SealedPacketMeta,
}

/// Açılmış datagram: Crypto'nun kimliklendirdiği bağlam + transport'un verdiği plaintext çerçeve.
#[derive(Debug)]
pub struct Opened {
    /// Server çok-oturumda demux bununla yapılır.
    pub ctx: ProtectionContext,
    /// Doğrulanmış düz çerçeve byte'ları.
    pub frame: Vec<u8>,
    pub meta: SealedPacketMeta,
}

/// Seal hataları — YALNIZ yapısal (S3 §3); kripto nedeni taşımaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// Çerçeve bağlamın `max_plaintext_bytes` sınırını aşıyor.
    FrameTooLarge,
    /// Bağlamdaki oturum bilinmiyor / kapanmış / superseded.
    UnknownSession,
    /// Sayaç tavanı (`SEND_PN_CAP`): yeni mühür YOK → oturum yenilenir. Sayaç ASLA sarmaz.
    CapReached,
}

impl fmt::Display for SealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FrameTooLarge => "frame too large",
            Self::UnknownSession => "unknown session",
            Self::CapReached => "cap reached",
        })
    }
}

impl std::error::Error for SealError {}

/// Datagram-düzeyi koruma (S3 §1.1). Endpoint başına TEK sahip (`Box<dyn PacketProtection>`;
/// `&mut` — gerçek katman PN/replay/oturum durumu tutar).
pub trait PacketProtection: Send + 'static {
    /// Düz çerçeveyi `ctx` bağlamında mühürler; PN'i Crypto atar (K2).
    ///
    /// # Errors
    /// Yapısal seal-reddi ([`SealError`]) — çağıran sayar, emisyonu kayıp-sayar (INV-RATE-1).
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError>;

    /// Wire datagram'ını açar/doğrular ve bağlamını kimliklendirir. `None` = SESSİZ düşürme;
    /// hiçbir state ilerlemez (K5).
    fn open(&mut self, wire: &[u8]) -> Option<Opened>;

    /// `plaintext_len` baytlık çerçevenin mühürlü wire boyu — KESİN (üst sınır değil); pacer
    /// admission'ı bununla yapılır: `seal(ctx, p)?.bytes.len() == sealed_len(ctx, p.len())?`.
    ///
    /// # Errors
    /// [`SealError::FrameTooLarge`] / [`SealError::UnknownSession`].
    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError>;

    /// Sınıfın TÜM profilleri için güvenli MİNİMUM; tutamaç gerektirmez (S1 `capabilities()`).
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits;

    /// Aktif bağlamın gerçek profili.
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits;
}

// ── §1.2 SessionControl ──────────────────────────────────────────────────────────────────────

/// Typed oturum komutu (K6: komut typed, zarf payload'ı OPAK).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// `early_unit` opak byte'tır (ccr §35.3 AMENDMENT-6); Crypto anlamını yorumlamaz.
    Initiate {
        peer: PeerRef,
        early_unit: Option<Vec<u8>>,
    },
    /// STORE_MESSAGE/DELIVER'dan gelen SSI1.
    AcceptInit {
        wire: Vec<u8>,
    },
    /// SSR1 — candidate seçimi Crypto'nun İÇİNDE (K7).
    AcceptResponse {
        wire: Vec<u8>,
    },
    Renew {
        session: SessionHandle,
    },
    Close {
        session: SessionHandle,
    },
}

/// Komut sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionResult {
    /// Transport bu zarfı Plane 1 ile yollar.
    OutboundEnvelope {
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    Established {
        session: SessionHandle,
    },
    /// `AcceptInit`: eşle mevcut oturum YOK → oturum hemen kullanılır, zarf = RESPONSE.
    EstablishedAndReply {
        session: SessionHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    /// `AcceptInit`: eşle mevcut oturum VAR (K10 / DR-CONNECT-1 K5). `new` PROVISIONAL'dır,
    /// `supersedes` TAM çalışmaya devam eder; zarf = RESPONSE. Onayda [`SessionEvent::Confirmed`].
    ProvisionalAndReply {
        new: SessionHandle,
        supersedes: SessionHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    NoOutput,
    /// OPAK (S3 §3): alt-sınıfsız; yanlış candidate ile bozuk paket ayırt edilemez.
    Failed,
}

/// Oturum yaşam-döngüsü olayı (Crypto → Transport). Transport bunu Application'a
/// [`crate::s1::SessionEvent`] olarak iletir; tek istisna `Confirmed`: Transport onu `Ready(new)`
/// olarak yayınlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Ready(SessionHandle),
    RenewRequired(SessionHandle),
    Closed(SessionHandle, CloseReason),
    /// Provisional oturum onaylandı: `new` üzerinde ilk AEAD-geçerli epoch-1 paketi açıldı.
    /// Ardından `supersedes` yalnız-alım drain'e düşer (KS-GRACE-1) → `Closed(_, Superseded)`.
    Confirmed(SessionHandle),
}

/// Oturum kontrolü (S3 §1.2). Crypto ağ retransmisyon timer'ı TUTMAZ (K9); retry Transport'un
/// cache'lediği zarfın byte-for-byte yeniden gönderimidir (INV-INIT-KEY-1).
pub trait SessionControl: Send + 'static {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult;
    fn poll_event(&mut self) -> Option<SessionEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Yüzey pini: kapalı kümeler. Variant eklenir/çıkarılırsa derlenmez.
    #[test]
    fn closed_sets_are_exhaustive() {
        match SealError::CapReached {
            SealError::FrameTooLarge | SealError::UnknownSession | SealError::CapReached => {}
        }
        let h = SessionHandle::from_raw(1);
        for ev in [
            SessionEvent::Ready(h),
            SessionEvent::RenewRequired(h),
            SessionEvent::Closed(h, CloseReason::Superseded),
            SessionEvent::Confirmed(h),
        ] {
            match ev {
                SessionEvent::Ready(_)
                | SessionEvent::RenewRequired(_)
                | SessionEvent::Closed(..)
                | SessionEvent::Confirmed(_) => {}
            }
        }
    }

    /// İki trait de dyn-uyumludur (Transport onları `Box<dyn …>` olarak tutar).
    #[test]
    fn traits_are_object_safe() {
        fn _p(_: &mut dyn PacketProtection) {}
        fn _s(_: &mut dyn SessionControl) {}
    }

    #[test]
    fn seal_error_is_a_std_error_with_stable_messages() {
        fn assert_error<E: std::error::Error + Send + Sync + 'static>(_: &E) {}
        for (e, msg) in [
            (SealError::FrameTooLarge, "frame too large"),
            (SealError::UnknownSession, "unknown session"),
            (SealError::CapReached, "cap reached"),
        ] {
            assert_error(&e);
            assert_eq!(e.to_string(), msg);
        }
    }

    #[test]
    fn hub_handle_roundtrips_its_wire_value() {
        assert_eq!(HubHandle::from_raw(0x00C0_FFEE).raw(), 0x00C0_FFEE);
    }

    /// `HubHandle` ile `SessionHandle` arasında dönüşüm YOKTUR (H-HANDLE'ın tip yarısının
    /// toolchain'den bağımsız kanıtı; hata metnine dayanan trybuild testi `kama-noise-wire`'da).
    ///
    /// Teknik: `Probe::<T>::CONVERTS` için iki aday vardır — `T: Into<SessionHandle>` iken seçilen
    /// inherent sabit ve her tip için geçerli trait sabiti. İnherent olan önceliklidir; dönüşüm
    /// eklenirse sabit `true` olur. İddialar DERLEME ZAMANINDA sınanır (`const { assert!(…) }`):
    /// bir dönüşüm eklenirse bu dosya derlenmez (2026-10-05'te ağaç dışı kopyada ölçüldü).
    #[test]
    fn hub_handle_does_not_convert_to_session_handle() {
        struct Probe<T>(core::marker::PhantomData<T>);
        trait Fallback {
            const CONVERTS: bool = false;
        }
        impl<T> Fallback for Probe<T> {}
        #[allow(dead_code)]
        impl<T: Into<SessionHandle>> Probe<T> {
            const CONVERTS: bool = true;
        }
        #[allow(dead_code)]
        struct Convertible;
        impl From<Convertible> for SessionHandle {
            fn from(_: Convertible) -> Self {
                SessionHandle::from_raw(0)
            }
        }
        // Yöntemin kendisi çalışıyor mu: dönüşümü OLAN bir tip `true` vermeli.
        const { assert!(Probe::<Convertible>::CONVERTS, "prob dönüşümü göremiyor") };
        const {
            assert!(
                !Probe::<HubHandle>::CONVERTS,
                "HubHandle → SessionHandle dönüşümü var"
            )
        };
        const { assert!(!Probe::<u32>::CONVERTS) };

        // Ters yön.
        struct ProbeRev<T>(core::marker::PhantomData<T>);
        trait FallbackRev {
            const CONVERTS: bool = false;
        }
        impl<T> FallbackRev for ProbeRev<T> {}
        #[allow(dead_code)]
        impl<T: Into<HubHandle>> ProbeRev<T> {
            const CONVERTS: bool = true;
        }
        const {
            assert!(
                !ProbeRev::<SessionHandle>::CONVERTS,
                "SessionHandle → HubHandle dönüşümü var"
            )
        };
    }
}
