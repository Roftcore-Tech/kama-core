# TRANSPORT BASELINE CLOSURE — EXECUTION STATE

Canonical mutable execution ledger. Immutable scientific artifacts live elsewhere and are not
superseded by this file.

| | |
|---|---|
| Branch | `transport-baseline-closure` |
| HEAD | `a4e9b33b` |
| Local `main` HEAD | `bb61c2da` (**not advanced — no milestone merged yet**) |
| Last successful commit | `a4e9b33b docs(transport): record baseline contracts, panels and supersessions` |
| Result-bearing calibration runs | **0** |
| Remote push / PR | **none** (out of scope) |

## Commits on this branch (5)

```
a4e9b33b docs(transport): record baseline contracts, panels and supersessions
41838ab9 test(transport): validate authenticated bootstrap RTT path
29dc1a10 feat(transport): add bounded bootstrap RTT sender and receiver core
d76d36aa fix(transport): make wakeup deadlines liveness-complete
0b9f6f36 test(transport): expose default-build and semantic feature configurations
```

## Milestones

| | status |
|---|---|
| **M1 baseline liveness** | **IN PROGRESS** — blocked on the gate below |
| M2 bootstrap cold-start | not started |
| M3 final ARQ + estimator | not started |
| M4 product runtime | not started |
| M5 final grounding | not started |

**Nothing merged to local `main`:** M1's acceptance is not met (one failing test under
`experimental-cold-start`), and §7 forbids merging a known-failing state.

## Completed this turn

* **§2 git/worktree:** dedicated branch created from `d1-sprint5c` HEAD `4aa11a93`; working tree
  preserved byte-identically (verified). Work split into 5 single-purpose commits. User WIP
  (`reports/d1-exit/data/cold-start/timeline-{65,300,1200}B.csv`) is in **none** of them.
* **§A1 compile visibility (from the prior turn, now committed):** two `TransmitKind::CsProbe` match
  arms un-gated — `budget_equivalence_pin.rs` (test target did not compile under default features)
  and `udp_runner.rs:1102` (**the LIB did not compile under `--features experimental-fec-budget`
  alone**). All 7 feature combinations compile. CI blind spot recorded: `.github/workflows/ci.yml:18`
  runs only `cargo test --all --all-features`, so the default build is never tested.
* **§1.2 inventory** (`EK4-A2-NEXT-WAKEUP-CONTRACT`, md `d59212b2…` / json `18dffa48…`), hashed
  before any code change.
* **§1.4 fix implemented:** `next_wakeup_ns()` un-gated into the default build and returning the
  earliest of the six existing authoritative sources; `next_timer_deadline_ns()` / 
  `pending_work_ready_ns()` split so C2 keeps its original `cs_wakeup` scope; read-only
  `TokenBucket::ns_until_sendable` and `TailRecovery::armed_at_ns`.
* **Two of my own defects, caught by tests and fixed:** (1) `LostRec.deadline_ns` used as a wakeup
  source — `arq.rs:110-111` is normative that it is an **ordering key**, not a timer, and using it
  produced progress-free wakeups; (2) a test-helper assertion that forbade *overdue* deadlines, which
  are legitimate ("due now").
* 3 of the 4 EK4-A2-family tests that the broader API initially broke now pass.

## Test matrix

| combination | passed | failed |
|---|---|---|
| `<default>` | **244** | 0 |
| `experimental-fec-budget` | **291** | 0 |
| `diag-sched-telemetry` | **244** | 0 |
| `experimental-cold-start` | 158 | **1** |
| `--all-features` | 198 | **1** |

clippy `-D warnings` (`adaptive-transport --all-targets`): **clean**.

## THE FAILING GATE

`dr_cs1_acceptance::f2b_di1c_rearmed_pto_lifecycle_exposure`

### Root cause — isolated to an exact lineage

Phase 2 of the scenario. Measured trace:

```
b2 (ACK of seq 4):  state=Startup  next_wakeup=61209677418 (61.2 s)
                    diag_loss=(0,1,0)  pacer=(rate 51 B/s, tokens 865, cap 1200)
wakeup it0..it19:   wake=61209677418 UNCHANGED, now advances 68.309 s → 68.328 s
                    out=0, state=Startup, pending_lost=0, nothing consumed
```

