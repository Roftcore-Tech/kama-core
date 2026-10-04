//! Kama Application katmanı — iskelet (B1.0).
//!
//! Bu crate'in bugünkü tek içeriği **Application ↔ Transport seam'inin (S1) derlenen hâlidir**:
//! `docs/seams/KM-SEAM-AT-001.md` §1 trait bloğu + §3 hata kümesi, DR-CONNECT-1 sonrası hâliyle.
//! Mesaj codec'i (AMP), metin/dosya/PTT akışları B1.1+ ve B2.* kartlarıyla gelir.
//!
//! # Katman kuralı (INV-NEIGHBOR-1)
//!
//! Application yalnız [`KamaTransport`]'u çağırır. Crypto'ya ve Plane 1'e (hub-RPC) erişimi YOKTUR;
//! bu, `Cargo.toml`'da kripto bağımlılığı olmamasıyla ve `scripts/ci/layer_check.py` ile zorlanır.
//! Application **retry yapmaz** (S1 K8): `send` başarısızsa kullanıcıya yansıtır ya da bekler.
//!
//! # `connect()` — kalıcı niyet (DR-CONNECT-1)
//!
//! `connect` için deadline YOKTUR. [`ConnectResult::Pending`] yalnız üç yolla biter:
//! [`SessionEvent::Ready`], [`KamaTransport::cancel`] ya da [`SessionEvent::ConnectAbandoned`].
//! [`SessionEvent::ConnectProgress`] HUB YOLUNUN durumudur; eşin canlılığı yalnız AMP
//! `RECEIPT(delivered)` ile bilinir.
//!
//! # Spec'ten iki bilinçli sapma (ikisi de S1 aynasındaki açık kalemlerle aynı)
//!
//! * `bytes: Bytes` yerine [`UnitBytes`] = `Vec<u8>` (SEAM-OPEN-1: `bytes` crate'ini doğrudan
//!   bağımlılık yapma kararı verilmedi). Sözleşme byte-özdeşlik ister, sahiplik modelini değil.
//! * `CloseReason`'ın variant kümesi spec'te kapalı ilan edilmedi (SEAM-OPEN-3); burada spec'in
//!   adlandırdığı ikisi var.

use std::time::Instant;

/// Sınırdan geçen birim byte'ları: bir AMP `AppMessageRecord` (application-message-protocol §2).
pub type UnitBytes = Vec<u8>;

/// §1 K5 — salt-okunur; değerleri Transport türetir, Application HESAPLAMAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportCapabilities {
    pub max_plaintext_unit_bytes: u32,
    pub preferred_plaintext_unit_bytes: u32,
}

/// §1 K3 — `bytes` bir `AppMessageRecord`'dur; Transport onu yapısal işler, semantik olarak asla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUnit {
    pub bytes: UnitBytes,
    pub compression_policy: CompressionPolicy,
}

/// §1 K4 — sıkıştırma Application'da yapılır; bu alan Transport için BİLGİLENDİRİCİDİR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionPolicy {
    Never,
    AppCompressed,
}

/// §4 — Application beyan eder, Transport uygular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryPolicy {
    Reliable,
    DeadlineBounded { deadline: Instant },
    BestEffort,
}

/// `recv()` çıktısı; `unit` gönderilen record ile byte-özdeştir (§2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    pub session: SessionHandle,
    pub unit: UnitBytes,
}

/// §1 K7 — olaylar typed'dır; Transport yorumlamadan iletir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Ready(SessionHandle, PeerRef),
    RenewRequired(SessionHandle),
    Closed(SessionHandle, CloseReason),
    /// Bekleyen niyet sonlandı — YALNIZ §4'teki kapalı nedenlerle. `reason` opak DEĞİLDİR.
    ConnectAbandoned(ConnectTicket, AbandonReason),
    /// Hub yolunun durumu (UI için). Eşin canlılığını BİLDİRMEZ.
    ConnectProgress(ConnectTicket, ConnectPhase),
}

/// Bekleyen niyetin sonlanma nedeni — kapalı liste; "zaman" YOKTUR (INV-NO-IDLE-TEARDOWN).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbandonReason {
    /// (a) Kullanıcı/uygulama `cancel` etti.
    Cancelled,
    /// (b) Karşı taraf kapattı.
    PeerClosed,
    /// (c) Anahtar hijyeni.
    KeyHygiene,
    /// (d) Kalıcı yerel hata: peer material yok, sertifika iptal/expire.
    LocalPermanent,
}

/// `ConnectProgress` fazı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectPhase {
    /// Hub yolu ölçülüyor (prob sürüyor).
    Measuring,
    /// Link yok; prob askıda.
    Suspended,
    /// Heartbeat yanıtı 60 s'dir yok (KS-UI-1). Yalnız UI; hiçbir şeyi sonlandırmaz.
    LongOutage,
}

