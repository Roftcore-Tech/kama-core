//! BİÇİMSEL DOĞRULAMA (Kani) — `seq.rs::resolve_seq`.
//!
//! **Neden ayrıca doğrulandı:** Bu fonksiyon `pn.rs::reconstruct` ile AYNI algoritmadır
//! (RFC 9000 A.3, 16-bit özelleştirmesi). `reconstruct` K-1 defekti veriyor; bu ise
//! KISITSIZ `u64` uzayında üç ispatı da geçiyor.
//!
//! Fark tek bir guard'da:
//! ```text
//! seq.rs:26   candidate <= expected - hwin && candidate <= u64::MAX - win   ← VAR
//! pn.rs:51    cand.checked_add(half).is_some_and(|v| v < center)            ← YOK
//! ```
//!
//! Yani K-1 bir tasarım tercihi değil, kopyalama sırasında düşmüş bir kontroldür ve
//! doğru sürümü zaten depoda mevcuttur.
//!
//! Koş: `cargo kani -p adaptive-transport --tests --harness resolve_seq_never_panics`

#![cfg(kani)]

use adaptive_transport::seq::resolve_seq;

/// Panik yok — `wire` saldırgan-kontrollü (SESSION_DATA'dan okunuyor).
#[kani::proof]
fn resolve_seq_never_panics() {
    let e: u64 = kani::any();
    let w: u16 = kani::any();
    let _ = resolve_seq(e, w);
}

/// Düşük 16 bit her zaman `wire`'a eşit — yeniden kurulum wire'ı bozmaz.
#[kani::proof]
fn resolve_seq_preserves_low_bits() {
    let e: u64 = kani::any();
    let w: u16 = kani::any();
    assert!(
        resolve_seq(e, w) & 0xFFFF == u64::from(w),
        "düşük bitler bozuldu"
    );
}

/// Sonuç `expected`'e en fazla bir pencere uzaklıkta — algoritmanın tanımı.
/// `seq_and_reader_bounds::result_is_always_within_one_window_of_expected` bunu 20 noktada
/// ÖRNEKLİYOR; bu, ispat hâli.
#[kani::proof]
fn resolve_seq_within_one_window() {
    let e: u64 = kani::any();
    let w: u16 = kani::any();
    let g = resolve_seq(e, w);
    let d = if g >= e { g - e } else { e - g };
    assert!(d <= (1u64 << 16), "sonuç bir pencereden uzağa taşındı");
}
