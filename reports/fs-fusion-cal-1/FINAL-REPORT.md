# FS-FUSION-CAL-1 — Final Report

> Authoritative data analysis → fuzzy ARQ/FEC calibration → safe product integration → holdout validation.
> Single report for the whole chain. Numbers cite committed artifacts. Nothing here authorizes default-ON.

## VERDICT: **CALIBRATION-NO-GO** (for calibration-v1's FEC-activation region)

A valid, pre-registered holdout produced a **harmful false positive**, which the frozen acceptance criteria
map directly to NO-GO. The result was **not** retuned away (§17 forbids feeding holdout outcomes back into
tuning). The safety architecture, the zero-diff guarantee and the decision-region science all stand; what
fails is the specific claim that **loss rate alone can gate FEC activation**.

**Product impact: none.** The mechanism is default-OFF and every reliability-critical module is byte-unchanged.

---

## 1. Data

| Domain | Source | Rows | Ingested | Excluded |
|---|---|---|---|---|
| `FS1_IN_PROCESS` | `reports/fs-1/raw/v1.3.2` | 240 | 240 | 0 |
| `R2A_IN_PROCESS` | `reports/dr-arq-tail-1/v1/raw` | 8400 | 8400 | 0 |
| `PRODUCT_NETNS` | `reports/dr-arq-product-tail-1/v1/result.csv` | 2290 | 2290 | 0 |
| **Total** | | **10930** | **10930** | **0** |

Canonical dataset `canonical-calibration-data.csv` sha256 `d3096454…`. Domain separation enforced (in-process
and netns never pooled). Never read: FS-1 v1.3.1 (SUPERSEDED-INCOMPLETE-SCHEMA), commit `44e0c50` (INVALIDATED
arm-label). Action vocabulary per domain: FS1 {ARQ_FROZEN, BLIND_DUP, FEC_R2, FEC_R4} × 60;
R2A {ARQ_FROZEN, ARQ_TAIL, BLIND_DUP, FEC_R2} × 2100; PRODUCT {ARQ_FROZEN, ARQ_TAIL} × 1145.

### The central data finding
**All 10930 authoritative rows have `runtime_features_present = 0`.** No runtime fusion-input telemetry
(loss_ppm, bandwidth, jitter, qd_ratio, ack_confidence) was ever recorded. The authoritative datasets ground
**cell → action → outcome** only; they cannot fit a runtime-feature → action map. Every downstream limitation
in this report descends from that fact.

---

## 2. Analysis

Method frozen before the new analysis (`analysis-method.json`), with a transparent **amendment v1.1** that
only *tightened* the conditions for declaring a winner (see §7). Lexicographic endpoints: safety → horizon
completion → realized wire → RMST → conditional latency; deadline strictly a separate secondary endpoint;
no scalar utility, no invented non-inferiority margin. Wilson 95 % CI, McNemar exact on paired discordance,
grouped bootstrap (10 000 resamples, seed 20260804, resampled at pairing-group level), KM/RMST for censoring.

**77 cells, 242 pairwise contrasts.** Preferred action by domain:

| Domain | ARQ_TAIL | ARQ_FROZEN | FEC_R2 | undecided |
|---|---|---|---|---|
| R2A_IN_PROCESS | 16 | — | 4 | 1 |
| PRODUCT_NETNS | 27 | 2 | — | 15 |
| FS1_IN_PROCESS (n=5, exploratory) | — | 6 | — | 6 |

**Where FEC actually wins** (`FEC_R2` Pareto-dominant): the three IID-20 % cells at every RTT — 59–66/100
completion vs `ARQ_TAIL` 12–14/100, **at lower realized wire** (5082–5592 B vs 6063–6339 B) — plus
`s1-rtt1000-d4` (81/100 vs 74/100, at higher cost).
**Where ARQ wins**: sparse terminal tails — equal completion (100/100 both) at ≈1111 B less than FEC.

