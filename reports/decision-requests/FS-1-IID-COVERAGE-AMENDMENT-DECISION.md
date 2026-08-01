# FS-1-IID-COVERAGE-AMENDMENT-DECISION (S3-R0)

> **ALT-1 RATIFIED (Abkarada, human-binding).** S3-A DUR-B (IID-COVERAGE-AXIS AMBIGUITY) çözümü: IID20 altında
> deficit-ekseni ÇÖKER. Amended FS-1 base-matris = **12 hücre** (9 ExactDeficit + 3 NaturalIid), projected
> **360 result-bearing-run** (12 × 6 arm × 5 rep). Eski 540, 180 execution-semantic-duplicate satır içeriyordu;
> **540 sırf tarihsel-sayısal-süreklilik için KORUNMAZ.** Bu belge PRE-OUTCOME (sonuç-öncesi); v1.2 DEĞİŞMEZ.

## 1. Provenance
```text
branch/HEAD : d1-sprint5c / (bu commit) · foundation d4c08c7 · S0 0ba3612 · S1 5d84f5a · S2 e511ef4 · S3-A 6e8f78f
karar-girişi: DR-FEC-FS-1 S3 RESUMPTION directive §0 (Abkarada human-decision, BINDING) — ALT-1.
```

## 2. FS-PREREG-v1.2 immutable (DEĞİŞMEDİ)
```text
sha256 : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
Bu belge v1.2'yi EDİT-ETMEZ. Amendment v1.3-freeze'de (S4-sonrası, ayrı-tur) resmileşir; S3'te freeze YOK.
```

## 3. Zero result-bearing runs
```text
FS-1 result-bearing : 0/540 (tarihsel planned) · 0/360 (amended candidate). Hiçbir sonuç-verisi mevcut-DEĞİL.
Bu belge + S3-R0 kodu DIAGNOSTIC (config/coverage-katmanı; manifest-DIŞI; 360-budget-harcamaz).
```

## 4. S3-A DUR-B verdict (kabul-edildi)
```text
FS-IID-CELL-SEMANTICS-AUDIT.md (6e8f78f): "deficit∈{1,2,4} IID20 altında executable deficit-anlamı GROUND-EDİLEMEZ."
11-agent adversarial audit; verdict DUR-B (IID-COVERAGE-AXIS AMBIGUITY). ALT-1 = bu DUR-B'nin Abkarada-çözümü.
```

## 5. NaturalIid deficit-inert kod-otoritesi
```text
fs_trace.rs:77       NaturalIid { loss_permille: u32, seed: u64 }   → deficit-field YOK
fs_trace.rs:138-145  hash_forward_spec(NaturalIid) → yalnız (loss_permille, seed)'e commit
fs_trace.rs:253-259  decide()(NaturalIid) = keyed(id,seed) → d GİRMEZ
fs_harness.rs:592-595 R arm-enum'dan (deficit'ten DEĞİL); fs_harness.rs:85-87 A1-target deficit-BAĞIMSIZ
⇒ channel/trace/seed/deadline/K katmanlarının HEPSİ IID20 altında d'yi YOK-SAYAR (çekişmesiz).
```

## 6. r=d REDDİ
```text
Tek aday-grounding (coverage-v1.0 §1 "A1-EARLY(r=d)") REDDEDİLDİ: (i) bağlayıcı v1.2 A1-EARLY'yi configured_r +
"deficit-BAĞIMSIZ" tanımlar, r=d demez (v1.2 > coverage-v1.0); (ii) kod enforce-etmez (pin r=2 hardcode);
(iii) benimsense 1/6-arm değişir (≥5/6 duplicate); (iv) d→R alan-relabel gerektirir + FS-3 K×R'yi çoğaltır.
KARAR: FS-1'de r=d BAĞLANMAZ; hiçbir arm için yeni r=d davranışı YOK (§9). deficit ARTIK R-alias DEĞİL.
```

## 7. matched-deficit IID REDDİ
```text
"IID-loss matched-deficit" (v1.0 C-P1) → v1.1 §5 "KALDIRILDI". FS-1 IID p=0.20 SABİT; d'ye matched-DEĞİL.
retry-until-d / forced-post-IID-loss / deficit-derived-seed / deficit-dependent-deadline HEPSİ REDDEDİLDİ (§1).
```

## 8. ALT-1 human-approval
```text
Abkarada (S3 RESUMPTION §0, BINDING): "The S3-A DUR-B is resolved through ALT-1 — COLLAPSE IID20 DEFICIT AXIS."
"Do NOT preserve 540 merely for historical numerical continuity."
```

