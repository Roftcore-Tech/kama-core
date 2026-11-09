//! [YOL3 · QK] G2 GÖSTERİM SAHNESİNİN quinn KOLU — `bench f0-quinn --role alice|bob …`.
//!
//! Amaç: Kama'nın gösterim sahnesini (`a_r12k_d150_l20_foto150k`: ısınma TEXT'leri, 150 KB dosya,
//! TEXT + sinyal + iki PTT basışı, karşı örnek) AYNI ölçülerle quinn üzerinde koşmak.
//!
//! f0_flows (`crates/kama-integration/examples/f0_flows.rs`) ile AYNI olanlar:
//! * argümanlar (hücrenin `F0A_ARGS` dizesi aynen geçer; Kama kollarının `--e1-arm`/`--fec-off`
//!   ekleri burada yoktur),
//! * akış takvimi (`build_plan`, `push_spurt`, `spurt_len` aynen), yük gövdeleri ve boyları
//!   (`text_body`, `sig_body`, `bundle_body`, `file_byte` aynen),
//! * eşzamanlama işaretleri (`sync/bob.up`, `sync/t0`, `sync/alice.done`, `sync/bob.exit`,
//!   `sync/bob.state`; dosya sistemi, ağ dışı),
//! * tamamlanma kuralı (plan bitti ∧ bob her şeyi aldı ∧ alındılar geldi ya da `receipt_grace`),
//!   pencere (`plan sonu + window`), sansür (analizde),
//! * çıktı (`alice.jsonl`/`bob.jsonl`: `hdr`, `send`, `rx`, `rcpt`, `ev`, `mark`; `alice.json`/
//!   `bob.json` özeti) — `scripts/ktp2/f0/a/f0a_analyze.py`, `f0a_tam.py` ve
//!   `scripts/yol3/tesis/oku.py` değişmeden okur. Kama'ya özgü `ts` (motor sondası) ve `tr` (birim
//!   izi) satırları YOKTUR; yerine `qs` (quinn bağlantı sayaçları) yazılır, analizörler okumaz.
//!
//! # Taşıyıcı (mevcut bench tanımı; `quic.rs` `transport_config` + `rival` TransportAck)
//!
//! quinn 0.11.11 / quinn-proto 0.11.18, TLS 1.3 (rustls/ring, self-signed), idle 300 s,
//! `initial_rtt` 333 ms (quinn varsayılanı), `initial_mtu` 1452, PMTUD kapalı, GSO kapalı,
//! eşzamanlı uni-stream kredisi 512. Kollar (`--q-kol`, iki uçta aynı):
//! * `Q_bbr` — BBR, ilk pencere quinn BBR varsayılanı 200 × 1200 = 240 000 B.
//! * `Q_bbr_rfc` — BBR, ilk pencere RFC 9002 §7.2 `min(10·MSS, max(2·MSS, 14720))`,
//!   MSS = `initial_mtu` 1452 → 14 520 B.
//! * `Q_cubic` — Cubic, ilk pencere quinn varsayılanı 12 000 B (aynı formül, MSS = 1200).
//!
//! # Öğe → stream (mesaj başına tek-yönlü stream; bench B ızgarası tanımı)
//!
//! Her öğe ayrı uni-stream: `tür u8 ‖ gövde`, ardından FIN.
//! * TEXT (W/T/K): `'T' ‖ mid u32 ‖ text_body`. bob okuyunca alındıyı ayrı stream ile döner:
//!   `'R' ‖ mid u32` (Kama Messenger RECEIPT'inin karşılığı; ters yön yükü de aynı).
//! * sinyal: `'S' ‖ sig_body`. PTT açılış / kapanış: `'O'|'C' ‖ basış u16`. Demet: `'P' ‖
//!   bundle_body` (her demet ayrı stream; basış içinde HOL yok).
//! * dosya: `'F' ‖ boy u64 ‖ bayt` tek stream; bob deseni doğrulayınca `'D'` alındısı (alice
//!   `file_delivered`).
//!
//! Öncelik: varsayılan YOK (`--q-oncelik off`): bütün stream'ler 0; quinn `send_fairness` (varsayılan
//! açık) eşit öncelikte stream'leri paket düzeyinde sırayla doldurur. Kama'da uygulama önceliği
//! yoktur (yeni veri kuyruğu FIFO); `--q-oncelik on` mixed.rs'in PTT 2 > TEXT/sinyal 1 > dosya 0
//! beyanını açar (betimsel, hücresi üretilmez).
//!
//! # Beyanlı farklar (Kama kollarına göre)
//!
//! * El sıkışma: quinn el sıkışması veri yolunda, netem İÇİNDEN geçer (kayıpta saniyeler sürebilir;
//!   `--connect-timeout-s` aşılırsa koşu hata). Kama kollarında zarflar Unix soketinden, netem
//!   DIŞINDAN geçer (f0_flows modül belgesi, DUR-K4G-11 → B). İkisinde de `t0` = bağlantı kurulduktan
//!   sonra: kurulum ölçüm penceresinin DIŞINDA. Kurulum süresi `t0` işaretine (`setup_ms`) yazılır.
//!   quinn `t0`'a el sıkışmasından gelen bir RTT örneğiyle girer; Kama soğuk girer (PreConverge).
//! * Sıralama: quinn stream'leri birbirinden bağımsızdır (akışlar arası HOL yok); Kama ürün yolunda
//!   oturum geneli sıralı teslim (S1 Reorder) vardır. Bu quinn'in doğal kullanımıdır, eşitlenmez.
//! * Karşı örnek tetiği: Kama `tx_queue_len == 0 ∧ unacked ≥ N` (motor sondası). quinn bu iç
//!   durumları açmaz; VEKİL [çıkarım]: (a) yeni veri tükendi ≈ `udp_tx.bytes − lost_bytes −
//!   datagrams × 27 B` (kısa başlık + AEAD etiketi tahmini; `t0`'dan delta) ≥ quinn'e verilen
//!   uygulama baytı; (b) teyitsiz ≥ N birim ≈ dosyanın bob'a henüz ulaşmamış kısmı (`sync/bob.state`
//!   `FB`) ≥ N × 1013 B. Diğer şartlar (dosya verildi, bob bitirmedi, `t ≥ file_at + 1 s`) aynen.
//! * Durma senaryoları (`dosyali_dur`, `dosyasiz_dur`) öykünülmez: tipli hata.
//! * Zaman damgaları: `send.t` = uygulamanın quinn'e çağrı anı (stream açılışından önce), `rx.t` =
//!   bob stream'i FIN'e dek okuduğu an, `rcpt.t` = alındı stream'i alice'te okunduğu an. İki süreç
//!   aynı makinede, Unix epoch ms (f0_flows `Clock` ile aynı çapa).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write as _};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
use quinn::{Connection, ConnectionStats, Endpoint, RecvStream};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

use crate::budget::BudgetMode;
use crate::error::BenchError;
use crate::rival::{Delivery, RivalTuning};
use crate::transport::Cc;

/// Çıktı şeması (f0_flows ile aynı satır biçimi; taşıyıcı başlıkta `quinn` nesnesiyle ayrılır).
const SCHEMA: &str = "f0a-flows-v1";
const ALICE: &str = "alice";
const BOB: &str = "bob";

const SYNC_DIR: &str = "sync";
const MARK_BOB_UP: &str = "sync/bob.up";
const MARK_T0: &str = "sync/t0";
const MARK_ALICE_DONE: &str = "sync/alice.done";
const MARK_BOB_EXIT: &str = "sync/bob.exit";
const BOB_STATE: &str = "sync/bob.state";

/// alice olay kuyruğu tavanı (dolarsa olay düşer, `listener_drops` sayılır; f0_flows ile aynı).
const EVENT_QUEUE_CAP: usize = 16_384;
const PUMP_STEP: Duration = Duration::from_millis(5);
const MARK_POLL: Duration = Duration::from_millis(20);
const BOB_EXIT_WAIT: Duration = Duration::from_secs(20);
const BOB_SLACK: Duration = Duration::from_secs(60);
const CLOSE_WAIT: Duration = Duration::from_secs(3);
/// Karşı örnek basışının sürücü numarası (f0_flows `KARSI_SPURT`).
const KARSI_SPURT: u32 = 90;
const SPURT_GAP: Duration = Duration::from_millis(200);
const FILE_PATTERN: usize = 251;
const TEXT_PREFIX: usize = 11;

/// Girdi tavanları (SR-AUDIT-001; f0_flows ile aynı).
const MAX_ITEMS: u32 = 5_000;
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SPURTS: usize = 8;
/// Dosya dışı tek öğe stream'inin okuma tavanı (bounds-before-alloc; en büyük öğe < 1 KB).
const MAX_MSG: usize = 64 * 1024;
const READ_CHUNK: usize = 64 * 1024;

/// Kama dosya parçası birimi (B) — karşı örnek "teyitsiz ≥ N birim" vekilinin birimi.
const FILE_UNIT: u64 = 1013;
/// quinn 1-RTT kısa başlık + AEAD etiketi tahmini (B): 1 bayrak + 8 DCID (quinn varsayılan CID
/// boyu) + 2 PN + 16 etiket. Yalnız karşı örnek vekilinde kullanılır [çıkarım].
const Q_SHORT_HDR_EST: u64 = 27;
/// Cargo.lock ile pinli (quinn 0.11.11'in bağımlılığı).
const QUINN_PROTO_VERSION: &str = "0.11.18";
/// quinn-proto 0.11.18 BBR varsayılan ilk penceresi: 200 × BASE_DATAGRAM_SIZE (1200).
const QUINN_BBR_DEFAULT_IW: u64 = 200 * 1200;
/// quinn-proto 0.11.18 Cubic varsayılanı: `14720.clamp(2 × 1200, 10 × 1200)`.
const QUINN_CUBIC_DEFAULT_IW: u64 = 12_000;
/// quinn varsayılan `initial_rtt` (ms; quic.rs değiştirmez).
const QUINN_INITIAL_RTT_MS: u64 = 333;
/// quic.rs TransportAck yolu: eşzamanlı uni-stream kredisi.
const QUINN_UNI_STREAMS: u32 = 512;

/// Tel çerçevesi türleri (stream'in ilk baytı).
const K_TEXT: u8 = b'T';
const K_SIG: u8 = b'S';
const K_OPEN: u8 = b'O';
const K_PUSH: u8 = b'P';
const K_CLOSE: u8 = b'C';
const K_FILE: u8 = b'F';
const K_RCPT: u8 = b'R';
const K_FILE_RCPT: u8 = b'D';

// ── Argümanlar ───────────────────────────────────────────────────────────────────────────────

/// Rol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Role {
    /// İstemci, gönderen.
    Alice,
    /// Sunucu, alıcı.
    Bob,
}

/// Senaryo (f0_flows adları).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scenario {
    /// TEXT + sinyal + PTT, dosya yok.
    #[value(name = "dosyasiz")]
    Dosyasiz,
    /// Aynısı + bir dosya.
    #[value(name = "dosyali")]
    Dosyali,
    /// Dosyalı + alıcı durması (bu kolda öykünülmez).
    #[value(name = "dosyali_dur")]
    DosyaliDur,
    /// Dosyasız + alıcı durması (bu kolda öykünülmez).
    #[value(name = "dosyasiz_dur")]
    DosyasizDur,
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Dosyasiz => "dosyasiz",
            Self::Dosyali => "dosyali",
            Self::DosyaliDur => "dosyali_dur",
            Self::DosyasizDur => "dosyasiz_dur",
        }
    }

    fn has_file(self) -> bool {
        matches!(self, Self::Dosyali | Self::DosyaliDur)
    }

    fn has_stall(self) -> bool {
        matches!(self, Self::DosyaliDur | Self::DosyasizDur)
    }
}

