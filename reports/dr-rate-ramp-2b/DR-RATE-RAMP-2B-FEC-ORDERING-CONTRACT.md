# DR-RATE-RAMP-2B — FEC ORDERING CONTRACT

What `budget_equivalence_pin` actually protects, derived from code and measured **before** any
assertion or product-semantics change. Start HEAD `4aa11a93af44b2b9e884461e8db3d1ed576c8b4f`.

At the time this artifact was hashed, the only tree changes were **two compile-visibility repairs**
(§A1), both of which remove a `#[cfg(feature = "experimental-cold-start")]` from a `TransmitKind::CsProbe`
match arm whose variant DR-CS-3 had un-gated. Neither changes any value, ordering, assertion or
pacing behaviour.

---

## The eight questions

### 1. What do the values in `[0,0,0,0,3,3,0,0,3]` represent?

They are the **pin's own kind tags**, not a wire encoding: `0 = TransmitKind::Data`,
`3 = TransmitKind::FecParity` (`budget_equivalence_pin.rs:22-31`). The vector is the
`TransmitDecision` stream **flattened across every scheduling turn** of the scenario — 6 `AppData`
turns, 1 `AckReceived` turn and 40 `TimerFired(Pto)` turns, appended by the `push` closure.

It is therefore **neither** an intra-turn order **nor** a per-FEC-block order. Full record with
seq/size:

```
[(0,0,1200) (0,1,1200) (0,2,1200) (0,3,1200) (3,4,400) (3,5,400) (0,6,1200) (0,7,1200) (3,8,400)]
```

Grouped by the turn that produced it (measured):

| turn | startup pacing OFF | startup pacing ON (product default) |
|---|---|---|
| `AppData@0ms` | `[D]` | `[D]` |
| `Ack@7ms` | `[D D]` | `[D D]` |
| `Pto@10ms` | `[D]` | `[D P P]` |
| `Pto@20ms` | `[D D]` | `[D D]` |
| `Pto@30ms` | `[P P P]` | `[P]` |

### 2. Does the pin only protect total byte / budget equality?

**No.** It carries five independent assertions: `enqueued_r_total == 3`; `emitted_r_total == 3`;
`(data_count, parity_count) == (6, 3)`; `last_data < first_par`; `total_bytes == 6·1200 + 3·400`;
`max_seq == 8`. Byte/budget equality is one part of it.

### 3. Or does it protect "all source DATA leaves before parity"?

The single assertion `last_data < first_par` (`:70-72`) asserts exactly that, **globally over the
whole scenario** — the last DATA decision *anywhere in the flattened stream* must precede the first
parity decision *anywhere in it*.

### 4. At what level is `DATA-before-PARITY` actually defined?

**Scheduler-turn level**, plus a separate structural property at block level.

* **Per turn (the sealed one).** `sender.rs:2831`:
  `// DR-S5-1/Commit-K: FEC-parity DATA/recovery SONRASI (DATA-ÖNCE-sıralaması mühürlü).`
  It is realised by the fixed call order inside `schedule_transmit` (`:2806-2852`):
  `plan_arq_retransmits?` → `drain_new_data` → recovery → `emit_tail_probe` →
  `drain_due_duplicates` → **`plan_fec_parity`** → (probe passes). Parity can never precede DATA
  *within one turn*.
* **Per FEC block (structural, separate).** In the authoritative path parity for block *B* is only
  staged by `close_fec_block_manual` (`:2946-2979` → `push_parity_authorized`), i.e. **after** *B*'s
  K sources have been emitted. Parity cannot precede its own block's source by construction.
* **Not defined globally across the flow.** No code or comment states a whole-flow ordering.

Note the pin does not exercise the block-level property at all: it uses the **deprecated** direct
`enqueue_fec_parity(9, esi, 400)` for `block_id = 9`, while its DATA are `OpaqueUnit{id: 100..105}`
with no block association. Its parity and its DATA are not in the same FEC block/group.

### 5. Does startup pacing change only timing, or the semantic wire order?

