# FS-PREREG-v1.1 — B0 AMENDMENT (DR-FEC-FS-1 v5 §4-§5)

**Statü:** FS-PREREG-v1.0 (sha `cf005739`) **BASE**; bu v1.1 yalnız §4-§5'te sayılan **dar, isimli
amendmanları** ekler ve outcome-görülmeden freeze edilir. v1.0'daki donmuş nicelikler (arm-set, statistics,
per-run instrument-chain, timing-chain, censoring-felsefesi, verdict-path) AYNEN geçerli; aşağıda YALNIZ
delta. Çelişkide sıra: v2-prompt > **bu v1.1** > coverage-v1.1 > v5-icra > denetim-raporları.

## B0.1 — BINARY / PROVENANCE BASELINE (§4.1)
```text
HEAD          : b1c017d (veya descendant) · branch d1-sprint5c
feature-set   : experimental-cold-start · experimental-fec-budget (FEC-actuation deneysel-yolu)
path_timing_snapshot() patch (b1c017d): READ-ONLY · EXPERIMENTAL · no-product-policy · no-wire · no-cold-start
build-ledger  : her arm-binary'sinin (cargo build --release) kimliği + byte-size FS-HARNESS'te kaydedilir;
                stale/wrong-feature/wrong-flag binary = INVALID — NO EVIDENCE (§9-4).
```

## B0.2 — DEADLINE FORMULARI (outcome-bağımsız; §4.2)
```text
T_serialize(cell) = ceil(block_bytes / bw_config_Bps × 1000) ms        ; block_bytes = K×payload + planned-parity-bytes
T_deadline(cell)  = T_serialize + N_recovery × RTT_config + M_fixed     ; N_recovery = 2 (birinci-uçuş + BİR bounded ARQ-recovery-turu), M_fixed = 50 ms

C-P2 (K=8, payload=300B, r=d parity, bw_config=1 Mbps → T_serialize≈ (2400+r·300)/125000·1000 ≈ 20-30ms):
  RTT=1000 → T_deadline = ~25 + 2000 + 50 = 2075 ms
  RTT=2500 → T_deadline = ~25 + 5000 + 50 = 5075 ms
Gerekçe: "birinci-uçuş + tam-bir ARQ-recovery-turu + sabit-marj" — ARQ'ya ADİL (bir retransmit-turu tanınır);
fiziğe-dayalı, outcome-dağılımından SEÇİLMEDİ. Sayı freeze; sonuç görülünce DEĞİŞTİRİLMEZ.

C-P5 (§4.2): d1 historical 305ms KORUNUR ("historical replication" etiketiyle). d2/d4 için:
  T_deadline(d) = T_serialize + N_recovery × RTT250 + M_fixed  ; RTT250, N_recovery=2, M_fixed=50
  → d2/d4 deadline = ~20 + 500 + 50 = 570 ms (aynı-formül; per-deficit block_bytes/parity farkı T_serialize'da).
  NOT: bu formül-türevi d2/d4 deadline'ı historical-305 ile AYNI-DEĞİL — d1'in 305'i ayrı "historical" etiket;
  d2/d4 formül-deadline'ı "product-derived" etiketiyle raporlanır (iki-etiket karışmaz).
```

## B0.3 — OBSERVATION HORIZON / CENSORING (§4.3)
```text
T_horizon(cell) = T_serialize_upper + k_rtt × RTT_config + bounded_recovery_allowance + M_fixed_horizon
  k_rtt = 6 (yeterli-recovery-turu üst-zarfı) · bounded_recovery_allowance = 3 × RTT_config · M_fixed_horizon = 200 ms
timeout > T_horizon → CENSORED. Censored latency SAYISAL-latency olarak analize GİRMEZ (§14/v1.0). Extreme-cell
primary-endpoint = P(complete ≤ T_horizon) (uncensored-latency-DEĞİL).
```

## B0.4 — FAITHFULNESS TOLERANCES (DONDUR; §4.4)
```text
RTT  : |p50_realized − configured| ≤ max(5ms, %5)  ∧  p95_realized ≤ configured + max(20ms, %10)
Loss : |realized − configured| ≤ max(2pp, %20-relative)
İhlal → INVALID — NO EVIDENCE (instrument-onar + rerun; §9-2). Harness-kapasitesi farklı-limit-gerektirirse
result-görülmeden burada değiştir+gerekçelendir (şimdilik değiştirilmedi).
```