/// `on|off` (f0_flows biçimi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OnOff {
    /// Açık.
    On,
    /// Kapalı.
    Off,
}

impl OnOff {
    fn on(self) -> bool {
        self == Self::On
    }
}

/// quinn kolu (`--q-kol`; iki uçta aynı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum QArm {
    /// BBR, ilk pencere quinn varsayılanı (240 000 B).
    #[value(name = "Q_bbr")]
    Bbr,
    /// BBR, ilk pencere RFC 9002 §7.2 (MSS 1452 → 14 520 B).
    #[value(name = "Q_bbr_rfc")]
    BbrRfc,
    /// Cubic, ilk pencere quinn varsayılanı (12 000 B).
    #[value(name = "Q_cubic")]
    Cubic,
}

impl QArm {
    fn name(self) -> &'static str {
        match self {
            Self::Bbr => "Q_bbr",
            Self::BbrRfc => "Q_bbr_rfc",
            Self::Cubic => "Q_cubic",
        }
    }

    fn cc(self) -> Cc {
        match self {
            Self::Bbr | Self::BbrRfc => Cc::Bbr,
            Self::Cubic => Cc::Cubic,
        }
    }

    /// `rival` üzerinden quic.rs'e verilen ilk pencere; `None` = kütüphane varsayılanı.
    fn iw_override(self) -> Option<u64> {
        match self {
            Self::BbrRfc => Some(rfc9002_iw(u64::from(crate::quic::INITIAL_MTU))),
            Self::Bbr | Self::Cubic => None,
        }
    }

    /// Etkin ilk pencere (B).
    fn iw_bytes(self) -> u64 {
        self.iw_override().unwrap_or(match self.cc() {
            Cc::Bbr => QUINN_BBR_DEFAULT_IW,
            Cc::Cubic => QUINN_CUBIC_DEFAULT_IW,
        })
    }

    fn iw_rule(self) -> &'static str {
        match self {
            Self::Bbr => "quinn BBR varsayılanı 200×1200",
            Self::BbrRfc => "RFC 9002 §7.2 min(10·MSS, max(2·MSS, 14720)), MSS = initial_mtu",
            Self::Cubic => "quinn Cubic varsayılanı 14720.clamp(2×1200, 10×1200)",
        }
    }
}

/// RFC 9002 §7.2 ilk pencere: `min(10 · mss, max(2 · mss, 14720))`.
fn rfc9002_iw(mss: u64) -> u64 {
    mss.saturating_mul(10)
        .min(mss.saturating_mul(2).max(14_720))
}

/// `f0-quinn` argümanları — adlar ve varsayılanlar f0_flows'un AYNISI (+ `--q-kol`, `--q-oncelik`).
#[derive(Debug, Args)]
pub struct F0qArgs {
    /// alice (istemci, gönderen) | bob (sunucu, alıcı).
    #[arg(long, value_enum)]
    pub role: Role,
    /// Koşu dizini (süreç buraya geçer).
    #[arg(long)]
    pub dir: PathBuf,
    /// alice bağlanma adresi.
    #[arg(long)]
    pub alice_addr: SocketAddr,
    /// bob dinleme adresi.
    #[arg(long)]
    pub bob_addr: SocketAddr,
    /// Senaryo.
    #[arg(long, value_enum)]
    pub scenario: Scenario,
    /// Profil etiketi (hücre geometrisi; aynen yazılır).
    #[arg(long)]
    pub profile: String,
    /// Tekrar numarası.
    #[arg(long, default_value_t = 0)]
    pub rep: u32,
    /// Dosya boyu (B).
    #[arg(long, default_value_t = 524_700)]
    pub file_bytes: usize,
    /// Dosya sunum anı (ms, t0'dan).
    #[arg(long, default_value_t = 5_000)]
    pub file_at_ms: u64,
    /// Isınma TEXT sayısı.
    #[arg(long, default_value_t = 3)]
    pub warmup_texts: u32,
    /// Isınma aralığı (ms).
    #[arg(long, default_value_t = 500)]
    pub warmup_every_ms: u64,
    /// TEXT sayısı.
    #[arg(long, default_value_t = 40)]
    pub texts: u32,
    /// İlk TEXT anı (ms).
    #[arg(long, default_value_t = 6_000)]
    pub text_at_ms: u64,
    /// TEXT aralığı (ms).
    #[arg(long, default_value_t = 2_500)]
    pub text_every_ms: u64,
    /// TEXT boyu (B).
    #[arg(long, default_value_t = 64)]
    pub text_bytes: usize,
    /// Sinyal sayısı.
    #[arg(long, default_value_t = 10)]
    pub sigs: u32,
    /// İlk sinyal anı (ms).
    #[arg(long, default_value_t = 7_000)]
    pub sig_at_ms: u64,
    /// Sinyal aralığı (ms).
    #[arg(long, default_value_t = 10_000)]
    pub sig_every_ms: u64,
    /// Sinyal boyu (B).
    #[arg(long, default_value_t = 120)]
    pub sig_bytes: usize,
    /// PTT basış anları (ms, virgüllü; "" ya da "none" = basış yok).
    #[arg(long, default_value = "15000,60000")]
    pub ptt_at_ms: String,
    /// Basış başına demet.
    #[arg(long, default_value_t = 25)]
    pub ptt_bundles: u32,
    /// Demet aralığı (ms).
    #[arg(long, default_value_t = 200)]
    pub ptt_bundle_ms: u8,
    /// Demet boyu (B).
    #[arg(long, default_value_t = 40)]
    pub ptt_bytes: usize,
    /// Durma anı (ms; yalnız `_dur` senaryoları — bu kolda reddedilir).
    #[arg(long, default_value_t = 20_000)]
    pub stall_at_ms: u64,
    /// Durma süresi (ms).
    #[arg(long, default_value_t = 30_000)]
    pub stall_ms: u64,
    /// Karşı örnek.
    #[arg(long, value_enum, default_value_t = OnOff::On)]
    pub karsi: OnOff,
    /// Karşı örnek TEXT sayısı.
    #[arg(long, default_value_t = 5)]
    pub karsi_texts: u32,
    /// Karşı örnek TEXT aralığı (ms).
    #[arg(long, default_value_t = 500)]
    pub karsi_every_ms: u64,
    /// Karşı örnek basışının demet sayısı.
    #[arg(long, default_value_t = 10)]
    pub karsi_bundles: u32,
    /// Karşı örnek tetiği: teyitsiz birim eşiği (quinn'de vekil, modül belgesi).
    #[arg(long, default_value_t = 3)]
    pub karsi_min_unacked: u32,
    /// Karşı örnek TEXT boyu (B).
    #[arg(long, default_value_t = 48)]
    pub karsi_text_bytes: usize,
    /// Plan sonrası pencere (s).
    #[arg(long, default_value_t = 300)]
    pub window_s: u64,
    /// Bağlanma tavanı (s; quinn el sıkışması dahil).
    #[arg(long, default_value_t = 30)]
    pub connect_timeout_s: u64,
    /// Sonda aralığı (ms; `qs` satırı, bob durumu, karşı örnek denetimi).
    #[arg(long, default_value_t = 200)]
    pub probe_ms: u64,
    /// f0_flows canlı tanı bayrağı: kabul edilir ve başlığa yazılır; quinn'de etkisi yok.
    #[arg(long, value_enum, default_value_t = OnOff::On)]
    pub live_diag: OnOff,
    /// Alındı bekleme payı (s).
    #[arg(long, default_value_t = 30)]
    pub receipt_grace_s: u64,
    /// quinn kolu.
    #[arg(long, value_enum)]
    pub q_kol: QArm,
    /// Stream önceliği PTT 2 > TEXT/sinyal 1 > dosya 0 (varsayılan kapalı: hepsi 0).
    #[arg(long, value_enum, default_value_t = OnOff::Off)]
    pub q_oncelik: OnOff,
}

/// Çözülmüş yapılandırma (f0_flows `Config` alanlarıyla aynı anlam).
#[derive(Debug, Clone)]
struct Cfg {
    role: Role,
    dir: PathBuf,
    alice_addr: SocketAddr,
    bob_addr: SocketAddr,
    scenario: Scenario,
    profile: String,
    rep: u32,
    file_bytes: usize,
    file_at: Duration,
    warmup_texts: u32,
    warmup_every: Duration,
    texts: u32,
    text_at: Duration,
    text_every: Duration,
    text_bytes: usize,
    sigs: u32,
    sig_at: Duration,
    sig_every: Duration,
    sig_bytes: usize,
    ptt_at: Vec<Duration>,
    ptt_bundles: u32,
    ptt_bundle_ms: u8,
    ptt_bytes: usize,
    stall_at: Duration,
    stall: Duration,
    karsi: bool,
    karsi_texts: u32,
    karsi_every: Duration,
    karsi_bundles: u32,
    karsi_min_unacked: u32,
    karsi_text_bytes: usize,
    window: Duration,
    connect_timeout: Duration,
    probe_every: Duration,
    live_diag: bool,
    receipt_grace: Duration,
    arm: QArm,
    prio: bool,
}

fn parse_ptt_at(list: &str) -> Result<Vec<Duration>, String> {
    if list.is_empty() || list == "none" {
        return Ok(Vec::new());
    }
    list.split(',')
        .map(|v| {
            v.trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| format!("--ptt-at-ms değeri sayı değil: {v:?}"))
        })
        .collect()
}

impl Cfg {
    fn from_args(a: &F0qArgs) -> Result<Self, String> {
        let ms = Duration::from_millis;
        let cfg = Self {
            role: a.role,
            dir: a.dir.clone(),
            alice_addr: a.alice_addr,
            bob_addr: a.bob_addr,
            scenario: a.scenario,
            profile: a.profile.clone(),
            rep: a.rep,
            file_bytes: a.file_bytes,
            file_at: ms(a.file_at_ms),
            warmup_texts: a.warmup_texts,
            warmup_every: ms(a.warmup_every_ms),
            texts: a.texts,
            text_at: ms(a.text_at_ms),
            text_every: ms(a.text_every_ms),
            text_bytes: a.text_bytes,
            sigs: a.sigs,
            sig_at: ms(a.sig_at_ms),
            sig_every: ms(a.sig_every_ms),
            sig_bytes: a.sig_bytes,
            ptt_at: parse_ptt_at(&a.ptt_at_ms)?,
            ptt_bundles: a.ptt_bundles,
            ptt_bundle_ms: a.ptt_bundle_ms,
            ptt_bytes: a.ptt_bytes,
            stall_at: ms(a.stall_at_ms),
            stall: ms(a.stall_ms),
            karsi: a.karsi.on(),
            karsi_texts: a.karsi_texts,
            karsi_every: ms(a.karsi_every_ms),
            karsi_bundles: a.karsi_bundles,
            karsi_min_unacked: a.karsi_min_unacked,
            karsi_text_bytes: a.karsi_text_bytes,
            window: Duration::from_secs(a.window_s),
            connect_timeout: Duration::from_secs(a.connect_timeout_s),
            probe_every: ms(a.probe_ms),
            live_diag: a.live_diag.on(),
            receipt_grace: Duration::from_secs(a.receipt_grace_s),
            arm: a.q_kol,
            prio: a.q_oncelik.on(),
        };
        validate(&cfg)?;
        Ok(cfg)
    }
}

