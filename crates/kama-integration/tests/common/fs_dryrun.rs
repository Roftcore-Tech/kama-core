//! DR-FEC-FS-1 S4-C — NON-RESULT executability dry-run (§19-22). Her manifest-satırı için RunCfg/workload/
//! source-schedule/arm/K-profile/forward-trace/reverse-trace/output KONSTRÜKTE-EDİLİR ve doğrulanır — AMA
//! channel event-loop İLERLETİLMEZ, campaign-paket admit/drop-EDİLMEZ, recovery/decode/completion/deadline-hit
//! HESAPLANMAZ, raw-result YAZILMAZ (result-bearing-firewall). Çıktı = executability-metadata (result_bearing=false).
#![allow(dead_code)]

#[path = "fs_manifest.rs"]
mod fs_manifest;
pub use fs_manifest::*;

use adaptive_transport::fs_trace::{
    RedundancyPolicy, ReverseSpec, TraceChannel, TracePacketId, TraceSpec,
};
use kama_fec_codec::by_id;
use std::collections::{BTreeMap, BTreeSet};

pub const DRYRUN_MODE: &str = "EXECUTABILITY_ONLY";

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowStatus {
    Dispatchable,
    Unsupported(String),
    Ambiguous(String),
    Invalid(String),
    Duplicate(String),
}
impl RowStatus {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            RowStatus::Dispatchable => "DISPATCHABLE",
            RowStatus::Unsupported(_) => "UNSUPPORTED",
            RowStatus::Ambiguous(_) => "AMBIGUOUS",
            RowStatus::Invalid(_) => "INVALID",
            RowStatus::Duplicate(_) => "DUPLICATE",
        }
    }
    #[must_use]
    pub fn is_dispatchable(&self) -> bool {
        matches!(self, RowStatus::Dispatchable)
    }
}

/// **§21 per-row executability.** GERÇEK obje-konstrüksiyonu (TraceChannel + codec by_id); loop-İLERLETMEZ.
#[must_use]
pub fn executability_status(c: &BaseCell, arm: &str, rep: u8) -> RowStatus {
    // arm dispatch (OPT-A set).
    if !FS1_RESULT_BEARING_ARMS.contains(&arm) {
        return RowStatus::Unsupported(format!("arm {arm} ∉ OPT-A result-bearing set"));
    }
    let ar = resolve_arm(arm);
    let h = row_hashes(c, arm, rep);
    // typed impairment construct (round-trip; invalid-combo temsil-edilemez).
    let parsed = match c.impairment {
        FsImpairment::ExactDeficit { drop_count } => {
            parse_impairment("ExactDeficit", Some(drop_count.get()), None)
        }
        FsImpairment::NaturalIid { loss_permille } => {
            parse_impairment("NaturalIid", None, Some(loss_permille.get()))
        }
    };
    if parsed.is_err() {
        return RowStatus::Invalid(format!("impairment-parse: {parsed:?}"));
    }
    // workload construct (48-symbol canonical).
    if canonical_sequence_hash() == 0 {
        return RowStatus::Invalid("workload-construct".into());
    }
    // forward + reverse trace construct (GERÇEK TraceChannel; loop-YOK).
    let fwd = match c.impairment {
        FsImpairment::ExactDeficit { .. } => TraceSpec::ExactDeficit {
            drop_initial: h.drop_ids.iter().map(|&g| (0u32, g)).collect(),
            redundancy: RedundancyPolicy::PassAll,
        },
        FsImpairment::NaturalIid { loss_permille } => TraceSpec::NaturalIid {
            loss_permille: u32::from(loss_permille.get()),
            seed: h.trace_seed,
        },
    };
    let ch = TraceChannel::with_reverse(fwd, ReverseSpec::Clear);
    // forward/reverse configured-identity ayrık (domain-separation).
    if ch.forward_schedule_id() == ch.reverse_schedule_id() {
        return RowStatus::Ambiguous("forward==reverse schedule-id".into());
    }
    // K/profile availability + block divisibility + repair config (FEC arms).
    if let Some(pid) = ar.selected_profile_id {
        if by_id(pid).is_none() {
            return RowStatus::Unsupported(format!("codec profile 0x{pid:02x} unavailable"));
        }
        let k = ar.selected_k.unwrap_or(0);
        if k == 0 || !FS1_SOURCE_SYMBOLS.is_multiple_of(k) {
            return RowStatus::Invalid(format!("48 % k={k} != 0"));
        }
        let blocks = FS1_SOURCE_SYMBOLS / k;
        if ar.source_block_count != Some(blocks) {
            return RowStatus::Invalid(format!("block-count != 48/{k}"));
        }
        if ar.repair_count != u32::from(blocks) * u32::from(ar.configured_r) {
            return RowStatus::Invalid("repair-count != blocks*r".into());
        }
    }
    // deadline present.
    if h.deadline == 0 {
        return RowStatus::Invalid("deadline=0".into());
    }
    // ExactDeficit drop-set validity.
    if let FsImpairment::ExactDeficit { drop_count } = c.impairment {
        if h.drop_ids.len() != drop_count.get() as usize {
            return RowStatus::Invalid("drop-set-size != drop_count".into());
        }
        if h.drop_ids.iter().any(|&g| g >= FS1_SOURCE_SYMBOLS) {
            return RowStatus::Invalid("drop-id ∉ [0,47]".into());
        }
    }
    RowStatus::Dispatchable
}

