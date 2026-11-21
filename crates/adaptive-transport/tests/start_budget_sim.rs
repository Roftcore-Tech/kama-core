//! [İŞ-33 C1 · C4 · C5] Başlangıç bütçesi ve ilk ACK'in varış penceresi: geri bildirimi aç profil
//! gerçek ileri hattan iyimserken ilk ACK'e dek darboğaz kuyruğu kuyruk payını aşmaz, ilk ölçüm
//! ilk ACK'te gelir; profil dürüstken ilk ACK hattı doğrular ve koruma devreye girmez.
//!
//! Düzenek `tests/line_guard_sim.rs` ile aynıdır (ayrık olay, 1 ms adım, deterministik; gerçek
//! `Sender` + gerçek `Receiver`; ürünün geri bildirimi aç profil kurulumu 12 500 / 62 B/s; ileri
//! hat 64 paket FIFO, 300 ms, serileşmeden sonra %20 tohumlu kayıp; dönüş 62 B/s, 300 ms). Farklar
//! ürün çalışma zamanının yeni iki girişidir:
//! * alıcıya her datagramdan önce UDP gövde boyu verilir (`Receiver::set_next_datagram_bytes`;
//!   ürün `recv_from` boyu) → ilk ACK ilk varış penceresinin hızını taşır;
//! * göndericiye her ACK'ten önce çerçevenin `arrival_rate` alanı verilir
//!   (`Sender::note_ack_arrival_rate`; ürün ACK çözme yolu).
//!
//! `old_receiver` kolu boy vermez: eski alıcı gibi alan daima sentinel'dir (uyum).
//!
//! Ölçülen: darboğazda her datagramın kuyruk beklemesi, kuyrukta düşmeler, kısa birimlerin
//! (ürünün PTT demeti) SIRALI teslim gecikmesi (ürün PTT şeridi sıralıdır: demet `k` ancak
//! `0..=k` geldikten sonra çalınır), dosyanın bitişi, ilk pencerenin sonucu.

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::line_guard::{FirstWindowOutcome, START_BURST_NS};
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
/// Ürün testinin başlangıç evresi (`asym_product_outcome` `START_PHASE`).
const START_PHASE_NS: u64 = 8 * SEC;
/// Ürün testinin PTT sınırı (`PTT_BOUND`).
const PTT_BOUND_NS: u64 = SEC;

struct Bottleneck {
    rate_bps: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64, u64)>,
    lcg: u64,
    dropped_queue: Vec<u64>,
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
        self.wire.push_back((done + ONE_WAY_NS, seq, unit_id, udp));
    }
}

struct ReturnLine {
    free_at: u64,
    wire: VecDeque<(u64, u64, u16, AckInfo)>,
}

impl ReturnLine {
    fn send(&mut self, now: u64, udp: u64, arrival_rate: u16, ack: AckInfo) {
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        self.wire
            .push_back((self.free_at + ONE_WAY_NS, udp, arrival_rate, ack));
    }
}

#[derive(Debug)]
struct Outcome {
    file_done_ns: Option<u64>,
    /// Dosyanın son yeni biriminin ilk iletim anı (bütçenin dosyaya bedeli; ARQ'dan bağımsız).
    file_last_new_sent_ns: Option<u64>,
    first_ack_ns: Option<u64>,
    /// İlk ACK'in göndericiye varışından ÖNCE gönderilen datagramların en uzun kuyruk beklemesi.
    max_wait_before_first_ack: u64,
    max_wait: u64,
    drops: usize,
    /// Kısa birimlerin sıralı teslim gecikmesinin en büyüğü (ns) ve 1 s'yi aşan birim sayısı.
    voice_max: u64,
    /// Başlangıç evresinde (plan < [`START_PHASE_NS`]) planlanan demetlerin en büyük sıralı teslim
    /// gecikmesi (ns).
    voice_max_start: u64,
    voice_over_1s: usize,
    voice_undelivered: usize,
    /// 1 s'yi aşan ilk demetler: (k, plan, kendi varışı, çalınma) (gözlem).
    voice_late: Vec<(u64, u64, u64, u64)>,
    first_window: Option<FirstWindowOutcome>,
    first_window_detail: Option<adaptive_transport::line_guard::FirstWindowDetail>,
    guard: Option<(Option<u64>, u64)>,
}

