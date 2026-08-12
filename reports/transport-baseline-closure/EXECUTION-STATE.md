# TRANSPORT BASELINE CLOSURE — EXECUTION STATE

Canonical mutable execution ledger. Immutable scientific artifacts live elsewhere and are not
superseded by this file.

| | |
|---|---|
| Branch | `transport-baseline-closure` |
| HEAD | see `git log -1` (latest commit below) |
| Local `main` HEAD | **`02915bd9`** — M1 merged `--ff-only`; verified green in a clean worktree AFTER the merge |
| Result-bearing calibration runs | **0** |
| Remote push / PR | **none** |

## Commits on this branch

```
56f196c5 ci: add semantic feature matrix with visible test counts
412ff8d8 test(transport): track the restored startup-pacing default in FS-CAL pins
b7b15ddc fix(transport): report wakeups the scheduler can actually act on, and restore startup pacing
ee5075d3 docs(transport): add canonical execution-state ledger
a4e9b33b docs(transport): record baseline contracts, panels and supersessions
41838ab9 test(transport): validate authenticated bootstrap RTT path
29dc1a10 feat(transport): add bounded bootstrap RTT sender and receiver core
d76d36aa fix(transport): make wakeup deadlines liveness-complete
0b9f6f36 test(transport): expose default-build and semantic feature configurations
```

## Milestones

| | status |
|---|---|
| **M1 baseline liveness** | **PASS** — verified entirely in a clean worktree from committed content |
| M2 bootstrap cold-start | not started |
| M3 final ARQ + estimator | not started |
| M4 product runtime | not started |
| M5 final grounding | not started |

**M1 IS MERGED.** `main` `bb61c2da` → `02915bd9` via `--ff-only`, local only (no push, no PR).

Post-merge verification in a second clean worktree from merged `main`:
`--workspace --all-targets --all-features --no-run` **exit 0** · workspace default **472 / 0** ·
`--all-features` **826 / 0** · `fuzzy_e2e` **5 / 0** · `ek4_a2_liveness` **5 / 0**.
The verdict was withheld until these exit codes were read.

**Next: M2** — DR-CS-3 panel contract V2, bootstrap validation, default decision.

### CLEAN-CHECKOUT DEFECT — corrected record and repair

**What was and was not broken:**

* `main@bb61c2da` **compiled cleanly before, and does now** — re-verified in a fresh worktree,
  `cargo build --all` exit 0. `main` points at it again. **No claim is made that `main` was broken.**
* What was broken is the **commit closure of `transport-baseline-closure` after `29dc1a10`**:
  committed files referenced sources that were never tracked.
* Untracked files in the dirty working tree **masked** the missing commits, so every in-tree gate
  passed while a clean checkout could not build.
* The temporary `--ff-only` merge therefore failed in a clean checkout, and the **branch pointer was
  safely restored** with `git branch -f` (pointer move only — no `reset --hard`, no working tree or
  user file touched).

**Missing closure, enumerated by `cargo test --workspace --all-targets --all-features --no-run`:**
`crates/adaptive-transport/src/fate_estimator.rs` · `tests/common/fs_cal2_{schema,spine,gen,policy}.rs`.
(`fs_exec.rs` and `fs_f0.rs` were already tracked; `examples/fs_cal2_runner.rs` is referenced by no
committed code.)

**Repair (`15a82531`) — decided per file from scope, not from the working-tree label:**

| item | stage it belongs to | decision |
|---|---|---|
| `pub mod fate_estimator;` | AŞAMA 6 / **M3** | hunk reverted — committed early; the file itself untouched on disk |
| `fs_cal2_pins.rs` | **FS-CAL-2 YETKİ-1** | untracked again (its whole `fs_cal2_*` harness is untracked); it was also the only committed user of `fate_estimator` |
| `dr_cs3_panel.rs`, `dr_cs3_panel_runner.rs` | **M2** | untracked again; need `fs_cal2_gen.rs` and will land in M2 with it |
| `fs-cal-pins` CI job | — | removed with the suite it invoked |

`git rm --cached` restores exactly the untracked state these files had before, so **nothing is lost
from disk**. M1 keeps only what it owns: the EK4-A2 wakeup fix and regressions, startup pacing
default-ON, the FEC-ordering and spurious-retransmit supersessions, the `fuzzy_e2e` observation fix,
and the semantic CI matrix.

**Clean worktree now compiles:** `cargo test --workspace --all-targets --all-features --no-run`
exit **0**.

**Process rule now binding:** the clean-worktree gate must **precede** the merge. Last time it ran
after, which is how a broken closure reached `main` at all.

## M1 BLOCKERS — ALL RESOLVED

| # | blocker | resolution |
|---|---|---|
| 1 | EK4-A2 event-loop liveness | fixed (`b7b15ddc`); stale `dup_queue` `due_ns` replaced by the instant the scheduler can act; 7 regressions sealed |
| 2 | FEC-ordering trajectory constant | formal supersession; per-turn and per-block DATA-before-PARITY hold with 0 violations |
| 3 | "FEC parity starvation" | **WITHDRAWN — never a product defect.** Both arms emit 4/4 parity; floor ON is 25 ms EARLIER. The harness counted parity only from the `Timer::Pto` return and discarded the ACK call's return; floor ON moved emission onto the ACK instant (20779). Corrigendum `3bfd4a9d…` |
| 4 | D2/R2 spurious-retransmit assertion | supersession `7718b94f…`: the floor removes the late-parity leak (`spurious 0`), which is an improvement. Original assertions preserved verbatim, re-scoped to the pacing-inactive path; companion test seals the default |

