//! A2-F1-A ADVERSARIAL LEDGER-PİNLERİ (DR-CS-1b/A2; Abkarada F1-belirleyici-listesi):
//! rollback (her eksen-reddi → her iki REZERVASYON byte-for-byte) · çift-release · eksenlerin
//! BİRBİRİNDEN-BAĞIMSIZ reddi · dedup · C-8-diag · session/tombstone cap'leri · P-1-çapa.

#![cfg(feature = "experimental-cold-start")]

use kama_engine::cold_rx_state::{
    CapKind, CapScope, ColdAdmitOutcome, ColdAdmitRequest, ColdReleaseReason, ColdRxState,
    GlobalColdLedger, GLOBAL_COLD_BYTE_CAP, RX_COLD_RETAINED_BYTE_CAP_PER_SESSION,
    SESSION_COUNT_CAP, TOMBSTONE_REGISTRY_CAP,
};
use kama_engine::EngineError;

fn req(block_id: u32, esi: u16, cost: u64, pre_param: bool) -> ColdAdmitRequest {
    ColdAdmitRequest {
        block_id,
        esi,
        is_repair: false,
        retained_cost_bytes: cost,
        pre_param,
    }
}

/// Reject-yolu atomiklik-yardımcısı: her iki REZERVASYON byte-for-byte önceki-durumda mı?
fn assert_reservations_unchanged(
    cold: &ColdRxState,
    cold_before: &ColdRxState,
    global: &GlobalColdLedger,
    global_before: &GlobalColdLedger,
) {
    assert_eq!(
        cold.res, cold_before.res,
        "session-rezervasyon değişti (atomiklik-ihlali)"
    );
    assert_eq!(
        global.res, global_before.res,
        "global-rezervasyon değişti (atomiklik-ihlali)"
    );
}

/// Eksen-1: SHARDS_PER_BLOCK — 24 distinct-shard dolar; 25. distinct → typed-reject + rollback.
#[test]
fn axis_shards_per_block_independent_reject() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    for esi in 0..24u16 {
        assert_eq!(
            cold.admit_cold_shard(1, req(5, esi, 100, false), &mut global),
            Ok(ColdAdmitOutcome::Admitted)
        );
    }
    let (cb, gb) = (cold.clone(), global);
    let e = cold.admit_cold_shard(1, req(5, 24, 100, false), &mut global);
    assert_eq!(
        e,
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Session,
            kind: CapKind::ShardsPerBlock,
            cap: 24,
            requested: 25
        })
    );
    assert_reservations_unchanged(&cold, &cb, &global, &gb);
    assert_eq!(cold.diag.session_cold_budget_exceeded, 1);
}

/// Eksen-2: OPEN_BLOCK — 8 blok dolar; 9. blok → typed-reject; MEVCUT-bloğa shard hâlâ kabul
/// (f4_di10: red = yeni-slot-yok, eviction-yok).
#[test]
fn axis_open_block_independent_reject_existing_block_unaffected() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    for b in 0..8u32 {
        assert_eq!(
            cold.admit_cold_shard(1, req(b, 0, 100, false), &mut global),
            Ok(ColdAdmitOutcome::Admitted)
        );
    }
    let (cb, gb) = (cold.clone(), global);
    let e = cold.admit_cold_shard(1, req(8, 0, 100, false), &mut global);
    assert_eq!(
        e,
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Session,
            kind: CapKind::OpenBlock,
            cap: 8,
            requested: 9
        })
    );
    assert_reservations_unchanged(&cold, &cb, &global, &gb);
    // Mevcut-blok yeni-shard: cap'e TAKILMAZ.
    assert_eq!(
        cold.admit_cold_shard(1, req(3, 1, 100, false), &mut global),
        Ok(ColdAdmitOutcome::Admitted)
    );
}

