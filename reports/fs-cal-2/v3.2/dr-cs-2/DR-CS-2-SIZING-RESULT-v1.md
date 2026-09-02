# DR-CS-2 SIZING — RESULT v1

## VERDICT: **`LADDER-NOT-ISOLATABLE`** — sizing diagnostic **NOT EXECUTED**

Per the frozen preregistration, the 80-run (40 pairs × 2 arms) sizing diagnostic runs only if the §A1
gate returns `LADDER-PRESENT-AND-ISOLATABLE`. It did not. **Executed sizing runs = 0.**
No CS scope class (`CS-SMALL` / `CS-MEDIUM` / `CS-SCHEDULING-ARCH` / `CS-NO-SIGNAL`) is claimed.

A real product ladder **does exist** — it is not a harness fake. It is confined to `sender.rs` (73
`#[cfg(feature = "experimental-cold-start")]` sites) and `udp_runner.rs` (45). Constants:
`CS_PROBE_FIRES_NS = [3, 9, 21, 45] s` (absolute offsets from arm, no backoff), `CS_GIVEUP_NS = 60 s`,
`CS_PROBE_WIRE_MAX = 44 B`, `CS_PROBE_TX_HARD_BUDGET = 200 B`. Emission is `TransmitKind::CsProbe` via
`drain_cs_probes`, appended at the END of `schedule_transmit` (DATA-first ordering explicitly preserved).

## Why it is not isolatable (five independent grounds; any one disqualifies)

1. **No runtime switch.** Every `cs_*` item is compile-gated; there is no arm/disarm API. `CS-BASELINE`
   and `CS-LADDER-ONLY` would therefore be **two different binaries**, not one binary under one flag.
2. **It reroutes the entire event pipeline.** `on_event` dispatches to `on_event_batch` (four-phase)
   instead of `on_event_core + schedule_transmit` (`sender.rs:1184` ON vs `:1193` OFF; batch at `:1276`),
   with early-return paths that skip `pacer.refill()` / clock advance under conditions the OFF build
   does not have.
3. **It makes the sender self-generate `Timer::Pto`.** `sender.rs:1588` —
   `self.on_event_core(Event::TimerFired(Timer::Pto), now_ns)` inside `CsTimerKind::BootstrapPto`.
   That changes the **ARQ loss-declaration and tail-probe firing schedule**: a second axis.
4. **Probes share the DATA token bucket** (`drain_cs_probes` → `pacer.try_send`), so the ladder cannot
   be evaluated independently of the very pacing axis under test.
5. **No product-path echo responder.** `receiver.rs` contains **zero** `CsProbe` references. The
   `0x19 → 0x1A` echo exists only in `udp_runner.rs`, whose module doc states it is a
   *"BENCH-RUNNER … ÜRETİM-YOLUNA-BAĞLI-DEĞİL"* (not connected to the production path). Modelling that
   echo in the FS-CAL harness would be exactly the **fake harness probe** the brief forbids — and
   without it, a ladder probe can never resolve `srtt` on the product path, so enabling it there would
   be pure cost with structurally zero benefit.

## Honest disclosure
A `CsProbe` carry + peer-echo path **was implemented** in the harness (feature `dr-cs-2-ladder`) before
finding (5). It is left in the tree but is **NOT FAITHFUL** — it models a peer capability the product
receiver does not implement — and it produced **no verdict**. It must not be used for science.

## What would unblock a future DR-CS-2 sizing study
- A **runtime** arm/disarm switch so both arms share one binary.
- Either a proof that ON-with-ladder-disarmed is byte-identical to OFF (including `pacer.refill`/clock
  advance and the `Timer::Pto` firing schedule), or decoupling `BootstrapPto` from the ladder.
- A **product-path** echo responder in `receiver.rs`.
- An explicit decision on whether ladder probes may share the DATA token bucket.

Until then the DR-CS-2 scope question ("is cold start fixed by the existing ladder, or is it a
scheduling-architecture decision?") **cannot be answered by a single-axis experiment.**
