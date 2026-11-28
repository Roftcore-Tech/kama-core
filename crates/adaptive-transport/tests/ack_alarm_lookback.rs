//! [DARALMA-ALARMI · EK YOL · sahip kararı 2026-11-28, #175 7239882322] Kısa pencereye
//! (`DROP_WINDOW_NS`, 500 ms) dört uygun varış sığmadığında son dört varışın en çok 1 s geriye
//! bakılarak değerlendirilmesi. Üç hüküm ayrı:
//! * **aday**: alıcının dedektörüyle aynı girdiyi alan bağımsız `RateDropDetector` (ilk ACK'ten
//!   sonraki her varış, her çıkan ACK'te `on_ack`; `ack_alarm_interval.rs` ile aynı ayna);
//! * **ACK**: alıcının kanıtla bıraktığı alarm (`diag_drop_alarms`) ve alarm ACK'inin çıkması
//!   (bütçe, yol verme alıcıda, değişmedi);
//! * **kesme**: ACK'lerin işlendiği göndericinin hat koruması (`LineGuard`, aralık örneği).
//!
//! Düzenek: açık döngü gönderici 1 200 B gövde (1 228 B tel) datagramı `send_gap` arayla gönderir,
//! istenirse bir süre durur. Tek FIFO darboğaz hat çizelgesiyle serileştirir, ileri yayılım 300 ms.
//! Alıcı varış sırasıyla, aralarda 1 ms'de bir zamanlayıcıyla; bitmap açık. ACK göndericiye
//! 300 ms + dönüş serileşmesiyle varır. Benzetim; ürün kabulü sayılmaz.

use adaptive_transport::ack::{AckFrame, ACK_DELAY_UNIT_US, RX_SPAN_NONE};
use adaptive_transport::arrival_rate::RateDropDetector;
use adaptive_transport::line_guard::{ForwardInterval, LineGuard};
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const OVH: u64 = 28;
const BODY: u64 = 1_200;
const WIRE: u64 = BODY + OVH;
const PROFILE: u64 = 12_500;
const NARROW: u64 = 6_250;
const ONE_WAY: u64 = 300 * MS;
/// Profil hızında gönderim aralığı: 1 228 B / 12 500 B/s = 98,24 ms.
const FAST_GAP: u64 = WIRE * SEC / PROFILE;
/// Dar hatta serileşme: 1 228 B / 6 250 B/s = 196,48 ms.
const NARROW_GAP: u64 = WIRE * SEC / NARROW;

struct Scenario {
    /// `(bu andan itibaren, hat hızı B/s tel)`.
    line: Vec<(u64, u64)>,
    send_gap: u64,
    /// Gönderimin durduğu `[başlangıç, bitiş)` aralıkları.
    pauses: Vec<(u64, u64)>,
    return_bps: u64,
    until: u64,
}

#[derive(Debug)]
struct Outcome {
    /// Aday (ayna dedektör) anları.
    candidates: Vec<u64>,
    /// Alıcının kanıtla alarm bıraktığı ilk an (`diag_drop_alarms` arttı).
    alarm_set: Option<u64>,
    /// Alarm ACK'inin çıktığı an.
    alarm_ack: Option<u64>,
    /// Alarm ACK'inden hemen önce ve o ACK işlendikten sonra göndericinin korunan hattı
    /// (`rate_bps`).
    rate_before_alarm_ack: Option<Option<u64>>,
    rate_after_alarm_ack: Option<Option<u64>>,
    /// Oturum sonunda göndericinin korunan hattı.
    rate_end: Option<u64>,
    budget_deferred: u64,
    /// Çıkan bütün ACK'lerin anları (ms).
    acks_ms: Vec<u64>,
}

fn line_rate(line: &[(u64, u64)], t: u64) -> u64 {
    line.iter()
        .rev()
        .find(|&&(from, _)| t >= from)
        .map_or(PROFILE, |&(_, r)| r)
}

