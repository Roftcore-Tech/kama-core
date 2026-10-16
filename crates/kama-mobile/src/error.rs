//! [`NodeError`] — `kama_node::NodeError`'ın UniFFI aynası (Kotlin'de `NodeException`) — ve motor
//! hatalarının aynaları. Çeviri tam eşleşmedir (`match`, `_` kolu yok): kaynak tip bir varyant
//! eklerse bu crate DERLENMEZ.
//!
//! Gizlilik: hiçbir varyant metin, dosya baytı, ses, sinyalleşme blob'u ya da anahtar taşımaz.
//! Kurulum hatalarının `reason` alanı kaynak hatanın `Display` metnidir; o metinler `kama-node`'un
//! kuralıyla yalnız sınıf, konum ve neden taşır (`kama_node::NodeError` belgesi).
//!
//! [`ListenerError`] dinleyici yönündedir: Kotlin'in `onEvent` içinde attığı istisna UniFFI'de
//! panik yerine bu hataya çevrilir (`From<UnexpectedUniFFICallbackError>`); düğüm iş parçacığı
//! etkilenmez.

use kama_app as app;
use kama_contracts as contracts;
use kama_node as node;

/// `KamaMobile` çağrılarının hatası — `kama_node::NodeError`'ın aynası. Kotlin'de `NodeException`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum NodeError {
    // ── Kurulum (`KamaMobile.start`) ──
    /// Cihaz provisioning dosyası okunamadı ya da doğrulanamadı (PROVISIONING-FORMAT-v0 §5).
    #[error("provisioning file: {reason}")]
    Config {
        /// Sınıf ve konum (`kama_provision::format::FormatError`).
        reason: String,
    },
    /// Doğrulanmış dosyadan eş dizini kurulamadı.
    #[error("peer directory: {reason}")]
    PeerDirectory {
        /// Neden.
        reason: String,
    },
    /// Bu cihazın `direct_addrs` girdisi yok: direct-mode veri alıcısı bağlanacak adresi bilmiyor.
    #[error("this device has no direct-mode address in its provisioning file")]
    NoDirectAddress,
    /// Veri dizini (ya da alınan dosyalar dizini) oluşturulamadı.
    #[error("data directory: {reason}")]
    DataDir {
        /// G/Ç nedeni.
        reason: String,
    },
    /// Kalıcı depo açılamadı (aynı `data_dir` başka bir düğümde açıksa).
    #[error("store: {reason}")]
    Store {
        /// Neden.
        reason: String,
    },
    /// Transport kurulamadı (UDP bind, çalışma-zamanı).
    #[error("transport: {reason}")]
    Transport {
        /// Neden.
        reason: String,
    },
    /// Transport'un birimi AMP kaydı için çok küçük.
    #[error("AMP limits: {reason}")]
    Limits {
        /// Neden.
        reason: String,
    },
    /// Düğüm iş parçacığı başlatılamadı.
    #[error("node thread could not be started: {reason}")]
    Spawn {
        /// G/Ç nedeni.
        reason: String,
    },
    /// Bu süreçte aynı cihaz kimliğiyle çalışan bir düğüm var (süreç-içi zarf rölesi); önceki
    /// düğüm `shutdown` ile kapanınca aynı kimlik yeniden açılabilir.
    #[error("a node with this device identity is already running in this process")]
    IdentityInUse,

    // ── Çağrı ──
    /// Provisioning dosyasında böyle bir eş adı yok.
    #[error("unknown peer")]
    UnknownPeer,
    /// Bu düğümün hiç vermediği tutamaç.
    #[error("unknown handle")]
    UnknownHandle,
    /// Tutamaç bu düğümce verilmişti ama kapandı; hiçbir zaman başka bir akışa/aktarıma çözülmez.
    #[error("handle is closed")]
    HandleClosed,
    /// Düğüm tutamaç uzayı tükendi; sarma YOK.
    #[error("node handle space is exhausted")]
    HandlesExhausted,
    /// Girdi tek bir AMP kaydına sığmıyor; komut kuyruğa ALINMADI.
    #[error("{what} of {len} bytes exceeds {max} bytes")]
    TooLarge {
        /// Girdinin türü (`"text"`, `"ptt payload"`, `"signaling blob"`).
        what: String,
        /// Verilen uzunluk.
        len: u64,
        /// Bu düğümün limiti.
        max: u64,
    },
    /// Komut kuyruğu dolu (geri basınç); çağrı bloklamadı, komut ALINMADI.
    #[error("node command queue is full")]
    Busy,
    /// Düğüm durdu: `shutdown` çağrıldı (ya da kapanış sürüyor) veya iş parçacığı bitti.
    #[error("node is stopped")]
    Stopped,
    /// Yanıt bekleyen çağrı dinleyicinin içinden (düğüm iş parçacığı) yapıldı ya da dinleyicinin
    /// içinden yapılan `shutdown` başka bir çağrıyı beklemek zorunda kalırdı — kilitlenme yerine
    /// tipli hata.
    #[error("blocking node call from the listener would deadlock")]
    ReentrantCall,
    /// Metin motoru reddetti (ör. `NoSession`: arayüz "Bağlandı" olayını bekler — DUR-K4G-11b).
    #[error("messenger: {reason:?}")]
    Messenger {
        /// Motorun nedeni.
        reason: MessengerError,
    },
    /// PTT motoru reddetti (ör. `NoSession`, `Transport(Backpressure)`).
    #[error("ptt: {reason:?}")]
    Ptt {
        /// Motorun nedeni.
        reason: PttError,
    },
    /// Dosya motoru reddetti (ör. `AlreadySent`).
    #[error("file transfer: {reason:?}")]
    File {
        /// Motorun nedeni.
        reason: FileSendError,
    },
    /// Sinyalleşme motoru reddetti (ör. `NoSession`, `Transport(Backpressure)`).
    #[error("signaling: {reason:?}")]
    Signaling {
        /// Motorun nedeni.
        reason: SignalingError,
    },
    /// Düğüm iş parçacığı panikle bitti.
    #[error("node thread panicked")]
    NodeThreadPanicked,
}

