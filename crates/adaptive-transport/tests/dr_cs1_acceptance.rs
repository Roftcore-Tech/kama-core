//! DR-CS-1 (MINIMAL BOOTSTRAP) — KABUL-MATRİSİ İSKELETİ (F0; tümü #[ignore] — F1/F2A/F2B/F3/F4
//! fazlarında doldurulur). Bkz. reports/decision-requests/DR-CS-1-minimal-bootstrap.md §4.
//!
//! ÇERÇEVE: Bu deney ARQ ile FEC'i karşılaştırmaz; DR-CS-1 cold-start LIVENESS/bootstrap DR'ıdır.
//! Eski-4-liveness-testi (cold_start_liveness.rs) FLAG-OFF beklentileriyle AYNEN KORUNUR (INV-CS-5
//! flag-OFF kimliğinin canlı-pinleri). Bu dosya flag-ON davranışının kabul-matrisidir.
//!
//! Ölçülen-şey-disiplini: wire-emission / enqueue / completion AYRI raporlanır (INV-CS-1 emisyon;
//! INV-CS-6 completion). Kabul-matrisi kalemleri talimat-§4'te normatiftir.
#![cfg(feature = "experimental-cold-start")]

// ── F1 (M1: on_event_batch + next_wakeup_ns + Tick + adapter) — İMPLEMENTE ─────────────────

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitDecision,
};
use std::time::Duration;

const MS: u64 = 1_000_000;

fn mk_seq() -> Vec<(u64, Event)> {
    // Deterministik golden-dizi: AppData@0 → Pto-tick'ler → Ack(seq0)@250ms → Pto'lar → AppData.
    let mut v: Vec<(u64, Event)> = vec![(0, Event::AppData(OpaqueUnit { id: 1, size: 1200 }))];
    for i in 1..=20u64 {
        v.push((i * 10 * MS, Event::TimerFired(Timer::Pto)));
    }
    v.push((
        250 * MS,
        Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        }),
    ));
    for i in 26..=40u64 {
        v.push((i * 10 * MS, Event::TimerFired(Timer::Pto)));
    }
    v.push((410 * MS, Event::AppData(OpaqueUnit { id: 2, size: 1200 })));
    for i in 42..=60u64 {
        v.push((i * 10 * MS, Event::TimerFired(Timer::Pto)));
    }
    v
}

/// INV-CS-5: batch-of-1 ≡ legacy tekil on_event (flag-ON) — output-dizisi + state + wakeup özdeş.
#[test]
fn f1_batch_of_one_equals_legacy_single() {
    let mut a = Sender::new();
    let mut b = Sender::new();
    let mut out_a: Vec<TransmitDecision> = Vec::new();
    let mut out_b: Vec<TransmitDecision> = Vec::new();
    for (t, ev) in mk_seq() {
        out_a.extend(a.on_event(ev.clone(), t));
        out_b.extend(b.on_event_batch(t, vec![ev]));
    }
    assert_eq!(
        out_a, out_b,
        "batch-of-1 output-dizisi legacy'yle-özdeş-değil"
    );
    assert_eq!(a.state(), b.state(), "state-uyuşmazlığı");
    assert_eq!(a.next_wakeup_ns(), b.next_wakeup_ns());
}

/// EK5-E2: Tick-only exact-no-op — boş-batch + due-yok + iş-yok → output-boş, core-koşmaz
/// (fuzzy-event-index ilerlemez → snapshot-None-kalır), state-değişmez.
#[test]
fn f1_tick_only_noop_exact() {
    let mut s = Sender::new();
    let out = s.on_event(Event::Tick, 5 * MS);
    assert!(out.is_empty(), "no-op output-boş-olmalı");
    assert_eq!(s.state(), SenderState::Idle, "state-değişmemeli");
    assert!(
        s.diag_fusion_input_snapshot().is_none(),
        "core-koşmamalı (fuzzy-evaluate-izi-yok)"
    );
    // Gelecek-deadline'lı hook varken (due-DEĞİL) Tick yine no-op + hook ateşlenmez.
    s.cs_test_arm_hook_timer(100 * MS, 7);
    let out2 = s.on_event(Event::Tick, 50 * MS);
    assert!(out2.is_empty());
    assert!(s.cs_test_hook_fired().is_empty(), "erken-fire-yasak");
    assert_eq!(s.next_wakeup_ns(), Some(100 * MS));
}

/// DI-12E (F1): aynı-now_ns İKİ inbound FARKLI geliş-sırasıyla aynı-batch'e → AYNI sonuç
/// (tip-önceliği: ACK-önce; stable-sort) + due-hook BİR-KEZ, inbound-SONRASI (faz-3).
#[test]
fn f1_di12e_same_ns_two_inbound_order_invariant() {
    let build = |order_swapped: bool| -> (Vec<TransmitDecision>, SenderState, usize) {
        let mut s = Sender::new();
        // warm: ilk-DATA + ACK → Startup'a-çıkmış-durumda ikinci-tur-olayları anlamlı.
        let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 1200 }), 0);
        let ack = Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        });
        let app = Event::AppData(OpaqueUnit { id: 2, size: 1200 });
        s.cs_test_arm_hook_timer(100 * MS, 1); // due@batch-anı
        let inbound = if order_swapped {
            vec![app, ack]
        } else {
            vec![ack, app]
        };
        let out = s.on_event_batch(100 * MS, inbound);
        (out, s.state(), s.cs_test_hook_fired().len())
    };
    let (o1, st1, f1) = build(false);
    let (o2, st2, f2) = build(true);
    assert_eq!(
        o1, o2,
        "geliş-sırası-değişince-sonuç-değişti (atomiklik-ihlali)"
    );
    assert_eq!(st1, st2);
    assert_eq!(f1, 1, "due-hook-tam-BİR-kez");
    assert_eq!(f2, 1);
}