/// f0_flows `validate` aynen + durma senaryolarının reddi.
fn validate(cfg: &Cfg) -> Result<(), String> {
    for (flag, n) in [
        ("--warmup-texts", cfg.warmup_texts),
        ("--texts", cfg.texts),
        ("--sigs", cfg.sigs),
        ("--ptt-bundles", cfg.ptt_bundles),
        ("--karsi-texts", cfg.karsi_texts),
        ("--karsi-bundles", cfg.karsi_bundles),
    ] {
        if n > MAX_ITEMS {
            return Err(format!("{flag} ≤ {MAX_ITEMS} olmalı"));
        }
    }
    if cfg.text_bytes < TEXT_PREFIX || cfg.karsi_text_bytes < TEXT_PREFIX {
        return Err(format!("TEXT boyu ≥ {TEXT_PREFIX} olmalı"));
    }
    if cfg.sig_bytes < 8 || cfg.ptt_bytes < 8 {
        return Err("--sig-bytes ve --ptt-bytes ≥ 8 olmalı".to_owned());
    }
    if cfg.text_bytes > MAX_MSG / 2 || cfg.sig_bytes > MAX_MSG / 2 || cfg.ptt_bytes > MAX_MSG / 2 {
        return Err(format!("öğe boyu ≤ {} olmalı", MAX_MSG / 2));
    }
    if cfg.scenario.has_file() && (cfg.file_bytes == 0 || cfg.file_bytes > MAX_FILE_BYTES) {
        return Err(format!("--file-bytes 1..={MAX_FILE_BYTES} olmalı"));
    }
    if cfg.ptt_at.len() > MAX_SPURTS {
        return Err(format!("--ptt-at-ms en çok {MAX_SPURTS} basış"));
    }
    if cfg.ptt_bundle_ms == 0 {
        return Err("--ptt-bundle-ms > 0 olmalı".to_owned());
    }
    if cfg.probe_every < Duration::from_millis(50) {
        return Err("--probe-ms ≥ 50 olmalı".to_owned());
    }
    if cfg.alice_addr == cfg.bob_addr {
        return Err("--alice-addr ve --bob-addr farklı olmalı".to_owned());
    }
    let len = spurt_len(cfg, cfg.ptt_bundles);
    for pair in cfg.ptt_at.windows(2) {
        if pair[1] < pair[0] + len + SPURT_GAP {
            return Err("--ptt-at-ms artan ve çakışmasız olmalı".to_owned());
        }
    }
    if cfg.scenario.has_stall() {
        return Err(format!(
            "senaryo {}: alıcı durması quinn kolunda öykünülmez (yalnız dosyasiz, dosyali)",
            cfg.scenario.name()
        ));
    }
    Ok(())
}

/// Bir basışın süresi: açılış · `n` demet · kapanış (f0_flows aynen).
fn spurt_len(cfg: &Cfg, bundles: u32) -> Duration {
    Duration::from_millis(u64::from(cfg.ptt_bundle_ms)).saturating_mul(bundles.saturating_add(1))
}

// ── Saat, çıktı, işaretler ───────────────────────────────────────────────────────────────────

/// Süreç-içi `Instant` → Unix epoch (f0_flows `Clock` aynen).
#[derive(Debug, Clone, Copy)]
struct Clock {
    inst0: Instant,
    unix0_ns: u64,
}

impl Clock {
    fn new() -> Self {
        let inst0 = Instant::now();
        let unix0_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
        Self { inst0, unix0_ns }
    }

    fn unix_ns(&self, t: Instant) -> u64 {
        match t.checked_duration_since(self.inst0) {
            Some(d) => self
                .unix0_ns
                .saturating_add(u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)),
            None => self.unix0_ns.saturating_sub(
                u64::try_from(self.inst0.duration_since(t).as_nanos()).unwrap_or(u64::MAX),
            ),
        }
    }

    fn ms(&self, t: Instant) -> f64 {
        self.unix_ns(t) as f64 / 1e6
    }
}

fn js(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"?\"".to_owned())
}

fn opt_ms(clock: &Clock, t: Option<Instant>) -> String {
    t.map_or_else(|| "null".to_owned(), |t| format!("{:.3}", clock.ms(t)))
}

fn ns_ms(ns: u64) -> String {
    if ns == 0 {
        "null".to_owned()
    } else {
        format!("{:.3}", ns as f64 / 1e6)
    }
}

/// Satır başına bir JSON kaydı (tamponlu; sürücü döngüsü saniyede bir boşaltır).
struct Out {
    w: BufWriter<fs::File>,
    lines: u64,
    errors: u64,
}

impl Out {
    fn create(path: &str) -> Result<Self, String> {
        let file = fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
        Ok(Self {
            w: BufWriter::new(file),
            lines: 0,
            errors: 0,
        })
    }

    fn line(&mut self, s: &str) {
        if writeln!(self.w, "{s}").is_ok() {
            self.lines += 1;
        } else {
            self.errors += 1;
        }
    }

    fn flush(&mut self) {
        if self.w.flush().is_err() {
            self.errors += 1;
        }
    }
}

/// Görevler arası paylaşılan çıktı. Kilit hiçbir `await` boyunca tutulmaz.
type SharedOut = Arc<Mutex<Out>>;

fn emit(out: &SharedOut, line: &str) {
    if let Ok(mut o) = out.lock() {
        o.line(line);
    }
}

fn flush(out: &SharedOut) {
    if let Ok(mut o) = out.lock() {
        o.flush();
    }
}

fn out_counts(out: &SharedOut) -> (u64, u64) {
    out.lock().map_or((0, 0), |o| (o.lines, o.errors))
}

fn mark_line(clock: &Clock, at: Instant, what: &str, extra: &str) -> String {
    format!(
        "{{\"k\":\"mark\",\"t\":{:.3},\"m\":{}{extra}}}",
        clock.ms(at),
        js(what)
    )
}

/// Atomik yazım: geçici dosya + yeniden adlandırma.
fn write_marker(path: &str, body: &str) -> Result<(), String> {
    let tmp = format!("{path}.tmp");
    fs::write(&tmp, body).map_err(|e| format!("{tmp}: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("{path}: {e}"))
}

fn read_marker(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

async fn wait_marker(path: &str, limit: Duration) -> Option<String> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(body) = read_marker(path) {
            return Some(body);
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(MARK_POLL).await;
    }
}

// ── Öğe kodlaması (f0_flows aynen) ───────────────────────────────────────────────────────────

/// TEXT gövdesi: `f0a <sınıf><sıra:05> ` + `.` dolgusu, tam `len` bayt (ASCII).
fn text_body(class: char, idx: u32, len: usize) -> String {
    let mut s = format!("f0a {class}{idx:05} ");
    while s.len() < len {
        s.push('.');
    }
    s.truncate(len.max(TEXT_PREFIX));
    s
}

fn parse_text(text: &str) -> Option<(char, u32)> {
    let rest = text.strip_prefix("f0a ")?;
    let mut chars = rest.chars();
    let class = chars.next()?;
    let digits = rest.get(class.len_utf8()..class.len_utf8() + 5)?;
    Some((class, digits.parse().ok()?))
}

/// Sinyal blob'u: `S` · sıra u32 · dolgu.
fn sig_body(idx: u32, len: usize) -> Vec<u8> {
    let mut b = vec![0x5A; len];
    b[0] = b'S';
    b[1..5].copy_from_slice(&idx.to_be_bytes());
    b
}

fn parse_sig(bytes: &[u8]) -> Option<u32> {
    if bytes.first() != Some(&b'S') {
        return None;
    }
    Some(u32::from_be_bytes(bytes.get(1..5)?.try_into().ok()?))
}

/// PTT demeti: basış u16 · sıra u16 · dolgu.
fn bundle_body(spurt: u32, idx: u32, len: usize) -> Vec<u8> {
    let mut b = vec![0xA5; len];
    b[0..2].copy_from_slice(&u16::try_from(spurt).unwrap_or(u16::MAX).to_be_bytes());
    b[2..4].copy_from_slice(&u16::try_from(idx).unwrap_or(u16::MAX).to_be_bytes());
    b
}

fn parse_bundle(bytes: &[u8]) -> Option<(u32, u32)> {
    let spurt = u16::from_be_bytes(bytes.get(0..2)?.try_into().ok()?);
    let idx = u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?);
    Some((u32::from(spurt), u32::from(idx)))
}

fn file_byte(i: usize) -> u8 {
    u8::try_from(i % FILE_PATTERN).unwrap_or(0)
}

fn frame(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(1 + body.len());
    f.push(kind);
    f.extend_from_slice(body);
    f
}

fn spurt_u16(spurt: u32) -> [u8; 2] {
    u16::try_from(spurt).unwrap_or(u16::MAX).to_be_bytes()
}

// ── Plan (f0_flows aynen) ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    Text { class: char, idx: u32 },
    Sig { idx: u32 },
    PttOpen { spurt: u32 },
    PttPush { spurt: u32, idx: u32 },
    PttClose { spurt: u32 },
    File,
}

