//! [YÜKSELİŞ KANITI · K1] Yükseliş alarmını tetikleyen çift ölçüsü, o alarmın ACK'iyle göndericiye
//! ulaşır mı.
//!
//! Girdi gerçek izdir: `yerel/ham-izler-netem-v1` @ `844fb172`, `netem-v1/kancali-1/1/v1_taban_t1/
//! aday/rep1`. `cap/B_vb0.jsonl` Bob arayüzüne varan ileri yön datagramları (0–62,5 s): an (ns,
//! senaryo sıfırına göre), seq (Alice çıkış sırası, `cap/A_va0.jsonl` pid eşlemesi), UDP gövdesi
//! (`ip_len − 28`). ACK'ler yalnız Bob'un izdeki tel çıkış anlarında (`bob.jsonl` `ack.t_wire`)
//! bırakılır; arada dönüş hattı veriye yol verir (`set_uplink_yield(true)`). Hat 60 s'de 12 500'e
//! döner; 15. ACK izde 61,699 s'de çıktı.
//!
//! Benzetim/birim testidir; kabul değildir.

use adaptive_transport::ack::{AckFrame, PAIR_RATE_NONE};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

include!("data/rise_alarm_pair_carry_844fb172.rs");

const SEC: u64 = 1_000_000_000;

#[derive(Debug, Default)]
struct Replay {
    /// (an, yükseliş alarmı sayısı, o anda bir sonraki ACK'in çift alanı).
    rise: Vec<(u64, u64, u16)>,
    /// (an, daralma alarmı sayısı).
    drop: Vec<(u64, u64)>,
    /// (an, çerçeve).
    acks: Vec<(u64, AckFrame)>,
}

fn replay(arrivals: &[(u64, u64, u64)], wire: &[u64]) -> Replay {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r.set_ack_bitmap_ptt_lean(true);
    r.set_pair_squeeze_line(Some(12_500));
    let mut out = Replay::default();
    let mut w = 0usize;
    let (mut rises, mut drops) = (0u64, 0u64);
    let release = |r: &mut Receiver, upto: u64, out: &mut Replay, w: &mut usize| {
        while *w < wire.len() && wire[*w] <= upto {
            let at = wire[*w];
            r.set_uplink_yield(false);
            r.set_return_line_ahead_ns(0);
            if let Some(f) = r.on_ack_timer(at) {
                out.acks.push((at, f));
            }
            r.set_uplink_yield(true);
            *w += 1;
        }
    };
    for &(t, seq, b) in arrivals {
        release(&mut r, t, &mut out, &mut w);
        r.set_uplink_yield(true);
        r.set_return_line_ahead_ns(0);
        r.set_next_datagram_bytes(b);
        let o = r.on_datagram(
            RxDatagram {
                seq,
                unit_id: seq,
                kind: RxKind::Data,
            },
            t,
        );
        out.acks.extend(o.ack.into_iter().map(|f| (t, f)));
        if r.diag_rise_alarms() > rises {
            rises = r.diag_rise_alarms();
            out.rise.push((t, rises, r.diag_pair_rate_field()));
        }
        if r.diag_drop_alarms() > drops {
            drops = r.diag_drop_alarms();
            out.drop.push((t, drops));
        }
    }
    release(&mut r, u64::MAX, &mut out, &mut w);
    out
}

fn pair(f: &AckFrame) -> u16 {
    f.bitmap.as_ref().map_or(PAIR_RATE_NONE, |b| b.pair_rate)
}

/// ADIM 1: izde 60 s'den sonraki yükseliş alarmı ve 15. ACK (izdeki çıkış 61,699 s).
#[test]
fn trace_rise_alarm_value_reaches_the_alarm_ack() {
    let o = replay(ARRIVALS, BOB_ACK_WIRE);
    for (t, n, f) in &o.rise {
        println!(
            "yükseliş alarmı #{n} @ {:.3} s: sıradaki ACK çift alanı {f}",
            *t as f64 / 1e9
        );
    }
    for (t, n) in &o.drop {
        println!("daralma alarmı #{n} @ {:.3} s", *t as f64 / 1e9);
    }
    for (i, (t, f)) in o.acks.iter().enumerate() {
        println!(
            "ACK {} @ {:.3} s largest {} pair_rate {}",
            i + 1,
            *t as f64 / 1e9,
            f.largest_seq,
            pair(f)
        );
    }
    let &(t_rise, _, at_alarm) = o
        .rise
        .iter()
        .find(|(t, ..)| *t >= 60 * SEC)
        .expect("60 s'den sonra yükseliş alarmı yok");
    let (t_ack, ack) = o
        .acks
        .iter()
        .find(|(t, _)| *t > t_rise)
        .expect("alarmdan sonra ACK yok");
    println!(
        "alarm {:.3} s alan {at_alarm} → ACK {:.3} s alan {}",
        t_rise as f64 / 1e9,
        *t_ack as f64 / 1e9,
        pair(ack)
    );
    assert_ne!(at_alarm, PAIR_RATE_NONE, "alarm anında değer yok");
    assert!(
        pair(ack) != PAIR_RATE_NONE && pair(ack) >= at_alarm,
        "alarm ACK'i alarmın değerini taşımıyor: alarm {at_alarm}, ACK {}",
        pair(ack)
    );
}

