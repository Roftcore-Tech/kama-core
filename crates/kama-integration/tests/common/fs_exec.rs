//! DR-FEC-FS-1 OPT-R1 — RESULT-BEARING EXECUTOR + PERSISTENCE CONTRACT (§1/§2/§3). HARNESS-ONLY.
//! Pure-Rust SHA-256 (artifact-integrity checksum; experimental-fs-harness-only; product-crypto DEĞİL — harness fnv
//! ile aynı kategori) · full raw-record (RAW-SCHEMA §8) · ATOMIC append-only raw-writer (temp→fsync→rename; overwrite-YOK)
//! · append-only ledger (dup-start/dup-valid reddi; interrupted-residue-quarantine) · INVALID-only-retry (max primary+2).
//! run_spine İÇERMEZ (caller SpineResult-türevi alanları verir → testable, double-include-YOK).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

// ── PURE-RUST SHA-256 (FIPS 180-4; deterministic; sha256sum ile byte-özdeş) ────────────────────────
const SHA256_H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];
#[rustfmt::skip]
const SHA256_K: [u32; 64] = [
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
];

#[must_use]
pub fn sha256_hex(msg: &[u8]) -> String {
    let mut h = SHA256_H0;
    let bitlen = (msg.len() as u64).wrapping_mul(8);
    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = String::with_capacity(64);
    for word in h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}

// ── TERMINAL STATUS + ERRORS ───────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    Invalid,
    ValidComplete,
    ValidCensored,
}
impl Validity {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Validity::Invalid => "INVALID",
            Validity::ValidComplete => "VALID_COMPLETE",
            Validity::ValidCensored => "VALID_CENSORED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    OutputExists(String),
    DuplicateStart(String),
    DuplicateValidOutcome(String),
    RetryExceeded(String),
    ChecksumMismatch(String),
    ProvenanceMismatch(String),
    WrongOrder(String),
    NotInManifest(String),
    InterruptedResidue(String),
}

// ── RAW RECORD (RAW-SCHEMA §8 subset; deterministic canonical JSON) ─────────────────────────────────
pub struct RawRecord {
    // identity
    pub run_id: String,
    pub attempt_index: u8,
    pub pairing_group_id: String,
    pub arm: String,
    pub base_cell_id: String,
    pub repetition_index: u8,
    // freeze provenance
    pub freeze_lock_sha256: String,
    pub final_manifest_sha256: String,
    pub execution_binary_sha256: String,
    pub execution_source_commit: String,
    // configuration
    pub base_cell_semantics_hash: String,
    pub run_execution_semantics_hash: String,
    pub resolved_behavior_semantics_hash: String,
    // outcome
    pub validity_class: Validity,
    pub deadline_hit: bool,
    pub delivered_fragments: u16,
    pub delivered_bytes: u32,
    pub reassembly_equal: bool,
    pub completion_ns: Option<u64>,
    pub censor_time_ns: Option<u64>,
    // cost
    pub base_source_bytes: u64,
    pub intervention_bytes: u64,
    pub arq_retransmit_bytes: u64,
    pub total_outcome_bytes: u64,
    pub retransmits: u64,
    pub fec_decoded_sources: usize,
}

fn opt_ns(v: Option<u64>) -> String {
    v.map_or("null".into(), |x| x.to_string())
}

