//! Engine typed-hataları (reachable todo!() YASAK).

use kama_fec_codec::FecError;
use kama_noise_wire::WireError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    /// PARAM-bind/verify (wire-katmanı; §8 fail-closed).
    #[error("param: {0}")]
    Param(#[from] WireError),
    /// PARAM_SET.profile_id ↔ bound-profil uyuşmazlığı (admission fail-closed).
    #[error("profile mismatch: shard {shard} != bound {bound}")]
    ProfileMismatch { shard: u8, bound: u8 },
    /// profile_id registry-allowlist dışında (by_id → None; fail-closed).
    #[error("unknown profile_id {0}")]
    UnknownProfile(u8),
    /// PARAM confirmed-değilken decode-yolu çağrıldı (admission-gate kapalı).
    #[error("param not confirmed (admission gate closed)")]
    ParamNotConfirmed,
    /// FEC-decode başarısızlığı (dup-esi/yetersiz/sınır; fail-closed).
    #[error("fec decode: {0}")]
    FecDecode(#[from] FecError),
    /// Reassembly (source-prefix parse / eksik-source).
    #[error("reassembly: {0}")]
    Reassembly(&'static str),
    /// PARAM_ACK hash-echo yerel-hash'le eşleşmiyor (confirmation reddi).
    #[error("param_ack hash mismatch")]
    ParamAckMismatch,
    /// Beklenen control-tipi değil (PARAM_SET/PARAM_ACK bekleniyordu).
    #[error("unexpected control kind")]
    UnexpectedControl,
    /// A2-fazı henüz uygulanmadı (typed-stub; reachable todo!() YASAK — CLAUDE.md §6).
    #[cfg(feature = "experimental-cold-start")]
    #[error("feature not implemented: {0}")]
    FeatureNotImplemented(&'static str),
    /// Cold-cap aşımı: sessiz-eviction YOK, typed-reject (INV-CS-RX-RESOURCE; [CAL]-freeze-r2).
    #[cfg(feature = "experimental-cold-start")]
    #[error("cold cap exceeded: {scope:?}/{kind:?} cap={cap} requested={requested}")]
    ColdCapExceeded {
        scope: crate::cold_rx_state::CapScope,
        kind: crate::cold_rx_state::CapKind,
        cap: u64,
        requested: u64,
    },
    /// PARAM hiç gelmedi; pre-PARAM blok per-block-deadline'da evict edildi (P-6).
    #[cfg(feature = "experimental-cold-start")]
    #[error("param never arrived; block {block_id} deadline expired")]
    ParamNeverArrived { block_id: u32 },
    /// Cold-invariant ihlali (örn. K-2 aynı-epoch/farklı-blok) → DUR-B tetikleyicisi.
    #[cfg(feature = "experimental-cold-start")]
    #[error("cold invariant violation: {0}")]
    ColdInvariantViolation(&'static str),
}
