//! DR-FEC-FS-1 S3-R0 — TYPED IMPAIRMENT SUM-TYPE + SEMANTIC CELL-UNIQUENESS (ALT-1 amendment).
//! HARNESS-ONLY / DIAGNOSTIC (config+coverage layer; result-bearing DEĞİL). Bu modül RUN-execution'a HENÜZ
//! wire-EDİLMEZ (R2 NaturalIid-plumbing). Amaç:
//!  (a) impairment = SUM-TYPE → geçersiz-kombinasyon (NaturalIid+deficit / ExactDeficit-drop_count'suz /
//!      variant-yabancı-param) TİP-DÜZEYİNDE temsil-EDİLEMEZ (RULE-9);
//!  (b) üç-katman semantik-hash (BASE_CELL / RUN_EXECUTION / ROW_IDENTITY) → RULE-8 semantic-cell-uniqueness;
//!  (c) ALT-1 12-hücre amended matris + tarihsel 18-satır duplicate-tespiti (S3-A DUR-B'nin makineleşmesi).
//! deficit ARTIK top-level ambiguous field DEĞİL — variant-local (ExactDeficit.drop_count). IID20 deficit-inert.
#![allow(dead_code)]

#[path = "fs_workload.rs"]
mod fs_workload;
use fs_workload::{
    canonical_workload, sequence_hash, APP_UNITS, APP_UNIT_BYTES, E, TOTAL_APP_BYTES,
    TOTAL_SOURCE_SYMBOLS,
};

use adaptive_transport::fs_harness::fs_sealed_len;
use std::collections::BTreeSet;

/// Canonical 48-symbol source-sequence hash (workload-invariant; tüm-hücre AYNI). Manifest workload-field'i.
#[must_use]
pub fn canonical_sequence_hash() -> u64 {
    sequence_hash(&canonical_workload())
}

/// Sealed source-symbol bytes (E+37 = 101). Manifest/pacing için tek-otorite.
#[must_use]
pub fn sealed_source_bytes() -> u64 {
    fs_sealed_len(E as u64)
}

/// Canonical source-symbol count (48; TOTAL_SOURCE_SYMBOLS'e bağlı; pub-re-export).
pub const FS1_SOURCE_SYMBOLS: u16 = TOTAL_SOURCE_SYMBOLS as u16;

// ── FS-1 FROZEN COVERAGE CONSTANTS (ALT-1 amended; Abkarada-ratified) ──────────────────────────────
pub const FS1_RTT_MS: [u64; 3] = [50, 250, 1000];
pub const FS1_EXACT_DEFICITS: [u8; 3] = [1, 2, 4];
pub const FS1_IID_LOSS_PERMILLE: u16 = 200; // p = 0.20 SABİT (matched-deficit / sweep YASAK; S3-A)
pub const FS1_BOTTLENECK_BPS: u64 = 1_000_000;
pub const FS1_SOURCE_OFFER_PACER_BPS: u64 = 800_000; // 0.80 × C (bottleneck-DÜŞÜRME değil; §13)
pub const FS1_ARMS: [&str; 6] = ["A0", "A1-EARLY", "F0", "F1", "F3", "F4"];
pub const FS1_REPS: u8 = 5;

// ── TYPED-VALIDATION ERRORS (§5 temsil-edilemez-invariant kapısı) ──────────────────────────────────
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CellConfigError {
    #[error("ExactDeficit drop_count {0} ∉ {{1,2,4}}")]
    InvalidDropCount(u8),
    #[error("NaturalIid loss_permille {0} != 200 (FS-1 sabit p=0.20; sweep YASAK)")]
    InvalidLossPermille(u16),
    #[error("NaturalIid + deficit=Some({0}) GEÇERSİZ (deficit IID20 altında inert; S3-A DUR-B / RULE-9)")]
    IidWithDeficit(u8),
    #[error("ExactDeficit drop_count YOK (variant-local param zorunlu)")]
    ExactDeficitWithoutCount,
    #[error("ExactDeficit + iid_loss_permille=Some({0}) GEÇERSİZ (variant-yabancı param; RULE-9)")]
    ExactDeficitWithIid(u16),
    #[error("bilinmeyen impairment_kind '{0}'")]
    UnknownKind(String),
}

