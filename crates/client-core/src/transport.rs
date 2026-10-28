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
//! varış sırasıyla verir; adaptör onları kaynak uç (= oturum) başına `unit_id` sırasına sokar.
//! Motor bir birimi hiçbir zaman terk etmediği için (ARQ teyide kadar sürer) bu sıra kalıcı bir
//! boşlukta beklemez; `BestEffort` / `DeadlineBounded` birimler de aynı sırayla gelir (S1 onlara
//! sırasızlık İZNİ verir, zorunluluğu değil).
//!
//! [K4G-01b] Sıralama durumu (`Reorder`) OTURUMA anahtarlıdır (`SessionHandle`; kaynak adres
//! DEĞİL): karşı ucun her oturumu tek bir uçtan gelir, `unit_id` uzayı oturumundur. Böylece
//! (i) aynı adresten gelen yeni oturumun birimleri eski oturumun `next`'ine takılıp sessizce
//! atılmaz; (ii) aynı oturumun birden çok eş yuvasından (adres) gelen kopyaları — ör. hattaki
//! saldırganın sahte adresten önce ilettiği kopya — S1'e TEK kez teslim edilir. Oturum `Closed`
//! olunca (teslim kabulü bitince) durumu silinir; tablo oturum tablosuyla sınırlıdır.
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
//! # Uç açılmadan `Ready` yok (K4G-05g, CEVAP PAKETİ 14 §5)
//!
//! Bir oturum S1'e YALNIZ veri ucu açılmışsa (ve varsa `early_unit` o uca verilmişse) `Ready`
//! olarak açılır. Uç açılamazsa (eşin veri adresi yok ya da soket kurulamadı) yol başına:
//!
//! | Yol | App bilir | S1 olayı | Crypto oturumu |
//! |---|---|---|---|
//! | bekleyen niyet (başlatan) | bilet | `ConnectAbandoned(_, LocalPermanent)` | `Close` |
//! | yanıtlayan, glare kaybı | bilet | `ConnectAbandoned(_, LocalPermanent)` | `Close` |
//! | yanıtlayan, niyet yok | hiçbir şey | yok (S1 K2/K7) | `Close` |
//! | yenileme cutover'ı | eski tutamaç | yok (aşağıda) | yenileme sürer |
//! | yenileme yedek kolu | eski tutamaç | `Closed(eski, Superseded)` | yeni + eski `Close` |
//! | provisional onayı | eski tutamaç | `Closed(eski, Superseded)` | yeni + eski `Close` |
//!
//! Niyeti olmayan yanıtlayanda tutamaç Application'a hiç verilmemiştir: `Ready`siz `Closed`
//! K7'nin olay sırasını (`Ready` → … → `Closed`) bozardı. Yenilemede uç `AcceptResponse`'tan ÖNCE
//! açılır (Crypto `Established`'de eski oturumu geri alınamaz biçimde drain'e alır); açılamazsa
//! yanıt tutulur, eski oturum tam çalışır, cutover sonraki `drive`'da yeniden denenir.
//!
//! Yerine geçen oturumun ucu, Crypto eski oturumu ZATEN drain'e aldıktan sonra açılamazsa —
//! provisional onayı (Crypto'nun açma yolunda verilir) ya da yenilemenin yedek kolu (yanıt
//! Crypto'da yenilemeyle tutulmadan eşleşti, uç önceden açılmadı) — S1 K7'nin TEK istisnası
//! uygulanır (DUR-05g-1 (a), CEVAP PAKETİ 15 §3): yeni oturum için `Ready` YAYIMLANMAZ, yeni
//! Crypto oturumu kapatılır, eski oturum `Closed(eski, Superseded)` ile biter — etkin bir ardıl
//! olmadan. Application eşi bağlantısız sayar ve yeniden `connect` eder (`abandon_successor`).
//!
//! Her başarısız deneme [`ClientTransport::endpoint_open_failures`]'ta sayılır.
//! `ConnectAbandoned`'da `early_unit` gönderilmemiştir (S1 K6, §4); Application onu yeni
//! `connect`'ten sonra yeniden verir.
//!
//! [DUR-05g-3 (B), CEVAP PAKETİ 16 §2] `connect` (ve kalıcı niyetin taze INIT'i, `reinitiate`)
//! INIT'ten ÖNCE yerel soketi sınar: ucun açılışındaki AYNI kurulum (`runtime::bind_peer_socket`,
//! bind + connect), uç görevi açılmadan; soket hemen bırakılır. Sınama başarısızsa INIT yola
//! çıkmaz (Crypto'da bekleyen kayıt, karşı uçta kayıt oluşmaz) ve niyet
//! `ConnectAbandoned(_, LocalPermanent)` ile biter — eşin adresi yokken olduğu gibi; sayaç
//! [`ClientTransport::local_probe_failures`]. Sınama YARDIMCI ÖNLEMDİR, kaynak sınırı DEĞİLDİR:
//! sınama ile gerçek açılış arasında koşul değişebilir (o zaman yukarıdaki açılış hatası kolları
//! aynen çalışır; sınama onların yerine geçmez) ve gelen INIT yollarını (yanıtlayan, provisional)
//! kapsamaz. Karşı uçtaki kaynak sınırı ayrı karttır (DUR-05g-3 (A)).
//!
//! # Bağlam-önce dağıtım (K4G-01 / D1 v1.1)
//!
//! Bağlam yükün tipinden türetilmez; bağlam dağıtıcıyı seçer. Yük, bağlam doğrulanmadan çözülmez.
//! Veri alıcısı açılan datagramın yükünü YALNIZ bağlamı bu adaptörün oturum tablosunda beklenen bir
//! `Session(h)` ise ve eş yuvası varsa o yuvanın oturumuysa işler; her oturum ucu yalnız kendi
//! `Session(h)`'sini. Beklenen küme bu tablodan yayımlanır (`sync_expected`); yeni bir tutamaç
//! zarfı (INIT/RESPONSE) yola çıkmadan eklenir. `HubChannel` (direct-mode'da hub yok; PoC'de
//! AEAD'siz no-op) ve başka her bağlam yüke dokunulmadan düşer ve sayılır
//! ([`ClientTransport::ctx_dispatch_stats`]).
//!
//! [K4G-01b] Veri alıcısının eş yuvası `(adres, SessionHandle)` ile anahtarlıdır: aynı adresten
//! yeni bir beklenen oturum YENİ yuva alır (yenileme / supersession / cancel+connect sonrası
//! efemeral port yeniden kullanımı `idle_hold` kadar düşmez); yuvanın oturum bağı veri datagramıyla
//! değişmez; bağlı oturum beklenen kümeden çıkınca yuva emekliye ayrılır (`crate::runtime`).
//!
//! INV-ACK-DELIVER-1: teslim yoluna kabul edilmeyen birim ACK'lenmez. Beklenen kümede her
//! tutamacın hazırlığı da yayımlanır: yalnız teslim kapısının (`drive_deliveries`) kabul ettiği
//! oturumlar `Ready`'dir; bekleyen INIT / yenileme, onaysız provisional ve `Ready` olayı
//! işlenmemiş yanıtlayan tutamaçların datagramları veri alıcısında ACK'siz düşer
//! (`not_ready_drops`), gönderen ARQ ile yeniden yollar.
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
use kama_noise_wire::protector::NoiseWireProtection;

