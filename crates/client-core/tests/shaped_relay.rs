//! [CC-PORT · gerçek yığın hücresi] Ürün runtime'ı (ClientEndpoint ↔ ServerEndpoint, gerçek UDP)
//! kullanıcı-alanı biçimlendirici röle üzerinden: hız (bayt/s), sınırlı FIFO kuyruk (paket), tek
//! yön gecikme, tohumlu rastgele kayıp — iki yönde ayrı. netem/tc GEREKTİRMEZ; motor simülasyonu
//! (`adaptive-transport/tests/cc_port_sim.rs`) ile netem arasındaki "gerçek yığın" boşluğunu kapatır.
//!
//! Gerçek zamanlıdır (belirlenimli değil): kayıp deseni tohumla sabit, zamanlama değil. Kapı
//! eşikleri bu yüzden geniştir. `CC_SHAPED=1` ile koşar (ölçüm); `CC_SHAPED_TRACE=1` rölede her
//! sunulan paketi yazar (an, yön, tel baytı, kuyruk doluluğu, düşme) — paket boyutundan türü
//! okunur (ör. 25 B/s hücresinde 120 B = 65 B ileti, 79 B = PARAM, 1080 B = FEC parite);
//! `CC_SHAPED_ENGINE=legacy|cc-port`
//! tek motor; `CC_SHAPED_SEED`, `CC_SHAPED_UNITS`, `CC_SHAPED_RATE`, `CC_SHAPED_Q`,
//! `CC_SHAPED_DELAY_MS`, `CC_SHAPED_JITTER_MS` (normal σ; yeniden sıralama üretir), `CC_SHAPED_LOSS_PM`,
//! `CC_SHAPED_UNIT_BYTES`, `CC_SHAPED_TMAX_S`, `CC_SHAPED_INTERVAL_MS` (ileti hücresi: birimler bu aralıkla
//! sunulur; 0 = hepsi başta) hücre anahtarları.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint, TransportEngine};
use client_core::seam::IdentitySeam;
use tokio::net::UdpSocket;

const DATAGRAM_MAX: usize = 2048;
const TICK: Duration = Duration::from_millis(1);
/// UDP/IP zarfı (serileştirme hesabına girer; röle yalnız UDP gövdesini görür).
const UDP_IP_OVERHEAD: u64 = 28;

/// Bir yönün biçimlendiricisi: serileştirme + sınırlı kuyruk + gecikme + rastgele kayıp.
struct Shaper {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    /// Gecikme jitter'ı (normal dağılım σ; netem `delay 300ms 60ms` gibi). Varışlar varış anına
    /// göre sıralanır → jitter yeniden sıralama üretir (netem'deki gibi).
    jitter: Duration,
    loss_permille: u64,
    lcg: u64,
    /// Serileştirmesi süren paketlerin bitiş anları (kuyruk doluluğu).
    busy: VecDeque<Instant>,
    /// Varış anı sırasıyla bekleyenler.
    wire: VecDeque<(Instant, Vec<u8>)>,
    sent: u64,
    bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
    /// `CC_SHAPED_TRACE=1`: her sunulan paket (an, yön, bayt, kuyruk, sonuç) stdout'a.
    trace: Option<(&'static str, Instant)>,
}

impl Shaper {
    fn new(
        rate_bps: u64,
        limit: usize,
        delay: Duration,
        jitter: Duration,
        loss_permille: u64,
        seed: u64,
    ) -> Self {
        Self {
            rate_bps,
            limit,
            delay,
            jitter,
            loss_permille,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            sent: 0,
            bytes: 0,
            dropped_queue: 0,
            dropped_random: 0,
            trace: None,
        }
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    /// [0, 1) belirlenimli.
    fn unit(&mut self) -> f64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.lcg >> 11) as f64) / ((1u64 << 53) as f64)
    }

    /// Jitter'lı gecikme: `delay + N(0, jitter)` (Box-Muller), alt sınır 0.
    fn delay_sample(&mut self) -> Duration {
        if self.jitter.is_zero() {
            return self.delay;
        }
        let u1 = self.unit().max(1e-12);
        let u2 = self.unit();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let d = self.delay.as_secs_f64() + z * self.jitter.as_secs_f64();
        Duration::from_secs_f64(d.max(0.0))
    }

    fn trace_line(&self, now: Instant, size: u64, what: &str) {
        if let Some((tag, t0)) = self.trace {
            println!(
                "İZ t={:.2}s {tag} bayt={size} kuyruk={} {what}",
                now.duration_since(t0).as_secs_f64(),
                self.busy.len()
            );
        }
    }

    fn offer(&mut self, now: Instant, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        let size = pkt.len() as u64 + UDP_IP_OVERHEAD;
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            self.trace_line(now, size, "KUYRUK_DÜŞTÜ");
            return;
        }
        self.trace_line(now, size, "");
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let ser = Duration::from_nanos(size * 1_000_000_000 / self.rate_bps.max(1));
        let done = start + ser;
        self.busy.push_back(done);
        self.sent += 1;
        self.bytes += size;
        if self.loss_permille > 0 && self.rand_permille() < self.loss_permille {
            self.dropped_random += 1;
            self.trace_line(now, size, "YOLDA_DÜŞTÜ");
            return;
        }
        let at = done + self.delay_sample();
        // Varış anına göre sıralı ekle (jitter yeniden sıralama üretebilir).
        let pos = self
            .wire
            .iter()
            .rposition(|(t, _)| *t <= at)
            .map_or(0, |i| i + 1);
        self.wire.insert(pos, (at, pkt));
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

