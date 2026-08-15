# DR-CS-3 — M2 EXECUTION ADDENDUM (TASK-A0R2)

Decision freeze only. No harness behaviour is implemented here, nothing is compiled or run, no
worktree is cleaned, no WIP is parked, and no result-bearing campaign data was read.

    source HEAD        : 832e6ef7791897772627644b74432d3fad4a68fe
    branch             : transport-baseline-closure
    tracked modified   : 0
    A0 manifest        : /tmp/kama-m2-a0/manifest.md
    A0 manifest sha256 : a38a1f221ee1aab05fcbcf9b51f54a5be032a2ce560f88b504e6a013536ebd0a  (verified)

---

## 1. Historical A0 hash record — what A0 actually preserved

A0 recorded **truncated** prefixes for four files and **no hash at all** for two:

    fate_estimator.rs   prefix 9116cc39af77870a
    fs_cal2_spine.rs    prefix 1ebe8feec17cbec0
    fs_cal2_schema.rs   prefix e76cfe1483e33424
    fs_cal2_pins.rs     prefix 007fdaaed10ec565
    fs_cal2_runner.rs   no hash
    fs_cal2_policy.rs   no hash

    A0 contains ZERO complete 64-hex WIP hashes.

The A0 manifest is **not edited** and is not retroactively completed. The hashes in §2 are newly
measured by A0R2 and are labelled as such; they are not presented as A0 values.

## 2. A0R2-CURRENT-WIP-SNAPSHOT

    classification : OPERATIONAL PARK/RESTORE INTEGRITY EVIDENCE
                     NOT EXPERIMENTAL RESULT
                     NOT A RETROACTIVE A0 BYTE SNAPSHOT
    measured at    : HEAD 832e6ef7, two independent passes, byte-identical

| # | path | tracked | bytes | mtime | sha256 | A0 continuity |
|---|---|---|---:|---|---|---|
| 1 | `crates/adaptive-transport/src/fate_estimator.rs` | UNTRACKED | 21736 | 2026-08-10 15:59:39 | `9116cc39af77870af5d94ac2c23be5649f8d18265d8ac0b25a9f6eada5db685c` | prefix matches A0 |
| 2 | `crates/kama-integration/tests/common/fs_cal2_spine.rs` | UNTRACKED | 51607 | 2026-08-12 10:06:31 | `1ebe8feec17cbec0effb25da208708dea19e7c95266ca4a3e58fc9c93a54c3d7` | prefix matches A0 |
| 3 | `crates/kama-integration/tests/common/fs_cal2_schema.rs` | UNTRACKED | 27546 | 2026-08-12 10:06:31 | `e76cfe1483e334241d3b89cca52f52be579a1f3aa63ddf523dbda0a5a5e283f9` | prefix matches A0 |
| 4 | `crates/kama-integration/tests/fs_cal2_pins.rs` | UNTRACKED | 14497 | 2026-08-12 17:26:43 | `007fdaaed10ec5659aee7b499a832a7b73ae902c568edb71638507a5f2312087` | prefix matches A0 |
| 5 | `crates/kama-integration/examples/fs_cal2_runner.rs` | UNTRACKED | 49800 | 2026-08-12 10:07:17 | `3400564f134eedfa32ee9440deb97eb3f4bedf363dd2fdf6957b208fdee4171c` | **no A0 hash exists** |
| 6 | `crates/kama-integration/tests/common/fs_cal2_policy.rs` | UNTRACKED | 11935 | 2026-08-10 16:21:44 | `be0c5846137e215054a535b534a8ee8b93e3658b26dfba727e0299df4cc58d14` | **no A0 hash exists** |

Files 5 and 6 were **discovered during A0** but never hashed there. A0R2 records their first
complete hash. **Continuity before A0R2 is not claimed for them.**

All six are untracked at A0 HEAD. Broad `git clean` / `rm` / `stash` remains forbidden; any future
parking must be exact-path-only, reversible, and hash-verified. Build artefacts
(`.gate-target/`, `.nb-*/`) and `reports/fs-cal-2/` are **separate classes** and must never be
conflated with these six WIP source files.

