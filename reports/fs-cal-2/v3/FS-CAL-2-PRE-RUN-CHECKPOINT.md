# FS-CAL-2 v3 — YETKİ-1 PRE-RUN CHECKPOINT

> Normative source: `docs/design/FS-CAL-2-EXPERIMENT-DESIGN-v3 (1).md`
> sha256 **`115fa266a40f744013d209d36711a698096a28509ede11dd417f23aa5cdb3393`** — VERIFIED (exact match).
> Scope: YETKİ-1 (tool/grounding) ONLY. **result-bearing runs executed = 0.**
> Machine-safety honored throughout: local Cargo `-j 2` / single target / debug only; no full-workspace
> release/all-features/miri locally; no netns; no large campaign. **No commit / push / PR** (per task).
>
> ## OVERALL VERDICT: **NOT READY for YETKİ-2 — §4.2b product-path eligibility pre-check FAILED (BLOCKING).**
> All grounding/tooling gates (§4.0–§4.4, S0, M1/M2, DATA-PROCESSABLE, units, actuator pins, clippy) are
> **green**, but §4.2b — the design's own gate before any result-bearing run — did **not** pass either cell
> (P1 0.825 / P2 0.475 design-faithful; PASS bar 0.90). Per §4.2b this mandates a **new design/calibration
> version** (Abkarada authority), not a silent adjustment. **Details in items 13 and 17.** STOP here.

## 1. §4.0–§4.4 gate table

| Gate | Status | Evidence |
|---|---|---|
| §4.0 product contract (inputs/outputs enumerated from code) | **PASS** | `grounding/FS-CAL-2-PRODUCT-CONTRACT-v3.json` sha `929249dc…` (10 inputs, 8 outputs; each with Rust symbol + file:line + source sha256) |
| §4.0b joint-action contract | **PASS** | controller-executable ARQ settings = **0** ⇒ joint action `{frozen ARQ} × {R0,R2}` (row budget 39,300 stands) |
| §4.0c utility/cost contract | **PASS (grounded)** | 3 grounded product symbols (`hard_fec_byte_budget=4096`, `hard_combined_byte_cap=8192`, `hard_max_redundancy_permille=800`) + horizon grounded + `minimum_completion_gain_pp=15` marked `EXPERIMENTAL_BOUND` |
| §4.1 FEC_R2 grounding | **PASS (executable in product path)** | K=8/E=64 profile `by_id(0x03)`, real `RsSimdCodec`, r=2, sealed 101 B, emitted via product `open_fec_block_authority`+`close_fec_block_manual`→`plan_fec_parity`→`TransmitKind::FecParity`; decode `RsSimdCodec.reconstruct`+`RecoveredControl`. Golden: `r2_parity_wire_proof_exact`, `fec_decode_success_recovers_tail`, `fec_decode_failure_arq_liveness` |
| §4.2 runtime estimator grounding | **PASS** | `fate_estimator.rs` (product crate, observation-only) + fate-lineage wiring in `fs_cal2_spine.rs`; guard = product `TailRecovery::base_pto_ns` symbol; 16/16 estimator unit tests (`replay_expected_vector_exact`, guard boundary, late-ACK, dedup, no-leakage, determinism, overflow) |
| §4.3 actuator golden proof | **PASS** | 29/29 `fs_cal2_pins`: R0 zero-diff wire digest, calibrated-R0 ≡ static-R0 wire-exact, R0/R2 shared source prefix + arm-invariant snapshot, R2 parity wire-exact, decode success, decode-failure ARQ liveness, hard caps, tail-probe recovery |
| §4.4 DATA-PROCESSABLE-PASS | **PASS** | runner → strict typed schema → analysis binary → recompute-from-fate → 12 artifacts **byte-identical ×2** (final binary); + §11.1 selftest 12/12 |

