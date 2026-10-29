//! [KTP2-F0 / Ajan D] KARIŞIK AKIŞ KIYASI — dosya + TEXT + PTT eşzamanlı, TAŞIMA DÜZEYİNDE.
//!
//! Yeni, seçmeli komutlar (`mixed-serve` / `mixed-client`); `serve`/`client` yollarına DOKUNMAZ.
//! Amaç: aynı uygulama iş yükünü üç taşıma üstünde koşup akış izolasyonunu göstermek (K-1 benzeri
//! tek hücre). Ürün uygulama yolu (kama-app, S1 adaptörü) bu ölçümün KAPSAMI DIŞINDADIR.
//!
//! ## Kollar
//! * `kama-product` — tek oturum, ürün runtime'ı (`ClientEndpoint`/`ServerEndpoint`, bayraksız
//!   ürün profili, `IdentitySeam`). Öğeler planlı anlarında TEK sunum kuyruğuna verilir (motorun
//!   yeni-veri kuyruğu FIFO'dur; uygulama önceliği yoktur). Teslim =
//!   `ServerEndpoint::next_delivery` (motorun SIRASIZ teslimi). client-core S1 adaptörünün
//!   oturum-geneli `Reorder` sıralaması bu yolda YOKTUR → bu kol ürün uygulama yolunun HOL'unu
//!   ÖLÇMEZ (Ajan A kapsamı).
//! * `quinn` — akış başına bağımsız TEK-YÖNLÜ stream (dosya · TEXT · PTT), stream önceliği
//!   PTT 2 > TEXT 1 > dosya 0 (`SendStream::set_priority`). `--quinn-per-item`: TEXT ve PTT
//!   öğelerinin HER BİRİ ayrı stream (akış içi HOL de kalkar). Yapılandırma `serve/client
//!   --rival-delivery transport-ack` koluyla aynıdır (uni-stream kredisi 512, idle 300 s).
//! * `tcp` — TEK bağlantı. Uygulama yazıcısı öncelikli (PTT > TEXT > dosya; her seferinde tek
//!   çerçeve). Çekirdek gönderim tamponundaki bayt ve bayt-akışı sırası (alıcıda HOL) TCP'nin
//!   doğal davranışıdır; normalize EDİLMEZ.
//!
//! ## Ölçüm
//! * Öğe = `bytes` uzunluğunda uygulama yükü: `flow u8 ‖ id u64 ‖ dolgu`. Bayt akışlarında (tcp,
//!   quinn stream) öğe başına ek `len u32` çerçevesi vardır (taşıma uyarlaması).
//! * Sunum anı = PLAN (`t0` + öğenin ofseti), CLOCK_REALTIME ns; gönderen tarafı bekleme gecikmeye
//!   DAHİLDİR. Teslim anı = ALICI uygulamaya ulaştığı an (CLOCK_REALTIME ns, `--rx-log`). İki süreç
//!   aynı makinede ve aynı saatte (ağ ad alanı saati ayırmaz).
//! * `t0` = bağlantı kurulduktan SONRA (kurulum `setup_ns` olarak ayrı; quinn el sıkışması ve tcp
//!   `connect` pencere dışında — `serve/client` kollarıyla aynı kural).
//! * İstemci çıktısı (`--out`): 1. satır `{"mixed_meta":{…}}`, sonra öğe başına `{"tx":{…}}`.
//!   Sunucu (`--rx-log`): öğe başına `{"rx":{…}}` (satır satır, tamponsuz — `kill` güvenli).
//!   Birleştirme ve metrikler: `scripts/ktp2/f0/d/karisik_analiz.py`.

use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::{Args, ValueEnum};
use client_core::runtime::{ClientEndpoint, ClientSender, RuntimeError, ServerEndpoint};
use client_core::seam::IdentitySeam;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{sleep, sleep_until, timeout, Instant};

use crate::budget::BudgetMode;
use crate::error::BenchError;
use crate::transport::Cc;

/// Öğe başlığı: `flow u8 ‖ id u64`.
const HDR: usize = 9;
/// Bayt akışı çerçevesi: `len u32`.
const LEN: usize = 4;
/// Öğe üst sınırı (bounds-before-alloc, SR-AUDIT-001).
const MAX_ITEM: usize = 1 << 20;
/// Sunum kuyruğu doluyken yeniden deneme (kama_product.rs ile aynı sözleşme).
const OFFER_RETRY: Duration = Duration::from_millis(1);
/// Çıktı şeması.
const MIXED_SCHEMA: &str = "kama-bench-mixed/v1";

