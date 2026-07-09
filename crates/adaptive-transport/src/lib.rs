//! Kama `adaptive-transport` — recovery-obsesif adaptif transport motoru.
//!
//! Type-agnostic + crypto-agnostic (INV-ENGINE-SOC): AEAD/anahtar TUTMAZ; yalnız
//! opak birim iletir + seq/block_id/shard_index metadata okur + ölçer.
//!
//! **Kapsam:** transport-control codec (ACK / seq-wrap — §2), RTT/BW estimator'lar
//! ([`rtt`], [`bw`] — §6/§7), SRC rate-control çekirdeği ([`rate`], [`pacer`] —
//! §8.2/§13), loss detection ([`loss`] — §5), fuzzy füzyon karar motoru ([`fusion`] —
//! §8.4/§15), budget clamp ([`budget`] — §8.5) ve emergency forward-recovery ([`emergency`]
//! — §11: davranış + retreat hysteresis) + deterministik test altyapısı ([`clock`],
//! [`sim`]). Hepsi INV-RATE-1 (loss ⟂ rate). Sıradaki: §9 ARQ / §10.1 FEC codec → §4.4 benchmark.
//!
//! **ADR-05 deferred not:** `ACK_FRAME` / `FEC_SHARD` hop-transport frame'leridir;
//! byte-otoritesi transport engine spec §2 (ccr §35 DEĞİL — o yalnız SESSION_*
//! envelope'ları kapsar). Fixed-width big-endian burada yerel yapılır; `ccr-codec`
//! public API'si tasarlanınca ortak primitive'ler oraya çıkarılıp reconcile edilir.
#![forbid(unsafe_code)]

pub mod ack;
pub mod budget;
pub mod bw;
pub mod clock;
pub mod emergency;
pub mod error;
pub mod fusion;
pub mod loss;
pub mod pacer;
pub mod rate;
pub mod rtt;
pub mod seq;
pub mod sim;
