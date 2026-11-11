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
use std::collections::{HashSet, VecDeque};

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
    let mut r = eng.receiver();
    if parity_per_block > 0 {
        s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
            .expect("manuel FEC kipi");
    }
    let mut fwd = Bottleneck::new(rate_bps, queue_limit, one_way, loss_permille, seed);
    // `CC_BLACKOUT=<başlangıç_s>,<süre_s>`: ileri yönde %100 kayıp penceresi (kesinti profili).
    if let Ok(v) = std::env::var("CC_BLACKOUT") {
        if let Some((a, d)) = v.split_once(',') {
            if let (Ok(a), Ok(d)) = (a.parse::<u64>(), d.parse::<u64>()) {
                fwd.blackout = Some((a * SEC, (a + d) * SEC));
            }
        }
    }
    let mut rev: VecDeque<(u64, AckInfo)> = VecDeque::new();
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
    };
    let trace = std::env::var("CC_TRACE").is_ok();
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Bottleneck, o: &mut Outcome| {
        for d in decs {
            match d.kind {
                TransmitKind::Data => o.sent_data += 1,
                TransmitKind::Retransmit | TransmitKind::Duplicate => o.sent_retx += 1,
                TransmitKind::FecParity => o.sent_parity += 1,
                TransmitKind::CsProbe => {}
            }
            o.sent_bytes += d.size;
            fwd.send(now, d.seq, d.size, d.unit_id, d.kind);
        }
    };
    // Dosya tek seferde kuyruğa girer (uygulama sınırlı değil).
    for i in 0..n_units {
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id: i,
                size: unit_size,
            }),
            0,
        );
        emit(decs, 0, &mut fwd, &mut o);
        if parity_per_block > 0 && (i + 1) % 8 == 0 {
            let block_id = (i / 8) as u32;
            for esi in 0..parity_per_block {
                s.enqueue_fec_parity(block_id, 8 + esi, unit_size);
            }
        }
    }
    let push_ack = |frame: adaptive_transport::ack::AckFrame,
                    expected: u64,
                    now: u64,
                    acked_seen: &mut HashSet<u64>,
                    rev: &mut VecDeque<(u64, AckInfo)>| {
        let new: Vec<u64> = frame
            .acked_full(expected)
            .into_iter()
            .filter(|q| acked_seen.insert(*q))
            .collect();
        if !new.is_empty() {
            rev.push_back((
                now + one_way,
                AckInfo {
                    acked_seqs: new,
                    ack_delay: frame.ack_delay_duration(),
                },
            ));
        }
    };
    let t_max = t_max_s * SEC;
    let mut now = 0u64;
    while now < t_max {
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
                delivered.insert(u);
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
        // Dönüş yolu (yalnız gecikme; `CC_REV_LOSS` ile rastgele SACK kaybı).
        while rev.front().is_some_and(|(t, _)| *t <= now) {
            let (_, ack) = rev.pop_front().unwrap();
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
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut o);
        }
        if trace && now.is_multiple_of(250 * MS) {
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
    )
}

fn fmt_s(v: Option<u64>) -> String {
    v.map_or("YOK".to_string(), |t| {
        format!("{:.2} s", t as f64 / SEC as f64)
    })
}

fn report(eng: Engine, label: &str, o: &Outcome) {
    println!(
        "[{}] {label}: tamamlanma={} teslim={}/{} data={} retx={} parite={} bayt={} kuyruk_düşen={} rastgele_düşen={} sack={}",
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
        o.acks
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
        let o = run_opt(eng, 12_500, 6, 300, 200, 7, 30, (6, 400), 0);
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

/// Hücre 6: kesinti — 150 kB dosya, 12,5 kB/s, 300 ms, %20 kayıp; t = 3 s'de 60 s tam kesinti
/// (gösterim "kesinti" profili: 60 s %100 kayıp, sonra eski profil). Oturum kesintiyi atlatmalı ve
/// dosya kesinti bittikten sonra makul sürede tamamlanmalı (kapı: port ≤ 63 + 30 s).
#[test]
fn blackout_60s_session_survives_and_completes() {
    for eng in ENGINES {
        let mut times = Vec::new();
        for seed in 1..=5 {
            // Kesinti penceresi ortam değişkeniyle (harness yerel; testler tek iş parçacığında koşar).
            std::env::set_var("CC_BLACKOUT", "3,60");
            let o = run(eng, 12_500, 1000, 300, 200, seed, 180);
            std::env::remove_var("CC_BLACKOUT");
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
