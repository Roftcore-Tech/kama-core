# FS-1 FROZEN ANALYSIS (Phase-1) — RAW-RECORD SCHEMA CONFORMANCE GAP → DUR-B

> **VERDICT: RAW-RECORD SCHEMA CONFORMANCE GAP — DUR-B. STOPPED at the Phase-1 §1/§6 gate, BEFORE building the
> analysis pipeline and BEFORE reading any campaign outcome.** The immutable campaign dataset does not conform to the
> frozen `FS-1-RAW-SCHEMA-v1` (c7ac2a85, referenced by freeze-lock v1.3.1): each record carries **27 keys** vs the
> schema's **54 required fields** — **32 schema-required fields are absent**, including **10 the frozen analysis §2
> outcome domains require**. No analysis pipeline can produce fields that were never written. Per §1/§4/§6/§10 I do not
> weaken the schema, invent proxies, build on the incomplete dataset, or re-run. This is Abkarada's decision.
> **Root cause is my OPT-R1 executor** — the `RawRecord` serializer never implemented the frozen raw-schema.

## §1 freeze verification — ALL PASS (recomputed; this is NOT a freeze/dataset-hash mismatch)
```text
analysis_spec 6e2056ba · oracle_map 0bee78db · coverage 3c80c685 · final_manifest 12fc0a75 · raw_schema c7ac2a85 ·
execution_order be692dea · dataset_concat 4048e663 (==checkpoint) · ledger a3e332bc (==checkpoint). All == freeze-lock.
```

## The gap — campaign record ⊄ frozen FS-1-RAW-SCHEMA-v1 (code-vs-schema; outcome-blind)
```text
frozen FS-1-RAW-SCHEMA-v1 required fields : 54  (identity 6 / freeze_provenance 10 / configuration 5 / trace 9 /
                                                 timing 9 / outcome 9 / cost 6)
campaign RawRecord serialized keys        : 27  (OPT-R1 fs_exec.rs RawRecord + serialize_raw_record)
MISSING (schema-required, absent) : 32
  [freeze_provenance] prereg_sha256 · coverage_sha256 · analysis_spec_sha256 · oracle_map_sha256 · cargo_lock_sha256 · toolchain_identity
  [configuration]     typed_manifest_row · configured_schedule_hash
  [trace]             configured_forward_hash · configured_reverse_hash · observed_forward_event_hash · observed_reverse_event_hash ·
                      initial_source_decisions · intervention_decisions · retransmission_decisions · actual_forward_event_count · actual_reverse_event_count
  [timing]            t_first_source_offer · t_final_source_wire_complete · t_redundancy_lifecycle · t_repair_usable · t_arq_lifecycle ·
                      t_full_workload_complete · deadline_ms · observation_horizon_ms
  [outcome]           completion_or_censoring · completion_cause · remaining_fragments · tail_status
  [cost]              proactive_intervention_bytes · control_bytes · spurious_retransmissions
EXTRA (present, not in schema / renamed) : completion_ns · intervention_bytes · retransmits · fec_decoded_sources · result_bearing
  (note: intervention_bytes vs schema proactive_intervention_bytes, and the absent spurious_retransmissions vs
   present-but-differently-named data — NAME nonconformance too, not only omissions.)
```

## Analysis impact — frozen §2 outcome domains that CANNOT be computed from the recorded dataset
```text
LATENCY   : repair-usable-time (t_repair_usable) ABSENT · ARQ-recovery-time (t_arq_lifecycle) ABSENT
            → only full-completion-time is available; repair/ARQ latency domains BLOCKED.
COST      : spurious_retransmissions ABSENT · control_bytes ABSENT → spurious/control cost BLOCKED.
MECHANISM : completion_cause ABSENT · tail-inclusion (tail_status) ABSENT · reverse-path-state (observed_reverse_event_hash) ABSENT ·
            duplicate/intervention-decisions (intervention_decisions) ABSENT → MECHANISM domain largely BLOCKED.
OUTCOME   : completion_or_censoring ABSENT · remaining_fragments ABSENT.
COMPUTABLE from what WAS recorded : reliability · deadline hit/miss/censored · full-completion latency (complete rows) ·
            byte-cost (intervention/ARQ/total) · censoring/pairing accounting · tail-membership DERIVABLE from frozen
            manifest exact_global_drop_ids · F4-derived policy · integrity.
```
A schema-conformant pipeline (§6: "eksik required-field sessizce kabul edilmez") would reject all 240 records
(32 missing required fields each). Re-deriving the fields by re-executing the deterministic kernel is forbidden (§10,
no re-run) and would not be the frozen on-disk dataset. → the current dataset cannot support the frozen analysis.

