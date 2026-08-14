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

---

# UPDATE — verification results (session 2)

HEAD `01555ee0`. Commits added: `08fe39a0` channel event log · `cb1035e0` resume point ·
`01555ee0` addendum erratum 1.

## 32x18 neutrality — BIDIRECTIONAL HALF DONE, PRIMARY HALF STILL RUNNING

Method: 4 locked panels run in PARALLEL from prebuilt binaries (builds sequential, -j2).
Baseline = `git worktree` at `d510070d` (839-line pre-instrumentation panel);
instrumented = working tree at `08fe39a0` (3684 lines).
Comparator: `scratchpad/neut_compare.py` — new event-log fields excluded by a FIXED NAME LIST,
never "whatever differs", so a real behavioural change cannot be silently absorbed.

Measured, not assumed: each run emits **16** records (8 reps x 2 tail configs
P0-tail-OFF / P1-tail-ON). Per tree primary 16 + bidirectional 16 = **32** — the 12.5 count.
One printed line == one record; `base[...]`/`treat[...]` are the OFF/ON arms WITHIN a record and
contribute their own fields to its 18. Splitting them would give 64 and inflate the denominator.

    BIDIRECTIONAL HALF:  records 16 | fields per record 18 (uniform) | mismatches 0 | missing 0

## Mutation proofs — 10/12 falsifiable, 2 BLOCKING defects

Repo worktree UNCHANGED in all four groups (`git diff --quiet` -> 0). Each mutation ran in the
agent's own scratchpad crate, narrowed with `-- --exact <test>` (seconds, not the ~12 min a full
run costs).

BLOCKING-A  EVENT-CAP-CONFIG-MISMATCH never fires in the LOOSE direction. Using the
  experimental-cold-start cap in the default build returned PASS in all four configurations; only
  the tightening direction fires. The dangerous direction is the unguarded one -- an over-large cap
  silently legitimises an overflow.

BLOCKING-B  control_13 is vacuous for Echo and for generic events. A fabricated extra event stayed
  GREEN unless it was a Probe. Diagnostic: echoes_generated=0, echo_events=0, probes_emitted=0,
  probe_events=0 in the tested configs. Independently found by the read-only audit as well
  ("structurally vacuous for BOTH reverse-direction logging sites") -- two methods, same defect.

Scope note (not blocking): the one-sided-trajectory criterion is meaningful only POST-boundary. A
pre-boundary identity change voids the pairing via PRE-TREATMENT-PREFIX-MISMATCH and dies in a
helper `expect`, not in the criterion.

## Read-only audit — 3 further BLOCKING defects in the event cap

1. F4 bounds duplicates with DEFAULT_DUP_FACTOR (2), but that is the FALLBACK; `dup_params()` can
   return another factor, so the bound can be wrong.
2. The second RecoverFwd duplicate site (sender.rs:4082-4094) emits `factor` copies, not
   `factor - 1`; F4's lineage cites it but no cap term corresponds to it.
3. `plan_forward_duplication`'s pure-silence tail-liveness branch (sender.rs:4062-4081) is a real
   forward producer with NO row and NO bound at all.
Plus MAJOR: per-producer bounds are never enforced (only the three sums are compared), so any row
can overshoot by the slack of the others.

## Also outstanding

* the SUPERSEDED metric is still named `channel_fate_divergence` and still computes an
  emission-tick symmetric difference; addendum section 6 requires it labelled
  SUPERSEDED - NOT CHANNEL FATE.
* `compare_channel_fates` records arm / chain / seed_domain / cell_id but never checks them, so it
  will happily compare an inadmissible pair.

## M2 STATUS: RED — 5 blocking items.
