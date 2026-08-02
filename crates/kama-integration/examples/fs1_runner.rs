//! DR-FEC-FS-1 S5-C — manifest-driven FS-1 runner. **Default mode = EXECUTABILITY_ONLY** (no channel-loop,
//! no raw-record, no run-id spent). **Result-bearing mode requires the explicit `--result-bearing` flag** AND
//! valid provenance (freeze-lock + manifest + binary + empty raw-namespace) — advances run_spine per manifest row,
//! classifies VALID_COMPLETE/VALID_CENSORED (INVALID = contract-failure), writes append-only raw records.
//! S5: result-bearing YASAK (freeze-lock henüz-YOK → refuse). This binary is the frozen S5-C execution baseline.
//!
//! Reproducible-build: no timestamp/hostname/workspace-path embedding (release + --remap-path-prefix + CARGO_INCREMENTAL=0).
#![allow(clippy::duplicate_mod)] // fs_dryrun + fs_spine bağımsız-harness-ağaçları fs_workload'ı ayrı-modül olarak içerir

#[path = "../tests/common/fs_dryrun.rs"]
mod fs_dryrun;
#[path = "../tests/common/fs_spine.rs"]
mod fs_spine;

use fs_dryrun::{
    amended_base_cells, base_cell_id, deadline_ms_for, exact_deficit_target_set, exact_rank_seed,
    executability_sweep, execution_order_entries, observation_horizon_ms, paired_trace_seed,
    BaseCell, FsImpairment, FS1_RESULT_BEARING_ARMS,
};
use fs_spine::{run_spine, SpineArm, SpineCfg, SpineResult, MS};

const LINK_BPS: u64 = 1_000_000;
const PACER_BPS: u64 = 800_000;
const PACE_DT_NS: u64 = 10_000; // elig-time'lar dt-katı (10µs) → 0.80C quantization-0

/// Manifest-row (base_cell, rep) → SpineCfg (owd=RTT/2; paced 0.80C; horizon = observation_horizon).
fn build_cfg(c: &BaseCell, rep: u8) -> SpineCfg {
    let owd = c.rtt_ms * MS / 2;
    let horizon_ns = observation_horizon_ms(c.rtt_ms) * MS;
    match c.impairment {
        FsImpairment::ExactDeficit { drop_count } => {
            let drops = exact_deficit_target_set(drop_count.get(), exact_rank_seed(c), rep);
            SpineCfg::diag(LINK_BPS, owd, &drops)
                .with_pacer(PACER_BPS)
                .with_dt(PACE_DT_NS)
                .with_horizon(horizon_ns)
        }
        FsImpairment::NaturalIid { loss_permille } => SpineCfg::iid(
            LINK_BPS,
            owd,
            u32::from(loss_permille.get()),
            paired_trace_seed(c, rep),
        )
        .with_pacer(PACER_BPS)
        .with_dt(PACE_DT_NS)
        .with_horizon(horizon_ns),
    }
}

fn arm_of(arm: &str) -> SpineArm {
    match arm {
        "A0" => SpineArm::A0,
        "A1-EARLY" => SpineArm::A1 { r: 2 },
        "F0" => SpineArm::F0 { r: 2 },
        "F3" => SpineArm::F3 { r: 2 },
        other => panic!("non-OPT-A arm: {other}"),
    }
}

/// Validity classification (§5). INVALID = contract-failure (provenance öncesi-elenir); burada VALID_*.
fn classify(r: &SpineResult) -> &'static str {
    if r.completed && r.reassembly_equal {
        "VALID_COMPLETE"
    } else {
        "VALID_CENSORED"
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result_bearing = args.iter().any(|a| a == "--result-bearing");

    if !result_bearing {
        // ── DEFAULT: EXECUTABILITY_ONLY (no loop, no raw, no run-id) ──
        let s = executability_sweep();
        println!("mode=EXECUTABILITY_ONLY result_bearing=false");
        println!(
            "total={} dispatchable={} unsupported={} ambiguous={} invalid={} duplicate={}",
            s.total, s.dispatchable, s.unsupported, s.ambiguous, s.invalid, s.duplicate
        );
        println!(
            "by-family ExactDeficit={} NaturalIid={}",
            s.by_family.get("ExactDeficit").copied().unwrap_or(0),
            s.by_family.get("NaturalIid").copied().unwrap_or(0)
        );
        // manifest-order preview (operational).
        let order = execution_order_entries();
        println!("execution_order_rows={} pairing_groups=60", order.len());
        if s.dispatchable != 240 || !s.failures.is_empty() {
            eprintln!("EXECUTABILITY FAIL: {:?}", s.failures);
            std::process::exit(2);
        }
        return;
    }

    // ── RESULT-BEARING: strict provenance gate (S5'te freeze-lock YOK → REFUSE) ──
    let freeze_lock = arg_value(&args, "--freeze-lock");
    let out_ns = arg_value(&args, "--out");
    if freeze_lock.is_none() || out_ns.is_none() {
        eprintln!("RESULT-BEARING REFUSED: --freeze-lock <path> ve --out <namespace> zorunlu (provenance-gate)");
        std::process::exit(3);
    }
    let lock_path = freeze_lock.unwrap();
    let out = out_ns.unwrap();
    if !std::path::Path::new(&lock_path).exists() {
        eprintln!("RESULT-BEARING REFUSED: freeze-lock '{lock_path}' YOK (S5-freeze tamamlanmadı → S6-öncesi execution YASAK)");
        std::process::exit(3);
    }
    if std::path::Path::new(&out).exists()
        && std::fs::read_dir(&out)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        eprintln!("RESULT-BEARING REFUSED: raw-namespace '{out}' BOŞ-DEĞİL (append-only firewall)");
        std::process::exit(3);
    }
    // NOTE: tam provenance-verify (freeze-lock↔manifest↔binary-hash eşleşmesi) S6-launch-review'de doğrulanır.
    // (S5'te bu koda ASLA ulaşılmaz: freeze-lock yok → yukarıda refuse. Aşağısı S6-yolu.)
    eprintln!("RESULT-BEARING requested; freeze-lock present. S6-launch-review + explicit authorization gerekir.");
    eprintln!("Bu binary result-bearing-YETENEKLİ; S5'te execution İCRA-EDİLMEZ (STOP).");
    std::process::exit(4);
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

/// (S6-yolu; S5'te çağrılmaz) tek-satır result-bearing execution → raw-record. run_spine deterministik.
#[allow(dead_code)]
fn execute_row(c: &BaseCell, arm: &str, rep: u8) -> String {
    let cfg = build_cfg(c, rep);
    let r = run_spine(arm_of(arm), &cfg);
    let validity = classify(&r);
    let deadline_hit = r
        .completion_ns
        .is_some_and(|t| t <= deadline_ms_for(c.rtt_ms) * MS);
    format!(
        "run_id=fs1--{}--{}--rep{} validity={} deadline_hit={} delivered={} total_bytes={}",
        base_cell_id(c),
        arm,
        rep,
        validity,
        deadline_hit,
        r.delivered_source_symbols,
        r.total_outcome_bytes
    )
}

/// (S6-yolu) execution-order üzerinden tüm 240-row. S5'te çağrılmaz.
#[allow(dead_code)]
fn all_result_rows() -> Vec<String> {
    let cells = amended_base_cells();
    let mut out = Vec::new();
    for (_, _, bcid, rep, arm, _) in execution_order_entries() {
        let c = cells.iter().find(|c| base_cell_id(c) == bcid).unwrap();
        let _ = FS1_RESULT_BEARING_ARMS.contains(&arm);
        out.push(execute_row(c, arm, rep));
    }
    out
}