fn run(sc: &Scenario) -> Outcome {
    // Gönderimler ve FIFO darboğazdan varışlar.
    let mut sends = Vec::new();
    let (mut t, mut free) = (SEC, 0u64);
    while t < sc.until {
        if let Some(&(_, b)) = sc.pauses.iter().find(|&&(a, b)| t >= a && t < b) {
            t = b;
            continue;
        }
        let start = free.max(t);
        free = start + WIRE * SEC / line_rate(&sc.line, start);
        sends.push((t, free + ONE_WAY));
        t += sc.send_gap;
    }
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(sc.return_bps));
    r.set_ack_bitmap(true);
    let mut g = LineGuard::new(
        LineModel {
            rate_bps: PROFILE,
            datagram_overhead: OVH,
        },
        LineModel {
            rate_bps: sc.return_bps,
            datagram_overhead: OVH,
        },
        PROFILE,
    );
    g.end_start_budget();
    let mut det = RateDropDetector::default();
    let mut o = Outcome {
        candidates: Vec::new(),
        alarm_set: None,
        alarm_ack: None,
        rate_before_alarm_ack: None,
        rate_after_alarm_ack: None,
        rate_end: None,
        budget_deferred: 0,
        acks_ms: Vec::new(),
    };
    let mut acks: Vec<(u64, AckFrame, u64)> = Vec::new();
    let mut now = SEC;
    for (seq, &(_, arrival)) in sends.iter().enumerate() {
        let seq = seq as u64;
        // Aralarda 1 ms'de bir zamanlayıcı.
        while now + MS < arrival {
            now += MS;
            if let Some(f) = r.on_ack_timer(now) {
                det.on_ack(now);
                acks.push((now, f, r.diag_drop_alarms()));
            }
        }
        now = arrival;
        if !acks.is_empty() && det.on_arrival_seq(arrival, BODY, Some(seq)) {
            o.candidates.push(arrival);
        }
        r.set_next_datagram_bytes(BODY);
        let before = r.diag_drop_alarms();
        let out = r.on_datagram(
            RxDatagram {
                seq,
                unit_id: seq,
                kind: RxKind::Data,
            },
            arrival,
        );
        if r.diag_drop_alarms() > before && o.alarm_set.is_none() {
            o.alarm_set = Some(arrival);
        }
        for f in out.ack.into_iter().chain(r.on_ack_timer(arrival)) {
            det.on_ack(arrival);
            acks.push((arrival, f, r.diag_drop_alarms()));
        }
    }
    o.budget_deferred = r.diag_ack_budget_deferred();
    o.acks_ms = acks.iter().map(|(at, _, _)| at / MS).collect();
    // Gönderici: kayıtlar gönderim sırasıyla, ACK'ler varış anlarıyla.
    let mut next_send = 0usize;
    for (at, f, alarms) in &acks {
        let wire = f.encode().len() as u64;
        let t_s = at + ONE_WAY + (wire + OVH) * SEC / sc.return_bps;
        while next_send < sends.len() && sends[next_send].0 <= t_s {
            g.on_sent(next_send as u64, sends[next_send].0, BODY);
            next_send += 1;
        }
        let largest = u64::from(f.largest_seq);
        let interval = f.bitmap.as_ref().and_then(|bm| {
            (bm.rx_span != RX_SPAN_NONE).then(|| ForwardInterval {
                first_seq: f.span_first_full(largest + 1).expect("span_first"),
                last_seq: largest,
                rx_span_ns: u64::from(bm.rx_span) * ACK_DELAY_UNIT_US * 1000,
                rx_wire: None,
            })
        });
        let before = g.rate_bps();
        g.note_ack_wire(wire);
        let delay = u64::try_from(f.ack_delay_duration().as_nanos()).unwrap_or(u64::MAX);
        let _ = g.on_ack_measured(t_s, delay, false, largest, interval);
        if *alarms >= 1 && o.alarm_ack.is_none() {
            o.alarm_ack = Some(*at);
            o.rate_before_alarm_ack = Some(before);
            o.rate_after_alarm_ack = Some(g.rate_bps());
        }
    }
    o.rate_end = g.rate_bps();
    o
}

const PAUSE_AT: u64 = 10 * SEC;

/// Karşı örnek: gönderim 10 s'de 1,2 s durur, hat duraklamada 12 500 → 6 250 B/s iner, gönderici
/// profil hızında yeniden başlar. Yeni hattaki dört varış 589,44 ms'ye yayılır; kısa pencere en çok
/// üçünü görür.
fn drop_during_pause(return_bps: u64) -> Scenario {
    Scenario {
        line: vec![(0, PROFILE), (PAUSE_AT + 500 * MS, NARROW)],
        send_gap: FAST_GAP,
        pauses: vec![(PAUSE_AT, PAUSE_AT + 1_200 * MS)],
        return_bps,
        until: 25 * SEC,
    }
}

/// (1) Yeterli kanıt ve bütçe: aday yeniden başlamadan sonraki dördüncü varışta, alarm ACK'i çıkar,
/// gönderici o ACK'in aralığından yeni hatta (6 250 ± %10) keser.
#[test]
fn lookback_candidate_ack_and_sender_cut_are_separate_verdicts() {
    let o = run(&drop_during_pause(62));
    println!("{o:?}");
    let resume = PAUSE_AT + 1_200 * MS;
    let cand = o.candidates.first().copied().expect("aday yok");
    assert!(
        cand > resume,
        "aday yeniden başlamadan önce ({} ms)",
        cand / MS
    );
    assert!(
        cand < resume + ONE_WAY + 5 * NARROW_GAP,
        "aday geç ({} ms)",
        cand / MS
    );
    let set = o.alarm_set.expect("alıcı alarmı kanıtla bırakmadı");
    let at = o.alarm_ack.expect("alarm ACK'i çıkmadı");
    assert!(
        set >= cand && at >= set,
        "sıra: aday {cand}, kanıt {set}, ACK {at}"
    );
    assert_eq!(
        o.rate_before_alarm_ack,
        Some(None),
        "kesme alarm ACK'inden önce geldi"
    );
    let cut = o
        .rate_after_alarm_ack
        .flatten()
        .expect("gönderici alarm ACK'inden kesmedi");
    assert!(
        cut.abs_diff(NARROW) * 10 <= NARROW,
        "kesme yeni hatta değil: {cut}"
    );
}

