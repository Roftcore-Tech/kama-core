# FS-PREREG-v1.3.1 (S5-E2 / OPT-R1; pre-outcome consolidated preregistration — EXECUTOR AMENDMENT)

> **BINDING CAMPAIGN: 12 base-cell × 4 result-bearing-arm × 5 rep = 240 row.** Pre-outcome; **0 result-bearing-run
> before v1.3.1 freeze (ve v1.3-öncesi de 0)**. SUPERSEDES-BY-chain: v1.0 (KORUNDU) → v1.1 (KORUNDU) →
> v1.2 (immutable da4af3c4…034; KORUNDU) → v1.3 (KORUNDU; **INVALIDATED-BEFORE-FIRST-RESULT / EXECUTOR GAP**) →
> **v1.3.1**. Bu belge kendi-SHA'sını İÇERMEZ (freeze-lock referanslar; acyclic §3).

## 0. Amendment gerekçesi (v1.3 → v1.3.1)
```text
v1.3 (freeze-lock 5a9bc09d…) INVALIDATED-BEFORE-FIRST-RESULT · reason=EXECUTOR GAP · result_bearing_runs=0.
  frozen fs1_runner (binary 75ca6eb7) result-bearing-execution-contract'ı yerine getiremiyordu (pozitif-yol
  kalıcı-refuse-stub; run+persist / append-only-raw-writer / ledger / checksum YOK). S6 (08a35c2) LAUNCH-NOT-AUTHORIZED.
sınıf : pre-outcome RUNTIME/PROVENANCE amendment — scientific-design amendment DEĞİL.
  FROZEN SCIENCE DEĞİŞMEDİ (kanıt: FS-1-SCIENCE-INVARIANCE-v1.3.1.md; scientific-behavior-changes=0).
eski-lock KORUNUR (overwrite/silme YOK) — FS-1-FREEZE-LOCK-v1.3-INVALIDATION.md.
```

## 1. Pre-outcome chain (§28; 33-madde)
```text
 1. v1.2 manifest/executability-gap (FS-1-MANIFEST-INCONSISTENCY.md)
 2. HARNESS-A closure (FS-HARNESS-A-CHECKPOINT.md)
 3. HARNESS-B closure (FS-HARNESS-B-CHECKPOINT.md)
 4. payload layer-mix discovery (FS-1-PAYLOAD-LAYER-MAPPING-NOTE.md)
 5. canonical 8×300B → 48×E64 mapping (fs_workload.rs; RATIFIED)
 6. harness-only fragmentation status (production-fragmenter YOK)
 7. global trace identity (global_source_fragment_id 0..47; RULE-4)
 8. F4 literal-K unrealizability (S0-A)
 9. direct CodecK correction (K2/K4/K8; OPTION-D)
10. sender-wide multi-block DATA-first authority (S0-B; FS-MULTIBLOCK-SCHEDULING-AUTHORITY.md)
11. F1 bulk structural-null (S2; FS-BULK-STRUCTURAL-NULL-v1.md)
12. F3 bulk parity-timing-null (S2; result-bearing-KALIR distinct K4/cost)
13. 0.80C measured source-offer grounding (S3-R1; envelope 48.48ms)
14. IID deficit-axis degeneration (S3-A DUR-B; FS-IID-CELL-SEMANTICS-AUDIT.md)
15. 18→12 base-cell correction (ALT-1; FS-1-IID-COVERAGE-AMENDMENT-DECISION.md)
16. 540→360 duplicate-row removal (ALT-1)
17. cross-arm behavior collapse (S4-A; FS-1-CROSS-ARM-BEHAVIORAL-COLLAPSE-AUDIT.md)
18. F1/F4 diagnostic-only decision (OPT-A; FS-1-ARM-SET-SCOPE-DECISION.md)
19. six→four result-bearing arms (OPT-A)
20. 360→240 duplicate-row removal (OPT-A)
21. repetition-lineage authority (RULE-10; FS-1-REPETITION-SEMANTICS-AUDIT.md)
22. final deadlines (RTT50=199 · RTT250=599 · RTT1000=2099)
23. final observation horizons (RTT50=699 · RTT250=2499 · RTT1000=9249; v1.1 §B0.3)
24. validity/censoring policy (INVALID/VALID_COMPLETE/VALID_CENSORED; FS-1-OBSERVATION-HORIZON-AUDIT.md)
25. retry/invalidity policy (only-INVALID; max-3; append-only; FS-1-RETRY-AND-INVALIDITY-POLICY.md)
26. execution-order authority (60-group deterministic; FS-1-EXECUTION-ORDER-v1.json)
27. final analysis/oracle authority (FS-ANALYSIS-SPEC-v1.md + FS-ORACLE-AUTHORITY-MAP-v1.md)
28. candidate manifest provenance hash (e9e331a4…; DEĞİŞMEDİ)
29. v1.3-lock EXECUTOR-GAP invalidation (OPT-R1; FS-1-FREEZE-LOCK-v1.3-INVALIDATION.md)      ← YENİ
30. result-bearing EXECUTOR + persistence tamamlandı (fs_exec/fs_campaign; S5-C2)             ← YENİ
31. reproducible executor-binary FROZEN (8438991b…; 4-build byte-özdeş; FS-1-EXECUTION-BASELINE-v1.3.1) ← YENİ
32. science-invariance kanıtı (scientific-behavior-changes=0; FS-1-SCIENCE-INVARIANCE-v1.3.1) ← YENİ
33. final manifest v1.3.1 (12fc0a75…) + zero result-bearing runs before v1.3.1 freeze (0/240)
```

