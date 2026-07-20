//! Typed noise-wire hataları (reachable todo!() YASAK). Reject-reason'lar içerik-SIZDIRMAZ
//! (AEAD-fail ile parse-fail dışarıya aynı-sınıf; iç-teşhis için variant ayrımı korunur).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    /// Datagram EXTENDED-minimum'dan kısa (26B outer) veya ct-boyu tutarsız.
    #[error("datagram too short: {got} < {min}")]
    DatagramTooShort { got: usize, min: usize },
    /// wire_format != EXTENDED_V1 (§2.5: bilinmeyen-format fail-closed).
    #[error("unknown wire_format {0:#04x}")]
    UnknownWireFormat(u8),
    /// wire_flags bits1..7 nonzero (§2.5: reserved → REJECT).
    #[error("reserved wire_flags bits set: {0:#04x}")]
    ReservedWireFlags(u8),
    /// PN-precheck reddi (§4: too-old / duplicate / future-gap). Window İLERLETİLMEMİŞTİR.
    #[error("pn rejected: {0}")]
    PnReject(#[from] PnReject),
    /// dh_epoch kabul-penceresi dışında (§4-adım-1: current veya grace'teki previous değil).
    #[error("dh_epoch {0} not accepted (not current, not in grace)")]
    EpochNotAccepted(u8),
    /// AEAD authentication başarısız. Window İLERLETİLMEMİŞTİR (§4: unauthenticated-paket iz-bırakmaz).
    #[error("aead authentication failed")]
    AeadFail,
    /// Decrypt-sonrası inner-parse hatası (§2.56 unknown-inner_kind / bounds).
    #[error("inner parse: {0}")]
    InnerParse(&'static str),
    /// Seal tarafında payload/parametre sınır ihlali.
    #[error("seal input: {0}")]
    SealInput(&'static str),
    /// Epoch-1 anahtarı henüz kurulmadan epoch-1 seal/open istendi.
    #[error("dh_epoch 1 keys not installed")]
    EpochKeysMissing,
    /// Full-PN cap'e ulaştı — SESSION_RENEW_REQUIRED tetiklenmeli (§4 WRAP; gönderim-reddi).
    #[error("pn cap reached; session renewal required")]
    PnCapReached,
}

/// §4 precheck reddi — hangi-kural düştüğü (window-ilerletmeden).
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum PnReject {
    #[error("too-old (candidate + WINDOW < expected)")]
    TooOld,
    #[error("duplicate (window bit set)")]
    Duplicate,
    #[error("future-gap (candidate > expected + FUTURE_GAP_MAX)")]
    FutureGap,
}