use crate::runtime::{
    bind_peer_socket, ClientEndpoint, ClientHandle, ClientLiveDiag, CtxDispatchCounters,
    CtxDispatchStats, ExpectedSessions, Readiness, RuntimeConfig, RuntimeError, ServerEndpoint,
    ServerHandle,
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
    /// kapanış boşaltma tavanıyla aynı. [DUR-19] Canlı teşhis aynası KAPALI
    /// (`RuntimeConfig::live_diag = false`, ürün profilinden).
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
    /// [K4G-05g] `AcceptResponse`'tan ÖNCE açılan yeni oturum ucu; Crypto'nun `Ready(pending)`'i
    /// işlenince oturuma geçer (`ClientTransport::release_held_renewals`).
    endpoint: Option<OpenedEndpoint>,
}

/// [K4G-05g] Açılmış oturum ucu + (yalnız test) uç dikişiyle paylaşılan ilk-emisyon kaybı kümesi.
/// `activate` yalnız bununla çağrılabilir: açılmamış uçla `Ready` üretilemez.
struct OpenedEndpoint {
    endpoint: ClientHandle,
    #[cfg(any(test, feature = "test-util"))]
    drop_ids: Arc<Mutex<BTreeSet<u64>>>,
}

/// Kaynak uç başına sıralı teslim (S1 §4).
///
/// [K4G-01b] Adaptör bunu OTURUM başına tutar (`ClientTransport::reorder`, `SessionHandle`
/// anahtarlı): karşı ucun bir oturumu tek bir uçtur. `unit < next` ve tutulan birim için ikinci
/// kopya YOK SAYILIR — aynı oturumun farklı eş yuvalarından gelen kopyaları burada tekilleşir.
#[derive(Default)]
struct Reorder {
    next: u64,
    held: BTreeMap<u64, (SessionHandle, Vec<u8>)>,
}

