//! CC-PORT sim kapısı (ISEMRI-CC-PORT §5 Ö-CC "6 paketlik kuyruk" hücresinin ayrık-olay
//! karşılığı). netem DEĞİL; deterministik (seed'li LCG, ADR-17 motor dışı), gerçek `Sender`
//! + gerçek `Receiver` + sınırlı FIFO darboğaz.
//!
//! İki motor AYNI süreçte, AYNI seed'lerle, eşli koşar (`Engine::Legacy` = `Sender::new()`,
//! `Engine::CcPort` = `Sender::with_cc_port()` + `Receiver::with_cc_port()`); her satır motoru
//! yazar:
//!
//! ```text
//! cargo test -p adaptive-transport --test cc_port_sim -- --nocapture
//! CC_MATRIX=1 cargo test -p adaptive-transport --test cc_port_sim matrix_explore -- --nocapture
//! CC_TRACE=1 CC_SEED=10 CC_LOSS=200 cargo test ... shallow_queue_file_no_loss -- --nocapture
//! ```
//!
//! Kabul (yalnız port kolu için bağlayıcı; legacy kolu rapor eder, kırmızı üretmez):
//! * 6 paketlik kuyruk, 12,5 kB/s, 300 ms RTT, kayıp %0: 150 kB dosya ≤ 25 s (≈1,5 ×
//!   quinn-bbr-rfc'nin netem'de ölçülen 16,4 s'i; `reports/yol3/2026-11-10`).
//! * Aynı hücre, kayıp %20, 10 seed: 10/10 teslim, p50 ≤ 60 s.
//! * Derin kuyruk (1000 paket), kayıp %20, 10 seed: 10/10 teslim.
//! * PTT demeti (6 × 400 B, %20 kayıp): ilk demet ≤ 3 s.
//! * FEC: blok başına 2 parite kuyruğa girer, ≥ %90'ı tele çıkar (eski motorda kova tavanı atar).
//!
//! Bu bir ÖLÇÜM değil, KAPIDIR: sayılar netem ile doğrulanmadan "geçti" sayılmaz; portun
//! motora doğru bağlandığını ve sığ kuyrukta çökmediğini pinler.

use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, FecActuationMode, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind,
};
use std::collections::{HashMap, HashSet, VecDeque};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const FILE_BYTES: u64 = 150_000;

/// Hangi motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Legacy,
    CcPort,
}

impl Engine {
    fn name(self) -> &'static str {
        match self {
            Engine::Legacy => "legacy (faz makinesi)",
            Engine::CcPort => "cc_port (quinn BBR + RFC 9002)",
        }
    }
    fn sender(self) -> Sender {
        match self {
            Engine::Legacy => Sender::new(),
            Engine::CcPort => Sender::with_cc_port(),
        }
    }
    fn receiver(self) -> Receiver {
        match self {
            Engine::Legacy => Receiver::new(),
            Engine::CcPort => Receiver::with_cc_port(),
        }
    }
}
const ENGINES: [Engine; 2] = [Engine::Legacy, Engine::CcPort];

/// Sınırlı FIFO darboğaz: `rate_bps` serileştirme, `limit` paket kuyruk, `delay_ns` yayılım.
struct Bottleneck {
    rate_bps: u64,
    limit: usize,
    delay_ns: u64,
    /// Serileştirmesi bitmemiş paketlerin bitiş anları (kuyruk doluluğu bunlardan sayılır).
    busy: VecDeque<u64>,
    /// Alıcıya varacaklar: (varış, seq, unit_id, kind).
    wire: VecDeque<(u64, u64, u64, TransmitKind)>,
    dropped_queue: u64,
    dropped_random: u64,
    lcg: u64,
    loss_permille: u64,
    /// Kesinti penceresi (ns): bu aralıkta ileri yönde %100 kayıp (gösterim "kesinti" profili).
    blackout: Option<(u64, u64)>,
}

