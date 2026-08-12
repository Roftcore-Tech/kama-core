# CORRIGENDUM — the "FEC parity starvation" was a MEASUREMENT blind spot, not a product defect

**Corrects commit `68165aca` and the `EXECUTION-STATE` entry it produced.** The earlier artifacts are
preserved verbatim; this document supersedes their *conclusion*, not their record.

## What I claimed (WRONG)

> `kama-engine/tests/fuzzy_e2e.rs` — the controller plans r=2 parity and, with the floor ACTIVE,
> **zero parity units reach the wire** in 900 ticks (9 s virtual) … Shipping the default would trade a
> fixed liveness defect for a broken FEC actuator.

Evidence cited: `floor ACTIVE ratio=800 planned=4 emitted=0` vs `floor INACTIVE … emitted=4`.

That observation was real. **The conclusion drawn from it was wrong.**

## What is actually true

Instrumenting `plan_fec_parity` at the emission site shows **identical** emissions in both arms:

```
FLOOR ON   t=20779  block=1 esi=0..3  send_record=true  kind=FecParity   (4 units)
FLOOR OFF  t=20804  block=1 esi=0..3  send_record=true  kind=FecParity   (4 units)
```

Both arms emit **all four** parity units. Floor ON emits them **25 ms EARLIER**. The drive window is
byte-identical in both arms (`t_ms=17534`, window `[17544, 26534]`), and both instants fall well
inside it.

## The real mechanism

The harness drain loop made two `on_event` calls per iteration and counted parity from only one:

```rust
let out = s.on_event(Event::TimerFired(Timer::Pto), now);   // counted
… collect parity from `out` …
if !data_seqs.is_empty() {
    s.on_event(ack(data_seqs), now + 5 * MS);               // return value DISCARDED
}
```

Tick instants are `17544 + 10k` → `…, 20774, 20784, …`; the ACK is delivered at `now + 5 ms` → **20779**.

* **Floor OFF** — parity happens to land on a Pto tick (`20804`) ⇒ counted ⇒ `emitted=4`.
* **Floor ON** — the flow is slightly faster, so parity lands in the **ACK-driven** schedule (`20779`)
  whose return value the harness threw away ⇒ `emitted=0`, while the wire really carried 4 units.

So the floor did not starve anything. It moved the emission by 25 ms across a boundary the harness
could not see.

## Fix applied

`real_latch_emit_decode` now collects parity from **both** `on_event` returns. The acceptance
assertion `planned > 0 && emitted == planned` is **unchanged** — no threshold was relaxed, no golden
rewritten, no FEC path touched. Only the observation gap that dropped real emissions was closed.

## Consequences

* `StartupPacingConfig::default()` is restored to **ON**; there is no FEC-parity blocker.
* Explicit FEC_R2 remains runtime-executable, and is marginally *faster* with the floor active.
* The class of error is worth recording: a fixed-cadence harness that samples only one of several
  scheduler entry points can convert a **timing shift** into an apparent **total loss of function**.
  The same pattern would hide any emission that migrates between entry points.

## Standing correction

Any statement in `EXECUTION-STATE` (pre-`fa48374f`), in commit `68165aca`, or in my turn reports that
described a "broken FEC actuator", "FEC parity starvation", or "zero parity reaches the wire" is
**withdrawn**. The measured fact is: **4/4 parity emitted in both arms; floor ON is 25 ms earlier.**
