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

/// Test eşi: `dest_hash`'in ilk sekiz baytı `n` (big-endian), kalanı sıfır. Farklı `n` farklı eş.
pub fn test_peer(n: u64) -> kama_contracts::handles::PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[..8].copy_from_slice(&n.to_be_bytes());
    kama_contracts::handles::PeerRef::from_dest_hash(dest_hash)
}