/// Duran kuyruklu tek FIFO darboğaz: her varış bir öncekinden kendi tel serileşmesi kadar sonra
/// (`(b + 28) / hat`). `plan`: (bu andan itibaren, hat B/s). 1 046 B gövde, kayıpsız.
fn standing_queue(plan: &[(u64, u64)], until: u64) -> Vec<(u64, u64, u64)> {
    let b = 1_046u64;
    let (mut t, mut seq, mut v) = (SEC, 0u64, Vec::new());
    while t < until {
        v.push((t, seq, b));
        let line = plan
            .iter()
            .rev()
            .find(|&&(from, _)| t >= from)
            .map_or(12_500, |&(_, l)| l);
        t += (b + 28) * SEC / line;
        seq += 1;
    }
    v
}

fn every(from: u64, step: u64, until: u64) -> Vec<u64> {
    (0..)
        .map(|i| from + i * step)
        .take_while(|&t| t < until)
        .collect()
}

/// Karşı örnek: yükselişten hemen sonra GERÇEK ikinci düşüş. Alarm ACK'i düşüş sonrası çiftleri
/// taşır; yükseliş değeri sızmaz (alan ≤ 6 875).
#[test]
fn real_second_drop_after_rise_does_not_carry_the_rise_value() {
    let plan = [
        (0, 12_500),
        (20 * SEC, 6_250),
        (40 * SEC, 12_500),
        (40 * SEC + 400_000_000, 6_250),
    ];
    let mut wire = every(2 * SEC, 4 * SEC, 40 * SEC);
    wire.push(43 * SEC);
    let o = replay(&standing_queue(&plan, 44 * SEC), &wire);
    let &(t_rise, ..) = o
        .rise
        .iter()
        .find(|(t, ..)| *t >= 40 * SEC)
        .expect("40 s'de yükseliş alarmı yok");
    let drop_after = o.drop.iter().find(|(t, _)| *t > t_rise);
    println!(
        "yükseliş {:.3} s, sonraki daralma {drop_after:?}",
        t_rise as f64 / 1e9
    );
    let (t_ack, ack) = o.acks.iter().find(|(t, _)| *t > t_rise).expect("ACK yok");
    println!("ACK {:.3} s alan {}", *t_ack as f64 / 1e9, pair(ack));
    assert!(
        drop_after.is_some_and(|(t, _)| t < t_ack),
        "ikinci düşüşün kanıtı ACK'ten önce gelmedi"
    );
    assert!(
        pair(ack) <= 6_875,
        "yükseliş değeri gerçek ikinci düşüşün ACK'ine sızdı: {}",
        pair(ack)
    );
}

/// Karşı örnek: hat düşüşten sonra 6 250'de kalır. Yükseliş alarmı yok, alan hattı aşmaz.
#[test]
fn honest_6250_line_shows_no_rise() {
    let plan = [(0, 12_500), (20 * SEC, 6_250)];
    let o = replay(
        &standing_queue(&plan, 60 * SEC),
        &every(2 * SEC, 4 * SEC, 60 * SEC),
    );
    assert!(
        o.rise.is_empty(),
        "dürüst 6 250 hatta yükseliş alarmı: {:?}",
        o.rise
    );
    for (t, f) in o.acks.iter().filter(|(t, _)| *t > 24 * SEC) {
        let p = pair(f);
        assert!(
            p == PAIR_RATE_NONE || u64::from(p) <= 6_250 * 1_046 / 1_074 + 1,
            "{:.3} s alan {p}",
            *t as f64 / 1e9
        );
    }
}

/// #256 açık 1: yükselişten sonra GERÇEK ikinci düşüş ve alarmdan sonra yeni çift ölçülmez (düşüş
/// sonrası datagramlar `PAIR_MIN_UDP_BYTES`'tan küçük: çift değeri üretmez, sıkışma da değildir).
/// Eski yükseliş değeri ACK'e taşınmamalı: alan NONE ya da ≤ 6 875.
#[test]
fn real_second_drop_without_new_pairs_does_not_carry_the_rise_value() {
    let b_small = 500u64;
    let plan = [
        (0, 12_500),
        (20 * SEC, 6_250),
        (40 * SEC, 12_500),
        (40 * SEC + 400_000_000, 6_250),
    ];
    let mut v = Vec::new();
    let (mut t, mut seq) = (SEC, 0u64);
    while t < 44 * SEC {
        let b = if t >= 40 * SEC + 400_000_000 {
            b_small
        } else {
            1_046
        };
        v.push((t, seq, b));
        let line = plan
            .iter()
            .rev()
            .find(|&&(from, _)| t >= from)
            .map_or(12_500, |&(_, l)| l);
        t += (b + 28) * SEC / line;
        seq += 1;
    }
    let mut wire = every(2 * SEC, 4 * SEC, 40 * SEC);
    wire.push(43 * SEC);
    let o = replay(&v, &wire);
    let &(t_rise, _, at_rise) = o
        .rise
        .iter()
        .find(|(t, ..)| *t >= 40 * SEC)
        .expect("40 s'de yükseliş alarmı yok");
    let drop_after = o.drop.iter().find(|(t, _)| *t > t_rise);
    let (t_ack, ack) = o.acks.iter().find(|(t, _)| *t > t_rise).expect("ACK yok");
    println!(
        "yükseliş {:.3} s (alan {at_rise}), sonraki daralma {drop_after:?}, ACK {:.3} s alan {}",
        t_rise as f64 / 1e9,
        *t_ack as f64 / 1e9,
        pair(ack)
    );
    assert!(
        drop_after.is_some_and(|(t, _)| t < t_ack),
        "ikinci düşüşün kanıtı ACK'ten önce gelmedi"
    );
    let p = pair(ack);
    assert!(
        p == PAIR_RATE_NONE || p <= 6_875,
        "yükseliş değeri yeni çiftsiz gerçek düşüşe taşındı: {p}"
    );
}