// ── SWEEP (240 row) ────────────────────────────────────────────────────────────────────────────────
pub struct ExecSummary {
    pub total: usize,
    pub dispatchable: usize,
    pub unsupported: usize,
    pub ambiguous: usize,
    pub invalid: usize,
    pub duplicate: usize,
    pub by_arm: BTreeMap<&'static str, usize>,
    pub by_family: BTreeMap<&'static str, usize>,
    pub output_paths_unique: bool,
    pub failures: Vec<(String, String)>, // (run_id, reason)
}

#[must_use]
pub fn executability_sweep() -> ExecSummary {
    let mut s = ExecSummary {
        total: 0,
        dispatchable: 0,
        unsupported: 0,
        ambiguous: 0,
        invalid: 0,
        duplicate: 0,
        by_arm: BTreeMap::new(),
        by_family: BTreeMap::new(),
        output_paths_unique: true,
        failures: Vec::new(),
    };
    let mut paths: BTreeSet<String> = BTreeSet::new();
    let mut seen_exec: BTreeSet<u64> = BTreeSet::new();
    for c in amended_base_cells() {
        for arm in FS1_RESULT_BEARING_ARMS {
            for rep in 0..FS1_REPS {
                s.total += 1;
                let h = row_hashes(&c, arm, rep);
                let path = format!("{RAW_ARTIFACT_NAMESPACE}/{}.json", h.run_id);
                if !paths.insert(path) {
                    s.output_paths_unique = false;
                }
                // DUPLICATE: aynı run_execution_semantics_hash daha-önce görüldüyse.
                let mut st = executability_status(&c, arm, rep);
                if st.is_dispatchable() && !seen_exec.insert(h.exec) {
                    st = RowStatus::Duplicate("run_execution_semantics_hash tekrarı".into());
                }
                match &st {
                    RowStatus::Dispatchable => {
                        s.dispatchable += 1;
                        *s.by_arm.entry(arm).or_default() += 1;
                        *s.by_family.entry(c.impairment.kind()).or_default() += 1;
                    }
                    RowStatus::Unsupported(r) => {
                        s.unsupported += 1;
                        s.failures
                            .push((h.run_id.clone(), format!("UNSUPPORTED: {r}")));
                    }
                    RowStatus::Ambiguous(r) => {
                        s.ambiguous += 1;
                        s.failures
                            .push((h.run_id.clone(), format!("AMBIGUOUS: {r}")));
                    }
                    RowStatus::Invalid(r) => {
                        s.invalid += 1;
                        s.failures.push((h.run_id.clone(), format!("INVALID: {r}")));
                    }
                    RowStatus::Duplicate(r) => {
                        s.duplicate += 1;
                        s.failures
                            .push((h.run_id.clone(), format!("DUPLICATE: {r}")));
                    }
                }
            }
        }
    }
    s
}