fn sender() -> Sender {
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
        below_id: FILE_BASE_ID,
        priority_below_id: FILE_BASE_ID,
        copies: 4,
        spacing_ns: 40 * MS,
    }));
    s
}

#[allow(clippy::too_many_lines)]
fn run(real_up: u64, seed: u64, new_receiver: bool) -> Outcome {
    let mut s = sender();
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    let mut fwd = Bottleneck::new(real_up, seed);
    let mut ret = ReturnLine {
        free_at: 0,
        wire: VecDeque::new(),
    };
    let mut voice_offered: Vec<u64> = Vec::new();
    let mut voice_arrived: HashMap<u64, u64> = HashMap::new();
    let mut file_seen: HashSet<u64> = HashSet::new();
    let mut file_done_ns = None;
    let mut file_last_new_sent_ns = None;
    let mut file_new_sent = 0u64;
    let mut first_ack_ns = None;
    let mut tx_datagrams = 0u64;
    let emit = |decs: Vec<TransmitDecision>,
                now: u64,
                fwd: &mut Bottleneck,
                tx: &mut u64,
                file_new_sent: &mut u64,
                file_last: &mut Option<u64>| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            *tx += 1;
            if d.kind == TransmitKind::Data && d.unit_id >= FILE_BASE_ID {
                *file_new_sent += 1;
                if *file_new_sent == FILE_UNITS {
                    *file_last = Some(now);
                }
            }
            fwd.send(now, d.seq, d.size, d.unit_id);
        }
    };
    for i in 0..FILE_UNITS {
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id: FILE_BASE_ID + i,
                size: FILE_UNIT,
            }),
            0,
        );
        emit(
            decs,
            0,
            &mut fwd,
            &mut tx_datagrams,
            &mut file_new_sent,
            &mut file_last_new_sent_ns,
        );
    }
    let mut now = 0u64;
    while now < T_MAX_NS {
        let next_voice = voice_offered.len() as u64;
        let voice_at = VOICE_START_NS + next_voice * VOICE_PERIOD_NS;
        if file_done_ns.is_none() && now >= voice_at {
            voice_offered.push(voice_at);
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: VOICE_UNIT,
                }),
                now,
            );
            emit(
                decs,
                now,
                &mut fwd,
                &mut tx_datagrams,
                &mut file_new_sent,
                &mut file_last_new_sent_ns,
            );
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(
            decs,
            now,
            &mut fwd,
            &mut tx_datagrams,
            &mut file_new_sent,
            &mut file_last_new_sent_ns,
        );
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(
                decs,
                now,
                &mut fwd,
                &mut tx_datagrams,
                &mut file_new_sent,
                &mut file_last_new_sent_ns,
            );
        }
        r.set_uplink_yield(ret.free_at > now);
        let mut acks = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, udp) = fwd.wire.pop_front().expect("var");
            if new_receiver {
                r.set_next_datagram_bytes(udp);
            }
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
                    if file_seen.len() as u64 == FILE_UNITS && file_done_ns.is_none() {
                        file_done_ns = Some(now);
                    }
                } else {
                    voice_arrived.entry(u).or_insert(now);
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
                frame.arrival_rate,
                AckInfo {
                    acked_seqs: frame.acked_full(expected),
                    ack_delay: frame.ack_delay_duration(),
                },
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, udp, arrival_rate, ack) = ret.wire.pop_front().expect("var");
            first_ack_ns.get_or_insert(now);
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: tx_datagrams,
                },
                now,
            );
            s.note_ack_arrival_rate(arrival_rate);
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(
                decs,
                now,
                &mut fwd,
                &mut tx_datagrams,
                &mut file_new_sent,
                &mut file_last_new_sent_ns,
            );
        }
        let voice_left = (0..voice_offered.len() as u64).any(|v| !voice_arrived.contains_key(&v));
        if file_done_ns.is_some() && !voice_left {
            break;
        }
        now += MS;
    }
    // Sıralı teslim: demet k, 0..=k'nin hepsi geldiğinde çalınır.
    let mut played_at = 0u64;
    let mut voice_max = 0u64;
    let mut voice_max_start = 0u64;
    let mut voice_over_1s = 0usize;
    let mut voice_undelivered = 0usize;
    let mut voice_late = Vec::new();
    for (k, &planned) in voice_offered.iter().enumerate() {
        match voice_arrived.get(&(k as u64)) {
            Some(&t) if voice_undelivered == 0 => {
                played_at = played_at.max(t);
                let delay = played_at - planned;
                voice_max = voice_max.max(delay);
                if planned < START_PHASE_NS {
                    voice_max_start = voice_max_start.max(delay);
                }
                if delay > SEC {
                    voice_over_1s += 1;
                    if voice_late.len() < 6 {
                        voice_late.push((k as u64, planned, t, played_at));
                    }
                }
            }
            _ => voice_undelivered += 1,
        }
    }
    let first = first_ack_ns.unwrap_or(u64::MAX);
    Outcome {
        file_done_ns,
        file_last_new_sent_ns,
        first_ack_ns,
        max_wait_before_first_ack: fwd
            .waits
            .iter()
            .filter(|&&(t, _)| t < first)
            .map(|&(_, w)| w)
            .max()
            .unwrap_or(0),
        max_wait: fwd.waits.iter().map(|&(_, w)| w).max().unwrap_or(0),
        drops: fwd.dropped_queue.len(),
        voice_max,
        voice_max_start,
        voice_over_1s,
        voice_undelivered,
        voice_late,
        first_window: s.diag_line_guard_first_window(),
        first_window_detail: s.diag_line_guard_first_window_detail(),
        guard: s.diag_line_guard(),
    }
}

