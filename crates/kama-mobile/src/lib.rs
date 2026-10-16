//! `kama-mobile` — UniFFI yüzeyi (K4G-15, MOBILE-HANDOFF; plan D10, D11).
//!
//! [`KamaMobile`] bileşim kökü `kama_node::KamaNode`'un (K4G-11) BİREBİR sarmalayıcısıdır: her
//! çağrı aynı adlı `KamaNode` çağrısına gider, değerler FFI aynalarına çevrilir (`src/event.rs`,
//! `src/error.rs`). Protokol mantığı burada YOKTUR (`scripts/ci/layer_check.py` C9). Kotlin tarafı
//! (Hasan'ın `RealKamaEngine`'i) ile eşleme: `docs/seams/KM-SEAM-AK-001.md`.
//!
//! # Olaylar
//!
//! Olaylar UniFFI callback interface'i [`EngineListener`] ile gelir; düğüm onu KENDİ iş
//! parçacığında çağırır (`kama_node::NodeListener` sözleşmesi): dinleyici hızlı dönmelidir.
//! Dinleyicinin içinden yanıt bekleyen çağrı [`NodeError::ReentrantCall`] alır (düğüm kendini
//! bekleyemez).
//!
//! Kotlin'in `onEvent` içinde attığı istisna UniFFI'de [`ListenerError`] değerine çevrilir ve
//! burada yutulur: olay o dinleyici için kaybolur, düğüm iş parçacığı etkilenmez. (Sonuç tipi
//! olmayan bir callback'te UniFFI bu durumda Rust tarafında panikler — düğüm iş parçacığı ölürdü;
//! Kotlin'deki imza iki durumda da `fun onEvent(ev: EngineEvent)`.)
//!
//! # Çağrılar ve kapanış
//!
//! Bütün çağrılar `KamaNode`'un kuralıyla çalışır: ağ ya da zamanlayıcı BEKLENMEZ, uzun iş
//! (dosyanın okunması) düğüm iş parçacığındadır. `start()` tokio/async bağlamından çağrılmaz (JVM
//! iş parçacığı tokio bağlamı değildir).
//!
//! `KamaNode::shutdown` düğümü TÜKETİR; FFI nesnesi ise paylaşılır (`Arc`). Bu yüzden düğüm bir
//! `RwLock<Option<_>>` yuvasında durur: çağrılar okuma kilidiyle (birbirini beklemez), `shutdown`
//! yazma kilidiyle düğümü yuvadan alır, kilidi BIRAKIR ve sonra kapatır. Kapanıştan sonra her
//! çağrı [`NodeError::Stopped`] alır; `shutdown` idempotenttir. Dinleyicinin içinden yapılan çağrı
//! kilidi BEKLEMEZ (`try_*`): kilit bir kapanış yüzünden alınamıyorsa çağrı `Stopped` (kapanış
//! sürüyor), kapanış `ReentrantCall` alır — aksi hâlde kapanışın beklediği bir çağrı düğüm iş
//! parçacığını beklerken düğüm iş parçacığı kapanışı bekler, kilitlenme olurdu. Panik FFI
//! sınırını geçmez: bu crate panik etmez (zehirli kilit içindeki değerle sürer) ve UniFFI her
//! çağrıyı ayrıca `catch_unwind` ile sarar.
//!
//! `unsafe` yalnız UniFFI'nin ürettiği iskelettedir (ADR-01 "mobil binding" istisnası); bu
//! crate'in kendi kodunda `unsafe` yoktur. `forbid(unsafe_code)` üretilen iskeleti
//! (`#[unsafe(no_mangle)]`) derletmez, o yüzden konmadı.

mod error;
mod event;

use std::cell::Cell;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use kama_node::{KamaNode, NodeListener};

