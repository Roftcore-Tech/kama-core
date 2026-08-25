//! P0 — patlamalı-kayıp kurtarma regresyon kapısı (deterministik, netem'siz, RNG'siz).
//!
//! Kanıt: kapi-eval-v1 exploratory h3 (GE-burst %40-60 @250kbit) — Kama 0.86-0.97 vs Quinn 1.000.
//! Birim izi (KAMA_BENCH_TRACE): `Data → Retx → Retx → Retx → Duplicate → SESSİZLİK` (Sınıf A) ve
//! kuyrukta `Data → (hiç)` (Sınıf B). İlke: **bir birim, teslim ACK'i gelene dek outstanding'dir;
//! kurtarma yolu asla sessizce vazgeçmez** (recovery-obsesif taban, INV-RATE-1'e dokunmaz).
//!
//! Kanal modeli: test, gönderilen HER iletim için (birim, kaçıncı-iletim) çiftine bakar; kural
//! "düşür" derse o iletimin ACK'i asla gelmez, aksi hâlde RTT sonra batch-ACK'lenir. Böylece
//! "aynı birimin ardışık N iletimi patlamada kaybolur" senaryosu birebir ve deterministik kurulur.
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

const MS: u64 = 1_000_000;
const RTT_NS: u64 = 40 * MS;
const STEP_NS: u64 = 25 * MS;

struct Outcome {
    delivered: HashSet<u64>,
    tx_per_unit: HashMap<u64, Vec<(u64, TransmitKind)>>,
}

/// `drop(unit, nth)` → true ise birimin n'inci (0-tabanlı) iletimi kaybolur.
fn run(
    mut s: Sender,
    units: u64,
    interval_ns: u64,
    horizon_ns: u64,
    drop: impl Fn(u64, usize) -> bool,
) -> Outcome {
    let mut pending: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    let mut seq_unit: HashMap<u64, u64> = HashMap::new();
    let mut tx: HashMap<u64, Vec<(u64, TransmitKind)>> = HashMap::new();
    let mut delivered: HashSet<u64> = HashSet::new();
    let mut next_submit = 0u64;
    let mut now = 0u64;
    let handle = |out: Vec<adaptive_transport::sender::TransmitDecision>,
                  now: u64,
                  pending: &mut BTreeMap<u64, Vec<u64>>,
                  seq_unit: &mut HashMap<u64, u64>,
                  tx: &mut HashMap<u64, Vec<(u64, TransmitKind)>>| {
        for d in out {
            if !matches!(
                d.kind,
                TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
            ) {
                continue;
            }
            let n = tx.entry(d.unit_id).or_default();
            let nth = n.len();
            n.push((now, d.kind));
            seq_unit.insert(d.seq, d.unit_id);
            if !drop(d.unit_id, nth) {
                pending.entry(now + RTT_NS).or_default().push(d.seq);
            }
        }
    };
    while now < horizon_ns {
        now += STEP_NS;
        while next_submit < units && next_submit * interval_ns <= now {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_submit,
                    size: 200,
                }),
                now,
            );
            handle(out, now, &mut pending, &mut seq_unit, &mut tx);
            next_submit += 1;
        }
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        handle(out, now, &mut pending, &mut seq_unit, &mut tx);
        let due: Vec<u64> = pending.range(..=now).map(|(&k, _)| k).collect();
        if !due.is_empty() {
            let mut seqs = Vec::new();
            for k in due {
                seqs.extend(pending.remove(&k).unwrap());
            }
            seqs.sort_unstable();
            for &q in &seqs {
                if let Some(&u) = seq_unit.get(&q) {
                    delivered.insert(u);
                }
            }
            let out = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: Duration::ZERO,
                }),
                now,
            );
            handle(out, now, &mut pending, &mut seq_unit, &mut tx);
        }
    }
    Outcome {
        delivered,
        tx_per_unit: tx,
    }
}

/// SINIF A: orta-akış birimi — orijinal + 3 retransmit + forward-kopya AYNI patlamada kaybolur.
/// Sonraki her iletim teslim edilir. Motor vazgeçmemeli: birim 60 s içinde teslim edilmeli.
/// (Mevcut motor: CapExceeded→HandToForward tek-atış kopya → kopya kaybı queue_losses'ta düşer
///  → birim bir daha hiç denenmez.)
#[test]
fn burst_kills_original_three_retx_and_forward_copy_unit_is_still_delivered() {
    for tail_on in [false, true] {
        let mut s = Sender::new();
        if tail_on {
            s.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
        }
        let victim = 10u64;
        let o = run(s, 40, 1000 * MS, 100_000 * MS, |u, nth| {
            u == victim && nth < 5
        });
        let tx = &o.tx_per_unit[&victim];
        assert!(
            tx.len() >= 6,
            "tail_on={tail_on}: birim {victim} beşinci iletimden sonra YENİDEN denenmeli; iz: {tx:?}"
        );
        assert!(
            o.delivered.contains(&victim),
            "tail_on={tail_on}: birim {victim} teslim edilmeli (recovery-obsesif: vazgeçme yok); iz: {tx:?}"
        );
        let others: Vec<u64> = (0..40).filter(|u| !o.delivered.contains(u)).collect();
        assert!(
            others.is_empty(),
            "tail_on={tail_on}: diğer birimler teslim edilmeli: {others:?}"
        );
    }
}

/// SINIF B: KUYRUK — son 4 birimin orijinali ve ilk 2 yeniden-iletimi patlamada kaybolur; sonra
/// hiç yeni paket gelmez (time-threshold kör: seq ≥ largest_acked). Motor PTO ile yeniden
/// iletmeye devam etmeli; hepsi 120 s içinde teslim edilmeli. (Mevcut motor: tail OFF → hiç
/// retransmit yok; tail-v1 ON → cap-3 tükenir, 3 probe'un hepsi kaybolunca kalır.)
#[test]
fn tail_burst_is_recovered_by_pto_loop_without_later_packets() {
    for tail_on in [false, true] {
        let mut s = Sender::new();
        if tail_on {
            s.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
        }
        let o = run(s, 20, 1000 * MS, 200_000 * MS, |u, nth| u >= 16 && nth < 5);
        let missing: Vec<u64> = (0..20).filter(|u| !o.delivered.contains(u)).collect();
        assert!(
            missing.is_empty(),
            "tail_on={tail_on}: kuyruk birimleri PTO döngüsüyle kurtarılmalı; eksik={missing:?}; iz16={:?}",
            o.tx_per_unit.get(&16)
        );
    }
}
