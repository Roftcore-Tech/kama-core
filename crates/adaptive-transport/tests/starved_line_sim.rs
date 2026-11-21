//! [İŞ-34] Geri bildirimi aç profilde hat korumasının karar girdisi (ölçüm aralığı) ve beş PTT
//! iletiminin kaybı: ayrık olaylı, deterministik benzetim (1 ms adım; gerçek `Sender` + gerçek
//! `Receiver`; ürünün geri bildirimi aç profil kurulumu 12 500 / 62 B/s).
//!
//! # Düzenek
//!
//! * İleri hat: hız çizelgesi (serileşme başladığı andaki hız), sınırlı FIFO, 300 ms yayılım,
//!   serileşmeden sonra tohumlu rastgele kayıp (LCG), isteğe bağlı yeniden sıralama (alıcıda bir
//!   datagram bir sonrakinden sonra işlenir) ve hedefli kayıp (bir birimin ilk `n` iletimi).
//! * Dönüş hattı: 62 B/s serileşme, gecikme çizelgesi (varışlar FIFO kalır).
//! * Alıcının ACK bütçesi dönüş hızından ayrı verilebilir (`ack_return_bw`): bitmap kolunun sık
//!   ACK'leri (ürün testi, dönüş-gecikmesi hücresi: ACK aralığı ≈ 1,2–2,5 s).
//! * Ölçüm aralığı (sözleşme #175 7067964700 §2): bu düzenek alıcının yerine hesaplar, çünkü
//!   alıcı tarafı (`receiver.rs`) koordinatörün işidir. M = alıcının aldığı seq'ler, iki ACK
//!   öncesinin en büyük seq'inden bu ACK'in en büyüğüne; `rx_span = ilk_varış(largest) −
//!   ilk_varış(m_first)`, 256 µs'ye aşağı yuvarlanır; m_first = largest ya da 0xFFFE birimi aşan
//!   süre → ölçüm yok. Gönderici girdisi `Sender::note_ack_forward_interval` (sözleşme §3).
//!   `interval = false` kolu bu çağrıyı yapmaz: eski cephe tahmincisi (karşı-olgu).
//!
//! # Ölçülen
//!
//! Darboğazda her datagramın kuyruk beklemesi ve kuyrukta düşmeler, korumanın hız tahmininin
//! zaman izi, PTT demetlerinin SIRALI teslim gecikmesi (ürün PTT şeridi sıralıdır), metnin
//! gecikmesi, dosyanın bitişi, ileri hatta PTT tel baytı.

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, Event, FeedbackStarvedRule, ForwardCopyLineScale, ForwardCopyRule, OpaqueUnit, Sender,
    Timer, TransmitDecision, TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const ACK_SEAL: u64 = 22;
const ONE_WAY_NS: u64 = 300 * MS;
const FILE_UNIT: u64 = 1000;
const FILE_BASE_ID: u64 = 1_000_000;
const TEXT_ID: u64 = 500_000;
const VOICE_UNIT: u64 = 86;
const VOICE_PERIOD_NS: u64 = 200 * MS;
const VOICE_START_NS: u64 = 500 * MS;
/// Ürün kabul ölçütleri (`asym_product_outcome`): PTT demeti ve metin 1 s, dürüst dosya 14 s.
const PTT_BOUND_NS: u64 = SEC;
/// Ürünün PTT kuralı (`client_core::runtime::PTT_FORWARD_COPIES` = 7,
/// `PTT_FORWARD_COPIES_MIN` = 6; aralık 200 ms / 7). 2026-11-16 kuralı: 5 iletim, 40 ms.
const PRODUCT_COPIES: u8 = 6;
const PRODUCT_MIN_COPIES: u8 = 5;
const PRODUCT_SPACING_NS: u64 = 200 * MS / 7;

/// Ürün testinin dönüş-gecikmesi hücresinin çizelgesi (`asym_product_outcome`
/// `Cell::return_delay_varies`).
const REV_DELAY_PLAN: [(u64, u64); 8] = [
    (0, 300 * MS),
    (3 * SEC, 400 * MS),
    (6 * SEC, 500 * MS),
    (9 * SEC, 700 * MS),
    (12 * SEC, 500 * MS),
    (15 * SEC, 300 * MS),
    (18 * SEC, 700 * MS),
    (21 * SEC, 300 * MS),
];

