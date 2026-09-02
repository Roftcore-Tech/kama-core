# DR-CS-3 — BOOTSTRAP RTT PRODUCT CONTRACT

> Frozen **before** any code change. Grounding: `DR-CS-3-PRODUCT-PATH-GROUNDING.md`
> (sha256 `dd6737987c43696982565bd714cceddb…`, verdict `PRODUCT-BOOTSTRAP-PROBE-PATH-GROUNDED`).
> One cohesive protocol mechanism under a single runtime config `BootstrapRttConfig` (default OFF until §9).

## Sender

| # | binding requirement | grounding |
|---|---|---|
| 1 | Probes only while `state == PreConverge && rtt.srtt().is_none()` | `sender.rs` state + `RttEstimator::srtt` |
| 2 | Ladder is the authoritative `[3, 9, 21, 45] s` | `sender.rs:192 CS_PROBE_FIRES_NS` |
| 3 | Steps are absolute offsets from the bootstrap arm instant | — |
| 4 | Each step yields **at most one** probe generation | generation = `probe_seq` |
| 5 | Poll/tick repetition can never create a generation (edge-only) | the DR-CS-2 poll-inflation lesson |
| 6 | Total sender probe budget ≤ **200 B** | `sender.rs:196 CS_PROBE_TX_HARD_BUDGET` |
| 7 | 4 × `PROBE_WIRE_EXACT` = 4 × 44 = **176 B ≤ 200 B**; exact size derived from the codec | `control.rs:60` |
| 8 | Allowance is **additive**: consumes no DATA token and creates **no** pacer debt | the DR-CS-2 failure |
| 9 | Allowance usable **only** by the bootstrap probe | — |
| 10 | Probe counted in total-wire **and** in a separate bootstrap-control account | — |
| 11 | Probe is **not** recorded as DATA inflight; separate bounded outstanding-bootstrap state | Karn integrity |
| 19 | Only an echo matching an **outstanding generation** yields an RTT sample | `probe_seq` correlation |
| 20 | RTT = local monotonic (echo-arrival − probe-send); both endpoints local | — |
| 21 | Any peer-carried timestamp is **never** accepted as RTT | `CsEcho.seen_count/arrival_gap` are LEDGER-ONLY per `control.rs:68` |
| 22 | Duplicate / replayed / unsolicited / mismatched echo yields **no** sample | — |
| 23 | At most **one** RTT sample per generation | — |
| 24 | The first valid sample establishes `srtt` via the existing `RttEstimator` API | no synthetic write |
| 25 | The first valid sample cancels the ladder | — |
| 26 | PreConverge exit, session close or give-up clears all bootstrap state | — |
| 27 | Karn exclusion unchanged — duplicates/retransmits still yield no RTT sample | `sender.rs:2060` |
| 28 | ARQ PTO / loss-declaration timing stays byte/state-equivalent until a bootstrap echo arrives | no `Timer::Pto` coupling |
| 29 | Tail and FEC behaviour unchanged | — |
| 30 | Bootstrap frames never reduce the normal DATA emission budget | additive |

## Receiver

| # | binding requirement | grounding |
|---|---|---|
| 12 | Responds only to an **authenticated, valid, connection-bound** `0x19` | `receiver_host.rs:318` AEAD-open then `:331`, inside the `h_ext` slot |
| 13 | Returns the opaque correlation exactly as the protocol defines | echo carries the request's `probe_seq` |
| 14 | Never trusts a remote timestamp and never derives a new RTT | `seen_count`/`arrival_gap` are ledger-only |
| 15 | Serialized echo size ≤ request size | `ECHO_WIRE_MAX 43 B < PROBE_WIRE_EXACT 44 B` |
| 16 | At most **one** echo per unique generation | — |
| 17 | Receiver budget derived from 4 steps × `ECHO_WIRE_MAX` = **172 B** | `control.rs:67` |
| 18 | Per-connection responder state is **fixed-size** — no unbounded set/map | — |

## Additive allowance (§3)
Sender: ≤200 B and ≤4 generations per bootstrap epoch, one emission each, separate bounded
bootstrap-control budget, never deducted from DATA/refill tokens, never repaid from future DATA tokens.
Receiver: echo only for a valid authenticated request, size ≤ request, ≤4 unique echoes per epoch,
≤ `4 × ECHO_WIRE_MAX`, separate bounded response allowance, no DATA token and no pacing debt.

Security justification for the exception: fixed 4 steps · small pre-known maximum bytes · amplification
ratio ≤ 1 · authenticated connection binding · fixed-size state · immediate shutdown on success.

## Scheduler displacement prohibition (§3)
A bootstrap frame must not delay DATA merely by occupying the single output slot. Binding invariants:
* adding a bootstrap frame must not drop DATA that the baseline would have emitted in the same turn;
* before the first valid echo, the treatment's **non-bootstrap** source-prefix must equal the baseline's —
  bootstrap frames may only be *interleaved*;
* the global DATA/recovery/parity priority model is **not** redesigned.

## Single axis (§4)
`BootstrapRttConfig` — one runtime config, one binary, sender emitter and receiver responder are **one**
mechanism (not two treatments). OFF must be byte/state/wire/timer/result zero-diff against the FAZ-1A
baseline. No compile-time treatment binary.

## Frozen predicates
`STALL(65s)` and `ZERO_TX_STALL` unchanged. Recovery lineage required for every rescue:
`probe edge → sender emission → receiver authenticated arrival → echo action → echo emission →
sender match → RTT sample → srtt>0 → PreConverge exit`.
