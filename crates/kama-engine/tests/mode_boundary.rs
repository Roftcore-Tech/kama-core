//! Commit-I/K — MODE-BOUNDARY test-matrisi (§2.1-2.10; gerçek-blok-state: blok-sahibi=bu-harness).
//! Yapısal-garanti: `FecBlockAuthority` witness blok-AÇILIŞINDA mode'u-yakalar; kapanış (Controller-
//! latch / manuel-close) witness'ı BY-VALUE-TÜKETİR → "açıldığı-mode'la-kapanır" + tek-kullanım
//! tip-düzeyi (caller-disiplin-varsayımı-DEĞİL). Commit-K: kapanışta-kuyruklanan-parity üyelik=izin
//! (sonraki-global-mode-değişimi-bastıramaz — block-stable; emisyon-testleri emission_continuity.rs).

mod common;

use adaptive_transport::sender::{
    ControllerDecisionSnapshot, Event, FecActuationMode, FecBlockAuthority, FecSuppressionReason,
    OpaqueUnit, Sender,
};

const MS: u64 = 1_000_000;

/// Blok-sahibi (harness): open-anında witness-yakalar; kapanışta witness-mode'una-göre-dal.
/// Commit-K: `close(self)` — witness'la-birlikte-sahibi-de-tüketilir (1-blok=1-witness=1-kapanış).
struct BlockOwner {
    authority: FecBlockAuthority,
    block_id: u32,
    sources_added: u8,
}

impl BlockOwner {
    fn open(s: &mut Sender, block_id: u32) -> Self {
        Self {
            authority: s.open_fec_block_authority(),
            block_id,
            sources_added: 0,
        }
    }
    fn add_source(&mut self, s: &mut Sender, t_ms: u64) {
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: u64::from(self.block_id) * 100 + u64::from(self.sources_added),
                size: 1200,
            }),
            t_ms * MS,
        );
        self.sources_added += 1;
    }
    /// Kapanış: witness-mode'una-göre (Manual→`close_fec_block_manual` / Controller→latch; her-iki-yol
    /// otorite-bağlı, staging-dahili). Döner: (mode-at-close, planned_r, controller_ratio_at_close).
    fn close(
        self,
        s: &mut Sender,
        manual_fixed_r: u16,
        r_max: u16,
    ) -> (FecActuationMode, u16, u32) {
        let mode = self.authority.mode();
        match mode {
            FecActuationMode::Manual { .. } => {
                let r = s
                    .close_fec_block_manual(self.authority, self.block_id, manual_fixed_r, 64)
                    .expect("aynı-sender + Manual-witness");
                (mode, r, 0)
            }
            FecActuationMode::Controller => {
                let snap = s.controller_decision_snapshot();
                let ratio = snap.map_or(0, |x| x.ratio_permille);
                let latch = s.fec_controller_latch(
                    self.authority,
                    snap,
                    self.sources_added,
                    r_max,
                    self.block_id,
                    64,
                );
                (mode, latch.planned_r, ratio)
            }
        }
    }
}

fn manual_sender(enabled: bool) -> Sender {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled })
        .unwrap();
    s
}

// ── 2.1: Manual{true} → Controller, BLOK-AÇIKKEN ──
#[test]
fn t21_manual_enabled_to_controller_open_block() {
    let mut s = manual_sender(true);
    let mut blk = BlockOwner::open(&mut s, 1);
    blk.add_source(&mut s, 0); // en-az-1-source blok'ta
                               // Blok-kapanmadan Controller-geçişi:
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    // Current-blok MANUAL-davranışıyla-kapanır (witness):
    let (mode, planned, _) = blk.close(&mut s, 2, 4);
    assert_eq!(
        mode,
        FecActuationMode::Manual { enabled: true },
        "current=Manual"
    );
    assert_eq!(planned, 2, "current-blok Manual-fixed-r");
    // Sonraki-blok CONTROLLER'la-başlar:
    let mut blk2 = BlockOwner::open(&mut s, 2);
    blk2.add_source(&mut s, 10);
    let (mode2, _, ratio2) = blk2.close(&mut s, 2, 4);
    assert_eq!(
        mode2,
        FecActuationMode::Controller,
        "next=Controller (effective-block-ID=2)"
    );
    let _ = ratio2; // controller-ratio-okundu (fixture-düşük-loss→0 olabilir; mode-kanıtı-esas)
    let d = s.diag_fec_actuation();
    assert_eq!(
        d.transition_applied_count, 2,
        "2-transition (ctor-sonrası Manual-set + Controller)"
    );
}

