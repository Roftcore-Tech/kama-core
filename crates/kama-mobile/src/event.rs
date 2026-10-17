//! [`EngineEvent`] ve yüzeyin değer tipleri — `kama_node`'un olay, ölçü ve tutamaç tiplerinin
//! UniFFI aynası (plan K4G-11 §2 listesi). Her çeviri tam eşleşmedir (`match` / yapı açımı, `..`
//! yok): `kama-node`, `kama-app` ya da `kama-contracts` bir varyant ya da alan eklerse bu crate
//! DERLENMEZ ve ayna aynı değişiklikte güncellenir. `kama-app` / `kama-contracts` tipleri yalnız
//! tam yolla ya da fonksiyon içinde tip takma adıyla (`Source`, `Target`) anılır (C9).
//!
//! **Gizlilik:** [`EngineEvent`]'in `Debug` çıktısı metni, ses yükünü ve sinyalleşme blob'unu değil
//! uzunluklarını yazar (`kama_node::NodeEvent` ile aynı kural). Kotlin'in `toString()`'i bu
//! `Debug`'a bağlıdır (`#[uniffi::export(Debug)]`: UniFFI her varyantın veri sınıfında
//! `toString()`'i Rust `Debug`'ını çağıracak biçimde üretir); Kotlin'de alanlara doğrudan erişim
//! (`ev.text`, `ev.payload`) yine içeriğin kendisidir — onu loglamak çağıranın kararıdır.

use std::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;

use kama_node as node;

use crate::error::{FileSendError, TransportError};

/// [`crate::EngineListener::on_event`] çıktısı — `kama_node::NodeEvent`'in aynası. Eşler
/// provisioning adıyla anılır (`peer_id`); giden akış ve aktarımlar düğüm tutamacıyla
/// ([`StreamHandle`], [`FileHandle`]), gelenler eşin tel kimliğiyle (`stream_id`, `transfer_id`).
///
/// Kotlin `toString()` Rust `Debug`'ıdır: metin, ses yükü ve blob yerine uzunlukları yazar.
#[derive(Clone, PartialEq, uniffi::Enum)]
#[uniffi::export(Debug)]
pub enum EngineEvent {
    /// Eşe bekleyen bağlantı niyetinin fazı (hub yolunun durumudur, eşin canlılığını BİLDİRMEZ).
    /// Direct-mode'da yalnız `Measuring` üretilir.
    ConnectProgress {
        /// Eş.
        peer_id: String,
        /// Faz.
        phase: ConnectPhase,
    },
    /// Eşle hazır oturum var — arayüzün "Bağlandı" olayı (DUR-K4G-11b: metin bundan sonra
    /// gönderilir). Supersession'ın yeni oturumu yeni olay üretmez.
    Connected {
        /// Eş.
        peer_id: String,
    },
    /// Eşin güncel oturumu kalmadı ya da bekleyen bağlantı niyeti bitti.
    Closed {
        /// Eş.
        peer_id: String,
        /// Neden.
        reason: CloseCause,
    },
    /// Eşten yeni metin (`RECEIPT(delivered)` gitti).
    TextReceived {
        /// Gönderen eş.
        peer_id: String,
        /// Gönderenin eş başına `msg_id`'si.
        msg_id: u32,
        /// UTF-8 metin.
        text: String,
    },
    /// Eş, `send_text`'in döndürdüğü `msg_id`'yi işlediğini bildirdi — `(peer_id, msg_id)` başına
    /// tam bir kez ("teslim edildi").
    Receipt {
        /// Eş.
        peer_id: String,
        /// Teslim edilen metnin `msg_id`'si.
        msg_id: u32,
    },
    /// Giden aktarımın kilometre taşı.
    FileProgress {
        /// `send_file`'ın döndürdüğü tutamaç.
        file: FileHandle,
        /// Alıcı eş.
        peer_id: String,
        /// Aşama.
        stage: FileStage,
    },
    /// Gelen dosya tamamlandı, özet tuttu ve diske yazıldı; yol yalnız bu olayla verilir (D8).
    FileReceived {
        /// Gönderen eş.
        peer_id: String,
        /// Gönderenin `transfer_id`'si.
        transfer_id: u64,
        /// `<data_dir>/files/<eş>-<transfer_id>` (çakışmada `-<n>` eki).
        path: String,
        /// Bayt.
        size: u64,
    },
    /// Aktarım başarısız bitti (gelen ya da giden).
    FileFailed {
        /// Aktarım.
        file: FileRef,
        /// Karşı eş.
        peer_id: String,
        /// Neden.
        reason: FileFailReason,
    },
    /// Eş konuşmaya başladı (yeni `stream_id`'li `PTT_OPEN`).
    PttOpened {
        /// Konuşan eş.
        peer_id: String,
        /// Eşin akış kimliği.
        stream_id: u32,
        /// Ses kodlayıcısı (akış boyunca sabit).
        codec: PttCodec,
        /// Demet süresi (ms; akış boyunca sabit, düğüm yorumlamaz).
        bundle_ms: u8,
    },
    /// Eşin etkin akışından ses demeti, teslim sırasıyla (Reliable; geç gelen düşmez).
    PttBundle {
        /// Konuşan eş.
        peer_id: String,
        /// Eşin akış kimliği.
        stream_id: u32,
        /// `MEDIA_BUNDLE.payload`, aynen (opak).
        payload: Vec<u8>,
    },
    /// Eşin akışı bitti (`PTT_CLOSE`, hareketsizlik ya da oturum sonu).
    PttClosed {
        /// Konuşan eş.
        peer_id: String,
        /// Eşin akış kimliği.
        stream_id: u32,
    },
    /// Eşten sinyalleşme blob'u (`SIGNALING_BLOB`) — alındısız ve dedupsuz; aynı blob birden çok
    /// kez gelebilir.
    SignalingReceived {
        /// Gönderen eş.
        peer_id: String,
        /// Blob, aynen (opak; medya motorunun).
        bytes: Vec<u8>,
    },
    /// Periyodik ölçü anlık görüntüsü (saniyede bir).
    Metrics {
        /// Ölçüler.
        metrics: NodeMetrics,
    },
}

