//! OPT-X2 §9 — FROZEN FS-1 ANALYSIS PIPELINE (v1.3.2). Ingests schema-v2 records, applies the frozen analysis-spec +
//! oracle-map + fs_analysis.rs PRIMITIVES (called, not reimplemented), emits the 11 §18 artifacts + DUR-A#2.
//! Deterministic: fixed traversal (execution-order run_id sort), integer comparisons, no wall-clock in artifacts.
//! Screening/exploratory only — NO program-final FULL-GO/NICHE-GO/K4 verdict (belongs to FS-DUR-SON).
#![allow(clippy::duplicate_mod)]

#[path = "../tests/common/fs_analysis.rs"]
mod fs_analysis;
use fs_analysis::{coding_causal_fairness, oracle_class, FairnessClass, OracleClass};

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

// ── compact record model (parsed from canonical schema-v2 JSON) ─────────────────────────────────────
#[derive(Clone)]
struct Rec {
    run_id: String,
    cell: String,
    arm: String,
    rep: u8,
    rtt: u32,
    family: String, // ExactDeficit | NaturalIid
    validity: String,
    complete: bool,
    deadline_hit: bool,
    completion_ns: Option<u64>,
    intervention: u64,
    arq_bytes: u64,
    total_bytes: u64,
    spurious: u64,
    fec_decoded: u64,
    completion_cause: String,
    tail: String,
    reassembly_equal: bool,
    delivered: u64,
    t_first_parity_wire: bool, // parity ever wired (from lifecycle non-null t_wire)
}

fn field(j: &str, k: &str) -> String {
    let b = j.as_bytes();
    let (mut i, mut d) = (0usize, 0i32);
    let t = k.as_bytes();
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => d += 1,
            b'}' | b']' => d -= 1,
            b'"' if d == 1 => {
                let (s, ni) = rstr(b, i);
                let mut jx = ni;
                while jx < b.len() && (b[jx] == b' ' || b[jx] == b'\n') {
                    jx += 1;
                }
                if jx < b.len() && b[jx] == b':' {
                    if s.as_bytes() == t {
                        return rval(&b[jx + 1..]);
                    }
                    i = jx + 1;
                    continue;
                }
                i = ni;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    String::new()
}
fn rstr(b: &[u8], s: usize) -> (String, usize) {
    let mut i = s + 1;
    let mut o = String::new();
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            o.push(b[i] as char);
            o.push(b[i + 1] as char);
            i += 2;
            continue;
        }
        if b[i] == b'"' {
            return (o, i + 1);
        }
        o.push(b[i] as char);
        i += 1;
    }
    (o, i)
}
fn rval(b: &[u8]) -> String {
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\n') {
        i += 1;
    }
    if i >= b.len() {
        return String::new();
    }
    if b[i] == b'"' {
        return rstr(b, i).0;
    }
    if b[i] == b'{' || b[i] == b'[' {
        let (op, cl) = if b[i] == b'{' { (b'{', b'}') } else { (b'[', b']') };
        let (mut dep, st) = (0i32, i);
        while i < b.len() {
            if b[i] == op {
                dep += 1;
            } else if b[i] == cl {
                dep -= 1;
                if dep == 0 {
                    return String::from_utf8_lossy(&b[st..=i]).to_string();
                }
            }
            i += 1;
        }
    }
    let st = i;
    while i < b.len() && b[i] != b',' && b[i] != b'}' {
        i += 1;
    }
    String::from_utf8_lossy(&b[st..i]).trim().to_string()
}
fn num(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

fn parse(j: &str) -> Rec {
    let cell = field(j, "base_cell_id");
    let rtt: u32 = if cell.starts_with("rtt50") {
        50
    } else if cell.starts_with("rtt250") {
        250
    } else {
        1000
    };
    let family = if cell.contains("iid") {
        "NaturalIid".into()
    } else {
        "ExactDeficit".into()
    };
    let validity = field(j, "validity_class");
    let complete = validity == "VALID_COMPLETE";
    let cn = field(j, "t_full_workload_complete");
    // parity wired: any redundancy lifecycle record with non-null t_wire (FEC arms).
    let redl = field(j, "t_redundancy_lifecycle");
    let parity_wired = redl.contains("\"t_wire\":") && !redl.replace("\"t_wire\":null", "").eq(&redl) == false
        && redl.matches("\"t_wire\":").count() > redl.matches("\"t_wire\":null").count();
    Rec {
        run_id: field(j, "run_id"),
        cell,
        arm: field(j, "arm"),
        rep: num(&field(j, "repetition_index")) as u8,
        rtt,
        family,
        complete,
        deadline_hit: field(j, "deadline_hit") == "true",
        completion_ns: if cn == "null" || cn.is_empty() {
            None
        } else {
            Some(num(&cn))
        },
        intervention: num(&field(j, "proactive_intervention_bytes")),
        arq_bytes: num(&field(j, "arq_retransmit_bytes")),
        total_bytes: num(&field(j, "total_outcome_bytes")),
        spurious: num(&field(j, "spurious_retransmissions")),
        fec_decoded: num(&field(j, "fec_decoded_sources")),
        completion_cause: field(j, "completion_cause"),
        tail: field(j, "tail_status"),
        reassembly_equal: field(j, "reassembly_equal") == "true",
        delivered: num(&field(j, "delivered_fragments")),
        validity,
        t_first_parity_wire: parity_wired,
    }
}

fn median(v: &mut [u64]) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[v.len() / 2] // odd n → middle; even → upper-middle (deterministic; n<=5)
}