// ── VARIANT-LOCAL NEWTYPES (validated) ─────────────────────────────────────────────────────────────
/// exact-deficit drop-count ∈ {1,2,4}. YALNIZ ExactDeficit variantına özgü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactDeficitCount(u8);
impl ExactDeficitCount {
    pub fn new(d: u8) -> Result<Self, CellConfigError> {
        match d {
            1 | 2 | 4 => Ok(Self(d)),
            _ => Err(CellConfigError::InvalidDropCount(d)),
        }
    }
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

/// IID loss-permille (FS-1 sabit 200 = p 0.20). YALNIZ NaturalIid variantına özgü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossPermille(u16);
impl LossPermille {
    pub fn new(p: u16) -> Result<Self, CellConfigError> {
        if p == FS1_IID_LOSS_PERMILLE {
            Ok(Self(p))
        } else {
            Err(CellConfigError::InvalidLossPermille(p))
        }
    }
    #[must_use]
    pub fn get(self) -> u16 {
        self.0
    }
}

// ── IMPAIRMENT SUM-TYPE (§5; RULE-9) ───────────────────────────────────────────────────────────────
/// **Impairment SUM-TYPE.** Variant-local param'lar; geçersiz-kombinasyon TEMSİL-EDİLEMEZ:
///  - `ExactDeficit`: drop_count ∈ {1,2,4} · iid_loss_permille = N/A
///  - `NaturalIid`  : loss_permille = 200 · exact_deficit = N/A  (deficit IID20-inert; S3-A DUR-B)
///
/// `NaturalIid{ deficit }` yapılandırması BU TİPTE TEMSİL-EDİLEMEZ (variant deficit-field-siz).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsImpairment {
    ExactDeficit { drop_count: ExactDeficitCount },
    NaturalIid { loss_permille: LossPermille },
}

impl FsImpairment {
    pub fn exact_deficit(d: u8) -> Result<Self, CellConfigError> {
        Ok(Self::ExactDeficit {
            drop_count: ExactDeficitCount::new(d)?,
        })
    }
    pub fn natural_iid(p: u16) -> Result<Self, CellConfigError> {
        Ok(Self::NaturalIid {
            loss_permille: LossPermille::new(p)?,
        })
    }
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ExactDeficit { .. } => "ExactDeficit",
            Self::NaturalIid { .. } => "NaturalIid",
        }
    }
    /// §6 result-schema: variant-local configured accessor'ı (ExactDeficit → Some(d); IID → None).
    #[must_use]
    pub fn configured_exact_drop_count(&self) -> Option<u8> {
        match self {
            Self::ExactDeficit { drop_count } => Some(drop_count.get()),
            Self::NaturalIid { .. } => None,
        }
    }
    /// §6 result-schema: variant-local configured accessor'ı (IID → Some(200); ExactDeficit → None).
    #[must_use]
    pub fn configured_iid_loss_permille(&self) -> Option<u16> {
        match self {
            Self::NaturalIid { loss_permille } => Some(loss_permille.get()),
            Self::ExactDeficit { .. } => None,
        }
    }
}

