# DR-CS-3 — TASK-D0 READ-ONLY SOURCE AUDIT

Outcome-blind source audit. Nothing was compiled, tested, gated or run; no selection or
paired-panel outcome record was opened. Every claim below is labelled **OBSERVED SOURCE FACT**,
**INFERENCE**, **RECOMMENDATION** or **UNRESOLVED AMBIGUITY**.

## 1. Source state and artifact hashes

    branch                : transport-baseline-closure
    HEAD                  : 88da9df5
    tracked modified      : 0
    plan (tracked)        : specs/Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md
      sha256              : 4bda43a82c89d01ded0aaac1b4cce9b36784dca76554746abb3bf4980292e95a
      lines / version     : 1444 / 2026-08-15 rev.2
    addendum (tracked)    : reports/dr-cs-3/panel-v2/DR-CS-3-M2-EXECUTION-ADDENDUM.md
      sha256              : f207246668eb4c13f6d9d60db30bc1eb67d3c997dcac40c22f30b956a511497a

Six M5 WIP hashes verified against the committed A0R2 addendum before and after this audit — all
six unchanged. Their contents were not opened.

## 2. `.gate-target` — verdict `GATE-SELF-DIRTY`

**OBSERVED SOURCE FACT.** `scripts/gate.sh:29` defaults
`CARGO_TARGET_DIR="${1:-${CARGO_TARGET_DIR:-$PWD/.gate-target}}"` — an **in-tree** path. gate.sh
sources no helper (full 93-line read: cargo, git, grep, paste, sed, python3, wc, printf, echo only),
so line 29 is the sole target-dir authority.

The tracked `.gitignore` holds exactly five patterns — `/target`, `**/*.rs.bk`, `Cargo.lock.orig`,
`/results`, `scratchbin/` — none matching `.gate-target` (`/target` is root-anchored and a different
name). `.git/info/exclude` is comments only and `core.excludesFile` is unset, so no local layer
covers it either.

`gate.sh:86` runs `git status --porcelain` with no `--untracked-files` pin, so untracked paths are
visible by default; `:87-88` fail on any non-empty result.

**INFERENCE (deterministic).** Step 1 (`:66`, `cargo build --workspace --all-targets --all-features`)
materialises the directory and step 8 then observes it **within the same invocation**. A fresh
worktree reproduces this from tracked rules alone: it materialises only tracked content, including
the `.gitignore` that lacks the rule, so `.gate-target/` becomes the **only** untracked path and the
gate goes RED purely on its own artifact — in a script whose introducing commit `3e6dfa30` is titled
*"stop tests writing into the repo tree"*.

**Provenance of the directory now on disk: MIXED / UNPROVABLE (conservative).** Birth
2026-08-12 20:02:16 is 10 s after `3e6dfa30`, so it is **not** pre-gate.sh residue; but
`.gate-target/release/` exists while `grep -n release scripts/gate.sh` returns nothing — gate.sh has
zero `--release` invocations — proving at least one writer was **not** gate.sh.

**Two alternatives, proposed and NOT implemented (D0 chooses neither):**

| | ALT-1 external `CARGO_TARGET_DIR` | ALT-2 tracked ignore rule |
|---|---|---|
| file changed | none — already supported | `.gitignore` |
| gate semantics | unchanged | step 8 stops seeing this artifact |
| shipping code | untouched | untouched |
| proof | run with `$1` or the env var; step 8 sees no new path | `git check-ignore` matches; step 8 clean |

**OBSERVED SOURCE FACT.** ALT-1 needs no edit at all: `gate.sh:29` already honours a positional `$1`
(documented at `:7`) and a pre-set env var. This is how `EXECUTION-STATE.md:410` could record
`12/12 PASS ... worktree clean` — those runs used `.nb-*` target dirs.

**UNRESOLVED AMBIGUITY.** gate.sh never `cd`s to the repo root and never resolves
`git rev-parse --show-toplevel`, so `$PWD/.gate-target` lands wherever it is invoked from; and the
unpinned `--untracked-files` leaves step 8's sensitivity dependent on machine-local git config.

## 3. Current runner topology

**OBSERVED SOURCE FACT — A0's citation is VERIFIED EXACT**, independently re-derived (A0 is known to
have mis-cited a line elsewhere, so this was re-measured rather than copied):
`run_pair` at `dr_cs3_panel_runner.rs:202-207` runs OFF (`:204`) then ON (`:205`) in one call, and is
invoked at `:265` (locked), `:292` (fresh), `:322`, `:345`, `:387` (controls).

