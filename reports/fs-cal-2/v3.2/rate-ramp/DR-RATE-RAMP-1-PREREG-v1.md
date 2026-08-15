# DR-RATE-RAMP-1 — PREREGISTRATION v1

> Separate namespace / manifest / artifacts from DR-CS-2. Cold-start ladder flag stays **OFF** here.
> `fit_use = FORBIDDEN` · `calibration_use = FORBIDDEN` · `result_bearing = false`

## C1 Question
After the sender receives a valid ACK/SACK and leaves `PreConverge`, why does the pacing rate in some
flows lock near a low floor and fail to rise on subsequent delivery/bandwidth evidence?

## C2 Design
```text
RR1  deterministic replay — known rate-lock run/seed + known healthy control from the same high-loss
     cell. DEBUGGING ONLY: establishes MECHANISM; produces NO prevalence or generalization claim.
RR2  fresh high-loss   GE-L10, p=20%, RTT=1000 ms, baseline (ladder OFF), N=40
     namespace DR-RATE-RAMP-1-HIGHLOSS-V1
RR3  fresh low-loss    IID,    p=5%,  RTT=1000 ms, baseline (ladder OFF), N=40
     namespace DR-RATE-RAMP-1-LOWLOSS-V1     (disjoint seed domain from RR2)
total fresh = 80 runs
```
The low-loss control separates "lock only under sustained burst loss" from "structural lock in the
convergence → sample → estimator → pacer chain".

## C4 Mechanical lock predicate (frozen BEFORE outcomes; not a post-hoc speed threshold)
Audit the sender's own update chain:
```text
valid delivery sample produced -> estimator accepted/rejected -> new bandwidth state
  -> applied to pacer -> token refill/emission behaviour changed
```
```text
LOCKED := the flow LEFT PreConverge
          AND max(burst_cap) == SEALED (101 B)
```
`burst_cap` is exactly the variable written by `TokenBucket::set_burst_cap(max(1 unit, rate × window))`,
so the predicate states: *the channel-derived burst clamp never released past its one-unit floor.*
Corroborating observation (recorded, not part of the predicate): BtlBw settles at ≈ one sealed unit per
RTT — i.e. the estimator's fixed point is the sender's own paced output rather than the path.

## Minimum-event gate (frozen)
A **fresh-cell** root-cause class requires **≥ 4** qualifying lock events (mirrors the DR-CS-2 B6 gate).
Below that, the fresh-cell class is reported as `INSUFFICIENT-EVENTS` and only the replay mechanism stands.

## C5 Classes
`RAMP-NO-SAMPLE` · `RAMP-SAMPLE-REJECTED` · `RAMP-ESTIMATOR-NOT-RESEEDED` · `RAMP-PACER-NOT-UPDATED` ·
`RAMP-TOKEN-DEADLOCK` · `RAMP-LOSS-CONDITIONAL` · `RAMP-STRUCTURAL` · `RAMP-HARNESS` · `RAMP-MIXED` ·
`RAMP-AMBIGUOUS`. No rate-ramp fix is applied in this turn.
