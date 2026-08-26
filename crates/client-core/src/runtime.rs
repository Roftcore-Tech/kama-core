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
use std::sync::Arc;
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, FecActuationMode, OpaqueUnit, SrLogicalRef, Timer, TransmitKind,
};
use adaptive_transport::seq::resolve_seq;
use kama_fec_codec::{
    by_id, FecCodec, FecProfile, ReceivedSymbol, RepairSymbol, RsSimdCodec, SourceSymbol,
};
use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::{inner_kind, SOURCE_PREFIX_LEN};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::seam::SecuritySeam;
use crate::wire::{
    decode_data, decode_fec, encode_data, encode_fec, FecFrameHeader, HDR_DATA, HDR_FEC,
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
    offer_tx: mpsc::Sender<Vec<u8>>,
    acked_rx: mpsc::Receiver<u64>,
    shutdown_tx: watch::Sender<bool>,
    task: JoinHandle<ClientStats>,
    max_unit_bytes: usize,
}

impl ClientEndpoint {
    /// Bağlan ve görevi başlat. `seam` = güvenlik dikişi (stub: [`crate::seam::IdentitySeam`]).
    pub async fn connect(
        server: SocketAddr,
        cfg: RuntimeConfig,
        seam: Arc<dyn SecuritySeam>,
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
        let max_unit_bytes = cfg.max_unit_bytes;
        let task = tokio::spawn(client_task(
            sock,
            cfg,
            seam,
            offer_rx,
            acked_tx,
            shutdown_rx,
        ));
        Ok(ClientHandle {
            offer_tx,
            acked_rx,
            shutdown_tx,
            task,
            max_unit_bytes,
        })
    }
}

impl ClientHandle {
    /// Bir birim sun (non-blocking; bounded). Dolu kuyruk → typed `QueueFull` (transport bloklamaz).
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
async fn client_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    seam: Arc<dyn SecuritySeam>,
    mut offer_rx: mpsc::Receiver<Vec<u8>>,
    acked_tx: mpsc::Sender<u64>,
    mut shutdown_rx: watch::Receiver<bool>,
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
                            let wire = seam.seal(frame);
                            stats.tx_datagrams += 1;
                            if sock.send(&wire).await.is_err() {
                                stats.send_errors += 1;
                            }
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
                let wire = seam.seal(frame);
                stats.tx_datagrams += 1;
                if sock.send(&wire).await.is_err() {
                    stats.send_errors += 1; // kayıp; asla geri itme (INV-RATE-1).
                }
            }
        };
    }
    // Gelen datagram: seam.open → ACK (V1 0x10 / V2 0x14) → ilk-ACK damgası + motor beslemesi.
    macro_rules! handle_rx {
        ($raw:expr, $now:expr) => {
            stats.rx_datagrams += 1;
            if let Some(plain) = seam.open($raw) {
                if plain.is_empty()
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
            // Offer YALNIZ store'da yer varken alınır (bounds-before-alloc: kanal + store çift sınır).
            maybe = offer_rx.recv(), if store.len() < cfg.max_inflight_units => {
                let Some(payload) = maybe else { break }; // tüm handle'lar düştü → drain'e geç.
                let id = next_unit;
                next_unit += 1;
                stats.offered += 1;
                let size = (HDR_DATA + payload.len() + seam.overhead()) as u64;
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
                        let parity_wire = (HDR_FEC + e + seam.overhead()) as u64;
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
            _ = rtprop.tick() => { let now = elapsed_ns(start); drive!(Event::TimerFired(Timer::RtpropProbe), now); }
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
    task: JoinHandle<ServerStats>,
    local_addr: SocketAddr,
}

impl ServerEndpoint {
    /// Bağla ve görevi başlat.
    pub async fn bind(
        bind: SocketAddr,
        cfg: RuntimeConfig,
        seam: Arc<dyn SecuritySeam>,
    ) -> Result<ServerHandle, RuntimeError> {
        validate_fec(&cfg)?; // [S1] koşu-öncesi typed FEC-config doğrulaması
        let sock = UdpSocket::bind(bind).await?;
        let local_addr = sock.local_addr()?;
        let (delivery_tx, delivery_rx) = mpsc::channel(cfg.delivery_queue);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(server_task(sock, cfg, seam, delivery_tx, shutdown_rx));
        Ok(ServerHandle {
            delivery_rx,
            shutdown_tx,
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
) {
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
}

/// Server görevi: bench `serve_loop` şekli + bounded peers + idle-emeklilik + teslim akışı.
async fn server_task(
    sock: UdpSocket,
    cfg: RuntimeConfig,
    seam: Arc<dyn SecuritySeam>,
    delivery_tx: mpsc::Sender<Delivery>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> ServerStats {
    let start = Instant::now();
    let mut stats = ServerStats::default();
    // [S1] FEC-runtime (cfg.fec varsa): repair-ingest + blok-birleştirme + kurtarım-teslimi.
    let fec: Option<FecProfile> = cfg.fec.and_then(|f| by_id(f.profile_id));
    let mut peers: HashMap<SocketAddr, PeerSlot> = HashMap::new();
    let mut ack_timer = interval(ACK_TICK);
    let mut patience = interval(PATIENCE_TICK);
    let mut buf = vec![0u8; SERVER_RECV_BUF];
    let idle_hold_ns = u64::try_from(cfg.idle_hold.as_nanos()).unwrap_or(u64::MAX);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            _ = ack_timer.tick() => {
                let now = elapsed_ns(start);
                // ACK_FREQ_N'e ulaşamayan kuyruğu timer teyit eder (bench birebir).
                let acks: Vec<(SocketAddr, Vec<u8>)> = peers
                    .iter_mut()
                    .filter_map(|(p, s)| s.rx.on_ack_timer(now).map(|a| (*p, a.encode())))
                    .collect();
                for (peer, plain) in acks {
                    let _ = sock.send_to(&seam.seal(plain), peer).await; // hata = kayıp.
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
                let Some(plain) = seam.open(&buf[..n]) else {
                    stats.open_failures += 1;
                    continue;
                };
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
                }
                let parsed = if let Some((wire_seq, unit_id, payload)) = decode_data(&plain) {
                    RxFrame::Data {
                        wire_seq,
                        unit_id,
                        payload,
                    }
                } else if let Some((h, symbol)) = decode_fec(&plain) {
                    RxFrame::Fec { h, symbol }
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
                            let _ = sock.send_to(&seam.seal(ack.encode()), peer).await;
                        }
                        // [S1] Source-symbol türetimi → blok-birleştirici (TX ile AYNI deterministik
                        // SourcePrefix; block/esi unit_id'den türer — iki uç hizalı).
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
                            fec_ingest(
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
                        }
                    }
                    RxFrame::Fec { h, symbol } => {
                        let Some(p) = fec.as_ref() else {
                            stats.unknown_frames += 1; // FEC kapalı — çerçeve işlenemez, sayılır.
                            continue;
                        };
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
                            let _ = sock.send_to(&seam.seal(ack.encode()), peer).await;
                        }
                        fec_ingest(
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
                    }
                }
            }
        }
    }
    stats
}
