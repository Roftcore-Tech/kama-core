//! [CC-PORT DENEY — ürün kuralı DEĞİL] Kayıp sınıflandırma: kuyruk gecikmesi eşliğindeki kayıp
//! tıkanıklık sayılır, kuyruk gecikmesi yokken kayıp sayılmaz (ISEMRI-CC-PORT v3 §3 deney kolu).
//!
//! `srtt − min_rtt`, kaybın nedenini tek başına belirlemez: kısa kuyruk taşması yumuşatılmış RTT'ye
//! yansımayabilir, yol gecikmesi değişimi kuyruk gibi görünebilir. Bu yüzden yalnız
//! [`crate::sender::Sender::with_cc_port_loss_filter`] kurucusunda etkindir ve Ö-CC'de referans
//! portla aynı hücrelerde yan yana ölçülür; sahip ancak ölçümden sonra INV-RATE-1'e karar verir.

/// Kuyruk gecikmesi eşiği: `max(QD_FLOOR_NS, QD_FRAC_PM‰ × min_rtt)`.
pub const QD_FLOOR_NS: u64 = 50_000_000;
/// min_rtt'nin binde kaçı (250‰ = %25).
pub const QD_FRAC_PM: u64 = 250;

/// `true` ⇒ kayıp tıkanıklık sayılır (BBR `on_congestion_event` çağrılır).
/// `srtt_ns == 0` ya da `min_rtt_ns == 0` (henüz örnek yok) ⇒ `true` (referans davranış; kör
/// durumda kaybı görmezden gelmek güvenli değil).
#[must_use]
pub fn is_congestive(srtt_ns: u64, min_rtt_ns: u64) -> bool {
    if srtt_ns == 0 || min_rtt_ns == 0 {
        return true;
    }
    let qd = srtt_ns.saturating_sub(min_rtt_ns);
    let thresh = QD_FLOOR_NS.max(min_rtt_ns / 1000 * QD_FRAC_PM);
    qd >= thresh
}

#[cfg(test)]
mod tests {
    use super::*;
    const MS: u64 = 1_000_000;

    #[test]
    fn random_loss_without_queue_delay_is_not_congestive() {
        // min 300 ms, srtt 310 ms: qd 10 ms < max(50, 75) ms.
        assert!(!is_congestive(310 * MS, 300 * MS));
    }

    #[test]
    fn loss_with_queue_delay_is_congestive() {
        // min 300 ms, srtt 400 ms: qd 100 ms ≥ 75 ms.
        assert!(is_congestive(400 * MS, 300 * MS));
    }

    #[test]
    fn mixed_near_threshold_and_blind_cases() {
        // min 100 ms: eşik max(50, 25) = 50 ms; srtt 149 → değil, 150 → evet.
        assert!(!is_congestive(149 * MS, 100 * MS));
        assert!(is_congestive(150 * MS, 100 * MS));
        // Örnek yokken referans davranış (tıkanıklık say).
        assert!(is_congestive(0, 0));
        assert!(is_congestive(300 * MS, 0));
    }
}