/// Eksen-3: PREPARAM alt-kapısı kontrol-sırasında OPEN_BLOCK'tan ÖNCE (192-preparam dolu →
/// 9.-blok-preparam-talebi PreparamShard-reddi üretir, OpenBlock değil).
#[test]
fn axis_preparam_order_before_open_block() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    for b in 0..8u32 {
        for esi in 0..24u16 {
            assert_eq!(
                cold.admit_cold_shard(1, req(b, esi, 10, true), &mut global),
                Ok(ColdAdmitOutcome::Admitted)
            );
        }
    }
    assert_eq!(cold.res.preparam_shards, 192);
    let (cb, gb) = (cold.clone(), global);
    let e = cold.admit_cold_shard(1, req(9, 0, 10, true), &mut global);
    assert_eq!(
        e,
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Session,
            kind: CapKind::PreparamShard,
            cap: 192,
            requested: 193
        })
    );
    assert_reservations_unchanged(&cold, &cb, &global, &gb);
}

/// Eksen-4: session-RETAINED_BYTE — byte-ekseni shard-sayısı-eksenlerinden bağımsız reddeder.
#[test]
fn axis_session_byte_independent_reject() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    for esi in 0..4u16 {
        assert_eq!(
            cold.admit_cold_shard(1, req(0, esi, 100 * 1024, false), &mut global),
            Ok(ColdAdmitOutcome::Admitted)
        );
    }
    let (cb, gb) = (cold.clone(), global);
    let e = cold.admit_cold_shard(1, req(0, 4, 100 * 1024, false), &mut global);
    assert_eq!(
        e,
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Session,
            kind: CapKind::RetainedByte,
            cap: RX_COLD_RETAINED_BYTE_CAP_PER_SESSION,
            requested: 500 * 1024
        })
    );
    assert_reservations_unchanged(&cold, &cb, &global, &gb);
}

/// Eksen-5 + ATOMİKLİK (session-OK + global-FAIL) + C-8: 32-session global'i tam-doldurur;
/// 33.-session'ın SESSION-EKSENLERİ-GEÇEN talebi GLOBAL'de reddedilir → 33.'nün session-rezervasyonu
/// byte-for-byte değişmez; C-8-diag'ları artar; snapshot session_count'u yazar.
#[test]
fn axis_global_byte_atomicity_and_c8_diag() {
    let mut global = GlobalColdLedger::new();
    let mut fillers: Vec<ColdRxState> = Vec::new();
    for _ in 0..32 {
        global.register_session().unwrap();
        let mut s = ColdRxState::new();
        assert_eq!(
            s.admit_cold_shard(
                1,
                req(0, 0, RX_COLD_RETAINED_BYTE_CAP_PER_SESSION, false),
                &mut global
            ),
            Ok(ColdAdmitOutcome::Admitted)
        );
        fillers.push(s);
    }
    assert_eq!(global.res.retained_bytes, GLOBAL_COLD_BYTE_CAP);

    global.register_session().unwrap(); // 33. session (SESSION_COUNT_CAP=64 içinde)
    let mut victim = ColdRxState::new();
    let (vb, gb) = (victim.clone(), global);
    let e = victim.admit_cold_shard(1, req(0, 0, 1536, false), &mut global);
    assert_eq!(
        e,
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Global,
            kind: CapKind::RetainedByte,
            cap: GLOBAL_COLD_BYTE_CAP,
            requested: GLOBAL_COLD_BYTE_CAP + 1536
        })
    );
    // ATOMİKLİK: session-rezervasyonu da global-rezervasyonu da byte-for-byte önceki-durumda.
    assert_reservations_unchanged(&victim, &vb, &global, &gb);
    // C-8 (yalnız diag-katmanı hareket eder):
    assert_eq!(global.diag.global_cold_budget_exceeded, 1);
    assert_eq!(global.diag.cold_admission_rejected_by_global_budget, 1);
    assert_eq!(global.diag.active_sessions_at_global_exhaustion, 33);
    // Session-diag'ı ARTMAZ (global-eksen reddi):
    assert_eq!(victim.diag.session_cold_budget_exceeded, 0);
}

