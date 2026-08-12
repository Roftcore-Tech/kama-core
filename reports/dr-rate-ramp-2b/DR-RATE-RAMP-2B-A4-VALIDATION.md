# DR-RATE-RAMP-2B — §A4 VALIDATION BATTERY

**Result: `A4 DOES NOT PASS` → startup pacing reverted to `default = false`; FAZ B NOT ENTERED.**

The FEC-ordering question (§A2/§A3) was fully resolved in favour of §A3.2. But enabling startup
pacing by default surfaced a **second, different** invariant failure that the FEC-ordering
supersession does not cover and that could not be resolved without uncertainty. §A3.2 is explicit:
*"Belirsizlik varsa supersession yapma; default startup pacing'i geçici OFF'a al ve DUR."*

---

## §A1 — pin compile/run visibility

| Question | Answer |
|---|---|
| Was the test hidden only by a cfg error? | **Yes, and there was a second one.** `budget_equivalence_pin.rs` did not compile under default features (`TransmitKind::CsProbe` arm feature-gated after DR-CS-3 un-gated the variant). Fixing that exposed a **new** break: `crates/adaptive-transport/src/udp_runner.rs:1102` had the identical defect, so **the LIB itself failed to compile under `--features experimental-fec-budget` alone**. |
| Can CI/test filters silently exclude it? | **Yes — structurally.** `.github/workflows/ci.yml:18` runs only `cargo test --all --all-features`. The **default** feature build is never tested. Any default-only breakage is invisible to CI by construction. Note the `--all-features` combination *would* have caught both failures; they went unnoticed because nothing has been committed since `4aa11a93`. |
| Are per-combination test counts reported? | **Now yes** — table below. |

### Compile matrix (after the two repairs) — 7/7 combinations compile

`<default>` · `experimental-cold-start` · `diag-sched-telemetry` · `experimental-fec-budget` ·
`cold-start + fec-budget` · `fec-budget + diag` · `--all-features`

### Test counts per combination

| combination | before (startup pacing ON) | after (default OFF) |
|---|---|---|
| `<default>` | 244 passed, `budget_equivalence_pin` **FAILED** | **244 passed, 0 failed** |
| `experimental-cold-start` | 158 passed, **2 targets failing** (run aborted early) | **267 passed, 0 failed** |
| `diag-sched-telemetry` | 244 passed, **1 failing** | **244 passed, 0 failed** |
| `experimental-fec-budget` | **LIB DID NOT COMPILE** | **291 passed, 0 failed** |
| `--all-features` | 198 passed, **2 targets failing** | **316 passed, 0 failed** |

No hidden or non-running pin remains.

---

## §A4 battery results

| Item | Result |
|---|---|
| `budget_equivalence_pin` | **5/5 PASS** (1 original re-scoped + 4 new sealing tests) |
| DATA-before-PARITY exact pins | **PASS** — per-turn violations 0 with pacing ON *and* OFF; block-level parity-after-close PASS |
| FEC_R0 / FEC_R2 scheduler + unit tests | **PASS** — `r0_zero_parity`, `r0_determinism_wire_digest_exact`, `r2_parity_wire_proof_exact`, `r2_within_grounded_hard_caps`, `fec_decode_success_recovers_tail`, `fec_decode_failure_arq_liveness`, `sprint5c_fec_emit`, `tv_fec` |
| Rate-ramp locked replay | **PASS** — `rate_lock_seeds_escape_the_fixed_point` (fixed points still escape under the explicit ON arm) |
| Rate-ramp 260 paired panel | **not required** — product *semantics* did not change. The explicit ON and OFF arms are byte-identical to their v3.5 behaviour; only which one is the default moved |
| No-loss wire check | **PASS — 30/30 pairs byte-identical** (`wire_digest`, `source_prefix_digest`, delivered all identical ON vs OFF; `inv_fail = 0`). The default flip is **wire-neutral under no loss** |
| Adaptive transport suite | **PASS** — 244 / 267 / 244 / 291 / 316 across the matrix, 0 failed |
| FS-CAL pins | **33/33 PASS** |
| kama-integration suites | **71 default / 109 `experimental-cold-start`, 0 failed** |
| Feature compile matrix | **PASS — 7/7** |
| Scoped clippy `-D warnings` | **clean ×4** (adaptive-transport default + all-features; kama-integration default + cold-start) |

### Acceptance verdict

