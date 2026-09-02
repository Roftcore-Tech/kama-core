//! Metrik HESABI (§4.4.3). SoC: HAM eventlerden ([`crate::event`]) hesaplar; transport'a
//! ASLA dokunmaz. **A)** çoklu tekrar → completion medyan + IQR + min/max (varyans, §4.4.9).
//! **B)** çoklu deadline → completion-vs-deadline eğrisi (§4.4.3 birincil metrik).

use crate::event::{RawEvent, RunOutput};
use std::time::Duration;

/// Bir deadline için tek-koşu metrikleri.
#[derive(Debug, Clone, PartialEq)]
pub struct DeadlineMetrics {
    /// Deadline.
    pub deadline: Duration,
    /// Deadline içinde teslim oranı [0,1].
    pub completion: f64,
    /// İlk teslim latency'si (deadline-altı).
    pub first_delivery: Option<Duration>,
    /// p50 teslim latency (deadline-altı).
    pub p50: Option<Duration>,
    /// p95 teslim latency (deadline-altı).
    pub p95: Option<Duration>,
}

/// Bir koşunun tüm deadline'lardaki metrikleri + wire türevleri.
#[derive(Debug, Clone)]
pub struct RunMetrics {
    /// Deadline başına metrik (B eğrisi).
    pub per_deadline: Vec<DeadlineMetrics>,
    /// udp_tx / teslim edilen mesaj (bytes/delivered-msg).
    pub bytes_per_delivered_msg: f64,
    /// udp_rx / toplam payload (dönüş yolu/ACK yükü oranı).
    pub ack_overhead_ratio: f64,
}

fn latency_ns(e: &RawEvent) -> Option<u64> {
    e.delivered_ns.map(|dn| dn.saturating_sub(e.sent_ns))
}

/// completion oranı: teslim latency ≤ deadline olan mesaj oranı. Boş → 0.
///
/// GEÇERLİLİK: `deadline` ≤ client'ın `max_deadline`'ı olmalı — üstündeki bir deadline
/// under-report eder (max'tan sonra gelecek mesaj `None` kaydedilir). run-bench.sh
/// `MAX_DEADLINE_S` ≥ max(DEADLINES) tutar (default 120 ≥ {10,30,120}).
#[must_use]
pub fn completion(events: &[RawEvent], deadline: Duration) -> f64 {
    if events.is_empty() {
        return 0.0;
    }
    let dn = dur_ns(deadline);
    let ok = events
        .iter()
        .filter(|e| latency_ns(e).is_some_and(|l| l <= dn))
        .count();
    ok as f64 / events.len() as f64
}

/// Tek koşu → deadline başına metrik (B).
#[must_use]
pub fn run_metrics(run: &RunOutput, deadlines: &[Duration]) -> RunMetrics {
    let ev = &run.events;
    let per_deadline = deadlines
        .iter()
        .map(|&dl| {
            let dn = dur_ns(dl);
            let mut lat: Vec<u64> = ev
                .iter()
                .filter_map(latency_ns)
                .filter(|&l| l <= dn)
                .collect();
            lat.sort_unstable();
            DeadlineMetrics {
                deadline: dl,
                completion: completion(ev, dl),
                first_delivery: lat.first().map(|&l| Duration::from_nanos(l)),
                p50: percentile_ns(&lat, 50).map(Duration::from_nanos),
                p95: percentile_ns(&lat, 95).map(Duration::from_nanos),
            }
        })
        .collect();

    let delivered = ev.iter().filter(|e| e.delivered_ns.is_some()).count();
    let payload_total = ev.iter().map(|e| e.payload_bytes).sum::<u64>().max(1);
    RunMetrics {
        per_deadline,
        // 0-teslim → NaN (tanımsız; §5.2 "yalnız ≥1-teslimde ikincil"). .max(1) tüm tx'i tek
        // mesaja yazıp medyanı kirletiyordu (severe-loss hücrelerinde tam isabet). [inceleme #9]
        bytes_per_delivered_msg: if delivered == 0 {
            f64::NAN
        } else {
            run.meta.udp_tx_bytes as f64 / delivered as f64
        },
        ack_overhead_ratio: run.meta.udp_rx_bytes as f64 / payload_total as f64,
    }
}

