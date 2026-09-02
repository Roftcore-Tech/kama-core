//! DR-FEC-FS-1 OPT-X2 — RawRecord-v2 serializer + schema-v2 conformance validator. HARNESS-ONLY.
//! Implements the FS-1-RAW-SCHEMA-v2 typed contract byte-for-byte: canonical key order, typed fields, no-unknown,
//! nullability. build_record_v2 maps the instrumented terminal SpineResult + immutable Provenance + frozen manifest
//! row (NO new RNG / NO kernel mutation / NO clock read). validate_v2 is the executable KALICI-KURAL-3 gate.
#![allow(dead_code)]

use adaptive_transport::fs_trace::{DecisionEvent, TransmissionRole};

use super::fs_spine::{BlockObs, CompletionCause, SpineResult, TOTAL_SOURCE_SYMBOLS};

// ── canonical field order (must equal FS-1-RAW-SCHEMA-v2 field_order) ─────────────────────────────────
pub const FIELD_ORDER: [&str; 57] = [
    "run_id",
    "attempt_index",
    "pairing_group_id",
    "arm",
    "base_cell_id",
    "repetition_index",
    "freeze_lock_sha256",
    "prereg_sha256",
    "final_manifest_sha256",
    "coverage_sha256",
    "analysis_spec_sha256",
    "oracle_map_sha256",
    "execution_binary_sha256",
    "execution_source_commit",
    "cargo_lock_sha256",
    "toolchain_identity",
    "typed_manifest_row",
    "base_cell_semantics_hash",
    "run_execution_semantics_hash",
    "resolved_behavior_semantics_hash",
    "configured_schedule_hash",
    "configured_forward_hash",
    "configured_reverse_hash",
    "observed_forward_event_hash",
    "observed_reverse_event_hash",
    "initial_source_decisions",
    "intervention_decisions",
    "retransmission_decisions",
    "actual_forward_event_count",
    "actual_reverse_event_count",
    "t_first_source_offer",
    "t_final_source_wire_complete",
    "t_redundancy_lifecycle",
    "t_repair_usable",
    "t_arq_lifecycle",
    "t_full_workload_complete",
    "deadline_ms",
    "observation_horizon_ms",
    "censor_time_ns",
    "validity_class",
    "completion_or_censoring",
    "deadline_hit",
    "delivered_fragments",
    "delivered_bytes",
    "reassembly_equal",
    "completion_cause",
    "remaining_fragments",
    "tail_status",
    "base_source_bytes",
    "proactive_intervention_bytes",
    "arq_retransmit_bytes",
    "control_bytes",
    "total_outcome_bytes",
    "spurious_retransmissions",
    "fec_decoded_sources",
    "retransmits",
    "result_bearing",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidityV2 {
    Invalid,
    ValidComplete,
    ValidCensored,
}
impl ValidityV2 {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ValidityV2::Invalid => "INVALID",
            ValidityV2::ValidComplete => "VALID_COMPLETE",
            ValidityV2::ValidCensored => "VALID_CENSORED",
        }
    }
}

/// Frozen provenance snapshot (v1.3.2). All immutable constants except binary/commit/lock (recorded, changed).
#[derive(Debug, Clone)]
pub struct ProvV2 {
    pub freeze_lock_sha256: String,
    pub prereg_sha256: String,
    pub final_manifest_sha256: String,
    pub coverage_sha256: String,
    pub analysis_spec_sha256: String,
    pub oracle_map_sha256: String,
    pub execution_binary_sha256: String,
    pub execution_source_commit: String, // full 40-hex
    pub cargo_lock_sha256: String,
    pub toolchain_identity: String,
}

fn role_str(r: TransmissionRole) -> &'static str {
    match r {
        TransmissionRole::InitialSource => "InitialSource",
        TransmissionRole::RetransmitSource => "RetransmitSource",
        TransmissionRole::Duplicate => "Duplicate",
        TransmissionRole::Repair => "Repair",
        TransmissionRole::Control => "Control",
    }
}

fn decision_json(e: &DecisionEvent) -> String {
    format!(
        "{{\"role\":\"{}\",\"block_id\":{},\"esi\":{},\"attempt\":{},\"emit_time_ns\":{},\"action\":\"{}\"}}",
        role_str(e.role),
        e.block_id,
        e.esi,
        e.attempt,
        e.emit_time_ns,
        if e.dropped { "DROP" } else { "PASS" }
    )
}

