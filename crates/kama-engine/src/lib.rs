//! # kama-engine — kompozisyon/orkestrasyon düğümü (Sprint-5d)
//!
//! noise-wire (AEAD/param_bind/`OpenedTransportShard`) + fec-codec (RS reconstruct/`by_id`) +
//! schema (`ParamFields`/`Control`) birleştiren ÜRETİM engine-katmanı. Bugüne-kadar bu-kompozisyon
//! yalnız `kama-integration` test-harness'ında yaşıyordu (üretim-grafiği-DIŞI). SoC: engine transport-
//! core'un ÜSTÜ; `adaptive-transport`'u üretimde TUTMAZ (yalnız direktif üretir — B2-bootstrap testi
//! Sender'ı dev-dep ile sürer).
//!
//! ## Sprint-5d kapsamı
//! - **5d-1 PARAM-bootstrap (B2):** [`param_bootstrap`] proaktif PARAM_SET-planı (PARAM_ACK-beklemeden
//!   FEC-emit direktifi) + [`param_state`] receiver bound-param gate (hash-verify fail-closed) +
//!   [`preparam_hold`] bounded pre-param hold (SR-AUDIT-001).
//! - **5d-2 decoder-admission köprüsü (INV-FEC-1):** [`admission`] — `OpenedTransportShard`
//!   (unforgeable witness) → FEC-decode. Type-gate: forge open()-atlayamaz → `OpenedTransportShard`
//!   üretemez → decode'a ULAŞAMAZ.
//!
//! ## N8-firewall (mühürlü-sınır)
//! Engine decode-başarısı (RECOVERED) BtlBw/delivered-EAI/W_BTLBW kredi-tabanına ASLA girmez —
//! kredi yalnız gerçek-wire-ACK (sender.bw.rs, mühürlü). RECOVERED = logical-teslim ≠ ACK.

pub mod admission;
pub mod error;
pub mod param_bootstrap;
pub mod param_state;
pub mod preparam_hold;
pub mod recovered_map;

pub use admission::admit_and_decode;
pub use error::EngineError;
pub use param_bootstrap::{plan_param_bootstrap, ParamAckTracker, ParamSetPlan};
pub use param_state::{BoundParam, ParamState};
pub use preparam_hold::{PreParamHold, MAX_PREPARAM_SHARDS};
pub use recovered_map::{RecoveredMap, ResolvedRecovered, RECOVERED_MAP_CAP};