impl Bottleneck {
    fn new(rate_bps: u64, limit: usize, delay_ns: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            limit,
            delay_ns,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            dropped_queue: 0,
            dropped_random: 0,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            loss_permille,
            blackout: None,
        }
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    fn send(&mut self, now: u64, seq: u64, size: u64, unit_id: u64, kind: TransmitKind) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + size * SEC / self.rate_bps;
        self.busy.push_back(done);
        if self.blackout.is_some_and(|(a, b)| (a..b).contains(&now)) {
            self.dropped_random += 1;
            return;
        }
        if self.loss_permille > 0 && self.rand_permille() < self.loss_permille {
            self.dropped_random += 1;
            return; // serileştirildi ama yolda düştü
        }
        self.wire
            .push_back((done + self.delay_ns, seq, unit_id, kind));
    }

    fn poll(&mut self, now: u64) -> Vec<(u64, u64, TransmitKind)> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, kind) = self.wire.pop_front().unwrap();
            out.push((seq, uid, kind));
        }
        out
    }
}

/// Dönüş yolu (SACK) darboğazı: `rate_bps` serileştirme (0 = sınırsız), `limit` paket kuyruk,
/// `delay_ns` yayılım. ACK baytı = kodlanmış çerçeve + `ACK_WIRE_OVERHEAD`. Düşük bant hücrelerinde
/// (r25: 25 B/s iki yön) ACK'lerin dönüş hattını doldurması ancak bununla görünür.
struct RevPath {
    rate_bps: u64,
    limit: usize,
    delay_ns: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, AckInfo)>,
    dropped_queue: u64,
    bytes: u64,
}

/// SACK çerçevesinin tel zarfı (Noise mühürlü başlık + UDP/IP) için yaklaşık ek — ürün ölçümü
/// ~65 B toplam ACK; kodlanmış çerçeve ~17 B.
const ACK_WIRE_OVERHEAD: u64 = 48;

impl RevPath {
    fn new(rate_bps: u64, limit: usize, delay_ns: u64) -> Self {
        Self {
            rate_bps,
            limit,
            delay_ns,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            dropped_queue: 0,
            bytes: 0,
        }
    }

    /// `size` bayt ACK'i kuyruğa koyar; kuyruk doluysa düşer (SACK birikimli: sonraki ACK kapsar).
    /// Döner: kuyruğa girdi mi.
    fn send(&mut self, now: u64, size: u64, ack: AckInfo) -> bool {
        if self.rate_bps == 0 {
            self.bytes += size;
            self.wire.push_back((now + self.delay_ns, ack));
            return true;
        }
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            return false;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + size * SEC / self.rate_bps;
        self.busy.push_back(done);
        self.bytes += size;
        self.wire.push_back((done + self.delay_ns, ack));
        true
    }
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    completed_ns: Option<u64>,
    delivered_units: usize,
    sent_data: u64,
    sent_retx: u64,
    sent_parity: u64,
    sent_bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
    /// Alıcının ürettiği SACK sayısı (dönüş yolu maliyeti).
    acks: u64,
    /// Dönüş yolunda gönderilen bayt (SACK + zarf).
    rev_bytes: u64,
    /// Dönüş yolu kuyruğunda düşen SACK sayısı.
    rev_dropped_queue: u64,
    /// Birim teslim gecikmesi p50 (sunumdan alıcıya teslime; ileti hücreleri için "ileti p50").
    unit_delay_p50_ns: u64,
    /// Birim teslim gecikmesi p95.
    unit_delay_p95_ns: u64,
}

