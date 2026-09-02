# DR-CS-3 — DISPLACEMENT METRIC CONTRACT

Frozen and hashed **before** any new measurement. Replaces the single `data_displacement` number,
which was not authoritative.

## Why the old metric was wrong

It compared DATA emission ticks up to `first_echo_tick`, and when no echo ever arrived that horizon
became `u64::MAX` — silently diffing two entire trajectories that legitimately differ. It reported
`disp=376..382` on runs whose treatment arm emitted at most 3 probes, which cannot displace 380 DATA
emissions. Those readings were an instrument artefact, not product displacement.

## Two distinct quantities

### 1. `scheduler_data_displacement`

DATA the treatment genuinely failed to emit **because of the treatment**, measured on the window
where the two arms MUST be identical: strictly **before the first Probe reaches the wire**.

* window: emission ticks `< first_probe_tick`
* value: count of ticks where the baseline emitted DATA and the treatment did not
* **must be 0.** Before any extra packet exists there is no legitimate source of divergence, so a
  non-zero value is a real scheduler defect.

### 2. `channel_fate_divergence`

How often DATA fates differ **after** an extra Probe/Echo has entered the shared packet-domain
chain.

* window: emission ticks `>= first_probe_tick`
* value: count of differing ticks
* **expected non-zero and not a defect.** One extra packet advances the shared GE chain by one step,
  which legitimately re-phases every subsequent fate. This is real packet-domain physics and must
  never be labelled DATA displacement.

### Echo-bounded quantities

Any metric whose window depends on an Echo arriving reports **`N/A — NO ECHO`** when none arrives.
It never silently expands to the whole trace. Terminal-horizon divergence may be reported separately
and explicitly.

## Positive controls (required)

1. **pre-treatment divergence = 0** — with bootstrap ON, no divergence before the first Probe.
2. **no extra packets ⇒ whole-trace divergence = 0** — with bootstrap OFF both arms are identical
   over the entire trace.
3. **one extra Probe advances the shared chain exactly once** — `forward_steps` differs by exactly
   the number of probes emitted.
4. **the two quantities are not conflated** — a run with `channel_fate_divergence > 0` must still
   have `scheduler_data_displacement == 0`.

## Reporting rule

`scheduler_data_displacement` is a **safety gate input** (C2 gate 5). `channel_fate_divergence` is
**descriptive only** and is never a gate, never a defect, and never presented as displacement.