/// **§5 TYPED-VALIDATION GATE.** Bir düz/serbest `(impairment_kind, drop_count?, iid_loss_permille?)` girişini
/// (manifest/CLI satırı benzeri) SUM-TYPE'a çevirir; GEÇERSİZ-kombinasyonu typed-error ile REDDEDER.
/// NaturalIid+deficit / ExactDeficit-count'suz / variant-yabancı-param → hata (temsil-edilemez-invariant kapısı).
pub fn parse_impairment(
    kind: &str,
    drop_count: Option<u8>,
    iid_loss_permille: Option<u16>,
) -> Result<FsImpairment, CellConfigError> {
    match kind {
        "ExactDeficit" => {
            if let Some(p) = iid_loss_permille {
                return Err(CellConfigError::ExactDeficitWithIid(p));
            }
            let d = drop_count.ok_or(CellConfigError::ExactDeficitWithoutCount)?;
            FsImpairment::exact_deficit(d)
        }
        "NaturalIid" => {
            if let Some(d) = drop_count {
                return Err(CellConfigError::IidWithDeficit(d));
            }
            FsImpairment::natural_iid(iid_loss_permille.unwrap_or(FS1_IID_LOSS_PERMILLE))
        }
        other => Err(CellConfigError::UnknownKind(other.to_string())),
    }
}

// ── BASE CELL + THREE-LEVEL SEMANTIC HASHES (§7) ───────────────────────────────────────────────────
/// Reverse'in cell-defining olup-olmadığı. FS-1 base-matriste reverse=Clear → cell-defining DEĞİL
/// (reverse-pairing S3-R1 AYRI eksen). cell-defining ise base-cell-hash'e girer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverseCellSpec {
    Clear,
    ScheduledId(u64),
}

/// **Bilimsel base-cell** (RULE-8 çekirdeği). arm / rep / seed / row-id / label İÇERMEZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseCell {
    pub rtt_ms: u64,
    pub impairment: FsImpairment,
    pub bottleneck_bps: u64,
    pub source_offer_pacer_bps: u64,
    pub reverse: ReverseCellSpec,
}

/// Arm-config (execution-affecting). configured_r = S1/S2-otoritesi (A0=0; diğer arm r). §9: DEĞİŞMEZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmConfig {
    pub arm: &'static str,
    pub configured_r: u16,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn push_impairment(b: &mut Vec<u8>, imp: &FsImpairment) {
    // deficit YALNIZ ExactDeficit'e girer; NaturalIid'de (loss_permille) — deficit GİRMEZ (S3-A çekirdeği).
    match imp {
        FsImpairment::ExactDeficit { drop_count } => {
            b.push(0x01);
            b.push(drop_count.get());
        }
        FsImpairment::NaturalIid { loss_permille } => {
            b.push(0x02);
            b.extend_from_slice(&loss_permille.get().to_le_bytes());
        }
    }
}

/// **A. BASE_CELL_SEMANTICS_HASH** — yalnız bilimsel base-cell: RTT · workload-geometri · impairment-variant +
/// variant-param · util/source-offer · (cell-defining) reverse. HARİÇ: arm · rep · seed · row-id · label.
#[must_use]
pub fn base_cell_semantics_hash(c: &BaseCell) -> u64 {
    let mut b: Vec<u8> = Vec::new();
    b.push(0xB1); // domain tag: base-cell
    b.extend_from_slice(&c.rtt_ms.to_le_bytes());
    // workload-geometri (sabit; yine de commit → grounding-chain / RULE-2).
    b.extend_from_slice(&(TOTAL_SOURCE_SYMBOLS as u64).to_le_bytes());
    b.extend_from_slice(&(E as u64).to_le_bytes());
    b.extend_from_slice(&(TOTAL_APP_BYTES as u64).to_le_bytes());
    b.extend_from_slice(&(APP_UNITS as u64).to_le_bytes());
    b.extend_from_slice(&(APP_UNIT_BYTES as u64).to_le_bytes());
    b.extend_from_slice(&fs_sealed_len(E as u64).to_le_bytes());
    push_impairment(&mut b, &c.impairment);
    b.extend_from_slice(&c.bottleneck_bps.to_le_bytes());
    b.extend_from_slice(&c.source_offer_pacer_bps.to_le_bytes());
    match c.reverse {
        ReverseCellSpec::Clear => b.push(0x10),
        ReverseCellSpec::ScheduledId(id) => {
            b.push(0x11);
            b.extend_from_slice(&id.to_le_bytes());
        }
    }
    fnv1a(&b)
}

