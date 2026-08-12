# TRANSPORT BASELINE CLOSURE — EXECUTION STATE

Canonical mutable execution ledger. Immutable scientific artifacts live elsewhere and are not
superseded by this file.

| | |
|---|---|
| Branch | `transport-baseline-closure` |
| HEAD | see `git log -1` (latest commit below) |
| Local `main` HEAD | `bb61c2da` — **not advanced; `main` IS an ancestor, so `--ff-only` is possible when M1 passes** |
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
| **M1 baseline liveness** | **NOT MERGED** — all technical gates pass in the working tree, but the branch does NOT build in a clean checkout (see CLEAN-CHECKOUT DEFECT) |
| M2 bootstrap cold-start | not started |
| M3 final ARQ + estimator | not started |
| M4 product runtime | not started |
| M5 final grounding | not started |

**`main` is at `bb61c2da` — the merge was performed and then UNDONE.**

### CLEAN-CHECKOUT DEFECT (my error, must be fixed before any merge)

`--ff-only` merged M1 into `main`, and the mandated clean-worktree verification then showed the
merged tree **does not compile**:

```
error[E0583]: file not found for module `fate_estimator`
  --> crates/adaptive-transport/src/lib.rs:34
```

Cause: commit `29dc1a10` committed `crates/adaptive-transport/src/lib.rs`, which carried a
**pre-existing uncommitted** line `pub mod fate_estimator;`, while
`crates/adaptive-transport/src/fate_estimator.rs` stayed **untracked** (I had classified it as user
WIP and excluded it from every commit). I committed a modified file without verifying its
dependencies were tracked.

`main` was restored to `bb61c2da` with `git branch -f` (branch-pointer move only; no working tree
touched, no `reset --hard`, no user file altered). The branch `transport-baseline-closure` still
holds all work at `48ec9d4d`.

**Files that committed code needs but that are untracked:**

| file | needed by |
|---|---|
| `crates/adaptive-transport/src/fate_estimator.rs` | `lib.rs:34` (**product library**) |
| `crates/kama-integration/tests/common/fs_cal2_schema.rs` | `fs_cal2_pins.rs` |
| `crates/kama-integration/tests/common/fs_cal2_spine.rs` | `fs_cal2_schema.rs` |
| `crates/kama-integration/tests/common/fs_cal2_gen.rs` | `fs_cal2_spine.rs`, `dr_cs3_panel.rs` |
| `crates/kama-integration/tests/common/fs_cal2_policy.rs` | `fs_cal2_spine.rs` |
| `crates/kama-integration/examples/fs_cal2_runner.rs` | referenced by reports/tooling |
| `crates/kama-integration/tests/common/fs_exec.rs` | `fs_cal2_gen.rs` (verify) |

**All measured green results remain valid for the working tree**; they were simply never a
clean-checkout claim. Nothing about EK4-A2, the FEC corrigendum or the supersessions changes.

### Next step on resume — exact

1. Enumerate the full untracked closure the committed code needs (`cargo build --all` in a fresh
   worktree until it compiles).
2. Decide per file: `fate_estimator.rs` is required **product source** and must be tracked; the
   FS-CAL harness files are required by committed tests. Neither is authored user WIP in the sense my
   manifest assumed.
3. Commit them in a single-purpose commit, or revert the `lib.rs` hunk if `fate_estimator` is not
   meant to ship yet — decide from the code, not from the manifest.
4. **Re-verify `cargo build --all` and `cargo test --all` in a FRESH worktree BEFORE merging.** The
   clean-worktree check must gate the merge, not follow it.


---

## DONE this turn

### EK4-A2 — **CLOSED**

The checkpoint's hypothesis (external `Timer::Pto` not reaching the loss-declaration branch) was
**REFUTED**: `on_event_core` routes `Event::TimerFired(timer) => self.on_timer(timer, now_ns)`
(`sender.rs:1506`), and `detect_time_losses` is already invoked there.

Per-source instrumentation found the real culprit — **source 3, `dup_queue`**:

```
cs=Some(79.096s)  arq=Some(79.096s)  tail=None  pend=None
dup=Some(61.210s)   ← stale, while now = 68.3 s
```

`next_wakeup_ns()` reported a scheduled geometric copy's `due_ns` verbatim. Once that instant has
passed but the pacer cannot afford the unit, `drain_due_duplicates` leaves the record queued and
`due_ns` never advances — so the same overdue instant was reported forever and a wakeup-driven driver
spun without progress. The cold-start and ARQ deadlines were coherent all along.

**Fix (minimal, no new timer, no second truth source):** the dup source now reports what the
scheduler can actually act on — `due_ns` when still in the future, otherwise the instant the pacer can
afford the unit. Both inputs read from existing authoritative state. ARQ share, PTO backoff, retry
counts and the startup pacing formula are untouched.

**Sealed by `crates/adaptive-transport/tests/ek4_a2_liveness.rs` (7 tests, all green):** external PTO
drives loss declaration at a due deadline · empty batch ≠ real PTO batch in a due-loss state ·
internal + external PTO in one batch do not double-process · wakeup-driven driver progresses with no
periodic tick · no stale-forever instant · `None` only when genuinely idle · accessor is
observation-only · PreConverge still freezes time-threshold declaration.

### CI semantic matrix — **DONE**

`.github/workflows/ci.yml` gains a `semantic-matrix` job that builds, clippies and tests each
configuration separately (default workspace · all-features · `experimental-cold-start` ·
`experimental-fec-budget` · diagnostic), printing per-configuration counts for compiled test
binaries, tests run, ignored and filtered out, and failing if a configuration compiled no test binary.
Plus a `fs-cal-pins` job running the pins in the **default** build. No product wire behaviour touched.

---

## M1 BLOCKERS — ALL RESOLVED

| # | blocker | resolution |
|---|---|---|
| 1 | EK4-A2 event-loop liveness | fixed (`b7b15ddc`); stale `dup_queue` `due_ns` replaced by the instant the scheduler can act; 7 regressions sealed |
| 2 | FEC-ordering trajectory constant | formal supersession; per-turn and per-block DATA-before-PARITY hold with 0 violations |
| 3 | "FEC parity starvation" | **WITHDRAWN — never a product defect.** Both arms emit 4/4 parity; floor ON is 25 ms EARLIER. The harness counted parity only from the `Timer::Pto` return and discarded the ACK call's return; floor ON moved emission onto the ACK instant (20779). Corrigendum `3bfd4a9d…` |
| 4 | D2/R2 spurious-retransmit assertion | supersession `7718b94f…`: the floor removes the late-parity leak (`spurious 0`), which is an improvement. Original assertions preserved verbatim, re-scoped to the pacing-inactive path; companion test seals the default |

## Test matrix (current tree, default OFF)

| scope | passed | failed |
|---|---|---|
| workspace `--all` (default features) | **521** | 0 |
| workspace `--all --all-features` | **873** | 0 |
| adaptive-transport default | 249 | 0 |
| adaptive-transport `experimental-cold-start` | 274 | 0 |
| adaptive-transport `diag-sched-telemetry` | 249 | 0 |
| adaptive-transport `experimental-fec-budget` | 296 | 0 |
| adaptive-transport `--all-features` | 323 | 0 |
| kama-integration default / cold-start | 71 / 109 | 0 |

clippy `-D warnings` clean on adaptive-transport and kama-integration, default and feature builds.
No-loss rate-ramp: ON vs OFF **30/30 byte-identical** (wire, source prefix, delivered).

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
