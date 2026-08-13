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
| M1 baseline liveness | **PASS**, merged; post-merge smoke green |
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


---

# M2 — DR-CS-3 (IN PROGRESS)

`main` = `3e6dfa30` · branch ahead with M2 work · no push/PR.

| step | status |
|---|---|
| 0 closure (`fs_cal2_gen` + `dr_cs3_panel` + runner) | **DONE** `e39b587e`, gate **12/12** |
| 1 C2 contract frozen | **DONE** `f742afd9` — md `0abbb114…`, json `353311bf…`; **committed bytes match the frozen hashes**, and committed **before** the harness change |
| 1b V1 verdict bounded | **DONE** `f742afd9` |
| 2 V2 channel implemented | **DONE** `595814ba` |
| 3 structural tests (8) | **NOT STARTED** |
| 4 gate after V2 change | **NOT RUN** |
| 5 panels | **NOT STARTED** |

## V1 verdict — bounded

`SUPERSEDED — INVALID FOR DEFAULT PROMOTION DUE TO TREATMENT-ONLY CHANNEL IMPAIRMENT`.

Proven only: baseline ACK and treatment Echo saw different reverse semantics; per-class streams gave
classes independent state; therefore the comparison was not comparable. **Not** claimed: that C2
explains rep14, that the mechanism is 3/3, or that rep14 will recover. Diagnostic prior (not a gate):
if rep14 still fails under V2, V1 stays invalid and §4 A0–A3 finds the additional cause.

## V2 channel as implemented

`GeChain` = one state per **direction**, advanced once per traversing packet, chronologically.
Forward: DATA + ARQ retransmits + Probe. Reverse: ACK + Echo. No per-class stream, no treatment-only
impairment. GE stays packet-domain; no time-decay conversion. Scripted overrides advance the shared
chain unconditionally so they cannot desynchronise state. New observations: `acks_reverse_dropped`,
`forward_steps`, `reverse_steps`, `outcome_class` ∈ {RTT-OPPORTUNITY, NETWORK-CENSORED, NO-BOOTSTRAP}.

## Remaining structural tests (all 8 required before any panel)

1. DATA→Probe→DATA consumes the forward sequence in order
2. ACK→Echo→ACK consumes the reverse sequence in order
3. changing packet class creates no separate GE state
4. same chronological prefix ⇒ same draws in baseline and treatment
5. on a treatment-free prefix, bootstrap ON/OFF channel state is identical
6. echoes 6/12/24 s apart synthesise no consecutive treatment-only draws
7. NETWORK-CENSORED classified separately but kept in the denominator
8. a negative fixture rebuilding V1 semantics FAILS the structural test

## gate.sh negative controls (§5, one-time)

compile-error control: **exit 1, step 1 FAIL** — a cargo error is not masked by the pipeline.
Failing-test and dirty-worktree controls: **pending**.

## Local build note

The DIRTY worktree holds untracked FS-CAL files (`fs_cal2_spine.rs`, `fs_cal2_schema.rs`,
`fs_cal2_runner.rs`) referencing `adaptive_transport::fate_estimator`, which M1 un-declared. They do
not exist in a clean worktree and do not affect the gate; they are deferred to M5 per E2. Local full
builds must be per-target or in a clean worktree.


---

## M2 — V2 semantics clarification (recorded)

Correct packet-domain meaning, as implemented:

* every real wire packet advances the shared GE state of its direction **exactly once**;
* four real Echo emissions consume four reverse draws — that is correct, not a defect;
* elapsed time alone never produces a draw;
* **the V1 defect was not that four echoes consumed four draws.** It was that Echo consumed them on
  a treatment-only stream separated from ACK;
* ACK and Echo share one chronological reverse stream; DATA, retransmissions, Probe and every other
  real forward datagram share one chronological forward stream;
* once the treatment emits an extra Probe, channel state after that point may legitimately diverge
  from the baseline — that is real packet-domain physics. Only the identical traffic **prefix before
  the treatment** must match bit for bit.

The frozen C2 contract is already consistent with this; it was **not** modified.

### Pairing language (reporting, not a gate)

A single chronological chain means the treatment arm does **not** consume the same draw sequence as
the baseline, because its extra Probes insert draws. This is fair by construction, but results must
**not** be described as "paired comparison on the same trace". The correct phrasing is: *shared seed,
bit-identical pre-treatment prefix, real packet-domain divergence thereafter.* The C2 gates are
absolute (locked 3/3, fresh ≥38/40), so this is not blocking.