#[must_use]
pub fn serialize_raw_record(r: &RawRecord) -> String {
    format!(
        concat!(
            "{{\"run_id\":\"{rid}\",\"attempt_index\":{ai},\"pairing_group_id\":\"{pg}\",\"arm\":\"{arm}\",",
            "\"base_cell_id\":\"{bc}\",\"repetition_index\":{rep},",
            "\"freeze_lock_sha256\":\"{fl}\",\"final_manifest_sha256\":\"{fm}\",",
            "\"execution_binary_sha256\":\"{eb}\",\"execution_source_commit\":\"{esc}\",",
            "\"base_cell_semantics_hash\":\"{bch}\",\"run_execution_semantics_hash\":\"{reh}\",",
            "\"resolved_behavior_semantics_hash\":\"{rbh}\",",
            "\"validity_class\":\"{vc}\",\"deadline_hit\":{dh},\"delivered_fragments\":{df},",
            "\"delivered_bytes\":{db},\"reassembly_equal\":{re},\"completion_ns\":{cn},\"censor_time_ns\":{ct},",
            "\"base_source_bytes\":{bsb},\"intervention_bytes\":{ib},\"arq_retransmit_bytes\":{arq},",
            "\"total_outcome_bytes\":{tob},\"retransmits\":{rtx},\"fec_decoded_sources\":{fds},",
            "\"result_bearing\":true}}"
        ),
        rid = r.run_id, ai = r.attempt_index, pg = r.pairing_group_id, arm = r.arm,
        bc = r.base_cell_id, rep = r.repetition_index,
        fl = r.freeze_lock_sha256, fm = r.final_manifest_sha256, eb = r.execution_binary_sha256,
        esc = r.execution_source_commit, bch = r.base_cell_semantics_hash,
        reh = r.run_execution_semantics_hash, rbh = r.resolved_behavior_semantics_hash,
        vc = r.validity_class.label(), dh = r.deadline_hit, df = r.delivered_fragments,
        db = r.delivered_bytes, re = r.reassembly_equal, cn = opt_ns(r.completion_ns),
        ct = opt_ns(r.censor_time_ns), bsb = r.base_source_bytes, ib = r.intervention_bytes,
        arq = r.arq_retransmit_bytes, tob = r.total_outcome_bytes, rtx = r.retransmits,
        fds = r.fec_decoded_sources,
    )
}

// ── ATOMIC APPEND-ONLY RAW WRITER (temp→fsync→rename; overwrite-YOK; sidecar checksum) ─────────────
pub struct RawWriter {
    pub namespace: PathBuf,
}
impl RawWriter {
    #[must_use]
    pub fn new(namespace: impl Into<PathBuf>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
    fn final_path(&self, run_id: &str, attempt: u8) -> PathBuf {
        self.namespace
            .join(format!("{run_id}.attempt{attempt}.json"))
    }
    /// Atomic write; returns raw-record SHA-256. Overwrite → ExecError::OutputExists. .sha256 sidecar yazılır.
    pub fn write(&self, r: &RawRecord) -> Result<String, ExecError> {
        std::fs::create_dir_all(&self.namespace).ok();
        let final_p = self.final_path(&r.run_id, r.attempt_index);
        if final_p.exists() {
            return Err(ExecError::OutputExists(final_p.display().to_string()));
        }
        let body = serialize_raw_record(r);
        let sha = sha256_hex(body.as_bytes());
        let tmp = self
            .namespace
            .join(format!("{}.attempt{}.json.tmp", r.run_id, r.attempt_index));
        {
            let mut f =
                std::fs::File::create(&tmp).map_err(|e| ExecError::OutputExists(e.to_string()))?;
            f.write_all(body.as_bytes()).ok();
            f.sync_all().ok(); // fsync
        }
        // atomic finalize
        std::fs::rename(&tmp, &final_p).map_err(|e| ExecError::OutputExists(e.to_string()))?;
        std::fs::write(final_p.with_extension("json.sha256"), format!("{sha}\n")).ok();
        Ok(sha)
    }
    /// OPT-X2 v2 path: atomic write of a PRE-SERIALIZED body (schema-v2 record already validated by caller).
    /// Same atomic temp→fsync→rename + .sha256 sidecar + no-overwrite semantics as `write`.
    pub fn write_serialized(
        &self,
        run_id: &str,
        attempt: u8,
        body: &str,
    ) -> Result<String, ExecError> {
        std::fs::create_dir_all(&self.namespace).ok();
        let final_p = self.final_path(run_id, attempt);
        if final_p.exists() {
            return Err(ExecError::OutputExists(final_p.display().to_string()));
        }
        let sha = sha256_hex(body.as_bytes());
        let tmp = self
            .namespace
            .join(format!("{run_id}.attempt{attempt}.json.tmp"));
        {
            let mut f =
                std::fs::File::create(&tmp).map_err(|e| ExecError::OutputExists(e.to_string()))?;
            f.write_all(body.as_bytes()).ok();
            f.sync_all().ok();
        }
        std::fs::rename(&tmp, &final_p).map_err(|e| ExecError::OutputExists(e.to_string()))?;
        std::fs::write(final_p.with_extension("json.sha256"), format!("{sha}\n")).ok();
        Ok(sha)
    }

    /// Interrupted-residue: yarım .tmp dosyası (finalize-edilmemiş). restart quarantine-etmeli, overwrite-DEĞİL.
    #[must_use]
    pub fn residue_tmps(&self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.namespace) {
            for e in rd.flatten() {
                if e.path().extension().is_some_and(|x| x == "tmp") {
                    v.push(e.path());
                }
            }
        }
        v.sort();
        v
    }
}

