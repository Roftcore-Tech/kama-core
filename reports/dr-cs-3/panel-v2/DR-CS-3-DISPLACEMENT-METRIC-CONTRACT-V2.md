# DR-CS-3 — DISPLACEMENT METRIC CONTRACT, V2 (corrected)

Supersedes `DR-CS-3-DISPLACEMENT-METRIC-CONTRACT` (md `61d64d55…`, json `9c32668a…`), which is
marked:

`SUPERSEDED — emission ticks were mislabeled as channel fates and gate-5 scope was not grounded`

Written **before** any seed search and before any new panel result. This is instrument-defect
closure, not a reopening of M2 or of any C2 threshold.

## 1. Frozen C2 gate 5, re-grounded

Exact text from the frozen contract (`b3a87d24`):

> | 5 | safety / budget / invariant violations: **0** |

and C2 §4 defines the class it counts:

> **`SAFETY-FAILURE`** — budget overrun · replay/mismatch RTT · duplicate RTT sample ·
> authentication violation · **DATA displacement** · ARQ timer diff. Never counted as censored.

**Consequence.** Gate 5's DATA-displacement component is scoped to the **whole run**, not to a
pre-Probe window. The superseded contract narrowed it to `ticks < first_probe_tick`, which makes it
structurally zero and silently redefines the frozen gate. That narrowing is withdrawn.

## 2. Four distinct quantities

### 2.1 `pre_treatment_scheduler_equivalence` — structural invariant

Prefix before the first treatment packet enters its directional channel. Traffic, event and channel
digests must be **exactly equal** between arms. This is an invariant, **not** gate 5.

### 2.2 `scheduler_data_displacement` — gate 5 component

Measured from the scheduler's **arbitration record**, over the whole run — never inferred from
emission-tick set differences. For each Probe selection the record answers:

* was a DATA eligible/queued at that decision point?
* did the Probe consume a pacing or budget opportunity a DATA would have taken?
* which DATA identity was affected?
* how many scheduler ticks was it deferred?
* was that DATA later sent, or suppressed entirely?

**Claim limit (12.2).** The counter proves only *directly observed Probe-over-DATA arbitration*. It
must not label long downstream trajectory differences as "Probe caused displacement".

### 2.3 `channel_fate_divergence` — matched attempts only (12.1)

Computed **only** over DATA transmission attempts that can be matched between arms. Stable identity
is derived from existing protocol identity: **DATA/source identity + transmission-attempt ordinal**.
No new random or outcome-dependent matching is invented.

Never reported as a single number. Every result reports:

```
matched_attempts
matched_fate_divergences
unmatched_baseline_attempts
unmatched_treatment_attempts
trajectory_divergence_start
```

Unmatched attempts are **not** counted as zero divergence; they belong to 2.4.

### 2.4 `trajectory_divergence` — descriptive only

Downstream traffic that cannot be matched between arms. Reported separately, never conflated with
displacement or with fate divergence, never a gate.

### Echo-bounded quantities

Report **`N/A — NO ECHO`** when no Echo arrives. Never expand to a whole-trace horizon.

## 3. Required directional channel event log

Per directional channel, per packet:

```
chronological draw index · tick / emission order · packet kind ·
stable DATA identity + transmission-attempt ordinal · drop/deliver fate ·
scripted-override source (if any)
```

New event-log fields are **excluded** from `wire_digest` and `source_prefix_digest`, so the log
stays observation-only (§12.5).

## 4. Positive controls must be non-vacuous

Each control first asserts that the event it tests actually occurred:

1. **PC3** — assert `probes_emitted > 0` **and** the fixture's tx equality explicitly; no conditional
   assert that can pass silently.
2. **PC4** — on a deterministic scripted fixture assert `matched_fate_divergences > 0` **and** the
   exact expected `scheduler_data_displacement`, together.
3. **no-echo** — assert `echoes_arrived == 0` first, then `echo_bounded_window == None`.
4. **no-extra-packet** — assert the entire directional event/fate trace is exactly equal.
5. **At least one fixture must PRODUCE post-treatment scheduler displacement** and the oracle must be
   shown catching it. Otherwise the metric is only a counter that has seen zero.

Each passing run reports **assertion count** and **fixture activation count**.

A synthetic positive control validates the oracle only. It is **not** evidence of product behaviour
and never alters the shipping scheduler.

## 5. Withdrawn claim

> *"a treatment emitting ≤3 probes cannot displace 380 DATA emissions"*

`SUPERSEDED — unsupported causal inference`. Correct statement: a few Probes can re-phase the shared
GE state and, through pacing and ACK feedback, produce long downstream trajectory divergence. The old
376–382 figures remain non-authoritative **because they used a `u64::MAX` horizon when no Echo
arrived** — not because of the probe count.
