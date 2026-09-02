//! PARAM-hash binding (§8 MADDE-8) — `param_hash = BLAKE2s-256( DS ‖ fields )`.
//!
//! Üretim-backend: RustCrypto `blake2::Blake2s256` (ADR-03 AMENDMENT-2 / DR-S5-5; pki-§13 primitif).
//! Canonical-byte-dizisi schema-otoritesinden (`ParamFields::canonical_hash_input`, DS="KAMA-PARAM-V1");
//! bu-modül YALNIZ hash'ler — schema kripto-free KALIR, transport crypto-agnostik KALIR (INV-ENGINE-SOC).
//!
//! **Golden-drift-gate:** çıktı `kama-transport-schema/tests/golden_v0.rs::golden_param_blake2s_hash`
//! vektörüyle BİT-ÖZDEŞ olmalı (aynı-backend, aynı-canonical-input).

use crate::error::WireError;
use blake2::{Blake2s256, Digest};
use kama_transport_schema::param::ParamFields;

/// `param_hash = BLAKE2s-256( DS ‖ canonical-fields )` (§8). B4-ihlalinde fail-closed.
///
/// # Errors
/// [`WireError::ParamInvalid`] — canonical_hash_input B4-invariant'ı reddederse (1≤K_shape≤255, E≥11).
pub fn bind_transport_parameters(fields: &ParamFields) -> Result<[u8; 32], WireError> {
    let input = fields
        .canonical_hash_input()
        .map_err(|_| WireError::ParamInvalid("canonical_hash_input B4-violation"))?;
    let mut h = Blake2s256::new();
    h.update(&input);
    Ok(h.finalize().into())
}

/// Gelen PARAM_SET.param_hash'i yerel-alanlardan yeniden-hesapla + karşılaştır (§8 fail-closed).
/// Sabit-zaman-karşılaştırma gerekmez: param_hash gizli-DEĞİL (public transport-parametre bağı);
/// mismatch = yanlış-profil/bozuk-PARAM → admission-reddi.
///
/// # Errors
/// [`WireError::ParamInvalid`] B4-ihlali · [`WireError::ParamHashMismatch`] hash uyuşmazlığı.
pub fn verify_param_hash(fields: &ParamFields, received: &[u8; 32]) -> Result<(), WireError> {
    let computed = bind_transport_parameters(fields)?;
    if &computed == received {
        Ok(())
    } else {
        Err(WireError::ParamHashMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> ParamFields {
        ParamFields {
            protocol_version: 1,
            fec_version: 1,
            code_suite_id: 1,
            profile_id: 2,
            k_shape: 8,
            r_code_max: 4,
            e: 64,
            partial_policy: 0,
            packet_scheduling_policy: 0,
        }
    }

    #[test]
    fn param_hash_deterministic_32b() {
        let f = fields();
        let a = bind_transport_parameters(&f).unwrap();
        let b = bind_transport_parameters(&f).unwrap();
        assert_eq!(a, b, "deterministik (aynı-input → aynı-32B)");
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn verify_roundtrip_and_mismatch() {
        let f = fields();
        let h = bind_transport_parameters(&f).unwrap();
        // Doğru-hash → Ok.
        verify_param_hash(&f, &h).unwrap();
        // Bozuk-hash → fail-closed ParamHashMismatch.
        let mut bad = h;
        bad[0] ^= 0xFF;
        assert_eq!(
            verify_param_hash(&f, &bad),
            Err(WireError::ParamHashMismatch)
        );
        // Farklı-alan (profile_id) → farklı-hash → gelen-eski-hash reddedilir.
        let mut f2 = f;
        f2.profile_id = 4;
        assert_eq!(
            verify_param_hash(&f2, &h),
            Err(WireError::ParamHashMismatch)
        );
    }

    #[test]
    fn b4_violation_fail_closed() {
        let mut f = fields();
        f.k_shape = 0; // B4: 1 ≤ K_shape → ihlal
        assert_eq!(
            bind_transport_parameters(&f),
            Err(WireError::ParamInvalid("canonical_hash_input B4-violation"))
        );
    }

    // Golden-drift-gate: schema golden-testiyle AYNI byte-dizisi (aynı canonical-input + aynı-backend).
    #[test]
    fn matches_schema_golden_vector() {
        // golden_v0.rs::golden_param_blake2s_hash ile aynı fields → aynı 32B (backend-özdeşlik kanıtı).
        let f = fields();
        let input = f.canonical_hash_input().unwrap();
        let mut h = Blake2s256::new();
        h.update(&input);
        let direct: [u8; 32] = h.finalize().into();
        assert_eq!(
            bind_transport_parameters(&f).unwrap(),
            direct,
            "bind == doğrudan-Blake2s256(canonical) (golden-backend özdeş)"
        );
    }
}
