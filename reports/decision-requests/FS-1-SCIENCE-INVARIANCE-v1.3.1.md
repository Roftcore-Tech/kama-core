# FS-1-SCIENCE-INVARIANCE-v1.3.1 (OPT-R1 §8)

> **İDDİA: v1.3 → v1.3.1 geçişinde FROZEN SCIENCE'ın HER alanı değişmez. scientific-behavior-changes = 0.**
> Değişen tek şey: executor-implementation + reproducible-binary + bunlara-bağlı provenance/operasyonel-alanlar
> (OPT-R1 pre-outcome runtime/provenance amendment; result_bearing_runs_before_correction = 0).

## 0. Kanıt yöntemi
İki bağımsız kanıt kolu:
- **(A) Kod-atalet**: bilimsel-davranışı-üreten harness modülleri baseline-anchor `5c7e005` → yeni-baseline
  `a3ab2fe` arası **byte-değişmez** (git-diff boş) → `run_spine` + manifest-generator + cell/dryrun/analysis
  semantiği ÖZDEŞ.
- **(B) Artefakt-atalet + manifest-alan-eşitliği**: her bilimsel-otorite dosyası byte-değişmez (sha256 sabit);
  final-manifest-v1.3.1 içindeki 54 bilimsel-alanın 240 satırda tümü candidate (e9e331a4) ve old-final (6e8014ab)
  ile ÖZDEŞ (behavior-affecting-delta = 0, `rebind_v131.py` doğrulaması).

## 1. Kod-atalet (A) — bilimsel-davranış üreten modüller byte-değişmez (5c7e005 → a3ab2fe)
```
fs_manifest.rs   UNCHANGED   (manifest_jsonl / row_hashes / execution_order_entries / deadline_ms_for)
fs_cell.rs       UNCHANGED   (typed FsImpairment / base_cell_semantics_hash / exact_deficit_target_set /
                              paired_trace_seed / observation_horizon_ms)
fs_dryrun.rs     UNCHANGED   (executability_sweep / RowStatus)
fs_spine.rs      UNCHANGED   (run_spine / SpineArm / SpineCfg / SpineResult)  ← arm-mekanizma + kanal fiziği
fs_analysis.rs   UNCHANGED   (classify_validity / oracle_class)
fs_workload.rs   UNCHANGED   (workload geometrisi)
```
Sonuç: aynı-girdi → aynı-çıktı. candidate-manifest generator-sha = **e9e331a4** (değişmez); yeni-binary
EXECUTABILITY_ONLY = **240/240** (ExactDeficit=180, NaturalIid=60, 60 group) — v1.3 ile ÖZDEŞ.

## 2. Bilimsel-otorite artefaktları byte-değişmez (sha256 sabit; git-unchanged)
```
FS-1-MANIFEST-v1.3-CANDIDATE.jsonl   e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10  (bilim-çapa)
FS-1-EXECUTION-ORDER-v1.json         be692dea79334c7a4b78c808d0106b3d99ddff81775e311d01363bcfe5752623  (§4 execution-order)
FS-1-RETRY-AND-INVALIDITY-POLICY.md  035c724e4740bb2a6c930b92083047fd7c78931577c59cd02355bc443710734a  (§3 retry/invalidity)
FS-1-OBSERVATION-HORIZON-AUDIT.md    92c9dbf76a7451a2f3b220dbdb7b4c0b72de80a72b6100d1339d32844458d99d  (horizon-otorite)
FS-ANALYSIS-SPEC-v1.md               6e2056ba23c9323308bba57ce8a748270af05270da48373e0c93fb52c274f567  (analiz-otorite)
FS-ORACLE-AUTHORITY-MAP-v1.md        0bee78dbed3747aae84704437430446e339bfb9275be80c7110e54151f7c6fca  (oracle-otorite)
FS-COVERAGE-PLAN-v1.2.md             3c80c6856a9d1383a4e1f1d2c761ff50578e325e08a1cbdf9860e25181e2df16  (kapsam)
FS-BULK-STRUCTURAL-NULL-v1.md        c37bfcbe6bfc5c1d8aeb31281f72d3d2f1a628ab213830166440dade47aeeefa  (yapısal-null)
FS-F4-DERIVED-POLICY-v1.md           08e304b2660c805572ff2aae1ba59c1105ab9eb1c0507796469f35e20cd4ff3f  (F4-türev-politika)
FS-1-RAW-SCHEMA-v1.json              c7ac2a85d25916111600e3a0d566774da2042bd037adc4a3b24623282217168c  (raw-şema)
FS-1-RAW-ARTIFACT-POLICY.md          cfd4beff7c97af8c20fa85249cb66990a57b2170f28c16d7c6db198517302bba  (raw-politika)
```

