//! [FUSION-POLICY] Kalibre edilmiş füzyon katmanının ÜRÜN YOLUNA kablolanması.
//!
//! KABLOLAMA BOŞLUĞU (denetim bulgusu): `fusion_policy.rs` 659 satırlık, kalibre edilmiş
//! (FS-FUSION-CAL-1) ve bağımsız güvenlik doğrulayıcılı tam bir karar katmanıydı — ve tüm ağaçta
//! ÜRETİMDE SIFIR ÇAĞIRANI vardı. `lib.rs`'te `pub mod fusion_policy;` ve kendi testleri dışında
//! hiçbir referans yoktu. Yani "FEC + ARQ füzyonu" ürünün içinde yazılıydı ama bağlı değildi;
//! sevk edilen karar, kodda açıkça "KALİBRE DEĞİL" etiketli `trusted_fixture()`'dan geliyordu.
//!
//! Bu dosya kabloyu pinler: politika KAPALIYKEN davranış byte-özdeş, AÇIKKEN oran onun
//! otoritesinde, ve her ret sebebi histogramda GÖRÜNÜR (sessiz sıfır yok).

use adaptive_transport::fusion_policy::{
    FallbackReason, FusionAction, FusionObservation, FusionPolicy, FusionSafetyConfig,
    FUSION_ACTION_KINDS,
};
use adaptive_transport::sender::*;
use std::time::Duration;

const MS: u64 = 1_000_000;

/// Akışı sürer: birimleri sun, converge için erken ACK ver, sonra timer pompala.
/// `acked` listesi kayıp desenini imal eder (ACK-ufku altındaki boşluklar kayıp ilan edilir).
fn drive(policy: Option<FusionPolicy>, units: u64, acked: Vec<u64>) -> Sender {
    let mut s = Sender::new();
    s.set_establish_probe(false);
    if let Some(p) = policy {
        s.set_fusion_policy(p);
    }
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=20u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    // İlk ACK: RTT örneği + PreConverge'den çıkış.
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        45_000 * MS,
    );
    for i in 1..=60u64 {
        s.on_event(Event::TimerFired(Timer::Pto), (45_000 + i * 200) * MS);
    }
    s
}

/// Default: politika KAPALI → hiçbir füzyon kararı sayılmaz, davranış eski yoldan gelir.
#[test]
fn policy_is_disabled_by_default_and_records_no_decisions() {
    let s = drive(None, 80, (1..400).filter(|q| q % 2 == 0).collect());
    assert_eq!(
        s.diag_fusion_actions(),
        [0; FUSION_ACTION_KINDS],
        "kapalı politika hiç karar kaydetmemeli (yol eski davranışta)"
    );
}

/// Politika AÇIK → her değerlendirmede bir aksiyon kaydedilir ve ret sebebi GÖRÜNÜR.
/// Bu, yapı-gate'te bir kez yaşanan "tek kovaya yığılan ret" hatasının tekrarını engeller.
#[test]
fn enabled_policy_records_every_decision_with_an_attributed_reason() {
    let s = drive(
        Some(FusionPolicy::calibration_v1()),
        80,
        (1..400).filter(|q| q % 2 == 0).collect(),
    );
    let h = s.diag_fusion_actions();
    let total: u64 = h.iter().sum();
    assert!(total > 0, "açık politika karar kaydetmeli; histogram={h:?}");
    assert_eq!(
        h[2], 0,
        "PolicyDisabled kovası AÇIK politikada dolamaz; histogram={h:?}"
    );
}

/// RTT örnek sayacı gerçekten sayıyor — güvenlik doğrulayıcısının `min_rtt_samples`(2) güven
/// kapısı bu değere bakar (rtt.rs ilk örnekte `rttvar = srtt/2` bootstrap'lar → jitter yapay
/// HIGH; ikinci örnekten önce güvenmemek kalibrasyonun kendi kuralı).
#[test]
fn rtt_sample_counter_feeds_the_confidence_gate() {
    let fresh = Sender::new();
    assert_eq!(fresh.diag_rtt_sample_count(), 0, "taze sender örneksizdir");
    let s = drive(None, 80, (1..400).filter(|q| q % 2 == 0).collect());
    assert!(
        s.diag_rtt_sample_count() >= 1,
        "ACK akışı RTT örneği üretmeli; got {}",
        s.diag_rtt_sample_count()
    );
}

