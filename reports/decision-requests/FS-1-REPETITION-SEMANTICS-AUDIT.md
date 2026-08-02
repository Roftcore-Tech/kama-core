# FS-1-REPETITION-SEMANTICS-AUDIT (S4-A)

> **5-repetition = 5 distinct trace-lineage (RULE-10).** ExactDeficit: 5 rep → 5 GARANTİLİ-distinct global-drop-set
> (non-overlapping ranked-window). NaturalIid: 5 rep → 5 distinct trace-seed. repetition_index TEK-BAŞINA distinct
> execution KURMAZ — her rep bir preregistered/frozen behavior-affecting lineage taşır. Pins:
> EXACT_DEFICIT_REPETITION_LINEAGE_UNIQUE + IID_REPETITION_LINEAGE_UNIQUE (fs_s4a_pins). DIAGNOSTIC; outcome-blind.

## 0. Provenance
```text
branch/HEAD : d1-sprint5c / 2e1f139 · v1.2 immutable da4af3c4…034 · FS-1 0/240
frozen seed-domains : "FS1_PAIRED_TRACE_v1" (per-rep lineage seed) · "FS1_EXACT_RANK_v1" (per-cell rank seed) ·
  "FS1_DROPSET_v1" (drop-set hash). Hepsi outcome-blind (yalnız base_cell_semantics_hash + rep).
```

## 1. Frozen lineage-derivation (§5/§6; RULE-10)
```text
paired_trace_seed(cell, rep)  = fnv1a("FS1_PAIRED_TRACE_v1" ++ base_cell_semantics_hash(cell) ++ rep)  [per-rep]
exact_rank_seed(cell)         = fnv1a("FS1_EXACT_RANK_v1" ++ base_cell_semantics_hash(cell))            [per-cell]
ExactDeficit target(cell,rep) = 48-gid'i exact_rank_seed(cell)-ile-rank-et → NON-OVERLAPPING window [rep*d,(rep+1)*d)
  → 5 rep GARANTİLİ-distinct (max rep*d = 4*4 = 16 < 48). size==d, dup-YOK, arm-BAĞIMSIZ, per-block-çarpım-YOK.
NaturalIid lineage(cell,rep)  = forward_trace_seed = paired_trace_seed(cell, rep) (distinct/rep); loss_permille=200 SABİT.
reverse(cell,rep)             = Clear (FS-1 base-matriste reverse cell-defining-DEĞİL; tüm-arm/tüm-rep AYNI).
A1 target lineage(cell,rep)   = dup_seed = domain_seed(paired_trace_seed(cell,rep), "A1_DUP_TARGET") (frozen türetme;
                                deficit-BAĞIMSIZ; loss-trace'ten AYRI — mevcut a1_duplicate_target_set otoritesi).
```
Seed-domain-separation (§6): trace_seed ⟂ A1_target_seed (domain-etiketli fnv1a). Semantic-behavior row-id/filename/
arm-display-name/iteration-order'dan TÜRETİLMEZ.

## 2. ExactDeficit repetition lineages (concrete; 9 cell × 5 rep = 45 lineage)
```text
cell             d | rep0        rep1        rep2        rep3        rep4          | distinct
-----------------+---+------------+-----------+-----------+-----------+-------------+---------
rtt50-exactd1    1 | [42]        [43]        [40]        [41]        [46]          | 5/5
rtt50-exactd2    2 | [28,29]     [30,31]     [24,25]     [26,27]     [20,21]       | 5/5
rtt50-exactd4    4 | [40..43]    [44..47]    [32..35]    [36..39]    [8..11]       | 5/5
rtt250-exactd1   1 | [43]        [42]        [41]        [40]        [47]          | 5/5
rtt250-exactd2   2 | [38,39]     [36,37]     [34,35]     [32,33]     [46,47]       | 5/5
rtt250-exactd4   4 | [40..43]    [44..47]    [32..35]    [36..39]    [24..27]      | 5/5
rtt1000-exactd1  1 | [24]        [25]        [26]        [27]        [28]          | 5/5
rtt1000-exactd2  2 | [20,21]     [22,23]     [16,17]     [18,19]     [28,29]       | 5/5
rtt1000-exactd4  4 | [40..43]    [44..47]    [32..35]    [36..39]    [8..11]       | 5/5
```
Her cell: 5 distinct drop-set (REPETITION SEMANTICS DUPLICATION-YOK → DUR-B tetiklenmedi). Tüm-6-arm AYNI drop-set
(arm target'a girmez). (Detaylı block-dağılım/tail-coverage → S4-C ExactDeficit-position-coverage-report.)

## 3. NaturalIid repetition lineages (3 cell × 5 rep = 15 lineage)
```text
cell            | 5 forward_trace_seed (distinct)
----------------+--------------------------------------------------------------------------------
rtt50-iid20     | 0xf1c2d27e… 0xf1c2d37e… 0xf1c2d47e… 0xf1c2d57e… 0xf1c2ce7e…  (5/5 distinct)
rtt250-iid20    | 0x385c4ec5… 0x385c4fc5… 0x385c50c5… 0x385c51c5… 0x385c52c5…  (5/5 distinct)
rtt1000-iid20   | 0x679d1231… 0x679d1331… 0x679d1431… 0x679d1531… 0x679d1631…  (5/5 distinct)
```
loss_permille = 200 (SABİT; matched-deficit YOK). configured_exact_drop_count = None (historical-deficit-label YOK).
(distinct configured-trace-hash + distinct source-admission-pattern → S4-C IID-seed-coverage-report.)

## 4. Değiştirilmeyenler (§5 "do not vary to manufacture uniqueness")
```text
workload · RTT · deficit-magnitude(d) · arm-mekanizma · deadline · utilization repetition-arası SABİT.
Yalnız trace-lineage (ExactDeficit drop-set / IID seed) + türetilmiş A1-target rep'e göre değişir → gerçek
behavior-affecting distinction (RULE-10). repetition-index tek-başına execution kurmaz.
```