/// Aynı (profil×CC) için N koşunun completion varyansı — deadline başına medyan/IQR/min/max.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateDeadline {
    /// Deadline.
    pub deadline: Duration,
    /// Koşu (tekrar) sayısı.
    pub runs: usize,
    /// Medyan completion.
    pub completion_median: f64,
    /// Q1 (25p).
    pub completion_q1: f64,
    /// Q3 (75p).
    pub completion_q3: f64,
    /// Min.
    pub completion_min: f64,
    /// Max.
    pub completion_max: f64,
}

/// A) çoklu koşu → deadline başına completion varyans özeti.
#[must_use]
pub fn aggregate(runs: &[RunMetrics], deadlines: &[Duration]) -> Vec<AggregateDeadline> {
    deadlines
        .iter()
        .enumerate()
        .map(|(i, &dl)| {
            let mut comps: Vec<f64> = runs
                .iter()
                .filter_map(|r| r.per_deadline.get(i).map(|d| d.completion))
                .collect();
            comps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            AggregateDeadline {
                deadline: dl,
                runs: comps.len(),
                completion_median: percentile_f64(&comps, 50),
                completion_q1: percentile_f64(&comps, 25),
                completion_q3: percentile_f64(&comps, 75),
                completion_min: comps.first().copied().unwrap_or(0.0),
                completion_max: comps.last().copied().unwrap_or(0.0),
            }
        })
        .collect()
}

/// Secondary-but-required (§4.4.3) — koşular arası medyan bytes/msg + ACK/dönüş-yolu overhead.
#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryAgg {
    /// Medyan bytes / delivered-msg.
    pub bytes_per_delivered_msg_median: f64,
    /// Medyan ACK/dönüş-yolu overhead oranı.
    pub ack_overhead_ratio_median: f64,
}

/// Secondary metriklerin koşular arası medyanı (varyansa dayanıklı).
#[must_use]
pub fn aggregate_secondary(runs: &[RunMetrics]) -> SecondaryAgg {
    // NaN (0-teslim) satırları medyana GİRMEZ (tanımsız gizlenmez: kapsam runs.len()−b.len()).
    let mut b: Vec<f64> = runs
        .iter()
        .map(|r| r.bytes_per_delivered_msg)
        .filter(|v| v.is_finite())
        .collect();
    let mut a: Vec<f64> = runs.iter().map(|r| r.ack_overhead_ratio).collect();
    b.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    a.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    SecondaryAgg {
        bytes_per_delivered_msg_median: percentile_f64(&b, 50),
        ack_overhead_ratio_median: percentile_f64(&a, 50),
    }
}

/// Nearest-rank percentile (deterministik). `p` ∈ [0,100]. Boş → `None`.
#[must_use]
pub fn percentile_ns(sorted: &[u64], p: u8) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    Some(sorted[rank_index(sorted.len(), f64::from(p))])
}

fn percentile_f64(sorted: &[f64], p: u8) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[rank_index(sorted.len(), f64::from(p))]
}

/// nearest-rank: idx = clamp(ceil(p/100·n), 1, n) − 1.
fn rank_index(n: usize, p: f64) -> usize {
    let rank = ((p / 100.0) * n as f64).ceil() as usize;
    rank.clamp(1, n) - 1
}

fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{RunMeta, RunOutput, TxComponents};

    fn ev(id: u64, sent_ms: u64, delivered_ms: Option<u64>) -> RawEvent {
        RawEvent {
            msg_id: id,
            sent_ns: sent_ms * 1_000_000,
            delivered_ns: delivered_ms.map(|m| m * 1_000_000),
            payload_bytes: 256,
        }
    }
    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn completion_deadline_bounded() {
        // 10 mesaj: 8'i 5s içinde teslim, 2'si timeout (None). completion(10s)=0.8.
        let mut events: Vec<_> = (0..8).map(|i| ev(i, i * 10, Some(i * 10 + 5000))).collect();
        events.push(ev(8, 80, None));
        events.push(ev(9, 90, None));
        assert!((completion(&events, s(10)) - 0.8).abs() < 1e-9);
        // 5s deadline'da hepsi 5s içinde → yine 0.8 (2 timeout hep dışarıda)
        assert!((completion(&events, s(5)) - 0.8).abs() < 1e-9);
        // çok küçük deadline (1ms) → hiçbiri (latency 5s) → 0.0
        assert!(completion(&events, Duration::from_millis(1)) < 1e-9);
    }

    #[test]
    fn percentile_nearest_rank() {
        let v = [10u64, 20, 30, 40, 50];
        assert_eq!(percentile_ns(&v, 50), Some(30)); // ceil(2.5)=3 → idx2
        assert_eq!(percentile_ns(&v, 95), Some(50)); // ceil(4.75)=5 → idx4
        assert_eq!(percentile_ns(&[], 50), None);
    }

    #[test]
    fn aggregate_variance_median_iqr() {
        // A: 5 tekrar, completion'lar {0.8,0.7,0.9,0.6,0.8} → medyan 0.8, IQR [0.7,0.8], min .6 max .9
        let deadlines = [s(10)];
        let comps = [0.8, 0.7, 0.9, 0.6, 0.8];
        let runs: Vec<RunMetrics> = comps
            .iter()
            .map(|&c| RunMetrics {
                per_deadline: vec![DeadlineMetrics {
                    deadline: s(10),
                    completion: c,
                    first_delivery: None,
                    p50: None,
                    p95: None,
                }],
                bytes_per_delivered_msg: 0.0,
                ack_overhead_ratio: 0.0,
            })
            .collect();
        let agg = aggregate(&runs, &deadlines);
        assert_eq!(agg.len(), 1);
        let a = &agg[0];
        assert_eq!(a.runs, 5);
        assert!((a.completion_median - 0.8).abs() < 1e-9);
        assert!((a.completion_q1 - 0.7).abs() < 1e-9);
        assert!((a.completion_q3 - 0.8).abs() < 1e-9);
        assert!((a.completion_min - 0.6).abs() < 1e-9);
        assert!((a.completion_max - 0.9).abs() < 1e-9);
    }

    #[test]
    fn multi_deadline_curve() {
        // B: bir mesaj 8s'de teslim → completion(5s)=0, completion(10s)=1.
        let meta = RunMeta {
            profile: "t".into(),
            cc: "cubic".into(),
            transport: "quinn".into(),
            quinn_version: "0.11.11".into(),
            seed: 0,
            repeat: 0,
            msg_count: 1,
            payload_bytes: 256,
            max_deadline_ns: dur_ns(s(120)),
            udp_tx_bytes: 1000,
            udp_rx_bytes: 200,
            tx: TxComponents::default(),
            kama_arm: String::new(),
            submit_interval_ns: 0,
            budget: Default::default(),
            t0_unix_ns: 0,
            events_epoch_offset_ns: 0,
            warmup_residue: 0,
        };
        let run = RunOutput {
            meta,
            events: vec![ev(0, 0, Some(8000))],
        };
        let m = run_metrics(&run, &[s(5), s(10), s(30)]);
        assert!((m.per_deadline[0].completion - 0.0).abs() < 1e-9); // 5s: 8s>5s
        assert!((m.per_deadline[1].completion - 1.0).abs() < 1e-9); // 10s: 8s≤10s
        assert!((m.per_deadline[2].completion - 1.0).abs() < 1e-9);
        assert_eq!(m.bytes_per_delivered_msg, 1000.0); // 1000 tx / 1 delivered
    }
}