// ── APPEND-ONLY LEDGER (dup-start/dup-valid reddi; retry-linkage; running-checksum) ────────────────
pub struct Ledger {
    pub path: PathBuf,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub run_id: String,
    pub attempt_index: u8,
    pub kind: String, // "STARTED" | "COMPLETE"
    pub validity: Option<Validity>,
    pub raw_sha256: Option<String>,
}
impl Ledger {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    #[must_use]
    pub fn entries(&self) -> Vec<LedgerEntry> {
        let mut out = Vec::new();
        if let Ok(s) = std::fs::read_to_string(&self.path) {
            for line in s.lines() {
                // canonical: run_id|attempt|kind|validity|raw_sha|entry_sha
                let f: Vec<&str> = line.split('|').collect();
                if f.len() >= 5 {
                    out.push(LedgerEntry {
                        run_id: f[0].to_string(),
                        attempt_index: f[1].parse().unwrap_or(0),
                        kind: f[2].to_string(),
                        validity: match f[3] {
                            "VALID_COMPLETE" => Some(Validity::ValidComplete),
                            "VALID_CENSORED" => Some(Validity::ValidCensored),
                            "INVALID" => Some(Validity::Invalid),
                            _ => None,
                        },
                        raw_sha256: if f[4] == "-" {
                            None
                        } else {
                            Some(f[4].to_string())
                        },
                    });
                }
            }
        }
        out
    }
    fn append_line(&self, run_id: &str, attempt: u8, kind: &str, validity: &str, raw_sha: &str) {
        let core = format!("{run_id}|{attempt}|{kind}|{validity}|{raw_sha}");
        let entry_sha = sha256_hex(core.as_bytes());
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .expect("ledger open");
        writeln!(f, "{core}|{entry_sha}").ok();
        f.sync_all().ok();
    }
    /// §11: primary-start reserve (atomic). dup-start (INVALID-öncesi-olmadan) → reddet.
    pub fn reserve_start(&self, run_id: &str, attempt: u8) -> Result<(), ExecError> {
        let es = self.entries();
        // aynı-run zaten VALID-tamamlandıysa → dup-start reddi.
        if es.iter().any(|e| {
            e.run_id == run_id
                && e.kind == "COMPLETE"
                && matches!(
                    e.validity,
                    Some(Validity::ValidComplete | Validity::ValidCensored)
                )
        }) {
            return Err(ExecError::DuplicateStart(format!(
                "{run_id} zaten VALID-tamam"
            )));
        }
        // aynı (run,attempt) STARTED zaten varsa → dup-start reddi (crash-residue quarantine ayrı).
        if es
            .iter()
            .any(|e| e.run_id == run_id && e.attempt_index == attempt && e.kind == "STARTED")
        {
            return Err(ExecError::DuplicateStart(format!(
                "{run_id} attempt{attempt} zaten STARTED"
            )));
        }
        self.append_line(run_id, attempt, "STARTED", "-", "-");
        Ok(())
    }
    /// commit terminal outcome. VALID-dup → reddet. attempt policy caller'da.
    pub fn commit(
        &self,
        run_id: &str,
        attempt: u8,
        v: Validity,
        raw_sha: &str,
    ) -> Result<(), ExecError> {
        let es = self.entries();
        if es.iter().any(|e| {
            e.run_id == run_id
                && e.kind == "COMPLETE"
                && matches!(
                    e.validity,
                    Some(Validity::ValidComplete | Validity::ValidCensored)
                )
        }) {
            return Err(ExecError::DuplicateValidOutcome(run_id.into()));
        }
        self.append_line(run_id, attempt, "COMPLETE", v.label(), raw_sha);
        Ok(())
    }
    /// §3 retry: INVALID-only; max primary(0)+2 → attempt ∈ {0,1,2}. next attempt eligible?
    #[must_use]
    pub fn next_attempt(&self, run_id: &str) -> Option<u8> {
        let es = self.entries();
        if es.iter().any(|e| {
            e.run_id == run_id
                && e.kind == "COMPLETE"
                && matches!(
                    e.validity,
                    Some(Validity::ValidComplete | Validity::ValidCensored)
                )
        }) {
            return None; // final; retry-YOK
        }
        let invalids = es
            .iter()
            .filter(|e| {
                e.run_id == run_id && e.kind == "COMPLETE" && e.validity == Some(Validity::Invalid)
            })
            .count();
        if invalids >= 3 {
            None // max primary+2
        } else {
            Some(invalids as u8)
        }
    }
    #[must_use]
    pub fn valid_complete_count(&self) -> usize {
        self.entries()
            .iter()
            .filter(|e| {
                e.kind == "COMPLETE"
                    && matches!(
                        e.validity,
                        Some(Validity::ValidComplete | Validity::ValidCensored)
                    )
            })
            .count()
    }
}