/// **B. RUN_EXECUTION_SEMANTICS_HASH** — base-cell + arm + arm-config + seed + config-version.
/// HARİÇ: filename · row-display-label · result-outcome.
#[must_use]
pub fn run_execution_semantics_hash(
    c: &BaseCell,
    arm: &ArmConfig,
    seed: u64,
    config_version: u32,
) -> u64 {
    let mut b: Vec<u8> = Vec::new();
    b.push(0xE2);
    b.extend_from_slice(&base_cell_semantics_hash(c).to_le_bytes());
    b.extend_from_slice(arm.arm.as_bytes());
    b.push(0x00);
    b.extend_from_slice(&arm.configured_r.to_le_bytes());
    b.extend_from_slice(&seed.to_le_bytes());
    b.extend_from_slice(&config_version.to_le_bytes());
    fnv1a(&b)
}

/// **C. ROW_IDENTITY_HASH** — manifest-row-kimliği + presentation-metadata (display-label dahil).
/// AYNI run_execution_semantics_hash + FARKLI row_identity_hash = duplicate-execution-row (RULE-8).
#[must_use]
pub fn row_identity_hash(
    exec_hash: u64,
    row_label: &str,
    display_deficit_label: Option<u8>,
) -> u64 {
    let mut b: Vec<u8> = Vec::new();
    b.push(0xC3);
    b.extend_from_slice(&exec_hash.to_le_bytes());
    b.extend_from_slice(row_label.as_bytes());
    b.push(0x00);
    if let Some(d) = display_deficit_label {
        b.push(0xDD);
        b.push(d);
    }
    fnv1a(&b)
}

// ── AMENDED (12) + HISTORICAL (18) MATRICES ────────────────────────────────────────────────────────
/// **ALT-1 amended base-matris = 12 hücre** (9 ExactDeficit + 3 NaturalIid).
#[must_use]
pub fn amended_base_cells() -> Vec<BaseCell> {
    let mut out = Vec::new();
    for &rtt in &FS1_RTT_MS {
        for &d in &FS1_EXACT_DEFICITS {
            out.push(BaseCell {
                rtt_ms: rtt,
                impairment: FsImpairment::exact_deficit(d).unwrap(),
                bottleneck_bps: FS1_BOTTLENECK_BPS,
                source_offer_pacer_bps: FS1_SOURCE_OFFER_PACER_BPS,
                reverse: ReverseCellSpec::Clear,
            });
        }
    }
    for &rtt in &FS1_RTT_MS {
        out.push(BaseCell {
            rtt_ms: rtt,
            impairment: FsImpairment::natural_iid(FS1_IID_LOSS_PERMILLE).unwrap(),
            bottleneck_bps: FS1_BOTTLENECK_BPS,
            source_offer_pacer_bps: FS1_SOURCE_OFFER_PACER_BPS,
            reverse: ReverseCellSpec::Clear,
        });
    }
    out
}

/// Tarihsel 18-satır candidate grid (RTT × deficit-label × impairment). IID satırları d-LABEL taşır AMA
/// impairment=NaturalIid (label INERT; S3-A). base-cell-hash label'ı YOK-SAYAR → IID d-satırları çöker.
pub struct HistoricalRow {
    pub rtt_ms: u64,
    pub deficit_label: u8,
    pub impairment: FsImpairment,
    pub row_label: String,
}

#[must_use]
pub fn historical_base_rows() -> Vec<HistoricalRow> {
    let mut out = Vec::new();
    for &rtt in &FS1_RTT_MS {
        for &d in &FS1_EXACT_DEFICITS {
            out.push(HistoricalRow {
                rtt_ms: rtt,
                deficit_label: d,
                impairment: FsImpairment::exact_deficit(d).unwrap(), // label == drop_count
                row_label: format!("rtt{rtt}-d{d}-exact"),
            });
        }
        for &d in &FS1_EXACT_DEFICITS {
            out.push(HistoricalRow {
                rtt_ms: rtt,
                deficit_label: d, // d-LABEL taşınır AMA impairment IID (label inert → duplicate)
                impairment: FsImpairment::natural_iid(FS1_IID_LOSS_PERMILLE).unwrap(),
                row_label: format!("rtt{rtt}-d{d}-iid20"),
            });
        }
    }
    out
}