/// Karışık-akış taşıması.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MixTransport {
    /// Ürün runtime'ı, tek oturum.
    KamaProduct,
    /// quinn, akış başına bağımsız stream.
    Quinn,
    /// Tek TCP bağlantısı.
    Tcp,
}

impl MixTransport {
    fn label(self) -> &'static str {
        match self {
            Self::KamaProduct => "kama-product",
            Self::Quinn => "quinn",
            Self::Tcp => "tcp",
        }
    }
}

/// quinn tıkanıklık denetimi (karışık kol).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MixCc {
    /// quinn Cubic.
    Cubic,
    /// quinn BBR.
    Bbr,
}

impl From<MixCc> for Cc {
    fn from(c: MixCc) -> Cc {
        match c {
            MixCc::Cubic => Cc::Cubic,
            MixCc::Bbr => Cc::Bbr,
        }
    }
}

/// Uygulama akışı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Ptt,
    Text,
    File,
}

impl Flow {
    fn code(self) -> u8 {
        match self {
            Self::Ptt => 0,
            Self::Text => 1,
            Self::File => 2,
        }
    }

    fn from_code(c: u8) -> Option<Self> {
        match c {
            0 => Some(Self::Ptt),
            1 => Some(Self::Text),
            2 => Some(Self::File),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ptt => "ptt",
            Self::Text => "text",
            Self::File => "file",
        }
    }

    /// Öncelik (yüksek önce): quinn stream önceliği ve tcp/kama aynı-an sıralaması.
    fn priority(self) -> i32 {
        match self {
            Self::Ptt => 2,
            Self::Text => 1,
            Self::File => 0,
        }
    }
}

/// Plandaki tek öğe.
#[derive(Debug, Clone, Copy)]
struct Item {
    flow: Flow,
    id: u64,
    bytes: usize,
    /// `t0`'dan ofset (ns).
    at_ns: u64,
}

/// `mixed-serve` argümanları.
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Bağlanma adresi.
    #[arg(long)]
    pub bind: SocketAddr,
    /// Taşıma.
    #[arg(long, value_enum)]
    pub transport: MixTransport,
    /// quinn CC (yalnız quinn).
    #[arg(long, value_enum, default_value_t = MixCc::Bbr)]
    pub cc: MixCc,
    /// kama-product alıcı mesaj tavanı = en büyük öğe (İSTEMCİYLE AYNI).
    #[arg(long, default_value_t = 1024)]
    pub payload: u64,
    /// Alım günlüğü (JSONL, öğe başına bir satır).
    #[arg(long)]
    pub rx_log: PathBuf,
}

/// `mixed-client` argümanları.
#[derive(Debug, Args)]
pub struct ClientArgs {
    /// Sunucu adresi.
    #[arg(long)]
    pub server: SocketAddr,
    /// Taşıma.
    #[arg(long, value_enum)]
    pub transport: MixTransport,
    /// quinn CC (yalnız quinn).
    #[arg(long, value_enum, default_value_t = MixCc::Bbr)]
    pub cc: MixCc,
    /// kama-product mesaj tavanı = en büyük öğe (SUNUCUYLA AYNI).
    #[arg(long, default_value_t = 1024)]
    pub payload: u64,
    /// Dosya boyu (bayt; 0 = dosyasız kontrol kolu).
    #[arg(long, default_value_t = 0)]
    pub file_bytes: u64,
    /// Dosya parçası (bayt).
    #[arg(long, default_value_t = 1024)]
    pub file_chunk: u64,
    /// Dosya sunum hızı (bayt/s; 0 = bütün dosya t0'da).
    #[arg(long, default_value_t = 0)]
    pub file_rate_bps: u64,
    /// TEXT öğe boyu (bayt; başlık dahil).
    #[arg(long, default_value_t = 65)]
    pub text_bytes: u64,
    /// TEXT öğe sayısı.
    #[arg(long, default_value_t = 15)]
    pub text_count: u64,
    /// TEXT aralığı (ms).
    #[arg(long, default_value_t = 2000)]
    pub text_interval_ms: u64,
    /// PTT çerçeve boyu (bayt; başlık dahil — 30 B kodlayıcı verisi + 9 B başlık).
    #[arg(long, default_value_t = 39)]
    pub ptt_bytes: u64,
    /// PTT çerçeve sayısı.
    #[arg(long, default_value_t = 150)]
    pub ptt_count: u64,
    /// PTT demet aralığı (ms).
    #[arg(long, default_value_t = 200)]
    pub ptt_interval_ms: u64,
    /// Son plandan sonra bekleme (s).
    #[arg(long, default_value_t = 20)]
    pub tail_s: u64,
    /// quinn el sıkışma / tcp bağlanma tavanı (s).
    #[arg(long, default_value_t = 60)]
    pub handshake_timeout_s: u64,
    /// quinn: TEXT ve PTT öğelerinin her biri ayrı stream.
    #[arg(long, default_value_t = false)]
    pub quinn_per_item: bool,
    /// Profil etiketi (hücre adı).
    #[arg(long, default_value = "mixed")]
    pub profile: String,
    /// Tekrar numarası.
    #[arg(long, default_value_t = 0)]
    pub repeat: u32,
    /// İleri yol bozulması (netem dizesi; aynen kaydedilir).
    #[arg(long, default_value = "")]
    pub fwd_impair: String,
    /// Dönüş yolu bozulması (netem dizesi; aynen kaydedilir).
    #[arg(long, default_value = "")]
    pub rev_impair: String,
    /// İstemci çıktısı (JSONL).
    #[arg(long)]
    pub out: PathBuf,
}

fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

fn dur_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

fn rt_err(what: &str, e: RuntimeError) -> BenchError {
    BenchError::Other(format!("mixed kama-product {what}: {e}"))
}

/// Sunum planı: öğeler ofset sırasında; aynı ofsette öncelik sırası (PTT > TEXT > dosya).
fn plan(a: &ClientArgs) -> Result<Vec<Item>, BenchError> {
    for (name, b) in [
        ("--ptt-bytes", a.ptt_bytes),
        ("--text-bytes", a.text_bytes),
        ("--file-chunk", a.file_chunk),
    ] {
        if b < HDR as u64 || b > MAX_ITEM as u64 {
            return Err(BenchError::Other(format!(
                "{name}={b}: öğe boyu {HDR}..={MAX_ITEM} aralığında olmalı (başlık {HDR} B)"
            )));
        }
    }
    let as_usize = |v: u64| usize::try_from(v).unwrap_or(MAX_ITEM);
    let mut items = Vec::new();
    let mut id: u64 = 0;
    if a.file_bytes > 0 {
        let chunk = a.file_chunk;
        for i in 0..a.file_bytes.div_ceil(chunk) {
            let rem = a.file_bytes - i * chunk;
            let bytes = rem.min(chunk).max(HDR as u64);
            let at_ns = if a.file_rate_bps == 0 {
                0
            } else {
                i.saturating_mul(chunk).saturating_mul(1_000_000_000) / a.file_rate_bps
            };
            items.push(Item {
                flow: Flow::File,
                id,
                bytes: as_usize(bytes),
                at_ns,
            });
            id += 1;
        }
    }
    for (flow, count, bytes, interval_ms) in [
        (Flow::Text, a.text_count, a.text_bytes, a.text_interval_ms),
        (Flow::Ptt, a.ptt_count, a.ptt_bytes, a.ptt_interval_ms),
    ] {
        for i in 0..count {
            items.push(Item {
                flow,
                id,
                bytes: as_usize(bytes),
                at_ns: i.saturating_mul(interval_ms).saturating_mul(1_000_000),
            });
            id += 1;
        }
    }
    items.sort_by_key(|it| (it.at_ns, std::cmp::Reverse(it.flow.priority()), it.id));
    Ok(items)
}

fn last_at(items: &[Item]) -> u64 {
    items.iter().map(|it| it.at_ns).max().unwrap_or(0)
}

/// Uygulama yükü: `flow ‖ id ‖ dolgu`.
fn encode(it: &Item) -> Vec<u8> {
    let mut v = vec![0x5Au8; it.bytes.max(HDR)];
    v[0] = it.flow.code();
    v[1..HDR].copy_from_slice(&it.id.to_be_bytes());
    v
}

/// Bayt akışı çerçevesi: `len u32 ‖ yük`.
fn framed(it: &Item) -> Vec<u8> {
    let body = encode(it);
    let mut v = Vec::with_capacity(LEN + body.len());
    v.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes());
    v.extend_from_slice(&body);
    v
}

fn decode(b: &[u8]) -> Option<(Flow, u64)> {
    let flow = Flow::from_code(*b.first()?)?;
    let id = u64::from_be_bytes(b.get(1..HDR)?.try_into().ok()?);
    Some((flow, id))
}

#[derive(Serialize)]
struct RxRec {
    flow: &'static str,
    id: u64,
    bytes: usize,
    rx_unix_ns: u64,
}

#[derive(Serialize)]
struct RxLine {
    rx: RxRec,
}

/// Alım günlüğü: satır satır, tamponsuz (sunucu `kill` edilse de yazılan satır kalır).
struct RxLog {
    file: Mutex<std::fs::File>,
    count: AtomicU64,
}

