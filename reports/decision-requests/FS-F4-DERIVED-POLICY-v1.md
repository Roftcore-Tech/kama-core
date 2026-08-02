# FS-F4-DERIVED-POLICY-v1 (S5-B; frozen pre-outcome)

> **F4 = derived diagnostic direct-K policy; result-bearing-independent-row YOK (RULE-17).** FS-1'de F4-outcome =
> paired-fixed-arm'ın (RTT50→F3, RTT250/1000→F0) outcome+cost'unu AYNEN devralır. Bağımsız-CI/kanıt SAYILAMAZ.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / 4b276f6 · authority : S0-A (f4_select_k OPTION-D) + S2 f4-pins + S4-A collapse-audit + OPT-A.
```

## 1. Frozen diagnostic direct-K rule (S0-A)
```text
base_rtt < 50ms → K2 · 50ms ≤ base_rtt < 150ms → K4 · base_rtt ≥ 150ms → K8 · cold/default → K4.
(mechanistic/exploratory; optimal-iddia-YOK. pre-decision base-RTT TEK-girdi; deficit/reverse/completion/outcome GİRMEZ —
F4_PREDECISION_FIREWALL_S3 pinli.)
```

## 2. FS-1 derived mapping (paired-fixed-arm)
```text
RTT=50   → resolved K4 → paired F3-sonucunu seç (F3 ile byte-özdeş; S4-A).
RTT=250  → resolved K8 → paired F0-sonucunu seç (F0 ile byte-özdeş).
RTT=1000 → resolved K8 → paired F0-sonucunu seç.
(FS-1'de K2 hiç tetiklenmez — min-RTT 50→K4.)
```

## 3. Non-independence (RULE-17)
```text
F4: independent-result-bearing-row-YOK · independent-sampling-unit-YOK · paired-fixed-arm outcome+cost'unu AYNEN-devralır ·
independent-confidence-interval-ALAMAZ · ek-kanıt-olarak-SAYILAMAZ (derived/collapsed diagnostic-policy).
```

## 4. Yasak iddialar
```text
K2-campaign-performance · policy-optimality · unseen-RTT-interpolation · online-controller-readiness · trained-adaptive-policy.
(Gelecek teacher-model distilasyonu → ayrı-governed-work; FS-1 kapsamı-DIŞI.)
```
