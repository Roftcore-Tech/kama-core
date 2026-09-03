//! P1 — transport çalışma-zamanı kabuğu (PRODUCT-RUNTIME item 2/3/7 + 8/9 kapıları).
//!
//! Tasarım otoritesi `docs/design/P1-RUNTIME-SHELL.md` (D1-D10). Döngü iskeleti kampanya-kanıtlı
//! bench şeklidir (tek görev `select!`, PACE 1 ms → `Timer::Pto` pompası, RTPROP 200 ms, alıcıda
//! ACK 20 ms; taraf başına TEK `Instant`, motor saf u64-ns olay-güdümlü — time-model kuralı).
//! `next_wakeup_ns()`'e tek-kaynak-uyku YOK (D3, EK4-A2 tarihi): pompa idempotent + kanıtlı.
//!
//! SoC: motor yalnız `OpaqueUnit{id,size}` / `RxDatagram` metadata görür; datagram'lar
//! [`crate::seam::SecuritySeam`]'den mühürlenir/açılır (kripto AYRI düzlem). INV-RATE-1: gönderim
//! hatası = kanal kaybı; motor durumu emisyonu ZATEN işledi, geri itilmez, rate'e dokunulmaz.
//!
//! SINIRLAR (SR-AUDIT-001, bounds-before-alloc; motor `tx_queue`'yu sınırlamaz — kabuk sınırlar):
//! offer-kanalı bounded (typed `QueueFull`) · unit-store bounded (doluyken offer alınmaz — doğal
//! backpressure) · server peer tablosu cap'li fail-closed (yeni-peer typed-drop + sayaç; aktif peer
//! ASLA sessiz-evict edilmez, boşalma idle-emeklilikle) · teslim kanalı bounded (`try_send`,
//! taşan teslim sayaçla raporlanır — app drenaj sözleşmesi).

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, FecActuationMode, OpaqueUnit, RecoveredInfo, RecoveredUnit, SrLogicalRef,
    Timer, TransmitKind,
};
use adaptive_transport::seq::resolve_seq;
use kama_engine::{plan_param_bootstrap, ParamState};
use kama_fec_codec::{
    by_id, FecCodec, FecProfile, ReceivedSymbol, RepairSymbol, RsSimdCodec, SourceSymbol,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::param::ParamFields;
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{inner_kind, SOURCE_PREFIX_LEN};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::seam::{SealArgs, SeamPeer, SecuritySeam};
use crate::wire::{
    decode_data, decode_fec, decode_param, decode_param_ack, decode_recovered, encode_data,
    encode_fec, encode_param, encode_param_ack, encode_recovered, FecFrameHeader, FRAME_PARAM_ACK,
    FRAME_RECOVERED, HDR_DATA, HDR_FEC,
};
use crate::ClientSession;

/// Pacing pompası (bench PACE_TICK): her tick §13 token drain + §5 zaman-eşiği loss; idempotent,
/// yüksek frekans güvenli. EK4-A2 dersi gereği ESAS uyandırma kaynağı budur (D3).
const PACE_TICK: Duration = Duration::from_millis(1);
/// RTprop probe periyodu (§8.3) — `next_wakeup_ns` kaynağı DEĞİL, kaba tick şart.
const RTPROP_TICK: Duration = Duration::from_millis(200);
/// Alıcı ACK timer'ı (§2.1 ACK_TIMER_T) — ACK_FREQ_N'e ulaşmayan son DATA'yı da teyit eder.
const ACK_TICK: Duration = Duration::from_millis(20);
/// Server bakım tick'i: idle-peer emekliliği (item 3 yaşam-döngüsü).
const PATIENCE_TICK: Duration = Duration::from_secs(1);
/// Client recv tamponu (yalnız ACK yönü) / server recv tamponu (max UDP datagram).
const CLIENT_RECV_BUF: usize = 2048;
const SERVER_RECV_BUF: usize = 65536;

/// Çalışma-zamanı konfigürasyonu. Tüm sınırlar açık; `Default` üretim-makul değerler.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Aynı anda outstanding (teslim-bekleyen) birim tavanı — unit-store sınırı.
    pub max_inflight_units: usize,
    /// App→transport offer kuyruğu (bounded mpsc).
    pub offer_queue: usize,
    /// Transport→app teslim kuyruğu (bounded mpsc; taşan teslim sayılır).
    pub delivery_queue: usize,
    /// Birim payload tavanı (bounds-before-alloc: büyük payload typed reddedilir).
    pub max_unit_bytes: usize,
    /// Server eş-tablosu tavanı (fail-closed; SESSION_COUNT_CAP=64 hizası).
    pub max_peers: usize,
    /// Alıcı R-kotası (sessizlik re-ACK bütçesi).
    pub quota_r: u32,
    /// Shutdown drain süre-tavanı (motor birim terk etmez → drain CAP'siz sonlanmayabilir, D8).
    pub drain_cap: Duration,
    /// Server idle-peer emeklilik eşiği (item 3; SESSION_IDLE_HOLD hizası 105 s).
    pub idle_hold: Duration,
    /// [S1] FEC-runtime. `None` = KAPALI (sevk default'u; flip S1-GATE kanıtına bağlı — R1
    /// disiplininin FEC karşılığı). `Some` = Controller-mode + yapı-gate (Q18-fix) + blok
    /// yaşam-döngüsü + repair emisyonu/kurtarımı. İKİ UÇTA AYNI kurulmalı (blok/esi ataması
    /// deterministik `unit_id`'den; PARAM-bootstrap bağlaması S2/B-hattı `kama_engine::param_state`).
    pub fec: Option<FecRuntimeConfig>,
}

/// [S1] FEC-runtime parametreleri.
#[derive(Debug, Clone, Copy)]
pub struct FecRuntimeConfig {
    /// `kama_fec_codec::by_id` profil kimliği (k_shape/r_code_max/e buradan; S1: tam-blok
    /// k_actual = k_shape, known_zero = 0).
    pub profile_id: u8,
    /// `Some(r)` = STATİK-r modu (`Manual{enabled}` otorite + `close_fec_block_manual`):
    /// her blokta sabit r repair — kampanyaların STATIC_R kolu / operatör-forced FEC.
    /// `None` = Controller modu: yapı-gate (Q18-fix) kararı latch'e akar (adaptif).
    pub manual_r: Option<u16>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_inflight_units: 4096,
            offer_queue: 1024,
            delivery_queue: 1024,
            max_unit_bytes: 1200,
            max_peers: 64,
            quota_r: 4,
            drain_cap: Duration::from_secs(3),
            idle_hold: Duration::from_secs(105),
            fec: None,
        }
    }
}

/// [S1] FEC-config doğrulaması (connect/bind, koşu-öncesi typed): profil bilinmeli ve kabul
/// edilen en büyük payload canonical symbol'e sığmalı (prefix dahil) — aksi hâlde blok/esi
/// hizası sessizce bozulurdu; bounds-before-run.
fn validate_fec(cfg: &RuntimeConfig) -> Result<Option<FecProfile>, RuntimeError> {
    let Some(f) = cfg.fec else { return Ok(None) };
    let p = by_id(f.profile_id).ok_or(RuntimeError::FecConfig("unknown profile_id"))?;
    if cfg.max_unit_bytes + SOURCE_PREFIX_LEN > p.e_usize() {
        return Err(RuntimeError::FecConfig(
            "max_unit_bytes + SOURCE_PREFIX_LEN > profile.e (payload symbol'e sığmıyor)",
        ));
    }
    if let Some(r) = f.manual_r {
        if r == 0 || r > p.r_code_max {
            return Err(RuntimeError::FecConfig("manual_r 1..=r_code_max dışında"));
        }
    }
    Ok(Some(p))
}

/// [v1/PARAM] Profilden canonical `ParamFields` (§8; iki uç AYNI türetir — hash buna bağlanır).
fn param_fields_for(p: &FecProfile) -> ParamFields {
    ParamFields {
        protocol_version: 1,
        fec_version: 1,
        code_suite_id: p.code_suite_id,
        profile_id: p.profile_id,
        k_shape: p.k_shape,
        r_code_max: p.r_code_max,
        e: p.e,
        partial_policy: 0,
        packet_scheduling_policy: 0,
    }
}

