//! Commit-K §6 — EMISSION-CONTINUITY matrisi (block-stable-authority, Seçenek-1): mode-transition
//! KUYRUKLU-parity'nin emisyonunu-BASTIRAMAZ (kuyruk-üyeliği=kalıcı-emisyon-izni; izin-kaynağı=
//! blok-otoritesi). Gerçek-kuyruk+token-yolu (plan_fec_parity/pacer.try_send; Pto-tick-drenajı).
//! 6.8 (witness-reuse/Clone) COMPILE-TIME: sender.rs `FecBlockAuthority`-doc compile_fail-doctest'leri
//! (E0382+E0599). 6.9 (cross-Sender) sender.rs::commit_k_foreign_authority_and_manual_close (typed).

mod common;

use adaptive_transport::sender::{
    AckInfo, ControllerDecisionSnapshot, Event, FecActuationMode, FecSuppressionReason, OpaqueUnit,
    RecoveredInfo, RecoveredUnit, Sender, Timer, TransmitKind,
};
use std::time::Duration;

const MS: u64 = 1_000_000;

/// Pto-tick-drenajı: FecParity-decisions'ı (block_id, esi) olarak-toplar. Emisyon-DIŞI-decisions
/// (DATA/retx) filtrelenir (kirlilik-değil; assert'ler-parity-odaklı).
fn drain(s: &mut Sender, start_ms: u64, ticks: u64) -> Vec<(u32, u16)> {
    let mut got = Vec::new();
    for t in 0..ticks {
        let out = s.on_event(Event::TimerFired(Timer::Pto), (start_ms + t * 10) * MS);
        for d in &out {
            if d.kind == TransmitKind::FecParity {
                let h = d.send_record.expect("FecParity hint-taşır");
                let adaptive_transport::sender::SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                got.push((block_id, esi));
            }
        }
    }
    got
}

/// Isınma: batch-ACK'li-DATA → btlbw/rate-kur (token-akışı; burst-cap≈2×MTU → 1200B-parity
/// çok-tick'e-yayılır — 6.5-token-delay-interleave-imkanı).
fn warm(s: &mut Sender, t0_ms: u64) -> u64 {
    let mut t = t0_ms;
    for _round in 0..3 {
        let mut seqs: Vec<u64> = Vec::new();
        for i in 0..8u64 {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: 90_000 + t + i,
                    size: 1200,
                }),
                t * MS,
            );
            seqs.extend(out.iter().map(|d| d.seq));
            t += 1;
        }
        if !seqs.is_empty() {
            s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: Duration::ZERO,
                }),
                t * MS,
            );
        }
        t += 2;
    }
    t
}

fn count_block(got: &[(u32, u16)], blk: u32) -> usize {
    got.iter().filter(|(b, _)| *b == blk).count()
}

// ── 6.1: Manual{true}-blok → parity-staged → Controller-geçişi → ESKİ-parity-EMİT-EDİLİR ──
#[test]
fn t61_manual_block_parity_survives_controller_transition() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    let t = warm(&mut s, 0);
    let auth = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(auth, 1, 2, 64), Ok(2)); // staged, drenaj-YOK
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap(); // emisyondan-ÖNCE geçiş
    let got = drain(&mut s, t + 10, 400);
    assert_eq!(
        count_block(&got, 1),
        2,
        "block-stable: Manual-bloğun-staged-parity'si Controller-geçişine-rağmen-EMİT"
    );
    // Sonraki-blok Controller-mode-kullanır:
    let auth2 = s.open_fec_block_authority();
    assert_eq!(
        auth2.mode(),
        FecActuationMode::Controller,
        "next=Controller"
    );
}

// ── 6.1-b MUTASYON-PİNİ (adversarial-MAJOR-bulgu): STAGING-izni-WITNESS'tan, AKTİF-mode'dan-DEĞİL.
// Öldürdüğü-mutasyon: staging'i `fec_emission_enabled()`-current-mode'a-bağlamak (reddedilen-Seçenek-2)
// — o-mutasyonda buradaki-close'lar 0-stage-eder ve emisyon-assert'leri-düşer. ──
#[test]
fn t61b_staging_permission_from_witness_not_current_mode() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    let t = warm(&mut s, 0);
    let auth = s.open_fec_block_authority(); // Manual{true}-witness
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap(); // CLOSE'DAN-ÖNCE aktif-mode-izinsiz
    assert_eq!(
        s.close_fec_block_manual(auth, 21, 2, 64),
        Ok(2),
        "manuel-staging-izni WITNESS'tan (aktif=Manual{{false}}-iken)"
    );
    let got = drain(&mut s, t + 10, 400);
    assert_eq!(count_block(&got, 21), 2, "staged-2 emit-2");
    // Controller-witness-varyantı: latch-staging-de-witness'tan:
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let auth2 = s.open_fec_block_authority(); // Controller-witness
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap(); // LATCH'TEN-ÖNCE aktif-mode-izinsiz
    let snap = s.controller_decision_snapshot().unwrap();
    let snap = ControllerDecisionSnapshot {
        ratio_permille: 500,
        ..snap
    };
    let latch = s.fec_controller_latch(auth2, Some(snap), 8, 4, 22, 64);
    assert_eq!(
        latch.planned_r, 4,
        "latch-staging witness'tan (aktif=Manual{{false}})"
    );
    let got2 = drain(&mut s, t + 5000, 600);
    assert_eq!(count_block(&got2, 22), 4, "staged-4 emit-4");
}