/// Güvenlik doğrulayıcısı kalibrasyondan BAĞIMSIZ: bir kalibrasyon DAHA AZ isteyebilir, daha
/// fazlasını asla. Sert tavanı sıfıra çeken bir safety config, kalibrasyon ne isterse istesin
/// FEC'i açtırmaz.
#[test]
fn safety_bounds_are_independent_of_the_calibration() {
    use adaptive_transport::fusion_policy::{FusionObservation, FusionSafetyConfig};
    let obs = FusionObservation {
        feature_schema: 1,
        inputs: adaptive_transport::fusion::FusionInputs {
            loss_ppm: 200_000,
            bandwidth_bps: 110_000,
            jitter_ppm: 0,
            qd_ratio_permille: 0,
            ack_confidence_permille: 1000,
        },
        rtt_sample_count: 8,
        telemetry_age_ns: 0,
        converged: true,
        fec_actuation_available: true,
        deadline_over_srtt_permille: None, // v1 bu ekseni ölçmedi → kararı etkilemez
    };
    // Kalibrasyon bu noktada FEC ister…
    let permissive = FusionPolicy::calibration_v1();
    assert!(
        matches!(
            permissive.decide(&obs, 1200),
            FusionAction::FecAssist { .. }
        ),
        "önkoşul: kalibrasyon 200k ppm'de FEC istemeli"
    );
    // …ama bağımsız güvenlik tavanı sıfırsa AÇTIRMAZ.
    let clamped = FusionPolicy::calibration_v1().with_safety(FusionSafetyConfig {
        hard_max_redundancy_permille: 0,
        ..FusionSafetyConfig::default()
    });
    assert!(
        clamped.decide(&obs, 1200).is_arq_only(),
        "sert tavan kalibrasyonu EZMELİ (kalibrasyon daha fazlasını isteyemez)"
    );
}

/// Cold-start kapısı: oturum kurulmadan politika FEC açmaz — elle eklediğim PreConverge
/// guard'ının tasarlanmış hâli burada. Politika açıkken guard BU katmandan gelir.
#[test]
fn cold_start_degrades_to_arq_not_to_something_more_aggressive() {
    use adaptive_transport::fusion_policy::FusionObservation;
    let obs = FusionObservation {
        feature_schema: 1,
        inputs: adaptive_transport::fusion::FusionInputs {
            loss_ppm: 200_000,
            bandwidth_bps: 110_000,
            jitter_ppm: 0,
            qd_ratio_permille: 0,
            ack_confidence_permille: 1000,
        },
        rtt_sample_count: 8,
        telemetry_age_ns: 0,
        converged: false, // ← kurulmamış oturum
        fec_actuation_available: true,
        deadline_over_srtt_permille: None,
    };
    let a = FusionPolicy::calibration_v1().decide(&obs, 1200);
    assert!(
        a.is_arq_only(),
        "kurulmamış oturumda ArqOnly'ye düşmeli; got {a:?}"
    );
}

// ── [DEADLINE-EKSENİ] calibration_v2_deadline ────────────────────────────────────────────────
//
// Bu testler tek bir şeyi savunuyor: FEC-MARGINAL-R v1+v2'nin ÖLÇTÜĞÜ kutu ile kodun AÇTIĞI kutu
// aynı kalsın. Bu repo daha önce ölçülmemiş bölgede parity açan bir kapı sevk etti; tekrarını
// mekanik olarak engelliyoruz.

fn dl_obs(loss_ppm: u64, deadline_permille: Option<u32>) -> FusionObservation {
    FusionObservation {
        feature_schema: adaptive_transport::fusion_policy::FEATURE_SCHEMA_V2,
        inputs: adaptive_transport::fusion::FusionInputs {
            loss_ppm,
            // [ÖLÇÜLDÜ] Kalibrasyon hücrelerinde motorun bildirdiği BtlBw 35–58 kB/s bandında.
            // `calibration_v1`'in varsaydığı 110 kB/s (= link kapasitesi) ürün yolunda HİÇ
            // gözlenmez; testin o değeri kullanması bölge kapısını sahte-geçirirdi.
            bandwidth_bps: 40_000,
            jitter_ppm: 0,
            qd_ratio_permille: 0,
            ack_confidence_permille: 1000,
        },
        rtt_sample_count: 8,
        telemetry_age_ns: 0,
        converged: true,
        fec_actuation_available: true,
        deadline_over_srtt_permille: deadline_permille,
    }
}

