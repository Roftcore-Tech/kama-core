# Q18 — CONFIRMED CALIBRATION DEFECT (armed, not fired)

> The shipped fuzzy controller plans **zero parity** at exactly the loss level where measurement
> says coding is the entire recovery mechanism.

Measured directly, not relayed. Pinned by `crates/adaptive-transport/tests/q18_check.rs`.

## 1. The defect

    loss = 200_000 ppm (20%), bw 125 kB/s  ->  fec_parity_ratio_permille = 0
    loss = 200_000 ppm (20%), bw 175 kB/s  ->  fec_parity_ratio_permille = 0
    loss = 250_000 ppm (25%)               ->  fec_parity_ratio_permille = 250
    loss = 500_000 ppm (50%)               ->  fec_parity_ratio_permille = 500

Cause: `fusion.rs` sets the loss-LOW shoulder to `shoulder_low(200_000, 300_000)`, i.e.
`Trap { a: 0, b: 0, c: 200_000, d: 300_000 }`. Membership in LOW is a full **1.0** up to 200_000,
so 20% loss reads as entirely LOW and rule R1 plans `fec_zero`. Raising bandwidth into the R2
HIGH region does not open it either, so it is not a bandwidth artefact — it is the shoulder.

## 2. Why that number is the wrong place to put a knee

FS-FUSION-CELL-1 measured the same 20% IID cells:

| cell | tail-capable ARQ alone | with coding |
|---|---|---|
| rtt50-iid20 | **0/12** | 9/12 |
| rtt250-iid20 | **0/12** | 5/12 |
| rtt1000-iid20 | **0/12** | 9/12 |

**The knee and the need sit at the same value, on opposite sides.** At 200_000 ppm the controller
decides coding is unnecessary; at 200_000 ppm the measurement says coding is the only thing that
completes anything at all.

## 3. Exposure — armed, not fired

| component | state |
|---|---|
| FEC actuation default | `Manual { enabled: false }` — **OFF** (`sender.rs:1261`) |
| `FecActuationMode::Controller` | present and reachable in the product path (`sender.rs:3064-3197`) |
| the fuzzy controller itself | **constructed inside every `Sender`** (`sender.rs:1002`, `:1230`) |
| calibrated `fusion_policy.rs` | 659 LOC, **zero call sites** — dead code |
| `SpineArm::A0Tail { r }` | test harness only, absent from `src/` |

So nothing ships this behaviour today. But the controller is not dormant code sitting on a shelf —
it is built into every `Sender` instance. The first caller to select
`set_fec_actuation_mode(FecActuationMode::Controller)` arms this decision, with no further change
and no warning.

**This is why the defect is recorded rather than deferred quietly: turning FEC actuation on is a
one-line change that a future session could reasonably make while believing the calibration behind
it is sound.**

## 4. What must NOT be concluded

* Not a product regression: no shipped default produces it.
* Not a claim that the controller is broken in general — the mechanism works further out (250_000
  and 500_000 both open parity correctly). It is a **boundary placement**, not a dead rule.
* Not a promotion or recalibration decision. Moving the shoulder requires a preregistered campaign;
  this document only fixes what is currently true.

## 5. Corroboration from the other direction

FS-FUSION-CAL-1 Q17 had already proved, by preregistered group-disjoint holdout, that **loss rate
alone cannot gate FEC activation**: at an identical 200_000 ppm, IID structure wants coding and
burst structure wants ARQ, and a rate-only boundary collapsed to a degenerate 199_999 -> 200_000 gap
with one harmful false positive.

FS-FUSION-CELL-1 reached the same conclusion from the opposite side: same loss level, opposite
answer depending on structure.

Two independent lines of evidence therefore point at the same fix, and it is not "move the knee":
**the gate needs a loss-structure input, not a finer loss-rate threshold.** `fate_estimator.rs`
already computes exactly that (`burst_delta_ppm = p11 - p01`) and was wired into the crate in
996fc9f6; it currently has no product consumer.
