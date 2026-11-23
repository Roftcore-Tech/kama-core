//! `kama-node` — bileşim kökü (K4G-11): gerçek anahtar + saat + entropi + UDP + disk.
//!
//! [`KamaNode::start`] ürünü ayağa kaldıran TEK yoldur (bugüne dek tek kurulum yolu test
//! fikstürüydü: `kama_noise_wire::seam_mock`, anahtarları GİZLİ DEĞİL). K4G-15 (UniFFI,
//! `crates/kama-mobile`) bu API'yi birebir dışa açar.
//!
//! # Kurulum ([`KamaNode::start`])
//!
//! 1. Provisioning: yalnız verilen cihaz dosyası okunur (`kama_provision::format::
//!    load_device_config`; PROVISIONING-FORMAT-v0). Kimlik, anahtarlar ve eş dizini BELLEKTE
//!    tutulur, depoya yazılmaz (D8 v1.1). `root.json` cihazda OKUNMAZ (D9): provisioning'den yalnız
//!    `config_path` açılır ve cihaz yapılandırmasında kök gizli anahtarı için alan yoktur.
//! 2. Veri dizini: `<data_dir>/kama-node.redb` ([`STORE_FILE_NAME`]) — `kama_store::RedbStore`,
//!    AYNI `Arc` nesnesi Application'a `AppStore` (sayaçlar, dedup, outbox), Crypto'ya
//!    `CounterStore` (`init_counter`) olarak verilir; `<data_dir>/files/` ([`RECEIVED_FILES_DIR`])
//!    alınan dosyalar.
//! 3. Crypto: `NoiseWireProtection::with_control` gerçek anahtarlarla, OS entropisi ve Crypto'nun
//!    saatiyle (`src/wiring.rs` — `kama_noise_wire` adının geçtiği TEK yer, layer_check C8).
//! 4. Zarf taşıyıcısı (INIT/RESPONSE): süreç-içi zarf rölesi `LoopbackEnvelopeRelay`
//!    (`src/carrier.rs`; sahip kararı DUR-K4G-11 → B) — ürün taşıyıcısı değildir — iki cihaz arası
//!    taşıyıcı = hub Plane 1 rölesi, S4, hub sprinti. AYNI süreçteki düğümler zarflarını bununla
//!    değiş tokuş eder (kimlik başına tek canlı düğüm: [`NodeError::IdentityInUse`]); iki telefon
//!    arası oturum hub sprintiyle gelir. Kayıtsız eşe zarf gitmez ve saklanmaz; geç başlayan eşte
//!    glare kuralı iki niyeti de takabilir (`A.dest_hash < B.dest_hash`) ve takılma ancak erken
//!    tarafın bekleyen INIT'i kalkınca (`cancel_connect` ya da `HALF_OPEN_TTL`) çözülür — kural ve
//!    atıflar `src/carrier.rs`'te. Tek süreçli testler (CORE, INT-02) düğümleri bağlanmadan önce
//!    başlatır ve el sıkışma sırasında yeniden başlatmaz.
//! 5. Transport: `client_core::transport::ClientTransport`, ürün profili
//!    (`ClientTransportConfig::product`), veri alıcısı bu cihazın `direct_addrs` adresinde, eş
//!    adresleri dosyadan (direct-mode; hub yok).
//! 6. Application: `Messenger::with_store` + `Ptt` + `FileTransfer` + `Signaling`; üçüne
//!    Application'ın AYNI saati (`kama_app::clock::SystemClock`, tek `Arc`) — Crypto'nunkinden AYRI
//!    (INV-NEIGHBOR-1; sahip kararı CEVAP PAKETİ 2 madde 3).
//! 7. Düğüm iş parçacığı (`std::thread`; yeni async runtime YOK): motorların tek sahibi; periyodik
//!    tur ve olay zinciri `src/node.rs` modül belgesinde.
//!
//! # API (plan K4G-11 NE 2; K4G-14 NE (2))
//!
//! Çağrılar: [`KamaNode::connect`] · [`KamaNode::cancel_connect`] · [`KamaNode::send_text`] ·
//! [`KamaNode::send_file`] · [`KamaNode::cancel_file`] · [`KamaNode::ptt_open`] ·
//! [`KamaNode::ptt_push`] · [`KamaNode::ptt_close`] · [`KamaNode::send_signaling`] (K4G-14b) ·
//! [`KamaNode::on_platform_signal`] · [`KamaNode::drain_background`] · [`KamaNode::metrics`] ·
//! [`KamaNode::shutdown`]. Olaylar: [`NodeEvent`] — K4G-14b ile [`NodeEvent::SignalingReceived`].
//!
//! # Çağrılar
//!
//! Çağrılar ağ ya da zamanlayıcı BEKLEMEZ. Komut düğüm iş parçacığına sınırlı kuyrukla gider
//! (doluysa [`NodeError::Busy`]); yanıtı olan çağrı yalnız komutun düğüm turundaki işlenmesini
//! bekler (`send_text`'te sayaç + outbox commit'i dahil). Uzun iş — dosyanın okunması ve özeti —
//! düğüm iş parçacığındadır ve çağıranı BEKLETMEZ: [`KamaNode::send_file`] tutamacı hemen döner,
//! sonuç olaydır. [`KamaNode::metrics`] son turun anlık görüntüsünü kopyalar. Dinleyiciden
//! (düğüm iş parçacığından) yanıt bekleyen çağrı kilitlenmez, [`NodeError::ReentrantCall`] alır.
//!
//! # Tutamaçlar
//!
//! [`KamaNode::ptt_open`] ve [`KamaNode::send_file`] DÜĞÜM TUTAMACI döner ([`StreamHandle`],
//! [`FileHandle`]), çıplak tel kimliği DEĞİL: düğüm-tekil, bellek içi, kesin artan, yeniden
//! kullanılmaz; kapanmış tutamaç [`NodeError::HandleClosed`] alır ve asla başka eşin akışına /
//! aktarımına gitmez (sahip kararı CEVAP PAKETİ 2 madde 1; `src/handle.rs`). `msg_id` tel
//! kimliğidir ama yalnız döner ve olayla gelir (hiçbir çağrı almaz); `(eş, msg_id)` tekildir.
//! [`KamaNode::send_signaling`]'in döndürdüğü `SignalingId` sinyalleşme motorunun kimliğidir; ayrı
//! düğüm tutamacı yoktur (gerekçe çağrının belgesinde).
//!
//! # KAPSAM DIŞI (plan §4.5)
//!
//! Hub, heartbeat (D18; `last_heartbeat_age_s` daima `None`), `BestEffort` / `DeadlineBounded`
//! (K4G-10 bu sprintte yok; `deadline_expired_drops` alanı yok), mesaj geçmişi, at-rest şifreleme.

#![forbid(unsafe_code)]

