//! DR-ARQ-TAIL-1 R1 — DIAGNOSTIC MECHANISM REPLAY (NOT confirmatory; uses previously-seen FS-1 tail configs).
//! Replays every FS-1 ExactDeficit (cell,rep) whose realized initial-source erasure includes the terminal id (47),
//! under A0-FROZEN / A0-TAIL / F0. Answers the mechanical question: how many of the frozen-A0-censored terminal-tail
//! rows does the bounded tail probe close, at what added ARQ retransmit cost, and does F0 still complete them.
#![allow(clippy::duplicate_mod)]
#[path = "../tests/common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;
use std::fmt::Write as _;

fn main() {
    let cells = amended_base_cells();
    let mut tail_rows = 0u32;        // terminal-tail erasure rows (arm-invariant config)
    let mut a0_censored = 0u32;      // of those, A0-FROZEN censors
    let mut a0tail_recovered = 0u32; // A0-TAIL closes (completes) an A0-censored tail row
    let mut f0_completed = 0u32;     // F0 completes the same rows
    let mut added_retx_bytes = 0u64; // A0-TAIL added ARQ retransmit bytes over A0
    let mut probe_rows = Vec::new();
    for c in &cells {
        let cid = base_cell_id(c);
        if !cid.contains("exact") { continue; } // ExactDeficit tail strata only
        for rep in 0..5u8 {
            let a0 = run_spine(SpineArm::A0, &build_cfg(c, rep));
            let is_tail = a0.realized_initial_source_drop_ids.contains(&47);
            if !is_tail { continue; }
            tail_rows += 1;
            if a0.completed { continue; }
            a0_censored += 1;
            let a0t = run_spine(SpineArm::A0Tail, &build_cfg(c, rep));
            let f0 = run_spine(SpineArm::F0 { r: 2 }, &build_cfg(c, rep));
            if a0t.completed { a0tail_recovered += 1; }
            if f0.completed { f0_completed += 1; }
            added_retx_bytes += a0t.arq_retransmit_bytes.saturating_sub(a0.arq_retransmit_bytes);
            probe_rows.push(format!("{cid} rep{rep}: A0=censored A0-TAIL={} F0={} +retx_bytes={}",
                if a0t.completed {"COMPLETE"} else {"censored"},
                if f0.completed {"COMPLETE"} else {"censored"},
                a0t.arq_retransmit_bytes.saturating_sub(a0.arq_retransmit_bytes)));
        }
    }
    let mut md = String::from("# DR-ARQ-TAIL-1 R1 — DIAGNOSTIC MECHANISM REPLAY\n\n");
    let _ = writeln!(md, "> Label: **DIAGNOSTIC MECHANISM REPLAY** (uses previously-seen FS-1 terminal-tail configs; NOT confirmatory).\n> Experiment-only deterministic tail-recovery deconfounder — NOT a product ARQ, NOT RFC 9002, NOT kernel/netem.\n");
    let _ = writeln!(md, "## Terminal-tail rows (ExactDeficit; realized erasure includes terminal source-id 47)\n");
    let _ = writeln!(md, "- terminal-tail erasure rows: **{tail_rows}**");
    let _ = writeln!(md, "- of those, A0-FROZEN censors: **{a0_censored}**");
    let _ = writeln!(md, "- **A0-TAIL closes (completes): {a0tail_recovered} / {a0_censored}**");
    let _ = writeln!(md, "- F0 completes the same rows: **{f0_completed} / {a0_censored}**");
    let _ = writeln!(md, "- A0-TAIL added ARQ retransmit bytes (total over the recovered set): {added_retx_bytes}");
    let _ = writeln!(md, "\n## Per-row (traceable)\n");
    for r in &probe_rows { let _ = writeln!(md, "- {r}"); }
    let _ = writeln!(md, "\n## Interpretation bounds (§6)\nA0-TAIL success does NOT mean 'Kama product ARQ fixed'. It shows the bounded probe recovers terminal-tail erasures the frozen ACK-driven ARQ cannot. F0 vs A0-TAIL cost-fairness is assessed in R2 (separate preregistered grid); this replay is diagnostic only.");
    std::fs::write("reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R1-DIAGNOSTIC.md", &md).unwrap();
    println!("R1: tail_rows={tail_rows} a0_censored={a0_censored} a0tail_recovered={a0tail_recovered} f0_completed={f0_completed} added_retx_bytes={added_retx_bytes}");
}
