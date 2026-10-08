//! L-T1 — Application ↔ Transport seam'inin (S1, [`KamaTransport`]) GERÇEK adaptörü.
//!
//! S1 çağrılarını iki alt yüzeye bağlar: veri yolu [`crate::runtime`] (oturum başına bir
//! [`ClientEndpoint`], düğüm başına bir [`ServerEndpoint`]) ve oturum kontrolü S3
//! (`SessionControl`; L-A5c `NoiseWireProtection` ya da sahtesi). Kripto bu modülün
//! ARKASINDADIR: Application yalnız [`KamaTransport`]'u görür (INV-NEIGHBOR-1).
//!
//! # Topoloji — direct-mode (iş emri T1)
//!
//! Her düğüm TEK bir veri alıcısı ([`ServerEndpoint`], `data_bind`) çalıştırır; her S1 oturumu için
//! eşin veri adresine bağlı ayrı bir [`ClientEndpoint`] açılır ve o ucun her çerçevesi
//! `ProtectionContext::Session(h)` ile mühürlenir. INIT/RESPONSE zarfları veri yolundan geçmez:
//! [`EnvelopeLink`] taşır (hub STORE/DELIVER'ın bu sprintteki dar yerine geçeni; iş emri T2). Hub
//! rölesi, hop-ARQ ve HubChannel ACK'i bu topolojide YOKTUR — INT-02 raporunda beyanlı boşluk.
//!
//! # Senkron yüzey, asenkron veri yolu
//!
//! [`KamaTransport`] senkrondur; veri yolu tokio görevleridir. Adaptör kendi çalışma-zamanını
//! taşır ve her S1 çağrısında bekleyen işi sürer ([`ClientTransport::drive`]): zarf olayları →
//! tutulan yenileme yanıtları → Crypto olayları → teslimler. Bekleyen bir niyet yalnız zarf
//! taşıyıcısından yanıt gelince çözülür; adaptör kendiliğinden yanıt uydurmaz.
//!
//! # Teslim sırası (S1 §4)
//!
//! Aynı oturumdaki `Reliable` birimler gönderim sırasıyla teslim edilir. Veri alıcısı birimleri
//! varış sırasıyla verir; adaptör onları kaynak uç başına `unit_id` sırasına sokar. Motor bir birimi
//! hiçbir zaman terk etmediği için (ARQ teyide kadar sürer) bu sıra kalıcı bir boşlukta beklemez;
//! `BestEffort` / `DeadlineBounded` birimler de aynı sırayla gelir (S1 onlara sırasızlık İZNİ verir,
//! zorunluluğu değil).
//!
//! # Yenileme: önce boşalt, sonra atomik cutover (S1 KARAR-2)
//!
//! `RenewRequired(h)` gelince adaptör arka planda yeni oturumu başlatır (`h` gönderim kabul etmeye
//! devam eder). Yeni oturumun RESPONSE'u gelince Crypto'ya hemen VERİLMEZ: `h`'nin teyitsiz
//! birimleri boşalana dek (ya da `renew_drain_cap` dolana dek) tutulur, sonra verilir. Crypto o anda
//! `h`'yi yalnız-alım drain'e alır (`Ready(h')`; `send(h)` → `SessionSuperseded`); drain bitince
//! `Closed(h, Superseded)`. Böylece cutover anında `h`'nin içinde kurtarılmayı bekleyen birim
//! kalmaz ve yeni oturuma hiçbir birim TAŞINMAZ (S1 K7).
//!
//! # Kalıcı niyet (DR-CONNECT-1, INV-NO-IDLE-TEARDOWN)
//!
//! `connect` için zaman sınırı yoktur. Crypto bekleyen bir INIT'in yarım-açık kaydını anahtar
//! hijyeni gereği düşürürse (`HALF_OPEN_TTL`) adaptör niyeti SÜRDÜRÜR: aynı bilet altında taze bir
//! INIT başlatır, Application hiçbir olay görmez. Niyet yalnız `Ready`, `cancel` ya da kapalı bir
//! `ConnectAbandoned` nedeniyle biter.
//!
//! # KAPSAM DIŞI (iş emri §4)
//!
//! `Suspended` / `LongOutage` tetikleri, heartbeat, backoff, rebind ve INIT'in byte-özdeş yeniden
//! gönderimi (T3: oturum kayıp başlamadan kurulur) bu sprintte YOKTUR; `ConnectProgress` yalnız
//! `Measuring` fazını üretir.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
use kama_contracts::s1::{
    AbandonReason, ConnectPhase, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy,
    KamaTransport, PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};
use kama_contracts::s3::{
    HubHandle, Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits,
    SealError, SealedUnit, SessionCommand, SessionControl, SessionEvent as CryptoEvent,
    SessionResult,
};

use crate::runtime::{
    ClientEndpoint, ClientHandle, RuntimeConfig, RuntimeError, ServerEndpoint, ServerHandle,
};
#[cfg(any(test, feature = "test-util"))]
use crate::wire::decode_data;
use crate::wire::HDR_DATA;

/// ccr §35.15 KS-DOS-1: "İstemci pending INIT tavanı = 16 (zincir gereği trial cap ile EŞİT)".
/// Tavan doluyken YENİ bir eşe `connect` → `Backpressure`.
pub const PENDING_INIT_CAP: usize = 16;

/// Application okumadan birikebilecek S1 olayı tavanı (SR-AUDIT-001). Dolunca adaptör yeni
/// olay üreten işi (zarf ve Crypto olayları) KAYNAKTA bekletir — olay düşmez.
pub const MAX_EVENTS: usize = 256;

