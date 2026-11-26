//! [ÇİFT YOKLAMASI · KAPSAMA · sahip kararı 2026-11-26, #175 7186813951] Yoklama treninin
//! bitişi alıcıda daralma alarmı doğurur. Alarm ACK'inin aralığı daralma başlangıcından
//! başladığı için (`#175 7084078903`) treni dışarıda bırakıyordu; sonraki ACK'in aralığı da o
//! ACK'ten sonra başladığı için treni hiçbir ACK kapsayamıyordu (#262 7186637924; kapalı döngü
//! `ca0608b8`: toparlanma kolunda kapsanmayan 8 yoklamanın 8'i).
//!
//! Düzenek (`ack_alarm_interval.rs` ile aynı alıcı kurulumu): 1 000 B gövde, dönüş 62 B/s, bitmap
//! açık, beyan hat 12 500. Açık döngü: varış = gönderim, darboğaz kuyruğu yok.
//! - (a) Gerçek toparlanmadaki yoklama: hat 12 500, gönderici taban 6 250'de (160 ms arayla), bir
//!   ACK'ten sonra 6 datagramlık tren 80 ms arayla (12 500), sonra yine tabana. Deney açıkken treni
//!   izleyen ilk ACK'in aralığı trenin ilk seq'ini içermeli. Deney kapalı kol bugünkü kesmeyi
//!   belgeler (alarm var, aralık trenden sonra başlıyor).
//! - (b) Gerçek daralma (hedef 1'in ölçüsü, Yerel'in notu 7186813951 (1)): 80 ms → 160 ms. Deney
//!   açık ve kapalı alıcının ACK akışı bayt-aynı; alarm ACK'inin aralığı düşüşten sonra başlıyor.

use std::collections::BTreeMap;

use adaptive_transport::ack::AckFrame;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const BODY: u64 = 1_000;
const BASE_GAP: u64 = 160 * MS;
const PROBE_GAP: u64 = 80 * MS;
const PROBE_DATAGRAMS: u64 = 6;
/// Yoklama bu andan sonraki ilk ACK'i izleyen datagramla başlar.
const PROBE_AFTER: u64 = 12 * SEC;

struct Run {
    /// (an, çerçeve, o ana dek daralma alarmı sayısı).
    acks: Vec<(u64, AckFrame, u64)>,
    arrival: BTreeMap<u64, u64>,
    /// Trenin ilk ve son seq'i (yalnız (a)).
    probe: Option<(u64, u64)>,
}

fn receiver(probe_span: bool) -> Receiver {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(Some(12_500));
    r.set_pair_probe_span(probe_span);
    r
}

fn largest(f: &AckFrame) -> u64 {
    u64::from(f.largest_seq)
}

fn span_first(f: &AckFrame) -> u64 {
    f.span_first_full(largest(f) + 1).expect("span_first")
}

/// `gap(seq, t, probe)`: bir sonraki varışa aralık. `probe_mode`: (a)'nın treni, ilk ACK'ten
/// sonra başlar.
fn run(probe_span: bool, until: u64, plan: impl Fn(u64) -> u64, probe_mode: bool) -> Run {
    let mut r = receiver(probe_span);
    let mut acks = Vec::new();
    let mut arrival = BTreeMap::new();
    let mut probe: Option<(u64, u64)> = None;
    let mut t = SEC;
    let mut seq = 0u64;
    while t < until {
        arrival.insert(seq, t);
        r.set_next_datagram_bytes(BODY);
        let mut out: Vec<(u64, AckFrame)> = r
            .on_datagram(
                RxDatagram {
                    seq,
                    unit_id: seq,
                    kind: RxKind::Data,
                },
                t,
            )
            .ack
            .into_iter()
            .map(|f| (t, f))
            .collect();
        out.extend(r.on_ack_timer(t).map(|f| (t, f)));
        let emitted = !out.is_empty();
        for (at, f) in out {
            acks.push((at, f, r.diag_drop_alarms()));
        }
        // Tren: PROBE_AFTER'den sonra çıkan ilk ACK'ten sonraki datagramla başlar.
        let gap = if probe_mode {
            if probe.is_none() && emitted && t >= PROBE_AFTER {
                probe = Some((seq + 1, seq + PROBE_DATAGRAMS));
            }
            match probe {
                // Trenin datagramları önceki datagramdan PROBE_GAP sonra varır.
                Some((a, b)) if seq + 1 >= a && seq + 1 <= b => PROBE_GAP,
                _ => BASE_GAP,
            }
        } else {
            plan(t)
        };
        seq += 1;
        t += gap;
    }
    Run {
        acks,
        arrival,
        probe,
    }
}

