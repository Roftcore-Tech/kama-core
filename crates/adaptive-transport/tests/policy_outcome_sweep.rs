//! Commit-Z1+Z2 — POLICY-SELECTION outcome-harness: PAIRED-TRACE kanal + blok-düzeyi-causal-ledger +
//! operational/extended-horizon + reliability-separating-sweep. [policy-seçimi-YAPMAZ; prod-değişmez]
//!
//! PRE-DECLARED (koşudan-ÖNCE; sonuç-görülmeden):
//! - PAIRED-TRACE: kanal-kaderi saf-hash(seed, kind-kimliği, attempt)-fonksiyonu → AYNI-unit'in-k'ıncı-
//!   denemesi TÜM-arm'larda-AYNI-kaderde (arm'lar-arası-attempt-sayısı-farkı-trace'i-bozmaz).
//!   Reverse/blackout: ZAMAN-slot-bazlı (50ms-slot-hash / deterministik-pencere-takvimi) → arm-invariant.
//! - OPERATIONAL-HORIZON formülü: T_op_ms = max(profile_deadline=10_000, 8 × configured_RTT(=2×d_ms),
//!   min_operational=10_000). EXTENDED = t_max. Blok-deadline'ları blok-DOĞUMUNDAN (ilk-DATA-emit):
//!   {2s, 5s, 10s, 30s, profil=10s}.
//! - DEFICIT-SINIFI = İLK-deneme-source-kayıp-sayısı (saf-trace-özelliği → arm-invariant; paired ✓):
//!   {0,1,2,3+}.
//! - GE/burst: 1s-pencere-kaderi-hash'li (bad-pencere p_bad; good p_good) — matched-realized-hedef
//!   ~%10; gerçekleşen-raporlanır; |realized−hedef|/hedef > 0.3 → hücre-INVALID-işaretli.
//! - Hücreler(13): iid10 · ge-short · ge-long · d1000-ge · d2500-iid10 · blackout-10s · blackout-30s-rep ·
//!   rev99 · offered-c1 · bw12 · gap80(30-seed,iid5) · manyblock · interleaved4. Arm'lar(7-ana):
//!   ARQ-off, r2/r4×{Leftover,P15,F15}; F10/F20(r4): {gap80,manyblock,d1000-ge}; T_floor-m1/m4(r4-F15):
//!   {bw12,d1000-ge,manyblock,offered-c1}. Controller-arm-YOK (kalibrasyonsuz≠policy-evidence).
//! - Utility-bölme-sıfırları çıktıda-"NA"-explicit (script).
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

/// Saf-kader-hash (xorshift-karma; RNG-akışı-YOK → paired-trace).
fn fate(seed: u64, tag: u64, a: u64, b: u64) -> u64 {
    let mut x = seed
        ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ a.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ b.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x
}
fn pass(seed: u64, tag: u64, a: u64, b: u64, p_loss: f64) -> bool {
    (fate(seed, tag, a, b) % 10_000) as f64 >= p_loss * 10_000.0
}

#[derive(Clone, Copy)]
enum Fwd {
    Iid(f64),
    /// 1s-pencere-hash'li burst: (p_window_bad, p_good, p_bad).
    Ge(f64, f64, f64),
}
#[derive(Clone, Copy)]
enum Rev {
    Loss(f64), // slot-hash'li
    /// (period_ms, dark_ms): her-period'un-ilk-dark_ms'i-KARANLIK (deterministik-takvim).
    Blackout(u64, u64),
}

