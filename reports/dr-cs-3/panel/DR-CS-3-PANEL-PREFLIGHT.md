# DR-CS-3 — §1 PANEL PREFLIGHT CONFORMANCE

**Verdict: `DR-CS-3 PANEL PREFLIGHT PASS`**

Scope: authenticated transport **component** with TEST-INJECTED session keys. No Noise handshake, no
product binary, no deployment network path is validated by anything in this document.

Start HEAD: `4aa11a93af44b2b9e884461e8db3d1ed576c8b4f` · branch `d1-sprint5c` · no commit made.

---

## §1.1 — Core/ReceiverHost ownership

### CORRIGENDUM (previous report language was stronger than the code)

Earlier DR-CS-3 reporting said *"ReceiverHost ortak core'u kullanıyor / delegates to the core"*. Read
literally that implies `ReceiverHost` **composes a `ReceiverControlCore` instance**. It does not.
Prior artifacts are left unmodified; this section is the correction.

**What the code actually is:**

| Concern | `ReceiverControlCore` (default build) | `ReceiverHost` (`experimental-cold-start`) | Shared? |
|---|---|---|---|
| Bootstrap responder state + policy | `BootstrapEchoResponder` | `BootstrapEchoResponder` | **YES — one type, one impl** |
| Echo frame construction | `BootstrapEchoResponder::echo_control` | `BootstrapEchoResponder::echo_control` | **YES — one fn** |
| Session map | own `BTreeMap<u32, CoreSession>` | own `BTreeMap<u32, SessionSlot>` | no (separate) |
| Session lookup | `h_ext` **parameter** | `h_ext` **demuxed from datagram\[0..4\]** + tombstone grace | no (separate) |
| AEAD open | `slot.session.protector_mut().open()` | `slot.session.protector_mut().open()` | same call, own instance |
| Control decode | `decode_control(opened.inner())` | `decode_control(opened.inner())` | same fn, own call site |
| Seal | `slot.session.seal(Control, SealArgs{wire_pn, 0, 0})` | identical shape | same fn, own call site |
| `control_id` for the echo | explicit parameter | captured into `bs_last_control_id` at ingest | equivalent value |
| Cold-RX / dedup / issue-gate | **none** | `ColdRxState`, `ControlDedupWindow`, `ControlIdIssueGate` | host-only |
| Session cap | `MAX_CORE_SESSIONS` (fail-closed) | `SESSION_COUNT_CAP` via `GlobalColdLedger` | separate |

So the accurate statement is: **`ReceiverHost` re-uses the single-source `BootstrapEchoResponder`
policy; it does NOT compose a `ReceiverControlCore`.** The session-lookup / AEAD-open / decode / seal
*paths* are separate code, but they call the same underlying primitives on their own instances.

**Full core delegation was NOT performed**, per the directive's own instruction not to refactor for
terminology alone: `ReceiverHost` demuxes `h_ext` from the wire, owns tombstone grace, cold-admission
and block assembly. Wrapping a `ReceiverControlCore` inside it would either duplicate the session map
or force the cold-RX state through a component that deliberately excludes it.

### Acceptance (all required items met)

| Requirement | Result | Evidence |
|---|---|---|
| `BootstrapEchoResponder` single implementation | **PASS** | struct defs = 1, `impl` blocks = 1 (whole `crates/` tree) |
| Default & experimental responder policy semantically identical | **PASS** | both embed the same type; refusal semantics asserted equal in test |
| At most one echo producer per probe | **PASS** | single `pending: Option<u16>` slot; `take_pending()` drains once; `commit()` is the only accounting path |
| **Byte-identical echo** on default & experimental vectors | **PASS** | `default_core_and_receiver_host_emit_byte_identical_echo` |
| Same security/budget rules on both paths | **PASS** | `echo_budget(H) == cs_echo_budget(H)` before and after a refused duplicate |

**Two-responder risk:** structurally possible only if the *same* datagram for the *same* `h_ext` were
fed to a `ReceiverControlCore` **and** a `ReceiverHost` in one process. No such composition exists in
the tree (there is no product runtime consumer at all — `DR-CS-3-PRODUCT-CONSUMER-INVENTORY`), and the
two types share no state. Recorded as a **composition rule for `PRODUCT-RUNTIME-INTEGRATION`: exactly
one responder owner per session.**

---

## §1.2 — `control_id` reflection safety

**Verdict: `CONTROL-ID-REFLECTION-SAFE`** — the panel is authorised to start.