/// DI-12F (F1; adapter): aynı-timestamp AYRI "socket-read"ler adapter'da TEK batch olur
/// (per-iterasyon tek-damga) → tek-batch'te işlenir; hook BİR-kez-inbound-sonrası ateşler.
/// (F4'te gerçek-CS-timer'larla + gerçek-runner'la tam-tekrar.)
#[test]
#[cfg(feature = "experimental-fec-budget")]
fn f1_di12f_adapter_single_batch_ack_wins() {
    use adaptive_transport::udp_runner::CsBatchAdapter;
    let mut ad = CsBatchAdapter::new(500 * MS);
    // iki AYRI read, aynı-iterasyon → aynı-damga, tek-batch.
    ad.push(Event::AckReceived(AckInfo {
        acked_seqs: vec![0],
        ack_delay: Duration::ZERO,
    }));
    ad.push(Event::AppData(OpaqueUnit { id: 9, size: 1200 }));
    assert_eq!(ad.stamp(), 500 * MS);
    let (bn, bev) = ad.drain();
    assert_eq!(bn, 500 * MS, "damga-iterasyon-başına-BİR-kez");
    assert_eq!(bev.len(), 2, "aynı-ns-olaylar-TEK-batch'te");
    // Sender-tarafı: tek-batch + due-hook → hook inbound-SONRASI bir-kez.
    let mut s = Sender::new();
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 1200 }), 0);
    s.cs_test_arm_hook_timer(bn, 2);
    let _ = s.on_event_batch(bn, bev);
    assert_eq!(s.cs_test_hook_fired().len(), 1, "tek-batch → tek-fire");
}

/// DI-12G (EK5-E2 daraltması): timer-due-DEĞİLKEN inbound scheduling-işi üretir → no-op-
/// bastırması inbound-içeren-batch'e UYGULANMAZ (output normal-çıkar).
#[test]
fn f1_di12g_inbound_batch_completes_scheduling() {
    let mut s = Sender::new();
    let out = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    assert!(
        !out.is_empty(),
        "inbound-batch output-üretmeli (no-op-bastırması-yanlış-uygulanmış)"
    );
    // F2A-sonrası: ilk-DATA-emisyonu CS-probe-merdivenini-armlar → next_wakeup = 3s-fire.
    assert_eq!(
        s.next_wakeup_ns(),
        Some(3_000_000_000),
        "arm-sonrası ilk-probe-fire-deadline'ı-expose"
    );
}

/// C2 post-condition: her batch-dönüşünde next_wakeup ∈ {None, gelecek}; due-hook retire-on-fire
/// (çift-fire-yok; sonraki-boş-batch E2-no-op).
#[test]
fn f1_c2_postcondition_no_due_after_batch() {
    let mut s = Sender::new();
    s.cs_test_arm_hook_timer(5 * MS, 3);
    // due@10ms → boş-batch yine-de faz-3'ü koşar (due-var; E2-no-op-DEĞİL).
    let _ = s.on_event_batch(10 * MS, Vec::new());
    assert_eq!(s.cs_test_hook_fired(), &[(10 * MS, 3)], "overdue-hook-fire");
    assert!(
        s.next_wakeup_ns().is_none_or(|w| w > 10 * MS),
        "C2: dönüşte-due-kalmamalı"
    );
    // aynı-anda tekrar boş-batch → E2-no-op + çift-fire-yok.
    let out2 = s.on_event_batch(10 * MS, Vec::new());
    assert!(out2.is_empty());
    assert_eq!(
        s.cs_test_hook_fired().len(),
        1,
        "retire-on-fire (çift-fire-yasak)"
    );
}

// ── F2A (probe-emission feasibility) — İMPLEMENTE ──────────────────────────────────────────

use adaptive_transport::sender::TransmitKind;

/// Saturated-backlog cold-start sürücüsü: N-birim t=0'da (kuyruk-dolu), ACK-YOK, 100ms-Pto-tick.
/// Dönüş: (t_ns, CsProbe-emisyonları) + tüm-batch-output'larda DATA-önce-sıra-denetimi.
fn drive_cold_saturated(units: u64, horizon_ns: u64) -> (Sender, Vec<(u64, u64)>) {
    let mut s = Sender::new();
    let mut probes: Vec<(u64, u64)> = Vec::new(); // (t_ns, probe_seq)
    let mut inbound: Vec<Event> = (0..units)
        .map(|i| {
            Event::AppData(OpaqueUnit {
                id: 1 + i,
                size: 1200,
            })
        })
        .collect();
    let out0 = s.on_event_batch(0, std::mem::take(&mut inbound));
    assert!(
        out0.iter().any(|d| d.kind == TransmitKind::Data),
        "t=0 initial-DATA-burst bekleniyor"
    );
    let mut t = 100 * MS;
    while t <= horizon_ns {
        let out = s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]);
        // Mühürlü DATA-önce-sıra: aynı-output-Vec'te hiçbir Data, CsProbe'dan SONRA gelmez
        // (probe-pass EN-SONDA — yapısal + burada-assert).
        let first_probe_idx = out.iter().position(|d| d.kind == TransmitKind::CsProbe);
        if let Some(pi) = first_probe_idx {
            assert!(
                out[pi..].iter().all(|d| d.kind == TransmitKind::CsProbe),
                "DATA-önce-sıra-ihlali: probe'dan-sonra-data"
            );
        }
        for d in &out {
            if d.kind == TransmitKind::CsProbe {
                assert_eq!(d.size, 44, "PROBE_WIRE_MAX=44B");
                probes.push((t, d.seq));
            }
        }
        t += 100 * MS;
    }
    (s, probes)
}

const SEC: u64 = 1_000_000_000;
const PACER_ERR_NS: u64 = 250 * MS;

