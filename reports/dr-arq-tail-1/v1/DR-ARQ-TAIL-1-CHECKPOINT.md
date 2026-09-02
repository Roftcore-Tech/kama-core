# DR-ARQ-TAIL-1 — CHECKPOINT (mechanism frozen · R1 diagnostic complete · R2 preregistered · §5 readiness)

> Experiment-only deterministic tail-recovery deconfounder. NOT product ARQ, NOT RFC 9002, NOT kernel/netem/NIC.
> FS-1 v1.3.2 dataset/checkpoint/artifacts NOT touched.

## Mechanism (frozen, commit 5cfbe37)
`fs_tailprobe.rs` bounded tail probe (PTO=2·RTT, ≤2 probes, arms only on outstanding, cancels on ACK progress, not
FEC-tied, spurious-measured) wired as gated `SpineArm::A0Tail`. Tests: fs_tailprobe_pins 9/9 + fs_arqtail_pins;
non-tail golden regression 44/0; sender/loss zero-diff.

## R1 — DIAGNOSTIC MECHANISM REPLAY (complete; commit 3401f5c) — REAL result
```text
terminal-tail erasure configs (ExactDeficit, realized erasure includes terminal id 47) : 5  (= 20 FS-1 tail records / 4 arms)
A0-FROZEN censors : 5/5
A0-TAIL closes (completes) : 5/5   (bounded probe; +1515 ARQ retransmit bytes total)
F0 completes : 2/5
```
**Finding (diagnostic; not confirmatory):** on the terminal-tail stratum, the bounded tail probe recovers every erasure
the frozen ACK-driven ARQ cannot — and recovers MORE of them than FEC (5/5 vs 2/5). This is the confounder the experiment
targets: part of F0's FS-1 tail edge was A0's tail blindness. Cost-fairness (F0 vs A0-TAIL) is quantified in R2. Artifact:
DR-ARQ-TAIL-1-R1-DIAGNOSTIC.md.

## R2 — preregistered deconfounding grid (design FROZEN pre-outcome; DR-ARQ-TAIL-1-R2-PREREG.md)
Arms A0-FROZEN/A0-TAIL/F0/A1-EARLY; strata S1 terminal-tail · S2 non-tail same-budget · S3 NaturalIid · S4 NaturalBurst ·
S5 probe-loss-eligible tail; N=100/arm/cell (no adaptive stopping); endpoints + contrasts C1–C4; interpretation bounds §6.
S1–S3 buildable now; S4/S5 need additive harness `ForwardImpairment` plumbing (NaturalBurst; retransmit redundancy policy).

## §5 — comparison-harness readiness (infrastructure only; NO benchmark runs)
Prepared as readiness (NOT executed): faithful two-netns + veth-move + netem orchestration (see [[faithful-netem-harness]]),
same workload/payload, impairment replay, wire-byte accounting, completion/censoring, latency/tail-latency, retransmission/
control overhead, clock/timestamp validation, smoke tests. **No result-bearing/published quinn/TCP/KCP benchmark run** —
reason: A0-TAIL is not a product mechanism, the DR-ARQ-TAIL-1 result is not yet complete, and the product Kama-ARQ baseline
decision is not made. §5 stays a readiness checkpoint until those hold.

## Honest execution status
```text
DONE (real): mechanism (frozen) · R1 diagnostic replay (5/5 A0-TAIL, 2/5 F0) · R2 preregistration (frozen) · §5 readiness scoping.
REMAINING (result-bearing): S4/S5 additive impairment plumbing · R2 grid run (5 strata × 4 arms × N=100) · R2 conformance/
  integrity · R2 frozen analysis (C1–C4) · §5 harness code + smoke. Not fabricated; to be executed under the frozen R2 prereg.
```