`FUSION_DOMINANT` is **unreachable**: no authoritative arm combines tail-capable ARQ with proactive FEC.

---

## 3. Calibration

`calibration-v1.json` sha256 `0197445e…`.

**The defect found.** Production runs the explicitly non-calibrated fixture: `Sender::new()` →
`ValidatedFuzzyConfig::trusted_fixture()` (sender.rs:983-985, fusion.rs:200-202), a path that also **skips
`validate()`**. That fixture sets `loss.low = shoulder_low(200_000, 300_000)`, so μ_LOW = **1.0 at 200 000 ppm**
— 20 % loss is classified *fully LOW*, firing R1 (`loss LOW → fec_zero`). The shipped engine therefore
**never recommends FEC in precisely the region where the data shows FEC Pareto-dominates**.

**What was calibrated.** Only the **loss** membership (the sole dimension the data identifies), moving the
LOW/HIGH boundary into the observed evidence gap between 83 333 ppm (ARQ-dominant) and 200 000 ppm (FEC-dominant).
**bandwidth, jitter, queue_delay, ack_confidence are RETAINED at fixture values and declared UNCALIBRATED** —
the authoritative data never varied them. Consequents and the rule table are structurally unchanged.

**Recorded structural limitation.** R2 — the only rule that can raise FEC to `fec_high` — is conjoined with
`bw HIGH`, a dimension never varied and placed by the fixture at 150–200 kB/s, while the entire tested regime
is ≈125 kB/s. Correcting the loss membership is therefore **necessary but not sufficient** for FEC activation.
Changing R2's antecedent is a rule-structure change this calibration deliberately did **not** make without
bandwidth evidence.

---

## 4. Product integration and reliability safety

`crates/adaptive-transport/src/fusion_policy.rs` (additive). Architecture:
`FusionObservation` (prefix-causal only) → `FusionPolicy` (pure, deterministic) → `FusionAdvice` →
`FusionSafetyValidator` (independent, final authority) → bounded `FusionAction`.

- **Default OFF**: `FusionPolicy::disabled()` ⇒ `FallbackArq(PolicyDisabled)` for every input.
- **ZERO-DIFF** on `sender.rs`, `arq.rs`, `loss.rs`, `rtt.rs`, `fusion.rs`, `tail_recovery.rs` (verified by
  `git diff --stat`, empty). `policy_disabled ⇒ baseline ARQ trace exact` holds **structurally**, not merely
  empirically — no membership breakpoint, threshold, rule weight, defuzzification or calibration constant was
  added to the sender.
- **Hard caps are independent of calibration** (`FusionSafetyConfig`): a calibration may request less, never
  more. Every one of version-mismatch / stale telemetry / cold-start / low-confidence / unsupported-region /
  invalid-input / FEC-unavailable degrades to `FallbackArq`.
- **Jitter bootstrap guard**: `rtt.rs` seeds `rttvar = srtt/2` on the first sample ⇒ `jitter_ppm = 500 000`,
  which lands in the fixture's jitter-HIGH shoulder. The validator requires ≥2 RTT samples before trusting it.
- **Tests**: `fusion_policy` 15/15 (boundary below/at/above, determinism ×64, overflow at `u64::MAX`,
  exhaustive monotone domain scan, cap clamping, every fallback reason); full `adaptive-transport` lib 113/113.

