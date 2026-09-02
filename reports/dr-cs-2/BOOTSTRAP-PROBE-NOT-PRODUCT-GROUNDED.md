# DR-CS-2B — FAZ 1 GROUNDING VERDICT: `BOOTSTRAP-PROBE-NOT-PRODUCT-GROUNDED`

> Gate evaluated **before** any treatment code was written, per §1.4. Outcome: **STOP.**
> No alternative treatment was applied, and no fake harness path was built.

## What EXISTS in the product

**The wire format is product-defined and shipping.**
* `crates/kama-transport-schema/src/inner_kind.rs:25` — `pub const CTRL_CS_ECHO: u8 = 0x1A;`
* `crates/kama-transport-schema/src/control.rs:67` — *"0x1A [DR-CS-1] — cold-start probe-echo
  (idempotent-stateless; ECHO_WIRE_MAX=43B)"*
* Probe request codepoint `0x19` likewise defined.

**The product composition root RECOGNIZES the probe.**
`crates/kama-integration/src/receiver_host.rs:331-334` (module doc: *"ürün-alıcı COMPOSITION-ROOT"*),
**unconditionally compiled**:
```rust
if *inner_kind == 0x19 || *inner_kind == 0x1A {
    // Stateless probe/echo (A-2): ControlDedupWindow'a GİRMEZ; O(1); patience-BESLEMEZ.
    self.cs_echo_count = self.cs_echo_count.saturating_add(1);
    return Ok(IngestOutcome::ProbeEchoed);
}
```

**Sender ladder constants are authoritative** (`sender.rs:192-198`): `CS_PROBE_FIRES_NS = [3,9,21,45] s`,
`CS_GIVEUP_NS = 60 s`, `CS_PROBE_WIRE_MAX = 44 B`, `CS_PROBE_TX_HARD_BUDGET = 200 B`.

## What DOES NOT exist — the disqualifying findings

**1. No product echo TRANSMITTER (decisive, §1.2).**
`receiver_host.rs` **counts** the probe and returns `IngestOutcome::ProbeEchoed` — it never emits an echo
frame. `IngestOutcome::ProbeEchoed` is consumed **only by tests**
(`tests/a2_f3_control_ingest.rs:48,55`, `tests/a2_f5_plateau_product.rs:92`). Workspace-wide, the only
code that actually writes an echo is `crates/adaptive-transport/src/udp_runner.rs:288` (`e[0] = 0x1A`),
inside a module whose own header declares it a
*"POLICY-AWARE UDP **BENCH-RUNNER** (feature-gated; **ÜRETİM-YOLUNA-BAĞLI-DEĞİL**)"* — explicitly not
connected to the production path — and it is additionally `experimental-cold-start`-gated
(`udp_runner.rs:768` *"yalnız flag-ON"*).

**2. The product receiver crate has zero probe support.**
`crates/adaptive-transport/src/receiver.rs` contains **0** references to `0x19`, `0x1A`, `CsProbe` or
`probe_echo`.

**3. No sender probe emitter in the shipped build.**
`TransmitKind::CsProbe` (`sender.rs:759-760`) and `drain_cs_probes` (`sender.rs:1808-1809`) are both
`#[cfg(feature = "experimental-cold-start")]`, i.e. **absent from the DEFAULT product build**.

## Why this is a STOP and not a small addition

Closing the gap requires implementing an **echo responder on the product receive path** — that is
precisely `Receiver/ACK-ingest yamalama`, forbidden by the standing rules for this turn, and it is a new
product wire *behaviour* (a response obligation for an inbound codepoint), not a scheduling change.
Modelling the echo in the harness instead would be exactly the **fake product path** the directive
forbids, and was already rejected once in the FAZ-2 A1 audit.

§1.4 therefore fails on its second requirement — *"Probe/echo iki uçta gerçek ürün yoludur"* — because
only ONE end (recognition) is product; the responding end is bench-only.

## Items that would unblock a future DR-CS-2B (design decisions, NOT taken here)
1. A **product-path echo responder**: `receiver_host.rs` (or `receiver.rs`) must actually emit `0x1A`
   in response to `0x19`, with the documented `ECHO_WIRE_MAX = 43 B ≤ request` amplification bound.
2. A **default-build sender probe emitter** that is *not* the `experimental-cold-start` pipeline —
   the A1 audit disqualified that pipeline on four independent grounds (no runtime switch, event-pipeline
   reroute, self-generated `Timer::Pto` via `BootstrapPto`, shared DATA token bucket).
3. A decision on **additive** send allowance for the probe (the FAZ-2 failure proved a pacing *debit*
   reallocates rather than adds).

## Status
```text
FAZ 0 (cleanup)                       DONE — failed treatment removed, zero-diff PASS
FAZ 1 (grounding)                     FAIL → BOOTSTRAP-PROBE-NOT-PRODUCT-GROUNDED
FAZ 2..7 (contract/impl/panels/default) NOT STARTED
alternative treatment                 NOT applied (forbidden)
product behaviour                     UNCHANGED by this turn
```
