//! Kama `adaptive-transport` — recovery-obsesif adaptif transport motoru.
//!
//! Type-agnostic + crypto-agnostic (INV-ENGINE-SOC): AEAD/anahtar TUTMAZ; yalnız
//! opak birim iletir + seq/block_id/shard_index metadata okur + ölçer.
//!
//! **Bu adımın kapsamı:** transport-control codec (ACK / seq-wrap — engine spec §2)
//! ve deterministik test altyapısı ([`clock`], [`sim`]). SRC / fuzzy / FEC / estimator
//! sonraki adımlarda.
//!
//! **ADR-05 deferred not:** `ACK_FRAME` / `FEC_SHARD` hop-transport frame'leridir;
//! byte-otoritesi transport engine spec §2 (ccr §35 DEĞİL — o yalnız SESSION_*
//! envelope'ları kapsar). Fixed-width big-endian burada yerel yapılır; `ccr-codec`
//! public API'si tasarlanınca ortak primitive'ler oraya çıkarılıp reconcile edilir.
#![forbid(unsafe_code)]

pub mod ack;
pub mod clock;
pub mod error;
pub mod seq;
pub mod sim;
