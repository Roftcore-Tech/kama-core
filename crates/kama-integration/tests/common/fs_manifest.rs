//! DR-FEC-FS-1 S4-B — FS-1 candidate MANIFEST (240 row) generator + resolved-arm config +
//! resolved_behavior_semantics_hash (RULE-11 4.-hash) + run-id/pairing-group + configured forward/reverse identity.
//! HARNESS-ONLY / DIAGNOSTIC (result-bearing-DEĞİL; v1.3-CANDIDATE-NOT-FROZEN; 360→240 OPT-A). serde-YOK (manual JSON).
#![allow(dead_code)]

#[path = "fs_cell.rs"]
mod fs_cell;
pub use fs_cell::*;

use adaptive_transport::fs_harness::{fs_deadline_ms, fs_effective_source_bps};

// ── PROVENANCE CONSTANTS (candidate; §25 CANDIDATE_NOT_FROZEN) ──────────────────────────────────────
pub const MANIFEST_SCHEMA_VERSION: &str = "FS-1-MANIFEST-v1.3-CANDIDATE";
pub const MANIFEST_GENERATOR_VERSION: u32 = 1;
pub const DOMAIN_DERIVATION_VERSION: u32 = 1;
pub const CANDIDATE_PREREG_STATUS: &str = "CANDIDATE_NOT_FROZEN";
pub const V12_HISTORICAL_HASH: &str =
    "da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034";
pub const S3_CANDIDATE_MATRIX_HASH: &str =
    "399bfcd4b5c75a25c1c036ed56719e0afe725a15f0df2c3d0d53116b67313b11";
pub const SOURCE_COMMIT_REF: &str = "dd73770"; // S4-A (generator provenance)
pub const EXPECTED_HARNESS_VERSION: &str = "fs-spine-S4";
pub const EXPECTED_BINARY_PROVENANCE: &str = "kama-integration@experimental-fs-harness";
pub const SCHEMA_VALIDATOR_VERSION: u32 = 1;
// S5-freeze SONRASI raw-dataset namespace (şu-an YOK; result-bearing-firewall).
pub const RAW_ARTIFACT_NAMESPACE: &str = "reports/fs-1/raw/PENDING-v1.3-FREEZE";

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ── RESOLVED ARM CONFIG (OPT-A arm-set {A0,A1-EARLY,F0,F3}) ─────────────────────────────────────────
pub struct ArmResolved {
    pub selected_k: Option<u16>,
    pub selected_profile_id: Option<u8>,
    pub source_block_count: Option<u16>,
    pub configured_r: u16,
    pub repair_count: u32,
    pub duplicate_policy: &'static str,
    pub close_policy: &'static str,
    pub structural_class: &'static str,
}

#[must_use]
pub fn resolve_arm(arm: &str) -> ArmResolved {
    match arm {
        "A0" => ArmResolved {
            selected_k: None,
            selected_profile_id: None,
            source_block_count: None,
            configured_r: 0,
            repair_count: 0,
            duplicate_policy: "none",
            close_policy: "none",
            structural_class: "N/A",
        },
        "A1-EARLY" => ArmResolved {
            selected_k: None,
            selected_profile_id: None,
            source_block_count: None,
            configured_r: 2,
            repair_count: 0,
            duplicate_policy: "blind-block-total-r2",
            close_policy: "per-source-adjacent-early",
            structural_class: "N/A",
        },
        "F0" => ArmResolved {
            selected_k: Some(8),
            selected_profile_id: Some(0x03),
            source_block_count: Some(6),
            configured_r: 2,
            repair_count: 12,
            duplicate_policy: "none",
            close_policy: "block0-early",
            structural_class: "N/A",
        },
        "F3" => ArmResolved {
            selected_k: Some(4),
            selected_profile_id: Some(0x02),
            source_block_count: Some(12),
            configured_r: 2,
            repair_count: 24,
            duplicate_policy: "none",
            close_policy: "any-block-early",
            structural_class: "BULK_PARITY_WIRE_NULL",
        },
        other => panic!("non-OPT-A arm in FS-1 manifest: {other}"),
    }
}

// ── CONFIGURED FORWARD IDENTITY (ExactDeficit drop-set / IID seed) ──────────────────────────────────
#[must_use]
pub fn forward_config_id(c: &BaseCell, drop_ids: &[u16], trace_seed: u64) -> u64 {
    let mut b: Vec<u8> = b"FS1_FWD_CONFIG_v1".to_vec();
    match c.impairment {
        FsImpairment::ExactDeficit { .. } => {
            b.push(1);
            for &g in drop_ids {
                b.extend_from_slice(&g.to_le_bytes());
            }
        }
        FsImpairment::NaturalIid { loss_permille } => {
            b.push(2);
            b.extend_from_slice(&loss_permille.get().to_le_bytes());
            b.extend_from_slice(&trace_seed.to_le_bytes());
        }
    }
    fnv(&b)
}

