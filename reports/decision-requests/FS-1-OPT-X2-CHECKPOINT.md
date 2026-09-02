# FS-1 OPT-X2/HYBRID — FINAL CHECKPOINT (schema-conformant re-run + frozen analysis + DUR-A#2)

> **COMPLETE end-to-end.** Schema-v2 conformance built (executable KALICI-KURAL-3 gate), kernel instrumented
> observation-only, v1.3.2 schema-conformant campaign re-run, science-invariance vs v1.3.1 **PASS**, frozen analysis
> executed once, **11/11 artifacts + DUR-A#2** produced. Screening/exploratory only — **NO program-final verdict**
> (FULL-GO/NICHE-GO/RESEARCH-ONLY/K4 belong to FS-DUR-SON).

## KALICI-KURAL-3 (permanent rule, recorded)
Artifact-hash identity ≠ contract conformance. An EXECUTABLE conformance check is required before freeze. Applied here:
`validate_v2` runs at executor startup (self-validate, fail-closed) AND per-record before every atomic write; the
golden suite `fs_s6_v2_pins` (8/8) exercises it on real `run_spine` output; negative cases (missing/unknown/enum/
nullability) reject. Schema-v1 hash matched but serializer did NOT conform — exactly the failure class this rule names.

## Instrumentation — scientific/execution semantics INVARIANT (NOT "byte-unchanged"; kernel source changed)
Observation-only additive instrumentation (fs_trace decision-capture + fs_spine forward_decisions/completion_cause/
per_block_obs/control_bytes). Proof (mechanical, not assertion):
- existing golden-vector/outcome pins unchanged: **KI fs-harness 122/0**, AT experimental-fec-budget 34/0.
- consistency pin: `recomputed_decision_log_hash() == decision_log_hash()` (capture mirrors rolling hash).
- **science-invariance comparator `fs_compare_v2`: matched 240/240, shared-field mismatch 0, shared digests byte-equal
  (0xa79df572dd7b7f6a), aggregate 176/176 & 64/64 → VERDICT=PASS.** Shared scientific/execution fields (validity,
  deadline, delivered, reassembly, completion-time, base-source/intervention/arq/total bytes, retransmits, fec-decoded)
  are EXACTLY equal v1.3.1↔v1.3.2. Provenance/version/binary EXCLUDED (whitelist).
- Binary/source hashes DO change (recorded below). No RNG call / branch outcome / actuation / packet order / clock changed.
- FLAG-OFF production unaffected (fs_trace/fs_harness gated experimental-fec-budget); sender.rs/loss.rs zero-diff.

## Freeze identities (v1.3.2)
```text
schema_v2                FS-1-RAW-SCHEMA-v2.json  sha 0d59bffed2a42ccf…  (57 typed fields; supersedes v1 c7ac2a85 PRESERVED)
executor_source_commit   2f24447333a0908f573c814595d939bdf03df6e6  (full 40-hex)
execution_binary_sha256  a85df4847703d9d1e218c96ce5ca3566ae96cb8497359d970eb89e0a299b474f  (instrumented v1.3.2 runner)
comparator_binary_sha256 4b9fe13c62508035348482f5a39e7b32040e7b0cd71c2aa1a1da838c2068301c
analysis_binary_sha256   18d16c1e8288e49190150af688e4bf803d81a78f705b25aed18cf0a06004827d
dataset_concat_sha256    a39d51de4bfdbfe04113d5ed0c190f658c7dce3fc7426558f0afae0f8e6b2721  (reports/fs-1/raw/v1.3.2; 240+240)
ledger_sha256            add33d572e903c958edfe882821cf07421889039f14f013d4fc6adf038342eff
analysis_artifacts_concat 05071376b09799be9eaafc56265aaa5da5a0f551477fa5b41ebd10bbbe0b5f0f  (11 artifacts)
```

## Freeze-delta whitelist (experiment-defining pins UNCHANGED; only these classes differ)
```text
UNCHANGED (experiment-defining): manifest 12fc0a75 · prereg c12b2bee · execution-order be692dea · analysis-spec 6e2056ba ·
  oracle-map 0bee78db · coverage 3c80c685 · cargo_lock 47893d24 · run_spine/kernel scientific semantics (comparator PASS).
CHANGED (permitted classes only): version/namespace v1.3.1→v1.3.2 · execution_binary_sha256 (instrumentation) ·
  executor_source_commit (full 40-hex) · raw-schema v1→v2 (definition amendment, authorized) · serializer/validator/
  comparator/analysis binary hashes · provenance metadata. NOT a "single field changed" claim — full delta shown above.
```

