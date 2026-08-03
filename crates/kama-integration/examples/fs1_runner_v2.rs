//! OPT-X2 Aşama-B — v1.3.2 schema-conformant campaign runner (instrumented executor + schema-v2 serializer).
//! Re-runs the frozen 240-row campaign writing FS-1-RAW-SCHEMA-v2 records to a new namespace. run_spine unchanged
//! (scientific outcomes invariant); only the persistence/observation layer differs. Fail-closed per-record validate.
#![allow(clippy::duplicate_mod)]

#[path = "../tests/common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

fn self_sha() -> String {
    std::fs::read("/proc/self/exe").map(|b| sha256_hex(&b)).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // frozen experiment-defining constants (v1.3.1; UNCHANGED — pinned in the freeze-delta whitelist).
    let prov = ProvV2 {
        freeze_lock_sha256: arg(&args, "--freeze-lock-sha").unwrap_or_default(),
        prereg_sha256: "c12b2bee7c407cb1bcc089630c78a2fe8eabea4568129718230b2c2ed013660e".into(),
        final_manifest_sha256: "12fc0a75da87e49bc078ed230e071c8d8751c981fe7ab9ae8226b591d35ddbcc".into(),
        coverage_sha256: "3c80c6856a9d1383a4e1f1d2c761ff50578e325e08a1cbdf9860e25181e2df16".into(),
        analysis_spec_sha256: "6e2056ba23c9323308bba57ce8a748270af05270da48373e0c93fb52c274f567".into(),
        oracle_map_sha256: "0bee78dbed3747aae84704437430446e339bfb9275be80c7110e54151f7c6fca".into(),
        execution_binary_sha256: self_sha(),
        execution_source_commit: arg(&args, "--commit").unwrap_or_else(|| "unknown".into()),
        cargo_lock_sha256: "47893d24b4ea9b80cb1198f653931a09b8a15c8a4767a24375beb4337b046ad4".into(),
        toolchain_identity: "rustc 1.96.0 (ac68faa20 2026-05-25); target x86_64-unknown-linux-gnu".into(),
    };
    // executable pins.
    assert!(tail_terminal_id_pin(), "tail terminal-id pin failed");
    assert_eq!(prov.execution_source_commit.len(), 40, "execution_source_commit must be full 40-hex");

    let default_mode = !args.iter().any(|a| a == "--result-bearing");
    if default_mode {
        // startup self-validation: a synthetic canonical record must pass the schema-v2 validator (fail-closed).
        let cells = amended_base_cells();
        let c = cells.iter().find(|c| base_cell_id(c) == "rtt50-exactd1").unwrap();
        let r = run_spine(arm_of("F0"), &build_cfg(c, 0));
        let h = row_hashes(c, "F0", 0);
        let rec = build_record_v2(
            "SELFCHECK", "rtt50-exactd1", "pg--rtt50-exactd1--rep0", 0, 0, classify_v2(&r), &r,
            &prov, "{}".into(), h.base, h.exec, h.behav,
            u32::try_from(deadline_ms_for(c.rtt_ms)).unwrap(),
            u32::try_from(observation_horizon_ms(c.rtt_ms)).unwrap(),
        );
        match validate_v2(&rec.serialize()) {
            Ok(()) => println!("mode=STARTUP_SELF_VALIDATE schema-v2 canonical record PASS; binary_sha={}", self_sha()),
            Err(e) => { eprintln!("STARTUP SELF-VALIDATE FAIL: {e:?}"); std::process::exit(2); }
        }
        return;
    }

    // ── RESULT-BEARING re-run (Aşama-B; authorized by OPT-X2 end-to-end directive) ──
    if arg(&args, "--i-have-launch-authorization").as_deref() != Some("FS1_PHASE2_GO_GRANTED") {
        eprintln!("REFUSED: --i-have-launch-authorization FS1_PHASE2_GO_GRANTED required for result-bearing re-run");
        std::process::exit(3);
    }
    let out = arg(&args, "--out").expect("--out namespace");
    let ledger_p = arg(&args, "--ledger").expect("--ledger");
    // namespace must be empty/absent (append-only firewall).
    if std::path::Path::new(&out).exists()
        && std::fs::read_dir(&out).map(|mut d| d.next().is_some()).unwrap_or(false)
    {
        eprintln!("REFUSED: namespace {out} not empty");
        std::process::exit(3);
    }
    let writer = RawWriter::new(&out);
    let ledger = Ledger::new(&ledger_p);
    let results = run_campaign_v2(&prov, &writer, &ledger);
    let vc = results.iter().filter(|(_, v)| matches!(v, ValidityV2::ValidComplete)).count();
    let cn = results.iter().filter(|(_, v)| matches!(v, ValidityV2::ValidCensored)).count();
    println!("V2 RE-RUN COMPLETE: rows={} valid_complete={vc} valid_censored={cn} binary_sha={}", results.len(), self_sha());
}
