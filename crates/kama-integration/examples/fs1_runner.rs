//! DR-FEC-FS-1 OPT-R1 — manifest-driven FS-1 runner (result-bearing EXECUTOR + persistence). **Default mode =
//! EXECUTABILITY_ONLY** (no loop / no raw / no run-id). **Result-bearing** (`--result-bearing`) YALNIZ TAM verify-chain
//! geçerse çalışır: freeze-lock ↔ binary-self-hash(/proc/self/exe) ↔ final-manifest ↔ candidate-generator-consistency ↔
//! empty-namespace ↔ empty/consistent-ledger. Aksi REFUSE (loop-ilerlemez; raw-yok; run-id-harcanmaz).
//! Acyclic: binary EMBEDS candidate-manifest-sha (binary-sha İÇERMEZ → cycle-YOK); lock→binary/candidate/final; final→binary.
#![allow(clippy::duplicate_mod)]

#[path = "../tests/common/fs_campaign.rs"]
mod fs_campaign;
use fs_campaign::*;

// Acyclic anchor: candidate-manifest (binary-sha İÇERMEZ). Generator self-consistency + lock cross-check.
const EXPECTED_CANDIDATE_MANIFEST_SHA: &str =
    "e9e331a4b25662b728fb667e88066e5d9dc42642a52503cb77227b0ac7f02c10";

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

fn lock_field(lock_text: &str, field: &str) -> Option<String> {
    // "field":"<64hex>" veya "field": "<64hex>"
    let needle = format!("\"{field}\"");
    let idx = lock_text.find(&needle)?;
    let tail = &lock_text[idx + needle.len()..];
    let hx: String = tail
        .chars()
        .skip_while(|c| !c.is_ascii_hexdigit())
        .take_while(char::is_ascii_hexdigit)
        .collect();
    if hx.len() == 64 {
        Some(hx)
    } else {
        None
    }
}

fn self_binary_sha() -> Option<String> {
    let exe = std::fs::read("/proc/self/exe").ok()?;
    Some(sha256_hex(&exe))
}

