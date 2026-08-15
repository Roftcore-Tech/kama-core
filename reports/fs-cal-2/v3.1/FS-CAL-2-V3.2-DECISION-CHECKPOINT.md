# FS-CAL-2 v3.2 — §4.2b FAILURE DIAGNOSIS + DECISION CHECKPOINT

> Normative source: `docs/design/FS-CAL-2-EXPERIMENT-DESIGN-v3 (1).md`
> sha256 **`115fa266a40f744013d209d36711a698096a28509ede11dd417f23aa5cdb3393`** — VERIFIED exact.
> Scope of this turn: (1) ARQ-contract verification, (2) cold-start root-cause diagnostic,
> (3) telemetry/window feasibility, (4) v3.2 decision checkpoint. **Nothing else.**
> HEAD `4aa11a93` unchanged · no commit/push/PR · user WIP untouched · 82 frozen §4.2b records byte-unmodified.
>
> ## HEADLINE
> **ROOT CAUSE = MIXED, with a CONFIRMED `PRODUCT-COLD-START-LIVENESS` component ⇒ `DR-CS-2 REQUIRED`.**
> Per the diagnosis contract, FS-CAL-2 v3.2 is **NOT** to be made to pass. **YETKİ-2 NOT AUTHORIZED.**
> Recommended: **PATH-B** (fix the product first). Reasoning in §10.

---

## 1. ARQ tail ON/OFF — definitive proof → **ARQ-CONTRACT-PASS**

The frozen ARQ base is genuinely **tail-capable ON**, and **identical in both arms**.

```text
arq_contract_id  = FSCAL2-ARQ-DEFAULTFEAT-TAILV1-v1        (fs_cal2_spine.rs:78; contract JSON:165) ✅
setting_symbol   = Sender::new() + set_tail_recovery_config(tail_on_config())
product default  = enabled: FALSE   (tail_recovery.rs:39 "DEFAULT OFF"; sender.rs:1021)
harness override = enabled: TRUE    (fs_cal2_spine.rs:82-92 tail_on_config)
```

| field | value | code |
|---|---|---|
| `enabled` | **true** | `fs_cal2_spine.rs:84` |
| `min_pto_ns` | 10 ms | `10 * MS` |
| `max_pto_ns` | 10 s | `10_000 * MS` |
| `initial_pto_ns` | 1 s | `1_000 * MS` |
| `max_backoff_pto_ns` | 30 s | `30_000 * MS` |
| `max_ack_delay_ns` | 25 ms | `25 * MS` |
| `max_tail_probes` | 3 | `max_tail_probes: 3` |

7/7 field-for-field verbatim identical to the frozen product config `dr_arqtail_product.rs:36-46`.

**Identical across arms — structural proof.** `tail_cfg` is bound once (`fs_cal2_spine.rs:309`) and applied
at `:363`, immediately after `Sender::new()` (`:362`) — **355 lines and one full loop-entry before the sole
`match &cfg.arm` at `:718`**. `cfg.arm` is referenced in exactly one executable location in the file. The only
arm-dependent sender mutation is `set_manual_fec_enabled` (`:540`), whose body (`sender.rs:2415-2427`) writes
**only** `self.fec_mode`. `TailRecovery` exposes no config mutator; its only writers are `sender.rs:1021`
and `:1146-1150`. ⇒ effective tail config in `STATIC_R0` ≡ `STATIC_R2`.

**Runtime assertion (not just code reading).** `fs_cal2_pins.rs:190 terminal_tail_r0_probe_recovers` asserts a
pure terminal tail completes with `probe_sent >= 1` **in the R0 arm**. So the R0 baseline is genuinely
tail-capable — the contrast is *marginal utility of `r` on an already tail-capable ARQ*, **not** a tail-blind
straw-man. Independently reconfirmed this turn: 29/29 `fs_cal2_pins` pass with the instrumented binary.

`enabled` gates exactly 3 sites: `sender.rs:1816` (ACK-progress reset/disarm), `:1878` (PTO probe-arm),
`:3373` (`emit_tail_probe` early return). All three were live during every diagnostic run.

