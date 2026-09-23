//! Application katmanı (Hayri). Transport'a açılan tek yüzey `transport` modülüdür
//! (KM-SEAM-AT-001 / S1). Mesaj codec'i (AMP) B2.1'de bu crate'e eklenir.

mod transport;

pub use transport::{
    CloseReason, CompressionPolicy, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy,
    KamaTransport, PeerRef, PreparedUnit, SessionEvent, SessionHandle, TransportCapabilities,
    TransportError,
};