/// Metin, ses yükü ve sinyalleşme blob'u YAZILMAZ; yalnız uzunlukları.
impl fmt::Debug for EngineEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectProgress { peer_id, phase } => f
                .debug_struct("ConnectProgress")
                .field("peer_id", peer_id)
                .field("phase", phase)
                .finish(),
            Self::Connected { peer_id } => f
                .debug_struct("Connected")
                .field("peer_id", peer_id)
                .finish(),
            Self::Closed { peer_id, reason } => f
                .debug_struct("Closed")
                .field("peer_id", peer_id)
                .field("reason", reason)
                .finish(),
            Self::TextReceived {
                peer_id,
                msg_id,
                text,
            } => f
                .debug_struct("TextReceived")
                .field("peer_id", peer_id)
                .field("msg_id", msg_id)
                .field("text_len", &text.len())
                .finish_non_exhaustive(),
            Self::Receipt { peer_id, msg_id } => f
                .debug_struct("Receipt")
                .field("peer_id", peer_id)
                .field("msg_id", msg_id)
                .finish(),
            Self::FileProgress {
                file,
                peer_id,
                stage,
            } => f
                .debug_struct("FileProgress")
                .field("file", file)
                .field("peer_id", peer_id)
                .field("stage", stage)
                .finish(),
            Self::FileReceived {
                peer_id,
                transfer_id,
                path,
                size,
            } => f
                .debug_struct("FileReceived")
                .field("peer_id", peer_id)
                .field("transfer_id", transfer_id)
                .field("path", path)
                .field("size", size)
                .finish(),
            Self::FileFailed {
                file,
                peer_id,
                reason,
            } => f
                .debug_struct("FileFailed")
                .field("file", file)
                .field("peer_id", peer_id)
                .field("reason", reason)
                .finish(),
            Self::PttOpened {
                peer_id,
                stream_id,
                codec,
                bundle_ms,
            } => f
                .debug_struct("PttOpened")
                .field("peer_id", peer_id)
                .field("stream_id", stream_id)
                .field("codec", codec)
                .field("bundle_ms", bundle_ms)
                .finish(),
            Self::PttBundle {
                peer_id,
                stream_id,
                payload,
            } => f
                .debug_struct("PttBundle")
                .field("peer_id", peer_id)
                .field("stream_id", stream_id)
                .field("payload_len", &payload.len())
                .finish_non_exhaustive(),
            Self::PttClosed { peer_id, stream_id } => f
                .debug_struct("PttClosed")
                .field("peer_id", peer_id)
                .field("stream_id", stream_id)
                .finish(),
            Self::SignalingReceived { peer_id, bytes } => f
                .debug_struct("SignalingReceived")
                .field("peer_id", peer_id)
                .field("bytes_len", &bytes.len())
                .finish_non_exhaustive(),
            Self::Metrics { metrics } => f.debug_tuple("Metrics").field(metrics).finish(),
        }
    }
}

