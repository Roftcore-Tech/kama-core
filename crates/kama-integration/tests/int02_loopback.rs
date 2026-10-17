//! INT-02 düzeneğinin netem'SİZ kısa duman testi (plan K4G-05 DÜZENEK 1; CI'da koşabilir).
//!
//! `examples/int02_node.rs` ile AYNI kod (modül olarak dahil edilir): iki ürün düğümü
//! (`KamaNode::start`; negatif kolda `start_recovery_off_for_test`) tek süreçte, loopback UDP;
//! anahtarlar `kama-provision` KÜTÜPHANESİYLE test içinde üretilir (elle yazılmış fikstür DEĞİL;
//! `root.json` cihaz dizinlerine girmez). Her kol az mesajla, KAYIPSIZ koşar: düzeneğin uçtan uca
//! çalıştığını ve ölçüm sondasının sayaçları okuduğunu sınar. Kayıp altındaki ölçüm (GATING) netem
//! sürücüsünün işidir (`scripts/netem/int02_run.sh`); burada hiçbir GATING hükmü verilmez.
//!
//! Kesinti kolu (`ek-kesinti`) netem'siz koşar: sürücünün işaret dosyası eşgüdümü bir yardımcı iş
//! parçacığıyla taklit edilir (gerçek kesinti YOK) — yalnız eşgüdüm yolunun ve PTT/metin akışının
//! uçtan uca çalıştığı sınanır.
//!
//! Zamanlama: her bekleme bir olayın ÜST SINIRIDIR (düzeneğin `--window-s` / `--connect-timeout-s`
//! karşılıkları); kesinti taklidindeki kısa uyku yalnız işaret dosyası yoklamasıdır.

#[path = "../examples/int02_node.rs"]
mod int02_node;

use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kama_noise_wire::entropy::OsEntropy;
use kama_provision::format::{DeviceName, ROOT_FILE_NAME};
use kama_provision::provision::{init, InitRequest};

use int02_node::{
    run, Arm, Config, Outcome, Report, Side, ALICE, BOB, MARK_APPLIED, MARK_DONE, MARK_START,
};

/// Yön başına TEXT (duman: az).
const COUNT: u32 = 5;
/// Kesinti taklidinin işaret dosyası yoklama adımı ve "kesinti" süresi (gerçek kesinti yok).
const MARK_POLL: Duration = Duration::from_millis(5);
const FAKE_OUTAGE: Duration = Duration::from_millis(100);
/// Taklit sürücünün `KESINTI_BASLA`'yı bekleme üst sınırı.
const MARK_WAIT: Duration = Duration::from_secs(60);

/// Port ayıran testler aynı ikilide sırayla koşar: `free_ports`'un bıraktığı port, düğüm onu
/// bağlayana dek kardeş testin `free_ports`'una düşmesin.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `std::env::temp_dir()` altında bu teste özgü dizin; Drop'ta silinir (panik etmez).
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("int02-{label}-{pid}-{seq}-{nanos}"));
        fs::create_dir(&path).expect("geçici dizin");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// İki farklı boş UDP portu (soketler aynı anda açık tutulur ki aynı port iki kez verilmesin).
fn free_addrs() -> [SocketAddr; 2] {
    let sockets = [
        UdpSocket::bind("127.0.0.1:0").expect("UDP soketi"),
        UdpSocket::bind("127.0.0.1:0").expect("UDP soketi"),
    ];
    sockets.map(|socket| socket.local_addr().expect("adres"))
}

