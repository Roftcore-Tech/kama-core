//! [İZ-ARALIK] Hat korumasının ACK başına karar kaydı (`Sender::diag_line_guard_trace`): sınırlı
//! halka, karar sınıfının korumanın kendi sayaçlarıyla tutarlılığı, salt gözlem (kayıt açık /
//! kapalı aynı kararlar) ve kuralsız yolda kayıt yok.
//!
//! Düzenek: gerçek `Sender` (ürünün geri bildirimi aç profil kurulumu 12 500 / 62 B/s) ve
//! belirlenimli, kayıpsız bir ileri hat (serileşme gerçek hızda, FIFO, 300 ms yayılım). Alıcının
//! yerine düzenek ACK üretir (sabit aralıkla; M = iki ACK öncesinin en büyük seq'i + 1'den bu ACK'in
//! en büyüğüne dek alınanlar; `rx_span` = ilk_varış(largest) − ilk_varış(m_first), 256 µs aşağı —
//! sözleşme #175 7067964700 §2, gerçek alıcının `bitmap_base` kuralı). ACK göndericiye 300 ms +
//! dönüş serileşmesi sonra varır; `ack_delay` = ACK anı − en büyük seq'in varışı.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::feedback_horizon_ns;
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, Event, FeedbackStarvedRule, LineGuardAckDecision, LineGuardAckTrace, OpaqueUnit,
    Sender, Timer, TransmitKind, LINE_GUARD_TRACE_CAP,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const ONE_WAY_NS: u64 = 300 * MS;
