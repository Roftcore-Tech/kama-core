//! GEÇİCİ yeniden-ihraç (B1 commit 1) — bir sonraki commit bu dosyayı SİLER.
//!
//! Burada KM-SEAM-AT-001 §1/§3'ün test-yerel aynası dururdu (prodüksiyon trait'i yokken). Trait ve
//! tipler artık `kama-contracts` crate'inde (`kama_contracts::s1`, `kama_contracts::handles`); süit
//! doğrudan onları kullanır ve `scripts/ci/seam-api-drift.py` spec'i O kaynakla karşılaştırır.
//!
//! Aynanın iki bilinçli sapması onunla birlikte kalktı: `Deadline = LogicalTick` (SEAM-OPEN-2;
//! sözleşme tipi `std::time::Instant` kullanır) ve iki variant'lı `CloseReason` (SEAM-OPEN-3; küme
//! artık kapalı: `Superseded`, `UserClosed`, `PeerClosed`, `KeyHygiene`).

#![allow(unused_imports)]

pub use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
pub use kama_contracts::s1::{
    AbandonReason, CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered,
    DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent, TransportCapabilities,
    TransportError, UnitBytes,
};
