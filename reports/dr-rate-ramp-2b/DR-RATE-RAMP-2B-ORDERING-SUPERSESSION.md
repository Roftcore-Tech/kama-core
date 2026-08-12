# DR-RATE-RAMP-2B — FORMAL SUPERSESSION OF THE COMMIT-W GLOBAL ORDERING CONSTANT

**Branch taken: §A3.2** — the binding `DATA-before-PARITY` definition is **not** violated; only the
whole-flow ordering *constant* recorded in `budget_equivalence_pin` is invalid under startup pacing.

Written and hashed **BEFORE** any assertion or product-semantics edit. Evidence base:
`DR-RATE-RAMP-2B-FEC-ORDERING-CONTRACT.md` sha256
`1fc3b7e2ede556feea3cd62b2017422af049b4ed394f1d8c3de1b3abe0853e42`.

---

## Why §A3.1 does NOT apply

§A3.1 would require that parity actually breaches the binding `DATA-before-PARITY` boundary. It does
not, at either level that has an authority behind it:

| Level | Authority | Result |
|---|---|---|
| Scheduler turn | `sender.rs:2831` — *"FEC-parity DATA/recovery SONRASI (DATA-ÖNCE-sıralaması mühürlü)"*, realised by the fixed call order in `schedule_transmit` | **0 violations measured**, both with and without startup pacing |
| FEC block | `close_fec_block_manual` stages parity only after the block's K sources | **structurally impossible** to invert |

With the floor active, the turn at `Pto@10ms` emits `[D, P, P]` — DATA first, then parity, which *is*
the sealed intra-turn order. The 5th and 6th DATA are emitted in the **later** `Pto@20ms` turn. The
flattened global view therefore shows DATA after parity while no per-turn or per-block rule is broken.

## Why §A3.2 applies — all five conditions verified

| §A3.2 condition | Evidence |
|---|---|
| The real `DATA-before-PARITY` definition is not violated | per-turn violations **0/0**; block-level structural |
| Decoder / group semantics unchanged | parity carries `SrLogicalRef::Fec{block_id, esi}`; RS decode is order-independent within a block; the pin's parity block (9) shares no group with its DATA |
| Total budget unchanged | `(data, parity) = (6, 3)`, `total_bytes = 8400`, `enqueued_r_total = 3`, `emitted_r_total = 3`, `max_seq = 8` |
| Completion / safety unchanged | confirmed by the §A4 battery (recorded in the run report) |
| Early parity is deliberate, safe product behaviour | parity drains from `pending_fec` exactly when the pacer grants a token (`plan_fec_parity` → `pacer.try_send`), always after DATA in the same turn. The startup floor raises the grant rate; it does not reorder the drain |

## What is superseded, and what is not

**SUPERSEDED — narrow and explicit:**

> `budget_equivalence_pin::leftover_equivalence_pin_constants`, assertion
> `assert!(last_data < first_par, "pin: DATA-önce-parity")`
> interpreted as a **whole-flow** ordering requirement valid under **any** pacing configuration.

Provenance of the superseded constant, from the test's own comment (`:63`):
`// PİN-SABİTLERİ (feature-OFF-baseline'dan-ölçüldü …)` — it was **measured from one baseline
trajectory**, not derived from a contract. No code or design document states a whole-flow ordering
rule.

**NOT SUPERSEDED — everything Commit-W actually sealed:**

* the per-turn `DATA-before-PARITY` order,
* the block-level parity-after-close property,
* `ON + LeftoverOnly ≡ OFF-baseline` decision-level equivalence — **re-verified byte-for-byte** in
  this turn (identical sequence, counts, bytes and seq span under both feature settings),
* every count and byte-budget constant in the pin.

## Discipline applied (mirrors the INV-RATE-1 supersession precedent)

1. This artifact is written and hashed **before** any test edit.
2. The old test is **preserved, not rewritten**: `leftover_equivalence_pin_constants` keeps all six
   assertions **verbatim**, including `last_data < first_par`. It is **re-scoped** to the
   startup-pacing-**inactive** path by setting `StartupPacingConfig { enabled: false }` explicitly,
   so it continues to guard the legacy trajectory it was captured from.
3. **No assertion is weakened.** Nothing is changed to `>=`, nothing is deleted, FEC is not disabled
   to make anything pass, and the startup floor formula is untouched.
4. The genuine invariants are sealed by **new exact tests** that run under the **product default**:
   * per-turn `DATA-before-PARITY` — no parity may precede a DATA *within a single*
     `schedule_transmit` result, asserted turn by turn, under startup pacing ON **and** OFF;
   * `experimental-fec-budget` ON ≡ OFF decision-level equivalence under the product default;
   * counts and serialized byte budget invariant to the pacing axis;
   * block-level: parity for a block never precedes that block's own source emissions on the
     authoritative `open_fec_block_authority` / `close_fec_block_manual` path.

## Product code

**No product behaviour is changed by this supersession.** The two `TransmitKind::CsProbe` match-arm
un-gatings (§A1) are compile-visibility repairs with no value or ordering effect. The startup pacing
formula, the FEC drain, the pacer and the scheduler call order are all untouched.

## Result

`STARTUP-PACING-PRESERVES-SEALED-FEC-ORDERING · GLOBAL-TRAJECTORY-CONSTANT-SUPERSEDED`

Startup pacing may remain **default-ON**, subject to the §A4 battery passing.