impl From<node::NodeEvent> for EngineEvent {
    fn from(event: node::NodeEvent) -> Self {
        match event {
            node::NodeEvent::ConnectProgress { peer_id, phase } => Self::ConnectProgress {
                peer_id,
                phase: phase.into(),
            },
            node::NodeEvent::Connected { peer_id } => Self::Connected { peer_id },
            node::NodeEvent::Closed { peer_id, reason } => Self::Closed {
                peer_id,
                reason: reason.into(),
            },
            node::NodeEvent::TextReceived {
                peer_id,
                msg_id,
                text,
            } => Self::TextReceived {
                peer_id,
                msg_id,
                text,
            },
            node::NodeEvent::Receipt { peer_id, msg_id } => Self::Receipt { peer_id, msg_id },
            node::NodeEvent::FileProgress {
                file,
                peer_id,
                stage,
            } => Self::FileProgress {
                file: file.into(),
                peer_id,
                stage: stage.into(),
            },
            node::NodeEvent::FileReceived {
                peer_id,
                transfer_id,
                path,
                size,
            } => Self::FileReceived {
                peer_id,
                transfer_id,
                path: path_text(path),
                size,
            },
            node::NodeEvent::FileFailed {
                file,
                peer_id,
                reason,
            } => Self::FileFailed {
                file: file.into(),
                peer_id,
                reason: reason.into(),
            },
            node::NodeEvent::PttOpened {
                peer_id,
                stream_id,
                codec,
                bundle_ms,
            } => Self::PttOpened {
                peer_id,
                stream_id,
                codec: codec.into(),
                bundle_ms,
            },
            node::NodeEvent::PttBundle {
                peer_id,
                stream_id,
                payload,
            } => Self::PttBundle {
                peer_id,
                stream_id,
                payload,
            },
            node::NodeEvent::PttClosed { peer_id, stream_id } => {
                Self::PttClosed { peer_id, stream_id }
            }
            node::NodeEvent::SignalingReceived { peer_id, bytes } => {
                Self::SignalingReceived { peer_id, bytes }
            }
            node::NodeEvent::Metrics(metrics) => Self::Metrics {
                metrics: metrics.into(),
            },
        }
    }
}

/// Alınan dosyanın yolu metne: yol `start`'a verilen (UTF-8) `data_dir` ile düğümün ürettiği ASCII
/// addan (`<eş>-<transfer_id>`) oluşur, dönüşüm kayıpsızdır; UTF-8 olmayan bir bileşen (sözleşme
/// dışı) panik değil `U+FFFD` olur.
fn path_text(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

/// `std::io::ErrorKind`'ın `Debug` biçimi (ör. `NotFound`, `StorageFull`) — TANILAMA METNİDİR:
/// std `Debug` biçimini kararlı saymaz, programatik eşleme için değildir. Ayrıntı (OS mesajı, yol)
/// taşımaz.
fn io_kind_name(kind: ErrorKind) -> String {
    format!("{kind:?}")
}

// ── Tutamaçlar ──────────────────────────────────────────────────────────────────────────────────

/// Giden PTT akışının düğüm tutamacı (`ptt_open` döner; `ptt_push` / `ptt_close` alır) — tel
/// `stream_id`'si DEĞİLDİR. Düğüm-tekil, kesin artan, yeniden kullanılmaz; kapanmış tutamaç
/// [`crate::NodeError::HandleClosed`], hiç verilmemiş değer [`crate::NodeError::UnknownHandle`]
/// alır. Kotlin'de değiştirilemez kayıttır (`val raw`; `uniffi-bindgen.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct StreamHandle {
    /// Ham değer (`kama_node::StreamHandle::raw`).
    pub raw: u64,
}

/// Giden dosya aktarımının düğüm tutamacı (`send_file` döner; `cancel_file` alır) — tel
/// `transfer_id`'si DEĞİLDİR. Kurallar [`StreamHandle`] ile aynı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct FileHandle {
    /// Ham değer (`kama_node::FileHandle::raw`).
    pub raw: u64,
}

