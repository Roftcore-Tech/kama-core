//! [VARIŞ-HIZI · §2.2] ACK V2 `arrival_rate` alanının ölçümü: oturumun ilk varış penceresi.
//!
//! # Sorun
//!
//! Geri bildirimi aç profilde (62 B/s dönüş, %25 ACK bütçesi) gönderici gerçek ileri hattı ancak
//! ikinci ACK'le ölçebiliyordu ([`crate::line_guard`]: iki ACK cephesi gerekir). İkinci ACK
//! dönüş bütçesiyle 3–5 s sonra gelir; bu sürede profilden yavaş bir hatta darboğaz kuyruğu
//! saniyelerce büyür ve PTT kuyruğun arkasında bekler (ürün testi, gerçek 6 250 / beyan 12 500,
//! tohum 1: ilk ölçüm 4,60 s'de, kuyruk 3,4 s, kuyrukta 41 datagram düştü).
//!
//! # Ölçüm
//!
//! Alıcı oturumun ilk datagramından sonra gelen datagramların UDP gövde baytını sayar. Sayım
//! [`FIRST_WINDOW_BYTES`]'a ulaştığı varışta pencere kapanır. Alan değeri
//! `(ilk datagramdan sonraki bayt) / (son varış − ilk varış)` B/s'dir (§2.2 "receiver-gözlemli
//! pencere-oranı, ilk bw örneği"). Bu değer yalnız oturumun ilk ACK'inde taşınır, sonraki ACK'ler
//! sentinel taşır ([`crate::ack::ARRIVAL_RATE_SENTINEL`]).
//!
//! Pencere bayta göre tanımlıdır, zamana göre değil. Gönderici ACK'in teyit ettiği seq'leri
//! sıralayıp aynı kuralı ([`window_end`]) kendi gönderim kaydına uygular ve pencerenin son seq'ini
//! bulur. Böylece ilk ACK dönüş hattında başka veriye yol verip geç çıksa da (ör. alındı) iki uç
//! aynı datagram kümesini konuşur. Gönderici pencere süresini `bayt / alan` olarak geri kurar ve
//! bunu [`crate::line_guard::LineGuard`]'ın iki nokta tahmincisine verir: kuyruk artışı varsa hat
//! `pencerede gönderilen tel baytı / pencere süresi`; artış yoksa ve pencere tavana yakın
//! gönderildiyse hat en az o hızdadır (doğrulama).
//!
//! Neden alan alıcı baytıyla değil göndericinin baytıyla hız vermiyor: rastgele kayıp darboğazdan
//! sonra olduğunda düşen datagram hattı kullanmıştır ama alıcı onu görmez. Yalnız alınan baytla
//! hesaplanan oran hattı `1 − kayıp` katı okur. Alıcı kaybı göndericinin baytıyla düzeltmez
//! (kayıp nedenlerini ayıramaz); düzeltme göndericide, hat korumasının bugünkü tahmincisiyle aynı
//! kabulle yapılır (pencerede gönderilen her bayt hattı kullandı; kuyruk taşması pencerede
//! beklenmez: pencere birkaç datagramdır).
//!
//! # Varsayımlar ve sentinel
//!
//! * Darboğaz FIFO'dur, pencere sırasız varış görmez. Pencerede sırasız varış (önceki seq'ten
//!   küçük seq) olursa ölçüm geçersizdir → sentinel.
//! * Gövde boyu iki uçta aynıdır: alıcının `recv_from` boyu = göndericinin birim boyu (mühürlü UDP
//!   gövdesi). FEC onarım sembolü birkaç bayt farklı olabilir; ilk pencerede onarım sembolü
//!   beklenmez (blok kapanmadan çıkmaz).
//! * Çift datagram (aynı seq) sayılmaz.
//! * Pencere [`FIRST_ACK_HOLD_MAX_NS`] içinde kapanmazsa ölçüm yoktur → sentinel.
//!
//! Determinizm: tamsayı aritmetiği, RNG yok, sabit bellek (ADR-17 / SR-AUDIT-001).

use crate::ack::ARRIVAL_RATE_SENTINEL;
use crate::rate::DEFAULT_MTU;

