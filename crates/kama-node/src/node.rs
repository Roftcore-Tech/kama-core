//! Düğüm iş parçacığı — motorların TEK sahibi ve periyodik yoklama döngüsü.
//!
//! # Tek sahip
//!
//! `Messenger` (Transport'un sahibi), `Ptt`, `FileTransfer` ve `Signaling` yalnız bu iş
//! parçacığında yaşar; API çağrıları onlara SINIRLI bir komut kuyruğuyla ([`Command`],
//! [`COMMAND_QUEUE_CAP`]) ulaşır. Kilit yoktur, motorlar `Sync` olmak zorunda değildir.
//! Messenger TEKTİR ([`AppEngines`]): motorlara her çağrıda aynı Messenger verilir (kama-app çağrı
//! sözleşmesi: bir motor tek bir Messenger'la kullanılır).
//!
//! # Tur (sahip kararı CEVAP PAKETİ 2 madde 2 — periyodik poll)
//!
//! 1. Kuyruktaki komutlar, en çok [`COMMAND_QUEUE_CAP`] tane.
//! 2. Olay zinciri: `Ptt::poll` Messenger'ın TEK okuyucusudur → `PttEvent::Messenger` olayı
//!    `FileTransfer::handle`'a → dosya motorunun TÜKETMEDİĞİ olay `Signaling::handle`'a — zincirin
//!    son halkası (K4G-14b): `SIGNALING_BLOB`'u tüketir ve `NodeEvent::SignalingReceived` olur,
//!    diğer olayı aynen geçirir (eşin `Ready`'sini geçirmeden önce eşin yeniden sunulacak
//!    blob'larını yeni oturuma sunar) → düğüm olayına; dosya motorunun doğurduğu olaylar hemen
//!    ardından. Turda en çok [`MAX_EVENTS_PER_TURN`] olay işlenir.
//! 3. Messenger kuyruğu BOŞALDIYSA (`Ptt::poll` → `None`) `FileTransfer::pump` (K4G-09 sözleşmesi:
//!    zamanlayıcılar `FILE_IDLE_TTL` / `FILE_RECEIPT_TTL` kuyruk boşaltıldıktan SONRA denetlenir;
//!    bekleyen parça önce işlenir ve aktarımı canlı tutar) ve AYNI koşulda `Signaling::pump`
//!    (K4G-14b): yeniden sunum kaydının ömrü (`SIGNALING_RESEND_TTL`) budanır ve olayı okunmadan
//!    benimsenen oturuma yeniden sunum yapılır — kama-app sözleşmesi: trafik olmasa da periyodik
//!    çağrılır. Ardından dosya motorunun olayları.
//! 4. Ölçü anlık görüntüsü (`metrics()`), [`crate::METRICS_TICK`]'te bir `Metrics` olayı.
//! 5. Bekleme: iş kaldıysa beklemeden yeni tur; yoksa komut gelene ya da [`crate::NODE_TICK`]
//!    dolana dek. Trafik olmasa da her `NODE_TICK`'te bir tur döner: PTT-R4
//!    (`PTT_STREAM_IDLE_TTL`, `Ptt::poll` içinde), dosya zamanlayıcıları ve sinyalleşme kaydının
//!    ömrü (`pump`) motor çağrılmadıkça denetlenmez. Transport'un zamanla ilerleyen işi (zarf
//!    olayları, yenileme tutması, Crypto zamanlayıcıları) de her `poll`'da sürülür
//!    (`ClientTransport::drive`).
//!
//! # Yanıt
//!
//! Yanıt bekleyen komut, düğüm onu İŞLEDİĞİ anda yanıtlanır (tur bitmeden): çağıran ağ ya da
//! zamanlayıcı beklemez, yalnız komutun düğüm turundaki işlenmesini (depo commit'i dahil) bekler.
//!
//! # Tutamaç kaydı motor tablosuyla AYNI ANDA kapanır (sahip şartı (1))
//!
//! Düğüm tutamacı (tutamaç → (eş, tel kimliği)) ancak motor o akışı/aktarımı açık tuttukça
//! çözülebilir; aksi hâlde kapanmış bir tutamaç, aynı tel kimliğini almış BAŞKA bir eşin
//! akışına/aktarımına ulaşırdı (tel kimlikleri eş başınadır). Kapatan yollar:
//!
//! * Dosya giden: motor aktarımı tablosundan HER çıkarışında (`cancel_send`'in `FILE_BEGIN` öncesi
//!   iptali, `pump`'ın bitirdiği aktarım, `handle`'ın uyguladığı `FILE_RECEIPT`, alındı zaman
//!   aşımı) aynı çağrıda tam bir son olay üretir (kama-app sözleşmesi: "kuyruğa alınmış her giden
//!   aktarım … tam bir son olayla biter"). Düğüm FileTransfer'a dokunan HER çağrıdan hemen sonra
//!   olay kuyruğunu boşaltır (`drain_file_events`); son olay kaydı kapatır. Böylece sıradaki hiçbir
//!   komut — aynı komut grubunda olsa bile — motorun bıraktığı tel kimliğine ulaşamaz.
//! * PTT giden: motor giden akışı YALNIZ `Ptt::close`'ta çıkarır (kendiliğinden kapatmaz); düğüm
//!   kaydı aynı komutta, `close`'tan ÖNCE silinir.
//! * Gelen akış ve aktarımların düğüm tutamacı yoktur: hiçbir API çağrısı gelen kimlik almaz.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use client_core::transport::MAX_INBOX;
use kama_app::codec::PttCodec;
use kama_app::file_transfer::{FileEvent, FileSendError, FileTransfer, TransferId};
use kama_app::messaging::{ConnectState, Messenger, MessengerEvent, MsgId};
use kama_app::ptt::{Ptt, PttEvent, StreamId};
use kama_app::signaling::{Signaling, SignalingEvent, SignalingId};
use kama_contracts::handles::{CloseReason, PeerRef};
use kama_contracts::s1::{ConnectResult, ConnectTicket, SessionEvent};

