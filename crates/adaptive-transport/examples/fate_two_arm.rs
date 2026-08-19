//! FATE2ARM-V1 — two-arm product-path measurement of the loss-structure estimator.
//!
//! Question, and only this question: on the REAL `Sender` → estimator product path, do 20%
//! scattered (IID-like) loss and 20% GE-L3 clustered loss produce separable estimator outputs?
//!
//! This selects no threshold and claims nothing about FEC. Config is declared before any run:
//! 300 units x 101 B · exactly 60 lost (20%) per run · ACK RTT 50 ms · 25 ms virtual steps ·
//! 240 s horizon · seeds 0..19 · namespace FNV1a("FATE2ARM-V1" ‖ arm ‖ seed) · overlay OFF ·
//! eligibility flags (warmup=true, stale=false, converged=true) are caller-supplied constants.
//!
//! The channel is played by this driver on ORIGINAL emissions only: a lost unit's original seq is
//! never ACKed; its ARQ retransmit is always delivered (retransmit ACKs feed nothing, S-0.7).
//! Losses therefore finalize through the real declaration → guard → advance lifecycle.
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use std::collections::BTreeMap;
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNITS: u64 = 300;
const LOST: usize = 60; // exactly 20%
const RTT_NS: u64 = 50 * MS;
const STEP_NS: u64 = 25 * MS;
const HORIZON_NS: u64 = 240_000 * MS;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Arm A: 60 positions sampled without replacement (seeded Fisher-Yates prefix) — scattered,
/// adjacency unconstrained, i.e. IID-conditioned-on-count.
fn pattern_scattered(seed: u64) -> Vec<bool> {
    let mut st = fnv1a(format!("FATE2ARM-V1|IID|{seed}").as_bytes());
    let mut idx: Vec<usize> = (0..UNITS as usize).collect();
    for i in 0..LOST {
        let j = i + (splitmix(&mut st) as usize) % (UNITS as usize - i);
        idx.swap(i, j);
    }
    let mut lost = vec![false; UNITS as usize];
    for &p in &idx[..LOST] {
        lost[p] = true;
    }
    lost
}

/// Arm B: 20 non-overlapping runs of length 3 (GE-L3 shaped), seeded — same total of 60 lost.
fn pattern_ge_l3(seed: u64) -> Vec<bool> {
    let mut st = fnv1a(format!("FATE2ARM-V1|GEL3|{seed}").as_bytes());
    let mut lost = vec![false; UNITS as usize];
    let mut runs = 0usize;
    let mut guard = 0usize;
    while runs < LOST / 3 {
        guard += 1;
        assert!(guard < 100_000, "run placement must terminate");
        let s = (splitmix(&mut st) as usize) % (UNITS as usize - 2);
        // Reject overlap or adjacency-merge so runs stay distinct L=3 clusters.
        let lo = s.saturating_sub(1);
        let hi = (s + 3).min(UNITS as usize - 1);
        if lost[lo..=hi].iter().any(|&l| l) {
            continue;
        }
        for l in &mut lost[s..s + 3] {
            *l = true;
        }
        runs += 1;
    }
    lost
}

struct RunOut {
    eligible: bool,
    first_elig_ms: Option<u64>,
    first_elig_n: Option<u32>,
    n00: u32,
    n01: u32,
    n10: u32,
    n11: u32,
    n: u32,
    burst_delta_ppm: i64,
    corrections: u64,
    measured_loss_ppm: u64,
}

fn run_one(lost: &[bool]) -> RunOut {
    let mut s = Sender::new();
    for i in 0..UNITS {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    // due_ns -> seqs to ack at that instant
    let mut pending: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    let mut first_elig: Option<(u64, u32)> = None;

    let handle = |out: Vec<adaptive_transport::sender::TransmitDecision>,
                  now: u64,
                  pending: &mut BTreeMap<u64, Vec<u64>>| {
        for d in out {
            let deliver = match d.kind {
                // Original: the channel decides by unit index.
                TransmitKind::Data => !lost[(d.unit_id - 1000) as usize],
                // Recovery copies are always delivered so the flow completes; their ACKs feed
                // nothing (S-0.7 originals-only ingest).
                TransmitKind::Retransmit | TransmitKind::Duplicate => true,
                _ => false,
            };
            if deliver {
                pending.entry(now + RTT_NS).or_default().push(d.seq);
            }
        }
    };

    let mut now = 0u64;
    while now < HORIZON_NS {
        now += STEP_NS;
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        handle(out, now, &mut pending);
        // Flush ACKs that are due at this instant, batched, ascending.
        let due_keys: Vec<u64> = pending.range(..=now).map(|(&k, _)| k).collect();
        if !due_keys.is_empty() {
            let mut seqs: Vec<u64> = Vec::new();
            for k in due_keys {
                seqs.extend(pending.remove(&k).unwrap());
            }
            seqs.sort_unstable();
            let out = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: Duration::ZERO,
                }),
                now,
            );
            handle(out, now, &mut pending);
        }
        if first_elig.is_none() {
            let f = s.diag_loss_structure();
            if f.eligible(true, false, true) {
                first_elig = Some((now / MS, f.n));
            }
        }
    }

    let f = s.diag_loss_structure();
    RunOut {
        eligible: f.eligible(true, false, true),
        first_elig_ms: first_elig.map(|(t, _)| t),
        first_elig_n: first_elig.map(|(_, n)| n),
        n00: f.n00,
        n01: f.n01,
        n10: f.n10,
        n11: f.n11,
        n: f.n,
        burst_delta_ppm: f.burst_delta_ppm,
        corrections: s.diag_fate_corrections().2,
        measured_loss_ppm: f.measured_loss_ppm,
    }
}

fn main() {
    println!(
        "arm,seed,eligible,first_elig_ms,first_elig_n,n00,n01,n10,n11,N,burst_delta_ppm,\
         loss_to_success_corrections,measured_loss_ppm"
    );
    for seed in 0..20u64 {
        for (arm, lost) in [
            ("IID", pattern_scattered(seed)),
            ("GEL3", pattern_ge_l3(seed)),
        ] {
            assert_eq!(
                lost.iter().filter(|&&l| l).count(),
                LOST,
                "both arms must lose exactly {LOST} of {UNITS}"
            );
            let r = run_one(&lost);
            println!(
                "{arm},{seed},{},{},{},{},{},{},{},{},{},{},{}",
                r.eligible,
                r.first_elig_ms.map_or(-1i64, |v| v as i64),
                r.first_elig_n.map_or(-1i64, i64::from),
                r.n00,
                r.n01,
                r.n10,
                r.n11,
                r.n,
                r.burst_delta_ppm,
                r.corrections,
                r.measured_loss_ppm
            );
        }
    }
}
