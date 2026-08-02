//! FINAL BUILD OPT-R1 (S5-C2) — result-bearing EXECUTOR + persistence synthetic tests (§5). ISOLATED temp
//! namespace/ledger + SYNTHETIC run-ids ONLY — final-run-id/namespace/ledger DOKUNULMAZ; campaign-data-YOK.
#![cfg(feature = "experimental-fs-harness")]
#![allow(clippy::duplicate_mod)]

#[path = "common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;

fn tmp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("fs1_synth_{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn synth_record(run_id: &str, attempt: u8, v: Validity) -> RawRecord {
    RawRecord {
        run_id: run_id.to_string(),
        attempt_index: attempt,
        pairing_group_id: "SYNTH-pg".into(),
        arm: "F0".into(),
        base_cell_id: "SYNTH-cell".into(),
        repetition_index: 0,
        freeze_lock_sha256: "0".repeat(64),
        final_manifest_sha256: "1".repeat(64),
        execution_binary_sha256: "2".repeat(64),
        execution_source_commit: "synth".into(),
        base_cell_semantics_hash: "0x0".into(),
        run_execution_semantics_hash: "0x0".into(),
        resolved_behavior_semantics_hash: "0x0".into(),
        validity_class: v,
        deadline_hit: matches!(v, Validity::ValidComplete),
        delivered_fragments: 48,
        delivered_bytes: 2400,
        reassembly_equal: matches!(v, Validity::ValidComplete),
        completion_ns: if matches!(v, Validity::ValidComplete) {
            Some(1)
        } else {
            None
        },
        censor_time_ns: if matches!(v, Validity::ValidCensored) {
            Some(699_000_000)
        } else {
            None
        },
        base_source_bytes: 4848,
        intervention_bytes: 0,
        arq_retransmit_bytes: 0,
        total_outcome_bytes: 4848,
        retransmits: 0,
        fec_decoded_sources: 0,
    }
}

const PROV: fn() -> Provenance = || Provenance {
    freeze_lock_sha256: "0".repeat(64),
    final_manifest_sha256: "1".repeat(64),
    execution_binary_sha256: "2".repeat(64),
    execution_source_commit: "synth".into(),
};

// ── SHA-256 regression (matches sha256sum; §2) ─────────────────────────────────────────────────────
#[test]
fn sha256_known_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// ── §5 VALID_COMPLETE persistence (real run_spine; synthetic-id; temp namespace) ────────────────────
#[test]
fn synth_valid_complete_persist() {
    let ns = tmp("vc_ns");
    let ledger = Ledger::new(tmp("vc").join("ledger.log"));
    let cells = amended_base_cells();
    let c = cells
        .iter()
        .find(|c| base_cell_id(c) == "rtt50-exactd1")
        .unwrap();
    // F0 recovers (FEC) → VALID_COMPLETE.
    let v = execute_row_with_id(
        "SYNTH-vc",
        c,
        "F0",
        0,
        &PROV(),
        &RawWriter::new(&ns),
        &ledger,
    )
    .unwrap();
    assert_eq!(v, Validity::ValidComplete);
    let raw = ns.join("SYNTH-vc.attempt0.json");
    assert!(raw.exists(), "raw record written");
    assert!(
        ns.join("SYNTH-vc.attempt0.json.sha256").exists(),
        "checksum sidecar"
    );
    // ledger/raw atomic consistency (§5): ledger raw_sha == sha256(raw file).
    let body = std::fs::read(&raw).unwrap();
    let file_sha = sha256_hex(&body);
    let entry = ledger
        .entries()
        .into_iter()
        .find(|e| e.kind == "COMPLETE")
        .unwrap();
    assert_eq!(
        entry.raw_sha256.unwrap(),
        file_sha,
        "ledger raw_sha == file sha"
    );
    assert!(
        body.windows(21).any(|w| w == b"\"result_bearing\":true"),
        "result_bearing=true"
    );
}