impl StreamHandle {
    /// Düğüm tutamacı.
    pub(crate) const fn to_node(self) -> node::StreamHandle {
        node::StreamHandle::from_raw(self.raw)
    }
}

impl From<node::StreamHandle> for StreamHandle {
    fn from(handle: node::StreamHandle) -> Self {
        Self { raw: handle.raw() }
    }
}

impl FileHandle {
    /// Düğüm tutamacı.
    pub(crate) const fn to_node(self) -> node::FileHandle {
        node::FileHandle::from_raw(self.raw)
    }
}

impl From<node::FileHandle> for FileHandle {
    fn from(handle: node::FileHandle) -> Self {
        Self { raw: handle.raw() }
    }
}

// ── Girdiler ────────────────────────────────────────────────────────────────────────────────────

/// Platform sinyali (`ui-api v0.3` `onPlatformSignal`) — `kama_node::PlatformSignal`'in aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PlatformSignal {
    /// Ağ bağlantısı değişti. D15: `SESSION_INIT_REF` akışı hub sprintindedir; bu sürümde kabul
    /// edilir ve hiçbir protokol davranışı tetiklemez.
    LinkChanged,
}

impl PlatformSignal {
    /// Düğüm sinyali.
    pub(crate) const fn to_node(self) -> node::PlatformSignal {
        match self {
            Self::LinkChanged => node::PlatformSignal::LinkChanged,
        }
    }
}

/// PTT ses kodlayıcısı (AMP §3 0x06 `PTT_OPEN.codec`) — `kama_app::codec::PttCodec`'in aynası.
/// Değer medya motorunundur; düğüm yorumlamaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PttCodec {
    /// 0x01 opus.
    Opus,
    /// 0x02 lyra.
    Lyra,
}

impl PttCodec {
    /// Uygulama katmanı değeri.
    pub(crate) const fn to_app(self) -> kama_app::codec::PttCodec {
        use kama_app::codec::PttCodec as Target;
        match self {
            Self::Opus => Target::Opus,
            Self::Lyra => Target::Lyra,
        }
    }
}

impl From<kama_app::codec::PttCodec> for PttCodec {
    fn from(codec: kama_app::codec::PttCodec) -> Self {
        use kama_app::codec::PttCodec as Source;
        match codec {
            Source::Opus => Self::Opus,
            Source::Lyra => Self::Lyra,
        }
    }
}

// ── Bağlantı ────────────────────────────────────────────────────────────────────────────────────

/// `ConnectProgress` fazı (S1) — `kama_contracts::s1::ConnectPhase`'in aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ConnectPhase {
    /// Hub yolu ölçülüyor (prob sürüyor).
    Measuring,
    /// Link yok; prob askıda.
    Suspended,
    /// Heartbeat yanıtı 60 s'dir yok. Yalnız UI; hiçbir şeyi sonlandırmaz.
    LongOutage,
}

impl From<kama_contracts::s1::ConnectPhase> for ConnectPhase {
    fn from(phase: kama_contracts::s1::ConnectPhase) -> Self {
        use kama_contracts::s1::ConnectPhase as Source;
        match phase {
            Source::Measuring => Self::Measuring,
            Source::Suspended => Self::Suspended,
            Source::LongOutage => Self::LongOutage,
        }
    }
}

/// [`EngineEvent::Closed`] nedeni — `kama_node::CloseCause`'un aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CloseCause {
    /// Oturum kapandı ve eşin yerine geçen oturumu yok.
    Session {
        /// Kapanış nedeni.
        reason: CloseReason,
    },
    /// Bekleyen bağlantı niyeti bitti (`cancel_connect` → `Cancelled`).
    Abandoned {
        /// Bitiş nedeni.
        reason: AbandonReason,
    },
}

impl From<node::CloseCause> for CloseCause {
    fn from(cause: node::CloseCause) -> Self {
        match cause {
            node::CloseCause::Session(reason) => Self::Session {
                reason: reason.into(),
            },
            node::CloseCause::Abandoned(reason) => Self::Abandoned {
                reason: reason.into(),
            },
        }
    }
}