// ── 6.2: Controller-blok (non-zero-r) → Manual{false}-geçişi → ESKİ-parity-EMİT; SONRAKİ-blok-parity-YOK ──
#[test]
fn t62_controller_block_parity_survives_manual_disable() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let t = warm(&mut s, 0);
    let snap = s.controller_decision_snapshot().expect("fuzzy-koştu");
    let snap = ControllerDecisionSnapshot {
        ratio_permille: 500,
        ..snap
    }; // kuyruk-semantiği-testi (fuzzy-E2E ayrı-kanıtlı); same-event-index-korunur
    let auth = s.open_fec_block_authority();
    let latch = s.fec_controller_latch(auth, Some(snap), 8, 4, 2, 64);
    assert_eq!(latch.planned_r, 4, "staged=4");
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap(); // emisyondan-ÖNCE disable
    let got = drain(&mut s, t + 10, 400);
    assert_eq!(
        count_block(&got, 2),
        4,
        "block-stable: Controller-bloğun-parity'si Manual{{false}}-geçişine-rağmen-EMİT"
    );
    // Sonraki-blok (Manual{false}-witness): parity-ÜRETMEZ:
    let auth2 = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(auth2, 3, 2, 64), Ok(0));
    let got2 = drain(&mut s, t + 5000, 100);
    assert_eq!(count_block(&got2, 3), 0, "next-blok: parity-yok");
}

// ── 6.3: Controller-blok kendi-r'siyle-tamamlanır → Manual{true}; sonraki-blok fixed-manual ──
#[test]
fn t63_controller_block_completes_then_manual_enabled_next() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let t = warm(&mut s, 0);
    let snap = s.controller_decision_snapshot().unwrap();
    let snap = ControllerDecisionSnapshot {
        ratio_permille: 250,
        ..snap
    };
    let auth = s.open_fec_block_authority();
    let latch = s.fec_controller_latch(auth, Some(snap), 8, 4, 4, 64);
    assert_eq!(latch.planned_r, 2, "kendi-r'si (250‰×8=2)");
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    let got = drain(&mut s, t + 10, 400);
    assert_eq!(
        count_block(&got, 4),
        2,
        "eski-Controller-blok kendi-r'siyle-tamam"
    );
    // Sonraki-blok fixed-manual:
    let auth2 = s.open_fec_block_authority();
    assert_eq!(auth2.mode(), FecActuationMode::Manual { enabled: true });
    assert_eq!(s.close_fec_block_manual(auth2, 5, 1, 64), Ok(1));
    let got2 = drain(&mut s, t + 5000, 200);
    assert_eq!(count_block(&got2, 5), 1, "next=fixed-manual-r");
}

// ── 6.4: emergency-suppression latch-anında-verildi → sonraki-transition-DEĞİŞTİREMEZ ──
#[test]
fn t64_emergency_suppression_stable_across_transition() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let t = warm(&mut s, 0);
    let snap = s.controller_decision_snapshot().unwrap();
    let em = ControllerDecisionSnapshot {
        ratio_permille: 500,
        emergency: true,
        ..snap
    };
    let auth = s.open_fec_block_authority();
    let latch = s.fec_controller_latch(auth, Some(em), 8, 4, 6, 64);
    assert_eq!(
        (latch.planned_r, latch.suppression),
        (0, Some(FecSuppressionReason::EmergencyPendingArbitration))
    );
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    // Karar-kaydı-geriye-dönük-DEĞİŞMEZ + o-blok-için-parity-YOK (staged-0'dı):
    assert_eq!(
        s.diag_fec_actuation().last_latch.unwrap().suppression,
        Some(FecSuppressionReason::EmergencyPendingArbitration),
        "transition suppression-kararını-değiştiremez"
    );
    let got = drain(&mut s, t + 10, 200);
    assert_eq!(count_block(&got, 6), 0, "interlock-blok: emisyon-yok");
    assert_eq!(
        s.diag_fec_actuation().enqueued_r_total,
        0,
        "vakum-pini: transition-retroaktif-staging-yapamaz (adversarial-m10)"
    );
}