## 2. Contract identities (exact file · Rust symbol · source sha256)
See `grounding/FS-CAL-2-PRODUCT-CONTRACT-v3.json` (full tables). Key source shas (working tree @ HEAD 4aa11a93):
```text
sender.rs         fc538d78…   fusion.rs        6c0ade9a…   fusion_policy.rs  4f2b0dc5…
arq.rs            1b154e6a…   loss.rs          74df36f3…   tail_recovery.rs  0cd0b9cb…
rtt.rs            69f3ea26…   fec codec.rs     (in contract) profile.rs      (in contract)
fate_estimator.rs 9116cc39…   fs_cal2_spine.rs 07cdede8…   fs_cal2_policy.rs be0c5846…
fs_cal2_gen.rs    1aeb2334…   fs_cal2_schema.rs 40a677ca…  fs_cal2_runner.rs 8037e32e…
```

## 3. Frozen ARQ contract + exact ARQ setting (§4.0b)
```text
arq_contract_id      = FSCAL2-ARQ-DEFAULTFEAT-TAILV1-v1
setting_symbol       = Sender::new() + set_tail_recovery_config(tail_on_config())
tail (frozen ON)     = enabled, min_pto 10ms, max_pto 10s, initial 1s, max_backoff 30s,
                        max_ack_delay 25ms, max_tail_probes 3   (DR-ARQ-PRODUCT-TAIL-1 identity)
retx (fixture)       = arq_retx_cap 3, reorder_k 3, rho 2/1, HandToForward
fuzzy config         = FuzzyConfig::fixture() (uncalibrated engine default — UNCHANGED)
compile features     = DEFAULT (no experimental-cold-start / experimental-fec-budget)
invalid_if_arq_setting_changes = true
```

## 4. Authoritative R2 profile / K / R (§4.1)
```text
codec        = kama_fec_codec::by_id(0x03)  (K=8, E=64, CODE_SUITE_RS_SIMD_V1, RsSimdCodec)
r            = 2 parity units per decision block
parity sealed= 101 B (== source sealed; fs_sealed_len(64))
r1/r4        = ABSENT (per §4.1)
executable in product path = YES (product Sender FEC authority; no fake harness arm)
```

## 5. Grounded cost caps (§4.0c)
```text
max_fec_bytes_per_block            = 4096   (FusionSafetyConfig::default().hard_fec_byte_budget)  GROUNDED
max_combined_recovery_overhead     = 8192   (hard_combined_byte_cap)                              GROUNDED
max_fec_wire_ratio_ppm             = 800000 (hard_max_redundancy_permille=800 ×1000)              GROUNDED
minimum_completion_gain_pp         = 15     (design-frozen)                          EXPERIMENTAL_BOUND
primary_horizon                    = {50→699,250→2499,1000→9249} ms from epoch                    GROUNDED
R2 realized: parity 202 B ≤ 4096 ; wire ratio 250000 ppm ≤ 800000                                (within caps)
```

## 6. Files added / modified (this turn)
```text
TRACKED modified (1): crates/adaptive-transport/src/lib.rs   (register observation-only pub mod fate_estimator)
NEW (product crate):  crates/adaptive-transport/src/fate_estimator.rs
NEW (harness):        crates/kama-integration/tests/common/{fs_cal2_gen,fs_cal2_policy,fs_cal2_spine,fs_cal2_schema}.rs
                      crates/kama-integration/tests/fs_cal2_pins.rs
                      crates/kama-integration/examples/fs_cal2_runner.rs
NEW (tools):          scripts/fs-cal-2/{fs_cal2_contracts,fs_cal2_manifest,fs_cal2_analyze,fs_cal2_s0,fs_cal2_diag42b}.py
NEW (artifacts):      reports/fs-cal-2/v3/**  (grounding, fixtures, this checkpoint)
User pre-existing WIP (cold-start CSVs, docs/tasks, run logs): NOT touched/staged/cleaned.
```

## 7. Tests + commands executed (local, capped) + exit status
```text
cargo test -p adaptive-transport --lib fate_estimator -j2        ok  16/16
cargo test -p kama-integration   --test fs_cal2_pins   -j2        ok  29/29
cargo clippy -p adaptive-transport --lib -j2 -D warnings          exit 0
cargo clippy -p kama-integration --test fs_cal2_pins --example fs_cal2_runner -j2 -D warnings   exit 0
cargo fmt -p adaptive-transport -p kama-integration               applied
runner emit-schema / s0 / m1 / m2 / fixtures                      ok (see below)
analysis --selftest                                               12/12 PASS
analysis pipeline ×2 (fixture)                                    PASS, 12/12 byte-identical
```

