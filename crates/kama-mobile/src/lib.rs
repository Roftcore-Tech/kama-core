//! `kama-mobile` — UniFFI yüzeyi (K4G-15, MOBILE-HANDOFF; plan D10, D11).
//!
//! [`KamaMobile`] bileşim kökü `kama_node::KamaNode`'un (K4G-11) BİREBİR sarmalayıcısıdır: her
//! çağrı aynı adlı `KamaNode` çağrısına gider, değerler FFI aynalarına çevrilir (`src/event.rs`,
//! `src/error.rs`). Protokol mantığı burada YOKTUR: `kama-app` ve `kama-contracts` yalnız tip
//! dönüşümü için anılır — yollarında yalnız tip ve varyant adları (`scripts/ci/layer_check.py` C9;
//! DUR-K4G-15-A). Kotlin tarafı (Hasan'ın `RealKamaEngine`'i) ile eşleme:
//! `docs/seams/KM-SEAM-AK-001.md`.
//!
//! # Olaylar
//!
//! Olaylar UniFFI callback interface'i [`EngineListener`] ile gelir; düğüm onu KENDİ iş
//! parçacığında çağırır (`kama_node::NodeListener` sözleşmesi): dinleyici hızlı dönmelidir.
//! Dinleyicinin içinden yapılan çağrı hiçbir kilidi beklemez; yanıt bekleyen çağrı
//! [`NodeError::ReentrantCall`] alır (düğüm kendini bekleyemez). Koruma yalnız aynı düğüm içindir;
//! dinleyiciden başka düğüme engelleyici çağrı yapma — iki düğümün dinleyicileri birbirini
//! beklerse ikisi de kilitlenir.
//!
//! Kotlin'in `onEvent` içinde attığı `kotlin.Exception` alt sınıfları UniFFI'de [`ListenerError`]
//! değerine çevrilir ve burada yutulur: olay o dinleyici için kaybolur, düğüm iş parçacığı
//! etkilenmez. (Sonuç tipi olmayan bir callback'te UniFFI bu durumda Rust tarafında panikler —
//! düğüm iş parçacığı ölürdü; Kotlin'deki imza iki durumda da `fun onEvent(ev: EngineEvent)`.)
//! `kotlin.Error` alt sınıflarını (ör. `NotImplementedError`, `OutOfMemoryError`) UniFFI
//! yakalamaz: JNA'nın geri çağırma işleyicisine düşerler (varsayılanı mesajı ve yığını loglar) ve
//! Rust tarafı olayı teslim edilmiş sayar. Bu yüzden README-HASAN `onEvent` gövdesinin
//! `Throwable` yakalamasını ister.
//!
//! # Kapanış
//!
//! `KamaNode::shutdown` düğümü TÜKETİR; FFI nesnesi ise paylaşılır (`Arc`). Bu yüzden düğüm bir
//! `RwLock<Option<_>>` yuvasında durur: çağrılar okuma kilidiyle (birbirini beklemez), `shutdown`
//! yazma kilidiyle düğümü yuvadan alır, kilidi BIRAKIR ve sonra kapatır. Kapanış serileştirilir
//! (`Lifecycle`: çalışıyor → [istendi] → kapanıyor → durdu):
//!
//! * JVM iş parçacığından `shutdown` TAM DURUŞU bekler: döndüğünde düğüm döngüsü bitmiş, depo
//!   kilidi, UDP soketi ve süreç-içi röle kaydı bırakılmıştır — aynı `data_dir` ile yeni `start`
//!   yapılabilir. Eşzamanlı ya da sonraki her `shutdown` da ilk kapanışın bitişini bekler ve onun
//!   sonucunu döner.
//! * Dinleyicinin içinden `shutdown` ASENKRONDUR, hiçbir kilidi beklemez ve kapanışı HER DURUMDA
//!   ister: yuva serbestse düğümü alır (düğüm iş parçacığı kendini beklemez; dinleyici döndükten
//!   sonra kuyruktaki komutları işleyip durur); yuva o an başka bir çağrıda tutuluyorsa (ör. JVM'de
//!   yanıt bekleyen `send_text`) JVM yolu kısa ömürlü bir iş parçacığına devredilir; kapanış zaten
//!   istendiyse ya da sürüyorsa onun durumu döner. Tek hata: o iş parçacığı başlatılamazsa
//!   [`NodeError::Spawn`] (kapanış istenmedi). Tam duruşu JVM iş parçacığından yapılan bir
//!   `shutdown` bekleyerek öğrenir.
//! * Dinleyicinin içinden BAŞKA bir düğümün `shutdown`'ı SENKRONDUR: o düğümün yuvası serbestse
//!   iş parçacığını join eder (dinleyici o süre bekler).
//! * Kapanış başladıktan (düğüm yuvadan alındıktan) sonra her çağrı [`NodeError::Stopped`] alır;
//!   `shutdown` idempotenttir.
//!
//! Kilit bekleme: yanıt bekleyen çağrılar JVM iş parçacığında süren bir kapanışın düğümü yuvadan
//! almasını bekleyebilir (en çok süren çağrıların bitişi kadar; sonra `Stopped`). Bloklamayan
//! çağrılar (`metrics`, `send_file`, `on_platform_signal`) HİÇBİR iş parçacığında yuva kilidini ve
//! düğüm turunu beklemez (`try_read`; kapanış sürüyorsa `Stopped`); `metrics` yalnız ölçü anlık
//! görüntüsünün kısa Mutex'ini alır. Dinleyicinin içinden bütün çağrılar `try_*` kullanır: kilit
//! bir kapanış yüzünden alınamıyorsa çağrı `Stopped` alır — aksi hâlde kapanışın beklediği bir
//! çağrı düğüm iş parçacığını beklerken düğüm iş parçacığı kapanışı bekler, kilitlenme olurdu.
//! Panik FFI sınırını geçmez: bu crate panik etmez (zehirli kilit içindeki değerle sürer) ve UniFFI
//! her çağrıyı ayrıca `catch_unwind` ile sarar.
//!
//! `unsafe` yalnız UniFFI'nin ürettiği iskelettedir (ADR-01 "mobil binding" istisnası); bu
//! crate'in kendi kodunda `unsafe` yoktur. `forbid(unsafe_code)` üretilen iskeleti
//! (`#[unsafe(no_mangle)]`) derletmez, o yüzden konmadı.

