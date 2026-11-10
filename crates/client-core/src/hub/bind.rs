//! PoC-BIND istemci durum makinesi (S4 §2.4(a)–(b), C2, C8; KK-H40, KK-H44).
//!
//! HB1-M0: yalnız sabitler (durum makinesi HB1-M1'de).

use core::time::Duration;

/// Bir turdaki (CLIENT_BIND ya da BIND_PROOF) azami gönderim (C2 "en çok 3 kez, 1 s arayla").
pub const BIND_ROUND_SENDS: u32 = 3;
/// Tur içi tekrar aralığı (jitter YOK; KK-H48).
pub const BIND_RESEND: Duration = Duration::from_secs(1);
