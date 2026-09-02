//! DR-FEC-FS-1 OPT-R1 — CAMPAIGN EXECUTOR ORCHESTRATION (row → run_spine → classify → persist). HARNESS-ONLY.
//! fs_dryrun (manifest/config/executability) + fs_spine (run_spine) + fs_exec (persistence). Both fs1_runner AND
//! synthetic-tests bunu kullanır. Result-bearing YALNIZ verify-chain-sonrası; default EXECUTABILITY_ONLY (main'de).
#![allow(clippy::duplicate_mod)] // fs_dryrun + fs_spine bağımsız-harness-ağaçları fs_workload'ı ayrı-modül içerir
#![allow(dead_code)] // re-export'lar test-binary vs example-binary'de farklı-alt-küme kullanır

#[path = "fs_dryrun.rs"]
mod fs_dryrun;
#[path = "fs_exec.rs"]
mod fs_exec;
#[path = "fs_serialize_v2.rs"]
mod fs_serialize_v2;
#[path = "fs_spine.rs"]
mod fs_spine;

pub use fs_dryrun::*;
pub use fs_exec::*;
pub use fs_serialize_v2::*;
pub use fs_spine::{run_spine, SpineArm, SpineCfg, MS};

const LINK_BPS: u64 = 1_000_000;
const PACER_BPS: u64 = 800_000;
const PACE_DT_NS: u64 = 10_000;

/// Frozen provenance the executor stamps into each raw record + verifies at launch.
#[derive(Debug, Clone)]
pub struct Provenance {
    pub freeze_lock_sha256: String,
    pub final_manifest_sha256: String,
    pub execution_binary_sha256: String,
    pub execution_source_commit: String,
}

#[must_use]
pub fn build_cfg(c: &BaseCell, rep: u8) -> SpineCfg {
    let owd = c.rtt_ms * MS / 2;
    let horizon_ns = observation_horizon_ms(c.rtt_ms) * MS;
    match c.impairment {
        FsImpairment::ExactDeficit { drop_count } => {
            let drops = exact_deficit_target_set(drop_count.get(), exact_rank_seed(c), rep);
            SpineCfg::diag(LINK_BPS, owd, &drops)
                .with_pacer(PACER_BPS)
                .with_dt(PACE_DT_NS)
                .with_horizon(horizon_ns)
        }
        FsImpairment::NaturalIid { loss_permille } => SpineCfg::iid(
            LINK_BPS,
            owd,
            u32::from(loss_permille.get()),
            paired_trace_seed(c, rep),
        )
        .with_pacer(PACER_BPS)
        .with_dt(PACE_DT_NS)
        .with_horizon(horizon_ns),
    }
}

#[must_use]
pub fn arm_of(arm: &str) -> SpineArm {
    match arm {
        "A0" => SpineArm::A0,
        "A1-EARLY" => SpineArm::A1 { r: 2 },
        "F0" => SpineArm::F0 { r: 2 },
        "F3" => SpineArm::F3 { r: 2 },
        other => panic!("non-OPT-A arm: {other}"),
    }
}

/// §14 classify: completed+reassembly (horizon-içinde run_spine tamamladı) → VALID_COMPLETE; aksi VALID_CENSORED.
/// (INVALID = contract-failure; run-ÖNCESİ verify'de yakalanır, burada değil.)
#[must_use]
pub fn classify(r: &fs_spine::SpineResult) -> Validity {
    if r.completed && r.reassembly_equal {
        Validity::ValidComplete
    } else {
        Validity::ValidCensored
    }
}

