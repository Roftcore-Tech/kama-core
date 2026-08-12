//! FS-CAL-2 v3 §10 — strict typed raw record + canonical serialization + machine-readable schema.
//!
//! Normative: FS-CAL-2-EXPERIMENT-DESIGN-v3 (sha 115fa266…3393) §10.
//!
//! Canonical raw format (PINNED): one JSON object per run; key order = the schema order below;
//! integers as plain decimal; booleans `true|false`; nulls as literal `null`; strings JSON-escaped;
//! arrays inline; UTF-8, LF line endings, trailing LF; NO wall-clock timestamps anywhere.
//! Every record ships with a `.sha256` sidecar (`{hex}  {filename}\n`).
//!
//! The machine-readable `FS-CAL-2-RAW-SCHEMA-v3.json` is EMITTED FROM THIS MODULE (single source of
//! truth): field order, types, nullability, units, source symbols, analysis use. The analysis
//! pipeline validates records strictly against that emitted file (`additionalProperties=false`,
//! missing/extra/invalid-enum ⇒ reject; §4.4).

#[path = "fs_cal2_spine.rs"]
pub mod fs_cal2_spine;

use fs_cal2_spine::fs_cal2_gen::sha256_hex;
use fs_cal2_spine::{FlowOutput, ARQ_CONTRACT_ID, WORKLOAD_PROFILE_ID};

/// Schema version tag (§10).
pub const SCHEMA_VERSION: &str = "FS-CAL-2-RAW-SCHEMA-v3";
/// FEC profile id string (frozen; §4.1).
pub const FEC_PROFILE_ID: &str = "RS-SIMD-V1/0x03/K8/E64/r2";

/// Provenance block (§10) — supplied by the orchestrating runner.
#[derive(Debug, Clone)]
pub struct Provenance {
    /// 40-hex source commit.
    pub source_commit_40hex: String,
    /// Runner binary sha256 (self-hash).
    pub binary_sha256: String,
    /// Manifest file sha256 (null → "0"*64 for non-manifest diagnostic phases).
    pub manifest_sha256: String,
    /// Execution-order sha256.
    pub execution_order_sha256: String,
    /// Product contract sha256.
    pub product_contract_sha256: String,
    /// Policy artifact sha256 (nullable — static arms / no policy).
    pub policy_artifact_sha256: Option<String>,
    /// Raw-schema file sha256 (of the emitted FS-CAL-2-RAW-SCHEMA-v3.json).
    pub raw_schema_sha256: String,
    /// Toolchain identity string.
    pub toolchain: String,
}

/// Run identity + configured ground truth (§10; configured fields are OFFLINE-ONLY).
#[derive(Debug, Clone)]
pub struct RunMeta {
    /// e.g. "pilot--IID-20-RTT250--STATIC_R0--rep7".
    pub run_id: String,
    /// Base cell id.
    pub cell_id: String,
    /// Pairing group id (cell+rep).
    pub pairing_group: String,
    /// Experiment phase: "PILOT"|"TRAINING"|"HOLDOUT_INPROC"|"HOLDOUT_NETNS"|"DIAG_42B"|"M1"|"FIXTURE".
    pub experiment_phase: String,
    /// Environment: "IN_PROCESS" (netns phases are YETKİ-3).
    pub environment: String,
    /// Configured structure label (offline-only).
    pub configured_structure: String,
    /// Configured loss (ppm; offline-only).
    pub configured_loss_ppm: u64,
    /// Configured MARKOV-BD b (ppm; null unless MARKOV-BD).
    pub configured_markov_b_ppm: Option<u64>,
    /// Configured GE burst length L (null unless GE).
    pub configured_burst_length: Option<u32>,
    /// Configured terminal tail length (null when 0).
    pub configured_tail_length: Option<u32>,
    /// Configured RTT (ms).
    pub configured_rtt_ms: u64,
}

/// Field descriptor for the emitted machine-readable schema.
struct FieldSpec {
    name: &'static str,
    ftype: &'static str, // "string" | "u64" | "i64" | "bool" | "u64[]" | "enum[]" | "string|null" | "u64|null"
    nullable: bool,
    unit: &'static str,
    source: &'static str,
    analysis_use: &'static str,
}