impl Act {
    /// `--q-oncelik on` öncelikleri (mixed.rs beyanı).
    fn priority(self) -> i32 {
        match self {
            Self::PttOpen { .. } | Self::PttPush { .. } | Self::PttClose { .. } => 2,
            Self::Text { .. } | Self::Sig { .. } => 1,
            Self::File => 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Action {
    /// t0'a göre vade.
    due: Duration,
    act: Act,
}

fn push_spurt(plan: &mut Vec<Action>, cfg: &Cfg, spurt: u32, start: Duration, bundles: u32) {
    let step = Duration::from_millis(u64::from(cfg.ptt_bundle_ms));
    plan.push(Action {
        due: start,
        act: Act::PttOpen { spurt },
    });
    for idx in 0..bundles {
        plan.push(Action {
            due: start + step.saturating_mul(idx + 1),
            act: Act::PttPush { spurt, idx },
        });
    }
    plan.push(Action {
        due: start + step.saturating_mul(bundles + 1),
        act: Act::PttClose { spurt },
    });
}

fn build_plan(cfg: &Cfg) -> Vec<Action> {
    let mut plan = Vec::new();
    for idx in 0..cfg.warmup_texts {
        plan.push(Action {
            due: cfg.warmup_every.saturating_mul(idx),
            act: Act::Text { class: 'W', idx },
        });
    }
    if cfg.scenario.has_file() {
        plan.push(Action {
            due: cfg.file_at,
            act: Act::File,
        });
    }
    for idx in 0..cfg.texts {
        plan.push(Action {
            due: cfg.text_at + cfg.text_every.saturating_mul(idx),
            act: Act::Text { class: 'T', idx },
        });
    }
    for idx in 0..cfg.sigs {
        plan.push(Action {
            due: cfg.sig_at + cfg.sig_every.saturating_mul(idx),
            act: Act::Sig { idx },
        });
    }
    for (spurt, start) in cfg.ptt_at.iter().enumerate() {
        let spurt = u32::try_from(spurt).unwrap_or(u32::MAX);
        push_spurt(&mut plan, cfg, spurt, *start, cfg.ptt_bundles);
    }
    plan.sort_by_key(|a| a.due);
    plan
}

/// Karşı örnek eylemleri (f0_flows `maybe_karsi` gövdesi aynen): `base`'den K TEXT'leri ve
/// planlı basışlarla çakışmayan ilk başlangıçta KP basışı.
fn karsi_actions(cfg: &Cfg, base: Duration) -> Vec<Action> {
    let mut added = Vec::new();
    for idx in 0..cfg.karsi_texts {
        added.push(Action {
            due: base + cfg.karsi_every.saturating_mul(idx),
            act: Act::Text { class: 'K', idx },
        });
    }
    if cfg.karsi_bundles > 0 {
        let len = spurt_len(cfg, cfg.karsi_bundles);
        let mut start = base + Duration::from_millis(250);
        let mut spans: Vec<(Duration, Duration)> = cfg
            .ptt_at
            .iter()
            .map(|s| (*s, *s + spurt_len(cfg, cfg.ptt_bundles)))
            .collect();
        spans.sort();
        for (s, e) in spans {
            if start < e + SPURT_GAP && start + len + SPURT_GAP > s {
                start = e + SPURT_GAP;
            }
        }
        push_spurt(&mut added, cfg, KARSI_SPURT, start, cfg.karsi_bundles);
    }
    added
}

/// Karşı örnek vekilinin girdileri (modül belgesi; [çıkarım]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KarsiProbe {
    /// quinn'e verilen uygulama baytı (t0'dan beri, bütün stream'ler).
    app_out: u64,
    /// t0'dan beri UDP gönderim baytı.
    tx_bytes: u64,
    /// t0'dan beri kayıp ilan edilen bayt.
    lost_bytes: u64,
    /// t0'dan beri UDP datagram sayısı.
    tx_datagrams: u64,
    /// Dosya boyu.
    file_bytes: u64,
    /// bob'un dosyadan aldığı bayt (`sync/bob.state` `FB`).
    bob_file_rx: u64,
}

impl KarsiProbe {
    /// Henüz tele çıkmamış uygulama baytı tahmini (negatif = tükendi).
    fn unsent_est(self) -> i128 {
        let wire_new = i128::from(self.tx_bytes)
            - i128::from(self.lost_bytes)
            - i128::from(self.tx_datagrams) * i128::from(Q_SHORT_HDR_EST);
        i128::from(self.app_out) - wire_new
    }

    /// (a) yeni veri tükendi ∧ (b) dosyanın bob'a ulaşmamış kısmı ≥ `min_units` × 1013 B.
    fn ready(self, min_units: u32) -> bool {
        let remaining = self.file_bytes.saturating_sub(self.bob_file_rx);
        self.unsent_est() <= 0 && remaining >= u64::from(min_units) * FILE_UNIT
    }
}

fn header_line(cfg: &Cfg, clock: &Clock) -> String {
    let spurts: Vec<String> = cfg
        .ptt_at
        .iter()
        .map(|d| d.as_millis().to_string())
        .collect();
    format!(
        "{{\"k\":\"hdr\",\"schema\":{},\"role\":{},\"scenario\":{},\"profile\":{},\"rep\":{},\
         \"t\":{:.3},\"pid\":{},\"alice_addr\":{},\"bob_addr\":{},\"params\":{{\"file_bytes\":{},\
         \"file_at_ms\":{},\"warmup_texts\":{},\"warmup_every_ms\":{},\"texts\":{},\"text_at_ms\":{},\
         \"text_every_ms\":{},\"text_bytes\":{},\"sigs\":{},\"sig_at_ms\":{},\"sig_every_ms\":{},\
         \"sig_bytes\":{},\"ptt_at_ms\":[{}],\"ptt_bundles\":{},\"ptt_bundle_ms\":{},\"ptt_bytes\":{},\
         \"stall_at_ms\":{},\"stall_ms\":{},\"karsi\":{},\"karsi_texts\":{},\"karsi_every_ms\":{},\
         \"karsi_bundles\":{},\"karsi_min_unacked\":{},\"karsi_text_bytes\":{},\"window_s\":{},\
         \"connect_timeout_s\":{},\"probe_ms\":{},\"live_diag\":{},\"receipt_grace_s\":{}}},\
         \"quinn\":{}}}",
        js(SCHEMA),
        js(if cfg.role == Role::Alice { ALICE } else { BOB }),
        js(cfg.scenario.name()),
        js(&cfg.profile),
        cfg.rep,
        clock.ms(Instant::now()),
        std::process::id(),
        js(&cfg.alice_addr.to_string()),
        js(&cfg.bob_addr.to_string()),
        if cfg.scenario.has_file() {
            cfg.file_bytes
        } else {
            0
        },
        cfg.file_at.as_millis(),
        cfg.warmup_texts,
        cfg.warmup_every.as_millis(),
        cfg.texts,
        cfg.text_at.as_millis(),
        cfg.text_every.as_millis(),
        cfg.text_bytes,
        cfg.sigs,
        cfg.sig_at.as_millis(),
        cfg.sig_every.as_millis(),
        cfg.sig_bytes,
        spurts.join(","),
        cfg.ptt_bundles,
        cfg.ptt_bundle_ms,
        cfg.ptt_bytes,
        cfg.stall_at.as_millis(),
        cfg.stall.as_millis(),
        cfg.karsi,
        cfg.karsi_texts,
        cfg.karsi_every.as_millis(),
        cfg.karsi_bundles,
        cfg.karsi_min_unacked,
        cfg.karsi_text_bytes,
        cfg.window.as_secs(),
        cfg.connect_timeout.as_secs(),
        cfg.probe_every.as_millis(),
        cfg.live_diag,
        cfg.receipt_grace.as_secs(),
        quinn_json(cfg)
    )
}

/// Kol tanımı (başlık ve özet).
fn quinn_json(cfg: &Cfg) -> String {
    let arm = cfg.arm;
    format!(
        "{{\"arm\":{},\"cc\":{},\"iw_bytes\":{},\"iw_rule\":{},\"initial_mtu\":{},\"pmtud\":false,\
         \"gso\":false,\"initial_rtt_ms\":{QUINN_INITIAL_RTT_MS},\"idle_s\":{},\
         \"uni_streams\":{QUINN_UNI_STREAMS},\"streams\":\"mesaj-basina-uni\",\"prio\":{},\
         \"quinn\":{},\"quinn_proto\":{},\"hs\":\"netem-ici; t0 = baglanti sonrasi (pencere disi)\",\
         \"karsi_vekil\":\"udp_tx-lost-datagram*{Q_SHORT_HDR_EST} >= app_out; dosya-FB >= N*{FILE_UNIT}\"}}",
        js(arm.name()),
        js(arm.cc().label()),
        arm.iw_bytes(),
        js(arm.iw_rule()),
        crate::quic::INITIAL_MTU,
        crate::quic::IDLE_TIMEOUT.as_secs(),
        js(if cfg.prio {
            "ptt2>text1>file0"
        } else {
            "off"
        }),
        js(crate::quic::QUINN_VERSION),
        js(QUINN_PROTO_VERSION),
    )
}

fn stats_json(st: &ConnectionStats) -> String {
    format!(
        "\"cwnd\":{},\"rtt_ms\":{:.1},\"min_rtt_ms\":{:.1},\"cong\":{},\"lost_p\":{},\"lost_b\":{},\
         \"sent_p\":{},\"tx_b\":{},\"tx_d\":{},\"rx_b\":{},\"rx_d\":{}",
        st.path.cwnd,
        st.path.rtt.as_secs_f64() * 1e3,
        st.path.min_rtt.as_secs_f64() * 1e3,
        st.path.congestion_events,
        st.path.lost_packets,
        st.path.lost_bytes,
        st.path.sent_packets,
        st.udp_tx.bytes,
        st.udp_tx.datagrams,
        st.udp_rx.bytes,
        st.udp_rx.datagrams
    )
}

fn counts_json(counts: &BTreeMap<char, u32>) -> String {
    let fields: Vec<String> = counts
        .iter()
        .map(|(k, v)| format!("{}:{v}", js(&k.to_string())))
        .collect();
    format!("{{{}}}", fields.join(","))
}

/// quinn yapılandırması: mevcut bench tanımı (`quic.rs`) + kolun ilk penceresi. Süreç başına bir kez.
fn set_tuning(arm: QArm) {
    crate::rival::set(RivalTuning {
        delivery: Delivery::TransportAck,
        quinn_iw_bytes: arm.iw_override(),
        ..RivalTuning::default()
    });
}

/// Giriş: rol koşar, `<rol>.json` özetini yazar ve stdout'a basar.
///
/// # Errors
/// Geçersiz argüman, koşu dizini ya da rol hatası (özet yine yazılır).
pub async fn run(a: F0qArgs) -> Result<(), BenchError> {
    let cfg = Cfg::from_args(&a).map_err(|e| BenchError::Other(format!("f0-quinn: {e}")))?;
    std::env::set_current_dir(&cfg.dir)
        .map_err(|e| BenchError::Other(format!("f0-quinn: --dir {:?}: {e}", cfg.dir)))?;
    fs::create_dir_all(SYNC_DIR)?;
    set_tuning(cfg.arm);
    let role = if cfg.role == Role::Alice { ALICE } else { BOB };
    let (summary, ok) = match cfg.role {
        Role::Alice => run_alice(cfg).await,
        Role::Bob => run_bob(cfg).await,
    };
    if let Err(e) = fs::write(format!("{role}.json"), format!("{summary}\n")) {
        eprintln!("f0-quinn: {role}.json: {e}");
    }
    println!("{summary}");
    if ok {
        Ok(())
    } else {
        Err(BenchError::Other(format!(
            "f0-quinn {role}: hata (bkz. {role}.json)"
        )))
    }
}

// ── alice (istemci, gönderen) ────────────────────────────────────────────────────────────────

/// alice'in alım görevlerinden sürücüye olaylar (damga görevde, olay anında alınır).
#[derive(Debug)]
enum AEvent {
    Receipt { at: Instant, mid: u32 },
    FileReceipt { at: Instant },
    FileSent { at: Instant, bytes: u64 },
    FileAcked { at: Instant },
    FileFailed { at: Instant, why: String },
    Closed { at: Instant, why: String },
    Unexpected { at: Instant, kind: u8 },
}

fn push(tx: &mpsc::Sender<AEvent>, drops: &AtomicU64, ev: AEvent) {
    if tx.try_send(ev).is_err() {
        drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct FileLog {
    sent_at: Option<Instant>,
    stage_sent_at: Option<Instant>,
    delivered_at: Option<Instant>,
    acked_at: Option<Instant>,
    failed_at: Option<Instant>,
    failed_reason: Option<String>,
}

#[derive(Default)]
struct BobView {
    counts: BTreeMap<char, u32>,
    file: String,
    file_rx: u64,
}

struct Alice {
    cfg: Cfg,
    clock: Clock,
    out: SharedOut,
    rx: mpsc::Receiver<AEvent>,
    tx: mpsc::Sender<AEvent>,
    drops: Arc<AtomicU64>,
    endpoint: Option<Endpoint>,
    conn: Option<Connection>,
    t0: Option<Instant>,
    connect_at: Option<Instant>,
    connected_at: Option<Instant>,
    plan: Vec<Action>,
    next: usize,
    sent: BTreeMap<char, u32>,
    msg_items: BTreeMap<u32, (char, u32)>,
    receipts: BTreeSet<u32>,
    next_mid: u32,
    file: FileLog,
    karsi_at: Option<Instant>,
    bob: BobView,
    bob_complete_at: Option<Instant>,
    next_probe: Instant,
    probes: u64,
    stats0: Option<ConnectionStats>,
    last_stats: Option<ConnectionStats>,
    app_out: u64,
}

impl Alice {
    fn conn(&self) -> Result<Connection, String> {
        self.conn
            .clone()
            .ok_or_else(|| "alice bağlantısı yok".to_owned())
    }

    fn t0(&self) -> Instant {
        self.t0.unwrap_or(self.clock.inst0)
    }

    fn line(&self, s: &str) {
        emit(&self.out, s);
    }

    fn bump(&mut self, class: char) {
        *self.sent.entry(class).or_insert(0) += 1;
    }

    fn send_line(&self, a: Action, t_call: Instant, extra: &str) {
        let (class, idx, spurt) = match a.act {
            Act::Text { class, idx } => (class.to_string(), idx, None),
            Act::Sig { idx } => ("S".to_owned(), idx, None),
            Act::PttOpen { spurt } => ("O".to_owned(), 0, Some(spurt)),
            Act::PttPush { spurt, idx } => ("P".to_owned(), idx, Some(spurt)),
            Act::PttClose { spurt } => ("C".to_owned(), 0, Some(spurt)),
            Act::File => ("F".to_owned(), 0, None),
        };
        let t_ret = Instant::now();
        let sp = spurt.map_or_else(|| "null".to_owned(), |s| s.to_string());
        self.line(&format!(
            "{{\"k\":\"send\",\"c\":{},\"i\":{idx},\"sp\":{sp},\"due\":{:.3},\"t\":{:.3},\
             \"tr\":{:.3},\"try\":0{extra}}}",
            js(&class),
            self.clock.ms(self.t0() + a.due),
            self.clock.ms(t_call),
            self.clock.ms(t_ret),
        ));
    }

    /// Tek öğe: yeni uni-stream, gövde, FIN.
    async fn send_msg(&mut self, bytes: &[u8], prio: i32) -> Result<(), String> {
        let conn = self.conn()?;
        let mut s = conn
            .open_uni()
            .await
            .map_err(|e| format!("open_uni: {e}"))?;
        if self.cfg.prio {
            s.set_priority(prio)
                .map_err(|e| format!("set_priority: {e}"))?;
        }
        s.write_all(bytes)
            .await
            .map_err(|e| format!("write: {e}"))?;
        s.finish().map_err(|e| format!("finish: {e}"))?;
        self.app_out = self
            .app_out
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        Ok(())
    }

    async fn execute(&mut self, a: Action) -> Result<(), String> {
        let t_call = Instant::now();
        let prio = a.act.priority();
        match a.act {
            Act::Text { class, idx } => {
                let len = if class == 'K' {
                    self.cfg.karsi_text_bytes
                } else {
                    self.cfg.text_bytes
                };
                let text = text_body(class, idx, len);
                let mid = self.next_mid;
                self.next_mid = self.next_mid.wrapping_add(1);
                let mut body = Vec::with_capacity(4 + text.len());
                body.extend_from_slice(&mid.to_be_bytes());
                body.extend_from_slice(text.as_bytes());
                self.send_msg(&frame(K_TEXT, &body), prio)
                    .await
                    .map_err(|e| format!("text {class}{idx}: {e}"))?;
                self.msg_items.insert(mid, (class, idx));
                self.bump(class);
                self.send_line(a, t_call, &format!(",\"mid\":{mid}"));
            }
            Act::Sig { idx } => {
                let body = sig_body(idx, self.cfg.sig_bytes);
                self.send_msg(&frame(K_SIG, &body), prio)
                    .await
                    .map_err(|e| format!("sig {idx}: {e}"))?;
                self.bump('S');
                self.send_line(a, t_call, "");
            }
            Act::PttOpen { spurt } => {
                self.send_msg(&frame(K_OPEN, &spurt_u16(spurt)), prio)
                    .await
                    .map_err(|e| format!("ptt_open {spurt}: {e}"))?;
                self.bump('O');
                self.send_line(a, t_call, "");
            }
            Act::PttPush { spurt, idx } => {
                let body = bundle_body(spurt, idx, self.cfg.ptt_bytes);
                self.send_msg(&frame(K_PUSH, &body), prio)
                    .await
                    .map_err(|e| format!("ptt_push {spurt}/{idx}: {e}"))?;
                self.bump('P');
                self.send_line(a, t_call, "");
            }
            Act::PttClose { spurt } => {
                self.send_msg(&frame(K_CLOSE, &spurt_u16(spurt)), prio)
                    .await
                    .map_err(|e| format!("ptt_close {spurt}: {e}"))?;
                self.bump('C');
                self.send_line(a, t_call, "");
            }
            Act::File => {
                self.start_file(prio)?;
                self.file.sent_at = Some(t_call);
                self.bump('F');
                let extra = format!(",\"bytes\":{}", self.cfg.file_bytes);
                self.send_line(a, t_call, &extra);
            }
        }
        Ok(())
    }

    /// Dosya görevi: tek uni-stream; quinn'e verilince `FileSent`, eşçe tamamı ACK'lenince
    /// `FileAcked` (bilgi; tamamlanma ölçütü bob'un alındısıdır).
    fn start_file(&mut self, prio: i32) -> Result<(), String> {
        let conn = self.conn()?;
        let size = self.cfg.file_bytes;
        let mut bytes = Vec::with_capacity(1 + 8 + size);
        bytes.push(K_FILE);
        bytes.extend_from_slice(&u64::try_from(size).unwrap_or(u64::MAX).to_be_bytes());
        bytes.extend((0..size).map(file_byte));
        let set_prio = self.cfg.prio;
        let tx = self.tx.clone();
        let drops = Arc::clone(&self.drops);
        tokio::spawn(async move {
            let res: Result<(), String> = async {
                let mut s = conn
                    .open_uni()
                    .await
                    .map_err(|e| format!("open_uni: {e}"))?;
                if set_prio {
                    s.set_priority(prio)
                        .map_err(|e| format!("set_priority: {e}"))?;
                }
                s.write_all(&bytes)
                    .await
                    .map_err(|e| format!("write: {e}"))?;
                s.finish().map_err(|e| format!("finish: {e}"))?;
                push(
                    &tx,
                    &drops,
                    AEvent::FileSent {
                        at: Instant::now(),
                        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    },
                );
                s.stopped().await.map_err(|e| format!("stopped: {e}"))?;
                push(&tx, &drops, AEvent::FileAcked { at: Instant::now() });
                Ok(())
            }
            .await;
            if let Err(why) = res {
                push(
                    &tx,
                    &drops,
                    AEvent::FileFailed {
                        at: Instant::now(),
                        why,
                    },
                );
            }
        });
        Ok(())
    }

    fn on_event(&mut self, ev: AEvent) {
        match ev {
            AEvent::Receipt { at, mid } => {
                let first = self.receipts.insert(mid);
                let (class, idx) = self.msg_items.get(&mid).copied().unwrap_or(('?', 0));
                self.line(&format!(
                    "{{\"k\":\"rcpt\",\"t\":{:.3},\"mid\":{mid},\"c\":{},\"i\":{idx},\
                     \"first\":{first}}}",
                    self.clock.ms(at),
                    js(&class.to_string())
                ));
            }
            AEvent::FileReceipt { at } => {
                self.file.delivered_at.get_or_insert(at);
                self.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"file_delivered\"}}",
                    self.clock.ms(at)
                ));
            }
            AEvent::FileSent { at, bytes } => {
                self.file.stage_sent_at.get_or_insert(at);
                self.app_out = self.app_out.saturating_add(bytes);
                self.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"file_sent\"}}",
                    self.clock.ms(at)
                ));
            }
            AEvent::FileAcked { at } => {
                self.file.acked_at.get_or_insert(at);
                self.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"file_acked\"}}",
                    self.clock.ms(at)
                ));
            }
            AEvent::FileFailed { at, why } => {
                if self.file.failed_at.is_none() {
                    self.file.failed_at = Some(at);
                    self.file.failed_reason = Some(format!("Outgoing({why})"));
                }
                self.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"file_failed\",\"file\":\"Outgoing\",\
                     \"why\":{}}}",
                    self.clock.ms(at),
                    js(&why)
                ));
            }
            AEvent::Closed { at, why } => {
                self.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"closed\",\"peer\":\"bob\",\"why\":{}}}",
                    self.clock.ms(at),
                    js(&why)
                ));
            }
            AEvent::Unexpected { at, kind } => {
                self.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"unexpected\",\"what\":{kind}}}",
                    self.clock.ms(at)
                ));
            }
        }
    }

    fn pump_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            self.on_event(ev);
        }
    }

    fn probe(&self, st: &ConnectionStats) -> KarsiProbe {
        let base = self.stats0.unwrap_or_default();
        KarsiProbe {
            app_out: self.app_out,
            tx_bytes: st.udp_tx.bytes.saturating_sub(base.udp_tx.bytes),
            lost_bytes: st.path.lost_bytes.saturating_sub(base.path.lost_bytes),
            tx_datagrams: st.udp_tx.datagrams.saturating_sub(base.udp_tx.datagrams),
            file_bytes: if self.cfg.scenario.has_file() {
                u64::try_from(self.cfg.file_bytes).unwrap_or(u64::MAX)
            } else {
                0
            },
            bob_file_rx: self.bob.file_rx,
        }
    }

    /// `qs` satırı (quinn bağlantı sayaçları; salt gözlem) + bob durumu.
    fn sample(&mut self) {
        self.read_bob_state();
        let Some(conn) = self.conn.as_ref() else {
            return;
        };
        let st = conn.stats();
        let at = Instant::now();
        let p = self.probe(&st);
        self.line(&format!(
            "{{\"k\":\"qs\",\"t\":{:.3},{},\"app_out\":{},\"unsent_est\":{},\"bob_file_rx\":{}}}",
            self.clock.ms(at),
            stats_json(&st),
            self.app_out,
            p.unsent_est(),
            self.bob.file_rx
        ));
        self.last_stats = Some(st);
        self.probes += 1;
    }

    fn read_bob_state(&mut self) {
        let Some(body) = read_marker(BOB_STATE) else {
            return;
        };
        let mut view = BobView {
            file: "none".to_owned(),
            ..BobView::default()
        };
        for line in body.lines() {
            let mut parts = line.split_whitespace();
            let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
                continue;
            };
            match key {
                "F" => value.clone_into(&mut view.file),
                "FB" => view.file_rx = value.parse().unwrap_or(0),
                _ => {
                    if let (Some(c), Ok(n)) = (key.chars().next(), value.parse::<u32>()) {
                        view.counts.insert(c, n);
                    }
                }
            }
        }
        self.bob = view;
    }

    fn bob_has_everything(&self) -> bool {
        let items = self
            .sent
            .iter()
            .filter(|(c, _)| **c != 'F')
            .all(|(c, n)| self.bob.counts.get(c).copied().unwrap_or(0) >= *n);
        let file = !self.cfg.scenario.has_file()
            || self.bob.file == "received"
            || self.bob.file == "failed";
        items && file
    }

    /// Karşı örnek: f0_flows şartları aynen, motor sondası yerine quinn vekili (modül belgesi).
    fn maybe_karsi(&mut self) {
        if !self.cfg.karsi
            || !self.cfg.scenario.has_file()
            || self.karsi_at.is_some()
            || self.file.stage_sent_at.is_none()
            || self.bob.file == "received"
            || self.bob.file == "failed"
        {
            return;
        }
        let now = Instant::now();
        if now < self.t0() + self.cfg.file_at + Duration::from_secs(1) {
            return;
        }
        let Some(st) = self.last_stats else {
            return;
        };
        let p = self.probe(&st);
        if !p.ready(self.cfg.karsi_min_unacked) {
            return;
        }
        self.karsi_at = Some(now);
        let base = now.saturating_duration_since(self.t0());
        let added = karsi_actions(&self.cfg, base);
        let extra = format!(
            ",\"session\":{{\"st\":\"quinn\",\"unsent_est\":{},\"bob_file_rx\":{},{}}},\
             \"added\":{}",
            p.unsent_est(),
            p.bob_file_rx,
            stats_json(&st),
            added.len()
        );
        self.line(&mark_line(&self.clock, now, "karsi_trigger", &extra));
        self.plan.extend(added);
        self.plan[self.next..].sort_by_key(|a| a.due);
    }

    fn complete(&mut self) -> bool {
        if self.next < self.plan.len() || !self.bob_has_everything() {
            return false;
        }
        let now = Instant::now();
        let since = *self.bob_complete_at.get_or_insert(now);
        self.receipts.len() >= self.msg_items.len() || now >= since + self.cfg.receipt_grace
    }

    fn plan_end(&self) -> Instant {
        let last = self.plan.last().map_or(Duration::ZERO, |a| a.due);
        self.t0() + last
    }

    async fn connect(&mut self) -> Result<(), String> {
        wait_marker(MARK_BOB_UP, self.cfg.connect_timeout)
            .await
            .ok_or_else(|| {
                format!(
                    "{MARK_BOB_UP} {:?} içinde gelmedi",
                    self.cfg.connect_timeout
                )
            })?;
        let (mut endpoint, _gov) =
            crate::quic::build_endpoint(self.cfg.alice_addr, None, BudgetMode::Unlimited)
                .map_err(|e| format!("alice endpoint: {e}"))?;
        endpoint.set_default_client_config(
            crate::quic::client_config(self.cfg.arm.cc())
                .map_err(|e| format!("alice client_config: {e}"))?,
        );
        let connect_at = Instant::now();
        self.connect_at = Some(connect_at);
        self.line(&mark_line(&self.clock, connect_at, "connect_requested", ""));
        let connecting = endpoint
            .connect(self.cfg.bob_addr, "localhost")
            .map_err(|e| format!("alice connect: {e}"))?;
        self.endpoint = Some(endpoint);
        let conn = match timeout(self.cfg.connect_timeout, connecting).await {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(format!("quinn el sıkışması: {e}")),
            Err(_) => {
                return Err(format!(
                    "connect {:?} içinde tamamlanmadı",
                    self.cfg.connect_timeout
                ))
            }
        };
        let at = Instant::now();
        self.connected_at = Some(at);
        self.line(&format!(
            "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"connected\",\"peer\":\"bob\"}}",
            self.clock.ms(at)
        ));
        tokio::spawn(alice_accept(
            conn.clone(),
            self.tx.clone(),
            Arc::clone(&self.drops),
        ));
        self.conn = Some(conn);
        Ok(())
    }

    /// Bağlantı + plan + tamamlanma. `Ok(true)` = tamamlandı, `Ok(false)` = pencere doldu.
    async fn execute_run(&mut self) -> Result<bool, String> {
        self.connect().await?;
        let t0 = Instant::now();
        self.t0 = Some(t0);
        write_marker(MARK_T0, &self.clock.unix_ns(t0).to_string())?;
        let st0 = self.conn()?.stats();
        self.stats0 = Some(st0);
        let setup_ms = self
            .connect_at
            .map_or(0.0, |c| t0.saturating_duration_since(c).as_secs_f64() * 1e3);
        let extra = format!(
            ",\"setup_ms\":{setup_ms:.3},\"hs\":{{{}}}",
            stats_json(&st0)
        );
        self.line(&mark_line(&self.clock, t0, "t0", &extra));
        self.next_probe = t0;
        let mut flush_at = t0;
        loop {
            let now = Instant::now();
            while self.next < self.plan.len() && self.t0() + self.plan[self.next].due <= now {
                let a = self.plan[self.next];
                self.execute(a).await?;
                self.next += 1;
            }
            self.pump_events();
            if Instant::now() >= self.next_probe {
                self.sample();
                self.next_probe = Instant::now() + self.cfg.probe_every;
                self.maybe_karsi();
            }
            if Instant::now() >= flush_at {
                flush(&self.out);
                flush_at = Instant::now() + Duration::from_secs(1);
            }
            if self.complete() {
                return Ok(true);
            }
            if self.next >= self.plan.len() && Instant::now() >= self.plan_end() + self.cfg.window {
                return Ok(false);
            }
            let wait = match self.plan.get(self.next) {
                Some(a) => (self.t0() + a.due)
                    .saturating_duration_since(Instant::now())
                    .min(PUMP_STEP),
                None => PUMP_STEP,
            };
            let got = tokio::select! {
                ev = self.rx.recv() => ev,
                () = sleep(wait) => None,
            };
            if let Some(ev) = got {
                self.on_event(ev);
            }
        }
    }

    async fn finish(&mut self, result: Result<bool, String>) -> (String, bool) {
        let ended_at = Instant::now();
        self.pump_events();
        self.sample();
        let _ = write_marker(MARK_ALICE_DONE, "done");
        let deadline = Instant::now() + BOB_EXIT_WAIT;
        let mut bob_exit = false;
        while Instant::now() < deadline {
            if read_marker(MARK_BOB_EXIT).is_some() {
                bob_exit = true;
                break;
            }
            let got = tokio::select! {
                ev = self.rx.recv() => ev,
                () = sleep(MARK_POLL) => None,
            };
            if let Some(ev) = got {
                self.on_event(ev);
            }
        }
        self.pump_events();
        self.read_bob_state();
        let (outcome, error, ok) = match &result {
            Ok(true) => ("complete", "null".to_owned(), true),
            Ok(false) => ("incomplete", "null".to_owned(), true),
            Err(e) => ("error", js(e), false),
        };
        let link = self.conn.as_ref().map_or_else(
            || "null".to_owned(),
            |c| format!("{{{}}}", stats_json(&c.stats())),
        );
        if let Some(conn) = self.conn.take() {
            conn.close(0u32.into(), b"done");
        }
        if let Some(ep) = self.endpoint.take() {
            let _ = timeout(CLOSE_WAIT, ep.wait_idle()).await;
        }
        let setup_ms = match (self.connect_at, self.connected_at) {
            (Some(c), Some(d)) => {
                format!("{:.3}", d.saturating_duration_since(c).as_secs_f64() * 1e3)
            }
            _ => "null".to_owned(),
        };
        let (lines, errors) = out_counts(&self.out);
        let summary = format!(
            "{{\"schema\":{},\"role\":\"alice\",\"scenario\":{},\"profile\":{},\"rep\":{},\
             \"result\":{},\"error\":{error},\"t0\":{},\"connected\":{},\"ended\":{:.3},\
             \"plan_actions\":{},\"plan_done\":{},\"retries\":0,\"sent\":{},\"receipts\":{},\
             \"texts_sent\":{},\"bob_counts\":{},\"bob_file\":{},\"file\":{{\"sent\":{},\
             \"stage_sent\":{},\"delivered\":{},\"acked\":{},\"failed\":{},\"why\":{}}},\"karsi\":{},\
             \"probes\":{},\"probe_errors\":0,\"listener_drops\":{},\"link\":{link},\
             \"setup_ms\":{setup_ms},\"bob_exit\":{bob_exit},\"shutdown_error\":null,\
             \"jsonl_lines\":{lines},\"jsonl_errors\":{errors},\"quinn\":{},\"notes\":[]}}",
            js(SCHEMA),
            js(self.cfg.scenario.name()),
            js(&self.cfg.profile),
            self.cfg.rep,
            js(outcome),
            opt_ms(&self.clock, self.t0),
            opt_ms(&self.clock, self.connected_at),
            self.clock.ms(ended_at),
            self.plan.len(),
            self.next,
            counts_json(&self.sent),
            self.receipts.len(),
            self.msg_items.len(),
            counts_json(&self.bob.counts),
            js(&self.bob.file),
            opt_ms(&self.clock, self.file.sent_at),
            opt_ms(&self.clock, self.file.stage_sent_at),
            opt_ms(&self.clock, self.file.delivered_at),
            opt_ms(&self.clock, self.file.acked_at),
            opt_ms(&self.clock, self.file.failed_at),
            self.file
                .failed_reason
                .as_deref()
                .map_or_else(|| "null".to_owned(), js),
            opt_ms(&self.clock, self.karsi_at),
            self.probes,
            self.drops.load(Ordering::Relaxed),
            quinn_json(&self.cfg),
        );
        flush(&self.out);
        (summary, ok)
    }
}