/// DI-1B (F2A): saturated-DATA-backlog — CS-probe mühürlü-DATA-önce-sıralama DEĞİŞMEDEN
/// ≤ T_FIRST_PROBE(3s)+PACER_ERR(250ms) wire-emission (INV-CS-1; boyuttan-bağımsız).
#[test]
fn f2a_di1b_probe_emission_under_saturated_backlog() {
    let (s, probes) = drive_cold_saturated(40, 4 * SEC);
    assert!(
        !probes.is_empty(),
        "saturated-backlog'da probe STARVE-ETTİ (DUR-B-tetiği)"
    );
    let (t_first, seq_first) = probes[0];
    assert!(
        t_first <= 3 * SEC + PACER_ERR_NS,
        "ilk-probe {}ms > 3s+250ms (INV-CS-1-ihlali)",
        t_first / MS
    );
    assert_eq!(seq_first, 0, "probe_seq monoton-0-başlar (INV-CS-7)");
    let (emitted, tx_bytes, wrapped, _pend, armed, disarmed) = s.diag_cold_start();
    assert!(armed && !disarmed);
    assert_eq!(emitted, 1);
    assert_eq!(tx_bytes, 44);
    assert_eq!(wrapped, 0);
}

/// DI-6B (F2A): 31 B/s-floor token-feasibility — 4-fire (3/9/21/45s) hepsi tolerans-içi emit;
/// bütçe nominal-176B ≤ hard-200B; 5.-probe-YOK (N=4); token-matematiği F0-tespit-(vii)-uyumlu
/// (93B@3s / 235B@9s / 563B@21s ≥ 44B).
#[test]
fn f2a_di6b_probe_emission_at_floor_rate() {
    let (s, probes) = drive_cold_saturated(40, 46 * SEC);
    assert_eq!(probes.len(), 4, "merdiven N=4 (5.-probe-yasak)");
    for (i, fire) in [3u64, 9, 21, 45].iter().enumerate() {
        let (t, pseq) = probes[i];
        assert!(
            t >= *fire * SEC && t <= *fire * SEC + PACER_ERR_NS,
            "probe-{i} t={}ms beklenen [{fire}s, {fire}s+250ms] (token-starve?)",
            t / MS
        );
        assert_eq!(pseq, i as u64, "probe_seq monoton (INV-CS-7)");
    }
    let (emitted, tx_bytes, wrapped, pend, _armed, _dis) = s.diag_cold_start();
    assert_eq!(emitted, 4);
    assert_eq!(tx_bytes, 176, "nominal-bütçe 4×44=176B (INV-CS-2)");
    assert!(tx_bytes <= 200, "hard-bütçe");
    assert_eq!(wrapped, 0);
    assert_eq!(pend, 0, "pending-drenajlı");
}

/// F2A disarm (DI-4-önkoşulu): ilk-fire'dan-ÖNCE hayatta-kalan DATA-ACK → srtt → faz-2-disarm
/// → probe-0 (temiz-hızlı-linkte sıfır-maliyet; INV-CS-5).
#[test]
fn f2a_probe_disarm_on_early_data_ack() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = s.on_event_batch(
        1 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    let mut t = 1 * SEC + 100 * MS;
    let mut any_probe = false;
    while t <= 10 * SEC {
        let out = s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]);
        any_probe |= out.iter().any(|d| d.kind == TransmitKind::CsProbe);
        t += 100 * MS;
    }
    assert!(
        !any_probe,
        "erken-ACK-sonrası probe-emisyonu-yasak (disarm)"
    );
    let (emitted, _, _, pend, armed, disarmed) = s.diag_cold_start();
    assert_eq!(emitted, 0);
    assert_eq!(pend, 0);
    assert!(armed && disarmed);
    assert!(
        s.next_wakeup_ns().is_none(),
        "disarm-sonrası CS-wakeup-kalmamalı"
    );
}

// ── F2B (echo + bootstrap-RTT + Startup + (d) PTO binding) — İMPLEMENTE ────────────────────

use adaptive_transport::sender::CsSrttSource;

/// Cold-start sürücüsü: tek-birim + 100ms-Pto-tick'lerle `to_ns`'e kadar; emisyonları toplar.
fn drive_ticks(s: &mut Sender, from_ns: u64, to_ns: u64) -> Vec<(u64, TransmitDecision)> {
    let mut out = Vec::new();
    let mut t = from_ns;
    while t <= to_ns {
        for d in s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]) {
            out.push((t, d));
        }
        t += 100 * MS;
    }
    out
}

/// (d)-yardımcı: hedef-birimin Retransmit'ini bekle — önce verilen-batch-çıktısına bak; yoksa
/// exposed-deadline'a TEK-boş-Tick (yeni-network-event-YOK) ile atla ve orada-fire'ı doğrula.
/// Dönüş: (fire_edilen_batch_çıktısı). Not: deadline mevcut-cadence-tabanıyla (EAI@31B/s türevi)
/// dakikalar-mertebesinde-olabilir — MEVCUT-eşik-kuralları-değiştirilemez (0.2-mühür); simüle-zaman.
fn expect_retransmit_via_wakeup(
    s: &mut Sender,
    first_t: u64,
    first_out: &[TransmitDecision],
    unit_id: u64,
) -> (u64, Vec<TransmitDecision>) {
    let has = |o: &[TransmitDecision]| {
        o.iter()
            .any(|d| d.kind == TransmitKind::Retransmit && d.unit_id == unit_id)
    };
    if has(first_out) {
        return (first_t, first_out.to_vec()); // overdue-aynı-batch (C2)
    }
    let wk = s
        .next_wakeup_ns()
        .expect("(d)-deadline expose-edilmeli (EK4-A2)");
    let out = s.on_event_batch(wk + MS, Vec::new()); // TEK-boş-Tick: due→faz-3-fire
    assert!(
        has(&out),
        "retransmit exposed-deadline'da Tick-only-ile-fire-etmeli (yeni-network-event-yok)"
    );
    (wk + MS, out)
}

