//! T3 — INIT tekrarının sabitleri ve aralık dizisi (S4 C3, KR-18, §6; KK-H35).
//!
//! Sabitler TEK yerdedir (bu dosya; v4 Y8). `kama_noise_wire::init_flight`'taki "5 retry, 200 ms,
//! ≤ 10 s" ürün yolunda değildir ve DEĞİŞMEZ.
//!
//! Aralıklar S4 C3'teki gibi AYNEN uygulanır (jitter YOK; KK-H48 "uygulanmadığı yerler"):
//! `1, 2, 4, 8, 16, 30, 30, …` s. `PEER_UNBOUND` / `RATE_LIMITED` / `HUB_CAPACITY` sonrasında bir
//! sonraki tekrar en erken [`BACKOFF_MIN`] sonradır.

use core::time::Duration;

/// İlk tekrar aralığı.
pub const T3_INITIAL: Duration = Duration::from_secs(1);
/// Her tekrarda çarpan.
pub const T3_FACTOR: u32 = 2;
/// Aralık tavanı.
pub const T3_CAP: Duration = Duration::from_secs(30);
/// `PEER_UNBOUND` / `RATE_LIMITED` / `HUB_CAPACITY` sonrası asgari bekleme ("mevcut geri çekilme ≥ 5 s").
pub const BACKOFF_MIN: Duration = Duration::from_secs(5);
/// Tamamlanmamış açılış denemesinin azami yaşı (S4 §6). Yalnız o deneme biter; kurulmuş oturuma
/// uygulanmaz.
pub const ATTEMPT_MAX_AGE: Duration = Duration::from_secs(3600);

/// `n`'inci tekrardan ÖNCEKİ bekleme (`n = 0` ilk gönderimden sonraki ilk tekrar): `1 s × 2ⁿ`,
/// tavan 30 s. Taşma yok: üs tavanın ötesinde doyar.
#[must_use]
pub fn t3_delay(n: u32) -> Duration {
    // 2^5 = 32 s > 30 s: n ≥ 5 her zaman tavandır; checked_pow taşmayı da kapatır.
    T3_FACTOR
        .checked_pow(n)
        .and_then(|m| T3_INITIAL.checked_mul(m))
        .map_or(T3_CAP, |d| d.min(T3_CAP))
}

/// Durum yanıtı geri çekilme isterse aralığı en az [`BACKOFF_MIN`]'e çeker.
#[must_use]
pub fn with_backoff(delay: Duration) -> Duration {
    delay.max(BACKOFF_MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t3_sequence_is_1_2_4_8_16_30_30() {
        let got: Vec<u64> = (0..8).map(|n| t3_delay(n).as_secs()).collect();
        assert_eq!(got, vec![1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(t3_delay(u32::MAX), T3_CAP);
    }

    #[test]
    fn backoff_is_at_least_five_seconds() {
        assert_eq!(with_backoff(Duration::from_secs(1)), BACKOFF_MIN);
        assert_eq!(
            with_backoff(Duration::from_secs(16)),
            Duration::from_secs(16)
        );
        assert_eq!(ATTEMPT_MAX_AGE, Duration::from_secs(3600));
    }
}
