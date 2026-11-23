//! KTP2-F0 Ajan A — akış bağımsızlığı düzeneği (`f0_flows`): İKİ SÜREÇ, gerçek ürün yolu.
//!
//! Her süreç bir ÜRÜN düğümüdür (`kama_node::KamaNode::start_with_link_for_test`, `test-util`):
//! gerçek `ClientTransport` (ürün profili), gerçek `NoiseWireProtection`, `kama-app` motorları
//! (Messenger, FileTransfer, Ptt, Signaling), gerçek UDP. alice (`--role alice`, istemci) ns-A'da,
//! bob (`--role bob`, sunucu) ns-B'de koşar (`scripts/ktp2/f0/f0_cell.sh`); veri datagramları orta
//! ad alanındaki yön başına ayrı netem'den geçer.
//!
//! BEYANLI BOŞLUK (INT-02 ile aynı, DUR-K4G-11 → B): el sıkışma zarfları (INIT/RESPONSE) veri
//! yolundan değil, koşu dizinindeki dosya yolu tabanlı Unix datagram soketlerinden geçer
//! (`relay-alice.sock`, `relay-bob.sock`; [`SockLink`]) — netem DIŞINDA. Ürün taşıyıcısı değildir.
//! Kayıtsız (soketi olmayan) eşe zarf gitmez ve saklanmaz (süreç-içi röleyle aynı anlam).
//!
//! # Senaryolar (`--scenario`)
//!
//! * `dosyasiz` — TEXT + sinyal + PTT (aynı zaman çizelgesi), dosya yok.
//! * `dosyali` — aynısı + bir dosya (`--file-bytes`, `--file-at-ms`); dosyadaki kayıp profilden.
//! * `dosyali_dur` — `dosyali` + alıcı tüketicisinin durması (`--stall-at-ms`, `--stall-ms`).
//! * `dosyasiz_dur` — `dosyasiz` + aynı durma (durmanın tek başına etkisi; betimsel).
//!
//! TÜKETİCİ DURMASI (beyan): FileTransfer alıcısının tüketimini durduran bir uygulama API'si YOKTUR
//! (`kama-app` `file_transfer.rs`: gelen parçalar düğüm iş parçacığında `FileTransfer::handle` ile
//! işlenir; ürünün tek tüketicisi düğüm iş parçacığıdır, `kama-node` `node.rs` `turn`). Durma, bob'un
//! uygulama dinleyicisinin (düğüm iş parçacığında çağrılır) `--stall-ms` boyunca bloklanmasıyla
//! öykünülür: arm anından sonraki ilk `Metrics` olayında. Bu mevcut mimaride BÜTÜN akışların
//! tüketicisini durdurur; dosyaya özgü durma ürün yolunda ifade edilemez.
//!
//! KARŞI ÖRNEK (`--karsi on`, yalnız dosyalı senaryolar): alice motorunun yeni veri kuyruğu boş
//! (`tx_queue_len == 0`) ve teyitsiz birim ≥ `--karsi-min-unacked` (eski dosya parçalarının
//! kurtarması canlı) iken BİR kez `--karsi-texts` TEXT ve `--karsi-bundles` demetlik bir PTT basışı
//! gönderilir.
//!
//! # Zaman damgaları
//!
//! İki süreç aynı makinededir: bütün anlar Unix epoch ms olarak yazılır (süreç başında alınan
//! `SystemTime` + `Instant` çapası; ortak gerçek saat). Katmanlar:
//! uygulama gönderimi (alice `send`), Transport kabulü (alice iz `S`), motor ilk iletimi (alice
//! `ts` örneklerindeki yeni veri kuyruğu derinliğinden türetilir; analiz betiği), alıcı motor kabulü
//! (bob iz `D`: teslim kanalından alınış), sıralı teslim (bob iz `R`: `Reorder::next` ilerlemesi),
//! uygulama teslimi (bob `rx`, dinleyici anı).
//!
//! # Çıktı (koşu dizinine; hüküm YOK — analiz `scripts/ktp2/f0/a/f0a_analyze.py`)
//!
//! `alice.jsonl` / `bob.jsonl` (satır başına bir kayıt: `hdr`, `send`, `rx`, `rcpt`, `ev`, `ts`,
//! `tr`, `mark`, `lg`, `lg_gap`, `rcpt_tx`, `rcpt_wire`, `ack`, `ack_gap`) ve `alice.json` /
//! `bob.json` (tek satır özet). stdout'a da özet basılır.
//!
//! # Ölçüm kancaları (salt gözlem; #252 madde 1 ve 4)
//!
//! * `lg` — ACK başına hat koruması kaydı (`ClientLiveDiag::line_guard`, kaynak
//!   `Sender::diag_line_guard_trace`): korumanın gönderim tavanı (`pace_*`) ve korunan hat hızı
//!   (`rate_*`) ACK'ten önce / sonra, karar sınıfı, yoklama hükmü. `ts.s[].pr` pacer kovasının
//!   hızıdır, hat tavanı DEĞİL; CC ve hızlanma ölçütleri bu satırdan okunur. Yalnız `--live-diag on`
//!   ve hat koruması kuruluyken (geri bildirimi aç yön hızı profili) çıkar. `t` = ACK'in motorda
//!   işlendiği an (ucun saat tabanı + `at_ns`, Unix ms). Sonda aralığında (`--probe-ms`) aynanın
//!   halkasından (64 kayıt) taşan ya da motor halkasından düşen kayıtlar `lg_gap` satırıyla sayılır.
//! * `rcpt_tx` (bob) — TEXT alındısının (RECEIPT) Transport'a kabul ettirildiği an (birim izi `S`;
//!   `Messenger` alındıyı TEXT'i işlerken verir). alice'in `rcpt` satırıyla `mid` üzerinden eşlenir.
//!   Bu an birimin TELE ÇIKIŞ anı değildir (motor kuyruğu / hız sınırı beklemesi dahil değil;
//!   birim başına ilk tel iletim anı üründe açık değildir).
//! * `rcpt_wire` (bob, en iyi çaba, yalnız `--live-diag on`) — alındı biriminin motordaki en eski
//!   kaydının gönderim anı; birim canlı aynanın kurtarma görüntüsünde en eski teyitsiz birim olarak
//!   ilk görüldüğünde (`SenderRecoveryDiag::oldest_unacked`, ≈ 200 ms). `exact:true` = tek emisyon
//!   kararı (orijinal iletim); değilse üst sınır. `rcpt_tx` ile `unit` (+ `sess`) üzerinden eşlenir.
//! * `ack` — bu ucun alıcı motorundan ÇIKAN her ACK çerçevesi (bob'da dosya / metin / PTT yönünün
//!   ACK'leri; alice'te alındı yönününkiler). Kaynak sunucu ucunun canlı aynasıdır
//!   (`ServerLiveDiag::rx_acks`; kayıt ACK gönderildiği anda, motorun kendi saatinden yazılır).
//!   Yalnız `--live-diag on` iken çıkar. Alanlar (anlar Unix ms; eksik alan `null`):
//!   - `t` — ACK'in motordan çıktığı an (çerçeve kuruldu, bütçe jetonu alındı; mühür aynı turda
//!     hemen ardından). `at_ns` aynı anın ucun saat tabanındaki ham değeri.
//!   - `t_evidence` — kanıtın yeterli olduğu an: alarm ACK'inin kanıtla bırakıldığı varış
//!     (`AlarmEvidence::sufficient`). ACK kanıtla bırakılmadıysa `null` (planlı ACK, yükseliş
//!     alarmının ACK'i, alarm yok); neden tahmin edilmez, şu olgular yazılır: `alarm`, `rise`,
//!     `detects` / `t_detect` (bu ACK aralığındaki daralma algılaması), `wait_open` (algılama
//!     oldu, kanıt yetmeden planlı ACK çıktı).
//!   - `t_ready` — ACK'in hazır olduğu an: alarm ACK'inde alarm kuralının kurulduğu an
//!     (`t_alarm`; daralmada `t_evidence` ile aynı), öbürlerinde önceki ACK'ten sonraki ilk
//!     gönderim denemesi (`t_first_try`). `t − t_ready` ertelenme süresidir.
//!   - `defer` — önceki ACK'ten bu ACK'e dek ertelenen deneme sayısı, nedene göre: `budget`
//!     (bütçe jetonu yetmedi), `yield` (dolu dönüş hattında veriye yol verdi), `no_info`
//!     (bilgi taşımayan kopya), `first_window` (ilk ACK ilk varış penceresini bekledi);
//!     `t_first_defer` ilkinin anı. `defer_alarm` — alarm kuralı kurulduktan sonrakiler.
//!   - `t_wire` — mühür + egress kapısı + `send_to` döndükten sonraki an (çerçeve çekirdeğe
//!     verildi). Arayüz kuyruğundaki (qdisc / netem) bekleme bu saatte görünmez. `wire` ≠
//!     `"sent"` ise (`egress_refused`, `seal_failed`) `null`.
//!   - `type` — çerçeve türü (`0x10` / `0x14` / `0x15` / `0x16` / `0x17`), `timer` — zamanlayıcı
//!     yolundan mı, `cum` — yuvadaki motorun kümülatif sayaçları (algılama, kanıtlı bırakma,
//!     yükseliş, kanıtsız biten bekleme, ertelenmeler).
//!   - `motor:false` — motor kayıt tutmadı (bütçesiz yol / eski motor): `t` motora verilen
//!     saattir, motor alanları `null`.
//!   - `sampled:false` — bütün anlar motorun ve gönderim yolunun kendi saatinden, ACK başına
//!     kesindir; sonda aralığı (`--probe-ms`) yalnız satırın dosyaya yazılma gecikmesidir. Sonda
//!     aralığında aynanın halkasından (64 kayıt) taşan ACK'ler `ack_gap` satırıyla sayılır.
//! * Özet: `ack_frames` (canlı aynanın ACK çerçevesi sayaçları; ACK türleri `wire.ack_rx`'te),
//!   `lg` (yazılan satır / boşluk sayıları), `ack_tx` (yazılan `ack` satırları: tür, alarm, kanıt
//!   anı dolu, tele çıkan sayıları), bob'da `rcpt_tx` (eşleme sayıları).

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write as _};
use std::net::SocketAddr;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use adaptive_transport::line_guard::FirstWindowOutcome;
use adaptive_transport::sender::LineGuardAckTrace;
use client_core::runtime::{RxAckDiag, RxAckWire, ServerLiveDiag};
use client_core::transport::{
    EnvelopeLink, LinkAttempt, LinkError, LinkEvent, UnitTraceKind, UnitTraceRecord,
};
use kama_app::codec::limits::{APP_RECORD_HEADER_BYTES, RECEIPT_BODY_BYTES};
use kama_app::codec::PttCodec;
use kama_app::messaging::MessengerError;
use kama_app::ptt::PttError;
use kama_app::signaling::SignalingError;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::TransportError;
use kama_contracts::s3::HubHandle;
use kama_node::{
    FileHandle, FileRef, FileStage, KamaNode, NodeError, NodeEvent, NodeListener, NodeProbe,
    SessionProbe, StreamHandle,
};
use kama_noise_wire::entropy::OsEntropy;
use kama_provision::format::DeviceName;
use kama_provision::provision::{init, InitRequest};

/// Çıktı şeması.
const SCHEMA: &str = "f0a-flows-v1";
/// Provisioning cihaz adları.
const ALICE: &str = "alice";
const BOB: &str = "bob";

/// Koşu dizinindeki (çalışma dizini) dosyalar. Göreli yollar: Unix soket yolu 108 bayt sınırına
/// takılmasın diye süreç koşu dizinine geçer (`--dir`).
const SOCK_ALICE: &str = "relay-alice.sock";
const SOCK_BOB: &str = "relay-bob.sock";
const PROV_DIR: &str = "prov";
const PROV_READY: &str = "prov/READY";
const SYNC_DIR: &str = "sync";
const MARK_BOB_UP: &str = "sync/bob.up";
const MARK_T0: &str = "sync/t0";
const MARK_ALICE_DONE: &str = "sync/alice.done";
const MARK_BOB_EXIT: &str = "sync/bob.exit";
const BOB_STATE: &str = "sync/bob.state";
const SRC_FILE: &str = "src-file.bin";

/// Zarf soketi çerçevesi: `F0` · tür · tutamaç u32 · zarf türü u8 · uzunluk u32 · bayt.
const LINK_MAGIC: [u8; 2] = *b"F0";
const LINK_KIND_INIT: u8 = 1;
const LINK_KIND_RESPONSE: u8 = 2;
const LINK_HDR: usize = 2 + 1 + 4 + 1 + 4;
const LINK_MAX_FRAME: usize = 64 * 1024;

/// Dinleyici → sürücü olay kuyruğu tavanı (dolarsa olay düşer, sayılır; düğümü bekletmez).
const EVENT_QUEUE_CAP: usize = 16_384;
/// Olay bekleme adımı.
const PUMP_STEP: Duration = Duration::from_millis(5);
/// Geri basınçta yeniden deneme beklemesi.
const RETRY_WAIT: Duration = Duration::from_millis(10);
/// İşaret dosyası yoklama adımı.
const MARK_POLL: Duration = Duration::from_millis(20);
/// alice'in bob'un çıkışını bekleme üst sınırı.
const BOB_EXIT_WAIT: Duration = Duration::from_secs(20);
/// bob'un alice.done'u bekleme payı (alice'in planı + pencere üstüne).
const BOB_SLACK: Duration = Duration::from_secs(60);
/// Karşı örnek basışının akış kimliği (sürücü numarası; telde değil).
const KARSI_SPURT: u32 = 90;
/// PTT basışları arası en küçük boşluk (eş başına tek açık akış).
const SPURT_GAP: Duration = Duration::from_millis(200);
/// Dosya içeriği deseni: asal modül.
const FILE_PATTERN: usize = 251;
/// TEXT gövde öneki: `f0a <sınıf><sıra:05> ` (11 bayt).
const TEXT_PREFIX: usize = 11;

/// Girdi tavanları (SR-AUDIT-001: kayıtlar bellekte).
const MAX_ITEMS: u32 = 5_000;
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SPURTS: usize = 8;

const USAGE: &str = "\
kullanım: f0_flows --role alice|bob --dir <koşu dizini> --alice-addr IP:PORT --bob-addr IP:PORT
                --scenario dosyasiz|dosyali|dosyali_dur|dosyasiz_dur --profile <ad> --rep <N>
                [--file-bytes B=524700] [--file-at-ms MS=5000]
                [--warmup-texts N=3] [--warmup-every-ms MS=500]
                [--texts N=40] [--text-at-ms MS=6000] [--text-every-ms MS=2500] [--text-bytes B=64]
                [--sigs N=10] [--sig-at-ms MS=7000] [--sig-every-ms MS=10000] [--sig-bytes B=120]
                [--ptt-at-ms MS,MS=15000,60000] [--ptt-bundles N=25] [--ptt-bundle-ms MS=200]
                [--ptt-bytes B=40]
                [--stall-at-ms MS=20000] [--stall-ms MS=30000]
                [--karsi on|off=on] [--karsi-texts N=5] [--karsi-every-ms MS=500]
                [--karsi-bundles N=10] [--karsi-min-unacked N=3] [--karsi-text-bytes B=48]
                [--window-s S=300] [--receipt-grace-s S=30] [--connect-timeout-s S=30]
                [--probe-ms MS=200]
                [--live-diag on|off=on]
       f0_flows --help