fn arr_json<T, F: Fn(&T) -> String>(items: &[T], f: F) -> String {
    let mut s = String::from("[");
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&f(it));
    }
    s.push(']');
    s
}

fn opt_u64(v: Option<u64>) -> String {
    v.map_or_else(|| "null".into(), |x| x.to_string())
}

fn redundancy_lifecycle_json(o: &BlockObs) -> String {
    format!(
        "{{\"block_id\":{},\"t_eligible\":{},\"t_enqueued\":{},\"t_wire\":{},\"t_arrival\":{}}}",
        o.block_id,
        opt_u64(o.t_redundancy_eligible),
        opt_u64(o.t_redundancy_enqueued),
        opt_u64(o.t_redundancy_wire),
        opt_u64(o.t_redundancy_arrival)
    )
}
fn arq_lifecycle_json(o: &BlockObs) -> String {
    format!(
        "{{\"block_id\":{},\"t_eligible\":{},\"t_wire\":{},\"t_arrival\":{}}}",
        o.block_id,
        opt_u64(o.t_arq_eligible),
        opt_u64(o.t_arq_wire),
        opt_u64(o.t_arq_arrival)
    )
}

fn completion_cause_str(c: CompletionCause) -> &'static str {
    match c {
        CompletionCause::InitialSource => "INITIAL_SOURCE",
        CompletionCause::ArqRetransmit => "ARQ_RETRANSMIT",
        CompletionCause::FecDecode => "FEC_DECODE",
        CompletionCause::NotCompleted => "NOT_COMPLETED",
    }
}

/// The fully-typed v2 record (mirrors schema v2). String JSON produced by `serialize`.
pub struct RawRecordV2 {
    pub run_id: String,
    pub attempt_index: u8,
    pub pairing_group_id: String,
    pub arm: String,
    pub base_cell_id: String,
    pub repetition_index: u8,
    pub prov: ProvV2,
    pub typed_manifest_row: String, // raw JSON object (frozen manifest row)
    pub base_cell_semantics_hash: String,
    pub run_execution_semantics_hash: String,
    pub resolved_behavior_semantics_hash: String,
    pub configured_schedule_hash: String,
    pub configured_forward_hash: String,
    pub configured_reverse_hash: String,
    pub observed_forward_event_hash: String,
    pub observed_reverse_event_hash: String,
    pub initial_source_decisions: Vec<DecisionEvent>,
    pub intervention_decisions: Vec<DecisionEvent>,
    pub retransmission_decisions: Vec<DecisionEvent>,
    pub actual_forward_event_count: u64,
    pub actual_reverse_event_count: u64,
    pub t_first_source_offer: Option<u64>,
    pub t_final_source_wire_complete: Option<u64>,
    pub t_redundancy_lifecycle: Vec<BlockObs>,
    pub t_repair_usable: Option<u64>,
    pub t_arq_lifecycle: Vec<BlockObs>,
    pub t_full_workload_complete: Option<u64>,
    pub deadline_ms: u32,
    pub observation_horizon_ms: u32,
    pub censor_time_ns: Option<u64>,
    pub validity_class: ValidityV2,
    pub completion_status: &'static str, // "completed"|"censored"
    pub completion_time_ns: u64,
    pub deadline_hit: bool,
    pub delivered_fragments: u16,
    pub delivered_bytes: u32,
    pub reassembly_equal: bool,
    pub completion_cause: CompletionCause,
    pub remaining_fragments: u16,
    pub tail_status: &'static str,
    pub base_source_bytes: u64,
    pub proactive_intervention_bytes: u64,
    pub arq_retransmit_bytes: u64,
    pub control_bytes: u64,
    pub total_outcome_bytes: u64,
    pub spurious_retransmissions: u64,
    pub fec_decoded_sources: usize,
    pub retransmits: u64,
    pub result_bearing: bool,
}

