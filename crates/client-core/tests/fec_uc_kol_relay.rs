//! [H1 · FEC üç kol · gerçek yığın hücresi] Ürün runtime'ı (`ClientEndpoint` ↔ `ServerEndpoint`,
//! gerçek UDP, `IdentitySeam`) kullanıcı-alanı biçimlendirici röle üzerinden; netem/tc GEREKTİRMEZ.
//! Röle `client-core/tests/shaped_relay.rs` (dal `fix/ack-after-admit`) ile aynı biçimlendiricidir
//! (hız, sınırlı FIFO, tek yön gecikme, tohumlu kayıp); buna Gilbert-Elliott kümeli kayıp eklendi.
//!
//! Burada ölçülen (ii): **gerçekten çözüldü ve uygulamaya teslim edildi** — alıcının
//! `fec_ingest` → `admit_chunk` yolu (`ServerStats::fec_recovered_units`) ve uygulamanın
//! `next_delivery` ile aldığı birimler. Simülatörün (i) "çözülebilir" sayımı buraya karışmaz.
//!
//! Kollar `RuntimeConfig::default()` (ürün profili) üstünde YALNIZ FEC alanını değiştirir:
//! FEC kapalı = `manual_r: Some(0)` (çerçeve aynı, parite yok) · sabit = `manual_r: Some(2)` ·
//! fuzzy = `manual_r: None` (ürün) · fuzzy+süre = fuzzy + `send_with_deadline`.
//!
//! Gerçek zamanlıdır (belirlenimli değil): kayıp deseni tohumla ve paket sırasıyla sabit, zamanlama
//! değil. Yalnız `CC_FEC3_REAL=1` ile koşar. Anahtarlar: `CC_FEC3_CELL` (ptt|ptt-ge|file|file-ge|
//! stream|stream-ge|all), `CC_FEC3_ARMS` (off,fixed,fuzzy,fuzzy-dl), `CC_FEC3_SEEDS` (varsayılan 10),
//! `CC_FEC3_DEADLINE_MS` (600).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use client_core::runtime::{
    ClientEndpoint, ClientStats, FecRuntimeConfig, RuntimeConfig, ServerEndpoint, ServerStats,
};
use client_core::seam::IdentitySeam;
use tokio::net::UdpSocket;

const DATAGRAM_MAX: usize = 2048;
const TICK: Duration = Duration::from_millis(1);
/// UDP/IP zarfı (serileştirme hesabına girer; röle yalnız UDP gövdesini görür).
const UDP_IP_OVERHEAD: u64 = 28;

#[derive(Debug, Clone, Copy)]
enum Loss {
    Bernoulli(u64),
    /// Gilbert-Elliott (ppm): p_gb, p_bg.
    Ge(u64, u64),
}

/// Bir yönün biçimlendiricisi: serileştirme + sınırlı kuyruk + gecikme + kayıp.
struct Shaper {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    loss: Loss,
    ge_bad: bool,
    lcg: u64,
    busy: VecDeque<Instant>,
    wire: VecDeque<(Instant, Vec<u8>)>,
    sent: u64,
    bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
}

impl Shaper {
    fn new(rate_bps: u64, limit: usize, delay: Duration, loss: Loss, seed: u64) -> Self {
        Self {
            rate_bps,
            limit,
            delay,
            loss,
            ge_bad: false,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            sent: 0,
            bytes: 0,
            dropped_queue: 0,
            dropped_random: 0,
        }
    }

    fn next(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.lcg >> 33
    }

    fn drops(&mut self) -> bool {
        match self.loss {
            Loss::Bernoulli(0) => false,
            Loss::Bernoulli(pm) => self.next() % 1000 < pm,
            Loss::Ge(p_gb, p_bg) => {
                let bad = self.ge_bad;
                let u = self.next() % 1_000_000;
                self.ge_bad = if bad { u >= p_bg } else { u < p_gb };
                bad
            }
        }
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            return;
        }
        let size = pkt.len() as u64 + UDP_IP_OVERHEAD;
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let ser = Duration::from_nanos(size * 1_000_000_000 / self.rate_bps.max(1));
        let done = start + ser;
        self.busy.push_back(done);
        self.sent += 1;
        self.bytes += size;
        if self.drops() {
            self.dropped_random += 1;
            return;
        }
        self.wire.push_back((done + self.delay, pkt));
    }

    fn take_due(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|(t, _)| *t <= now) {
            if let Some((_, p)) = self.wire.pop_front() {
                out.push(p);
            }
        }
        out
    }
}

