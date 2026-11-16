//! [HAT-KORUMASI] Geri bildirimi aç profilde gerçek ileri hattın ölçülmesi: ölçüm profilin hat
//! hızını yalnız DÜŞÜRÜR, hiçbir zaman profilin üstüne çıkarmaz.
//!
//! # Sorun
//!
//! Geri bildirimi aç profilde ([`crate::sender::FeedbackStarvedRule`]) gönderici hızı ve hat
//! modelini profilin yukarı yön hızına sert bağlar; uçuş tabanı `hat × (geri bildirim ufku + RTT)`
//! olur (62 B/s dönüşte ≈ 11 s × hat — bütün dosyadan büyük). Profil gerçek hattan iyimserse
//! göndericinin hiçbir sınırı gerçek hatta inmez: darboğaz kuyruğu hat farkı hızında büyür, PTT ve
//! metin kuyruğun arkasında saniyelerce bekler, kuyruk taşınca paketler düşer.
//!
//! # Neden BBR'ın bant tahmini (BtlBw) değil
//!
//! BtlBw teslim hızıdır (ACK'lenen bayt / ACK aralığı). Bu hatta iki nedenle hattı ölçmez:
//! * ileri yönde rastgele kayıp (ürün hücresinde %20) hattı kullanır ama teslim edilmez: teslim
//!   hızı en iyi hâlde hattın `1 − kayıp` katıdır;
//! * ACK'ler 62 B/s dönüşte saniyelerce aralıkla ve kendi serileşmeleri (65–129 B → 1,05–2,08 s)
//!   kadar kayarak gelir; örnek aralığı ACK varış anlarının farkıdır.
//!
//! Ölçüldü (ürün testi, tohum 1): profil 12 500 B/s iken BtlBw gerçek 12 500 B/s hatta 3,9 kB/s,
//! 6 250 B/s hatta 2,8–2,9 kB/s okudu. Tavan olarak kullanılsa dürüst profili üçte bire düşürürdü.
//!
//! # Ölçüm: gönderim cephesinin alıcıya varış gecikmesi
//!
//! Her ACK çerçevesinin en büyük teyitli iletimi `L` (gönderim anı `s`, o ana dek bu motorun hatta
//! verdiği kümülatif tel baytı `c`) alıcıya `r` anında varmıştır. Darboğaz FIFO'dur: `L`'den önce
//! gönderilen her bayt `r`'den önce hattı geçmiş (teslim edilmiş ya da yolda rastgele düşmüş)
//! ya da kuyrukta düşmüştür. Gönderici `r`'yi bilmez ama ACK'in kendi alanlarından tam türetir:
//!
//! ```text
//! τ = ACK varışı − ack_delay − ACK'in dönüş hattında serileşmesi
//!   = r + dönüş yayılımı            (alıcı ACK'i dönüş hattı boşken çıkarır; bkz. aşağı)
//! ```
//!
//! * `ack_delay` alıcının `L`'yi aldıktan sonra ACK'i tuttuğu süredir (bütçe, hatta yol verme);
//! * ACK'in serileşmesi = (ACK datagramı + UDP/IP zarfı) / profilin dönüş hızı; ACK boyu çalışma
//!   zamanından gelir (`Sender::observe_ack_feedback`).
//!
//! İki ACK noktası `j < i` arasında `Δs = s_i − s_j` gönderim süresi, `Δτ = τ_i − τ_j` alıcıdaki
//! varış süresidir. `Δτ − Δs` cephenin kuyrukta beklediği sürenin artışıdır. Artış ölçüm
//! çözünürlüğünü aşıyorsa darboğaz o aralıkta doluydu ve aralıkta gönderilen her bayt hattan
//! geçti: hat hızı `(c_i − c_j) / Δτ`. Rastgele kayıp tahmini düşürmez (düşen bayt hattı kullandı);
//! artış yoksa örnek yoktur (hat en az gönderim hızı kadardır, ama ne kadar fazla olduğu bilinmez).
//!
//! # Gürültünün yönü
//!
//! Dönüş hattında beklemenin (ACK sıkışması) iki ucu vardır:
//! * başlangıç noktası `j`'nin beklemesi `τ_j`'yi büyütür → `Δτ` küçülür → tahmin YÜKSELİR;
//! * bitiş noktası `i`'nin beklemesi tahmini düşürür. Ürün alıcısı bütçeli ACK'i yukarı yön hattı
//!   kendi baytlarından boşalmadan çıkarmaz (`Receiver::set_uplink_yield`); bekleme `ack_delay`'in
//!   içindedir. Kalan sıkışma için tahmin, saklanan bütün başlangıç noktalarına göre aralıkların
//!   EN BÜYÜĞÜdür: tek bir gecikmiş ACK tahmini aşağı çekmez, aralıktaki boşta kalma (hat boş,
//!   gönderici bekliyor) da yalnız o aralığı düşürür.
//! * Kuyrukta düşen bayt aralığın gönderim baytına girer ama hattı kullanmamıştır → tahmin yükselir.
//!   Ölçüldü (ürün testi, gerçek 6 250 B/s, tohum 2): kuyruk taşarken gönderilmiş baytları kapsayan
//!   aralık 10 024 B/s verdi; önceki temiz aralık 6 249 B/s.
//!
//! Yükselten hatalar korumayı kaldırabilir, düşürenler yalnız hızdan yer. Bu yüzden korunan hız
//! oturum boyunca ARTMAZ: her kabul edilen örnek `min(önceki, örnek)` olarak işlenir; profil hattı
//! üst sınırdır. Kabul edilen bir örnek, aralığın gönderim hızının yaklaşık %5 altındadır (kuyruk
//! artışı eşiği): gürültü düzeyindeki artış örnek vermez, hız yalnız gerçek kanıtla iner. Bedeli:
//! oturum içinde hızlanan hat yeniden ölçülmez (yeni oturum profil hattından başlar).
//!
//! # Kuyruk boşaltma
//!
//! Korumanın devreye girdiği ana dek gönderici profil hızında göndermiştir; gerçek hatta o farkın
//! kuyruğu birikmiştir. Hız yalnız ölçülen hatta inerse kuyruk kalıcı olur ve sonraki her PTT
//! demeti onun arkasında bekler. Bu yüzden her ölçüm noktasında darboğaz kuyruğu da tahmin edilir:
//! `L`'nin hattı terk ettiği andan bu yana hat `oran × süre` bayt boşaltmıştır; `L`'den sonra
//! gönderilen baytların kalanı kuyruktadır. Gönderici bu kuyruğu kendi hat modeline
//! (`TokenBucket::line_raise_backlog`) yazar: yeni datagram, gerçek kuyruk boşalınca çıkar.
//! `L`'nin hattı terk ettiği an `τ − taban`'dır; taban, görülen en küçük `τ − s` (kuyruksuz iki
//! yönlü yayılım; oturumun ilk paketleri boş hatta gider).
//!
//! Bayt sayımı kuyrukta düşmüş baytı da kuyrukta sayar (gönderici hangi baytın düştüğünü bilmez);
//! kuyruk taşmışsa tahmin gerçek kuyruktan büyüktür ve gönderici gereğinden uzun bekler — PTT ve
//! metin de bekler. Ölçüldü (ürün testi, gerçek 6 250 B/s, tohum 2–3): kuyruk ≈ 3 s iken bekleme
//! 5,5–6 s. Bu yüzden yazılan kuyruk ayrıca `L`'nin kendisinin kuyrukta beklediği süreyle
//! (`τ − s − taban`) sınırlıdır: gönderici `L`'den bu yana en az hat hızında gönderdiyse kuyruk o
//! andan küçülmemiştir, bu değer gerçek kuyruğun alt sınırıdır. Sınır, kuyruk `L`'den sonra da
//! büyüdüyse eksik boşaltır; kalan, bir sonraki ölçüm noktasında yine ölçülür ve yazılır.
//!
//! Determinizm: tamsayı aritmetiği, RNG yok, sınırlı bellek (ADR-17 / SR-AUDIT-001).

