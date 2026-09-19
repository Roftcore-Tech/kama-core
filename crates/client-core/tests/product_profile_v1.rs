//! [KAMA v1 ÜRÜN PROFİLİ] Sevk yapılandırmasının invaryant pinleri.
//!
//! Bu dosya "Kama v1" adının ne anlama geldiğini kilitler. Buradaki bir assertion'ı değiştirmek
//! ÜRÜN DAVRANIŞINI değiştirmektir ve ayrı bir sevk kararı ister (denetim:
//! `reports/transport-v1-audit/`). Benchmark kolları da aynı profilden türer — bu pin,
//! "PRODUCT DEFAULT ≠ BENCH SEVK CONFIG" sınıfı hatanın tekrarını engeller.

use client_core::runtime::{FecStructurePolicy, LossConfirmHorizon, PersistMode, RuntimeConfig};
use client_core::KamaV1Profile;

#[test]
fn product_default_is_the_v1_profile() {
    let d = RuntimeConfig::default();
    let p = KamaV1Profile::runtime_config();
    assert_eq!(d.max_unit_bytes, p.max_unit_bytes);
    assert_eq!(d.max_message_bytes, p.max_message_bytes);
    assert_eq!(d.initial_window_bytes, p.initial_window_bytes);
    assert_eq!(d.fec.map(|f| f.profile_id), p.fec.map(|f| f.profile_id));
    assert_eq!(
        d.fec.and_then(|f| f.manual_r),
        p.fec.and_then(|f| f.manual_r)
    );
    assert_eq!(
        d.structure_gate_level_permille,
        p.structure_gate_level_permille
    );
}

/// v1 = FEC AÇIK ve ADAPTİF. `fec: None` (denetim B-1) ürünü FEC'siz bırakıyordu: spekülatif kol
/// açık olmasına rağmen yönettiği alt-sistem yoktu.
#[test]
fn product_v1_ships_adaptive_fec() {
    let c = RuntimeConfig::default();
    let f = c.fec.expect("v1 profilinde FEC AÇIK olmalı (denetim B-1)");
    assert_eq!(f.profile_id, KamaV1Profile::FEC_PROFILE_ID);
    assert_eq!(f.profile_id, 0x05, "K=8, E=1024 sevk profili");
    assert_eq!(
        f.manual_r, None,
        "Controller/adaptif mod: parity oranını arbiter belirler (statik r DEĞİL)"
    );
}

/// Birim tavanı FEC symbol'üne TAM oturmalı: `max_unit_bytes + SOURCE_PREFIX_LEN ≤ profile.e`.
/// Sabit katalogtan türetilir — profil değişirse bu test kırılır (tek-kaynak kilidi).
#[test]
fn fec_v1_profile_is_symbol_aligned() {
    let cap = client_core::runtime::fec_chunk_capacity(KamaV1Profile::FEC_PROFILE_ID)
        .expect("sevk profili katalogda olmalı");
    assert_eq!(
        KamaV1Profile::MAX_UNIT_BYTES,
        cap,
        "MAX_UNIT_BYTES = E − SOURCE_PREFIX_LEN (katalog tek kaynak)"
    );
    assert_eq!(RuntimeConfig::default().max_unit_bytes, cap);
}

/// Profilin geri kalanı: kanıtlanmış mekanizmalar açık, kalibre edilmemişler bilinçli kapalı.
#[test]
fn product_v1_profile_is_pinned() {
    let c = RuntimeConfig::default();
    // AÇIK
    assert_eq!(c.initial_window_bytes, KamaV1Profile::INITIAL_WINDOW_BYTES);
    assert_eq!(
        c.initial_window_bytes, 2400,
        "[IW] clamp düzeltmesi sevk edilir"
    );
    assert_eq!(c.structure_gate_level_permille, Some(250));
    assert!(c.fec_loss_risk.enabled, "spekülatif kol (bounded_v1) açık");
    assert_eq!(c.fec_loss_risk.bootstrap_cap_permille, 250);
    assert_eq!(c.fec_loss_risk.persistent_cap_permille, 250);
    assert_eq!(
        c.fec_loss_risk.reorder_suppress_permille, 300,
        "C4 guard G1"
    );
    assert_eq!(
        c.fec_loss_risk.persist_mode,
        PersistMode::LearnedSpuriousAge
    );
    // KAPALI (bilinçli; her biri ayrı ölçüm kapısına bağlı)
    assert!(!c.bootstrap_rtt, "DR-CS-3 merdiveni kalibre edilmemiş");
    assert!(!c.bootstrap_fast_ladder);
    assert!(
        c.fusion_policy.is_none(),
        "kalibre füzyon politikası opt-in"
    );
    assert_eq!(c.fec_structure_policy, FecStructurePolicy::Q18Iid);
    assert_eq!(c.max_message_bytes, None, "fragmenter açık opt-in");
    assert!(matches!(
        c.loss_confirm_horizon,
        LossConfirmHorizon::CadenceLegacy
    ));
}
