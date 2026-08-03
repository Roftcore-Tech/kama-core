//! OPT-X2 §8 — PRE-FROZEN SCIENCE-INVARIANCE COMPARATOR (v1.3.1 ↔ v1.3.2).
//! Matches rows by science-neutral logical key run_id (derived from cell/arm/rep in the frozen manifest/execution-
//! order — NOT from version/binary). Requires 240/240 correspondence, EXACT equality on shared scientific/execution
//! fields, and aggregate 176/64. Provenance/version/binary fields are EXCLUDED (whitelist). Emits ONLY PASS/FAIL,
//! matched/mismatch counts, and deterministic hashes — NEVER outcome values. Outcome values are only mechanically
//! processed here for exact invariance; no value is emitted and no scientific interpretation is made.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn read_dir_records(ns: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    if let Ok(rd) = fs::read_dir(ns) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.ends_with(".json") && !name.ends_with(".sha256") {
                if let Ok(body) = fs::read_to_string(&p) {
                    if let Some(rid) = field(&body, "run_id") {
                        m.insert(rid.trim_matches('"').to_string(), body);
                    }
                }
            }
        }
    }
    m
}

/// scalar value token for a top-level key (quoted string or bare number/bool/null). depth-1 only.
fn field(json: &str, key: &str) -> Option<String> {
    let b = json.as_bytes();
    let (mut i, mut depth) = (0usize, 0i32);
    let target = key.as_bytes();
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b'"' if depth == 1 => {
                let (s, ni) = read_str(b, i);
                let mut j = ni;
                while j < b.len() && (b[j] == b' ' || b[j] == b'\n') {
                    j += 1;
                }
                if j < b.len() && b[j] == b':' {
                    if s.as_bytes() == target {
                        return Some(read_val(&b[j + 1..]));
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
fn read_str(b: &[u8], start: usize) -> (String, usize) {
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
fn read_val(b: &[u8]) -> String {
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\n') {
        i += 1;
    }
    if i >= b.len() {
        return String::new();
    }
    if b[i] == b'"' {
        let (s, _) = read_str(b, i);
        return s;
    }
    if b[i] == b'{' {
        // object (e.g. completion_or_censoring): return the inner status/time via nested scan by caller instead.
        let mut depth = 0;
        let st = i;
        while i < b.len() {
            if b[i] == b'{' {
                depth += 1;
            } else if b[i] == b'}' {
                depth -= 1;
                if depth == 0 {
                    return String::from_utf8_lossy(&b[st..=i]).to_string();
                }
            }
            i += 1;
        }
    }
    let st = i;
    while i < b.len() && b[i] != b',' && b[i] != b'}' {
        i += 1;
    }
    String::from_utf8_lossy(&b[st..i]).trim().to_string()
}

/// FNV-1a over the ordered shared-field tuple (deterministic dataset-invariance digest; no values leaked in output).
fn fnv(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &c in s.as_bytes() {
        h ^= u64::from(c);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// canonical shared-scientific tuple string for a record (old or new schema), using the field alias map.
fn shared_tuple(json: &str, new_schema: bool) -> String {
    let g = |k: &str| field(json, k).unwrap_or_else(|| "MISSING".into());
    let completion = if new_schema {
        g("t_full_workload_complete")
    } else {
        g("completion_ns")
    };
    let intervention = if new_schema {
        g("proactive_intervention_bytes")
    } else {
        g("intervention_bytes")
    };
    // shared scientific/execution fields present in BOTH schemas (provenance/version/binary EXCLUDED).
    format!(
        "vc={};dh={};df={};db={};re={};comp={};bsb={};ib={};arq={};tob={};rtx={};fec={}",
        g("validity_class"),
        g("deadline_hit"),
        g("delivered_fragments"),
        g("delivered_bytes"),
        g("reassembly_equal"),
        completion,
        g("base_source_bytes"),
        intervention,
        g("arq_retransmit_bytes"),
        g("total_outcome_bytes"),
        g("retransmits"),
        g("fec_decoded_sources"),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let old_ns = arg(&args, "--old").unwrap_or_else(|| "reports/fs-1/raw/v1.3.1".into());
    let new_ns = arg(&args, "--new").unwrap_or_else(|| "reports/fs-1/raw/v1.3.2".into());
    if !Path::new(&old_ns).exists() || !Path::new(&new_ns).exists() {
        eprintln!("COMPARATOR ERROR: dataset namespace missing (old={old_ns} new={new_ns})");
        std::process::exit(2);
    }
    let old = read_dir_records(&old_ns);
    let new = read_dir_records(&new_ns);

    let mut matched = 0u32;
    let mut mismatch: Vec<String> = Vec::new();
    let mut digest_old = 0u64;
    let mut digest_new = 0u64;
    let (mut vc_old, mut cn_old, mut vc_new, mut cn_new) = (0u32, 0u32, 0u32, 0u32);

    // 240/240 correspondence.
    let mut keys: Vec<&String> = old.keys().collect();
    keys.sort();
    for rid in &keys {
        let Some(nrec) = new.get(*rid) else {
            mismatch.push(format!("{rid}:MISSING_IN_NEW"));
            continue;
        };
        let orec = &old[*rid];
        matched += 1;
        let to = shared_tuple(orec, false);
        let tn = shared_tuple(nrec, true);
        digest_old ^= fnv(&format!("{rid}|{to}"));
        digest_new ^= fnv(&format!("{rid}|{tn}"));
        if to != tn {
            // report ONLY which shared fields differ (names), never the values.
            let diff_fields: Vec<&str> = to
                .split(';')
                .zip(tn.split(';'))
                .filter(|(a, b)| a != b)
                .map(|(a, _)| a.split('=').next().unwrap_or("?"))
                .collect();
            mismatch.push(format!("{rid}:{}", diff_fields.join(",")));
        }
        match field(orec, "validity_class").as_deref() {
            Some("VALID_COMPLETE") => vc_old += 1,
            Some("VALID_CENSORED") => cn_old += 1,
            _ => {}
        }
        match field(nrec, "validity_class").as_deref() {
            Some("VALID_COMPLETE") => vc_new += 1,
            Some("VALID_CENSORED") => cn_new += 1,
            _ => {}
        }
    }
    for rid in new.keys() {
        if !old.contains_key(rid) {
            mismatch.push(format!("{rid}:MISSING_IN_OLD"));
        }
    }

    let agg_ok = vc_old == vc_new && cn_old == cn_new && vc_new == 176 && cn_new == 64;
    let corr_ok = matched == 240 && old.len() == 240 && new.len() == 240;
    let field_ok = mismatch.is_empty();
    let pass = agg_ok && corr_ok && field_ok;

    println!("SCIENCE-INVARIANCE COMPARATOR");
    println!("matched_rows={matched} old_count={} new_count={}", old.len(), new.len());
    println!("aggregate: complete_old={vc_old} complete_new={vc_new} censored_old={cn_old} censored_new={cn_new} (expected 176/64)");
    println!("shared_field_mismatch_rows={}", mismatch.len());
    println!("shared_digest_old=0x{digest_old:016x} shared_digest_new=0x{digest_new:016x} digests_equal={}", digest_old == digest_new);
    if !mismatch.is_empty() {
        println!("--- mismatch identities (logical-row:field-names ONLY; NO values) ---");
        for m in mismatch.iter().take(50) {
            println!("  {m}");
        }
    }
    println!("VERDICT={}", if pass { "PASS" } else { "FAIL" });
    if !pass {
        std::process::exit(1);
    }
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}
