//! [ASİMETRİK-YOL · ürün hedefi] Yön profili açıkken port göndericisi darboğaz kuyruğunu
//! doldurmaz: profil pacer tavanı (`Sender::set_cc_profile_pacing_cap`) hem hızı hem patlamayı
//! sınırlar. Akışlar birbirini bloklamaz: dosyanın ortasında sunulan metin, darboğaz kuyruğunda
//! dosya datagramlarının arkasında saniyelerce beklemez.
//!
//! # Hangi boşluğu kapatır
//!
//! Profil (12,5 kB/s yukarı, 62 B/s aşağı) göndericiye 67,5 kB'lık ilk pencere ve hattın hızında bir
//! pacer tavanı verir. İki yerde tavan aşılıyordu (S1 hücresinde rölenin paket izi, 120 × 900 B):
//!
//! 1. Tavan `rate.min(cap.max(measured_pacing_target))` idi; `measured_pacing_target` = BBR kazancı ×
//!    BtlBw. Startup'ta kazanç 2,885'tir: ölçülen BtlBw tavanın %35'ini geçer geçmez hedef tavanı
//!    aşar ve pacer hattın üstüne çıkar (izde 48 ms datagram aralığı ≈ 19,4 kB/s, hat 12,5 kB/s;
//!    darboğaz kuyruğu 2 s'de 13 → 28 paket).
//! 2. Pacer'ın patlama tavanı port formülünün tabanıdır: 10 · MTU = 12 000 B (quinn'in hızlı
//!    yollar için zamanlayıcı tabanı). ACK beklenen saniyelerde (bütçeli dönüşte ≈ 5,4 s) pacer bu
//!    tavana dek kredi biriktirir; ACK gelince 13 datagram bir anda çıkar (kuyruk 0 → 13). Tavan
//!    hattın hızında olduğundan kuyruk sonra boşalmaz.
//!
//! # Düzenek (belirlenimli; ağ yok)
//!
//! Gerçek `Sender::with_cc_port` + gerçek `Receiver::with_cc_port`, ürünün profil kurulumuyla
//! (`client-core` `runtime::client_task`: ilk pencere = yukarı hız × bütçeli ACK aralığı, pacer
//! tavanı = yukarı hız × MTU / (MTU + 28); alıcıda `set_return_bandwidth(62)`). İleri yön 12,5 kB/s
//! (UDP/IP 28 B dahil), 64 paket FIFO, 300 ms; dönüş 62 B/s, 64 paket, 300 ms. Dosya 120 birim ×
//! 933 B (ürünün 900 B'lık biriminin mühürlü boyu). 1 ms'lik saat; alıcı ACK zamanlayıcısı 20 ms
//! (ürünün `ACK_TICK`'i). Kayıpsız ve %20 tohumlu kayıplı kollar.
//!
//! # Eşikler (ölçümden önce, ürün hedefinden)
//!
//! * [`QUEUE_BOUND`] = 8 paket: darboğaz kuyruğunun her an en çok bu kadar paketi. 961 B'lık
//!   datagram 12,5 kB/s'de 77 ms serileşir; 8 paket ≈ 0,62 s. Bu kuyruğun arkasına giren metin
//!   300 ms yayılımla ≈ 0,93 s'de varır — dosya ortasındaki metin için 1 s kabul ölçütünün altı.
//! * [`FILE_BOUND`]: dosyanın tamamlanması. Kayıpsız 120 × 961 B hattın kendisinde 9,2 s;
//!   ilk ACK 1,8–2,4 s'de döner ve pencere ilk RTT'de 67,5 kB'tır → 30 s. Kayıplıda kayıplar ACK
//!   aralıklarında (≈ 5,4 s) yeniden gider → 90 s.

use adaptive_transport::ack::AckFrame;
use adaptive_transport::receiver::{budgeted_ack_interval_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use std::collections::{HashSet, VecDeque};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const UDP_IP: u64 = 28;
const FWD_BPS: u64 = 12_500;
const REV_BPS: u64 = 62;
const QUEUE_PKTS: usize = 64;
const ONE_WAY: u64 = 300 * MS;
const UNITS: u64 = 120;
const UNIT_WIRE: u64 = 933;
/// SACK'in mühürlü başlığı (UDP/IP hariç): ürün izinde ACK datagramları 55–101 B.
const ACK_SEAL: u64 = 37;
const QUEUE_BOUND: usize = 8;
const FILE_BOUND_LOSSLESS: u64 = 30 * SEC;
const FILE_BOUND_LOSSY: u64 = 90 * SEC;

/// Serileştirme + sınırlı FIFO + sabit gecikme + tohumlu bağımsız kayıp (LCG).
struct Line {
    rate_bps: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64)>,
    lcg: u64,
    loss_permille: u64,
    peak: usize,
    dropped_queue: u64,
}

