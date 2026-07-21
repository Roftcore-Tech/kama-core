//! 5d-1 receiver-tarafı bound-param durumu + admission-gate (§8 fail-closed).
//!
//! Authenticated-PARAM_SET alınınca yerel-beklenen `ParamFields`'la param_hash-verify → bind.
//! Mismatch → fail-closed (bound olmaz; admission kapalı kalır). PARAM_SET-öncesi shard'lar
//! [`crate::preparam_hold`] ile bounded-tutulur; bind sonrası release edilir.

use crate::error::EngineError;
use kama_noise_wire::param_bind::verify_param_hash;
use kama_transport_schema::control::Control;
use kama_transport_schema::param::ParamFields;

/// Doğrulanmış-bağlı transport-parametreleri (admission-gate açık).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundParam {
    pub profile_id: u8,
    pub code_suite_id: u8,
    pub param_hash: [u8; 32],
}

/// Receiver bound-param durumu. `bound=None` → admission KAPALI (shard'lar hold'a).
#[derive(Debug, Default)]
pub struct ParamState {
    bound: Option<BoundParam>,
}

impl ParamState {
    #[must_use]
    pub fn new() -> Self {
        Self { bound: None }
    }

    /// Authenticated-PARAM_SET işle: yerel-beklenen `expected` alanlarla param_hash-verify (§8) +
    /// profile_id-eşleşmesi → bind. **Fail-closed:** hash-mismatch veya profile-mismatch → bound OLMAZ.
    ///
    /// # Errors
    /// [`EngineError::UnexpectedControl`] PARAM_SET değilse · [`EngineError::ProfileMismatch`] profile_id≠expected ·
    /// [`EngineError::Param`] (`ParamHashMismatch`/`ParamInvalid`) hash-verify fail-closed.
    pub fn on_param_set(
        &mut self,
        param_set: &Control,
        expected: &ParamFields,
    ) -> Result<(), EngineError> {
        let Control::ParamSet {
            param_hash,
            profile_id,
            code_suite_id,
            ..
        } = param_set
        else {
            return Err(EngineError::UnexpectedControl);
        };
        // profile_id gelen-PARAM ile yerel-beklenen eşleşmeli (yanlış-profil admission-almaz).
        if *profile_id != expected.profile_id {
            return Err(EngineError::ProfileMismatch {
                shard: *profile_id,
                bound: expected.profile_id,
            });
        }
        // param_hash = BLAKE2s-256(yerel-beklenen-alanlar) ile bit-özdeş mi? (§8 fail-closed)
        verify_param_hash(expected, param_hash)?;
        self.bound = Some(BoundParam {
            profile_id: *profile_id,
            code_suite_id: *code_suite_id,
            param_hash: *param_hash,
        });
        Ok(())
    }

    /// Admission-gate açık mı (bound-param var mı)?
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        self.bound.is_some()
    }

    /// Bağlı profile_id (decode-profili çözümü için; `None` → henüz bound-değil).
    #[must_use]
    pub fn bound_profile_id(&self) -> Option<u8> {
        self.bound.map(|b| b.profile_id)
    }

    /// Bağlı parametreler (teşhis/test).
    #[must_use]
    pub fn bound(&self) -> Option<BoundParam> {
        self.bound
    }
}
