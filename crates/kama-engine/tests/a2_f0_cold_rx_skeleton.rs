//! A2-F0 kabul-pinleri (F1-A-evrimiyle) — SEALED-sabitler + türetim + saf-tip (DR-CS-1b/A2).
//!
//! F0-Pin-3 (typed-stub) F1-A'da DOLDURULDU → pin temel-canlılığa evrildi (stub-beklentisi
//! fazla-yaşamaz; davranış-pinleri a2_f1a_cold_ledger.rs'te). Pin-1/2/4 aynen.

#![cfg(feature = "experimental-cold-start")]

use kama_engine::cold_rx_state::{
    ColdAdmitOutcome, ColdAdmitRequest, ColdRxState, GlobalColdLedger, GLOBAL_COLD_BYTE_CAP,
    GLOBAL_COLD_RETAINED_MEMORY_BUDGET, MAX_SHARDS_BY_BLOCK_SHAPE,
    RX_COLD_OPEN_BLOCK_CAP_PER_SESSION, RX_COLD_RETAINED_BYTE_CAP_PER_SESSION,
    RX_COLD_SHARDS_PER_BLOCK_CAP, RX_COLD_TOTAL_SHARD_CAP_PER_SESSION,
    RX_PREPARAM_SHARD_CAP_PER_SESSION, SESSION_COUNT_CAP, TOMBSTONE_REGISTRY_CAP,
};
use kama_engine::MAX_PREPARAM_SHARDS;

/// Pin-1: SEALED-değerler freeze-r2 §3 birebir (Abkarada-mühürlü; değişiklik = yeni-versiyon+onay).
#[test]
fn a2_f0_sealed_cal_values_pinned() {
    assert_eq!(RX_COLD_OPEN_BLOCK_CAP_PER_SESSION, 8);
    assert_eq!(RX_COLD_SHARDS_PER_BLOCK_CAP, 24);
    assert_eq!(MAX_SHARDS_BY_BLOCK_SHAPE, 192);
    assert_eq!(RX_COLD_TOTAL_SHARD_CAP_PER_SESSION, 192);
    assert_eq!(RX_PREPARAM_SHARD_CAP_PER_SESSION, 192);
    assert_eq!(RX_COLD_RETAINED_BYTE_CAP_PER_SESSION, 480 * 1024);
    assert_eq!(GLOBAL_COLD_RETAINED_MEMORY_BUDGET, 15 * 1024 * 1024);
    assert_eq!(GLOBAL_COLD_BYTE_CAP, 15 * 1024 * 1024);
    assert_eq!(SESSION_COUNT_CAP, 64);
    assert_eq!(TOMBSTONE_REGISTRY_CAP, 64);
}

/// Pin-2: türetim-tutarlılığı (freeze-r2 C-1/C-4/C-5 aynaları; bağlayıcı-eksen = blok-şekli).
// Sabit-üzerinde-assert BİLEREK: bu test SEALED-değer/türetim değişikliğini yakalayan pindir.
#[allow(clippy::assertions_on_constants)]
#[test]
fn a2_f0_derivation_consistency() {
    assert_eq!(
        MAX_SHARDS_BY_BLOCK_SHAPE,
        RX_COLD_OPEN_BLOCK_CAP_PER_SESSION * RX_COLD_SHARDS_PER_BLOCK_CAP
    );
    assert_eq!(
        RX_COLD_TOTAL_SHARD_CAP_PER_SESSION,
        MAX_SHARDS_BY_BLOCK_SHAPE
    );
    assert_eq!(
        RX_PREPARAM_SHARD_CAP_PER_SESSION,
        RX_COLD_TOTAL_SHARD_CAP_PER_SESSION.min(MAX_PREPARAM_SHARDS as u64)
    );
    assert!(RX_PREPARAM_SHARD_CAP_PER_SESSION <= MAX_PREPARAM_SHARDS as u64);
    assert_eq!(
        32 * RX_COLD_RETAINED_BYTE_CAP_PER_SESSION,
        GLOBAL_COLD_BYTE_CAP
    );
    assert!(GLOBAL_COLD_BYTE_CAP <= GLOBAL_COLD_RETAINED_MEMORY_BUDGET);
}

/// Pin-3 (F1-A-evrimi): temel-canlılık — ilk admit iki-seviye commit yapar; defterler tutarlı artar.
#[test]
fn a2_f0_first_admit_commits_both_ledgers() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    let req = ColdAdmitRequest {
        block_id: 0,
        esi: 0,
        is_repair: false,
        retained_cost_bytes: 1536,
        pre_param: true,
    };
    assert_eq!(
        cold.admit_cold_shard(7, req, &mut global),
        Ok(ColdAdmitOutcome::Admitted)
    );
    assert_eq!(cold.res.retained_bytes, 1536);
    assert_eq!(cold.res.total_shards, 1);
    assert_eq!(cold.res.preparam_shards, 1);
    assert_eq!(cold.res.open_blocks, 1);
    assert_eq!(global.res.retained_bytes, 1536);
    assert_eq!(cold.block_first_arrival_ns(0), Some(7));
}

/// Pin-4 (K-9): GlobalColdLedger saf-tip — boş-defter; C-8 diag-alanları mevcut ve sıfır.
#[test]
fn a2_f0_global_ledger_pure_type_with_c8_diag_fields() {
    let g = GlobalColdLedger::new();
    assert_eq!(g.res.retained_bytes, 0);
    assert_eq!(g.res.session_count, 0);
    assert_eq!(g.res.tombstone_count, 0);
    assert_eq!(g.diag.global_cold_budget_exceeded, 0);
    assert_eq!(g.diag.cold_admission_rejected_by_global_budget, 0);
    assert_eq!(g.diag.active_sessions_at_global_exhaustion, 0);
    assert_eq!(g.diag.distinct_authenticated_peers_at_global_exhaustion, 0);
}