fn secs(ns: u64) -> String {
    format!("{:.2} s", ns as f64 / SEC as f64)
}

fn opt_secs(ns: Option<u64>) -> String {
    ns.map_or("YOK".to_string(), secs)
}

fn report(tag: &str, real_up: u64, seed: u64, o: &Outcome) {
    println!(
        "BAŞLANGIÇ-SİM {tag} gerçek {real_up} tohum {seed}: ilk_ACK={} ilk_pencere={:?} {:?} \
         koruma={:?} bekleme_max(ilk ACK öncesi)={} bekleme_max={} kuyruk_düşen={} \
         ptt_sıralı_max={} ptt_başlangıç_max={} ptt_1s_üstü={} ptt_teslimsiz={} dosya_son_yeni={} \
         dosya={} geç={}",
        opt_secs(o.first_ack_ns),
        o.first_window,
        o.first_window_detail,
        o.guard,
        secs(o.max_wait_before_first_ack),
        secs(o.max_wait),
        o.drops,
        secs(o.voice_max),
        secs(o.voice_max_start),
        o.voice_over_1s,
        o.voice_undelivered,
        opt_secs(o.file_last_new_sent_ns),
        opt_secs(o.file_done_ns),
        o.voice_late
            .iter()
            .map(|&(k, p, t, pl)| format!("#{k}@{}→{}/{}", secs(p), secs(t), secs(pl)))
            .collect::<Vec<_>>()
            .join(" "),
    );
}

/// Gerçek hat beyanın yarısıyken (P/2) ilk ACK'e dek kurulabilecek en büyük kuyruk (ns):
/// patlamanın sonunda `START_BURST_NS × (P − gerçek) / gerçek`, artı bir tam datagramın gerçek
/// hatta serileşmesi (FIFO çözünürlüğü).
fn start_queue_bound_ns(real_up: u64) -> u64 {
    START_BURST_NS * (PROFILE_UP - real_up) / real_up + (MTU + OVH) * SEC / real_up
}