mod error;
mod event;
#[cfg(test)]
mod test_support;

use std::cell::Cell;
use std::fmt;
use std::path::Path;
use std::sync::{
    Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, TryLockError,
};
use std::thread;

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

/// Dinleyicinin içinden, yuva meşgulken istenen kapanışı devralan iş parçacığının adı (Linux'ta 15
/// bayta sığar).
const STOP_THREAD_NAME: &str = "kama-stop";

/// Olay dinleyicisi — UniFFI callback interface'i; Kotlin uygular (`interface EngineListener { fun
/// onEvent(ev: EngineEvent) }`).
#[uniffi::export(callback_interface)]
pub trait EngineListener: Send + Sync {
    /// Bir düğüm olayı. DÜĞÜM İŞ PARÇACIĞINDA çağrılır: hızlı dön (olayı bir kanala ya da
    /// `StateFlow`'a koy). İçinden aynı `KamaMobile`'ın yanıt bekleyen çağrısı `ReentrantCall`
    /// alır, `shutdown` asenkrondur. Koruma yalnız aynı düğüm için; dinleyiciden başka düğüme
    /// engelleyici çağrı yapma (başka düğümün `shutdown`'ı da senkrondur).
    ///
    /// # Errors
    /// Kotlin uygulaması hata DÖNDÜRMEZ; attığı `kotlin.Exception` UniFFI'de
    /// [`ListenerError::Unexpected`] olur ve yutulur (bkz. crate belgesi).
    fn on_event(&self, ev: EngineEvent) -> Result<(), ListenerError>;
}

/// Düğüm yuvası: kapanış başlayınca `None` (bkz. crate belgesi, "Kapanış").
type Slot = RwLock<Option<KamaNode>>;