use crate::error::NodeError;
use crate::event::{
    CloseCause, FileFailReason, FileRef, FileStage, LinkState, NodeEvent, NodeMetrics,
    PeerConnectState,
};
use crate::files::{self, ReceivedFiles};
use crate::handle::{FileHandle, HandleAllocator, Registry, StreamHandle};
use crate::peers::PeerBook;
#[cfg(feature = "test-util")]
use crate::probe::{NodeProbe, SessionEventCounts, SessionProbe};
use crate::wiring::Transport;
use crate::{NodeListener, PlatformSignal, METRICS_TICK, NODE_TICK};

/// Komut kuyruğu tavanı — yerel sınır, normatif DEĞİL `[KALİBRASYON]`. Yanıt bekleyen her çağıran
/// en çok bir komut tutar (yanıtı bekler); tavan eşzamanlı çağıran iş parçacıkları ve yanıt
/// beklemeyen komutlar (`send_file`, `on_platform_signal`) içindir. Doluyken çağrı bloklamaz:
/// [`NodeError::Busy`] (geri basınç). Kuyruğun bellek tavanı da budur: komut yükleri kuyruğa
/// girmeden ÖNCE sınırlanır (metin, PTT yükü ve sinyalleşme blob'u bir AMP kaydına, dosya yalnız
/// yoluyla).
pub(crate) const COMMAND_QUEUE_CAP: usize = 64;

/// Turda işlenen en çok olay — Transport'un teslim kuyruğu tavanı (`client_core::transport::
/// MAX_INBOX`; türetilmiş). Komutlar yoğun gelen trafikte aç kalmaz; tavana ulaşan tur bekleme
/// yapmadan yeniden başlar, kuyruk her turda en az bir kez dolusu kadar boşalır.
const MAX_EVENTS_PER_TURN: usize = MAX_INBOX;

/// Tek yanıtlık kanal: her komut tam bir yanıt alır, gönderim bloklamaz.
pub(crate) const REPLY_CAPACITY: usize = 1;

/// Yanıt kanalı.
pub(crate) type Reply<T> = SyncSender<Result<T, NodeError>>;

/// API → düğüm komutu. Yanıtlı komutlar düğüm onları işlediği anda yanıtlanır.
pub(crate) enum Command {
    Connect {
        peer: PeerRef,
        reply: Reply<()>,
    },
    CancelConnect {
        peer: PeerRef,
        reply: Reply<bool>,
    },
    SendText {
        peer: PeerRef,
        text: String,
        reply: Reply<MsgId>,
    },
    /// Yanıtsız: tutamaç çağıranda verildi; sonuç olaydır (`FileProgress` / `FileFailed`).
    SendFile {
        file: FileHandle,
        peer: PeerRef,
        path: PathBuf,
    },
    CancelFile {
        file: FileHandle,
        reply: Reply<()>,
    },
    PttOpen {
        peer: PeerRef,
        codec: PttCodec,
        bundle_ms: u8,
        reply: Reply<StreamHandle>,
    },
    PttPush {
        stream: StreamHandle,
        payload: Vec<u8>,
        reply: Reply<()>,
    },
    PttClose {
        stream: StreamHandle,
        reply: Reply<()>,
    },
    /// Blob'un boyu kuyruktan ÖNCE sınandı (`KamaNode::send_signaling`).
    SendSignaling {
        peer: PeerRef,
        bytes: Vec<u8>,
        reply: Reply<SignalingId>,
    },
    /// Yanıtsız.
    Signal(PlatformSignal),
    DrainBackground {
        reply: Reply<()>,
    },
    /// [K4G-05] YALNIZ test-util: ölçüm sondası (`KamaNode::probe_for_test`; salt okuma).
    #[cfg(feature = "test-util")]
    Probe {
        reply: Reply<NodeProbe>,
    },
    /// Döngüyü bitirir. `KamaNode::stop` onu kuyruğun SONUNA koyar: döngü ona vardığında önündeki
    /// komutların hepsi işlenmiştir (ardında komut olmaz — kapanıştan sonra çağıran kalmaz). Kuyruk
    /// doluyken `stop` bunun yerine kapanış bayrağını kaldırır ([`Shared::stopping`]).
    Shutdown,
}