#[derive(Debug, Clone)]
struct Scenario {
    /// İleri hattın hız çizelgesi: (başlangıç anı, B/s tel).
    fwd_plan: Vec<(u64, u64)>,
    /// Dönüş hattının tek yön gecikme çizelgesi: (başlangıç anı, ns).
    rev_delay_plan: Vec<(u64, u64)>,
    loss_permille: u64,
    queue_pkts: usize,
    /// Alıcıda bir datagramın bir sonrakinden sonra işlenme olasılığı (‰).
    reorder_permille: u64,
    /// Hedefli kayıp: bu birimin ilk `n` iletimi serileşmeden sonra düşer.
    targeted_drop: Option<(u64, u32)>,
    /// Dosya birimleri ve ilk sunum anı; `file_gap_ns > 0` ise birimler bu aralıkla tek tek
    /// sunulur (uygulamanın az göndermesi), 0 ise hepsi birden.
    file_units: u64,
    file_at_ns: u64,
    file_gap_ns: u64,
    voice_until_ns: u64,
    text_at_ns: Option<u64>,
    copies: u8,
    spacing_ns: u64,
    copy_scale_min: Option<u8>,
    /// Alıcının ACK bütçesinin hesaplandığı dönüş hızı (B/s).
    ack_return_bw: u64,
    /// Ölçüm aralığı göndericiye verilir mi (`false` = karşı-olgu: eski cephe tahmincisi).
    interval: bool,
    seed: u64,
    t_max_ns: u64,
}

impl Scenario {
    fn honest(seed: u64) -> Self {
        Self {
            fwd_plan: vec![(0, PROFILE_UP)],
            rev_delay_plan: vec![(0, ONE_WAY_NS)],
            loss_permille: 200,
            queue_pkts: 64,
            reorder_permille: 0,
            targeted_drop: None,
            file_units: 41,
            file_at_ns: 0,
            file_gap_ns: 0,
            voice_until_ns: 0,
            text_at_ns: None,
            copies: PRODUCT_COPIES,
            spacing_ns: PRODUCT_SPACING_NS,
            copy_scale_min: Some(PRODUCT_MIN_COPIES),
            ack_return_bw: PROFILE_DOWN,
            interval: true,
            seed,
            t_max_ns: 60 * SEC,
        }
    }
}

struct Bottleneck {
    plan: Vec<(u64, u64)>,
    queue_pkts: usize,
    loss_permille: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64, u64)>,
    lcg: u64,
    dropped_queue: Vec<u64>,
    waits: Vec<(u64, u64)>,
    targeted: Option<(u64, u32)>,
    tx_of_unit: HashMap<u64, u32>,
}

impl Bottleneck {
    fn rate_at(&self, t: u64) -> u64 {
        self.plan
            .iter()
            .rev()
            .find(|&&(from, _)| from <= t)
            .map_or(self.plan[0].1, |&(_, r)| r)
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    fn send(&mut self, now: u64, seq: u64, udp: u64, unit_id: u64) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.queue_pkts {
            self.dropped_queue.push(now);
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + (udp + OVH) * SEC / self.rate_at(start);
        self.busy.push_back(done);
        self.waits.push((now, start - now));
        let n = self.tx_of_unit.entry(unit_id).or_insert(0);
        *n += 1;
        let targeted = self.targeted.is_some_and(|(u, k)| u == unit_id && *n <= k);
        let lost = self.rand_permille() < self.loss_permille;
        if lost || targeted {
            return;
        }
        self.wire.push_back((done + ONE_WAY_NS, seq, unit_id, udp));
    }
}

#[derive(Default)]
struct TxCount {
    datagrams: u64,
    ptt_wire_bytes: u64,
}

struct ReturnLine {
    plan: Vec<(u64, u64)>,
    free_at: u64,
    last_arrival: u64,
    wire: VecDeque<(u64, u64, u16, AckInfo, IntervalMsg)>,
}

/// Ölçüm aralığı (sözleşme §2): (m_first, largest, rx_span, M).
type IntervalMsg = (u64, u64, Option<u64>, Vec<u64>);

impl ReturnLine {
    fn delay_at(&self, t: u64) -> u64 {
        self.plan
            .iter()
            .rev()
            .find(|&&(from, _)| from <= t)
            .map_or(self.plan[0].1, |&(_, d)| d)
    }

