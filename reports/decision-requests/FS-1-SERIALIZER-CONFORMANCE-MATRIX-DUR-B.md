# FS-1 OPT-X Aşama-A — 54-FIELD CONFORMANCE MATRIX → DUR-B

> **VERDICT: the frozen raw-schema cannot be satisfied by a persistence-layer-only change. STOPPED before any code
> change (§2/§3).** The executable 54-of-54 conformance check (KALICI-KURAL-3) fails: a subset of `FS-1-RAW-SCHEMA-v1`
> fields is **not exposed by the terminal `SpineResult`/kernel** (recording them needs a `SpineResult`/kernel change,
> which §1 forbids → **SCIENCE-LAYER-CHANGE-REQUIRED**), and a further subset is **not defined at the type/representation
> level by the frozen schema** (→ **UNAVAILABLE-OR-UNDERSPECIFIED-SCHEMA-FIELD**). OPT-X (serializer-only) is therefore
> insufficient. This is Abkarada's decision. **No outcome value read; no code/kernel/schema modified.**

## Reconciliation (matches the directive's expected arithmetic)
```text
frozen FS-1-RAW-SCHEMA-v1 required fields : 54
current v1.3.1 serialized keys            : 27
missing required (by name)                : 32   matched required (by name) : 22   → 5 current keys outside schema set
```

## The 5 current keys outside the schema-required set (§2 explanation)
```text
completion_ns          LEGACY ALIAS → canonical timing.t_full_workload_complete (mechanical rename; keep canonical in v1.3.2 record).
intervention_bytes     LEGACY ALIAS → canonical cost.proactive_intervention_bytes (mechanical rename).
retransmits            EXTRA (count). Schema names retransmission_decisions (a different, per-event field). retransmits ≠ that field.
fec_decoded_sources    EXTRA. Not in schema field_groups. Useful MECHANISM datum (FEC-recovered-source-count) the analysis wants but
                       the frozen schema does not list → schema does not permit unknown fields (§6) unless amended.
result_bearing         EXTRA (provenance flag). Not in schema field_groups.
(Frozen FS-1-RAW-SCHEMA-v1 gives NO "allow unknown fields" clause → these 5 are non-conformant as-is.)
```

## 54-field source matrix (status per field)
Status: **DIRECT** = present in terminal SpineResult/provenance/manifest · **MECH** = mechanical serializer rename/arith on
kernel-provided values · **BLOCK-UNAVAIL** = not exposed by terminal SpineResult (needs kernel/SpineResult change → §1) ·
**BLOCK-USPEC** = frozen schema does not define its type/representation/derivation/boundary → §3.