Downstream obligation:

* **TASK-A1** verifies these six hashes immediately before parking.
* **TASK-A2** verifies both the parked bytes and the restored bytes against them.
* Any mismatch stops the workflow.

## 3. Worker-contention calibration

    WORKER-CONTENTION CALIBRATION:
      WAIVED — SERIAL EXECUTION

    authoritative_worker_count = 1

Concurrency is **eliminated, not estimated**, so there is nothing to calibrate.

* `CARGO_BUILD_JOBS=2` is **compile** parallelism and is explicitly **not** evidence for
  experiment-worker concurrency.
* CPU accounting remains **NOT MEASURED**.
* Wall-time and RSS values are empirical estimates, never bounds.
* No worker-count sweep will be executed.
* Any future `worker_count > 1` requires a **separately preregistered and validated** contention
  calibration.

Banked estimate, not promoted to a bound:

    512 x 5.518 s ~= 47 minutes
    EMPIRICAL SERIAL EXECUTION ESTIMATE — NOT A RUNTIME GUARANTEE

This waives **only** §12.6's parallel-worker calibration requirement. It does **not** waive §12.6
as a whole.

## 4. Lossless disposition of every TASK-C0 / TASK-C1 obligation

Read from the M2 execution plan, `TASK-C0` (lines 954-975) and `TASK-C1` (lines 979-1004).

**Provenance warning.** `specs/Kama_M2_Claude_Code_Atomik_Yurutme_Plani_2026-08-15.md` is
**UNTRACKED** (`git ls-files --error-unmatch` fails; `git status` shows `??`). The whole disposition
below therefore rests on a file that is not in git — the same reproducibility failure mode §12
records for the deleted `/tmp` comparator. Tracking that plan is a prerequisite for D0.