use std::collections::VecDeque;

use crate::pacer::LineModel;

/// Gönderim kaydı tavanı (SR-AUDIT-001). Tavan dolunca en eski kayıt düşer; cephesi düşmüş bir
/// kayda çarpan ACK örnek vermez (tahmin yalnız zayıflar).
const SENT_CAP: usize = 8192;
/// Saklanan ACK noktası (aralık başlangıcı) sayısı.
const POINT_CAP: usize = 8;
/// Kuyruk artışı en az aralığın bu kesri olmalı (1/20 = %5): daha küçük artış ölçüm
/// çözünürlüğündedir (zamanlayıcı, ACK gecikmesi kuantumu, hattı paylaşan küçük denetim
/// datagramları) ve örnek vermez.
const GROWTH_MIN_DIV: u64 = 20;

/// Ölçüm sonucu: korunan hat hızı ve darboğazda tahmin edilen kuyruk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineGuardUpdate {
    /// Korunan hat hızı (B/s, tel baytı — zarf dahil); profil hattını aşmaz.
    pub rate_bps: u64,
    /// Darboğaz kuyruğunda şu an beklediği tahmin edilen kendi baytlarımız (zarf dahil).
    pub backlog_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct SentRec {
    seq: u64,
    send_ns: u64,
    /// Bu iletim dahil hatta verilen kümülatif tel baytı.
    cum: u64,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    tau_ns: u64,
    send_ns: u64,
    cum: u64,
}