/// alice'e gelen stream'ler: TEXT alındıları (`'R' ‖ mid`) ve dosya alındısı (`'D'`).
async fn alice_accept(conn: Connection, tx: mpsc::Sender<AEvent>, drops: Arc<AtomicU64>) {
    loop {
        match conn.accept_uni().await {
            Ok(recv) => {
                tokio::spawn(alice_read(recv, tx.clone(), Arc::clone(&drops)));
            }
            Err(e) => {
                push(
                    &tx,
                    &drops,
                    AEvent::Closed {
                        at: Instant::now(),
                        why: e.to_string(),
                    },
                );
                return;
            }
        }
    }
}

async fn alice_read(mut recv: RecvStream, tx: mpsc::Sender<AEvent>, drops: Arc<AtomicU64>) {
    let Ok(buf) = recv.read_to_end(MAX_MSG).await else {
        return;
    };
    let at = Instant::now();
    let ev = match (buf.first().copied(), buf.get(1..)) {
        (Some(K_RCPT), Some(rest)) if rest.len() == 4 => {
            let mut b = [0u8; 4];
            b.copy_from_slice(rest);
            AEvent::Receipt {
                at,
                mid: u32::from_be_bytes(b),
            }
        }
        (Some(K_FILE_RCPT), _) => AEvent::FileReceipt { at },
        (kind, _) => AEvent::Unexpected {
            at,
            kind: kind.unwrap_or(0),
        },
    };
    push(&tx, &drops, ev);
}