| Claim | Result | Code evidence |
|---|---|---|
| `control_id` is **not** an AEAD nonce | **PASS** | `protector.rs:36` `nonce96(full_pn) = [0x00000000 ‖ LE64(full_pn)]`; only call sites `nonce_for(args.wire_pn)` (seal, `:269`) and `nonce_for(candidate)` (open, `:334`) |
| `control_id` is **not** an AAD input | **PASS** | `aad.rs:16-24` AAD = `AAD_DS ‖ h_ext ‖ EXTENDED_V1 ‖ wire_flags ‖ pn_trunc`; `control_id` occurrences in `aad.rs` = **0** |
| Sealing uses its own monotone counter | **PASS** | `SealArgs.wire_pn` + per-epoch `last_sealed_pn[ei]`; `wire_pn <= last ⇒ WireError::SealInput("non-monotonic wire_pn (nonce-reuse guard)")` (`:252-258`) + `SEND_PN_CAP` |
| Probe/Echo genuinely outside `ControlIdIssueGate` | **PASS** | `control_id_gate.rs:4-9` PRODUCER-INVENTORY lists `0x10..0x18` only and states *"CsProbe(0x19)/CsEcho(0x1A) … gate'e de girmez"*; core `issue_gate` refs = **0**; host `emit_cs_echo` no longer calls `issue()` |
| Probe/Echo genuinely outside inbound control-id dedup | **PASS** | `receiver_host.rs:342-361` returns **before** `slot.dedup.check(*control_id)`; core has no dedup window at all; `InboundControlDedup` has **no production consumer** (tests only) |
| Same numeric `control_id` in both directions ⇒ no nonce reuse | **PASS** | directions use **different keys** (`send_cipher` ← `send_key`, `recv_cipher` ← `recv_key`); identical plaintext at identical `wire_pn` yields different ciphertext and cross-direction open fails |
| `probe_seq` is the authoritative correlation | **PASS** | `Event::CsProbeEcho { probe_seq }` carries `probe_seq` **only** — `control_id` never reaches the sender's RTT logic; `sender.rs:2341` rejects `seq != probe_seq` |
| `control_id` carried only as idempotent request/response identity | **PASS** | echoed verbatim; `RetryClass::of(0x19) == of(0x1A) == NoRetry` |
| Replay/duplicate bounded by fixed-size `probe_seq` state | **PASS** | `echoed: [Option<u16>; 4]` — fixed array, never a growing set |

### Exact tests (all seven required, plus three structural)

| # | Test | Result |
|---|---|---|
| 1 | same numeric `control_id`, opposite directions → different nonce/ciphertext | **PASS** `same_control_id_in_both_directions_uses_different_keys_and_ciphertext` |
| 2 | replayed probe → no additional echo | **PASS** `replayed_probe_produces_no_additional_echo` (both byte-identical replay and fresh-pn re-send of an answered generation) |
| 3 | replayed echo → no additional RTT sample | **PASS** `replayed_echo_produces_no_additional_rtt_sample` (3 replays; `echo_match_valid` stays 1) |
| 4 | same `control_id` + wrong `probe_seq` → no RTT | **PASS** `wrong_probe_seq_produces_no_rtt` |
| 5 | same `probe_seq` + wrong session → no RTT | **PASS** `same_probe_seq_wrong_session_produces_no_echo_and_no_state_change` |
| 6 | fifth generation → no echo | **PASS** `fifth_generation_produces_no_echo` |
| 7 | sender/receiver budgets not exceeded | **PASS** `sender_and_receiver_budgets_hold_with_amplification_below_one` (sender ≤ 200 B, receiver ≤ 172 B, each echo 43 B < probe 44 B) |
| + | nonce is a pure function of `wire_pn` | **PASS** `nonce_is_derived_from_wire_pn_only_and_never_from_control_id` |
| + | `probe_seq` authoritative, `control_id` reflected verbatim | **PASS** `probe_seq_is_the_authoritative_correlation_and_control_id_is_reflected_verbatim` |
| + | four answered probes consume **zero** control-id issuance budget | **PASS** `probe_echo_consume_no_control_id_issuance_budget` |

**Run counts:** `dr_cs3_panel_preflight` — **10/10 default build**, **12/12 `experimental-cold-start`**;
0 failed in both.

---

## §1.3 — Session capacity scope record (`MAX_CORE_SESSIONS = 1024`)

The value was **not** changed in this turn.

- **Fail-closed:** a new handle at capacity returns `CoreError::SessionCapacity { cap: 1024 }`; no
  insertion occurs.
- **Eviction:** **none.** There is no LRU, no TTL, no silent replacement.
- **Effect on installed sessions:** **none.** Sessions already installed keep ingesting, echoing and
  accounting normally while the cap is saturated. Capacity is reclaimed only by explicit
  `close_session`.
- **Re-install of an existing handle is permitted at capacity** (`contains_key` is checked first). It
  replaces that handle's session and resets its responder — a rekey path, not an eviction path.
- **Status:** this is a **component safety cap**, chosen to bound fixed-size state in a library
  component. It is **not** a product capacity decision.
- **Ownership of the final decision:** `PRODUCT-RUNTIME-INTEGRATION` (see
  `PRODUCT-RUNTIME-INTEGRATION-PENDING`). Real capacity depends on the runtime's memory budget,
  session TTL, admission control and handshake rate limiting — none of which exist yet.

Grounded by `session_capacity_is_fail_closed_without_eviction`.

---

## Preflight outcome

| Gate | Result |
|---|---|
| §1.1 ownership acceptance (5 items) | **PASS** (with corrigendum recorded) |
| §1.2 `control_id` reflection safety (9 claims, 10 tests) | **PASS** — `CONTROL-ID-REFLECTION-SAFE` |
| §1.3 capacity scope record | **RECORDED** (value unchanged) |

**`DR-CS-3 PANEL PREFLIGHT PASS` — proceeding to §2.**
