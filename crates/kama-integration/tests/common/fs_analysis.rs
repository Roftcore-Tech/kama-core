//! DR-FEC-FS-1 S5-B — outcome-blind ANALYSIS SKELETON + validators (FS-ANALYSIS-SPEC-v1). Synthetic/diagnostic/
//! schema-fixture ONLY — campaign-data INGEST-YOK (S5 pre-outcome). classify/pair/fairness/oracle/leakage-firewall.
#![allow(dead_code)]

#[path = "fs_manifest.rs"]
mod fs_manifest;
pub use fs_manifest::*;
use std::collections::BTreeSet;

/// §5 top-level validity states (frozen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidityClass {
    Invalid,
    ValidComplete,
    ValidCensored,
}

/// Synthetic attempt-record (analysis-skeleton; campaign-data DEĞİL).
#[derive(Debug, Clone)]
pub struct AttemptRecord {
    pub run_id: String,
    pub arm: &'static str,
    pub pairing_group: String,
    pub attempt_index: u8,
    pub provenance_ok: bool, // freeze-lock/manifest/binary/trace/source-hash match
    pub schema_ok: bool,
    pub delivered_fragments: u16,
    pub delivered_bytes: u32,
    pub completed_before_horizon: bool,
    pub intervention_bytes: u32,
    pub total_bytes: u32,
    pub tail_included: bool,
}

#[must_use]
pub fn classify_validity(r: &AttemptRecord) -> ValidityClass {
    if !r.provenance_ok || !r.schema_ok {
        return ValidityClass::Invalid;
    }
    if r.completed_before_horizon && r.delivered_fragments == 48 && r.delivered_bytes == 2400 {
        ValidityClass::ValidComplete
    } else {
        ValidityClass::ValidCensored // legitimate-incomplete (tail-stall dahil); INVALID-DEĞİL
    }
}

/// F1/F4 result-bearing-DIŞI (OPT-A; diagnostic-only). result-dataset'ten reddedilir.
#[must_use]
pub fn is_result_bearing_arm(arm: &str) -> bool {
    matches!(arm, "A0" | "A1-EARLY" | "F0" | "F3")
}

/// §17 coding-causal fairness: FEC-arm vs A1 realized intervention-wire-byte parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FairnessClass {
    StrictlyDominated,
    NonDominated,
    BudgetMismatch,
    NoCodingCausalClaim,
}

/// realized_sealed_wire_bytes parity (C-P4; v1.2 A1.2). eşit-değil → BUDGET_MISMATCH → coding-causal YASAK.
#[must_use]
pub fn coding_causal_fairness(
    fec_intervention_bytes: u32,
    a1_intervention_bytes: u32,
) -> FairnessClass {
    if fec_intervention_bytes != a1_intervention_bytes {
        FairnessClass::BudgetMismatch // realized-wire eşit-değil → no-coding-causal-attribution
    } else {
        FairnessClass::NonDominated // matched-wire → causal-karşılaştırma-uygun (dominance outcome'a-göre)
    }
}

/// §12 oracle outcome classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleClass {
    NoRedundancyValue, // O0
    RedundancyValue,   // O1
    FecSpecificValue,  // O2
    Insufficient,      // O3 / ABSTAIN
}

/// O2 (FEC-specific) YALNIZ: FEC>A1 + matched-realized-wire (NonDominated). aksi O1/O3. FEC>A0-tek-başına O2-DEĞİL.
#[must_use]
pub fn oracle_class(
    a1_beats_a0: bool,
    fec_beats_a0: bool,
    fec_beats_a1: bool,
    fairness: FairnessClass,
    sufficient: bool,
) -> OracleClass {
    if !sufficient {
        return OracleClass::Insufficient;
    }
    if fec_beats_a1 && fairness == FairnessClass::NonDominated {
        OracleClass::FecSpecificValue
    } else if a1_beats_a0 || fec_beats_a0 {
        OracleClass::RedundancyValue
    } else {
        OracleClass::NoRedundancyValue
    }
}

/// §20 pre-decision feature whitelist (leakage-firewall). realized-loss/completion/deadline/outcome → YASAK.
#[must_use]
pub fn is_allowed_decision_feature(feature: &str) -> bool {
    const FORBIDDEN: &[&str] = &[
        "realized_loss",
        "realized_initial_source_loss",
        "completion",
        "deadline_hit",
        "delivered_bytes",
        "delivered_fragments",
        "validity_class",
        "censored",
        "reassembly",
        "fec_decoded",
    ];
    !FORBIDDEN.iter().any(|f| feature.contains(f))
}

/// §14 pairing completeness: 4 result-bearing arm hepsi non-INVALID mevcut mu.
#[must_use]
pub fn pairing_complete(records: &[AttemptRecord]) -> bool {
    let arms: BTreeSet<&str> = records
        .iter()
        .filter(|r| classify_validity(r) != ValidityClass::Invalid)
        .map(|r| r.arm)
        .collect();
    FS1_RESULT_BEARING_ARMS.iter().all(|a| arms.contains(a))
}

/// RULE-17: F4 derived/collapsed diagnostic-policy → independent-campaign-evidence DEĞİL.
#[must_use]
pub fn f4_is_independent_evidence() -> bool {
    false
}

/// duplicate-output tespiti: aynı (run_id, attempt_index) iki-kez.
#[must_use]
pub fn has_duplicate_output(records: &[AttemptRecord]) -> bool {
    let mut seen = BTreeSet::new();
    for r in records {
        if !seen.insert((r.run_id.clone(), r.attempt_index)) {
            return true;
        }
    }
    false
}

/// §6 retry legitimacy: yalnız-INVALID retry-edilebilir; VALID_* için retry YASAK.
#[must_use]
pub fn retry_permitted(prior: &AttemptRecord) -> bool {
    classify_validity(prior) == ValidityClass::Invalid && prior.attempt_index < 2
    // primary(0)+2 retry → max index 2
}