/// ATOMİKLİK ters-yön (global-OK + session-FAIL): global-rezervasyon byte-for-byte değişmez.
#[test]
fn atomicity_global_untouched_on_session_reject() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    assert_eq!(
        cold.admit_cold_shard(1, req(0, 0, 400 * 1024, false), &mut global),
        Ok(ColdAdmitOutcome::Admitted)
    );
    let gb = global;
    let e = cold.admit_cold_shard(1, req(0, 1, 100 * 1024, false), &mut global);
    assert!(matches!(
        e,
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Session,
            kind: CapKind::RetainedByte,
            ..
        })
    ));
    assert_eq!(
        global.res, gb.res,
        "global-rezervasyon session-reddinde değişti"
    );
}

/// Dedup (F1-A-düzeltmesi; Abkarada): kimlik (block_id,esi) — aynı-role → Duplicate;
/// FARKLI-role → typed ShardRoleConflict; İKİ-durumda-da LİTERAL byte-for-byte ledger-değişmezliği
/// (delta=0; mevcut-kayıt değişmez; pre-PARAM role-flip'le ikinci-rezervasyon yapısal-engelli).
#[test]
fn dedup_identity_block_esi_role_is_verified_property() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    assert_eq!(
        cold.admit_cold_shard(1, req(0, 7, 100, true), &mut global),
        Ok(ColdAdmitOutcome::Admitted)
    );

    // Durum-1: aynı-kimlik + AYNI-role → Duplicate; byte-for-byte pin (res + TÜM-state).
    let (cb, gb) = (cold.clone(), global);
    assert_eq!(
        cold.admit_cold_shard(2, req(0, 7, 100, true), &mut global),
        Ok(ColdAdmitOutcome::Duplicate)
    );
    assert_reservations_unchanged(&cold, &cb, &global, &gb);
    assert_eq!(
        cold, cb,
        "Duplicate blok-defterini de değiştirdi (literal-değişmezlik ihlali)"
    );

    // Durum-2: aynı-kimlik + FARKLI-role → typed ShardRoleConflict; byte-for-byte pin;
    // mevcut-kayıt (role=pre_param-rezervi dahil) DEĞİŞMEZ.
    let (cb2, gb2) = (cold.clone(), global);
    let mut flip = req(0, 7, 100, true);
    flip.is_repair = true;
    assert_eq!(
        cold.admit_cold_shard(2, flip, &mut global),
        Err(EngineError::ShardRoleConflict {
            block_id: 0,
            esi: 7
        })
    );
    assert_reservations_unchanged(&cold, &cb2, &global, &gb2);
    assert_eq!(
        cold, cb2,
        "RoleConflict state-değiştirdi (mevcut-kayıt korunmalı)"
    );
    assert_eq!(
        cold.res.total_shards, 1,
        "role-flip ikinci-rezervasyon aldı (yapısal-engel ihlali)"
    );
    assert_eq!(cold.res.preparam_shards, 1);

    // Farklı-esi hâlâ ayrı-kimlik (normal-yol regresyonu):
    assert_eq!(
        cold.admit_cold_shard(2, req(0, 8, 100, false), &mut global),
        Ok(ColdAdmitOutcome::Admitted)
    );
    assert_eq!(cold.res.total_shards, 2);
}

/// Çift-release (blok): ilk-iade bütçeyi düşürür + slot-açar; İKİNCİ-iade typed-invariant-hatası.
#[test]
fn release_block_returns_budget_and_double_release_typed() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    for esi in 0..3u16 {
        cold.admit_cold_shard(1, req(2, esi, 512, esi == 0), &mut global)
            .unwrap();
    }
    let freed = cold
        .release_block(2, ColdReleaseReason::Delivered, &mut global)
        .unwrap();
    assert_eq!(freed.bytes, 3 * 512);
    assert_eq!(freed.shards, 3);
    assert_eq!(freed.preparam_shards, 1);
    assert_eq!(cold.res.retained_bytes, 0);
    assert_eq!(cold.res.open_blocks, 0);
    assert_eq!(global.res.retained_bytes, 0);
    // Çift-release → typed (sessiz-no-op DEĞİL):
    assert_eq!(
        cold.release_block(2, ColdReleaseReason::Delivered, &mut global),
        Err(EngineError::ColdInvariantViolation(
            "release_unknown_or_double"
        ))
    );
    // Slot gerçekten açıldı (yeniden-admit):
    assert_eq!(
        cold.admit_cold_shard(9, req(2, 0, 64, false), &mut global),
        Ok(ColdAdmitOutcome::Admitted)
    );
}

