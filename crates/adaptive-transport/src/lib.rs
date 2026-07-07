//! Kama `adaptive-transport` — recovery-obsesif adaptif transport motoru.
//!
//! Type-agnostic + crypto-agnostic (INV-ENGINE-SOC): AEAD/anahtar TUTMAZ; yalnız
//! opak birim iletir + seq/block_id/shard_index metadata okur + ölçer.
//!
//! **Kapsam:** transport-control codec (ACK / seq-wrap — §2), RTT/BW estimator'lar
//! ([`rtt`], [`bw`] — §6/§7), SRC rate-control çekirdeği ([`rate`], [`pacer`] —
//! §8.2/§13), loss detection ([`loss`] — §5, recovery sinyali) ve deterministik test
//! altyapısı ([`clock`], [`sim`]). Hepsi INV-RATE-1 (loss ⟂ rate). Sıradaki: §8.4
//! FEC↔ARQ füzyon + §11 forward-recovery (Tier-1 kazanma onlara bağlı, §4.4).
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
pub mod loss;
pub mod pacer;
pub mod rate;
pub mod rtt;
pub mod seq;
pub mod sim;
