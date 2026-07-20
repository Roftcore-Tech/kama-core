//! PARAM-SET canonical field-serializer (kama-noise-wire-tdd §8, MADDE-8).
//!
//! `param_hash = BLAKE2s-256( DS || fields )` — DS = ASCII("KAMA-PARAM-V1"); fields canonical,
//! big-endian, sıra-SABİT. Bu modül canonical **byte-dizisini** üretir (DS || fields); asıl
//! BLAKE2s-256 hash'i binding-katmanı (engine, §8 bind_transport_parameters) uygular — schema
//! kripto/hash-primitive İÇERMEZ. Golden-vector bu byte-dizisini ayrıca assert eder.

use crate::{CodeSuiteId, ProfileId, SchemaError};

/// Domain-separator (§8): ASCII("KAMA-PARAM-V1").
pub const PARAM_DS: &[u8] = b"KAMA-PARAM-V1";

/// B4-invariant üst-sınırı (§8): K_shape ≤ 255 (shard k_actual-u8 uyumu; K_shape-u16-format korunur).
pub const K_SHAPE_MAX: u16 = 255;
/// B4-invariant alt-sınırı (§8): E ≥ 11 (prefix sığar).
pub const E_MIN: u16 = 11;

/// PARAM_SET canonical alanları (§8, sıra-sabit big-endian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamFields {
    pub protocol_version: u8,
    pub fec_version: u8,
    pub code_suite_id: CodeSuiteId,
    pub profile_id: ProfileId,
    pub k_shape: u16,
    pub r_code_max: u16,
    pub e: u16,
    pub partial_policy: u8,
    pub packet_scheduling_policy: u8,
}

impl ParamFields {
    /// B4-invariant kontrolü (§8): 1 ≤ K_shape ≤ 255, E ≥ 11.
    ///
    /// # Errors
    /// İhlalde [`SchemaError::ProfileBound`] (fail-closed).
    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.k_shape < 1 || self.k_shape > K_SHAPE_MAX {
            return Err(SchemaError::ProfileBound("1 <= K_shape <= 255"));
        }
        if self.e < E_MIN {
            return Err(SchemaError::ProfileBound("E >= 11"));
        }
        Ok(())
    }

    /// Canonical hash-girdisi: `DS || fields` (BLAKE2s-256'ya beslenecek byte-dizisi, §8). Native-
    /// struct/serde/padding KULLANILMAZ; her integer big-endian, sıra-sabit.
    ///
    /// # Errors
    /// B4-invariant ihlalinde [`SchemaError::ProfileBound`].
    pub fn canonical_hash_input(&self) -> Result<Vec<u8>, SchemaError> {
        self.validate()?;
        let mut v = Vec::with_capacity(PARAM_DS.len() + 12);
        v.extend_from_slice(PARAM_DS);
        v.push(self.protocol_version);
        v.push(self.fec_version);
        v.push(self.code_suite_id);
        v.push(self.profile_id);
        v.extend_from_slice(&self.k_shape.to_be_bytes());
        v.extend_from_slice(&self.r_code_max.to_be_bytes());
        v.extend_from_slice(&self.e.to_be_bytes());
        v.push(self.partial_policy);
        v.push(self.packet_scheduling_policy);
        Ok(v)
    }
}
