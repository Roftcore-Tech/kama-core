# AŞAMA 1 (EK4-A2) — RESUME CHECKPOINT

**Status: `IN PROGRESS — root cause CONFIRMED and now EXPOSED by the new API; acceptance NOT met.`**
Startup pacing therefore stays **default-OFF**. No half-finished behaviour is default-ON.

HEAD `4aa11a93` · no commit/push/PR · calibration result-bearing runs **0**.

---

## Completed in this turn

### §1.2 inventory — DONE, hashed **before** any code change
`EK4-A2-NEXT-WAKEUP-CONTRACT.md` `d59212b2…` / `.json` `18dffa48…`

**Root cause, confirmed from code:** `next_wakeup_ns()` was
`#[cfg(feature = "experimental-cold-start")]` and returned `min(cs_wakeup)` — **one** source out of
six, scoped to the DR-CS-1 cold-start registry (`sender.rs:116`), and absent from the default build
entirely. In the EK4-A2 scenario the outstanding unit had already been **declared lost** (moved from
`loss.inflight` into `pending_lost`), so `loss.next_time_loss_deadline_ns(..)` returned `None`,
`BootstrapPto` was never pushed, and `cs_wakeup` was empty ⇒ `None` while work was outstanding.

The six authoritative sources are enumerated in the artifact (CS timers · ARQ time-threshold ·
geometric copies · bootstrap ladder · tail PTO · pacer token refill).

### §1.4 minimal fix — IMPLEMENTED
* `pacer.rs`: **read-only** `ns_until_sendable(size)` — the exact integer inverse of `refill()`
  (same sub-byte fixed-point, ADR-17 compliant). `None` when tokens can never reach `size`.
* `tail_recovery.rs`: **read-only** `armed_at_ns()` accessor.
* `sender.rs`: `next_wakeup_ns()` **un-gated** (default build) and now returns the **earliest**
  across the existing authoritative sources. Split into:
  * `next_timer_deadline_ns()` — sources 1–5 plus `LostRec.deadline_ns`,
  * `pending_work_ready_ns()` — source 6, restricted to the classes drained **unconditionally**
    every turn (`tx_queue`, `pending_fec`).
* **No new timer, no second truth source**, no change to PTO backoff/retries, the startup pacing
  formula, or FEC/tail/bootstrap behaviour.
* **C2 kept at its original scope** (`cs_wakeup` only). It is a property of the CS registry's
  retire-on-fire discipline; extending it to the broader set was my error and was reverted.

### Two defects of my own, found and fixed by the tests
1. Source 6 initially included `pending_lost` — whose emission is **state**-gated
   (`plan_arq_retransmits` runs only in `RecoverArq`), so pacer-readiness alone produced
   progress-free wakeups: **exactly the busy-loop §1.1 forbids.** Fixed by reporting `pending_lost`
   through its own authoritative `LostRec.deadline_ns` instead.
2. The upgraded test helper asserted `wk >= t`; an **overdue** deadline is legitimate ("due now").
   Fixed to advance to `max(wk, now) + ε`.

### Test state

| combination | passed | failed |
|---|---|---|
| `<default>` | **244** | 0 |
| `experimental-fec-budget` | **291** | 0 |
| `diag-sched-telemetry` | **244** | 0 |
| `experimental-cold-start` | 158 | **1** |
| `--all-features` | 198 | **1** |

clippy `-D warnings` on `adaptive-transport --all-targets`: **clean**.

3 of the 4 EK4-A2-family tests that the new API initially broke now pass
(`f2b_di1`, `f2b_di1a`, `f2b_di2`).

---

## THE OPEN ITEM (exact next step)

**`dr_cs1_acceptance::f2b_di1c_rearmed_pto_lifecycle_exposure` fails**, and it now fails for a
*substantive* reason rather than a missing accessor:

```
retransmit exposed-deadline zincirinde fire etmedi (yeni-network-event-yok)
```

The §1.3-style wakeup-driven driver (no periodic tick, empty batches only) follows up to **64**
successive `next_wakeup_ns()` deadlines and the retransmit of unit 1 **never fires**.

`next_wakeup_ns()` no longer returns `None` — so the *reporting* gap is closed — but the chain does
not make progress. **Leading hypothesis, to verify first on resume:** `pending_lost` exposes
`LostRec.deadline_ns`, yet `plan_arq_retransmits` only runs when
`state == SenderState::RecoverArq` (or on the `K_ARQ_SHARE` tick) — so waking exactly at that
deadline yields no emission, and the deadline is re-reported unchanged. That is a **real liveness
gap**, and it is precisely what EK4-A2 exists to catch.

If confirmed, the §1.4-compliant fix is to make the reported deadline reflect the instant at which
the item can actually be planned (i.e. derive it from the same state the planner gates on), **not**
to add a timer, relax the planner's state gate, or reintroduce a periodic tick.

### Not yet started in AŞAMA 1
§1.3 standalone reference-runtime driver artifact · §1.5 full battery (locked replay, FEC ordering,
FS-CAL pins, kama-integration, feature matrix re-run) · §1.6 default restoration.

---

## Product defaults — all safe

| config | default |
|---|---|
| startup pacing | **OFF** (§A3.2/§A4 revert; Stage-1 acceptance not met, so NOT restored) |
| bootstrap RTT | **OFF** |
| tail recovery | **OFF** |
| FEC adaptive policy | **OFF** |

## Decisions already made — do NOT re-ask

* `next_wakeup_ns()` is the general event-loop deadline API and lives in the **default build**.
* It aggregates the **earliest** of the existing authoritative sources; it never invents a timer.
* C2 stays scoped to `cs_wakeup`.
* Immediate readiness is reported as `Some(now)`; a driver that drains then re-queries must converge.
* Source 6 covers only unconditionally-drained classes; state-gated classes report via their own
  deadlines.

## Stages 2–13 — not started

CI semantic matrix · panel contract V2 · bootstrap validation · tail-v2 · estimator wiring · runtime
ADR · Noise handshake · UDP runtime · process-level and two-netns E2E · debt ledger · final
re-grounding. All gated behind AŞAMA 1 acceptance.