impl From<node::NodeError> for NodeError {
    fn from(error: node::NodeError) -> Self {
        match error {
            node::NodeError::Config(source) => Self::Config {
                reason: source.to_string(),
            },
            node::NodeError::PeerDirectory(source) => Self::PeerDirectory {
                reason: source.to_string(),
            },
            node::NodeError::NoDirectAddress => Self::NoDirectAddress,
            node::NodeError::DataDir(source) => Self::DataDir {
                reason: source.to_string(),
            },
            node::NodeError::Store(source) => Self::Store {
                reason: source.to_string(),
            },
            node::NodeError::Transport(source) => Self::Transport {
                reason: source.to_string(),
            },
            node::NodeError::Limits(source) => Self::Limits {
                reason: source.to_string(),
            },
            node::NodeError::Spawn(source) => Self::Spawn {
                reason: source.to_string(),
            },
            node::NodeError::IdentityInUse => Self::IdentityInUse,
            node::NodeError::UnknownPeer => Self::UnknownPeer,
            node::NodeError::UnknownHandle => Self::UnknownHandle,
            node::NodeError::HandleClosed => Self::HandleClosed,
            node::NodeError::HandlesExhausted => Self::HandlesExhausted,
            node::NodeError::TooLarge { what, len, max } => Self::TooLarge {
                what: what.to_owned(),
                len: wide(len),
                max: wide(max),
            },
            node::NodeError::Busy => Self::Busy,
            node::NodeError::Stopped => Self::Stopped,
            node::NodeError::ReentrantCall => Self::ReentrantCall,
            node::NodeError::Messenger(reason) => Self::Messenger {
                reason: reason.into(),
            },
            node::NodeError::Ptt(reason) => Self::Ptt {
                reason: reason.into(),
            },
            node::NodeError::File(reason) => Self::File {
                reason: reason.into(),
            },
            node::NodeError::Signaling(reason) => Self::Signaling {
                reason: reason.into(),
            },
            node::NodeError::NodeThreadPanicked => Self::NodeThreadPanicked,
        }
    }
}

/// `usize` → `u64` (FFI'da `usize` yok). Desteklenen her hedefte kayıpsızdır; değilse tavana
/// doyar — panik yok.
fn wide(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Dinleyici hatası (Kotlin'de `ListenerException`). Kotlin uygulaması bunu ATMAK ZORUNDA DEĞİLDİR:
/// `onEvent`'in attığı HER istisna UniFFI'de bu değere çevrilir; neden metni taşınmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ListenerError {
    /// Dinleyici istisna attı; olay bu dinleyici için kayboldu, düğüm turu sürer.
    #[error("listener raised an exception")]
    Unexpected,
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ListenerError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        // Neden metni (Kotlin istisnasının metni/yığını) bilinçli olarak atılır: hassas içerik
        // taşıyabilir ve hiçbir yere yazılmaz.
        Self::Unexpected
    }
}

// ── Motor hatalarının aynaları ──────────────────────────────────────────────────────────────────

