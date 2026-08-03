# DR-ARQ-TAIL-1-R2A-ENDPOINT-AND-BUDGET-CORRIGENDUM (additive)

> Additive corrigendum to the LOCKED R2A artifacts (final report + CELL-RESULTS.csv + PAIRWISE.csv are NOT modified;
> the dataset is NOT re-run). All corrections below are recomputed from the frozen 8400-row dataset (deadline_hit and
> realized-wire were recorded per row and are mechanically re-reportable). The dataset is NOT invalid.

## 1. Completion endpoint — REPORTING/ENDPOINT CORRIGENDUM
The CSV/report field `complete` is **completion within the observation horizon** (699/2499/9249 ms), which is NOT the
same as the preregistered **deadline hit** (199/599/2099 ms). The deadline endpoint was under-surfaced in the R2A final
report. Recomputed (of 2100 rows/arm):
```text
                 deadline_hit        horizon_completion
A0-FROZEN        0 / 2100            951 / 2100
A0-TAIL          46 / 2100           1814 / 2100
A1-EARLY         0 / 2100            968 / 2100
F0               0 / 2100            1836 / 2100
```
**Correction:** every C1/C2/C3 "completion" statement in the R2A report must be read as **"completion within the
observation horizon"**, NOT deadline-performance or real-time success. On the deadline endpoint, ALL arms are ~0 except
A0-TAIL (46/2100) — i.e., under the frozen tight deadlines NONE of these arms is a real-time success, and the FEC-vs-ARQ
and coding-vs-duplication contrasts are about horizon recovery, not deadline attainment. Deadline and observation horizon
are reported as SEPARATE endpoints henceforth. Dataset valid; `deadline_hit` is present per row.

## 2. C3 budget wording — CORRECTION
The R2A report's "matched realized-wire budget 21/21 (NON_DOMINATED)" claim is **RETRACTED**. Facts:
```text
configured intervention budget : F0 = 1212 B, A1-EARLY = 1212 B  (equal by construction)
realized total-wire relation   : equal 3/21 · F0 cheaper 18/21 · F0 more expensive 0/21
```
`coding_causal_fairness` compares CONFIGURED intervention parity (1212==1212 → NON_DOMINATED) — that is a configured-budget
statement, not a realized-wire-equality statement. **Correct statement:** "At equal configured intervention budget (1212 B),
F0 produced 868 paired F0-only horizon completions against 0 A1-only completions; F0 was equal-or-lower in realized total
wire in all 21 cells (equal 3, cheaper 18, never more expensive)." (This strengthens F0-vs-A1: F0 completes far more AND is
never costlier — but the causal claim rests on configured-budget parity, with realized wire reported alongside, not assumed equal.)

## 3. S3 conditional-latency limitation
In S3 (NaturalIid) F0's completion-latency medians appear lower, but latency is computed over COMPLETED rows only and the
completion populations differ across arms (F0 1836 vs A0-TAIL 1814 vs A1 968 vs A0-FROZEN 951 horizon completions). A lower
conditional median over a different (and generally larger) completed subset is NOT, by itself, evidence of unconditional
latency superiority. No unconditional latency-superiority claim is made from R2A.

## 4. Controller / loss-confound limitation
R2A does NOT separate loss STRUCTURE from loss INTENSITY:
```text
S1/S2 : deterministic/positional erasure, ~2–8% of symbols  (drop_count {1,2,4} of 48)
S3    : IID, 20% per symbol
```
Therefore a production FEC-activation threshold **cannot** be derived from `loss_ppm` alone from R2A — loss rate and loss
structure are confounded. A FEC policy ("open FEC if loss_ppm high") is NOT written now; it awaits a separate DR-FUSION-POLICY-1
grid crossing loss-rate × loss-structure × RTT (see the product-ARQ-baseline plan). This is the binding reason §7 defers FEC policy.

## Status
Additive corrigendum only. R2A dataset + structured artifacts remain LOCKED and unmodified. The scientific bottom line is
unchanged in direction (C1 tail recovery real; C2 F0's tail edge largely ARQ tail-blindness; C3 coding>duplication confirmed)
but every completion claim is now correctly scoped to the observation-horizon endpoint, and the C3 budget wording is corrected.