pub use error::{
    AppStoreError, EncodeError, FileSendError, ListenerError, MessengerError, NodeError, PttError,
    SignalingError, TransportError,
};
pub use event::{
    AbandonReason, CloseCause, CloseReason, ConnectPhase, EngineEvent, FileAbortReason,
    FileFailReason, FileFailure, FileHandle, FileRef, FileStage, LinkState, NodeMetrics,
    PeerConnectState, PlatformSignal, PttCodec, SendFailure, StreamHandle,
};

uniffi::setup_scaffolding!("kama_mobile");

/// Olay dinleyicisi — UniFFI callback interface'i; Kotlin uygular (`interface EngineListener { fun
/// onEvent(ev: EngineEvent) }`).
#[uniffi::export(callback_interface)]
pub trait EngineListener: Send + Sync {
    /// Bir düğüm olayı. DÜĞÜM İŞ PARÇACIĞINDA çağrılır: hızlı dön (olayı bir kanala ya da
    /// `StateFlow`'a koy). İçinden `KamaMobile`'ın yanıt bekleyen çağrısı `ReentrantCall` alır.
    ///
    /// # Errors
    /// Kotlin uygulaması hata DÖNDÜRMEZ; attığı istisna UniFFI'de [`ListenerError::Unexpected`]
    /// olur ve yutulur (bkz. crate belgesi).
    fn on_event(&self, ev: EngineEvent) -> Result<(), ListenerError>;
}

/// Çalışan bir Kama düğümünün FFI nesnesi (Kotlin'de `KamaMobile`) — `kama_node::KamaNode`'un
/// birebir sarmalayıcısı. Bütün çağrılar birden çok iş parçacığından yapılabilir.
#[derive(uniffi::Object)]
pub struct KamaMobile {
    /// Düğüm; `shutdown` sonrası `None` (bkz. crate belgesi, "Çağrılar ve kapanış").
    node: RwLock<Option<KamaNode>>,
}

impl fmt::Debug for KamaMobile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Kimlik, anahtar ve eş bilgisi Debug'a GİRMEZ.
        f.debug_struct("KamaMobile").finish_non_exhaustive()
    }
}

#[uniffi::export]
impl KamaMobile {
    /// Düğümü kurar ve başlatır (`KamaNode::start`). `config_path` cihazın provisioning
    /// dosyasıdır (`device-<ad>.json`, PROVISIONING-FORMAT-v0); `data_dir` uygulamaya özel veri
    /// dizinidir, yoksa oluşturulur. Olaylar `listener`'a gelir. Async/tokio bağlamından çağrılmaz;
    /// JVM iş parçacığından çağrılır.
    ///
    /// # Errors
    /// [`NodeError::Config`] · [`NodeError::NoDirectAddress`] · [`NodeError::DataDir`] ·
    /// [`NodeError::Store`] · [`NodeError::PeerDirectory`] · [`NodeError::IdentityInUse`] ·
    /// [`NodeError::Transport`] · [`NodeError::Limits`] · [`NodeError::Spawn`].
    #[uniffi::constructor]
    pub fn start(
        config_path: String,
        data_dir: String,
        listener: Box<dyn EngineListener>,
    ) -> Result<Arc<Self>, NodeError> {
        let bridge = Box::new(ListenerBridge { listener });
        let node = KamaNode::start(Path::new(&config_path), Path::new(&data_dir), bridge)?;
        Ok(Arc::new(Self {
            node: RwLock::new(Some(node)),
        }))
    }

