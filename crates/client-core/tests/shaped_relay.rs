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
//! `CC_SHAPED_UNIT_BYTES`, `CC_SHAPED_REV_RATE` (dönüş hızı), `CC_SHAPED_IW`, `CC_SHAPED_TMAX_S`,
//! `CC_SHAPED_INTERVAL_MS` (ileti hücresi: birimler bu aralıkla sunulur; 0 = hepsi başta) hücre
//! anahtarları. Yön hızları (dağıtım profili): `CC_SHAPED_LINK=1` iki uca verir (alıcıda ACK
//! bütçesi, göndericide yol ilk penceresi ve profil pacer tavanı);
//! `CC_SHAPED_LINK_SIDE=server|client` yalnız bir uca; `CC_SHAPED_LINK_FWD` / `CC_SHAPED_LINK_REV`
//! profilin söylediği ileri/dönüş hızı (B/s; varsayılan hattın gerçek hızı — profil ≠ gerçek
//! hücresi için).

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

/// [ASİMETRİK-YOL] Hücrenin dağıtım profili: uçlara verilen yön hızları. Profil gerçek hattan
/// farklı olabilir (`fwd_bps`/`rev_bps` ≠ biçimlendirici hızları) ve yalnız bir uca verilebilir.
#[derive(Debug, Clone, Copy)]
struct LinkProfile {
    /// Profilin söylediği ileri yön (istemci → sunucu) hızı, B/s.
    fwd_bps: u64,
    /// Profilin söylediği dönüş yönü (sunucu → istemci) hızı, B/s.
    rev_bps: u64,
    /// Alıcıya (sunucu) verilir: ACK dönüş bütçesi.
    server: bool,
    /// Göndericiye (istemci) verilir: yol ilk penceresi.
    client: bool,
}

impl LinkProfile {
    /// İki uca, gerçek hat hızlarıyla.
    fn exact(fwd: Dir, rev: Dir) -> Self {
        Self {
            fwd_bps: fwd.rate_bps,
            rev_bps: rev.rate_bps,
            server: true,
            client: true,
        }
    }
}