**Product FEC actuation is POLICY-ONLY** — there is no product send-path FEC encoder (a decode path exists but
is default-OFF and unreachable from any product sender), and the fusion ARQ output is inert (`arq.plan` uses
`arq_aggressiveness_permille` only as an `==0` gate, which the fixture's ≥200 never triggers). The validator
therefore refuses to emit a FEC action when `fec_actuation_available` is false.

---

## 5. Holdout (pre-registered, executed once)

`freeze.json` → `holdout-policy-results.csv`, `holdout-cell-results.json`.

Split: **group-disjoint by RTT stratum** — TRAIN = cells at 50/250 ms (52 cells), HOLDOUT = cells at 1000 ms
(25 cells). No cell, seed lineage or pairing group on both sides; row-random splitting forbidden. The boundary
was **re-derived from TRAIN only**, then evaluated on never-inspected HOLDOUT cells.

| Outcome | Count |
|---|---|
| MATCH | 12 |
| **HARMFUL FALSE POSITIVE** | **1** |
| FALSE NEGATIVE (conservative) | 1 |
| Abstain (no Pareto preference) | 11 |

**The failure — and why it matters.** `PRODUCT_NETNS burst/20/high` at 200 000 ppm has Pareto-preferred
`ARQ_TAIL`, but the policy recommended `FecAssist`. At the *same observable loss rate*, IID structure wants FEC
(`s3-rtt1000-iid20` → MATCH_FEC) while BURST structure wants ARQ. The policy cannot distinguish them because
**loss burstiness / run-length / gap statistics are computed nowhere in `adaptive-transport`** — the loss module
retains per-event `seq` and `declared_ns` but never differences, sorts for adjacency, or run-length-encodes them.

The training split had already signalled this: boundary derivation from TRAIN alone collapsed to a degenerate
`199 999 → 200 000` gap, because at 200 000 ppm the training cells contain **both** ARQ-preferred and
FEC-preferred outcomes. Loss rate alone is not a separating feature.

Per the frozen acceptance mapping (any harmful false positive ⇒ NO-GO) the verdict is **CALIBRATION-NO-GO**.
The result was reported, not retuned.

---

## 6. Claim boundary

**SUPPORTED NOW**
- The shipped fuzzy config is uncalibrated and cannot recommend FEC where the data shows FEC dominates (§3).
- Per-cell Pareto regions over 10930 authoritative rows (§2).
- A safe, versioned, default-OFF policy/safety architecture with structural ARQ zero-diff (§4).
- Loss rate alone is insufficient to gate FEC activation — demonstrated on held-out data (§5).

**NOT SUPPORTED**
- Any default-ON or canary authorization.
- "The calibrated policy improves the product" — it cannot actuate anything today (POLICY-ONLY FEC, inert ARQ gate).
- Any FEC-activation threshold on loss rate.
- Any product-wide FEC policy, burst conclusion, or external-protocol comparison.

---

## 7. Method integrity

- Method frozen before the new analysis; **amendment v1.1** documented in `analysis-method.json` and the
  uncorrected first pass preserved as `decision-regions.v1-uncorrected.csv`. The amendment only *tightened*
  winner declaration: (a) an underpowered completion test may no longer let cost silently decide against the
  numeric completion direction, (b) `preferred_action` surfaced because family-level labels collapsed the
  product domain. No endpoint, alpha, seed, ordering or domain rule changed.
- Leakage firewall: arm/cell/impairment/completion/censoring/cause labels are offline-only; the runtime policy
  consumes prefix-causal features exclusively.
- Holdout opened only after `freeze.json`; no adaptive stopping; no retuning on holdout.

---

## 8. Remaining work (not authorized this turn)

1. **Runtime loss-structure estimator** (prefix-causal burstiness/run-length from the existing loss events) —
   the blocking prerequisite for any safe FEC activation. Then re-calibrate and re-hold-out.
2. **Product FEC actuator** (safe, feature-gated, bounded) — prerequisite for `CALIBRATION-GO — PRODUCT-VALIDATED`.
3. **Bandwidth-varying evidence** — R2's `bw HIGH` antecedent is currently unfalsifiable in the tested regime.
4. Product-v2 tail tuning (adaptive cap/backoff for multi-unit tails; cold-start interaction).
5. R2B (NaturalBurst + probe-loss-eligible).
6. Quinn/TCP/KCP comparison (explicitly deferred by this directive).