/// ACK datagramının UDP gövdesi (B; bitmap çerçevesi + mühür, ürün ölçümlerinin aralığında).
const ACK_UDP: u64 = 60;
/// ACK'in göndericiye varışı: dönüş serileşmesi (zarf dahil) + yayılım.
const ACK_FLIGHT_NS: u64 = (ACK_UDP + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
const FILE_UNIT: u64 = 1000;

#[derive(Clone, Copy)]
struct Scenario {
    /// Gerçek ileri hat (B/s, tel).
    real_bps: u64,
    /// Ölçüm aralığı göndericiye verilir mi (`false` = eski alıcı: cephe tahmincisi).
    interval: bool,
    /// Geri bildirimi aç kural (hat koruması) kurulu mu.
    rule: bool,
    /// Kayıt açık mı.
    trace: bool,
    /// Dosya birimleri hepsi t = 0'da (`app_gap_ns == 0`) ya da bu aralıkla tek tek.
    units: u64,
    app_gap_ns: u64,
    ack_period_ns: u64,
    t_max_ns: u64,
}

impl Scenario {
    fn narrow() -> Self {
        Self {
            real_bps: 6_250,
            interval: true,
            rule: true,
            trace: true,
            units: 150,
            app_gap_ns: 0,
            ack_period_ns: 1_500 * MS,
            t_max_ns: 40 * SEC,
        }
    }
}

struct Outcome {
    trace: Vec<LineGuardAckTrace>,
    total: u64,
    /// Korumanın sayaçları koşu sonunda: (korunan hız, örnek sayısı).
    guard: Option<(Option<u64>, u64)>,
    /// Göndericinin bütün iletim kararları: (an, seq, birim, boy).
    decisions: Vec<(u64, u64, u64, u64)>,
    /// Göndericiye işletilen ACK sayısı.
    acks: u64,
}

fn sender(sc: &Scenario) -> Sender {
    let mut s = Sender::with_cc_port();
    let t_ack = adaptive_transport::receiver::budgeted_ack_interval_ns(PROFILE_DOWN);
    s.set_cc_initial_window(PROFILE_UP * t_ack / SEC);
    let cap = PROFILE_UP * MTU / (MTU + OVH);
    s.set_cc_profile_pacing_cap(Some(cap));
    if sc.rule {
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
    }
    s.set_diag_line_guard_trace(sc.trace);
    s
}

/// Düzeneğin ACK'i: (varış anı, ack_delay, M, (m_first, largest, rx_span)).
type PendingAck = (u64, u64, Vec<u64>, (u64, u64, Option<u64>));

/// Kararlar ileri hatta: serileşme gerçek hızda (FIFO), 300 ms yayılım, kayıpsız.
fn emit(
    decs: Vec<adaptive_transport::sender::TransmitDecision>,
    now: u64,
    real_bps: u64,
    (free_at, wire): (&mut u64, &mut VecDeque<(u64, u64)>),
    decisions: &mut Vec<(u64, u64, u64, u64)>,
) {
    for d in decs {
        if d.kind == TransmitKind::CsProbe {
            continue;
        }
        decisions.push((now, d.seq, d.unit_id, d.size));
        let start = (*free_at).max(now);
        *free_at = start + (d.size + OVH) * SEC / real_bps;
        wire.push_back((*free_at + ONE_WAY_NS, d.seq));
    }
}

fn run(sc: &Scenario) -> Outcome {
    let mut s = sender(sc);
    let mut free_at = 0u64;
    let mut wire: VecDeque<(u64, u64)> = VecDeque::new();
    let mut arrived: BTreeMap<u64, u64> = BTreeMap::new();
    let mut fronts: VecDeque<u64> = VecDeque::new();
    let mut acks_wire: VecDeque<PendingAck> = VecDeque::new();
    let mut decisions = Vec::new();
    let mut offered = 0u64;
    let mut acks = 0u64;
    let mut next_ack = 2 * SEC;
    let mut now = 0u64;
    while now < sc.t_max_ns {
        let due = now
            .checked_div(sc.app_gap_ns)
            .map_or(sc.units, |k| (k + 1).min(sc.units));
        while offered < due {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: 1_000_000 + offered,
                    size: FILE_UNIT,
                }),
                now,
            );
            offered += 1;
            emit(
                decs,
                now,
                sc.real_bps,
                (&mut free_at, &mut wire),
                &mut decisions,
            );
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(
            decs,
            now,
            sc.real_bps,
            (&mut free_at, &mut wire),
            &mut decisions,
        );
        while wire.front().is_some_and(|&(t, _)| t <= now) {
            let (t, seq) = wire.pop_front().expect("var");
            arrived.entry(seq).or_insert(t);
        }
        if now >= next_ack {
            next_ack += sc.ack_period_ns;
            // Alıcı yalnız yeni veriyle ACK'ler (cephe ilerlemeli).
            let fresh = arrived
                .iter()
                .next_back()
                .filter(|(&q, _)| fronts.back().is_none_or(|&f| q > f));
            if let Some((&largest, &at_largest)) = fresh {
                let base = if fronts.len() >= 2 {
                    fronts[fronts.len() - 2] + 1
                } else {
                    0
                };
                let m: Vec<u64> = arrived.range(base..=largest).map(|(&q, _)| q).collect();
                let m_first = m.first().copied().unwrap_or(largest);
                let rx_span = (m_first != largest)
                    .then(|| (at_largest - arrived[&m_first]) / 256_000 * 256_000);
                if fronts.len() >= 3 {
                    fronts.pop_front();
                }
                fronts.push_back(largest);
                acks_wire.push_back((
                    now + ACK_FLIGHT_NS,
                    now - at_largest,
                    m,
                    (m_first, largest, rx_span),
                ));
            }
        }
        while acks_wire.front().is_some_and(|a| a.0 <= now) {
            let (_, delay, m, (first, last, rx_span)) = acks_wire.pop_front().expect("var");
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: ACK_UDP,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: decisions.len() as u64,
                },
                now,
            );
            if sc.interval {
                s.note_ack_forward_interval(first, last, rx_span, &m);
            }
            let decs = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: m,
                    ack_delay: Duration::from_nanos(delay),
                }),
                now,
            );
            acks += 1;
            emit(
                decs,
                now,
                sc.real_bps,
                (&mut free_at, &mut wire),
                &mut decisions,
            );
        }
        now += MS;
    }
    Outcome {
        trace: s.diag_line_guard_trace().copied().collect(),
        total: s.diag_line_guard_trace_total(),
        guard: s.diag_line_guard(),
        decisions,
        acks,
    }
}

