//! [S1 TEZ-DE-RISK] FEC-recovery kompozisyon hücresi — deterministik, in-process, netem YOK.
//!
//! İki mekanizma yarısı AYRI AYRI kanıtlı: (1) Q18 yapı-gate'i IID yüksek-kayıpta FEC'i 500‰'e açar
//! (`adaptive-transport/tests/structure_gate_fec.rs`); (2) RS codec 8/8 bloğunda 4 kaybı 4 repair'le
//! kurtarır (`kama-fec-codec/tests/roundtrip.rs`). Kanıtlanmayan = KOMPOZİSYON: gerçek Sender'ın
//! gate→latch zinciri, gerçek codec'le, ARQ'nun sansürlediği bir erasure'ı gerçekten kurtarıyor mu?
//! Bu hücre onu ölçer — tez-de-risk'in ilk sistem-düzeyi kanıtı (S1 GATE).
//!
//! ARQ-sansürü modeli: bir burst-erasure'da bir birimin TÜM kopyaları (orijinal + retransmit) aynı
//! patlamada düşer → ARQ o birimi asla kurtaramaz (retransmit de kaybolur). İleri-parity kazanır:
//! aynı erasure'da yeterli repair varsa reconstruct eder. Hücre bu ZITLIĞI pinler.

use adaptive_transport::sender::{
    compute_requested_r, AckInfo, Event, FecActuationMode, OpaqueUnit, Sender, Timer,
};
use kama_fec_codec::{by_id, FecCodec, FecError, ReceivedSymbol, RsSimdCodec, SourceSymbol};
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

/// Deterministik 8-source blok (roundtrip.rs deseni): her shard esi ile damgalı, E=64 byte.
fn block_sources(k: usize, e: usize) -> Vec<Vec<u8>> {
    (0..k)
        .map(|i| {
            (0..e)
                .map(|j| (i as u8).wrapping_mul(31).wrapping_add(j as u8))
                .collect()
        })
        .collect()
}

/// IID yüksek-kayıp akışını gerçek Sender'dan geçirir (structure_gate_fec omurgası): converge →
/// seyrek-ACK ile boşluklar kayıp ilan edilir → timer pompası kayıpları fate-penceresine finalize
/// eder ve her event'te Q18 gate koşar. Gate ateşleyince (sayaç artınca) DURUR ve o anki gerçek
/// controller-snapshot'ı döndürür.
fn drive_until_gate_fires(
    s: &mut Sender,
) -> adaptive_transport::sender::ControllerDecisionSnapshot {
    feed(s, 80);
    for t in 1..=20u64 {
        pto(s, t * 200);
    }
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        }),
        4_100 * MS,
    );
    for t in 21..=220u64 {
        pto(s, t * 200);
    }
    // ~%50 IID: alternating S/L → korelasyonsuz → burst_delta ≤ 0 → gate IID okur.
    let acked: Vec<u64> = (1..400u64).filter(|q| q % 2 == 0).collect();
    let t_ack = 45_000u64;
    s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: Duration::ZERO,
        }),
        t_ack * MS,
    );
    let before = s.diag_structure_gate_opened();
    for i in 1..=120u64 {
        pto(s, t_ack + i * 200);
        if s.diag_structure_gate_opened() > before {
            return s
                .controller_decision_snapshot()
                .expect("fuzzy ran → snapshot present");
        }
    }
    panic!(
        "Q18 gate did not fire on the IID high-loss flow; got {:?}",
        s.diag_loss_structure()
    );
}

/// KOMPOZİSYON: gerçek gate → gerçek latch → gerçek codec, ARQ'nun sansürlediği burst-erasure'ı
/// kurtarır. Gate'in IID kararı (ratio) K=8 bloğunu latch'ler; planlanan r repair, 4-kayıplı
/// erasure'ı reconstruct eder; aynı erasure repair'siz (ARQ-görünümü) Insufficient kalır.
#[test]
fn structure_gated_fec_recovers_burst_erasure_that_arq_censors() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let snap = drive_until_gate_fires(&mut s);

    // Gate gerçekten FEC açtı: snapshot fuzzy'nin rate-only 0'ını değil yapı-gate'in fec_med'ini taşır.
    assert_eq!(
        snap.ratio_permille, 500,
        "the structure gate must have fed fec_med (500‰) into the controller snapshot"
    );
    assert!(
        !snap.no_rule,
        "a high-loss flow must fire fuzzy rules (not the zero-weight fallback)"
    );

    // Gerçek latch: K_actual=8, R_max=4, block, parity_size=E=64. Emergency-interlock varsa planned=0
    // olur ve burada GÖRÜNÜR (gizlenmez — gerçek etkileşim).
    let auth = s.open_fec_block_authority();
    let latch = s.fec_controller_latch(auth, Some(snap), 8, 4, 1, 64);
    let (want_computed, _) = compute_requested_r(500, 8, 4);
    assert_eq!(want_computed, 4, "500‰ × K8 → r=4 (pre-clamp)");
    assert_eq!(
        latch.planned_r, 4,
        "the gated decision must plan the full r=4 for a K=8 block; suppression={:?}",
        latch.suppression
    );

    // ── Gerçek codec: bloğun kaynaklarını RS-encode et (transport'un OPAK taşıdığı execution). ──
    let p = by_id(0x03).unwrap(); // K=8, R=4, E=64
    let data = block_sources(8, 64);
    let src: Vec<SourceSymbol> = data
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect();
    let repairs = RsSimdCodec.encode(&p, &src).unwrap();
    assert_eq!(repairs.len(), 4, "R_code_max repair üretildi");

    // Burst-erasure: bitişik esi 2,3,4,5 düşer (bir patlama). ARQ-görünümü = yalnız kalan 4 source.
    let survived_source: Vec<ReceivedSymbol> = data
        .iter()
        .enumerate()
        .filter(|(i, _)| ![2usize, 3, 4, 5].contains(i))
        .map(|(i, b)| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: b,
        })
        .collect();
    assert_eq!(survived_source.len(), 4);

    // (a) ARQ-görünümü (repair yok): 4 < K=8 → Insufficient. Birimler SANSÜRLÜ (burst'te retx de düştü).
    assert!(
        matches!(
            RsSimdCodec.reconstruct(&p, &survived_source, 0),
            Err(FecError::Insufficient { .. })
        ),
        "without FEC repair, a 4-of-8 burst erasure is unrecoverable — ARQ censors it"
    );

    // (b) FEC-görünümü: kalan 4 source + latch'in planladığı planned_r repair → ≥K → reconstruct.
    let mut with_fec = survived_source.clone();
    for r in repairs.iter().take(latch.planned_r as usize) {
        with_fec.push(ReceivedSymbol {
            esi: r.esi,
            is_repair: true,
            bytes: &r.bytes,
        });
    }
    let recovered = RsSimdCodec
        .reconstruct(&p, &with_fec, 0)
        .expect("survived 4 source + 4 repair ≥ K=8 → decodable");
    assert_eq!(
        recovered.len(),
        4,
        "exactly the 4 censored source units are recovered"
    );
    for esi in [2u16, 3, 4, 5] {
        let got = recovered
            .iter()
            .find(|r| r.esi == esi)
            .expect("censored esi recovered");
        assert_eq!(
            got.bytes, data[esi as usize],
            "recovered unit {esi} must be byte-identical to the original"
        );
    }
}