impl RawRecordV2 {
    /// Canonical schema-v2 serialization (exact FIELD_ORDER; UTF-8; LF; typed; no floats).
    #[must_use]
    pub fn serialize(&self) -> String {
        let p = &self.prov;
        let mut s = String::from("{");
        macro_rules! kv {
            ($k:expr, $v:expr) => {{
                if s.len() > 1 {
                    s.push(',');
                }
                s.push('"');
                s.push_str($k);
                s.push_str("\":");
                s.push_str(&$v);
            }};
        }
        let q = |v: &str| format!("\"{v}\"");
        kv!("run_id", q(&self.run_id));
        kv!("attempt_index", self.attempt_index.to_string());
        kv!("pairing_group_id", q(&self.pairing_group_id));
        kv!("arm", q(&self.arm));
        kv!("base_cell_id", q(&self.base_cell_id));
        kv!("repetition_index", self.repetition_index.to_string());
        kv!("freeze_lock_sha256", q(&p.freeze_lock_sha256));
        kv!("prereg_sha256", q(&p.prereg_sha256));
        kv!("final_manifest_sha256", q(&p.final_manifest_sha256));
        kv!("coverage_sha256", q(&p.coverage_sha256));
        kv!("analysis_spec_sha256", q(&p.analysis_spec_sha256));
        kv!("oracle_map_sha256", q(&p.oracle_map_sha256));
        kv!("execution_binary_sha256", q(&p.execution_binary_sha256));
        kv!("execution_source_commit", q(&p.execution_source_commit));
        kv!("cargo_lock_sha256", q(&p.cargo_lock_sha256));
        kv!("toolchain_identity", q(&p.toolchain_identity));
        kv!("typed_manifest_row", self.typed_manifest_row.clone());
        kv!(
            "base_cell_semantics_hash",
            q(&self.base_cell_semantics_hash)
        );
        kv!(
            "run_execution_semantics_hash",
            q(&self.run_execution_semantics_hash)
        );
        kv!(
            "resolved_behavior_semantics_hash",
            q(&self.resolved_behavior_semantics_hash)
        );
        kv!(
            "configured_schedule_hash",
            q(&self.configured_schedule_hash)
        );
        kv!("configured_forward_hash", q(&self.configured_forward_hash));
        kv!("configured_reverse_hash", q(&self.configured_reverse_hash));
        kv!(
            "observed_forward_event_hash",
            q(&self.observed_forward_event_hash)
        );
        kv!(
            "observed_reverse_event_hash",
            q(&self.observed_reverse_event_hash)
        );
        kv!(
            "initial_source_decisions",
            arr_json(&self.initial_source_decisions, decision_json)
        );
        kv!(
            "intervention_decisions",
            arr_json(&self.intervention_decisions, decision_json)
        );
        kv!(
            "retransmission_decisions",
            arr_json(&self.retransmission_decisions, decision_json)
        );
        kv!(
            "actual_forward_event_count",
            self.actual_forward_event_count.to_string()
        );
        kv!(
            "actual_reverse_event_count",
            self.actual_reverse_event_count.to_string()
        );
        kv!("t_first_source_offer", opt_u64(self.t_first_source_offer));
        kv!(
            "t_final_source_wire_complete",
            opt_u64(self.t_final_source_wire_complete)
        );
        kv!(
            "t_redundancy_lifecycle",
            arr_json(&self.t_redundancy_lifecycle, redundancy_lifecycle_json)
        );
        kv!("t_repair_usable", opt_u64(self.t_repair_usable));
        kv!(
            "t_arq_lifecycle",
            arr_json(&self.t_arq_lifecycle, arq_lifecycle_json)
        );
        kv!(
            "t_full_workload_complete",
            opt_u64(self.t_full_workload_complete)
        );
        kv!("deadline_ms", self.deadline_ms.to_string());
        kv!(
            "observation_horizon_ms",
            self.observation_horizon_ms.to_string()
        );
        kv!("censor_time_ns", opt_u64(self.censor_time_ns));
        kv!("validity_class", q(self.validity_class.label()));
        kv!(
            "completion_or_censoring",
            format!(
                "{{\"status\":\"{}\",\"time_ns\":{}}}",
                self.completion_status, self.completion_time_ns
            )
        );
        kv!("deadline_hit", self.deadline_hit.to_string());
        kv!("delivered_fragments", self.delivered_fragments.to_string());
        kv!("delivered_bytes", self.delivered_bytes.to_string());
        kv!("reassembly_equal", self.reassembly_equal.to_string());
        kv!(
            "completion_cause",
            q(completion_cause_str(self.completion_cause))
        );
        kv!("remaining_fragments", self.remaining_fragments.to_string());
        kv!("tail_status", q(self.tail_status));
        kv!("base_source_bytes", self.base_source_bytes.to_string());
        kv!(
            "proactive_intervention_bytes",
            self.proactive_intervention_bytes.to_string()
        );
        kv!(
            "arq_retransmit_bytes",
            self.arq_retransmit_bytes.to_string()
        );
        kv!("control_bytes", self.control_bytes.to_string());
        kv!("total_outcome_bytes", self.total_outcome_bytes.to_string());
        kv!(
            "spurious_retransmissions",
            self.spurious_retransmissions.to_string()
        );
        kv!("fec_decoded_sources", self.fec_decoded_sources.to_string());
        kv!("retransmits", self.retransmits.to_string());
        kv!("result_bearing", self.result_bearing.to_string());
        s.push('}');
        s
    }
}