// ── §26 EXACTDEFICIT POSITION COVERAGE (diagnostic; outcome-blind) ─────────────────────────────────
#[must_use]
pub fn exact_deficit_position_coverage_md() -> String {
    let mut s =
        String::from("# FS-1-EXACTDEFICIT-POSITION-COVERAGE (S4-C diagnostic; outcome-blind)\n\n");
    s.push_str("> ExactDeficit 9-cell × 5-rep seçili global-drop-ID'ler + tail-inclusion + K8/K4/K2 blok-dağılımı.\n");
    s.push_str("> Bilimsel-outcome-analizi DEĞİL; trace-diversity/strata-temsili gösterir. Post-outcome tune-YOK.\n\n");
    s.push_str("| cell | rep | drop_ids | tail(47)? | blocks@K8 | blocks@K4 | blocks@K2 |\n");
    s.push_str("|------|-----|----------|-----------|-----------|-----------|-----------|\n");
    let block_dist = |ids: &[u16], k: u16| -> String {
        let mut set = BTreeSet::new();
        for &g in ids {
            set.insert(g / k);
        }
        let v: Vec<String> = set.iter().map(u16::to_string).collect();
        format!("{{{}}}", v.join(","))
    };
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
    {
        let d = c.impairment.configured_exact_drop_count().unwrap();
        let rs = exact_rank_seed(c);
        for rep in 0..FS1_REPS {
            let ids = exact_deficit_target_set(d, rs, rep);
            let tail = ids.contains(&47);
            s.push_str(&format!(
                "| {} | {} | {:?} | {} | {} | {} | {} |\n",
                base_cell_id(c),
                rep,
                ids,
                if tail { "YES" } else { "no" },
                block_dist(&ids, 8),
                block_dist(&ids, 4),
                block_dist(&ids, 2),
            ));
        }
    }
    let any_tail = amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "ExactDeficit")
        .any(|c| {
            let d = c.impairment.configured_exact_drop_count().unwrap();
            let rs = exact_rank_seed(c);
            (0..FS1_REPS).any(|rep| exact_deficit_target_set(d, rs, rep).contains(&47))
        });
    s.push_str(&format!(
        "\ntail(47) HERHANGİ-bir ExactDeficit-rep'te temsil-edildi mi: {}\n",
        if any_tail { "EVET" } else { "HAYIR" }
    ));
    s
}

// ── §27 IID SEED COVERAGE (mechanism-only; non-result channel-decision enumerator) ─────────────────
#[must_use]
pub fn iid_admission_pattern_hash(seed: u64) -> u64 {
    let mut ch = TraceChannel::new(TraceSpec::NaturalIid {
        loss_permille: 200,
        seed,
    });
    let mut b: Vec<u8> = Vec::new();
    for g in 0..FS1_SOURCE_SYMBOLS {
        let dropped = ch.decide(TracePacketId::initial_source(0, g), u64::from(g) * 1000);
        b.push(u8::from(dropped));
    }
    fnv(&b)
}

