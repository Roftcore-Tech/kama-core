# FS-1-MANIFEST-FREEZE-DIFF (S5-D; candidate → final)

> **behavior-affecting changes = 0.** FS-1-MANIFEST-v1.3-CANDIDATE.jsonl → FS-1-MANIFEST-v1.3.jsonl controlled-transform:
> YALNIZ provenance-only + operational-path-only field-değişimi. Candidate KORUNDU (sha e9e331a4… değişmedi).

## 0. Provenance
```text
candidate : FS-1-MANIFEST-v1.3-CANDIDATE.jsonl · sha256 e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10 (KORUNDU)
final     : FS-1-MANIFEST-v1.3.jsonl · sha256 6e8014ab63a2e6fda4a72759c4fee2989131daf0131205b5a71bd04354ed890b
transform : deterministic 6-sed (order-preserving); regen byte-identical (§26). runtime-code DEĞİŞMEDİ (post-baseline 5c7e005; §23).
```

## 1. Changed-field sınıflandırması (§24)
```text
field                          | candidate → final                                   | class
-------------------------------+-----------------------------------------------------+---------------------
schema_version                 | FS-1-MANIFEST-v1.3-CANDIDATE → FS-1-MANIFEST-v1.3    | PROVENANCE-ONLY (label)
prereg_status                  | CANDIDATE_NOT_FROZEN → FROZEN                        | PROVENANCE-ONLY
candidate_prereg_version       | FS-1-MANIFEST-v1.3-CANDIDATE → FS-PREREG-v1.3        | PROVENANCE-ONLY (prereg-version name; SHA DEĞİL — §3-B)
source_commit                  | dd73770 → 5c7e005 (execution-baseline commit)       | PROVENANCE-ONLY
raw_artifact_namespace         | reports/fs-1/raw/PENDING-v1.3-FREEZE → .../v1.3      | OPERATIONAL-PATH-ONLY
canonical_output_path          | .../PENDING-v1.3-FREEZE/<id>.json → .../v1.3/<id>.json | OPERATIONAL-PATH-ONLY
+intended_execution_mode       | (yok) → RESULT_BEARING                               | OPERATIONAL-PATH-ONLY (added; §27)
+execution_binary_sha256       | (yok) → 75ca6eb7…0461                                | PROVENANCE-ONLY (added; execution-baseline)
+execution_source_commit       | (yok) → 5c7e005                                     | PROVENANCE-ONLY (added)
-------------------------------+-----------------------------------------------------+---------------------
BEHAVIOR-AFFECTING             | (hiçbiri)                                            | **0**
```

## 2. Behavior-affecting parity (candidate = final; IDENTICAL — kanıtlandı)
```text
DEĞİŞMEDİ (extract+diff empty): base_cell_semantics_hash · resolved_behavior_semantics_hash · configured_schedule_hash ·
run_execution_semantics_hash · row_identity_hash · canonical_source_sequence_hash · source_offer_schedule_hash ·
forward_trace_hash · exact_drop_set_hash · forward_trace_seed · rtt_ms · deadline_ms · exact_global_drop_ids ·
iid_loss_permille · source_offer_pacer_bps · bottleneck_capacity_bps · selected_k · repair_count · configured_r_per_block ·
source_block_count. ⇒ base-cell-set/arm-set/repetition-lineage/ExactDeficit-targets/IID-seeds/RTT/deadline/K/arm-geometry
DEĞİŞMEDİ (§24 forbidden-list HEPSİ korundu). behavior-affecting-change = 0 → S4-evidence GEÇERLİ.
```

## 3. §25 final-manifest checks
```text
rows=240 · base-cells=12 · arms=4 · reps=5 · pairing-groups=60 · rows/group=4.
240 unique run-id · 240 unique output-path · 240 unique exec-hash · 240 unique resolved-behavior-hash.
F1-row=0 · F4-row=0 · NaturalIid-deficit=0 · pending/candidate-namespace=0 · 305ms=0 · RTT2500=0 · 5099-FS1-deadline=0.
```

## 4. §26 final executability + determinism
```text
frozen-binary (75ca6eb7…0461; main-target reproduce ✓) EXECUTABILITY_ONLY → 240/240 dispatchable; 0 unsupp/ambig/invalid/dup;
0 event-loop; 0 raw-file; 0 run-id. regen byte-identical (transform 2× → aynı-sha 6e8014ab…). candidate KORUNDU (e9e331a4…).
```
