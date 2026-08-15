# FS-FUSION-CELL-1 — FINAL REPORT

Screening result for the cell FS-FUSION-CAL-1's corrigendum named as absent from the entire corpus:
**tail-capable ARQ together with proactive redundancy.**

    prereg   : reports/fs-fusion-cell-1/PREREGISTRATION.md  (committed before the run)
    runner   : crates/kama-integration/examples/fs_fusion_cell.rs
    arm      : SpineArm::A0Tail { r }  (87866477)
    data     : reports/fs-fusion-cell-1/CELL-RESULTS.csv
    12 cells, repetitions derived per cell from the erasure generator's limit, 3 arms, 0 failures

## 1. Result

| stratum | cells | tail-only | fusion | F0 | verdict |
|---|---|---|---|---|---|
| ExactDeficit (tail) | 9 | **12/12 every cell** | **12/12 every cell** | 10-12/12 | `NO_COMPLETION_CHANGE` ×9 |
| IID-20% | 3 | **0/12 every cell** | 9 / 5 / 9 of 12 | 9 / 5 / 9 | `FUSION_PAYS_AT_A_COST` ×3 |

Totals over all paired repetitions: `fusion_only = 23 · tail_only = 0 · both = 108 · neither = 13`.

## 2. The answer to the question

> Does proactive redundancy still pay once ARQ can already recover its own tail?

**On tail loss: no. On IID loss: yes.** The two mechanisms are not competitors — they answer
different loss structures, and each is nearly useless where the other works.

**Tail strata — coding buys nothing.** Tail-capable ARQ already completes 12/12 in all nine cells.
Adding r=2 keeps it at 12/12 and costs roughly +1111 bytes per run (4949 → 6060 at d1). This is the
preregistered reading that *the historical FEC-versus-ARQ tail advantage was ARQ's tail blindness,
not coding* — R2A C2 reached it by deconfounding, and this reaches it directly by giving ARQ the
tail probe and watching coding's advantage disappear.

**IID strata — coding is the whole mechanism.** Tail-capable ARQ completes **0/12** in all three
IID cells; fusion completes 9, 5 and 9. A tail probe cannot help when loss is spread through the
body rather than concentrated at the end, and the bounded probe has nothing to aim at.

## 3. Fusion is never harmful

`tail_only = 0` in every cell: there is no repetition anywhere in the grid where tail-only completed
and fusion did not. Adding redundancy costs bytes, never completions. That is the same direction
FS-FUSION-CAL-1's corrigendum found for r0→r2 over frozen ARQ (`base_only = 0` in 21/21 cells), now
confirmed on the tail-capable arm.

## 4. Fusion versus F0 — the reason the cell mattered

F0 is frozen ARQ plus the same coding. On IID the two are **identical** (9/5/9 both). On tail cells
F0 is **worse**: 10/12, 10/12, 10/12 at d4 and 11/12 at rtt1000-d2, against fusion's 12/12
everywhere.

So in completion terms fusion weakly dominates F0 across this grid — it matches coding wherever
coding is what matters, and keeps the tail recovery F0 does not have. That comparison was
unavailable until now for exactly the reason the corrigendum recorded: the arm could not be
expressed.

## 5. What this does NOT establish

* **No promotion, no default flip.** Screening. Product tail recovery ships default-OFF and this
  changes nothing about that.
* **No statistical claim.** Repetitions are the generator's maximum, not a power calculation. No
  p-values, no confidence intervals.
* **No deadline claim.** R2A already recorded that no arm meets the frozen tight deadlines; this
  campaign did not revisit it.
* **Not the product.** `run_spine` is an in-process deterministic model, not netem, and not the
  shipped `Sender` path.
* **r = 2 only.** r=4 was not run, so nothing here says where the redundancy optimum sits.

## 6. Execution integrity

* The decision rule in §4 of the preregistration was fixed before any number existed, and the
  verdicts above are that rule applied mechanically.
* All three arms share cell, repetition and seed; the pairing is exact.
* No seed was searched, skipped or replaced, and no cell was dropped.
* A first attempt at N=30 panicked in the erasure generator, which carves a disjoint window per
  repetition out of 48 source symbols and therefore admits at most `48/d` reps. That was a
  feasibility limit discovered at runtime, not an outcome, and the fix derives the count from the
  generator instead of guessing it. The single cell observed before the panic (rtt50-exactd1,
  `NO_COMPLETION_CHANGE`) is at d=1, where 30 reps were feasible, so the limit never touched it —
  and it reports the same verdict in the completed run.
* The runner is committed together with its results.
