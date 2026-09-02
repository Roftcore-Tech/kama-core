# FS-COVERAGE-PLAN-v1.2 (S5-B; frozen pre-outcome)

> **12 base-cell × 4 result-bearing-arm × 5 rep = 240.** BULK-ONLY. ExactDeficit(9) + NaturalIid(3). Result-bearing
> {A0,A1-EARLY,F0,F3}; diagnostic-only {F1,F4}. SUPERSEDES v1.1 (18×6×5=540) + candidate (12×6×5=360) via ALT-1 + OPT-A.

## 0. Provenance + supersession
```text
branch/HEAD : d1-sprint5c / 4b276f6 (S5-A) · v1.2-prereg immutable da4af3c4…034
supersession : coverage v1.0(630) → v1.1(540) → **v1.2(240)**. v1.0/v1.1 KORUNUR (silinmez); v1.2 = ALT-1(IID-deficit
  çök: 18→12) + OPT-A(cross-arm-collapse: 6→4 arm; 360→240). Karar-belgeleri: FS-1-IID-COVERAGE-AMENDMENT-DECISION.md +
  FS-1-ARM-SET-SCOPE-DECISION.md.
```

## 1. Workload (bulk-only)
```text
8 × 300 B = 2400 app-byte · 48 E64 source-symbol (sealed 101B; base-source-wire 4848B) · MULTI-BLOCK ·
current sender-wide DATA-before-parity ordering. (single-block = FS-2; interleaved-parity = DR-FEC-INT-1.)
```

## 2. Base cells (12)
```text
ExactDeficit : RTT{50,250,1000} × exact_drop_count{1,2,4} = 9 (deficit variant-local; grounded exact InitialSource erasure).
NaturalIid   : RTT{50,250,1000} × p=0.20 (loss_permille=200; deficit=N/A) = 3 (deficit-ekseni ÇÖKTÜ; S3-A).
TOTAL = 12 base-cell (12 unique base_cell_semantics_hash; FS-1-CELL-MATRIX-v1.3-CANDIDATE.json 399bfcd4…).
```

## 3. Result-bearing arms (4) + diagnostic-only (2)
```text
RESULT-BEARING : A0 (redundancy-YOK) · A1-EARLY (blind-dup r=2) · F0 (K8/6blk/12rep/1212B) · F3 (K4/12blk/24rep/2424B).
DIAGNOSTIC-ONLY (result-bearing-DIŞI; OPT-A cross-arm-collapse): F1 (bulk structural-null → F0'a collapse) ·
  F4 (direct-K derived-adaptive-policy → paired-fixed-arm'a collapse; independent-row-YOK).
```

## 4. Repetitions + total
```text
5 preregistered deterministic trace-lineage/cell (RULE-10; ExactDeficit non-overlapping ranked-window; IID distinct-seed).
TOTAL = 12 × 4 × 5 = 240 result-bearing-row · 60 pairing-group (12 cell × 5 rep; her-grup 4 arm).
```

## 5. Rate / deadline / horizon (frozen)
```text
bottleneck C=1_000_000 bps · source-offer-pacer 0.80C=800_000 bps (measured; envelope 48.48ms; bottleneck-DÜŞÜRME-YOK).
deadline (arm-indep): RTT50=199 · RTT250=599 · RTT1000=2099 ms. (305ms = historical-anchor; FS-1-deadline DEĞİL.)
observation-horizon (v1.1 §B0.3; arm-indep): RTT50=699 · RTT250=2499 · RTT1000=9249 ms. (FS-1-OBSERVATION-HORIZON-AUDIT.md)
```

## 6. OUT OF SCOPE (explicit)
```text
- single-message/single-block : ZORUNLU gelecek FS-2 stratum.
- interleaved-parity : DR-FEC-INT-1.
- K2 result-bearing campaign-performance : current-RTT-grid seçmez (FS-1'de K2-row YOK).
- systematic K×R : FS-3.
- IID loss-rate sweep (p≠0.20) : ayrı-preregistered gelecek-campaign.
- ARQ + future tail-timer : DR-ARQ-TAIL-1.
FS-1 product-wide-FEC-evaluation DEĞİL — bulk-multi-block + current-scheduling + current-ARQ-substrate karakterizasyonu.
```