/// İlk varış penceresinin ilk datagramdan sonraki gövde baytı: yarım MTU (ürünün dosya parçasıyla
/// ilk datagramdan sonra bir parça). Üç koşuldan çıkar:
/// * pencere göndericinin ilk patlamasının içinde kapanmalıdır
///   ([`crate::line_guard::START_BURST_NS`]): patlamadan sonra gönderilen datagram pencereye
///   girerse kuyruk boşalmış olabilir ve artış görünmez;
/// * ilk ACK pencereyi beklerken dönüş hattı boş kalmalıdır: 62 B/s dönüşte alıcının uygulama
///   verisi (ör. ilk metnin alındısı, ürün testinde ilk varıştan ≈ 0,3 s sonra) ACK'in önüne
///   girerse ilk ACK saniyelerce gecikir (ölçüldü: iki parçalık pencereyle ilk ACK 1,8 s yerine
///   5,5 s'de vardı);
/// * ölçüm çözünürlüğü: yarı hızlı bir hatta (beyanın yarısı) tek parçanın kuyruk artışı bir
///   parça serileşmesi kadardır (≈ 86 ms), gerçek 10 000 B/s'de ≈ 21 ms; eşik pencerenin %5'i
///   (≈ 4 ms). Aradaki kayıp datagram ölçümü bozmaz: gönderici onun baytını kendi kaydından ekler.
/// [CAL]
pub const FIRST_WINDOW_BYTES: u64 = DEFAULT_MTU / 2;

/// İlk ACK'in pencere için en uzun bekletmesi (ilk varıştan, ns). Beyanın yarısı hızında
/// (6 250 B/s) bir parça ve üç kayıp ≈ 0,7 s sürer; daha yavaş hat ölçüm vermez, ilk ACK bu süre
/// dolunca sentinel'le çıkar. [CAL] — spec §12 [R1] etkisi: ilk ACK bugün ilk varışlarda
/// hemen çıkıyordu; bekletme sahip kararı gerektirir.
pub const FIRST_ACK_HOLD_MAX_NS: u64 = 700_000_000;

/// Alıcıda oturumun ilk varış penceresi.
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstArrivalWindow {
    /// İlk varış anı (ns); `None` = henüz boyu bilinen varış yok.
    first_ns: Option<u64>,
    /// Pencerede görülen en büyük seq (sırasız varış denetimi).
    last_seq: u64,
    /// Pencerenin son varış anı (ns).
    last_ns: u64,
    /// İlk datagramdan sonra gelen gövde baytı.
    bytes_after_first: u64,
    /// Pencere [`FIRST_WINDOW_BYTES`]'a ulaştı.
    complete: bool,
    /// Pencerede sırasız varış görüldü: ölçüm yok.
    invalid: bool,
}

impl FirstArrivalWindow {
    /// Boyu bilinen yeni (çift olmayan) bir datagram geldi.
    pub fn on_arrival(&mut self, seq: u64, udp_bytes: u64, now_ns: u64) {
        if self.complete || self.invalid {
            return;
        }
        let Some(first_ns) = self.first_ns else {
            self.first_ns = Some(now_ns);
            self.last_seq = seq;
            self.last_ns = now_ns;
            return;
        };
        if seq <= self.last_seq {
            self.invalid = true;
            return;
        }
        self.last_seq = seq;
        self.last_ns = now_ns.max(first_ns);
        self.bytes_after_first = self.bytes_after_first.saturating_add(udp_bytes);
        if self.bytes_after_first >= FIRST_WINDOW_BYTES {
            self.complete = true;
        }
    }

    /// Ölçüm başladı mı (boyu bilinen en az bir varış).
    #[must_use]
    pub fn started(&self) -> bool {
        self.first_ns.is_some()
    }

    /// İlk ACK pencere kapanana dek bekletilmeli mi: ölçüm başladı, kapanmadı, geçersiz değil ve
    /// en uzun bekletme dolmadı.
    #[must_use]
    pub fn holds_first_ack(&self, now_ns: u64) -> bool {
        match self.first_ns {
            Some(first) if !self.complete && !self.invalid => {
                now_ns < first.saturating_add(FIRST_ACK_HOLD_MAX_NS)
            }
            _ => false,
        }
    }

