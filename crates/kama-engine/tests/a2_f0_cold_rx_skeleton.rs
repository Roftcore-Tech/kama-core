//! A2-F0 kabul-pinleri — ColdRxState iskeleti (DR-CS-1b/A2).
//!
//! Pinler: (1) [CAL]-SEALED sabitler freeze-r2 (sha256=5b298244…) ile BİREBİR — tuning/gevşeme bu
//! testi kırar; (2) türetim-tutarlılığı (C-1/C-4/C-5 aynaları); (3) F0-stub'ları typed-error döner
//! (reachable todo!() yok); (4) K-9: GlobalColdLedger saf-tip (engine'de instance-lifecycle yok).

#![cfg(feature = "experimental-cold-start")]

use kama_engine::cold_rx_state::{
    ColdAdmitRequest, ColdRxState, GlobalColdLedger, GLOBAL_COLD_BYTE_CAP,
    GLOBAL_COLD_RETAINED_MEMORY_BUDGET, MAX_SHARDS_BY_BLOCK_SHAPE,
    RX_COLD_OPEN_BLOCK_CAP_PER_SESSION, RX_COLD_RETAINED_BYTE_CAP_PER_SESSION,
    RX_COLD_SHARDS_PER_BLOCK_CAP, RX_COLD_TOTAL_SHARD_CAP_PER_SESSION,
    RX_PREPARAM_SHARD_CAP_PER_SESSION, SESSION_COUNT_CAP, TOMBSTONE_REGISTRY_CAP,
};
use kama_engine::{EngineError, MAX_PREPARAM_SHARDS};

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
// Sabit-üzerinde-assert BİLEREK: bu test SEALED-değer/türetim değişikliğini yakalayan pindir
// (const-fold edilebilir olması pinin amacını değiştirmez).
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

/// Pin-3: F0-stub'ları TYPED-error döner (panik/todo değil); ledger'lar değişmez.
#[test]
fn a2_f0_stubs_return_typed_errors_and_do_not_mutate() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    let before_cold = cold;
    let before_global = global;

    let req = ColdAdmitRequest {
        block_id: 0,
        esi: 0,
        retained_cost_bytes: 1536,
        pre_param: true,
    };
    assert_eq!(
        cold.admit_cold_shard(0, req, &mut global),
        Err(EngineError::FeatureNotImplemented("a2_f1_admit"))
    );
    assert_eq!(
        cold.release_session(&mut global),
        Err(EngineError::FeatureNotImplemented("a2_f1_release"))
    );
    // Stub hiçbir ledger'ı değiştirmez (iki-seviye-atomiklik F1-öncesi vakum-hali).
    assert_eq!(cold, before_cold);
    assert_eq!(global, before_global);
}

/// Pin-4 (K-9): GlobalColdLedger saf-tip — Default/new boş-defter; diag'lar sıfır (C-8 alanları var).
#[test]
fn a2_f0_global_ledger_pure_type_with_c8_diag_fields() {
    let g = GlobalColdLedger::new();
    assert_eq!(g.retained_bytes, 0);
    assert_eq!(g.session_count, 0);
    assert_eq!(g.tombstone_count, 0);
    assert_eq!(g.diag.global_cold_budget_exceeded, 0);
    assert_eq!(g.diag.cold_admission_rejected_by_global_budget, 0);
    assert_eq!(g.diag.active_sessions_at_global_exhaustion, 0);
    assert_eq!(g.diag.distinct_authenticated_peers_at_global_exhaustion, 0);
}