impl RxLog {
    fn create(p: &Path) -> Result<Self, BenchError> {
        Ok(Self {
            file: Mutex::new(std::fs::File::create(p)?),
            count: AtomicU64::new(0),
        })
    }

    /// Damga ÖNCE alınır (kilit beklemesi teslim anına eklenmez).
    fn record(&self, item: &[u8]) {
        let rx_unix_ns = unix_ns();
        let Some((flow, id)) = decode(item) else {
            return;
        };
        let rec = RxLine {
            rx: RxRec {
                flow: flow.label(),
                id,
                bytes: item.len(),
                rx_unix_ns,
            },
        };
        let mut line = serde_json::to_string(&rec).unwrap_or_default();
        line.push('\n');
        if let Ok(mut f) = self.file.lock() {
            if f.write_all(line.as_bytes()).is_ok() {
                self.count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// quinn tarafı: `transport-ack` koluyla aynı yapılandırma (uni-stream kredisi 512).
fn set_quinn_uni_tuning() {
    crate::rival::set(crate::rival::RivalTuning {
        delivery: crate::rival::Delivery::TransportAck,
        ..crate::rival::RivalTuning::default()
    });
}

// ───────────────────────────────────── SUNUCU ─────────────────────────────────────

/// `mixed-serve`: teslimleri alım günlüğüne yazar; süreç `kill` edilene dek çalışır.
///
/// # Errors
/// Günlük dosyası, bağlanma ya da ürün runtime hatası.
pub async fn serve(a: ServeArgs) -> Result<(), BenchError> {
    let log = Arc::new(RxLog::create(&a.rx_log)?);
    match a.transport {
        MixTransport::KamaProduct => serve_kama(&a, &log).await,
        MixTransport::Quinn => {
            set_quinn_uni_tuning();
            serve_quinn(&a, log).await
        }
        MixTransport::Tcp => serve_tcp(&a, log).await,
    }
}

async fn serve_kama(a: &ServeArgs, log: &RxLog) -> Result<(), BenchError> {
    let (cfg, _delta) = crate::kama_product::canonical_cfg_for_mixed("mixed-server", a.payload);
    let mut srv =
        ServerEndpoint::bind_with_policy(a.bind, cfg, Box::new(IdentitySeam::new()), None)
            .await
            .map_err(|e| rt_err("bind", e))?;
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    loop {
        tokio::select! {
            _ = sigterm.recv() => break,
            _ = sigint.recv() => break,
            d = srv.next_delivery() => match d {
                Some(d) => log.record(&d.payload),
                None => break,
            },
        }
    }
    let stats = srv.shutdown().await;
    eprintln!(
        "MIXED-SERVER transport=kama-product rx_items={} stats={stats:?}",
        log.count()
    );
    Ok(())
}

async fn serve_quinn(a: &ServeArgs, log: Arc<RxLog>) -> Result<(), BenchError> {
    let sc = crate::quic::server_config(a.cc.into())?;
    let (endpoint, _gov) = crate::quic::build_endpoint(a.bind, Some(sc), BudgetMode::Unlimited)?;
    while let Some(incoming) = endpoint.accept().await {
        let Ok(conn) = incoming.await else { continue };
        let log = Arc::clone(&log);
        tokio::spawn(async move {
            while let Ok(recv) = conn.accept_uni().await {
                tokio::spawn(read_frames_quinn(recv, Arc::clone(&log)));
            }
        });
    }
    Ok(())
}

async fn read_frames_quinn(mut recv: quinn::RecvStream, log: Arc<RxLog>) {
    loop {
        let mut lb = [0u8; LEN];
        if recv.read_exact(&mut lb).await.is_err() {
            return;
        }
        let len = usize::try_from(u32::from_be_bytes(lb)).unwrap_or(usize::MAX);
        if !(HDR..=MAX_ITEM).contains(&len) {
            return;
        }
        let mut body = vec![0u8; len];
        if recv.read_exact(&mut body).await.is_err() {
            return;
        }
        log.record(&body);
    }
}

async fn serve_tcp(a: &ServeArgs, log: Arc<RxLog>) -> Result<(), BenchError> {
    let listener = TcpListener::bind(a.bind).await?;
    loop {
        let Ok((sock, _peer)) = listener.accept().await else {
            continue;
        };
        sock.set_nodelay(true).ok();
        tokio::spawn(read_frames_tcp(sock, Arc::clone(&log)));
    }
}

async fn read_frames_tcp(mut sock: TcpStream, log: Arc<RxLog>) {
    loop {
        let mut lb = [0u8; LEN];
        if sock.read_exact(&mut lb).await.is_err() {
            return;
        }
        let len = usize::try_from(u32::from_be_bytes(lb)).unwrap_or(usize::MAX);
        if !(HDR..=MAX_ITEM).contains(&len) {
            return;
        }
        let mut body = vec![0u8; len];
        if sock.read_exact(&mut body).await.is_err() {
            return;
        }
        log.record(&body);
    }
}

// ───────────────────────────────────── İSTEMCİ ────────────────────────────────────

/// İstemci koşusunun özeti (çıktının meta satırına girer).
struct Outcome {
    connected: bool,
    setup_ns: u64,
    t0_unix_ns: u64,
    /// Taşımanın gözlediği gönderim baytı (quinn: UDP; kama: runtime; tcp: UYGULAMA baytı).
    tx_bytes: u64,
    rx_bytes: u64,
    note: String,
}

impl Outcome {
    fn failed(setup_ns: u64, note: &str) -> Self {
        Self {
            connected: false,
            setup_ns,
            t0_unix_ns: unix_ns(),
            tx_bytes: 0,
            rx_bytes: 0,
            note: note.to_string(),
        }
    }
}

#[derive(Serialize)]
struct MixedMeta<'a> {
    schema: &'static str,
    commit: &'static str,
    transport: &'static str,
    cc: &'static str,
    profile: &'a str,
    repeat: u32,
    connected: bool,
    setup_ns: u64,
    t0_unix_ns: u64,
    payload_cap: u64,
    file_bytes: u64,
    file_chunk: u64,
    file_rate_bps: u64,
    text_bytes: u64,
    text_count: u64,
    text_interval_ms: u64,
    ptt_bytes: u64,
    ptt_count: u64,
    ptt_interval_ms: u64,
    tail_s: u64,
    handshake_timeout_s: u64,
    quinn_per_item: bool,
    fwd_impair: &'a str,
    rev_impair: &'a str,
    tx_bytes: u64,
    rx_bytes: u64,
    note: &'a str,
}

#[derive(Serialize)]
struct MetaLine<'a> {
    mixed_meta: MixedMeta<'a>,
}

#[derive(Serialize)]
struct TxRec {
    flow: &'static str,
    id: u64,
    bytes: usize,
    sent_unix_ns: u64,
}

#[derive(Serialize)]
struct TxLine {
    tx: TxRec,
}

/// `mixed-client`: planı koşar, çıktıyı yazar.
///
/// # Errors
/// Geçersiz plan, çıktı dosyası ya da ürün runtime hatası. El sıkışma/bağlanma başarısızlığı
/// HATA DEĞİLDİR (`connected=false` ile yazılır; bütün öğeler teslim edilmemiş sayılır).
pub async fn client(a: ClientArgs) -> Result<(), BenchError> {
    let items = plan(&a)?;
    if a.transport == MixTransport::KamaProduct {
        if let Some(it) = items.iter().find(|it| it.bytes as u64 > a.payload) {
            return Err(BenchError::Other(format!(
                "öğe {} B > --payload {} (kama-product mesaj tavanı; iki uçta aynı verilmeli)",
                it.bytes, a.payload
            )));
        }
    }
    let out = match a.transport {
        MixTransport::KamaProduct => client_kama(&a, &items).await?,
        MixTransport::Quinn => {
            set_quinn_uni_tuning();
            client_quinn(&a, &items).await?
        }
        MixTransport::Tcp => client_tcp(&a, &items).await?,
    };
    eprintln!(
        "MIXED-CLIENT transport={} connected={} setup_ms={:.1} items={} tx={}B rx={}B note={}",
        a.transport.label(),
        out.connected,
        out.setup_ns as f64 / 1e6,
        items.len(),
        out.tx_bytes,
        out.rx_bytes,
        out.note
    );
    write_out(&a, &items, &out)
}

fn write_out(a: &ClientArgs, items: &[Item], o: &Outcome) -> Result<(), BenchError> {
    let meta = MetaLine {
        mixed_meta: MixedMeta {
            schema: MIXED_SCHEMA,
            commit: option_env!("KAMA_GIT_SHA").unwrap_or("unknown"),
            transport: a.transport.label(),
            cc: Cc::from(a.cc).label(),
            profile: &a.profile,
            repeat: a.repeat,
            connected: o.connected,
            setup_ns: o.setup_ns,
            t0_unix_ns: o.t0_unix_ns,
            payload_cap: a.payload,
            file_bytes: a.file_bytes,
            file_chunk: a.file_chunk,
            file_rate_bps: a.file_rate_bps,
            text_bytes: a.text_bytes,
            text_count: a.text_count,
            text_interval_ms: a.text_interval_ms,
            ptt_bytes: a.ptt_bytes,
            ptt_count: a.ptt_count,
            ptt_interval_ms: a.ptt_interval_ms,
            tail_s: a.tail_s,
            handshake_timeout_s: a.handshake_timeout_s,
            quinn_per_item: a.quinn_per_item,
            fwd_impair: &a.fwd_impair,
            rev_impair: &a.rev_impair,
            tx_bytes: o.tx_bytes,
            rx_bytes: o.rx_bytes,
            note: &o.note,
        },
    };
    // Atomik: tmp + rename (yarıda ölen istemci bozuk çıktı bırakmaz; event.rs ile aynı kural).
    let tmp = a.out.with_extension("jsonl.tmp");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        writeln!(f, "{}", serde_json::to_string(&meta)?)?;
        for it in items {
            let line = TxLine {
                tx: TxRec {
                    flow: it.flow.label(),
                    id: it.id,
                    bytes: it.bytes,
                    sent_unix_ns: o.t0_unix_ns.saturating_add(it.at_ns),
                },
            };
            writeln!(f, "{}", serde_json::to_string(&line)?)?;
        }
        f.flush()?;
    }
    std::fs::rename(&tmp, &a.out)?;
    Ok(())
}

/// Bir mesaj sunar; kuyruk doluyken bekler-yeniden-dener (kama_product.rs `offer` ile aynı).
async fn offer(tx: &ClientSender, payload: &[u8]) -> Result<(), BenchError> {
    loop {
        match tx.send_with_deadline(payload.to_vec(), None) {
            Ok(()) => return Ok(()),
            Err(RuntimeError::QueueFull) => sleep(OFFER_RETRY).await,
            Err(e) => return Err(rt_err("offer", e)),
        }
    }
}

async fn client_kama(a: &ClientArgs, items: &[Item]) -> Result<Outcome, BenchError> {
    let (cfg, delta) = crate::kama_product::canonical_cfg_for_mixed("mixed-client", a.payload);
    let setup = std::time::Instant::now();
    let mut cli =
        ClientEndpoint::connect_with_policy(a.server, cfg, Box::new(IdentitySeam::new()), None)
            .await
            .map_err(|e| rt_err("connect", e))?;
    let setup_ns = dur_ns(setup.elapsed());
    let tx = cli.sender();
    let t0 = Instant::now();
    let t0_unix_ns = unix_ns();
    let end = t0 + Duration::from_nanos(last_at(items)) + Duration::from_secs(a.tail_s);
    let total = items.len() as u64;
    let mut next = 0usize;
    let mut acked: u64 = 0;
    let end_sleep = sleep_until(end);
    tokio::pin!(end_sleep);
    while acked < total {
        let due = items.get(next).map(|it| t0 + Duration::from_nanos(it.at_ns));
        tokio::select! {
            () = &mut end_sleep => break,
            () = sleep_until(due.unwrap_or(end)), if due.is_some() => {
                // Planı gelmiş bütün öğeler (dosyanın tamamı t0'da) TEK kuyruğa, plan sırasıyla.
                let now_ns = dur_ns(t0.elapsed());
                while let Some(it) = items.get(next) {
                    if it.at_ns > now_ns {
                        break;
                    }
                    offer(&tx, &encode(it)).await?;
                    next += 1;
                }
            }
            ack = cli.acked() => {
                if ack.is_none() {
                    break; // endpoint öldü
                }
                acked += 1;
            }
        }
    }
    let stats = cli.shutdown().await;
    Ok(Outcome {
        connected: true,
        setup_ns,
        t0_unix_ns,
        tx_bytes: stats.tx_bytes,
        rx_bytes: stats.rx_bytes,
        note: format!(
            "kama-product single-session unordered-engine-delivery acked={acked}/{total} \
             offered={next} tx[data={} retx={} dup={} fec={} ctrl={}] profile_delta={delta:?}",
            stats.tx_data_bytes,
            stats.tx_retx_bytes,
            stats.tx_dup_bytes,
            stats.tx_fec_bytes,
            stats.tx_ctrl_bytes,
        ),
    })
}

async fn client_quinn(a: &ClientArgs, items: &[Item]) -> Result<Outcome, BenchError> {
    let cc: Cc = a.cc.into();
    let (mut endpoint, _gov) = crate::quic::build_endpoint(
        "[::]:0".parse().expect("valid addr"),
        None,
        BudgetMode::Unlimited,
    )?;
    endpoint.set_default_client_config(crate::quic::client_config(cc)?);
    let setup = std::time::Instant::now();
    let connecting = endpoint
        .connect(a.server, "localhost")
        .map_err(|e| BenchError::Quic(e.to_string()))?;
    let conn = match timeout(Duration::from_secs(a.handshake_timeout_s), connecting).await {
        Ok(Ok(c)) => c,
        _ => {
            return Ok(Outcome::failed(
                dur_ns(setup.elapsed()),
                "quinn handshake failed/timeout",
            ))
        }
    };
    let setup_ns = dur_ns(setup.elapsed());
    let t0 = Instant::now();
    let t0_unix_ns = unix_ns();
    let end = t0 + Duration::from_nanos(last_at(items)) + Duration::from_secs(a.tail_s);
    let mut set = JoinSet::new();
    for flow in [Flow::File, Flow::Text, Flow::Ptt] {
        let mine: Vec<Item> = items.iter().filter(|it| it.flow == flow).copied().collect();
        if mine.is_empty() {
            continue;
        }
        let per_item = a.quinn_per_item && flow != Flow::File;
        set.spawn(quinn_flow(conn.clone(), mine, t0, per_item));
    }
    {
        // Hepsi yazılıp eşçe ACK'lenince (stopped) erken biter; yoksa plan + kuyruk süresi.
        let drain = async { while set.join_next().await.is_some() {} };
        tokio::select! {
            () = sleep_until(end) => {},
            () = drain => {},
        }
    }
    set.abort_all();
    let stats = conn.stats();
    conn.close(0u32.into(), b"done");
    let _ = timeout(Duration::from_secs(3), endpoint.wait_idle()).await;
    Ok(Outcome {
        connected: true,
        setup_ns,
        t0_unix_ns,
        tx_bytes: stats.udp_tx.bytes,
        rx_bytes: stats.udp_rx.bytes,
        note: format!(
            "quinn cc={} per_item={} streams=uni prio[ptt=2,text=1,file=0] lost_packets={} \
             lost_bytes={} congestion_events={} rtt_ms={:.1} cwnd={}",
            cc.label(),
            a.quinn_per_item,
            stats.path.lost_packets,
            stats.path.lost_bytes,
            stats.path.congestion_events,
            stats.path.rtt.as_secs_f64() * 1e3,
            stats.path.cwnd,
        ),
    })
}

/// Bir akışın öğelerini planlı anlarında yazar. Akış başına tek stream (ya da öğe başına stream);
/// sonunda bütün stream'lerin eşçe ACK'lenmesini bekler (yalnız erken bitiş için; ölçüm alıcıda).
async fn quinn_flow(conn: quinn::Connection, items: Vec<Item>, t0: Instant, per_item: bool) {
    let Some(first) = items.first() else {
        return;
    };
    let prio = first.flow.priority();
    let mut shared: Option<quinn::SendStream> = None;
    let mut finished: Vec<quinn::SendStream> = Vec::new();
    for it in &items {
        sleep_until(t0 + Duration::from_nanos(it.at_ns)).await;
        let frame = framed(it);
        if per_item {
            let Ok(mut s) = conn.open_uni().await else {
                return;
            };
            let _ = s.set_priority(prio);
            if s.write_all(&frame).await.is_err() {
                return;
            }
            let _ = s.finish();
            finished.push(s);
        } else {
            if shared.is_none() {
                let Ok(s) = conn.open_uni().await else {
                    return;
                };
                let _ = s.set_priority(prio);
                shared = Some(s);
            }
            let Some(s) = shared.as_mut() else {
                return;
            };
            if s.write_all(&frame).await.is_err() {
                return;
            }
        }
    }
    if let Some(mut s) = shared {
        let _ = s.finish();
        finished.push(s);
    }
    for s in finished {
        let _ = s.stopped().await;
    }
}

async fn client_tcp(a: &ClientArgs, items: &[Item]) -> Result<Outcome, BenchError> {
    let setup = std::time::Instant::now();
    let sock = match timeout(
        Duration::from_secs(a.handshake_timeout_s),
        TcpStream::connect(a.server),
    )
    .await
    {
        Ok(Ok(s)) => s,
        _ => return Ok(Outcome::failed(dur_ns(setup.elapsed()), "tcp connect failed/timeout")),
    };
    sock.set_nodelay(true).ok();
    let setup_ns = dur_ns(setup.elapsed());
    let t0 = Instant::now();
    let t0_unix_ns = unix_ns();
    let end = t0 + Duration::from_nanos(last_at(items)) + Duration::from_secs(a.tail_s);
    let (rd, mut wr) = sock.into_split();
    let (ptx, mut prx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (ttx, mut trx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (ftx, mut frx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut producers = JoinSet::new();
    for (flow, chan) in [(Flow::Ptt, ptx), (Flow::Text, ttx), (Flow::File, ftx)] {
        let mine: Vec<Item> = items.iter().filter(|it| it.flow == flow).copied().collect();
        producers.spawn(async move {
            for it in &mine {
                sleep_until(t0 + Duration::from_nanos(it.at_ns)).await;
                if chan.send(framed(it)).is_err() {
                    return;
                }
            }
        });
    }
    // Öncelikli tek yazıcı: önce bekleyen PTT, sonra TEXT, sonra dosya; her seferinde TEK çerçeve.
    let written = Arc::new(AtomicU64::new(0));
    let w2 = Arc::clone(&written);
    let writer = tokio::spawn(async move {
        loop {
            let next = if let Ok(f) = prx.try_recv() {
                Some(f)
            } else if let Ok(f) = trx.try_recv() {
                Some(f)
            } else if let Ok(f) = frx.try_recv() {
                Some(f)
            } else {
                tokio::select! {
                    biased;
                    Some(f) = prx.recv() => Some(f),
                    Some(f) = trx.recv() => Some(f),
                    Some(f) = frx.recv() => Some(f),
                    else => None,
                }
            };
            let Some(f) = next else { break };
            if wr.write_all(&f).await.is_err() {
                break;
            }
            w2.fetch_add(f.len() as u64, Ordering::Relaxed);
        }
        wr
    });
    sleep_until(end).await;
    producers.abort_all();
    writer.abort();
    if let Ok(w) = writer.await {
        drop(w);
    }
    drop(rd);
    Ok(Outcome {
        connected: true,
        setup_ns,
        t0_unix_ns,
        tx_bytes: written.load(Ordering::Relaxed),
        rx_bytes: 0,
        note: "tcp single-connection app-priority-writer[ptt>text>file] tx=app-bytes \
               (kernel overhead invisible)"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> ClientArgs {
        ClientArgs {
            server: "127.0.0.1:1".parse().unwrap(),
            transport: MixTransport::Tcp,
            cc: MixCc::Bbr,
            payload: 1024,
            file_bytes: 2500,
            file_chunk: 1024,
            file_rate_bps: 0,
            text_bytes: 65,
            text_count: 3,
            text_interval_ms: 2000,
            ptt_bytes: 39,
            ptt_count: 4,
            ptt_interval_ms: 200,
            tail_s: 1,
            handshake_timeout_s: 1,
            quinn_per_item: false,
            profile: "t".into(),
            repeat: 0,
            fwd_impair: String::new(),
            rev_impair: String::new(),
            out: PathBuf::from("/dev/null"),
        }
    }

    /// Plan: dosya parçaları t0'da; aynı anda öncelik PTT > TEXT > dosya; id'ler tekil.
    #[test]
    fn plan_sirasi_ve_boylar() {
        let items = plan(&args()).expect("plan");
        assert_eq!(items.len(), 3 + 3 + 4, "3 dosya parçası + 3 TEXT + 4 PTT");
        assert_eq!(items[0].flow, Flow::Ptt, "t0'da önce PTT");
        assert_eq!(items[1].flow, Flow::Text, "sonra TEXT");
        assert_eq!(items[2].flow, Flow::File);
        let last_file = items
            .iter()
            .rev()
            .find(|it| it.flow == Flow::File)
            .expect("dosya");
        assert_eq!(last_file.bytes, 2500 - 2 * 1024, "son parça kalan bayt");
        let mut ids: Vec<u64> = items.iter().map(|it| it.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), items.len(), "id'ler tekil");
    }

    /// Kodlama ↔ çözme: başlık korunur; bayt akışı çerçevesi uzunluğu taşır.
    #[test]
    fn kodlama_cozme() {
        let it = Item {
            flow: Flow::Text,
            id: 77,
            bytes: 65,
            at_ns: 0,
        };
        let b = encode(&it);
        assert_eq!(b.len(), 65);
        assert_eq!(decode(&b), Some((Flow::Text, 77)));
        let f = framed(&it);
        assert_eq!(f.len(), LEN + 65);
        assert_eq!(u32::from_be_bytes([f[0], f[1], f[2], f[3]]), 65);
    }

    /// Başlıktan küçük öğe reddedilir (sessiz kırpma yok).
    #[test]
    fn kucuk_oge_reddedilir() {
        let mut a = args();
        a.ptt_bytes = 4;
        assert!(plan(&a).is_err());
    }
}