/// Kayıp ölçülen noktada (%20) ve bölge içinde — v1 mantığı burada FEC açardı. v2 AÇMAZ:
/// kararın dayandığı eksen bildirilmemiş. Kayıp oranından açmak ölçümün çürüttüğü davranıştır.
#[test]
fn v2_bildirilmemis_deadline_ile_parity_acmaz() {
    let a = FusionPolicy::calibration_v2_deadline().decide(&dl_obs(200_000, None), 1200);
    assert_eq!(
        a,
        FusionAction::FallbackArq(FallbackReason::NoDeadlineDeclared)
    );
}

/// 4×RTT = 4000‰. Ölçümde bu noktada kazanç ≈0, iid20/r2'de NEGATİF (v1 FINAL-REPORT Bulgu 2).
#[test]
fn v2_gevsek_deadline_ile_parity_acmaz() {
    let a = FusionPolicy::calibration_v2_deadline().decide(&dl_obs(200_000, Some(4_000)), 1200);
    assert_eq!(
        a,
        FusionAction::FallbackArq(FallbackReason::DeadlineTooLoose)
    );
}

/// d_msg = 2×RTT (2000‰), kayıp %20 → ölçülen fayda noktası: +10.4 pp (iid20) / +19.2 pp (cor20),
/// her ikisi de CI-ayrık. Seviye r=2 @ K=8 = 250‰; v1 ölçtü ki r=4 iki katı parity ile AYNI
/// kurtarmayı verir (781 vs 782 birim), yani verim yarıya düşer.
#[test]
fn v2_olculen_kutuda_r2_seviyesini_ister() {
    match FusionPolicy::calibration_v2_deadline().decide(&dl_obs(200_000, Some(2_000)), 1200) {
        FusionAction::FecAssist {
            redundancy_permille,
            ..
        } => assert_eq!(redundancy_permille, 250),
        other => panic!("ölçülen kutuda FecAssist beklenirdi, gelen: {other:?}"),
    }
}

/// %30 kayıp HİÇ ölçülmedi → bölge dışı, fail-safe.
#[test]
fn v2_bolge_disi_kayipta_parity_acmaz() {
    let a = FusionPolicy::calibration_v2_deadline().decide(&dl_obs(300_000, Some(2_000)), 1200);
    assert_eq!(
        a,
        FusionAction::FallbackArq(FallbackReason::UnsupportedRegion)
    );
}

/// v1 bu ekseni hiç ölçmedi; kısıtsız kalmalı — yoksa geçmiş davranışı sessizce değiştirmiş olurduk.
#[test]
fn v1_kalibrasyonu_deadline_ekseninden_etkilenmez() {
    let mut with = dl_obs(200_000, Some(9_999));
    let mut without = dl_obs(200_000, None);
    with.feature_schema = 1;
    without.feature_schema = 1;
    let p = FusionPolicy::calibration_v1();
    assert_eq!(p.decide(&with, 1200), p.decide(&without, 1200));
}

/// Kuruluş kapısı deadline kapısından ÖNCE gelmeli: srtt yokken oran zaten türetilemez.
#[test]
fn v2_kurulmamis_oturumda_cold_start_sebebini_verir() {
    let mut obs = dl_obs(200_000, None);
    obs.converged = false;
    let a = FusionPolicy::calibration_v2_deadline().decide(&obs, 1200);
    assert_eq!(a, FusionAction::FallbackArq(FallbackReason::ColdStart));
}

/// Bağımsız güvenlik doğrulayıcısı hâlâ üstte: kalibrasyon 250‰ isterken sert tavan 0 ise açtırmaz.
#[test]
fn v2_dogrulayici_kalibrasyondan_fazlasini_veremez() {
    let clamped = FusionPolicy::calibration_v2_deadline().with_safety(FusionSafetyConfig {
        hard_max_redundancy_permille: 0,
        ..FusionSafetyConfig::default()
    });
    assert!(clamped
        .decide(&dl_obs(200_000, Some(2_000)), 1200)
        .is_arq_only());
}