## IMPORTANT — the execution kernel already computes these fields (defect is in the serializer, not the science)
```text
run_spine SpineResult ALREADY produces (byte-unchanged frozen kernel): t_first_source_offer · t_final_source_wire_complete ·
  t_first_repair_usable · t_full_workload_complete · spurious_retransmits · a1_dup_hits/a1_dup_misses · reverse_event_hash ·
  reverse_event_count · reverse_dropped_count · reverse_blackout_intervals · dropped_global_ids · realized_initial_source_drop_ids ·
  realized_{initial_source,retransmission,duplicate,repair}_losses · structural_class · path_timing (PathTimingLedger) ·
  timing (BlockTiming) · reconstruct_byte_equal · recovered_control_used · forward_trace_hash · configured_schedule_hash · …
The provenance fields (prereg/coverage/analysis_spec/oracle_map/cargo_lock/toolchain) are frozen constants available from
the lock/manifest. → essentially the entire 54-field schema is producible; the OPT-R1 build_raw_record/RawRecord simply
serialized a ~27-key subset. The SCIENCE is intact in-kernel; only the PERSISTED record is incomplete.
```

## Root cause (honest)
```text
OPT-R1 executor: fs_exec.rs RawRecord + fs_campaign.rs build_raw_record implemented a REDUCED ad-hoc record and did NOT
cross-validate against the frozen FS-1-RAW-SCHEMA-v1 (the "RAW-SCHEMA §8" comment was a mis-reference; the schema has
field_groups, no §8). Latent through OPT-R1 → S6-R → the executed campaign; surfaces now at the analysis §1/§6 gate.
This is a defect I introduced, surfaced plainly (not minimized).
```

## Why STOP (not build / not fix+re-run / not amend — all need Abkarada)
```text
§1  do not modify/weaken the frozen raw-schema or analysis spec.
§4  do not invent a scientifically-meaningful default (missing-field handling here is defined by schema, data lacks them).
§6  missing required fields are not silently accepted; the schema is NOT silent → not an UNDERSPECIFIED-invent case.
§10 no experiment row may be re-run without a separate GO; re-derivation-by-re-execution is not the frozen dataset.
§14/§20 (analysis directive) required frozen inputs absent → insufficient, do NOT invent a proxy.
Fixing the serializer + re-running, or amending the spec to reduce scope, are BOTH governance decisions.
```

## Integrity firewall (held)
```text
NO campaign outcome VALUE read; NO real raw record parsed. Findings are from: frozen schema doc (FS-1-RAW-SCHEMA-v1) +
fs_exec.rs/fs_campaign.rs code (RawRecord/build_raw_record) + fs_spine.rs SpineResult struct. Dataset, spec, primitives,
freeze-locks, and the campaign binary are UNCHANGED. No Phase-1 pipeline code was written or committed.
```

## Resolution options (Abkarada decision)
```text
OPT-X (RECOMMENDED — serializer-completeness fix + re-run; SCIENCE-INVARIANT):
  Expand RawRecord + build_raw_record to serialize the FULL FS-1-RAW-SCHEMA-v1 (all 54 fields) from the already-rich
  SpineResult + Provenance + manifest. run_spine (frozen scientific kernel) stays BYTE-UNCHANGED → scientific outcomes
  invariant (same completions/censoring/costs), only the recorded field set expands. → new executor binary → re-freeze
  (v1.3.2; current freeze-lock v1.3.1 + current dataset PRESERVED as SUPERSEDED-INCOMPLETE-SCHEMA, not deleted) →
  science-invariance proof (run_spine byte-unchanged; recorded outcomes reproduce) → SEPARATE re-run GO → schema-conformant
  240-row dataset → THEN Phase-1 analysis-pipeline build over the conformant schema. (Structurally the raw-record analog
  of OPT-R1: fix the persistence layer, keep the frozen science, re-freeze, re-run.)

OPT-Y (frozen amendment — reduce analysis scope to the recorded subset): NOT recommended. Amend FS-ANALYSIS-SPEC +
  FS-1-RAW-SCHEMA to the recorded domains (RELIABILITY + DEADLINE + full-completion LATENCY + byte-COST + tail-from-manifest)
  and explicitly DROP MECHANISM (completion_cause/tail_status/reverse/duplicate), LATENCY repair/ARQ times, COST
  spurious/control. This permanently narrows the science to work around a serialization bug the kernel does not have —
  discarding capability run_spine already supports. Requires explicit authorization + documented scientific-scope caveats.

NOT permitted (any path): proxy-invention for absent fields · silent partial pipeline build · re-derivation by re-executing
  the deterministic kernel · deleting/overwriting the current dataset.
```

## STATUS
```text
DUR-B : RAW-RECORD SCHEMA CONFORMANCE GAP. Phase-1 pipeline NOT built. No scientific interpretation. Freeze chain intact;
        current dataset immutable + preserved. Awaiting Abkarada decision (OPT-X recommended). Do NOT proceed unprompted.
```
