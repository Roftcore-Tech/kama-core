//! App↔Transport seam harness'inin TEK giriş noktası.
//!
//! Test ikilileri bu dosyayı `#[path = "../common/seam/harness.rs"] mod harness;` ile dahil eder.
//! Alt modüller YALNIZ burada tanımlanır: `api.rs`'i iki ayrı yerde `mod` etmek, aynı dosyayı iki
//! AYRI modül olarak derler ve `harness::api::PreparedUnit` ile
//! `harness::target::api::PreparedUnit` farklı tipler olurdu — "aynı süit iki hedefte" iddiası
//! tam da burada sessizce kırılırdı.

#![allow(dead_code)]

#[path = "api.rs"]
pub mod api;

#[path = "target.rs"]
pub mod target;

#[path = "contract.rs"]
pub mod contract;
