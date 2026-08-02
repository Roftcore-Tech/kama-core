# FS-ANALYSIS-SPEC-v1 (S5-B; frozen pre-outcome; outcome-blind)

> **Analysis unit = pairing-group (1 base-cell, 1 rep-lineage, 4 arm).** n=5 paired-lineage/cell. Screening/exploratory
> (asymptotic-p YOK). Validators YALNIZ synthetic/diagnostic/schema-fixture ile (campaign-data INGEST-YOK). Primary-
> hierarchy oracle-map'ten. Downstream data-mining leakage-firewall + grouping-by-lineage.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / 4b276f6 · authority : FS-ORACLE-AUTHORITY-MAP-v1 + v1.1 §B0.3/§14 (censoring/latency) +
  v1.2 §8 (iki-cost) + §A1.2 (C-P4 fairness). validators: fs_analysis.rs + fs_s5b_pins (synthetic-only).
```

## 1. Analysis unit + primary comparisons (§14)
```text
scientific-unit : bir pairing-group (1 base-cell × 1 rep-lineage × 4 arm). NOT-unit: packet/FEC-block/repair/ACK/fragment.
primary paired-comparison : A1−A0 · F0−A0 · F3−A0 · F0−A1 · F3−A1 · F3−F0.
pairing-eşitlik-gereği : base-cell-semantics · trace-lineage · source-schedule · reverse-schedule · deadline · horizon ·
  binary · manifest · freeze-lock. bir-arm unresolved/INVALID → pair-KORU (unpaired-satırla DEĞİŞTİRME); availability AÇIK-raporla.
```

## 2. Outcome domains (§15; primary-hierarchy oracle-map'ten)
```text
RELIABILITY : full-workload-completion · VALID_CENSORED-status · delivered-bytes/fragment@horizon
DEADLINE    : deadline-hit · deadline-miss · censored-incomplete
LATENCY     : full-completion-time · repair-usable-time · ARQ-recovery-time (censored latency-değeri-DEĞİL; v1.1 §14)
COST        : intervention-bytes · ARQ-bytes · total-outcome-bytes · spurious-retransmissions
MECHANISM   : completion-cause · FEC-recovered-source-count · duplicate-hit/miss · tail-inclusion · reverse-path-state
```

## 3. Screening statistics (§16; n=5 paired/cell)
```text
her comparison: 5 paired-delta HEPSİ · valid/censored-pattern · median-delta · mean-delta(anlamlıysa) · min/max ·
  intervention-cost-delta · total-cost-delta. packet/block-count sample-size-DEĞİL. asymptotic-p (n=5) KULLANMA.
bootstrap/permutation/interval KULLANILIRSA: yöntem ŞİMDİ-frozen + pairing-lineage-düzeyinde. post-outcome-threshold-YOK.
FS-1 = screening/exploratory (binding-authority confirmatory-rule tanımlamadıkça).
```

## 4. Fairness / Pareto / causal (§17; C-P4/C-P5 authority)
```text
REDUNDANCY-value : A1 VEYA FEC A0'ı geçer (broad).  FEC-SPECIFIC-value : FEC A1'i fair/non-dominated realized-wire'da geçer.
F0 vs A1 : configured 12×101=1212B nominal-matched (realized-ledger'a-tabi) → coding-causal-uygun.
F3 : 24×101=2424B → F3-iyileşmesi OTOMATİK same-budget-coding-gain/pure-coding-gain/FEC-specific-dominance DEĞİL.
frozen classes : STRICTLY_DOMINATED · NON_DOMINATED · BUDGET_MISMATCH · NO_CODING_CAUSAL_CLAIM (C-P4/C-P5 exact-authority).
gerçek-transmitted-byte'ları normalize-etme.
```

## 5. Tail + NaturalIid analysis (§18/§19)
```text
TAIL : ExactDeficit tail-stratum frozen-global-drop-ID'lerden ÖN-İLAN (hangi-satır-stall'dan ÇIKARIM-YOK). tail-vs-nontail
  AYRI-raporla. substrate-limiti: ARQ tail-erasure kurtaramayabilir (RTO/TLP-YOK); FEC-completion o-sınıfı kapsayabilir AMA
  otomatik product-wide/future-tail-ARQ üstünlüğü / A1-cost-karşılaştırmasız FEC-specific-value KANITLAMAZ.
NATURALIID : configured p=0.20; realized-loss-count = GÖZLEM-değişkeni (configured-deficit DEĞİL). primary = RTT-cell ×
  lineage × arm. exploratory realized-strata (InitialSource/tail/repair/dup/retransmit-loss) primary'yi DEĞİŞTİRMEZ /
  elverişsiz-lineage-DÜŞÜRME gerekçesi-DEĞİL.
```

## 6. Downstream data-mining firewall (§20)
```text
FS-DM-1/FS-5B/fuzzy-calibration DOWNSTREAM (S5-DIŞI). S5 yalnız: allowed-schema · pre-decision-feature-whitelist ·
  leakage-prohibition · grouping-rule · model-eval-skeleton frozen.
- random-row-split YOK → group-by trace-lineage/scenario-family.
- realized-loss/completion/deadline decision-feature'a SIZDIRMA.
- F4 diagnostic-policy-outcome bağımsız-training-data DEĞİL (RULE-17).
- S5'te model-TRAIN-YOK; CART/GBT/GAM-output FS-1-primary-endpoint DEĞİL; authority-siz threshold-freeze-YOK.
```

## 7. Validity firewall (§21) + synthetic tests (§22)
```text
quarantine : freeze-lock/manifest/prereg/binary/exec-hash/source-hash/trace-lineage/schedule-mismatch · schema-fail ·
  duplicate-output · overwritten-attempt · impossible-impairment. KORU (INVALID/censored/superseded/retry append-only; silme-YOK).
synthetic-tests (fs_s5b_pins; campaign-data-YOK): complete-4-arm-group · censored-A0-tail · invalid-provenance ·
  missing-paired-arm · duplicate-output · retry-lineage · F0/A1-equal-budget · F3-budget-mismatch · NaturalIid-realized-strata ·
  F4-derived-policy-non-independence · F1-diagnostic-only-rejection.
```
