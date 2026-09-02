# FS-1-ARM-SET-SCOPE-DECISION (S4-A → OPT-A)

> **OPT-A RATIFIED (Abkarada, human-binding).** S4-A cross-arm-collapse DUR-B çözümü: FS-1 result-bearing
> arm-seti **{A0, A1-EARLY, F0, F3}** = 4 arm → **240 result-bearing-run** (12-cell × 4-arm × 5-rep). **F1 ve F4
> DIAGNOSTIC mechanism-validation'a taşınır** (FS-1-DIŞI; §11 "mechanism-validation fixtures may remain diagnostic
> outside FS-1"). 360 (6-arm) SUPERSEDED — 120-row (F1:60 + F4:60) behaviorally-collapsed duplicate idi. Bu belge
> PRE-OUTCOME; v1.2 immutable. S3-R0 ALT-1 kayıtları (12 base-cell) DEĞİŞMEZ (arm-set AYRI eksen).

## 1. Provenance
```text
branch/HEAD : d1-sprint5c / 2e1f139 (S4-A collapse-audit) · v1.2 immutable da4af3c4…034 · FS-1 0/360→0/240
karar-girişi : S4 cross-arm-collapse DUR-B → AskUserQuestion scope-seçimi → OPT-A (Abkarada human-decision, BINDING).
kanıt : FS-1-CROSS-ARM-BEHAVIORAL-COLLAPSE-AUDIT.md + FS-1-RESOLVED-ARM-SEMANTICS.md (2e1f139).
```

## 2. Karar
```text
FS-1 RESULT-BEARING ARM-SET = { A0, A1-EARLY, F0, F3 }  (4 distinct resolved-behavior)
FS-1 DIAGNOSTIC-ONLY (result-bearing-DIŞI) = { F1, F4 }
count : 12 base-cell × 4 arm × 5 rep = 240 result-bearing-run (SUPERSEDES 360).
```

## 3. Gerekçe (collapse-kanıtı)
```text
- F1 ≡ F0 (her hücre; K8 both; early-logical-close bulk'ta parity-wire'ı advance-ETMEZ = ACCEPTED S0-B/S2
  structural-null → outcome + forward_trace_hash byte-özdeş). F1'in structural-null-değeri DIAGNOSTIC-pinlerde
  (fs_s2_pins f1_bulk_parity_wire_null; fs_s3r1_pins f1_bulk_null_regression_s3) ZATEN kanıtlı.
- F4 ≡ F3 (RTT50 K4) / ≡ F0 (RTT250,1000 K8; adaptive mevcut-K'ye çözülür; FS-1'de K2 hiç tetiklenmez). F4'ün
  direct-K/no-leakage-değeri DIAGNOSTIC-pinlerde (fs_s2_pins f4_*; fs_s3r1_pins f4_predecision_firewall_s3) kanıtlı.
- DISTINCT result-bearing kalan: A0 (redundancy-YOK) · A1 (duplicate) · F0 (K8; interv 1212B) · F3 (K4; interv 2424B).
```

## 4. Etki (S3-kayıtlarına)
```text
- 12 base-cell matrisi (S3-R0 ALT-1; FS-1-CELL-MATRIX-v1.3-CANDIDATE.json) DEĞİŞMEZ — arm-set base-cell'den AYRI eksen.
- S3-R0 pinleri (iid_coverage_alt1_ratified 12×6×5=360; historical 180) S3-KAYDI olarak KORUNUR (6-arm candidate-pool
  bağlamında doğruydu). fs_cell.FS1_ARMS (6) = S3 candidate-pool kaydı; YENİ FS1_RESULT_BEARING_ARMS (4) = OPT-A.
- S3 checkpoint'teki "360" projected-count OPT-A ile SUPERSEDED (240). v1.3-freeze (S5) 240'ı resmileştirir.
```

## 5. Manifest yapısı (S4-B; OPT-A)
```text
ExactDeficit : 9 base-cell × 4 arm × 5 rep = 180 row
NaturalIid   : 3 base-cell × 4 arm × 5 rep =  60 row
TOPLAM       = 240 row · pairing-group = 12 × 5 = 60 (her grup 4 arm) · per-arm = 60/60 (A0/A1/F0/F3).
```

## 6. RULE-11 (resolved-arm behavior uniqueness) uygulaması
```text
OPT-A = RULE-11'in doğrudan uygulaması: farklı-arm-label AYNI resolved-behavior üretirse ayrı result-bearing-execution
OLARAK sayılmaz. F1/F4 (collapsed) result-bearing-DIŞI; mechanism-kimlikleri DIAGNOSTIC'te korunur.
```
