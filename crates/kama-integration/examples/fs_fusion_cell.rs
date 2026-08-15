//! FS-FUSION-CELL-1 — the (tail-capable ARQ, r>0) cell that no authoritative dataset contains.
//!
//! FS-FUSION-CAL-1's framing corrigendum §3 records that cell as "genuinely absent from every
//! authoritative dataset": the coverage matrix has r0/r2/r4 for frozen ARQ but only r0 for the
//! tail-capable arm. Every FEC result in the repo therefore compares coding against an ARQ that is
//! deliberately tail-blind, and R2A C2 already showed most of FEC's tail advantage was that
//! blindness rather than coding.
//!
//! Question, and only this question: **does proactive redundancy still pay once ARQ can already
//! recover its own tail?**
//!
//! Design, decision rule and limits are frozen in reports/fs-fusion-cell-1/PREREGISTRATION.md,
//! committed before this ran. Screening at N=30 — no promotion, no p-values, no deadline claims.
#![allow(clippy::duplicate_mod)]
#[path = "../tests/common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;
use std::fmt::Write as _;

/// Repetitions are **derived per cell from the erasure generator's own limit**, never chosen.
///
/// `exact_deficit_target_set` carves a DISJOINT window per repetition out of the source symbols:
/// `ranked[rep * d .. rep * d + d]` over `TOTAL_SOURCE_SYMBOLS`. So the largest usable rep count
/// is `TOTAL_SOURCE_SYMBOLS / d`, and asking for more panics with an out-of-range slice.
///
/// A literal here would have been a guess about someone else's generator. The first draft guessed
/// 30, which is fine at d=1 (limit 48) and impossible at d=4 (limit 12) -- it ran one cell and then
/// panicked. Deriving it means the campaign takes every repetition each cell can actually supply.
fn reps_for(c: &BaseCell) -> u8 {
    let d = c
        .impairment
        .configured_exact_drop_count()
        .unwrap_or(1)
        .max(1);
    let generator_limit = (FS1_SOURCE_SYMBOLS / u16::from(d)) as u8;
    // Cap so a d=1 cell does not run 48 reps while a d=4 cell runs 12; the comparison stays
    // balanced across the grid at the tightest cell's limit.
    generator_limit.min(12)
}

struct Paired {
    fusion_only: u32,
    tail_only: u32,
    both: u32,
    neither: u32,
    fusion_bytes: u64,
    tail_bytes: u64,
    f0_complete: u32,
    f0_bytes: u64,
}

fn verdict(p: &Paired) -> &'static str {
    let cost = p.fusion_bytes as i64 - p.tail_bytes as i64;
    match (p.fusion_only.cmp(&p.tail_only), cost) {
        (std::cmp::Ordering::Greater, c) if c > 0 => "FUSION_PAYS_AT_A_COST",
        (std::cmp::Ordering::Greater, _) => "FUSION_DOMINATES",
        (std::cmp::Ordering::Equal, _) => "NO_COMPLETION_CHANGE",
        (std::cmp::Ordering::Less, _) => "FUSION_HARMS",
    }
}

fn main() {
    let cells = amended_base_cells();
    let mut csv = String::from(
        "cell,n,tail_complete,fusion_complete,f0_complete,fusion_only,tail_only,both,neither,\
         tail_bytes_mean,fusion_bytes_mean,f0_bytes_mean,cost_delta_mean,verdict\n",
    );
    let mut totals = (0u32, 0u32, 0u32, 0u32);
    let mut cells_run = 0u32;

    for c in &cells {
        let cid = base_cell_id(c);
        let mut p = Paired {
            fusion_only: 0,
            tail_only: 0,
            both: 0,
            neither: 0,
            fusion_bytes: 0,
            tail_bytes: 0,
            f0_complete: 0,
            f0_bytes: 0,
        };
        let (mut tc, mut fc) = (0u32, 0u32);
        let n_reps = reps_for(c);

        for rep in 0..n_reps {
            let cfg = build_cfg(c, rep);
            // Same cell, same rep, same seed across all three arms — the pairing the
            // preregistration fixes.
            let tail = run_spine(SpineArm::A0Tail { r: 0 }, &cfg);
            let fusion = run_spine(SpineArm::A0Tail { r: 2 }, &cfg);
            let f0 = run_spine(SpineArm::F0 { r: 2 }, &cfg);

            tc += u32::from(tail.completed);
            fc += u32::from(fusion.completed);
            p.f0_complete += u32::from(f0.completed);
            p.tail_bytes += tail.total_outcome_bytes;
            p.fusion_bytes += fusion.total_outcome_bytes;
            p.f0_bytes += f0.total_outcome_bytes;

            match (fusion.completed, tail.completed) {
                (true, false) => p.fusion_only += 1,
                (false, true) => p.tail_only += 1,
                (true, true) => p.both += 1,
                (false, false) => p.neither += 1,
            }
        }

        let n = u64::from(n_reps);
        let (tb, fb, f0b) = (p.tail_bytes / n, p.fusion_bytes / n, p.f0_bytes / n);
        let v = verdict(&p);
        writeln!(
            csv,
            "{cid},{n_reps},{tc},{fc},{},{},{},{},{},{tb},{fb},{f0b},{},{v}",
            p.f0_complete,
            p.fusion_only,
            p.tail_only,
            p.both,
            p.neither,
            fb as i64 - tb as i64
        )
        .ok();

        totals.0 += p.fusion_only;
        totals.1 += p.tail_only;
        totals.2 += p.both;
        totals.3 += p.neither;
        cells_run += 1;
        eprintln!(
            "  {cid}: tail {tc}/{n_reps} fusion {fc}/{n_reps} f0 {}/{n_reps} | fusion_only {} tail_only {} | {v}",
            p.f0_complete, p.fusion_only, p.tail_only
        );
    }

    std::fs::create_dir_all("reports/fs-fusion-cell-1").ok();
    std::fs::write("reports/fs-fusion-cell-1/CELL-RESULTS.csv", &csv).ok();
    println!("FS-FUSION-CELL-1 DONE cells={cells_run} (reps derived per cell)");
    println!(
        "  fusion_only={} tail_only={} both={} neither={}",
        totals.0, totals.1, totals.2, totals.3
    );
    println!("  reports/fs-fusion-cell-1/CELL-RESULTS.csv");
}