## §2 remaining structural items — DONE

| item | how it is pinned |
|---|---|
| 2.1 chain cardinality | `forward_chain_key` / `reverse_chain_key` exposed and asserted canonical + distinct; combined with the two accounting identities, exactly one chain per direction accounts for every datagram |
| 2.2 same-timestamp tie-break | forward indices are dense and in emission order; `drain_bootstrap_probes` runs LAST so DATA takes strictly earlier indices than a same-turn Probe; reproducible across runs |
| 2.3 datagram inventory | `TransmitKind` = {Data, Retransmit, Duplicate, FecParity, CsProbe}. First four traverse the forward chain; **FecParity is now an explicit arm with a `channel_bypass_violations` counter asserted 0** — the silent `_ => {}` catch-all is gone |
| 2.4 default configuration | the structural test file carries no `cfg(feature)` gate, so all of it runs in the DEFAULT build; a test asserts the shipped defaults (startup pacing ON, bootstrap OFF) |

## §3 rep14 reverse draw-position pre-analysis (BEFORE any panel run)

From the frozen record `preflight--P0-tail-OFF--rep14.json`, by inspection — no new run:

```
ack_batches            = 0
first_ack_ns           = None
delivered_src_at_epoch = 0
```

**Zero real ACKs traverse the reverse direction in rep14**, because no DATA was ever delivered.
This is the `0` branch: on the shared reverse chain the echoes occupy draw positions `0,1,2,3` —
the *same positions* a private chain would give them. Sharing the reverse chain with ACK therefore
does **not** shift rep14's echo draw positions.

So V2's effect on rep14 comes only from:

1. **the forward chain** — Probes now share DATA's chain, so which probes survive the forward path
   changes; and
2. **the reverse model itself** — in the **primary continuity panel** the reverse direction is
   lossless for *both* ACK and Echo, so rep14's V1 failure mode (4/4 echoes reverse-dropped) is
   **structurally impossible** there. Its outcome depends entirely on whether at least one Probe
   survives the forward chain.

In the **bidirectional robustness panel** the reverse direction is impaired for both classes; with
zero ACKs the echo positions are again `0..3`, but on a *different key* (`V2Reverse`, not the V1
`DRCS3EchoReverse`), so the realised bits differ.

**This changes no gate.** It only sharpens, before results exist, what may be concluded: if rep14
recovers in the primary panel that is largely attributable to the reverse model, not to proof that
the mechanism handles reverse loss; and a bidirectional censored outcome remains `NETWORK-CENSORED`
in the denominator.

## Cost note

`gate.sh` is now ~355 s because the structural tests drive real panels. Recorded as a cost item for
later milestones, not a defect.

---

# M2 — V2 PANEL FINDING (§4 closure, no seed search)

## Verdict on C2 gate 1

`UNEVALUABLE — FROZEN CHALLENGE PHENOTYPE NOT REPRODUCED UNDER V2`

Not PASS, not FAIL. The current result is **no evidence about bootstrap efficacy**. M3 stays blocked.

## §0 Empty-pass rule (recorded before further results)

Gate 1 came out unevaluable *visibly*. The same structure can produce an **invisible** empty pass on
the other gates: if the BASELINE (OFF) arm already meets a gate, the treatment arm meeting it says
nothing about the mechanism. Therefore, for every panel gate, without changing any threshold:

> **POSITIVE CONTROL** — the panel's OFF arm is reported against the same threshold. If the OFF arm
> meets the gate on its own, that gate is labelled **`MECHANISM-UNEVALUABLE (NON-HARM ONLY)`**, is
> not counted as PASS, and is not used as a default-ON justification.

Binding in particular for `fresh primary N=40 · first valid ACK ≥ 38/40`.

## §2 Key-domain lineage — measured, not assumed

V1 forward draws came from `stream_key(domain, cell, rep, InitialSource)` =
`…|rep{N}|InitialSource`. V2 uses `…|rep{N}|V2Forward`.

First 20 forward drops (`D` = drop), computed directly from the frozen generator:

| rep | V1 `InitialSource` | V2 `V2Forward` |
|---|---|---|
| 9 | `DDDDDDDDDDDDDDDDDDDD` | `....................` |
| 14 | `DDDDDDDDDDDDDDDDDDDD` | `....................` |
| 17 | `DDDDDDDDDDDDDDDDDDDD` | `..........DDDDDD....` |
| 5 | `DDDDDDDD.....DDDD...` | `....................` |
| 16 | `DDDDDDD.............` | `DDDDDDDDDD..........` |
| 25 | `DDDDDD..............` | `....................` |
| 38 | `DDDDDDD.............` | `....................` |
| 39 | `DDDDDDDDDDDDDDDDDDD.` | `DDDDDDDDD...........` |