    /// Eşle oturum ister (kalıcı niyet; idempotent). Sonuç olaylarla gelir: `ConnectProgress`,
    /// `Connected`, `Closed`.
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::Messenger`] · kuyruk/iş parçacığı hataları
    /// ([`NodeError::Busy`], [`NodeError::Stopped`], [`NodeError::ReentrantCall`]).
    pub fn connect(&self, peer_id: String) -> Result<(), NodeError> {
        self.with_node(|node| node.connect(&peer_id))
    }

    /// Eşe bekleyen niyeti iptal eder; `true` = niyet vardı (sonuç `Closed(Abandoned(Cancelled))`).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · kuyruk/iş parçacığı hataları.
    pub fn cancel_connect(&self, peer_id: String) -> Result<bool, NodeError> {
        self.with_node(|node| node.cancel_connect(&peer_id))
    }

    /// TEXT gönderir; eşin kalıcı `msg_id`'sini döner. `Ok` = kuyruğa alındı (teslim DEĞİL); teslim
    /// `Receipt { msg_id }` ile gelir. Eşin hazır oturumu gerekir: arayüz `Connected`'ı bekler,
    /// oturumsuz çağrı `Messenger(NoSession)` alır (DUR-K4G-11b).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::TooLarge`] · [`NodeError::Messenger`] ·
    /// kuyruk/iş parçacığı hataları.
    pub fn send_text(&self, peer_id: String, text: String) -> Result<u32, NodeError> {
        self.with_node(|node| node.send_text(&peer_id, &text))
    }

    /// Dosyayı eşe gönderir; düğüm tutamacını HEMEN döner (dosya düğüm iş parçacığında okunur).
    /// Gelişme `FileProgress`, başarısızlık `FileFailed` ile gelir — olay tutamaç dönmeden önce
    /// gelebilir (`KamaNode::send_file` notu). `path` yerel dosya sistemi yoludur (`content://` URI
    /// değil).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::HandlesExhausted`] · [`NodeError::Busy`] ·
    /// [`NodeError::Stopped`].
    pub fn send_file(&self, peer_id: String, path: String) -> Result<FileHandle, NodeError> {
        self.with_node(|node| node.send_file(&peer_id, Path::new(&path)))
            .map(FileHandle::from)
    }

    /// Giden aktarımı iptal eder (sonuç `FileFailed(Outgoing(Cancelled))`).
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] · [`NodeError::File`] ·
    /// kuyruk/iş parçacığı hataları.
    pub fn cancel_file(&self, file: FileHandle) -> Result<(), NodeError> {
        self.with_node(|node| node.cancel_file(file.to_node()))
    }

    /// Basış başlar: `PTT_OPEN` (Reliable); düğüm tutamacını döner. `codec` ve `bundle_ms`
    /// çağıranın (medya motorunun) değeridir; düğüm uydurmaz ve yorumlamaz.
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::HandlesExhausted`] · [`NodeError::Ptt`] ·
    /// kuyruk/iş parçacığı hataları.
    pub fn ptt_open(
        &self,
        peer_id: String,
        codec: PttCodec,
        bundle_ms: u8,
    ) -> Result<StreamHandle, NodeError> {
        self.with_node(|node| node.ptt_open(&peer_id, codec.to_app(), bundle_ms))
            .map(StreamHandle::from)
    }

    /// Ses demeti (`MEDIA_BUNDLE`, Reliable). Reddedilen demeti düğüm yeniden denemez: çağıran
    /// bekler ve aynı demeti yeniden verir (ör. `Ptt(Transport(Backpressure))`).
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] · [`NodeError::TooLarge`] ·
    /// [`NodeError::Ptt`] · kuyruk/iş parçacığı hataları.
    pub fn ptt_push(&self, stream: StreamHandle, payload: Vec<u8>) -> Result<(), NodeError> {
        self.with_node(|node| node.ptt_push(stream.to_node(), payload))
    }

    /// Basış biter: `PTT_CLOSE`. Tutamaç HER DURUMDA kapanır.
    ///
    /// # Errors
    /// [`NodeError::HandleClosed`] / [`NodeError::UnknownHandle`] · [`NodeError::Ptt`] ·
    /// kuyruk/iş parçacığı hataları.
    pub fn ptt_close(&self, stream: StreamHandle) -> Result<(), NodeError> {
        self.with_node(|node| node.ptt_close(stream.to_node()))
    }

    /// Sinyalleşme blob'u (`SIGNALING_BLOB`, Reliable) eşin güncel oturumuna; karşı uçta
    /// `SignalingReceived`. Alındısız ve dedupsuz: içerik kendi `call_id`/sıra numarasını taşımalı;
    /// oturum değişiminde ilk kabulden 60 s içindeki bloblar yeniden sunulabilir. Dönen değer
    /// sinyalleşme motorunun yerel kimliğidir (telde yok; hiçbir çağrı onu almaz).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::TooLarge`] · [`NodeError::Signaling`] ·
    /// kuyruk/iş parçacığı hataları.
    pub fn send_signaling(&self, peer_id: String, bytes: Vec<u8>) -> Result<u64, NodeError> {
        self.with_node(|node| node.send_signaling(&peer_id, bytes))
            .map(|id| id.get())
    }

    /// Platform sinyali. Yanıt beklemez.
    ///
    /// # Errors
    /// [`NodeError::Busy`] · [`NodeError::Stopped`].
    pub fn on_platform_signal(&self, signal: PlatformSignal) -> Result<(), NodeError> {
        self.with_node(|node| node.on_platform_signal(signal.to_node()))
    }

    /// Arka plan boşaltması (`ui-api v0.3` `background.drain()`; PoC-KAPSAM-001 K6 / EK-2): yerel
    /// outbox'u ve Transport'un yeniden gönderim kuyruğunu yeniden pompalar, oturumu ve bekleyen
    /// niyeti olmayan eşlere `connect` tetikler. Hub mailbox yoktur.
    ///
    /// # Errors
    /// İlk `connect` hatası ([`NodeError::Messenger`]) · kuyruk/iş parçacığı hataları.
    pub fn drain_background(&self) -> Result<(), NodeError> {
        self.with_node(KamaNode::drain_background)
    }

    /// Son düğüm turunun ölçüleri (bloklamaz; kopyalar).
    ///
    /// # Errors
    /// [`NodeError::Stopped`] (`shutdown` sonrası; `KamaNode`'da düğüm tükendiği için bu durum
    /// yok).
    pub fn metrics(&self) -> Result<NodeMetrics, NodeError> {
        self.with_node(|node| Ok(node.metrics()))
            .map(NodeMetrics::from)
    }

    /// Düğümü kapatır (`KamaNode::shutdown`): dönüşte depo kilidi, UDP soketi ve süreç-içi röle
    /// kaydı bırakılmıştır — aynı `data_dir` ile yeni bir `start` yapılabilir. İdempotent.
    ///
    /// # Errors
    /// [`NodeError::NodeThreadPanicked`] · [`NodeError::ReentrantCall`] (dinleyicinin içinden,
    /// başka bir çağrı sürerken).
    pub fn shutdown(&self) -> Result<(), NodeError> {
        // Yazma kilidi YALNIZ düğümü yuvadan almak için tutulur; kapanış (düğüm iş parçacığının
        // beklenmesi) kilitsiz yapılır — o sırada dinleyicinin çağrıları `Stopped` alır.
        let node = self.write_slot()?.take();
        match node {
            Some(node) => node.shutdown().map_err(NodeError::from),
            None => Ok(()),
        }
    }
}