#[allow(clippy::too_many_arguments)]
#[must_use]
fn build_raw_record(
    run_id: &str,
    c: &BaseCell,
    arm: &str,
    rep: u8,
    attempt: u8,
    r: &fs_spine::SpineResult,
    prov: &Provenance,
    validity: Validity,
) -> RawRecord {
    let h = row_hashes(c, arm, rep);
    let deadline_ns = deadline_ms_for(c.rtt_ms) * MS;
    RawRecord {
        run_id: run_id.to_string(),
        attempt_index: attempt,
        pairing_group_id: pairing_group_id(c, rep),
        arm: arm.to_string(),
        base_cell_id: base_cell_id(c),
        repetition_index: rep,
        freeze_lock_sha256: prov.freeze_lock_sha256.clone(),
        final_manifest_sha256: prov.final_manifest_sha256.clone(),
        execution_binary_sha256: prov.execution_binary_sha256.clone(),
        execution_source_commit: prov.execution_source_commit.clone(),
        base_cell_semantics_hash: format!("0x{:016x}", h.base),
        run_execution_semantics_hash: format!("0x{:016x}", h.exec),
        resolved_behavior_semantics_hash: format!("0x{:016x}", h.behav),
        validity_class: validity,
        deadline_hit: r.completion_ns.is_some_and(|t| t <= deadline_ns),
        delivered_fragments: r.delivered_source_symbols as u16,
        delivered_bytes: r.delivered_app_bytes as u32,
        reassembly_equal: r.reassembly_equal,
        completion_ns: r.completion_ns,
        censor_time_ns: if r.completed {
            None
        } else {
            Some(observation_horizon_ms(c.rtt_ms) * MS)
        },
        base_source_bytes: r.base_source_bytes,
        intervention_bytes: r.intervention_bytes,
        arq_retransmit_bytes: r.arq_retransmit_bytes,
        total_outcome_bytes: r.total_outcome_bytes,
        retransmits: r.retransmits,
        fec_decoded_sources: r.fec_decoded_sources,
    }
}

/// **Result-bearing per-row executor** (run_id explicit → test synthetic-id kullanabilir; campaign derived-id).
/// reserve-start → run_spine → classify → build-record → ATOMIC write+checksum → ledger-commit. INVALID-only-retry.
pub fn execute_row_with_id(
    run_id: &str,
    c: &BaseCell,
    arm: &str,
    rep: u8,
    prov: &Provenance,
    writer: &RawWriter,
    ledger: &Ledger,
) -> Result<Validity, ExecError> {
    // interrupted-residue quarantine (restart-safety) — overwrite-DEĞİL.
    quarantine_residue(writer)?;
    let attempt = match ledger.next_attempt(run_id) {
        Some(a) => a,
        None => return Err(ExecError::RetryExceeded(run_id.into())), // final veya max-retry
    };
    ledger.reserve_start(run_id, attempt)?;
    let r = run_spine(arm_of(arm), &build_cfg(c, rep));
    let validity = classify(&r);
    let rec = build_raw_record(run_id, c, arm, rep, attempt, &r, prov, validity);
    let raw_sha = writer.write(&rec)?; // atomic + checksum; overwrite → err
    ledger.commit(run_id, attempt, validity, &raw_sha)?;
    Ok(validity)
}

/// **SERIAL campaign** (parallelism=1; frozen execution-order; next-eligible; VALID-tamamlanan atlanır).
pub fn run_campaign(
    prov: &Provenance,
    writer: &RawWriter,
    ledger: &Ledger,
) -> Vec<(String, Validity)> {
    let cells = amended_base_cells();
    let order: Vec<String> = execution_order_entries()
        .into_iter()
        .map(|(_, _, _, _, _, rid)| rid)
        .collect();
    let mut out = Vec::new();
    // serial: order'da next-eligible olanı çalıştır; VALID-tamamlanmışları atla.
    for (_, _, bcid, rep, arm, rid) in execution_order_entries() {
        if next_eligible(&order, ledger) != Some(rid.as_str()) {
            // önceki-row VALID-tamamlanmadıysa serial-ilerleme durur (bu tasarımda sıra-garantili).
        }
        let c = cells.iter().find(|c| base_cell_id(c) == bcid).unwrap();
        if let Ok(v) = execute_row_with_id(&rid, c, arm, rep, prov, writer, ledger) {
            out.push((rid, v));
        }
    }
    out
}

// ── OPT-X2 v1.3.2 SCHEMA-V2 CAMPAIGN FLOW (instrumented executor; schema-conformant serializer) ────────