/// Typed çalışma-zamanı hataları — reachable `todo!()` YASAK; her sınır typed reddeder.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Offer kuyruğu dolu (backpressure): app yeniden dener veya düşürür — transport BLOKLAMAZ.
    #[error("offer kuyruğu dolu (bounded backpressure)")]
    QueueFull,
    /// Payload birim tavanını aşıyor (bounds-before-alloc).
    #[error("payload {got} B > max_unit_bytes {cap} B")]
    UnitTooLarge { got: usize, cap: usize },
    /// Endpoint kapatılmış (görev bitti / shutdown sonrası).
    #[error("endpoint kapalı")]
    Closed,
    /// Soket kuruluş hatası (bind/connect) — koşu-öncesi; koşu-içi send hataları kayıp sayılır.
    #[error("soket: {0}")]
    Io(#[from] std::io::Error),
    /// [S1] FEC konfigürasyonu geçersiz (bilinmeyen profil / payload-tavanı symbol'e sığmıyor).
    #[error("fec config: {0}")]
    FecConfig(&'static str),
}

/// Egress kapı-dikişi (opsiyonel; üretim default'u `None` — hiçbir kapı yok).
///
/// **SoC:** politika datagram'ın NE taşıdığını bilmez — yalnız mühürlü uzunluğu ve endpoint
/// koşu-saatini görür. Transport↔security körlüğünün egress tarafı: kapı içeriğe BAKMAZ.
///
/// `admit` `false` dönerse datagram sokete YAZILMAZ ve bu **kanal-kaybı eşdeğeridir**: motor
/// durumu emisyonu ZATEN işledi, karar geri itilmez, send_rate'e dokunulmaz (INV-RATE-1).
/// Politika ASLA geciktirmez/kuyruk tutmaz — ret ya da geçir (paket-atomik).
///
/// Kullanım: benchmark harness'ının yön-başına bütçe governor'ü; ileride operatör kotası.
/// `Arc<Mutex<_>>` ile sarılırsa çağıran koşu sonunda politikanın kendi sayaçlarını okur.
pub trait EgressPolicy: Send {
    /// Bu datagram kanala enjekte edilebilir mi? `len` = mühürlü UDP payload byte'ı;
    /// `now_ns` = endpoint koşu-saati (monoton, `connect`/`bind` anından itibaren).
    fn admit(&mut self, len: usize, now_ns: u64) -> bool;

    /// Arm işareti ([`ClientHandle::mark_t0`] / [`ServerHandle::mark_t0`]): bu andan ÖNCESİ
    /// politikadan muaftır (kuruluş/warmup). Default: no-op.
    fn arm(&mut self, now_ns: u64) {
        let _ = now_ns;
    }
}

/// tx byte'ının sınıf etiketi — YALNIZ sayaç ayrımı için; wire'a hiçbir şey yazmaz.
#[derive(Clone, Copy)]
enum TxClass {
    /// Yeni DATA (ilk emisyon).
    Data,
    /// ARQ retransmit (§9).
    Retx,
    /// §11 forward duplication.
    Dup,
    /// FEC repair-shard.
    Fec,
    /// Kontrol düzlemi (PARAM_SET / PARAM_ACK / RECOVERED / ACK).
    Ctrl,
}

/// Client koşu istatistikleri (item 9 doğrulama yüzeyi; gözlem-amaçlı).
#[derive(Debug, Default, Clone, Copy)]
pub struct ClientStats {
    /// App'ten kabul edilen birim.
    pub offered: u64,
    /// İlk-ACK'i görülen birim (ilk-ack-kazanır dedup).
    pub acked: u64,
    /// Drain sonunda hâlâ teslim-teyitsiz birim (kabul edilen residue).
    pub undelivered: u64,
    /// Wire'a yazılan / wire'dan okunan datagram.
    pub tx_datagrams: u64,
    pub rx_datagrams: u64,
    /// `seam.open` reddi (doğrulanamayan gelen datagram — sessiz düşürüldü).
    pub open_failures: u64,
    /// Açıldı ama tanınmayan çerçeve tipi (ne DATA ne ACK) — kontrol-düzlemi dal-noktası
    /// (item 6, workstream B'de kama-noise-wire formatıyla bağlanır); sessiz yutulmaz, sayılır.
    pub unknown_frames: u64,
    /// Payload'ı store'da bulunamayan (çoktan ACK'lenmiş birime geç karar) emisyon — kayıp sayıldı.
    pub stale_decisions: u64,
    /// Soket send hatası (kanal kaybı eşdeğeri, INV-RATE-1).
    pub send_errors: u64,
    /// Shutdown anında offer-kuyruğunda kalmış, HİÇ işlenmemiş birim (app `Ok` almıştı ama
    /// gönderim denenmedi — sessiz kayıp yerine sayaç; app kapanışta bunu okur).
    pub unaccepted_offers: u64,
    /// [S1] Latch'in parity planladığı (ve RS-encode edilen) blok sayısı.
    pub fec_blocks_latched: u64,
    /// [S1] Wire'a yazılan gerçek repair-shard.
    pub fec_repairs_tx: u64,
    /// [S1] Repair-store'da bulunamayan FecParity kararı (evict/latch-0) — başlık-only gitti.
    pub fec_repair_missing: u64,
    /// [S1] RECOVERED-control ile teslim-teyidi alan birim (alıcı FEC-kurtardı → retransmit iptal,
    /// store'dan düşer, app `acked()` akışına girer).
    pub fec_recovered_ctrl_units: u64,
    /// [v1] Typed seal-reddi (seam hazır-değil/PN) — emisyon kayıp sayıldı (INV-RATE-1).
    pub seal_failures: u64,
    /// [v1/PARAM] Gönderilen PARAM_SET çerçevesi (bounded retry).
    pub param_sends: u64,
    /// [v1/PARAM] PARAM_ACK hash-echo doğrulandı (0/1; B2: emisyon zaten akıyordu).
    pub param_confirmed: u64,
    /// Wire'a yazılmak üzere ÜRETİLEN mühürlü UDP payload byte'ı — DENEME düzlemi
    /// ([`EgressPolicy`] reddettikleri DAHİL; gerçek-enjekte düzlem politikanın kendi
    /// sayacındadır). Alt-sınıf ayrımı `tx_*_bytes` alanlarında; toplamları bu değere eşittir.
    pub tx_bytes: u64,
    /// Wire'dan okunan UDP payload byte'ı.
    pub rx_bytes: u64,
    /// Yeni DATA byte'ı.
    pub tx_data_bytes: u64,
    /// ARQ retransmit byte'ı (§9).
    pub tx_retx_bytes: u64,
    /// §11 forward duplication byte'ı.
    pub tx_dup_bytes: u64,
    /// FEC repair-shard byte'ı.
    pub tx_fec_bytes: u64,
    /// Kontrol-düzlemi byte'ı (PARAM_SET).
    pub tx_ctrl_bytes: u64,
    /// [`EgressPolicy`] reddi: hiç enjekte edilmeyen datagram/byte (kanal-kaybı eşdeğeri).
    pub egress_refused_pkts: u64,
    /// Reddedilen byte toplamı.
    pub egress_refused_bytes: u64,
}

/// Server koşu istatistikleri.
#[derive(Debug, Default, Clone, Copy)]
pub struct ServerStats {
    /// App'e teslim edilen birim (ilk-teslim).
    pub delivered: u64,
    /// Teslim kanalı doluyken düşen teslim (app drenaj sözleşmesi ihlali görünür olsun).
    pub delivery_dropped: u64,
    /// Peer-cap fail-closed reddi (yeni peer, tablo dolu).
    pub dropped_new_peers: u64,
    /// Idle-emekliliğe ayrılan peer (item 3).
    pub retired_idle_peers: u64,
    pub rx_datagrams: u64,
    pub open_failures: u64,
    /// Açıldı ama tanınmayan çerçeve (kontrol-düzlemi dal-noktası; bkz. ClientStats.unknown_frames).
    pub unknown_frames: u64,
    /// [S1] Alınan repair-shard (blok-birleştiriciye girdi).
    pub fec_repairs_rx: u64,
    /// [S1] FEC-reconstruct ile kurtarılıp app'e teslim edilen birim (DATA'sı hiç gelmemişti).
    pub fec_recovered_units: u64,
    /// [S1] Hazır-görünen blokta reconstruct hatası (blok KORUNUR — shard'lar atılmaz; readiness
    /// sayım-bug'ının düzeltilmiş davranışı).
    pub fec_decode_failures: u64,
    /// [S1] Kapasite gereği düşürülen en-eski blok (bounded birleştirici).
    pub fec_blocks_evicted: u64,
    /// [S1] Wire'a yazılan RECOVERED-control çerçevesi (blok başına bir).
    pub fec_recovered_ctrl_tx: u64,
    /// [v1] Typed seal-reddi (ACK/RECOVERED/PARAM_ACK dönüş yolu).
    pub seal_failures: u64,
    /// [v1/PARAM] PARAM_SET doğrulanıp bağlanan peer (ilk-bind başına 1).
    pub param_bound_peers: u64,
    /// [v1/PARAM] PARAM_SET doğrulama reddi (hash/profil-mismatch; fail-closed, bind YOK).
    pub param_rejects: u64,
    /// [v1/PARAM] PARAM-bağlanmadan gelen FEC-shard (fail-closed sayaçlı düşürme).
    pub fec_pre_param_dropped: u64,
    /// Dönüş yönünde üretilen datagram (ACK/RECOVERED/PARAM_ACK) — DENEME düzlemi.
    pub tx_datagrams: u64,
    /// Dönüş yönünde üretilen mühürlü UDP payload byte'ı — DENEME düzlemi ([`EgressPolicy`]
    /// reddettikleri DAHİL; client tarafındaki `tx_bytes` ile aynı sözleşme).
    pub tx_bytes: u64,
    /// Wire'dan okunan UDP payload byte'ı.
    pub rx_bytes: u64,
    /// [`EgressPolicy`] reddi: hiç enjekte edilmeyen datagram/byte.
    pub egress_refused_pkts: u64,
    /// Reddedilen byte toplamı.
    pub egress_refused_bytes: u64,
}

/// Teslim olayı: hangi eşten hangi birim, payload'ıyla.
#[derive(Debug, Clone)]
pub struct Delivery {
    pub peer: SocketAddr,
    pub unit_id: u64,
    pub payload: Vec<u8>,
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

// ─────────────────────────────── CLIENT ───────────────────────────────

/// Client ucu: [`ClientSession`] (G2 sevk default'ları — tail-ON validated_v1) + UDP döngüsü.
pub struct ClientEndpoint;

/// Client tutamağı: bounded offer + ACK olay akışı + kontrollü kapanış.
pub struct ClientHandle {
    offer: ClientSender,
    acked_rx: mpsc::Receiver<u64>,
    shutdown_tx: watch::Sender<bool>,
    t0_tx: mpsc::Sender<oneshot::Sender<()>>,
    task: JoinHandle<ClientStats>,
}

/// Çok-üreticili offer tutamağı ([`ClientHandle::sender`]). ACK akışını `&mut` ile tutan
/// döngüden BAĞIMSIZ ödünç alınabilir (aynı `select!` içinde hem sunmak hem ACK drenajı).
#[derive(Clone)]
pub struct ClientSender {
    offer_tx: mpsc::Sender<Vec<u8>>,
    max_unit_bytes: usize,
}

impl ClientSender {
    /// Bir birim sun (non-blocking; bounded). Dolu kuyruk → typed `QueueFull` (transport bloklamaz).
    ///
    /// # Errors
    /// `UnitTooLarge` (bounds-before-alloc) · `QueueFull` (backpressure) · `Closed`.
    pub fn send(&self, payload: Vec<u8>) -> Result<(), RuntimeError> {
        if payload.len() > self.max_unit_bytes {
            return Err(RuntimeError::UnitTooLarge {
                got: payload.len(),
                cap: self.max_unit_bytes,
            });
        }
        self.offer_tx.try_send(payload).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => RuntimeError::QueueFull,
            mpsc::error::TrySendError::Closed(_) => RuntimeError::Closed,
        })
    }
}