```text
GROUP identity (6) — all DIRECT
  run_id · attempt_index · pairing_group_id · arm · base_cell_id · repetition_index        → DIRECT (run ctx + SpineResult.arm)

GROUP freeze_provenance (10) — all DIRECT (immutable constants from freeze-lock/manifest)
  freeze_lock_sha256 · prereg_sha256 · final_manifest_sha256 · coverage_sha256 · analysis_spec_sha256 ·
  oracle_map_sha256 · execution_binary_sha256 · execution_source_commit · cargo_lock_sha256 · toolchain_identity → DIRECT

GROUP configuration (5) — all DIRECT
  typed_manifest_row (frozen manifest row) · base_cell_semantics_hash · run_execution_semantics_hash ·
  resolved_behavior_semantics_hash · configured_schedule_hash (SpineResult.configured_schedule_hash)          → DIRECT

GROUP trace (9) — 3 DIRECT, 6 BLOCKED
  configured_forward_hash        DIRECT  SpineResult.forward_trace_hash
  configured_reverse_hash        DIRECT  SpineResult.reverse_trace_hash
  observed_reverse_event_hash    DIRECT  SpineResult.reverse_event_hash (= ch.reverse_decision_log_hash())
  actual_reverse_event_count     DIRECT  SpineResult.reverse_event_count
  observed_forward_event_hash    BLOCK-UNAVAIL  forward observed-event hash (ch.decision_log_hash()) is NOT exposed in terminal SpineResult.
  actual_forward_event_count     BLOCK-USPEC    no forward event count in SpineResult; derivation not defined by schema.
  initial_source_decisions       BLOCK-USPEC    representation undefined (vector? drop-id list? hash?); per-event vector not retained.
  intervention_decisions         BLOCK-UNAVAIL  per-event decision VECTOR not retained — TraceChannel keeps only a rolling FNV hash
                                                (decision records XOR-ed into decision_log_hash then DISCARDED). Kernel change needed.
  retransmission_decisions       BLOCK-UNAVAIL  same — no per-event retransmission decision vector retained anywhere.

GROUP timing (9) — 5 DIRECT/MECH, 2 BLOCKED
  t_first_source_offer           DIRECT  SpineResult.t_first_source_offer
  t_final_source_wire_complete   DIRECT  SpineResult.t_final_source_wire_complete
  t_repair_usable                DIRECT  SpineResult.t_first_repair_usable / timing.t_repair_usable
  t_full_workload_complete       DIRECT  SpineResult.t_full_workload_complete
  deadline_ms                    DIRECT  manifest/config
  observation_horizon_ms         DIRECT  config
  censor_time_ns                 MECH    from horizon when !completed (source-driven)
  t_redundancy_lifecycle         BLOCK-USPEC  BlockTiming has t_redundancy_{eligible,enqueued,wire,arrival} (4 distinct) — "lifecycle"
                                             as a single record value is undefined; timing is PER-BLOCK (which block? aggregation undefined).
  t_arq_lifecycle                BLOCK-USPEC  BlockTiming has t_arq_{eligible,wire,arrival} — single "lifecycle" value + per-block
                                             aggregation undefined by the frozen schema.

GROUP outcome (9) — 5 DIRECT/MECH, 4 BLOCKED
  validity_class                 DIRECT  classify()
  deadline_hit                   DIRECT
  delivered_fragments            DIRECT  SpineResult.delivered_source_symbols
  delivered_bytes                DIRECT  SpineResult.delivered_app_bytes
  reassembly_equal               DIRECT
  remaining_fragments            MECH    source_symbol_count − delivered_source_symbols (both kernel-provided)
  completion_or_censoring        BLOCK-USPEC  representation undefined (timestamp? enum? which value).
  completion_cause               BLOCK-USPEC  no frozen rule maps {fec_decoded_sources, retransmits, structural_class} → a completion-cause
                                             enum; deriving one is invention. (StructuralClass enum ≠ completion-cause taxonomy.)
  tail_status                    BLOCK-USPEC  the TAIL boundary (which global-drop-IDs = "tail") is NOT defined in any frozen artifact
                                             (spec §5 says "tail-stratum frozen-global-drop-ID'lerden ÖN-İLAN" but gives no boundary).

GROUP cost (6) — 5 DIRECT/MECH, 1 BLOCKED
  base_source_bytes              DIRECT
  proactive_intervention_bytes   MECH    SpineResult.intervention_bytes (rename)
  arq_retransmit_bytes           DIRECT
  total_outcome_bytes            DIRECT
  spurious_retransmissions       MECH    SpineResult.spurious_retransmits (rename)
  control_bytes                  BLOCK-UNAVAIL  no control_bytes count in SpineResult (only recovered_control_used: bool). Not exposed.
```

## Summary of blocked fields (11)
```text
BLOCK-UNAVAIL (kernel/SpineResult does not expose the data → §1 SCIENCE-LAYER-CHANGE-REQUIRED): 4
  intervention_decisions · retransmission_decisions · observed_forward_event_hash · control_bytes
BLOCK-USPEC (frozen schema does not define representation/derivation/boundary → §3 UNAVAILABLE-OR-UNDERSPECIFIED): 7
  initial_source_decisions · actual_forward_event_count · t_redundancy_lifecycle · t_arq_lifecycle ·
  completion_or_censoring · completion_cause · tail_status
DIRECT/MECH (sourceable without kernel change): 43 / 54.
```
The 3 hardest are provable-unavailable: `intervention_decisions`, `retransmission_decisions` (per-event vectors are
XOR-hashed then discarded — never stored), and `control_bytes` (never computed). These alone break OPT-X's
persistence-only boundary: recording them requires changing `TraceChannel`/`SpineResult` (kernel), which §1 forbids.