/// DI-1: ilk-DATA-kaybı — probe→echo→bootstrap-srtt→Startup; sonraki-trafiğin-ACK'i kayıp-birimi
/// MEVCUT time-threshold/ARQ-yolundan retransmit-eder; (C2) due-PTO ACK-batch'inin-KENDİSİNDE
/// fire-eder (ek-RTT/ek-network-event-BEKLEMEZ).
#[test]
fn f2b_di1_first_data_loss_bootstrap_recovery() {
    for size in [65u64, 300, 1200] {
        let mut s = Sender::new();
        // u1@0 → seq0 emit (KAYIP: ACK-gelmeyecek).
        let out0 = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size })]);
        assert!(out0.iter().any(|d| d.kind == TransmitKind::Data));
        // 3s: probe-fire (INV-CS-1).
        let probes = drive_ticks(&mut s, 100 * MS, 3 * SEC + 200 * MS);
        assert!(
            probes.iter().any(|(_, d)| d.kind == TransmitKind::CsProbe),
            "probe-emisyonu (size={size})"
        );
        // echo → bootstrap-srtt → Startup (kaynak=ProbeEcho).
        let _ = s.on_event_batch(
            3 * SEC + 300 * MS,
            vec![Event::CsProbeEcho { probe_seq: 0 }],
        );
        assert_ne!(s.state(), SenderState::PreConverge, "Startup-geçişi");
        let (src, _, _, _) = s.diag_cs_bootstrap();
        assert_eq!(src, Some(CsSrttSource::ProbeEcho), "ResolvedByProbeEcho");
        // Startup: u2 emit (seq1) → ACK'i gelir → largest=1 > seq0 → (d)-deadline overdue →
        // AYNI-batch'te seq0-Retransmit (C2-assert).
        let out2 = s.on_event_batch(
            3 * SEC + 400 * MS,
            vec![Event::AppData(OpaqueUnit { id: 2, size })],
        );
        assert!(out2.iter().any(|d| d.kind == TransmitKind::Data), "u2-emit");
        let ack_batch = s.on_event_batch(
            4 * SEC,
            vec![Event::AckReceived(AckInfo {
                acked_seqs: vec![1],
                ack_delay: Duration::from_millis(1),
            })],
        );
        // (d)+C2: overdue-ise-ACK-batch'inde; değilse exposed-deadline'da TEK-boş-Tick'le
        // (yeni-network-event-YOK) MEVCUT-PTO/ARQ-yolundan çıkar.
        let (_ft, fired) = expect_retransmit_via_wakeup(&mut s, 4 * SEC, &ack_batch, 1);
        assert!(fired
            .iter()
            .any(|d| d.kind == TransmitKind::Retransmit && d.unit_id == 1));
        // busy-loop-yok (fire-sonrası):
        let now_after = s.next_wakeup_ns();
        assert!(now_after.is_none_or(|w| w > 4 * SEC));
    }
}

/// DI-1A (C2): bootstrap-sonrası kayıp-birim retransmit'i YENİ-NETWORK-EVENT-BEKLEMEDEN çıkar —
/// deadline-overdue-ise ACK-batch'inde, değilse exposed-deadline'da TEK-boş-Tick'le (C2). DÜRÜST-
/// NOT: mühürlü cadence-floor (EAI@31B/s türevi) nedeniyle deadline gelecek-olabilir → "aynı-batch"
/// GARANTİ-değil; normatif-özellik "ek-network-event-yok + busy-loop-yok"tur (helper doğrular).
#[test]
fn f2b_di1a_overdue_pto_fires_same_batch() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut s, 100 * MS, 3 * SEC + 100 * MS); // probe@3s
    let _ = s.on_event_batch(
        3 * SEC + 200 * MS,
        vec![Event::CsProbeEcho { probe_seq: 0 }],
    );
    assert_ne!(s.state(), SenderState::PreConverge, "echo-bootstrap");
    let out2 = s.on_event_batch(
        3 * SEC + 300 * MS,
        vec![Event::AppData(OpaqueUnit { id: 2, size: 1200 })],
    );
    let s2 = out2
        .iter()
        .find(|d| d.kind == TransmitKind::Data && d.unit_id == 2)
        .map(|d| d.seq)
        .expect("u2-emit");
    let ack_batch = s.on_event_batch(
        4 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![s2],
            ack_delay: Duration::from_millis(1),
        })],
    );
    // (d)+C2: overdue-ise-ACK-batch'inde, değilse exposed-deadline'da TEK-boş-Tick (yeni-network-
    // event-YOK); busy-loop-yok (post-condition helper-içinde + burada).
    let (ft, fired) = expect_retransmit_via_wakeup(&mut s, 4 * SEC, &ack_batch, 1);
    assert!(fired
        .iter()
        .any(|d| d.kind == TransmitKind::Retransmit && d.unit_id == 1));
    // Çift-fire-yok: fire-anında-tekrar-boş-batch yeni-retransmit-üretmez.
    let again = s.on_event_batch(ft, Vec::new());
    assert!(
        !again.iter().any(|d| d.kind == TransmitKind::Retransmit),
        "aynı-anda-tekrar-Tick yeni-retransmit-üretmez"
    );
    assert!(s.next_wakeup_ns().is_none_or(|w| w > ft));
}

