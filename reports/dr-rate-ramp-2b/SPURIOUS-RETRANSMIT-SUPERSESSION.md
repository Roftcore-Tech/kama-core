# SUPERSESSION — startup pacing removes the D2/R2 spurious-retransmit leak

Written and hashed **BEFORE** any test edit.

## Measured

`kama-integration/tests/fs_fec_arms.rs::f0_cost_split_intervention_vs_total_outcome`, toggling only
the startup-pacing axis in `run_f0`'s sender (instrumentation since removed; `fs_f0.rs` restored
from backup, `git diff` empty):

| startup pacing | `spurious_retransmits` | test |
|---|---|---|
| **OFF** | `> 0` | passes |
| **ON** | `0` | fails on `assert!(r.spurious_retransmits > 0)` |

## What the old assertion documents

The test is a **§8 COST-SPLIT proof**: it separates INTERVENTION-BUDGET (C-P4, proactive parity
sealed wire) from TOTAL-OUTCOME-COST (`arq_retransmit_bytes + spurious_retransmits`). Its
`spurious_retransmits > 0` line exists to *demonstrate the leak channel is real* — F0 D2/R2 completes
the FEC block early, then late-arriving post-DATA parity leaks a spurious ARQ retransmit.

It asserts the **presence of a cost**, not a safety property.

## Why this is a supersession and not a regression

With the floor active the leak does not occur: the block completes without the late-parity race that
produced the spurious retransmit. Fewer wasted retransmissions is **strictly better** — lower
total-outcome cost, unchanged intervention budget.

The other assertions in the test are unaffected and must keep holding exactly:
* `fec_decoded_sources == 2`
* `realized_sealed_wire_bytes == 2 × fs_sealed_len(64)`
* `arq_retransmit_bytes == retransmits × sealed` (accounting identity)

## Discipline

1. This artifact is hashed before the test changes.
2. The original test is **preserved verbatim** — every assertion, including
   `spurious_retransmits > 0` — and re-scoped to the startup-pacing-**INACTIVE** path it was measured
   on. Nothing is weakened, deleted, or turned into `>=`.
3. The new behaviour is sealed by a companion test under the **product default** asserting
   `spurious_retransmits == 0` together with the unchanged accounting identity and budget, so the
   cost-split proof still has both halves.

## Result

`STARTUP-PACING-REMOVES-D2R2-SPURIOUS-LEAK · COST-SPLIT-PROOF-PRESERVED-ON-BOTH-AXES`