// ── §5 VALID_CENSORED persistence (A0 tail-stall) ──────────────────────────────────────────────────
#[test]
fn synth_valid_censored_persist() {
    let ns = tmp("cn_ns");
    let ledger = Ledger::new(tmp("cn").join("ledger.log"));
    let cells = amended_base_cells();
    // rtt50-exactd4 rep1 target includes tail g47 → A0 tail-erasure → VALID_CENSORED.
    let c = cells
        .iter()
        .find(|c| base_cell_id(c) == "rtt50-exactd4")
        .unwrap();
    let v = execute_row_with_id(
        "SYNTH-cn",
        c,
        "A0",
        1,
        &PROV(),
        &RawWriter::new(&ns),
        &ledger,
    )
    .unwrap();
    assert_eq!(v, Validity::ValidCensored, "A0 tail-stall = VALID_CENSORED");
    assert!(ns.join("SYNTH-cn.attempt0.json").exists());
    let body = std::fs::read_to_string(ns.join("SYNTH-cn.attempt0.json")).unwrap();
    assert!(body.contains("\"validity_class\":\"VALID_CENSORED\""));
    assert!(body.contains("\"censor_time_ns\":") && !body.contains("\"censor_time_ns\":null"));
}

// ── §5 no-overwrite (existing-output rejection) ────────────────────────────────────────────────────
#[test]
fn synth_no_overwrite() {
    let ns = tmp("ow_ns");
    let w = RawWriter::new(&ns);
    let rec = synth_record("SYNTH-ow", 0, Validity::ValidComplete);
    let sha1 = w.write(&rec).unwrap();
    assert_eq!(sha1.len(), 64);
    // ikinci-write → OutputExists (overwrite-YOK).
    assert!(
        matches!(w.write(&rec), Err(ExecError::OutputExists(_))),
        "no-overwrite"
    );
}

// ── §5 duplicate-start + duplicate-valid rejection ─────────────────────────────────────────────────
#[test]
fn synth_ledger_dup_rejection() {
    let ledger = Ledger::new(tmp("dup").join("ledger.log"));
    ledger.reserve_start("SYNTH-d", 0).unwrap();
    assert!(
        matches!(
            ledger.reserve_start("SYNTH-d", 0),
            Err(ExecError::DuplicateStart(_))
        ),
        "dup-start"
    );
    ledger
        .commit("SYNTH-d", 0, Validity::ValidComplete, &"a".repeat(64))
        .unwrap();
    assert!(
        matches!(
            ledger.commit("SYNTH-d", 0, Validity::ValidComplete, &"a".repeat(64)),
            Err(ExecError::DuplicateValidOutcome(_))
        ),
        "dup-valid"
    );
    // VALID sonrası reserve_start → dup-start (final).
    assert!(matches!(
        ledger.reserve_start("SYNTH-d", 1),
        Err(ExecError::DuplicateStart(_))
    ));
}

// ── §5 INVALID retry + primary+2 ceiling ───────────────────────────────────────────────────────────
#[test]
fn synth_invalid_retry_ceiling() {
    let ledger = Ledger::new(tmp("retry").join("ledger.log"));
    // 3 INVALID → attempt 0,1,2; sonra retry-YOK.
    assert_eq!(ledger.next_attempt("SYNTH-r"), Some(0));
    ledger.commit("SYNTH-r", 0, Validity::Invalid, "-").unwrap();
    assert_eq!(ledger.next_attempt("SYNTH-r"), Some(1), "INVALID → retry");
    ledger.commit("SYNTH-r", 1, Validity::Invalid, "-").unwrap();
    assert_eq!(ledger.next_attempt("SYNTH-r"), Some(2));
    ledger.commit("SYNTH-r", 2, Validity::Invalid, "-").unwrap();
    assert_eq!(
        ledger.next_attempt("SYNTH-r"),
        None,
        "primary+2 ceiling → retry-YOK"
    );
    // VALID_CENSORED sonrası retry-YOK.
    let l2 = Ledger::new(tmp("retry2").join("ledger.log"));
    l2.commit("SYNTH-c", 0, Validity::ValidCensored, "-")
        .unwrap();
    assert_eq!(
        l2.next_attempt("SYNTH-c"),
        None,
        "VALID_CENSORED final → retry-YOK"
    );
}