The reported deadline is the **ARQ time-threshold loss deadline** for inflight `seq 3` (the first
retransmit of unit 1), which is below `largest_acked = 4`. It is already ~7 s overdue and is
**re-reported unchanged forever** — the progress-free chain.

Established by reading the code:

* `LossDetector` declares `on_sent` / `on_ack` / `detect_time_losses` / `on_unrecovered`.
* `detect_time_losses` **is** already invoked on the `Timer::Pto` path
  (`sender.rs:2351-2362`), guarded only by `state != PreConverge` — and the state was `Startup`.
* Its eligibility predicate (`loss.rs:375-386`) is algebraically **identical** to the deadline
  formula in `next_time_loss_deadline_ns` (`loss.rs:280-297`): both use
  `max(scale(srtt, num, den), max(min_delay, cadence_floor))`.

So at `now = 68.3 s > deadline = 61.2 s` the unit *should* be declared lost, and it is not.

### Next hypothesis to verify FIRST on resume

**Externally-supplied `Timer::Pto` is not reaching the loss-declaration branch through
`on_event_batch`.** Evidence: batches containing `TimerFired(Pto)` produced *byte-identical* results
to empty batches (`out=0` in both). The CS batch phase-machine generates its own `Timer::Pto`
internally when a `cs_wakeup` entry fires (`CsTimerKind::BootstrapPto` → "fire = mevcut `Timer::Pto`
core-yolu"), which is why the old `cs_wakeup`-only API worked with empty batches.

Verify by tracing `on_event_batch`'s phase machine for how it handles a caller-supplied
`Event::TimerFired(Timer::Pto)`.

If confirmed, the §3-compliant fix is to make the caller-supplied timer event drive the same
existing loss-declaration path (deadline → existing state-machine eligibility), **not** to add a
timer, relax the ARQ share, change PTO backoff/retries, or reintroduce a periodic tick.

## Invariants that must not change

`arq.rs` deadline-as-ordering-key · C2 scoped to `cs_wakeup` · no new timer / second truth source ·
no periodic safety tick · PTO backoff and retry counts · startup pacing formula · FEC drain and
scheduler call order · per-turn and per-block DATA-before-PARITY · Karn exclusion · no synthetic RTT.

## Defaults (all safe — nothing half-finished is ON)

| config | default |
|---|---|
| startup pacing | **OFF** (restore only after EK4-A2 acceptance) |
| bootstrap RTT | **OFF** |
| tail recovery | **OFF** |
| FEC adaptive policy | **OFF** (deliberate; awaits calibration, not a debt) |

## User WIP manifest (never to be committed)

`reports/d1-exit/data/cold-start/timeline-65B.csv` ·
`reports/d1-exit/data/cold-start/timeline-300B.csv` ·
`reports/d1-exit/data/cold-start/timeline-1200B.csv`

Plus untracked user docs/logs: `docs/design/*`, `docs/tasks/*`, `reports/d1-exit/data/*.log`,
`reports/d1-exit/data/netem-a2/console.txt`, `crates/adaptive-transport/src/fate_estimator.rs`,
`crates/kama-integration/{examples/fs_cal2_runner.rs,tests/common/fs_cal2_*.rs}`,
`reports/fs-cal-2/`, `scripts/fs-cal-2/`.

## Decisions already made — do NOT re-ask

* `next_wakeup_ns()` is the general event-loop deadline API and lives in the default build.
* It aggregates the **earliest** of existing authoritative sources; it never invents a timer.
* C2 stays scoped to `cs_wakeup`.
* Immediate readiness is `Some(now)`; a driver that drains then re-queries must converge.
* Source 6 covers classes planned every turn (`tx_queue`, `pending_fec`, `pending_lost`);
  `dup_queue` reports via its own `due_ns`.
* `LostRec.deadline_ns` is an ordering key and is never a wakeup source.
* A timer wakeup means the event loop delivers `Timer::Pto` at that instant — that is timer-driven,
  not a periodic tick.