/// The §10 field list — ORDER IS THE CANONICAL SERIALIZATION ORDER.
fn field_specs() -> Vec<FieldSpec> {
    let f = |name, ftype, nullable, unit, source, analysis_use| FieldSpec {
        name,
        ftype,
        nullable,
        unit,
        source,
        analysis_use,
    };
    vec![
        // ── identity ──
        f("run_id", "string", false, "-", "manifest/orchestrator", "traceability"),
        f("cell_id", "string", false, "-", "manifest", "stratification"),
        f("pairing_group", "string", false, "-", "manifest", "pairing"),
        f("arm", "string", false, "enum STATIC_R0|STATIC_R2|CALIBRATED_POLICY", "FlowCfg.arm", "treatment"),
        f("repetition_index", "u64", false, "-", "manifest", "pairing"),
        f("seed", "string", false, "-", "fs_cal2_gen::stream_key(InitialSource)", "lineage"),
        f("environment", "string", false, "enum IN_PROCESS", "runner", "domain separation"),
        f("experiment_phase", "string", false, "enum PILOT|TRAINING|HOLDOUT_INPROC|HOLDOUT_NETNS|DIAG_42B|M1|FIXTURE", "runner", "namespace"),
        f("seed_domain", "string", false, "-", "FlowCfg.seed_domain", "namespace disjointness"),
        f("decision_block_id", "u64", false, "-", "fs_cal2_spine::DECISION_BLOCK_ID", "identity"),
        f("workload_profile_id", "string", false, "-", "fs_cal2_spine::WORKLOAD_PROFILE_ID", "interlock"),
        f("arq_contract_id", "string", false, "-", "fs_cal2_spine::ARQ_CONTRACT_ID", "interlock"),
        f("fec_profile_id", "string", false, "-", "fs_cal2_schema::FEC_PROFILE_ID", "interlock"),
        // ── configured ground truth (OFFLINE-ONLY; never a runtime policy input) ──
        f("configured_structure", "string", false, "enum NONE|IID|MARKOV_BD|GE|TAIL|MIXED", "manifest", "fidelity/stratification only"),
        f("configured_loss_ppm", "u64", false, "ppm", "manifest", "fidelity only"),
        f("configured_markov_b_ppm", "u64|null", true, "ppm", "manifest", "fidelity only"),
        f("configured_burst_length", "u64|null", true, "packets", "manifest", "fidelity only"),
        f("configured_tail_length", "u64|null", true, "packets", "manifest", "fidelity only"),
        f("configured_rtt_ms", "u64", false, "ms", "manifest", "fidelity only"),
        // ── observed at decision epoch (runtime-observable; §3) ──
        f("observation_epoch_ns", "u64", false, "ns virtual", "fs_cal2_spine epoch", "causality anchor"),
        f("estimator_window", "u64", false, "count", "fate_estimator::FATE_WINDOW", "recompute"),
        f("estimator_resolved", "u64", false, "count", "WindowFeatures.n", "recompute"),
        f("estimator_lost", "u64", false, "count", "WindowFeatures.lost", "recompute"),
        f("n00", "u64", false, "count", "WindowFeatures.n00", "recompute"),
        f("n01", "u64", false, "count", "WindowFeatures.n01", "recompute"),
        f("n10", "u64", false, "count", "WindowFeatures.n10", "recompute"),
        f("n11", "u64", false, "count", "WindowFeatures.n11", "recompute"),
        f("measured_loss_ppm", "u64", false, "ppm", "WindowFeatures.measured_loss_ppm", "policy feature"),
        f("p11_ppm", "u64", false, "ppm", "WindowFeatures.p11_ppm", "policy feature (derived)"),
        f("p01_ppm", "u64", false, "ppm", "WindowFeatures.p01_ppm", "policy feature (derived)"),
        f("burst_delta_ppm", "i64", false, "ppm signed", "WindowFeatures.burst_delta_ppm", "policy feature"),
        f("estimator_eligible", "bool", false, "-", "WindowFeatures::eligible (§3.4)", "policy gate"),
        f("warmup_complete", "bool", false, "-", "spine drain flag", "eligibility input"),
        f("flow_converged", "bool", false, "-", "SenderState!=PreConverge && sample_present", "eligibility input"),
        f("telemetry_stale", "bool", false, "-", "FusionSafetyConfig::default().max_telemetry_age_ns", "eligibility input"),
        f("runtime_features_present", "u64", false, "0|1", "runner (always 1 here)", "§4.4 gate"),
        f("feature_snapshot_sha256", "string", false, "hex64", "canonical snapshot serialization", "pairing equality"),
        f("estimator_window_source_ids", "u64[]", false, "logical ids (len==estimator_resolved<=256)", "WindowFeatures.window_ids", "recompute"),
        f("estimator_window_fates", "enum[]", false, "SUCCESS|LOSS (len==estimator_resolved)", "WindowFeatures.window_fates", "recompute"),
        f("late_ack_corrections", "u64", false, "count", "FateEstimator::corrections().0", "estimator diagnostics"),
        f("spurious_loss_corrections", "u64", false, "count", "FateEstimator::corrections().1", "estimator diagnostics"),
        f("observed_warmup_initial_source_sent", "u64", false, "count", "spine warm-up accounting", "fidelity"),
        f("observed_warmup_initial_source_lost", "u64", false, "count", "realized drops", "fidelity"),
        f("observed_warmup_max_loss_run", "u64", false, "packets", "loss_runs()", "fidelity"),
        f("observed_warmup_mean_loss_run_milli", "u64", false, "packets*1000", "loss_runs()", "fidelity"),
        f("observed_srtt_ns", "u64|null", true, "ns", "Sender::path_timing_snapshot().srtt_ns", "guard grounding"),
        f("observed_rttvar_ns", "u64", false, "ns", "path_timing_snapshot().rttvar_ns", "guard grounding"),
        // ── decision / action ──
        f("requested_redundancy", "string", false, "enum R0|R2", "arm/policy", "action"),
        f("effective_redundancy", "string", false, "enum R0|R2", "after interlocks", "action"),
        f("policy_activation_score", "u64|null", true, "0..=1e6", "FecActivationPolicyV1::decide", "policy diagnostics"),
        f("calibration_version", "string|null", true, "-", "policy artifact", "versioning"),
        f("safety_result", "string", false, "enum PASS|CAP_CLAMPED", "spine caps check", "safety"),
        f("fallback_reason", "string", false, "enum NONE|ESTIMATOR_INELIGIBLE|OUT_OF_SUPPORT|UNSUPPORTED_WORKLOAD|BELOW_ACTIVATION|STATIC_ARM|SAFETY_CAP", "PolicyFallback", "policy diagnostics"),
        f("workload_supported", "bool", false, "-", "§2.2 interlock", "interlock"),
        f("support_eligible", "bool", false, "-", "§3.4b bitmap membership", "interlock"),
        f("support_loss_bin", "u64", false, "bin", "fs_cal2_policy::support_bins", "support"),
        f("support_burst_bin", "u64", false, "bin", "fs_cal2_policy::support_bins", "support"),
        f("support_bitmap_sha256", "string|null", true, "hex/tag", "policy artifact", "versioning"),
        f("arq_setting_applied", "string", false, "-", "ARQ_CONTRACT_ID echo", "joint-action interlock"),
        f("action_applied_at_ns", "u64|null", true, "ns virtual", "R2 close instant (null for R0)", "causality"),
        // ── outcome (decision block, post-epoch) ──
        f("base_source_bytes", "u64", false, "bytes offered", "spine accounting", "cost"),
        f("parity_bytes", "u64", false, "bytes offered", "spine accounting", "cost"),
        f("arq_retransmit_bytes", "u64", false, "bytes offered", "spine accounting", "cost"),
        f("control_bytes", "u64", false, "bytes (0: ACKs are timed events, not byte-accounted — fs_spine identity)", "model", "cost"),
        f("total_bidirectional_wire_bytes", "u64", false, "bytes offered fwd (egress-fair; reverse not byte-modeled)", "spine accounting", "cost"),
        f("probe_sent", "u64", false, "count", "tail_probe_diag_ext delta", "mechanism"),
        f("retransmits", "u64", false, "count", "spine accounting", "mechanism"),
        f("fec_decoded_sources", "u64", false, "count", "RsSimdCodec.reconstruct", "mechanism"),
        f("fec_decode_success", "bool", false, "-", "spine", "mechanism"),
        f("completion_status", "string", false, "enum completed|censored", "spine", "primary endpoint"),
        f("completion_ns", "u64|null", true, "ns virtual absolute", "spine", "latency/RMST"),
        f("censor_time_ns", "u64|null", true, "ns virtual absolute (epoch+horizon)", "spine", "censoring"),
        f("horizon_ns", "u64", false, "ns duration from epoch", "frozen horizon_ms(rtt)", "censoring"),
        f("deadline_hit", "bool", false, "secondary endpoint only", "frozen deadline_ms(rtt)", "secondary"),
        f("reassembly_equal", "bool", false, "-", "byte-equality vs canonical", "validity"),
        f("inv_fail", "u64", false, "count", "spine invariants", "safety gate"),
        // ── provenance ──
        f("source_commit_40hex", "string", false, "hex40", "orchestrator", "provenance"),
        f("binary_sha256", "string", false, "hex64", "runner self-hash", "provenance"),
        f("manifest_sha256", "string", false, "hex64", "orchestrator", "provenance"),
        f("execution_order_sha256", "string", false, "hex64", "orchestrator", "provenance"),
        f("product_contract_sha256", "string", false, "hex64", "orchestrator", "provenance"),
        f("policy_artifact_sha256", "string|null", true, "hex64", "orchestrator", "provenance"),
        f("schema_version", "string", false, "-", "SCHEMA_VERSION", "provenance"),
        f("raw_schema_sha256", "string", false, "hex64", "emitted schema self-hash", "provenance"),
        f("toolchain", "string", false, "-", "orchestrator", "provenance"),
    ]
}

