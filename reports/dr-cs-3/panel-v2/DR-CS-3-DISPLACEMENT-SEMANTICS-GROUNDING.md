# DR-CS-3 — "DATA displacement" SEMANTICS, GROUNDED

Resolved from binding sources **before** implementing any metric. The contract text is not changed
and the metric contract is not silently reinterpreted.

## Verdict

`SEMANTICS A — DIRECT SCHEDULING DELAY (contended grant/token)`

> **DATA displacement** = a treatment packet consumed a pacing/budget opportunity that an *eligible,
> queued* DATA would otherwise have taken, in the same finite grant.

Not B (terminal/net loss) and not C (bounded-horizon overrun). Both are ruled out below.

## Binding sources, in order

### 1. Product definition — `sender.rs:2427-2434`

```
ADDITIVE, NOT PACED: this deliberately does NOT call try_send_pre_parity / pacer.try_send
and creates NO pacer debt — the DR-CS-2 failure proved a debit only reallocates the DATA budget.

NO DISPLACEMENT: called at the very END of schedule_transmit, after DATA, recovery and parity
have already been planned in the same turn, so it can only ever *append* — it can never take a
slot a normal DATA emission would have used.
```

This states the concept **and** its own falsification condition: displacement is *"taking a slot a
normal DATA emission would have used"*. That is A, verbatim.

### 2. Structural verification in code

| function | `pacer.try_send` calls |
|---|---|
| `drain_bootstrap_probes` | **0** |
| `drain_new_data` | **4** |

Probes never enter the token path, and are planned **after** DATA/recovery/parity in the same
`schedule_transmit`. So under A the product asserts displacement is **structurally impossible**, not
merely small.

### 3. DR-CS-2 precedent — what displacement looks like when it is real

`CREDIT-IS-NET-NEUTRAL-UNDER-DEBIT` (`DR-CS-2-RECOVERY-PERMIT-FAILED.md`): a token-**debiting**
recovery credit produced `tx_data 20 → 18` with `tx_duplicate 0 → 2` under total token starvation —
an immediate reallocation of the same finite budget, observed within the run.

This is the binding precedent for the concept, and it is an **A**-shaped observation: contention for
one grant, immediate substitution. It is not a terminal-horizon accounting (B) and involves no
bounded window (C).

## Why B is ruled out

B would require displacement to mean "DATA never emitted before a binding run/deadline horizon". The
DR-CS-2 precedent counted displacement at the moment of substitution, with no horizon involved, and
no binding source defines a terminal horizon for this gate. Adopting B would also make the gate
weaker than the precedent that established it.

## Why C is ruled out

C requires a binding horizon (one grant interval, a PTO, N ticks, or a stated latency/deadline
bound). No frozen contract, referenced document, or shipping constant supplies one for this gate.
Per the rule, a horizon may not be invented, chosen against results, or derived from a panel — so C
is unresolvable and therefore not selectable.

## Consequences for measurement

`scheduler_data_displacement` is measured from the **scheduler arbitration record**, per grant:

* was a DATA eligible **and** queued at this decision point?
* did the Probe consume the pacing/budget opportunity that DATA would have taken?
* which DATA identity was affected, and by how many ticks/grants was it deferred?

Since probes take no token and are appended after DATA planning, the expected value is **0**, and a
non-zero value is a real defect — exactly the property gate 5 is meant to police. The counter proves
only *directly observed Probe-over-DATA arbitration*; it never attributes long downstream trajectory
divergence to the Probe.

## Not blocked

A single meaning is determined by binding text and code, so `CONTRACT-SEMANTICS-BLOCKED` does **not**
apply and no user decision is required here.