    fn send(&mut self, now: u64, udp: u64, arrival_rate: u16, ack: AckInfo, iv: IntervalMsg) {
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        let arrival = (self.free_at + self.delay_at(self.free_at)).max(self.last_arrival);
        self.last_arrival = arrival;
        self.wire.push_back((arrival, udp, arrival_rate, ack, iv));
    }
}

#[derive(Debug)]
struct Outcome {
    file_done_ns: Option<u64>,
    /// (gönderim anı, darboğaz beklemesi).
    waits: Vec<(u64, u64)>,
    drops_queue: usize,
    /// Korumanın hız tahmininin değiştiği anlar: (an, tahmin).
    rate_trace: Vec<(u64, Option<u64>)>,
    guard: Option<(Option<u64>, u64)>,
    /// Demet başına (plan, sıralı teslim gecikmesi); teslim edilmeyen `None`.
    voice: Vec<(u64, Option<u64>)>,
    text_delay_ns: Option<u64>,
    ptt_wire_bytes: u64,
    acks_at: Vec<u64>,
    interval: Option<(u64, u64, Option<i64>)>,
}

impl Outcome {
    fn min_rate(&self) -> Option<u64> {
        self.rate_trace.iter().filter_map(|&(_, r)| r).min()
    }

    fn rate_at_end(&self) -> Option<u64> {
        self.guard.and_then(|g| g.0)
    }

    fn voice_max_from(&self, t: u64) -> u64 {
        self.voice
            .iter()
            .filter(|&&(p, _)| p >= t)
            .map(|&(_, d)| d.unwrap_or(u64::MAX))
            .max()
            .unwrap_or(0)
    }

    fn max_wait_between(&self, from: u64, to: u64) -> u64 {
        self.waits
            .iter()
            .filter(|&&(t, _)| t >= from && t < to)
            .map(|&(_, w)| w)
            .max()
            .unwrap_or(0)
    }
}

fn sender(sc: &Scenario) -> Sender {
    let mut s = Sender::with_cc_port();
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
        below_id: TEXT_ID,
        priority_below_id: FILE_BASE_ID,
        copies: sc.copies,
        spacing_ns: sc.spacing_ns,
    }));
    s.set_forward_copy_line_scale(sc.copy_scale_min.map(|m| ForwardCopyLineScale {
        declared_bps: PROFILE_UP,
        min_copies: m,
    }));
    s
}