#[must_use]
pub fn iid_seed_coverage_md() -> String {
    let mut s =
        String::from("# FS-1-IID-SEED-COVERAGE (S4-C mechanism-diagnostic; FS-1-KANIT-DEĞİL)\n\n");
    s.push_str("> IID 3-cell × 5-rep: forward_trace_seed + configured-forward-id + 48-InitialSource admission-pattern-hash\n");
    s.push_str("> (non-result channel-decision enumerator; transport-outcome KOŞULMAZ). loss_permille=200; role=InitialSource.\n\n");
    s.push_str("| cell | rep | trace_seed | fwd_config_id | admission_pattern_hash |\n");
    s.push_str("|------|-----|------------|---------------|------------------------|\n");
    let mut all_patterns: BTreeSet<u64> = BTreeSet::new();
    let mut all_seeds: BTreeSet<u64> = BTreeSet::new();
    for c in amended_base_cells()
        .iter()
        .filter(|c| c.impairment.kind() == "NaturalIid")
    {
        for rep in 0..FS1_REPS {
            let seed = paired_trace_seed(c, rep);
            let fwd = forward_config_id(c, &[], seed);
            let pat = iid_admission_pattern_hash(seed);
            all_seeds.insert(seed);
            all_patterns.insert(pat);
            s.push_str(&format!(
                "| {} | {} | 0x{seed:016x} | 0x{fwd:016x} | 0x{pat:016x} |\n",
                base_cell_id(c),
                rep
            ));
        }
    }
    s.push_str(&format!(
        "\nunique trace-seed: {} / 15 · unique admission-pattern: {} / 15 (collision → açıkça-raporlanır; seed-swap YOK)\n",
        all_seeds.len(),
        all_patterns.len()
    ));
    s
}

// ── EXECUTABILITY REPORTS (§23; JSON + MD) ─────────────────────────────────────────────────────────
#[must_use]
pub fn executability_report_json() -> String {
    let s = executability_sweep();
    let mut o = String::new();
    o.push_str("{\n");
    o.push_str("  \"artifact\": \"FS-1-MANIFEST-EXECUTABILITY-v1.3-CANDIDATE\",\n");
    o.push_str(&format!("  \"mode\": \"{DRYRUN_MODE}\",\n"));
    o.push_str("  \"result_bearing\": false,\n");
    o.push_str(&format!(
        "  \"manifest_schema_version\": \"{MANIFEST_SCHEMA_VERSION}\",\n"
    ));
    o.push_str(&format!(
        "  \"candidate_prereg_status\": \"{CANDIDATE_PREREG_STATUS}\",\n"
    ));
    o.push_str(&format!(
        "  \"v12_historical_hash\": \"{V12_HISTORICAL_HASH}\",\n"
    ));
    o.push_str(&format!(
        "  \"s3_candidate_matrix_hash\": \"{S3_CANDIDATE_MATRIX_HASH}\",\n"
    ));
    o.push_str(&format!("  \"source_commit\": \"{SOURCE_COMMIT_REF}\",\n"));
    o.push_str(&format!(
        "  \"expected_binary_provenance\": \"{EXPECTED_BINARY_PROVENANCE}\",\n"
    ));
    o.push_str(&format!("  \"total\": {},\n", s.total));
    o.push_str(&format!("  \"dispatchable\": {},\n", s.dispatchable));
    o.push_str(&format!("  \"unsupported\": {},\n", s.unsupported));
    o.push_str(&format!("  \"ambiguous\": {},\n", s.ambiguous));
    o.push_str(&format!("  \"invalid\": {},\n", s.invalid));
    o.push_str(&format!("  \"duplicate\": {},\n", s.duplicate));
    o.push_str(&format!(
        "  \"by_family\": {{ \"ExactDeficit\": {}, \"NaturalIid\": {} }},\n",
        s.by_family.get("ExactDeficit").copied().unwrap_or(0),
        s.by_family.get("NaturalIid").copied().unwrap_or(0)
    ));
    o.push_str(&format!(
        "  \"by_arm\": {{ \"A0\": {}, \"A1-EARLY\": {}, \"F0\": {}, \"F3\": {} }},\n",
        s.by_arm.get("A0").copied().unwrap_or(0),
        s.by_arm.get("A1-EARLY").copied().unwrap_or(0),
        s.by_arm.get("F0").copied().unwrap_or(0),
        s.by_arm.get("F3").copied().unwrap_or(0)
    ));
    o.push_str("  \"pairing_groups\": 60,\n");
    o.push_str(&format!(
        "  \"output_paths_unique\": {},\n",
        s.output_paths_unique
    ));
    o.push_str("  \"result_bearing_firewall\": {\n");
    o.push_str(&format!(
        "    \"raw_artifact_namespace\": \"{RAW_ARTIFACT_NAMESPACE}\",\n"
    ));
    o.push_str("    \"raw_result_files_written\": 0,\n");
    o.push_str("    \"channel_loop_advanced\": false,\n");
    o.push_str("    \"outcome_fields_populated\": false,\n");
    o.push_str("    \"fs1_run_ids_spent\": 0\n");
    o.push_str("  },\n");
    o.push_str("  \"failures\": [");
    let fjoin: Vec<String> = s
        .failures
        .iter()
        .map(|(rid, r)| format!("{{ \"run_id\": \"{rid}\", \"reason\": \"{r}\" }}"))
        .collect();
    o.push_str(&fjoin.join(", "));
    o.push_str("]\n}\n");
    o
}

