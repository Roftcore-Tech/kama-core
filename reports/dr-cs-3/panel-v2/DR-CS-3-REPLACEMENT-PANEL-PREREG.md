# DR-CS-3 — REPLACEMENT CHALLENGE PANEL, PREREGISTRATION

Frozen and hashed **before any seed search**. No ON-arm result exists at the time of writing.

## Why a replacement is needed

The frozen locked seeds no longer express the challenge phenotype under V2. Measured cause:
key re-domaining. First 20 forward draws for rep9/14/17 are `DDDDDDDDDDDDDDDDDDDD` under the V1
`InitialSource` key and `....................` under `V2Forward` — the phenotype is absent from
draw 0, before any retransmission or Probe could interleave.

C2 gate 1 is therefore `UNEVALUABLE — FROZEN CHALLENGE PHENOTYPE NOT REPRODUCED UNDER V2`.
Neither PASS nor FAIL, and no evidence about bootstrap efficacy.

Per the key-selection guard, the key is **not** changed to bring the old stalls back: both keys
conform to C2 clause 5, so that choice may not be made on outcome grounds.

## 1. Finite seed universe — fixed now

```
domain      DRCS3-V2-CHALLENGE
cell        P2-GE20L10-RTT1000
RTT         1000 ms
forward     GE p=200000 ppm, L=10        reverse  lossless (primary continuity)
universe    repetition ∈ [0, 512)        SIZE FIXED HERE, never widened post hoc
```

**Size justification (from §2b, before any result).** The chain's initial state is seed-derived:
`next_drop` at `i == 0` uses the stationary probability. So

```
P(first packet already bad)              = p        = 0.200
P(stay bad)                              = 1 - 1/L  = 0.900
PreConverge window                       ≈ 20 packets (31 B/s floor, 101 B unit, 65 s)
P(bad run covers all 20 | bad at start)             = 0.1351
P(start bad AND run covers 20)                      = 0.0270
```

≈ 2.7 % per seed (a lower bound — it ignores seeds entering bad after draw 0). 512 seeds gives an
expected ≈ 13.8 qualifying seeds, comfortably above the 3 needed while leaving a confirmation set.

## 2. Exact OFF-stall phenotype — fixed now

A seed qualifies iff its **OFF (baseline) arm** satisfies **all** of:

```
preconverge_exit_ns  is None          (never left PreConverge)
ack_batches          == 0             (no ACK ever ingested)
delivered_src_at_epoch == 0           (nothing delivered)
epoch_trigger        == "TIMEOUT"
```

This is exactly the V1 cold-start stall phenotype, restated against V2 fields. No weaker variant is
admissible.

## 3. Selection procedure — fixed now

1. Run **OFF arm only** over the whole universe. The ON arm is **not built, not run, not read**
   during selection.
2. Collect every seed meeting §2 in ascending repetition order.
3. Take the **first 3** in that order for the challenge set (deterministic, lexicographic-by-index).
4. Take the **next 3** as a disjoint **confirmation set**.
5. Write both lists to the ledger and **hash them** (§5b) *before* any ON arm is run.
6. If fewer than 3 qualify: do **not** widen the universe, lower the phenotype, or change the seed
   domain. Report
   `CHANNEL-MODEL-PROPERTY / DEFAULT PROMOTION BLOCKED` and apply C1's bounded product-visible path.

## 4. Gates — unchanged

The C2 gates are **not** modified by this replacement:

* challenge panel: **3/3** of the selected stalls recover (original strictness);
* new stalls: **0**;
* fresh primary N=40: first valid ACK ≥ **38/40**;
* all other C2 gates as frozen;
* `NETWORK-CENSORED` stays in the denominator;
* the §0 empty-pass rule applies: any gate the OFF arm already meets is
  `MECHANISM-UNEVALUABLE (NON-HARM ONLY)`, not PASS.

## 5. Prohibited

Seed shopping · reading ON results during selection · widening the universe after seeing counts ·
weakening the phenotype · changing the seed domain to obtain stalls · post-hoc exclusion of
`NETWORK-CENSORED` · altering any C2 threshold.

---

# ADDENDUM (explanatory only; added before any seed search)

## A1. The 2.7 % figure is an analytic sizing assumption, not a measured rate

`0.2 × 0.9^19 ≈ 0.027017` is derived from the channel parameters and a **20-packet** PreConverge
window. It has **not** been shown equivalent to the exact OFF-stall phenotype of §2. Until that
equivalence is demonstrated it stands only as sizing rationale.

If the assumption is exact or a sufficient lower bound, then for N = 512:

```
E[X] ≈ 13.83        P(X ≥ 6) ≈ 0.9943
```

The universe stays **512**. It is not enlarged merely to raise these numbers.

## A2. Window-assumption sensitivity, and its attribution

The rate is sharply sensitive to the assumed window:

```
20 packets → E[X] ≈ 13.8     30 packets → ≈ 4.8     35 packets → ≈ 2.8   (BELOW 3)
```

The OFF-only selection run therefore records the **observed forward-packet-count distribution** at no
extra cost. If fewer than three seeds qualify, attribution comes from that distribution:

* observed window ≈ 20 → `CHANNEL-MODEL-PROPERTY` (the model does not produce the phenotype);
* observed window ≫ 20 → `SIZING-ASSUMPTION-MISMATCH` (the universe was mis-sized).

A `SIZING-ASSUMPTION-MISMATCH` is **not** fixed by enlarging this universe. It requires a **new**
preregistration with a corrected window. This preregistration is never silently reinterpreted, and
packet count alone is not conclusive causal evidence.

## A3. Confirmation is a real holdout — binding roles

Frozen before any seed search:

1. first three qualifying seeds = **challenge set**;
2. next three = **confirmation holdout**;
3. both lists hashed **before** any ON arm runs;
4. challenge ON runs first;
5. confirmation is **not opened** until challenge is 3/3;
6. product fixes may be made against the challenge set; each fix requires challenge 3/3 again;
7. confirmation opens **once**, only after challenge 3/3 **and** the final product commit is frozen;
8. default promotion requires confirmation **3/3** as well;
9. if the product changes after the confirmation result is seen, that set is no longer a holdout and
   may not be reused to produce a PASS;
10. confirmation failure **blocks M2** and requires §4 lineage / C1.

This does not relax the frozen C2 challenge threshold; it is a stricter acceptance condition added
before any result was seen.

## A4. Selection-enrichment warning

The old locked set had 3 stalls in 8 seeds (37.5 %) against an analytic base rate of ≈ 2.7 % —
roughly **14× enrichment**. The replacement panel has the same property by construction: three seeds
chosen from 512 are, by definition, worst cases.

```
⇒ "3/3 recovered" is MECHANISM evidence, NOT PREVALENCE evidence.
```

This distinction is preserved in the default-ON justification and in any external report:
*"the mechanism recovers the selected stall phenotype"* — **not** *"cold start is solved"*. A
prevalence claim may come only from an unselected sample (the fresh panel).
