//! FecCodec trait + reed-solomon-simd adapter (fec-core §2). OPAQUE-symbol API — Noise/AEAD/nonce/
//! key TİPLERİ SIZMAZ (K-FEC-SOC-1/2). symbol-byte plaintext/ciphertext FARK-ETMEZ.

use crate::error::FecError;
use crate::profile::FecProfile;
use reed_solomon_simd::{ReedSolomonDecoder, ReedSolomonEncoder};

/// Kaynak-shard (encode girdisi). esi ∈ [0, K_shape). bytes.len() == E.
#[derive(Debug, Clone)]
pub struct SourceSymbol<'a> {
    pub esi: u16,
    pub bytes: &'a [u8],
}

/// Repair-shard (encode çıktısı). esi ∈ [0, R_code_max). bytes.len() == E.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairSymbol {
    pub esi: u16,
    pub bytes: Vec<u8>,
}

/// Alıcıya ulaşan authenticated-shard (decode girdisi). is_repair ile source/repair ayrımı.
#[derive(Debug, Clone)]
pub struct ReceivedSymbol<'a> {
    pub esi: u16,
    pub is_repair: bool,
    pub bytes: &'a [u8],
}

/// Reconstruct edilen kayıp-source. esi = source-index; bytes = E-byte canonical symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredSource {
    pub esi: u16,
    pub bytes: Vec<u8>,
}