İki süreç aynı argümanlarla (yalnız --role farklı) başlatılır; önce bob. bob provisioning'i koşu
dizinine yazar (prov/), alice prov/READY ve sync/bob.up işaretlerini bekler. Zarflar Unix
soketlerinden (netem dışı, beyanlı), veri gerçek UDP'den. Çıktı: <rol>.jsonl + <rol>.json.
Zaman ofsetleri alice'in Connected anına (t0) göredir; bob durmayı sync/t0'dan hizalar.
";

// ── Yapılandırma ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Alice,
    Bob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Dosyasiz,
    Dosyali,
    DosyaliDur,
    DosyasizDur,
}

impl Scenario {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "dosyasiz" => Self::Dosyasiz,
            "dosyali" => Self::Dosyali,
            "dosyali_dur" => Self::DosyaliDur,
            "dosyasiz_dur" => Self::DosyasizDur,
            _ => return None,
        })
    }

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

#[derive(Debug, Clone)]
struct Config {
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
    /// [PTT-BİRLEŞİK · pilot · #233] `--ptt-pilot on`: basış `KamaNode::ptt_open_press` ile açılır
    /// (çağıranın süresi `--ptt-bundle-ms`). Düğüm SEÇTİĞİ süreyi ve birleşik biçimin kullanılıp
    /// kullanılmadığını döner; bu basışın kalan demetleri o süreyle yeniden zamanlanır ve yük aynı
    /// ses hızıyla ölçeklenir (`--ptt-bytes` × seçilen / çağıranın süresi); demetler
    /// `ptt_push_timed` ile (sonuncusu `last`) verilir. Seçim `send` kaydına yazılır
    /// (`bundle_ms`, `coalesced`). Düğümün pilot ayarı ortamdan gelir (`KAMA_PTT_PILOT`,
    /// `KAMA_LINK_RATES`); ayar kapalıysa ya da eş doğrulanmamışsa düğüm çağıranın süresini döner.
    ptt_pilot: bool,
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
    /// bob her öğeyi aldıktan sonra TEXT alındıları (RECEIPT) için ek bekleme üst sınırı.
    receipt_grace: Duration,
}

const KNOWN_FLAGS: [&str; 39] = [
    "--role",
    "--dir",
    "--alice-addr",
    "--bob-addr",
    "--scenario",
    "--profile",
    "--rep",
    "--file-bytes",
    "--file-at-ms",
    "--warmup-texts",
    "--warmup-every-ms",
    "--texts",
    "--text-at-ms",
    "--text-every-ms",
    "--text-bytes",
    "--sigs",
    "--sig-at-ms",
    "--sig-every-ms",
    "--sig-bytes",
    "--ptt-at-ms",
    "--ptt-bundles",
    "--ptt-bundle-ms",
    "--ptt-bytes",
    "--ptt-pilot",
    "--stall-at-ms",
    "--stall-ms",
    "--karsi",
    "--karsi-texts",
    "--karsi-every-ms",
    "--karsi-bundles",
    "--karsi-min-unacked",
    "--karsi-text-bytes",
    "--window-s",
    "--connect-timeout-s",
    "--probe-ms",
    "--live-diag",
    "--receipt-grace-s",
    "--help",
    "-h",
];

enum Parsed {
    Help,
    Run(Box<Config>),
}

fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut values: Flags<'_> = BTreeMap::new();
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
    let v = &values;
    let role = match required(v, "--role")? {
        "alice" => Role::Alice,
        "bob" => Role::Bob,
        other => return Err(format!("--role alice|bob olmalı: {other:?}")),
    };
    let scenario_name = required(v, "--scenario")?;
    let scenario = Scenario::parse(scenario_name)
        .ok_or_else(|| format!("bilinmeyen senaryo {scenario_name:?}"))?;
    let ptt_at = match values.get("--ptt-at-ms").copied() {
        None => vec![Duration::from_millis(15_000), Duration::from_millis(60_000)],
        Some("" | "none") => Vec::new(),
        Some(list) => list
            .split(',')
            .map(|v| {
                v.trim()
                    .parse::<u64>()
                    .map(Duration::from_millis)
                    .map_err(|_| format!("--ptt-at-ms değeri sayı değil: {v:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    let bundle_ms = number(v, "--ptt-bundle-ms", 200)?;
    let cfg = Config {
        role,
        dir: PathBuf::from(required(v, "--dir")?),
        alice_addr: address(v, "--alice-addr")?,
        bob_addr: address(v, "--bob-addr")?,
        scenario,
        profile: required(v, "--profile")?.to_owned(),
        rep: number_u32(v, "--rep", 0)?,
        file_bytes: number_usize(v, "--file-bytes", 524_700)?,
        file_at: millis(v, "--file-at-ms", 5_000)?,
        warmup_texts: number_u32(v, "--warmup-texts", 3)?,
        warmup_every: millis(v, "--warmup-every-ms", 500)?,
        texts: number_u32(v, "--texts", 40)?,
        text_at: millis(v, "--text-at-ms", 6_000)?,
        text_every: millis(v, "--text-every-ms", 2_500)?,
        text_bytes: number_usize(v, "--text-bytes", 64)?,
        sigs: number_u32(v, "--sigs", 10)?,
        sig_at: millis(v, "--sig-at-ms", 7_000)?,
        sig_every: millis(v, "--sig-every-ms", 10_000)?,
        sig_bytes: number_usize(v, "--sig-bytes", 120)?,
        ptt_at,
        ptt_bundles: number_u32(v, "--ptt-bundles", 25)?,
        ptt_bundle_ms: u8::try_from(bundle_ms)
            .map_err(|_| "--ptt-bundle-ms ≤ 255 olmalı".to_owned())?,
        ptt_bytes: number_usize(v, "--ptt-bytes", 40)?,
        ptt_pilot: on_off(v, "--ptt-pilot", false)?,
        stall_at: millis(v, "--stall-at-ms", 20_000)?,
        stall: millis(v, "--stall-ms", 30_000)?,
        karsi: on_off(v, "--karsi", true)?,
        karsi_texts: number_u32(v, "--karsi-texts", 5)?,
        karsi_every: millis(v, "--karsi-every-ms", 500)?,
        karsi_bundles: number_u32(v, "--karsi-bundles", 10)?,
        karsi_min_unacked: number_u32(v, "--karsi-min-unacked", 3)?,
        karsi_text_bytes: number_usize(v, "--karsi-text-bytes", 48)?,
        window: Duration::from_secs(number(v, "--window-s", 300)?),
        connect_timeout: Duration::from_secs(number(v, "--connect-timeout-s", 30)?),
        probe_every: millis(v, "--probe-ms", 200)?,
        live_diag: on_off(v, "--live-diag", true)?,
        receipt_grace: Duration::from_secs(number(v, "--receipt-grace-s", 30)?),
    };
    validate(&cfg)?;
    Ok(Parsed::Run(Box::new(cfg)))
}

type Flags<'a> = BTreeMap<&'a str, &'a str>;

fn required<'a>(values: &Flags<'a>, flag: &str) -> Result<&'a str, String> {
    values
        .get(flag)
        .copied()
        .ok_or_else(|| format!("{flag} zorunlu"))
}

fn number(values: &Flags<'_>, flag: &str, default: u64) -> Result<u64, String> {
    match values.get(flag) {
        None => Ok(default),
        Some(text) => text
            .parse()
            .map_err(|_| format!("{flag} değeri sayı değil: {text:?}")),
    }
}

fn number_u32(values: &Flags<'_>, flag: &str, default: u64) -> Result<u32, String> {
    u32::try_from(number(values, flag, default)?).map_err(|_| format!("{flag} çok büyük"))
}

fn number_usize(values: &Flags<'_>, flag: &str, default: u64) -> Result<usize, String> {
    usize::try_from(number(values, flag, default)?).map_err(|_| format!("{flag} çok büyük"))
}

fn millis(values: &Flags<'_>, flag: &str, default: u64) -> Result<Duration, String> {
    number(values, flag, default).map(Duration::from_millis)
}

fn on_off(values: &Flags<'_>, flag: &str, default: bool) -> Result<bool, String> {
    match values.get(flag).copied() {
        None => Ok(default),
        Some("on") => Ok(true),
        Some("off") => Ok(false),
        Some(other) => Err(format!("{flag} on|off olmalı: {other:?}")),
    }
}

fn address(values: &Flags<'_>, flag: &str) -> Result<SocketAddr, String> {
    let text = required(values, flag)?;
    text.parse()
        .map_err(|_| format!("{flag} IP:PORT değil: {text:?}"))
}

fn validate(cfg: &Config) -> Result<(), String> {
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
    // Basışlar sırayla ve çakışmasız (eş başına tek açık giden akış).
    let len = spurt_len(cfg, cfg.ptt_bundles);
    for pair in cfg.ptt_at.windows(2) {
        if pair[1] < pair[0] + len + SPURT_GAP {
            return Err("--ptt-at-ms artan ve çakışmasız olmalı".to_owned());
        }
    }
    Ok(())
}

/// Bir basışın süresi: açılış · `n` demet (`ptt_bundle_ms` aralıklı) · kapanış.
fn spurt_len(cfg: &Config, bundles: u32) -> Duration {
    spurt_step(cfg).saturating_mul(bundles.saturating_add(1))
}

// ── Saat ─────────────────────────────────────────────────────────────────────────────────────

/// Süreç-içi `Instant` → Unix epoch (ortak gerçek saat; iki süreç aynı makinede).
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

    fn now_ns(&self) -> u64 {
        self.unix_ns(Instant::now())
    }
}

// ── JSON / JSONL ─────────────────────────────────────────────────────────────────────────────

fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

fn opt_ms(clock: &Clock, t: Option<Instant>) -> String {
    t.map_or_else(|| "null".to_owned(), |t| format!("{:.3}", clock.ms(t)))
}

fn opt_u(v: Option<u64>) -> String {
    v.map_or_else(|| "null".to_owned(), |v| v.to_string())
}

fn ns_ms(ns: u64) -> String {
    if ns == 0 {
        "null".to_owned()
    } else {
        format!("{:.3}", ns as f64 / 1e6)
    }
}

/// Satır başına bir JSON kaydı (tamponlu; her sürücü turunda boşaltılır).
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

// ── İşaret dosyaları ─────────────────────────────────────────────────────────────────────────

/// Atomik yazım: geçici dosya + yeniden adlandırma.
fn write_marker(path: &str, body: &str) -> Result<(), String> {
    let tmp = format!("{path}.tmp");
    fs::write(&tmp, body).map_err(|e| format!("{tmp}: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("{path}: {e}"))
}

fn read_marker(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn wait_marker(path: &str, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(body) = read_marker(path) {
            return Some(body);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(MARK_POLL);
    }
}

// ── Zarf taşıyıcısı: dosya yolu tabanlı Unix datagram soketi (netem dışı, beyanlı) ─────────────

#[derive(Debug, Default)]
struct LinkCounters {
    inits: AtomicU64,
    responses: AtomicU64,
    received: AtomicU64,
    send_errors: AtomicU64,
    malformed: AtomicU64,
}

impl LinkCounters {
    fn json(&self) -> String {
        format!(
            "{{\"inits\":{},\"responses\":{},\"received\":{},\"send_errors\":{},\"malformed\":{}}}",
            self.inits.load(Ordering::Relaxed),
            self.responses.load(Ordering::Relaxed),
            self.received.load(Ordering::Relaxed),
            self.send_errors.load(Ordering::Relaxed),
            self.malformed.load(Ordering::Relaxed)
        )
    }
}

/// `client_core::transport::EnvelopeLink` — iki süreç arası zarf taşıyıcısı. Süreç-içi rölenin
/// (`kama-node` `carrier.rs`) anlamı: `send_init` tutamacı atar, başlatana `HandleAssigned`, eşe AYNI
/// tutamaçla `IncomingInit`; `send_response` eşe `Response`. Tutamaç uzayları ayrık (alice tek,
/// bob çift; 0 verilmez, sarma YOK). Eşin soketi yoksa `Err(LinkError)`; hiçbir şey saklanmaz.
/// Tek eşli düzenek: `peer` yok sayılır (her zarf diğer sürece gider).
struct SockLink {
    sock: UnixDatagram,
    peer_path: PathBuf,
    next_handle: u32,
    next_attempt: u64,
    local: VecDeque<LinkEvent>,
    buf: Vec<u8>,
    counters: Arc<LinkCounters>,
}

impl SockLink {
    fn bind(
        own: &str,
        peer: &str,
        first_handle: u32,
        counters: Arc<LinkCounters>,
    ) -> Result<Self, String> {
        let _ = fs::remove_file(own);
        let sock = UnixDatagram::bind(own).map_err(|e| format!("zarf soketi {own}: {e}"))?;
        sock.set_nonblocking(true)
            .map_err(|e| format!("zarf soketi {own}: {e}"))?;
        Ok(Self {
            sock,
            peer_path: PathBuf::from(peer),
            next_handle: first_handle,
            next_attempt: 1,
            local: VecDeque::new(),
            buf: vec![0; LINK_MAX_FRAME],
            counters,
        })
    }

    fn send_frame(
        &self,
        kind: u8,
        handle: u32,
        envelope_type: u8,
        bytes: &[u8],
    ) -> Result<(), LinkError> {
        if LINK_HDR + bytes.len() > LINK_MAX_FRAME {
            return Err(LinkError);
        }
        let len = u32::try_from(bytes.len()).map_err(|_| LinkError)?;
        let mut frame = Vec::with_capacity(LINK_HDR + bytes.len());
        frame.extend_from_slice(&LINK_MAGIC);
        frame.push(kind);
        frame.extend_from_slice(&handle.to_be_bytes());
        frame.push(envelope_type);
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(bytes);
        match self.sock.send_to(&frame, &self.peer_path) {
            Ok(n) if n == frame.len() => Ok(()),
            _ => {
                self.counters.send_errors.fetch_add(1, Ordering::Relaxed);
                Err(LinkError)
            }
        }
    }
}

fn decode_frame(frame: &[u8]) -> Option<LinkEvent> {
    if frame.len() < LINK_HDR || frame[..2] != LINK_MAGIC {
        return None;
    }
    let kind = frame[2];
    let handle = u32::from_be_bytes(frame[3..7].try_into().ok()?);
    let envelope_type = frame[7];
    let len = usize::try_from(u32::from_be_bytes(frame[8..12].try_into().ok()?)).ok()?;
    let bytes = frame.get(LINK_HDR..)?;
    if bytes.len() != len || handle == 0 {
        return None;
    }
    let hub_handle = HubHandle::from_raw(handle);
    let bytes = bytes.to_vec();
    match kind {
        LINK_KIND_INIT => Some(LinkEvent::IncomingInit {
            hub_handle,
            envelope_type,
            bytes,
        }),
        LINK_KIND_RESPONSE => Some(LinkEvent::Response {
            hub_handle,
            envelope_type,
            bytes,
        }),
        _ => None,
    }
}

impl EnvelopeLink for SockLink {
    fn send_init(
        &mut self,
        _peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        let handle = self.next_handle;
        let following_handle = handle.checked_add(2).ok_or(LinkError)?;
        let attempt = self.next_attempt;
        let following_attempt = attempt.checked_add(1).ok_or(LinkError)?;
        self.send_frame(LINK_KIND_INIT, handle, envelope_type, &bytes)?;
        self.next_handle = following_handle;
        self.next_attempt = following_attempt;
        self.counters.inits.fetch_add(1, Ordering::Relaxed);
        self.local.push_back(LinkEvent::HandleAssigned {
            attempt: LinkAttempt(attempt),
            hub_handle: HubHandle::from_raw(handle),
        });
        Ok(LinkAttempt(attempt))
    }

    fn send_response(
        &mut self,
        _peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        self.send_frame(LINK_KIND_RESPONSE, hub_handle.raw(), envelope_type, &bytes)?;
        self.counters.responses.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        if let Some(event) = self.local.pop_front() {
            return Some(event);
        }
        loop {
            let n = self.sock.recv(&mut self.buf).ok()?;
            match decode_frame(&self.buf[..n]) {
                Some(event) => {
                    self.counters.received.fetch_add(1, Ordering::Relaxed);
                    return Some(event);
                }
                None => {
                    self.counters.malformed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

// ── Dinleyici ve tüketici durması ────────────────────────────────────────────────────────────

/// Tüketici durması (bkz. modül belgesi): arm anından sonraki ilk `Metrics` olayında dinleyici
/// `length` boyunca bloklar (düğüm iş parçacığı). Bir kez.
struct Stall {
    clock: Clock,
    armed_at_ns: AtomicU64,
    length: Duration,
    started_ns: AtomicU64,
    ended_ns: AtomicU64,
}

impl Stall {
    fn arm(&self, at_ns: u64) {
        let _ =
            self.armed_at_ns
                .compare_exchange(0, at_ns.max(1), Ordering::AcqRel, Ordering::Acquire);
    }

    fn maybe_block(&self) {
        let armed = self.armed_at_ns.load(Ordering::Acquire);
        if armed == 0 {
            return;
        }
        let now = self.clock.now_ns();
        if now < armed {
            return;
        }
        if self
            .started_ns
            .compare_exchange(0, now.max(1), Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        thread::sleep(self.length);
        self.ended_ns
            .store(self.clock.now_ns().max(1), Ordering::Release);
    }
}

struct Stamped {
    at: Instant,
    event: NodeEvent,
}

/// Olayları damgalayıp sınırlı kuyruğa koyan dinleyici (düğüm iş parçacığında çağrılır).
struct Tap {
    tx: SyncSender<Stamped>,
    dropped: Arc<AtomicU64>,
    stall: Option<Arc<Stall>>,
}

impl NodeListener for Tap {
    fn on_event(&self, event: NodeEvent) {
        if matches!(event, NodeEvent::Metrics(_)) {
            if let Some(stall) = &self.stall {
                stall.maybe_block();
            }
            return;
        }
        let stamped = Stamped {
            at: Instant::now(),
            event,
        };
        if self.tx.try_send(stamped).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// ── Öğe kodlaması ────────────────────────────────────────────────────────────────────────────

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

/// [PTT-BİRLEŞİK · pilot · ölçüm] Pilot basışının demet yükündeki zaman alanları (4. bayttan):
/// basış başı (Unix µs, u64) · demetin ilk çerçevesinin oluşumu (Unix µs, u64) · demet süresi
/// (ms, u16) · basıştaki demet sayısı (u16). İki süreç aynı makinede, ortak gerçek saat.
const PRESS_TIMING_BYTES: usize = 4 + 8 + 8 + 2 + 2;

/// Pilot basışının demet yükü: [`bundle_body`] + zaman alanları (yük yeterince uzunsa).
fn press_body(spurt: u32, idx: u32, len: usize, press_us: u64, step_ms: u16, n: u32) -> Vec<u8> {
    let mut b = bundle_body(spurt, idx, len);
    if b.len() >= PRESS_TIMING_BYTES {
        let created_us = press_us + u64::from(step_ms) * 1_000 * u64::from(idx);
        b[4..12].copy_from_slice(&press_us.to_be_bytes());
        b[12..20].copy_from_slice(&created_us.to_be_bytes());
        b[20..22].copy_from_slice(&step_ms.to_be_bytes());
        b[22..24].copy_from_slice(&u16::try_from(n).unwrap_or(u16::MAX).to_be_bytes());
    }
    b
}

/// [`press_body`] zaman alanları: (basış başı ms, oluşum ms, demet süresi ms, demet sayısı).
/// Alanlar dolu değilse (pilot dışı yük: 0xA5 dolgusu) `None`.
fn parse_press_timing(bytes: &[u8]) -> Option<(f64, f64, u32, u32)> {
    if bytes.len() < PRESS_TIMING_BYTES {
        return None;
    }
    let press_us = u64::from_be_bytes(bytes[4..12].try_into().ok()?);
    let created_us = u64::from_be_bytes(bytes[12..20].try_into().ok()?);
    let step_ms = u16::from_be_bytes(bytes[20..22].try_into().ok()?);
    let n = u16::from_be_bytes(bytes[22..24].try_into().ok()?);
    if step_ms == 0xA5A5 || step_ms == 0 || created_us < press_us {
        return None;
    }
    Some((
        press_us as f64 / 1e3,
        created_us as f64 / 1e3,
        u32::from(step_ms),
        u32::from(n),
    ))
}

/// [TEL-KANITI · #233] Koşunun kabul kipini KENDİ kaydından kanıtlayan alanlar (düğüm sondasından;
/// canlı ayna kapalıysa ACK / CAPS / 0x09 sayaçları 0'dır ve `live_diag` bunu söyler): gelen ACK
/// çerçeveleri türe göre, gönderilen CAPS, yalın 0x09 iletimi, gönderilen birleşik demetler
/// (açılışlı / kompakt / END) ve alıcı PTT sayaçları. `ptt_wire_bytes` / `ptt_wire_datagrams`: bu
/// ucun mühürlediği bütün PTT şeridi datagramlarının tel baytı (IPv4/UDP 28 B dahil; koşunun
/// bütün basışları) — bant kapısının (5000 bit/s × basış ÷ 8) girdisi.
fn wire_json(node: &KamaNode, live_diag: bool) -> String {
    let Ok(probe) = node.probe_for_test() else {
        return "null".to_owned();
    };
    let wire = probe
        .sessions
        .iter()
        .fold(probe.retired.wire, |acc, s| acc.plus(s.diag.wire));
    let a = wire.ack_rx_types;
    let p = probe.ptt;
    format!(
        "{{\"live_diag\":{live_diag},\"ack_rx\":{{\"0x10\":{},\"0x14\":{},\"0x15\":{},\"0x16\":{},\"0x17\":{}}},\
         \"caps_sends\":{},\"ptt_lean_0x09_tx\":{},\"ptt_wire_bytes\":{},\"ptt_wire_datagrams\":{},\"spurt_tx\":{{\"open\":{},\"compact\":{},\"end\":{}}},\
         \"ptt_rx\":{{\"spurt_opens\":{},\"spurt_ends\":{},\"ended_by_gap\":{},\"orphan_bundles\":{},\
         \"spurt_refused\":{}}}}}",
        a[0],
        a[1],
        a[2],
        a[3],
        a[4],
        wire.caps_sends,
        wire.ptt_lean_tx,
        wire.ptt_wire_bytes,
        wire.ptt_wire_datagrams,
        p.spurt_open_tx,
        p.spurt_compact_tx,
        p.spurt_end_tx,
        p.spurt_opens,
        p.spurt_ends,
        p.streams_ended_by_gap,
        p.ptt_orphan_bundles,
        p.spurt_refused,
    )
}

/// Özet nesnesinin sonuna alan ekler (`{…}` → `{…,<extra>}`).
fn with_extra(summary: String, extra: &str) -> String {
    match summary.strip_suffix('}') {
        Some(head) => format!("{head},{extra}}}"),
        None => summary,
    }
}

/// [PTT-BİRLEŞİK · pilot · ölçüm] Bob'un bir akış (basış) için gözlemi.
#[derive(Default)]
struct PressRx {
    bundle_ms: u16,
    started: u32,
    ended: u32,
    ended_at: Option<f64>,
    /// Basış başı (ms), demet süresi (ms), basıştaki demet sayısı — ilk zaman alanlı demetten.
    press: Option<(f64, u32, u32)>,
    /// Demet sırası → (oluşum ms, teslim ms); ilk teslim.
    bundles: BTreeMap<u32, (f64, f64)>,
}

impl PressRx {
    fn json(&self, stream: u32) -> String {
        let mut lat: Vec<f64> = self.bundles.values().map(|(c, r)| r - c).collect();
        lat.sort_by(f64::total_cmp);
        let pick = |i: usize| {
            lat.get(i)
                .map_or_else(|| "null".to_owned(), |v| format!("{v:.1}"))
        };
        let (press_ms, step, n) = self.press.unwrap_or((0.0, 0, 0));
        let frames_per = u64::from(step / 20);
        let on_time = self
            .bundles
            .values()
            .filter(|(c, r)| r - c <= 1_000.0)
            .count() as u64
            * frames_per;
        let first_audio = self
            .bundles
            .values()
            .map(|(_, r)| *r)
            .min_by(f64::total_cmp)
            .filter(|_| self.press.is_some())
            .map_or_else(|| "null".to_owned(), |r| format!("{:.1}", r - press_ms));
        let release = press_ms + f64::from(step) * f64::from(n);
        let release_to_ended = match (self.ended_at, self.press) {
            (Some(e), Some(_)) => format!("{:.1}", e - release),
            _ => "null".to_owned(),
        };
        format!(
            "{{\"stream\":{stream},\"bundle_ms\":{},\"ptt_started\":{},\"ptt_ended\":{},\
             \"bundles_planned\":{n},\"bundles_delivered\":{},\"frames_planned\":{},\
             \"frames_on_time\":{on_time},\"first_audio_ms\":{first_audio},\
             \"creation_to_delivery_ms\":{{\"min\":{},\"median\":{},\"max\":{}}},\
             \"release_to_ended_ms\":{release_to_ended}}}",
            self.bundle_ms,
            self.started,
            self.ended,
            self.bundles.len(),
            u64::from(n) * frames_per,
            pick(0),
            pick(lat.len() / 2),
            pick(lat.len().saturating_sub(1)),
        )
    }
}

fn file_byte(i: usize) -> u8 {
    u8::try_from(i % FILE_PATTERN).unwrap_or(0)
}

// ── Sonda ve iz satırları ────────────────────────────────────────────────────────────────────

fn session_json(s: &mut String, clock: &Clock, sp: &SessionProbe) {
    let d = &sp.diag;
    let c = &d.control;
    let srtt = if d.rtt_samples > 0 {
        format!("{:.3}", d.srtt_us as f64 / 1000.0)
    } else {
        "null".to_owned()
    };
    let state = c
        .state
        .map_or_else(|| "null".to_owned(), |st| js(&format!("{st:?}")));
    let peer = sp.peer_id.as_deref().map_or_else(|| "null".to_owned(), js);
    let _ = write!(
        s,
        "{{\"sess\":{},\"peer\":{},\"rtx\":{},\"tlp\":{},\"dup\":{},\"par\":{},\"sup\":{},\
         \"txb\":{},\"rtxb\":{},\"txd\":{},\"rxd\":{},\"srtt\":{},\"bw\":{},\"pr\":{},\"qd\":{:.3},\
         \"inb\":{},\"st\":{}",
        js(&format!("{:?}", sp.session)),
        peer,
        d.retransmits,
        d.tail_probes,
        d.dups,
        d.parity_emitted,
        d.recovery_suppressed,
        d.tx_bytes,
        d.tx_retx_bytes,
        d.tx_datagrams,
        d.rx_datagrams,
        srtt,
        c.btlbw_bps,
        c.pacer_rate_bps,
        c.queue_delay_us as f64 / 1000.0,
        c.inflight_bytes,
        state
    );
    match &d.recovery {
        Some(r) => {
            let t = &r.timers;
            let _ = write!(
                s,
                ",\"rec\":{{\"ta\":{:.3},\"un\":{},\"txq\":{},\"pl\":{},\"inf\":{},\"dq\":{},\
                 \"ts\":{}}}",
                clock.ms(r.taken_at),
                r.unacked_units,
                t.tx_queue_len,
                t.pending_lost_len,
                t.inflight_len,
                t.dup_queue_len,
                js(&format!("{:?}", t.state))
            );
        }
        None => s.push_str(",\"rec\":null"),
    }
    match sp.reorder {
        Some(r) => {
            let _ = write!(
                s,
                ",\"reo\":{{\"n\":{},\"h\":{},\"f\":{}}}",
                r.next_unit,
                r.held_units,
                opt_u(r.first_held_unit)
            );
        }
        None => s.push_str(",\"reo\":null"),
    }
    s.push('}');
}

fn ts_line(clock: &Clock, at: Instant, p: &NodeProbe) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"k\":\"ts\",\"t\":{:.3},\"out\":{},\"nrd\":{},\"dcd\":{},\"cmd\":{},\"s\":[",
        clock.ms(at),
        p.outstanding_units,
        p.not_ready_drops,
        p.delivery_channel_drops,
        p.ctx_mismatch_drops
    );
    for (i, sp) in p.sessions.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        session_json(&mut s, clock, sp);
    }
    s.push_str("],\"ro\":[");
    for (i, r) in p.reorder_other.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"n\":{},\"h\":{},\"f\":{}}}",
            r.next_unit,
            r.held_units,
            opt_u(r.first_held_unit)
        );
    }
    let m = &p.messenger;
    let _ = write!(
        s,
        "],\"app\":{{\"rdrop\":{},\"unk\":{},\"dupt\":{},\"below\":{},\"rsent\":{},\
         \"runsent\":{},\"rign\":{},\"resent\":{},\"refused\":{},\"store\":{}}}}}",
        m.records_dropped,
        m.units_unknown_session,
        m.duplicate_texts,
        m.below_window_texts,
        m.receipts_sent,
        m.receipts_unsent,
        m.receipts_ignored,
        m.resent_on_new_session,
        m.resend_refused,
        m.store_errors
    );
    s
}

fn trace_line(clock: &Clock, at: Instant, records: &[UnitTraceRecord], dropped: u64) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"k\":\"tr\",\"t\":{:.3},\"drop\":{},\"r\":[",
        clock.ms(at),
        dropped
    );
    for (i, r) in records.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let kind = match r.kind {
            UnitTraceKind::Sent => "S",
            UnitTraceKind::Drained => "D",
            UnitTraceKind::Released => "R",
        };
        let _ = write!(
            s,
            "[\"{}\",{},{},{},{:.3}]",
            kind,
            js(&format!("{:?}", r.session)),
            r.unit,
            r.len,
            clock.ms(r.at)
        );
    }
    s.push_str("]}");
    s
}

fn mark_line(clock: &Clock, at: Instant, what: &str, extra: &str) -> String {
    format!(
        "{{\"k\":\"mark\",\"t\":{:.3},\"m\":{}{}}}",
        clock.ms(at),
        js(what),
        extra
    )
}

// ── Ölçüm kancaları: hat koruması ACK kaydı, ACK çerçevesi sayaçları, alındı gönderim anı ─────
//
// SALT GÖZLEM: hepsi sondanın (`KamaNode::probe_for_test`) ve birim izinin
// (`KamaNode::take_unit_trace_for_test`) düzeneğin zaten aldığı kopyalarını okur; düğüme yeni komut
// eklemez (tek istisna: bob'un koşu sonundaki son iz okuması), motoru sürmez, hiçbir karar bu
// kayıtları okumaz.

/// Bob'un eşleme kuyruklarının tavanı (SR-AUDIT-001): TEXT sayısı ≤ 3 × `MAX_ITEMS` (ısınma + plan
/// + karşı örnek); taşan kayıt sayılır, eşlenmez.
const RCPT_QUEUE_CAP: usize = 4 * MAX_ITEMS as usize;

/// TEXT alındısının (`RECEIPT`) Transport birimi boyu: kayıt başlığı + gövde (`kama-app` codec).
/// Bob'un gönderdiği birimler arasında bu boydaki tek kayıt türüdür (`FILE_RECEIPT` gövdesi 9 B).
const RECEIPT_UNIT_BYTES: usize = APP_RECORD_HEADER_BYTES + RECEIPT_BODY_BYTES;

/// Düğüm ya da sonda yokken özetin kanca alanları.
const HOOKS_NULL: &str = "\"ack_frames\":null,\"lg\":null,\"ack_tx\":null";

fn opt_i(v: Option<i64>) -> String {
    v.map_or_else(|| "null".to_owned(), |v| v.to_string())
}

/// [ÖLÇÜM-KANCASI · #252 madde 1] Bir hat koruması ACK kaydı → `lg` satırı. `t` = ACK'in motorda
/// işlendiği an (ucun saat tabanı `origin` + `at_ns`; taban yoksa `null`, `at_ns` yine yazılır).
fn lg_line(clock: &Clock, sess: &str, origin: Option<Instant>, r: &LineGuardAckTrace) -> String {
    let at = origin.and_then(|o| o.checked_add(Duration::from_nanos(r.at_ns)));
    let note = r.note.map_or_else(
        || "null".to_owned(),
        |n| {
            format!(
                "{{\"first_seq\":{},\"last_seq\":{},\"rx_span_ns\":{}}}",
                n.first_seq,
                n.last_seq,
                opt_u(n.rx_span_ns)
            )
        },
    );
    let (first_window, first_window_bps) = match r.first_window {
        None => ("null".to_owned(), None),
        Some(FirstWindowOutcome::Sample(bps)) => (js("Sample"), Some(bps)),
        Some(FirstWindowOutcome::Verified) => (js("Verified"), None),
        Some(FirstWindowOutcome::NoInfo) => (js("NoInfo"), None),
    };
    format!(
        "{{\"k\":\"lg\",\"t\":{},\"sess\":{},\"index\":{},\"at_ns\":{},\"largest_seq\":{},\
         \"decision\":{},\"measured\":{},\"g_ns\":{},\"note\":{note},\"pace_before\":{},\
         \"pace_after\":{},\"rate_before\":{},\"rate_after\":{},\"first_window\":{first_window},\
         \"first_window_bps\":{},\"raised\":{},\"probe_failed\":{}}}",
        opt_ms(clock, at),
        js(sess),
        r.index,
        r.at_ns,
        r.largest_seq,
        js(&format!("{:?}", r.decision)),
        r.measured,
        opt_i(r.g_ns),
        r.pace_before,
        r.pace_after,
        opt_u(r.rate_before),
        opt_u(r.rate_after),
        opt_u(first_window_bps),
        r.raised,
        r.probe_failed
    )
}

/// [ÖLÇÜM-KANCASI · #252 madde 1] Canlı aynanın hat koruması halkasından YENİ kayıtları `lg`
/// satırına döker. Oturum başına iki sayı tutar: aynanın halka sayacı (`DiagRing::total`; yeni
/// kayıt sayısını verir, motor kaydı yeniden kurulsa da şaşmaz) ve yazılan son kayıt sırası
/// (`LineGuardAckTrace::index`; her kayıt bir kez yazılır, sıradaki boşluk `lg_gap` satırıdır).
/// Canlı ayna kapalıyken halka boştur: satır çıkmaz.
#[derive(Default)]
struct LgTap {
    /// Oturum → (görülen halka sayacı, yazılan son kayıt sırası). Yalnız sondadaki oturumlar tutulur.
    seen: BTreeMap<String, (u64, u64)>,
    lines: u64,
    gap_lines: u64,
    /// `lg_gap` satırlarındaki atlanan kayıt toplamı.
    skipped: u64,
    /// Kayıt sırasının geri sardığı (motor kaydının yeniden kurulduğu) durum sayısı.
    resets: u64,
    /// [ÖLÇÜM-KANCASI] Giden ACK kaydı dökümü (`ack` satırları); aynı sondadan, aynı çağrıda.
    ack_tx: AckTxTap,
}

impl LgTap {
    fn drain(&mut self, clock: &Clock, at: Instant, probe: &NodeProbe, out: &mut Out) {
        self.ack_tx.drain(clock, at, &probe.rx, out);
        let keys: Vec<String> = probe
            .sessions
            .iter()
            .map(|sp| format!("{:?}", sp.session))
            .collect();
        self.seen.retain(|k, _| keys.contains(k));
        for (sp, key) in probe.sessions.iter().zip(keys) {
            let ring = &sp.diag.line_guard;
            let total = ring.total();
            let (seen_total, mut last) = self.seen.get(&key).copied().unwrap_or((0, 0));
            if total == seen_total {
                continue;
            }
            // Sayaç tek bir uçta azalmaz; azalmış görünürse halkadaki her kayıt yeni sayılır.
            let fresh = total.checked_sub(seen_total).unwrap_or(total);
            let held = ring.recent().count();
            let take = usize::try_from(fresh).unwrap_or(usize::MAX).min(held);
            for rec in ring.recent().skip(held - take) {
                let reset = rec.index <= last;
                let base = if reset { 0 } else { last };
                let skipped = rec.index.saturating_sub(base).saturating_sub(1);
                if reset || skipped > 0 {
                    out.line(&format!(
                        "{{\"k\":\"lg_gap\",\"t\":{:.3},\"sess\":{},\"after_index\":{base},\
                         \"next_index\":{},\"skipped\":{skipped},\"reset\":{reset}}}",
                        clock.ms(at),
                        js(&key),
                        rec.index
                    ));
                    self.gap_lines += 1;
                    self.skipped = self.skipped.saturating_add(skipped);
                    self.resets += u64::from(reset);
                }
                out.line(&lg_line(clock, &key, sp.diag.clock_origin, rec));
                self.lines += 1;
                last = rec.index;
            }
            self.seen.insert(key, (total, last));
        }
    }
}

/// Motor saatindeki an (`origin` + `ns`) → Unix ms; taban ya da an yoksa `null`.
fn at_ms(clock: &Clock, origin: Option<Instant>, ns: Option<u64>) -> String {
    opt_ms(
        clock,
        origin
            .zip(ns)
            .and_then(|(o, ns)| o.checked_add(Duration::from_nanos(ns))),
    )
}

/// ACK çerçevesi türünün özet sırası: `0x10`, `0x14`, `0x15`, `0x16`, `0x17`, öbürü.
const ACK_TYPES: [u8; 5] = [0x10, 0x14, 0x15, 0x16, 0x17];

/// [ÖLÇÜM-KANCASI] Bir giden ACK kaydı → `ack` satırı (alan tanımları modül belgesinde). Anlar
/// sunucu ucunun saat tabanından (`origin`) Unix ms'ye çevrilir; hiçbir alan türetilmiş tahmin
/// değildir, motorun yazmadığı alan `null`dır.
fn ack_line(clock: &Clock, origin: Option<Instant>, r: &RxAckDiag) -> String {
    let ms = |ns: Option<u64>| at_ms(clock, origin, ns);
    let sess = r
        .session
        .map_or_else(|| "null".to_owned(), |h| js(&format!("{h:?}")));
    let wire = match r.wire {
        RxAckWire::Sent => "sent",
        RxAckWire::EgressRefused => "egress_refused",
        RxAckWire::SealFailed => "seal_failed",
    };
    let t_wire = ms((r.wire == RxAckWire::Sent).then_some(r.wire_ns));
    let c = r.totals;
    let cum = format!(
        "{{\"drop_detects\":{},\"drop_alarms\":{},\"rise_alarms\":{},\"wait_planned\":{},\
         \"budget\":{},\"yield\":{},\"no_info\":{}}}",
        c.drop_detects,
        c.drop_alarms,
        c.rise_alarms,
        c.wait_planned,
        c.budget_deferred,
        c.yield_deferred,
        c.no_info_deferred
    );
    let motor = match r.motor {
        Some(m) => {
            let ready = if m.alarm {
                m.alarm_ns
            } else {
                Some(m.first_try_ns)
            };
            format!(
                "\"t\":{},\"motor\":true,\"motor_ack\":{},\"t_evidence\":{},\"t_ready\":{},\
                 \"t_wire\":{t_wire},\"t_detect\":{},\"t_alarm\":{},\"t_first_try\":{},\
                 \"t_first_defer\":{},\"alarm\":{},\"rise\":{},\"wait_open\":{},\"detects\":{},\
                 \"defer\":{{\"budget\":{},\"yield\":{},\"no_info\":{},\"first_window\":{}}},\
                 \"defer_alarm\":{{\"budget\":{},\"yield\":{}}}",
                ms(Some(m.emit_ns)),
                m.index,
                ms(m.evidence_ns),
                ms(ready),
                ms(m.detect_ns),
                ms(m.alarm_ns),
                ms(Some(m.first_try_ns)),
                ms(m.first_defer_ns),
                m.alarm,
                m.rise,
                m.wait_open,
                m.detects,
                m.defer_budget,
                m.defer_yield,
                m.defer_no_info,
                m.defer_first_window,
                m.alarm_defer_budget,
                m.alarm_defer_yield
            )
        }
        None => format!(
            "\"t\":{},\"motor\":false,\"motor_ack\":null,\"t_evidence\":null,\"t_ready\":null,\
             \"t_wire\":{t_wire},\"t_detect\":null,\"t_alarm\":null,\"t_first_try\":null,\
             \"t_first_defer\":null,\"alarm\":null,\"rise\":null,\"wait_open\":null,\
             \"detects\":null,\"defer\":null,\"defer_alarm\":null",
            ms(Some(r.motor_ns))
        ),
    };
    format!(
        "{{\"k\":\"ack\",{motor},\"sess\":{sess},\"index\":{},\"type\":\"0x{:02x}\",\"bytes\":{},\
         \"timer\":{},\"wire\":{},\"at_ns\":{},\"wire_ns\":{},\"cum\":{cum},\"sampled\":false}}",
        r.index,
        r.frame_type,
        r.frame_bytes,
        r.timer,
        js(wire),
        r.motor_ns,
        r.wire_ns
    )
}

/// [ÖLÇÜM-KANCASI] Sunucu ucunun canlı aynasındaki giden ACK halkasından YENİ kayıtları `ack`
/// satırına döker ([`LgTap`] ile aynı yöntem: halka sayacı yeni kayıt sayısını, kayıt sırası
/// boşluğu verir). Halkadan taşan kayıtlar `ack_gap` satırıyla sayılır. Canlı ayna kapalıyken
/// halka boştur: satır çıkmaz.
#[derive(Default)]
struct AckTxTap {
    /// Görülen halka sayacı ve yazılan son kayıt sırası.
    seen_total: u64,
    last: u64,
    lines: u64,
    gap_lines: u64,
    skipped: u64,
    /// Yazılan satırlar: türe göre ([`ACK_TYPES`] sırası + öbürü), motor kaydı olan, alarm
    /// kuralıyla çıkan, kanıt anı dolu, tele çıkan (`wire: sent`).
    types: [u64; 6],
    motor: u64,
    alarm: u64,
    evidence: u64,
    wire_sent: u64,
}

impl AckTxTap {
    fn drain(&mut self, clock: &Clock, at: Instant, rx: &ServerLiveDiag, out: &mut Out) {
        let ring = &rx.rx_acks;
        let total = ring.total();
        if total == self.seen_total {
            return;
        }
        let fresh = total.checked_sub(self.seen_total).unwrap_or(total);
        let held = ring.recent().count();
        let take = usize::try_from(fresh).unwrap_or(usize::MAX).min(held);
        for rec in ring.recent().skip(held - take) {
            let skipped = rec.index.saturating_sub(self.last).saturating_sub(1);
            if skipped > 0 {
                out.line(&format!(
                    "{{\"k\":\"ack_gap\",\"t\":{:.3},\"after_index\":{},\"next_index\":{},\
                     \"skipped\":{skipped}}}",
                    clock.ms(at),
                    self.last,
                    rec.index
                ));
                self.gap_lines += 1;
                self.skipped = self.skipped.saturating_add(skipped);
            }
            out.line(&ack_line(clock, rx.clock_origin, rec));
            self.lines += 1;
            let slot = ACK_TYPES
                .iter()
                .position(|&t| t == rec.frame_type)
                .unwrap_or(ACK_TYPES.len());
            self.types[slot] += 1;
            if let Some(m) = rec.motor {
                self.motor += 1;
                self.alarm += u64::from(m.alarm);
                self.evidence += u64::from(m.evidence_ns.is_some());
            }
            self.wire_sent += u64::from(rec.wire == RxAckWire::Sent);
            self.last = rec.index;
        }
        self.seen_total = total;
    }

    /// Özetin `ack_tx` alanı. `ring_total` = aynanın halkasına giren kayıt sayısı (`lines +
    /// skipped` ile karşılaştırılır).
    fn json(&self) -> String {
        let t = self.types;
        format!(
            "{{\"lines\":{},\"gap_lines\":{},\"skipped\":{},\"ring_total\":{},\"types\":{{\
             \"0x10\":{},\"0x14\":{},\"0x15\":{},\"0x16\":{},\"0x17\":{},\"other\":{}}},\
             \"motor\":{},\"alarm\":{},\"t_evidence_set\":{},\"wire_sent\":{}}}",
            self.lines,
            self.gap_lines,
            self.skipped,
            self.seen_total,
            t[0],
            t[1],
            t[2],
            t[3],
            t[4],
            t[5],
            self.motor,
            self.alarm,
            self.evidence,
            self.wire_sent
        )
    }
}

/// [ÖLÇÜM-KANCASI] Özet alanları: canlı aynanın ACK çerçevesi sayaçları (`ClientLiveDiag::ack`;
/// açık oturumlar + kapanmış oturumlar toplamı) ve `lg` satırı sayıları. ACK TÜRLERİ burada
/// yinelenmez: `wire.ack_rx` ([`wire_json`]). `lg.ring_total` = açık oturumların ayna halkasına
/// giren kayıt sayısı (`lines + skipped` ile karşılaştırılır). Canlı ayna kapalıyken hepsi 0.
/// Özetin sondası son `lg` ve `ack` kayıtlarını da döker (son örneklemeden sonra işlenen /
/// çıkan ACK'ler). `ack_tx` = yazılan `ack` satırlarının sayıları ([`AckTxTap::json`]).
fn hooks_json(node: &KamaNode, clock: &Clock, lg: &mut LgTap, out: &mut Out) -> String {
    let Ok(probe) = node.probe_for_test() else {
        return HOOKS_NULL.to_owned();
    };
    lg.drain(clock, Instant::now(), &probe, out);
    let mut ack = [
        probe.retired.ack.frames,
        probe.retired.ack.mapped,
        probe.retired.ack.new_units,
        probe.retired.ack.inflight_shrunk,
    ];
    let mut ring_total = 0u64;
    for sp in &probe.sessions {
        let a = sp.diag.ack;
        for (sum, v) in ack
            .iter_mut()
            .zip([a.frames, a.mapped, a.new_units, a.inflight_shrunk])
        {
            *sum = sum.saturating_add(v);
        }
        ring_total = ring_total.saturating_add(sp.diag.line_guard.total());
    }
    format!(
        "\"ack_frames\":{{\"frames\":{},\"mapped\":{},\"new_units\":{},\"inflight_shrunk\":{}}},\
         \"lg\":{{\"lines\":{},\"gap_lines\":{},\"skipped\":{},\"resets\":{},\"ring_total\":{}}},\
         \"ack_tx\":{}",
        ack[0],
        ack[1],
        ack[2],
        ack[3],
        lg.lines,
        lg.gap_lines,
        lg.skipped,
        lg.resets,
        ring_total,
        lg.ack_tx.json()
    )
}

/// [ÖLÇÜM-KANCASI · #252 madde 4] Bob'un TEXT alındılarının (RECEIPT) Transport'a kabul anı.
///
/// Kaynak birim izidir (`UnitTraceKind::Sent`, kaydın anı `Instant`): `Messenger` alındıyı TEXT'i
/// işlerken, `TextReceived` olayını üretmeden ÖNCE Transport'a verir (`kama-app` `messaging.rs`
/// `on_text` → `send_receipt`); düğüm olayı aynı turda dinleyiciye iletir. İz kaydı `msg_id`
/// taşımaz; eşleme sıra ve zamanladır: bir TEXT olayının alındısı, önceki TEXT olayından sonra ve
/// bu olayın dinleyici anından önce kabul edilen SON alındı boyundaki `S` kaydıdır. Aynı aralıktaki
/// daha eski kayıtlar olay üretmeyen (tekrar / pencere altı) TEXT'lerin alındılarıdır: `mid`'leri
/// düzenekten bilinemez (`mid:null`). Olayı olup `S` kaydı olmayan TEXT'in alındısı Transport'a
/// verilememiştir (`t:null`; `MessengerStats::receipts_unsent`).
///
/// TELE ÇIKIŞ (`rcpt_wire`, en iyi çaba): kabul anı tele çıkış anı değildir (birim motor kuyruğunda
/// ve hız sınırında bekler). Birim başına ilk iletim anını veren bir ürün erişicisi yoktur; var olan
/// tek kaynak canlı aynanın kurtarma görüntüsüdür (`SenderRecoveryDiag::oldest_unacked`, ≈ 200 ms'de
/// bir, yalnız EN ESKİ teyitsiz birim): motorun o birim için tuttuğu en eski kaydın gönderim anı.
/// Alındı birimi bu görüntüde ilk görüldüğünde bir `rcpt_wire` satırı yazılır; `exact` = o anda
/// tek emisyon kararı vardı (kayıt orijinal iletimdir). Birden çok kararda değer üst sınırdır
/// (orijinalin kaydı düşmüş olabilir). Görüntüye hiç girmeden teyit edilen alındı için satır yoktur.
#[derive(Default)]
struct RcptTap {
    /// TEXT olayları, dinleyici anı sırasıyla: (`msg_id`, sınıf, sıra, an).
    texts: VecDeque<(u32, char, u32, Instant)>,
    /// Alındı boyundaki `S` kayıtları, kabul anı sırasıyla.
    sends: VecDeque<UnitTraceRecord>,
    matched: u64,
    /// Olayı olan ama alındı gönderim kaydı bulunmayan TEXT.
    no_send: u64,
    /// TEXT olayına bağlanamayan alındı gönderimi.
    unattributed: u64,
    /// Birim izinin tavan dolu olduğu için düşürdüğü kayıt (eşleme eksik kalabilir).
    trace_dropped: u64,
    /// Kuyruk tavanı yüzünden eşlemeye girmeyen kayıt.
    overflow: u64,
    /// Tele çıkışı henüz görülmemiş alındı gönderimleri (kabul sırasıyla) ve biliniyorsa `msg_id`.
    unwired: VecDeque<(UnitTraceRecord, Option<u32>)>,
    /// `rcpt_wire` satırı yazılan / görüntüye girmeden geçilen alındı.
    wire_seen: u64,
    wire_unseen: u64,
}

/// [ÖLÇÜM-KANCASI] Sondadaki oturumların en eski teyitsiz biriminin motor kaydı: (oturum sırası,
/// birim, emisyon kararı sayısı, en eski kaydın gönderim anı, görüntünün anı).
type OldestEmission = (usize, u64, u32, Instant, Instant);

fn oldest_emissions(probe: &NodeProbe) -> Vec<OldestEmission> {
    probe
        .sessions
        .iter()
        .enumerate()
        .filter_map(|(i, sp)| {
            let rec = sp.diag.recovery?;
            let oldest = rec.oldest_unacked?;
            let since = Duration::from_nanos(oldest.since_earliest_record_ns?);
            let first = rec.taken_at.checked_sub(since)?;
            Some((
                i,
                oldest.unit_id,
                oldest.emission_decisions,
                first,
                rec.taken_at,
            ))
        })
        .collect()
}

impl RcptTap {
    /// Sondanın en eski teyitsiz birim gözlemlerinden alındı birimlerinin `rcpt_wire` satırları.
    fn note_oldest(
        &mut self,
        clock: &Clock,
        probe: &NodeProbe,
        seen: &[OldestEmission],
        out: &mut Out,
    ) {
        for &(i, unit, emissions, first, taken_at) in seen {
            let Some(sp) = probe.sessions.get(i) else {
                continue;
            };
            let Some(pos) = self
                .unwired
                .iter()
                .position(|(s, _)| s.session == sp.session && s.unit == unit)
            else {
                continue;
            };
            let Some((send, mid)) = self.unwired.remove(pos) else {
                continue;
            };
            // Bu oturumun daha eski alındıları görüntüye girmeden teyit edildi.
            let mut k = 0;
            let mut older = 0u64;
            self.unwired.retain(|(s, _)| {
                let passed = k < pos && s.session == sp.session;
                k += 1;
                older += u64::from(passed);
                !passed
            });
            self.wire_unseen = self.wire_unseen.saturating_add(older);
            out.line(&format!(
                "{{\"k\":\"rcpt_wire\",\"t\":{:.3},\"mid\":{},\"unit\":{unit},\"sess\":{},\
                 \"emissions\":{emissions},\"exact\":{},\"accept_t\":{:.3},\"seen_t\":{:.3}}}",
                clock.ms(first),
                opt_u(mid.map(u64::from)),
                js(&format!("{:?}", send.session)),
                emissions == 1,
                clock.ms(send.at),
                clock.ms(taken_at)
            ));
            self.wire_seen += 1;
        }
    }

    fn note_text(&mut self, msg_id: u32, class: char, idx: u32, at: Instant) {
        if self.texts.len() >= RCPT_QUEUE_CAP {
            self.overflow += 1;
            return;
        }
        self.texts.push_back((msg_id, class, idx, at));
    }

    fn note_trace(&mut self, records: &[UnitTraceRecord], dropped: u64) {
        self.trace_dropped = self.trace_dropped.saturating_add(dropped);
        for r in records {
            let receipt = r.kind == UnitTraceKind::Sent
                && usize::try_from(r.len).is_ok_and(|len| len == RECEIPT_UNIT_BYTES);
            if !receipt {
                continue;
            }
            if self.sends.len() >= RCPT_QUEUE_CAP || self.unwired.len() >= RCPT_QUEUE_CAP {
                self.overflow += 1;
                continue;
            }
            self.sends.push_back(*r);
            self.unwired.push_back((*r, None));
        }
    }

    fn line(
        clock: &Clock,
        text: Option<(u32, char, u32, Instant)>,
        send: Option<&UnitTraceRecord>,
    ) -> String {
        let (mid, class, idx, rx_t) = match text {
            Some((mid, class, idx, at)) => (
                mid.to_string(),
                js(&class.to_string()),
                idx.to_string(),
                Some(at),
            ),
            None => (
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                None,
            ),
        };
        let lag_us = match (rx_t, send) {
            (Some(rx), Some(s)) => {
                opt_u(u64::try_from(rx.saturating_duration_since(s.at).as_micros()).ok())
            }
            _ => "null".to_owned(),
        };
        format!(
            "{{\"k\":\"rcpt_tx\",\"t\":{},\"mid\":{mid},\"c\":{class},\"i\":{idx},\"rx_t\":{},\
             \"lag_us\":{lag_us},\"unit\":{},\"len\":{},\"sess\":{}}}",
            opt_ms(clock, send.map(|s| s.at)),
            opt_ms(clock, rx_t),
            opt_u(send.map(|s| s.unit)),
            opt_u(send.map(|s| u64::from(s.len))),
            send.map_or_else(|| "null".to_owned(), |s| js(&format!("{:?}", s.session)))
        )
    }

    /// Eşler ve yazar. `cut` = son iz okumasından hemen önceki an: dinleyici anı bundan sonra olan
    /// TEXT'in alındı kaydı henüz okunmamış olabilir, sonraki tura kalır. `None` = koşu sonu:
    /// kalan her kayıt yazılır.
    fn resolve(&mut self, clock: &Clock, cut: Option<Instant>, out: &mut Out) {
        while let Some(&text) = self.texts.front() {
            let rx_at = text.3;
            if cut.is_some_and(|c| rx_at > c) {
                break;
            }
            self.texts.pop_front();
            let mut own: Option<UnitTraceRecord> = None;
            while self.sends.front().is_some_and(|s| s.at <= rx_at) {
                if let Some(older) = own.take() {
                    out.line(&Self::line(clock, None, Some(&older)));
                    self.unattributed += 1;
                }
                own = self.sends.pop_front();
            }
            if let Some(own) = own.as_ref() {
                self.matched += 1;
                if let Some(entry) = self
                    .unwired
                    .iter_mut()
                    .find(|(s, _)| s.session == own.session && s.unit == own.unit)
                {
                    entry.1 = Some(text.0);
                }
            } else {
                self.no_send += 1;
            }
            out.line(&Self::line(clock, Some(text), own.as_ref()));
        }
        if cut.is_none() {
            while let Some(send) = self.sends.pop_front() {
                out.line(&Self::line(clock, None, Some(&send)));
                self.unattributed += 1;
            }
            let left = u64::try_from(self.unwired.len()).unwrap_or(u64::MAX);
            self.wire_unseen = self.wire_unseen.saturating_add(left);
            self.unwired.clear();
        }
    }

    fn json(&self) -> String {
        format!(
            "\"rcpt_tx\":{{\"matched\":{},\"no_send\":{},\"unattributed\":{},\"trace_dropped\":{},\
             \"overflow\":{},\"wire_seen\":{},\"wire_unseen\":{}}}",
            self.matched,
            self.no_send,
            self.unattributed,
            self.trace_dropped,
            self.overflow,
            self.wire_seen,
            self.wire_unseen
        )
    }
}

// ── Ortak ────────────────────────────────────────────────────────────────────────────────────

fn device_path(name: &str) -> Result<PathBuf, String> {
    let device = DeviceName::new(name).map_err(|e| format!("cihaz adı {name}: {e:?}"))?;
    Ok(Path::new(PROV_DIR).join("dev").join(device.file_name()))
}

/// bob: provisioning (alice + bob, direct-mode adresleri) → `prov/dev/`; otorite dizini (root.json)
/// hemen silinir — cihazlar yalnız kendi dosyasını okur (D9).
fn provision(cfg: &Config) -> Result<(), String> {
    let authority = Path::new(PROV_DIR).join("authority");
    let dev = Path::new(PROV_DIR).join("dev");
    let _ = fs::remove_dir_all(PROV_DIR);
    fs::create_dir_all(&dev).map_err(|e| format!("{dev:?}: {e}"))?;
    let names = [
        DeviceName::new(ALICE).map_err(|e| format!("{e:?}"))?,
        DeviceName::new(BOB).map_err(|e| format!("{e:?}"))?,
    ];
    let mut direct = BTreeMap::new();
    direct.insert(names[0].clone(), cfg.alice_addr);
    direct.insert(names[1].clone(), cfg.bob_addr);
    let request = InitRequest {
        out_dir: authority.clone(),
        devices: names.to_vec(),
        hub: None,
        direct,
    };
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("sistem saati: {e}"))?
        .as_secs();
    init(&request, &mut OsEntropy::new(), created_at)
        .map_err(|e| format!("provisioning: {e:?}"))?;
    for name in &names {
        let from = authority.join(name.file_name());
        let to = dev.join(name.file_name());
        fs::copy(&from, &to).map_err(|e| format!("{from:?} → {to:?}: {e}"))?;
    }
    let _ = fs::remove_dir_all(&authority);
    write_marker(PROV_READY, "ok")
}

fn start_node(cfg: &Config, me: &str, tap: Tap, link: SockLink) -> Result<KamaNode, String> {
    let config = device_path(me)?;
    let data = PathBuf::from(format!("node-{me}"));
    let node = KamaNode::start_with_link_for_test(
        &config,
        &data,
        Box::new(tap),
        Box::new(link),
        cfg.live_diag,
        true,
    )
    .map_err(|e| format!("{me} başlatılamadı: {e}"))?;
    // Anahtarlar bellekte; cihaz dosyası diskte kalmaz.
    let _ = fs::remove_file(&config);
    Ok(node)
}

fn header_line(cfg: &Config, clock: &Clock) -> String {
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
         \"connect_timeout_s\":{},\"probe_ms\":{},\"live_diag\":{},\"receipt_grace_s\":{}}}}}",
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
        cfg.receipt_grace.as_secs()
    )
}

fn is_text_backpressure(error: &NodeError) -> bool {
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

fn is_sig_backpressure(error: &NodeError) -> bool {
    matches!(
        error,
        NodeError::Busy
            | NodeError::Signaling(SignalingError::Transport(TransportError::Backpressure))
    )
}

fn counts_text(counts: &BTreeMap<char, u32>) -> String {
    let mut s = String::new();
    for (k, v) in counts {
        let _ = writeln!(s, "{k} {v}");
    }
    s
}

fn counts_json(counts: &BTreeMap<char, u32>) -> String {
    let fields: Vec<String> = counts
        .iter()
        .map(|(k, v)| format!("{}:{v}", js(&k.to_string())))
        .collect();
    format!("{{{}}}", fields.join(","))
}

// ── alice (istemci, gönderen) ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    Text { class: char, idx: u32 },
    Sig { idx: u32 },
    PttOpen { spurt: u32 },
    PttPush { spurt: u32, idx: u32 },
    PttClose { spurt: u32 },
    File,
}

#[derive(Debug, Clone, Copy)]
struct Action {
    /// t0'a göre vade.
    due: Duration,
    act: Act,
}

/// Planın demet aralığı: çağıranın süresi (`--ptt-bundle-ms`). Pilot kolunda basış açılınca
/// düğümün seçtiği süreyle yeniden zamanlanır ([`Alice::adopt_press`]).
fn spurt_step(cfg: &Config) -> Duration {
    Duration::from_millis(u64::from(cfg.ptt_bundle_ms))
}

fn push_spurt(plan: &mut Vec<Action>, cfg: &Config, spurt: u32, start: Duration, bundles: u32) {
    let step = spurt_step(cfg);
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

fn build_plan(cfg: &Config) -> Vec<Action> {
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

#[derive(Default)]
struct FileLog {
    handle: Option<FileHandle>,
    sent_at: Option<Instant>,
    stage_sent_at: Option<Instant>,
    delivered_at: Option<Instant>,
    failed_at: Option<Instant>,
    failed_reason: Option<String>,
}

struct Alice {
    cfg: Config,
    clock: Clock,
    out: Out,
    rx: Receiver<Stamped>,
    dropped: Arc<AtomicU64>,
    link: Arc<LinkCounters>,
    node: Option<KamaNode>,
    t0: Option<Instant>,
    connected_at: Option<Instant>,
    plan: Vec<Action>,
    next: usize,
    /// Bu eylem için geri basınç denemeleri (başarıda sıfırlanır).
    tries: u32,
    retries: u64,
    streams: BTreeMap<u32, StreamHandle>,
    /// [PTT-BİRLEŞİK · pilot] Pilot API'siyle açılan basışlar: düğümün seçtiği demet süresi ve
    /// o süreye ölçeklenmiş demet yükü (≤ basış sayısı).
    pressed: BTreeMap<u32, (Duration, usize, u64)>,
    /// [PTT-BİRLEŞİK · pilot] Basış başına düğümün seçimi (özet kaydı için).
    chosen: Vec<String>,
    file: FileLog,
    sent: BTreeMap<char, u32>,
    msg_items: BTreeMap<u32, (char, u32)>,
    receipts: BTreeSet<u32>,
    karsi_at: Option<Instant>,
    bob_counts: BTreeMap<char, u32>,
    bob_file: String,
    bob_complete_at: Option<Instant>,
    next_probe: Instant,
    probes: u64,
    probe_errors: u64,
    last_probe: Option<NodeProbe>,
    notes: Vec<String>,
    /// [ÖLÇÜM-KANCASI] Hat koruması ACK kaydı dökümü (`lg` satırları).
    lg: LgTap,
}

impl Alice {
    fn node(&self) -> Result<&KamaNode, String> {
        self.node
            .as_ref()
            .ok_or_else(|| "alice düğümü yok".to_owned())
    }

    fn t0(&self) -> Instant {
        self.t0.unwrap_or(self.clock.inst0)
    }

    fn bump(&mut self, class: char) {
        *self.sent.entry(class).or_insert(0) += 1;
    }

    /// [PTT-BİRLEŞİK · pilot] Düğümün basış için seçtiği biçimi benimser: bu basışın henüz
    /// işlenmemiş demet ve kapanış eylemleri seçilen süreyle yeniden zamanlanır, yük aynı ses
    /// hızıyla ölçeklenir. Döner: `send` kaydına eklenecek alanlar.
    fn adopt_press(&mut self, open: Action, spurt: u32, press: kama_node::PttPress) -> String {
        let ms = kama_app::codec::bundle_ms_from_code(press.bundle_ms);
        let step = Duration::from_millis(u64::from(ms));
        let caller = usize::from(self.cfg.ptt_bundle_ms).max(1);
        let bytes = (self.cfg.ptt_bytes * usize::from(ms) / caller).max(4);
        // Basış başı: açılış eyleminin vadesi (ilk 20 ms çerçevenin oluşumu), Unix µs.
        let press_us = self.clock.unix_ns(self.t0() + open.due) / 1_000;
        self.pressed.insert(spurt, (step, bytes, press_us));
        self.chosen.push(format!(
            "{{\"sp\":{spurt},\"bundle_ms\":{ms},\"coalesced\":{},\"caller_ms\":{}}}",
            press.coalesced, self.cfg.ptt_bundle_ms
        ));
        let bundles = if spurt == KARSI_SPURT {
            self.cfg.karsi_bundles
        } else {
            self.cfg.ptt_bundles
        };
        let from = (self.next + 1).min(self.plan.len());
        for action in &mut self.plan[from..] {
            match action.act {
                Act::PttPush { spurt: s, idx } if s == spurt => {
                    action.due = open.due + step.saturating_mul(idx + 1);
                }
                Act::PttClose { spurt: s } if s == spurt => {
                    action.due = open.due + step.saturating_mul(bundles + 1);
                }
                _ => {}
            }
        }
        self.plan[from..].sort_by_key(|a| a.due);
        format!(
            ",\"bundle_ms\":{ms},\"bundle_code\":{},\"coalesced\":{},\"caller_ms\":{},\"bundle_bytes\":{bytes}",
            press.bundle_ms, press.coalesced, self.cfg.ptt_bundle_ms
        )
    }

    fn send_line(&mut self, a: Action, t_call: Instant, extra: &str) {
        let (class, idx, spurt) = match a.act {
            Act::Text { class, idx } => (class.to_string(), idx, None),
            Act::Sig { idx } => ("S".to_owned(), idx, None),
            Act::PttOpen { spurt } => ("O".to_owned(), 0, Some(spurt)),
            Act::PttPush { spurt, idx } => ("P".to_owned(), idx, Some(spurt)),
            Act::PttClose { spurt } => ("C".to_owned(), 0, Some(spurt)),
            Act::File => ("F".to_owned(), 0, None),
        };
        let t_ret = Instant::now();
        let line = format!(
            "{{\"k\":\"send\",\"c\":{},\"i\":{},\"sp\":{},\"due\":{:.3},\"t\":{:.3},\"tr\":{:.3},\
             \"try\":{}{}}}",
            js(&class),
            idx,
            opt_u(spurt.map(u64::from)),
            self.clock.ms(self.t0() + a.due),
            self.clock.ms(t_call),
            self.clock.ms(t_ret),
            self.tries,
            extra
        );
        self.out.line(&line);
    }

    /// `Ok(true)` = yapıldı; `Ok(false)` = geri basınç, yeniden denenecek.
    fn execute(&mut self, a: Action) -> Result<bool, String> {
        let t_call = Instant::now();
        match a.act {
            Act::Text { class, idx } => {
                let len = if class == 'K' {
                    self.cfg.karsi_text_bytes
                } else {
                    self.cfg.text_bytes
                };
                let text = text_body(class, idx, len);
                let result = self.node()?.send_text(BOB, &text);
                match result {
                    Ok(msg_id) => {
                        self.msg_items.insert(msg_id, (class, idx));
                        self.bump(class);
                        self.send_line(a, t_call, &format!(",\"mid\":{msg_id}"));
                        Ok(true)
                    }
                    Err(e) if is_text_backpressure(&e) => Ok(false),
                    Err(e) => Err(format!("send_text {class}{idx}: {e}")),
                }
            }
            Act::Sig { idx } => {
                let body = sig_body(idx, self.cfg.sig_bytes);
                let result = self.node()?.send_signaling(BOB, body);
                match result {
                    Ok(_) => {
                        self.bump('S');
                        self.send_line(a, t_call, "");
                        Ok(true)
                    }
                    Err(e) if is_sig_backpressure(&e) => Ok(false),
                    Err(e) => Err(format!("send_signaling {idx}: {e}")),
                }
            }
            Act::PttOpen { spurt } => {
                let mut chosen = String::new();
                let result = if self.cfg.ptt_pilot {
                    let opened =
                        self.node()?
                            .ptt_open_press(BOB, PttCodec::Lyra, self.cfg.ptt_bundle_ms);
                    opened.map(|press| {
                        chosen = self.adopt_press(a, spurt, press);
                        press.stream
                    })
                } else {
                    self.node()?
                        .ptt_open(BOB, PttCodec::Lyra, self.cfg.ptt_bundle_ms)
                };
                match result {
                    Ok(stream) => {
                        self.streams.insert(spurt, stream);
                        self.bump('O');
                        self.send_line(a, t_call, &chosen);
                        Ok(true)
                    }
                    Err(e) if is_ptt_backpressure(&e) => Ok(false),
                    Err(e) => Err(format!("ptt_open {spurt}: {e}")),
                }
            }
            Act::PttPush { spurt, idx } => {
                let stream = *self
                    .streams
                    .get(&spurt)
                    .ok_or_else(|| format!("ptt_push {spurt}/{idx}: akış açık değil"))?;
                let result = match self.pressed.get(&spurt).copied() {
                    Some((step, bytes, press_us)) => {
                        // Demet çerçeveleri var olunca verilir: ilk çerçeve bir demet süresi
                        // önce oluştu. Basışın son demeti `END`'i taşır.
                        let last = idx + 1
                            == if spurt == KARSI_SPURT {
                                self.cfg.karsi_bundles
                            } else {
                                self.cfg.ptt_bundles
                            };
                        self.node()?.ptt_push_timed(
                            stream,
                            press_body(
                                spurt,
                                idx,
                                bytes,
                                press_us,
                                u16::try_from(step.as_millis()).unwrap_or(u16::MAX),
                                if spurt == KARSI_SPURT {
                                    self.cfg.karsi_bundles
                                } else {
                                    self.cfg.ptt_bundles
                                },
                            ),
                            step,
                            last,
                        )
                    }
                    None => self
                        .node()?
                        .ptt_push(stream, bundle_body(spurt, idx, self.cfg.ptt_bytes)),
                };
                match result {
                    Ok(()) => {
                        self.bump('P');
                        self.send_line(a, t_call, "");
                        Ok(true)
                    }
                    Err(e) if is_ptt_backpressure(&e) => Ok(false),
                    Err(e) => Err(format!("ptt_push {spurt}/{idx}: {e}")),
                }
            }
            Act::PttClose { spurt } => {
                let stream = self
                    .streams
                    .remove(&spurt)
                    .ok_or_else(|| format!("ptt_close {spurt}: akış açık değil"))?;
                // Tutamaç her durumda kapanır; hata yalnız PTT_CLOSE'un verilemediğidir (kayda geçer).
                let result = self.node()?.ptt_close(stream);
                self.bump('C');
                let extra = match &result {
                    Ok(()) => String::new(),
                    Err(e) => format!(",\"err\":{}", js(&e.to_string())),
                };
                self.send_line(a, t_call, &extra);
                Ok(true)
            }
            Act::File => {
                if self.file.handle.is_none() {
                    let bytes: Vec<u8> = (0..self.cfg.file_bytes).map(file_byte).collect();
                    fs::write(SRC_FILE, &bytes).map_err(|e| format!("{SRC_FILE}: {e}"))?;
                }
                let result = self.node()?.send_file(BOB, Path::new(SRC_FILE));
                match result {
                    Ok(handle) => {
                        self.file.handle = Some(handle);
                        self.file.sent_at = Some(t_call);
                        self.bump('F');
                        let extra = format!(",\"bytes\":{}", self.cfg.file_bytes);
                        self.send_line(a, t_call, &extra);
                        Ok(true)
                    }
                    Err(NodeError::Busy) => Ok(false),
                    Err(e) => Err(format!("send_file: {e}")),
                }
            }
        }
    }

    fn on_event(&mut self, stamped: Stamped) {
        let Stamped { at, event } = stamped;
        let t = self.clock.ms(at);
        match event {
            NodeEvent::Connected { peer_id } => {
                if peer_id == BOB {
                    self.connected_at.get_or_insert(at);
                }
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"connected\",\"peer\":{}}}",
                    js(&peer_id)
                ));
            }
            NodeEvent::Closed { peer_id, reason } => {
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"closed\",\"peer\":{},\"why\":{}}}",
                    js(&peer_id),
                    js(&format!("{reason:?}"))
                ));
            }
            NodeEvent::Receipt { msg_id, .. } => {
                let first = self.receipts.insert(msg_id);
                let (class, idx) = self.msg_items.get(&msg_id).copied().unwrap_or(('?', 0));
                self.out.line(&format!(
                    "{{\"k\":\"rcpt\",\"t\":{t:.3},\"mid\":{msg_id},\"c\":{},\"i\":{idx},\
                     \"first\":{first}}}",
                    js(&class.to_string())
                ));
            }
            NodeEvent::FileProgress { file, stage, .. } => {
                if self.file.handle == Some(file) {
                    let name = match stage {
                        FileStage::Sent => {
                            self.file.stage_sent_at.get_or_insert(at);
                            "file_sent"
                        }
                        FileStage::Delivered => {
                            self.file.delivered_at.get_or_insert(at);
                            "file_delivered"
                        }
                    };
                    self.out
                        .line(&format!("{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"{name}\"}}"));
                }
            }
            NodeEvent::FileFailed { file, reason, .. } => {
                if let FileRef::Outgoing(handle) = file {
                    if self.file.handle == Some(handle) && self.file.failed_at.is_none() {
                        self.file.failed_at = Some(at);
                        self.file.failed_reason = Some(format!("{reason:?}"));
                    }
                }
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"file_failed\",\"file\":{},\"why\":{}}}",
                    js(&format!("{file:?}")),
                    js(&format!("{reason:?}"))
                ));
            }
            NodeEvent::ConnectProgress { .. } | NodeEvent::Metrics(_) => {}
            other => {
                // alice'e beklenmeyen gelen kayıt (bob yalnız alındı gönderir).
                let name = match other {
                    NodeEvent::TextReceived { .. } => "text_received",
                    NodeEvent::PttOpened { .. } => "ptt_opened",
                    NodeEvent::PttBundle { .. } => "ptt_bundle",
                    NodeEvent::PttClosed { .. } => "ptt_closed",
                    NodeEvent::SignalingReceived { .. } => "signaling_received",
                    NodeEvent::FileReceived { .. } => "file_received",
                    _ => "other",
                };
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"unexpected\",\"what\":\"{name}\"}}"
                ));
            }
        }
    }

    fn pump_events(&mut self) {
        while let Ok(stamped) = self.rx.try_recv() {
            self.on_event(stamped);
        }
    }

    /// Sonda + birim izi; bob durumu.
    fn sample(&mut self) {
        let Some(node) = self.node.as_ref() else {
            return;
        };
        let probe = node.probe_for_test();
        let at = Instant::now();
        let trace = node.take_unit_trace_for_test();
        match probe {
            Ok(p) => {
                self.out.line(&ts_line(&self.clock, at, &p));
                self.lg.drain(&self.clock, at, &p, &mut self.out);
                self.last_probe = Some(p);
                self.probes += 1;
            }
            Err(_) => self.probe_errors += 1,
        }
        match trace {
            Ok((records, dropped)) => {
                if !records.is_empty() || dropped > 0 {
                    self.out
                        .line(&trace_line(&self.clock, Instant::now(), &records, dropped));
                }
            }
            Err(_) => self.probe_errors += 1,
        }
        self.read_bob_state();
    }

    fn read_bob_state(&mut self) {
        let Some(body) = read_marker(BOB_STATE) else {
            return;
        };
        let mut counts = BTreeMap::new();
        let mut file = String::from("none");
        for line in body.lines() {
            let mut parts = line.split_whitespace();
            let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
                continue;
            };
            if key == "F" {
                value.clone_into(&mut file);
            } else if let (Some(c), Ok(n)) = (key.chars().next(), value.parse::<u32>()) {
                counts.insert(c, n);
            }
        }
        self.bob_counts = counts;
        self.bob_file = file;
    }

    fn bob_has_everything(&self) -> bool {
        let items = self
            .sent
            .iter()
            .filter(|(c, _)| **c != 'F')
            .all(|(c, n)| self.bob_counts.get(c).copied().unwrap_or(0) >= *n);
        // alice'in gönderen zaman aşımı (`FILE_RECEIPT_TTL`, 60 s; duman-2'de her dosyalı koşuda
        // 65. s'de) bob'un alımını bitirmez: kesin sonuç bob'unkidir (alındı ya da alıcıda hata).
        let file = !self.cfg.scenario.has_file()
            || self.bob_file == "received"
            || self.bob_file == "failed";
        items && file
    }

    /// Karşı örnek tetiği (bkz. modül belgesi): yeni veri kuyruğu boş, eski dosya parçalarının
    /// kurtarması canlı, bob dosyayı henüz bitirmedi. alice'in kendi dosya sonucu ölçüt DEĞİLDİR:
    /// gönderen `FILE_RECEIPT_TTL`'i (60 s) `FILE_END`'in Transport kabulünden sayar ve dosya
    /// hâlâ yoldayken `ReceiptTimeout` verir (duman-2).
    fn maybe_karsi(&mut self) {
        if !self.cfg.karsi
            || !self.cfg.scenario.has_file()
            || self.karsi_at.is_some()
            || self.file.sent_at.is_none()
            || self.bob_file == "received"
            || self.bob_file == "failed"
        {
            return;
        }
        let now = Instant::now();
        if now < self.t0() + self.cfg.file_at + Duration::from_secs(1) {
            return;
        }
        let Some(p) = self.last_probe.as_ref() else {
            return;
        };
        let Some(rec) = p
            .sessions
            .iter()
            .find(|s| s.peer_id.as_deref() == Some(BOB))
            .and_then(|s| s.diag.recovery)
        else {
            return;
        };
        if rec.timers.tx_queue_len != 0 || rec.unacked_units < self.cfg.karsi_min_unacked {
            return;
        }
        self.karsi_at = Some(now);
        let base = now.saturating_duration_since(self.t0());
        let mut added = Vec::new();
        for idx in 0..self.cfg.karsi_texts {
            added.push(Action {
                due: base + self.cfg.karsi_every.saturating_mul(idx),
                act: Act::Text { class: 'K', idx },
            });
        }
        if self.cfg.karsi_bundles > 0 {
            // Eş başına tek açık akış: kalan plandaki basışlarla çakışmayan ilk başlangıç.
            let len = spurt_len(&self.cfg, self.cfg.karsi_bundles);
            let mut start = base + Duration::from_millis(250);
            let mut spans: Vec<(Duration, Duration)> = self
                .cfg
                .ptt_at
                .iter()
                .map(|s| (*s, *s + spurt_len(&self.cfg, self.cfg.ptt_bundles)))
                .collect();
            spans.sort();
            for (s, e) in spans {
                if start < e + SPURT_GAP && start + len + SPURT_GAP > s {
                    start = e + SPURT_GAP;
                }
            }
            push_spurt(
                &mut added,
                &self.cfg,
                KARSI_SPURT,
                start,
                self.cfg.karsi_bundles,
            );
        }
        let s = &p.sessions;
        let detail = s
            .iter()
            .find(|s| s.peer_id.as_deref() == Some(BOB))
            .map(|sp| {
                let mut line = String::new();
                session_json(&mut line, &self.clock, sp);
                line
            })
            .unwrap_or_else(|| "null".to_owned());
        let extra = format!(",\"session\":{detail},\"added\":{}", added.len());
        self.out
            .line(&mark_line(&self.clock, now, "karsi_trigger", &extra));
        self.plan.extend(added);
        self.plan[self.next..].sort_by_key(|a| a.due);
    }

    fn complete(&mut self) -> bool {
        if self.next < self.plan.len() {
            return false;
        }
        if !self.bob_has_everything() {
            return false;
        }
        let now = Instant::now();
        let since = *self.bob_complete_at.get_or_insert(now);
        let texts_sent = self.msg_items.len();
        self.receipts.len() >= texts_sent || now >= since + self.cfg.receipt_grace
    }

    fn plan_end(&self) -> Instant {
        let last = self.plan.last().map_or(Duration::ZERO, |a| a.due);
        self.t0() + last
    }

    /// Bağlantı + plan + tamamlanma. `Ok(true)` = tamamlandı, `Ok(false)` = pencere doldu.
    fn execute_run(&mut self) -> Result<bool, String> {
        wait_marker(MARK_BOB_UP, self.cfg.connect_timeout).ok_or_else(|| {
            format!(
                "{MARK_BOB_UP} {:?} içinde gelmedi",
                self.cfg.connect_timeout
            )
        })?;
        self.node()?
            .connect(BOB)
            .map_err(|e| format!("alice connect: {e}"))?;
        let connect_at = Instant::now();
        self.out
            .line(&mark_line(&self.clock, connect_at, "connect_requested", ""));
        let deadline = connect_at + self.cfg.connect_timeout;
        while self.connected_at.is_none() {
            if Instant::now() >= deadline {
                return Err(format!(
                    "connect {:?} içinde tamamlanmadı",
                    self.cfg.connect_timeout
                ));
            }
            match self.rx.recv_timeout(PUMP_STEP) {
                Ok(stamped) => self.on_event(stamped),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
        }
        let t0 = Instant::now();
        self.t0 = Some(t0);
        write_marker(MARK_T0, &self.clock.unix_ns(t0).to_string())?;
        self.out.line(&mark_line(&self.clock, t0, "t0", ""));
        self.next_probe = t0;
        let mut flush_at = t0;
        loop {
            let now = Instant::now();
            while self.next < self.plan.len() && self.t0() + self.plan[self.next].due <= now {
                let a = self.plan[self.next];
                if self.execute(a)? {
                    self.next += 1;
                    self.tries = 0;
                } else {
                    self.tries += 1;
                    self.retries += 1;
                    break;
                }
            }
            self.pump_events();
            if Instant::now() >= self.next_probe {
                self.sample();
                self.next_probe = Instant::now() + self.cfg.probe_every;
                self.maybe_karsi();
            }
            if Instant::now() >= flush_at {
                self.out.flush();
                flush_at = Instant::now() + Duration::from_secs(1);
            }
            if self.complete() {
                return Ok(true);
            }
            if self.next >= self.plan.len() && Instant::now() >= self.plan_end() + self.cfg.window {
                return Ok(false);
            }
            let wait = if self.tries > 0 {
                RETRY_WAIT
            } else {
                match self.plan.get(self.next) {
                    Some(a) => (self.t0() + a.due)
                        .saturating_duration_since(Instant::now())
                        .min(PUMP_STEP),
                    None => PUMP_STEP,
                }
            };
            match self.rx.recv_timeout(wait) {
                Ok(stamped) => self.on_event(stamped),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
        }
    }

    fn finish(&mut self, result: Result<bool, String>) -> (String, bool) {
        let ended_at = Instant::now();
        self.pump_events();
        self.sample();
        let _ = write_marker(MARK_ALICE_DONE, "done");
        // bob son sondasını alıp çıkana dek bekle (olaylar işlenmeye devam eder).
        let deadline = Instant::now() + BOB_EXIT_WAIT;
        let mut bob_exit = false;
        while Instant::now() < deadline {
            if read_marker(MARK_BOB_EXIT).is_some() {
                bob_exit = true;
                break;
            }
            match self.rx.recv_timeout(MARK_POLL) {
                Ok(stamped) => self.on_event(stamped),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
        }
        self.pump_events();
        self.read_bob_state();
        let (outcome, error, ok) = match &result {
            Ok(true) => ("complete", "null".to_owned(), true),
            Ok(false) => ("incomplete", "null".to_owned(), true),
            Err(e) => ("error", js(e), false),
        };
        let wire = self
            .node
            .as_ref()
            .map_or_else(|| "null".to_owned(), |n| wire_json(n, self.cfg.live_diag));
        let hooks = match self.node.as_ref() {
            Some(n) => hooks_json(n, &self.clock, &mut self.lg, &mut self.out),
            None => HOOKS_NULL.to_owned(),
        };
        let mut shutdown_note = "null".to_owned();
        if let Some(node) = self.node.take() {
            if let Err(e) = node.shutdown() {
                shutdown_note = js(&e.to_string());
            }
        }
        let _ = fs::remove_file(SRC_FILE);
        let _ = fs::remove_dir_all(format!("node-{ALICE}"));
        let notes: Vec<String> = self.notes.iter().map(|n| js(n.as_str())).collect();
        let summary = format!(
            "{{\"schema\":{},\"role\":\"alice\",\"scenario\":{},\"profile\":{},\"rep\":{},\
             \"result\":{},\"error\":{},\"t0\":{},\"connected\":{},\"ended\":{:.3},\
             \"plan_actions\":{},\"plan_done\":{},\"retries\":{},\"sent\":{},\"receipts\":{},\
             \"texts_sent\":{},\"bob_counts\":{},\"bob_file\":{},\"file\":{{\"sent\":{},\
             \"stage_sent\":{},\"delivered\":{},\"failed\":{},\"why\":{}}},\"karsi\":{},\
             \"probes\":{},\"probe_errors\":{},\"listener_drops\":{},\"link\":{},\"bob_exit\":{},\
             \"shutdown_error\":{},\"jsonl_lines\":{},\"jsonl_errors\":{},\"notes\":[{}]}}",
            js(SCHEMA),
            js(self.cfg.scenario.name()),
            js(&self.cfg.profile),
            self.cfg.rep,
            js(outcome),
            error,
            opt_ms(&self.clock, self.t0),
            opt_ms(&self.clock, self.connected_at),
            self.clock.ms(ended_at),
            self.plan.len(),
            self.next,
            self.retries,
            counts_json(&self.sent),
            self.receipts.len(),
            self.msg_items.len(),
            counts_json(&self.bob_counts),
            js(&self.bob_file),
            opt_ms(&self.clock, self.file.sent_at),
            opt_ms(&self.clock, self.file.stage_sent_at),
            opt_ms(&self.clock, self.file.delivered_at),
            opt_ms(&self.clock, self.file.failed_at),
            self.file
                .failed_reason
                .as_deref()
                .map_or_else(|| "null".to_owned(), js),
            opt_ms(&self.clock, self.karsi_at),
            self.probes,
            self.probe_errors,
            self.dropped.load(Ordering::Relaxed),
            self.link.json(),
            bob_exit,
            shutdown_note,
            self.out.lines,
            self.out.errors,
            notes.join(",")
        );
        let summary = with_extra(
            summary,
            &format!(
                "\"wire\":{wire},\"ptt_presses\":[{}]",
                self.chosen.join(",")
            ),
        );
        let summary = with_extra(summary, &hooks);
        self.out.flush();
        (summary, ok)
    }
}

fn start_alice_node(
    cfg: &Config,
    tap: Tap,
    counters: Arc<LinkCounters>,
) -> Result<KamaNode, String> {
    wait_marker(PROV_READY, cfg.connect_timeout)
        .ok_or_else(|| format!("{PROV_READY} {:?} içinde gelmedi", cfg.connect_timeout))?;
    let link = SockLink::bind(SOCK_ALICE, SOCK_BOB, 1, counters)?;
    start_node(cfg, ALICE, tap, link)
}

fn run_alice(cfg: Config) -> (String, bool) {
    let clock = Clock::new();
    let mut out = match Out::create("alice.jsonl") {
        Ok(out) => out,
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
    out.line(&header_line(&cfg, &clock));
    let (tx, rx) = mpsc::sync_channel(EVENT_QUEUE_CAP);
    let dropped = Arc::new(AtomicU64::new(0));
    let link_counters = Arc::new(LinkCounters::default());
    let plan = build_plan(&cfg);
    let mut alice = Alice {
        cfg: cfg.clone(),
        clock,
        out,
        rx,
        dropped: Arc::clone(&dropped),
        link: Arc::clone(&link_counters),
        node: None,
        t0: None,
        connected_at: None,
        plan,
        next: 0,
        tries: 0,
        retries: 0,
        streams: BTreeMap::new(),
        pressed: BTreeMap::new(),
        chosen: Vec::new(),
        file: FileLog::default(),
        sent: BTreeMap::new(),
        msg_items: BTreeMap::new(),
        receipts: BTreeSet::new(),
        karsi_at: None,
        bob_counts: BTreeMap::new(),
        bob_file: String::from("none"),
        bob_complete_at: None,
        next_probe: Instant::now(),
        probes: 0,
        probe_errors: 0,
        last_probe: None,
        notes: Vec::new(),
        lg: LgTap::default(),
    };
    let tap = Tap {
        tx,
        dropped,
        stall: None,
    };
    let result = match start_alice_node(&cfg, tap, link_counters) {
        Ok(node) => {
            alice.node = Some(node);
            alice
                .out
                .line(&mark_line(&alice.clock, Instant::now(), "node_up", ""));
            alice.execute_run()
        }
        Err(e) => Err(e),
    };
    alice.finish(result)
}

// ── bob (sunucu, alıcı) ──────────────────────────────────────────────────────────────────────

struct Bob {
    cfg: Config,
    clock: Clock,
    out: Out,
    rx: Receiver<Stamped>,
    dropped: Arc<AtomicU64>,
    link: Arc<LinkCounters>,
    node: Option<KamaNode>,
    stall: Option<Arc<Stall>>,
    stall_logged: (bool, bool),
    t0_ns: Option<u64>,
    seen: BTreeSet<(char, u32, u32)>,
    counts: BTreeMap<char, u32>,
    duplicates: u64,
    unparsed: u64,
    file_status: &'static str,
    file_size: u64,
    file_match: Option<bool>,
    file_at: Option<Instant>,
    state_dirty: bool,
    next_probe: Instant,
    probes: u64,
    probe_errors: u64,
    /// [PTT-BİRLEŞİK · pilot · ölçüm] Akış başına gözlem (≤ basış sayısı).
    presses: BTreeMap<u32, PressRx>,
    /// [ÖLÇÜM-KANCASI] Hat koruması ACK kaydı dökümü (`lg` satırları).
    lg: LgTap,
    /// [ÖLÇÜM-KANCASI] Alındı gönderim anı eşlemesi (`rcpt_tx` satırları).
    rcpt: RcptTap,
}

impl Bob {
    fn count(&mut self, class: char, spurt: u32, idx: u32) -> bool {
        if self.seen.insert((class, spurt, idx)) {
            *self.counts.entry(class).or_insert(0) += 1;
            self.state_dirty = true;
            true
        } else {
            self.duplicates += 1;
            false
        }
    }

    fn check_file(path: &Path, size: u64) -> Option<bool> {
        let bytes = fs::read(path).ok()?;
        Some(
            u64::try_from(bytes.len()).ok()? == size
                && bytes.iter().enumerate().all(|(i, b)| *b == file_byte(i)),
        )
    }

    fn on_event(&mut self, stamped: Stamped) {
        let Stamped { at, event } = stamped;
        let t = self.clock.ms(at);
        match event {
            NodeEvent::TextReceived { msg_id, text, .. } => match parse_text(&text) {
                Some((class, idx)) => {
                    self.rcpt.note_text(msg_id, class, idx, at);
                    let first = self.count(class, 0, idx);
                    self.out.line(&format!(
                        "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":{},\"i\":{idx},\"mid\":{msg_id},\
                         \"len\":{},\"first\":{first}}}",
                        js(&class.to_string()),
                        text.len()
                    ));
                }
                None => {
                    self.rcpt.note_text(msg_id, '?', 0, at);
                    self.unparsed += 1;
                }
            },
            NodeEvent::SignalingReceived { bytes, .. } => match parse_sig(&bytes) {
                Some(idx) => {
                    let first = self.count('S', 0, idx);
                    self.out.line(&format!(
                        "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"S\",\"i\":{idx},\"len\":{},\
                         \"first\":{first}}}",
                        bytes.len()
                    ));
                }
                None => self.unparsed += 1,
            },
            NodeEvent::PttOpened {
                stream_id,
                bundle_ms,
                ..
            } => {
                let first = self.count('O', stream_id, 0);
                let press = self.presses.entry(stream_id).or_default();
                press.started += 1;
                press.bundle_ms = kama_app::codec::bundle_ms_from_code(bundle_ms);
                self.out.line(&format!(
                    "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"O\",\"stream\":{stream_id},\
                     \"first\":{first},\"bundle_ms\":{}}}",
                    kama_app::codec::bundle_ms_from_code(bundle_ms)
                ));
            }
            NodeEvent::PttBundle {
                stream_id, payload, ..
            } => match parse_bundle(&payload) {
                Some((spurt, idx)) => {
                    if let Some((press_ms, created_ms, step, n)) = parse_press_timing(&payload) {
                        let press = self.presses.entry(stream_id).or_default();
                        press.press.get_or_insert((press_ms, step, n));
                        press.bundles.entry(idx).or_insert((created_ms, t));
                    }
                    let first = self.count('P', spurt, idx);
                    self.out.line(&format!(
                        "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"P\",\"sp\":{spurt},\"i\":{idx},\
                         \"stream\":{stream_id},\"first\":{first}}}"
                    ));
                }
                None => self.unparsed += 1,
            },
            NodeEvent::PttClosed { stream_id, .. } => {
                let first = self.count('C', stream_id, 0);
                let press = self.presses.entry(stream_id).or_default();
                press.ended += 1;
                press.ended_at.get_or_insert(t);
                self.out.line(&format!(
                    "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"C\",\"stream\":{stream_id},\
                     \"first\":{first}}}"
                ));
            }
            NodeEvent::FileReceived { path, size, .. } => {
                let matched = Self::check_file(&path, size);
                let _ = fs::remove_file(&path);
                if self.file_at.is_none() {
                    self.file_at = Some(at);
                    self.file_status = "received";
                    self.file_size = size;
                    self.file_match = matched;
                    self.state_dirty = true;
                }
                self.out.line(&format!(
                    "{{\"k\":\"rx\",\"t\":{t:.3},\"c\":\"F\",\"size\":{size},\"match\":{}}}",
                    matched.map_or_else(|| "null".to_owned(), |m| m.to_string())
                ));
            }
            NodeEvent::FileFailed { file, reason, .. } => {
                if matches!(file, FileRef::Incoming(_)) && self.file_at.is_none() {
                    self.file_at = Some(at);
                    self.file_status = "failed";
                    self.state_dirty = true;
                }
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"file_failed\",\"file\":{},\"why\":{}}}",
                    js(&format!("{file:?}")),
                    js(&format!("{reason:?}"))
                ));
            }
            NodeEvent::Connected { peer_id } => {
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"connected\",\"peer\":{}}}",
                    js(&peer_id)
                ));
            }
            NodeEvent::Closed { peer_id, reason } => {
                self.out.line(&format!(
                    "{{\"k\":\"ev\",\"t\":{t:.3},\"e\":\"closed\",\"peer\":{},\"why\":{}}}",
                    js(&peer_id),
                    js(&format!("{reason:?}"))
                ));
            }
            NodeEvent::Receipt { .. }
            | NodeEvent::FileProgress { .. }
            | NodeEvent::ConnectProgress { .. }
            | NodeEvent::Metrics(_) => {}
        }
    }

    fn pump_events(&mut self) {
        while let Ok(stamped) = self.rx.try_recv() {
            self.on_event(stamped);
        }
    }

    fn write_state(&mut self) {
        if !self.state_dirty {
            return;
        }
        let mut body = counts_text(&self.counts);
        let _ = writeln!(body, "F {}", self.file_status);
        if write_marker(BOB_STATE, &body).is_ok() {
            self.state_dirty = false;
        }
    }

    fn log_stall(&mut self) {
        let Some(stall) = self.stall.as_ref() else {
            return;
        };
        let started = stall.started_ns.load(Ordering::Acquire);
        let ended = stall.ended_ns.load(Ordering::Acquire);
        let mut lines = Vec::new();
        if started != 0 && !self.stall_logged.0 {
            lines.push(format!(
                "{{\"k\":\"mark\",\"t\":{},\"m\":\"stall_start\"}}",
                ns_ms(started)
            ));
            self.stall_logged.0 = true;
        }
        if ended != 0 && !self.stall_logged.1 {
            lines.push(format!(
                "{{\"k\":\"mark\",\"t\":{},\"m\":\"stall_end\"}}",
                ns_ms(ended)
            ));
            self.stall_logged.1 = true;
        }
        for line in lines {
            self.out.line(&line);
        }
    }

    fn sample(&mut self) {
        let Some(node) = self.node.as_ref() else {
            return;
        };
        let probe = node.probe_for_test();
        let at = Instant::now();
        let trace = node.take_unit_trace_for_test();
        let mut oldest = None;
        match probe {
            Ok(p) => {
                self.out.line(&ts_line(&self.clock, at, &p));
                self.lg.drain(&self.clock, at, &p, &mut self.out);
                self.probes += 1;
                oldest = Some((oldest_emissions(&p), p));
            }
            Err(_) => self.probe_errors += 1,
        }
        match trace {
            Ok((records, dropped)) => {
                if !records.is_empty() || dropped > 0 {
                    self.out
                        .line(&trace_line(&self.clock, Instant::now(), &records, dropped));
                }
                // `at` iz okumasından önce alındı: o ana dek dinleyiciye verilmiş her TEXT'in
                // alındı kaydı bu okumada ya da öncekilerdedir.
                self.rcpt.note_trace(&records, dropped);
                self.rcpt.resolve(&self.clock, Some(at), &mut self.out);
            }
            Err(_) => self.probe_errors += 1,
        }
        // İz bu sondadan SONRA okundu: sondadaki en eski teyitsiz birim izdeki kayıtlardandır.
        if let Some((seen, p)) = oldest {
            self.rcpt.note_oldest(&self.clock, &p, &seen, &mut self.out);
        }
    }

    /// [ÖLÇÜM-KANCASI] Koşu sonu: son sondadan sonra işlenen TEXT'lerin alındı kayıtları için izi
    /// bir kez daha okur (salt okuma), kalan bütün eşlemeleri yazar.
    fn finish_receipts(&mut self) {
        if let Some(node) = self.node.as_ref() {
            match node.take_unit_trace_for_test() {
                Ok((records, dropped)) => {
                    if !records.is_empty() || dropped > 0 {
                        self.out
                            .line(&trace_line(&self.clock, Instant::now(), &records, dropped));
                    }
                    self.rcpt.note_trace(&records, dropped);
                }
                Err(_) => self.probe_errors += 1,
            }
        }
        self.rcpt.resolve(&self.clock, None, &mut self.out);
    }

    fn arm_from_t0(&mut self) {
        if self.t0_ns.is_some() {
            return;
        }
        let Some(t0) = read_marker(MARK_T0).and_then(|b| b.trim().parse::<u64>().ok()) else {
            return;
        };
        self.t0_ns = Some(t0);
        let mut extra = format!(",\"t0\":{}", ns_ms(t0));
        if let Some(stall) = self.stall.as_ref() {
            let at =
                t0.saturating_add(u64::try_from(self.cfg.stall_at.as_nanos()).unwrap_or(u64::MAX));
            stall.arm(at);
            let _ = write!(extra, ",\"stall_armed\":{}", ns_ms(at));
        }
        self.out
            .line(&mark_line(&self.clock, Instant::now(), "t0_seen", &extra));
    }

    fn hard_deadline(&self, started: Instant) -> Instant {
        let mut plan_len = self
            .cfg
            .text_at
            .saturating_add(self.cfg.text_every.saturating_mul(self.cfg.texts))
            .max(self.cfg.sig_at + self.cfg.sig_every.saturating_mul(self.cfg.sigs));
        for s in &self.cfg.ptt_at {
            plan_len = plan_len.max(*s + spurt_len(&self.cfg, self.cfg.ptt_bundles));
        }
        started
            + self.cfg.connect_timeout.saturating_mul(2)
            + plan_len
            + self.cfg.window
            + self.cfg.stall
            + BOB_SLACK
    }

    fn execute_run(&mut self) -> Result<bool, String> {
        let started = Instant::now();
        let deadline = self.hard_deadline(started);
        self.next_probe = started;
        let mut flush_at = started;
        loop {
            self.pump_events();
            self.arm_from_t0();
            self.log_stall();
            if Instant::now() >= self.next_probe {
                self.sample();
                self.next_probe = Instant::now() + self.cfg.probe_every;
                self.write_state();
            }
            if Instant::now() >= flush_at {
                self.out.flush();
                flush_at = Instant::now() + Duration::from_secs(1);
            }
            if read_marker(MARK_ALICE_DONE).is_some() {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            match self.rx.recv_timeout(PUMP_STEP) {
                Ok(stamped) => self.on_event(stamped),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
        }
    }

    fn finish(&mut self, result: Result<bool, String>) -> (String, bool) {
        self.pump_events();
        self.log_stall();
        self.sample();
        self.pump_events();
        self.finish_receipts();
        self.state_dirty = true;
        self.write_state();
        let (outcome, error, ok) = match &result {
            Ok(true) => ("alice_done", "null".to_owned(), true),
            Ok(false) => ("deadline", "null".to_owned(), true),
            Err(e) => ("error", js(e), false),
        };
        let (stall_start, stall_end) = self.stall.as_ref().map_or((0, 0), |s| {
            (
                s.started_ns.load(Ordering::Acquire),
                s.ended_ns.load(Ordering::Acquire),
            )
        });
        let wire = self
            .node
            .as_ref()
            .map_or_else(|| "null".to_owned(), |n| wire_json(n, self.cfg.live_diag));
        let hooks = match self.node.as_ref() {
            Some(n) => hooks_json(n, &self.clock, &mut self.lg, &mut self.out),
            None => HOOKS_NULL.to_owned(),
        };
        let mut shutdown_note = "null".to_owned();
        if let Some(node) = self.node.take() {
            if let Err(e) = node.shutdown() {
                shutdown_note = js(&e.to_string());
            }
        }
        let _ = fs::remove_dir_all(format!("node-{BOB}"));
        let _ = fs::remove_dir_all(PROV_DIR);
        let summary = format!(
            "{{\"schema\":{},\"role\":\"bob\",\"scenario\":{},\"profile\":{},\"rep\":{},\
             \"result\":{},\"error\":{},\"t0\":{},\"counts\":{},\"duplicates\":{},\"unparsed\":{},\
             \"file\":{{\"status\":{},\"size\":{},\"match\":{},\"at\":{}}},\"stall\":{{\"start\":{},\
             \"end\":{}}},\"probes\":{},\"probe_errors\":{},\"listener_drops\":{},\"link\":{},\
             \"shutdown_error\":{},\"jsonl_lines\":{},\"jsonl_errors\":{}}}",
            js(SCHEMA),
            js(self.cfg.scenario.name()),
            js(&self.cfg.profile),
            self.cfg.rep,
            js(outcome),
            error,
            self.t0_ns.map_or_else(|| "null".to_owned(), ns_ms),
            counts_json(&self.counts),
            self.duplicates,
            self.unparsed,
            js(self.file_status),
            self.file_size,
            self.file_match
                .map_or_else(|| "null".to_owned(), |m| m.to_string()),
            opt_ms(&self.clock, self.file_at),
            ns_ms(stall_start),
            ns_ms(stall_end),
            self.probes,
            self.probe_errors,
            self.dropped.load(Ordering::Relaxed),
            self.link.json(),
            shutdown_note,
            self.out.lines,
            self.out.errors
        );
        let presses: Vec<String> = self
            .presses
            .iter()
            .map(|(stream, press)| press.json(*stream))
            .collect();
        let summary = with_extra(
            summary,
            &format!("\"wire\":{wire},\"ptt_presses\":[{}]", presses.join(",")),
        );
        let summary = with_extra(summary, &format!("{hooks},{}", self.rcpt.json()));
        self.out.flush();
        let _ = write_marker(MARK_BOB_EXIT, "exit");
        (summary, ok)
    }
}

fn start_bob_node(cfg: &Config, tap: Tap, counters: Arc<LinkCounters>) -> Result<KamaNode, String> {
    provision(cfg)?;
    let link = SockLink::bind(SOCK_BOB, SOCK_ALICE, 2, counters)?;
    start_node(cfg, BOB, tap, link)
}

fn run_bob(cfg: Config) -> (String, bool) {
    let clock = Clock::new();
    let mut out = match Out::create("bob.jsonl") {
        Ok(out) => out,
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
    out.line(&header_line(&cfg, &clock));
    let (tx, rx) = mpsc::sync_channel(EVENT_QUEUE_CAP);
    let dropped = Arc::new(AtomicU64::new(0));
    let link_counters = Arc::new(LinkCounters::default());
    let stall = cfg.scenario.has_stall().then(|| {
        Arc::new(Stall {
            clock,
            armed_at_ns: AtomicU64::new(0),
            length: cfg.stall,
            started_ns: AtomicU64::new(0),
            ended_ns: AtomicU64::new(0),
        })
    });
    let mut bob = Bob {
        cfg: cfg.clone(),
        clock,
        out,
        rx,
        dropped: Arc::clone(&dropped),
        link: Arc::clone(&link_counters),
        node: None,
        stall: stall.clone(),
        stall_logged: (false, false),
        t0_ns: None,
        seen: BTreeSet::new(),
        counts: BTreeMap::new(),
        duplicates: 0,
        unparsed: 0,
        file_status: "none",
        file_size: 0,
        file_match: None,
        file_at: None,
        state_dirty: true,
        next_probe: Instant::now(),
        probes: 0,
        probe_errors: 0,
        presses: BTreeMap::new(),
        lg: LgTap::default(),
        rcpt: RcptTap::default(),
    };
    let tap = Tap { tx, dropped, stall };
    let result = match start_bob_node(&cfg, tap, link_counters) {
        Ok(node) => {
            bob.node = Some(node);
            match write_marker(MARK_BOB_UP, "up") {
                Ok(()) => {
                    bob.out
                        .line(&mark_line(&bob.clock, Instant::now(), "node_up", ""));
                    bob.execute_run()
                }
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    };
    bob.finish(result)
}

// ── Giriş ────────────────────────────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match parse_args(&args) {
        Ok(Parsed::Help) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Run(cfg)) => *cfg,
        Err(e) => {
            eprintln!("f0_flows: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = std::env::set_current_dir(&cfg.dir) {
        eprintln!("f0_flows: --dir {:?}: {e}", cfg.dir);
        return ExitCode::from(2);
    }
    if let Err(e) = fs::create_dir_all(SYNC_DIR) {
        eprintln!("f0_flows: {SYNC_DIR}: {e}");
        return ExitCode::from(2);
    }
    let role_kind = cfg.role;
    let (role, (summary, ok)) = match role_kind {
        Role::Alice => (ALICE, run_alice(cfg)),
        Role::Bob => (BOB, run_bob(cfg)),
    };
    if let Err(e) = fs::write(format!("{role}.json"), format!("{summary}\n")) {
        eprintln!("f0_flows: {role}.json: {e}");
    }
    println!("{summary}");
    if ok {
        ExitCode::SUCCESS
    } else {
        eprintln!("f0_flows: {role}: hata (bkz. {role}.json)");
        ExitCode::from(1)
    }
}