fn line(t: &LineGuardAckTrace) -> String {
    format!(
        "İZ-ARALIK sim an={} sıra={} en_büyük={} not={:?} ölçüm={} g={:?} karar={:?} tavan={}→{} hız={:?}→{:?}",
        t.at_ns / MS,
        t.index,
        t.largest_seq,
        t.note.map(|n| (n.first_seq, n.last_seq, n.rx_span_ns)),
        t.measured,
        t.g_ns,
        t.decision,
        t.pace_before,
        t.pace_after,
        t.rate_before,
        t.rate_after
    )
}

fn count(o: &Outcome, d: LineGuardAckDecision) -> usize {
    o.trace.iter().filter(|t| t.decision == d).count()
}

/// Dar hat (gerçek 6 250, beyan 12 500), ölçüm aralığı verilir: kayıt her ACK'i yazar, kararı
/// alıcının aralığından aldığını gösterir (aralık örneği: `g` eşiğin üstünde, hız iner) ve
/// sınıf korumanın kendi örnek sayacıyla tutarlıdır.
#[test]
fn trace_shows_interval_decision_on_a_narrow_line() {
    let o = run(&Scenario::narrow());
    for t in &o.trace {
        println!("{}", line(t));
    }
    assert!(
        o.acks > 0 && (o.acks as usize) < LINE_GUARD_TRACE_CAP,
        "ack={}",
        o.acks
    );
    assert_eq!(o.total, o.acks, "her işlenen ACK bir kayıt");
    assert_eq!(o.trace.len() as u64, o.total);
    let samples = o.guard.expect("kural kurulu").1;
    let traced_samples =
        count(&o, LineGuardAckDecision::IntervalSample) + count(&o, LineGuardAckDecision::Frontier);
    assert_eq!(
        traced_samples as u64, samples,
        "kayıt korumanın örnek sayacıyla tutarlı"
    );
    let cuts: Vec<&LineGuardAckTrace> = o
        .trace
        .iter()
        .filter(|t| t.decision == LineGuardAckDecision::IntervalSample)
        .collect();
    assert!(!cuts.is_empty(), "dar hatta aralık örneği yok");
    for t in &cuts {
        assert!(t.measured, "örnek ölçümsüz ACK'te: {}", line(t));
        assert!(
            t.note.is_some_and(|n| n.rx_span_ns.is_some()),
            "{}",
            line(t)
        );
        assert!(t.g_ns.is_some_and(|g| g > 0), "g yok: {}", line(t));
        // Örnek `min(tavan, örnek)` olarak uygulanır: tavan artmaz, korunan hız tavandadır.
        assert_eq!(t.rate_after, Some(t.pace_after), "{}", line(t));
        assert!(t.pace_after <= t.pace_before, "tavan arttı: {}", line(t));
    }
    assert!(
        cuts[0].pace_after < cuts[0].pace_before && cuts[0].rate_before.is_none(),
        "ilk aralık örneği hızı indirir: {}",
        line(cuts[0])
    );
    let first_cut = cuts[0].rate_after.expect("hız");
    assert!(
        first_cut * 10 < PROFILE_UP * 7,
        "ilk aralık örneği gerçek hatta yakın olmalı: {first_cut}"
    );
    assert_eq!(
        count(&o, LineGuardAckDecision::Frontier),
        0,
        "aralık kipinde cephe örneği"
    );
}