#[derive(Clone, Copy)]
struct Sc {
    fwd: Fwd,
    rev: Rev,
    d_ms: u64,
    cap_pps: u64,
    app_gap_ms: u64,
    w_units: u64,
    t_max_ms: u64,
    interleave: u64, // 1=ardışık-bloklar; N=unit→blok round-robin-N-yollu
}
impl Default for Sc {
    fn default() -> Self {
        Sc {
            fwd: Fwd::Iid(0.10),
            rev: Rev::Loss(0.0),
            d_ms: 50,
            cap_pps: 50,
            app_gap_ms: 40,
            w_units: 400,
            t_max_ms: 300_000,
            interleave: 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Pol {
    Off,
    Leftover,
    P(u32, u32),
    F(u32, u32, u32),
}
#[derive(Clone, Copy)]
struct Arm {
    name: &'static str,
    r: u16,
    pol: Pol,
}
fn pp(e: u32, h: u32, m: u32) -> PolicyParams {
    let mut p = PolicyParams::defaults();
    p.share_permille = e;
    p.hardcap_share_permille = h;
    p.floor_mult_permille = m;
    p
}

#[derive(Debug, Default, Clone)]
struct Blk {
    gen: u64,
    open_ms: i64,
    close_ms: i64,
    planned: u64,
    admitted: u64,
    emitted: u64,
    first_par: i64,
    last_par: i64,
    repairs_rx: u64,
    src_lost_first: u64, // deficit-sınıfı (trace-özelliği)
    retx_units: BTreeSet<u64>,
    decode_ms: i64,
    complete_ms: i64, // son-source-teslimi
    avoided_retx: u64,
    canceled: bool,
    moot_parity: u64, // decode-sonrası-emit
}

#[derive(Debug, Clone, Copy)]
struct Arrival {
    t: u64,
    uid: u64,
    isp: bool,
    b: u32,
    e: u16,
    seq: u64,
}
type Sack = (u64, Vec<u64>, Vec<(u32, u16, u64)>);

#[allow(clippy::too_many_lines)]
fn run_arm(sc: Sc, arm: Arm, seed: u64, cell: &str, blk_csv: &mut String, run_csv: &mut String) {
    let ser = 1000 / sc.cap_pps;
    let mut s = Sender::new();
    if arm.pol != Pol::Off {
        s.set_manual_fec_enabled(true).unwrap();
    }
    match arm.pol {
        Pol::Off | Pol::Leftover => {}
        Pol::P(e, h) => s
            .set_budget_policy(BudgetPolicy::ProtectedShareExperimental(pp(e, h, 2000)))
            .unwrap(),
        Pol::F(e, h, m) => s
            .set_budget_policy(BudgetPolicy::FloorCapExperimental(pp(e, h, m)))
            .unwrap(),
    }
    let fec = arm.pol != Pol::Off;
    let nblk = sc.w_units.div_ceil(K) as u32;
    let blk_of = |u: u64| -> (u32, u16) {
        let i = u - DATA_ID_BASE;
        if sc.interleave <= 1 {
            ((i / K) as u32, (i % K) as u16)
        } else {
            // Round-robin-interleave: blok = i % (interleave-grubu), grup-ilerlemeli.
            let g = i / (K * sc.interleave);
            let b = (g * sc.interleave + i % sc.interleave) as u32;
            let e = ((i / sc.interleave) % K) as u16;
            (b.min(nblk - 1), e)
        }
    };
    // Kanal: forward-kader = pass(seed,1,uid_or_pid,attempt); GE: pencere-kaderi pass(seed,2,win,0).
    let mut att: BTreeMap<u64, u64> = BTreeMap::new(); // kimlik→attempt-sayacı
    let fwd_pass = |seed: u64, key: u64, attempt: u64, t_ms: u64, fwd: Fwd| -> bool {
        match fwd {
            Fwd::Iid(p) => pass(seed, 1, key, attempt, p),
            Fwd::Ge(pw, pg, pb) => {
                let win = t_ms / 1000;
                let bad = !pass(seed, 2, win, 0, pw);
                pass(seed, 1, key, attempt, if bad { pb } else { pg })
            }
        }
    };
    let rev_ok = |seed: u64, t_ms: u64, rev: Rev| -> bool {
        match rev {
            Rev::Loss(p) => pass(seed, 3, t_ms / 50, 0, p),
            Rev::Blackout(per, dark) => (t_ms % per) >= dark,
        }
    };

    let mut blks: BTreeMap<u32, Blk> = BTreeMap::new();
    for b in 0..nblk {
        blks.insert(
            b,
            Blk {
                open_ms: -1,
                close_ms: -1,
                first_par: -1,
                last_par: -1,
                decode_ms: -1,
                complete_ms: -1,
                ..Blk::default()
            },
        );
    }
    let mut emitted_in_block: BTreeMap<u32, u64> = BTreeMap::new();
    let mut deliver: BTreeMap<u64, u64> = BTreeMap::new();
    let mut sealed = 0u64;
    let mut rx_acked: BTreeSet<u64> = BTreeSet::new();
    let mut have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
    let mut decoded: BTreeSet<u32> = BTreeSet::new();
    let mut pend_rec: Vec<(u32, u16, u64)> = Vec::new();
    let mut tx_seen: BTreeSet<u64> = BTreeSet::new();
    let mut arrivals: Vec<Arrival> = Vec::new();
    let mut sacks: Vec<Sack> = Vec::new();
    let mut link_free = 0u64;
    let mut retx_dup = 0u64;
    let mut unit_arrived: BTreeSet<u64> = BTreeSet::new();
    let mut retx_bytes = 0u64;

    macro_rules! proc {
        ($out:expr, $t:expr) => {{
            for d in &$out {
                sealed += d.size + OVERHEAD;
                let (isp, b, e, key) = if d.kind == TransmitKind::FecParity {
                    let h = d.send_record.expect("hint");
                    let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                    let bl = blks.get_mut(&block_id).unwrap();
                    bl.emitted += 1;
                    if bl.first_par < 0 {
                        bl.first_par = $t as i64;
                    }
                    bl.last_par = $t as i64;
                    if bl.decode_ms >= 0 {
                        bl.moot_parity += 1;
                    }
                    (
                        true,
                        block_id,
                        K as u16 + esi,
                        2_000_000 + u64::from(block_id) * 64 + u64::from(esi),
                    )
                } else if d.unit_id >= DATA_ID_BASE {
                    let (b, e) = blk_of(d.unit_id);
                    (false, b, e, d.unit_id)
                } else {
                    // Parity-RETX (unit_id=esi; blok-eşleme-yok): ayrı-kader-uzayı (seq-anahtarlı).
                    (false, u32::MAX, u16::MAX, 3_000_000 + d.seq)
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
                        let bl = blks.get_mut(&b).unwrap();
                        if bl.open_ms < 0 {
                            bl.open_ms = $t as i64;
                        }
                    }
                    if d.kind == TransmitKind::Retransmit {
                        blks.get_mut(&b).unwrap().retx_units.insert(d.unit_id);
                        retx_bytes += d.size;
                    }
                }
                if matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate) {
                    retx_dup += 1;
                }
                let a = att.entry(key).or_insert(0);
                let ok = fwd_pass(seed, key, *a, $t, sc.fwd);
                if !isp && b != u32::MAX && *a == 0 && !ok {
                    blks.get_mut(&b).unwrap().src_lost_first += 1;
                }
                *a += 1;
                let dep = link_free.max($t);
                link_free = dep + ser;
                if ok {
                    arrivals.push(Arrival {
                        t: dep + ser + sc.d_ms,
                        uid: d.unit_id,
                        isp,
                        b,
                        e,
                        seq: d.seq,
                    });
                }
            }
        }};
    }

    let mut app = 0u64;
    let mut t = 0u64;
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
        if fec {
            let ready: Vec<u32> = emitted_in_block
                .iter()
                .filter(|&(_, &c)| c >= K)
                .map(|(&b, _)| b)
                .collect();
            for b in ready {
                emitted_in_block.remove(&b);
                let enq0 = s.diag_fec_actuation().enqueued_r_total;
                let a = s.open_fec_block_authority();
                let g = a.generation();
                let _ = s.close_fec_block_manual(a, b, arm.r, UNIT).unwrap();
                let bl = blks.get_mut(&b).unwrap();
                bl.gen = g;
                bl.close_ms = t as i64;
                bl.planned = u64::from(arm.r);
                bl.admitted = s.diag_fec_actuation().enqueued_r_total - enq0;
            }
        }
        let (due, rest): (Vec<_>, Vec<_>) = arrivals.drain(..).partition(|a| a.t <= t);
        arrivals = rest;
        for a in due {
            rx_acked.insert(a.seq);
            if a.b != u32::MAX {
                have.entry(a.b).or_default().insert(a.e);
            }
            if a.isp && a.b != u32::MAX {
                blks.get_mut(&a.b).unwrap().repairs_rx += 1;
            } else if a.uid >= DATA_ID_BASE {
                unit_arrived.insert(a.uid);
                deliver.entry(a.uid).or_insert(t);
            }
            if fec && a.b != u32::MAX && !decoded.contains(&a.b) {
                let hv = &have[&a.b];
                let miss: Vec<u16> = (0..K as u16).filter(|e| !hv.contains(e)).collect();
                if hv.len() as u64 >= K && !miss.is_empty() {
                    decoded.insert(a.b);
                    blks.get_mut(&a.b).unwrap().decode_ms = t as i64;
                    for e in miss {
                        let base_i = if sc.interleave <= 1 {
                            u64::from(a.b) * K + u64::from(e)
                        } else {
                            let g = u64::from(a.b) / sc.interleave;
                            let lane = u64::from(a.b) % sc.interleave;
                            g * K * sc.interleave + u64::from(e) * sc.interleave + lane
                        };
                        let u = DATA_ID_BASE + base_i;
                        if u < DATA_ID_BASE + sc.w_units {
                            deliver.entry(u).or_insert(t);
                            pend_rec.push((a.b, e, u));
                        }
                    }
                }
            }
        }
        if t.is_multiple_of(50)
            && (!rx_acked.is_empty() || !pend_rec.is_empty())
            && rev_ok(seed, t, sc.rev)
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
            if fec && !rec.is_empty() {
                let c0 = s.diag_recovered_cancels();
                let units: Vec<RecoveredUnit> = rec
                    .iter()
                    .map(|&(b, e, u)| RecoveredUnit {
                        unit_id: u,
                        block_id: b,
                        esi: e,
                    })
                    .collect();
                s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
                let mut delta = s.diag_recovered_cancels() - c0;
                // Avoided-retx-atıfı (çift-sayım-yok: toplam==delta; batch-unit'lerine-FIFO-dağıtım).
                for &(b, _, u) in &rec {
                    if delta == 0 {
                        break;
                    }
                    let bl = blks.get_mut(&b).unwrap();
                    if !unit_arrived.contains(&u) && !bl.retx_units.contains(&u) {
                        bl.avoided_retx += 1;
                        delta -= 1;
                    }
                }
                for b in rec.iter().map(|x| x.0).collect::<BTreeSet<_>>() {
                    blks.get_mut(&b).unwrap().canceled = true;
                }
            }
        }
        if deliver.len() as u64 >= sc.w_units {
            break;
        }
    }
    // Blok-tamamlama: son-source-teslimi.
    for i in 0..sc.w_units {
        let (b, _) = blk_of(DATA_ID_BASE + i);
        if let Some(&dt) = deliver.get(&(DATA_ID_BASE + i)) {
            let bl = blks.get_mut(&b).unwrap();
            bl.complete_ms = bl.complete_ms.max(dt as i64);
        } else {
            blks.get_mut(&b).unwrap().complete_ms = i64::MAX; // teslim-edilmedi
        }
    }
    let t_op = 10_000u64.max(8 * 2 * sc.d_ms);
    let bd = s.diag_budget();
    let fa = s.diag_fec_actuation();
    for (b, bl) in &blks {
        if bl.open_ms < 0 {
            continue; // hiç-doğmadı (censored-üretim)
        }
        let comp_rel = if bl.complete_ms == i64::MAX {
            -1
        } else {
            bl.complete_ms - bl.open_ms
        };
        let dl = |d: i64| -> u8 { u8::from(comp_rel >= 0 && comp_rel <= d) };
        blk_csv.push_str(&format!(
            "{cell},{},{seed},{b},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            arm.name,
            bl.gen,
            bl.open_ms,
            bl.close_ms,
            bl.planned,
            bl.admitted,
            bl.emitted,
            bl.first_par,
            bl.last_par,
            bl.repairs_rx,
            bl.src_lost_first,
            bl.retx_units.len(),
            u8::from(bl.decode_ms >= 0),
            bl.decode_ms,
            comp_rel,
            dl(2000),
            dl(5000),
            dl(10_000),
            dl(30_000),
            dl(t_op as i64),
            u8::from(comp_rel >= 0),
            bl.avoided_retx,
            u8::from(bl.canceled),
            bl.moot_parity,
            bl.emitted * UNIT,
            u8::from(bl.complete_ms == i64::MAX),
        ));
    }
    let tot = sc.w_units as f64;
    run_csv.push_str(&format!(
        "{cell},{},{seed},{:.4},{:.4},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
        arm.name,
        deliver.len() as f64 / tot,
        deliver.len() as f64 * UNIT as f64 / sealed.max(1) as f64,
        fa.enqueued_r_total,
        fa.emitted_r_total,
        fa.canceled_r_total,
        retx_dup,
        retx_bytes,
        bd.new_data_displaced,
        bd.retx_displaced,
        bd.reservation_waste_total,
        bd.tk_missed_unique,
        bd.tk_on_time,
        bd.tk_late,
        att.len(),
        t,
        u8::from((deliver.len() as u64) < sc.w_units),
    ));
}

