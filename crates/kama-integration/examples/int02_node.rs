//! INT-02 düğüm sürücüsü (plan K4G-05; CORE-CLOSE kritik yolu).
//!
//! TEK süreçte iki ÜRÜN düğümü (`kama_node::KamaNode::start`; negatif kolda
//! `KamaNode::start_recovery_off_for_test`): gerçek `ClientTransport` + gerçek
//! `NoiseWireProtection` + `kama-app` motorları; anahtarlar `kama-provision` çıktısından (cihaz
//! dosyaları `--alice-config` / `--bob-config`). Veri datagramları loopback UDP'den geçer (netem
//! `lo` üzerinde; sürücü `scripts/netem/int02_run.sh`); el sıkışma zarfları süreç-içi röleden netem
//! DIŞINDA geçer (DUR-K4G-11 → B; beyanlı boşluk). İki düğüm de connect ÖNCESİ başlatılır; glare
//! kolu hariç bağlantıyı alice başlatır. `KamaNode::start` tokio bağlamından çağrılmaz: bu süreç
//! düz iş parçacıklarıdır.
//!
//! ÇIKTI: stdout'a TEK satır JSON (şema [`SCHEMA`]) — yalnız GÖZLEM (ham sayılar, ham anlar). GATING
//! hükmü burada VERİLMEZ: `scripts/netem/int02_analyze.py`. Tanılama ve eşgüdüm işaretleri stderr'e.
//! Zaman: tek süreç → tek monotonik saat (`Instant`); bütün anlar düzeneğin başlangıcına (`t0`)
//! göre ms. Bir olayın anı, düğümün dinleyiciyi çağırdığı andır (düğüm iş parçacığında damgalanır).
//!
//! KOLLAR (`--arm`):
//! * `gating1` — alice → bob `--count` TEXT (`--interval-ms` aralıklı); bob'dan RECEIPT.
//! * `gating2` — bob (yanıtlayan) KENDİ `Connected` olayının hemen ardından bob → alice `--count`
//!   TEXT gönderir: yanıtlayanın gönderim epoch'u, başlatanın ilk epoch-1 paketi gelene dek 0'dır
//!   (noise-wire §4-B3) → epoch 0→1 geçişi akışın İÇİNDE olur; `gating2` bunu ölçer.
//! * `gating6` — eşzamanlı connect (glare): iki düğüm iki iş parçacığından bir engelle AYNI ANDA
//!   `connect` eder; sonra iki yönde `--count` TEXT (sırayla, `--interval-ms` aralıklı).
//! * `negatif` — `gating1` akışı, İKİ düğümde de kurtarma KAPALI: ARQ retransmit, §11 kopya ve FEC
//!   repair kararları tele çıkmaz (`recovery_suppressed`). Ölçümün kaybı GÖREBİLDİĞİNİN kanıtıdır.
//! * `ek-dosya-ptt` (NON-GATING) — `gating1` metinleri + 1 dosya (`--file-bytes`) + 1 PTT klibi
//!   (`--bundles` demet, `--bundle-ms` aralıklı, `--bundle-bytes` boyunda).
//! * `ek-kesinti` (NON-GATING) — PTT açılır; metinlerin ve demetlerin İLK yarısı; sonra tam kesinti
//!   (sürücü `loss 100%` uygular); kesinti SIRASINDA ikinci yarı + `ptt_close`; kesinti bitince teslim
//!   beklenir. Eşgüdüm `--sync-dir` işaret dosyalarıyla: örnek [`MARK_START`]'ı yaratır (ve stderr'e
//!   aynı satırı yazar), sürücü kesintiyi uygulayınca [`MARK_APPLIED`]'ı, eski profile dönünce
//!   [`MARK_DONE`]'ı yaratır.
//!
//! Gönderen tarafta geri basınçla (Transport kaydı KABUL ETMEDİ) reddedilen çağrı aynı metinle
//! yeniden denenir: kabul edilmemiş kayıt tele çıkmamıştır, bu Application yeniden gönderimi DEĞİLDİR
//! ve ayrı sayılır (`backpressure_retries`).

#![forbid(unsafe_code)]
// Bu dosya `tests/int02_loopback.rs` tarafından MODÜL olarak da derlenir (aynı düzenek, kısa duman
// koşusu): tipli rapor alanlarını (`Report::flows`, `Report::nodes`, …) yalnız o test okur, ikili
// yalnız JSON satırını basar. İkili derlemede okunmayan alanlar bu yüzden ölü kod sayılmaz.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use client_core::runtime::ClientLiveDiag;
use kama_app::codec::PttCodec;
use kama_app::messaging::MessengerError;
use kama_app::ptt::PttError;
use kama_contracts::s1::TransportError;
use kama_node::{
    CloseCause, FileHandle, FileStage, KamaNode, NodeError, NodeEvent, NodeListener, NodeMetrics,
    NodeProbe, StreamHandle,
};

/// Çıktı şeması.
pub const SCHEMA: &str = "int02-node-v1";
/// Provisioning cihaz adları (`kama-provision init --devices alice,bob`).
pub const ALICE: &str = "alice";
/// Bkz. [`ALICE`].
pub const BOB: &str = "bob";

/// Kesinti eşgüdümü: örnek kesintiyi İSTER.
pub const MARK_START: &str = "KESINTI_BASLA";
/// Kesinti eşgüdümü: sürücü `loss 100%`'ü UYGULADI.
pub const MARK_APPLIED: &str = "KESINTI_UYGULANDI";
/// Kesinti eşgüdümü: sürücü eski profile DÖNDÜ.
pub const MARK_DONE: &str = "KESINTI_BITTI";

/// Varsayılanlar (`--help`).
const DEFAULT_COUNT: u32 = 100;
const DEFAULT_INTERVAL_MS: u64 = 100;
const DEFAULT_CONNECT_TIMEOUT_S: u64 = 60;
const DEFAULT_DRAIN_S: u64 = 120;
const DEFAULT_SETTLE_S: u64 = 10;
/// PoC fotoğrafı ≈150 KB (PoC-KAPSAM-001 §2).
const DEFAULT_FILE_BYTES: usize = 150 * 1024;
const DEFAULT_BUNDLES: u32 = 50;
const DEFAULT_BUNDLE_MS: u8 = 40;
const DEFAULT_BUNDLE_BYTES: usize = 32;
const DEFAULT_OUTAGE_WAIT_S: u64 = 180;

/// Girdi tavanları (SR-AUDIT-001: kayıtlar bellekte tutulur).
const MAX_COUNT: u32 = 10_000;
const MAX_BUNDLES: u32 = 10_000;
/// `kama-app` dosya tavanı (64 MiB; PoC-KAPSAM-001).
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
/// Demet yükünün ilk 4 baytı demet sırasıdır (alıcıda sıra denetimi).
const BUNDLE_INDEX_BYTES: usize = 4;
const MAX_INTERVAL_MS: u64 = 60_000;

/// Dinleyici → düzenek olay kuyruğu tavanı. Dolarsa olay DÜŞER ve sayılır (`listener_drops`);
/// dinleyici düğüm iş parçacığını BEKLETMEZ. Geçerli koşuda 0 olmalıdır.
const EVENT_QUEUE_CAP: usize = 16_384;
/// Olay bekleme adımı ve epoch örnekleme aralığı (düğüm ölçüsü her düğüm turunda tazelenir;
/// tur ≤ `kama_node::NODE_TICK`).
const PUMP_STEP: Duration = Duration::from_millis(5);
const SAMPLE_EVERY: Duration = Duration::from_millis(5);
/// Geri basınçta yeniden deneme beklemesi.
const BACKPRESSURE_WAIT: Duration = Duration::from_millis(10);
/// Sakinleşme (outstanding = 0) yoklama aralığı.
const PROBE_EVERY: Duration = Duration::from_millis(100);
/// Sürücünün kesintiyi uygulaması için azami bekleme.
const MARK_APPLY_TIMEOUT: Duration = Duration::from_secs(30);
/// JSON'a yazılan kimlik / not listelerinin tavanı.
const MAX_LISTED: usize = 32;
/// Dosya içeriği deseni: asal modül (parça sınırlarıyla hizalanmaz).
const FILE_PATTERN: usize = 251;