// ── VERIFY HELPERS (§1 chain) ──────────────────────────────────────────────────────────────────────
#[must_use]
pub fn verify_hash(expected: &str, actual: &str) -> bool {
    expected == actual && expected.len() == 64
}

/// §4 provenance-verify öncesi run_id manifest-order'da mı + doğru-sıra-pozisyonu mu.
pub fn verify_order_position(
    order: &[String],
    next_expected: &str,
    run_id: &str,
) -> Result<(), ExecError> {
    if !order.iter().any(|r| r == run_id) {
        return Err(ExecError::NotInManifest(run_id.into()));
    }
    if next_expected != run_id {
        return Err(ExecError::WrongOrder(format!(
            "beklenen {next_expected}, gelen {run_id}"
        )));
    }
    Ok(())
}

/// namespace'te finalize-edilmemiş .tmp varsa restart quarantine (overwrite-DEĞİL).
pub fn quarantine_residue(writer: &RawWriter) -> Result<(), ExecError> {
    let r = writer.residue_tmps();
    if r.is_empty() {
        Ok(())
    } else {
        // quarantine: .tmp'leri .quarantine'a taşı (silme-YOK; overwrite-YOK).
        for p in &r {
            let q = p.with_extension("tmp.quarantine");
            std::fs::rename(p, &q).ok();
        }
        Err(ExecError::InterruptedResidue(format!(
            "{} residue quarantined",
            r.len()
        )))
    }
}

/// serial next-eligible: order'da ilk VALID-tamamlanmamış run_id.
#[must_use]
pub fn next_eligible<'a>(order: &'a [String], ledger: &Ledger) -> Option<&'a str> {
    let es = ledger.entries();
    let done: BTreeMap<String, ()> = es
        .iter()
        .filter(|e| {
            e.kind == "COMPLETE"
                && matches!(
                    e.validity,
                    Some(Validity::ValidComplete | Validity::ValidCensored)
                )
        })
        .map(|e| (e.run_id.clone(), ()))
        .collect();
    order
        .iter()
        .find(|r| !done.contains_key(*r))
        .map(String::as_str)
}
