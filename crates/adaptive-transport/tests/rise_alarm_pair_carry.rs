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
    let mut release = |r: &mut Receiver, upto: u64, out: &mut Replay, w: &mut usize| {
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