#[allow(clippy::too_many_lines)]
fn run(sc: &Scenario) -> Outcome {
    let mut s = sender(sc);
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(sc.ack_return_bw));
    let mut fwd = Bottleneck {
        plan: sc.fwd_plan.clone(),
        queue_pkts: sc.queue_pkts,
        loss_permille: sc.loss_permille,
        busy: VecDeque::new(),
        wire: VecDeque::new(),
        lcg: sc
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407),
        dropped_queue: Vec::new(),
        waits: Vec::new(),
        targeted: sc.targeted_drop,
        tx_of_unit: HashMap::new(),
    };
    let mut reorder_lcg = sc.seed ^ 0x5A5A_5A5A;
    let mut ret = ReturnLine {
        plan: sc.rev_delay_plan.clone(),
        free_at: 0,
        last_arrival: 0,
        wire: VecDeque::new(),
    };
    let mut first_arrival: HashMap<u64, u64> = HashMap::new();
    let mut received: Vec<u64> = Vec::new();
    let mut ack_fronts: VecDeque<u64> = VecDeque::new();
    let mut held: Option<(u64, u64, u64)> = None;
    let mut voice_offered: Vec<u64> = Vec::new();
    let mut voice_arrived: HashMap<u64, u64> = HashMap::new();
    let mut text_arrived: Option<u64> = None;
    let mut file_seen: HashSet<u64> = HashSet::new();
    let mut file_offered = 0u64;
    let mut file_done_ns = None;
    let mut tx = TxCount::default();
    let mut rate_trace: Vec<(u64, Option<u64>)> = Vec::new();
    let mut acks_at = Vec::new();
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Bottleneck, tx: &mut TxCount| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            tx.datagrams += 1;
            if d.unit_id < TEXT_ID {
                tx.ptt_wire_bytes += d.size + OVH;
            }
            fwd.send(now, d.seq, d.size, d.unit_id);
        }
    };
    let mut now = 0u64;
    while now < sc.t_max_ns {
        // Uygulama.
        let file_due = if sc.file_gap_ns == 0 {
            if now >= sc.file_at_ns {
                sc.file_units
            } else {
                0
            }
        } else if now >= sc.file_at_ns {
            ((now - sc.file_at_ns) / sc.file_gap_ns + 1).min(sc.file_units)
        } else {
            0
        };
        while file_offered < file_due {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: FILE_BASE_ID + file_offered,
                    size: FILE_UNIT,
                }),
                now,
            );
            file_offered += 1;
            emit(decs, now, &mut fwd, &mut tx);
        }
        let next_voice = voice_offered.len() as u64;
        let voice_at = VOICE_START_NS + next_voice * VOICE_PERIOD_NS;
        if voice_at < sc.voice_until_ns && now >= voice_at {
            voice_offered.push(voice_at);
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: VOICE_UNIT,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx);
        }
        if sc.text_at_ns == Some(now) {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: TEXT_ID,
                    size: 80,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx);
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut tx);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut tx);
        }
        // Alıcı.
        r.set_uplink_yield(ret.free_at > now);
        let mut arrived = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, udp) = fwd.wire.pop_front().expect("var");
            arrived.push((seq, uid, udp));
        }
        let mut deliver = Vec::new();
        for a in arrived {
            if let Some(h) = held.take() {
                deliver.push(a);
                deliver.push(h);
                continue;
            }
            reorder_lcg = reorder_lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            if (reorder_lcg >> 33) % 1000 < sc.reorder_permille {
                held = Some(a);
            } else {
                deliver.push(a);
            }
        }
        let mut acks = Vec::new();
        for (seq, uid, udp) in deliver {
            if let std::collections::hash_map::Entry::Vacant(e) = first_arrival.entry(seq) {
                e.insert(now);
                received.push(seq);
            }
            r.set_next_datagram_bytes(udp);
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: RxKind::Data,
                },
                now,
            );
            for u in out.delivered {
                if u >= FILE_BASE_ID {
                    file_seen.insert(u);
                    if file_seen.len() as u64 == sc.file_units && file_done_ns.is_none() {
                        file_done_ns = Some(now);
                    }
                } else if u == TEXT_ID {
                    text_arrived.get_or_insert(now);
                } else {
                    voice_arrived.entry(u).or_insert(now);
                }
            }
            acks.extend(out.ack);
        }
        acks.extend(r.on_ack_timer(now));
        for frame in acks {
            let largest = r.largest_seq().unwrap_or(0);
            let expected = largest + 1;
            let udp = frame.encode().len() as u64 + ACK_SEAL;
            // Ölçüm aralığı (sözleşme §2): iki ACK öncesinin cephesinden bu ACK'in en büyüğüne.
            let base = if ack_fronts.len() >= 2 {
                ack_fronts[ack_fronts.len() - 2]
            } else {
                0
            };
            let mut m: Vec<u64> = received
                .iter()
                .copied()
                .filter(|&q| q >= base && q <= largest)
                .collect();
            m.sort_unstable();
            let m_first = m.first().copied().unwrap_or(largest);
            let rx_span = (m_first != largest)
                .then(|| first_arrival[&largest].saturating_sub(first_arrival[&m_first]))
                .map(|d| d / 256_000)
                .filter(|&u| u <= 0xFFFE)
                .map(|u| u * 256_000);
            if ack_fronts.len() >= 3 {
                ack_fronts.pop_front();
            }
            ack_fronts.push_back(largest);
            ret.send(
                now,
                udp,
                frame.arrival_rate,
                AckInfo {
                    acked_seqs: frame.acked_full(expected),
                    ack_delay: frame.ack_delay_duration(),
                },
                (m_first, largest, rx_span, m),
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _, _, _)| t <= now) {
            let (_, udp, arrival_rate, ack, (first, last, rx_span, m)) =
                ret.wire.pop_front().expect("var");
            acks_at.push(now);
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: tx.datagrams,
                },
                now,
            );
            s.note_ack_arrival_rate(arrival_rate);
            if sc.interval {
                s.note_ack_forward_interval(first, last, rx_span, &m);
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut tx);
            let g = s.diag_line_guard().and_then(|g| g.0);
            if rate_trace.last().map(|&(_, r)| r) != Some(g) {
                rate_trace.push((now, g));
            }
        }
        let voice_left = (0..voice_offered.len() as u64).any(|v| !voice_arrived.contains_key(&v));
        let text_left = sc.text_at_ns.is_some_and(|t| now >= t) && text_arrived.is_none();
        let file_left = file_done_ns.is_none() && sc.file_units > 0;
        if now >= sc.voice_until_ns && !voice_left && !text_left && !file_left {
            break;
        }
        now += MS;
    }
    // Sıralı teslim: demet k, 0..=k'nin hepsi geldiğinde çalınır.
    let mut played_at = 0u64;
    let mut voice = Vec::new();
    let mut gap = false;
    for (k, &planned) in voice_offered.iter().enumerate() {
        match voice_arrived.get(&(k as u64)) {
            Some(&t) if !gap => {
                played_at = played_at.max(t);
                voice.push((planned, Some(played_at - planned)));
            }
            _ => {
                gap = true;
                voice.push((planned, None));
            }
        }
    }
    Outcome {
        file_done_ns,
        waits: fwd.waits.clone(),
        drops_queue: fwd.dropped_queue.len(),
        rate_trace,
        guard: s.diag_line_guard(),
        voice,
        text_delay_ns: sc.text_at_ns.zip(text_arrived).map(|(a, b)| b - a),
        ptt_wire_bytes: tx.ptt_wire_bytes,
        acks_at,
        interval: s.diag_line_guard_interval(),
    }
}