impl ClientEndpoint {
    /// Bağlan ve görevi başlat. `seam` = güvenlik dikişi (stub: [`crate::seam::IdentitySeam`]).
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind/connect hatası.
    pub async fn connect(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn SecuritySeam>,
    ) -> Result<ClientHandle, RuntimeError> {
        Self::connect_with_policy(server, cfg, seam, None).await
    }

    /// [`connect`](Self::connect) + opsiyonel egress kapısı ([`EgressPolicy`]). Politika `None`
    /// iken davranış `connect` ile BİREBİR aynıdır (kapı kodu hiç çalışmaz).
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind/connect hatası.
    pub async fn connect_with_policy(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn SecuritySeam>,
        egress: Option<Box<dyn EgressPolicy>>,
    ) -> Result<ClientHandle, RuntimeError> {
        validate_fec(&cfg)?; // [S1] koşu-öncesi typed FEC-config doğrulaması
        let sock = UdpSocket::bind(match server {
            SocketAddr::V4(_) => "0.0.0.0:0",
            SocketAddr::V6(_) => "[::]:0",
        })
        .await?;
        sock.connect(server).await?;
        let (offer_tx, offer_rx) = mpsc::channel(cfg.offer_queue);
        let (acked_tx, acked_rx) = mpsc::channel(cfg.offer_queue.max(cfg.max_inflight_units));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Kapasite 1 + onay-oneshot: mark_t0 görev işleyene dek bekler (ölçüm sözleşmesi).
        let (t0_tx, t0_rx) = mpsc::channel::<oneshot::Sender<()>>(1);
        let max_unit_bytes = cfg.max_unit_bytes;
        let task = tokio::spawn(client_task(
            sock,
            cfg,
            seam,
            egress,
            offer_rx,
            acked_tx,
            shutdown_rx,
            t0_rx,
        ));
        Ok(ClientHandle {
            offer: ClientSender {
                offer_tx,
                max_unit_bytes,
            },
            acked_rx,
            shutdown_tx,
            t0_tx,
            task,
        })
    }
}

impl ClientHandle {
    /// Bir birim sun (non-blocking; bounded). Dolu kuyruk → typed `QueueFull` (transport bloklamaz).
    ///
    /// # Errors
    /// `UnitTooLarge` · `QueueFull` · `Closed` — bkz. [`ClientSender::send`].
    pub fn send(&self, payload: Vec<u8>) -> Result<(), RuntimeError> {
        self.offer.send(payload)
    }

    /// Klonlanabilir offer tutamağı (bkz. [`ClientSender`]).
    #[must_use]
    pub fn sender(&self) -> ClientSender {
        self.offer.clone()
    }

    /// Ölçüm-t0 işareti: **wire sayaçları sıfırlanır, TRANSPORT DURUMU KORUNUR**
    /// (counter-reset-without-state-reset) ve varsa [`EgressPolicy`] arm edilir — bu andan
    /// öncesi (kuruluş/warmup) kapıdan muaftır. App-düzeyi sayaçlara (`offered`/`acked`/FEC)
    /// DOKUNMAZ.
    ///
    /// **Görev İŞLEYENE KADAR bekler.** Bu bir kolaylık değil, ölçüm sözleşmesidir: dönüşten
    /// sonra sunulan HİÇBİR birim arm-öncesi muafiyetten yararlanamaz. (Ateşle-unut bir işaret
    /// bunu garanti edemezdi — `select!` t0 ile bekleyen bir offer arasında rastgele seçer ve
    /// ilk datagram bütçesiz kaçardı; sıfır-bütçe kolunda bir teslim olarak yakalandı.)
    /// Görev ölmüşse sessizce döner.
    pub async fn mark_t0(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.t0_tx.send(ack_tx).await.is_ok() {
            let _ = ack_rx.await;
        }
    }

    /// İlk-ACK olay akışı (birim id'leri; ilk-ack-kazanır).
    pub async fn acked(&mut self) -> Option<u64> {
        self.acked_rx.recv().await
    }

    /// Kontrollü kapanış: drain (pif→0 VEYA `drain_cap`) → istatistik. Görev panik ederse
    /// varsayılan istatistik döner (panik yutulmaz: ayrı gözlemlenebilir, unwrap edilmez).
    pub async fn shutdown(self) -> ClientStats {
        let _ = self.shutdown_tx.send(true);
        self.task.await.unwrap_or_default()
    }
}

