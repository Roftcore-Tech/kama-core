# BURST-GATE DESIGN — DRAFT, NOT APPROVED FOR IMPLEMENTATION

> **STATUS: BLOCKED — 3 blocking defects, 3 major concerns found by adversarial review.**
> This document is committed as a working record, not as an approved design. Do not implement it
> as written. The review that follows the design is the authoritative part.

Design question: how does the loss-structure signal (`burst_delta_ppm`, wired observation-only in
`f54b1cdc`) enter the FEC gate, given that Q17 proved loss rate alone cannot gate FEC and Q18
confirmed the shipped controller plans zero parity at exactly the loss level where measurement says
coding is the only working mechanism.

---

All load-bearing numbers verified against source. One correction to the grounding: the 215k–240k band was fit on pooled data; honoring the CSV's `split` column, 215_000 leaks 0.61% IID false-positive on the **holdout**, 240_000 is clean on both splits. The design uses 240_000.

---

# DR-STRUCT-GATE-1 — Loss-structure gating of proactive redundancy

**Status:** DESIGN ONLY — no implementation, no source edit, no cargo run.
**Base:** `transport-baseline-closure` @ `f54b1cdc`
**Supersedes nothing.** Additive to `fusion.rs`. Does not modify FS-FUSION-CAL-1's `fusion_policy.rs`.

## 0. Problem statement and what this document decides

Q17 established on a preregistered group-disjoint holdout that **loss rate alone cannot gate proactive redundancy**: at an identical 200_000 ppm, IID structure wants coding and burst structure does not, and a rate-only boundary derived from TRAIN collapsed to a degenerate `199_999 → 200_000` gap with one harmful false positive (`reports/fs-fusion-cal-1/FINAL-REPORT.md:134`, `:125-140`). The named blocking prerequisite was a runtime loss-structure estimator (`ibid. §8.1`). That estimator now exists and is observation-only (`crates/adaptive-transport/src/fate_estimator.rs`, wired at `f54b1cdc`).

This document specifies how `burst_delta_ppm` enters the decision, what happens when it is untrustworthy, what it breaks, and the test suite that makes both outcomes forceable.

### 0.1 Where the gate lives, and why

Two engines exist. The gate goes in **`fusion.rs` (`DeterministicFuzzyController`)**, not `fusion_policy.rs`.

`fusion_policy.rs` is the more comfortable home — versioned, default-OFF, already has `FallbackReason::LowConfidence`, an independent safety validator. It is also **wired into nothing**: `grep` finds no consumer of `FusionPolicy` outside its own `#[cfg(test)]` module; only `FusionSafetyConfig` is referenced externally (`crates/kama-integration/tests/fs_cal2_pins.rs:12,179`). A gate placed there changes no product behaviour and cannot be driven end-to-end through `Sender`. That fails the project's own standard — a mechanism that cannot be exercised is the failure class this repo has shipped repeatedly.

`fusion.rs` is what `Sender::new()` actually runs (`sender.rs:1208-1211`). The cost is golden-vector exposure, and §3 discharges it algebraically rather than by re-running.

### 0.2 Blocking prerequisites — the gate cannot ship on the current estimator

These are live product defects, not design choices. Each must be fixed and pinned **before** any rule consumes structure.

**P-1 (BLOCKING). The estimator can never record a LOSS in the product.** `FateEstimator::advance()` is the only path that moves a provisional loss into `finalized` (`fate_estimator.rs:217-228`). Verified by grep over `crates/adaptive-transport/src/`: the only product references are the field decl (`sender.rs:1020`), `::new()` (`:1245`), `.snapshot()` (`:1463`), `.on_original_ack()` (`:2274`), `.on_loss_evidence()` (`:2619`). **`advance()` and `drop_unresolved_as_unknown()` have zero product callers** — every occurrence is inside `fate_estimator.rs`'s own test module. Loss evidence enters `provisional` and stays there forever.

Consequence: `diag_loss_structure()` today can only return an all-SUCCESS window. `lost == 0`, `n10 + n11 == 0`, and `eligible()` is **unconditionally false**. The estimator is not observation-only, it is inert.

The existing non-vacuity pin does not catch this: `loss_structure_observation.rs:49` asserts `f.n > 0`, and `n` counts SUCCESS entries, which finalize immediately (`fate_estimator.rs:200`). It passes on a 100%-success window.

*Fix:* call `advance(now_ns)` at the top of `Sender::on_event` for every event kind, before any snapshot, using the `now_ns` already threaded. Call `drop_unresolved_as_unknown()` only at a decision epoch. Reference wiring is `crates/kama-integration/tests/common/fs_cal2_spine.rs:1024-1026`. Pinned by **T-15**.

**P-2 (BLOCKING). Two unbounded maps — SR-AUDIT-001.** `finalized` (`fate_estimator.rs:170`) is insert-only and never pruned; `snapshot()` bounds the *view* at 256 (`:258-270`) but not the storage. `provisional` (`:173`) is drained only by the two uncalled methods. A 1-hour flow at 1000 pkt/s holds ~3.6M `BTreeMap` entries. *Fix:* prune `finalized` to `FATE_WINDOW` on insert (keeps `snapshot(&self)`), bound `provisional` by count.

**P-3. Parity ESI ids alias caller DATA ids.** `sender.rs:2611` feeds `on_loss_evidence` for every `origin != Duplicate`, which includes `FecParity`; parity is sent as `OpaqueUnit { id: u64::from(p.esi) }` (`sender.rs:3564-3572`) where ESI is a small block-relative integer. *Fix:* restrict loss evidence to `origin == Data || origin == Retransmit`. Currently masked by P-1.

**P-4. `OpaqueUnit.id` contiguity is an unstated caller contract.** A transition is counted only when `ids[i+1] == ids[i] + 1` (`fate_estimator.rs:133`). Non-contiguous caller ids ⇒ all four counters stay 0 ⇒ permanently and silently ineligible. *Fix:* state the contract in `OpaqueUnit`'s docs; pinned by **T-16**.

**P-5 (non-blocking, document it).** The spurious-loss correction arm is unreachable: loss declaration removes the packet from `inflight` (`sender.rs:2597`) before any late ACK can reach the `on_original_ack` call site (`sender.rs:2274`, inside `if let Some(p) = self.inflight.remove(&seq)`). `spurious_loss_corrections` cannot be non-zero, and `corrections()` has no `Sender` accessor. Either wire the tombstone (`:2287-2320`) / un-loss (`:2330-2354`) branches, or delete the claim. Do not leave a guard with a severed input.

---

## 1. THE RULE

### 1.1 The decision the gate actually makes

**Framing, held to throughout.** ARQ runs in every branch. Every `arq` consequent is `>= arq_light = 200`, so `arq == 0` is structurally unreachable from the controller (`fusion.rs:513-515`). The gate decides **how much proactive redundancy rides on top of an always-running ARQ**. It never selects between mechanisms. This is visible in the measured data: on the IID cells the *fusion* arm still spends 19998 / 32118 / 54035 bytes of ARQ retransmission alongside its parity (`reports/fs-fusion-cell-1/ARQ-CONTRIBUTION.csv`). Coding was added to ARQ, not substituted for it.

**Direction of the gate.** Q17's harmful false positive was *opening parity on burst* (`PRODUCT_NETNS burst/20/high` at 200_000 ppm, Pareto-preferred `ARQ_TAIL`, policy said `FecAssist`). So:

