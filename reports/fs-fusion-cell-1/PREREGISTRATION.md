# FS-FUSION-CELL-1 — preregistration

**Frozen before any run.** Written and committed before the campaign exists, so the decision rule
cannot be chosen after seeing the numbers.

    branch      : transport-baseline-closure
    arm commit  : 87866477  (SpineArm::A0Tail { r })
    harness     : crates/kama-integration/tests/common/fs_spine.rs :: run_spine

## 1. The question

FS-FUSION-CAL-1's framing corrigendum §3 records one cell as missing from the entire corpus:

> the (tail-capable, r>0) fusion cell is genuinely absent from every authoritative dataset

The coverage matrix carries r0/r2/r4 for frozen ARQ but only r0 for tail-capable ARQ. Every FEC
result in the repo therefore compares coding against an ARQ that is **deliberately tail-blind**, and
DR-ARQ-TAIL-1 R2A already showed that most of FEC's apparent tail advantage was that blindness
rather than coding.

So the open question is narrow and specific:

> **Does proactive redundancy still pay once ARQ can already recover its own tail?**

## 2. Arms

| arm | label | meaning |
|---|---|---|
| tail-only | `A0-TAIL` | `A0Tail { r: 0 }` — tail-capable ARQ, no coding |
| **fusion** | `A0-TAIL+F` | `A0Tail { r: 2 }` — tail-capable ARQ **and** coding |
| coding-only | `F0` | `F0 { r: 2 }` — frozen ARQ + coding, the existing reference |

`A0-FROZEN` is not re-run: its behaviour on this grid is already banked and nothing here changes it.

## 3. Design

Screening, not confirmatory. Explicitly labelled as such because N is small.

    grid : the existing R2A grid (S1 tail / S2 non-tail / S3 IID), 3 RTTs x 3 deficits per stratum
    N    : 30 repetitions per cell per arm
    seeds: the existing per-cell repetition seeds, unchanged
    pairing: same cell, same rep, same seed across all three arms

Cost was measured before committing to it, from the fusion pin's own runtime (~2-3.5 s per run):
3 arms x N=30 x the grid, serial, worker=1.

## 4. Decision rule — frozen

Per cell, over the paired repetitions:

* `fusion_only_complete` — reps where `A0-TAIL+F` completes and `A0-TAIL` does not
* `tail_only_complete`  — reps where `A0-TAIL` completes and `A0-TAIL+F` does not
* `cost_delta`          — mean `total_outcome_bytes`, fusion minus tail-only

Per-cell verdict:

| condition | verdict |
|---|---|
| `fusion_only > tail_only` and cost_delta > 0 | `FUSION_PAYS_AT_A_COST` |
| `fusion_only > tail_only` and cost_delta <= 0 | `FUSION_DOMINATES` |
| `fusion_only == tail_only` | `NO_COMPLETION_CHANGE` |
| `fusion_only < tail_only` | `FUSION_HARMS` |

Programme-level reading, fixed now:

* If `NO_COMPLETION_CHANGE` dominates the tail strata (S1), then **coding adds nothing once ARQ is
  tail-capable**, and the historical FEC-versus-ARQ tail advantage was tail-blindness, exactly as
  R2A C2 suggested.
* If `FUSION_PAYS_AT_A_COST` appears where R2A found FEC Pareto-dominant (S3 IID-20%), the two
  mechanisms are complementary rather than redundant.
* `FUSION_HARMS` anywhere is reported, never suppressed.

## 5. What this run may NOT conclude

* No promotion decision, no default flip. This is screening at N=30.
* No p-values or confidence claims — N is chosen for feasibility, not power.
* Nothing about deadlines: R2A already recorded that **no arm** meets the frozen tight deadlines
  (`deadline_hit` 0 for A0-FROZEN, A1-EARLY and F0; 46 of 2100 for A0-TAIL). This campaign does not
  revisit that.
* Nothing about the product. `run_spine` is an in-process deterministic model, not netem, and the
  product tail mechanism ships default-OFF.

## 6. Integrity

* The runner is committed in the same commit as its results.
* `A0Tail { r: 0 }` is pinned byte-identical to the pre-existing tail-only arm, so the banked R2A
  rows remain valid (`fs_arqtail_pins::a0tail_r0_is_unchanged_by_the_fusion_parameter`).
* Seeds are not searched, skipped or replaced. A cell that produces an uncomfortable verdict is
  reported with it.