impl Line {
    fn new(rate_bps: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            lcg: seed.wrapping_mul(6_364_136_223_846_793_005) | 1,
            loss_permille,
            peak: 0,
            dropped_queue: 0,
        }
    }

    /// Kuyruktaki (serileşmesi bitmemiş) paket sayısı, `now` anında.
    fn depth(&mut self, now: u64) -> usize {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        self.busy.len()
    }

    /// Döner: kuyruğa girdi mi.
    fn send(&mut self, now: u64, wire_bytes: u64, a: u64, b: u64) -> bool {
        if self.depth(now) >= QUEUE_PKTS {
            self.dropped_queue += 1;
            return false;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + wire_bytes * SEC / self.rate_bps;
        self.busy.push_back(done);
        self.peak = self.peak.max(self.busy.len());
        if self.loss_permille > 0 {
            self.lcg = self
                .lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            if (self.lcg >> 33) % 1000 < self.loss_permille {
                return true;
            }
        }
        self.wire.push_back((done + ONE_WAY, a, b));
        true
    }

    fn poll(&mut self, now: u64) -> Vec<(u64, u64)> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|&(t, _, _)| t <= now) {
            let (_, a, b) = self.wire.pop_front().expect("var");
            out.push((a, b));
        }
        out
    }
}

#[derive(Debug)]
struct Out {
    completed_ns: Option<u64>,
    queue_peak: usize,
    queue_dropped: u64,
    rev_bytes: u64,
    /// Pacer hızının profil tavanını ve ölçülen BtlBw'yi birlikte aştığı en büyük değer (B/s).
    rate_over_cap_max: u64,
    burst_cap_max: u64,
}

/// Ürünün profil kurulumu (`client-core` `runtime::client_task`, `link_profile_window_bytes`,
/// `link_pacing_cap_bps`).
fn profile_sender() -> (Sender, u64) {
    let mut s = Sender::with_cc_port();
    let t_ack = budgeted_ack_interval_ns(REV_BPS);
    let iw =
        u64::try_from(u128::from(FWD_BPS) * u128::from(t_ack) / u128::from(SEC)).expect("pencere");
    let cap = FWD_BPS * MTU / (MTU + UDP_IP);
    s.set_cc_initial_window(iw);
    s.set_cc_profile_pacing_cap(Some(cap));
    (s, cap)
}

