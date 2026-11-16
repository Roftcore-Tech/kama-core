//! [HAT-KORUMASI] Geri bildirimi aç profil gerçek ileri hattan iyimserken darboğaz kuyruğu sınırlı
//! kalır, dosya biter ve sesli kısa birimler kuyruğun arkasında beklemez; profil dürüstken koruma
//! hiç devreye girmez.
//!
//! Düzenek (ayrık olay, deterministik; gerçek `Sender` + gerçek `Receiver`): ürünün geri bildirimi
//! aç profil kurulumu (`client-core` `runtime.rs`, profil 12 500 / 62 B/s) — profil pacer tavanı,
//! profilden ilk pencere, `FeedbackStarvedRule`, PTT benzeri öncelikli kısa birimlerin ACK'siz
//! kopyaları; alıcı dönüş bütçesiyle (62 B/s) ve ürün çalışma zamanı gibi dönüş hattı boşken ACK
//! çıkarır (`set_uplink_yield`). ACK'in tel boyu göndericiye ürün yolundaki gibi
//! `observe_ack_feedback` ile verilir.
//!
//! * ileri hat: gerçek hız (profil 12 500 B/s bildirir), 64 paket FIFO, 300 ms, serileşmeden sonra
//!   %20 rastgele kayıp (tohumlu LCG), datagram başına 28 B UDP/IP zarfı;
//! * dönüş hattı: 62 B/s, 300 ms, ACK tel boyu = kodlanmış çerçeve + 50 B (UDP/IP 28 + mühür 22).
//!
//! Yük: t = 0'da 41 × 1 000 B dosya; t = 0,5 s'den başlayarak 200 ms'de bir 86 B'lik öncelikli
//! kısa birim (114 B tel; ürünün PTT demeti), dört ACK'siz kopya, 40 ms aralık.

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, Event, FeedbackStarvedRule, ForwardCopyRule, OpaqueUnit, Sender, Timer,
    TransmitDecision, TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
/// ACK datagramının UDP gövdesinde kodlanmış çerçeveye eklenen mühür baytı.
const ACK_SEAL: u64 = 22;
const ONE_WAY_NS: u64 = 300 * MS;
const QUEUE_PKTS: usize = 64;
const LOSS_PERMILLE: u64 = 200;
const FILE_UNITS: u64 = 41;
const FILE_UNIT: u64 = 1000;
const FILE_BASE_ID: u64 = 1_000_000;
const VOICE_UNIT: u64 = 86;
const VOICE_PERIOD_NS: u64 = 200 * MS;
const VOICE_START_NS: u64 = 500 * MS;
const T_MAX_NS: u64 = 90 * SEC;

struct Bottleneck {
    rate_bps: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64)>,
    lcg: u64,
    dropped_queue: Vec<u64>,
    /// (gönderim anı, kuyrukta bekleme ns) — kuyrukta düşmeyen her datagram için.
    waits: Vec<(u64, u64)>,
}

impl Bottleneck {
    fn new(rate_bps: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            dropped_queue: Vec::new(),
            waits: Vec::new(),
        }
    }

    fn send(&mut self, now: u64, seq: u64, udp: u64, unit_id: u64) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= QUEUE_PKTS {
            self.dropped_queue.push(now);
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + (udp + OVH) * SEC / self.rate_bps;
        self.busy.push_back(done);
        self.waits.push((now, start - now));
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        if (self.lcg >> 33) % 1000 < LOSS_PERMILLE {
            return;
        }
        self.wire.push_back((done + ONE_WAY_NS, seq, unit_id));
    }
}

/// Dönüş hattı: FIFO serileşme + yayılım.
struct ReturnLine {
    free_at: u64,
    wire: VecDeque<(u64, u64, AckInfo)>,
}

impl ReturnLine {
    fn send(&mut self, now: u64, udp: u64, ack: AckInfo) {
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        self.wire.push_back((self.free_at + ONE_WAY_NS, udp, ack));
    }
}