// ── 2.2: Manual{false} → Controller ──
#[test]
fn t22_manual_disabled_to_controller() {
    let mut s = manual_sender(false);
    let mut blk = BlockOwner::open(&mut s, 1);
    blk.add_source(&mut s, 0);
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let (mode, planned, _) = blk.close(&mut s, 2, 4);
    assert_eq!(
        (mode, planned),
        (FecActuationMode::Manual { enabled: false }, 0),
        "current: parity-YOK"
    );
    let mut blk2 = BlockOwner::open(&mut s, 2);
    blk2.add_source(&mut s, 10);
    let (mode2, ..) = blk2.close(&mut s, 2, 4);
    assert_eq!(
        mode2,
        FecActuationMode::Controller,
        "next: controller-kararı"
    );
}

// ── 2.3/2.4: Controller → Manual{false} / Manual{true} ──
#[test]
fn t23_t24_controller_to_manual() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let mut blk = BlockOwner::open(&mut s, 1);
    blk.add_source(&mut s, 0);
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap();
    // Current-Controller-bloğu mevcut-controller-snapshot'ıyla-kapanır:
    let (mode, ..) = blk.close(&mut s, 2, 4);
    assert_eq!(mode, FecActuationMode::Controller, "2.3-current=Controller");
    // Sonraki: Manual{false}:
    let mut b2 = BlockOwner::open(&mut s, 2);
    b2.add_source(&mut s, 10);
    assert_eq!(
        b2.close(&mut s, 2, 4).1,
        0,
        "2.3-next=Manual{{false}} parity-yok"
    );
    // 2.4: Controller-blok-açık → Manual{true}:
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let mut b3 = BlockOwner::open(&mut s, 3);
    b3.add_source(&mut s, 20);
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    assert_eq!(
        b3.close(&mut s, 2, 4).0,
        FecActuationMode::Controller,
        "2.4-current=Controller"
    );
    let mut b4 = BlockOwner::open(&mut s, 4);
    b4.add_source(&mut s, 30);
    let (m4, p4, _) = b4.close(&mut s, 2, 4);
    assert_eq!(
        (m4, p4),
        (FecActuationMode::Manual { enabled: true }, 2),
        "2.4-next=fixed-manual"
    );
}

// ── 2.5: idle-transition (açık-blok-yok) HEMEN ──
#[test]
fn t25_idle_immediate() {
    let mut s = manual_sender(true);
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    assert_eq!(
        s.diag_fec_actuation().authority_mode,
        FecActuationMode::Controller
    );
    let blk = BlockOwner::open(&mut s, 1);
    assert_eq!(
        blk.authority.mode(),
        FecActuationMode::Controller,
        "yeni-blok-yeni-mode-hemen"
    );
}

// ── 2.6: partial-block (k_actual<K) açıkken-transition ──
#[test]
fn t26_partial_block_transition() {
    let mut s = manual_sender(true);
    let mut blk = BlockOwner::open(&mut s, 1);
    for t in 0..5 {
        blk.add_source(&mut s, t); // k_actual=5 partial
    }
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let (mode, planned, _) = blk.close(&mut s, 2, 4);
    assert_eq!(
        (mode, planned),
        (FecActuationMode::Manual { enabled: true }, 2),
        "partial-eski-mode"
    );
    let b2 = BlockOwner::open(&mut s, 2);
    assert_eq!(
        b2.authority.mode(),
        FecActuationMode::Controller,
        "sonraki-yeni-mode"
    );
}