**Attribution: KEY RE-DOMAINING, not packet interleaving.** Under the new key the first 20 forward
draws for rep9/14/17 are already all-good, so the flow never enters the stall phenotype — this
happens *before* any retransmission or Probe could interleave. Interleaving perturbs the sequence
only after recovery traffic starts, so it cannot explain a phenotype that is absent from draw 0.

(It also explains rep16: `DDDDDDDDDD` under V2Forward — the only locked rep that still stalls, and
only in the bidirectional panel where reverse loss compounds it.)

## §2c Key-selection guard — ruling

The C2 contract clause 5 says only: *"A distinct RNG stream per direction is permitted."* It does
**not** name `V2Forward`. Re-using the `InitialSource` key for the forward direction would be
equally contract-conformant.

Both keys conform ⇒ per §2c the choice **may not** be made on "which one brings the old stalls
back". **The current key is kept** and the §5 preregistered replacement path is the only route to a
new challenge panel. No key change is made here.

## §2b Chain initial state

`GeChain::new` sets `bad = false`, but `next_drop` at `i == 0` uses the **stationary** probability,
so the initial state is **seed-derived**, not fixed-good:

```
P(first packet already bad)   = p                = 0.200
P(good -> bad) per packet     = pgb              = 25 000 ppm = 0.0250
P(stay bad)                   = 1 - 1/L          = 0.900   (mean bad run L = 10 packets)
PreConverge window            ~ 20 packets       (31 B/s floor, 101 B unit, 65 s)
P(bad run covers all 20 | bad at start)          = 0.1351
P(start bad AND run covers 20)                   = 0.0270
```

⇒ per-seed OFF-stall rate ≈ **2.7 %** (lower bound; ignores paths that enter bad after draw 0).
⇒ a finite universe of ≈ **371 seeds** is indicated to expect ≥ 10 OFF-stalls. This sizes §5's
universe; it changes no gate or threshold.

## §6 Bidirectional claim limit (recorded before results)

All nine C2 gates are keyed to the **primary** panels. The bidirectional robustness panel is
reported separately and **has no gate**. rep14's real V1 failure was a reverse-loss phenomenon,
which is structurally impossible in the primary panel (reverse lossless). So reverse-loss behaviour
will be measured but not gated.

1. **CLAIM LIMIT.** A default-ON decision taken on the primary gates means *"bootstrap is safe and
   effective under the primary continuity model"*. It does **not** mean *"bootstrap survives reverse
   loss"*.
2. **PRESENTATION REQUIREMENT.** The bidirectional result is presented in the **same report**, beside
   the default-ON recommendation. If it shows weakness it is recorded by name as an M7 backlog item,
   never passed over silently.

## Still open in M2

§1 gate at the panel commit (`de5119c0`) · §3 zero-probe paired invariant (6 reps) · §4 displacement
metric split (`scheduler_data_displacement` vs `channel_fate_divergence`, `N/A — NO ECHO` instead of
`u64::MAX`) · §5 preregistered replacement panel (universe + phenotype + OFF-only selection + seed-set
hash before any ON run).

Panels 1–2 are retained as **DIAGNOSTIC / UNGATED**.

## §1 gate at the panel commit — SATISFIED

`gate.sh` at `de5119c0` (the exact commit the panels ran on): **12/12 PASS**
(492 / 539 / 492 / 602 / 846, clippy clean ×5, worktree clean). Panels 1–2 are therefore **gated
evidence**, not ungated diagnostics. Their *interpretation* is unchanged: gate 1 stays
`UNEVALUABLE`.

## §3 zero-probe paired invariant — PASS, 0 violations

Six locked-primary P0 reps where bootstrap never armed (`probes=0`, `echoes=0`):

| rep | OFF pc | ON pc | OFF/ON tx | OFF/ON acks | wire digest | src prefix |
|---|---|---|---|---|---|---|
| 5, 9, 14, 17, 25, 38 | 1000 ms | 1000 ms | 384 / 384 | equal | **equal** | **equal** |

With `Probe=0` and `Echo=0` the arms are bit-identical on traffic and channel digests. No
nondeterminism, no treatment-before-arm effect.