// ── §5 checksum-mismatch detection ─────────────────────────────────────────────────────────────────
#[test]
fn synth_checksum_mismatch() {
    let ns = tmp("cs_ns");
    let w = RawWriter::new(&ns);
    let sha = w
        .write(&synth_record("SYNTH-cs", 0, Validity::ValidComplete))
        .unwrap();
    let raw = ns.join("SYNTH-cs.attempt0.json");
    let good = sha256_hex(&std::fs::read(&raw).unwrap());
    assert!(verify_hash(&sha, &good), "yazılan-sha == dosya-sha");
    // tamper → mismatch.
    std::fs::write(&raw, b"tampered").unwrap();
    let bad = sha256_hex(&std::fs::read(&raw).unwrap());
    assert!(
        !verify_hash(&sha, &bad),
        "tamper → checksum-mismatch tespit"
    );
}

// ── §5 interrupted-write recovery (quarantine; no-overwrite) ────────────────────────────────────────
#[test]
fn synth_interrupted_recovery() {
    let ns = tmp("int_ns");
    std::fs::write(ns.join("SYNTH-int.attempt0.json.tmp"), b"half-written").unwrap();
    let w = RawWriter::new(&ns);
    assert_eq!(w.residue_tmps().len(), 1, "residue tespit");
    assert!(
        matches!(
            quarantine_residue(&w),
            Err(ExecError::InterruptedResidue(_))
        ),
        "residue → quarantine"
    );
    assert!(
        ns.join("SYNTH-int.attempt0.json.tmp.quarantine").exists(),
        "quarantined (silinmedi)"
    );
    assert!(quarantine_residue(&w).is_ok(), "quarantine sonrası temiz");
}

// ── §5 wrong-order rejection + serial next-eligible (resume) ────────────────────────────────────────
#[test]
fn synth_order_and_resume() {
    let order: Vec<String> = vec!["r0".into(), "r1".into(), "r2".into()];
    assert!(verify_order_position(&order, "r0", "r0").is_ok());
    assert!(
        matches!(
            verify_order_position(&order, "r0", "r1"),
            Err(ExecError::WrongOrder(_))
        ),
        "wrong-order"
    );
    assert!(
        matches!(
            verify_order_position(&order, "r0", "rX"),
            Err(ExecError::NotInManifest(_))
        ),
        "not-in-manifest"
    );
    // resume: r0 VALID → next-eligible = r1.
    let ledger = Ledger::new(tmp("resume").join("ledger.log"));
    assert_eq!(next_eligible(&order, &ledger), Some("r0"));
    ledger
        .commit("r0", 0, Validity::ValidComplete, "-")
        .unwrap();
    assert_eq!(
        next_eligible(&order, &ledger),
        Some("r1"),
        "resume after r0"
    );
}

// ── §5 executor determinism (aynı synthetic-row 2× → aynı validity + aynı raw-sha) ─────────────────
#[test]
fn synth_executor_deterministic() {
    let cells = amended_base_cells();
    let c = cells
        .iter()
        .find(|c| base_cell_id(c) == "rtt50-exactd1")
        .unwrap();
    let run = |tag: &str| {
        let ns = tmp(tag);
        let ledger = Ledger::new(tmp(&format!("{tag}_l")).join("ledger.log"));
        execute_row_with_id(
            "SYNTH-det",
            c,
            "F0",
            0,
            &PROV(),
            &RawWriter::new(&ns),
            &ledger,
        )
        .unwrap();
        sha256_hex(&std::fs::read(ns.join("SYNTH-det.attempt0.json")).unwrap())
    };
    assert_eq!(
        run("det1"),
        run("det2"),
        "run_spine+persist deterministik (RNG-YOK)"
    );
}
