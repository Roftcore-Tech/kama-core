# DR-CS-3 — RECEIVER SHIPPING AUDIT

## VERDICT: **`SHIPPING RECEIVER COMPOSITION ROOT NOT GROUNDED`** → **STOP** (§2 DUR clause)

`receiver_host` was **not** un-gated. The gate is not cosmetic, and no shipping consumer exists.

## Audit table

| Component | Needed for the default product path? | Experimental dependency? | Shipping decision |
|---|---|---|---|
| Session slot / state | yes | **YES** — slot embeds `ControlDedupWindow` + `ControlIdIssueGate` (both GATED) and `ColdRxState` (GATED module) | cannot ship as-is |
| AEAD open / seal | yes | no — `Session`, `KamaNoiseWireProtector`, `SealArgs`, `OpenedTransportShard` are **ungated** | ship-ready |
| Control ingest | yes | **YES** — the ingest path lives inside the slot and uses `DedupDecision` (GATED) for non-probe control | cannot ship as-is |
| Recovered-control response (`emit_recovered`) | no (not needed by DR-CS-3) | **YES** — `ControlIdIssueGate` (GATED) | stays gated |
| CS probe parse (`0x19`) | yes | no — `decode_control` + `Control::CsProbe` are **ungated** (schema crate has zero gating) | ship-ready |
| CS echo response (`emit_cs_echo`) | yes | **YES** — issues a `ControlId` via `ControlIdIssueGate` (GATED) | blocked by one gated dependency |
| Pending outbound action (`bs_pending_echo` / `take_pending_cs_echo`) | yes | no — plain fixed-size state added by DR-CS-3 | ship-ready |
| Legacy experimental ladder | no | n/a — it lives in `sender.rs`, not here (and is already superseded when DR-CS-3 is enabled) | stays gated |
| Bench / test helper (`insert_active_session_for_test`) | no | in-module test bridge | stays gated |
| Diagnostic-only state (`cs_echo_count`, `delivered`) | no | no | would ship incidentally |

## Why is `receiver_host` gated?

Not for cosmetic reasons, and not merely "because the old cold-start experiment used it". **The module
does not compile in the default build** — proven by a direct compile probe (temporarily un-gated, then
reverted):

```text
error[E0432]: unresolved import `kama_engine::cold_rx_state`
error[E0432]: unresolved imports `kama_noise_wire::ControlDedupWindow`,
              `kama_noise_wire::ControlIdIssueGate`, `kama_noise_wire::DedupDecision`
error[E0599]: no variant ... `EngineError::ColdInvariantViolation`
```

The module body itself contains **zero** `cfg(feature)` — the gating is entirely **transitive**:

* `kama-engine/src/lib.rs:24-25` — the whole `cold_rx_state` module is
  `#[cfg(feature = "experimental-cold-start")]`, documented *"flag-OFF'ta modül DERLENMEZ (INV-CS-5)"*.
  It carries `GlobalColdLedger`, `TombstoneRegistry`, `ColdRxState`, `CommittedTombstoneLease`,
  `SESSION_COUNT_CAP`, cold admission.
* `kama-noise-wire/src/lib.rs:45-48` — `ControlDedupWindow`, `DedupDecision`, `ControlIdIssueGate` gated.
* `EngineError::ColdInvariantViolation` — gated variant.

So promoting `receiver_host` means promoting the **entire DR-CS-1b/A2 cold-RX subsystem across three
crates** (global cold ledger, tombstone registry, cold admission, control dedup window, control-id
issuance gate) into the shipped build. That is a large architectural promotion in its own right — far
beyond "complete the bootstrap RTT path" — and it was never audited or validated as such.

## Is there a shipping consumer / composition root?

**No.** Workspace-wide, `ReceiverHost` is referenced only by:
* `crates/kama-integration/examples/a2_rx_netem_runner.rs` — an **example** (netem bench runner),
* **7 test files** (`a2_f0/f1b/f2/f3/f5`, `a2_c1b`, and the new `dr_cs3_bootstrap_rtt`),
* doc comments in `kama-engine/src/cold_rx_state.rs`.

`kama-integration` declares **no `[[bin]]`**; the only binary in the workspace is `crates/bench`. None of
the product server crates constructs a `ReceiverHost`.

⇒ Un-gating would expose a module that **no shipping binary consumes**, solely so a test can run under
default features. §2's DUR clause forbids exactly that:
*"Faithful default shipping consumer veya composition root tanımlanamıyorsa, sırf testi geçirmek için
modülü un-gate etme."*

## PATH-A / PATH-B assessment

* **PATH-A rejected** — the module is not shipping-ready: it does not compile under default features and
  drags in a gated three-crate subsystem.
* **PATH-B attempted and stopped short** — the shipping-safe core (authenticated control ingest, typed
  outbound action, AEAD seal/open, fixed-size responder state, `0x19 → 0x1A`) is *almost* separable:
  AEAD, `Session`, `decode_control`, the schema and the DR-CS-3 pending-action state are all ungated.
  The single blocking dependency is **`ControlIdIssueGate`** (needed to issue the echo's `ControlId`).
  Extracting that core would still require promoting `ControlIdIssueGate` out of the gate in
  `kama-noise-wire` — a cross-crate control-plane change — **and it would still have no shipping
  consumer**. Creating a parallel copy is explicitly forbidden by §2. So PATH-B does not reach a
  faithful shipping path in this turn either.

## What would unblock this (decisions, NOT taken here)
1. Name the **shipping composition root**: which product crate/binary will own the receive path and
   construct it. Without that, "shipping" has no referent.
2. Decide whether the **cold-RX subsystem** (`kama_engine::cold_rx_state` + the gated `kama-noise-wire`
   control-plane items) is promoted to the default build — a separate, auditable architectural step.
3. If only the bootstrap responder is wanted sooner: promote **`ControlIdIssueGate`** alone (it is
   generic control-plane machinery, not cold-start-specific) and host the responder in whichever crate
   owns the shipping receive path.

## Status
```text
receiver_host un-gated?            NO (compile probe reverted; tree unchanged)
new parallel implementation?       NO (forbidden by §2)
cold-start FIXED verdict?          NOT issued
BootstrapRttConfig default         OFF (unchanged)
product behaviour                  UNCHANGED this turn
```