fn run(loss_permille: u64, seed: u64, t_max: u64) -> Out {
    let trace = std::env::var("PCQ_TRACE").is_ok();
    let (mut s, cap) = profile_sender();
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(REV_BPS));
    let mut fwd = Line::new(FWD_BPS, loss_permille, seed);
    let mut rev = Line::new(REV_BPS, 0, seed ^ 0x5A5A);
    let mut acks: Vec<AckInfo> = Vec::new();
    let mut rev_bytes = 0u64;
    let mut acked_seen: HashSet<u64> = HashSet::new();
    let mut delivered: HashSet<u64> = HashSet::new();
    let mut out = Out {
        completed_ns: None,
        queue_peak: 0,
        queue_dropped: 0,
        rev_bytes: 0,
        rate_over_cap_max: 0,
        burst_cap_max: 0,
    };
    let emit = |decs: Vec<adaptive_transport::sender::TransmitDecision>,
                now: u64,
                s: &Sender,
                fwd: &mut Line| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            let q = fwd.depth(now);
            if trace {
                let (rate, tokens, burst) = s.diag_pacer();
                let mode = s.diag_cc().map(|c| (c.0, c.2));
                println!(
                    "t={:.3} {:?} seq={} birim={} kuyruk={q} pacer={rate}/{tokens}/{burst} bbr={mode:?}",
                    now as f64 / SEC as f64,
                    d.kind,
                    d.seq,
                    d.unit_id,
                );
            }
            fwd.send(now, d.size + UDP_IP, d.seq, d.unit_id);
        }
    };
    // Dosya t = 0'da motora verilir (ürünün şerit katmanı da profil penceresini doldurur).
    for i in 0..UNITS {
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id: i,
                size: UNIT_WIRE,
            }),
            0,
        );
        emit(decs, 0, &s, &mut fwd);
    }
    let mut now = 0u64;
    while now < t_max {
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &s, &mut fwd);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &s, &mut fwd);
        }
        {
            let (rate, _, burst) = s.diag_pacer();
            let bw = s.diag_cc().map_or(0, |c| c.2);
            if rate > cap.max(bw) {
                out.rate_over_cap_max = out.rate_over_cap_max.max(rate);
            }
            out.burst_cap_max = out.burst_cap_max.max(burst);
        }
        let mut frames: Vec<AckFrame> = Vec::new();
        for (seq, unit) in fwd.poll(now) {
            let o = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: unit,
                    kind: RxKind::Data,
                },
                now,
            );
            delivered.extend(o.delivered);
            if let Some(f) = o.ack {
                frames.push(f);
            }
        }
        if now.is_multiple_of(20 * MS) {
            if let Some(f) = r.on_ack_timer(now) {
                frames.push(f);
            }
        }
        for f in frames {
            let expected = r.largest_seq().map_or(0, |l| l + 1);
            let new: Vec<u64> = f
                .acked_full(expected)
                .into_iter()
                .filter(|q| acked_seen.insert(*q))
                .collect();
            let wire = f.encode().len() as u64 + ACK_SEAL + UDP_IP;
            let idx = acks.len() as u64;
            if rev.send(now, wire, idx, 0) {
                rev_bytes += wire;
                acks.push(AckInfo {
                    acked_seqs: new,
                    ack_delay: f.ack_delay_duration(),
                });
            } else {
                for q in new {
                    acked_seen.remove(&q);
                }
            }
        }
        for (idx, _) in rev.poll(now) {
            let ack = std::mem::replace(
                &mut acks[usize::try_from(idx).expect("sıra")],
                AckInfo {
                    acked_seqs: Vec::new(),
                    ack_delay: std::time::Duration::ZERO,
                },
            );
            if ack.acked_seqs.is_empty() {
                continue;
            }
            if trace {
                println!(
                    "t={:.3} ACK {} seq",
                    now as f64 / SEC as f64,
                    ack.acked_seqs.len()
                );
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &s, &mut fwd);
        }
        if delivered.len() as u64 >= UNITS {
            out.completed_ns = Some(now);
            break;
        }
        now += MS;
    }
    out.queue_peak = fwd.peak;
    out.queue_dropped = fwd.dropped_queue;
    out.rev_bytes = rev_bytes;
    out
}

fn check(label: &str, o: &Out, file_bound: u64) {
    eprintln!(
        "ÖLÇÜM profil-tavanı {label}: kuyruk en çok {} paket · tamamlanma {:?} s · dönüş {} B · \
         tavan+BtlBw üstü hız en çok {} B/s · patlama tavanı en çok {} B · kuyrukta düşen {}",
        o.queue_peak,
        o.completed_ns.map(|t| t as f64 / SEC as f64),
        o.rev_bytes,
        o.rate_over_cap_max,
        o.burst_cap_max,
        o.queue_dropped,
    );
    let done = o.completed_ns.expect("dosya tamamlanmadı");
    assert!(
        done <= file_bound,
        "{label}: dosya {:.1} s > {:.0} s",
        done as f64 / SEC as f64,
        file_bound as f64 / SEC as f64
    );
    assert!(
        o.queue_peak <= QUEUE_BOUND,
        "{label}: darboğaz kuyruğu {} paket > {QUEUE_BOUND} (dosya ortasındaki metin ≥ 1 s bekler)",
        o.queue_peak
    );
}

#[test]
fn profile_cap_keeps_the_bottleneck_queue_short_lossless() {
    let o = run(0, 1, 120 * SEC);
    check("kayıpsız", &o, FILE_BOUND_LOSSLESS);
}

#[test]
fn profile_cap_keeps_the_bottleneck_queue_short_with_loss() {
    for seed in 1..=3 {
        let o = run(200, seed, 240 * SEC);
        check(&format!("%20 kayıp tohum {seed}"), &o, FILE_BOUND_LOSSY);
    }
}
