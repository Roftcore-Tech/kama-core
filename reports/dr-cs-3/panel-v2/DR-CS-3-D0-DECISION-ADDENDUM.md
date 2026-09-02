# DR-CS-3 — D0 DECISION ADDENDUM (TASK-D0A)

Decision freeze only, written outcome-blind. **No code is repaired here, no neutrality field is
defined here, and no run of any kind was performed.** No result-bearing selection or paired-panel
record was opened.

    branch : transport-baseline-closure
    HEAD   : 2909ff86
    plan   : specs/Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md
             sha256 4bda43a82c89d01ded0aaac1b4cce9b36784dca76554746abb3bf4980292e95a
             1444 lines, 2026-08-15 rev.2
    inputs : DR-CS-3-D0-SOURCE-AUDIT.md, DR-CS-3-M2-EXECUTION-ADDENDUM.md (both tracked)
    WIP    : six M5 files verified unchanged against the A0R2 snapshot, before and after

---

## 1. D0 status

    TASK-D0: BLOCKED-LOCKED-PANEL-DECLARATION

Because the historical 18-field neutrality set is absent from tracked source, the lost `/tmp`
comparator cannot be recovered, the recorded output unit does not arithmetically identify 18
semantic fields, and locked-panel expectations cannot be frozen against an unknown field set.

**D0 successfully closed 9 of 10 source audits. This BLOCKED refers to a missing neutrality
contract — not to a product defect and not to a cold-start efficacy failure.**

## 2. Historical 32×18 — reclassified

    BANKED 32x18 NEUTRALITY:
      HISTORICAL-ONLY
      FIELD SET UNRECOVERABLE
      COMPARATOR UNRECOVERABLE
      PROFILE UNKNOWN / AMBIGUOUS
      NOT REPRODUCIBLE
      NOT CITED AS EVIDENCE
      NOT A PASS GATE FOR THE CURRENT HEAD

Binding constraints:

* **Do not** attempt to remember or infer the lost 18 fields. This addendum deliberately contains no
  candidate list.
* **Do not** infer them from the count of stdout tokens — the recorded unit prints ~22 values per
  line, so token arithmetic cannot recover a 18-field semantic set.
* **Do not** call any future field set "the original 18".
* The historical run stays in the chronology but carries **no current neutrality evidentiary
  weight**.
* Original banked panel evidence remains evidence **at its own historical commit**; continuity from
  that commit to the current instrumented tree is **not proven**.

## 3. New V1 definition

**V1 remains a mandatory efficacy gate.** It is no longer defined as a rerun of the unrecoverable
18-field comparator. It will use a new versioned contract:

    NEUTRALITY-SCHEMA-V2

The schema must be derived from **tracked source, not historical output data**; frozen before any
neutrality run; tied to **one** explicitly selected structured record surface; semantic rather than
stdout-position-based; typed; keyed by an explicit record identity; complete about missing and extra
records; explicit about diagnostic exclusions; identical for baseline and candidate; executed under
`--release` with the candidate logger **OFF**, default features, `experimental-cold-start=false`.

**No cardinality is selected in advance.** The result may be called "32×18" only if its frozen
cardinality independently turns out to be 32 records × 18 fields.

Failure remains:

    OBSERVATION-NEUTRALITY-FAILURE — EFFICACY PATH BLOCKED

## 4. Neutrality recovery chain — frozen

**TASK-N0 — source-derived neutrality contract.** Inspect tracked record/output surfaces; choose
exactly one authoritative structured surface; enumerate every candidate field and its type; define
the semantic record key and the expected-cardinality rule; classify every field as
`REQUIRED-LOCKED` / `DIAGNOSTIC-EXCLUDED` / `TREATMENT-AXIS-EXCLUDED` / `INVALID/DEFERRED` with
source lineage and rationale for each; freeze the exact comparison semantics; freeze the
locked-panel declaration. No cargo, no result-bearing data.

**TASK-N1 — tracked comparator and fixtures.** Implement the comparator from
`NEUTRALITY-SCHEMA-V2`; **no positional stdout parsing**; provide a positive fixture and five
negative fixtures — mismatch, missing-record, extra-record, type/parse-error, and
excluded-field-difference. **Each negative fixture must turn RED through its own intended guard.**
Two deterministic runs must produce identical artifact hashes. No real neutrality data.