/// Reverse = Clear (FS-1 base-matriste cell-defining-DEĞİL; tüm-arm/rep AYNI). configured reverse identity.
#[must_use]
pub fn reverse_config_id() -> u64 {
    fnv(b"FS1_REV_CLEAR_v1")
}

#[must_use]
pub fn a1_target_seed(trace_seed: u64) -> u64 {
    let mut b: Vec<u8> = b"A1_DUP_TARGET".to_vec();
    b.extend_from_slice(&trace_seed.to_le_bytes());
    fnv(&b)
}

/// **RULE-11 4.-hash — resolved_behavior_semantics_hash.** Tüm behavior-affecting resolved-config; arm-display-name
/// / row-id / filename HARİÇ. İki-arm farklı-label AMA aynı-resolved-behavior → aynı-hash (collapse-tespiti).
#[must_use]
pub fn resolved_behavior_semantics_hash(
    c: &BaseCell,
    ar: &ArmResolved,
    fwd_id: u64,
    rev_id: u64,
    deadline_ms: u64,
    trace_seed: u64,
) -> u64 {
    let mut b: Vec<u8> = b"FS1_RESOLVED_BEHAVIOR_v1".to_vec();
    b.extend_from_slice(&base_cell_semantics_hash(c).to_le_bytes()); // workload+RTT+impairment+source-offer
    b.extend_from_slice(&fwd_id.to_le_bytes()); // configured forward identity
    b.extend_from_slice(&rev_id.to_le_bytes()); // configured reverse identity
    b.push(ar.selected_k.map_or(0xFF, |k| k as u8));
    b.push(ar.selected_profile_id.unwrap_or(0));
    b.extend_from_slice(&ar.source_block_count.unwrap_or(0).to_le_bytes());
    b.extend_from_slice(&ar.configured_r.to_le_bytes());
    b.extend_from_slice(&ar.repair_count.to_le_bytes());
    b.extend_from_slice(ar.duplicate_policy.as_bytes());
    b.push(0);
    b.extend_from_slice(ar.close_policy.as_bytes());
    b.push(0);
    b.extend_from_slice(&deadline_ms.to_le_bytes());
    b.extend_from_slice(&trace_seed.to_le_bytes()); // intervention/trace seed authority
    fnv(&b)
}

#[must_use]
pub fn run_id(c: &BaseCell, arm: &str, rep: u8) -> String {
    format!("fs1--{}--{}--rep{}", base_cell_id(c), arm, rep)
}
#[must_use]
pub fn pairing_group_id(c: &BaseCell, rep: u8) -> String {
    format!("pg--{}--rep{}", base_cell_id(c), rep)
}
#[must_use]
pub fn deadline_ms_for(rtt_ms: u64) -> u64 {
    let eff = fs_effective_source_bps(FS1_SOURCE_OFFER_PACER_BPS, FS1_BOTTLENECK_BPS);
    fs_deadline_ms(
        (TOTAL_SOURCE_SYMBOLS_U64) * sealed_source_bytes(),
        eff,
        rtt_ms,
    )
}

const TOTAL_SOURCE_SYMBOLS_U64: u64 = 48;

// ── ROW HASHES (single-source; manifest_row_json + validator-pinler AYNI değerleri kullanır) ──────
pub struct RowHashes {
    pub run_id: String,
    pub base: u64,
    pub exec: u64,
    pub behav: u64,
    pub row: u64,
    pub cfg_sched: u64,
    pub fwd: u64,
    pub rev: u64,
    pub trace_seed: u64,
    pub deadline: u64,
    pub drop_ids: Vec<u16>,
    pub exact_dc: Option<u8>,
}