mod a5;
mod carrier;
mod error;
mod event;
mod files;
mod handle;
mod hub_mode;
mod link_profile;
mod node;
mod peers;
mod pilot_record;
#[cfg(feature = "test-util")]
mod probe;
mod ptt_pilot;
#[cfg(test)]
mod test_support;
mod wiring;

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle, ThreadId};
use std::time::Duration;

use client_core::transport::{ClientTransportConfig, EnvelopeLink};
use kama_app::clock::{MonotonicClock, SystemClock};
use kama_app::codec::limits::TEXT_PREFIX_BYTES;
use kama_app::codec::{AmpLimits, PttCodec};
use kama_app::file_transfer::FileTransfer;
use kama_app::messaging::{Messenger, MsgId};
use kama_app::ptt::max_bundle_payload_bytes;
use kama_app::signaling::{max_blob_bytes, Signaling, SignalingId};
use kama_app::store::AppStore;
use kama_provision::format::load_device_config;
use kama_store::RedbStore;

pub use error::NodeError;
pub use event::{
    CloseCause, FileFailReason, FileRef, FileStage, LinkState, NodeEvent, NodeMetrics,
    PeerConnectState,
};
pub use files::RECEIVED_FILES_DIR;
pub use handle::{FileHandle, PttPress, StreamHandle};
#[cfg(feature = "test-util")]
pub use probe::{EnvelopeCounts, NodeProbe, PeerSessionProbe, SessionEventCounts, SessionProbe};

use files::ReceivedFiles;
use node::{AppEngines, Command, NodeLoop, Reply, Shared, COMMAND_QUEUE_CAP, REPLY_CAPACITY};
use peers::PeerBook;
use wiring::Transport;

/// Kalıcı deponun `data_dir` altındaki dosyası (redb; şema `kama-store` v0).
pub const STORE_FILE_NAME: &str = "kama-node.redb";

/// Trafiksiz düğüm turunun aralığı — `[KALİBRASYON]`, normatif DEĞİL. Değer client-core veri
/// alıcısının ACK zamanlayıcısıyla (`ACK_TICK` = 20 ms) aynı mertebededir: gelen birimin
/// uygulamaya ulaşmasına en çok bir ACK periyodu kadar gecikme ekler. Komut gelince tur beklemeden
/// döner; bu aralık yalnız trafik ve komut yokken zamanlayıcıların (PTT-R4, `FILE_IDLE_TTL`,
/// `FILE_RECEIPT_TTL`, Crypto zamanlayıcıları) denetim sıklığıdır.
pub const NODE_TICK: Duration = Duration::from_millis(20);

/// [`NodeEvent::Metrics`] aralığı — `[KALİBRASYON]`, normatif DEĞİL: ölçüm overlay'inin (RTT,
/// kayıp, epoch …; PoC-KAPSAM-001 §2 Android) insan gözüyle izlenen saniyelik tazelemesi.
/// [`KamaNode::metrics`] her turda güncellenir; bu yalnız itilen olayın sıklığıdır.
pub const METRICS_TICK: Duration = Duration::from_secs(1);

/// Düğüm iş parçacığının adı (Linux'ta 15 bayta sığar).
const NODE_THREAD_NAME: &str = "kama-node";

#[cfg(test)]
thread_local! {
    /// [PTT-BİRLEŞİK · deney] Yalnız birim testi: `true` iken bu iş parçacığında kurulan düğümün
    /// `RuntimeConfig::ptt_coalesce` bayrağı açılır (anahtarın türetildiğini sınamak için).
    pub(crate) static PTT_COALESCE_FOR_TEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// [PTT-BİRLEŞİK · pilot] Yalnız birim testi: `Some` iken bu iş parçacığında kurulan düğüm
    /// `KAMA_PTT_PILOT` yerine bu değeri okur (süreç ortamı paylaşıldığından ortam değişkeni
    /// testte kurulmaz).
    pub(crate) static PTT_PILOT_FOR_TEST: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Düğüm olaylarının dinleyicisi. DÜĞÜM İŞ PARÇACIĞINDA çağrılır: hızlı dönmelidir (dönene dek
/// düğüm turu durur). İçinden yanıt bekleyen düğüm çağrısı [`NodeError::ReentrantCall`] alır.
pub trait NodeListener: Send {
    /// Bir olay.
    fn on_event(&self, event: NodeEvent);
}

/// Platform sinyali (`ui-api v0.3` `onPlatformSignal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlatformSignal {
    /// Ağ bağlantısı değişti (PoC-KAPSAM-001 K4). D15: `SESSION_INIT_REF` akışı hub sprintindedir;
    /// bu sprintte (direct-mode) kabul edilir ve hiçbir protokol davranışı tetiklemez.
    LinkChanged,
}

/// Komut kuyruğuna girmeden önce sınanan girdi tavanları (AMP limitlerinden türetilir).
#[derive(Debug, Clone, Copy)]
struct InputLimits {
    /// TEXT metni: `MAX_BODY_BYTES − TEXT_PREFIX_BYTES` (AMP §3 0x01; D14 "tek TEXT ≤ 1002 B").
    max_text_bytes: usize,
    /// `MEDIA_BUNDLE` yükü: `kama_app::ptt::max_bundle_payload_bytes`.
    max_bundle_bytes: usize,
    /// `SIGNALING_BLOB` blob'u: `kama_app::signaling::max_blob_bytes` (`MAX_BODY_BYTES −
    /// blob_len öneki`; AMP §3 0x09).
    max_blob_bytes: usize,
}

impl InputLimits {
    fn from_amp(limits: &AmpLimits) -> Self {
        Self {
            max_text_bytes: limits.max_body_bytes().saturating_sub(TEXT_PREFIX_BYTES),
            max_bundle_bytes: max_bundle_payload_bytes(limits),
            max_blob_bytes: max_blob_bytes(limits),
        }
    }
}

/// Kurulmuş ama başlatılmamış düğüm ([`KamaNode::build`]): döngü + API'nin paylaştığı durum.
pub(crate) struct Built {
    /// Düğüm döngüsü (üründe `spawn` onu kendi iş parçacığına taşır; birim testleri elle sürer).
    pub(crate) node: NodeLoop,
    shared: Arc<Shared>,
    peers: Arc<PeerBook>,
    limits: InputLimits,
}

/// Çalışan bir Kama düğümü (bkz. crate belgesi). `Send + Sync`: birden çok iş parçacığından
/// paylaşılabilir. Düşerken düğümü kapatır ([`KamaNode::shutdown`] ile aynı; Drop panik etmez).
pub struct KamaNode {
    commands: SyncSender<Command>,
    shared: Arc<Shared>,
    peers: Arc<PeerBook>,
    limits: InputLimits,
    node_thread: ThreadId,
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for KamaNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Kimlik, anahtar ve eş özetleri Debug'a GİRMEZ.
        f.debug_struct("KamaNode").finish_non_exhaustive()
    }
}

/// [ACK-BİÇİMİ · PoC 2026-11-23] ACK biçimi dağıtım anahtarı (bkz. `Node::start`).
pub const ACK_FORMAT_ENV_VAR: &str = "KAMA_ACK_FORMAT";

