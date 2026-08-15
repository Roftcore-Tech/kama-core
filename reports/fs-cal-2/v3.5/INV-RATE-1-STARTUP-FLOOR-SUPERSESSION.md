# INV-RATE-1 — STARTUP FLOOR SUPERSESSION

> Written and hashed **BEFORE** any test was modified. Authorized by the PRODUCT REPAIR CONTINUATION
> directive §1A.1. HEAD at start: `4aa11a93af44b2b9e884461e8db3d1ed576c8b4f`.

## 1. The superseded test and the decision record that sealed it

```text
test    crates/adaptive-transport/src/sender.rs
        sender::tests::inv_rate_1_loss_declaration_preserves_phase_gain
seal    "[TUR-7 β1 KİLİT-GOLDEN — tez-koruması, Abkarada karar-2]"
        cift-kilit-v1 §0-ÖN/S1
```

## 2. The safety purpose the old invariant protects

`INV-RATE-1` (CLAUDE.md §4): **loss must NOT reduce `send_rate`.** Rate is paced from delivered-rate
(BtlBw) + queue delay; loss feeds only the FEC/ARQ recovery arm. The golden closed a specific blind spot:
before TUR-7 β1, declaring loss during the Startup ramp flipped the pacing phase gain from `2.885×` to
`1.0×` — an effective ×2.885 rate cut caused by loss. The test pins that a loss declaration
(`pending_lost > 0` → `RecoverArq` overlay) leaves the phase gain untouched, and that recovery exit
returns to the `Startup` **base** rather than collapsing to `Steady`.

**That purpose is unchanged and remains binding.**

## 3. Why the exact equality `rate == Gain::STARTUP × btlbw` no longer holds in all cases

The golden encodes the safety purpose as an **exact equality against one particular rate formula**:

```rust
assert_eq!(rate_before,        Gain::STARTUP.apply(btlbw));        // setup
assert_eq!(s.pacer.rate(),     Gain::STARTUP.apply(btlbw_after));  // the lock
```

DR-RATE-RAMP-2 adds a **bootstrap startup floor** at the `PreConverge → Startup` transition, because the
first post-convergence `BtlBw` is itself sampled under the 31 B/s PreConverge floor (one sealed unit per
RTT), producing a self-clocked fixed point (`BtlBw = SEALED/srtt`, `burst_cap` pinned at one unit). The
effective rate becomes the **safe composition**

```text
effective = max( phase_derived_rate , startup_floor )     where startup_floor <= cwnd/srtt
```

In the golden's fixture (`btlbw ≈ 2400`, `srtt = 500 ms`) the phase-derived rate is `6924 B/s` and the
grounded floor is `9600 B/s`, so the equality against the phase-derived value alone no longer holds.

## 4. Why the new floor does NOT violate the old safety purpose

1. **Direction.** `INV-RATE-1` forbids loss from **reducing** the rate. The floor is a `max(...)`: it can
   only ever **raise** the paced rate. It cannot produce a rate cut, which is the entire failure mode the
   golden exists to prevent.
2. **Loss cannot arm it.** The floor is armed **only** on the `PreConverge → Startup` transition. A loss
   declaration can neither arm nor raise it. Pinned by a new test.
3. **Phase gain untouched.** The floor does not modify `PacingPhase`, `Gain::STARTUP`, `base_is_startup`,
   or the recovery-overlay exit-to-base behaviour. The old "loss must not change the phase gain" property
   is re-asserted verbatim in the new active-floor test, expressed against the composed rate.
4. **The hard ceiling still has the last word.** `cwnd/srtt` (documented in `rate.rs` as the structural
   HARD ceiling, "son söz") bounds the floor by construction, so the floor can never push the sender past
   the congestion cap — including when that cap shrinks after loss.
5. **Bootstrap only.** The floor is released deterministically and permanently once the naturally derived
   rate catches up or the Startup/plateau exit condition fires; it cannot re-arm in later phases.

## 5. Disposition of the old test

**NOT deleted. Preserved verbatim, historically, and re-scoped.**
`inv_rate_1_loss_declaration_preserves_phase_gain` keeps its name, its fixture and both of its exact
equality assertions. One line is added that explicitly **disables** the startup floor, making it the
authoritative **legacy floor-inactive** pin: with the floor off, the old rate law must hold **exactly**,
byte for byte. Any future regression of the legacy path still fails this test.

## 6. Superseding tests (new binding contract)

| new test | pins |
|---|---|
| `inv_rate_1_loss_declaration_preserves_phase_gain` *(re-scoped)* | floor INACTIVE ⇒ old exact rate law holds unchanged |
| `inv_rate_1_active_floor_loss_does_not_reduce_rate` | floor ACTIVE ⇒ loss declaration does not reduce rate; rate equals the exact composition `max(phase_rate, floor)`; phase gain and exit-to-base unchanged |
| `startup_floor_arms_only_on_preconverge_exit` | only the `PreConverge → Startup` transition arms it |
| `startup_floor_not_armed_or_raised_by_loss` | loss declaration alone can neither arm nor raise it |
| `startup_floor_never_exceeds_cwnd_over_srtt` | hard ceiling has the last word, including after a shrinking cap |
| `startup_floor_writes_no_synthetic_btlbw_samples` | max-filter input count is unchanged by the floor |
| `startup_floor_release_is_deterministic_and_permanent` | released when natural rate catches up or startup exits; never re-arms |

No assertion is weakened to a loose `>=` where the old test used exact equality.

## 7. Status
```text
SUPERSESSION: AUTHORIZED (directive §1A.1) — recorded BEFORE any test edit
OLD TEST:     PRESERVED (re-scoped to the floor-inactive legacy path)
NEW CONTRACT: §1A.2, sealed by the tests in §6
```