/// Timeout-release yolu (DeadlineEvicted) aynı mekanik-iadeyi kullanır (F2 deadline-tetiği buna bağlanır).
#[test]
fn release_block_deadline_evicted_path() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    cold.admit_cold_shard(1, req(0, 0, 1536, true), &mut global)
        .unwrap();
    let freed = cold
        .release_block(0, ColdReleaseReason::DeadlineEvicted, &mut global)
        .unwrap();
    assert_eq!(freed.bytes, 1536);
    assert_eq!(global.res.retained_bytes, 0);
}

/// Session-release: İDEMPOTENT (belgeli); tüm rezervler globale döner; ikinci çağrı no-op.
#[test]
fn release_session_idempotent() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    for b in 0..3u32 {
        cold.admit_cold_shard(1, req(b, 0, 1000, true), &mut global)
            .unwrap();
    }
    assert_eq!(global.res.retained_bytes, 3000);
    cold.release_session(&mut global).unwrap();
    assert_eq!(cold.res.retained_bytes, 0);
    assert_eq!(cold.res.open_blocks, 0);
    assert_eq!(global.res.retained_bytes, 0);
    let gb = global;
    cold.release_session(&mut global).unwrap(); // idempotent
    assert_eq!(global.res, gb.res);
}

/// SESSION_COUNT_CAP: 64 dolu → 65. typed-reject; unregister-underflow typed.
#[test]
fn session_count_cap_and_underflow() {
    let mut global = GlobalColdLedger::new();
    for _ in 0..SESSION_COUNT_CAP {
        global.register_session().unwrap();
    }
    assert!(matches!(
        global.register_session(),
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Global,
            kind: CapKind::SessionCount,
            cap: 64,
            requested: 65
        })
    ));
    for _ in 0..SESSION_COUNT_CAP {
        global.unregister_session().unwrap();
    }
    assert_eq!(
        global.unregister_session(),
        Err(EngineError::ColdInvariantViolation(
            "session_count_underflow"
        ))
    );
}

/// TOMBSTONE_REGISTRY_CAP (K-1: registry-dolu → fail-closed typed-diag) + underflow typed.
#[test]
fn tombstone_registry_cap_and_underflow() {
    let mut global = GlobalColdLedger::new();
    for _ in 0..TOMBSTONE_REGISTRY_CAP {
        global.reserve_tombstone().unwrap();
    }
    assert!(matches!(
        global.reserve_tombstone(),
        Err(EngineError::ColdCapExceeded {
            scope: CapScope::Global,
            kind: CapKind::TombstoneRegistry,
            cap: 64,
            requested: 65
        })
    ));
    for _ in 0..TOMBSTONE_REGISTRY_CAP {
        global.release_tombstone().unwrap();
    }
    assert_eq!(
        global.release_tombstone(),
        Err(EngineError::ColdInvariantViolation(
            "tombstone_count_underflow"
        ))
    );
}

/// P-1 çapası: first_arrival yalnız İLK shard'da yazılır; sonraki shard'lar DEĞİŞTİRMEZ
/// (non-renewable-deadline'ın defter-tabanı; deadline-hükmü F2'de).
#[test]
fn first_arrival_anchor_non_renewable_base() {
    let mut cold = ColdRxState::new();
    let mut global = GlobalColdLedger::new();
    cold.admit_cold_shard(1000, req(0, 0, 10, false), &mut global)
        .unwrap();
    cold.admit_cold_shard(9999, req(0, 1, 10, false), &mut global)
        .unwrap();
    assert_eq!(cold.block_first_arrival_ns(0), Some(1000));
}