## §4 displacement metric — split and frozen

Contract `DR-CS-3-DISPLACEMENT-METRIC-CONTRACT` md `61d64d55…` / json `9c32668a…`, hashed **before**
any new measurement.

The old single number was **not authoritative**: it used `first_echo_tick`, and with no echo the
horizon became `u64::MAX`, diffing two whole trajectories. That is how runs whose treatment emitted
at most 3 probes reported `disp=376..382`. Those readings were an instrument artefact, **not** DATA
displacement.

Replaced by two quantities split at the **first Probe on the wire** — the true boundary between
"arms must be identical" and "divergence is legitimate":

| metric | window | expectation | role |
|---|---|---|---|
| `scheduler_data_displacement` | ticks `< first_probe_tick` | **must be 0** | safety-gate input (C2 gate 5) |
| `channel_fate_divergence` | ticks `>= first_probe_tick` | non-zero is normal | descriptive only, never a gate, never called displacement |

Echo-bounded windows now report **`N/A — NO ECHO`** instead of silently spanning the trace.

Positive controls PC1–PC4 written (pre-treatment divergence 0 · no extra packets ⇒ whole-trace
identical · one Probe advances the shared chain exactly once · the two quantities never conflated);
execution in flight.

## Still open in M2

§4 positive-control results · §5 preregistered replacement panel (finite universe sized by the
2.7 % rate ≈ 371 seeds · exact OFF-stall phenotype · OFF/control-only selection · deterministic
first-K rule · **selected seed set hashed before any ON run** · original 3/3 strictness · fresh N=40
and zero-new-stall gates unchanged · separate confirmation set).

rep16's locked-bidirectional stall stays **diagnostic only** — no product-failure claim without its
exact Probe/Echo authentication and loss lineage.

## §12.5 behaviour-neutrality — PASS (retroactive for `b2c7f120`)

Locked panels re-run at **both** `de5119c0` and `b2c7f120`, same seeds/domain/parameters, primary and
bidirectional, OFF and ON arms:

```
records compared : 32
fields compared  : 18 per record
field mismatches : 0
missing records  : 0
```

Compared: `wire_digest` · `source_prefix_digest` · terminal classification · tx DATA/retransmit/Probe ·
ACK/Echo counts · PreConverge exit · directional draw counts. New event-log fields are excluded from
the digests by design.

⇒ `b2c7f120` is **observation-only**; the panel 1–2 evidence gated at `de5119c0` is **not stale**.
This check is now mandatory for every future `dr_cs3_panel.rs` instrumentation commit.

## §4 metric closure — corrected contract, implementation still owed

Superseded: `DR-CS-3-DISPLACEMENT-METRIC-CONTRACT` (md `61d64d55…`) —
`SUPERSEDED — emission ticks were mislabeled as channel fates and gate-5 scope was not grounded`.

Replacement: `DR-CS-3-DISPLACEMENT-METRIC-CONTRACT-V2` md `e4e4212a…` / json `3c5d1864…`.

**Gate 5 re-grounded.** Frozen text is *"safety / budget / invariant violations: 0"*; C2 §4 puts
**DATA displacement** inside `SAFETY-FAILURE`. So its scope is the **whole run**. The earlier
narrowing to `ticks < first_probe_tick` made it structurally zero and silently redefined a frozen
gate — **withdrawn**.

**Withdrawn claim:** *"a treatment emitting ≤3 probes cannot displace 380 DATA emissions"* →
`SUPERSEDED — unsupported causal inference`. A few Probes can re-phase the shared GE state and, via
pacing and ACK feedback, produce long downstream divergence. The 376–382 figures remain
non-authoritative **because of the `u64::MAX` horizon**, not the probe count.

**Still owed before any selection run:**

1. directional event log (draw index · emission order · packet kind · DATA identity + attempt
   ordinal · fate · override source), excluded from digests;
2. `channel_fate_divergence` recomputed over **matched attempts only**, reported with
   `matched_attempts` / `matched_fate_divergences` / `unmatched_baseline_attempts` /
   `unmatched_treatment_attempts` / `trajectory_divergence_start`;
3. `scheduler_data_displacement` measured from the **arbitration record**, whole run;
4. non-vacuous controls, including **one fixture that produces displacement** so the oracle is shown
   catching it; assertion and fixture-activation counts reported;
5. §12.6 finite run budget frozen before the 512-seed selection.

The current `channel_fate_divergence` still measures emission-schedule differences and **must not be
reported under that name** until (2) lands.
