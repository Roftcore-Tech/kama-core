# FS-BULK-STRUCTURAL-NULL-v1 (S5-B; frozen pre-outcome)

> **F1 → F0 collapse (diagnostic-only).** **F3 result-bearing** (distinct K4/cost) AMA physical-first-parity-wire'ı
> F0'a-göre-ADVANCE-ETMEZ (bulk sender-wide DATA-first). Gelecek-campaign çelişirse → STRUCTURAL ANALYSIS FALSIFIED → DUR-A.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / 4b276f6 · authority : S0-B (FS-MULTIBLOCK-SCHEDULING-AUTHORITY.md) + S2 structural-null pins
  (fs_s2_pins f1/f3_bulk_parity_wire_null) + S3-R1 regression (f1/f3_bulk_null_regression_s3) + S4-A collapse-audit.
```

## 1. F1 (bulk structural-null → diagnostic-only)
```text
Canonical bulk-workload + sender-wide DATA-before-parity ordering altında: F1'in physical-parity davranışı F0'a
COLLAPSE eder (early-logical-close parity-wire'ı ADVANCE-ETMEZ; S0-B). S4-A empirik: F1 ≡ F0 byte-özdeş outcome+ftrace.
⇒ F1 DIAGNOSTIC-ONLY; result-bearing-row-YOK (OPT-A). structural-null-değeri fs_s2/fs_s3r1 pinlerinde kanıtlı.
```

## 2. F3 (result-bearing; distinct AMA parity-timing-null)
```text
F3 result-bearing KALIR çünkü davranışı DISTINCT: K=4 (K8-değil) · 12 source-block (6-değil) · 24 repair (12-değil;
R=2) · 2424B intervention-cost (1212B-değil). AMA: F3 frozen-bulk-ordering altında F0'a-göre physical-first-parity-
wire-time'ı ADVANCE-ETMESİ BEKLENMEZ (parity yine tüm-source-SONRASI). F3-farkı = K/blok/repair/cost, emisyon-head-start-DEĞİL.
```

## 3. Falsification kuralı (DUR-A)
```text
Gelecek-campaign-enstrümantasyonu bunu çürütürse (F3 physical-first-parity-wire F0'ı GEÇERSE):
  tüm-artifact KORU · STRUCTURAL ANALYSIS FALSIFIED sınıfla · DUR-A mechanism-audit tetikle.
F3-outcome-farkını "emission head-start" olarak AÇIKLAMA — structural-prediction gerçekten-çürütülüp audit-edilmedikçe.
```