/// Tek koşu. `units`: (birim sayısı, boyut); `parity_per_block > 0`: her 8 birimlik blok için
/// bu kadar parite kuyruğa girer (manuel FEC otoritesi; motor yalnız pace eder — DR-S5-1).
#[allow(clippy::too_many_arguments)]
#[allow(deprecated)] // legacy-enqueue: pace yolunu ölçer (üretimde otorite-bağlı close; Commit-K)
fn run_opt(
    eng: Engine,
    rate_bps: u64,
    queue_limit: usize,
    rtt_ms: u64,
    loss_permille: u64,
    seed: u64,
    t_max_s: u64,
    units: (u64, u64),
    parity_per_block: u16,
    blackout: Option<(u64, u64)>,
    rev_path: Option<(u64, usize)>,
    app_interval_ms: Option<u64>,
    wire_overhead: Option<u64>,
) -> Outcome {
    let one_way = rtt_ms * MS / 2;
    let mut s = eng.sender();
    // `CC_PTO_MAX_S=<s>`: port kolunda PTO geri çekilme tavanı (varsayılan validated_v1: 30 s).
    if let Some(m) = std::env::var("CC_PTO_MAX_S")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        if eng != Engine::Legacy {
            let mut c = adaptive_transport::tail_recovery::TailRecoveryConfig::rfc9002_cc_port();
            c.max_backoff_pto_ns = m * SEC;
            s.set_tail_recovery_config(c);
        }
    }
    // `CC_IW=<bayt>`: port motorunun ilk uçuş penceresi (varsayılan kurucu: 10·MTU; ürün: 4·MTU).
    if let Some(iw) = std::env::var("CC_IW")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        s.set_cc_initial_window(iw);
    }
    let mut r = eng.receiver();
    if parity_per_block > 0 {
        s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
            .expect("manuel FEC kipi");
    }
    let mut fwd = Bottleneck::new(rate_bps, queue_limit, one_way, loss_permille, seed);
    // Kesinti penceresi `(başlangıç_s, süre_s)`: ileri yönde %100 kayıp (kesinti profili). Testler
    // parametreyle verir (süreç-geneli ortam değişkeni paralel testlere sızar — CI'da görüldü);
    // `CC_BLACKOUT=<başlangıç_s>,<süre_s>` yalnız elle koşular için yedek.
    let blackout = blackout.or_else(|| {
        let v = std::env::var("CC_BLACKOUT").ok()?;
        let (a, d) = v.split_once(',')?;
        Some((a.parse::<u64>().ok()?, d.parse::<u64>().ok()?))
    });
    if let Some((a, d)) = blackout {
        fwd.blackout = Some((a * SEC, (a + d) * SEC));
    }
    // `CC_REV_RATE=<bayt/s>` (0 = sınırsız) ve `CC_REV_Q=<paket>`: dönüş yolu darboğazı. Varsayılan
    // sınırsız (eski davranış, bayt düzeyinde aynı). `rev_path` parametresi önceliklidir.
    let (rev_rate, rev_q) = rev_path.unwrap_or_else(|| {
        let rr = std::env::var("CC_REV_RATE")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let rq = std::env::var("CC_REV_Q")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(queue_limit);
        (rr, rq)
    });
    let mut rev = RevPath::new(rev_rate, rev_q, one_way);
    // `CC_APP_INTERVAL_MS=<ms>`: birimler uygulamadan bu aralıkla sunulur (ileti hücresi);
    // varsayılan 0 = hepsi t=0'da (dosya). `app_interval_ms` parametresi önceliklidir.
    let app_interval = app_interval_ms
        .or_else(|| {
            std::env::var("CC_APP_INTERVAL_MS")
                .ok()?
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0)
        * MS;
    let mut offer_at: HashMap<u64, u64> = HashMap::new();
    let mut unit_delays: Vec<u64> = Vec::new();
    // `CC_REV_LOSS=<binde>`: dönüş yolunda (SACK) rastgele kayıp; varsayılan 0 (kayıpsız dönüş).
    let rev_loss: u64 = std::env::var("CC_REV_LOSS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut rev_lcg: u64 = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(12345);
    let mut acked_seen: HashSet<u64> = HashSet::new();
    let mut delivered: HashSet<u64> = HashSet::new();
    let (n_units, unit_size) = units;
    let mut o = Outcome {
        completed_ns: None,
        delivered_units: 0,
        sent_data: 0,
        sent_retx: 0,
        sent_parity: 0,
        sent_bytes: 0,
        dropped_queue: 0,
        dropped_random: 0,
        acks: 0,
        rev_bytes: 0,
        rev_dropped_queue: 0,
        unit_delay_p50_ns: 0,
        unit_delay_p95_ns: 0,
    };
    // `CC_TRACE=1` iz; `CC_TRACE_MS=<ms>` iz aralığı (varsayılan 250).
    let trace = std::env::var("CC_TRACE").is_ok();
    let trace_every = std::env::var("CC_TRACE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(250)
        * MS;
    // `CC_WIRE_OVERHEAD=<bayt>`: ileri yönde her gönderime eklenen tel zarfı (Noise mühürlü başlık +
    // UDP/IP); varsayılan 0 (eski davranış). `wire_overhead` parametresi önceliklidir.
    let wire_overhead = wire_overhead
        .or_else(|| std::env::var("CC_WIRE_OVERHEAD").ok()?.parse::<u64>().ok())
        .unwrap_or(0);
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Bottleneck, o: &mut Outcome| {
        for d in decs {
            match d.kind {
                TransmitKind::Data => o.sent_data += 1,
                TransmitKind::Retransmit | TransmitKind::Duplicate => o.sent_retx += 1,
                TransmitKind::FecParity => o.sent_parity += 1,
                TransmitKind::CsProbe => {}
            }
            o.sent_bytes += d.size + wire_overhead;
            fwd.send(now, d.seq, d.size + wire_overhead, d.unit_id, d.kind);
        }
    };
    // Dosya tek seferde kuyruğa girer (uygulama sınırlı değil); ileti hücresinde birimler
    // `app_interval` aralığıyla sunulur.
    let mut next_offer: u64 = 0;
    let offer = |i: u64,
                 now: u64,
                 s: &mut Sender,
                 fwd: &mut Bottleneck,
                 o: &mut Outcome,
                 offer_at: &mut HashMap<u64, u64>| {
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id: i,
                size: unit_size,
            }),
            now,
        );
        offer_at.insert(i, now);
        emit(decs, now, fwd, o);
        if parity_per_block > 0 && (i + 1).is_multiple_of(8) {
            let block_id = (i / 8) as u32;
            for esi in 0..parity_per_block {
                s.enqueue_fec_parity(block_id, 8 + esi, unit_size);
            }
        }
    };
    if app_interval == 0 {
        for i in 0..n_units {
            offer(i, 0, &mut s, &mut fwd, &mut o, &mut offer_at);
        }
        next_offer = n_units;
    }
    let push_ack = |frame: adaptive_transport::ack::AckFrame,
                    expected: u64,
                    now: u64,
                    acked_seen: &mut HashSet<u64>,
                    rev: &mut RevPath| {
        let new: Vec<u64> = frame
            .acked_full(expected)
            .into_iter()
            .filter(|q| acked_seen.insert(*q))
            .collect();
        if !new.is_empty() {
            let size = frame.encode().len() as u64 + ACK_WIRE_OVERHEAD;
            let ack = AckInfo {
                acked_seqs: new.clone(),
                ack_delay: frame.ack_delay_duration(),
            };
            if !rev.send(now, size, ack) {
                // Dönüş kuyruğunda düştü: seq'ler sonraki birikimli SACK'te yine gelir.
                for q in &new {
                    acked_seen.remove(q);
                }
            }
        }
    };
    let t_max = t_max_s * SEC;
    let mut now = 0u64;
    while now < t_max {
        // Uygulama sunumu (ileti hücresi).
        while app_interval > 0 && next_offer < n_units && next_offer * app_interval <= now {
            offer(next_offer, now, &mut s, &mut fwd, &mut o, &mut offer_at);
            next_offer += 1;
        }
        // Gönderici saati: 1 ms tick (pacer dolumu + PTO) — desen1_sim ile aynı sürüş.
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut o);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut o);
        }
        // Alıcı: varışlar.
        for (seq, uid, kind) in fwd.poll(now) {
            if kind == TransmitKind::CsProbe {
                continue;
            }
            let rx_kind = if kind == TransmitKind::FecParity {
                RxKind::FecShard {
                    block_id: 0,
                    esi: 0,
                    k_actual: 8,
                    is_repair: true,
                }
            } else {
                RxKind::Data
            };
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: rx_kind,
                },
                now,
            );
            for u in out.delivered {
                if delivered.insert(u) {
                    if let Some(&t0) = offer_at.get(&u) {
                        unit_delays.push(now - t0);
                    }
                }
            }
            if let Some(frame) = out.ack {
                o.acks += 1;
                let expected = r.largest_seq().unwrap_or(seq) + 1;
                push_ack(frame, expected, now, &mut acked_seen, &mut rev);
            }
        }
        if let Some(frame) = r.on_ack_timer(now) {
            o.acks += 1;
            let expected = r.largest_seq().unwrap_or(0) + 1;
            push_ack(frame, expected, now, &mut acked_seen, &mut rev);
        }
        // Dönüş yolu (serileştirme + gecikme; `CC_REV_LOSS` ile rastgele SACK kaybı).
        while rev.wire.front().is_some_and(|(t, _)| *t <= now) {
            let (_, ack) = rev.wire.pop_front().unwrap();
            if rev_loss > 0 {
                rev_lcg = rev_lcg
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                if (rev_lcg >> 33) % 1000 < rev_loss {
                    // SACK yolda düştü: içindeki seq'ler bir sonraki SACK'te yine gelir
                    // (alıcı birikimli bildirir); `acked_seen` filtresi bu yüzden burada geri alınır.
                    for q in &ack.acked_seqs {
                        acked_seen.remove(q);
                    }
                    continue;
                }
            }
            if trace {
                println!(
                    "[{}] t={:.2}s ACK seqs={:?} ack_delay={:?} rtt_samples={} e2a={:?}",
                    eng.name(),
                    now as f64 / SEC as f64,
                    ack.acked_seqs,
                    ack.ack_delay,
                    s.diag_rtt_sample_count(),
                    s.diag_e2a()
                );
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut o);
        }
        if trace && now.is_multiple_of(trace_every) {
            println!(
                "[{}] t={:.2}s st={:?} cc={:?} inflight={} delivered={} tail={:?}",
                eng.name(),
                now as f64 / SEC as f64,
                s.state(),
                s.diag_cc(),
                s.inflight_bytes(),
                delivered.len(),
                s.tail_probe_diag()
            );
        }
        if o.completed_ns.is_none() && delivered.len() as u64 >= n_units {
            o.completed_ns = Some(now);
            break;
        }
        now += MS;
    }
    o.delivered_units = delivered.len();
    o.dropped_queue = fwd.dropped_queue;
    o.dropped_random = fwd.dropped_random;
    o.rev_bytes = rev.bytes;
    o.rev_dropped_queue = rev.dropped_queue;
    if !unit_delays.is_empty() {
        unit_delays.sort_unstable();
        o.unit_delay_p50_ns = unit_delays[unit_delays.len() / 2];
        o.unit_delay_p95_ns =
            unit_delays[(unit_delays.len() * 95 / 100).min(unit_delays.len() - 1)];
    }
    o
}