/// Client görevi: bench `client_loop` şekli, ölçüm makinesi çıkarılmış, sınırlar eklenmiş hâli.
#[allow(clippy::too_many_arguments)]
async fn client_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    mut seam: Box<dyn SecuritySeam>,
    mut egress: Option<Box<dyn EgressPolicy>>,
    mut offer_rx: mpsc::Receiver<Vec<u8>>,
    acked_tx: mpsc::Sender<u64>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut t0_rx: mpsc::Receiver<oneshot::Sender<()>>,
) -> ClientStats {
    let start = Instant::now();
    // G2 sevk default'ları TEK kaynaktan: ClientSession (tail-ON validated_v1, startup-pacing motor
    // default'u ON, bootstrap OFF, FEC Manual-off). Burada default'a dokunulmaz.
    let mut session = ClientSession::new();
    let mut stats = ClientStats::default();
    // [S1] FEC-runtime kuruluşu (cfg.fec varsa): Controller-mode — yapı-gate'in (Q18) kararı
    // latch'e ancak bu modda akar. validate_fec connect'te koştu; profil burada çözülür.
    let fec: Option<FecProfile> = cfg.fec.and_then(|f| by_id(f.profile_id));
    let fec_manual_r: Option<u16> = cfg.fec.and_then(|f| f.manual_r);
    if fec.is_some() {
        let mode = if fec_manual_r.is_some() {
            FecActuationMode::Manual { enabled: true } // statik-r (STATIC_R kolu)
        } else {
            FecActuationMode::Controller // adaptif: yapı-gate (Q18) kararı latch'e akar
        };
        session
            .sender_mut()
            .set_fec_actuation_mode(mode)
            .expect("taze Sender mode-set kabul eder");
    }
    // [v1] Seam-PN sayacı (per-endpoint; noise-wire caller-supplied sözleşmesi) + PARAM planı.
    let mut tx_pn: u64 = 0;
    const CLIENT_SEAM_PEER: SeamPeer = 0;
    let param_plan = fec
        .as_ref()
        .and_then(|p| plan_param_bootstrap(1, &param_fields_for(p)).ok());
    let mut param_confirmed = param_plan.is_none(); // FEC-yoksa PARAM-akışı yok.
    let mut param_last_tick_ms: u64 = 0;
    let mut param_tries: u32 = 0;
    // [S1] Ardışık-blok birikimi: unit_id → (block = id/K, esi = id%K) deterministik (iki uç da
    // türetir); yalnız SON blok birikir. K dolunca same-event latch + gerçek RS-encode →
    // bounded repair_store (en-eski düşer → o bloğun FecParity kararı başlık-only'e düşer).
    let mut cur_block: Vec<Vec<u8>> = Vec::new();
    let mut cur_block_id: u32 = 0;
    let mut repair_store: BTreeMap<u32, Vec<RepairSymbol>> = BTreeMap::new();
    const REPAIR_STORE_BLOCKS: usize = 64;
    // unit-store: unit_id → payload; varlık = outstanding. İlk-ACK'te düşer (sınır: cfg).
    let mut store: HashMap<u64, Vec<u8>> = HashMap::new();
    // seq → unit haritası + birim → seq'leri (ACK'te toplu budama — harita sınırlı kalır).
    let mut seq_to_unit: HashMap<u64, u64> = HashMap::new();
    let mut unit_seqs: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut max_emitted_seq: u64 = 0;
    let mut next_unit: u64 = 0;
    let mut pace = interval(PACE_TICK);
    let mut rtprop = interval(RTPROP_TICK);
    let mut buf = vec![0u8; CLIENT_RECV_BUF];

    // Motoru bir olayla sürer + kararları mühürleyip wire'a yazar. seq→unit kaydı GÖNDERİMDEN
    // BAĞIMSIZ önce yapılır (acked_full(max_emitted+1) doğruluğu buna bağlı; refused/failed send
    // de seq'ini işgal eder). Send hatası = kanal kaybı (INV-RATE-1; abort YOK).
    // [v1] Tek-nokta seal+send: typed seal-reddi sayılır, PN yalnız başarılı seal'de ilerler.
    macro_rules! seal_send {
        ($frame:expr, $class:expr, $now:expr) => {
            match seam.seal(
                CLIENT_SEAM_PEER,
                $frame,
                SealArgs {
                    wire_pn: tx_pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            ) {
                Ok(sd) => {
                    tx_pn = tx_pn.wrapping_add(1);
                    let n = sd.bytes.len();
                    let n64 = n as u64;
                    // DENEME düzlemi: kapı reddetse de üretilen byte sayılır (sınıf ayrımıyla).
                    stats.tx_datagrams += 1;
                    stats.tx_bytes += n64;
                    match $class {
                        TxClass::Data => stats.tx_data_bytes += n64,
                        TxClass::Retx => stats.tx_retx_bytes += n64,
                        TxClass::Dup => stats.tx_dup_bytes += n64,
                        TxClass::Fec => stats.tx_fec_bytes += n64,
                        TxClass::Ctrl => stats.tx_ctrl_bytes += n64,
                    }
                    // Egress kapısı: ret = kanal-kaybı eşdeğeri. Motor durumu emisyonu ZATEN
                    // işledi; geri itilmez, rate'e dokunulmaz (INV-RATE-1).
                    let permitted = match egress.as_mut() {
                        Some(p) => p.admit(n, $now),
                        None => true,
                    };
                    if permitted {
                        if sock.send(&sd.bytes).await.is_err() {
                            stats.send_errors += 1; // kayıp; asla geri itme (INV-RATE-1).
                        }
                    } else {
                        stats.egress_refused_pkts += 1;
                        stats.egress_refused_bytes += n64;
                    }
                }
                Err(_) => {
                    stats.seal_failures += 1; // typed-red; emisyon kayıp sayıldı.
                }
            }
        };
    }
    macro_rules! drive {
        ($ev:expr, $now:expr) => {
            for d in session.on_event($ev, $now) {
                // Seq-uzayı takibi HER kararda (acked_full(max+1) doğruluğu — repair'ler de seq
                // işgal eder); ama seq→unit haritası YALNIZ birim-taşıyan kararlarda: FecParity'nin
                // unit_id'si ESI'dir (AYRI ad-uzayı) — haritaya girseydi repair-ACK'i gerçek bir
                // birimin store'unu silerdi (S1'de yakalanan tuzak).
                max_emitted_seq = max_emitted_seq.max(d.seq);
                let carries_unit = matches!(
                    d.kind,
                    TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
                );
                if carries_unit {
                    seq_to_unit.insert(d.seq, d.unit_id);
                    unit_seqs.entry(d.unit_id).or_default().push(d.seq);
                } else {
                    // [S1] FecParity (FEC-açıkken): repair-store'daki gerçek RS-repair →
                    // FRAME_FEC → seam → wire. Bulunamazsa (evict/latch-0) başlık-only emisyon
                    // (DATA store-miss gerekçesiyle aynı: motor her emisyonu inflight izler; hiç
                    // göndermemek yapay kayıp sinyali üretirdi).
                    if d.kind == TransmitKind::FecParity {
                        if let (Some(p), Some(hint)) = (fec.as_ref(), d.send_record) {
                            let SrLogicalRef::Fec { block_id, esi } = hint.logical_ref;
                            let sym: Option<&[u8]> = repair_store
                                .get(&block_id)
                                .and_then(|rs| rs.iter().find(|r| r.esi == esi))
                                .map(|r| r.bytes.as_slice());
                            if sym.is_some() {
                                stats.fec_repairs_tx += 1;
                            } else {
                                stats.fec_repair_missing += 1;
                            }
                            let frame = encode_fec(
                                &FecFrameHeader {
                                    kind: inner_kind::FEC_REPAIR,
                                    profile_id: p.profile_id,
                                    block_id,
                                    esi,
                                    k_actual: u8::try_from(p.k_shape).unwrap_or(255),
                                    seq: (d.seq & 0xFFFF) as u16,
                                },
                                sym.unwrap_or(&[]),
                            );
                            seal_send!(frame, TxClass::Fec, $now);
                            continue;
                        }
                    }
                    // CsProbe / FEC-kapalıyken FecParity: v0 kabuk taşımaz, sayar (bootstrap OFF;
                    // sevk default'larında üretilmezler — typed genişleme noktası).
                    stats.stale_decisions += 1;
                    continue;
                }
                // Store-miss = birim BAŞKA bir kopyasıyla çoktan teslim edilmiş, motor eski seq'in
                // retransmit'ini istiyor. Emisyon YİNE wire'a çıkar (başlık-only): motor her
                // emisyonu inflight izler — hiç göndermemek yapay kayıp sinyali üretirdi (bench her
                // kararı gönderirdi). Alıcı birimi dedup eder, seq'i ACK'ler → motor temiz kapatır.
                let (payload, stale): (&[u8], bool) = match store.get(&d.unit_id) {
                    Some(p) => (p.as_slice(), false),
                    None => (&[], true),
                };
                if stale {
                    stats.stale_decisions += 1;
                }
                let frame = encode_data((d.seq & 0xFFFF) as u16, d.unit_id, payload);
                // Sınıf ayrımı motorun karar tipinden: ilk emisyon / ARQ retransmit / §11 dup.
                let class = match d.kind {
                    TransmitKind::Retransmit => TxClass::Retx,
                    TransmitKind::Duplicate => TxClass::Dup,
                    _ => TxClass::Data,
                };
                seal_send!(frame, class, $now);
            }
        };
    }
    // Gelen datagram: seam.open → ACK (V1 0x10 / V2 0x14) → ilk-ACK damgası + motor beslemesi.
    macro_rules! handle_rx {
        ($raw:expr, $now:expr) => {
            let raw: &[u8] = $raw;
            stats.rx_datagrams += 1;
            stats.rx_bytes += raw.len() as u64;
            if let Ok(od) = seam.open(raw) {
                let plain = od.plain;
                if plain.first() == Some(&FRAME_PARAM_ACK) {
                    // [v1/PARAM] hash-echo confirmation (B2: yalnız retry-durdurur; emisyon zaten akıyor).
                    if let (Some(plan), Some(h)) = (param_plan.as_ref(), decode_param_ack(&plain)) {
                        if h == plan.param_hash && !param_confirmed {
                            param_confirmed = true;
                            stats.param_confirmed = 1;
                        }
                    } else {
                        stats.unknown_frames += 1;
                    }
                } else if plain.first() == Some(&FRAME_RECOVERED) {
                    // [S1] RECOVERED-control: alıcı bu ESI'leri FEC ile kurtardı → birim teslim-
                    // teyitli: store'dan düşer, app acked() akışına girer, motor saf-logical iptal
                    // eder (retransmit israfı biter). unit_id = block*K + esi (deterministik atama).
                    match (fec.as_ref(), decode_recovered(&plain)) {
                        (Some(p), Some((block_id, esis))) => {
                            let mut units = Vec::with_capacity(esis.len());
                            for esi in esis {
                                let unit =
                                    u64::from(block_id) * u64::from(p.k_shape) + u64::from(esi);
                                if store.remove(&unit).is_some() {
                                    stats.fec_recovered_ctrl_units += 1;
                                    let _ = acked_tx.try_send(unit);
                                    for old in unit_seqs.remove(&unit).unwrap_or_default() {
                                        seq_to_unit.remove(&old);
                                    }
                                }
                                units.push(RecoveredUnit {
                                    unit_id: unit,
                                    block_id,
                                    esi,
                                });
                            }
                            drive!(Event::RecoveredControl(RecoveredInfo { units }), $now);
                        }
                        _ => stats.unknown_frames += 1,
                    }
                } else if plain.is_empty()
                    || (plain[0] != FRAME_TYPE_ACK && plain[0] != FRAME_TYPE_ACK_V2)
                {
                    stats.unknown_frames += 1;
                } else {
                    if let Ok(frame) = AckFrame::decode(&plain) {
                        let acked = frame.acked_full(max_emitted_seq + 1);
                        for s in &acked {
                            let Some(&unit) = seq_to_unit.get(s) else { continue };
                            if store.remove(&unit).is_some() {
                                stats.acked += 1;
                                let _ = acked_tx.try_send(unit); // app drenajı; taşarsa olay düşer, teslim durumu store'da zaten kesin.
                                // Birimin TÜM seq kayıtları budanır — haritalar store disipliniyle sınırlı kalır.
                                for old in unit_seqs.remove(&unit).unwrap_or_default() {
                                    seq_to_unit.remove(&old);
                                }
                            }
                        }
                        // Kampanya-kanıtlı sözleşme (bench birebir): acked_full çıktısı diff'siz
                        // beslenir; motor inflight.remove ile idempotent işler.
                        drive!(
                            Event::AckReceived(AckInfo {
                                acked_seqs: acked,
                                ack_delay: frame.ack_delay_duration(),
                            }),
                            $now
                        );
                    }
                }
            } else {
                stats.open_failures += 1; // verify-fail: sessiz düşür, durum değişmez.
            }
        };
    }

    // ── Ana döngü ──
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            // Ölçüm-t0 işareti (ClientHandle::mark_t0): wire sayaçları sıfırlanır, MOTOR DURUMU
            // KORUNUR (counter-reset-without-state-reset) ve egress kapısı arm edilir. Onay
            // gönderilene dek çağıran bekler → t0'dan SONRAKİ hiçbir sunum arm-öncesi kalmaz.
            Some(ack) = t0_rx.recv() => {
                let now = elapsed_ns(start);
                if let Some(p) = egress.as_mut() { p.arm(now); }
                stats.tx_datagrams = 0;
                stats.rx_datagrams = 0;
                stats.tx_bytes = 0;
                stats.rx_bytes = 0;
                stats.tx_data_bytes = 0;
                stats.tx_retx_bytes = 0;
                stats.tx_dup_bytes = 0;
                stats.tx_fec_bytes = 0;
                stats.tx_ctrl_bytes = 0;
                stats.egress_refused_pkts = 0;
                stats.egress_refused_bytes = 0;
                let _ = ack.send(()); // çağıran serbest: bundan sonrası ölçüm penceresi
            }
            // Offer YALNIZ store'da yer varken alınır (bounds-before-alloc: kanal + store çift sınır).
            maybe = offer_rx.recv(), if store.len() < cfg.max_inflight_units => {
                let Some(payload) = maybe else { break }; // tüm handle'lar düştü → drain'e geç.
                let id = next_unit;
                next_unit += 1;
                stats.offered += 1;
                let size = seam.sealed_len(HDR_DATA + payload.len()) as u64;
                store.insert(id, payload);
                let now = elapsed_ns(start);
                drive!(Event::AppData(OpaqueUnit { id, size }), now);
                // [S1] Blok birikimi + K'de SAME-EVENT latch: drive! az önce on_event koşturdu →
                // snapshot.event_index taze; arada başka event YOK (tek-task sıralılık) → latch'in
                // same-event-causality guard'ı geçer.
                if let Some(p) = fec.as_ref() {
                    let e = p.e_usize();
                    let mut sym = vec![0u8; e];
                    let payload_ref = store.get(&id).expect("az önce insert edildi");
                    let pfx = SourcePrefix {
                        message_id: (id & 0xFFFF_FFFF) as u32,
                        chunk_offset: 0,
                        actual_payload_len: payload_ref.len() as u16,
                        source_flags: 0,
                    };
                    // validate_fec tavanı garantiledi; olmazsa iki uç da aynı sonucu türetir
                    // (deterministik) — sıfır-symbol hizayı korur.
                    let _ = pfx.serialize_symbol(payload_ref, &mut sym);
                    debug_assert_eq!(cur_block.len() as u64, id % u64::from(p.k_shape));
                    cur_block.push(sym);
                    if cur_block.len() == p.k_shape_usize() {
                        let parity_wire = seam.sealed_len(HDR_FEC + e) as u64;
                        // Statik-r (Manual{true} + manuel-close) YA DA adaptif (Controller + latch).
                        let planned_r: u16 = if let Some(r) = fec_manual_r {
                            let auth = session.sender_mut().open_fec_block_authority();
                            session
                                .sender_mut()
                                .close_fec_block_manual(auth, cur_block_id, r, parity_wire)
                                .unwrap_or(0)
                        } else {
                            let snap = session.sender().controller_decision_snapshot();
                            let auth = session.sender_mut().open_fec_block_authority();
                            session
                                .sender_mut()
                                .fec_controller_latch(
                                    auth,
                                    snap,
                                    u8::try_from(p.k_shape).unwrap_or(255),
                                    p.r_code_max,
                                    cur_block_id,
                                    parity_wire,
                                )
                                .planned_r
                        };
                        if planned_r > 0 {
                            let srcs: Vec<SourceSymbol> = cur_block
                                .iter()
                                .enumerate()
                                .map(|(i, b)| SourceSymbol {
                                    esi: i as u16,
                                    bytes: b,
                                })
                                .collect();
                            if let Ok(reps) = RsSimdCodec.encode(p, &srcs) {
                                stats.fec_blocks_latched += 1;
                                repair_store.insert(cur_block_id, reps);
                                while repair_store.len() > REPAIR_STORE_BLOCKS {
                                    repair_store.pop_first();
                                }
                            }
                        }
                        cur_block.clear();
                        cur_block_id = cur_block_id.wrapping_add(1);
                    }
                }
            }
            _ = pace.tick() => { let now = elapsed_ns(start); drive!(Event::TimerFired(Timer::Pto), now); }
            _ = rtprop.tick() => {
                let now = elapsed_ns(start);
                // [v1/PARAM] bounded retry (B2): confirmed olana dek her ≥200ms'de bir, ≤25 deneme.
                if let Some(plan) = param_plan.as_ref() {
                    let now_ms = now / 1_000_000;
                    let due = param_tries == 0 || now_ms >= param_last_tick_ms + 200;
                    if !param_confirmed && param_tries < 25 && due {
                        if let (Some(p), Control::ParamSet { param_hash, .. }) = (fec.as_ref(), &plan.control) {
                            let f = encode_param(p.profile_id, p.code_suite_id, param_hash);
                            seal_send!(f, TxClass::Ctrl, now);
                            stats.param_sends += 1;
                            param_tries += 1;
                            param_last_tick_ms = now_ms;
                        }
                    }
                }
                drive!(Event::TimerFired(Timer::RtpropProbe), now);
            }
            r = sock.recv(&mut buf) => {
                let Ok(n) = r else { continue };
                let now = elapsed_ns(start);
                handle_rx!(&buf[..n], now);
            }
        }
    }

    // Kuyrukta kalan (hiç işlenmemiş) offer'lar sessizce kaybolmaz — sayılır.
    while offer_rx.try_recv().is_ok() {
        stats.unaccepted_offers += 1;
    }
    // ── Drain (D8): motor birim terk etmez → pif→0 hedefi MUTLAKA süre-tavanlı. ──
    let deadline = tokio::time::Instant::now() + cfg.drain_cap;
    loop {
        if store.is_empty() && session.sender().diag_pif() == 0 {
            break;
        }
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => break,
            _ = pace.tick() => { let now = elapsed_ns(start); drive!(Event::TimerFired(Timer::Pto), now); }
            r = sock.recv(&mut buf) => {
                let Ok(n) = r else { continue };
                let now = elapsed_ns(start);
                handle_rx!(&buf[..n], now);
            }
        }
    }
    stats.undelivered = store.len() as u64;
    stats
}