/// Geri bildirimi aç profilde gerçek ileri hattın tahmincisi (modül belgesi).
#[derive(Debug, Clone)]
pub struct LineGuard {
    /// Profilin yukarı yön hattı (üst sınır).
    line: LineModel,
    /// Profilin dönüş hattı (ACK serileşmesi için).
    return_line: LineModel,
    /// Hatta verilen kümülatif tel baytı (motorun iletimleri + motor dışı datagramlar).
    cum: u64,
    sent: VecDeque<SentRec>,
    points: VecDeque<Point>,
    /// Son ölçüm noktasının cephesi (seq); cephe ilerlemeyen ACK nokta üretmez.
    frontier: Option<u64>,
    /// Görülen en küçük `τ − s` (ns).
    base_ns: Option<u64>,
    /// Bir sonraki ACK çerçevesinin tel boyu (UDP gövdesi; `note_ack_wire`).
    pending_wire: Option<u64>,
    /// Korunan hat hızı (B/s, tel); `None` = ölçüm hattı profilin altında göstermedi. Artmaz.
    rate_bps: Option<u64>,
    /// Kabul edilen örnek sayısı (gözlem).
    samples: u64,
}

impl LineGuard {
    /// Profilin ileri (`line`) ve dönüş (`return_line`) hatlarıyla boş tahminci.
    #[must_use]
    pub fn new(line: LineModel, return_line: LineModel) -> Self {
        Self {
            line,
            return_line,
            cum: 0,
            sent: VecDeque::new(),
            points: VecDeque::new(),
            frontier: None,
            base_ns: None,
            pending_wire: None,
            rate_bps: None,
            samples: 0,
        }
    }

    /// Motorun bir iletimi hatta verildi (`udp_bytes` = UDP gövdesi; zarf burada eklenir).
    pub fn on_sent(&mut self, seq: u64, now_ns: u64, udp_bytes: u64) {
        self.cum = self
            .cum
            .saturating_add(udp_bytes.saturating_add(self.line.datagram_overhead));
        if self.sent.len() >= SENT_CAP {
            self.sent.pop_front();
        }
        self.sent.push_back(SentRec {
            seq,
            send_ns: now_ns,
            cum: self.cum,
        });
    }