fn run(
    eng: Engine,
    rate_bps: u64,
    queue_limit: usize,
    rtt_ms: u64,
    loss_permille: u64,
    seed: u64,
    t_max_s: u64,
) -> Outcome {
    run_opt(
        eng,
        rate_bps,
        queue_limit,
        rtt_ms,
        loss_permille,
        seed,
        t_max_s,
        (FILE_BYTES.div_ceil(MTU), MTU),
        0,
        None,
        None,
        None,
        None,
    )
}

fn fmt_s(v: Option<u64>) -> String {
    v.map_or("YOK".to_string(), |t| {
        format!("{:.2} s", t as f64 / SEC as f64)
    })
}

fn report(eng: Engine, label: &str, o: &Outcome) {
    println!(
        "[{}] {label}: tamamlanma={} teslim={}/{} data={} retx={} parite={} bayt={} kuyruk_düşen={} rastgele_düşen={} sack={} dönüş_bayt={} dönüş_düşen={} birim_p50={} birim_p95={}",
        eng.name(),
        fmt_s(o.completed_ns),
        o.delivered_units,
        FILE_BYTES.div_ceil(MTU),
        o.sent_data,
        o.sent_retx,
        o.sent_parity,
        o.sent_bytes,
        o.dropped_queue,
        o.dropped_random,
        o.acks,
        o.rev_bytes,
        o.rev_dropped_queue,
        fmt_s(Some(o.unit_delay_p50_ns)),
        fmt_s(Some(o.unit_delay_p95_ns)),
    );
}

