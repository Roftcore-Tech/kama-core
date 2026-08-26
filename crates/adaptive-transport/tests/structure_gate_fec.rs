//! [Q18-FIX] Yapı-farkında FEC gate — Sender-düzeyi regresyon pini.
//!
//! Q18 (`q18_check.rs`) fuzzy controller'ın rate-only çıktısını pinler: %20 kayıpta parity=0 — ve
//! o pin GEÇERLİ KALIR (fuzzy'ye dokunulmadı, diz kaydırılmadı; Q17 bunu yasaklar). Fix Sender'da:
//! `structure_gate_fec`, fate_estimator'ın loss-YAPI sinyalini tüketir. Gate iki kanıt ister:
//! TAZE kayıp (ledger `loss_windowed_ppm` ≥ Q18 bandı — trafik durunca doğal decay, bayat-FEC yok)
//! ve YAPI (fate-penceresi `burst_delta`, ≥32 fate-known). Bu dosya ürün-yolundaki sonucu pinler:
//! aynı kayıp ORANINDA IID yapı FEC'i AÇAR, burst yapı ARQ'ya bırakır, düşük kayıp byte-özdeş kalır,
//! ve kayıp bitince gate kapanır.
use adaptive_transport::fusion::FusionDecision;
use adaptive_transport::sender::*;
use std::time::Duration;

const MS: u64 = 1_000_000;

fn feed(s: &mut Sender, units: u64) {
    for i in 0..units {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1000 + i,
                size: 101,
            }),
            0,
        );
    }
}

fn pto(s: &mut Sender, t_ms: u64) {
    s.on_event(Event::TimerFired(Timer::Pto), t_ms * MS);
}

/// Ürün-şekilli sürüş (`loss_structure_observation.rs` omurgası): emit → seq-0 erken-ACK ile
/// converge → `acked` seq'leri ACK'le (ACK-ufku altındaki boşluklar kayıp ilan edilir). Kuyruk
/// pompası ÇAĞIRANA bırakılır: gate'in ateşlediği anı yakalamak ve sonra decay'i izlemek için.
/// Döner: (sender, son-ACK zamanı ms).
///
/// Wire-seq ≠ unit-id: seq her emisyonda (retransmit dahil) artar. ACK deseni bu yüzden geniş bir
/// seq aralığına (`SEQ_SPAN`) uygulanır; emit edilmemiş seq'ler ACK'te yok sayılır. Aksi hâlde
/// desenin dışında kalan son emisyonlar PTO ile bitişik "kuyruk-burst"e dönüşür (ilk sürümün
/// düşük-kayıp hücresinde 12/72 = 166k ppm ölçen artefakt).
const SEQ_SPAN: u64 = 400;
fn drive_until_loss_ack(units: u64, acked: Vec<u64>) -> (Sender, u64) {
    let mut s = Sender::new();
    feed(&mut s, units);
    for t in 1..=20u64 {
        pto(&mut s, t * 200);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    // Converge sonrası TÜM birimler emit edilene kadar pompala (PreConverge-taban pacing yavaş:
    // 17 s'de ~63/80 çıkıyordu; kalan 9'u ACK-sonrası emit olup PTO ile bitişik "kuyruk-burst"e
    // dönüşüyordu — ilk sürümün düşük-kayıp hücresinde 12/72 = 166k ppm ölçen artefakt).
    for t in 21..=220u64 {
        pto(&mut s, t * 200);
    }
    let t_ack = 45_000;
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        t_ack * MS,
    );
    (s, t_ack)
}

/// Kayıp-ACK'inden sonra timer pompala; gate'in İLK ateşlediği event'te (sayaç artınca) o anki
/// kararı döndür. Ateşlemezse `None`. Fate-finalize guard'ı (≈base_pto) bu pencerede geçer.
fn pump_until_gate_opens(s: &mut Sender, t_ack: u64, steps: u64) -> Option<FusionDecision> {
    let before = s.diag_structure_gate_opened();
    for i in 1..=steps {
        pto(s, t_ack + i * 200);
        if s.diag_structure_gate_opened() > before {
            return Some(s.last_decision());
        }
    }
    None
}

