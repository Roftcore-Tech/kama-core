//! [`NodeError`] — bileşim kökünün tipli hataları. Hiçbiri girdiden gizli değer taşımaz:
//! provisioning hataları (`kama_provision::format::FormatError`) yalnız sınıf ve konum, depo ve
//! transport hataları yalnız neden taşır; metin, dosya baytı, ses, sinyalleşme blob'u ve anahtar
//! hiçbir varyanta girmez.

use kama_app::codec::LimitsError;
use kama_app::file_transfer::FileSendError;
use kama_app::messaging::MessengerError;
use kama_app::ptt::PttError;
use kama_app::signaling::SignalingError;
use kama_provision::format::FormatError;
use kama_store::StoreOpenError;

use client_core::transport::TransportBuildError;

use crate::wiring::DirectoryError;

/// [`crate::KamaNode`] hatası.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    // ── Kurulum (`KamaNode::start`) ──
    /// Cihaz provisioning dosyası okunamadı ya da doğrulanamadı (PROVISIONING-FORMAT-v0 §5).
    #[error("provisioning file: {0}")]
    Config(#[from] FormatError),
    /// Doğrulanmış dosyadan eş dizini kurulamadı (yinelenen `dest_hash` / `cert_ref`, tüm-sıfır
    /// anahtar).
    #[error("peer directory: {0}")]
    PeerDirectory(DirectoryError),
    /// Bu cihazın `direct_addrs` girdisi yok: direct-mode veri alıcısı bağlanacak adresi bilmiyor.
    #[error("this device has no direct-mode address in its provisioning file")]
    NoDirectAddress,
    /// Veri dizini (ya da alınan dosyalar dizini) oluşturulamadı.
    #[error("data directory: {0}")]
    DataDir(std::io::Error),
    /// Kalıcı depo açılamadı (aynı `data_dir` başka bir düğümde açıksa `AlreadyOpen`).
    #[error("store: {0}")]
    Store(#[from] StoreOpenError),
    /// Transport kurulamadı (UDP bind, çalışma-zamanı).
    #[error("transport: {0}")]
    Transport(#[from] TransportBuildError),
    /// Transport'un birimi AMP kaydı için çok küçük.
    #[error("AMP limits: {0}")]
    Limits(LimitsError),
    /// Düğüm iş parçacığı başlatılamadı.
    #[error("node thread could not be started: {0}")]
    Spawn(std::io::Error),
    /// Bu süreçte aynı cihaz kimliğiyle (provisioning `dest_hash`) çalışan bir düğüm var: süreç-içi
    /// zarf rölesi bir kimliği tek uç noktaya bağlar, sessizce devralmaz. Önceki düğüm kapanınca
    /// (`shutdown` dönünce) aynı kimlik yeniden açılabilir.
    #[error("a node with this device identity is already running in this process")]
    IdentityInUse,

    // ── Çağrı ──
    /// Provisioning dosyasında böyle bir eş adı yok.
    #[error("unknown peer")]
    UnknownPeer,
    /// Bu düğümün hiç vermediği tutamaç.
    #[error("unknown handle")]
    UnknownHandle,
    /// Tutamaç bu düğümce verilmişti ama kapandı (akış kapatıldı, aktarım bitti ya da kuyruğa hiç
    /// alınamadı). Tutamaçlar yeniden kullanılmaz: kapanmış tutamaç hiçbir zaman başka bir akışa
    /// ya da aktarıma çözülmez.
    #[error("handle is closed")]
    HandleClosed,
    /// Düğüm tutamaç uzayı (`u64`) tükendi; sarma YOK.
    #[error("node handle space is exhausted")]
    HandlesExhausted,
    /// Girdi tek bir AMP kaydına sığmıyor; komut kuyruğa ALINMADI.
    #[error("{what} of {len} bytes exceeds {max} bytes")]
    TooLarge {
        /// Girdinin türü (`"text"`, `"ptt payload"`, `"signaling blob"`).
        what: &'static str,
        /// Verilen uzunluk.
        len: usize,
        /// Bu düğümün limiti.
        max: usize,
    },
    /// Komut kuyruğu dolu (geri basınç); çağrı bloklamadı, komut ALINMADI.
    #[error("node command queue is full")]
    Busy,
    /// Düğüm durdu (kapatıldı ya da iş parçacığı bitti).
    #[error("node is stopped")]
    Stopped,
    /// Yanıt bekleyen çağrı düğüm iş parçacığının kendisinden (dinleyiciden) yapıldı: düğüm kendini
    /// bekleyemez — kilitlenme yerine tipli hata.
    #[error("blocking node call from the node thread (listener) would deadlock")]
    ReentrantCall,
    /// Metin motoru reddetti (`kama_app::messaging`).
    #[error("messenger: {0}")]
    Messenger(MessengerError),
    /// PTT motoru reddetti (`kama_app::ptt`).
    #[error("ptt: {0}")]
    Ptt(PttError),
    /// Dosya motoru reddetti (`kama_app::file_transfer`).
    #[error("file transfer: {0}")]
    File(FileSendError),
    /// Sinyalleşme motoru reddetti (`kama_app::signaling`; ör. `NoSession`, `Transport`). Yalnız
    /// uzunluk ve neden taşır, blob baytı taşımaz.
    #[error("signaling: {0}")]
    Signaling(SignalingError),
    /// Düğüm iş parçacığı panikle bitti.
    #[error("node thread panicked")]
    NodeThreadPanicked,
}
