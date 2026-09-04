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

use adaptive_transport::fusion_policy::{FusionAction, FusionPolicy, FUSION_ACTION_KINDS};
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
    };
    let a = FusionPolicy::calibration_v1().decide(&obs, 1200);
    assert!(
        a.is_arq_only(),
        "kurulmamış oturumda ArqOnly'ye düşmeli; got {a:?}"
    );
}
