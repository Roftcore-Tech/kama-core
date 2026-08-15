# FS-CAL-2 — DOWNSTREAM BLOCKERS v2

FS-CAL-2 is now a **downstream consumer**. It cannot be finally frozen until the two product studies
below conclude. This file supersedes nothing; it records downstream correctness after the v3.2 diagnosis.

## Standing status

```text
ARQ-CONTRACT-PASS                      STILL VALID (re-verified; tail ON and identical in both arms)
PRODUCT-ESTIMATOR-INTEGRATION-FEASIBLE  FEASIBLE is NOT a PASS
FS-CAL YETKİ-2                          NOT AUTHORIZED
result-bearing calibration runs          0
product behaviour                        NOT CHANGED
```

### `PRODUCT-ESTIMATOR-INTEGRATION-FEASIBLE` is **not** `PASS`
Still to be built before it can become a PASS:
1. A **real product caller** for the estimator (today `fate_estimator` has none; `fusion_policy` has none either).
2. The **ordinal counter** (logical source ordering) the estimator needs in-product.
3. A **grounded telemetry operand** — `FusionObservation::telemetry_age_ns` has **no producer**
   anywhere outside a `#[cfg(test)]` fixture, so the 2 s staleness bound is a grounded threshold with an
   ungrounded operand and **must not** be used as an architectural finding.

### Contract corrigendum
`FS-CAL-2-PRODUCT-CONTRACT-v3.json` records a **stale** `fs_cal2_spine.rs` sha256 and must be regenerated.

## Invalidation rule (hard)

If DR-CS-2 **or** the rate-ramp remedy changes `Sender` behaviour, ALL of the following are invalidated
and require re-grounding and re-freezing:

```text
arq_contract_id · product contract · golden traces · M1/M2 · DATA-PROCESSABLE · S0 · §4.2b
```

## Mandatory order

```text
product fix
  -> re-run the YETKİ-1 grounding chain
  -> fresh S0
  -> fresh §4.2b
  -> ONLY on PASS: FS-CAL YETKİ-2
```

FS-CAL v3.2 **cannot be finally frozen** until both product studies complete.

## Blocker state after this turn

| blocker | state |
|---|---|
| **DR-CS-2** (cold-start liveness) | **Sizing NOT EXECUTED** — `LADDER-NOT-ISOLATABLE`. The existing experimental ladder cannot be enabled as a single-axis treatment (no runtime switch; reroutes the event pipeline; self-generates `Timer::Pto`; shares the DATA token bucket; **no product-path echo responder**). The cold-start scope question is therefore still **open**, and answering it now requires a product decision, not another experiment. |
| **DR-RATE-RAMP-1** (post-convergence rate lock) | **Root-caused.** Mechanism (replay): coupled `RAMP-ESTIMATOR-NOT-RESEEDED` + `RAMP-TOKEN-DEADLOCK` — a self-clocked loop whose fixed point is one sealed unit per RTT. Fresh high-loss 1/40, low-loss 0/40 ⇒ `RAMP-LOSS-CONDITIONAL`, but the fresh-cell class is **INSUFFICIENT-EVENTS** (n=1 < 4). No fix applied. |
