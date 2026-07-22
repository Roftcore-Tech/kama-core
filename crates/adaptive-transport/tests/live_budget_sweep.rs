//! Commit-X — CANLI-BÜTÇE-MICRO-SWEEP (feature-gated; policy-default-seçimi-YAPMAZ).
//! Harness = MS1-modeli (deterministik-paket-sim; kapasiteli-FIFO; kümülatif-SACK+RECOVERED-piggyback;
//! MDS-sayım-decode; GERÇEK-Sender + GERÇEK-feedback-loop — parity-emisyonu kanal/BtlBw/loss'u-etkiler).
//!
//! PRE-DECLARED-DARALTMA (sonuç-görülmeden-yazıldı): (a) İSTATİSTİKSEL-hücreler (paired-seed):
//! nominal · gap80(30-seed) · d1000 · d2500 · loss20 · rev99 · bw12 · offered-c1 · offered-1p5c ·
//! manyblock — 10-hücre. (b) FONKSİYONEL-senaryolar (no-data/bursty/cap-exhausted/idle-burst/mode-
//! transition/RECOVERED/interleaved/scheduler-gap/partial-blok) Commit-W-canlı-testleri + kernel-sim-
//! trace'leriyle-kapsanır (istatistiksel-tekrar-yok). (c) Arm'lar: ARQ-off · r2/r4×Leftover ·
//! r2/r4×P15 · r2/r4×F15 (7-ana); sensitivity: F10/F20(r4) yalnız {gap80, d1000, offered-c1};
//! T_floor-mult{1,4} (F15-r4) yalnız {bw12, d1000, d2500, manyblock, offered-c1}.
//! (d) Controller-arm YOK (kalibrasyonsuz → policy-evidence-değil; talimat-§18).
//! Practical-equivalence + validity-sınıfları MS1-pre-declared-kurallarıyla (scripts/lbs_analyze.py).
#![cfg(feature = "experimental-fec-budget")]