**Only timing** — specifically, how many units fit in a turn. Measured per-turn
`DATA-before-PARITY` violations:

| configuration | per-turn violations |
|---|---|
| startup pacing OFF | **0** |
| startup pacing ON (product default) | **0** |

With the floor active, `Pto@10ms` emits `[D, P, P]`: DATA first, then parity — the sealed intra-turn
order exactly. The 5th and 6th DATA land in the *later* `Pto@20ms` turn, so they follow parity in the
flattened global view while never violating the per-turn contract.

### 6. Does early parity affect decoder or FEC grouping semantics?

**No.**
* Parity carries its own `SrLogicalRef::Fec { block_id, esi }`; the receiver groups by `block_id`.
  Reed–Solomon is an erasure code over a block — decoding needs K symbols **of that block**, in any
  arrival order. Relative order against *another* block's source is semantically irrelevant.
* Same-block inversion is structurally impossible in the authoritative path (Q4).
* In the pin's own scenario the parity block (9) shares no group with the DATA units, so no grouping
  semantics are touched.

### 7. Are total DATA/parity counts and the serialized byte budget the same?

**Yes, identical.** `(data, parity) = (6, 3)`; `total_bytes = 8400 = 6·1200 + 3·400`;
`enqueued_r_total = 3`; `emitted_r_total = 3`; `max_seq = 8`. All five of these assertions still pass
under the product default; only `last_data < first_par` fails.

### 8. Which binding decision / golden sealed the pin?

**Commit-W — "LEFTOVER-EŞDEĞERLİK-PİNİ"** (`budget_equivalence_pin.rs:1-5`). Its stated purpose is
feature equivalence for `experimental-fec-budget`:

> "OFF-koşusu=baseline; ON-koşusu (`--features experimental-fec-budget`; policy-dokunulmaz=LeftoverOnly-default)
> AYNI sabitleri geçmeli → **'ON+LeftoverOnly ≡ OFF-baseline' karar-düzeyi kanıtı**."

and the constants are explicitly described as measured *from* a baseline run:
`// PİN-SABİTLERİ (feature-OFF-baseline'dan-ölçüldü; ON-koşusu-AYNI-değerleri-geçmeli)`.

**It was never sealed as a startup-pacing contract.** It is a *decision-level equivalence* pin whose
expected values were captured from one concrete baseline trajectory.

---

## The pin's sealed property, measured directly

Running the pin's exact scenario under both feature settings and comparing the full decision stream:

| | `experimental-fec-budget` **OFF** | `experimental-fec-budget` **ON** |
|---|---|---|
| sequence | `[(0,0,1200)(0,1,1200)(0,2,1200)(0,3,1200)(3,4,400)(3,5,400)(0,6,1200)(0,7,1200)(3,8,400)]` | **identical** |
| `enqueued_r_total` | 3 | 3 |
| `emitted_r_total` | 3 | 3 |
| `total_bytes` | 8400 | 8400 |
| `max_seq` | 8 | 8 |

**`ON + LeftoverOnly ≡ OFF-baseline` holds byte-for-byte.** The property Commit-W sealed is intact.
What became invalid is the hard-coded *global ordering constant* captured from a
startup-pacing-OFF trajectory.

---

## Contract statement

| Level | Invariant | Status under startup pacing ON |
|---|---|---|
| Scheduler turn | parity is planned after DATA/recovery within a turn (`sender.rs:2831`) | **HOLDS** (0 violations, measured) |
| FEC block | parity for block *B* only staged after *B* is closed (`close_fec_block_manual`) | **HOLDS** (structural) |
| Feature equivalence | `experimental-fec-budget` ON ≡ OFF at decision level (Commit-W) | **HOLDS** (byte-identical) |
| Budget/counts | DATA count, parity count, serialized bytes, seq span | **HOLDS** (unchanged) |
| Whole-flow ordering | last DATA anywhere precedes first parity anywhere | **DOES NOT HOLD** — and is not stated by any code or design authority |

**Consequence for §A3:** the failing assertion is the only one of the five that has no authority
behind it. It is a trajectory constant, not an invariant.
