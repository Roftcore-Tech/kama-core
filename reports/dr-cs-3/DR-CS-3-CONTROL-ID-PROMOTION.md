# DR-CS-3 — CONTROL-ID PRIMITIVE PROMOTION (§4)

## Verdict: **PROMOTION NOT REQUIRED — the bootstrap responder must not use the gate at all**

The audit found a **defect in my own DR-CS-3 responder**, not a missing promotion.

### Audit of `ControlIdIssueGate` (`kama-noise-wire/src/control_id_gate.rs`)

| Question | Finding |
|---|---|
| Depends on `ColdRxState` / `GlobalColdLedger` / `TombstoneRegistry`? | **No** — it imports only `kama_transport_schema::control::ControlId`. |
| Depends on `DedupDecision`? | **No** (it *derives* the CAL-10 window constant; no code dependency). |
| Depends on a bench/test driver? | **No.** |
| Purpose | Bounded, monotonic outbound `ControlId` issuance with hard rate/burst bounds. **Generic control-plane**, not cold-start-specific. |
| Fixed-size state / defined exhaustion | Yes — `MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION = 32`, `MAX_CONTROL_ID_BURST = 64`, typed `ControlIdIssueError`, compile-time asserts on `CONTROL_DEDUP_WINDOW == 4096`. |

### The decisive line — the gate's own contract excludes probe/echo

```text
control_id_gate.rs:9
  CsProbe(0x19)/CsEcho(0x1A) control_id-dedup'a GİRMEZ (A-2; stateless) → gate'e de girmez.
```
Its documented producer inventory is `Ack · Sack · Recovered · BlockComplete · ParamSet · ParamAck ·
BlockAbort · SessionClose · SessionRenewRequired` — **`CsProbe`/`CsEcho` are explicitly excluded.**

### Consequence
The previous turn's `emit_cs_echo` called `slot.issue_gate.issue()`, **violating that contract** by
consuming a control-id budget for a frame class that never enters control-id issuance or dedup. That is
now **fixed**: the echo carries the **request's own `control_id`**, echoed back — idempotent and
stateless, matching the schema's *"idempotent-stateless"* description of `0x1A`.

This removes the only gated dependency the bootstrap responder had, so **no promotion of
`ControlIdIssueGate` (and no promotion of the cold-RX subsystem) is needed**. The gate stays exactly
where and as it is; the experimental `ReceiverHost` continues to use it for `emit_recovered`.

Verified: `grep -c issue_gate receiver_control_core.rs` = **0**.
