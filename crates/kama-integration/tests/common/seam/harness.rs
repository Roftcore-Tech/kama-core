//! App↔Transport seam harness'inin TEK giriş noktası.
//!
//! Test ikilileri bu dosyayı `#[path = "../common/seam/harness.rs"] mod harness;` ile dahil eder.
//! Alt modüller YALNIZ burada tanımlanır: bir dosyayı iki ayrı yerde `mod` etmek onu iki AYRI modül
//! olarak derler ve içindeki tipler farklı tipler olurdu — "aynı süit iki hedefte" iddiası tam da
//! burada sessizce kırılırdı.
//!
//! Sözleşme tipleri bu harness'te TANIMLANMAZ: `kama-contracts` crate'inden gelir
//! (`kama_contracts::s1`, `kama_contracts::handles`). B1'den (2026-10-05) önce burada bir test-yerel
//! ayna (`api.rs`) vardı; prodüksiyon tipi indiği için silindi.

#![allow(dead_code)]

#[path = "target.rs"]
pub mod target;

#[path = "contract.rs"]
pub mod contract;

/// [L-T1] Gerçek hedefin fikstürü (`client-core` adaptörü + gerçek kripto + UDP loopback).
#[path = "real.rs"]
pub mod real;

/// Test eşi: `dest_hash`'in ilk sekiz baytı `n` (big-endian), kalanı sıfır. Farklı `n` farklı eş.
pub fn test_peer(n: u64) -> kama_contracts::handles::PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[..8].copy_from_slice(&n.to_be_bytes());
    kama_contracts::handles::PeerRef::from_dest_hash(dest_hash)
}