/// Kullanım metni.
pub const USAGE: &str = "\
kullanım: int02_node --arm <kol> --seed <S> --profile <ad>
                  --alice-config <device-alice.json> --bob-config <device-bob.json>
                  --alice-data <dizin> --bob-data <dizin>
                  [--count N=100] [--interval-ms MS=100] [--connect-timeout-s S=60]
                  [--drain-s S=120] [--settle-s S=10]
                  [--file-bytes B=153600] [--bundles N=50] [--bundle-ms MS=40] [--bundle-bytes B=32]
                  [--scratch-dir <dizin>] [--sync-dir <dizin>] [--outage-wait-s S=180]
       int02_node --help

kollar: gating1 | gating2 | gating6 | negatif | ek-dosya-ptt | ek-kesinti
çıktı : stdout'a TEK satır JSON (şema int02-node-v1); hüküm scripts/netem/int02_analyze.py'de.
ek-kesinti: --sync-dir zorunlu; KESINTI_BASLA / KESINTI_UYGULANDI / KESINTI_BITTI işaret dosyaları.
";

// ── Yapılandırma ─────────────────────────────────────────────────────────────────────────────

/// Kol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    Gating1,
    Gating2,
    Gating6,
    Negative,
    EkDosyaPtt,
    EkKesinti,
}

impl Arm {
    /// Komut satırı adından.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "gating1" => Self::Gating1,
            "gating2" => Self::Gating2,
            "gating6" => Self::Gating6,
            "negatif" => Self::Negative,
            "ek-dosya-ptt" => Self::EkDosyaPtt,
            "ek-kesinti" => Self::EkKesinti,
            _ => return None,
        })
    }

    /// Komut satırı / JSON adı.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Gating1 => "gating1",
            Self::Gating2 => "gating2",
            Self::Gating6 => "gating6",
            Self::Negative => "negatif",
            Self::EkDosyaPtt => "ek-dosya-ptt",
            Self::EkKesinti => "ek-kesinti",
        }
    }
}

/// Koşu yapılandırması.
#[derive(Debug, Clone)]
pub struct Config {
    pub arm: Arm,
    /// Netem tohumu — yalnız kayda geçer (netem'i sürücü kurar).
    pub seed: u64,
    /// Netem profil adı — yalnız kayda geçer.
    pub profile: String,
    pub alice_config: PathBuf,
    pub bob_config: PathBuf,
    pub alice_data: PathBuf,
    pub bob_data: PathBuf,
    /// Yön başına TEXT sayısı.
    pub count: u32,
    /// Ardışık TEXT gönderimleri arası.
    pub interval: Duration,
    pub connect_timeout: Duration,
    /// Son gönderimden (kesinti kolunda kesinti bitişinden) sonra tamamlanma için azami bekleme.
    pub drain: Duration,
    /// Tamamlanmadan sonra Transport'un sakinleşmesi (teyitsiz birim 0) için azami bekleme.
    pub settle: Duration,
    pub file_bytes: usize,
    pub bundles: u32,
    pub bundle_ms: u8,
    pub bundle_bytes: usize,
    /// Dosya kolunun kaynak dosyasının dizini (varsayılan: alice veri dizininin üstü).
    pub scratch_dir: Option<PathBuf>,
    /// Kesinti eşgüdümü dizini (yalnız `ek-kesinti`).
    pub sync_dir: Option<PathBuf>,
    /// Kesinti uygulandıktan sonra `KESINTI_BITTI` için azami bekleme.
    pub outage_wait: Duration,
}

impl Config {
    /// Varsayılan parametrelerle yapılandırma.
    #[must_use]
    pub fn new(
        arm: Arm,
        seed: u64,
        profile: &str,
        configs: [PathBuf; 2],
        data: [PathBuf; 2],
    ) -> Self {
        let [alice_config, bob_config] = configs;
        let [alice_data, bob_data] = data;
        Self {
            arm,
            seed,
            profile: profile.to_owned(),
            alice_config,
            bob_config,
            alice_data,
            bob_data,
            count: DEFAULT_COUNT,
            interval: Duration::from_millis(DEFAULT_INTERVAL_MS),
            connect_timeout: Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_S),
            drain: Duration::from_secs(DEFAULT_DRAIN_S),
            settle: Duration::from_secs(DEFAULT_SETTLE_S),
            file_bytes: DEFAULT_FILE_BYTES,
            bundles: DEFAULT_BUNDLES,
            bundle_ms: DEFAULT_BUNDLE_MS,
            bundle_bytes: DEFAULT_BUNDLE_BYTES,
            scratch_dir: None,
            sync_dir: None,
            outage_wait: Duration::from_secs(DEFAULT_OUTAGE_WAIT_S),
        }
    }

    /// Tavan ve tutarlılık denetimi.
    ///
    /// # Errors
    /// Açıklayıcı metin.
    pub fn validate(&self) -> Result<(), String> {
        if self.count == 0 || self.count > MAX_COUNT {
            return Err(format!("--count 1..={MAX_COUNT} olmalı"));
        }
        if self.interval > Duration::from_millis(MAX_INTERVAL_MS) {
            return Err(format!("--interval-ms ≤ {MAX_INTERVAL_MS} olmalı"));
        }
        if self.bundles == 0 || self.bundles > MAX_BUNDLES {
            return Err(format!("--bundles 1..={MAX_BUNDLES} olmalı"));
        }
        if self.bundle_bytes < BUNDLE_INDEX_BYTES {
            return Err(format!("--bundle-bytes ≥ {BUNDLE_INDEX_BYTES} olmalı"));
        }
        if self.file_bytes == 0 || self.file_bytes > MAX_FILE_BYTES {
            return Err(format!("--file-bytes 1..={MAX_FILE_BYTES} olmalı"));
        }
        if self.arm == Arm::EkKesinti {
            if self.sync_dir.is_none() {
                return Err("ek-kesinti kolu --sync-dir ister".to_owned());
            }
            if self.count < 2 || self.bundles < 2 {
                return Err("ek-kesinti kolu --count ≥ 2 ve --bundles ≥ 2 ister".to_owned());
            }
        }
        Ok(())
    }
}

/// Ayrıştırma sonucu.
#[derive(Debug)]
pub enum Parsed {
    Help,
    Run(Box<Config>),
}

/// `std::env::args` (program adı HARİÇ) ayrıştırması; clap YOK (D11). Her bayrak tam bir değer alır
/// ve en çok bir kez verilir.
///
/// # Errors
/// Kullanım hatası metni.
pub fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--help" || flag == "-h" {
            return Ok(Parsed::Help);
        }
        if !KNOWN_FLAGS.contains(&flag.as_str()) {
            return Err(format!("bilinmeyen argüman {flag:?}"));
        }
        let value = it.next().ok_or_else(|| format!("{flag} için değer yok"))?;
        if values.insert(flag.as_str(), value.as_str()).is_some() {
            return Err(format!("{flag} birden fazla verildi"));
        }
    }
    let arm_name = required(&values, "--arm")?;
    let arm = Arm::parse(arm_name).ok_or_else(|| format!("bilinmeyen kol {arm_name:?}"))?;
    let seed = number::<u64>("--seed", required(&values, "--seed")?)?;
    let profile = required(&values, "--profile")?;
    let configs = [
        PathBuf::from(required(&values, "--alice-config")?),
        PathBuf::from(required(&values, "--bob-config")?),
    ];
    let data = [
        PathBuf::from(required(&values, "--alice-data")?),
        PathBuf::from(required(&values, "--bob-data")?),
    ];
    let mut cfg = Config::new(arm, seed, profile, configs, data);
    if let Some(v) = values.get("--count") {
        cfg.count = number("--count", v)?;
    }
    if let Some(v) = values.get("--interval-ms") {
        cfg.interval = Duration::from_millis(number("--interval-ms", v)?);
    }
    if let Some(v) = values.get("--connect-timeout-s") {
        cfg.connect_timeout = Duration::from_secs(number("--connect-timeout-s", v)?);
    }
    if let Some(v) = values.get("--drain-s") {
        cfg.drain = Duration::from_secs(number("--drain-s", v)?);
    }
    if let Some(v) = values.get("--settle-s") {
        cfg.settle = Duration::from_secs(number("--settle-s", v)?);
    }
    if let Some(v) = values.get("--file-bytes") {
        cfg.file_bytes = number("--file-bytes", v)?;
    }
    if let Some(v) = values.get("--bundles") {
        cfg.bundles = number("--bundles", v)?;
    }
    if let Some(v) = values.get("--bundle-ms") {
        cfg.bundle_ms = number("--bundle-ms", v)?;
    }
    if let Some(v) = values.get("--bundle-bytes") {
        cfg.bundle_bytes = number("--bundle-bytes", v)?;
    }
    if let Some(v) = values.get("--scratch-dir") {
        cfg.scratch_dir = Some(PathBuf::from(*v));
    }
    if let Some(v) = values.get("--sync-dir") {
        cfg.sync_dir = Some(PathBuf::from(*v));
    }
    if let Some(v) = values.get("--outage-wait-s") {
        cfg.outage_wait = Duration::from_secs(number("--outage-wait-s", v)?);
    }
    cfg.validate()?;
    Ok(Parsed::Run(Box::new(cfg)))
}

