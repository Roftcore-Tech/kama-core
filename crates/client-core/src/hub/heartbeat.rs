//! Heartbeat, aktif yoklama ve "koptu" (S4 C6, C7; KK-H41) + RNG'siz jitter (KK-H48).
//!
//! * Heartbeat yalnız **sessizlikte**: hub'a giden son datagramdan (her tür: kontrol çerçevesi ya da
//!   HB2'nin bildirdiği oturum datagramı) [`HEARTBEAT_FOREGROUND`] + jitter sonra `HEARTBEAT{token}`.
//! * ACK [`ACK_WAIT`] içinde gelmezse aktif yoklama: [`PROBE_COUNT`] heartbeat × [`PROBE_INTERVAL`].
//!   Hiçbiri cevaplanmazsa "koptu" → yeniden bind (C7). Pasif sessizlik tek başına hiçbir şeyi
//!   düşürmez; kaçırma sayacı yoktur.
//! * Yoklamayı bugün heartbeat cevapsızlığı ve soket hatası (HB2) tetikler. ARQ RTO yedeklemesi ve OS
//!   sinyali Aralık'tadır (KK-H42).
//!
//! Jitter (KK-H48, transport-tdd §10 AMENDMENT-10 madde 5 "Jitter ZORUNLU"):
//! `jitter = BLAKE2s-256(seed ‖ u32_be(deneme))[0..4] (u32 BE) mod pencere`, `pencere` = adımın %25'i
//! (ms çözünürlüğü). Uygulandığı yerler: heartbeat vadesi ve tükenen bind turundan sonraki bekleme.
//! Uygulanmadığı yerler: S4'ün sabit tekrarları (bind turu 3 × 1 s, yoklama 3 × 1 s) ve T3. `seed` =
//! bind sonrası son kabul edilen bind'in `client_nonce`'u, bind öncesi o denemenin `client_nonce`'u.
//! `dest_hash` tohum DEĞİLDİR. AMENDMENT-10 madde 1'in "500 ms, ×2" prob merdiveni uygulanmaz: hub
//! istemcisinin kadansı S4 C2/C6/C8'dir.

use core::time::Duration;
use std::time::Instant;

use blake2::{Blake2s256, Digest};

use super::codec::{self, NONCE_LEN};

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

/// KK-H48 jitter'ı: `[0, step/4)` içinde, aynı `(seed, n, step)` için aynı değer.
///
/// Geri alma: bu işlev sıfır döndürülerek jitter kapatılır (tek yer).
#[must_use]
pub fn jitter(seed: &[u8; NONCE_LEN], n: u32, step: Duration) -> Duration {
    let window_ms = u64::try_from(step.as_millis() / 4).unwrap_or(u64::MAX);
    if window_ms == 0 {
        return Duration::ZERO;
    }
    let mut h = Blake2s256::new();
    h.update(seed);
    h.update(n.to_be_bytes());
    let out = h.finalize();
    let mut head = [0u8; 4];
    head.copy_from_slice(&out[..4]);
    Duration::from_millis(u64::from(u32::from_be_bytes(head)) % window_ms)
}

/// Heartbeat makinesinin bir adımının sonucu.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HbOutcome {
    Nothing,
    Send(Vec<u8>),
    /// Cevapsız aktif yoklama: "koptu".
    Disconnected,
}

enum State {
    /// Sessizlik bekleniyor; vade = son giden + 8 s + jitter(`seq`).
    Idle,
    /// Bir ya da daha çok heartbeat cevap bekliyor.
    Waiting {
        /// Cevap bekleyen tokenlar (≤ 1 + PROBE_COUNT).
        outstanding: Vec<u32>,
        probes: u32,
        next: Instant,
    },
}

/// Bağlıyken çalışan heartbeat makinesi.
pub(crate) struct Heartbeat {
    state: State,
    /// Heartbeat sıra numarası (jitter `deneme` girdisi).
    seq: u32,
    next_token: u32,
}

impl Heartbeat {
    pub(crate) fn new(token_base: u32) -> Self {
        Self {
            state: State::Idle,
            seq: 0,
            next_token: token_base,
        }
    }

    /// Yeni bind: sessizlik ölçümünden başla (yoklama durumu silinir).
    pub(crate) fn reset(&mut self) {
        self.state = State::Idle;
    }

    fn idle_due(&self, last_tx: Instant, seed: &[u8; NONCE_LEN]) -> Instant {
        last_tx + HEARTBEAT_FOREGROUND + jitter(seed, self.seq, HEARTBEAT_FOREGROUND)
    }

