# DR-CS-3 — PRODUCT PATH GROUNDING

## VERDICT: **`PRODUCT-BOOTSTRAP-PROBE-PATH-GROUNDED`**

All six §1.4 requirements are met. The bootstrap RTT path can be productized by **completing** the
existing shipping protocol — no new codepoint, no second protocol, no bench-runner reuse.

## 1. Wire format — shipping, ungated, amplification-bounded

`crates/kama-transport-schema/` contains **zero** `experimental-cold-start` gating (verified across
`control.rs`, `inner_kind.rs`, `prefix.rs`, `param.rs`, `lib.rs`).

```rust
// control.rs:60-75
/// 0x19 [DR-CS-1] — cold-start CS-probe (subtype=0x01; pad=9B-sıfır wire-codec'te; amp≤1:
/// PROBE_WIRE_EXACT=44B). probe_seq: sender-session-monoton (INV-CS-7). RETRY-EDİLMEZ.
Control::CsProbe { id: ControlId, subtype: u8, probe_seq: u16 }
/// 0x1A [DR-CS-1] — cold-start probe-echo (idempotent-stateless; ECHO_WIRE_MAX=43B).
Control::CsEcho  { id: ControlId, subtype: u8, probe_seq: u16, seen_count: u32, arrival_gap: u32 }
```
`inner_kind.rs:23,25` — `CTRL_CS_PROBE = 0x19`, `CTRL_CS_ECHO = 0x1A`, both accepted by
`inner_kind.rs:81-82`. **Correlation value exists**: `probe_seq` ("sender-session-monoton, INV-CS-7").
**Amplification ≤ 1 by construction**: `ECHO_WIRE_MAX (43 B) < PROBE_WIRE_EXACT (44 B)`.

## 2. Inbound chain — authenticated and connection-bound (§1.3 satisfied)

```text
datagram
 → ReceiverHost::ingest              receiver_host.rs:290  "GERÇEK INGEST YOLU … h_ext-demux → gerçek AEAD-open"
 → h_ext session demux (slot)        connection binding
 → slot.session.protector_mut().open(datagram)   receiver_host.rs:318
      Err ⇒ IngestOutcome::Dropped("aead verify failed")   :320   ← verify-before-admission §6
 → opened.inner()                    :325   authenticated plaintext
 → InnerPlain::Control{inner_kind}   :326
 → inner_kind == 0x19 / 0x1A         :331   ← the probe is recognised AFTER authentication, inside a session
```

The `0x19` handler therefore already sits at the correct security boundary: **post-AEAD-verify and
connection-bound**. No blind responder placement is required.

## 3. Outbound response chain — a shipping typed-action pattern already exists

`ReceiverHost::emit_recovered` (`receiver_host.rs:431-463`) is the product's existing
"typed control response emitted from the composition root":

```text
emit_recovered(h_ext, …)
 → sessions.get_mut(&h_ext)                    UnknownSession if absent      (connection-bound)
 → slot.issue_gate.issue()                     F-CID-0 monotonic ControlId + BURST-ENFORCED
 → Control::Recovered { id, … }                typed control action
 → slot.session.seal(UnsealedTransportPacket::Control(ctrl), SealArgs{wire_pn,…})
 → Ok(sealed.bytes)                            real sealed wire datagram returned to the caller
end_emission_batch(h_ext)                       resets the per-batch burst counters
```

A `0x19 → 0x1A` responder is the **same shape**: issue a `ControlId`, build `Control::CsEcho`, seal, return
sealed bytes, bounded per bootstrap epoch. The bench writer `udp_runner.rs:288` is **not** used.

## 4. Outbound sender chain — buildable without the disqualified pipeline

`Control::CsProbe` encodes/decodes in the ungated schema crate, so a default-build sender emitter needs
**only** an independent bootstrap timer + generation + allowance + outstanding-correlation state. None of
the four disqualified `experimental-cold-start` behaviours are required or reused:
`on_event → on_event_batch` rerouting · `BootstrapPto → Timer::Pto` · ARQ loss-declaration schedule
changes · compile-time two-binary treatment.

## 5. §1.4 checklist

| requirement | status |
|---|---|
| probe/echo is a real product path at both ends | **YES** — format + authenticated recognition ship; the responder is a completion of the existing `emit_recovered` pattern, not a new subsystem |
| echo correlation gives an unambiguous RTT sample | **YES** — `probe_seq` (session-monotonic, INV-CS-7) |
| no response amplification | **YES** — 43 B echo ≤ 44 B probe, by schema definition |
| probe timer separable from ARQ/PTO | **YES** — independent bootstrap timer; no `Timer::Pto` coupling |
| runtime ON/OFF in one binary | **YES** — nothing in the path is feature-gated except the *old* sender emitter, which is not reused |
| bounded additive allowance representable | **YES** — sender `CS_PROBE_TX_HARD_BUDGET = 200 B` / 4 steps; receiver `4 × ECHO_WIRE_MAX` |

## 6. VALIDATION-ARCHITECTURE FINDING (bounds §8.4 / §9 — surfaced, not decided)

Two facts constrain how far an end-to-end claim can be taken **in this repository**:

1. **The FS-CAL spine harness does not use `ReceiverHost` at all** (`grep -c` = 0). It models the peer
   abstractly (`Chan` + ACK queue, no AEAD, no session). So the locked/fresh panels can validate the
   **sender** bootstrap machine, with the harness peer mirroring the product responder — legitimate only
   *because the product responder will actually exist and be independently tested*. (This is the precise
   difference from the DR-CS-2B rejection, where **no** product responder existed and the harness echo
   modelled a non-existent capability.)
2. **A real Noise handshake does not exist in this repo.** `crates/crypto/src/lib.rs` is a **3-line
   skeleton**; zero `x25519` / `snow` / `Handshake` references. `ReceiverHost::open_session` takes
   **injected** directional keys and its own doc says *"Product: gerçek-handshake-Split()"*. Therefore a
   faithful two-netns run can only be performed with **test-injected keys** — the authenticated path is
   real, but the key establishment is not. Any §8.4 two-netns result must be labelled accordingly, and a
   full end-to-end product claim remains gated on the Sprint-6 handshake work.

Neither fact blocks §1.4 grounding; both bound what §9 may assert.