const KNOWN_FLAGS: [&str; 20] = [
    "--arm",
    "--seed",
    "--profile",
    "--alice-config",
    "--bob-config",
    "--alice-data",
    "--bob-data",
    "--count",
    "--interval-ms",
    "--connect-timeout-s",
    "--drain-s",
    "--settle-s",
    "--file-bytes",
    "--bundles",
    "--bundle-ms",
    "--bundle-bytes",
    "--scratch-dir",
    "--sync-dir",
    "--outage-wait-s",
    "--help",
];

fn required<'a>(values: &BTreeMap<&str, &'a str>, flag: &str) -> Result<&'a str, String> {
    values
        .get(flag)
        .copied()
        .ok_or_else(|| format!("{flag} zorunlu"))
}

fn number<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} değeri sayı değil: {value:?}"))
}

// ── Olay toplama ─────────────────────────────────────────────────────────────────────────────

/// Düğüm tarafı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Alice,
    Bob,
}

impl Side {
    const BOTH: [Self; 2] = [Self::Alice, Self::Bob];

    /// Provisioning adı.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Alice => ALICE,
            Self::Bob => BOB,
        }
    }

    /// Karşı taraf.
    #[must_use]
    pub fn peer(self) -> Self {
        match self {
            Self::Alice => Self::Bob,
            Self::Bob => Self::Alice,
        }
    }

    /// Dizi indeksi.
    #[must_use]
    pub fn idx(self) -> usize {
        match self {
            Self::Alice => 0,
            Self::Bob => 1,
        }
    }
}

/// Damgalı olay.
struct Stamped {
    side: Side,
    at: Instant,
    event: NodeEvent,
}

/// Olayları damgalayıp sınırlı kuyruğa koyan dinleyici. `Metrics` olayları atılır (ölçü
/// `KamaNode::metrics` ile örneklenir).
struct Tap {
    side: Side,
    tx: SyncSender<Stamped>,
    dropped: Arc<AtomicU64>,
}

impl NodeListener for Tap {
    fn on_event(&self, event: NodeEvent) {
        if matches!(event, NodeEvent::Metrics(_)) {
            return;
        }
        let stamped = Stamped {
            side: self.side,
            at: Instant::now(),
            event,
        };
        if self.tx.try_send(stamped).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Bir yönün (gönderen tarafın) metin kaydı.
#[derive(Default)]
struct Flow {
    /// `msg_id` → (gönderim anı, metin).
    sent: BTreeMap<u32, (Instant, String)>,
    /// `msg_id` → ilk `TextReceived` anı (yalnız gönderilmiş `msg_id`'ler).
    delivered: BTreeMap<u32, Instant>,
    /// `msg_id` → ilk `Receipt` anı.
    receipts: BTreeMap<u32, Instant>,
    duplicate_deliveries: u64,
    mismatched_texts: u64,
    unknown_deliveries: u64,
    duplicate_receipts: u64,
    unknown_receipts: u64,
    backpressure_retries: u64,
    first_send_at: Option<Instant>,
    last_send_at: Option<Instant>,
    /// İlk gönderimden hemen önce örneklenen, göndericinin bu eşle oturum epoch'u.
    first_send_epoch: Option<Option<u8>>,
    /// Göndericinin örneklenen epoch'u 0 iken yapılan gönderimler.
    sends_at_epoch0: u32,
}

impl Flow {
    fn done(&self, expected: u32) -> bool {
        let sent = self.sent.len();
        sent == expected as usize && self.delivered.len() == sent && self.receipts.len() == sent
    }
}

/// Bir düğümün olay ve epoch kaydı.
#[derive(Default)]
struct NodeLog {
    connected: u64,
    first_connected_at: Option<Instant>,
    closed_session: u64,
    abandoned: u64,
    connect_progress: u64,
    signaling: u64,
    epoch0_first: Option<Instant>,
    epoch1_first: Option<Instant>,
    epoch_last: Option<u8>,
}

#[derive(Default)]
struct FileLog {
    handle: Option<FileHandle>,
    source: Option<PathBuf>,
    source_len: usize,
    sent_at: Option<Instant>,
    progress_sent_at: Option<Instant>,
    delivered_at: Option<Instant>,
    received_at: Option<Instant>,
    received_path: Option<PathBuf>,
    received_size: u64,
    bytes_match: Option<bool>,
    failures: Vec<String>,
}

#[derive(Default)]
struct PttLog {
    opened_at: Option<Instant>,
    pushed: u32,
    push_retries: u64,
    closed_at: Option<Instant>,
    peer_opened_events: u64,
    peer_opened_at: Option<Instant>,
    first_bundle_at: Option<Instant>,
    received: Vec<u32>,
    received_overflow: u64,
    unreadable_bundles: u64,
    peer_closed_events: u64,
    peer_closed_at: Option<Instant>,
}

impl PttLog {
    fn done(&self) -> bool {
        self.peer_closed_at.is_some() && self.received.len() >= self.pushed as usize
    }

    /// Demetler 0..pushed sırasıyla, eksiksiz ve tekrarsız mı geldi.
    fn in_order(&self) -> bool {
        self.received.len() == self.pushed as usize
            && self
                .received
                .iter()
                .zip(0..self.pushed)
                .all(|(got, want)| *got == want)
    }
}

#[derive(Default)]
struct OutageLog {
    requested_at: Option<Instant>,
    applied_at: Option<Instant>,
    restored_at: Option<Instant>,
}

/// Koşu sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Kolun bütün teslim koşulları süre içinde gerçekleşti.
    Complete,
    /// Süre doldu (ölçüm geçerli; teslim eksik).
    Incomplete,
    /// Düzenek hatası (kurulum, connect, çağrı reddi).
    Error(String),
}

impl Outcome {
    fn name(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
            Self::Error(_) => "error",
        }
    }
}

/// Bir yönün sayıları (test iddiaları için).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowCounts {
    pub sent: usize,
    pub delivered: usize,
    pub duplicate_deliveries: u64,
    pub mismatched_texts: u64,
    pub unknown_deliveries: u64,
    pub receipts: usize,
    pub duplicate_receipts: u64,
}

/// Bir düğümün sayıları (test iddiaları için).
#[derive(Debug, Clone, Default)]
pub struct NodeCounts {
    pub connected: u64,
    pub closed_session: u64,
    pub abandoned: u64,
    pub final_epoch: Option<u8>,
    pub probe: Option<NodeProbe>,
}

/// Koşu raporu: tek satır JSON + test iddiaları için tipli özet.
#[derive(Debug, Clone)]
pub struct Report {
    pub outcome: Outcome,
    /// Gönderen tarafa göre (`Side::idx`).
    pub flows: [FlowCounts; 2],
    pub nodes: [NodeCounts; 2],
    pub listener_drops: u64,
    pub file_ok: Option<bool>,
    pub ptt_ok: Option<bool>,
    pub quiescent: bool,
    /// Stdout'a basılan satır.
    pub json: String,
}

// ── Düzenek ──────────────────────────────────────────────────────────────────────────────────

/// Koşar ve raporlar. Hata da rapordur (`Outcome::Error`); panik yolu yoktur.
#[must_use]
pub fn run(cfg: &Config) -> Report {
    let mut harness = Harness::new(cfg.clone());
    let result = harness.execute();
    harness.finish(result)
}

struct Harness {
    cfg: Config,
    t0: Instant,
    rx: Receiver<Stamped>,
    tx: SyncSender<Stamped>,
    dropped: Arc<AtomicU64>,
    nodes: [Option<KamaNode>; 2],
    flows: [Flow; 2],
    logs: [NodeLog; 2],
    file: FileLog,
    ptt: PttLog,
    outage: OutageLog,
    next_sample: Instant,
    connect_requested_at: Option<Instant>,
    notes: Vec<String>,
}

