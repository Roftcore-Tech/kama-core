//! Application ↔ Transport seam (S1). Kontrat: `docs/seems/KM-SEAM-AT-001.md`.
//!
//! `KamaTransport`, Application'ın gördüğü TEK API'dir (INV-NEIGHBOR-1). Application bu
//! trait'in arkasında Crypto'yu, Plane 1'i, ARQ/FEC/pacing'i asla görmez.

use std::time::Instant;

/// Transport'un bu an için sağladığı boyut sınırları. Değerler Transport'tan TÜRETİLİR;
/// Application hesaplamaz (K5) — 18 B E2E kayıt ek yükü / 4 B hub_handle gibi sabitler
/// Crypto/Transport'un, Application'a asla sızmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportCapabilities {
    pub max_plaintext_unit_bytes: u32,
    pub preferred_plaintext_unit_bytes: u32,
}

/// Sınırdan geçen tek byte türü: bir `AppMessageRecord` (AMP §2). Transport bunu yapısal
/// işler (uzunluk/segment/batch/FEC), semantik olarak asla (msg_type okumaz — K3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUnit {
    pub bytes: Vec<u8>,
    pub compression_policy: CompressionPolicy,
}

/// Transport için yalnızca bilgilendirici (K4) — davranışı değiştirmez, sıkıştırma/açma
/// Application'da yapılır ve `bytes` içinde zaten işaretlenmiştir (AMP §5 flags.bit0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionPolicy {
    Never,
    AppCompressed,
}

/// Application'ın beyan ettiği, Transport'un uyguladığı teslim politikası (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryPolicy {
    Reliable,
    DeadlineBounded { deadline: Instant },
    BestEffort,
}

/// `recv()`'ten dönen teslim edilmiş birim; `unit` gönderilen `PreparedUnit.bytes` ile
/// byte-identical olmalıdır (H-ROUNDTRIP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    pub session: SessionHandle,
    pub unit: Vec<u8>,
}

/// Bir session için garanti edilen olay sırası (K7): `Ready → (RenewRequired)* → Closed`.
/// `ConnectFailed` bir session'a değil bekleyen bir `connect()` çağrısına aittir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Ready(SessionHandle, PeerRef),
    RenewRequired(SessionHandle),
    Closed(SessionHandle, CloseReason),
    ConnectFailed(ConnectTicket),
}

/// `Closed` olayının sebebi. `Renewed`, atomik cutover sonrası eski handle'ın kapanışıdır
/// (H-EVENTS: `Closed(h, Renewed)`). Yeni sebepler eklenebilir (minor sürüm — §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CloseReason {
    Renewed,
    LocalClose,
    PeerClose,
    Error,
}

/// `connect()` KARAR-1 gereği idempotenttir: aktif session varsa `Ready(h)` döner ve
/// olay YAYINLAMAZ; pending varsa aynı `Pending(ticket)`; hiç yoksa yeni handshake başlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectResult {
    Ready(SessionHandle),
    Pending(ConnectTicket),
}

/// Opak session tanıtıcısı. Application yalnız taşır/karşılaştırır; içini yorumlamaz.
/// `from_raw`/`raw` yalnız Transport implementasyonları (ve test mock'ları) içindir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionHandle(u64);

impl SessionHandle {
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Bekleyen bir `connect()` çağrısını tanımlayan opak bilet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectTicket(u64);

impl ConnectTicket {
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Opak peer referansı (StaticPeerMaterialProvider anahtarı). Application içeriğini
/// yorumlamaz, yalnız `connect()`'e geçirir.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerRef(Vec<u8>);

impl PeerRef {
    pub fn from_bytes(id: impl Into<Vec<u8>>) -> Self {
        Self(id.into())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Application'ın düzeltebileceği yapısal hatalar (K9). Kripto/route/hub sebepleri buraya
/// asla sızmaz — bağlantı kurma başarısızlıkları tek tip `ConnectFailed` ile gelir (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("unknown session")]
    UnknownSession,
    #[error("session superseded by cutover")]
    SessionSuperseded,
    #[error("early_unit conflicts with an active or pending connect")]
    EarlyUnitConflict,
    #[error("unit exceeds max_plaintext_unit_bytes")]
    UnitTooLarge,
    #[error("transport backpressure")]
    Backpressure,
    #[error("delivery deadline expired")]
    DeadlineExpired,
    #[error("not connected")]
    NotConnected,
}

/// Application'ın gördüğü tek alt komşu (INV-NEIGHBOR-1). Kontrat: KM-SEAM-AT-001 §1.
pub trait KamaTransport {
    fn capabilities(&self) -> TransportCapabilities;

    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError>;

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
    ) -> Result<(), TransportError>;

    /// Poll; sıralama garantisi §4 (aynı session içinde Reliable birimler gönderim sırasında).
    fn recv(&mut self) -> Option<Delivered>;

    /// Poll; Ready | RenewRequired | Closed | ConnectFailed.
    fn poll_event(&mut self) -> Option<SessionEvent>;

    fn close(&mut self, session: SessionHandle);
}
