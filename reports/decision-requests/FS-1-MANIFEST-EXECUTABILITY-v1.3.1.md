# FS-1-MANIFEST-EXECUTABILITY-v1.3.1 (S5-D2 / OPT-R1)

> v1.3.1 reproducible executor-binary (`8438991b…`) **EXECUTABILITY_ONLY** dry-run + final-manifest-v1.3.1 yapısal doğrulama.
> **result-bearing YOK** (0 loop / 0 raw / 0 run-id). Frozen science DEĞİŞMEDİ.

## Binary dry-run (default mode)
```
mode=EXECUTABILITY_ONLY result_bearing=false
total=240 dispatchable=240 unsupported=0 ambiguous=0 invalid=0 duplicate=0
by-family ExactDeficit=180 NaturalIid=60 | execution_order_rows=240 pairing_groups=60
```

## Final-manifest-v1.3.1 yapısal doğrulama (dosya)
```
rows=240  arms=4 {A0,A1-EARLY,F0,F3; 60/arm}  cells=12  reps=5  groups=60
unique_run_id=240  unique_canonical_output_path=240
execution_binary_sha256 tüm-satır = 8438991b…  raw_artifact_namespace tüm-satır = reports/fs-1/raw/v1.3.1
schema_version tüm-satır = FS-1-MANIFEST-v1.3.1  prereg_status tüm-satır = FROZEN
```

## Kimlik hash'leri
```
final_manifest_sha256      = 12fc0a75da87e49bc078ed230e071c8d8751c981fe7ab9ae8226b591d35ddbcc
candidate_manifest_sha256  = e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10  (v1.3 ile ÖZDEŞ)
execution_binary_sha256    = 8438991bf5cf01309b3a8c9f1b1ec7c2c648a1941e6ce0d49687c2595ac7e514
execution_source_commit    = a3ab2fe
cargo_lock_sha256          = 47893d24…  (v1 ile ÖZDEŞ; yeni-dep YOK)
```

## Behavior-affecting delta
```
candidate → final-v1.3.1        = 0
old-final(v1.3) → new-final(v1.3.1) = 0   (yalnız 7 provenance/operasyonel alan değişti:
  schema_version, candidate_prereg_version, source_commit, execution_source_commit,
  execution_binary_sha256, raw_artifact_namespace, canonical_output_path)
```

## Result-bearing firewall
```
event_loops_advanced=0  campaign_raw_files_created=0  run_ids_spent=0
raw_namespace=reports/fs-1/raw/v1.3.1 (BOŞ; henüz-YOK)
FS-1 ÇALIŞTIRILMADI.
```
