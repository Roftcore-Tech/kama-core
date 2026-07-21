//! 5d-1 sender-tarafı PARAM-bootstrap (B2 akışı, §8).
//!
//! **B2-KRİTİK:** PARAM_SET-gönderimi + FEC-emisyon ATOMİK-PROAKTİF — sender PARAM_SET'i yollar ve
//! HEMEN FEC-emisyonu başlatır, PARAM_ACK-BEKLEMEDEN (Bob-offline'ken bile FEC akar). PARAM_ACK =
//! confirmation-only (retry-durdurur; emisyonu DEĞİŞTİRMEZ, admission-kapısı DEĞİL).

use crate::error::EngineError;
use kama_noise_wire::param_bind::bind_transport_parameters;
use kama_transport_schema::control::Control;
use kama_transport_schema::control::ControlId;
use kama_transport_schema::param::ParamFields;

/// PARAM-bootstrap planı: gönderilecek PARAM_SET + proaktif-emit direktifi + yerel-hash (ACK-doğrulaması için).
#[derive(Debug, Clone)]
pub struct ParamSetPlan {
    /// Seal+emit edilecek PARAM_SET control-frame'i (param_hash bind-edilmiş).
    pub control: Control,
    /// B2: `true` → çağıran HEMEN `sender.set_fec_enabled(true)` yapar (PARAM_ACK-beklemeden).
    pub start_fec_immediately: bool,
    /// Yerel-hesaplanan param_hash (PARAM_ACK-echo doğrulaması için saklanır).
    pub param_hash: [u8; 32],
}

/// PARAM_SET-planla: param_hash=BLAKE2s-256(§8) bind + proaktif-emit direktifi.
///
/// # Errors
/// [`EngineError::Param`] — B4-invariant ihlali (canonical_hash_input fail-closed).
pub fn plan_param_bootstrap(
    id: ControlId,
    fields: &ParamFields,
) -> Result<ParamSetPlan, EngineError> {
    let param_hash = bind_transport_parameters(fields)?;
    Ok(ParamSetPlan {
        control: Control::ParamSet {
            id,
            param_hash,
            profile_id: fields.profile_id,
            code_suite_id: fields.code_suite_id,
        },
        start_fec_immediately: true, // B2: proaktif (PARAM_ACK-beklemeden)
        param_hash,
    })
}

/// PARAM_ACK confirmation izleyicisi (emisyonu ETKİLEMEZ; yalnız retry-durdurma/confirmed-işaret).
#[derive(Debug, Default)]
pub struct ParamAckTracker {
    confirmed: bool,
}

impl ParamAckTracker {
    #[must_use]
    pub fn new() -> Self {
        Self { confirmed: false }
    }

    /// PARAM_ACK geldi → hash-echo'yu yerel-hash'le doğrula → confirmed-işaretle. **Emisyonu DEĞİŞTİRMEZ**
    /// (B2: FEC zaten akıyor; ACK yalnız confirmation). Yanlış-echo → confirmation reddi (emit sürer).
    ///
    /// # Errors
    /// [`EngineError::UnexpectedControl`] PARAM_ACK değilse · [`EngineError::ParamAckMismatch`] echo≠yerel-hash.
    pub fn on_param_ack(
        &mut self,
        ack: &Control,
        expected_hash: &[u8; 32],
    ) -> Result<(), EngineError> {
        let Control::ParamAck { param_hash, .. } = ack else {
            return Err(EngineError::UnexpectedControl);
        };
        if param_hash == expected_hash {
            self.confirmed = true;
            Ok(())
        } else {
            Err(EngineError::ParamAckMismatch)
        }
    }

    /// PARAM_ACK ile confirmation geldi mi? (emisyon bundan BAĞIMSIZ akar — B2).
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}