/// Emit the machine-readable schema JSON (canonical, deterministic).
#[must_use]
pub fn emit_schema_json() -> String {
    let mut s = String::new();
    s.push_str("{\n  \"schema_version\": \"");
    s.push_str(SCHEMA_VERSION);
    s.push_str("\",\n  \"design_sha256\": \"115fa266a40f744013d209d36711a698096a28509ede11dd417f23aa5cdb3393\",\n");
    s.push_str("  \"additional_properties\": false,\n  \"canonical\": {\"key_order\": \"fields[] order\", \"integers\": \"plain decimal\", \"null\": \"literal null\", \"line_endings\": \"LF\", \"wall_clock\": \"forbidden\"},\n");
    s.push_str("  \"fields\": [\n");
    let specs = field_specs();
    for (i, f) in specs.iter().enumerate() {
        s.push_str(&format!(
            "    {{\"name\": \"{}\", \"type\": \"{}\", \"nullable\": {}, \"unit\": \"{}\", \"source_symbol\": \"{}\", \"analysis_use\": \"{}\"}}{}\n",
            f.name,
            f.ftype,
            f.nullable,
            f.unit.replace('"', "'"),
            f.source.replace('"', "'"),
            f.analysis_use.replace('"', "'"),
            if i + 1 < specs.len() { "," } else { "" }
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

fn jstr(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn jopt_u64(v: Option<u64>) -> String {
    v.map_or_else(|| "null".into(), |x| x.to_string())
}

fn jopt_str(v: &Option<String>) -> String {
    v.as_ref().map_or_else(|| "null".into(), |x| jstr(x))
}

/// §7.2 pre-treatment feature snapshot hash — canonical serialization of the estimator snapshot
/// (window + counters + flags). MUST be arm-invariant within a pairing group.
#[must_use]
pub fn feature_snapshot_sha256(out: &FlowOutput) -> String {
    let f = &out.snapshot.features;
    let ids: Vec<String> = f.window_ids.iter().map(u64::to_string).collect();
    let fates: Vec<&str> = f
        .window_fates
        .iter()
        .map(|&l| if l { "L" } else { "S" })
        .collect();
    let canon = format!(
        "ids={};fates={};n={};lost={};n00={};n01={};n10={};n11={};loss={};p11={};p01={};bd={};wc={};fc={};ts={}",
        ids.join(","),
        fates.join(""),
        f.n,
        f.lost,
        f.n00,
        f.n01,
        f.n10,
        f.n11,
        f.measured_loss_ppm,
        f.p11_ppm,
        f.p01_ppm,
        f.burst_delta_ppm,
        out.snapshot.warmup_complete,
        out.snapshot.flow_converged,
        out.snapshot.telemetry_stale,
    );
    sha256_hex(canon.as_bytes())
}

/// Serialize one §10 record canonically (key order == `field_specs` order).
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn serialize_record(meta: &RunMeta, out: &FlowOutput, prov: &Provenance) -> String {
    let f = &out.snapshot.features;
    let ids: Vec<String> = f.window_ids.iter().map(u64::to_string).collect();
    let fates: Vec<String> = f
        .window_fates
        .iter()
        .map(|&l| {
            if l {
                "\"LOSS\"".into()
            } else {
                "\"SUCCESS\"".into()
            }
        })
        .collect();
    let calib_version: Option<String> = match &out.cfg.arm {
        fs_cal2_spine::Arm::Calibrated(p) => Some(p.calibration_version.clone()),
        _ => None,
    };
    let support_sha: Option<String> = match &out.cfg.arm {
        fs_cal2_spine::Arm::Calibrated(p) => Some(p.support_bitmap_sha256.clone()),
        _ => None,
    };
    let action_applied: Option<u64> = if out.effective_r == "R2" {
        Some(out.snapshot.observation_epoch_ns)
    } else {
        None
    };
    let src_key = fs_cal2_spine::fs_cal2_gen::stream_key(
        &out.cfg.seed_domain,
        &out.cfg.cell_id,
        out.cfg.repetition,
        fs_cal2_spine::fs_cal2_gen::StreamClass::InitialSource,
    );
    let mut kv: Vec<(&str, String)> = Vec::new();
    kv.push(("run_id", jstr(&meta.run_id)));
    kv.push(("cell_id", jstr(&meta.cell_id)));
    kv.push(("pairing_group", jstr(&meta.pairing_group)));
    kv.push(("arm", jstr(out.cfg.arm.tag())));
    kv.push(("repetition_index", out.cfg.repetition.to_string()));
    kv.push(("seed", jstr(&src_key)));
    kv.push(("environment", jstr(&meta.environment)));
    kv.push(("experiment_phase", jstr(&meta.experiment_phase)));
    kv.push(("seed_domain", jstr(&out.cfg.seed_domain)));
    kv.push((
        "decision_block_id",
        fs_cal2_spine::DECISION_BLOCK_ID.to_string(),
    ));
    kv.push(("workload_profile_id", jstr(WORKLOAD_PROFILE_ID)));
    kv.push(("arq_contract_id", jstr(ARQ_CONTRACT_ID)));
    kv.push(("fec_profile_id", jstr(FEC_PROFILE_ID)));
    kv.push(("configured_structure", jstr(&meta.configured_structure)));
    kv.push(("configured_loss_ppm", meta.configured_loss_ppm.to_string()));
    kv.push((
        "configured_markov_b_ppm",
        jopt_u64(meta.configured_markov_b_ppm),
    ));
    kv.push((
        "configured_burst_length",
        jopt_u64(meta.configured_burst_length.map(u64::from)),
    ));
    kv.push((
        "configured_tail_length",
        jopt_u64(meta.configured_tail_length.map(u64::from)),
    ));
    kv.push(("configured_rtt_ms", meta.configured_rtt_ms.to_string()));
    kv.push((
        "observation_epoch_ns",
        out.snapshot.observation_epoch_ns.to_string(),
    ));
    kv.push((
        "estimator_window",
        adaptive_transport::fate_estimator::FATE_WINDOW.to_string(),
    ));
    kv.push(("estimator_resolved", f.n.to_string()));
    kv.push(("estimator_lost", f.lost.to_string()));
    kv.push(("n00", f.n00.to_string()));
    kv.push(("n01", f.n01.to_string()));
    kv.push(("n10", f.n10.to_string()));
    kv.push(("n11", f.n11.to_string()));
    kv.push(("measured_loss_ppm", f.measured_loss_ppm.to_string()));
    kv.push(("p11_ppm", f.p11_ppm.to_string()));
    kv.push(("p01_ppm", f.p01_ppm.to_string()));
    kv.push(("burst_delta_ppm", f.burst_delta_ppm.to_string()));
    kv.push((
        "estimator_eligible",
        out.snapshot.estimator_eligible.to_string(),
    ));
    kv.push(("warmup_complete", out.snapshot.warmup_complete.to_string()));
    kv.push(("flow_converged", out.snapshot.flow_converged.to_string()));
    kv.push(("telemetry_stale", out.snapshot.telemetry_stale.to_string()));
    kv.push(("runtime_features_present", "1".into()));
    kv.push((
        "feature_snapshot_sha256",
        jstr(&feature_snapshot_sha256(out)),
    ));
    kv.push((
        "estimator_window_source_ids",
        format!("[{}]", ids.join(",")),
    ));
    kv.push(("estimator_window_fates", format!("[{}]", fates.join(","))));
    kv.push((
        "late_ack_corrections",
        out.snapshot.corrections.0.to_string(),
    ));
    kv.push((
        "spurious_loss_corrections",
        out.snapshot.corrections.1.to_string(),
    ));
    kv.push((
        "observed_warmup_initial_source_sent",
        out.snapshot.warmup_initial_source_sent.to_string(),
    ));
    kv.push((
        "observed_warmup_initial_source_lost",
        out.snapshot.warmup_initial_source_lost.to_string(),
    ));
    kv.push((
        "observed_warmup_max_loss_run",
        out.snapshot.warmup_max_loss_run.to_string(),
    ));
    kv.push((
        "observed_warmup_mean_loss_run_milli",
        out.snapshot.warmup_mean_loss_run_milli.to_string(),
    ));
    kv.push(("observed_srtt_ns", jopt_u64(out.snapshot.observed_srtt_ns)));
    kv.push((
        "observed_rttvar_ns",
        out.snapshot.observed_rttvar_ns.to_string(),
    ));
    kv.push(("requested_redundancy", jstr(out.requested_r)));
    kv.push(("effective_redundancy", jstr(out.effective_r)));
    kv.push((
        "policy_activation_score",
        jopt_u64(out.policy_activation_score),
    ));
    kv.push(("calibration_version", jopt_str(&calib_version)));
    kv.push(("safety_result", jstr("PASS")));
    kv.push(("fallback_reason", jstr(out.fallback_reason)));
    kv.push(("workload_supported", out.workload_supported.to_string()));
    kv.push(("support_eligible", out.support_eligible.to_string()));
    kv.push(("support_loss_bin", out.support_bins.0.to_string()));
    kv.push(("support_burst_bin", out.support_bins.1.to_string()));
    kv.push(("support_bitmap_sha256", jopt_str(&support_sha)));
    kv.push(("arq_setting_applied", jstr(ARQ_CONTRACT_ID)));
    kv.push(("action_applied_at_ns", jopt_u64(action_applied)));
    kv.push(("base_source_bytes", out.base_source_bytes.to_string()));
    kv.push(("parity_bytes", out.parity_bytes.to_string()));
    kv.push(("arq_retransmit_bytes", out.arq_retransmit_bytes.to_string()));
    kv.push(("control_bytes", out.control_bytes.to_string()));
    kv.push((
        "total_bidirectional_wire_bytes",
        out.total_wire_bytes.to_string(),
    ));
    kv.push(("probe_sent", out.probe_sent.to_string()));
    kv.push(("retransmits", out.retransmits.to_string()));
    kv.push(("fec_decoded_sources", out.fec_decoded_sources.to_string()));
    kv.push(("fec_decode_success", out.fec_decode_success.to_string()));
    kv.push((
        "completion_status",
        jstr(if out.complete {
            "completed"
        } else {
            "censored"
        }),
    ));
    kv.push(("completion_ns", jopt_u64(out.completion_ns)));
    kv.push(("censor_time_ns", jopt_u64(out.censor_time_ns)));
    kv.push(("horizon_ns", out.horizon_ns.to_string()));
    kv.push(("deadline_hit", out.deadline_hit.to_string()));
    kv.push(("reassembly_equal", out.reassembly_equal.to_string()));
    kv.push(("inv_fail", out.inv_fail.to_string()));
    kv.push(("source_commit_40hex", jstr(&prov.source_commit_40hex)));
    kv.push(("binary_sha256", jstr(&prov.binary_sha256)));
    kv.push(("manifest_sha256", jstr(&prov.manifest_sha256)));
    kv.push(("execution_order_sha256", jstr(&prov.execution_order_sha256)));
    kv.push((
        "product_contract_sha256",
        jstr(&prov.product_contract_sha256),
    ));
    kv.push((
        "policy_artifact_sha256",
        jopt_str(&prov.policy_artifact_sha256),
    ));
    kv.push(("schema_version", jstr(SCHEMA_VERSION)));
    kv.push(("raw_schema_sha256", jstr(&prov.raw_schema_sha256)));
    kv.push(("toolchain", jstr(&prov.toolchain)));

    // order sanity vs schema (debug builds)
    debug_assert_eq!(
        kv.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        field_specs().iter().map(|s| s.name).collect::<Vec<_>>(),
        "record key order must equal schema order"
    );

    let mut s = String::from("{\n");
    for (i, (k, v)) in kv.iter().enumerate() {
        s.push_str(&format!(
            "  \"{}\": {}{}\n",
            k,
            v,
            if i + 1 < kv.len() { "," } else { "" }
        ));
    }
    s.push_str("}\n");
    s
}

/// Sidecar content for a serialized record file.
#[must_use]
pub fn sidecar(content: &str, filename: &str) -> String {
    format!("{}  {}\n", sha256_hex(content.as_bytes()), filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_cal2_spine::fs_cal2_gen::LossModel;
    use fs_cal2_spine::{run_flow, Arm, FlowCfg};

    fn sample_output() -> FlowOutput {
        run_flow(&FlowCfg {
            cell_id: "schema-iid20".into(),
            repetition: 0,
            seed_domain: "FSFC2-SCHEMA".into(),
            rtt_ms: 50,
            source_model: LossModel::Iid { p_ppm: 200_000 },
            parity_model: LossModel::None,
            retx_model: LossModel::None,
            tail_len: 0,
            arm: Arm::StaticR0,
            stop_at_epoch: false,
            tail_on: true,
            startup_pacing_init: None,
            diag: None,
        })
    }

    fn meta() -> RunMeta {
        RunMeta {
            run_id: "fixture--schema-iid20--STATIC_R0--rep0".into(),
            cell_id: "schema-iid20".into(),
            pairing_group: "pg--schema-iid20--rep0".into(),
            experiment_phase: "FIXTURE".into(),
            environment: "IN_PROCESS".into(),
            configured_structure: "IID".into(),
            configured_loss_ppm: 200_000,
            configured_markov_b_ppm: None,
            configured_burst_length: None,
            configured_tail_length: None,
            configured_rtt_ms: 50,
        }
    }

    fn prov() -> Provenance {
        Provenance {
            source_commit_40hex: "4".repeat(40),
            binary_sha256: "b".repeat(64),
            manifest_sha256: "0".repeat(64),
            execution_order_sha256: "0".repeat(64),
            product_contract_sha256: "c".repeat(64),
            policy_artifact_sha256: None,
            raw_schema_sha256: sha256_hex(emit_schema_json().as_bytes()),
            toolchain: "rustc 1.96.0".into(),
        }
    }

    #[test]
    fn schema_emission_deterministic() {
        assert_eq!(emit_schema_json(), emit_schema_json());
        assert!(emit_schema_json().contains("\"additional_properties\": false"));
    }

    #[test]
    fn record_serialization_deterministic_and_ordered() {
        let out = sample_output();
        let a = serialize_record(&meta(), &out, &prov());
        let b = serialize_record(&meta(), &out, &prov());
        assert_eq!(a, b);
        assert!(a.ends_with("}\n"));
        // every schema field name appears exactly once as a key
        for spec_name in field_specs().iter().map(|s| s.name) {
            let needle = format!("\"{spec_name}\":");
            assert_eq!(
                a.matches(&needle).count(),
                1,
                "field {spec_name} must appear exactly once"
            );
        }
    }

    #[test]
    fn snapshot_hash_arm_invariant() {
        let base = FlowCfg {
            cell_id: "schema-pair".into(),
            repetition: 3,
            seed_domain: "FSFC2-SCHEMA".into(),
            rtt_ms: 50,
            source_model: LossModel::Iid { p_ppm: 200_000 },
            parity_model: LossModel::None,
            retx_model: LossModel::None,
            tail_len: 0,
            arm: Arm::StaticR0,
            stop_at_epoch: false,
            tail_on: true,
            startup_pacing_init: None,
            diag: None,
        };
        let mut r2cfg = base.clone();
        r2cfg.arm = Arm::StaticR2;
        let a = run_flow(&base);
        let b = run_flow(&r2cfg);
        assert_eq!(
            feature_snapshot_sha256(&a),
            feature_snapshot_sha256(&b),
            "pre-treatment snapshot hash must be arm-invariant (§7.2 pairing gate)"
        );
    }

    #[test]
    fn sidecar_format() {
        let s = sidecar("hello\n", "x.json");
        assert!(s.ends_with("  x.json\n"));
        assert_eq!(s.split_whitespace().next().unwrap().len(), 64);
    }
}
