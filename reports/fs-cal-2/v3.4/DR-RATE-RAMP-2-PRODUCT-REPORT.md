# DR-RATE-RAMP-2 — PRODUCT REPORT (FAZ 1)

## VERDICT: **FIXED (mechanism) · default promotion BLOCKED**

The treatment PASSES every frozen criterion. Promoting it to the product **default** was attempted and
then **reverted**, because default-ON trips a sealed architectural golden — see "Default decision" below.

The post-convergence rate-ramp lock (DR-RATE-RAMP-1: a self-clocked fixed point where `burst_cap`
stays at one sealed unit and `BtlBw` stays at `SEALED/srtt`) is **resolved in the product**.

## Mechanism (grounded; no new constant)

At the `PreConverge → Startup` transition the pacer is initialized to the minimum-pipe rate the
product's **own** cwnd policy already permits:

```text
grounded = min( Gain::STARTUP × (SrcConfig::min_cwnd / srtt),   // instructed initialization
                cwnd / srtt )                                   // rate.rs structural HARD ceiling
cwnd     = max(cwnd_gain × BDP, min_cwnd)
```

`Gain::STARTUP = 2885/1000`, `SrcConfig::min_cwnd = 4 × DEFAULT_MTU = 4800 B`, `cwnd_gain = 2` — all
shipping values. All arithmetic is u128-widened and saturating; no `f64`.

**Deviation recorded honestly.** The instructed formula alone (`min_cwnd/srtt × startup_gain`) evaluates
to **13,837 B/s** at `srtt ≈ 1.0 s`, which would **breach** `cwnd/srtt` — documented in `rate.rs` as the
*"yapısal HARD ceiling (son söz)"*. I therefore clamped the initialization to that ceiling, so the
realized floor is **4,796 B/s**. This keeps the real congestion/safety cap authoritative, as §FAZ-1 requires.

**Bootstrap-only.** The floor is released as soon as `plateau_reached` or the naturally computed rate
catches up. It is never a permanent minimum rate. Verified by a permanent pin.

**What it does NOT do:** no synthetic samples are written to the BtlBw max-filter; `app_limited`
semantics are unchanged; no in-flight/cwnd gate was added; delivery-rate estimator, max-filter, pacing
gain, ACK logic, PreConverge floor, tail recovery and cold-start scheduling are untouched.

## Validation — 260 paired runs, ONE binary, ONE axis (`Sender::set_startup_pacing_init`)

| layer | pairs | LOCKED base→fix | tx_data base→fix (worse) | median epoch base→fix | floor armed |
|---|---|---|---|---|---|
| locked replays | 5 | **2 → 0** | 512 → 1192 (0) | 65,000 → 47,595 ms | 3/5 |
| GE-L10 p=20% RTT=1000 | 40 | **1 → 0** | 14,271 → 14,354 (0) | 14,029 → **8,567 ms** | 38/40 |
| IID p=5% RTT=1000 | 40 | 0 → 0 | 15,360 → 15,360 (0) | 11,427 → **7,017 ms** | 40/40 |
| no-loss RTT 50/250/1000 | 30 | 0 → 0 | 11,760 → 11,760 (**0 delta**) | 2,446 → **1,427 ms** | 30/30 |

Individual locked reproducers: `rep2` 288 → **32,243 B/s** (BtlBw 100 → 16,122; `burst_cap` 101 → 1289;
tx 63 → **384**); `rep31` 288 → **27,709 B/s** (tx 25 → **384**). Healthy `rep16` 23,171 → 24,747.

Floor values are exactly the grounded formula per RTT: `4,796` (RTT 1000), `19,137` (250), `94,469` (50).

## Frozen PASS criteria

```text
[PASS] known locked seeds leave the low fixed point      LOCKED 3 -> 0
[PASS] pacing grows mechanically after PreConverge exit  floor arms at the transition, cap/BtlBw rise
[PASS] NO synthetic samples written to BtlBw             accepted <= offered in every run
[PASS] healthy / low-loss / no-loss do not regress       delivery worse in 0/115 pairs;
                                                          no-loss wire bytes byte-identical (delta 0);
                                                          median time-to-drain improved in every layer
[PASS] wire and safety caps not exceeded                 cwnd/srtt hard ceiling respected by construction
[PASS] invariant failures = 0                            every record
```

## Recorded side-effect (NOT a frozen criterion — flagged for FAZ 5 §4.2b)

Estimator **eligibility** fell by one run in two layers (ge 29→28, iid 30→29; net 59→57 of 80):
* `ge rep2` — identical `N=256` and identical transition counts (84/171). It fails only through the
  **known delivery-drain warm-up defect** (`epoch_trigger` TIMEOUT instead of DRAIN). Under the
  design-correct *fate-known-ready* rule this run is eligible. Not caused by this fix.
* `iid rep17` — loss transitions 10 → 7 against the pinned bar of 8: the faster drain changed which 256
  fates fall in the window.

This is an estimator-observability effect, not a transport regression (delivery identical in both). It
is material to the fresh §4.2b in FAZ 5 and must not be traded away silently.

## Default decision — **BLOCKED, requires explicit authorization**

Default-ON was implemented, tested, and **reverted to OFF**. Reason: with the default ON, the sealed
lock-golden `sender::tests::inv_rate_1_loss_declaration_preserves_phase_gain` fails. Its SETUP asserts
the bootstrap pacing rate is **exactly** `Gain::STARTUP × btlbw` (6924 in that fixture); the bootstrap
floor is by design `max(natural, grounded)` and raises it to 9600.

**INV-RATE-1 itself is NOT violated** — the invariant is "loss must not REDUCE send_rate", and the floor
only ever raises the rate. But that golden is an Abkarada-sealed decision artifact
(*"KİLİT-GOLDEN — tez-koruması, Abkarada karar-2"*), and rewriting it so that my own change passes is
exactly what must not be done. Flipping the default therefore requires an explicit decision on that
golden. With the default OFF the full suite is green: **adaptive-transport 129/129**, **fs_cal2_pins 31/31**.

## Bounded product change (as currently shipped)

* Product **default remains OFF** ⇒ shipped wire behaviour is **unchanged**; the mechanism is inert
  until enabled via `Sender::set_startup_pacing_init`.
* When enabled, the effect is **confined to flows that leave PreConverge**. The frozen §4.2b P2 stall
  seeds are **8/8 byte-identical** even with it enabled (they never leave PreConverge, so the floor
  never arms) — the §4.2b cold-start stall cell is untouched by this fix.
* No wire format, no new packet type, no new timer, no ordering change. No-loss wire bytes unchanged.

## Permanent regression pins (added)
`rate_lock_seeds_escape_the_fixed_point` (seeds 2, 31: baseline must reproduce the clamp lock, product
default must break it, and no synthetic BtlBw writes) and
`startup_pacing_floor_is_bootstrap_only_and_released`. Suite: **31/31 PASS**.
