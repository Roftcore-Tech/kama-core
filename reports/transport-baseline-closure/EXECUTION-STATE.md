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
| **M1 baseline liveness** | **EK4-A2 DONE · CI matrix DONE · startup-pacing default-ON BLOCKED** |
| M2 bootstrap cold-start | not started |
| M3 final ARQ + estimator | not started |
| M4 product runtime | not started |
| M5 final grounding | not started |

**Nothing merged to local `main`** — M1 requires startup pacing default-ON, which is blocked below.

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

## THE BLOCKING GATE — startup pacing default-ON

Restoring the default to ON was attempted and **reverted**. It is a THIRD, separate blocker, unrelated
to EK4-A2 and to the FEC-ordering supersession.

**`crates/kama-engine/tests/fuzzy_e2e.rs`**
`e2e_real_fuzzy_nonzero_r2_to_emission` and `e2e_real_fuzzy_partial_block`

```
floor ACTIVE   : ratio=800 planned=4 emitted=0     ← zero parity reaches the wire
floor INACTIVE : ratio=800 planned=4 emitted=4     ← all four emit
```

Isolated by toggling **only** `set_startup_pacing_init` in `controller_sender()` (file restored with
`cp`, never `git checkout`; `git diff` on it is empty).

The controller plans r=2 parity and, with the floor active, **no parity unit reaches the wire in 900
ticks (9 s virtual)**. `planned > 0 && emitted == planned` fails at `fuzzy_e2e.rs:277`.

This is not a stale trajectory constant: it is a materially worse outcome than the floor-OFF
behaviour, and §12's final acceptance requires explicit FEC_R2 to remain **runtime-executable**.
Shipping the default would trade a fixed liveness defect for a broken FEC actuator, so the default
stays OFF. The mechanism remains available via `set_startup_pacing_init`.

### Next step on resume — exact

Root-cause why the startup pacing floor starves `plan_fec_parity`. Facts already established:

* `plan_fec_parity` runs unconditionally each turn (`schedule_transmit`), after DATA/recovery.
* It drains `pending_fec` only while `pacer.try_send(p.size)` succeeds.
* In this scenario there is **no** `AppData` — only `Timer::Pto` ticks — so parity is the only
  pending work and cannot be starved by DATA competition.
* The floor **raises** `send_rate`, which should increase tokens, so the mechanism is
  counter-intuitive and must be measured, not assumed.

Instrument `pacer.tokens()/rate()/burst_cap()` per tick with the floor ON vs OFF in that scenario and
identify which one blocks `try_send`. Prime suspect to check first: `set_burst_cap` clamping
(`tokens = min(tokens, burst_cap)`) interacting with the floor-driven rate, since parity size (64 B
symbols → sealed units) versus a channel-derived cap decides whether a token is ever available.

Do **not** fix this by disabling FEC, relaxing the parity path, changing the floor formula, or adding
a tick.

---

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

`reports/d1-exit/data/cold-start/timeline-{65,300,1200}B.csv` plus untracked user docs/logs
(`docs/design/*`, `docs/tasks/*`, `reports/d1-exit/data/*.log`,
`reports/d1-exit/data/netem-a2/console.txt`, `crates/adaptive-transport/src/fate_estimator.rs`,
`crates/kama-integration/{examples/fs_cal2_runner.rs,tests/common/fs_cal2_*.rs}`,
`reports/fs-cal-2/`, `scripts/fs-cal-2/`).

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