// ─────────────────────────────── SERVER ───────────────────────────────

/// Server ucu: eş-başına [`Receiver`] (bounded tablo) + ACK/patience tick'leri.
pub struct ServerEndpoint;

/// Server tutamağı: teslim akışı + kontrollü kapanış.
pub struct ServerHandle {
    delivery_rx: mpsc::Receiver<Delivery>,
    shutdown_tx: watch::Sender<bool>,
    t0_tx: mpsc::Sender<oneshot::Sender<()>>,
    task: JoinHandle<ServerStats>,
    local_addr: SocketAddr,
}

impl ServerEndpoint {
    /// Bağla ve görevi başlat.
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind hatası.
    pub async fn bind(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn SecuritySeam>,
    ) -> Result<ServerHandle, RuntimeError> {
        Self::bind_with_policy(bind, cfg, seam, None).await
    }

    /// [`bind`](Self::bind) + opsiyonel dönüş-yönü egress kapısı ([`EgressPolicy`]). Politika
    /// `None` iken davranış `bind` ile BİREBİR aynıdır.
    ///
    /// # Errors
    /// FEC-config reddi (koşu-öncesi typed) veya soket bind hatası.
    pub async fn bind_with_policy(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Box<dyn SecuritySeam>,
        egress: Option<Box<dyn EgressPolicy>>,
    ) -> Result<ServerHandle, RuntimeError> {
        validate_fec(&cfg)?; // [S1] koşu-öncesi typed FEC-config doğrulaması
        let sock = UdpSocket::bind(bind).await?;
        let local_addr = sock.local_addr()?;
        let (delivery_tx, delivery_rx) = mpsc::channel(cfg.delivery_queue);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Kapasite 1 + onay-oneshot: mark_t0 görev işleyene dek bekler (ölçüm sözleşmesi).
        let (t0_tx, t0_rx) = mpsc::channel::<oneshot::Sender<()>>(1);
        let task = tokio::spawn(server_task(
            sock,
            cfg,
            seam,
            egress,
            delivery_tx,
            shutdown_rx,
            t0_rx,
        ));
        Ok(ServerHandle {
            delivery_rx,
            shutdown_tx,
            t0_tx,
            task,
            local_addr,
        })
    }
}

