# PRODUCT-RUNTIME-INTEGRATION-PENDING

> Binding technical debt. Until it is closed, the transport is **not** a usable product.

## Where we are
The **default-build receiver component** (`ReceiverControlCore`) and the **default-build sender bootstrap
emitter** are implemented, bounded, security-tested and integration-tested. Library-level, the authenticated
bootstrap RTT mechanism is ready:
`Sender → sealed CsProbe → AEAD open → decode → bounded responder → typed CsEcho → seal → echo ingest →
matched local RTT → srtt>0 → PreConverge exit → startup pacing`.

## What is missing (all of it, today)

| Missing piece | Status |
|---|---|
| Product crate / binary owner | **None.** Only `crates/bench` has a `[[bin]]`. `hub-server`, `client-core`, `admin-server`, `ra-ca`, `discovery` are 3-line stubs with no transport deps. |
| UDP receive / send loop | **None** in product code. Sockets exist only in bench + examples + the feature-gated bench runner. |
| Connection lifecycle | **None.** Nothing creates/retires sessions at process level. |
| Noise handshake | **None.** `crates/crypto/src/lib.rs` is 3 lines; zero `x25519`/`snow`/handshake code. |
| Split-derived directional keys | **None.** Keys are test-injected; `install_session_with_*` docs say *"Product: gerçek-handshake-Split()"*. |
| Sender ↔ core wiring | **Not done.** No code drives both together over a socket. |
| Outbound datagram drain | Component returns sealed bytes; **nobody sends them**. |
| Process-level end-to-end test | **None.** |
| Shutdown / error / resource-bound validation | **None** at process level. |

## Claims that remain FORBIDDEN until this debt is closed
`SHIPPING RECEIVER COMPLETE` · `FULL PRODUCT END-TO-END PASS` · `DEPLOYABLE TRANSPORT` ·
`TRANSPORT PRODUCT READY` · `PRODUCT COLD-START FULLY FIXED` · any "Noise handshake verified" or
"deployment network path verified" statement.

## Next phase: `PRODUCT-RUNTIME-INTEGRATION` (NOT started)
1. Name the real product crate/binary owner.
2. UDP receive/send loop.
3. Connection lifecycle.
4. Noise handshake.
5. Split-derived directional keys.
6. `Sender` ↔ `ReceiverControlCore` wiring.
7. Outbound datagram drain.
8. Process-level end-to-end test.
9. Shutdown / error / resource-bound validation.

Also still pending, separately: the DR-CS-3 **locked/fresh panel validation** that gates
`BootstrapRttConfig` default-ON.


---

## STATUS UPDATE — 2026-08-26 (P1 runtime shell, `ddc9ac94`+)

Items **1, 2, 3(v0), 7, 8, 9**: CLOSED by `crates/client-core/{seam,wire,runtime}.rs` + gates
(loopback 7/7 in CI; two-netns netem gate GREEN at 5%/20ms, 20%/40ms, sustained 500@10%/40ms with
peak-RSS bound ~4 MB). Item **6**: PARTIAL — library-level wiring proven (CLOSURE-1B); UDP-level
completion deliberately deferred to the security plane (workstream B, S3) because the control plane
rides the kama-noise-wire sealed format; branch point visible via `unknown_frames`. Items **4, 5**:
security plane (workstream B; SoC correction `1b38936d`). Forbidden-claims list REMAINS IN FORCE
until items 4-6 close (no "deployable transport" / "Noise verified" claims; the runtime ships an
IdentitySeam stub and claims no security).