const SEEDS10: [u64; 10] = [11, 23, 37, 41, 53, 67, 79, 83, 97, 101];
const SEEDS30X: [u64; 20] = [
    103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193, 197,
    199,
];

fn cells() -> Vec<(&'static str, Sc)> {
    let b = Sc::default();
    vec![
        ("iid10", b),
        (
            "ge-short",
            Sc {
                fwd: Fwd::Ge(0.10, 0.02, 0.50),
                ..b
            },
        ), // kısa-bad-pencereler
        (
            "ge-long",
            Sc {
                fwd: Fwd::Ge(0.25, 0.02, 0.35),
                ..b
            },
        ),
        (
            "d1000-ge",
            Sc {
                fwd: Fwd::Ge(0.10, 0.02, 0.50),
                d_ms: 1000,
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "d2500-iid10",
            Sc {
                d_ms: 2500,
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "blackout-10s",
            Sc {
                rev: Rev::Blackout(20_000, 10_000),
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "blackout-30s",
            Sc {
                rev: Rev::Blackout(60_000, 30_000),
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "rev99",
            Sc {
                rev: Rev::Loss(0.99),
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
            "bw12",
            Sc {
                cap_pps: 12,
                app_gap_ms: 166,
                t_max_ms: 600_000,
                ..b
            },
        ),
        (
            "gap80",
            Sc {
                fwd: Fwd::Iid(0.05),
                app_gap_ms: 80,
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
        ("interleaved4", Sc { interleave: 4, ..b }),
    ]
}
fn arms_main() -> Vec<Arm> {
    vec![
        Arm {
            name: "ARQ-off",
            r: 0,
            pol: Pol::Off,
        },
        Arm {
            name: "r2-L",
            r: 2,
            pol: Pol::Leftover,
        },
        Arm {
            name: "r4-L",
            r: 4,
            pol: Pol::Leftover,
        },
        Arm {
            name: "r2-P15",
            r: 2,
            pol: Pol::P(150, 250),
        },
        Arm {
            name: "r4-P15",
            r: 4,
            pol: Pol::P(150, 250),
        },
        Arm {
            name: "r2-F15",
            r: 2,
            pol: Pol::F(150, 250, 2000),
        },
        Arm {
            name: "r4-F15",
            r: 4,
            pol: Pol::F(150, 250, 2000),
        },
    ]
}

/// CI-SMOKE: determinizm + paired-trace-invariantı (deficit-sınıfı-arm-invariant) + ledger-tutarlılık.
#[test]
fn pos_smoke() {
    let sc = Sc {
        w_units: 80,
        t_max_ms: 60_000,
        ..Sc::default()
    };
    let mut b1 = String::new();
    let mut r1 = String::new();
    run_arm(sc, arms_main()[2], 11, "smoke", &mut b1, &mut r1);
    let mut b2 = String::new();
    let mut r2 = String::new();
    run_arm(sc, arms_main()[2], 11, "smoke", &mut b2, &mut r2);
    assert_eq!(b1, b2, "deterministik-rerun");
    // Paired-deficit-invariantı: src_lost_first (kolon-13) arm'lar-arası-AYNI (saf-trace-özelliği).
    let mut bl = String::new();
    let mut rl = String::new();
    run_arm(sc, arms_main()[1], 11, "smoke", &mut bl, &mut rl); // r2-L
    let deficits = |s: &str| -> Vec<String> {
        s.lines()
            .map(|l| l.split(',').nth(13).unwrap().to_string())
            .collect()
    };
    assert_eq!(
        deficits(&b1),
        deficits(&bl),
        "deficit-sınıfı-arm-invariant (paired-trace)"
    );
}

#[test]
#[ignore]
fn policy_outcome_sweep() {
    let out = std::env::var("POS_OUT").expect("POS_OUT");
    std::fs::create_dir_all(&out).unwrap();
    let mut bf = std::fs::File::create(format!("{out}/pos-block-ledger.csv")).unwrap();
    let mut rf = std::fs::File::create(format!("{out}/pos-run-level.csv")).unwrap();
    writeln!(bf, "cell,arm,seed,block,gen,open_ms,close_ms,planned,admitted,emitted,first_par,last_par,repairs_rx,src_lost_first,retx_units,fec_decoded,decode_ms,comp_rel_ms,dl2,dl5,dl10,dl30,dl_op,completed_ext,avoided_retx,canceled,moot_parity,parity_bytes,never_delivered").unwrap();
    writeln!(rf, "cell,arm,seed,completion,goodput,enq,emit,canc,retx_dup,retx_bytes,ndisp,rdisp,waste,tk_miss,tk_on,tk_late,distinct_keys,end_t,censored").unwrap();
    for (cell, sc) in cells() {
        let seeds: Vec<u64> = if cell == "gap80" {
            SEEDS10.iter().chain(SEEDS30X.iter()).copied().collect()
        } else {
            SEEDS10.to_vec()
        };
        let mut arms = arms_main();
        if matches!(cell, "gap80" | "manyblock" | "d1000-ge") {
            arms.push(Arm {
                name: "r4-F10",
                r: 4,
                pol: Pol::F(100, 200, 2000),
            });
            arms.push(Arm {
                name: "r4-F20",
                r: 4,
                pol: Pol::F(200, 300, 2000),
            });
        }
        if matches!(cell, "bw12" | "d1000-ge" | "manyblock" | "offered-c1") {
            arms.push(Arm {
                name: "r4-F15-m1",
                r: 4,
                pol: Pol::F(150, 250, 1000),
            });
            arms.push(Arm {
                name: "r4-F15-m4",
                r: 4,
                pol: Pol::F(150, 250, 4000),
            });
        }
        for arm in &arms {
            for &seed in &seeds {
                let mut b = String::new();
                let mut r = String::new();
                run_arm(sc, *arm, seed, cell, &mut b, &mut r);
                bf.write_all(b.as_bytes()).unwrap();
                rf.write_all(r.as_bytes()).unwrap();
            }
        }
        eprintln!("[pos] hücre-tamam: {cell}");
    }
}