/// `KAMA_ACK_FORMAT` değerini çözer: boş/`v2` → V2 (ürün varsayılanı), `bitmap` → bitmap pilotu;
/// başka değer yapılandırma hatası.
fn parse_ack_format(v: &str) -> Result<client_core::runtime::AckFormat, NodeError> {
    client_core::runtime::AckFormat::parse(v).ok_or(NodeError::Config(
        kama_provision::format::FormatError::Invalid("KAMA_ACK_FORMAT: 'v2' ya da 'bitmap' olmalı"),
    ))
}

/// [İŞ-30] Çekirdeğin derleme kimliği (başlangıç kaydı ve pilot özeti): paket sürümü ve derlemede
/// verilmişse `KAMA_GIT_SHA` (yoksa "bilinmiyor"; uydurulmaz).
pub(crate) fn core_build_line() -> String {
    format!(
        "çekirdek_sürümü={}\nçekirdek_git={}\n",
        env!("CARGO_PKG_VERSION"),
        option_env!("KAMA_GIT_SHA").unwrap_or("bilinmiyor (derlemede KAMA_GIT_SHA verilmedi)"),
    )
}

impl KamaNode {
    /// Düğümü kurar ve başlatır (bkz. crate belgesi, "Kurulum"). `config_path` cihazın
    /// `device-<ad>.json`'udur; `data_dir` yoksa oluşturulur (Unix'te `0700`).
    ///
    /// **Çağıran async / tokio bağlamında olmamalıdır:** client-core veri yolu kurulurken kendi
    /// çalışma-zamanını kurar ve `block_on` kullanır (bir tokio bağlamının içinden çağrı panikler).
    /// Async bir uygulama `start`'ı ayrı bir iş parçacığından ya da `spawn_blocking` ile çağırır
    /// (K4G-05 testleri; K4G-15 README). UniFFI / JVM iş parçacıkları tokio bağlamı değildir.
    ///
    /// # Errors
    /// [`NodeError::Config`] · [`NodeError::NoDirectAddress`] · [`NodeError::DataDir`] ·
    /// [`NodeError::Store`] · [`NodeError::PeerDirectory`] · [`NodeError::IdentityInUse`] (bu
    /// süreçte aynı cihaz kimliğiyle çalışan bir düğüm var — süreç-içi zarf rölesi) ·
    /// [`NodeError::Transport`] · [`NodeError::Limits`] · [`NodeError::Spawn`]. Hata yolunda
    /// açılan her şey (depo, röle kaydı, soket) kapanır.
    pub fn start(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
    ) -> Result<Self, NodeError> {
        let app_clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
        Self::spawn(Self::build(config_path, data_dir, listener, app_clock)?)
    }

    /// Kurulumun TEK gövdesi (iş parçacığı BAŞLATILMAZ). Sıra: provisioning → veri dizini → depo →
    /// Crypto (aynı `Arc` depo `CounterStore` olarak) → `drop(config)` → zarf taşıyıcısı (süreç-içi
    /// röle kaydı; `src/carrier.rs`) → Transport → Application motorları (aynı `Arc` depo
    /// `AppStore` olarak; `app_clock` `Ptt`, `FileTransfer` ve `Signaling`'in ortak saati —
    /// Crypto'nunkinden AYRI; üründe `kama_app::clock::SystemClock`). Birim testleri döngüyü elle
    /// sürmek için bunu doğrudan kullanır.
    pub(crate) fn build(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
        app_clock: Arc<dyn MonotonicClock>,
    ) -> Result<Built, NodeError> {
        Self::build_with_link(config_path, data_dir, listener, app_clock, None)
    }

