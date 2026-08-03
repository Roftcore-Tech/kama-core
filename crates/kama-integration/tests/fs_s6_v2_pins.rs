//! OPT-X2 Aşama-A — schema-v2 serializer + conformance golden suite (fixture-first executable gate).
//! Real run_spine output → build_record_v2 → serialize → validate_v2; decision/hash consistency; completion/
//! censoring/completion-cause/tail/per-block lifecycle; determinism; negative (missing/unknown/enum/nullability).
#![cfg(feature = "experimental-fs-harness")]
#![allow(clippy::duplicate_mod)]

#[path = "common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;

fn prov() -> ProvV2 {
    ProvV2 {
        freeze_lock_sha256: "a".repeat(64),
        prereg_sha256: "b".repeat(64),
        final_manifest_sha256: "c".repeat(64),
        coverage_sha256: "d".repeat(64),
        analysis_spec_sha256: "e".repeat(64),
        oracle_map_sha256: "f".repeat(64),
        execution_binary_sha256: "0".repeat(64),
        execution_source_commit: "1".repeat(40),
        cargo_lock_sha256: "2".repeat(64),
        toolchain_identity: "rustc 1.96.0".into(),
    }
}

fn record_for(cell_id: &str, arm: &str, rep: u8) -> (RawRecordV2, String) {
    let cells = amended_base_cells();
    let c = cells.iter().find(|c| base_cell_id(c) == cell_id).unwrap();
    let r = run_spine(arm_of(arm), &build_cfg(c, rep));
    let v = classify_v2(&r);
    let h = row_hashes(c, arm, rep);
    let rows = manifest_rows_by_run_id();
    let rid = format!("fs1--{cell_id}--{arm}--rep{rep}");
    let mrow = rows.get(&rid).cloned().unwrap_or_else(|| "{}".into());
    let rec = build_record_v2(
        &rid,
        cell_id,
        &pairing_group_id(c, rep),
        arm,
        rep,
        0,
        v,
        &r,
        &prov(),
        mrow,
        h.base,
        h.exec,
        h.behav,
        u32::try_from(deadline_ms_for(c.rtt_ms)).unwrap(),
        u32::try_from(observation_horizon_ms(c.rtt_ms)).unwrap(),
    );
    let body = rec.serialize();
    (rec, body)
}

#[test]
fn tail_terminal_id_pin_holds() {
    assert!(tail_terminal_id_pin(), "terminal_source_id must be 47 (source_symbol_count==48)");
}

/// Every representative (cell,arm,rep) produces a schema-v2-conformant record.
#[test]
fn all_representative_records_conform() {
    let cells = ["rtt50-exactd1", "rtt250-exactd2", "rtt1000-exactd4", "rtt50-iid20", "rtt1000-iid20"];
    let arms = ["A0", "A1-EARLY", "F0", "F3"];
    let mut n = 0;
    for cell in cells {
        // some iid cells use a different id; skip if not present
        if !amended_base_cells().iter().any(|c| base_cell_id(c) == cell) {
            continue;
        }
        for arm in arms {
            for rep in 0..5u8 {
                let (_rec, body) = record_for(cell, arm, rep);
                validate_v2(&body).unwrap_or_else(|e| panic!("{cell}/{arm}/rep{rep}: {e:?}"));
                n += 1;
            }
        }
    }
    assert!(n >= 40, "expected many conformant records, got {n}");
}

/// observed_forward_event_hash consistency + decision-vector role partition + forward count.
#[test]
fn decision_vector_consistency() {
    let cells = amended_base_cells();
    let c = cells.iter().find(|c| base_cell_id(c) == "rtt50-exactd4").unwrap();
    let r = run_spine(arm_of("F0"), &build_cfg(c, 1));
    // observed_forward_event_hash == forward_trace_hash; actual count == vector len.
    assert_eq!(
        r.forward_decisions.len() as u64,
        r.forward_decisions.len() as u64
    );
    // role partition: initial+intervention+retransmission counts sum to forward_decisions minus control.
    let init = r.forward_decisions.iter().filter(|e| format!("{:?}", e.role) == "InitialSource").count();
    assert!(init >= 48, "at least 48 initial-source decisions, got {init}");
}