impl ServerHandle {
    /// Gerçek bağlanılan adres (test/portsuz bind için).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Sıradaki teslim (ilk-teslim dedup'lu; kanal bounded).
    pub async fn next_delivery(&mut self) -> Option<Delivery> {
        self.delivery_rx.recv().await
    }

    /// Ölçüm-t0 işareti (dönüş yönü): wire sayaçları sıfırlanır, ALICI DURUMU KORUNUR ve varsa
    /// [`EgressPolicy`] arm edilir. Görev işleyene dek bekler — bkz. [`ClientHandle::mark_t0`].
    pub async fn mark_t0(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.t0_tx.send(ack_tx).await.is_ok() {
            let _ = ack_rx.await;
        }
    }

    /// Kapanış → istatistik.
    pub async fn shutdown(self) -> ServerStats {
        let _ = self.shutdown_tx.send(true);
        self.task.await.unwrap_or_default()
    }
}

/// Birim-düzeyi teslim-dedup penceresi (eş başına; SR-AUDIT-001 sınırı). Retransmit YENİ seq
/// taşıdığından [`Receiver`]'ın seq-dedup'u birim tekrarını YAKALAMAZ (ilk netem kapı-koşusu
/// bunu 26 duplicate-teslimle gösterdi); ilk-teslim sözleşmesi bu pencereyle sağlanır. Pencere
/// dolunca en-eski id düşer (bellek sınırı; pencere-dışı çok-geç duplicate teorik olarak yeniden
/// teslim edilebilir — E2E envelope katmanı §4 nihai dedup otoritesidir).
const DELIVERED_DEDUP_WINDOW: usize = 8192;

/// [S1] Peer-başına FEC blok-birleştirme tavanı (bounded, SR-AUDIT-001; en-eski düşer + sayaç).
const RX_BLOCKS_CAP: usize = 32;

/// [S1] Blok-birleştirme durumu: esi→symbol. Source-symbol DATA payload'ından deterministik
/// türetilir (TX ile aynı `SourcePrefix`); repair FRAME_FEC'ten gelir. BTreeMap = dup-ESI doğal
/// dedup (codec DuplicateEsi reddine hiç düşülmez).
struct BlockAsm {
    k: u8,
    sources: BTreeMap<u16, Vec<u8>>,
    repairs: BTreeMap<u16, Vec<u8>>,
}

/// Eş yuvası: alıcı motoru + idle-emeklilik çapası (item 3) + teslim-dedup penceresi.
struct PeerSlot {
    rx: Receiver,
    last_seen_ns: u64,
    delivered_seen: std::collections::BTreeSet<u64>,
    /// [S1] FEC blok-birleştirici (bounded; yalnız FEC-açıkken dolar).
    blocks: BTreeMap<u32, BlockAsm>,
    /// [v1] Seam-peer (open'ın kimliklendirdiği; dönüş-seal bu kimlikle).
    seam_peer: SeamPeer,
    /// [v1] Dönüş-yönü seam-PN sayacı (per-peer monoton).
    tx_pn: u64,
    /// [v1/PARAM] Per-peer bound-param durumu (fail-closed admission-gate).
    param: ParamState,
    /// [v1/PARAM] Bind-öncesi gelen FEC-shard'ların bounded bekletmesi (PreParamHold deseni;
    /// DÜŞÜRME yarış-duyarlıydı: repair'ler yeniden-üretilmez — CI yavaş-runner'da yakalandı).
    pre_param_fec: Vec<(FecFrameHeader, Vec<u8>)>,
}

/// [v1/PARAM] Per-peer pre-param hold tavanı (MAX_PREPARAM_SHARDS hizası, küçük: E≤1KB sınıfı).
const PRE_PARAM_FEC_CAP: usize = 64;

impl PeerSlot {
    fn next_pn(&mut self) -> u64 {
        let v = self.tx_pn;
        self.tx_pn = self.tx_pn.wrapping_add(1);
        v
    }
}

/// [v1] Server dönüş-yönü emisyon bağlamı: güvenlik dikişi + opsiyonel egress kapısı + koşu
/// saati. Tek yerde toplanır — böylece `srv_seal_send`'in dokuz çağrı-yeri saati/kapıyı elden
/// ele taşımaz (ve yeni bir çağrı-yeri kapıyı ATLAYAMAZ: tek geçit).
struct SrvTx {
    seam: Box<dyn SecuritySeam>,
    egress: Option<Box<dyn EgressPolicy>>,
    start: Instant,
}

/// [v1] Server-yönü tek-nokta seal+send (typed seal-reddi sayılır). Egress kapısı reddederse
/// datagram enjekte EDİLMEZ (kanal-kaybı eşdeğeri; alıcı durumu geri itilmez).
async fn srv_seal_send(
    tx: &mut SrvTx,
    sock: &UdpSocket,
    stats: &mut ServerStats,
    peer: SocketAddr,
    sp: SeamPeer,
    pn: u64,
    frame: Vec<u8>,
) {
    match tx.seam.seal(
        sp,
        frame,
        SealArgs {
            wire_pn: pn,
            dh_epoch: 0,
            send_generation: 0,
        },
    ) {
        Ok(sd) => {
            let n = sd.bytes.len();
            // DENEME düzlemi (client `tx_bytes` ile aynı sözleşme: ret dahil).
            stats.tx_datagrams += 1;
            stats.tx_bytes += n as u64;
            let permitted = match tx.egress.as_mut() {
                Some(p) => p.admit(n, elapsed_ns(tx.start)),
                None => true,
            };
            if permitted {
                let _ = sock.send_to(&sd.bytes, peer).await; // hata = kayıp.
            } else {
                stats.egress_refused_pkts += 1;
                stats.egress_refused_bytes += n as u64;
            }
        }
        Err(_) => stats.seal_failures += 1,
    }
}