    /// [`Self::build`]'in gövdesi. `link` = `None` → ürün zarf taşıyıcısı (süreç-içi röle kaydı;
    /// ürün yolunun tek değeri). `Some` YALNIZ ölçüm kurucusundan gelir
    /// ([`Self::start_with_link_for_test`], `test-util`; KTP2-F0 iki süreçli düzenek): taşıyıcı
    /// çağıranındır, röle kaydı yapılmaz. Kurulum sırası iki durumda da aynıdır.
    fn build_with_link(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
        app_clock: Arc<dyn MonotonicClock>,
        link: Option<Box<dyn EnvelopeLink>>,
    ) -> Result<Built, NodeError> {
        // (1) Provisioning — yalnız bu dosya; kimlik/anahtar/eş dizini bellekte (D8, D9).
        let config = load_device_config(config_path)?;
        let peers = Arc::new(PeerBook::from_config(&config));
        // [S2-A3-2] Hub kipi (KK-H50): dışarıdan taşıyıcı verilmemiş ∧ cihaz dosyasında `hub`.
        // Hub kipinde veri alıcısı geri döngüdedir ve `direct_addrs` gerekmez (KK-H51, KK-H63).
        let hub = match link {
            None => hub_mode::params(&config)?,
            Some(_) => None,
        };
        let data_bind = match hub {
            Some(_) => hub_mode::DATA_BIND,
            None => peers.own_addr().ok_or(NodeError::NoDirectAddress)?,
        };
        // (2) Veri dizini + depo (AppStore + CounterStore = aynı nesne).
        files::create_private_dir(data_dir).map_err(NodeError::DataDir)?;
        let received = ReceivedFiles::open(data_dir).map_err(NodeError::DataDir)?;
        let store = Arc::new(RedbStore::open(&data_dir.join(STORE_FILE_NAME))?);
        // (3) Crypto: gerçek anahtarlar + Crypto'nun saati + OS entropisi + kalıcı init_counter.
        let protection = wiring::protection(&config, &store).map_err(NodeError::PeerDirectory)?;
        // [S2-A3-2] Hub girdileri (imzacı dahil) dosya düşmeden kurulur (KK-H56).
        let hub = hub.map(|params| hub_mode::inputs(&config, params));
        // Gizli anahtar taşıyan dosya içeriği burada düşer (Drop'ta sıfırlanır).
        drop(config);
        // (4) Zarf taşıyıcısı: süreç-içi röle kaydı (kimlik başına tek canlı düğüm).
        let (protection, link, hub) = match link {
            Some(link) => (wiring::direct_crypto(protection), link, None),
            None => match hub {
                // [S2-A3-2] Hub kipi: hub zarf taşıyıcısı + tek soket (KK-H50, KK-H51).
                Some(inputs) => {
                    let started = hub_mode::start(protection, inputs)?;
                    (started.crypto, started.link, Some(started.side))
                }
                None => (
                    wiring::direct_crypto(protection),
                    carrier::product_envelope_link(peers.me())?,
                    None,
                ),
            },
        };
        let mut transport_config = ClientTransportConfig::product(data_bind);
        // [CC-PORT] Motor anahtarı: `KAMA_TRANSPORT_ENGINE=cc-port` ⇒ quinn BBR + RFC 9002 portu
        // (`RuntimeConfig::engine`). Cihaz dosyası ve `start` imzası değişmez (KK-H50); yalnız
        // ölçüm/gösterim düzeneği kurar, iki uçta AYNI olmalıdır. Boş/`legacy` = ürün (bit-eşit);
        // tanınmayan değer yapılandırma hatasıdır (sessizce legacy'ye düşmez).
        if let Ok(v) = std::env::var("KAMA_TRANSPORT_ENGINE") {
            transport_config.runtime.engine =
                client_core::TransportEngine::parse(&v).ok_or_else(|| {
                    NodeError::Config(kama_provision::format::FormatError::Invalid(
                        "KAMA_TRANSPORT_ENGINE: 'legacy' ya da 'cc-port' olmalı",
                    ))
                })?;
        }
        // [ACK-BİÇİMİ · PoC 2026-11-23] `KAMA_ACK_FORMAT=bitmap` ⇒ bitmap ACK pilotu
        // (`RuntimeConfig::ack_format`; 0x15, eş yalın PTT'yi biliyorsa 0x16). Boş/`v2` = ürün
        // varsayılanı V2 (bit-eşit). Yalnız dağıtım ayarıdır, iki uçta AYNI olmalıdır; tanınmayan
        // değer yapılandırma hatasıdır (sessizce V2'ye düşmez). Bu anahtar olmadan gerçek istemci
        // bitmap yolunu (ve ona bağlı yalın PTT'yi) hiç seçemiyordu.
        let ack_format_explicit = if let Ok(v) = std::env::var(ACK_FORMAT_ENV_VAR) {
            transport_config.runtime.ack_format = parse_ack_format(&v)?;
            true
        } else {
            false
        };
        // [TELEFON-PİLOTU] Yön hızı profili dağıtım ayarından (`KAMA_LINK_RATES` ya da
        // `<data_dir>/link-profile`; `src/link_profile.rs`). Yoksa `None`: bugünkü ürün davranışı.
        let env_rates = std::env::var(link_profile::ENV_VAR).ok();
        let (link_rates, link_source) = link_profile::load(data_dir, env_rates.as_deref())
            .map_err(|e| match e {
                link_profile::LoadError::Invalid => {
                    NodeError::Config(kama_provision::format::FormatError::Invalid(
                        "KAMA_LINK_RATES / link-profile: '<yukarı_B/s>,<aşağı_B/s>' (iki pozitif tamsayı) olmalı",
                    ))
                }
                link_profile::LoadError::Io(io) => NodeError::DataDir(io),
            })?;
        transport_config.runtime.link_rates = link_rates;
        // [PTT-BİRLEŞİK · deney] Yalnız birim testi: bu iş parçacığında kurulan düğümün Transport
        // bayrağı açılır (ürün yolunda bayrak kapalı).
        #[cfg(test)]
        if PTT_COALESCE_FOR_TEST.with(std::cell::Cell::get) {
            transport_config.runtime.ptt_coalesce = true;
        }
        // [PTT-BİRLEŞİK · pilot · #233] Dağıtım ayarı (`KAMA_PTT_PILOT` ya da
        // `<data_dir>/ptt-pilot`; `src/ptt_pilot.rs`). Yoksa kapalı: bugünkü ürün davranışı
        // (ACK V2, ayrı PTT_OPEN / PTT_CLOSE). Açıkken bitmap pilot yolu: birleşik PTT bayrağı
        // ve bitmap ACK (0x17 ve yalın 0x09 yalnız onunla vardır; iki uç da pilotla başlar).
        let ptt_pilot_env = std::env::var(ptt_pilot::ENV_VAR).ok();
        #[cfg(test)]
        let ptt_pilot_env = PTT_PILOT_FOR_TEST
            .with(|v| v.borrow().clone())
            .or(ptt_pilot_env);
        let ptt_pilot =
            ptt_pilot::load(data_dir, ptt_pilot_env.as_deref()).map_err(|e| match e {
                ptt_pilot::LoadError::Invalid => {
                    NodeError::Config(kama_provision::format::FormatError::Invalid(
                        "KAMA_PTT_PILOT / ptt-pilot: '0', '1' ya da '1:<eş>[,<eş>…]' olmalı",
                    ))
                }
                ptt_pilot::LoadError::Io(io) => NodeError::DataDir(io),
            })?;
        // Yönetilen eş adları dizinde olmalı (sessizce yok sayılmaz).
        let mut ptt_provisioned = Vec::with_capacity(ptt_pilot.provisioned.len());
        for name in &ptt_pilot.provisioned {
            ptt_provisioned.push(peers.resolve(name).map_err(|_| {
                NodeError::Config(kama_provision::format::FormatError::Invalid(
                    "KAMA_PTT_PILOT / ptt-pilot: yönetilen eş cihaz dosyasında yok",
                ))
            })?);
        }
        if ptt_pilot.enabled {
            // Pilotun kabul kipi cc-port + bitmap + doğrulanmış 300 ms PTT'dir. `KAMA_ACK_FORMAT`
            // verilmediyse bitmap seçilir; açıkça `v2` verildiyse çelişki yapılandırma hatasıdır
            // (pilot sessizce 0x17'siz / yalın PTT'siz başlamaz).
            if ack_format_explicit
                && transport_config.runtime.ack_format != client_core::runtime::AckFormat::Bitmap
            {
                return Err(NodeError::Config(
                    kama_provision::format::FormatError::Invalid(
                        "KAMA_PTT_PILOT açıkken KAMA_ACK_FORMAT 'bitmap' olmalı (ya da verilmemeli)",
                    ),
                ));
            }
            transport_config.runtime.ptt_coalesce = true;
            transport_config.runtime.ack_format = client_core::runtime::AckFormat::Bitmap;
        }
        // [İŞ-30] Pilot kaydı açıkça etkinleştirilir (`src/pilot_record.rs`); açıkken canlı teşhis
        // aynası açılır (salt gözlem), kapalıyken hiçbir şey yazılmaz.
        let pilot_env = std::env::var(pilot_record::ENV_VAR).ok();
        let pilot = pilot_record::enabled(data_dir, pilot_env.as_deref()).map_err(|_| {
            NodeError::Config(kama_provision::format::FormatError::Invalid(
                "KAMA_PILOT_RECORD: '1' (açık) ya da '0' (kapalı) olmalı",
            ))
        })?;
        if pilot {
            transport_config.runtime.live_diag = true;
        }
        let start_record = format!(
            "{}{}{}pilot_kaydı={}\n",
            core_build_line(),
            ptt_pilot::start_line(&ptt_pilot),
            link_profile::start_record(
                &transport_config.runtime,
                transport_config.lanes.is_some(),
                link_source,
            ),
            if pilot {
                "açık (canlı teşhis aynası açık; özet pilot-summary.txt)"
            } else {
                "kapalı"
            },
        );
        transport_config.peer_addrs = match &hub {
            // Hub kipinde her eşin veri adresi yerel rölenin adresidir (KK-H51).
            Some(side) => peers
                .iter()
                .map(|(_, peer)| (peer, side.relay_addr()))
                .collect(),
            None => peers.peer_addrs(),
        };
        // (5) Transport: gerçek UDP veri yolu, ürün profili.
        let transport = Transport::new(transport_config, protection, link)?;
        // Başlangıç kaydı: gerçekten kurulan taşıma yapılandırması (motor, şeritler, yön hızları,
        // koruma kipi). Saha provasında ilk okunacak dosya; anahtar/kimlik taşımaz.
        files::write_private(
            &data_dir.join(link_profile::START_RECORD_FILE),
            start_record.as_bytes(),
        )
        .map_err(NodeError::DataDir)?;
        if let Some(side) = &hub {
            side.set_server_addr(transport.data_addr());
        }
        // (6) Application motorları; Application'ın saati Crypto'nunkinden AYRI.
        let app_store: Arc<dyn AppStore> = store;
        let messenger = Messenger::with_store(transport, app_store).map_err(NodeError::Limits)?;
        let limits = InputLimits::from_amp(messenger.limits());
        let mut ptt = node::ptt_engine(&messenger, Arc::clone(&app_clock));
        // [PTT-BİRLEŞİK · pilot] Yönetilen yetenek: dağıtımın doğruladığı eşler.
        node::provision_ptt_pilot(&mut ptt, &ptt_provisioned)?;
        let engines = AppEngines {
            messenger,
            ptt,
            files: FileTransfer::new(Arc::clone(&app_clock)),
            signaling: Signaling::new(app_clock),
        };
        let shared = Arc::new(Shared::new(node::idle_metrics(&peers)));
        let recorder = if pilot {
            Some(pilot_record::PilotRecorder::start(data_dir).map_err(NodeError::DataDir)?)
        } else {
            None
        };
        let mut node = NodeLoop::new(
            engines,
            Arc::clone(&peers),
            listener,
            Arc::clone(&shared),
            received,
            hub,
        );
        node.set_pilot(recorder);
        Ok(Built {
            node,
            shared,
            peers,
            limits,
        })
    }