#[derive(Debug, Clone, Copy)]
struct Dir {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    jitter: Duration,
    loss_permille: u64,
}

struct RelayStats {
    fwd_sent: u64,
    fwd_bytes: u64,
    fwd_dropped_queue: u64,
    fwd_dropped_random: u64,
    rev_sent: u64,
    rev_bytes: u64,
    rev_dropped_queue: u64,
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

struct Outcome {
    /// FEC sayaçları (gönderici, alıcı) — kalıcı FEC kapısı okur.
    fec: Option<(
        client_core::runtime::ClientStats,
        client_core::runtime::ServerStats,
    )>,
    completed: Option<Duration>,
    delivered: u64,
    stats: RelayStats,
}

#[allow(clippy::too_many_arguments)]
async fn run_cell(
    engine: TransportEngine,
    fwd_cfg: Dir,
    rev_cfg: Dir,
    seed: u64,
    units: u64,
    unit_bytes: usize,
    t_max: Duration,
    interval: Duration,
) -> Outcome {
    let mut cfg = RuntimeConfig {
        engine,
        ..RuntimeConfig::default()
    };
    // `CC_SHAPED_FEC=off` → FEC profili yok (A/B kolu); `=prior0` → profil var, kanal ön-oranı yok.
    match std::env::var("CC_SHAPED_FEC").as_deref() {
        Ok("off") => cfg.fec = None,
        Ok("prior0") => {
            if let Some(f) = cfg.fec.as_mut() {
                f.channel_loss_permille = None;
            }
        }
        _ => {}
    }
    let server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
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
    let fwd = Arc::new(Mutex::new(Shaper::new(
        fwd_cfg.rate_bps,
        fwd_cfg.limit,
        fwd_cfg.delay,
        fwd_cfg.jitter,
        fwd_cfg.loss_permille,
        seed,
    )));
    let rev = Arc::new(Mutex::new(Shaper::new(
        rev_cfg.rate_bps,
        rev_cfg.limit,
        rev_cfg.delay,
        rev_cfg.jitter,
        rev_cfg.loss_permille,
        seed ^ 0x5A5A,
    )));
    if std::env::var("CC_SHAPED_TRACE").is_ok() {
        let t0 = Instant::now();
        fwd.lock().expect("kilit").trace = Some(("İLERİ", t0));
        rev.lock().expect("kilit").trace = Some(("DÖNÜŞ", t0));
    }
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
    let mut offered = 0u64;
    let mut offer_at: Vec<Instant> = Vec::with_capacity(units as usize);
    let mut offer_one = |client: &client_core::runtime::ClientHandle,
                         offer_at: &mut Vec<Instant>| {
        let mut payload = vec![0x5Au8; unit_bytes];
        payload[..8].copy_from_slice(&offered.to_be_bytes());
        client.send(payload).expect("offer");
        offer_at.push(Instant::now());
        offered += 1;
    };
    if interval.is_zero() {
        for _ in 0..units {
            offer_one(&client, &mut offer_at);
        }
    } else {
        offer_one(&client, &mut offer_at);
    }
    let mut delivered = 0u64;
    let mut delays: Vec<Duration> = Vec::new();
    let mut marks: [Option<Duration>; 3] = [None; 3];
    let completed = tokio::time::timeout(t_max, async {
        let mut next_offer = tokio::time::interval(if interval.is_zero() {
            Duration::from_secs(3600)
        } else {
            interval
        });
        next_offer.tick().await;
        while delivered < units {
            let d = tokio::select! {
                d = server.next_delivery() => d,
                _ = next_offer.tick(), if !interval.is_zero() && offer_at.len() < units as usize => {
                    offer_one(&client, &mut offer_at);
                    continue;
                }
            };
            if let Some(d) = d {
                delivered += 1;
                if let Some(&t) = offer_at.get(d.unit_id as usize) {
                    delays.push(t.elapsed());
                }
                for (i, pct) in [10u64, 50, 90].iter().enumerate() {
                    if marks[i].is_none() && delivered * 100 >= units * pct {
                        marks[i] = Some(t0.elapsed());
                    }
                }
            } else {
                break;
            }
        }
        t0.elapsed()
    })
    .await
    .ok();
    let f = |d: Option<Duration>| d.map_or("-".to_string(), |d| format!("{:.1}", d.as_secs_f64()));
    delays.sort_unstable();
    let p = |q: usize| {
        delays
            .get((delays.len() * q / 100).min(delays.len().saturating_sub(1)))
            .map(|d| format!("{:.2}", d.as_secs_f64()))
            .unwrap_or("-".to_string())
    };
    println!(
        "[{engine:?}] PROFİL: %10={} %50={} %90={} son={} birim_gecikme p50={} p95={}",
        f(marks[0]),
        f(marks[1]),
        f(marks[2]),
        f(completed),
        p(50),
        p(95)
    );
    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    if std::env::var("CC_SHAPED_FEC_DIAG").is_ok() {
        // FEC karar yolu teşhisi (yalnız elle koşular): gate/arbiter/latch sayaçları + alıcı FEC.
        println!(
            "[{engine:?}] FEC-TEŞHİS: durum={} kuruldu={} rtt_örnek={} ledger_loss={} tepe={} fate(n={} lost={} n00={} n01={} n10={} n11={} delta={}) gate_opened={} gate_outcomes={:?} arbiter={:?} spec_levels={:?} preconv_blocked={} latch={:?} son_bastırma={:?} blok_latched={} repair_tx={} fec_bytes={} completeness={:?} aktivasyon(fec_cfg={} parity_emit={} parity_denied={}) | alıcı: repairs_rx={} recovered={} decoded={} evicted={}",
            cs.fate.sender_state, cs.fate.left_preconverge, cs.fate.rtt_samples,
            cs.fate.ledger_loss_ppm, cs.fate.ledger_loss_ppm_peak,
            cs.fate.n, cs.fate.lost, cs.fate.n00, cs.fate.n01, cs.fate.n10, cs.fate.n11, cs.fate.burst_delta_ppm,
            cs.fate.structure_gate_opened, cs.fate.gate_outcomes, cs.fate.fec_arbiter_reasons, cs.fate.fec_spec_levels,
            cs.fate.preconverge_parity_blocked, cs.fate.fec_latch_diag, cs.fate.fec_last_suppression,
            cs.fec_blocks_latched, cs.fec_repairs_tx, cs.tx_fec_bytes, cs.fate.fec_completeness,
            cs.fate.activation.fec_configured, cs.fate.activation.parity_emitted, cs.fate.activation.parity_token_denied,
            ss.fec_repairs_rx, ss.fec_recovered_units, ss.fec_blocks_decoded, ss.fec_blocks_evicted,
        );
        println!(
            "[{engine:?}] FEC-ALICI: repairs_useful={} wasted_complete={} wasted_evicted={} complete_with_repairs={} decode_failures={} recovered_not_admitted={} recovered_ctrl_tx={} pre_param_dropped={}",
            ss.fec_repairs_useful, ss.fec_repairs_wasted_complete, ss.fec_repairs_wasted_evicted,
            ss.fec_blocks_complete_with_repairs, ss.fec_decode_failures, ss.fec_recovered_not_admitted,
            ss.fec_recovered_ctrl_tx, ss.fec_pre_param_dropped,
        );
    }
    let _ = stop_tx.send(());
    let _ = relay_task.await;
    let f = fwd.lock().expect("kilit");
    let r = rev.lock().expect("kilit");
    Outcome {
        fec: Some((cs, ss)),
        completed,
        delivered,
        stats: RelayStats {
            fwd_sent: f.sent,
            fwd_bytes: f.bytes,
            fwd_dropped_queue: f.dropped_queue,
            fwd_dropped_random: f.dropped_random,
            rev_sent: r.sent,
            rev_bytes: r.bytes,
            rev_dropped_queue: r.dropped_queue,
        },
    }
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Ölçüm hücresi (yalnız `CC_SHAPED=1`): varsayılan 12,5 kB/s · 6 paket kuyruk · 300 ms tek yön ·
/// %30 kayıp (manşet yaklaşığı, p1000) · 150 × 1000 B (150 kB), dönüş yolu aynı hız/gecikme, kayıpsız.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shaped_relay_measure() {
    if std::env::var("CC_SHAPED").is_err() {
        return;
    }
    let rate = env_u64("CC_SHAPED_RATE", 12_500);
    let q = env_u64("CC_SHAPED_Q", 6) as usize;
    let delay = Duration::from_millis(env_u64("CC_SHAPED_DELAY_MS", 300));
    let jitter = Duration::from_millis(env_u64("CC_SHAPED_JITTER_MS", 0));
    let loss = env_u64("CC_SHAPED_LOSS_PM", 300);
    let units = env_u64("CC_SHAPED_UNITS", 150);
    let unit_bytes = env_u64("CC_SHAPED_UNIT_BYTES", 1000) as usize;
    let seed = env_u64("CC_SHAPED_SEED", 1);
    let engines: Vec<TransportEngine> = match std::env::var("CC_SHAPED_ENGINE").as_deref() {
        Ok("legacy") => vec![TransportEngine::Legacy],
        Ok("cc-port") => vec![TransportEngine::CcPort],
        _ => vec![TransportEngine::Legacy, TransportEngine::CcPort],
    };
    for eng in engines {
        let o = run_cell(
            eng,
            Dir {
                rate_bps: rate,
                limit: q,
                delay,
                jitter,
                loss_permille: loss,
            },
            Dir {
                rate_bps: env_u64("CC_SHAPED_REV_RATE", rate),
                limit: q.max(64),
                delay,
                jitter,
                loss_permille: env_u64("CC_SHAPED_REV_LOSS_PM", 0),
            },
            seed,
            units,
            unit_bytes,
            Duration::from_secs(env_u64("CC_SHAPED_TMAX_S", 180)),
            Duration::from_millis(env_u64("CC_SHAPED_INTERVAL_MS", 0)),
        )
        .await;
        println!(
            "[{eng:?}] shaped r{rate} q{q} d{}±{}ms l{loss}‰ u{units}×{unit_bytes} seed{seed}: tamamlanma={} teslim={}/{units} ileri={} pkt / {} B (kuyruk_düşen={} rastgele_düşen={}) dönüş={} pkt / {} B (düşen={})",
            delay.as_millis(),
            jitter.as_millis(),
            o.completed.map_or("YOK".to_string(), |d| format!("{:.2} s", d.as_secs_f64())),
            o.delivered,
            o.stats.fwd_sent,
            o.stats.fwd_bytes,
            o.stats.fwd_dropped_queue,
            o.stats.fwd_dropped_random,
            o.stats.rev_sent,
            o.stats.rev_bytes,
            o.stats.rev_dropped_queue,
        );
    }
}

/// [FEC-PRIOR + ERTELENMİŞ LATCH + FEC-SIRA] Kalıcı kapı: kanal profili verilmiş ürün
/// yapılandırmasında (KamaV1: `channel_loss_permille = Some(200)`) FEC **ilk bloklardan itibaren**
/// parite üretir ve kayıpta kurtarır. Hücre: ileri 12,5 kB/s · 300 ms · %25 rastgele kayıp;
/// dönüş yolu 2 s gecikmeli (ARQ turu ≥ 2,3 s) → onarım sembolü ARQ'dan önce varır, kurtarma
/// belirlenimli olarak FEC'e düşer. 64 × 1000 B toplu sunum (dosya örüntüsü — eski kodda bütün
/// bloklar t=0'da r=0 ile kapanıyordu).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fec_profile_prior_protects_first_blocks_and_recovers_loss() {
    let units = 64u64;
    let o = run_cell(
        TransportEngine::CcPort,
        Dir {
            rate_bps: 12_500,
            limit: 6,
            delay: Duration::from_millis(300),
            jitter: Duration::ZERO,
            loss_permille: 250,
        },
        Dir {
            rate_bps: 12_500,
            limit: 64,
            delay: Duration::from_millis(2000),
            jitter: Duration::ZERO,
            loss_permille: 0,
        },
        7,
        units,
        1000,
        Duration::from_secs(90),
        Duration::ZERO,
    )
    .await;
    assert_eq!(o.delivered, units, "teslim {}/{units}", o.delivered);
    assert!(o.completed.is_some(), "90 s içinde tamamlanmadı");
    let (cs, ss) = o.fec.expect("FEC sayaçları");
    assert!(
        cs.fate.fec_prior_applied >= 1,
        "kanal ön-oranı hiçbir blokta uygulanmadı: {:?}",
        cs.fate
    );
    assert!(
        cs.fec_blocks_latched >= 6,
        "latch'lenen blok {} < 6 (ertelenmiş latch çalışmadı?)",
        cs.fec_blocks_latched
    );
    assert!(
        cs.fec_repairs_tx >= 6,
        "tele çıkan onarım {} < 6",
        cs.fec_repairs_tx
    );
    assert!(ss.fec_repairs_rx >= 1, "alıcıya onarım ulaşmadı");
    assert!(
        ss.fec_recovered_units >= 1,
        "kayıpta FEC kurtarması yok (useful={} wasted_complete={} decoded={})",
        ss.fec_repairs_useful,
        ss.fec_repairs_wasted_complete,
        ss.fec_blocks_decoded
    );
}