## 3. Manifest-alan-eşitliği (B) — final-v1.3.1'in 54 bilimsel-alanı 240 satırda ÖZDEŞ
```
behavior_affecting_changes(new-final vs old-final) = 0
behavior_affecting_changes(new-final vs candidate)  = 0
```
Kapsanan 54 bilimsel-alan (provenance/operasyonel HARİÇ): run_id · base_cell_id · pairing_group_id · arm ·
repetition_index · app_unit_bytes/count · total_app_bytes · source_symbol_E/count · sealed_source_bytes ·
base_source_wire_bytes · canonical_source_sequence_hash · rtt_ms · bottleneck_capacity_bps ·
source_offer_pacer_bps · source_offer_schedule_hash · source_offer_envelope_ns · deadline_ms · impairment_kind ·
configured_exact_drop_count · exact_global_drop_ids · exact_drop_set_hash · iid_loss_permille · forward_trace_seed ·
forward_trace_hash · reverse_spec/schedule_id/trace_seed/trace_hash · configured_blackout_intervals · selected_k ·
selected_profile_id · source_block_count · configured_r_per_block · repair_count · duplicate_policy · close_policy ·
expected_structural_classification · trace_seed · reverse_seed · a1_target_seed · domain_derivation_version ·
base_cell_semantics_hash · run_execution_semantics_hash · resolved_behavior_semantics_hash · row_identity_hash ·
configured_schedule_hash · schema_validator_version · result_bearing · expected_binary_provenance · expected_harness_version.

## 4. OPT-R1 donmuş-bilim maddeleri → kanıt eşlemesi
```
12 base-cell                     → base_cell_id 12-distinct + base_cell_semantics_hash EŞİT  (§3)
{A0,A1-EARLY,F0,F3}              → arm 4-distinct, 60/arm EŞİT  (§3)
5 rep                            → repetition_index EŞİT  (§3)
240 row / 60 group               → satır=240, pairing_group_id 60-distinct, 240 unique run_id  (§3)
workload-geometri                → app_unit_*/source_symbol_*/sealed_source_bytes/base_source_wire_bytes/
                                   canonical_source_sequence_hash EŞİT  (§3)
ExactDeficit-target-set          → exact_global_drop_ids + exact_drop_set_hash + configured_exact_drop_count EŞİT  (§3)
NaturalIid-seed                  → forward_trace_seed/hash + iid_loss_permille + trace_seed EŞİT  (§3)
RTT'ler                          → rtt_ms EŞİT  (§3)
deadline'lar (199/599/2099)      → deadline_ms EŞİT  (§3)
horizon'lar (699/2499/9249)      → observation_horizon_ms(rtt) fs_cell.rs byte-değişmez + rtt EŞİT → türetilmiş EŞİT  (§1)
source-pacing (0.80C)            → source_offer_pacer_bps=800000 + schedule_hash + envelope_ns +
                                   bottleneck_capacity_bps EŞİT  (§3)
arm-mekanizmaları                → run_spine byte-değişmez (§1) + selected_k/configured_r_per_block/repair_count/
                                   a1_target_seed/resolved_behavior_semantics_hash EŞİT  (§3)
execution-order                  → FS-1-EXECUTION-ORDER-v1.json (be692dea) byte-değişmez  (§2)
validity/censoring               → FS-1-RAW-SCHEMA (c7ac2a85) + fs_analysis.classify byte-değişmez  (§1,§2)
retry-policy                     → FS-1-RETRY-AND-INVALIDITY-POLICY.md (035c724e) byte-değişmez  (§2)
analysis/oracle-authority        → FS-ANALYSIS-SPEC (6e2056ba) + FS-ORACLE-AUTHORITY-MAP (0bee78db) byte-değişmez  (§2)
```

## 5. Değişen (pre-outcome runtime/provenance; bilim-DIŞI)
```
executor-implementation (fs_exec.rs YENİ + fs_campaign.rs YENİ + fs1_runner.rs pozitif-yol) ·
execution-binary  75ca6eb7 → 8438991b (4-build byte-özdeş; reproducible) ·
execution-source-commit  5c7e005 → a3ab2fe ·
manifest provenance/operasyonel alanları  schema_version(v1.3→v1.3.1) · candidate_prereg_version(→FS-PREREG-v1.3.1) ·
  source_commit(→a3ab2fe) · execution_source_commit(→a3ab2fe) · execution_binary_sha256(→8438991b) ·
  raw_artifact_namespace(v1.3→v1.3.1) · canonical_output_path(→v1.3.1) ·
prereg  FS-PREREG-v1.3 → FS-PREREG-v1.3.1 ·  lock  FS-1-FREEZE-LOCK-v1.3(5a9bc09d; INVALIDATED-preserved) → v1.3.1.
Cargo.lock byte-özdeş (47893d24; yeni-dep YOK). result_bearing_runs_before_correction = 0.
```

## 6. Sonuç
```
scientific-behavior-changes = 0.
Kanıt: kod-atalet (6 modül byte-değişmez) + artefakt-atalet (11 otorite sha256 sabit) +
       manifest 54-bilimsel-alan × 240-satır ÖZDEŞ (candidate & old-final ile).
v1.3.1 = v1.3'ün EXECUTOR-tamamlanmış hali; bilim AYNI kampanya.
```