/// Röle: `front` (istemci tarafı) ↔ `back` (sunucuya bağlı). Kilit `await` boyunca tutulmaz.
async fn relay(
    front: UdpSocket,
    back: UdpSocket,
    fwd: Arc<Mutex<Shaper>>,
    rev: Arc<Mutex<Shaper>>,
    mut stop: tokio::sync::oneshot::Receiver<()>,
) {
    let mut client: Option<SocketAddr> = None;
    let mut up = vec![0u8; DATAGRAM_MAX];
    let mut down = vec![0u8; DATAGRAM_MAX];
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            r = front.recv_from(&mut up) => {
                let Ok((n, from)) = r else { continue };
                client = Some(from);
                fwd.lock().expect("kilit").offer(Instant::now(), up[..n].to_vec());
            }
            r = back.recv(&mut down) => {
                let Ok(n) = r else { continue };
                rev.lock().expect("kilit").offer(Instant::now(), down[..n].to_vec());
            }
            _ = tick.tick() => {
                let now = Instant::now();
                let due_f = fwd.lock().expect("kilit").take_due(now);
                for p in due_f {
                    let _ = back.send(&p).await;
                }
                let due_r = rev.lock().expect("kilit").take_due(now);
                if let Some(to) = client {
                    for p in due_r {
                        let _ = front.send_to(&p, to).await;
                    }
                }
            }
            _ = &mut stop => break,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    Off,
    Fixed,
    Fuzzy,
    FuzzyDeadline,
}

