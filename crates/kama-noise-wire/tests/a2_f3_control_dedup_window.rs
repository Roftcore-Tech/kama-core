//! F3 — ControlDedupWindow fixed-window dedup KABUL-PİNLERİ (Abkarada §9 semantiği).

#![cfg(feature = "experimental-cold-start")]

use kama_noise_wire::{ControlDedupWindow, DedupDecision, CONTROL_DEDUP_WINDOW};

/// id > highest → Fresh (high-water ilerler).
#[test]
fn f3_forward_progress_fresh() {
    let mut w = ControlDedupWindow::new();
    assert_eq!(w.check(0), DedupDecision::Fresh);
    assert_eq!(w.check(1), DedupDecision::Fresh);
    assert_eq!(w.check(100), DedupDecision::Fresh);
    assert_eq!(w.check(101), DedupDecision::Fresh);
}

/// window-içi marked → DuplicateAckOnly; unmarked (reorder) → Fresh.
#[test]
fn f3_window_marked_vs_unmarked() {
    let mut w = ControlDedupWindow::new();
    // 10 işaretlenir (high-water=10). Ara-id'ler (0..9) unmarked (advance temizledi).
    assert_eq!(w.check(10), DedupDecision::Fresh);
    // 10-tekrar → marked → DuplicateAckOnly.
    assert_eq!(w.check(10), DedupDecision::DuplicateAckOnly);
    // window-içi unmarked (reorder-gelen 5) → Fresh.
    assert_eq!(w.check(5), DedupDecision::Fresh);
    // 5-tekrar → marked → DuplicateAckOnly.
    assert_eq!(w.check(5), DedupDecision::DuplicateAckOnly);
}

/// window-altı → DuplicateAckOnly + below-window teşhis (evict-edilmiş aralık).
#[test]
fn f3_below_window_diag() {
    let mut w = ControlDedupWindow::new();
    // high-water WINDOW+50 → 0..50 window-altına düşer.
    let high = CONTROL_DEDUP_WINDOW + 50;
    assert_eq!(w.check(high), DedupDecision::Fresh);
    assert_eq!(w.below_window_count(), 0);
    // 10 (high - WINDOW - 40 < high-WINDOW) window-altı → DuplicateAckOnly + diag.
    assert_eq!(w.check(10), DedupDecision::DuplicateAckOnly);
    assert_eq!(w.below_window_count(), 1, "below-window teşhis sayılmadı");
    // window-içi (high-100) hâlâ ayırt edilir.
    assert_eq!(w.check(high - 100), DedupDecision::Fresh);
}

/// Büyük atlama (≥ WINDOW): bitmap tamamen temizlenir; eski-marked yeniden-Fresh (window-dışı).
#[test]
fn f3_large_jump_clears_bitmap() {
    let mut w = ControlDedupWindow::new();
    assert_eq!(w.check(5), DedupDecision::Fresh);
    // 5 + 2×WINDOW → büyük atlama; 5 artık window-altı.
    let far = 5 + 2 * CONTROL_DEDUP_WINDOW;
    assert_eq!(w.check(far), DedupDecision::Fresh);
    // far'a yakın yeni-id → Fresh (temiz-bitmap).
    assert_eq!(w.check(far - 10), DedupDecision::Fresh);
    assert_eq!(w.check(far - 10), DedupDecision::DuplicateAckOnly);
}