/// Application okumadan birikebilecek teslim tavanı. Dolunca teslimler veri alıcısının kendi
/// sınırlı kanalında bekler — adaptör teslim düşürmez.
pub const MAX_INBOX: usize = 1024;

// ── Zarf taşıyıcısı (iş emri T2) ─────────────────────────────────────────────────────────────

/// Bir INIT gönderimini, taşıyıcının sonradan bildirdiği hub tutamacıyla eşleyen kimlik.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LinkAttempt(pub u64);

/// Zarf taşıyıcısının bildirdiği olay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// Hub bu INIT'in oturumuna veri yolu tutamacı atadı (noise-wire §2.55 HANDLE_ASSIGN).
    HandleAssigned {
        attempt: LinkAttempt,
        hub_handle: HubHandle,
    },
    /// Bir eşten SSI1 geldi; oturumun veri yolu tutamacı `hub_handle`'dır.
    IncomingInit {
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    /// Bekleyen bir oturuma SSR1 geldi (`hub_handle` = o oturumun tutamacı).
    Response {
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
}

/// Zarf taşıyıcısı yolu kuramadı (yol yok / taşıyıcı kapalı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("zarf taşıyıcısı kullanılamıyor")]
pub struct LinkError;

/// INIT/RESPONSE zarflarının taşıyıcısı (Plane 1'in bu sprintteki dar yerine geçeni). Zarf
/// baytları OPAKTIR; taşıyıcı onları değiştirmeden iletir (S3 K6).
pub trait EnvelopeLink: Send {
    /// SSI1'i eşe yollar.
    ///
    /// # Errors
    /// [`LinkError`] — yol kurulamadı.
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError>;

    /// SSR1'i INIT'i gönderen eşe, oturumun tutamacıyla yollar.
    ///
    /// # Errors
    /// [`LinkError`] — yol kurulamadı.
    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError>;

    /// Bekleyen bir taşıyıcı olayı varsa onu döndürür.
    fn poll(&mut self) -> Option<LinkEvent>;
}

// ── Yapılandırma ─────────────────────────────────────────────────────────────────────────────

/// Adaptör yapılandırması.
#[derive(Clone)]
pub struct ClientTransportConfig {
    /// Veri yolu yapılandırması (ürün profili: [`RuntimeConfig::default`]). Fragmenter KAPALI
    /// olmalıdır: bir S1 birimi bir veri birimidir.
    pub runtime: RuntimeConfig,
    /// Yerel veri alıcısının bağlanacağı adres.
    pub data_bind: SocketAddr,
    /// Eşlerin veri adresleri (direct-mode). Eksik eşe `connect` kalıcı yerel hatayla biter.
    pub peer_addrs: HashMap<PeerRef, SocketAddr>,
    /// Kayıp kurtarma. Ürün builder'ı `false`'u REDDEDER ([`ClientTransport::new`]); `false`
    /// yalnız test yapılandırmasıdır ve `TransportCapabilities::reliable` ile BEYAN edilir.
    pub recovery: bool,
    /// Yenilemede eski oturumun teyitsiz birimlerinin boşalması için azami bekleme; dolunca
    /// cutover yine yapılır (kalan birimler eski oturumda kalır, S1 K7).
    pub renew_drain_cap: Duration,
}

impl ClientTransportConfig {
    /// Ürün profiliyle yapılandırma: kurtarma açık, yenileme boşaltma tavanı veri yolunun
    /// kapanış boşaltma tavanıyla aynı.
    #[must_use]
    pub fn product(data_bind: SocketAddr) -> Self {
        let runtime = RuntimeConfig::default();
        let renew_drain_cap = runtime.drain_cap;
        Self {
            runtime,
            data_bind,
            peer_addrs: HashMap::new(),
            recovery: true,
            renew_drain_cap,
        }
    }
}

/// Adaptör kurulamadı.
#[derive(Debug, thiserror::Error)]
pub enum TransportBuildError {
    /// Kurtarma kapalı yapılandırma ürün builder'ında reddedilir (S1 `reliable`).
    #[error("kurtarma kapalı yapılandırma ürün builder'ında reddedilir (S1 reliable)")]
    RecoveryOff,
    /// Fragmenter açık: bir S1 birimi tek veri birimi olmalı.
    #[error("fragmenter açık yapılandırma desteklenmez (S1 birimi = bir veri birimi)")]
    FragmenterOn,
    /// Veri yolu kurulamadı.
    #[error("veri yolu: {0}")]
    Runtime(#[from] RuntimeError),
    /// Çalışma-zamanı kurulamadı.
    #[error("çalışma-zamanı: {0}")]
    Io(#[from] std::io::Error),
}

// ── Ortak Crypto dikişleri ───────────────────────────────────────────────────────────────────

/// Kilitleme zehirlenmeye karşı panik yapmaz: önceki tutucu panik etmiş olsa da durum okunur
/// (Crypto'nun her komutu kendi içinde tutarlıdır; burada panik zinciri kurmak daha kötü olurdu).
fn lock<C>(m: &Mutex<C>) -> MutexGuard<'_, C> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Veri alıcısına verilen dikiş: ortak Crypto'ya yönlendirir.
struct CryptoSeam<C>(Arc<Mutex<C>>);

impl<C: PacketProtection> PacketProtection for CryptoSeam<C> {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        lock(&self.0).seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        lock(&self.0).open(wire)
    }
    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        lock(&self.0).sealed_len(ctx, plaintext_len)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        lock(&self.0).conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        lock(&self.0).limits(ctx)
    }
}

/// Oturum uçlarına verilen dikiş: ortak Crypto + (yalnız test) ilk-emisyon kaybı planı.
struct EndpointSeam<C> {
    crypto: Arc<Mutex<C>>,
    /// İlk emisyonu kaybettirilecek birim kimlikleri (yalnız test). Kimliği adaptör `send`
    /// SIRASINDA kaydeder — kayıp eşzamanlı uç görevlerinin mühür sırasına bağlı kalmaz.
    #[cfg(any(test, feature = "test-util"))]
    drop_ids: Arc<Mutex<BTreeSet<u64>>>,
    /// Bu ucun mühürlediği en büyük birim kimliği: ilk emisyonlar artan sırayla çıkar, yeniden
    /// gönderimler daha küçük ya da eşit kimlik taşır.
    #[cfg(any(test, feature = "test-util"))]
    max_unit_sealed: Option<u64>,
}

impl<C: PacketProtection> PacketProtection for EndpointSeam<C> {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        #[allow(unused_mut)]
        let mut sealed = lock(&self.crypto).seal(ctx, frame)?;
        #[cfg(any(test, feature = "test-util"))]
        if let Some((_, unit, _)) = decode_data(frame) {
            if self.max_unit_sealed.is_none_or(|m| unit > m) {
                self.max_unit_sealed = Some(unit);
                let take = self
                    .drop_ids
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&unit);
                if take {
                    // İlk emisyonun kaybı: tel baytı bozulur → alıcının AEAD'i reddeder (sessiz
                    // düşme; PN tüketildi, kurtarma Transport'un işidir).
                    if let Some(b) = sealed.bytes.last_mut() {
                        *b ^= 0xFF;
                    }
                }
            }
        }
        Ok(sealed)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        lock(&self.crypto).open(wire)
    }
    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        lock(&self.crypto).sealed_len(ctx, plaintext_len)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        lock(&self.crypto).conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        lock(&self.crypto).limits(ctx)
    }
}

