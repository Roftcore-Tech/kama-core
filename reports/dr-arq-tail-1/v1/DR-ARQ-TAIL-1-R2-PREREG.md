# DR-ARQ-TAIL-1 R2 — PREREGISTRATION (deconfounding grid; pre-outcome frozen)

> New preregistered deconfounding grid with a seed/repetition domain DISJOINT from FS-1 v1.3.2. Experiment-only
> deterministic tail-recovery deconfounder — NOT product ARQ, NOT RFC 9002, NOT kernel/netem/NIC. Namespace:
> `reports/dr-arq-tail-1/v1/`. Minimal-delta: reuses schema-v2 record/validator, atomic writer, ingest, frozen primitives.

## Arms (arm mapping)
```text
A0-FROZEN  = SpineArm::A0     (current ACK-driven ARQ; no tail recovery)
A0-TAIL    = SpineArm::A0Tail (A0 + frozen experiment-only bounded tail probe; PTO=2·RTT, ≤2 probes)
F0         = SpineArm::F0{r:2}(FEC K8, 12 repair, 1212B intervention)
A1-EARLY   = SpineArm::A1{r:2}(blind duplication r=2, 1212B intervention — matched-budget coding-vs-dup control)
```

## Strata (impairment semantics — frozen; NOT changed after outcomes)
```text
S1 terminal-tail loss        : ExactDeficit drop-set INCLUDES terminal source-id 47 (ARQ-unrecoverable tail).
S2 non-tail loss same-budget : ExactDeficit same drop_count as S1 but EXCLUDES 47 (mid-stream; ACK-recoverable).
S3 NaturalIid                : p=0.20 hash-keyed per-(id,role,attempt) loss (blind-dup can lose both copies).
S4 NaturalBurst              : time-domain burst drop intervals (arm-independent).  [needs ForwardImpairment::NaturalBurst]
S5 probe-loss-eligible tail  : S1 tail BUT RetransmitSource ALSO subject to loss (RedundancyPolicy::Iid on the
                               retransmit channel) — SEPARATE from ExactDeficit's "only InitialSource drops"; the probe
                               itself can be lost → exercises bounded-failure. EXPLICITLY LABELED probe-loss-eligible.
                               [needs ForwardImpairment carrying a redundancy policy]
Channel/impairment semantics are frozen pre-outcome; the "only InitialSource drops" default (S1-S4) vs probe-loss (S5)
distinction is declared here and not altered after seeing results.
```

## Seed domain (disjoint from FS-1)
```text
DR-ARQ-TAIL-1 uses master tag "DRARQTAIL1" (≠ FS-1 domain). Per-cell trace seed = domain_seed("DRARQTAIL1", cell_id||rep).
Terminal-tail drop-sets are constructed to include id 47 (S1/S5); non-tail sets exclude 47 (S2). RTT grid {50,250,1000}ms
(reused; deadline/horizon reused from FS-1 config for comparability). This grid does NOT reuse FS-1's per-row seeds.
```

## Sample size (frozen; no adaptive stopping)
```text
N = 100 repetitions per arm/cell (pre-registered). Rationale: worst-case ~±0.10 95% Wilson precision on a binomial
completion rate. NO early stopping, NO N change based on outcomes. If a cell is genuinely runtime-infeasible, a justified
fixed N is set BEFORE seeing results (documented), never adaptive.
```

## Primary endpoints (per arm/cell/stratum; N_valid & N_censored always shown)
```text
completion-by-deadline · censoring · completion latency (complete rows only; frozen censoring rule — censored is NOT a
latency value) · total realized wire bytes · ARQ retransmit bytes · FEC parity bytes · probe count · spurious probe
count/bytes · remaining fragments · completion cause · tail status.
```

## Primary contrasts
```text
C1 tail-recovery effect     : A0-TAIL vs A0-FROZEN (esp. S1) — completion gain · added wire cost · added latency ·
                              spurious probe cost · bounded-retry failures.
C2 FEC vs tail-capable ARQ  : F0 vs A0-TAIL — verify per-cell realized-wire budget FIRST. equal→causal; unequal→
                              Pareto/cost-effectiveness; budget-mismatch→NO "FEC superior" verdict.
C3 coding vs blind dup      : F0 vs A1-EARLY at matched budget, independent seeds (re-confirm FS-1's matched-budget result).
C4 probe robustness         : A0-TAIL under S5 (probe can be lost) — 1st/2nd probe recovery · bounded failure · added overhead.
```

## Interpretation bounds (§6; binding)
```text
Keep separate and non-substitutable: (1) FEC > blind duplication · (2) FEC > frozen ACK-driven ARQ · (3) FEC > tail-capable
ARQ · (4) at what cost FEC is Pareto-optimal. A0-TAIL success does NOT mean "Kama product ARQ fixed" — correct label:
experiment-only deterministic tail-recovery deconfounder. In-process results are NOT quinn/TCP/KCP or real-kernel results.
```

## Pre-outcome freeze + execution plan (§7)
```text
Freeze (pre-outcome): this prereg · grid/manifest · seed domain · arm mapping · impairment semantics · N · endpoints/
contrasts · analysis runner/binary · exact execution command. Then, without separate GO: run R1 (done — DIAGNOSTIC) →
run R2 once → conformance/integrity checks → frozen analysis → final DR-ARQ-TAIL-1 checkpoint. Mechanical errors fixed
autonomously; failed attempts preserved INVALIDATED; scientific rules never changed by outcome.
```

## Status (honest)
```text
S1/S2/S3 buildable with existing SpineCfg::diag/iid. S4/S5 require additive ForwardImpairment plumbing (NaturalBurst;
redundancy-policy on ExactDeficit) — harness-only, gated, FS-1 arms unaffected. R2 result-bearing execution (5 strata ×
4 arms × N=100) is the remaining result-bearing step under this frozen prereg.
```
