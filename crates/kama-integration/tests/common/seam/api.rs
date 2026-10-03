//! KM-SEAM-AT-001 §1/§3 — App↔Transport arayüzünün **test-yerel aynası**.
//!
//! # Bu dosya prodüksiyon API'si DEĞİLDİR
//!
//! `KamaTransport` kama-core'un prodüksiyon kodunda henüz YOK. Sözleşme iki imzalı
//! (KM-SEAM-AT-001 başlık: "ikisi imzalamadan sözleşme değildir") ve §1 bloğunda dört kalem
//! henüz kimsenin kararı değil (aşağıda `SEAM-OPEN-1..4`). Bu tipleri bugün `client-core`'un public
//! yüzeyine indirmek, imzasız bir kararı tek taraflı dondurmak olurdu.
//!
//! Bunun yerine: ayna burada durur, sözleşme süiti buna karşı derlenir ve
//! `scripts/ci/seam-api-drift.py` **spec §1/§3 ile bu dosyayı karşılaştırır** — ikisi ayrışırsa
//! CI kırmızı. Ayna "sessizce eskiyen ikinci hakikat" olamaz; olursa kapı söyler.
//!
//! Prodüksiyon trait'i indiğinde bu dosya SİLİNİR ve süit gerçek tipi kullanır; o iş bu görevin
//! kapsamı değildir (KM-SEAM-AT-001 §8 "§6 harness + §7 stub'lar" kaleminin yalnız iskeleti).
//!
//! # Açık kalemler (spec'in çözmediği, tek satırda takas edilebilir tutulanlar)
//!
//! | # | spec'te | burada | neden karar bekliyor |
//! |---|---|---|---|
//! | SEAM-OPEN-1 | `bytes: Bytes` | [`UnitBytes`] = `Vec<u8>` | `bytes` 1.12.1 lock'ta var ama hiçbir crate doğrudan kullanmıyor; doğrudan dep ekleme kararı Hayri'yi de bağlar |
//! | SEAM-OPEN-2 | `deadline: Instant` | [`Deadline`] = [`LogicalTick`] | ADR-17: süit deterministik olmalı; `std::time::Instant` duvar saati sokar. Repoda `adaptive_transport::clock::Clock` soyutlaması zaten var |
//! | SEAM-OPEN-3 | `CloseReason` | `Renewed`, `Superseded` | spec tipi anıyor ama variant kümesini KAPATMIYOR (§1: `Renewed` KARAR-2, `Superseded` DR-CONNECT-1 H6, "…") |
//! | SEAM-OPEN-4 | — | crate yeri | `client-core` kendi doc'unda "henüz ürün crate'i değil" diyor (`lib.rs:21-33`) |
//!
//! Not: `crates/bench/src/transport.rs:296` içinde AYNI ADI taşıyan ilgisiz bir bench tipi var
//! (`pub struct KamaTransport`). Çakışma yok — kama-integration bench'e bağımlı değil — ama
//! grep'le arayan biri ikisini karıştırmasın diye kayda geçiriliyor.

#![allow(dead_code)]

/// SEAM-OPEN-1 — sınırdan geçen birim byte'ları (AMP `AppMessageRecord`; ccr/AMP §2).
///
/// Sözleşme byte-özdeşlik ister (§2), sahiplik/kopyalama modelini istemez. `Vec<u8>` o yüzden
/// yeterli; `Bytes`'a geçiş bu tek satırdır.
pub type UnitBytes = Vec<u8>;

/// SEAM-OPEN-2 — deadline'ın deterministik zaman birimi (ADR-17; duvar saati YOK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LogicalTick(pub u64);

/// SEAM-OPEN-2 — spec'teki `Instant`'ın süitteki karşılığı.
pub type Deadline = LogicalTick;

/// §1 K5 — salt-okunur; değerleri Transport türetir, Application hesaplamaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportCapabilities {
    pub max_plaintext_unit_bytes: u32,
    pub preferred_plaintext_unit_bytes: u32,
}