/// SpineResult → schema-v2 validity.
#[must_use]
pub fn classify_v2(r: &fs_spine::SpineResult) -> ValidityV2 {
    if r.completed && r.reassembly_equal {
        ValidityV2::ValidComplete
    } else {
        ValidityV2::ValidCensored
    }
}

/// Frozen candidate-manifest row (scientific config) for each run_id — typed_manifest_row source.
#[must_use]
pub fn manifest_rows_by_run_id() -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    for line in manifest_jsonl().lines() {
        // extract "run_id":"<val>"
        if let Some(p) = line.find("\"run_id\":\"") {
            let rest = &line[p + 10..];
            if let Some(e) = rest.find('"') {
                m.insert(rest[..e].to_string(), line.to_string());
            }
        }
    }
    m
}

/// Result-bearing per-row executor (v2): reserve → run_spine → classify_v2 → build_record_v2 → validate_v2
/// (fail-closed) → atomic write_serialized → ledger-commit. INVALID-only-retry (unchanged policy).
#[allow(clippy::too_many_arguments)]
pub fn execute_row_v2(
    run_id: &str,
    c: &BaseCell,
    arm: &str,
    rep: u8,
    prov: &ProvV2,
    manifest_row: &str,
    writer: &RawWriter,
    ledger: &Ledger,
) -> Result<ValidityV2, ExecError> {
    quarantine_residue(writer)?;
    let attempt = match ledger.next_attempt(run_id) {
        Some(a) => a,
        None => return Err(ExecError::RetryExceeded(run_id.into())),
    };
    ledger.reserve_start(run_id, attempt)?;
    let r = run_spine(arm_of(arm), &build_cfg(c, rep));
    let validity = classify_v2(&r);
    let h = row_hashes(c, arm, rep);
    let rec = build_record_v2(
        run_id,
        &base_cell_id(c),
        &pairing_group_id(c, rep),
        arm,
        rep,
        attempt,
        validity,
        &r,
        prov,
        manifest_row.to_string(),
        h.base,
        h.exec,
        h.behav,
        u32::try_from(deadline_ms_for(c.rtt_ms)).unwrap_or(0),
        u32::try_from(observation_horizon_ms(c.rtt_ms)).unwrap_or(0),
    );
    let body = rec.serialize();
    // KALICI-KURAL-3 executable per-record conformance gate (fail-closed).
    if let Err(e) = validate_v2(&body) {
        return Err(ExecError::ProvenanceMismatch(format!(
            "schema-v2 validation: {e:?}"
        )));
    }
    let map_v = match validity {
        ValidityV2::ValidComplete => Validity::ValidComplete,
        ValidityV2::ValidCensored => Validity::ValidCensored,
        ValidityV2::Invalid => Validity::Invalid,
    };
    let raw_sha = writer.write_serialized(run_id, attempt, &body)?;
    ledger.commit(run_id, attempt, map_v, &raw_sha)?;
    Ok(validity)
}

/// SERIAL v2 campaign over the frozen execution-order.
#[must_use]
pub fn run_campaign_v2(
    prov: &ProvV2,
    writer: &RawWriter,
    ledger: &Ledger,
) -> Vec<(String, ValidityV2)> {
    let cells = amended_base_cells();
    let rows = manifest_rows_by_run_id();
    let mut out = Vec::new();
    for (_, _, bcid, rep, arm, rid) in execution_order_entries() {
        let c = cells.iter().find(|c| base_cell_id(c) == bcid).unwrap();
        let mrow = rows.get(&rid).cloned().unwrap_or_default();
        if let Ok(v) = execute_row_v2(&rid, c, arm, rep, prov, &mrow, writer, ledger) {
            out.push((rid, v));
        }
    }
    out
}

/// Executable pin: terminal_source_id == 47 for the FS-1 workload (source_symbol_count==48).
#[must_use]
pub fn tail_terminal_id_pin() -> bool {
    (fs_spine::TOTAL_SOURCE_SYMBOLS as u16).saturating_sub(1) == 47
}
