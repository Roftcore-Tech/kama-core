# FS-1-ANALYSIS-REPORT-v1.3.2

## MANDATORY SCOPE LIMITS
1. FS-1 characterizes the frozen deterministic in-process BottleneckChannel/run_spine model — NOT kernel networking / netem / netns / real NIC.
2. No real-kernel/production/hardware/netem equivalence claim.
3. Historical two-netns/netem results are a DIFFERENT environment — cross-environment regime replication only (qualitative), never identical-numeric.
4. Bulk-only workload (8×300B / 48 symbols); single-block/small-message = FS-2.
5. Result-bearing arms {A0,A1-EARLY,F0,F3}; F1/F4 diagnostic-only (structural-null preserved).
6. 12 cells / 240 rows / 60 pairing-groups; lineage n=5 per cell (screening/exploratory; NO asymptotic-p).
7. Current ARQ substrate has no RTO/TLP tail recovery; K2 not result-bearing in the grid.
8. execution_source_commit now full 40-hex (v1.3.2); binary/source hashes changed (recorded); scientific semantics invariant vs v1.3.1 (comparator PASS).
9. Screening only — NO product-wide FEC verdict, NO FULL-GO/NICHE-GO/RESEARCH-ONLY/K4 (FS-DUR-SON).

## Dataset + integrity
total=240 · complete=176 · censored=64 (26.7%) · invalid=0 · schema=FS-1-RAW-SCHEMA-v2 · invariance vs v1.3.1 = PASS (shared-field mismatch 0; 176/64).

## Oracle (per-cell) distribution
O0=0 O1=0 O2=12 O3=0 — see FS-1-ORACLE-VERDICT-v1.3.2.md. FEC>A0 alone is NOT FEC-specific; O2 requires FEC>A1 at matched realized wire (F0, 1212B==1212B); F3 (2424B) is BUDGET_MISMATCH → no coding-causal attribution.

## DUR-A#2 mechanism screening
See FS-1-DUR-A2-MECHANISM-SCREENING-v1.3.2.md (frozen mechanical criteria; RETAIN/ELIMINATE/SCOPE-LIMITED). No program-final verdict.

## Labels
Every cell/pairwise row is a MEASURED FACT (per-cell CSV, pairwise CSV). Oracle classes are FROZEN-RULE CLASSIFICATIONS. Tail/mechanism notes are LIMITATIONS/EXPLORATORY. Next: FS-DUR-SON (separately governed).