| # | Original obligation | Source | Disposition | How PASS/FAIL is evaluated |
|---|---|---|---|---|
| 1 | worker-contention calibration (worker candidates 1 and 2) | C0 | **WAIVED — serial worker=1** | §3; a sweep is not run, and `worker_count = 1` is asserted by S0 and by every S1 repetition |
| 2 | final authoritative binary build + hash | C0 | **TASK-V5** | V5 emits the binary path and sha256 (§5); S0/S1 abort on `FROZEN-BINARY-MISMATCH` |
| 3 | logger OFF | C0 | **TASK-S0 preflight + every TASK-S1 run** | S0 asserts `logger = OFF`; S1 re-asserts per repetition |
| 4 | default-feature axis | C0 | **TASK-V5 build**, re-verified by S0/S1 fingerprint | V5 records the complete feature fingerprint; S0/S1 compare against it |
| 5 | `experimental-cold-start = false` | C0 | **TASK-V5 build fingerprint**, verified by S0/S1 | S0 asserts it directly; mismatch ⇒ `INVALID-BASELINE-CONFIG — ABORT SELECTION` |
| 6 | rep9-converging / rep16-stalling calibration samples | C0 | **SUPERSEDED — no longer required** | These existed only to size a worker sweep. With `worker_count = 1` there is no sweep to size. No integrity gate depends on them; the serial estimate in §3 replaces their only downstream use |
| 7 | separate compile time from experiment run time | C0 | **TASK-V5 / TASK-S1** | V5 owns compilation; S0/S1 invoke the frozen binary directly and never rebuild, so the two are disjoint by construction (§5) |
| 8 | result collection keyed by repetition index | C0 | **TASK-S1 assertion** | S1 asserts the collection key is the repetition index for every result |
| 9 | worker-completion order must not be used | C0 | **TASK-S1 assertion** | Serial execution makes it degenerate, but S1 still asserts completion order is never used as result identity |
| 10 | sim-tick measurement | C0 | **TASK-S1 descriptive record** | Recorded per repetition, classified `EMPIRICAL DESCRIPTIVE OBSERVATION — NOT A PRODUCT BOUND` |
| 11 | forward/reverse event measurement | C0 | **TASK-S1 descriptive record** | As above; must not become an event cap, a runtime bound, or a product finding |
| 12 | wall time / process CPU / peak RSS measurement | C0 | **TASK-S1 descriptive record**, CPU explicitly NOT MEASURED | Wall and RSS recorded as estimates; CPU accounting stays unmeasured per §3 |
| 13 | per-worker command / wall / CPU / RSS measured | C1 | **WAIVED — serial worker=1** | Only one worker exists; no per-worker sweep is run |
| 14 | no OOM, swap thrash, or target-dir collision | C1 | **TASK-S1 execution-health check** | S1 records the condition; target-dir collision is structurally impossible since S0/S1 invoke a prebuilt binary and never write a Cargo target dir |
| 15 | chosen worker count rests on **measured** resource limits, not a blind default 2 | C1 | **WAIVED — NOT SATISFIED BY MEASUREMENT** | Honest statement: nothing was measured. §3 declares CPU accounting `NOT MEASURED`, so this obligation is **not** met on its own terms. It is waived together with rows 1 and 13, because eliminating concurrency removes the resource-limit question the obligation exists to answer. Recorded as a waiver, not as satisfaction |
| 16 | binary/config/logger fingerprint equals V5 | C1 | **TASK-S0 preflight + TASK-S1 per-run assertion** | Any mismatch ⇒ `FROZEN-BINARY-MISMATCH — ABORT` |
| 17 | **C0 PASS: decision rule and commands hashed before results** (plan line 975) | C0 | **TASK-A0R2 (this addendum) + TASK-S0** | This addendum freezes the decision rule (§3-§10) and the command ledger (§11) and is committed **before** any selection result exists; its committed blob sha256 is the hash. S0 re-verifies the ledger against the frozen binary before reading any result |
| 18 | **C1 FAIL: dropping to worker 1 is permitted only if the C0 rule allows it, else BLOCKED** (plan line 1004) | C1 | **SATISFIED — C0 rule permits it** | C0 line 968 states `Worker adayları: 1 ve 2`. `1` is inside C0's own candidate set, so selecting it is permitted by the C0 rule and the `BLOCKED` branch does not apply. Recorded explicitly rather than assumed |

Every obligation from both cards appears exactly once above with an explicit destination. No row
says merely "covered elsewhere".

Scope note on the table's sources: rows 1-12 and 17 come from C0, rows 13-16 and 18 from C1. An
earlier draft carried a row for "review of the retained freeze obligations" attributed to C1; that
obligation does **not** exist in C0 or C1 — the plan's reviewer convention lives at §2.4
(lines 153-157) and in the separate audit cards — so it has been removed rather than left as an
invented obligation. Independent review of this disposition still happens, but it is inherited from
§2.4, not from C0/C1.

Therefore, and only therefore:

    TASK-C0: SUPERSEDED-BY-A0R2-DISPOSITION
    TASK-C1: SUPERSEDED-BY-A0R2-DISPOSITION

This does **not** mean their obligations were waived. Rows 1, 13 and 15 — the worker-contention
calibration itself and the measured-resource-limit basis that only exists to justify a worker count
above 1 — are waived. Everything else was reassigned losslessly. Row 6 is the single obligation
retired outright, and it is retired because its only purpose was to size the waived sweep.

## 5. TASK-V5 — authoritative binary contract

TASK-V5 is the **only** authoritative experiment-binary builder.

V5 builds with:

    cargo build --release
    Cargo default features
    experimental-cold-start = false

The exact package / bin / example target is taken from the resolved A0 entry-point lineage
(`-p kama-integration --example dr_cs3_panel_runner`, §11) and is **not invented here**.

V5 output must record:

    full binary path
    full binary sha256
    rustc version
    Cargo profile = release
    complete feature fingerprint
    experimental-cold-start = false
    logger capability state
    source HEAD
    dirty / tracked state
    build command
    build timestamp

After V5:

* S0 and S1 **may not** use `cargo run`.
* S0 and S1 **may not** silently rebuild.
* S0 and S1 invoke the frozen binary **directly**.
* Before every authoritative run the measured binary sha256 must equal V5's.
* Any mismatch: `FROZEN-BINARY-MISMATCH — ABORT`.

Profile and features are **orthogonal**: default features does not imply debug, and `--release`
does not change the frozen feature axis.

## 6. TASK-S0 — preflight contract

Before any selection result is read, S0 asserts:

    binary sha256 == V5
    Cargo profile fingerprint = release
    Cargo feature fingerprint = default features
    experimental-cold-start = false
    logger = OFF
    OFF-arm bootstrap probe delta = 0
    OFF-arm legacy CS probe delta = 0
    campaign worker_count = 1
    result ordering key = repetition index

Any mismatch:

    INVALID-BASELINE-CONFIG — ABORT SELECTION

No seed substitution and no continuation is permitted.

## 7. TASK-S1 — collection contract

For every repetition S1 asserts:

    exact frozen V5 binary hash
    identical feature / config / logger fingerprint
    worker_count = 1
    repetition index is the collection key
    completion order is never used as result identity
    sim ticks recorded
    forward event count recorded
    reverse event count recorded

The last three are classified:

    EMPIRICAL DESCRIPTIVE OBSERVATIONS — NOT PRODUCT BOUNDS

They must not be converted into an event cap, a runtime bound, or a product finding.

## 8. Dependency update

Plan graph before (from the untracked plan document, lines 319-334):

    TASK-V5 → TASK-C0 → TASK-C1 → TASK-S0 → TASK-S1 → TASK-S2

After removing the two superseded nodes:

    TASK-V5 → TASK-S0 → TASK-S1 → TASK-S2

Every other pre-existing predecessor/successor edge is preserved unchanged:

* the A0 fan-out roots `A0 → A1 → A2`, `A0 → D0 → D1 → D2`, `A0 → E1 → E2`, `A0 → V0 → V1 → V2 → V3`;
* `V4 → V5` on the normal path, **and** the 13:00 time-cut variant (plan line 328) under which
  `V5` is reached without `V4` and `V4` moves after P4 — that variant is preserved as written;
* the `n<3` tail `normalde TASK-R0; time-cut yolunda TASK-V4 mümkünse → TASK-R0`;
* the `n≥3` tail `P0 → P1 → P2 → P3 → P4 → time-cut yolunda TASK-V4 mümkünse → TASK-R0`.

The conditional `TASK-V4` in both R0 tails is part of the graph and is **not** dropped here.

Explicitly:

* worker calibration was waived;
* non-calibration obligations were reassigned (§4);
* **no integrity gate was dropped**;
* selection remains impossible before V5 **and** S0 PASS.

## 9. Release-neutrality scope

    BANKED_NEUTRALITY_PROFILE: UNKNOWN / AMBIGUOUS

The tracked result artifact `DR-CS-3-NEUTRALITY-32x18-RESULT.txt` records the record count, the
field count, the comparator name and the commit lineage, but **not the Cargo profile**. A0 recorded
the same ambiguity from the opposite direction: `RESUME-NEXT-SESSION.md` documents the entry point
without `--release`, while every prebuilt example binary found on disk is a release binary and
`EXECUTION-STATE.md:561` refers to a "warmed release binary". Session-level path lineage suggests
the banked run used debug binaries, but that is **not** immutable tracked evidence and is therefore
not recorded as proof.

Consequently the banked PASS is preserved as **historical supporting evidence** and is
**insufficient** as the final authoritative release-neutrality gate.

**No debug→release or release→debug neutrality inference is permitted in either direction.**

    V1-EFFICACY:
      baseline and candidate both built and executed under --release
      identical rustc / toolchain
      identical default feature set
      experimental-cold-start = false
      identical invocation
      candidate logger OFF
      frozen 32x18 comparator
      zero mismatch / missing / extra

Failure:

    OBSERVATION-NEUTRALITY-FAILURE — EFFICACY PATH BLOCKED