    /// Motorun kararı olmadan bu uçtan aynı hatta çıkan datagram (ör. PARAM): hattı kullanır,
    /// sonraki iletimlerin kümülatif baytına girer.
    pub fn on_unsequenced(&mut self, udp_bytes: u64) {
        self.cum = self
            .cum
            .saturating_add(udp_bytes.saturating_add(self.line.datagram_overhead));
    }

    /// Bir sonraki ACK çerçevesini taşıyan datagramın tel boyu (UDP gövdesi).
    pub fn note_ack_wire(&mut self, udp_bytes: u64) {
        self.pending_wire = Some(udp_bytes);
    }

    /// Korunan hat hızı (B/s, tel); `None` = koruma devrede değil.
    #[must_use]
    pub fn rate_bps(&self) -> Option<u64> {
        self.rate_bps
    }

    /// Kabul edilen örnek sayısı (gözlem).
    #[must_use]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// Bir ACK çerçevesi işlendi: `largest_seq` çerçevenin en büyük teyitli iletimi, `ack_delay_ns`
    /// çerçevenin bildirdiği tutma süresi, `saturated` tutma süresi kodlama tavanında mı.
    /// Koruma devredeyse güncel hız ve kuyruk tahmini döner.
    pub fn on_ack(
        &mut self,
        now_ns: u64,
        ack_delay_ns: u64,
        saturated: bool,
        largest_seq: u64,
    ) -> Option<LineGuardUpdate> {
        let wire = self.pending_wire.take()?;
        if saturated || self.frontier.is_some_and(|f| largest_seq <= f) {
            return None;
        }
        let i = self
            .sent
            .binary_search_by_key(&largest_seq, |r| r.seq)
            .ok()?;
        let rec = self.sent[i];
        let ser_ns = ns_for(
            wire.saturating_add(self.return_line.datagram_overhead),
            self.return_line.rate_bps,
        )?;
        let tau = now_ns.checked_sub(ack_delay_ns)?.checked_sub(ser_ns)?;
        let lag = tau.checked_sub(rec.send_ns)?;
        self.frontier = Some(largest_seq);
        self.base_ns = Some(self.base_ns.map_or(lag, |b| b.min(lag)));
        // Kayıtlar artık bu cepheden eskiye gerekmez (sonraki ACK'ler daha büyük cephe ister).
        self.sent.drain(..i);

        let current = self.rate_bps.unwrap_or(self.line.rate_bps);
        let mtu_ns = ns_for(
            crate::rate::DEFAULT_MTU + self.line.datagram_overhead,
            current,
        )
        .unwrap_or(0);
        let mut best: Option<u64> = None;
        for p in &self.points {
            let (Some(ds), Some(dtau)) = (
                rec.send_ns.checked_sub(p.send_ns),
                tau.checked_sub(p.tau_ns),
            ) else {
                continue;
            };
            if ds == 0 {
                continue;
            }
            let growth = dtau.saturating_sub(ds);
            if growth <= mtu_ns.max(ds / GROWTH_MIN_DIV) {
                continue;
            }
            let bytes = rec.cum.saturating_sub(p.cum);
            let est = u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(dtau))
                .unwrap_or(u64::MAX);
            best = Some(best.map_or(est, |b| b.max(est)));
        }
        if self.points.len() >= POINT_CAP {
            self.points.pop_front();
        }
        self.points.push_back(Point {
            tau_ns: tau,
            send_ns: rec.send_ns,
            cum: rec.cum,
        });
        if let Some(est) = best {
            let est = est.clamp(1, self.line.rate_bps);
            self.rate_bps = Some(self.rate_bps.map_or(est, |r| r.min(est)));
            self.samples = self.samples.saturating_add(1);
        }
        let rate = self.rate_bps?;
        // `L` hattı `τ − taban` anında terk etti; o andan bu yana hat `rate × süre` boşalttı.
        let since_ns = now_ns
            .saturating_sub(tau)
            .saturating_add(self.base_ns.unwrap_or(0));
        let drained = u64::try_from(u128::from(rate) * u128::from(since_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        let by_bytes = self.cum.saturating_sub(rec.cum).saturating_sub(drained);
        // `L`'nin kuyrukta beklediği süre kadar hat baytı (gerçek kuyruğun alt sınırı).
        let queued_ns = lag.saturating_sub(self.base_ns.unwrap_or(lag));
        let by_delay = u64::try_from(u128::from(rate) * u128::from(queued_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        Some(LineGuardUpdate {
            rate_bps: rate,
            backlog_bytes: by_bytes.min(by_delay),
        })
    }
}

/// `bytes` baytın `rate_bps` hızında serileşmesi (ns); hız 0 ise `None`.
fn ns_for(bytes: u64, rate_bps: u64) -> Option<u64> {
    if rate_bps == 0 {
        return None;
    }
    u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(rate_bps)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;
    const SEC: u64 = 1_000_000_000;
    const PROFILE: u64 = 12_500;
    const RET: u64 = 62;
    const OVH: u64 = 28;

    fn guard() -> LineGuard {
        LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
        )
    }

    /// Profil hızında `n` × 1 172 B gövde (1 200 B tel) gönderir, gerçek hat `real` B/s, ileri
    /// yayılım 300 ms; her iletimin alıcıya varış anını döner.
    fn send_into(g: &mut LineGuard, real: u64, n: u64) -> Vec<u64> {
        let mut free = 0u64;
        let mut arrivals = Vec::new();
        for k in 0..n {
            let s = k * 1200 * SEC / PROFILE;
            g.on_sent(k, s, 1172);
            let done = free.max(s) + 1200 * SEC / real;
            free = done;
            arrivals.push(done + 300 * MS);
        }
        arrivals
    }

    /// Alıcı `seq`'i aldığında hemen `wire` B'lik ACK çıkarır; ACK 300 ms + serileşmeyle gelir.
    fn ack(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        wire: u64,
    ) -> (u64, Option<LineGuardUpdate>) {
        let at = arrivals[seq as usize] + 300 * MS + (wire + OVH) * SEC / RET;
        g.note_ack_wire(wire);
        (at, g.on_ack(at, 0, false, seq))
    }

    #[test]
    fn honest_line_gives_no_sample_even_with_unequal_ack_sizes() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 60);
        // 37 B ve 101 B: ACK serileşmesi 1,05 s ↔ 2,08 s. Düzeltmesiz bu 1 s'lik sahte kuyruk
        // artışı olurdu.
        for (seq, wire) in [(1, 37), (25, 101), (50, 55)] {
            let (_, u) = ack(&mut g, &arr, seq, wire);
            assert_eq!(u, None, "dürüst hatta koruma devreye girmez");
        }
        assert_eq!(g.rate_bps(), None);
    }