/// `kama-provision` kütüphanesiyle alice + bob (direct-mode); her "telefon"a yalnız kendi cihaz
/// dosyası kopyalanır. Dönüş: (cihaz dosyaları, veri dizinleri), alice önce.
fn provision(root: &Path) -> ([PathBuf; 2], [PathBuf; 2]) {
    let authority = root.join("authority");
    let names = [
        DeviceName::new(ALICE).expect("cihaz adı"),
        DeviceName::new(BOB).expect("cihaz adı"),
    ];
    let request = InitRequest {
        out_dir: authority.clone(),
        devices: names.to_vec(),
        hub: None,
        direct: names.iter().cloned().zip(free_addrs()).collect(),
    };
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("sistem saati")
        .as_secs();
    init(&request, &mut OsEntropy::new(), created_at).expect("provisioning");
    let phone = |name: &DeviceName| -> (PathBuf, PathBuf) {
        let dir = root.join(name.as_str());
        let config_dir = dir.join("config");
        fs::create_dir_all(&config_dir).expect("telefon dizini");
        let config = config_dir.join(name.file_name());
        fs::copy(authority.join(name.file_name()), &config).expect("cihaz dosyası");
        assert!(
            !config_dir.join(ROOT_FILE_NAME).exists(),
            "D9: root.json cihaz dizinine girmez"
        );
        (config, dir.join("data"))
    };
    let (alice_config, alice_data) = phone(&names[0]);
    let (bob_config, bob_data) = phone(&names[1]);
    ([alice_config, bob_config], [alice_data, bob_data])
}

/// Duman parametreleri: az mesaj, kısa aralık; bekleme üst sınırları geniş (yavaş CI).
fn smoke(arm: Arm, root: &Path) -> Config {
    let (configs, data) = provision(root);
    let mut cfg = Config::new(arm, 0, "loopback-netemsiz", configs, data);
    cfg.count = COUNT;
    cfg.interval = Duration::from_millis(10);
    cfg.connect_timeout = Duration::from_secs(30);
    cfg.window = Duration::from_secs(60);
    cfg.settle = Duration::from_secs(10);
    cfg.file_bytes = 8 * 1024;
    cfg.bundles = 6;
    cfg.bundle_ms = 10;
    cfg.bundle_bytes = 16;
    cfg.scratch_dir = Some(root.to_path_buf());
    if arm == Arm::EkKesinti {
        // Kesinti kolu eşgüdüm dizini ister (`Config::validate`); işaret dosyalarını taklit sürücü
        // bu dizinde yazar.
        let sync = root.join("sync");
        fs::create_dir(&sync).expect("eşgüdüm dizini");
        cfg.sync_dir = Some(sync);
    }
    cfg.validate().expect("duman yapılandırması geçerli");
    cfg
}