/// (2) Bütçe yokluğu: dönüş 4 B/s (bütçe 1 B/s, kova iki tam ACK). Planlı ACK yedeği bekler ama
/// alarm ACK'i yedekten muaftır, yalnız kendi baytını öder; bu yüzden bütçesizlik, yedeğin önceki
/// ACK'lerle harcanmasıdır. İlk ACK, 5 s'deki 1 s'lik duraklamanın akış-sonu ACK'i ve 10 s'deki
/// duraklamanın akış-sonu ACK'i kovayı bir ACK'in altına indirir. Aday ve alıcının kanıtı (1)'deki
/// gibi; alarm ACK'i bütçeyi aşmaz (oturum sonuna dek ertelenir), gönderici kesmez.
#[test]
fn lookback_candidate_does_not_bypass_the_ack_budget() {
    let mut sc = drop_during_pause(4);
    sc.pauses.insert(0, (5 * SEC, 6 * SEC));
    let o = run(&sc);
    println!("{o:?}");
    assert!(!o.candidates.is_empty(), "aday yok");
    assert!(o.alarm_set.is_some(), "alıcı alarmı kanıtla bırakmadı");
    assert_eq!(o.alarm_ack, None, "bütçesiz alarm ACK'i çıktı");
    assert!(o.budget_deferred > 0, "bütçe ertelemesi yok");
    assert_eq!(o.rate_end, None, "ACK'siz kesme");
}

/// (3) Sağlıklı sabit hat: gönderim hat hızında, hat sabit (12 500 ve 6 250 B/s): aday yok, alarm
/// yok, kesme yok.
#[test]
fn steady_line_gives_no_candidate_no_alarm_no_cut() {
    for (rate, gap) in [(PROFILE, FAST_GAP), (NARROW, NARROW_GAP)] {
        let o = run(&Scenario {
            line: vec![(0, rate)],
            send_gap: gap,
            pauses: Vec::new(),
            return_bps: 62,
            until: 25 * SEC,
        });
        assert!(o.candidates.is_empty(), "{rate}: aday {:?}", o.candidates);
        assert_eq!(o.alarm_set, None, "{rate}: alarm");
        assert_eq!(o.rate_end, None, "{rate}: kesme");
    }
}

/// (4) Akış boşluğu, hat sabit 12 500: gönderim `pause` durur, aynı hızla devam eder.
/// * 1,5 s (geriye bakıştan uzun): aday yok.
/// * 600 ms (sınır): dört varışlık pencerede aralık düzeni kuralı boşluğu geçirir, ek yol aday
///   üretir. Alıcı kanıtı yetersiz bulur: duraklamada çıkan akış-sonu ACK'i aralığın tabanını
///   boşluğun sonrasına taşır, sonrası eski hızla gelir. Alarm ACK'i yok, gönderici kesmez.
#[test]
fn flow_gap_candidate_is_not_a_sender_cut() {
    let gap = |pause: u64| {
        run(&Scenario {
            line: vec![(0, PROFILE)],
            send_gap: FAST_GAP,
            pauses: vec![(PAUSE_AT, PAUSE_AT + pause)],
            return_bps: 62,
            until: 25 * SEC,
        })
    };
    let long = gap(1_500 * MS);
    assert!(long.candidates.is_empty(), "1,5 s boşlukta aday: {long:?}");
    let short = gap(600 * MS);
    println!("600 ms: {short:?}");
    assert!(
        !short.candidates.is_empty(),
        "600 ms boşlukta aday yok (sınır değişti)"
    );
    let pause_ack = short
        .acks_ms
        .iter()
        .any(|a| (PAUSE_AT / MS..(PAUSE_AT + 600 * MS + ONE_WAY) / MS).contains(a));
    assert!(
        pause_ack,
        "duraklamada akış-sonu ACK'i yok: {:?}",
        short.acks_ms
    );
    assert_eq!(short.alarm_set, None, "akış boşluğunda alıcı alarm bıraktı");
    assert_eq!(short.alarm_ack, None, "akış boşluğunda alarm ACK'i");
    assert_eq!(short.rate_end, None, "akış boşluğunda gönderici kesti");
}