**OBSERVED SOURCE FACT — the nesting is deeper than "per-repetition".** `locked` (`:260`) and `fresh`
(`:287`) nest the repetition loop inside a `for (panel, tail_on)` loop; `control` adds a `for rtt`
loop (`:314`). So an ON arm precedes not only the next repetition's OFF arm but the next *panel's*.

**OBSERVED SOURCE FACT — the runner contains ZERO assertions.** No `assert`/`panic!` anywhere; the
only failure exits are `process::exit(2)` on a bad `--reverse`. Verification is therefore, by
construction, an external post-hoc step.

**OBSERVED SOURCE FACT — fingerprint/probe/phenotype assertions are ABSENT from the runner.** They
exist only in `cfg(test)` targets; `feature_fingerprint()` and `legacy_ladder_compiled()`
(`dr_cs3_panel.rs:909-950`) are never called from the runner.

**OBSERVED SOURCE FACT — the logger is runtime-OFF on every arm of every mode.**

## 4. Design A vs Design B

**DESIGN A** (all paired OFF → one global gate → all paired ON): **`DESIGN-A-FEASIBLE-MEDIUM`**.
No architectural blocker. Requires a new bounded buffer for OFF records keyed by
repetition/seed; the output schema **can stay unchanged**; locked-panel and selection behaviour stay
unchanged **only in the new-mode variant**; and **no ON record can reach disk before the gate**.

**DESIGN B** (per-repetition OFF→ON with retrospective invalidation): mechanically smaller, but
**leaks ON records to disk before the gate completes**. It remains **NOT AUTHORISED**, and no claim
of scientific equivalence between A and B is made here.

**UNRESOLVED AMBIGUITY.** Whether D1 implements Design A as a new additive mode or by restructuring
the five existing call sites is a scope decision no tracked source fixes. The *content* of the
"global fingerprint/probe/phenotype gate" is likewise not frozen in tracked source — only the
phenotype half is grounded.

## 5. Locked-panel declaration — `BLOCKED-LOCKED-PANEL-DECLARATION`

**The formal declaration cannot be frozen, because the thing it must enumerate does not exist in the
repository.**

**OBSERVED SOURCE FACT.** The card requires the declaration to cover *"all frozen 18 neutrality
fields"*. That list exists **nowhere** in tracked source, in the rev.2 plan, or in any committed
addendum. `DR-CS-3-NEUTRALITY-32x18-RESULT.txt:18` names `scratchpad/neut_compare.py` — a path under
`/tmp` that no longer exists. The comparator is gone **and the field list was never written down**.

**OBSERVED SOURCE FACT.** The recorded comparison unit does not arithmetically yield 18:
`RESUME-NEXT-SESSION.md:73-76` treats one printed stdout line as one record, but the runner prints
~22 values per line. The stated fixed-name exclusion mechanism has no referent in that output.

**Conditional substantive declaration — recorded, but NOT a freeze.** If and only if the 18 fields
are later shown to be a subset of the runner's existing observable surface, the correct declaration
is:

    COMMIT-D LOCKED-PANEL BEHAVIOUR:
      MUST REMAIN BYTE/SEMANTICALLY UNCHANGED
      implementation is additive/reporting-only
      expected frozen-field mismatch count = 0

**OBSERVED SOURCE FACT supporting it:** `git diff --stat d510070d 08fe39a0` is one file
(`dr_cs3_panel.rs`, +2863/-18) — the change surface is closed and small.

This is **INFERENCE conditional on an unresolved premise**, so it is **not** the frozen declaration
the card demands. Per the card: **do not run neutrality.**

## 6. Logger-OFF call graph

**OBSERVED SOURCE FACT.** Logger enablement is **runtime**, not compile-time.

* **Class (1) present-but-never-executed:** enumerated.
* **Class (2) executed WITH logger OFF:** **16 distinct added executions**, all provably state-neutral.
* **Class (3) ON-only:** enumerated, including three chain reads that are ON-only by placement.
* **Class (4) treatment-dependent even with logger OFF:** **NON-EMPTY.** Every member is
  state-neutral — no treatment-dependent code added by `08fe39a0` mutates any state the logger-OFF
  run reads.

**OBSERVED SOURCE FACT — the OFF path is not inert.** `dr_cs3_panel.rs:2601` writes
`out.shipping_probes_emitted = d.probe_emitted` **unconditionally**, outside all five
`if let Some(l) = log.as_mut()` guards.

**OBSERVED SOURCE FACT.** Event caps have **no OFF-path surface**; the ON-path cap is report-only and
never enforced.

Same-binary reasoning is **not** treated as neutrality proof anywhere in this audit.