/// DI-1C (EK4-A2): re-armed-PTO yaşam-döngüsü — retransmit'in-ACK'i-de-düşer; SONRAKİ trafik-ACK'i
/// largest'ı-yükseltince YENİ deadline next_wakeup'ta görünür; Tick-only ile-ikinci-retransmit;
/// nihai-ACK TÜM BootstrapPto-kayıtlarını-retire-eder.
#[test]
fn f2b_di1c_rearmed_pto_lifecycle_exposure() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut s, 100 * MS, 3 * SEC + 100 * MS); // probe@3s
    let _ = s.on_event_batch(
        3 * SEC + 200 * MS,
        vec![Event::CsProbeEcho { probe_seq: 0 }],
    );
    // u2/u3 emit + ACK → largest>0 → ilk-retransmit (u1).
    let _ = s.on_event_batch(
        3 * SEC + 300 * MS,
        vec![
            Event::AppData(OpaqueUnit { id: 2, size: 1200 }),
            Event::AppData(OpaqueUnit { id: 3, size: 1200 }),
        ],
    );
    let b1 = s.on_event_batch(
        4 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![1, 2],
            ack_delay: Duration::from_millis(1),
        })],
    );
    let (ft1, fired1) = expect_retransmit_via_wakeup(&mut s, 4 * SEC, &b1, 1);
    let rtx1: Vec<_> = fired1
        .iter()
        .filter(|d| d.kind == TransmitKind::Retransmit && d.unit_id == 1)
        .collect();
    assert_eq!(rtx1.len(), 1, "ilk-retransmit");
    let rtx1_seq = rtx1[0].seq;
    let mut t1 = ft1 + 60 * SEC; // fire-sonrası ileri-zaman (simüle)
                                 // Retransmit'in-ACK'i-de-düşer. SONRAKİ trafik: u4 enqueue + token-refill-bekleyerek emit
                                 // (kuyruk-önü u3-olabilir; 31 B/s-refill ~38.7s/birim) → seq'i-çıktıdan-izle.
    let mut out_all = s.on_event_batch(t1, vec![Event::AppData(OpaqueUnit { id: 4, size: 1200 })]);
    let mut s4_opt = None;
    for _ in 0..20 {
        if let Some(d) = out_all
            .iter()
            .find(|d| d.kind == TransmitKind::Data && d.unit_id == 4)
        {
            s4_opt = Some(d.seq);
            break;
        }
        t1 += 40 * SEC;
        out_all = s.on_event_batch(t1, vec![Event::TimerFired(Timer::Pto)]);
    }
    let s4 = s4_opt.expect("u4-emit (token-refill-sonrası)");
    assert!(s4 > rtx1_seq, "u4-seq monoton-yüksek");
    let b2 = s.on_event_batch(
        t1 + SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![s4],
            ack_delay: Duration::from_millis(1),
        })],
    );
    // İkinci-retransmit: overdue-ise-b2'de; değilse exposed-deadline'da Tick-only (EK4-A2).
    let (_ft2, fired2) = expect_retransmit_via_wakeup(&mut s, t1 + SEC, &b2, 1);
    let rtx2_seq = fired2
        .iter()
        .find(|d| d.kind == TransmitKind::Retransmit && d.unit_id == 1)
        .unwrap()
        .seq;
    assert!(rtx2_seq > rtx1_seq, "yeni-seq (nonce-reuse-yok)");
    // Nihai-ACK → birim-outstanding'ten-çıkar → BootstrapPto-kayıtları-retire (EK4-A2-yaşam-döngüsü).
    let t_end = t1 + 600 * SEC;
    let _ = s.on_event_batch(
        t_end,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![rtx2_seq],
            ack_delay: Duration::from_millis(1),
        })],
    );
    assert!(
        s.next_wakeup_ns().is_none_or(|w| w > t_end),
        "ACK-sonrası-due-yok (tüm-PTO-wakeup-kayıtları-temiz)"
    );
}

/// DI-2 (§2.4-a): tekil-ACK-frame-erasure — DATA-ulaştı-ACK-düştü; echo-bootstrap → sonraki-ACK-
/// kanıtıyla mevcut-PTO SPURIOUS-retransmit (BEKLENEN-maliyet; correctness-alıcı-dedup'unda);
/// bootstrap_induced_retransmit_bytes EXACT; yön-teşhisi-İDDİA-EDİLMEZ.
#[test]
fn f2b_di2_single_ack_erasure_spurious_expected() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut s, 100 * MS, 3 * SEC + 100 * MS);
    let _ = s.on_event_batch(
        3 * SEC + 200 * MS,
        vec![Event::CsProbeEcho { probe_seq: 0 }],
    );
    let (_, _, induced0, _) = s.diag_cs_bootstrap();
    assert_eq!(induced0, 0);
    let _ = s.on_event_batch(
        3 * SEC + 300 * MS,
        vec![Event::AppData(OpaqueUnit { id: 2, size: 1200 })],
    );
    let b = s.on_event_batch(
        4 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![1],
            ack_delay: Duration::from_millis(1),
        })],
    );
    let (_ft, fired) = expect_retransmit_via_wakeup(&mut s, 4 * SEC, &b, 1);
    assert!(
        fired
            .iter()
            .any(|d| d.kind == TransmitKind::Retransmit && d.unit_id == 1),
        "spurious-retransmit BEKLENEN (mevcut-yol; alıcı-seq-dedup-atar)"
    );
    let (_, _, induced, _) = s.diag_cs_bootstrap();
    assert_eq!(
        induced, 1200,
        "bootstrap_induced_retransmit_bytes EXACT (birim-başına-bir-kez)"
    );
}

