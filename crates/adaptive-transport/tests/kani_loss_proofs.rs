//! BİÇİMSEL DOĞRULAMA (Kani) — `loss.rs` saf eşik fonksiyonları.
//!
//! ---
//! **[ÖLÇÜLDÜ] BULGU K-2 — `loss.rs:44` sıfıra bölme.**
//!
//! ```rust
//! fn scale_duration(d: Duration, num: u64, den: u64) -> Duration {
//!     let ns = d.as_nanos() * u128::from(num) / u128::from(den);   // ← den == 0 → PANİK
//! ```
//!
//! `den` `LossConfig.time_den` alanından geliyor; alan **public** ve `with_config()` ile
//! keyfi değer alabiliyor (bkz. `emergency_and_accessors::with_config_actually_carries_the_given_config`
//! — config'in gerçekten taşındığı ölçüldü, yani yol açık).
//!
//! **Aynı dosyada asimetri var:** `ratio_ppm` `if den == 0 { return 0; }` ile koruyor,
//! `scale_duration` korumuyor. `pn.rs` / `seq.rs` asimetrisiyle (K-1) birebir aynı desen:
//! doğru sürüm zaten kod tabanında mevcut, yalnız bu çağrı yerinde uygulanmamış.
//!
//! [BEKLİYOR — Abkarada] `scale_duration`'a `den == 0` guard'ı mı eklensin, yoksa
//! `LossConfig::validate()` mi gerekli (`time_den >= 1` invariantı)?
//! ---
//!
//! Koş: `cargo kani -p adaptive-transport --tests --harness time_threshold_unconstrained_never_panics`

#![cfg(kani)]

use adaptive_transport::loss::{packet_threshold_lost, time_threshold_lost};
use std::time::Duration;

/// Panik yok — `seq` ve `largest_acked` saldırgan-etkili.
#[kani::proof]
fn packet_threshold_never_panics() {
    let (s, l, k): (u64, u64, u64) = (kani::any(), kani::any(), kani::any());
    let _ = packet_threshold_lost(s, l, k);
}

/// **Sözleşme içi** (`den != 0`) panik yok — [ÖLÇÜLDÜ] SUCCESSFUL.
/// Bu, "varsayılan config ile sistem güvenli" iddiasının dayanağı.
#[kani::proof]
fn time_threshold_bounded_never_panics() {
    let (e, s, m): (u64, u64, u64) = (kani::any(), kani::any(), kani::any());
    let (num, den): (u64, u64) = (kani::any(), kani::any());
    kani::assume(den != 0);
    kani::assume(num <= 16);
    kani::assume(e < 1u64 << 40 && s < 1u64 << 40 && m < 1u64 << 40);
    let _ = time_threshold_lost(
        Duration::from_nanos(e),
        Duration::from_nanos(s),
        Duration::from_nanos(m),
        num,
        den,
    );
}

/// **KISITSIZ** (`den == 0` dahil) — [ÖLÇÜLDÜ] FAILED, `division by zero` @ `loss.rs:44`.
///
/// KASTEN KIRMIZI: guard eklendiğinde yeşile döner ve düzeltmeyi haber verir.
#[kani::proof]
fn time_threshold_unconstrained_never_panics() {
    let (e, s, m): (u64, u64, u64) = (kani::any(), kani::any(), kani::any());
    let (num, den): (u64, u64) = (kani::any(), kani::any());
    kani::assume(e < 1u64 << 40 && s < 1u64 << 40 && m < 1u64 << 40);
    kani::assume(num <= 16);
    let _ = time_threshold_lost(
        Duration::from_nanos(e),
        Duration::from_nanos(s),
        Duration::from_nanos(m),
        num,
        den,
    );
}
