# DR-CS-3 — CHANNEL-FATE INTERPRETATION ADDENDUM

**Outcome-blind.** Frozen and committed before any channel-fate result exists. It constrains what a
matched-attempt fate divergence may be claimed to mean, so the interpretation cannot be chosen after
seeing the number.

Base: structural oracle CLOSED at `8d0c2fee` / `74a6736c` / `4f11545c`.
Companion: `DR-CS-3-SELECTION-BUILD-AXIS-ADDENDUM.md`
(sha256 `1e56423ef63c3016d75a941633bf9e379301ea7367e34a0126d9c3fd1aaf9480`).

---

## 1. The GE model is packet-domain, and the treatment re-phases it

`GeChain::next_drop` advances on the **channel event ordinal**. An extra Probe in the treatment arm
therefore:

* consumes its own directional chain event;
* re-phases the packet-to-draw pairing for every subsequent packet in that direction;
* can place the two arms' later matched DATA attempts at different GE ordinals and different GE
  states.

This is the mechanism of the existing V2 packet-domain channel model. Neither the logger nor the
comparator changes it, and neither may be redesigned to hide it. It is stated here **before** any
result because it structurally inflates matched fate divergence, and that inflation must not later
be read as evidence of harm.

## 2. Per-direction treatment boundary

For each direction separately, define:

```
first_treatment_induced_channel_event
  = the canonical directional event at which the two arms' channel-admitted event membership
    or packet semantic record first differ.
```

Forward and reverse boundaries are computed **independently**. It must not be assumed that a forward
Probe re-phases the reverse chain at the same instant, or at all.

### Pre-treatment prefix

Strictly before the boundary, the two arms must agree on every one of:

* canonical event membership
* packet kind
* semantic identity
* attempt ordinal
* GE state before and after
* actual fate
* directional event ordinal

Any mismatch in the prefix is:

```
PRE-TREATMENT-PREFIX-MISMATCH — INVALID PAIRING
```

That is an instrument failure, not a result. The pairing is void and no fate metric may be reported
from it.

## 3. What a post-boundary matched fate divergence is, and is not

After the first treatment-induced event both arms still use the same seed, the same channel
transition law and the same packet-domain model — but they are **no longer expected to consume
event-index-aligned draws for the same DATA attempt**.

`matched_data_attempt_fate_divergence` is therefore a valid **diagnostic** metric. It is **NOT**:

* a comparison of the same physical channel event;
* a direct-effect estimator under a shared exogenous draw;
* on its own, evidence of Probe harm or Probe benefit;
* a Gate-5 safety failure;
* a default-promotion threshold;
* a measure of Probe wire-byte cost;
* a measure of direct grant displacement.

**Authoritative reporting language:**

> The n-th channel-admission attempt of the same semantic DATA unit received a different channel
> fate under the two treatment trajectories.

**Forbidden language:**

> The same physical packet received a different fate under the same channel realisation.

A large matched fate divergence is **not** automatically a harm signal. Packet-domain re-phasing can
enlarge it structurally, with no change in product behaviour.

### Legitimate uses

* verifying pre-treatment prefix equality;
* matched vs unmatched attempt classification;
* detecting logger or comparator defects;
* quantifying post-treatment channel re-phasing as a diagnostic;
* trajectory-divergence lineage.

### Where the efficacy / default decision actually comes from

* multi-seed aggregate terminal outcomes;
* the preregistered challenge / confirmation lists;
* budget and latency outcomes;
* channel-independent arbitration evidence (the structural oracle).

## 4. Directional fate counts

Matched fate divergence is reported in at least these four directions, never collapsed:

```
off_delivered_on_dropped
off_dropped_on_delivered
both_delivered
both_dropped
```

Converting the first two into a summed "net harm" score, or presenting either as a causal cost, is
forbidden.

Each matched attempt additionally carries:

```
pre_boundary            | post_boundary
prior_attempt_prefix_aligned | prior_attempt_prefix_diverged
event_ordinal_aligned   | event_ordinal_rephased
```

