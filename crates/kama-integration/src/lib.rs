//! kama-integration — cross-crate integration + ÜRÜN-ALICI COMPOSITION-ROOT (DR-CS-1b/A2).
//!
//! Sprint-5a rolü (KORUNUR): fec-codec + schema + noise-wire'ı engine-gibi birleştiren test-harness
//! (tests/; mock-key). SoC (dependency-graph: fec-core(bağımsız) ← transport-core ← engine → noise-wire).
//!
//! **A2-üretimleştirme (Abkarada D-1/K-9; feature-gated `experimental-cold-start`, DEFAULT-OFF):**
//! [`receiver_host`] — ürün-alıcı composition-root. K-9: process-wide/global cold-resource
//! sahipliği BURADA (engine yalnız saf-tip sağlar). Flag-OFF'ta bu modül DERLENMEZ; crate eski
//! harness-rolünde byte-özdeş kalır (INV-CS-5).

#![forbid(unsafe_code)]

/// DR-CS-3 — DEFAULT-BUILD authenticated control component (library, not a product runtime).
/// Ungated: pulls in none of the `experimental-cold-start` cold-RX subsystem.
pub mod receiver_control_core;

/// DR-CS-1b/A2 — ürün-alıcı composition-root (K-8/K-9 sahiplik-sınırlarıyla).
#[cfg(feature = "experimental-cold-start")]
pub mod receiver_host;

pub use receiver_control_core::{
    BootstrapEchoResponder, CoreError, CoreIngest, ReceiverControlCore,
};
#[cfg(feature = "experimental-cold-start")]
pub use receiver_host::{CachedCloseAck, HostError, IngestOutcome, ReceiverHost, SlotLifecycle};