## Knowledge-state at re-freeze (§10)
At re-freeze only the pre-recorded execution-checkpoint aggregate **176 completion / 64 censoring** was known. No arm-,
cell-, lineage-, pair-, tail-, or row-level outcome was opened or interpreted while building schema-v2/instrumentation/
serializer/validator/comparator (all authored against schema + synthetic fixtures). The re-freeze reason is solely
serializer/raw-schema non-conformance. 176/64 was not newly computed pre-run; it was the known limit.

## v1.3.1 preservation (§9)
v1.3.1 freeze-lock/dataset/ledger/manifest/binary: NOT deleted/moved/overwritten/edited. Marked additively:
**SUPERSEDED-INCOMPLETE-SCHEMA · reason: serializer/schema non-conformance** (not a scientific-outcome reason; no OPT-Y
scope reduction — the corrected schema is a superset serving the full analysis domains).

## INVALIDATED mechanical attempt (append-only; §10)
The first v1.3.2 dataset (committed `44e0c50`) had correct scientific outcomes but wrong `arm` LABELS (record `arm`
written from internal `SpineResult.arm`="A1" instead of canonical "A1-EARLY"), which zeroed A1-EARLY in analysis
(→ spurious DUR-A#2 ELIMINATE + all-cell O3). Preserved immutably in git `44e0c50` as the INVALIDATED attempt; fix
`2f24447` (canonical arm + arm-enum validator); dataset regenerated; comparator re-run still PASS (arm is not a shared
scientific field). New binary hash a85df484.

## Dataset + analysis results (screening; N reported; traceable via run_ids in FS-1-ANALYSIS-RESULTS-v1.3.2.json)
```text
records=240  complete=176  censored=64  invalid=0  schema=FS-1-RAW-SCHEMA-v2  invariance=PASS
per-arm (RETAIN screening): A0 39/21 · A1-EARLY 39/21 · F0 48/12 · F3 50/10 (complete/censored of 60).
DUR-A#2 (frozen mechanical criteria: decode-failure/permanent-starvation/unbounded-cost/parity-unusable/invariant): 
  A0 RETAIN · A1-EARLY RETAIN · F0 RETAIN · F3 RETAIN (none eliminated; no catastrophic mechanical failure).
oracle (per-cell; frozen oracle_class + C-P4 fairness): O0=0 O1=0 O2=12 O3=0.
  O2_FEC_SPECIFIC in all 12 cells: F0 beats A1 at MATCHED realized wire (1212B==1212B → NON_DOMINATED) — on reliability
  and/or latency. Under NaturalIid A1 completes 0/5 while F0 completes 1–4/5 (blind-dup loses both copies; parity
  reconstructs). F3 vs A1 = BUDGET_MISMATCH (2424B vs 1212B) → coding-causal attribution PROHIBITED (any F3>A1 is NOT
  coding gain). tail-erasure rows=20 (REAL_TAIL_ERASURE; terminal source-id 47).
```
**Labels:** cell/pairwise rows = MEASURED FACTS · oracle classes = FROZEN-RULE CLASSIFICATIONS · tail/mechanism = LIMITATIONS.
Screening only (n=5, no asymptotic-p). O2 here is within the frozen deterministic in-process model — NOT product-wide
FEC value, NOT kernel/netem equivalence, NOT a program verdict.

## Scope limits (mandatory)
In-process deterministic BottleneckChannel/run_spine (NOT netem/netns/kernel/NIC) · bulk-only 8×300B/48-symbol · current
ARQ without RTO/TLP tail recovery · K2 not result-bearing · F1/F4 diagnostic-only (structural-null preserved) · 12 cells/
240 rows/60 groups/n=5 · cross-environment (vs historical netem) = qualitative regime only, never identical-numeric.

## Commit trail (branch d1-sprint5c)
```text
c72ab0e A-1 schema-v2 + fs_trace decision-capture       5b85f3d A-2 fs_spine instrumentation (KI 122/0)
7a42af1 A-3 serializer+validator+golden 8/8             92ae696 A-4 fs1_runner_v2
44e0c50 B re-run(#1 INVALIDATED arm-label)+comparator   2f24447 fix arm-canonical + analysis pipeline
(this)  final checkpoint + corrected dataset + 11 artifacts
```

## NEXT (separately governed) — FS-DUR-SON
Program-final FULL-GO / NICHE-GO / RESEARCH-ONLY / K4 verdict over this screening + confirmatory campaigns. Not decided here.