fn probe_run(probe_span: bool) -> Run {
    run(probe_span, 30 * SEC, |_| BASE_GAP, true)
}

/// Treni izleyen ilk bitmap ACK'i: en büyüğü trenin son seq'ine ulaşan ilk ACK.
fn first_ack_after_probe(o: &Run) -> (u64, &AckFrame, u64) {
    let (_, last) = o.probe.expect("tren");
    let (at, f, alarms) = o
        .acks
        .iter()
        .find(|(_, f, _)| f.bitmap.is_some() && largest(f) >= last)
        .expect("treni izleyen ACK yok");
    (*at, f, *alarms)
}

/// (a) Deney açık: treni izleyen ilk ACK'in aralığı trenin ilk seq'ini içerir.
#[test]
fn a_the_ack_after_a_probe_train_covers_the_train() {
    let o = probe_run(true);
    let (first, last) = o.probe.expect("tren");
    let (at, f, alarms) = first_ack_after_probe(&o);
    let sf = span_first(f);
    println!(
        "deney açık: tren [{first}, {last}] ({}–{} ms); ACK {} ms aralık [{sf}, {}], alarm {alarms}",
        o.arrival[&first] / MS,
        o.arrival[&last] / MS,
        at / MS,
        largest(f)
    );
    assert!(
        sf <= first,
        "treni izleyen ACK'in aralığı {sf}'ten başlıyor, tren {first}'ten: tren kapsam dışında"
    );
}

/// (a) karşı-olgu, deney kapalı (bugünkü kural): trenin bitişi daralma alarmı doğurur ve alarm
/// ACK'inin aralığı trenin son datagramından başlar (kapalı döngüde FIFO hücrelerindeki gibi). Bu kol testin alarm yolunu sınadığını gösterir.
#[test]
fn a_without_the_experiment_the_alarm_ack_after_a_probe_train_excludes_the_train() {
    let o = probe_run(false);
    let (first, last) = o.probe.expect("tren");
    let (at, f, alarms) = first_ack_after_probe(&o);
    let sf = span_first(f);
    println!(
        "deney kapalı: tren [{first}, {last}]; ACK {} ms aralık [{sf}, {}], alarm {alarms}",
        at / MS,
        largest(f)
    );
    assert!(alarms >= 1, "trenin bitişi daralma alarmı doğurmadı");
    // Aralık trenin son datagramından başlıyor: trenin varış aralıkları ölçüde yok.
    assert!(
        sf >= last,
        "bugünkü kural: aralık trenin sonundan başlamalı ({sf} < {last})"
    );
}

/// (b) Gerçek daralma: deney açık ve kapalı alıcının ACK akışı bayt-aynı; alarm ACK'inin aralığı
/// düşüşten sonra başlıyor (hedef 1'in ölçüsü değişmez).
#[test]
fn b_a_real_narrowing_reports_the_same_acks_with_and_without_the_experiment() {
    const DROP_AT: u64 = 30 * SEC;
    let plan = |t: u64| if t >= DROP_AT { BASE_GAP } else { PROBE_GAP };
    let on = run(true, 40 * SEC, plan, false);
    let off = run(false, 40 * SEC, plan, false);
    assert_eq!(on.acks.len(), off.acks.len(), "ACK sayısı");
    for ((ta, fa, aa), (tb, fb, ab)) in on.acks.iter().zip(off.acks.iter()) {
        assert_eq!((ta, aa), (tb, ab), "ACK anı / alarm sayısı");
        assert_eq!(format!("{fa:?}"), format!("{fb:?}"), "ACK çerçevesi {ta}");
    }
    let alarm = on
        .acks
        .iter()
        .find(|(_, f, a)| *a >= 1 && f.bitmap.is_some())
        .expect("daralma alarmının ACK'i yok");
    let sf = span_first(&alarm.1);
    assert!(
        on.arrival[&sf] >= DROP_AT,
        "alarm ACK'inin aralığı düşüş öncesinde başlıyor ({} ms)",
        on.arrival[&sf] / MS
    );
}