/// Dürüst hat (gerçek = beyan) ve aralık: ACK'ler ölçülür (`g` var) ama örnek yok; hız inmez.
#[test]
fn trace_shows_interval_without_sample_on_an_honest_line() {
    let o = run(&Scenario {
        real_bps: PROFILE_UP,
        ..Scenario::narrow()
    });
    for t in &o.trace {
        println!("{}", line(t));
    }
    assert_eq!(count(&o, LineGuardAckDecision::IntervalSample), 0);
    assert_eq!(count(&o, LineGuardAckDecision::Frontier), 0);
    let measured: Vec<&LineGuardAckTrace> = o
        .trace
        .iter()
        .filter(|t| t.decision == LineGuardAckDecision::IntervalNoSample)
        .collect();
    assert!(measured.len() >= 3, "ölçülen aralık az: {}", measured.len());
    assert!(measured.iter().all(|t| t.g_ns.is_some() && t.measured));
    assert!(
        o.trace.iter().all(|t| t.rate_after.is_none()),
        "dürüst hat korundu"
    );
}

/// Eski alıcı (aralık yok), dar hat: kararı cephe tahmincisi verir; kayıt ölçümsüz ve notsuz.
#[test]
fn trace_shows_frontier_decision_without_interval() {
    let o = run(&Scenario {
        interval: false,
        ..Scenario::narrow()
    });
    for t in &o.trace {
        println!("{}", line(t));
    }
    assert!(
        count(&o, LineGuardAckDecision::Frontier) >= 1,
        "cephe örneği yok"
    );
    assert!(o
        .trace
        .iter()
        .all(|t| !t.measured && t.note.is_none() && t.g_ns.is_none()));
    assert_eq!(count(&o, LineGuardAckDecision::IntervalSample), 0);
    assert_eq!(count(&o, LineGuardAckDecision::IntervalNoSample), 0);
    let samples = o.guard.expect("kural kurulu").1;
    assert_eq!(
        count(&o, LineGuardAckDecision::Frontier) as u64,
        samples,
        "kayıt korumanın örnek sayacıyla tutarlı"
    );
}

/// Halka sınırlı: [`LINE_GUARD_TRACE_CAP`]'ten çok ACK'te en yeni kayıtlar kalır, sıra boşluksuz
/// ve toplam her ACK'i sayar.
#[test]
fn trace_ring_is_bounded() {
    let o = run(&Scenario {
        real_bps: PROFILE_UP,
        units: 1_200,
        app_gap_ns: 100 * MS,
        t_max_ns: 120 * SEC,
        ..Scenario::narrow()
    });
    assert!(
        o.acks as usize > LINE_GUARD_TRACE_CAP + 8,
        "ACK az: {}",
        o.acks
    );
    assert_eq!(o.total, o.acks, "toplam her ACK'i sayar");
    assert_eq!(o.trace.len(), LINE_GUARD_TRACE_CAP, "halka tavanı");
    let first = o.total - LINE_GUARD_TRACE_CAP as u64 + 1;
    for (k, t) in o.trace.iter().enumerate() {
        assert_eq!(
            t.index,
            first + k as u64,
            "sıra boşluksuz, en yeniler kalır"
        );
    }
    assert!(o.trace.windows(2).all(|w| w[0].at_ns < w[1].at_ns));
}

/// Salt gözlem: kayıt açık ve kapalı koşuların bütün iletim kararları ve korumanın sonu aynı.
#[test]
fn trace_does_not_change_decisions() {
    for sc in [
        Scenario::narrow(),
        Scenario {
            interval: false,
            ..Scenario::narrow()
        },
    ] {
        let on = run(&sc);
        let off = run(&Scenario { trace: false, ..sc });
        assert!(!on.trace.is_empty());
        assert!(off.trace.is_empty() && off.total == 0, "kapalı kayıt boş");
        assert_eq!(on.decisions, off.decisions, "iletim kararları aynı");
        assert_eq!(on.guard, off.guard);
        assert_eq!(on.acks, off.acks);
    }
}

/// Kural (hat koruması) kurulu değilse kayıt açık olsa da yazılmaz.
#[test]
fn trace_is_not_written_without_the_rule() {
    let o = run(&Scenario {
        rule: false,
        t_max_ns: 15 * SEC,
        ..Scenario::narrow()
    });
    assert!(o.acks > 0);
    assert!(o.guard.is_none());
    assert_eq!(o.total, 0);
    assert!(o.trace.is_empty());
}