async fn run_alice(cfg: Cfg) -> (String, bool) {
    let clock = Clock::new();
    let out = match Out::create("alice.jsonl") {
        Ok(out) => Arc::new(Mutex::new(out)),
        Err(e) => {
            return (
                format!(
                    "{{\"role\":\"alice\",\"result\":\"error\",\"error\":{}}}",
                    js(&e)
                ),
                false,
            )
        }
    };
    emit(&out, &header_line(&cfg, &clock));
    let (tx, rx) = mpsc::channel(EVENT_QUEUE_CAP);
    let plan = build_plan(&cfg);
    let mut alice = Alice {
        cfg,
        clock,
        out,
        rx,
        tx,
        drops: Arc::new(AtomicU64::new(0)),
        endpoint: None,
        conn: None,
        t0: None,
        connect_at: None,
        connected_at: None,
        plan,
        next: 0,
        sent: BTreeMap::new(),
        msg_items: BTreeMap::new(),
        receipts: BTreeSet::new(),
        next_mid: 1,
        file: FileLog::default(),
        karsi_at: None,
        bob: BobView {
            file: "none".to_owned(),
            ..BobView::default()
        },
        bob_complete_at: None,
        next_probe: Instant::now(),
        probes: 0,
        stats0: None,
        last_stats: None,
        app_out: 0,
    };
    alice.line(&mark_line(&alice.clock, Instant::now(), "node_up", ""));
    let result = alice.execute_run().await;
    alice.finish(result).await
}

// ── bob (sunucu, alıcı) ──────────────────────────────────────────────────────────────────────

/// bob'un alım görevleri arasında paylaşılan durum (kilit `await` boyunca tutulmaz).
struct BobState {
    seen: BTreeSet<(char, u32, u32)>,
    counts: BTreeMap<char, u32>,
    duplicates: u64,
    unparsed: u64,
    file_status: &'static str,
    file_size: u64,
    file_match: Option<bool>,
    file_at: Option<Instant>,
    file_rx: u64,
    dirty: bool,
}

impl BobState {
    fn count(&mut self, class: char, spurt: u32, idx: u32) -> bool {
        if self.seen.insert((class, spurt, idx)) {
            *self.counts.entry(class).or_insert(0) += 1;
            self.dirty = true;
            true
        } else {
            self.duplicates += 1;
            false
        }
    }

    /// `sync/bob.state`: f0_flows biçimi (`<sınıf> <adet>`, `F <durum>`) + `FB <dosya baytı>`.
    fn state_text(&self) -> String {
        let mut s = String::new();
        for (k, v) in &self.counts {
            let _ = writeln!(s, "{k} {v}");
        }
        let _ = writeln!(s, "F {}", self.file_status);
        let _ = writeln!(s, "FB {}", self.file_rx);
        s
    }
}

type SharedBob = Arc<Mutex<BobState>>;

/// bob alım görevlerinin ortak bağlamı.
#[derive(Clone)]
struct BobCtx {
    clock: Clock,
    out: SharedOut,
    st: SharedBob,
}

