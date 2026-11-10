//! E1 gösterim bayrakları (CC §4.1; ISEMRI-YOL3-M2) — yalnız genel API ile kapalı-döngü testleri.
//!
//! Düzenek: tek darboğaz (FIFO seri hız), sabit tek yön gecikmesi, her teslime anında tek-seq ACK;
//! pompa runtime'ınkiyle aynı (1 ms `Timer::Pto`, 200 ms `Timer::RtpropProbe`). Kayıp yalnız
//! deterministik desenle (motor girdisinde RNG yok, ADR-17). Çıktı: her emisyonun
//! (zaman, tür, seq, birim, boyut, send_record) dizisi ve FNV-1a 64 özeti.
//!
//! `e1_flags_off_trace_digest_reference` özetleri basar (`--nocapture`): bayraklar kapalıyken
//! karar dizisinin önceki `main` ile bit-aynı olduğu, aynı senaryo `set_e1_flags` çağrısı
//! çıkarılarak önceki `main` ağacında koşulup özetler karşılaştırılarak sınanır (PR gövdesi).

use adaptive_transport::rate::E1Flags;
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitDecision};
use std::time::Duration;

const MS: u64 = 1_000_000;

#[derive(Clone, Copy)]
enum Loss {
    None,
    /// Her `n` emisyondan biri (`n`'inci) düşer.
    EveryNth(u64),
}

/// (ms, birim boyu) teklifleri; `dur_ms` boyunca kapalı döngü. Dönüş: emisyon dizisi.
fn run(
    s: &mut Sender,
    offers: &[(u64, u64)],
    dur_ms: u64,
    rate_bps: u64,
    owd_ms: u64,
    loss: Loss,
) -> Vec<(u64, TransmitDecision)> {
    let owd = owd_ms * MS;
    let mut link_free = 0u64;
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut trace = Vec::new();
    let mut n_sent = 0u64;
    let mut next_offer = 0usize;
    let mut feed = |s: &mut Sender,
                    ev: Event,
                    now: u64,
                    acks: &mut Vec<(u64, u64)>,
                    trace: &mut Vec<(u64, TransmitDecision)>| {
        for d in s.on_event(ev, now) {
            trace.push((now, d));
            n_sent += 1;
            if let Loss::EveryNth(k) = loss {
                if n_sent.is_multiple_of(k) {
                    continue;
                }
            }
            let start = now.max(link_free);
            link_free = start + d.size * 1_000_000_000 / rate_bps;
            let at = link_free + 2 * owd;
            let pos = acks.partition_point(|&(t, _)| t <= at);
            acks.insert(pos, (at, d.seq));
        }
    };
    for t_ms in 0..dur_ms {
        let now = t_ms * MS;
        while let Some(&(t, seq)) = acks.first() {
            if t > now {
                break;
            }
            acks.remove(0);
            let ev = Event::AckReceived(AckInfo {
                acked_seqs: vec![seq],
                ack_delay: Duration::ZERO,
            });
            feed(s, ev, t, &mut acks, &mut trace);
        }
        while next_offer < offers.len() && offers[next_offer].0 == t_ms {
            let u = OpaqueUnit {
                id: next_offer as u64,
                size: offers[next_offer].1,
            };
            next_offer += 1;
            feed(s, Event::AppData(u), now, &mut acks, &mut trace);
        }
        feed(s, Event::TimerFired(Timer::Pto), now, &mut acks, &mut trace);
        if t_ms % 200 == 0 {
            feed(
                s,
                Event::TimerFired(Timer::RtpropProbe),
                now,
                &mut acks,
                &mut trace,
            );
        }
    }
    trace
}

fn fnv1a(trace: &[(u64, TransmitDecision)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (t, d) in trace {
        for b in format!("{t}:{d:?};").bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// Senaryo: (ad, teklifler, süre ms, hız B/s, tek yön ms, kayıp).
type Scenario = (&'static str, Vec<(u64, u64)>, u64, u64, u64, Loss);

fn scenarios() -> Vec<Scenario> {
    let bulk = |n: u64| (0..n).map(|_| (0u64, 1200u64)).collect::<Vec<_>>();
    let ptt: Vec<(u64, u64)> = (0..250u64).map(|k| (80 * k, 160)).collect();
    let mut mixed = bulk(125);
    mixed.extend((0..38u64).map(|k| (10_000 + 80 * k, 160)));
    vec![
        (
            "bulk_r12k_d150_p0",
            bulk(150),
            30_000,
            12_500,
            150,
            Loss::None,
        ),
        (
            "bulk_r12k_d150_l20",
            bulk(150),
            30_000,
            12_500,
            150,
            Loss::EveryNth(5),
        ),
        ("ptt_r12k_d150_p0", ptt, 20_000, 12_500, 150, Loss::None),
        (
            "mixed_r12k_d150_l10",
            mixed,
            30_000,
            12_500,
            150,
            Loss::EveryNth(10),
        ),
    ]
}

/// Bayraklar kapalı: özetleri basar (çapraz-commit karşılaştırma girdisi) ve `set_e1_flags`
/// çağrılmamış motorla `set_e1_flags(E1Flags::default())` çağrılmış motorun dizisini kıyaslar.
#[test]
fn e1_flags_off_trace_digest_reference() {
    for (name, offers, dur, rate, owd, loss) in scenarios() {
        let mut a = Sender::new();
        let ta = run(&mut a, &offers, dur, rate, owd, loss);
        let mut b = Sender::new();
        b.set_e1_flags(E1Flags::default());
        let tb = run(&mut b, &offers, dur, rate, owd, loss);
        assert!(!ta.is_empty(), "kurulum: emisyon var ({name})");
        assert_eq!(ta, tb, "varsayılan bayraklar = setter'sız motor ({name})");
        println!(
            "E1-OFF-DIGEST {name} n={} fnv1a64={:016x}",
            ta.len(),
            fnv1a(&ta)
        );
    }
}

/// Bayraklar genel setter'dan motora ulaşır: tam aday (K2) en az bir senaryoda kapalıdan farklı
/// dizi üretir (bayrak bağlantısı ölü değil).
#[test]
fn e1_flags_reach_the_engine_through_the_public_setter() {
    let k2 = E1Flags {
        e1_startup_cwnd: true,
        e1_applimited_plateau: true,
        e1_drainrt_gate: true,
        e1_probe_keep: true,
        retx_first: true,
    };
    let mut differs = 0;
    for (_, offers, dur, rate, owd, loss) in scenarios() {
        let mut off = Sender::new();
        let t_off = run(&mut off, &offers, dur, rate, owd, loss);
        let mut on = Sender::new();
        on.set_e1_flags(k2);
        assert_eq!(on.e1_flags(), k2);
        let t_on = run(&mut on, &offers, dur, rate, owd, loss);
        if t_off != t_on {
            differs += 1;
        }
    }
    assert!(differs >= 1, "K2 en az bir senaryoda farklı dizi üretir");
}