fn secs(ns: u64) -> String {
    if ns == u64::MAX {
        return "YOK".to_string();
    }
    format!("{:.2} s", ns as f64 / SEC as f64)
}

fn report(tag: &str, sc: &Scenario, o: &Outcome) {
    println!(
        "SİM {tag} tohum {} aralık={} dosya={} kuyruk_max={} kuyruk_düşen={} hız_izi={:?} \
         koruma={:?} ptt_max={} metin={} ptt_tel={} B ack={}",
        sc.seed,
        sc.interval,
        o.file_done_ns.map_or("YOK".to_string(), secs),
        secs(o.waits.iter().map(|&(_, w)| w).max().unwrap_or(0)),
        o.drops_queue,
        o.rate_trace
            .iter()
            .map(|&(t, r)| format!(
                "{}:{}",
                secs(t),
                r.map_or("-".to_string(), |r| r.to_string())
            ))
            .collect::<Vec<_>>(),
        o.guard,
        secs(o.voice_max_from(0)),
        o.text_delay_ns.map_or("-".to_string(), secs),
        o.ptt_wire_bytes,
        o.acks_at.len(),
    );
}

/// Korumanın hiç aşağı yönlü örnek almadığı (ya da beyanın %5'inden fazla indirmediği) koşu.
fn never_cut(o: &Outcome) -> bool {
    o.min_rate().is_none_or(|m| m * 20 >= PROFILE_UP * 19)
}

/// (a) İleri hat sabit (dürüst 12 500), yalnız dönüş gecikmesi artıyor (ürün hücresinin çizelgesi),
/// ACK'ler sık (bitmap kolu), dosya erken biter ve gönderici sonra yalnız PTT gönderir (tavanın
/// altında): kapasite düşürülmez.
///
/// Karşı-olgu (aynı koşu, ölçüm aralığı verilmeden: eski cephe tahmincisi): en az bir tohumda hat
/// beyanın %5'inden fazla düşürülür. Bu kol senaryonun kusuru gerçekten ürettiğini gösterir.
#[test]
fn a_return_delay_rise_does_not_cut_an_honest_line() {
    let mut control_cut = false;
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("a/aday", &sc, &o);
        assert!(
            never_cut(&o),
            "tohum {seed}: dürüst hat düşürüldü: {:?}",
            o.rate_trace
        );
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("a/karşı-olgu", &control, &oc);
        control_cut |= !never_cut(&oc);
    }
    assert!(
        control_cut,
        "karşı-olgu: eski tahminci bu senaryoda kesmedi"
    );
}

/// Bir koşunun ölçülen hatta yakınsadığı ilk an: tahmin `real`'in %10 içine girdiği an.
fn converged_at(o: &Outcome, real: u64) -> Option<u64> {
    o.rate_trace
        .iter()
        .find(|&&(_, r)| r.is_some_and(|r| r * 10 <= real * 11 && r * 10 >= real * 9))
        .map(|&(t, _)| t)
}

