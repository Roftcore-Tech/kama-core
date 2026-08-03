# DR-ARQ-TAIL-1-R2A-SCOPE-AMENDMENT (additive; pre-outcome)

> Additive scope amendment to the IMMUTABLE `DR-ARQ-TAIL-1-R2-PREREG.md` (not edited). Created BEFORE any R2
> result-bearing outcome was seen.

## Knowledge state at amendment time
```text
KNOWN    : FS-1 v1.3.2 results (locked) + R1 diagnostic replay (5 terminal-tail configs; A0-FROZEN 5/5 censor;
           A0-TAIL 5/5 recover; F0 2/5 complete).
UNKNOWN  : all R2 result-bearing grid outcomes (no R2 result-bearing row has been executed or seen).
REASON   : S4 (NaturalBurst) and S5 (probe-loss-eligible) require NEW additive ForwardImpairment plumbing; R2A can
           answer C1/C2/C3 NOW with the already-validated instrument (SpineCfg::diag/iid + frozen A0Tail probe).
           No outcome-based selection was made — this split is by instrument-readiness, not by results.
```

## R2A scope (this amendment)
```text
strata   : S1 terminal-tail · S2 non-tail same-budget · S3 NaturalIid
contrasts: C1 tail-recovery effect (A0-TAIL vs A0-FROZEN) · C2 F0 vs A0-TAIL · C3 F0 vs A1-EARLY
```

## Deferred to R2B (separate future amendment)
```text
strata   : S4 NaturalBurst · S5 probe-loss-eligible tail
contrasts: C4 probe robustness
```

## Correct label (NOT a scientific narrowing)
```text
PRE-OUTCOME SCOPE SPLIT — R2A evidence does NOT cover burst loss or probe-loss robustness.
```

## Frozen grid (typed; single source generates manifest/dispatch/expected-count/grouping)
```text
RTT {50,250,1000} ms · S1 ExactDeficit drop_count {1,2,4} incl terminal id 47 · S2 ExactDeficit drop_count {1,2,4}
excl 47 (same count as S1) · S3 NaturalIid p=0.20 · arms {A0-FROZEN,A0-TAIL,F0,A1-EARLY} · N=100 (no adaptive stopping).
cells = 9(S1)+9(S2)+3(S3) = 21 · rows = 21×4×100 = 8400. Disjoint seed domain "DRARQTAIL1". Reuses schema-v2 record/
validator + atomic writer + frozen fs_analysis primitives (coding_causal_fairness). Preflight: ALL PASS (21/21, 8400,
S1⊇47, S2⊉47, pairing exact, per-arm valid record, unknown-arm fail-closed, byte-identical synthetic builds).
```