impl Arm {
    fn label(self, deadline_ms: u64) -> String {
        match self {
            Self::Off => "FEC kapalı".into(),
            Self::Fixed => "sabit 2/8".into(),
            Self::Fuzzy => "fuzzy (ürün)".into(),
            Self::FuzzyDeadline => format!("fuzzy+süre {deadline_ms} ms"),
        }
    }
    fn manual_r(self) -> Option<u16> {
        match self {
            Self::Off => Some(0),
            Self::Fixed => Some(2),
            Self::Fuzzy | Self::FuzzyDeadline => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Cell {
    key: &'static str,
    label: &'static str,
    units: u64,
    unit_bytes: usize,
    offer_interval: Duration,
    loss: Loss,
    t_max: Duration,
}

const GE: Loss = Loss::Ge(62_500, 250_000);

fn cells() -> Vec<Cell> {
    let ptt = |key, label, loss| Cell {
        key,
        label,
        units: 6,
        unit_bytes: 400,
        offer_interval: Duration::ZERO,
        loss,
        t_max: Duration::from_secs(60),
    };
    let file = |key, label, loss, iv| Cell {
        key,
        label,
        units: 150,
        unit_bytes: 1000,
        offer_interval: iv,
        loss,
        t_max: Duration::from_secs(240),
    };
    vec![
        ptt("ptt", "PTT 6×400 B · %20 rastgele", Loss::Bernoulli(200)),
        ptt("ptt-ge", "PTT 6×400 B · %20 GE (patlama 4)", GE),
        file(
            "file",
            "dosya 150×1000 B · %20 rastgele",
            Loss::Bernoulli(200),
            Duration::ZERO,
        ),
        file(
            "file-ge",
            "dosya 150×1000 B · %20 GE (patlama 4)",
            GE,
            Duration::ZERO,
        ),
        file(
            "stream",
            "akış 150×1000 B / 160 ms · %20 rastgele",
            Loss::Bernoulli(200),
            Duration::from_millis(160),
        ),
        file(
            "stream-ge",
            "akış 150×1000 B / 160 ms · %20 GE (patlama 4)",
            GE,
            Duration::from_millis(160),
        ),
    ]
}

struct Run {
    completed: Option<Duration>,
    unit_times: Vec<Duration>,
    /// Tamamlanma anında ileri yönde tele çıkan bayt (röle sayacı; UDP/IP dahil) — tel bütçesi.
    fwd_bytes_at_done: u64,
    client: ClientStats,
    server: ServerStats,
}

async fn run_cell(cell: &Cell, arm: Arm, seed: u64, deadline_ms: u64) -> Run {
    let mut cfg = RuntimeConfig::default();
    let mut fec = cfg.fec.expect("ürün profilinde FEC açık");
    fec.manual_r = arm.manual_r();
    cfg.fec = Some(FecRuntimeConfig { ..fec });
    let server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().expect("adres"),
        cfg.clone(),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("server bind");
    let front = UdpSocket::bind("127.0.0.1:0").await.expect("röle ön");
    let back = UdpSocket::bind("127.0.0.1:0").await.expect("röle arka");
    back.connect(server.local_addr())
        .await
        .expect("röle→sunucu");
    let front_addr = front.local_addr().expect("adres");
    let delay = Duration::from_millis(150); // tek yön → RTT 300 ms
    let fwd = Arc::new(Mutex::new(Shaper::new(12_500, 6, delay, cell.loss, seed)));
    let rev = Arc::new(Mutex::new(Shaper::new(
        12_500,
        64,
        delay,
        Loss::Bernoulli(0),
        seed ^ 0x5A5A,
    )));
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let relay_task = tokio::spawn(relay(
        front,
        back,
        Arc::clone(&fwd),
        Arc::clone(&rev),
        stop_rx,
    ));
    let client = ClientEndpoint::connect(front_addr, cfg, Box::new(IdentitySeam::new()))
        .await
        .expect("client connect");
    let mut server = server;
    let t0 = Instant::now();
    let units = cell.units;
    let unit_bytes = cell.unit_bytes;
    let iv = cell.offer_interval;
    let deadline_ns = (arm == Arm::FuzzyDeadline).then_some(deadline_ms * 1_000_000);
    let sender = client.sender();
    let offer_task = tokio::spawn(async move {
        for i in 0..units {
            if !iv.is_zero() {
                let due = t0 + iv * u32::try_from(i).unwrap_or(u32::MAX);
                tokio::time::sleep_until(due.into()).await;
            }
            let mut payload = vec![0x5Au8; unit_bytes];
            payload[..8].copy_from_slice(&i.to_be_bytes());
            sender
                .send_with_deadline(payload, deadline_ns)
                .expect("offer");
        }
    });
    let mut unit_times = Vec::new();
    let mut fwd_at_done = 0;
    let completed = tokio::time::timeout(cell.t_max, async {
        while (unit_times.len() as u64) < units {
            if server.next_delivery().await.is_some() {
                unit_times.push(t0.elapsed());
            } else {
                break;
            }
        }
        fwd_at_done = fwd.lock().expect("kilit").bytes;
        t0.elapsed()
    })
    .await
    .ok();
    let _ = offer_task.await;
    let client_stats = client.shutdown().await;
    let server_stats = server.shutdown().await;
    let _ = stop_tx.send(());
    let _ = relay_task.await;
    if completed.is_none() {
        fwd_at_done = fwd.lock().expect("kilit").bytes;
    }
    Run {
        completed,
        unit_times,
        fwd_bytes_at_done: fwd_at_done,
        client: client_stats,
        server: server_stats,
    }
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

fn pct(v: &mut [f64], p: usize) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[((v.len() - 1) * p) / 100])
}

fn f2(v: Option<f64>) -> String {
    v.map_or("—".into(), |x| format!("{x:.2}"))
}

fn row(cell: &Cell, label: &str, runs: &[&Run]) {
    let n = runs.len().max(1) as u64;
    let mut comp: Vec<f64> = runs
        .iter()
        .filter_map(|r| r.completed.map(|d| d.as_secs_f64()))
        .collect();
    let mut units: Vec<f64> = runs
        .iter()
        .flat_map(|r| r.unit_times.iter().map(Duration::as_secs_f64))
        .collect();
    let done = comp.len();
    let max = comp
        .iter()
        .copied()
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))));
    let s = |f: fn(&Run) -> u64| runs.iter().map(|r| f(r)).sum::<u64>();
    let parity_runs = runs.iter().filter(|r| r.client.tx_fec_bytes > 0).count();
    println!(
        "| {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} |",
        cell.label,
        label,
        done,
        runs.len(),
        f2(pct(&mut comp.clone(), 50)),
        f2(pct(&mut comp, 95)),
        f2(max),
        f2(pct(&mut units.clone(), 50)),
        f2(pct(&mut units, 95)),
        s(|r| r.server.fec_recovered_units),
        s(|r| r.server.fec_repairs_useful),
        s(|r| r.client.tx_data_bytes) / n,
        s(|r| r.client.tx_retx_bytes + r.client.tx_dup_bytes) / n,
        s(|r| r.client.tx_fec_bytes) / n,
        s(|r| r.client.tx_bytes) / n,
        s(|r| r.fwd_bytes_at_done) / n,
        parity_runs,
        runs.len(),
    );
}

