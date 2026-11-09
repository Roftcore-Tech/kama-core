//! S1 — Application ↔ Transport seam'i (`docs/seams/KM-SEAM-AT-001.md` §1 trait bloğu + §3 hata
//! kümesi, DR-CONNECT-1 sonrası hâliyle).
//!
//! # Katman kuralı (INV-NEIGHBOR-1)
//!
//! Application yalnız [`KamaTransport`]'u çağırır. Crypto'ya ve Plane 1'e (hub-RPC) erişimi YOKTUR;
//! `kama-app`'in bağımlılık grafiğinde `kama-contracts` dışında hiçbir katman crate'i bulunamaz
//! (`scripts/ci/layer_check.py`). Application **retry yapmaz** (K8): `send` başarısızsa kullanıcıya
//! yansıtır ya da bekler.
//!
//! # `connect()` — kalıcı niyet (DR-CONNECT-1)
//!
//! `connect` için deadline YOKTUR. [`ConnectResult::Pending`] yalnız üç yolla biter:
//! [`SessionEvent::Ready`], [`KamaTransport::cancel`] ya da [`SessionEvent::ConnectAbandoned`].
//! [`SessionEvent::ConnectProgress`] HUB YOLUNUN durumudur; eşin canlılığı yalnız AMP
//! `RECEIPT(delivered)` ile bilinir.
//!
//! # Spec'ten bilinçli fark
//!
//! `bytes: Bytes` yerine [`UnitBytes`] = `Vec<u8>` (`bytes` crate'i bağımlılık yapılmadı; bu
//! crate'in harici bağımlılığı yoktur). Sözleşme byte-özdeşlik ister, sahiplik modelini değil.
//!
//! Aynı adı taşıyan [`crate::s3::SessionEvent`] FARKLI bir tiptir: Crypto'nun Transport'a verdiği
//! olaydır (`Ready(h)` eş taşımaz; `Confirmed` yalnız oradadır ve Transport onu buradaki
//! [`SessionEvent::Ready`]'ye çevirir).

use core::fmt;
use std::time::Instant;

use crate::handles::{CloseReason, PeerRef, SessionHandle};

/// Sınırdan geçen birim byte'ları: bir AMP `AppMessageRecord` (application-message-protocol §2).
pub type UnitBytes = Vec<u8>;

/// §1 K5 — salt-okunur; değerleri Transport türetir, Application HESAPLAMAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportCapabilities {
    pub max_plaintext_unit_bytes: u32,
    pub preferred_plaintext_unit_bytes: u32,
    /// [2026-10-06 F0.2(iv)] `early_unit` tavanı (K6): Crypto `conservative_limits`'ten INIT zarfı
    /// için türetilir. Aşan `early_unit` → `connect()` `UnitTooLarge`.
    pub max_early_unit_bytes: u32,
    /// [2026-10-06 F0.2(iv)] Ürün builder'ı `false`'u reddeder. Test yapılandırmasında recovery
    /// OFF iken `DeliveryPolicy::Reliable` best-effort'tur ve `reliable = false` ile BEYAN edilir.
    pub reliable: bool,
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

/// [2026-11-09 S1 v2 — EK-1 D-08/D-09] Trafik sınıfı: Application'ın beyan ettiği POLİTİKA
/// etiketi (K3: tip bilgisi değil). Transport sınıfı yalnız sunum sırası ve penceresi için okur;
/// birim baytını ve kayıt türünü OKUMAZ (INV-ENGINE-SOC, D-04 notu). Telde karşılığı YOKTUR (tel ve
/// altın vektörler değişmez). Küme kapalıdır; [`TrafficClass::code`] D-08 numarasıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TrafficClass {
    /// BE 0 — sıradan veri (TEXT); `connect(early_unit)` da bu sınıftır (D-09).
    Be,
    /// CTRL 1 — küçük denetim kayıtları (alındılar).
    Ctrl,
    /// SIG 2 — sinyalleşme.
    Sig,
    /// BK 3 — toplu aktarım (dosya).
    Bk,
    /// VO 4 — canlı ses (gösterimde kullanılmaz; Aralık T1-5).
    Vo,
    /// VO_REL 5 — güvenilir ses (PTT; D-10 B).
    VoRel,
}