use adaptive_transport::budget_kernel::PolicyParams;
use adaptive_transport::sender::{
    AckInfo, BudgetPolicy, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, SrLogicalRef,
    Timer, TransmitKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT: u64 = 1200;
const OVERHEAD: u64 = 37;
const K: u64 = 8;
const TICK_MS: u64 = 5;
const PTO_MS: u64 = 50;
const DATA_ID_BASE: u64 = 1_000_000;

struct Rng(u64);
impl Rng {
    fn new(s: u64) -> Self {
        Rng(s.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pass(&mut self, p: f64) -> bool {
        (self.next() % 10_000) as f64 >= p * 10_000.0
    }
}

#[derive(Clone, Copy)]
struct Sc {
    p_fwd: f64,
    p_rev: f64,
    d_ms: u64,
    cap_pps: u64,
    app_gap_ms: u64,
    w_units: u64,
    t_max_ms: u64,
}
impl Default for Sc {
    fn default() -> Self {
        Sc {
            p_fwd: 0.05,
            p_rev: 0.0,
            d_ms: 50,
            cap_pps: 50,
            app_gap_ms: 40,
            w_units: 400,
            t_max_ms: 300_000,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ArmPolicy {
    Off,
    Leftover,
    Share(u32, u32),         // (ent‰, hc‰) → ProtectedShare
    FloorCap(u32, u32, u32), // (ent‰, hc‰, mult‰) → FloorCap
}

#[derive(Clone, Copy)]
struct Arm {
    name: &'static str,
    r: u16,
    policy: ArmPolicy,
}

fn params(ent: u32, hc: u32, mult: u32) -> PolicyParams {
    let mut p = PolicyParams::defaults();
    p.share_permille = ent;
    p.hardcap_share_permille = hc;
    p.floor_mult_permille = mult;
    p
}

#[derive(Debug, Clone, Copy)]
struct Arrival {
    t_ms: u64,
    unit_id: u64,
    is_parity: bool,
    block: u32,
    esi: u16,
    seq: u64,
}
type Sack = (u64, Vec<u64>, Vec<(u32, u16, u64)>);

#[allow(clippy::too_many_lines)]
fn run_arm(sc: Sc, arm: Arm, seed: u64) -> String {
    let ser_ms = 1000 / sc.cap_pps;
    let mut s = Sender::new();
    if arm.policy != ArmPolicy::Off {
        s.set_manual_fec_enabled(true).unwrap();
    }
    match arm.policy {
        ArmPolicy::Off | ArmPolicy::Leftover => {}
        ArmPolicy::Share(e, h) => s
            .set_budget_policy(BudgetPolicy::ProtectedShareExperimental(params(e, h, 2000)))
            .unwrap(),
        ArmPolicy::FloorCap(e, h, m) => s
            .set_budget_policy(BudgetPolicy::FloorCapExperimental(params(e, h, m)))
            .unwrap(),
    }
    let mut rng = Rng::new(seed);
    let fec_on = arm.policy != ArmPolicy::Off;
    let mut unit_lref: BTreeMap<u64, (u32, u16)> = BTreeMap::new();
    for i in 0..sc.w_units {
        unit_lref.insert(DATA_ID_BASE + i, ((i / K) as u32, (i % K) as u16));
    }
    let mut emitted_in_block: BTreeMap<u32, u64> = BTreeMap::new();
    let mut deliver_t: BTreeMap<u64, u64> = BTreeMap::new();
    let mut sealed: u64 = 0;
    let mut rx_acked: BTreeSet<u64> = BTreeSet::new();
    let mut have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
    let mut decoded: BTreeSet<u32> = BTreeSet::new();
    let mut pend_rec: Vec<(u32, u16, u64)> = Vec::new();
    let mut tx_seen: BTreeSet<u64> = BTreeSet::new();
    let mut arrivals: Vec<Arrival> = Vec::new();
    let mut sacks: Vec<Sack> = Vec::new();
    let mut link_free: u64 = 0;
    let mut retx_dup = 0u64;
    let mut maxq = 0u64;
    let mut seq_unit: BTreeMap<u64, u64> = BTreeMap::new();
    let mut unit_arrived: BTreeSet<u64> = BTreeSet::new();
    let mut first_par: BTreeMap<u32, u64> = BTreeMap::new();
    let mut close_t: BTreeMap<u32, u64> = BTreeMap::new();

    macro_rules! proc {
        ($out:expr, $t:expr) => {{
            for d in &$out {
                sealed += d.size + OVERHEAD;
                let (isp, b, e) = if d.kind == TransmitKind::FecParity {
                    let h = d.send_record.expect("hint");
                    let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                    first_par.entry(block_id).or_insert($t);
                    (true, block_id, K as u16 + esi)
                } else {
                    let (b, e) = *unit_lref.get(&d.unit_id).unwrap_or(&(u32::MAX, u16::MAX));
                    (false, b, e)
                };
                if !isp && b != u32::MAX {
                    s.register_sent_logical(
                        d.seq,
                        SrLogicalRef::Fec {
                            block_id: b,
                            esi: e,
                        },
                    );
                    if d.kind == TransmitKind::Data {
                        *emitted_in_block.entry(b).or_insert(0) += 1;
                    }
                }
                if matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate) {
                    retx_dup += 1;
                }
                seq_unit.insert(d.seq, d.unit_id);
                let dep = link_free.max($t);
                link_free = dep + ser_ms;
                maxq = maxq.max(dep.saturating_sub($t));
                if rng.pass(sc.p_fwd) {
                    arrivals.push(Arrival {
                        t_ms: dep + ser_ms + sc.d_ms,
                        unit_id: d.unit_id,
                        is_parity: isp,
                        block: b,
                        esi: e,
                        seq: d.seq,
                    });
                }
            }
        }};
    }

    let mut app = 0u64;
    let mut t: u64 = 0;
    while t < sc.t_max_ms {
        t += TICK_MS;
        let now = t * MS;
        if t.is_multiple_of(sc.app_gap_ms) && app < sc.w_units {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: DATA_ID_BASE + app,
                    size: UNIT,
                }),
                now,
            );
            app += 1;
            proc!(out, t);
        }
        if t.is_multiple_of(PTO_MS) {
            let out = s.on_event(Event::TimerFired(Timer::Pto), now);
            proc!(out, t);
        }
        if fec_on {
            let ready: Vec<u32> = emitted_in_block
                .iter()
                .filter(|&(_, &c)| c >= K)
                .map(|(&b, _)| b)
                .collect();
            for b in ready {
                emitted_in_block.remove(&b);
                let a = s.open_fec_block_authority();
                let _ = s.close_fec_block_manual(a, b, arm.r, UNIT).unwrap();
                close_t.insert(b, t);
            }
        }
        let (due, rest): (Vec<_>, Vec<_>) = arrivals.drain(..).partition(|a| a.t_ms <= t);
        arrivals = rest;
        for a in due {
            rx_acked.insert(a.seq);
            if a.block != u32::MAX {
                have.entry(a.block).or_default().insert(a.esi);
            }
            if !a.is_parity && a.unit_id >= DATA_ID_BASE {
                unit_arrived.insert(a.unit_id);
                deliver_t.entry(a.unit_id).or_insert(t);
            }
            if fec_on && a.block != u32::MAX && !decoded.contains(&a.block) {
                let hv = &have[&a.block];
                let miss: Vec<u16> = (0..K as u16).filter(|e| !hv.contains(e)).collect();
                if hv.len() as u64 >= K && !miss.is_empty() {
                    decoded.insert(a.block);
                    for e in miss {
                        let u = DATA_ID_BASE + u64::from(a.block) * K + u64::from(e);
                        if u < DATA_ID_BASE + sc.w_units {
                            deliver_t.entry(u).or_insert(t);
                            pend_rec.push((a.block, e, u));
                        }
                    }
                }
            }
        }
        if t.is_multiple_of(50)
            && (!rx_acked.is_empty() || !pend_rec.is_empty())
            && rng.pass(sc.p_rev)
        {
            sacks.push((
                t + sc.d_ms,
                rx_acked.iter().copied().collect(),
                pend_rec.clone(),
            ));
        }
        let (dv, rs): (Vec<_>, Vec<_>) = sacks.drain(..).partition(|x| x.0 <= t);
        sacks = rs;
        for (_, snap, rec) in dv {
            let new: Vec<u64> = snap
                .iter()
                .copied()
                .filter(|q| !tx_seen.contains(q))
                .collect();
            for q in &new {
                tx_seen.insert(*q);
            }
            if !new.is_empty() {
                let out = s.on_event(
                    Event::AckReceived(AckInfo {
                        acked_seqs: new,
                        ack_delay: Duration::ZERO,
                    }),
                    now,
                );
                proc!(out, t);
            }
            if fec_on && !rec.is_empty() {
                let units: Vec<RecoveredUnit> = rec
                    .iter()
                    .map(|&(b, e, u)| RecoveredUnit {
                        unit_id: u,
                        block_id: b,
                        esi: e,
                    })
                    .collect();
                s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
            }
        }
        if deliver_t.len() as u64 >= sc.w_units {
            break;
        }
    }
    let mut times: Vec<u64> = deliver_t.values().copied().collect();
    times.sort_unstable();
    let pct = |v: &[u64], p: f64| -> u64 {
        if v.is_empty() {
            u64::MAX
        } else {
            v[((v.len() as f64 - 1.0) * p) as usize]
        }
    };
    let fa = s.diag_fec_actuation();
    let b = s.diag_budget();
    let (_, btlbw, _, rate, _) = s.diag_rate();
    let fi = s.diag_fusion_input_snapshot();
    let mut fdel: Vec<u64> = first_par
        .iter()
        .filter_map(|(blk, &ft)| close_t.get(blk).map(|&ct| ft.saturating_sub(ct)))
        .collect();
    fdel.sort_unstable();
    let dl = [2_000u64, 5_000, 10_000].map(|d| deliver_t.values().filter(|&&x| x <= d).count());
    let tot = sc.w_units as f64;
    format!(
        "{},{seed},{:.4},{:.4},{},{},{:.4},{:.4},{:.4},{},{},{},{},{:.4},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        arm.name,
        deliver_t.len() as f64 / tot,
        deliver_t.len() as f64 * UNIT as f64 / sealed.max(1) as f64,
        pct(&times, 0.5),
        pct(&times, 0.95),
        dl[0] as f64 / tot,
        dl[1] as f64 / tot,
        dl[2] as f64 / tot,
        retx_dup,
        fa.enqueued_r_total,
        fa.emitted_r_total,
        fa.canceled_r_total,
        fa.emitted_r_total as f64 / fa.enqueued_r_total.max(1) as f64,
        u8::from((deliver_t.len() as u64) < sc.w_units),
        t,
        maxq,
        fdel.get(fdel.len() / 2).copied().unwrap_or(0),
        b.tk_admitted,
        b.tk_on_time,
        b.tk_late,
        b.tk_missed_unique,
        b.tk_cancel_before + b.tk_cancel_after,
        b.tk_rejected,
        b.new_data_displaced,
        b.retx_displaced,
        b.reservation_waste_total,
        b.cap_suppressed_events,
        btlbw,
        rate,
        fi.map_or(0, |x| x.loss_windowed_ppm),
    )
}

const SEEDS10: [u64; 10] = [11, 23, 37, 41, 53, 67, 79, 83, 97, 101];
const SEEDS30X: [u64; 20] = [
    103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193, 197,
    199,
];

fn cells() -> Vec<(&'static str, Sc)> {
    let b = Sc::default();
    vec![
        ("nominal", b),
        (
            "gap80",
            Sc {
                app_gap_ms: 80,
                ..b
            },
        ),
        (
            "d1000",
            Sc {
                d_ms: 1000,
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "d2500",
            Sc {
                d_ms: 2500,
                t_max_ms: 600_000,
                ..b
            },
        ),
        ("loss20", Sc { p_fwd: 0.20, ..b }),
        ("rev99", Sc { p_rev: 0.99, ..b }),
        (
            "bw12",
            Sc {
                cap_pps: 12,
                app_gap_ms: 166,
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "offered-c1",
            Sc {
                app_gap_ms: 20,
                ..b
            },
        ),
        (
            "offered-1p5c",
            Sc {
                app_gap_ms: 13,
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "manyblock",
            Sc {
                app_gap_ms: 10,
                w_units: 600,
                t_max_ms: 600_000,
                ..b
            },
        ),
    ]
}

fn main_arms() -> Vec<Arm> {
    vec![
        Arm {
            name: "ARQ-off",
            r: 0,
            policy: ArmPolicy::Off,
        },
        Arm {
            name: "r2-Leftover",
            r: 2,
            policy: ArmPolicy::Leftover,
        },
        Arm {
            name: "r4-Leftover",
            r: 4,
            policy: ArmPolicy::Leftover,
        },
        Arm {
            name: "r2-P15",
            r: 2,
            policy: ArmPolicy::Share(150, 250),
        },
        Arm {
            name: "r4-P15",
            r: 4,
            policy: ArmPolicy::Share(150, 250),
        },
        Arm {
            name: "r2-F15",
            r: 2,
            policy: ArmPolicy::FloorCap(150, 250, 2000),
        },
        Arm {
            name: "r4-F15",
            r: 4,
            policy: ArmPolicy::FloorCap(150, 250, 2000),
        },
    ]
}

/// CI-SMOKE: tek-hücre-mini (determinizm + ledger-tutarlılık + arm'lar-panik-siz).
#[test]
fn lbs_smoke() {
    let sc = Sc {
        w_units: 80,
        t_max_ms: 60_000,
        ..Sc::default()
    };
    for arm in main_arms() {
        let a = run_arm(sc, arm, 11);
        let bb = run_arm(sc, arm, 11);
        assert_eq!(a, bb, "deterministik-rerun: {}", arm.name);
    }
}

#[test]
#[ignore]
fn live_budget_sweep() {
    let out_dir = std::env::var("LBS_OUT").expect("LBS_OUT");
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut f = std::fs::File::create(format!("{out_dir}/lbs-seed-level.csv")).unwrap();
    writeln!(f, "cell,arm,seed,completion,goodput,t_p50,t_p95,dl2,dl5,dl10,retx_dup,par_enq,par_emit,par_canc,realization,censored,end_t_ms,max_q_ms,first_par_delay_p50,tk_admitted,tk_on_time,tk_late,tk_missed,tk_canceled,tk_rejected,new_data_displaced,retx_displaced,waste,cap_suppressed,btlbw,rate,loss_ppm_p50").unwrap();
    for (cell, sc) in cells() {
        let seeds: Vec<u64> = if cell == "gap80" {
            SEEDS10.iter().chain(SEEDS30X.iter()).copied().collect()
        } else {
            SEEDS10.to_vec()
        };
        let mut arms = main_arms();
        if matches!(cell, "gap80" | "d1000" | "offered-c1") {
            arms.push(Arm {
                name: "r4-F10",
                r: 4,
                policy: ArmPolicy::FloorCap(100, 200, 2000),
            });
            arms.push(Arm {
                name: "r4-F20",
                r: 4,
                policy: ArmPolicy::FloorCap(200, 300, 2000),
            });
        }
        if matches!(
            cell,
            "bw12" | "d1000" | "d2500" | "manyblock" | "offered-c1"
        ) {
            arms.push(Arm {
                name: "r4-F15-m1",
                r: 4,
                policy: ArmPolicy::FloorCap(150, 250, 1000),
            });
            arms.push(Arm {
                name: "r4-F15-m4",
                r: 4,
                policy: ArmPolicy::FloorCap(150, 250, 4000),
            });
        }
        for arm in &arms {
            for &seed in &seeds {
                writeln!(f, "{cell},{}", run_arm(sc, *arm, seed)).unwrap();
            }
        }
        eprintln!("[lbs] hücre-tamam: {cell}");
    }
}