fn p50(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    v[v.len() / 2]
}

/// 10 seed'lik hücre: (teslim sayısı, p50, max, gönderim/yük p50).
fn ten_seeds(
    eng: Engine,
    label: &str,
    rate: u64,
    q: usize,
    rtt: u64,
    loss: u64,
) -> (usize, Option<u64>, Option<u64>, f64) {
    let mut times = Vec::new();
    let mut bytes = Vec::new();
    for seed in 1..=10 {
        let o = run(eng, rate, q, rtt, loss, seed, 120);
        if std::env::var("CC_VERBOSE").is_ok() {
            report(eng, &format!("{label} seed{seed}"), &o);
        }
        if let Some(t) = o.completed_ns {
            times.push(t);
        }
        bytes.push(o.sent_bytes);
    }
    let ok = times.len();
    let p = (!times.is_empty()).then(|| p50(times.clone()));
    let m = times.iter().copied().max();
    let overhead = p50(bytes) as f64 / FILE_BYTES as f64;
    println!(
        "[{}] {label}: teslim {ok}/10 p50={} max={} gönderim/yük p50={overhead:.2}",
        eng.name(),
        fmt_s(p),
        fmt_s(m)
    );
    (ok, p, m, overhead)
}

/// Hücre 1: 6 paketlik kuyruk, kayıp yok — sığ kuyruk çöküşü (ISEMRI-CC-PORT §1 #4).
#[test]
fn shallow_queue_file_no_loss() {
    let seed = std::env::var("CC_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let loss = std::env::var("CC_LOSS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for eng in ENGINES {
        let o = run(eng, 12_500, 6, 300, loss, seed, 120);
        report(eng, "q6 r12.5k d300 l0 150kB", &o);
        if eng == Engine::CcPort && loss == 0 {
            let t = o.completed_ns.expect("port: dosya tamamlanmalı");
            assert!(
                t <= 25 * SEC,
                "port: sığ kuyrukta {:.1} s > 25 s",
                t as f64 / SEC as f64
            );
        }
    }
}

/// Hücre 2: 6 paketlik kuyruk, %20 rastgele kayıp, 10 seed.
#[test]
fn shallow_queue_file_loss20_ten_seeds() {
    for eng in ENGINES {
        let (ok, p, _, _) = ten_seeds(eng, "q6 r12.5k d300 l20", 12_500, 6, 300, 200);
        if eng == Engine::CcPort {
            assert_eq!(ok, 10, "port: 10/10 teslim");
            assert!(p.unwrap() <= 60 * SEC, "port: p50 > 60 s");
        }
    }
}

/// Hücre 3: derin kuyruk (1000), %20 kayıp — K1 profilinin benzeri; port burada da teslim etmeli.
#[test]
fn deep_queue_file_loss20_ten_seeds() {
    for eng in ENGINES {
        let (ok, _, _, _) = ten_seeds(eng, "q1000 r12.5k d300 l20", 12_500, 1000, 300, 200);
        if eng == Engine::CcPort {
            assert_eq!(ok, 10, "port: 10/10 teslim");
        }
    }
}

/// Hücre 4: PTT-benzeri kısa demet (6 × 400 B, %20 kayıp, sığ kuyruk) — ilk demet ≤ 3 s.
#[test]
fn ptt_bundle_first_delivery() {
    for eng in ENGINES {
        let o = run_opt(
            eng,
            12_500,
            6,
            300,
            200,
            7,
            30,
            (6, 400),
            0,
            None,
            None,
            None,
            None,
        );
        println!(
            "[{}] ptt 6×400B q6 l20: ilk demet={}",
            eng.name(),
            fmt_s(o.completed_ns)
        );
        if eng == Engine::CcPort {
            let t = o.completed_ns.expect("port: demet teslim edilmeli");
            assert!(
                t <= 3 * SEC,
                "port: ilk demet {:.2} s > 3 s",
                t as f64 / SEC as f64
            );
        }
    }
}

/// Hücre 5: FEC paritesi tele çıkıyor mu (ISEMRI-CC-PORT §1 #5). Blok başına 2 parite kuyruğa
/// girer; eski motorda kova tavanı pariteyi atar (26,45 kB/s altında), portta burst tabanı 10·MTU.
#[test]
fn fec_parity_reaches_the_wire_below_26kbps() {
    let expected = (FILE_BYTES.div_ceil(MTU) / 8) * 2;
    for eng in ENGINES {
        let o = run_opt(
            eng,
            12_500,
            1000,
            300,
            0,
            1,
            120,
            (FILE_BYTES.div_ceil(MTU), MTU),
            2,
            None,
            None,
            None,
            None,
        );
        report(eng, "q1000 r12.5k d300 l0 + 2 parite/8 birim", &o);
        println!(
            "[{}] parite: beklenen {expected}, tele çıkan {}",
            eng.name(),
            o.sent_parity
        );
        if eng == Engine::CcPort {
            assert!(
                o.sent_parity >= expected * 9 / 10,
                "port: parite tele çıkmalı ({} / {expected})",
                o.sent_parity
            );
        }
    }
}

/// Keşif matrisi (yalnız `CC_MATRIX=1` ile; kapı değil): hücre × 10 seed × iki motor.
#[test]
fn matrix_explore() {
    if std::env::var("CC_MATRIX").is_err() {
        return;
    }
    let cells: [(&str, u64, usize, u64, u64); 6] = [
        ("q6 r12.5k d300 l30", 12_500, 6, 300, 300),
        ("q6 r12.5k d1000 l20", 12_500, 6, 1000, 200),
        ("q1000 r12.5k d1000 l0", 12_500, 1000, 1000, 0),
        ("q6 r3k d600 l25", 3_000, 6, 600, 250),
        ("q6 r50k d100 l10", 50_000, 6, 100, 100),
        ("q1000 r12.5k d300 l30", 12_500, 1000, 300, 300),
    ];
    for (label, rate, q, rtt, loss) in cells {
        for eng in ENGINES {
            ten_seeds(eng, label, rate, q, rtt, loss);
        }
    }
}

/// r25 ileti hücresi (bench `b_r25_d300_l20_p65` yaklaşığı): 25 B/s İKİ yönde, 300 ms tek yön,
/// %20 kayıp, 60 × 65 B ileti 10 s aralıkla (tel 130 B ≈ 5,2 s; yük ≈ %52). Ölçü: ileti teslim gecikmesi p50 ve teslim/60.
/// Dönüş yolu serileştirilir (SACK ~65 B ≈ 2,6 s). Kapı: 60/60 teslim; port ileti p50 ≤ 20 s. `CC_R25_SEEDS=<n>` ile tohum sayısı (varsayılan 10).
#[test]
fn r25_lowband_messages() {
    let seeds: u64 = std::env::var("CC_R25_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    for eng in ENGINES {
        let (mut p50s, mut deliv, mut fwd_b, mut rev_b, mut retx) =
            (vec![], vec![], vec![], vec![], vec![]);
        for seed in 1..=seeds {
            let o = run_opt(
                eng,
                25,
                20,
                300,
                200,
                seed,
                700,
                (60, 65),
                0,
                None,
                Some((25, 20)),
                Some(10_000),
                Some(65),
            );
            report(eng, &format!("r25 seed{seed}"), &o);
            p50s.push(o.unit_delay_p50_ns);
            deliv.push(o.delivered_units as u64);
            fwd_b.push(o.sent_bytes);
            rev_b.push(o.rev_bytes);
            retx.push(o.sent_retx);
        }
        println!(
            "[{}] r25 ÖZET: ileti p50 medyan={} teslim medyan={}/60 ileri bayt medyan={} dönüş bayt medyan={} retx medyan={}",
            eng.name(),
            fmt_s(Some(p50(p50s.clone()))),
            p50(deliv.clone()),
            p50(fwd_b),
            p50(rev_b),
            p50(retx)
        );
        // Kapı: her motor 60/60 teslim (medyan); port ileti p50 ≤ 20 s (U10–U12 öncesi 95 s; eski
        // motor ~6,6 s — eşitlenince kapı sıkılaşır).
        assert_eq!(
            p50(deliv),
            60,
            "[{}] r25 teslim medyanı < 60/60",
            eng.name()
        );
        if eng != Engine::Legacy {
            assert!(
                p50(p50s) <= 20 * SEC,
                "[{}] r25 ileti p50 medyanı > 20 s",
                eng.name()
            );
        }
    }
}

/// Hücre 6: kesinti — 150 kB dosya, 12,5 kB/s, 300 ms, %20 kayıp; t = 3 s'de 60 s tam kesinti
/// (gösterim "kesinti" profili: 60 s %100 kayıp, sonra eski profil). Oturum kesintiyi atlatmalı ve
/// dosya kesinti bittikten sonra makul sürede tamamlanmalı (kapı: port ≤ 63 + 30 s).
#[test]
fn blackout_60s_session_survives_and_completes() {
    for eng in ENGINES {
        let mut times = Vec::new();
        for seed in 1..=5 {
            let o = run_opt(
                eng,
                12_500,
                1000,
                300,
                200,
                seed,
                180,
                (FILE_BYTES.div_ceil(MTU), MTU),
                0,
                Some((3, 60)),
                None,
                None,
                None,
            );
            report(eng, &format!("kesinti 60s @3s q1000 l20 seed{seed}"), &o);
            if let Some(t) = o.completed_ns {
                times.push(t);
            }
        }
        println!(
            "[{}] kesinti: teslim {}/5 p50={}",
            eng.name(),
            times.len(),
            (!times.is_empty())
                .then(|| p50(times.clone()))
                .map_or("YOK".into(), |t| format!("{:.1} s", t as f64 / SEC as f64))
        );
        if eng == Engine::CcPort {
            assert_eq!(times.len(), 5, "port: kesinti sonrası 5/5 tamamlanmalı");
            assert!(
                p50(times) <= 93 * SEC,
                "port: kesinti sonrası ≤ 30 s içinde bitmeli"
            );
        }
    }
}