/// Transport reddi (S1 §3) — `kama_contracts::s1::TransportError`'ın aynası (küme kapalı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TransportError {
    /// Bilinmeyen oturum.
    UnknownSession,
    /// Oturumun yerine yenisi geçti.
    SessionSuperseded,
    /// Erken birim çakışması.
    EarlyUnitConflict,
    /// Birim çok büyük.
    UnitTooLarge,
    /// Kuyruk dolu (geri basınç): çağıran bekler ve aynı girdiyi yeniden verir.
    Backpressure,
    /// Birim deadline'ı geçti.
    DeadlineExpired,
    /// Bağlı değil.
    NotConnected,
}

impl From<contracts::s1::TransportError> for TransportError {
    fn from(error: contracts::s1::TransportError) -> Self {
        match error {
            contracts::s1::TransportError::UnknownSession => Self::UnknownSession,
            contracts::s1::TransportError::SessionSuperseded => Self::SessionSuperseded,
            contracts::s1::TransportError::EarlyUnitConflict => Self::EarlyUnitConflict,
            contracts::s1::TransportError::UnitTooLarge => Self::UnitTooLarge,
            contracts::s1::TransportError::Backpressure => Self::Backpressure,
            contracts::s1::TransportError::DeadlineExpired => Self::DeadlineExpired,
            contracts::s1::TransportError::NotConnected => Self::NotConnected,
        }
    }
}

/// AMP kodlama hatası — `kama_app::codec::EncodeError`'ın aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EncodeError {
    /// Gövde `MAX_BODY_BYTES`'ı aşıyor.
    BodyTooLarge {
        /// Gövde uzunluğu.
        body_len: u64,
        /// Bu limitlerdeki `MAX_BODY_BYTES`.
        max_body_bytes: u64,
    },
    /// `FILE_BEGIN.name` 255 baytı aşıyor.
    NameTooLong {
        /// Adın bayt uzunluğu.
        name_len: u64,
    },
}

impl From<app::codec::EncodeError> for EncodeError {
    fn from(error: app::codec::EncodeError) -> Self {
        match error {
            app::codec::EncodeError::BodyTooLarge {
                body_len,
                max_body_bytes,
            } => Self::BodyTooLarge {
                body_len: wide(body_len),
                max_body_bytes: wide(max_body_bytes),
            },
            app::codec::EncodeError::NameTooLong { name_len } => Self::NameTooLong {
                name_len: wide(name_len),
            },
        }
    }
}

/// Kalıcı depo hatası — `kama_app::store::AppStoreError`'ın aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AppStoreError {
    /// Sayaç uzayı tükendi; sarma YOK.
    Exhausted,
    /// Depo okunamadı, yazılamadı ya da işlem kalıcılaştırılamadı.
    Persist,
    /// Kalıcı kayıt depo düzenine uymuyor.
    Corrupt,
    /// Eşin outbox'ı dolu.
    OutboxFull,
    /// Kayıt outbox kayıt tavanını aşıyor.
    RecordTooLarge {
        /// Reddedilen kaydın boyu (bayt).
        len: u64,
    },
}

impl From<app::store::AppStoreError> for AppStoreError {
    fn from(error: app::store::AppStoreError) -> Self {
        match error {
            app::store::AppStoreError::Exhausted => Self::Exhausted,
            app::store::AppStoreError::Persist => Self::Persist,
            app::store::AppStoreError::Corrupt => Self::Corrupt,
            app::store::AppStoreError::OutboxFull => Self::OutboxFull,
            app::store::AppStoreError::RecordTooLarge { len } => {
                Self::RecordTooLarge { len: wide(len) }
            }
        }
    }
}

/// Metin motoru reddi — `kama_app::messaging::MessengerError`'ın aynası. Hiçbirini motor yeniden
/// denemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MessengerError {
    /// Eşin hazır oturumu yok; metin kuyruğa ALINMADI (DUR-K4G-11b: arayüz `Connected`'ı bekler).
    NoSession,
    /// Eş tablosu dolu ve bu eş izlenmiyor.
    PeerTableFull,
    /// Eşe RECEIPT bekleyen metin tavanı doldu — geri basınç.
    OutboxFull,
    /// En eski RECEIPT'siz `msg_id` ile sıradaki arası dedup penceresine ulaştı — geri basınç.
    UnackedSpanFull {
        /// RECEIPT'i gelmemiş en eski `msg_id`.
        oldest: u32,
    },
    /// Eşin `msg_id` alanı tükendi; sarma YOK.
    MsgIdExhausted,
    /// Metin bir kayda sığmıyor.
    Encode {
        /// Kodlama nedeni.
        reason: EncodeError,
    },
    /// Transport reddetti; kayıt kuyruğa ALINMADI.
    Transport {
        /// Transport'un nedeni.
        reason: TransportError,
    },
    /// Kalıcı depo hatası; kayıt Transport'a verilmedi.
    Store {
        /// Deponun nedeni.
        reason: AppStoreError,
    },
}