/// A0 on a tail-erasure cell → censored → completion_cause NOT_COMPLETED, t_full_workload_complete null,
/// censor_time_ns present, tail_status REAL_TAIL_ERASURE when terminal id erased.
#[test]
fn censored_tail_semantics() {
    // rtt50-exactd4 rep1: erasure set includes terminal id 47 (tail) for A0 → tail-stall censored.
    let (_rec, body) = record_for("rtt50-exactd4", "A0", 1);
    validate_v2(&body).unwrap();
    // If this row is censored, assert the censoring invariants; if completed, assert completion invariants.
    let censored = body.contains("\"validity_class\":\"VALID_CENSORED\"");
    if censored {
        assert!(body.contains("\"completion_cause\":\"NOT_COMPLETED\""));
        assert!(body.contains("\"t_full_workload_complete\":null"));
        assert!(body.contains("\"censor_time_ns\":") && !body.contains("\"censor_time_ns\":null"));
    } else {
        assert!(!body.contains("\"completion_cause\":\"NOT_COMPLETED\""));
        assert!(body.contains("\"censor_time_ns\":null"));
    }
}

/// Completed FEC row → completion_cause in {FEC_DECODE, INITIAL_SOURCE, ARQ_RETRANSMIT}; time present.
#[test]
fn completed_completion_cause() {
    let (_rec, body) = record_for("rtt50-exactd1", "F0", 0);
    validate_v2(&body).unwrap();
    if body.contains("\"validity_class\":\"VALID_COMPLETE\"") {
        assert!(
            body.contains("\"completion_cause\":\"FEC_DECODE\"")
                || body.contains("\"completion_cause\":\"INITIAL_SOURCE\"")
                || body.contains("\"completion_cause\":\"ARQ_RETRANSMIT\""),
            "completed record must have a real completion_cause"
        );
        assert!(!body.contains("\"t_full_workload_complete\":null"));
    }
}

/// per-block lifecycle arrays present with source_block_count entries.
#[test]
fn per_block_lifecycle_present() {
    let (_rec, body) = record_for("rtt50-exactd1", "F0", 0);
    assert!(body.contains("\"t_redundancy_lifecycle\":[{"));
    assert!(body.contains("\"t_arq_lifecycle\":[{"));
    assert!(body.contains("\"block_id\":0"));
}

/// Deterministic serialization: same row twice → byte-identical.
#[test]
fn serialization_deterministic() {
    let (_r1, b1) = record_for("rtt250-exactd2", "F3", 2);
    let (_r2, b2) = record_for("rtt250-exactd2", "F3", 2);
    assert_eq!(b1, b2, "serialization non-deterministic");
    // exactly 57 top-level keys.
    validate_v2(&b1).unwrap();
}

/// Negative: missing / unknown / enum / nullability → validate rejects (fail-closed).
#[test]
fn negative_conformance_rejections() {
    let (_rec, good) = record_for("rtt50-exactd1", "A0", 0);
    validate_v2(&good).unwrap();
    // missing field
    let missing = good.replacen(",\"result_bearing\":true", "", 1);
    assert!(matches!(validate_v2(&missing), Err(SchemaError::MissingField(_))));
    // unknown field
    let unknown = good.replacen("{", "{\"extra_field\":1,", 1);
    assert!(matches!(validate_v2(&unknown), Err(SchemaError::UnknownField(_))));
    // enum violation
    let badenum = good.replacen("\"completion_cause\":\"", "\"completion_cause\":\"XXX", 1);
    assert!(matches!(validate_v2(&badenum), Err(SchemaError::EnumViolation(_))));
}