// ── 2.7: emergency-sırasında-transition (suppression≠transition; effective-block-doğru) ──
#[test]
fn t27_emergency_and_transition() {
    let mut s = Sender::new();
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    let mut blk = BlockOwner::open(&mut s, 1);
    blk.add_source(&mut s, 0);
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .unwrap();
    // Current-Controller-bloğu EMERGENCY-snapshot'la-kapanır: suppression-planned=0 AMA mode=Controller.
    let snap = s.controller_decision_snapshot().unwrap();
    let em = ControllerDecisionSnapshot {
        emergency: true,
        ratio_permille: 500,
        ..snap
    };
    let latch = s.fec_controller_latch(blk.authority, Some(em), 8, 4, 1, 64);
    assert_eq!(latch.planned_r, 0);
    assert_eq!(
        latch.suppression,
        Some(FecSuppressionReason::EmergencyPendingArbitration)
    );
    // Transition-etkisi-sonraki-blokta (mode-state-korunmuş; suppression-transition-DEĞİL):
    let b2 = BlockOwner::open(&mut s, 2);
    assert_eq!(
        b2.authority.mode(),
        FecActuationMode::Manual { enabled: true }
    );
}

// ── 2.8: pending-parity varken transition → mevcut-blok-KARARI-geriye-dönük-değişmez ──
#[test]
fn t28_pending_parity_decision_immutable() {
    let mut s = manual_sender(true);
    let mut blk = BlockOwner::open(&mut s, 1);
    blk.add_source(&mut s, 0);
    let (_, planned, _) = blk.close(&mut s, 3, 4); // 3-parity-enqueued (pending)
    assert_eq!(planned, 3);
    let enq_before = s.diag_fec_actuation().enqueued_r_total;
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap();
    // Karar/diag geriye-dönük-DEĞİŞMEZ:
    assert_eq!(s.diag_fec_actuation().enqueued_r_total, enq_before);
    // Commit-K (Seçenek-1 block-stable): enqueued-parity'nin EMİSYONU-da-artık-current-mode'a-TABİ-DEĞİL
    // (üyelik=izin; eski-DR-S5-1-pause/resume-drain-semantiği-bilinçli-kaldırıldı) — davranış-kanıtı
    // emission_continuity.rs::t62-ailesi.
}

// ── 2.9: repeated-transition aynı-blok-açıkken → LAST-WRITE-WINS (aktif); witness-etkilenmez ──
#[test]
fn t29_repeated_transition_last_write_wins() {
    let mut s = manual_sender(true);
    let blk = BlockOwner::open(&mut s, 1);
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: false })
        .unwrap();
    // Semantik-AÇIK: last-write-wins aktif-mode'da:
    assert_eq!(
        s.diag_fec_actuation().authority_mode,
        FecActuationMode::Manual { enabled: false }
    );
    // Açık-witness-ETKİLENMEZ:
    assert_eq!(
        blk.authority.mode(),
        FecActuationMode::Manual { enabled: true }
    );
    assert_eq!(s.diag_fec_actuation().transition_applied_count, 3);
}

// ── 2.10: no-future-block → pending-leak-YOK (witness-modeli pending-tutmaz) ──
#[test]
fn t210_no_future_block_no_leak() {
    let mut s = manual_sender(true);
    s.set_fec_actuation_mode(FecActuationMode::Controller)
        .unwrap();
    // Yeni-blok-HİÇ-açılmasa-da: aktif-mode HEMEN-Controller (API-garantisi); pending-state-YOK.
    let d = s.diag_fec_actuation();
    assert_eq!(
        d.authority_mode,
        FecActuationMode::Controller,
        "gözlenen-aktif-mode"
    );
    assert_eq!(d.transition_applied_count, 2);
    // Leak-testi: başka-hiçbir-transition-alanı-beklemede-değil (pending-alanı-tip-düzeyinde-yok).
    // Commit-K: tüketilmeden-drop-edilen-witness-de-leak-üretmez (sender-tarafı-state-yok;
    // scope-sonu-doğal-drop — Drop-impl-yok):
    {
        let _dropped = s.open_fec_block_authority();
    }
    assert_eq!(
        s.diag_fec_actuation().enqueued_r_total,
        0,
        "drop=terk; iz-yok"
    );
}