/// IID @ ~%50: her ikinci seq ACK'lenir → S L S L … → n11≈0 → p11 ≪ p01 → burst_delta belirgin
/// NEGATİF → gate IID okur → fuzzy'nin fec=0'ı `fec_med`(500)'e açılır. Sonra trafik yok → ledger
/// decay → gate KAPANIR (bayat-FEC yok; yapı-penceresi kalsa bile taze-kayıp şartı düşer).
#[test]
fn iid_loss_opens_fec_through_structure_gate_then_closes_on_decay() {
    let acked: Vec<u64> = (1..SEQ_SPAN).filter(|q| q % 2 == 0).collect();
    let (mut s, t_ack) = drive_until_loss_ack(80, acked);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(
        f.n >= 32,
        "kanıt-eşiği: pencerede ≥32 fate-known birim gerekir; got n={} ({f:?})",
        f.n
    );
    assert!(
        f.burst_delta_ppm <= 50_000,
        "alternating S/L must read as IID (burst_delta ≤ 50k); got {} ({f:?})",
        f.burst_delta_ppm
    );
    let d = fired.expect("Q18-FIX: the structure gate must open FEC on an IID high-loss flow");
    assert_eq!(
        d.fec_parity_ratio_permille, 500,
        "IID @ high loss → decision carries fec_med (500‰), not fuzzy's rate-only 0"
    );

    // Decay: uzun sessizlik → ledger boşalır → taze-kayıp şartı düşer → gate artık açmaz.
    // Ledger penceresi = ack_epoch×K_WL(8); bu sürüşte srtt onlarca saniye (seq-0 ACK'i 4.1 s'de,
    // kayıp-ACK'i 45 s'de) → pencere dakikalar. Sim-ucuz: ~15 dk sessizlik pompala.
    let opened_at_end_of_episode = s.diag_structure_gate_opened();
    for i in 61..=4_500u64 {
        pto(&mut s, t_ack + i * 200);
    }
    let snap = s.diag_fusion_input_snapshot().expect("fuzzy ran");
    assert!(
        snap.loss_windowed_ppm < 100_000,
        "precondition: ledger must have decayed; got {}",
        snap.loss_windowed_ppm
    );
    assert_eq!(
        s.last_decision().fec_parity_ratio_permille,
        0,
        "no stale FEC: once fresh loss decays the gate must close (structure window persisting \
         is not enough on its own)"
    );
    // Ve decay boyunca bir kez daha açılmamış olmalı (ledger < band).
    assert!(
        s.diag_structure_gate_opened() >= opened_at_end_of_episode,
        "counter is monotone"
    );
}

/// Burst: ilk yarı tamamen ACK, ikinci yarı bitişik kayıp → LLLL… → n11 yüksek → p11 ≫ p01 →
/// burst_delta belirgin POZİTİF → gate burst okur → DOKUNMAZ (ARQ'ya bırakır): fec kalır 0.
#[test]
fn burst_loss_leaves_fec_to_arq() {
    let acked: Vec<u64> = (1..40u64).collect();
    let (mut s, t_ack) = drive_until_loss_ack(80, acked);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(f.n >= 32, "kanıt-eşiği; got n={} ({f:?})", f.n);
    assert!(
        f.burst_delta_ppm > 50_000,
        "contiguous loss run must read as BURST (burst_delta > 50k); got {} ({f:?})",
        f.burst_delta_ppm
    );
    assert!(
        fired.is_none(),
        "burst structure must NOT open FEC (ARQ handles correlated loss; Q17 harmful-FP guard)"
    );
    assert_eq!(s.diag_structure_gate_opened(), 0);
    assert_eq!(s.last_decision().fec_parity_ratio_permille, 0);
}

/// Düşük kayıp (Q18 bandı altı, ~%5): gate hiç devreye girmez → fuzzy çıktısı byte-özdeş (fec=0).
#[test]
fn low_loss_is_untouched_by_the_gate() {
    let acked: Vec<u64> = (1..SEQ_SPAN).filter(|q| q % 20 != 0).collect();
    let (mut s, t_ack) = drive_until_loss_ack(80, acked);
    let fired = pump_until_gate_opens(&mut s, t_ack, 60);
    let f = s.diag_loss_structure();
    assert!(
        f.measured_loss_ppm < 100_000,
        "precondition: fate-window loss below the Q18 band; got {} ({f:?})",
        f.measured_loss_ppm
    );
    assert!(
        fired.is_none(),
        "gate must never open below the Q18 loss band"
    );
    assert_eq!(s.diag_structure_gate_opened(), 0);
    assert_eq!(s.last_decision().fec_parity_ratio_permille, 0);
}
