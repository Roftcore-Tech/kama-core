//! kama-noise-wire — Kama-Secure-Wire-Profile EXTENDED katmanı (kama-noise-wire-tdd-v2.0).
//!
//! **SORUMLULUK (§0):** EXTENDED seal/open · canonical-AAD (§2.8) · epoch-aware truncated-PN
//! reconstruction + anti-replay (§4, DR-3 per-width sabitler) · sealed-OPENED-tip (§6,
//! K-WIRE-SEALED-TYPE-1) · sealed_len-sözleşmesi + SealedPacketMeta (§0.5).
//!
//! **MÜHÜRLÜ-SINIR (§0.9):** legacy-SESSION_DATA-zarfı (22B) ccr-§35-otoritesinde — bu-crate-onu
//! OKUR (PN-sabitleri DR-3-legacy-seti), YENİDEN-TANIMLAMAZ. Handshake-pattern (Noise-N-türevi
//! one-way zero-RTT) mühürlü; session/handshake-katmanı Sprint-4.
//!
//! AEAD: ChaCha20-Poly1305 (RFC 8439; pki-§13-registry). Nonce-kurgusu DR-4 (protector-notu).
//! ADR-05: fixed-width big-endian; VARINT YASAK.

#![forbid(unsafe_code)]

pub mod aad;
pub mod control;
pub mod epoch_machine;
pub mod error;
pub mod handle_assign;
pub mod hub_sim;
pub mod inner;
pub mod opened;
pub mod outer;
pub mod pn;
pub mod protector;
pub mod retry;
pub mod session;

pub use control::{
    decode_control_body, encode_control_body, split_for_send, ACK_MAX_RANGES,
    RECOVERED_MAX_ESI_RANGES, SACK_MAX_GAP_BLOCKS,
};
pub use epoch_machine::{DhEpochMachine, EpochAction, RecvEpoch0, Role, GRACE_MIN_PACKETS};
pub use error::{PnReject, WireError};
pub use handle_assign::{HandleAssign, HandleAssignAck, HANDLE_ASSIGN_LEN};
pub use hub_sim::{HubSim, ResponderDelivery, HANDLE_ASSIGN_MAX_RETRIES};
pub use inner::InnerPlain;
pub use opened::{OpenedTransportShard, PrivateAdmissionProof};
pub use outer::ExtendedOuter;
pub use pn::{reconstruct, PnParams, ReplayWindow, EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};
pub use protector::{
    nonce96, KamaNoiseWireProtector, KamaNoiseWireSizer, LogicalPacketRef, PacketClass, SealArgs,
    SealedPacket, SealedPacketMeta, WireSizeError, KEY_LEN, SEND_PN_CAP,
};
pub use retry::{InboundControlDedup, InboundOutcome, RetryClass};
pub use session::{Direction, Session, SessionId, SessionLifecycle, SessionManager};