impl KamaMobile {
    /// Düğüm çağrısı: okuma kilidi altında; düğüm kapandıysa [`NodeError::Stopped`].
    fn with_node<T>(
        &self,
        call: impl FnOnce(&KamaNode) -> Result<T, kama_node::NodeError>,
    ) -> Result<T, NodeError> {
        let slot = self.read_slot()?;
        let node = slot.as_ref().ok_or(NodeError::Stopped)?;
        call(node).map_err(NodeError::from)
    }

    /// Okuma kilidi. Dinleyicinin içinden BEKLEMEZ: yazar (kapanış) varsa [`NodeError::Stopped`].
    fn read_slot(&self) -> Result<RwLockReadGuard<'_, Option<KamaNode>>, NodeError> {
        if !in_listener() {
            return Ok(self.node.read().unwrap_or_else(PoisonError::into_inner));
        }
        match self.node.try_read() {
            Ok(slot) => Ok(slot),
            Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => Err(NodeError::Stopped),
        }
    }

    /// Yazma kilidi. Dinleyicinin içinden BEKLEMEZ: süren çağrı varsa [`NodeError::ReentrantCall`].
    fn write_slot(&self) -> Result<RwLockWriteGuard<'_, Option<KamaNode>>, NodeError> {
        if !in_listener() {
            return Ok(self.node.write().unwrap_or_else(PoisonError::into_inner));
        }
        match self.node.try_write() {
            Ok(slot) => Ok(slot),
            Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => Err(NodeError::ReentrantCall),
        }
    }
}