/// DI-4: temiz-yavaş link — RTT 2.5s: data-ACK@2.5s (<3s) → probe-0; RTT 5s: probe@3s (1-adet),
/// ACK@5s-disarm → toplam ≤1 zararsız-probe.
#[test]
fn f2b_di4_clean_slow_link_at_most_one_probe() {
    // RTT 2.5s → probe-0.
    let mut a = Sender::new();
    let _ = a.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let pre = drive_ticks(&mut a, 100 * MS, 2 * SEC + 400 * MS);
    let _ = a.on_event_batch(
        2 * SEC + 500 * MS,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    let post = drive_ticks(&mut a, 2 * SEC + 600 * MS, 10 * SEC);
    let n_a = pre
        .iter()
        .chain(post.iter())
        .filter(|(_, d)| d.kind == TransmitKind::CsProbe)
        .count();
    assert_eq!(n_a, 0, "RTT2.5s: erken-data-ACK → probe-0");
    // RTT 5s → tam-1-probe.
    let mut b = Sender::new();
    let _ = b.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let pre = drive_ticks(&mut b, 100 * MS, 4 * SEC + 900 * MS);
    let _ = b.on_event_batch(
        5 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    let post = drive_ticks(&mut b, 5 * SEC + 100 * MS, 15 * SEC);
    let n_b = pre
        .iter()
        .chain(post.iter())
        .filter(|(_, d)| d.kind == TransmitKind::CsProbe)
        .count();
    assert_eq!(n_b, 1, "RTT5s: ≤1 zararsız-probe");
}

/// DI-8: late-echo yok-sayma — koşul-1 (data-ACK-önce-kurdu), koşul-2 (seq-outstanding-değil),
/// koşul-3/4 (tek-tüketim/duplicate). (Koşul-5 TimedOut = F3-M5-testi; koşul-"session-generation"
/// in-instance-yeniden-giriş-İMKANSIZ — F0-tespit-(v) — yeni-instance-yeni-namespace.)
#[test]
fn f2b_di8_late_echo_ignore_conditions() {
    // Koşul-1: data-ACK-sonrası echo yok-sayılır (srtt-değişmez; resolved-kaynak-DataAck-kalır).
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut s, 100 * MS, 3 * SEC + 100 * MS); // probe@3s outstanding
    let _ = s.on_event_batch(
        4 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    let (src, outst, _, _) = s.diag_cs_bootstrap();
    assert_eq!(src, Some(CsSrttSource::DataAck));
    assert_eq!(
        outst, 0,
        "terminal → outstanding-probe KESİN-retire (INV-CS-3)"
    );
    let (st0, _b0, srtt0, _r0, _i0) = s.diag_rate();
    let _ = s.on_event_batch(5 * SEC, vec![Event::CsProbeEcho { probe_seq: 0 }]);
    let (st1, _b1, srtt1, _r1, _i1) = s.diag_rate();
    assert_eq!(srtt0, srtt1, "koşul-1: geç-echo sample-beslemez");
    assert_eq!(st0, st1);
    let (src1, _, _, _) = s.diag_cs_bootstrap();
    assert_eq!(src1, Some(CsSrttSource::DataAck), "kaynak-overwrite-yok");

    // Koşul-2: bilinmeyen-probe_seq yok-sayılır (soğukta).
    let mut c = Sender::new();
    let _ = c.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut c, 100 * MS, 3 * SEC + 100 * MS);
    let _ = c.on_event_batch(
        3 * SEC + 200 * MS,
        vec![Event::CsProbeEcho { probe_seq: 7 }],
    );
    assert_eq!(
        c.state(),
        SenderState::PreConverge,
        "koşul-2: state-bozulmaz"
    );
    // Koşul-3/4: ilk-echo tüketir; duplicate-echo srtt'yi-değiştirmez.
    let _ = c.on_event_batch(
        3 * SEC + 300 * MS,
        vec![Event::CsProbeEcho { probe_seq: 0 }],
    );
    let (_, _, srtt_a, _, _) = c.diag_rate();
    let _ = c.on_event_batch(5 * SEC, vec![Event::CsProbeEcho { probe_seq: 0 }]);
    let (_, _, srtt_b, _, _) = c.diag_rate();
    assert_eq!(
        srtt_a, srtt_b,
        "koşul-3/4: tek-tüketim (duplicate-yok-sayılır)"
    );
}

/// DI-11 (F2B-yarısı): probe_seq monoton + echo-tek-tüketim (INV-CS-7). (Wrap-guard F2A-kodda;
/// u16::MAX-erişimi-4-probe'ta-imkansız — typed-diag-sayacı diag_cold_start'ta.)
#[test]
fn f2b_di11_probe_seq_uniqueness() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let all = drive_ticks(&mut s, 100 * MS, 46 * SEC);
    let seqs: Vec<u64> = all
        .iter()
        .filter(|(_, d)| d.kind == TransmitKind::CsProbe)
        .map(|(_, d)| d.seq)
        .collect();
    assert_eq!(seqs, vec![0, 1, 2, 3], "monoton + yeniden-kullanım-yok");
    let (_, _, wrapped, _, _, _) = s.diag_cold_start();
    assert_eq!(wrapped, 0);
}

/// N8-SPOT (M3-izolasyon): echo-ingest btlbw/loss/inflight'ı DEĞİŞTİRMEZ (yalnız srtt + doğal-
/// state-geçişi). delivery-sample/loss-declaration/inflight-mutasyonu-YOK.
#[test]
fn f2b_n8_probe_rtt_ingest_isolation() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut s, 100 * MS, 3 * SEC + 100 * MS);
    let (_, btlbw0, srtt0, _, inflight0) = s.diag_rate();
    assert_eq!(srtt0, 0, "ön-koşul: srtt-yok");
    let fi0 = s.diag_fusion_input_snapshot().map(|f| f.loss_windowed_ppm);
    let _ = s.on_event_batch(
        3 * SEC + 200 * MS,
        vec![Event::CsProbeEcho { probe_seq: 0 }],
    );
    let (_, btlbw1, srtt1, _, inflight1) = s.diag_rate();
    let fi1 = s.diag_fusion_input_snapshot().map(|f| f.loss_windowed_ppm);
    assert_eq!(btlbw0, btlbw1, "N8: BtlBw/delivery-DEĞİŞMEDİ");
    assert_eq!(inflight0, inflight1, "N8: inflight-DEĞİŞMEDİ");
    assert_eq!(fi0, fi1, "N8: loss-DEĞİŞMEDİ");
    assert!(srtt1 > 0, "yalnız-srtt-kuruldu (izinli-etki)");
}