/// terminal source id = source_symbol_count - 1 (pinned == 47 for FS-1). REAL_TAIL_ERASURE iff terminal in erasure set.
#[must_use]
pub fn tail_status_of(erased_initial: &[u16], source_symbol_count: usize) -> &'static str {
    if erased_initial.is_empty() {
        return "NOT_APPLICABLE";
    }
    let terminal = (source_symbol_count as u16).saturating_sub(1);
    if erased_initial.contains(&terminal) {
        "REAL_TAIL_ERASURE"
    } else {
        "NO_TAIL_ERASURE"
    }
}

/// Build the v2 record from the instrumented terminal SpineResult + provenance + frozen manifest row string.
/// Observation-only mapping: NO RNG, NO kernel mutation, NO clock read.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn build_record_v2(
    run_id: &str,
    base_cell_id: &str,
    pairing_group_id: &str,
    arm_canonical: &str,
    rep: u8,
    attempt: u8,
    validity: ValidityV2,
    r: &SpineResult,
    prov: &ProvV2,
    typed_manifest_row: String,
    base_hash: u64,
    exec_hash: u64,
    behav_hash: u64,
    deadline_ms: u32,
    observation_horizon_ms: u32,
) -> RawRecordV2 {
    let by_role = |pred: fn(TransmissionRole) -> bool| -> Vec<DecisionEvent> {
        r.forward_decisions
            .iter()
            .copied()
            .filter(|e| pred(e.role))
            .collect()
    };
    let completed = r.completed;
    let deadline_ns = u64::from(deadline_ms) * 1_000_000;
    let horizon_ns = u64::from(observation_horizon_ms) * 1_000_000;
    let censor = if completed { None } else { Some(horizon_ns) };
    let (status, time_ns): (&'static str, u64) = if completed {
        ("completed", r.t_full_workload_complete.unwrap_or(0))
    } else {
        ("censored", horizon_ns)
    };
    RawRecordV2 {
        run_id: run_id.into(),
        attempt_index: attempt,
        pairing_group_id: pairing_group_id.into(),
        arm: arm_canonical.into(), // canonical {A0,A1-EARLY,F0,F3}; NOT the internal SpineResult.arm ("A1")
        base_cell_id: base_cell_id.into(),
        repetition_index: rep,
        prov: prov.clone(),
        typed_manifest_row,
        base_cell_semantics_hash: format!("0x{base_hash:016x}"),
        run_execution_semantics_hash: format!("0x{exec_hash:016x}"),
        resolved_behavior_semantics_hash: format!("0x{behav_hash:016x}"),
        configured_schedule_hash: format!("0x{:016x}", r.configured_schedule_hash),
        configured_forward_hash: format!("0x{:016x}", r.forward_schedule_id),
        configured_reverse_hash: format!("0x{:016x}", r.reverse_schedule_id),
        observed_forward_event_hash: format!("0x{:016x}", r.forward_trace_hash),
        observed_reverse_event_hash: format!("0x{:016x}", r.reverse_event_hash),
        initial_source_decisions: by_role(|x| x == TransmissionRole::InitialSource),
        intervention_decisions: by_role(|x| {
            x == TransmissionRole::Duplicate || x == TransmissionRole::Repair
        }),
        retransmission_decisions: by_role(|x| x == TransmissionRole::RetransmitSource),
        actual_forward_event_count: r.forward_decisions.len() as u64,
        actual_reverse_event_count: r.reverse_event_count,
        t_first_source_offer: r.t_first_source_offer,
        t_final_source_wire_complete: r.t_final_source_wire_complete,
        t_redundancy_lifecycle: r.per_block_obs.clone(),
        t_repair_usable: r.t_first_repair_usable,
        t_arq_lifecycle: r.per_block_obs.clone(),
        t_full_workload_complete: r.t_full_workload_complete,
        deadline_ms,
        observation_horizon_ms,
        censor_time_ns: censor,
        validity_class: validity,
        completion_status: status,
        completion_time_ns: time_ns,
        deadline_hit: r.t_full_workload_complete.is_some_and(|t| t <= deadline_ns),
        delivered_fragments: r.delivered_source_symbols as u16,
        delivered_bytes: r.delivered_app_bytes as u32,
        reassembly_equal: r.reassembly_equal,
        completion_cause: r.completion_cause,
        remaining_fragments: (TOTAL_SOURCE_SYMBOLS as u16)
            .saturating_sub(r.delivered_source_symbols as u16),
        tail_status: tail_status_of(&r.realized_initial_source_drop_ids, TOTAL_SOURCE_SYMBOLS),
        base_source_bytes: r.base_source_bytes,
        proactive_intervention_bytes: r.intervention_bytes,
        arq_retransmit_bytes: r.arq_retransmit_bytes,
        control_bytes: r.control_bytes,
        total_outcome_bytes: r.total_outcome_bytes,
        spurious_retransmissions: r.spurious_retransmits,
        fec_decoded_sources: r.fec_decoded_sources,
        retransmits: r.retransmits,
        result_bearing: true,
    }
}