## 8. Golden R0 zero-diff + R2 wire-actuation proof
```text
R0 determinism: run_flow(cfg)×2 → identical wire_digest (fs_cal2_pins::r0_determinism_wire_digest_exact)
Calibrated→R0 ≡ Static-R0: identical wire_digest (calibrated_r0_equals_static_r0_wire_exact)
R2 wire: parity_bytes == 2×101 == 202 via product FEC authority (r2_parity_wire_proof_exact)
Decode: 2 dropped tail sources recovered by RsSimdCodec, byte-equal (fec_decode_success_recovers_tail)
Liveness: all parity dropped ⇒ ARQ (tail probe) still completes (fec_decode_failure_arq_liveness)
```

## 9. Schema + analysis pipeline hashes
```text
FS-CAL-2-RAW-SCHEMA-v3.json          sha256 f6c541a0…  (emitted from fs_cal2_schema.rs; additionalProperties=false)
FS-CAL-2-PRODUCT-CONTRACT-v3.json    sha256 929249dc…
fs_cal2_analyze.py                   sha256 9b632c9e…
S0-VERDICT.json                      sha256 ff54d4b1…
```

## 10. Two processability runs — 12-artifact hash comparison
```text
Analysis run A vs run B (same fixture dataset, final binary): 12/12 IDENTICAL
  INTEGRITY, DATA-CONFORMANCE, PAIR-RECORDS, CELL-RESULTS, SUPPORT-BITMAP, CANDIDATE-SUMMARY,
  IDENTIFIABILITY, CALIBRATION-POLICY, HOLDOUT-RESULTS, NETNS-FIDELITY, FINAL-REPORT, FINAL-CHECKPOINT
Data reproducibility: fixtures raw-run1 vs raw-run2 dataset_concat_sha256 IDENTICAL.
Fixture pipeline verdicts (EXPECTED): conformance PASS (45/45); identifiability overall=FALSE
  (K1 direction-diversity + K7 support-activation FAIL) — CORRECT: fixture data (n=5/cell) yields no
  R2_PREFERRED cell, so the pipeline correctly REFUSES to certify. DATA-PROCESSABLE validates
  processability + determinism (both PASS), NOT science.
```

## 11. §12.0 M1 / M2
```text
M1 (outcome-blind; wall+RSS only, values mechanically discarded): n=20 wall_time_ms=399368 peak_rss_bytes=13336576
M2 (parallelism determinism): 2-proc event_log_sha256 == 8-proc event_log_sha256  → PASS
   (final binary: 081bd209…)  ⇒ campaign may run parallel across processes; netns stays serial (YETKİ-3)
```

## 12. S0 (10,000 synthetic estimator traces)
```text
overall_pass = TRUE  (verdict: grounding/s0/S0-VERDICT.json sha ff54d4b1…)
  AUC IID-vs-GE-L3 / GE-L10 @ p∈{10,20,30}%: 95% lower bound ≥ 0.90 (all ≥ 0.9989)
  MARKOV-BD burst-delta medians within |0.05| of b; ordering IID<b15<b35<GE-L3<GE-L10 at each p
  MARKOV-BD eligibility ≥ 95% (the §3.5-scoped gate)
  GE eligibility OBSERVATIONAL (not a §3.5 gate): GE-L10-p10 = 0.81 → low-loss long-burst snapshots
    are estimator-INELIGIBLE ⇒ R0 fallback (safe/expected; recorded)
  p=5% eligibility LOW (0.564–0.944) → p≤5% EXCLUDED from support by the mechanical rule (§3.5)
```

