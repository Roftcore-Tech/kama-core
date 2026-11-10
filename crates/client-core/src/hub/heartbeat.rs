//! Heartbeat, aktif yoklama ve "koptu" (S4 C6, C7, C9; KK-H41) + RNG'siz jitter (KK-H48).
//!
//! HB1-M0: yalnız sabitler (gövdeler HB1-M1'de).

use core::time::Duration;

/// Ön plan kadansı (KS-HB-1): hub'a giden son datagramdan bu kadar sonra heartbeat.
pub const HEARTBEAT_FOREGROUND: Duration = Duration::from_secs(8);
/// Arka plan kadansı (KS-HB-1). Tanımlıdır ama kullanılmaz: kama-node'un ön/arka plan sinyali yok
/// (KK-H41; DR-CONNECT-1 K5 PARK; transport-tdd §10 AMENDMENT-10 madde 6).
pub const HEARTBEAT_BACKGROUND: Duration = Duration::from_secs(25);
/// Heartbeat ACK bekleme süresi; dolarsa aktif yoklama başlar.
pub const ACK_WAIT: Duration = Duration::from_secs(1);
/// Aktif yoklama: 3 heartbeat × 1 s (C6; jitter YOK).
pub const PROBE_COUNT: u32 = 3;
/// Yoklama aralığı.
pub const PROBE_INTERVAL: Duration = Duration::from_secs(1);