**Required corrigendum (artifact integrity, does NOT change ARQ identity):**
`FS-CAL-2-PRODUCT-CONTRACT-v3.json` lines 69/161 record `fs_cal2_spine.rs = cd740097dd67…3893`, but the file
was `07cdede80fde…f1d3` at freeze time (the newer `FS-CAL-2-PRE-RUN-CHECKPOINT.md:33` already records the
correct value). The spine is untracked, so the recorded version cannot be reconstructed. Re-freeze the
contract as part of the new-design-version response. 11/12 other source hashes matched exactly.

---

## 2. Estimator product-call-path ruling → **PRODUCT-ESTIMATOR-INTEGRATION-FEASIBLE**

`fate_estimator.rs` having no product caller is **(a) deliberate experiment telemetry**, not a gap that
forecloses a runtime `CALIBRATED_POLICY`. Every input it consumes is *already held, per original source
packet*, at named `&mut self` sites inside `Sender`:

| estimator input | product site holding the same value |
|---|---|
| SUCCESS (`on_original_ack`) | `Sender::on_ack_received` `sender.rs:1695-1707` — `p.unit.id` with `p.origin == TransmitKind::Data` is exactly "ACK covers an ORIGINAL transmission" |
| late-ACK / spurious correction | `sender.rs:1708-1723` (`tombstones.remove`, `t.retx_count == 0`, `t.origin == Data`) |
| LOSS lineage (`on_loss_evidence`) | `queue_losses` `sender.rs:1921-1951` (purest: the declaration itself); also `plan_arq_retransmits:3291`, `emit_tail_probe:3394`, `drain_due_duplicates:3469` — all carry `unit.id` |
| recovered-source evidence | `on_recovered_control` `sender.rs:2725-2745` |
| reordering guard | already computed in-product at `sender.rs:1878-1883` via `tail_recovery.base_pto_ns(srtt, rttvar)` |
| FEC decision hook | `open_fec_block_authority` / `close_fec_block_manual` / `plan_fec_parity` |

**What is missing is wiring plus one ordinal counter — not measurement.** Ruling: **FEASIBLE**.
(Precedent: `fusion_policy.rs` is *more* unwired than the estimator — `FusionPolicy` has no product caller at all.)

---

## 3. D1 — deterministic stall replay (8 seeds, extended 65 s budget)

Same `seed_domain=FSFC2-DIAG42B` + `cell_id` + `repetition` ⇒ byte-identical loss streams. Debugging only.

| rep | trigger | N | elig | conv | tx_data | dup | acks | PreConverge exit | class |
|---|---|---|---|---|---|---|---|---|---|
| 5 | DRAIN | 256 | ✅ | ✅ | 384 | 2 | 381 | 27.065 s | HARNESS-TIME-BUDGET |
| 9 | TIMEOUT | 0 | ❌ | ❌ | 20 | 0 | 0 | **never** | **PRODUCT-COLD-START-LIVENESS** |
| 14 | TIMEOUT | 0 | ❌ | ❌ | 20 | 0 | 0 | **never** | **PRODUCT-COLD-START-LIVENESS** |
| 16 | DRAIN | 256 | ✅ | ✅ | 384 | 0 | 382 | 23.807 s | HARNESS-TIME-BUDGET |
| 17 | TIMEOUT | 0 | ❌ | ❌ | 20 | 0 | 0 | **never** | **PRODUCT-COLD-START-LIVENESS** |
| 25 | TIMEOUT | 256 | ❌ | ✅ | 384 | 2 | 366 | 20.549 s | HARNESS-TIME-BUDGET |
| 38 | DRAIN | 256 | ✅ | ✅ | 384 | 0 | 368 | 23.807 s | HARNESS-TIME-BUDGET |
| 39 | TIMEOUT | 2 | ❌ | ✅ | 23 | 0 | 2 | 62.904 s | HARNESS-TIME-BUDGET |

**3/8 become eligible** once the budget passes their 20.5–27.1 s convergence point (the frozen budget is
18.498 s — it cut them off). **3/8 never converge even at 65 s** (> authoritative `CS_GIVEUP_NS` = 60 s).

---