#[must_use]
pub fn base_cell_of(row: &HistoricalRow) -> BaseCell {
    BaseCell {
        rtt_ms: row.rtt_ms,
        impairment: row.impairment,
        bottleneck_bps: FS1_BOTTLENECK_BPS,
        source_offer_pacer_bps: FS1_SOURCE_OFFER_PACER_BPS,
        reverse: ReverseCellSpec::Clear,
    }
}

#[must_use]
pub fn unique_base_hashes(cells: &[BaseCell]) -> BTreeSet<u64> {
    cells.iter().map(base_cell_semantics_hash).collect()
}

/// **§26 CANDIDATE-MATRIX artifact (v1.3-candidate; NON-FROZEN; deterministik JSON).** 12 base-cell
/// (9 ExactDeficit + 3 NaturalIid) + base_cell_semantics_hash'ler + projected 360-run. serde-YOK (manual,
/// canonical). Bu S4-manifest DEĞİL; 360 result-row ÜRETİLMEZ.
#[must_use]
pub fn candidate_matrix_json() -> String {
    let cells = amended_base_cells();
    let uniq = unique_base_hashes(&cells);
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str("  \"artifact\": \"FS-1-CELL-MATRIX-v1.3-CANDIDATE\",\n");
    s.push_str("  \"frozen\": false,\n");
    s.push_str("  \"amendment\": \"ALT-1\",\n");
    s.push_str(
        "  \"note\": \"S4-manifest DEGIL; 360 result-row URETILMEZ; v1.3-freeze S3'te YOK.\",\n",
    );
    s.push_str(&format!("  \"base_cell_count\": {},\n", cells.len()));
    s.push_str(&format!(
        "  \"unique_base_cell_semantics\": {},\n",
        uniq.len()
    ));
    s.push_str(&format!(
        "  \"duplicate_base_cells\": {},\n",
        cells.len() - uniq.len()
    ));
    s.push_str("  \"invalid_variant_combos\": 0,\n");
    s.push_str(&format!("  \"arms\": {},\n", FS1_ARMS.len()));
    s.push_str(&format!("  \"reps\": {},\n", FS1_REPS));
    s.push_str(&format!(
        "  \"projected_result_bearing_runs\": {},\n",
        cells.len() * FS1_ARMS.len() * FS1_REPS as usize
    ));
    s.push_str("  \"cells\": [\n");
    for (i, c) in cells.iter().enumerate() {
        let (kind, dc, lp) = match c.impairment {
            FsImpairment::ExactDeficit { drop_count } => (
                "ExactDeficit",
                format!("{}", drop_count.get()),
                "null".into(),
            ),
            FsImpairment::NaturalIid { loss_permille } => (
                "NaturalIid",
                "null".into(),
                format!("{}", loss_permille.get()),
            ),
        };
        let comma = if i + 1 < cells.len() { "," } else { "" };
        s.push_str(&format!(
            "    {{ \"index\": {}, \"rtt_ms\": {}, \"impairment\": \"{}\", \"exact_drop_count\": {}, \"iid_loss_permille\": {}, \"bottleneck_bps\": {}, \"source_offer_pacer_bps\": {}, \"base_cell_semantics_hash\": \"0x{:016x}\" }}{}\n",
            i, c.rtt_ms, kind, dc, lp, c.bottleneck_bps, c.source_offer_pacer_bps,
            base_cell_semantics_hash(c), comma
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

// ── S4-A: RESULT-BEARING ARM-SET (OPT-A) + REPETITION LINEAGE (frozen, outcome-blind; RULE-10/11) ──
/// OPT-A result-bearing arm-set (F1/F4 collapse → DIAGNOSTIC-only; FS-1-CROSS-ARM-BEHAVIORAL-COLLAPSE-AUDIT).
pub const FS1_RESULT_BEARING_ARMS: [&str; 4] = ["A0", "A1-EARLY", "F0", "F3"];
/// OPT-A: result-bearing-DIŞI (mechanism-validation diagnostic; §11).
pub const FS1_DIAGNOSTIC_ARMS: [&str; 2] = ["F1", "F4"];
/// OPT-A projected result-bearing = 12 × 4 × 5 = 240 (SUPERSEDES 360).
pub const FS1_RESULT_BEARING_RUNS: usize = 12 * 4 * 5;

/// Base-cell readable id (deterministik; scientific-semantik typed-field/hash'ten gelir, string-parse'tan DEĞİL).
#[must_use]
pub fn base_cell_id(c: &BaseCell) -> String {
    match c.impairment {
        FsImpairment::ExactDeficit { drop_count } => {
            format!("rtt{}-exactd{}", c.rtt_ms, drop_count.get())
        }
        FsImpairment::NaturalIid { .. } => format!("rtt{}-iid20", c.rtt_ms),
    }
}

/// Paired trace-seed (base-cell + repetition). Distinct-rep → distinct-seed → distinct-lineage (RULE-10).
/// Outcome-blind (yalnız base-cell-semantics + rep). Frozen domain "FS1_PAIRED_TRACE_v1".
#[must_use]
pub fn paired_trace_seed(c: &BaseCell, rep: u8) -> u64 {
    let mut b: Vec<u8> = b"FS1_PAIRED_TRACE_v1".to_vec();
    b.extend_from_slice(&base_cell_semantics_hash(c).to_le_bytes());
    b.push(rep);
    fnv1a(&b)
}

/// Per-cell ExactDeficit ranking seed (rep-BAĞIMSIZ; cell-sabit). rep, non-overlapping window seçer.
/// Frozen domain "FS1_EXACT_RANK_v1"; outcome-blind (yalnız base-cell-semantics).
#[must_use]
pub fn exact_rank_seed(c: &BaseCell) -> u64 {
    let mut b: Vec<u8> = b"FS1_EXACT_RANK_v1".to_vec();
    b.extend_from_slice(&base_cell_semantics_hash(c).to_le_bytes());
    fnv1a(&b)
}

/// ExactDeficit hedef-seti: 48-gid cell-ranking (rank_seed-keyed) + rep NON-OVERLAPPING window
/// `[rep*d, (rep+1)*d)`. 5-rep → 5 GARANTİLİ-distinct set (max rep*d = 4*4 = 16 < 48). deterministik;
/// outcome-blind; arm-BAĞIMSIZ; per-block-çarpım-YOK. Sıralı döner. distinctness-by-construction (§5).
#[must_use]
pub fn exact_deficit_target_set(drop_count: u8, rank_seed: u64, rep: u8) -> Vec<u16> {
    let mut ranked: Vec<(u64, u16)> = (0..TOTAL_SOURCE_SYMBOLS as u16)
        .map(|g| {
            let mut b = rank_seed.to_le_bytes().to_vec();
            b.extend_from_slice(&g.to_le_bytes());
            (fnv1a(&b), g)
        })
        .collect();
    ranked.sort_unstable();
    let start = rep as usize * drop_count as usize;
    let mut out: Vec<u16> = ranked[start..start + drop_count as usize]
        .iter()
        .map(|(_, g)| *g)
        .collect();
    out.sort_unstable();
    out
}

#[must_use]
pub fn exact_drop_set_hash(ids: &[u16]) -> u64 {
    let mut b: Vec<u8> = b"FS1_DROPSET_v1".to_vec();
    for &g in ids {
        b.extend_from_slice(&g.to_le_bytes());
    }
    fnv1a(&b)
}
