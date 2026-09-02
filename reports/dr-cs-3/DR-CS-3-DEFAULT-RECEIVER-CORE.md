# DR-CS-3 — DEFAULT-BUILD RECEIVER CORE (§5/§6)

## `crates/kama-integration/src/receiver_control_core.rs` — **ungated, in the default build**

`grep -c 'cfg(feature' receiver_control_core.rs` = **0**. The only mentions of `ColdRxState`,
`GlobalColdLedger`, `TombstoneRegistry`, `ControlDedupWindow`, `ControlIdIssueGate` are in the module
doc comment saying it does **not** use them (lines 13-14). Imports are exclusively ungated:
`Session`, `SessionId`, `Direction`, `KamaNoiseWireProtector`, `SealArgs`, `decode_control`,
`InnerPlain`, `Control`, `UnsealedTransportPacket`.

### Responsibilities (owned)
session slot lookup · AEAD open (verify-before-admission) · authenticated control decode · valid session
context · typed pending outbound action · session seal · fixed-size bootstrap responder state · sealed
outbound control datagram return.

### Deliberately NOT owned
UDP socket · receive/send loop · process lifecycle · connection lifecycle · Noise handshake ·
`ColdRxState` · `GlobalColdLedger` · `TombstoneRegistry` · cold admission · `DedupDecision` pipeline ·
recovered-control subsystem · legacy ladder · bench runner · test bridge.

### Single source (§5)
`BootstrapEchoResponder` is defined **once** (`grep -c 'pub struct BootstrapEchoResponder'` = 1) and holds
the entire bounded policy (`on_authenticated_probe`, `take_pending`, `commit`, `echo_control`, `budget`,
`reset`) plus the fixed-size state (`[Option<u16>; 4]`). Both consumers embed **that one type**:
* `ReceiverControlCore` — default build,
* `ReceiverHost` — experimental overlay, which now **delegates** (`slot.bs.on_authenticated_probe`,
  `slot.bs.take_pending()`, `slot.bs.commit()`, `BootstrapEchoResponder::echo_control`).

No duplicated policy, no second state, no second responder, no copied "minimal receiver". The overlay
adds only cold-RX-specific behaviour.

### API (§6)
`new` · `install_session_with_directional_keys` · `ingest -> CoreIngest` · `take_pending_cs_echo` ·
`emit_cs_echo -> Vec<u8>` (sealed) · `echo_budget` · `close_session` · `session_count`.
Bounded: `MAX_CORE_SESSIONS = 1024` (fail-closed), `MAX_ECHOES = 4`, `ECHO_BUDGET_BYTES = 172`.
No unbounded queue is returned; the sealed datagram is returned by value (explicit ownership). No
test-only type is exposed. Socket I/O is the future runtime consumer's job.

### Feature matrix (§8)
```text
DEFAULT build      ReceiverControlCore · Session/AEAD/decode/seal · bootstrap responder ·
                   sender bootstrap emitter                                     PRESENT
DEFAULT build      ColdRxState · GlobalColdLedger · TombstoneRegistry ·
                   legacy ladder · bench runner · experimental pipeline         ABSENT
EXPERIMENTAL build same core + same responder + cold-RX overlay                 no re-implementation
```

### Gates
```text
default build     kama-integration suites            0 failed
experimental      kama-integration suites            0 failed
adaptive-transport lib (default / experimental)      145/145 · 144/144
default-build component integration                  7/7
feature-enabled component integration                6/6
fs_cal2_pins (rate-ramp supersession)                33/33
Bootstrap OFF vs FAZ-1A baseline zero-diff           192/192 PASS
scoped clippy -D (3 configs)                         clean
BootstrapEchoResponder definitions                   1
issue_gate used by the echo path                     0
```