thread_local! {
    /// Bu iş parçacığı şu an bir [`EngineListener::on_event`] çağrısının içinde mi (düğüm iş
    /// parçacığı). Kilit beklemeyi dinleyicinin içinden yasaklamak için.
    static IN_LISTENER: Cell<bool> = const { Cell::new(false) };
}

/// Dinleyici çağrısının kapsamı: girerken bayrağı kaldırır, çıkarken önceki değere döner. Hiçbir
/// yolda panik etmez (iş parçacığı yerel değeri yok ediliyorsa sessizce geçer).
struct ListenerScope {
    outer: bool,
}

impl ListenerScope {
    fn enter() -> Self {
        let outer = IN_LISTENER
            .try_with(|flag| flag.replace(true))
            .unwrap_or(false);
        Self { outer }
    }
}

impl Drop for ListenerScope {
    fn drop(&mut self) {
        let outer = self.outer;
        let _ = IN_LISTENER.try_with(|flag| flag.set(outer));
    }
}

fn in_listener() -> bool {
    IN_LISTENER.try_with(Cell::get).unwrap_or(false)
}

/// `kama_node::NodeListener` → [`EngineListener`] köprüsü: olayı aynaya çevirir ve dinleyiciyi
/// kapsam bayrağıyla çağırır.
struct ListenerBridge {
    listener: Box<dyn EngineListener>,
}

impl NodeListener for ListenerBridge {
    fn on_event(&self, event: kama_node::NodeEvent) {
        let event = EngineEvent::from(event);
        let _scope = ListenerScope::enter();
        // Dinleyicinin hatası (Kotlin istisnası) olayı yalnız bu dinleyici için düşürür; düğüm turu
        // sürer. Neden metni `ListenerError`'a hiç girmez (bkz. `error.rs`).
        let _ = self.listener.on_event(event);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Her olayda hata döndüren (Kotlin'in istisna attığı) dinleyici; çağrıları ve bayrağı
    /// kaydeder.
    struct Throwing {
        calls: Arc<AtomicUsize>,
        flagged: Arc<AtomicUsize>,
    }

    impl EngineListener for Throwing {
        fn on_event(&self, _ev: EngineEvent) -> Result<(), ListenerError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if in_listener() {
                self.flagged.fetch_add(1, Ordering::SeqCst);
            }
            Err(ListenerError::Unexpected)
        }
    }

    #[test]
    fn a_failing_listener_is_absorbed_and_the_scope_flag_is_restored() {
        const EVENTS: usize = 2;
        let calls = Arc::new(AtomicUsize::new(0));
        let flagged = Arc::new(AtomicUsize::new(0));
        let bridge = ListenerBridge {
            listener: Box::new(Throwing {
                calls: Arc::clone(&calls),
                flagged: Arc::clone(&flagged),
            }),
        };
        for _ in 0..EVENTS {
            NodeListener::on_event(
                &bridge,
                kama_node::NodeEvent::Connected {
                    peer_id: "bob".to_owned(),
                },
            );
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            EVENTS,
            "hata sonraki olayı engellemez"
        );
        assert_eq!(
            flagged.load(Ordering::SeqCst),
            EVENTS,
            "dinleyici kapsam bayrağıyla çağrılır"
        );
        assert!(!in_listener(), "kapsam bitince bayrak iner");
    }
}