impl TrafficClass {
    /// Kapalı küme, D-08 sırasıyla.
    pub const ALL: [Self; 6] = [
        Self::Be,
        Self::Ctrl,
        Self::Sig,
        Self::Bk,
        Self::Vo,
        Self::VoRel,
    ];

    /// EK-1 D-08 numarası. Telde taşınmaz (sınıf öneki T1-2b).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Be => 0,
            Self::Ctrl => 1,
            Self::Sig => 2,
            Self::Bk => 3,
            Self::Vo => 4,
            Self::VoRel => 5,
        }
    }
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

/// §1 K2 / KARAR-1 — idempotent `connect`: TEKRAR eden `connect` olay üretmez (duplicate
/// Ready/Measuring yok); yeni niyetin `Pending(ticket)`'ı F0.2(ii) gereği
/// `ConnectProgress(ticket, Measuring)` üretir. [2026-10-06]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectResult {
    /// Aktif oturum zaten var.
    Ready(SessionHandle),
    /// Handshake sürüyor (ya da az önce başladı).
    Pending(ConnectTicket),
}

/// Opak; `Pending` sonucunun, `cancel`'ın ve `ConnectAbandoned`/`ConnectProgress`'in taşıyıcısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectTicket(u64);

impl ConnectTicket {
    /// Yalnız Transport tarafı (ve sahteleri) basar.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

/// §3 — hata kümesi KAPALIDIR ve yalnız YAPISALDIR: Application'ın düzeltebileceği şeyler.
/// Kripto nedeni buraya sızmaz (K9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    UnknownSession,
    SessionSuperseded,
    EarlyUnitConflict,
    UnitTooLarge,
    /// Kuyruk dolu ya da pending INIT tavanı dolu (§3).
    Backpressure,
    /// BİRİM deadline'ı (`DeliveryPolicy::DeadlineBounded`); bağlantı deadline'ı değil (§3).
    DeadlineExpired,
    NotConnected,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownSession => "unknown session",
            Self::SessionSuperseded => "session superseded",
            Self::EarlyUnitConflict => "early unit conflict",
            Self::UnitTooLarge => "unit too large",
            Self::Backpressure => "backpressure",
            Self::DeadlineExpired => "deadline expired",
            Self::NotConnected => "not connected",
        })
    }
}

impl std::error::Error for TransportError {}

/// §1 — Application'ın gördüğü TEK yüzey. Crypto ve Plane 1 bu trait'in ARKASINDADIR (K1).
pub trait KamaTransport {
    fn capabilities(&self) -> TransportCapabilities;

    /// İdempotent (KARAR-1). `early_unit` yalnız YENİ handshake yaratılırken tüketilir (K6).
    ///
    /// [2026-10-06 F0.2(ii)] `Pending(ticket)` döndüğü anda `ConnectProgress(ticket, Measuring)`
    /// kuyruğa girer; faz değişince yeni olay; aynı faz tekrar yayınlanmaz (§4).
    ///
    /// # Errors
    /// `EarlyUnitConflict` (aktif/pending peer'a `Some(_)`), `UnitTooLarge` (`bytes.len() >`
    /// `max_early_unit_bytes`), `Backpressure`.
    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError>;