Logger-ON neutrality, if executed, is classified separately:

    V1-DIAGNOSTIC:
      logger ON
      failure invalidates diagnostic replay only
      failure does not automatically become a cold-start product finding

V1-DIAGNOSTIC can never substitute for the release / logger-OFF efficacy neutrality above.

## 10. Pairing execution contract

Preferred structural order, preserved:

    all paired OFF runs
      → global fingerprint / probe / phenotype gate
      → all paired ON runs

TASK-D0 must first measure:

* the exact source change required;
* the affected call sites;
* whether locked-panel behaviour changes;
* whether the change is additive / reporting-only;
* the expected neutrality fields;
* implementation and validation cost.

A per-repetition OFF→ON fallback is **not authorised** by this card. Retroactive invalidation is
**not** equivalent to the structural all-OFF-before-ON guarantee.

Measured basis (A0 manifest §7): `run_pair` at `dr_cs3_panel_runner.rs:202-207` runs OFF then ON in
a single call, and is invoked from inside the per-repetition loop at `:265`, `:292`, `:322`, `:345`
and `:387` — so `ON(rep_i)` precedes `OFF(rep_i+1)`. The runner contains **zero assertions**, so
verification is by construction an external post-hoc step.

## 11. Entry-point ledger — A0 manifest §6-§7

**The six commands below are byte-for-byte verbatim from A0.** The surrounding annotations are not:
`source :` lines were added for items (2), (3), (4) and (6) (A0 recorded them only in prose), and
item (7)'s **status label** carries this card's decision rather than A0's wording — see the note
under (7). Everything a later card executes is the verbatim A0 command.

No `${REPO_ROOT}` substitution is used: A0's commands contain no absolute repo path, so the
substitution would be vacuous.

**(1) 32x18 neutrality — record production — RESOLVED**

    cargo run --release -p kama-integration --example dr_cs3_panel_runner -- locked --reverse none
    cargo run --release -p kama-integration --example dr_cs3_panel_runner -- locked --reverse ge

    source  : crates/kama-integration/examples/dr_cs3_panel_runner.rs:256-278
    profile : --release as resolved by A0 (see the ambiguity note below)
    16 records per invocation (2 tail configs x 8 reps); 32 per tree.
    HAZARD  : default --outdir reports/dr-cs-3/panel/locked is relative and already holds the 32
              untracked neutrality records — re-running OVERWRITES them. Pass --outdir <fresh>.

**(2) structural oracle — RESOLVED**

    cargo test -j2 -p adaptive-transport --test dr_cs3_structural_oracle -- --nocapture

    source : crates/adaptive-transport/tests/dr_cs3_structural_oracle.rs

**(3) dr_cs3_v2_channel — RESOLVED**

    cargo test -j2 -p kama-integration --test dr_cs3_v2_channel

    source : crates/kama-integration/tests/dr_cs3_v2_channel.rs
             + controls in crates/kama-integration/tests/common/dr_cs3_panel.rs
    ~723 s for 45 tests — target individual tests with `-- --exact <name>`.

**(4) 13-check gate — RESOLVED**

    scripts/gate.sh

    source : scripts/gate.sh
    MUST run in a FRESH worktree: A0 measured 86 untracked paths in this checkout and `.gate-target`
    is not ignored, so step 8 (worktree-clean) fails here regardless of code state.
    Step 0 (fmt) is expected RED: POST-CI-FORMAT-DRIFT.

**(5) 512 OFF-only selection — RESOLVED**

    cargo run --release -p kama-integration --example dr_cs3_panel_runner -- select \
      --n 512 --domain DRCS3-V2-CHALLENGE 2>&1 | tee <ledger>/select-512-off.log

    source : crates/kama-integration/examples/dr_cs3_panel_runner.rs:406-433
    OFF-only is hard-coded (`c.bootstrap_on = false` at :417) and cannot be disabled from the CLI.
    Both flags equal the code defaults and the frozen prereg, so passing them is pinning.
    GAP    : `select` writes NO files (hence the tee), and emits tx_data only for QUALIFYING seeds,
             so the full-universe forward-packet-count distribution required by prereg addendum A2
             is never produced.