impl Harness {
    fn new(cfg: Config) -> Self {
        let (tx, rx) = mpsc::sync_channel(EVENT_QUEUE_CAP);
        let t0 = Instant::now();
        Self {
            cfg,
            t0,
            rx,
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
            nodes: [None, None],
            flows: [Flow::default(), Flow::default()],
            logs: [NodeLog::default(), NodeLog::default()],
            file: FileLog::default(),
            ptt: PttLog::default(),
            outage: OutageLog::default(),
            next_sample: t0,
            connect_requested_at: None,
            notes: Vec::new(),
        }
    }

    fn node(&self, side: Side) -> Result<&KamaNode, String> {
        self.nodes[side.idx()]
            .as_ref()
            .ok_or_else(|| format!("{} düğümü yok", side.name()))
    }

    fn start_node(&self, side: Side) -> Result<KamaNode, String> {
        let tap = Box::new(Tap {
            side,
            tx: self.tx.clone(),
            dropped: Arc::clone(&self.dropped),
        });
        let (config, data) = match side {
            Side::Alice => (&self.cfg.alice_config, &self.cfg.alice_data),
            Side::Bob => (&self.cfg.bob_config, &self.cfg.bob_data),
        };
        let started = if self.cfg.arm == Arm::Negative {
            KamaNode::start_recovery_off_for_test(config, data, tap)
        } else {
            KamaNode::start(config, data, tap)
        };
        started.map_err(|e| format!("{} başlatılamadı: {e}", side.name()))
    }

    /// Kolun akışı. `Ok(true)` = tamamlandı, `Ok(false)` = süre doldu.
    fn execute(&mut self) -> Result<bool, String> {
        // İki düğüm de connect ÖNCESİ başlatılır (süreç-içi röle kayıtsız eşe zarf saklamaz).
        self.nodes[Side::Alice.idx()] = Some(self.start_node(Side::Alice)?);
        self.nodes[Side::Bob.idx()] = Some(self.start_node(Side::Bob)?);
        self.connect()?;
        let n = self.cfg.count;
        match self.cfg.arm {
            Arm::Gating1 | Arm::Negative => self.send_series(&series(Side::Alice, 0..n))?,
            Arm::Gating2 => self.send_series(&series(Side::Bob, 0..n))?,
            Arm::Gating6 => self.send_series(&interleaved(0..n))?,
            Arm::EkDosyaPtt => {
                self.send_series(&series(Side::Alice, 0..n))?;
                self.send_file()?;
                let stream = self.ptt_open()?;
                self.push_bundles(stream, 0..self.cfg.bundles)?;
                self.ptt_close(stream)?;
            }
            Arm::EkKesinti => {
                let half = n / 2;
                let bundle_half = self.cfg.bundles / 2;
                let stream = self.ptt_open()?;
                self.send_series(&series(Side::Alice, 0..half))?;
                self.push_bundles(stream, 0..bundle_half)?;
                self.begin_outage()?;
                self.send_series(&series(Side::Alice, half..n))?;
                self.push_bundles(stream, bundle_half..self.cfg.bundles)?;
                self.ptt_close(stream)?;
                self.await_outage_end()?;
            }
        }
        let deadline = Instant::now() + self.cfg.drain;
        Ok(self.wait_for(deadline, Self::settled_or_hopeless) && self.complete())
    }

    /// Bağlantı: glare kolunda iki iş parçacığından engelle eşzamanlı; diğerlerinde alice →
    /// bob. `gating2` yalnız bob'un `Connected`'ını bekler (bob hemen gönderir).
    fn connect(&mut self) -> Result<(), String> {
        if self.cfg.arm == Arm::Gating6 {
            let alice = self.node(Side::Alice)?;
            let bob = self.node(Side::Bob)?;
            let barrier = Barrier::new(2);
            let (a, b) = thread::scope(|scope| {
                let a = scope.spawn(|| {
                    barrier.wait();
                    alice.connect(BOB).map_err(|e| e.to_string())
                });
                let b = scope.spawn(|| {
                    barrier.wait();
                    bob.connect(ALICE).map_err(|e| e.to_string())
                });
                (a.join(), b.join())
            });
            for (who, joined) in [(ALICE, a), (BOB, b)] {
                match joined {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => return Err(format!("{who} connect: {e}")),
                    Err(_) => return Err(format!("{who} connect iş parçacığı panikledi")),
                }
            }
        } else {
            self.node(Side::Alice)?
                .connect(BOB)
                .map_err(|e| format!("alice connect: {e}"))?;
        }
        self.connect_requested_at = Some(Instant::now());
        let deadline = Instant::now() + self.cfg.connect_timeout;
        let ok = if self.cfg.arm == Arm::Gating2 {
            self.wait_for(deadline, |h| h.logs[Side::Bob.idx()].connected > 0)
        } else {
            self.wait_for(deadline, |h| h.logs.iter().all(|log| log.connected > 0))
        };
        if ok {
            Ok(())
        } else {
            Err(format!(
                "connect {:?} içinde tamamlanmadı",
                self.cfg.connect_timeout
            ))
        }
    }

    // ── Gönderim ──

    /// Metinleri `--interval-ms` aralıkla gönderir (aralarda olaylar işlenir).
    fn send_series(&mut self, plan: &[(Side, u32)]) -> Result<(), String> {
        let mut next = Instant::now();
        for &(side, index) in plan {
            self.pump_until(next);
            self.send_text(side, index)?;
            next += self.cfg.interval;
        }
        Ok(())
    }

    fn text_of(&self, side: Side, index: u32) -> String {
        format!(
            "int02 {} s{} {}->{} #{index:05}",
            self.cfg.arm.name(),
            self.cfg.seed,
            side.name(),
            side.peer().name()
        )
    }

    fn send_text(&mut self, side: Side, index: u32) -> Result<(), String> {
        let text = self.text_of(side, index);
        let deadline = Instant::now() + self.cfg.drain;
        loop {
            let node = self.node(side)?;
            let epoch = peer_epoch(&node.metrics(), side.peer().name());
            let at = Instant::now();
            match node.send_text(side.peer().name(), &text) {
                Ok(msg_id) => {
                    let flow = &mut self.flows[side.idx()];
                    flow.first_send_at.get_or_insert(at);
                    flow.last_send_at = Some(at);
                    flow.first_send_epoch.get_or_insert(epoch);
                    if epoch == Some(0) {
                        flow.sends_at_epoch0 += 1;
                    }
                    if flow.sent.insert(msg_id, (at, text)).is_some() {
                        return Err(format!("{}: msg_id {msg_id} iki kez döndü", side.name()));
                    }
                    return Ok(());
                }
                Err(e) if is_backpressure(&e) && Instant::now() < deadline => {
                    self.flows[side.idx()].backpressure_retries += 1;
                    self.pump_until(Instant::now() + BACKPRESSURE_WAIT);
                }
                Err(e) => return Err(format!("{} send_text: {e}", side.name())),
            }
        }
    }

    fn scratch_dir(&self) -> PathBuf {
        self.cfg.scratch_dir.clone().unwrap_or_else(|| {
            self.cfg
                .alice_data
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        })
    }

    fn send_file(&mut self) -> Result<(), String> {
        let source = self
            .scratch_dir()
            .join(format!("int02-source-s{}.bin", self.cfg.seed));
        let bytes: Vec<u8> = (0..self.cfg.file_bytes)
            .map(|i| u8::try_from(i % FILE_PATTERN).unwrap_or(0))
            .collect();
        fs::write(&source, &bytes).map_err(|e| format!("kaynak dosya {source:?}: {e}"))?;
        let at = Instant::now();
        let handle = self
            .node(Side::Alice)?
            .send_file(BOB, &source)
            .map_err(|e| format!("send_file: {e}"))?;
        self.file.handle = Some(handle);
        self.file.source = Some(source);
        self.file.source_len = bytes.len();
        self.file.sent_at = Some(at);
        Ok(())
    }

    fn ptt_open(&mut self) -> Result<StreamHandle, String> {
        let deadline = Instant::now() + self.cfg.drain;
        let stream = loop {
            let node = self.node(Side::Alice)?;
            match node.ptt_open(BOB, PttCodec::Lyra, self.cfg.bundle_ms) {
                Ok(stream) => break stream,
                Err(e) if is_ptt_backpressure(&e) && Instant::now() < deadline => {
                    self.ptt.push_retries += 1;
                    self.pump_until(Instant::now() + BACKPRESSURE_WAIT);
                }
                Err(e) => return Err(format!("ptt_open: {e}")),
            }
        };
        self.ptt.opened_at = Some(Instant::now());
        Ok(stream)
    }