    pub(crate) fn next_deadline(&self, last_tx: Instant, seed: &[u8; NONCE_LEN]) -> Instant {
        match &self.state {
            State::Idle => self.idle_due(last_tx, seed),
            State::Waiting { next, .. } => *next,
        }
    }

    fn token(&mut self) -> u32 {
        let t = self.next_token;
        self.next_token = self.next_token.wrapping_add(1);
        t
    }

    pub(crate) fn on_tick(
        &mut self,
        now: Instant,
        last_tx: Instant,
        seed: &[u8; NONCE_LEN],
    ) -> HbOutcome {
        match &self.state {
            State::Idle => {
                if now < self.idle_due(last_tx, seed) {
                    return HbOutcome::Nothing;
                }
                self.seq = self.seq.wrapping_add(1);
                let token = self.token();
                self.state = State::Waiting {
                    outstanding: vec![token],
                    probes: 0,
                    next: now + ACK_WAIT,
                };
                HbOutcome::Send(codec::encode_heartbeat(token))
            }
            State::Waiting { probes, next, .. } => {
                if now < *next {
                    return HbOutcome::Nothing;
                }
                if *probes >= PROBE_COUNT {
                    self.state = State::Idle;
                    return HbOutcome::Disconnected;
                }
                let token = self.token();
                if let State::Waiting {
                    outstanding,
                    probes,
                    next,
                } = &mut self.state
                {
                    if outstanding.len() > PROBE_COUNT as usize {
                        outstanding.remove(0);
                    }
                    outstanding.push(token);
                    *probes += 1;
                    *next = now + PROBE_INTERVAL;
                }
                HbOutcome::Send(codec::encode_heartbeat(token))
            }
        }
    }

    /// Eşleşen ACK → sessizlik ölçümüne dön. Döndürür: eşleşti mi.
    pub(crate) fn on_ack(&mut self, token: u32) -> bool {
        match &self.state {
            State::Waiting { outstanding, .. } if outstanding.contains(&token) => {
                self.state = State::Idle;
                true
            }
            _ => false,
        }
    }

    /// Soket hatası (HB2): bekleyen yoklama yoksa hemen aktif yoklama başlar.
    pub(crate) fn start_probe(&mut self, now: Instant) {
        if let State::Idle = self.state {
            self.state = State::Waiting {
                outstanding: Vec::with_capacity(PROBE_COUNT as usize + 1),
                probes: 0,
                next: now,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_is_bounded_and_deterministic() {
        let step = HEARTBEAT_FOREGROUND;
        for n in 0..200 {
            let a = jitter(&[n as u8; 16], n, step);
            assert!(a < Duration::from_secs(2), "pencere %25 = 2 s");
            assert_eq!(a, jitter(&[n as u8; 16], n, step));
        }
        // Farklı tohum / deneme genelde farklı değer verir (sabit değil).
        let set: std::collections::BTreeSet<_> =
            (0..32).map(|n| jitter(&[7; 16], n, step)).collect();
        assert!(set.len() > 16);
        assert_eq!(
            jitter(&[1; 16], 0, Duration::from_millis(3)),
            Duration::ZERO
        );
    }

    #[test]
    fn probe_series_then_disconnect() {
        let t0 = Instant::now();
        let seed = [3; 16];
        let mut hb = Heartbeat::new(100);
        let due = hb.next_deadline(t0, &seed);
        assert!(due >= t0 + Duration::from_secs(8) && due < t0 + Duration::from_secs(10));
        assert_eq!(
            hb.on_tick(due - Duration::from_millis(1), t0, &seed),
            HbOutcome::Nothing
        );
        assert_eq!(
            hb.on_tick(due, t0, &seed),
            HbOutcome::Send(codec::encode_heartbeat(100))
        );
        let mut t = due;
        for k in 1..=3u32 {
            t += Duration::from_secs(1);
            assert_eq!(
                hb.on_tick(t, due, &seed),
                HbOutcome::Send(codec::encode_heartbeat(100 + k))
            );
        }
        t += Duration::from_secs(1);
        assert_eq!(hb.on_tick(t, due, &seed), HbOutcome::Disconnected);
    }

    #[test]
    fn any_outstanding_ack_ends_probe() {
        let t0 = Instant::now();
        let mut hb = Heartbeat::new(7);
        hb.start_probe(t0);
        assert_eq!(
            hb.on_tick(t0, t0, &[0; 16]),
            HbOutcome::Send(codec::encode_heartbeat(7))
        );
        assert!(!hb.on_ack(99));
        assert!(hb.on_ack(7));
        assert!(matches!(hb.state, State::Idle));
    }
}
