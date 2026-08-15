# DR-RATE-RAMP-1 — ROOT CAUSE v1

> Cold-start ladder OFF throughout. `fit_use = FORBIDDEN` · `result_bearing = false`. No fix applied.

## Results

| layer | condition | n | LOCKED | median max BtlBw (locked) | median max BtlBw (healthy) |
|---|---|---|---|---|---|
| RR1 replay *(debug only)* | GE-L10 p=20% RTT=1000 | 3 | **2** | **100 B/s** | 11,586 B/s |
| RR2 fresh high-loss | GE-L10 p=20% RTT=1000 | 40 | **1** (2.5%) | 624 B/s | 11,567 B/s |
| RR3 fresh low-loss control | IID p=5% RTT=1000 | 40 | **0** | — | 14,217 B/s |

Wilson 95% CI on the fresh high-loss lock rate: **[0.4%, 12.9%]**.

## Mechanism (from the RR1 deterministic replay — MECHANISM only, no prevalence claim)

Locked and healthy runs are **identical** at t = 1.10 s — both leave `PreConverge` on the first valid
ACK with `rate = 288`, `btlbw = 100`, `burst_cap = 101`, `pif = 1`. They then diverge:

| | locked (rep2) | healthy (rep16) |
|---|---|---|
| `pif` (packets in flight) | 1 → 9 → **1** | 1 → 135 → **199** |
| `burst_cap` | **101** (one sealed unit) throughout | 101 → 423 → **1200** |
| `btlbw` | **100 B/s** | 5,300 → 11,586 B/s |
| `send_rate` | 288 → **100** | 10,599 → **23,171** |
| bw samples offered / accepted | 58 / **58** | 362 / 361 |
| rejected app-limited / interval / invalid | 0 / 0 / 0 | 1 / 0 / 0 |

**Every step of the update chain completes correctly.** Samples ARE produced (58), ALL are accepted
(0 rejections of any kind), the estimator DOES move off zero, and the pacer DOES track the estimator
(`rate ≈ gain × btlbw`: 288≈2×100 and 23,171≈2×11,586). No single link is broken.

The defect is that **the loop's fixed point is the floor**:

```text
burst_cap = 101 (one sealed unit)
   -> at most one unit in flight per token grant  (pif = 1)
   -> the delivery-rate sample measures 101 B over one RTT  = 100 B/s
   -> BtlBw = 100 B/s   (exactly SEALED / srtt — the sender's OWN paced output, not the path)
   -> send_rate = gain x 100 = 288 B/s
   -> set_burst_cap(max(1 unit, rate x window)) keeps the cap at ONE UNIT
   -> (repeat)
```

`btlbw = 100 B/s` is arithmetically `101 B / 1.0081 s` — one sealed unit per RTT — the signature of
`pif = 1`. The healthy run escapes only because it accumulates enough in-flight units early (`pif` → 135)
for a sample to measure the real 125,000 B/s path.

## Class

```text
MECHANISM (replay, deterministic):   RAMP-ESTIMATOR-NOT-RESEEDED + RAMP-TOKEN-DEADLOCK (COUPLED)
                                     — two halves of one self-clocked feedback loop, not two defects.
                                     REFUTED by evidence: RAMP-NO-SAMPLE, RAMP-SAMPLE-REJECTED,
                                     RAMP-PACER-NOT-UPDATED, RAMP-HARNESS.
FRESH-CELL class:                    RAMP-TOKEN-DEADLOCK — **INSUFFICIENT-EVENTS** (n_lock = 1 < 4 gate)
CONDITIONALITY:                      RAMP-LOSS-CONDITIONAL (1/40 high-loss vs 0/40 low-loss),
                                     reported with the CI above; the low-loss control shows the chain
                                     completing correctly, so this is NOT RAMP-STRUCTURAL.
```

`RAMP-HARNESS` is refuted: in every locked run the harness offered all 384 units and drove
`Timer::Pto` every tick; the sender simply never raised its own clamp.

## Honest limits

* The fresh high-loss cell produced **one** qualifying event. Per the frozen minimum-event gate this is
  **below** the ≥4 threshold, so the fresh-cell class is **not** firm. At the observed 2.5% rate,
  **N ≈ 160** would be needed for 4 events.
* RR1 is a replay: it establishes the mechanism precisely and reproducibly, but **cannot** support any
  prevalence or generalization claim.
* No fix was applied, and no product behaviour was changed.

## Boundary of the product design work this implies (NOT a decision)

The observed loop is: *burst clamp derived from a rate that is itself derived from a sample taken under
that clamp.* Any remedy must break that circularity — e.g. a convergence-exit seed/reseed for the
bandwidth estimator, a minimum in-flight allowance so the first post-convergence samples can exceed one
unit per RTT, or excluding self-clocked samples from the max-filter. **Choosing among these is a product
design decision and is explicitly out of scope for this turn.**