## 7. V1 role — `V1-REMAINS-EFFICACY-GATE`

Nine prerequisites for reclassification; **three PASS, one PARTIAL, five FAIL or undecidable**:

| # | prerequisite | result |
|---|---|---|
| 1 | logger-OFF performs no semantic mutation | **FAIL** (`:2601` unconditional write) |
| 2 | no ordinal changes under logger OFF | PASS |
| 3 | no cap/overflow branch reachable under logger OFF | PASS |
| 4 | no treatment-dependent observation work under logger OFF | **FAIL** (Class 4 non-empty) |
| 5 | selection + paired panel both rerun on the final tree | UNPROVABLE → conservative **FAIL** |
| 6 | both use the same frozen V5 release binary | UNPROVABLE → conservative **FAIL** |
| 7 | both use logger OFF | PASS |
| 8 | no banked panel record consumed by the new verdict | **FAIL** (self-defeating under reclassification) |
| 9 | V2/V3 cover shipping/oracle/control regressions | **PARTIAL** — not V1's cross-tree continuity axis |

**RECOMMENDATION: `V1-REMAINS-EFFICACY-GATE`.** The conservative verdict is forced several times
over. **D0 does not waive V1, does not modify the plan or the addendum, and any reclassification
requires a separately user-authorised TASK-D0A preregistered before any result-bearing run.**

## 8. Five-defect disposition

**OBSERVED SOURCE FACT.** No source file changed between the ledger commit and HEAD
(`git diff --name-only 01555ee0..HEAD` returns only report/spec paths), so every lineage below is
current. The ledger resolves **uniquely** onto the plan's A–E lettering.

**DECISIVE FOR ALL THREE OF AUDITS 6-8:** `cap_report`, `CapVerdict`, `EventCapPlan` and the
channel-event-log apparatus have **no consumer on the authoritative path**.

| id | defect | at HEAD | constructible on authoritative path | scope | disposition | false PRODUCT FINDING? | alters outcome metrics? |
|---|---|---|---|---|---|---|---|
| A | `EVENT-CAP-CONFIG-MISMATCH` fires only in the tightening direction | present | **no** | diagnostic replay + integration gate | **DEFER** | no | no |
| B | `control_13` vacuous for Echo/generic events | present | **no** | diagnostic replay (instrument neutrality) | **DEFER** | no | no |
| C | F4 bounds duplicates with `DEFAULT_DUP_FACTOR` fallback | present | product behaviour reachable (`factor=3`), but the cap it feeds is never evaluated authoritatively | diagnostic | **DEFER** | no | no |
| D | second RecoverFwd site emits `factor` copies, no cap term | present (line drift) | — | diagnostic replay + lineage fidelity | **DISSOLVED-BY-CORRECTED-PREMISE** as a bound defect | no | no |
| E | `plan_forward_duplication` silence branch — producer with no row/bound | present | **yes** as product behaviour (no feature gate) | diagnostic replay + inventory completeness | **DISSOLVED-BY-CORRECTED-PREMISE** | no | no |

## 9. Event cap / product bound — `UNBOUNDED-BY-PRODUCT`

**OBSERVED SOURCE FACT, re-grounded at HEAD with unchanged line numbers:** `sender.rs:4062`
`fn plan_forward_duplication`; `:4063` the `pending_lost.is_empty()` branch; `:4074-4078` the
`DupRec { due_ns, unit, interval_ns: srtt }` push. Production is driven by a recurring interval with
no count ceiling, so **no finite tick-independent product bound can be derived.**

Consequences, all binding:

* an **observed maximum is not a bound**;
* a harness ceiling requires a **separate preregistered decision** (D0 selects no numeric cap);
* **cap overflow can never be classified as a PRODUCT FINDING, a Gate-5 safety failure, or a
  cold-start efficacy failure**;
* defects **D and E dissolve** under this corrected premise — they were bound defects only while a
  finite product bound was presumed to exist.

## 10. `control_13` — `CONTROL-13-UNCONSTRUCTIBLE-BY-CONFIG`

Under `--release`, default features, `experimental-cold-start=false`, logger OFF its triggering event
**cannot occur**. It is therefore classified an **explicit unreachable control** and is **not marked
PASS**. Exercising it would require an `experimental-cold-start`-only diagnostic replay. Authoritative
features are **not** changed to make it constructible.

## 11. Efficacy-consumer scan — rule NOT triggered

Full membership scan, not a named-call-site sample.

