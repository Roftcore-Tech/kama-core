//! A2-F4 — SENDER PostTimeoutNotice KABUL-PİNLERİ (Abkarada §10; K-8/D-6):
//! TimedOut-sonrası valid-proof → PostTimeoutNotice (first-valid-proof-wins; duplicate/stale no-op);
//! N8 BtlBw-bit-özdeşlik (notice-fires-vs-noop); tek-terminal (CsTerminal DEĞİŞMEZ); flag-OFF-özdeş.

#![cfg(feature = "experimental-cold-start")]

use adaptive_transport::sender::{
    CsTerminal, Event, OpaqueUnit, PostTimeoutCause, RecoveredInfo, RecoveredUnit, Sender, Timer,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;

fn drive_ticks(s: &mut Sender, from_ns: u64, to_ns: u64) {
    let mut t = from_ns;
    while t <= to_ns {
        let _ = s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]);
        t += 100 * MS;
    }
}

fn recovered(block_id: u32) -> RecoveredInfo {
    RecoveredInfo {
        units: vec![RecoveredUnit {
            unit_id: u64::from(block_id) * 100,
            block_id,
            esi: 0,
        }],
    }
}

/// Bootstrap → giveup@60s → TimedOut (cs_epoch_timed_out set). take_cs_terminal drain eder.
fn drive_to_timed_out(s: &mut Sender) {
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    drive_ticks(s, 100 * MS, 60 * SEC + 200 * MS);
    assert_eq!(s.take_cs_terminal(), Some(CsTerminal::TimedOut));
}

/// TETİK-2b: TimedOut-sonrası RecoveredControl → PostTimeoutNotice{RecoveredControl} (65B-sınıfı heal).
#[test]
fn f4_recovered_control_after_timeout_yields_notice() {
    let mut s = Sender::new();
    drive_to_timed_out(&mut s);
    assert_eq!(
        s.take_post_timeout_notice(),
        None,
        "TimedOut tek-başına notice üretmez"
    );

    let _ = s.on_event(Event::RecoveredControl(recovered(7)), 61 * SEC);
    let n = s
        .take_post_timeout_notice()
        .expect("RecoveredControl notice üretmeli");
    assert_eq!(n.cause, PostTimeoutCause::RecoveredControl);
    assert_eq!(n.block_id, 7);
}

/// FIRST-VALID-PROOF-WINS: iki RecoveredControl (take-etmeden) → tek notice (ilk kanıt kalır).
#[test]
fn f4_first_valid_proof_wins() {
    let mut s = Sender::new();
    drive_to_timed_out(&mut s);
    let _ = s.on_event(Event::RecoveredControl(recovered(5)), 61 * SEC);
    let _ = s.on_event(Event::RecoveredControl(recovered(9)), 62 * SEC);
    let n = s.take_post_timeout_notice().unwrap();
    assert_eq!(
        n.block_id, 5,
        "ikinci proof ilk-notice'i ezdi (first-valid-proof ihlali)"
    );
    // Drain sonrası slot boş.
    assert_eq!(s.take_post_timeout_notice(), None);
}

/// TimedOut-ÖNCESİ RecoveredControl → notice YOK (yalnız post-timeout).
#[test]
fn f4_recovered_before_timeout_no_notice() {
    let mut s = Sender::new();
    let _ = s.on_event_batch(0, vec![Event::AppData(OpaqueUnit { id: 1, size: 1200 })]);
    // TimedOut'tan ÖNCE RecoveredControl.
    let _ = s.on_event(Event::RecoveredControl(recovered(3)), 5 * SEC);
    assert_eq!(
        s.take_post_timeout_notice(),
        None,
        "TimedOut-öncesi RecoveredControl notice üretti"
    );
}

/// N8 BİT-ÖZDEŞLİK: RecoveredControl-fires (post-timeout, notice-üretir) BtlBw'yi DEĞİŞTİRMEZ.
/// notice SALT-LOGICAL — kredi/pacer/rate'e sıfır etki (fires-vs-noop bit-özdeş taban).
#[test]
fn f4_notice_does_not_touch_btlbw_n8() {
    let mut s = Sender::new();
    drive_to_timed_out(&mut s);
    let (lfb_before, w_before) = s.diag_wbtlbw();
    // Notice-üreten RecoveredControl.
    let _ = s.on_event(Event::RecoveredControl(recovered(7)), 61 * SEC);
    assert!(s.take_post_timeout_notice().is_some(), "notice üretilmeli");
    let (lfb_after, w_after) = s.diag_wbtlbw();
    assert_eq!(
        lfb_before, lfb_after,
        "notice BtlBw-last-filtered'ı değiştirdi (N8 ihlali)"
    );
    assert_eq!(
        w_before, w_after,
        "notice W_BTLBW'yi değiştirdi (N8 ihlali)"
    );
}