/// [S1] Shard'ı peer'ın blok-birleştiricisine koyar; hazırsa reconstruct dener, kurtarılanları
/// teslim eder. READINESS (receiver_host'taki sayım-bug'ının DÜZELTİLMİŞ hâli): yalnız
/// eksik-source > 0 VE distinct(source)+distinct(repair) ≥ K iken denenir; reconstruct HATASI
/// bloğu SİLMEZ (shard'lar korunur — sonraki shard tamamlayabilir). Tam blok (sources ≥ K) işi
/// bitmiştir, düşülür. Kapasite aşımında en-eski blok düşer (sayaçla görünür).
#[allow(clippy::too_many_arguments)]
fn fec_ingest(
    stats: &mut ServerStats,
    delivery_tx: &mpsc::Sender<Delivery>,
    peer: SocketAddr,
    slot: &mut PeerSlot,
    p: &FecProfile,
    block_id: u32,
    k_actual: u8,
    esi: u16,
    symbol: Vec<u8>,
    is_repair: bool,
) -> Vec<u16> {
    let mut recovered_esis: Vec<u16> = Vec::new();
    let asm = slot.blocks.entry(block_id).or_insert_with(|| BlockAsm {
        k: k_actual,
        sources: BTreeMap::new(),
        repairs: BTreeMap::new(),
    });
    if is_repair {
        asm.repairs.entry(esi).or_insert(symbol);
    } else {
        asm.sources.entry(esi).or_insert(symbol);
    }
    let (complete, ready) = {
        let k = usize::from(asm.k);
        (
            asm.sources.len() >= k,
            asm.sources.len() + asm.repairs.len() >= k,
        )
    };
    if complete {
        slot.blocks.remove(&block_id); // tüm source'lar geldi — kurtarılacak yok.
    } else if ready {
        let result = {
            let asm = slot.blocks.get(&block_id).expect("entry az önce kuruldu");
            let recv: Vec<ReceivedSymbol> = asm
                .sources
                .iter()
                .map(|(e, b)| ReceivedSymbol {
                    esi: *e,
                    is_repair: false,
                    bytes: b,
                })
                .chain(asm.repairs.iter().map(|(e, b)| ReceivedSymbol {
                    esi: *e,
                    is_repair: true,
                    bytes: b,
                }))
                .collect();
            RsSimdCodec.reconstruct(p, &recv, 0)
        };
        match result {
            Ok(recovered) => {
                let k = slot.blocks.get(&block_id).map_or(k_actual, |a| a.k);
                for r in recovered {
                    recovered_esis.push(r.esi);
                    let unit_id = u64::from(block_id) * u64::from(k) + u64::from(r.esi);
                    let Ok(pfx) = SourcePrefix::parse(&r.bytes) else {
                        continue;
                    };
                    let len = usize::from(pfx.actual_payload_len);
                    let Some(payload) = r.bytes.get(SOURCE_PREFIX_LEN..SOURCE_PREFIX_LEN + len)
                    else {
                        continue;
                    };
                    // İlk-teslim dedup penceresi DATA-yoluyla ORTAK: DATA sonra gelirse (geç
                    // retransmit) çift-teslim edilmez; tersi de simetrik.
                    if slot.delivered_seen.insert(unit_id) {
                        if slot.delivered_seen.len() > DELIVERED_DEDUP_WINDOW {
                            slot.delivered_seen.pop_first();
                        }
                        let d = Delivery {
                            peer,
                            unit_id,
                            payload: payload.to_vec(),
                        };
                        if delivery_tx.try_send(d).is_ok() {
                            stats.delivered += 1;
                            stats.fec_recovered_units += 1;
                        } else {
                            stats.delivery_dropped += 1;
                        }
                    }
                }
                slot.blocks.remove(&block_id);
            }
            Err(_) => {
                stats.fec_decode_failures += 1; // blok KORUNUR (bug-fix): shard atılmaz.
            }
        }
    }
    while slot.blocks.len() > RX_BLOCKS_CAP {
        slot.blocks.pop_first();
        stats.fec_blocks_evicted += 1;
    }
    recovered_esis
}