/// Her yeni ret sebebi histogramda AYRI kova almalı — sessiz sıfır yok.
#[test]
fn yeni_ret_sebepleri_histogramda_ayrisir() {
    let p = FusionPolicy::calibration_v2_deadline();
    let i_none = p.decide(&dl_obs(200_000, None), 1200).index();
    let i_loose = p.decide(&dl_obs(200_000, Some(4_000)), 1200).index();
    let i_region = p.decide(&dl_obs(300_000, Some(2_000)), 1200).index();
    assert_ne!(i_none, i_loose);
    assert_ne!(i_none, i_region);
    assert_ne!(i_loose, i_region);
    for i in [i_none, i_loose, i_region] {
        assert!(i < FUSION_ACTION_KINDS, "kova {i} histogram dışında");
    }
}

// ── [ONAY UFKU] Kayıp-defteri confirm ufku kolu ──────────────────────────────────────────────
//
// NEDEN BU TESTLER VAR: sevk edilen ufuk ürün yolunda ölçüldü ve HİÇ ateşlemiyordu
// (3 koşu, ~13k tick, confirmed_total = 0) → loss_ppm yapısal olarak sıfır → kayıp tüketen
// HER mekanizma kör. Kol default KAPALI; bu testler iki şeyi pinler: (a) default gerçekten
// sevk davranışı, (b) açık kolda ufuk ÖLÇÜLEN srtt ile ORANTILI, sabit bir prior'dan değil.

/// `drive` ile aynı akış, ama sender dışarıdan verilir (kol kurulumu sürüşten ÖNCE olmalı).
fn drive_sender(mut s: Sender, units: u64, acked: Vec<u64>) -> Sender {
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
    for t in 1..=20u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        45_000 * MS,
    );
    for t in 221..=320u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * 200 * MS);
    }
    s
}

fn ufuk_ns(k: Option<u32>, acked: Vec<u64>) -> u64 {
    let mut s = Sender::new();
    s.set_establish_probe(false);
    if let Some(k) = k {
        s.set_loss_confirm_horizon(LossConfirmHorizon::ChannelDerived { k_permille: k });
    }
    drive_sender(s, 80, acked).diag_confirm_path().2
}

/// Default sevk davranışıdır. Bir "ölçüm kolu" sessizce varsayılan olursa kolun kendisi
/// ölçülemez hâle gelir.
#[test]
fn onay_ufku_varsayilani_sevk_davranisidir() {
    assert_eq!(
        Sender::new().loss_confirm_horizon(),
        LossConfirmHorizon::CadenceLegacy
    );
}

/// Kanal-türevli ufuk legacy'den FARKLI olmalı — aksi hâlde kol hiçbir şey değiştirmiyor demektir
/// (vakum kol). Legacy ufuk cadence-tabandan gelir ve içinde kanaldan BAĞIMSIZ
/// `EAI_SACK_WIRE_BYTES / EAI_RETURN_BW_PRIOR` = 65 B / 31 B·s⁻¹ ≈ 2.097 s sabiti vardır.
#[test]
fn kanal_turevli_ufuk_legacyden_farklidir() {
    let acked: Vec<u64> = (1..400).filter(|q| q % 2 == 0).collect();
    let legacy = ufuk_ns(None, acked.clone());
    let kanal = ufuk_ns(Some(2000), acked);
    assert!(legacy > 0 && kanal > 0, "iki kolda da tick koşmalı");
    assert_ne!(legacy, kanal);
}

/// Ufuk srtt ile ORANTILI: k iki katına çıkınca ufuk da iki katına çıkar. Bu, ufkun gerçekten
/// ÖLÇÜLEN kanaldan türediğinin testidir — sabit bir prior k'dan etkilenmezdi.
#[test]
fn kanal_turevli_ufuk_k_ile_olceklenir() {
    let acked: Vec<u64> = (1..400).filter(|q| q % 2 == 0).collect();
    let k1 = ufuk_ns(Some(1000), acked.clone());
    let k2 = ufuk_ns(Some(2000), acked.clone());
    let k4 = ufuk_ns(Some(4000), acked);
    assert!(k1 > 0, "srtt ölçülmüş olmalı (yoksa legacy'ye düşerdi)");
    assert_eq!(k2, k1 * 2, "ufuk k ile lineer ölçeklenmeli");
    assert_eq!(k4, k1 * 4);
}
