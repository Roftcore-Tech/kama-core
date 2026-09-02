# DR-ARQ-TAIL-1 R2A-Core — FINAL REPORT

> Experiment-only deterministic deconfounder (S1 terminal-tail / S2 non-tail same-budget / S3 NaturalIid; N=100/arm/cell; disjoint seed domain DRARQTAIL1). NOT product ARQ, NOT RFC 9002, NOT kernel/netem. PRE-OUTCOME SCOPE SPLIT — R2A does NOT cover burst loss or probe-loss robustness (R2B).

## Integrity
records=8400 (expected 8400) · 21 cells × 4 arms × N=100 · schema=FS-1-RAW-SCHEMA-v2 · per-record validated fail-closed.


## HEADLINE FINDINGS (per-cell primary; screening; N=100; 95% Wilson CI)

**C1 — tail-recovery effect (A0-TAIL vs A0-FROZEN).** Large on S1 terminal-tail, never harmful (A0-FROZEN-only-complete = 0 in every cell):
- s1-rtt50-d1:  A0-FROZEN 0/100 → **A0-TAIL 100/100** (+101 realized bytes ≈ 1 retransmit).
- s1-rtt50-d4:  A0-FROZEN 33/100 → **A0-TAIL 100/100** (+519 bytes).
- s1-rtt1000-d4: A0-FROZEN 29/100 → **A0-TAIL 74/100** (+650 bytes).
Modest on S2 non-tail (A0-TAIL recovers 11–23/100 more), negligible on S3 NaturalIid (heavy p=0.20 loss is not a tail problem).

**C2 — FEC vs tail-capable ARQ (the deconfounding payoff).** Over the 900 S1 paired rows: F0-only-complete = **26**, **A0-TAIL-only-complete = 64**, both = 810 — and F0 costs MORE realized wire in every S1 cell (budget UNEQUAL → Pareto/cost-effectiveness, NOT a causal "FEC superior" verdict, per §6). Example s1-rtt50-d4: **A0-TAIL 100/100 @5224B Pareto-dominates F0 83/100 @5767B** (more completion, less cost). Only s1-rtt1000-d4 shows F0 adding completion (F0 81/100 vs A0-TAIL 74/100), still at higher cost (5585 vs 5218 B). **⇒ F0's FS-1 terminal-tail edge was substantially the frozen ARQ's tail blindness; a bounded tail probe closes most of it at lower wire cost.** This does NOT say "Kama product ARQ fixed" — A0-TAIL is an experiment-only deterministic deconfounder.

**C3 — coding vs blind duplication (independent confirmation).** Over the 2100 paired rows at matched realized-wire budget (frozen coding_causal_fairness NON_DOMINATED in 21/21 cells): F0-only-complete = **868**, **A1-EARLY-only-complete = 0**. F0 completes 868 rows blind duplication does not; duplication completes none F0 does not. Example s1-rtt50-d1: **F0 100/100 vs A1-EARLY 0/100 at equal 6060 B**. ⇒ the FS-1 matched-budget coding>duplication result is **independently confirmed in the disjoint DRARQTAIL1 seed domain** (NOT generalized to burst — R2B).

**Separation (§6, binding):** these are four distinct, non-substitutable facts — FEC > blind duplication (C3, strong); FEC > frozen ACK-driven ARQ (implied); FEC vs tail-capable ARQ (C2: NOT dominant on the tail stratum, and costlier); and the cost level at which FEC is Pareto-optimal (per-cell, above). In-process deterministic model only.

## C1 — tail-recovery effect (A0-TAIL vs A0-FROZEN), S1 terminal-tail cells
| cell | A0-FROZEN complete | A0-TAIL complete | A0-TAIL−A0 retx(max) | A0-TAIL−A0 arq_bytes(max) |
|---|---|---|---|---|
| s1-rtt1000-d1 | 0/100 | 100/100 | 1 | 101 |
| s1-rtt1000-d2 | 32/100 | 100/100 | 46 | 4646 |
| s1-rtt1000-d4 | 29/100 | 74/100 | 44 | 4444 |
| s1-rtt250-d1 | 0/100 | 100/100 | 1 | 101 |
| s1-rtt250-d2 | 0/100 | 100/100 | 47 | 4747 |
| s1-rtt250-d4 | 32/100 | 100/100 | 44 | 4444 |
| s1-rtt50-d1 | 0/100 | 100/100 | 1 | 101 |
| s1-rtt50-d2 | 0/100 | 100/100 | 47 | 4747 |
| s1-rtt50-d4 | 33/100 | 100/100 | 44 | 4444 |

## C2 — F0 vs A0-TAIL (cost-aware; see PAIRWISE.csv for realized-wire relation)
Budget fairness verified per cell (realized total bytes). Where costs unequal → Pareto/cost-effectiveness, NOT a causal 'FEC superior'/'ARQ superior' verdict. Question: in which cells does FEC add completion over tail-capable bounded ARQ, and at what wire-byte cost — read from PAIRWISE.csv (F0 x_only_complete vs A0-TAIL y_only_complete + budget_relation).

## C3 — F0 vs A1-EARLY (matched-budget coding vs blind duplication)
Realized-intervention parity (frozen coding_causal_fairness primitive) NON_DOMINATED in 21/21 cells. Independent-confirmation claim holds ONLY where R2A's new-seed-domain completion rates + paired discordance actually support it (see CELL-RESULTS.csv + PAIRWISE.csv C3 rows). NOT generalized to burst.

## Interpretation bounds (§6)
Keep separate: FEC>blind-dup · FEC>frozen-ACK-ARQ · FEC>tail-capable-ARQ · FEC Pareto cost level. A0-TAIL success ≠ 'Kama product ARQ fixed'. In-process only. Per-cell primary; stratum aggregate secondary (no silent pooling).

## Artifacts
DR-ARQ-TAIL-1-R2A-CELL-RESULTS.csv (per-cell/arm: N, completion, Wilson CI, latency, cost) · DR-ARQ-TAIL-1-R2A-PAIRWISE.csv (paired discordance + realized-cost relation, C1/C2/C3). Every row traceable to run_id dra1--<cell>--<arm>--rep<n>.