// ── 6.5: token-delay — çok-scheduling-turu-beklerken-transition; authority-bağlamı-kaybolmaz ──
#[test]
fn t65_token_delayed_emission_across_transition() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    let t = warm(&mut s, 0);
    // 4×1200B parity: burst-cap≈2×MTU → tek-tick'te-hepsi-ÇIKAMAZ → çok-tur-garanti.
    let auth = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(auth, 7, 4, 1200), Ok(4));
    // İlk-emisyon-görülene-kadar-sür; SONRA transition; kalanlar-yeni-mode-altında-emit-olmalı.
    let mut got: Vec<(u32, u16)> = Vec::new();
    let mut tick = 0u64;
    while count_block(&got, 7) == 0 && tick < 400 {
        got.extend(drain(&mut s, t + 10 + tick * 10, 1));
        tick += 1;
    }
    let before = count_block(&got, 7);
    assert!(before >= 1, "en-az-1-parity-token-geldi (tick={tick})");
    assert!(before < 4, "token-delay-gerçek: hepsi-tek-turda-çıkmadı");
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap(); // bekleme-ortasında-transition
    got.extend(drain(&mut s, t + 10 + tick * 10 + 10, 4000));
    assert_eq!(
        count_block(&got, 7),
        4,
        "kalan-parity transition-sonrası-da-emit (authority-bağlamı-kuyruk-üyeliğinde-kalıcı)"
    );
    let d = s.diag_fec_actuation();
    assert_eq!(d.emitted_r_total, 4, "diag-zinciri: enqueued==emitted");
}

// ── 6.6: RECOVERED transition-SONRASI eski-blok-parity'sini-iptal-edebilir ──
#[test]
fn t66_recovered_cancels_parity_after_transition() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    let t = warm(&mut s, 0);
    let auth = s.open_fec_block_authority();
    assert_eq!(s.close_fec_block_manual(auth, 8, 3, 64), Ok(3)); // staged; drenaj-YOK
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap();
    // RECOVERED (receiver-decode) → blok-8-parity'si-anlamsız → cancel (authority-binding-bozulmaz):
    s.on_event(
        Event::RecoveredControl(RecoveredInfo {
            units: vec![
                RecoveredUnit {
                    unit_id: 800,
                    block_id: 8,
                    esi: 0,
                },
                RecoveredUnit {
                    unit_id: 801,
                    block_id: 8,
                    esi: 1,
                },
            ],
        }),
        (t + 5) * MS,
    );
    assert_eq!(
        s.diag_fec_actuation().canceled_r_total,
        3,
        "3-staged-parity-iptal — ÇİFT-SAYIM-YOK (2-unit-aynı-blok → 3, 6-değil; canceled_r_total=PARITY-iptalleri; corrigendum)"
    );
    let got = drain(&mut s, t + 10, 300);
    assert_eq!(count_block(&got, 8), 0, "iptal-sonrası-emisyon-yok");
}

// ── 6.7: interleaved-bloklar farklı-captured-mode → her-blok-kendi-otoritesiyle; contamination-YOK ──
#[test]
fn t67_interleaved_blocks_no_contamination() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    let t = warm(&mut s, 0);
    let auth1 = s.open_fec_block_authority(); // Manual{true}-witness
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let auth2 = s.open_fec_block_authority(); // Controller-witness (interleaved-açık)
    assert_ne!(auth1.generation(), auth2.generation(), "witness-uniqueness");
    // İki-blok-kendi-witness'ıyla-kapanır; ÜÇÜNCÜ-geçiş İKİ-CLOSE-ARASINDA (mutasyon-pini:
    // auth2-latch'i aktif=Manual{false}-iken-koşar → staging-current-mode'a-bağlansaydı-0-stage'lerdi):
    assert_eq!(s.close_fec_block_manual(auth1, 11, 2, 64), Ok(2));
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap();
    let snap = s.controller_decision_snapshot().unwrap();
    let snap = ControllerDecisionSnapshot {
        ratio_permille: 500,
        ..snap
    };
    let latch = s.fec_controller_latch(auth2, Some(snap), 8, 4, 12, 64);
    assert_eq!(latch.planned_r, 4);
    let got = drain(&mut s, t + 10, 600);
    assert_eq!(
        count_block(&got, 11),
        2,
        "blok-11 (Manual-witness): kendi-2'si"
    );
    assert_eq!(
        count_block(&got, 12),
        4,
        "blok-12 (Controller-witness): kendi-4'ü"
    );
}