    /// Demetleri `--bundle-ms` aralıkla verir; demet yükünün ilk 4 baytı sırasıdır.
    fn push_bundles(&mut self, stream: StreamHandle, range: Range<u32>) -> Result<(), String> {
        let step = Duration::from_millis(u64::from(self.cfg.bundle_ms));
        let deadline = Instant::now() + self.cfg.drain;
        let mut next = Instant::now();
        for index in range {
            self.pump_until(next);
            let payload = bundle_payload(index, self.cfg.bundle_bytes);
            loop {
                let node = self.node(Side::Alice)?;
                match node.ptt_push(stream, payload.clone()) {
                    Ok(()) => {
                        self.ptt.pushed += 1;
                        break;
                    }
                    Err(e) if is_ptt_backpressure(&e) && Instant::now() < deadline => {
                        self.ptt.push_retries += 1;
                        self.pump_until(Instant::now() + BACKPRESSURE_WAIT);
                    }
                    Err(e) => return Err(format!("ptt_push #{index}: {e}")),
                }
            }
            next += step;
        }
        Ok(())
    }

    fn ptt_close(&mut self, stream: StreamHandle) -> Result<(), String> {
        self.node(Side::Alice)?
            .ptt_close(stream)
            .map_err(|e| format!("ptt_close: {e}"))?;
        self.ptt.closed_at = Some(Instant::now());
        Ok(())
    }

    // ── Kesinti eşgüdümü ──

    fn sync_dir(&self) -> Result<PathBuf, String> {
        self.cfg
            .sync_dir
            .clone()
            .ok_or_else(|| "--sync-dir yok".to_owned())
    }

    fn begin_outage(&mut self) -> Result<(), String> {
        let dir = self.sync_dir()?;
        fs::write(dir.join(MARK_START), b"").map_err(|e| format!("{MARK_START}: {e}"))?;
        self.outage.requested_at = Some(Instant::now());
        eprintln!("{MARK_START}");
        let applied = dir.join(MARK_APPLIED);
        if !self.wait_for(Instant::now() + MARK_APPLY_TIMEOUT, |_| applied.exists()) {
            return Err(format!(
                "{MARK_APPLIED} {MARK_APPLY_TIMEOUT:?} içinde gelmedi"
            ));
        }
        self.outage.applied_at = Some(Instant::now());
        Ok(())
    }

    fn await_outage_end(&mut self) -> Result<(), String> {
        let done = self.sync_dir()?.join(MARK_DONE);
        let from = self.outage.applied_at.unwrap_or_else(Instant::now);
        if !self.wait_for(from + self.cfg.outage_wait, |_| done.exists()) {
            return Err(format!(
                "{MARK_DONE} {:?} içinde gelmedi",
                self.cfg.outage_wait
            ));
        }
        self.outage.restored_at = Some(Instant::now());
        eprintln!("{MARK_DONE} görüldü");
        Ok(())
    }

    // ── Olaylar ──