#[must_use]
pub fn row_hashes(c: &BaseCell, arm: &str, rep: u8) -> RowHashes {
    let ar = resolve_arm(arm);
    let trace_seed = paired_trace_seed(c, rep);
    let deadline = deadline_ms_for(c.rtt_ms);
    let (drop_ids, exact_dc): (Vec<u16>, Option<u8>) = match c.impairment {
        FsImpairment::ExactDeficit { drop_count } => (
            exact_deficit_target_set(drop_count.get(), exact_rank_seed(c), rep),
            Some(drop_count.get()),
        ),
        FsImpairment::NaturalIid { .. } => (Vec::new(), None),
    };
    let fwd = forward_config_id(c, &drop_ids, trace_seed);
    let rev = reverse_config_id();
    let base = base_cell_semantics_hash(c);
    let exec = run_execution_semantics_hash(
        c,
        &ArmConfig {
            arm: leak(arm),
            configured_r: ar.configured_r,
        },
        trace_seed,
        MANIFEST_GENERATOR_VERSION,
    );
    let behav = resolved_behavior_semantics_hash(c, &ar, fwd, rev, deadline, trace_seed);
    let rid = run_id(c, arm, rep);
    let row = row_identity_hash(exec, &rid, exact_dc);
    let cfg_sched = fnv(&[fwd.to_le_bytes(), rev.to_le_bytes()].concat());
    RowHashes {
        run_id: rid,
        base,
        exec,
        behav,
        row,
        cfg_sched,
        fwd,
        rev,
        trace_seed,
        deadline,
        drop_ids,
        exact_dc,
    }
}

// ── ONE MANIFEST ROW (JSON object; deterministik) ──────────────────────────────────────────────────
fn json_u16_array(ids: &[u16]) -> String {
    let inner: Vec<String> = ids.iter().map(u16::to_string).collect();
    format!("[{}]", inner.join(","))
}
fn opt_u16(v: Option<u16>) -> String {
    v.map_or("null".into(), |x| x.to_string())
}
fn opt_u8(v: Option<u8>) -> String {
    v.map_or("null".into(), |x| x.to_string())
}

#[must_use]
pub fn manifest_row_json(c: &BaseCell, arm: &str, rep: u8) -> String {
    let ar = resolve_arm(arm);
    let h = row_hashes(c, arm, rep);
    let iid_lp = c.impairment.configured_iid_loss_permille();
    let a1_seed = if arm == "A1-EARLY" {
        format!("\"0x{:016x}\"", a1_target_seed(h.trace_seed))
    } else {
        "null".into()
    };

    format!(
        concat!(
            "{{",
            "\"schema_version\":\"{sv}\",\"run_id\":\"{rid}\",\"base_cell_id\":\"{bcid}\",",
            "\"pairing_group_id\":\"{pg}\",\"arm\":\"{arm}\",\"repetition_index\":{rep},",
            "\"prereg_status\":\"{prereg}\",\"expected_binary_provenance\":\"{bin}\",",
            "\"expected_harness_version\":\"{hv}\",\"candidate_prereg_version\":\"{sv}\",",
            "\"source_commit\":\"{commit}\",\"v12_historical_hash\":\"{v12}\",",
            "\"s3_candidate_matrix_hash\":\"{s3m}\",",
            "\"app_unit_bytes\":{aub},\"app_unit_count\":{auc},\"total_app_bytes\":{tab},",
            "\"source_symbol_E\":{e},\"source_symbol_count\":{ssc},\"sealed_source_bytes\":{ssb},",
            "\"base_source_wire_bytes\":{bsw},\"canonical_source_sequence_hash\":\"0x{csh:016x}\",",
            "\"rtt_ms\":{rtt},\"bottleneck_capacity_bps\":{bot},\"source_offer_pacer_bps\":{pac},",
            "\"source_offer_schedule_hash\":\"0x{sos:016x}\",\"source_offer_envelope_ns\":{env},\"deadline_ms\":{dl},",
            "\"impairment_kind\":\"{ik}\",\"configured_exact_drop_count\":{edc},",
            "\"exact_global_drop_ids\":{dids},\"exact_drop_set_hash\":{edsh},",
            "\"iid_loss_permille\":{iid},\"forward_trace_seed\":\"0x{fts:016x}\",\"forward_trace_hash\":\"0x{fth:016x}\",",
            "\"reverse_spec\":\"Clear\",\"reverse_schedule_id\":\"0x{rsid:016x}\",\"reverse_trace_seed\":null,",
            "\"reverse_trace_hash\":null,\"configured_blackout_intervals\":[],",
            "\"selected_k\":{sk},\"selected_profile_id\":{spid},\"source_block_count\":{sbc},",
            "\"configured_r_per_block\":{cr},\"repair_count\":{rc},\"duplicate_policy\":\"{dp}\",",
            "\"close_policy\":\"{cp}\",\"expected_structural_classification\":\"{sc}\",",
            "\"trace_seed\":\"0x{fts:016x}\",\"reverse_seed\":null,\"a1_target_seed\":{a1s},",
            "\"domain_derivation_version\":{ddv},",
            "\"base_cell_semantics_hash\":\"0x{bh:016x}\",\"run_execution_semantics_hash\":\"0x{eh:016x}\",",
            "\"resolved_behavior_semantics_hash\":\"0x{rbh:016x}\",\"row_identity_hash\":\"0x{rh:016x}\",",
            "\"configured_schedule_hash\":\"0x{csch:016x}\",",
            "\"canonical_output_path\":\"{raw}/{rid}.json\",\"raw_artifact_namespace\":\"{raw}\",",
            "\"schema_validator_version\":{svv},\"result_bearing\":false",
            "}}"
        ),
        sv = MANIFEST_SCHEMA_VERSION,
        rid = h.run_id,
        bcid = base_cell_id(c),
        pg = pairing_group_id(c, rep),
        arm = arm,
        rep = rep,
        prereg = CANDIDATE_PREREG_STATUS,
        bin = EXPECTED_BINARY_PROVENANCE,
        hv = EXPECTED_HARNESS_VERSION,
        commit = SOURCE_COMMIT_REF,
        v12 = V12_HISTORICAL_HASH,
        s3m = S3_CANDIDATE_MATRIX_HASH,
        aub = APP_UNIT_BYTES_U64,
        auc = APP_UNITS_U64,
        tab = TOTAL_APP_BYTES_U64,
        e = E_U64,
        ssc = TOTAL_SOURCE_SYMBOLS_U64,
        ssb = sealed_source_bytes(),
        bsw = TOTAL_SOURCE_SYMBOLS_U64 * sealed_source_bytes(),
        csh = canonical_sequence_hash(),
        rtt = c.rtt_ms,
        bot = c.bottleneck_bps,
        pac = c.source_offer_pacer_bps,
        sos = source_offer_schedule_id(c),
        env = source_offer_envelope_ns(),
        dl = h.deadline,
        ik = c.impairment.kind(),
        edc = opt_u8(h.exact_dc),
        dids = json_u16_array(&h.drop_ids),
        edsh = if h.drop_ids.is_empty() {
            "null".to_string()
        } else {
            format!("\"0x{:016x}\"", exact_drop_set_hash(&h.drop_ids))
        },
        iid = opt_u16(iid_lp),
        fts = h.trace_seed,
        fth = h.fwd,
        rsid = h.rev,
        sk = opt_u16(ar.selected_k),
        spid = opt_u8(ar.selected_profile_id),
        sbc = opt_u16(ar.source_block_count),
        cr = ar.configured_r,
        rc = ar.repair_count,
        dp = ar.duplicate_policy,
        cp = ar.close_policy,
        sc = ar.structural_class,
        a1s = a1_seed,
        ddv = DOMAIN_DERIVATION_VERSION,
        bh = h.base,
        eh = h.exec,
        rbh = h.behav,
        rh = h.row,
        csch = h.cfg_sched,
        raw = RAW_ARTIFACT_NAMESPACE,
        svv = SCHEMA_VALIDATOR_VERSION,
    )
}