## 13. §4.2b 80-run product-path eligibility pre-check (P1 IID-30 / P2 GE-L10-20, RTT 1000 ms)
```text
STATUS: **FAILED — BLOCKING.**  (grounding/diag-42b/DIAG-42B-VERDICT.json + DIAG-42B-ROOTCAUSE.json)

PASS bar (each cell, separately): estimator_eligible ≥ 0.90, runtime_features_present == 1.0, schema/inv == 0.

                          eligible/40   fraction   rfp    schema  inv   PASS
  as-run (see NOTE):
    P1 IID-30 RTT1000        10/40        0.250     1.0     0      0     NO
    P2 GE-L10-20 RTT1000     19/40        0.475     1.0     0      0     NO
  design-faithful (see NOTE; EXACT recompute on same raw fields):
    P1 IID-30 RTT1000        33/40        0.825     1.0     0      0     NO
    P2 GE-L10-20 RTT1000     19/40        0.475     1.0     0      0     NO

  runtime_features_present = 1.0 and schema/invariant failures = 0 in BOTH cells (tooling is sound;
  the failure is scientific/product-path eligibility, not a data-integrity failure).

NOTE — as-run vs design-faithful:
  The runner's recorded `warmup_complete` is computed as `drained && !timed_out` where `drained` =
  ALL 384 warm-up units DELIVERED/recovered (fs_cal2_spine.rs:682-694). That is a HARNESS DEFECT vs the
  design: §2.3 + §3.1 + line-130 define warm-up completion by 256 FATE-KNOWN packets (a lost packet's
  fate finalizes on loss-declaration + reordering guard, WITHOUT requiring delivery), never by full
  delivery. Correcting the flag to design-faithful "epoch-reached" DOES NOT change snapshot timing
  (flag-only), so eligibility recomputed from the recorded raw fields (N, n00/n01/n10/n11, stale,
  converged) is EXACT → the design-faithful column above. It raises P1 0.25→0.825 but changes NO verdict.

WHY IT STILL FAILS (design-faithful, most-generous reading) — real product-path properties at RTT=1000 ms:
  • P2: 8/40 runs are SENDER COLD-START STALLS — sender never leaves PreConverge, zero observed
    transmission, all counters 0, epoch pinned at the 18498 ms timeout (converged=False, stale=True).
    GE-L10 opening burst at RTT=1000 ms. Product-path/sender phenomenon, NOT an eligibility-flag bug.
    OPEN QUESTION: genuine product cold-start vs harness sender-driving limitation at high RTT.
  • telemetry_stale: driven by the GROUNDED product FusionSafetyConfig max_telemetry_age_ns = 2 s
    (fusion_policy.rs:142). At RTT=1000 ms the ACK cadence is ~1 s, so any loss burst exceeds 2 s →
    the estimator is LEGITIMATELY stale (real product behavior). P1: 3/40 stale, P2: 17/40 stale.
  • N<256 fate-known: exactly the failure §4.2b is written to catch ("uç hücrelerde 384 warm-up
    paketinden 256 fate-known çıkmayabilir"). P1: 4/40, P2: 12/40.
  • Epoch-trigger direction is settled: non-zero-N epochs land at 10.3–18.5 s (delivery-drain). A
    "faithful" fate-known-drain trigger would fire ~2.5 s, when the sender is EVEN LESS converged →
    WORSE. The current delivery-based trigger is the MOST GENEROUS faithful reading and still fails.
    ⇒ There is NO faithful harness fix that makes §4.2b pass.
```