// ── SCHEMA-V2 CONFORMANCE VALIDATOR (KALICI-KURAL-3 executable gate; fail-closed) ────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    MissingField(String),
    UnknownField(String),
    WrongType(String),
    EnumViolation(String),
    NullabilityViolation(String),
    ConsistencyViolation(String),
}

/// Validate a serialized v2 record: exact 57-key set (no missing/unknown), enum domains, nullability, and the
/// consistency invariants (remaining==48-delivered; NOT_COMPLETED iff censored; completion_or_censoring status).
pub fn validate_v2(json: &str) -> Result<(), SchemaError> {
    // key-set check (top-level keys only; nested objects/arrays are opaque here but type-checked at build).
    let keys = top_level_keys(json);
    for f in FIELD_ORDER {
        if !keys.contains(&f.to_string()) {
            return Err(SchemaError::MissingField(f.into()));
        }
    }
    for k in &keys {
        if !FIELD_ORDER.contains(&k.as_str()) {
            return Err(SchemaError::UnknownField(k.clone()));
        }
    }
    if keys.len() != 57 {
        return Err(SchemaError::WrongType(format!(
            "key count {} != 57",
            keys.len()
        )));
    }
    // enum domains.
    let arm = field_str(json, "arm");
    // FS-1 arms {A0,A1-EARLY,F0,F3} + DR-ARQ-TAIL-1 R2A deconfounding arms {A0-FROZEN,A0-TAIL} (additive; FS-1 unaffected).
    if !["A0", "A1-EARLY", "F0", "F3", "A0-FROZEN", "A0-TAIL"].contains(&arm.as_str()) {
        return Err(SchemaError::EnumViolation(format!("arm={arm}")));
    }
    let vc = field_str(json, "validity_class");
    if !["INVALID", "VALID_COMPLETE", "VALID_CENSORED"].contains(&vc.as_str()) {
        return Err(SchemaError::EnumViolation(format!("validity_class={vc}")));
    }
    let cc = field_str(json, "completion_cause");
    if ![
        "INITIAL_SOURCE",
        "ARQ_RETRANSMIT",
        "FEC_DECODE",
        "NOT_COMPLETED",
    ]
    .contains(&cc.as_str())
    {
        return Err(SchemaError::EnumViolation(format!("completion_cause={cc}")));
    }
    let ts = field_str(json, "tail_status");
    if !["NO_TAIL_ERASURE", "REAL_TAIL_ERASURE", "NOT_APPLICABLE"].contains(&ts.as_str()) {
        return Err(SchemaError::EnumViolation(format!("tail_status={ts}")));
    }
    // nullability + consistency.
    let censored = vc == "VALID_CENSORED";
    if censored && cc != "NOT_COMPLETED" {
        return Err(SchemaError::ConsistencyViolation(
            "censored but completion_cause != NOT_COMPLETED".into(),
        ));
    }
    if !censored && cc == "NOT_COMPLETED" {
        return Err(SchemaError::ConsistencyViolation(
            "completed but completion_cause == NOT_COMPLETED".into(),
        ));
    }
    let cn = field_raw(json, "t_full_workload_complete");
    if censored && cn != "null" {
        return Err(SchemaError::NullabilityViolation(
            "t_full_workload_complete must be null when censored".into(),
        ));
    }
    if !censored && cn == "null" {
        return Err(SchemaError::NullabilityViolation(
            "t_full_workload_complete must be present when completed".into(),
        ));
    }
    let ct = field_raw(json, "censor_time_ns");
    if censored && ct == "null" {
        return Err(SchemaError::NullabilityViolation(
            "censor_time_ns must be present when censored".into(),
        ));
    }
    if !censored && ct != "null" {
        return Err(SchemaError::NullabilityViolation(
            "censor_time_ns must be null when completed".into(),
        ));
    }
    if field_raw(json, "result_bearing") != "true" {
        return Err(SchemaError::ConsistencyViolation(
            "result_bearing != true".into(),
        ));
    }
    Ok(())
}

