# DR-RATE-RAMP-2 — PRODUCT REPORT (FAZ 1A)

## RESULT: `RATE-RAMP FIXED · DEFAULT-ON · SUPERSESSION VALID`

The post-convergence rate-ramp lock is fixed **in the product**, shipping **default-ON**, with the
INV-RATE-1 lock-golden **formally superseded** rather than silently rewritten.

## 1A.1 Supersession (written and hashed BEFORE any test edit)

```text
INV-RATE-1-STARTUP-FLOOR-SUPERSESSION.md    38a3fc74eb935a9aee5b2e6cdcf346d957a7b5653f72876a80a7aabd67e3d131
INV-RATE-1-STARTUP-FLOOR-SUPERSESSION.json  1b716d3540bb39f487afd3e5dff30c6efb46484ad263590a1d5a88a24d2db03c
```

The old golden `inv_rate_1_loss_declaration_preserves_phase_gain` is **preserved verbatim** — same name,
same fixture, both exact equalities — and re-scoped to the authoritative **floor-INACTIVE legacy** path.
No assertion was weakened to a loose `>=`.

## 1A.2 New binding invariant contract (6 new tests, all green)

| test | pins |
|---|---|
| `inv_rate_1_loss_declaration_preserves_phase_gain` *(re-scoped)* | floor inactive ⇒ old exact rate law holds |
| `inv_rate_1_active_floor_loss_does_not_reduce_rate` | floor active ⇒ rate == exact `max(phase_rate, floor)`; loss changes neither the phase gain nor exit-to-base |
| `startup_floor_arms_only_on_preconverge_exit` | only that transition arms it |
| `startup_floor_not_armed_or_raised_by_loss` | loss can neither arm nor raise it |
| `startup_floor_never_exceeds_cwnd_over_srtt` | hard ceiling has the last word at **every application** |
| `startup_floor_writes_no_synthetic_btlbw_samples` | max-filter sample count and BtlBw identical to the floor-off run |
| `startup_floor_release_is_deterministic_and_permanent` | released on the Startup-exit phase change; can never re-arm |

Two implementation defects were found **by these new tests** and fixed:
1. **Ceiling was only applied at arming.** Now `cwnd/srtt` is re-evaluated and clamps the floor at
   *every* application, so a shrinking congestion cap (e.g. after loss) shrinks the floor with it.
2. **Release signal was unobservable.** `plateau_reached` is *consumed* by `update_state` in the same
   call, and `base_is_startup` is *re-armed* on every event while `state == Startup`. The durable signal
   is the phase itself: release when the sender leaves the Startup ramp (and its recovery overlays) for
   a post-ramp phase. An `armed_once` latch makes the release permanent (no Plateau/ProbeBW re-entry).

## 1A.3 Default promotion

`StartupPacingConfig::default()` → **`enabled: true`**.

```text
adaptive-transport lib          135/135 PASS   (incl. the 7 tests above)
kama-integration fs_cal2_pins    33/33 PASS   (incl. default_on_equals_explicit_on_treatment,
                                               explicit_off_differs_from_default_on)
scoped clippy -D warnings        clean x3 (default / diag-sched-telemetry / integration)
```

**Default ≡ explicit treatment** is pinned exactly: `startup_pacing_init: None` (product default) and
`Some(true)` produce identical `wire_digest`, `source_prefix_digest`, decision epoch and estimator window
on seeds 2/16/31. The axis is proven real: explicit-OFF differs.

The release condition changed semantically during 1A.2, so the frozen paired panel was **re-run in full**
(115 pairs / 260 runs, new namespace `reports/fs-cal-2/v3.5/panel/`):

| layer | pairs | LOCKED base→fix | tx base→fix (worse) | median epoch base→fix |
|---|---|---|---|---|
| locked replays | 5 | **2 → 0** | 512 → 1192 (0) | 65,000 → 47,595 ms |
| GE-L10 p=20% RTT=1000 | 40 | **1 → 0** | 14,271 → 14,354 (0) | 14,029 → **8,567 ms** |
| IID p=5% RTT=1000 | 40 | 0 → 0 | 15,360 → 15,360 (0) | 11,427 → **7,017 ms** |
| no-loss RTT 50/250/1000 | 30 | 0 → 0 | 11,760 → 11,760 (**0**) | 2,446 → **1,427 ms** |

```text
[PASS] locked replay seeds leave the fixed point       LOCKED 3 -> 0
[PASS] delivery never regresses                        0/115 pairs worse
[PASS] no-loss wire / accounting unchanged             delta 0
[PASS] NO synthetic BtlBw samples                      accepted <= offered everywhere
[PASS] floor released by end of run (bootstrap-only)   0 runs left armed
[PASS] floor only takes grounded per-RTT values        {4796, 19137, 94469} = min_cwnd/srtt per cell
```

## Bounded product change

* **Default state: ON.** Product wire behaviour now includes the bootstrap pacing initialization.
* Change is **confined to flows that leave PreConverge**; the frozen §4.2b cold-start stall seeds are
  **8/8 byte-identical** (they never leave PreConverge, so the floor never arms).
* No wire format, no new packet type, no new timer, no ordering change; no-loss wire bytes unchanged.
* ARQ base unchanged; the single axis changed is the pacer's initial rate at the PreConverge exit.

## Recorded side-effect (carried into FAZ 5)
Estimator eligibility moved 59→57 of 80 in the previous panel (`ge rep2` fails only via the known
delivery-drain warm-up defect — identical N/transitions; `iid rep17` loss-transitions 10→7 vs the pinned
bar of 8). Observability, not transport. Must be re-checked by the fresh §4.2b.