| metric family | consumers |
|---|---|
| channel event logs | all `diagnostic assertion` or `reporting only` |
| matched-attempt fate metrics | `reporting only` + 10 diagnostic controls |
| attempt ordinals | `diagnostic assertion` only |
| event-cap / overflow state | `diagnostic assertion` only, **structurally unreachable** in the authoritative binary |
| diagnostic divergence counts | two distinct colliding families; neither consumed by an efficacy decision |

**`EFFICACY-CONSUMES-DEFERRED-DIAGNOSTIC` does NOT apply.** OFF phenotype gate, ON efficacy verdict
and promotion threshold have **zero implementations** at HEAD.

**CONFIRMED COVERAGE GAP.** The strong per-event **pre-treatment prefix assertion is structurally
unreachable on the authoritative logger-OFF path** — it exists only with the logger ON.

### NEW HIGH-SEVERITY FINDING (not in the five-defect ledger)

**OBSERVED SOURCE FACT.** `PanelOut::first_probe_tick` is never initialised to its sentinel, so the
**Gate-5-labelled `scheduler_data_displacement` counter is a hard-wired constant 0**
(`dr_cs3_panel.rs:20xx`). A displacement metric that cannot be non-zero proves nothing; any past or
future "displacement = 0" reading from this path is vacuous. It is **reporting-only** today, so it
does not trigger the AUDIT-9 rule — but it must be fixed before any Gate-5 claim rests on it.

Two further hazards, same scan: the panel's `scheduler_data_displacement` still cites the
**withdrawn** metric contract in its doc-comment; and the JSON field written at
`dr_cs3_panel_runner.rs:153` is named `channel_fate_divergence`, colliding with the **superseded**
emission-tick metric of the same name.

## 12. Release-runtime smoke candidate

    RELEASE SPEEDUP: UNKNOWN — NOT MEASURED

Nothing was built, run or timed. Candidate: `crates/kama-fec-codec/examples/measure.rs` (tracked,
117 lines). **OBSERVED SOURCE FACT:** it reads no campaign seed or result — a grep over
`fs::|File::|read_to_string|include_str|include_bytes|Path::new|env::var|\.json|reports/` returns
zero matches; `fn main()` takes no arguments and generates all input in-process. Its crate has no
`[features]` section, so it sits natively on the authoritative default-feature axis.

Future two-phase command (Phase A untimed, Phase B the only timed step), with `CARGO_TARGET_DIR`
outside the worktree per §2 ALT-1:

    A: cargo build --release -p kama-fec-codec --example measure     # discarded
    B: "$CARGO_TARGET_DIR"/release/examples/measure                  # timed

Compile and execution time separate two ways: structurally (Phase B compiles nothing) and
intrinsically (the harness self-measures with `Instant` after warmup). **A future timing measurement
is a separate authorised card and must not use a selection or paired-panel seed.**

## 13. Proposed subsequent cards

1. **TASK-D0B — reconstruct the frozen 18-field list** (unblocks §5 and TASK-V1). Without it neither
   the locked-panel declaration nor the V1 comparator can be frozen.
2. **TASK-D0A — V1 reclassification preregistration** (only if ever pursued; D0 recommends against).
3. **TASK-D1 — Design A implementation**, MEDIUM class, new bounded OFF buffer, schema unchanged.
4. **TASK-D1x — fix `first_probe_tick` initialisation** before any Gate-5 claim.
5. **Ignore-rule or external-target decision** for `.gate-target` (D0 chooses neither).

## 14. Unresolved ambiguities

* the identity of the 18 frozen neutrality fields — **not reconstructible from tracked source**;
* which observable surface the 32×18 comparison actually ran over (runner stdout vs JSON records);
* whether the banked 32×18 run used debug or release binaries (already `UNKNOWN / AMBIGUOUS`);
* the exact writer of `.gate-target/debug/**`;
* whether Design A becomes a new mode or restructures the five existing call sites;
* the frozen content of the global fingerprint/probe/phenotype gate;
* the selected-seed list's file format, path and hash (plan TASK-P2 item 1 requires it).

## 15. Final D0 verdict

    STATUS: BLOCKED — BLOCKED-LOCKED-PANEL-DECLARATION

Nine of the ten audits returned source-grounded verdicts. AUDIT 3 cannot: the declaration must
enumerate the 18 frozen neutrality fields, and that list exists nowhere in the repository. The
comparator that knew them lived under `/tmp` and is gone.

`GATE-SELF-DIRTY`, `UNBOUNDED-BY-PRODUCT`, `CONTROL-13-UNCONSTRUCTIBLE-BY-CONFIG` and
`V1-REMAINS-EFFICACY-GATE` are **valid audit findings and do not fail D0**. The blocker is the
missing field list alone.

**Do not run neutrality. Do not start D1.**