// ── Durum ────────────────────────────────────────────────────────────────────────────────────

/// S1 oturumunun evresi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Gönderim kabul eder.
    Active,
    /// `Ready(h')` verildi; `Closed(h, Superseded)` henüz okunmadı → `send(h)` →
    /// `SessionSuperseded`. Alım drain'i sürer.
    Superseded,
    /// `Closed` kuyrukta, henüz okunmadı → `send(h)` → `UnknownSession`.
    Closed,
}

struct Sess {
    peer: PeerRef,
    phase: Phase,
    endpoint: Option<ClientHandle>,
    /// Uca verilen birim sayısı: veri yolu birimlere sırayla kimlik verir (fragmenter kapalı →
    /// birim = mesaj), sıradaki birimin kimliği budur.
    sent: u64,
    /// Uç dikişiyle paylaşılan ilk-emisyon kaybı kümesi (yalnız test).
    #[cfg(any(test, feature = "test-util"))]
    drop_ids: Arc<Mutex<BTreeSet<u64>>>,
}

/// Bekleyen `connect` niyeti.
struct Intent {
    ticket: ConnectTicket,
    peer: PeerRef,
    /// Crypto'nun bekleyen oturumu (INIT'i uçuşta).
    pending: SessionHandle,
    /// `Ready`'de ilk gönderim olur (S1 K6; Crypto'ya verilmez).
    early: Option<Vec<u8>>,
}

/// Kendi başlattığımız yenileme.
struct Renewal {
    old: SessionHandle,
    peer: PeerRef,
    pending: SessionHandle,
    /// Tutulan RESPONSE ve tutulmaya başlandığı an.
    held: Option<(Vec<u8>, Instant)>,
}

/// Kaynak uç başına sıralı teslim (S1 §4).
#[derive(Default)]
struct Reorder {
    next: u64,
    held: BTreeMap<u64, (SessionHandle, Vec<u8>)>,
}

impl Reorder {
    /// Birimi koyar; sırası gelen birimler `out`'a akar. Tutulan birim `cap`'i aşarsa en küçük
    /// boşluk atlanır (motorun gönderim penceresi bunu meşru trafikte imkânsız kılar).
    fn push(
        &mut self,
        unit: u64,
        session: SessionHandle,
        payload: Vec<u8>,
        cap: usize,
        out: &mut VecDeque<Delivered>,
    ) {
        if unit < self.next {
            return; // veri alıcısının ilk-teslim penceresinin dışından gelen geç kopya
        }
        self.held.entry(unit).or_insert((session, payload));
        if self.held.len() > cap {
            if let Some((&first, _)) = self.held.first_key_value() {
                self.next = first;
            }
        }
        while let Some(entry) = self.held.first_entry() {
            if *entry.key() != self.next {
                break;
            }
            let (session, unit) = entry.remove();
            out.push_back(Delivered { session, unit });
            self.next = self.next.saturating_add(1);
        }
    }
}

// ── Adaptör ──────────────────────────────────────────────────────────────────────────────────

