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
//! 4. Zarf taşıyıcısı (INIT/RESPONSE): **DUR-K4G-11** — ürün gerçeklemesi yok; tek kurucu tipli
//!    hata döner ([`ENVELOPE_LINK_PENDING`]; `src/carrier.rs`).
//! 5. Transport: `client_core::transport::ClientTransport`, ürün profili
//!    (`ClientTransportConfig::product`), veri alıcısı bu cihazın `direct_addrs` adresinde, eş
//!    adresleri dosyadan (direct-mode; hub yok).
//! 6. Application: `Messenger::with_store` + `Ptt` + `FileTransfer`; ikisine Application'ın saati
//!    (`kama_app::clock::SystemClock`) — Crypto'nunkinden AYRI (INV-NEIGHBOR-1; sahip kararı
//!    CEVAP PAKETİ 2 madde 3).
//! 7. Düğüm iş parçacığı (`std::thread`; yeni async runtime YOK): motorların tek sahibi; periyodik
//!    tur ve olay zinciri `src/node.rs` modül belgesinde.
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
//!
//! # KAPSAM DIŞI (plan §4.5)
//!
//! Hub, heartbeat (D18; `last_heartbeat_age_s` daima `None`), signaling (`send_signaling` /
//! `SignalingReceived` — K4G-14b uzantısı), `BestEffort` / `DeadlineBounded` (K4G-10 bu sprintte
//! yok; `deadline_expired_drops` alanı yok), mesaj geçmişi, at-rest şifreleme.

#![forbid(unsafe_code)]

mod carrier;
mod error;
mod event;
mod files;
mod handle;
mod node;
mod peers;
#[cfg(test)]
mod test_support;
mod wiring;

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
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
use kama_app::ptt::{max_bundle_payload_bytes, Ptt};
use kama_app::store::AppStore;
use kama_contracts::handles::PeerRef;
use kama_provision::format::load_device_config;
use kama_store::RedbStore;

pub use error::NodeError;
pub use event::{
    CloseCause, FileFailReason, FileRef, FileStage, LinkState, NodeEvent, NodeMetrics,
    PeerConnectState,
};
pub use files::RECEIVED_FILES_DIR;
pub use handle::{FileHandle, StreamHandle};

use files::ReceivedFiles;
use node::{Command, NodeLoop, Reply, Shared, COMMAND_QUEUE_CAP, REPLY_CAPACITY};
use peers::PeerBook;
use wiring::Transport;

/// Kalıcı deponun `data_dir` altındaki dosyası (redb; şema `kama-store` v0).
pub const STORE_FILE_NAME: &str = "kama-node.redb";

/// Ürün zarf taşıyıcısının bekleme noktası (DUR-K4G-11): [`KamaNode::start`] bugün
/// `NodeError::FeatureNotImplemented(ENVELOPE_LINK_PENDING)` döner.
pub const ENVELOPE_LINK_PENDING: &str = "envelope_link (DUR-K4G-11)";

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
}