## 14. Remaining blockers / risks
```text
- **§4.2b PRE-CHECK FAILED (BLOCKING) — see item 13.** This is the design's gating pre-check before any
  result-bearing manifest/freeze/run. Per §4.2b, a fail means "warm-up uzunluğu, pencere boyutu veya
  eligibility eşikleri yeniden değerlendirilir; result-bearing training başlamaz. Her değişiklik yeni
  tasarım sürümü gerektirir ve S0 ile bu pre-check yeniden çalıştırılır." ⇒ NEW DESIGN/CALIBRATION
  VERSION required (Abkarada authority); YETKİ-2 NOT authorized.
- Hosted CI: BLOCKED this turn (task forbids commit/push ⇒ cannot trigger hosted CI). Local scoped gates
  (fmt/clippy-D/unit/actuator/DATA-PROCESSABLE) all green; NOT a substitute for the hosted full gate.
- Full §7.3 candidate grid runs only in fixture-COARSE mode locally (labeled); the FULL grid fit is a
  YETKİ-2/hosted concern (documented in fs_cal2_analyze.py).
- No result-bearing rows produced (by design). Manifest is PREVIEW (row budget 39,300 verified) — NOT frozen.
- Product behavior UNCHANGED: fate_estimator has no product caller (observation-only, like fusion_policy);
  sender/arq/loss/rtt/fusion/tail_recovery byte-unchanged. The §4.2b failure is a property of the
  estimator+warm-up+staleness design at RTT=1000 ms, NOT a product regression.
- Harness defect recorded (NOT fixed this turn): warmup_complete = delivery-based (fs_cal2_spine.rs:694)
  vs design fate-known semantics. Left as-is so code stays consistent with the dataset; correction belongs
  to the §4.2b-failure design-version response (flag + the deeper delivery-vs-fate-known epoch-trigger
  question) — see item 17.
```

## 15. result-bearing runs executed = 0
Confirmed. S0 (10,000 synthetic estimator traces), the §4.2b 80 estimator-only diagnostic runs, and M1/M2
are explicitly NOT result-bearing (design §3.5/§4.2b/§12.0) and do not enter training/holdout/fit.

## 16. signal pilot / training / holdout / netns = NOT RUN
Confirmed. 800-run pilot, 14,800-run training, 21,600 in-process holdout, 2,100 netns holdout: **NOT RUN.**
No calibration fit, no policy freeze, no `(tail,r>0)` result-bearing experiment, no old-data holdout.

## 17. YETKİ-2 readiness — **NOT READY (BLOCKED by §4.2b)**
```text
VERDICT: NOT READY. The §4.2b product-path eligibility pre-check FAILED both cells (item 13), and per
the design §4.2b the required response is a NEW DESIGN/CALIBRATION VERSION — result-bearing training does
NOT start. This is a scientific DUR at a defined gate FAIL (exactly the stop the task specifies), not a
mechanical detail.

Everything ELSE in YETKİ-1 is green and stands as delivered tooling:
  §4.0–§4.4 gates, S0 (10,000 traces), M1/M2, DATA-PROCESSABLE (12/12 byte-identical), 16/16 estimator
  units, 29/29 actuator pins, scoped clippy -D clean. Contracts + manifest (PREVIEW) + analysis pipeline
  are built and self-tested. These do not require rework for their own sake.

WHAT THE §4.2b FAILURE REQUIRES (Abkarada authority — a new design version, NOT this turn):
  (1) Warm-up-drain SEMANTICS decision: correct warmup_complete (delivery → fate-known/epoch-reached,
      §2.3/§3.1/line-130) AND decide the epoch-TRIGGER (delivery-drain vs fate-known-drain). Evidence
      shows fate-known-drain fires ~2.5 s and worsens convergence — so this is a genuine design choice,
      not a mechanical fix.
  (2) Re-evaluate (per §4.2b): warm-up LENGTH (384), estimator WINDOW (256), and/or eligibility handling
      at RTT=1000 ms — because even the most-generous faithful reading yields P1 0.825 / P2 0.475.
  (3) Sender cold-start at RTT=1000 ms + GE-L10: determine whether the 8/40 PreConverge stalls are a
      genuine product cold-start property or a harness sender-driving limitation at high RTT. This gates
      whether the fix is a design change, a harness fix, or both.
  (4) The 2 s telemetry-staleness bound (grounded product config) vs RTT=1000 ms ACK cadence: decide
      whether high-RTT operation needs a different bound or the pre-check cells need revisiting.
  Then: re-run S0 + §4.2b under the new version before any YETKİ-2 authorization.

Hosted CI remains separately BLOCKED (commit/push forbidden this turn); not the gating issue here.

THIS TURN STOPS HERE. No result-bearing runs. No code change to the blocked warm-up path. No YETKİ-2.
```