/// §1 K3 — `bytes` bir AMP `AppMessageRecord`'dur; Transport onu semantik olarak ASLA işlemez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUnit {
    pub bytes: UnitBytes,
    pub compression_policy: CompressionPolicy,
}

/// §1 K4 — sıkıştırma Application'da; bu alan Transport için BİLGİLENDİRİCİDİR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionPolicy {
    Never,
    AppCompressed,
}

/// §4 — Application beyan eder, Transport uygular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryPolicy {
    Reliable,
    DeadlineBounded { deadline: Deadline },
    BestEffort,
}

/// §1 — `recv()` çıktısı; `unit` gönderilen record ile byte-özdeş (§2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    pub session: SessionHandle,
    pub unit: UnitBytes,
}

/// §1 K7 — olaylar typed; Transport yorumlamadan iletir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Ready(SessionHandle, PeerRef),
    RenewRequired(SessionHandle),
    Closed(SessionHandle, CloseReason),
    /// DR-CONNECT-1 H9 — yalnız §4'teki kapalı sonlandırma listesiyle; `ConnectFailed` KALKTI
    /// (connect için deadline yok).
    ConnectAbandoned(ConnectTicket, AbandonReason),
    /// DR-CONNECT-1 H9 — HUB YOLUNUN durumu; eşin canlılığı DEĞİL.
    ConnectProgress(ConnectTicket, ConnectPhase),
}

/// DR-CONNECT-1 H2/H9 — sonlandırma nedenleri (a)–(d); OPAK DEĞİL (§3: yerel yaşam-döngüsü
/// durumu, kripto oracle'ı değil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbandonReason {
    Cancelled,
    PeerClosed,
    KeyHygiene,
    LocalPermanent,
}

/// DR-CONNECT-1 H9 — `ConnectProgress` fazı (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectPhase {
    Measuring,
    Suspended,
    LongOutage,
}

/// §1 K2 / KARAR-1 — idempotent `connect`; olay ÜRETMEZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectResult {
    Ready(SessionHandle),
    Pending(ConnectTicket),
}

/// SEAM-OPEN-3 — spec iki variant adlandırıyor: `Renewed` (§K2 atomik cutover) ve `Superseded`
/// (DR-CONNECT-1 H6 peer-restart supersession); küme spec'te kapalı ilan edilmedi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    Renewed,
    Superseded,
}

/// §1 K2 — opak; yalnız Transport basar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionHandle(u64);

/// §1 — opak; `Pending` sonucunun, `cancel`'ın ve `ConnectAbandoned`/`ConnectProgress`'in taşıyıcısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectTicket(u64);

/// §1 — opak peer referansı (`StaticPeerMaterialProvider` anahtarı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerRef(u64);

impl SessionHandle {
    /// Yalnız hedef implementasyonları (Transport tarafı) handle basar — §1 K2.
    pub fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

impl ConnectTicket {
    /// Yalnız Transport tarafı basar.
    pub fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

impl PeerRef {
    /// Application peer'ı adlandırır; içerik opak kalır.
    pub fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

/// §3 — hata kümesi KAPALIDIR. Yeni variant eklemek API kararıdır, harness kararı değil;
/// stub'ların "henüz uygulanmadı" durumu bu kümeye SIZDIRILMAZ (bkz. `target::TargetUnavailable`).
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

    /// # Errors
    /// §3 typed hata kümesi (`EarlyUnitConflict` K6, `Backpressure`, …).
    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError>;

    /// # Errors
    /// §3 typed hata kümesi (`UnknownSession` K7, `SessionSuperseded` KARAR-2, `UnitTooLarge` K5…).
    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
    ) -> Result<(), TransportError>;

    fn recv(&mut self) -> Option<Delivered>;

    fn poll_event(&mut self) -> Option<SessionEvent>;

    fn close(&mut self, session: SessionHandle);

    /// DR-CONNECT-1 H9 — bekleyen niyetten TEK çıkış; `ConnectAbandoned(ticket, Cancelled)` üretir.
    fn cancel(&mut self, ticket: ConnectTicket);
}