* **burst_delta LOW (IID-like)** ⇒ coding is the mechanism ⇒ **open parity**.
* **burst_delta HIGH (clearly bursty)** ⇒ ARQ already handles it ⇒ **do not open parity** (today's behaviour).

### 1.2 New type — the input is an enum, never a bare `i64`

`burst_delta_ppm` is `i64` (`fate_estimator.rs:83`) and carries no unknown sentinel. Computed exactly from the Laplace formulas (`:147-148`):

| estimator state | `burst_delta_ppm` | reads as |
|---|---|---|
| empty (`n = 0`) | `0` | *pure IID* |
| 4 clean packets | `+300_000` | *bursty* |
| 256 clean packets | `+496_109` | *very bursty* |

Both are catastrophic misreadings, and the empty case is exactly what P-1 produces today. A bare `i64` input would make the gate open parity on every cold-started flow and close it on every clean one. Therefore:

```rust
/// Loss-structure evidence at a decision epoch. `Unknown` is the IDENTITY ELEMENT of the
/// structure rule (R5 weight 0) — see §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LossStructure {
    /// Estimator ineligible (fate_estimator §3.4) — no trustworthy structure evidence.
    #[default]
    Unknown,
    /// Eligible: burst_delta from a FULL 256-entry fate window.
    Measured { burst_delta_ppm: i64 },
}
```

`FusionInputs` gains a sixth field `pub structure: LossStructure`. `FusionInputSnapshot` gains the same field (required by the replay-equality test at `sender.rs:5455`; see §3).

`Sender` constructs it at `sender.rs:2679` as:

```
let f = self.fate_estimator.snapshot();
let pt = self.path_timing_snapshot();
let converged   = self.state() != SenderState::PreConverge && pt.sample_present;
let stale       = now_ns.saturating_sub(self.last_ack_progress_ns) > MAX_TELEMETRY_AGE_NS;
let warmup_done = self.startup_round_complete;
structure = if f.eligible(warmup_done, stale, converged) {
    LossStructure::Measured { burst_delta_ppm: f.burst_delta_ppm }
} else {
    LossStructure::Unknown
};
```

The three runtime flags mirror the reference wiring at `fs_cal2_spine.rs:1028-1032`; `path_timing_snapshot()` already exists (`sender.rs:4457-4471`).

### 1.3 Domain transform — stated as explicitly as `log2_fp`

Every fuzzify path is `u64` (`Trap::mu(&self, x: u64)`, `fusion.rs:97`). `burst_delta_ppm` is signed. The transform is offset-binary, **exact and strictly monotone** (unlike `log2_fp`, which has a documented 2–4‰ staircase above `e=8`):

```
BURST_OFFSET: i64 = 1_000_000
BURST_U_MAX:  u64 = 2_000_000
burst_u(bd) = clamp(bd + BURST_OFFSET, 0, BURST_U_MAX) as u64
```

`bd = 0 → 1_000_000`. `bd = +240_000 → 1_240_000`. Achievable input range (§1.5) maps to `[15_444, 1_977_497]`, strictly inside the domain, so the clamp is a defensive bound and never active on real estimator output.

`validate()` gains `FuzzyConfigError::BurstDomain` for thresholds `> BURST_U_MAX` (`u64::MAX` exempt as shoulder sentinel), mirroring the existing `BandwidthDomain` check (`fusion.rs:287-297`). That check exists to catch "someone passed a raw signed ppm where offset-binary was expected" — the exact class of bug `BandwidthDomain` catches for bandwidth.

### 1.4 Config additions — two named `Trap`s and one consequent, no dead bands

`FuzzyConfig` gains:

```rust
/// Loss floor below which structure evidence does not open redundancy (ppm domain).
pub coding_floor: Trap,      // shoulder_high(90_000, 140_000)
/// "Not clearly bursty" membership (burst_u domain).
pub burst_iid_like: Trap,    // shoulder_low(1_240_000, 1_290_000)
/// R5 consequent.
pub fec_structure: u32,      // 800
```

Deliberately **single `Trap`s, not `VarThresholds`**. A three-band set would leave two unread bands — the `sr_med` dead-config smell already present at `fusion.rs:236` (value 750, no rule reads it). Every field added here is consumed by R5.

`validate()` additions: `TrapOrder` for both traps; `LossPpmDomain` for `coding_floor` (`<= 1_000_000`, `u64::MAX` exempt); `BurstDomain` for `burst_iid_like`; `ConsequentRange` for `fec_structure`. `covers_domain()` is unchanged and does not apply — these are not `VarThresholds`.

**R5 reuses the existing `arq_moderate = 600`.** No new ARQ consequent.

### 1.5 Threshold placement — justified from the estimator's range and from measurement

**Achievable axis.** I enumerated every reachable counter vector for a full 256-window by parameterising over (loss count `L`, maximal loss runs `r`, starts-with-loss, ends-with-loss) — these four determine `(n00,n01,n10,n11)` exactly, so this covers all 2²⁵⁶ bit strings.

```
full achievable range: [-984_556, +977_497]      both endpoints eligible
  MAX +977_497  L=106, one contiguous terminal loss run  (n00,n01,n10,n11)=(149,1,0,105)
  MIN -984_556  perfect alternation S L S L …            (n00,n01,n10,n11)=(0,128,127,0)
```

The threshold sits mid-range with ~740k of headroom above and ~1.22M below. Structurally the opposite of Q17's `199_999 → 200_000` cliff.

**Pure IID does not read zero.** Laplace's `+1/+2` prior pulls `p11` toward 0.5 far harder than `p01`, because `p11`'s denominator (`n10+n11+2`) is much smaller than `p01`'s whenever loss is a minority. IID reads *positive*, and the bias grows as loss falls. This is why the threshold cannot be placed at 0.

**Measured separation.** `reports/fs-cal-2/v3/grounding/s0/S0-FEATURES.csv`, 10,000 windows at N=256, eligible rows only. Median `burst_delta_ppm`:

| structure | p=5% | p=10% | p=20% | p=30% |
|---|---|---|---|---|
| IID | +44_721 | +10_611 | +8_528 | +1_058 |
| MARKOV-BD-b15 | +183_055 | +148_664 | +154_021 | +143_370 |
| MARKOV-BD-b35 | +355_199 | +338_864 | +349_659 | +339_218 |
| GE-L3 | +641_770 | +598_245 | +567_118 | +506_103 |
| GE-L10 | +858_190 | +854_728 | +845_935 | +837_319 |

**Correction to the grounding, and it changes the number.** The grounding derived a "215_000–240_000" band from the pooled file. The CSV carries a `split` column (7065 `train` / 2935 `val`). Honoring it:

| threshold | train IID-FP (p=10/20/30%) | **val** IID-FP | val GE-detection |
|---|---|---|---|
| 200_000 | 0.90 / 0.29 / 0.00 % | 0.61 / 0.62 / 0.00 % | 100 / 100 / 100 % |
| 215_000 | 0.30 / 0.00 / 0.00 % | **0.61 / 0.00 / 0.00 %** | 100 / 100 / 100 % |
| **240_000** | **0.00 / 0.00 / 0.00 %** | **0.00 / 0.00 / 0.00 %** | 100 / 100 / 100 % |

**215_000 leaks a 0.61% IID false-positive on the holdout. 240_000 is clean on both splits.** Threshold = **`burst_delta_ppm = +240_000`**.

**Ramp width.** `shoulder_low(1_240_000, 1_290_000)` — full membership to `bd = +240_000`, linear to zero at `+290_000`. Placed inside the measured empty gap: highest observed IID value on the holdout is `+239_177` (p=10%); lowest GE-L3 5th-percentile is `+408_730`. The ramp occupies `[240_000, 290_000]`, leaving ~118_700 ppm of margin to the nearest GE observation. The ramp — not a step — is the deliberate anti-Q17 property: see §5 T-06 for the one-permille-per-ppm behaviour at the boundary.

**Loss floor `shoulder_high(90_000, 140_000)`.** Reuses the FS-FUSION-CAL-1 calibrated breakpoints (`fusion_policy.rs:224-229`: `low_full 90_000 / low_zero 140_000`), which were fitted in the observed evidence gap between ARQ_TAIL Pareto-dominance at 20_833–83_333 ppm and FEC Pareto-dominance at 200_000 ppm. Reusing them, rather than inventing a number, keeps the two calibrations on the same evidence.

A second floor is already structural and free: eligibility requires `n10 + n11 >= 8` in a 256-window, so `LossStructure::Measured` is only ever produced above `floor(8·10⁶/256) = 31_250 ppm`. **The gate is structurally silent below 3.125% loss, at any burstiness.** State this as a property, not an accident.

**Scope limit, stated up front.** MARKOV-BD-b15 is **not separable at N=256** — detection at the 240_000 threshold is 13.5% (train) / 15.8% (val) at p=10%. b35 detects at 86% / 86%. The gate separates **IID-and-mild from GE-class burst**. It does not resolve b15-class mild burst, and treats it as IID-like — i.e. gives it parity. That is the fail-safe direction argued in §2, and it is a §6 measurement obligation, not a claim.

### 1.6 The rule table

R1–R4 are **unchanged, verbatim**. One rule is appended.

| rule | antecedent | combiner | fec | arq |
|---|---|---|---|---|
| R1 | `loss LOW` | — | `fec_zero` = 0 | `arq_light` = 200 |
| R2 | `loss HIGH ∧ bw HIGH ∧ jitter LOW` | `min3` | `fec_high` = 800 | `arq_moderate` = 600 |
| R3 | `loss MED ∧ bw MED` | `min` | `fec_med` = 500 | `arq_med` = 500 |
| R4 | `loss LOW ∧ jitter HIGH` | `min` | `fec_zero` = 0 | `arq_light` = 200 |
| **R5** | **`loss ≥ coding_floor ∧ structure IID-like`** | **`min`** | **`fec_structure` = 800** | **`arq_moderate` = 600** |

```rust
// R5: loss above the coding floor ∧ loss structure NOT clearly bursty
//     → proactive redundancy on top of the ARQ that is already running.
//     structure Unknown ⇒ burst_mu = 0 ⇒ weight 0 ⇒ IDENTITY (§3).
let burst_mu = match inputs.structure {
    LossStructure::Unknown => 0,
    LossStructure::Measured { burst_delta_ppm } =>
        c.burst_iid_like.mu(burst_u(burst_delta_ppm)),
};
let r5 = c.coding_floor.mu(inputs.loss_ppm).min(burst_mu);
rules[4] = (r5, c.fec_structure, c.arq_moderate);
```

Defuzzification, `no_rule`, `send_rate_gain`, `Mode`, and `compress` are **untouched**. `total_weight` sums five terms; a zero-weight term contributes 0 to both numerator and denominator of `weighted_average` (`fusion.rs:564-572`), so `no_rule` is likewise unchanged.

### 1.7 INV-STRUCT-MONO — a provable invariant, not just a tested one

> **For identical non-structure inputs, structure evidence can only RAISE `fec_parity_ratio_permille` and `arq_aggressiveness_permille`, never lower either.**

*Proof.* Adding a term `(w, v)` to a weighted average raises or holds it iff `v >= ` the current average. The maximum average achievable over R1–R4 is `max(fec_zero, fec_high, fec_med) = 800 = fec_structure`, and `max(arq_light, arq_moderate, arq_med) = 600 = arq_moderate`. R5's consequents equal those maxima, so R5 at any weight `w >= 0` is monotone non-decreasing on both axes. ∎

Verified numerically over 7872 grid points (loss 0–1M × 41, bw ∈ {500, 25k, 80k, 125k, 175k, 300k}, jitter ∈ {0, 50k, 100k, 450k}, bd ∈ {−900k, −240k, 0, 100k, 240k, 300k, 700k, 977_497}): **0 violations**.

Two consequences worth stating:
* R5 choosing `arq_moderate` rather than `arq_med` is not cosmetic. With `arq_med = 500`, R5 firing alongside a lone R2 (average 600) would *lower* ARQ aggressiveness to 550. Structure must never weaken ARQ. This is the design rule, not an accident.
* The gate is **strictly additive**. On burst it produces byte-identical output to today. It can never remove redundancy the current controller would have planned.

### 1.8 Worked outputs (exact, computed from the arithmetic above)

At `bw = 125_000` (`log2_fp = 4328` ⇒ `bw.high = 0`, so R2 is unreachable), `jitter = 100`, `qd = 0`, `ack = 1000`, `loss = 200_000`:

| structure | R1 | R5 | fec | arq | `no_rule` | `r` at `k=5` |
|---|---|---|---|---|---|---|
| `Unknown` | 1000 | 0 | **0** | 200 | false | 0 |
| `Measured { +8_528 }` (IID p50 @20%) | 1000 | 1000 | **400** | 400 | false | **2** |
| `Measured { +700_000 }` (GE-L10) | 1000 | 0 | **0** | 200 | false | 0 |

`fec = (1000·0 + 1000·800) / 2000 = 400`. `arq = (1000·200 + 1000·600) / 2000 = 400`.

`compute_requested_r(400, 5, r_max)` = `(400·5 + 500)/1000` = **2** (`sender.rs:537-542`). That is exactly the `r` of the FS-FUSION-CELL-1 fusion arm (`SpineArm::A0Tail { r = 2 }`) that took IID-20% from 0/12 to 5–9/12. The design lands on the measured `r` by arithmetic, not by tuning to it.

R1 co-firing at full strength halves R5's contribution — the effective ceiling in this regime is `fec_structure / 2 = 400`. This is deliberate conservatism, and it is why `fec_structure = 800` rather than 500: with `fec_med = 500` the output would be 250, which yields `r = 1`, below the measured arm.

---

## 2. ELIGIBILITY DEGRADATION

### 2.1 The rule

`eligible() == false` ⇒ `LossStructure::Unknown` ⇒ `burst_mu = 0` ⇒ R5 weight 0 ⇒ **byte-identical output to the current 4-rule controller**, including `no_rule`, `send_rate_gain_permille`, `compress` and `Mode`.

Degradation to current behaviour is by *construction*, not by convention. §3 proves it algebraically.

Eligibility is the full six-conjunct predicate at `fate_estimator.rs:100-105`, unmodified: `n == 256` **exactly**, `n10+n11 >= 8`, `n00+n01 >= 32`, `warmup_complete`, `!telemetry_stale`, `flow_converged`. These are PINNED for FS-CAL-2 v3; changing any of them requires a new calibration version and an S0 re-run (design §3.4). **This design changes none of them.**

### 2.2 Which error is worse — the measured asymmetry

From `reports/fs-fusion-cell-1/ARQ-CONTRIBUTION.csv` (12 repetitions per cell; `parity_bytes` is the per-cell aggregate):

**Cost of spending parity you did not need** — the nine tail (ExactDeficit) cells:

| cell | arm | complete | parity_bytes | total_bytes |
|---|---|---|---|---|
| rtt50-exactd1 | tail-only | 12/12 | 0 | 59388 |
| rtt50-exactd1 | fusion | 12/12 | 14544 | 72720 |
| rtt250-exactd4 | tail-only | 12/12 | 0 | 62721 |
| rtt250-exactd4 | fusion | 12/12 | 13332 | 76053 |

Parity bought **zero additional completions** — 12/12 either way in all nine cells. Cost ≈ 1111–1212 bytes per run on a ~59–63 KB baseline, roughly **+2% bytes**. And `tail_only = 0` in every cell in the grid: there is no repetition anywhere where tail-only completed and fusion did not. **Redundancy costs bytes; it never costs a completion.**

**Cost of failing to add parity you did need** — the three IID-20% cells:

| cell | tail-only complete | fusion complete | parity_bytes | fec_decoded_sources |
|---|---|---|---|---|
| rtt50-iid20 | **0/12** | 9/12 | 13332 | 69 |
| rtt250-iid20 | **0/12** | 5/12 | 9696 | 44 |
| rtt1000-iid20 | **0/12** | 9/12 | 13332 | 32 |

Tail-capable ARQ completes **0/12** in all three. The transfer does not complete at all.

**The asymmetry: ~2% bytes and provably zero lost completions, versus total failure of the transfer.** It is not close.

### 2.3 Why the ineligible path nonetheless degrades toward NOT spending

The asymmetry above argues for a spending bias. The degradation direction is the opposite. Both are correct, and conflating them is the error to avoid:

> **The asymmetry sets the threshold's bias *inside* the eligible region. It does not license acting on no evidence at all.**

Three reasons the ineligible path must degrade to today's behaviour:

**(a) Ineligibility is not structure-neutral — it is anti-correlated with the case that wants coding.** Measured eligibility fractions (`S0-FEATURES.csv`, computed directly):

| structure | p=5% | p=10% | p=20% | p=30% |
|---|---|---|---|---|
| IID | **0.944** | 1.000 | 1.000 | 1.000 |
| MARKOV-BD-b15 | 0.922 | 1.000 | 1.000 | 1.000 |
| MARKOV-BD-b35 | 0.860 | 0.996 | 1.000 | 1.000 |
| GE-L3 | **0.728** | 0.978 | 1.000 | 1.000 |
| GE-L10 | **0.564** | 0.810 | 0.986 | 1.000 |

At 5–10% loss the estimator is ineligible *most often for exactly the structure it most needs to detect* — long bursts concentrate loss into few runs, so a 256-window may contain fewer than 8 loss-adjacent transitions. Opening parity on ineligibility would preferentially open it on burst: precisely Q17's harmful false positive, reintroduced through the fallback. S0 already applied the mechanical consequence (`p05_excluded_from_support: true`).

**"Ineligible" must never be read as "probably IID."**

**(b) Today's behaviour is the measured baseline; "spend anyway" is unmeasured.** The ineligible path covers cold start, short flows, sub-3.125%-loss flows and stale telemetry. There is no campaign covering redundancy behaviour on any of them. Degrading to a measured baseline is defensible; degrading to a new untested behaviour is a claim without evidence.

**(c) The spending bias is already expressed where evidence exists.** The 240_000 threshold classifies everything below "clearly bursty" as IID-like, including b15-class mild burst it cannot resolve (13–19% detection). That *is* the asymmetry applied — it errs toward parity in the ambiguous band, where the measured cost of erring is ~2% bytes.

### 2.4 The cost of eligibility, stated plainly

256 fate-known DATA-origin originals, `>= 8` of them lost, `>= ~33` succeeded, contiguous logical ids, each loss having outlived its reordering guard (one `base_pto`; 1 s when no RTT sample exists yet). The FS-CAL-2 harness feeds 384 originals before a first admissible decision (`fs_cal2_spine.rs:62`, design §3.5) — ~38.8 KB of source at the pinned 101 B unit.

**Consequence the design owns rather than hides:** the FS-FUSION-CELL-1 workload is 48 units × 101 B. **In the very cell that motivated this gate, the window can never fill and the gate is permanently ineligible.** The gate engages on *long* flows only. That is not a defect of the design; it is the honest scope. Shortening `FATE_WINDOW` is a PINNED-constant change requiring a new calibration version and an S0 re-run (`fate_estimator.rs:35`, design §3.4), and is explicitly **out of scope here**. §6 lists it as the first follow-on question.

---

## 3. GOLDEN-VECTOR COMPATIBILITY

### 3.1 The ADR-17 argument

ADR-17 requires: same input → same output, no RNG, golden-vector pinned. The sufficient condition here is precise and checkable per-vector rather than by re-running:

> Every existing golden vector constructs `FusionInputs` directly and therefore carries **no** structure evidence, i.e. `LossStructure::Unknown` (which is also `#[default]`). `Unknown` is the identity element of R5: weight 0 contributes 0 to both the numerator and the denominator of `weighted_average`, and 0 to `total_weight`, so `fec`, `arq`, `no_rule`, `send_rate_gain_permille`, `compress` and `Mode` are all bit-identical to the 4-rule controller.

Verified numerically against every pinned output value:

| pinned vector | source | pinned output | with R5, `structure: Unknown` |
|---|---|---|---|
| `tv_fusion_b_decision_s1` (550k, 175k, 50k, 0, 1000) | `tv_fusion.rs:39-56` | fec 650, arq 550, gain 1000, `!compress`, Normal | **650 / 550 / 1000 / Normal — unchanged** |
| `tv_fusion_c_loss_decoupling` | `:58-80` | fec 650 vs 0; gain identical == 1000 | **unchanged** |
| `tv_fusion_d_emergency_trigger` (650k, 25k, 50k, 2500, 100) | `:82-96` | Emergency, gain 500, compress | **unchanged** |
| `tv_fusion_zero_weight_fallback` (700k, 500, 0, 0, 1000) | `:228-260` | `no_rule` true, fec 0, arq 200, gain 1000 | **unchanged, `no_rule` still true** |
| `q18_check` (200k, 125k) | `q18_check.rs:29-33` | fec **== 0** | **0 — the pin does NOT flip** |
| `q18_check` (200k, 175k) | `:37` | fec == 0 | **0** |
| `q18_check` (250k, 125k) | `:41` | fec > 0 (250) | **250** |
| `q18_check` (500k, 125k) | `:46` | fec > 0 (500) | **500** |
| `rule_coverage_map` anchors ×3 | `rule_coverage_map.rs:68-85` | R2-only ratio 800; `no_rule`; `emergency ∧ no_rule` | **unchanged** |
| `fuzzy_e2e` R2/R1/no_rule/partial/emergency | `fuzzy_e2e.rs:249-415` | ratios and planned parity | **unchanged** while `Sender` supplies `Unknown` |
| `fusion_policy.rs` 13 tests | `:395-658` | calibrated-policy boundaries | **unchanged — `fusion_policy` is not modified** |

**No pinned output value changes. Zero.**

### 3.2 What DOES break — stated plainly, nothing hidden

Three categories. Two are compile breaks that are *forcing functions*; one is a real semantic break requiring a replacement invariant.

**(A) Compile breaks at 14 `FusionInputs { … }` literal sites.** `FusionInputs` is not `#[non_exhaustive]` (verified) and has no `Default`. Sites: `fusion.rs:415`, `sender.rs:2679`, `fusion_policy.rs:402`, `q18_check.rs:16`, `tv_fusion.rs:44,62,70,86,124,234,251,390`, plus 2 more in the same files.

**Do not add `#[non_exhaustive]`.** It forbids literal construction *outside the crate*, which would break `tv_fusion.rs` and `q18_check.rs` (integration tests are external) far worse. Do not add `..Default::default()` either — it hides which vectors carry structure. Each of the 14 sites should gain an explicit `structure: LossStructure::Unknown`. The compiler enumerates exactly the places that must decide, and every golden vector then *visibly declares* that it carries no structure evidence. That is an audit property, not a chore.

Same treatment for `FusionInputSnapshot` (`fusion.rs:397-408`) and its `to_inputs()` (`:415`). Omitting it silently breaks `fusion_snapshot_equals_controller_input` (`sender.rs:5440-5465`), which replays the snapshot through `decide()` and asserts bit-identity with `last_decision()`.

**(B) `rule_coverage_map.rs:31-58` hand-mirrors the rule table** (R1–R4, `min3`, `weighted_avg`, no-rule fallback, emergency), because `decide()` is not decomposable from outside; the drift risk is acknowledged in its own header (`:5-9`). **R5 must be mirrored there in the same commit** or its three anchors validate a stale model. Anchor *values* do not change (all three carry `Unknown`).

**(C) `loss_structure_observation.rs` — a real break, and the correct signal.**

* `estimator_has_exactly_one_reader` (`:60-89`) asserts `snapshot_reads == 1` and **`uses.len() == 3` exactly**. Wiring `advance()` (P-1) makes it 4; routing the snapshot into `fuzzy_evaluate` changes the reader count. **This test will fail, and it is supposed to** — it is the neutrality pin for an observation-only estimator, and the estimator is ceasing to be observation-only.
* `estimator_observes_real_loss` (`:28-53`) asserts only `f.n > 0`, which SUCCESS entries satisfy. It does not detect P-1 and must be strengthened regardless.

**Re-pinning requirement — the replacement invariant must be at least as hard to fake as the one it retires.** The retiring pin is a source-text scan; a weaker successor would be a regression in rigour. Replacement (T-12, T-13):

> **Structure may influence `fec_parity_ratio_permille` and `arq_aggressiveness_permille` and nothing else.** Proved behaviourally by driving an identical `Sender` event sequence twice with structure held at both extremes and asserting that `send_rate_gain_permille`, the full `TransmitDecision` sequence (seq, size, kind, ordering), `diag_rate()`, `no_rule`, `Mode` and `compress` are byte-identical, while `fec`/`arq` differ. Plus a retained source-scan that every `self.fate_estimator` use is one of `on_original_ack` / `on_loss_evidence` / `advance` / `drop_unresolved_as_unknown` / `snapshot`, with the count pinned to the new exact value.

Amend it in the same commit as the wiring. **Do not delete it.**

**(D) `FuzzyConfig::fixture()` gains 3 fields and `validate()` gains 4 checks.** `dr_1e_validation::fixture_validates` (`tv_fusion.rs:268-272`) still passes; the four existing rejection tests (`:274-360`) are unaffected. `ValidatedFuzzyConfig::trusted_fixture()` (`fusion.rs:199-202`) still skips `validate()` — this design does **not** repair that (see §4.4).

### 3.3 Determinism

R5 adds integer `min`, one `Trap::mu` (`u128` intermediates, floor division), and one `i64 → u64` offset with clamp. No floats, no RNG, no wall clock, no iteration order dependence. `validated_determinism` (`tv_fusion.rs:380-401`) extends to the new input via T-08.

---

## 4. WHAT STAYS OUT

### 4.1 Pacing — INV-RATE-1 is preserved by construction

`send_rate_gain` is computed on an independent path (`fusion.rs:528-533`):

```rust
let sr1 = bw.low.min(qd.high);
let sr_default = PERMILLE.saturating_sub(sr1);
let send_rate_gain = weighted_average([(sr1, c.sr_low), (sr_default, c.sr_high)].into_iter());
```

It reads **only** `bw.low` and `qd.high`; the denominator is structurally `sr1 + (1000 − sr1) = 1000`, so it can never reach the zero-weight branch. Loss is already not an input to it, and neither is structure. **R5 does not appear in this expression, and no design element proposes changing it.** Keeping structure out is sufficient; nothing further is required. Pinned by T-09 (unit) and T-13 (Sender-level).

The gate's only actuation path is `fec_parity_ratio_permille` → `controller_decision_snapshot()` (`sender.rs:3192-3202`) → `fec_controller_latch` (`:3210-3304`), and `arq_aggressiveness_permille` → `self.arq.plan()` (`:3955`), which orders retransmissions. Neither touches the pacer.

### 4.2 No ARQ-vs-FEC reframing

R5 raises `fec` **and** `arq` together (§1.7). No rule in the table turns ARQ off; every `arq` consequent is `>= arq_light = 200`, making `ArqScheduler::plan`'s `== 0` suppression branch structurally unreachable from the controller (`fusion.rs:513-515`, `arq.rs:105-109`). The measured data this design rests on shows the same thing: the fusion arm spends 19998–54035 bytes of ARQ retransmission *alongside* its parity in every IID cell.

Forbidden vocabulary — `ARQ_DOMINANT`, `FEC_DOMINANT`, "ARQ vs FEC", "coding beats retransmission" — appears nowhere in this design, and must not appear in the implementation's identifiers, comments, counters or reports. The gate's noun is **proactive redundancy**.

### 4.3 The Q18 shoulder — not a precondition, and NOT made irrelevant

**Not a precondition.** R5's loss antecedent is `coding_floor`, a *new, separate* `Trap`. The existing `loss` `VarThresholds` — including `shoulder_low(200_000, 300_000)` — is untouched, R1–R4 are untouched, and the Q18 pin at `(200_000, 125_000)` still evaluates to `fec == 0` under `structure: Unknown`. The two are fully decoupled and can be fixed in either order.

**Explicitly NOT made irrelevant.** The Q18 boundary governs the entire `structure: Unknown` path, which is:
* every flow shorter than 256 fate-known originals — **including the FS-FUSION-CELL-1 workload itself** (48 units);
* every flow below 31_250 ppm loss (eligibility's structural floor);
* every cold start, every stale-telemetry window, every unconverged flow;
* per §2.3(a), disproportionately the *bursty* low-loss flows.

That is a large fraction of real traffic, and on all of it the controller still plans zero parity at 20% loss.

**Both need fixing.** This design fixes the structure axis and makes the rate axis non-blocking. Repairing the Q18 shoulder is separate work with its own campaign, its own pin update, and its own golden-vector break (`q18_check.rs:29-33` carries an exact-equality assertion whose failure message demands a citing campaign). Anyone who reads this design as closing Q18 has misread it.

### 4.4 Also out of scope

* **`trusted_fixture()` skipping `validate()`** (`fusion.rs:199-202`, shipped at `sender.rs:1208-1211`). Real defect, recorded in `reports/fs-fusion-cal-1/FINAL-REPORT.md §3`. Not repaired here; T-11 at least pins that the *new* fields validate.
* **R2's `bw HIGH` conjunct.** `bw.high > 0` requires `log2_fp(bw) > 4388`, i.e. `bw > ~150 kB/s`; the authoritative regime is ~125 kB/s (`log2_fp = 4328`), so `fec_high` via R2 is unreachable there. FS-FUSION-CAL-1 deliberately refused to change R2 without bandwidth evidence, and this design makes the same refusal. R5 does not carry a bandwidth antecedent at all — it reaches `fec = 400` at 125 kB/s *without* touching R2. This is how the design escapes cal-1's "necessary but not sufficient" outcome, and it is why R5 is a new rule rather than a modified antecedent.
* **`FATE_WINDOW`, `MIN_LOSS_TRANSITIONS`, `MIN_NONLOSS_TRANSITIONS`** — PINNED; unchanged (§2.4).
* **`fusion_policy.rs`** — untouched. Its `LossBoundary` breakpoints are *reused as values* for `coding_floor`; the module itself is not modified and its `CalibrationVersion` is not bumped.
* **`sr_med` dead config** (`fusion.rs:236`) — leave it; removing it is unrelated churn.
* **Default flip.** The gate ships behind the existing `FecActuationMode`, whose default is `Manual { enabled: false }` (`sender.rs:1267`). **No default-ON authorization is requested or implied.**

---

## 5. THE UNIT TEST SUITE

Sixteen tests. Every one states why it cannot pass vacuously — this repo has shipped counters that could not be non-zero and guards that could not fail, and P-1/P-5 are two live instances.

**The forcing lever.** `features_from_fate_bits(id0, &[bool])` and `features_from_window(&ids, &fates)` are public, pure and deterministic (`fate_estimator.rs:113-163`), every `WindowFeatures` field is `pub`, and `bits_vs_replay_equivalence` (`:474-489`) proves the bit-vector path is equivalent to full event replay. So any window — both sides of any threshold, any eligibility combination — is constructible with no network, no timers, no `Sender`.

**Forceability at `Sender` level requires four things simultaneously**, three of which are silent zeroing guards: `FecActuationMode::Controller`, `no_rule == false`, `Mode::Normal`, and a matching `event_index`. A correct gate still emits zero parity if any is wrong, and the test would prove nothing. Every Sender-level test below asserts these preconditions explicitly *before* asserting the outcome.

### Group 1 — Both outcomes forced (the whole point of the design)

**T-01 `structure_opens_parity_where_rate_alone_cannot`** — unit, `fusion.rs` (new `tests/tv_structure_gate.rs`).
*Asserts:* at the exact regime Q18 pins as broken, IID structure opens parity.
*Inputs:* `loss_ppm = 200_000`, `bandwidth_bps = 125_000`, `jitter_ppm = 100`, `qd = 0`, `ack = 1000`, `structure = Measured { burst_delta_ppm: 8_528 }` (the S0 median for IID @ p=20%).
*Expected:* `fec_parity_ratio_permille == 400` (exact), `arq_aggressiveness_permille == 400` (exact), `no_rule == false`, `Mode::Normal`, `send_rate_gain_permille == 1000`. Additionally `compute_requested_r(400, 5, 4) == (2, 2)`.
*Non-vacuity:* exact equality on a value that is 0 in the shipped controller; asserted jointly with T-02, which shares every input except structure. `r == 2` ties the output to the measured FS-FUSION-CELL-1 arm rather than to any number chosen here.

**T-02 `structure_withholds_parity_on_burst_at_the_same_loss_rate`** — unit, `fusion.rs`.
*Asserts:* **the same loss rate, differing only in structure, must NOT open parity.** This is the Q17 harmful false positive, prevented.
*Inputs:* identical to T-01 except `structure = Measured { burst_delta_ppm: 700_000 }` (S0 GE-L10 p05 @ p=20% is +708_082).
*Expected:* `fec == 0`, `arq == 200`, `no_rule == false`, gain `== 1000` — byte-identical to the shipped controller.
*Non-vacuity:* T-01 and T-02 must be written as one `#[test]` sharing a single `FusionInputs` value mutated in exactly one field, with `assert_ne!(a.fec, b.fec)` alongside the two exact-equality assertions. A gate that ignored structure fails on `assert_ne!`; a gate stuck open fails T-02; a gate stuck closed fails T-01. **No single-value bug passes this pair.**

**T-03 `sender_level_both_outcomes_through_the_fec_latch`** — Sender integration, `sender.rs` `#[cfg(test)]`.
*Asserts:* the gate reaches actual planned parity, not just a ratio field.
*Inputs:* two `Sender`s driven with identical `AppData`/`AckReceived`/`TimerFired` sequences producing ~20% loss over ≥256 contiguous-id originals; estimator fed so one ends eligible-IID and one eligible-burst (via the erasure pattern, not injection). `FecActuationMode::Controller`.
*Expected:* preconditions first — `snapshot.no_rule == false`, `!emergency`, `snapshot.event_index == fusion_event_index`; then `fec_controller_latch(...).planned_r >= 1` on the IID sender and `== 0` with `suppression == None` on the burst sender, and the IID sender's parity queue non-empty.
*Non-vacuity:* the three preconditions are asserted, so a pass cannot come from a silent zeroing guard; `planned_r` is the value that actually causes bytes on the wire.

### Group 2 — Eligibility degradation

**T-04 `ineligible_structure_is_byte_identical_to_the_four_rule_controller`** — unit, `fusion.rs`.
*Asserts:* exact degradation to current behaviour.
*Inputs:* a 5×5×4×3×3 sweep over `loss ∈ {0, 100k, 200k, 350k, 700k}`, `bw ∈ {500, 25k, 80k, 125k, 175k}`, `jitter ∈ {0, 50k, 150k, 450k}`, `qd ∈ {0, 750, 2500}`, `ack ∈ {100, 500, 1000}`, each with `structure = Unknown`, compared against a locally reconstructed 4-rule reference computed in the test.
*Expected:* full `FusionDecision` equality — `fec`, `arq`, `send_rate_gain`, `compress`, `mode`, `no_rule` — at all 900 points.
*Non-vacuity:* the reference is computed independently in the test, not read back from `decide()`. It covers the `no_rule` region (loss 700k @ bw 500) and the emergency region (loss 700k, bw 25k, ack 100), so the fallback and Mode paths are exercised, not just the common case.

**T-05 `eligibility_predicate_gates_the_structure_input`** — Sender integration.
*Asserts:* `Sender` emits `Unknown` iff `WindowFeatures::eligible(...)` is false, for each of the six conjuncts independently.
*Inputs:* six `WindowFeatures` fixtures built on `eligibility_exact_thresholds` (`fate_estimator.rs:447-471`), each falsifying exactly one conjunct: `n = 255`; `n10+n11 = 7`; `n00+n01 = 31`; `!warmup_complete`; `telemetry_stale`; `!flow_converged`. Plus the all-satisfied control.
*Expected:* six ⇒ `Unknown`; the control ⇒ `Measured`. Both `fec` outcomes follow.
*Non-vacuity:* the control case proves eligibility is reachable at all — without it, six `Unknown`s could come from a predicate hardwired to false, which is exactly P-1's failure mode.

### Group 3 — Cold start

**T-06 `cold_start_empty_estimator_does_not_read_as_iid`** — unit + Sender.
*Asserts:* a brand-new flow does not open parity, despite `burst_delta_ppm == 0`.
*Inputs:* (a) unit: `WindowFeatures` with `n = 0` — verified to yield `p11 = 500_000`, `p01 = 500_000`, `burst_delta_ppm = 0`; assert `eligible(true, false, true) == false` and that `Sender`'s mapping yields `Unknown`. (b) Sender: a fresh `Sender::new()` after a single `AppData`, at `loss_ppm = 200_000` forced through the loss window.
*Expected:* `structure == Unknown`, `fec == 0`.
*Non-vacuity:* `0` is the *most* IID-looking value on the axis. A gate consuming a bare `i64` passes T-01/T-02 and **fails here** — this is the test that forces `LossStructure` to be an enum.

**T-07 `clean_flow_walk_up_does_not_read_as_burst`** — unit.
*Asserts:* a loss-free flow's Laplace drift never reaches the decision.
*Inputs:* all-SUCCESS windows at `n ∈ {1, 2, 3, 4, 8, 32, 128, 256}`.
*Expected:* exact `burst_delta_ppm` values `{0, +166_667, +250_000, +300_000, +388_889, +469_697, +492_249, +496_109}` (computed from `fate_estimator.rs:147-148`), and `eligible(true, false, true) == false` at every one — `n10 + n11 = 0 < 8`. Sender maps all to `Unknown`.
*Non-vacuity:* **after 4 clean packets the raw value is +300_000, above the 240_000 threshold.** Pinning the exact walk-up values makes the pin fail if the Laplace prior or the window arithmetic is ever changed, and demonstrates concretely why `eligible()` — not a value comparison — is the unknown marker. Extends `laplace_no_loss_window_ineligible` (`fate_estimator.rs:437-444`), which pins `p11`/`p01` but not `burst_delta_ppm`.

### Group 4 — The boundary

**T-08 `boundary_just_inside_and_just_outside`** — unit, `fusion.rs`.
*Asserts:* exact behaviour at and around `burst_delta_ppm = +240_000`, and that the boundary is a ramp, not a cliff.
*Inputs:* base of T-01, sweeping `burst_delta_ppm ∈ {239_999, 240_000, 240_001, 250_000, 265_000, 289_950, 290_000}`.
*Expected (exact):*

| `burst_delta_ppm` | `fec` | `arq` |
|---|---|---|
| +239_999 | 400 | 400 |
| **+240_000** | **400** | **400** |
| **+240_001** | **399** | **399** |
| +250_000 | 355 | 377 |
| +265_000 | 266 | 333 |
| +289_950 | 0 | 200 |
| +290_000 | 0 | 200 |

*Non-vacuity:* a one-ppm input change moves the output by exactly one permille. **This is the anti-Q17 assertion** — Q17's rate-only boundary collapsed to a degenerate `199_999 → 200_000` step, and this pin makes that shape impossible to reintroduce silently. The seven exact values also pin the offset transform, the trapezoid arithmetic and the defuzzifier weights simultaneously; any of them drifting fails.

**T-09 `structure_gate_is_monotone_non_decreasing`** — unit, `fusion.rs`.
*Asserts:* INV-STRUCT-MONO (§1.7) — structure can only raise `fec` and `arq`.
*Inputs:* the 7872-point grid of §1.7 (loss 0..1_000_000 step 25_000 × bw ∈ {500, 25k, 80k, 125k, 175k, 300k} × jitter ∈ {0, 50k, 100k, 450k} × `burst_delta_ppm ∈ {−900_000, −240_000, 0, 100_000, 240_000, 300_000, 700_000, 977_497}`).
*Expected:* `fec(Measured{bd}) >= fec(Unknown)` and `arq(Measured{bd}) >= arq(Unknown)` at every point. **0 violations** (pre-verified).
*Non-vacuity:* includes `bd = −900_000` (anti-correlated, near the achievable minimum `−984_556`) and `+977_497` (the achievable maximum), so the extremes are covered, not just the plausible band. A design that gave R5 `arq_med = 500` fails this at the R2-only points — the test detects the specific error §1.7 warns about.

### Group 5 — Determinism (ADR-17)

**T-10 `structure_gate_determinism`** — unit, `fusion.rs`.
*Asserts:* same input → same output, including the new field.
*Inputs:* 64 iterations over 8 distinct `FusionInputs` spanning `Unknown`, `Measured{−984_556}`, `Measured{0}`, `Measured{+240_000}`, `Measured{+240_001}`, `Measured{+496_109}`, `Measured{+977_497}`, and a value at the clamp edge `Measured{i64::MIN}`.
*Expected:* all 64 decisions per input bit-identical (full `FusionDecision` including `no_rule`); no panic, no overflow at `i64::MIN`/`i64::MAX`.
*Non-vacuity:* the `i64::MIN` case exercises the offset-and-clamp path where a naive `bd + 1_000_000` overflows. Mirrors `validated_determinism` (`tv_fusion.rs:380-401`) and `overflow_safe` (`fate_estimator.rs:529-537`).

**T-11 `fixture_and_validate_accept_the_new_fields`** — unit, `fusion.rs`.
*Asserts:* the config additions are structurally valid and their domain guards fire.
*Inputs:* `FuzzyConfig::fixture().validate()`; then four mutations — `coding_floor` with `a > b`; `coding_floor.b = 2_000_000`; `burst_iid_like.c = 3_000_000`; `fec_structure = 1001`.
*Expected:* `Ok(())` for the fixture; `TrapOrder`, `LossPpmDomain`, **`BurstDomain`**, `ConsequentRange { field: "fec_structure" }` respectively.
*Non-vacuity:* `BurstDomain` is the guard that catches a raw signed ppm passed where offset-binary is expected — the exact bug class `BandwidthDomain` catches for bandwidth. Without the mutation it would be an unfireable guard.

### Group 6 — Neutrality (replacing the retired pin)

**T-12 `structure_affects_only_redundancy_never_pacing`** — Sender integration. **Replaces `estimator_has_exactly_one_reader`.**
*Asserts:* INV-RATE-1 and the influence boundary, behaviourally.
*Inputs:* two `Sender`s driven with an identical event sequence, structure forced to opposite extremes (`Measured{0}` vs `Measured{+900_000}`) via the erasure pattern.
*Expected:* `send_rate_gain_permille` **byte-identical**; the full `TransmitDecision` sequence (seq, size, kind, ordering) byte-identical; `diag_rate()` identical; `no_rule`, `Mode`, `compress` identical. Only `fec_parity_ratio_permille` and `arq_aggressiveness_permille` differ.
*Non-vacuity:* the differing pair is asserted with `assert_ne!`. If the two senders produced identical fec/arq the test fails, so it cannot pass by driving a degenerate flow where structure never engaged. This is strictly stronger than the retired source-text scan, because it constrains behaviour rather than syntax.

**T-13 `estimator_use_sites_are_enumerated`** — Sender integration. **Amends `estimator_has_exactly_one_reader`.**
*Asserts:* the retained source-scan invariant at its new exact value.
*Inputs:* `include_str!("../src/sender.rs")`, newline-flattened (the existing continuation-joining trick at `loss_structure_observation.rs:66-70` must be kept).
*Expected:* every `self.fate_estimator` use is one of `on_original_ack` / `on_loss_evidence` / `advance` / `drop_unresolved_as_unknown` / `snapshot`; the count pinned to the exact post-wiring number; `snapshot` read from exactly the two permitted sites (`diag_loss_structure` and `fuzzy_evaluate`).
*Non-vacuity:* an exact count, not a bound. Any new estimator consumer — including a well-meaning one — fails the test and forces a deliberate amendment. This preserves the rigour of the pin it replaces rather than relaxing it.

### Group 7 — Mutation-style pins (the severed-input class)

**T-14 `severing_the_structure_input_fails_a_test`** — unit, `fusion.rs`.
*Asserts:* the structure input is load-bearing, not decorative.
*Method:* the T-01/T-02 pair is the mutation detector. Documented mutations that MUST fail at least one test, verified once during development with `cp`-backup (never `git checkout` — see the mutation-hygiene rule):

| mutation | test that fails |
|---|---|
| `burst_mu` hardwired to `0` | T-01 (fec 0 ≠ 400) |
| `burst_mu` hardwired to `1000` | T-02, T-06 |
| `LossStructure::Measured` arm returns the `Unknown` value | T-01 |
| R5 weight `min` → `max` | T-02 (burst opens parity) |
| `BURST_OFFSET` = 0 | T-01 (clamp floors all negatives) |
| `coding_floor` → `shoulder_high(0, 0)` | T-05 low-loss case |
| R5 appended with weight but `fec_structure = fec_zero` | T-01 |

*Non-vacuity:* this is the test-of-tests. The mutation table is committed alongside the suite so a future reader can re-verify that each pin still detects its mutation.

**T-15 `estimator_records_real_losses_in_the_product` (P-1 pin)** — Sender integration. **Replaces `estimator_observes_real_loss`.**
*Asserts:* the estimator can actually record a LOSS through the product path.
*Inputs:* the existing lossy-flow driver at `loss_structure_observation.rs:28-53`, unchanged.
*Expected:* `f.lost > 0` **and** `f.n10 + f.n11 > 0` — not merely `f.n > 0`.
*Non-vacuity:* **the current `f.n > 0` assertion passes on a 100%-success window and therefore does not detect P-1, which is live right now.** `lost > 0` is unsatisfiable until `advance()` is wired, and `n10 + n11 > 0` is unsatisfiable unless transitions are also being counted. This single test converts P-1 from an invisible defect into a red build.

**T-16 `non_contiguous_ids_are_detected_not_silently_ineligible` (P-4 pin)** — Sender integration.
*Asserts:* the `OpaqueUnit.id` contiguity contract is observable.
*Inputs:* a lossy flow with ids `1000, 1002, 1004, …` (stride 2).
*Expected:* `f.n > 0` but `n00 + n01 + n10 + n11 == 0`, and the mapping yields `Unknown` (never `Measured`).
*Non-vacuity:* documents and pins the failure mode where a caller silently disables the gate forever. Without it, a stride-2 integration would ship with a permanently closed gate and no signal.

### Group 8 — Campaign-level

**S-01 `spine_level_structure_gate_campaign`** — spine-level campaign check, **not a CI test.**
*Asserts:* the §6 calibration claim end-to-end on the FS-CAL-2 spine.
*Scope:* the S1–S4 design in §6. Runs once, out-of-band, results committed as a report. **Must not be added to the CI gate** — it is a measurement, not a regression pin, and putting a calibration campaign in CI is how thresholds get retuned by accident.

### CI placement

| location | tests |
|---|---|
| `crates/adaptive-transport/tests/tv_structure_gate.rs` (new) | T-01, T-02, T-04, T-06a, T-07, T-08, T-09, T-10, T-11, T-14 |
| `crates/adaptive-transport/src/sender.rs` `#[cfg(test)]` | T-03, T-05, T-06b, T-12 |
| `crates/adaptive-transport/tests/loss_structure_observation.rs` (amended) | T-13, T-15, T-16 |
| out-of-band campaign | S-01 |

**CI blind spot to fix in the same change.** The FS-CAL-2 memory records that `ci.yml` runs only `--all-features`, so the default build is never tested — and two `cfg` breaks have already shipped through that hole. The gate must be exercised in **both** configurations, or T-01/T-02 can pass in CI while the shipped default build has no gate at all.

---

## 6. WHAT MUST BE MEASURED BEFORE THIS SHIPS

**The 240_000 threshold is a calibration claim. Design does not settle it.** Three specific weaknesses in the evidence as it stands:

1. **The separation is measured on synthetic generators, not on the product.** `S0-FEATURES.csv` comes from `fs_cal2_gen.rs`'s IID / MARKOV-BD / GE bit generators. Whether a real netem GE channel through the real `Sender` produces the same `burst_delta` distribution is untested. The estimator's own UNKNOWN handling biases toward IID (gaps delete loss-adjacent transitions preferentially, `fate_estimator.rs:133`), and that bias does not exist in the synthetic path.
2. **The outcome link is inferred, not measured.** S0 shows structure is *separable*. FS-FUSION-CELL-1 shows coding pays on IID-20% and not on tail. **Nothing measures completion as a function of `burst_delta` directly.** The chain "high burst_delta ⇒ parity does not pay" is an inference across two campaigns with different arms and workloads.
3. **The threshold is a holdout-clean point on ONE frozen dataset**, not a preregistered decision on fresh data. The Q17 discipline (`FINAL-REPORT.md §7`) requires the holdout to open only after `freeze.json`. `S0-FEATURES.csv` has already been read to place this threshold, so its `val` split is no longer a holdout for it.

### DR-STRUCT-GATE-1-CAL — the campaign that would justify it

**Preregister before any number exists**: threshold, ramp width, arms, cells, N, the decision rule, and the falsification criteria below. Freeze the binary, commit `freeze.json`, then run.

* **Arms (4).** `OFF` (structure Unknown forced — today's controller); `GATE` (R5 live at 240_000); `ALWAYS` (R5 forced open, structure ignored); `NEVER` (R5 forced closed). ARQ always-on in all four — `ALWAYS`/`NEVER` bound the gate's value between the two degenerate policies. Without them a `GATE`-vs-`OFF` win cannot be attributed to *gating* rather than to *more parity*.
* **Cells.** Structure ∈ {IID, MARKOV-BD-b15, MARKOV-BD-b35, GE-L3, GE-L10} × loss ∈ {5, 10, 20, 30}% × RTT ∈ {50, 250, 1000} ms. Faithful two-netns/netem harness driving the real `Sender` (the single-ns veth trap is recorded in `[[faithful-netem-harness]]`; single-ns silently ignores netem).
* **Workload.** ≥ 2000 originals per run, contiguous ids — long enough that the 256-window fills and eligibility is actually reached. The 48-unit FS-FUSION-CELL-1 workload cannot exercise this gate at all (§2.4).
* **N.** Powered for the completion-rate delta, not "the generator's maximum". FS-FUSION-CELL-1 explicitly disclaimed statistical inference for this reason.
* **Split.** A fresh group-disjoint holdout, groups = (structure family × loss), opened only after freeze. `S0-FEATURES.csv` is now training data for this threshold and may not serve as the holdout.
* **Primary endpoints.** (a) completion rate per cell per arm; (b) total wire bytes; (c) `burst_delta_ppm` distribution *as observed by the product estimator*, compared against S0's synthetic distribution; (d) eligibility fraction per cell; (e) parity bytes emitted.

### What would justify the threshold

* On the **holdout**, `GATE` completion `>= OFF` in every cell, strictly greater in the IID/mild cells at ≥ 10% loss.
* `GATE` byte cost within ~5% of `OFF` on the GE-class cells — i.e. the gate actually withholds parity where it should.
* `GATE` ≈ `ALWAYS` on completion in IID cells **and** `GATE` ≈ `NEVER` on bytes in GE cells. That pair is the definition of a working gate; either alone is not.
* Product-observed IID `burst_delta` p99 stays below 240_000 at every measured loss rate.
* Zero cells where `GATE` completes and `OFF` does not fail — i.e. no completion regression anywhere (the `tail_only = 0` property, re-established on the product path).

### What would falsify it

Any one of these is a NO-GO. Report it; do not retune it.

* **Any harmful false positive on the holdout** — a cell where `GATE` opens parity and completion drops below `OFF`. This is the exact criterion Q17 applied to itself, and Q17 mapped it to NO-GO without retuning.
* **Product-observed IID `burst_delta` p99 ≥ 240_000** at any measured loss rate ⇒ the threshold sits inside the IID distribution on the real path and the synthetic separation did not transfer.
* **GE-class product-observed p05 < 290_000** ⇒ the ramp overlaps real burst and the gate leaks parity onto burst.
* **`GATE` ≈ `ALWAYS` on bytes in the GE cells** ⇒ the gate is not withholding; it is an unconditional parity increase wearing a gate's name.
* **`GATE` ≈ `OFF` on completions everywhere** ⇒ eligibility is never reached on the product path (the P-1/P-4 class, or `FATE_WINDOW = 256` being too long for real flows) ⇒ the gate is inert and must not ship as if it were live.
* **Eligibility fraction < 0.5 in the IID/mild cells at ≥ 10% loss** ⇒ the gate is off more than it is on in exactly the region it was built for.

### Open questions this campaign does not answer, and that must not be silently assumed

1. **`FATE_WINDOW = 256` may be too long for real flows.** It is PINNED; shortening it needs a new FS-CAL-2 calibration version and a full S0 re-run. If S-01 shows eligibility is rarely reached, that is the next campaign, not a quick constant edit.
2. **MARKOV-BD-b15 is unresolvable at N=256** (13–19% detection). The design gives it parity. Whether that is right is unmeasured; b15 cells in S-01 are the first evidence either way.
3. **Estimator write-set width.** Success evidence is fed only for `TransmitKind::Data` originals (`sender.rs:2273-2275`) and loss evidence only for non-`Duplicate` origins (`sender.rs:2611-2620`). Both are Karn-correct, but tail probes and duplicates contribute no structure signal, which may bias burst-vs-IID discrimination. Measure before widening — widening it is a change to what "structure" means.
4. **`coding_floor` at 90_000/140_000 is inherited from FS-FUSION-CAL-1**, whose holdout returned CALIBRATION-NO-GO. The breakpoints were not themselves falsified (the missing structure feature was), but they have never passed a holdout either. S-01's 5% and 10% cells are the first test of them.

---

## Appendix A — Verification log

Every load-bearing number in this document was checked against source at `f54b1cdc`. Read-only; no file edited, no cargo command run.

| claim | verification |
|---|---|
| `advance()` / `drop_unresolved_as_unknown()` have zero product callers | grep over `crates/adaptive-transport/src/` — only `sender.rs:1020,1245,1462-1463,2274,2619` reference the estimator; neither method appears |
| fixture `loss.low = shoulder_low(200_000, 300_000)` | `fusion.rs:330` |
| rule table R1–R4, `min3`, weighted-average defuzzification | `fusion.rs:496-526`, `:558-572` |
| `send_rate_gain` reads only `bw.low`, `qd.high`; denominator fixed 1000 | `fusion.rs:528-533` |
| Q18 pin asserts `fec == 0` at (200_000, 125_000) with exact equality | `q18_check.rs:25-46` |
| `compute_requested_r(ratio,k,max) = (ratio·k + 500)/1000` clamped | `sender.rs:537-542` |
| eligibility = 6 conjuncts, `n == 256` exactly, 8 / 32 | `fate_estimator.rs:93-106`, constants `:35,37,39` |
| achievable `burst_delta_ppm` range `[−984_556, +977_497]`, both eligible | exhaustive enumeration over (L, r, startsLoss, endsLoss) for N=256 |
| clean-flow walk-up 0 / +166_667 / +250_000 / **+300_000 at n=4** / … / +496_109 at n=256 | computed from `fate_estimator.rs:147-148` |
| eligibility floor `measured_loss_ppm >= 31_250` | `floor(8·10⁶/256)` |
| **240_000 is zero-FP on train AND val; 215_000 leaks 0.61% on val** | sweep over `S0-FEATURES.csv` honoring its `split` column (7065 train / 2935 val) — **corrects the grounding, which pooled the splits** |
| eligibility fractions GE-L10 0.564 / GE-L3 0.728 / IID 0.944 @ p=5% | computed from `S0-FEATURES.csv` |
| IID 0/12 → 5–9/12 on IID cells; 12/12 both on tail cells | `reports/fs-fusion-cell-1/CELL-RESULTS.csv` |
| parity_bytes 9696–13332 on IID cells; 13332–14544 on tail cells | `reports/fs-fusion-cell-1/ARQ-CONTRIBUTION.csv` — **per-cell aggregate over 12 reps** (≈808–1212 B/run), not per run |
| ARQ still spends 19998–54035 B in the fusion arm on IID | `ibid.`, `arq_retx_bytes` |
| all existing pinned outputs unchanged under `structure: Unknown` | recomputed: 650/550, 0, 0, 250, 500, `no_rule` true — all match |
| worked outputs 400/400, `r = 2`; boundary 400 → 399 at +240_001 | computed from the specified traps and defuzzifier |
| INV-STRUCT-MONO holds | 0 violations over 7872 grid points |
| `FusionInputs` not `#[non_exhaustive]`; 14 literal construction sites | grep; `fusion.rs:378-391` |
| Q17 degenerate `199_999 → 200_000`; R2 "necessary but not sufficient" | `reports/fs-fusion-cal-1/FINAL-REPORT.md:134`, `:81-85` |
| FS-FUSION-CAL-1 `LossBoundary` 90k/140k/160k/200k | `fusion_policy.rs:224-229` |
| reference eligibility-flag wiring | `crates/kama-integration/tests/common/fs_cal2_spine.rs:1024-1032`; `sender.rs:4457-4471` |

**One correction to the grounding provided with this task:** the threshold band "215_000–240_000, 0.0% IID false-positive" was fit on the pooled dataset. `S0-FEATURES.csv` carries a `split` column, and honoring it shows 215_000 leaks a 0.61% IID false-positive on the held-out rows at p=10%. 240_000 is the lowest threshold clean on both splits, and is the number this design uses.

---

# ADVERSARIAL REVIEW

# ADVERSARIAL REVIEW — DR-STRUCT-GATE-1

**Verdict: NOT SOUND AS WRITTEN.** 3 blocking defects, 10 major concerns. The calibration work is genuinely good — every S0 and FS-FUSION-CELL-1 number reproduces exactly, and the range derivation is correct — but the *safety* argument (§3, "No pinned output value changes. Zero.") is invalidated by the design's own mandatory P-1 fix, and the gate's single actuated axis is smaller than the document claims.

---

## VERIFIED CORRECT (re-derived independently, not trusted)

| claim | my result |
|---|---|
| achievable `burst_delta_ppm` range `[−984_556, +977_497]`, both eligible | **exact** — exhaustive enumeration over (L, r, starts_loss, ends_loss) for N=256 with a corrected feasibility constraint (success-run count must be ≥ needed). Max exemplar `(n00,n01,n10,n11)=(149,1,0,105)` → 977_497 ✓; min `(0,128,127,0)` → −984_556 ✓ |
| eligibility loss floor 31_250 ppm | **exact** — min L among eligible configs = 8 → `8·10⁶/256` |
| clean walk-up `0 / +166_667 / +250_000 / +300_000 / +388_889 / +469_697 / +492_249 / +496_109` | **exact**, all ineligible (`n10+n11=0`) |
| empty estimator `bd = 0` → gate outputs `fec=400` | **confirmed**. T-06 is load-bearing and the enum is genuinely required |
| worked output 400/400, `compute_requested_r(400,5,4) = (2,2)` | **exact** (`sender.rs:537`) |
| T-08 boundary 400/400/**399**/355/266/0/0 | **all seven exact** |
| all existing unit goldens byte-identical under `structure: Unknown` | **confirmed** — I reimplemented `decide()` and diffed R4-vs-R5 on tv_fusion B/C/D/zero_weight and all four q18 points (250 and 500 at bw=125k reproduce as 250/500) |
| S0 medians, eligibility fractions, FP/detection at 200k/215k/240k | **exact to the digit** — 215_000 leaks 0.61% val at p=10%; 240_000 is 0.00/0.00/0.00 on both splits for p≥10%; b15 13.51/15.79; b35 85.92/86.00; GE 100% |
| FS-FUSION-CELL-1 completions, `tail_only=0` everywhere, parity/arq_retx bytes | **exact** |
| P-1, P-2, P-3, P-4, P-5 | **all five real**. `advance()`/`drop_unresolved_as_unknown()` have zero product callers (only `fs_cal2_spine.rs:978,1024-1025`); `on_loss_evidence` at `sender.rs:2619` fires for `FecParity` whose `unit.id = esi` (`sender.rs:3564`); the spurious arm is severed by `inflight.remove` at `queue_losses` |
| INV-RATE-1 | **preserved.** `send_rate_gain` reads only `bw.low`/`qd.high`, denominator structurally 1000 (`fusion.rs:530-533`). Nothing in the design contacts the pacer |
| no ARQ-vs-FEC vocabulary | **clean** |

---

## BLOCKING

### B-1. The golden-vector argument is voided by the design's own P-1 fix

§3.1 asserts *"No pinned output value changes. Zero."* The `fuzzy_e2e` row is hedged with *"unchanged **while `Sender` supplies `Unknown`**"* — and §0.2 P-1 **mandates** wiring `advance()`, which is exactly what makes the Sender stop supplying `Unknown`. §3.2's "what DOES break" enumeration omits this entirely.

Concretely, `crates/kama-engine/tests/fuzzy_e2e.rs::e2e_real_fuzzy_no_rule_literal_zero`:
- drives `drive_regime(10,2,2,10,10,0)` + `drive_regime(60,6,1,10,230,t)` = **380 contiguous-id originals** (`next_id += 1`), well past `FATE_WINDOW = 256`;
- the measured operating point is recorded in `rule_coverage_map.rs:82` as `loss=659_090, bw=80_000` → `coding_floor.mu = 1000`;
- the emission pattern is 6 emitted / last-1 ACKed → per-round fate `L L L L L S` → `n11≈171, n10≈43, n01≈43, n00≈0`, i.e. `n10+n11≈213 ≥ 8` and `n00+n01≈43 ≥ 32` → **eligible**;
- `p11 = 172/216 = 796_296`, `p01 = 44/45 = 977_778` → `burst_delta ≈ −181_482` → `burst_u = 818_518` → `burst_iid_like.mu = 1000`.

R5 fires at full weight ⇒ `no_rule` flips to `false`, `fec = 800`. Three assertions fail: `assert!(snap.no_rule)`, `assert_eq!((planned, emitted), (0,0))`, `suppression == Some(FecSuppressionReason::NoRule)`.

Whether it breaks hinges on `n00+n01 ≈ 43` staying above 32 — a **coin-flip margin is not a safety argument** for a CI pin. (4.1/4.5 are safe: both R2 and R5 carry consequent 800, so the ratio stays 800. 4.2 is safe: 120 originals < 256. 4.4 adapts.)

### B-2. `no_rule` is not preserved, and it is a literal-zero FEC interlock

§1.6: *"Defuzzification, `no_rule`, `send_rate_gain`, `Mode`, and `compress` are **untouched**."* True only at R5 weight 0. On the design's **own** §1.7 / T-09 grid (loss 0–1M ×41, bw ×6, jitter ×4, bd ×8 = 7872 points) I measured:

```
fec/arq monotonicity violations = 0     (INV-STRUCT-MONO holds — this part is correct)
no_rule FLIPS true -> false      = 2205 / 7872  (28.0 %)
example: loss=300_000 bw=500 jit=0 bd=0
         base {fec:0,   arq:200, no_rule:True}
         R5   {fec:800, arq:600, no_rule:False}
```

Downstream this is not observability. `sender.rs:3256`:
```rust
Some(s) if s.no_rule => {
    self.no_rule_zero_count = ...;
    zero(0, FecSuppressionReason::NoRule, true) // LITERAL-0 (fec_zero-config'e-BAĞLI-DEĞİL)
}
```
R5 removes that interlock in the sub-kbps / ≥60 %-loss class the fallback was built for after a **measured** degeneration (`fusion.rs:510-516`). At `loss=700_000, bw=500` the design would plan `fec=800` → `r = clamp((800·8+500)/1000) = 4` on a 500 B/s link.

INV-STRUCT-MONO is stated over `fec`/`arq` only, so **T-09 passes blind to this**, and no test in the 16-test suite pins `no_rule` under a firing R5. R5 also inherits none of FS-FUSION-CAL-1's declared support region (`fusion_policy.rs:225-229`: `loss_ppm_max: 200_000, bandwidth_bps_min: 100_000, bandwidth_bps_max: 125_000`) while reusing its breakpoints as "the same evidence".

### B-3. §2.2's cost asymmetry is wrong by ~10×

`ARQ-CONTRIBUTION.csv` `total_bytes` is the **12-rep aggregate**: `59388 / 12 = 4949` = `CELL-RESULTS.csv` `tail_bytes_mean`. The design divides the *per-run* cost (`cost_delta_mean` = 1111–1212 B) by the *aggregate* baseline (59–63 KB) and reports **"roughly +2 % bytes"**.

Per run: `1111 / 4949 = **+22.4 %**` (rtt50-exactd1: 4949 → 6060). Appendix A even flags the aggregate/per-run distinction for `parity_bytes`, then §2.2 commits the error anyway. The conclusion (asymmetry still favours spending) probably survives; the number that carries §2.2 **and** §2.3(c) does not.

---

## MAJOR

**M-1. §1.2's wiring is not implementable.** `self.last_ack_progress_ns` and `MAX_TELEMETRY_AGE_NS` **do not exist** in `Sender` (grep over `crates/adaptive-transport/src/`). They exist only as a spine-local `let mut last_ack_progress_ns` (`fs_cal2_spine.rs:582,949`) and `FusionSafetyConfig::max_telemetry_age_ns` (`fusion_policy.rs:131,142`) — in the module §4.4 declares untouched. The `telemetry_stale` eligibility conjunct has no product source.

**M-2. R5's ARQ consequent is provably inert — and the framing defence rests on it.** `arq.rs:105-109`: `arq_aggressiveness_permille` is used *only* as an `== 0` gate; the rest of `plan()` sorts by `deadline_ns` and ignores it. The design itself argues `== 0` is structurally unreachable. So 200 → 600 changes **zero bytes**. This is already recorded in-repo — `reports/fs-fusion-cal-1/FINAL-REPORT.md §4`: *"the fusion ARQ output is inert (`arq.plan` uses `arq_aggressiveness_permille` only as an `==0` gate, which the fixture's ≥200 never triggers)"*. Consequences: §1.7's `arq_moderate`-vs-`arq_med` argument is about a dead axis; §4.2's "raises fec **and** arq together" is satisfied only in name; and T-12's `assert_ne!` on the arq half proves nothing.

**M-3. The ramp-margin number is invented.** §1.5: *"the ramp occupies [240_000, 290_000], leaving ~118_700 ppm of margin to the nearest GE observation."* The nearest GE **observation** is GE-L3 **min = 145_528** (p=10 %) — inside `burst_iid_like`'s full-membership plateau; 4/1853 GE-L3 rows sit below 290_000. A 5th percentile was substituted for "nearest observation". The quoted `+408_730` also does not reproduce (index-p05: 408_120–433_880; linear-interp: 406_054–433_282), nor does `+708_082` (I get 706_848).

**M-4. "240_000 clean on both splits" silently excludes p=5 %.** At `p_ppm=50_000`, eligible IID rows ≥ 240_000 are **0.93 % train / 1.35 % val**, max 283_541 on val. Every FP table covers p=10/20/30 only. The rescue — `coding_floor.mu(50_000) = 0` — is never stated, and if stated would depend on `inputs.loss_ppm` (ledger-windowed, `fuzzy_evaluate:2679`), the estimator window's `measured_loss_ppm`, and the generator's `p` all agreeing. They are three different measurements.

**M-5. Per-event cost on the hot path.** `structure` is built in `fuzzy_evaluate` (`sender.rs:2661`), called on **every** event (`sender.rs:1562`). `snapshot()` iterates all of `finalized` and allocates `window_ids: Vec<u64>` + `window_fates: Vec<bool>` (`fate_estimator.rs:258-270`). Before the P-2 prune that is O(total) per packet; after it, still 2 heap allocations + 256-entry copy per event. The repo's own bar for touching this path (`sender.rs:2694-2697`) is *"one fixed-size Copy assignment with no allocation, I/O or formatting."* §3.3 covers determinism and omits cost entirely.

**M-6. `drop_unresolved_as_unknown()` placement is under-specified with a severe failure mode.** §0.2: *"only at a decision epoch."* In the product every event is a decision epoch. Called per event it clears every provisional loss before its guard matures (`expiry = now + base_pto`, up to 1 s cold) — essentially no LOSS ever finalizes, reproducing P-1's all-SUCCESS symptom by a different route while T-15 might still squeak through.

**M-7. T-05 is not implementable as specified.** It is listed as a *Sender integration* test that constructs six `WindowFeatures` fixtures and asserts the Sender's mapping. There is no way to inject `WindowFeatures` into a `Sender`, and the repo forbids injection on this path (`fuzzy_e2e.rs` header: *"ENJEKSİYON-YOK; snapshot-alanı-yazılmaz"*). It needs a pure `pub(crate)` mapping function the design does not specify — after which it no longer tests the wiring.

**M-8. T-10's overflow justification is inverted.** `burst_u(bd) = clamp(bd + 1_000_000, 0, 2_000_000)`. `i64::MIN + 1_000_000` is representable; **`i64::MAX + 1_000_000` overflows**. T-10's input list contains `i64::MIN` and not `i64::MAX`, and its non-vacuity note claims the `i64::MIN` case catches the naive-add overflow. As written, T-10 cannot detect the overflow it names.

**M-9. Sender-level both-outcome forceability (T-03) is unproven and may be unreachable.** Unit-level forceability (T-01/T-02) is trivial — the input is injected. T-03 requires the *product* path to reach `burst_delta ≥ +290_000` from ACK-driven loss declaration, which is §6's own open question #1. My one product-shaped datapoint points the wrong way: `fuzzy_e2e` 4.3's genuinely run-structured 5-in-6 pattern computes to **−181_482**. If T-03 cannot be made green, the design has no end-to-end both-outcome proof — the exact failure class the task brief forbids.

**M-10. The negative half of the axis is uncalibrated and fully open.** `burst_iid_like = shoulder_low(1_240_000, 1_290_000)` grants membership **1000** to every `bd ≤ +240_000`, i.e. the whole range down to −984_556. `burst_delta = p11 − p01` is not monotone in burstiness: at high loss a deterministic run pattern with `p01 → 1` reads strongly negative while being maximally run-structured (B-1's own worked case). S0 contains no anti-persistent generator, so the left side of the threshold has zero calibration and the gate opens parity there by default. §1.7's grid includes `bd = −900_000` but only checks `fec/arq ≥`.

---

## MINOR

- **§5's CI blind spot is stale.** `.github/workflows/ci.yml` already has the `semantic-matrix` job with `default-workspace` (`args: ""`) doing build + clippy + test, added by DR-RATE-RAMP-2B §A1 with that exact rationale in the comment (lines 21-63). The design asks the implementer to fix something already fixed.
- **`FusionInputs` literal sites: 12, not 14.** `fusion.rs:415`, `fusion_policy.rs:402`, `sender.rs:2679`, `q18_check.rs:16`, `tv_fusion.rs:{44,62,70,86,124,234,251,390}`. The "plus 2 more in the same files" do not exist.
- Adding a `LossStructure` enum to `FusionInputSnapshot` breaks its documented contract *"Alan-tipleri FusionInputs ile HİZALI (u64)"* (`fusion.rs:396`) — needs a doc amendment, not just a field.
- The two new bare `Trap`s escape `covers_domain()` (only `VarThresholds` are checked in `validate()`'s `traps()` helper) — an invariant asymmetry vs the five existing inputs, by design but unremarked.
- `fuzzy_evaluate` runs at `sender.rs:1562`, `update_state` at `:1565`, so `self.state()` in the proposed snippet is the *previous* event's state.
- "Structurally silent below 3.125 % loss" is a property of the estimator window's `measured_loss_ppm`; `coding_floor` reads the ledger-windowed `inputs.loss_ppm`. They do not compose into one loss floor.

---

## NOT DEFECTS (attacks that failed)

- The threshold **is** justified by the estimator's real numeric range — I re-derived it exhaustively and both endpoints and the mid-range placement check out. This is not an invented number.
- Eligibility degradation **does** reduce exactly to current behaviour *at the controller level* (`Unknown` is a true identity element of `weighted_average`), verified numerically against every pinned unit vector.
- No unit golden breaks. No pacing contact. No ARQ-vs-FEC reframing in the prose.
- T-01/T-02/T-06/T-07/T-08/T-14 are non-vacuous as specified and T-06 in particular cannot pass with a bare-`i64` input — that argument is correct and well made.

---

## WEAKEST REMAINING ASSUMPTION

Even with B-1…B-3 and M-1…M-10 repaired, the load-bearing assumption is the one the design names in §6 open question #1 and then does not weight: **that the synthetic S0 `burst_delta` distribution transfers to the product's ACK-driven, guard-delayed, UNKNOWN-punctured fate window.** My single product-shaped datapoint (fuzzy_e2e 4.3, deterministic 5-loss runs → `bd ≈ −181_000`) transfers in the *wrong direction*: run-structured product loss read as maximally IID-like. If that generalises, the gate is not merely uncalibrated on the product path — it is inverted on it, and the §6 falsifier *"GATE ≈ ALWAYS on bytes in the GE cells"* would fire.