/// API ile düğüm iş parçacığının paylaştığı durum.
#[derive(Debug)]
pub(crate) struct Shared {
    /// Son tur sonundaki ölçüler (`KamaNode::metrics` kopyalar).
    pub(crate) metrics: Mutex<NodeMetrics>,
    /// Düğüm-tekil tutamaç sayacı (`send_file` tutamacı çağıranda, `ptt_open`'unki düğümde
    /// verilir — ikisi aynı sayaçtan).
    pub(crate) handles: HandleAllocator,
    /// Kapanış isteği, YALNIZ kuyruk doluyken (`Shutdown` komutu sığmadı). Döngü bayrağı komut
    /// grubu sonlarında okur: en geç SIRADAKİ komut grubunun sonunda biter. Kuyrukta yer varsa
    /// kapanış `Shutdown` komutuyladır ve bayrak kalkmaz.
    pub(crate) stopping: AtomicBool,
}

impl Shared {
    pub(crate) fn new(initial: NodeMetrics) -> Self {
        Self {
            metrics: Mutex::new(initial),
            handles: HandleAllocator::new(),
            stopping: AtomicBool::new(false),
        }
    }
}

/// Zehirlenmeye karşı panik yapmaz: korunan değer her yazımda tümüyle değiştirilir.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Application motorları: TEK Messenger ve onun yanında duran motorlar (`KamaNode::build` kurar;
/// `Ptt`, `FileTransfer` ve `Signaling` Application'ın AYNI saatini paylaşır).
pub(crate) struct AppEngines {
    pub(crate) messenger: Messenger<Transport>,
    pub(crate) ptt: Ptt,
    pub(crate) files: FileTransfer,
    pub(crate) signaling: Signaling,
}

/// Düğüm iş parçacığının durumu (bkz. modül belgesi).
pub(crate) struct NodeLoop {
    messenger: Messenger<Transport>,
    ptt: Ptt,
    files: FileTransfer,
    signaling: Signaling,
    peers: Arc<PeerBook>,
    listener: Box<dyn NodeListener>,
    shared: Arc<Shared>,
    received: ReceivedFiles,
    /// Açık giden PTT akışları: tutamaç → (eş, tel `stream_id`). Boyu `Ptt`'nin giden tablosuyla
    /// sınırlı (eş başına bir açık akış).
    streams: Registry<(PeerRef, StreamId)>,
    /// Açık giden aktarımlar: tutamaç → (eş, tel `transfer_id`). Boyu `FileTransfer`'in giden
    /// tablosuyla sınırlı (`MAX_OUTGOING_TRANSFERS`).
    outgoing: Registry<(PeerRef, TransferId)>,
    /// Ters eşleme (dosya motorunun olayları `(eş, transfer_id)` taşır).
    outgoing_by_wire: BTreeMap<(PeerRef, TransferId), FileHandle>,
    /// Düğümün açtığı bağlantı niyetleri: bilet → eş. Bilet SON olayına (`Ready` ya da
    /// `ConnectAbandoned`) dek kalır — aynı komut grubunda iptal + yeni `connect` gelse de iptalin
    /// `Closed` olayı kaybolmaz. Boyu, son olayı henüz işlenmemiş niyetlerle sınırlıdır.
    tickets: BTreeMap<ConnectTicket, PeerRef>,
    /// Güncel oturumu olan eşler (`Connected` / `Closed` geçişleri için; ≤ eş sayısı).
    up: BTreeSet<PeerRef>,
    /// Sıradaki `Metrics` olayının zamanı.
    metrics_due: Instant,
    /// [K4G-05] YALNIZ test-util: işlenen S1 oturum olaylarının sayımı (ölçüm sondası).
    #[cfg(feature = "test-util")]
    session_events: SessionEventCounts,
}

impl NodeLoop {
    pub(crate) fn new(
        engines: AppEngines,
        peers: Arc<PeerBook>,
        listener: Box<dyn NodeListener>,
        shared: Arc<Shared>,
        received: ReceivedFiles,
    ) -> Self {
        let AppEngines {
            messenger,
            ptt,
            files,
            signaling,
        } = engines;
        Self {
            messenger,
            ptt,
            files,
            signaling,
            peers,
            listener,
            shared,
            received,
            streams: Registry::new(),
            outgoing: Registry::new(),
            outgoing_by_wire: BTreeMap::new(),
            tickets: BTreeMap::new(),
            up: BTreeSet::new(),
            metrics_due: Instant::now(),
            #[cfg(feature = "test-util")]
            session_events: SessionEventCounts::default(),
        }
    }