impl From<app::messaging::MessengerError> for MessengerError {
    fn from(error: app::messaging::MessengerError) -> Self {
        match error {
            app::messaging::MessengerError::NoSession => Self::NoSession,
            app::messaging::MessengerError::PeerTableFull => Self::PeerTableFull,
            app::messaging::MessengerError::OutboxFull => Self::OutboxFull,
            app::messaging::MessengerError::UnackedSpanFull { oldest } => {
                Self::UnackedSpanFull { oldest }
            }
            app::messaging::MessengerError::MsgIdExhausted => Self::MsgIdExhausted,
            app::messaging::MessengerError::Encode(reason) => Self::Encode {
                reason: reason.into(),
            },
            app::messaging::MessengerError::Transport(reason) => Self::Transport {
                reason: reason.into(),
            },
            app::messaging::MessengerError::Store(reason) => Self::Store {
                reason: reason.into(),
            },
        }
    }
}

/// PTT motoru reddi — `kama_app::ptt::PttError`'ın aynası. Reddedilen kaydı motor yeniden denemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PttError {
    /// Eşin hazır oturumu yok.
    NoSession,
    /// Bu eşe açık bir giden akış var; önce `ptt_close`.
    StreamActive {
        /// Açık akışın tel kimliği.
        stream_id: u32,
    },
    /// Akış açık değil.
    UnknownStream,
    /// Giden akış tablosu dolu.
    StreamTableFull,
    /// Eşin `stream_id` alanı tükendi; sarma YOK.
    StreamIdExhausted,
    /// Ses yükü bir `MEDIA_BUNDLE` kaydına sığmıyor.
    PayloadTooLarge {
        /// Verilen yükün uzunluğu.
        payload_len: u64,
        /// En büyük yük.
        max_payload_len: u64,
    },
    /// Kayıt kodlanamadı.
    Encode {
        /// Kodlama nedeni.
        reason: EncodeError,
    },
    /// Transport reddetti; kayıt kuyruğa ALINMADI.
    Transport {
        /// Transport'un nedeni.
        reason: TransportError,
    },
    /// Kalıcı depo hatası; `PTT_OPEN` gönderilmedi.
    Store {
        /// Deponun nedeni.
        reason: AppStoreError,
    },
}

impl From<app::ptt::PttError> for PttError {
    fn from(error: app::ptt::PttError) -> Self {
        match error {
            app::ptt::PttError::NoSession => Self::NoSession,
            app::ptt::PttError::StreamActive { stream_id } => Self::StreamActive { stream_id },
            app::ptt::PttError::UnknownStream => Self::UnknownStream,
            app::ptt::PttError::StreamTableFull => Self::StreamTableFull,
            app::ptt::PttError::StreamIdExhausted => Self::StreamIdExhausted,
            app::ptt::PttError::PayloadTooLarge {
                payload_len,
                max_payload_len,
            } => Self::PayloadTooLarge {
                payload_len: wide(payload_len),
                max_payload_len: wide(max_payload_len),
            },
            app::ptt::PttError::Encode(reason) => Self::Encode {
                reason: reason.into(),
            },
            app::ptt::PttError::Transport(reason) => Self::Transport {
                reason: reason.into(),
            },
            app::ptt::PttError::Store(reason) => Self::Store {
                reason: reason.into(),
            },
        }
    }
}

/// Dosya motoru reddi — `kama_app::file_transfer::FileSendError`'ın aynası. Her hata yolunda durum
/// değişmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FileSendError {
    /// Eşin hazır oturumu yok; dosya kuyruğa ALINMADI.
    NoSession,
    /// Giden aktarım yuvaları dolu — geri basınç.
    QueueFull,
    /// Eşin `transfer_id` alanı tükendi; sarma YOK.
    TransferIdExhausted,
    /// Dosya alıcının birleştirme tavanını aşıyor.
    TooLarge {
        /// Dosya boyu (bayt; taşmada `u64::MAX`).
        file_size: u64,
    },
    /// Transport'un birimi parça yükü taşımıyor.
    NoChunkRoom,
    /// `FILE_BEGIN` bir kayda sığmıyor.
    Encode {
        /// Kodlama nedeni.
        reason: EncodeError,
    },
    /// Kalıcı depo `transfer_id` ayıramadı.
    Store {
        /// Deponun nedeni.
        reason: AppStoreError,
    },
    /// İzlenen giden aktarım değil.
    UnknownTransfer,
    /// Son kayıt (`FILE_END`) kabul edildi: iptal edilecek bir şey kalmadı; alındı yine beklenir.
    AlreadySent,
}