    /// (7) Düğüm iş parçacığı.
    fn spawn(built: Built) -> Result<Self, NodeError> {
        let Built {
            node,
            shared,
            peers,
            limits,
        } = built;
        let (commands, inbox) = mpsc::sync_channel(COMMAND_QUEUE_CAP);
        let thread = thread::Builder::new()
            .name(NODE_THREAD_NAME.to_owned())
            .spawn(move || node.run(inbox))
            .map_err(NodeError::Spawn)?;
        Ok(Self {
            commands,
            shared,
            peers,
            limits,
            node_thread: thread.thread().id(),
            thread: Some(thread),
        })
    }

    // ── API (plan K4G-11 NE 2) ───────────────────────────────────────────────────────────────

    /// Eşle oturum ister (kalıcı niyet, zaman sınırı yok — DR-CONNECT-1). İdempotent: hazır
    /// oturum ya da bekleyen niyet varsa yeni bir şey başlamaz. Sonuç olaylarla gelir
    /// (`ConnectProgress`, `Connected`, `Closed`).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::Messenger`] · kuyruk/iş parçacığı hataları
    /// ([`NodeError::Busy`], [`NodeError::Stopped`], [`NodeError::ReentrantCall`]).
    pub fn connect(&self, peer_id: &str) -> Result<(), NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        self.request(|reply| Command::Connect { peer, reply })
    }

    /// Eşe bekleyen niyeti iptal eder; `true` = niyet vardı (sonuç `Closed { reason:
    /// Abandoned(Cancelled) }` olayıyla gelir).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · kuyruk/iş parçacığı hataları.
    pub fn cancel_connect(&self, peer_id: &str) -> Result<bool, NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        self.request(|reply| Command::CancelConnect { peer, reply })
    }

    /// TEXT gönderir; eşin kalıcı `msg_id`'sini döner. `Ok` = Transport kaydı kuyruğa aldı
    /// (teslim DEĞİL); teslim `Receipt { msg_id }` ile gelir. Eşin hazır oturumu gerekir.
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::TooLarge`] (metin bir TEXT kaydına sığmıyor;
    /// kuyruğa alınmadı) · [`NodeError::Messenger`] (ör. `NoSession`) · kuyruk/iş parçacığı
    /// hataları.
    pub fn send_text(&self, peer_id: &str, text: &str) -> Result<MsgId, NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        let max = self.limits.max_text_bytes;
        if text.len() > max {
            return Err(NodeError::TooLarge {
                what: "text",
                len: text.len(),
                max,
            });
        }
        let text = text.to_owned();
        self.request(|reply| Command::SendText { peer, text, reply })
    }

    /// Dosyayı eşe gönderir; DÜĞÜM TUTAMACINI hemen döner. Dosya düğüm iş parçacığında okunur
    /// (çağıran beklemez); kuyruğa alınamazsa `FileFailed { file: Outgoing(tutamaç) }`, gelişme
    /// `FileProgress` (`Sent`, `Delivered` = eşin `FILE_RECEIPT(ok)`'u) ile gelir.
    ///
    /// **Sıra:** komut düğüm iş parçacığında bu çağrı dönmeden işlenebilir — dinleyici bu tutamacın
    /// `FileFailed` (ya da `FileProgress`) olayını, çağıran tutamacı ALMADAN önce görebilir.
    /// Dinleyici bilmediği bir `FileHandle`'ın olayını saklamalı ve tutamaç dönünce eşlemelidir
    /// (tutamaçlar kesin artandır ve yeniden kullanılmaz: olay ancak bu çağrının tutamacına
    /// aittir).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::HandlesExhausted`] · [`NodeError::Busy`] ·
    /// [`NodeError::Stopped`].
    pub fn send_file(&self, peer_id: &str, path: &Path) -> Result<FileHandle, NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        let file = FileHandle::from_raw(self.shared.handles.issue()?);
        self.enqueue(Command::SendFile {
            file,
            peer,
            path: PathBuf::from(path),
        })?;
        Ok(file)
    }

    /// Giden aktarımı iptal eder (sonuç `FileFailed { reason: Outgoing(Cancelled) }`).
    ///
    /// **Sıra:** `FILE_BEGIN` henüz Transport'a sunulmadıysa motor aktarımı bu komutta bırakır ve
    /// son olay (`FileFailed { file: Outgoing(tutamaç), reason: Outgoing(Cancelled) }`) dinleyiciye
    /// bu çağrı DÖNMEDEN verilebilir (dinleyici düğüm iş parçacığında çağrılır; `send_file`
    /// notundaki gibi eşlenmelidir). Sunulduysa yerine `FILE_ABORT(user_cancel)` sıradaki turda
    /// sunulur ve son olay çağrıdan sonra gelir.
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] · [`NodeError::File`] (ör.
    /// `AlreadySent`: son kayıt kabul edildi, alındı yine beklenir) · kuyruk/iş parçacığı
    /// hataları.
    pub fn cancel_file(&self, file: FileHandle) -> Result<(), NodeError> {
        self.request(|reply| Command::CancelFile { file, reply })
    }

    /// Basış başlar: `PTT_OPEN` (Reliable) eşin güncel oturumuna; DÜĞÜM TUTAMACINI döner.
    /// `codec` ve `bundle_ms` medya motorunundur (AMP §3 0x06; akış boyunca sabit, düğüm
    /// yorumlamaz) — plan imzasına (`ptt_open(peer_id)`) beyanlı ek: değer uydurulmaz.
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::HandlesExhausted`] · [`NodeError::Ptt`] (ör.
    /// `NoSession`, `StreamActive`) · kuyruk/iş parçacığı hataları.
    pub fn ptt_open(
        &self,
        peer_id: &str,
        codec: PttCodec,
        bundle_ms: u8,
    ) -> Result<StreamHandle, NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        self.request(|reply| Command::PttOpen {
            peer,
            codec,
            bundle_ms,
            pilot: false,
            reply,
        })
        .map(|press| press.stream)
    }

    /// [PTT-BİRLEŞİK · pilot · #233] Basış başlar ve demet süresini pilot kuralı seçer
    /// (`kama_app::ptt::Ptt::open_press`); döner: [`PttPress`] — düğüm tutamacı, akışın SEÇİLEN
    /// `bundle_ms` kodu ve birleşik biçimin kullanılıp kullanılmadığı. Medya/yakalama tarafı
    /// çerçeveleri DÖNEN süreyle demetler. Pilot açık ve eşin desteği basıştan ÖNCE doğrulanmışsa (yönetilen ya da
    /// daha önce bant içi 0x17) 300 ms'nin kodu; aksi hâlde `fallback_bundle_ms` aynen ve tel
    /// [`Self::ptt_open`] ile bayt-aynı. Basışın biçimi basış boyunca değişmez.
    ///
    /// # Errors
    /// [`Self::ptt_open`] ile aynı.
    pub fn ptt_open_press(
        &self,
        peer_id: &str,
        codec: PttCodec,
        fallback_bundle_ms: u8,
    ) -> Result<PttPress, NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        self.request(|reply| Command::PttOpen {
            peer,
            codec,
            bundle_ms: fallback_bundle_ms,
            pilot: true,
            reply,
        })
    }

    /// Ses demeti (`MEDIA_BUNDLE`, Reliable — PTT-R0). Reddedilen demeti düğüm yeniden denemez
    /// (S1 K8): çağıran bekler ve aynı demeti yeniden verir.
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] · [`NodeError::TooLarge`] ·
    /// [`NodeError::Ptt`] (ör. `Transport(Backpressure)`) · kuyruk/iş parçacığı hataları.
    pub fn ptt_push(&self, stream: StreamHandle, payload: Vec<u8>) -> Result<(), NodeError> {
        let max = self.limits.max_bundle_bytes;
        if payload.len() > max {
            return Err(NodeError::TooLarge {
                what: "ptt payload",
                len: payload.len(),
                max,
            });
        }
        self.request(|reply| Command::PttPush {
            stream,
            payload,
            timed: None,
            reply,
        })
    }

    /// [PTT-BİRLEŞİK · pilot · #233] Ses demeti, sesin oluşum anıyla: `audio_age` = demetin İLK
    /// 20 ms çerçevesinin oluşumundan bu çağrıya dek geçen süre (son tarih oluşumdan 1 s sonradır;
    /// [`Self::ptt_push`]'ta sunumdan 1 s). `last` = basışın SON demeti: birleşik akışta `END`
    /// işaretini taşır ve ardından gelen [`Self::ptt_close`] tele hiçbir şey çıkarmaz (`last`
    /// verilmezse `ptt_close` ayrı bir yüksüz `END` datagramı gönderir: +55 B tel). Birleşik
    /// olmayan akışta sıradan demettir.
    ///
    /// # Errors
    /// [`Self::ptt_push`] ile aynı.
    pub fn ptt_push_timed(
        &self,
        stream: StreamHandle,
        payload: Vec<u8>,
        audio_age: Duration,
        last: bool,
    ) -> Result<(), NodeError> {
        let max = self.limits.max_bundle_bytes;
        if payload.len() > max {
            return Err(NodeError::TooLarge {
                what: "ptt payload",
                len: payload.len(),
                max,
            });
        }
        let now = std::time::Instant::now();
        let audio_start = now.checked_sub(audio_age).unwrap_or(now);
        self.request(|reply| Command::PttPush {
            stream,
            payload,
            timed: Some((audio_start, last)),
            reply,
        })
    }

    /// Basış biter: `PTT_CLOSE`. Tutamaç HER DURUMDA kapanır; hata yalnız `PTT_CLOSE`'un
    /// Transport'a verilemediğini söyler (alıcı akışı PTT-R4 ya da oturum sonuyla bitirir).
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] · [`NodeError::Ptt`] ·
    /// kuyruk/iş parçacığı hataları.
    pub fn ptt_close(&self, stream: StreamHandle) -> Result<(), NodeError> {
        self.request(|reply| Command::PttClose { stream, reply })
    }

    /// Sinyalleşme blob'u (K4G-14b; plan K4G-14 NE (2)): opak `SIGNALING_BLOB` (AMP §3 0x09,
    /// `Reliable`) eşin güncel oturumuna; karşı uçta [`NodeEvent::SignalingReceived`]. Sesli /
    /// görüntülü aramanın kurulum ve kontrol mesajlarıdır: içerik medya motorunundur, düğüm okumaz.
    /// Eşin hazır oturumu gerekir.
    ///
    /// `Ok` = Transport kuyruğa aldı, teslim DEĞİL (S1 K8); 0x09 alındısız ve dedupsuz; içerik
    /// kendi `call_id`/sıra numarasını taşımalı; oturum değişiminde ilk kabulden 60 s içindeki
    /// bloblar yeni oturuma bir kez yeniden sunulur (AMP §3). Yeniden sunum ve ömrün budanması
    /// düğümdedir: eşin `Ready`'sinde, o eşe her `send_signaling`'de ve kuyruğu boşalan her turda
    /// (`Signaling::pump`); eş başına en çok 256 kayıt tutulur, en eskisi düşer (DUR-K4G-14a
    /// CEVAP). Reddedilen blob'u düğüm yeniden denemez (S1 K8): çağıran bekler ve aynı blob'u
    /// yeniden verir.
    ///
    /// Dönen `SignalingId` motorun kimliğidir; ayrı bir düğüm tutamacı GEREKMEZ: düğümün ömrü
    /// boyunca TEK `Signaling` motoru vardır (motor-tekil = düğüm-tekil); kimlik kesin artandır ve
    /// hiçbir zaman yeniden verilmez — eşten, oturumdan ve kaydın düşmesinden bağımsız; hiçbir
    /// çağrı onu ALMAZ, yani [`StreamHandle`] / [`FileHandle`]'ın kapattığı use-after-close yüzeyi
    /// (eş başına tel kimliğinin başka bir eşin akışına / aktarımına çözülmesi) burada yoktur.
    /// Telde yoktur ve kalıcı değildir: yeniden başlatılan düğümde kimlikler baştan başlar.
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::TooLarge`] (blob bir `SIGNALING_BLOB` kaydına
    /// sığmıyor; kuyruğa alınmadı) · [`NodeError::Signaling`] (ör. `NoSession`,
    /// `Transport(Backpressure)`) · kuyruk/iş parçacığı hataları.
    pub fn send_signaling(&self, peer_id: &str, bytes: Vec<u8>) -> Result<SignalingId, NodeError> {
        let peer = self.peers.resolve(peer_id)?;
        let max = self.limits.max_blob_bytes;
        if bytes.len() > max {
            return Err(NodeError::TooLarge {
                what: "signaling blob",
                len: bytes.len(),
                max,
            });
        }
        self.request(|reply| Command::SendSignaling { peer, bytes, reply })
    }

    /// Platform sinyali (bkz. [`PlatformSignal`]). Yanıt beklemez.
    ///
    /// # Errors
    /// [`NodeError::Busy`] · [`NodeError::Stopped`].
    pub fn on_platform_signal(&self, signal: PlatformSignal) -> Result<(), NodeError> {
        self.enqueue(Command::Signal(signal))
    }

    /// Arka plan boşaltması (`ui-api v0.3` `background.drain()`; PoC-KAPSAM-001 K6 / EK-2): yerel
    /// outbox'u ve Transport'un yeniden gönderim kuyruğunu YENİDEN POMPALAR, oturumu ve bekleyen
    /// niyeti olmayan eşlere `connect` tetikler. Hub mailbox PoC'de YOKTUR: hiçbir şey çekilmez.
    ///
    /// # Errors
    /// İlk `connect` hatası ([`NodeError::Messenger`]; diğer eşler yine denenir) · kuyruk/iş
    /// parçacığı hataları.
    pub fn drain_background(&self) -> Result<(), NodeError> {
        self.request(|reply| Command::DrainBackground { reply })
    }

    /// Son düğüm turunun ölçüleri (bloklamaz; kopyalar).
    #[must_use]
    pub fn metrics(&self) -> NodeMetrics {
        node::lock(&self.shared.metrics).clone()
    }

    // ── [K4G-05] YALNIZ test-util (INT-02 ölçüm düzeneği) ───────────────────────────────────────

    /// [DUR-19] Canlı teşhis aynası AÇIK düğüm (INT-02 ölçüm kolları): ürün yolunun AYNISI
    /// (`KamaNode::build` — ürün profili, ürün builder'ı), ardından düğüm iş parçacığı başlamadan
    /// Transport'un canlı ayna bayrağı açılır (`ClientTransport::set_live_diag_for_test(true)` →
    /// `RuntimeConfig::live_diag`; çalışma-zamanı bayrağı, feature DEĞİL — aynı ikili). Bayrak
    /// yalnız salt-gözlem yazıcısını açar; motor kararları değişmez. Ürün yolu
    /// ([`KamaNode::start`]) bayrağı KAPALI kurar: orada sondanın Transport aynası alanları
    /// ([`NodeProbe`] `sessions[].diag`, `retired`) varsayılan (sıfır) değerdedir.
    ///
    /// # Errors
    /// [`KamaNode::start`] ile aynı.
    #[cfg(feature = "test-util")]
    pub fn start_live_diag_for_test(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
    ) -> Result<Self, NodeError> {
        let app_clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
        let mut built = Self::build(config_path, data_dir, listener, app_clock)?;
        built.node.set_live_diag_for_test(true);
        Self::spawn(built)
    }

    /// recovery-OFF NEGATİF kolunun kurucusu (INT-02, plan K4G-05): ürün yolunun AYNISI
    /// (`KamaNode::build` — ürün profili, ürün builder'ı), ardından düğüm iş parçacığı başlamadan
    /// Transport'un kurtarma anahtarı kapatılır (`ClientTransport::set_recovery_for_test(false)`,
    /// S1 H-APP-RETRY-0 kancası): ARQ retransmit, §11 kopya ve FEC repair kararları tele çıkmaz
    /// (motor kararı verir, çalışma-zamanı bastırır ve `recovery_suppressed`'te sayar). Ürün
    /// builder'ı `recovery: false` yapılandırmasını reddettiği için kol bu kancayla kurulur;
    /// `TransportCapabilities::reliable` beyanı değişmez. [DUR-19] Kol bastırılan kararı canlı
    /// aynadan okur: canlı teşhis aynası da açılır ([`Self::start_live_diag_for_test`] gibi). Ürün
    /// yolu (`start`) bu yöntemden etkilenmez.
    ///
    /// # Errors
    /// [`KamaNode::start`] ile aynı.
    #[cfg(feature = "test-util")]
    pub fn start_recovery_off_for_test(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
    ) -> Result<Self, NodeError> {
        let app_clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
        let mut built = Self::build(config_path, data_dir, listener, app_clock)?;
        built.node.set_live_diag_for_test(true);
        built.node.set_recovery_for_test(false);
        Self::spawn(built)
    }

    /// Ölçüm sondası (bkz. [`NodeProbe`]): Transport ARQ / RTT aynası, bağlam-önce dağıtım
    /// düşüşleri, Application sayaçları, oturum olayları ve süreç-içi röleye verilen el sıkışma
    /// zarfları. SALT OKUMADIR; düğüm turunda tek bir komut olarak işlenir (ağ beklenmez).
    /// [DUR-19] Transport aynası alanları yalnız canlı ayna açık kurulan düğümde dolar
    /// ([`Self::start_live_diag_for_test`], [`Self::start_recovery_off_for_test`]); ürün yolunda
    /// ([`Self::start`]) varsayılan (sıfır) değerdedir.
    ///
    /// # Errors
    /// Kuyruk/iş parçacığı hataları ([`NodeError::Busy`], [`NodeError::Stopped`],
    /// [`NodeError::ReentrantCall`]).
    #[cfg(feature = "test-util")]
    pub fn probe_for_test(&self) -> Result<NodeProbe, NodeError> {
        let mut probe = self.request(|reply| Command::Probe { reply })?;
        probe.envelopes = carrier::envelope_counts_for_test(self.peers.me());
        Ok(probe)
    }

    /// [KTP2-F0 · A] İKİ SÜREÇLİ ölçüm düzeneğinin kurucusu (akış bağımsızlığı): ürün yolunun AYNISI
    /// (`KamaNode::build` gövdesi — ürün profili, ürün builder'ı, aynı kurulum sırası), yalnız zarf
    /// taşıyıcısı çağıranındır (`link`; süreç-içi röleye kayıt YAPILMAZ). El sıkışma zarfları
    /// (INIT/RESPONSE) bu taşıyıcıdan, veri yolunun DIŞINDA geçer — INT-02'deki beyanlı boşlukla
    /// aynı (DUR-K4G-11 → B); taşıyıcı ürün taşıyıcısı değildir. Düğüm iş parçacığı başlamadan
    /// `live_diag` açıksa Transport canlı teşhis aynası ([`Self::start_live_diag_for_test`] gibi),
    /// `unit_trace` açıksa Transport birim izi açılır
    /// (`ClientTransport::set_unit_trace_for_test`). İkisi de salt gözlemdir; motor kararları
    /// değişmez. Ürün yolu ([`KamaNode::start`]) bu yöntemden etkilenmez. Sondanın el sıkışma zarfı
    /// sayıları ([`NodeProbe`] `envelopes`) bu kurucuda sıfırdır (röle kaydı yok).
    ///
    /// # Errors
    /// [`KamaNode::start`] ile aynı (`IdentityInUse` hariç).
    #[cfg(feature = "test-util")]
    pub fn start_with_link_for_test(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
        link: Box<dyn EnvelopeLink>,
        live_diag: bool,
        unit_trace: bool,
    ) -> Result<Self, NodeError> {
        let app_clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
        let mut built =
            Self::build_with_link(config_path, data_dir, listener, app_clock, Some(link))?;
        if live_diag {
            built.node.set_live_diag_for_test(true);
        }
        if unit_trace {
            built.node.set_unit_trace_for_test(true);
        }
        Self::spawn(built)
    }

    /// [KTP2-F0 · A] Transport birim izinin okunmamış kayıtları (eskiden yeniye) ve son okumadan
    /// beri tavan dolu olduğu için düşen kayıt sayısı; iz boşalır (`ClientTransport::
    /// take_unit_trace_for_test`). İz yalnız [`Self::start_with_link_for_test`]'te `unit_trace`
    /// açıkken tutulur; değilse boş. Düğüm turunda tek bir komut olarak işlenir (ağ beklenmez);
    /// motor durumu değişmez.
    ///
    /// # Errors
    /// Kuyruk/iş parçacığı hataları ([`NodeError::Busy`], [`NodeError::Stopped`],
    /// [`NodeError::ReentrantCall`]).
    #[cfg(feature = "test-util")]
    pub fn take_unit_trace_for_test(
        &self,
    ) -> Result<(Vec<client_core::transport::UnitTraceRecord>, u64), NodeError> {
        self.request(|reply| Command::TakeUnitTrace { reply })
    }

    /// Düğümü kapatır: döngü biter, Transport (soketler, çalışma-zamanı) ve depo (dosya kilidi)
    /// kapanır; dönüşte aynı `data_dir` ile yeni bir düğüm açılabilir. Ağ boşaltılmaz: yolda olan
    /// aktarımlar biter, outbox depoda kalır.
    ///
    /// Kuyruktaki komutların akıbeti: kuyrukta yer varsa kapanış kuyruğun SONUNA `Shutdown` koyar
    /// ve ondan önce kuyruğa girmiş komutların HEPSİ sırayla işlenir (yanıtlıları yanıt alır,
    /// `send_file`'ınki olay doğurur). Kuyruk doluysa (`Shutdown` sığmazsa) kapanış bayrağı kalkar
    /// ve döngüyü en geç SIRADAKİ komut grubunun (en çok kuyruk tavanı kadar komut) sonunda
    /// bitirir: kuyruktaki yanıtsız komutların (`send_file`, `on_platform_signal`) bir kısmı ya da
    /// tamamı işlenebilir, işlenmeyenler olay üretmeden düşer. (Yanıt bekleyen çağıran olsaydı
    /// [`NodeError::Stopped`] alırdı; `shutdown` düğümü tükettiği ve `Drop` son referansta
    /// çalıştığı için o anda yanıt bekleyen başka çağıran olamaz.)
    ///
    /// # Errors
    /// [`NodeError::NodeThreadPanicked`].
    pub fn shutdown(mut self) -> Result<(), NodeError> {
        self.stop()
    }

    // ── iç ──

    /// Yanıt bekleyen komut: düğüm onu işleyince döner (ağ beklenmez).
    fn request<T>(&self, make: impl FnOnce(Reply<T>) -> Command) -> Result<T, NodeError> {
        if thread::current().id() == self.node_thread {
            return Err(NodeError::ReentrantCall);
        }
        let (reply, answer) = mpsc::sync_channel(REPLY_CAPACITY);
        self.enqueue(make(reply))?;
        answer.recv().map_err(|_| NodeError::Stopped)?
    }

    /// Bloklamadan kuyruğa koyar.
    fn enqueue(&self, command: Command) -> Result<(), NodeError> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => NodeError::Busy,
                TrySendError::Disconnected(_) => NodeError::Stopped,
            })
    }

    /// Kapanış (idempotent; bkz. [`KamaNode::shutdown`]). Kuyrukta yer varsa `Shutdown` kuyruğun
    /// SONUNA girer: önceki komutlar sırayla işlenir, kapanış bayrağı KALKMAZ. Kuyruk doluysa
    /// (`Shutdown` sığmadı) bayrak kalkar; döngü onu komut grubu sonlarında okur ve en geç sıradaki
    /// komut grubunun sonunda biter. Düğüm iş parçacığının kendisinden çağrılırsa (dinleyici son
    /// referansı düşürdüyse) kendini beklemez.
    fn stop(&mut self) -> Result<(), NodeError> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        match self.commands.try_send(Command::Shutdown) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.shared.stopping.store(true, Ordering::Release),
            // Döngü zaten bitti (ör. iş parçacığı paniği): sonucu `join` söyler.
            Err(TrySendError::Disconnected(_)) => {}
        }
        if thread::current().id() == self.node_thread {
            return Ok(());
        }
        thread.join().map_err(|_| NodeError::NodeThreadPanicked)
    }
}

impl Drop for KamaNode {
    fn drop(&mut self) {
        // Drop'ta panik YOK: kapanış hatası (iş parçacığı paniği) burada raporlanacak yer bulamaz.
        let _ = self.stop();
    }
}

#[cfg(test)]
mod ack_format_switch_tests {
    //! [ACK-BİÇİMİ · PoC 2026-11-23] `KAMA_ACK_FORMAT` çözümü: gerçek istemci bitmap pilotunu yalnız
    //! açık ayarla seçer; varsayılan V2'dir; tanınmayan değer sessizce V2'ye düşmez.
    use super::parse_ack_format;
    use client_core::runtime::AckFormat;

    #[test]
    fn switch_selects_bitmap_only_when_asked_and_rejects_unknown_values() {
        assert_eq!(parse_ack_format("bitmap").ok(), Some(AckFormat::Bitmap));
        assert_eq!(parse_ack_format("v2").ok(), Some(AckFormat::V2));
        assert_eq!(parse_ack_format("").ok(), Some(AckFormat::V2));
        assert!(
            parse_ack_format("bitmap2").is_err(),
            "tanınmayan değer hata olmalı"
        );
    }
}
