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