impl From<app::file_transfer::FileSendError> for FileSendError {
    fn from(error: app::file_transfer::FileSendError) -> Self {
        match error {
            app::file_transfer::FileSendError::NoSession => Self::NoSession,
            app::file_transfer::FileSendError::QueueFull => Self::QueueFull,
            app::file_transfer::FileSendError::TransferIdExhausted => Self::TransferIdExhausted,
            app::file_transfer::FileSendError::TooLarge { file_size } => {
                Self::TooLarge { file_size }
            }
            app::file_transfer::FileSendError::NoChunkRoom => Self::NoChunkRoom,
            app::file_transfer::FileSendError::Encode(reason) => Self::Encode {
                reason: reason.into(),
            },
            app::file_transfer::FileSendError::Store(reason) => Self::Store {
                reason: reason.into(),
            },
            app::file_transfer::FileSendError::UnknownTransfer => Self::UnknownTransfer,
            app::file_transfer::FileSendError::AlreadySent => Self::AlreadySent,
        }
    }
}

/// Sinyalleşme motoru reddi — `kama_app::signaling::SignalingError`'ın aynası. Reddedilen blob'u
/// motor yeniden denemez: çağıran bekler ve aynı blob'u yeniden verir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignalingError {
    /// Blob bir `SIGNALING_BLOB` kaydına sığmıyor.
    TooLarge {
        /// Verilen blob'un uzunluğu.
        blob_len: u64,
        /// En büyük blob.
        max_blob_len: u64,
    },
    /// Eşin hazır oturumu yok; blob kuyruğa ALINMADI.
    NoSession,
    /// Motorun kimlik alanı tükendi; sarma YOK.
    IdExhausted,
    /// Kayıt kodlanamadı.
    Encode {
        /// Kodlama nedeni.
        reason: EncodeError,
    },
    /// Transport reddetti; kayıt kuyruğa ALINMADI.
    Transport {
        /// Transport'un nedeni.
        reason: TransportError,
    },
}

impl From<app::signaling::SignalingError> for SignalingError {
    fn from(error: app::signaling::SignalingError) -> Self {
        match error {
            app::signaling::SignalingError::TooLarge {
                blob_len,
                max_blob_len,
            } => Self::TooLarge {
                blob_len: wide(blob_len),
                max_blob_len: wide(max_blob_len),
            },
            app::signaling::SignalingError::NoSession => Self::NoSession,
            app::signaling::SignalingError::IdExhausted => Self::IdExhausted,
            app::signaling::SignalingError::Encode(reason) => Self::Encode {
                reason: reason.into(),
            },
            app::signaling::SignalingError::Transport(reason) => Self::Transport {
                reason: reason.into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_errors_keep_their_typed_cause() {
        let no_session = node::NodeError::Messenger(app::messaging::MessengerError::NoSession);
        assert_eq!(
            NodeError::from(no_session),
            NodeError::Messenger {
                reason: MessengerError::NoSession
            }
        );
        let rejected = app::ptt::PttError::Transport(contracts::s1::TransportError::Backpressure);
        assert_eq!(
            NodeError::from(node::NodeError::Ptt(rejected)),
            NodeError::Ptt {
                reason: PttError::Transport {
                    reason: TransportError::Backpressure
                }
            }
        );
    }

    #[test]
    fn too_large_keeps_kind_and_lengths() {
        const LEN: usize = 2000;
        const MAX: usize = 1002;
        let too_large = node::NodeError::TooLarge {
            what: "text",
            len: LEN,
            max: MAX,
        };
        assert_eq!(
            NodeError::from(too_large),
            NodeError::TooLarge {
                what: "text".to_owned(),
                len: wide(LEN),
                max: wide(MAX),
            }
        );
    }

    #[test]
    fn a_listener_exception_becomes_a_typed_error_without_its_text() {
        let exception = uniffi::UnexpectedUniFFICallbackError::new("kotlin: gizli-mesaj");
        let error = ListenerError::from(exception);
        assert_eq!(error, ListenerError::Unexpected);
        assert!(!format!("{error:?} {error}").contains("gizli"));
    }
}