/// Oturum kapanış nedeni — `kama_contracts::handles::CloseReason`'ın aynası (küme kapalı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CloseReason {
    /// Yeni oturum eskisinin yerini aldı.
    Superseded,
    /// Yerel kullanıcı/uygulama kapattı.
    UserClosed,
    /// Karşı taraf kapattı.
    PeerClosed,
    /// Anahtar hijyeni.
    KeyHygiene,
}

impl From<kama_contracts::handles::CloseReason> for CloseReason {
    fn from(reason: kama_contracts::handles::CloseReason) -> Self {
        use kama_contracts::handles::CloseReason as Source;
        match reason {
            Source::Superseded => Self::Superseded,
            Source::UserClosed => Self::UserClosed,
            Source::PeerClosed => Self::PeerClosed,
            Source::KeyHygiene => Self::KeyHygiene,
        }
    }
}

/// Bekleyen niyetin bitiş nedeni — `kama_contracts::s1::AbandonReason`'ın aynası ("zaman" yok).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AbandonReason {
    /// Kullanıcı/uygulama iptal etti.
    Cancelled,
    /// Karşı taraf kapattı.
    PeerClosed,
    /// Anahtar hijyeni.
    KeyHygiene,
    /// Kalıcı yerel hata.
    LocalPermanent,
}

impl From<kama_contracts::s1::AbandonReason> for AbandonReason {
    fn from(reason: kama_contracts::s1::AbandonReason) -> Self {
        use kama_contracts::s1::AbandonReason as Source;
        match reason {
            Source::Cancelled => Self::Cancelled,
            Source::PeerClosed => Self::PeerClosed,
            Source::KeyHygiene => Self::KeyHygiene,
            Source::LocalPermanent => Self::LocalPermanent,
        }
    }
}

// ── Dosya ───────────────────────────────────────────────────────────────────────────────────────

/// Giden aktarımın aşaması — `kama_node::FileStage`'in aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FileStage {
    /// Son kayıt Transport'a kabul ettirildi — teslim DEĞİL.
    Sent,
    /// Eş dosyayı bütün olarak aldı ve doğruladı (`FILE_RECEIPT(ok)`) — teslim edildi.
    Delivered,
}

impl From<node::FileStage> for FileStage {
    fn from(stage: node::FileStage) -> Self {
        match stage {
            node::FileStage::Sent => Self::Sent,
            node::FileStage::Delivered => Self::Delivered,
        }
    }
}

/// Başarısız aktarımın kimliği — `kama_node::FileRef`'in aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FileRef {
    /// Giden: `send_file`'ın tutamacı.
    Outgoing {
        /// Tutamaç.
        file: FileHandle,
    },
    /// Gelen: gönderen eşin `transfer_id`'si (olayın `peer_id`'siyle birlikte tekil).
    Incoming {
        /// Gönderenin `transfer_id`'si.
        transfer_id: u64,
    },
}

impl From<node::FileRef> for FileRef {
    fn from(file: node::FileRef) -> Self {
        match file {
            node::FileRef::Outgoing(handle) => Self::Outgoing {
                file: handle.into(),
            },
            node::FileRef::Incoming(transfer_id) => Self::Incoming { transfer_id },
        }
    }
}

/// [`EngineEvent::FileFailed`] nedeni — `kama_node::FileFailReason`'ın aynası.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FileFailReason {
    /// Gelen: özet tutmadı ya da parça gelmedi; eşe karşılık gelen `FILE_RECEIPT` gitti.
    Incoming {
        /// Neden.
        failure: FileFailure,
    },
    /// Gelen: gönderen iptal etti (`FILE_ABORT`).
    AbortedBySender {
        /// Gönderenin nedeni.
        reason: FileAbortReason,
    },
    /// Gelen: dosya doğrulandı ama diske yazılamadı (yerel hata, eşe bildirilmez).
    Storage {
        /// `std::io::ErrorKind`'ın `Debug` biçimi (ör. `StorageFull`) — tanılama metni,
        /// programatik eşleme için değil.
        io_kind: String,
    },
    /// Giden: aktarım teslim edilmeden bitti.
    Outgoing {
        /// Neden.
        failure: SendFailure,
    },
    /// Giden: dosya kuyruğa ALINMADI; eşe hiçbir şey gitmedi (ör. `NoSession`: oturumsuz
    /// `send_file`).
    NotQueued {
        /// Motorun reddi.
        reason: FileSendError,
    },
    /// Giden: kaynak dosya okunamadı (`InvalidInput` = düzenli dosya değil); eşe hiçbir şey
    /// gitmedi.
    SourceUnreadable {
        /// `std::io::ErrorKind`'ın `Debug` biçimi (ör. `NotFound`) — tanılama metni, programatik
        /// eşleme için değil.
        io_kind: String,
    },
    /// Giden: kaynak dosyanın adı UTF-8 değil; eşe hiçbir şey gitmedi.
    SourceName,
}