| §A4 acceptance criterion | Result |
|---|---|
| Rate-lock fixed points still resolved | **PASS** |
| Sealed FEC ordering invariant PASS | **PASS** |
| FEC_R2 still executable | **PASS** |
| Total DATA/parity byte budget unchanged | **PASS** (6/3, 8400 B, seq span 8) |
| No no-loss completion/wire regression | **PASS** (30/30 byte-identical) |
| No hidden / non-running pin | **PASS** |
| — implicit: the suite is green **with startup pacing default-ON** | **FAIL** |

The battery passes on every listed criterion, but **only after reverting the default**. It does not
pass with startup pacing default-ON, which is what §A4 was gating. Therefore: default OFF, STOP.

---

## The blocking finding — EK4-A2, a SECOND invariant

`dr_cs1_acceptance::f2b_di1c_rearmed_pto_lifecycle_exposure` fails under `experimental-cold-start`
(and `--all-features`) **solely because of startup pacing default-ON** — verified by toggling only
that axis (the file was restored with `cp`, never `git checkout`; `git diff` on it is empty).

Failure point: `expect_retransmit_via_wakeup` →
`s.next_wakeup_ns().expect("(d)-deadline expose-edilmeli (EK4-A2)")` returns **`None`** while unit 1
is still outstanding.

### What was measured

| | startup pacing OFF | startup pacing ON |
|---|---|---|
| emissions before the ACK | `Data seq0=u1`, `CsProbe`, `Data seq1=u2` | **identical** |
| the ACK batch (`acked_seqs = [1, 2]`) | emits nothing | additionally emits `Data seq2 = u3` |
| `next_wakeup_ns()` right after the ACK | `Some(7307010999)` | **`None`** |
| u1 retransmitted on idle ticks only | yes, at **7500 ms** | yes, at **4500 ms** |

**So the retransmit is NOT lost — it happens, and 3 s earlier.** The functional recovery is intact and
strictly better.

### Why this is nevertheless not waved through

`next_wakeup_ns()` is the accessor a real event loop uses to decide how long to sleep. `None` while a
unit is outstanding tells a runtime "nothing is scheduled". The probe above drove ticks every 500 ms
unconditionally, so it demonstrates the retransmit fires **when ticks are driven** — it does **not**
demonstrate that an idle, sleep-until-next-wakeup runtime would ever wake. Given that this whole work
stream exists because of cold-start stalls, an unexposed deadline is precisely the class of thing that
must not be assumed benign.

This is a **different invariant** (ARQ/PTO deadline lifecycle, EK4-A2) from the FEC ordering one, and
`DR-RATE-RAMP-2B-ORDERING-SUPERSESSION` deliberately does **not** cover it. Resolving it needs either
a product change (expose the re-derived deadline) or an authority ruling that `None` is correct when
the retransmit is already due — neither of which is in this turn's scope.

---

## Changes made

| File | Change | Class |
|---|---|---|
| `crates/adaptive-transport/src/udp_runner.rs` | un-gate the `TransmitKind::CsProbe` match arm (value `0x19` unchanged) | compile-visibility repair, no semantic effect |
| `crates/adaptive-transport/src/sender.rs` | `StartupPacingConfig::default()` → `enabled: false`, with the reason recorded inline | **default revert** per §A3.2/§A4 |
| `crates/adaptive-transport/tests/budget_equivalence_pin.rs` | original pin preserved verbatim + explicitly re-scoped to pacing-OFF; 4 new sealing tests | test |
| `crates/kama-integration/tests/fs_cal2_pins.rs` | the two **default-encoding** pins now track the default (`default_equals_matching_explicit_treatment`, `explicit_arm_differs_from_default`); assertions unchanged | test |
| `crates/kama-integration/tests/dr_cs3_*.rs` | bootstrap senders now set the startup-pacing axis **explicitly** instead of relying on the default (they assert the handoff *into* it) | test |

The startup pacing mechanism, its formula, the FEC drain, the pacer and the scheduler call order are
**untouched**. The mechanism remains fully available via `set_startup_pacing_init`.

---

## Consequence

`FAZ B NOT ENTERED.` No panel V1 corrigendum, no panel contract V2, no channel contracts, no fresh
campaign, no default promotion. Per §A4: *"PASS olmazsa startup pacing default-OFF'a dönmeli ve burada
DUR edilmeli. FAZ B'ye geçme."*