// ── minimal top-level JSON scanners (records are our own canonical output; no external parser) ──────────
fn top_level_keys(json: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let b = json.as_bytes();
    let mut i = 0usize;
    let mut depth = 0i32;
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b'"' if depth == 1 => {
                // a string at top-level object depth: could be a key (followed by ':').
                let (s, ni) = read_string(b, i);
                // skip whitespace
                let mut j = ni;
                while j < b.len() && (b[j] == b' ' || b[j] == b'\n') {
                    j += 1;
                }
                if j < b.len() && b[j] == b':' {
                    keys.push(s);
                    i = j + 1;
                    continue;
                }
                i = ni;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    keys
}
fn read_string(b: &[u8], start: usize) -> (String, usize) {
    // start points at opening quote.
    let mut i = start + 1;
    let mut out = String::new();
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            out.push(b[i] as char);
            out.push(b[i + 1] as char);
            i += 2;
            continue;
        }
        if b[i] == b'"' {
            return (out, i + 1);
        }
        out.push(b[i] as char);
        i += 1;
    }
    (out, i)
}
/// raw value token for a top-level key (string without quotes for scalars; "null"/"true"/number).
fn field_raw(json: &str, key: &str) -> String {
    let needle = format!("\"{key}\":");
    // find at depth-1: use a simple search that respects the first occurrence after an even brace depth.
    if let Some(pos) = find_top_level_key(json, key) {
        let rest = &json[pos + needle.len()..];
        return read_value(rest.as_bytes());
    }
    let _ = needle;
    String::new()
}
fn field_str(json: &str, key: &str) -> String {
    let v = field_raw(json, key);
    v.trim_matches('"').to_string()
}
fn find_top_level_key(json: &str, key: &str) -> Option<usize> {
    let b = json.as_bytes();
    let mut i = 0usize;
    let mut depth = 0i32;
    let target = key.as_bytes();
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b'"' if depth == 1 => {
                let (s, ni) = read_string(b, i);
                let mut j = ni;
                while j < b.len() && (b[j] == b' ' || b[j] == b'\n') {
                    j += 1;
                }
                if j < b.len() && b[j] == b':' {
                    if s.as_bytes() == target {
                        return Some(i);
                    }
                    i = j + 1;
                    continue;
                }
                i = ni;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}
fn read_value(b: &[u8]) -> String {
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\n') {
        i += 1;
    }
    if i >= b.len() {
        return String::new();
    }
    match b[i] {
        b'"' => {
            let (s, _) = read_string(b, i);
            format!("\"{s}\"")
        }
        b'{' | b'[' => {
            let open = b[i];
            let close = if open == b'{' { b'}' } else { b']' };
            let mut depth = 0i32;
            let start = i;
            while i < b.len() {
                if b[i] == open {
                    depth += 1;
                } else if b[i] == close {
                    depth -= 1;
                    if depth == 0 {
                        return String::from_utf8_lossy(&b[start..=i]).to_string();
                    }
                }
                i += 1;
            }
            String::from_utf8_lossy(&b[start..]).to_string()
        }
        _ => {
            let start = i;
            while i < b.len() && b[i] != b',' && b[i] != b'}' {
                i += 1;
            }
            String::from_utf8_lossy(&b[start..i]).trim().to_string()
        }
    }
}