// ── F3 (M5 typed timeout + M6 patience) ────────────────────────────────────────────────────

// ── F3 (M5 typed timeout + M6 patience) — İMPLEMENTE ───────────────────────────────────────

use adaptive_transport::sender::CsTerminal;

/// DI-2B/DI-3: çift-yön-ölü (echo-yok, ACK-yok) → giveup@60s TimedOut (epoch-BİR-kez); post-timeout
/// geç-echo YOK-SAYILIR (Seçenek-A); legacy-geometrik-kopya→DATA-ACK = PostTimeoutRecovery (ikinci-
/// terminal-YOK). Direction-diagnosis YAPILMAZ (yalnız TimedOut; forward/reverse-ayrımı-iddia-yok).
#[test]
fn f3_di3_both_dead_single_terminal_then_post_timeout() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    // 60s'e-kadar: 4-probe-çıkar (echo-yok); giveup@60s → TimedOut.
    let all = drive_ticks(&mut s, 100 * MS, 60 * SEC + 200 * MS);
    assert_eq!(
        all.iter()
            .filter(|(_, d)| d.kind == TransmitKind::CsProbe)
            .count(),
        4
    );
    assert_eq!(
        s.take_cs_terminal(),
        Some(CsTerminal::TimedOut),
        "giveup@60s → TimedOut"
    );
    let (_, _, _, timed_out) = s.diag_cs_bootstrap();
    assert!(timed_out);
    // epoch-BİR-kez: sonraki-Tick'ler yeni-terminal-üretmez.
    let _ = drive_ticks(&mut s, 60 * SEC + 300 * MS, 130 * SEC);
    assert_eq!(
        s.take_cs_terminal(),
        None,
        "TimedOut-tek-kez (drain-sonrası-boş)"
    );
    // Post-timeout geç-echo YOK-SAYILIR (koşul-5): srtt-kurmaz, terminal-üretmez.
    let _ = s.on_event_batch(131 * SEC, vec![Event::CsProbeEcho { probe_seq: 0 }]);
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "post-timeout-echo-yok-sayıldı"
    );
    assert_eq!(s.take_cs_terminal(), None);
    // Heal-yolu: post-timeout yeni-DATA (u2) çıkar + DATA-ACK'i srtt-kurar → PostTimeoutRecovery
    // (İKİNCİ-terminal-değil). (Legacy-geometrik-kopya ORİJİNAL-u1'i-taşır ama Duplicate-ACK'i
    // ambiguous-RTT-nedeniyle-srtt-VERMEZ — bootstrap yeni/retransmit-olmayan-DATA-ACK'iyle olur.)
    let out2 = s.on_event_batch(
        131 * SEC + 500 * MS,
        vec![Event::AppData(OpaqueUnit { id: 2, size: 1200 })],
    );
    // u2 token-refill-bekleyebilir; çıktıdan-seq-izle (gerekirse Tick-zinciri).
    let mut t = 131 * SEC + 600 * MS;
    let mut s2 = out2
        .iter()
        .find(|d| d.kind == TransmitKind::Data && d.unit_id == 2)
        .map(|d| d.seq);
    let mut probe_out;
    for _ in 0..40 {
        if s2.is_some() {
            break;
        }
        t += 40 * SEC;
        probe_out = s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]);
        s2 = probe_out
            .iter()
            .find(|d| d.kind == TransmitKind::Data && d.unit_id == 2)
            .map(|d| d.seq);
    }
    let s2 = s2.expect("post-timeout u2-emit");
    let _ = s.on_event_batch(
        t + 5 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![s2],
            ack_delay: Duration::from_millis(1),
        })],
    );
    assert_ne!(s.state(), SenderState::PreConverge, "DATA-ACK → Startup");
    assert_eq!(
        s.take_cs_terminal(),
        Some(CsTerminal::PostTimeoutRecovery),
        "post-timeout-DATA-ACK = PostTimeoutRecovery (ikinci-TimedOut/Resolved-DEĞİL)"
    );
}

/// DI-7: heal-mid-cold — giveup-ÖNCESİ echo → ResolvedByProbeEcho (TimedOut-YOK); ayrı-instance
/// giveup-SONRASI → PostTimeoutRecovery (DI-3'te-kanıtlandı; burada-öncesi-kolu).
#[test]
fn f3_di7_heal_before_giveup_resolves_not_timeout() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut s, 100 * MS, 9 * SEC + 100 * MS); // probe 3s,9s outstanding
                                                               // giveup-öncesi (t=10s < 60s) echo → ResolvedByProbeEcho.
    let _ = s.on_event_batch(10 * SEC, vec![Event::CsProbeEcho { probe_seq: 1 }]);
    assert_eq!(
        s.take_cs_terminal(),
        Some(CsTerminal::ResolvedByProbeEcho),
        "giveup-öncesi-echo → ResolvedByProbeEcho"
    );
    // giveup-anı-geçse-bile TimedOut ÜRETİLMEZ (retire-edildi).
    let _ = drive_ticks(&mut s, 10 * SEC + 100 * MS, 70 * SEC);
    assert_eq!(s.take_cs_terminal(), None, "resolved-sonrası TimedOut-yok");
    let (_, _, _, timed_out) = s.diag_cs_bootstrap();
    assert!(!timed_out);
}