## 9. Eski 18-hücre matris
```text
RTT{50,250,1000}(3) × deficit{1,2,4}(3) × impairment{ExactDeficit,IID20}(2) = 18 base-cell
  → 9 ExactDeficit + 9 IID20. IID20-9'unun 6'sı (RTT-başına 2) execution-semantic-duplicate (d IID-inert).
```

## 10. Amended 12-hücre matris
```text
EXACT-DEFICIT AİLESİ : RTT{50,250,1000} × d{1,2,4}          = 9 hücre (d grounded: exact InitialSource erasure)
NATURAL-IID   AİLESİ : RTT{50,250,1000} × p=0.20 × (d=N/A)   = 3 hücre (deficit-ekseni ÇÖKTÜ)
TOPLAM                                                        = 12 benzersiz base-cell
Pin AMENDED_BASE_CELL_UNIQUENESS: 12 hücre → 12 benzersiz base_cell_semantics_hash · duplicate = 0. ✓
```

## 11. Eski count 540
```text
540 = 18 × 6-arm{A0,A1-EARLY,F0,F1,F3,F4} × 5-rep  (FS-COVERAGE-PLAN-v1.1 L12; v1.2 §D "6 × 18 × 5").
```

## 12. Amended count 360
```text
360 = 12 × 6-arm × 5-rep. Pin iid_coverage_alt1_ratified: 12 × 6 × 5 == 360. ✓
```

## 13. Removed duplicate rows = 180
```text
18 tarihsel → 12 benzersiz = 6 duplicate base-satır (RTT-başına 2 IID × 3 RTT).
6 duplicate-cell × 6-arm × 5-rep = 180 avoided result-bearing-run.
Pin HISTORICAL_IID_DUPLICATE_DETECTION: (18−12) × 6 × 5 == 180 · RTT-başına 3-IID-d-satırı → 1 benzersiz base-hash;
  3-ExactDeficit-d-satırı → 3 benzersiz base-hash. ✓
```

## 14. Bulk-only scope boundary
```text
FS-1 BULK-ONLY DEĞİŞMEZ: app_unit_bytes=300 · app_unit_count=8 · total=2400 · source_symbol_count=48 · E=64 · MULTI-BLOCK.
FS-1'e single-block stratum EKLENMEZ (FS-1-BULK-SCOPE-BOUNDARY.md).
```

## 15. Future single-block scope
```text
FS-2 : en-az bir AYRI-preregistered single-message/single-block workload stratum (zorunlu, gelecek).
DR-FEC-INT-1 : sender-ordering / interleaved-parity redesign (ayrı-tur).
```

## 16. Future IID loss-rate scope
```text
IID loss-rate SWEEP (p≠0.20) FS-1'de YOK. Ayrı FS-2 veya FS-IID-RATE coverage-kararına ait.
FS-1 NaturalIid TEK sabit p=0.20 (loss_permille=200).
```

## 17. v1.2 immutable status
```text
FS-PREREG-v1.2 (da4af3c4…034) FROZEN/immutable. Bu amendment onu değiştirmez; v1.3'te resmileşecek.
```

## 18. v1.3 amendment requirement
```text
Amended 12-hücre/360-run + typed-impairment + RULE-8/9 → FS-PREREG-v1.3-freeze'de resmileşir.
v1.3-freeze S3'te YAPILMAZ (S4-sonrası; Abkarada-onayı + re-freeze). S3-R3 yalnız CANDIDATE artifact üretir.
```

## 19. Semantic-cell uniqueness governance
```text
RULE-8 SEMANTIC CELL UNIQUENESS: aynı executable-config/intervention-authority/evaluation-run üreten iki satır
  = ayrı bilimsel-hücre DEĞİL; label/filename/row-id/metadata-alias/seed-alias/analysis-label FARKI distinctness KURMAZ.
RULE-9 VARIANT-LOCAL PARAMETERS: bir impairment-ailesine ait param, başka aile için ambiguous top-level field OLARAK
  var-olamaz; geçersiz-kombinasyon typed-config ile temsil-EDİLEMEZ.
Makine: base_cell_semantics_hash (A) / run_execution_semantics_hash (B) / row_identity_hash (C) — üç-katman ayrım.
  crates/kama-integration/tests/common/fs_cell.rs (FsImpairment sum-type + 3-hash + amended/historical matris).
```

## 20. Pre-outcome doğrulaması
```text
Bu karar SONUÇ-ÖNCESİ: 0/360 result-bearing-run; hiçbir FS-1 outcome-verisi ÜRETİLMEDİ; hiçbir arm-kazananı gözlenmedi.
ALT-1 = bilimsel-distinctness kararı (S3-A kod+otorite kanıtına dayalı), outcome-güdümlü DEĞİL. sender.rs/loss.rs
zero-diff; production graph/defaults DEĞİŞMEDİ. v1.2 immutable. S3-R1..R3 gate-green artışlarla devam eder.
```
