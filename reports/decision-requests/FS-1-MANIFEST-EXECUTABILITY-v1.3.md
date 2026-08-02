# FS-1-MANIFEST-EXECUTABILITY-v1.3 (S5-D; final)

> **240/240 DISPATCHABLE.** Final manifest (FS-1-MANIFEST-v1.3.jsonl; sha 6e8014ab…) frozen-binary (75ca6eb7…)
> ile EXECUTABILITY_ONLY doğrulandı. behavior-affecting-change candidate→final = 0. regen byte-identical.
> result-bearing İCRA-YOK (0 loop / 0 raw / 0 run-id).

## Totals
```text
total=240 · DISPATCHABLE=240 · unsupported=0 · ambiguous=0 · invalid=0 · duplicate=0
by-family : ExactDeficit=180/180 · NaturalIid=60/60
by-arm    : A0=60/60 · A1-EARLY=60/60 · F0=60/60 · F3=60/60
pairing-groups=60 · output_paths_unique=true
```

## Provenance
```text
final_manifest_sha256     : 6e8014ab63a2e6fda4a72759c4fee2989131daf0131205b5a71bd04354ed890b
candidate_manifest_sha256 : e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10 (KORUNDU)
execution_binary_sha256   : 75ca6eb7b65047809ab9e27e579e20c819bd8e664e6085130c540703896e0461 (3+1 build byte-özdeş)
execution_source_commit   : 5c7e005 · cargo_lock_sha256 47893d24…4 · rustc/cargo 1.96.0
behavior_affecting_changes: 0 (FS-1-MANIFEST-FREEZE-DIFF.md) · regeneration_byte_identical: true
```

## Result-bearing firewall
```text
mode=EXECUTABILITY_ONLY · event_loops_advanced=0 · campaign_raw_files_created=0 · run_ids_spent=0 ·
raw_namespace=reports/fs-1/raw/v1.3 (BOŞ; henüz-YOK). intended_execution_mode=RESULT_BEARING (S6-onayı + --result-bearing
flag + freeze-lock ile İCRA). dry-run command: target/release/examples/fs1_runner (frozen; default EXECUTABILITY_ONLY).
```