/// Server görevi: bench `serve_loop` şekli + bounded peers + idle-emeklilik + teslim akışı.
async fn server_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    seam: Box<dyn SecuritySeam>,
    egress: Option<Box<dyn EgressPolicy>>,
    delivery_tx: mpsc::Sender<Delivery>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut t0_rx: mpsc::Receiver<oneshot::Sender<()>>,
) -> ServerStats {
    let start = Instant::now();
    let mut tx = SrvTx {
        seam,
        egress,
        start,
    };
    let mut stats = ServerStats::default();
    // [S1] FEC-runtime (cfg.fec varsa): repair-ingest + blok-birleştirme + kurtarım-teslimi.
    let fec: Option<FecProfile> = cfg.fec.and_then(|f| by_id(f.profile_id));
    // [v1/PARAM] yerel-beklenen canonical alanlar (fail-closed doğrulama tabanı).
    let param_expected: Option<ParamFields> = fec.as_ref().map(param_fields_for);
    let mut peers: HashMap<SocketAddr, PeerSlot> = HashMap::new();
    let mut ack_timer = interval(ACK_TICK);
    let mut patience = interval(PATIENCE_TICK);
    let mut buf = vec![0u8; SERVER_RECV_BUF];
    let idle_hold_ns = u64::try_from(cfg.idle_hold.as_nanos()).unwrap_or(u64::MAX);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            // Ölçüm-t0 (ServerHandle::mark_t0): wire sayaçları sıfır, ALICI DURUMU KORUNUR.
            Some(ack) = t0_rx.recv() => {
                let now = elapsed_ns(start);
                if let Some(p) = tx.egress.as_mut() { p.arm(now); }
                stats.tx_datagrams = 0;
                stats.rx_datagrams = 0;
                stats.tx_bytes = 0;
                stats.rx_bytes = 0;
                stats.egress_refused_pkts = 0;
                stats.egress_refused_bytes = 0;
                let _ = ack.send(());
            }
            _ = ack_timer.tick() => {
                let now = elapsed_ns(start);
                // ACK_FREQ_N'e ulaşamayan kuyruğu timer teyit eder (bench birebir).
                let acks: Vec<(SocketAddr, SeamPeer, u64, Vec<u8>)> = peers
                    .iter_mut()
                    .filter_map(|(p, s)| {
                        s.rx.on_ack_timer(now)
                            .map(|a| (*p, s.seam_peer, s.next_pn(), a.encode()))
                    })
                    .collect();
                for (peer, sp, pn, plain) in acks {
                    srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, plain).await;
                }
            }
            _ = patience.tick() => {
                // Item 3: idle-peer emekliliği — tablo boşalması YALNIZ burada (sessiz-eviction yok;
                // emeklilik gözlemlenir). ACK-durumu peer'la gider; geç DATA yeni peer-slot açar.
                let now = elapsed_ns(start);
                let before = peers.len();
                peers.retain(|_, s| now.saturating_sub(s.last_seen_ns) < idle_hold_ns);
                stats.retired_idle_peers += (before - peers.len()) as u64;
            }
            r = sock.recv_from(&mut buf) => {
                let Ok((n, peer)) = r else { continue };
                stats.rx_datagrams += 1;
                stats.rx_bytes += n as u64;
                let Ok(od) = tx.seam.open(&buf[..n]) else {
                    stats.open_failures += 1;
                    continue;
                };
                let (seam_peer, plain) = (od.peer, od.plain);
                // [S1] Çerçeve ayrımı: DATA / FEC-shard. İkisi de peer-slot + motor-ACK yolundan
                // geçer (repair de seq işgal eder — RxKind::FecShard motor tarafından ACK'lenir).
                enum RxFrame<'a> {
                    Data {
                        wire_seq: u16,
                        unit_id: u64,
                        payload: &'a [u8],
                    },
                    Fec {
                        h: FecFrameHeader,
                        symbol: &'a [u8],
                    },
                    Param {
                        profile_id: u8,
                        code_suite_id: u8,
                        param_hash: [u8; 32],
                    },
                }
                let parsed = if let Some((wire_seq, unit_id, payload)) = decode_data(&plain) {
                    RxFrame::Data {
                        wire_seq,
                        unit_id,
                        payload,
                    }
                } else if let Some((h, symbol)) = decode_fec(&plain) {
                    RxFrame::Fec { h, symbol }
                } else if let Some((profile_id, code_suite_id, param_hash)) = decode_param(&plain) {
                    RxFrame::Param {
                        profile_id,
                        code_suite_id,
                        param_hash,
                    }
                } else {
                    stats.unknown_frames += 1; // kontrol-düzlemi dal-noktası (item 6 / B-hattı).
                    continue;
                };
                let now = elapsed_ns(start);
                // Peer-cap FAIL-CLOSED (bench'in sınırsız tablosu SR-AUDIT ihlaliydi — D6):
                // tablo doluyken YENİ peer typed-drop; mevcut peer'lar etkilenmez.
                if !peers.contains_key(&peer) {
                    if peers.len() >= cfg.max_peers {
                        stats.dropped_new_peers += 1;
                        continue;
                    }
                    let mut rx = Receiver::new();
                    rx.set_quota_r(cfg.quota_r);
                    peers.insert(
                        peer,
                        PeerSlot {
                            rx,
                            last_seen_ns: now,
                            delivered_seen: std::collections::BTreeSet::new(),
                            blocks: BTreeMap::new(),
                            seam_peer,
                            tx_pn: 0,
                            param: ParamState::new(),
                            pre_param_fec: Vec::new(),
                        },
                    );
                }
                let Some(slot) = peers.get_mut(&peer) else { continue };
                slot.last_seen_ns = now;
                match parsed {
                    RxFrame::Data {
                        wire_seq,
                        unit_id,
                        payload,
                    } => {
                        let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                        let full = resolve_seq(expected, wire_seq);
                        let out = slot.rx.on_datagram(
                            RxDatagram {
                                seq: full,
                                unit_id,
                                kind: RxKind::Data,
                            },
                            now,
                        );
                        // İlk-teslim: seq-yeni VE birim-yeni (dedup penceresi) ise app'e akar.
                        if out.delivered.contains(&unit_id) && slot.delivered_seen.insert(unit_id) {
                            if slot.delivered_seen.len() > DELIVERED_DEDUP_WINDOW {
                                slot.delivered_seen.pop_first();
                            }
                            let d = Delivery {
                                peer,
                                unit_id,
                                payload: payload.to_vec(),
                            };
                            if delivery_tx.try_send(d).is_ok() {
                                stats.delivered += 1;
                            } else {
                                stats.delivery_dropped += 1; // bounded kanal; drenaj sözleşmesi.
                            }
                        }
                        if let Some(ack) = out.ack {
                            let (sp, pn) = (slot.seam_peer, slot.next_pn());
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, ack.encode())
                                .await;
                        }
                        // [S1] Source-symbol türetimi → blok-birleştirici (TX ile AYNI deterministik
                        // SourcePrefix; block/esi unit_id'den türer — iki uç hizalı).
                        // [v1/PARAM] Source-türetimi param-GATE'SİZ: profil YEREL cfg'den (validate_fec
                        // garantili), admission-semantiği yok, bounded (RX_BLOCKS_CAP) — bind'den önce
                        // gelen DATA'ların symbol'leri kaybolursa blok repair-only kalır ve ASLA
                        // tamamlanamazdı (ilk koşuda yakalanan yarış). Fail-closed gate YALNIZ
                        // FEC-shard ingest'inde (mismatch/saldırı yüzeyi orası).
                        if let Some(p) = fec.as_ref() {
                            let block_id = (unit_id / u64::from(p.k_shape)) as u32;
                            let esi = (unit_id % u64::from(p.k_shape)) as u16;
                            let mut sym = vec![0u8; p.e_usize()];
                            let pfx = SourcePrefix {
                                message_id: (unit_id & 0xFFFF_FFFF) as u32,
                                chunk_offset: 0,
                                actual_payload_len: payload.len() as u16,
                                source_flags: 0,
                            };
                            let _ = pfx.serialize_symbol(payload, &mut sym);
                            let k = u8::try_from(p.k_shape).unwrap_or(255);
                            let rec = fec_ingest(
                                &mut stats,
                                &delivery_tx,
                                peer,
                                slot,
                                p,
                                block_id,
                                k,
                                esi,
                                sym,
                                false,
                            );
                            if !rec.is_empty() {
                                stats.fec_recovered_ctrl_tx += 1;
                                let f = encode_recovered(block_id, &rec);
                                let (sp, pn) = (slot.seam_peer, slot.next_pn());
                                srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
                            }
                        }
                    }
                    RxFrame::Fec { h, symbol } => {
                        let Some(p) = fec.as_ref() else {
                            stats.unknown_frames += 1; // FEC kapalı — çerçeve işlenemez, sayılır.
                            continue;
                        };
                        // [v1/PARAM] fail-closed AMA bounded-HOLD: bind-öncesi shard decode/teslim
                        // ALMAZ, bekletilir; bind'de drain edilir. (Düşürme yarış-duyarlıydı:
                        // repair yeniden-üretilmediğinden kurtarım kalıcı kaybolurdu — CI'da yakalandı.)
                        if !slot.param.is_confirmed() {
                            if slot.pre_param_fec.len() < PRE_PARAM_FEC_CAP {
                                slot.pre_param_fec.push((h, symbol.to_vec()));
                            } else {
                                stats.fec_pre_param_dropped += 1; // taşan düşer (bounded).
                            }
                            // Motor-ACK'i yine verilir (seq işgal edildi; kayıp-sinyali üretme).
                            let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                            let full = resolve_seq(expected, h.seq);
                            let is_repair = h.kind == inner_kind::FEC_REPAIR;
                            let out = slot.rx.on_datagram(
                                RxDatagram {
                                    seq: full,
                                    unit_id: u64::from(h.esi),
                                    kind: RxKind::FecShard {
                                        block_id: h.block_id,
                                        esi: h.esi,
                                        k_actual: h.k_actual,
                                        is_repair,
                                    },
                                },
                                now,
                            );
                            if let Some(ack) = out.ack {
                                let (sp, pn) = (slot.seam_peer, slot.next_pn());
                                srv_seal_send(
                                    &mut tx, &sock, &mut stats, peer, sp, pn,
                                    ack.encode(),
                                )
                                .await;
                            }
                            continue;
                        }
                        if h.profile_id != p.profile_id || symbol.len() != p.e_usize() {
                            stats.unknown_frames += 1; // profil/symbol uyumsuz — fail-closed.
                            continue;
                        }
                        stats.fec_repairs_rx += 1;
                        let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                        let full = resolve_seq(expected, h.seq);
                        let is_repair = h.kind == inner_kind::FEC_REPAIR;
                        let out = slot.rx.on_datagram(
                            RxDatagram {
                                seq: full,
                                unit_id: u64::from(h.esi),
                                kind: RxKind::FecShard {
                                    block_id: h.block_id,
                                    esi: h.esi,
                                    k_actual: h.k_actual,
                                    is_repair,
                                },
                            },
                            now,
                        );
                        if let Some(ack) = out.ack {
                            let (sp, pn) = (slot.seam_peer, slot.next_pn());
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, ack.encode())
                                .await;
                        }
                        let rec = fec_ingest(
                            &mut stats,
                            &delivery_tx,
                            peer,
                            slot,
                            p,
                            h.block_id,
                            h.k_actual,
                            h.esi,
                            symbol.to_vec(),
                            is_repair,
                        );
                        if !rec.is_empty() {
                            stats.fec_recovered_ctrl_tx += 1;
                            let f = encode_recovered(h.block_id, &rec);
                            let (sp, pn) = (slot.seam_peer, slot.next_pn());
                            srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
                        }
                    }
                    RxFrame::Param {
                        profile_id,
                        code_suite_id,
                        param_hash,
                    } => {
                        // [v1/PARAM] §8 fail-closed doğrulama: yerel-beklenen alanlarla hash-verify
                        // → bind (admission açılır); mismatch → red-sayaç, ACK YOK.
                        let Some(exp) = param_expected.as_ref() else {
                            stats.unknown_frames += 1; // FEC kapalı uçta PARAM işlenmez.
                            continue;
                        };
                        let ctl = Control::ParamSet {
                            id: 0,
                            param_hash,
                            profile_id,
                            code_suite_id,
                        };
                        let was = slot.param.is_confirmed();
                        match slot.param.on_param_set(&ctl, exp) {
                            Ok(()) => {
                                if !was {
                                    stats.param_bound_peers += 1;
                                }
                                let f = encode_param_ack(&param_hash);
                                let (sp, pn) = (slot.seam_peer, slot.next_pn());
                                srv_seal_send(&mut tx, &sock, &mut stats, peer, sp, pn, f).await;
                                // Bind: bekletilen shard'lar drain → normal ingest (kurtarımlar
                                // RECOVERED ile bildirilir).
                                if let Some(p) = fec.as_ref() {
                                    for (fh, sym) in std::mem::take(&mut slot.pre_param_fec) {
                                        let is_rep = fh.kind == inner_kind::FEC_REPAIR;
                                        let rec = fec_ingest(
                                            &mut stats,
                                            &delivery_tx,
                                            peer,
                                            slot,
                                            p,
                                            fh.block_id,
                                            fh.k_actual,
                                            fh.esi,
                                            sym,
                                            is_rep,
                                        );
                                        if !rec.is_empty() {
                                            stats.fec_recovered_ctrl_tx += 1;
                                            let f2 = encode_recovered(fh.block_id, &rec);
                                            let (sp2, pn2) = (slot.seam_peer, slot.next_pn());
                                            srv_seal_send(
                                                &mut tx, &sock, &mut stats, peer, sp2, pn2, f2,
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                            Err(_) => stats.param_rejects += 1,
                        }
                    }
                }
            }
        }
    }
    stats
}