/// S1 [`KamaTransport`]'un gerçek implementasyonu (bkz. modül belgesi).
pub struct ClientTransport<C: PacketProtection + SessionControl> {
    caps: TransportCapabilities,
    runtime_cfg: RuntimeConfig,
    peer_addrs: HashMap<PeerRef, SocketAddr>,
    renew_drain_cap: Duration,
    /// Uçlara uygulanan kurtarma anahtarı (ürün: daima açık).
    recovery_on: bool,
    crypto: Arc<Mutex<C>>,
    link: Box<dyn EnvelopeLink>,
    next_ticket: u64,
    intents: Vec<Intent>,
    sessions: BTreeMap<SessionHandle, Sess>,
    /// Yanıtlayan rolde kurulan oturumun eşi (Crypto'nun `Ready`'si eş taşımaz).
    responder_peer: BTreeMap<SessionHandle, PeerRef>,
    /// Onay bekleyen provisional supersession: yeni → (eş, eski).
    provisional: BTreeMap<SessionHandle, (PeerRef, SessionHandle)>,
    renewals: Vec<Renewal>,
    attempts: BTreeMap<LinkAttempt, SessionHandle>,
    /// Bekleyen oturumların hub tutamacı (RESPONSE'u yenilemeyle eşlemek için).
    hub_of: BTreeMap<SessionHandle, HubHandle>,
    /// `close` ile kapatılan oturumlar: Crypto'nun `Closed(UserClosed)`'ı yutulur (olay verildi).
    locally_closed: BTreeSet<SessionHandle>,
    events: VecDeque<SessionEvent>,
    inbox: VecDeque<Delivered>,
    reorder: HashMap<SocketAddr, Reorder>,
    /// Sıradaki `send`'lerden kaçının ilk emisyonu kaybettirilecek (yalnız test).
    #[cfg(any(test, feature = "test-util"))]
    drops: usize,
    server: ServerHandle,
    /// SON alan: alanlar bildirim sırasıyla düşer — uç tutamaçları çalışma-zamanından önce.
    rt: tokio::runtime::Runtime,
}

impl<C: PacketProtection + SessionControl> ClientTransport<C> {
    /// Ürün builder'ı.
    ///
    /// # Errors
    /// [`TransportBuildError::RecoveryOff`] (kurtarma kapalı yapılandırma),
    /// [`TransportBuildError::FragmenterOn`], veri yolu ya da çalışma-zamanı kurulum hatası.
    pub fn new(
        cfg: ClientTransportConfig,
        crypto: C,
        link: Box<dyn EnvelopeLink>,
    ) -> Result<Self, TransportBuildError> {
        if !cfg.recovery {
            return Err(TransportBuildError::RecoveryOff);
        }
        Self::build(cfg, crypto, link)
    }

    /// Test builder'ı: kurtarma kapalı yapılandırmayı da kabul eder (`reliable = false` beyanı).
    ///
    /// # Errors
    /// Bkz. [`Self::new`] (`RecoveryOff` hariç).
    #[cfg(any(test, feature = "test-util"))]
    pub fn new_for_test(
        cfg: ClientTransportConfig,
        crypto: C,
        link: Box<dyn EnvelopeLink>,
    ) -> Result<Self, TransportBuildError> {
        Self::build(cfg, crypto, link)
    }

