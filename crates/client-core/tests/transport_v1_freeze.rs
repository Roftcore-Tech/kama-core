//! [FAZ-6 DONDURMA] Transport v1 SÖZLEŞME pinleri.
//!
//! `product_profile_v1.rs` profil alanlarını pinler; bu dosya DONDURMANIN KENDİSİNİ pinler:
//! kimlik sabitleri, sevk edilen alt-yapılandırmaların (spekülatif kol, tail-recovery) tam
//! değerleri ve FEC profil geometrisi. Buradaki bir assertion'ı değiştirmek = Transport v1'in
//! davranışını değiştirmek = EVOLUTION.md §22'deki altı beyan + yeni sürüm kimliği.
//!
//! Uygulama detayı KOPYALANMAZ; yalnız sözleşme (docs/transport-v1/PRODUCT-PROFILE.md) test edilir.

use adaptive_transport::sender::{FecLossRiskConfig, PersistMode};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use client_core::runtime::{
    fec_chunk_capacity, RuntimeConfig, TRANSPORT_V1_ARCHITECTURE, TRANSPORT_V1_PROFILE_ID,
};
use client_core::KamaV1Profile;

/// Kimlikler TEK KAYNAKTAN ve donmuş değerlerde. Bench etiketi bu sabiti okur (INV-008).
#[test]
fn kimlik_sabitleri_donmus() {
    assert_eq!(TRANSPORT_V1_PROFILE_ID, "kama-v1-full");
    assert_eq!(TRANSPORT_V1_ARCHITECTURE, "transport-v1");
}

/// FEC geometrisi: profil 0x05 → birim tavanı 1013 = E 1024 − SOURCE_PREFIX 11 (katalog tek kaynak).
#[test]
fn fec_profil_geometrisi_donmus() {
    assert_eq!(KamaV1Profile::FEC_PROFILE_ID, 0x05);
    assert_eq!(fec_chunk_capacity(0x05), Some(1013));
    assert_eq!(KamaV1Profile::MAX_UNIT_BYTES, 1013);
    assert_eq!(KamaV1Profile::INITIAL_WINDOW_BYTES, 2400);
    assert_eq!(KamaV1Profile::STRUCTURE_GATE_LEVEL_PERMILLE, 250);
}

/// Spekülatif FEC kolu `bounded_v1`: ürün profilindeki değer, kaynak constructor'la ALAN ALAN aynı.
#[test]
fn spekulatif_kol_bounded_v1_donmus() {
    let c = RuntimeConfig::default().fec_loss_risk;
    let v = FecLossRiskConfig::bounded_v1();
    assert!(c.enabled && v.enabled);
    assert_eq!(
        (c.bootstrap_cap_permille, c.persistent_cap_permille),
        (v.bootstrap_cap_permille, v.persistent_cap_permille)
    );
    assert_eq!(
        (c.bootstrap_cap_permille, c.persistent_cap_permille),
        (250, 250)
    );
    assert_eq!(c.persist_mode, PersistMode::LearnedSpuriousAge);
    assert_eq!(
        (
            c.persist_srtt_permille,
            c.persist_rho_permille,
            c.persist_q_permille
        ),
        (500, 1250, 950),
        "persistence P2: taban 0.5×srtt, ρ 1.25, Q95"
    );
    assert_eq!(c.reorder_suppress_permille, 300, "C4 guard G1");
}

/// Tail-recovery `validated_v1`: ClientSession'ın kurduğu sevk yapılandırması.
#[test]
fn tail_recovery_validated_v1_donmus() {
    let t = TailRecoveryConfig::validated_v1();
    assert!(t.enabled);
    assert_eq!(t.min_pto_ns, 10_000_000);
    assert_eq!(t.max_pto_ns, 10_000_000_000);
    assert_eq!(t.initial_pto_ns, 1_000_000_000, "RTT yokken 1 s (Karn)");
    assert_eq!(t.max_backoff_pto_ns, 30_000_000_000);
    assert_eq!(t.max_ack_delay_ns, 25_000_000);
    assert_eq!(t.max_tail_probes, 3);
}

/// Bilinçli KAPALI mekanizmalar: her biri bir ADR/sınır kararına bağlı; sessizce açılamaz.
#[test]
fn bilincli_kapali_mekanizmalar_donmus() {
    let c = RuntimeConfig::default();
    assert!(
        !c.bootstrap_rtt,
        "ADR-zero-ack-bootstrap: v1'de KAPALI (LIM-001)"
    );
    assert!(!c.bootstrap_fast_ladder, "ADR-zero-ack-bootstrap");
    assert!(c.fusion_policy.is_none(), "opt-in ölçüm kapısı");
    assert_eq!(
        c.max_message_bytes, None,
        "fragmenter opt-in (tel formatı değişir)"
    );
    assert_eq!(
        c.ts_sample_period_ns, None,
        "handoff enstrümanı salt gözlem, default kapalı"
    );
}

/// Sınır sabitleri (SR-AUDIT-001 bounded) ve eş/kota hizaları.
#[test]
fn sinirlar_ve_kotalar_donmus() {
    let c = RuntimeConfig::default();
    assert_eq!(c.max_inflight_units, 4096);
    assert_eq!((c.offer_queue, c.delivery_queue), (1024, 1024));
    assert_eq!(c.max_peers, 64, "SESSION_COUNT_CAP hizası");
    assert_eq!(c.quota_r, 4, "§16.1: R=8 dar dönüşte kota doyumu");
    assert_eq!(c.drain_cap.as_secs(), 3);
    assert_eq!(c.idle_hold.as_secs(), 105, "SESSION_IDLE_HOLD hizası");
}