impl BobCtx {
    fn with<R>(&self, f: impl FnOnce(&mut BobState) -> R) -> Option<R> {
        self.st.lock().ok().map(|mut g| f(&mut g))
    }
}

async fn bob_accept(endpoint: Endpoint, ctx: BobCtx, slot: Arc<Mutex<Option<Connection>>>) {
    while let Some(incoming) = endpoint.accept().await {
        let Ok(conn) = incoming.await else {
            continue;
        };
        let at = Instant::now();
        emit(
            &ctx.out,
            &format!(
                "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"connected\",\"peer\":\"alice\"}}",
                ctx.clock.ms(at)
            ),
        );
        if let Ok(mut s) = slot.lock() {
            *s = Some(conn.clone());
        }
        tokio::spawn(bob_conn(conn, ctx.clone()));
    }
}

async fn bob_conn(conn: Connection, ctx: BobCtx) {
    loop {
        match conn.accept_uni().await {
            Ok(recv) => {
                tokio::spawn(bob_read(recv, conn.clone(), ctx.clone()));
            }
            Err(e) => {
                emit(
                    &ctx.out,
                    &format!(
                        "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"closed\",\"peer\":\"alice\",\"why\":{}}}",
                        ctx.clock.ms(Instant::now()),
                        js(&e.to_string())
                    ),
                );
                return;
            }
        }
    }
}

/// Alındı stream'i (bob → alice). Hata yalnız kayda geçer (alıcı tarafı ölçümü etkilemez).
async fn bob_reply(conn: &Connection, bytes: &[u8]) {
    if let Ok(mut s) = conn.open_uni().await {
        if s.write_all(bytes).await.is_ok() {
            let _ = s.finish();
        }
    }
}

async fn bob_read(mut recv: RecvStream, conn: Connection, ctx: BobCtx) {
    let mut kind = [0u8; 1];
    if recv.read_exact(&mut kind).await.is_err() {
        return;
    }
    if kind[0] == K_FILE {
        bob_read_file(recv, &conn, &ctx).await;
        return;
    }
    let Ok(body) = recv.read_to_end(MAX_MSG).await else {
        ctx.with(|s| s.unparsed += 1);
        return;
    };
    let at = Instant::now();
    let t = ctx.clock.ms(at);
    let line = match kind[0] {
        K_TEXT => {
            let parsed = body.get(0..4).and_then(|m| {
                let text = std::str::from_utf8(body.get(4..)?).ok()?;
                let (class, idx) = parse_text(text)?;
                Some((u32::from_be_bytes(m.try_into().ok()?), class, idx, text.len()))
            });
            match parsed {
                Some((mid, class, idx, len)) => {
                    let first = ctx.with(|s| s.count(class, 0, idx)).unwrap_or(false);
                    let l = format!(
                        "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":{},\"i\":{idx},\"mid\":{mid},\
                         \"len\":{len},\"first\":{first}}}",
                        js(&class.to_string())
                    );
                    emit(&ctx.out, &l);
                    let mut r = vec![K_RCPT];
                    r.extend_from_slice(&mid.to_be_bytes());
                    bob_reply(&conn, &r).await;
                    return;
                }
                None => None,
            }
        }
        K_SIG => parse_sig(&body).map(|idx| {
            let first = ctx.with(|s| s.count('S', 0, idx)).unwrap_or(false);
            format!(
                "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"S\",\"i\":{idx},\"len\":{},\"first\":{first}}}",
                body.len()
            )
        }),
        K_OPEN | K_CLOSE => body.get(0..2).map(|b| {
            let spurt = u32::from(u16::from_be_bytes([b[0], b[1]]));
            let class = if kind[0] == K_OPEN { 'O' } else { 'C' };
            let first = ctx.with(|s| s.count(class, spurt, 0)).unwrap_or(false);
            format!(
                "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"{class}\",\"stream\":{spurt},\"first\":{first}}}"
            )
        }),
        K_PUSH => parse_bundle(&body).map(|(spurt, idx)| {
            let first = ctx.with(|s| s.count('P', spurt, idx)).unwrap_or(false);
            format!(
                "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"P\",\"sp\":{spurt},\"i\":{idx},\
                 \"stream\":{spurt},\"first\":{first}}}"
            )
        }),
        _ => None,
    };
    match line {
        Some(l) => emit(&ctx.out, &l),
        None => {
            ctx.with(|s| s.unparsed += 1);
        }
    }
}

/// Dosya stream'i: boy (bounds-before-alloc), sıralı parçalar desenle doğrulanır (bellekte tutulmaz),
/// FIN'de `rx F` + `'D'` alındısı. Stream/bağlantı hatası = alıcıda dosya hatası.
async fn bob_read_file(mut recv: RecvStream, conn: &Connection, ctx: &BobCtx) {
    let mut sz = [0u8; 8];
    let fail = |why: String| {
        let at = Instant::now();
        ctx.with(|s| {
            if s.file_at.is_none() {
                s.file_at = Some(at);
                s.file_status = "failed";
                s.dirty = true;
            }
        });
        emit(
            &ctx.out,
            &format!(
                "{{\"k\":\"ev\",\"t\":{:.3},\"e\":\"file_failed\",\"file\":\"Incoming\",\"why\":{}}}",
                ctx.clock.ms(at),
                js(&why)
            ),
        );
    };
    if let Err(e) = recv.read_exact(&mut sz).await {
        fail(format!("boy: {e}"));
        return;
    }
    let size = u64::from_be_bytes(sz);
    if size > u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX) {
        fail(format!("boy {size} > {MAX_FILE_BYTES}"));
        return;
    }
    let mut got: u64 = 0;
    let mut matched = true;
    loop {
        match recv.read_chunk(READ_CHUNK, true).await {
            Ok(Some(chunk)) => {
                for (i, b) in chunk.bytes.iter().enumerate() {
                    let off = usize::try_from(got).unwrap_or(usize::MAX).saturating_add(i);
                    if *b != file_byte(off) {
                        matched = false;
                    }
                }
                got = got.saturating_add(u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX));
                ctx.with(|s| {
                    s.file_rx = got;
                    s.dirty = true;
                });
            }
            Ok(None) => break,
            Err(e) => {
                fail(format!("okuma: {e}"));
                return;
            }
        }
    }
    let at = Instant::now();
    let ok = matched && got == size;
    ctx.with(|s| {
        if s.file_at.is_none() {
            s.file_at = Some(at);
            s.file_status = "received";
            s.file_size = got;
            s.file_match = Some(ok);
            s.dirty = true;
        }
    });
    emit(
        &ctx.out,
        &format!(
            "{{\"k\":\"rx\",\"t\":{:.3},\"c\":\"F\",\"size\":{got},\"match\":{ok}}}",
            ctx.clock.ms(at)
        ),
    );
    bob_reply(conn, &[K_FILE_RCPT]).await;
}

/// bob'un sert son tarihi (f0_flows `hard_deadline` aynen).
fn bob_deadline(cfg: &Cfg, started: Instant) -> Instant {
    let mut plan_len = cfg
        .text_at
        .saturating_add(cfg.text_every.saturating_mul(cfg.texts))
        .max(cfg.sig_at + cfg.sig_every.saturating_mul(cfg.sigs));
    for s in &cfg.ptt_at {
        plan_len = plan_len.max(*s + spurt_len(cfg, cfg.ptt_bundles));
    }
    started + cfg.connect_timeout.saturating_mul(2) + plan_len + cfg.window + cfg.stall + BOB_SLACK
}

async fn run_bob(cfg: Cfg) -> (String, bool) {
    let clock = Clock::new();
    let out = match Out::create("bob.jsonl") {
        Ok(out) => Arc::new(Mutex::new(out)),
        Err(e) => {
            return (
                format!(
                    "{{\"role\":\"bob\",\"result\":\"error\",\"error\":{}}}",
                    js(&e)
                ),
                false,
            )
        }
    };
    emit(&out, &header_line(&cfg, &clock));
    let st: SharedBob = Arc::new(Mutex::new(BobState {
        seen: BTreeSet::new(),
        counts: BTreeMap::new(),
        duplicates: 0,
        unparsed: 0,
        file_status: "none",
        file_size: 0,
        file_match: None,
        file_at: None,
        file_rx: 0,
        dirty: true,
    }));
    let ctx = BobCtx {
        clock,
        out: Arc::clone(&out),
        st: Arc::clone(&st),
    };
    let slot: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
    let mut t0_ns: Option<u64> = None;
    let mut probes: u64 = 0;
    let result: Result<bool, String> = async {
        let sc = crate::quic::server_config(cfg.arm.cc())
            .map_err(|e| format!("bob server_config: {e}"))?;
        let (endpoint, _gov) =
            crate::quic::build_endpoint(cfg.bob_addr, Some(sc), BudgetMode::Unlimited)
                .map_err(|e| format!("bob endpoint: {e}"))?;
        tokio::spawn(bob_accept(endpoint.clone(), ctx.clone(), Arc::clone(&slot)));
        write_marker(MARK_BOB_UP, "up")?;
        emit(&out, &mark_line(&clock, Instant::now(), "node_up", ""));
        let started = Instant::now();
        let deadline = bob_deadline(&cfg, started);
        let mut next_probe = started;
        let mut flush_at = started;
        let res = loop {
            if t0_ns.is_none() {
                if let Some(t0) = read_marker(MARK_T0).and_then(|b| b.trim().parse::<u64>().ok()) {
                    t0_ns = Some(t0);
                    emit(
                        &out,
                        &mark_line(
                            &clock,
                            Instant::now(),
                            "t0_seen",
                            &format!(",\"t0\":{}", ns_ms(t0)),
                        ),
                    );
                }
            }
            if Instant::now() >= next_probe {
                bob_sample(&ctx, &slot);
                probes += 1;
                next_probe = Instant::now() + cfg.probe_every;
            }
            if Instant::now() >= flush_at {
                flush(&out);
                flush_at = Instant::now() + Duration::from_secs(1);
            }
            if read_marker(MARK_ALICE_DONE).is_some() {
                break Ok(true);
            }
            if Instant::now() >= deadline {
                break Ok(false);
            }
            sleep(PUMP_STEP).await;
        };
        bob_sample(&ctx, &slot);
        let conn = slot.lock().ok().and_then(|mut s| s.take());
        if let Some(c) = conn {
            c.close(0u32.into(), b"done");
        }
        endpoint.close(0u32.into(), b"done");
        res
    }
    .await;
    let (outcome, error, ok) = match &result {
        Ok(true) => ("alice_done", "null".to_owned(), true),
        Ok(false) => ("deadline", "null".to_owned(), true),
        Err(e) => ("error", js(e), false),
    };
    let body = ctx.with(|s| {
        s.dirty = true;
        format!(
            "\"counts\":{},\"duplicates\":{},\"unparsed\":{},\"file\":{{\"status\":{},\
             \"size\":{},\"match\":{},\"at\":{},\"rx_bytes\":{}}}",
            counts_json(&s.counts),
            s.duplicates,
            s.unparsed,
            js(s.file_status),
            s.file_size,
            s.file_match
                .map_or_else(|| "null".to_owned(), |m| m.to_string()),
            opt_ms(&clock, s.file_at),
            s.file_rx
        )
    });
    write_state(&ctx);
    let (lines, errors) = out_counts(&out);
    let summary = format!(
        "{{\"schema\":{},\"role\":\"bob\",\"scenario\":{},\"profile\":{},\"rep\":{},\
         \"result\":{},\"error\":{error},\"t0\":{},{},\"stall\":{{\"start\":null,\"end\":null}},\
         \"probes\":{probes},\"probe_errors\":0,\"listener_drops\":0,\"link\":null,\
         \"shutdown_error\":null,\"jsonl_lines\":{lines},\"jsonl_errors\":{errors},\"quinn\":{}}}",
        js(SCHEMA),
        js(cfg.scenario.name()),
        js(&cfg.profile),
        cfg.rep,
        js(outcome),
        t0_ns.map_or_else(|| "null".to_owned(), ns_ms),
        body.unwrap_or_else(|| "\"counts\":{}".to_owned()),
        quinn_json(&cfg),
    );
    flush(&out);
    let _ = write_marker(MARK_BOB_EXIT, "exit");
    (summary, ok)
}