/// §1 K2 / KARAR-1 — idempotent `connect`; olay ÜRETMEZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectResult {
    /// Aktif oturum zaten var.
    Ready(SessionHandle),
    /// Handshake sürüyor (ya da az önce başladı).
    Pending(ConnectTicket),
}

/// Oturum kapanış nedeni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// Atomik cutover (KARAR-2).
    Renewed,
    /// Provisional supersession onaylandı (DR-CONNECT-1 H6): teslim edilmemiş mesajlar `msg_id` ile
    /// yeni oturumda yeniden gönderilir.
    Superseded,
}

/// §1 K2 — opak; yalnız Transport basar. `Closed` sonrası kullanılamaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionHandle(u64);

/// Opak; `Pending` sonucunun, `cancel`'ın ve `ConnectAbandoned`/`ConnectProgress`'in taşıyıcısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectTicket(u64);

/// Opak peer referansı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerRef(u64);

impl SessionHandle {
    /// Yalnız Transport tarafı (ve sahteleri) tutamaç basar.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

impl ConnectTicket {
    /// Yalnız Transport tarafı basar.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

impl PeerRef {
    /// Application peer'ı adlandırır; içerik opak kalır.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

/// §3 — hata kümesi KAPALIDIR ve yalnız YAPISALDIR: Application'ın düzeltebileceği şeyler.
/// Kripto nedeni buraya sızmaz (K9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("unknown session")]
    UnknownSession,
    #[error("session superseded")]
    SessionSuperseded,
    #[error("early unit conflict")]
    EarlyUnitConflict,
    #[error("unit too large")]
    UnitTooLarge,
    #[error("backpressure")]
    Backpressure,
    #[error("deadline expired")]
    DeadlineExpired,
    #[error("not connected")]
    NotConnected,
}

/// §1 — Application'ın gördüğü TEK yüzey. Crypto ve Plane 1 bu trait'in ARKASINDADIR (K1).
pub trait KamaTransport {
    fn capabilities(&self) -> TransportCapabilities;

    /// İdempotent (KARAR-1). `early_unit` yalnız YENİ handshake yaratılırken tüketilir (K6).
    ///
    /// # Errors
    /// `EarlyUnitConflict` (aktif/pending peer'a `Some(_)`), `UnitTooLarge`, `Backpressure`.
    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError>;

    /// `Ok(())` = kuyruğa alındı; teslim DEĞİL (K8).
    ///
    /// # Errors
    /// `UnknownSession`, `SessionSuperseded`, `UnitTooLarge`, `Backpressure`, `DeadlineExpired`.
    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
    ) -> Result<(), TransportError>;

    fn recv(&mut self) -> Option<Delivered>;

    fn poll_event(&mut self) -> Option<SessionEvent>;

    fn close(&mut self, session: SessionHandle);

    /// Bekleyen niyetten TEK çıkış: `ConnectAbandoned(ticket, Cancelled)` üretir; `early_unit`
    /// teslim edilmemiştir.
    fn cancel(&mut self, ticket: ConnectTicket);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Yüzey pini: bir variant eklenir/çıkarılırsa bu eşleşmeler derlenmez (kapalı kümeler).
    #[test]
    fn closed_sets_are_exhaustive() {
        match TransportError::NotConnected {
            TransportError::UnknownSession
            | TransportError::SessionSuperseded
            | TransportError::EarlyUnitConflict
            | TransportError::UnitTooLarge
            | TransportError::Backpressure
            | TransportError::DeadlineExpired
            | TransportError::NotConnected => {}
        }
        match AbandonReason::Cancelled {
            AbandonReason::Cancelled
            | AbandonReason::PeerClosed
            | AbandonReason::KeyHygiene
            | AbandonReason::LocalPermanent => {}
        }
        match ConnectPhase::Measuring {
            ConnectPhase::Measuring | ConnectPhase::Suspended | ConnectPhase::LongOutage => {}
        }
        let (h, t, p) = (
            SessionHandle::from_raw(1),
            ConnectTicket::from_raw(1),
            PeerRef::from_raw(1),
        );
        for ev in [
            SessionEvent::Ready(h, p),
            SessionEvent::RenewRequired(h),
            SessionEvent::Closed(h, CloseReason::Renewed),
            SessionEvent::Closed(h, CloseReason::Superseded),
            SessionEvent::ConnectAbandoned(t, AbandonReason::Cancelled),
            SessionEvent::ConnectProgress(t, ConnectPhase::LongOutage),
        ] {
            match ev {
                SessionEvent::Ready(..)
                | SessionEvent::RenewRequired(_)
                | SessionEvent::Closed(..)
                | SessionEvent::ConnectAbandoned(..)
                | SessionEvent::ConnectProgress(..) => {}
            }
        }
    }

    /// `KamaTransport` dyn-uyumludur (Application onu `Box<dyn KamaTransport>` olarak tutabilir).
    #[test]
    fn trait_is_object_safe() {
        fn _take(_: &mut dyn KamaTransport) {}
    }
}
