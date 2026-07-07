//! Kama `adaptive-transport` — recovery-obsesif adaptif transport motoru.
//!
//! Type-agnostic + crypto-agnostic (INV-ENGINE-SOC): AEAD/anahtar TUTMAZ; yalnız
//! opak birim iletir + seq/block_id/shard_index metadata okur + ölçer.
//!
//! **Kapsam:** transport-control codec (ACK / seq-wrap — engine spec §2), RTT/BW
//! estimator'lar ([`rtt`], [`bw`] — delivered-rate, loss-invariant BtlBw §6/§7) ve
//! deterministik test altyapısı ([`clock`], [`sim`]). SRC / fuzzy / rate-control
//! sonraki adımda (§8).
//!
//! **ADR-05 deferred not:** `ACK_FRAME` / `FEC_SHARD` hop-transport frame'leridir;
//! byte-otoritesi transport engine spec §2 (ccr §35 DEĞİL — o yalnız SESSION_*
//! envelope'ları kapsar). Fixed-width big-endian burada yerel yapılır; `ccr-codec`
//! public API'si tasarlanınca ortak primitive'ler oraya çıkarılıp reconcile edilir.
#![forbid(unsafe_code)]

pub mod ack;
pub mod bw;
pub mod clock;
pub mod error;
pub mod rtt;
pub mod seq;
pub mod sim;