    /// `until`'e dek olayları işler ve epoch'u örnekler.
    fn pump_until(&mut self, until: Instant) {
        loop {
            self.sample_if_due();
            let now = Instant::now();
            if now >= until {
                return;
            }
            match self.rx.recv_timeout((until - now).min(PUMP_STEP)) {
                Ok(stamped) => {
                    self.on_event(stamped);
                    while let Ok(more) = self.rx.try_recv() {
                        self.on_event(more);
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
        }
    }

    /// `done` doğru olana ya da `deadline`'a dek olay işler.
    fn wait_for(&mut self, deadline: Instant, done: impl Fn(&Self) -> bool) -> bool {
        loop {
            if done(self) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            self.pump_until((now + PUMP_STEP).min(deadline));
        }
    }

    fn sample_if_due(&mut self) {
        let now = Instant::now();
        if now < self.next_sample {
            return;
        }
        self.next_sample = now + SAMPLE_EVERY;
        self.sample_epochs(now);
    }

    fn sample_epochs(&mut self, now: Instant) {
        for side in Side::BOTH {
            let Some(node) = self.nodes[side.idx()].as_ref() else {
                continue;
            };
            let epoch = peer_epoch(&node.metrics(), side.peer().name());
            let log = &mut self.logs[side.idx()];
            match epoch {
                Some(0) => {
                    log.epoch0_first.get_or_insert(now);
                }
                Some(1) => {
                    log.epoch1_first.get_or_insert(now);
                }
                _ => {}
            }
            if epoch.is_some() {
                log.epoch_last = epoch;
            }
        }
    }

    fn on_event(&mut self, stamped: Stamped) {
        let Stamped { side, at, event } = stamped;
        let log = &mut self.logs[side.idx()];
        match event {
            NodeEvent::Connected { .. } => {
                log.connected += 1;
                log.first_connected_at.get_or_insert(at);
            }
            NodeEvent::Closed { reason, .. } => match reason {
                CloseCause::Session(_) => log.closed_session += 1,
                CloseCause::Abandoned(_) => log.abandoned += 1,
            },
            NodeEvent::ConnectProgress { .. } => log.connect_progress += 1,
            NodeEvent::SignalingReceived { .. } => log.signaling += 1,
            NodeEvent::TextReceived { msg_id, text, .. } => {
                // Alıcı `side`; yön gönderen tarafındır.
                let flow = &mut self.flows[side.peer().idx()];
                match flow.sent.get(&msg_id) {
                    None => flow.unknown_deliveries += 1,
                    Some((_, expected)) => {
                        if *expected != text {
                            flow.mismatched_texts += 1;
                        }
                        if flow.delivered.contains_key(&msg_id) {
                            flow.duplicate_deliveries += 1;
                        } else {
                            flow.delivered.insert(msg_id, at);
                        }
                    }
                }
            }
            NodeEvent::Receipt { msg_id, .. } => {
                let flow = &mut self.flows[side.idx()];
                if !flow.sent.contains_key(&msg_id) {
                    flow.unknown_receipts += 1;
                } else if flow.receipts.contains_key(&msg_id) {
                    flow.duplicate_receipts += 1;
                } else {
                    flow.receipts.insert(msg_id, at);
                }
            }
            NodeEvent::FileProgress { file, stage, .. } => {
                if self.file.handle == Some(file) {
                    match stage {
                        FileStage::Sent => {
                            self.file.progress_sent_at.get_or_insert(at);
                        }
                        FileStage::Delivered => {
                            self.file.delivered_at.get_or_insert(at);
                        }
                    }
                }
            }
            NodeEvent::FileReceived { path, size, .. } => {
                if self.file.received_at.is_none() {
                    self.file.received_at = Some(at);
                    self.file.received_path = Some(path);
                    self.file.received_size = size;
                }
            }
            NodeEvent::FileFailed { file, reason, .. } => {
                if self.file.failures.len() < MAX_LISTED {
                    self.file
                        .failures
                        .push(format!("{}: {file:?} {reason:?}", side.name()));
                }
            }
            NodeEvent::PttOpened { .. } => {
                self.ptt.peer_opened_events += 1;
                self.ptt.peer_opened_at.get_or_insert(at);
            }
            NodeEvent::PttBundle { payload, .. } => {
                self.ptt.first_bundle_at.get_or_insert(at);
                match bundle_index(&payload) {
                    None => self.ptt.unreadable_bundles += 1,
                    Some(_) if self.ptt.received.len() >= self.received_bundle_cap() => {
                        self.ptt.received_overflow += 1;
                    }
                    Some(index) => self.ptt.received.push(index),
                }
            }
            NodeEvent::PttClosed { .. } => {
                self.ptt.peer_closed_events += 1;
                self.ptt.peer_closed_at.get_or_insert(at);
            }
            NodeEvent::Metrics(_) => {}
        }
    }

    /// Alınan demet kaydının tavanı (SR-AUDIT-001): beklenenin iki katı + pay.
    fn received_bundle_cap(&self) -> usize {
        (self.cfg.bundles as usize)
            .saturating_mul(2)
            .saturating_add(16)
    }

    // ── Tamamlanma ──

    fn complete(&self) -> bool {
        let n = self.cfg.count;
        let alice = &self.flows[Side::Alice.idx()];
        let bob = &self.flows[Side::Bob.idx()];
        match self.cfg.arm {
            Arm::Gating1 | Arm::Negative => alice.done(n),
            Arm::Gating2 => bob.done(n),
            Arm::Gating6 => alice.done(n) && bob.done(n),
            Arm::EkDosyaPtt => alice.done(n) && self.file_done() && self.ptt.done(),
            Arm::EkKesinti => alice.done(n) && self.ptt.done(),
        }
    }

    fn file_done(&self) -> bool {
        self.file.delivered_at.is_some() && self.file.received_at.is_some()
    }

    /// Beklemenin sonu: tamamlandı ya da artık tamamlanamaz (dosya başarısız).
    fn settled_or_hopeless(&self) -> bool {
        self.complete() || (self.cfg.arm == Arm::EkDosyaPtt && !self.file.failures.is_empty())
    }

    /// Transport sakinleşene (iki düğümde teyitsiz birim 0) ya da `--settle-s` dolana dek bekler.
    fn settle(&mut self) -> bool {
        let deadline = Instant::now() + self.cfg.settle;
        loop {
            let quiet = Side::BOTH.iter().all(|&side| {
                self.node(side)
                    .ok()
                    .and_then(|node| node.probe_for_test().ok())
                    .is_some_and(|probe| probe.outstanding_units == 0)
            });
            if quiet {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            self.pump_until((now + PROBE_EVERY).min(deadline));
        }
    }

    // ── Kapanış ve rapor ──

    fn finish(mut self, result: Result<bool, String>) -> Report {
        let outcome = match result {
            Ok(true) => Outcome::Complete,
            Ok(false) => Outcome::Incomplete,
            Err(e) => Outcome::Error(e),
        };
        let have_nodes = self.nodes.iter().all(Option::is_some);
        // Negatif kolda kaybolan birim hiç teyit edilmez: sakinleşme beklenmez.
        let quiescent = have_nodes && self.cfg.arm != Arm::Negative && self.settle();
        // Son olaylar ve son epoch örneği.
        while let Ok(stamped) = self.rx.try_recv() {
            self.on_event(stamped);
        }
        self.sample_epochs(Instant::now());
        let mut probes: [Option<NodeProbe>; 2] = [None, None];
        let mut metrics: [Option<NodeMetrics>; 2] = [None, None];
        for side in Side::BOTH {
            if let Some(node) = self.nodes[side.idx()].as_ref() {
                match node.probe_for_test() {
                    Ok(probe) => probes[side.idx()] = Some(probe),
                    Err(e) => self.notes.push(format!("{} sonda: {e}", side.name())),
                }
                metrics[side.idx()] = Some(node.metrics());
            }
        }
        self.check_file_bytes();
        for side in Side::BOTH {
            if let Some(node) = self.nodes[side.idx()].take() {
                if let Err(e) = node.shutdown() {
                    self.notes.push(format!("{} kapanış: {e}", side.name()));
                }
            }
        }
        while let Ok(stamped) = self.rx.try_recv() {
            self.on_event(stamped);
        }
        let wall = self.t0.elapsed();
        let json = self.json(&outcome, quiescent, &probes, &metrics, wall);
        let listener_drops = self.dropped.load(Ordering::Relaxed);
        let flows = [self.flow_counts(Side::Alice), self.flow_counts(Side::Bob)];
        let [alice_probe, bob_probe] = probes;
        let nodes = [
            self.node_counts(Side::Alice, alice_probe),
            self.node_counts(Side::Bob, bob_probe),
        ];
        let file_ok = (self.cfg.arm == Arm::EkDosyaPtt)
            .then(|| self.file_done() && self.file.bytes_match == Some(true));
        let ptt_ok = matches!(self.cfg.arm, Arm::EkDosyaPtt | Arm::EkKesinti)
            .then(|| self.ptt.done() && self.ptt.in_order());
        Report {
            outcome,
            flows,
            nodes,
            listener_drops,
            file_ok,
            ptt_ok,
            quiescent,
            json,
        }
    }

    fn check_file_bytes(&mut self) {
        let (Some(source), Some(received)) = (&self.file.source, &self.file.received_path) else {
            return;
        };
        let same = match (fs::read(source), fs::read(received)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        self.file.bytes_match = Some(same);
    }

    fn flow_counts(&self, side: Side) -> FlowCounts {
        let flow = &self.flows[side.idx()];
        FlowCounts {
            sent: flow.sent.len(),
            delivered: flow.delivered.len(),
            duplicate_deliveries: flow.duplicate_deliveries,
            mismatched_texts: flow.mismatched_texts,
            unknown_deliveries: flow.unknown_deliveries,
            receipts: flow.receipts.len(),
            duplicate_receipts: flow.duplicate_receipts,
        }
    }

    fn node_counts(&self, side: Side, probe: Option<NodeProbe>) -> NodeCounts {
        let log = &self.logs[side.idx()];
        NodeCounts {
            connected: log.connected,
            closed_session: log.closed_session,
            abandoned: log.abandoned,
            final_epoch: log.epoch_last,
            probe,
        }
    }

    /// `t0`'a göre ms.
    fn at(&self, t: Option<Instant>) -> Value {
        t.map_or(Value::Null, |t| {
            Value::F(ms(t.saturating_duration_since(self.t0)))
        })
    }

    fn json(
        &self,
        outcome: &Outcome,
        quiescent: bool,
        probes: &[Option<NodeProbe>; 2],
        metrics: &[Option<NodeMetrics>; 2],
        wall: Duration,
    ) -> String {
        let cfg = &self.cfg;
        let error = match outcome {
            Outcome::Error(e) => Value::S(e.clone()),
            _ => Value::Null,
        };
        let params = obj(vec![
            ("count", Value::U(u64::from(cfg.count))),
            ("interval_ms", Value::F(ms(cfg.interval))),
            ("connect_timeout_s", Value::U(cfg.connect_timeout.as_secs())),
            ("drain_s", Value::U(cfg.drain.as_secs())),
            ("settle_s", Value::U(cfg.settle.as_secs())),
            ("file_bytes", Value::U(cfg.file_bytes as u64)),
            ("bundles", Value::U(u64::from(cfg.bundles))),
            ("bundle_ms", Value::U(u64::from(cfg.bundle_ms))),
            ("bundle_bytes", Value::U(cfg.bundle_bytes as u64)),
            ("outage_wait_s", Value::U(cfg.outage_wait.as_secs())),
        ]);
        let flows = obj(vec![
            ("alice_to_bob", self.flow_json(Side::Alice)),
            ("bob_to_alice", self.flow_json(Side::Bob)),
        ]);
        let nodes = obj(vec![
            (ALICE, self.node_json(Side::Alice, probes, metrics)),
            (BOB, self.node_json(Side::Bob, probes, metrics)),
        ]);
        let notes = Value::A(self.notes.iter().map(|n| Value::S(n.clone())).collect());
        let root = obj(vec![
            ("schema", Value::S(SCHEMA.to_owned())),
            ("arm", Value::S(cfg.arm.name().to_owned())),
            ("seed", Value::U(cfg.seed)),
            ("profile", Value::S(cfg.profile.clone())),
            ("result", Value::S(outcome.name().to_owned())),
            ("error", error),
            ("params", params),
            ("connect_requested_ms", self.at(self.connect_requested_at)),
            ("flows", flows),
            ("nodes", nodes),
            ("file", self.file_json()),
            ("ptt", self.ptt_json()),
            ("outage", self.outage_json()),
            ("quiescent", Value::Bool(quiescent)),
            (
                "listener_drops",
                Value::U(self.dropped.load(Ordering::Relaxed)),
            ),
            ("notes", notes),
            ("wall_ms", Value::F(ms(wall))),
        ]);
        let mut out = String::new();
        root.write(&mut out);
        out
    }

    fn flow_json(&self, side: Side) -> Value {
        let flow = &self.flows[side.idx()];
        let sent = flow.sent.len();
        let latency: Vec<f64> = flow
            .delivered
            .iter()
            .filter_map(|(id, got)| {
                flow.sent
                    .get(id)
                    .map(|(at, _)| ms(got.saturating_duration_since(*at)))
            })
            .collect();
        let receipt_rtt: Vec<f64> = flow
            .receipts
            .iter()
            .filter_map(|(id, got)| {
                flow.sent
                    .get(id)
                    .map(|(at, _)| ms(got.saturating_duration_since(*at)))
            })
            .collect();
        let undelivered: Vec<Value> = flow
            .sent
            .keys()
            .filter(|id| !flow.delivered.contains_key(*id))
            .take(MAX_LISTED)
            .map(|id| Value::U(u64::from(*id)))
            .collect();
        let first_send_epoch = match flow.first_send_epoch {
            Some(Some(epoch)) => Value::U(u64::from(epoch)),
            _ => Value::Null,
        };
        obj(vec![
            ("sent", Value::U(sent as u64)),
            ("delivered", Value::U(flow.delivered.len() as u64)),
            ("delivered_pct", pct(flow.delivered.len(), sent)),
            ("duplicate_deliveries", Value::U(flow.duplicate_deliveries)),
            ("mismatched_texts", Value::U(flow.mismatched_texts)),
            ("unknown_deliveries", Value::U(flow.unknown_deliveries)),
            ("receipts", Value::U(flow.receipts.len() as u64)),
            ("receipt_pct", pct(flow.receipts.len(), sent)),
            ("duplicate_receipts", Value::U(flow.duplicate_receipts)),
            ("unknown_receipts", Value::U(flow.unknown_receipts)),
            ("backpressure_retries", Value::U(flow.backpressure_retries)),
            ("first_send_ms", self.at(flow.first_send_at)),
            ("last_send_ms", self.at(flow.last_send_at)),
            ("first_send_epoch", first_send_epoch),
            ("sends_at_epoch0", Value::U(u64::from(flow.sends_at_epoch0))),
            ("latency_ms", summary(latency)),
            ("receipt_rtt_ms", summary(receipt_rtt)),
            ("undelivered_msg_ids", Value::A(undelivered)),
        ])
    }

    fn node_json(
        &self,
        side: Side,
        probes: &[Option<NodeProbe>; 2],
        metrics: &[Option<NodeMetrics>; 2],
    ) -> Value {
        let log = &self.logs[side.idx()];
        let connected_at = log.first_connected_at;
        let since_connected = |t: Option<Instant>| match (t, connected_at) {
            (Some(t), Some(c)) => Value::F(signed_ms(t, c)),
            _ => Value::Null,
        };
        let epoch = obj(vec![
            ("first_epoch0_ms", self.at(log.epoch0_first)),
            ("first_epoch1_ms", self.at(log.epoch1_first)),
            (
                "epoch1_after_connected_ms",
                since_connected(log.epoch1_first),
            ),
            ("last", opt_u8(log.epoch_last)),
        ]);
        let events = obj(vec![
            ("connected", Value::U(log.connected)),
            ("first_connected_ms", self.at(connected_at)),
            ("closed_session", Value::U(log.closed_session)),
            ("connect_abandoned", Value::U(log.abandoned)),
            ("connect_progress", Value::U(log.connect_progress)),
            ("signaling_received", Value::U(log.signaling)),
        ]);
        let metrics_value = metrics[side.idx()]
            .as_ref()
            .map_or(Value::Null, metrics_json);
        let probe_value = probes[side.idx()].as_ref().map_or(Value::Null, probe_json);
        obj(vec![
            ("events", events),
            ("epoch", epoch),
            ("metrics", metrics_value),
            ("probe", probe_value),
        ])
    }

    fn file_json(&self) -> Value {
        if self.cfg.arm != Arm::EkDosyaPtt {
            return Value::Null;
        }
        let f = &self.file;
        let between = |a: Option<Instant>, b: Option<Instant>| match (a, b) {
            (Some(a), Some(b)) => Value::F(ms(b.saturating_duration_since(a))),
            _ => Value::Null,
        };
        obj(vec![
            ("bytes", Value::U(f.source_len as u64)),
            ("sent_ms", self.at(f.sent_at)),
            ("progress_sent_ms", self.at(f.progress_sent_at)),
            ("received_ms", self.at(f.received_at)),
            ("delivered_ms", self.at(f.delivered_at)),
            ("send_to_received_ms", between(f.sent_at, f.received_at)),
            ("send_to_receipt_ms", between(f.sent_at, f.delivered_at)),
            ("received_size", Value::U(f.received_size)),
            (
                "bytes_match",
                f.bytes_match.map_or(Value::Null, Value::Bool),
            ),
            (
                "failures",
                Value::A(f.failures.iter().map(|s| Value::S(s.clone())).collect()),
            ),
        ])
    }

    fn ptt_json(&self) -> Value {
        if !matches!(self.cfg.arm, Arm::EkDosyaPtt | Arm::EkKesinti) {
            return Value::Null;
        }
        let p = &self.ptt;
        let press_to_first_audio = match (p.opened_at, p.first_bundle_at) {
            (Some(open), Some(first)) => Value::F(ms(first.saturating_duration_since(open))),
            _ => Value::Null,
        };
        obj(vec![
            ("bundles_planned", Value::U(u64::from(self.cfg.bundles))),
            ("pushed", Value::U(u64::from(p.pushed))),
            ("push_retries", Value::U(p.push_retries)),
            ("opened_ms", self.at(p.opened_at)),
            ("closed_ms", self.at(p.closed_at)),
            ("peer_opened_events", Value::U(p.peer_opened_events)),
            ("peer_opened_ms", self.at(p.peer_opened_at)),
            ("first_bundle_ms", self.at(p.first_bundle_at)),
            ("press_to_first_audio_ms", press_to_first_audio),
            ("received", Value::U(p.received.len() as u64)),
            ("received_overflow", Value::U(p.received_overflow)),
            ("unreadable_bundles", Value::U(p.unreadable_bundles)),
            ("in_order_complete", Value::Bool(p.in_order())),
            ("peer_closed_events", Value::U(p.peer_closed_events)),
            ("peer_closed_ms", self.at(p.peer_closed_at)),
        ])
    }

    fn outage_json(&self) -> Value {
        if self.cfg.arm != Arm::EkKesinti {
            return Value::Null;
        }
        let o = &self.outage;
        let flow = &self.flows[Side::Alice.idx()];
        // Kesinti SIRASINDA gönderilen metinler: gönderim anı [uygulandı, bitti) aralığında.
        let during: Vec<(u32, Instant)> = match (o.applied_at, o.restored_at) {
            (Some(applied), Some(restored)) => flow
                .sent
                .iter()
                .filter(|(_, (at, _))| *at >= applied && *at < restored)
                .map(|(id, (at, _))| (*id, *at))
                .collect(),
            _ => Vec::new(),
        };
        let after_restore: Vec<f64> = match o.restored_at {
            Some(restored) => during
                .iter()
                .filter_map(|(id, _)| flow.delivered.get(id))
                .map(|got| signed_ms(*got, restored))
                .collect(),
            None => Vec::new(),
        };
        let delivered_during = during
            .iter()
            .filter(|(id, _)| flow.delivered.contains_key(id))
            .count();
        let duration = match (o.applied_at, o.restored_at) {
            (Some(a), Some(r)) => Value::F(ms(r.saturating_duration_since(a))),
            _ => Value::Null,
        };
        obj(vec![
            ("requested_ms", self.at(o.requested_at)),
            ("applied_ms", self.at(o.applied_at)),
            ("restored_ms", self.at(o.restored_at)),
            ("duration_ms", duration),
            ("texts_sent_during", Value::U(during.len() as u64)),
            (
                "texts_delivered_of_during",
                Value::U(delivered_during as u64),
            ),
            ("delivery_after_restore_ms", summary(after_restore)),
        ])
    }
}

// ── Yardımcılar ──────────────────────────────────────────────────────────────────────────────

fn series(side: Side, range: Range<u32>) -> Vec<(Side, u32)> {
    range.map(|index| (side, index)).collect()
}

fn interleaved(range: Range<u32>) -> Vec<(Side, u32)> {
    range
        .flat_map(|index| [(Side::Alice, index), (Side::Bob, index)])
        .collect()
}

fn peer_epoch(metrics: &NodeMetrics, peer: &str) -> Option<u8> {
    metrics
        .connect_state
        .iter()
        .find(|state| state.peer_id == peer)
        .and_then(|state| state.epoch)
}

/// Kayıt Transport'a KABUL ETTİRİLEMEDİ (yeniden deneme kararı çağıranın): komut kuyruğu dolu,
/// Transport geri basıncı, outbox / gönderen penceresi dolu.
fn is_backpressure(error: &NodeError) -> bool {
    matches!(
        error,
        NodeError::Busy
            | NodeError::Messenger(
                MessengerError::Transport(TransportError::Backpressure)
                    | MessengerError::OutboxFull
                    | MessengerError::UnackedSpanFull { .. }
            )
    )
}

fn is_ptt_backpressure(error: &NodeError) -> bool {
    matches!(
        error,
        NodeError::Busy | NodeError::Ptt(PttError::Transport(TransportError::Backpressure))
    )
}

fn bundle_payload(index: u32, len: usize) -> Vec<u8> {
    let mut payload = vec![index.to_le_bytes()[0]; len];
    payload[..BUNDLE_INDEX_BYTES].copy_from_slice(&index.to_be_bytes());
    payload
}

fn bundle_index(payload: &[u8]) -> Option<u32> {
    payload
        .get(..BUNDLE_INDEX_BYTES)
        .and_then(|head| <[u8; BUNDLE_INDEX_BYTES]>::try_from(head).ok())
        .map(u32::from_be_bytes)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// `t − origin` (ms; `t` önceyse negatif).
fn signed_ms(t: Instant, origin: Instant) -> f64 {
    if t >= origin {
        ms(t - origin)
    } else {
        -ms(origin - t)
    }
}

fn pct(part: usize, whole: usize) -> Value {
    if whole == 0 {
        Value::Null
    } else {
        Value::F(100.0 * part as f64 / whole as f64)
    }
}

fn opt_u8(v: Option<u8>) -> Value {
    v.map_or(Value::Null, |v| Value::U(u64::from(v)))
}

/// n, en küçük, medyan, 95p, en büyük (ms). Yüzdelikler EN YAKIN SIRA (nearest-rank) yöntemiyle:
/// p-yüzdelik = artan sıradaki ⌈p·n⌉'inci değer.
fn summary(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let rank = |p: f64| -> Value {
        if n == 0 {
            return Value::Null;
        }
        let k = (p * n as f64).ceil() as usize;
        Value::F(values[k.clamp(1, n) - 1])
    };
    obj(vec![
        ("n", Value::U(n as u64)),
        ("min", values.first().map_or(Value::Null, |v| Value::F(*v))),
        ("median", rank(0.5)),
        ("p95", rank(0.95)),
        ("max", values.last().map_or(Value::Null, |v| Value::F(*v))),
    ])
}

fn metrics_json(m: &NodeMetrics) -> Value {
    obj(vec![
        ("epoch", opt_u8(m.epoch)),
        ("ctx_mismatch_drops", Value::U(m.ctx_mismatch_drops)),
        ("hub_frames_dropped", Value::U(m.hub_frames_dropped)),
        ("delivery_channel_drops", Value::U(m.delivery_channel_drops)),
        (
            "rtt_ms",
            m.rtt_ms.map_or(Value::Null, |v| Value::U(u64::from(v))),
        ),
        (
            "loss_pct",
            m.loss_pct.map_or(Value::Null, |v| Value::F(f64::from(v))),
        ),
        (
            "bitrate_kbps",
            m.bitrate_kbps
                .map_or(Value::Null, |v| Value::U(u64::from(v))),
        ),
        (
            "last_heartbeat_age_s",
            m.last_heartbeat_age_s.map_or(Value::Null, Value::U),
        ),
    ])
}

/// ARQ / kurtarma sayaçları (+ uç başına RTT alanları, `rtt` doğruysa).
fn diag_json(d: &ClientLiveDiag, rtt: bool) -> Value {
    let mut fields = vec![
        ("retransmits", Value::U(d.retransmits)),
        ("tail_probes", Value::U(d.tail_probes)),
        ("dups", Value::U(d.dups)),
        ("parity_emitted", Value::U(d.parity_emitted)),
        ("recovery_suppressed", Value::U(d.recovery_suppressed)),
        ("tx_retx_bytes", Value::U(d.tx_retx_bytes)),
    ];
    if rtt {
        let sampled = d.rtt_samples > 0;
        let us_to_ms = |us: u64| {
            if sampled {
                Value::F(us as f64 / 1000.0)
            } else {
                Value::Null
            }
        };
        let left = if d.left_preconverge_ns == 0 {
            Value::Null
        } else {
            Value::F(d.left_preconverge_ns as f64 / 1_000_000.0)
        };
        fields.push(("rtt_samples", Value::U(u64::from(d.rtt_samples))));
        fields.push(("srtt_ms", us_to_ms(d.srtt_us)));
        fields.push(("min_rtt_ms", us_to_ms(d.rtprop_us)));
        fields.push(("left_preconverge_ms", left));
    }
    obj(fields)
}

fn probe_json(p: &NodeProbe) -> Value {
    let mut total = p.retired;
    for session in &p.sessions {
        total.retransmits += session.diag.retransmits;
        total.tail_probes += session.diag.tail_probes;
        total.dups += session.diag.dups;
        total.parity_emitted += session.diag.parity_emitted;
        total.recovery_suppressed += session.diag.recovery_suppressed;
        total.tx_retx_bytes += session.diag.tx_retx_bytes;
    }
    let sessions = Value::A(
        p.sessions
            .iter()
            .map(|s| {
                obj(vec![
                    ("peer", s.peer_id.clone().map_or(Value::Null, Value::S)),
                    ("epoch", opt_u8(s.epoch)),
                    ("diag", diag_json(&s.diag, true)),
                ])
            })
            .collect(),
    );
    let m = &p.messenger;
    let app = obj(vec![
        ("resent_on_new_session", Value::U(m.resent_on_new_session)),
        ("resend_refused", Value::U(m.resend_refused)),
        ("duplicate_texts", Value::U(m.duplicate_texts)),
        ("below_window_texts", Value::U(m.below_window_texts)),
        ("receipts_sent", Value::U(m.receipts_sent)),
        ("receipts_unsent", Value::U(m.receipts_unsent)),
        ("receipts_ignored", Value::U(m.receipts_ignored)),
        ("records_dropped", Value::U(m.records_dropped)),
        ("units_unknown_session", Value::U(m.units_unknown_session)),
        ("peers_refused", Value::U(m.peers_refused)),
        ("store_errors", Value::U(m.store_errors)),
    ]);
    let events = obj(vec![
        ("ready", Value::U(p.events.ready)),
        ("closed", Value::U(p.events.closed)),
        ("renew_required", Value::U(p.events.renew_required)),
        ("connect_abandoned", Value::U(p.events.connect_abandoned)),
    ]);
    let crypto = obj(vec![
        ("inits_sent", Value::U(p.envelopes.inits_sent)),
        ("responses_sent", Value::U(p.envelopes.responses_sent)),
        ("identical_resends", Value::U(p.envelopes.identical_resends)),
    ]);
    obj(vec![
        ("sessions_live", Value::U(p.sessions.len() as u64)),
        ("sessions", sessions),
        ("retired", diag_json(&p.retired, false)),
        ("arq_total", diag_json(&total, false)),
        ("not_ready_drops", Value::U(p.not_ready_drops)),
        ("ctx_mismatch_drops", Value::U(p.ctx_mismatch_drops)),
        ("hub_frames_dropped", Value::U(p.hub_frames_dropped)),
        ("delivery_channel_drops", Value::U(p.delivery_channel_drops)),
        ("outstanding_units", Value::U(p.outstanding_units as u64)),
        ("app", app),
        (
            "app_retransmit",
            Value::U(m.resent_on_new_session + m.resend_refused),
        ),
        ("session_events", events),
        ("crypto", crypto),
        ("crypto_retransmit", Value::U(p.envelopes.identical_resends)),
    ])
}

// ── En küçük JSON yazıcısı (yeni bağımlılık YOK, D11) ────────────────────────────────────────

/// JSON değeri.
#[derive(Debug, Clone)]
enum Value {
    Null,
    Bool(bool),
    U(u64),
    F(f64),
    S(String),
    A(Vec<Value>),
    O(Vec<(String, Value)>),
}

fn obj(fields: Vec<(&str, Value)>) -> Value {
    Value::O(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

impl Value {
    fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::U(n) => {
                let _ = write!(out, "{n}");
            }
            Self::F(x) => {
                if x.is_finite() {
                    let _ = write!(out, "{x:.3}");
                } else {
                    out.push_str("null");
                }
            }
            Self::S(s) => escape(s, out),
            Self::A(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Self::O(fields) => {
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    escape(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ── Giriş ────────────────────────────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match parse_args(&args) {
        Ok(Parsed::Help) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Run(cfg)) => cfg,
        Err(e) => {
            eprintln!("int02_node: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let report = run(&cfg);
    println!("{}", report.json);
    match report.outcome {
        Outcome::Error(e) => {
            eprintln!("int02_node: {e}");
            ExitCode::from(1)
        }
        Outcome::Complete | Outcome::Incomplete => ExitCode::SUCCESS,
    }
}
