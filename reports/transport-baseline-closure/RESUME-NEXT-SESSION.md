# RESUME POINT — M2, channel event log

## State at stop

HEAD `08fe39a0`, branch `transport-baseline-closure`, working tree CLEAN.

Committed this session, in order:

    8d0c2fee  three-measurement-point instrument rule
    74a6736c  structural oracle CLOSED
    4f11545c  parity-pending vs parity-actuated split + selection build-axis addendum
    7e1af66b  live fmt gate in scripts/gate.sh (contract is now 13 checks, not 12)
    d510070d  channel-fate interpretation addendum (prereg, outcome-blind)
    08fe39a0  channel event log: DATA identity at the GE decision + directional fate

Persistent backup of the panel file: `~/Desktop/dr_cs3_panel.rs.backup`
(the `/tmp` scratchpad does NOT survive a reboot).

## What is DONE

* structural oracle CLOSED; Gate-5 denominator {cell_3, cell_7}, displacement 0
* channel event log implemented: identity `(direction, unit_id)`, matched key
  `(direction, unit_id, attempt_ordinal)`, fate read from the GE branch, six metric
  sets kept separate, per-direction boundaries, 13 controls
* `cargo test -p kama-integration --test dr_cs3_v2_channel` -> 45 passed, 0 failed

## What is OUTSTANDING (do these first, next session)

1. frozen 32x18 neutrality — locked panels, primary + bidirectional, OFF and ON,
   baseline `d510070d` vs instrumented `08fe39a0`.
   Entry point: `cargo run -p kama-integration --example dr_cs3_panel_runner -- locked --reverse none`
   and `... --reverse ge`. Required: 32 records, 18 fields, 0 mismatches, 0 missing.
   Get the baseline with `git show d510070d:<path>` / `git archive` into a scratch dir —
   NEVER `git checkout` or `git stash` in the repo tree.
2. the twelve mutation proofs, out of tree
3. per-config event cap table + the five-feature matrix
4. then: separate format-only cleanup commit -> 32x18 again -> oracle regression ->
   full 13-check gate -> instrumented HEAD freeze -> section 12.6 budget ->
   512 OFF-only selection

## Two things that cost time — plan around them

* A single `dr_cs3_v2_channel` run takes **~12 minutes**. Verification work must be
  sized against that; a agent given dozens of runs will exhaust itself before finishing.
* Untracked USER WIP (`crates/adaptive-transport/src/fate_estimator.rs`,
  `crates/kama-integration/tests/common/fs_cal2_{spine,schema}.rs`,
  `crates/kama-integration/tests/fs_cal2_pins.rs`) breaks `--all-targets` builds of
  kama-integration. Do NOT fix or delete it. Work around it with specific `--test` targets.

## Standing status

    STRUCTURAL/EXPERIMENT COMPONENTS GREEN
    FULL LOCAL GATE RED — FORMAT CHECK        (POST-CI-FORMAT-DRIFT, 146 hunks)
    READY-FOR-SEPARATE-FORMAT-CLEANUP

No selection, panel, seed search or M3 has been started. Shipping source diff: 0.