    #[test]
    fn slow_line_is_measured_within_one_percent_and_never_above_profile() {
        for real in [6_250u64, 10_000] {
            let mut g = guard();
            let arr = send_into(&mut g, real, 60);
            let _ = ack(&mut g, &arr, 1, 37);
            let (_, u) = ack(&mut g, &arr, 20, 101);
            let u = u.expect("kuyruk büyüdü → ölçüm");
            assert!(
                u.rate_bps.abs_diff(real) * 100 <= real,
                "gerçek {real} B/s, tahmin {} B/s",
                u.rate_bps
            );
            assert!(u.rate_bps <= PROFILE);
        }
    }

    /// Yazılan kuyruk gerçek kuyruğu aşmaz ve cephenin kendi beklediği kuyruktan az değildir;
    /// kuyrukta düşen bayt kuyruk sayılmaz.
    #[test]
    fn backlog_estimate_never_exceeds_the_bottleneck_queue() {
        for dropped in [0u64, 20_000] {
            let real = 6_250u64;
            let mut g = guard();
            let mut free = 0u64;
            let mut arr = Vec::new();
            for k in 0..60u64 {
                let s = k * 1200 * SEC / PROFILE;
                if k == 25 {
                    g.on_unsequenced(dropped);
                }
                g.on_sent(k, s, 1172);
                free = free.max(s) + 1200 * SEC / real;
                arr.push(free + 300 * MS);
            }
            let _ = ack(&mut g, &arr, 0, 37);
            let (at, u) = ack(&mut g, &arr, 20, 101);
            let u = u.expect("ölçüm");
            // Gerçek kuyruk `at` anında: gönderilmiş ama hattı henüz terk etmemiş iletimler.
            let sent_by = (0..60u64)
                .filter(|k| k * 1200 * SEC / PROFILE <= at)
                .count() as u64;
            let left_by = arr.iter().filter(|&&a| a - 300 * MS <= at).count() as u64;
            let queue = (sent_by - left_by) * 1200;
            // Cephe (seq 20) kuyrukta `varış − 300 ms − gönderim − kendi serileşmesi` bekledi.
            let own_wait = arr[20] - 300 * MS - 20 * 1200 * SEC / PROFILE - 1200 * SEC / real;
            let floor = real * own_wait / SEC;
            assert!(
                u.backlog_bytes <= queue && u.backlog_bytes + 1200 >= floor,
                "düşen {dropped} B: kuyruk {queue} B, cephenin kuyruğu {floor} B, yazılan {} B",
                u.backlog_bytes
            );
        }
    }