/// JSON tek satırdır ve bir nesnedir; kol ve şema yazılıdır.
fn assert_json_line(report: &Report, arm: Arm) {
    let line = &report.json;
    assert!(!line.contains('\n'), "tek satır: {line}");
    assert!(line.starts_with('{') && line.ends_with('}'), "{line}");
    assert!(line.contains(r#""schema":"int02-node-v3""#), "{line}");
    assert!(
        line.contains(&format!(r#""arm":"{}""#, arm.name())),
        "{line}"
    );
}

/// Kolun tamamlandığı ve iki düğümde de: tek `Connected`, kapanış / terk yok; Application ve
/// Crypto yeniden gönderimi 0; bağlam-önce dağıtım ve teslim kanalı düşüşü 0; dinleyici olay
/// düşürmedi.
fn assert_clean(report: &Report, arm: Arm) {
    assert_json_line(report, arm);
    assert_eq!(report.outcome, Outcome::Complete, "{}", report.json);
    assert_eq!(report.listener_drops, 0, "{}", report.json);
    for side in [Side::Alice, Side::Bob] {
        let node = &report.nodes[side.idx()];
        assert_eq!(node.connected, 1, "{}: {}", side.name(), report.json);
        assert_eq!(node.closed_session, 0, "{}: {}", side.name(), report.json);
        assert_eq!(node.abandoned, 0, "{}: {}", side.name(), report.json);
        let probe = node.probe.as_ref().expect("sonda okundu");
        assert_eq!(probe.messenger.resent_on_new_session, 0, "{}", report.json);
        assert_eq!(probe.messenger.resend_refused, 0, "{}", report.json);
        assert_eq!(probe.envelopes.identical_resends, 0, "{}", report.json);
        assert_eq!(probe.ctx_mismatch_drops, 0, "{}", report.json);
        assert_eq!(probe.hub_frames_dropped, 0, "{}", report.json);
        assert_eq!(probe.delivery_channel_drops, 0, "{}", report.json);
        assert_eq!(probe.events.closed, 0, "{}", report.json);
        assert_eq!(probe.events.renew_required, 0, "{}", report.json);
        assert!(!probe.sessions.is_empty(), "canlı oturum: {}", report.json);
    }
}

/// `sender` yönünde `COUNT` metin tam bir kez teslim edildi ve alındılandı.
fn assert_flow(report: &Report, sender: Side) {
    let flow = &report.flows[sender.idx()];
    let n = COUNT as usize;
    assert_eq!(flow.sent, n, "{}", report.json);
    assert_eq!(flow.delivered, n, "{}", report.json);
    assert_eq!(flow.receipts, n, "{}", report.json);
    assert_eq!(flow.duplicate_deliveries, 0, "{}", report.json);
    assert_eq!(flow.mismatched_texts, 0, "{}", report.json);
    assert_eq!(flow.unknown_deliveries, 0, "{}", report.json);
    assert_eq!(flow.duplicate_receipts, 0, "{}", report.json);
}

/// Tek oturum: düğüm başına tek `Ready`, tek canlı oturum.
fn assert_single_session(report: &Report) {
    for side in [Side::Alice, Side::Bob] {
        let probe = report.nodes[side.idx()]
            .probe
            .as_ref()
            .expect("sonda okundu");
        assert_eq!(probe.events.ready, 1, "{}: {}", side.name(), report.json);
        assert_eq!(probe.sessions.len(), 1, "{}: {}", side.name(), report.json);
    }
}

#[test]
fn gating1_alice_to_bob_text_and_receipt() {
    let _serial = serial();
    let root = TempDir::new("g1");
    let report = run(&smoke(Arm::Gating1, root.path()));
    assert_clean(&report, Arm::Gating1);
    assert_flow(&report, Side::Alice);
    assert_single_session(&report);
    assert!(report.quiescent, "teyitsiz birim kalmadı: {}", report.json);
    let alice = report.nodes[Side::Alice.idx()]
        .probe
        .as_ref()
        .expect("sonda");
    let bob = report.nodes[Side::Bob.idx()].probe.as_ref().expect("sonda");
    assert_eq!(alice.envelopes.inits_sent, 1, "{}", report.json);
    assert_eq!(bob.envelopes.responses_sent, 1, "{}", report.json);
    // [CEVAP PAKETİ 6] M1/M2 alanları: gönderen (alice) ucu ACK çerçevelerini saydı ve ilerleme
    // getirenleri ayırdı (ilerleme getiren ⊆ motorun işlediği ⊆ çözülen); denetim durumu (faz,
    // pacer) dolu; ACK günlüğü ve zaman çizelgesinin motor alanları JSON'da.
    let diag = &alice.sessions[0].diag;
    assert!(
        diag.ack.frames >= 1
            && diag.ack.new_units >= 1
            && diag.ack.new_units <= diag.ack.mapped
            && diag.ack.mapped <= diag.ack.frames,
        "{:?}",
        diag.ack
    );
    assert_eq!(diag.acks.total(), diag.ack.frames, "{:?}", diag.ack);
    assert!(
        diag.control.state.is_some() && diag.control.pacer_rate_bps > 0,
        "{:?}",
        diag.control
    );
    assert!(diag.clock_origin.is_some());
    assert!(
        report.acks_logged[Side::Alice.idx()] >= 1,
        "{}",
        report.json
    );
    assert!(
        report.timeline_cap > 0 && report.timeline_samples <= report.timeline_cap,
        "{}",
        report.json
    );
    for field in [
        r#""m_log":{"enabled":true"#,
        r#""new_units":"#,
        r#""last_ack_now":"#,
        r#""motor":{"#,
        r#""pacer_rate_bps":"#,
        r#""ack_frames_new_units":"#,
        r#""timeline":{"every_ms":5000.000"#,
    ] {
        assert!(report.json.contains(field), "{field}: {}", report.json);
    }
}

#[test]
fn gating2_responder_sends_first_and_both_ends_reach_epoch_one() {
    let _serial = serial();
    let root = TempDir::new("g2");
    let report = run(&smoke(Arm::Gating2, root.path()));
    assert_clean(&report, Arm::Gating2);
    assert_flow(&report, Side::Bob);
    assert_single_session(&report);
    for side in [Side::Alice, Side::Bob] {
        assert_eq!(
            report.nodes[side.idx()].final_epoch,
            Some(1),
            "{}: {}",
            side.name(),
            report.json
        );
    }
}

#[test]
fn gating6_simultaneous_connect_then_text_both_ways() {
    let _serial = serial();
    let root = TempDir::new("g6");
    let report = run(&smoke(Arm::Gating6, root.path()));
    assert_clean(&report, Arm::Gating6);
    assert_flow(&report, Side::Alice);
    assert_flow(&report, Side::Bob);
}

/// Negatif kolun kurucusu (`start_recovery_off_for_test`) uçtan uca çalışır. Netem'siz koşuda
/// kayıp yoktur: kolun kaybı göstermesi beklenmez; yalnız düzenek hatası olmadığı sınanır.
#[test]
fn negative_arm_builds_and_runs_with_recovery_off() {
    let _serial = serial();
    let root = TempDir::new("neg");
    let report = run(&smoke(Arm::Negative, root.path()));
    assert_json_line(&report, Arm::Negative);
    assert!(
        !matches!(report.outcome, Outcome::Error(_)),
        "{}",
        report.json
    );
    for side in [Side::Alice, Side::Bob] {
        let probe = report.nodes[side.idx()].probe.as_ref().expect("sonda");
        let wire_retx: u64 = probe.sessions.iter().map(|s| s.diag.tx_retx_bytes).sum();
        assert_eq!(
            wire_retx, 0,
            "kurtarma kapalı: tele retransmit yok: {}",
            report.json
        );
    }
}

#[test]
fn ek_file_and_ptt_clip_over_one_session() {
    let _serial = serial();
    let root = TempDir::new("ek-dp");
    let report = run(&smoke(Arm::EkDosyaPtt, root.path()));
    assert_clean(&report, Arm::EkDosyaPtt);
    assert_flow(&report, Side::Alice);
    assert_eq!(report.file_ok, Some(true), "{}", report.json);
    assert_eq!(report.ptt_ok, Some(true), "{}", report.json);
    // [CEVAP PAKETİ 4] Dosyanın kesin sonucu ayrı alanda; pencere son gönderimden başlar ve kol
    // tamamlanınca biter.
    assert_eq!(report.file_status, Some("delivered"), "{}", report.json);
    for field in [
        r#""outcome":{"status":"delivered","reason":null"#,
        r#""anchor":"last_send""#,
        r#""ended_by":"complete""#,
    ] {
        assert!(report.json.contains(field), "{field}: {}", report.json);
    }
}

/// Sürücünün işaret dosyası eşgüdümünü taklit eder: `KESINTI_BASLA` görülünce
/// `KESINTI_UYGULANDI`, kısa bir süre sonra `KESINTI_BITTI` (gerçek kesinti YOK).
fn fake_outage_driver(sync: PathBuf) -> JoinHandle<bool> {
    thread::spawn(move || {
        let deadline = Instant::now() + MARK_WAIT;
        while !sync.join(MARK_START).exists() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(MARK_POLL);
        }
        if fs::write(sync.join(MARK_APPLIED), b"").is_err() {
            return false;
        }
        thread::sleep(FAKE_OUTAGE);
        fs::write(sync.join(MARK_DONE), b"").is_ok()
    })
}

#[test]
fn ek_outage_coordination_path_runs_end_to_end() {
    let _serial = serial();
    let root = TempDir::new("ek-k");
    let cfg = smoke(Arm::EkKesinti, root.path());
    let sync = cfg.sync_dir.clone().expect("kesinti kolu eşgüdüm dizini");
    let driver = fake_outage_driver(sync);
    let report = run(&cfg);
    assert!(driver.join().expect("taklit sürücü"), "{}", report.json);
    assert_clean(&report, Arm::EkKesinti);
    assert_flow(&report, Side::Alice);
    assert_eq!(report.ptt_ok, Some(true), "{}", report.json);
    // [CEVAP PAKETİ 4] "Aynı oturum" kanıtı (kesinti başı → koşu sonu) ve metin teslimi ayrı
    // alanlarda; pencere kesinti bitiminden başlar.
    assert_eq!(report.outage_same_session, Some(true), "{}", report.json);
    for field in [
        r#""texts_complete":true"#,
        r#""same_session":true"#,
        r#""same_session_outage_only":true"#,
        r#""same_session_handles":true"#,
        r#""anchor":"outage_end""#,
    ] {
        assert!(report.json.contains(field), "{field}: {}", report.json);
    }
}

/// Komut satırı: zorunlu bayraklar, bilinmeyen / tekrarlı bayrak ve kesinti kolunun `--sync-dir`
/// şartı.
#[test]
fn command_line_is_parsed_without_clap() {
    let base: Vec<String> = [
        "--arm",
        "gating1",
        "--seed",
        "7",
        "--profile",
        "d200j200l30",
        "--alice-config",
        "a.json",
        "--bob-config",
        "b.json",
        "--alice-data",
        "ad",
        "--bob-data",
        "bd",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    match int02_node::parse_args(&base) {
        Ok(int02_node::Parsed::Run(cfg)) => {
            assert_eq!(cfg.arm, Arm::Gating1);
            assert_eq!(cfg.seed, 7);
            assert_eq!(cfg.count, 100, "varsayılan");
        }
        other => panic!("ayrıştırılmalıydı: {other:?}"),
    }
    // [CEVAP PAKETİ 4] Tek pencere parametresi `--window-s`; eski `--drain-s` yok.
    let mut window = base.clone();
    window.extend(["--window-s".to_owned(), "600".to_owned()]);
    match int02_node::parse_args(&window) {
        Ok(int02_node::Parsed::Run(cfg)) => assert_eq!(cfg.window, Duration::from_secs(600)),
        other => panic!("ayrıştırılmalıydı: {other:?}"),
    }
    let mut drain = base.clone();
    drain.extend(["--drain-s".to_owned(), "600".to_owned()]);
    assert!(int02_node::parse_args(&drain).is_err(), "--drain-s kalktı");
    // [CEVAP PAKETİ 6] Zaman çizelgesi aralığı ms cinsinden (`--timeline-every-s` kalktı); 0 =
    // kapalı (tavan 0), kapalı değilse ≥ 100 ms. Tavan parametrelerden türer: 600 s pencere ve 1 s
    // aralıkla 600'den fazla örnek, mutlak üst sınır 4096.
    let mut every = window.clone();
    every.extend(["--timeline-every-ms".to_owned(), "1000".to_owned()]);
    match int02_node::parse_args(&every) {
        Ok(int02_node::Parsed::Run(cfg)) => {
            assert_eq!(cfg.timeline_every, Duration::from_millis(1000));
            let cap = cfg.timeline_cap();
            assert!(cap > 600 && cap <= 4096, "tavan {cap}");
        }
        other => panic!("ayrıştırılmalıydı: {other:?}"),
    }
    let mut old_every = base.clone();
    old_every.extend(["--timeline-every-s".to_owned(), "1".to_owned()]);
    assert!(
        int02_node::parse_args(&old_every).is_err(),
        "--timeline-every-s kalktı"
    );
    let mut too_fast = base.clone();
    too_fast.extend(["--timeline-every-ms".to_owned(), "50".to_owned()]);
    assert!(int02_node::parse_args(&too_fast).is_err(), "en az 100 ms");
    let mut off = base.clone();
    off.extend(["--timeline-every-ms".to_owned(), "0".to_owned()]);
    match int02_node::parse_args(&off) {
        Ok(int02_node::Parsed::Run(cfg)) => assert_eq!(cfg.timeline_cap(), 0),
        other => panic!("ayrıştırılmalıydı: {other:?}"),
    }
    let mut unknown = base.clone();
    unknown.extend(["--nope".to_owned(), "1".to_owned()]);
    assert!(int02_node::parse_args(&unknown).is_err());
    let mut repeated = base.clone();
    repeated.extend(["--seed".to_owned(), "8".to_owned()]);
    assert!(int02_node::parse_args(&repeated).is_err());
    let outage: Vec<String> = base
        .iter()
        .map(|s| {
            if s == "gating1" {
                "ek-kesinti".to_owned()
            } else {
                s.clone()
            }
        })
        .collect();
    assert!(
        int02_node::parse_args(&outage).is_err(),
        "ek-kesinti --sync-dir ister"
    );
    assert!(matches!(
        int02_node::parse_args(&["--help".to_owned()]),
        Ok(int02_node::Parsed::Help)
    ));
}