#[derive(Debug)]
struct Outcome {
    file_done_ns: Option<u64>,
    /// Bu andan sonra gönderilen datagramların darboğazda en uzun beklemesi (ns).
    max_wait_after: u64,
    /// Bu andan sonra kuyrukta düşen datagram sayısı.
    drops_after: usize,
    /// Bu andan sonra sunulan kısa birimlerin en büyük teslim gecikmesi (ns).
    voice_max_after: u64,
    voice_undelivered: usize,
    guard: Option<(Option<u64>, u64)>,
}

fn sender() -> Sender {
    let mut s = Sender::with_cc_port();
    // Ürün: ilk pencere = yukarı yön × bütçeli ACK aralığı; profil pacer tavanı.
    let t_ack = adaptive_transport::receiver::budgeted_ack_interval_ns(PROFILE_DOWN);
    s.set_cc_initial_window(PROFILE_UP * t_ack / SEC);
    let cap = PROFILE_UP * MTU / (MTU + OVH);
    s.set_cc_profile_pacing_cap(Some(cap));
    s.set_cc_feedback_starved(Some(FeedbackStarvedRule {
        pacing_cap_bps: cap,
        feedback_ns: feedback_horizon_ns(PROFILE_DOWN),
        line: LineModel {
            rate_bps: PROFILE_UP,
            datagram_overhead: OVH,
        },
        return_line: LineModel {
            rate_bps: PROFILE_DOWN,
            datagram_overhead: OVH,
        },
    }));
    s.set_forward_copy_rule(Some(ForwardCopyRule {
        below_id: FILE_BASE_ID,
        priority_below_id: FILE_BASE_ID,
        copies: 4,
        spacing_ns: 40 * MS,
    }));
    s
}

fn run(real_up: u64, seed: u64, settle_ns: u64) -> Outcome {
    let mut s = sender();
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    let mut fwd = Bottleneck::new(real_up, seed);
    let mut ret = ReturnLine {
        free_at: 0,
        wire: VecDeque::new(),
    };
    let mut offered: HashMap<u64, u64> = HashMap::new();
    let mut delivered: HashSet<u64> = HashSet::new();
    let mut voice_max_after = 0u64;
    let mut file_left = FILE_UNITS;
    let mut file_done_ns = None;
    let mut tx_datagrams = 0u64;
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Bottleneck, tx: &mut u64| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            *tx += 1;
            fwd.send(now, d.seq, d.size, d.unit_id);
        }
    };
    for i in 0..FILE_UNITS {
        let id = FILE_BASE_ID + i;
        offered.insert(id, 0);
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id,
                size: FILE_UNIT,
            }),
            0,
        );
        emit(decs, 0, &mut fwd, &mut tx_datagrams);
    }
    let mut next_voice = 0u64;
    let mut now = 0u64;
    while now < T_MAX_NS {
        let voice_at = VOICE_START_NS + next_voice * VOICE_PERIOD_NS;
        if file_done_ns.is_none() && now >= voice_at {
            offered.insert(next_voice, now);
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: VOICE_UNIT,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx_datagrams);
            next_voice += 1;
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut tx_datagrams);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut tx_datagrams);
        }
        // Alıcı: ürün çalışma zamanı gibi dönüş hattı doluyken bütçeli ACK çıkmaz.
        r.set_uplink_yield(ret.free_at > now);
        let mut acks = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _)| t <= now) {
            let (_, seq, uid) = fwd.wire.pop_front().expect("var");
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: RxKind::Data,
                },
                now,
            );
            for u in out.delivered {
                if delivered.insert(u) {
                    let t0 = offered.get(&u).copied().unwrap_or(0);
                    if u >= FILE_BASE_ID {
                        file_left -= 1;
                        if file_left == 0 {
                            file_done_ns = Some(now);
                        }
                    } else if t0 >= settle_ns {
                        voice_max_after = voice_max_after.max(now - t0);
                    }
                }
            }
            acks.extend(out.ack);
        }
        acks.extend(r.on_ack_timer(now));
        for frame in acks {
            let expected = r.largest_seq().unwrap_or(0) + 1;
            let udp = frame.encode().len() as u64 + ACK_SEAL;
            ret.send(
                now,
                udp,
                AckInfo {
                    acked_seqs: frame.acked_full(expected),
                    ack_delay: frame.ack_delay_duration(),
                },
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _)| t <= now) {
            let (_, udp, ack) = ret.wire.pop_front().expect("var");
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: tx_datagrams,
                },
                now,
            );
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut tx_datagrams);
        }
        let voice_left = (0..next_voice).any(|v| !delivered.contains(&v));
        if file_done_ns.is_some() && !voice_left {
            break;
        }
        now += MS;
    }
    Outcome {
        file_done_ns,
        max_wait_after: fwd
            .waits
            .iter()
            .filter(|&&(t, _)| t >= settle_ns)
            .map(|&(_, w)| w)
            .max()
            .unwrap_or(0),
        drops_after: fwd
            .dropped_queue
            .iter()
            .filter(|&&t| t >= settle_ns)
            .count(),
        voice_max_after,
        voice_undelivered: (0..next_voice).filter(|v| !delivered.contains(v)).count(),
        guard: s.diag_line_guard(),
    }
}