## 4. D2 — fresh diagnostic (disjoint seed domain `FSFC2-DIAG-CS-V32`, N=40, 65 s budget)

```text
CONVERGED-NORMALLY           34/40   (converge at ~1.001 s — first packet survives ⇒ ACK at 1 RTT)
HARNESS-TIME-BUDGET           4/40   (converge AFTER 18.498 s)
PRODUCT-COLD-START-LIVENESS   2/40   (never leave PreConverge within 65 s)

estimator_eligible           29/40 = 0.725     ← DIAGNOSTIC ONLY, not a §4.2b re-qualification
                                                 (§4.2b bar 0.90; frozen-budget §4.2b actual was 0.475)
```

**Extending the budget alone does NOT reach the bar.** The 11 ineligible runs fail for **four independent**
reasons (a run may fail several):

| binding condition | runs | note |
|---|---|---|
| `warmup_incomplete` (delivery-drain rule) | **8** | the *most frequent* binding condition — a harness-semantics defect (§8) |
| `telemetry_stale` | 6 | ungrounded operand (§6) |
| `few_loss_transitions` (n10+n11 < 8) | **5** | reps 8/20/27 have N=256, converged, fresh — only 4–7 loss transitions |
| `N < 256` | 4 | |
| `few_nonloss_transitions` | 3 | |
| `unconverged` | 2 | the hard product stalls |

`few_loss_tr` is a **structural estimator tension**, not a bug: GE-L10's long bursts produce *few*
loss-boundary transitions, so the very structures the policy most wants to detect are the ones that most
often fail the transition floor. This is the product-path echo of the S0 observational finding
(GE-L10-p10 eligibility 0.81).

---

## 5. Cold-start root-cause class → **MIXED (PRODUCT-COLD-START-LIVENESS confirmed)**

### 5.1 The mechanism, with exact lineage

```text
PRE_CONVERGE_FLOOR_BW = 31 B/s      sender.rs:726 ("§3.3-A worst-corner prior pacing floor (B/s) ~0.25 kbps")
applied unconditionally             sender.rs:2306-2308  compute_send_rate: PreConverge ⇒ return floor
PreConverge exits ONLY on first     sender.rs:56-58, 2066-2073 ("İlk geçerli SACK'te → Startup" [K-KEYSTONE])
  valid SACK
PTO / time-threshold loss           sender.rs:1847 (guarded), :1808 — FROZEN while in PreConverge
  declaration
cold-start probe ladder + give-up   sender.rs:192-198 CS_PROBE_FIRES_NS=[3,9,21,45]s, CS_GIVEUP_NS=60s
                                    — ALL behind #[cfg(feature="experimental-cold-start")] ⇒ NOT COMPILED
                                      in FS-CAL-2's DEFAULT-feature build ⇒ no escape hatch exists
DATA-first scheduling               sender.rs:2356 before :2364/:2365 — fresh DATA consumes the tokens
                                      before §3.3-B geometric copies and the tail probe can fire
```

With a 101 B sealed unit at 31 B/s, **one packet costs ~3.26 s of token accrual**.

### 5.2 Measured confirmation (recorded counters, not inference)

| budget | predicted `tx_data` = budget×31/101 | **observed** |
|---|---|---|
| 18.498 s (frozen) | 5.68 | **6** (all 8 stall seeds) |
| 65.0 s (extended) | 19.95 | **20** (all 5 stalled runs) |

Across all 5 runs that never left PreConverge:

```text
app_enqueued        = 384   ALL   ⇒ the harness offered every unit — HARNESS-DRIVER REFUTED
ack_batches         = 0     ALL
tx_duplicate        = 0     ALL   ⇒ §3.3-B geometric copies NEVER fired
tail_probe_sent     = 0     ALL   ⇒ despite tail_probe_due = 6,400,000 probe-due decisions
state_changes       = [[0,"PreConverge"]]  — one state for the entire 65 s
inflight            = 101 → 2020 monotonic, never drains
```

