//! kama-integration — ENGINE-SEVİYESİ cross-crate integration harness (Sprint-5a).
//!
//! SoC (dependency-graph: fec-core(bağımsız) ← transport-core ← engine → noise-wire): fec-codec ve
//! noise-wire KARDEŞ-crate'lerdir; bu-harness ikisini + schema'yı ENGINE-gibi birleştirir. Production
//! adaptive-transport-entegrasyonu (paylaşılan-kod, §2-cerrahi-protokol) Sprint-5c. Burada mock-key.
//!
//! Bu-crate lib-kod içermez (yalnız tests/); üretim-grafiğine GİRMEZ.

#![forbid(unsafe_code)]