## Overarching finding (KALICI-KURAL-3)
```text
FS-1-RAW-SCHEMA-v1 is a field-NAME list grouped into 7 categories. It defines NO field types, nullability, representations,
enum domains (beyond validity_class_domain), or null/zero/empty semantics. A §6 type-exact conformance VALIDATOR
("exact types/enums · required/nullability · 0 unknown field") therefore cannot be built against it without inventing the
type contract (forbidden §3/§4). Artifact-hash identity (c7ac2a85 verified) did NOT prove serializer conformance — exactly
the failure class KALICI-KURAL-3 names. The frozen raw-schema and the frozen kernel (run_spine/TraceChannel/SpineResult)
are mutually INCONSISTENT: the schema requires data the kernel discards/never-computes, and underspecifies several fields.
```

## Why STOP (not proceed / not invent / not touch kernel)
```text
§1  changing SpineResult/TraceChannel/kernel to expose decisions/control_bytes exceeds the OPT-X boundary → SCIENCE-LAYER-CHANGE-REQUIRED.
§2  "Sayılar veya sınıflandırma tutmazsa kod değiştirmeden DUR-B ile dur." — 54-of-54 does not hold → STOP before code change.
§3  no invented/estimated/sentinel values; no schema-undefined representations; null/empty semantics undefined for optional fields.
§4  do not invent scientifically-meaningful defaults (tail-boundary, completion-cause taxonomy, lifecycle aggregation).
Integrity firewall: no campaign outcome VALUE read. Findings are from frozen schema doc + kernel source (SpineResult,
BlockTiming, TraceChannel, StructuralClass) + fs_exec/fs_campaign code. Kernel, schema, spec, dataset, binary UNCHANGED.
```

## Resolution options (Abkarada decision — OPT-X alone cannot succeed)
```text
OPT-X2 (kernel instrumentation + schema field-definition amendment, then re-run) :
  (a) Instrument TraceChannel/SpineResult to RETAIN + expose the missing terminal data (per-event decision vectors,
      control_bytes) as ADDITIVE observation only — proven NOT to alter RNG draws / call-order / outcomes (instrumentation-
      invariance proof; kernel scientific behavior byte-equivalent). (b) A frozen SCHEMA-DEFINITION amendment must FIRST
      define the currently-undefined fields (representation of *_decisions/completion_or_censoring, the TAIL boundary, the
      completion_cause taxonomy, the *_lifecycle single-value + per-block aggregation, field types/nullability) — since the
      schema is silent and §3/§4 forbid me inventing them. Then serializer conformance + re-freeze (v1.3.2) + separate re-run GO.
      This is materially larger than "persistence-only OPT-X"; it touches the kernel and needs schema authorship.

OPT-Z (correct/redefine the frozen raw-schema to the kernel's ACTUAL rich terminal outputs, then re-run) :
  Re-issue FS-1-RAW-SCHEMA to a schema that (i) matches what run_spine/SpineResult genuinely exposes (which is rich:
  full timing lifecycle sub-fields, realized-loss vectors, dup hit/miss, structural_class, decision-log HASHES, etc.),
  (ii) replaces unimplementable per-event decision VECTORS with the retained decision-log HASHES, (iii) defines the tail
  boundary / completion_cause / lifecycle representations explicitly, and drops truly-uncomputed fields (control_bytes)
  or defines them. Governance schema-amendment (NOT a scientific-scope reduction of analysis; the analysis domains map
  onto the corrected schema). Then serializer conforms trivially, re-freeze v1.3.2, separate re-run GO. RECOMMENDED if the
  original 54-field schema was aspirational and never kernel-backed — it aligns contract with the real (rich) instrument.

NOT permitted: inventing representations/boundaries/values · silently accepting missing/extra fields · claiming 54/54 when it
  is 43/54 · changing the kernel under the "serializer-only" OPT-X label without an explicit science-layer authorization.
```

## STATUS
```text
DUR-B : SERIALIZER CONFORMANCE UNACHIEVABLE UNDER OPT-X (43/54 sourceable; 4 kernel-unavailable + 7 schema-underspecified).
        No code/kernel/schema/dataset change. Freeze chain intact. Awaiting Abkarada decision (OPT-X2 or OPT-Z). STOP.
```
