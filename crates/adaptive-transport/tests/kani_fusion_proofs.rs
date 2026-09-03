//! BİÇİMSEL DOĞRULAMA (Kani) — `fusion.rs` saf tamsayı fonksiyonları.
//!
//! `log2_fp` shift aritmetiği yapıyor (`x >> (e-8)`, `x << (8-e)`) ve `e == 0` civarında
//! shift-overflow riski taşıyor. `Trap::mu` ise bölme yapıyor ve `a==b` / `c==d` omuz
//! durumlarında sıfıra bölme riski var. İkisi de her karar döngüsünde çalışıyor.
//!
//! Koş: `cargo kani -p adaptive-transport --tests --harness log2_fp_never_panics`

#![cfg(kani)]

use adaptive_transport::fusion::{log2_fp, Trap, PERMILLE};

/// Panik yok — shift-overflow ve `ilog2(0)` dahil her `u64` için.
#[kani::proof]
fn log2_fp_never_panics() {
    let x: u64 = kani::any();
    let _ = log2_fp(x);
}

/// **Monotonluk** — bant sıralaması buna dayanıyor. `a <= b` ise `log2_fp(a) <= log2_fp(b)`
/// olmalı; aksi hâlde düşük bant yüksek membership alır ve karar tersine döner.
#[kani::proof]
fn log2_fp_is_monotone() {
    let a: u64 = kani::any();
    let b: u64 = kani::any();
    kani::assume(a <= b);
    assert!(log2_fp(a) <= log2_fp(b), "log2_fp monoton değil");
}

/// Keyfi trapezoid için panik yok — sıfıra bölme dahil.
#[kani::proof]
fn trap_mu_never_panics() {
    let (a, b, c, d): (u64, u64, u64, u64) = (kani::any(), kani::any(), kani::any(), kani::any());
    let x: u64 = kani::any();
    let _ = Trap { a, b, c, d }.mu(x);
}

/// **Sınırlılık** — μ her zaman `[0, PERMILLE]` içinde. Aşarsa min/max kuralları
/// (`min3`, `max`) bozulur ve karar füzyonu tanımsız hâle gelir.
#[kani::proof]
fn trap_mu_is_bounded() {
    let (a, b, c, d): (u64, u64, u64, u64) = (kani::any(), kani::any(), kani::any(), kani::any());
    let x: u64 = kani::any();
    kani::assume(a <= b && b <= c && c <= d); // iyi-biçimli trapezoid
    let m = Trap { a, b, c, d }.mu(x);
    assert!(m <= PERMILLE, "mu tavanı aştı");
}