**TASK-N2 — comparator review and freeze.** Independent source/schema/fixture review; prove the
comparator consumes **every** `REQUIRED-LOCKED` field; prove excluded fields **cannot** affect PASS;
freeze the comparator hash, the fixture hashes and the exact V1 command; then stop for separate V1
authorisation.

    TASK-D0A → TASK-N0 → TASK-N1 → TASK-N2 → TASK-D1 / other authorised implementation → TASK-V1

**No neutrality run is authorised by D0A.**

## 5. `first_probe_tick` erratum

**`PanelOut::first_probe_tick` is never initialised on the audited path.**

Consequences:

* `scheduler_data_displacement` on the `PanelOut` path is **hard-wired to zero**;
* historical zero values from that path are **vacuous**;
* they are **withdrawn as empirical Gate-5 evidence**;
* they must not appear in any promotion, safety or product-finding claim;
* the metric is classified **`INVALID/DEFERRED`** in `NEUTRALITY-SCHEMA-V2`.

**Scope boundary — deliberately narrow.** This defect affects the `PanelOut`
`scheduler_data_displacement` path **only**. It does **not** automatically invalidate the separate
structural-oracle `direct_displacement_count` implementation, whose synthetic positive control
produced a **1** on a different path. Therefore:

    structural Gate-5 source/oracle claim : remains recorded at its own commit
    PanelOut empirical confirmation       : WITHDRAWN

**Positive-control lesson.** Any future `PanelOut` displacement metric requires an independent
positive fixture capable of producing a non-zero value. **A counter that cannot be forced non-zero
cannot support an invariant claim** — that is exactly how a hard-wired zero survived as "evidence".

Disposition:

    first_probe_tick / scheduler_data_displacement repair:
      DEFERRED WITH CHANNEL-FATE DIAGNOSTICS

D0's full membership scan found no consumer in selection, qualification, efficacy, promotion or the
Gate-5 verdict, so deferral is safe. **No cold-start efficacy claim may cite it while deferred.**

Retaining the metric later requires, in order: initialise `first_probe_tick` from a source-grounded
event; add a non-zero positive control; prove the counter is not tautologically zero; rerun
diagnostic neutrality.

## 6. Metric-name collision erratum

* `scheduler_data_displacement` still cites a **withdrawn** metric contract.
* The JSON field `channel_fate_divergence` collides semantically with the **superseded**
  emission-tick metric of the same name.
* **Neither name nor value may be used as current channel-fate evidence.** Both are
  diagnostic-deferred.
* Future repair must use unambiguous **versioned** field names under a frozen metric contract.

No field is renamed in D0A.

## 7. Channel-fate / cap verdicts

    A / B / C : DEFERRED — DIAGNOSTIC PATH
    D / E     : DISSOLVED-BY-CORRECTED-PREMISE
    EVENT CAP : UNBOUNDED-BY-PRODUCT
    CONTROL 13: CONTROL-13-UNCONSTRUCTIBLE-BY-CONFIG

Binding:

* no tick-independent finite product event cap is claimed;
* an observed maximum is **not** a bound;
* **no numeric harness ceiling is selected here**;
* cap overflow can **never** be labelled a `PRODUCT FINDING`, a Gate-5 failure, or a cold-start
  failure;
* `control_13` is **not** marked PASS;
* channel-fate attribution remains **`DEFERRED — NOT CLOSED`**;
* **M2 channel-fate PASS must not be reported.**

## 8. Efficacy-path separation

D0's complete consumer scan found that **no deferred channel-fate or event-cap metric reaches**
selection, seed qualification, the OFF phenotype gate, the ON efficacy verdict, a promotion
threshold, or the Gate-5 safety verdict.

    CHANNEL-FATE ATTRIBUTION : DEFERRED
    COLD-START EFFICACY PATH : not invalidated by the deferred diagnostic defects,
                               but still blocked by mandatory V1 and unfinished execution cards

**Cold-start efficacy is not proven and is not claimed here.**

## 9. Logger-OFF and V1

    V1-REMAINS-EFFICACY-GATE

Because logger enablement is a **runtime** switch; the logger-OFF path is **not inert**; Class-4
(treatment-dependent behaviour with the logger OFF) is **non-empty**; five of nine reclassification
prerequisites failed or were undecidable; and same-binary reasoning is insufficient.

**State-neutrality observed by source audit does not replace empirical release / logger-OFF
neutrality.**

## 10. Pairing and the prefix limit

    DESIGN-A-FEASIBLE-MEDIUM

Preferred execution remains **all OFF → global fingerprint/probe/phenotype gate → all ON**. The
per-repetition OFF→ON fallback remains **unauthorised**.