    /// Döngü (bkz. modül belgesi). `Shutdown` komutuna varınca (önündeki komutlar işlenmiş
    /// olarak), bir komut grubunun sonunda kapanış bayrağını görünce ya da API düşünce biter;
    /// dönüşte motorlar düşer (Transport: soketler ve çalışma-zamanı; depo: dosya kilidi) ve
    /// kuyrukta kalan komutlar alıcıyla birlikte olay üretmeden düşer — yanıt bekleyen çağıran
    /// `Stopped` alır.
    pub(crate) fn run(mut self, commands: Receiver<Command>) {
        let mut waiting: Option<Command> = None;
        loop {
            let mut handled = 0;
            while handled < COMMAND_QUEUE_CAP {
                let command = match waiting.take() {
                    Some(command) => command,
                    None => match commands.try_recv() {
                        Ok(command) => command,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    },
                };
                if matches!(command, Command::Shutdown) {
                    return;
                }
                self.execute(command);
                handled += 1;
            }
            if self.shared.stopping.load(Ordering::Acquire) {
                return;
            }
            let drained = self.turn();
            self.publish_metrics();
            let busy = !drained || handled >= COMMAND_QUEUE_CAP;
            let wait = if busy { Duration::ZERO } else { NODE_TICK };
            match commands.recv_timeout(wait) {
                Ok(command) => waiting = Some(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    // ── Komutlar ─────────────────────────────────────────────────────────────────────────────

    fn execute(&mut self, command: Command) {
        match command {
            Command::Connect { peer, reply } => answer(&reply, self.connect(peer)),
            Command::CancelConnect { peer, reply } => {
                answer(&reply, Ok(self.messenger.cancel_connect(peer)));
            }
            Command::SendText { peer, text, reply } => {
                let sent = self.messenger.send_text(peer, &text);
                answer(&reply, sent.map_err(NodeError::Messenger));
            }
            Command::SendFile { file, peer, path } => self.send_file(file, peer, &path),
            Command::CancelFile { file, reply } => answer(&reply, self.cancel_file(file)),
            Command::PttOpen {
                peer,
                codec,
                bundle_ms,
                reply,
            } => answer(&reply, self.ptt_open(peer, codec, bundle_ms)),
            Command::PttPush {
                stream,
                payload,
                reply,
            } => answer(&reply, self.ptt_push(stream, &payload)),
            Command::PttClose { stream, reply } => answer(&reply, self.ptt_close(stream)),
            Command::SendSignaling { peer, bytes, reply } => {
                let sent = self.signaling.send(&mut self.messenger, peer, &bytes);
                answer(&reply, sent.map_err(NodeError::Signaling));
            }
            Command::Signal(signal) => self.on_platform_signal(signal),
            Command::DrainBackground { reply } => answer(&reply, self.drain_background()),
            #[cfg(feature = "test-util")]
            Command::Probe { reply } => answer(&reply, Ok(self.probe())),
            // `run` döngüyü bundan önce bitirir.
            Command::Shutdown => {}
        }
    }

    /// `Messenger::connect` (idempotent, S1 KARAR-1). Bekleyen niyetin bileti eşe bağlanır:
    /// `ConnectProgress` / `ConnectAbandoned` olayları bilet taşır.
    fn connect(&mut self, peer: PeerRef) -> Result<(), NodeError> {
        match self.messenger.connect(peer).map_err(NodeError::Messenger)? {
            // Biten bir niyetin bileti (ör. aynı grupta iptal edilmiş) SİLİNMEZ: onun
            // `ConnectAbandoned` olayı henüz işlenmemiş olabilir.
            ConnectResult::Pending(ticket) => {
                self.tickets.insert(ticket, peer);
            }
            ConnectResult::Ready(_) => self.note_up(peer),
        }
        Ok(())
    }

    /// Plan K4G-11 NE 2 (ek 2026-10-11; PoC-KAPSAM-001 EK-2): yerel outbox'u ve Transport'un
    /// yeniden gönderim kuyruğunu YENİDEN POMPALAR, gerekirse eş başına `connect` tetikler. Hub
    /// mailbox PoC'de YOKTUR: bu çağrı hiçbir şey "çekmez". "Gerekirse" = eşin hazır oturumu da
    /// bekleyen niyeti de yok (`ConnectState::Idle`): `connect` eşin kalıcı outbox'ını yükler ve
    /// kayıtlar sıradaki oturumda aynı `msg_id`'yle gider (K4G-07). Hazır oturumlu eşte pompalama
    /// komutun ardından gelen turdur (`Messenger::poll` reddedilmiş yeniden gönderimleri yeniden
    /// sunar, `ClientTransport::drive` ARQ'yu sürer, `FileTransfer::pump` bekleyen kayıtları).
    /// İlk `connect` hatası döner; diğer eşler yine denenir.
    fn drain_background(&mut self) -> Result<(), NodeError> {
        let idle: Vec<PeerRef> = self
            .peers
            .iter()
            .map(|(_, peer)| peer)
            .filter(|&peer| self.messenger.connect_state(peer) == ConnectState::Idle)
            .collect();
        let mut first_error = None;
        for peer in idle {
            if let Err(error) = self.connect(peer) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// D15: `LinkChanged` → `SESSION_INIT_REF` akışı hub sprintindedir (PoC-KAPSAM-001 K4). Bu
    /// sprintte (direct-mode) sinyal kabul edilir ve hiçbir protokol davranışı tetiklemez.
    fn on_platform_signal(&self, signal: PlatformSignal) {
        match signal {
            PlatformSignal::LinkChanged => {}
        }
    }

    /// Dosyayı okur (düğüm iş parçacığında; çağıran beklemez) ve kuyruğa alır. Sonuç olaydır —
    /// kuyruğa alınamazsa `FileFailed`, çağıran tutamacı almadan önce de gelebilir.
    fn send_file(&mut self, file: FileHandle, peer: PeerRef, path: &Path) {
        match self.queue_file(peer, path) {
            Ok(transfer_id) => {
                self.outgoing.insert(file.raw(), (peer, transfer_id));
                self.outgoing_by_wire.insert((peer, transfer_id), file);
            }
            Err(reason) => self.emit_for(peer, |peer_id| NodeEvent::FileFailed {
                file: FileRef::Outgoing(file),
                peer_id,
                reason,
            }),
        }
        self.drain_file_events();
    }

    fn queue_file(&mut self, peer: PeerRef, path: &Path) -> Result<TransferId, FileFailReason> {
        // Oturumsuz eşe dosya OKUNMAZ (motor da reddederdi; okuma boşa olurdu).
        if self.messenger.session_of(peer).is_none() {
            return Err(FileFailReason::NotQueued(FileSendError::NoSession));
        }
        let (name, bytes) = files::read_outgoing(path)?;
        self.files
            .send_file(&self.messenger, peer, &name, bytes)
            .map_err(FileFailReason::NotQueued)
    }

    fn cancel_file(&mut self, file: FileHandle) -> Result<(), NodeError> {
        let &(_, transfer_id) = self.outgoing.get(file.raw(), &self.shared.handles)?;
        let cancelled = self.files.cancel_send(transfer_id).map_err(NodeError::File);
        // `FILE_BEGIN` öncesi iptalde motor aktarımı tablosundan BU çağrıda çıkardı ve son olayı
        // kuyruğa koydu: kayıt AYNI komutta kapanır (modül belgesi, sahip şartı (1)).
        self.drain_file_events();
        cancelled
    }

    fn ptt_open(
        &mut self,
        peer: PeerRef,
        codec: PttCodec,
        bundle_ms: u8,
    ) -> Result<StreamHandle, NodeError> {
        // Tutamaç ÖNCE: sayaç tükenmişse akış açılmaz.
        let stream = StreamHandle::from_raw(self.shared.handles.issue()?);
        let stream_id = self
            .ptt
            .open(&mut self.messenger, peer, codec, bundle_ms)
            .map_err(NodeError::Ptt)?;
        self.streams.insert(stream.raw(), (peer, stream_id));
        Ok(stream)
    }

    fn ptt_push(&mut self, stream: StreamHandle, payload: &[u8]) -> Result<(), NodeError> {
        let &(_, stream_id) = self.streams.get(stream.raw(), &self.shared.handles)?;
        self.ptt
            .send_bundle(&mut self.messenger, stream_id, payload)
            .map_err(NodeError::Ptt)
    }

    /// Tutamaç HER DURUMDA kapanır (`Ptt::close` akışı her durumda kapatır; hata yalnız
    /// `PTT_CLOSE`'un Transport'a verilemediğini söyler).
    fn ptt_close(&mut self, stream: StreamHandle) -> Result<(), NodeError> {
        let (_, stream_id) = self.streams.remove(stream.raw(), &self.shared.handles)?;
        self.ptt
            .close(&mut self.messenger, stream_id)
            .map_err(NodeError::Ptt)
    }

    // ── Tur ──────────────────────────────────────────────────────────────────────────────────

    /// Olay zinciri + (kuyruk boşaldıysa) dosya ve sinyalleşme pompaları. `true` = Messenger
    /// kuyruğu boşaldı.
    fn turn(&mut self) -> bool {
        let mut budget = MAX_EVENTS_PER_TURN;
        let drained = loop {
            if budget == 0 {
                break false;
            }
            budget -= 1;
            let Some(event) = self.ptt.poll(&mut self.messenger) else {
                break true;
            };
            self.on_ptt_event(event);
        };
        if drained {
            self.files.pump(&mut self.messenger);
            // AYNI koşul (K4G-14b): yeniden sunum ve ömür budaması buna bağlıdır. Dosya motoruna ve
            // tutamaç kaydına dokunmaz; tur sırası kama-app `signaling` modül belgesindeki gibidir.
            self.signaling.pump(&mut self.messenger);
        }
        self.drain_file_events();
        drained
    }

    /// Dosya motorunun bekleyen olayları (≤ motorun olay kuyruğu tavanı). FileTransfer'a dokunan
    /// HER çağrıdan sonra çağrılır: son olay tutamaç kaydını motor tablosuyla aynı anda kapatır.
    fn drain_file_events(&mut self) {
        while let Some(event) = self.files.poll_event() {
            self.on_file_event(event);
        }
    }

    fn on_ptt_event(&mut self, event: PttEvent) {
        match event {
            PttEvent::Messenger(event) => {
                if let Some(event) = self.files.handle(&mut self.messenger, event) {
                    // Zincirin son halkası (K4G-14b).
                    match self.signaling.handle(&mut self.messenger, event) {
                        SignalingEvent::SignalingReceived { peer, bytes } => {
                            self.emit_for(peer, |peer_id| NodeEvent::SignalingReceived {
                                peer_id,
                                bytes,
                            });
                        }
                        SignalingEvent::Messenger(event) => self.on_messenger_event(event),
                    }
                }
                // `handle` bir alındıyla giden aktarımı bitirmiş ya da gelen aktarımı tamamlamış
                // olabilir: olaylar (ve kayıt kapanışı) hemen, zincirdeki sırasıyla.
                self.drain_file_events();
            }
            PttEvent::PttStarted {
                peer,
                stream_id,
                codec,
                bundle_ms,
            } => self.emit_for(peer, |peer_id| NodeEvent::PttOpened {
                peer_id,
                stream_id,
                codec,
                bundle_ms,
            }),
            PttEvent::PttAudio {
                peer,
                stream_id,
                payload,
            } => self.emit_for(peer, |peer_id| NodeEvent::PttBundle {
                peer_id,
                stream_id,
                payload,
            }),
            PttEvent::PttEnded { peer, stream_id } => {
                self.emit_for(peer, |peer_id| NodeEvent::PttClosed { peer_id, stream_id });
            }
        }
    }

    fn on_messenger_event(&mut self, event: MessengerEvent) {
        match event {
            MessengerEvent::Session(event) => self.on_session_event(event),
            MessengerEvent::TextReceived { peer, msg_id, text } => {
                self.emit_for(peer, |peer_id| NodeEvent::TextReceived {
                    peer_id,
                    msg_id,
                    text,
                });
            }
            MessengerEvent::Delivered { peer, msg_id } => {
                self.emit_for(peer, |peer_id| NodeEvent::Receipt { peer_id, msg_id });
            }
            // Zincirin sonu: dosya ve sinyalleşme motorlarının tüketmediği kayıtlar
            // (`SIGNALING_BLOB` buraya ulaşmaz; `RECEIPT(read)` PoC dışıdır). Uygulama olayı yok.
            MessengerEvent::Other { .. } => {}
        }
    }

    fn on_session_event(&mut self, event: SessionEvent) {
        #[cfg(feature = "test-util")]
        self.count_session_event(&event);
        match event {
            SessionEvent::Ready(_, peer) => self.note_up(peer),
            SessionEvent::Closed(_, reason) => self.note_down(reason),
            SessionEvent::ConnectProgress(ticket, phase) => {
                if let Some(&peer) = self.tickets.get(&ticket) {
                    self.emit_for(peer, |peer_id| NodeEvent::ConnectProgress {
                        peer_id,
                        phase,
                    });
                }
            }
            SessionEvent::ConnectAbandoned(ticket, reason) => {
                if let Some(peer) = self.tickets.remove(&ticket) {
                    self.emit_for(peer, |peer_id| NodeEvent::Closed {
                        peer_id,
                        reason: CloseCause::Abandoned(reason),
                    });
                }
            }
            // Yenileme Transport'un işidir (arka planda; S1 KARAR-2).
            SessionEvent::RenewRequired(_) => {}
        }
    }

    /// Eş Messenger'da hazır oturum edindiyse ve önceden yoksa `Connected`.
    fn note_up(&mut self, peer: PeerRef) {
        if self.messenger.session_of(peer).is_none() {
            // Messenger oturumu benimseyemedi (eşin kalıcı outbox'ı yüklenemedi; Messenger
            // `store_errors`'ta sayar): eş bağlı sayılmaz.
            return;
        }
        self.tickets.retain(|_, waiting| *waiting != peer);
        if self.up.insert(peer) {
            self.emit_for(peer, |peer_id| NodeEvent::Connected { peer_id });
        }
    }

    /// `Closed(h, reason)` işlendi: güncel oturumu kalmayan bağlı eşler `Closed` alır. Yerini yeni
    /// oturuma bırakan oturumun kapanışı (supersession; S1 K7 yeni `Ready`'yi önce verir) olay
    /// ÜRETMEZ: eş bağlı kalır.
    fn note_down(&mut self, reason: CloseReason) {
        let gone: Vec<PeerRef> = self
            .up
            .iter()
            .copied()
            .filter(|&peer| self.messenger.session_of(peer).is_none())
            .collect();
        for peer in gone {
            self.up.remove(&peer);
            self.emit_for(peer, |peer_id| NodeEvent::Closed {
                peer_id,
                reason: CloseCause::Session(reason),
            });
        }
    }

    fn on_file_event(&mut self, event: FileEvent) {
        match event {
            // `name` eşten gelir: dosya adına GİRMEZ (yol enjeksiyonu yok) ve olaya konmaz.
            FileEvent::Received {
                peer,
                transfer_id,
                name: _,
                bytes,
            } => self.on_received(peer, transfer_id, &bytes),
            FileEvent::Failed {
                peer,
                transfer_id,
                reason,
            } => self.emit_for(peer, |peer_id| NodeEvent::FileFailed {
                file: FileRef::Incoming(transfer_id),
                peer_id,
                reason: FileFailReason::Incoming(reason),
            }),
            FileEvent::Aborted {
                peer,
                transfer_id,
                reason,
            } => self.emit_for(peer, |peer_id| NodeEvent::FileFailed {
                file: FileRef::Incoming(transfer_id),
                peer_id,
                reason: FileFailReason::AbortedBySender(reason),
            }),
            FileEvent::Sent { peer, transfer_id } => {
                if let Some(&file) = self.outgoing_by_wire.get(&(peer, transfer_id)) {
                    self.emit_for(peer, |peer_id| NodeEvent::FileProgress {
                        file,
                        peer_id,
                        stage: FileStage::Sent,
                    });
                }
            }
            FileEvent::Delivered { peer, transfer_id } => {
                if let Some(file) = self.close_outgoing(peer, transfer_id) {
                    self.emit_for(peer, |peer_id| NodeEvent::FileProgress {
                        file,
                        peer_id,
                        stage: FileStage::Delivered,
                    });
                }
            }
            FileEvent::SendFailed {
                peer,
                transfer_id,
                reason,
            } => {
                if let Some(file) = self.close_outgoing(peer, transfer_id) {
                    self.emit_for(peer, |peer_id| NodeEvent::FileFailed {
                        file: FileRef::Outgoing(file),
                        peer_id,
                        reason: FileFailReason::Outgoing(reason),
                    });
                }
            }
        }
    }

    /// Doğrulanmış gelen dosyayı `<data_dir>/files/<eş>-<transfer_id>`'ye yazar; yol olayla gider.
    fn on_received(&self, peer: PeerRef, transfer_id: TransferId, bytes: &[u8]) {
        let Some(peer_id) = self.peers.name(peer) else {
            return;
        };
        let event = match self.received.store(peer_id, transfer_id, bytes) {
            Ok(path) => NodeEvent::FileReceived {
                peer_id: peer_id.to_owned(),
                transfer_id,
                path,
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            },
            Err(error) => NodeEvent::FileFailed {
                file: FileRef::Incoming(transfer_id),
                peer_id: peer_id.to_owned(),
                reason: FileFailReason::Storage(error.kind()),
            },
        };
        self.listener.on_event(event);
    }

    /// Giden aktarım son olayıyla bitti: tutamacı kapatır.
    fn close_outgoing(&mut self, peer: PeerRef, transfer_id: TransferId) -> Option<FileHandle> {
        let file = self.outgoing_by_wire.remove(&(peer, transfer_id))?;
        // Eşleme iki yönde aynı anda kurulur ve silinir: girdi vardır.
        let _ = self.outgoing.remove(file.raw(), &self.shared.handles);
        Some(file)
    }

    // ── Ölçüler ve olaylar ───────────────────────────────────────────────────────────────────

    fn publish_metrics(&mut self) {
        let snapshot = self.snapshot();
        let now = Instant::now();
        if now >= self.metrics_due {
            self.metrics_due = now + METRICS_TICK;
            self.listener.on_event(NodeEvent::Metrics(snapshot.clone()));
        }
        *lock(&self.shared.metrics) = snapshot;
    }

    /// Salt-okunur kaynaklardan anlık görüntü (bkz. `NodeMetrics` alan belgeleri).
    fn snapshot(&self) -> NodeMetrics {
        let transport = self.messenger.transport();
        let ctx = transport.ctx_dispatch_stats();
        let connect_state: Vec<PeerConnectState> = self
            .peers
            .iter()
            .map(|(name, peer)| {
                let (state, epoch) = match self.messenger.connect_state(peer) {
                    ConnectState::Idle => (LinkState::Idle, None),
                    ConnectState::Connecting { phase, .. } => (LinkState::Connecting(phase), None),
                    ConnectState::Ready(session) => {
                        (LinkState::Connected, transport.session_epoch(session))
                    }
                };
                PeerConnectState {
                    peer_id: name.to_owned(),
                    state,
                    epoch,
                }
            })
            .collect();
        NodeMetrics {
            rtt_ms: None,
            loss_pct: None,
            bitrate_kbps: None,
            epoch: connect_state.iter().filter_map(|peer| peer.epoch).min(),
            last_heartbeat_age_s: None,
            ctx_mismatch_drops: ctx.ctx_mismatch_drops,
            hub_frames_dropped: ctx.hub_frames_dropped,
            delivery_channel_drops: transport.delivery_dropped(),
            connect_state,
        }
    }

    /// Olayı eşin provisioning adıyla dinleyiciye verir. Ad bulunamazsa (Crypto dizin dışı eşle
    /// oturum açmaz; sözleşme dışı) olay düşer.
    fn emit_for(&self, peer: PeerRef, make: impl FnOnce(String) -> NodeEvent) {
        if let Some(name) = self.peers.name(peer) {
            self.listener.on_event(make(name.to_owned()));
        }
    }

    // ── [K4G-05] YALNIZ test-util: ölçüm sondası ve recovery-OFF kancası ───────────────────────

    /// İşlenen S1 oturum olayını sayar (salt sayım; olayın işlenişi değişmez).
    #[cfg(feature = "test-util")]
    fn count_session_event(&mut self, event: &SessionEvent) {
        let counts = &mut self.session_events;
        let counter = match event {
            SessionEvent::Ready(..) => &mut counts.ready,
            SessionEvent::Closed(..) => &mut counts.closed,
            SessionEvent::RenewRequired(_) => &mut counts.renew_required,
            SessionEvent::ConnectAbandoned(..) => &mut counts.connect_abandoned,
            SessionEvent::ConnectProgress(..) => return,
        };
        *counter = counter.saturating_add(1);
    }

    /// Ölçüm sondası (bkz. `crate::probe`): salt-okunur kaynakların o anki değeri. Röle zarf
    /// sayaçları süreç-geneldir; onları `KamaNode::probe_for_test` doldurur.
    #[cfg(feature = "test-util")]
    fn probe(&self) -> NodeProbe {
        let transport = self.messenger.transport();
        let ctx = transport.ctx_dispatch_stats();
        let sessions = transport
            .session_live_diags()
            .into_iter()
            .map(|(session, diag)| SessionProbe {
                session,
                peer_id: transport
                    .peer_of_for_test(session)
                    .and_then(|peer| self.peers.name(peer))
                    .map(str::to_owned),
                epoch: transport.session_epoch(session),
                diag,
            })
            .collect();
        NodeProbe {
            sessions,
            retired: transport.retired_live_diag(),
            not_ready_drops: ctx.not_ready_drops,
            ctx_mismatch_drops: ctx.ctx_mismatch_drops,
            hub_frames_dropped: ctx.hub_frames_dropped,
            delivery_channel_drops: transport.delivery_dropped(),
            outstanding_units: transport.outstanding_for_test(),
            messenger: self.messenger.stats(),
            events: self.session_events,
            envelopes: crate::probe::EnvelopeCounts::default(),
        }
    }

    /// Transport'un kurtarma anahtarı (`ClientTransport::set_recovery_for_test`; S1 H-APP-RETRY-0
    /// kancası): kapalıyken ARQ retransmit, §11 kopya ve FEC repair kararları tele çıkmaz.
    /// Sonradan açılan oturum uçlarına da uygulanır. YALNIZ recovery-OFF negatif kolu
    /// (`KamaNode::start_recovery_off_for_test`).
    #[cfg(feature = "test-util")]
    pub(crate) fn set_recovery_for_test(&mut self, on: bool) {
        self.messenger.transport_mut().set_recovery_for_test(on);
    }
}

/// Yanıt kanalına tek yanıt. Kanal kapasitesi bir ve tek gönderim olduğundan bloklamaz; çağıran
/// gitmişse (iş parçacığı bitmiş) yanıt düşer.
fn answer<T>(reply: &Reply<T>, result: Result<T, NodeError>) {
    let _ = reply.send(result);
}

/// Taze düğümün ölçüleri: bütün eşler boşta, sayaçlar sıfır.
pub(crate) fn idle_metrics(peers: &PeerBook) -> NodeMetrics {
    NodeMetrics {
        rtt_ms: None,
        loss_pct: None,
        bitrate_kbps: None,
        epoch: None,
        last_heartbeat_age_s: None,
        ctx_mismatch_drops: 0,
        hub_frames_dropped: 0,
        delivery_channel_drops: 0,
        connect_state: peers
            .iter()
            .map(|(name, _)| PeerConnectState {
                peer_id: name.to_owned(),
                state: LinkState::Idle,
                epoch: None,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod signaling_tests;