impl From<node::FileFailReason> for FileFailReason {
    fn from(reason: node::FileFailReason) -> Self {
        match reason {
            node::FileFailReason::Incoming(failure) => Self::Incoming {
                failure: failure.into(),
            },
            node::FileFailReason::AbortedBySender(reason) => Self::AbortedBySender {
                reason: reason.into(),
            },
            node::FileFailReason::Storage(kind) => Self::Storage {
                io_kind: io_kind_name(kind),
            },
            node::FileFailReason::Outgoing(failure) => Self::Outgoing {
                failure: failure.into(),
            },
            node::FileFailReason::NotQueued(error) => Self::NotQueued {
                reason: error.into(),
            },
            node::FileFailReason::SourceUnreadable(kind) => Self::SourceUnreadable {
                io_kind: io_kind_name(kind),
            },
            node::FileFailReason::SourceName => Self::SourceName,
        }
    }
}

/// Gelen aktarımın başarısızlığı — `kama_app::file_transfer::FileFailure`'ın aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FileFailure {
    /// Birleştirilen baytların özeti `FILE_BEGIN.whole_file_hash`'e eşit değil.
    HashMismatch,
    /// `FILE_IDLE_TTL` boyunca yeni parça gelmedi.
    IdleTimeout,
}

impl From<kama_app::file_transfer::FileFailure> for FileFailure {
    fn from(failure: kama_app::file_transfer::FileFailure) -> Self {
        use kama_app::file_transfer::FileFailure as Source;
        match failure {
            Source::HashMismatch => Self::HashMismatch,
            Source::IdleTimeout => Self::IdleTimeout,
        }
    }
}

/// `FILE_ABORT.reason` (AMP §3) — `kama_app::codec::FileAbortReason`'ın aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FileAbortReason {
    /// 0x01 `user_cancel`.
    UserCancel,
    /// 0x02 `hash_mismatch`.
    HashMismatch,
    /// 0x03 `storage_full`.
    StorageFull,
    /// 0x04 `timeout`.
    Timeout,
    /// 0x05 `superseded`.
    Superseded,
}

impl From<kama_app::codec::FileAbortReason> for FileAbortReason {
    fn from(reason: kama_app::codec::FileAbortReason) -> Self {
        use kama_app::codec::FileAbortReason as Source;
        match reason {
            Source::UserCancel => Self::UserCancel,
            Source::HashMismatch => Self::HashMismatch,
            Source::StorageFull => Self::StorageFull,
            Source::Timeout => Self::Timeout,
            Source::Superseded => Self::Superseded,
        }
    }
}

/// Giden aktarımın başarısızlığı — `kama_app::file_transfer::SendFailure`'ın aynası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SendFailure {
    /// `FILE_RECEIPT(aborted_by_receiver)`: alıcının kullanıcısı iptal etti.
    AbortedByReceiver,
    /// `FILE_RECEIPT(hash_mismatch)`: alıcıda özet tutmadı.
    HashMismatch,
    /// `FILE_RECEIPT(timeout)`: alıcıya parça ulaşmadı.
    ReceiverTimeout,
    /// `FILE_RECEIPT(rejected)`: alıcı `FILE_BEGIN`'i kabul etmedi.
    Rejected,
    /// `FILE_END` sonrası `FILE_RECEIPT_TTL` içinde alındı gelmedi.
    ReceiptTimeout,
    /// Yarım aktarımın oturumu değişti (`FILE_ABORT(superseded)`). Yeniden gönderim yeni bir
    /// `send_file`'dır (yeni `transfer_id`; plan D6); `ui-api v0.3` karşılığı KM-SEAM-AK-001 §4.5.
    Superseded,
    /// Gönderenin kullanıcısı iptal etti (`cancel_file`).
    Cancelled,
    /// Transport bir kaydı kalıcı hatayla reddetti; eşe sinyal gitmedi.
    Transport {
        /// Transport'un nedeni.
        reason: TransportError,
    },
}