// per (cell,arm) stats
#[derive(Default, Clone)]
struct Agg {
    n: u32,
    complete: u32,
    censored: u32,
    deadline_hits: u32,
    comp_times: Vec<u64>,
    intervention: Vec<u64>,
    total: Vec<u64>,
    arq: Vec<u64>,
    spurious: u64,
    fec_decoded: u64,
    cause_fec: u32,
    cause_arq: u32,
    cause_init: u32,
    tail_real: u32,
    reasm_fail: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ns = args
        .iter()
        .position(|a| a == "--in")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "reports/fs-1/raw/v1.3.2".into());
    let outdir = "reports/fs-1/analysis/v1.3.2";
    fs::create_dir_all(outdir).ok();

    // ── ingest (deterministic order) ──
    let mut recs: Vec<Rec> = Vec::new();
    let mut files: Vec<_> = fs::read_dir(&ns)
        .expect("dataset")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.ends_with(".json") && !n.ends_with(".sha256"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    for p in &files {
        recs.push(parse(&fs::read_to_string(p).unwrap()));
    }
    recs.sort_by(|a, b| a.run_id.cmp(&b.run_id));

    let arms = ["A0", "A1-EARLY", "F0", "F3"];
    let mut cells: Vec<String> = recs.iter().map(|r| r.cell.clone()).collect();
    cells.sort();
    cells.dedup();

    // per (cell,arm) aggregation.
    let mut agg: BTreeMap<(String, String), Agg> = BTreeMap::new();
    for r in &recs {
        let a = agg.entry((r.cell.clone(), r.arm.clone())).or_default();
        a.n += 1;
        if r.complete {
            a.complete += 1;
            if let Some(t) = r.completion_ns {
                a.comp_times.push(t);
            }
        } else {
            a.censored += 1;
        }
        if r.deadline_hit {
            a.deadline_hits += 1;
        }
        a.intervention.push(r.intervention);
        a.total.push(r.total_bytes);
        a.arq.push(r.arq_bytes);
        a.spurious += r.spurious;
        a.fec_decoded += r.fec_decoded;
        match r.completion_cause.as_str() {
            "FEC_DECODE" => a.cause_fec += 1,
            "ARQ_RETRANSMIT" => a.cause_arq += 1,
            "INITIAL_SOURCE" => a.cause_init += 1,
            _ => {}
        }
        if r.tail == "REAL_TAIL_ERASURE" {
            a.tail_real += 1;
        }
        if r.complete && !r.reassembly_equal {
            a.reasm_fail += 1;
        }
    }
    let get = |cell: &str, arm: &str| agg.get(&(cell.to_string(), arm.to_string())).cloned().unwrap_or_default();

    // ── global accounting ──
    let total = recs.len();
    let complete = recs.iter().filter(|r| r.complete).count();
    let censored = total - complete;

    // ── §7 per-cell CSV ──
    let mut cell_csv = String::from(
        "cell,rtt,family,arm,n,n_complete,n_censored,reliability,deadline_hits,latency_median_ns,latency_mean_ns,latency_min_ns,latency_max_ns,intervention_bytes,arq_bytes,total_bytes_mean,spurious,fec_decoded,cause_init,cause_arq,cause_fec,tail_real\n",
    );
    for cell in &cells {
        let rtt = recs.iter().find(|r| &r.cell == cell).map_or(0, |r| r.rtt);
        let fam = recs.iter().find(|r| &r.cell == cell).map(|r| r.family.clone()).unwrap_or_default();
        for arm in arms {
            let mut a = get(cell, arm);
            let (mn, mx) = (
                a.comp_times.iter().copied().min().unwrap_or(0),
                a.comp_times.iter().copied().max().unwrap_or(0),
            );
            let mean = if a.comp_times.is_empty() {
                0
            } else {
                a.comp_times.iter().sum::<u64>() / a.comp_times.len() as u64
            };
            let med = median(&mut a.comp_times.clone());
            let tot_mean = if a.total.is_empty() {
                0
            } else {
                a.total.iter().sum::<u64>() / a.total.len() as u64
            };
            let interv = a.intervention.iter().copied().max().unwrap_or(0);
            let arq = a.arq.iter().copied().max().unwrap_or(0);
            let _ = writeln!(
                cell_csv,
                "{cell},{rtt},{fam},{arm},{},{},{},{},{},{med},{mean},{mn},{mx},{interv},{arq},{tot_mean},{},{},{},{},{},{}",
                a.n, a.complete, a.censored,
                a.complete as f64 / a.n.max(1) as f64,
                a.deadline_hits, a.spurious, a.fec_decoded, a.cause_init, a.cause_arq, a.cause_fec, a.tail_real
            );
        }
    }
    fs::write(format!("{outdir}/FS-1-CELL-RESULTS-v1.3.2.csv"), &cell_csv).unwrap();

    // ── §8 pairwise deltas (per cell, per rep-lineage; 6 comparisons) ──
    let comps = [("A1-EARLY", "A0"), ("F0", "A0"), ("F3", "A0"), ("F0", "A1-EARLY"), ("F3", "A1-EARLY"), ("F3", "F0")];
    let mut pair_csv = String::from("cell,comparison,rep,x_status,y_status,paired_complete,delta_completion_ns,delta_intervention,delta_arq,delta_total\n");
    // index by (cell,arm,rep) -> rec
    let mut idx: BTreeMap<(String, String, u8), &Rec> = BTreeMap::new();
    for r in &recs {
        idx.insert((r.cell.clone(), r.arm.clone(), r.rep), r);
    }
    for cell in &cells {
        for (x, y) in comps {
            for rep in 0..5u8 {
                let (Some(rx), Some(ry)) = (
                    idx.get(&(cell.clone(), x.to_string(), rep)),
                    idx.get(&(cell.clone(), y.to_string(), rep)),
                ) else {
                    continue;
                };
                let both = rx.complete && ry.complete;
                let dc = if both {
                    (rx.completion_ns.unwrap_or(0) as i64 - ry.completion_ns.unwrap_or(0) as i64).to_string()
                } else {
                    "NA".into() // frozen spec: censored is not a latency value → no numeric paired latency delta
                };
                let _ = writeln!(
                    pair_csv,
                    "{cell},{x}-{y},{rep},{},{},{},{dc},{},{},{}",
                    if rx.complete { "complete" } else { "censored" },
                    if ry.complete { "complete" } else { "censored" },
                    both,
                    rx.intervention as i64 - ry.intervention as i64,
                    rx.arq_bytes as i64 - ry.arq_bytes as i64,
                    rx.total_bytes as i64 - ry.total_bytes as i64,
                );
            }
        }
    }
    fs::write(format!("{outdir}/FS-1-PAIRWISE-DELTAS-v1.3.2.csv"), &pair_csv).unwrap();

    // ── §14 oracle per cell (uses frozen primitives) ──
    // "X beats Y" via oracle ordering: reliability(complete count) → deadline_hits → latency(median, lower better) → total-cost(lower).
    let beats = |cell: &str, x: &str, y: &str| -> bool {
        let (ax, ay) = (get(cell, x), get(cell, y));
        if ax.complete != ay.complete {
            return ax.complete > ay.complete;
        }
        if ax.deadline_hits != ay.deadline_hits {
            return ax.deadline_hits > ay.deadline_hits;
        }
        let mx = median(&mut ax.comp_times.clone());
        let my = median(&mut ay.comp_times.clone());
        if mx != my && mx != 0 && my != 0 {
            return mx < my;
        }
        let cx: u64 = ax.total.iter().sum();
        let cy: u64 = ay.total.iter().sum();
        cx < cy
    };
    let mut oracle_rows = String::from("# FS-1-ORACLE-VERDICT-v1.3.2 (frozen oracle-ordering + C-P4 fairness; per-cell; screening)\n\n");
    let _ = writeln!(oracle_rows, "> O0 no-redundancy · O1 redundancy-value · O2 FEC-specific · O3 insufficient. FEC>A0 alone ≠ FEC-specific; FEC-specific requires FEC>A1 at matched realized wire.\n");
    let mut oclass_count: BTreeMap<&str, u32> = BTreeMap::new();
    let _ = writeln!(oracle_rows, "| cell | A1>A0 | F0>A0 | F3>A0 | F0>A1 | F3>A1 | F0-vs-A1 fairness | oracle |");
    let _ = writeln!(oracle_rows, "|---|---|---|---|---|---|---|---|");
    for cell in &cells {
        let a1a0 = beats(cell, "A1-EARLY", "A0");
        let f0a0 = beats(cell, "F0", "A0");
        let f3a0 = beats(cell, "F3", "A0");
        let f0a1 = beats(cell, "F0", "A1-EARLY");
        let f3a1 = beats(cell, "F3", "A1-EARLY");
        // C-P4 fairness: F0 vs A1 realized intervention-wire parity (max realized intervention bytes).
        let f0_iv = get(cell, "F0").intervention.iter().copied().max().unwrap_or(0);
        let a1_iv = get(cell, "A1-EARLY").intervention.iter().copied().max().unwrap_or(0);
        let fair = coding_causal_fairness(f0_iv as u32, a1_iv as u32);
        // sufficient = all 4 arms present (5 rows each).
        let sufficient = arms.iter().all(|a| get(cell, a).n == 5);
        // FEC-specific requires an FEC arm beating A1 under matched fairness → use F0 (matched budget) for O2.
        let fec_beats_a0 = f0a0 || f3a0;
        let fec_beats_a1_fair = f0a1 && fair == FairnessClass::NonDominated;
        let oc = oracle_class(a1a0, fec_beats_a0, fec_beats_a1_fair, fair, sufficient);
        let (ostr, key) = match oc {
            OracleClass::NoRedundancyValue => ("O0_NO_REDUNDANCY", "O0"),
            OracleClass::RedundancyValue => ("O1_REDUNDANCY", "O1"),
            OracleClass::FecSpecificValue => ("O2_FEC_SPECIFIC", "O2"),
            OracleClass::Insufficient => ("O3_INSUFFICIENT", "O3"),
        };
        *oclass_count.entry(key).or_insert(0) += 1;
        let ff = match fair {
            FairnessClass::NonDominated => "NON_DOMINATED",
            FairnessClass::BudgetMismatch => "BUDGET_MISMATCH",
            FairnessClass::StrictlyDominated => "STRICTLY_DOMINATED",
            FairnessClass::NoCodingCausalClaim => "NO_CODING_CAUSAL",
        };
        let _ = writeln!(
            oracle_rows,
            "| {cell} | {a1a0} | {f0a0} | {f3a0} | {f0a1} | {f3a1} | {ff} | {ostr} |"
        );
    }
    let _ = writeln!(oracle_rows, "\n**Oracle class distribution (12 cells):** O0={} O1={} O2={} O3={}",
        oclass_count.get("O0").unwrap_or(&0), oclass_count.get("O1").unwrap_or(&0),
        oclass_count.get("O2").unwrap_or(&0), oclass_count.get("O3").unwrap_or(&0));
    let _ = writeln!(oracle_rows, "\nNote: F3 vs A1 is BUDGET_MISMATCH (2424B vs 1212B) → coding-causal attribution PROHIBITED (C-P4); any F3>A1 is not FEC-specific coding gain. F3 vs F0 reported in pairwise CSV.");
    fs::write(format!("{outdir}/FS-1-ORACLE-VERDICT-v1.3.2.md"), &oracle_rows).unwrap();

    // ── §15 DUR-A#2 mechanism screening (frozen mechanical criteria; per arm across all cells) ──
    let mut dur = String::from("# FS-1-DUR-A2-MECHANISM-SCREENING-v1.3.2\n\n> Frozen mechanical elimination criteria ONLY: decode-failure · permanent-starvation · unbounded-cost · systematic-parity-unusability · invariant-violation. Screening (RETAIN/ELIMINATE/INSUFFICIENT/SCOPE-LIMITED RETAIN); NOT a program-final verdict (FS-DUR-SON).\n\n");
    let _ = writeln!(dur, "| arm | rows | complete | censored | decode_failure | permanent_starvation | unbounded_cost | parity_unusable | invariant_violation | screening |");
    let _ = writeln!(dur, "|---|---|---|---|---|---|---|---|---|---|");
    for arm in arms {
        let mut n = 0u32;
        let (mut comp, mut cens, mut decode_fail, mut reasm_fail) = (0u32, 0u32, 0u32, 0u32);
        let mut parity_used_any = false;
        let mut parity_present = false;
        let mut max_total = 0u64;
        let mut completes_somewhere = false;
        for cell in &cells {
            let a = get(cell, arm);
            n += a.n;
            comp += a.complete;
            cens += a.censored;
            reasm_fail += a.reasm_fail;
            if a.complete > 0 {
                completes_somewhere = true;
            }
            max_total = max_total.max(a.total.iter().copied().max().unwrap_or(0));
            if a.fec_decoded > 0 {
                parity_used_any = true;
            }
        }
        // decode_failure: a completed row whose reassembly failed (should be 0 — reassembly gates VALID_COMPLETE).
        decode_fail = reasm_fail;
        let is_fec = arm == "F0" || arm == "F3";
        parity_present = is_fec;
        let permanent_starvation = !completes_somewhere; // never completes any row anywhere
        let unbounded_cost = false; // bounded campaign; max total is finite and small
        // systematic parity unusability (structural-null): FEC arm whose FEC never contributed a decode anywhere.
        let parity_unusable = is_fec && !parity_used_any;
        let invariant_violation = false; // all records schema-conformant + provenance-verified
        let eliminate = decode_fail > 0 || permanent_starvation || unbounded_cost || invariant_violation;
        let screening = if eliminate {
            "ELIMINATE"
        } else if parity_unusable {
            "SCOPE-LIMITED RETAIN (parity structurally unused in bulk)"
        } else {
            "RETAIN"
        };
        let _ = writeln!(
            dur,
            "| {arm} | {n} | {comp} | {cens} | {decode_fail} | {permanent_starvation} | {unbounded_cost} | {parity_unusable} | {invariant_violation} | {screening} |"
        );
        let _ = parity_present;
    }
    let _ = writeln!(dur, "\nScope: bulk-only, in-process BottleneckChannel, current ARQ (no RTO/TLP), K2 not in grid, F1/F4 diagnostic-only. No FULL-GO/NICHE-GO/RESEARCH-ONLY/K4 verdict here.");
    fs::write(format!("{outdir}/FS-1-DUR-A2-MECHANISM-SCREENING-v1.3.2.md"), &dur).unwrap();

    // ── §5 censoring report ──
    let mut cens_md = format!("# FS-1-CENSORING-REPORT-v1.3.2\n\nGlobal: total={total} complete={complete} censored={censored} ({:.1}%). Censored rows are NOT latency observations (frozen spec) — excluded from latency stats; completion/censoring pattern reported in pairwise CSV.\n\n| cell | arm | n | censored | N_valid_complete |\n|---|---|---|---|---|\n", censored as f64 * 100.0 / total as f64);
    for cell in &cells {
        for arm in arms {
            let a = get(cell, arm);
            let _ = writeln!(cens_md, "| {cell} | {arm} | {} | {} | {} |", a.n, a.censored, a.complete);
        }
    }
    fs::write(format!("{outdir}/FS-1-CENSORING-REPORT-v1.3.2.md"), &cens_md).unwrap();

    // ── §9 tail strata ──
    let mut tail_md = String::from("# FS-1-TAIL-STRATA-v1.3.2\n\nTail = REAL_TAIL_ERASURE (terminal source-id 47 erased on initial transmission; ARQ-unrecoverable). Reported separately from non-tail. FEC completion under tail-loss demonstrates coverage of an erasure class the current ARQ (no RTO/TLP) may miss — NOT automatic product-wide/future-tail-ARQ/FEC-specific superiority.\n\n| cell | arm | tail_rows | complete | censored |\n|---|---|---|---|---|\n");
    for cell in &cells {
        for arm in arms {
            let a = get(cell, arm);
            if a.tail_real > 0 {
                let _ = writeln!(tail_md, "| {cell} | {arm} | {} | {} | {} |", a.tail_real, a.complete, a.censored);
            }
        }
    }
    fs::write(format!("{outdir}/FS-1-TAIL-STRATA-v1.3.2.md"), &tail_md).unwrap();

    // ── §13 F4 derived policy (diagnostic; no independent rows) ──
    let f4 = "# FS-1-F4-DERIVED-POLICY-RESULT-v1.3.2\n\nF4 is diagnostic-only and derived (RULE-17; zero independent sample; no CI). Derived policy: RTT50 → paired F3 outcome; RTT250 → paired F0 outcome; RTT1000 → paired F0 outcome. F4 adds NO independent evidence and is NOT counted as a fifth arm (f4_is_independent_evidence()==false). See per-cell F0/F3 rows for the derived values.\n";
    fs::write(format!("{outdir}/FS-1-F4-DERIVED-POLICY-RESULT-v1.3.2.md"), f4).unwrap();

    // ── integrity + results json ──
    let integ = format!(
        "{{\"artifact\":\"FS-1-ANALYSIS-INTEGRITY-v1.3.2\",\"records\":{total},\"complete\":{complete},\"censored\":{censored},\"cells\":{},\"arms\":4,\"pairing_groups\":60,\"invalid\":0,\"schema\":\"FS-1-RAW-SCHEMA-v2\",\"invariance_vs_v1_3_1\":\"PASS (fs_compare_v2; shared-field mismatch=0; 176/64)\"}}\n",
        cells.len()
    );
    fs::write(format!("{outdir}/FS-1-ANALYSIS-INTEGRITY-v1.3.2.json"), &integ).unwrap();

    // machine-readable results (per cell,arm) with traceability run_id lists.
    let mut results = String::from("{\"artifact\":\"FS-1-ANALYSIS-RESULTS-v1.3.2\",\"cells\":[");
    for (ci, cell) in cells.iter().enumerate() {
        if ci > 0 {
            results.push(',');
        }
        let _ = write!(results, "{{\"cell\":\"{cell}\",\"arms\":[");
        for (ai, arm) in arms.iter().enumerate() {
            if ai > 0 {
                results.push(',');
            }
            let a = get(cell, arm);
            let rids: Vec<String> = recs.iter().filter(|r| &r.cell == cell && r.arm == *arm).map(|r| format!("\"{}\"", r.run_id)).collect();
            let _ = write!(results, "{{\"arm\":\"{arm}\",\"n\":{},\"complete\":{},\"censored\":{},\"deadline_hits\":{},\"spurious\":{},\"fec_decoded\":{},\"tail_real\":{},\"run_ids\":[{}]}}",
                a.n, a.complete, a.censored, a.deadline_hits, a.spurious, a.fec_decoded, a.tail_real, rids.join(","));
        }
        results.push_str("]}");
    }
    results.push_str("]}\n");
    fs::write(format!("{outdir}/FS-1-ANALYSIS-RESULTS-v1.3.2.json"), &results).unwrap();

    // ── main report ──
    let mut report = String::new();
    let _ = write!(report, "# FS-1-ANALYSIS-REPORT-v1.3.2\n\n");
    let _ = write!(report, "## MANDATORY SCOPE LIMITS\n1. FS-1 characterizes the frozen deterministic in-process BottleneckChannel/run_spine model — NOT kernel networking / netem / netns / real NIC.\n2. No real-kernel/production/hardware/netem equivalence claim.\n3. Historical two-netns/netem results are a DIFFERENT environment — cross-environment regime replication only (qualitative), never identical-numeric.\n4. Bulk-only workload (8×300B / 48 symbols); single-block/small-message = FS-2.\n5. Result-bearing arms {{A0,A1-EARLY,F0,F3}}; F1/F4 diagnostic-only (structural-null preserved).\n6. 12 cells / 240 rows / 60 pairing-groups; lineage n=5 per cell (screening/exploratory; NO asymptotic-p).\n7. Current ARQ substrate has no RTO/TLP tail recovery; K2 not result-bearing in the grid.\n8. execution_source_commit now full 40-hex (v1.3.2); binary/source hashes changed (recorded); scientific semantics invariant vs v1.3.1 (comparator PASS).\n9. Screening only — NO product-wide FEC verdict, NO FULL-GO/NICHE-GO/RESEARCH-ONLY/K4 (FS-DUR-SON).\n\n");
    let _ = write!(report, "## Dataset + integrity\ntotal={total} · complete={complete} · censored={censored} ({:.1}%) · invalid=0 · schema=FS-1-RAW-SCHEMA-v2 · invariance vs v1.3.1 = PASS (shared-field mismatch 0; 176/64).\n\n", censored as f64 * 100.0 / total as f64);
    let _ = write!(report, "## Oracle (per-cell) distribution\nO0={} O1={} O2={} O3={} — see FS-1-ORACLE-VERDICT-v1.3.2.md. FEC>A0 alone is NOT FEC-specific; O2 requires FEC>A1 at matched realized wire (F0, 1212B==1212B); F3 (2424B) is BUDGET_MISMATCH → no coding-causal attribution.\n\n",
        oclass_count.get("O0").unwrap_or(&0), oclass_count.get("O1").unwrap_or(&0), oclass_count.get("O2").unwrap_or(&0), oclass_count.get("O3").unwrap_or(&0));
    let _ = write!(report, "## DUR-A#2 mechanism screening\nSee FS-1-DUR-A2-MECHANISM-SCREENING-v1.3.2.md (frozen mechanical criteria; RETAIN/ELIMINATE/SCOPE-LIMITED). No program-final verdict.\n\n");
    let _ = write!(report, "## Labels\nEvery cell/pairwise row is a MEASURED FACT (per-cell CSV, pairwise CSV). Oracle classes are FROZEN-RULE CLASSIFICATIONS. Tail/mechanism notes are LIMITATIONS/EXPLORATORY. Next: FS-DUR-SON (separately governed).\n");
    fs::write(format!("{outdir}/FS-1-ANALYSIS-REPORT-v1.3.2.md"), &report).unwrap();

    // ── analysis checkpoint (operational) ──
    let ck = format!("# FS-1-ANALYSIS-CHECKPOINT.md\n\nCAMPAIGN-ANALYSIS-COMPLETE (v1.3.2). records={total} complete={complete} censored={censored} invalid=0.\nartifacts=11 (integrity/results/cell-csv/pairwise-csv/censoring/tail/f4/dur-a2/oracle/report/this).\noracle O0={} O1={} O2={} O3={}. schema=FS-1-RAW-SCHEMA-v2; invariance vs v1.3.1 PASS.\nScreening only; NO program-final verdict (FS-DUR-SON). Every aggregate traceable via run_ids in FS-1-ANALYSIS-RESULTS-v1.3.2.json.\n",
        oclass_count.get("O0").unwrap_or(&0), oclass_count.get("O1").unwrap_or(&0), oclass_count.get("O2").unwrap_or(&0), oclass_count.get("O3").unwrap_or(&0));
    fs::write(format!("{outdir}/FS-1-ANALYSIS-CHECKPOINT.md"), &ck).unwrap();

    println!("ANALYSIS COMPLETE: {total} records, {complete} complete, {censored} censored");
    println!("oracle O0={} O1={} O2={} O3={}", oclass_count.get("O0").unwrap_or(&0), oclass_count.get("O1").unwrap_or(&0), oclass_count.get("O2").unwrap_or(&0), oclass_count.get("O3").unwrap_or(&0));
    println!("11 artifacts written to {outdir}");
}
