//! FS-5A (DR-FEC-FS-1) — PATH-TIMING OBSERVABILITY PLUMBING kanıtı.
//!
//! İDDİA-DÜZELTMESİ: "RTT gözlemlenemiyor → observability-impossible" YANLIŞ. Engine ([`rtt`]) srtt +
//! min_rtt(rtprop) + rttvar + queue_delay'i ZATEN ölçüyor; `FusionInputs` yalnız-ORAN taşıdığından mutlak-
//! magnitude FEC-selector'a ulaşmıyordu = **PLUMBING_GAP**. `path_timing_snapshot()` bunu salt-forward eder
//! (yeni-ölçüm YOK; production-policy/FusionInputs/wire DEĞİŞMEZ).
//!
//! Bu test FS-5A'yı kanıtlar (state MEVCUT + artık FEC-selector'a görünür). FS-5B (bu-state FEC-beneficial-
//! region'ı GÜVENİLİR-ayırıyor-mu) AYRI + confirmatory-run'larla gösterilir — burada VARSAYILMAZ.

use adaptive_transport::rtt::RttEstimator;
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender};
use std::time::Duration;

const MS: u64 = 1_000_000;

fn ms(d: Duration) -> u64 {
    d.as_millis() as u64
}

/// CASE-A vs CASE-B (Abkarada): AYNI srtt-bucket, FARKLI rejim. Engine srtt-DIŞINDA min_rtt+queue_delay
/// ile AYIRIR → srtt-only-ambiguity engine'de ÇÖZÜLÜ (yalnız FusionInputs'a taşınmıyordu).
#[test]
fn fs5a_case_a_vs_b_engine_discriminates_at_equal_srtt() {
    // CASE-A: base/min RTT ≈ 220ms, srtt ≈ 240ms, queue-delay DÜŞÜK.
    let mut a = RttEstimator::new();
    a.set_qd_window(Duration::from_millis(500)); // ayakta-kuyruk kısa-penceresi
    a.observe(0, Duration::from_millis(220)); // base
    let mut t = 100u64;
    for _ in 0..25 {
        a.observe(t * MS, Duration::from_millis(245));
        t += 100;
    }

    // CASE-B: base/min RTT ≈ 30ms, srtt ≈ 240ms, queue-delay YÜKSEK (kalıcı-kuyruk).
    let mut b = RttEstimator::new();
    b.set_qd_window(Duration::from_millis(500));
    b.observe(0, Duration::from_millis(30)); // düşük base (bir kez; rtprop 10s-pencerede yaşar)
    let mut t = 100u64;
    for _ in 0..25 {
        b.observe(t * MS, Duration::from_millis(245)); // ayakta-kuyruk → yüksek anlık-rtt
        t += 100;
    }

    let (srtt_a, min_a, qd_a) = (
        ms(a.srtt().unwrap()),
        ms(a.rtprop().unwrap()),
        ms(a.queue_delay()),
    );
    let (srtt_b, min_b, qd_b) = (
        ms(b.srtt().unwrap()),
        ms(b.rtprop().unwrap()),
        ms(b.queue_delay()),
    );

    // (1) srtt AYNI-bucket (ambiguity): ikisi de yüksek + birbirine-yakın → srtt-only AYIRAMAZ.
    assert!(
        srtt_a > 200 && srtt_b > 200,
        "srtt A={srtt_a} B={srtt_b} (ikisi yüksek beklenir)"
    );
    assert!(
        srtt_a.abs_diff(srtt_b) < 30,
        "srtt A={srtt_a} B={srtt_b} yakın-olmalı (srtt-only ambiguous)"
    );

    // (2) min_rtt/base DECISIVE ayrım: A≈220 (yüksek-base), B≈30 (düşük-base).
    assert!(min_a > 150, "A base-RTT yüksek olmalı: {min_a}ms");
    assert!(min_b < 80, "B base-RTT düşük olmalı: {min_b}ms");
    assert!(
        min_a.abs_diff(min_b) > 120,
        "min_rtt A={min_a} B={min_b} AYRIŞMALI"
    );

    // (3) queue_delay DECISIVE ayrım: A düşük-kuyruk, B yüksek-kuyruk.
    assert!(qd_a < 80, "A queue-delay düşük olmalı: {qd_a}ms");
    assert!(qd_b > 150, "B queue-delay yüksek olmalı: {qd_b}ms");

    // SONUÇ: aynı-srtt'de engine (min_rtt+queue_delay) A(yüksek-base/az-kuyruk) ile B(düşük-base/çok-kuyruk)'yi
    // AYIRIR. Bilgi engine'de MEVCUT; FusionInputs-oranları bunu düşürüyordu → PLUMBING_GAP (mimari-imkânsızlık-DEĞİL).
}

/// PLUMBING: `path_timing_snapshot()` engine-state'i FEC-selector'a MUTLAK-değer olarak forward eder.
#[test]
fn fs5a_sender_snapshot_forwards_absolute_path_timing() {
    let mut s = Sender::new();
    // RTT-sample-öncesi: confidence-yok (sample_present=false; srtt/min_rtt None).
    let pre = s.path_timing_snapshot();
    assert!(!pre.sample_present, "sample-öncesi confidence-yok");
    assert!(
        pre.srtt_ns.is_none() && pre.min_rtt_ns.is_none(),
        "sample-öncesi srtt/min_rtt None"
    );

    // Bir DATA + ACK → gerçek RTT-sample (send@0 → ack@200ms).
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 1200 }), 0);
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        }),
        200 * MS,
    );

    let snap = s.path_timing_snapshot();
    assert!(
        snap.sample_present,
        "ACK-sonrası RTT-sample işlenmeli (confidence)"
    );
    assert!(snap.srtt_ns.is_some(), "srtt mutlak-değer forward edilmeli");
    assert!(
        snap.min_rtt_ns.is_some(),
        "min_rtt/rtprop mutlak-değer forward edilmeli"
    );
    // Gerçek-mutlak (oran-DEĞİL): srtt makul RTT-mertebesinde.
    let srtt_ms = snap.srtt_ns.unwrap() / MS;
    assert!(
        (10..=400).contains(&srtt_ms),
        "srtt gerçek-mutlak-ns forward: {srtt_ms}ms"
    );
}