const DROP_AT_NS: u64 = 8 * SEC;
const DROP_TO: u64 = 6_250;

fn drop_scenario(seed: u64, interval: bool) -> Scenario {
    Scenario {
        fwd_plan: vec![(0, PROFILE_UP), (DROP_AT_NS, DROP_TO)],
        file_units: 150,
        voice_until_ns: 40 * SEC,
        ack_return_bw: 4 * PROFILE_DOWN,
        interval,
        ..Scenario::honest(seed)
    }
}

/// (b) Gerçek ileri kapasite 8 s'de 12 500'den 6 250'ye düşüyor (gönderici dosyayla tavanda):
/// daralma algılanır ve hat ölçülen hızda kalır.
///
/// Sınırlar (varsayımlarla): ACK aralığı `T_ack` = düşüşten önceki en büyük ACK aralığı
/// (göndericide; alıcının ACK kadansı, bu düzenekte ≈ 2,1 s), dönüş geçişi `T_ret` = en büyük
/// ACK'in (129 B + 28) 62 B/s'de serileşmesi + 300 ms. İlk yazımda `T_ack` bütçe formülündendi
/// (`budgeted_ack_interval_ns`, ≈ 1,3 s); düzeneğin gerçek kadansı daha seyrek çıktı, formül aynı
/// kaldı. Düşüşten sonra M'si tamamen düşüş sonrası varışlardan oluşan ilk ACK en
/// geç üç ACK aralığı + bir dönüş geçişi sonra gelir:
/// * yakınsama (tahmin gerçek hattın %10 içinde) ≤ düşüş + 3·T_ack + T_ret;
/// * yakınsamadan 2 s sonra darboğaz beklemesi ≤ iki tam datagramın gerçek hatta serileşmesi;
/// * yakınsamadan 9 s sonra planlanan PTT demetleri sıralı teslimde ≤ 1 s (düşüş anında kuyrukta
///   düşen demetlerin ARQ kurtarması iki ACK turu sürer).
///
/// Düşüşten yakınsamaya dek kuyruk FIFO'yla (64 paket) sınırlıdır; bu aralıkta PTT 1 s'yi aşar.
/// Bu, kullanılan geri bildirim gecikmesinin (≈ 3–4 s) ve FIFO modelinin sonucudur, bütün
/// politikalar için imkânsızlık hükmü değildir.
///
/// Karşı-olgu: eski cephe tahmincisi aynı koşuda yakınsama sınırını üç tohumda da aşar (tahmin
/// düşüşten 24–26 s sonra oturur).
#[test]
fn b_real_capacity_drop_is_detected_and_the_queue_drains() {
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    for seed in 1..=3u64 {
        let sc = drop_scenario(seed, true);
        let o = run(&sc);
        report("b/aday", &sc, &o);
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max()
            .expect("düşüşten önce ACK'ler");
        let conv_bound = DROP_AT_NS + 3 * t_ack + t_ret;
        let conv = converged_at(&o, DROP_TO).expect("yakınsadı");
        assert!(
            conv <= conv_bound,
            "tohum {seed}: yakınsama {} (sınır {})",
            secs(conv),
            secs(conv_bound)
        );
        let end = o.rate_at_end().expect("ölçüldü");
        assert!(end * 10 <= DROP_TO * 11, "tohum {seed}: son tahmin {end}");
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        assert!(
            w <= wait_bound,
            "tohum {seed}: yakınsama sonrası bekleme {}",
            secs(w)
        );
        let ptt = o.voice_max_from(conv + 9 * SEC);
        assert!(
            ptt <= PTT_BOUND_NS,
            "tohum {seed}: yakınsama sonrası PTT {}",
            secs(ptt)
        );
        let control = run(&drop_scenario(seed, false));
        report("b/karşı-olgu", &drop_scenario(seed, false), &control);
        let c_conv = converged_at(&control, DROP_TO).unwrap_or(u64::MAX);
        assert!(
            c_conv > conv_bound,
            "karşı-olgu tohum {seed}: eski tahminci de sınırda yakınsadı ({})",
            secs(c_conv)
        );
    }
}

