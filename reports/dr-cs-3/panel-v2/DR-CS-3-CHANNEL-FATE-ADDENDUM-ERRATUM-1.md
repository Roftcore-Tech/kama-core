# ERRATUM 1 — DR-CS-3 CHANNEL-FATE INTERPRETATION ADDENDUM

Applies to `DR-CS-3-CHANNEL-FATE-INTERPRETATION-ADDENDUM.md`
sha256 `c08254d0f598c111ecc1fb7c5895e574e318d5e0914419f87f89f24a61859b77` (committed `d510070d`).

The addendum is **not edited**. It was frozen outcome-blind and its hash is load-bearing, so the
correction is recorded here instead.

## What is wrong

Addendum §5.1 justifies identity arm-independence with:

> the treatment flag's entire reachable write-set is
> `{bs_armed_ns, bs_next_step, bs_seq_next, bs_outstanding, bs_bytes, bs_done, bs_diag}` plus one
> appended decision with `unit_id = 0`.

That enumeration is **incomplete**. A second product path exists:

    Event::CsProbeEcho   (sender.rs:1515-1518)
      -> bs_on_echo
      -> self.rtt.observe (sender.rs:2503)   ==> srtt = Some(..)

So a delivered Echo in the treatment arm writes `srtt`, which is outside the seven listed fields.

## What survives, and what does not

**Survives — the conclusion.** `srtt` does not reach `tx_queue` (sole writer `sender.rs:1489-1490`)
and does not reach `OpaqueUnit.id` (`sender.rs:77-83`, `Copy`, never reassigned). The stable DATA
identity `(direction, unit_id)` is therefore still arm-independent, and the matched key
`(direction, unit_id, attempt_ordinal)` still joins the same semantic unit across arms.

**Does not survive — the stated reason.** The claim "the entire reachable write-set is those seven
fields" is false as written. The correct statement is narrower and is what the identity argument
actually needs:

    No treatment-reachable write reaches tx_queue or OpaqueUnit.id.

## Why it matters beyond wording

`srtt` is not inert. It gates PreConverge exit and feeds pacing, so a delivered Echo can change
*when* subsequent units are scheduled. That is a **trajectory** effect, and it makes the addendum's
own §1 (packet-domain re-phasing) stronger, not weaker: the treatment perturbs the schedule through
two channels — consuming a directional chain event, and moving `srtt`. Both are reasons a
post-boundary matched fate divergence is a diagnostic rather than a same-physical-event comparison.

It also means the per-direction treatment boundary (§2.2) can be reached through the reverse
direction: a reverse Echo delivery can change forward scheduling. Forward and reverse boundaries
must stay independently computed, as §2.2 already requires — this erratum removes any temptation to
treat the reverse boundary as derivable from the forward one.

## Status

The identity contract, the matched key, the metric separation and the claim-language constraints of
the addendum are **unchanged and still binding**. Only the §5.1 write-set enumeration is corrected,
and it is corrected in the direction of a *weaker* premise reaching the *same* conclusion.

Found by source-lineage audit of the committed implementation, before any channel-fate result was
interpreted.