**Both product cold-start recovery mechanisms were token-starved.** This independently corroborates the
DR-ARQ-PRODUCT-TAIL-1 record ("high-loss cold-start fragility — probe token-starved in PreConverge floor
pacing") and the code-derived predictor `stall ⟺ leading_drop_run ≥ 6`, which reproduces **8/8 stalls and
72/72 non-stalls** across the original §4.2b cells with zero error.

### 5.3 Classification

```text
MIXED.
  HARNESS-TIME-BUDGET          9 runs — recover at 20.5–62.9 s; the frozen 18.498 s budget is ~68× too
                                 small to survive a cold start (draining 384 units at the floor needs
                                 384 × 3.258 s ≈ 1251 s). NOTE: the budget is design-normative
                                 (design :124-126), NOT a harness deviation. It cannot CREATE a stall.
  PRODUCT-COLD-START-LIVENESS  5 runs — never leave PreConverge within the authoritative bound while the
                                 harness drives every product path.  ⇒ **DR-CS-2 REQUIRED**
  HARNESS-DRIVER               REFUTED (app_enqueued = 384 on every stalled run).
```

**Scope honesty:** the severity is *workload-conditional*. Token starvation of §3.3-B requires a
permanently backlogged `tx_queue`; an app-limited flow would drain it, `front_size` would become
`DEFAULT_MTU=1200` (`sender.rs:1249`), and copies/probes could fire. So this is **"product cold-start
liveness under a sustained bulk workload"**, not unconditionally.

### 5.4 Second, distinct pathology found (NOT cold start)

`D2 rep2` leaves PreConverge at **1.1 s** — then locks at **288 B/s** in `Startup`→`RecoverArq` and never
ramps, emitting only **63 packets in 65 s** (healthy runs reach 27,525 B/s; a 95× gap). `probe_due=388,427`,
`probe_sent=0`. `rep31` shows the same shape. This is a **post-convergence rate-ramp lock at high
loss/high RTT**, separate from the cold-start stall, and it is *not* addressed by any warm-up or window change.

### 5.5 Extended-timeout derivation (from code, not estimated)

```text
guard = RTT(1.000 s) + ceil(SEALED 101 B / PRE_CONVERGE_FLOOR_BW 31 B/s = 3.259 s) + dt(10 µs)
      = 4.259 s → rounded up to 5 s
extended_timeout = max( CS_GIVEUP_NS 60 s + guard = 65 s,
                        last probe step 45 s + RTT 1 s + guard = 51 s,
                        60 s )
                 = 65 s          (3.51× the frozen 18.498 s)
```

---

## 6. Telemetry-staleness decomposition

> **CORRECTION to the v3 checkpoint.** It recorded staleness as "real product behavior". That was wrong in
> a way that changes the decision: the **threshold is grounded, the OPERAND is not**.

`max_telemetry_age_ns = 2_000_000_000` (`fusion_policy.rs:142`) has **exactly one** comparison site
(`:351-353`, strict `>`, ⇒ `FallbackArq(StaleTelemetry)`). But the quantity it is compared against —
`FusionObservation::telemetry_age_ns` (`:76-77`) — **has no producer anywhere in the repository** outside a
`#[cfg(test)]` fixture (`:399-414`). `FusionPolicy`/`FusionSafetyValidator` likewise have no product caller.
The harness invented its own quantity and compared it to the product's constant.

| Q | answer |
|---|---|
| **Q1 — telemetry never produced?** | **Yes, for 5/48 runs (10.4%)**: zero ACK batches, `srtt` never set. Their `telemetry_stale=true` comes from the harness **zero-initialisation** of `last_ack_progress_ns` (`= 0`), not from an aged measurement. |
| **Q2 — produced but > 2 s at the decision?** | **Yes, for 5 runs** (11.6% of runs that had any telemetry); epoch ages 15.7 / 31.0 / 33.0 / 49.4 / 52.3 s. |
| **Q3 — single input or combined snapshot age?** | Harness: **single** (`now − last ACK ARRIVAL`, one assignment site). Product: **UNDETERMINED** — the doc says "newest telemetry update feeding `inputs`" over 5 heterogeneous signals (a MIN-over-ages, i.e. *permissive*, reading), but no code implements it. Under the literal reading `loss_ppm` is re-ticked at `now` on every `fuzzy_evaluate` (`sender.rs:1988`), making `StaleTelemetry` practically unreachable. |
| **Q4 — freshness refreshed after ACK/fate corrections?** | **No.** Refreshed only on a non-empty ACK batch; late-ACK and spurious-loss corrections do NOT refresh it. It is refreshed by **parity and retransmit** ACKs too, and performs **no inflight-shrink progress test despite its name**. |
| **Q5 — burst-localised or whole RTT1000?** | **Neither — two distinct populations**, not one gradient: (A) 5 runs with *no telemetry at all*, (B) 5 runs with genuinely *aged* telemetry. |

**A genuine product analogue exists and is unused:** `TailRecovery::last_progress_ns` (`tail_recovery.rs:67`,
written at `sender.rs:1816-1819` gated on `inflight` shrinking) is a real ACK-**progress** timestamp with an
explicit progress predicate — but it is not publicly readable (`tail_probe_diag*` expose counters only).

**Clock-class note:** `max_telemetry_age_ns` is a **fixed wall-clock** bound in an engine that deliberately
moved everything else to channel-derived clocks (`rtt.rs:61-63`, `:117-118`, `:123-124`). That is precisely
this repo's recorded recurring bug class. **Value NOT changed this turn** (forbidden, and correctly so).

---

## 7. Window feasibility table — OUTCOME-BLIND

Inputs: the identical frozen S0 traces (`FSFC2-S0`) with the pure `features_from_window` core recomputed over
the last W bits, plus the FSFC2-DIAG-CS-V32 fate-known time series. **No calibration outcome, no R0/R2
completion difference, no treatment effect.**

| W | min AUC lo95 | all AUC ≥ 0.90 | max abs bias (ppm) | mean SD (ppm) | MBD coverage (pinned thr.) | state bytes | **reach fraction** | median time-to-W |
|---|---|---|---|---|---|---|---|---|
| 64 | 0.9771 | ✅ | **36,888** | 126,251 | **0.294** | 576 | **0.8333** | 7,850 ms |
| 128 | 0.9979 | ✅ | 23,172 | 96,517 | 0.814 | 1,152 | **0.8333** | 9,400 ms |
| 192 | 0.9987 | ✅ | 15,681 | 81,666 | 0.992 | 1,728 | **0.8333** | 10,300 ms |
| 256 | 0.9989 | ✅ | **10,782** | 71,902 | **0.996** | 2,304 | **0.8333** | 11,000 ms |

**Decisive: `reach_fraction` is IDENTICAL (0.8333) at every W.** The runs that fail to reach 256 fate-known
also fail to reach 64 — because they have ~**zero** fate-known, not "slightly under 256". Shrinking the window
buys **no** eligibility coverage and only 3.15 s of median time-to-ready, while costing **3.4× bias**,
1.75× variance, and collapsing MARKOV-BD threshold coverage from 0.996 to 0.294.

⇒ **Window reduction is NOT a remedy for the §4.2b failure.** Prefix-causality is unchanged by W (shrinking
removes OLD fates, never adds future information); cost is linear and negligible at every candidate.
**No window is selected here** — any change requires a new design version frozen BEFORE data, plus fresh S0
and fresh §4.2b.

---

## 8. Harness warm-up semantics — decision + rationale

**Implemented (as-run):** `warmup_complete = drained && !timed_out`, where `drained` = **all 384 warm-up units
DELIVERED** (`fs_cal2_spine.rs`).
**Design (§2.3 + §3.1 + line 130):** warm-up completion is defined by **256 FATE-KNOWN** — a lost packet's fate
finalizes on loss-declaration + reordering guard, *without requiring delivery*.

**These were not chosen by "which gives higher eligibility."** The correct semantics is fixed by *when the real
product could make its policy decision*: a product decides at the **decision-bearing block boundary**, using a
snapshot of *already-finalized fates*. It cannot wait for every warm-up unit to be delivered — delivery of
warm-up traffic is not a precondition for a redundancy decision on the *next* block. So **fate-known-ready is
the design-correct trigger**, and the implemented delivery-drain rule is a **harness defect**.

**Measured timing gap** (all runs reaching 256 fate-known; fate-known-ready is earlier in **every** run):

```text
median  1,767 ms earlier      min  669 ms      max  53,804 ms
```

**v3.2 must define these as SEPARATE timestamps/flags** (currently conflated into one boolean):

```text
t_warmup_end            last warm-up unit OFFERED (offer-pacer completion)
t_delivery_drain        all 384 warm-up units delivered/recovered      [current, WRONG as a gate]
t_fate_known_ready      |fate-known| first reaches the window size     [design-correct trigger]
t_estimator_snapshot    instant the pre-treatment snapshot is frozen
t_decision_block        first authoritative bulk FEC block boundary after the snapshot
```

**Not applied to the existing dataset**, per instruction. **No new §4.2b dataset produced.**
**LIMITATION (recorded honestly):** the transition counters (`n10+n11`, `n00+n01`) were **not** sampled at
`t_fate_known_ready` — only N, convergence and telemetry age were. A v3.2 semantics change therefore still
requires a dedicated fresh re-qualification run; the counterfactuals in §9 are diagnostic, not a verdict.

---

## 9. Diagnostic counterfactuals — **NOT authorization to change anything**

Cumulative, on D2 (fresh, N=40). §4.2b bar = 0.90.

| scenario | eligible | | 
|---|---|---|
| frozen budget (the actual §4.2b P2 result) | 0.475 | FAIL |
| + extended 65 s budget (as-run D2) | **0.725** | FAIL |
| + fate-known-ready warm-up semantics | 0.750 | FAIL |
| + staleness removed *(hypothetical)* | 0.825 | FAIL |
| + loss-transition bar 8→4 *(hypothetical)* | **0.900** | exactly at bar |

Marginal effect of each change **alone**: fate-known-ready **+1**, staleness removed **+0**, transition bar **+3**.

**No single change reaches 0.90.** Only the full cumulative stack does — and two of its four components
(lowering the transition bar, removing staleness) are explicitly forbidden. This is the strongest quantitative
argument against trying to make v3.2 pass by tuning.

---

## 10. PATH-A vs PATH-B — exact scope and cost

### PATH-A — narrow supported domain

**Runtime support predicate (must be runtime-observable; §3.4b forbids configured labels).** The predicate
**cannot** be "RTT == 1000" — that is a configured label. The only admissible mechanical predicate is the one
already implemented: the §3.4 eligibility interlock over the *observed* snapshot
(`N == 256 ∧ n10+n11 ≥ 8 ∧ n00+n01 ≥ 32 ∧ warmup_complete ∧ ¬stale ∧ converged`) plus the §3.4b support bins
over observed `(measured_loss_ppm, burst_delta_ppm)`. Ineligible ⇒ `R0 / ESTIMATOR_INELIGIBLE`.

**R0 fallback interlock:** already exists and is proven (`calibrated_r0_equals_static_r0_wire_exact` — the
calibrated arm falling back to R0 is **wire-identical** to STATIC_R0).

**Exact row-budget cost** (recomputed from the manifest generator):

| phase | cells | arms | N | rows | RTT1000 cells | rows dropped | rows kept |
|---|---|---|---|---|---|---|---|
| PILOT | 8 | 2 | 50 | 800 | 4 | 400 | 400 |
| TRAINING (main) | 23 | 2 | 200 | 9,200 | 0 | 0 | 9,200 |
| TRAINING (rtt) | 14 | 2 | 200 | 5,600 | 7 | 2,800 | 2,800 |
| HOLDOUT_INPROC | 36 | 3 | 200 | 21,600 | 18 | 10,800 | 10,800 |
| HOLDOUT_NETNS | 14 | 3 | 50 | 2,100 | 7 | 1,050 | 1,050 |
| **TOTAL** | | | | **39,300** | | **15,050** | **24,250** |

⇒ **24,250 rows; 15,050 dropped (38.3% of the budget).**

**Product coverage lost — stated plainly.** RTT≈1000 ms is exactly the long-haul / BLOS / satellite-relay
regime that is a *stated target* for this product (hostile networks, UAV/BLOS, tactical relay). PATH-A does
**not** make the product work there — it only stops us **claiming** it. The claim boundary becomes:
"calibrated proactive redundancy is validated only where the runtime estimator is eligible; elsewhere the
system degrades mechanically to the frozen tail-capable ARQ baseline."

**Honesty caveat that weakens PATH-A:** at RTT=1000, **34/40 fresh runs converge normally**. The failures are a
*tail*, not the regime. Excluding the whole RTT cell is a blunt instrument that discards a mostly-working
region, and it leaves both product defects (§5.1 cold-start lock, §5.4 rate-ramp lock) unfixed and unmeasured.

**Fresh validation plan:** new design version → fresh S0 → fresh §4.2b on the narrowed cells → only then YETKİ-2.

### PATH-B — high-RTT support first (**RECOMMENDED**)

FS-CAL-2 waits. The separate product work runs first:

1. **DR-CS-2 (cold-start liveness)** — **REQUIRED** by the diagnosis contract (§5.3). Scope from this
   diagnostic: PreConverge floor 31 B/s vs a 101 B unit; exit only on first SACK; `CS_*` machinery
   feature-gated OFF in the default build; DATA-first scheduling starving §3.3-B copies and the tail probe
   (`probe_due=6.4M`, `probe_sent=0`).
2. **RTT-relative freshness** — a separate safety design/DR for `max_telemetry_age_ns`: today a grounded
   threshold with an **unimplemented operand** (§6), and a fixed wall-clock bound in a channel-clocked engine.
   `FusionObservation::telemetry_age_ns` needs a real producer before any staleness gate is meaningful.
3. **Post-convergence rate-ramp lock** (§5.4) — separate from cold start; not fixed by warm-up/window changes.
4. **Estimator v2** *only if* (1)–(3) leave a real gap. §7 shows window reduction is **not** the lever; the
   open structural tension is the transition floor vs GE-burst structure (§4).
5. Then: product-path validation → new FS-CAL v3.2 → fresh S0 + fresh §4.2b.

**`max_telemetry_age_ns = k × srtt` MUST NOT be implemented silently inside FS-CAL** — it is a product safety
parameter change requiring its own design.

---

## 11. Files changed + tests

```text
MODIFIED (harness, additive observation-only):
  crates/kama-integration/tests/common/fs_cal2_spine.rs   DiagOpts / DiagSample / DiagTimeline;
                                                          FlowCfg.diag: Option<DiagOpts>;
                                                          FlowOutput.diag; capture blocks; FATE_WINDOW_LEN;
                                                          state_name(); warm-up timeout override
  crates/kama-integration/examples/fs_cal2_runner.rs       + "diagcs" mode (D1/D2), + "s0win" mode,
                                                          serialize_diagcs_record()
  crates/kama-integration/tests/fs_cal2_pins.rs            diag: None (1 literal)
  crates/kama-integration/tests/common/fs_cal2_schema.rs   diag: None (2 literals)
NEW (tools):
  scripts/fs-cal-2/fs_cal2_diagcs_manifest.py     immutable FSFC2-DIAG-CS-V32 manifest + timeout derivation
  scripts/fs-cal-2/fs_cal2_diagcs_analyze.py      root-cause + telemetry decomposition + semantics timing
  scripts/fs-cal-2/fs_cal2_window_feasibility.py  outcome-blind window table
NEW (artifacts, separate namespace):
  reports/fs-cal-2/v3.1/diag-cs/{FSFC2-DIAG-CS-V32-MANIFEST.json, d1/, d2/, invariance-control/,
                                 DIAG-CS-V32-ANALYSIS.json}
  reports/fs-cal-2/v3.1/window-feasibility/{S0-WINDOW-FEATURES.csv, WINDOW-FEASIBILITY.json}
  reports/fs-cal-2/v3.1/FS-CAL-2-V3.2-DECISION-CHECKPOINT.{md,json}
UNCHANGED: all reports/fs-cal-2/v3/** (82 frozen §4.2b records verified byte-identical);
           HEAD 4aa11a93; tracked diff still only crates/adaptive-transport/src/lib.rs; user WIP untouched.
```

**Tests / gates run this turn**

```text
cargo test -p kama-integration --test fs_cal2_pins -j2            29/29 PASS (with instrumented binary)
cargo clippy -p kama-integration --test fs_cal2_pins
       --example fs_cal2_runner -j2 -- -D warnings                exit 0
SCIENCE-INVARIANCE control (8 stall seeds @ frozen 18.498 s)      120/120 fields identical to the
                                                                  frozen §4.2b records — 0 mismatches
frozen diag-42b artifact integrity                                82/82 unmodified
```

**Defect I introduced and fixed during instrumentation (recorded, not hidden):** the first patch placed the
diagnostic block between `if epoch.is_none() {…}` and its `else if !decision_fed {…}`, breaking the chain so
the decision block was fed at t=0 instead of at the epoch. Caught immediately by 4 failing pins
(`base_source_bytes` 0 vs 808). Fixed by moving the block after the chain; 29/29 restored. This is exactly
why KALICI-KURAL-3 requires an **executable** conformance check rather than inspection.

**Hosted CI:** BLOCKED — commit/push forbidden this turn. Local scoped gates are green but are not a substitute.

---

## 12. result-bearing calibration runs = **0**

Confirmed. The 8 D1 + 40 D2 + 8 invariance-control runs are **FSFC2-DIAG-CS-V32**, namespace-disjoint, carry
`experiment_phase=DIAG_CS_V32`, `fit_use=FORBIDDEN`, `result_bearing=false`, and live under
`reports/fs-cal-2/v3.1/diag-cs/`. They are liveness/root-cause only and **must never** enter any
training / support-bitmap / holdout / parameter-fit dataset. The `s0win` sweep is a recomputation over the
existing frozen S0 synthetic traces. No calibration fit, no policy freeze, no support bitmap, no holdout.

---

## 13. **YETKİ-2 NOT AUTHORIZED**

§4.2b remains failed. A `PRODUCT-COLD-START-LIVENESS` component is **confirmed**, which per the diagnosis
contract mandates a **`DR-CS-2 REQUIRED`** checkpoint and a stop — not an attempt to make v3.2 pass.
No pilot, no training, no holdout, no netns campaign, no calibration fit, no policy freeze.
`max_telemetry_age_ns` unchanged. Eligibility thresholds unchanged. The 0.90 bar unchanged.
RTT=1000 **not** removed from scope.

---

## 14. Open items requiring an explicit user decision

1. **PATH-A or PATH-B?** (recommended **PATH-B**; PATH-A costs 15,050 rows / 38.3% of budget and narrows the
   product claim away from the BLOS/long-haul regime **without fixing either product defect**).
2. **Authorize DR-CS-2?** — scope, and whether `experimental-cold-start` (the `CS_PROBE`/`CS_GIVEUP` ladder,
   currently compiled OUT) is the intended remedy or whether the PreConverge floor / DATA-first scheduling
   priority is to be revisited instead.
3. **`max_telemetry_age_ns` ownership** — it is a grounded threshold with **no implemented operand**. Decide
   whether to (a) implement a real `telemetry_age_ns` producer, (b) make the bound RTT-relative, or
   (c) drop staleness from the v3.2 eligibility set until a product definition exists. **All three are product
   safety decisions, not FS-CAL decisions.**
4. **Warm-up semantics for v3.2** — adopt `t_fate_known_ready` as the epoch trigger and split the five
   timestamps (§8)? This is design-correct but changes the gate, so it needs a new design version.
5. **Transition-floor vs burst structure** — GE-L10 systematically yields few loss transitions (5/40 runs
   ineligible on this alone with N=256, converged, fresh). Decide whether the estimator's transition floor is
   re-specified (new design version) or whether long-burst regimes are accepted as R0-fallback by design.
6. **§5.4 post-convergence rate-ramp lock** — separate DR, or folded into DR-CS-2?
7. **Contract corrigendum** (§1) — re-freeze `FS-CAL-2-PRODUCT-CONTRACT-v3.json` against the current spine.

---

**STOP.** v3.2 is not frozen. YETKİ-2 not entered. No product safety parameter changed.
