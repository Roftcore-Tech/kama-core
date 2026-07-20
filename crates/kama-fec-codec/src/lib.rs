//! kama-fec-codec — reed-solomon-simd adapter + enumerated profiles + encode/decode (fec-core §2-6).
//! Kripto-AGNOSTİK (K-FEC-SOC-1/2): dependency-graph'ta Noise/AEAD/key/identity YOK; opaque-symbol.
//!
//! **BİLİR:** profile/block/ESI/K_actual/R_code_max/symbol-bytes. **BİLMEZ:** NoiseState/AEAD/nonce/
//! key-phase/tag/crypto-epoch (§1). symbol-byte plaintext/ciphertext FARK-ETMEZ.

#![forbid(unsafe_code)]

pub mod codec;
pub mod error;
pub mod profile;

pub use codec::{
    FecCodec, ReceivedSymbol, RecoveredSource, RepairSymbol, RsSimdCodec, SourceSymbol,
};
pub use error::FecError;
pub use profile::{by_id, FecProfile, CODE_SUITE_RS_SIMD_V1, PROFILES};