## 2. Binding campaign
```text
12 base-cell (9 ExactDeficit + 3 NaturalIid) · 4 result-bearing-arm {A0,A1-EARLY,F0,F3} · 5 rep · 240 row · 60 pairing-group.
diagnostic-only (result-bearing-DIŞI): F1 (structural-null→F0), F4 (derived-K→paired-fixed-arm). RULE-17.
(v1.3 ile ÖZDEŞ — FS-1-SCIENCE-INVARIANCE-v1.3.1.md §4.)
```

## 3. Frozen authority hashes (final-manifest-v1.3.1 + authority; acyclic — prereg kendi-SHA'sını içermez)
```text
final_manifest_v1.3.1 (BINDING)  12fc0a75da87e49bc078ed230e071c8d8751c981fe7ab9ae8226b591d35ddbcc   ← YENİ
candidate_manifest               e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10   (DEĞİŞMEDİ)
candidate_matrix                 399bfcd4b5c75a25c1c036ed56719e0afe725a15f0df2c3d0d53116b67313b11
coverage_v1.2                    3c80c6856a9d1383a4e1f1d2c761ff50578e325e08a1cbdf9860e25181e2df16
analysis_spec_v1                 6e2056ba23c9323308bba57ce8a748270af05270da48373e0c93fb52c274f567
oracle_map_v1                    0bee78dbed3747aae84704437430446e339bfb9275be80c7110e54151f7c6fca
structural_null_v1               c37bfcbe6bfc5c1d8aeb31281f72d3d2f1a628ab213830166440dade47aeeefa
f4_derived_policy_v1             08e304b2660c805572ff2aae1ba59c1105ab9eb1c0507796469f35e20cd4ff3f
observation_horizon_audit        92c9dbf76a7451a2f3b220dbdb7b4c0b72de80a72b6100d1339d32844458d99d
retry_policy                     035c724e4740bb2a6c930b92083047fd7c78931577c59cd02355bc443710734a
execution_order_v1               be692dea79334c7a4b78c808d0106b3d99ddff81775e311d01363bcfe5752623
raw_schema_v1                    c7ac2a85d25916111600e3a0d566774da2042bd037adc4a3b24623282217168c
raw_artifact_policy              cfd4beff7c97af8c20fa85249cb66990a57b2170f28c16d7c6db198517302bba
execution_baseline_v1.3.1        05344828761f9d383192ef3b85abd07d12d5b0981d2273d512ff46b60fac9162   ← YENİ
science_invariance_v1.3.1        8716e7eeda25f6452bb09c36eb05ea08acf9be97bc4c6452b336285d4bf41ff8   ← YENİ
iid_coverage_amendment           e30d2fa841a897a84c2e1bd4a2b849df75970d1851b5ce76bb43d0374b9b9e61
arm_set_scope_decision           850c6d767d8b91998b3075b1d27fa46ae75a1fbd7333e418a550dce9c660dd37
repetition_semantics_audit       7fa3575d84cdd7f18ef316682cea32477044f1e637b92e0e233b8b5137c99fe2
v1.3_lock_invalidated (preserved) 5a9bc09da3b0bf1e345ed1546134ffbc0ab2d3668bea6f124226b044542079d6   ← YENİ (invalidated)
execution_source_commit          a3ab2fe · execution_binary_sha256 8438991b…e514 · cargo_lock_sha256 47893d24…4
v1.2_immutable_historical        da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
```

## 4. Supersession (§28)
```text
v1.0 (630) KORUNDU · v1.1 (540) KORUNDU · v1.2 (immutable) KORUNDU · v1.3 (240) KORUNDU (INVALIDATED-BEFORE-FIRST-RESULT;
EXECUTOR GAP) · v1.3 SUPERSEDED-BY v1.3.1 (240; aynı-bilim + tamamlanmış-executor). Eski-dosyalar sessiz-EDİT-EDİLMEDİ.
```

## 5. Scope — v1.3.1 CLAIM-edebilir (§29; v1.3 ile ÖZDEŞ)
```text
bulk multi-block traffic · current sender-wide DATA-first scheduling · current ARQ substrate · A0/A1/F0/F3 ·
ExactDeficit + NaturalIid p=0.20 · frozen deadlines/horizons · proactive-redundancy + coding-cost distinctions.
```

## 6. Scope — v1.3.1 CLAIM-EDEMEZ (§29; v1.3 ile ÖZDEŞ)
```text
product-wide FEC value · small-message behavior · interleaved-parity performance · K2 campaign-performance ·
adaptive-policy optimality · controller-readiness · complete-R-sweep · future-tail-timer-sonrası ARQ-performance.
```

## 7. Pre-outcome doğrulama
```text
0 result-bearing-run (0/240; v1.3.1 ve v1.3-öncesi) · hiçbir outcome-verisi ÜRETİLMEDİ · scope/arm/cell/lineage/
deadline/horizon/retry/execution-order/oracle/analysis SONUÇ-GÖRMEDEN donduruldu · executor de SONUÇ-ÜRETMEDEN
kuruldu+test-edildi (sentetik-only). sender/loss zero-diff; v1.2 immutable. freeze-lock S5-F2.
```