impl From<kama_app::file_transfer::SendFailure> for SendFailure {
    fn from(failure: kama_app::file_transfer::SendFailure) -> Self {
        use kama_app::file_transfer::SendFailure as Source;
        match failure {
            Source::AbortedByReceiver => Self::AbortedByReceiver,
            Source::HashMismatch => Self::HashMismatch,
            Source::ReceiverTimeout => Self::ReceiverTimeout,
            Source::Rejected => Self::Rejected,
            Source::ReceiptTimeout => Self::ReceiptTimeout,
            Source::Superseded => Self::Superseded,
            Source::Cancelled => Self::Cancelled,
            Source::Transport(reason) => Self::Transport {
                reason: reason.into(),
            },
        }
    }
}

// ── Ölçüler ─────────────────────────────────────────────────────────────────────────────────────

/// Ölçüler — `kama_node::NodeMetrics`'in aynası. `None` alanlar bugün salt-okunur bir kaynaktan
/// ALINAMAYAN değerlerdir; uydurma değer verilmez (README-HASAN zorunlu satır 3).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct NodeMetrics {
    /// Bu sürümde daima `None` (overlay alanı; hub sprintinde dolar).
    pub rtt_ms: Option<u32>,
    /// Bu sürümde daima `None` (overlay alanı; hub sprintinde dolar).
    pub loss_pct: Option<f32>,
    /// Bu sürümde daima `None` (overlay alanı; hub sprintinde dolar).
    pub bitrate_kbps: Option<u32>,
    /// Hazır oturumların EN DÜŞÜK gönderim DH epoch'u (`0` / `1`); hazır oturum yoksa `None`.
    pub epoch: Option<u8>,
    /// PoC'de daima `None`: heartbeat hub sprintindedir (D18).
    pub last_heartbeat_age_s: Option<u64>,
    /// Beklenen tutamaçla birebir eşleşmeyen `Session(_)` datagramı (K4G-01).
    pub ctx_mismatch_drops: u64,
    /// HubChannel bağlamında açılan ve düşürülen datagram (direct-mode'da hub yok).
    pub hub_frames_dropped: u64,
    /// Teslim kanalı doluyken düşen teslim (DUR-K4G-01 (i)); 0 olmalı.
    pub delivery_channel_drops: u64,
    /// Eş başına bağlantı durumu, ad sırasıyla — provisioning dosyasındaki BÜTÜN eşler.
    pub connect_state: Vec<PeerConnectState>,
}

impl From<node::NodeMetrics> for NodeMetrics {
    fn from(metrics: node::NodeMetrics) -> Self {
        let node::NodeMetrics {
            rtt_ms,
            loss_pct,
            bitrate_kbps,
            epoch,
            last_heartbeat_age_s,
            ctx_mismatch_drops,
            hub_frames_dropped,
            delivery_channel_drops,
            connect_state,
        } = metrics;
        Self {
            rtt_ms,
            loss_pct,
            bitrate_kbps,
            epoch,
            last_heartbeat_age_s,
            ctx_mismatch_drops,
            hub_frames_dropped,
            delivery_channel_drops,
            connect_state: connect_state
                .into_iter()
                .map(PeerConnectState::from)
                .collect(),
        }
    }
}

/// Bir eşin bağlantı durumu — `kama_node::PeerConnectState`'in aynası.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PeerConnectState {
    /// Eş (provisioning adı).
    pub peer_id: String,
    /// Durum.
    pub state: LinkState,
    /// Eşin güncel oturumunun gönderim DH epoch'u; hazır oturum yoksa `None`.
    pub epoch: Option<u8>,
}

impl From<node::PeerConnectState> for PeerConnectState {
    fn from(state: node::PeerConnectState) -> Self {
        let node::PeerConnectState {
            peer_id,
            state,
            epoch,
        } = state;
        Self {
            peer_id,
            state: state.into(),
            epoch,
        }
    }
}

/// Eşin bağlantı durumu (UI) — `kama_node::LinkState`'in aynası. Eşin canlılığını BİLDİRMEZ: tek
/// canlılık sinyali alındılardır (`Receipt`, `FileProgress(Delivered)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LinkState {
    /// Hazır oturum da bekleyen niyet de yok.
    Idle,
    /// Bekleyen niyet ve son fazı.
    Connecting {
        /// Faz.
        phase: ConnectPhase,
    },
    /// Eşin güncel oturumu var.
    Connected,
}

