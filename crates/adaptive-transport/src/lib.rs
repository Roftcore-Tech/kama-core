//! Kama `adaptive-transport` — recovery-obsesif adaptif transport motoru.
//!
//! Type-agnostic + crypto-agnostic (INV-ENGINE-SOC): AEAD/anahtar TUTMAZ; yalnız
//! opak birim iletir + seq/block_id/shard_index metadata okur + ölçer.
//!
//! **Kapsam:** transport-control codec (ACK / seq-wrap — §2), RTT/BW estimator'lar
//! ([`rtt`], [`bw`] — §6/§7), SRC rate-control çekirdeği ([`rate`], [`pacer`] —
//! §8.2/§13), loss detection ([`loss`] — §5), fuzzy füzyon ([`fusion`] — §8.4/§15),
//! budget clamp ([`budget`] — §8.5), emergency forward-recovery ([`emergency`] — §11),
//! ARQ selective retransmit ([`arq`] — §9), FEC_SHARD framing codec ([`fec`] — §10.1)
//! ve deterministik test altyapısı ([`clock`], [`sim`]). Hepsi INV-RATE-1 (loss ⟂ rate).
//!
//! **Engine loop (iki yarı TAM):** [`sender`] Sender State Machine (§3) — state machine + ölçüm +
//! karar pipeline + DATA pacing + recovery İCRA (ARQ retransmit §9 / forward duplication §11);
//! [`receiver`] Receiver State Machine (§4) — dedup / reorder / FEC-buffer + SACK üretimi (§2).
//! Sıradaki: bench `KamaTransport`'u motora bağla → §4.4 Kama vs quinn benchmark.
//!
//! **ADR-05 deferred not:** `ACK_FRAME` / `FEC_SHARD` hop-transport frame'leridir;
//! byte-otoritesi transport engine spec §2 (ccr §35 DEĞİL — o yalnız SESSION_*
//! envelope'ları kapsar). Fixed-width big-endian burada yerel yapılır; `ccr-codec`
//! public API'si tasarlanınca ortak primitive'ler oraya çıkarılıp reconcile edilir.
#![forbid(unsafe_code)]

pub mod ack;
pub mod arq;
pub mod budget;
pub mod budget_kernel;
pub mod bw;
pub mod clock;
pub mod emergency;
pub mod error;
pub mod fec;
/// DR-FEC-FS-1 FS-HARNESS-A — A1-EARLY duplication emitter + redundancy cost-accounting (PIN-A/PIN-E).
#[cfg(feature = "experimental-fec-budget")]
pub mod fs_harness;
/// DR-FEC-FS-1 FS-HARNESS-A — trace-driven paired replay channel (pin-2 pairing semantics).
#[cfg(feature = "experimental-fec-budget")]
pub mod fs_trace;
pub mod fusion;
/// FS-FUSION-CAL-1 — versioned calibration + pure fusion policy + independent safety validator
/// (default OFF; advisory only; every failure mode degrades to ArqOnly).
pub mod fusion_policy;
pub mod loss;
pub mod pacer;
pub mod rate;
pub mod receiver;
pub mod rtt;
pub mod sender;
pub mod seq;
pub mod sim;
/// DR-ARQ-PRODUCT-TAIL-1 — bounded tail-loss recovery state machine (default OFF; INV-CS-4 supersession TDD).
pub mod tail_recovery;
#[cfg(feature = "experimental-fec-budget")]
pub mod udp_runner;

// Internal: ack/fec ortak bounds-checked reader (ADR-05 → ccr-codec).
mod read;
