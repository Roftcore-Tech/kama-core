# FS-1-MANIFEST-EXECUTABILITY-v1.3-CANDIDATE (S4-C)

> NON-RESULT executability dry-run (mode=EXECUTABILITY_ONLY; result_bearing=false). Her satır
> RunCfg/workload/source-schedule/arm/K-profile/forward-trace/reverse-trace KONSTRÜKTE-edildi ve
> doğrulandı; channel-loop İLERLETİLMEDİ; raw-result YAZILMADI; FS-1-run-ID HARCANMADI.

manifest : FS-1-MANIFEST-v1.3-CANDIDATE (candidate; NOT-FROZEN) · source_commit dd73770

## Executability totals
```text
total=240 · DISPATCHABLE=240 · unsupported=0 · ambiguous=0 · invalid=0 · duplicate=0
by-family: ExactDeficit=180/180 · NaturalIid=60/60
by-arm: A0=60/60 · A1-EARLY=60/60 · F0=60/60 · F3=60/60
pairing-groups=60 · output_paths_unique=true
```

## Result-bearing firewall
```text
mode=EXECUTABILITY_ONLY · raw_result_files_written=0 · channel_loop_advanced=false ·
outcome_fields_populated=false · fs1_run_ids_spent=0 · raw_namespace=reports/fs-1/raw/PENDING-v1.3-FREEZE
```

## Dry-run command
```text
cargo test -p kama-integration --features experimental-fs-harness --test fs_s4c_pins
```

failures: NONE (240/240 dispatchable).
