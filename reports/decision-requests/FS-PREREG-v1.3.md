# FS-PREREG-v1.3 (S5-E; pre-outcome consolidated preregistration)

> **BINDING CAMPAIGN: 12 base-cell × 4 result-bearing-arm × 5 rep = 240 row.** Pre-outcome; 0 result-bearing-run
> before freeze. SUPERSEDES-BY-chain: v1.0 (KORUNDU) → v1.1 (KORUNDU) → v1.2 (immutable da4af3c4…034; KORUNDU) →
> **v1.3**. Bu belge kendi-SHA'sını İÇERMEZ (freeze-lock referanslar; acyclic §3). final-manifest + tüm-authority SHA aşağıda.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / fc20fab (S5-D) · execution-baseline commit 5c7e005 · FS-1 result-bearing = 0/240.
v1.2 immutable : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (SUPERSEDED-BY v1.3; DEĞİŞMEDİ).
```

## 1. Pre-outcome chain (§28; 31-madde)
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
28. candidate manifest provenance hash (e9e331a4…)
29. final manifest hash (6e8014ab…)
30. execution binary provenance (75ca6eb7…; commit 5c7e005; Cargo.lock 47893d24…)
31. zero result-bearing runs before freeze (0/240)
```

## 2. Binding campaign
```text
12 base-cell (9 ExactDeficit + 3 NaturalIid) · 4 result-bearing-arm {A0,A1-EARLY,F0,F3} · 5 rep · 240 row · 60 pairing-group.
diagnostic-only (result-bearing-DIŞI): F1 (structural-null→F0), F4 (derived-K→paired-fixed-arm). RULE-17.
```

## 3. Frozen authority hashes (final-manifest + authority; acyclic — prereg kendi-SHA'sını içermez)
```text
final_manifest (BINDING)     6e8014ab63a2e6fda4a72759c4fee2989131daf0131205b5a71bd04354ed890b
candidate_manifest           e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10
candidate_matrix             399bfcd4b5c75a25c1c036ed56719e0afe725a15f0df2c3d0d53116b67313b11
coverage_v1.2                3c80c6856a9d1383a4e1f1d2c761ff50578e325e08a1cbdf9860e25181e2df16
analysis_spec_v1             6e2056ba23c9323308bba57ce8a748270af05270da48373e0c93fb52c274f567
oracle_map_v1                0bee78dbed3747aae84704437430446e339bfb9275be80c7110e54151f7c6fca
structural_null_v1           c37bfcbe6bfc5c1d8aeb31281f72d3d2f1a628ab213830166440dade47aeeefa
f4_derived_policy_v1         08e304b2660c805572ff2aae1ba59c1105ab9eb1c0507796469f35e20cd4ff3f
observation_horizon_audit    92c9dbf76a7451a2f3b220dbdb7b4c0b72de80a72b6100d1339d32844458d99d
retry_policy                 035c724e4740bb2a6c930b92083047fd7c78931577c59cd02355bc443710734a
execution_order_v1           be692dea79334c7a4b78c808d0106b3d99ddff81775e311d01363bcfe5752623
raw_schema_v1                c7ac2a85d25916111600e3a0d566774da2042bd037adc4a3b24623282217168c
raw_artifact_policy          cfd4beff7c97af8c20fa85249cb66990a57b2170f28c16d7c6db198517302bba
execution_baseline_v1        3459433d8f80c9d61e52daa6a19f4128230a6b3985e6c505fe7919fe46588ed0
iid_coverage_amendment       e30d2fa841a897a84c2e1bd4a2b849df75970d1851b5ce76bb43d0374b9b9e61
arm_set_scope_decision       850c6d767d8b91998b3075b1d27fa46ae75a1fbd7333e418a550dce9c660dd37
repetition_semantics_audit   7fa3575d84cdd7f18ef316682cea32477044f1e637b92e0e233b8b5137c99fe2
execution_source_commit      5c7e005 · execution_binary_sha256 75ca6eb7…0461 · cargo_lock_sha256 47893d24…4
v1.2_immutable_historical    da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
```

## 4. Supersession (§28)
```text
v1.0 (630) KORUNDU · v1.1 (540) KORUNDU · v1.2 (immutable) KORUNDU · v1.2 SUPERSEDED-BY v1.3 (240). Eski-dosyalar
sessiz-EDİT-EDİLMEDİ. v1.3 = ALT-1 (IID-deficit-çök) + OPT-A (cross-arm-collapse; F1/F4-diagnostic).
```

## 5. Scope — v1.3 CLAIM-edebilir (§29)
```text
bulk multi-block traffic · current sender-wide DATA-first scheduling · current ARQ substrate · A0/A1/F0/F3 ·
ExactDeficit + NaturalIid p=0.20 · frozen deadlines/horizons · proactive-redundancy + coding-cost distinctions.
```

## 6. Scope — v1.3 CLAIM-EDEMEZ (§29)
```text
product-wide FEC value · small-message behavior · interleaved-parity performance · K2 campaign-performance ·
adaptive-policy optimality · controller-readiness · complete-R-sweep · future-tail-timer-sonrası ARQ-performance.
```

## 7. Pre-outcome doğrulama
```text
0 result-bearing-run (0/240) · hiçbir outcome-verisi ÜRETİLMEDİ · scope/arm/cell/lineage/deadline/horizon/retry/
execution-order/oracle/analysis SONUÇ-GÖRMEDEN donduruldu. sender/loss zero-diff; v1.2 immutable. freeze-lock S5-F.
```