**(6) paired OFF/ON panel — RESOLVED**

    cargo run --release -p kama-integration --example dr_cs3_panel_runner -- fresh \
      --n 40 --domain DRCS3-FRESH-V1 --reverse none

    source : crates/kama-integration/examples/dr_cs3_panel_runner.rs:282-306

**(7) §12.6 calibration**

A0's own status line, verbatim:

    SECTION 12.6 CALIBRATION (CPU / RSS / worker budget)      NOT RESOLVED

A0's finding, verbatim: the `calib` mode has no CPU accounting, no per-case selector and no
worker-contention sweep; `/usr/bin/time -v` around a run would give wall time and peak RSS only, not
the measured user/system CPU and 1/2/4-worker contention the section specifies — **producing it
requires an EDIT**.

This card's decision, layered on top of that unchanged A0 finding:

    PARALLEL WORKER-CONTENTION ENTRY POINT:
      NOT PRODUCIBLE AT HEAD
      WAIVED BECAUSE AUTHORITATIVE EXECUTION IS SERIAL

A0 said `NOT RESOLVED`; that A0 status is preserved above and is **not** overwritten. The waiver is
this card's disposition (§3), recorded separately.

**Profile ambiguity is preserved, not repaired.** A0 resolved these commands with `--release`, while
`RESUME-NEXT-SESSION.md` documents the runner without it. That contradiction is recorded as A0 left
it. §5's release contract is the **authoritative future rule** and is recorded separately; it does
not retroactively re-label the banked commands.

## 12. Requirements carried forward from TASK-A0R

**Neutrality comparator reproducibility.** The deleted `/tmp` comparator does **not** retroactively
invalidate the banked PASS record — but the banked result is **not fully reproducible** while its
comparator is absent. Before final V1 a **tracked** comparator must be reconstructed containing:

* the frozen 18-field comparison list;
* fixed-name diagnostic exclusions (never "whatever differs");
* mismatch detection;
* missing-record detection;
* extra-record detection;
* a positive synthetic fixture;
* a negative mismatch fixture;
* a negative missing-record fixture;
* a negative extra-record fixture.

Every negative fixture must turn the comparator RED **through its own intended guard**. Final
release neutrality must be re-run with this tracked comparator.

**Baseline reconstruction.** The prior baseline worktree is prunable/absent and is **not**
authoritative state. V1 must recreate a detached worktree from `d510070d`, verify commit identity
before building, and use the same toolchain, feature set, profile and invocation as the candidate.
No result may depend on the deleted worktree.

**`.gate-target` status.**

    UNRESOLVED — D0 READ-ONLY INSPECTION REQUIRED

D0 must determine whether `scripts/gate.sh` creates `.gate-target` inside the checked worktree,
whether gate step 8 observes it, whether the directory is pre-existing residue or gate-created,
whether a fresh worktree reproduces the issue, and whether `CARGO_TARGET_DIR` can safely live
outside the repo. **No `.gitignore` or `gate.sh` fix is authorised here.**

**D0 locked-panel declaration.** Before V1 and before any result-bearing run, D0 must freeze one of:

    COMMIT-D LOCKED-PANEL BEHAVIOUR:
      UNCHANGED — additive / reporting-only
      expected V1 mismatches: zero

or:

    COMMIT-D LOCKED-PANEL BEHAVIOUR:
      expected changed fields: <exact frozen list>
      causal source: <exact code path>

The declaration must be made **before** neutrality output is observed. A field not listed in advance
cannot be excused after V1.

**Provenance.** Source HEAD `832e6ef7791897772627644b74432d3fad4a68fe`; A0 manifest sha256
`a38a1f221ee1aab05fcbcf9b51f54a5be032a2ce560f88b504e6a013536ebd0a`. **No result-bearing data was
accessed while producing this addendum.**