impl InputLimits {
    fn from_amp(limits: &AmpLimits) -> Self {
        Self {
            max_text_bytes: limits.max_body_bytes().saturating_sub(TEXT_PREFIX_BYTES),
            max_bundle_bytes: max_bundle_payload_bytes(limits),
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

impl KamaNode {
    /// Düğümü kurar ve başlatır (bkz. crate belgesi, "Kurulum"). `config_path` cihazın
    /// `device-<ad>.json`'udur; `data_dir` yoksa oluşturulur (Unix'te `0700`).
    ///
    /// # Errors
    /// [`NodeError::Config`] · [`NodeError::NoDirectAddress`] · [`NodeError::DataDir`] ·
    /// [`NodeError::Store`] · [`NodeError::PeerDirectory`] · [`NodeError::FeatureNotImplemented`]
    /// (bugün daima: ürün zarf taşıyıcısı — DUR-K4G-11) · [`NodeError::Transport`] ·
    /// [`NodeError::Limits`] · [`NodeError::Spawn`]. Hata yolunda açılan her şey (depo, soket)
    /// kapanır.
    pub fn start(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
    ) -> Result<Self, NodeError> {
        Self::compose(
            config_path,
            data_dir,
            listener,
            carrier::product_envelope_link,
        )
    }

    /// YALNIZ test (`test-util`): [`KamaNode::start`] ile AYNI kurulum, yalnız zarf taşıyıcısı
    /// dışarıdan verilir (`link(bu cihazın PeerRef'i)`). DUR-K4G-11 çözülene dek CORE testi gerçek
    /// kimlik + depo + saat + entropi + UDP + motor yolunu bununla koşar.
    ///
    /// # Errors
    /// [`KamaNode::start`] ile aynı (taşıyıcı hatası hariç).
    #[cfg(any(test, feature = "test-util"))]
    pub fn start_with_envelope_link(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
        link: impl FnOnce(PeerRef) -> Box<dyn EnvelopeLink>,
    ) -> Result<Self, NodeError> {
        Self::compose(config_path, data_dir, listener, move |me, _, _| {
            Ok(link(me))
        })
    }

    /// Kurulum + başlatma. Application saati üründe `kama_app::clock::SystemClock`'tur.
    fn compose(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
        carrier: impl FnOnce(
            PeerRef,
            SocketAddr,
            &HashMap<PeerRef, SocketAddr>,
        ) -> Result<Box<dyn EnvelopeLink>, NodeError>,
    ) -> Result<Self, NodeError> {
        let app_clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
        Self::spawn(Self::build(
            config_path,
            data_dir,
            listener,
            carrier,
            app_clock,
        )?)
    }

    /// Kurulumun TEK gövdesi (iş parçacığı BAŞLATILMAZ); taşıyıcı tek bir kurucu olarak girer (bkz.
    /// `src/carrier.rs`). Sıra: provisioning → veri dizini → depo → Crypto (aynı `Arc` depo
    /// `CounterStore` olarak) → `drop(config)` → taşıyıcı → Transport → Application motorları
    /// (aynı `Arc` depo `AppStore` olarak; `app_clock` `Ptt` ve `FileTransfer`'ın ortak saati —
    /// Crypto'nunkinden AYRI). Birim testleri döngüyü elle sürmek için bunu doğrudan kullanır.
    pub(crate) fn build(
        config_path: &Path,
        data_dir: &Path,
        listener: Box<dyn NodeListener>,
        carrier: impl FnOnce(
            PeerRef,
            SocketAddr,
            &HashMap<PeerRef, SocketAddr>,
        ) -> Result<Box<dyn EnvelopeLink>, NodeError>,
        app_clock: Arc<dyn MonotonicClock>,
    ) -> Result<Built, NodeError> {
        // (1) Provisioning — yalnız bu dosya; kimlik/anahtar/eş dizini bellekte (D8, D9).
        let config = load_device_config(config_path)?;
        let peers = Arc::new(PeerBook::from_config(&config));
        let data_bind = peers.own_addr().ok_or(NodeError::NoDirectAddress)?;
        // (2) Veri dizini + depo (AppStore + CounterStore = aynı nesne).
        files::create_private_dir(data_dir).map_err(NodeError::DataDir)?;
        let received = ReceivedFiles::open(data_dir).map_err(NodeError::DataDir)?;
        let store = Arc::new(RedbStore::open(&data_dir.join(STORE_FILE_NAME))?);
        // (3) Crypto: gerçek anahtarlar + Crypto'nun saati + OS entropisi + kalıcı init_counter.
        let protection = wiring::protection(&config, &store).map_err(NodeError::PeerDirectory)?;
        // Gizli anahtar taşıyan dosya içeriği burada düşer (Drop'ta sıfırlanır).
        drop(config);
        // (4) Zarf taşıyıcısı — DUR-K4G-11.
        let mut transport_config = ClientTransportConfig::product(data_bind);
        transport_config.peer_addrs = peers.peer_addrs();
        let link = carrier(peers.me(), data_bind, &transport_config.peer_addrs)?;
        // (5) Transport: gerçek UDP veri yolu, ürün profili.
        let transport = Transport::new(transport_config, protection, link)?;
        // (6) Application motorları; Application'ın saati Crypto'nunkinden AYRI.
        let app_store: Arc<dyn AppStore> = store;
        let messenger = Messenger::with_store(transport, app_store).map_err(NodeError::Limits)?;
        let limits = InputLimits::from_amp(messenger.limits());
        let ptt = Ptt::new(Arc::clone(&app_clock));
        let files = FileTransfer::new(app_clock);
        let shared = Arc::new(Shared::new(node::idle_metrics(&peers)));
        let node = NodeLoop::new(
            messenger,
            ptt,
            files,
            Arc::clone(&peers),
            listener,
            Arc::clone(&shared),
            received,
        );
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
