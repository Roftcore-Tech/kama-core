# FS-ORACLE-AUTHORITY-MAP-v1 (S5-B; frozen pre-outcome)

> **No conflict → DUR-B YOK.** Binding oracle-authority = FS-PREREG-v1.1 §B0.5 (oracle-ordering) + v1.0 §8 + C-P1..C-P5
> (v1.1 §5) + v1.2 A1.2 (C-P4 fairness=realized_sealed_wire_bytes; BUDGET_MISMATCH-guard). v1.2 "v1.1 §B0.5 AYNEN".
> REDUNDANCY-value ≠ FEC-specific-value; FEC-specific A1-karşılaştırması + realized-wire-fairness GEREKTİRİR.

## 0. Provenance + conflict check
```text
branch/HEAD : d1-sprint5c / 4b276f6 · sources : FS-PREREG.md §8 · FS-PREREG-v1.1 §B0.5 + §5(C-P1..C-P5) ·
  FS-PREREG-v1.2 §D + §A1.2 + §8(iki-cost-kavramı). CONFLICT-CHECK: v1.2 v1.1'i AYNEN-korur; amendment'ler
  (C-P1 matched-deficit-kaldırma) SUPERSESSION (conflict-DEĞİL). primary-endpoint/threshold/Pareto/censoring/oracle-
  precedence çelişkisi YOK → ORACLE-AUTHORITY-GAP DUR-B TETİKLENMEZ.
```

## 1. Oracle ordering (v1.1 §B0.5; measured-arm-only)
```text
1. reliability (full-workload-completion / P(complete≤horizon))
2. deadline-hit
3. latency (completion-time; censored latency-değeri-DEĞİL)
4. total-outcome-cost (Pareto)
5. CPU/RSS constraint/tiebreak
Oracle YALNIZ measured-arm seçer · missing-arm → ORACLE-ABSTAIN · interpolation YASAK.
```

## 2. Oracle outcome classes (O0..O3)
```text
O0 NO_REDUNDANCY_VALUE   : ne A1 ne FEC A0'ı geçmez (proactive-redundancy değer-katmıyor).
O1 REDUNDANCY_VALUE      : A1 VEYA FEC A0'ı geçer (broad proactive-redundancy değeri; FEC-specific-DEĞİL).
O2 FEC_SPECIFIC_VALUE    : FEC-arm A1'i fair/non-dominated realized-wire-cost altında geçer (coding-causal).
O3 INSUFFICIENT/ABSTAIN  : yetersiz-kanıt / unresolved / missing-pair → O3 (favorable-sınıfa ZORLANMAZ).
```

## 3. C-P1..C-P5 binding verdicts (confirmatory; v1.1 §5)
```text
C-P1 RTT250 replication : exact-deterministic-deficit paired-replay (IID-ifadesi KALKTI; v1.1 §69). arms A0,A1-EARLY(r=d),F0(r=d).
C-P2 high-RTT emission  : arms A0,A1-EARLY,F0,F1,F3 (5). üç-claim AYRI raporlanır.
C-P3 reverse-blackout   : REDUNDANCY_NICHE=max(A1,F*)-beats-A0 · FEC_SPECIFIC_NICHE=F*-beats-A1 (iki-verdict AYRI).
C-P4 coding-vs-dup      : configured-redundancy-class-match ∧ realized_sealed_wire_bytes-parity ∧ BUDGET_MISMATCH-guard.
C-P5 deadline-critical  : historical-305-corrigendum-üç-etiket ÖNCE; sonra REDUNDANCY_ESSENTIAL=A1|FEC-tutar.
(C-P1..C-P5 = confirmatory-campaign; FS-1-screening AYRI. FS-1 oracle-ordering §1 + fairness §4-authority kullanır.)
```

## 4. Per-oracle audit fields (§12)
```text
her endpoint/oracle için: source-doc/version · input-fields · comparison-arms · deadline/censor-handling ·
  cost-fairness-rule · missing/invalid-pair-behavior (→ ABSTAIN/O3) · exact-output-classification.
```

## 5. Preserved principles (§12 minimum)
```text
- A1 VEYA FEC > A0 = proactive-redundancy value (O1).
- A1-success ≠ FEC-specific-value.
- FEC > A0 tek-başına ≠ FEC-specific-value.
- FEC-specific-value A1-karşılaştırması GEREKTİRİR (O2).
- coding-causal-claim'i realized intervention-wire-fairness YÖNETİR (C-P4; realized_sealed_wire_bytes).
- budget-mismatch → coding-causal-attribution YASAK (BUDGET_MISMATCH; F3 2424B vs A1/F0 1212B).
- yetersiz-kanıt → O3/insufficient (favorable-sınıfa ZORLANMAZ).
```

## 6. FS-1 arm-set uygulaması (OPT-A {A0,A1,F0,F3})
```text
primary paired: A1−A0 · F0−A0 · F3−A0 · F0−A1 · F3−A1 · F3−F0. (F1/F4 diagnostic; oracle-evidence-DEĞİL — RULE-17.)
F0 vs A1 : configured 1212B=1212B (matched-r=2 → matched-wire; C-P4 gerçek-adil) → coding-causal-uygun.
F3 vs A1 : 2424B vs 1212B → BUDGET_MISMATCH → coding-causal-YASAK (F3-üstünlüğü otomatik-coding-gain DEĞİL).
```