#[must_use]
pub fn executability_report_md() -> String {
    let s = executability_sweep();
    let mut o = String::from("# FS-1-MANIFEST-EXECUTABILITY-v1.3-CANDIDATE (S4-C)\n\n");
    o.push_str("> NON-RESULT executability dry-run (mode=EXECUTABILITY_ONLY; result_bearing=false). Her satır\n");
    o.push_str("> RunCfg/workload/source-schedule/arm/K-profile/forward-trace/reverse-trace KONSTRÜKTE-edildi ve\n");
    o.push_str("> doğrulandı; channel-loop İLERLETİLMEDİ; raw-result YAZILMADI; FS-1-run-ID HARCANMADI.\n\n");
    o.push_str(&format!(
        "manifest : {MANIFEST_SCHEMA_VERSION} (candidate; NOT-FROZEN) · source_commit {SOURCE_COMMIT_REF}\n\n"
    ));
    o.push_str("## Executability totals\n```text\n");
    o.push_str(&format!(
        "total={} · DISPATCHABLE={} · unsupported={} · ambiguous={} · invalid={} · duplicate={}\n",
        s.total, s.dispatchable, s.unsupported, s.ambiguous, s.invalid, s.duplicate
    ));
    o.push_str(&format!(
        "by-family: ExactDeficit={}/180 · NaturalIid={}/60\n",
        s.by_family.get("ExactDeficit").copied().unwrap_or(0),
        s.by_family.get("NaturalIid").copied().unwrap_or(0)
    ));
    o.push_str(&format!(
        "by-arm: A0={}/60 · A1-EARLY={}/60 · F0={}/60 · F3={}/60\n",
        s.by_arm.get("A0").copied().unwrap_or(0),
        s.by_arm.get("A1-EARLY").copied().unwrap_or(0),
        s.by_arm.get("F0").copied().unwrap_or(0),
        s.by_arm.get("F3").copied().unwrap_or(0)
    ));
    o.push_str(&format!(
        "pairing-groups=60 · output_paths_unique={}\n",
        s.output_paths_unique
    ));
    o.push_str("```\n\n## Result-bearing firewall\n```text\n");
    o.push_str(&format!(
        "mode={DRYRUN_MODE} · raw_result_files_written=0 · channel_loop_advanced=false ·\n"
    ));
    o.push_str(&format!("outcome_fields_populated=false · fs1_run_ids_spent=0 · raw_namespace={RAW_ARTIFACT_NAMESPACE}\n"));
    o.push_str("```\n\n## Dry-run command\n```text\n");
    o.push_str("cargo test -p kama-integration --features experimental-fs-harness --test fs_s4c_pins\n```\n");
    if s.failures.is_empty() {
        o.push_str("\nfailures: NONE (240/240 dispatchable).\n");
    } else {
        o.push_str("\n## Failures\n```text\n");
        for (rid, r) in &s.failures {
            o.push_str(&format!("{rid} : {r}\n"));
        }
        o.push_str("```\n");
    }
    o
}