## B0.5 — ORACLE ORDERING (v1.0 §8 teyit; §4.5)
```text
1. completion probability · 2. deadline success · 3. p95 completion latency · 4. exact realized sealed-wire cost ·
5. CPU/RSS constraint/tiebreak. Oracle YALNIZ measured-arm seçer; missing → ORACLE-ABSTAIN; interpolation YASAK.
```

## B0.6 — SNAPSHOT CONTAMINATION RULE (BAĞLAYICI; §4.6)
```text
FS-1..FS-4 result-bearing runs'da path_timing_snapshot() YALNIZ (a) harness-ledger ve (b) F4 adaptive-K
tarafından tüketilebilir. A0/A1-EARLY/F0/F1/F3 intervention/arm-selection/decision-logic bunu TÜKETEMEZ.
İhlal = INVALID — NO EVIDENCE. (F2 DROPPED — bkz. FS-SEAL-COLLISION-MAPS.md.)
```

## B0.7 — CONFIRMATORY CAUSAL FIXES (§5; result-görülmeden)
```text
C-P1 (RTT250): "IID-loss matched-deficit" KALDIRILDI → deterministic exact-deficit paired-replay.
  RTT=250 · payload=300B · d∈{1,2,4} · util=0.80C · trace=exact deterministic source-loss positions ·
  arms=A0,A1-EARLY(r=d),F0(r=d) · N=10 paired. Üç-soru AYRI: F0-vs-A0(redundancy) · A1-vs-A0(early-byte) ·
  F0-vs-A1(coding-specific; primary coding-claim yalnız BU matched-budget).
C-P2 (high-RTT): arms=A0,A1-EARLY,F0,F1,F3 (5). Üç-claim AYRI raporlanır:
  EMISSION_GAIN=F1/F3-vs-F0 · REDUNDANCY_UTILITY=F1/F3-vs-A0 · CODING_GAIN=F1/F3-vs-A1-EARLY(matched-budget).
  F1/F3 F0'ı-geçip A1'i-geçmezse: emission-fix-VAR, coding-specific-advantage KANITLANMADI.
C-P3 (reverse-blackout): iki-verdict: REDUNDANCY_NICHE=max(A1,F*)-beats-A0 · FEC_SPECIFIC_NICHE=F*-beats-A1
  (matched-budget VEYA eşdeğer-utility-daha-düşük-realized-wire-cost). A1-only-başarı FEC-NICHE-raporlanamaz.
C-P4 (coding-vs-dup): configured-redundancy-class-match ∧ realized-sealed-wire-byte-parity ∧ automated-
  BUDGET-MISMATCH-guard ∧ same-deterministic-trace. Sınıf: CODING_ADVANTAGE|EQUIVALENT_REDUNDANCY|
  DUPLICATION_ADVANTAGE|BUDGET_MISMATCH-NO-CAUSAL-EVIDENCE.
C-P5 (deadline): historical-305-corrigendum-üç-durum-etiket ÖNCE (§14). Sonra: REDUNDANCY_ESSENTIAL=A1|FEC-tutt.
  A0-tuttur-amaz · FEC_ESSENTIAL=FEC-tutt. A1-tuttur-amaz VEYA aynı-success-daha-düşük-wire-cost. FEC>A0-tek-başına
  FEC-Essential-DEĞİL. hits/N exact; ≈% YASAK.
```

## B0.8 — COST/PARETO (v1.0 §6 teyit; v5 §6)
STRICTLY-DOMINATED (primary-utility-kötü/eşit ∧ cost-daha-kötü) → o-cell GO-evidence-taşımaz. Post-hoc-cap-icat YASAK;
sayısal product-cost-kabulü DUR-SON→Abkarada. Hard-disqualifier: unbounded-growth|persistent-starvation|invariant-
ihlal|BUDGET_MISMATCH(coding-claim'inde).

## PROVENANCE
```text
base    : FS-PREREG-v1.0 sha256=cf005739d4fca2d42faf75e5c22c761f379eb6a438f811843d34cd9b3fbee5dc (SUPERSEDED-BY-v1.1)
sha256  : f77e204f01a3feb7c6eb48973981f18603d3edb8fd44f8c2e8091ab9736db941
tarih   : 2026-07-31T14:23:52Z
coverage: FS-COVERAGE-PLAN-v1.1.md (ayrı-hash) · seal-maps: FS-SEAL-COLLISION-MAPS.md
```
