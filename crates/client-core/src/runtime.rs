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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Timer, TransmitKind};
use adaptive_transport::seq::resolve_seq;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::seam::SecuritySeam;
use crate::wire::{decode_data, encode_data, HDR_DATA};
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
        }
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
    /// Payload'ı store'da bulunamayan (çoktan ACK'lenmiş birime geç karar) emisyon — kayıp sayıldı.
    pub stale_decisions: u64,
    /// Soket send hatası (kanal kaybı eşdeğeri, INV-RATE-1).
    pub send_errors: u64,
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
                seq_to_unit.insert(d.seq, d.unit_id);
                unit_seqs.entry(d.unit_id).or_default().push(d.seq);
                max_emitted_seq = max_emitted_seq.max(d.seq);
                if !matches!(
                    d.kind,
                    TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
                ) {
                    // FecParity: FEC Manual-off (Q18); CsProbe: bootstrap OFF. Sevk default'larında
                    // üretilmezler; üretilirse (ileri konfig) shard/probe byte'ları güvenlik
                    // düzleminden gelir — v0 kabuk bunları taşımaz, kayıp sayar (typed genişleme P2).
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
                if !plain.is_empty()
                    && (plain[0] == FRAME_TYPE_ACK || plain[0] == FRAME_TYPE_ACK_V2)
                {
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

/// Eş yuvası: alıcı motoru + idle-emeklilik çapası (item 3) + teslim-dedup penceresi.
struct PeerSlot {
    rx: Receiver,
    last_seen_ns: u64,
    delivered_seen: std::collections::BTreeSet<u64>,
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
                let Some((wire_seq, unit_id, payload)) = decode_data(&plain) else { continue };
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
                        },
                    );
                }
                let Some(slot) = peers.get_mut(&peer) else { continue };
                slot.last_seen_ns = now;
                let expected = slot.rx.largest_seq().map_or(0, |m| m + 1);
                let full = resolve_seq(expected, wire_seq);
                let out = slot.rx.on_datagram(RxDatagram { seq: full, unit_id, kind: RxKind::Data }, now);
                // İlk-teslim: seq-yeni VE birim-yeni (dedup penceresi) ise payload app'e akar.
                if out.delivered.contains(&unit_id) && slot.delivered_seen.insert(unit_id) {
                    if slot.delivered_seen.len() > DELIVERED_DEDUP_WINDOW {
                        slot.delivered_seen.pop_first();
                    }
                    let d = Delivery { peer, unit_id, payload: payload.to_vec() };
                    if delivery_tx.try_send(d).is_ok() {
                        stats.delivered += 1;
                    } else {
                        stats.delivery_dropped += 1; // bounded kanal; app drenaj sözleşmesi.
                    }
                }
                if let Some(ack) = out.ack {
                    let _ = sock.send_to(&seam.seal(ack.encode()), peer).await;
                }
            }
        }
    }
    stats
}
