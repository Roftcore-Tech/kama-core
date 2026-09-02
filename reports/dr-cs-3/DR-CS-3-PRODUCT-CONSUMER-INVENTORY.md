# DR-CS-3 — PRODUCT CONSUMER INVENTORY (§3)

## Answer: **NO product runtime consumer exists.** ⇒ `PRODUCT-RUNTIME-INTEGRATION-PENDING`

| Question | Finding |
|---|---|
| Product binary / runtime crate? | **None.** The only `[[bin]]` in the workspace is `crates/bench` (a benchmark). `kama-integration` declares **0** bin targets. |
| Product server/client crates | `hub-server`, `client-core`, `admin-server`, `ra-ca`, `discovery`, `crypto`, `ccr-codec` are all **3-line stubs** with **zero** transport dependencies. |
| UDP socket / receive loop owner | Only `crates/bench/src/kama.rs`, `adaptive-transport/src/udp_runner.rs` (feature-gated **bench runner**, self-declared *"ÜRETİM-YOLUNA-BAĞLI-DEĞİL"*), and two `examples/`. **No product owner.** |
| Session lifecycle owner | The `Session`/`SessionManager` **types** live in `kama-noise-wire`; the only *user* is the gated `ReceiverHost` and now the default-build `ReceiverControlCore`. No process-level owner. |
| Noise handshake | **Does not exist.** `crates/crypto/src/lib.rs` is **3 lines**. `install_session_with_*` docs say *"Product: gerçek-handshake-Split()"* — i.e. planned, not built. |
| Who depends on the transport stack? | Only `crates/bench`, `adaptive-transport`, `kama-engine`. No product crate does. |

## Where the default core can live without a cycle
Dependency direction today: `kama-transport-schema → kama-noise-wire → kama-engine → kama-integration`,
with `adaptive-transport` beside them. `kama-integration` is the **leaf** — nothing in the product graph
depends on it — so placing `ReceiverControlCore` there introduces **no cycle** and no layering violation.

**This does not make `kama-integration` the product runtime owner.** When a real runtime crate appears it
will depend on `kama-integration` (or the core will move down beside `kama-noise-wire`); both directions
stay acyclic because the core imports only `kama-noise-wire` + `kama-transport-schema`.