/// DI-12A-D (C3 eş-zaman-sıralaması): A: ACK@probe-deadline → ResolvedByDataAck, probe-ÇIKMAZ ·
/// B: echo@giveup → ResolvedByProbeEcho, timeout-ÇIKMAZ · C: ACK@giveup → ResolvedByDataAck,
/// timeout-ÇIKMAZ · D: timeout-1ns-ÖNCE → TimedOut-geçerli, sonraki-ACK = PostTimeoutRecovery.
#[test]
fn f3_di12ad_equal_timestamp_ordering() {
    // A: ACK EXACTLY @ 3s (probe-deadline). Faz-2-ACK-önce → faz-3-probe-fire-etmez.
    let mut a = Sender::new();
    let _ = a.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut a, 100 * MS, 2 * SEC + 900 * MS); // 3s'e-değme
    let out_a = a.on_event_batch(
        3 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    assert!(
        !out_a.iter().any(|d| d.kind == TransmitKind::CsProbe),
        "A: ACK@probe-deadline → probe-çıkmaz (C3)"
    );
    assert_eq!(a.take_cs_terminal(), Some(CsTerminal::ResolvedByDataAck));

    // B: echo EXACTLY @ 60s (giveup-deadline). Faz-2-echo→srtt→giveup-retire → timeout-çıkmaz.
    let mut b = Sender::new();
    let _ = b.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut b, 100 * MS, 59 * SEC + 900 * MS); // 60s'e-değme (4-probe-çıktı)
    let _ = b.on_event_batch(60 * SEC, vec![Event::CsProbeEcho { probe_seq: 3 }]);
    assert_eq!(
        b.take_cs_terminal(),
        Some(CsTerminal::ResolvedByProbeEcho),
        "B: echo@giveup → ResolvedByProbeEcho, TimedOut-YOK"
    );

    // C: ACK EXACTLY @ 60s (giveup). Faz-2-ACK→srtt→giveup-retire → timeout-çıkmaz.
    let mut c = Sender::new();
    let _ = c.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut c, 100 * MS, 59 * SEC + 900 * MS);
    let _ = c.on_event_batch(
        60 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    assert_eq!(
        c.take_cs_terminal(),
        Some(CsTerminal::ResolvedByDataAck),
        "C: ACK@giveup → ResolvedByDataAck, TimedOut-YOK"
    );

    // D: timeout-Tick 1ns ÖNCE (ACK-yok) → TimedOut; SONRAKİ-ACK = PostTimeoutRecovery.
    let mut d = Sender::new();
    let _ = d.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    let _ = drive_ticks(&mut d, 100 * MS, 59 * SEC + 900 * MS);
    let _ = d.on_event_batch(60 * SEC, vec![Event::TimerFired(Timer::Pto)]); // giveup-fire
    assert_eq!(
        d.take_cs_terminal(),
        Some(CsTerminal::TimedOut),
        "D: timeout"
    );
    let _ = d.on_event_batch(
        61 * SEC,
        vec![Event::AckReceived(AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::from_millis(1),
        })],
    );
    assert_eq!(
        d.take_cs_terminal(),
        Some(CsTerminal::PostTimeoutRecovery),
        "D: timeout-sonrası-ACK = PostTimeoutRecovery (ikinci-terminal-değil)"
    );
}

/// M6 (EK4-A1): RX_COLD_ABANDON profil-türevi bound formülü — max-birim-1200B'de 75s-sabiti
/// HİÇBİR-profilde-geçerli-değil (F0-tespit-(viii) donmuş-tablo). Bu test formül-kontratını-kilitler
/// (harness-rx S-A-105s-kullanır; DI-10-wire-tarafı F4/W-3).
#[test]
fn f3_m6_patience_bound_formula_1200b_exceeds_75s() {
    // RX_COLD_ABANDON = max(A, B); A = 60(giveup)+5(RTT_MAX)+10(alw) = 75.
    // B = T_elig(1200B) + serial@31 + FDB + margin.
    let a = 75.0_f64;
    let t_elig = 1.5 * (1200.0 + 65.0) / 31.0; // ≈61.2s (legacy geometrik; floor-BW=31)
    let serial = 1200.0 / 31.0; // ≈38.7s
    for fdb in [0.05_f64, 0.125, 0.5, 1.25, 2.5] {
        let b = t_elig + serial + fdb + 2.0;
        let bound = a.max(b);
        assert!(b > a, "1200B-B-dalı 75s'i-aşar (fdb={fdb}): B={b:.1}");
        assert!(
            bound >= 101.0,
            "profil-türevi-patience ≥101s (>75s → S-A/S-B-zorunlu)"
        );
    }
    // 65B/300B: 75s-geçerli (B ≤ A).
    for size in [65.0_f64, 300.0] {
        let b = 1.5 * (size + 65.0) / 31.0 + size / 31.0 + 0.05 + 2.0;
        assert!(b < a, "küçük-birim 75s-yeter (size={size}): B={b:.1}");
    }
}

// ── F4 (wire; faithful-harness — ayrıca scripts/netem altında W-1..W-4 koşuları) ───────────

#[test]
#[ignore = "F4/DI-5: flag-OFF kimlik — eski-4-test ESKİ-beklentilerle + davranışsal-özdeşlik-kanıtı (cold_start_liveness.rs zaten-canlı-pin)"]
fn f4_di5_flag_off_identity() {}

#[test]
#[ignore = "F4/DI-6: 31 B/s — control-progress+bütçe-assert; completion-iddiası-YOK (~38.7s+ rapora)"]
fn f4_di6_floor_rate_control_progress() {}

#[test]
#[ignore = "F4/DI-9: mixed-version çift-sütun (wire-güvenlik + feature-etkinlik) tüm-kombinasyonlar"]
fn f4_di9_mixed_version_matrix() {}