impl From<node::LinkState> for LinkState {
    fn from(state: node::LinkState) -> Self {
        match state {
            node::LinkState::Idle => Self::Idle,
            node::LinkState::Connecting(phase) => Self::Connecting {
                phase: phase.into(),
            },
            node::LinkState::Connected => Self::Connected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testlerdeki ses yükü ve blob boyu.
    const LEN: usize = 40;

    /// `shown` uzunluğu (`label: LEN`) taşır; baytı ne ondalık ne onaltılık taşır.
    fn assert_redacted(shown: &str, label: &str, byte: u8) {
        assert!(shown.contains(&format!("{label}: {LEN}")), "{shown}");
        for needle in [byte.to_string(), format!("{byte:x}")] {
            assert!(
                !shown.contains(&needle),
                "Debug çıktısında {needle}: {shown}"
            );
        }
    }

    #[test]
    fn debug_redacts_text_audio_and_blob_and_keeps_lengths() {
        const SECRET_TEXT: &str = "gizli-metin-içeriği";
        const PAYLOAD_BYTE: u8 = 0xA7;
        const BLOB_BYTE: u8 = 0xC3;
        let text = EngineEvent::TextReceived {
            peer_id: "alice".to_owned(),
            msg_id: 3,
            text: SECRET_TEXT.to_owned(),
        };
        let shown = format!("{text:?}");
        assert!(!shown.contains(SECRET_TEXT), "{shown}");
        assert!(
            shown.contains(&format!("text_len: {}", SECRET_TEXT.len())),
            "{shown}"
        );
        let audio = EngineEvent::PttBundle {
            peer_id: "alice".to_owned(),
            stream_id: 1,
            payload: vec![PAYLOAD_BYTE; LEN],
        };
        assert_redacted(&format!("{audio:?}"), "payload_len", PAYLOAD_BYTE);
        let blob = EngineEvent::SignalingReceived {
            peer_id: "alice".to_owned(),
            bytes: vec![BLOB_BYTE; LEN],
        };
        assert_redacted(&format!("{blob:?}"), "bytes_len", BLOB_BYTE);
    }

    #[test]
    fn handles_and_codec_round_trip_through_the_node_types() {
        const RAW: u64 = 7;
        assert_eq!(StreamHandle { raw: RAW }.to_node().raw(), RAW);
        assert_eq!(FileHandle { raw: RAW }.to_node().raw(), RAW);
        assert_eq!(
            StreamHandle::from(node::StreamHandle::from_raw(RAW)),
            StreamHandle { raw: RAW }
        );
        assert_eq!(
            FileHandle::from(node::FileHandle::from_raw(RAW)),
            FileHandle { raw: RAW }
        );
        for codec in [PttCodec::Opus, PttCodec::Lyra] {
            assert_eq!(PttCodec::from(codec.to_app()), codec);
        }
    }

    #[test]
    fn metrics_mirror_keeps_every_field() {
        let measuring = kama_contracts::s1::ConnectPhase::Measuring;
        let metrics = node::NodeMetrics {
            rtt_ms: None,
            loss_pct: None,
            bitrate_kbps: None,
            epoch: Some(1),
            last_heartbeat_age_s: None,
            ctx_mismatch_drops: 2,
            hub_frames_dropped: 3,
            delivery_channel_drops: 4,
            connect_state: vec![node::PeerConnectState {
                peer_id: "bob".to_owned(),
                state: node::LinkState::Connecting(measuring),
                epoch: None,
            }],
        };
        assert_eq!(
            NodeMetrics::from(metrics),
            NodeMetrics {
                rtt_ms: None,
                loss_pct: None,
                bitrate_kbps: None,
                epoch: Some(1),
                last_heartbeat_age_s: None,
                ctx_mismatch_drops: 2,
                hub_frames_dropped: 3,
                delivery_channel_drops: 4,
                connect_state: vec![PeerConnectState {
                    peer_id: "bob".to_owned(),
                    state: LinkState::Connecting {
                        phase: ConnectPhase::Measuring
                    },
                    epoch: None,
                }],
            }
        );
    }
}