/// Profil iyimserken (gerçek 6 250 ve 10 000 B/s, beyan 12 500): ilk ACK ilk varış penceresiyle
/// hattı ölçer (±%2), ilk ACK'e dek ve bütün oturumda darboğaz kuyruğu başlangıç sınırını aşmaz,
/// kuyrukta düşme yok, başlangıç evresinde (ürün `START_PHASE` = 8 s) PTT demetleri sıralı
/// teslimde 1 s içinde.
///
/// Sınırlar ölçümden önce, fizikten (`start_queue_bound_ns`): 6 250'de 0,5 + 0,2 s, 10 000'de
/// 0,125 + 0,12 s. Ölçülen (2026-11-21, tohum 1–3): 6 250'de 0,49 s, 10 000'de 0,12 s.
#[test]
fn optimistic_profile_is_measured_on_the_first_ack_and_the_start_queue_stays_bounded() {
    for real_up in [6_250u64, 10_000] {
        for seed in 1..=3u64 {
            let o = run(real_up, seed, true);
            report("yeni-alıcı", real_up, seed, &o);
            let Some(FirstWindowOutcome::Sample(est)) = o.first_window else {
                panic!(
                    "gerçek {real_up} tohum {seed}: ilk pencere {:?}",
                    o.first_window
                );
            };
            assert!(
                est.abs_diff(real_up) * 50 <= real_up,
                "gerçek {real_up} tohum {seed}: ilk pencere tahmini {est}"
            );
            let bound = start_queue_bound_ns(real_up);
            assert!(
                o.max_wait_before_first_ack <= bound && o.max_wait <= bound,
                "gerçek {real_up} tohum {seed}: kuyruk {} / {} (sınır {})",
                secs(o.max_wait_before_first_ack),
                secs(o.max_wait),
                secs(bound)
            );
            assert_eq!(o.drops, 0, "gerçek {real_up} tohum {seed}: kuyrukta düşme");
            assert!(
                o.voice_max_start <= PTT_BOUND_NS,
                "gerçek {real_up} tohum {seed}: başlangıç PTT {}",
                secs(o.voice_max_start)
            );
            assert_eq!(o.voice_undelivered, 0);
            assert!(
                o.file_done_ns.is_some(),
                "gerçek {real_up} tohum {seed}: dosya bitmedi"
            );
        }
    }
}

/// Profil dürüstken: ilk ACK hattı doğrular, koruma hiç devreye girmez, kuyruk yok. Bütçenin
/// dosyaya bedeli ilk ACK'in gelişine dek bekleyen gönderimle sınırlıdır: dosyanın son yeni
/// birimi en geç `ilk ACK + (dosya + PTT tel baytı) / profil` anında çıkar (bütçe ilk ACK'ten önce
/// hiç gönderim yaptırmasaydı). Ölçülen (2026-11-21, tohum 1–3): 5,35–5,95 s; sınır ≈ 6,2–6,5 s.
#[test]
fn honest_profile_is_verified_on_the_first_ack_and_the_budget_costs_at_most_the_wait() {
    for seed in 1..=3u64 {
        let o = run(PROFILE_UP, seed, true);
        report("yeni-alıcı", PROFILE_UP, seed, &o);
        assert_eq!(
            o.first_window,
            Some(FirstWindowOutcome::Verified),
            "tohum {seed}"
        );
        assert_eq!(
            o.guard,
            Some((None, 0)),
            "tohum {seed}: koruma devreye girdi"
        );
        assert!(
            o.max_wait <= (MTU + OVH) * SEC / PROFILE_UP,
            "tohum {seed}: kuyruk"
        );
        assert_eq!(o.drops, 0);
        let first_ack = o.first_ack_ns.expect("ilk ACK geldi");
        let file_wire = FILE_UNITS * (FILE_UNIT + OVH);
        // PTT: 200 ms'de bir demet, orijinal + dört kopya.
        let ptt_wire_bps = 5 * (VOICE_UNIT + OVH) * SEC / VOICE_PERIOD_NS;
        let bound = first_ack + file_wire * SEC / (PROFILE_UP - ptt_wire_bps);
        let last_new = o.file_last_new_sent_ns.expect("dosya gönderildi");
        assert!(
            last_new <= bound,
            "tohum {seed}: dosyanın son yeni birimi {} (sınır {})",
            secs(last_new),
            secs(bound)
        );
        assert!(
            o.voice_max <= PTT_BOUND_NS,
            "tohum {seed}: PTT {}",
            secs(o.voice_max)
        );
    }
}

/// Eski alıcı (boy verilmez, alan daima sentinel): ilk pencere yok, bütçe ilk ACK'le kalkar ve
/// gönderici bugünkü hat korumasına döner; ilk ACK'e dek kuyruk yine başlangıç sınırında.
#[test]
fn old_receiver_falls_back_to_todays_guard() {
    for real_up in [6_250u64, PROFILE_UP] {
        for seed in 1..=3u64 {
            let o = run(real_up, seed, false);
            report("eski-alıcı", real_up, seed, &o);
            assert_eq!(o.first_window, None, "gerçek {real_up} tohum {seed}");
            assert!(
                o.max_wait_before_first_ack <= start_queue_bound_ns(real_up),
                "gerçek {real_up} tohum {seed}: ilk ACK öncesi kuyruk {}",
                secs(o.max_wait_before_first_ack)
            );
            assert!(
                o.file_done_ns.is_some(),
                "gerçek {real_up} tohum {seed}: dosya bitmedi"
            );
        }
    }
}