// small consts (u64 mirrors for format).
const APP_UNIT_BYTES_U64: u64 = 300;
const APP_UNITS_U64: u64 = 8;
const TOTAL_APP_BYTES_U64: u64 = 2400;
const E_U64: u64 = 64;

fn leak(s: &str) -> &'static str {
    match s {
        "A0" => "A0",
        "A1-EARLY" => "A1-EARLY",
        "F0" => "F0",
        "F3" => "F3",
        _ => "UNKNOWN",
    }
}

/// source-offer schedule identity (configured; arm-BAĞIMSIZ). 48-elig-time (0.80C) fnv.
#[must_use]
pub fn source_offer_schedule_id(_c: &BaseCell) -> u64 {
    let sealed = sealed_source_bytes();
    let pacer = FS1_SOURCE_OFFER_PACER_BPS;
    let v: Vec<u8> = (0..TOTAL_SOURCE_SYMBOLS_U64)
        .flat_map(|g| {
            let elig = g
                .saturating_mul(sealed)
                .saturating_mul(8)
                .saturating_mul(1_000_000_000)
                / pacer;
            elig.to_le_bytes()
        })
        .collect();
    fnv(&v)
}

#[must_use]
pub fn source_offer_envelope_ns() -> u64 {
    TOTAL_SOURCE_SYMBOLS_U64 * sealed_source_bytes() * 8 * 1_000_000_000
        / FS1_SOURCE_OFFER_PACER_BPS
}

// ── FULL MANIFEST (240 row JSONL; deterministik ordering) ──────────────────────────────────────────
#[must_use]
pub fn manifest_jsonl() -> String {
    let mut s = String::new();
    for c in amended_base_cells() {
        for arm in FS1_RESULT_BEARING_ARMS {
            for rep in 0..FS1_REPS {
                s.push_str(&manifest_row_json(&c, arm, rep));
                s.push('\n');
            }
        }
    }
    s
}

#[must_use]
pub fn manifest_row_count() -> usize {
    manifest_jsonl().lines().count()
}