## Test matrix — CLEAN WORKTREE, committed content only

| gate | result |
|---|---|
| `cargo test --workspace --all-targets --all-features --no-run` | **exit 0** |
| workspace default | **472 / 0** |
| workspace `--all-features` | **826 / 0** |
| adaptive-transport `experimental-cold-start` | **258 / 0** |
| adaptive-transport `experimental-fec-budget` | **280 / 0** |
| adaptive-transport `diag-sched-telemetry` | **233 / 0** |
| `fuzzy_e2e` (FEC_R2 `planned=4 emitted=4`) | **5 / 0** |
| `fs_fec_arms` (cost split, both pacing axes) | **4 / 0** |
| `ek4_a2_liveness` default / cold-start | **5 / 0**, **7 / 0** |
| `budget_equivalence_pin` | **5 / 0** |
| `startup_floor_*` seals | **5 / 0** |
| `inv_rate_1_*` seals | **2 / 0** |
| clippy `-D warnings` ×4 scopes | **clean** |

Counts are lower than the earlier dirty-tree figures (which were 521 / 874) because `fs_cal2_pins.rs`
and the panel harness are deferred; this is the honest committed-content baseline.

**Rate-ramp coverage in M1 — correction.** An earlier note claimed deferring `fs_cal2_pins.rs`
removed the startup-floor regressions from M1. It did not; that was a wrong test filter. The full
active-floor contract is in the `adaptive-transport` lib under `startup_floor_*` /`inv_rate_1_*`
names and all of it passes. Only the **panel-level seeded replay**
(`rate_lock_seeds_escape_the_fixed_point`) defers with the FS-CAL harness.

**Post-test cleanliness caveat (known, benign).** After a test run the clean worktree shows
`reports/d1-exit/data/cold-start/timeline-*.csv` modified:
`crates/adaptive-transport/tests/cold_start_liveness.rs` writes evidence artifacts into the repo tree
via `std::fs::write`. This is a repo-hygiene issue (a test writing into `reports/`), not a product
defect and not an unexplained change. Recorded rather than worked around.

## Defaults (nothing unverified is ON)

| config | default |
|---|---|
| startup pacing | **OFF** (blocked by the FEC-parity starvation above) |
| bootstrap RTT | **OFF** |
| tail recovery | **OFF** |
| FEC adaptive policy | **OFF** (deliberate; awaits calibration, not a debt) |

## Invariants that must not change

`arq.rs` deadline-as-ordering-key · C2 scoped to `cs_wakeup` · no new timer / second truth source ·
no periodic safety tick · PTO backoff and retry counts · startup pacing formula · FEC drain and
scheduler call order · per-turn and per-block DATA-before-PARITY · Karn exclusion · no synthetic RTT ·
explicit FEC_R2 must stay runtime-executable.

## User WIP manifest (never to be committed)

**CORRECTION.** `reports/d1-exit/data/cold-start/timeline-{65,300,1200}B.csv` and `summary.csv` were
listed here as user WIP. That was **wrong**: they are **test-generated evidence artifacts**.
`crates/adaptive-transport/tests/cold_start_liveness.rs::cold_start_timeline_evidence_artifact`
(`:132-174`) writes them into the repo tree with `std::fs::write` on every run, so **any workspace
test run rewrites them**. A `cargo test --all` in this turn did exactly that; the prior bytes were
never committed or stashed and are not recoverable. Nothing hand-authored was lost — the earlier
modified state was itself drift from a previous run (present in this session's first `git status`,
before any of my changes) — and the files now match HEAD exactly.

They must still stay **out of every product-repair commit**, but for a different reason: they are
regenerated output, not authored content.

Genuine user WIP (untracked, never touched): `docs/design/*`, `docs/tasks/*`,
`reports/d1-exit/data/*.log`, `reports/d1-exit/data/netem-a2/console.txt`,
`crates/adaptive-transport/src/fate_estimator.rs`,
`crates/kama-integration/{examples/fs_cal2_runner.rs,tests/common/fs_cal2_*.rs}`,
`reports/fs-cal-2/`, `scripts/fs-cal-2/`.

## Decisions already made — do NOT re-ask

* `next_wakeup_ns()` is the general event-loop deadline API, lives in the default build, and
  aggregates the **earliest** of existing authoritative sources; it never invents a timer.
* C2 stays scoped to `cs_wakeup`.
* Immediate readiness is `Some(now)`; a driver that drains then re-queries must converge.
* A source whose emission is token-gated must report the instant the scheduler can act, not a stale
  eligibility timestamp.
* `LostRec.deadline_ns` is an ordering key and is never a wakeup source.
* A timer wakeup means the event loop delivers `Timer::Pto` at that instant — timer-driven, not a
  periodic tick.
* Default-tracking pins follow whatever the default is; their assertions never weaken.