    fn build(
        cfg: ClientTransportConfig,
        crypto: C,
        link: Box<dyn EnvelopeLink>,
    ) -> Result<Self, TransportBuildError> {
        if cfg.runtime.max_message_bytes.is_some() {
            return Err(TransportBuildError::FragmenterOn);
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let crypto = Arc::new(Mutex::new(crypto));
        let server = rt.block_on(ServerEndpoint::bind(
            cfg.data_bind,
            cfg.runtime.clone(),
            Box::new(CryptoSeam(Arc::clone(&crypto))),
        ))?;
        // K5: kapasite Crypto'nun oturum sınıfı için güvenli minimumundan türer; veri çerçevesinin
        // başlığı düşülür ve veri yolunun birim tavanıyla kırpılır.
        let limits = lock(&crypto).conservative_limits(ProtectionClass::Session);
        let unit_cap = (limits.max_plaintext_bytes as usize)
            .saturating_sub(HDR_DATA)
            .min(cfg.runtime.max_unit_bytes);
        let unit_cap = u32::try_from(unit_cap).unwrap_or(u32::MAX);
        Ok(Self {
            caps: TransportCapabilities {
                max_plaintext_unit_bytes: unit_cap,
                preferred_plaintext_unit_bytes: unit_cap,
                // early_unit bu sprint INIT'e gömülmez: Ready'de ilk veri birimi olarak gider (S1).
                max_early_unit_bytes: unit_cap,
                reliable: cfg.recovery,
            },
            runtime_cfg: cfg.runtime,
            peer_addrs: cfg.peer_addrs,
            renew_drain_cap: cfg.renew_drain_cap,
            recovery_on: cfg.recovery,
            crypto,
            link,
            next_ticket: 0,
            intents: Vec::new(),
            sessions: BTreeMap::new(),
            responder_peer: BTreeMap::new(),
            provisional: BTreeMap::new(),
            renewals: Vec::new(),
            attempts: BTreeMap::new(),
            hub_of: BTreeMap::new(),
            locally_closed: BTreeSet::new(),
            events: VecDeque::new(),
            inbox: VecDeque::new(),
            reorder: HashMap::new(),
            #[cfg(any(test, feature = "test-util"))]
            drops: 0,
            server,
            rt,
        })
    }

    /// Yerel veri alıcısının adresi (eşlerin `peer_addrs`'ı için).
    #[must_use]
    pub fn data_addr(&self) -> SocketAddr {
        self.server.local_addr()
    }

    /// Bir eşin veri adresini kaydeder (direct-mode adres defteri).
    pub fn add_peer_addr(&mut self, peer: PeerRef, addr: SocketAddr) {
        self.peer_addrs.insert(peer, addr);
    }

    /// Bekleyen işi sürer: zarf olayları → tutulan yenileme yanıtları → Crypto olayları →
    /// teslimler. Her S1 çağrısı bunu kendisi çağırır; uygulama döngüsü de doğrudan çağırabilir.
    pub fn drive(&mut self) {
        self.drive_link();
        self.release_held_renewals();
        self.drive_crypto_events();
        self.drive_deliveries();
    }

    fn command(&self, cmd: SessionCommand) -> SessionResult {
        lock(&self.crypto).command(cmd)
    }

    fn room_for_events(&self) -> bool {
        self.events.len() < MAX_EVENTS
    }

    // ── zarf yolu ──

    fn drive_link(&mut self) {
        while self.room_for_events() {
            let Some(ev) = self.link.poll() else { break };
            match ev {
                LinkEvent::HandleAssigned {
                    attempt,
                    hub_handle,
                } => {
                    if let Some(pending) = self.attempts.remove(&attempt) {
                        let r = self.command(SessionCommand::AssignHandle {
                            session: pending,
                            hub_handle,
                        });
                        if r == SessionResult::NoOutput {
                            self.hub_of.insert(pending, hub_handle);
                        }
                    }
                }
                LinkEvent::IncomingInit {
                    hub_handle, bytes, ..
                } => self.on_incoming_init(hub_handle, bytes),
                LinkEvent::Response {
                    hub_handle, bytes, ..
                } => self.on_response(hub_handle, bytes),
            }
        }
    }

    fn on_incoming_init(&mut self, hub_handle: HubHandle, wire: Vec<u8>) {
        match self.command(SessionCommand::AcceptInit { wire }) {
            SessionResult::EstablishedAndReply {
                session,
                peer,
                envelope_type,
                bytes,
            } => {
                let _ = self.command(SessionCommand::AssignHandle {
                    session,
                    hub_handle,
                });
                self.hub_of.insert(session, hub_handle);
                self.responder_peer.insert(session, peer);
                let _ = self
                    .link
                    .send_response(peer, hub_handle, envelope_type, bytes);
            }
            SessionResult::ProvisionalAndReply {
                new,
                supersedes,
                peer,
                envelope_type,
                bytes,
            } => {
                let _ = self.command(SessionCommand::AssignHandle {
                    session: new,
                    hub_handle,
                });
                self.hub_of.insert(new, hub_handle);
                self.provisional.insert(new, (peer, supersedes));
                let _ = self
                    .link
                    .send_response(peer, hub_handle, envelope_type, bytes);
            }
            // `Failed` OPAKTIR (S3 §3): sahte/bozuk/bayat INIT hiçbir olay üretmez.
            _ => {}
        }
    }

    fn on_response(&mut self, hub_handle: HubHandle, wire: Vec<u8>) {
        let renewal = self
            .renewals
            .iter()
            .position(|r| self.hub_of.get(&r.pending) == Some(&hub_handle));
        match renewal {
            // KARAR-2: önce eski oturum boşalır, sonra cutover — yanıt tutulur.
            Some(i) => self.renewals[i].held = Some((wire, Instant::now())),
            // `Failed` OPAKTIR: sahte RESPONSE niyeti DÜŞÜRMEZ, olay üretmez.
            None => {
                let _ = self.command(SessionCommand::AcceptResponse { wire });
            }
        }
    }

    fn release_held_renewals(&mut self) {
        let now = Instant::now();
        for i in 0..self.renewals.len() {
            let Some((_, since)) = &self.renewals[i].held else {
                continue;
            };
            let old = self.renewals[i].old;
            let drained = self
                .sessions
                .get(&old)
                .and_then(|s| s.endpoint.as_ref())
                .is_none_or(|e| e.outstanding() == 0);
            if !drained && now.saturating_duration_since(*since) < self.renew_drain_cap {
                continue;
            }
            if let Some((wire, _)) = self.renewals[i].held.take() {
                // Established → Crypto `Ready(pending)` → `on_crypto_ready` eşler. Failed (yanlış
                // aday) → yenileme sürer, yeni bir yanıt beklenir.
                let _ = self.command(SessionCommand::AcceptResponse { wire });
            }
        }
    }

    // ── Crypto olayları ──

    fn drive_crypto_events(&mut self) {
        while self.room_for_events() {
            let ev = lock(&self.crypto).poll_event();
            let Some(ev) = ev else { break };
            match ev {
                CryptoEvent::Ready(h) => self.on_crypto_ready(h),
                CryptoEvent::Confirmed(h) => self.on_confirmed(h),
                CryptoEvent::RenewRequired(h) => self.on_renew_required(h),
                CryptoEvent::Closed(h, reason) => self.on_crypto_closed(h, reason),
            }
        }
    }

    fn on_crypto_ready(&mut self, h: SessionHandle) {
        // (a) Kendi yenilemem tamamlandı: atomik cutover.
        if let Some(i) = self.renewals.iter().position(|r| r.pending == h) {
            let r = self.renewals.remove(i);
            self.hub_of.remove(&h);
            if let Some(old) = self.sessions.get_mut(&r.old) {
                if old.phase == Phase::Active {
                    old.phase = Phase::Superseded;
                }
            }
            self.activate(h, r.peer, None);
            return;
        }
        // (b) Başlattığım niyet çözüldü.
        if let Some(i) = self.intents.iter().position(|it| it.pending == h) {
            let it = self.intents.remove(i);
            self.hub_of.remove(&h);
            self.activate(h, it.peer, it.early);
            return;
        }
        // (c) Yanıtlayan rolde kurulan oturum. Glare'i kaybettiysek Crypto bu eşe bekleyen
        // INIT'imizi sildi; niyetimiz bu oturumla çözülür.
        if let Some(peer) = self.responder_peer.remove(&h) {
            self.hub_of.remove(&h);
            let early = self
                .intents
                .iter()
                .position(|it| it.peer == peer)
                .and_then(|i| {
                    let it = self.intents.remove(i);
                    self.attempts.retain(|_, p| *p != it.pending);
                    self.hub_of.remove(&it.pending);
                    it.early
                });
            self.activate(h, peer, early);
        }
    }

    fn on_confirmed(&mut self, new: SessionHandle) {
        let Some((peer, old)) = self.provisional.remove(&new) else {
            return;
        };
        self.hub_of.remove(&new);
        if let Some(s) = self.sessions.get_mut(&old) {
            if s.phase == Phase::Active {
                s.phase = Phase::Superseded;
            }
        }
        self.activate(new, peer, None);
    }

    fn on_renew_required(&mut self, h: SessionHandle) {
        let Some(peer) = self
            .sessions
            .get(&h)
            .filter(|s| s.phase == Phase::Active)
            .map(|s| s.peer)
        else {
            return;
        };
        self.events.push_back(SessionEvent::RenewRequired(h));
        if self.renewals.iter().any(|r| r.old == h) {
            return;
        }
        if let SessionResult::Initiated {
            session: pending,
            envelope_type,
            bytes,
        } = self.command(SessionCommand::Renew { session: h })
        {
            if let Ok(a) = self.link.send_init(peer, envelope_type, bytes) {
                self.attempts.insert(a, pending);
            }
            self.renewals.push(Renewal {
                old: h,
                peer,
                pending,
                held: None,
            });
        }
    }

    fn on_crypto_closed(&mut self, h: SessionHandle, reason: CloseReason) {
        // (1) Bekleyen niyetin INIT'i anahtar hijyeniyle düştü: niyet kalıcıdır → taze INIT.
        if let Some(i) = self.intents.iter().position(|it| it.pending == h) {
            self.attempts.retain(|_, p| *p != h);
            self.hub_of.remove(&h);
            self.reinitiate(i);
            return;
        }
        // (2) Bekleyen yenileme düştü: eski oturum sürer.
        if let Some(i) = self.renewals.iter().position(|r| r.pending == h) {
            self.renewals.remove(i);
            self.attempts.retain(|_, p| *p != h);
            self.hub_of.remove(&h);
            return;
        }
        // (3) Onaylanmayan provisional oturum hiçbir olay üretmez.
        if self.provisional.remove(&h).is_some() {
            self.hub_of.remove(&h);
            return;
        }
        // (4) `close` ile yerelde kapatılan: olay zaten kuyruğa girdi.
        if self.locally_closed.remove(&h) {
            return;
        }
        // (5) S1 oturumu sona erdi.
        let Some(s) = self.sessions.get_mut(&h) else {
            return;
        };
        if s.phase == Phase::Closed {
            return;
        }
        // F0.2(i): supersede edilmiş tutamaç `Closed` OKUNANA dek `SessionSuperseded` döner;
        // diğer kapanışlarda tutamaç hemen kullanılamaz (`UnknownSession`).
        if s.phase == Phase::Active {
            s.phase = Phase::Closed;
        }
        s.endpoint = None;
        self.events.push_back(SessionEvent::Closed(h, reason));
        if let Some(i) = self.renewals.iter().position(|r| r.old == h) {
            let r = self.renewals.remove(i);
            let _ = self.command(SessionCommand::Close { session: r.pending });
        }
    }

    fn reinitiate(&mut self, i: usize) {
        let peer = self.intents[i].peer;
        match self.command(SessionCommand::Initiate {
            peer,
            early_unit: None,
        }) {
            SessionResult::Initiated {
                session,
                envelope_type,
                bytes,
            } => {
                self.intents[i].pending = session;
                if let Ok(a) = self.link.send_init(peer, envelope_type, bytes) {
                    self.attempts.insert(a, session);
                }
            }
            _ => {
                // Kalıcı yerel hata (eş malzemesi yok): niyet kapalı nedenle biter.
                let it = self.intents.remove(i);
                self.events.push_back(SessionEvent::ConnectAbandoned(
                    it.ticket,
                    AbandonReason::LocalPermanent,
                ));
            }
        }
    }

    /// Oturumu S1'e açar: veri ucu + `Ready(h, peer)` + (varsa) `early_unit` ilk gönderim.
    fn activate(&mut self, h: SessionHandle, peer: PeerRef, early: Option<Vec<u8>>) {
        #[cfg(any(test, feature = "test-util"))]
        let drop_ids = Arc::new(Mutex::new(BTreeSet::new()));
        let endpoint = self.open_endpoint(
            h,
            peer,
            #[cfg(any(test, feature = "test-util"))]
            Arc::clone(&drop_ids),
        );
        let mut sent = 0;
        if let (Some(e), Some(bytes)) = (endpoint.as_ref(), early) {
            // Kuyruk taze uçta boştur; ret yalnız uç kurulamadıysa olur (birim düşer).
            if e.send(bytes).is_ok() {
                sent = 1;
            }
        }
        self.sessions.insert(
            h,
            Sess {
                peer,
                phase: Phase::Active,
                endpoint,
                sent,
                #[cfg(any(test, feature = "test-util"))]
                drop_ids,
            },
        );
        self.events.push_back(SessionEvent::Ready(h, peer));
    }

    fn open_endpoint(
        &self,
        h: SessionHandle,
        peer: PeerRef,
        #[cfg(any(test, feature = "test-util"))] drop_ids: Arc<Mutex<BTreeSet<u64>>>,
    ) -> Option<ClientHandle> {
        let addr = *self.peer_addrs.get(&peer)?;
        let seam = EndpointSeam {
            crypto: Arc::clone(&self.crypto),
            #[cfg(any(test, feature = "test-util"))]
            drop_ids,
            #[cfg(any(test, feature = "test-util"))]
            max_unit_sealed: None,
        };
        let endpoint = self
            .rt
            .block_on(ClientEndpoint::connect_session(
                addr,
                self.runtime_cfg.clone(),
                Box::new(seam),
                None,
                ProtectionContext::Session(h),
            ))
            .ok()?;
        endpoint.set_recovery(self.recovery_on);
        Some(endpoint)
    }

    // ── teslimler ──

    fn drive_deliveries(&mut self) {
        let cap = self.runtime_cfg.max_inflight_units;
        while self.inbox.len() < MAX_INBOX {
            let Some(d) = self.server.try_next_delivery() else {
                break;
            };
            let ProtectionContext::Session(h) = d.ctx else {
                continue;
            };
            if !self.sessions.contains_key(&h) {
                // Yarış: oturumu duyuran Crypto olayı (onay) teslimden sonra işlenebilir.
                self.drive_crypto_events();
            }
            match self.sessions.get(&h) {
                Some(s) if s.phase != Phase::Closed => {}
                // Kapanmış / bilinmeyen oturumun teslimi düşer.
                _ => continue,
            }
            self.reorder.entry(d.peer).or_default().push(
                d.unit_id,
                h,
                d.payload,
                cap,
                &mut self.inbox,
            );
        }
    }

    // ── yalnız test (`test-util`) ──

    /// Uçların kurtarma anahtarı (S1 H-APP-RETRY-0 kancası). Sonradan açılan uçlara da uygulanır.
    #[cfg(any(test, feature = "test-util"))]
    pub fn set_recovery_for_test(&mut self, on: bool) {
        self.recovery_on = on;
        for s in self.sessions.values() {
            if let Some(e) = &s.endpoint {
                e.set_recovery(on);
            }
        }
    }

    /// Sonraki `n` `send`'in İLK emisyonu kaybettirilir (tel baytı bozulur; S1 kancası). Sıra
    /// `send` çağrı sırasıdır — eşzamanlı uç görevlerinin mühür sırası değil.
    #[cfg(any(test, feature = "test-util"))]
    pub fn drop_next_units_for_test(&mut self, n: usize) {
        self.drops = self.drops.saturating_add(n);
    }

    /// Bütün uçlarda kabul edilmiş ama teyidi gelmemiş birim sayısı (fikstürün durgunluk ölçüsü).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn outstanding_for_test(&self) -> usize {
        self.sessions
            .values()
            .filter_map(|s| s.endpoint.as_ref())
            .map(ClientHandle::outstanding)
            .sum()
    }