`prior_attempt_prefix_aligned` is true only when attempts `0..n-1` exist in **both** arms with the
**same** actual fates. Otherwise the attempt is
`TRAJECTORY-CONDITIONED MATCHED-ATTEMPT FATE DIVERGENCE`.

## 5. Identity contract

### 5.1 Stable DATA identity

```
(direction, unit_id)
```

`direction` is load-bearing: without it a reverse ACK key derived from a delivered unit would be
numerically identical to that unit's forward DATA key, and a reverse-channel fate would be scored as
a forward DATA fate.

`unit_id = 0` — the treatment Probe decision — is never a stable DATA identity.

Source lineage (no new shipping accessor, no identity threading):

| element | source |
|---|---|
| `OpaqueUnit.id`, `Copy`, never reassigned | `sender.rs:77-83` |
| sole `tx_queue` writer | `sender.rs:1489-1490` |
| identity in scope at the decision paths | `sender.rs:4178`, `:3932`, `:4047`, `:4136` |

Arm-independence: the treatment flag's entire reachable write-set is
`{bs_armed_ns, bs_next_step, bs_seq_next, bs_outstanding, bs_bytes, bs_done, bs_diag}` plus one
appended decision with `unit_id = 0`. None of it reaches `tx_queue` or `OpaqueUnit.id`.

### 5.2 Attempt key

Per `(direction, unit_id)`, an **arm-local** monotone `attempt_ordinal: u32` incremented at channel
admission. Authoritative matched key:

```
(direction, unit_id, attempt_ordinal)
```

Ordinals start at 0, increase only on an actual DATA-like channel admission, and admit no duplicate
and no gap; overflow is fatal. The ordinal is never derived from queue position, tick, or global
event ordinal.

### 5.3 `class` is metadata until proven otherwise

`class ∈ {Data, Retransmit, Duplicate}` is recorded as **metadata** and does not substitute for the
stable unit identity.

The question *"can the same `(direction, unit_id, attempt_ordinal)` carry a different class in the
two arms?"* is answered from source lineage **before** any outcome is seen. If it can, `class` stays
out of the matched key, the attempt remains matched, and the difference is reported separately as
`attempt_class_divergence`. If a code invariant forbids it, that invariant is proved by exact source
lineage plus an adversarial control.

Folding `class` into the key would silently convert one matchable n-th attempt of the same unit into
two unmatched attempts. That is forbidden.

### 5.4 `tick`

The existing `tick` field is retained as **diagnostic scheduling time only**. It is never the stable
identity, the attempt ordinal, the matching key, or a source of channel fate.

## 6. Metric separation

These sets are kept apart and never summed into an aggregate total:

```
matched_data_attempts
matched_data_attempt_fate_divergences
off_only_data_attempts
on_only_data_attempts
non_data_channel_events
attempt_class_divergences
```

`TRAJECTORY DIVERGENCE` = an attempt present in only one arm. Unmatched traffic that appears because
the Probe advanced the GE state is reported as trajectory divergence and is never forced into the
matched fate count.

Comparison is a **full key-set membership scan** in both directions; zip-only comparison is
forbidden.

Never summed together: matched fate divergence, trajectory divergence, attempt class divergence,
emission schedule divergence, direct grant displacement, Probe bytes.

The old emission-tick symmetric-difference metric remains `SUPERSEDED — NOT CHANNEL FATE`.

## 7. Instrument neutrality is a precondition, not a result

The logger produces no packet, advances no channel state, consumes no draw, changes no event ordinal
or packet order, invokes no callback, touches no scheduler or pacer state, uses no wall-clock or
worker-completion order, and never truncates. Actual fate is read directly from the GE decision
branch — never inferred from an ACK, a terminal outcome, or an emission tick.

Because this instrumentation touches the shared test path, the frozen 32x18 neutrality
(32 records, 18 fields, 0 mismatches, 0 missing) is mandatory. On any behavioural difference outside
the logger's own records:

```
OBSERVATION-NEUTRALITY-FAILURE — INSTRUMENT INVALID
```

and work stops. Refreshing the baseline, dropping a field, allowlisting a mismatch, or carrying old
panel evidence onto non-neutral instrumentation are all forbidden.