    /// Alan değeri (B/s, gövde baytı): pencere kapandıysa oran, değilse sentinel. Oran
    /// sentinel'e eşit ya da büyükse sentinel ("üst bantta").
    #[must_use]
    pub fn rate_field(&self) -> u16 {
        let Some(first) = self.first_ns else {
            return ARRIVAL_RATE_SENTINEL;
        };
        if !self.complete || self.invalid {
            return ARRIVAL_RATE_SENTINEL;
        }
        let span = self.last_ns.saturating_sub(first);
        if span == 0 {
            return ARRIVAL_RATE_SENTINEL;
        }
        let rate = u128::from(self.bytes_after_first) * 1_000_000_000 / u128::from(span);
        u16::try_from(rate)
            .ok()
            .filter(|&r| r != ARRIVAL_RATE_SENTINEL && r > 0)
            .unwrap_or(ARRIVAL_RATE_SENTINEL)
    }
}

/// Göndericinin pencere kuralı: ACK'in teyit ettiği seq'ler artan sırada ve her birinin gövde
/// boyu verildiğinde pencerenin ilk ve son seq'i ile ilkinden sonraki gövde baytı. Alıcının
/// [`FirstArrivalWindow`] kuralıyla aynı: ilk seq hariç bayt toplamı [`FIRST_WINDOW_BYTES`]'a
/// ulaştığı seq pencereyi kapatır. Ulaşılmazsa `None`.
pub fn window_end<I>(acked_ascending: I) -> Option<(u64, u64, u64)>
where
    I: IntoIterator<Item = (u64, u64)>,
{
    let mut it = acked_ascending.into_iter();
    let (first, _) = it.next()?;
    let mut bytes = 0u64;
    for (seq, udp) in it {
        bytes = bytes.saturating_add(udp);
        if bytes >= FIRST_WINDOW_BYTES {
            return Some((first, seq, bytes));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    #[test]
    fn window_closes_at_the_byte_threshold_and_reports_the_rate() {
        let mut w = FirstArrivalWindow::default();
        assert!(!w.started());
        w.on_arrival(0, 128, 300 * MS);
        assert!(w.holds_first_ack(300 * MS));
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
        // Küçük datagram pencereyi kapatmaz (gövde < yarım MTU).
        w.on_arrival(1, 80, 300 * MS + 30 * MS);
        assert!(w.holds_first_ack(400 * MS));
        // 1 046 B gövde, 6 250 B/s tel hızında ≈ 172,8 ms; seq 2 düştü → 2 × 172,8 ms.
        w.on_arrival(3, 1046, 300 * MS + 346 * MS);
        assert!(!w.holds_first_ack(650 * MS));
        // 1 126 B / 0,346 s = 3 254 B/s.
        assert_eq!(w.rate_field(), 3254);
        // Pencere kapandı: sonraki varış ölçümü değiştirmez.
        w.on_arrival(4, 1046, 900 * MS);
        assert_eq!(w.rate_field(), 3254);
    }

    #[test]
    fn hold_ends_at_the_cap_and_an_unfinished_window_is_the_sentinel() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(0, 1046, 0);
        w.on_arrival(1, 100, 10 * MS);
        w.on_arrival(2, 100, 20 * MS);
        assert!(w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS - 1));
        assert!(!w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS));
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn a_reordered_arrival_voids_the_window() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(5, 1046, 0);
        w.on_arrival(4, 1046, 10 * MS);
        assert!(!w.holds_first_ack(20 * MS));
        w.on_arrival(6, 4000, 30 * MS);
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn an_upper_band_rate_is_the_sentinel() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(0, 1046, 0);
        // 600 B / 1 ms = 600 kB/s > u16.
        w.on_arrival(1, 600, MS);
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn sender_rule_finds_the_same_window_as_the_receiver() {
        let acked = [
            (10u64, 128u64),
            (11, 1046),
            (13, 1046),
            (14, 1046),
            (15, 1046),
        ];
        assert_eq!(window_end(acked), Some((10, 11, 1046)));
        assert_eq!(window_end(acked[..1].iter().copied()), None);
        let small = [(10u64, 1046u64), (11, 80), (12, 80), (14, 1046)];
        assert_eq!(window_end(small), Some((10, 14, 1206)));
        assert_eq!(window_end(std::iter::empty()), None);
    }
}
