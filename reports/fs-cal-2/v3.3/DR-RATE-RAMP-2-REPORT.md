# DR-RATE-RAMP-2 — REPORT (FAZ 2)

## VERDICT: **`MIN-PIPE-MAPPING-NOT-GROUNDED`** — treatment NOT implemented, phase **STOPPED** at §2.1

Per §2.1 the `RR-MIN-PIPE` treatment may only be built if Kama has a variable **equivalent to an
authoritative in-flight volume cap**. It does not. **No product behaviour was changed.**

## §2.1 Grounding — what each variable actually is

| question | answer (grounded) |
|---|---|
| What does `burst_cap` limit? | The **token-bucket burst quantum** in `pacer.rs` (`TokenBucket{rate_bps, tokens, burst_cap, refill_rem}`). Derived per event as `clamp(send_rate × BURST_WINDOW_NS, front_size, max(front_size, 2×DEFAULT_MTU))`. |
| Is it an in-flight volume / cwnd equivalent? | **NO.** It caps how many bytes may be released in one paced burst, not how many bytes may be outstanding. |
| Is there an authoritative in-flight volume cap? | **NO gate exists.** `cwnd` lives in `rate.rs` but is only converted to a *rate*: `cwnd = max(cwnd_gain×BDP, min_cwnd)`, `send_rate = min(pacing_rate, cwnd/srtt)`. **No code compares `inflight_bytes` against `cwnd` to block a transmission** — every `inflight_bytes` use is init/increment/decrement/averaging/diagnostics. The only emission gate is the pacer token bucket. |
| How is `pif` computed? | `bw_sampler.packets_in_flight()` — an accounting counter, not a gate. |
| Under which send limit is the delivery sample formed? | Under the **pacer** limit. With `burst_cap = 101 B` only one sealed unit is released per grant, so the sample measures 101 B over one RTT. |
| How does a sample taken under the cap regenerate the same cap? | `burst_cap ← clamp(send_rate × window, …)`, `send_rate ← gain × BtlBw`, `BtlBw ← max-filter(samples)`, and the samples are produced under that same `burst_cap`. The loop's fixed point is one sealed unit per RTT. |

## Why a min-pipe floor cannot work here (measured, from the locked replay)

```text
locked run:  BtlBw = 100 B/s   srtt = 1.0008 s   observed send_rate = 100   burst_cap = 101 B

BDP               = BtlBw × rtprop                = 100 B
cwnd              = max(2×BDP, min_cwnd)          = max(200, 4800) = 4800 B   <- min_cwnd ALREADY binds
inflight_cap_rate = cwnd / srtt                   = 4796 B/s
send_rate         = min(pacing_rate, cap_rate)    = min(100, 4796) = 100 B/s
                                                    => the cwnd ceiling is NON-BINDING by 48x
```

Two independent disqualifications:

1. **A MinPipeCwnd analog already exists and is already non-binding.** `SrcConfig::min_cwnd = 4 ×
   DEFAULT_MTU = 4800 B` (`rate.rs`, "[STD:policy 4×MTU]"). In the locked state it *does* determine
   `cwnd` (4800 > 2×BDP=200) — yet the resulting ceiling (4796 B/s) is 48× above the binding term.
   The binding term is the **pacing rate derived from BtlBw**, not the cwnd ceiling. Raising a floor
   that is already 48× slack cannot move `send_rate`.
2. **The proposed value is smaller than what already ships.** "Four source units" = 4 × 101 = **404 B**,
   versus the existing `min_cwnd` = **4800 B** — the proposal is **11.9× smaller**. Implementing it as
   specified would be a *reduction*, and still non-binding.

Even if a true in-flight gate were added, it would not by itself break the loop: the lock is in the
**BtlBw max-filter fixed point**, which §2.2 explicitly places out of scope for this phase
("`model_limited` sample'ları max-filter'dan çıkarmak ayrı ve henüz kanıtlanmamış bir treatment'tır;
bu fazda uygulama").

## §2.2 compliance
No `app_limited` reinterpretation was implemented, and no `model_limited` max-filter exclusion was
applied. The BBR `app_limited` filter was **not** repurposed for the model-limited condition.

## What was NOT done
`RR-MIN-PIPE` was not built. No delivery-rate estimator, max-filter, pacing gain, ACK logic,
PreConverge, tail-recovery or cold-start scheduling code was touched. The §2.4 validation matrix was
not run, because there is no treatment to validate.

## Required next step (separate design checkpoint — NOT taken here)
A design decision is needed on which single axis may be opened:
* introduce a genuine in-flight volume gate (`inflight_bytes` vs `cwnd`) — a new congestion-control
  surface, not a floor tweak; or
* break the measurement loop at the estimator (exclude self-clocked / model-limited samples from the
  max-filter) — explicitly out of scope for this phase; or
* seed/reseed BtlBw at the PreConverge→Startup transition.

Each is a distinct treatment and must be frozen before data.