fn secs(ns: u64) -> String {
    format!("{:.2} s", ns as f64 / SEC as f64)
}

/// Gerçek hat profilin yarısı (6 250 B/s): koruma gerçek hattı ölçer, darboğaz kuyruğu boşaltılır
/// ve ölçümden sonra kısa birimler kuyruksuz gider; dosya biter.
///
/// Sınırlar (ölçümden önce, fizikten): ilk ölçüm ilk iki ACK'le gelir (≈ 4,5 s; ACK ≈ 5 s'de
/// bir), o ana dek profil hızında biriken kuyruk (≈ 4 s'lik hat süresi) boşaltılır; 15 s'den sonra
/// gönderilen datagram darboğazda en çok iki tam boy datagramın serileşmesi kadar (2 × 1 228 B /
/// 6 250 B/s ≈ 0,4 s) bekler ve kuyrukta düşmez. Dosya ≤ 60 s.
#[test]
fn optimistic_profile_queue_stays_bounded_and_file_completes() {
    for seed in 1..=3u64 {
        let o = run(6_250, seed, 15 * SEC);
        println!(
            "HAT-KORUMASI gerçek 6 250 B/s tohum {seed}: dosya={} bekleme_max(15 s sonrası)={} \
             kuyruk_düşen(15 s sonrası)={} kısa_birim_max(15 s sonrası)={} teslimsiz={} koruma={:?}",
            o.file_done_ns.map_or("YOK".to_string(), secs),
            secs(o.max_wait_after),
            o.drops_after,
            secs(o.voice_max_after),
            o.voice_undelivered,
            o.guard
        );
        let (rate, samples) = o.guard.expect("kural kurulu");
        let rate = rate.expect("hat profilin altında ölçüldü");
        assert!(samples >= 1);
        assert!(
            rate.abs_diff(6_250) * 20 <= 6_250,
            "tohum {seed}: ölçülen hat {rate} B/s (gerçek 6 250)"
        );
        assert!(
            o.max_wait_after <= 2 * (MTU + OVH) * SEC / 6_250,
            "tohum {seed}: darboğazda bekleme {}",
            secs(o.max_wait_after)
        );
        assert_eq!(o.drops_after, 0, "tohum {seed}: kuyrukta düşme");
        assert!(
            o.file_done_ns.is_some_and(|t| t <= 60 * SEC),
            "tohum {seed}: dosya {:?}",
            o.file_done_ns
        );
    }
}

/// Profil dürüstken (gerçek = profil) koruma hiçbir örnek kabul etmez: kural #214'teki gibidir.
#[test]
fn honest_profile_guard_stays_off() {
    for seed in 1..=3u64 {
        let o = run(PROFILE_UP, seed, 0);
        println!(
            "HAT-KORUMASI gerçek 12 500 B/s tohum {seed}: dosya={} bekleme_max={} kuyruk_düşen={} \
             koruma={:?}",
            o.file_done_ns.map_or("YOK".to_string(), secs),
            secs(o.max_wait_after),
            o.drops_after,
            o.guard
        );
        assert_eq!(
            o.guard,
            Some((None, 0)),
            "tohum {seed}: koruma devreye girdi"
        );
        assert!(o.file_done_ns.is_some(), "tohum {seed}: dosya bitmedi");
    }
}