struct Outcome {
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
    link: Option<LinkProfile>,
) -> Outcome {
    let cfg = RuntimeConfig {
        engine,
        // `CC_SHAPED_LIVE=1`: gönderici canlı aynası (salt gözlem) — saniyede bir iz satırı.
        live_diag: std::env::var("CC_SHAPED_LIVE").is_ok(),
        // `CC_SHAPED_IW=<B>`: ilk pencere (keşif).
        initial_window_bytes: std::env::var("CC_SHAPED_IW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(client_core::runtime::KamaV1Profile::INITIAL_WINDOW_BYTES),
        ..RuntimeConfig::default()
    };
    // [ASİMETRİK-YOL] Yön hızları her uca kendi bakış açısından — alıcı (sunucu) yukarı = dönüş,
    // aşağı = ileri; gönderici (istemci) yukarı = ileri, aşağı = dönüş.
    let server_cfg = RuntimeConfig {
        link_rates: link
            .filter(|l| l.server)
            .map(|l| client_core::runtime::LinkRates {
                uplink_bps: l.rev_bps,
                downlink_bps: l.fwd_bps,
            }),
        ..cfg.clone()
    };
    let cfg = RuntimeConfig {
        link_rates: link
            .filter(|l| l.client)
            .map(|l| client_core::runtime::LinkRates {
                uplink_bps: l.fwd_bps,
                downlink_bps: l.rev_bps,
            }),
        ..cfg
    };
    let server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        server_cfg,
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
        let mut live_tick = tokio::time::interval(Duration::from_secs(1));
        let live_on = std::env::var("CC_SHAPED_LIVE").is_ok();
        while delivered < units {
            let d = tokio::select! {
                d = server.next_delivery() => d,
                _ = live_tick.tick(), if live_on => {
                    let l = client.live_diag();
                    let c = l.control;
                    println!(
                        "CANLI t={:>5.1}s teslim={delivered} durum={:?} btlbw={} B/s pacer={} B/s uçuş={} B srtt={} ms rtprop={} ms rtt_örnek={} retx={} prob={} ack_çerçeve={}",
                        t0.elapsed().as_secs_f64(),
                        c.state,
                        c.btlbw_bps,
                        c.pacer_rate_bps,
                        c.inflight_bytes,
                        l.srtt_us / 1000,
                        l.rtprop_us / 1000,
                        l.rtt_samples,
                        l.retransmits,
                        l.tail_probes,
                        l.ack.frames,
                    );
                    continue;
                }
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
    let _ = client.shutdown().await;
    let _ = server.shutdown().await;
    let _ = stop_tx.send(());
    let _ = relay_task.await;
    let f = fwd.lock().expect("kilit");
    let r = rev.lock().expect("kilit");
    Outcome {
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
    let fwd = Dir {
        rate_bps: rate,
        limit: q,
        delay,
        jitter,
        loss_permille: loss,
    };
    let rev = Dir {
        // `CC_SHAPED_REV_RATE=<B/s>`: dönüş yolu hızı (asimetrik yol; varsayılan ileri hız).
        rate_bps: env_u64("CC_SHAPED_REV_RATE", rate),
        limit: q.max(64),
        delay,
        jitter,
        loss_permille: 0,
    };
    let side = std::env::var("CC_SHAPED_LINK_SIDE").ok();
    let link = (std::env::var("CC_SHAPED_LINK").is_ok() || side.is_some()).then(|| LinkProfile {
        fwd_bps: env_u64("CC_SHAPED_LINK_FWD", fwd.rate_bps),
        rev_bps: env_u64("CC_SHAPED_LINK_REV", rev.rate_bps),
        server: side.as_deref() != Some("client"),
        client: side.as_deref() != Some("server"),
    });
    for eng in engines {
        let o = run_cell(
            eng,
            fwd,
            rev,
            seed,
            units,
            unit_bytes,
            Duration::from_secs(env_u64("CC_SHAPED_TMAX_S", 180)),
            Duration::from_millis(env_u64("CC_SHAPED_INTERVAL_MS", 0)),
            link,
        )
        .await;
        println!(
            "[{eng:?}] shaped r{rate} rr{} q{q} d{}±{}ms l{loss}‰ u{units}×{unit_bytes} seed{seed} profil={}: tamamlanma={} teslim={}/{units} ileri={} pkt / {} B (kuyruk_düşen={} rastgele_düşen={}) dönüş={} pkt / {} B (düşen={})",
            rev.rate_bps,
            delay.as_millis(),
            jitter.as_millis(),
            link.map_or("kapalı".to_string(), |l| format!(
                "{}{}:{}/{}",
                if l.server { "S" } else { "" },
                if l.client { "C" } else { "" },
                l.fwd_bps,
                l.rev_bps
            )),
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

/// [ASİMETRİK-YOL] Kalıcı kapı — saha koşulu: ileri 12,5 kB/s, dönüş 62 B/s (yukarı yön ≈ 0,5 kbit/s),
/// tek yön 300 ms, %20 ileri kayıp, 40 × 1000 B. Yön hızları verilmezse port motoru bu hücrede 57–62 s
/// sürüyor ve dönüşe 3,6–4,2 kB ACK basıyor: ACK'ler 62 B/s hatta sıra bekliyor, gönderici ACK-saatli
/// döngüde yakınsamıyor. Yön hızlarıyla (alıcı ACK bütçesi + yol ilk penceresi + profil pacer
/// tavanı; PARAM geri çekilmesi tabanında) ölçüm: tohum 1–2 15,3–19,0 s, dönüş 565–693 B; bu kapı
/// (tohum 1) 3 koşu 14,9–19,0 s, 630–693 B. Eşikler (≤ 35 s, ≤ 1 500 B) gerçek zaman gürültüsüne
/// geniş; yön hızları olmadan hücre iki eşiği de aşar.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn asymmetric_return_path_with_link_rates_bounds_acks_and_completes() {
    let fwd = Dir {
        rate_bps: 12_500,
        limit: 64,
        delay: Duration::from_millis(300),
        jitter: Duration::ZERO,
        loss_permille: 200,
    };
    let rev = Dir {
        rate_bps: 62,
        limit: 64,
        delay: Duration::from_millis(300),
        jitter: Duration::ZERO,
        loss_permille: 0,
    };
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        40,
        1000,
        Duration::from_secs(120),
        Duration::ZERO,
        Some(LinkProfile::exact(fwd, rev)),
    )
    .await;
    report("B", &o);
    let done = o
        .completed
        .expect("asimetrik hücre 120 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 40, "40/40 teslim");
    assert!(
        done <= Duration::from_secs(35),
        "yön hızlarıyla tamamlanma {done:?} > 35 s (yön hızları olmadan 57–62 s)"
    );
    assert!(
        o.stats.rev_bytes <= 1_500,
        "dönüşe {} B > 1500 B (bütçe ihlali; yön hızları olmadan 3,6–4,2 kB)",
        o.stats.rev_bytes
    );
}

/// Kapı özeti (CI günlüğünde eşiklerin yanında görünür).
fn report(tag: &str, o: &Outcome) {
    println!(
        "KAPI {tag}: tamamlanma={} teslim={} ileri_kuyruk_düşen={} ileri_rastgele_düşen={} dönüş={} pkt / {} B",
        o.completed
            .map_or("YOK".to_string(), |d| format!("{:.2} s", d.as_secs_f64())),
        o.delivered,
        o.stats.fwd_dropped_queue,
        o.stats.fwd_dropped_random,
        o.stats.rev_sent,
        o.stats.rev_bytes
    );
}

/// Tek yön 300 ms, jitter'sız yön.
fn dir300(rate_bps: u64, limit: usize, loss_permille: u64) -> Dir {
    Dir {
        rate_bps,
        limit,
        delay: Duration::from_millis(300),
        jitter: Duration::ZERO,
        loss_permille,
    }
}

/// [ASİMETRİK-YOL] Kalıcı kapı — doğru profil, sığ kuyruk: ileri 12,5 kB/s (kuyruk 16 paket), dönüş
/// 62 B/s, %20 ileri kayıp, 40 × 1000 B, profil iki uçta ve hatla aynı. Profil ilk pencereyi
/// ≈ 67 kB'ye büyütür; pencere profil hızıyla serileşir (pacer tavanı). Tavansız pacer ilk RTT
/// örneğinden önce ≈ 840 kB/s ile çalışıyor ve 40 birimi 50 ms'de 16 paketlik kuyruğa boşaltıyordu:
/// tohum 1/2 37,5/42,3 s, ileri kuyruk düşmesi 28/33 (düşenler uçuşun sonu, yalnız PTO ile
/// seziliyordu). Tavanla tohum 1/2 14,9/19,5 s, düşme 0/1; bu kapı (tohum 1) 3 koşu 19,0–20,3 s,
/// düşme 0–1. Eşik: düşme ≤ 8 (asıl ayırıcı), tamamlanma ≤ 35 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn link_rates_initial_window_is_paced_into_a_shallow_queue() {
    let fwd = dir300(12_500, 16, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        40,
        1000,
        Duration::from_secs(120),
        Duration::ZERO,
        Some(LinkProfile::exact(fwd, rev)),
    )
    .await;
    report("C0", &o);
    let done = o
        .completed
        .expect("sığ kuyruk hücresi 120 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 40, "40/40 teslim");
    assert!(
        o.stats.fwd_dropped_queue <= 8,
        "ileri kuyrukta {} paket düştü > 8 (ilk pencere profil hızıyla serileşmiyor; tavansız 28–33)",
        o.stats.fwd_dropped_queue
    );
    assert!(
        done <= Duration::from_secs(35),
        "tamamlanma {done:?} > 35 s (tavansız 37–42 s)"
    );
}

/// [ASİMETRİK-YOL] Kalıcı kapı — fazla iyimser profil, sığ kuyruk: ileri gerçek 2,5 kB/s (kuyruk 8
/// paket), profil 12,5 kB/s (5 kat); dönüş 62 B/s, %20 ileri kayıp, 24 × 1000 B. Profil ilk
/// pencereyi gerçeğin 5 katına büyütür; ilk ACK'ten önce bunu bilecek ölçüm yoktur, fazlası
/// kuyrukta düşer (16–17 paket). Teslim çökmemeli. Ölçüm (tohum 1, 3 koşu): 19,6–26,4 s, 24/24;
/// aynı hatta yön hızları kapalı kol 33,9 s (3 koşu). Eşik 45 s (kapalı kolun ≈ 1,3 katı).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overoptimistic_link_profile_on_shallow_queue_still_completes() {
    let fwd = dir300(2_500, 8, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        24,
        1000,
        Duration::from_secs(150),
        Duration::ZERO,
        Some(LinkProfile {
            fwd_bps: 12_500,
            ..LinkProfile::exact(fwd, rev)
        }),
    )
    .await;
    report("C", &o);
    let done = o
        .completed
        .expect("fazla iyimser profil hücresi 150 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 24, "24/24 teslim");
    assert!(
        done <= Duration::from_secs(45),
        "fazla iyimser profil: tamamlanma {done:?} > 45 s (yön hızları kapalı kol 33,9 s)"
    );
}

/// [ASİMETRİK-YOL] Kalıcı kapı — profil yalnız alıcıda: ileri 12,5 kB/s, dönüş 62 B/s, tek yön
/// 300 ms, %20 ileri kayıp, 20 × 1000 B. Alıcı ACK'leri bütçeyle seyreltir, göndericinin ilk
/// penceresi büyümez (4,8 kB): ACK'ler saniyeler arayla gelir, gönderici küçük pencereyle ACK
/// bekler. Kalıcı bekleme olmamalı. Ölçüm (tohum 1, 3 koşu): 21,5–25,5 s, dönüş 574–706 B; yön
/// hızları kapalı aynı hücre 24,8–25,4 s, 1,6–1,7 kB (2 koşu). Eşik 60 s (ilerleyen taşımaya iki
/// kattan fazla pay; kalıcı beklemede 150 s'de zaman aşımı).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn link_rates_on_receiver_only_completes() {
    let fwd = dir300(12_500, 64, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        20,
        1000,
        Duration::from_secs(150),
        Duration::ZERO,
        Some(LinkProfile {
            client: false,
            ..LinkProfile::exact(fwd, rev)
        }),
    )
    .await;
    report("D-alıcı", &o);
    let done = o
        .completed
        .expect("yalnız alıcıda profil: 150 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 20, "20/20 teslim");
    assert!(
        done <= Duration::from_secs(60),
        "yalnız alıcıda profil: tamamlanma {done:?} > 60 s"
    );
}

/// [ASİMETRİK-YOL] Kalıcı kapı — profil yalnız göndericide: aynı hücre. İlk pencere büyür
/// (≈ 67 kB, profil hızıyla serileşir), alıcı bütçesiz quinn kuralıyla ACK'ler: ACK'ler 62 B/s
/// dönüş kuyruğunda birikir, RTT örnekleri şişer. Kalıcı bekleme olmamalı. Ölçüm (tohum 1, 3 koşu):
/// 12,5–26,8 s, dönüş 1 717–1 838 B (bütçesiz). Eşik 60 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn link_rates_on_sender_only_completes() {
    let fwd = dir300(12_500, 64, 200);
    let rev = dir300(62, 64, 0);
    let o = run_cell(
        TransportEngine::CcPort,
        fwd,
        rev,
        1,
        20,
        1000,
        Duration::from_secs(150),
        Duration::ZERO,
        Some(LinkProfile {
            server: false,
            ..LinkProfile::exact(fwd, rev)
        }),
    )
    .await;
    report("D-gönderici", &o);
    let done = o
        .completed
        .expect("yalnız göndericide profil: 150 s içinde tamamlanmalı");
    assert_eq!(o.delivered, 20, "20/20 teslim");
    assert!(
        done <= Duration::from_secs(60),
        "yalnız göndericide profil: tamamlanma {done:?} > 60 s"
    );
}