    /// `Ok(())` = kuyruğa alındı; teslim DEĞİL (K8).
    ///
    /// [2026-10-06 F0.2(i)] Eski tutamaç: `Ready(h')` ile `Closed(h)` olayının `poll_event` ile
    /// OKUNMASI arasında `send(h)` → `SessionSuperseded`; `Closed(h)` okunduktan sonra →
    /// `UnknownSession`. Application'ın `close(h)` çağırması GEREKMEZ (§3).
    ///
    /// [2026-11-09 S1 v2 — D-09] `class` politika etiketidir ([`TrafficClass`]); Transport onu
    /// yalnız sunum sırası/penceresi için okur. Aynı oturum ve aynı sınıf içinde `Reliable`
    /// birimler gönderim sırasıyla teslim edilir (§4); sınıflar arası `send` sırası korunmayabilir.
    ///
    /// # Errors
    /// `UnknownSession`, `SessionSuperseded`, `UnitTooLarge`, `Backpressure`, `DeadlineExpired`.
    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
        class: TrafficClass,
    ) -> Result<(), TransportError>;

    fn recv(&mut self) -> Option<Delivered>;

    fn poll_event(&mut self) -> Option<SessionEvent>;

    /// [2026-10-06 F07.1] Senkron döner. `Ok(())` ⟺ tam BİR `Closed(h, UserClosed)` olayı
    /// kuyruğa girer; her oturum sonu tam bir `Closed` olayı (F0.2(iii), §4).
    ///
    /// # Errors
    /// `UnknownSession` — bilinmeyen ya da zaten kapatılmış (`Closed`'ı kuyrukta ya da okunmuş)
    /// tutamaç; olay YOK. `SessionSuperseded` — `Ready(h')` ile `Closed(h, Superseded)`'ın
    /// `poll_event` ile okunması arası (aynı pencerede `send(h)` ile aynı hata); olay YOK, durum
    /// değişmez (`Closed(h, Superseded)` zaten kuyruktadır).
    fn close(&mut self, session: SessionHandle) -> Result<(), TransportError>;

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
            PeerRef::from_dest_hash([1; 32]),
        );
        for ev in [
            SessionEvent::Ready(h, p),
            SessionEvent::RenewRequired(h),
            SessionEvent::Closed(h, CloseReason::Superseded),
            SessionEvent::Closed(h, CloseReason::UserClosed),
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

    /// [S1 v2] `TrafficClass` kapalı kümesi ve D-08 numaraları pinli: variant eklenirse eşleşme
    /// derlenmez; numara değişirse test kırmızı.
    #[test]
    fn traffic_class_set_is_closed_and_numbered_by_d08() {
        for class in TrafficClass::ALL {
            match class {
                TrafficClass::Be
                | TrafficClass::Ctrl
                | TrafficClass::Sig
                | TrafficClass::Bk
                | TrafficClass::Vo
                | TrafficClass::VoRel => {}
            }
        }
        let codes: Vec<u8> = TrafficClass::ALL.iter().map(|c| c.code()).collect();
        assert_eq!(codes, [0, 1, 2, 3, 4, 5]);
    }

    /// `KamaTransport` dyn-uyumludur (Application onu `Box<dyn KamaTransport>` olarak tutabilir).
    #[test]
    fn trait_is_object_safe() {
        fn _take(_: &mut dyn KamaTransport) {}
    }

    /// `thiserror` olmadan da hata tipi `std::error::Error`'dır ve mesajları kararlıdır.
    #[test]
    fn transport_error_is_a_std_error_with_stable_messages() {
        fn assert_error<E: std::error::Error + Send + Sync + 'static>(_: &E) {}
        let all = [
            (TransportError::UnknownSession, "unknown session"),
            (TransportError::SessionSuperseded, "session superseded"),
            (TransportError::EarlyUnitConflict, "early unit conflict"),
            (TransportError::UnitTooLarge, "unit too large"),
            (TransportError::Backpressure, "backpressure"),
            (TransportError::DeadlineExpired, "deadline expired"),
            (TransportError::NotConnected, "not connected"),
        ];
        for (e, msg) in all {
            assert_error(&e);
            assert_eq!(e.to_string(), msg);
        }
    }
}