/// Ölçüm (yalnız `CC_FEC3_REAL=1`): hücre × kol × tohum; markdown tablosu basar. Kapı değildir.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fec_three_arms_real_stack() {
    if std::env::var("CC_FEC3_REAL").is_err() {
        return;
    }
    let seeds = env_u64("CC_FEC3_SEEDS", 10);
    let deadline_ms = env_u64("CC_FEC3_DEADLINE_MS", 600);
    let want_cell = std::env::var("CC_FEC3_CELL").unwrap_or_else(|_| "all".into());
    let want_arms =
        std::env::var("CC_FEC3_ARMS").unwrap_or_else(|_| "off,fixed,fuzzy,fuzzy-dl".into());
    let arms: Vec<Arm> = want_arms
        .split(',')
        .filter_map(|a| match a {
            "off" => Some(Arm::Off),
            "fixed" => Some(Arm::Fixed),
            "fuzzy" => Some(Arm::Fuzzy),
            "fuzzy-dl" => Some(Arm::FuzzyDeadline),
            _ => None,
        })
        .collect();
    println!("| hücre | kol | teslim | tamamlanma p50 s | p95 s | max s | birim p50 s | birim p95 s | (ii) FEC ile çözülüp teslim edilen birim (Σ) | yararlı parite (Σ) | veri B/koşu | retx+dup B/koşu | parite B/koşu | gönderici toplam B/koşu (kapanış dahil) | tel B/koşu tamamlanmada (UDP/IP dahil) | parite tele çıkan koşu |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for cell in cells()
        .iter()
        .filter(|c| want_cell == "all" || want_cell == c.key)
    {
        for &arm in &arms {
            let mut runs = Vec::new();
            for seed in 1..=seeds {
                let r = run_cell(cell, arm, seed, deadline_ms).await;
                if std::env::var("CC_FEC3_VERBOSE").is_ok() {
                    println!(
                        "  {} {} seed{seed}: tamam={} teslim={}/{} kurtarılan={} yararlı_parite={} parite_rx={} boşa(tamam/tahliye)={}/{} blok_latch={} fec_B={} retx_B={} tel_B={}",
                        cell.key,
                        arm.label(deadline_ms),
                        r.completed.map_or("YOK".into(), |d| format!("{:.2}", d.as_secs_f64())),
                        r.unit_times.len(),
                        cell.units,
                        r.server.fec_recovered_units,
                        r.server.fec_repairs_useful,
                        r.server.fec_repairs_rx,
                        r.server.fec_repairs_wasted_complete,
                        r.server.fec_repairs_wasted_evicted,
                        r.client.fec_blocks_latched,
                        r.client.tx_fec_bytes,
                        r.client.tx_retx_bytes,
                        r.fwd_bytes_at_done,
                    );
                }
                runs.push(r);
            }
            let all: Vec<&Run> = runs.iter().collect();
            row(cell, &arm.label(deadline_ms), &all);
            if matches!(arm, Arm::Fuzzy | Arm::FuzzyDeadline) {
                let active: Vec<&Run> = runs.iter().filter(|r| r.client.tx_fec_bytes > 0).collect();
                if !active.is_empty() && active.len() < runs.len() {
                    row(
                        cell,
                        &format!("{} — yalnız parite çıkan alt küme", arm.label(deadline_ms)),
                        &active,
                    );
                }
            }
        }
    }
}