/// (c) Gönderici kapasitenin altında sunuyor (dosya birimleri 400 ms'de bir, ≈ 2,6 kB/s, artı
/// PTT), dönüş gecikmesi artıyor: düşük sunum hızı hat kapasitesi sanılmaz.
///
/// Karşı-olgu: eski cephe tahmincisi hattı üç tohumda da sunum hızına indirir.
#[test]
fn c_a_low_offered_rate_is_not_taken_as_capacity() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            file_units: 30,
            file_gap_ns: 400 * MS,
            voice_until_ns: 22 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("c/aday", &sc, &o);
        assert!(never_cut(&o), "tohum {seed}: {:?}", o.rate_trace);
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("c/karşı-olgu", &control, &oc);
        assert!(
            !never_cut(&oc),
            "karşı-olgu tohum {seed}: eski tahminci kesmedi"
        );
    }
}

/// (d-1) %20 rastgele kayıp, dürüst hat, gönderici tavanda, dönüş gecikmesi sabit: kayıp alınan
/// baytı düşürür ama kuyruk artışı üretmez; ölçüm geçerli ve hat düşürülmez. Davranışı gösterir
/// (eski tahminci de bu koşuda kesmez: gönderimin baytıyla sayar).
#[test]
fn d1_random_loss_is_not_queue_growth() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            file_units: 120,
            voice_until_ns: 15 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("d1/aday", &sc, &o);
        let (checks, samples, _) = o.interval.expect("kural kurulu");
        assert!(checks >= 5, "tohum {seed}: ölçülen aralık {checks}");
        assert_eq!(samples, 0, "tohum {seed}: rastgele kayıp örnek verdi");
        assert!(never_cut(&o), "tohum {seed}: {:?}", o.rate_trace);
    }
}

/// (d-2) Kuyruk taşması: (b)'nin düşüşünde 64 paketlik FIFO dolar ve kuyrukta datagram düşer. M'de
/// kuyrukta düşen seq'ler varış süresine girmez, tahmin göndericinin baytıyla yapılır (taşmanın
/// olduğu aralıkta tahmin gerçek hattın üstünde kalır). Davranış: taşma olur, yine de tahmin
/// gerçek hatta oturur ve bir daha taşma olmaz.
#[test]
fn d2_queue_overflow_still_converges_and_stops_overflowing() {
    for seed in 1..=3u64 {
        let sc = drop_scenario(seed, true);
        let o = run(&sc);
        assert!(
            o.drops_queue > 0,
            "tohum {seed}: taşma olmadı (senaryo sınamıyor)"
        );
        let conv = converged_at(&o, DROP_TO).expect("yakınsadı");
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        let fifo_ns = 64 * (VOICE_UNIT + OVH) * SEC / DROP_TO;
        assert!(
            w < fifo_ns,
            "tohum {seed}: yakınsamadan sonra kuyruk dolu ({})",
            secs(w)
        );
    }
}

/// (d-3) Yeniden sıralama (%3: bir datagram bir sonrakinden sonra işlenir): m_first geç gelirse
/// `rx_span` kısalır, ölçüm daralmayı küçük gösterir (güvenli taraf). Dürüst hatta dönüş
/// gecikmesi artarken hat düşürülmez; gerçek 6 250'de koruma yine ölçülen hatta oturur.
///
/// Karşı-olgu (dürüst hat): eski cephe tahmincisi üç tohumda da keser.
#[test]
fn d3_reordering_keeps_the_measurement_on_the_safe_side() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            reorder_permille: 30,
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("d3/aday", &sc, &o);
        assert!(never_cut(&o), "tohum {seed}: {:?}", o.rate_trace);
        let control = Scenario {
            interval: false,
            ..sc.clone()
        };
        let oc = run(&control);
        report("d3/karşı-olgu", &control, &oc);
        assert!(
            !never_cut(&oc),
            "karşı-olgu tohum {seed}: eski tahminci kesmedi"
        );
        let slow = Scenario {
            fwd_plan: vec![(0, DROP_TO)],
            file_units: 80,
            voice_until_ns: 20 * SEC,
            rev_delay_plan: vec![(0, ONE_WAY_NS)],
            ..sc
        };
        let os = run(&slow);
        report("d3/6250", &slow, &os);
        let end = os.rate_at_end().expect("ölçüldü");
        assert!(
            end * 10 <= DROP_TO * 11,
            "tohum {seed}: 6 250'de son tahmin {end}"
        );
    }
}