fn write_state(ctx: &BobCtx) {
    let body = ctx.with(|s| {
        if s.dirty {
            s.dirty = false;
            Some(s.state_text())
        } else {
            None
        }
    });
    if let Some(Some(b)) = body {
        if write_marker(BOB_STATE, &b).is_err() {
            ctx.with(|s| s.dirty = true);
        }
    }
}

/// bob sondası: durum dosyası + `qs` satırı (bağlantı varsa).
fn bob_sample(ctx: &BobCtx, slot: &Arc<Mutex<Option<Connection>>>) {
    write_state(ctx);
    let conn = slot.lock().ok().and_then(|s| s.clone());
    if let Some(c) = conn {
        let st = c.stats();
        emit(
            &ctx.out,
            &format!(
                "{{\"k\":\"qs\",\"t\":{:.3},{}}}",
                ctx.clock.ms(Instant::now()),
                stats_json(&st)
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gösterim analoğu hücresinin argümanları (`scripts/yol3/cells/a/a_r12k_d150_l20_foto150k.cell`
    /// `F0A_ARGS`, yer tutucular doldurulmuş) + kol bayrağı.
    const G2_ARGS: &str = "--dir /tmp/f0q --alice-addr 10.10.0.1:20001 \
        --bob-addr 10.10.1.1:20001 --scenario dosyali --profile r12k_d150_l20 --rep 1 \
        --file-bytes 150480 --window-s 300 --warmup-texts 3 --warmup-every-ms 500 \
        --file-at-ms 5000 --texts 40 --text-at-ms 6000 --text-every-ms 2500 --text-bytes 64 \
        --sigs 10 --sig-at-ms 7000 --sig-every-ms 10000 --sig-bytes 120 --ptt-bundles 25 \
        --ptt-bundle-ms 200 --stall-at-ms 20000 --stall-ms 30000 --karsi on --karsi-texts 5 \
        --karsi-every-ms 500 --karsi-bundles 10 --karsi-min-unacked 3 --karsi-text-bytes 48 \
        --connect-timeout-s 30 --probe-ms 200 --live-diag on --receipt-grace-s 30 \
        --ptt-bytes 80 --ptt-at-ms 15000,60000";

    #[derive(clap::Parser)]
    struct T {
        #[command(flatten)]
        a: F0qArgs,
    }

    fn parse(extra: &str) -> Result<Cfg, String> {
        use clap::Parser;
        let mut argv = vec!["t".to_owned(), "--role".to_owned(), "alice".to_owned()];
        argv.extend(G2_ARGS.split_whitespace().map(str::to_owned));
        argv.extend(extra.split_whitespace().map(str::to_owned));
        let t = T::try_parse_from(argv).map_err(|e| e.to_string())?;
        Cfg::from_args(&t.a)
    }

    /// Hücrenin `F0A_ARGS` dizesi AYNEN kabul edilir; değerler f0_flows'taki gibi çözülür.
    #[test]
    fn g2_hucre_argumanlari_aynen() {
        let c = parse("--q-kol Q_bbr").expect("G2 argümanları");
        assert_eq!(c.scenario, Scenario::Dosyali);
        assert_eq!(c.file_bytes, 150_480);
        assert_eq!(c.ptt_bytes, 80);
        assert_eq!(
            c.ptt_at,
            vec![Duration::from_millis(15_000), Duration::from_millis(60_000)]
        );
        assert!(c.karsi && c.live_diag && !c.prio);
        assert_eq!(c.arm, QArm::Bbr);
    }

    /// Kol tablosu: CC ve ilk pencere; RFC 9002 formülü MSS 1452 → 14 520, MSS 1200 → 12 000.
    #[test]
    fn kol_tanimlari() {
        assert_eq!(rfc9002_iw(1452), 14_520);
        assert_eq!(rfc9002_iw(1200), QUINN_CUBIC_DEFAULT_IW);
        assert_eq!(rfc9002_iw(1500), 14_720);
        assert_eq!(crate::quic::INITIAL_MTU, 1452);
        assert_eq!((QArm::Bbr.cc(), QArm::Bbr.iw_bytes()), (Cc::Bbr, 240_000));
        assert_eq!(QArm::Bbr.iw_override(), None);
        assert_eq!(
            (QArm::BbrRfc.cc(), QArm::BbrRfc.iw_override()),
            (Cc::Bbr, Some(14_520))
        );
        assert_eq!(
            (QArm::Cubic.cc(), QArm::Cubic.iw_bytes()),
            (Cc::Cubic, 12_000)
        );
        assert_eq!(QArm::Cubic.iw_override(), None);
        let c = parse("--q-kol Q_bbr_rfc --q-oncelik on").expect("kol");
        assert_eq!(c.arm, QArm::BbrRfc);
        assert!(c.prio);
        assert!(parse("--q-kol Q_tcp").is_err(), "bilinmeyen kol reddedilir");
        assert!(parse("").is_err(), "--q-kol zorunlu");
    }

    /// Kama kollarına özgü bayraklar ve durma senaryoları reddedilir (sessiz kabul yok).
    #[test]
    fn red_edilenler() {
        assert!(parse("--q-kol Q_bbr --e1-arm K2").is_err());
        assert!(parse("--q-kol Q_bbr --fec-off on").is_err());
        let mut argv = vec!["t".to_owned(), "--role".to_owned(), "bob".to_owned()];
        argv.extend(
            G2_ARGS
                .replace("--scenario dosyali", "--scenario dosyali_dur")
                .split_whitespace()
                .map(str::to_owned),
        );
        argv.extend(["--q-kol".to_owned(), "Q_cubic".to_owned()]);
        use clap::Parser;
        let t = T::try_parse_from(argv).expect("clap");
        assert!(Cfg::from_args(&t.a).unwrap_err().contains("öykünülmez"));
    }

    /// Plan f0_flows ile aynı: 3 W + F + 40 T + 10 S + 2 × (O + 25 P + C); vadeler.
    #[test]
    fn plan_f0_flows_ile_ayni() {
        let c = parse("--q-kol Q_bbr").expect("cfg");
        let plan = build_plan(&c);
        assert_eq!(plan.len(), 3 + 1 + 40 + 10 + 2 * 27);
        assert_eq!(plan[0].act, Act::Text { class: 'W', idx: 0 });
        let file = plan.iter().find(|a| a.act == Act::File).expect("dosya");
        assert_eq!(file.due, Duration::from_millis(5_000));
        let close = plan
            .iter()
            .find(|a| a.act == Act::PttClose { spurt: 1 })
            .expect("2. basış kapanışı");
        assert_eq!(close.due, Duration::from_millis(60_000 + 26 * 200));
        let last = plan.last().expect("son");
        assert_eq!(
            last.act,
            Act::Text {
                class: 'T',
                idx: 39
            }
        );
        assert_eq!(last.due, Duration::from_millis(6_000 + 39 * 2_500));
        assert!(plan.windows(2).all(|w| w[0].due <= w[1].due));
    }

    /// Karşı örnek eylemleri: 5 K + KP basışı (10 demet); planlı basışla çakışırsa sonrasına kayar.
    #[test]
    fn karsi_eylemleri() {
        let c = parse("--q-kol Q_bbr").expect("cfg");
        let a = karsi_actions(&c, Duration::from_millis(6_000));
        assert_eq!(a.len(), 5 + 12);
        assert_eq!(a[5].act, Act::PttOpen { spurt: KARSI_SPURT });
        assert_eq!(a[5].due, Duration::from_millis(6_250));
        let b = karsi_actions(&c, Duration::from_millis(14_000));
        let open = b
            .iter()
            .find(|x| x.act == Act::PttOpen { spurt: KARSI_SPURT })
            .expect("KP");
        assert_eq!(open.due, Duration::from_millis(15_000 + 26 * 200 + 200));
    }

    /// Vekil: tükenme tahmini (başlık payı düşülür) ve kalan dosya eşiği.
    #[test]
    fn karsi_vekili() {
        let p = KarsiProbe {
            app_out: 100_000,
            tx_bytes: 100_000 + 80 * Q_SHORT_HDR_EST,
            lost_bytes: 0,
            tx_datagrams: 80,
            file_bytes: 150_480,
            bob_file_rx: 10_000,
        };
        assert_eq!(p.unsent_est(), 0);
        assert!(p.ready(3));
        let lost = KarsiProbe {
            lost_bytes: 1_400,
            ..p
        };
        assert_eq!(
            lost.unsent_est(),
            1_400,
            "kayıp ilan edilen bayt yeniden gönderilecek"
        );
        assert!(!lost.ready(3));
        let near_done = KarsiProbe {
            bob_file_rx: 150_480 - 3 * FILE_UNIT + 1,
            ..p
        };
        assert!(!near_done.ready(3), "kalan < 3 birim");
        assert!(KarsiProbe {
            bob_file_rx: 150_480 - 3 * FILE_UNIT,
            ..p
        }
        .ready(3));
    }

    /// Gövdeler f0_flows ile bayt bayt aynı; çerçeve türü ilk bayt.
    #[test]
    fn govdeler() {
        let t = text_body('T', 7, 64);
        assert_eq!(t.len(), 64);
        assert!(t.starts_with("f0a T00007 ."));
        assert_eq!(parse_text(&t), Some(('T', 7)));
        let s = sig_body(3, 120);
        assert_eq!((s.len(), parse_sig(&s)), (120, Some(3)));
        let b = bundle_body(KARSI_SPURT, 9, 80);
        assert_eq!((b.len(), parse_bundle(&b)), (80, Some((KARSI_SPURT, 9))));
        assert_eq!(file_byte(250), 250);
        assert_eq!(file_byte(251), 0);
        assert_eq!(frame(K_SIG, &s)[0], b'S');
        assert_eq!(spurt_u16(1), [0, 1]);
    }

    /// Başlık: f0_flows `params` alanları + kol tanımı; JSON olarak çözülür.
    #[test]
    fn baslik_json() {
        let c = parse("--q-kol Q_bbr_rfc").expect("cfg");
        let h: serde_json::Value =
            serde_json::from_str(&header_line(&c, &Clock::new())).expect("json");
        assert_eq!(h["k"], "hdr");
        assert_eq!(h["params"]["ptt_bytes"], 80);
        assert_eq!(h["params"]["ptt_at_ms"], serde_json::json!([15000, 60000]));
        assert_eq!(h["quinn"]["arm"], "Q_bbr_rfc");
        assert_eq!(h["quinn"]["iw_bytes"], 14_520);
        assert_eq!(h["quinn"]["prio"], "off");
    }

    /// bob durum dosyası f0_flows biçimini korur, `FB` ekler; alice ayrıştırması tersini verir.
    #[test]
    fn bob_durumu() {
        let mut s = BobState {
            seen: BTreeSet::new(),
            counts: BTreeMap::new(),
            duplicates: 0,
            unparsed: 0,
            file_status: "received",
            file_size: 0,
            file_match: None,
            file_at: None,
            file_rx: 150_480,
            dirty: false,
        };
        assert!(s.count('T', 0, 1));
        assert!(!s.count('T', 0, 1), "yineleme sayılmaz");
        assert!(s.count('P', 1, 0));
        assert_eq!(s.state_text(), "P 1\nT 1\nF received\nFB 150480\n");
        assert_eq!(s.duplicates, 1);
    }
}