/// Verify-chain (§1/§13). Ok(Provenance) → execute; Err(reason) → REFUSE.
fn verify_chain(args: &[String]) -> Result<Provenance, String> {
    let lock_p = arg_value(args, "--freeze-lock").ok_or("--freeze-lock zorunlu")?;
    let manifest_p = arg_value(args, "--final-manifest").ok_or("--final-manifest zorunlu")?;
    let out_ns = arg_value(args, "--out").ok_or("--out (namespace) zorunlu")?;
    let ledger_p = arg_value(args, "--ledger").ok_or("--ledger zorunlu")?;

    // (1) generator self-consistency: binary'nin ürettiği candidate-manifest == embedded frozen candidate.
    let gen_sha = sha256_hex(manifest_jsonl().as_bytes());
    if gen_sha != EXPECTED_CANDIDATE_MANIFEST_SHA {
        return Err(format!(
            "generator-candidate-sha {gen_sha} != embedded (binary/generator drift)"
        ));
    }
    // (2) freeze-lock present + parse.
    let lock_text =
        std::fs::read_to_string(&lock_p).map_err(|_| format!("freeze-lock '{lock_p}' YOK"))?;
    let lock_candidate = lock_field(&lock_text, "candidate_manifest_sha256")
        .ok_or("lock candidate_manifest_sha256 YOK")?;
    let lock_final =
        lock_field(&lock_text, "final_manifest_sha256").ok_or("lock final_manifest_sha256 YOK")?;
    let lock_binary = lock_field(&lock_text, "execution_binary_sha256")
        .ok_or("lock execution_binary_sha256 YOK")?;
    let lock_commit = lock_field(&lock_text, "execution_source_commit")
        .or_else(|| Some("unknown".into()))
        .unwrap();
    // (3) lock ↔ embedded candidate.
    if lock_candidate != EXPECTED_CANDIDATE_MANIFEST_SHA {
        return Err("lock.candidate_manifest_sha256 != embedded".into());
    }
    // (4) binary self-hash ↔ lock.
    let self_sha = self_binary_sha().ok_or("self-binary-hash okunamadı")?;
    if self_sha != lock_binary {
        return Err(format!(
            "binary-self-hash {self_sha} != lock.execution_binary_sha256 (wrong binary)"
        ));
    }
    // (5) final-manifest file sha ↔ lock; candidate-manifest supplied → reddet.
    let man_text = std::fs::read_to_string(&manifest_p)
        .map_err(|_| format!("final-manifest '{manifest_p}' YOK"))?;
    let man_sha = sha256_hex(man_text.as_bytes());
    if man_sha == EXPECTED_CANDIDATE_MANIFEST_SHA {
        return Err("candidate-manifest verildi (final-manifest DEĞİL) → reddet".into());
    }
    if man_sha != lock_final {
        return Err(format!(
            "final-manifest-sha {man_sha} != lock.final_manifest_sha256 (wrong manifest)"
        ));
    }
    // (6) namespace empty (append-only firewall).
    if std::path::Path::new(&out_ns).exists()
        && std::fs::read_dir(&out_ns)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!("raw-namespace '{out_ns}' BOŞ-DEĞİL"));
    }
    // (7) ledger empty veya no completed rows.
    let ledger = Ledger::new(&ledger_p);
    if ledger.valid_complete_count() != 0 {
        return Err("primary-ledger BOŞ-DEĞİL (conflicting run)".into());
    }
    Ok(Provenance {
        freeze_lock_sha256: sha256_hex(lock_text.as_bytes()),
        final_manifest_sha256: lock_final,
        execution_binary_sha256: lock_binary,
        execution_source_commit: lock_commit,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--result-bearing") {
        // ── DEFAULT: EXECUTABILITY_ONLY ──
        let s = executability_sweep();
        println!("mode=EXECUTABILITY_ONLY result_bearing=false");
        println!(
            "total={} dispatchable={} unsupported={} ambiguous={} invalid={} duplicate={}",
            s.total, s.dispatchable, s.unsupported, s.ambiguous, s.invalid, s.duplicate
        );
        println!(
            "by-family ExactDeficit={} NaturalIid={} | execution_order_rows={} pairing_groups=60",
            s.by_family.get("ExactDeficit").copied().unwrap_or(0),
            s.by_family.get("NaturalIid").copied().unwrap_or(0),
            execution_order_entries().len()
        );
        if s.dispatchable != 240 || !s.failures.is_empty() {
            eprintln!("EXECUTABILITY FAIL: {:?}", s.failures);
            std::process::exit(2);
        }
        return;
    }

    // ── RESULT-BEARING: TAM verify-chain; aksi REFUSE (execution-YOK) ──
    match verify_chain(&args) {
        Err(reason) => {
            eprintln!("RESULT-BEARING REFUSED: {reason}");
            std::process::exit(3);
        }
        Ok(prov) => {
            // S6-öncesi/S6: launch-authorization AYRICA gerekir. Bu koda ulaşmak için geçerli-freeze-lock +
            // eşleşen-binary + empty-namespace şart (S6-review'de kurulmaz → ulaşılmaz). Gerçek-launch: post-S6
            // explicit-command + --i-have-launch-authorization <path> ile.
            if arg_value(&args, "--i-have-launch-authorization").is_none() {
                eprintln!("RESULT-BEARING verify-chain PASSED ama --i-have-launch-authorization <path> YOK → STOP (S6+ explicit-command gerekir).");
                std::process::exit(4);
            }
            let out_ns = arg_value(&args, "--out").unwrap();
            let ledger_p = arg_value(&args, "--ledger").unwrap();
            let writer = RawWriter::new(&out_ns);
            let ledger = Ledger::new(&ledger_p);
            let results = run_campaign(&prov, &writer, &ledger);
            let vc = results
                .iter()
                .filter(|(_, v)| matches!(v, Validity::ValidComplete))
                .count();
            let cn = results
                .iter()
                .filter(|(_, v)| matches!(v, Validity::ValidCensored))
                .count();
            println!(
                "RESULT-BEARING COMPLETE: rows={} valid_complete={vc} valid_censored={cn}",
                results.len()
            );
        }
    }
}
