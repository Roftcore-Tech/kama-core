# DR-CS-3 — COLLATERAL FINDING: sealed leftover-equivalence pin fails under rate-ramp default-ON

**Status: `REGRESSION FOUND · ROOT-CAUSED · NOT FIXED (out of scope, needs an explicit decision)`**

This is **not** a DR-CS-3 panel result. It was uncovered while running the full
`cargo test -p adaptive-transport` gate for this turn, and it invalidates part of an earlier
"all green" claim.

## What happened

`crates/adaptive-transport/tests/budget_equivalence_pin.rs` (Commit-W leftover-equivalence pin) had
**stopped compiling under default features**:

```
error[E0004]: non-exhaustive patterns: `TransmitKind::CsProbe` not covered
```

The match arm for `TransmitKind::CsProbe` carried `#[cfg(feature = "experimental-cold-start")]`, but
DR-CS-3 **un-gated** the variant so the shipping bootstrap emitter could live in the default build.
The test target therefore silently stopped building — and a broken build is not a test failure, so
the earlier suite runs that reported green were not exercising this pin at all.

**Fix applied (test-only, one line):** the `#[cfg]` was removed from that arm, restoring compilation.
This is unambiguously correct — the variant is un-gated, so gating the arm makes the match
non-exhaustive.

## What the restored pin then revealed

With compilation restored the pin **fails on behaviour**:

```
thread 'leftover_equivalence_pin_constants' panicked at budget_equivalence_pin.rs:72:
pin: DATA-önce-parity
```

Isolated with a temporary diagnostic (since removed), toggling only the startup-pacing axis:

| configuration | decision-kind sequence | `last_data` | `first_par` | pin |
|---|---|---|---|---|
| startup pacing **OFF** | `[0,0,0,0,0,0,3,3,3]` | 5 | 6 | **passes** |
| startup pacing **ON** (the current product default) | `[0,0,0,0,3,3,0,0,3]` | 7 | 4 | **fails** |
| product default (no override) | `[0,0,0,0,3,3,0,0,3]` | 7 | 4 | **fails** |

(`0` = Data, `3` = FecParity.)

## Root cause

**DR-RATE-RAMP-2 / FAZ-1A default-ON**, not DR-CS-3. The bootstrap probe is inert in this scenario
(`BootstrapRttConfig` default OFF, and the scenario never enters the bootstrap gate). The startup
pacing floor changes the pacer token trajectory: only 4 of the 6 DATA units fit before the parity
drain, so the 5th and 6th DATA are emitted in later PTO turns, *after* parity has already gone out.

The pin's other four assertions still hold: `enqueued_r_total=3`, `emitted_r_total=3`,
`(data_count, parity_count)=(6,3)`, `total_bytes=6·1200+3·400`, `max_seq=8`. Only the **global**
ordering assertion `last_data < first_par` fails.

## Why it is NOT fixed here

* §0 of this turn forbids changing the startup-pacing axis.
* Weakening or re-baselining a **sealed** pin to make it pass is exactly the "change the criterion
  after seeing the result" move that is forbidden.
* The pin asserts a *global* ordering across ~47 scheduling turns. The sealed DATA-first invariant is
  a *per-turn* property, and per-turn DATA-before-parity still holds here (the deferred DATA and the
  parity emissions are in different turns). Whether the pin's global form is still the intended
  contract under a startup pacing floor is a **specification question**, not something to decide
  post-hoc while chasing a different gate.

## Consequence for earlier claims

The earlier DR-RATE-RAMP-2 validation reported no regressions. That conclusion did not cover this
pin, because the target was not compiling at the time. The rate-ramp default-ON change should be
treated as **having one known open regression** until this is resolved.

## Decision required (Abkarada)

One of:

1. **Amend the pin** to the per-turn DATA-first invariant it is actually protecting, with a recorded
   supersession (the same versioned-supersession discipline used for INV-RATE-1 / INV-CS-4); or
2. **Change startup pacing** so the global ordering is preserved; or
3. **Accept and record** the ordering change as intended, superseding the sealed constant explicitly.

Until then the tree contains one failing test in `adaptive-transport`
(`budget_equivalence_pin::leftover_equivalence_pin_constants`). It is left **failing and visible**
rather than hidden behind a compile error.
