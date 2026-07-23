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

// ── F2B (echo + bootstrap-RTT + Startup + (d) PTO binding) ─────────────────────────────────

#[test]
#[ignore = "F2B/DI-1: ilk-DATA-kaybı (65/300/1200B) — probe≤3s+PACER_ERR; echo→bootstrap-srtt→Startup; retransmit MEVCUT-PTO/ARQ-yolundan; C2-assert: due-PTO ek-RTT/ek-network-event beklemez"]
fn f2b_di1_first_data_loss_bootstrap_recovery() {}

#[test]
#[ignore = "F2B/DI-1A (C2): overdue-PTO-after-bootstrap — echo-anında deadline geçmişte → AYNI batch faz-3'te fire; busy-loop-yok; çift-fire-yok"]
fn f2b_di1a_overdue_pto_fires_same_batch() {}

#[test]
#[ignore = "F2B/DI-1C (EK4-A2): re-armed-PTO-lifecycle — her-yeni-PTO-deadline next_wakeup'ta; ACK tüm-kayıtları-retire; çift-fire/busy-loop-yok"]
fn f2b_di1c_rearmed_pto_lifecycle_exposure() {}

#[test]
#[ignore = "F2B/DI-2: tekil-ACK-frame-erasure — echo→srtt→(d)→mevcut-PTO spurious-retransmit (BEKLENEN; §2.4-a); dup-sayaç-artar; bootstrap_induced_retransmit_bytes-doğru; yön-teşhisi-İDDİA-EDİLMEZ"]
fn f2b_di2_single_ack_erasure_spurious_expected() {}

#[test]
#[ignore = "F2B/DI-4: temiz-yavaş (RTT 2.5s/5s) — ≤1 zararsız-probe; erken-data-ACK merdiveni-disarm (probe 0)"]
fn f2b_di4_clean_slow_link_at_most_one_probe() {}

#[test]
#[ignore = "F2B/DI-8: late-echo 5-yok-sayma-koşulu tek-tek — sample-beslenmez, seq-retire, state-bozulmaz"]
fn f2b_di8_late_echo_five_ignore_conditions() {}

#[test]
#[ignore = "F2B/DI-11: probe_seq benzersizliği (INV-CS-7) — monoton; yeniden-kullanım-yok; stale-echo-yok-sayılır; wrap→probe-yok+typed-diag"]
fn f2b_di11_probe_seq_uniqueness_and_wrap() {}

#[test]
#[ignore = "F2B/N8-spot: probe-RTT-ingest cwnd/BtlBw/delivery-rate/loss/ack-conf/pacing'i DEĞİŞTİRMEZ (izolasyon; M3)"]
fn f2b_n8_probe_rtt_ingest_isolation() {}

// ── F3 (M5 typed timeout + M6 patience) ────────────────────────────────────────────────────

#[test]
#[ignore = "F3/DI-2B: sustained-reverse-blackout — echo-beklenmez; bütçe+program+typed-timeout; direction-diagnosis-YAPILMAZ"]
fn f3_di2b_reverse_blackout_budget_and_timeout() {}

#[test]
#[ignore = "F3/DI-3: çift-yön-ölü — timeout ≤60s+ε epoch'ta-BİR-kez; geç-echo-YOK-SAYILIR (Seçenek-A); legacy-kopya→DATA-ACK=PostTimeoutRecovery; ikinci-terminal-YOK"]
fn f3_di3_both_dead_single_terminal_timeout() {}

#[test]
#[ignore = "F3/DI-7: heal-mid-cold — giveup-öncesi→ResolvedBy*; sonrası→yalnız-DATA-ACK-PostTimeoutRecovery"]
fn f3_di7_heal_before_and_after_giveup() {}

#[test]
#[ignore = "F3/DI-12A-D (C3): eş-zaman-sıralaması — A: ACK@probe-deadline→probe-çıkmaz; B: echo@giveup→timeout-çıkmaz; C: ACK@giveup→timeout-çıkmaz; D: timeout-1ns-önce→TimedOut-geçerli+PostTimeoutRecovery"]
fn f3_di12ad_equal_timestamp_ordering() {}

#[test]
#[ignore = "F3/DI-13A-D (C4): muhasebe — probe/echo/induced-retransmit sayaçları EXACT; rate-limit-suppress-ledger; 31B/s-shared-medium-serialization"]
fn f3_di13ad_bidirectional_accounting() {}

#[test]
#[ignore = "F3/DI-10: RX-kaynak-sınırları — hard-cap-assert; typed-diagnostic; wire-görünür-Superseded; patience EK4-A1-formül-uygunluk"]
fn f3_di10_rx_resource_caps_and_patience() {}

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