**Explicitly recorded:** the authoritative logger-OFF path does **not** provide the deferred
channel-event prefix assertion, so **no empirical pre-treatment-prefix equality claim will be made**.
Paired fairness instead rests on the preregistered set: same frozen binary; same feature/config; same
seed; same invocation; only treatment differs; a global OFF phenotype/fingerprint/probe gate; and
deterministic repetition-index ordering. **Prefix coverage is not silently claimed.**

## 11. Gate verdict

    GATE: GATE-SELF-DIRTY

Additional findings: CWD-sensitive; untracked-file handling not explicitly pinned; the writer of the
existing `.gate-target/debug` tree cannot be uniquely attributed.

**TASK-G0 — gate hardening (frozen, not chosen or implemented here).** Required scope: make
target-directory handling explicit and outside the checked worktree, or adopt another source-grounded
non-self-dirty design; pin untracked-file semantics; remove CWD ambiguity; preserve **every** existing
gate check; supply an expected-RED proof before the fix and an expected-GREEN proof after; shipping
source changes = **0**.

**TASK-G0 must PASS before the full V4 gate.**

## 12. Current M2 status

    STRUCTURAL ORACLE              : CLOSED at its committed source/oracle scope
    PANEL-OUT DISPLACEMENT CONFIRM : WITHDRAWN — HARD-WIRED-ZERO DEFECT
    BANKED 32x18 NEUTRALITY        : HISTORICAL / UNREPRODUCIBLE / NOT EVIDENCE
    CHANNEL-FATE ATTRIBUTION       : DEFERRED — NOT CLOSED
    COLD-START EFFICACY            : NOT YET RUN — VALIDATION IN PROGRESS
    FULL LOCAL GATE                : NOT GREEN — GATE-SELF-DIRTY plus pending format/validation work

None of the following may be written: *M2 complete*, *channel-fate PASS*, *neutrality PASS*,
*cold-start solved*, *product finding*.

## 13. Updated critical path

    D0A → N0 → N1 → N2
        → D1/D2 as separately authorised
        → G0 before V4
        → V1 / V2 / V3 / V4
        → V5
        → S0 / S1 / S2
        → paired panel
        → final cold-start verdict

Diagnostic channel-fate repair stays **outside** the efficacy critical path unless a later membership
scan proves otherwise.

## 14. Superseded predecessors — recorded, not silently dropped

Per the project rule that deprecated content is marked rather than deleted:

| item | origin | status |
|---|---|---|
| `TASK-D0B — reconstruct the frozen 18-field list` | D0 audit §13 item 1 | **SUPERSEDED — MUST NOT IMPLEMENT.** §2 forbids reconstruction; N0 derives a new schema from tracked source instead |
| `TASK-D1x — fix first_probe_tick` | D0 audit §13 item 4 | **SUPERSEDED by §5** — folded into `DEFERRED WITH CHANNEL-FATE DIAGNOSTICS` with a four-step retain condition |
| "the frozen 18-field comparison list" fixture requirement | M2 execution addendum §12 | **UNSATISFIABLE — SUPERSEDED by TASK-N1's six fixtures.** The list it names is unrecoverable |
| banked 32×18 as "historical **supporting** evidence" | M2 execution addendum §9 | **TIGHTENED by §2** to `NOT CITED AS EVIDENCE`. This is a deliberate downgrade of a committed label, in the conservative direction, recorded here rather than applied silently |

## 15. Defect signature for TASK-N0

Recorded so N0 inherits the exact shape, not a paraphrase:

* `dr_cs3_panel.rs:2097-2100` builds `PanelOut { epoch_trigger: "NONE", first_echo_tick: u64::MAX,
  ..PanelOut::default() }`. The **sibling** sentinel `first_echo_tick` is initialised;
  `first_probe_tick` falls through to `Default` = `0`. The asymmetry is the defect signature.
* The only writer, `:2266`, is `if out.first_probe_tick == u64::MAX { ... }` — it can never fire.
* `:2642` then reads `let horizon = treatment.first_probe_tick;` and filters, yielding a constant `0`.
* Consequence beyond §5: `channel_fate_divergence` (`:2663-2665`) reads the same field behind an
  `if from == u64::MAX { return 0 }` guard that likewise never fires — so that metric is **not**
  zero-wired but silently **mis-scoped to the whole trace**. §6's diagnostic deferral already covers
  it; the mechanism is recorded here because it differs from §5's.