/// [K4G-05 / INT-02 TEŞHİS] Bir oturumun sıralı teslim durumu ([`ClientTransport::reorder_diags`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReorderDiag {
    /// Teslim beklenen sıradaki birim kimliği (bundan küçükler teslim edildi).
    pub next_unit: u64,
    /// `next_unit` gelmediği için tutulan (sırası gelmemiş) birim sayısı.
    pub held_units: u32,
    /// Tutulan en küçük birim kimliği; tutulan yoksa `None`.
    pub first_held_unit: Option<u64>,
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
    /// [K4G-01b] Oturum başına sıralı teslim — anahtar `SessionHandle`, kaynak adres DEĞİL (bkz.
    /// modül belgesi "Teslim sırası"). Girdi yalnız teslim kabul eden (`phase != Closed`) oturum
    /// için açılır ve oturum `Closed` olunca silinir (`close`, `on_crypto_closed`, `Closed`'ın
    /// okunması) → anahtar kümesi `sessions`'ın alt kümesidir.
    reorder: HashMap<SessionHandle, Reorder>,
    /// [K4G-01 / D1 (v1.1)] Veri alıcısının BEKLEDİĞİ oturumlar — bu adaptörün oturum tablosunun
    /// yayını ([`Self::sync_expected`]); yeni tutamaç zarfı (INIT/RESPONSE) yola çıkmadan eklenir.
    expected: ExpectedSessions,
    /// [K4G-01] Düğüm geneli bağlam-önce dağıtım sayaçları (veri alıcısı + bütün oturum uçları).
    ctx_counters: CtxDispatchCounters,
    /// [K4G-05] Salt gözlem: ucu bırakılan (kapanan) oturumların ARQ / kurtarma sayaçlarının
    /// toplamı — ucun bırakıldığı andaki canlı aynasından ([`Self::retired_live_diag`]). Hiçbir
    /// karar okumaz. [DUR-19] Yalnız aynası olan uç eklenir (`RuntimeConfig::live_diag`).
    retired_diag: ClientLiveDiag,
    /// [K4G-05g] Salt gözlem: oturum ucunun kullanılamadığı deneme sayısı
    /// ([`Self::endpoint_open_failures`]). Hiçbir karar okumaz.
    endpoint_open_failures: u64,
    /// [K4G-05g / DUR-05g-3 (B)] Salt gözlem: INIT öncesi yerel soket sınamasının başarısız olduğu
    /// deneme sayısı ([`Self::local_probe_failures`]). Hiçbir karar okumaz.
    local_probe_failures: u64,
    /// Sıradaki `send`'lerden kaçının ilk emisyonu kaybettirilecek (yalnız test).
    #[cfg(any(test, feature = "test-util"))]
    drops: usize,
    /// [K4G-05g] Sıradaki kaç uç açma denemesi soket kurulum hatası gibi başarısız olacak (yalnız
    /// test; [`Self::set_endpoint_open_failures_for_test`]).
    #[cfg(any(test, feature = "test-util"))]
    endpoint_open_fail_next: usize,
    /// [K4G-05g / DUR-05g-3 (B)] Sıradaki kaç yerel soket sınaması başarısız olacak (yalnız test;
    /// [`Self::set_socket_probe_failures_for_test`]).
    #[cfg(any(test, feature = "test-util"))]
    socket_probe_fail_next: usize,
    /// [K4G-05g] Ucu açılamadığı için S1'e açılmayan son tutamaç (yalnız test).
    #[cfg(any(test, feature = "test-util"))]
    last_failed_activation: Option<SessionHandle>,
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
        // [K4G-01 / D1 (v1.1)] Veri alıcısının beklediği oturumlar bu adaptörün oturum tablosundan
        // yayımlanır (`sync_expected`); kurulumda tablo boştur → hiçbir `Session(_)` beklenmez.
        let expected = ExpectedSessions::new();
        let ctx_counters = CtxDispatchCounters::new();
        let server = rt.block_on(ServerEndpoint::bind_expecting(
            cfg.data_bind,
            cfg.runtime.clone(),
            Box::new(CryptoSeam(Arc::clone(&crypto))),
            None,
            expected.clone(),
            ctx_counters.clone(),
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
            expected,
            ctx_counters,
            retired_diag: ClientLiveDiag::default(),
            endpoint_open_failures: 0,
            local_probe_failures: 0,
            #[cfg(any(test, feature = "test-util"))]
            drops: 0,
            #[cfg(any(test, feature = "test-util"))]
            endpoint_open_fail_next: 0,
            #[cfg(any(test, feature = "test-util"))]
            socket_probe_fail_next: 0,
            #[cfg(any(test, feature = "test-util"))]
            last_failed_activation: None,
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

    /// [K4G-01 / D1 (v1.1)] Düğüm geneli bağlam-önce dağıtım sayaçları (telemetri; canlı):
    /// `ctx_mismatch_drops` (beklenmeyen `Session(_)` bağlamı) ve `hub_frames_dropped`
    /// (HubChannel). Veri alıcısı ve bütün oturum uçları aynı kümeyi paylaşır; uç kapansa da
    /// sayım kaybolmaz.
    #[must_use]
    pub fn ctx_dispatch_stats(&self) -> CtxDispatchStats {
        self.ctx_counters.snapshot()
    }

    /// [K4G-11] Salt-okunur telemetri: veri alıcısının teslim kanalı doluyken düşen teslim sayısı
    /// (`ServerStats::delivery_dropped`; bkz. [`ServerHandle::delivery_dropped`]). Adaptör teslim
    /// kanalını her `drive`'da boşalttığı için uygulama `recv`'i sürdükçe sıfır kalır; sıfırdan
    /// büyükse ACK'lenmiş birim uygulamaya ulaşmadan düşmüştür (app drenaj sözleşmesi ihlali).
    #[must_use]
    pub fn delivery_dropped(&self) -> u64 {
        self.server.delivery_dropped()
    }

    /// [K4G-05g] Salt-okunur telemetri: bir oturumun veri ucunun kullanılamadığı deneme sayısı —
    /// uç açılamadı (eşin veri adresi yok ya da soket kurulamadı) ya da taze uç erken birimi kabul
    /// etmedi. Her deneme bir kez sayılır; yenileme cutover'ı uç açılana dek her `drive`'da yeniden
    /// dener (bkz. modül belgesi "Uç açılmadan `Ready` yok"). Durum değişmez.
    #[must_use]
    pub fn endpoint_open_failures(&self) -> u64 {
        self.endpoint_open_failures
    }

    /// [K4G-05g / DUR-05g-3 (B)] Salt-okunur telemetri: `connect` (ya da kalıcı niyetin taze
    /// INIT'i) öncesi yerel soket sınamasının başarısız olduğu ve bu yüzden INIT'in yola ÇIKMADIĞI
    /// deneme sayısı. Durum değişmez.
    #[must_use]
    pub fn local_probe_failures(&self) -> u64 {
        self.local_probe_failures
    }

    /// [K4G-05] Salt-okunur telemetri: ucu açık her S1 oturumunun motor / ARQ aynası
    /// ([`ClientHandle::live_diag`]), tutamaç sırasıyla. Ucu kurulamamış ya da bırakılmış
    /// oturum listede yoktur (onun sayaçları [`Self::retired_live_diag`]'dadır). Durum değişmez.
    ///
    /// [DUR-19] Canlı ayna `RuntimeConfig::live_diag` açıkken vardır (ürün varsayılanı KAPALI;
    /// [`ClientTransportConfig::product`]). Kapalıyken liste yine ucu açık her oturumu içerir ama
    /// her değer [`ClientLiveDiag::default`]'tur (sıfır sayaçlar; "trafik yok" ANLAMINA GELMEZ).
    #[must_use]
    pub fn session_live_diags(&self) -> Vec<(SessionHandle, ClientLiveDiag)> {
        self.sessions
            .iter()
            .filter_map(|(h, s)| s.endpoint.as_ref().map(|e| (*h, e.live_diag())))
            .collect()
    }

    /// [K4G-05] Salt-okunur telemetri: ucu bırakılan (kapanan) oturumların ARQ / kurtarma
    /// SAYAÇLARININ toplamı (RTT alanları 0). Değer ucun bırakıldığı andaki aynadır: ucun kapanış
    /// boşaltmasında verdiği kararlar girmez (alt sınır). [DUR-19] Yalnız aynası olan uçlar
    /// eklenir; canlı ayna kapalıyken (ürün varsayılanı) kapanışta kopya alınmaz ve değer HER ZAMAN
    /// [`ClientLiveDiag::default`]'tur.
    #[must_use]
    pub fn retired_live_diag(&self) -> ClientLiveDiag {
        self.retired_diag
    }

    /// [K4G-05 / INT-02 TEŞHİS] Salt-okunur: alıcı tarafta oturum başına sıralı teslimin (`Reorder`,
    /// S1 §4) durumu, tutamaç sırasıyla — teslim beklenen sıradaki birim (boşluk oradadır), onun
    /// arkasında tutulan birim sayısı ve tutulan en küçük birim. Durum değişmez; teslim yolu bu
    /// değerleri okumaz.
    #[must_use]
    pub fn reorder_diags(&self) -> Vec<(SessionHandle, ReorderDiag)> {
        let mut out: Vec<(SessionHandle, ReorderDiag)> = self
            .reorder
            .iter()
            .map(|(h, r)| {
                (
                    *h,
                    ReorderDiag {
                        next_unit: r.next,
                        held_units: u32::try_from(r.held.len()).unwrap_or(u32::MAX),
                        first_held_unit: r.held.keys().next().copied(),
                    },
                )
            })
            .collect();
        out.sort_by_key(|(h, _)| *h);
        out
    }

    /// Bekleyen işi sürer: zarf olayları → tutulan yenileme yanıtları → Crypto olayları →
    /// teslimler. Her S1 çağrısı bunu kendisi çağırır; uygulama döngüsü de doğrudan çağırabilir.
    pub fn drive(&mut self) {
        self.drive_link();
        self.release_held_renewals();
        self.drive_crypto_events();
        self.drive_deliveries();
        self.sync_expected();
    }

    /// [K4G-01 / D1 (v1.1) + INV-ACK-DELIVER-1] Veri alıcısının BEKLEDİĞİ oturum kümesini ve her
    /// tutamacın teslim hazırlığını oturum tablosundan yeniden yayımlar. "Beklenen" YALNIZ bu
    /// tablodur; ilk gelen datagramdan öğrenme (TOFU) yoktur. Yeni bir tutamaç ayrıca, zarfı
    /// (INIT/RESPONSE) yola çıkmadan ÖNCE `NotReady` eklenir: karşı uç o zarfı almadan bu oturumda
    /// veri yollayamaz, yani tablo ile veri yolu arasında yarış kalmaz.
    ///
    /// | Kategori | Hazırlık | `drive_deliveries` kabulü |
    /// |---|---|---|
    /// | `intents[].pending`, `renewals[].pending` | `NotReady` | hayır (S1 oturumu değil) |
    /// | `provisional`, `responder_peer` | `NotReady` | ancak `Confirmed` / `Ready` işlenince |
    /// | `sessions`, `Active` / `Superseded` | `Ready` | evet |
    /// | `sessions`, `Closed` | yayımlanmaz | hayır |
    ///
    /// `Ready` kümesi teslim kapısının kabul koşuluyla (`phase != Closed`) AYNIDIR. Bir tutamaç iki
    /// kategoride birden bulunursa (ör. yinelenen INIT'in yanıtlayan kaydı) `Ready` sona eklendiği
    /// için o geçerlidir.
    fn sync_expected(&self) {
        let not_ready = self
            .intents
            .iter()
            .map(|it| it.pending)
            .chain(self.renewals.iter().map(|r| r.pending))
            .chain(self.provisional.keys().copied())
            .chain(self.responder_peer.keys().copied())
            .map(|h| (h, Readiness::NotReady));
        let ready = self
            .sessions
            .iter()
            .filter(|(_, s)| s.phase != Phase::Closed)
            .map(|(h, _)| (*h, Readiness::Ready));
        self.expected.replace(not_ready.chain(ready));
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
                // [K4G-01] RESPONSE'tan ÖNCE beklenir: başlatan ancak onu alınca veri yollar.
                self.expected.insert_pending(session);
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
                // [K4G-01] Onayı getirecek ilk epoch-1 paketi bu oturumun bağlamında açılır:
                // RESPONSE'tan ÖNCE beklenir.
                self.expected.insert_pending(new);
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
            let Some(since) = self.renewals[i].held.as_ref().map(|(_, since)| *since) else {
                continue;
            };
            let old = self.renewals[i].old;
            let drained = self
                .sessions
                .get(&old)
                .and_then(|s| s.endpoint.as_ref())
                .is_none_or(|e| e.outstanding() == 0);
            if !drained && now.saturating_duration_since(since) < self.renew_drain_cap {
                continue;
            }
            // [K4G-05g] Yeni oturumun ucu `AcceptResponse`'tan ÖNCE açılır: Crypto `Established`'de
            // eski oturumu yalnız-alım drain'e alır ve bu geri alınamaz. Uç açılamazsa yanıt
            // TUTULUR (Crypto'ya verilmez): eski oturum Crypto'da da tam çalışmaya devam eder,
            // Application hiçbir olay görmez, cutover sonraki `drive`'da yeniden denenir.
            let (pending, peer) = (self.renewals[i].pending, self.renewals[i].peer);
            let Some(opened) = self.open_endpoint(pending, peer) else {
                continue;
            };
            if let Some((wire, _)) = self.renewals[i].held.take() {
                // Established → Crypto `Ready(pending)` → `on_crypto_ready` eşler ve bu ucu
                // kullanır. Failed (yanlış aday) → yenileme sürer, yeni bir yanıt beklenir; uç
                // bırakılır (bir sonraki yanıtta yeniden açılır).
                let established = self.command(SessionCommand::AcceptResponse { wire })
                    == (SessionResult::Established { session: pending });
                if established {
                    self.renewals[i].endpoint = Some(opened);
                }
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
        // (a) Kendi yenilemem tamamlandı: atomik cutover. [K4G-05g] Uç `AcceptResponse`'tan ÖNCE
        // açıldı (`release_held_renewals`); yanıt Crypto'da bu yenilemeyle başka bir yoldan
        // eşleştiyse (önceden açılmış uç yok) burada açılır.
        if let Some(i) = self.renewals.iter().position(|r| r.pending == h) {
            let mut r = self.renewals.remove(i);
            self.hub_of.remove(&h);
            match r.endpoint.take().or_else(|| self.open_endpoint(h, r.peer)) {
                Some(opened) => {
                    self.supersede(r.old);
                    // Erken birim yok → başarısız olamaz.
                    let _ = self.activate(h, r.peer, None, opened);
                }
                // Yedek kol (DUR-05g-1 (a)): Crypto eski oturumu ZATEN drain'e aldı.
                None => self.abandon_successor(h, r.old),
            }
            return;
        }
        // (b) Başlattığım niyet çözüldü. [K4G-05g] Uç açılamazsa niyet kapalı nedenle biter:
        // Application'ın elinde yalnız bilet vardır; `early_unit` gönderilmemiştir (S1 K6, §4).
        if let Some(i) = self.intents.iter().position(|it| it.pending == h) {
            let it = self.intents.remove(i);
            self.hub_of.remove(&h);
            if !self.open_and_activate(h, it.peer, it.early) {
                self.abort_activation(h);
                self.events.push_back(SessionEvent::ConnectAbandoned(
                    it.ticket,
                    AbandonReason::LocalPermanent,
                ));
            }
            return;
        }
        // (c) Yanıtlayan rolde kurulan oturum. Glare'i kaybettiysek Crypto bu eşe bekleyen
        // INIT'imizi sildi; niyetimiz bu oturumla çözülür.
        if let Some(peer) = self.responder_peer.remove(&h) {
            self.hub_of.remove(&h);
            let glare = self.intents.iter().position(|it| it.peer == peer).map(|i| {
                let it = self.intents.remove(i);
                self.attempts.retain(|_, p| *p != it.pending);
                self.hub_of.remove(&it.pending);
                it
            });
            let (lost, early) = match glare {
                Some(it) => (Some((it.ticket, it.pending)), it.early),
                None => (None, None),
            };
            if !self.open_and_activate(h, peer, early) {
                // [K4G-05g] Niyet yoksa Application bu tutamacı hiç görmedi: S1 olayı YOK (K2:
                // tutamaç yalnız `Ready` ile verilir; K7: oturumun olay sırası `Ready` ile başlar),
                // yalnız sayaç. Glare kaybında bilet bilinir → `ConnectAbandoned`; `early_unit`
                // gönderilmedi.
                self.abort_activation(h);
                if let Some((ticket, pending)) = lost {
                    // Crypto glare kaybında bekleyeni zaten sildi; `Close` bilinmeyen tutamaçta
                    // olay üretmez (s3 v1.1) — zombi bekleyen kalmadığının güvencesi.
                    let _ = self.command(SessionCommand::Close { session: pending });
                    self.events.push_back(SessionEvent::ConnectAbandoned(
                        ticket,
                        AbandonReason::LocalPermanent,
                    ));
                }
            }
        }
    }

    fn on_confirmed(&mut self, new: SessionHandle) {
        let Some((peer, old)) = self.provisional.remove(&new) else {
            return;
        };
        self.hub_of.remove(&new);
        match self.open_endpoint(new, peer) {
            Some(opened) => {
                self.supersede(old);
                // Erken birim yok → başarısız olamaz.
                let _ = self.activate(new, peer, None, opened);
            }
            // DUR-05g-1 (a): onay Crypto'nun açma yolunda verildi ve eski oturum ZATEN drain'de.
            None => self.abandon_successor(new, old),
        }
    }

    /// Yerine geçen oturum S1'e açılırken eski oturum `send` kabul etmeyi bırakır (`Ready(h')` ile
    /// `Closed(h)`'in okunması arası `SessionSuperseded`; S1 F0.2(i)).
    fn supersede(&mut self, old: SessionHandle) {
        if let Some(s) = self.sessions.get_mut(&old) {
            if s.phase == Phase::Active {
                s.phase = Phase::Superseded;
            }
        }
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
            // [K4G-01] INIT'ten ÖNCE beklenir (tablo ile veri yolu arasında yarış yok).
            self.expected.insert_pending(pending);
            if let Ok(a) = self.link.send_init(peer, envelope_type, bytes) {
                self.attempts.insert(a, pending);
            }
            self.renewals.push(Renewal {
                old: h,
                peer,
                pending,
                held: None,
                endpoint: None,
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
        let closed = s.phase == Phase::Closed;
        // [K4G-05] Salt gözlem: uç bırakılmadan önce aynası kapanmış oturumlar toplamına eklenir.
        // [DUR-19] Yalnız aynası olan uçta; canlı ayna kapalıyken kopya alınmaz (kilit yok). Uç her
        // durumda burada bırakılır.
        if let Some(endpoint) = s.endpoint.take() {
            if endpoint.live_diag_enabled() {
                self.retired_diag.add_counters(&endpoint.live_diag());
            }
        }
        // [K4G-01b] Teslim kabulü bitti (`drive_deliveries` `Closed`'u reddeder) → sıralama durumu
        // silinir; içinde tutulan birim artık hiçbir yoldan teslim edilemezdi. Supersede edilmiş
        // oturum `Closed` okunana dek teslim kabul eder (yalnız-alım drain) — onunki okunurken
        // (`poll_event`) silinir.
        if closed {
            self.reorder.remove(&h);
        }
        self.events.push_back(SessionEvent::Closed(h, reason));
        if let Some(i) = self.renewals.iter().position(|r| r.old == h) {
            let r = self.renewals.remove(i);
            let _ = self.command(SessionCommand::Close { session: r.pending });
        }
    }

    fn reinitiate(&mut self, i: usize) {
        let peer = self.intents[i].peer;
        // [K4G-05g / DUR-05g-3 (B)] Taze INIT'ten ÖNCE yerel soket sınaması (bkz. modül belgesi).
        if !self.local_socket_ok(peer) {
            let it = self.intents.remove(i);
            self.events.push_back(SessionEvent::ConnectAbandoned(
                it.ticket,
                AbandonReason::LocalPermanent,
            ));
            return;
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
                self.intents[i].pending = session;
                // [K4G-01] INIT'ten ÖNCE beklenir (tablo ile veri yolu arasında yarış yok).
                self.expected.insert_pending(session);
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

    /// Oturumu S1'e açar: açılmış uç + `Ready(h, peer)` + (varsa) `early_unit` ilk gönderim.
    ///
    /// [K4G-05g] Uç PARAMETREDİR: açılmamış uçla `Ready` üretilemez. Taze uç erken birimi kabul
    /// etmezse (kuyruk taze uçta boştur; ret yalnız ucun görevi bitmişse olur) oturum S1'e AÇILMAZ:
    /// `Ready` yok, uç bırakılır, [`Self::endpoint_open_failures`] artar ve `false` döner — olayı
    /// ve Crypto oturumunu çağıran yol belirler. Erken birim yoksa başarısız olamaz.
    fn activate(
        &mut self,
        h: SessionHandle,
        peer: PeerRef,
        early: Option<Vec<u8>>,
        opened: OpenedEndpoint,
    ) -> bool {
        let endpoint = opened.endpoint;
        let mut sent = 0;
        if let Some(bytes) = early {
            if endpoint.send(bytes).is_err() {
                self.endpoint_open_failures = self.endpoint_open_failures.saturating_add(1);
                return false;
            }
            sent = 1;
        }
        self.sessions.insert(
            h,
            Sess {
                peer,
                phase: Phase::Active,
                endpoint: Some(endpoint),
                sent,
                #[cfg(any(test, feature = "test-util"))]
                drop_ids: opened.drop_ids,
            },
        );
        self.events.push_back(SessionEvent::Ready(h, peer));
        true
    }

    /// [K4G-05g] Ucu açar ve oturumu S1'e açar; ikisinden biri olmazsa `false` (`Ready` yok).
    fn open_and_activate(
        &mut self,
        h: SessionHandle,
        peer: PeerRef,
        early: Option<Vec<u8>>,
    ) -> bool {
        match self.open_endpoint(h, peer) {
            Some(opened) => self.activate(h, peer, early, opened),
            None => false,
        }
    }

    /// [K4G-05g] Ucu açılamayan oturum S1'e AÇILMADI: Crypto oturumu kapatılır (zombi kripto
    /// oturumu kalmaz). Crypto'nun bu kapanış için yayınlayacağı `Closed(h, UserClosed)`
    /// `on_crypto_closed`'da hiçbir tabloda bulunmaz ve sessizce düşer (adım 5: S1'in bilmediği
    /// tutamaç). `h` hiçbir tabloda olmadığı için bir sonraki `sync_expected` onu beklenen kümeden
    /// çıkarır.
    fn abort_activation(&mut self, h: SessionHandle) {
        let _ = self.command(SessionCommand::Close { session: h });
        #[cfg(any(test, feature = "test-util"))]
        {
            self.last_failed_activation = Some(h);
        }
    }

    /// [K4G-05g / DUR-05g-1 (a) — S1 K7'nin TEK istisnası, CEVAP PAKETİ 15 §3] Yerine geçen
    /// oturumun (provisional onayı ya da yenileme cutover'ının yedek kolu) yerel ucu açılamadı ve
    /// Crypto eski oturumu ZATEN yalnız-alım drain'e aldı (`Confirmed` / `Established`; geri
    /// alınamaz, eski oturumun gönderimi Crypto'da kapandı). Yeni oturum için `Ready` YAYIMLANMAZ,
    /// yeni Crypto oturumu kapatılır (`abort_activation`); eski oturum `Closed(eski, Superseded)`
    /// ile biter — etkin bir ardıl olmadan. Kapanış `close` ile aynı yoldur: uç bırakılır, sıralama
    /// durumu silinir, eski oturumun bekleyen yenilemesi kapatılır ve Crypto'nun eski oturum için
    /// vereceği TEK `Closed` (drain sonu ya da buradaki `Close`) yutulur — S1'e tam BİR `Closed`.
    /// Eski oturum zaten `Active` değilse (başka bir ardıl `Ready` oldu ya da `Closed` kuyrukta)
    /// yalnız yeni oturum kapatılır.
    fn abandon_successor(&mut self, new: SessionHandle, old: SessionHandle) {
        self.abort_activation(new);
        let Some(s) = self.sessions.get_mut(&old) else {
            return;
        };
        if s.phase != Phase::Active {
            return;
        }
        s.phase = Phase::Closed;
        // [K4G-05] Salt gözlem: uç bırakılmadan önce aynası kapanmış oturumlar toplamına eklenir.
        if let Some(endpoint) = s.endpoint.take() {
            if endpoint.live_diag_enabled() {
                self.retired_diag.add_counters(&endpoint.live_diag());
            }
        }
        self.reorder.remove(&old);
        self.locally_closed.insert(old);
        let _ = self.command(SessionCommand::Close { session: old });
        if let Some(i) = self.renewals.iter().position(|r| r.old == old) {
            let r = self.renewals.remove(i);
            let _ = self.command(SessionCommand::Close { session: r.pending });
        }
        self.events
            .push_back(SessionEvent::Closed(old, CloseReason::Superseded));
    }

    /// [K4G-05g / DUR-05g-3 (B)] INIT öncesi yerel soket sınaması: ucun açılışındaki AYNI soket
    /// kurulumu (`runtime::bind_peer_socket`: bind + connect), uç görevi açılmadan; soket sınamadan
    /// hemen sonra bırakılır. Eşin adresi yoksa ya da kurulum başarısızsa `false` ve
    /// [`Self::local_probe_failures`] artar. Yardımcı önlemdir, kaynak sınırı DEĞİLDİR (sınama ile
    /// açılış arasında koşul değişebilir; gelen INIT yollarını kapsamaz).
    fn local_socket_ok(&mut self, peer: PeerRef) -> bool {
        let ok = self.try_local_socket(peer);
        if !ok {
            self.local_probe_failures = self.local_probe_failures.saturating_add(1);
        }
        ok
    }

    fn try_local_socket(&mut self, peer: PeerRef) -> bool {
        let Some(&addr) = self.peer_addrs.get(&peer) else {
            return false;
        };
        // (yalnız test) Kalıcı yerel soket hatasının belirlenimli karşılığı; ürün derlemesinde
        // yoktur.
        #[cfg(any(test, feature = "test-util"))]
        if self.socket_probe_fail_next > 0 {
            self.socket_probe_fail_next -= 1;
            return false;
        }
        // Soket çalışma-zamanının içinde açılır ve bırakılır (reaktör kaydı orada silinir).
        self.rt
            .block_on(async move { bind_peer_socket(addr).await.map(drop) })
            .is_ok()
    }

    /// Oturumun veri ucunu açar. Açılamazsa (`peer_addrs`'ta eşin adresi yok ya da
    /// `ClientEndpoint::connect_session_counted` hatası) `None` ve
    /// [`Self::endpoint_open_failures`] artar.
    fn open_endpoint(&mut self, h: SessionHandle, peer: PeerRef) -> Option<OpenedEndpoint> {
        let opened = self.try_open_endpoint(h, peer);
        if opened.is_none() {
            self.endpoint_open_failures = self.endpoint_open_failures.saturating_add(1);
        }
        opened
    }

    fn try_open_endpoint(&mut self, h: SessionHandle, peer: PeerRef) -> Option<OpenedEndpoint> {
        let addr = *self.peer_addrs.get(&peer)?;
        // (yalnız test) Soket kurulum hatasının belirlenimli karşılığı; ürün derlemesinde yoktur.
        #[cfg(any(test, feature = "test-util"))]
        if self.endpoint_open_fail_next > 0 {
            self.endpoint_open_fail_next -= 1;
            return None;
        }
        #[cfg(any(test, feature = "test-util"))]
        let drop_ids = Arc::new(Mutex::new(BTreeSet::new()));
        let seam = EndpointSeam {
            crypto: Arc::clone(&self.crypto),
            #[cfg(any(test, feature = "test-util"))]
            drop_ids: Arc::clone(&drop_ids),
            #[cfg(any(test, feature = "test-util"))]
            max_unit_sealed: None,
        };
        // [K4G-01 / D1 (v1.1)] Ucun beklediği bağlam `Session(h)`'dir; sayaçlar düğüm geneli.
        let endpoint = self
            .rt
            .block_on(ClientEndpoint::connect_session_counted(
                addr,
                self.runtime_cfg.clone(),
                Box::new(seam),
                None,
                ProtectionContext::Session(h),
                self.ctx_counters.clone(),
            ))
            .ok()?;
        endpoint.set_recovery(self.recovery_on);
        Some(OpenedEndpoint {
            endpoint,
            #[cfg(any(test, feature = "test-util"))]
            drop_ids,
        })
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
            // [K4G-01 / INV-ACK-DELIVER-1] Bu kabul koşulu `sync_expected`'in `Ready` kümesiyle
            // AYNIDIR: veri alıcısı yalnız o kümedeki oturumun birimini ACK'ler. Biri değişirse
            // öteki de değişmeli — aksi hâlde ACK'lenmiş birim burada kaybolur.
            match self.sessions.get(&h) {
                Some(s) if s.phase != Phase::Closed => {}
                // Kapanmış / bilinmeyen oturumun teslimi düşer.
                _ => continue,
            }
            // [K4G-01b] Sıra OTURUMUN (`h`), kaynak adresin (`d.peer`) değil: aynı adresten gelen
            // yeni oturum eski oturumun `next`'ine takılmaz; aynı oturumun başka bir yuvadan
            // (adres) gelen kopyası burada tekilleşir.
            self.reorder
                .entry(h)
                .or_default()
                .push(d.unit_id, h, d.payload, cap, &mut self.inbox);
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

    /// [K4G-05 / DUR-19] Canlı teşhis aynası (`RuntimeConfig::live_diag`) — YALNIZ ölçüm düzeneği
    /// (INT-02; `kama-node` `test-util` kurucuları). Sonradan açılan uçlara uygulanır; açık uçlar
    /// değişmez (ayna uç kurulurken kurulur ya da kurulmaz). Ürün yolu bu kancayı çağırmaz:
    /// [`ClientTransportConfig::product`] kapalı kurar.
    #[cfg(any(test, feature = "test-util"))]
    pub fn set_live_diag_for_test(&mut self, on: bool) {
        self.runtime_cfg.live_diag = on;
    }

    /// Sonraki `n` `send`'in İLK emisyonu kaybettirilir (tel baytı bozulur; S1 kancası). Sıra
    /// `send` çağrı sırasıdır — eşzamanlı uç görevlerinin mühür sırası değil.
    #[cfg(any(test, feature = "test-util"))]
    pub fn drop_next_units_for_test(&mut self, n: usize) {
        self.drops = self.drops.saturating_add(n);
    }

    /// [K4G-05g] Sonraki `n` oturum ucu açma denemesi, `ClientEndpoint::connect_session_counted`'ın
    /// `Err`'i gibi başarısız olur (eşin adres denetiminden SONRA). `n` ATANIR (eklenmez): `0`
    /// kancayı kapatır. Ürün derlemesinde yoktur; ürün yolu bu alanı okumaz.
    #[cfg(any(test, feature = "test-util"))]
    pub fn set_endpoint_open_failures_for_test(&mut self, n: usize) {
        self.endpoint_open_fail_next = n;
    }

    /// [K4G-05g / DUR-05g-3 (B)] Sonraki `n` INIT öncesi yerel soket sınaması kalıcı yerel soket
    /// hatası gibi başarısız olur (eşin adres denetiminden SONRA). Uç açılışını ETKİLEMEZ (o
    /// [`Self::set_endpoint_open_failures_for_test`]'tir): "sınama geçti, açılış düştü" kolu ayrı
    /// sınanır. `n` ATANIR (eklenmez); `0` kancayı kapatır. Ürün derlemesinde yoktur.
    #[cfg(any(test, feature = "test-util"))]
    pub fn set_socket_probe_failures_for_test(&mut self, n: usize) {
        self.socket_probe_fail_next = n;
    }

    /// [K4G-05g] Ucu açılamadığı için S1'e açılmayan ve Crypto oturumu kapatılan son tutamaç
    /// (Application bu tutamacı hiç görmez; fikstür Crypto'da zombi kalmadığını onunla sınar).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn last_failed_activation_for_test(&self) -> Option<SessionHandle> {
        self.last_failed_activation
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

    /// [K4G-01b] Sıralama durumu tutulan oturumlar (sıralı). Kapanan oturumunki silinir.
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn reorder_sessions_for_test(&self) -> Vec<SessionHandle> {
        let mut out: Vec<SessionHandle> = self.reorder.keys().copied().collect();
        out.sort();
        out
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

/// [K4G-11] Ürün kriptosuna özgü salt-okunur telemetri. Genel adaptör Crypto'yu yalnız S3
/// trait'leriyle görür ve S3 (FROZEN) epoch sorgusu taşımaz; bu blok yalnız ürün kurulumu
/// (`NoiseWireProtection`) için vardır ve hiçbir durumu değiştirmez.
impl ClientTransport<NoiseWireProtection> {
    /// Oturumun gönderim DH epoch'u (noise-wire §4-B3): `0` = INIT anahtarları (per-INIT
    /// ephemeral confidentiality), `1` = yanıtlayanın efemerali katıldıktan sonraki anahtarlar.
    /// Crypto'nun kurulu oturum tablosundan okunur; bekleyen INIT ya da bilinmeyen tutamaç →
    /// `None`.
    #[must_use]
    pub fn session_epoch(&self, session: SessionHandle) -> Option<u8> {
        lock(&self.crypto).session_send_epoch(session)
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
        // [K4G-05g / DUR-05g-3 (B)] INIT'ten ÖNCE yerel soket sınaması: kalıcı yerel hatada INIT
        // yola çıkmaz (Crypto `Initiate` çağrılmaz, karşı uçta kayıt oluşmaz). Yardımcı önlemdir;
        // açılış hatası kolu (`on_crypto_ready` (b)) aynen yerinde (bkz. modül belgesi).
        if !self.local_socket_ok(peer) {
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
                // [K4G-01] INIT'ten ÖNCE beklenir (tablo ile veri yolu arasında yarış yok).
                self.expected.insert_pending(session);
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
            // [K4G-01b] Bilinmeyen oturumun teslimi düşer → sıralama durumu da silinir (anahtar
            // kümesi `sessions`'ın alt kümesi kalır).
            self.reorder.remove(&h);
            self.sync_expected();
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
        // [K4G-05] Salt gözlem: uç bırakılmadan önce aynası kapanmış oturumlar toplamına eklenir.
        // [DUR-19] Yalnız aynası olan uçta; canlı ayna kapalıyken kopya alınmaz (kilit yok). Uç her
        // durumda burada bırakılır.
        if let Some(endpoint) = s.endpoint.take() {
            if endpoint.live_diag_enabled() {
                self.retired_diag.add_counters(&endpoint.live_diag());
            }
        }
        self.locally_closed.insert(session);
        // [K4G-01b] `Closed` oturumun teslimi `drive_deliveries`'te düşer → sıralama durumu
        // silinir.
        self.reorder.remove(&session);
        let _ = self.command(SessionCommand::Close { session });
        if let Some(i) = self.renewals.iter().position(|r| r.old == session) {
            let r = self.renewals.remove(i);
            let _ = self.command(SessionCommand::Close { session: r.pending });
        }
        self.sync_expected();
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
        self.sync_expected();
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

    /// [K4G-11] Salt-okunur erişimciler taze ürün adaptöründe: Crypto'nun tutmadığı tutamacın
    /// epoch'u yok, teslim kanalı taşması yok. Kurulu oturumun epoch'u (`Some(1)`, iki uçta) ve
    /// canlı taşma değeri uçtan uca `crates/kama-node/tests/two_nodes_loopback.rs` ile
    /// `runtime::delivery_dropped_mirror_tests`'te sınanır.
    #[test]
    fn read_only_accessors_on_a_fresh_product_adapter() {
        use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
        use kama_noise_wire::session_control::EnvelopeKind;

        let me = PeerRef::from_dest_hash([0x4B; 32]);
        let directory = fixture_directory([*me.as_bytes()]).expect("fikstür dizini");
        let (crypto, _clock) = fixture_protection(*me.as_bytes(), directory, EnvelopeKind::Legacy);
        let hub = crate::memory_link::MemoryHub::new();
        let cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
        // [DUR-19] Ürün yapılandırmasında canlı teşhis aynası KAPALI.
        assert!(!cfg.runtime.live_diag, "ürün: canlı ayna kapalı");
        let transport =
            ClientTransport::new(cfg, crypto, Box::new(hub.link(me))).expect("ürün adaptörü");
        assert_eq!(transport.session_epoch(SessionHandle::from_raw(7)), None);
        assert_eq!(transport.delivery_dropped(), 0);
        // [K4G-05] Oturum yok → canlı ayna listesi boş, kapanmış oturum toplamı sıfır.
        assert!(transport.session_live_diags().is_empty());
        assert_eq!(transport.retired_live_diag(), ClientLiveDiag::default());
        assert!(transport.reorder_diags().is_empty());
    }
}