    /// Teslim kuyruğunda (sıralı, okunmayı bekleyen) birim sayısı.
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn inbox_len_for_test(&self) -> usize {
        self.inbox.len()
    }

    /// Eşin gönderim kabul eden (aktif) oturumu — fikstür yenileme/supersession'ın bittiğini
    /// buradan okur (olay kuyruğuna dokunmadan).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn active_session_for_test(&self, peer: PeerRef) -> Option<SessionHandle> {
        self.sessions
            .iter()
            .find(|(_, s)| s.peer == peer && s.phase == Phase::Active)
            .map(|(h, _)| *h)
    }

    /// S1'in bildiği bir oturumun eşi.
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn peer_of_for_test(&self, session: SessionHandle) -> Option<PeerRef> {
        self.sessions.get(&session).map(|s| s.peer)
    }
}

impl<C: PacketProtection + SessionControl> KamaTransport for ClientTransport<C> {
    fn capabilities(&self) -> TransportCapabilities {
        self.caps
    }

    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError> {
        if let Some((h, _)) = self
            .sessions
            .iter()
            .find(|(_, s)| s.peer == peer && s.phase == Phase::Active)
        {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }
            return Ok(ConnectResult::Ready(*h));
        }
        if let Some(it) = self.intents.iter().find(|it| it.peer == peer) {
            if early_unit.is_some() {
                return Err(TransportError::EarlyUnitConflict);
            }
            return Ok(ConnectResult::Pending(it.ticket));
        }
        if early_unit
            .as_ref()
            .is_some_and(|u| u.bytes.len() > self.caps.max_early_unit_bytes as usize)
        {
            return Err(TransportError::UnitTooLarge);
        }
        if self.intents.len() >= PENDING_INIT_CAP {
            return Err(TransportError::Backpressure);
        }
        let ticket = ConnectTicket::from_raw(self.next_ticket);
        self.next_ticket = self.next_ticket.saturating_add(1);
        // F0.2(ii): `Pending` döndüğü anda `Measuring`.
        self.events.push_back(SessionEvent::ConnectProgress(
            ticket,
            ConnectPhase::Measuring,
        ));
        if !self.peer_addrs.contains_key(&peer) {
            // Direct-mode: eşin veri adresi yok → kalıcı yerel hata.
            self.events.push_back(SessionEvent::ConnectAbandoned(
                ticket,
                AbandonReason::LocalPermanent,
            ));
            return Ok(ConnectResult::Pending(ticket));
        }
        match self.command(SessionCommand::Initiate {
            peer,
            early_unit: None,
        }) {
            SessionResult::Initiated {
                session,
                envelope_type,
                bytes,
            } => {
                if let Ok(a) = self.link.send_init(peer, envelope_type, bytes) {
                    self.attempts.insert(a, session);
                }
                self.intents.push(Intent {
                    ticket,
                    peer,
                    pending: session,
                    early: early_unit.map(|u| u.bytes),
                });
            }
            // G1: dizinde olmayan eş → opak `Failed` → kalıcı yerel hata (eş malzemesi yok).
            _ => self.events.push_back(SessionEvent::ConnectAbandoned(
                ticket,
                AbandonReason::LocalPermanent,
            )),
        }
        Ok(ConnectResult::Pending(ticket))
    }

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        let unit_cap = self.caps.max_plaintext_unit_bytes as usize;
        let s = self
            .sessions
            .get_mut(&session)
            .ok_or(TransportError::UnknownSession)?;
        match s.phase {
            Phase::Active => {}
            Phase::Superseded => return Err(TransportError::SessionSuperseded),
            Phase::Closed => return Err(TransportError::UnknownSession),
        }
        if unit.bytes.len() > unit_cap {
            return Err(TransportError::UnitTooLarge);
        }
        let deadline_ns = match policy {
            DeliveryPolicy::DeadlineBounded { deadline } => {
                let now = Instant::now();
                if deadline <= now {
                    return Err(TransportError::DeadlineExpired);
                }
                Some(u64::try_from((deadline - now).as_nanos()).unwrap_or(u64::MAX))
            }
            DeliveryPolicy::Reliable | DeliveryPolicy::BestEffort => None,
        };
        let endpoint = s.endpoint.as_ref().ok_or(TransportError::NotConnected)?;
        // (yalnız test) Kayıp, birim uca verilmeden ÖNCE kimliğiyle kaydedilir.
        #[cfg(any(test, feature = "test-util"))]
        let marked = self.drops > 0
            && s.drop_ids
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(s.sent);
        let result = endpoint
            .send_with_deadline(unit.bytes, deadline_ns)
            .map_err(|e| match e {
                RuntimeError::UnitTooLarge { .. } => TransportError::UnitTooLarge,
                RuntimeError::QueueFull => TransportError::Backpressure,
                _ => TransportError::NotConnected,
            });
        #[cfg(any(test, feature = "test-util"))]
        if marked {
            if result.is_ok() {
                self.drops -= 1;
            } else {
                s.drop_ids
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&s.sent);
            }
        }
        if result.is_ok() {
            s.sent = s.sent.saturating_add(1);
        }
        result
    }

    fn recv(&mut self) -> Option<Delivered> {
        self.drive();
        self.inbox.pop_front()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.drive();
        let ev = self.events.pop_front()?;
        if let SessionEvent::Closed(h, _) = ev {
            // F0.2(i): `Closed(h)` OKUNDUKTAN sonra tutamaç bilinmez.
            self.sessions.remove(&h);
        }
        Some(ev)
    }

    fn close(&mut self, session: SessionHandle) -> Result<(), TransportError> {
        let s = self
            .sessions
            .get_mut(&session)
            .ok_or(TransportError::UnknownSession)?;
        match s.phase {
            Phase::Active => {}
            Phase::Superseded => return Err(TransportError::SessionSuperseded),
            Phase::Closed => return Err(TransportError::UnknownSession),
        }
        s.phase = Phase::Closed;
        s.endpoint = None;
        self.locally_closed.insert(session);
        let _ = self.command(SessionCommand::Close { session });
        if let Some(i) = self.renewals.iter().position(|r| r.old == session) {
            let r = self.renewals.remove(i);
            let _ = self.command(SessionCommand::Close { session: r.pending });
        }
        self.events
            .push_back(SessionEvent::Closed(session, CloseReason::UserClosed));
        Ok(())
    }

    fn cancel(&mut self, ticket: ConnectTicket) {
        let Some(i) = self.intents.iter().position(|it| it.ticket == ticket) else {
            return;
        };
        let it = self.intents.remove(i);
        // s3 v1.1: Close bekleyeni de kapatır; geç gelen RESPONSE `Failed` olur.
        let _ = self.command(SessionCommand::Close {
            session: it.pending,
        });
        self.attempts.retain(|_, p| *p != it.pending);
        self.hub_of.remove(&it.pending);
        self.events.push_back(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::Cancelled,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_init_cap_matches_the_crypto_trial_cap_chain() {
        // ccr §35.15: istemci pending INIT tavanı trial cap ile EŞİT (zincir).
        assert_eq!(
            PENDING_INIT_CAP,
            kama_noise_wire::session_control::PENDING_RESPONSE_TRIAL_CAP
        );
    }

    #[test]
    fn reorder_delivers_in_unit_order_and_skips_nothing() {
        let h = SessionHandle::from_raw(7);
        let mut r = Reorder::default();
        let mut out = VecDeque::new();
        r.push(1, h, b"b".to_vec(), 16, &mut out);
        assert!(out.is_empty(), "0 gelmeden 1 teslim edilmez");
        r.push(0, h, b"a".to_vec(), 16, &mut out);
        r.push(1, h, b"tekrar".to_vec(), 16, &mut out);
        let got: Vec<Vec<u8>> = out.into_iter().map(|d| d.unit).collect();
        assert_eq!(got, [b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn reorder_is_bounded_and_advances_past_a_gap_only_over_cap() {
        let h = SessionHandle::from_raw(1);
        let mut r = Reorder::default();
        let mut out = VecDeque::new();
        for u in 1..=3 {
            r.push(u, h, vec![u as u8], 3, &mut out);
        }
        assert!(out.is_empty(), "tavan dolmadan boşluk atlanmaz");
        r.push(4, h, vec![4], 3, &mut out);
        let got: Vec<u8> = out.into_iter().map(|d| d.unit[0]).collect();
        assert_eq!(
            got,
            [1, 2, 3, 4],
            "tavan aşılınca en küçük boşluk atlanır, sıra korunur"
        );
    }
}