/// Kripto-agnostik FEC codec sözleşmesi (fec-core §2).
pub trait FecCodec {
    /// K_shape source-symbol → R repair-symbol (R ≤ R_code_max). Model-A: known-zero-slot'lar da
    /// source-set'e girer (çağıran doldurur); bu API tam K_shape source bekler.
    ///
    /// # Errors
    /// Profile-bound / source-count / symbol-len / RS-engine.
    fn encode(
        &self,
        profile: &FecProfile,
        source: &[SourceSymbol<'_>],
    ) -> Result<Vec<RepairSymbol>, FecError>;

    /// Admitted-distinct source+repair → kayıp-source reconstruct. known_zero_count canonical-zero
    /// bilinen-source enjekte edilir (Model-A; §6). Decode-koşulu: admitted + known_zero ≥ K_shape.
    ///
    /// # Errors
    /// Profile-bound / dup-ESI / ESI-range / insufficient / RS-engine.
    fn reconstruct(
        &self,
        profile: &FecProfile,
        received: &[ReceivedSymbol<'_>],
        known_zero_count: usize,
    ) -> Result<Vec<RecoveredSource>, FecError>;
}

/// reed-solomon-simd tabanlı codec (Alpha CODE_SUITE_RS_SIMD_V1).
#[derive(Debug, Default, Clone, Copy)]
pub struct RsSimdCodec;

fn rs_err(e: impl core::fmt::Display) -> FecError {
    FecError::RsEngine(e.to_string())
}

impl FecCodec for RsSimdCodec {
    fn encode(
        &self,
        profile: &FecProfile,
        source: &[SourceSymbol<'_>],
    ) -> Result<Vec<RepairSymbol>, FecError> {
        profile.validate()?;
        let k = profile.k_shape_usize();
        let e = profile.e_usize();
        let r = profile.r_code_max as usize;
        if source.len() != k {
            return Err(FecError::SourceCount {
                got: source.len(),
                expected: k,
            });
        }
        // ESI 0..K sırasına diz (RS add_original_shard sırası = source-index).
        let mut ordered: Vec<Option<&[u8]>> = vec![None; k];
        for s in source {
            if s.bytes.len() != e {
                return Err(FecError::SymbolLen {
                    got: s.bytes.len(),
                    expected: e,
                });
            }
            let idx = usize::from(s.esi);
            if idx >= k {
                return Err(FecError::EsiOutOfRange {
                    esi: s.esi,
                    kind: "source",
                });
            }
            if ordered[idx].is_some() {
                return Err(FecError::DuplicateEsi(s.esi));
            }
            ordered[idx] = Some(s.bytes);
        }
        let mut enc = ReedSolomonEncoder::new(k, r, e).map_err(rs_err)?;
        for (idx, slot) in ordered.iter().enumerate() {
            let bytes = slot.ok_or(FecError::EsiOutOfRange {
                esi: idx as u16,
                kind: "source-gap",
            })?;
            enc.add_original_shard(bytes).map_err(rs_err)?;
        }
        let result = enc.encode().map_err(rs_err)?;
        Ok(result
            .recovery_iter()
            .enumerate()
            .map(|(i, rec)| RepairSymbol {
                esi: i as u16,
                bytes: rec.to_vec(),
            })
            .collect())
    }

    fn reconstruct(
        &self,
        profile: &FecProfile,
        received: &[ReceivedSymbol<'_>],
        known_zero_count: usize,
    ) -> Result<Vec<RecoveredSource>, FecError> {
        profile.validate()?;
        let k = profile.k_shape_usize();
        let e = profile.e_usize();
        let r = profile.r_code_max as usize;

        // Admitted-distinct ayrıştır + dedup + range-check.
        let mut src_seen = vec![false; k];
        let mut rep_seen = vec![false; r];
        let mut src: Vec<(usize, &[u8])> = Vec::new();
        let mut rep: Vec<(usize, &[u8])> = Vec::new();
        for s in received {
            if s.bytes.len() != e {
                return Err(FecError::SymbolLen {
                    got: s.bytes.len(),
                    expected: e,
                });
            }
            let idx = usize::from(s.esi);
            if s.is_repair {
                if idx >= r {
                    return Err(FecError::EsiOutOfRange {
                        esi: s.esi,
                        kind: "repair",
                    });
                }
                if rep_seen[idx] {
                    return Err(FecError::DuplicateEsi(s.esi));
                }
                rep_seen[idx] = true;
                rep.push((idx, s.bytes));
            } else {
                if idx >= k {
                    return Err(FecError::EsiOutOfRange {
                        esi: s.esi,
                        kind: "source",
                    });
                }
                if src_seen[idx] {
                    return Err(FecError::DuplicateEsi(s.esi));
                }
                src_seen[idx] = true;
                src.push((idx, s.bytes));
            }
        }
        // Decode-koşulu (§6): admitted_source + admitted_repair + known_zero ≥ K_shape.
        let admitted = src.len() + rep.len();
        if admitted + known_zero_count < k {
            return Err(FecError::Insufficient {
                admitted,
                known_zero: known_zero_count,
                k_shape: k,
            });
        }
        // Known-zero injection: son known_zero_count source-slot canonical-zero (§6 Model-A).
        // K_actual = K_shape − known_zero_count → indexler [K_actual, K_shape) zero-source.
        let k_actual = k.saturating_sub(known_zero_count);
        let zero_shard = vec![0u8; e];
        for (offset, seen) in src_seen[k_actual..k].iter_mut().enumerate() {
            if !*seen {
                *seen = true;
                src.push((k_actual + offset, &zero_shard));
            }
        }
        // Tüm-source zaten mevcutsa (kayıp yok) → reconstruct-gereksiz.
        let missing: Vec<usize> = (0..k).filter(|i| !src_seen[*i]).collect();
        if missing.is_empty() {
            return Ok(Vec::new());
        }
        let mut dec = ReedSolomonDecoder::new(k, r, e).map_err(rs_err)?;
        for (idx, bytes) in &src {
            dec.add_original_shard(*idx, bytes).map_err(rs_err)?;
        }
        for (idx, bytes) in &rep {
            dec.add_recovery_shard(*idx, bytes).map_err(rs_err)?;
        }
        let result = dec.decode().map_err(rs_err)?;
        let mut out: Vec<RecoveredSource> = result
            .restored_original_iter()
            .filter(|(idx, _)| *idx < k_actual) // zero-injected slot'ları app'e verme
            .map(|(idx, bytes)| RecoveredSource {
                esi: idx as u16,
                bytes: bytes.to_vec(),
            })
            .collect();
        out.sort_by_key(|r| r.esi);
        Ok(out)
    }
}