/// Çalışan bir Kama düğümünün FFI nesnesi (Kotlin'de `KamaMobile`) — `kama_node::KamaNode`'un
/// birebir sarmalayıcısı. Bütün çağrılar birden çok iş parçacığından yapılabilir.
#[derive(uniffi::Object)]
pub struct KamaMobile {
    /// Düğüm yuvası. `Arc`: dinleyicinin içinden yuva meşgulken istenen kapanış, yuvayı kısa
    /// ömürlü bir iş parçacığına devreder.
    node: Arc<Slot>,
    /// Kapanışın aşaması; dinleyici köprüsüyle paylaşılır (düğüm döngüsünün bitişi).
    life: Arc<Lifecycle>,
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
        let life = Arc::new(Lifecycle::new());
        let bridge = Box::new(ListenerBridge {
            listener,
            life: Arc::clone(&life),
        });
        let node = KamaNode::start(Path::new(&config_path), Path::new(&data_dir), bridge)?;
        Ok(Arc::new(Self {
            node: Arc::new(RwLock::new(Some(node))),
            life,
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
    /// gelebilir (`KamaNode::send_file` notu). Oturumsuz çağrı hata DEĞİL olaydır:
    /// `FileFailed(Outgoing(tutamaç), NotQueued(NoSession))`. `path` yerel dosya sistemi yoludur
    /// (`content://` URI değil). Yuva kilidini ve düğüm turunu beklemez (kapanış sürüyorsa
    /// `Stopped`).
    ///
    /// # Errors
    /// [`NodeError::UnknownPeer`] · [`NodeError::HandlesExhausted`] · [`NodeError::Busy`] ·
    /// [`NodeError::Stopped`].
    pub fn send_file(&self, peer_id: String, path: String) -> Result<FileHandle, NodeError> {
        self.with_node_now(|node| node.send_file(&peer_id, Path::new(&path)))
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
        // `SignalingId::get` tip dönüşümüdür (DUR-K4G-15-C CEVAP (a); C9 izin listesi).
        self.with_node(|node| node.send_signaling(&peer_id, bytes))
            .map(kama_app::signaling::SignalingId::get)
    }

    /// Platform sinyali. Yanıt beklemez; yuva kilidini ve düğüm turunu beklemez (kapanış sürüyorsa
    /// `Stopped`).
    ///
    /// # Errors
    /// [`NodeError::Busy`] · [`NodeError::Stopped`].
    pub fn on_platform_signal(&self, signal: PlatformSignal) -> Result<(), NodeError> {
        self.with_node_now(|node| node.on_platform_signal(signal.to_node()))
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

    /// Son düğüm turunun ölçüleri (kopyalar). Yuva kilidini ve düğüm turunu beklemez; yalnız ölçü
    /// anlık görüntüsünün kısa Mutex'ini alır.
    ///
    /// # Errors
    /// [`NodeError::Stopped`]: kapanış başladı ya da sürüyor (`KamaNode`'da düğüm tükendiği için
    /// bu durum yok).
    pub fn metrics(&self) -> Result<NodeMetrics, NodeError> {
        self.with_node_now(|node| Ok(node.metrics()))
            .map(NodeMetrics::from)
    }

    /// Düğümü kapatır (`KamaNode::shutdown`; bkz. crate belgesi, "Kapanış"). İdempotent.
    ///
    /// * JVM iş parçacığından TAM DURUŞU bekler: dönüşte düğüm döngüsü bitmiş, depo kilidi, UDP
    ///   soketi ve süreç-içi röle kaydı bırakılmıştır — aynı `data_dir` ile yeni `start`
    ///   yapılabilir. Eşzamanlı ya da sonraki çağrılar da ilk kapanışın bitişini bekler ve onun
    ///   sonucunu döner.
    /// * Dinleyicinin içinden ASENKRONDUR ve kapanışı her durumda ister (yuva meşgulse kısa ömürlü
    ///   bir iş parçacığıyla); beklemeden döner, düğüm dinleyici döndükten sonra durur. Başka bir
    ///   düğümün `shutdown`'ı dinleyiciden senkrondur (join).
    ///
    /// # Errors
    /// [`NodeError::NodeThreadPanicked`] · [`NodeError::Spawn`] (dinleyicinin içinden, yuva
    /// meşgulken kapanışı devralacak iş parçacığı başlatılamadı — kapanış istenmedi).
    pub fn shutdown(&self) -> Result<(), NodeError> {
        if !in_listener() {
            return stop_and_wait(&self.node, &self.life);
        }
        // Dinleyicinin içinden hiçbir kilit beklenmez: düğüm iş parçacığı kendini bekleyemez.
        let mut slot = match self.node.try_write() {
            Ok(slot) => slot,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return self.request_stop(),
        };
        let taken = take_node(&mut slot, &self.life);
        // Yuva kilidi kapanıştan (başka düğümde join) önce bırakılır.
        drop(slot);
        match taken {
            Some(node) => finish_stop(node, &self.life),
            None => self.life.current(),
        }
    }
}

impl KamaMobile {
    /// Yanıt bekleyen çağrılar: JVM iş parçacığında süren bir kapanışın düğümü yuvadan almasını
    /// bekleyebilir; dinleyicinin içinden BEKLEMEZ (`try_read_slot`).
    fn with_node<T>(
        &self,
        call: impl FnOnce(&KamaNode) -> Result<T, kama_node::NodeError>,
    ) -> Result<T, NodeError> {
        let slot = if in_listener() {
            self.try_read_slot()?
        } else {
            self.node.read().unwrap_or_else(PoisonError::into_inner)
        };
        call_node(slot.as_ref(), call)
    }

    /// Bloklamayan çağrılar (`metrics`, `send_file`, `on_platform_signal`): HİÇBİR iş parçacığında
    /// yuva kilidini beklemez.
    fn with_node_now<T>(
        &self,
        call: impl FnOnce(&KamaNode) -> Result<T, kama_node::NodeError>,
    ) -> Result<T, NodeError> {
        let slot = self.try_read_slot()?;
        call_node(slot.as_ref(), call)
    }

    /// Okuma kilidi, BEKLEMEDEN: yazar (kapanış — tek yazar odur) kilidi tutuyor ya da bekliyorsa
    /// [`NodeError::Stopped`].
    fn try_read_slot(&self) -> Result<RwLockReadGuard<'_, Option<KamaNode>>, NodeError> {
        match self.node.try_read() {
            Ok(slot) => Ok(slot),
            Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => Err(NodeError::Stopped),
        }
    }

    /// Dinleyicinin içinden, yuva başka bir çağrıda (okuyucu) ya da bir JVM kapanışında (yazar)
    /// tutulurken. Kapanış zaten istendiyse ya da sürüyorsa onun durumu döner; istenmediyse JVM
    /// yolu (`stop_and_wait`) kısa ömürlü bir iş parçacığına devredilir: kapanış BEKLEMEDEN
    /// istenir, yuva serbest kalınca o iş parçacığı düğümü alır ve kapatır; sonucu sonraki bir
    /// `shutdown` döner.
    fn request_stop(&self) -> Result<(), NodeError> {
        if !self.life.request() {
            return self.life.current();
        }
        let (slot, life) = (Arc::clone(&self.node), Arc::clone(&self.life));
        let spawned = thread::Builder::new()
            .name(STOP_THREAD_NAME.to_owned())
            .spawn(move || {
                let _ = stop_and_wait(&slot, &life);
            });
        match spawned {
            Ok(_detached) => Ok(()),
            Err(error) => {
                self.life.withdraw();
                Err(NodeError::Spawn {
                    reason: error.to_string(),
                })
            }
        }
    }
}

/// Yuvadaki düğümü çağırır; düğüm yoksa (kapanış başladı) [`NodeError::Stopped`].
fn call_node<T>(
    node: Option<&KamaNode>,
    call: impl FnOnce(&KamaNode) -> Result<T, kama_node::NodeError>,
) -> Result<T, NodeError> {
    let node = node.ok_or(NodeError::Stopped)?;
    call(node).map_err(NodeError::from)
}

/// JVM yolu: yazma kilidini bekler, düğümü alır ve TAM DURUŞU bekler; yuva boşsa ilk kapanışın
/// bitişini bekler.
fn stop_and_wait(slot: &Slot, life: &Lifecycle) -> Result<(), NodeError> {
    let mut guard = slot.write().unwrap_or_else(PoisonError::into_inner);
    let taken = take_node(&mut guard, life);
    // Yuva kilidi kapanıştan (join) önce bırakılır: o sırada çağrılar `Stopped` alır.
    drop(guard);
    match taken {
        Some(node) => finish_stop(node, life),
        None => life.wait_stopped(),
    }
}

/// Yuvadaki düğümü kapanış için alır; aldıysa aşamayı işaretler. Yazma kilidi altında çağrılır:
/// yuva boşken aşama hiçbir an `Running` / `Requested` görünmez.
fn take_node(slot: &mut Option<KamaNode>, life: &Lifecycle) -> Option<KamaNode> {
    let node = slot.take();
    if node.is_some() {
        life.begin();
    }
    node
}

/// `KamaNode::shutdown`: düğüm iş parçacığının kendisinden çağrılırsa beklemez (döngü sonra
/// biter), başka bir iş parçacığından join eder. Sonuç aşamaya yazılır.
fn finish_stop(node: KamaNode, life: &Lifecycle) -> Result<(), NodeError> {
    let result = node.shutdown().map_err(NodeError::from);
    life.finish(result.clone());
    result
}

/// Kapanışın aşaması (bkz. crate belgesi, "Kapanış").
enum Phase {
    /// Düğüm yuvada.
    Running,
    /// Dinleyicinin içinden kapanış istendi; yardımcı iş parçacığı yuvayı bekliyor (düğüm hâlâ
    /// yuvada, çağrılar sürer).
    Requested,
    /// Bir `shutdown` düğümü yuvadan aldı; `KamaNode::shutdown` sürüyor.
    Stopping,
    /// `KamaNode::shutdown` döndü ama düğüm döngüsü henüz bitmedi (dinleyicinin içinden kapanış:
    /// düğüm iş parçacığı kendini beklemez). Döngü bitince `Stopped`.
    Detached(Result<(), NodeError>),
    /// Düğüm döngüsü bitti ve kaynaklar bırakıldı; ilk kapanışın sonucu.
    Stopped(Result<(), NodeError>),
}

/// `Lifecycle`'ın kilit altındaki durumu.
struct LifeState {
    /// Aşama.
    phase: Phase,
    /// Düğüm döngüsü bitti (`ListenerBridge` düştü).
    loop_ended: bool,
}

/// Kapanışın serileştirilmesi: JVM iş parçacığından gelen `shutdown` çağıranları `Stopped`
/// aşamasını bekler. Hiçbir yolda panik etmez (zehirli kilit içindeki değerle sürer).
struct Lifecycle {
    state: Mutex<LifeState>,
    changed: Condvar,
}

impl Lifecycle {
    fn new() -> Self {
        Self {
            state: Mutex::new(LifeState {
                phase: Phase::Running,
                loop_ended: false,
            }),
            changed: Condvar::new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, LifeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Dinleyicinin içinden, yuva meşgulken kapanış isteği: `Running` → `Requested`. İlk istek mi
    /// (yalnız ilk istek yardımcı iş parçacığı açar).
    fn request(&self) -> bool {
        let mut state = self.state();
        let first = matches!(state.phase, Phase::Running);
        if first {
            state.phase = Phase::Requested;
        }
        first
    }

    /// İstek yerine getirilemedi (yardımcı iş parçacığı başlatılamadı): `Requested` → `Running`.
    fn withdraw(&self) {
        let mut state = self.state();
        if matches!(state.phase, Phase::Requested) {
            state.phase = Phase::Running;
        }
    }

    /// Bir `shutdown` düğümü yuvadan aldı (yuvanın yazma kilidi altında çağrılır: yuva boşken
    /// aşama hiçbir an `Running` / `Requested` görünmez).
    fn begin(&self) {
        self.state().phase = Phase::Stopping;
    }

    /// `KamaNode::shutdown` döndü: döngü bittiyse durdu; bitmediyse (dinleyicinin içinden
    /// kapanış) döngünün bitişi beklenir.
    fn finish(&self, result: Result<(), NodeError>) {
        let mut state = self.state();
        state.phase = if state.loop_ended {
            Phase::Stopped(result)
        } else {
            Phase::Detached(result)
        };
        drop(state);
        self.changed.notify_all();
    }

    /// Düğüm döngüsü bitti (`ListenerBridge` düştü); `panicked` = düğüm iş parçacığı panikle
    /// çözülürken.
    fn loop_ended(&self, panicked: bool) {
        let mut state = self.state();
        state.loop_ended = true;
        let stopped = match &state.phase {
            Phase::Detached(_) if panicked => Some(Err(NodeError::NodeThreadPanicked)),
            Phase::Detached(result) => Some(result.clone()),
            Phase::Running | Phase::Requested | Phase::Stopping | Phase::Stopped(_) => None,
        };
        if let Some(result) = stopped {
            state.phase = Phase::Stopped(result);
        }
        drop(state);
        self.changed.notify_all();
    }

    /// Kapanışın bitişini (düğüm döngüsü bitti, kaynaklar bırakıldı) bekler; ilk kapanışın
    /// sonucunu döner. Yalnız yuva boşken çağrılır (aşama `Running` / `Requested` değildir).
    fn wait_stopped(&self) -> Result<(), NodeError> {
        let mut state = self.state();
        loop {
            if let Phase::Stopped(result) = &state.phase {
                return result.clone();
            }
            let waited = self.changed.wait(state);
            state = waited.unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Beklemeden (dinleyicinin içinden): kapanış bittiyse sonucu; istendiyse ya da sürüyorsa `Ok`.
    fn current(&self) -> Result<(), NodeError> {
        match &self.state().phase {
            Phase::Stopped(result) => result.clone(),
            Phase::Running | Phase::Requested | Phase::Stopping | Phase::Detached(_) => Ok(()),
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
/// kapsam bayrağıyla çağırır. Düşmesi düğüm döngüsünün bittiğini bildirir (bkz. `Drop`).
struct ListenerBridge {
    listener: Box<dyn EngineListener>,
    life: Arc<Lifecycle>,
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

impl Drop for ListenerBridge {
    fn drop(&mut self) {
        // Köprü düğüm döngüsünün (`kama_node`'un `NodeLoop`'u) bir alanıdır ve döngü bitince
        // düşer. `NodeLoop` alanlarını bildirim sırasıyla düşürür; Transport'u (UDP soketi, röle
        // kaydı) ve depoyu taşıyan `messenger` alanı dinleyiciden ÖNCE gelir — köprü düştüğünde bu
        // kaynaklar bırakılmıştır. Dinleyicinin içinden başlatılan (join'siz) kapanışın tamamlanma
        // sinyali budur. Bu sıra kama-node'un açık sözleşmesi DEĞİLDİR; belge + test ile kabul
        // edildi (DUR-K4G-15-E CEVAP (a), CEVAP PAKETİ 6), kama-node'un açık kapanış kancası
        // KATMAN 6'da. Testi `listener_shutdown_is_async_and_a_jvm_shutdown_waits_for_the_stop`
        // yalnız şunu sınar: köprü düştükten sonra aynı cihaz dosyası, veri dizini ve UDP portuyla
        // yeniden `start` başarılı. Sıra bozulursa yeniden açılış eski iş parçacığıyla yarışır;
        // test o durumda deterministik olarak kırmızı OLMAYABİLİR — sırayı garanti etmez. Panik
        // yok.
        self.life.loop_ended(thread::panicking());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::{OnceLock, Weak};
    use std::time::Duration;

    use super::*;
    use crate::test_support::{self, Phone, TempDir, BOB, WAIT};

    /// Beklenen bir dönüşün OLMADIĞINI göstermek için verilen süre: düğüm iş parçacığı tutulurken
    /// ya da döngü bitmeden bu sürede dönen kapanış erken dönmüştür.
    const HOLD: Duration = Duration::from_millis(300);

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

    fn throwing() -> Box<Throwing> {
        Box::new(Throwing {
            calls: Arc::new(AtomicUsize::new(0)),
            flagged: Arc::new(AtomicUsize::new(0)),
        })
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
            life: Arc::new(Lifecycle::new()),
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

    #[test]
    fn a_detached_stop_completes_when_the_bridge_drops() {
        let life = Arc::new(Lifecycle::new());
        let bridge = ListenerBridge {
            listener: throwing(),
            life: Arc::clone(&life),
        };
        // Dinleyicinin içinden kapanış: `KamaNode::shutdown` döndü, döngü sürüyor.
        life.begin();
        life.finish(Ok(()));
        assert_eq!(life.current(), Ok(()), "dinleyici yolu beklemez");
        // Döngü bitti (köprü düştü): bekleyen JVM çağıranı sonucu alır.
        drop(bridge);
        assert_eq!(life.wait_stopped(), Ok(()));
        // Düğüm iş parçacığı panikle çözülürken düşen köprü sonucu panik yapar.
        let panicked = Lifecycle::new();
        panicked.begin();
        panicked.finish(Ok(()));
        panicked.loop_ended(true);
        let result = panicked.wait_stopped();
        assert_eq!(result, Err(NodeError::NodeThreadPanicked));
    }

    #[test]
    fn a_blocked_waiter_wakes_when_a_detached_stop_ends() {
        let life = Arc::new(Lifecycle::new());
        // Dinleyicinin içinden kapanış: `KamaNode::shutdown` döndü, döngü sürüyor (Detached).
        life.begin();
        life.finish(Ok(()));
        let (tx, rx) = mpsc::channel();
        let waiter = Arc::clone(&life);
        let handle = thread::spawn(move || {
            let _ = tx.send(waiter.wait_stopped());
        });
        // Döngü bitmeden bekleyen dönemez (aşama Detached; dönüş yalnız Stopped'da).
        assert!(rx.recv_timeout(HOLD).is_err(), "döngü bitmeden döndü");
        // Döngü bitti: koşul değişkeni bekleyeni uyandırır.
        life.loop_ended(false);
        let result = rx.recv_timeout(WAIT).expect("bekleyen uyanmadı");
        assert_eq!(result, Ok(()));
        handle.join().expect("bekleyen iş parçacığı");
    }

    #[test]
    fn a_stop_request_is_made_once_and_can_be_withdrawn() {
        let life = Lifecycle::new();
        assert!(life.request(), "ilk istek yardımcı iş parçacığı açar");
        assert!(!life.request(), "ikinci istek açmaz");
        assert_eq!(life.current(), Ok(()), "istenmiş kapanış beklemeden Ok");
        life.withdraw();
        assert!(life.request(), "geri alınan istek yeniden yapılabilir");
        life.begin();
        life.withdraw();
        let stopping = matches!(life.state().phase, Phase::Stopping);
        assert!(stopping, "başlamış kapanış geri alınmaz");
    }

    /// Dinleyicinin içinde (bir sonraki olayda, düğüm iş parçacığında) bir kez koşacak iş.
    type Task = Box<dyn FnOnce(&KamaMobile) + Send>;

    /// İşi dinleyicinin içinde koşturan dinleyici. Düğüm her saniye `Metrics` olayı verir; iş en
    /// geç onunla koşar.
    struct Probe {
        mobile: Arc<OnceLock<Weak<KamaMobile>>>,
        task: Arc<Mutex<Option<Task>>>,
    }

    impl EngineListener for Probe {
        fn on_event(&self, _ev: EngineEvent) -> Result<(), ListenerError> {
            let Some(mobile) = self.mobile.get().and_then(Weak::upgrade) else {
                return Ok(());
            };
            let mut armed = self.task.lock().unwrap_or_else(PoisonError::into_inner);
            let task = armed.take();
            drop(armed);
            if let Some(task) = task {
                task(&mobile);
            }
            Ok(())
        }
    }

    /// Gerçek düğüm (`KamaMobile::start`, provisioning ile üretilmiş cihaz dosyası) + iş kancası.
    struct Node {
        mobile: Arc<KamaMobile>,
        task: Arc<Mutex<Option<Task>>>,
    }

    impl Node {
        fn start(phone: &Phone) -> Self {
            let slot = Arc::new(OnceLock::new());
            let task = Arc::new(Mutex::new(None));
            let probe = Probe {
                mobile: Arc::clone(&slot),
                task: Arc::clone(&task),
            };
            let listener = Box::new(probe);
            let started = KamaMobile::start(phone.config.clone(), phone.data.clone(), listener);
            let mobile = started.expect("KamaMobile.start");
            let _ = slot.set(Arc::downgrade(&mobile));
            Self { mobile, task }
        }

        /// `task`'ı bir sonraki olayda dinleyicinin içinde bir kez koşturur (beklemez).
        fn arm(&self, task: impl FnOnce(&KamaMobile) + Send + 'static) {
            *self.task.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(task));
        }

        /// `task`'ı dinleyicinin içinde koşturur ve sonucunu bekler (en çok `WAIT`).
        fn run_in_listener<T: Send + 'static>(
            &self,
            task: impl FnOnce(&KamaMobile) -> T + Send + 'static,
        ) -> T {
            let (tx, rx) = mpsc::channel();
            self.arm(move |mobile| {
                let _ = tx.send(task(mobile));
            });
            rx.recv_timeout(WAIT).expect("dinleyici işi koşmadı")
        }
    }

    /// `call`'ı bir JVM iş parçacığında (dinleyici dışı) koşturur ve sonucunu en çok `WAIT` bekler:
    /// süre aşılırsa test açık hatayla düşer (sonsuz bekleme yok).
    fn on_jvm_thread<T: Send + 'static>(
        mobile: &Arc<KamaMobile>,
        what: &str,
        call: impl FnOnce(&KamaMobile) -> T + Send + 'static,
    ) -> T {
        let mobile = Arc::clone(mobile);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(call(mobile.as_ref()));
        });
        match rx.recv_timeout(WAIT) {
            Ok(result) => result,
            Err(_) => panic!("{what}: {WAIT:?} içinde dönmedi"),
        }
    }

    /// JVM iş parçacığından `shutdown` — süre sınırlı.
    fn jvm_shutdown(mobile: &Arc<KamaMobile>) -> Result<(), NodeError> {
        on_jvm_thread(mobile, "JVM shutdown", KamaMobile::shutdown)
    }

    #[test]
    fn listener_calls_never_wait_for_the_slot_lock() {
        let dir = TempDir::new("lock");
        let node = Node::start(&test_support::provision(dir.path()));
        let lock = &node.mobile.node;

        // Kapanış yuvanın yazma kilidini tutarken: dinleyicinin çağrıları beklemez, `Stopped` alır.
        let writer = lock.write().unwrap_or_else(PoisonError::into_inner);
        let (metrics, text) = node.run_in_listener(|mobile| {
            let text = mobile.send_text(BOB.to_owned(), "x".to_owned());
            (mobile.metrics(), text)
        });
        drop(writer);
        assert_eq!(metrics, Err(NodeError::Stopped));
        assert_eq!(text, Err(NodeError::Stopped));

        // Kilit serbestken: yanıt bekleyen çağrı `ReentrantCall`, bloklamayan çağrı yanıt verir.
        let (connect, metrics) = node.run_in_listener(|mobile| {
            let connect = mobile.connect(BOB.to_owned());
            (connect, mobile.metrics())
        });
        assert_eq!(connect, Err(NodeError::ReentrantCall));
        assert!(metrics.is_ok(), "{metrics:?}");
        assert_eq!(jvm_shutdown(&node.mobile), Ok(()));
    }

    #[test]
    fn jvm_non_blocking_calls_never_wait_for_the_slot_lock() {
        let dir = TempDir::new("jvm-non-blocking");
        let node = Node::start(&test_support::provision(dir.path()));
        let file = dir.path().join("gonderilmez.bin");
        let path = file.to_str().expect("geçici yol UTF-8").to_owned();

        // Yuvanın yazarı varken (kapanış): JVM iş parçacığındaki üç bloklamayan çağrı yuva
        // kilidini BEKLEMEDEN `Stopped` alır. Bekleselerdi yazar bırakılmadan dönemezlerdi.
        let lock = &node.mobile.node;
        let writer = lock.write().unwrap_or_else(PoisonError::into_inner);
        let calls = move |mobile: &KamaMobile| {
            let metrics = mobile.metrics().err();
            let sent = mobile.send_file(BOB.to_owned(), path).err();
            let signal = mobile.on_platform_signal(PlatformSignal::LinkChanged);
            (metrics, sent, signal)
        };
        let (metrics, sent, signal) = on_jvm_thread(&node.mobile, "bloklamayan", calls);
        drop(writer);
        assert_eq!(metrics, Some(NodeError::Stopped));
        assert_eq!(sent, Some(NodeError::Stopped));
        assert_eq!(signal, Err(NodeError::Stopped));
        assert_eq!(jvm_shutdown(&node.mobile), Ok(()));
    }

    #[test]
    fn listener_shutdown_is_async_and_a_jvm_shutdown_waits_for_the_stop() {
        let dir = TempDir::new("listener-shutdown");
        let phone = test_support::provision(dir.path());
        let node = Node::start(&phone);

        // Dinleyicinin içinden: kapanışı ister ve beklemeden döner; sonraki çağrılar `Stopped`.
        let (first, second, metrics) = node.run_in_listener(|mobile| {
            let first = mobile.shutdown();
            let second = mobile.shutdown();
            (first, second, mobile.metrics())
        });
        assert_eq!(first, Ok(()));
        assert_eq!(second, Ok(()), "ikinci shutdown da beklemez");
        assert_eq!(metrics, Err(NodeError::Stopped));

        // JVM iş parçacığından: döngünün bitişini bekler (Detached → Stopped; süre sınırlı) ve
        // dönüşte aynı cihaz dosyası ve `data_dir` ile yeniden açılır.
        assert_eq!(jvm_shutdown(&node.mobile), Ok(()));
        let again = Node::start(&phone);
        assert_eq!(jvm_shutdown(&again.mobile), Ok(()));
    }

    #[test]
    fn a_listener_shutdown_on_a_busy_slot_is_handed_to_a_stop_thread() {
        let dir = TempDir::new("listener-shutdown-busy");
        let phone = test_support::provision(dir.path());
        let node = Node::start(&phone);

        // Süren bir JVM çağrısı yuvanın okuma kilidini tutuyor: dinleyicinin içinden `shutdown`
        // yine kapanışı ister (yardımcı iş parçacığına devreder); ikinci istek yeni iş parçacığı
        // açmaz.
        let lock = &node.mobile.node;
        let reader = lock.read().unwrap_or_else(PoisonError::into_inner);
        let (first, second) = node.run_in_listener(|mobile| {
            let first = mobile.shutdown();
            (first, mobile.shutdown())
        });
        assert_eq!(first, Ok(()), "kapanış istendi");
        assert_eq!(second, Ok(()), "istenmiş kapanış beklemeden Ok");
        let requested = matches!(node.mobile.life.state().phase, Phase::Requested);
        // Yardımcı iş parçacığı yuvayı bekliyor: düğüm henüz alınmadı.
        assert!(requested, "düğüm henüz alınmadı");

        // Yuva serbest kalınca kapanış tamamlanır; dönüşte aynı dosya ve dizinle yeniden açılır.
        drop(reader);
        assert_eq!(jvm_shutdown(&node.mobile), Ok(()));
        let again = Node::start(&phone);
        assert_eq!(jvm_shutdown(&again.mobile), Ok(()));
    }

    #[test]
    fn concurrent_shutdowns_both_wait_for_the_full_stop() {
        let dir = TempDir::new("concurrent-shutdown");
        let phone = test_support::provision(dir.path());
        let node = Node::start(&phone);

        // Düğüm iş parçacığını dinleyicide tut: döngü bitemez, kapanış tamamlanamaz.
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        node.arm(move |_| {
            let _ = entered_tx.send(());
            let _ = release_rx.recv_timeout(WAIT);
        });
        entered_rx.recv_timeout(WAIT).expect("dinleyici");

        // İki JVM iş parçacığı aynı anda kapatır: biri düğümü alıp join eder, öteki yuvayı boş
        // bulur ve bekler. Düğüm durmadan HİÇBİRİ dönmez.
        let (done_tx, done_rx) = mpsc::channel();
        let callers: Vec<_> = (0..2)
            .map(|_| {
                let mobile = Arc::clone(&node.mobile);
                let done = done_tx.clone();
                thread::spawn(move || {
                    let _ = done.send(mobile.shutdown());
                })
            })
            .collect();
        assert!(
            done_rx.recv_timeout(HOLD).is_err(),
            "shutdown düğüm durmadan döndü"
        );
        release_tx.send(()).expect("dinleyiciyi bırak");
        for _ in 0..callers.len() {
            let result = done_rx.recv_timeout(WAIT).expect("shutdown dönmedi");
            assert_eq!(result, Ok(()));
        }
        for caller in callers {
            caller.join().expect("çağıran iş parçacığı");
        }

        // Dönüşte kaynaklar bırakılmıştır: aynı cihaz dosyası ve `data_dir` ile yeniden açılır.
        let again = Node::start(&phone);
        assert_eq!(jvm_shutdown(&again.mobile), Ok(()));
    }
}