    /// Kuyruk taşarken gönderilmiş baytları kapsayan aralık (gönderim baytı hattın taşıdığından
    /// fazla) korunan hızı yükseltemez.
    #[test]
    fn inflated_span_cannot_raise_the_guard() {
        let real = 6_250u64;
        let mut g = guard();
        let mut free = 0u64;
        let mut arr = Vec::new();
        for k in 0..60u64 {
            let s = k * 1200 * SEC / PROFILE;
            if k == 25 {
                // 20 000 B gönderildi ama kuyrukta düştü: hattı kullanmadı.
                g.on_unsequenced(20_000);
            }
            g.on_sent(k, s, 1172);
            free = free.max(s) + 1200 * SEC / real;
            arr.push(free + 300 * MS);
        }
        let _ = ack(&mut g, &arr, 1, 37);
        let (_, u) = ack(&mut g, &arr, 20, 101);
        let first = u.expect("temiz aralık").rate_bps;
        assert!(first.abs_diff(real) * 100 <= real, "temiz ölçüm {first}");
        // 20 → 50 aralığı düşen baytı içerir: tek başına ≈ 9,7 kB/s verirdi.
        let (_, later) = ack(&mut g, &arr, 50, 101);
        assert_eq!(
            later.map(|u| u.rate_bps),
            Some(first),
            "şişkin aralık hızı yükseltmez"
        );
    }

    #[test]
    fn stale_or_saturated_acks_give_no_point() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 60);
        let _ = ack(&mut g, &arr, 10, 37);
        // Cephe ilerlemedi.
        let (_, u) = ack(&mut g, &arr, 10, 37);
        assert_eq!(u, None);
        // Tutma süresi kodlama tavanında: alıcının gerçek tutma süresi bilinmez.
        g.note_ack_wire(37);
        assert_eq!(g.on_ack(60 * SEC, 0, true, 30), None);
        // ACK boyu bildirilmeden gelen çerçeve nokta üretmez.
        assert_eq!(g.on_ack(60 * SEC, 0, false, 40), None);
        assert_eq!(g.rate_bps(), None);
    }
}
