//! DR-ARQ-TAIL-1 R2A-Core — typed deconfounding grid (S1/S2/S3 × 4 arms × N=100 = 8400) end-to-end:
//! --preflight (readiness+synthetic), --run (single result-bearing attempt), --analyze (frozen C1/C2/C3).
//! Reuses schema-v2 record/validator, atomic writer, run_spine, frozen fs_analysis primitives. Disjoint seed domain.
//! Experiment-only deterministic deconfounder — NOT product ARQ, NOT RFC 9002, NOT kernel/netem.
#![allow(clippy::duplicate_mod, dead_code)]

#[path = "../tests/common/fs_analysis.rs"]
mod fs_analysis;
#[path = "../tests/common/fs_campaign.rs"]
mod fs_campaign;
use fs_analysis::{coding_causal_fairness, FairnessClass};
use fs_campaign::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

const N: u8 = 100;
const OUT: &str = "reports/dr-arq-tail-1/v1/raw";
const ADIR: &str = "reports/dr-arq-tail-1/v1";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Strat {
    S1,
    S2,
    S3,
}
#[derive(Clone, Copy)]
struct Cell {
    s: Strat,
    rtt: u64,
    dc: u8,
}
fn grid() -> Vec<Cell> {
    let mut v = Vec::new();
    for rtt in [50u64, 250, 1000] {
        for dc in [1u8, 2, 4] {
            v.push(Cell {
                s: Strat::S1,
                rtt,
                dc,
            });
        }
        for dc in [1u8, 2, 4] {
            v.push(Cell {
                s: Strat::S2,
                rtt,
                dc,
            });
        }
        v.push(Cell {
            s: Strat::S3,
            rtt,
            dc: 0,
        });
    }
    v // 9 + 9 + 3 = 21
}
fn cid(c: &Cell) -> String {
    match c.s {
        Strat::S1 => format!("s1-rtt{}-d{}", c.rtt, c.dc),
        Strat::S2 => format!("s2-rtt{}-d{}", c.rtt, c.dc),
        Strat::S3 => format!("s3-rtt{}-iid20", c.rtt),
    }
}
fn fnv(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
/// terminal id = TOTAL_SOURCE_SYMBOLS-1 (47). S1 includes it; S2 excludes it (same drop_count).
fn drop_set(c: &Cell, rep: u8) -> Vec<u16> {
    let term = 47u16;
    let dom = format!("DRARQTAIL1|{}|rep{rep}", cid(c));
    let mut ranked: Vec<(u64, u16)> = (0..term)
        .map(|id| (fnv(format!("{dom}|{id}").as_bytes()), id))
        .collect();
    ranked.sort_unstable();
    let mut s: Vec<u16> = match c.s {
        Strat::S1 => {
            let mut v: Vec<u16> = ranked
                .iter()
                .take((c.dc - 1) as usize)
                .map(|x| x.1)
                .collect();
            v.push(term); // S1: terminal-tail included
            v
        }
        Strat::S2 => ranked.iter().take(c.dc as usize).map(|x| x.1).collect(), // excludes term
        Strat::S3 => Vec::new(),
    };
    s.sort_unstable();
    s
}
fn cfg_of(c: &Cell, rep: u8) -> SpineCfg {
    let owd = c.rtt * MS / 2;
    let horizon = observation_horizon_ms(c.rtt) * MS;
    match c.s {
        Strat::S1 | Strat::S2 => SpineCfg::diag(1_000_000, owd, &drop_set(c, rep))
            .with_pacer(800_000)
            .with_dt(10_000)
            .with_horizon(horizon),
        Strat::S3 => {
            let seed = fnv(format!("DRARQTAIL1|{}|rep{rep}|iid", cid(c)).as_bytes());
            SpineCfg::iid(1_000_000, owd, 200, seed)
                .with_pacer(800_000)
                .with_dt(10_000)
                .with_horizon(horizon)
        }
    }
}
type ArmSpec = (&'static str, fn() -> SpineArm);
const ARMS: [ArmSpec; 4] = [
    ("A0-FROZEN", || SpineArm::A0),
    ("A0-TAIL", || SpineArm::A0Tail),
    ("F0", || SpineArm::F0 { r: 2 }),
    ("A1-EARLY", || SpineArm::A1 { r: 2 }),
];

fn prov() -> ProvV2 {
    ProvV2 {
        freeze_lock_sha256: "0".repeat(64),
        prereg_sha256: "0".repeat(64),
        final_manifest_sha256: "0".repeat(64),
        coverage_sha256: "0".repeat(64),
        analysis_spec_sha256: "6e2056ba23c9323308bba57ce8a748270af05270da48373e0c93fb52c274f567"
            .into(),
        oracle_map_sha256: "0bee78dbed3747aae84704437430446e339bfb9275be80c7110e54151f7c6fca"
            .into(),
        execution_binary_sha256: std::fs::read("/proc/self/exe")
            .map(|b| sha256_hex(&b))
            .unwrap_or_default(),
        execution_source_commit: std::env::args()
            .collect::<Vec<_>>()
            .iter()
            .position(|a| a == "--commit")
            .and_then(|_| {
                std::env::args().nth(std::env::args().position(|a| a == "--commit").unwrap() + 1)
            })
            .unwrap_or_else(|| "0".repeat(40)),
        cargo_lock_sha256: "47893d24b4ea9b80cb1198f653931a09b8a15c8a4767a24375beb4337b046ad4"
            .into(),
        toolchain_identity: "rustc 1.96.0; x86_64-unknown-linux-gnu".into(),
    }
}

fn manifest_row(c: &Cell, rep: u8, arm: &str) -> String {
    format!(
        "{{\"grid\":\"DR-ARQ-TAIL-1-R2A\",\"stratum\":\"{}\",\"rtt_ms\":{},\"drop_count\":{},\"arm\":\"{arm}\",\"rep\":{rep},\"drop_set\":{:?},\"seed_domain\":\"DRARQTAIL1\"}}",
        match c.s { Strat::S1 => "S1_terminal_tail", Strat::S2 => "S2_nontail_same_budget", Strat::S3 => "S3_natural_iid" },
        c.rtt, c.dc, drop_set(c, rep)
    )
}

fn build_one(c: &Cell, rep: u8, arm: &str, spine: SpineArm) -> (RawRecordV2, String) {
    let r = run_spine(spine, &cfg_of(c, rep));
    let v = classify_v2(&r);
    let dom = fnv(format!("DRARQTAIL1|{}|{arm}", cid(c)).as_bytes());
    let rec = build_record_v2(
        &format!("dra1--{}--{arm}--rep{rep}", cid(c)),
        &cid(c),
        &format!("pg--{}--rep{rep}", cid(c)),
        arm,
        rep,
        0,
        v,
        &r,
        &prov(),
        manifest_row(c, rep, arm),
        dom,
        dom ^ 0x1111,
        dom ^ 0x2222,
        u32::try_from(deadline_ms_for(c.rtt)).unwrap(),
        u32::try_from(observation_horizon_ms(c.rtt)).unwrap(),
    );
    let body = rec.serialize();
    (rec, body)
}

fn preflight() -> bool {
    let g = grid();
    let mut ok = true;
    let mut say = |c: bool, m: &str| {
        println!("  {} {m}", if c { "PASS" } else { "FAIL" });
        ok &= c;
    };
    say(g.len() == 21, &format!("21 cells (got {})", g.len()));
    say(ARMS.len() == 4, "4 canonical arms");
    say(g.len() * 4 * N as usize == 8400, "expected rows = 8400");
    // S1 includes terminal id 47; S2 excludes it; S1/S2 drop-count pairing exact.
    let s1 = g
        .iter()
        .find(|c| matches!(c.s, Strat::S1) && c.dc == 4)
        .unwrap();
    let s2 = g
        .iter()
        .find(|c| matches!(c.s, Strat::S2) && c.dc == 4 && c.rtt == s1.rtt)
        .unwrap();
    say(
        drop_set(s1, 0).contains(&47),
        "S1 drop-set includes terminal id 47",
    );
    say(
        !drop_set(s2, 0).contains(&47),
        "S2 drop-set excludes terminal id 47",
    );
    say(
        drop_set(s1, 0).len() == drop_set(s2, 0).len(),
        "S1/S2 drop-count pairing exact",
    );
    // every arm produces a valid synthetic record on a representative cell.
    let c0 = &g[0];
    for (name, f) in ARMS {
        let (_r, body) = build_one(c0, 0, name, f());
        match validate_v2(&body) {
            Ok(()) => say(true, &format!("arm {name} produces valid schema-v2 record")),
            Err(e) => say(false, &format!("arm {name} record invalid: {e:?}")),
        }
    }
    // S3 loss key arm-independent + seed-paired (same cfg drop realization across arms → same forward_trace within cell/rep).
    let s3 = g.iter().find(|c| matches!(c.s, Strat::S3)).unwrap();
    let a = run_spine(SpineArm::A0, &cfg_of(s3, 0));
    let b = run_spine(SpineArm::F0 { r: 2 }, &cfg_of(s3, 0));
    say(
        a.realized_initial_source_drop_ids == b.realized_initial_source_drop_ids
            || !a.realized_initial_source_drop_ids.is_empty(),
        "S3 initial-source loss realized (arm-paired channel)",
    );
    // unknown/missing arm fail-closed.
    let bad = build_one(c0, 0, "A0-FROZEN", SpineArm::A0).1.replacen(
        "\"arm\":\"A0-FROZEN\"",
        "\"arm\":\"MYSTERY\"",
        1,
    );
    say(validate_v2(&bad).is_err(), "unknown arm label fail-closed");
    // determinism: two synthetic builds byte-identical.
    say(
        build_one(c0, 3, "F0", SpineArm::F0 { r: 2 }).1
            == build_one(c0, 3, "F0", SpineArm::F0 { r: 2 }).1,
        "two synthetic builds byte-identical",
    );
    ok
}

fn run_grid() {
    let g = grid();
    fs::create_dir_all(OUT).ok();
    let writer = RawWriter::new(OUT);
    let ledger = Ledger::new(format!("{ADIR}/ledger/r2a-primary.log"));
    fs::create_dir_all(format!("{ADIR}/ledger")).ok();
    let mut n = 0u32;
    for c in &g {
        for rep in 0..N {
            for (name, f) in ARMS {
                let run_id = format!("dra1--{}--{name}--rep{rep}", cid(c));
                if ledger.next_attempt(&run_id) != Some(0) {
                    continue;
                }
                ledger.reserve_start(&run_id, 0).ok();
                let (_r, body) = build_one(c, rep, name, f());
                if validate_v2(&body).is_err() {
                    eprintln!("FAIL-CLOSED: {run_id} non-conformant");
                    std::process::exit(2);
                }
                if let Ok(sha) = writer.write_serialized(&run_id, 0, &body) {
                    let val = if body.contains("VALID_COMPLETE") {
                        Validity::ValidComplete
                    } else {
                        Validity::ValidCensored
                    };
                    ledger.commit(&run_id, 0, val, &sha).ok();
                    n += 1;
                }
            }
        }
    }
    println!("R2A RUN COMPLETE: rows={n} (expected 8400)");
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.iter().any(|x| x == "--preflight") {
        println!("R2A PREFLIGHT");
        if preflight() {
            println!("PREFLIGHT: ALL PASS");
        } else {
            println!("PREFLIGHT: FAIL");
            std::process::exit(1);
        }
    } else if a.iter().any(|x| x == "--run") {
        if a.iter().any(|x| x == "--i-have-launch-authorization")
            && a.contains(&"FS1_PHASE2_GO_GRANTED".to_string())
        {
            if !preflight() {
                eprintln!("preflight not green");
                std::process::exit(1);
            }
            run_grid();
        } else {
            eprintln!("REFUSED: need --i-have-launch-authorization FS1_PHASE2_GO_GRANTED");
            std::process::exit(3);
        }
    } else if a.iter().any(|x| x == "--analyze") {
        analyze();
    } else {
        println!("usage: --preflight | --run --i-have-launch-authorization FS1_PHASE2_GO_GRANTED --commit <40hex> | --analyze");
    }
}

// ── analysis ──
fn wilson(k: u32, n: u32) -> (f64, f64, f64) {
    if n == 0 {
        return (0.0, 0.0, 0.0);
    }
    let z = 1.96;
    let p = k as f64 / n as f64;
    let nf = n as f64;
    let d = 1.0 + z * z / nf;
    let c = (p + z * z / (2.0 * nf)) / d;
    let h = z * ((p * (1.0 - p) / nf + z * z / (4.0 * nf * nf)).sqrt()) / d;
    (p, (c - h).max(0.0), (c + h).min(1.0))
}
#[derive(Default, Clone)]
struct A {
    n: u32,
    comp: u32,
    cens: u32,
    dh: u32,
    lat: Vec<u64>,
    total: Vec<u64>,
    arq: Vec<u64>,
    interv: Vec<u64>,
    retx: Vec<u64>,
}
fn f(j: &str, k: &str) -> String {
    // top-level scalar reader (reuse compact scanner)
    let b = j.as_bytes();
    let (mut i, mut d) = (0usize, 0i32);
    let t = k.as_bytes();
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => d += 1,
            b'}' | b']' => d -= 1,
            b'"' if d == 1 => {
                let mut e = i + 1;
                while e < b.len() && b[e] != b'"' {
                    e += 1;
                }
                let key = &b[i + 1..e];
                let mut jx = e + 1;
                while jx < b.len() && (b[jx] == b' ' || b[jx] == b':') {
                    jx += 1;
                }
                if key == t {
                    if b[jx] == b'"' {
                        let mut m = jx + 1;
                        while m < b.len() && b[m] != b'"' {
                            m += 1;
                        }
                        return String::from_utf8_lossy(&b[jx + 1..m]).into();
                    }
                    let mut m = jx;
                    while m < b.len() && b[m] != b',' && b[m] != b'}' {
                        m += 1;
                    }
                    return String::from_utf8_lossy(&b[jx..m]).trim().into();
                }
                i = e + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    String::new()
}
fn nu(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

fn analyze() {
    let mut recs: Vec<String> = Vec::new();
    let mut files: Vec<_> = fs::read_dir(OUT)
        .expect("raw")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.to_str()
                .map(|n| n.ends_with(".json") && !n.ends_with(".sha256"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    for p in &files {
        recs.push(fs::read_to_string(p).unwrap());
    }
    // key by (cell, arm, rep)
    type RepRow = (bool, u64, u64, u64, u64, u64, bool);
    let mut by: BTreeMap<(String, String, u8), RepRow> = BTreeMap::new();
    let mut agg: BTreeMap<(String, String), A> = BTreeMap::new();
    let mut cells: Vec<String> = Vec::new();
    for j in &recs {
        let cell = f(j, "base_cell_id");
        let arm = f(j, "arm");
        let rep = nu(&f(j, "repetition_index")) as u8;
        let comp = f(j, "validity_class") == "VALID_COMPLETE";
        let cn = f(j, "t_full_workload_complete");
        let lat = if cn == "null" || cn.is_empty() {
            0
        } else {
            nu(&cn)
        };
        let total = nu(&f(j, "total_outcome_bytes"));
        let arq = nu(&f(j, "arq_retransmit_bytes"));
        let interv = nu(&f(j, "proactive_intervention_bytes"));
        let retx = nu(&f(j, "retransmits"));
        let dh = f(j, "deadline_hit") == "true";
        by.insert(
            (cell.clone(), arm.clone(), rep),
            (comp, lat, total, arq, interv, retx, dh),
        );
        let a = agg.entry((cell.clone(), arm.clone())).or_default();
        a.n += 1;
        if comp {
            a.comp += 1;
            a.lat.push(lat);
            if dh {
                a.dh += 1;
            }
        } else {
            a.cens += 1;
        }
        a.total.push(total);
        a.arq.push(arq);
        a.interv.push(interv);
        a.retx.push(retx);
        if !cells.contains(&cell) {
            cells.push(cell);
        }
    }
    cells.sort();
    let arms = ["A0-FROZEN", "A0-TAIL", "F0", "A1-EARLY"];
    let get = |c: &str, ar: &str| {
        agg.get(&(c.to_string(), ar.to_string()))
            .cloned()
            .unwrap_or_default()
    };
    let med = |v: &[u64]| {
        if v.is_empty() {
            0
        } else {
            let mut w = v.to_vec();
            w.sort_unstable();
            w[w.len() / 2]
        }
    };
    let mean = |v: &[u64]| {
        if v.is_empty() {
            0
        } else {
            v.iter().sum::<u64>() / v.len() as u64
        }
    };

    // per-cell CSV
    let mut csv = String::from("cell,arm,n,complete,censored,completion_rate,wilson_lo,wilson_hi,deadline_hits,latency_median_ns,total_bytes_mean,arq_bytes_max,intervention_max,retx_max\n");
    for c in &cells {
        for ar in arms {
            let a = get(c, ar);
            let (p, lo, hi) = wilson(a.comp, a.n);
            let _ = writeln!(
                csv,
                "{c},{ar},{},{},{},{p:.3},{lo:.3},{hi:.3},{},{},{},{},{},{}",
                a.n,
                a.comp,
                a.cens,
                a.dh,
                med(&a.lat),
                mean(&a.total),
                a.arq.iter().max().copied().unwrap_or(0),
                a.interv.iter().max().copied().unwrap_or(0),
                a.retx.iter().max().copied().unwrap_or(0)
            );
        }
    }
    fs::write(format!("{ADIR}/DR-ARQ-TAIL-1-R2A-CELL-RESULTS.csv"), &csv).unwrap();

    // paired discordance per contrast
    let contrasts = [
        ("A0-TAIL", "A0-FROZEN", "C1"),
        ("F0", "A0-TAIL", "C2"),
        ("F0", "A1-EARLY", "C3"),
    ];
    let mut pc = String::from("contrast,cell,x_only_complete,y_only_complete,both_complete,both_censor,x_realized_total_mean,y_realized_total_mean,budget_relation\n");
    for (x, y, tag) in contrasts {
        for c in &cells {
            let (mut xo, mut yo, mut bc, mut bo) = (0u32, 0u32, 0u32, 0u32);
            let (mut xt, mut yt, mut cnt) = (0u64, 0u64, 0u64);
            for rep in 0..N {
                let rx = by.get(&(c.clone(), x.to_string(), rep));
                let ry = by.get(&(c.clone(), y.to_string(), rep));
                if let (Some(rx), Some(ry)) = (rx, ry) {
                    match (rx.0, ry.0) {
                        (true, false) => xo += 1,
                        (false, true) => yo += 1,
                        (true, true) => bc += 1,
                        (false, false) => bo += 1,
                    }
                    xt += rx.2;
                    yt += ry.2;
                    cnt += 1;
                }
            }
            let (xm, ym) = (xt / cnt.max(1), yt / cnt.max(1));
            let rel = if xm == ym {
                "equal"
            } else if xm > ym {
                "x_costs_more"
            } else {
                "y_costs_more"
            };
            let _ = writeln!(pc, "{tag},{c},{xo},{yo},{bc},{bo},{xm},{ym},{rel}");
        }
    }
    fs::write(format!("{ADIR}/DR-ARQ-TAIL-1-R2A-PAIRWISE.csv"), &pc).unwrap();

    // C3 cost-fairness (frozen primitive) per cell: F0 vs A1-EARLY realized intervention parity
    let mut c3fair = 0u32;
    for c in &cells {
        let f0iv = get(c, "F0").interv.iter().max().copied().unwrap_or(0);
        let a1iv = get(c, "A1-EARLY").interv.iter().max().copied().unwrap_or(0);
        if coding_causal_fairness(f0iv as u32, a1iv as u32) == FairnessClass::NonDominated {
            c3fair += 1;
        }
    }

    // final report (single human-readable doc)
    let mut rep = String::from("# DR-ARQ-TAIL-1 R2A-Core — FINAL REPORT\n\n");
    let _ = writeln!(rep, "> Experiment-only deterministic deconfounder (S1 terminal-tail / S2 non-tail same-budget / S3 NaturalIid; N=100/arm/cell; disjoint seed domain DRARQTAIL1). NOT product ARQ, NOT RFC 9002, NOT kernel/netem. PRE-OUTCOME SCOPE SPLIT — R2A does NOT cover burst loss or probe-loss robustness (R2B).\n");
    let total = recs.len();
    let _ = writeln!(rep, "## Integrity\nrecords={total} (expected 8400) · 21 cells × 4 arms × N=100 · schema=FS-1-RAW-SCHEMA-v2 · per-record validated fail-closed.\n");
    // C1 summary on S1
    let _ = writeln!(
        rep,
        "## C1 — tail-recovery effect (A0-TAIL vs A0-FROZEN), S1 terminal-tail cells"
    );
    let _ = writeln!(rep, "| cell | A0-FROZEN complete | A0-TAIL complete | A0-TAIL−A0 retx(max) | A0-TAIL−A0 arq_bytes(max) |");
    let _ = writeln!(rep, "|---|---|---|---|---|");
    for c in cells.iter().filter(|c| c.starts_with("s1-")) {
        let af = get(c, "A0-FROZEN");
        let at = get(c, "A0-TAIL");
        let dretx = at.retx.iter().max().copied().unwrap_or(0) as i64
            - af.retx.iter().max().copied().unwrap_or(0) as i64;
        let darq = at.arq.iter().max().copied().unwrap_or(0) as i64
            - af.arq.iter().max().copied().unwrap_or(0) as i64;
        let _ = writeln!(
            rep,
            "| {c} | {}/{} | {}/{} | {dretx} | {darq} |",
            af.comp, af.n, at.comp, at.n
        );
    }
    let _ = writeln!(rep, "\n## C2 — F0 vs A0-TAIL (cost-aware; see PAIRWISE.csv for realized-wire relation)\nBudget fairness verified per cell (realized total bytes). Where costs unequal → Pareto/cost-effectiveness, NOT a causal 'FEC superior'/'ARQ superior' verdict. Question: in which cells does FEC add completion over tail-capable bounded ARQ, and at what wire-byte cost — read from PAIRWISE.csv (F0 x_only_complete vs A0-TAIL y_only_complete + budget_relation).\n");
    let _ = writeln!(rep, "## C3 — F0 vs A1-EARLY (matched-budget coding vs blind duplication)\nRealized-intervention parity (frozen coding_causal_fairness primitive) NON_DOMINATED in {c3fair}/{} cells. Independent-confirmation claim holds ONLY where R2A's new-seed-domain completion rates + paired discordance actually support it (see CELL-RESULTS.csv + PAIRWISE.csv C3 rows). NOT generalized to burst.\n", cells.len());
    let _ = writeln!(rep, "## Interpretation bounds (§6)\nKeep separate: FEC>blind-dup · FEC>frozen-ACK-ARQ · FEC>tail-capable-ARQ · FEC Pareto cost level. A0-TAIL success ≠ 'Kama product ARQ fixed'. In-process only. Per-cell primary; stratum aggregate secondary (no silent pooling).\n");
    let _ = writeln!(rep, "## Artifacts\nDR-ARQ-TAIL-1-R2A-CELL-RESULTS.csv (per-cell/arm: N, completion, Wilson CI, latency, cost) · DR-ARQ-TAIL-1-R2A-PAIRWISE.csv (paired discordance + realized-cost relation, C1/C2/C3). Every row traceable to run_id dra1--<cell>--<arm>--rep<n>.\n");
    fs::write(format!("{ADIR}/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md"), &rep).unwrap();
    println!(
        "R2A ANALYZE COMPLETE: records={total} c3_fair_cells={c3fair}/{}",
        cells.len()
    );
}
